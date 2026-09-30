"""Source guards for the fail-closed Taira Inrou canary identity preflight."""

from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / "crates/iroha_cli/src/taira.rs").read_text(encoding="utf-8")
VALIDATOR = (ROOT / "crates/iroha_wallet/src/faucet_pow.rs").read_text(encoding="utf-8")


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


def validate_identity_preflight(source: str, owner: str) -> None:
    preflight = source.split("fn preflight_taira_network_identity", 1)[1].split("#[derive(Debug)]", 1)[0]
    assert 'use iroha_wallet::faucet_pow::{solve_account_faucet_claim, validate_puzzle_identity};' in source
    assert 'join_url(public_root, "/v1/accounts/faucet/puzzle")' in preflight
    assert "if puzzle.status != 200" in preflight
    assert "validate_puzzle_identity(body, &config.network_id, config.account_chain_discriminant)" in preflight
    validator = owner.split("pub fn validate_puzzle_identity(", 1)[1].split("fn required_u64", 1)[0]
    for check in (
        ".parse::<NetworkId>()",
        "network_id.to_string() != network_id_literal",
        "&network_id != expected_network_id",
        'u16::try_from(required_u64(puzzle, "chain_discriminant")?)',
        "chain_discriminant != expected_chain_discriminant",
    ):
        assert check in validator, check


def test_identity_preflight_binds_the_remote_puzzle_to_configured_taira() -> None:
    validate_identity_preflight(SOURCE, VALIDATOR)


@pytest.mark.parametrize("target,old,new", (
    ("source", "if puzzle.status != 200", "if puzzle.status == 200"),
    ("source", "validate_puzzle_identity(body, &config.network_id, config.account_chain_discriminant)",
     "validate_puzzle_identity(body, &config.network_id, DEFAULT_CHAIN_DISCRIMINANT)"),
    ("source", "validate_puzzle_identity(body, &config.network_id, config.account_chain_discriminant)",
     "validate_puzzle_identity(body, &NetworkId::default(), config.account_chain_discriminant)"),
    ("validator", "network_id.to_string() != network_id_literal", "false"),
    ("validator", "&network_id != expected_network_id", "false"),
    ("validator", "chain_discriminant != expected_chain_discriminant", "false"),
    ("validator", 'u16::try_from(required_u64(puzzle, "chain_discriminant")?)', '0u16'),
))
def test_identity_preflight_rejects_unbound_identity_mutations(target: str, old: str, new: str) -> None:
    source, owner = SOURCE, VALIDATOR
    original = source if target == "source" else owner
    assert original.count(old) == 1
    mutated = original.replace(old, new, 1)
    if target == "source":
        source = mutated
    else:
        owner = mutated
    with pytest.raises(AssertionError):
        validate_identity_preflight(source, owner)
