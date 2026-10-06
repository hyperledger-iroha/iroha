"""Shared helpers and constants of the SCCP Python reference (`specs/sccp.md` §0, §3.9)."""

from __future__ import annotations

import hashlib

# §3.9 constants (destination immutables and layout sizes).
MAX_VALIDITY_MS = 2_592_000_000
MAX_FAST_PAUSE_MS = 2_592_000_000
MAX_INIT_SLACK_MS = 3_600_000
MAX_CERTIFICATES_PER_CALL = 16
TON_CHECKPOINT_SLOTS = 3
MAX_VOID_FROZEN_RANGE_EVM = 256
MAX_VOID_FROZEN_RANGE_TON = 512
MAX_BLOCK_LEAVES = 512
MAX_BLOCK_PATH = 9
MAX_HISTORY_PATH = 32
MAX_HISTORY_SIZE = 1 << 32
SCCP_ANCHOR_CHUNK = 4096


def be(value: int, width: int) -> bytes:
    """Big-endian unsigned integer of exactly `width` bytes."""
    if value < 0 or value >= 1 << (8 * width):
        raise ValueError(f"{value} does not fit in {width} bytes")
    return value.to_bytes(width, "big")


def be32(value: int) -> bytes:
    return be(value, 4)


def be64(value: int) -> bytes:
    return be(value, 8)


def word(value: int) -> bytes:
    """The 32-byte ABI word of an unsigned integer (§0)."""
    return be(value, 32)


def sha256(*parts: bytes) -> bytes:
    return hashlib.sha256(b"".join(parts)).digest()


def h_iroha(*parts: bytes) -> bytes:
    """`iroha_crypto::Hash`: Blake2b-256 with the low bit of the last byte forced to 1."""
    digest = bytearray(hashlib.blake2b(b"".join(parts), digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


def rep(byte: int, count: int = 32) -> bytes:
    """`byte` repeated `count` times (the `0xNN^32` notation of the spec)."""
    return bytes([byte]) * count


def hx(data: bytes) -> str:
    """0x-prefixed lowercase hex, the fixture byte-string convention."""
    return "0x" + data.hex()


def unhex(text: str) -> bytes:
    if not text.startswith("0x"):
        raise ValueError(f"expected 0x-prefixed hex: {text!r}")
    return bytes.fromhex(text[2:])


class SccpError(Exception):
    """A rejection named after the §5.2.2 error table (or a layout invariant id)."""

    def __init__(self, name: str, detail: str = "", **args):
        super().__init__(f"{name}: {detail}" if detail else name)
        self.name = name
        self.detail = detail
        self.args_map = args
