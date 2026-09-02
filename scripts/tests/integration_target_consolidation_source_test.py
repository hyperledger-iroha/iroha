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
    "crates/iroha_primitives": "e50a81a1a73a621cf671aaf80b82fbf490487e85518fc472177d10bb33947ed2",
    "crates/iroha_version_derive": "d652a0868d36147e19e196692830566858eed0a46b92b752ae451a93eaeb2977",
    "crates/iroha_zkp_halo2": "a36b5af199792222b2622bced949bac7599fea6e9d6137ab336543226f8152aa",
    "crates/soranet_pq": "09814d2ba4ed385c0683a0ecb7b8936b99f7df631391ce09dbe783e96c791c83",
    "mochi/mochi-core": "7dd684c46e9f4984673370b7b194c7ba029f99cab228c848586c98680cc987c5",
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
    "crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs": "6350e46bd567e6eb2ea523e725e50fb93d76878c76244b27220456a692a520f9",
    "crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs": "5cde89a58cffa1e77d578b20c44a92b30e924b6fe80eb9a3d56cc84b81039d23",
    "crates/soranet_pq/tests/kat_vectors.rs": "84b89d698051989013d4147dffd10d5261c741e352f213ad9150aa0b1c28321c",
    "crates/soranet_pq/tests/pq_kat.rs": "69491d6c86e58a8f3edb74d40ca46cd261801fbfbd2f459cfcd86cfe64fcc98a",
    "crates/sorafs_node/tests/cli.rs": "7e8d976e7fc2e1496d4d5524191faa0d8f0268138c13ad8e6c61273d9703ff43",
    "crates/sorafs_node/tests/pin_workflows.rs": "24e7d0547db730c3eb561c55d606b4f5526309b29432923d048f9b511e8f675d",
    "mochi/mochi-core/tests/composer_drafts.rs": "9aee3bade320bf3c19c9cb00cd8d2e8dab96250d3e91c058b97331bac4daf32b",
    "mochi/mochi-core/tests/torii_streams.rs": "091795ea64d5407e272f5e7327f813ae40b8e47d0cd93069e0e2ae31e8e0a05f",
    "mochi/mochi-integration/tests/readiness_smoke.rs": "c8bbc0479a27383548d726d9f5d427363205280635199f176008714d1f38cbc3",
    "mochi/mochi-integration/tests/supervisor.rs": "36948cb1d0bad6a6f4d09d82308cd2623a147236ebcad047b27fc64effb0a7cd",
    "tools/soranet-handshake-harness/tests/fixtures_verify.rs": "8487b4d970bdf1cdbab49a344bfbc07c7e7e2e4b3ca701f97c8811952d7e48dc",
    "tools/soranet-handshake-harness/tests/interop_parity.rs": "8f6fdaa1660770c86bd0d5a1c1c6c6b8cd61fdbf6bef95749c9694e1896aa745",
    "tools/soranet-handshake-harness/tests/perf_gate.rs": "daa16e6ec412927c7a6adc7056b34c22e69e9fb6be4fd934aa43bfa1c49dbff2",
    "tools/soranet-handshake-harness/tests/simulate_cli.rs": "bf336921bcf4832dccab35e2adf36f64d606db46b76c53b8e42d5f56c52ff9c2",
}

WAVE_TWO_TARGET_INVENTORY_SHA256 = (
    "088785f56a3ac0ab2a879a503c5e4d26a989e55b623a873d84a8438ee20a2640"
)
WAVE_TWO_MODULE_INVENTORY_SHA256 = (
    "931d70b2ecfbf8865ee9e1c9ed4486b4280d508cb109776b52b8e4397bed56f6"
)
WAVE_TWO_TEST_INVENTORY_SHA256 = (
    "5ed15c8aa7951bbab01d42c03820f284ccfc6e4a793b911eb66ed5495bf76d30"
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
    if len(test_rows) != 60 or test_digest != WAVE_TWO_TEST_INVENTORY_SHA256:
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
        modules=(("multi_peer_fetch", "multi_peer_fetch.rs", None),),
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
    ),
}

WAVE_THREE_MANIFEST_BASE_SHA256 = {
    "crates/iroha": "a9251faa0432d815ec56cbe0abac7a1bcd5ee90b597c728e6d0d36144011cc3d",
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
    "crates/sorafs_orchestrator/tests/orchestrator_parity.rs": "487b914ff97de086633d1652af71ec0d2be2400aaaf8cbfd61eacfa1df8283c4",
}

WAVE_THREE_TARGET_INVENTORY_SHA256 = (
    "431c72f60cf1d12c89002347d876ddbe72e3fb236f369da7eee9374c5664277d"
)
WAVE_THREE_MODULE_INVENTORY_SHA256 = (
    "1e977ed36600fa9bcdb34d789b5a391d8f812077478c190df367773bd16963a4"
)
WAVE_THREE_TEST_INVENTORY_SHA256 = (
    "3f54d6f255e599928461fe0ababee28e2d5544a6a5f676b1619e9779538635b1"
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
    if len(module_rows) != 5 or module_digest != WAVE_THREE_MODULE_INVENTORY_SHA256:
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
    if len(test_rows) != 34 or test_digest != WAVE_THREE_TEST_INVENTORY_SHA256:
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
