from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TAIRA_DIR = ROOT / "configs" / "soranexus" / "taira"


def test_verify_soraswap_rollout_passes_expected_git_sha_to_mcp_check() -> None:
    source = (TAIRA_DIR / "verify_soraswap_rollout.sh").read_text(encoding="utf-8")

    assert 'EXPECTED_TAIRA_GIT_SHA="${EXPECTED_TAIRA_GIT_SHA:-}"' in source
    assert "--expected-git-sha)" in source
    assert 'mcp_cmd+=(--expected-git-sha "$EXPECTED_TAIRA_GIT_SHA")' in source


def test_rollout_bundle_manifest_followup_pins_mcp_and_soraswap_checks() -> None:
    source = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")

    assert (
        "check_mcp_rollout.sh --public-root https://<public-torii-root> "
        "--write-config /run/secrets/taira-canary-client.toml --expected-git-sha "
        in source
    )
    assert "verify_soraswap_rollout.sh --expected-git-sha " in source
    assert '+ os.environ["GIT_HEAD"]' in source
