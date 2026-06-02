"""Scanner boundaries for fixture and production Zcash scanning.

The production scanner should trial-decrypt compact blocks with a UFVK/FVK/IVK
inside the attested TEE and bind owned outputs to their on-chain Orchard note
commitments. The fixture scanner keeps the same output contract while avoiding
live chain dependencies.
"""

from __future__ import annotations

import json
import os
import shlex
import subprocess
from dataclasses import dataclass
from typing import Any, Protocol

from .crypto import normalize_commitment, sha256_hex
from .enclave_key import decrypt_viewing_capability
from .lightwalletd import LightwalletdClient


@dataclass(frozen=True)
class BlockRange:
    start: int
    end: int

    def contains(self, height: int) -> bool:
        return self.start <= height <= self.end

    def to_dict(self) -> dict[str, int]:
        return {"start": self.start, "end": self.end}


@dataclass(frozen=True)
class ScanResult:
    status: str
    owned_commitments: list[str]
    error: str | None = None


@dataclass(frozen=True)
class EncryptedViewingCapability:
    """Opaque requester-provided viewing capability ciphertext.

    The service validates only the non-secret envelope here. Decryption and
    key parsing must happen inside the TEE-local scanner implementation.
    """

    scheme: str
    ciphertext: str
    capability_type: str
    key_id: str | None = None
    ephemeral_public_key: str | None = None
    nonce: str | None = None


@dataclass(frozen=True)
class ChainSource:
    """Declared source of compact block data for production scanning."""

    source_type: str
    endpoint: str | None = None
    bundle_manifest_hash: str | None = None


class Scanner(Protocol):
    def scan(self, *, viewing_scope_id: str, block_range: BlockRange, network: str, pool: str) -> ScanResult: ...


def viewing_scope_commitment(viewing_scope_id: str) -> str:
    return sha256_hex(f"clean-wallet-scope-v0:{viewing_scope_id}")


class FixtureScanner:
    def __init__(self, fixture: dict[str, Any]):
        self.fixture = fixture

    def scan(self, *, viewing_scope_id: str, block_range: BlockRange, network: str, pool: str) -> ScanResult:
        if self.fixture.get("error"):
            return ScanResult(status="ERROR", owned_commitments=[], error=str(self.fixture["error"]))
        if self.fixture.get("network") != network:
            return ScanResult(status="ERROR", owned_commitments=[], error="fixture network mismatch")
        if self.fixture.get("pool") != pool:
            return ScanResult(status="ERROR", owned_commitments=[], error="fixture pool mismatch")

        owned: list[str] = []
        for output in self.fixture.get("outputs", []):
            height = int(output["height"])
            if not block_range.contains(height):
                continue
            if output.get("viewing_scope_id") != viewing_scope_id:
                continue
            owned.append(normalize_commitment(str(output["commitment"])))
        return ScanResult(status="OK", owned_commitments=sorted(set(owned)))


class ZcashViewingKeyScanner:
    """TEE-local Zcash scanning boundary.

    The scanner fetches compact blocks inside the enclave/container and invokes a
    scanner command to do real Sapling/Orchard trial-decryption. The requester is
    not allowed to submit owned commitments in production. If the scanner command
    is unavailable or fails, this returns ERROR and therefore cannot mint PASS.
    """

    def __init__(
        self,
        *,
        viewing_capability: EncryptedViewingCapability,
        chain_source: ChainSource,
        scanner_cmd: str | None = None,
        lightwalletd_client_factory: Any | None = None,
    ):
        self.viewing_capability = viewing_capability
        self.chain_source = chain_source
        self.scanner_cmd = (
            scanner_cmd if scanner_cmd is not None else os.environ.get("CLEAN_WALLET_ZCASH_SCANNER_CMD", "")
        )
        self.lightwalletd_client_factory = lightwalletd_client_factory or LightwalletdClient

    def scan(self, *, viewing_scope_id: str, block_range: BlockRange, network: str, pool: str) -> ScanResult:
        viewing_key = ""
        try:
            viewing_key = decrypt_viewing_capability(self.viewing_capability)
            compact_blocks = self._load_compact_blocks(block_range)
            owned_commitments = self._run_scanner_command(
                viewing_key=viewing_key,
                viewing_scope_id=viewing_scope_id,
                block_range=block_range,
                network=network,
                pool=pool,
                compact_blocks=compact_blocks,
            )
            return ScanResult(status="OK", owned_commitments=sorted(set(owned_commitments)))
        except Exception as exc:  # noqa: BLE001 - proof layer must return ERROR, never PASS.
            return ScanResult(status="ERROR", owned_commitments=[], error=f"zcash scanner failed: {exc}")
        finally:
            viewing_key = ""

    def _load_compact_blocks(self, block_range: BlockRange) -> list[dict[str, Any]]:
        if self.chain_source.source_type != "lightwalletd":
            raise ValueError("production Zcash scanning currently requires chain_source.type=lightwalletd")
        client = self.lightwalletd_client_factory(self.chain_source.endpoint or "")
        return client.fetch_compact_blocks(start=block_range.start, end=block_range.end)

    def _run_scanner_command(
        self,
        *,
        viewing_key: str,
        viewing_scope_id: str,
        block_range: BlockRange,
        network: str,
        pool: str,
        compact_blocks: list[dict[str, Any]],
    ) -> list[str]:
        if not self.scanner_cmd.strip():
            raise RuntimeError("CLEAN_WALLET_ZCASH_SCANNER_CMD is not configured")
        request = {
            "schema_version": "clean-wallet-zcash-scan-request-v0",
            "viewing_key": viewing_key,
            "viewing_scope_id": viewing_scope_id,
            "viewing_capability_type": self.viewing_capability.capability_type,
            "key_id": self.viewing_capability.key_id,
            "network": network,
            "pool": pool,
            "block_range": block_range.to_dict(),
            "compact_blocks": compact_blocks,
        }
        proc = subprocess.run(
            shlex.split(self.scanner_cmd),
            input=json.dumps(request).encode("utf-8"),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=float(os.environ.get("CLEAN_WALLET_ZCASH_SCANNER_TIMEOUT", "120")),
            check=False,
        )
        if proc.returncode != 0:
            stderr = proc.stderr.decode("utf-8", errors="replace").strip().splitlines()[:1]
            raise RuntimeError("scanner command failed" + (f": {stderr[0]}" if stderr else ""))
        try:
            response = json.loads(proc.stdout.decode("utf-8"))
        except json.JSONDecodeError as exc:
            raise RuntimeError("scanner command returned invalid JSON") from exc
        commitments = response.get("owned_commitments")
        if not isinstance(commitments, list):
            raise RuntimeError("scanner command must return owned_commitments list")
        return [normalize_commitment(str(item)) for item in commitments]
