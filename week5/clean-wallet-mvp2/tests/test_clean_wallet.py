from __future__ import annotations

import copy
import json
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path

from clean_wallet.attestation import MockAttestor
from clean_wallet.blacklist import build_manifest, verify_manifest
from clean_wallet.crypto import normalize_commitment
from clean_wallet.proof import ProofRequest, create_report, verify_report
from clean_wallet.scanner import BlockRange, FixtureScanner

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


if __name__ == "__main__":
    unittest.main()
