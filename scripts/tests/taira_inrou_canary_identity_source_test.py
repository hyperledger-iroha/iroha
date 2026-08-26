"""Source guards for the fail-closed Taira Inrou canary identity preflight."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / "crates/iroha_cli/src/taira.rs").read_text(encoding="utf-8")


def _section(start: str, end: str) -> str:
    return SOURCE.split(start, 1)[1].split(end, 1)[0]


def test_identity_preflight_runs_before_exact_canary_preparation() -> None:
    run = _section("fn run_inrou_canary_exact", "fn require_inrou_binding_current")
    ordered_calls = [
        "validate_inrou_canary_timeout",
        "ensure_canonical_taira_client_identity",
        "normalize_root_url",
        "args.binding",
        "args.prepared_action",
        "require_inrou_binding_current",
        "preflight_taira_network_identity",
        "prove_inrou_predecessor_applied",
        "prepare_taira_inrou_canary_operation",
        "write_prepared_envelope",
    ]
    positions = [run.index(call) for call in ordered_calls]
    assert positions == sorted(positions)


def test_aggregate_canary_mutation_paths_are_absent() -> None:
    for retired in (
        "run_taira_inrou_canary_deployment",
        "find_applied_taira_inrou_mutation",
        "TairaInrouCanaryDeployment",
    ):
        assert retired not in SOURCE


def test_identity_preflight_binds_the_remote_puzzle_to_configured_taira() -> None:
    preflight = _section("fn preflight_taira_network_identity", "#[derive(Debug)]")
    assert 'join_url(public_root, "/v1/accounts/faucet/puzzle")' in preflight
    assert "if puzzle.status != 200" in preflight
    assert "validate_taira_puzzle_identity(body, &config.network_id)" in preflight

    validator = _section("fn validate_taira_puzzle_identity", "fn required_u64")
    assert "&network_id != expected_network_id" in validator
    assert "chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT" in validator
