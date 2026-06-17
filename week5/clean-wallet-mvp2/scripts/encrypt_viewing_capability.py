#!/usr/bin/env python3
"""Encrypt a Zcash viewing capability for Clean Wallet `/proof`.

Reads the FVK/UFVK/UIVK/IVK from hidden terminal input by default. Do not pass
viewing keys as shell arguments.
"""

from __future__ import annotations

import argparse
import getpass
import json
import secrets
import sys
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from clean_wallet.client_encrypt import (  # noqa: E402
    build_proof_payload,
    encrypt_viewing_capability,
    extract_encryption_key_descriptor,
)


def _json_load_path(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as fh:
        value = json.load(fh)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def _json_dump_path(path: str | None, payload: dict[str, Any]) -> None:
    rendered = json.dumps(payload, indent=2, sort_keys=True)
    if path:
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(rendered + "\n")
    else:
        print(rendered)


def _read_secret(args: argparse.Namespace) -> str:
    if args.viewing_key_stdin:
        secret = sys.stdin.read().strip()
    else:
        secret = getpass.getpass(f"Enter Zcash {args.capability_type.upper()} / viewing capability: ").strip()
    if not secret:
        raise ValueError("viewing capability input was empty")
    return secret


def _service_url(base: str, path: str, query: dict[str, str] | None = None) -> str:
    parsed = urllib.parse.urlparse(base)
    if not parsed.scheme or not parsed.netloc:
        raise ValueError("--service-url must include scheme and host, e.g. http://127.0.0.1:8080")
    clean = base.rstrip("/") + path
    if query:
        return clean + "?" + urllib.parse.urlencode(query)
    return clean


def _fetch_json(url: str) -> dict[str, Any]:
    with urllib.request.urlopen(url, timeout=30) as response:  # noqa: S310 - user-supplied service URL is intended.
        payload = json.loads(response.read().decode("utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("HTTP response was not a JSON object")
    return payload


def _post_json(url: str, payload: dict[str, Any]) -> dict[str, Any]:
    body = json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=120) as response:  # noqa: S310 - user-supplied service URL is intended.
        result = json.loads(response.read().decode("utf-8"))
    if not isinstance(result, dict):
        raise ValueError("/proof response was not a JSON object")
    return result


def _attestation_url(args: argparse.Namespace, nonce: str) -> str:
    if args.attestation_url:
        parsed = urllib.parse.urlparse(args.attestation_url)
        query = dict(urllib.parse.parse_qsl(parsed.query))
        query.setdefault("purpose", "enclave-key")
        query.setdefault("nonce", nonce)
        return urllib.parse.urlunparse(parsed._replace(query=urllib.parse.urlencode(query)))
    if args.service_url:
        return _service_url(args.service_url, "/attestation", {"purpose": "enclave-key", "nonce": nonce})
    raise ValueError("provide --service-url or --attestation-url")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Encrypt FVK/UFVK/UIVK/IVK for Clean Wallet /proof")
    parser.add_argument("--service-url", help="Clean Wallet service base URL, e.g. http://127.0.0.1:8080")
    parser.add_argument("--attestation-url", help="Full /attestation URL; purpose/nonce are added if missing")
    parser.add_argument("--proof-url", help="Full /proof URL; defaults to <service-url>/proof when --submit is used")
    parser.add_argument("--submit", action="store_true", help="POST generated payload to /proof and output the report")
    parser.add_argument("--capability-type", default="fvk", choices=["ivk", "uivk", "fvk", "ufvk"])
    parser.add_argument("--viewing-key-stdin", action="store_true", help="Read viewing key from stdin instead of hidden prompt")
    parser.add_argument("--blacklist-manifest", required=True, help="Path to signed blacklist manifest JSON")
    parser.add_argument("--network", default="mainnet")
    parser.add_argument("--pool", default="orchard")
    parser.add_argument("--block-start", required=True, type=int)
    parser.add_argument("--block-end", required=True, type=int)
    parser.add_argument("--viewing-scope-id", default=None, help="Client-chosen scope label; omitted labels get a random one")
    parser.add_argument("--lightwalletd-endpoint", default="https://lightwalletd.mainnet.cipherscan.app:443")
    parser.add_argument("--expected-measurement", help="Optional measurement to match in mock/local quote responses")
    parser.add_argument("--nonce", default=None, help="Client nonce for key attestation; random by default")
    parser.add_argument("--out", help="Write generated proof payload/report to this file instead of stdout")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    nonce = args.nonce or secrets.token_hex(16)
    attestation = _fetch_json(_attestation_url(args, nonce))
    descriptor = extract_encryption_key_descriptor(
        attestation,
        expected_nonce=nonce,
        expected_measurement=args.expected_measurement,
    )
    viewing_key = _read_secret(args)
    try:
        envelope = encrypt_viewing_capability(
            viewing_key,
            descriptor,
            capability_type=args.capability_type,
        )
    finally:
        viewing_key = ""
    payload = build_proof_payload(
        encrypted_viewing_capability=envelope,
        blacklist_manifest=_json_load_path(args.blacklist_manifest),
        network=args.network,
        pool=args.pool,
        block_start=args.block_start,
        block_end=args.block_end,
        viewing_scope_id=args.viewing_scope_id or f"scope-{secrets.token_hex(8)}",
        lightwalletd_endpoint=args.lightwalletd_endpoint,
    )
    if args.submit:
        proof_url = args.proof_url or (args.service_url and _service_url(args.service_url, "/proof"))
        if not proof_url:
            raise ValueError("--submit requires --proof-url or --service-url")
        _json_dump_path(args.out, _post_json(proof_url, payload))
    else:
        _json_dump_path(args.out, payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
