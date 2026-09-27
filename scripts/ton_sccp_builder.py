#!/usr/bin/env python3
"""Pinned Acton 1.2.0 build, test and vector tooling for the SCCP v1 TON contracts.

Purpose
    Builds, formats, checks and tests `contracts/ton/sccp` (SccpTairaXorMinter,
    SccpTairaXorWallet, SccpConsumedBucket; specs/sccp.md §5.3, §5.5) with the
    native Acton 1.2.0 release that bundles Tolk 1.4.2. No Docker, Rosetta or
    container runtime is involved.

Toolchain
    The Acton release archive for the host (macOS arm64/x86-64, Linux
    arm64/x86-64) is downloaded from the official GitHub release once, verified
    against the SHA-256 pinned below, and unpacked into the ignored
    `contracts/ton/sccp/.toolchain/` cache. `--acton PATH` uses an existing
    executable instead; it must report the exact pinned version. `--offline`
    forbids downloads. No environment variable changes behaviour.

Commands
    toolchain   resolve (download and verify) Acton and print its path
    vectors     regenerate `tests/unit/sccp-test-vectors.gen.tolk` (`--check`
                only compares)
    fmt         check Tolk formatting of the SCCP sources (`--write` rewrites)
    build       compile the three contracts and print code hashes and depths
    wrappers    check that `wrappers/*.gen.tolk` match `acton wrapper --all`
                (`--write` regenerates them in place)
    test        run the Acton emulator suite `tests/unit`
    all         vectors --check, fmt, build, wrappers, test and the StateInit
                golden check (`generate_ton_sccp_stateinit_golden.py --check`)

The module also provides dependency-free Keccak-256, secp256k1, TON cell and
BoC primitives shared by `generate_ton_sccp_stateinit_golden.py` and the test
vector generator. Requires Python 3.9+ and nothing outside the standard
library.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

ROOT = Path(__file__).resolve().parents[1]
PROJECT = ROOT / "contracts" / "ton" / "sccp"
TOOLCHAIN_DIR = PROJECT / ".toolchain"
UNIT_TESTS = PROJECT / "tests" / "unit"
VECTORS_FILE = UNIT_TESTS / "sccp-test-vectors.gen.tolk"

ACTON_VERSION = "1.2.0"
ACTON_REPORTED_VERSION = "acton 1.2.0 (16d49e1 2026-09-16)"
TOLK_VERSION = "1.4.2"
ACTON_RELEASE_URL = "https://github.com/ton-blockchain/acton/releases/download/v1.2.0/"
# (system, machine) -> (archive name, archive SHA-256) from the release `sha256.sum`.
ACTON_ARCHIVES: Dict[Tuple[str, str], Tuple[str, str]] = {
    ("Darwin", "arm64"): (
        "acton-aarch64-apple-darwin.tar.gz",
        "cf87b7978d5e38687a3c1570aa2a737918b34777725600e361db39230c8a303f",
    ),
    ("Darwin", "x86_64"): (
        "acton-x86_64-apple-darwin.tar.gz",
        "8260cac4c8d4de7c43d4726d3849bb85235acf7c0c753c4f71b551f8da4764ec",
    ),
    ("Linux", "aarch64"): (
        "acton-aarch64-unknown-linux-gnu.tar.gz",
        "ae44c81b54996f74cbc80cb88c765f0e22341b9e83294c82f603309fad1983de",
    ),
    ("Linux", "arm64"): (
        "acton-aarch64-unknown-linux-gnu.tar.gz",
        "ae44c81b54996f74cbc80cb88c765f0e22341b9e83294c82f603309fad1983de",
    ),
    ("Linux", "x86_64"): (
        "acton-x86_64-unknown-linux-gnu.tar.gz",
        "c12c5ca622e1e41a26d7f7ebea024e4b8a716c65ee92ce7f9e220e7af0e08cd2",
    ),
}
MAX_ARCHIVE_BYTES = 256 * 1024 * 1024

CONTRACTS = ("SccpTairaXorMinter", "SccpTairaXorWallet", "SccpConsumedBucket")
SOURCE_FILES = (
    "contracts/SccpTairaXorMinter.tolk",
    "contracts/SccpTairaXorWallet.tolk",
    "contracts/SccpConsumedBucket.tolk",
    "contracts/sccp-addresses.tolk",
    "contracts/sccp-cells.tolk",
    "contracts/sccp-constants.tolk",
    "contracts/sccp-crypto.tolk",
    "contracts/sccp-errors.tolk",
    "contracts/sccp-fees.tolk",
    "contracts/sccp-messages.tolk",
    "contracts/sccp-storage.tolk",
    "contracts/sccp-verify.tolk",
    "scripts/stateinit-golden.tolk",
)
WRAPPER_FILES = tuple(f"wrappers/{name}.gen.tolk" for name in CONTRACTS)


class TonBuilderError(RuntimeError):
    """A bounded, user-facing failure."""


# ---------------------------------------------------------------------------
# Toolchain.


def host_archive() -> Tuple[str, str]:
    """Returns the pinned (archive name, SHA-256) for this host."""

    key = (platform.system(), platform.machine())
    if key not in ACTON_ARCHIVES:
        raise TonBuilderError(f"no pinned Acton {ACTON_VERSION} build for host {key[0]}/{key[1]}")
    return ACTON_ARCHIVES[key]


def sha256_file(path: Path) -> str:
    """Lower-hex SHA-256 of a file."""

    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def extract_acton(archive: bytes, destination: Path) -> None:
    """Extracts exactly the `acton` executable from a verified release archive."""

    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
        members = [m for m in tar.getmembers() if m.isfile()]
        names = sorted(m.name for m in members)
        if names not in (["./acton"], ["acton"]):
            raise TonBuilderError(f"unexpected Acton archive layout: {names}")
        source = tar.extractfile(members[0])
        if source is None:
            raise TonBuilderError("Acton archive member is unreadable")
        data = source.read()
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(".partial")
    temporary.write_bytes(data)
    temporary.chmod(0o755)
    os.replace(temporary, destination)


def acton_version(executable: Path) -> str:
    """The first line of `acton --version`."""

    completed = subprocess.run(
        [str(executable), "--version"], capture_output=True, text=True, timeout=60, check=False
    )
    if completed.returncode != 0:
        raise TonBuilderError(f"{executable} --version failed")
    return completed.stdout.strip().splitlines()[0] if completed.stdout.strip() else ""


def resolve_acton(explicit: Optional[str] = None, offline: bool = False) -> Path:
    """Returns a verified Acton executable, downloading the pinned archive if needed."""

    if explicit:
        path = Path(explicit)
        if not path.is_absolute() or not path.is_file() or not os.access(path, os.X_OK):
            raise TonBuilderError("--acton must name an absolute executable file")
        version = acton_version(path)
        if version != ACTON_REPORTED_VERSION:
            raise TonBuilderError(f"--acton reports {version!r}, expected {ACTON_REPORTED_VERSION!r}")
        return path

    archive_name, archive_sha = host_archive()
    install = TOOLCHAIN_DIR / f"acton-{ACTON_VERSION}-{archive_sha[:16]}"
    executable = install / "acton"
    stamp = install / "archive.sha256"
    if executable.is_file() and stamp.is_file() and stamp.read_text().strip() == archive_sha:
        version = acton_version(executable)
        if version == ACTON_REPORTED_VERSION:
            return executable
    if offline:
        raise TonBuilderError(f"Acton {ACTON_VERSION} is not cached and --offline forbids downloading")
    url = ACTON_RELEASE_URL + archive_name
    print(f"downloading {url}", file=sys.stderr)
    with urllib.request.urlopen(url, timeout=300) as response:  # noqa: S310 - pinned https URL
        archive = response.read(MAX_ARCHIVE_BYTES + 1)
    if len(archive) > MAX_ARCHIVE_BYTES:
        raise TonBuilderError("Acton archive exceeds the size bound")
    actual = hashlib.sha256(archive).hexdigest()
    if actual != archive_sha:
        raise TonBuilderError(f"Acton archive SHA-256 mismatch: {actual} != {archive_sha}")
    extract_acton(archive, executable)
    stamp.write_text(archive_sha + "\n")
    version = acton_version(executable)
    if version != ACTON_REPORTED_VERSION:
        raise TonBuilderError(f"downloaded Acton reports {version!r}")
    return executable


def run_acton(acton: Path, arguments: Sequence[str], cwd: Path = PROJECT) -> str:
    """Runs Acton in `cwd`; raises with the tail of its output on failure."""

    completed = subprocess.run(
        [str(acton), "--color", "never", *arguments],
        cwd=str(cwd),
        capture_output=True,
        text=True,
        timeout=3600,
        check=False,
    )
    output = completed.stdout + completed.stderr
    if completed.returncode != 0:
        tail = "\n".join(output.splitlines()[-80:])
        raise TonBuilderError(f"acton {' '.join(arguments)} failed:\n{tail}")
    return output


def check_tolk_stdlib(project: Path = PROJECT) -> None:
    """Checks that the project stdlib Acton installed is Tolk 1.4.2 / Acton 1.2.0."""

    version_file = project / ".acton" / ".version"
    common = project / ".acton" / "tolk-stdlib" / "common.tolk"
    if not version_file.is_file() or version_file.read_text().strip() != ACTON_VERSION:
        raise TonBuilderError(".acton/.version does not name Acton 1.2.0")
    header = [line.strip() for line in common.read_text(encoding="utf-8").splitlines()[:8]]
    if f"tolk {TOLK_VERSION}" not in header:
        raise TonBuilderError(f"Tolk stdlib header does not declare `tolk {TOLK_VERSION}`")


# ---------------------------------------------------------------------------
# Keccak-256 (Ethereum, not SHA3).

_KECCAK_RC = (
    0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
    0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
    0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
)
_KECCAK_ROT = (
    (0, 36, 3, 41, 18), (1, 44, 10, 45, 2), (62, 6, 43, 15, 61), (28, 55, 25, 21, 56),
    (27, 20, 39, 8, 14),
)
_M64 = (1 << 64) - 1


def _rol(value: int, shift: int) -> int:
    return ((value << shift) | (value >> (64 - shift))) & _M64 if shift else value


def _keccak_f(state: List[List[int]]) -> List[List[int]]:
    for rc in _KECCAK_RC:
        c = [state[x][0] ^ state[x][1] ^ state[x][2] ^ state[x][3] ^ state[x][4] for x in range(5)]
        d = [c[(x - 1) % 5] ^ _rol(c[(x + 1) % 5], 1) for x in range(5)]
        state = [[state[x][y] ^ d[x] for y in range(5)] for x in range(5)]
        b = [[0] * 5 for _ in range(5)]
        for x in range(5):
            for y in range(5):
                b[y][(2 * x + 3 * y) % 5] = _rol(state[x][y], _KECCAK_ROT[x][y])
        state = [
            [b[x][y] ^ ((~b[(x + 1) % 5][y]) & b[(x + 2) % 5][y]) for y in range(5)]
            for x in range(5)
        ]
        state[0][0] ^= rc
    return state


def keccak256(data: bytes) -> bytes:
    """Ethereum Keccak-256."""

    rate = 136
    message = bytearray(data)
    message.append(0x01)
    while len(message) % rate:
        message.append(0)
    message[-1] |= 0x80
    state = [[0] * 5 for _ in range(5)]
    for offset in range(0, len(message), rate):
        block = message[offset:offset + rate]
        for i in range(rate // 8):
            state[i % 5][i // 5] ^= int.from_bytes(block[8 * i:8 * i + 8], "little")
        state = _keccak_f(state)
    return b"".join(state[i % 5][i // 5].to_bytes(8, "little") for i in range(4))


def keccak_int(data: bytes) -> int:
    """Keccak-256 as a big-endian integer."""

    return int.from_bytes(keccak256(data), "big")


def u(value: int, width: int) -> bytes:
    """Big-endian unsigned integer of `width` bytes."""

    return value.to_bytes(width, "big")


# ---------------------------------------------------------------------------
# secp256k1.

SECP_P = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
SECP_N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
SECP_HALF_N = SECP_N // 2
SECP_G = (
    0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
    0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8,
)
Point = Optional[Tuple[int, int]]


def _point_add(p: Point, q: Point) -> Point:
    if p is None:
        return q
    if q is None:
        return p
    if p[0] == q[0] and (p[1] + q[1]) % SECP_P == 0:
        return None
    if p == q:
        lam = 3 * p[0] * p[0] * pow(2 * p[1], SECP_P - 2, SECP_P) % SECP_P
    else:
        lam = (q[1] - p[1]) * pow(q[0] - p[0], SECP_P - 2, SECP_P) % SECP_P
    x = (lam * lam - p[0] - q[0]) % SECP_P
    return (x, (lam * (p[0] - x) - p[1]) % SECP_P)


def point_mul(k: int, p: Point = SECP_G) -> Point:
    """Scalar multiplication."""

    result: Point = None
    addend = p
    while k:
        if k & 1:
            result = _point_add(result, addend)
        addend = _point_add(addend, addend)
        k >>= 1
    return result


def eth_address(point: Tuple[int, int]) -> int:
    """`keccak256(X ‖ Y)[12..32]` as an integer (§3.8)."""

    return keccak_int(u(point[0], 32) + u(point[1], 32)) & ((1 << 160) - 1)


def ecdsa_sign(private: int, digest: int, nonce: int) -> Tuple[int, int, int]:
    """Low-S ECDSA over a 32-byte digest with an explicit nonce; returns (r, s, v)."""

    r_point = point_mul(nonce)
    assert r_point is not None
    r = r_point[0] % SECP_N
    if r == 0 or r_point[0] >= SECP_N:
        raise TonBuilderError("nonce yields an unusable r")
    s = pow(nonce, SECP_N - 2, SECP_N) * (digest + r * private) % SECP_N
    recid = r_point[1] & 1
    if s > SECP_HALF_N:
        s = SECP_N - s
        recid ^= 1
    return r, s, 27 + recid


def ecdsa_recover(digest: int, r: int, s: int, v: int) -> int:
    """Recovers the signer address of a §3.8 signature."""

    x = r
    alpha = (pow(x, 3, SECP_P) + 7) % SECP_P
    beta = pow(alpha, (SECP_P + 1) // 4, SECP_P)
    y = beta if (beta & 1) == (v - 27) else SECP_P - beta
    r_inv = pow(r, SECP_N - 2, SECP_N)
    sr = point_mul(s, (x, y))
    zg = point_mul((SECP_N - digest % SECP_N) % SECP_N)
    public = point_mul(r_inv, _point_add(sr, zg))
    assert public is not None
    return eth_address(public)


# ---------------------------------------------------------------------------
# TON cells and bags of cells.


class Cell:
    """An ordinary (level-0) TON cell."""

    def __init__(self, bits: str, refs: Sequence["Cell"] = ()) -> None:
        if len(bits) > 1023 or len(refs) > 4 or set(bits) - {"0", "1"}:
            raise TonBuilderError("invalid cell shape")
        self.bits = bits
        self.refs = list(refs)
        self._hash: Optional[bytes] = None
        self._depth: Optional[int] = None

    def data_bytes(self) -> bytes:
        """Augmented data (completion tag when not byte-aligned)."""

        bits = self.bits
        if len(bits) % 8:
            bits += "1" + "0" * (7 - len(bits) % 8)
        return int(bits, 2).to_bytes(len(bits) // 8, "big") if bits else b""

    def descriptors(self) -> bytes:
        return bytes([len(self.refs), (len(self.bits) + 7) // 8 + len(self.bits) // 8])

    @property
    def depth(self) -> int:
        if self._depth is None:
            self._depth = 0 if not self.refs else 1 + max(ref.depth for ref in self.refs)
        return self._depth

    @property
    def hash(self) -> bytes:
        if self._hash is None:
            material = self.descriptors() + self.data_bytes()
            material += b"".join(u(ref.depth, 2) for ref in self.refs)
            material += b"".join(ref.hash for ref in self.refs)
            self._hash = hashlib.sha256(material).digest()
        return self._hash

    def data_hex(self) -> str:
        """Hex of the data bits, zero-padded on the right to whole bytes."""

        bits = self.bits + "0" * (-len(self.bits) % 8)
        return int(bits, 2).to_bytes(len(bits) // 8, "big").hex() if bits else ""


class Builder:
    """Minimal cell builder mirroring TVM serialization."""

    def __init__(self) -> None:
        self.bits = ""
        self.refs: List[Cell] = []

    def uint(self, value: int, width: int) -> "Builder":
        if value < 0 or value >= 1 << width:
            raise TonBuilderError(f"{value} does not fit uint{width}")
        if width:
            self.bits += format(value, f"0{width}b")
        return self

    def int(self, value: int, width: int) -> "Builder":
        return self.uint(value % (1 << width), width)

    def bit(self, value: bool) -> "Builder":
        self.bits += "1" if value else "0"
        return self

    def coins(self, value: int) -> "Builder":
        length = (value.bit_length() + 7) // 8
        if length > 15:
            raise TonBuilderError("coins overflow")
        return self.uint(length, 4).uint(value, 8 * length)

    def address(self, workchain: int, account: int) -> "Builder":
        return self.uint(0b100, 3).int(workchain, 8).uint(account, 256)

    def ref(self, cell: Cell) -> "Builder":
        self.refs.append(cell)
        return self

    def maybe_ref(self, cell: Optional[Cell]) -> "Builder":
        self.bit(cell is not None)
        if cell is not None:
            self.refs.append(cell)
        return self

    def end(self) -> Cell:
        return Cell(self.bits, self.refs)


def parse_boc(data: bytes) -> Cell:
    """Parses a single-root bag of ordinary cells (`serialized_boc#b5ee9c72`)."""

    if data[:4] != bytes.fromhex("b5ee9c72"):
        raise TonBuilderError("not a serialized_boc")
    flags = data[4]
    has_index = bool(flags & 0x80)
    has_crc = bool(flags & 0x40)
    size = flags & 0x07
    offset_bytes = data[5]
    position = 6

    def read(width: int) -> int:
        nonlocal position
        value = int.from_bytes(data[position:position + width], "big")
        position += width
        return value

    cell_count = read(size)
    root_count = read(size)
    read(size)  # absent
    total_size = read(offset_bytes)
    roots = [read(size) for _ in range(root_count)]
    if root_count != 1:
        raise TonBuilderError("expected one BoC root")
    if has_index:
        position += cell_count * offset_bytes
    raw: List[Tuple[str, List[int]]] = []
    end = position + total_size
    while position < end:
        d1 = data[position]
        d2 = data[position + 1]
        position += 2
        ref_count = d1 & 7
        if d1 & 8 or d1 >> 5:
            raise TonBuilderError("exotic or leveled cells are not supported")
        byte_len = (d2 + 1) // 2
        payload = data[position:position + byte_len]
        position += byte_len
        bits = "".join(format(byte, "08b") for byte in payload)
        if d2 % 2:
            bits = bits.rstrip("0")[:-1]
        refs = [read(size) for _ in range(ref_count)]
        raw.append((bits, refs))
    if has_crc:
        position += 4
    if len(raw) != cell_count or position != len(data):
        raise TonBuilderError("malformed BoC")
    cells: List[Optional[Cell]] = [None] * cell_count
    for index in range(cell_count - 1, -1, -1):
        bits, refs = raw[index]
        cells[index] = Cell(bits, [cells[ref] for ref in refs])  # type: ignore[misc]
    root = cells[roots[0]]
    assert root is not None
    return root


def cell_tree_size(root: Cell) -> Tuple[int, int]:
    """(unique cells, total data bits) of a cell tree."""

    seen: Dict[bytes, Cell] = {}
    stack = [root]
    while stack:
        cell = stack.pop()
        if cell.hash in seen:
            continue
        seen[cell.hash] = cell
        stack.extend(cell.refs)
    return len(seen), sum(len(cell.bits) for cell in seen.values())


def load_compiled_code(name: str, project: Path = PROJECT) -> Cell:
    """Code cell of a built contract; checks the hash Acton recorded."""

    record = json.loads((project / "build" / f"{name}.json").read_text(encoding="utf-8"))
    import base64

    code = parse_boc(base64.b64decode(record["code_boc64"]))
    if code.hash.hex() != record["hash"].lower():
        raise TonBuilderError(f"{name}: BoC hash does not match Acton's recorded hash")
    return code


# ---------------------------------------------------------------------------
# SCCP structures (§3.7, §5.3.1).

TAG_ROSTER = b"SCCP/ROSTER/V1"
MEMBERS_PER_CHUNK = 6


def roster_threshold(n: int) -> int:
    """`⌊2n/3⌋ + 1`."""

    return 2 * n // 3 + 1


def roster_digest(
    network_id: int, generation: int, valid_from_ms: int, valid_until_ms: int, members: Sequence[int]
) -> int:
    """§3.7 roster digest (validates n, order and validity)."""

    n = len(members)
    if not 4 <= n <= 31 or valid_until_ms <= valid_from_ms or generation < 1:
        raise TonBuilderError("invalid roster")
    previous = 0
    for member in members:
        if member == 0:
            if previous:
                raise TonBuilderError("zero member after a nonzero member")
        else:
            if member <= previous:
                raise TonBuilderError("members not strictly ascending")
            previous = member
    preimage = (
        TAG_ROSTER + u(network_id, 32) + u(generation, 8) + u(valid_from_ms, 8)
        + u(valid_until_ms, 8) + u(n, 1) + u(roster_threshold(n), 1)
        + b"".join(u(member, 20) for member in members)
    )
    return keccak_int(preimage)


def member_chunks(members: Sequence[int]) -> Cell:
    """Canonical `^MemberChunk` list (6 per chunk except the last)."""

    tail: Optional[Cell] = None
    starts = list(range(0, len(members), MEMBERS_PER_CHUNK))
    for start in reversed(starts):
        builder = Builder()
        for member in members[start:start + MEMBERS_PER_CHUNK]:
            builder.uint(member, 160)
        tail = builder.maybe_ref(tail).end()
    assert tail is not None
    return tail


def minter_initial_data(
    network_id: int,
    route_revision: int,
    max_supply: int,
    generation: int,
    valid_from_ms: int,
    valid_until_ms: int,
    members: Sequence[int],
    wallet_code: Cell,
    bucket_code: Cell,
) -> Dict[str, Cell]:
    """Canonical initial minter data of §5.3.1; returns the named cells."""

    digest = roster_digest(network_id, generation, valid_from_ms, valid_until_ms, members)
    config = (
        Builder().uint(network_id, 256).uint(route_revision, 32).coins(max_supply)
        .uint(generation, 64).uint(digest, 256).ref(wallet_code).ref(bucket_code).end()
    )
    members_cell = member_chunks(members)
    roster = (
        Builder().uint(digest, 256).uint(generation, 64).uint(valid_from_ms, 64)
        .uint(valid_until_ms, 64).uint(len(members), 8).uint(roster_threshold(len(members)), 8)
        .ref(members_cell).end()
    )
    root = (
        Builder().bit(False).coins(0).coins(0).uint(0, 64).uint(0, 64).uint(0, 64).bit(False)
        .uint(0, 64).ref(config).ref(roster).maybe_ref(None).end()
    )
    return {"root": root, "config": config, "roster": roster, "members": members_cell}


def state_init(code: Cell, data: Cell) -> Cell:
    """`StateInit` with only code and data (bits `00110`)."""

    return Builder().uint(0b00110, 5).ref(code).ref(data).end()


# ---------------------------------------------------------------------------
# Tolk test vectors.

TEST_KEY_COUNT = 31
VECTOR_NETWORK_ID = int("11" * 32, 16)


def test_keys() -> List[Dict[str, int]]:
    """31 deterministic test keys sorted by address, each with a fixed ECDSA nonce."""

    keys = []
    for index in range(TEST_KEY_COUNT):
        private = int.from_bytes(hashlib.sha256(b"sccp-ton-test-key-%d" % index).digest(), "big") % SECP_N
        counter = 0
        while True:
            nonce = int.from_bytes(
                hashlib.sha256(b"sccp-ton-test-nonce-%d-%d" % (index, counter)).digest(), "big"
            ) % SECP_N
            r_point = point_mul(nonce)
            assert r_point is not None
            if 0 < r_point[0] < SECP_N:
                break
            counter += 1
        public = point_mul(private)
        assert public is not None
        keys.append(
            {
                "private": private,
                "address": eth_address(public),
                "r": r_point[0] % SECP_N,
                "k_inverse": pow(nonce, SECP_N - 2, SECP_N),
                "parity": r_point[1] & 1,
                "nonce": nonce,
            }
        )
    keys.sort(key=lambda key: key["address"])
    return keys


def _snake_pieces(data: bytes) -> List[str]:
    return [data[i:i + 127].hex() for i in range(0, len(data), 127)]


def inbound_payload(
    nonce: int, revision: int, deadline_ms: int, amount: int, sender: bytes, recipient_hash: int
) -> bytes:
    """Taira→TON §3.2 payload."""

    return (
        u(2, 1) + u(1, 1) + u(0, 4) + u(4, 4) + u(nonce, 8) + u(revision, 4) + u(deadline_ms, 8)
        + u(0, 4) + u(1, 1) + u(3, 2) + b"xor" + u(amount, 16)
        + u(3, 1) + u(len(sender), 2) + sender
        + u(7, 1) + u(36, 2) + u(0, 4) + u(recipient_hash, 32)
        + u(1, 1) + u(13, 2) + b"taira_ton_xor"
    )


def outbound_payload(nonce: int, revision: int, amount: int, owner_hash: int, recipient: bytes) -> bytes:
    """TON→Taira §3.2 payload of a burn."""

    return (
        u(2, 1) + u(1, 1) + u(4, 4) + u(0, 4) + u(nonce, 8) + u(revision, 4) + u(0, 8)
        + u(0, 4) + u(1, 1) + u(3, 2) + b"xor" + u(amount, 16)
        + u(7, 1) + u(36, 2) + u(0, 4) + u(owner_hash, 32)
        + u(3, 1) + u(len(recipient), 2) + recipient
        + u(1, 1) + u(13, 2) + b"taira_ton_xor"
    )


TON_WORD = (1 << 256) - 239


def lane(source_tag: int, source_word: int, target_tag: int, target_word: int) -> bytes:
    """`lane_bytes(source, target)`."""

    return u(source_tag, 1) + u(source_word, 32) + u(target_tag, 1) + u(target_word, 32)


def control_leaf(network_id: int, target_tag: int, target_word: int, destination: int,
                 revision: int, nonce: int, paused: bool) -> int:
    """§3.4 control leaf."""

    return keccak_int(
        b"SCCP/CONTROL/V1" + lane(0x40, network_id, target_tag, target_word) + u(destination, 32)
        + u(revision, 4) + u(nonce, 8) + u(1 if paused else 0, 1)
    )


def merkle_node(left: int, right: int) -> int:
    return keccak_int(b"SCCP/NODE/V1" + u(left, 32) + u(right, 32))


def merkle_levels(leaves: Sequence[int]) -> List[List[int]]:
    """Promote-odd levels (§3.4)."""

    levels = [list(leaves)]
    while len(levels[-1]) > 1:
        level = levels[-1]
        nxt = [merkle_node(level[i], level[i + 1]) for i in range(0, len(level) - 1, 2)]
        if len(level) % 2:
            nxt.append(level[-1])
        levels.append(nxt)
    return levels


def merkle_path(leaves: Sequence[int], index: int) -> List[int]:
    """Sibling path of `index` (promoted levels contribute nothing)."""

    path = []
    for level in merkle_levels(leaves)[:-1]:
        sibling = index ^ 1
        if sibling < len(level):
            path.append(level[sibling])
        index >>= 1
    return path


def eip712_domain(network_id: int) -> int:
    type_hash = keccak_int(b"EIP712Domain(string name,string version,bytes32 salt)")
    return keccak_int(u(type_hash, 32) + keccak256(b"SCCP") + keccak256(b"1") + u(network_id, 32))


ATTESTATION_TYPE = (
    b"SccpAttestation(uint64 height,uint64 epoch,uint64 timestampMs,bytes32 blockHash,"
    b"bytes32 sccpRoot,uint32 messageCount,bytes32 historyRoot,uint64 historySize,"
    b"bytes32 rosterDigest,bytes32 nextRosterDigest)"
)


def attestation_digest(network_id: int, fields: Sequence[int]) -> int:
    """EIP-712 digest of an attestation given its 10 fields in type order."""

    struct_hash = keccak_int(keccak256(ATTESTATION_TYPE) + b"".join(u(f, 32) for f in fields))
    return keccak_int(b"\x19\x01" + u(eip712_domain(network_id), 32) + u(struct_hash, 32))


def _self_check_spec_constants() -> None:
    """Pins the Python primitives against the constants printed in the spec."""

    expected = {
        keccak_int(b"EIP712Domain(string name,string version,bytes32 salt)"):
            0x599a80fcaa47b95e2323ab4d34d34e0cc9feda4b843edafcc30c7bdf60ea15bf,
        keccak_int(b"SCCP"): 0xd7bacbdfe367013f66397ff7242325c31126ab14c95620817c994f22f1d123ba,
        keccak_int(b"1"): 0xc89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc6,
        keccak_int(ATTESTATION_TYPE): 0x6ea54d1f320a2e5892d6a362adc4b569967facc991d3a327cbc84b746f520c66,
        keccak_int(b"SccpControlApplied(uint64,bool)"):
            0x38b98e72aaf30cffd4309ad36378e1587525135a709fe14074156ebb44254d65,
        control_leaf(VECTOR_NETWORK_ID, 0x41, 1, int("22" * 20, 16), 1, 1, True):
            0x93d641053e51b4d28f40930e098212e8b6ab340ceaaf8ad38a458203ce98c662,
        control_leaf(VECTOR_NETWORK_ID, 0x41, 1, int("22" * 20, 16), 1, 2, False):
            0x85fcfc8718c845c212df8ae486161d03bde8116eaa7dd74bbb2312667bce5cdf,
    }
    for actual, wanted in expected.items():
        if actual != wanted:
            raise TonBuilderError(f"spec constant mismatch: {actual:#x} != {wanted:#x}")


def _tolk_int_array(name: str, values: Iterable[int], doc: str) -> str:
    body = ",\n".join(f"        {value:#x}" for value in values)
    return f"/// {doc}\nfun {name}(): array<int> {{\n    return [\n{body},\n    ];\n}}\n"


def _tolk_str_array(name: str, values: Iterable[str], doc: str) -> str:
    body = ",\n".join(f'        "{value}".hexToSlice()' for value in values)
    return f"/// {doc}\nfun {name}(): array<slice> {{\n    return [\n{body},\n    ];\n}}\n"


def render_test_vectors() -> str:
    """Tolk source with the test keys and independent golden vectors."""

    _self_check_spec_constants()
    keys = test_keys()
    network = VECTOR_NETWORK_ID
    parts: List[str] = [
        "// Generated by `python3 scripts/ton_sccp_builder.py vectors`; do not edit.\n"
        "//\n"
        "// Deterministic secp256k1 test keys (sorted by address) with one fixed ECDSA\n"
        "// nonce per key, so tests sign any digest with two modular products, and\n"
        "// golden vectors computed by the independent Python implementation of\n"
        "// specs/sccp.md §3 in `scripts/ton_sccp_builder.py`.\n\n",
        f"const VEC_KEY_COUNT = {TEST_KEY_COUNT}\n",
        f"const VEC_NETWORK_ID = {network:#x}\n\n",
        _tolk_int_array("vecKeyPrivates", (k["private"] for k in keys), "Private keys."),
        _tolk_int_array("vecKeyAddresses", (k["address"] for k in keys), "Addresses (ascending)."),
        _tolk_int_array("vecKeyR", (k["r"] for k in keys), "`r` of the fixed nonce of each key."),
        _tolk_int_array("vecKeyKInverse", (k["k_inverse"] for k in keys), "`k⁻¹ mod N` of each fixed nonce."),
        _tolk_int_array("vecKeyParity", (k["parity"] for k in keys), "Parity of `R.y` of each fixed nonce."),
    ]
    # EIP-712.
    fields = [
        1234, 56, 1_700_000_000_000, int("aa" * 32, 16), int("bb" * 32, 16), 3,
        int("cc" * 32, 16), 9, int("dd" * 32, 16), 0,
    ]
    parts.append(f"const VEC_DOMAIN_SEPARATOR = {eip712_domain(network):#x}\n")
    parts.append(_tolk_int_array("vecAttestationFields", fields, "height..nextRosterDigest in type order."))
    parts.append(f"const VEC_ATTESTATION_DIGEST = {attestation_digest(network, fields):#x}\n\n")
    # Rosters.
    addresses = [k["address"] for k in keys]
    roster4 = addresses[:4]
    roster7 = [0, 0] + addresses[:5]
    parts.append(_tolk_int_array("vecRoster4Members", roster4, "n = 4 roster members."))
    parts.append(
        f"const VEC_ROSTER4_DIGEST = {roster_digest(network, 7, 1_700_000_000_000, 1_701_209_600_000, roster4):#x}\n"
    )
    parts.append(_tolk_int_array("vecRoster7Members", roster7, "n = 7 roster with two keyless slots."))
    parts.append(
        f"const VEC_ROSTER7_DIGEST = {roster_digest(network, 8, 1_700_000_000_000, 1_701_209_600_000, roster7):#x}\n"
    )
    parts.append(
        f"const VEC_ROSTER31_DIGEST = {roster_digest(network, 9, 1_700_000_000_000, 1_701_209_600_000, [0, 0, 0] + addresses[:28]):#x}\n\n"
    )
    # Control leaves (TON target, destination word 0x22…22, revision 1).
    destination = int("22" * 32, 16)
    parts.append(f"const VEC_CONTROL_LEAF_1 = {control_leaf(network, 0x44, TON_WORD, destination, 1, 1, True):#x}\n")
    parts.append(f"const VEC_CONTROL_LEAF_2 = {control_leaf(network, 0x44, TON_WORD, destination, 1, 2, False):#x}\n\n")
    # Inbound payload, message id and transfer leaf.
    sender = bytes(range(1, 36))
    inbound = inbound_payload(77, 1, 1_700_000_600_000, 123_456_789_000, sender, int("33" * 32, 16))
    inbound_hash = keccak_int(b"SCCP/PAYLOAD/V1" + inbound)
    inbound_id = keccak_int(b"SCCP/MESSAGE/V1" + lane(0x40, network, 0x44, TON_WORD) + u(inbound_hash, 32))
    parts.append(_tolk_str_array("vecInboundPayloadPieces", _snake_pieces(inbound), "Inbound payload bytes in 127-byte pieces."))
    parts.append(f"const VEC_INBOUND_PAYLOAD_HASH = {inbound_hash:#x}\n")
    parts.append(f"const VEC_INBOUND_MESSAGE_ID = {inbound_id:#x}\n")
    parts.append(
        f"const VEC_INBOUND_LEAF = {keccak_int(b'SCCP/LEAF/V1' + u(inbound_id, 32) + u(destination, 32)):#x}\n\n"
    )
    # Outbound payload of a burn.
    recipient = bytes(range(200, 240))
    outbound = outbound_payload(5, 1, 987_654_321, int("44" * 32, 16), recipient)
    outbound_hash = keccak_int(b"SCCP/PAYLOAD/V1" + outbound)
    outbound_id = keccak_int(b"SCCP/MESSAGE/V1" + lane(0x44, TON_WORD, 0x40, network) + u(outbound_hash, 32))
    parts.append(_tolk_str_array("vecOutboundRecipientPieces", _snake_pieces(recipient), "Burn recipient bytes."))
    parts.append(_tolk_str_array("vecOutboundPayloadPieces", _snake_pieces(outbound), "Outbound payload bytes."))
    parts.append(f"const VEC_OUTBOUND_PAYLOAD_HASH = {outbound_hash:#x}\n")
    parts.append(f"const VEC_OUTBOUND_MESSAGE_ID = {outbound_id:#x}\n\n")
    # Merkle tree of 5 leaves and the history leaf.
    leaves = [keccak_int(bytes([i])) for i in range(5)]
    parts.append(_tolk_int_array("vecMerkleLeaves", leaves, "Five leaves keccak256(i)."))
    parts.append(f"const VEC_MERKLE_ROOT5 = {merkle_levels(leaves)[-1][0]:#x}\n")
    parts.append(_tolk_int_array("vecMerklePath5Index3", merkle_path(leaves, 3), "Path of leaf 3."))
    parts.append(_tolk_int_array("vecMerklePath5Index4", merkle_path(leaves, 4), "Path of leaf 4 (promoted twice)."))
    history = keccak_int(b"SCCP/HISTORY/V1" + u(42, 8) + u(int("ee" * 32, 16), 32) + u(3, 4))
    parts.append(f"const VEC_HISTORY_LEAF = {history:#x}\n\n")
    # An independently signed ECDSA vector (nonce different from the fixed one).
    digest = keccak_int(b"sccp-ton-ecrecover-vector")
    r, s, v = ecdsa_sign(keys[0]["private"], digest, 0x1234567890ABCDEF)
    if ecdsa_recover(digest, r, s, v) != keys[0]["address"]:
        raise TonBuilderError("ECDSA self-check failed")
    parts.append(f"const VEC_ECDSA_DIGEST = {digest:#x}\n")
    parts.append(f"const VEC_ECDSA_R = {r:#x}\nconst VEC_ECDSA_S = {s:#x}\nconst VEC_ECDSA_V = {v}\n")
    return "".join(parts)


# ---------------------------------------------------------------------------
# Commands.


def command_vectors(check: bool) -> None:
    rendered = render_test_vectors()
    current = VECTORS_FILE.read_text(encoding="utf-8") if VECTORS_FILE.is_file() else None
    if check:
        if current != rendered:
            raise TonBuilderError(f"{VECTORS_FILE.relative_to(ROOT)} is stale; run `vectors`")
        print("test vectors are current")
        return
    VECTORS_FILE.parent.mkdir(parents=True, exist_ok=True)
    VECTORS_FILE.write_text(rendered, encoding="utf-8")
    print(f"wrote {VECTORS_FILE.relative_to(ROOT)}")


def format_targets() -> List[str]:
    """SCCP-owned Tolk sources (the retired files are excluded)."""

    targets = [path for path in SOURCE_FILES if (PROJECT / path).is_file()]
    targets += sorted(str(p.relative_to(PROJECT)) for p in UNIT_TESTS.glob("*.tolk") if not p.name.endswith(".gen.tolk"))
    return targets


def command_fmt(acton: Path, write: bool) -> None:
    targets = format_targets()
    run_acton(acton, ["fmt", *targets] if write else ["fmt", "--check", *targets])
    print("Tolk formatting is canonical" if not write else "formatted Tolk sources")


def command_build(acton: Path) -> Dict[str, Tuple[str, int]]:
    run_acton(acton, ["build"])
    check_tolk_stdlib()
    result = {}
    for name in CONTRACTS:
        code = load_compiled_code(name)
        cells, bits = cell_tree_size(code)
        result[name] = (code.hash.hex(), code.depth)
        print(f"{name}: code hash {code.hash.hex()} depth {code.depth} cells {cells} bits {bits}")
    return result


def _copy_project(destination: Path) -> None:
    for relative in ("Acton.toml", "contracts", "wrappers", "tests", "scripts"):
        source = PROJECT / relative
        if source.is_dir():
            shutil.copytree(source, destination / relative)
        elif source.is_file():
            shutil.copy2(source, destination / relative)


def command_wrappers(acton: Path, write: bool) -> None:
    if write:
        run_acton(acton, ["wrapper", "--all"])
        print("regenerated wrappers")
        return
    with tempfile.TemporaryDirectory(prefix="sccp-ton-wrappers-") as scratch:
        copy = Path(scratch) / "project"
        copy.mkdir()
        _copy_project(copy)
        run_acton(acton, ["wrapper", "--all"], cwd=copy)
        for relative in WRAPPER_FILES:
            if (copy / relative).read_bytes() != (PROJECT / relative).read_bytes():
                raise TonBuilderError(f"{relative} is stale; run `wrappers --write`")
    print("wrappers are current")


def command_test(acton: Path, extra: Sequence[str]) -> str:
    output = run_acton(acton, ["test", "tests/unit", *extra])
    print("\n".join(line for line in output.splitlines() if line.strip()))
    return output


def command_all(acton: Path) -> None:
    command_vectors(check=True)
    command_fmt(acton, write=False)
    command_build(acton)
    command_wrappers(acton, write=False)
    command_test(acton, [])
    sys.path.insert(0, str(ROOT / "scripts"))
    import generate_ton_sccp_stateinit_golden as golden  # noqa: E402

    golden.run(acton, check=True, build=False)
    print("SCCP TON contracts: build, wrappers, tests and StateInit golden all pass")


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--acton", help="absolute Acton executable reporting exactly " + ACTON_REPORTED_VERSION)
    parser.add_argument("--offline", action="store_true", help="never download the pinned Acton archive")
    sub = parser.add_subparsers(dest="command")
    sub.add_parser("toolchain", help="resolve and print the pinned Acton executable")
    vectors = sub.add_parser("vectors", help="regenerate the Tolk test vectors")
    vectors.add_argument("--check", action="store_true")
    fmt = sub.add_parser("fmt", help="check (or --write) Tolk formatting")
    fmt.add_argument("--write", action="store_true")
    sub.add_parser("build", help="compile the contracts")
    wrappers = sub.add_parser("wrappers", help="check (or --write) generated wrappers")
    wrappers.add_argument("--write", action="store_true")
    sub.add_parser("test", help="run the Acton emulator suite (extra arguments go to `acton test`)")
    sub.add_parser("all", help="run every check (default)")
    return parser


def main(arguments: Optional[Sequence[str]] = None) -> int:
    """CLI entry point."""

    parser = _parser()
    parsed, extra = parser.parse_known_args(arguments)
    command = parsed.command or "all"
    if extra and command != "test":
        parser.error(f"unrecognized arguments for {command}")
    try:
        if command == "vectors":
            command_vectors(parsed.check)
            return 0
        acton = resolve_acton(parsed.acton, parsed.offline)
        if command == "toolchain":
            print(acton)
        elif command == "fmt":
            command_fmt(acton, parsed.write)
        elif command == "build":
            command_build(acton)
        elif command == "wrappers":
            command_wrappers(acton, parsed.write)
        elif command == "test":
            command_test(acton, extra)
        else:
            command_all(acton)
        return 0
    except TonBuilderError as error:
        print(f"TON SCCP builder failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
