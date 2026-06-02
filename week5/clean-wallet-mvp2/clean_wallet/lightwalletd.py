"""lightwalletd compact-block client for enclave-side Zcash scanning.

This module deliberately fetches block data inside the service process instead
of accepting prover-submitted scan results. It only returns compact block data;
actual Sapling/Orchard trial-decryption is delegated to a scanner command that
runs inside the same enclave/container.
"""

from __future__ import annotations

import importlib
import os
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

PROTO_DIR = Path(__file__).resolve().parent / "proto"
GENERATED_DIR = Path(tempfile.gettempdir()) / "clean_wallet_lightwalletd_proto_v2"


def _ensure_generated() -> tuple[Any, Any]:
    """Generate and import Python gRPC bindings from vendored lightwalletd protos."""

    service_py = GENERATED_DIR / "service_pb2.py"
    service_grpc_py = GENERATED_DIR / "service_pb2_grpc.py"
    if not service_py.exists() or not service_grpc_py.exists():
        from grpc_tools import protoc  # type: ignore

        GENERATED_DIR.mkdir(parents=True, exist_ok=True)
        # service.proto imports compact_formats.proto by basename.
        for name in ("compact_formats.proto", "service.proto"):
            shutil.copyfile(PROTO_DIR / name, GENERATED_DIR / name)
        rc = protoc.main(
            [
                "grpc_tools.protoc",
                f"-I{GENERATED_DIR}",
                f"--python_out={GENERATED_DIR}",
                f"--grpc_python_out={GENERATED_DIR}",
                str(GENERATED_DIR / "compact_formats.proto"),
                str(GENERATED_DIR / "service.proto"),
            ]
        )
        if rc != 0:
            raise RuntimeError(f"lightwalletd proto generation failed with exit code {rc}")
    if str(GENERATED_DIR) not in sys.path:
        sys.path.insert(0, str(GENERATED_DIR))
    return importlib.import_module("service_pb2"), importlib.import_module("service_pb2_grpc")


def _hex(value: bytes) -> str:
    return bytes(value).hex()


def _compact_block_to_scanner_dict(block: Any) -> dict[str, Any]:
    scanner_block = {
        "protoVersion": int(getattr(block, "protoVersion", 0)),
        "height": int(block.height),
        "hash": _hex(block.hash),
        "prevHash": _hex(block.prevHash),
        "time": int(block.time),
        "vtx": [_compact_tx_to_scanner_dict(tx) for tx in getattr(block, "vtx", [])],
    }
    metadata = _chain_metadata_to_scanner_dict(getattr(block, "chainMetadata", None))
    if metadata is not None:
        scanner_block["chainMetadata"] = metadata
    return scanner_block


def _chain_metadata_to_scanner_dict(metadata: Any) -> dict[str, int] | None:
    if metadata is None:
        return None
    sapling_size = int(getattr(metadata, "saplingCommitmentTreeSize", 0))
    orchard_size = int(getattr(metadata, "orchardCommitmentTreeSize", 0))
    if sapling_size == 0 and orchard_size == 0:
        return None
    return {
        "saplingCommitmentTreeSize": sapling_size,
        "orchardCommitmentTreeSize": orchard_size,
    }


def _compact_tx_to_scanner_dict(tx: Any) -> dict[str, Any]:
    return {
        "index": int(tx.index),
        "txid": _hex(tx.hash),
        "fee": int(getattr(tx, "fee", 0)),
        "spends": [{"nf": _hex(spend.nf)} for spend in getattr(tx, "spends", [])],
        "outputs": [
            {
                "cmu": _hex(output.cmu),
                "ephemeralKey": _hex(getattr(output, "ephemeralKey", b"") or getattr(output, "epk", b"")),
                "ciphertext": _hex(output.ciphertext),
            }
            for output in getattr(tx, "outputs", [])
        ],
        "actions": [
            {
                "nf": _hex(getattr(action, "nullifier", b"") or getattr(action, "nf", b"")),
                "cmx": _hex(action.cmx),
                "ephemeralKey": _hex(action.ephemeralKey),
                "ciphertext": _hex(action.ciphertext),
            }
            for action in getattr(tx, "actions", [])
        ],
    }


class LightwalletdClient:
    def __init__(self, endpoint: str, *, timeout_seconds: float | None = None):
        if not endpoint or not endpoint.strip():
            raise ValueError("lightwalletd endpoint is required")
        self.endpoint = endpoint.strip()
        self.timeout_seconds = timeout_seconds or float(os.environ.get("CLEAN_WALLET_LIGHTWALLETD_TIMEOUT", "30"))

    def latest_height(self) -> int:
        service_pb2, service_pb2_grpc = _ensure_generated()
        import grpc  # type: ignore

        channel = self._channel(grpc)
        try:
            stub = service_pb2_grpc.CompactTxStreamerStub(channel)
            return int(stub.GetLatestBlock(service_pb2.ChainSpec(), timeout=self.timeout_seconds).height)
        finally:
            close = getattr(channel, "close", None)
            if close is not None:
                close()

    def _channel(self, grpc: Any) -> Any:
        parsed = urlparse(self.endpoint if "://" in self.endpoint else "https://" + self.endpoint)
        target = parsed.netloc or parsed.path
        if not target:
            raise ValueError("lightwalletd endpoint must include host[:port]")
        if parsed.scheme in {"http", "grpc", "plaintext", "insecure"}:
            return grpc.insecure_channel(target)
        if parsed.scheme in {"https", "grpcs", "tls"}:
            return grpc.secure_channel(target, grpc.ssl_channel_credentials())
        raise ValueError(f"unsupported lightwalletd endpoint scheme: {parsed.scheme}")

    def fetch_compact_blocks(self, *, start: int, end: int) -> list[dict[str, Any]]:
        if start < 0 or end < start:
            raise ValueError("lightwalletd block range must satisfy 0 <= start <= end")
        service_pb2, service_pb2_grpc = _ensure_generated()
        import grpc  # type: ignore

        channel = self._channel(grpc)
        stub = service_pb2_grpc.CompactTxStreamerStub(channel)
        request = service_pb2.BlockRange(
            start=service_pb2.BlockID(height=start),
            end=service_pb2.BlockID(height=end),
        )
        blocks = []
        try:
            for block in stub.GetBlockRange(request, timeout=self.timeout_seconds):
                blocks.append(_compact_block_to_scanner_dict(block))
        finally:
            close = getattr(channel, "close", None)
            if close is not None:
                close()
        expected = end - start + 1
        if len(blocks) != expected:
            raise RuntimeError(f"lightwalletd returned {len(blocks)} compact blocks; expected {expected}")
        heights = [block["height"] for block in blocks]
        if heights != list(range(start, end + 1)):
            raise RuntimeError("lightwalletd returned non-contiguous compact block heights")
        return blocks
