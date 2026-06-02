"""Command line interface for the Clean Wallet PoC."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from . import __version__
from .attestation import build_attestor, build_verifier, package_measurement
from .blacklist import build_manifest, load_manifest
from .proof import ProofRequest, create_report, verify_report
from .scanner import BlockRange, FixtureScanner

DEFAULT_BLACKLIST_KEY = "demo-blacklist-issuer-key"
DEFAULT_ATTESTATION_KEY = "demo-attestation-key"


def read_json(path: str | Path) -> dict:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write_json(path: str | Path, data: dict) -> None:
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def cmd_measurement(_: argparse.Namespace) -> int:
    print(package_measurement())
    return 0


def cmd_build_blacklist(args: argparse.Namespace) -> int:
    commitments = [
        line.strip()
        for line in Path(args.commitments).read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
    manifest = build_manifest(
        commitments,
        network=args.network,
        pool=args.pool,
        issuer=args.issuer,
        version=args.version,
        signing_key=args.blacklist_key,
    )
    write_json(args.output, manifest.to_public_dict())
    print(f"blacklist manifest written: {args.output}")
    print(f"root: {manifest.root}")
    return 0


def cmd_request_proof(args: argparse.Namespace) -> int:
    manifest = load_manifest(read_json(args.blacklist))
    fixture = read_json(args.fixture)
    request = ProofRequest(
        network=args.network,
        pool=args.pool,
        block_range=BlockRange(args.start_block, args.end_block),
        viewing_scope_id=args.viewing_scope_id,
    )
    attestor = build_attestor(args.attestor, attestation_key=args.attestation_key)
    report = create_report(
        request=request,
        scanner=FixtureScanner(fixture),
        manifest=manifest,
        blacklist_signing_key=args.blacklist_key,
        attestor=attestor,
        scanner_version=__version__,
    )
    write_json(args.output, report)
    print(f"proof report written: {args.output}")
    print(f"result: {report['result']}")
    return 0 if report["result"] in {"PASS", "FAIL"} else 2


def cmd_verify_report(args: argparse.Namespace) -> int:
    manifest = load_manifest(read_json(args.blacklist))
    report = read_json(args.report)
    quote = dict(report.get("signature_or_quote", {}))
    attestor_kind = args.attestor or str(quote.get("mode", "mock-tee-v0"))
    measurement = args.measurement or str(
        report.get("measurement") or quote.get("measurement") or package_measurement()
    )
    attestor = build_verifier(
        attestor_kind,
        attestation_key=args.attestation_key,
        measurement=str(quote.get("measurement", measurement)),
    )
    verify_report(
        report=report,
        manifest=manifest,
        blacklist_signing_key=args.blacklist_key,
        attestor=attestor,
        allowed_measurements={measurement},
        max_age_seconds=args.max_age_seconds,
    )
    print("report verification: PASS")
    print(f"result: {report['result']}")
    print(
        f"scope: {report['network']} {report['pool']} "
        f"blocks {report['block_range']['start']}..{report['block_range']['end']}"
    )
    print("note: PASS is bounded to submitted scope/range/list; it is not a global innocence proof.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Clean Wallet non-association PoC")
    sub = parser.add_subparsers(dest="command", required=True)

    measurement = sub.add_parser("measurement", help="print mock enclave measurement")
    measurement.set_defaults(func=cmd_measurement)

    build = sub.add_parser("build-blacklist", help="build signed blacklist manifest")
    build.add_argument("--commitments", required=True)
    build.add_argument("--output", required=True)
    build.add_argument("--network", default="regtest")
    build.add_argument("--pool", default="orchard")
    build.add_argument("--issuer", default="demo-issuer")
    build.add_argument("--version", default="v0")
    build.add_argument("--blacklist-key", default=DEFAULT_BLACKLIST_KEY)
    build.set_defaults(func=cmd_build_blacklist)

    proof = sub.add_parser("request-proof", help="scan fixture, compare blacklist, emit report")
    proof.add_argument("--fixture", required=True)
    proof.add_argument("--blacklist", required=True)
    proof.add_argument("--output", required=True)
    proof.add_argument("--viewing-scope-id", required=True, help="demo scope id; never included raw in report")
    proof.add_argument("--network", default="regtest")
    proof.add_argument("--pool", default="orchard")
    proof.add_argument("--start-block", type=int, required=True)
    proof.add_argument("--end-block", type=int, required=True)
    proof.add_argument("--blacklist-key", default=DEFAULT_BLACKLIST_KEY)
    proof.add_argument("--attestation-key", default=DEFAULT_ATTESTATION_KEY)
    proof.add_argument(
        "--attestor",
        default="mock",
        choices=["mock", "phala"],
        help="attestation backend: mock for local demo, phala inside Phala/dstack CVM",
    )
    proof.set_defaults(func=cmd_request_proof)

    verify = sub.add_parser("verify-report", help="verify report integrity, blacklist binding, and mock quote")
    verify.add_argument("--report", required=True)
    verify.add_argument("--blacklist", required=True)
    verify.add_argument("--measurement", help="allowlisted measurement; defaults to current package measurement")
    verify.add_argument("--max-age-seconds", type=int)
    verify.add_argument("--blacklist-key", default=DEFAULT_BLACKLIST_KEY)
    verify.add_argument("--attestation-key", default=DEFAULT_ATTESTATION_KEY)
    verify.add_argument(
        "--attestor",
        choices=["mock", "phala"],
        help="override verifier backend; defaults to report quote mode",
    )
    verify.set_defaults(func=cmd_verify_report)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return args.func(args)
    except Exception as exc:  # noqa: BLE001 - CLI boundary should print concise failure.
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
