"""Attestation adapters and production TEE seam.

TEE plan:
- v0 uses MockAttestor so the PoC is runnable on any laptop.
- The report body is hashed and placed into `report_data`, mirroring the real
  SGX/TDX pattern where attestation binds arbitrary report data to an enclave
  measurement.
- v1 uses Phala Cloud / dstack in an Intel TDX CVM. The application mounts
  `/var/run/dstack.sock`, calls dstack SDK `get_quote(report_data)`, and binds
  the Clean Wallet `report_hash` into the TDX quote's reportData field.

Scanner/proof/report code should depend only on the small attestor interface:
`quote(report_hash) -> object with to_dict()` and
`verify_quote(quote, expected_report_hash, allowed_measurements) -> None`.
"""

from __future__ import annotations

import json
import os
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

from .crypto import canonical_json, constant_time_equal, hmac_sha256_hex, sha256_hex

PHALA_VERIFY_URL = "https://cloud-api.phala.com/api/v1/attestations/verify"


class QuoteLike(Protocol):
    def to_dict(self) -> dict[str, Any]: ...


class Attestor(Protocol):
    measurement: str

    def quote(self, report_hash: str) -> QuoteLike: ...

    def verify_quote(
        self,
        quote: dict[str, Any],
        *,
        expected_report_hash: str,
        allowed_measurements: set[str],
    ) -> None: ...


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


@dataclass(frozen=True)
class PhalaDstackQuote:
    """Serializable subset of a Phala/dstack TDX quote response.

    `report_data` records the expected Clean Wallet report hash for local report
    readability. Verifiers must still check that the hardware quote itself
    contains this value via Phala's verification API or a local DCAP verifier.
    """

    mode: str
    measurement: str
    report_data: str
    quote: str
    event_log: Any
    app_id: str | None = None
    instance_id: str | None = None
    vm_config: Any = None

    def to_dict(self) -> dict[str, Any]:
        data: dict[str, Any] = {
            "mode": self.mode,
            "measurement": self.measurement,
            "report_data": self.report_data,
            "quote": self.quote,
            "event_log": self.event_log,
        }
        if self.app_id:
            data["app_id"] = self.app_id
        if self.instance_id:
            data["instance_id"] = self.instance_id
        if self.vm_config is not None:
            data["vm_config"] = self.vm_config
        return data


class PhalaDstackAttestor:
    """Attestor backed by Phala Cloud/dstack inside a TDX CVM.

    Requirements inside the Phala CVM container:
    - `dstack-sdk` is installed.
    - `/var/run/dstack.sock:/var/run/dstack.sock` is mounted in Docker Compose.

    Verification defaults to Phala Cloud's quote verification endpoint. High
    assurance deployments can replace this boundary with local DCAP/QVL
    verification while preserving the same `verify_quote` contract.
    """

    mode = "phala-dstack-tdx-v0"

    def __init__(self, client: Any | None = None, verify_url: str | None = None, measurement: str | None = None):
        self.client = client or self._load_default_client()
        self.verify_url = verify_url or os.environ.get("PHALA_ATTESTATION_VERIFY_URL", PHALA_VERIFY_URL)
        self.info = self.client.info()
        self.measurement = measurement or _extract_measurement(self.info)
        self.app_id = _read_attr(self.info, "app_id")
        self.instance_id = _read_attr(self.info, "instance_id")

    @staticmethod
    def _load_default_client() -> Any:
        try:
            from dstack_sdk import DstackClient  # type: ignore
        except ImportError as exc:  # pragma: no cover - exercised only in Phala image/runtime.
            raise RuntimeError(
                "dstack-sdk is required for PhalaDstackAttestor; install dstack-sdk in the CVM image"
            ) from exc
        return DstackClient()

    def quote(self, report_hash: str) -> PhalaDstackQuote:
        report_data = _report_hash_to_report_data(report_hash)
        quote_result = self.client.get_quote(report_data)
        return PhalaDstackQuote(
            mode=self.mode,
            measurement=self.measurement,
            report_data=report_hash,
            quote=str(_read_attr(quote_result, "quote")),
            event_log=_read_attr(quote_result, "event_log"),
            app_id=self.app_id,
            instance_id=self.instance_id,
            vm_config=_read_attr(quote_result, "vm_config"),
        )

    def verify_quote(self, quote: dict[str, Any], *, expected_report_hash: str, allowed_measurements: set[str]) -> None:
        _verify_phala_dstack_quote(
            quote,
            expected_report_hash=expected_report_hash,
            allowed_measurements=allowed_measurements,
            verify_url=self.verify_url,
        )


class PhalaDstackVerifier:
    """Verifier-side Phala quote checker that does not require dstack.sock.

    Use this outside the CVM when validating a report emitted by
    `PhalaDstackAttestor`. It verifies the hardware quote through Phala Cloud's
    attestation API and checks that quote reportData binds the Clean Wallet
    report hash.
    """

    mode = PhalaDstackAttestor.mode

    def __init__(self, verify_url: str | None = None, measurement: str = "verifier-only"):
        self.verify_url = verify_url or os.environ.get("PHALA_ATTESTATION_VERIFY_URL", PHALA_VERIFY_URL)
        self.measurement = measurement

    def quote(self, report_hash: str) -> QuoteLike:
        _ = report_hash
        raise RuntimeError("PhalaDstackVerifier cannot generate quotes; use PhalaDstackAttestor inside the CVM")

    def verify_quote(self, quote: dict[str, Any], *, expected_report_hash: str, allowed_measurements: set[str]) -> None:
        _verify_phala_dstack_quote(
            quote,
            expected_report_hash=expected_report_hash,
            allowed_measurements=allowed_measurements,
            verify_url=self.verify_url,
        )


def build_attestor(kind: str, *, attestation_key: str, measurement: str | None = None) -> Attestor:
    """Factory used by proof-generation boundaries to select mock or Phala TEE mode."""

    normalized = kind.strip().lower()
    if normalized in {"mock", "mock-tee", "mock-tee-v0"}:
        return MockAttestor(attestation_key, measurement=measurement)
    if normalized in {"phala", "phala-dstack", "phala-dstack-tdx-v0"}:
        return PhalaDstackAttestor(measurement=measurement)
    raise ValueError(f"unsupported attestor: {kind}")


def build_verifier(kind: str, *, attestation_key: str, measurement: str | None = None) -> Attestor:
    """Factory used by verifier boundaries. Phala verification works off-CVM."""

    normalized = kind.strip().lower()
    if normalized in {"mock", "mock-tee", "mock-tee-v0"}:
        return MockAttestor(attestation_key, measurement=measurement)
    if normalized in {"phala", "phala-dstack", "phala-dstack-tdx-v0"}:
        return PhalaDstackVerifier(measurement=measurement or "verifier-only")
    raise ValueError(f"unsupported attestor: {kind}")


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


def _read_attr(value: Any, name: str, default: Any = None) -> Any:
    if isinstance(value, dict):
        return value.get(name, default)
    return getattr(value, name, default)


def _extract_measurement(info: Any) -> str:
    tcb_info = _read_attr(info, "tcb_info", {})
    if isinstance(tcb_info, str):
        try:
            tcb_info = json.loads(tcb_info)
        except json.JSONDecodeError:
            pass
    for key in ("rtmr3", "rt_mr3", "mr_config", "mrconfig"):
        value = _read_attr(tcb_info, key)
        if value:
            return str(value)
    app_id = _read_attr(info, "app_id")
    if app_id:
        return f"app_id:{app_id}"
    raise ValueError("could not determine Phala/dstack measurement from TCB info")


def _report_hash_to_report_data(report_hash: str) -> bytes:
    if len(report_hash) != 64:
        raise ValueError("report_hash must be a SHA-256 hex string")
    try:
        return bytes.fromhex(report_hash)
    except ValueError as exc:
        raise ValueError("report_hash must be valid hex") from exc


def _verify_phala_dstack_quote(
    quote: dict[str, Any], *, expected_report_hash: str, allowed_measurements: set[str], verify_url: str
) -> None:
    if quote.get("mode") != PhalaDstackAttestor.mode:
        raise ValueError("unsupported attestation mode")
    measurement = str(quote.get("measurement", ""))
    if measurement not in allowed_measurements:
        raise ValueError("measurement is not allowlisted")
    hardware_quote = str(quote.get("quote", ""))
    if not hardware_quote:
        raise ValueError("missing TDX quote")

    verified_quote = _verify_phala_quote(verify_url, hardware_quote)
    verified = _find_first(verified_quote, {"verified"})
    if verified is not True and verified != 1:
        raise ValueError("Phala quote verification failed")

    attested_report_data = _find_first(verified_quote, {"report_data", "reportdata"})
    if attested_report_data is None:
        raise ValueError("verified quote response did not include reportData")
    if not _report_data_matches_hash(str(attested_report_data), expected_report_hash):
        raise ValueError("quote reportData does not bind report hash")


def _verify_phala_quote(verify_url: str, hardware_quote: str) -> dict[str, Any]:
    body = json.dumps({"hex": hardware_quote}).encode("utf-8")
    request = urllib.request.Request(
        verify_url,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=20) as response:  # noqa: S310 - verifier URL is explicit config.
        return json.loads(response.read().decode("utf-8"))


def _find_first(value: Any, names: set[str]) -> Any:
    if isinstance(value, dict):
        for key, item in value.items():
            if key.lower() in names:
                return item
        for item in value.values():
            found = _find_first(item, names)
            if found is not None:
                return found
    elif isinstance(value, list):
        for item in value:
            found = _find_first(item, names)
            if found is not None:
                return found
    return None


def _report_data_matches_hash(report_data: str, expected_report_hash: str) -> bool:
    normalized = report_data.lower().strip()
    if normalized.startswith("0x"):
        normalized = normalized[2:]
    expected = expected_report_hash.lower()
    # TDX reportData is 64 bytes. dstack callers often pass 32-byte hashes, so
    # verifier responses may expose `<hash><zero padding>` rather than exact
    # equality. Prefix match preserves the binding while accepting that padding.
    return normalized == expected or normalized.startswith(expected)
