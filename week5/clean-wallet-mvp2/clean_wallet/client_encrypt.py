"""Client-side encrypted viewing-capability helper.

This module is intentionally outside the enclave decrypt path. It helps a
requester encrypt a UFVK/FVK/UIVK/IVK to the enclave's attested X25519 public
key without ever placing plaintext viewing-key material into the `/proof`
payload.
"""

from __future__ import annotations

import base64
import secrets
from typing import Any

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import x25519
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

from .crypto import canonical_json, sha256_hex
from .enclave_key import DEFAULT_ENCLAVE_KEY_SCHEME, _derive_capability_key

SUPPORTED_CAPABILITY_TYPES = {"ivk", "uivk", "fvk", "ufvk"}


def _b64decode_public_key(value: str) -> bytes:
    try:
        decoded = base64.b64decode(value, validate=True)
    except Exception as exc:  # noqa: BLE001 - client-facing input validation.
        raise ValueError("enclave public_key must be valid base64") from exc
    if len(decoded) != 32:
        raise ValueError("enclave public_key must encode 32 raw X25519 bytes")
    return decoded


def _b64(value: bytes) -> str:
    return base64.b64encode(value).decode("ascii")


def extract_encryption_key_descriptor(
    attestation_response: dict[str, Any],
    *,
    expected_nonce: str | None = None,
    expected_measurement: str | None = None,
) -> dict[str, str]:
    """Extract and sanity-check an enclave encryption-key descriptor.

    This verifies the local `attestation_payload_hash` binding. Full TDX quote
    verification still belongs to the caller/verifier policy, but this catches
    malformed or mismatched key-attestation responses before encrypting secrets.
    """

    payload = attestation_response.get("attestation_payload")
    if not isinstance(payload, dict):
        raise ValueError("attestation response missing attestation_payload")
    if expected_nonce is not None and payload.get("nonce") != expected_nonce:
        raise ValueError("attestation nonce mismatch")
    advertised_hash = attestation_response.get("attestation_payload_hash")
    if not isinstance(advertised_hash, str) or not advertised_hash:
        raise ValueError("attestation response missing attestation_payload_hash")
    computed_hash = sha256_hex(canonical_json(payload))
    if advertised_hash != computed_hash:
        raise ValueError("attestation_payload_hash does not match attestation_payload")

    quote = attestation_response.get("quote")
    if isinstance(quote, dict):
        report_data = quote.get("report_data") or quote.get("reportData")
        if report_data is not None and str(report_data) != advertised_hash:
            raise ValueError("quote report_data does not bind the enclave key payload hash")
        if expected_measurement is not None and quote.get("measurement") != expected_measurement:
            raise ValueError("attestation measurement mismatch")

    descriptor = payload.get("encryption_key")
    if not isinstance(descriptor, dict):
        raise ValueError("attestation payload missing encryption_key descriptor")
    normalized = {str(key): str(value) for key, value in descriptor.items()}
    if normalized.get("status") != "configured":
        raise ValueError("enclave encryption key is not configured")
    if normalized.get("scheme") != DEFAULT_ENCLAVE_KEY_SCHEME:
        raise ValueError("unsupported enclave encryption key scheme")
    if not normalized.get("key_id"):
        raise ValueError("enclave encryption key descriptor missing key_id")
    _b64decode_public_key(normalized.get("public_key", ""))
    return normalized


def encrypt_viewing_capability(
    viewing_key: str,
    descriptor: dict[str, str],
    *,
    capability_type: str,
) -> dict[str, str]:
    """Encrypt a viewing capability to an enclave descriptor.

    The returned envelope is safe to include in `/proof`; it does not include
    the plaintext viewing key.
    """

    if not isinstance(viewing_key, str) or not viewing_key.strip():
        raise ValueError("viewing capability must be non-empty")
    capability = capability_type.lower().strip()
    if capability not in SUPPORTED_CAPABILITY_TYPES:
        raise ValueError("capability_type must be one of ivk, uivk, fvk, ufvk")
    scheme = descriptor.get("scheme", "")
    if scheme != DEFAULT_ENCLAVE_KEY_SCHEME:
        raise ValueError("unsupported enclave encryption key scheme")
    key_id = descriptor.get("key_id", "").strip()
    if not key_id:
        raise ValueError("enclave encryption key descriptor missing key_id")

    enclave_public_key = x25519.X25519PublicKey.from_public_bytes(
        _b64decode_public_key(descriptor.get("public_key", ""))
    )
    requester_private_key = x25519.X25519PrivateKey.generate()
    requester_public_bytes = requester_private_key.public_key().public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    shared_secret = requester_private_key.exchange(enclave_public_key)
    key = _derive_capability_key(shared_secret, key_id=key_id, capability_type=capability)
    nonce = secrets.token_bytes(12)
    aad = canonical_json(
        {
            "scheme": scheme,
            "key_id": key_id,
            "capability_type": capability,
        }
    ).encode("utf-8")
    ciphertext = ChaCha20Poly1305(key).encrypt(nonce, viewing_key.encode("utf-8"), aad)
    return {
        "scheme": scheme,
        "capability_type": capability,
        "ciphertext": _b64(ciphertext),
        "key_id": key_id,
        "ephemeral_public_key": _b64(requester_public_bytes),
        "nonce": _b64(nonce),
    }


def build_proof_payload(
    *,
    encrypted_viewing_capability: dict[str, str],
    blacklist_manifest: dict[str, Any],
    network: str,
    pool: str,
    block_start: int,
    block_end: int,
    viewing_scope_id: str,
    lightwalletd_endpoint: str,
) -> dict[str, Any]:
    if block_start < 0 or block_end < block_start:
        raise ValueError("block range must satisfy 0 <= start <= end")
    if not viewing_scope_id.strip():
        raise ValueError("viewing_scope_id is required")
    if not lightwalletd_endpoint.strip():
        raise ValueError("lightwalletd_endpoint is required")
    return {
        "request": {
            "network": network,
            "pool": pool,
            "block_range": {"start": block_start, "end": block_end},
            "viewing_scope_id": viewing_scope_id,
            "encrypted_viewing_capability": encrypted_viewing_capability,
            "chain_source": {
                "type": "lightwalletd",
                "endpoint": lightwalletd_endpoint,
            },
        },
        "blacklist_manifest": blacklist_manifest,
    }
