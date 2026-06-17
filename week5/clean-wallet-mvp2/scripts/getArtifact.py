#!/usr/bin/env python3
"""Extract local Clean Wallet test artifacts from a Zcash viewing capability.

This is intentionally a local/debug helper. It reads the UFVK/FVK/UIVK/IVK from
hidden input, fetches compact blocks from lightwalletd, runs the local Rust
scanner, and writes the owned commitments that `/proof` normally keeps private.
Do not use this helper in the public attested proof path.
"""

from __future__ import annotations

import argparse
import getpass
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from clean_wallet.blacklist import build_manifest  # noqa: E402
from clean_wallet.crypto import sha256_hex  # noqa: E402
from clean_wallet.lightwalletd import LightwalletdClient  # noqa: E402

DEFAULT_SCANNER = ROOT / "zcash_scanner" / "target" / "release" / "clean-wallet-zcash-scanner"
DEFAULT_ENDPOINT = "https://lightwalletd.mainnet.cipherscan.app:443"
DEFAULT_BLACKLIST_KEY = "demo-blacklist-issuer-key"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Locally scan a UFVK/FVK/UIVK/IVK and export owned commitments as a test artifact."
    )
    parser.add_argument("--capability-type", default="ufvk", choices=["ivk", "uivk", "fvk", "ufvk"])
    parser.add_argument("--viewing-key-stdin", action="store_true", help="Read viewing key from stdin instead of prompt")
    parser.add_argument("--network", default="mainnet")
    parser.add_argument("--pool", default="orchard")
    parser.add_argument("--block-start", required=True, type=int)
    parser.add_argument("--block-end", required=True, type=int)
    parser.add_argument("--lightwalletd-endpoint", default=DEFAULT_ENDPOINT)
    parser.add_argument("--scanner-bin", default=str(DEFAULT_SCANNER))
    parser.add_argument("--no-build", action="store_true", help="Do not auto-build the Rust scanner if missing")
    parser.add_argument(
        "--out",
        default=None,
        help="Artifact JSON output path. Defaults to artifacts/owned-commitments-<start>-<end>.json",
    )
    parser.add_argument(
        "--blacklist-out",
        default=None,
        help="Optional path for a demo blacklist manifest containing the first owned commitment.",
    )
    parser.add_argument("--blacklist-key", default=DEFAULT_BLACKLIST_KEY)
    parser.add_argument("--issuer", default="local-artifact-debug")
    parser.add_argument("--version", default=None)
    return parser.parse_args(argv)


def read_viewing_key(args: argparse.Namespace) -> str:
    if args.viewing_key_stdin:
        viewing_key = sys.stdin.read().strip()
    else:
        viewing_key = getpass.getpass(
            f"Enter Zcash {args.capability_type.upper()} / viewing capability: "
        ).strip()
    if not viewing_key:
        raise ValueError("viewing capability input was empty")
    return viewing_key


def ensure_scanner(scanner_bin: Path, *, no_build: bool) -> None:
    if scanner_bin.exists():
        return
    if no_build:
        raise FileNotFoundError(f"scanner binary not found: {scanner_bin}")
    subprocess.run(
        ["cargo", "build", "--release", "--manifest-path", str(ROOT / "zcash_scanner" / "Cargo.toml")],
        cwd=ROOT,
        check=True,
    )
    if not scanner_bin.exists():
        raise FileNotFoundError(f"scanner binary was not produced: {scanner_bin}")


def scan_owned_commitments(args: argparse.Namespace, viewing_key: str) -> tuple[list[str], dict[str, Any], dict[str, Any]]:
    blocks = LightwalletdClient(args.lightwalletd_endpoint).fetch_compact_blocks(
        start=args.block_start,
        end=args.block_end,
    )
    scanner_request = {
        "viewing_key": viewing_key,
        "viewing_capability_type": args.capability_type,
        "network": args.network,
        "compact_blocks": blocks,
    }
    proc = subprocess.run(
        [args.scanner_bin],
        input=json.dumps(scanner_request),
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"scanner failed with exit code {proc.returncode}: {proc.stderr.strip()}")
    response = json.loads(proc.stdout)
    commitments = response.get("owned_commitments")
    if not isinstance(commitments, list) or not all(isinstance(item, str) for item in commitments):
        raise ValueError("scanner response did not contain owned_commitments list")
    derived_addresses = response.get("derived_addresses") or {}
    if not isinstance(derived_addresses, dict):
        raise ValueError("scanner response derived_addresses was not an object")
    block_summary = {
        "count": len(blocks),
        "first_height": blocks[0]["height"] if blocks else None,
        "last_height": blocks[-1]["height"] if blocks else None,
        "tx_count": sum(len(block.get("vtx", [])) for block in blocks),
        "orchard_action_count": sum(
            len(tx.get("actions", [])) for block in blocks for tx in block.get("vtx", [])
        ),
        "sapling_output_count": sum(
            len(tx.get("outputs", [])) for block in blocks for tx in block.get("vtx", [])
        ),
        "first_chain_metadata": blocks[0].get("chainMetadata") if blocks else None,
    }
    return commitments, block_summary, derived_addresses


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def default_out_path(args: argparse.Namespace) -> Path:
    return ROOT / "artifacts" / f"owned-commitments-{args.network}-{args.block_start}-{args.block_end}.json"


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    scanner_bin = Path(args.scanner_bin)
    ensure_scanner(scanner_bin, no_build=args.no_build)
    args.scanner_bin = str(scanner_bin)

    viewing_key = read_viewing_key(args)
    try:
        commitments, block_summary, derived_addresses = scan_owned_commitments(args, viewing_key)
    finally:
        viewing_key = ""

    artifact = {
        "schema_version": "clean-wallet-local-artifact-v0",
        "created_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "network": args.network,
        "pool": args.pool,
        "block_range": {"start": args.block_start, "end": args.block_end},
        "lightwalletd_endpoint": args.lightwalletd_endpoint,
        "capability_type": args.capability_type,
        "block_summary": block_summary,
        "derived_addresses": derived_addresses,
        "owned_commitment_count": len(commitments),
        "first_owned_commitment": commitments[0] if commitments else None,
        "owned_commitments": commitments,
    }
    artifact["artifact_hash"] = sha256_hex(json.dumps(artifact, sort_keys=True, separators=(",", ":")))

    out_path = Path(args.out) if args.out else default_out_path(args)
    write_json(out_path, artifact)
    print(f"Wrote artifact: {out_path}")
    print(f"owned_commitment_count={len(commitments)}")
    if commitments:
        print(f"first_owned_commitment={commitments[0]}")
    else:
        print("first_owned_commitment=<none>")
    if derived_addresses.get("default_unified_address"):
        print(f"default_unified_address={derived_addresses['default_unified_address']}")

    if args.blacklist_out:
        if not commitments:
            raise ValueError("--blacklist-out requested, but scanner found no owned commitments")
        manifest = build_manifest(
            [commitments[0]],
            network=args.network,
            pool=args.pool,
            issuer=args.issuer,
            version=args.version or f"local-{args.block_start}-{args.block_end}",
            signing_key=args.blacklist_key,
        )
        blacklist_path = Path(args.blacklist_out)
        write_json(blacklist_path, manifest.to_public_dict())
        print(f"Wrote demo blacklist manifest: {blacklist_path}")
        print(f"blacklist_manifest_hash={manifest.manifest_hash}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
