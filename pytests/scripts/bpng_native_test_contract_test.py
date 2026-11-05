"""Keep native BPNG development coverage separate from sealed qualification."""

from pathlib import Path
import re
import tomllib


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "integration_tests/tests/alias_registry_bootstrap_network.rs"
FEATURE = "bpng-release-qualification"
DEVELOPMENT = "bpng_development_bootstrap_survives_four_peer_retained_kura_catalog_expansion"
RELEASE = "bpng_native_bootstrap_survives_four_peer_retained_kura_catalog_expansion"


def entry(source: str, name: str) -> tuple[str, str]:
    """Read a small entry wrapper and the attributes immediately preceding it."""
    match = re.search(
        rf"((?:#\[[^\n]+\]\n)+)async fn {name}\(\)\s*->\s*Result<\(\)>\s*\{{(.*?)\n\}}",
        source,
        re.DOTALL,
    )
    assert match, name
    return match[1], match[2]


def test_default_and_release_entries_share_the_complete_native_scenario() -> None:
    source = SOURCE.read_text()
    development_attributes, development = entry(source, DEVELOPMENT)
    release_attributes, release = entry(source, RELEASE)
    assert "cfg(" not in development_attributes
    assert f'#[cfg(feature = "{FEATURE}")]' in release_attributes
    assert "ignore" not in development_attributes + release_attributes
    for body, prerequisite in (
        (development, "required_development_prebuilt_binaries()?;"),
        (release, "required_prebuilt_binaries()?;"),
    ):
        assert body.index(prerequisite) < body.index("run_bpng_retained_kura_scenario(")
        assert body.rstrip().endswith(".await")
        assert "if " not in body and "match " not in body
    shared = source.split("async fn run_bpng_retained_kura_scenario(", 1)[1]
    shared = shared.split("\n#[test]", 1)[0]
    assert "required_prebuilt_binaries" not in shared
    assert "required_development_prebuilt_binaries" not in shared
    assert "assert_same_authenticated_prefix(&after_restart_evidence)?;" in shared
    assert "skipping is forbidden" in shared
    harness = (ROOT / "integration_tests/tests/network_functional.rs").read_text()
    assert harness.startswith(
        '//! Grouped integration harness for network churn and functional network scenarios.\n'
        '#[path = "alias_registry_bootstrap_network.rs"]\nmod alias_registry_bootstrap_network;'
    )


def test_release_feature_has_a_concrete_nonignored_exact_invocation() -> None:
    manifest = tomllib.loads((ROOT / "integration_tests/Cargo.toml").read_text())
    assert manifest["features"][FEATURE] == []
    assert FEATURE not in manifest["features"]["default"]
    target = next(test for test in manifest["test"] if test["name"] == "network_functional")
    assert "required-features" not in target
    contract = (ROOT / "specs/bpng_native_bootstrap_test_contract.md").read_text()
    assert f"--features {FEATURE} --no-run --message-format=json-render-diagnostics" in contract
    assert f"--exact alias_registry_bootstrap_network::{RELEASE}" in contract
    assert "--ignored" not in contract


def test_development_admission_probes_all_tools_without_configuration() -> None:
    source = SOURCE.read_text()
    body = source.split("fn required_development_prebuilt_binaries()", 1)[1].split("\n#[test]", 1)[0]
    assert 'development_source_revision(option_env!("VERGEN_GIT_SHA"))?' in body
    for environment in ("TEST_NETWORK_BIN_IROHAD", "TEST_NETWORK_BIN_IROHA", "KAGAMI_BIN"):
        assert f'("{environment}",' in body
    assert "resolve_development_binary(&path)?" in body
    assert ".env_clear()" in body and ".current_dir(directory.path())" in body
    assert "validate_development_tool_identity(" in body
    assert "resolve_release_prebuilt_binary" not in body


def test_release_entry_retains_sealed_source_and_native_bundle_prerequisites() -> None:
    source = SOURCE.read_text()
    strict = source.split("fn required_prebuilt_binaries()", 1)[1].split("\n#[derive", 1)[0]
    assert 'std::env::var("IROHA_TEST_SKIP_BUILD").as_deref() == Ok("1")' in strict
    assert 'required_exact_env("IROHA_TEST_BUILD_PROFILE")? == "release"' in strict
    assert 'required_exact_env("PROFILE")? == "release"' in strict
    assert "verify_clean_release_source(&repo_root)?" in strict
    assert "resolve_release_prebuilt_binary(kind)?" in strict
    assert "resolved.diagnostic_path()?" in strict
    assert "resolve_development_binary" not in strict
    assert '["nexus", "staking",' not in source
