"""Small deterministic crypto helpers for the PoC.

This is not a production signature system. HMAC is used only to make the demo
self-contained without adding dependencies. Production should replace this with
issuer signatures and hardware-backed attestation quote verification.
"""

from __future__ import annotations

import hmac
import json
from hashlib import sha256
from typing import Any

COMMITMENT_HEX_BYTES = 32


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_hex(data: bytes | str) -> str:
    if isinstance(data, str):
        data = data.encode("utf-8")
    return sha256(data).hexdigest()


def hmac_sha256_hex(key: str, message: bytes | str) -> str:
    if isinstance(message, str):
        message = message.encode("utf-8")
    return hmac.new(key.encode("utf-8"), message, sha256).hexdigest()


def constant_time_equal(left: str, right: str) -> bool:
    return hmac.compare_digest(left, right)


def normalize_commitment(value: str) -> str:
    raw = value.strip().lower()
    if raw.startswith("0x"):
        raw = raw[2:]
    expected_len = COMMITMENT_HEX_BYTES * 2
    if len(raw) != expected_len:
        raise ValueError(f"commitment must be {expected_len} hex characters")
    try:
        bytes.fromhex(raw)
    except ValueError as exc:
        raise ValueError("commitment must be valid hex") from exc
    return raw


def root_for_commitments(commitments: list[str]) -> str:
    """Return a deterministic manifest root for normalized commitments.

    This is a PoC manifest root, not a Zcash note commitment tree root. The
    blacklist issuer uses it to bind a signed canonical list.
    """

    normalized = sorted({normalize_commitment(item) for item in commitments})
    return sha256_hex(canonical_json(normalized))
