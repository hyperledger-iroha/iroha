"""Focused tests for the public-only Taira NEVO genesis composer."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts/compose_taira_nevo_reset_genesis.py"
SPEC = importlib.util.spec_from_file_location("compose_taira_nevo_reset_genesis", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
composer = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = composer
SPEC.loader.exec_module(composer)

TAIRA_DIR = REPO_ROOT / "configs/soranexus/taira"
BASE_GENESIS = TAIRA_DIR / "genesis.json"
BASE_CONFIG = TAIRA_DIR / "config.toml"
EXAMPLE_INPUTS = TAIRA_DIR / "nevo-reset-public-inputs.example.json"


PUBLIC_TEST_KEYS = (
    bytes.fromhex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
    bytes.fromhex("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"),
    bytes.fromhex("fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025"),
    bytes.fromhex("ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf"),
)


def _account(index: int) -> str:
    canonical = composer.ED25519_SINGLE_CONTROLLER_PREFIX + PUBLIC_TEST_KEYS[index]
    return composer._encode_taira_i105_account(canonical)


def _token_hash(label: str) -> str:
    return f"blake3:{hashlib.sha256(label.encode('ascii')).hexdigest()}"


def _public_payload() -> dict[str, str]:
    return {
        "schema": composer.PUBLIC_INPUT_SCHEMA,
        "onboarding_authority_account_id": _account(0),
        "api_signer_account_id": _account(1),
        "dpn_inori_account_id": _account(2),
        "dpn_epr_guard_account_id": _account(3),
        "is2_onboarding_token_hash": _token_hash("synthetic-is2-token-hash"),
        "dpn_onboarding_token_hash": _token_hash("synthetic-dpn-token-hash"),
    }


def _write_public_inputs(tmp_path: Path, payload: dict[str, str] | None = None) -> Path:
    path = tmp_path / "public-inputs.json"
    path.write_text(
        json.dumps(payload or _public_payload(), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return path


def _loaded(tmp_path: Path):
    public_path = _write_public_inputs(tmp_path)
    inputs, canonical_inputs = composer.load_public_inputs(public_path)
    genesis, genesis_bytes = composer.load_base_genesis(BASE_GENESIS)
    config, config_bytes = composer.load_base_config(BASE_CONFIG, genesis)
    return (
        public_path,
        inputs,
        canonical_inputs,
        genesis,
        genesis_bytes,
        config,
        config_bytes,
    )


def test_composition_is_deterministic_unsigned_and_secret_free(tmp_path: Path) -> None:
    (
        _,
        inputs,
        canonical_inputs,
        base,
        base_bytes,
        config,
        config_bytes,
    ) = _loaded(tmp_path)
    first = composer.compose_genesis(base, inputs, config)
    second = composer.compose_genesis(
        json.loads(base_bytes.decode("utf-8")), inputs, config
    )
    first_bytes = composer._pretty_json_bytes(first)
    second_bytes = composer._pretty_json_bytes(second)
    assert first_bytes == second_bytes
    assert inputs.is2_onboarding_token_hash.encode() not in first_bytes
    assert inputs.dpn_onboarding_token_hash.encode() not in first_bytes
    assert b"private_key" not in first_bytes.lower()
    assert b"signed_genesis" not in first_bytes.lower()
    assert not composer._contains_retired_namespace(first_bytes.decode("utf-8"))

    overlay = first["transactions"][-1]
    assert overlay["ivm_triggers"] == []
    assert overlay["topology"] == []
    keys = [next(iter(instruction)) for instruction in overlay["instructions"]]
    assert keys == (
        ["Register"] * 4
        + ["Mint"] * 4
        + ["EnsureAlias"] * 6
        + ["Grant"] * 9
        + ["EnrollFeeSponsorBeneficiary"] * 4
    )
    registered_accounts = [
        instruction["Register"]["Account"]["id"]
        for instruction in overlay["instructions"]
        if "Register" in instruction
    ]
    assert registered_accounts == [
        inputs.onboarding_authority_account_id,
        inputs.api_signer_account_id,
        inputs.dpn_inori_account_id,
        inputs.dpn_epr_guard_account_id,
    ]
    funded_accounts = [
        instruction["Mint"]["Asset"]["destination"].rsplit("#", 1)[1]
        for instruction in overlay["instructions"]
        if "Mint" in instruction
    ]
    assert funded_accounts == registered_accounts
    alias_targets = [
        composer._ensure_alias_target(instruction)
        for instruction in overlay["instructions"]
        if "EnsureAlias" in instruction
    ]
    assert alias_targets == [
        ("dataspace", "dpn"),
        ("dataspace", "is2"),
        ("domain", "nevo.dpn"),
        ("account_alias", "admin@universal"),
        ("account_alias", "inori@universal"),
        ("account_alias", "source_guard@universal"),
    ]
    account_alias_intents = [
        instruction["EnsureAlias"]["intent"]["intent"]
        for instruction in overlay["instructions"]
        if instruction.get("EnsureAlias", {}).get("intent", {}).get("kind")
        == "account_alias"
    ]
    assert [intent["target_account"] for intent in account_alias_intents] == [
        inputs.api_signer_account_id,
        inputs.dpn_inori_account_id,
        inputs.dpn_epr_guard_account_id,
    ]
    assert all(
        intent["provision"] == {"kind": "existing", "value": None}
        and intent["role"] == {"kind": "primary", "value": None}
        and intent["alias"]["dataspace_id"] == 0
        and intent["alias"]["canonical_name"]["domain"] is None
        for intent in account_alias_intents
    )
    grants = [
        instruction["Grant"]["Permission"]["object"]["name"]
        for instruction in overlay["instructions"]
        if "Grant" in instruction
    ]
    assert grants == [
        "CanRegisterAccount",
        "CanEnrollFeeSponsorProgram",
        "CanRegisterSmartContractCode",
        "DpnAdmin",
        "DpnUser",
        "DpnInori",
        "DpnSettlement",
        "DpnUser",
        "DpnEprGuard",
    ]
    dpn_grants = [
        instruction["Grant"]["Permission"]
        for instruction in overlay["instructions"]
        if instruction.get("Grant", {})
        .get("Permission", {})
        .get("object", {})
        .get("name", "")
        .startswith("Dpn")
    ]
    assert all(grant["object"]["payload"] is None for grant in dpn_grants)
    settlement_holders = [
        grant["destination"]
        for grant in dpn_grants
        if grant["object"]["name"] == "DpnSettlement"
    ]
    assert settlement_holders == [inputs.dpn_inori_account_id]
    enrolled_accounts = [
        instruction["EnrollFeeSponsorBeneficiary"]["beneficiary"]
        for instruction in overlay["instructions"]
        if "EnrollFeeSponsorBeneficiary" in instruction
    ]
    assert enrolled_accounts == registered_accounts

    review = composer.build_review_manifest(
        inputs=inputs,
        canonical_inputs=canonical_inputs,
        config=config,
        base_genesis_bytes=base_bytes,
        base_config_bytes=config_bytes,
        unsigned_genesis_bytes=first_bytes,
        instruction_count=len(overlay["instructions"]),
    )
    assert review["state"] == "unsigned_operator_review_required"
    assert review["unsigned_genesis_sha256"] == hashlib.sha256(first_bytes).hexdigest()
    assert review["secret_boundary"] == {
        "raw_tokens_accepted": False,
        "private_keys_accepted": False,
        "genesis_signed": False,
    }
    assert [row["token_hash"] for row in review["credential_hash_bindings"]] == [
        inputs.is2_onboarding_token_hash,
        inputs.dpn_onboarding_token_hash,
    ]
    assert review["public_identities"] == {
        "onboarding_authority_account_id": inputs.onboarding_authority_account_id,
        "api_signer_account_id": inputs.api_signer_account_id,
        "dpn_inori_account_id": inputs.dpn_inori_account_id,
        "dpn_epr_guard_account_id": inputs.dpn_epr_guard_account_id,
    }
    assert review["genesis_overlay"]["dpn_permission_grants"] == [
        {
            "account_id": inputs.api_signer_account_id,
            "permissions": ["DpnAdmin", "DpnUser"],
        },
        {
            "account_id": inputs.dpn_inori_account_id,
            "permissions": ["DpnInori", "DpnSettlement", "DpnUser"],
        },
        {
            "account_id": inputs.dpn_epr_guard_account_id,
            "permissions": ["DpnEprGuard"],
        },
    ]
    assert review["genesis_overlay"]["contract_deployment_permission_grant"] == {
        "account_id": inputs.api_signer_account_id,
        "permission": "CanRegisterSmartContractCode",
        "payload": None,
    }
    assert (
        review["genesis_overlay"]["dpn_settlement_holder_account_id"]
        == inputs.dpn_inori_account_id
    )


def test_review_verification_recomposes_exact_unsigned_genesis(tmp_path: Path) -> None:
    (
        _, inputs, canonical_inputs, base, base_bytes, config, config_bytes
    ) = _loaded(tmp_path)
    composed = composer.compose_genesis(base, inputs, config)
    unsigned = composer._pretty_json_bytes(composed)
    review = composer.build_review_manifest(
        inputs=inputs,
        canonical_inputs=canonical_inputs,
        config=config,
        base_genesis_bytes=base_bytes,
        base_config_bytes=config_bytes,
        unsigned_genesis_bytes=unsigned,
        instruction_count=len(composed["transactions"][-1]["instructions"]),
    )

    assert composer.verify_reviewed_payloads(
        unsigned_genesis_bytes=unsigned,
        review_bytes=composer._pretty_json_bytes(review),
        base_genesis_bytes=base_bytes,
        base_config_bytes=config_bytes,
    ) == review


def test_review_verification_rejects_genesis_or_token_hash_splice(tmp_path: Path) -> None:
    (
        _, inputs, canonical_inputs, base, base_bytes, config, config_bytes
    ) = _loaded(tmp_path)
    composed = composer.compose_genesis(base, inputs, config)
    unsigned = composer._pretty_json_bytes(composed)
    review = composer.build_review_manifest(
        inputs=inputs,
        canonical_inputs=canonical_inputs,
        config=config,
        base_genesis_bytes=base_bytes,
        base_config_bytes=config_bytes,
        unsigned_genesis_bytes=unsigned,
        instruction_count=len(composed["transactions"][-1]["instructions"]),
    )
    spliced = json.loads(json.dumps(review))
    spliced["credential_hash_bindings"][1]["token_hash"] = _token_hash(
        "spliced-dpn-token-hash"
    )

    with pytest.raises(composer.CompositionError, match="recomposition"):
        composer.verify_reviewed_payloads(
            unsigned_genesis_bytes=unsigned + b" ",
            review_bytes=composer._pretty_json_bytes(review),
            base_genesis_bytes=base_bytes,
            base_config_bytes=config_bytes,
        )
    with pytest.raises(composer.CompositionError, match="closed review"):
        composer.verify_reviewed_payloads(
            unsigned_genesis_bytes=unsigned,
            review_bytes=composer._pretty_json_bytes(spliced),
            base_genesis_bytes=base_bytes,
            base_config_bytes=config_bytes,
        )


def test_cli_publishes_new_mode_0600_outputs_and_refuses_replacement(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    public_inputs = _write_public_inputs(tmp_path)
    output = tmp_path / "nevo-reset.genesis.json"
    review = tmp_path / "nevo-reset.review.json"
    original_digest = hashlib.sha256(BASE_GENESIS.read_bytes()).hexdigest()
    arguments = [
        "--public-inputs",
        str(public_inputs),
        "--base-genesis",
        str(BASE_GENESIS),
        "--base-config",
        str(BASE_CONFIG),
        "--output-genesis",
        str(output),
        "--review-out",
        str(review),
    ]
    assert composer.main(arguments) == 0
    stdout = capsys.readouterr().out
    assert "unsigned_genesis_sha256=" in stdout
    assert stat_mode(output) == 0o600
    assert stat_mode(review) == 0o600
    assert output != BASE_GENESIS
    assert hashlib.sha256(BASE_GENESIS.read_bytes()).hexdigest() == original_digest
    review_payload = json.loads(review.read_text(encoding="utf-8"))
    assert review_payload["unsigned_genesis_sha256"] == hashlib.sha256(
        output.read_bytes()
    ).hexdigest()

    assert composer.main(arguments) == 2
    assert "refusing to overwrite existing output" in capsys.readouterr().err


def test_cli_imports_only_its_sealed_sibling_closure_in_isolated_mode() -> None:
    result = subprocess.run(
        [sys.executable, "-I", "-S", str(SCRIPT), "--help"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr


def stat_mode(path: Path) -> int:
    return path.stat().st_mode & 0o777


def test_output_paths_can_never_alias_checked_in_or_source_files(tmp_path: Path) -> None:
    public_inputs = _write_public_inputs(tmp_path)
    review = tmp_path / "review.json"
    with pytest.raises(composer.CompositionError, match="must not overwrite"):
        composer.validate_output_paths(
            public_inputs=public_inputs,
            base_genesis=BASE_GENESIS,
            base_config=BASE_CONFIG,
            output_genesis=BASE_GENESIS,
            review_out=review,
        )
    with pytest.raises(composer.CompositionError, match="must not overwrite"):
        composer.validate_output_paths(
            public_inputs=public_inputs,
            base_genesis=BASE_GENESIS,
            base_config=BASE_CONFIG,
            output_genesis=tmp_path / "output.json",
            review_out=public_inputs,
        )


@pytest.mark.parametrize(
    "secret_field",
    ["private_key", "private-key-file", "raw_token", "api_token", "seed_hex", "secret"],
)
def test_public_input_schema_rejects_secret_bearing_fields(
    tmp_path: Path, secret_field: str
) -> None:
    payload = _public_payload()
    payload[secret_field] = "not-accepted"
    path = _write_public_inputs(tmp_path, payload)
    with pytest.raises(composer.CompositionError, match="secret-bearing field"):
        composer.load_public_inputs(path)


def test_public_input_schema_rejects_duplicate_or_weak_credentials(tmp_path: Path) -> None:
    payload = _public_payload()
    payload["dpn_onboarding_token_hash"] = payload["is2_onboarding_token_hash"]
    path = _write_public_inputs(tmp_path, payload)
    with pytest.raises(composer.CompositionError, match="distinct token hashes"):
        composer.load_public_inputs(path)

    payload = _public_payload()
    payload["dpn_onboarding_token_hash"] = "blake3:" + ("00" * 32)
    path.write_text(json.dumps(payload), encoding="utf-8")
    with pytest.raises(composer.CompositionError, match="too weak"):
        composer.load_public_inputs(path)


@pytest.mark.parametrize(
    ("duplicate_field", "source_field"),
    [
        ("api_signer_account_id", "onboarding_authority_account_id"),
        ("dpn_inori_account_id", "onboarding_authority_account_id"),
        ("dpn_epr_guard_account_id", "onboarding_authority_account_id"),
        ("dpn_inori_account_id", "api_signer_account_id"),
        ("dpn_epr_guard_account_id", "api_signer_account_id"),
        ("dpn_epr_guard_account_id", "dpn_inori_account_id"),
    ],
)
def test_public_input_schema_rejects_every_reused_account_pair(
    tmp_path: Path, duplicate_field: str, source_field: str
) -> None:
    payload = _public_payload()
    payload[duplicate_field] = payload[source_field]
    path = _write_public_inputs(tmp_path, payload)
    with pytest.raises(composer.CompositionError, match="pairwise distinct"):
        composer.load_public_inputs(path)


def test_public_input_schema_rejects_noncanonical_account(tmp_path: Path) -> None:
    payload = _public_payload()
    payload["api_signer_account_id"] = "REPLACE_WITH_ACCOUNT"
    path = _write_public_inputs(tmp_path, payload)
    with pytest.raises(composer.CompositionError, match="canonical domainless"):
        composer.load_public_inputs(path)


def test_public_input_schema_rejects_checksum_valid_invalid_ed25519_key(
    tmp_path: Path,
) -> None:
    payload = _public_payload()
    invalid = composer.ED25519_SINGLE_CONTROLLER_PREFIX + bytes([0x41]) * 32
    payload["api_signer_account_id"] = composer._encode_taira_i105_account(invalid)
    path = _write_public_inputs(tmp_path, payload)
    with pytest.raises(composer.CompositionError, match="invalid Ed25519 public key"):
        composer.load_public_inputs(path)


def test_retired_sample_namespace_is_rejected_case_insensitively(tmp_path: Path) -> None:
    retired = "wonder" + "land"
    payload = json.loads(BASE_GENESIS.read_text(encoding="utf-8"))
    payload["retired_fixture"] = retired.swapcase()
    path = tmp_path / "base-genesis.json"
    path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    with pytest.raises(composer.CompositionError, match="retired sample namespace"):
        composer.load_base_genesis(path)


def test_example_is_placeholder_only_and_fails_until_operator_fills_it() -> None:
    example = json.loads(EXAMPLE_INPUTS.read_text(encoding="utf-8"))
    assert set(example) == composer.EXPECTED_PUBLIC_INPUT_FIELDS
    assert all(
        "REPLACE" in value
        for key, value in example.items()
        if key != "schema"
    )
    with pytest.raises(composer.CompositionError):
        composer.load_public_inputs(EXAMPLE_INPUTS)


def test_dry_run_writes_nothing_and_reports_same_unsigned_digest(tmp_path: Path) -> None:
    public_inputs = _write_public_inputs(tmp_path)
    arguments = argparse.Namespace(
        public_inputs=public_inputs,
        base_genesis=BASE_GENESIS,
        base_config=BASE_CONFIG,
        output_genesis=None,
        review_out=None,
        dry_run=True,
    )
    review = composer.run(arguments)
    assert review["state"] == "unsigned_operator_review_required"
    assert set(tmp_path.iterdir()) == {public_inputs}


def test_pristine_base_guard_rejects_partial_overlay_reuse(tmp_path: Path) -> None:
    _, inputs, _, base, _, config, _ = _loaded(tmp_path)
    composed = composer.compose_genesis(base, inputs, config)
    with pytest.raises(composer.CompositionError, match="already"):
        composer.compose_genesis(composed, inputs, config)


def test_pristine_base_guard_rejects_contract_account_alias_collision(
    tmp_path: Path,
) -> None:
    _, inputs, _, base, _, config, _ = _loaded(tmp_path)
    base["transactions"][-1]["instructions"].append(
        composer._ensure_account_alias(
            composer.ADMIN_ACCOUNT_ALIAS,
            inputs.api_signer_account_id,
            config.fee_asset_definition_id,
        )
    )
    with pytest.raises(composer.CompositionError, match="alias target"):
        composer.compose_genesis(base, inputs, config)


def test_source_paths_contain_no_retired_namespace_literal() -> None:
    for path in (SCRIPT, EXAMPLE_INPUTS, Path(__file__)):
        text = path.read_text(encoding="utf-8")
        assert not composer._contains_retired_namespace(text)
