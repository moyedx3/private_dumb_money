"""Minimal HTTP boundary for running Clean Wallet inside a TEE CVM.

This server is intentionally small. It exposes the existing proof engine over
HTTP and defaults to Phala/dstack attestation so production CVM deployments
fail closed if `/var/run/dstack.sock` or `dstack-sdk` is unavailable. Local mock
attestation remains available with `CLEAN_WALLET_ATTESTOR=mock`.

Production proof mode rejects prover-submitted fixture commitments. `/proof`
accepts an encrypted viewing capability envelope plus a lightwalletd chain source;
the enclave process decrypts the viewing key, fetches compact blocks itself, and
invokes `CLEAN_WALLET_ZCASH_SCANNER_CMD` to perform real Zcash trial-decryption.
If that scanner backend is absent or fails, the result is ERROR, never PASS.
"""

from __future__ import annotations

import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import parse_qs, urlparse

from . import __version__
from .attestation import build_attestor, package_measurement
from .blacklist import BlacklistManifest, load_manifest
from .cli import DEFAULT_ATTESTATION_KEY, DEFAULT_BLACKLIST_KEY
from .enclave_key import (
    enclave_encryption_key_descriptor,
    enclave_key_attestation_hash,
    enclave_key_attestation_payload,
)
from .proof import ProofRequest, create_report
from .scanner import (
    BlockRange,
    ChainSource,
    EncryptedViewingCapability,
    FixtureScanner,
    Scanner,
    ZcashViewingKeyScanner,
)

PLAINTEXT_VIEWING_CAPABILITY_FIELDS = {
    "viewing_key",
    "viewingkey",
    "incoming_viewing_key",
    "outgoing_viewing_key",
    "ivk",
    "fvk",
    "ufvk",
    "uivk",
    "secret_key",
    "spending_key",
    "seed_phrase",
    "mnemonic",
}
VIEWING_CAPABILITY_TYPES = {"ivk", "uivk", "fvk", "ufvk"}
CHAIN_SOURCE_TYPES = {"lightwalletd"}


def _fixture_proofs_allowed() -> bool:
    configured = os.environ.get("CLEAN_WALLET_ALLOW_FIXTURE_PROOFS", "").strip().lower()
    if configured in {"1", "true", "yes"}:
        return True
    if configured in {"0", "false", "no"}:
        return False
    return _attestor_kind() == "mock"


def _json_safe(value: Any) -> Any:
    """Convert dstack SDK response objects into JSON-safe public values."""

    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, bytes):
        return value.hex()
    if isinstance(value, dict):
        return {str(key): _json_safe(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_json_safe(item) for item in value]
    if hasattr(value, "to_dict"):
        return _json_safe(value.to_dict())
    if hasattr(value, "__dict__"):
        return {str(key): _json_safe(item) for key, item in vars(value).items() if not str(key).startswith("_")}
    return str(value)


def _json_response(handler: BaseHTTPRequestHandler, status: int, payload: dict[str, Any]) -> None:
    body = json.dumps(_json_safe(payload), indent=2, sort_keys=True).encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json")
    handler.send_header("Content-Length", str(len(body)))
    handler.end_headers()
    handler.wfile.write(body)


def _attestor_kind() -> str:
    return os.environ.get("CLEAN_WALLET_ATTESTOR", "phala")


def _attestor():
    return build_attestor(
        _attestor_kind(),
        attestation_key=os.environ.get("CLEAN_WALLET_ATTESTATION_KEY", DEFAULT_ATTESTATION_KEY),
    )


def _public_dstack_info(info: Any) -> Any:
    return _json_safe(info)


def _runtime_info() -> dict[str, Any]:
    kind = _attestor_kind()
    if kind == "mock":
        return {
            "attestor": kind,
            "measurement": package_measurement(),
            "dstack": None,
            "encryption_key": enclave_encryption_key_descriptor(),
        }
    attestor = _attestor()
    return {
        "attestor": kind,
        "measurement": attestor.measurement,
        "dstack": _public_dstack_info(getattr(attestor, "info", None)),
        "app_id": getattr(attestor, "app_id", None),
        "instance_id": getattr(attestor, "instance_id", None),
        "encryption_key": enclave_encryption_key_descriptor(),
    }


def _require_mapping(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{field} must be an object")
    return value


def _require_string(mapping: dict[str, Any], field: str) -> str:
    key = field.rsplit(".", 1)[-1]
    value = mapping.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{field} must be a non-empty string")
    return value


def _reject_plaintext_viewing_capability(value: Any) -> None:
    """Reject raw viewing-key material by key name without echoing values."""

    if isinstance(value, dict):
        for key, child in value.items():
            if str(key).lower() in PLAINTEXT_VIEWING_CAPABILITY_FIELDS:
                raise ValueError(f"plaintext viewing capability field is not accepted: {key}")
            _reject_plaintext_viewing_capability(child)
    elif isinstance(value, list):
        for child in value:
            _reject_plaintext_viewing_capability(child)


def _parse_block_range(request_payload: dict[str, Any]) -> BlockRange:
    block_range = _require_mapping(request_payload.get("block_range"), "request.block_range")
    start = int(block_range["start"])
    end = int(block_range["end"])
    if start < 0 or end < start:
        raise ValueError("request.block_range must satisfy 0 <= start <= end")
    return BlockRange(start, end)


def _parse_common_request(request_payload: dict[str, Any]) -> ProofRequest:
    pool = request_payload.get("pool", "orchard")
    if not isinstance(pool, str) or not pool.strip():
        raise ValueError("request.pool must be a non-empty string")
    return ProofRequest(
        network=_require_string(request_payload, "request.network"),
        pool=pool,
        block_range=_parse_block_range(request_payload),
        viewing_scope_id=_require_string(request_payload, "request.viewing_scope_id"),
    )


def _parse_encrypted_viewing_capability(request_payload: dict[str, Any]) -> EncryptedViewingCapability:
    encrypted = _require_mapping(
        request_payload.get("encrypted_viewing_capability"),
        "request.encrypted_viewing_capability",
    )
    capability_type = str(encrypted.get("capability_type", request_payload.get("viewing_capability_type", ""))).lower()
    if capability_type not in VIEWING_CAPABILITY_TYPES:
        raise ValueError("request.encrypted_viewing_capability.capability_type must be one of ivk, uivk, fvk, ufvk")
    key_id = _require_string(encrypted, "request.encrypted_viewing_capability.key_id")
    ephemeral_public_key = encrypted.get("ephemeral_public_key") or encrypted.get("sender_public_key")
    nonce = encrypted.get("nonce")
    if not isinstance(ephemeral_public_key, str) or not ephemeral_public_key.strip():
        raise ValueError("request.encrypted_viewing_capability.ephemeral_public_key must be a non-empty string")
    if not isinstance(nonce, str) or not nonce.strip():
        raise ValueError("request.encrypted_viewing_capability.nonce must be a non-empty string")
    return EncryptedViewingCapability(
        scheme=_require_string(encrypted, "request.encrypted_viewing_capability.scheme"),
        ciphertext=_require_string(encrypted, "request.encrypted_viewing_capability.ciphertext"),
        capability_type=capability_type,
        key_id=key_id,
        ephemeral_public_key=ephemeral_public_key,
        nonce=nonce,
    )


def _parse_chain_source(request_payload: dict[str, Any]) -> ChainSource:
    chain_source = _require_mapping(request_payload.get("chain_source"), "request.chain_source")
    source_type = _require_string(chain_source, "request.chain_source.type")
    if source_type not in CHAIN_SOURCE_TYPES:
        raise ValueError("request.chain_source.type must be lightwalletd for the real Zcash PoC")
    endpoint = None
    bundle_manifest_hash = None
    endpoint = _require_string(chain_source, "request.chain_source.endpoint")
    return ChainSource(
        source_type=source_type,
        endpoint=endpoint,
        bundle_manifest_hash=bundle_manifest_hash,
    )


def _parse_proof_payload(payload: dict[str, Any]) -> tuple[ProofRequest, Scanner, BlacklistManifest]:
    """Build proof inputs from either fixture MVP or encrypted production contract."""

    payload = _require_mapping(payload, "payload")
    if "fixture" not in payload:
        _reject_plaintext_viewing_capability(payload)
    request_payload = _require_mapping(payload.get("request"), "request")
    request = _parse_common_request(request_payload)
    manifest = load_manifest(_require_mapping(payload.get("blacklist_manifest"), "blacklist_manifest"))

    if "fixture" in payload:
        if not _fixture_proofs_allowed():
            raise ValueError(
                "fixture proofs are disabled for this attestor; use encrypted_viewing_capability + lightwalletd"
            )
        return request, FixtureScanner(_require_mapping(payload["fixture"], "fixture")), manifest

    scanner = ZcashViewingKeyScanner(
        viewing_capability=_parse_encrypted_viewing_capability(request_payload),
        chain_source=_parse_chain_source(request_payload),
    )
    return request, scanner, manifest


class CleanWalletHandler(BaseHTTPRequestHandler):
    server_version = "clean-wallet-tee/0.1"

    def do_GET(self) -> None:  # noqa: N802 - stdlib HTTP handler API.
        parsed = urlparse(self.path)
        if parsed.path == "/health":
            try:
                runtime = _runtime_info()
            except Exception as exc:  # noqa: BLE001 - fail closed when Phala runtime is unavailable.
                _json_response(
                    self,
                    503,
                    {
                        "ok": False,
                        "version": __version__,
                        "attestor": _attestor_kind(),
                        "error": str(exc),
                    },
                )
                return
            _json_response(
                self,
                200,
                {
                    "ok": True,
                    "version": __version__,
                    "attestor": runtime["attestor"],
                    "measurement": runtime["measurement"],
                },
            )
            return
        if parsed.path == "/info":
            try:
                runtime = _runtime_info()
            except Exception as exc:  # noqa: BLE001 - HTTP boundary returns sanitized error.
                _json_response(self, 503, {"error": str(exc), "attestor": _attestor_kind()})
                return
            _json_response(self, 200, runtime)
            return
        if parsed.path == "/measurement":
            try:
                runtime = _runtime_info()
            except Exception as exc:  # noqa: BLE001 - HTTP boundary returns sanitized error.
                _json_response(self, 503, {"error": str(exc), "attestor": _attestor_kind()})
                return
            _json_response(self, 200, runtime)
            return
        if parsed.path == "/attestation":
            try:
                query = parse_qs(parsed.query)
                if query.get("purpose", [""])[0] == "enclave-key":
                    payload = enclave_key_attestation_payload(nonce=query.get("nonce", [""])[0])
                    payload_hash = enclave_key_attestation_hash(payload)
                    quote = _attestor().quote(payload_hash).to_dict()
                    _json_response(
                        self,
                        200,
                        {
                            "attestation_payload": payload,
                            "attestation_payload_hash": payload_hash,
                            "quote": quote,
                        },
                    )
                    return
                report_hash = query.get("report_hash", [""])[0]
                quote = _attestor().quote(report_hash).to_dict()
            except Exception as exc:  # noqa: BLE001 - HTTP boundary returns sanitized error.
                _json_response(self, 400, {"error": str(exc)})
                return
            _json_response(self, 200, {"quote": quote})
            return
        _json_response(self, 404, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802 - stdlib HTTP handler API.
        if urlparse(self.path).path != "/proof":
            _json_response(self, 404, {"error": "not found"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            payload = json.loads(self.rfile.read(length).decode("utf-8"))
            request, scanner, manifest = _parse_proof_payload(payload)
            report = create_report(
                request=request,
                scanner=scanner,
                manifest=manifest,
                blacklist_signing_key=os.environ.get("CLEAN_WALLET_BLACKLIST_KEY", DEFAULT_BLACKLIST_KEY),
                attestor=_attestor(),
                scanner_version=__version__,
            )
        except Exception as exc:  # noqa: BLE001 - HTTP boundary returns sanitized error.
            _json_response(self, 400, {"error": str(exc)})
            return
        _json_response(self, 200, report)

    def log_message(self, format: str, *args: object) -> None:
        # Avoid accidentally logging request bodies or viewing-key material in
        # future production scanner paths. Keep access logs off by default.
        if os.environ.get("CLEAN_WALLET_ACCESS_LOGS") == "1":
            super().log_message(format, *args)


def main() -> int:
    host = os.environ.get("CLEAN_WALLET_HOST", "0.0.0.0")
    port = int(os.environ.get("CLEAN_WALLET_PORT", "8080"))
    server = ThreadingHTTPServer((host, port), CleanWalletHandler)
    print(f"clean-wallet TEE service listening on {host}:{port} attestor={_attestor_kind()}")
    server.serve_forever()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
