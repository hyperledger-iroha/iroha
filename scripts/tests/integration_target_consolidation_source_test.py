"""Source guards for aggregated Cargo integration-test targets."""

from __future__ import annotations

import re
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class TargetContract:
    package: str
    target: str
    root: str
    modules: tuple[tuple[str, str], ...]
    tests: tuple[str, ...]


XTASK_MODULES = tuple(
    (name, f"{name}.rs")
    for name in (
        "address_vectors",
        "android_dashboard_parity_cli",
        "codec_rans_tables",
        "da_proof_bench",
        "iso_bridge_lint",
        "ministry_agenda",
        "mochi_bundle",
        "sm_wycheproof_sync",
        "soradns_cli",
        "sorafs_fetch_fixture",
        "soranet_bug_bounty",
        "soranet_chaos",
        "soranet_gateway_billing",
        "soranet_gateway_m0",
        "soranet_gateway_m1",
        "soranet_gateway_m2",
        "soranet_gateway_ops_m0",
        "soranet_pop_template",
        "streaming_bundle_check",
        "streaming_entropy_bench",
    )
)


XTASK_TESTS = tuple(
    line.strip()
    for line in """
address_vectors::address_vectors_verify_defaults
address_vectors::address_vectors_write_custom_path
android_dashboard_parity_cli::dashboard_parity_cli_respects_cli_overrides
codec_rans_tables::rans_tables_generation_is_deterministic
codec_rans_tables::verify_tables_detects_tampering
codec_rans_tables::bundled_tables_enable_roundtrip
da_proof_bench::da_proof_bench_emits_reports
iso_bridge_lint::iso_bridge_lint_defaults_pass
iso_bridge_lint::iso_bridge_lint_rejects_unknown_instrument_fixture
ministry_agenda::ministry_agenda_validate_example_passes
ministry_agenda::ministry_agenda_duplicate_conflict_is_reported
mochi_bundle::mochi_bundle_command_generates_manifest
mochi_bundle::mochi_bundle_matrix_and_smoke
mochi_bundle::mochi_bundle_stage_directory_copies_bundle
sm_wycheproof_sync::sm_wycheproof_sync_from_file
sm_wycheproof_sync::sm_wycheproof_sync_from_url
soradns_cli::soradns_hosts_reports_expected_derivations
soradns_cli::soradns_hosts_supports_taira_mon_pretty_suffix
soradns_cli::soradns_binding_template_writes_payload_and_headers
soradns_cli::soradns_gar_template_renders_payload
soradns_cli::soradns_gar_template_derives_manifest_metadata
soradns_cli::soradns_cache_plan_renders_targets
soradns_cli::soradns_acme_plan_covers_canonical_and_pretty_hosts
soradns_cli::soradns_acme_plan_supports_taira_mon_pretty_suffix
sorafs_fetch_fixture::sorafs_fetch_fixture_copies_and_verifies_local_files
soranet_bug_bounty::bug_bounty_pack_is_emitted
soranet_chaos::soranet_chaos_kit_and_report_round_trip
soranet_gateway_billing::soranet_gateway_billing_runs_end_to_end
soranet_gateway_m0::soranet_gateway_m0_pack_is_deterministic
soranet_gateway_m1::soranet_gateway_m1_bundle_is_emitted
soranet_gateway_m2::soranet_gateway_m2_pipeline_emits_beta_and_ga
soranet_gateway_ops_m0::soranet_gateway_ops_m0_pack_is_deterministic
soranet_pop_template::soranet_pop_template_renders_fixture
soranet_pop_template::soranet_pop_template_writes_resolver_config
soranet_pop_template::soranet_pop_validate_reports_metadata
soranet_pop_template::soranet_pop_policy_report_emits_monitoring_pack
soranet_pop_template::soranet_pop_bundle_embeds_route_health_probe
soranet_pop_template::soranet_pop_bundle_writes_manifest_and_assets
soranet_pop_template::soranet_popctl_aliases_pop_bundle
streaming_bundle_check::streaming_bundle_check_reports_bundled_requirements
streaming_entropy_bench::streaming_entropy_bench_emits_metrics
streaming_entropy_bench::streaming_entropy_bench_roundtrips_chroma_and_yuv_psnr
streaming_entropy_bench::streaming_entropy_bench_respects_quantizer_override
streaming_entropy_bench::streaming_entropy_bench_supports_quantizer_ladder_and_tiny_preset
""".splitlines()
    if line.strip()
)


SCHEMA = TargetContract(
    package="crates/iroha_schema",
    target="schema",
    root="schema.rs",
    modules=(
        ("architecture_dependent", "architecture-dependent.rs"),
        ("enum_with_default_discriminants", "enum_with_default_discriminants.rs"),
        ("enum_with_various_discriminants", "enum_with_various_discriminants.rs"),
        ("fieldless_enum", "fieldless_enum.rs"),
        ("floats", "floats.rs"),
        ("non_zero", "non_zero.rs"),
        ("numbers_compact_and_fixed", "numbers_compact_and_fixed.rs"),
        ("schema_json", "schema_json.rs"),
        ("struct_with_generic_bounds", "struct_with_generic_bounds.rs"),
        ("struct_with_named_fields", "struct_with_named_fields.rs"),
        ("struct_with_unnamed_fields", "struct_with_unnamed_fields.rs"),
        ("transparent_types", "transparent_types.rs"),
    ),
    tests=tuple(
        line.strip()
        for line in """
architecture_dependent::usize_isize_not_into_schema
enum_with_default_discriminants::default_discriminants
enum_with_various_discriminants::discriminant
enum_with_various_discriminants::schema_discriminants_match_encoded_u32_tags
fieldless_enum::discriminant
floats::float_primitives_have_explicit_schema_metadata
non_zero::non_zero_integers_schema
non_zero::arch_dependent_non_zero_are_excluded
numbers_compact_and_fixed::compact
schema_json::test_struct
schema_json::test_struct_codec_attr
schema_json::test_transparent
schema_json::test_enum
schema_json::test_enum_with_norito_rename_all
schema_json::test_enum_codec_attr
struct_with_generic_bounds::check_generic
struct_with_named_fields::named_fields
struct_with_unnamed_fields::unnamed
transparent_types::transparent_types
""".splitlines()
        if line.strip()
    ),
)


XTASK = TargetContract(
    package="xtask",
    target="integration",
    root="integration.rs",
    modules=XTASK_MODULES,
    tests=XTASK_TESTS,
)


def _test_tables(manifest: str) -> list[str]:
    return [
        table.strip()
        for table in re.findall(r"(?ms)^\[\[test\]\]\n(.*?)(?=^\[|\Z)", manifest)
    ]


def _module_rows(source: str) -> tuple[tuple[str, str], ...]:
    return tuple(
        (module, path)
        for path, module in re.findall(
            r'#\[path = "([^"]+)"\]\nmod ([a-z0-9_]+);', source
        )
    )


def _test_names(module: str, source: str) -> tuple[str, ...]:
    attrs = re.findall(
        r"(?m)^\s*#\[([^]]*test[^]]*)\]\s*\n\s*fn\s+([A-Za-z0-9_]+)", source
    )
    if any(attr != "test" for attr, _ in attrs):
        raise AssertionError(f"{module}: non-plain test attribute found: {attrs!r}")
    return tuple(f"{module}::{name}" for _, name in attrs)


def validate(contract: TargetContract) -> None:
    package = ROOT / contract.package
    manifest = (package / "Cargo.toml").read_text(encoding="utf-8")
    package_table = manifest.split("\n[", 1)[0]
    if not re.search(r"(?m)^autotests = false$", package_table):
        raise AssertionError(f"{contract.package}: autotests must be disabled")
    tables = _test_tables(manifest)
    expected_table = f'name = "{contract.target}"\npath = "tests/{contract.root}"'
    if tables != [expected_table]:
        raise AssertionError(f"{contract.package}: unexpected test target tables: {tables!r}")

    tests_dir = package / "tests"
    aggregate = (tests_dir / contract.root).read_text(encoding="utf-8")
    if _module_rows(aggregate) != contract.modules:
        raise AssertionError(f"{contract.package}: aggregate module inventory drifted")

    actual_tests: list[str] = []
    for module, path in contract.modules:
        source = (tests_dir / path).read_text(encoding="utf-8")
        actual_tests.extend(_test_names(module, source))
    if tuple(actual_tests) != contract.tests:
        raise AssertionError(f"{contract.package}: historical test inventory drifted")


class IntegrationTargetConsolidationTest(unittest.TestCase):
    def test_xtask_target_is_aggregated(self) -> None:
        validate(XTASK)

    def test_iroha_schema_target_is_aggregated(self) -> None:
        validate(SCHEMA)


if __name__ == "__main__":
    unittest.main()
