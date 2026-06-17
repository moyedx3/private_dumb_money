"""Proof request, report generation, and verification."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any

from .attestation import Attestor
from .blacklist import BlacklistManifest, verify_manifest
from .crypto import canonical_json, constant_time_equal, sha256_hex
from .scanner import BlockRange, Scanner, viewing_scope_commitment

RESULT_PASS = "PASS"
RESULT_FAIL = "FAIL"
RESULT_ERROR = "ERROR"


@dataclass(frozen=True)
class ProofRequest:
    network: str
    pool: str
    block_range: BlockRange
    viewing_scope_id: str


def _report_body_without_hash_or_quote(report: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in report.items() if key not in {"report_hash", "signature_or_quote"}}


def compute_report_hash(report_without_hash_or_quote: dict[str, Any]) -> str:
    return sha256_hex(canonical_json(report_without_hash_or_quote))


def create_report(
    *,
    request: ProofRequest,
    scanner: Scanner,
    manifest: BlacklistManifest,
    blacklist_signing_key: str,
    attestor: Attestor,
    scanner_version: str,
    timestamp_utc: str | None = None,
) -> dict[str, Any]:
    verify_manifest(manifest, signing_key=blacklist_signing_key)
    timestamp = timestamp_utc or datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    scan = scanner.scan(
        viewing_scope_id=request.viewing_scope_id,
        block_range=request.block_range,
        network=request.network,
        pool=request.pool,
    )

    if scan.status != "OK":
        result = RESULT_ERROR
        error = scan.error or "unknown scanner error"
    else:
        overlap = set(scan.owned_commitments).intersection(manifest.commitments)
        result = RESULT_FAIL if overlap else RESULT_PASS
        error = None

    body: dict[str, Any] = {
        "schema_version": "clean-wallet-report-v0",
        "result": result,
        "claim": (
            "No exact commitment overlap within declared scope"
            if result == RESULT_PASS
            else "Bounded exact-overlap check completed"
        ),
        "network": request.network,
        "pool": request.pool,
        "block_range": request.block_range.to_dict(),
        "viewing_scope_commitment": viewing_scope_commitment(request.viewing_scope_id),
        "blacklist_root": manifest.root,
        "blacklist_manifest_hash": manifest.manifest_hash,
        "measurement": attestor.measurement,
        "timestamp_utc": timestamp,
        "scanner_version": scanner_version,
        "disclaimer": (
            "PASS is bounded to submitted scope/range/list and is not a global innocence proof; "
            "the submitted viewing scope is not proof of wallet completeness or identity ownership."
        ),
    }
    if error:
        body["error"] = error
        body["claim"] = "Scanner or attestation boundary returned an error; no clean-wallet claim is made"

    report_hash = compute_report_hash(body)
    quote = attestor.quote(report_hash).to_dict()
    body["report_hash"] = report_hash
    body["signature_or_quote"] = quote
    return body


def verify_report(
    *,
    report: dict[str, Any],
    manifest: BlacklistManifest,
    blacklist_signing_key: str,
    attestor: Attestor,
    allowed_measurements: set[str],
    max_age_seconds: int | None = None,
    now: datetime | None = None,
) -> None:
    verify_manifest(manifest, signing_key=blacklist_signing_key)
    if report.get("schema_version") != "clean-wallet-report-v0":
        raise ValueError("unsupported report schema")
    if report.get("blacklist_root") != manifest.root:
        raise ValueError("report blacklist_root does not match manifest")
    if report.get("blacklist_manifest_hash") != manifest.manifest_hash:
        raise ValueError("report blacklist manifest hash mismatch")

    body = _report_body_without_hash_or_quote(report)
    expected_hash = compute_report_hash(body)
    if not constant_time_equal(expected_hash, str(report.get("report_hash", ""))):
        raise ValueError("report_hash mismatch")
    attestor.verify_quote(
        dict(report.get("signature_or_quote", {})),
        expected_report_hash=expected_hash,
        allowed_measurements=allowed_measurements,
    )
    if report.get("measurement") not in allowed_measurements:
        raise ValueError("report measurement is not allowlisted")
    if report.get("measurement") != report.get("signature_or_quote", {}).get("measurement"):
        raise ValueError("report measurement does not match quote measurement")
    if report.get("result") not in {RESULT_PASS, RESULT_FAIL, RESULT_ERROR}:
        raise ValueError("unsupported result")
    if max_age_seconds is not None:
        ts = datetime.fromisoformat(str(report["timestamp_utc"]).replace("Z", "+00:00"))
        current = now or datetime.now(timezone.utc)
        age = abs((current - ts).total_seconds())
        if age > max_age_seconds:
            raise ValueError("report timestamp is outside freshness policy")
