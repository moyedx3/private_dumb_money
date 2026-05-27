"""Mock attestation adapter and production TEE seam.

TEE plan:
- v0 uses MockAttestor so the PoC is runnable on any laptop.
- The report body is hashed and placed into `report_data`, mirroring the real
  SGX/TDX pattern where attestation binds arbitrary report data to an enclave
  measurement.
- v1 replaces `MockAttestor.quote/verify_quote` with DCAP/TDX/Nitro quote
  generation and verification. Scanner/proof/report code should not change.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .crypto import canonical_json, constant_time_equal, hmac_sha256_hex, sha256_hex


@dataclass(frozen=True)
class MockQuote:
    mode: str
    measurement: str
    report_data: str
    signature: str

    def to_dict(self) -> dict[str, str]:
        return {
            "mode": self.mode,
            "measurement": self.measurement,
            "report_data": self.report_data,
            "signature": self.signature,
        }


class MockAttestor:
    mode = "mock-tee-v0"

    def __init__(self, attestation_key: str, measurement: str | None = None):
        self.attestation_key = attestation_key
        self.measurement = measurement or package_measurement()

    def quote(self, report_hash: str) -> MockQuote:
        payload = {
            "mode": self.mode,
            "measurement": self.measurement,
            "report_data": report_hash,
        }
        signature = hmac_sha256_hex(self.attestation_key, canonical_json(payload))
        return MockQuote(signature=signature, **payload)

    def verify_quote(self, quote: dict[str, Any], *, expected_report_hash: str, allowed_measurements: set[str]) -> None:
        if quote.get("mode") != self.mode:
            raise ValueError("unsupported attestation mode")
        measurement = str(quote.get("measurement", ""))
        if measurement not in allowed_measurements:
            raise ValueError("measurement is not allowlisted")
        if quote.get("report_data") != expected_report_hash:
            raise ValueError("quote report_data does not bind report hash")
        payload = {
            "mode": self.mode,
            "measurement": measurement,
            "report_data": expected_report_hash,
        }
        expected_signature = hmac_sha256_hex(self.attestation_key, canonical_json(payload))
        if not constant_time_equal(expected_signature, str(quote.get("signature", ""))):
            raise ValueError("attestation signature mismatch")


def package_measurement() -> str:
    """Deterministic mock measurement over the PoC package source files."""

    root = Path(__file__).resolve().parent
    chunks: list[str] = []
    for path in sorted(root.glob("*.py")):
        if path.name == "__pycache__":
            continue
        chunks.append(path.name)
        chunks.append(path.read_text(encoding="utf-8"))
    return sha256_hex("\n".join(chunks))
