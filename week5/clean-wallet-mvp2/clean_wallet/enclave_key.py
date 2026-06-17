"""Attested encryption-key descriptor for viewing capability submission.

This module exposes the public enclave encryption-key descriptor, binds that
descriptor into attestation payloads, and decrypts viewing capabilities inside
the enclave process with X25519 + ChaCha20-Poly1305.
"""

from __future__ import annotations

import base64
import os
from typing import Mapping

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import x25519
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

from .crypto import canonical_json, sha256_hex

ENCLAVE_KEY_CONTRACT_VERSION = "clean-wallet-enclave-key-v0"
DEFAULT_ENCLAVE_KEY_SCHEME = "x25519-chacha20poly1305-v0"

_RUNTIME_EPHEMERAL_PRIVATE_KEY = x25519.X25519PrivateKey.generate()


def _runtime_ephemeral_private_key_bytes() -> bytes:
    return _RUNTIME_EPHEMERAL_PRIVATE_KEY.private_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PrivateFormat.Raw,
        encryption_algorithm=serialization.NoEncryption(),
    )


def _auto_generate_enabled(source: Mapping[str, str]) -> bool:
    return source.get("CLEAN_WALLET_AUTO_GENERATE_ENCLAVE_KEY", "").strip().lower() in {
        "1",
        "true",
        "yes",
    }


def _configured_private_key_bytes(source: Mapping[str, str]) -> bytes | None:
    private_key_b64 = source.get("CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64", "").strip()
    if private_key_b64:
        private_key_bytes = _b64decode_field(private_key_b64, "CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64")
        if len(private_key_bytes) != 32:
            raise ValueError("CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64 must encode 32 bytes")
        return private_key_bytes
    if _auto_generate_enabled(source):
        return _runtime_ephemeral_private_key_bytes()
    return None


def _public_key_for_private_bytes(private_key_bytes: bytes) -> str:
    private_key = x25519.X25519PrivateKey.from_private_bytes(private_key_bytes)
    public_bytes = private_key.public_key().public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    return base64.b64encode(public_bytes).decode("ascii")


def enclave_encryption_key_descriptor(env: Mapping[str, str] | None = None) -> dict[str, str]:
    """Return the public descriptor clients use before encrypting an IVK/FVK/UFVK.

    A stable private key may be supplied via `CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64`.
    For Phala PoC deployment, `CLEAN_WALLET_AUTO_GENERATE_ENCLAVE_KEY=1` creates
    a process-local ephemeral private key and exposes only its public key in the
    attested descriptor. Without either option, the descriptor is `unconfigured`.
    """

    source = env or os.environ
    public_key = source.get("CLEAN_WALLET_ENCLAVE_PUBLIC_KEY", "").strip()
    key_origin = "env-public" if public_key else ""
    if not public_key:
        private_key_bytes = _configured_private_key_bytes(source)
        if private_key_bytes is not None:
            public_key = _public_key_for_private_bytes(private_key_bytes)
            key_origin = (
                "env-private"
                if source.get("CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64", "").strip()
                else "runtime-ephemeral"
            )
    descriptor = {
        "contract_version": ENCLAVE_KEY_CONTRACT_VERSION,
        "scheme": source.get("CLEAN_WALLET_ENCLAVE_KEY_SCHEME", DEFAULT_ENCLAVE_KEY_SCHEME).strip()
        or DEFAULT_ENCLAVE_KEY_SCHEME,
        "key_id": source.get("CLEAN_WALLET_ENCLAVE_KEY_ID", "unconfigured").strip() or "unconfigured",
        "status": "configured" if public_key else "unconfigured",
    }
    if public_key:
        descriptor["public_key"] = public_key
        descriptor["key_origin"] = key_origin
    return descriptor


def enclave_key_attestation_payload(*, nonce: str, env: Mapping[str, str] | None = None) -> dict[str, object]:
    """Build the public payload whose hash is bound into quote reportData."""

    if not isinstance(nonce, str) or not nonce.strip():
        raise ValueError("nonce is required for enclave key attestation")
    return {
        "purpose": "encrypted-viewing-capability-key",
        "nonce": nonce,
        "encryption_key": enclave_encryption_key_descriptor(env),
    }


def enclave_key_attestation_hash(payload: dict[str, object]) -> str:
    """Hash the key-attestation payload for dstack/TDX reportData binding."""

    return sha256_hex(canonical_json(payload))


def _b64decode_field(value: str, field: str) -> bytes:
    try:
        return base64.b64decode(value, validate=True)
    except Exception as exc:  # noqa: BLE001 - sanitize secret parsing errors.
        raise ValueError(f"{field} must be valid base64") from exc


def _derive_capability_key(shared_secret: bytes, *, key_id: str, capability_type: str) -> bytes:
    return HKDF(
        algorithm=hashes.SHA256(),
        length=32,
        salt=f"clean-wallet:{key_id}:{capability_type}".encode("utf-8"),
        info=b"encrypted-viewing-capability-v0",
    ).derive(shared_secret)


def decrypt_viewing_capability(encrypted: object, env: Mapping[str, str] | None = None) -> str:
    """Decrypt a requester viewing capability inside the enclave process.

    Supported production PoC scheme: `x25519-chacha20poly1305-v0`. The request
    envelope must provide base64 `ciphertext`, `nonce`, and
    `ephemeral_public_key`/`sender_public_key`. The enclave private key is either
    supplied via `CLEAN_WALLET_ENCLAVE_PRIVATE_KEY_B64` or generated in-memory
    when `CLEAN_WALLET_AUTO_GENERATE_ENCLAVE_KEY=1`.

    The returned plaintext is intentionally only handed to the scanner command;
    callers must not log or include it in reports.
    """

    source = env or os.environ
    scheme = getattr(encrypted, "scheme", "")
    if scheme != "x25519-chacha20poly1305-v0":
        raise ValueError("unsupported encrypted viewing capability scheme")
    private_key_bytes = _configured_private_key_bytes(source)
    if private_key_bytes is None:
        raise ValueError("enclave viewing-capability private key is not configured")
    peer_public_b64 = (
        getattr(encrypted, "ephemeral_public_key", None)
        or getattr(encrypted, "sender_public_key", None)
        or ""
    )
    nonce_b64 = getattr(encrypted, "nonce", None) or ""
    ciphertext_b64 = getattr(encrypted, "ciphertext", "")
    peer_public_bytes = _b64decode_field(peer_public_b64, "ephemeral_public_key")
    nonce = _b64decode_field(nonce_b64, "nonce")
    ciphertext = _b64decode_field(ciphertext_b64, "ciphertext")
    if len(peer_public_bytes) != 32:
        raise ValueError("ephemeral_public_key must encode 32 bytes")
    if len(nonce) != 12:
        raise ValueError("nonce must encode 12 bytes for chacha20poly1305")

    private_key = x25519.X25519PrivateKey.from_private_bytes(private_key_bytes)
    peer_public_key = x25519.X25519PublicKey.from_public_bytes(peer_public_bytes)
    shared_secret = private_key.exchange(peer_public_key)
    key = _derive_capability_key(
        shared_secret,
        key_id=str(getattr(encrypted, "key_id", "")),
        capability_type=str(getattr(encrypted, "capability_type", "")),
    )
    aad = canonical_json(
        {
            "scheme": scheme,
            "key_id": getattr(encrypted, "key_id", None),
            "capability_type": getattr(encrypted, "capability_type", None),
        }
    ).encode("utf-8")
    plaintext = ChaCha20Poly1305(key).decrypt(nonce, ciphertext, aad)
    try:
        return plaintext.decode("utf-8")
    finally:
        plaintext = b""


def public_key_for_private_key_b64(private_key_b64: str) -> str:
    """Return base64 X25519 public key for local/demo provisioning tests."""

    private_key_bytes = _b64decode_field(private_key_b64, "private_key")
    return _public_key_for_private_bytes(private_key_bytes)
