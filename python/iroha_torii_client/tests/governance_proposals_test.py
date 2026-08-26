"""Closed first-release governance proposal reader tests."""

from __future__ import annotations

import copy

import pytest

from client_test_support import CANONICAL_OWNER
from iroha_torii_client.governance_proposals import (
    GovernanceSccpAdvanceLaneTrustAnchor,
    GovernanceSccpEvmDestinationDeployment,
    GovernanceSccpGovernedRoute,
    GovernanceSccpInitializeLaneTrustAnchor,
    GovernanceSccpRegisterRoute,
    GovernanceSccpRouteAction,
    GovernanceSccpRouteKey,
    GovernanceSccpSetRouteActivation,
    GovernanceSccpSolanaDestinationDeployment,
    GovernanceSccpSwitchRouteRevision,
    GovernanceProposalDeployContract,
    GovernanceProposalKind,
    GovernanceProposalKindTag,
    GovernanceProposalMusubiRegistryGovernance,
    GovernanceProposalRecord,
    GovernanceProposalRuntimeUpgrade,
    GovernanceProposalSccpRouteGovernance,
    GovernanceProposalSorafsProviderGovernance,
    GovernanceProposalValidationFeePayoutLifecycle,
    GovernanceProposalValidationFeePolicy,
)

CONTRACT_ADDRESS = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
NETWORK_ID = "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7"
PUBLIC_SIGNAL_SCHEMA_HASH = (
    "7567439F41173D6745A3D51923CB70371ACC7D66F23CEFB4100D6D5D7A432CBB"
)
TAIRA_CHAIN_ID_HASH = (
    "CF1CFC0F57B0BFA4C21882A9870317A1F4812F86533897095E3944BE34C5BBA7"
)


def _lane() -> dict[str, object]:
    return {
        "source": {"network": "ethereum_mainnet", "profile": None},
        "target": {"network": "sora_taira", "profile": None},
    }


def _activation(value: str) -> dict[str, object]:
    return {"activation": value, "direction": None}


def _route_key(revision: int = 1) -> dict[str, object]:
    return {
        "lane_id": _lane(),
        "route_id": "taira_eth_xor",
        "asset_key": "xor",
        "revision": revision,
    }


def _native_anchor(height: int, fill: str) -> dict[str, object]:
    return {
        "backend": {"backend": "ethereum_beacon_v1", "protocol": None},
        "anchor_hash": fill * 64,
        "checkpoint_height": height,
    }


def _g1(fill: str) -> dict[str, str]:
    return {"x": fill * 64, "y": "01" * 32}


def _g2(fill: str) -> dict[str, str]:
    return {
        "x_c0": fill * 64,
        "x_c1": "01" * 32,
        "y_c0": "02" * 32,
        "y_c1": "03" * 32,
    }


def _verifying_key() -> dict[str, object]:
    ic_fields = ("constant",) + tuple(f"signal_{index}" for index in range(11))
    return {
        "version": 1,
        "alpha1": _g1("0"),
        "beta2": _g2("0"),
        "gamma2": _g2("1"),
        "delta2": _g2("2"),
        "ic": {field: _g1("0") for field in ic_fields},
    }


def _outbound_proof_policy() -> dict[str, object]:
    return {
        "version": 1,
        "semantic_profile": {
            "profile": "sora_taira_finality_inclusion_groth16_bn254",
            "commitments": {
                "version": 1,
                "circuit_commitment": "20" * 32,
                "witness_generator_commitment": "21" * 32,
                "public_signal_schema_hash": PUBLIC_SIGNAL_SCHEMA_HASH,
            },
        },
        "sora_finality_anchor": {
            "version": 1,
            "source_network": {"network": "sora_taira", "profile": None},
            "protocol_version": 4,
            "chain_id_hash": TAIRA_CHAIN_ID_HASH,
            "checkpoint_height": 12,
            "checkpoint_block_hash": "22" * 32,
            "checkpoint_context_id": "23" * 32,
            "checkpoint_finality_artifact_hash": "24" * 32,
        },
    }


def _register_action() -> dict[str, object]:
    route_address = "13" * 20
    route_code_hash = "17" * 32
    route = {
        **_route_key(),
        "activation": _activation("staged"),
        "inbound_finality_cutoff": None,
        "source_identity": {
            "lane": _lane(),
            "emitter": {
                "emitter": "evm",
                "identity": {
                    "address": route_address,
                    "runtime_code_hash": route_code_hash,
                    "route_config_hash": "18" * 32,
                },
            },
        },
        "destination": {
            "family": "evm",
            "deployment": {
                "token_address": "11" * 20,
                "token_code_hash": "14" * 32,
                "verifier_address": "12" * 20,
                "verifier_code_hash": "15" * 32,
                "verifying_key": _verifying_key(),
                "verifier_key_hash": "16" * 32,
                "outbound_proof_policy": _outbound_proof_policy(),
                "route_address": route_address,
                "route_code_hash": route_code_hash,
                "taira_to_token_multiplier": 1_000_000_000,
            },
        },
        "sora_outbound_execution_policy": {
            "version": 1,
            "semantics": "ivm_proved_record_sccp_message_v1",
            "contract_artifact_sha256": "19" * 32,
            "vk_ref": {
                "backend": "halo2/ipa",
                "name": "sccp_route_v1",
                "version": 1,
                "commitment": "1A" * 32,
            },
            "gas_limit": 1_000_000,
        },
        "settlement": {
            "asset_definition_id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
            "custody_owner": CANONICAL_OWNER,
            "payload_amount_scale": 9,
        },
    }
    return {
        "action": "Register",
        "route": {"route": route, "native_trust_anchor": None},
    }


def _solana_register_action() -> dict[str, object]:
    lane = {
        "source": {"network": "solana_testnet", "profile": None},
        "target": {"network": "sora_taira", "profile": None},
    }
    route = {
        "lane_id": lane,
        "route_id": "taira_sol_xor",
        "asset_key": "xor",
        "revision": 1,
        "activation": _activation("staged"),
        "inbound_finality_cutoff": None,
        "source_identity": {
            "lane": copy.deepcopy(lane),
            "emitter": {
                "emitter": "solana",
                "identity": {
                    "program_id": "31" * 32,
                    "program_data_address": "32" * 32,
                    "program_data_slot": 3,
                    "state_account": "33" * 32,
                    "program_code_hash": "34" * 32,
                    "route_config_hash": "35" * 32,
                },
            },
        },
        "destination": {
            "family": "solana",
            "deployment": {
                "token_mint_address": "41" * 32,
                "route_program_id": "42" * 32,
                "route_program_data_address": "43" * 32,
                "route_program_data_slot": 4,
                "route_state_account": "44" * 32,
                "route_program_code_hash": "45" * 32,
                "native_verifier_program_id": "46" * 32,
                "native_verifier_program_data_address": "47" * 32,
                "native_verifier_program_data_slot": 5,
                "native_verifier_material_account": "48" * 32,
                "native_verifier_program_code_hash": "49" * 32,
                "native_verifier_config_hash": "4A" * 32,
                "verifying_key": _verifying_key(),
                "verifier_key_hash": "4B" * 32,
                "outbound_proof_policy": _outbound_proof_policy(),
                "taira_to_token_multiplier": 1,
            },
        },
        "sora_outbound_execution_policy": {
            "version": 1,
            "semantics": "ivm_proved_record_sccp_message_v1",
            "contract_artifact_sha256": "51" * 32,
            "vk_ref": {
                "backend": "halo2/ipa",
                "name": "sccp_solana_route_v1",
                "version": 1,
                "commitment": "52" * 32,
            },
            "gas_limit": 1_000_000,
        },
        "settlement": {
            "asset_definition_id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
            "custody_owner": CANONICAL_OWNER,
            "payload_amount_scale": 9,
        },
    }
    return {
        "action": "Register",
        "route": {"route": route, "native_trust_anchor": None},
    }


def _payout_binding() -> dict[str, object]:
    return {
        "contract_address": CONTRACT_ADDRESS,
        "code_hash": [17] * 32,
        "entrypoint": "autonomous_validation_fee_tick",
        "treasury_account_id": CANONICAL_OWNER,
        "ds_asset_id": "xor#wonderland",
        "xor_asset_id": "xor#sora",
        "pool_vault_account_id": CANONICAL_OWNER,
        "batch_ds": "10",
        "min_xor_out": "4",
        "max_xor_out": "100",
        "recipients": [
            {"account_id": CANONICAL_OWNER, "share": "0.25"} for _ in range(4)
        ],
    }


def _policy() -> dict[str, object]:
    return {
        "schema_version": 1,
        "network_id": NETWORK_ID,
        "policy_version": "1",
        "previous_policy_hash": None,
        "ds_asset_id": "xor#wonderland",
        "ds_scale": 0,
        "fee": "0",
        "treasury_account_id": CANONICAL_OWNER,
        "charging_mode": {"charging_mode": "DISABLED", "value": None},
        "effective_from_height": "1",
        "expires_after_height": None,
        "exemption_classes": [],
        "treasury_payout_binding": None,
    }


def _variants() -> list[tuple[str, dict[str, object], type[object]]]:
    lane = {
        "source": {"network": "ethereum_mainnet", "profile": None},
        "target": {"network": "sora_taira", "profile": None},
    }
    return [
        (
            "DeployContract",
            {
                "contract_address": CONTRACT_ADDRESS,
                "code_hash": "11" * 32,
                "abi_hash": "22" * 32,
                "abi_version": 1,
                "manifest_provenance": None,
            },
            GovernanceProposalDeployContract,
        ),
        (
            "RuntimeUpgrade",
            {
                "manifest": {
                    "name": "runtime-v1",
                    "description": "first release",
                    "abi_version": 1,
                    "abi_hash": [34] * 32,
                    "added_syscalls": [],
                    "added_pointer_types": [],
                    "start_height": 10,
                    "end_height": 20,
                    "sbom_digests": [],
                    "slsa_attestation": "",
                    "provenance": [],
                }
            },
            GovernanceProposalRuntimeUpgrade,
        ),
        (
            "SccpRouteGovernance",
            {
                "anchor": {
                    "network_id": NETWORK_ID,
                    "action": {
                        "action": "Remove",
                        "route": {
                            "lane_id": lane,
                            "route_id": "eth-mainnet",
                            "asset_key": "xor",
                            "revision": 1,
                        },
                    },
                }
            },
            GovernanceProposalSccpRouteGovernance,
        ),
        (
            "ValidationFeePolicy",
            {
                "proposal_operator": CANONICAL_OWNER,
                "policy": _policy(),
                "payout_lifecycle_proposal_id": None,
            },
            GovernanceProposalValidationFeePolicy,
        ),
        (
            "ValidationFeePayoutLifecycle",
            {
                "proposal_operator": CANONICAL_OWNER,
                "payout_binding": _payout_binding(),
            },
            GovernanceProposalValidationFeePayoutLifecycle,
        ),
        (
            "MusubiRegistryGovernance",
            {
                "kind": "RetargetAlias",
                "value": {
                    "alias": ["wallet"],
                    "target": {
                        "home_dataspace": 1,
                        "scope": {"kind": "DataspaceRoot", "value": None},
                        "name": ["wallet"],
                    },
                    "expected_revision": 1,
                },
            },
            GovernanceProposalMusubiRegistryGovernance,
        ),
        (
            "SorafsProviderGovernance",
            {
                "action": {
                    "action": "establish",
                    "value": {"provider_id": [[51] * 32], "owner": CANONICAL_OWNER},
                }
            },
            GovernanceProposalSorafsProviderGovernance,
        ),
    ]


@pytest.mark.parametrize(("tag", "payload", "payload_type"), _variants())
def test_proposal_kind_accepts_each_closed_v1_variant(
    tag: str, payload: dict[str, object], payload_type: type[object]
) -> None:
    proposal = GovernanceProposalKind.from_payload({"kind": tag, "payload": payload})

    assert proposal.kind is GovernanceProposalKindTag(tag)
    assert isinstance(proposal.payload, payload_type)


@pytest.mark.parametrize(
    "payload",
    [
        {"DeployContract": _variants()[0][1]},
        {"kind": "ApproveGovernanceProposal", "payload": {}},
        {"kind": "DeployContract", "payload": {**_variants()[0][1], "window": {}}},
        {"kind": "DeployContract", "payload": _variants()[0][1], "legacy": True},
    ],
)
def test_proposal_kind_rejects_unknown_and_retired_shapes(payload: object) -> None:
    with pytest.raises(TypeError):
        GovernanceProposalKind.from_payload(payload)


def test_closed_nested_action_tags_reject_unknown_values() -> None:
    variants = _variants()
    sccp = copy.deepcopy(variants[2][1])
    sccp["anchor"]["action"]["action"] = "ReplaceEverything"  # type: ignore[index]
    with pytest.raises(TypeError, match="SCCP action"):
        GovernanceProposalKind.from_payload({"kind": "SccpRouteGovernance", "payload": sccp})

    musubi = copy.deepcopy(variants[5][1])
    musubi["kind"] = "LegacyRecovery"
    with pytest.raises(TypeError, match="Musubi action"):
        GovernanceProposalKind.from_payload({"kind": "MusubiRegistryGovernance", "payload": musubi})

    sorafs = copy.deepcopy(variants[6][1])
    sorafs["action"]["action"] = "replace"  # type: ignore[index]
    with pytest.raises(TypeError, match="provider action"):
        GovernanceProposalKind.from_payload({"kind": "SorafsProviderGovernance", "payload": sorafs})

    direct_provider_id = copy.deepcopy(variants[6][1])
    direct_provider_id["action"]["value"]["provider_id"] = [51] * 32  # type: ignore[index]
    with pytest.raises(TypeError, match="one-field ProviderId tuple"):
        GovernanceProposalKind.from_payload(
            {"kind": "SorafsProviderGovernance", "payload": direct_provider_id}
        )

    scalar_musubi_newtypes = copy.deepcopy(variants[5][1])
    scalar_musubi_newtypes["value"]["alias"] = "wallet"  # type: ignore[index]
    with pytest.raises(TypeError, match="one-field string tuple"):
        GovernanceProposalKind.from_payload(
            {"kind": "MusubiRegistryGovernance", "payload": scalar_musubi_newtypes}
        )

    scalar_package_name = copy.deepcopy(variants[5][1])
    scalar_package_name["value"]["target"]["name"] = "wallet"  # type: ignore[index]
    with pytest.raises(TypeError, match="one-field string tuple"):
        GovernanceProposalKind.from_payload(
            {"kind": "MusubiRegistryGovernance", "payload": scalar_package_name}
        )


def test_sccp_register_action_is_recursively_typed() -> None:
    parsed = GovernanceSccpRouteAction.from_payload(_register_action())

    assert isinstance(parsed.route, GovernanceSccpRegisterRoute)
    assert isinstance(parsed.route.route, GovernanceSccpGovernedRoute)
    assert isinstance(
        parsed.route.route.destination.deployment,
        GovernanceSccpEvmDestinationDeployment,
    )
    assert parsed.route.route.destination.deployment.outbound_proof_policy.version == 1
    assert parsed.route.route.sora_outbound_execution_policy.vk_ref.name == "sccp_route_v1"
    assert parsed.route.route.settlement.custody_owner == CANONICAL_OWNER


def test_sccp_solana_register_uses_the_exact_typed_deployment() -> None:
    parsed = GovernanceSccpRouteAction.from_payload(_solana_register_action())

    assert isinstance(parsed.route, GovernanceSccpRegisterRoute)
    deployment = parsed.route.route.destination.deployment
    assert isinstance(deployment, GovernanceSccpSolanaDestinationDeployment)
    assert deployment.native_verifier_material_account == "48" * 32
    assert deployment.route_program_data_slot == 4


@pytest.mark.parametrize(
    "field",
    ["native_verifier_material_account", "native_verifier_config_hash"],
)
def test_sccp_solana_register_rejects_missing_closed_deployment_roles(field: str) -> None:
    action = _solana_register_action()
    action["route"]["route"]["destination"]["deployment"].pop(field)  # type: ignore[index]

    with pytest.raises(TypeError, match="missing required field"):
        GovernanceSccpRouteAction.from_payload(action)


def test_sccp_solana_slots_reject_unsafe_json_integers() -> None:
    action = _solana_register_action()
    action["route"]["route"]["destination"]["deployment"][
        "native_verifier_program_data_slot"
    ] = 1 << 53  # type: ignore[index]

    with pytest.raises(TypeError, match="integer"):
        GovernanceSccpRouteAction.from_payload(action)


@pytest.mark.parametrize(
    ("action", "route_type"),
    [
        (
            {
                "action": "SetActivation",
                "route": {
                    "key": _route_key(),
                    "expected_current": _activation("staged"),
                    "next": _activation("bidirectional"),
                    "inbound_finality_cutoff": None,
                },
            },
            GovernanceSccpSetRouteActivation,
        ),
        (
            {
                "action": "SwitchRevision",
                "route": {
                    "previous_key": _route_key(),
                    "expected_previous": _activation("bidirectional"),
                    "previous_next": _activation("inbound_only"),
                    "previous_inbound_finality_cutoff": None,
                    "successor_key": _route_key(2),
                    "successor_next": _activation("bidirectional"),
                },
            },
            GovernanceSccpSwitchRouteRevision,
        ),
        (
            {
                "action": "InitializeTrustAnchor",
                "route": {
                    "lane_id": _lane(),
                    "expected_current": None,
                    "initial": _native_anchor(10, "A"),
                },
            },
            GovernanceSccpInitializeLaneTrustAnchor,
        ),
        (
            {
                "action": "AdvanceTrustAnchor",
                "route": {
                    "lane_id": _lane(),
                    "expected_current": _native_anchor(10, "A"),
                    "next": _native_anchor(11, "B"),
                },
            },
            GovernanceSccpAdvanceLaneTrustAnchor,
        ),
        (
            {"action": "Remove", "route": _route_key()},
            GovernanceSccpRouteKey,
        ),
    ],
)
def test_every_non_register_sccp_action_has_a_typed_payload(
    action: dict[str, object], route_type: type[object]
) -> None:
    assert isinstance(GovernanceSccpRouteAction.from_payload(action).route, route_type)


@pytest.mark.parametrize(
    "mutate",
    [
        lambda action: action["route"]["route"]["destination"]["deployment"].__setitem__(
            "rpc_url", "https://example.invalid"
        ),
        lambda action: action["route"]["route"].pop("sora_outbound_execution_policy"),
        lambda action: action["route"]["route"]["destination"]["deployment"][
            "outbound_proof_policy"
        ]["sora_finality_anchor"].__setitem__("checkpoint_height", 1 << 53),
        lambda action: action["route"]["route"]["destination"]["deployment"].__setitem__(
            "token_code_hash", "aa" * 32
        ),
        lambda action: action["route"]["route"]["activation"].__setitem__(
            "direction", {}
        ),
        lambda action: action["route"].pop("native_trust_anchor"),
    ],
)
def test_sccp_register_rejects_nested_raw_fallbacks_and_noncanonical_values(
    mutate: object,
) -> None:
    action = _register_action()
    mutate(action)  # type: ignore[operator]

    with pytest.raises(TypeError):
        GovernanceSccpRouteAction.from_payload(action)


def test_sccp_action_u64_fields_reject_unsafe_json_integers() -> None:
    action = {
        "action": "AdvanceTrustAnchor",
        "route": {
            "lane_id": _lane(),
            "expected_current": _native_anchor(10, "A"),
            "next": _native_anchor(11, "B"),
        },
    }
    action["route"]["next"]["checkpoint_height"] = 1 << 53  # type: ignore[index]

    with pytest.raises(TypeError, match="integer"):
        GovernanceSccpRouteAction.from_payload(action)


def test_proposal_record_is_exact_and_rejects_retired_wrapper_fields() -> None:
    record = {
        "proposer": CANONICAL_OWNER,
        "kind": {"kind": _variants()[0][0], "payload": _variants()[0][1]},
        "created_height": 7,
        "status": "Superseded",
    }
    parsed = GovernanceProposalRecord.from_payload(record)
    assert parsed.created_height == 7

    for old_field in ("pipeline", "parliament_snapshot", "finalization_evidence"):
        with pytest.raises(TypeError, match="unknown field"):
            GovernanceProposalRecord.from_payload({**record, old_field: None})


@pytest.mark.parametrize(
    "status",
    ["Proposed", "Rejected", "Enacted", "Superseded", "ExecutionFailed"],
)
def test_proposal_record_accepts_only_first_release_statuses(status: str) -> None:
    record = {
        "proposer": CANONICAL_OWNER,
        "kind": {"kind": _variants()[0][0], "payload": _variants()[0][1]},
        "created_height": 7,
        "status": status,
    }

    assert GovernanceProposalRecord.from_payload(record).status.value == status

    record["status"] = "Approved"
    with pytest.raises(TypeError, match="status is unsupported"):
        GovernanceProposalRecord.from_payload(record)
