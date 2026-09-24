"""Exact delimiter-context equivalence and bounded-work source scanner controls."""

from __future__ import annotations

import importlib.util
import random
import sys
from dataclasses import FrozenInstanceError
from pathlib import Path
from types import ModuleType

import pytest


@pytest.fixture(scope="module")
def checker() -> ModuleType:
    path = (
        Path(__file__).resolve().parents[2]
        / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
    )
    spec = importlib.util.spec_from_file_location("_delimiter_context_checker", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def eager_context(checker: ModuleType, source: str, end: int) -> tuple:
    """Evaluate the preceding eager algorithm as a small differential oracle."""
    stack = []
    matching = {')': '(', ']': '[', '}': '{'}
    last_boundary = 0
    for index, char in enumerate(source[:end]):
        if char in "([{":
            stack.append(
                (char, index, checker.rust_code_tokens(source[last_boundary:index]))
            )
            if char == "{":
                last_boundary = index + 1
        elif char in ")]}":
            if stack and stack[-1][0] == matching[char]:
                stack.pop()
            else:
                stack.append((f"unmatched:{char}", index, ()))
            if char == "}":
                last_boundary = index + 1
        elif char == ";":
            last_boundary = index + 1
    return tuple(stack)


@pytest.mark.parametrize("source", [
    "",
    "impl Owner {\nfn run<'a>(v: &'a [u8]) { call(v); }\nfn next() {}\n}",
    "a // fn fake() {\nb /* outer /* { [] */ ) */ c",
    r'''b"quote\"//" c"string" "\u{1234}" 'x' b'\x7f' '\u{01}' ''',
    '''r###" // /* \" nested "### br#"byte"# cr##"C"## code''',
    "x /* unterminated { [ (",
    'x "unterminated\\',
    'x r##"unterminated {',
    "'a 'static 'é' '😀' // Unicode界\r\nnext\n",
    "/*\n*/\n//\n'\\n' \"\n\" r#\"\n\"#",
    '#![cfg_attr(feature = "guard", cfg(any()))]\nmod m {\nfn f() {}\n}',
    'macro_rules! m { () => { fn fake() {} } }\nm!(fn other() {});',
    "proof fn p(x: int) requires if x > 0 { x > 1 } else { true }, { assert(true); }",
    "abc([)]; {) after; [tail",
])
def test_all_source_prefixes_match_eager_context(checker: ModuleType, source: str) -> None:
    structural = checker.mask_rust_comments_and_literals(source)
    for end in range(-len(structural) - 1, len(structural) + 2):
        assert checker._rust_delimiter_context(structural, end) == eager_context(
            checker, structural, end
        ), (source, end)


@pytest.mark.parametrize(("source", "expected"), [
    (")", (("unmatched:)", 0, ()),)),
    ("(]", (("(", 0, ()), ("unmatched:]", 1, ()))),
    ("([)]", (("(", 0, ()), ("[", 1, ("(",)),
                ("unmatched:)", 2, ()), ("unmatched:]", 3, ()))),
    ("{)[]", (("{", 0, ()), ("unmatched:)", 1, ()))),
    ("a;}b(", (("unmatched:}", 2, ()), ("(", 4, ("b",)))),
    ("(;)tail[", (("[", 7, (")", "tail")),)),
])
def test_malformed_stack_retains_exact_unmatched_markers(
    checker: ModuleType, source: str, expected: tuple
) -> None:
    assert checker._rust_delimiter_context(source, len(source)) == expected
    assert eager_context(checker, source, len(source)) == expected


def test_seeded_arbitrary_prefixes_match_eager_context(checker: ModuleType) -> None:
    rng = random.Random(0x49524F4841)
    alphabet = "()[]{};#'\"/\\\n abc09界"
    for _ in range(512):
        source = "".join(rng.choice(alphabet) for _ in range(rng.randrange(96)))
        # Also exercise arbitrary structural input, preserving legacy semantics
        # even when a caller supplies an incompletely masked/malformed prefix.
        for structural in (source, checker.mask_rust_comments_and_literals(source)):
            end = rng.randrange(-len(structural) - 1, len(structural) + 2)
            assert checker._rust_delimiter_context(structural, end) == eager_context(
                checker, structural, end
            )


def test_only_surviving_headers_are_tokenized(
    checker: ModuleType, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = "mod outer {\nimpl Owner {\n" + "fn done() { call([1, (2)]); }\n" * 512
    expected = eager_context(checker, source, len(source))
    original = checker.rust_code_tokens
    calls = []

    def observe(value: str) -> tuple[str, ...]:
        calls.append(value)
        return original(value)

    checker._rust_delimiter_context.cache_clear()
    monkeypatch.setattr(checker, "rust_code_tokens", observe)
    assert checker._rust_delimiter_context(source, len(source)) == expected
    assert calls == ["mod outer ", "\nimpl Owner "]
    assert checker._rust_delimiter_context(source, len(source)) == expected
    assert len(calls) == 2


def test_exact_source_and_end_cache_remains_bounded(checker: ModuleType) -> None:
    cache = checker._rust_delimiter_context
    cache.cache_clear()
    source = "impl Owner {"
    assert cache(source, len(source)) == (("{", 11, ("impl", "Owner")),)
    first = cache.cache_info()
    assert (first.hits, first.misses, first.maxsize) == (0, 1, 512)
    assert cache(source.encode().decode(), len(source)) == (("{", 11, ("impl", "Owner")),)
    assert cache(source, len(source) - 1) == ()
    assert cache(source.replace("Owner", "Other"), len(source)) == (
        ("{", 11, ("impl", "Other")),
    )
    assert (cache.cache_info().hits, cache.cache_info().misses) == (1, 3)
    for index in range(520):
        cache(f"impl Owner{index} {{", 99)
    assert cache.cache_info().currsize == 512
    before = cache.cache_info().misses
    assert cache(source, len(source)) == (("{", 11, ("impl", "Owner")),)
    assert cache.cache_info().misses == before + 1


def test_returned_item_mutation_never_contaminates_later_extraction(checker: ModuleType) -> None:
    source = "impl Owner {\n#[cfg(any())]\nfn run() {}\n}"
    (first,) = checker.rust_items(source, "run")
    expected = dict(first.__dict__)
    with pytest.raises(FrozenInstanceError):
        first.body = "replacement"
    # RustItem remains a fresh frozen dataclass per extraction, not a cached
    # mutable instance. Even deliberate __dict__ bypass cannot poison a hit.
    first.__dict__.update(name="forged", body="replacement", attributes=())
    (second,) = checker.rust_items(source, "run")
    assert first is not second
    assert second.__dict__ == expected
    with pytest.raises(TypeError):
        second.delimiter_context[0] = ("unmatched:)", 0, ())
    with pytest.raises(TypeError):
        second.delimiter_context[0][2][0] = "forged"


@pytest.mark.parametrize("hidden", [
    "/* fn run() {} */", "/* outer /* fn run() {} */ end */",
    '// fn run() {}\n', 'const X: &str = "\nfn run() {}\n";',
    'const X: &str = r###"\nfn run() {}\n"###;',
    'const X: &[u8] = br#"\nfn run() {}\n"#;',
    'const X: &str = cr##"\nfn run() {}\n"##;',
])
def test_comment_or_literal_item_stuffing_stays_rejected(
    checker: ModuleType, hidden: str
) -> None:
    errors = []
    assert checker._require_rust_item(Path("fixture.rs"), hidden, "run", errors) is None
    assert errors == [
        "fixture.rs: require exactly one real Rust/Verus function item named run; found 0"
    ]
    source = hidden + "\nfn run() {}\nfn run() {}\n"
    errors = []
    assert checker._require_rust_item(Path("fixture.rs"), source, "run", errors) is None
    assert errors == [
        "fixture.rs: require exactly one real Rust/Verus function item named run; found 2"
    ]
    items = checker.rust_items(source, "run")
    assert [item.line for item in items] == [
        hidden.count("\n") + 2, hidden.count("\n") + 3,
    ]


def test_nested_cfg_and_duplicate_owner_contexts_remain_distinct(checker: ModuleType) -> None:
    source = (
        '#![cfg_attr(feature = "ship", cfg(any()))]\n'
        'mod hidden {\n#![cfg(any())]\nimpl First {\nfn run() {}\n}\n}\n'
        'impl Second {\n#[cfg(test)]\nfn run() {}\n}\n'
    )
    first, second = checker.rust_items(source, "run")
    assert first.brace_context == (
        ("#", "!", "[", "cfg_attr", "(", "feature", "=", ",", "cfg", "(",
         "any", "(", ")", ")", ")", "]", "mod", "hidden"),
        ("#", "!", "[", "cfg", "(", "any", "(", ")", ")", "]", "impl", "First"),
    )
    assert second.brace_context == (("impl", "Second"),)
    assert first.ancestor_inner_attributes == (
        '#![cfg_attr(feature = "ship", cfg(any()))]', '#![cfg(any())]',
    )
    assert second.ancestor_inner_attributes == (
        '#![cfg_attr(feature = "ship", cfg(any()))]',
    )
    assert first.attributes == ()
    assert second.attributes == ("#[cfg(test)]",)


def test_lint_only_impl_attribute_preserves_reviewed_method_context(
    checker: ModuleType,
) -> None:
    source = (
        '#[cfg_attr(not(test), allow(dead_code, reason = "TODO: native runner cutover"))]\n'
        'impl Owner {\n    fn run(&self) {}\n}\n'
    )
    (item,) = checker.rust_items(source, "run")
    assert item.brace_context == (("impl", "Owner"),)
    assert tuple((opener, header) for opener, _, header in item.delimiter_context) == (
        ("{", ("impl", "Owner")),
    )
    errors: list[str] = []
    assert checker._require_qualified_rust_item(
        Path("fixture.rs"), source, "Owner", "run", errors, "reviewed owner"
    ) == item
    assert errors == []

    method_gated_source = source.replace(
        "    fn run", "    #[cfg(test)]\n    fn run"
    )
    (gated_item,) = checker.rust_items(method_gated_source, "run")
    assert gated_item.attributes == ("#[cfg(test)]",)
    errors = []
    checker._require_qualified_rust_item(
        Path("fixture.rs"), method_gated_source, "Owner", "run", errors,
        "reviewed owner",
    )
    assert len(errors) == 1 and "unreviewed cfg/cfg_attr attributes" in errors[0]


def test_real_merge_sidecar_impl_attribute_preserves_reviewed_method_context(
    checker: ModuleType,
) -> None:
    path = (
        Path(__file__).resolve().parents[2]
        / "crates/iroha_core/src/merge_sidecar.rs"
    )
    source = path.read_text()
    (item,) = checker.rust_items(source, "inbound_session_capacity")
    assert item.brace_context == (("impl", "MergeSidecarLimits"),)
    errors: list[str] = []
    assert checker._require_qualified_rust_item(
        path, source, "MergeSidecarLimits", "inbound_session_capacity", errors,
        "reviewed sidecar limit",
    ) == item
    assert errors == []


@pytest.mark.parametrize("gating_attribute", [
    "#[cfg(test)]",
    "#[cfg_attr(not(test), cfg(any()))]",
    '#[cfg_attr(not(test), allow(dead_code, reason = "TODO: native runner cutover"))]\n#[cfg(test)]',
])
def test_impl_gating_attribute_remains_in_reviewed_context(
    checker: ModuleType, gating_attribute: str
) -> None:
    source = f"{gating_attribute}\nimpl Owner {{\n    fn run(&self) {{}}\n}}\n"
    (item,) = checker.rust_items(source, "run")
    assert item.brace_context != (("impl", "Owner"),)
    errors: list[str] = []
    assert checker._require_qualified_rust_item(
        Path("fixture.rs"), source, "Owner", "run", errors, "reviewed owner"
    ) is None
    assert errors == [
        "fixture.rs: require exactly one real Rust/Verus function item named Owner::run; found 0"
    ]
