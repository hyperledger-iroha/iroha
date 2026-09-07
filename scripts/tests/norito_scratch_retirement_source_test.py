#!/usr/bin/env python3
"""Check retired scratch surfaces and their retained codec regression owners.

Opening Git objects remain historical evidence, tested separately. Current
acceptance uses target registration and executable assertions, not historical
manifest, lockfile, source hashes or item order. Rust suites and authoritative
source/dependency budgets remain independent required checks.
"""

from __future__ import annotations

import hashlib
import json
import re
import stat
import subprocess
import unittest

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 uses scripts/requirements.txt.
    import tomli as tomllib

from scripts import check_norito_codec_contracts as rust
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

OPENING_MANIFEST_BLOB = "c629283d5728fec9e900563c613a7a0df4d41642"
OPENING_MANIFEST_SHA256 = "e78ebe2ef7d33c41c39419b074dfef56a59394098e1e9c7e3ac7b0d5483d1232"
OPENING_MANIFEST_BYTES = 4_778
OPENING_MANIFEST_LINES = 231

OPENING_LOCK_BLOB = "bf7633694c3f2fdca07de4d99743a09bad2daa12"
OPENING_LOCK_SHA256 = "0ddb3f3938cf32035371317100674cd1601c3cb41232237f7a7d28b3aeab6222"
OPENING_LOCK_BYTES = 315_333
OPENING_LOCK_LINES = 13_758

RETIRED_MODULE_BLOCKS = (
    '#[path = "../temp_print_nested.rs"]\nmod temp_print_nested;\n',
    '#[path = "../temp_print_small3.rs"]\nmod temp_print_small3;\n',
    '#[path = "../type_debug.rs"]\nmod type_debug;\n',
)

# The historical group removed the following independent obsolete codec target.
# Retained modules are required to remain registered, without pinning their order.
CURRENT_REMOVED_MODULE_BLOCKS = (
    *RETIRED_MODULE_BLOCKS,
    '#[path = "../sequential_roundtrip.rs"]\nmod sequential_roundtrip;\n',
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

REPLACEMENT_SOURCE_PINS = {
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
    for path, (blob, sha256, byte_count, line_count) in REPLACEMENT_SOURCE_PINS.items():
        _authenticate_blob(path, blob, sha256, byte_count, line_count)
    return group, manifest


def _manifest(manifest: str) -> dict:
    try:
        return tomllib.loads(manifest)
    except tomllib.TOMLDecodeError as error:
        raise GuardError("manifest.invalid_toml") from error


def _test_tables(manifest: str) -> tuple[tuple[str, str, tuple[str, ...]], ...]:
    tables = _manifest(manifest).get("test", [])
    _require(isinstance(tables, list) and bool(tables), "targets.test_tables_missing")
    rows = []
    for table in tables:
        _require(isinstance(table.get("name"), str) and isinstance(table.get("path"), str), "targets.test_fields")
        _require(table.get("harness", True) is True and table.get("test", True) is True, "targets.test_disabled")
        features = table.get("required-features", [])
        _require(isinstance(features, list) and all(isinstance(item, str) for item in features), "targets.test_features")
        rows.append((table["name"], table["path"], tuple(sorted(features))))
    _require(len({row[0] for row in rows}) == len(rows) and len({row[1] for row in rows}) == len(rows), "targets.test_duplicate")
    return tuple(rows)


def _module_entries(source: str) -> tuple[tuple[str, str], ...]:
    entries = []
    pattern = r'(?m)^((?:[ \t]*#\[[^\n]+\]\s*)*)[ \t]*mod\s+(\w+)\s*;'
    masked = rust._mask_non_code(source)
    for match in re.finditer(pattern, source):
        if "mod" not in masked[match.start():match.end()]:
            continue
        attrs = match.group(1)
        paths = re.findall(r'#\[path\s*=\s*"([^"\n]+)"\]', attrs)
        _require(len(paths) == 1 and not re.search(r'\b(?:cfg|cfg_attr|ignore)\b', rust.compact(attrs)), "targets.group_module_disabled")
        entries.append((match.group(2), paths[0]))
    _require(bool(entries) and len({name for name, _ in entries}) == len(entries), "targets.group_modules_invalid")
    return tuple(entries)


GROUP_ROOTS = tuple(f"crates/norito/tests/grouped/group_{index:02}.rs" for index in range(1, 7))
REPLACEMENT_GROUPS = {
    "crates/norito/tests/aos_ncb_more_golden.rs": GROUP_ROOTS[0],
    "crates/norito/tests/codec.rs": GROUP_ROOTS[0],
    "crates/norito/tests/containers_decode.rs": GROUP_ROOTS[1],
    "crates/norito/tests/ncb_enum_iter_samples.rs": GROUP_ROOTS[3],
}
REPLACEMENT_ASSERTIONS = {
    "crates/norito/tests/aos_ncb_more_golden.rs": {
        "ncb_enum_offsets_code_delta_variant1_fixture": ("letbytes=ncb::encode_ncb_u64_enum_bool(&rows,false,false,true);", "assert_eq!(bytes,fix,"),
        "ncb_enum_offsets_code_delta_variant2_fixture": ("letbytes=ncb::encode_ncb_u64_enum_bool(&rows,false,false,true);", "assert_eq!(bytes,fix,"),
    },
    "crates/norito/tests/ncb_enum_iter_samples.rs": {
        "offsets_nested_window_fixture": ("letbytes=ncb::encode_ncb_u64_enum_bool(&rows,true,false,true);", "forbin&bytes", "assert_eq!(r#gen,hex,"),
    },
    "crates/norito/tests/codec.rs": {
        "box_roundtrip": ("Box::<u32>::decode_all(&mut&bytes[..])", "Box::<String>::decode_all(&mut&bytes[..])", "assert_eq!(value,decoded);", "assert_eq!(str_box,decoded);"),
        "vecdeque_roundtrip": ("letbytes=deque.encode();", "VecDeque::<String>::decode_all(&mut&bytes[..])", "assert_eq!(deque,decoded);"),
        "binaryheap_roundtrip": ("letbytes=heap.encode();", "BinaryHeap::<u32>::decode_all(&mut&bytes[..])", "assert_eq!(heap.clone().into_sorted_vec(),decoded.into_sorted_vec());"),
    },
    "crates/norito/tests/containers_decode.rs": {
        "vecdeque_roundtrip": ("to_bytes(&vd)", "decode_from_bytes(&bytes)", "assert_eq!(vd,out);"),
        "binaryheap_roundtrip": ("to_bytes(&heap)", "decode_from_bytes(&bytes)", "assert_eq!(heap.into_sorted_vec(),out.into_sorted_vec());"),
    },
}
REPLACEMENT_FIXTURES = {
    "ncb_enum_offsets_code_delta_variant1_fixture": "tests/data/enum_offsets_code_delta_variant1.hex",
    "ncb_enum_offsets_code_delta_variant2_fixture": "tests/data/enum_offsets_code_delta_variant2.hex",
    "offsets_nested_window_fixture": "tests/data/enum_offsets_nested_window.hex",
}


def _replacement_test(source: str, path: str, name: str):
    items = [item for item in rust.functions(source) if item.name == name]
    _require(len(items) == 1, f"replacement.missing:{path}::{name}")
    item = items[0]
    attrs = re.search(r"((?:\s*#\[[^\n]*\])+\s*)$", source[:item.start])
    _require(attrs is not None and rust.has_literal_syntax(attrs.group(1), "#[test]") and not re.search(r"\b(?:cfg|cfg_attr|ignore)\b", rust.compact(attrs.group(1))), f"replacement.disabled:{path}::{name}")
    _require(not re.search(r"(?m)^#!\[cfg", rust._mask_non_code(source)), f"replacement.disabled:{path}::{name}")
    return item


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
    arguments.extend(("--", ".", ":(exclude)docs/history/**", f":(exclude){GUARD_PATH}"))
    result = _git(*arguments, check=False)
    _require(result.returncode in (0, 1), "active-consumer scan failed")
    if result.returncode == 1:
        return ()
    return tuple(line for line in result.stdout.decode("utf-8").splitlines() if line)


def _snapshot() -> Snapshot:
    manifest = _regular_bytes(MANIFEST)
    targets = _test_tables(manifest.decode())
    paths = {MANIFEST, *GROUP_ROOTS, *REPLACEMENT_MARKERS}
    paths.update(f"crates/norito/{row[1]}" for row in targets)
    paths.update(f"crates/norito/{path}" for path in REPLACEMENT_FIXTURES.values())
    files: dict[str, bytes | None] = {path: _regular_bytes(path) for path in paths}
    for pin in SOURCE_PINS:
        path = ROOT / pin.path
        files[pin.path] = _regular_bytes(pin.path) if path.exists() or path.is_symlink() else None
    return Snapshot(files, _example_targets(), _active_consumer_hits())


def _validate(snapshot: Snapshot, opening_group: bytes) -> None:
    for pin in SOURCE_PINS:
        _require(snapshot.files[pin.path] is None, f"retired.source:{pin.path}")
    _require(not snapshot.consumer_hits, "retired.active_consumer")
    names = [name for name, _ in snapshot.example_targets]
    _require(len(names) == len(set(names)), "targets.example_duplicate")
    _require(not set(names).intersection(RETIRED_IDENTIFIERS), "retired.example_target")

    manifest = snapshot.files[MANIFEST]
    _require(manifest is not None, "manifest.missing")
    document = _manifest(manifest.decode())
    package = document.get("package", {})
    _require(package.get("autotests") is False, "targets.autotests_must_be_disabled")
    _require(package.get("autoexamples", True) is True, "targets.autoexamples_must_be_enabled")
    criterion = document.get("dev-dependencies", {}).get("criterion", {})
    _require(isinstance(criterion, dict) and criterion.get("workspace") is True, "manifest.criterion_bench_dependency")
    tables = _test_tables(manifest.decode())
    required = (*EXPECTED_TEST_TABLES, ("json_object_key_allocations", "tests/json_object_key_allocations.rs", ("json",)))
    _require(set(required).issubset(tables), "targets.required_test_registration")
    for name, path, _features in tables:
        _require(Path(path).is_relative_to("tests") and ".." not in Path(path).parts, "targets.test_path")
        _require(snapshot.files.get(f"crates/norito/{path}") is not None, f"targets.test_source:{name}")
    for table in document.get("example", []):
        _require(table.get("name") not in RETIRED_IDENTIFIERS, "retired.example_target")
    _require(set(EXPECTED_EXAMPLE_TARGETS).issubset(snapshot.example_targets), "targets.retained_example_missing")

    groups = {}
    for path in GROUP_ROOTS:
        source = snapshot.files[path]
        _require(source is not None, f"targets.group_missing:{path}")
        _require(not re.search(r"(?m)^#!\[cfg", rust._mask_non_code(source.decode())), f"targets.group_disabled:{path}")
        groups[path] = _module_entries(source.decode())
    current = [entry for entries in groups.values() for entry in entries]
    _require(len(current) == len({name for name, _ in current}) == len({path for _, path in current}), "targets.group_duplicate")
    retired_modules = {Path(pin.path).stem for pin in SOURCE_PINS}
    retired_modules.add("sequential_roundtrip")
    required_modules = [entry for entry in _module_entries(opening_group.decode()) if entry[0] not in retired_modules]
    _require(all(entry in current for entry in required_modules), "targets.retained_group_module_missing")
    _require(not any(name in retired_modules for name, _ in current), "retired.group_module")
    for path, group in REPLACEMENT_GROUPS.items():
        entry = (Path(path).stem, "../" + Path(path).name)
        _require(entry in groups[group], f"replacement.registration:{path}")

    for path, contracts in REPLACEMENT_ASSERTIONS.items():
        source = snapshot.files[path]
        _require(source is not None, f"replacement.source:{path}")
        text = source.decode()
        for name, assertions in contracts.items():
            item = _replacement_test(text, path, name)
            _require(all(re.search(r"(?<![\w])" + re.escape(assertion), item.code) for assertion in assertions), f"replacement.assertions:{path}::{name}")
            if name in REPLACEMENT_FIXTURES:
                fixture = REPLACEMENT_FIXTURES[name]
                _require(rust.has_literal_syntax(item.raw_body, ('read_hex_fixture' if name.startswith('ncb_enum_') else '.join') + '("' + fixture + '")'), f"replacement.fixture:{name}")
                payload = snapshot.files.get("crates/norito/" + fixture)
                _require(payload is not None and re.fullmatch(rb"[0-9a-fA-F\s]+", payload) is not None and bool(bytes.fromhex(payload.decode())), f"replacement.fixture_data:{name}")


def _mutate(snapshot: Snapshot, path: str, data: bytes | None) -> Snapshot:
    files = dict(snapshot.files)
    files[path] = data
    return Snapshot(
        files=files,
        example_targets=snapshot.example_targets,
        consumer_hits=snapshot.consumer_hits,
    )


class NoritoScratchRetirementSourceTest(unittest.TestCase):
    """Each negative first proves the actual current-source baseline passes."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.opening_group, cls.opening_manifest = _authenticate_openings()
        cls.snapshot = _snapshot()

    def validate(self, snapshot: Snapshot) -> None:
        _validate(snapshot, self.opening_group)

    def assert_rejected(self, changed: Snapshot, diagnostic: str) -> None:
        self.validate(self.snapshot)
        self.assertNotEqual(changed, self.snapshot, "mutation must change the input")
        with self.assertRaises(GuardError) as raised:
            self.validate(changed)
        self.assertEqual(str(raised.exception), diagnostic)

    def changed(self, path: str, old: bytes, new: bytes) -> Snapshot:
        source = self.snapshot.files[path]
        self.assertIsNotNone(source)
        self.assertIn(old, source, "mutation target must exist")
        return _mutate(self.snapshot, path, source.replace(old, new, 1))

    def test_retirement_contract(self) -> None:
        self.validate(self.snapshot)

    def test_historical_preimages_remain_authenticated(self) -> None:
        self.assertEqual(_authenticate_openings(), (self.opening_group, self.opening_manifest))

    def test_retired_surfaces_rejected(self) -> None:
        self.assert_rejected(_mutate(self.snapshot, SOURCE_PINS[0].path, b"#[test]\nfn restored() {}\n"), f"retired.source:{SOURCE_PINS[0].path}")
        self.assert_rejected(Snapshot(self.snapshot.files, self.snapshot.example_targets, ("README.md:1:repro_vecdeque",)), "retired.active_consumer")
        self.assert_rejected(Snapshot(self.snapshot.files, (*self.snapshot.example_targets, ("repro_vecdeque", "crates/norito/examples/repro_vecdeque/main.rs")), ()), "retired.example_target")
        group = self.snapshot.files[GROUP_ROOT]
        self.assert_rejected(_mutate(self.snapshot, GROUP_ROOT, group + RETIRED_MODULE_BLOCKS[0].encode()), "retired.group_module")

    def test_target_registration_mutations_rejected(self) -> None:
        cases = (
            (b"autotests = false", b"autotests = true", "targets.autotests_must_be_disabled"),
            (b"autotests = false", b"autotests = false\nautoexamples = false # disabled", "targets.autoexamples_must_be_enabled"),
            (b'name = "norito_group_05"', b'name = "unregistered_group"', "targets.required_test_registration"),
            (b'name = "norito_group_05"', b'name = "norito_group_05"\nharness = false', "targets.test_disabled"),
            (b"criterion = { workspace = true }\n", b"", "manifest.criterion_bench_dependency"),
            (b'name = "json_object_key_allocations"', b'name = "removed_allocation_contract"', "targets.required_test_registration"),
        )
        for old, new, diagnostic in cases:
            with self.subTest(diagnostic=diagnostic):
                self.assert_rejected(self.changed(MANIFEST, old, new), diagnostic)
        self.assert_rejected(self.changed(GROUP_ROOT, b"mod transport_capabilities;", b"mod lost_transport_capabilities;"), "targets.retained_group_module_missing")
        path = "crates/norito/tests/containers_decode.rs"
        group = REPLACEMENT_GROUPS[path]
        self.assert_rejected(self.changed(group, b'#[path = "../containers_decode.rs"]\nmod containers_decode;\n', b""), f"replacement.registration:{path}")
        self.assert_rejected(self.changed(group, b'mod containers_decode;', b'#[cfg(any())]\nmod containers_decode;'), "targets.group_module_disabled")

    def test_group_roots_and_unique_module_ownership_are_enforced(self) -> None:
        path = GROUP_ROOTS[0]
        source = self.snapshot.files[path]
        self.assert_rejected(_mutate(self.snapshot, path, b"#![cfg(any())]\n" + source), f"targets.group_disabled:{path}")
        self.assert_rejected(_mutate(self.snapshot, path, source + b'#[path = "../codec.rs"]\nmod duplicate_codec;\n'), "targets.group_duplicate")

    def test_replacement_tests_and_assertions_cannot_be_disabled(self) -> None:
        for path, contracts in REPLACEMENT_ASSERTIONS.items():
            for name in contracts:
                old = f"#[test]\nfn {name}".encode()
                for replacement in (f"fn {name}", f"#[test]\n#[ignore]\nfn {name}", f"#[test]\n#[cfg(any())]\nfn {name}"):
                    with self.subTest(path=path, name=name, replacement=replacement):
                        self.assert_rejected(self.changed(path, old, replacement.encode()), f"replacement.disabled:{path}::{name}")
        path = "crates/norito/tests/aos_ncb_more_golden.rs"
        name = "ncb_enum_offsets_code_delta_variant1_fixture"
        self.assert_rejected(self.changed(path, f"fn {name}()".encode(), b"fn removed_fixture()"), f"replacement.missing:{path}::{name}")
        for number in (1, 2):
            name = f"ncb_enum_offsets_code_delta_variant{number}_fixture"
            old = f'assert_eq!(bytes, fix, "offsets+code-delta variant{number} bytes mismatch");'.encode()
            self.assert_rejected(self.changed(path, old, b"/* " + old + b" */"), f"replacement.assertions:{path}::{name}")
        for path, contracts in REPLACEMENT_ASSERTIONS.items():
            for name in contracts:
                source = self.snapshot.files[path].decode()
                item = _replacement_test(source, path, name)
                body = item.raw_body
                self.assertIn("assert_eq!", body)
                changed = source[:item.opening + 1] + body.replace("assert_eq!", "debug_assert_eq!") + source[item.end:]
                self.assert_rejected(_mutate(self.snapshot, path, changed.encode()), f"replacement.assertions:{path}::{name}")

    def test_fixture_disconnection_rejected(self) -> None:
        path = "crates/norito/tests/aos_ncb_more_golden.rs"
        name = "ncb_enum_offsets_code_delta_variant1_fixture"
        old = REPLACEMENT_FIXTURES[name].encode()
        self.assert_rejected(self.changed(path, old, b"tests/data/wrong.hex"), f"replacement.fixture:{name}")
        source = self.snapshot.files[path]
        call = b'read_hex_fixture("' + old + b'")'
        self.assert_rejected(_mutate(self.snapshot, path, source.replace(call, b'read_hex_fixture("tests/data/wrong.hex") /* ' + call + b' */', 1)), f"replacement.fixture:{name}")
        self.assert_rejected(_mutate(self.snapshot, "crates/norito/" + old.decode(), b"not hex"), f"replacement.fixture_data:{name}")

    def test_unrelated_growth_and_module_order_are_not_historical_pins(self) -> None:
        self.validate(self.snapshot)
        manifest = self.snapshot.files[MANIFEST]
        changed = _mutate(self.snapshot, MANIFEST, manifest + b'\n[package.metadata.retirement_test]\nnote = "unrelated package metadata"\n')
        self.assertNotEqual(changed, self.snapshot)
        self.validate(changed)
        group = self.snapshot.files[GROUP_ROOT]
        first = b'#[path = "../decode_sequence_limits.rs"]\nmod decode_sequence_limits;\n'
        self.assertIn(first, group)
        self.validate(_mutate(self.snapshot, GROUP_ROOT, group.replace(first, b"", 1) + first))
        added = Snapshot(self.snapshot.files, (*self.snapshot.example_targets, ("new_example", "crates/norito/examples/new_example.rs")), ())
        self.assertNotEqual(added, self.snapshot)
        self.validate(added)


if __name__ == "__main__":
    unittest.main()
