"""Blacklist manifest construction and verification."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any

from .crypto import (
    canonical_json,
    constant_time_equal,
    hmac_sha256_hex,
    normalize_commitment,
    root_for_commitments,
    sha256_hex,
)


@dataclass(frozen=True)
class BlacklistManifest:
    schema_version: str
    network: str
    pool: str
    issuer: str
    version: str
    created_at: str
    commitments: list[str]
    commitment_count: int
    root: str
    manifest_hash: str
    signature: str

    def to_public_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "network": self.network,
            "pool": self.pool,
            "issuer": self.issuer,
            "version": self.version,
            "created_at": self.created_at,
            "commitments": self.commitments,
            "commitment_count": self.commitment_count,
            "root": self.root,
            "manifest_hash": self.manifest_hash,
            "signature": self.signature,
        }


def _payload_without_signature(payload: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in payload.items() if key not in {"signature", "manifest_hash"}}


def build_manifest(
    commitments: list[str],
    *,
    network: str,
    pool: str = "orchard",
    issuer: str,
    version: str,
    signing_key: str,
    created_at: str | None = None,
) -> BlacklistManifest:
    normalized = sorted({normalize_commitment(item) for item in commitments})
    created = created_at or datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    root = root_for_commitments(normalized)
    payload = {
        "schema_version": "clean-wallet-blacklist-v0",
        "network": network,
        "pool": pool,
        "issuer": issuer,
        "version": version,
        "created_at": created,
        "commitments": normalized,
        "commitment_count": len(normalized),
        "root": root,
    }
    manifest_hash = sha256_hex(canonical_json(payload))
    signature = hmac_sha256_hex(signing_key, manifest_hash)
    return BlacklistManifest(manifest_hash=manifest_hash, signature=signature, **payload)


def load_manifest(data: dict[str, Any]) -> BlacklistManifest:
    required = {
        "schema_version",
        "network",
        "pool",
        "issuer",
        "version",
        "created_at",
        "commitments",
        "commitment_count",
        "root",
        "manifest_hash",
        "signature",
    }
    missing = sorted(required - data.keys())
    if missing:
        raise ValueError(f"blacklist manifest missing fields: {', '.join(missing)}")
    commitments = sorted({normalize_commitment(item) for item in data["commitments"]})
    return BlacklistManifest(
        schema_version=str(data["schema_version"]),
        network=str(data["network"]),
        pool=str(data["pool"]),
        issuer=str(data["issuer"]),
        version=str(data["version"]),
        created_at=str(data["created_at"]),
        commitments=commitments,
        commitment_count=int(data["commitment_count"]),
        root=str(data["root"]),
        manifest_hash=str(data["manifest_hash"]),
        signature=str(data["signature"]),
    )


def verify_manifest(manifest: BlacklistManifest, *, signing_key: str) -> None:
    if manifest.schema_version != "clean-wallet-blacklist-v0":
        raise ValueError("unsupported blacklist schema")
    expected_root = root_for_commitments(manifest.commitments)
    if not constant_time_equal(expected_root, manifest.root):
        raise ValueError("blacklist root does not match commitments")
    if manifest.commitment_count != len(manifest.commitments):
        raise ValueError("blacklist commitment_count mismatch")
    payload = _payload_without_signature(manifest.to_public_dict())
    expected_hash = sha256_hex(canonical_json(payload))
    if not constant_time_equal(expected_hash, manifest.manifest_hash):
        raise ValueError("blacklist manifest_hash mismatch")
    expected_sig = hmac_sha256_hex(signing_key, manifest.manifest_hash)
    if not constant_time_equal(expected_sig, manifest.signature):
        raise ValueError("blacklist signature mismatch")
