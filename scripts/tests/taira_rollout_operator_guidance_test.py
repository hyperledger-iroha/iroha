from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TAIRA_DIR = ROOT / "configs" / "soranexus" / "taira"


def test_verify_soraswap_rollout_passes_expected_git_sha_to_mcp_check() -> None:
    source = (TAIRA_DIR / "verify_soraswap_rollout.sh").read_text(encoding="utf-8")

    assert 'EXPECTED_TAIRA_GIT_SHA="${EXPECTED_TAIRA_GIT_SHA:-}"' in source
    assert "--expected-git-sha)" in source
    assert 'mcp_cmd+=(--expected-git-sha "$EXPECTED_TAIRA_GIT_SHA")' in source
    assert "--validator-root)" in source
    assert 'mcp_cmd+=(--validator-root "$validator_root_spec")' in source
    assert 'mcp_cmd+=(--offline-asset-definition-id "$OFFLINE_ASSET_DEFINITION_ID")' in source
    assert 'mcp_cmd+=(--offline-expected-identity "$OFFLINE_EXPECTED_IDENTITY_PATH")' in source
    assert "public SoraSwap mutation/release paths cannot skip the mandatory Taira offline/fleet gate" in source


def test_rollout_bundle_manifest_followup_pins_mcp_and_soraswap_checks() -> None:
    source = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")

    assert (
        "check_mcp_rollout.sh --public-root https://<public-torii-root> "
        "--validator-root <label>=<validator-url> (once per validator) "
        "--require-all-validators --offline-asset-definition-id "
        "<registered-scale-2-ds-asset-definition-id> "
        "--offline-expected-identity "
        "/run/secrets/taira-offline-release-identity.json "
        "--write-config /run/secrets/taira-canary-client.toml --expected-git-sha "
        in source
    )
    assert (
        "verify_soraswap_rollout.sh --public-root https://<public-torii-root> "
        "--validator-root <label>=<validator-url> (once per validator) "
        "--offline-asset-definition-id <registered-scale-2-ds-asset-definition-id> "
        "--offline-expected-identity /run/secrets/taira-offline-release-identity.json "
        "--expected-git-sha "
        in source
    )
    assert '+ os.environ["GIT_HEAD"]' in source


def test_mcp_rollout_has_no_default_offline_asset_escape_hatch() -> None:
    source = (TAIRA_DIR / "check_mcp_rollout.sh").read_text(encoding="utf-8")

    assert 'OFFLINE_ASSET_DEFINITION_ID="${OFFLINE_ASSET_DEFINITION_ID:-}"' in source
    assert 'OFFLINE_EXPECTED_IDENTITY_PATH="${OFFLINE_EXPECTED_IDENTITY_PATH:-}"' in source
    assert 'OFFLINE_ASSET_DEFINITION_ID:-${ROLLOUT_CANARY_FAUCET_ASSET_ID}' not in source
    assert (
        "--offline-asset-definition-id must be one canonical unprefixed Base58 "
        "asset-definition ID" in source
    )
    assert "--offline-expected-identity is mandatory" in source
    assert "asset_scale is not exact Digital Shekel scale 2" in source


def test_public_cutover_cannot_skip_fleet_or_exact_commit() -> None:
    source = (TAIRA_DIR / "check_mcp_rollout.sh").read_text(encoding="utf-8")

    assert "TAIRA_RELEASE_VALIDATOR_COUNT=4" in source
    assert "public Taira rollout requires --require-all-validators" in source
    assert "public Taira rollout requires exactly ${TAIRA_RELEASE_VALIDATOR_COUNT}" in source
    assert "public Taira rollout requires --expected-git-sha with the exact full 40-character commit" in source
    assert "public Taira rollout requires at least three advancing validator fleet samples" in source
    assert "REQUIRE_EXACT_GIT_SHA=1" in source
    assert "Taira MCP local diagnostic checks passed; this is not public cutover evidence." in source


def test_readme_rollout_commands_are_executable_under_fail_closed_parser() -> None:
    readme = (TAIRA_DIR / "README.md").read_text(encoding="utf-8")
    command_lines = [
        line
        for line in readme.splitlines()
        if "check_mcp_rollout.sh" in line and line.lstrip().startswith(("- `bash", "`bash"))
    ]
    assert command_lines
    for line in command_lines:
        assert "--offline-asset-definition-id" in line, line
        assert "--offline-expected-identity" in line, line
        if "--public-root" in line:
            assert '"${TAIRA_VALIDATOR_ARGS[@]}"' in line, line
            assert "--require-all-validators" in line, line
            assert "--expected-git-sha" in line, line
