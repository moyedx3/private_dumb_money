from __future__ import annotations

import base64
import copy
import json
import os
import subprocess
import sys
import threading
import tempfile
import textwrap
import unittest
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import x25519
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

from clean_wallet.attestation import MockAttestor
from clean_wallet.blacklist import build_manifest, verify_manifest
from clean_wallet.client_encrypt import (
    build_proof_payload,
    encrypt_viewing_capability,
    extract_encryption_key_descriptor,
)
from clean_wallet.crypto import canonical_json, normalize_commitment
from clean_wallet.enclave_key import (
    _derive_capability_key,
    decrypt_viewing_capability,
    enclave_encryption_key_descriptor,
    enclave_key_attestation_hash,
    enclave_key_attestation_payload,
    public_key_for_private_key_b64,
)
from clean_wallet.lightwalletd import _compact_block_to_scanner_dict
from clean_wallet.proof import ProofRequest, create_report, verify_report
from clean_wallet.scanner import (
    BlockRange,
    ChainSource,
    EncryptedViewingCapability,
    FixtureScanner,
    ZcashViewingKeyScanner,
)
import clean_wallet.service as service
from clean_wallet.service import _parse_proof_payload

BLACKLIST_KEY = "demo-blacklist-issuer-key"
ATTESTATION_KEY = "demo-attestation-key"
SCOPE = "alice-orchard-account-0"
C1 = "1" * 64
C2 = "2" * 64
OWNED_CLEAN = "a" * 64


def manifest():
    return build_manifest(
        [C1, C2, C1],
        network="regtest",
        pool="orchard",
        issuer="issuer",
        version="v0",
        signing_key=BLACKLIST_KEY,
        created_at="2026-05-27T00:00:00Z",
    )


def request():
    return ProofRequest(
        network="regtest",
        pool="orchard",
        block_range=BlockRange(100, 110),
        viewing_scope_id=SCOPE,
    )


def make_report(fixture):
    attestor = MockAttestor(ATTESTATION_KEY, measurement="test-measurement")
    return create_report(
        request=request(),
        scanner=FixtureScanner(fixture),
        manifest=manifest(),
        blacklist_signing_key=BLACKLIST_KEY,
        attestor=attestor,
        scanner_version="test",
        timestamp_utc="2026-05-27T00:00:00Z",
    )


class CleanWalletTests(unittest.TestCase):
    def test_normalize_commitment_rejects_bad_values(self):
        self.assertEqual(normalize_commitment("0x" + C1), C1)
        with self.assertRaises(ValueError):
            normalize_commitment("abc")
        with self.assertRaises(ValueError):
            normalize_commitment("z" * 64)

    def test_blacklist_manifest_is_deterministic_and_signed(self):
        m = manifest()
        self.assertEqual(m.commitment_count, 2)
        verify_manifest(m, signing_key=BLACKLIST_KEY)
        tampered = copy.copy(m)
        object.__setattr__(tampered, "root", "0" * 64)
        with self.assertRaises(ValueError):
            verify_manifest(tampered, signing_key=BLACKLIST_KEY)

    def test_disjoint_fixture_returns_pass_without_private_fields(self):
        report = make_report(
            {
                "network": "regtest",
                "pool": "orchard",
                "outputs": [
                    {"height": 100, "viewing_scope_id": SCOPE, "commitment": OWNED_CLEAN},
                    {"height": 101, "viewing_scope_id": "bob", "commitment": C1},
                ],
            }
        )
        self.assertEqual(report["result"], "PASS")
        rendered = json.dumps(report, sort_keys=True)
        self.assertNotIn(SCOPE, rendered)
        self.assertNotIn(OWNED_CLEAN, rendered)
        verify_report(
            report=report,
            manifest=manifest(),
            blacklist_signing_key=BLACKLIST_KEY,
            attestor=MockAttestor(ATTESTATION_KEY, measurement="test-measurement"),
            allowed_measurements={"test-measurement"},
            now=datetime(2026, 5, 27, tzinfo=timezone.utc),
        )

    def test_overlap_fixture_returns_fail(self):
        report = make_report(
            {
                "network": "regtest",
                "pool": "orchard",
                "outputs": [{"height": 100, "viewing_scope_id": SCOPE, "commitment": C1}],
            }
        )
        self.assertEqual(report["result"], "FAIL")

    def test_scanner_error_is_never_pass(self):
        report = make_report({"network": "regtest", "pool": "orchard", "error": "missing blocks", "outputs": []})
        self.assertEqual(report["result"], "ERROR")
        self.assertIn("no clean-wallet claim", report["claim"])

    def test_report_tamper_is_rejected(self):
        report = make_report(
            {
                "network": "regtest",
                "pool": "orchard",
                "outputs": [{"height": 100, "viewing_scope_id": SCOPE, "commitment": OWNED_CLEAN}],
            }
        )
        report["result"] = "PASS" if report["result"] != "PASS" else "FAIL"
        with self.assertRaises(ValueError):
            verify_report(
                report=report,
                manifest=manifest(),
                blacklist_signing_key=BLACKLIST_KEY,
                attestor=MockAttestor(ATTESTATION_KEY, measurement="test-measurement"),
                allowed_measurements={"test-measurement"},
            )

    def test_wrong_measurement_is_rejected(self):
        report = make_report(
            {
                "network": "regtest",
                "pool": "orchard",
                "outputs": [{"height": 100, "viewing_scope_id": SCOPE, "commitment": OWNED_CLEAN}],
            }
        )
        with self.assertRaises(ValueError):
            verify_report(
                report=report,
                manifest=manifest(),
                blacklist_signing_key=BLACKLIST_KEY,
                attestor=MockAttestor(ATTESTATION_KEY, measurement="test-measurement"),
                allowed_measurements={"other-measurement"},
            )


class ClientViewingCapabilityEncryptionTests(unittest.TestCase):
    def _enclave_env(self):
        private_key = x25519.X25519PrivateKey.generate()
        private_bytes = private_key.private_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PrivateFormat.Raw,
            encryption_algorithm=serialization.NoEncryption(),
        )
        private_b64 = base64.b64encode(private_bytes).decode("ascii")
        return {
            "CLEAN_WALLET_ENCLAVE_KEY_ID": "attested-key-1",
            "CLEAN_WALLET_ENCLAVE_PUBLIC_KEY": public_key_for_private_key_b64(private_b64),
            "CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64": private_b64,
        }

    def test_client_encrypted_fvk_round_trips_only_inside_enclave_decrypt(self):
        env = self._enclave_env()
        descriptor = enclave_encryption_key_descriptor(env)

        envelope = encrypt_viewing_capability(
            "uview1-test-fvk-secret",
            descriptor,
            capability_type="fvk",
        )
        rendered = json.dumps(envelope, sort_keys=True)

        self.assertNotIn("uview1-test-fvk-secret", rendered)
        self.assertEqual(envelope["capability_type"], "fvk")
        decrypted = decrypt_viewing_capability(
            EncryptedViewingCapability(**envelope),
            env=env,
        )
        self.assertEqual(decrypted, "uview1-test-fvk-secret")

    def test_extract_encryption_descriptor_verifies_payload_hash_binding(self):
        env = self._enclave_env()
        payload = enclave_key_attestation_payload(nonce="client-nonce", env=env)
        payload_hash = enclave_key_attestation_hash(payload)
        response = {
            "attestation_payload": payload,
            "attestation_payload_hash": payload_hash,
            "quote": MockAttestor(ATTESTATION_KEY, measurement="test-measurement").quote(payload_hash).to_dict(),
        }

        descriptor = extract_encryption_key_descriptor(response, expected_nonce="client-nonce")
        self.assertEqual(descriptor["key_id"], "attested-key-1")

        tampered = copy.deepcopy(response)
        tampered["attestation_payload"]["nonce"] = "other"
        with self.assertRaisesRegex(ValueError, "attestation nonce mismatch"):
            extract_encryption_key_descriptor(tampered, expected_nonce="client-nonce")

    def test_build_proof_payload_places_only_ciphertext_in_request(self):
        env = self._enclave_env()
        envelope = encrypt_viewing_capability(
            "secret-fvk",
            enclave_encryption_key_descriptor(env),
            capability_type="fvk",
        )

        payload = build_proof_payload(
            encrypted_viewing_capability=envelope,
            blacklist_manifest=manifest().to_public_dict(),
            network="mainnet",
            pool="orchard",
            block_start=1,
            block_end=2,
            viewing_scope_id="local-scope",
            lightwalletd_endpoint="https://lightwalletd.mainnet.cipherscan.app:443",
        )
        rendered = json.dumps(payload, sort_keys=True)

        self.assertIn("encrypted_viewing_capability", rendered)
        self.assertNotIn("secret-fvk", rendered)
        self.assertNotIn("fvk", payload["request"])

    def test_cli_reads_fvk_from_stdin_and_outputs_decryptable_payload_without_plaintext(self):
        env = self._enclave_env()
        payload = enclave_key_attestation_payload(nonce="cli-nonce", env=env)
        payload_hash = enclave_key_attestation_hash(payload)
        response = {
            "attestation_payload": payload,
            "attestation_payload_hash": payload_hash,
            "quote": MockAttestor(ATTESTATION_KEY, measurement="test-measurement").quote(payload_hash).to_dict(),
        }

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):  # noqa: N802 - stdlib test handler API.
                body = json.dumps(response).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, format, *args):  # noqa: A002 - stdlib signature.
                return

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as tmp:
                manifest_path = Path(tmp) / "blacklist.json"
                manifest_path.write_text(json.dumps(manifest().to_public_dict()), encoding="utf-8")
                proc = subprocess.run(
                    [
                        sys.executable,
                        "scripts/encrypt_viewing_capability.py",
                        "--attestation-url",
                        f"http://127.0.0.1:{server.server_port}/attestation",
                        "--nonce",
                        "cli-nonce",
                        "--viewing-key-stdin",
                        "--capability-type",
                        "fvk",
                        "--blacklist-manifest",
                        str(manifest_path),
                        "--block-start",
                        "1",
                        "--block-end",
                        "1",
                    ],
                    cwd=Path(__file__).resolve().parents[1],
                    input="uview1-cli-secret\n",
                    text=True,
                    capture_output=True,
                    check=False,
                )
        finally:
            server.shutdown()
            server.server_close()

        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertNotIn("uview1-cli-secret", proc.stdout)
        self.assertNotIn("uview1-cli-secret", proc.stderr)
        generated = json.loads(proc.stdout)
        decrypted = decrypt_viewing_capability(
            EncryptedViewingCapability(**generated["request"]["encrypted_viewing_capability"]),
            env=env,
        )
        self.assertEqual(decrypted, "uview1-cli-secret")


class ServiceContractTests(unittest.TestCase):
    def encrypted_payload(self):
        return {
            "request": {
                "network": "regtest",
                "pool": "orchard",
                "block_range": {"start": 100, "end": 110},
                "viewing_scope_id": SCOPE,
                "encrypted_viewing_capability": {
                    "scheme": "x25519-chacha20poly1305-v0",
                    "capability_type": "ivk",
                    "ciphertext": "ciphertext-do-not-log",
                    "key_id": "attested-key-1",
                    "ephemeral_public_key": base64.b64encode(b"e" * 32).decode("ascii"),
                    "nonce": base64.b64encode(b"n" * 12).decode("ascii"),
                },
                "chain_source": {
                    "type": "lightwalletd",
                    "endpoint": "https://lightwalletd.invalid",
                },
            },
            "blacklist_manifest": manifest().to_public_dict(),
        }

    def test_fixture_payload_contract_still_builds_legacy_scanner(self):
        old = os.environ.get("CLEAN_WALLET_ALLOW_FIXTURE_PROOFS")
        os.environ["CLEAN_WALLET_ALLOW_FIXTURE_PROOFS"] = "1"
        payload = {
            "request": {
                "network": "regtest",
                "pool": "orchard",
                "block_range": {"start": 100, "end": 110},
                "viewing_scope_id": SCOPE,
            },
            "fixture": {
                "network": "regtest",
                "pool": "orchard",
                "outputs": [{"height": 100, "viewing_scope_id": SCOPE, "commitment": OWNED_CLEAN}],
            },
            "blacklist_manifest": manifest().to_public_dict(),
        }
        try:
            parsed_request, scanner, parsed_manifest = _parse_proof_payload(payload)
        finally:
            if old is None:
                os.environ.pop("CLEAN_WALLET_ALLOW_FIXTURE_PROOFS", None)
            else:
                os.environ["CLEAN_WALLET_ALLOW_FIXTURE_PROOFS"] = old
        report = create_report(
            request=parsed_request,
            scanner=scanner,
            manifest=parsed_manifest,
            blacklist_signing_key=BLACKLIST_KEY,
            attestor=MockAttestor(ATTESTATION_KEY, measurement="test-measurement"),
            scanner_version="test",
            timestamp_utc="2026-05-27T00:00:00Z",
        )

        self.assertEqual(report["result"], "PASS")

    def test_fixture_payload_is_rejected_for_phala_by_default(self):
        previous_attestor = os.environ.pop("CLEAN_WALLET_ATTESTOR", None)
        previous_allow = os.environ.pop("CLEAN_WALLET_ALLOW_FIXTURE_PROOFS", None)
        try:
            payload = {
                "request": {
                    "network": "regtest",
                    "pool": "orchard",
                    "block_range": {"start": 100, "end": 110},
                    "viewing_scope_id": SCOPE,
                },
                "fixture": {"network": "regtest", "pool": "orchard", "outputs": []},
                "blacklist_manifest": manifest().to_public_dict(),
            }
            with self.assertRaisesRegex(ValueError, "fixture proofs are disabled"):
                _parse_proof_payload(payload)
        finally:
            if previous_attestor is not None:
                os.environ["CLEAN_WALLET_ATTESTOR"] = previous_attestor
            if previous_allow is not None:
                os.environ["CLEAN_WALLET_ALLOW_FIXTURE_PROOFS"] = previous_allow

    def test_encrypted_viewing_capability_contract_returns_error_until_scanner_is_wired(self):
        payload = self.encrypted_payload()
        parsed_request, scanner, parsed_manifest = _parse_proof_payload(payload)
        report = create_report(
            request=parsed_request,
            scanner=scanner,
            manifest=parsed_manifest,
            blacklist_signing_key=BLACKLIST_KEY,
            attestor=MockAttestor(ATTESTATION_KEY, measurement="test-measurement"),
            scanner_version="test",
            timestamp_utc="2026-05-27T00:00:00Z",
        )

        rendered = json.dumps(report, sort_keys=True)
        self.assertEqual(report["result"], "ERROR")
        self.assertIn("no clean-wallet claim", report["claim"])
        self.assertNotIn("ciphertext-do-not-log", rendered)
        self.assertNotIn(SCOPE, rendered)

    def test_parse_accepts_encrypted_uivk_capability_contract(self) -> None:
        payload = self.encrypted_payload()
        payload["request"]["encrypted_viewing_capability"]["capability_type"] = "uivk"

        request, scanner, parsed_manifest = _parse_proof_payload(payload)

        self.assertEqual(request.network, "regtest")
        self.assertEqual(scanner.viewing_capability.capability_type, "uivk")
        self.assertEqual(parsed_manifest.commitments[0], C1)

    def test_plaintext_viewing_capability_fields_are_rejected_without_echoing_secret(self):
        payload = self.encrypted_payload()
        payload["request"]["ivk"] = "plaintext-secret-should-not-appear"

        with self.assertRaisesRegex(ValueError, "plaintext viewing capability field is not accepted: ivk") as raised:
            _parse_proof_payload(payload)

        self.assertNotIn("plaintext-secret-should-not-appear", str(raised.exception))

    def test_encrypted_viewing_capability_requires_attested_key_id(self):
        payload = self.encrypted_payload()
        del payload["request"]["encrypted_viewing_capability"]["key_id"]

        with self.assertRaisesRegex(ValueError, "key_id must be a non-empty string"):
            _parse_proof_payload(payload)

    def test_encrypted_viewing_capability_requires_ephemeral_key_and_nonce(self):
        payload = self.encrypted_payload()
        del payload["request"]["encrypted_viewing_capability"]["ephemeral_public_key"]

        with self.assertRaisesRegex(ValueError, "ephemeral_public_key must be a non-empty string"):
            _parse_proof_payload(payload)

        payload = self.encrypted_payload()
        del payload["request"]["encrypted_viewing_capability"]["nonce"]

        with self.assertRaisesRegex(ValueError, "nonce must be a non-empty string"):
            _parse_proof_payload(payload)

    def test_only_lightwalletd_chain_source_is_accepted_for_real_poc(self):
        payload = self.encrypted_payload()
        payload["request"]["chain_source"] = {"type": "compact_block_bundle", "bundle_manifest_hash": "abc"}

        with self.assertRaisesRegex(ValueError, "chain_source.type must be lightwalletd"):
            _parse_proof_payload(payload)

    def test_json_safe_converts_sdk_style_objects(self):
        class SdkLike:
            def __init__(self):
                self.public_bytes = b"abc"
                self.nested = {"items": ({"value": b"\x00\xff"},)}
                self._private = "hidden"

        safe = service._json_safe({"sdk": SdkLike(), "set": {"x"}})

        self.assertEqual(safe["sdk"]["public_bytes"], "616263")
        self.assertEqual(safe["sdk"]["nested"]["items"][0]["value"], "00ff")
        self.assertEqual(safe["set"], ["x"])
        self.assertNotIn("_private", safe["sdk"])

    def test_enclave_key_descriptor_is_hash_bound_for_attestation(self):
        env = {
            "CLEAN_WALLET_ENCLAVE_KEY_ID": "attested-key-1",
            "CLEAN_WALLET_ENCLAVE_PUBLIC_KEY": "base64-public-key",
        }
        descriptor = enclave_encryption_key_descriptor(env)
        payload = enclave_key_attestation_payload(nonce="client-nonce-1", env=env)
        payload_hash = enclave_key_attestation_hash(payload)
        quote = MockAttestor(ATTESTATION_KEY, measurement="test-measurement").quote(payload_hash).to_dict()

        self.assertEqual(descriptor["status"], "configured")
        self.assertEqual(payload["encryption_key"], descriptor)
        self.assertEqual(len(payload_hash), 64)
        self.assertEqual(quote["report_data"], payload_hash)

    def test_enclave_key_descriptor_is_explicitly_unconfigured_without_public_key(self):
        descriptor = enclave_encryption_key_descriptor({})

        self.assertEqual(descriptor["status"], "unconfigured")
        self.assertNotIn("public_key", descriptor)

    def test_runtime_ephemeral_enclave_key_can_encrypt_without_env_private_key(self):
        env = {
            "CLEAN_WALLET_AUTO_GENERATE_ENCLAVE_KEY": "1",
            "CLEAN_WALLET_ENCLAVE_KEY_ID": "runtime-key",
        }
        descriptor = enclave_encryption_key_descriptor(env)
        envelope = encrypt_viewing_capability(
            "uview1-runtime-ephemeral-secret",
            descriptor,
            capability_type="fvk",
        )

        self.assertEqual(descriptor["status"], "configured")
        self.assertEqual(descriptor["key_origin"], "runtime-ephemeral")
        self.assertNotIn("CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64", json.dumps(descriptor))
        self.assertEqual(
            decrypt_viewing_capability(EncryptedViewingCapability(**envelope), env=env),
            "uview1-runtime-ephemeral-secret",
        )

    def test_service_defaults_to_phala_and_requires_mock_opt_in(self):
        previous = os.environ.pop("CLEAN_WALLET_ATTESTOR", None)
        try:
            self.assertEqual(service._attestor_kind(), "phala")
            os.environ["CLEAN_WALLET_ATTESTOR"] = "mock"
            self.assertEqual(service._runtime_info()["attestor"], "mock")
        finally:
            if previous is None:
                os.environ.pop("CLEAN_WALLET_ATTESTOR", None)
            else:
                os.environ["CLEAN_WALLET_ATTESTOR"] = previous


class RealZcashScannerBoundaryTests(unittest.TestCase):
    def test_lightwalletd_converter_preserves_chain_metadata(self):
        class Metadata:
            saplingCommitmentTreeSize = 123
            orchardCommitmentTreeSize = 456

        class Block:
            protoVersion = 1
            height = 3363067
            hash = b"a" * 32
            prevHash = b"b" * 32
            time = 1
            chainMetadata = Metadata()
            vtx = []

        converted = _compact_block_to_scanner_dict(Block())

        self.assertEqual(
            converted["chainMetadata"],
            {
                "saplingCommitmentTreeSize": 123,
                "orchardCommitmentTreeSize": 456,
            },
        )

    def _encrypted_capability(self, plaintext: str = "uview1realtestkey"):
        enclave_private = x25519.X25519PrivateKey.generate()
        requester_private = x25519.X25519PrivateKey.generate()
        enclave_private_bytes = enclave_private.private_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PrivateFormat.Raw,
            encryption_algorithm=serialization.NoEncryption(),
        )
        requester_public_bytes = requester_private.public_key().public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )
        shared_secret = requester_private.exchange(enclave_private.public_key())
        key = _derive_capability_key(shared_secret, key_id="attested-key-1", capability_type="ufvk")
        nonce = b"1" * 12
        aad = canonical_json(
            {
                "scheme": "x25519-chacha20poly1305-v0",
                "key_id": "attested-key-1",
                "capability_type": "ufvk",
            }
        ).encode("utf-8")
        ciphertext = ChaCha20Poly1305(key).encrypt(nonce, plaintext.encode("utf-8"), aad)
        encrypted = EncryptedViewingCapability(
            scheme="x25519-chacha20poly1305-v0",
            capability_type="ufvk",
            key_id="attested-key-1",
            ciphertext=base64.b64encode(ciphertext).decode("ascii"),
            nonce=base64.b64encode(nonce).decode("ascii"),
            ephemeral_public_key=base64.b64encode(requester_public_bytes).decode("ascii"),
        )
        return encrypted, {
            "CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64": base64.b64encode(enclave_private_bytes).decode("ascii"),
            "CLEAN_WALLET_ENCLAVE_PUBLIC_KEY": public_key_for_private_key_b64(
                base64.b64encode(enclave_private_bytes).decode("ascii")
            ),
        }

    def test_encrypted_viewing_capability_decrypts_only_with_enclave_private_key(self):
        encrypted, env = self._encrypted_capability("uview1secret")

        self.assertEqual(decrypt_viewing_capability(encrypted, env), "uview1secret")
        with self.assertRaisesRegex(ValueError, "private key is not configured"):
            decrypt_viewing_capability(encrypted, {})

    def test_zcash_scanner_fetches_lightwalletd_blocks_and_invokes_scanner_command(self):
        encrypted, env = self._encrypted_capability("uview1secret")

        class FakeLightwalletdClient:
            def __init__(self, endpoint):
                self.endpoint = endpoint

            def fetch_compact_blocks(self, *, start, end):
                self.seen = (start, end)
                return [
                    {
                        "protoVersion": 1,
                        "height": start,
                        "hash": "aa" * 32,
                        "prevHash": "bb" * 32,
                        "time": 1,
                        "vtx": [],
                    }
                ]

        with tempfile.TemporaryDirectory() as tmp:
            script = Path(tmp) / "scanner.py"
            captured = Path(tmp) / "captured.json"
            script.write_text(
                textwrap.dedent(
                    f"""
                    import json, pathlib, sys
                    request = json.loads(sys.stdin.read())
                    pathlib.Path({str(captured)!r}).write_text(json.dumps(request, sort_keys=True))
                    print(json.dumps({{"owned_commitments": ["{C1}"]}}))
                    """
                )
            )
            old_env = os.environ.copy()
            os.environ.update(env)
            try:
                scanner = ZcashViewingKeyScanner(
                    viewing_capability=encrypted,
                    chain_source=ChainSource(source_type="lightwalletd", endpoint="https://lightwalletd.example:9067"),
                    scanner_cmd=f"python3 {script}",
                    lightwalletd_client_factory=FakeLightwalletdClient,
                )
                result = scanner.scan(
                    viewing_scope_id=SCOPE,
                    block_range=BlockRange(100, 100),
                    network="testnet",
                    pool="orchard",
                )
            finally:
                os.environ.clear()
                os.environ.update(old_env)

            self.assertEqual(result.status, "OK")
            self.assertEqual(result.owned_commitments, [C1])
            captured_request = json.loads(captured.read_text())
            self.assertEqual(captured_request["viewing_key"], "uview1secret")
            self.assertEqual(captured_request["compact_blocks"][0]["height"], 100)
            self.assertEqual(captured_request["network"], "testnet")

    def test_zcash_scanner_output_can_generate_attested_fail_report(self):
        encrypted, env = self._encrypted_capability("uview1secret")

        class FakeLightwalletdClient:
            def __init__(self, endpoint):
                self.endpoint = endpoint

            def fetch_compact_blocks(self, *, start, end):
                return [
                    {
                        "protoVersion": 1,
                        "height": start,
                        "hash": "aa" * 32,
                        "prevHash": "bb" * 32,
                        "time": 1,
                        "vtx": [],
                    }
                ]

        with tempfile.TemporaryDirectory() as tmp:
            script = Path(tmp) / "scanner.py"
            script.write_text(
                textwrap.dedent(
                    f"""
                    import json, sys
                    _ = json.loads(sys.stdin.read())
                    print(json.dumps({{"owned_commitments": ["{C1}"]}}))
                    """
                )
            )
            old_env = os.environ.copy()
            os.environ.update(env)
            try:
                scanner = ZcashViewingKeyScanner(
                    viewing_capability=encrypted,
                    chain_source=ChainSource(source_type="lightwalletd", endpoint="https://lightwalletd.example:9067"),
                    scanner_cmd=f"python3 {script}",
                    lightwalletd_client_factory=FakeLightwalletdClient,
                )
                report = create_report(
                    request=request(),
                    scanner=scanner,
                    manifest=manifest(),
                    blacklist_signing_key=BLACKLIST_KEY,
                    attestor=MockAttestor(ATTESTATION_KEY, measurement="test-measurement"),
                    scanner_version="test",
                    timestamp_utc="2026-05-27T00:00:00Z",
                )
            finally:
                os.environ.clear()
                os.environ.update(old_env)

        rendered = json.dumps(report, sort_keys=True)
        self.assertEqual(report["result"], "FAIL")
        self.assertNotIn("uview1secret", rendered)
        self.assertNotIn(SCOPE, rendered)
        verify_report(
            report=report,
            manifest=manifest(),
            blacklist_signing_key=BLACKLIST_KEY,
            attestor=MockAttestor(ATTESTATION_KEY, measurement="test-measurement"),
            allowed_measurements={"test-measurement"},
            now=datetime(2026, 5, 27, tzinfo=timezone.utc),
        )

    def test_zcash_scanner_without_real_backend_returns_error_not_pass(self):
        encrypted, env = self._encrypted_capability("uview1secret")
        old_env = os.environ.copy()
        os.environ.update(env)
        try:
            scanner = ZcashViewingKeyScanner(
                viewing_capability=encrypted,
                chain_source=ChainSource(source_type="lightwalletd", endpoint="https://lightwalletd.example:9067"),
                scanner_cmd="",
                lightwalletd_client_factory=lambda endpoint: None,
            )
            result = scanner.scan(
                viewing_scope_id=SCOPE,
                block_range=BlockRange(100, 100),
                network="testnet",
                pool="orchard",
            )
        finally:
            os.environ.clear()
            os.environ.update(old_env)

        self.assertEqual(result.status, "ERROR")
        self.assertIn("scanner", result.error or "")



class PhalaAdapterTests(unittest.TestCase):
    def test_phala_attestor_binds_report_hash_as_report_data(self):
        from clean_wallet.attestation import PhalaDstackAttestor

        class FakeTcb:
            rtmr3 = "rtmr3-allowlisted"

        class FakeInfo:
            app_id = "app-1"
            instance_id = "instance-1"
            tcb_info = FakeTcb()

        class FakeQuoteResult:
            quote = "abc123"
            event_log = [{"event": "compose-hash", "event_payload": "hash"}]
            vm_config = {"app_compose": "compose-hash"}

        class FakeClient:
            def __init__(self):
                self.report_data = None

            def info(self):
                return FakeInfo()

            def get_quote(self, report_data):
                self.report_data = report_data
                return FakeQuoteResult()

        client = FakeClient()
        attestor = PhalaDstackAttestor(client=client)
        report_hash = "a" * 64
        quote = attestor.quote(report_hash).to_dict()

        self.assertEqual(client.report_data, bytes.fromhex(report_hash))
        self.assertEqual(quote["mode"], "phala-dstack-tdx-v0")
        self.assertEqual(quote["measurement"], "rtmr3-allowlisted")
        self.assertEqual(quote["report_data"], report_hash)
        self.assertEqual(quote["quote"], "abc123")
        self.assertEqual(quote["vm_config"], {"app_compose": "compose-hash"})

    def test_phala_verifier_checks_verified_report_data_prefix(self):
        import clean_wallet.attestation as attestation
        from clean_wallet.attestation import PhalaDstackVerifier

        old_verify = attestation._verify_phala_quote
        try:
            attestation._verify_phala_quote = lambda verify_url, hardware_quote: {
                "quote": {"verified": True, "body": {"report_data": "0x" + "b" * 64 + "0" * 64}}
            }
            verifier = PhalaDstackVerifier(measurement="unused")
            verifier.verify_quote(
                {
                    "mode": "phala-dstack-tdx-v0",
                    "measurement": "rtmr3-allowlisted",
                    "quote": "abc123",
                },
                expected_report_hash="b" * 64,
                allowed_measurements={"rtmr3-allowlisted"},
            )
        finally:
            attestation._verify_phala_quote = old_verify


if __name__ == "__main__":
    unittest.main()
