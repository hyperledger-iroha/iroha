#!/usr/bin/env python3
"""Verify the sealed Kotodama compiler test-source fixture inventories."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
import sys
from typing import Any, Iterable, Optional


FORMAT = "iroha.kotodama.test-sources"
SCHEMA_VERSION = 1
DEFAULT_MANIFEST = Path("crates/kotodama_lang/kotodama_fixtures_v1.manifest.json")
LEGACY_FORMAT = "iroha.kotodama.legacy-test-sources"
LEGACY_SCHEMA_VERSION = 1
LEGACY_ORIGIN_MERGE = "58a8040dc1726359edfdc72759b73ea3649ae38c"
DEFAULT_LEGACY_MANIFEST = Path(
    "crates/kotodama_lang/kotodama_legacy_test_sources_v1.manifest.json"
)
EXPECTED_LEGACY_FIXTURE_COUNT = 52
EXPECTED_LEGACY_SOURCES = (
    "crates/kotodama_lang/src/semantic.rs",
    "crates/kotodama_lang/src/ir.rs",
    "crates/kotodama_lang/src/ir_tail_tests.rs",
)
EXPECTED_LEGACY_DIRECTORIES = (
    "crates/kotodama_lang/src/semantic/test_sources",
    "crates/kotodama_lang/src/ir/test_sources",
)
EXPECTED_SOURCES = (
    "crates/kotodama_lang/src/compiler.rs",
    "crates/kotodama_lang/src/semantic.rs",
    "crates/kotodama_lang/src/ir.rs",
)
EXPECTED_FIXTURE_COUNT = 248
EXPECTED_TEMPLATE_COUNT = 60
ROOT_KEYS = frozenset(
    {
        "format",
        "schema_version",
        "fixtures_sha256",
        "retained_templates_sha256",
        "test_inventory_sha256",
        "source_files",
        "fixtures",
        "retained_templates",
    }
)
SOURCE_KEYS = frozenset(
    {"path", "fixture_directory", "test_names", "test_names_sha256"}
)
COMMON_LITERAL_KEYS = frozenset(
    {
        "ordinal",
        "owner_source",
        "owner_function",
        "owner_is_test",
        "source_line_before_migration",
        "raw_hashes",
        "byte_len",
        "newline_count",
        "starts_with_lf",
        "ends_with_lf",
        "content_sha256",
        "raw_literal_sha256",
    }
)
FIXTURE_KEYS = COMMON_LITERAL_KEYS | frozenset({"asset", "include_path"})
LEGACY_ROOT_KEYS = frozenset(
    {
        "format",
        "schema_version",
        "origin_merge",
        "inventory_sha256",
        "source_files",
        "fixtures",
    }
)
LEGACY_FIXTURE_KEYS = frozenset({"path", "byte_len", "content_sha256"})
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
RAW_STRING_RE = re.compile(r'(?<![A-Za-z0-9_])r(?P<hashes>#{0,255})"')
INCLUDE_STR_RE = re.compile(r'include_str!\(\s*"(?P<path>[^"]+)"\s*\)')
FUNCTION_RE = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\([^\n)]*\))?[ \t]+)?"
    r"(?:async[ \t]+)?(?:unsafe[ \t]+)?fn[ \t]+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)[^;{]*\{"
)
TEST_FUNCTION_RE = re.compile(
    r"(?m)^[ \t]*#\[test\][ \t]*\n"
    r"(?:[ \t]*#\[[^\n]+\][ \t]*\n)*"
    r"[ \t]*(?:pub(?:\([^\n)]*\))?[ \t]+)?fn[ \t]+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)


class ValidationError(ValueError):
    """Raised when the fixture inventory is incomplete or inconsistent."""


@dataclass(frozen=True)
class FunctionSpan:
    """One Rust function in a masked source file."""

    name: str
    start: int
    end: int
    is_test: bool


@dataclass(frozen=True)
class RawLiteral:
    """One multiline Rust raw-string literal."""

    start: int
    end: int
    hashes: int
    content: str


@dataclass(frozen=True)
class ValidationStats:
    """Summary returned after a successful validation."""

    fixtures: int
    legacy_fixtures: int
    retained_templates: int
    tests: int


def _fail(message: str) -> None:
    raise ValidationError(message)


def _exact_keys(value: Any, expected: frozenset[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be a JSON object")
    actual = frozenset(value)
    if actual != expected:
        _fail(
            f"{label} keys differ: missing={sorted(expected - actual)}, "
            f"unknown={sorted(actual - expected)}"
        )
    return value


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        _fail(f"{label} must be a non-empty string")
    return value


def _integer(value: Any, label: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _boolean(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        _fail(f"{label} must be a boolean")
    return value


def _sha256(value: Any, label: str) -> str:
    digest = _string(value, label)
    if SHA256_RE.fullmatch(digest) is None:
        _fail(f"{label} must be a lowercase SHA-256 digest")
    return digest


def _relative_path(value: Any, label: str) -> str:
    raw = _string(value, label)
    if "\\" in raw:
        _fail(f"{label} must use POSIX separators")
    path = PurePosixPath(raw)
    if path.is_absolute() or ".." in path.parts or path.as_posix() != raw:
        _fail(f"{label} must be a canonical repository-relative path")
    return raw


def _digest_json(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _mask_rust(source: str) -> str:
    """Blank comments and literals while preserving byte offsets and newlines."""

    masked = list(source)

    def blank(start: int, end: int) -> None:
        for index in range(start, end):
            if masked[index] != "\n":
                masked[index] = " "

    cursor = 0
    while cursor < len(source):
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            end = len(source) if end < 0 else end
            blank(cursor, end)
            cursor = end
            continue
        if source.startswith("/*", cursor):
            start = cursor
            depth = 1
            cursor += 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                _fail("unterminated Rust block comment")
            blank(start, cursor)
            continue
        raw = RAW_STRING_RE.match(source, cursor)
        if raw is not None:
            marker = '"' + raw.group("hashes")
            end = source.find(marker, raw.end())
            if end < 0:
                _fail("unterminated Rust raw string")
            end += len(marker)
            blank(cursor, end)
            cursor = end
            continue
        quote = cursor + 1 if source.startswith('b"', cursor) else cursor
        if quote < len(source) and source[quote] == '"':
            start = cursor
            cursor = quote + 1
            while cursor < len(source):
                if source[cursor] == "\\":
                    cursor += 2
                elif source[cursor] == '"':
                    cursor += 1
                    break
                else:
                    cursor += 1
            else:
                _fail("unterminated Rust string")
            blank(start, cursor)
            continue
        if source[cursor] == "'" and cursor + 2 < len(source):
            end = cursor + (3 if source[cursor + 1] == "\\" else 2)
            if end < len(source) and source[end] == "'":
                blank(cursor, end + 1)
                cursor = end + 1
                continue
        cursor += 1
    return "".join(masked)


def _raw_literals(source: str) -> list[RawLiteral]:
    literals: list[RawLiteral] = []
    cursor = 0
    while match := RAW_STRING_RE.search(source, cursor):
        hashes = match.group("hashes")
        marker = '"' + hashes
        end = source.find(marker, match.end())
        if end < 0:
            _fail("unterminated Rust raw string")
        content = source[match.end():end]
        literal_end = end + len(marker)
        if content.count("\n") + 1 > 1:
            literals.append(
                RawLiteral(match.start(), literal_end, len(hashes), content)
            )
        cursor = literal_end
    return literals


def _function_spans(source: str) -> list[FunctionSpan]:
    masked = _mask_rust(source)
    test_starts = {
        match.start("name"): match.group("name")
        for match in TEST_FUNCTION_RE.finditer(masked)
    }
    spans: list[FunctionSpan] = []
    for match in FUNCTION_RE.finditer(masked):
        brace = masked.find("{", match.start(), match.end())
        depth = 0
        cursor = brace
        while cursor < len(masked):
            if masked[cursor] == "{":
                depth += 1
            elif masked[cursor] == "}":
                depth -= 1
                if depth == 0:
                    break
            cursor += 1
        if depth:
            _fail(f"unterminated Rust function `{match.group('name')}`")
        name_start = match.start("name")
        spans.append(
            FunctionSpan(
                name=match.group("name"),
                start=match.start(),
                end=cursor + 1,
                is_test=name_start in test_starts,
            )
        )
    return spans


def _owner(spans: Iterable[FunctionSpan], position: int) -> FunctionSpan:
    owners = [span for span in spans if span.start <= position < span.end]
    if not owners:
        _fail(f"no Rust function owns source byte {position}")
    return min(owners, key=lambda span: span.end - span.start)


def _validate_common_literal(entry: dict[str, Any], label: str) -> None:
    _integer(entry["ordinal"], f"{label}.ordinal", minimum=1)
    _relative_path(entry["owner_source"], f"{label}.owner_source")
    _string(entry["owner_function"], f"{label}.owner_function")
    _boolean(entry["owner_is_test"], f"{label}.owner_is_test")
    _integer(
        entry["source_line_before_migration"],
        f"{label}.source_line_before_migration",
        minimum=1,
    )
    _integer(entry["raw_hashes"], f"{label}.raw_hashes", minimum=1)
    _integer(entry["byte_len"], f"{label}.byte_len", minimum=1)
    _integer(entry["newline_count"], f"{label}.newline_count", minimum=1)
    _boolean(entry["starts_with_lf"], f"{label}.starts_with_lf")
    _boolean(entry["ends_with_lf"], f"{label}.ends_with_lf")
    _sha256(entry["content_sha256"], f"{label}.content_sha256")
    _sha256(entry["raw_literal_sha256"], f"{label}.raw_literal_sha256")


def _regular_bytes(path: Path, label: str) -> bytes:
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        _fail(f"{label} is not a regular file")
    return path.read_bytes()


def _validate_legacy_manifest(root: Path, manifest_path: Path) -> int:
    """Validate the recovered legacy test-source assets and their Rust includes."""

    try:
        payload: Any = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValidationError(f"failed to read legacy manifest: {error}") from error
    payload = _exact_keys(payload, LEGACY_ROOT_KEYS, "legacy_manifest")
    if payload["format"] != LEGACY_FORMAT:
        _fail(f"legacy manifest format must be {LEGACY_FORMAT!r}")
    if payload["schema_version"] != LEGACY_SCHEMA_VERSION:
        _fail(f"legacy manifest schema_version must be {LEGACY_SCHEMA_VERSION}")
    if payload["origin_merge"] != LEGACY_ORIGIN_MERGE:
        _fail(f"legacy manifest origin_merge must be {LEGACY_ORIGIN_MERGE}")

    source_files = payload["source_files"]
    fixtures = payload["fixtures"]
    if source_files != list(EXPECTED_LEGACY_SOURCES):
        _fail(
            "legacy manifest source_files must be exactly "
            f"{list(EXPECTED_LEGACY_SOURCES)} in order"
        )
    if not isinstance(fixtures, list) or len(fixtures) != EXPECTED_LEGACY_FIXTURE_COUNT:
        _fail(
            "legacy manifest fixtures must contain exactly "
            f"{EXPECTED_LEGACY_FIXTURE_COUNT} entries"
        )
    inventory = {"fixtures": fixtures, "source_files": source_files}
    if (
        _sha256(payload["inventory_sha256"], "legacy_manifest.inventory_sha256")
        != _digest_json(inventory)
    ):
        _fail("legacy manifest inventory_sha256 does not authenticate the inventory")

    expected_paths: list[str] = []
    allowed_directories = frozenset(EXPECTED_LEGACY_DIRECTORIES)
    for index, raw_entry in enumerate(fixtures):
        label = f"legacy_manifest.fixtures[{index}]"
        entry = _exact_keys(raw_entry, LEGACY_FIXTURE_KEYS, label)
        path = _relative_path(entry["path"], f"{label}.path")
        if PurePosixPath(path).parent.as_posix() not in allowed_directories:
            _fail(f"{label}.path is outside the legacy fixture directories")
        if not path.endswith(".ko"):
            _fail(f"{label}.path must name a .ko source fixture")
        expected_paths.append(path)
        asset = root / path
        try:
            data = _regular_bytes(asset, path)
        except FileNotFoundError:
            _fail(f"legacy fixture is missing: {path}")
        if len(data) != _integer(entry["byte_len"], f"{label}.byte_len", minimum=1):
            _fail(f"{path} byte length changed")
        if hashlib.sha256(data).hexdigest() != _sha256(
            entry["content_sha256"], f"{label}.content_sha256"
        ):
            _fail(f"{path} content hash changed")

    if expected_paths != sorted(expected_paths) or len(expected_paths) != len(
        set(expected_paths)
    ):
        _fail("legacy fixture paths must be unique and sorted")

    observed_includes: list[str] = []
    for source_path in source_files:
        try:
            source = _regular_bytes(root / source_path, source_path).decode("utf-8")
        except FileNotFoundError:
            _fail(f"legacy fixture owner source is missing: {source_path}")
        source_parent = PurePosixPath(source_path).parent
        for match in INCLUDE_STR_RE.finditer(source):
            include_path = match.group("path")
            if "/test_sources/" not in include_path:
                continue
            include_path = _relative_path(include_path, "legacy include path")
            observed_includes.append((source_parent / include_path).as_posix())
    if len(observed_includes) != len(set(observed_includes)):
        _fail("legacy fixture include inventory contains duplicates")
    if set(observed_includes) != set(expected_paths):
        _fail("legacy fixture include inventory differs from the sealed manifest")

    for directory in EXPECTED_LEGACY_DIRECTORIES:
        observed_assets = {
            path.relative_to(root).as_posix()
            for path in (root / directory).glob("*.ko")
            if path.is_file()
        }
        expected_assets = {
            path
            for path in expected_paths
            if PurePosixPath(path).parent.as_posix() == directory
        }
        if observed_assets != expected_assets:
            _fail(f"legacy fixture directory membership changed under {directory}")

    return len(fixtures)


def validate_manifest(
    root: Path,
    manifest_path: Path,
    legacy_manifest_path: Optional[Path] = None,
) -> ValidationStats:
    """Validate every source reference, payload byte, and reconstruction hash."""

    root = root.resolve()
    manifest_path = manifest_path.resolve()
    try:
        payload: Any = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValidationError(f"failed to read manifest: {error}") from error
    payload = _exact_keys(payload, ROOT_KEYS, "manifest")
    if payload["format"] != FORMAT:
        _fail(f"manifest format must be {FORMAT!r}")
    if payload["schema_version"] != SCHEMA_VERSION:
        _fail(f"manifest schema_version must be {SCHEMA_VERSION}")

    source_entries = payload["source_files"]
    fixtures = payload["fixtures"]
    retained = payload["retained_templates"]
    if not isinstance(source_entries, list):
        _fail("manifest.source_files must be an array")
    if not isinstance(fixtures, list) or len(fixtures) != EXPECTED_FIXTURE_COUNT:
        _fail(f"manifest.fixtures must contain exactly {EXPECTED_FIXTURE_COUNT} entries")
    if not isinstance(retained, list) or len(retained) != EXPECTED_TEMPLATE_COUNT:
        _fail(
            "manifest.retained_templates must contain exactly "
            f"{EXPECTED_TEMPLATE_COUNT} entries"
        )
    if _sha256(payload["fixtures_sha256"], "fixtures_sha256") != _digest_json(fixtures):
        _fail("fixtures_sha256 does not authenticate the ordered fixture inventory")
    if (
        _sha256(payload["retained_templates_sha256"], "retained_templates_sha256")
        != _digest_json(retained)
    ):
        _fail("retained_templates_sha256 does not authenticate the template inventory")
    if (
        _sha256(payload["test_inventory_sha256"], "test_inventory_sha256")
        != _digest_json(source_entries)
    ):
        _fail("test_inventory_sha256 does not authenticate the test inventory")

    sources: dict[str, tuple[str, str, list[str]]] = {}
    total_tests = 0
    for index, raw_entry in enumerate(source_entries):
        label = f"source_files[{index}]"
        entry = _exact_keys(raw_entry, SOURCE_KEYS, label)
        path = _relative_path(entry["path"], f"{label}.path")
        directory = _relative_path(
            entry["fixture_directory"], f"{label}.fixture_directory"
        )
        names = entry["test_names"]
        if not isinstance(names, list) or not all(
            isinstance(name, str) and name for name in names
        ):
            _fail(f"{label}.test_names must be an array of non-empty strings")
        if len(names) != len(set(names)):
            _fail(f"{label}.test_names contains duplicates")
        names_digest = hashlib.sha256(
            ("\n".join(names) + "\n").encode("utf-8")
        ).hexdigest()
        if _sha256(entry["test_names_sha256"], f"{label}.test_names_sha256") != names_digest:
            _fail(f"{label}.test_names_sha256 mismatch")
        if path in sources:
            _fail(f"duplicate source inventory for {path}")
        sources[path] = (directory, entry["test_names_sha256"], names)
        total_tests += len(names)
    if tuple(sources) != EXPECTED_SOURCES:
        _fail(f"source inventory must be exactly {list(EXPECTED_SOURCES)} in order")

    source_text: dict[str, str] = {}
    source_spans: dict[str, list[FunctionSpan]] = {}
    include_inventory: dict[str, list[tuple[str, FunctionSpan]]] = {}
    retained_inventory: list[dict[str, Any]] = []
    for source_path, (directory, _, expected_names) in sources.items():
        path = root / source_path
        text = _regular_bytes(path, source_path).decode("utf-8")
        source_text[source_path] = text
        spans = _function_spans(text)
        source_spans[source_path] = spans
        masked = _mask_rust(text)
        observed_names = [
            match.group("name") for match in TEST_FUNCTION_RE.finditer(masked)
        ]
        if observed_names != expected_names:
            _fail(f"Rust test name/order inventory changed in {source_path}")
        expected_include_prefix = PurePosixPath(directory).relative_to(
            PurePosixPath(source_path).parent
        ).as_posix() + "/"
        includes: list[tuple[str, FunctionSpan]] = []
        for match in INCLUDE_STR_RE.finditer(text):
            include_path = match.group("path")
            if include_path.startswith(expected_include_prefix):
                includes.append((include_path, _owner(spans, match.start())))
        include_inventory[source_path] = includes
        template_ordinal = 0
        for literal in _raw_literals(text):
            template_ordinal += 1
            owner = _owner(spans, literal.start)
            content = literal.content.encode("utf-8")
            token = text[literal.start:literal.end].encode("utf-8")
            retained_inventory.append(
                {
                    "ordinal": template_ordinal,
                    "owner_source": source_path,
                    "owner_function": owner.name,
                    "owner_is_test": owner.is_test,
                    "source_line_before_migration": None,
                    "raw_hashes": literal.hashes,
                    "byte_len": len(content),
                    "newline_count": literal.content.count("\n"),
                    "starts_with_lf": literal.content.startswith("\n"),
                    "ends_with_lf": literal.content.endswith("\n"),
                    "content_sha256": hashlib.sha256(content).hexdigest(),
                    "raw_literal_sha256": hashlib.sha256(token).hexdigest(),
                }
            )

    seen_assets: set[str] = set()
    expected_by_source: dict[str, list[tuple[str, str, bool]]] = {
        source: [] for source in sources
    }
    ordinal_by_source = {source: 0 for source in sources}
    for index, raw_entry in enumerate(fixtures):
        label = f"fixtures[{index}]"
        entry = _exact_keys(raw_entry, FIXTURE_KEYS, label)
        _validate_common_literal(entry, label)
        source = entry["owner_source"]
        if source not in sources:
            _fail(f"{label}.owner_source is not declared")
        ordinal_by_source[source] += 1
        if entry["ordinal"] != ordinal_by_source[source]:
            _fail(f"{label}.ordinal is not contiguous within {source}")
        asset = _relative_path(entry["asset"], f"{label}.asset")
        include_path = _relative_path(entry["include_path"], f"{label}.include_path")
        if asset in seen_assets:
            _fail(f"duplicate fixture asset {asset}")
        seen_assets.add(asset)
        directory = sources[source][0]
        if PurePosixPath(asset).parent.as_posix() != directory:
            _fail(f"{label}.asset must be directly under {directory}")
        expected_include = PurePosixPath(asset).relative_to(
            PurePosixPath(source).parent
        ).as_posix()
        if include_path != expected_include:
            _fail(f"{label}.include_path does not resolve to its asset")
        data = _regular_bytes(root / asset, asset)
        if len(data) != entry["byte_len"]:
            _fail(f"{asset} byte length changed")
        if data.count(b"\n") != entry["newline_count"]:
            _fail(f"{asset} newline count changed")
        if data.startswith(b"\n") != entry["starts_with_lf"]:
            _fail(f"{asset} leading-LF policy changed")
        if data.endswith(b"\n") != entry["ends_with_lf"]:
            _fail(f"{asset} final-LF policy changed")
        if hashlib.sha256(data).hexdigest() != entry["content_sha256"]:
            _fail(f"{asset} content hash changed")
        hashes = b"#" * entry["raw_hashes"]
        reconstructed = b"r" + hashes + b'"' + data + b'"' + hashes
        if hashlib.sha256(reconstructed).hexdigest() != entry["raw_literal_sha256"]:
            _fail(f"{asset} no longer reconstructs its original raw literal")
        expected_by_source[source].append(
            (include_path, entry["owner_function"], entry["owner_is_test"])
        )

    for source, expected in expected_by_source.items():
        observed = [
            (path, owner.name, owner.is_test)
            for path, owner in include_inventory[source]
        ]
        if observed != expected:
            _fail(f"fixture include order/ownership changed in {source}")
        directory = root / sources[source][0]
        observed_assets = {
            path.relative_to(root).as_posix()
            for path in directory.glob("*.ko")
            if path.is_file()
        }
        expected_assets = {
            entry["asset"] for entry in fixtures if entry["owner_source"] == source
        }
        if observed_assets != expected_assets:
            _fail(f"fixture directory membership changed under {directory.relative_to(root)}")

    if len(retained_inventory) != len(retained):
        _fail("retained multiline template count changed")
    for index, raw_entry in enumerate(retained):
        label = f"retained_templates[{index}]"
        expected = _exact_keys(raw_entry, COMMON_LITERAL_KEYS, label)
        _validate_common_literal(expected, label)
        observed = retained_inventory[index]
        historical_line = expected["source_line_before_migration"]
        observed["source_line_before_migration"] = historical_line
        if observed != expected:
            _fail(f"{label} content/order/ownership changed")

    if legacy_manifest_path is None:
        legacy_manifest_path = root / DEFAULT_LEGACY_MANIFEST
    elif not legacy_manifest_path.is_absolute():
        legacy_manifest_path = root / legacy_manifest_path
    legacy_fixtures = _validate_legacy_manifest(root, legacy_manifest_path.resolve())

    return ValidationStats(
        fixtures=len(fixtures),
        legacy_fixtures=legacy_fixtures,
        retained_templates=len(retained),
        tests=total_tests,
    )


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (default: inferred from this script)",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="manifest path, relative to --root by default",
    )
    parser.add_argument(
        "--legacy-manifest",
        type=Path,
        default=DEFAULT_LEGACY_MANIFEST,
        help="legacy fixture manifest path, relative to --root by default",
    )
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    root = args.root.resolve()
    manifest = args.manifest
    if not manifest.is_absolute():
        manifest = root / manifest
    legacy_manifest = args.legacy_manifest
    if not legacy_manifest.is_absolute():
        legacy_manifest = root / legacy_manifest
    try:
        stats = validate_manifest(root, manifest, legacy_manifest)
    except (OSError, UnicodeError, ValidationError) as error:
        print(f"ERROR: Kotodama test-source validation failed: {error}", file=sys.stderr)
        return 1
    print(
        "kotodama_test_sources: "
        f"fixtures={stats.fixtures} "
        f"legacy_fixtures={stats.legacy_fixtures} "
        f"retained_templates={stats.retained_templates} tests={stats.tests}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
