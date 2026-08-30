"""Source guards for aggregated Cargo integration-test targets."""

from __future__ import annotations

import hashlib
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


@dataclass(frozen=True)
class WaveTwoTarget:
    package: str
    target: str
    root: str
    modules: tuple[tuple[str, str], ...]
    required_features: tuple[str, ...] = ()
    dead_code_modules: tuple[str, ...] = ()
    serial: bool = False


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


WAVE_TWO_TARGETS = (
    WaveTwoTarget(
        package="crates/iroha_derive",
        target="container_enum_from_variant",
        root="container_enum_from_variant.rs",
        modules=(("enum_from_variant_attrs", "enum_from_variant_attrs.rs"),),
        dead_code_modules=("enum_from_variant_attrs",),
    ),
    WaveTwoTarget(
        package="crates/iroha_derive",
        target="ui",
        root="ui.rs",
        modules=(("config_base_ui", "config_base_ui.rs"),),
        required_features=("trybuild-tests",),
        serial=True,
    ),
    WaveTwoTarget(
        package="crates/iroha_monitor",
        target="smoke",
        root="smoke.rs",
        modules=(
            ("attach_render", "attach_render.rs"),
            ("http_limits", "http_limits.rs"),
            ("invalid_credentials", "invalid_credentials.rs"),
        ),
        serial=True,
    ),
    WaveTwoTarget(
        package="crates/iroha_primitives",
        target="addr_parsing",
        root="addr_parsing.rs",
        modules=(("numeric_inspect", "numeric_inspect.rs"),),
    ),
    WaveTwoTarget(
        package="crates/iroha_primitives",
        target="ui",
        root="ui.rs",
        modules=(),
        required_features=("trybuild-tests",),
    ),
    WaveTwoTarget(
        package="crates/iroha_version_derive",
        target="codec",
        root="codec.rs",
        modules=(("json", "json.rs"),),
    ),
    WaveTwoTarget(
        package="crates/iroha_version_derive",
        target="ui",
        root="ui.rs",
        modules=(),
        required_features=("trybuild-tests",),
    ),
    WaveTwoTarget(
        package="crates/iroha_zkp_halo2",
        target="vega_engine_reachability",
        root="vega_engine_reachability.rs",
        modules=(
            (
                "vega_microsoft_cross_conformance",
                "vega_microsoft_cross_conformance.rs",
            ),
        ),
    ),
    WaveTwoTarget(
        package="crates/soranet_pq",
        target="kat_vectors",
        root="kat_vectors.rs",
        modules=(("pq_kat", "pq_kat.rs"),),
    ),
    WaveTwoTarget(
        package="mochi/mochi-core",
        target="composer_drafts",
        root="composer_drafts.rs",
        modules=(("torii_streams", "torii_streams.rs"),),
    ),
    WaveTwoTarget(
        package="mochi/mochi-integration",
        target="readiness_smoke",
        root="readiness_smoke.rs",
        modules=(("supervisor", "supervisor.rs"),),
    ),
    WaveTwoTarget(
        package="crates/sorafs_node",
        target="pin_workflows",
        root="pin_workflows.rs",
        modules=(("cli", "cli.rs"),),
    ),
    WaveTwoTarget(
        package="tools/soranet-handshake-harness",
        target="fixtures_verify",
        root="fixtures_verify.rs",
        modules=(
            ("interop_parity", "interop_parity.rs"),
            ("perf_gate", "perf_gate.rs"),
            ("simulate_cli", "simulate_cli.rs"),
        ),
        serial=True,
    ),
)

WAVE_TWO_MANIFEST_BASE_SHA256 = {
    "crates/iroha_derive": "93c00d79bedfb21c6f4be400b7090050faedd8beeac477ce119ef03695e30b35",
    "crates/iroha_monitor": "b939b6dacf84952700e3d4fe47d657c31cfa90b2a29894f9334edba541483bb5",
    "crates/iroha_primitives": "094bbfb1e32d123a20708cf0650b79985bd23b5edccc9b04ac771ae785fe4dc5",
    "crates/iroha_version_derive": "d652a0868d36147e19e196692830566858eed0a46b92b752ae451a93eaeb2977",
    "crates/iroha_zkp_halo2": "191aa250aabbac123f7cbc7fa6f51ed0481cbf921081940fa0fddb7cad1d9ba6",
    "crates/soranet_pq": "09814d2ba4ed385c0683a0ecb7b8936b99f7df631391ce09dbe783e96c791c83",
    "mochi/mochi-core": "41aa8dfc5df7782ebf28c7ce4e536f22ec339251bf1598af049f36d202ae3087",
    "mochi/mochi-integration": "d85af2df1130e942def7e0754c26470977eb54d42405bcc361a300c34f7e7009",
    "crates/sorafs_node": "72b2aabf798dc4f94967cbfd79bbc1b819c4a885e0af376106c65e64503706c8",
    "tools/soranet-handshake-harness": "1c4aeb7b28cf94c43e3ff11242591f3039bc486c326f27df242da50d08e28708",
}

WAVE_TWO_SOURCE_SHA256 = {
    "crates/iroha_derive/tests/config_base_ui.rs": "2428e6cf3038ef5096df366414909912f115045db2f1edc69c9bd1fe074f2163",
    "crates/iroha_derive/tests/container_enum_from_variant.rs": "f8a93d96864140846183f29253dbedf47a21980e2b171bf1bfb4c370f6b313af",
    "crates/iroha_derive/tests/enum_from_variant_attrs.rs": "bd4fc5f4611a55b4fe34837a265c4ed156ae56fea64ba7ab59bdad958d1ee093",
    "crates/iroha_derive/tests/ui.rs": "a4a0541a2832119a443d3d1b79826a8417090906e78479641d90c61e6e7b5408",
    "crates/iroha_monitor/tests/attach_render.rs": "a556a01b6ebac2ad4820f3def963bb222f09e190d456e520ff61e7107d7dc4d2",
    "crates/iroha_monitor/tests/http_limits.rs": "36fc8d7a4b767480ebd33994cd875825787ee09f6c93247e37763e73bffb37bc",
    "crates/iroha_monitor/tests/invalid_credentials.rs": "6802abad720812bd69e29f7ee620f7d6ff8f52ea49c322de0606c47851586c25",
    "crates/iroha_monitor/tests/smoke.rs": "5b74f2816cd63b0e5ab354cc44b632499341b4364d3aab110db5263b0042ae0d",
    "crates/iroha_primitives/tests/addr_parsing.rs": "609cdcf28f60920931fc88b584cab6d319bd6983eaf734fe746c39e53e2bb3d3",
    "crates/iroha_primitives/tests/numeric_inspect.rs": "5034969e36547a4b70294280f6ba2dbdec1089eae5b8f6aac4dbc51685205459",
    "crates/iroha_primitives/tests/ui.rs": "ca0d4e7a21ea77122e52f0ad9f2eb14c0a9d4865fb556db3ba3d935521e04818",
    "crates/iroha_version_derive/tests/codec.rs": "f4386043897cf21f4c4115ad453c1495a59d258b65e12471dc5ea75e116ccf46",
    "crates/iroha_version_derive/tests/json.rs": "8092c6769cdeec5ffde1cacf13626193c88324ae8d76a940cc41a5d29d21a041",
    "crates/iroha_version_derive/tests/ui.rs": "72fcf2f051e0f9d9d85a5918820d62e2d65168d4ad00d8cb7eb7584236776823",
    "crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs": "6a014a9cbbc9482ff6796f86ec762bf1c699eeb014a0ac6db5d32939a049c11e",
    "crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs": "e0324517601b889fd7eb2ab52b6b550502fc5a77971eadd1065e0f8dcfb9a15b",
    "crates/soranet_pq/tests/kat_vectors.rs": "84b89d698051989013d4147dffd10d5261c741e352f213ad9150aa0b1c28321c",
    "crates/soranet_pq/tests/pq_kat.rs": "69491d6c86e58a8f3edb74d40ca46cd261801fbfbd2f459cfcd86cfe64fcc98a",
    "crates/sorafs_node/tests/cli.rs": "ba3232c0349859e47003b163faf21b7cc4a0b86f3a3217f0c9c2750a0ec6ef1f",
    "crates/sorafs_node/tests/pin_workflows.rs": "24e7d0547db730c3eb561c55d606b4f5526309b29432923d048f9b511e8f675d",
    "mochi/mochi-core/tests/composer_drafts.rs": "9aee3bade320bf3c19c9cb00cd8d2e8dab96250d3e91c058b97331bac4daf32b",
    "mochi/mochi-core/tests/torii_streams.rs": "9eeff756579057756031e656fe84e404d0931461b06c8afcce035a9327a6bdc0",
    "mochi/mochi-integration/tests/readiness_smoke.rs": "c8bbc0479a27383548d726d9f5d427363205280635199f176008714d1f38cbc3",
    "mochi/mochi-integration/tests/supervisor.rs": "36948cb1d0bad6a6f4d09d82308cd2623a147236ebcad047b27fc64effb0a7cd",
    "tools/soranet-handshake-harness/tests/fixtures_verify.rs": "8487b4d970bdf1cdbab49a344bfbc07c7e7e2e4b3ca701f97c8811952d7e48dc",
    "tools/soranet-handshake-harness/tests/interop_parity.rs": "8f6fdaa1660770c86bd0d5a1c1c6c6b8cd61fdbf6bef95749c9694e1896aa745",
    "tools/soranet-handshake-harness/tests/perf_gate.rs": "daa16e6ec412927c7a6adc7056b34c22e69e9fb6be4fd934aa43bfa1c49dbff2",
    "tools/soranet-handshake-harness/tests/simulate_cli.rs": "da3e01bfc1434feabbce2e46ec22fb420a37d779cc1c74459caa8283da241363",
}

WAVE_TWO_TARGET_INVENTORY_SHA256 = (
    "088785f56a3ac0ab2a879a503c5e4d26a989e55b623a873d84a8438ee20a2640"
)
WAVE_TWO_MODULE_INVENTORY_SHA256 = (
    "931d70b2ecfbf8865ee9e1c9ed4486b4280d508cb109776b52b8e4397bed56f6"
)
WAVE_TWO_TEST_INVENTORY_SHA256 = (
    "d1f595e10d9f0c50607357285fdb8d4a2ba2fc235f1925c38262f98fb44acccc"
)

SERIAL_GUARD_SOURCE = """fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
"""
SERIAL_CALL = "    let _serial = crate::serial_guard();\n"


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


def _wave_two_packages() -> tuple[str, ...]:
    return tuple(dict.fromkeys(target.package for target in WAVE_TWO_TARGETS))


def _wave_two_table(target: WaveTwoTarget) -> str:
    lines = [f'name = "{target.target}"', f'path = "tests/{target.root}"']
    if target.required_features:
        features = ", ".join(f'"{feature}"' for feature in target.required_features)
        lines.append(f"required-features = [{features}]")
    return "\n".join(lines)


def _wave_two_module_declaration(
    target: WaveTwoTarget, module: str, path: str
) -> str:
    allow = "#[allow(dead_code)]\n" if module in target.dead_code_modules else ""
    return f'{allow}#[path = "{path}"]\nmod {module};\n'


def _wave_two_expected_paths() -> set[str]:
    paths = {f"{package}/Cargo.toml" for package in _wave_two_packages()}
    paths.update(WAVE_TWO_SOURCE_SHA256)
    return paths


def _read_wave_two_sources() -> dict[str, str]:
    paths = {f"{package}/Cargo.toml" for package in _wave_two_packages()}
    for package in _wave_two_packages():
        tests_dir = ROOT / package / "tests"
        paths.update(
            path.relative_to(ROOT).as_posix() for path in tests_dir.glob("*.rs")
        )
    return {
        path: (ROOT / path).read_text(encoding="utf-8")
        for path in sorted(paths)
    }


def _test_items(source: str) -> tuple[tuple[str, str], ...]:
    return tuple(
        re.findall(
            r"(?m)^\s*#\[((?:tokio::)?test(?:\([^]\n]*\))?)\]\s*\n"
            r"\s*(?:async\s+)?fn\s+([A-Za-z0-9_]+)",
            source,
        )
    )


def _manifest_base(source: str) -> str:
    source = re.sub(r"(?m)^autotests = false\n", "", source)
    source = re.sub(r"(?ms)^\[\[test\]\]\n.*?(?=^\[|\Z)", "", source)
    return source.rstrip() + "\n"


def _serial_members() -> set[str]:
    members: set[str] = set()
    for target in WAVE_TWO_TARGETS:
        if not target.serial:
            continue
        members.add(f"{target.package}/tests/{target.root}")
        members.update(f"{target.package}/tests/{path}" for _, path in target.modules)
    return members


def _normalized_wave_two_source(path: str, source: str) -> str:
    for target in WAVE_TWO_TARGETS:
        root = f"{target.package}/tests/{target.root}"
        if path != root:
            continue
        for module, module_path in target.modules:
            declaration = _wave_two_module_declaration(target, module, module_path)
            if source.count(declaration) != 1:
                raise AssertionError(f"{path}: module declaration is not unique")
            source = source.replace(declaration, "", 1)
        if target.serial:
            if source.count(SERIAL_GUARD_SOURCE) != 1:
                raise AssertionError(f"{path}: serial guard is not unique")
            source = source.replace(SERIAL_GUARD_SOURCE, "", 1)

    if path in _serial_members():
        expected_calls = len(_test_items(source))
        if source.count(SERIAL_CALL) != expected_calls:
            raise AssertionError(f"{path}: serial call count drifted")
        source = source.replace(SERIAL_CALL, "")
    elif SERIAL_CALL in source:
        raise AssertionError(f"{path}: unexpected serial call")
    return source


def validate_wave_two(sources: dict[str, str] | None = None) -> None:
    if sources is None:
        sources = _read_wave_two_sources()
    expected_paths = _wave_two_expected_paths()
    if set(sources) != expected_paths:
        raise AssertionError("wave-two source path inventory drifted")

    target_rows: list[str] = []
    module_rows: list[str] = []
    for package in _wave_two_packages():
        manifest_path = f"{package}/Cargo.toml"
        manifest = sources[manifest_path]
        package_table = manifest.split("\n[", 1)[0]
        if package_table.count("autotests = false") != 1:
            raise AssertionError(f"{package}: autotests must be disabled exactly once")
        targets = tuple(
            target for target in WAVE_TWO_TARGETS if target.package == package
        )
        expected_tables = [_wave_two_table(target) for target in targets]
        if _test_tables(manifest) != expected_tables:
            raise AssertionError(f"{package}: explicit target inventory drifted")
        base_digest = hashlib.sha256(_manifest_base(manifest).encode()).hexdigest()
        if base_digest != WAVE_TWO_MANIFEST_BASE_SHA256[package]:
            raise AssertionError(f"{package}: manifest content outside target tables drifted")

        for target in targets:
            features = ",".join(target.required_features)
            target_rows.append(f"{package}\0{target.target}\0{features}\n")
            root_path = f"{package}/tests/{target.root}"
            root = sources[root_path]
            if _module_rows(root) != target.modules:
                raise AssertionError(f"{root_path}: child module inventory drifted")
            for module, module_path in target.modules:
                marker = "dead_code" if module in target.dead_code_modules else ""
                module_rows.append(
                    f"{package}\0{target.root}\0{module}\0{module_path}\0{marker}\n"
                )
                declaration = _wave_two_module_declaration(
                    target, module, module_path
                )
                if root.count(declaration) != 1:
                    raise AssertionError(
                        f"{root_path}: child module declaration drifted"
                    )

    target_digest = hashlib.sha256("".join(sorted(target_rows)).encode()).hexdigest()
    if len(target_rows) != 13 or target_digest != WAVE_TWO_TARGET_INVENTORY_SHA256:
        raise AssertionError("wave-two target count or identity drifted")
    module_digest = hashlib.sha256("".join(sorted(module_rows)).encode()).hexdigest()
    if len(module_rows) != 15 or module_digest != WAVE_TWO_MODULE_INVENTORY_SHA256:
        raise AssertionError("wave-two module item count or identity drifted")

    serial_members = _serial_members()
    serial_test_count = 0
    for path in serial_members:
        source = sources[path]
        for _, name in _test_items(source):
            serial_test_count += 1
            first_statement = f"fn {name}() {{\n{SERIAL_CALL}"
            if source.count(first_statement) != 1:
                raise AssertionError(f"{path}: {name} is not directly serialized")
    if serial_test_count != 13:
        raise AssertionError("serialized test count drifted")

    test_rows: list[str] = []
    for path, opening_digest in sorted(WAVE_TWO_SOURCE_SHA256.items()):
        source = sources[path]
        normalized = _normalized_wave_two_source(path, source)
        digest = hashlib.sha256(normalized.encode()).hexdigest()
        if digest != opening_digest:
            raise AssertionError(f"{path}: executable source drifted")
        for attribute, name in _test_items(source):
            test_rows.append(f"{path}\0{attribute}\0{name}\n")
    test_digest = hashlib.sha256("".join(test_rows).encode()).hexdigest()
    if len(test_rows) != 64 or test_digest != WAVE_TWO_TEST_INVENTORY_SHA256:
        raise AssertionError("wave-two test ID, attribute, or order drifted")


def _replace_once(
    sources: dict[str, str], path: str, before: str, after: str
) -> dict[str, str]:
    mutated = dict(sources)
    if mutated[path].count(before) != 1:
        raise AssertionError(f"{path}: mutation anchor is not unique")
    mutated[path] = mutated[path].replace(before, after, 1)
    return mutated


class IntegrationTargetConsolidationTest(unittest.TestCase):
    def test_xtask_target_is_aggregated(self) -> None:
        validate(XTASK)

    def test_iroha_schema_target_is_aggregated(self) -> None:
        validate(SCHEMA)

    def test_wave_two_targets_are_aggregated(self) -> None:
        validate_wave_two()

    def test_wave_two_guard_rejects_mutations(self) -> None:
        sources = _read_wave_two_sources()
        validate_wave_two(sources)
        mutations = (
            _replace_once(
                sources,
                "crates/iroha_monitor/Cargo.toml",
                "autotests = false",
                "autotests = true",
            ),
            _replace_once(
                sources,
                "crates/iroha_monitor/tests/smoke.rs",
                '#[path = "attach_render.rs"]',
                '#[path = "invalid_credentials.rs"]',
            ),
            _replace_once(
                sources,
                "crates/iroha_derive/Cargo.toml",
                'required-features = ["trybuild-tests"]',
                "required-features = []",
            ),
            _replace_once(
                sources,
                "tools/soranet-handshake-harness/tests/fixtures_verify.rs",
                "fn canonical_fixtures_match_generator_output()",
                "fn canonical_fixtures_match_generator_output_mutated()",
            ),
            _replace_once(
                sources,
                "crates/iroha_derive/tests/config_base_ui.rs",
                SERIAL_CALL,
                "",
            ),
            _replace_once(
                sources,
                "crates/iroha_derive/tests/ui.rs",
                "        .unwrap_or_else(std::sync::PoisonError::into_inner)",
                "        .unwrap()",
            ),
        )
        body_mutation = dict(sources)
        body_mutation[
            "crates/iroha_primitives/tests/numeric_inspect.rs"
        ] += "\ntype Callback = fn();\n"
        extra_source = dict(sources)
        extra_source["crates/iroha_monitor/tests/unexpected.rs"] = (
            "#[test]\nfn unexpected() {}\n"
        )
        for mutation in (*mutations, body_mutation, extra_source):
            with self.subTest():
                with self.assertRaises(AssertionError):
                    validate_wave_two(mutation)


@dataclass(frozen=True)
class WaveThreeAggregate:
    package: str
    target: str
    root: str
    modules: tuple[tuple[str, str, str | None], ...]


WAVE_THREE_TARGETS = (
    ("crates/iroha", "musubi_archive_fetch_memory", "musubi_archive_fetch_memory.rs"),
    ("crates/iroha", "tx_confirmation", "tx_confirmation.rs"),
    ("crates/iroha", "tx_ttl", "tx_ttl.rs"),
    ("crates/iroha_p2p", "mod", "mod.rs"),
    ("crates/norito_derive", "strict_json", "strict_json.rs"),
    ("crates/sorafs_chunker", "vectors", "vectors.rs"),
    ("crates/sorafs_chunker", "one_gib", "one_gib.rs"),
    (
        "crates/sorafs_orchestrator",
        "orchestrator_parity",
        "orchestrator_parity.rs",
    ),
    ("crates/sorafs_orchestrator", "sorafs_cli", "sorafs_cli.rs"),
)

WAVE_THREE_AGGREGATES = (
    WaveThreeAggregate(
        package="crates/iroha",
        target="tx_ttl",
        root="tx_ttl.rs",
        modules=(("sm_signing", "sm_signing.rs", None),),
    ),
    WaveThreeAggregate(
        package="crates/iroha_p2p",
        target="mod",
        root="mod.rs",
        modules=(("retired_relay_surface", "retired_relay_surface.rs", None),),
    ),
    WaveThreeAggregate(
        package="crates/norito_derive",
        target="strict_json",
        root="strict_json.rs",
        modules=(("ui", "ui.rs", "trybuild-tests"),),
    ),
    WaveThreeAggregate(
        package="crates/sorafs_chunker",
        target="vectors",
        root="vectors.rs",
        modules=(("backpressure", "backpressure.rs", None),),
    ),
    WaveThreeAggregate(
        package="crates/sorafs_orchestrator",
        target="orchestrator_parity",
        root="orchestrator_parity.rs",
        modules=(
            ("multi_peer_fetch", "multi_peer_fetch.rs", None),
            ("taikai_cache", "taikai_cache.rs", None),
        ),
    ),
)

WAVE_THREE_TOP_LEVEL_RS = {
    "crates/iroha": (
        "musubi_archive_fetch_memory.rs",
        "sm_signing.rs",
        "tx_confirmation.rs",
        "tx_ttl.rs",
    ),
    "crates/iroha_p2p": ("mod.rs", "retired_relay_surface.rs"),
    "crates/norito_derive": ("strict_json.rs", "ui.rs"),
    "crates/sorafs_chunker": ("backpressure.rs", "one_gib.rs", "vectors.rs"),
    "crates/sorafs_orchestrator": (
        "multi_peer_fetch.rs",
        "orchestrator_parity.rs",
        "sorafs_cli.rs",
        "taikai_cache.rs",
    ),
}

WAVE_THREE_MANIFEST_BASE_SHA256 = {
    "crates/iroha": "693059e6642a42461357c2bb59b7171e131e9e7ff465fd4aa5fac676c27f63b4",
    "crates/iroha_p2p": "dad3d6d19e110a7a2785acf87fb35d20c2d3b0b3a7e1a16f221ba7248ef90cb8",
    "crates/norito_derive": "d093eec7685436db5a148bb34282979f162d6b3b33c058f83372584a566f54aa",
    "crates/sorafs_chunker": "7d651022b88c1c27bb6236ba14cfed1a653fe4db6f417586d342a0b227acc63d",
    "crates/sorafs_orchestrator": "95ecbb6e32564069ffbf0a88571ad69ad2c977c314cce46c250b2ec3de319a30",
}

WAVE_THREE_SOURCE_SHA256 = {
    "crates/iroha/tests/sm_signing.rs": "61670002c8ce09924f900f0700f61ef21f790d249b39565f3622550941927da9",
    "crates/iroha/tests/tx_ttl.rs": "7c17dbea1faa9569c7fe481a850f48fc59476316eac99d93c7509bdde62f6c82",
    "crates/iroha_p2p/tests/mod.rs": "3f464bb6ad0588884947c4e2c557bf8ce89ccaa0f8f83b872aab2ad9b1bcf118",
    "crates/iroha_p2p/tests/retired_relay_surface.rs": "e6199a3e93277a89ea49b2fcb22628a34f59473f26b7199a07201233d5bdd000",
    "crates/norito_derive/tests/strict_json.rs": "6dc05f698def2803c35c3e08526ba6f7fb29d7a7d86714662f415503f9de34cb",
    "crates/norito_derive/tests/ui.rs": "0d9b0871576c8f2c2f475c08c532a61e298959f2937236d0b24240b3e9010536",
    "crates/sorafs_chunker/tests/backpressure.rs": "6a633a8e836441f35e952ca526d225f5d8b60cb42bcf26d2db08635d91c39363",
    "crates/sorafs_chunker/tests/one_gib.rs": "d9457984ecff184b50f38cd4b791a84bb3bf518a8696d34fba2cb68dcf60d547",
    "crates/sorafs_chunker/tests/vectors.rs": "cec047ab1a41958fbc750ef390292a56bda9a9e271685c4525490aadce43c8f6",
    "crates/sorafs_orchestrator/tests/multi_peer_fetch.rs": "4ba24970d87483f3f0c5947080feb6abf14627d5f550a8098e08698adb5a4e30",
    "crates/sorafs_orchestrator/tests/orchestrator_parity.rs": "1a21ad0365822933346695375b32a860141f3f99e3043c3a726badaa4331c3b8",
    "crates/sorafs_orchestrator/tests/taikai_cache.rs": "46ec18a662250118359838f52847b071f39fac9189cb756ff5c641912ffa5f5d",
}

WAVE_THREE_TARGET_INVENTORY_SHA256 = (
    "431c72f60cf1d12c89002347d876ddbe72e3fb236f369da7eee9374c5664277d"
)
WAVE_THREE_MODULE_INVENTORY_SHA256 = (
    "d036081bdc704d58b0ac69272555570adb2d86437c3d3185333cd4e3c40c7ae0"
)
WAVE_THREE_TEST_INVENTORY_SHA256 = (
    "3bd3ea43b20a472f3e5656b3ae6096e8fe7a348136c6559b21549752a34f257e"
)
WAVE_THREE_DOC_PATH = "specs/sorafs/chunker_profile_authoring.md"
WAVE_THREE_OLD_DOC_COMMAND = (
    "cargo test --locked -p sorafs_chunker --test backpressure"
)
WAVE_THREE_NEW_DOC_COMMAND = (
    "cargo test --locked -p sorafs_chunker --test vectors backpressure"
)
WAVE_THREE_DOC_SHA256 = (
    "40303efff016a0900364bd4a89264f024c01eb2d4ea7e900398dfc3159268285"
)


def _wave_three_packages() -> tuple[str, ...]:
    return tuple(dict.fromkeys(package for package, _, _ in WAVE_THREE_TARGETS))


def _wave_three_table(target: str, root: str) -> str:
    return f'name = "{target}"\npath = "tests/{root}"'


def _wave_three_module_declaration(
    module: str, path: str, required_feature: str | None
) -> str:
    cfg = (
        f'#[cfg(feature = "{required_feature}")]\n'
        if required_feature is not None
        else ""
    )
    return f'\n{cfg}#[path = "{path}"]\nmod {module};\n'


def _wave_three_expected_paths() -> set[str]:
    paths = {f"{package}/Cargo.toml" for package in _wave_three_packages()}
    paths.update(WAVE_THREE_SOURCE_SHA256)
    paths.add(WAVE_THREE_DOC_PATH)
    return paths


def _read_wave_three_sources() -> dict[str, str]:
    return {
        path: (ROOT / path).read_text(encoding="utf-8")
        for path in sorted(_wave_three_expected_paths())
    }


def _wave_three_top_level_inventory() -> dict[str, tuple[str, ...]]:
    return {
        package: tuple(
            sorted(path.name for path in (ROOT / package / "tests").glob("*.rs"))
        )
        for package in _wave_three_packages()
    }


def _wave_three_test_items(source: str) -> tuple[tuple[str, str], ...]:
    items: list[tuple[str, str]] = []
    for block, name in re.findall(
        r"(?m)((?:^[ \t]*#\[[^\n]+\]\n)+)"
        r"^[ \t]*(?:async[ \t]+)?fn[ \t]+([A-Za-z0-9_]+)[ \t]*\(",
        source,
    ):
        attributes = tuple(line.strip() for line in block.splitlines())
        if "#[test]" not in attributes and not any(
            attribute.startswith("#[tokio::test") for attribute in attributes
        ):
            continue
        items.append(("\n".join(attributes), name))
    return tuple(items)


def _normalized_wave_three_source(path: str, source: str) -> str:
    for aggregate in WAVE_THREE_AGGREGATES:
        root = f"{aggregate.package}/tests/{aggregate.root}"
        if path != root:
            continue
        for module, module_path, required_feature in aggregate.modules:
            declaration = _wave_three_module_declaration(
                module, module_path, required_feature
            )
            if source.count(declaration) != 1:
                raise AssertionError(f"{path}: module declaration is not unique")
            source = source.replace(declaration, "", 1)
    return source


def validate_wave_three(
    sources: dict[str, str] | None = None,
    inventories: dict[str, tuple[str, ...]] | None = None,
) -> None:
    if sources is None:
        sources = _read_wave_three_sources()
    if set(sources) != _wave_three_expected_paths():
        raise AssertionError("wave-three source path inventory drifted")
    if inventories is None:
        inventories = _wave_three_top_level_inventory()
    if inventories != WAVE_THREE_TOP_LEVEL_RS:
        raise AssertionError("wave-three top-level Rust source inventory drifted")

    target_rows: list[str] = []
    for package in _wave_three_packages():
        manifest = sources[f"{package}/Cargo.toml"]
        package_table = manifest.split("\n[", 1)[0]
        if package_table.count("autotests = false") != 1:
            raise AssertionError(f"{package}: autotests must be disabled exactly once")
        targets = tuple(
            (target, root)
            for target_package, target, root in WAVE_THREE_TARGETS
            if target_package == package
        )
        expected_tables = [_wave_three_table(target, root) for target, root in targets]
        if _test_tables(manifest) != expected_tables:
            raise AssertionError(f"{package}: explicit target inventory drifted")
        base_digest = hashlib.sha256(_manifest_base(manifest).encode()).hexdigest()
        if base_digest != WAVE_THREE_MANIFEST_BASE_SHA256[package]:
            raise AssertionError(f"{package}: manifest content outside targets drifted")
        target_rows.extend(
            f"{package}\0{target}\0{root}\n" for target, root in targets
        )

    target_digest = hashlib.sha256("".join(sorted(target_rows)).encode()).hexdigest()
    if len(target_rows) != 9 or target_digest != WAVE_THREE_TARGET_INVENTORY_SHA256:
        raise AssertionError("wave-three target count or identity drifted")

    module_rows: list[str] = []
    for aggregate in WAVE_THREE_AGGREGATES:
        root_path = f"{aggregate.package}/tests/{aggregate.root}"
        root = sources[root_path]
        expected_modules = tuple(
            (module, path) for module, path, _ in aggregate.modules
        )
        if _module_rows(root) != expected_modules:
            raise AssertionError(f"{root_path}: child module inventory drifted")
        for module, module_path, required_feature in aggregate.modules:
            declaration = _wave_three_module_declaration(
                module, module_path, required_feature
            )
            if root.count(declaration) != 1:
                raise AssertionError(f"{root_path}: child declaration drifted")
            feature = required_feature or ""
            module_rows.append(
                f"{aggregate.package}\0{aggregate.root}\0{module}\0"
                f"{module_path}\0{feature}\n"
            )

    module_digest = hashlib.sha256("".join(sorted(module_rows)).encode()).hexdigest()
    if len(module_rows) != 6 or module_digest != WAVE_THREE_MODULE_INVENTORY_SHA256:
        raise AssertionError("wave-three module count or identity drifted")

    test_rows: list[str] = []
    for path, opening_digest in sorted(WAVE_THREE_SOURCE_SHA256.items()):
        source = sources[path]
        normalized = _normalized_wave_three_source(path, source)
        digest = hashlib.sha256(normalized.encode()).hexdigest()
        if digest != opening_digest:
            raise AssertionError(f"{path}: executable source drifted")
        for attributes, name in _wave_three_test_items(source):
            test_rows.append(f"{path}\0{attributes}\0{name}\n")
    test_digest = hashlib.sha256("".join(test_rows).encode()).hexdigest()
    if len(test_rows) != 36 or test_digest != WAVE_THREE_TEST_INVENTORY_SHA256:
        raise AssertionError("wave-three test ID, attribute, or order drifted")

    docs = sources[WAVE_THREE_DOC_PATH]
    if docs.count(WAVE_THREE_OLD_DOC_COMMAND) != 0:
        raise AssertionError("retired backpressure target command remains documented")
    if docs.count(WAVE_THREE_NEW_DOC_COMMAND) != 1:
        raise AssertionError("aggregated backpressure command drifted")
    docs_digest = hashlib.sha256(docs.encode()).hexdigest()
    if docs_digest != WAVE_THREE_DOC_SHA256:
        raise AssertionError("chunker profile authoring documentation drifted")


class WaveThreeIntegrationTargetConsolidationTest(unittest.TestCase):
    def test_wave_three_targets_are_aggregated(self) -> None:
        validate_wave_three()

    def test_wave_three_guard_rejects_mutations(self) -> None:
        sources = _read_wave_three_sources()
        inventories = _wave_three_top_level_inventory()
        validate_wave_three(sources, inventories)
        mutations = (
            _replace_once(
                sources,
                "crates/iroha/Cargo.toml",
                "autotests = false",
                "autotests = true",
            ),
            _replace_once(
                sources,
                "crates/iroha/Cargo.toml",
                'name = "tx_ttl"',
                'name = "sm_signing"',
            ),
            _replace_once(
                sources,
                "crates/sorafs_chunker/tests/vectors.rs",
                '#[path = "backpressure.rs"]',
                '#[path = "one_gib.rs"]',
            ),
            _replace_once(
                sources,
                "crates/norito_derive/tests/strict_json.rs",
                '#[cfg(feature = "trybuild-tests")]',
                '#[cfg(feature = "other")]',
            ),
            _replace_once(
                sources,
                "crates/sorafs_orchestrator/tests/taikai_cache.rs",
                "fn cache_admission_envelope_roundtrip_and_verify()",
                "fn cache_admission_envelope_roundtrip_and_verify_mutated()",
            ),
            _replace_once(
                sources,
                "crates/sorafs_chunker/tests/vectors.rs",
                '#[ignore = "utility for regenerating the fixture digest"]\n#[test]',
                '#[test]\n#[ignore = "utility for regenerating the fixture digest"]',
            ),
            _replace_once(
                sources,
                WAVE_THREE_DOC_PATH,
                WAVE_THREE_NEW_DOC_COMMAND,
                WAVE_THREE_OLD_DOC_COMMAND,
            ),
        )
        body_mutation = dict(sources)
        body_mutation["crates/iroha/tests/sm_signing.rs"] += "\ntype Callback = fn();\n"
        inventory_mutation = dict(inventories)
        inventory_mutation["crates/sorafs_chunker"] = tuple(
            sorted((*inventory_mutation["crates/sorafs_chunker"], "unexpected.rs"))
        )
        for mutation in (*mutations, body_mutation):
            with self.subTest():
                with self.assertRaises(AssertionError):
                    validate_wave_three(mutation, inventories)
        with self.assertRaises(AssertionError):
            validate_wave_three(sources, inventory_mutation)


# BEGIN WAVE FOUR INTEGRATION-TARGET CONTRACT
@dataclass(frozen=True)
class WaveFourTarget:
    package: str
    target: str
    root: str
    table_suffix: str = ""


@dataclass(frozen=True)
class WaveFourModule:
    package: str
    root: str
    module: str
    path: str
    required_feature: str | None
    leading_blank: bool
    trailing_blank: bool


WAVE_FOUR_GROUP_COMMENT = (
    "# Grouped integration-test harnesses keep `cargo test` from linking one "
    "binary per tests/*.rs file."
)

WAVE_FOUR_TARGETS = (
    WaveFourTarget(
        package="crates/iroha_core",
        target="swift_confidential_unshield_redeem",
        root="swift_confidential_unshield_redeem.rs",
        table_suffix=f"\n\n{WAVE_FOUR_GROUP_COMMENT}",
    ),
    WaveFourTarget(
        package="crates/iroha_core",
        target="iroha_core_group_01",
        root="grouped/group_01.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_core",
        target="iroha_core_group_02",
        root="grouped/group_02.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_core",
        target="iroha_core_group_03",
        root="grouped/group_03.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_core",
        target="iroha_core_group_04",
        root="grouped/group_04.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_core",
        target="iroha_core_group_05",
        root="grouped/group_05.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_data_model",
        target="iroha_data_model_group_01",
        root="grouped/group_01.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_data_model",
        target="iroha_data_model_group_02",
        root="grouped/group_02.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_data_model",
        target="query_serialization_allocations",
        root="query_serialization_allocations.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_data_model",
        target="block_signature_serialization_allocations",
        root="block_signature_serialization_allocations.rs",
    ),
    WaveFourTarget(
        package="crates/iroha_data_model_derive",
        target="derive_integration",
        root="derive_integration.rs",
    ),
)

WAVE_FOUR_MODULES = (
    WaveFourModule(
        package="crates/iroha_core",
        root="swift_confidential_unshield_redeem.rs",
        module="kaigi_privacy",
        path="kaigi_privacy.rs",
        required_feature="zk-tests",
        leading_blank=False,
        trailing_blank=True,
    ),
    WaveFourModule(
        package="crates/iroha_core",
        root="swift_confidential_unshield_redeem.rs",
        module="kagemusha_artifact_v4_streaming",
        path="kagemusha_artifact_v4_streaming.rs",
        required_feature=None,
        leading_blank=True,
        trailing_blank=False,
    ),
    WaveFourModule(
        package="crates/iroha_data_model",
        root="query_serialization_allocations.rs",
        module="ui",
        path="ui.rs",
        required_feature="trybuild-tests",
        leading_blank=True,
        trailing_blank=True,
    ),
    WaveFourModule(
        package="crates/iroha_data_model_derive",
        root="derive_integration.rs",
        module="ui",
        path="ui.rs",
        required_feature="trybuild-tests",
        leading_blank=False,
        trailing_blank=True,
    ),
)

WAVE_FOUR_ROOT_MODULES = {
    "crates/iroha_core/tests/swift_confidential_unshield_redeem.rs": (
        ("kaigi_privacy", "kaigi_privacy.rs"),
        (
            "kagemusha_artifact_v4_streaming",
            "kagemusha_artifact_v4_streaming.rs",
        ),
    ),
    "crates/iroha_data_model/tests/query_serialization_allocations.rs": (
        ("ui", "ui.rs"),
    ),
    "crates/iroha_data_model_derive/tests/derive_integration.rs": (
        ("ui", "ui.rs"),
        ("event_set", "event_set.rs"),
        ("has_origin", "has_origin.rs"),
        ("has_origin_generics", "has_origin_generics.rs"),
        ("id_eq_ord_hash", "id_eq_ord_hash.rs"),
        ("model_macro", "model_macro.rs"),
    ),
}

WAVE_FOUR_MANIFEST_BASE_SHA256 = {
    "crates/iroha_core": "e1a24cb5ce458f24abeca22d6cf4b65a982e5a031369c23a8ebed23077e6f797",
    "crates/iroha_data_model": "7b278a625c8d1e0caacc41bb91c9b13ce17b75ea154fa884e10cfb330313ea06",
    "crates/iroha_data_model_derive": "9d6ae350da056233903b17e51a6d6c616b39060a0e2916ace12b20604b13fbb1",
}

WAVE_FOUR_SOURCE_SHA256 = {
    "crates/iroha_core/tests/swift_confidential_unshield_redeem.rs": "158977f8837214618d7e9aa4623b58b0b278f71488528c9b015b682a1f74fb66",
    "crates/iroha_core/tests/kaigi_privacy.rs": "ca7b75630e8256409d8dc56a8432f576d0d1bd08680de03db625dcbd255dfc06",
    "crates/iroha_core/tests/kagemusha_artifact_v4_streaming.rs": "cd36e1946b112a27381760e32b6b67e52af3ef9bb94224c1a1025c224abf6bee",
    "crates/iroha_core/tests/common/world_fixture.rs": "0b5a48b01d34081f8f1948c4557c2aaf673ffea21f24a7680e76437e9ba6fdbf",
    "crates/iroha_data_model/tests/query_serialization_allocations.rs": "d737cadc0ed62a5f78aa95af8667edd7383d04ad7d5f28804b04114da1946133",
    "crates/iroha_data_model/tests/ui.rs": "f0993625a4ca603d9fd3fc5f7ce1e7423750d3a6d35a1495ddafe1e7702ecf48",
    "crates/iroha_data_model/tests/ui/instruction_registry.rs": "189319ce277d4b0132b7b0e5b141a9ff4a7d94ab86bf13cd03b49e1b6ce8d206",
    "crates/iroha_data_model_derive/tests/derive_integration.rs": "fb1e370d010a316c0fa64684f46fb0765ddbd414511076cd333fdd5dc4b31bee",
    "crates/iroha_data_model_derive/tests/ui.rs": "ee58c04ab7fe2e45c20d8b87b93609f92e53aaf50293dacae07a9f2f413a5fbf",
    "crates/iroha_data_model_derive/tests/event_set.rs": "67536f21a866e1ac68f68a9df213f5f1e08a78fab3cfad138450d619541bb66c",
    "crates/iroha_data_model_derive/tests/has_origin.rs": "9579c53c7990ec044edf810c407351e5d2222d689bce280aaac9b9009dd5bb46",
    "crates/iroha_data_model_derive/tests/has_origin_generics.rs": "db1b0aef78c84d5cb97732bb458256e8a711ba307b9b5dfb6aa412ece0fd2d87",
    "crates/iroha_data_model_derive/tests/id_eq_ord_hash.rs": "2b98cff909c8cbf935af4a3a46cc443a0cef5083f66cce95037f5b266d522c39",
    "crates/iroha_data_model_derive/tests/model_macro.rs": "6e9faf1bc6313b632ba3c9e11b0d4d939e1be13b5c69a581cfa504e845941f9b",
    "crates/iroha_data_model_derive/tests/ui_fail/has_origin_multiple_attributes.rs": "03e5d1cb9d30fdc387b6b7e1138cf8a56ac00c70d6993578d0bef5a1ea820f3d",
    "crates/iroha_data_model_derive/tests/ui_fail/has_origin_multiple_attributes.stderr": "03cf06e6678794f0fc896f348e2335a94d7658031cb869fd9f74553f03e9a99e",
    "crates/iroha_data_model_derive/tests/ui_fail/model_ffi_type_wrong_gate.rs": "b8e412b68bff8579d68362828a3bbe29dd83d1ade1ae118cb85bc609e6e049b9",
    "crates/iroha_data_model_derive/tests/ui_fail/model_ffi_type_wrong_gate.stderr": "5166e572bf6c8eb139b29d9d08da52b27aa5c43193af55271bad3314814eabe1",
    "crates/iroha_data_model_derive/tests/ui_fail/transparent_api_private_field.rs": "61b51f5784d564a37dc97dd787a1794188878e6b970dc02255131d2701de81e5",
    "crates/iroha_data_model_derive/tests/ui_fail/transparent_api_private_field.stderr": "6115d66683b51bbbdc1a44e61e7f4a09163c306fac0e91bd8b0ff82bf2d4a8f1",
    "crates/iroha_data_model_derive/tests/ui_fail/transparent_api_private_item.rs": "b8f62a0a4fbe02c5d906088464c52aa69f43f9700631735d3dab3fc9effd6f12",
    "crates/iroha_data_model_derive/tests/ui_fail/transparent_api_private_item.stderr": "39eb7491bd88f3bcbacf37ceae937e2c32b980964150f96f0aff5b77e42a4886",
}

WAVE_FOUR_ROOT_POST_SHA256 = {
    "crates/iroha_core/tests/swift_confidential_unshield_redeem.rs": "9fc54180668a1941a1900575d6da7d1f1d94a015f3699107bfccfc468339c667",
    "crates/iroha_data_model/tests/query_serialization_allocations.rs": "46bf47369cd6f874d9e3d7df5045f8b4e535d88b5090546e83d33b74be01854f",
    "crates/iroha_data_model_derive/tests/derive_integration.rs": "2d69bdcbee3299c4443e7dedae7349450468a1d650ad11fa4318c1a07ea3a90c",
}

WAVE_FOUR_FIXTURE_INVENTORIES = {
    "crates/iroha_data_model/tests/ui": ("instruction_registry.rs",),
    "crates/iroha_data_model_derive/tests/ui_pass": (),
    "crates/iroha_data_model_derive/tests/ui_fail": (
        "has_origin_multiple_attributes.rs",
        "has_origin_multiple_attributes.stderr",
        "model_ffi_type_wrong_gate.rs",
        "model_ffi_type_wrong_gate.stderr",
        "transparent_api_private_field.rs",
        "transparent_api_private_field.stderr",
        "transparent_api_private_item.rs",
        "transparent_api_private_item.stderr",
    ),
}

WAVE_FOUR_TARGET_INVENTORY_SHA256 = (
    "4df2bfa8cee076910556ab76fa53d0c43811acfae069d7874c064aa55df7b7d3"
)
WAVE_FOUR_MODULE_INVENTORY_SHA256 = (
    "c28fc0e6af8901152d4da26c538661e9cd865fac5e045ffdd83dc7a35f3db465"
)
WAVE_FOUR_TEST_INVENTORY_SHA256 = (
    "ca7bd522f46077145181d42a826d73102285d3db01e2a4205114bad17498c71e"
)


def _wave_four_packages() -> tuple[str, ...]:
    return tuple(dict.fromkeys(target.package for target in WAVE_FOUR_TARGETS))


def _wave_four_table(target: WaveFourTarget) -> str:
    return (
        f'name = "{target.target}"\npath = "tests/{target.root}"'
        f"{target.table_suffix}"
    )


def _wave_four_module_declaration(module: WaveFourModule) -> str:
    cfg = (
        f'#[cfg(feature = "{module.required_feature}")]\n'
        if module.required_feature is not None
        else ""
    )
    body = f'{cfg}#[path = "{module.path}"]\nmod {module.module};\n'
    leading = "\n" if module.leading_blank else ""
    trailing = "\n" if module.trailing_blank else ""
    return f"{leading}{body}{trailing}"


def _wave_four_expected_paths() -> set[str]:
    paths = {f"{package}/Cargo.toml" for package in _wave_four_packages()}
    paths.update(WAVE_FOUR_SOURCE_SHA256)
    return paths


def _read_wave_four_sources() -> dict[str, str]:
    return {
        path: (ROOT / path).read_text(encoding="utf-8")
        for path in sorted(_wave_four_expected_paths())
    }


def _wave_four_fixture_inventories() -> dict[str, tuple[str, ...]]:
    inventories: dict[str, tuple[str, ...]] = {}
    for directory in WAVE_FOUR_FIXTURE_INVENTORIES:
        fixture_dir = ROOT / directory
        inventories[directory] = tuple(
            sorted(path.name for path in fixture_dir.iterdir() if path.is_file())
        ) if fixture_dir.exists() else ()
    return inventories


def _normalized_wave_four_source(path: str, source: str) -> str:
    for module in WAVE_FOUR_MODULES:
        root = f"{module.package}/tests/{module.root}"
        if path != root:
            continue
        declaration = _wave_four_module_declaration(module)
        if source.count(declaration) != 1:
            raise AssertionError(f"{path}: module declaration is not unique")
        source = source.replace(declaration, "", 1)
    return source


def validate_wave_four(
    sources: dict[str, str] | None = None,
    fixture_inventories: dict[str, tuple[str, ...]] | None = None,
) -> None:
    if sources is None:
        sources = _read_wave_four_sources()
    if set(sources) != _wave_four_expected_paths():
        raise AssertionError("wave-four source path inventory drifted")
    if fixture_inventories is None:
        fixture_inventories = _wave_four_fixture_inventories()
    if fixture_inventories != WAVE_FOUR_FIXTURE_INVENTORIES:
        raise AssertionError("wave-four fixture inventory drifted")

    target_rows: list[str] = []
    for package in _wave_four_packages():
        manifest = sources[f"{package}/Cargo.toml"]
        package_table = manifest.split("\n[", 1)[0]
        if package_table.count("autotests = false") != 1:
            raise AssertionError(f"{package}: autotests must be disabled exactly once")
        targets = tuple(
            target for target in WAVE_FOUR_TARGETS if target.package == package
        )
        expected_tables = [_wave_four_table(target) for target in targets]
        if _test_tables(manifest) != expected_tables:
            raise AssertionError(f"{package}: explicit target inventory drifted")
        base_digest = hashlib.sha256(_manifest_base(manifest).encode()).hexdigest()
        if base_digest != WAVE_FOUR_MANIFEST_BASE_SHA256[package]:
            raise AssertionError(f"{package}: manifest content outside targets drifted")
        target_rows.extend(
            f"{package}\0{target.target}\0{target.root}\n" for target in targets
        )

    target_digest = hashlib.sha256("".join(sorted(target_rows)).encode()).hexdigest()
    if len(target_rows) != 11 or target_digest != WAVE_FOUR_TARGET_INVENTORY_SHA256:
        raise AssertionError("wave-four target count or identity drifted")

    for root, expected_modules in WAVE_FOUR_ROOT_MODULES.items():
        if _module_rows(sources[root]) != expected_modules:
            raise AssertionError(f"{root}: complete module inventory drifted")

    module_rows: list[str] = []
    for module in WAVE_FOUR_MODULES:
        root = f"{module.package}/tests/{module.root}"
        declaration = _wave_four_module_declaration(module)
        if sources[root].count(declaration) != 1:
            raise AssertionError(f"{root}: wave-four declaration drifted")
        feature = module.required_feature or ""
        module_rows.append(
            f"{module.package}\0{module.root}\0{module.module}\0"
            f"{module.path}\0{feature}\n"
        )
    module_digest = hashlib.sha256("".join(sorted(module_rows)).encode()).hexdigest()
    if len(module_rows) != 4 or module_digest != WAVE_FOUR_MODULE_INVENTORY_SHA256:
        raise AssertionError("wave-four module count or identity drifted")

    test_rows: list[str] = []
    for path, opening_digest in sorted(WAVE_FOUR_SOURCE_SHA256.items()):
        source = sources[path]
        expected_post_digest = WAVE_FOUR_ROOT_POST_SHA256.get(path)
        if expected_post_digest is not None:
            post_digest = hashlib.sha256(source.encode()).hexdigest()
            if post_digest != expected_post_digest:
                raise AssertionError(f"{path}: aggregate postimage drifted")
        normalized = _normalized_wave_four_source(path, source)
        digest = hashlib.sha256(normalized.encode()).hexdigest()
        if digest != opening_digest:
            raise AssertionError(f"{path}: opening source or body bytes drifted")
        for attributes, name in _wave_three_test_items(source):
            test_rows.append(f"{path}\0{attributes}\0{name}\n")
    test_digest = hashlib.sha256("".join(test_rows).encode()).hexdigest()
    if len(test_rows) != 25 or test_digest != WAVE_FOUR_TEST_INVENTORY_SHA256:
        raise AssertionError("wave-four test ID, attribute, or order drifted")


class WaveFourIntegrationTargetConsolidationTest(unittest.TestCase):
    def test_wave_four_targets_are_aggregated(self) -> None:
        validate_wave_four()

    def test_wave_four_guard_rejects_mutations(self) -> None:
        sources = _read_wave_four_sources()
        inventories = _wave_four_fixture_inventories()
        validate_wave_four(sources, inventories)
        mutations = (
            _replace_once(
                sources,
                "crates/iroha_core/Cargo.toml",
                "autotests = false",
                "autotests = true",
            ),
            _replace_once(
                sources,
                "crates/iroha_core/Cargo.toml",
                '[[test]]\nname = "swift_confidential_unshield_redeem"',
                '[[test]]\nname = "kaigi_privacy"\n'
                'path = "tests/kaigi_privacy.rs"\n'
                'required-features = ["zk-tests"]\n\n'
                '[[test]]\nname = "swift_confidential_unshield_redeem"',
            ),
            _replace_once(
                sources,
                "crates/iroha_core/Cargo.toml",
                'path = "tests/grouped/group_05.rs"',
                'path = "tests/grouped/group_04.rs"',
            ),
            _replace_once(
                sources,
                "crates/iroha_data_model/Cargo.toml",
                'trybuild-tests = ["dep:trybuild"]',
                "trybuild-tests = []",
            ),
            _replace_once(
                sources,
                "crates/iroha_core/tests/swift_confidential_unshield_redeem.rs",
                '#[cfg(feature = "zk-tests")]',
                '#[cfg(feature = "other")]',
            ),
            _replace_once(
                sources,
                "crates/iroha_core/tests/swift_confidential_unshield_redeem.rs",
                '#[path = "kagemusha_artifact_v4_streaming.rs"]',
                '#[path = "kaigi_privacy.rs"]',
            ),
            _replace_once(
                sources,
                "crates/iroha_data_model/tests/query_serialization_allocations.rs",
                '#[cfg(feature = "trybuild-tests")]',
                '#[cfg(feature = "other")]',
            ),
            _replace_once(
                sources,
                "crates/iroha_data_model_derive/tests/derive_integration.rs",
                '#[path = "event_set.rs"]\nmod event_set;\n'
                '#[path = "has_origin.rs"]\nmod has_origin;',
                '#[path = "has_origin.rs"]\nmod has_origin;\n'
                '#[path = "event_set.rs"]\nmod event_set;',
            ),
            _replace_once(
                sources,
                "crates/iroha_core/tests/kaigi_privacy.rs",
                "#[test]\nfn kaigi_privacy_join_fails_closed_until_authority_is_bound()",
                "#[ignore]\n#[test]\n"
                "fn kaigi_privacy_join_fails_closed_until_authority_is_bound()",
            ),
            _replace_once(
                sources,
                "crates/iroha_core/tests/kagemusha_artifact_v4_streaming.rs",
                "fn reader_writer_matches_in_memory_frame_and_descriptor()",
                "fn reader_writer_matches_in_memory_frame_and_descriptor_mutated()",
            ),
            _replace_once(
                sources,
                "crates/iroha_data_model/tests/ui.rs",
                "not(coverage)",
                "coverage",
            ),
            _replace_once(
                sources,
                "crates/iroha_data_model_derive/tests/ui_fail/transparent_api_private_item.stderr",
                "error[E0603]",
                "error[E9999]",
            ),
        )
        callback_mutation = dict(sources)
        callback_mutation[
            "crates/iroha_data_model_derive/tests/model_macro.rs"
        ] += "\ntype Callback = fn();\n"
        extra_source = dict(sources)
        extra_source["crates/iroha_data_model_derive/tests/unexpected.rs"] = (
            "#[test]\nfn unexpected() {}\n"
        )
        inventory_mutation = dict(inventories)
        inventory_mutation["crates/iroha_data_model_derive/tests/ui_fail"] = tuple(
            sorted(
                (*inventory_mutation["crates/iroha_data_model_derive/tests/ui_fail"],
                 "unexpected.rs")
            )
        )
        for index, mutation in enumerate((*mutations, callback_mutation, extra_source)):
            with self.subTest(index=index):
                with self.assertRaises(AssertionError):
                    validate_wave_four(mutation, inventories)
        with self.assertRaises(AssertionError):
            validate_wave_four(sources, inventory_mutation)


# END WAVE FOUR INTEGRATION-TARGET CONTRACT


# BEGIN EXACT12 RELEASE-HARNESS OWNERSHIP CONTRACT
EXACT12_RELEASE_TARGET = "privacy_release_network"
EXACT12_RELEASE_ROOT = "integration_tests/tests/privacy_release_network.rs"
EXACT12_REQUIRED_FEATURES = ("zk-stark", "privacy-release-evidence")
EXACT12_NETWORK_MODULES = (
    (
        "privacy_exact12_activation_network",
        "privacy_exact12_activation_network.rs",
    ),
    ("privacy_exact12_jindo_network", "privacy_exact12_jindo_network.rs"),
    (
        "privacy_exact12_orchard_pq_masp_network",
        "privacy_exact12_orchard_pq_masp_network.rs",
    ),
    (
        "privacy_exact12_retained_network",
        "privacy_exact12_retained_network.rs",
    ),
    (
        "privacy_exact12_zk_ams_vega_network",
        "privacy_exact12_zk_ams_vega_network.rs",
    ),
    (
        "privacy_exact12_zk_x509_network",
        "privacy_exact12_zk_x509_network.rs",
    ),
    ("zk_ace_localnet", "zk_ace_localnet.rs"),
)
EXACT12_REQUIRED_TESTS = {
    "privacy_exact12_activation_network": (
        "canonical_exact12_governance_survives_four_peer_activation_replay_and_restart"
    ),
    "privacy_exact12_jindo_network": (
        "canonical_jindo_direct_action_survives_four_peer_activation_replay_and_restart"
    ),
    "privacy_exact12_orchard_pq_masp_network": (
        "canonical_orchard_and_pq_masp_actions_survive_four_peer_da_replay_and_restart"
    ),
    "privacy_exact12_retained_network": (
        "canonical_retained_exact12_actions_survive_four_peer_adversarial_replay_and_restart"
    ),
    "privacy_exact12_zk_ams_vega_network": (
        "canonical_zk_ams_and_vega_actions_survive_four_validator_activation_replay_and_restart"
    ),
    "privacy_exact12_zk_x509_network": (
        "canonical_zk_x509_action_survives_four_peer_activation_replay_and_restart"
    ),
    "zk_ace_localnet": "zk_ace_privacy_transfer_fails_closed_taira_localnet",
}
NETWORK_FUNCTIONAL_ROOT = "integration_tests/tests/network_functional.rs"
NETWORK_FUNCTIONAL_MODULES = (
    ("concurrency", "concurrency.rs"),
    ("extra_functional", "extra_functional/mod.rs"),
    ("observer_sync", "observer_sync.rs"),
    ("sccp_route_governance", "sccp_route_governance.rs"),
)
EXACT12_README = "integration_tests/README.md"
EXACT12_AGENTS = "integration_tests/AGENTS.md"


def _integration_test_targets(manifest: str) -> tuple[tuple[str, str, str], ...]:
    targets: list[tuple[str, str, str]] = []
    for table in _test_tables(manifest):
        name_match = re.search(r'(?m)^name = "([^"]+)"$', table)
        path_match = re.search(r'(?m)^path = "([^"]+)"$', table)
        if name_match is None or path_match is None:
            raise AssertionError("integration_tests: malformed explicit test target")
        targets.append((name_match.group(1), path_match.group(1), table))
    return tuple(targets)


def _read_exact12_release_sources() -> dict[str, str]:
    manifest_path = "integration_tests/Cargo.toml"
    manifest = (ROOT / manifest_path).read_text(encoding="utf-8")
    paths = {
        manifest_path,
        EXACT12_README,
        EXACT12_AGENTS,
        *(f"integration_tests/tests/{path}" for _, path in EXACT12_NETWORK_MODULES),
    }
    paths.update(
        f"integration_tests/{path}"
        for _, path, _ in _integration_test_targets(manifest)
    )
    return {
        path: (ROOT / path).read_text(encoding="utf-8") for path in sorted(paths)
    }


def validate_exact12_release_harness(
    sources: dict[str, str] | None = None,
) -> None:
    if sources is None:
        sources = _read_exact12_release_sources()
    manifest = sources["integration_tests/Cargo.toml"]
    targets = _integration_test_targets(manifest)
    target_tables = {name: table for name, _, table in targets}
    if len(target_tables) != len(targets):
        raise AssertionError("integration_tests: duplicate explicit test target name")

    required_features = ", ".join(
        f'"{feature}"' for feature in EXACT12_REQUIRED_FEATURES
    )
    expected_release_table = (
        f'name = "{EXACT12_RELEASE_TARGET}"\n'
        'path = "tests/privacy_release_network.rs"\n'
        f"required-features = [{required_features}]"
    )
    if target_tables.get(EXACT12_RELEASE_TARGET) != expected_release_table:
        raise AssertionError(
            "privacy_release_network must require the exact release feature pair"
        )
    expected_ambient_table = (
        'name = "network_functional"\npath = "tests/network_functional.rs"'
    )
    if target_tables.get("network_functional") != expected_ambient_table:
        raise AssertionError("network_functional target contract drifted")

    release_root = sources[EXACT12_RELEASE_ROOT]
    if _module_rows(release_root) != EXACT12_NETWORK_MODULES:
        raise AssertionError(
            "privacy_release_network must own the exact seven-module inventory"
        )
    if re.search(r"(?m)^#!?\[cfg", release_root):
        raise AssertionError(
            "privacy_release_network module inventory must be unconditional"
        )

    ambient_root = sources[NETWORK_FUNCTIONAL_ROOT]
    if _module_rows(ambient_root) != NETWORK_FUNCTIONAL_MODULES:
        raise AssertionError(
            "network_functional must retain only its four ambient network modules"
        )

    owners = {module: [] for module, _ in EXACT12_NETWORK_MODULES}
    exact_paths = {path: module for module, path in EXACT12_NETWORK_MODULES}
    exact_modules = dict(EXACT12_NETWORK_MODULES)
    for target, path, _ in targets:
        root_path = f"integration_tests/{path}"
        if root_path not in sources:
            raise AssertionError(f"missing explicit target root {root_path}")
        for module, module_path in _module_rows(sources[root_path]):
            canonical_module = exact_paths.get(module_path)
            if canonical_module is None and module in exact_modules:
                canonical_module = module
            if canonical_module is not None:
                owners[canonical_module].append((target, module, module_path))
    for module, path in EXACT12_NETWORK_MODULES:
        expected_owner = [(EXACT12_RELEASE_TARGET, module, path)]
        if owners[module] != expected_owner:
            raise AssertionError(
                f"{module}: Exact12 module must have one release-harness owner; "
                f"found {owners[module]!r}"
            )

    allowed_crate_cfgs = {
        f'feature = "{feature}"' for feature in EXACT12_REQUIRED_FEATURES
    }
    for module, path in EXACT12_NETWORK_MODULES:
        source_path = f"integration_tests/tests/{path}"
        source = sources[source_path]
        crate_cfgs = set(re.findall(r"(?m)^#!\[cfg\(([^]]+)\)\]$", source))
        if not crate_cfgs.issubset(allowed_crate_cfgs):
            raise AssertionError(
                f"{module}: release module has an unsatisfied crate-level cfg"
            )
        tests = {name: attributes for attributes, name in _wave_three_test_items(source)}
        required_test = EXACT12_REQUIRED_TESTS[module]
        attributes = tests.get(required_test)
        if attributes is None:
            raise AssertionError(
                f"{module}: missing non-vacuous release test {required_test}"
            )
        if "ignore" in attributes or "cfg(" in attributes:
            raise AssertionError(
                f"{module}: release test {required_test} must be unconditional and non-ignored"
            )

    readme = sources[EXACT12_README]
    for statement in (
        "sole Cargo harness that owns all seven Exact12 network",
        "general-purpose `network_functional` harness retains only its",
    ):
        if statement not in readme:
            raise AssertionError("Exact12 release-harness ownership documentation drifted")
    agents = sources[EXACT12_AGENTS]
    if (
        "`privacy_release_network` (requires "
        "`zk-stark,privacy-release-evidence` and solely owns the seven Exact12 "
        "network modules)"
    ) not in agents:
        raise AssertionError("integration-test contributor guidance drifted")


class Exact12ReleaseHarnessOwnershipTest(unittest.TestCase):
    def test_exact12_release_harness_is_the_sole_non_vacuous_owner(self) -> None:
        validate_exact12_release_harness()

    def test_exact12_release_harness_guard_rejects_mutations(self) -> None:
        sources = _read_exact12_release_sources()
        validate_exact12_release_harness(sources)
        mutations = (
            _replace_once(
                sources,
                "integration_tests/Cargo.toml",
                'required-features = ["zk-stark", "privacy-release-evidence"]',
                'required-features = ["zk-stark"]',
            ),
            _replace_once(
                sources,
                EXACT12_RELEASE_ROOT,
                '#[path = "zk_ace_localnet.rs"]\nmod zk_ace_localnet;',
                "",
            ),
            _replace_once(
                sources,
                NETWORK_FUNCTIONAL_ROOT,
                '#[path = "sccp_route_governance.rs"]',
                '#[path = "privacy_exact12_jindo_network.rs"]\n'
                'mod privacy_exact12_jindo_network;\n'
                '#[path = "sccp_route_governance.rs"]',
            ),
            _replace_once(
                sources,
                EXACT12_RELEASE_ROOT,
                '#[path = "privacy_exact12_jindo_network.rs"]',
                '#[cfg(feature = "zk-stark")]\n'
                '#[path = "privacy_exact12_jindo_network.rs"]',
            ),
            _replace_once(
                sources,
                "integration_tests/tests/privacy_exact12_jindo_network.rs",
                '#[tokio::test(flavor = "multi_thread", worker_threads = 4)]\n'
                "async fn canonical_jindo_direct_action_survives_four_peer_activation_replay_and_restart",
                '#[ignore]\n'
                '#[tokio::test(flavor = "multi_thread", worker_threads = 4)]\n'
                "async fn canonical_jindo_direct_action_survives_four_peer_activation_replay_and_restart",
            ),
        )
        for index, mutation in enumerate(mutations):
            with self.subTest(index=index):
                with self.assertRaises(AssertionError):
                    validate_exact12_release_harness(mutation)


# END EXACT12 RELEASE-HARNESS OWNERSHIP CONTRACT


if __name__ == "__main__":
    unittest.main()
