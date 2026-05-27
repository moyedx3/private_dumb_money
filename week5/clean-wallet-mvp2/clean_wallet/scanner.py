"""Fixture scanner boundary.

The production scanner should trial-decrypt compact blocks with a UFVK/FVK and
bind owned outputs to their on-chain Orchard note commitments. This fixture
scanner keeps the same output contract while avoiding live chain dependencies.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .crypto import normalize_commitment, sha256_hex


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
