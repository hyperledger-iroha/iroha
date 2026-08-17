#!/usr/bin/env python3
"""Guard typed Iroha CLI compaction against authenticated donor blobs."""

from __future__ import annotations

import difflib
import hashlib
import re
import subprocess
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MINIMUM_RUST_LINE_REDUCTION = 1_500


@dataclass(frozen=True)
class SourcePin:
    path: Path
    preimage_blob: str
    line_ceiling: int


PINS = (
    SourcePin(
        Path("crates/iroha_cli/src/commands/sorafs.rs"),
        "014e2952c15d131913e4f0e27a7c4859cb40fae2",
        24_547,
    ),
    SourcePin(
        Path("crates/iroha_cli/src/soracloud.rs"),
        "45d8d3fb7fcef23256f65780569caffe4929c8cf",
        26_499,
    ),
    SourcePin(
        Path("crates/iroha_cli/src/main_shared.rs"),
        "3212c23a6ecc8cd31549ebc619228eb85de7c4a0",
        10_601,
    ),
)

SORAFS_EMITTER = """macro_rules! impl_run_with_client_methods {
    ($args:ty, $($method:path),+ $(,)?) => {
        impl Run for $args {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                self.run_with(context, $($method),+)
            }
        }
    };
}

"""
SORACLOUD_REGION_SHA256 = (
    "22c6fdd9271bb905c2f7cd1a3b9d827c76862ac9ffa3d98cc2b5536d7cfb5c76"
)
MAIN_SHARED_REGION_SHA256 = (
    "235a6d57b5bfaf88d1a61b5937d19e319466187a658c0513b5886347b281b564"
)

DIRECT_TEST = re.compile(
    r"(?P<attrs>(?:^[ \t]*#\[[^\n]+\]\n)+)"
    r"^[ \t]*(?:async\s+)?fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\(",
    re.MULTILINE,
)
GENERATED_SORACLOUD_TEST = re.compile(
    r"^[ \t]*manifest_pair_(?:submission|status)_service_name_case!\(\s*\n?"
    r"[ \t]*(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*,",
    re.MULTILINE,
)
DIRECT_RUN_IMPL = re.compile(
    r"^impl Run for (?P<name>[A-Za-z_][A-Za-z0-9_]*) \{\n"
    r"(?P<body>.*?)^\}\n",
    re.MULTILINE | re.DOTALL,
)
RUN_ROW = re.compile(
    r"^impl_run_with_client_methods!\((?P<body>.*?)\);\n",
    re.MULTILINE | re.DOTALL,
)
FORBIDDEN_ADDITION = re.compile(
    r"dyn\s+Fn|FnMut|FnOnce|\bfn\s*\(|\$(?:body|setup)|"
    r"\b(?:Action|Step)\b|include_(?:str|bytes)!"
)


def _git(*arguments: str) -> str:
    return subprocess.check_output(
        ["git", *arguments], cwd=ROOT, text=True, encoding="utf-8"
    )


def _region(source: str, start_marker: str, end_marker: str) -> str:
    if source.count(start_marker) != 1:
        raise AssertionError(f"region start marker count changed: {start_marker!r}")
    start = source.index(start_marker)
    end = source.find(end_marker, start)
    if end < 0:
        raise AssertionError(f"region end marker missing: {end_marker!r}")
    return source[start:end]


def _test_inventory(source: str, generated: bool) -> tuple[tuple[str, tuple[str, ...]], ...]:
    cases: list[tuple[int, str, tuple[str, ...]]] = []
    for match in DIRECT_TEST.finditer(source):
        attributes = tuple(line.strip() for line in match.group("attrs").splitlines())
        if "#[test]" in attributes:
            cases.append((match.start(), match.group("name"), attributes))
    if generated:
        for match in GENERATED_SORACLOUD_TEST.finditer(source):
            cases.append((match.start(), match.group("name"), ("#[test]",)))
    cases.sort()
    inventory = tuple((name, attributes) for _, name, attributes in cases)
    return inventory


def _old_sorafs_run_rows(source: str) -> dict[str, tuple[str, ...]]:
    rows: dict[str, tuple[str, ...]] = {}
    for match in DIRECT_RUN_IMPL.finditer(source):
        body = match.group("body")
        if "self.run_with" not in body:
            continue
        name = match.group("name")
        rows[name] = tuple(re.findall(r"Client::([A-Za-z_][A-Za-z0-9_]*)", body))
    return rows


def _new_sorafs_run_rows(source: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    rows = []
    for match in RUN_ROW.finditer(source):
        columns = [column.strip() for column in match.group("body").split(",")]
        columns = [column for column in columns if column]
        if len(columns) < 2 or any(
            not method.startswith("Client::") for method in columns[1:]
        ):
            raise AssertionError("malformed typed Run row")
        rows.append(
            (columns[0], tuple(method.removeprefix("Client::") for method in columns[1:]))
        )
    if len(rows) != 53 or len({name for name, _ in rows}) != len(rows):
        raise AssertionError("typed Run row inventory changed")
    return tuple(rows)


def _added_lines(indexed: str, current: str) -> tuple[str, ...]:
    return tuple(
        line[2:]
        for line in difflib.ndiff(indexed.splitlines(), current.splitlines())
        if line.startswith("+ ")
    )


def _assertion_heads(source: str) -> tuple[str, ...]:
    region = _region(
        source,
        '#[cfg(all(test, feature = "cli_integration_harness"))]\n'
        "mod cli_integration_harness_tests {\n",
        "// Experimental: feature-gated integration harness for CLI queries.\n",
    )
    prefixes = ("assert!", "assert_eq!", "assert_ne!", "debug_assert!")
    return tuple(
        line.strip()
        for line in region.splitlines()
        if line.lstrip().startswith(prefixes)
    )


def _skip_quoted(source: str, start: int, quote: str) -> int:
    index = start + 1
    while index < len(source):
        if source[index] == "\\":
            index += 2
        elif source[index] == quote:
            return index + 1
        elif source[index] == "\n" and quote == "'":
            raise AssertionError("unterminated Rust character literal")
        else:
            index += 1
    raise AssertionError("unterminated Rust quoted literal")


def _raw_string_end(source: str, start: int) -> int | None:
    prefix = None
    for candidate in ("br", "rb", "r"):
        if source.startswith(candidate, start):
            prefix = candidate
            break
    if prefix is None:
        return None
    index = start + len(prefix)
    hashes = 0
    while index < len(source) and source[index] == "#":
        hashes += 1
        index += 1
    if index >= len(source) or source[index] != '"':
        return None
    terminator = '"' + "#" * hashes
    end = source.find(terminator, index + 1)
    if end < 0:
        raise AssertionError("unterminated Rust raw string")
    return end + len(terminator)


def _assert_balanced_rust_delimiters(source: str) -> None:
    pairs = {"(": ")", "[": "]", "{": "}"}
    closing = set(pairs.values())
    stack: list[tuple[str, int]] = []
    index = 0
    block_comment_depth = 0
    while index < len(source):
        if block_comment_depth:
            if source.startswith("/*", index):
                block_comment_depth += 1
                index += 2
            elif source.startswith("*/", index):
                block_comment_depth -= 1
                index += 2
            else:
                index += 1
            continue
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", index):
            block_comment_depth = 1
            index += 2
            continue
        raw_end = _raw_string_end(source, index)
        if raw_end is not None:
            index = raw_end
            continue
        character = source[index]
        if character == '"':
            index = _skip_quoted(source, index, '"')
            continue
        if character == "'":
            lifetime = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", source[index:])
            if lifetime is not None:
                after = index + len(lifetime.group(0))
                if after >= len(source) or source[after] != "'":
                    index = after
                    continue
            index = _skip_quoted(source, index, "'")
            continue
        if character in pairs:
            stack.append((character, index))
        elif character in closing:
            if not stack or pairs[stack[-1][0]] != character:
                raise AssertionError(f"unbalanced Rust delimiter at byte {index}")
            stack.pop()
        index += 1
    if block_comment_depth:
        raise AssertionError("unterminated Rust block comment")
    if stack:
        raise AssertionError(f"unclosed Rust delimiter at byte {stack[-1][1]}")


def _validate(
    current: dict[Path, str], indexed: dict[Path, str]
) -> None:
    reduction = 0
    for pin in PINS:
        source = current[pin.path]
        preimage = indexed[pin.path]
        lines = len(source.splitlines())
        if lines > pin.line_ceiling:
            raise AssertionError(f"{pin.path}: line ceiling exceeded")
        reduction += len(preimage.splitlines()) - lines
        _assert_balanced_rust_delimiters(source)
        additions = "\n".join(_added_lines(preimage, source))
        if FORBIDDEN_ADDITION.search(additions):
            raise AssertionError(f"{pin.path}: forbidden compaction escape hatch")
        generated = pin.path.name == "soracloud.rs"
        if _test_inventory(source, generated) != _test_inventory(preimage, False):
            raise AssertionError(f"{pin.path}: test name or attribute inventory changed")
    if reduction < MINIMUM_RUST_LINE_REDUCTION:
        raise AssertionError(f"Rust line reduction is only {reduction}")

    sorafs_path = PINS[0].path
    sorafs = current[sorafs_path]
    if sorafs.count("macro_rules! impl_run_with_client_methods") != 1:
        raise AssertionError("typed Run emitter count changed")
    emitter_start = sorafs.index("macro_rules! impl_run_with_client_methods")
    if sorafs[emitter_start : emitter_start + len(SORAFS_EMITTER)] != SORAFS_EMITTER:
        raise AssertionError("typed Run emitter changed")
    old_rows = _old_sorafs_run_rows(indexed[sorafs_path])
    for name, methods in _new_sorafs_run_rows(sorafs):
        if old_rows.get(name) != methods:
            raise AssertionError(f"typed Run row drifted: {name}")
        if re.search(rf"^impl Run for {re.escape(name)} \{{", sorafs, re.MULTILINE):
            raise AssertionError(f"direct Run impl survived typed row: {name}")

    soracloud = current[PINS[1].path]
    soracloud_region = _region(
        soracloud,
        "    macro_rules! manifest_pair_submission_service_name_case {\n",
        "    #[test]\n"
        "    fn model_upload_encryption_recipient_args_can_attach_service_plan_from_manifest_pair() {\n",
    )
    if hashlib.sha256(soracloud_region.encode()).hexdigest() != SORACLOUD_REGION_SHA256:
        raise AssertionError("SoraCloud typed manifest-pair family changed")
    if soracloud_region.count("            #[test]\n            fn $name() {") != 2:
        raise AssertionError("SoraCloud test emitters lost their exact #[test] attribute")

    main_shared = current[PINS[2].path]
    main_region = _region(
        main_shared,
        "    trait HarnessQueryFixture {\n",
        "}\n// Experimental: feature-gated integration harness for CLI queries.\n",
    )
    if hashlib.sha256(main_region.encode()).hexdigest() != MAIN_SHARED_REGION_SHA256:
        raise AssertionError("typed query-harness family changed")
    if main_region.count("HarnessQueryExecutor::<") != 19:
        raise AssertionError("typed query-harness case inventory changed")
    if _assertion_heads(main_shared) != _assertion_heads(indexed[PINS[2].path]):
        raise AssertionError("query-harness assertions differ from the index preimage")


class IrohaCliTypedCompactionSourceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.current = {pin.path: (ROOT / pin.path).read_text() for pin in PINS}
        cls.indexed = {
            pin.path: _git("cat-file", "blob", pin.preimage_blob) for pin in PINS
        }

    def test_authenticated_preimages_and_typed_expansions_are_exact(self) -> None:
        for pin in PINS:
            self.assertEqual(_git("cat-file", "-t", pin.preimage_blob).strip(), "blob")
        _validate(self.current, self.indexed)

    def test_test_name_mutation_is_rejected(self) -> None:
        changed = dict(self.current)
        changed[PINS[2].path] = changed[PINS[2].path].replace(
            "fn pagination_sorting_nfts_desc()",
            "fn pagination_sorting_nfts_reverse()",
            1,
        )
        with self.assertRaises(AssertionError):
            _validate(changed, self.indexed)

    def test_test_attribute_mutation_is_rejected(self) -> None:
        changed = dict(self.current)
        changed[PINS[2].path] = changed[PINS[2].path].replace(
            "    #[test]\n    fn pagination_sorting_nfts_desc()",
            "    #[ignore]\n    fn pagination_sorting_nfts_desc()",
            1,
        )
        with self.assertRaises(AssertionError):
            _validate(changed, self.indexed)

    def test_typed_run_method_mutation_is_rejected(self) -> None:
        changed = dict(self.current)
        changed[PINS[0].path] = changed[PINS[0].path].replace(
            "impl_run_with_client_methods!(BillingStatusArgs, Client::get_sorafs_billing_status);",
            "impl_run_with_client_methods!(BillingStatusArgs, Client::get_sorafs_billing_statements);",
            1,
        )
        with self.assertRaises(AssertionError):
            _validate(changed, self.indexed)

    def test_typed_fixture_mutation_is_rejected(self) -> None:
        changed = dict(self.current)
        changed[PINS[2].path] = changed[PINS[2].path].replace(
            "HarnessRows::RankedFive(seed) => (F::ranked_five(seed), true),",
            "HarnessRows::RankedFive(seed) => (F::positioned_five(seed), true),",
            1,
        )
        with self.assertRaises(AssertionError):
            _validate(changed, self.indexed)

    def test_callback_escape_hatch_is_rejected(self) -> None:
        changed = dict(self.current)
        changed[PINS[2].path] = changed[PINS[2].path].replace(
            "    trait HarnessQueryFixture {",
            "    // dyn Fn callback escape hatch\n    trait HarnessQueryFixture {",
            1,
        )
        with self.assertRaises(AssertionError):
            _validate(changed, self.indexed)

    def test_unbalanced_delimiter_is_rejected(self) -> None:
        changed = dict(self.current)
        changed[PINS[1].path] += "}\n"
        with self.assertRaises(AssertionError):
            _validate(changed, self.indexed)

    def test_line_budget_regression_is_rejected(self) -> None:
        changed = dict(self.current)
        changed[PINS[2].path] += "// budget regression\n" * 100
        with self.assertRaises(AssertionError):
            _validate(changed, self.indexed)


if __name__ == "__main__":
    unittest.main()
