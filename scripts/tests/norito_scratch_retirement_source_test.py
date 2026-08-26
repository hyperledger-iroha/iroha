#!/usr/bin/env python3
"""Fail closed on retired Norito print-only scratch targets.

This stdlib-only guard authenticates the deleted source preimages through Git
objects, seals the grouped-test postimage, preserves the complete Norito target
manifest and lockfile, and requires stronger retained regression coverage. Its
mutation tests operate only on in-memory snapshots.
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
GROUP_ROOT = "crates/norito/tests/grouped/group_05.rs"
MANIFEST = "crates/norito/Cargo.toml"
LOCKFILE = "Cargo.lock"
GUARD_PATH = "scripts/tests/norito_scratch_retirement_source_test.py"


@dataclass(frozen=True)
class SourcePin:
    path: str
    blob: str
    sha256: str
    byte_count: int
    line_count: int
    functions: tuple[str, ...]
    tests: tuple[tuple[str, tuple[str, ...]], ...]
    test_ledger_sha256: str
    crate_cfg: str | None = None


SOURCE_PINS = (
    SourcePin(
        "crates/norito/tests/temp_print_small3.rs",
        "540d5447a0dcfc93d1ff987df9a46e0d12f9ba20",
        "e5e89bc5080d345c5da967c250b8bc71a5767866dce574c43d1b82397865f40f",
        1_406,
        39,
        (
            "to_hex",
            "print_offsets_code_delta_variant1",
            "print_offsets_code_delta_variant2",
        ),
        (
            ("print_offsets_code_delta_variant1", ("#[test]",)),
            ("print_offsets_code_delta_variant2", ("#[test]",)),
        ),
        "0ce5b1ff93edeaa96d9d75ce9c5402ebd207c26693fdbeab5eaf89fa1b4f5861",
        '#![cfg(feature = "json")]',
    ),
    SourcePin(
        "crates/norito/tests/temp_print_nested.rs",
        "a6d4a2378c04bb65962876e9dc0d648d8639d15a",
        "98873be296cfcd28d99f832095757c8d50629a477e2eda595f65cdf1e41b8970",
        970,
        27,
        ("to_hex", "print_offsets_nested_window"),
        (("print_offsets_nested_window", ("#[test]",)),),
        "335cd6ee2e55610a7ae4d4980b97832493e173c58d643f7715d07df9b59e352a",
        '#![cfg(feature = "json")]',
    ),
    SourcePin(
        "crates/norito/tests/type_debug.rs",
        "9f222a05c30e3bfd405218a160f9aa98633340b4",
        "9a213b442d6afac968b38d0ffc9bb697fd74ad3c8d1c6127350fe1e43ad25839",
        133,
        5,
        ("print_archived_box_ty",),
        (("print_archived_box_ty", ("#[test]",)),),
        "4c378e64e2b15bcbd0944cc0de8009dfca3df1ff5027b5c17d74537abb810ee1",
    ),
    SourcePin(
        "crates/norito/examples/repro_vecdeque.rs",
        "1ebc1920750df272823e0066c0b1321a9fc16aff",
        "2e13d79828872ff35b1f142f9699e77cb1167a49e63f034629bfad6138662a38",
        2_531,
        77,
        ("main",),
        (),
        "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
    ),
)

RETIRED_RUST_LINES = 148
RETIRED_SOURCE_BYTES = 5_040
RETIRED_TESTS = 4
RETIRED_FUNCTION_ITEMS = 7
RETIRED_MODULE_ITEMS = 3
RETIRED_COMPILER_UNITS = 1

GROUP_BLOB = "9c5cd30984cb13ea7a280d9bc2786a9551078a31"
GROUP_SHA256 = "32ef1238333208aa963ef6937b41b0c8a955190619d65f1a8cd4a2b3b90a4968"
GROUP_BYTES = 2_156
GROUP_LINES = 68
POST_GROUP_SHA256 = "67df42e03e9155d6e21353cdd5c609299d485aa7098a3d544d073e75b37805e1"
POST_GROUP_BYTES = 1_993
POST_GROUP_LINES = 62

OPENING_MANIFEST_BLOB = "c629283d5728fec9e900563c613a7a0df4d41642"
OPENING_MANIFEST_SHA256 = "e78ebe2ef7d33c41c39419b074dfef56a59394098e1e9c7e3ac7b0d5483d1232"
OPENING_MANIFEST_BYTES = 4_778
OPENING_MANIFEST_LINES = 231
MANIFEST_BLOB = "32617b52ced72537c979c749d14adb3006b238a8"
MANIFEST_SHA256 = "001921234ba49efed859155722186f8a3b1a52d48af593becd8724cb922b6912"
MANIFEST_BYTES = 4_618
MANIFEST_LINES = 226

OPENING_LOCK_BLOB = "bf7633694c3f2fdca07de4d99743a09bad2daa12"
OPENING_LOCK_SHA256 = "0ddb3f3938cf32035371317100674cd1601c3cb41232237f7a7d28b3aeab6222"
OPENING_LOCK_BYTES = 315_333
OPENING_LOCK_LINES = 13_758
LOCK_BLOB = "e320ffaa8af21674f079573a1aaa1a8d73185ae8"
LOCK_SHA256 = "71df4943f58ae56f1a6f5286962ed02ae21b5c1940ac8d3bede09dc10dd424d2"
LOCK_BYTES = 311_205
LOCK_LINES = 13_616

RETIRED_MODULE_BLOCKS = (
    '#[path = "../temp_print_nested.rs"]\nmod temp_print_nested;\n',
    '#[path = "../temp_print_small3.rs"]\nmod temp_print_small3;\n',
    '#[path = "../type_debug.rs"]\nmod type_debug;\n',
)

EXPECTED_TEST_TABLES = (
    ("norito_group_01", "tests/grouped/group_01.rs", ()),
    ("norito_group_02", "tests/grouped/group_02.rs", ()),
    ("norito_group_03", "tests/grouped/group_03.rs", ()),
    ("norito_group_04", "tests/grouped/group_04.rs", ()),
    ("norito_group_05", "tests/grouped/group_05.rs", ()),
    ("norito_group_06", "tests/grouped/group_06.rs", ()),
    (
        "exact_field_streaming_allocations",
        "tests/exact_field_streaming_allocations.rs",
        (),
    ),
    (
        "json_parse_string_allocations",
        "tests/json_parse_string_allocations.rs",
        ("json",),
    ),
)

EXPECTED_EXAMPLE_TARGETS = (
    ("dump_tape", "crates/norito/examples/dump_tape.rs"),
    ("gen_enum_large_hex", "crates/norito/examples/gen_enum_large_hex.rs"),
    ("gen_struct_tape", "crates/norito/examples/gen_struct_tape.rs"),
    ("gpu_threshold", "crates/norito/examples/gpu_threshold.rs"),
    ("reader_unescape", "crates/norito/examples/reader_unescape.rs"),
    ("stage1_cutover", "crates/norito/examples/stage1_cutover.rs"),
    ("telemetry_dump", "crates/norito/examples/telemetry_dump.rs"),
    ("telemetry_watch", "crates/norito/examples/telemetry_watch.rs"),
)

REPLACEMENT_MARKERS = {
    "crates/norito/tests/aos_ncb_more_golden.rs": (
        ("fn ncb_enum_offsets_code_delta_variant1_fixture()", 1),
        ("tests/data/enum_offsets_code_delta_variant1.hex", 1),
        ("fn ncb_enum_offsets_code_delta_variant2_fixture()", 1),
        ("tests/data/enum_offsets_code_delta_variant2.hex", 1),
    ),
    "crates/norito/tests/ncb_enum_iter_samples.rs": (
        ("fn offsets_nested_window_fixture()", 1),
        ("tests/data/enum_offsets_nested_window.hex", 1),
        ('assert_eq!(r#gen, hex, "offsets nested window fixture mismatch");', 1),
    ),
    "crates/norito/tests/codec.rs": (
        ("fn box_roundtrip()", 1),
        ("fn vecdeque_roundtrip()", 1),
        ("fn binaryheap_roundtrip()", 1),
    ),
    "crates/norito/tests/containers_decode.rs": (
        ("fn vecdeque_roundtrip()", 1),
        ("fn binaryheap_roundtrip()", 1),
        ("assert_eq!(heap.into_sorted_vec(), out.into_sorted_vec());", 1),
    ),
}

OPENING_REPLACEMENT_SOURCE_PINS = {
    "crates/norito/tests/aos_ncb_more_golden.rs": (
        "e3727e973be22bcca426934398e2205085d17f7a",
        "4d1154124399e218a597a5a70a8dbdd565db89fd0f87726a5a471736846b5fd8",
        14_539,
        338,
    ),
    "crates/norito/tests/ncb_enum_iter_samples.rs": (
        "63352420d53e66a5c623c697b986d8d8117646ef",
        "7e55a259c5813ee374ceb06459fc0387ad745fabdd8608050d73609f27654818",
        42_883,
        1_099,
    ),
    "crates/norito/tests/codec.rs": (
        "ec75d1ad45a735c580dbff5b2eb38e47f6d1cd20",
        "a8ca675d7628f9cfb59fa26aa8cac0da5696138067fbfc2ac39cc02c5af339cd",
        15_377,
        454,
    ),
    "crates/norito/tests/containers_decode.rs": (
        "6dbc6eb099f96d0b554feff1a1f9dd3ef17fb25d",
        "4535283cfa1333630e62b2e27ff29036f0156d65dfb9cdf8893b41f17d6955a0",
        2_416,
        69,
    ),
}

REPLACEMENT_SOURCE_PINS = {
    **OPENING_REPLACEMENT_SOURCE_PINS,
    "crates/norito/tests/codec.rs": (
        "4f1d755d3982c18669094aff58f80a4572d9f2b9",
        "a0f45a11fdb725e917f7f9aaaec0e6d866a0d3e236623ae0e3f8303d59781666",
        15_454,
        455,
    ),
}

RETIRED_IDENTIFIERS = (
    "temp_print_small3",
    "temp_print_nested",
    "type_debug",
    "repro_vecdeque",
    "print_offsets_code_delta_variant1",
    "print_offsets_code_delta_variant2",
    "print_offsets_nested_window",
    "print_archived_box_ty",
    "OFFSETS_CODE_DELTA_VARIANT1",
    "OFFSETS_CODE_DELTA_VARIANT2",
    "OFFSETS_NESTED_WINDOW",
)

TEST_RE = re.compile(
    r"(?P<attrs>(?:^[ \t]*#\[[^\n]+\]\n)+)"
    r"^[ \t]*(?:pub(?:\([^\n)]*\))?\s+)?(?:async\s+)?fn\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\(",
    re.MULTILINE,
)
FUNCTION_RE = re.compile(
    r"^[ \t]*(?:pub(?:\([^\n)]*\))?\s+)?(?:async\s+)?fn\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)\s*\(",
    re.MULTILINE,
)
TABLE_RE = re.compile(
    r"^\[\[(?P<kind>[^\]]+)\]\]\n(?P<body>.*?)(?=^\[\[|^\[[^[]|\Z)",
    re.MULTILINE | re.DOTALL,
)
FIELD_RE = re.compile(r'^([A-Za-z0-9_-]+)\s*=\s*"([^"]*)"$', re.MULTILINE)
FEATURE_RE = re.compile(r"^required-features\s*=\s*\[([^]]*)\]$", re.MULTILINE)


class GuardError(AssertionError):
    """Raised when the authenticated retirement contract changes."""


@dataclass(frozen=True)
class Snapshot:
    files: dict[str, bytes | None]
    example_targets: tuple[tuple[str, str], ...]
    consumer_hits: tuple[str, ...]


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


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
        attributes = tuple(line.strip() for line in match.group("attrs").splitlines())
        if "#[test]" in attributes:
            tests.append((match.group("name"), attributes))
    return tuple(tests)


def _test_digest(tests: tuple[tuple[str, tuple[str, ...]], ...]) -> str:
    return _sha256(json.dumps(tests, separators=(",", ":")).encode("utf-8"))


def _authenticate_blob(
    path: str,
    blob: str,
    sha256: str,
    byte_count: int,
    line_count: int,
) -> bytes:
    tree_blob = _git("rev-parse", f"{OPENING_COMMIT}:{path}").stdout.decode().strip()
    _require(tree_blob == blob, f"opening tree blob changed: {path}")
    data = _git_blob(blob)
    _require(len(data) == byte_count, f"opening byte count changed: {path}")
    _require(data.count(b"\n") == line_count, f"opening line count changed: {path}")
    _require(_sha256(data) == sha256, f"opening content hash changed: {path}")
    return data


def _authenticate_openings() -> tuple[bytes, bytes]:
    commit_type = _git("cat-file", "-t", OPENING_COMMIT).stdout.strip()
    _require(commit_type == b"commit", "authenticated opening commit is unavailable")
    _require(
        sum(pin.line_count for pin in SOURCE_PINS) == RETIRED_RUST_LINES,
        "retired Rust line ledger changed",
    )
    _require(
        sum(pin.byte_count for pin in SOURCE_PINS) == RETIRED_SOURCE_BYTES,
        "retired source byte ledger changed",
    )
    _require(sum(len(pin.tests) for pin in SOURCE_PINS) == RETIRED_TESTS, "test ledger changed")
    _require(
        sum(len(pin.functions) for pin in SOURCE_PINS) == RETIRED_FUNCTION_ITEMS,
        "function-item ledger changed",
    )
    _require(len(RETIRED_MODULE_BLOCKS) == RETIRED_MODULE_ITEMS, "module-item ledger changed")
    _require(
        sum(pin.path.startswith("crates/norito/examples/") for pin in SOURCE_PINS)
        == RETIRED_COMPILER_UNITS,
        "compiler-unit ledger changed",
    )
    for pin in SOURCE_PINS:
        data = _authenticate_blob(
            pin.path, pin.blob, pin.sha256, pin.byte_count, pin.line_count
        )
        source = data.decode("utf-8")
        _require(tuple(FUNCTION_RE.findall(source)) == pin.functions, f"function ledger changed: {pin.path}")
        tests = _test_inventory(source)
        _require(tests == pin.tests, f"test inventory changed: {pin.path}")
        _require(_test_digest(tests) == pin.test_ledger_sha256, f"test digest changed: {pin.path}")
        if pin.crate_cfg is not None:
            _require(source.count(pin.crate_cfg) == 1, f"feature gate changed: {pin.path}")
    group = _authenticate_blob(GROUP_ROOT, GROUP_BLOB, GROUP_SHA256, GROUP_BYTES, GROUP_LINES)
    manifest = _authenticate_blob(
        MANIFEST,
        OPENING_MANIFEST_BLOB,
        OPENING_MANIFEST_SHA256,
        OPENING_MANIFEST_BYTES,
        OPENING_MANIFEST_LINES,
    )
    _authenticate_blob(
        LOCKFILE,
        OPENING_LOCK_BLOB,
        OPENING_LOCK_SHA256,
        OPENING_LOCK_BYTES,
        OPENING_LOCK_LINES,
    )
    for path, (blob, sha256, byte_count, line_count) in OPENING_REPLACEMENT_SOURCE_PINS.items():
        _authenticate_blob(path, blob, sha256, byte_count, line_count)
    return group, manifest


def _expected_group(opening: bytes) -> bytes:
    text = opening.decode("utf-8")
    for block in RETIRED_MODULE_BLOCKS:
        _require(text.count(block) == 1, "retired module opening count changed")
        text = text.replace(block, "", 1)
    postimage = text.encode("utf-8")
    _require(len(postimage) == POST_GROUP_BYTES, "group postimage byte ledger changed")
    _require(postimage.count(b"\n") == POST_GROUP_LINES, "group postimage line ledger changed")
    _require(_sha256(postimage) == POST_GROUP_SHA256, "group postimage hash changed")
    return postimage


def _test_tables(manifest: str) -> tuple[tuple[str, str, tuple[str, ...]], ...]:
    rows = []
    for table in TABLE_RE.finditer(manifest):
        if table.group("kind") != "test":
            continue
        body = table.group("body")
        fields = dict(FIELD_RE.findall(body))
        _require("name" in fields and "path" in fields, "test table fields changed")
        feature_match = FEATURE_RE.search(body)
        features = () if feature_match is None else tuple(re.findall(r'"([^"]+)"', feature_match.group(1)))
        rows.append((fields["name"], fields["path"], features))
    return tuple(rows)


def _example_targets() -> tuple[tuple[str, str], ...]:
    directory = ROOT / "crates/norito/examples"
    targets = []
    for path in (*directory.glob("*.rs"), *directory.glob("*/main.rs")):
        relative = path.relative_to(ROOT).as_posix()
        _regular_bytes(relative)
        name = path.stem if path.parent == directory else path.parent.name
        targets.append((name, relative))
    return tuple(sorted(targets))


def _active_consumer_hits() -> tuple[str, ...]:
    arguments = ["grep", "--untracked", "-n", "-I", "-F"]
    for identifier in RETIRED_IDENTIFIERS:
        arguments.extend(("-e", identifier))
    arguments.extend(("--", ".", ":(exclude)status.md", ":(exclude)roadmap.md", f":(exclude){GUARD_PATH}"))
    result = _git(*arguments, check=False)
    _require(result.returncode in (0, 1), "active-consumer scan failed")
    if result.returncode == 1:
        return ()
    return tuple(line for line in result.stdout.decode("utf-8").splitlines() if line)


def _snapshot() -> Snapshot:
    paths = {GROUP_ROOT, MANIFEST, LOCKFILE, *REPLACEMENT_MARKERS}
    files: dict[str, bytes | None] = {path: _regular_bytes(path) for path in paths}
    for pin in SOURCE_PINS:
        path = ROOT / pin.path
        files[pin.path] = _regular_bytes(pin.path) if path.exists() or path.is_symlink() else None
    return Snapshot(
        files=files,
        example_targets=_example_targets(),
        consumer_hits=_active_consumer_hits(),
    )


def _validate(snapshot: Snapshot, opening_group: bytes, _opening_manifest: bytes) -> None:
    for pin in SOURCE_PINS:
        _require(snapshot.files[pin.path] is None, f"retired source resurrected: {pin.path}")

    group = snapshot.files[GROUP_ROOT]
    _require(group is not None, "grouped test root is missing")
    _require(group == _expected_group(opening_group), "grouped test root postimage drifted")
    _require(_sha256(group) == POST_GROUP_SHA256, "grouped test root hash drifted")
    _require(len(re.findall(rb"(?m)^mod [A-Za-z_][A-Za-z0-9_]*;$", group)) == 30, "grouped module ledger changed")

    manifest = snapshot.files[MANIFEST]
    _require(manifest is not None, "Norito manifest is missing")
    _require(len(manifest) == MANIFEST_BYTES, "Norito manifest byte count changed")
    _require(manifest.count(b"\n") == MANIFEST_LINES, "Norito manifest line count changed")
    _require(_sha256(manifest) == MANIFEST_SHA256, "Norito manifest content changed")
    _require(manifest == _git_blob(MANIFEST_BLOB), "Norito manifest differs from current authority")
    manifest_text = manifest.decode("utf-8")
    _require(len(re.findall(r"^autotests\s*=\s*false$", manifest_text, re.MULTILINE)) == 1, "autotests=false contract changed")
    _require(not re.search(r"^autoexamples\s*=", manifest_text, re.MULTILINE), "autoexample discovery contract changed")
    _require(_test_tables(manifest_text) == EXPECTED_TEST_TABLES, "explicit test target ledger changed")
    _require(
        snapshot.example_targets == EXPECTED_EXAMPLE_TARGETS,
        "autoexample target ledger changed",
    )

    lock = snapshot.files[LOCKFILE]
    _require(lock is not None, "Cargo.lock is missing")
    _require(len(lock) == LOCK_BYTES, "Cargo.lock byte count changed")
    _require(lock.count(b"\n") == LOCK_LINES, "Cargo.lock line count changed")
    _require(_sha256(lock) == LOCK_SHA256, "Cargo.lock hash changed")
    _require(lock == _git_blob(LOCK_BLOB), "Cargo.lock differs from current authority")

    for path, markers in REPLACEMENT_MARKERS.items():
        data = snapshot.files[path]
        _require(data is not None, f"replacement source is missing: {path}")
        blob, sha256, byte_count, line_count = REPLACEMENT_SOURCE_PINS[path]
        _require(len(data) == byte_count, f"replacement source byte count changed: {path}")
        _require(
            data.count(b"\n") == line_count,
            f"replacement source line count changed: {path}",
        )
        _require(_sha256(data) == sha256, f"replacement source content changed: {path}")
        _require(data == _git_blob(blob), f"replacement source differs from opening: {path}")
        source = data.decode("utf-8")
        for marker, count in markers:
            _require(source.count(marker) == count, f"replacement marker changed in {path}: {marker}")
    _require(not snapshot.consumer_hits, f"active retired-surface consumer found: {snapshot.consumer_hits}")


def _mutate(snapshot: Snapshot, path: str, data: bytes | None) -> Snapshot:
    files = dict(snapshot.files)
    files[path] = data
    return Snapshot(
        files=files,
        example_targets=snapshot.example_targets,
        consumer_hits=snapshot.consumer_hits,
    )


class NoritoScratchRetirementSourceTest(unittest.TestCase):
    """Authenticate the retirement and prove representative mutations fail."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.opening_group, cls.opening_manifest = _authenticate_openings()
        cls.snapshot = _snapshot()

    def validate(self, snapshot: Snapshot) -> None:
        _validate(snapshot, self.opening_group, self.opening_manifest)

    def test_retirement_contract(self) -> None:
        self.validate(self.snapshot)

    def test_mutation_deleted_source_resurrection_fails(self) -> None:
        mutated = _mutate(self.snapshot, SOURCE_PINS[0].path, b"#[test]\nfn resurrected() {}\n")
        with self.assertRaisesRegex(GuardError, "retired source resurrected"):
            self.validate(mutated)

    def test_mutation_module_resurrection_fails(self) -> None:
        group = self.snapshot.files[GROUP_ROOT]
        assert group is not None
        mutated = _mutate(self.snapshot, GROUP_ROOT, group + RETIRED_MODULE_BLOCKS[0].encode())
        with self.assertRaisesRegex(GuardError, "grouped test root"):
            self.validate(mutated)

    def test_mutation_group_order_fails(self) -> None:
        group = self.snapshot.files[GROUP_ROOT]
        assert group is not None
        mutated = _mutate(
            self.snapshot,
            GROUP_ROOT,
            group.replace(b"mod transport_capabilities;", b"mod z_transport_capabilities;", 1),
        )
        with self.assertRaisesRegex(GuardError, "grouped test root"):
            self.validate(mutated)

    def test_mutation_commented_autoexamples_disable_fails(self) -> None:
        manifest = self.snapshot.files[MANIFEST]
        assert manifest is not None
        mutated = _mutate(
            self.snapshot,
            MANIFEST,
            manifest.replace(
                b"autotests = false\n",
                b"autotests = false\nautoexamples = false # drift\n",
                1,
            ),
        )
        with self.assertRaisesRegex(GuardError, "Norito manifest"):
            self.validate(mutated)

    def test_mutation_test_target_fails(self) -> None:
        manifest = self.snapshot.files[MANIFEST]
        assert manifest is not None
        mutated = _mutate(
            self.snapshot,
            MANIFEST,
            manifest.replace(b'name = "norito_group_05"', b'name = "norito_group_05_drift"', 1),
        )
        with self.assertRaisesRegex(GuardError, "Norito manifest"):
            self.validate(mutated)

    def test_mutation_dependency_drift_fails(self) -> None:
        manifest = self.snapshot.files[MANIFEST]
        assert manifest is not None
        mutated = _mutate(
            self.snapshot,
            MANIFEST,
            manifest.replace(b"criterion = { workspace = true }\n", b"", 1),
        )
        with self.assertRaisesRegex(GuardError, "Norito manifest"):
            self.validate(mutated)

    def test_mutation_lock_fails(self) -> None:
        lock = self.snapshot.files[LOCKFILE]
        assert lock is not None
        mutated = _mutate(self.snapshot, LOCKFILE, lock + b"# drift\n")
        with self.assertRaisesRegex(GuardError, "Cargo.lock"):
            self.validate(mutated)

    def test_mutation_active_reference_fails(self) -> None:
        mutated = Snapshot(
            files=dict(self.snapshot.files),
            example_targets=self.snapshot.example_targets,
            consumer_hits=("README.md:1:cargo run -p norito --example repro_vecdeque",),
        )
        with self.assertRaisesRegex(GuardError, "active retired-surface consumer"):
            self.validate(mutated)

    def test_mutation_replacement_marker_fails(self) -> None:
        path = "crates/norito/tests/aos_ncb_more_golden.rs"
        source = self.snapshot.files[path]
        assert source is not None
        mutated = _mutate(
            self.snapshot,
            path,
            source.replace(
                b"fn ncb_enum_offsets_code_delta_variant1_fixture()",
                b"fn removed_variant1_fixture()",
                1,
            ),
        )
        with self.assertRaisesRegex(GuardError, "replacement source"):
            self.validate(mutated)

    def test_mutation_replacement_test_attribute_removed_fails(self) -> None:
        path = "crates/norito/tests/aos_ncb_more_golden.rs"
        source = self.snapshot.files[path]
        assert source is not None
        mutated = _mutate(
            self.snapshot,
            path,
            source.replace(
                b"#[test]\nfn ncb_enum_offsets_code_delta_variant1_fixture()",
                b"fn ncb_enum_offsets_code_delta_variant1_fixture()",
                1,
            ),
        )
        with self.assertRaisesRegex(GuardError, "replacement source"):
            self.validate(mutated)

    def test_mutation_replacement_test_ignore_added_fails(self) -> None:
        path = "crates/norito/tests/aos_ncb_more_golden.rs"
        source = self.snapshot.files[path]
        assert source is not None
        mutated = _mutate(
            self.snapshot,
            path,
            source.replace(
                b"#[test]\nfn ncb_enum_offsets_code_delta_variant1_fixture()",
                b"#[test]\n#[ignore]\nfn ncb_enum_offsets_code_delta_variant1_fixture()",
                1,
            ),
        )
        with self.assertRaisesRegex(GuardError, "replacement source"):
            self.validate(mutated)

    def test_mutation_replacement_test_false_cfg_added_fails(self) -> None:
        path = "crates/norito/tests/aos_ncb_more_golden.rs"
        source = self.snapshot.files[path]
        assert source is not None
        mutated = _mutate(
            self.snapshot,
            path,
            source.replace(
                b"#[test]\nfn ncb_enum_offsets_code_delta_variant2_fixture()",
                b"#[test]\n#[cfg(any())]\nfn ncb_enum_offsets_code_delta_variant2_fixture()",
                1,
            ),
        )
        with self.assertRaisesRegex(GuardError, "replacement source"):
            self.validate(mutated)

    def test_mutation_variant_assertions_removed_fails(self) -> None:
        path = "crates/norito/tests/aos_ncb_more_golden.rs"
        source = self.snapshot.files[path]
        assert source is not None
        mutated_source = source.replace(
            b'    assert_eq!(bytes, fix, "offsets+code-delta variant1 bytes mismatch");\n',
            b"",
            1,
        ).replace(
            b'    assert_eq!(bytes, fix, "offsets+code-delta variant2 bytes mismatch");\n',
            b"",
            1,
        )
        mutated = _mutate(self.snapshot, path, mutated_source)
        with self.assertRaisesRegex(GuardError, "replacement source"):
            self.validate(mutated)

    def test_mutation_extra_example_fails(self) -> None:
        mutated = Snapshot(
            files=dict(self.snapshot.files),
            example_targets=tuple(
                sorted(
                    (
                        *self.snapshot.example_targets,
                        ("extra_example", "crates/norito/examples/extra_example.rs"),
                    )
                )
            ),
            consumer_hits=self.snapshot.consumer_hits,
        )
        with self.assertRaisesRegex(GuardError, "autoexample target ledger"):
            self.validate(mutated)

    def test_mutation_nested_retired_example_resurrection_fails(self) -> None:
        mutated = Snapshot(
            files=dict(self.snapshot.files),
            example_targets=tuple(
                sorted(
                    (
                        *self.snapshot.example_targets,
                        (
                            "repro_vecdeque",
                            "crates/norito/examples/repro_vecdeque/main.rs",
                        ),
                    )
                )
            ),
            consumer_hits=self.snapshot.consumer_hits,
        )
        with self.assertRaisesRegex(GuardError, "autoexample target ledger"):
            self.validate(mutated)


if __name__ == "__main__":
    unittest.main()
