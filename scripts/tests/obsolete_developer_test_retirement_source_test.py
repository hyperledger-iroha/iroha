#!/usr/bin/env python3
"""Fail closed on retired obsolete developer and shadow-test units.

The stdlib-only guard authenticates deleted Rust preimages through Git objects,
derives the exact declaration postimages, pins surviving Cargo-equivalent
target/module ledgers, and seals unchanged replacements and Cargo.lock.
Mutations stay in memory.
"""

from __future__ import annotations

import hashlib
import json
import re
import stat
import subprocess
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OPENING_COMMIT = "13768a6bed26d978ed159340f3d0f2536e3b626f"
LOCKFILE = "Cargo.lock"
IVM_MANIFEST = "crates/ivm/Cargo.toml"
WORKFLOW = ".github/workflows/pr.yml"
GUARD_PATH = "scripts/tests/obsolete_developer_test_retirement_source_test.py"
GUARD_MODULE = "scripts.tests.obsolete_developer_test_retirement_source_test"


@dataclass(frozen=True)
class SourcePin:
    path: str
    blob: str
    sha256: str
    byte_count: int
    line_count: int
    function_items: int
    test_names: tuple[str, ...]
    test_ledger_sha256: str


SOURCE_PINS = (
    SourcePin(
        "crates/ivm/tests/streaming_access_contract.rs",
        "274d80e20f030dbe1e3fe0e70c0bf1ba55aac41a",
        "fb77b83d93d66efe5c2b5a6332b48858007cfe0747abcb39a33fd9315ec46f85",
        13_884,
        429,
        11,
        (
            "mint_debit_exhaustes_ticket",
            "refund_shortfall_transitions_ticket",
            "expire_transitions_ticket_to_expired_state",
        ),
        "6b30040f4904699ebc3c5b00abd53183dec03edc6e4973897fbeec5c3c6b1e98",
    ),
    SourcePin(
        "crates/norito/benches/adaptive_telemetry.rs",
        "8380900e2036d49ebff863112d64585c325f8600",
        "3689189be63f4cf0c7bd3592f579f2a3376bd053cce6a9a56255eb3865a509ae",
        5_408,
        129,
        1,
        (),
        "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
    ),
)
RETIRED_RUST_LINES = 558
RETIRED_FUNCTION_ITEMS = 12
RETIRED_TESTS = 3
RETIRED_STANDALONE_TARGETS = 1
RETIRED_GROUP_MODULES = 1


@dataclass(frozen=True)
class ManifestPin:
    path: str
    opening_blob: str
    opening_sha256: str
    opening_bytes: int
    opening_lines: int
    removal: bytes
    post_blob: str
    post_sha256: str
    post_bytes: int
    post_lines: int
    current_blob: str
    current_sha256: str
    current_bytes: int
    current_lines: int


MANIFEST_PINS = (
    ManifestPin(
        "crates/ivm/tests/grouped/group_08.rs",
        "524f511e980801ebb66b46810df02e2df558f55e",
        "e966de8f500c41891077437fc56a75401c5b2aca66ace4840e29ae0c328eb899",
        1_993,
        68,
        b"""#[path = "../streaming_access_contract.rs"]
mod streaming_access_contract;
""",
        "9976b49482fdb087149d40bb05bbdb2c93116645",
        "64a4df71cd6c9c0f792b41cf34543479bde1141ae451ced9741427e39a0bcabf",
        1_918,
        66,
        "9976b49482fdb087149d40bb05bbdb2c93116645",
        "64a4df71cd6c9c0f792b41cf34543479bde1141ae451ced9741427e39a0bcabf",
        1_918,
        66,
    ),
    ManifestPin(
        "crates/norito/Cargo.toml",
        "c629283d5728fec9e900563c613a7a0df4d41642",
        "e78ebe2ef7d33c41c39419b074dfef56a59394098e1e9c7e3ac7b0d5483d1232",
        4_778,
        231,
        b"""[[bench]]
name = "adaptive_telemetry"
harness = false

""",
        "3aca94a7824e4ffe940941c3d0f4a14e8e787016",
        "99d9aba43f5e6aae77a818631f034d9465cf1c4e6963fc0b9719ae2016c7d0ab",
        4_723,
        227,
        "32617b52ced72537c979c749d14adb3006b238a8",
        "001921234ba49efed859155722186f8a3b1a52d48af593becd8724cb922b6912",
        4_618,
        226,
    ),
)


@dataclass(frozen=True)
class FilePin:
    path: str
    blob: str
    sha256: str
    byte_count: int
    line_count: int


IVM_MANIFEST_PIN = FilePin(
    IVM_MANIFEST,
    "affa10d914c1178bab90ee383ac29d78964cdb8a",
    "8ecbfcfcad9c4829f87a3019d7625697d5e1de27b0ef61eb0780986a3b551eaf",
    8_846,
    324,
)
OPENING_LOCK_PIN = FilePin(
    LOCKFILE,
    "bf7633694c3f2fdca07de4d99743a09bad2daa12",
    "0ddb3f3938cf32035371317100674cd1601c3cb41232237f7a7d28b3aeab6222",
    315_333,
    13_758,
)
LOCK_PIN = FilePin(
    LOCKFILE,
    "e320ffaa8af21674f079573a1aaa1a8d73185ae8",
    "71df4943f58ae56f1a6f5286962ed02ae21b5c1940ac8d3bede09dc10dd424d2",
    311_205,
    13_616,
)
REPLACEMENT_PINS = (
    FilePin(
        "crates/iroha_core/src/streaming.rs",
        "e280345a3c0d211e16137d88cc522c0305ee2f26",
        "48535dd18a998c8196782b7ba4727af9f81674d54923a3c5b3fe0c5606b1a50d",
        258_160,
        6_403,
    ),
    FilePin(
        "crates/norito/examples/telemetry_dump.rs",
        "337ee11cf81f11ab985bb4a0cd27fe60a28a2415",
        "6583649aea781765a4b4247744907211023cb70d2620ea2760fb8eb8622a7458",
        1_502,
        43,
    ),
    FilePin(
        "crates/norito/tests/adaptive_telemetry.rs",
        "ca89936d065ab3842e6f7f0804ada7b60c087051",
        "0d00d220ff8480e47c9ff7457208b4bdafa250115ef3f3c93db0574780fbf98a",
        1_827,
        35,
    ),
    FilePin(
        "crates/norito/tests/adaptive_codec_telemetry.rs",
        "c55ee86364860df54f73e9891df88bda210bec18",
        "1f5c27882e63a2cb05a25ecfc117336101983e513e3339adde1e6a619210429c",
        995,
        23,
    ),
    FilePin(
        "crates/norito/tests/adaptive_more_shapes.rs",
        "b11e279025bc8d57b13e41b19b10c49c0b6d85ce",
        "c44206c54b2890b244dd101ffeb131d4d4436242a80aedf91ad4e5544c0627db",
        3_135,
        84,
    ),
    FilePin(
        "crates/norito/tests/adaptive_combo.rs",
        "2b25119c745d65bdf6d3f217f44d5f2b7fe6d487",
        "fc6f5d3fc2c6f511ac6d5f6afb5a652129c6e8cf4bb19bb36acef844090f4b7b",
        12_641,
        337,
    ),
    FilePin(
        "crates/norito/tests/adaptive_opt_rows.rs",
        "d02053fc91556a9f13a7e86ff04eadc557d53996",
        "4d3c195f3d39c8f3eb4c474ca9a6eae05ec8b5b0851644771d17e2da2463457f",
        4_955,
        120,
    ),
    FilePin(
        "crates/norito/tests/adaptive_enum_rows.rs",
        "432ba3c33de7cf4343f36ce211111bb4cfdebb1e",
        "e8c260f9703afb168e2280f6f34ae1fba81c8797101ba1b34e75ab9271edd910",
        3_096,
        70,
    ),
    FilePin(
        "crates/norito/tests/grouped/group_01.rs",
        "c85519a4088ccc92a1943d8429ec7ad8d7597c55",
        "cc18b96a73943910a37047c0c1b8dafed01e8b5f83c8c307baaeaa593e66ebe3",
        1_909,
        66,
    ),
    FilePin(
        "crates/norito/README.md",
        "8da233a7f39bf4258f9b057f24230d73f54df05b",
        "8a8048219d08298605e66e55e3f4d48b45fd6c1400a0fd32d8fd18380c63425f",
        31_139,
        531,
    ),
)

OPENING_REPLACEMENT_PIN_OVERRIDES = {
    "crates/iroha_core/src/streaming.rs": FilePin(
        "crates/iroha_core/src/streaming.rs",
        "08257a928e5c284ee621e5896a801920788bace4",
        "0d0c5d82d9848c449bd90feef445cab3df86a0fc5b76a472a536502417abe588",
        258_071,
        6_403,
    ),
    "crates/norito/README.md": FilePin(
        "crates/norito/README.md",
        "77c08877ecc5598a599b85e8dc3cea30fe342e7a",
        "831a82698461c7eab98413c2a15229063dd795ce218f4361525321eaee575539",
        30_917,
        529,
    ),
}


@dataclass(frozen=True)
class InventoryPin:
    count: int
    sha256: str


NORITO_BENCH_INVENTORY = InventoryPin(
    25, "6fa6a27cd9bd202974b15f2c7484d0e248fd76c49376c104f1d70381d5ee1e8a"
)
NORITO_IMPLICIT_BENCH_INVENTORY = InventoryPin(
    25, "c5bca93569d8780245446f075fa671975faa81b35d37af1575781bf953e5588d"
)
IVM_TEST_INVENTORY = InventoryPin(
    10, "856416240f096d19170d4c6921b7bd1b6f6b3c81136d8c6f305712d3593c7481"
)
GROUP_MODULE_INVENTORY = InventoryPin(
    32, "e7961fd8e3307294240ea0c01e08f106adb82f6f7fa93b11c3fdeca1d1519b28"
)
WORKFLOW_INVENTORY = InventoryPin(
    13, "205a4336752d37d15b0a88218ca6f15a8b21f375660ebd80b721947520fdfd4d"
)

REPLACEMENT_MARKERS = {
    "crates/iroha_core/src/streaming.rs": (
        ("fn process_streaming_event_registers_ticket_from_ready_event()", 1),
        ("fn process_streaming_event_revokes_ticket()", 1),
        ("fn register_stream_ticket_populates_soranet_defaults()", 1),
        ("fn ticket_revoked_removes_registered_state()", 1),
        ("fn duplicate_ticket_nullifier_is_rejected()", 1),
        ("fn ticket_envelope_commitment_mismatch_is_rejected()", 1),
    ),
    "crates/norito/examples/telemetry_dump.rs": (
        ("fn main()", 1),
        ("norito::columnar::encode_rows_u64_str_bool_adaptive(&rows)", 1),
        ("norito::telemetry::snapshot_json_string()", 1),
    ),
    "crates/norito/tests/grouped/group_01.rs": (
        ('#[path = "../adaptive_telemetry.rs"]', 1),
        ("mod adaptive_telemetry;", 1),
        ('#[path = "../adaptive_codec_telemetry.rs"]', 1),
        ("mod adaptive_codec_telemetry;", 1),
    ),
    "crates/norito/README.md": (
        ("cargo run -p norito --example telemetry_dump", 2),
    ),
}

CONSUMER_PATTERNS = (
    "streaming_access_contract",
    "--bench adaptive_telemetry",
    "crates/norito/benches/adaptive_telemetry.rs",
)
ALLOWED_CONSUMER_HITS: tuple[tuple[str, str], ...] = ()

TEST_RE = re.compile(
    r"(?P<attrs>(?:^[ \t]*#\[[^\n]+\]\n)+)"
    r"^[ \t]*(?:pub(?:\([^\n)]*\))?\s+)?(?:async\s+)?fn\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\(",
    re.MULTILINE,
)
FUNCTION_RE = re.compile(
    r"^[ \t]*(?:pub(?:\([^\n)]*\))?\s+)?(?:async\s+)?(?:const\s+)?fn\s+"
    r"[A-Za-z_][A-Za-z0-9_]*\s*\(",
    re.MULTILINE,
)
TABLE_RE = re.compile(
    r"^\[\[(?P<kind>[^\]]+)\]\]\n(?P<body>.*?)(?=^\[\[|^\[[^[]|\Z)",
    re.MULTILINE | re.DOTALL,
)
FIELD_RE = re.compile(r'^([A-Za-z0-9_-]+)\s*=\s*"([^"]*)"$', re.MULTILINE)
FEATURE_RE = re.compile(r'^required-features\s*=\s*\[([^]]*)\]$', re.MULTILINE)
GROUP_MODULE_RE = re.compile(
    r'^#\[path = "([^"]+)"\]\nmod ([A-Za-z_][A-Za-z0-9_]*);$',
    re.MULTILINE,
)


class GuardError(AssertionError):
    """Raised when the obsolete-unit retirement contract changes."""


@dataclass(frozen=True)
class Snapshot:
    files: dict[str, bytes | None]
    consumer_hits: tuple[tuple[str, str], ...]
    implicit_benches: tuple[str, ...]


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _git_blob_id(data: bytes) -> str:
    header = f"blob {len(data)}\0".encode("ascii")
    return hashlib.sha1(header + data).hexdigest()


def _inventory_digest(rows: tuple[object, ...]) -> str:
    return _sha256(json.dumps(rows, separators=(",", ":")).encode("utf-8"))


def _check_inventory(rows: tuple[object, ...], pin: InventoryPin, label: str) -> None:
    _require(len(rows) == pin.count, f"{label} count changed")
    _require(_inventory_digest(rows) == pin.sha256, f"{label} order or contents changed")


def _git(*arguments: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
    try:
        return subprocess.run(
            ["git", *arguments],
            cwd=ROOT,
            check=check,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise GuardError(f"git {' '.join(arguments)} failed: {error}") from error


def _git_blob(blob: str) -> bytes:
    return _git("cat-file", "blob", blob).stdout


def _regular_bytes(relative: str) -> bytes:
    path = ROOT / relative
    _require(not path.is_symlink(), f"symlink is not allowed: {relative}")
    try:
        mode = path.stat().st_mode
    except OSError as error:
        raise GuardError(f"cannot stat {relative}: {error}") from error
    _require(stat.S_ISREG(mode), f"not a regular file: {relative}")
    try:
        path.resolve(strict=True).relative_to(ROOT.resolve(strict=True))
    except ValueError as error:
        raise GuardError(f"path escapes repository: {relative}") from error
    return path.read_bytes()


def _test_inventory(source: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    tests = []
    for match in TEST_RE.finditer(source):
        attrs = tuple(line.strip() for line in match.group("attrs").splitlines())
        if "#[test]" in attrs or "#[tokio::test]" in attrs:
            tests.append((match.group("name"), attrs))
    return tuple(tests)


def _authenticate_file(pin: FilePin, *, opening_tree: bool) -> bytes:
    if opening_tree:
        tree_blob = _git("rev-parse", f"{OPENING_COMMIT}:{pin.path}").stdout.decode().strip()
        _require(tree_blob == pin.blob, f"opening tree blob changed: {pin.path}")
    data = _git_blob(pin.blob)
    _require(len(data) == pin.byte_count, f"opening byte count changed: {pin.path}")
    _require(data.count(b"\n") == pin.line_count, f"opening line count changed: {pin.path}")
    _require(_sha256(data) == pin.sha256, f"opening content hash changed: {pin.path}")
    return data


def _authenticate_openings() -> dict[str, bytes]:
    _require(
        _git("cat-file", "-t", OPENING_COMMIT).stdout.strip() == b"commit",
        "authenticated opening commit is unavailable",
    )
    _require(sum(pin.line_count for pin in SOURCE_PINS) == RETIRED_RUST_LINES, "Rust line ledger")
    _require(
        sum(pin.function_items for pin in SOURCE_PINS) == RETIRED_FUNCTION_ITEMS,
        "function-item ledger",
    )
    _require(sum(len(pin.test_names) for pin in SOURCE_PINS) == RETIRED_TESTS, "test ledger")
    _require(RETIRED_STANDALONE_TARGETS == 1, "standalone-target ledger")
    _require(RETIRED_GROUP_MODULES == 1, "group-module ledger")

    for pin in SOURCE_PINS:
        tree_blob = _git("rev-parse", f"{OPENING_COMMIT}:{pin.path}").stdout.decode().strip()
        _require(tree_blob == pin.blob, f"opening tree blob changed: {pin.path}")
        data = _git_blob(pin.blob)
        _require(len(data) == pin.byte_count, f"opening byte count changed: {pin.path}")
        _require(data.count(b"\n") == pin.line_count, f"opening line count changed: {pin.path}")
        _require(_sha256(data) == pin.sha256, f"opening content hash changed: {pin.path}")
        source = data.decode("utf-8")
        _require(
            len(FUNCTION_RE.findall(source)) == pin.function_items,
            f"opening function inventory changed: {pin.path}",
        )
        tests = _test_inventory(source)
        _require(tuple(name for name, _attrs in tests) == pin.test_names, "opening test names")
        _require(all(attrs == ("#[test]",) for _name, attrs in tests), "opening test attrs")
        _require(
            _inventory_digest(tests) == pin.test_ledger_sha256,
            f"opening test ledger hash changed: {pin.path}",
        )

    openings = {}
    for pin in MANIFEST_PINS:
        tree_blob = _git("rev-parse", f"{OPENING_COMMIT}:{pin.path}").stdout.decode().strip()
        _require(tree_blob == pin.opening_blob, f"opening declaration blob changed: {pin.path}")
        data = _git_blob(pin.opening_blob)
        _require(len(data) == pin.opening_bytes, f"opening declaration bytes changed: {pin.path}")
        _require(data.count(b"\n") == pin.opening_lines, "opening declaration lines changed")
        _require(_sha256(data) == pin.opening_sha256, "opening declaration hash changed")
        openings[pin.path] = data

    _authenticate_file(IVM_MANIFEST_PIN, opening_tree=True)
    _authenticate_file(OPENING_LOCK_PIN, opening_tree=True)
    for pin in REPLACEMENT_PINS:
        _authenticate_file(
            OPENING_REPLACEMENT_PIN_OVERRIDES.get(pin.path, pin),
            opening_tree=True,
        )
    return openings


def _expected_post(pin: ManifestPin, opening: bytes) -> bytes:
    _require(opening.count(pin.removal) == 1, f"retired declaration count changed: {pin.path}")
    postimage = opening.replace(pin.removal, b"", 1)
    _require(len(postimage) == pin.post_bytes, f"postimage byte ledger changed: {pin.path}")
    _require(postimage.count(b"\n") == pin.post_lines, f"postimage lines changed: {pin.path}")
    _require(_sha256(postimage) == pin.post_sha256, f"postimage hash changed: {pin.path}")
    _require(_git_blob_id(postimage) == pin.post_blob, f"postimage blob changed: {pin.path}")
    return postimage


def _tables(manifest: str, kind: str) -> tuple[tuple[str, str | None, tuple[str, ...]], ...]:
    rows = []
    for table in TABLE_RE.finditer(manifest):
        if table.group("kind") != kind:
            continue
        body = table.group("body")
        fields = dict(FIELD_RE.findall(body))
        feature_match = FEATURE_RE.search(body)
        features = (
            tuple(re.findall(r'"([^"]+)"', feature_match.group(1))) if feature_match else ()
        )
        rows.append((fields["name"], fields.get("path"), features))
    return tuple(rows)


def _workflow_modules(workflow: str) -> tuple[str, ...]:
    blocks = re.findall(
        r"python3 -m unittest \\\n(?P<body>(?:[ \t]+scripts\.tests\.[A-Za-z0-9_]+(?: \\\n|\n))+)",
        workflow,
    )
    matches = [
        tuple(re.findall(r"scripts\.tests\.[A-Za-z0-9_]+", block))
        for block in blocks
        if GUARD_MODULE in block
    ]
    _require(len(matches) == 1, "retirement guard unittest block changed")
    return matches[0]


def _consumer_inventory() -> tuple[tuple[str, str], ...]:
    arguments = ["grep", "--no-index", "--exclude-standard", "-n", "-I", "-F"]
    for pattern in CONSUMER_PATTERNS:
        arguments.extend(("-e", pattern))
    arguments.extend(("--", "."))
    result = _git(*arguments, check=False)
    _require(result.returncode in (0, 1), "active-consumer scan failed")
    rows = []
    for raw_line in result.stdout.decode("utf-8").splitlines():
        path, _line, text = raw_line.split(":", 2)
        path = path.removeprefix("./")
        if path in {GUARD_PATH, "status.md"}:
            continue
        rows.append((path, text))
    return tuple(sorted(rows))


def _implicit_norito_benches() -> tuple[str, ...]:
    root = ROOT / "crates/norito/benches"
    _require(root.is_dir() and not root.is_symlink(), "Norito bench root changed")
    names = []
    for entry in root.iterdir():
        _require(not entry.is_symlink(), f"Norito bench symlink is not allowed: {entry.name}")
        if entry.is_file() and entry.suffix == ".rs":
            names.append(entry.stem)
        elif entry.is_dir():
            main = entry / "main.rs"
            _require(not main.is_symlink(), f"Norito nested bench symlink: {entry.name}")
            if main.is_file():
                names.append(entry.name)
    return tuple(sorted(names))


def _snapshot() -> Snapshot:
    paths = {
        LOCKFILE,
        IVM_MANIFEST,
        WORKFLOW,
        *(pin.path for pin in MANIFEST_PINS),
        *(pin.path for pin in REPLACEMENT_PINS),
    }
    files: dict[str, bytes | None] = {path: _regular_bytes(path) for path in paths}
    for pin in SOURCE_PINS:
        path = ROOT / pin.path
        files[pin.path] = _regular_bytes(pin.path) if path.exists() or path.is_symlink() else None
    return Snapshot(files, _consumer_inventory(), _implicit_norito_benches())


def _validate(snapshot: Snapshot, openings: dict[str, bytes]) -> None:
    for pin in SOURCE_PINS:
        _require(snapshot.files[pin.path] is None, f"retired source resurrected: {pin.path}")
    for pin in MANIFEST_PINS:
        data = snapshot.files[pin.path]
        _require(data is not None, f"declaration file missing: {pin.path}")
        _expected_post(pin, openings[pin.path])
        _require(
            len(data) == pin.current_bytes,
            f"declaration postimage bytes changed: {pin.path}",
        )
        _require(
            data.count(b"\n") == pin.current_lines,
            f"declaration postimage lines changed: {pin.path}",
        )
        _require(
            _sha256(data) == pin.current_sha256,
            f"declaration postimage hash changed: {pin.path}",
        )
        _require(
            data == _git_blob(pin.current_blob),
            f"declaration postimage differs from current authority: {pin.path}",
        )

    norito = snapshot.files["crates/norito/Cargo.toml"]
    ivm = snapshot.files[IVM_MANIFEST]
    group = snapshot.files["crates/ivm/tests/grouped/group_08.rs"]
    assert norito is not None and ivm is not None and group is not None
    _require(norito.decode().count("autotests = false") == 1, "Norito autotests changed")
    benches = _tables(norito.decode(), "bench")
    _require(all(path is None and not features for _name, path, features in benches), "bench shape")
    _require(norito.decode().count("harness = false") == len(benches), "bench harness ledger")
    _check_inventory(benches, NORITO_BENCH_INVENTORY, "Norito bench")
    _check_inventory(
        snapshot.implicit_benches,
        NORITO_IMPLICIT_BENCH_INVENTORY,
        "Norito implicit bench",
    )
    _require(ivm == _git_blob(IVM_MANIFEST_PIN.blob), "IVM manifest changed")
    _require(ivm.decode().count("autobins = false") == 1, "IVM autobins changed")
    _require(ivm.decode().count("autotests = false") == 1, "IVM autotests changed")
    _check_inventory(_tables(ivm.decode(), "test"), IVM_TEST_INVENTORY, "IVM test target")
    modules = tuple(GROUP_MODULE_RE.findall(group.decode()))
    _check_inventory(modules, GROUP_MODULE_INVENTORY, "IVM group_08 module")

    lock = snapshot.files[LOCKFILE]
    assert lock is not None
    _require(lock == _git_blob(LOCK_PIN.blob), "Cargo.lock differs from current authority")
    for pin in REPLACEMENT_PINS:
        data = snapshot.files[pin.path]
        _require(data is not None, f"replacement missing: {pin.path}")
        _require(len(data) == pin.byte_count, f"replacement bytes changed: {pin.path}")
        _require(data.count(b"\n") == pin.line_count, f"replacement lines changed: {pin.path}")
        _require(_sha256(data) == pin.sha256, f"replacement content changed: {pin.path}")
        _require(_git_blob_id(data) == pin.blob, f"replacement blob changed: {pin.path}")
    for path, markers in REPLACEMENT_MARKERS.items():
        source = snapshot.files[path]
        assert source is not None
        text = source.decode()
        for marker, count in markers:
            _require(text.count(marker) == count, f"replacement marker changed: {path}: {marker}")

    workflow = snapshot.files[WORKFLOW]
    assert workflow is not None
    modules = _workflow_modules(workflow.decode())
    _check_inventory(modules, WORKFLOW_INVENTORY, "workflow unittest")
    _require(modules == tuple(sorted(modules)), "workflow unittest inventory is not alphabetized")
    _require(
        snapshot.consumer_hits == ALLOWED_CONSUMER_HITS,
        f"active retired-surface consumer inventory changed: {snapshot.consumer_hits}",
    )


def _mutate(snapshot: Snapshot, path: str, data: bytes | None) -> Snapshot:
    files = dict(snapshot.files)
    files[path] = data
    return Snapshot(files, snapshot.consumer_hits, snapshot.implicit_benches)


class ObsoleteDeveloperTestRetirementSourceTest(unittest.TestCase):
    """Authenticate the retirement and representative fail-closed mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.openings = _authenticate_openings()
        cls.snapshot = _snapshot()

    def test_retirement_contract(self) -> None:
        _validate(self.snapshot, self.openings)

    def test_mutation_each_deleted_source_resurrection_fails(self) -> None:
        for pin in SOURCE_PINS:
            with self.subTest(path=pin.path):
                with self.assertRaisesRegex(GuardError, "retired source resurrected"):
                    _validate(
                        _mutate(self.snapshot, pin.path, b"fn resurrected() {}\n"),
                        self.openings,
                    )

    def test_mutation_each_declaration_resurrection_fails(self) -> None:
        for pin in MANIFEST_PINS:
            with self.subTest(path=pin.path):
                data = self.snapshot.files[pin.path]
                assert data is not None
                with self.assertRaisesRegex(GuardError, "declaration postimage"):
                    _validate(_mutate(self.snapshot, pin.path, data + pin.removal), self.openings)

    def test_mutation_replacement_source_fails(self) -> None:
        path = "crates/iroha_core/src/streaming.rs"
        source = self.snapshot.files[path]
        assert source is not None
        mutated = source.replace(
            b"fn process_streaming_event_registers_ticket_from_ready_event(",
            b"fn weakened_streaming_event_registers_ticket_from_ready_event(",
            1,
        )
        with self.assertRaisesRegex(GuardError, "replacement"):
            _validate(_mutate(self.snapshot, path, mutated), self.openings)

    def test_mutation_lock_fails(self) -> None:
        lock = self.snapshot.files[LOCKFILE]
        assert lock is not None
        with self.assertRaisesRegex(GuardError, "Cargo.lock"):
            _validate(_mutate(self.snapshot, LOCKFILE, lock + b"# drift\n"), self.openings)

    def test_mutation_active_consumer_fails(self) -> None:
        mutated = Snapshot(
            dict(self.snapshot.files),
            self.snapshot.consumer_hits + (("README.md", "run streaming_access_contract"),),
            self.snapshot.implicit_benches,
        )
        with self.assertRaisesRegex(GuardError, "active retired-surface consumer"):
            _validate(mutated, self.openings)

    def test_mutation_nested_bench_resurrection_fails(self) -> None:
        mutated = Snapshot(
            dict(self.snapshot.files),
            self.snapshot.consumer_hits,
            tuple(sorted((*self.snapshot.implicit_benches, "adaptive_telemetry"))),
        )
        with self.assertRaisesRegex(GuardError, "Norito implicit bench"):
            _validate(mutated, self.openings)

    def test_mutation_workflow_hook_removal_fails(self) -> None:
        workflow = self.snapshot.files[WORKFLOW]
        assert workflow is not None
        line = f"            {GUARD_MODULE} \\\n".encode()
        with self.assertRaisesRegex(GuardError, "retirement guard unittest block"):
            _validate(_mutate(self.snapshot, WORKFLOW, workflow.replace(line, b"", 1)), self.openings)

    def test_mutation_workflow_order_fails(self) -> None:
        workflow = self.snapshot.files[WORKFLOW]
        assert workflow is not None
        first = f"            {GUARD_MODULE} \\\n".encode()
        second = b"            scripts.tests.shared_proc_macro_emitter_source_test\n"
        swapped = second.rstrip(b"\n") + b" \\\n" + first.rstrip(b" \\\n") + b"\n"
        with self.assertRaisesRegex(GuardError, "workflow unittest"):
            _validate(
                _mutate(self.snapshot, WORKFLOW, workflow.replace(first + second, swapped, 1)),
                self.openings,
            )


if __name__ == "__main__":
    unittest.main()
