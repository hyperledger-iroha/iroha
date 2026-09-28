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

# The historical helper compaction used a 20,976-line opening and a 1,500-line
# reduction gate. Those measurements are historical evidence, not a current
# allowance: the parent now obeys the existing repository source-file budget.
MAXIMUM_RUST_LINES = 16_100
OWNER_PATHS = (
    Path("crates/kotodama_lang/src/semantic/value_traits.rs"),
    Path("crates/kotodama_lang/src/semantic/trigger_lowering.rs"),
)
OWNER_SHA256 = {
    OWNER_PATHS[0]: "a78dde31458feb19788fa50c8b784d0a43e59fdba55d67434fb424a6b6acffad",
    OWNER_PATHS[1]: "b0ba94606473025eb0a75fe88fdfeb2e44dbb06aa41d75212b51f7cbd71e465d",
}

# First-release V1 pins include explicit labels, nominal errors, named patterns,
# exact rejection selectors and bounded map scanning. Executable semantics are
# covered by the compiler suite; these hashes detect unreviewed source drift.
TEST_MARKER = "#[cfg(test)]\nmod tests {"
TEST_SUFFIX_SHA256 = (
    "f51ef5c4a03ab2eafb606a464aa7917eb10aba742ed90a94784cc1dde9d7b89b"
)
TEST_RECORDS_SHA256 = (
    "7dfbc68fd6602c52b4f4d952c6b093bf3c13234c3724452dd73260178fd16aa1"
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
        "888c4838d9cdb63eac76cce2743c311863ce1f7be22045f20a58824864dddc12",
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
        "f343fde2e3e3ce828af21319b541df2453358b9cb78076c107d83be55c154da6",
        15,
        "4670b929d5ceb05df2edefd79b9256743f46b12cd3581e6bb81e85b1270eddc7",
    ),
)
BUILTIN_SET_SHA256 = (
    "e0e01279c573e3fd1e1b0431376da947769bff242eb431d49609d01d7265afea"
)
DIAGNOSTIC_CODE_SET_SHA256 = (
    "bccf5784d961fa57a1f90f23eebf7efb418f08c82e3a273fdd996825cf6ff200"
)
ORDERED_DIAGNOSTIC_CODES_SHA256 = (
    "5755d3213713f0f709d8b2544950db445feb9aeba94469a975f61463cdd782c6"
)
HELPER_REGION_SHA256 = (
    "3bf3fa9a54c96010f767a936914bff251cc7217bac1d5ed181583b93059c1ed9"
)
EFFECT_REGION_SHA256 = (
    "b50a1543f0c038c504ad9ceafe2431342fc6f9ee9c313bb936afc2849cf4cda8"
)
DEFINITE_INIT_REGION_SHA256 = (
    "e7a9b1c8fba713da180748af056b40947f87e616fd394932fee58c0173bb501a"
)
FIXED_BUILTINS_SHA256 = (
    "4da094f1ba49b0950240568b97c5c8851f7f72027cee27328b57ba2e65a95276"
)
CUSTOM_BUILTINS_SHA256 = (
    "96ba2c45673c2581cfcaa13365949e0ed3028adf809255ab5c55e3d5c7d9d931"
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
    "DecodeInt",
    "EncodeInt",
    "UseAssetHandle",
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


def _read_repo_source(relative: Path) -> str:
    path = REPO_ROOT / relative
    _require(path.is_file() and not path.is_symlink(), f"invalid source path: {path}")
    try:
        path.resolve(strict=True).relative_to(REPO_ROOT)
    except ValueError as error:
        raise GuardError(f"source escapes repository root: {path}") from error
    return path.read_text(encoding="utf-8")


def _read_source() -> str:
    return _read_repo_source(SOURCE_PATH)


def _read_owners() -> dict[Path, str]:
    return {path: _read_repo_source(path) for path in OWNER_PATHS}


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


def validate_source(source: str, owners: dict[Path, str] | None = None) -> None:
    """Validate the parent, its two cohesive owners and semantic invariants."""

    line_count = len(source.splitlines())
    _require(
        line_count <= MAXIMUM_RUST_LINES,
        f"semantic.rs grew to {line_count} lines; maximum is {MAXIMUM_RUST_LINES}",
    )
    _require(source.count(TEST_MARKER) == 1, "test module marker changed")
    marker_index = source.index(TEST_MARKER)
    parent_production = source[:marker_index]
    test_suffix = source[marker_index:]
    owners = _read_owners() if owners is None else owners
    _require(set(owners) == set(OWNER_PATHS), "semantic owner inventory changed")
    for path in OWNER_PATHS:
        _require(_sha256(owners[path]) == OWNER_SHA256[path], f"semantic owner changed: {path}")
        _require(not _test_records(owners[path]), f"tests relocated into production owner: {path}")
    for declaration in (
        "mod trigger_lowering;", "mod value_traits;",
        "use trigger_lowering::analyze_trigger;",
        "use value_traits::render_source_type_name;",
        "pub use value_traits::render_type_name;",
        "pub(crate) use value_traits::type_name;",
    ):
        _require(parent_production.count(declaration) == 1, f"semantic owner wiring changed: {declaration}")
    # Diagnostic order is pinned in explicit parent/value-traits/trigger-owner
    # order. Its identity multiset is unchanged by moving the complete owners.
    production = parent_production + "\n" + "\n".join(owners[path] for path in OWNER_PATHS)

    _require(_sha256(test_suffix) == TEST_SUFFIX_SHA256, "test suffix changed")
    test_records = _test_records(test_suffix)
    _require(len(test_records) == 126, "direct test count changed")
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
    _require(len(builtins) == 227, "production Builtin reference set changed")
    _require(
        _json_sha256(builtins) == BUILTIN_SET_SHA256,
        "production Builtin variants changed",
    )
    codes = _diagnostic_codes(production)
    _require(len(set(codes)) == 147, "diagnostic identity set changed")
    _require(
        _json_sha256(sorted(set(codes))) == DIAGNOSTIC_CODE_SET_SHA256,
        "diagnostic identities changed",
    )
    _require(len(codes) == 468, "diagnostic site count changed")
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
    _require(len(fixed_builtins) == 155, "fixed Builtin partition changed")
    _require(len(custom_builtins) == 70, "custom Builtin partition changed")
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
        "crate::checked_arithmetic::evaluate(&expression)",
        "Ok(Some(value)) => Ok(value.into_typed_expr())",
        "Ok(None) => Ok(expression)",
        "code: error.code()",
        "Builtin::StageAnchoredSpend",
        '"AxtAnchoredSpendV1" => ty == &Type::AxtAnchoredSpendV1',
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
        owners = _read_owners()
        validate_source(source, owners)
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
                    validate_source(mutated, owners)


    def test_owner_mutations_fail_closed(self) -> None:
        source = _read_source()
        owners = _read_owners()
        validate_source(source, owners)
        mutations = (
            (
                "trigger diagnostic", OWNER_PATHS[1],
                'code: "E_TRIGGER_INVALID_NAME"',
                'code: "E_TRIGGER_INVALID_NAME_CHANGED"',
                1,
            ),
            (
                "nominal type identity", OWNER_PATHS[0],
                "if left_name != right_name || left_fields.len() != right_fields.len()",
                "if left_fields.len() != right_fields.len()",
                2,
            ),
            (
                "active typed equality", OWNER_PATHS[0],
                "while let Some((left, right)) = pending.pop()",
                "while let Some((left, right)) = pending.first().copied()",
                1,
            ),
        )
        for label, path, old, new, expected_count in mutations:
            with self.subTest(label=label):
                mutated = dict(owners)
                self.assertEqual(owners[path].count(old), expected_count)
                mutated[path] = owners[path].replace(old, new, 1)
                with self.assertRaises(GuardError):
                    validate_source(source, mutated)

    def test_missing_owner_or_public_path_is_rejected(self) -> None:
        source = _read_source()
        owners = _read_owners()
        validate_source(source, owners)
        with self.assertRaises(GuardError):
            validate_source(source, {OWNER_PATHS[0]: owners[OWNER_PATHS[0]]})
        with self.assertRaises(GuardError):
            validate_source(
                _replace_once(source, "pub use value_traits::render_type_name;", ""),
                owners,
            )


if __name__ == "__main__":
    unittest.main()
