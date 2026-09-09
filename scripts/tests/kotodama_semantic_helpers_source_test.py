#!/usr/bin/env python3
"""Fail closed on the Kotodama semantic-helper consolidation contract."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = Path("crates/kotodama_lang/src/semantic.rs")

# The post-merge stack-safety hardening replaces recursive typed-HIR traits and
# public semantic entry points with explicit iterative traversals. Rebase the
# original compacted-source baseline by that reviewed net +1,907-line delta
# while preserving the same reduction requirement and headroom.
BASELINE_RUST_LINES = 20_976
MINIMUM_RUST_LINE_REDUCTION = 1_500
MAXIMUM_RUST_LINES = BASELINE_RUST_LINES - MINIMUM_RUST_LINE_REDUCTION

# First-release V1 pins include explicit labels, nominal errors, named patterns,
# exact rejection selectors and bounded map scanning. Executable semantics are
# covered by the compiler suite; these hashes detect unreviewed source drift.
TEST_MARKER = "#[cfg(test)]\nmod tests {"
TEST_SUFFIX_SHA256 = (
    "e768aab893125104d704338f5f1ef37a1db7d1502ef54463ea2f8cf7c931ad83"
)
TEST_RECORDS_SHA256 = (
    "7d1f725bd23d36b28e4b2746917519d2dc2548cb398789f79451e2b882ca985b"
)
TEST_LEAVES = (
    (
        Path("crates/kotodama_lang/src/semantic/tests/numeric_rounding_modes.rs"),
        "a9b884a4d3b647b5e29d40a178edd0bd755bf5e04d4f18045b656567e70f7b34",
        1,
        "8d5036613dcf371bbe7e51cad2c95158b6a1f5d5fbb91d139635146fac815549",
    ),
    (
        Path("crates/kotodama_lang/src/semantic/tests/trigger_semantics_tests.rs"),
        "d6ee5192a815e9b52cc60518610d87f16837f9d22c5f1daab159e4b8af8389d2",
        13,
        "c9e496eff35dca6bdec40e87651766d71f3dc5d511abdfc58f26938165e21cfc",
    ),
    (
        Path("crates/kotodama_lang/src/semantic_sum_tests.rs"),
        "fddc9300a5b1a4bf7402162ed698f6873cb40217a76ff0d6594e498051c412d5",
        1,
        "60c30af0f8ef4a55f1004d351c8a4fa24c4d78aa6d30c61b712c4e5cefa9b42b",
    ),
    (
        Path("crates/kotodama_lang/src/semantic/tests/call_labels_and_patterns.rs"),
        "9a6dd091acca75f14c030600d7a57b175724038587f7cc521b1affb28f1bfe8c",
        15,
        "4670b929d5ceb05df2edefd79b9256743f46b12cd3581e6bb81e85b1270eddc7",
    ),
)
BUILTIN_SET_SHA256 = (
    "75cbaa9e9484e0e13066a0ba4ca2b04844f171dc2d65f7c53bfb63e56ecb2157"
)
DIAGNOSTIC_CODE_SET_SHA256 = (
    "0bc55f7619e6b0298976b22a5fa2a1939e39c46823c336d11dc4e12a014b4d79"
)
ORDERED_DIAGNOSTIC_CODES_SHA256 = (
    "05ff7160457dcf3550049c1903af8b56171c13f05b02617ad87650285ce0104b"
)
HELPER_REGION_SHA256 = (
    "033b87af0a45f0cbcf147c19a7117d9239259ce7a0a9af4ac992b13bcf1ed964"
)
EFFECT_REGION_SHA256 = (
    "b50a1543f0c038c504ad9ceafe2431342fc6f9ee9c313bb936afc2849cf4cda8"
)
DEFINITE_INIT_REGION_SHA256 = (
    "e7a9b1c8fba713da180748af056b40947f87e616fd394932fee58c0173bb501a"
)
FIXED_BUILTINS_SHA256 = (
    "e4c8510f457b3d5fd17cf477bad25f585ced471f451e151cbb16c131fa7cc582"
)
CUSTOM_BUILTINS_SHA256 = (
    "4e37cc9a546a5b9d1ad4b23b33fecbc16fdec65b3ddea7e2d4e36a9307c6cc87"
)

RETIRED_BUILTINS = (
    "AnonymousEscrowAccept",
    "AnonymousEscrowCancel",
    "AnonymousEscrowMarkPaymentSent",
    "AnonymousEscrowOpenDispute",
    "AnonymousEscrowOpenOffer",
    "AnonymousEscrowRelease",
    "AnonymousEscrowResolveDispute",
    "BuildPathKeyNoritoDirect",
    "BuildUnshieldInline",
    "CreateTrigger",
    "JsonGetAccountIdDirect",
    "JsonGetAssetDefinitionIdDirect",
    "JsonGetBlobHexDirect",
    "JsonGetDecimalDirect",
    "JsonGetIntDirect",
    "JsonGetJsonDirect",
    "JsonGetNameDirect",
    "JsonGetNftIdDirect",
    "JsonGetQuantityDirect",
    "JsonSetAccountIdDirect",
    "JsonSetIntDirect",
    "RemoveTrigger",
    "ScExecuteUnshield",
    "SchemaDecodeDirect",
    "SchemaEncodeDirect",
    "SchemaInfoDirect",
    "SoracloudEgressFetch",
    "SoracloudReadCredential",
    "SoracloudReadSecret",
    "ZkVerifyTransfer",
    "ZkVerifyUnshield",
)


class GuardError(AssertionError):
    """Raised when the protected source contract changes."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _read_source() -> str:
    path = REPO_ROOT / SOURCE_PATH
    _require(path.is_file() and not path.is_symlink(), f"invalid source path: {path}")
    try:
        path.resolve(strict=True).relative_to(REPO_ROOT)
    except ValueError as error:
        raise GuardError(f"source escapes repository root: {path}") from error
    return path.read_text(encoding="utf-8")


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def _json_sha256(value: object) -> str:
    payload = json.dumps(value, separators=(",", ":"))
    return _sha256(payload)


def _region(source: str, start: str, end: str) -> str:
    _require(source.count(start) == 1, f"expected one region start: {start!r}")
    start_index = source.index(start)
    _require(source.count(end, start_index) == 1, f"expected one region end: {end!r}")
    end_index = source.index(end, start_index)
    return source[start_index:end_index]


def _test_records(source: str) -> list[tuple[tuple[str, ...], str]]:
    pattern = re.compile(
        r"(?m)^(?P<attrs>(?:[ \t]*#\[[^\n]+\][ \t]*\n)+)"
        r"[ \t]*(?:async[ \t]+)?fn[ \t]+(?P<name>[A-Za-z_]\w*)[ \t]*\("
    )
    records = []
    for match in pattern.finditer(source):
        attributes = tuple(re.findall(r"#\[[^\n]+\]", match.group("attrs")))
        if "#[test]" in attributes:
            records.append((attributes, match.group("name")))
    return records


def _builtin_variants(source: str) -> list[str]:
    return sorted(set(re.findall(r"Builtin::([A-Za-z0-9_]+)", source)))


def _diagnostic_codes(source: str) -> list[str]:
    return re.findall(
        r'(?:code:\s*|sem_err\(\s*)"([A-ZK][A-Z0-9_]*)"',
        source,
    )


def validate_source(source: str) -> None:
    """Validate the compact source and every preserved semantic invariant."""

    line_count = len(source.splitlines())
    _require(
        line_count <= MAXIMUM_RUST_LINES,
        f"semantic.rs grew to {line_count} lines; maximum is {MAXIMUM_RUST_LINES}",
    )
    _require(source.count(TEST_MARKER) == 1, "test module marker changed")
    marker_index = source.index(TEST_MARKER)
    production = source[:marker_index]
    test_suffix = source[marker_index:]

    _require(_sha256(test_suffix) == TEST_SUFFIX_SHA256, "test suffix changed")
    test_records = _test_records(test_suffix)
    _require(len(test_records) == 127, "direct test count changed")
    _require(
        _json_sha256(test_records) == TEST_RECORDS_SHA256,
        "test identifiers, attributes, or order changed",
    )
    for path, digest, record_count, records_digest in TEST_LEAVES:
        leaf = REPO_ROOT / path
        _require(leaf.is_file() and not leaf.is_symlink(), f"invalid test leaf: {path}")
        try:
            leaf.resolve(strict=True).relative_to(REPO_ROOT)
        except ValueError as error:
            raise GuardError(f"test leaf escapes repository root: {path}") from error
        leaf_source = leaf.read_text(encoding="utf-8")
        _require(_sha256(leaf_source) == digest, f"test leaf changed: {path}")
        leaf_records = _test_records(leaf_source)
        _require(len(leaf_records) == record_count, f"test leaf count changed: {path}")
        _require(
            _json_sha256(leaf_records) == records_digest,
            f"test leaf identities changed: {path}",
        )

    builtins = _builtin_variants(production)
    _require(len(builtins) == 229, "production Builtin reference set changed")
    _require(
        _json_sha256(builtins) == BUILTIN_SET_SHA256,
        "production Builtin variants changed",
    )
    codes = _diagnostic_codes(production)
    _require(len(set(codes)) == 148, "diagnostic identity set changed")
    _require(
        _json_sha256(sorted(set(codes))) == DIAGNOSTIC_CODE_SET_SHA256,
        "diagnostic identities changed",
    )
    _require(len(codes) == 472, "diagnostic site count changed")
    _require(
        _json_sha256(codes) == ORDERED_DIAGNOSTIC_CODES_SHA256,
        "diagnostic identity order changed",
    )

    helper_region = _region(
        production,
        "fn typed_expr(",
        "\nfn enclosing_return_type(",
    )
    effect_region = _region(
        production,
        "fn block_effects(",
        "\nfn is_state_identifier(",
    )
    definite_init_region = _region(
        production,
        "fn validate_scalar_state_initialization(",
        "\nfn enforce_permission_requirements(",
    )
    _require(
        _sha256(helper_region) == HELPER_REGION_SHA256,
        "fixed-builtin helper region changed",
    )
    _require(
        _sha256(effect_region) == EFFECT_REGION_SHA256,
        "unified effect walker changed",
    )
    _require(
        _sha256(definite_init_region) == DEFINITE_INIT_REGION_SHA256,
        "definite scalar-state initialization flow changed",
    )

    fixed_region = _region(
        helper_region,
        "fn fixed_builtin_message(",
        "\nfn fixed_builtin_arg_accepts(",
    )
    custom_start = "fn analyze_surface_builtin_call("
    _require(
        helper_region.count(custom_start) == 1,
        "surface Builtin analyzer boundary changed",
    )
    custom_region = helper_region[helper_region.index(custom_start) :]
    fixed_builtins = _builtin_variants(fixed_region)
    custom_builtins = _builtin_variants(custom_region)
    _require(len(fixed_builtins) == 156, "fixed Builtin partition changed")
    _require(len(custom_builtins) == 71, "custom Builtin partition changed")
    _require(
        not set(fixed_builtins).intersection(custom_builtins),
        "fixed and custom Builtin partitions overlap",
    )
    _require(
        _json_sha256(fixed_builtins) == FIXED_BUILTINS_SHA256,
        "fixed Builtin partition identities changed",
    )
    _require(
        _json_sha256(custom_builtins) == CUSTOM_BUILTINS_SHA256,
        "custom Builtin partition identities changed",
    )

    for retired in RETIRED_BUILTINS:
        _require(
            re.search(rf"\bBuiltin::{re.escape(retired)}\b", production) is None,
            f"retired Builtin returned: {retired}",
        )
    for token in (
        "macro_rules!",
        "$action",
        "$body",
        "$step",
        "dyn Fn",
        "impl Fn",
        ": fn(",
        "fn (",
        "#[rustfmt::skip]",
        "#[path =",
        "include!",
    ):
        _require(token not in helper_region + effect_region, f"forbidden helper token: {token}")

    _require(
        production.count("direct_effects: block_effects(") == 2,
        "effect summaries no longer use the unified walker exactly twice",
    )
    for old_name in (
        "block_contains_host_side_effects",
        "block_contains_instruction_emission",
        "block_mutates_durable_state",
        "statement_contains_host_side_effects",
        "statement_contains_instruction_emission",
        "statement_mutates_durable_state",
        "expr_contains_host_side_effects",
        "expr_contains_instruction_emission",
        "expr_mutates_durable_state",
    ):
        _require(old_name not in production, f"parallel effect walker returned: {old_name}")

    _require("*vars = loop_env;" not in production, "for-loop locals leaked into outer scope")

    for required in (
        "query page offset must be in 0..=i64::MAX",
        "query page offset plus limit must fit i64",
        ".or_else(|| value.try_to_u128().map(JsonNumber::U128))",
        "_ => analyze_fixed_builtin_call(builtin, arg_typed)",
        "effects.merge_from(statement_effects(context, statement));",
        "effects.mutates_durable_state |= typed_map_expr_is_state(context, map);",
        "let mut t1 = analyze_expr_expected(context, then_expr, vars, expected)?;",
        "struct DefiniteInitExprFlow {",
        "fn continue_definite_init_expr(",
    ):
        _require(required in production, f"required current semantic invariant missing: {required}")
    for required_test in (
        "fn trigger_metadata_integer_domain_is_exact_through_u128(",
        "fn typed_aggregate_traits_are_spawn_free_for_flat_width(",
        "fn semantic_type_and_expression_traits_are_iterative_at_the_depth_boundary(",
        "fn public_semantic_apis_handoff_from_a_small_caller(",
        "fn ternary_literals_inherit_the_enclosing_numeric_context(",
        "fn raw_semantic_analysis_does_not_leak_range_iterators(",
        "fn scalar_state_initialization_checks_early_returns_inside_expressions(",
    ):
        _require(required_test in test_suffix, f"required regression test missing: {required_test}")
    for stale in (
        "query page offset must be non-negative and fit u64",
        "E_UNSHIELD_AMOUNT_RANGE",
    ):
        _require(stale not in production, f"stale donor semantic returned: {stale}")


def _replace_once(source: str, old: str, new: str) -> str:
    _require(source.count(old) == 1, f"mutation anchor changed: {old!r}")
    return source.replace(old, new, 1)


class KotodamaSemanticHelpersSourceTest(unittest.TestCase):
    """Authenticate the compact helpers and prove the guard fails closed."""

    def test_repository_source_contract(self) -> None:
        validate_source(_read_source())

    def test_mutations_fail_closed(self) -> None:
        source = _read_source()
        variant_anchor = (
            "        _ => return None,\n"
            "    })\n"
            "}\n\n"
            "fn fixed_builtin_arg_accepts"
        )
        mutations = {
            "line budget": _replace_once(
                source,
                TEST_MARKER,
                ("// line-budget mutation\n" * 1_501) + TEST_MARKER,
            ),
            "fixed diagnostic": _replace_once(
                source,
                "query page offset plus limit must fit i64",
                "query page offset plus limit must fit u64",
            ),
            "u128 trigger metadata": _replace_once(
                source,
                ".or_else(|| value.try_to_u128().map(JsonNumber::U128))",
                ".or_else(|| value.try_to_u64().map(JsonNumber::U64))",
            ),
            "effect merge": _replace_once(
                source,
                "effects.merge_from(statement_effects(context, statement));",
                "let _ = statement;",
            ),
            "test identity": _replace_once(
                source,
                "fn param_type_enforcement_primitives(",
                "fn param_type_enforcement_primitives_mutated(",
            ),
            "retired variant": _replace_once(
                source,
                variant_anchor,
                "        Builtin::BuildUnshieldInline => unreachable!(),\n"
                + variant_anchor,
            ),
            "split effect walk": _replace_once(
                source,
                "direct_effects: block_effects(&context, &function.body)",
                "direct_effects: block_contains_host_side_effects(&function.body)",
            ),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label):
                self.assertNotEqual(mutated, source)
                with self.assertRaises(GuardError):
                    validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
