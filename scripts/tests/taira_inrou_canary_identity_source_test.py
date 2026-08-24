"""Source guards for the fail-closed Taira Inrou canary identity preflight."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / "crates/iroha_cli/src/taira.rs").read_text(encoding="utf-8")


def _section(start: str, end: str) -> str:
    return SOURCE.split(start, 1)[1].split(end, 1)[0]


def test_identity_preflight_runs_before_any_canary_publication() -> None:
    run = _section("impl Run for InrouCanary", "fn ensure_canonical_taira_client_identity")
    ordered_calls = [
        "ensure_canonical_taira_client_identity",
        "preflight_taira_network_identity",
        "run_taira_inrou_canary_deployment",
        "verify_inrou_canary",
    ]
    positions = [run.index(call) for call in ordered_calls]
    assert positions == sorted(positions)


def test_timeout_validation_runs_before_any_canary_publication() -> None:
    run = _section("impl Run for InrouCanary", "fn ensure_canonical_taira_client_identity")
    assert run.index("validate_inrou_canary_timeout") < run.index(
        "run_taira_inrou_canary_deployment"
    )


def test_identity_preflight_binds_the_remote_puzzle_to_configured_taira() -> None:
    preflight = _section("fn preflight_taira_network_identity", "#[derive(Debug)]")
    assert 'join_url(public_root, "/v1/accounts/faucet/puzzle")' in preflight
    assert "if puzzle.status != 200" in preflight
    assert "validate_taira_puzzle_identity(body, &config.network_id)" in preflight

    validator = _section("fn validate_taira_puzzle_identity", "fn required_u64")
    assert "&network_id != expected_network_id" in validator
    assert "chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT" in validator
