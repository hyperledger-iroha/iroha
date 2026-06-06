from __future__ import annotations

import asyncio
import base64
import hashlib
import inspect
import re
import sys
from collections import deque
from pathlib import Path
from typing import Any, Dict, Mapping

import pytest

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

import iroha_torii_client.sccp as sccp_module  # noqa: E402
import iroha_torii_client as iroha_torii_client_package  # noqa: E402

from iroha_torii_client import (  # noqa: E402
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_SORA,
    SCCP_DOMAIN_SORA2,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_ETH_MAINNET_EVM_CHAIN_ID,
    SCCP_ETH_MAINNET_NETWORK_ID,
    SCCP_BSC_MAINNET_EVM_CHAIN_ID,
    SCCP_BSC_MAINNET_NETWORK_ID,
    SCCP_EVM_CONTRACT_CALL_ABI_TUPLE_V1,
    SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
    SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1,
    SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1,
    SCCP_SUBMIT_MESSAGE_PROOF_ABI_V1,
    SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1,
    SCCP_SOLANA_BORSH_INSTRUCTION_V1,
    SCCP_SOLANA_MAINNET_GENESIS_HASH,
    SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
    SCCP_SOLANA_MAINNET_SLOTS_PER_EPOCH,
    SCCP_SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH,
    SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH,
    SCCP_SOLANA_MAX_VALIDATORS,
    SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1,
    SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
    SCCP_SOLANA_UPGRADEABLE_LOADER_ID,
    SCCP_SOLANA_STAKE_PROGRAM_ID,
    SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID,
    SCCP_SOLANA_SYSVAR_PROGRAM_ID,
    SCCP_SOLANA_VOTE_PROGRAM_ID,
    SCCP_STARK_FRI_PROOF_FAMILY_V1,
    SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES,
    SCCP_SOURCE_STATE_MAX_PROOF_BYTES,
    SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES,
    SCCP_TON_CONTRACT_PROOF_BACKEND_V1,
    SCCP_TON_CONFIG_PARAM_KEY_BITS,
    SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM,
    SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1,
    SCCP_TON_MESSAGE_BODY_BOC_V1,
    SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_TRON_CONTRACT_CALL_ABI_TUPLE_V1,
    SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1,
    SCCP_SUBSTRATE_RUNTIME_CALL_SCALE_V1,
    SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1,
    SCCP_SUBSTRATE_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
    SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1,
    SCCP_ZERO_HASH_V1,
    BscMainnetSccp,
    BscMainnetSccpProver,
    EthereumMainnetSccp,
    EvmSccpProver,
    EvmSccpProverUnavailableError,
    SolanaSccpSourceStateProver,
    SolanaSccpSourceStateProverUnavailableError,
    SolanaSccpProver,
    SolanaSccpProverUnavailableError,
    SubstrateSccpProver,
    SubstrateSccpProverUnavailableError,
    TonSccpSourceStateProver,
    TonSccpSourceStateProverUnavailableError,
    TonSccpProver,
    TonSccpProverUnavailableError,
    TronSccpProver,
    TronSccpProverUnavailableError,
    bsc_commit_message_hash,
    bsc_commit_seal_hash,
    bsc_validator_set_metadata_proof_hash,
    bsc_validator_set_hash_from_payload,
    bsc_validator_set_payload_from_header_rlp,
    bsc_validator_set_payload_from_parlia_extra,
    bsc_validator_set_payload_hash,
    bsc_validator_set_storage_value_hash,
    ethereum_mainnet_sccp_destination_binding,
    ethereum_mainnet_sccp_destination_binding_hash,
    bsc_mainnet_sccp_destination_binding,
    bsc_mainnet_sccp_destination_binding_hash,
    build_ethereum_mainnet_sccp_destination_proof_request,
    build_ethereum_mainnet_sccp_destination_submission,
    build_ethereum_mainnet_sccp_local_admission_submission,
    build_bsc_mainnet_sccp_destination_proof_request,
    build_bsc_mainnet_sccp_destination_submission,
    build_bsc_mainnet_sccp_local_admission_submission,
    bsc_validator_set_transition_message_hash,
    bsc_sccp_receipt_proof_hash,
    build_evm_sccp_bridge_proof_submit_payload,
    build_evm_sccp_proof_request,
    build_evm_sccp_submission,
    require_bsc_mainnet_chain_id,
    require_ethereum_mainnet_chain_id,
    wrap_ethereum_mainnet_sccp_destination_proof_result,
    wrap_bsc_mainnet_sccp_destination_proof_result,
    wrap_evm_sccp_proof_result,
    build_solana_sccp_accounts_lt_hash_proof_request,
    build_solana_sccp_full_light_client_audit_proof_requests,
    build_solana_sccp_tower_replay_proof_request,
    build_solana_sccp_proof_request,
    build_solana_sccp_submission,
    wrap_solana_sccp_source_state_verification_proof,
    wrap_solana_sccp_proof_result,
    build_substrate_sccp_runtime_storage_proof_request,
    build_substrate_sccp_proof_request,
    build_substrate_sccp_submission,
    wrap_substrate_sccp_proof_result,
    build_ton_shard_state_proof_request,
    build_ton_sccp_full_light_client_audit_proof_requests,
    build_ton_sccp_message_body_boc,
    build_ton_sccp_proof_request,
    wrap_ton_sccp_proof_result,
    wrap_ton_sccp_source_state_verification_proof,
    build_tron_sccp_bridge_proof_submit_payload,
    build_ton_sccp_submission,
    build_tron_sccp_proof_request,
    build_tron_sccp_submission,
    wrap_tron_sccp_proof_result,
    canonical_bsc_commit_message_bytes,
    canonical_bsc_commit_seal_bytes,
    canonical_bsc_validator_set_metadata_proof_bytes,
    canonical_bsc_validator_set_payload_bytes,
    canonical_bsc_validator_set_transition_message_bytes,
    canonical_bsc_sccp_receipt_proof_bytes,
    canonical_evm_receipt_root_mpt_value,
    canonical_evm_sccp_receipt_proof_bytes,
    canonical_eth_sync_committee_payload_bytes,
    canonical_eth_sync_committee_transition_message_bytes,
    canonical_eth_sync_committee_transition_signature_bytes,
    canonical_sccp_source_adapter_deployment_binding_bytes,
    canonical_sccp_source_adapter_engine_deployment_bytes,
    canonical_sccp_ton_submission_metadata_bytes,
    canonical_sccp_message_transparent_public_inputs_bytes,
    canonical_sccp_source_verifier_material_bytes,
    canonical_solana_sccp_epoch_stake_root_bytes,
    canonical_solana_sccp_account_opening_bytes,
    canonical_solana_sccp_account_inclusion_leaf_bytes,
    canonical_solana_sccp_account_inclusion_node_bytes,
    canonical_solana_sccp_bank_fork_bytes,
    canonical_solana_sccp_accounts_lt_hash_proof_public_inputs_bytes,
    canonical_solana_sccp_source_state_verification_proof_bytes,
    canonical_solana_sccp_finality_context_bytes,
    canonical_solana_sccp_full_light_client_audit_statement_bytes,
    canonical_solana_sccp_accounts_lt_hash_opened_contributions_bytes,
    canonical_solana_sccp_route_canary_evidence_bytes,
    canonical_solana_sccp_accounts_lt_hash_commitment_bytes,
    canonical_solana_sccp_accounts_lt_hash_verification_context_bytes,
    canonical_ton_sccp_route_canary_evidence_bytes,
    canonical_tron_sccp_route_canary_evidence_bytes,
    canonical_solana_sccp_message_proof_bytes,
    canonical_solana_sccp_transaction_status_leaf_bytes,
    canonical_solana_sccp_proof_context_bytes,
    canonical_solana_sccp_stake_activation_bytes,
    canonical_solana_sccp_stake_account_state_bytes,
    canonical_solana_sccp_vote_account_data_bytes,
    canonical_solana_sccp_stake_account_data_bytes,
    canonical_solana_sccp_stake_history_sysvar_data_bytes,
    canonical_solana_sccp_stake_history_bytes,
    canonical_solana_sccp_tower_lockout_bytes,
    canonical_solana_sccp_tower_replay_bytes,
    canonical_solana_sccp_witness_bytes,
    canonical_substrate_authority_set_payload_bytes,
    canonical_substrate_authority_set_transition_justification_bytes,
    canonical_substrate_authority_set_transition_message_bytes,
    canonical_substrate_sccp_storage_proof_bytes,
    canonical_substrate_sccp_runtime_storage_verification_context_bytes,
    canonical_substrate_sccp_runtime_storage_verification_statement_bytes,
    canonical_ton_sccp_shard_proof_bytes,
    canonical_ton_sccp_full_light_client_audit_statement_bytes,
    canonical_ton_sccp_source_state_verification_proof_bytes,
    canonical_ton_shard_state_proof_public_inputs_bytes,
    canonical_ton_shard_state_verification_context_bytes,
    canonical_ton_shard_state_witness_commitment_bytes,
    canonical_ton_masterchain_block_message_bytes,
    canonical_ton_masterchain_config_leaf_bytes,
    canonical_ton_masterchain_config_proof_bytes,
    canonical_ton_masterchain_validator_signatures_bytes,
    canonical_ton_validator_set_bytes,
    canonical_ton_validator_set_payload_bytes,
    canonical_ton_validator_set_transition_message_bytes,
    canonical_ton_validator_set_transition_signature_bytes,
    canonical_tron_raw_block_header_bytes,
    canonical_tron_receipt_root_mpt_value,
    canonical_tron_sccp_receipt_proof_bytes,
    canonical_tron_sccp_receipt_state_proof_bytes,
    canonical_tron_sccp_transaction_source_proof_bytes,
    canonical_tron_solid_block_message_bytes,
    canonical_tron_solid_block_header_proof_bytes,
    canonical_tron_witness_seal_bytes,
    canonical_tron_witness_schedule_transition_message_bytes,
    canonical_tron_witness_schedule_transition_seal_bytes,
    canonical_tron_witness_schedule_payload_bytes,
    eth_sync_committee_hash,
    eth_sync_committee_hash_from_payload,
    eth_sync_committee_payload_hash,
    eth_beacon_block_header_root,
    eth_beacon_body_root_from_execution_payload_branch,
    eth_execution_payload_header_root_from_rlp,
    eth_mainnet_sync_committee_period_for_slot,
    eth_sync_committee_transition_message_hash,
    eth_sync_committee_transition_signature_hash,
    evm_sccp_destination_binding,
    evm_sccp_destination_binding_hash,
    evm_sccp_source_event_topic,
    evm_sccp_receipt_proof_hash,
    normalize_evm_sccp_proof_context,
    normalize_sccp_source_adapter_deployment_binding,
    normalize_sccp_source_adapter_engine_deployment,
    normalize_sccp_source_verifier_material,
    normalize_solana_sccp_witness,
    normalize_ton_sccp_proof_context,
    normalize_tron_sccp_proof_context,
    sccp_groth16_bn254_public_signal_words,
    sccp_message_transparent_public_input_abi_words,
    sccp_submit_message_proof_call_data,
    sccp_destination_binding_hash,
    sccp_destination_binding_key,
    sccp_source_adapter_deployment_binding_hash,
    sccp_source_adapter_engine_deployment_hash,
    sccp_solana_full_light_client_gate_hash,
    sccp_ton_full_light_client_gate_hash,
    sccp_source_adapter_verifier_vk_hash,
    sccp_source_verifier_material_hash,
    solana_sccp_message_proof_hash,
    solana_sccp_transaction_status_leaf_hash,
    solana_sccp_transaction_status_root_from_branch,
    solana_sccp_agave_bank_hash,
    solana_sccp_account_opening_hash,
    solana_sccp_account_raw_data_hash,
    solana_sccp_account_inclusion_leaf_hash,
    solana_sccp_account_inclusion_node_hash,
    solana_sccp_account_inclusion_root_from_branch,
    solana_sccp_account_inclusion_root_and_branches,
    solana_sccp_opened_account_inclusion_witness,
    solana_sccp_account_lt_hash,
    solana_sccp_accounts_lt_hash_checksum,
    solana_sccp_accounts_lt_hash_from_openings,
    solana_sccp_accounts_lt_hash_opened_contributions_hash,
    solana_sccp_accounts_lt_hash_opened_residual,
    solana_sccp_accounts_lt_hash_opened_residual_checksum,
    solana_sccp_accounts_lt_hash_open_verify_schema_descriptor,
    solana_sccp_accounts_lt_hash_public_input_columns,
    solana_sccp_accounts_lt_hash_proof_public_inputs_hash,
    solana_sccp_accounts_lt_hash_proof_hash,
    solana_sccp_bank_fork_hash,
    solana_sccp_epoch_stake_root,
    solana_sccp_finality_context_hash,
    solana_sccp_vote_message_hash,
    solana_sccp_full_light_client_audit_statement_hash,
    solana_sccp_full_light_client_audit_public_input_columns,
    solana_sccp_full_light_client_audit_open_verify_schema_descriptor,
    solana_sccp_mainnet_epoch_for_slot,
    solana_sccp_proof_context_hash,
    solana_sccp_stake_activation_hash,
    solana_sccp_stake_account_state_hash,
    solana_sccp_vote_account_data_hash,
    solana_sccp_vote_account_data_from_raw_vote_state,
    solana_sccp_vote_account_data_hash_from_raw_vote_state,
    solana_sccp_vote_account_data_from_raw_vote_state_v1_or_v3,
    solana_sccp_vote_account_data_hash_from_raw_vote_state_v1_or_v3,
    solana_sccp_stake_account_data_hash,
    solana_sccp_stake_account_data_from_raw_stake_state_v2,
    solana_sccp_stake_account_data_hash_from_raw_stake_state_v2,
    solana_sccp_stake_history_sysvar_data_hash,
    solana_sccp_stake_history_sysvar_data_hash_from_raw_data,
    solana_sccp_stake_history_hash,
    solana_sccp_tower_lockout_hash,
    solana_sccp_tower_replay_hash,
    solana_sccp_route_canary_evidence_hash,
    ton_sccp_route_canary_evidence_hash,
    tron_sccp_route_canary_evidence_hash,
    substrate_authority_set_hash_from_payload,
    substrate_authority_set_payload_hash,
    substrate_authority_set_transition_justification_hash,
    substrate_authority_set_transition_message_hash,
    substrate_sccp_runtime_storage_open_verify_schema_descriptor,
    substrate_sccp_runtime_storage_proof_public_inputs_hash,
    substrate_sccp_runtime_storage_public_input_columns,
    substrate_sccp_storage_proof_hash,
    ton_config_validator_set_payload_from_proof_boc,
    ton_config_validator_set_payload_hash_from_proof_boc,
    ton_hashmap_e_cell_ref_value_hash,
    ton_hashmap_e_proof_root_hash,
    ton_shard_accounts_last_transaction,
    ton_shard_accounts_last_transaction_hash,
    ton_sccp_submission_query_id,
    ton_shard_state_proof_root_hash,
    ton_shard_state_accounts_root_hash,
    ton_boc_root_hashes,
    ton_boc_single_root_hash,
    ton_sccp_shard_proof_hash,
    ton_shard_state_open_verify_schema_descriptor,
    ton_shard_state_proof_public_inputs_hash,
    ton_shard_state_public_input_columns,
    ton_sccp_shard_state_verification_proof_hash,
    ton_sccp_full_light_client_audit_statement_hash,
    ton_sccp_full_light_client_audit_public_input_columns,
    ton_sccp_full_light_client_audit_open_verify_schema_descriptor,
    ton_masterchain_block_message_hash,
    ton_masterchain_config_leaf_hash,
    ton_masterchain_config_proof_hash,
    ton_masterchain_validator_signatures_hash,
    ton_validator_set_hash,
    ton_validator_set_hash_from_payload,
    ton_validator_set_payload_hash,
    ton_validator_set_transition_message_hash,
    ton_validator_set_transition_signature_hash,
    tron_sccp_destination_binding,
    tron_sccp_destination_binding_hash,
    tron_sccp_receipt_proof_hash,
    tron_sccp_receipt_state_proof_hash,
    tron_sccp_source_message_call_data,
    tron_sccp_transaction_source_proof_hash,
    tron_block_id_from_raw_data_hash,
    tron_raw_block_header_hash,
    tron_solid_block_message_hash,
    tron_solid_block_header_proof_hash,
    tron_witness_seal_hash,
    tron_witness_schedule_transition_message_hash,
    tron_witness_schedule_transition_seal_hash,
    tron_witness_schedule_hash_from_payload,
    tron_witness_schedule_payload_hash,
)
from iroha_torii_client.sccp import _keccak_256  # noqa: E402


HEX32_A = "0x" + "aa" * 32
HEX32_B = "0x" + "bb" * 32
HEX32_C = "0x" + "cc" * 32
HEX32_D = "0x" + "dd" * 32
HEX32_E = "0x" + "ee" * 32
HEX32_F = "0x" + "12" * 32
HEX32_G = "0x" + "56" * 32
HEX32_H = "0x" + "78" * 32
SOURCE_EVENT_DIGEST = "0x" + "34" * 32
SOURCE_BRIDGE_ADDRESS = "0x" + "44" * 20
ETHEREUM_FINALITY_BRANCH = [
    "0x" + f"{0x50 + index:02x}" * 32 for index in range(6)
]
ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS = (
    "0x" + "ff" * 42 + "3f" + "00" * 21
)
LOW_ETHEREUM_SYNC_COMMITTEE_BITS = "0x01" + "00" * 63


def source_event_log(**overrides: Any) -> Dict[str, Any]:
    log: Dict[str, Any] = {
        "address": SOURCE_BRIDGE_ADDRESS,
        "transactionHash": HEX32_A,
        "blockHash": HEX32_B,
        "blockNumber": "0x1234",
        "topics": [evm_sccp_source_event_topic(), SOURCE_EVENT_DIGEST],
        "data": "0x",
    }
    log.update(overrides)
    return log


def ethereum_beacon_finality(**overrides: Any) -> Dict[str, Any]:
    finality: Dict[str, Any] = {
        "executionBlockNumber": "0x1234",
        "executionBlockHash": HEX32_B,
        "executionReceiptsRoot": HEX32_C,
        "finalizedHeaderRoot": HEX32_D,
        "syncCommitteeRoot": HEX32_E,
        "beaconSlot": "11",
        "finalityBranch": ETHEREUM_FINALITY_BRANCH,
        "syncCommitteeBits": ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS,
        "syncCommitteeSignature": "0x" + "34" * 96,
        "syncSignatureSlot": "12",
        "syncCommitteeParticipation": "342",
    }
    finality.update(overrides)
    return finality


TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8"
)
TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8"
)
TRON_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "0xfea8effb3cddfa458ea79a5a9af6f2d2c33a460b3a66d9305963908c2a3ea67a"
)
TRON_ROUTE_CANARY_EVIDENCE_HASH_VECTOR = (
    "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56"
)
SOLANA_MAINNET_GENESIS_PUBLIC_INPUT = (
    "0x8dbaadfbc441ded0257a4700cd26d814b5a196be44b963454cff8dd9543f13b5"
)


def test_package_all_exports_sccp_proof_result_wrappers() -> None:
    exported = set(iroha_torii_client_package.__all__)

    assert {
        "wrap_evm_sccp_proof_result",
        "wrap_ton_sccp_proof_result",
        "wrap_tron_sccp_proof_result",
        "wrap_substrate_sccp_proof_result",
    } <= exported


def test_package_all_exports_public_sccp_symbols() -> None:
    exported = set(iroha_torii_client_package.__all__)
    public_sccp_symbols = {
        name
        for name, value in vars(sccp_module).items()
        if not name.startswith("_")
        and (
            (
                name.startswith("SCCP_")
                and isinstance(value, (str, int))
            )
            or (
                (inspect.isfunction(value) or inspect.isclass(value))
                and value.__module__ == sccp_module.__name__
            )
        )
    }

    assert public_sccp_symbols <= exported
    for name in sorted(public_sccp_symbols):
        assert getattr(iroha_torii_client_package, name) is getattr(sccp_module, name)


def assert_immutable_fastpq_proof_request(
    request: Mapping[str, Any],
    byte_fields: tuple[str, ...],
) -> None:
    with pytest.raises(TypeError, match="immutable"):
        request["version"] = 2  # type: ignore[index]
    with pytest.raises(TypeError, match="immutable"):
        request["public_input_columns"].append(["tampered"])  # type: ignore[attr-defined]
    with pytest.raises(TypeError, match="immutable"):
        request["public_input_columns"][0].append("tampered")  # type: ignore[index,attr-defined]
    with pytest.raises(TypeError, match="immutable"):
        request["fastpq_public_inputs"]["slot"] = "tampered"  # type: ignore[index]
    with pytest.raises(TypeError, match="immutable"):
        request["fastpq_transitions"].append({"key": "tampered"})  # type: ignore[attr-defined]
    with pytest.raises(TypeError, match="immutable"):
        request["fastpq_transitions"][0]["new_value"] = "tampered"  # type: ignore[index]

    for field in byte_fields:
        field_bytes = request[field]
        assert isinstance(field_bytes, bytes)
        assert len(field_bytes) > 0
        with pytest.raises(TypeError):
            field_bytes[0] = field_bytes[0] ^ 0xFF  # type: ignore[index]


def mutable_proof_request(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {key: mutable_proof_request(child) for key, child in value.items()}
    if isinstance(value, (list, tuple)):
        return [mutable_proof_request(child) for child in value]
    if isinstance(value, bytes):
        return bytes(value)
    return value


def abi_word(value: int) -> bytes:
    return value.to_bytes(32, "big")


BN254_G2_GENERATOR_WORDS = (
    abi_word(
        int("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed", 16)
    ),
    abi_word(
        int("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2", 16)
    ),
    abi_word(
        int("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa", 16)
    ),
    abi_word(
        int("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b", 16)
    ),
)


def groth16_proof_bytes(*, words: Mapping[int, bytes] | None = None) -> bytes:
    proof_words = [
        abi_word(1),
        bytes([0x11]) * 32,
        abi_word(SCCP_DOMAIN_SORA),
        bytes([0x33]) * 32,
        abi_word(1),
        abi_word(2),
        *BN254_G2_GENERATOR_WORDS,
        abi_word(1),
        abi_word(2),
    ]
    for index, word in (words or {}).items():
        proof_words[index] = word
    return b"".join(proof_words)


GROTH16_PROOF_BYTES = groth16_proof_bytes()
SOLANA_SIGNATURE_55 = (
    "2hxGyn4y9Mjkii76BqmxVoNYbTs3tw97bmtZRXnDoZPAw7VZTWhhk1aV11DtFgYGVibPaty4PQLHVLaKrT24NxGU"
)
SOLANA_SIGNATURE_01 = (
    "2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2"
)
SOLANA_ZERO_SIGNATURE = "1" * 64
SOLANA_PROGRAM_42 = "5TeWSsjg2gbxCyWVniXeCmwM7UtHTCK7svzJr5xYJzHf"
SOLANA_PROGRAM_02 = "8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR"
SOLANA_ZERO_PROGRAM = "1" * 32
BSC_VALIDATOR_SET_PAYLOAD_HEX = (
    "0102000000"
    + "11" * 20
    + "0100000000000000"
    + "22" * 20
    + "0200000000000000"
)
BSC_VALIDATOR_SET_PAYLOAD_HASH = "0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370"
BSC_VALIDATOR_SET_HASH = "0x3ef5ecfb6dc4f5fc9e970cc18cd72164495c827e96f77851813973a286f5c762"
BSC_COMMIT_VALIDATOR_PUBLIC_KEYS = [
    "0x0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    "0x02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
    "0x02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
    "0x02e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13",
]
BSC_COMMIT_VALIDATOR_POWERS = ["1", "1", "1", "1"]
BSC_COMMIT_VALIDATOR_SET_HASH = "0xc5152802f6ca9ec72a4249646aca7476496f00b71ab5b1482c881a31fb42dd8c"
BSC_COMMIT_MESSAGE_HASH = "0x5832165d1a87ed49a323f2ecaecbef973489aed1a42e7eab369244e7abec43c7"
BSC_COMMIT_SIGNATURES = [
    "0x1b8802069b82c3d4cb6d7bec82323853f36d965c1e71647560084e7c7a0de9c17c85fcc3c6222f905cbbc4ba5b5f3f005f07d144304184181be67b3d02d1ba9f00",
    "0x921d39c29fb793c496f96cf647128232d228024ed2f3e68cc6a52aa4cf64facf6bbd9dfcf7d703165f7880e7e1310f34d1b0fb8ca6dd8f506bf289ba012387f001",
    "0xcfa11aa1ec214278afdb4ef7f3c40af97a2784e0336afb5ebef345c0d2eaa9ef629ad2d25cf9709eb9b842fb2fb3f749ce365af97af6e7064771614312d3619600",
]
BSC_COMMIT_SEAL_HASH = "0xcd9d87b24d8c1cf7615cb4267cde5a3fc24bbb770807134ee75d4ddaba992172"


def sample_solana_stake_state_v2_stake_account() -> bytes:
    data = bytearray(200)
    data[0:4] = (2).to_bytes(4, "little")
    data[12:44] = bytes([0x81]) * 32
    data[44:76] = bytes([0x91]) * 32
    data[124:156] = bytes([0xA1]) * 32
    data[156:164] = (1_000).to_bytes(8, "little")
    data[164:172] = (2).to_bytes(8, "little")
    data[172:180] = (9).to_bytes(8, "little")
    data[180:188] = bytes([0x0A, 0xD7, 0xA3, 0x70, 0x3D, 0x0A, 0xB7, 0x3F])
    data[188:196] = (123).to_bytes(8, "little")
    data[196] = 1
    return bytes(data)


def sample_solana_vote_state_account(has_latency: bool = True) -> bytes:
    data = bytearray(3_762)
    cursor = 0

    def write_u8(value: int) -> None:
        nonlocal cursor
        data[cursor] = value
        cursor += 1

    def write_u32(value: int) -> None:
        nonlocal cursor
        data[cursor : cursor + 4] = value.to_bytes(4, "little")
        cursor += 4

    def write_u64(value: int) -> None:
        nonlocal cursor
        data[cursor : cursor + 8] = value.to_bytes(8, "little")
        cursor += 8

    def write_repeated(value: int, length: int) -> None:
        nonlocal cursor
        data[cursor : cursor + length] = bytes([value]) * length
        cursor += length

    write_u32(2 if has_latency else 1)
    write_repeated(0x51, 32)
    write_repeated(0x71, 32)
    write_u8(7)
    write_u64(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH)
    for index in range(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH):
        if has_latency:
            write_u8(0)
        write_u64(11 + index)
        write_u32(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH - index)
    write_u8(1)
    write_u64(10)
    write_u64(2)
    write_u64(1)
    write_repeated(0x60, 32)
    write_u64(3)
    write_repeated(0x61, 32)
    return bytes(data)


def sample_solana_vote_state_v4_account(
    with_bls: bool = True, authorized_voter_count: int = 2
) -> bytes:
    data = bytearray(3_762)
    cursor = 0

    def write_u8(value: int) -> None:
        nonlocal cursor
        data[cursor] = value
        cursor += 1

    def write_u16(value: int) -> None:
        nonlocal cursor
        data[cursor : cursor + 2] = value.to_bytes(2, "little")
        cursor += 2

    def write_u32(value: int) -> None:
        nonlocal cursor
        data[cursor : cursor + 4] = value.to_bytes(4, "little")
        cursor += 4

    def write_u64(value: int) -> None:
        nonlocal cursor
        data[cursor : cursor + 8] = value.to_bytes(8, "little")
        cursor += 8

    def write_repeated(value: int, length: int) -> None:
        nonlocal cursor
        data[cursor : cursor + length] = bytes([value]) * length
        cursor += length

    write_u32(3)
    write_repeated(0x51, 32)
    write_repeated(0x71, 32)
    write_repeated(0x81, 32)
    write_repeated(0x91, 32)
    write_u16(1_234)
    write_u16(9_876)
    write_u64(456)
    write_u8(1 if with_bls else 0)
    if with_bls:
        write_repeated(0xA5, 48)
    write_u64(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH)
    for index in range(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH):
        write_u8(0)
        write_u64(11 + index)
        write_u32(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH - index)
    write_u8(1)
    write_u64(10)
    write_u64(authorized_voter_count)
    for index in range(authorized_voter_count):
        write_u64(index + 1)
        write_repeated(0x60 + index, 32)
    return bytes(data)


TON_VALIDATOR_SET_HASH = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938"
TON_NEXT_VALIDATOR_SET_PAYLOAD_HEX = (
    "0102000000"
    + "33" * 32
    + "0300000000000000"
    + "44" * 32
    + "0400000000000000"
)
TON_NEXT_VALIDATOR_SET_HASH = "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f"
TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH = "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983"
TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH = "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19"
TON_VALIDATOR_SET_TRANSITION_SIGNATURE_HASH = "0xd784461f68495981c2c00e60316dc9353ea4b5be3bc261b26feadc7c83c4f6a7"
TON_VALIDATOR_SET_PAYLOAD_HASH = "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0"
TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH = (
    "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f"
)
TON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES = {
    "source_trust_anchor_hash": "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
    "consensus_verifier_hash": "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473",
    "message_inclusion_verifier_hash": "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353",
    "source_state_verifier_hash": TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH,
    "finality_policy_hash": "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43",
}
SOLANA_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES = {
    "source_trust_anchor_hash": "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
    "consensus_verifier_hash": "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba",
    "message_inclusion_verifier_hash": "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0",
    "source_state_verifier_hash": SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
    "finality_policy_hash": "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56",
}
TRON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES = {
    "source_trust_anchor_hash": "0x3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c",
    "consensus_verifier_hash": "0x8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea",
    "message_inclusion_verifier_hash": "0xf39db56474b288680ad9561389cca7a841bd1fd223719255324705e1038fcacc",
    "finality_policy_hash": "0xad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864",
}
TON_MASTERCHAIN_CONFIG_LEAF_HASH = "0xed92ba8082850092da7cc296a2184cc4576877aaee08c72748d96ea449b16e39"
TON_MASTERCHAIN_CONFIG_PROOF_BOC = bytes.fromhex(
    "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0"
)
TON_MASTERCHAIN_CONFIG_ROOT = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af"
TON_MASTERCHAIN_CONFIG_VALUE_HASH = "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50"
TON_MASTERCHAIN_CONFIG_PROOF_HASH = "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c"
TON_SHARD_STATE_MASTERCHAIN_CONFIG_PROOF_HASH = "0x235c1f0946e38bc210a6a8e193fbe52399ccc4d82693ef3f123be20e27697fc3"
TON_MASTERCHAIN_BLOCK_MESSAGE_HASH = "0x0ca07d5072adb7db3d6a0f831294c7e119c451884aaa1afcbb23e0df0911d8bd"
TON_MASTERCHAIN_SIGNATURES_HASH = "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15"
TON_ORDINARY_BOC = bytes.fromhex("b5ee9c720101020100070001020101000202")
TON_ORDINARY_BOC_CRC = bytes.fromhex("b5ee9c724101020100070001020101000202be1c1df5")
TON_ORDINARY_BOC_ROOT_HASH = "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe"
TON_PRUNED_BRANCH_BOC = bytes.fromhex(
    "b5ee9c72010101010026002848010149725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe0001"
)
TON_PRUNED_BRANCH_ROOT_HASH = "0xcc9095f882fb62a27bb19ad4aa84e19571a3283988ae40b75e238ad240cf1a96"
TON_LEGACY_PRUNED_PROOF_BOC = bytes.fromhex(
    "b5ee9c7201010601005f0022012001052201620203284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0040004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001"
)
TON_LEGACY_PRUNED_PROOF_ROOT_HASH = "0x9c769b035b601b0ddc098e9b148d9bdab0761c14bfe310ac090962ba1f39739a"
TON_MERKLE_PROOF_BOC = bytes.fromhex(
    "b5ee9c7201010301002d0009460349725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe00010101020102000202"
)
TON_MERKLE_PROOF_ROOT_HASH = "0xe749bc5225cabbe3fa78fc12d74a734c365379bc0d302123dcf7bfa2ee3fbd21"
TON_HASHMAP_E_CELL_REF_BOC = bytes.fromhex(
    "b5ee9c72010109010028000101c001020120020702016203050103a0c004000403090103a0c0060004006f0101de08000403e7"
)
TON_HASHMAP_E_DIRECT_PROOF_BOC = bytes.fromhex(
    "b5ee9c72010107010063002101c00122012002062201620304284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0050004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001"
)
TON_HASHMAP_E_MERKLE_PROOF_BOC = bytes.fromhex(
    "b5ee9c72010108010089000101c001094603e714f85374c2c336ed499a5a35e6c4f87441184532e7c23be795ce71b457f1bf00030222012003072201620405284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0060004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001"
)
TON_HASHMAP_E_VALUE_HASH = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419"
TON_HASHMAP_E_ROOT_HASH = "0x767fcde38f7a8e9eb21d75271ed20e2b92c30e9f1726ee0247c98829b900199d"
TON_SHARD_ACCOUNTS_BOC = bytes.fromhex(
    "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000"
)
TON_SHARD_ACCOUNTS_ROOT_HASH = "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3"
TON_SHARD_STATE_PROOF_BOC = bytes.fromhex(
    "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000"
)
TON_SHARD_STATE_ROOT_HASH = "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270"
TON_SHARD_ACCOUNT_KEY = bytes([17] + [0] * 31)
TRON_WITNESS_SCHEDULE_PAYLOAD_HEX = (
    "0102000000"
    + "41"
    + "11" * 20
    + "0100000000000000"
    + "41"
    + "22" * 20
    + "0200000000000000"
)
TRON_WITNESS_SCHEDULE_PAYLOAD_HASH = "0xd6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429"
TRON_WITNESS_SCHEDULE_HASH = "0x0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be"
TRON_PARENT_RAW_HEADER_HEX = (
    "08b8b096ffbc311220"
    + "cc" * 32
    + "1a20"
    + "bb" * 32
    + "38b8604a1541"
    + "11" * 20
    + "5001"
    + "5a20"
    + "aa" * 32
)
TRON_RAW_HEADER_HEX = (
    "08b9b096ffbc311220"
    + "dd" * 32
    + "1a200000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4"
    + "38b9604a1541"
    + "11" * 20
    + "5001"
    + "5a20"
    + "ee" * 32
)
TRON_PARENT_RAW_HEADER_HASH = "0x5647d462e78851c6701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4"
TRON_RAW_HEADER_HASH = "0x614a09275b6d0fffb6bc08fb34f737c093d9dd2adefccb04344715e2619c8286"
TRON_PARENT_BLOCK_ID = "0x0000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4"
TRON_BLOCK_ID = "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286"
TRON_SOLID_BLOCK_HEADER_PROOF_HASH = "0x25416bda5734ecef1ab9920d15f1011e962f6ff90e9c6247ff6b2ce34a5ab49f"
TRON_SOLID_BLOCK_MESSAGE_HASH = "0x065173d89272a549b504258936729c5226dfdb866ccb9422757d95ec9fa6d688"
TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR = "be9223cdfd6728fd2512f270a44f928fbd58df98f8e9e5fe13c4dc73503192e4"
TRON_SOURCE_EVENT_SIGNATURE_VECTOR = (
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    "38508a4cf743e4a97ab3550672d69d980545ff8d776f6e9bade4ff4196f3693b"
    "00"
)
TRON_TEST_OWNER_ADDRESS = "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf"
TRON_WITNESS_SEAL_HASH = "0x4266cf4de71c96e4fde925b686abbd50e67026f63ad90e0cf4899d4925d45849"
TRON_PARENT_WITNESS_SCHEDULE_PAYLOAD_HEX = (
    "0101000000417e5f4552091a69125d5dfcb7b8c2659029395bdf0100000000000000"
)
TRON_PARENT_WITNESS_SCHEDULE_HASH = "0x87174bbfde1c4b8473a6be18df37b60979c7609ebf1788ce8cf97604311474b6"
TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH = "0x6e53d3f7d1253223a70a163a02544a8df27b74171cb0c76c8f42d71419fabd43"
TRON_WITNESS_SCHEDULE_TRANSITION_SIGNATURE = (
    "0xc6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
    "65d3d639f676a837945854abb3f59c4b93355bb55a789e31a25aee261500932d01"
)
TRON_WITNESS_SCHEDULE_TRANSITION_SEAL_HASH = "0xbb3b7ef87bd3efb77d9b7f0a4dba8e7398827621d59039c694c285a7e2deacce"


def tron_header_signature(recovery_id: int) -> bytes:
    return bytes([0xAA] * 32 + [0x01] * 32 + [recovery_id])


TRON_RECEIPT_STATE_MPT_NODE_HEX = "0xe4822080a0" + "bb" * 32
EVM_RECEIPT_ROOT_MPT_VALUE_HEX = (
    "f8409e736363703a65766d3a726563656970742d726f6f742d76616c75653a7631a0"
    + "bb" * 32
)
EVM_RECEIPT_STATE_MPT_NODE_HEX = "0xf847822080b842" + EVM_RECEIPT_ROOT_MPT_VALUE_HEX
EVM_RECEIPT_STATE_TRANSACTION_ROOT = (
    "0x6438aaabb78989f2803c6b0f227ee0f94beecde07cdd9c737e258e4faf581b68"
)
TRON_RECEIPT_ROOT_MPT_VALUE_HEX = (
    "f8419f736363703a74726f6e3a726563656970742d726f6f742d76616c75653a7631a0"
    + "bb" * 32
)
TRON_RECEIPT_STATE_TRANSACTION_ROOT = (
    "0x21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079"
)
TRON_RECEIPT_STATE_PROOF_HASH = (
    "0x847c5ee3e6f4f83fef4d754a9aed93fae38c6677011cae03b10228c17c60b13b"
)
TRON_SOURCE_MESSAGE_CALL_DATA_HEX = (
    "06841e30" + "00" * 31 + "05" + "00" * 32 + "34" * 32
)
TRON_TRANSACTION_SOURCE_BYTES_HEX = (
    "0x0af3010a02123418b9602208565656565656565640959aef3a5acf01081f12ca"
    "010a31747970652e676f6f676c65617069732e636f6d2f70726f746f636f6c2e"
    "54726967676572536d617274436f6e74726163741294010a15417e5f4552091a"
    "69125d5dfcb7b8c2659029395bdf121541454545454545454545454545454545"
    "4545454545226406841e30000000000000000000000000000000000000000000"
    "0000000000000000000005000000000000000000000000000000000000000000"
    "0000000000000000000000343434343434343434343434343434343434343434"
    "34343434343434343434347090e5ee3a900180e1eb171241cc58d7ac52c91117"
    "92495fee682b53cab96ff4229043c5b8b90c31447f5934553d8854ab35de3437"
    "2c13331bf3ef5cefd8f2cc5ad026faf223da83969fe8973c012a0410001801"
)
TRON_TRANSACTION_SOURCE_ROOT = (
    "0x1751c62dce36d5d642e48480b45d48ed16dd1b9b40ce216bc2f15c1b1ccf300b"
)
TRON_TRANSACTION_SOURCE_PROOF_HASH = (
    "0xfc98a09ae9e7f63ccd383b2f3e104efce0d2c291dc7900ffd49e4f391e6016b6"
)
SUBSTRATE_AUTHORITY_SET_PAYLOAD_HEX = (
    "0102000000"
    + "11" * 32
    + "0100000000000000"
    + "22" * 32
    + "0200000000000000"
)
SUBSTRATE_AUTHORITY_SET_PAYLOAD_HASH = "0xdedc4ebe5f91162a5029cb67f88cdbbf94c2bf2b9d0d373bd3e670321565cc16"
SUBSTRATE_AUTHORITY_SET_HASH = "0xde84b8b7a5409c0f2cff1191173d6caa681d902b35e42669106ec6ea3193a117"
SUBSTRATE_PARENT_AUTHORITY_SET_PAYLOAD_HEX = (
    "0103000000"
    + "11" * 32
    + "0500000000000000"
    + "22" * 32
    + "0700000000000000"
    + "33" * 32
    + "0b00000000000000"
)
SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HEX = (
    "0103000000"
    + "aa" * 32
    + "0d00000000000000"
    + "bb" * 32
    + "1100000000000000"
    + "cc" * 32
    + "1300000000000000"
)
SUBSTRATE_PARENT_AUTHORITY_SET_HASH = "0xb2efd5d86304ea728a8a9ed4013aab8f3e10c0cf862e859c9cade55e660934ef"
SUBSTRATE_NEXT_AUTHORITY_SET_HASH = "0x07cdbba0d61fdd4324b571dd793965e52acbf7f4c163af328e26c92c047501b3"
SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH = "0x12ce972498ba5cd8a760aee0429fdc30d8b6447890e1bf77d8dde46f86b40d85"
SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH = "0x60589333bf798bf592b2642d0fbac39b4e9305576cd2ebe9dd1f448a97a0596b"
SUBSTRATE_AUTHORITY_SET_TRANSITION_JUSTIFICATION_HASH = "0x9528bad2f181eb20a86a9c106cf529e60abf5db81f05cce9c9c3027b78c6cf01"


def _minimal_be_length_bytes(length: int) -> bytes:
    return length.to_bytes((length.bit_length() + 7) // 8 or 1, "big")


def _rlp_string(value: bytes) -> bytes:
    if len(value) == 1 and value[0] < 0x80:
        return value
    if len(value) < 56:
        return bytes([0x80 + len(value)]) + value
    length_bytes = _minimal_be_length_bytes(len(value))
    return bytes([0xB7 + len(length_bytes)]) + length_bytes + value


def _rlp_list(fields: list[bytes]) -> bytes:
    payload = b"".join(fields)
    if len(payload) < 56:
        return bytes([0xC0 + len(payload)]) + payload
    length_bytes = _minimal_be_length_bytes(len(payload))
    return bytes([0xF7 + len(length_bytes)]) + length_bytes + payload


def _sample_bsc_parlia_extra() -> bytes:
    return (
        bytes([0x11]) * 32
        + bytes([2])
        + bytes([0x11]) * 20
        + bytes([0x01]) * 48
        + bytes([0x22]) * 20
        + bytes([0x02]) * 48
        + bytes([0x99]) * 65
    )


def _sample_bsc_parlia_header_rlp(extra_data: bytes) -> bytes:
    return _rlp_list(
        [
            _rlp_string(bytes([0x10]) * 32),
            _rlp_string(bytes([0x11]) * 32),
            _rlp_string(bytes([0x12]) * 20),
            _rlp_string(bytes([0x13]) * 32),
            _rlp_string(bytes([0x14]) * 32),
            _rlp_string(bytes([0x15]) * 32),
            _rlp_string(bytes([0x00]) * 256),
            _rlp_string(bytes([2])),
            _rlp_string(bytes([1])),
            _rlp_string(bytes([1])),
            _rlp_string(bytes([1])),
            _rlp_string(bytes([1])),
            _rlp_string(extra_data),
            _rlp_string(bytes([0x00]) * 32),
            _rlp_string(bytes([0x00]) * 8),
        ]
    )


def _sample_eth_execution_header_rlp(receipts_root: bytes = bytes([0x15]) * 32) -> bytes:
    return _rlp_list(
        [
            _rlp_string(bytes([0x10]) * 32),
            _rlp_string(bytes([0x11]) * 32),
            _rlp_string(bytes([0x12]) * 20),
            _rlp_string(bytes([0x13]) * 32),
            _rlp_string(bytes([0x14]) * 32),
            _rlp_string(receipts_root),
            _rlp_string(bytes([0x00]) * 256),
            _rlp_string(b""),
            _rlp_string(bytes([0x2A])),
            _rlp_string(bytes([0x01, 0xC9, 0xC3, 0x80])),
            _rlp_string(bytes([0x52, 0x08])),
            _rlp_string(bytes([0x65, 0x53, 0xF1, 0x00])),
            _rlp_string(b"iroha-sccp-test"),
            _rlp_string(bytes([0x16]) * 32),
            _rlp_string(bytes([0x00]) * 8),
            _rlp_string(bytes([0x3B, 0x9A, 0xCA, 0x00])),
            _rlp_string(bytes([0x17]) * 32),
            _rlp_string(b""),
            _rlp_string(b""),
            _rlp_string(bytes([0x18]) * 32),
        ]
    )


def test_keccak_256_uses_ethereum_padding() -> None:
    assert (
        _keccak_256(b"").hex()
        == "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    )


def sample_witness(**overrides: Any) -> Dict[str, Any]:
    witness = {
        "target_domain": SCCP_DOMAIN_SORA,
        "finalized_slot": 321,
        "parent_slot": 320,
        "bank_signature_count": 8,
        "parent_bank_hash": "0x" + "c0" * 32,
        "blockhash": "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
        "bank_hash": HEX32_A,
        "transaction_status_root": HEX32_B,
        "message_proof_hash": HEX32_C,
        "account_inclusion_root": "0x" + "77" * 32,
        "accounts_lt_hash_checksum": "0x" + "88" * 32,
        "transaction_signature": SOLANA_SIGNATURE_55,
        "emitter_program_id": SOLANA_PROGRAM_42,
        "message_id": HEX32_D,
        "payload_hash": HEX32_E,
        "commitment_root": HEX32_F,
        "source_event_digest": "0x" + "34" * 32,
        "statement_hash": HEX32_G,
        "destination_binding_hash": HEX32_H,
    }
    witness.update(overrides)
    if (
        ("inclusion_branch" in overrides or "inclusionBranch" in overrides)
        and "transaction_status_root" not in overrides
        and "transactionStatusRoot" not in overrides
    ):
        witness["transaction_status_root"] = solana_sccp_transaction_status_root_from_branch(witness)
    return witness


def sample_production_witness(**overrides: Any) -> Dict[str, Any]:
    inclusion_branch = overrides.get("inclusion_branch", overrides.get("inclusionBranch", [HEX32_G]))
    blockhash = "0x" + "9a" * 32
    accounts_lt_hash = bytes((index % 251) + 1 for index in range(2_048))
    witness_overrides = dict(overrides)
    witness_overrides.pop("inclusion_branch", None)
    witness_overrides.pop("inclusionBranch", None)
    production_defaults = {
        "blockhash": blockhash,
        "accounts_lt_hash": accounts_lt_hash,
        "accounts_lt_hash_checksum": solana_sccp_accounts_lt_hash_checksum(accounts_lt_hash),
        "bank_hash": solana_sccp_agave_bank_hash(
            {
                "parent_bank_hash": "0x" + "c0" * 32,
                "bank_signature_count": 8,
                "blockhash": blockhash,
                "accounts_lt_hash": accounts_lt_hash,
            }
        ),
        "source_state_verifier_hash": HEX32_C,
        "source_adapter_deployment_hash": HEX32_A,
        "source_adapter_deployment_receipt_hash": HEX32_B,
    }
    production_defaults.update(witness_overrides)
    production_defaults["inclusion_branch"] = inclusion_branch
    witness = sample_witness(**production_defaults)
    if (
        len(inclusion_branch) > 0
        and "message_proof_hash" not in overrides
        and "messageProofHash" not in overrides
    ):
        witness["message_proof_hash"] = solana_sccp_message_proof_hash(witness)
    return witness


def sample_solana_route_canary_evidence(**overrides: Any) -> Dict[str, Any]:
    value = {
        "route_allowlist_hash": "0x" + "31" * 32,
        "destination_binding_hash": sccp_destination_binding_hash(SCCP_DOMAIN_SOL),
        "source_verifier_material_hash": "0x" + "33" * 32,
        "source_adapter_engine_deployment_hash": "0x" + "34" * 32,
        "verifier_identity": "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
        "verifier_code_hash": "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
        "solana_rpc_commitment": "finalized",
        "solana_program_owner": SCCP_SOLANA_UPGRADEABLE_LOADER_ID,
        "solana_programdata_owner": SCCP_SOLANA_UPGRADEABLE_LOADER_ID,
        "solana_program_immutable": True,
        "solana_program_account_data_base64": "AgAAABERERERERERERERERERERERERERERERERERERERERER",
        "solana_programdata_address": "29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2",
        "solana_programdata_slot": "4321",
        "solana_expected_programdata_slot": "4321",
        "solana_program_account_context_slot": "5000",
        "solana_programdata_account_context_slot": "5001",
        "solana_programdata_metadata_blake2b256": (
            "0x2b5f26278ea949463e97c1dc5e53a821b82515b405454a1b0e3cd652c3b00209"
        ),
        "solana_programdata_metadata_base64": (
            "AwAAAOEQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        ),
        "solana_programdata_executable_blake2b256": (
            "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411"
        ),
        "solana_programdata_executable_base64": "f0VMRgECAwQF",
    }
    value.update(overrides)
    return value


def sample_ton_route_canary_evidence(**overrides: Any) -> Dict[str, Any]:
    value = {
        "route_allowlist_hash": "0x" + "31" * 32,
        "destination_binding_hash": sccp_destination_binding_hash(SCCP_DOMAIN_TON),
        "source_verifier_material_hash": "0x" + "33" * 32,
        "source_adapter_engine_deployment_hash": "0x" + "34" * 32,
        "verifier_contract_address": "0:" + "11" * 32,
        "verifier_code_hash": "0x" + "44" * 32,
        "account_status": "active",
        "account_state_hash": "0x" + "55" * 32,
        "last_transaction_lt": "123456789",
        "last_transaction_hash": "0x" + "66" * 32,
        "verifier_code_boc_root_hash": "0x" + "44" * 32,
    }
    value.update(overrides)
    return value


def sample_tron_route_canary_evidence(**overrides: Any) -> Dict[str, Any]:
    destination_binding = sample_tron_destination_binding()
    value = {
        "route_allowlist_hash": TRON_ROUTE_ALLOWLIST_HASH_VECTOR,
        "destination_binding_hash": destination_binding["binding_hash"],
        "source_verifier_material_hash": TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "source_adapter_engine_deployment_hash": (
            TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
        "network_id": destination_binding["network_id"],
        "verifier_address": destination_binding["verifier_address"],
        "verifier_code_hash": destination_binding["verifier_code_hash"],
        "verifier_key_hash": destination_binding["verifier_key_hash"],
        "source_domain": SCCP_DOMAIN_SORA,
        "target_domain": SCCP_DOMAIN_TRON,
        "transaction_id": "0x" + "fa" * 32,
        "transaction_owner_address": (
            "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf"
        ),
        "block_number": 234,
        "block_timestamp": 567000,
        "log_index": 0,
        "message_id": "0x" + "dd" * 32,
        "call_data_sha256": (
            "0xf96dfb36d47a61e7e80df4f19e00b78c12f9a3f3c542e8dac06a7422e1d5f951"
        ),
        "payload_hash": "0x" + "ab" * 32,
        "commitment_root": "0x" + "ee" * 32,
        "finality_height": "0x" + "00" * 31 + "7b",
        "finality_block_hash": "0x" + "cd" * 32,
        "statement_hash": "0x" + "f1" * 32,
        "proof_version": 1,
        "proof_source_domain": SCCP_DOMAIN_SORA,
        "used_message_proof": True,
        "raw_data_owner_matches_transaction": True,
        "signature_sha256": "0x" + "c4" * 32,
        "signature_recovered_address": (
            "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf"
        ),
        "signature_recovers_to_owner": True,
        "route_canary_evidence_hash": TRON_ROUTE_CANARY_EVIDENCE_HASH_VECTOR,
    }
    value.update(overrides)
    return value


def sample_solana_opened_accounts_lt_hash_input(**overrides: Any) -> Dict[str, Any]:
    vote_opening = {
        "address": "0x" + "31" * 32,
        "owner": SCCP_SOLANA_VOTE_PROGRAM_ID,
        "lamports": 1_000_000,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "91" * 32,
    }
    stake_opening = {
        "address": "0x" + "32" * 32,
        "owner": SCCP_SOLANA_STAKE_PROGRAM_ID,
        "lamports": 2_000_000,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "92" * 32,
    }
    stake_history_opening = {
        "address": SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID,
        "owner": SCCP_SOLANA_SYSVAR_PROGRAM_ID,
        "lamports": 1,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "93" * 32,
    }
    unopened_opening = {
        "address": "0x" + "34" * 32,
        "owner": SCCP_SOLANA_STAKE_PROGRAM_ID,
        "lamports": 3_000_000,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "94" * 32,
    }
    vote_raw_data = b"\x01\x02\x03"
    stake_raw_data = b"\x04\x05\x06"
    stake_history_raw_data = b"\x07\x08\x09"
    unopened_raw_data = b"\x0a\x0b\x0c"
    accounts_lt_hash = solana_sccp_accounts_lt_hash_from_openings(
        [vote_opening, stake_opening, stake_history_opening, unopened_opening],
        [vote_raw_data, stake_raw_data, stake_history_raw_data, unopened_raw_data],
    )
    value = {
        "finalized_slot": 1_296_096,
        "account_inclusion_root": "0x" + "77" * 32,
        "accounts_lt_hash": accounts_lt_hash,
        "accounts_lt_hash_checksum": solana_sccp_accounts_lt_hash_checksum(accounts_lt_hash),
        "validator_vote_account_openings": [vote_opening],
        "validator_vote_account_raw_data": [vote_raw_data],
        "validator_stake_account_openings": [stake_opening],
        "validator_stake_account_raw_data": [stake_raw_data],
        "stake_history_sysvar_opening": stake_history_opening,
        "stake_history_sysvar_raw_data": stake_history_raw_data,
    }
    value.update(overrides)
    return value


def test_solana_route_canary_evidence_binds_programdata_snapshot() -> None:
    evidence = sample_solana_route_canary_evidence()

    assert len(canonical_solana_sccp_route_canary_evidence_bytes(evidence)) == 475
    assert (
        solana_sccp_route_canary_evidence_hash(evidence)
        == "0x77296e47d5681f97136dc79d66dbda4478c3c5ec80271bfd4f1f3b3dbb8e15ca"
    )
    assert (
        iroha_torii_client_package.solana_sccp_route_canary_evidence_hash(evidence)
        == "0x77296e47d5681f97136dc79d66dbda4478c3c5ec80271bfd4f1f3b3dbb8e15ca"
    )
    with pytest.raises(TypeError, match="solanaExpectedProgramdataSlot"):
        solana_sccp_route_canary_evidence_hash(
            sample_solana_route_canary_evidence(solana_programdata_slot="4322")
        )
    with pytest.raises(TypeError, match="BPF ELF"):
        solana_sccp_route_canary_evidence_hash(
            sample_solana_route_canary_evidence(
                solana_programdata_executable_base64="AQIDBA=="
            )
        )
    with pytest.raises(
        TypeError,
        match="destinationBindingHash must match canonical Solana destination binding",
    ):
        solana_sccp_route_canary_evidence_hash(
            sample_solana_route_canary_evidence(destination_binding_hash=HEX32_H)
        )
    with pytest.raises(
        TypeError,
        match="expectedDestinationBindingHash must match canonical Solana destination binding",
    ):
        solana_sccp_route_canary_evidence_hash(
            sample_solana_route_canary_evidence(expected_destination_binding_hash=HEX32_H)
        )


def test_ton_route_canary_evidence_binds_live_account_snapshot() -> None:
    evidence = sample_ton_route_canary_evidence()

    assert len(canonical_ton_sccp_route_canary_evidence_bytes(evidence)) == 358
    assert (
        ton_sccp_route_canary_evidence_hash(evidence)
        == "0xf128e8405017b9ca7733bb10d43eeaf783e38d39740a3455aa353c76655c6942"
    )
    assert (
        iroha_torii_client_package.ton_sccp_route_canary_evidence_hash(evidence)
        == "0xf128e8405017b9ca7733bb10d43eeaf783e38d39740a3455aa353c76655c6942"
    )
    with pytest.raises(
        TypeError,
        match="destinationBindingHash must match canonical TON destination binding",
    ):
        ton_sccp_route_canary_evidence_hash(
            sample_ton_route_canary_evidence(destination_binding_hash=HEX32_H)
        )
    with pytest.raises(TypeError, match="verifierContractAddress workchain"):
        ton_sccp_route_canary_evidence_hash(
            sample_ton_route_canary_evidence(verifier_contract_address="1:" + "11" * 32)
        )
    with pytest.raises(TypeError, match="accountStatus must be active"):
        ton_sccp_route_canary_evidence_hash(
            sample_ton_route_canary_evidence(account_status="uninit")
        )
    with pytest.raises(TypeError, match="lastTransactionLt must be a positive decimal"):
        ton_sccp_route_canary_evidence_hash(
            sample_ton_route_canary_evidence(last_transaction_lt="0123")
        )
    with pytest.raises(TypeError, match="verifierCodeBocRootHash"):
        ton_sccp_route_canary_evidence_hash(
            sample_ton_route_canary_evidence(verifier_code_boc_root_hash="0x" + "45" * 32)
        )
    with pytest.raises(TypeError, match="accountStatus must not use multiple aliases"):
        ton_sccp_route_canary_evidence_hash(
            sample_ton_route_canary_evidence(accountStatus="active")
        )


def test_tron_route_canary_evidence_binds_transaction_transcript() -> None:
    evidence = sample_tron_route_canary_evidence()

    assert len(canonical_tron_sccp_route_canary_evidence_bytes(evidence)) == 551
    assert (
        tron_sccp_route_canary_evidence_hash(evidence)
        == TRON_ROUTE_CANARY_EVIDENCE_HASH_VECTOR
    )
    assert (
        iroha_torii_client_package.tron_sccp_route_canary_evidence_hash(evidence)
        == TRON_ROUTE_CANARY_EVIDENCE_HASH_VECTOR
    )
    with pytest.raises(TypeError, match="routeAllowlistHash must match canonical"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(route_allowlist_hash=HEX32_H)
        )
    with pytest.raises(
        TypeError,
        match=r"destinationBinding\.bindingHash must match destinationBinding",
    ):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(destination_binding_hash=HEX32_H)
        )
    with pytest.raises(ValueError, match="targetDomain must be TRON"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(target_domain=SCCP_DOMAIN_ETH)
        )
    with pytest.raises(ValueError, match="blockNumber must be positive"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(block_number=0)
        )
    with pytest.raises(TypeError, match="usedMessageProof must be true"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(used_message_proof=False)
        )
    with pytest.raises(TypeError, match="rawDataOwnerMatchesTransaction must be true"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(raw_data_owner_matches_transaction=False)
        )
    with pytest.raises(TypeError, match="signatureRecoversToOwner must be true"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(signature_recovers_to_owner=False)
        )
    with pytest.raises(TypeError, match="signatureRecoveredAddress must match"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(
                signature_recovered_address="0x41" + "12" * 20
            )
        )
    with pytest.raises(TypeError, match="targetDomain must not use multiple aliases"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(targetDomain=SCCP_DOMAIN_TRON)
        )
    with pytest.raises(TypeError, match="routeCanaryEvidenceHash must match"):
        tron_sccp_route_canary_evidence_hash(
            sample_tron_route_canary_evidence(route_canary_evidence_hash=HEX32_H)
        )


def sample_solana_accounts_lt_hash_proof_input(**overrides: Any) -> Dict[str, Any]:
    opened_overrides = overrides.pop("opened", {})
    opened = sample_solana_opened_accounts_lt_hash_input(**opened_overrides)
    parent_bank_hash = "0x" + "c0" * 32
    blockhash = "0x" + "42" * 32
    bank_signature_count = 8
    bank_hash = solana_sccp_agave_bank_hash(
        {
            "parent_bank_hash": parent_bank_hash,
            "bank_signature_count": bank_signature_count,
            "blockhash": blockhash,
            "accounts_lt_hash": opened["accounts_lt_hash"],
        }
    )
    value = {
        **opened,
        "parent_slot": 1_296_095,
        "bank_signature_count": bank_signature_count,
        "parent_bank_hash": parent_bank_hash,
        "blockhash": blockhash,
        "bank_hash": bank_hash,
        "transaction_status_root": HEX32_B,
        "source_state_verifier_hash": HEX32_A,
    }
    value.update(overrides)
    return value


def sample_solana_full_light_client_audit_proof_input(**overrides: Any) -> Dict[str, Any]:
    source_state_verifier_hash = "0x" + "99" * 32
    base = sample_solana_accounts_lt_hash_proof_input(
        source_state_verifier_hash=source_state_verifier_hash,
    )
    value = {
        **sample_source_record_input(SCCP_DOMAIN_SOL),
        "source_state_verifier_hash": source_state_verifier_hash,
        "solana_tower_replay_verifier_hash": "0x" + "b1" * 32,
        "solana_full_accountsdb_lattice_verifier_hash": "0x" + "c2" * 32,
        "solana_bank_fork_choice_verifier_hash": "0x" + "d3" * 32,
        **base,
        "message_proof_hash": HEX32_C,
        "source_event_digest": "0x" + "34" * 32,
        "transaction_signature": SOLANA_SIGNATURE_55,
        "emitter_program_id": SOLANA_PROGRAM_42,
        "message_id": HEX32_D,
        "payload_hash": HEX32_E,
        "commitment_root": HEX32_F,
        "epoch": 3,
        "rooted_slot": 1_296_065,
        "tower_vote_slots": list(
            range(1_296_066, 1_296_066 + SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH)
        ),
        "epoch_stake_root": "0x" + "13" * 32,
        "stake_activation_hash": "0x" + "14" * 32,
        "stake_account_state_hash": "0x" + "15" * 32,
        "stake_history_hash": "0x" + "16" * 32,
        "stake_history_sysvar_account_hash": "0x" + "17" * 32,
        "accounts_lt_hash_proof": {
            "version": 1,
            "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
            "circuit_id": SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
            "proof_bytes": b"\x01\x02\x03\x04",
        },
    }
    value.update(overrides)
    if (
        "source_adapter_deployment_hash" not in overrides
        and "sourceAdapterDeploymentHash" not in overrides
    ):
        value["source_adapter_deployment_hash"] = (
            sccp_source_adapter_engine_deployment_hash(value)
        )
    if (
        "source_adapter_deployment_receipt_hash" not in overrides
        and "sourceAdapterDeploymentReceiptHash" not in overrides
    ):
        value["source_adapter_deployment_receipt_hash"] = value[
            "deployment_receipt_hash"
        ]
    return value


def sample_ton_public_inputs(**overrides: Any) -> Dict[str, Any]:
    public_inputs = {
        "version": 1,
        "message_id": HEX32_D,
        "payload_hash": HEX32_E,
        "target_domain": SCCP_DOMAIN_TON,
        "commitment_root": HEX32_F,
        "finality_height": 19,
        "finality_block_hash": HEX32_A,
    }
    public_inputs.update(overrides)
    return public_inputs


def sample_ton_request_input(**overrides: Any) -> Dict[str, Any]:
    request = {
        "public_inputs": sample_ton_public_inputs(),
        "bundle_bytes": bytes([5, 6, 7]),
        "source_proof_bytes": bytes([9, 10]),
        "statement_hash": HEX32_G,
        "destination_binding_hash": HEX32_H,
        "source_state_verifier_hash": HEX32_C,
    }
    request.update(overrides)
    return request


def sample_ton_message_body_input(**overrides: Any) -> Dict[str, Any]:
    value = {
        "public_inputs": sample_ton_public_inputs(),
        "proof_bytes": bytes([1, 2, 3, 4]),
        "bundle_bytes": bytes([5, 6, 7]),
        "statement_hash": HEX32_B,
        "destination_binding_hash": HEX32_G,
        "metadata_bytes": bytes([8, 9]),
    }
    value.update(overrides)
    return value


def sample_ton_message_body_input_with_result(**overrides: Any) -> Dict[str, Any]:
    value = sample_ton_message_body_input()
    value.update(overrides)
    if "proof_result" not in value and "proofResult" not in value:
        request = build_ton_sccp_proof_request(
            sample_ton_request_input(
                public_inputs=value["public_inputs"],
                bundle_bytes=value["bundle_bytes"],
                source_proof_bytes=value.get("source_proof_bytes", bytes([9, 10])),
                statement_hash=value["statement_hash"],
                destination_binding_hash=value["destination_binding_hash"],
                source_state_verifier_hash=HEX32_C,
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
        value["proof_result"] = wrap_ton_sccp_proof_result(value["proof_bytes"], request)
    return value


def sample_tron_public_inputs(**overrides: Any) -> Dict[str, Any]:
    public_inputs = {
        "version": 1,
        "message_id": "0x" + "11" * 32,
        "payload_hash": "0x" + "22" * 32,
        "target_domain": SCCP_DOMAIN_TRON,
        "commitment_root": "0x" + "33" * 32,
        "finality_height": "19",
        "finality_block_hash": "0x" + "44" * 32,
    }
    public_inputs.update(overrides)
    return public_inputs


def sample_tron_request_input(**overrides: Any) -> Dict[str, Any]:
    request = {
        "public_inputs": sample_tron_public_inputs(),
        "bundle_bytes": bytes([5, 6, 7]),
        "source_proof_bytes": bytes([9, 10]),
        "source_domain": SCCP_DOMAIN_SORA,
        "statement_hash": "0x" + "55" * 32,
        "destination_binding_hash": "0x" + "66" * 32,
    }
    request.update(overrides)
    return request


def sample_tron_destination_binding(**overrides: Any) -> Dict[str, Any]:
    binding = {
        "network_id": "0x" + "33" * 32,
        "verifier_address": "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        "verifier_code_hash": "0x" + "bb" * 32,
        "verifier_key_hash": "0x" + "cc" * 32,
    }
    binding.update(overrides)
    return tron_sccp_destination_binding(binding)


def sample_tron_production_request_input(**overrides: Any) -> Dict[str, Any]:
    request = sample_tron_request_input(
        destination_binding=sample_tron_destination_binding(),
        destination_binding_hash=None,
    )
    request.update(overrides)
    return request


def sample_evm_public_inputs(**overrides: Any) -> Dict[str, Any]:
    public_inputs = {
        "version": 1,
        "message_id": "0x" + "11" * 32,
        "payload_hash": "0x" + "22" * 32,
        "target_domain": SCCP_DOMAIN_ETH,
        "commitment_root": "0x" + "33" * 32,
        "finality_height": "19",
        "finality_block_hash": "0x" + "44" * 32,
    }
    public_inputs.update(overrides)
    return public_inputs


def sample_evm_request_input(**overrides: Any) -> Dict[str, Any]:
    request = {
        "public_inputs": sample_evm_public_inputs(),
        "bundle_bytes": bytes([5, 6, 7]),
        "source_proof_bytes": bytes([9, 10]),
        "source_domain": SCCP_DOMAIN_SORA,
        "statement_hash": "0x" + "55" * 32,
        "destination_binding_hash": "0x" + "66" * 32,
    }
    request.update(overrides)
    return request


def sample_evm_destination_binding(**overrides: Any) -> Dict[str, Any]:
    binding = {
        "target_domain": SCCP_DOMAIN_ETH,
        "network_id": "0x" + "33" * 32,
        "verifier_address": "0x" + "11" * 20,
        "bridge_address": "0x" + "22" * 20,
        "verifier_code_hash": "0x" + "bb" * 32,
        "verifier_key_hash": "0x" + "cc" * 32,
    }
    binding.update(overrides)
    return evm_sccp_destination_binding(binding)


def sample_evm_production_request_input(**overrides: Any) -> Dict[str, Any]:
    request = sample_evm_request_input(
        destination_binding=sample_evm_destination_binding(),
        destination_binding_hash=None,
    )
    request.update(overrides)
    return request


def sample_substrate_public_inputs(**overrides: Any) -> Dict[str, Any]:
    public_inputs = {
        "version": 1,
        "message_id": "0x" + "21" * 32,
        "payload_hash": "0x" + "22" * 32,
        "target_domain": SCCP_DOMAIN_SORA2,
        "commitment_root": "0x" + "23" * 32,
        "finality_height": "42",
        "finality_block_hash": "0x" + "24" * 32,
    }
    public_inputs.update(overrides)
    return public_inputs


def sample_substrate_request_input(**overrides: Any) -> Dict[str, Any]:
    request = {
        "public_inputs": sample_substrate_public_inputs(),
        "bundle_bytes": bytes([5, 6, 7]),
        "source_proof_bytes": bytes([9, 10]),
        "source_domain": SCCP_DOMAIN_SORA,
        "statement_hash": "0x" + "55" * 32,
        "destination_binding_hash": "0x" + "66" * 32,
    }
    request.update(overrides)
    return request


def test_normalizes_solana_sccp_witness_input_for_local_proof_requests() -> None:
    witness = normalize_solana_sccp_witness(sample_witness())

    assert witness["version"] == 1
    assert witness["source_domain"] == SCCP_DOMAIN_SOL
    assert witness["target_domain"] == SCCP_DOMAIN_SORA
    assert witness["mainnet_genesis_hash"] == SCCP_SOLANA_MAINNET_GENESIS_HASH
    assert witness["finalized_slot"] == "321"
    assert witness["parent_slot"] == "320"
    assert witness["bank_signature_count"] == "8"
    assert re.fullmatch(r"0x[0-9a-f]{64}", witness["blockhash"])
    assert canonical_solana_sccp_witness_bytes(
        sample_witness()
    ) == canonical_solana_sccp_witness_bytes(
        sample_witness(blockhash=witness["blockhash"])
    )
    assert build_solana_sccp_proof_request(
        sample_witness()
    )["witness_hash"] == build_solana_sccp_proof_request(
        sample_witness(blockhash=witness["blockhash"])
    )["witness_hash"]
    assert witness[
        "accounts_lt_hash_proof_public_inputs_hash"
    ] == solana_sccp_accounts_lt_hash_proof_public_inputs_hash(witness)
    assert witness["message_id"] == HEX32_D
    assert witness["source_event_digest"] == "0x" + "34" * 32
    assert witness["source_state_verifier_id"] == SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1
    assert witness["source_state_verifier_hash"] == SCCP_ZERO_HASH_V1
    assert witness["source_adapter_deployment_hash"] == SCCP_ZERO_HASH_V1
    assert witness["source_adapter_deployment_receipt_hash"] == SCCP_ZERO_HASH_V1
    assert len(canonical_solana_sccp_witness_bytes(witness)) > 0
    for field, expected_error in (
        ("source_state_verifier_id", "sourceStateVerifierId"),
        ("source_state_verifier_hash", "sourceStateVerifierHash"),
        ("source_adapter_deployment_hash", "sourceAdapterDeploymentHash"),
        ("source_adapter_deployment_receipt_hash", "sourceAdapterDeploymentReceiptHash"),
    ):
        with pytest.raises(TypeError, match=expected_error):
            normalize_solana_sccp_witness(sample_witness(**{field: ""}))


def test_derives_solana_message_proof_hash_from_inclusion_witness() -> None:
    inclusion_branch = [HEX32_G]
    leaf_input = {
        "source_event_digest": "0x" + "34" * 32,
        "transaction_signature": SOLANA_SIGNATURE_55,
        "emitter_program_id": SOLANA_PROGRAM_42,
    }
    root_input = {**leaf_input, "inclusion_branch": inclusion_branch}
    transaction_status_root = solana_sccp_transaction_status_root_from_branch(root_input)
    assert (
        transaction_status_root
        == "0xb048ca31d8ad7b2a0d15cbeb81d536350743483d44dd93136e859df93d3863b2"
    )
    message_input = {**root_input, "transaction_status_root": transaction_status_root}
    derived = solana_sccp_message_proof_hash(message_input)

    assert derived.startswith("0x") and len(derived) == 66
    assert len(canonical_solana_sccp_transaction_status_leaf_bytes(leaf_input)) > 0
    assert solana_sccp_transaction_status_leaf_hash(
        leaf_input
    ) == "0x4e12efed6d53466de0596f05aa6cc767df1efd6a4d1549276c4ec8b69118515d"
    for patch, pattern in (
        (
            {"sourceEventDigest": leaf_input["source_event_digest"]},
            "sourceEventDigest must not use multiple aliases",
        ),
        (
            {"transactionSignature": SOLANA_SIGNATURE_55},
            "transactionSignature must not use multiple aliases",
        ),
        (
            {"emitterProgramId": SOLANA_PROGRAM_42},
            "emitterProgramId must not use multiple aliases",
        ),
    ):
        with pytest.raises(TypeError, match=pattern):
            solana_sccp_transaction_status_leaf_hash({**leaf_input, **patch})
    with pytest.raises(TypeError, match="inclusionBranch must not use multiple aliases"):
        solana_sccp_transaction_status_root_from_branch(
            {**root_input, "inclusionBranch": inclusion_branch}
        )
    for patch, pattern in (
        (
            {"sourceEventDigest": leaf_input["source_event_digest"]},
            "sourceEventDigest must not use multiple aliases",
        ),
        (
            {"receiptOrMessageRoot": transaction_status_root},
            "transactionStatusRoot must not use multiple aliases",
        ),
        (
            {"transactionSignature": SOLANA_SIGNATURE_55},
            "transactionSignature must not use multiple aliases",
        ),
        (
            {"emitterProgramId": SOLANA_PROGRAM_42},
            "emitterProgramId must not use multiple aliases",
        ),
        (
            {"inclusionBranch": inclusion_branch},
            "inclusionBranch must not use multiple aliases",
        ),
    ):
        with pytest.raises(TypeError, match=pattern):
            solana_sccp_message_proof_hash({**message_input, **patch})
    with pytest.raises(TypeError, match="transactionSignature must not decode to zero"):
        solana_sccp_transaction_status_leaf_hash(
            {
                "source_event_digest": "0x" + "34" * 32,
                "transaction_signature": SOLANA_ZERO_SIGNATURE,
                "emitter_program_id": SOLANA_PROGRAM_42,
            }
        )
    with pytest.raises(TypeError, match="emitterProgramId must not decode to zero"):
        solana_sccp_transaction_status_leaf_hash(
            {
                "source_event_digest": "0x" + "34" * 32,
                "transaction_signature": SOLANA_SIGNATURE_55,
                "emitter_program_id": SOLANA_ZERO_PROGRAM,
            }
        )
    assert (
        len(
            canonical_solana_sccp_message_proof_bytes(message_input)
        )
        > 0
    )
    with pytest.raises(TypeError, match="sourceEventDigest must not be zero"):
        solana_sccp_message_proof_hash(
            {
                "source_event_digest": "0x" + "00" * 32,
                "transaction_status_root": transaction_status_root,
                "transaction_signature": SOLANA_SIGNATURE_55,
                "emitter_program_id": SOLANA_PROGRAM_42,
                "inclusion_branch": inclusion_branch,
            }
        )
    with pytest.raises(TypeError, match="transactionStatusRoot must not be zero"):
        solana_sccp_message_proof_hash(
            {
                "source_event_digest": "0x" + "34" * 32,
                "transaction_status_root": "0x" + "00" * 32,
                "transaction_signature": SOLANA_SIGNATURE_55,
                "emitter_program_id": SOLANA_PROGRAM_42,
                "inclusion_branch": inclusion_branch,
            }
        )
    with pytest.raises(TypeError, match="transactionSignature must not decode to zero"):
        solana_sccp_message_proof_hash(
            {
                "source_event_digest": "0x" + "34" * 32,
                "transaction_status_root": transaction_status_root,
                "transaction_signature": SOLANA_ZERO_SIGNATURE,
                "emitter_program_id": SOLANA_PROGRAM_42,
                "inclusion_branch": inclusion_branch,
            }
        )
    with pytest.raises(TypeError, match="emitterProgramId must not decode to zero"):
        solana_sccp_message_proof_hash(
            {
                "source_event_digest": "0x" + "34" * 32,
                "transaction_status_root": transaction_status_root,
                "transaction_signature": SOLANA_SIGNATURE_55,
                "emitter_program_id": SOLANA_ZERO_PROGRAM,
                "inclusion_branch": inclusion_branch,
            }
        )
    normalized = normalize_solana_sccp_witness(
        sample_witness(message_proof_hash="", inclusion_branch=inclusion_branch)
    )
    assert normalized["message_proof_hash"] == derived
    assert normalized["inclusion_branch"] == inclusion_branch
    with pytest.raises(TypeError, match="sourceEventDigest must not be zero"):
        normalize_solana_sccp_witness(
            sample_witness(source_event_digest="0x" + "00" * 32, inclusion_branch=inclusion_branch)
        )
    with pytest.raises(TypeError, match="transactionSignature must not decode to zero"):
        normalize_solana_sccp_witness(
            sample_witness(
                transaction_signature=SOLANA_ZERO_SIGNATURE,
                inclusion_branch=inclusion_branch,
            )
        )
    with pytest.raises(TypeError, match="emitterProgramId must not decode to zero"):
        normalize_solana_sccp_witness(
            sample_witness(
                emitter_program_id=SOLANA_ZERO_PROGRAM,
                inclusion_branch=inclusion_branch,
            )
        )
    assert derived != solana_sccp_message_proof_hash(
        {
            "source_event_digest": "0x" + "34" * 32,
            "transaction_status_root": transaction_status_root,
            "transaction_signature": SOLANA_SIGNATURE_01,
            "emitter_program_id": SOLANA_PROGRAM_42,
            "inclusion_branch": inclusion_branch,
        }
    )
    assert derived != solana_sccp_message_proof_hash(
        {
            "source_event_digest": "0x" + "34" * 32,
            "transaction_status_root": transaction_status_root,
            "transaction_signature": SOLANA_SIGNATURE_55,
            "emitter_program_id": SOLANA_PROGRAM_02,
            "inclusion_branch": inclusion_branch,
        }
    )

    with pytest.raises(TypeError, match="messageProofHash must match inclusionBranch"):
        normalize_solana_sccp_witness(
            sample_witness(message_proof_hash=HEX32_C, inclusion_branch=inclusion_branch)
        )
    with pytest.raises(ValueError, match="inclusionBranch must not be empty"):
        solana_sccp_message_proof_hash(
            {
                "source_event_digest": "0x" + "34" * 32,
                "transaction_status_root": transaction_status_root,
                "transaction_signature": SOLANA_SIGNATURE_55,
                "emitter_program_id": SOLANA_PROGRAM_42,
                "inclusion_branch": [],
            }
        )
    with pytest.raises(TypeError, match="transactionSignature must be canonical base58"):
        solana_sccp_message_proof_hash(
            {
                "source_event_digest": "0x" + "34" * 32,
                "transaction_status_root": HEX32_B,
                "transaction_signature": "not-a-solana-signature",
                "emitter_program_id": SOLANA_PROGRAM_42,
                "inclusion_branch": inclusion_branch,
            }
        )


def test_derives_solana_epoch_stake_root_for_vote_witnesses() -> None:
    input_value = {
        "epoch": 3,
        "validator_public_keys": ["0x" + "11" * 32, "0x" + "22" * 32],
        "validator_stakes": [1, 2],
    }

    assert SCCP_SOLANA_MAINNET_SLOTS_PER_EPOCH == 432_000
    assert solana_sccp_mainnet_epoch_for_slot(864_000) == 2
    assert len(canonical_solana_sccp_epoch_stake_root_bytes(input_value)) == 134
    assert (
        solana_sccp_epoch_stake_root(input_value)
        == "0x1d86a5ecfac6e63bfcefdc1a3bfefd962a33e2a4cf65cd4e8518bcebea771f0a"
    )
    assert solana_sccp_epoch_stake_root(
        {
            "finalized_slot": 1_296_000,
            "validatorPublicKeys": input_value["validator_public_keys"],
            "validatorStakes": input_value["validator_stakes"],
        }
    ) == solana_sccp_epoch_stake_root(input_value)

    with pytest.raises(TypeError, match="validatorPublicKeys must not use multiple aliases"):
        solana_sccp_epoch_stake_root(
            {
                **input_value,
                "validatorPublicKeys": input_value["validator_public_keys"],
            }
        )
    with pytest.raises(TypeError, match="validatorStakes must not use multiple aliases"):
        solana_sccp_epoch_stake_root(
            {
                **input_value,
                "validatorStakes": input_value["validator_stakes"],
            }
        )

    with pytest.raises(TypeError, match=r"validatorPublicKeys\[0\] must be 32 bytes"):
        solana_sccp_epoch_stake_root(
            {
                **input_value,
                "validator_public_keys": ["0x" + "11" * 31],
                "validator_stakes": [1],
            }
        )

    with pytest.raises(TypeError, match=r"validatorPublicKeys\[0\] must not be zero"):
        solana_sccp_epoch_stake_root(
            {
                **input_value,
                "validator_public_keys": ["0x" + "00" * 32],
                "validator_stakes": [1],
            }
        )

    oversized_validator_public_keys = [
        "0x" + "00" * 24 + f"{index + 1:016x}"
        for index in range(SCCP_SOLANA_MAX_VALIDATORS + 1)
    ]
    with pytest.raises(ValueError, match=r"validatorPublicKeys must contain 1\.\.8192 entries"):
        solana_sccp_epoch_stake_root(
            {
                **input_value,
                "validator_public_keys": oversized_validator_public_keys,
                "validator_stakes": [1] * len(oversized_validator_public_keys),
            }
        )


def test_derives_solana_tower_lockout_hash_for_finality_context() -> None:
    input_value = {
        "finalized_slot": 1_296_096,
        "rooted_slot": 1_296_065,
        "parent_slot": 1_296_095,
        "parent_bank_hash": "0x" + "33" * 32,
    }

    assert SCCP_SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH == 32
    assert SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH == 31
    assert len(canonical_solana_sccp_tower_lockout_bytes(input_value)) == 73
    assert solana_sccp_tower_lockout_hash(input_value).startswith("0x")
    assert solana_sccp_tower_lockout_hash(
        {**input_value, "epoch": 3}
    ) == solana_sccp_tower_lockout_hash(input_value)

    with pytest.raises(TypeError, match="finalizedSlot must not use multiple aliases"):
        solana_sccp_tower_lockout_hash(
            {**input_value, "finalizedSlot": input_value["finalized_slot"]}
        )

    with pytest.raises(TypeError, match="epoch must not use multiple aliases"):
        solana_sccp_tower_lockout_hash({**input_value, "epoch": 3, "validatorEpoch": 3})

    with pytest.raises(TypeError, match="rootedSlot must not use multiple aliases"):
        solana_sccp_tower_lockout_hash(
            {**input_value, "rootedSlot": input_value["rooted_slot"]}
        )

    with pytest.raises(TypeError, match="parentBankHash must not use multiple aliases"):
        solana_sccp_tower_lockout_hash(
            {**input_value, "parentBankHash": input_value["parent_bank_hash"]}
        )

    with pytest.raises(ValueError, match="epoch must match Solana mainnet finalizedSlot"):
        solana_sccp_tower_lockout_hash({**input_value, "epoch": 4})

    with pytest.raises(ValueError, match="rootedSlot must satisfy"):
        solana_sccp_tower_lockout_hash({**input_value, "rooted_slot": 1_296_066})

    with pytest.raises(ValueError, match="parentSlot must be the direct parent"):
        solana_sccp_tower_lockout_hash({**input_value, "parent_slot": 1_296_094})

    with pytest.raises(TypeError, match="parentBankHash must not be zero"):
        solana_sccp_tower_lockout_hash(
            {**input_value, "parent_bank_hash": "0x" + "00" * 32}
        )


def test_derives_solana_tower_replay_hash_for_finality_context() -> None:
    input_value = {
        "finalized_slot": 1_296_096,
        "rooted_slot": 1_296_065,
        "parent_slot": 1_296_095,
        "bank_fork_hash": HEX32_A,
        "tower_vote_slots": list(range(1_296_066, 1_296_097)),
    }

    assert len(canonical_solana_sccp_tower_replay_bytes(input_value)) == 573
    assert solana_sccp_tower_replay_hash(input_value).startswith("0x")
    assert solana_sccp_tower_replay_hash(
        {**input_value, "epoch": 3}
    ) == solana_sccp_tower_replay_hash(input_value)
    assert solana_sccp_tower_replay_hash(
        {**input_value, "bank_fork_hash": HEX32_B}
    ) != solana_sccp_tower_replay_hash(input_value)
    with pytest.raises(TypeError, match="finalizedSlot must not use multiple aliases"):
        solana_sccp_tower_replay_hash(
            {**input_value, "finalizedSlot": input_value["finalized_slot"]}
        )
    with pytest.raises(TypeError, match="bankForkHash must not use multiple aliases"):
        solana_sccp_tower_replay_hash(
            {**input_value, "bankForkHash": input_value["bank_fork_hash"]}
        )
    with pytest.raises(TypeError, match="towerVoteSlots must not use multiple aliases"):
        solana_sccp_tower_replay_hash(
            {**input_value, "voteSlots": input_value["tower_vote_slots"]}
        )
    with pytest.raises(TypeError, match="bankForkHash must not be zero"):
        solana_sccp_tower_replay_hash(
            {**input_value, "bank_fork_hash": SCCP_ZERO_HASH_V1}
        )

    with pytest.raises(ValueError, match="epoch must match Solana mainnet finalizedSlot"):
        solana_sccp_tower_replay_hash({**input_value, "epoch": 4})

    with pytest.raises(ValueError, match="towerVoteSlots must contain 31 active post-root slots"):
        solana_sccp_tower_replay_hash(
            {**input_value, "tower_vote_slots": input_value["tower_vote_slots"][1:]}
        )

    unsorted_vote_slots = list(input_value["tower_vote_slots"])
    unsorted_vote_slots[0], unsorted_vote_slots[1] = (
        unsorted_vote_slots[1],
        unsorted_vote_slots[0],
    )
    with pytest.raises(ValueError, match="towerVoteSlots must be strictly increasing"):
        solana_sccp_tower_replay_hash(
            {**input_value, "tower_vote_slots": unsorted_vote_slots}
        )

    wrong_last_vote_slots = list(input_value["tower_vote_slots"])
    wrong_last_vote_slots[-1] -= 1
    with pytest.raises(ValueError, match="last towerVoteSlots entry must equal finalizedSlot"):
        solana_sccp_tower_replay_hash(
            {**input_value, "tower_vote_slots": wrong_last_vote_slots}
        )


def test_derives_solana_stake_activation_hash_for_finality_context() -> None:
    input_value = {
        "epoch": 3,
        "validator_public_keys": ["0x" + "11" * 32, "0x" + "22" * 32],
        "validator_stakes": [1, 2],
        "validator_activation_epochs": [0, 2],
        "validator_deactivation_epochs": [2**64 - 1, 9],
    }

    assert len(canonical_solana_sccp_stake_activation_bytes(input_value)) == 165
    assert (
        solana_sccp_stake_activation_hash(input_value)
        == "0xdb418c62a1aeb8ae15cb26e3a198d46890cefa3545df8e1921be2e83f57dabf3"
    )
    with pytest.raises(TypeError, match="epoch must not use multiple aliases"):
        solana_sccp_stake_activation_hash(
            {**input_value, "validatorEpoch": input_value["epoch"]}
        )
    with pytest.raises(TypeError, match="validatorActivationEpochs must not use multiple aliases"):
        solana_sccp_stake_activation_hash(
            {
                **input_value,
                "activationEpochs": input_value["validator_activation_epochs"],
            }
        )
    with pytest.raises(TypeError, match="validatorDeactivationEpochs must not use multiple aliases"):
        solana_sccp_stake_activation_hash(
            {
                **input_value,
                "deactivationEpochs": input_value["validator_deactivation_epochs"],
            }
        )
    with pytest.raises(ValueError, match=r"validatorActivationEpochs\[0\]"):
        solana_sccp_stake_activation_hash(
            {**input_value, "validator_activation_epochs": [4, 2]}
        )
    with pytest.raises(ValueError, match=r"validatorActivationEpochs\[0\]"):
        solana_sccp_stake_activation_hash(
            {**input_value, "validator_activation_epochs": [3, 2]}
        )
    with pytest.raises(ValueError, match=r"validatorDeactivationEpochs\[1\]"):
        solana_sccp_stake_activation_hash(
            {**input_value, "validator_deactivation_epochs": [2**64 - 1, 2]}
        )
    assert len(
        solana_sccp_stake_activation_hash(
            {**input_value, "validator_deactivation_epochs": [2**64 - 1, 3]}
        )
    ) == 66
    with pytest.raises(ValueError, match="validator activation epochs must match"):
        solana_sccp_stake_activation_hash(
            {**input_value, "validator_activation_epochs": [0]}
        )


def test_derives_solana_account_opening_hash_for_finality_context() -> None:
    input_value = {
        "address": "0x" + "31" * 32,
        "owner": SCCP_SOLANA_VOTE_PROGRAM_ID,
        "lamports": 1_000_000,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "71" * 32,
    }

    assert len(canonical_solana_sccp_account_opening_bytes(input_value)) == 122
    account_hash = solana_sccp_account_opening_hash(input_value)
    assert account_hash.startswith("0x") and len(account_hash) == 66
    assert account_hash != solana_sccp_account_opening_hash(
        {**input_value, "owner": SCCP_SOLANA_STAKE_PROGRAM_ID}
    )
    assert account_hash != solana_sccp_account_opening_hash(
        {**input_value, "executable": True}
    )
    with pytest.raises(TypeError, match="address must not use multiple aliases"):
        solana_sccp_account_opening_hash(
            {**input_value, "accountAddress": input_value["address"]}
        )
    with pytest.raises(TypeError, match="owner must not use multiple aliases"):
        solana_sccp_account_opening_hash(
            {**input_value, "ownerProgramId": input_value["owner"]}
        )
    with pytest.raises(TypeError, match="rentEpoch must not use multiple aliases"):
        solana_sccp_account_opening_hash(
            {**input_value, "rentEpoch": input_value["rent_epoch"]}
        )
    with pytest.raises(TypeError, match="dataHash must not use multiple aliases"):
        solana_sccp_account_opening_hash(
            {**input_value, "dataHash": input_value["data_hash"]}
        )
    with pytest.raises(ValueError, match="lamports"):
        solana_sccp_account_opening_hash({**input_value, "lamports": 0})


def test_derives_solana_opened_accounts_lt_hash_contribution_bindings() -> None:
    vote_opening = {
        "address": "0x" + "31" * 32,
        "owner": SCCP_SOLANA_VOTE_PROGRAM_ID,
        "lamports": 1_000_000,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "91" * 32,
    }
    stake_opening = {
        "address": "0x" + "32" * 32,
        "owner": SCCP_SOLANA_STAKE_PROGRAM_ID,
        "lamports": 2_000_000,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "92" * 32,
    }
    stake_history_opening = {
        "address": SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID,
        "owner": SCCP_SOLANA_SYSVAR_PROGRAM_ID,
        "lamports": 1,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "93" * 32,
    }
    unopened_opening = {
        "address": "0x" + "34" * 32,
        "owner": SCCP_SOLANA_STAKE_PROGRAM_ID,
        "lamports": 3_000_000,
        "rent_epoch": 0,
        "executable": False,
        "data_hash": "0x" + "94" * 32,
    }
    vote_raw_data = b"\x01\x02\x03"
    stake_raw_data = b"\x04\x05\x06"
    stake_history_raw_data = b"\x07\x08\x09"
    unopened_raw_data = b"\x0a\x0b\x0c"
    accounts_lt_hash = solana_sccp_accounts_lt_hash_from_openings(
        [vote_opening, stake_opening, stake_history_opening, unopened_opening],
        [vote_raw_data, stake_raw_data, stake_history_raw_data, unopened_raw_data],
    )
    opened_lt_hash = solana_sccp_accounts_lt_hash_from_openings(
        [vote_opening, stake_opening, stake_history_opening],
        [vote_raw_data, stake_raw_data, stake_history_raw_data],
    )
    unopened_lt_hash = solana_sccp_accounts_lt_hash_from_openings(
        [unopened_opening],
        [unopened_raw_data],
    )
    input_value = {
        "finalized_slot": 1_296_096,
        "account_inclusion_root": "0x" + "77" * 32,
        "accounts_lt_hash": accounts_lt_hash,
        "accounts_lt_hash_checksum": solana_sccp_accounts_lt_hash_checksum(accounts_lt_hash),
        "validator_vote_account_openings": [vote_opening],
        "validator_vote_account_raw_data": [vote_raw_data],
        "validator_stake_account_openings": [stake_opening],
        "validator_stake_account_raw_data": [stake_raw_data],
        "stake_history_sysvar_opening": stake_history_opening,
        "stake_history_sysvar_raw_data": stake_history_raw_data,
    }

    assert solana_sccp_accounts_lt_hash_opened_residual(input_value) == unopened_lt_hash
    assert solana_sccp_accounts_lt_hash_opened_residual_checksum(
        input_value
    ) == solana_sccp_accounts_lt_hash_checksum(unopened_lt_hash)
    assert len(canonical_solana_sccp_accounts_lt_hash_opened_contributions_bytes(input_value)) == 10_696
    contribution_hash = solana_sccp_accounts_lt_hash_opened_contributions_hash(input_value)
    assert contribution_hash == "0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9"
    assert contribution_hash != solana_sccp_accounts_lt_hash_opened_contributions_hash(
        {**input_value, "validator_vote_account_raw_data": [b"\x01\x02\x04"]}
    )
    with pytest.raises(TypeError, match="accountsLtHashChecksum must match accountsLtHash"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {**input_value, "accounts_lt_hash_checksum": "0x" + "88" * 32}
        )
    with pytest.raises(TypeError, match="finalizedSlot must not use multiple aliases"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {**input_value, "finalizedSlot": input_value["finalized_slot"]}
        )
    with pytest.raises(TypeError, match="accountInclusionRoot must not use multiple aliases"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {**input_value, "accountsRoot": input_value["account_inclusion_root"]}
        )
    with pytest.raises(TypeError, match="accountsLtHashChecksum must not use multiple aliases"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {**input_value, "accountsLtHashRoot": input_value["accounts_lt_hash_checksum"]}
        )
    with pytest.raises(TypeError, match="accountsLtHash must not use multiple aliases"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {**input_value, "accountsLtHash": input_value["accounts_lt_hash"]}
        )
    with pytest.raises(TypeError, match="validatorVoteAccountOpenings must not use multiple aliases"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {
                **input_value,
                "voteAccountOpenings": input_value["validator_vote_account_openings"],
            }
        )
    with pytest.raises(TypeError, match="stakeHistorySysvarOpening must not use multiple aliases"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {
                **input_value,
                "stakeHistorySysvarOpening": input_value["stake_history_sysvar_opening"],
            }
        )
    with pytest.raises(TypeError, match="openedAccountsLtHashResidual must not be zero"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {
                **input_value,
                "accounts_lt_hash": opened_lt_hash,
                "accounts_lt_hash_checksum": solana_sccp_accounts_lt_hash_checksum(
                    opened_lt_hash
                ),
            }
        )
    with pytest.raises(ValueError, match="opened account addresses must be unique"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {
                **input_value,
                "validator_stake_account_openings": [
                    {**stake_opening, "address": vote_opening["address"]}
                ],
            }
        )
    with pytest.raises(ValueError, match="lamports must be greater than zero"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {
                **input_value,
                "validator_vote_account_openings": [
                    {**vote_opening, "lamports": 0}
                ],
            }
        )
    with pytest.raises(ValueError, match="validatorVoteAccountOpenings.*at most"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {
                **input_value,
                "validator_vote_account_openings": [vote_opening]
                * (SCCP_SOLANA_MAX_VALIDATORS + 1),
                "validator_vote_account_raw_data": [vote_raw_data]
                * (SCCP_SOLANA_MAX_VALIDATORS + 1),
            }
        )


def test_builds_solana_accounts_lt_hash_source_state_proof_requests() -> None:
    input_value = sample_solana_accounts_lt_hash_proof_input()
    request = build_solana_sccp_accounts_lt_hash_proof_request(input_value)

    with pytest.raises(TypeError, match="finalizedSlot must not use multiple aliases"):
        build_solana_sccp_accounts_lt_hash_proof_request(
            {**input_value, "finalizedSlot": input_value["finalized_slot"]}
        )
    with pytest.raises(TypeError, match="blockhash must not use multiple aliases"):
        build_solana_sccp_accounts_lt_hash_proof_request(
            {
                **input_value,
                "blockhashBytes": bytes.fromhex(input_value["blockhash"][2:]),
            }
        )
    with pytest.raises(
        TypeError,
        match="sourceStateVerifierHash must not use multiple aliases",
    ):
        build_solana_sccp_accounts_lt_hash_proof_request(
            {
                **input_value,
                "sourceStateVerifierHash": input_value["source_state_verifier_hash"],
            }
        )

    assert_immutable_fastpq_proof_request(
        request,
        (
            "statement_bytes",
            "account_commitment_bytes",
            "verification_context_bytes",
            "schema_descriptor",
        ),
    )
    assert request["version"] == 1
    assert request["proof_family"] == "stark-fri-v1"
    assert request["circuit_id"] == SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1
    assert request["parameter_set"] == "fastpq-lane-balanced"
    assert request["source_state_verifier_id"] == (
        SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1
    )
    assert request["source_state_verifier_hash"] == HEX32_A
    assert request["accounts_lt_hash_proof_public_inputs_hash"] == (
        solana_sccp_accounts_lt_hash_proof_public_inputs_hash(input_value)
    )
    assert request["opened_accounts_lt_hash_contributions_hash"] == (
        solana_sccp_accounts_lt_hash_opened_contributions_hash(input_value)
    )
    vote_lt_hash = solana_sccp_account_lt_hash(
        input_value["validator_vote_account_openings"][0],
        input_value["validator_vote_account_raw_data"][0],
    )
    stake_lt_hash = solana_sccp_account_lt_hash(
        input_value["validator_stake_account_openings"][0],
        input_value["validator_stake_account_raw_data"][0],
    )
    stake_history_lt_hash = solana_sccp_account_lt_hash(
        input_value["stake_history_sysvar_opening"],
        input_value["stake_history_sysvar_raw_data"],
    )
    precomputed_opened_rows_input = {
        **input_value,
        "validator_vote_account_lt_hashes": [vote_lt_hash],
        "validator_stake_account_lt_hashes": [stake_lt_hash],
        "stake_history_sysvar_account_lt_hash": stake_history_lt_hash,
    }
    assert solana_sccp_accounts_lt_hash_opened_contributions_hash(
        precomputed_opened_rows_input
    ) == request["opened_accounts_lt_hash_contributions_hash"]
    wrong_vote_lt_hash = bytearray(vote_lt_hash)
    wrong_vote_lt_hash[0] ^= 1
    with pytest.raises(TypeError, match=r"validatorVoteAccountLtHashes\[0\] must match"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {
                **input_value,
                "validator_vote_account_lt_hashes": [bytes(wrong_vote_lt_hash)],
            }
        )
    with pytest.raises(TypeError, match="stakeHistorySysvarAccountLtHash must match"):
        solana_sccp_accounts_lt_hash_opened_residual_checksum(
            {
                **input_value,
                "stake_history_sysvar_account_lt_hash": bytes(wrong_vote_lt_hash),
            }
        )
    assert request["opened_accounts_lt_hash_residual_checksum"] == (
        solana_sccp_accounts_lt_hash_opened_residual_checksum(input_value)
    )
    assert request["statement_bytes"] == (
        canonical_solana_sccp_accounts_lt_hash_proof_public_inputs_bytes(input_value)
    )
    with pytest.raises(TypeError, match="bankHash must match Agave bank hash inputs"):
        canonical_solana_sccp_accounts_lt_hash_proof_public_inputs_bytes(
            {**input_value, "bank_hash": HEX32_C}
        )
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        canonical_solana_sccp_accounts_lt_hash_proof_public_inputs_bytes(
            {**input_value, "source_domain": SCCP_DOMAIN_SOL, "sourceDomain": SCCP_DOMAIN_SOL}
        )
    with pytest.raises(TypeError, match="finalizedSlot must not use multiple aliases"):
        canonical_solana_sccp_accounts_lt_hash_proof_public_inputs_bytes(
            {**input_value, "finalizedSlot": input_value["finalized_slot"]}
        )
    with pytest.raises(TypeError, match="blockhash must not use multiple aliases"):
        canonical_solana_sccp_accounts_lt_hash_proof_public_inputs_bytes(
            {
                **input_value,
                "blockhashBytes": bytes.fromhex(input_value["blockhash"][2:]),
            }
        )
    with pytest.raises(TypeError, match="accountInclusionRoot must not use multiple aliases"):
        canonical_solana_sccp_accounts_lt_hash_proof_public_inputs_bytes(
            {**input_value, "accountsRoot": input_value["account_inclusion_root"]}
        )
    with pytest.raises(TypeError, match="accountsLtHashChecksum must match accountsLtHash"):
        solana_sccp_accounts_lt_hash_proof_public_inputs_hash(
            {**input_value, "accounts_lt_hash_checksum": HEX32_C}
        )
    assert request["account_commitment_bytes"] == (
        canonical_solana_sccp_accounts_lt_hash_commitment_bytes(input_value)
    )
    assert request["verification_context_bytes"] == (
        canonical_solana_sccp_accounts_lt_hash_verification_context_bytes(input_value)
    )
    assert request["public_input_columns"] == (
        solana_sccp_accounts_lt_hash_public_input_columns(input_value)
    )
    assert request["public_input_columns"][1][0] == SOLANA_MAINNET_GENESIS_PUBLIC_INPUT
    assert request["public_input_columns"][-2][0] == (
        request["opened_accounts_lt_hash_contributions_hash"]
    )
    assert request["public_input_columns"][-1][0] == (
        request["opened_accounts_lt_hash_residual_checksum"]
    )
    assert request["schema_descriptor"] == (
        solana_sccp_accounts_lt_hash_open_verify_schema_descriptor(input_value)
    )
    assert b"opened_accounts_lt_hash_residual_checksum" in request["schema_descriptor"]
    assert b"source_state_verifier_id" in request["schema_descriptor"]
    assert b"mainnet_genesis_hash" in request["schema_descriptor"]
    assert (
        SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1.encode()
        in request["schema_descriptor"]
    )
    assert b"source_state_verifier_hash" in request["schema_descriptor"]
    assert bytes.fromhex(HEX32_A.removeprefix("0x")) in request["schema_descriptor"]
    assert [transition["key"] for transition in request["fastpq_transitions"]] == [
        "sccp:solana:accounts-lt:v1:statement",
        "sccp:solana:accounts-lt:v1:accounts",
        "sccp:solana:accounts-lt:v1:opened-contributions",
        "sccp:solana:accounts-lt:v1:residual",
        "sccp:solana:accounts-lt:v1:context",
    ]
    assert request["fastpq_public_inputs"]["old_root"] == input_value["parent_bank_hash"]
    assert request["fastpq_public_inputs"]["new_root"] == input_value["bank_hash"]
    proof_capsule = wrap_solana_sccp_source_state_verification_proof(
        b"\x01\x02\x03",
        request,
    )
    assert proof_capsule["version"] == 1
    assert proof_capsule["proof_family"] == SCCP_STARK_FRI_PROOF_FAMILY_V1
    assert proof_capsule["circuit_id"] == (
        SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert proof_capsule["proof_bytes"] == b"\x01\x02\x03"
    assert proof_capsule["proof_base64"] == "AQID"
    assert solana_sccp_accounts_lt_hash_proof_hash(proof_capsule).startswith("0x")
    with pytest.raises(TypeError, match="sourceStateProof.proofBase64"):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {**proof_capsule, "proof_base64": "AAAA"}
        )
    with pytest.raises(TypeError, match="immutable"):
        proof_capsule["proof_bytes"] = b"\x04"
    with pytest.raises(TypeError, match="all zero"):
        wrap_solana_sccp_source_state_verification_proof(b"\x00\x00", request)
    oversized_proof_bytes = b"\x01" * (SCCP_SOURCE_STATE_MAX_PROOF_BYTES + 1)
    with pytest.raises(TypeError, match="at most"):
        wrap_solana_sccp_source_state_verification_proof(
            oversized_proof_bytes, request
        )
    with pytest.raises(TypeError, match="at most"):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {**proof_capsule, "proof_bytes": oversized_proof_bytes}
        )
    with pytest.raises(TypeError, match=r"proofFamily.*at most"):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {
                "version": 1,
                "proof_family": "x" * (SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES + 1),
                "circuit_id": request["circuit_id"],
                "proof_bytes": b"\x01",
            }
        )
    with pytest.raises(TypeError, match=r"circuitId.*at most"):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {
                "version": 1,
                "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
                "circuit_id": "x" * (SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES + 1),
                "proof_bytes": b"\x01",
            }
        )
    wrong_genesis_request = dict(request)
    wrong_genesis_request["public_input_columns"] = [
        list(column) for column in request["public_input_columns"]
    ]
    wrong_genesis_request["public_input_columns"][1][0] = HEX32_A
    with pytest.raises(TypeError, match="mainnet_genesis_hash"):
        wrap_solana_sccp_source_state_verification_proof(b"\x01", wrong_genesis_request)
    wrong_residual_column_request = dict(request)
    wrong_residual_column_request["public_input_columns"] = [
        list(column) for column in request["public_input_columns"]
    ]
    wrong_residual_column_request["public_input_columns"][-1][0] = HEX32_C
    with pytest.raises(TypeError, match="opened_accounts_lt_hash_residual_checksum"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", wrong_residual_column_request
        )
    stale_accounts_hash_request = mutable_proof_request(request)
    stale_accounts_hash_request["accounts_lt_hash_proof_public_inputs_hash"] = HEX32_C
    with pytest.raises(
        TypeError,
        match=r"accountsLtHashProofPublicInputsHash must match request\.statementBytes",
    ):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", stale_accounts_hash_request
        )
    wrong_accounts_dsid_request = mutable_proof_request(request)
    wrong_accounts_dsid_request["fastpq_public_inputs"]["dsid"] = (
        "0x" + "00" * 16
    )
    with pytest.raises(TypeError, match=r"fastpqPublicInputs\.dsid"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", wrong_accounts_dsid_request
        )
    wrong_accounts_tx_set_request = mutable_proof_request(request)
    wrong_accounts_tx_set_request["fastpq_public_inputs"]["tx_set_hash"] = HEX32_C
    with pytest.raises(TypeError, match=r"fastpqPublicInputs\.txSetHash"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", wrong_accounts_tx_set_request
        )
    duplicate_source_domain_alias_request = mutable_proof_request(request)
    duplicate_source_domain_alias_request["sourceDomain"] = (
        duplicate_source_domain_alias_request["source_domain"]
    )
    with pytest.raises(TypeError, match=r"request\.sourceDomain.*multiple aliases"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", duplicate_source_domain_alias_request
        )
    duplicate_fastpq_alias_request = mutable_proof_request(request)
    duplicate_fastpq_alias_request["fastpq_public_inputs"]["txSetHash"] = (
        duplicate_fastpq_alias_request["fastpq_public_inputs"]["tx_set_hash"]
    )
    with pytest.raises(
        TypeError,
        match=r"request\.fastpqPublicInputs\.txSetHash.*multiple aliases",
    ):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", duplicate_fastpq_alias_request
        )
    duplicate_transition_alias_request = mutable_proof_request(request)
    duplicate_transition_alias_request["fastpq_transitions"][0]["newValue"] = (
        duplicate_transition_alias_request["fastpq_transitions"][0]["new_value"]
    )
    with pytest.raises(
        TypeError,
        match=r"request\.fastpqTransitions\[0\]\.newValue.*multiple aliases",
    ):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", duplicate_transition_alias_request
        )
    wrong_transition_request = mutable_proof_request(request)
    wrong_transition_request["fastpq_transitions"][0]["new_value"] = "0x00"
    with pytest.raises(TypeError, match="canonical Solana source-state request"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", wrong_transition_request
        )
    wrong_old_value_transition_request = mutable_proof_request(request)
    wrong_old_value_transition_request["fastpq_transitions"][0]["old_value"] = "0x00"
    with pytest.raises(TypeError, match="canonical Solana source-state request"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01", wrong_old_value_transition_request
        )
    with pytest.raises(TypeError, match="OpenVerify circuit"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01",
            {**request, "circuit_id": SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1},
        )
    request_without_source_domain = dict(request)
    del request_without_source_domain["source_domain"]
    with pytest.raises(TypeError, match="sourceDomain is required"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01",
            request_without_source_domain,
        )
    request_without_statement = dict(request)
    del request_without_statement["statement_bytes"]
    with pytest.raises(TypeError, match=r"request\.statementBytes is required"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01",
            request_without_statement,
        )
    with pytest.raises(TypeError, match=r"request\.parameterSet"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01",
            {**request, "parameter_set": "debug"},
        )
    with pytest.raises(TypeError, match="Solana template verifier hash"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01",
            {
                **request,
                "source_state_verifier_hash": (
                    SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1
                ),
            },
        )
    with pytest.raises(
        TypeError,
        match=r"request\.openedAccountsLtHashResidualChecksum must not be zero",
    ):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01",
            {
                **request,
                "opened_accounts_lt_hash_residual_checksum": SCCP_ZERO_HASH_V1,
            },
        )
    with pytest.raises(TypeError, match="direct parent"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x01",
            {**request, "parent_slot": request["finalized_slot"]},
        )

    zero_accounts_lt_hash = bytes(2_048)
    zero_accounts_lt_hash_checksum = solana_sccp_accounts_lt_hash_checksum(
        zero_accounts_lt_hash
    )
    assert zero_accounts_lt_hash_checksum.startswith("0x")
    with pytest.raises(TypeError, match="accountsLtHash must not be zero"):
        solana_sccp_agave_bank_hash(
            {
                "parent_bank_hash": input_value["parent_bank_hash"],
                "bank_signature_count": input_value["bank_signature_count"],
                "blockhash": input_value["blockhash"],
                "accounts_lt_hash": zero_accounts_lt_hash,
            }
        )
    with pytest.raises(TypeError, match="accountsLtHash must not be zero"):
        solana_sccp_accounts_lt_hash_opened_contributions_hash(
            {
                **input_value,
                "accounts_lt_hash": zero_accounts_lt_hash,
                "accounts_lt_hash_checksum": zero_accounts_lt_hash_checksum,
            }
        )

    with pytest.raises(TypeError, match="sourceStateVerifierHash"):
        build_solana_sccp_accounts_lt_hash_proof_request(
            {**input_value, "source_state_verifier_hash": SCCP_ZERO_HASH_V1}
        )
    with pytest.raises(TypeError, match="Solana template verifier hash"):
        build_solana_sccp_accounts_lt_hash_proof_request(
            {
                **input_value,
                "source_state_verifier_hash": (
                    SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1
                ),
            }
        )
    with pytest.raises(TypeError, match="bankHash must match"):
        build_solana_sccp_accounts_lt_hash_proof_request(
            {**input_value, "bank_hash": HEX32_C}
        )
    camel_accounts_lt_hash_input = {
        **input_value,
        "accountsLtHash": input_value["accounts_lt_hash"],
    }
    del camel_accounts_lt_hash_input["accounts_lt_hash"]
    camel_accounts_lt_hash_request = build_solana_sccp_accounts_lt_hash_proof_request(
        camel_accounts_lt_hash_input
    )
    assert (
        camel_accounts_lt_hash_request["opened_accounts_lt_hash_contributions_hash"]
        == request["opened_accounts_lt_hash_contributions_hash"]
    )
    assert (
        camel_accounts_lt_hash_request["opened_accounts_lt_hash_residual_checksum"]
        == request["opened_accounts_lt_hash_residual_checksum"]
    )


def test_builds_solana_full_light_client_audit_role_proof_requests() -> None:
    input_value = sample_solana_full_light_client_audit_proof_input()
    requests = build_solana_sccp_full_light_client_audit_proof_requests(input_value)
    finality_context_hash = solana_sccp_finality_context_hash(input_value)
    accounts_lt_hash_proof_hash = solana_sccp_accounts_lt_hash_proof_hash(
        input_value["accounts_lt_hash_proof"]
    )
    with pytest.raises(TypeError, match="role-separated"):
        build_solana_sccp_tower_replay_proof_request(
            sample_solana_full_light_client_audit_proof_input(
                solana_tower_replay_verifier_hash=requests["tower_replay"][
                    "audit_statement_hash"
                ],
            )
        )
    with pytest.raises(TypeError, match="towerVoteSlots must not use multiple aliases"):
        build_solana_sccp_full_light_client_audit_proof_requests(
            {**input_value, "towerVoteSlots": input_value["tower_vote_slots"]}
        )
    with pytest.raises(
        TypeError,
        match="finalityContextHash must not use multiple aliases",
    ):
        build_solana_sccp_full_light_client_audit_proof_requests(
            {
                **input_value,
                "finalityContextHash": finality_context_hash,
                "finality_context_hash": finality_context_hash,
            }
        )
    with pytest.raises(
        TypeError,
        match="sourceVerifierMaterial must not use multiple aliases",
    ):
        build_solana_sccp_full_light_client_audit_proof_requests(
            {
                **input_value,
                "sourceVerifierMaterial": {},
                "source_verifier_material": {},
            }
        )
    camel_accounts_lt_hash_input = {
        **input_value,
        "accountsLtHash": input_value["accounts_lt_hash"],
    }
    del camel_accounts_lt_hash_input["accounts_lt_hash"]
    camel_accounts_lt_hash_requests = (
        build_solana_sccp_full_light_client_audit_proof_requests(
            camel_accounts_lt_hash_input
        )
    )
    assert (
        camel_accounts_lt_hash_requests["tower_replay"]["audit_statement_hash"]
        == requests["tower_replay"]["audit_statement_hash"]
    )
    assert (
        camel_accounts_lt_hash_requests["tower_replay"]["public_input_columns"]
        == requests["tower_replay"]["public_input_columns"]
    )
    expected_vectors = {
        "tower_replay": {
            "statement_hash": "0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3",
            "statement_len": 777,
            "public_input_columns": [
                ["0x0100000000000000000000000000000000000000000000000000000000000000"],
                ["0x0300000000000000000000000000000000000000000000000000000000000000"],
                [SOLANA_MAINNET_GENESIS_PUBLIC_INPUT],
                ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
                ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
                ["0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3"],
                ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
                ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
                ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
                ["0xb1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1"],
                ["0x0300000000000000000000000000000000000000000000000000000000000000"],
                ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
                ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
                ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
                ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
                ["0x922a426e06d6263986a0c9ff0f956f5429288c9c1310cb67fbaf30918de58b40"],
                ["0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"],
                ["0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"],
                ["0x1313131313131313131313131313131313131313131313131313131313131313"],
                ["0x1414141414141414141414141414141414141414141414141414141414141414"],
                ["0x1515151515151515151515151515151515151515151515151515151515151515"],
                ["0x1616161616161616161616161616161616161616161616161616161616161616"],
                ["0x1717171717171717171717171717171717171717171717171717171717171717"],
                ["0x7777777777777777777777777777777777777777777777777777777777777777"],
            ],
        },
        "full_accountsdb_lattice": {
            "statement_hash": "0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0",
            "statement_len": 440,
            "public_input_columns": [
                ["0x0200000000000000000000000000000000000000000000000000000000000000"],
                ["0x0300000000000000000000000000000000000000000000000000000000000000"],
                [SOLANA_MAINNET_GENESIS_PUBLIC_INPUT],
                ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
                ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
                ["0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0"],
                ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
                ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
                ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
                ["0xc2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2"],
                ["0x0300000000000000000000000000000000000000000000000000000000000000"],
                ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
                ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
                ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
                ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
                ["0x7777777777777777777777777777777777777777777777777777777777777777"],
                ["0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"],
                ["0xc1b7c880344a2551d0842848f68b8519027e8b228a4c92c4e754141821d63810"],
                ["0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9"],
                ["0x336bb79a5e96c331ddca555aedde346438de4ca1b227ae09f7faaa5e0e455be0"],
                ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
            ],
        },
        "bank_fork_choice": {
            "statement_hash": "0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8",
            "statement_len": 509,
            "public_input_columns": [
                ["0x0300000000000000000000000000000000000000000000000000000000000000"],
                ["0x0300000000000000000000000000000000000000000000000000000000000000"],
                [SOLANA_MAINNET_GENESIS_PUBLIC_INPUT],
                ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
                ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
                ["0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8"],
                ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
                ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
                ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
                ["0xd3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3"],
                ["0x0300000000000000000000000000000000000000000000000000000000000000"],
                ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
                ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
                ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
                ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
                ["0xc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0"],
                ["0x46bf9f58208a9c61b931640824eb13d636d3af5b0268cce866c958367bd6a451"],
                ["0x4242424242424242424242424242424242424242424242424242424242424242"],
                ["0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],
                ["0x7777777777777777777777777777777777777777777777777777777777777777"],
                ["0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"],
                ["0x0800000000000000000000000000000000000000000000000000000000000000"],
                ["0x1d2a51ef7c068fe46c9f588c252ce9cea8b66d87453bf73c9920005802e738bc"],
                ["0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"],
                ["0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"],
            ],
        },
    }

    assert canonical_solana_sccp_source_state_verification_proof_bytes(
        input_value["accounts_lt_hash_proof"]
    )
    assert canonical_solana_sccp_finality_context_bytes(input_value)
    witness = normalize_solana_sccp_witness(input_value)
    bank_fork_hash = solana_sccp_bank_fork_hash(input_value)
    direct_finality_context = {
        "version": 1,
        "epoch": input_value["epoch"],
        "rooted_slot": input_value["rooted_slot"],
        "parent_slot": input_value["parent_slot"],
        "tower_vote_slots": input_value["tower_vote_slots"],
        "parent_bank_hash": input_value["parent_bank_hash"],
        "bank_signature_count": input_value["bank_signature_count"],
        "bank_hash_hard_fork_data": witness["bank_hash_hard_fork_data"],
        "epoch_stake_root": input_value["epoch_stake_root"],
        "stake_activation_hash": input_value["stake_activation_hash"],
        "stake_account_state_hash": input_value["stake_account_state_hash"],
        "stake_history_hash": input_value["stake_history_hash"],
        "stake_history_sysvar_account_hash": input_value[
            "stake_history_sysvar_account_hash"
        ],
        "account_inclusion_root": witness["account_inclusion_root"],
        "accounts_lt_hash_checksum": witness["accounts_lt_hash_checksum"],
        "accounts_lt_hash_proof_public_inputs_hash": witness[
            "accounts_lt_hash_proof_public_inputs_hash"
        ],
        "tower_lockout_hash": solana_sccp_tower_lockout_hash(input_value),
        "tower_replay_hash": solana_sccp_tower_replay_hash(
            {**input_value, "bank_fork_hash": bank_fork_hash}
        ),
        "bank_fork_hash": bank_fork_hash,
    }
    assert canonical_solana_sccp_finality_context_bytes(
        direct_finality_context
    ) == canonical_solana_sccp_finality_context_bytes(input_value)
    with pytest.raises(TypeError, match="parentBankHash must not use multiple aliases"):
        canonical_solana_sccp_finality_context_bytes(
            {
                **direct_finality_context,
                "parentBankHash": direct_finality_context["parent_bank_hash"],
            }
        )
    with pytest.raises(TypeError, match="towerVoteSlots must not use multiple aliases"):
        canonical_solana_sccp_finality_context_bytes(
            {
                **direct_finality_context,
                "towerVoteSlots": direct_finality_context["tower_vote_slots"],
            }
        )
    with pytest.raises(TypeError, match="bankForkHash must not use multiple aliases"):
        canonical_solana_sccp_finality_context_bytes(
            {
                **direct_finality_context,
                "bankForkHash": direct_finality_context["bank_fork_hash"],
            }
        )
    assert list(requests.keys()) == [
        "tower_replay",
        "full_accountsdb_lattice",
        "bank_fork_choice",
    ]
    with pytest.raises(TypeError, match="immutable"):
        requests["tower_replay"] = {}
    assert (
        requests["tower_replay"]["circuit_id"]
        == SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert (
        requests["full_accountsdb_lattice"]["circuit_id"]
        == SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert (
        requests["bank_fork_choice"]["circuit_id"]
        == SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert len({request["circuit_id"] for request in requests.values()}) == 3
    for role, request in requests.items():
        assert_immutable_fastpq_proof_request(
            request,
            ("statement_bytes", "verification_context_bytes", "schema_descriptor"),
        )
        assert request["audit_statement_hash"] == expected_vectors[role]["statement_hash"]
        assert len(request["statement_bytes"]) == expected_vectors[role]["statement_len"]
        assert request["public_input_columns"] == expected_vectors[role]["public_input_columns"]
        assert request["version"] == 1
        assert request["proof_family"] == SCCP_STARK_FRI_PROOF_FAMILY_V1
        assert request["parameter_set"] == "fastpq-lane-balanced"
        assert request["finality_context_hash"] == finality_context_hash
        assert request["accounts_lt_hash_proof_hash"] == accounts_lt_hash_proof_hash
        assert request["full_light_client_gate_hash"] == (
            sccp_solana_full_light_client_gate_hash(input_value)
        )
        assert len(request["fastpq_transitions"]) == 3
        assert all(
            transition["key"].startswith("0x")
            for transition in request["fastpq_transitions"]
        )
        assert request["schema_descriptor"] == (
            solana_sccp_full_light_client_audit_open_verify_schema_descriptor(
                input_value,
                request["role"],
            )
        )
        assert request["public_input_columns"] == (
            solana_sccp_full_light_client_audit_public_input_columns(
                input_value,
                request["role"],
            )
        )
        assert request["audit_statement_hash"] == (
            solana_sccp_full_light_client_audit_statement_hash(
                input_value,
                request["role"],
            )
        )
        assert request["statement_bytes"] == (
            canonical_solana_sccp_full_light_client_audit_statement_bytes(
                input_value,
                request["role"],
            )
        )
        if role == "full_accountsdb_lattice":
            assert request["statement_bytes"][-32:] == bytes.fromhex(
                accounts_lt_hash_proof_hash[2:]
            )
            assert request["statement_bytes"][-32:] != bytes.fromhex(
                witness["accounts_lt_hash_proof_public_inputs_hash"][2:]
            )
        proof_capsule = wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            request,
        )
        assert proof_capsule["circuit_id"] == request["circuit_id"]
        assert proof_capsule["proof_family"] == request["proof_family"]
        assert proof_capsule["proof_bytes"] == b"\x09\x08\x07"
        assert proof_capsule["proof_base64"] == "CQgH"
        assert len(canonical_solana_sccp_source_state_verification_proof_bytes(proof_capsule)) > 0
        with pytest.raises(TypeError, match="Solana AccountsLtHash"):
            solana_sccp_accounts_lt_hash_proof_hash(proof_capsule)
    wrong_audit_genesis_request = dict(requests["bank_fork_choice"])
    wrong_audit_genesis_request["public_input_columns"] = [
        list(column) for column in requests["bank_fork_choice"]["public_input_columns"]
    ]
    wrong_audit_genesis_request["public_input_columns"][2][0] = HEX32_A
    with pytest.raises(TypeError, match="mainnet_genesis_hash"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            wrong_audit_genesis_request,
        )
    wrong_audit_statement_column_request = dict(requests["tower_replay"])
    wrong_audit_statement_column_request["public_input_columns"] = [
        list(column) for column in requests["tower_replay"]["public_input_columns"]
    ]
    wrong_audit_statement_column_request["public_input_columns"][5][0] = HEX32_C
    with pytest.raises(TypeError, match="audit_statement_hash"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            wrong_audit_statement_column_request,
        )
    stale_audit_hash_request = mutable_proof_request(requests["tower_replay"])
    stale_audit_hash_request["audit_statement_hash"] = HEX32_C
    with pytest.raises(
        TypeError,
        match=r"auditStatementHash must match request\.statementBytes",
    ):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            stale_audit_hash_request,
        )
    wrong_audit_dsid_request = mutable_proof_request(requests["tower_replay"])
    wrong_audit_dsid_request["fastpq_public_inputs"]["dsid"] = (
        "0x" + "00" * 16
    )
    with pytest.raises(TypeError, match=r"fastpqPublicInputs\.dsid"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            wrong_audit_dsid_request,
        )
    wrong_audit_tx_set_request = mutable_proof_request(requests["tower_replay"])
    wrong_audit_tx_set_request["fastpq_public_inputs"]["tx_set_hash"] = HEX32_C
    with pytest.raises(TypeError, match=r"fastpqPublicInputs\.txSetHash"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            wrong_audit_tx_set_request,
        )
    duplicate_audit_role_alias_request = mutable_proof_request(requests["tower_replay"])
    duplicate_audit_role_alias_request["audit_role"] = (
        duplicate_audit_role_alias_request["role"]
    )
    with pytest.raises(TypeError, match=r"request\.role.*multiple aliases"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            duplicate_audit_role_alias_request,
        )
    duplicate_audit_fastpq_alias_request = mutable_proof_request(requests["tower_replay"])
    duplicate_audit_fastpq_alias_request["fastpq_public_inputs"]["oldRoot"] = (
        duplicate_audit_fastpq_alias_request["fastpq_public_inputs"]["old_root"]
    )
    with pytest.raises(
        TypeError,
        match=r"request\.fastpqPublicInputs\.oldRoot.*multiple aliases",
    ):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            duplicate_audit_fastpq_alias_request,
        )
    wrong_audit_transition_request = mutable_proof_request(requests["tower_replay"])
    wrong_audit_transition_request["fastpq_transitions"][0]["new_value"] = "0x00"
    with pytest.raises(TypeError, match="canonical Solana source-state request"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            wrong_audit_transition_request,
        )
    wrong_audit_old_value_transition_request = mutable_proof_request(
        requests["tower_replay"]
    )
    wrong_audit_old_value_transition_request["fastpq_transitions"][0][
        "old_value"
    ] = "0x00"
    with pytest.raises(TypeError, match="canonical Solana source-state request"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            wrong_audit_old_value_transition_request,
        )
    with pytest.raises(TypeError, match=r"request\.roleCode must match request\.role"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            {**requests["tower_replay"], "role": "bank_fork_choice"},
        )
    with pytest.raises(TypeError, match=r"request\.verifierHash must not be zero"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            {**requests["tower_replay"], "verifier_hash": SCCP_ZERO_HASH_V1},
        )
    reused_source_state_verifier_request = mutable_proof_request(
        requests["tower_replay"]
    )
    reused_source_state_verifier_request["verifier_hash"] = (
        reused_source_state_verifier_request["source_state_verifier_hash"]
    )
    with pytest.raises(TypeError, match="role-separated"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            reused_source_state_verifier_request,
        )
    reused_audit_statement_request = mutable_proof_request(requests["tower_replay"])
    reused_audit_statement_request["verifier_hash"] = (
        reused_audit_statement_request["audit_statement_hash"]
    )
    with pytest.raises(TypeError, match="role-separated"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            reused_audit_statement_request,
        )
    with pytest.raises(TypeError, match="Solana template verifier hash"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            {
                **requests["tower_replay"],
                "source_state_verifier_hash": (
                    SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1
                ),
            },
        )
    with pytest.raises(TypeError, match=r"request\.parameterSet"):
        wrap_solana_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            {**requests["tower_replay"], "parameter_set": "debug"},
        )
    assert requests["tower_replay"]["vote_message_hash"] == solana_sccp_vote_message_hash(
        {
            "source_domain": SCCP_DOMAIN_SOL,
            "finalized_slot": input_value["finalized_slot"],
            "blockhash": input_value["blockhash"],
            "bank_hash": input_value["bank_hash"],
            "transaction_status_root": input_value["transaction_status_root"],
            "message_proof_hash": input_value["message_proof_hash"],
            "finality_context_hash": finality_context_hash,
        }
    )
    assert (
        requests["full_accountsdb_lattice"]["public_input_columns"][-1][0]
        == accounts_lt_hash_proof_hash
    )
    assert requests["bank_fork_choice"]["public_input_columns"][19] == [
        input_value["account_inclusion_root"]
    ]
    assert b"mainnet_genesis_hash" in requests["tower_replay"]["schema_descriptor"]
    assert b"full_light_client_gate_hash" in requests["tower_replay"]["schema_descriptor"]
    assert requests["tower_replay"]["public_input_columns"][20] == [
        input_value["stake_account_state_hash"]
    ]
    assert requests["tower_replay"]["public_input_columns"][22] == [
        input_value["stake_history_sysvar_account_hash"]
    ]
    assert requests["tower_replay"]["public_input_columns"][23] == [
        input_value["account_inclusion_root"]
    ]
    assert b"stake_account_state_hash" in requests["tower_replay"]["schema_descriptor"]
    assert (
        b"stake_history_sysvar_account_hash"
        in requests["tower_replay"]["schema_descriptor"]
    )
    assert b"account_inclusion_root" in requests["tower_replay"]["schema_descriptor"]
    assert (
        b"account_inclusion_root"
        in requests["bank_fork_choice"]["schema_descriptor"]
    )
    assert (
        b"bank_hash_hard_fork_data_hash"
        in requests["bank_fork_choice"]["schema_descriptor"]
    )
    assert (
        requests["tower_replay"]["audit_statement_hash"]
        != requests["bank_fork_choice"]["audit_statement_hash"]
    )
    without_proof = dict(input_value)
    without_proof.pop("accounts_lt_hash_proof")
    with pytest.raises(TypeError, match="accountsLtHashProofHash"):
        build_solana_sccp_tower_replay_proof_request(without_proof)
    proof_hash_only = dict(without_proof)
    proof_hash_only["accounts_lt_hash_proof_hash"] = accounts_lt_hash_proof_hash
    with pytest.raises(TypeError, match="accountsLtHashProof is required"):
        build_solana_sccp_tower_replay_proof_request(proof_hash_only)
    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        solana_sccp_accounts_lt_hash_proof_hash(
            {
                **input_value["accounts_lt_hash_proof"],
                "proof_bytes": b"\0\0\0",
            }
        )
    with pytest.raises(TypeError, match="accountsLtHashProof.proofBase64"):
        solana_sccp_accounts_lt_hash_proof_hash(
            {**input_value["accounts_lt_hash_proof"], "proofBase64": "AAAA"}
        )
    with pytest.raises(
        TypeError,
        match=r"sourceStateProof\.proofBase64 must not use multiple aliases",
    ):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {
                **input_value["accounts_lt_hash_proof"],
                "proof_base64": base64.b64encode(
                    input_value["accounts_lt_hash_proof"]["proof_bytes"]
                ).decode("ascii"),
                "proofBase64": base64.b64encode(
                    input_value["accounts_lt_hash_proof"]["proof_bytes"]
                ).decode("ascii"),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"sourceStateProof\.circuitId must not use multiple aliases",
    ):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {
                **input_value["accounts_lt_hash_proof"],
                "circuitId": input_value["accounts_lt_hash_proof"]["circuit_id"],
            }
        )
    with pytest.raises(
        TypeError,
        match=r"sourceStateProof\.proofBytes must not use multiple aliases",
    ):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {
                **input_value["accounts_lt_hash_proof"],
                "proofBytes": input_value["accounts_lt_hash_proof"]["proof_bytes"],
            }
        )
    with pytest.raises(ValueError, match="accountsLtHashProof.version"):
        solana_sccp_accounts_lt_hash_proof_hash(
            {**input_value["accounts_lt_hash_proof"], "version": 0}
        )
    with pytest.raises(TypeError, match="sourceStateProof.version"):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {**input_value["accounts_lt_hash_proof"], "version": None}
        )
    with pytest.raises(TypeError, match="sourceStateProof.proofFamily"):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {**input_value["accounts_lt_hash_proof"], "proof_family": None}
        )
    with pytest.raises(TypeError, match="Solana source-state"):
        canonical_solana_sccp_source_state_verification_proof_bytes(
            {
                **input_value["accounts_lt_hash_proof"],
                "circuit_id": SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
            }
        )
    with pytest.raises(TypeError, match="accountsLtHashProof.proofFamily"):
        solana_sccp_accounts_lt_hash_proof_hash(
            {**input_value["accounts_lt_hash_proof"], "proof_family": ""}
        )
    with pytest.raises(TypeError, match="accountsLtHashProofHash must match"):
        build_solana_sccp_tower_replay_proof_request(
            {**input_value, "accounts_lt_hash_proof_hash": HEX32_A}
        )
    with pytest.raises(TypeError, match="sourceVerifierMaterialHash must match"):
        build_solana_sccp_tower_replay_proof_request(
            {**input_value, "source_verifier_material_hash": HEX32_A}
        )
    with pytest.raises(TypeError, match="sourceVerifierMaterialHash"):
        build_solana_sccp_tower_replay_proof_request(
            {**input_value, "source_verifier_material_hash": None}
        )
    with pytest.raises(TypeError, match="sourceAdapterDeploymentHash must match"):
        build_solana_sccp_tower_replay_proof_request(
            {**input_value, "source_adapter_deployment_hash": HEX32_B}
        )
    missing_witness_deployment_hash = {
        **input_value,
        "source_adapter_deployment": dict(input_value),
    }
    missing_witness_deployment_hash.pop("source_adapter_deployment_hash")
    missing_witness_deployment_hash.pop("source_adapter_deployment_receipt_hash")
    with pytest.raises(TypeError, match="sourceAdapterDeploymentHash must match witness"):
        build_solana_sccp_tower_replay_proof_request(missing_witness_deployment_hash)
    with pytest.raises(TypeError, match="fullLightClientGateHash must match"):
        build_solana_sccp_tower_replay_proof_request(
            {**input_value, "full_light_client_gate_hash": HEX32_B}
        )
    with pytest.raises(
        TypeError,
        match="sourceAdapterDeploymentReceiptHash must match witness",
    ):
        build_solana_sccp_tower_replay_proof_request(
            {
                **input_value,
                "source_adapter_deployment": dict(input_value),
                "source_adapter_deployment_receipt_hash": HEX32_B,
            }
        )
    with pytest.raises(ValueError, match="role-separated"):
        build_solana_sccp_tower_replay_proof_request(
            {
                **input_value,
                "solana_tower_replay_verifier_hash": input_value[
                    "solana_full_accountsdb_lattice_verifier_hash"
                ],
            }
        )
    with pytest.raises(ValueError, match="must not reuse"):
        build_solana_sccp_tower_replay_proof_request(
            {
                **input_value,
                "solana_tower_replay_verifier_hash": input_value[
                    "source_state_verifier_hash"
                ],
            }
        )
    with pytest.raises(ValueError, match="template material"):
        build_solana_sccp_tower_replay_proof_request(
            {
                **input_value,
                "solana_tower_replay_verifier_hash": (
                    SOLANA_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES[
                        "source_trust_anchor_hash"
                    ]
                ),
            }
        )


def test_solana_source_state_prover_wraps_linked_accounts_lt_hash_proofs() -> None:
    seen = []

    async def prove(request: Mapping[str, Any], options: Mapping[str, Any]) -> Mapping[str, Any]:
        seen.append((request, options))
        assert request["circuit_id"] == SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1
        return {
            "proof_bytes": b"\x01\x02\x03",
            "version": 1,
            "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
            "circuit_id": request["circuit_id"],
            "proof_base64": "AQID",
            "parameter_set": request["parameter_set"],
            "source_domain": str(request["source_domain"]),
            "finalized_slot": int(request["finalized_slot"]),
            "source_state_verifier_id": request["source_state_verifier_id"],
            "source_state_verifier_hash": request[
                "source_state_verifier_hash"
            ].upper(),
            "accounts_lt_hash_proof_public_inputs_hash": request[
                "accounts_lt_hash_proof_public_inputs_hash"
            ].upper(),
            "opened_accounts_lt_hash_contributions_hash": request[
                "opened_accounts_lt_hash_contributions_hash"
            ].upper(),
            "opened_accounts_lt_hash_residual_checksum": request[
                "opened_accounts_lt_hash_residual_checksum"
            ].upper(),
            "public_input_columns": request["public_input_columns"],
            "fastpq_public_inputs": request["fastpq_public_inputs"],
            "fastpq_transitions": request["fastpq_transitions"],
            "statement_bytes": request["statement_bytes"],
            "account_commitment_bytes": request["account_commitment_bytes"],
            "verification_context_bytes": request["verification_context_bytes"],
            "schema_descriptor": request["schema_descriptor"],
        }

    proof = asyncio.run(
        SolanaSccpSourceStateProver(prove=prove).prove_accounts_lt_hash(
            sample_solana_accounts_lt_hash_proof_input(),
            source="ui",
        )
    )

    assert len(seen) == 1
    assert seen[0][1]["source"] == "ui"
    assert proof["circuit_id"] == SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1
    assert proof["proof_bytes"] == b"\x01\x02\x03"
    assert proof["proof_base64"] == "AQID"

    def prove_fastpq_aliases(
        request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Mapping[str, Any]:
        def upper_hex(value: str) -> str:
            return value if value == "0x" else value.upper()

        fastpq = request["fastpq_public_inputs"]
        return {
            "proof_bytes": b"\x07\x08\x09",
            "fastpqPublicInputs": {
                "dsid": fastpq["dsid"].upper(),
                "slot": int(fastpq["slot"]),
                "oldRoot": fastpq["old_root"].upper(),
                "newRoot": fastpq["new_root"].upper(),
                "permRoot": fastpq["perm_root"].upper(),
                "txSetHash": fastpq["tx_set_hash"].upper(),
            },
            "fastpqTransitions": [
                {
                    "key": transition["key"],
                    "operation": transition["operation"],
                    "oldValue": upper_hex(transition["old_value"]),
                    "newValue": upper_hex(transition["new_value"]),
                }
                for transition in request["fastpq_transitions"]
            ],
        }

    fastpq_alias_proof = asyncio.run(
        SolanaSccpSourceStateProver(
            prove=prove_fastpq_aliases,
        ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
    )
    assert fastpq_alias_proof["proof_bytes"] == b"\x07\x08\x09"

    oversized_proof_bytes = b"\x01" * (SCCP_SOURCE_STATE_MAX_PROOF_BYTES + 1)
    with pytest.raises(TypeError, match="at most"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda _request, _options: {
                    "proof_bytes": oversized_proof_bytes,
                    "proof_base64": "AQID",
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.proofBase64 must match proofBytes",
    ):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda _request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "proof_base64": " AQID ",
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(TypeError, match=r"source-state prover result\.proofBytes"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda _request, _options: {
                    "proofBytes": b"\x01\x02\x03",
                    "proof_bytes": b"\x01\x02\x03",
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(TypeError, match=r"source-state prover result\.proofBase64"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda _request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "proofBase64": "AQID",
                    "proof_base64": "AQID",
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(TypeError, match=r"source-state prover result\.version"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda _request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "proofVersion": 0,
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(TypeError, match=r"source-state prover result\.version"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda _request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "version": 1,
                    "proof_version": 1,
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(TypeError, match=r"source-state prover result\.proofFamily"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda _request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "proof_family": f" {SCCP_STARK_FRI_PROOF_FAMILY_V1} ",
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(TypeError, match=r"source-state prover result\.circuitId"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "circuit_id": f" {request['circuit_id']} ",
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(TypeError, match=r"source-state prover result\.parameterSet"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "parameter_set": f" {request['parameter_set']} ",
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(SolanaSccpSourceStateProverUnavailableError) as exc:
        asyncio.run(SolanaSccpSourceStateProver().prove_request(seen[0][0]))
    assert exc.value.code == "ERR_SCCP_SOLANA_SOURCE_STATE_PROVER_UNAVAILABLE"
    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.sourceStateVerifierHash must match request\.sourceStateVerifierHash",
    ):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "source_state_verifier_hash": HEX32_C,
                    "public_input_columns": request["public_input_columns"],
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.statementBytes must match request\.statementBytes",
    ):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "statement_bytes": bytes([request["statement_bytes"][0] ^ 0xFF]),
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )

    def prove_padded_columns(
        request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Mapping[str, Any]:
        columns = [list(row) for row in request["public_input_columns"]]
        columns[0][0] = f" {columns[0][0]} "
        return {
            "proof_bytes": b"\x01\x02\x03",
            "public_input_columns": columns,
        }

    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.publicInputColumns must match request\.publicInputColumns",
    ):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=prove_padded_columns
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )
    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.publicInputColumns must match request\.publicInputColumns",
    ):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "public_input_columns": [
                        [HEX32_C],
                        *request["public_input_columns"][1:],
                    ],
                }
            ).prove_accounts_lt_hash(sample_solana_accounts_lt_hash_proof_input())
        )


def test_solana_source_state_prover_snapshots_mutable_callback_requests() -> None:
    built_request = build_solana_sccp_accounts_lt_hash_proof_request(
        sample_solana_accounts_lt_hash_proof_input()
    )
    mutable_request = mutable_proof_request(built_request)

    def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> Mapping[str, Any]:
        assert request is not mutable_request
        with pytest.raises(TypeError, match="immutable"):
            request["statement_bytes"] = b""  # type: ignore[index]
        mutable_request["statement_bytes"] = b""
        mutable_request["circuit_id"] = SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1
        return b"\x04\x05\x06"

    proof = asyncio.run(SolanaSccpSourceStateProver(prove=prove).prove_request(mutable_request))

    assert proof["circuit_id"] == SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1
    assert proof["proof_base64"] == "BAUG"


def test_solana_source_state_prover_wraps_full_light_audit_role_proofs() -> None:
    roles = []
    camel_role = {
        "tower_replay": "towerReplay",
        "full_accountsdb_lattice": "fullAccountsdbLattice",
        "bank_fork_choice": "bankForkChoice",
    }

    def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> bytes:
        roles.append(request["role"])
        return {
            "proof_bytes": b"\x09\x08\x07",
            "version": 1,
            "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
            "circuit_id": request["circuit_id"],
            "parameter_set": request["parameter_set"],
            "role": camel_role[request["role"]],
            "role_code": int(request["role_code"]),
            "source_domain": str(request["source_domain"]),
            "finalized_slot": int(request["finalized_slot"]),
            "verifier_id": request["verifier_id"],
            "verifier_hash": request["verifier_hash"].upper(),
            "source_state_verifier_id": request["source_state_verifier_id"],
            "source_state_verifier_hash": request[
                "source_state_verifier_hash"
            ].upper(),
            "source_verifier_material_hash": request[
                "source_verifier_material_hash"
            ].upper(),
            "source_adapter_deployment_hash": request[
                "source_adapter_deployment_hash"
            ].upper(),
            "full_light_client_gate_hash": request["full_light_client_gate_hash"].upper(),
            "finality_context_hash": request["finality_context_hash"].upper(),
            "vote_message_hash": request["vote_message_hash"].upper(),
            "accounts_lt_hash_proof_hash": request["accounts_lt_hash_proof_hash"].upper(),
            "audit_statement_hash": request["audit_statement_hash"].upper(),
            "public_input_columns": request["public_input_columns"],
            "fastpq_public_inputs": request["fastpq_public_inputs"],
            "fastpq_transitions": request["fastpq_transitions"],
            "statement_bytes": request["statement_bytes"],
            "verification_context_bytes": request["verification_context_bytes"],
            "schema_descriptor": request["schema_descriptor"],
        }

    proofs = asyncio.run(
        SolanaSccpSourceStateProver(prove=prove).prove_full_light_client_audit(
            sample_solana_full_light_client_audit_proof_input()
        )
    )

    assert list(proofs.keys()) == [
        "tower_replay",
        "full_accountsdb_lattice",
        "bank_fork_choice",
    ]
    assert roles == ["tower_replay", "full_accountsdb_lattice", "bank_fork_choice"]
    assert proofs["tower_replay"]["circuit_id"] == (
        SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert proofs["full_accountsdb_lattice"]["circuit_id"] == (
        SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert proofs["bank_fork_choice"]["circuit_id"] == (
        SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert proofs["bank_fork_choice"]["proof_base64"] == "CQgH"

    with pytest.raises(TypeError, match="all zero"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda _request, _options: b"\x00\x00"
            ).prove_full_light_client_audit(
                sample_solana_full_light_client_audit_proof_input()
            )
        )
    with pytest.raises(TypeError, match=r"result\.circuitId"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "circuit_id": (
                        SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1
                        if request["role"] == "tower_replay"
                        else request["circuit_id"]
                    ),
                }
            ).prove_full_light_client_audit(
                sample_solana_full_light_client_audit_proof_input()
            )
        )
    with pytest.raises(TypeError, match=r"result\.proofBase64"):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "circuit_id": request["circuit_id"],
                    "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
                    "proof_base64": "AAAA",
                    "version": 1,
                }
            ).prove_full_light_client_audit(
                sample_solana_full_light_client_audit_proof_input()
            )
        )
    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.role must match request\.role",
    ):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "role": f" {request['role']} ",
                }
            ).prove_full_light_client_audit(
                sample_solana_full_light_client_audit_proof_input()
            )
        )
    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.role must match request\.role",
    ):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "role": (
                        "bank_fork_choice"
                        if request["role"] == "tower_replay"
                        else request["role"]
                    ),
                }
            ).prove_full_light_client_audit(
                sample_solana_full_light_client_audit_proof_input()
            )
        )
    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.verifierHash must match request\.verifierHash",
    ):
        asyncio.run(
            SolanaSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "verifier_hash": (
                        HEX32_A if request["role"] == "tower_replay" else request["verifier_hash"]
                    ),
                }
            ).prove_full_light_client_audit(
                sample_solana_full_light_client_audit_proof_input()
            )
        )


def test_derives_solana_vote_and_stake_account_data_hashes() -> None:
    tower_vote_slots = list(range(11, 42))
    vote_input = {
        "node_pubkey": "0x" + "51" * 32,
        "authorized_voter": "0x" + "61" * 32,
        "authorized_withdrawer": "0x" + "71" * 32,
        "inflation_rewards_collector": "0x" + "81" * 32,
        "block_revenue_collector": "0x" + "51" * 32,
        "inflation_rewards_commission_bps": 700,
        "block_revenue_commission_bps": 10_000,
        "pending_delegator_rewards": 123,
        "bls_pubkey_compressed": b"",
        "root_slot": 10,
        "tower_vote_slots": tower_vote_slots,
    }
    assert len(canonical_solana_sccp_vote_account_data_bytes(vote_input)) == 457
    vote_hash = solana_sccp_vote_account_data_hash(vote_input)
    assert vote_hash.startswith("0x") and len(vote_hash) == 66
    assert vote_hash != solana_sccp_vote_account_data_hash(
        {**vote_input, "authorized_voter": "0x" + "62" * 32}
    )
    assert vote_hash != solana_sccp_vote_account_data_hash(
        {**vote_input, "inflation_rewards_commission_bps": 701}
    )
    with pytest.raises(TypeError, match="nodePubkey must not use multiple aliases"):
        solana_sccp_vote_account_data_hash(
            {**vote_input, "nodePubkey": vote_input["node_pubkey"]}
        )
    with pytest.raises(
        TypeError,
        match="inflationRewardsCommissionBps must not use multiple aliases",
    ):
        solana_sccp_vote_account_data_hash(
            {
                **vote_input,
                "inflationRewardsCommissionBps": vote_input[
                    "inflation_rewards_commission_bps"
                ],
            }
        )
    with pytest.raises(TypeError, match="towerVoteSlots must not use multiple aliases"):
        solana_sccp_vote_account_data_hash(
            {**vote_input, "towerVoteSlots": tower_vote_slots}
        )
    with pytest.raises(ValueError, match=r"towerVoteSlots\[0\]"):
        solana_sccp_vote_account_data_hash(
            {**vote_input, "tower_vote_slots": [10, *tower_vote_slots[1:]]}
        )

    stake_input = {
        "staker": "0x" + "81" * 32,
        "withdrawer": "0x" + "91" * 32,
        "voter_pubkey": "0x" + "a1" * 32,
        "delegated_stake": 1_000,
        "activation_epoch": 2,
        "deactivation_epoch": 9,
        "warmup_cooldown_rate_bytes": bytes(
            [0x0A, 0xD7, 0xA3, 0x70, 0x3D, 0x0A, 0xB7, 0x3F]
        ),
        "credits_observed": 123,
        "stake_flags": 1,
    }
    assert len(canonical_solana_sccp_stake_account_data_bytes(stake_input)) == 154
    stake_hash = solana_sccp_stake_account_data_hash(stake_input)
    assert stake_hash.startswith("0x") and len(stake_hash) == 66
    assert stake_hash != solana_sccp_stake_account_data_hash(
        {**stake_input, "voter_pubkey": "0x" + "a2" * 32}
    )
    assert solana_sccp_stake_account_data_hash(
        {
            **stake_input,
            "warmup_cooldown_rate_bytes": bytes(
                [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD0, 0x3F]
            ),
        }
    )
    with pytest.raises(TypeError, match="warmupCooldownRateBytes"):
        solana_sccp_stake_account_data_hash(
            {**stake_input, "warmup_cooldown_rate_bytes": bytes(8)}
        )
    assert stake_hash != solana_sccp_stake_account_data_hash(
        {**stake_input, "stake_flags": 0}
    )
    with pytest.raises(TypeError, match="voterPubkey must not use multiple aliases"):
        solana_sccp_stake_account_data_hash(
            {**stake_input, "voterPubkey": stake_input["voter_pubkey"]}
        )
    with pytest.raises(TypeError, match="delegatedStake must not use multiple aliases"):
        solana_sccp_stake_account_data_hash(
            {**stake_input, "delegatedStake": stake_input["delegated_stake"]}
        )
    with pytest.raises(
        TypeError,
        match="warmupCooldownRateBytes must not use multiple aliases",
    ):
        solana_sccp_stake_account_data_hash(
            {
                **stake_input,
                "warmupCooldownRateBytes": stake_input[
                    "warmup_cooldown_rate_bytes"
                ],
            }
        )
    with pytest.raises(ValueError, match="deactivationEpoch"):
        solana_sccp_stake_account_data_hash(
            {**stake_input, "deactivation_epoch": 2}
        )
    with pytest.raises(ValueError, match="stakeFlags"):
        solana_sccp_stake_account_data_hash({**stake_input, "stake_flags": 2})
    with pytest.raises(TypeError, match="warmupCooldownRateBytes"):
        solana_sccp_stake_account_data_hash(
            {**stake_input, "warmup_cooldown_rate_bytes": bytes(7)}
        )


def test_derives_solana_vote_account_data_hash_from_raw_vote_state() -> None:
    vote_account_address = "0x" + "81" * 32
    raw_v3 = sample_solana_vote_state_account(True)
    parsed = solana_sccp_vote_account_data_from_raw_vote_state(
        raw_v3, 3, vote_account_address
    )
    assert parsed["nodePubkey"] == bytes([0x51]) * 32
    assert parsed["authorizedVoter"] == bytes([0x61]) * 32
    assert parsed["authorizedWithdrawer"] == bytes([0x71]) * 32
    assert parsed["inflationRewardsCollector"] == bytes([0x81]) * 32
    assert parsed["blockRevenueCollector"] == bytes([0x51]) * 32
    assert parsed["inflationRewardsCommissionBps"] == 700
    assert parsed["blockRevenueCommissionBps"] == 10_000
    assert parsed["pendingDelegatorRewards"] == 0
    assert parsed["blsPubkeyCompressed"] == b""
    assert parsed["rootSlot"] == 10
    assert parsed["towerVoteSlots"] == list(range(11, 42))
    assert (
        solana_sccp_vote_account_data_hash_from_raw_vote_state(
            raw_v3, 3, vote_account_address
        )
        == solana_sccp_vote_account_data_hash(parsed)
    )
    assert (
        solana_sccp_vote_account_data_hash_from_raw_vote_state_v1_or_v3(
            raw_v3, 3, vote_account_address
        )
        == solana_sccp_vote_account_data_hash(parsed)
    )

    raw_v1 = sample_solana_vote_state_account(False)
    assert (
        solana_sccp_vote_account_data_from_raw_vote_state(
            raw_v1, 3, vote_account_address
        )["towerVoteSlots"]
        == parsed["towerVoteSlots"]
    )

    raw_v4 = sample_solana_vote_state_v4_account(True)
    parsed_v4 = solana_sccp_vote_account_data_from_raw_vote_state(
        raw_v4, 3, vote_account_address
    )
    assert parsed_v4["inflationRewardsCollector"] == bytes([0x81]) * 32
    assert parsed_v4["blockRevenueCollector"] == bytes([0x91]) * 32
    assert parsed_v4["inflationRewardsCommissionBps"] == 1_234
    assert parsed_v4["blockRevenueCommissionBps"] == 9_876
    assert parsed_v4["pendingDelegatorRewards"] == 456
    assert parsed_v4["blsPubkeyCompressed"] == bytes([0xA5]) * 48
    v4_inflation_commission_bps_offset = 4 + (4 * 32)
    excessive_inflation_commission_v4 = bytearray(raw_v4)
    excessive_inflation_commission_v4[
        v4_inflation_commission_bps_offset : v4_inflation_commission_bps_offset + 2
    ] = (10_001).to_bytes(2, "little")
    with pytest.raises(ValueError, match="inflationRewardsCommissionBps"):
        solana_sccp_vote_account_data_from_raw_vote_state(
        bytes(excessive_inflation_commission_v4), 3, vote_account_address
    )
    excessive_block_commission_v4 = bytearray(raw_v4)
    v4_block_commission_bps_offset = v4_inflation_commission_bps_offset + 2
    excessive_block_commission_v4[
        v4_block_commission_bps_offset : v4_block_commission_bps_offset + 2
    ] = (10_001).to_bytes(2, "little")
    with pytest.raises(ValueError, match="blockRevenueCommissionBps"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(excessive_block_commission_v4), 3, vote_account_address
        )
    with pytest.raises(TypeError, match="blsPubkeyCompressed"):
        solana_sccp_vote_account_data_hash(
            {**parsed_v4, "blsPubkeyCompressed": bytes(48)}
        )
    all_zero_bls_v4 = bytearray(sample_solana_vote_state_v4_account(True))
    v4_bls_pubkey_offset = 4 + (4 * 32) + 2 + 2 + 8 + 1
    all_zero_bls_v4[v4_bls_pubkey_offset : v4_bls_pubkey_offset + 48] = bytes(48)
    with pytest.raises(TypeError, match="blsPubkeyCompressed"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(all_zero_bls_v4), 3, vote_account_address
        )
    parsed_v4_four_authorized = solana_sccp_vote_account_data_from_raw_vote_state(
        sample_solana_vote_state_v4_account(True, 4), 3, vote_account_address
    )
    assert parsed_v4_four_authorized["authorizedVoter"] == bytes([0x62]) * 32

    wrong_vote_count = bytearray(raw_v3)
    wrong_vote_count[(4 + 32 + 32 + 1) : (4 + 32 + 32 + 1 + 8)] = (
        30
    ).to_bytes(8, "little")
    with pytest.raises(ValueError, match="31 active post-root slots"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(wrong_vote_count), 3, vote_account_address
        )

    vote_entry_offset = 4 + 32 + 32 + 1 + 8
    first_vote_slot_offset = vote_entry_offset + 1
    first_confirmation_offset = first_vote_slot_offset + 8
    second_vote_slot_offset = vote_entry_offset + (1 + 8 + 4) + 1
    root_option_offset = vote_entry_offset + (31 * (1 + 8 + 4))

    wrong_confirmation_count = bytearray(raw_v3)
    wrong_confirmation_count[
        first_confirmation_offset : first_confirmation_offset + 4
    ] = (30).to_bytes(4, "little")
    with pytest.raises(ValueError, match="invalid Tower confirmation count"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(wrong_confirmation_count), 3, vote_account_address
        )

    repeated_vote_slot = bytearray(raw_v3)
    repeated_vote_slot[second_vote_slot_offset : second_vote_slot_offset + 8] = (
        11
    ).to_bytes(8, "little")
    with pytest.raises(ValueError, match="greater than the previous slot"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(repeated_vote_slot), 3, vote_account_address
        )

    no_root = bytearray(raw_v3)
    no_root[root_option_offset] = 0
    with pytest.raises(TypeError, match="rooted vote state"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(no_root), 3, vote_account_address
        )

    root_overlaps_vote_stack = bytearray(raw_v3)
    root_overlaps_vote_stack[root_option_offset + 1 : root_option_offset + 9] = (
        11
    ).to_bytes(8, "little")
    with pytest.raises(ValueError, match="greater than the previous slot"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(root_overlaps_vote_stack), 3, vote_account_address
        )

    bad_prior_voters = bytearray(raw_v3)
    prior_voters_offset = root_option_offset + 1 + 8 + 8 + (2 * (8 + 32))
    zero_prior_voter_with_epoch_bounds = bytearray(raw_v3)
    zero_prior_voter_with_epoch_bounds[
        prior_voters_offset + 32 : prior_voters_offset + 40
    ] = (1).to_bytes(8, "little")
    with pytest.raises(TypeError, match=r"priorVoters\[0\]"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(zero_prior_voter_with_epoch_bounds), 3, vote_account_address
        )
    bad_prior_voters[prior_voters_offset + (32 * (32 + 8 + 8)) + 8] = 2
    with pytest.raises(ValueError, match="priorVoters"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(bad_prior_voters), 3, vote_account_address
        )

    v4_authorized_voters_offset = (
        4 + 32 + 32 + 32 + 32 + 2 + 2 + 8 + 1 + 48 + 8
        + (31 * (1 + 8 + 4))
        + 1
        + 8
    )
    zero_future_authorized_voter = bytearray(
        sample_solana_vote_state_v4_account(True, 4)
    )
    fourth_authorized_voter_key_offset = (
        v4_authorized_voters_offset + 8 + (3 * (8 + 32)) + 8
    )
    zero_future_authorized_voter[
        fourth_authorized_voter_key_offset : fourth_authorized_voter_key_offset + 32
    ] = bytes(32)
    with pytest.raises(TypeError, match=r"authorizedVoters\[3\]\.authorizedVoter"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(zero_future_authorized_voter), 3, vote_account_address
        )
    too_many_v4_authorized_voters = sample_solana_vote_state_v4_account(True, 5)
    with pytest.raises(ValueError, match=r"1\.\.4 entries for VoteStateV4"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            too_many_v4_authorized_voters, 3, vote_account_address
        )

    too_many_epoch_credits = bytearray(raw_v4)
    v4_epoch_credits_offset = v4_authorized_voters_offset + 8 + (2 * (8 + 32))
    too_many_epoch_credits[v4_epoch_credits_offset : v4_epoch_credits_offset + 8] = (
        65
    ).to_bytes(8, "little")
    with pytest.raises(ValueError, match="epochCredits"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(too_many_epoch_credits), 3, vote_account_address
        )

    v3_epoch_credits_offset = prior_voters_offset + (32 * (32 + 8 + 8)) + 8 + 1
    future_epoch_credit = bytearray(raw_v3)
    future_epoch_credit[v3_epoch_credits_offset : v3_epoch_credits_offset + 8] = (
        1
    ).to_bytes(8, "little")
    future_epoch_credit[
        v3_epoch_credits_offset + 8 : v3_epoch_credits_offset + 16
    ] = (4).to_bytes(8, "little")
    future_epoch_credit[
        v3_epoch_credits_offset + 16 : v3_epoch_credits_offset + 24
    ] = (1).to_bytes(8, "little")
    with pytest.raises(ValueError, match="epochCredits"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(future_epoch_credit), 3, vote_account_address
        )

    last_timestamp_slot_offset = v3_epoch_credits_offset + 8
    future_last_timestamp_slot = bytearray(raw_v3)
    future_last_timestamp_slot[
        last_timestamp_slot_offset : last_timestamp_slot_offset + 8
    ] = (42).to_bytes(8, "little")
    with pytest.raises(ValueError, match="lastTimestamp"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(future_last_timestamp_slot), 3, vote_account_address
        )

    negative_last_timestamp = bytearray(raw_v3)
    negative_last_timestamp[
        last_timestamp_slot_offset : last_timestamp_slot_offset + 8
    ] = (41).to_bytes(8, "little")
    negative_last_timestamp[
        last_timestamp_slot_offset + 8 : last_timestamp_slot_offset + 16
    ] = (-1).to_bytes(8, "little", signed=True)
    with pytest.raises(ValueError, match="lastTimestamp"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(negative_last_timestamp), 3, vote_account_address
        )

    nonzero_padding = bytearray(raw_v3)
    nonzero_padding[-1] = 1
    with pytest.raises(ValueError, match="padding"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            bytes(nonzero_padding), 3, vote_account_address
        )

    with pytest.raises(ValueError, match="at or before epoch"):
        solana_sccp_vote_account_data_from_raw_vote_state(
            raw_v3, 0, vote_account_address
        )


def test_derives_solana_stake_account_data_hash_from_raw_stake_state_v2() -> None:
    raw = sample_solana_stake_state_v2_stake_account()
    parsed = solana_sccp_stake_account_data_from_raw_stake_state_v2(raw)
    assert parsed["staker"] == bytes([0x81]) * 32
    assert parsed["withdrawer"] == bytes([0x91]) * 32
    assert parsed["voterPubkey"] == bytes([0xA1]) * 32
    assert parsed["delegatedStake"] == 1_000
    assert parsed["activationEpoch"] == 2
    assert parsed["deactivationEpoch"] == 9
    assert parsed["warmupCooldownRateBytes"] == bytes(
        [0x0A, 0xD7, 0xA3, 0x70, 0x3D, 0x0A, 0xB7, 0x3F]
    )
    assert parsed["creditsObserved"] == 123
    assert parsed["stakeFlags"] == 1
    assert (
        solana_sccp_stake_account_data_hash_from_raw_stake_state_v2(raw)
        == solana_sccp_stake_account_data_hash(parsed)
    )

    wrong_variant = bytearray(raw)
    wrong_variant[0:4] = (1).to_bytes(4, "little")
    with pytest.raises(TypeError, match="StakeStateV2::Stake"):
        solana_sccp_stake_account_data_from_raw_stake_state_v2(bytes(wrong_variant))
    with pytest.raises(TypeError, match="200-byte"):
        solana_sccp_stake_account_data_from_raw_stake_state_v2(raw[:199])

    hidden_padding = bytearray(raw)
    hidden_padding[197] = 1
    with pytest.raises(TypeError, match="padding"):
        solana_sccp_stake_account_data_from_raw_stake_state_v2(bytes(hidden_padding))

    unknown_flags = bytearray(raw)
    unknown_flags[196] = 2
    with pytest.raises(TypeError, match="StakeFlags"):
        solana_sccp_stake_account_data_from_raw_stake_state_v2(bytes(unknown_flags))

    zero_voter = bytearray(raw)
    zero_voter[124:156] = bytes(32)
    with pytest.raises(TypeError, match="voterPubkey"):
        solana_sccp_stake_account_data_from_raw_stake_state_v2(bytes(zero_voter))

    zero_delegation = bytearray(raw)
    zero_delegation[156:164] = (0).to_bytes(8, "little")
    with pytest.raises(ValueError, match="delegatedStake"):
        solana_sccp_stake_account_data_from_raw_stake_state_v2(
            bytes(zero_delegation)
        )

    legacy_warmup_cooldown_rate = bytearray(raw)
    legacy_warmup_cooldown_rate[180:188] = bytes(
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD0, 0x3F]
    )
    assert solana_sccp_stake_account_data_from_raw_stake_state_v2(
        bytes(legacy_warmup_cooldown_rate)
    )["warmupCooldownRateBytes"] == bytes(
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD0, 0x3F]
    )

    zero_warmup_cooldown_rate = bytearray(raw)
    zero_warmup_cooldown_rate[180:188] = bytes(8)
    with pytest.raises(TypeError, match="warmupCooldownRateBytes"):
        solana_sccp_stake_account_data_from_raw_stake_state_v2(
            bytes(zero_warmup_cooldown_rate)
        )

    invalid_epoch_order = bytearray(raw)
    invalid_epoch_order[172:180] = (2).to_bytes(8, "little")
    with pytest.raises(ValueError, match="deactivationEpoch"):
        solana_sccp_stake_account_data_from_raw_stake_state_v2(
            bytes(invalid_epoch_order)
        )


def test_derives_solana_stake_account_state_hash_for_finality_context() -> None:
    input_value = {
        "epoch": 3,
        "validator_public_keys": ["0x" + "11" * 32, "0x" + "22" * 32],
        "validator_stakes": [1, 2],
        "validator_activation_epochs": [0, 2],
        "validator_deactivation_epochs": [2**64 - 1, 9],
        "validator_vote_account_addresses": ["0x" + "33" * 32, "0x" + "44" * 32],
        "validator_stake_account_addresses": ["0x" + "55" * 32, "0x" + "66" * 32],
        "validator_vote_account_hashes": ["0x" + "77" * 32, "0x" + "88" * 32],
        "validator_stake_account_hashes": ["0x" + "99" * 32, "0x" + "aa" * 32],
    }

    assert len(canonical_solana_sccp_stake_account_state_bytes(input_value)) == 437
    assert (
        solana_sccp_stake_account_state_hash(input_value)
        == "0x34f6086dd8c1770770802be17b833ed7c973fdaa002c866c0462c33d6938f5b5"
    )
    with pytest.raises(TypeError, match="epoch must not use multiple aliases"):
        solana_sccp_stake_account_state_hash(
            {**input_value, "validatorEpoch": input_value["epoch"]}
        )
    with pytest.raises(
        TypeError,
        match="validatorVoteAccountAddresses must not use multiple aliases",
    ):
        solana_sccp_stake_account_state_hash(
            {
                **input_value,
                "validatorVoteAccountAddresses": input_value[
                    "validator_vote_account_addresses"
                ],
            }
        )
    with pytest.raises(
        TypeError,
        match="validatorVoteAccountHashes must not use multiple aliases",
    ):
        solana_sccp_stake_account_state_hash(
            {
                **input_value,
                "voteAccountHashes": input_value["validator_vote_account_hashes"],
            }
        )
    with pytest.raises(TypeError, match="validatorVoteAccountAddresses must match"):
        solana_sccp_stake_account_state_hash(
            {**input_value, "validator_vote_account_addresses": ["0x" + "33" * 32]}
        )
    with pytest.raises(TypeError, match="validatorVoteAccountAddresses must not contain duplicates"):
        solana_sccp_stake_account_state_hash(
            {
                **input_value,
                "validator_vote_account_addresses": [
                    "0x" + "33" * 32,
                    "0x" + "33" * 32,
                ],
            }
        )
    with pytest.raises(TypeError, match=r"validatorStakeAccountAddresses\[1\]"):
        solana_sccp_stake_account_state_hash(
            {
                **input_value,
                "validator_stake_account_addresses": [
                    "0x" + "55" * 32,
                    "0x" + "44" * 32,
                ],
            }
        )
    with pytest.raises(TypeError, match=r"validatorVoteAccountAddresses\[0\]"):
        solana_sccp_stake_account_state_hash(
            {
                **input_value,
                "validator_vote_account_addresses": [
                    "0x" + "66" * 32,
                    "0x" + "44" * 32,
                ],
            }
        )
    with pytest.raises(TypeError, match=r"validatorVoteAccountHashes\[1\]"):
        solana_sccp_stake_account_state_hash(
            {
                **input_value,
                "validator_vote_account_hashes": [
                    "0x" + "77" * 32,
                    "0x" + "00" * 32,
                ],
            }
        )


def test_derives_solana_stake_history_hash_for_finality_context() -> None:
    input_value = {
        "epoch": 3,
        "validator_public_keys": ["0x" + "11" * 32, "0x" + "22" * 32],
        "validator_stakes": [1, 2],
        "validator_delegated_stakes": [1, 3],
        "validator_activation_epochs": [0, 2],
        "validator_deactivation_epochs": [2**64 - 1, 9],
        "validator_vote_account_addresses": ["0x" + "33" * 32, "0x" + "44" * 32],
        "validator_stake_account_addresses": ["0x" + "55" * 32, "0x" + "66" * 32],
        "validator_vote_account_hashes": ["0x" + "77" * 32, "0x" + "88" * 32],
        "validator_stake_account_hashes": ["0x" + "99" * 32, "0x" + "aa" * 32],
        "stake_history_entries": [
            {"epoch": 2, "effective": 23, "activating": 3, "deactivating": 0},
            {"epoch": 3, "effective": 3, "activating": 1, "deactivating": 0},
        ],
    }

    assert len(canonical_solana_sccp_stake_history_bytes(input_value)) == 249
    assert (
        solana_sccp_stake_history_hash(input_value)
        == "0xd75957eec3cf9f5b88076c8dc18e81c5debd627adfbed7e03e35443bcc4d14b6"
    )
    with pytest.raises(TypeError, match="validatorPublicKeys must not use multiple aliases"):
        solana_sccp_stake_history_hash(
            {
                **input_value,
                "validatorPublicKeys": input_value["validator_public_keys"],
            }
        )
    with pytest.raises(TypeError, match="validatorActivationEpochs must not use multiple aliases"):
        solana_sccp_stake_history_hash(
            {
                **input_value,
                "activationEpochs": input_value["validator_activation_epochs"],
            }
        )
    with pytest.raises(TypeError, match="validatorDelegatedStakes must not use multiple aliases"):
        solana_sccp_stake_history_hash(
            {
                **input_value,
                "delegatedStakes": input_value["validator_delegated_stakes"],
            }
        )
    with pytest.raises(TypeError, match="stakeHistoryEntries must not use multiple aliases"):
        solana_sccp_stake_history_hash(
            {
                **input_value,
                "stakeHistory": input_value["stake_history_entries"],
            }
        )
    with pytest.raises(ValueError, match=r"validatorDelegatedStakes\[0\]"):
        solana_sccp_stake_history_hash(
            {**input_value, "validator_delegated_stakes": [0, 3]}
        )
    with pytest.raises(ValueError, match=r"validatorStakes\[1\]"):
        solana_sccp_stake_history_hash(
            {**input_value, "validator_stakes": [1, 1]}
        )
    with pytest.raises(
        ValueError,
        match="signed epoch StakeHistory effective stake must equal replayed validator effective stake",
    ):
        solana_sccp_stake_history_hash(
            {
                **input_value,
                "stake_history_entries": [
                    input_value["stake_history_entries"][0],
                    {**input_value["stake_history_entries"][1], "effective": 4},
                ],
            }
        )
    with pytest.raises(ValueError, match="stakeHistoryEntries must include the signed epoch"):
        solana_sccp_stake_history_hash(
            {**input_value, "stake_history_entries": input_value["stake_history_entries"][:1]}
        )
    with pytest.raises(ValueError, match="strictly increasing epoch"):
        solana_sccp_stake_history_hash(
            {
                **input_value,
                "stake_history_entries": list(
                    reversed(input_value["stake_history_entries"])
                ),
            }
        )


def test_derives_solana_stake_history_sysvar_data_hash() -> None:
    input_value = {
        "stake_history_entries": [
            {"epoch": 2, "effective": 10, "activating": 3, "deactivating": 1},
            {"epoch": 3, "effective": 12, "activating": 0, "deactivating": 0},
        ]
    }

    canonical = canonical_solana_sccp_stake_history_sysvar_data_bytes(input_value)
    assert len(canonical) == 72
    assert int.from_bytes(canonical[8:16], "little") == 3
    data_hash = solana_sccp_stake_history_sysvar_data_hash(input_value)
    assert data_hash.startswith("0x") and len(data_hash) == 66
    assert solana_sccp_stake_history_sysvar_data_hash_from_raw_data(canonical) == data_hash
    with pytest.raises(TypeError, match="stakeHistoryEntries must not use multiple aliases"):
        solana_sccp_stake_history_sysvar_data_hash(
            {
                **input_value,
                "stakeHistory": input_value["stake_history_entries"],
            }
        )
    assert data_hash != solana_sccp_stake_history_sysvar_data_hash(
        {
            "stake_history_entries": [
                input_value["stake_history_entries"][0],
                {
                    **input_value["stake_history_entries"][1],
                    "effective": 13,
                },
            ]
            }
        )
    with pytest.raises(TypeError, match="bincode Vec"):
        solana_sccp_stake_history_sysvar_data_hash_from_raw_data(canonical[:9])
    wrong_count = bytearray(canonical)
    wrong_count[:8] = (3).to_bytes(8, "little")
    with pytest.raises(TypeError, match="1..512"):
        solana_sccp_stake_history_sysvar_data_hash_from_raw_data(bytes(wrong_count))
    ascending_raw = bytearray(canonical)
    ascending_raw[8:40] = canonical[40:72]
    ascending_raw[40:72] = canonical[8:40]
    with pytest.raises(TypeError, match="newest-first"):
        solana_sccp_stake_history_sysvar_data_hash_from_raw_data(bytes(ascending_raw))
    with pytest.raises(ValueError, match="strictly increasing epoch"):
        solana_sccp_stake_history_sysvar_data_hash(
            {
                "stake_history_entries": list(
                    reversed(input_value["stake_history_entries"])
                )
            }
        )


def test_derives_solana_bank_fork_hash_for_finality_context() -> None:
    accounts_lt_hash = bytes([0x99]) * 2048
    bank_signature_count = 8
    parent_bank_hash = "0x" + "33" * 32
    blockhash = "0x" + "55" * 32
    bank_hash = solana_sccp_agave_bank_hash(
        {
            "parent_bank_hash": parent_bank_hash,
            "bank_signature_count": bank_signature_count,
            "blockhash": blockhash,
            "accounts_lt_hash": accounts_lt_hash,
        }
    )
    input_value = {
        "finalized_slot": 1_296_096,
        "parent_slot": 1_296_095,
        "bank_signature_count": bank_signature_count,
        "parent_bank_hash": parent_bank_hash,
        "bank_hash": bank_hash,
        "blockhash": blockhash,
        "accounts_lt_hash": accounts_lt_hash,
        "transaction_status_root": "0x" + "66" * 32,
        "account_inclusion_root": "0x" + "77" * 32,
        "accounts_lt_hash_checksum": solana_sccp_accounts_lt_hash_checksum(accounts_lt_hash),
    }

    assert len(canonical_solana_sccp_bank_fork_bytes(input_value)) == 229
    assert (
        solana_sccp_bank_fork_hash(input_value)
        == "0x8c496fb25a4499947e454a84f638211a84445748bc5242fbb6fb511edd82e531"
    )
    assert solana_sccp_bank_fork_hash(
        {**input_value, "epoch": 3}
    ) == solana_sccp_bank_fork_hash(input_value)

    with pytest.raises(TypeError, match="finalizedSlot must not use multiple aliases"):
        solana_sccp_bank_fork_hash(
            {**input_value, "finalizedSlot": input_value["finalized_slot"]}
        )

    with pytest.raises(TypeError, match="epoch must not use multiple aliases"):
        solana_sccp_bank_fork_hash({**input_value, "epoch": 3, "validatorEpoch": 3})

    with pytest.raises(TypeError, match="parentSlot must not use multiple aliases"):
        solana_sccp_bank_fork_hash(
            {**input_value, "parentSlot": input_value["parent_slot"]}
        )

    with pytest.raises(TypeError, match="bankHash must not use multiple aliases"):
        solana_sccp_bank_fork_hash(
            {**input_value, "bankHash": input_value["bank_hash"]}
        )

    with pytest.raises(TypeError, match="transactionStatusRoot must not use multiple aliases"):
        solana_sccp_bank_fork_hash(
            {
                **input_value,
                "receiptOrMessageRoot": input_value["transaction_status_root"],
            }
        )

    with pytest.raises(ValueError, match="epoch must match Solana mainnet finalizedSlot"):
        solana_sccp_bank_fork_hash({**input_value, "epoch": 4})

    with pytest.raises(ValueError, match="parentSlot must be the direct parent"):
        solana_sccp_bank_fork_hash({**input_value, "parent_slot": 1_296_094})

    with pytest.raises(ValueError, match="bankSignatureCount must be nonzero"):
        solana_sccp_bank_fork_hash({**input_value, "bank_signature_count": 0})

    with pytest.raises(TypeError, match="parentBankHash must differ from bankHash"):
        solana_sccp_bank_fork_hash(
            {**input_value, "bank_hash": input_value["parent_bank_hash"]}
        )

    with pytest.raises(TypeError, match="bankHash must match Agave bank hash inputs"):
        solana_sccp_bank_fork_hash(
            {
                **input_value,
                "accounts_lt_hash": accounts_lt_hash,
                "accounts_lt_hash_checksum": solana_sccp_accounts_lt_hash_checksum(
                    accounts_lt_hash
                ),
                "bank_hash": "0x" + "44" * 32,
            }
        )

    with pytest.raises(TypeError, match="blockhash must not be zero"):
        solana_sccp_bank_fork_hash({**input_value, "blockhash": "0x" + "00" * 32})

    with pytest.raises(TypeError, match="accountInclusionRoot must not be zero"):
        solana_sccp_bank_fork_hash(
            {**input_value, "account_inclusion_root": "0x" + "00" * 32}
        )

    with pytest.raises(TypeError, match="accountsLtHashChecksum must not be zero"):
        solana_sccp_bank_fork_hash(
            {**input_value, "accounts_lt_hash_checksum": "0x" + "00" * 32}
        )

    with pytest.raises(TypeError, match="accountsLtHashChecksum must match accountsLtHash"):
        solana_sccp_bank_fork_hash(
            {**input_value, "accounts_lt_hash_checksum": "0x" + "88" * 32}
        )

    with pytest.raises(TypeError, match="parentBankHash must not use multiple aliases"):
        solana_sccp_agave_bank_hash(
            {
                "parent_bank_hash": parent_bank_hash,
                "parentBankHash": parent_bank_hash,
                "bank_signature_count": bank_signature_count,
                "blockhash": blockhash,
                "accounts_lt_hash": accounts_lt_hash,
            }
        )

    with pytest.raises(TypeError, match="bankSignatureCount must not use multiple aliases"):
        solana_sccp_agave_bank_hash(
            {
                "parent_bank_hash": parent_bank_hash,
                "bank_signature_count": bank_signature_count,
                "bankSignatureCount": bank_signature_count,
                "blockhash": blockhash,
                "accounts_lt_hash": accounts_lt_hash,
            }
        )

    with pytest.raises(TypeError, match="blockhash must not use multiple aliases"):
        solana_sccp_agave_bank_hash(
            {
                "parent_bank_hash": parent_bank_hash,
                "bank_signature_count": bank_signature_count,
                "blockhash": blockhash,
                "blockhashBytes": bytes.fromhex(blockhash[2:]),
                "accounts_lt_hash": accounts_lt_hash,
            }
        )

    with pytest.raises(TypeError, match="accountsLtHash must not use multiple aliases"):
        solana_sccp_agave_bank_hash(
            {
                "parent_bank_hash": parent_bank_hash,
                "bank_signature_count": bank_signature_count,
                "blockhash": blockhash,
                "accounts_lt_hash": accounts_lt_hash,
                "accountsLtHash": accounts_lt_hash,
            }
        )

    with pytest.raises(ValueError, match="bankHashHardForkData is too large"):
        solana_sccp_agave_bank_hash(
            {
                **input_value,
                "accounts_lt_hash": accounts_lt_hash,
                "bank_hash_hard_fork_data": bytes(1025),
            }
        )


def test_solana_accounts_lt_hash_helpers_use_pure_python_blake3_fallback(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert sccp_module._blake3_digest(b"").hex() == (
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    )
    assert sccp_module._blake3_digest(b"abc", length=64).hex() == (
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        "1fb250ae7393f5d02813b65d521a0d492d9ba09cf7ce7f4cffd900f23374bf0b"
    )
    monkeypatch.setattr(sccp_module, "_blake3", None)
    opening = {
        "address": "0x" + "11" * 32,
        "owner": "0x" + "22" * 32,
        "lamports": 1,
    }
    assert sccp_module._blake3_digest(b"abc", length=64).hex() == (
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        "1fb250ae7393f5d02813b65d521a0d492d9ba09cf7ce7f4cffd900f23374bf0b"
    )
    assert len(solana_sccp_account_lt_hash(opening, b"account-data")) == 2048


def test_solana_accounts_lt_hash_fallback_matches_max_account_data_vector(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(sccp_module, "_blake3", None)
    opening = {
        "address": "0x" + "11" * 32,
        "owner": "0x" + "22" * 32,
        "lamports": 123_456_789,
        "rent_epoch": 0,
        "executable": False,
    }
    raw_data = bytes(index & 0xFF for index in range(65_536))
    account_lt_hash = solana_sccp_account_lt_hash(opening, raw_data)

    assert account_lt_hash[:32].hex() == (
        "9812a52eb28f98016e17315f8868bd0a2794525d0ddab658e79bc2f1ed4f5945"
    )
    assert account_lt_hash[-32:].hex() == (
        "9fd58303a142ffb1502efd15bc37dc205012d1ab14b305f307197badaa234b79"
    )
    assert solana_sccp_accounts_lt_hash_checksum(account_lt_hash) == (
        "0x498cd269c3aff95266102559e7cc8e0acc58400fa5570aa7c01267c4bb550c10"
    )
    with pytest.raises(TypeError, match="executable must be a boolean"):
        solana_sccp_account_lt_hash({**opening, "executable": "false"}, raw_data)
    with pytest.raises(TypeError, match="address must not use multiple aliases"):
        solana_sccp_account_lt_hash(
            {**opening, "accountAddress": opening["address"]}, raw_data
        )


def test_derives_solana_account_inclusion_leaves_branches_and_roots() -> None:
    finalized_slot = 1_296_096
    openings = [
        {
            "address": "0x" + "31" * 32,
            "owner": SCCP_SOLANA_VOTE_PROGRAM_ID,
            "lamports": 1_000_000,
            "rent_epoch": 0,
            "executable": False,
            "data_hash": "0x" + "91" * 32,
        },
        {
            "address": "0x" + "41" * 32,
            "owner": SCCP_SOLANA_STAKE_PROGRAM_ID,
            "lamports": 1_000_001,
            "rent_epoch": 0,
            "executable": False,
            "data_hash": "0x" + "92" * 32,
        },
        {
            "address": "0x" + "51" * 32,
            "owner": SCCP_SOLANA_STAKE_PROGRAM_ID,
            "lamports": 1_000_002,
            "rent_epoch": 0,
            "executable": False,
            "data_hash": "0x" + "93" * 32,
        },
    ]
    raw_data = ["0x" + "01" * 64, "0x" + "02" * 64, "0x" + "03" * 64]
    leaf_inputs = [
        {"finalized_slot": finalized_slot, "opening": opening, "raw_data": raw_data[index]}
        for index, opening in enumerate(openings)
    ]
    assert len(canonical_solana_sccp_account_inclusion_leaf_bytes(leaf_inputs[0])) == 109
    assert solana_sccp_account_raw_data_hash(raw_data[0]).startswith("0x")
    with pytest.raises(TypeError, match="finalizedSlot must not use multiple aliases"):
        canonical_solana_sccp_account_inclusion_leaf_bytes(
            {**leaf_inputs[0], "finalizedSlot": finalized_slot}
        )
    with pytest.raises(TypeError, match="opening must not use multiple aliases"):
        canonical_solana_sccp_account_inclusion_leaf_bytes(
            {**leaf_inputs[0], "accountOpening": leaf_inputs[0]["opening"]}
        )
    with pytest.raises(TypeError, match="rawData must not use multiple aliases"):
        canonical_solana_sccp_account_inclusion_leaf_bytes(
            {**leaf_inputs[0], "rawData": raw_data[0]}
        )
    with pytest.raises(TypeError, match="rawDataHash must match rawData"):
        canonical_solana_sccp_account_inclusion_leaf_bytes(
            {**leaf_inputs[0], "raw_data_hash": "0x" + "44" * 32}
        )
    with pytest.raises(TypeError, match=r"opening\.address must not use multiple aliases"):
        canonical_solana_sccp_account_inclusion_leaf_bytes(
            {
                **leaf_inputs[0],
                "opening": {
                    **leaf_inputs[0]["opening"],
                    "accountAddress": leaf_inputs[0]["opening"]["address"],
                },
            }
        )
    leaves = [solana_sccp_account_inclusion_leaf_hash(item) for item in leaf_inputs]
    assert len(canonical_solana_sccp_account_inclusion_node_bytes(leaves[0], leaves[1])) == 65
    assert solana_sccp_account_inclusion_node_hash(leaves[0], leaves[1]).startswith("0x")

    witness = solana_sccp_account_inclusion_root_and_branches(leaves)
    assert witness["root"].startswith("0x")
    assert len(witness["branches"]) == len(leaves)
    assert solana_sccp_account_inclusion_root_from_branch(
        leaves[0], witness["branches"][0]
    ) == witness["root"]
    assert solana_sccp_account_inclusion_root_from_branch(
        leaves[1], witness["branches"][1]
    ) == witness["root"]

    opened_witness = solana_sccp_opened_account_inclusion_witness(
        {
            "finalized_slot": finalized_slot,
            "validator_vote_account_openings": [openings[0]],
            "validator_vote_account_raw_data": [raw_data[0]],
            "validator_stake_account_openings": [openings[1]],
            "validator_stake_account_raw_data": [raw_data[1]],
            "stake_history_sysvar_opening": openings[2],
            "stake_history_sysvar_raw_data": raw_data[2],
            "account_inclusion_root": witness["root"],
        }
    )
    assert opened_witness["branches"] == witness["branches"]
    assert opened_witness["validator_vote_account_branches"] == [witness["branches"][0]]
    assert opened_witness["validator_stake_account_branches"] == [witness["branches"][1]]
    assert opened_witness["stake_history_sysvar_branch"] == witness["branches"][2]
    with pytest.raises(TypeError, match="finalizedSlot must not use multiple aliases"):
        solana_sccp_opened_account_inclusion_witness(
            {
                "finalized_slot": finalized_slot,
                "finalizedSlot": finalized_slot,
                "validator_vote_account_openings": [openings[0]],
                "validator_vote_account_raw_data": [raw_data[0]],
                "validator_stake_account_openings": [openings[1]],
                "validator_stake_account_raw_data": [raw_data[1]],
                "stake_history_sysvar_opening": openings[2],
                "stake_history_sysvar_raw_data": raw_data[2],
            }
        )
    with pytest.raises(TypeError, match="validatorVoteAccountOpenings must not use multiple aliases"):
        solana_sccp_opened_account_inclusion_witness(
            {
                "finalized_slot": finalized_slot,
                "validator_vote_account_openings": [openings[0]],
                "voteAccountOpenings": [openings[0]],
                "validator_vote_account_raw_data": [raw_data[0]],
                "validator_stake_account_openings": [openings[1]],
                "validator_stake_account_raw_data": [raw_data[1]],
                "stake_history_sysvar_opening": openings[2],
                "stake_history_sysvar_raw_data": raw_data[2],
            }
        )
    with pytest.raises(TypeError, match="stakeHistorySysvarOpening must not use multiple aliases"):
        solana_sccp_opened_account_inclusion_witness(
            {
                "finalized_slot": finalized_slot,
                "validator_vote_account_openings": [openings[0]],
                "validator_vote_account_raw_data": [raw_data[0]],
                "validator_stake_account_openings": [openings[1]],
                "validator_stake_account_raw_data": [raw_data[1]],
                "stake_history_sysvar_opening": openings[2],
                "stakeHistorySysvarOpening": openings[2],
                "stake_history_sysvar_raw_data": raw_data[2],
            }
        )
    with pytest.raises(TypeError, match="accountInclusionRoot must not use multiple aliases"):
        solana_sccp_opened_account_inclusion_witness(
            {
                "finalized_slot": finalized_slot,
                "validator_vote_account_openings": [openings[0]],
                "validator_vote_account_raw_data": [raw_data[0]],
                "validator_stake_account_openings": [openings[1]],
                "validator_stake_account_raw_data": [raw_data[1]],
                "stake_history_sysvar_opening": openings[2],
                "stake_history_sysvar_raw_data": raw_data[2],
                "account_inclusion_root": witness["root"],
                "accountsRoot": witness["root"],
            }
        )
    duplicate_stake_opening = {**openings[1], "address": openings[0]["address"]}
    with pytest.raises(ValueError, match="opened account addresses must be unique"):
        solana_sccp_opened_account_inclusion_witness(
            {
                "finalized_slot": finalized_slot,
                "validator_vote_account_openings": [openings[0]],
                "validator_vote_account_raw_data": [raw_data[0]],
                "validator_stake_account_openings": [duplicate_stake_opening],
                "validator_stake_account_raw_data": [raw_data[1]],
                "stake_history_sysvar_opening": openings[2],
                "stake_history_sysvar_raw_data": raw_data[2],
            }
        )
    with pytest.raises(TypeError, match="accountInclusionRoot must match"):
        solana_sccp_opened_account_inclusion_witness(
            {
                "finalized_slot": finalized_slot,
                "validator_vote_account_openings": [openings[0]],
                "validator_vote_account_raw_data": [raw_data[0]],
                "validator_stake_account_openings": [openings[1]],
                "validator_stake_account_raw_data": [raw_data[1]],
                "stake_history_sysvar_opening": openings[2],
                "stake_history_sysvar_raw_data": raw_data[2],
                "account_inclusion_root": "0x" + "77" * 32,
            }
        )

    mutated_leaf = solana_sccp_account_inclusion_leaf_hash(
        {
            "finalized_slot": finalized_slot,
            "opening": openings[0],
            "raw_data": "0x" + "04" * 64,
        }
    )
    assert (
        solana_sccp_account_inclusion_root_from_branch(
            mutated_leaf, witness["branches"][0]
        )
        != witness["root"]
    )
    with pytest.raises(TypeError, match="leaf"):
        solana_sccp_account_inclusion_root_from_branch("0x" + "00" * 32, [])
    with pytest.raises(ValueError, match="at most 64"):
        solana_sccp_account_inclusion_root_from_branch(leaves[0], [HEX32_E] * 65)
    with pytest.raises(ValueError, match="validatorVoteAccountOpenings.*at most"):
        solana_sccp_opened_account_inclusion_witness(
            {
                "finalized_slot": finalized_slot,
                "validator_vote_account_openings": [openings[0]]
                * (SCCP_SOLANA_MAX_VALIDATORS + 1),
                "validator_vote_account_raw_data": [raw_data[0]]
                * (SCCP_SOLANA_MAX_VALIDATORS + 1),
                "validator_stake_account_openings": [openings[1]],
                "validator_stake_account_raw_data": [raw_data[1]],
                "stake_history_sysvar_opening": openings[2],
                "stake_history_sysvar_raw_data": raw_data[2],
            }
        )
    with pytest.raises((TypeError, ValueError), match="rawData"):
        solana_sccp_account_raw_data_hash("0x")
    with pytest.raises(TypeError, match="unique"):
        solana_sccp_account_inclusion_root_and_branches([leaves[0], leaves[0]])


def test_derives_all_source_proof_hashes_from_canonical_witness_material() -> None:
    source_event_digest = "0x" + "34" * 32
    inclusion_branch = [HEX32_E]
    changed_branch = [HEX32_F]

    evm_input = {
        "source_domain": SCCP_DOMAIN_ETH,
        "source_event_digest": source_event_digest,
        "beacon_slot": "11",
        "execution_block_number": "12",
        "execution_block_hash": HEX32_A,
        "execution_receipts_root": EVM_RECEIPT_STATE_TRANSACTION_ROOT,
        "beacon_finalized_root": HEX32_C,
        "sync_committee_root": HEX32_D,
        "receipt_root_index": "0",
        "receipt_trie_proof_nodes": [EVM_RECEIPT_STATE_MPT_NODE_HEX],
        "inclusion_branch": inclusion_branch,
    }
    assert len(canonical_evm_sccp_receipt_proof_bytes(evm_input)) == 306
    assert (
        evm_sccp_receipt_proof_hash(evm_input)
        == "0x83401a795ea5b44da20f79de7e1d441c9b616867b7704d68ec374538394244a5"
    )
    with pytest.raises(ValueError, match="sourceDomain"):
        canonical_evm_sccp_receipt_proof_bytes({**evm_input, "source_domain": SCCP_DOMAIN_BSC})
    with pytest.raises(TypeError, match="sourceEventDigest must not be zero"):
        canonical_evm_sccp_receipt_proof_bytes(
            {**evm_input, "source_event_digest": SCCP_ZERO_HASH_V1}
        )
    evm_receipt_alias_cases = [
        ({"sourceDomain": SCCP_DOMAIN_ETH}, "sourceDomain"),
        ({"sourceEventDigest": source_event_digest}, "sourceEventDigest"),
        ({"beaconSlot": "11"}, "beaconSlot"),
        ({"finalityHeight": "12"}, "executionBlockNumber"),
        ({"finalityBlockHash": HEX32_A}, "executionBlockHash"),
        ({"receiptOrMessageRoot": EVM_RECEIPT_STATE_TRANSACTION_ROOT}, "executionReceiptsRoot"),
        ({"beaconFinalizedRoot": HEX32_C}, "beaconFinalizedRoot"),
        ({"syncCommitteeRoot": HEX32_D}, "syncCommitteeRoot"),
        ({"receiptRootIndex": "0"}, "receiptRootIndex"),
        ({"receiptTrieProofNodes": [EVM_RECEIPT_STATE_MPT_NODE_HEX]}, "receiptTrieProofNodes"),
        ({"inclusionBranch": inclusion_branch}, "inclusionBranch"),
    ]
    for patch, label in evm_receipt_alias_cases:
        with pytest.raises(TypeError, match=f"{label} must not use multiple aliases"):
            canonical_evm_sccp_receipt_proof_bytes({**evm_input, **patch})
    assert evm_sccp_receipt_proof_hash(evm_input) != evm_sccp_receipt_proof_hash(
        {**evm_input, "inclusion_branch": changed_branch}
    )

    bsc_input = {
        "source_domain": SCCP_DOMAIN_BSC,
        "source_event_digest": source_event_digest,
        "validator_epoch": "21",
        "block_number": "22",
        "block_hash": HEX32_A,
        "receipts_root": EVM_RECEIPT_STATE_TRANSACTION_ROOT,
        "validator_set_hash": HEX32_C,
        "commit_seal_hash": HEX32_D,
        "receipt_root_index": "0",
        "receipt_trie_proof_nodes": [EVM_RECEIPT_STATE_MPT_NODE_HEX],
        "inclusion_branch": inclusion_branch,
    }
    assert len(canonical_bsc_sccp_receipt_proof_bytes(bsc_input)) == 306
    assert (
        bsc_sccp_receipt_proof_hash(bsc_input)
        == "0x392d082fce4b345666e176a1f0cf34ba83bd00b3307e8f93ba62ecda7c760f9e"
    )
    with pytest.raises(ValueError, match="sourceDomain"):
        canonical_bsc_sccp_receipt_proof_bytes({**bsc_input, "source_domain": SCCP_DOMAIN_ETH})
    with pytest.raises(TypeError, match="sourceEventDigest must not be zero"):
        canonical_bsc_sccp_receipt_proof_bytes(
            {**bsc_input, "source_event_digest": SCCP_ZERO_HASH_V1}
        )
    bsc_receipt_alias_cases = [
        ({"sourceDomain": SCCP_DOMAIN_BSC, "source_domain": SCCP_DOMAIN_BSC}, "sourceDomain"),
        ({"sourceEventDigest": source_event_digest}, "sourceEventDigest"),
        ({"finalityHeight": "22"}, "blockNumber"),
        ({"finalityBlockHash": HEX32_A}, "blockHash"),
        ({"receiptOrMessageRoot": EVM_RECEIPT_STATE_TRANSACTION_ROOT}, "receiptsRoot"),
        ({"validatorSetHash": HEX32_C}, "validatorSetHash"),
        ({"commitSealHash": HEX32_D}, "commitSealHash"),
        ({"receiptRootIndex": "0"}, "receiptRootIndex"),
        ({"receiptTrieProofNodes": [EVM_RECEIPT_STATE_MPT_NODE_HEX]}, "receiptTrieProofNodes"),
        ({"inclusionBranch": inclusion_branch}, "inclusionBranch"),
    ]
    for patch, label in bsc_receipt_alias_cases:
        with pytest.raises(TypeError, match=f"{label} must not use multiple aliases"):
            canonical_bsc_sccp_receipt_proof_bytes({**bsc_input, **patch})
    with pytest.raises(ValueError, match="publicInputs.version"):
        canonical_sccp_message_transparent_public_inputs_bytes(
            sample_evm_public_inputs(version=0)
        )
    with pytest.raises(TypeError, match="publicInputs.version"):
        canonical_sccp_message_transparent_public_inputs_bytes(
            {**sample_evm_public_inputs(), "version": None}
        )
    assert bsc_sccp_receipt_proof_hash(bsc_input) != bsc_sccp_receipt_proof_hash(
        {**bsc_input, "inclusion_branch": changed_branch}
    )

    validator_payload_input = {
        "validator_addresses": ["0x" + "11" * 20, "0x" + "22" * 20],
        "validator_powers": ["1", "2"],
    }
    validator_payload = canonical_bsc_validator_set_payload_bytes(validator_payload_input)
    assert validator_payload.hex() == BSC_VALIDATOR_SET_PAYLOAD_HEX
    assert bsc_validator_set_payload_hash(validator_payload_input) == BSC_VALIDATOR_SET_PAYLOAD_HASH
    assert bsc_validator_set_payload_hash(validator_payload) == BSC_VALIDATOR_SET_PAYLOAD_HASH
    assert bsc_validator_set_hash_from_payload(validator_payload_input) == BSC_VALIDATOR_SET_HASH
    with pytest.raises(TypeError, match="validatorAddresses must not use multiple aliases"):
        canonical_bsc_validator_set_payload_bytes(
            {
                **validator_payload_input,
                "validatorAddresses": validator_payload_input["validator_addresses"],
            }
        )
    with pytest.raises(TypeError, match="validatorPowers must not use multiple aliases"):
        canonical_bsc_validator_set_payload_bytes(
            {
                **validator_payload_input,
                "validatorPowers": validator_payload_input["validator_powers"],
            }
        )
    with pytest.raises(ValueError, match="at most 255"):
        canonical_bsc_validator_set_payload_bytes(
            {
                "validator_addresses": [
                    "0x" + format(index + 1, "040x") for index in range(256)
                ],
                "validator_powers": ["1"] * 256,
            }
        )
    parlia_payload = canonical_bsc_validator_set_payload_bytes(
        {
            "validator_addresses": validator_payload_input["validator_addresses"],
            "validator_powers": ["1", "1"],
        }
    )
    parlia_extra = _sample_bsc_parlia_extra()
    assert bsc_validator_set_payload_from_parlia_extra(parlia_extra) == parlia_payload
    assert (
        bsc_validator_set_payload_from_header_rlp(
            _sample_bsc_parlia_header_rlp(parlia_extra)
        )
        == parlia_payload
    )
    with pytest.raises(ValueError, match="RLP list"):
        bsc_validator_set_payload_from_header_rlp(b"\x80")
    with pytest.raises(ValueError, match="unique"):
        canonical_bsc_validator_set_payload_bytes(
            {
                "validator_addresses": ["0x" + "11" * 20, "0x" + "11" * 20],
                "validator_powers": ["1", "2"],
            }
        )
    with pytest.raises(ValueError, match="must not be zero"):
        canonical_bsc_validator_set_payload_bytes(
            {"validator_addresses": ["0x" + "11" * 20], "validator_powers": ["0"]}
        )

    commit_message = {
        "validator_epoch": "2",
        "block_number": "401",
        "block_hash": "0x" + "22" * 32,
        "receipts_root": "0x" + "33" * 32,
        "validator_set_hash": BSC_COMMIT_VALIDATOR_SET_HASH,
    }
    assert len(canonical_bsc_commit_message_bytes(commit_message)) == 117
    assert bsc_commit_message_hash(commit_message) == BSC_COMMIT_MESSAGE_HASH
    with pytest.raises(ValueError, match="sourceDomain"):
        bsc_commit_message_hash({**commit_message, "source_domain": SCCP_DOMAIN_ETH})
    with pytest.raises(TypeError, match="validatorEpoch must not use multiple aliases"):
        bsc_commit_message_hash({**commit_message, "validatorEpoch": "2"})
    with pytest.raises(TypeError, match="validatorSetHash must not use multiple aliases"):
        bsc_commit_message_hash({**commit_message, "validatorSetHash": BSC_COMMIT_VALIDATOR_SET_HASH})

    commit_seal = {
        "total_power": "4",
        "signed_power": "3",
        "commit_message_hash": BSC_COMMIT_MESSAGE_HASH,
        "validator_public_keys": BSC_COMMIT_VALIDATOR_PUBLIC_KEYS,
        "validator_powers": BSC_COMMIT_VALIDATOR_POWERS,
        "signers_bitmap": "0x07",
        "signatures": BSC_COMMIT_SIGNATURES,
        "validator_set_hash": BSC_COMMIT_VALIDATOR_SET_HASH,
    }
    assert len(canonical_bsc_commit_seal_bytes(commit_seal)) == 297
    assert bsc_commit_seal_hash(commit_seal) == BSC_COMMIT_SEAL_HASH
    with pytest.raises(ValueError, match="two thirds"):
        canonical_bsc_commit_seal_bytes(
            {
                **commit_seal,
                "signed_power": "2",
                "signers_bitmap": "0x03",
                "signatures": BSC_COMMIT_SIGNATURES[:2],
            }
        )
    with pytest.raises(ValueError, match="padding bits"):
        canonical_bsc_commit_seal_bytes({**commit_seal, "signers_bitmap": "0x1f"})
    with pytest.raises(ValueError, match="recover"):
        canonical_bsc_commit_seal_bytes(
            {
                **commit_seal,
                "signatures": [
                    "0x31" + BSC_COMMIT_SIGNATURES[0][4:],
                    *BSC_COMMIT_SIGNATURES[1:],
                ],
            }
        )
    with pytest.raises(ValueError, match="validatorSetHash"):
        canonical_bsc_commit_seal_bytes({**commit_seal, "validator_set_hash": HEX32_A})
    with pytest.raises(TypeError, match="totalPower must not use multiple aliases"):
        canonical_bsc_commit_seal_bytes({**commit_seal, "totalPower": "4"})
    with pytest.raises(TypeError, match="validatorSetHash must not use multiple aliases"):
        canonical_bsc_commit_seal_bytes(
            {**commit_seal, "validatorSetHash": BSC_COMMIT_VALIDATOR_SET_HASH}
        )

    storage_value = "0x02"
    storage_value_hash = bsc_validator_set_storage_value_hash(storage_value)
    assert storage_value_hash == "0x" + _keccak_256(
        b"sccp:bsc:validator-set-storage-value:v1" + bytes.fromhex("02")
    ).hex()
    metadata_proof = {
        "state_root": HEX32_A,
        "next_validator_set_payload_hash": BSC_VALIDATOR_SET_PAYLOAD_HASH,
        "validator_contract_address": "0x" + "00" * 18 + "1000",
        "account_proof_nodes": ["0xf842a0" + "11" * 32],
        "storage_root": HEX32_B,
        "validator_set_length_slot": HEX32_C,
        "validator_set_length_value": storage_value,
        "validator_set_length_value_hash": storage_value_hash,
        "validator_set_length_proof_nodes": ["0xe4822080a0" + "22" * 32],
        "validator_storage_proofs": [
            {
                "validator_index": 0,
                "storage_slot": HEX32_D,
                "storage_value": "0x94" + "11" * 20,
                "storage_value_hash": bsc_validator_set_storage_value_hash("0x94" + "11" * 20),
                "storage_proof_nodes": ["0xe4822080a0" + "33" * 32],
            },
            {
                "validator_index": 1,
                "storage_slot": HEX32_E,
                "storage_value": "0x94" + "22" * 20,
                "storage_value_hash": bsc_validator_set_storage_value_hash("0x94" + "22" * 20),
                "storage_proof_nodes": ["0xe4822080a0" + "44" * 32],
            },
        ],
    }
    metadata_bytes = canonical_bsc_validator_set_metadata_proof_bytes(metadata_proof)
    assert len(metadata_bytes) == 560
    metadata_hash = bsc_validator_set_metadata_proof_hash(metadata_proof)
    with pytest.raises(ValueError, match="BSC ValidatorSet metadata proof version"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {**metadata_proof, "version": 0}
        )
    with pytest.raises(TypeError, match="stateRoot must not use multiple aliases"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {**metadata_proof, "stateRoot": HEX32_A}
        )
    with pytest.raises(TypeError, match="validatorSetLengthProofNodes must not use multiple aliases"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {
                **metadata_proof,
                "validatorSetLengthProofNodes": metadata_proof["validator_set_length_proof_nodes"],
            }
        )
    with pytest.raises(TypeError, match="validatorStorageProofs must not use multiple aliases"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {
                **metadata_proof,
                "validatorStorageProofs": metadata_proof["validator_storage_proofs"],
            }
        )
    with pytest.raises(ValueError, match="BSC validator storage proof version"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {
                **metadata_proof,
                "validator_storage_proofs": [
                    {**metadata_proof["validator_storage_proofs"][0], "version": 0}
                ],
            }
        )
    with pytest.raises(ValueError, match="validatorSetLengthValueHash"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {**metadata_proof, "validator_set_length_value_hash": HEX32_F}
        )
    with pytest.raises(TypeError, match="storageProofNodes must not use multiple aliases"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {
                **metadata_proof,
                "validator_storage_proofs": [
                    {
                        **metadata_proof["validator_storage_proofs"][0],
                        "storageProofNodes": metadata_proof["validator_storage_proofs"][0][
                            "storage_proof_nodes"
                        ],
                    }
                ],
            }
        )
    with pytest.raises(TypeError, match="storageValueHash must not use multiple aliases"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {
                **metadata_proof,
                "validator_storage_proofs": [
                    {
                        **metadata_proof["validator_storage_proofs"][0],
                        "storageValueHash": metadata_proof["validator_storage_proofs"][0][
                            "storage_value_hash"
                        ],
                    }
                ],
            }
        )
    with pytest.raises(ValueError, match="storageValueHash"):
        canonical_bsc_validator_set_metadata_proof_bytes(
            {
                **metadata_proof,
                "validator_storage_proofs": [
                    {
                        **metadata_proof["validator_storage_proofs"][0],
                        "storage_value_hash": HEX32_F,
                    }
                ],
            }
        )
    assert metadata_hash != bsc_validator_set_metadata_proof_hash(
        {**metadata_proof, "state_root": HEX32_B}
    )
    transition_message = {
        "from_validator_epoch": "41",
        "to_validator_epoch": "42",
        "transition_block_number": "8400",
        "transition_block_hash": HEX32_A,
        "parent_validator_set_hash": HEX32_B,
        "next_validator_set_hash": BSC_VALIDATOR_SET_HASH,
        "next_validator_set_payload_hash": BSC_VALIDATOR_SET_PAYLOAD_HASH,
        "validator_set_metadata_proof_hash": metadata_hash,
    }
    assert len(canonical_bsc_validator_set_transition_message_bytes(transition_message)) == 189
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        canonical_bsc_validator_set_transition_message_bytes(
            {
                **transition_message,
                "sourceDomain": SCCP_DOMAIN_BSC,
                "source_domain": SCCP_DOMAIN_BSC,
            }
        )
    with pytest.raises(TypeError, match="fromValidatorEpoch must not use multiple aliases"):
        canonical_bsc_validator_set_transition_message_bytes(
            {**transition_message, "fromValidatorEpoch": "41"}
        )
    with pytest.raises(TypeError, match="nextValidatorSetPayloadHash must not use multiple aliases"):
        canonical_bsc_validator_set_transition_message_bytes(
            {
                **transition_message,
                "nextValidatorSetPayloadHash": BSC_VALIDATOR_SET_PAYLOAD_HASH,
            }
        )
    with pytest.raises(ValueError, match="BSC validator-set transition message version"):
        canonical_bsc_validator_set_transition_message_bytes(
            {**transition_message, "version": 0}
        )
    assert bsc_validator_set_transition_message_hash(
        transition_message
    ) != bsc_validator_set_transition_message_hash(
        {**transition_message, "validator_set_metadata_proof_hash": HEX32_C}
    )
    with pytest.raises(ValueError, match="epoch-start block"):
        bsc_validator_set_transition_message_hash(
            {**transition_message, "transition_block_number": "8401"}
        )
    with pytest.raises(ValueError, match="fromValidatorEpoch"):
        bsc_validator_set_transition_message_hash(
            {**transition_message, "to_validator_epoch": "43"}
        )
    with pytest.raises(ValueError, match="sourceDomain"):
        bsc_validator_set_transition_message_hash({**transition_message, "source_domain": 0})

    witness_payload_input = {
        "witness_addresses": ["0x41" + "11" * 20, "0x41" + "22" * 20],
        "witness_weights": ["1", "2"],
    }
    witness_payload = canonical_tron_witness_schedule_payload_bytes(witness_payload_input)
    assert witness_payload.hex() == TRON_WITNESS_SCHEDULE_PAYLOAD_HEX
    assert (
        tron_witness_schedule_payload_hash(witness_payload_input)
        == TRON_WITNESS_SCHEDULE_PAYLOAD_HASH
    )
    assert tron_witness_schedule_payload_hash(witness_payload) == TRON_WITNESS_SCHEDULE_PAYLOAD_HASH
    assert tron_witness_schedule_hash_from_payload(witness_payload_input) == TRON_WITNESS_SCHEDULE_HASH
    with pytest.raises(TypeError, match="witnessAddresses must not use multiple aliases"):
        canonical_tron_witness_schedule_payload_bytes(
            {**witness_payload_input, "witnessAddresses": witness_payload_input["witness_addresses"]}
        )
    with pytest.raises(TypeError, match="witnessWeights must not use multiple aliases"):
        canonical_tron_witness_schedule_payload_bytes(
            {**witness_payload_input, "witnessWeights": witness_payload_input["witness_weights"]}
        )
    zero_witness_payload = bytes.fromhex("010100000041" + "00" * 20 + "0100000000000000")
    with pytest.raises(ValueError, match="TRON 0x41-prefixed address"):
        tron_witness_schedule_payload_hash(zero_witness_payload)
    with pytest.raises(ValueError, match="TRON 0x41-prefixed address"):
        tron_witness_schedule_hash_from_payload(zero_witness_payload)
    with pytest.raises(ValueError, match="at most 64"):
        canonical_tron_witness_schedule_payload_bytes(
            {
                "witness_addresses": [
                    "0x41" + "11" * 19 + f"{index:02x}" for index in range(65)
                ],
                "witness_weights": ["1"] * 65,
            }
        )
    with pytest.raises(ValueError, match="unique"):
        canonical_tron_witness_schedule_payload_bytes(
            {
                "witness_addresses": ["0x41" + "11" * 20, "0x41" + "11" * 20],
                "witness_weights": ["1", "2"],
            }
        )
    with pytest.raises(ValueError, match="TRON 0x41-prefixed address"):
        canonical_tron_witness_schedule_payload_bytes(
            {"witness_addresses": ["0x41" + "00" * 20], "witness_weights": ["1"]}
        )
    with pytest.raises(ValueError, match="must not be zero"):
        canonical_tron_witness_schedule_payload_bytes(
            {"witness_addresses": ["0x41" + "11" * 20], "witness_weights": ["0"]}
        )
    with pytest.raises(ValueError, match="total must fit u64"):
        canonical_tron_witness_schedule_payload_bytes(
            {
                "witness_addresses": ["0x41" + "11" * 20, "0x41" + "22" * 20],
                "witness_weights": [str((1 << 64) - 1), "1"],
            }
        )
    overflowing_witness_payload = bytes.fromhex(
        "0102000000"
        + "41"
        + "11" * 20
        + "ffffffffffffffff"
        + "41"
        + "22" * 20
        + "0100000000000000"
    )
    with pytest.raises(ValueError, match="total weight must fit u64"):
        tron_witness_schedule_payload_hash(overflowing_witness_payload)
    with pytest.raises(ValueError, match="total weight must fit u64"):
        tron_witness_schedule_hash_from_payload(overflowing_witness_payload)

    tron_solid_message = {
        "source_domain": SCCP_DOMAIN_TRON,
        "solid_block_number": "12345",
        "block_hash": TRON_BLOCK_ID,
        "witness_schedule_hash": TRON_WITNESS_SCHEDULE_HASH,
        "receipt_root": HEX32_B,
        "transaction_root": HEX32_D,
        "receipt_proof_hash": HEX32_C,
    }
    assert len(canonical_tron_solid_block_message_bytes(tron_solid_message)) == 173
    assert tron_solid_block_message_hash(tron_solid_message) == TRON_SOLID_BLOCK_MESSAGE_HASH
    for patch, pattern in [
        ({"sourceDomain": SCCP_DOMAIN_TRON}, "sourceDomain must not use multiple aliases"),
        ({"solidBlockNumber": "12345"}, "solidBlockNumber must not use multiple aliases"),
        ({"blockHash": TRON_BLOCK_ID}, "blockHash must not use multiple aliases"),
        ({"witnessScheduleHash": TRON_WITNESS_SCHEDULE_HASH}, "witnessScheduleHash must not use multiple aliases"),
        ({"receiptRoot": HEX32_B}, "receiptRoot must not use multiple aliases"),
        ({"transactionRoot": HEX32_D}, "transactionRoot must not use multiple aliases"),
        ({"receiptProofHash": HEX32_C}, "receiptProofHash must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_solid_block_message_bytes({**tron_solid_message, **patch})
    with pytest.raises(ValueError, match="sourceDomain must be TRON"):
        canonical_tron_solid_block_message_bytes({**tron_solid_message, "source_domain": 0})
    with pytest.raises(ValueError, match="receiptRoot must not be zero"):
        canonical_tron_solid_block_message_bytes(
            {**tron_solid_message, "receipt_root": SCCP_ZERO_HASH_V1}
        )

    tron_witness_seal = {
        "version": 1,
        "total_weight": "1",
        "signed_weight": "1",
        "solid_block_message_hash": "0x" + TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        "witness_addresses": [TRON_TEST_OWNER_ADDRESS],
        "witness_weights": ["1"],
        "signers_bitmap": "0x01",
        "signatures": ["0x" + TRON_SOURCE_EVENT_SIGNATURE_VECTOR],
    }
    assert len(canonical_tron_witness_seal_bytes(tron_witness_seal)) == 200
    assert tron_witness_seal_hash(tron_witness_seal) == TRON_WITNESS_SEAL_HASH
    for patch, pattern in [
        ({"totalWeight": "1"}, "totalWeight must not use multiple aliases"),
        ({"signedWeight": "1"}, "signedWeight must not use multiple aliases"),
        (
            {"solidBlockMessageHash": "0x" + TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR},
            "solidBlockMessageHash must not use multiple aliases",
        ),
        ({"witnessAddresses": [TRON_TEST_OWNER_ADDRESS]}, "witnessAddresses must not use multiple aliases"),
        ({"witnessWeights": ["1"]}, "witnessWeights must not use multiple aliases"),
        ({"signersBitmap": "0x01"}, "signersBitmap must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_witness_seal_bytes({**tron_witness_seal, **patch})
    with pytest.raises(ValueError, match="signatures length"):
        canonical_tron_witness_seal_bytes({**tron_witness_seal, "signatures": []})
    with pytest.raises(ValueError, match="declared signer"):
        canonical_tron_witness_seal_bytes(
            {
                **tron_witness_seal,
                "witness_addresses": ["0x41" + "11" * 20],
            }
        )
    with pytest.raises(ValueError, match="exceed two thirds"):
        canonical_tron_witness_seal_bytes(
            {
                **tron_witness_seal,
                "total_weight": "3",
                "signed_weight": "1",
                "witness_addresses": [
                    TRON_TEST_OWNER_ADDRESS,
                    "0x41" + "22" * 20,
                ],
                "witness_weights": ["1", "2"],
                "signers_bitmap": "0x01",
            }
        )

    authority_payload_input = {
        "authority_public_keys": ["0x" + "11" * 32, "0x" + "22" * 32],
        "authority_weights": ["1", "2"],
    }
    authority_payload = canonical_substrate_authority_set_payload_bytes(authority_payload_input)
    assert authority_payload.hex() == SUBSTRATE_AUTHORITY_SET_PAYLOAD_HEX
    assert (
        substrate_authority_set_payload_hash(authority_payload_input)
        == SUBSTRATE_AUTHORITY_SET_PAYLOAD_HASH
    )
    assert substrate_authority_set_payload_hash(authority_payload) == SUBSTRATE_AUTHORITY_SET_PAYLOAD_HASH
    assert (
        substrate_authority_set_hash_from_payload(authority_payload_input)
        == SUBSTRATE_AUTHORITY_SET_HASH
    )
    with pytest.raises(TypeError, match="authorityPublicKeys must not use multiple aliases"):
        canonical_substrate_authority_set_payload_bytes(
            {
                **authority_payload_input,
                "authorityPublicKeys": authority_payload_input["authority_public_keys"],
            }
        )
    with pytest.raises(TypeError, match="authorityWeights must not use multiple aliases"):
        canonical_substrate_authority_set_payload_bytes(
            {
                **authority_payload_input,
                "authorityWeights": authority_payload_input["authority_weights"],
            }
        )
    with pytest.raises(ValueError, match="unique"):
        canonical_substrate_authority_set_payload_bytes(
            {
                "authority_public_keys": ["0x" + "11" * 32, "0x" + "11" * 32],
                "authority_weights": ["1", "2"],
            }
        )
    with pytest.raises(ValueError, match="must not be zero"):
        canonical_substrate_authority_set_payload_bytes(
            {"authority_public_keys": ["0x" + "00" * 32], "authority_weights": ["1"]}
        )
    zero_authority_payload = bytearray(45)
    zero_authority_payload[0] = 1
    zero_authority_payload[1] = 1
    zero_authority_payload[37] = 1
    with pytest.raises(ValueError, match="must not be zero"):
        substrate_authority_set_hash_from_payload(bytes(zero_authority_payload))
    with pytest.raises(ValueError, match="must not be zero"):
        canonical_substrate_authority_set_payload_bytes(
            {"authority_public_keys": ["0x" + "11" * 32], "authority_weights": ["0"]}
        )
    with pytest.raises(ValueError, match="at most 2048"):
        canonical_substrate_authority_set_payload_bytes(
            {
                "authority_public_keys": ["0x" + "11" * 32] * 2049,
                "authority_weights": ["1"] * 2049,
            }
        )
    with pytest.raises(ValueError, match="at most"):
        substrate_authority_set_payload_hash(bytes(81926))


def test_derives_tron_witness_schedule_transition_transcripts_from_ui_witness_material() -> None:
    parent_payload = bytes.fromhex(TRON_PARENT_WITNESS_SCHEDULE_PAYLOAD_HEX)
    next_payload = bytes.fromhex(TRON_WITNESS_SCHEDULE_PAYLOAD_HEX)

    assert (
        tron_witness_schedule_hash_from_payload(parent_payload)
        == TRON_PARENT_WITNESS_SCHEDULE_HASH
    )
    assert tron_witness_schedule_hash_from_payload(next_payload) == TRON_WITNESS_SCHEDULE_HASH
    assert tron_witness_schedule_payload_hash(next_payload) == TRON_WITNESS_SCHEDULE_PAYLOAD_HASH

    message_input = {
        "source_domain": SCCP_DOMAIN_TRON,
        "from_witness_schedule_epoch": "7",
        "to_witness_schedule_epoch": "8",
        "transition_block_number": "12345",
        "transition_block_hash": TRON_BLOCK_ID,
        "parent_witness_schedule_hash": TRON_PARENT_WITNESS_SCHEDULE_HASH,
        "next_witness_schedule_hash": TRON_WITNESS_SCHEDULE_HASH,
        "next_witness_schedule_payload": next_payload,
    }
    expected_message = b"".join(
        (
            b"\x01",
            SCCP_DOMAIN_TRON.to_bytes(4, "little"),
            (7).to_bytes(8, "little"),
            (8).to_bytes(8, "little"),
            (12345).to_bytes(8, "little"),
            bytes.fromhex(TRON_BLOCK_ID.removeprefix("0x")),
            bytes.fromhex(TRON_PARENT_WITNESS_SCHEDULE_HASH.removeprefix("0x")),
            bytes.fromhex(TRON_WITNESS_SCHEDULE_HASH.removeprefix("0x")),
            bytes.fromhex(TRON_WITNESS_SCHEDULE_PAYLOAD_HASH.removeprefix("0x")),
        )
    )
    assert (
        canonical_tron_witness_schedule_transition_message_bytes(message_input)
        == expected_message
    )
    assert len(expected_message) == 157
    assert (
        tron_witness_schedule_transition_message_hash(message_input)
        == TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH
    )
    for patch, pattern in [
        ({"sourceDomain": SCCP_DOMAIN_TRON}, "sourceDomain must not use multiple aliases"),
        ({"fromWitnessScheduleEpoch": "7"}, "fromWitnessScheduleEpoch must not use multiple aliases"),
        ({"toWitnessScheduleEpoch": "8"}, "toWitnessScheduleEpoch must not use multiple aliases"),
        ({"transitionBlockNumber": "12345"}, "transitionBlockNumber must not use multiple aliases"),
        ({"transitionBlockHash": TRON_BLOCK_ID}, "transitionBlockHash must not use multiple aliases"),
        (
            {"parentWitnessScheduleHash": TRON_PARENT_WITNESS_SCHEDULE_HASH},
            "parentWitnessScheduleHash must not use multiple aliases",
        ),
        ({"nextWitnessScheduleHash": TRON_WITNESS_SCHEDULE_HASH}, "nextWitnessScheduleHash must not use multiple aliases"),
        ({"nextWitnessSchedulePayload": next_payload}, "nextWitnessSchedulePayload must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_witness_schedule_transition_message_bytes(
                {**message_input, **patch}
            )
    with pytest.raises(TypeError, match="nextWitnessSchedulePayloadHash must not use multiple aliases"):
        canonical_tron_witness_schedule_transition_message_bytes(
            {
                **message_input,
                "next_witness_schedule_payload_hash": TRON_WITNESS_SCHEDULE_PAYLOAD_HASH,
                "nextWitnessSchedulePayloadHash": TRON_WITNESS_SCHEDULE_PAYLOAD_HASH,
            }
        )
    assert tron_witness_schedule_transition_message_hash(
        {**message_input, "transition_block_hash": HEX32_D}
    ) != tron_witness_schedule_transition_message_hash(message_input)

    with pytest.raises(ValueError, match="toWitnessScheduleEpoch"):
        canonical_tron_witness_schedule_transition_message_bytes(
            {**message_input, "to_witness_schedule_epoch": "9"}
        )
    with pytest.raises(ValueError, match="sourceDomain"):
        canonical_tron_witness_schedule_transition_message_bytes(
            {**message_input, "source_domain": SCCP_DOMAIN_ETH}
        )
    with pytest.raises(TypeError, match="nextWitnessScheduleHash"):
        canonical_tron_witness_schedule_transition_message_bytes(
            {**message_input, "next_witness_schedule_hash": HEX32_D}
        )

    seal_input = {
        **message_input,
        "next_witness_schedule_payload_hash": TRON_WITNESS_SCHEDULE_PAYLOAD_HASH,
        "transition_message_hash": TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH,
        "seal_proof": {
            "version": 1,
            "total_weight": "1",
            "signed_weight": "1",
            "solid_block_message_hash": TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH,
            "witness_addresses": [TRON_TEST_OWNER_ADDRESS],
            "witness_weights": ["1"],
            "signers_bitmap": "0x01",
            "signatures": [TRON_WITNESS_SCHEDULE_TRANSITION_SIGNATURE],
        },
    }
    assert len(canonical_tron_witness_schedule_transition_seal_bytes(seal_input)) == 456
    assert (
        tron_witness_schedule_transition_seal_hash(seal_input)
        == TRON_WITNESS_SCHEDULE_TRANSITION_SEAL_HASH
    )
    for patch, pattern in [
        (
            {"nextWitnessSchedulePayloadHash": TRON_WITNESS_SCHEDULE_PAYLOAD_HASH},
            "nextWitnessSchedulePayloadHash must not use multiple aliases",
        ),
        (
            {"transitionMessageHash": TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH},
            "transitionMessageHash must not use multiple aliases",
        ),
        ({"sealProof": seal_input["seal_proof"]}, "sealProof must not use multiple aliases"),
        ({"witnessSealProof": seal_input["seal_proof"]}, "sealProof must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_witness_schedule_transition_seal_bytes(
                {**seal_input, **patch}
            )
    with pytest.raises(TypeError, match="totalWeight must not use multiple aliases"):
        canonical_tron_witness_schedule_transition_seal_bytes(
            {
                **seal_input,
                "seal_proof": {**seal_input["seal_proof"], "totalWeight": "1"},
            }
        )

    with pytest.raises(TypeError, match="transitionMessageHash"):
        canonical_tron_witness_schedule_transition_seal_bytes(
            {**seal_input, "transition_message_hash": HEX32_D}
        )
    with pytest.raises(ValueError, match="declared signer"):
        canonical_tron_witness_schedule_transition_seal_bytes(
            {
                **seal_input,
                "seal_proof": {
                    **seal_input["seal_proof"],
                    "signatures": [
                        "0x"
                        + f"{int(TRON_WITNESS_SCHEDULE_TRANSITION_SIGNATURE[2:4], 16) ^ 1:02x}"
                        + TRON_WITNESS_SCHEDULE_TRANSITION_SIGNATURE[4:]
                    ],
                },
            }
        )
    with pytest.raises(TypeError, match="nextWitnessSchedulePayloadHash"):
        canonical_tron_witness_schedule_transition_seal_bytes(
            {**seal_input, "next_witness_schedule_payload_hash": HEX32_D}
        )


def test_derives_substrate_authority_set_transition_transcripts_from_ui_witness_material() -> None:
    parent = {
        "authority_public_keys": ["0x" + "11" * 32, "0x" + "22" * 32, "0x" + "33" * 32],
        "authority_weights": ["5", "7", "11"],
    }
    next_set = {
        "authority_public_keys": ["0x" + "aa" * 32, "0x" + "bb" * 32, "0x" + "cc" * 32],
        "authority_weights": ["13", "17", "19"],
    }
    parent_payload = canonical_substrate_authority_set_payload_bytes(parent)
    next_payload = canonical_substrate_authority_set_payload_bytes(next_set)

    assert parent_payload.hex() == SUBSTRATE_PARENT_AUTHORITY_SET_PAYLOAD_HEX
    assert next_payload.hex() == SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HEX
    assert substrate_authority_set_hash_from_payload(parent_payload) == SUBSTRATE_PARENT_AUTHORITY_SET_HASH
    assert substrate_authority_set_hash_from_payload(next_payload) == SUBSTRATE_NEXT_AUTHORITY_SET_HASH
    assert substrate_authority_set_payload_hash(next_payload) == SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH

    message_input = {
        "source_domain": SCCP_DOMAIN_SORA_KUSAMA,
        "from_grandpa_set_id": "41",
        "to_grandpa_set_id": "42",
        "transition_block_number": "9001",
        "transition_block_hash": "0x" + "44" * 32,
        "parent_authority_set_hash": SUBSTRATE_PARENT_AUTHORITY_SET_HASH,
        "next_authority_set_hash": SUBSTRATE_NEXT_AUTHORITY_SET_HASH,
        "next_authority_set_payload_hash": SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH,
    }
    assert len(canonical_substrate_authority_set_transition_message_bytes(message_input)) == 157
    assert (
        substrate_authority_set_transition_message_hash(message_input)
        == SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH
    )
    for patch, pattern in [
        ({"sourceDomain": SCCP_DOMAIN_SORA_KUSAMA}, "sourceDomain must not use multiple aliases"),
        ({"fromGrandpaSetId": "41"}, "fromGrandpaSetId must not use multiple aliases"),
        ({"toGrandpaSetId": "42"}, "toGrandpaSetId must not use multiple aliases"),
        ({"transitionBlockNumber": "9001"}, "transitionBlockNumber must not use multiple aliases"),
        ({"transitionBlockHash": "0x" + "44" * 32}, "transitionBlockHash must not use multiple aliases"),
        (
            {"parentAuthoritySetHash": SUBSTRATE_PARENT_AUTHORITY_SET_HASH},
            "parentAuthoritySetHash must not use multiple aliases",
        ),
        (
            {"nextAuthoritySetHash": SUBSTRATE_NEXT_AUTHORITY_SET_HASH},
            "nextAuthoritySetHash must not use multiple aliases",
        ),
        (
            {"nextAuthoritySetPayloadHash": SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH},
            "nextAuthoritySetPayloadHash must not use multiple aliases",
        ),
        (
            {"nextAuthoritySetProofHash": SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH},
            "nextAuthoritySetPayloadHash must not use multiple aliases",
        ),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_substrate_authority_set_transition_message_bytes(
                {**message_input, **patch}
            )

    justification_input = {
        **message_input,
        "version": 1,
        "next_authority_set_payload": next_payload,
        "transition_message_hash": SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH,
        "grandpa_justification": {
            "version": 1,
            "total_weight": "23",
            "signed_weight": "23",
            "precommit_message_hash": SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH,
            **parent,
            "signers_bitmap": "0x07",
            "signatures": [
                "0x" + "77" * 64,
                "0x" + "88" * 64,
                "0x" + "99" * 64,
            ],
        },
    }
    assert (
        len(canonical_substrate_authority_set_transition_justification_bytes(justification_input))
        == 752
    )
    assert (
        substrate_authority_set_transition_justification_hash(justification_input)
        == SUBSTRATE_AUTHORITY_SET_TRANSITION_JUSTIFICATION_HASH
    )
    for patch, pattern in [
        (
            {"grandpaJustification": justification_input["grandpa_justification"]},
            "grandpaJustification must not use multiple aliases",
        ),
        (
            {"nextAuthoritySetPayload": next_payload},
            "nextAuthoritySetPayload must not use multiple aliases",
        ),
        (
            {"transitionMessageHash": SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH},
            "transitionMessageHash must not use multiple aliases",
        ),
        ({"sourceDomain": SCCP_DOMAIN_SORA_KUSAMA}, "sourceDomain must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_substrate_authority_set_transition_justification_bytes(
                {**justification_input, **patch}
            )
    for patch, pattern in [
        ({"totalWeight": "23"}, "totalWeight must not use multiple aliases"),
        ({"signedWeight": "23"}, "signedWeight must not use multiple aliases"),
        (
            {"precommitMessageHash": SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH},
            "precommitMessageHash must not use multiple aliases",
        ),
        (
            {"authorityPublicKeys": parent["authority_public_keys"]},
            "authorityPublicKeys must not use multiple aliases",
        ),
        (
            {"authorityWeights": parent["authority_weights"]},
            "authorityWeights must not use multiple aliases",
        ),
        ({"signersBitmap": "0x07"}, "signersBitmap must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_substrate_authority_set_transition_justification_bytes(
                {
                    **justification_input,
                    "grandpa_justification": {
                        **justification_input["grandpa_justification"],
                        **patch,
                    },
                }
            )
    with pytest.raises(
        ValueError,
        match="Substrate authority-set transition justification version",
    ):
        canonical_substrate_authority_set_transition_justification_bytes(
            {**justification_input, "version": 0}
        )
    with pytest.raises(ValueError, match="Substrate GRANDPA justification version"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {
                **justification_input,
                "grandpa_justification": {
                    **justification_input["grandpa_justification"],
                    "version": 0,
                },
            }
        )

    with pytest.raises(TypeError, match="nextAuthoritySetPayloadHash must match"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {**justification_input, "next_authority_set_payload_hash": HEX32_B}
        )
    with pytest.raises(TypeError, match="nextAuthoritySetHash must match"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {**justification_input, "next_authority_set_hash": HEX32_B}
        )
    with pytest.raises(TypeError, match="parentAuthoritySetHash must match"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {**justification_input, "parent_authority_set_hash": HEX32_B}
        )
    with pytest.raises(ValueError, match="two thirds"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {
                **justification_input,
                "grandpa_justification": {
                    **justification_input["grandpa_justification"],
                    "signed_weight": "12",
                    "signers_bitmap": "0x03",
                    "signatures": [
                        "0x" + "77" * 64,
                        "0x" + "88" * 64,
                    ],
                },
            }
        )
    with pytest.raises(ValueError, match="signersBitmap"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {
                **justification_input,
                "grandpa_justification": {
                    **justification_input["grandpa_justification"],
                    "signers_bitmap": "0x" + "ff" * 257,
                },
            }
        )
    with pytest.raises(TypeError, match="signatures length"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {
                **justification_input,
                "grandpa_justification": {
                    **justification_input["grandpa_justification"],
                    "signatures": ["0x" + "77" * 64, "0x" + "88" * 64],
                },
            }
        )
    with pytest.raises(ValueError, match="signedWeight"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {
                **justification_input,
                "grandpa_justification": {
                    **justification_input["grandpa_justification"],
                    "signed_weight": "12",
                },
            }
        )
    with pytest.raises(ValueError, match="greater than two thirds"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {
                **justification_input,
                "grandpa_justification": {
                    **justification_input["grandpa_justification"],
                    "signed_weight": "12",
                    "signers_bitmap": "0x03",
                    "signatures": ["0x" + "77" * 64, "0x" + "88" * 64],
                },
            }
        )
    with pytest.raises(TypeError, match="must not be all zero"):
        canonical_substrate_authority_set_transition_justification_bytes(
            {
                **justification_input,
                "grandpa_justification": {
                    **justification_input["grandpa_justification"],
                    "signatures": [
                        "0x" + "77" * 64,
                        bytes(64),
                        "0x" + "99" * 64,
                    ],
                },
            }
        )


def test_derives_ton_and_substrate_source_proof_transcripts_from_witness_material() -> None:
    source_event_digest = "0x" + "34" * 32
    inclusion_branch = [HEX32_E]
    changed_branch = [HEX32_F]

    ton_input = {
        "source_event_digest": source_event_digest,
        "masterchain_seqno": "19",
        "masterchain_block_hash": HEX32_A,
        "shard_workchain_id": 0,
        "shard_shard": 0x8000000000000000,
        "shard_seqno": 7,
        "shard_block_hash": HEX32_B,
        "shard_file_hash": "0x" + "bc" * 32,
        "shard_state_root": HEX32_C,
        "transaction_root": HEX32_D,
        "transaction_lt": 7,
        "shard_state_leaf_index": "0",
        "shard_state_inclusion_branch": changed_branch,
        "inclusion_branch": inclusion_branch,
    }
    assert len(canonical_ton_sccp_shard_proof_bytes(ton_input)) == 309
    assert (
        ton_sccp_shard_proof_hash(ton_input)
        == "0x09c63ca1185b537f0a37b7b248600a0992e5b7ed64ace9d1d437db7caae00686"
    )
    with pytest.raises(TypeError, match="sourceEventDigest must not be zero"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_input, "source_event_digest": SCCP_ZERO_HASH_V1}
        )
    with pytest.raises(TypeError, match="masterchainSeqno must not use multiple aliases"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_input, "masterchainSeqno": ton_input["masterchain_seqno"]}
        )
    with pytest.raises(TypeError, match="transactionRoot must not use multiple aliases"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_input, "receiptOrMessageRoot": ton_input["transaction_root"]}
        )
    with pytest.raises(TypeError, match="inclusionBranch must not use multiple aliases"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_input, "inclusionBranch": ton_input["inclusion_branch"]}
        )
    ton_dictionary_input = {
        **ton_input,
        "shard_state_root": TON_SHARD_STATE_ROOT_HASH,
        "transaction_root": TON_HASHMAP_E_VALUE_HASH,
        "shard_state_proof_boc": TON_SHARD_STATE_PROOF_BOC,
        "shard_state_dictionary_root": TON_SHARD_ACCOUNTS_ROOT_HASH,
        "shard_state_dictionary_key_bit_len": "256",
        "shard_state_dictionary_key": TON_SHARD_ACCOUNT_KEY,
        "shard_state_dictionary_proof_boc": TON_SHARD_ACCOUNTS_BOC,
        "shard_state_inclusion_branch": [],
    }
    assert (
        ton_shard_accounts_last_transaction(TON_SHARD_ACCOUNTS_BOC, TON_SHARD_ACCOUNT_KEY, 256)
        == {"hash": TON_HASHMAP_E_VALUE_HASH, "lt": 7}
    )
    assert (
        ton_shard_accounts_last_transaction_hash(TON_SHARD_ACCOUNTS_BOC, TON_SHARD_ACCOUNT_KEY, 256)
        == TON_HASHMAP_E_VALUE_HASH
    )
    assert len(canonical_ton_sccp_shard_proof_bytes(ton_dictionary_input)) == 662
    assert (
        ton_sccp_shard_proof_hash(ton_dictionary_input)
        == "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf"
    )
    ton_shard_state_source_state_input = {
        "source_domain": SCCP_DOMAIN_TON,
        "masterchain_seqno": "19",
        "masterchain_workchain_id": -1,
        "masterchain_shard": str(0x8000_0000_0000_0000),
        "masterchain_block_hash": HEX32_A,
        "masterchain_file_hash": "0x" + "a5" * 32,
        "validator_set_hash": TON_VALIDATOR_SET_HASH,
        "masterchain_config_root": TON_MASTERCHAIN_CONFIG_ROOT,
        "masterchain_config_proof_hash": TON_SHARD_STATE_MASTERCHAIN_CONFIG_PROOF_HASH,
        "shard_workchain_id": 0,
        "shard_shard": str(0x8000_0000_0000_0000),
        "shard_seqno": "7",
        "shard_block_hash": HEX32_B,
        "shard_file_hash": "0x" + "bc" * 32,
        "shard_state_root": TON_SHARD_STATE_ROOT_HASH,
        "transaction_root": TON_HASHMAP_E_VALUE_HASH,
        "transaction_lt": "7",
        "shard_state_proof_boc": TON_SHARD_STATE_PROOF_BOC,
        "shard_state_dictionary_root": TON_SHARD_ACCOUNTS_ROOT_HASH,
        "shard_state_dictionary_key_bit_len": "256",
        "shard_state_dictionary_key": TON_SHARD_ACCOUNT_KEY,
        "shard_state_dictionary_proof_boc": TON_SHARD_ACCOUNTS_BOC,
        "masterchain_signature_hash": TON_MASTERCHAIN_SIGNATURES_HASH,
        "shard_proof_hash": "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf",
        "config_dictionary_proof_boc": TON_MASTERCHAIN_CONFIG_PROOF_BOC,
        "source_state_verifier_hash": "0x" + "d4" * 32,
        "source_trust_anchor_hash": TON_VALIDATOR_SET_HASH,
        "consensus_verifier_hash": "0x" + "b2" * 32,
        "message_inclusion_verifier_hash": "0x" + "c3" * 32,
        "finality_policy_hash": "0x" + "c4" * 32,
    }
    assert (
        len(canonical_ton_shard_state_proof_public_inputs_bytes(
            ton_shard_state_source_state_input
        ))
        == 603
    )
    assert (
        ton_shard_state_proof_public_inputs_hash(ton_shard_state_source_state_input)
        == "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19"
    )
    assert (
        len(canonical_ton_shard_state_witness_commitment_bytes(
            ton_shard_state_source_state_input
        ))
        == 480
    )
    assert (
        len(canonical_ton_shard_state_verification_context_bytes(
            ton_shard_state_source_state_input
        ))
        == 467
    )
    assert len(ton_shard_state_open_verify_schema_descriptor(
        ton_shard_state_source_state_input
    )) == 436
    request = build_ton_shard_state_proof_request(ton_shard_state_source_state_input)
    assert_immutable_fastpq_proof_request(
        request,
        (
            "statement_bytes",
            "witness_commitment_bytes",
            "verification_context_bytes",
            "schema_descriptor",
        ),
    )
    assert request["circuit_id"] == "sccp-ton-shard-state-light-client-v1"
    assert request["shard_state_proof_public_inputs_hash"] == (
        "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19"
    )
    assert request["fastpq_public_inputs"] == {
        "dsid": "0x27e44edc7d124906a8176e94557996c3",
        "slot": "19",
        "old_root": TON_MASTERCHAIN_CONFIG_ROOT,
        "new_root": TON_SHARD_STATE_ROOT_HASH,
        "perm_root": TON_SHARD_ACCOUNTS_ROOT_HASH,
        "tx_set_hash": (
            "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19"
        ),
    }
    assert [transition["key"] for transition in request["fastpq_transitions"]] == [
        "sccp:ton:shard-state:v1:statement",
        "sccp:ton:shard-state:v1:witness",
        "sccp:ton:shard-state:v1:context",
    ]
    with pytest.raises(TypeError, match="masterchainSeqno must not use multiple aliases"):
        build_ton_shard_state_proof_request(
            {
                **ton_shard_state_source_state_input,
                "masterchainSeqno": ton_shard_state_source_state_input["masterchain_seqno"],
            }
        )
    with pytest.raises(TypeError, match="shardStateDictionaryProofBoc must not use multiple aliases"):
        canonical_ton_shard_state_proof_public_inputs_bytes(
            {
                **ton_shard_state_source_state_input,
                "shardStateDictionaryProofBoc": ton_shard_state_source_state_input[
                    "shard_state_dictionary_proof_boc"
                ],
            }
        )
    with pytest.raises(TypeError, match="configDictionaryProofBoc must not use multiple aliases"):
        build_ton_shard_state_proof_request(
            {
                **ton_shard_state_source_state_input,
                "masterchainConfigProof": {
                    "configDictionaryProofBoc": ton_shard_state_source_state_input[
                        "config_dictionary_proof_boc"
                    ],
                },
            }
        )
    with pytest.raises(TypeError, match="sourceStateVerifierHash must not use multiple aliases"):
        canonical_ton_shard_state_witness_commitment_bytes(
            {
                **ton_shard_state_source_state_input,
                "sourceStateVerifierHash": ton_shard_state_source_state_input[
                    "source_state_verifier_hash"
                ],
            }
        )
    ton_transition_proof = {
        "version": 1,
        "source_domain": SCCP_DOMAIN_TON,
        "from_validator_set_seqno": "7",
        "to_validator_set_seqno": "8",
        "masterchain_seqno": "19",
        "masterchain_workchain_id": -1,
        "masterchain_shard": str(0x8000_0000_0000_0000),
        "masterchain_block_hash": HEX32_A,
        "masterchain_file_hash": "0x" + "a5" * 32,
        "parent_validator_set_hash": TON_VALIDATOR_SET_HASH,
        "next_validator_set_hash": TON_NEXT_VALIDATOR_SET_HASH,
        "next_validator_set_payload": bytes.fromhex(TON_NEXT_VALIDATOR_SET_PAYLOAD_HEX),
        "next_validator_set_payload_hash": TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH,
        "next_validator_set_config_hash": HEX32_C,
        "transition_message_hash": TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
        "transition_signature_hash": TON_VALIDATOR_SET_TRANSITION_SIGNATURE_HASH,
        "validator_signature_proof": {
            "version": 1,
            "total_weight": "3",
            "signed_weight": "3",
            "block_message_hash": TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
            "validator_public_keys": ["0x" + "11" * 32, "0x" + "22" * 32],
            "validator_weights": ["1", "2"],
            "signers_bitmap": b"\x03",
            "signatures": [bytes([0xAB]) * 64, bytes([0xCD]) * 64],
        },
    }
    transition_bound_input = {
        **ton_shard_state_source_state_input,
        "validator_set_transition_proofs": [ton_transition_proof],
    }
    tampered_transition_proof = {
        **ton_transition_proof,
        "validator_signature_proof": {
            **ton_transition_proof["validator_signature_proof"],
            "signatures": [bytes([0xAA]) + bytes([0xAB]) * 63, bytes([0xCD]) * 64],
        },
    }
    with pytest.raises(TypeError, match="transitionSignatureHash"):
        canonical_ton_shard_state_proof_public_inputs_bytes(
            {
                **transition_bound_input,
                "validator_set_transition_proofs": [tampered_transition_proof],
            }
        )
    with pytest.raises(TypeError, match="transitionSignatureHash must not use multiple aliases"):
        build_ton_shard_state_proof_request(
            {
                **transition_bound_input,
                "validator_set_transition_proofs": [
                    {
                        **ton_transition_proof,
                        "transitionSignatureHash": TON_VALIDATOR_SET_TRANSITION_SIGNATURE_HASH,
                    }
                ],
            }
        )
    with pytest.raises(TypeError, match="TON template verifier hash"):
        build_ton_shard_state_proof_request(
            {
                **ton_shard_state_source_state_input,
                "source_state_verifier_hash": TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH,
            }
        )
    assert ton_shard_state_public_input_columns(ton_shard_state_source_state_input)[15] == [
        "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19"
    ]
    with pytest.raises(TypeError, match="last transaction hash"):
        ton_shard_state_proof_public_inputs_hash(
            {**ton_shard_state_source_state_input, "transaction_root": HEX32_C}
        )
    with pytest.raises(TypeError, match="must be empty"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_dictionary_input, "shard_state_inclusion_branch": [HEX32_F]}
        )
    with pytest.raises(TypeError, match="must not be empty"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_dictionary_input, "shard_state_proof_boc": b""}
        )
    with pytest.raises(TypeError, match="root must match shardStateRoot"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_dictionary_input, "shard_state_root": "0x" + "66" * 32}
        )
    with pytest.raises(
        TypeError, match="ShardAccount last transaction hash must match transactionRoot"
    ):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_dictionary_input, "transaction_root": "0x" + "66" * 32}
        )
    with pytest.raises(
        TypeError, match="ShardAccount last transaction lt must match transactionLt"
    ):
        canonical_ton_sccp_shard_proof_bytes({**ton_dictionary_input, "transaction_lt": 8})
    with pytest.raises(TypeError, match="accounts root must match"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_dictionary_input, "shard_state_dictionary_root": "0x" + "66" * 32}
        )
    with pytest.raises(TypeError, match="must not be zero"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_dictionary_input, "shard_state_dictionary_root": "0x" + "00" * 32}
        )
    wrong_global_id_proof_boc = bytearray(TON_SHARD_STATE_PROOF_BOC)
    wrong_global_id_tag_offset = wrong_global_id_proof_boc.find(bytes([0x90, 0x23, 0xAF, 0xE2]))
    assert wrong_global_id_tag_offset >= 0
    wrong_global_id_proof_boc[wrong_global_id_tag_offset + 4 : wrong_global_id_tag_offset + 8] = b"\0\0\0\0"
    wrong_global_id_proof_boc = bytes(wrong_global_id_proof_boc)
    assert ton_shard_state_accounts_root_hash(wrong_global_id_proof_boc) == TON_SHARD_ACCOUNTS_ROOT_HASH
    with pytest.raises(TypeError, match="global_id"):
        canonical_ton_sccp_shard_proof_bytes(
            {
                **ton_dictionary_input,
                "shard_state_root": ton_shard_state_proof_root_hash(wrong_global_id_proof_boc),
                "shard_state_proof_boc": wrong_global_id_proof_boc,
            }
        )
    wrong_workchain_id_proof_boc = bytearray(TON_SHARD_STATE_PROOF_BOC)
    wrong_workchain_id_tag_offset = wrong_workchain_id_proof_boc.find(
        bytes([0x90, 0x23, 0xAF, 0xE2])
    )
    assert wrong_workchain_id_tag_offset >= 0
    wrong_workchain_shard_ident_offset = wrong_workchain_id_tag_offset + 8
    wrong_workchain_id_proof_boc[
        wrong_workchain_shard_ident_offset + 1 : wrong_workchain_shard_ident_offset + 5
    ] = b"\xff\xff\xff\xff"
    wrong_workchain_id_proof_boc = bytes(wrong_workchain_id_proof_boc)
    assert (
        ton_shard_state_accounts_root_hash(wrong_workchain_id_proof_boc)
        == TON_SHARD_ACCOUNTS_ROOT_HASH
    )
    with pytest.raises(TypeError, match="workchain_id"):
        canonical_ton_sccp_shard_proof_bytes(
            {
                **ton_dictionary_input,
                "shard_state_root": ton_shard_state_proof_root_hash(
                    wrong_workchain_id_proof_boc
                ),
                "shard_state_proof_boc": wrong_workchain_id_proof_boc,
            }
        )
    zero_gen_utime_proof_boc = bytearray(TON_SHARD_STATE_PROOF_BOC)
    zero_gen_utime_tag_offset = zero_gen_utime_proof_boc.find(bytes([0x90, 0x23, 0xAF, 0xE2]))
    assert zero_gen_utime_tag_offset >= 0
    zero_gen_utime_proof_boc[zero_gen_utime_tag_offset + 29 : zero_gen_utime_tag_offset + 33] = (
        b"\0\0\0\0"
    )
    zero_gen_utime_proof_boc = bytes(zero_gen_utime_proof_boc)
    with pytest.raises(TypeError, match="gen_utime"):
        canonical_ton_sccp_shard_proof_bytes(
            {
                **ton_dictionary_input,
                "shard_state_root": ton_shard_state_proof_root_hash(
                    zero_gen_utime_proof_boc
                ),
                "shard_state_proof_boc": zero_gen_utime_proof_boc,
            }
        )
    future_min_ref_mc_seqno_proof_boc = bytearray(TON_SHARD_STATE_PROOF_BOC)
    future_min_ref_mc_seqno_tag_offset = future_min_ref_mc_seqno_proof_boc.find(
        bytes([0x90, 0x23, 0xAF, 0xE2])
    )
    assert future_min_ref_mc_seqno_tag_offset >= 0
    future_min_ref_mc_seqno_proof_boc[future_min_ref_mc_seqno_tag_offset + 44] = 0x14
    future_min_ref_mc_seqno_proof_boc = bytes(future_min_ref_mc_seqno_proof_boc)
    with pytest.raises(TypeError, match="min_ref_mc_seqno"):
        canonical_ton_sccp_shard_proof_bytes(
            {
                **ton_dictionary_input,
                "shard_state_root": ton_shard_state_proof_root_hash(
                    future_min_ref_mc_seqno_proof_boc
                ),
                "shard_state_proof_boc": future_min_ref_mc_seqno_proof_boc,
            }
        )
    mismatched_shard_prefix_proof_boc = bytearray(TON_SHARD_STATE_PROOF_BOC)
    shard_state_tag_offset = mismatched_shard_prefix_proof_boc.find(bytes([0x90, 0x23, 0xAF, 0xE2]))
    assert shard_state_tag_offset >= 0
    shard_ident_offset = shard_state_tag_offset + 8
    mismatched_shard_prefix_proof_boc[shard_ident_offset] = 0x08
    mismatched_shard_prefix_proof_boc[shard_ident_offset + 5] = 0x12
    mismatched_shard_prefix_proof_boc = bytes(mismatched_shard_prefix_proof_boc)
    assert (
        ton_shard_state_accounts_root_hash(mismatched_shard_prefix_proof_boc)
        == TON_SHARD_ACCOUNTS_ROOT_HASH
    )
    with pytest.raises(TypeError, match="ShardIdent prefix"):
        canonical_ton_sccp_shard_proof_bytes(
            {
                **ton_dictionary_input,
                "shard_state_root": ton_shard_state_proof_root_hash(
                    mismatched_shard_prefix_proof_boc
                ),
                "shard_state_proof_boc": mismatched_shard_prefix_proof_boc,
                "shard_shard": 0x1280000000000000,
            }
        )
    with pytest.raises(TypeError, match="key bit length must be 256"):
        canonical_ton_sccp_shard_proof_bytes(
            {
                **ton_dictionary_input,
                "shard_state_dictionary_key_bit_len": "7",
                "shard_state_dictionary_key": bytes([17]),
            }
        )
    with pytest.raises(ValueError, match="at most 64"):
        canonical_ton_sccp_shard_proof_bytes(
            {**ton_input, "inclusion_branch": [HEX32_E] * 65}
        )
    ton_validator_set = {
        "validator_public_keys": ["0x" + "11" * 32, "0x" + "22" * 32],
        "validator_weights": ["1", "2"],
    }
    ton_next_validator_set = {
        "validator_public_keys": ["0x" + "33" * 32, "0x" + "44" * 32],
        "validator_weights": ["3", "4"],
    }
    ton_next_validator_set_payload = canonical_ton_validator_set_payload_bytes(
        ton_next_validator_set
    )
    ton_transition_message = {
        "source_domain": SCCP_DOMAIN_TON,
        "from_validator_set_seqno": "7",
        "to_validator_set_seqno": "8",
        "masterchain_seqno": "19",
        "masterchain_workchain_id": -1,
        "masterchain_shard": str(0x8000_0000_0000_0000),
        "masterchain_block_hash": HEX32_A,
        "masterchain_file_hash": "0x" + "a5" * 32,
        "parent_validator_set_hash": TON_VALIDATOR_SET_HASH,
        "next_validator_set_hash": TON_NEXT_VALIDATOR_SET_HASH,
        "next_validator_set_payload": ton_next_validator_set_payload,
        "next_validator_set_payload_hash": TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH,
        "next_validator_set_config_hash": HEX32_C,
    }
    ton_transition_signature = {
        **ton_transition_message,
        "version": 1,
        "transition_message_hash": TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
        "validator_signature_proof": {
            "version": 1,
            "total_weight": "3",
            "signed_weight": "3",
            "block_message_hash": TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
            **ton_validator_set,
            "signers_bitmap": b"\x03",
            "signatures": [bytes([0xAB]) * 64, bytes([0xCD]) * 64],
        },
    }
    assert len(canonical_ton_validator_set_bytes(ton_validator_set)) == 85
    assert ton_validator_set_hash(ton_validator_set) == TON_VALIDATOR_SET_HASH
    assert ton_next_validator_set_payload.hex() == TON_NEXT_VALIDATOR_SET_PAYLOAD_HEX
    assert (
        ton_validator_set_payload_hash(ton_next_validator_set_payload)
        == TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH
    )
    assert (
        ton_validator_set_hash_from_payload(ton_next_validator_set_payload)
        == TON_NEXT_VALIDATOR_SET_HASH
    )
    assert len(canonical_ton_validator_set_transition_message_bytes(ton_transition_message)) == 233
    assert (
        ton_validator_set_transition_message_hash(ton_transition_message)
        == TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH
    )
    assert (
        len(canonical_ton_validator_set_transition_signature_bytes(ton_transition_signature))
        == 676
    )
    assert (
        ton_validator_set_transition_signature_hash(ton_transition_signature)
        == TON_VALIDATOR_SET_TRANSITION_SIGNATURE_HASH
    )
    with pytest.raises(TypeError, match="validatorPublicKeys must not use multiple aliases"):
        canonical_ton_validator_set_bytes(
            {
                **ton_validator_set,
                "validatorPublicKeys": ton_validator_set["validator_public_keys"],
            }
        )
    with pytest.raises(TypeError, match="masterchainSeqno must not use multiple aliases"):
        canonical_ton_validator_set_transition_message_bytes(
            {
                **ton_transition_message,
                "masterchainSeqno": ton_transition_message["masterchain_seqno"],
            }
        )
    with pytest.raises(TypeError, match="nextValidatorSetPayloadHash must not use multiple aliases"):
        canonical_ton_validator_set_transition_signature_bytes(
            {
                **ton_transition_signature,
                "nextValidatorSetPayloadHash": TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH,
            }
        )
    with pytest.raises(TypeError, match="totalWeight must not use multiple aliases"):
        canonical_ton_validator_set_transition_signature_bytes(
            {
                **ton_transition_signature,
                "validator_signature_proof": {
                    **ton_transition_signature["validator_signature_proof"],
                    "totalWeight": "3",
                },
            }
        )
    with pytest.raises(TypeError, match="TON validator-set transition version"):
        canonical_ton_validator_set_transition_message_bytes(
            {**ton_transition_message, "version": 0}
        )
    with pytest.raises(TypeError, match="TON validator-set transition proof version"):
        canonical_ton_validator_set_transition_signature_bytes(
            {**ton_transition_signature, "version": 0}
        )
    with pytest.raises(TypeError, match="TON validator signature proof version"):
        canonical_ton_validator_set_transition_signature_bytes(
            {
                **ton_transition_signature,
                "validator_signature_proof": {
                    **ton_transition_signature["validator_signature_proof"],
                    "version": 0,
                },
            }
        )
    assert ton_validator_set_transition_message_hash(
        {**ton_transition_message, "next_validator_set_config_hash": HEX32_D}
    ) != TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH
    with pytest.raises(TypeError, match="parentValidatorSetHash"):
        canonical_ton_validator_set_transition_signature_bytes(
            {**ton_transition_signature, "parent_validator_set_hash": HEX32_D}
        )
    with pytest.raises(TypeError, match="transitionMessageHash"):
        canonical_ton_validator_set_transition_signature_bytes(
            {**ton_transition_signature, "transition_message_hash": HEX32_D}
        )
    with pytest.raises(TypeError, match="toValidatorSetSeqno"):
        canonical_ton_validator_set_transition_message_bytes(
            {**ton_transition_message, "to_validator_set_seqno": "9"}
        )
    with pytest.raises(TypeError, match="blockMessageHash"):
        canonical_ton_validator_set_transition_signature_bytes(
            {
                **ton_transition_signature,
                "validator_signature_proof": {
                    **ton_transition_signature["validator_signature_proof"],
                    "block_message_hash": HEX32_D,
                },
            }
        )
    with pytest.raises(ValueError, match="must not be zero"):
        canonical_ton_validator_set_bytes(
            {**ton_validator_set, "validator_weights": ["1", "0"]}
        )
    with pytest.raises(ValueError, match="must not be zero"):
        canonical_ton_validator_set_bytes(
            {
                **ton_validator_set,
                "validator_public_keys": [bytes(32), ton_validator_set["validator_public_keys"][1]],
            }
        )
    zero_key_validator_set_payload = bytearray(
        canonical_ton_validator_set_payload_bytes(ton_validator_set)
    )
    zero_key_validator_set_payload[5:37] = bytes(32)
    with pytest.raises(ValueError, match="must not be zero"):
        ton_validator_set_hash_from_payload(bytes(zero_key_validator_set_payload))
    oversized_validator_keys = [
        bytes([0x80]) + bytes(27) + index.to_bytes(4, "little")
        for index in range(1025)
    ]
    oversized_validator_set = {
        "validator_public_keys": oversized_validator_keys,
        "validator_weights": ["1"] * len(oversized_validator_keys),
    }
    with pytest.raises(ValueError, match="1..1024"):
        canonical_ton_validator_set_bytes(oversized_validator_set)
    oversized_validator_set_payload = bytearray([1])
    oversized_validator_set_payload.extend((1025).to_bytes(4, "little"))
    for public_key in oversized_validator_keys:
        oversized_validator_set_payload.extend(public_key)
        oversized_validator_set_payload.extend((1).to_bytes(8, "little"))
    with pytest.raises(ValueError, match="validator count"):
        ton_validator_set_hash_from_payload(bytes(oversized_validator_set_payload))
    with pytest.raises(TypeError, match="signatures length"):
        canonical_ton_validator_set_transition_signature_bytes(
            {
                **ton_transition_signature,
                "validator_signature_proof": {
                    **ton_transition_signature["validator_signature_proof"],
                    "signatures": [bytes(64)],
                },
            }
        )
    with pytest.raises(TypeError, match="greater than two thirds"):
        canonical_ton_validator_set_transition_signature_bytes(
            {
                **ton_transition_signature,
                "validator_signature_proof": {
                    **ton_transition_signature["validator_signature_proof"],
                    "signed_weight": "1",
                    "signers_bitmap": b"\x01",
                    "signatures": [bytes(64)],
                },
            }
        )
    with pytest.raises(TypeError, match="64 bytes"):
        canonical_ton_validator_set_transition_signature_bytes(
            {
                **ton_transition_signature,
                "validator_signature_proof": {
                    **ton_transition_signature["validator_signature_proof"],
                    "signatures": [bytes(63), bytes(64)],
                },
            }
        )
    with pytest.raises(TypeError, match="must not be all zero"):
        canonical_ton_validator_set_transition_signature_bytes(
            {
                **ton_transition_signature,
                "validator_signature_proof": {
                    **ton_transition_signature["validator_signature_proof"],
                    "signatures": [bytes(64), b"\x01" * 64],
                },
            }
        )
    with pytest.raises(TypeError, match="nextValidatorSetHash"):
        canonical_ton_validator_set_transition_signature_bytes(
            {**ton_transition_signature, "next_validator_set_hash": HEX32_B}
        )

    ton_validator_set_payload = canonical_ton_validator_set_payload_bytes(
        ton_validator_set
    )
    ton_config_leaf = {
        "source_domain": SCCP_DOMAIN_TON,
        "masterchain_seqno": "19",
        "masterchain_workchain_id": -1,
        "masterchain_shard": str(0x8000_0000_0000_0000),
        "masterchain_block_hash": HEX32_A,
        "masterchain_file_hash": "0x" + "a5" * 32,
        "shard_state_root": HEX32_C,
        "validator_set_hash": TON_VALIDATOR_SET_HASH,
        "validator_set_payload_hash": TON_VALIDATOR_SET_PAYLOAD_HASH,
    }
    ton_config_proof = {
        **ton_config_leaf,
        "config_root": TON_MASTERCHAIN_CONFIG_ROOT,
        "config_leaf_hash": TON_MASTERCHAIN_CONFIG_LEAF_HASH,
        "config_leaf_index": str(SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM),
        "config_value_hash": TON_MASTERCHAIN_CONFIG_VALUE_HASH,
        "config_dictionary_proof_boc": TON_MASTERCHAIN_CONFIG_PROOF_BOC,
        "config_inclusion_branch": [],
    }
    assert (
        ton_validator_set_payload_hash(ton_validator_set_payload)
        == TON_VALIDATOR_SET_PAYLOAD_HASH
    )
    assert (
        ton_config_validator_set_payload_from_proof_boc(TON_MASTERCHAIN_CONFIG_PROOF_BOC)
        == ton_validator_set_payload
    )
    assert (
        ton_config_validator_set_payload_hash_from_proof_boc(TON_MASTERCHAIN_CONFIG_PROOF_BOC)
        == TON_VALIDATOR_SET_PAYLOAD_HASH
    )
    assert ton_hashmap_e_proof_root_hash(TON_MASTERCHAIN_CONFIG_PROOF_BOC) == TON_MASTERCHAIN_CONFIG_ROOT
    assert (
        ton_hashmap_e_cell_ref_value_hash(
            TON_MASTERCHAIN_CONFIG_PROOF_BOC,
            int(SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM).to_bytes(4, "big"),
            SCCP_TON_CONFIG_PARAM_KEY_BITS,
        )
        == TON_MASTERCHAIN_CONFIG_VALUE_HASH
    )
    assert len(canonical_ton_masterchain_config_leaf_bytes(ton_config_leaf)) == 141
    with pytest.raises(TypeError, match="TON masterchain config leaf version"):
        canonical_ton_masterchain_config_leaf_bytes({**ton_config_leaf, "version": 0})
    assert (
        ton_masterchain_config_leaf_hash(ton_config_leaf)
        == TON_MASTERCHAIN_CONFIG_LEAF_HASH
    )
    assert len(canonical_ton_masterchain_config_proof_bytes(ton_config_proof)) == 411
    with pytest.raises(TypeError, match="TON masterchain config proof version"):
        canonical_ton_masterchain_config_proof_bytes({**ton_config_proof, "version": 0})
    assert (
        ton_masterchain_config_proof_hash(ton_config_proof)
        == TON_MASTERCHAIN_CONFIG_PROOF_HASH
    )
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        canonical_ton_masterchain_config_leaf_bytes(
            {**ton_config_leaf, "sourceDomain": SCCP_DOMAIN_TON}
        )
    with pytest.raises(TypeError, match="configRoot must not use multiple aliases"):
        canonical_ton_masterchain_config_proof_bytes(
            {**ton_config_proof, "configRoot": TON_MASTERCHAIN_CONFIG_ROOT}
        )
    with pytest.raises(TypeError, match="must be empty"):
        canonical_ton_masterchain_config_proof_bytes(
            {**ton_config_proof, "config_inclusion_branch": [bytes([0xEE]) * 32]}
        )
    with pytest.raises(TypeError, match="ValidatorSet"):
        canonical_ton_masterchain_config_proof_bytes(
            {**ton_config_proof, "validator_set_payload_hash": HEX32_E}
        )
    with pytest.raises(TypeError, match="configLeafHash"):
        canonical_ton_masterchain_config_proof_bytes(
            {**ton_config_proof, "config_leaf_hash": HEX32_E}
        )
    with pytest.raises(TypeError, match="validatorSetHash"):
        canonical_ton_masterchain_config_proof_bytes(
            {
                **ton_config_proof,
                "validator_set_hash": HEX32_E,
                "config_leaf_hash": ton_masterchain_config_leaf_hash(
                    {**ton_config_leaf, "validator_set_hash": HEX32_E}
                ),
            }
        )
    with pytest.raises(TypeError, match="sourceDomain"):
        canonical_ton_masterchain_config_proof_bytes(
            {
                **ton_config_proof,
                "source_domain": SCCP_DOMAIN_SOL,
                "config_leaf_hash": ton_masterchain_config_leaf_hash(
                    {**ton_config_leaf, "source_domain": SCCP_DOMAIN_SOL}
                ),
            }
        )
    ton_block_message = {
        "source_domain": SCCP_DOMAIN_TON,
        "masterchain_seqno": "19",
        "masterchain_workchain_id": -1,
        "masterchain_shard": str(0x8000_0000_0000_0000),
        "masterchain_block_hash": HEX32_A,
        "masterchain_file_hash": "0x" + "a5" * 32,
        "validator_set_hash": TON_VALIDATOR_SET_HASH,
        "masterchain_config_root": TON_MASTERCHAIN_CONFIG_ROOT,
        "masterchain_config_proof_hash": TON_MASTERCHAIN_CONFIG_PROOF_HASH,
        "shard_workchain_id": 0,
        "shard_shard": str(0x8000_0000_0000_0000),
        "shard_seqno": "7",
        "shard_block_hash": HEX32_B,
        "shard_file_hash": "0x" + "bc" * 32,
        "shard_state_root": HEX32_C,
        "transaction_root": HEX32_D,
        "shard_proof_hash": HEX32_E,
    }
    assert len(canonical_ton_masterchain_block_message_bytes(ton_block_message)) == 365
    assert (
        ton_masterchain_block_message_hash(ton_block_message)
        == TON_MASTERCHAIN_BLOCK_MESSAGE_HASH
    )
    ton_masterchain_signatures = {
        "version": 1,
        "total_weight": "3",
        "signed_weight": "3",
        "block_message_hash": TON_MASTERCHAIN_BLOCK_MESSAGE_HASH,
        **ton_validator_set,
        "validator_set_hash": TON_VALIDATOR_SET_HASH,
        "signers_bitmap": b"\x03",
        "signatures": [bytes([0xAB]) * 64, bytes([0xCD]) * 64],
    }
    assert (
        len(canonical_ton_masterchain_validator_signatures_bytes(ton_masterchain_signatures))
        == 322
    )
    assert (
        ton_masterchain_validator_signatures_hash(ton_masterchain_signatures)
        == TON_MASTERCHAIN_SIGNATURES_HASH
    )
    with pytest.raises(TypeError, match="shardProofHash must not use multiple aliases"):
        canonical_ton_masterchain_block_message_bytes(
            {**ton_block_message, "shardProofHash": HEX32_E}
        )
    with pytest.raises(TypeError, match="validatorSetHash must not use multiple aliases"):
        canonical_ton_masterchain_validator_signatures_bytes(
            {**ton_masterchain_signatures, "validatorSetHash": TON_VALIDATOR_SET_HASH}
        )
    with pytest.raises(TypeError, match="masterchainWorkchainId"):
        canonical_ton_masterchain_block_message_bytes(
            {**ton_block_message, "masterchain_workchain_id": 0}
        )
    with pytest.raises(TypeError, match="masterchainShard"):
        canonical_ton_masterchain_block_message_bytes(
            {**ton_block_message, "masterchain_shard": "0"}
        )
    with pytest.raises(TypeError, match="masterchainFileHash"):
        canonical_ton_masterchain_block_message_bytes(
            {**ton_block_message, "masterchain_file_hash": "0x" + "00" * 32}
        )
    with pytest.raises(TypeError, match="shardWorkchainId"):
        canonical_ton_masterchain_block_message_bytes(
            {**ton_block_message, "shard_workchain_id": -1}
        )
    with pytest.raises(TypeError, match="shardSeqno"):
        canonical_ton_masterchain_block_message_bytes(
            {**ton_block_message, "shard_seqno": "0"}
        )
    with pytest.raises(TypeError, match="shardFileHash"):
        canonical_ton_masterchain_block_message_bytes(
            {**ton_block_message, "shard_file_hash": "0x" + "00" * 32}
        )
    assert ton_boc_root_hashes(TON_ORDINARY_BOC) == [TON_ORDINARY_BOC_ROOT_HASH]
    assert ton_boc_single_root_hash(TON_ORDINARY_BOC) == TON_ORDINARY_BOC_ROOT_HASH
    assert ton_boc_single_root_hash(TON_ORDINARY_BOC_CRC) == TON_ORDINARY_BOC_ROOT_HASH
    bad_crc = bytearray(TON_ORDINARY_BOC_CRC)
    bad_crc[-1] ^= 1
    with pytest.raises(TypeError, match="CRC32C"):
        ton_boc_single_root_hash(bad_crc)
    changed_child = bytearray(TON_ORDINARY_BOC)
    changed_child[-1] ^= 1
    assert ton_boc_single_root_hash(changed_child) != TON_ORDINARY_BOC_ROOT_HASH
    cyclic_ref = bytearray(TON_ORDINARY_BOC)
    cyclic_ref[14] = 0
    with pytest.raises(TypeError, match="forward internal refs"):
        ton_boc_single_root_hash(cyclic_ref)
    exotic_cell = bytearray(TON_ORDINARY_BOC)
    exotic_cell[11] |= 0x08
    with pytest.raises(TypeError, match="pruned branch"):
        ton_boc_single_root_hash(exotic_cell)
    invalid_partial_data = bytearray(TON_ORDINARY_BOC)
    invalid_partial_data[16] = 1
    invalid_partial_data[17] = 0
    with pytest.raises(TypeError, match="padding"):
        ton_boc_single_root_hash(invalid_partial_data)
    assert ton_boc_single_root_hash(TON_PRUNED_BRANCH_BOC) == TON_PRUNED_BRANCH_ROOT_HASH
    assert ton_boc_single_root_hash(TON_LEGACY_PRUNED_PROOF_BOC) == TON_LEGACY_PRUNED_PROOF_ROOT_HASH
    assert ton_boc_single_root_hash(TON_MERKLE_PROOF_BOC) == TON_MERKLE_PROOF_ROOT_HASH
    mismatched_merkle_proof = bytearray(TON_MERKLE_PROOF_BOC)
    mismatched_merkle_proof[14] ^= 1
    with pytest.raises(TypeError, match="Merkle proof"):
        ton_boc_single_root_hash(mismatched_merkle_proof)
    assert (
        ton_hashmap_e_cell_ref_value_hash(TON_HASHMAP_E_CELL_REF_BOC, bytes([17]), 8)
        == TON_HASHMAP_E_VALUE_HASH
    )
    assert ton_hashmap_e_cell_ref_value_hash(TON_HASHMAP_E_CELL_REF_BOC, bytes([18]), 8) is None
    with pytest.raises(TypeError, match="key length"):
        ton_hashmap_e_cell_ref_value_hash(TON_HASHMAP_E_CELL_REF_BOC, bytes([17]), 7)
    assert (
        ton_hashmap_e_cell_ref_value_hash(TON_HASHMAP_E_DIRECT_PROOF_BOC, bytes([17]), 8)
        == TON_HASHMAP_E_VALUE_HASH
    )
    assert ton_hashmap_e_cell_ref_value_hash(TON_HASHMAP_E_DIRECT_PROOF_BOC, bytes([1]), 8) is None
    assert (
        ton_hashmap_e_cell_ref_value_hash(TON_HASHMAP_E_MERKLE_PROOF_BOC, bytes([17]), 8)
        == TON_HASHMAP_E_VALUE_HASH
    )
    assert ton_shard_state_proof_root_hash(TON_SHARD_STATE_PROOF_BOC) == TON_SHARD_STATE_ROOT_HASH
    assert ton_shard_state_accounts_root_hash(TON_SHARD_STATE_PROOF_BOC) == TON_SHARD_ACCOUNTS_ROOT_HASH
    bad_shard_state_tag = bytearray(TON_SHARD_STATE_PROOF_BOC)
    tag_offset = bad_shard_state_tag.find(bytes.fromhex("9023afe2"))
    assert tag_offset != -1
    bad_shard_state_tag[tag_offset] ^= 1
    with pytest.raises(TypeError, match="ShardStateUnsplit"):
        ton_shard_state_accounts_root_hash(bad_shard_state_tag)
    shard_ident_offset = tag_offset + 8
    bad_shard_ident_tag = bytearray(TON_SHARD_STATE_PROOF_BOC)
    bad_shard_ident_tag[shard_ident_offset] |= 0x80
    with pytest.raises(TypeError, match="ShardIdent"):
        ton_shard_state_accounts_root_hash(bad_shard_ident_tag)
    bad_shard_ident_prefix_len = bytearray(TON_SHARD_STATE_PROOF_BOC)
    bad_shard_ident_prefix_len[shard_ident_offset] = 0x3D
    with pytest.raises(TypeError, match="ShardIdent"):
        ton_shard_state_accounts_root_hash(bad_shard_ident_prefix_len)
    basechain_custom = bytearray(TON_SHARD_STATE_PROOF_BOC)
    basechain_custom[tag_offset + 45] |= 0x40
    with pytest.raises(TypeError, match="custom"):
        ton_shard_state_accounts_root_hash(basechain_custom)
    assert (
        ton_masterchain_block_message_hash({**ton_block_message, "shard_proof_hash": HEX32_F})
        != TON_MASTERCHAIN_BLOCK_MESSAGE_HASH
    )
    with pytest.raises(TypeError, match="validatorSetHash"):
        canonical_ton_masterchain_validator_signatures_bytes(
            {**ton_masterchain_signatures, "validator_set_hash": HEX32_B}
        )
    with pytest.raises(TypeError, match="must not be all zero"):
        canonical_ton_masterchain_validator_signatures_bytes(
            {
                **ton_masterchain_signatures,
                "signatures": [bytes(64), b"\x01" * 64],
            }
        )
    with pytest.raises(ValueError, match="config param 34"):
        ton_masterchain_config_proof_hash({**ton_config_proof, "config_leaf_index": "0"})
    with pytest.raises(TypeError, match="value does not match"):
        canonical_ton_masterchain_config_proof_bytes(
            {**ton_config_proof, "config_value_hash": HEX32_E}
        )

    tron_input = {
        "source_event_digest": source_event_digest,
        "receipt_root": HEX32_B,
        "transaction_root": HEX32_D,
        "inclusion_branch": inclusion_branch,
    }
    assert canonical_evm_receipt_root_mpt_value(HEX32_B).hex() == EVM_RECEIPT_ROOT_MPT_VALUE_HEX
    with pytest.raises(TypeError, match="32 bytes"):
        canonical_evm_receipt_root_mpt_value("0x1234")
    assert canonical_tron_receipt_root_mpt_value(HEX32_B).hex() == TRON_RECEIPT_ROOT_MPT_VALUE_HEX
    with pytest.raises(TypeError, match="32 bytes"):
        canonical_tron_receipt_root_mpt_value("0x1234")
    with pytest.raises(TypeError, match="must not be zero"):
        canonical_tron_receipt_root_mpt_value(SCCP_ZERO_HASH_V1)
    assert len(canonical_tron_sccp_receipt_proof_bytes(tron_input)) == 133
    for patch, pattern in [
        ({"sourceEventDigest": source_event_digest}, "sourceEventDigest must not use multiple aliases"),
        ({"receiptOrMessageRoot": HEX32_B}, "receiptRoot must not use multiple aliases"),
        ({"transactionRoot": HEX32_D}, "transactionRoot must not use multiple aliases"),
        ({"inclusionBranch": inclusion_branch}, "inclusionBranch must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_sccp_receipt_proof_bytes({**tron_input, **patch})
    for field_name in ("source_event_digest", "receipt_root", "transaction_root"):
        with pytest.raises(TypeError, match="must not be zero"):
            canonical_tron_sccp_receipt_proof_bytes(
                {**tron_input, field_name: SCCP_ZERO_HASH_V1}
            )
    with pytest.raises(TypeError, match="sourceEventDigest must be canonical hex"):
        canonical_tron_sccp_receipt_proof_bytes(
            {**tron_input, "source_event_digest": source_event_digest + "\n"}
        )
    with pytest.raises(ValueError, match="inclusionBranch must not be empty"):
        canonical_tron_sccp_receipt_proof_bytes(
            {**tron_input, "inclusion_branch": []}
        )
    assert (
        tron_sccp_receipt_proof_hash(tron_input)
        == "0xd806aff1c058f8d1ca18b6106a5fd54b54557edc24127a0658d0ef62057e7ee5"
    )
    assert tron_sccp_receipt_proof_hash(tron_input) != tron_sccp_receipt_proof_hash(
        {**tron_input, "inclusion_branch": changed_branch}
    )
    tron_receipt_state_input = {
        "source_event_digest": source_event_digest,
        "receipt_root": HEX32_B,
        "transaction_root": TRON_RECEIPT_STATE_TRANSACTION_ROOT,
        "receipt_root_index": "0",
        "receipt_trie_proof_nodes": [TRON_RECEIPT_STATE_MPT_NODE_HEX],
        "inclusion_branch": inclusion_branch,
    }
    assert len(canonical_tron_sccp_receipt_state_proof_bytes(tron_receipt_state_input)) == 186
    for patch, pattern in [
        ({"sourceEventDigest": source_event_digest}, "sourceEventDigest must not use multiple aliases"),
        ({"receiptOrMessageRoot": HEX32_B}, "receiptRoot must not use multiple aliases"),
        ({"transactionRoot": TRON_RECEIPT_STATE_TRANSACTION_ROOT}, "transactionRoot must not use multiple aliases"),
        ({"receiptRootIndex": "0"}, "receiptRootIndex must not use multiple aliases"),
        (
            {"receiptTrieProofNodes": [TRON_RECEIPT_STATE_MPT_NODE_HEX]},
            "receiptTrieProofNodes must not use multiple aliases",
        ),
        ({"inclusionBranch": inclusion_branch}, "inclusionBranch must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_sccp_receipt_state_proof_bytes(
                {**tron_receipt_state_input, **patch}
            )
    for field_name in ("source_event_digest", "receipt_root", "transaction_root"):
        with pytest.raises(TypeError, match="must not be zero"):
            canonical_tron_sccp_receipt_state_proof_bytes(
                {**tron_receipt_state_input, field_name: SCCP_ZERO_HASH_V1}
            )
    with pytest.raises(ValueError, match="inclusionBranch must not be empty"):
        canonical_tron_sccp_receipt_state_proof_bytes(
            {**tron_receipt_state_input, "inclusion_branch": []}
        )
    assert (
        tron_sccp_receipt_state_proof_hash(tron_receipt_state_input)
        == TRON_RECEIPT_STATE_PROOF_HASH
    )
    assert tron_sccp_receipt_state_proof_hash(tron_receipt_state_input) != (
        tron_sccp_receipt_state_proof_hash(
            {**tron_receipt_state_input, "receipt_root_index": "1"}
        )
    )
    tron_transaction_source_input = {
        "source_event_digest": source_event_digest,
        "receipt_root": HEX32_B,
        "transaction_root": TRON_TRANSACTION_SOURCE_ROOT,
        "transaction_index": "0",
        "transaction_count": "1",
        "transaction_bytes": TRON_TRANSACTION_SOURCE_BYTES_HEX,
        "transaction_merkle_branch": [],
        "inclusion_branch": ["0x" + "aa" * 32],
    }
    assert tron_sccp_source_message_call_data(5, 0, source_event_digest).hex() == (
        TRON_SOURCE_MESSAGE_CALL_DATA_HEX
    )
    assert tron_sccp_source_message_call_data("5", "0", source_event_digest).hex() == (
        TRON_SOURCE_MESSAGE_CALL_DATA_HEX
    )
    with pytest.raises(ValueError, match="sourceDomain"):
        tron_sccp_source_message_call_data(0, 0, source_event_digest)
    with pytest.raises(ValueError, match="targetDomain"):
        tron_sccp_source_message_call_data(5, 5, source_event_digest)
    for source_domain, target_domain in [
        ("05", 0),
        ("0x5", 0),
        ("+5", 0),
        (" 5", 0),
        (5.0, 0),
        (5, "00"),
    ]:
        with pytest.raises(TypeError, match="u32 domain id"):
            tron_sccp_source_message_call_data(
                source_domain,
                target_domain,
                source_event_digest,
            )
    with pytest.raises(TypeError, match="sourceEventDigest"):
        tron_sccp_source_message_call_data(5, 0, "0x" + "00" * 32)
    with pytest.raises(TypeError, match="sourceEventDigest must be canonical hex"):
        tron_sccp_source_message_call_data(
            5,
            0,
            "0x" + "34" * 16 + " " + "34" * 16,
        )
    assert (
        len(
            canonical_tron_sccp_transaction_source_proof_bytes(
                tron_transaction_source_input
            )
        )
        == 476
    )
    assert (
        canonical_tron_sccp_transaction_source_proof_bytes(
            {
                **tron_transaction_source_input,
                "source_bridge_emitter_address": "0x" + "45" * 20,
                "source_bridge_owner_address": "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
            }
        )
        == canonical_tron_sccp_transaction_source_proof_bytes(
            tron_transaction_source_input
        )
    )
    for patch, pattern in [
        ({"sourceEventDigest": source_event_digest}, "sourceEventDigest must not use multiple aliases"),
        ({"receiptOrMessageRoot": HEX32_B}, "receiptRoot must not use multiple aliases"),
        ({"transactionRoot": TRON_TRANSACTION_SOURCE_ROOT}, "transactionRoot must not use multiple aliases"),
        ({"transactionIndex": "0"}, "transactionIndex must not use multiple aliases"),
        ({"transactionCount": "1"}, "transactionCount must not use multiple aliases"),
        ({"transactionBytes": TRON_TRANSACTION_SOURCE_BYTES_HEX}, "transactionBytes must not use multiple aliases"),
        ({"transactionMerkleBranch": []}, "transactionMerkleBranch must not use multiple aliases"),
        ({"inclusionBranch": ["0x" + "aa" * 32]}, "inclusionBranch must not use multiple aliases"),
        (
            {
                "bridgeAddress": "0x" + "45" * 20,
                "source_bridge_emitter_address": "0x" + "45" * 20,
            },
            "sourceBridgeEmitterAddress must not use multiple aliases",
        ),
        (
            {
                "ownerAddress": "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
                "source_bridge_owner_address": "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
            },
            "sourceBridgeOwnerAddress must not use multiple aliases",
        ),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_sccp_transaction_source_proof_bytes(
                {**tron_transaction_source_input, **patch}
            )
    assert (
        tron_sccp_transaction_source_proof_hash(tron_transaction_source_input)
        == TRON_TRANSACTION_SOURCE_PROOF_HASH
    )
    omitted_default_ret_transaction_bytes = TRON_TRANSACTION_SOURCE_BYTES_HEX.replace(
        "2a0410001801",
        "2a021801",
    )
    omitted_default_ret_input = {
        **tron_transaction_source_input,
        "transaction_root": "0x"
        + hashlib.sha256(
            bytes.fromhex(omitted_default_ret_transaction_bytes.removeprefix("0x"))
        ).hexdigest(),
        "transaction_bytes": omitted_default_ret_transaction_bytes,
    }
    assert canonical_tron_sccp_transaction_source_proof_bytes(
        omitted_default_ret_input
    )
    assert tron_sccp_transaction_source_proof_hash(omitted_default_ret_input)
    with pytest.raises(ValueError, match="successful TRON TriggerSmartContract source call"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {
                **tron_transaction_source_input,
                "source_bridge_emitter_address": "0x" + "46" * 20,
                "source_bridge_owner_address": "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
            }
        )
    with pytest.raises(ValueError, match="successful TRON TriggerSmartContract source call"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {
                **tron_transaction_source_input,
                "source_bridge_emitter_address": "0x" + "45" * 20,
                "source_bridge_owner_address": "0x" + "22" * 20,
            }
        )
    noncanonical_transaction_signature = TRON_TRANSACTION_SOURCE_BYTES_HEX.replace(
        "fe8973c012a0410001801", "fe8973c1f2a0410001801"
    )
    with pytest.raises(ValueError, match="successful TRON TriggerSmartContract source call"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {
                **tron_transaction_source_input,
                "transaction_bytes": noncanonical_transaction_signature,
            }
        )
    wrong_signer_transaction_signature = TRON_TRANSACTION_SOURCE_BYTES_HEX.replace(
        "cc58d7ac52c9111792495fee682b53cab96ff4229043c5b8b90c31447f5934553d8854ab35de34372c13331bf3ef5cefd8f2cc5ad026faf223da83969fe8973c01",
        "b50455577deef2a0d6c3c521d97de050d5b9ba46df00c8ddad014bac4ca3345173223f1d4c5940538f1b1da069bed6828a9b27794bd1eac1a35810baaef28d2101",
    )
    with pytest.raises(ValueError, match="successful TRON TriggerSmartContract source call"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {
                **tron_transaction_source_input,
                "transaction_bytes": wrong_signer_transaction_signature,
            }
        )
    with pytest.raises(ValueError, match="transactionIndex"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {**tron_transaction_source_input, "transaction_index": "1"}
        )
    with pytest.raises(TypeError, match="transactionMerkleBranch"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {
                **tron_transaction_source_input,
                "transaction_merkle_branch": ["0x" + "11" * 31],
                "transaction_count": "3",
            }
        )
    with pytest.raises(TypeError, match="transactionRoot"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {**tron_transaction_source_input, "transaction_root": HEX32_C}
        )
    with pytest.raises(TypeError, match="truncated protobuf bytes field"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {
                **tron_transaction_source_input,
                "transaction_root": "0xe4a77765ae41dc30b8bf3f7d9847170e0646e3dd0189433d2e3c88296221c942",
                "transaction_index": "1",
                "transaction_count": "3",
                "transaction_bytes": "0x123456",
                "transaction_merkle_branch": ["0x" + "11" * 32, "0x" + "22" * 32],
            }
        )
    with pytest.raises(ValueError, match="inclusionBranch must not be empty"):
        canonical_tron_sccp_transaction_source_proof_bytes(
            {**tron_transaction_source_input, "inclusion_branch": []}
        )
    parent_raw_header_input = {
        "number": "12344",
        "tx_trie_root": HEX32_C,
        "account_state_root": "0x" + "aa" * 32,
        "parent_block_id": HEX32_B,
        "witness_address": "0x41" + "11" * 20,
        "header_version": "1",
        "timestamp_ms": "1700000012344",
    }
    parent_raw_header = canonical_tron_raw_block_header_bytes(parent_raw_header_input)
    raw_header_input = {
        "number": "12345",
        "tx_trie_root": HEX32_D,
        "account_state_root": "0x" + "ee" * 32,
        "parent_block_id": TRON_PARENT_BLOCK_ID,
        "witness_address": "0x41" + "11" * 20,
        "header_version": "1",
        "timestamp_ms": "1700000012345",
    }
    raw_header = canonical_tron_raw_block_header_bytes(raw_header_input)
    assert parent_raw_header.hex() == TRON_PARENT_RAW_HEADER_HEX
    assert raw_header.hex() == TRON_RAW_HEADER_HEX
    for patch, pattern in [
        ({"blockNumber": "12345"}, "number must not use multiple aliases"),
        ({"txTrieRoot": HEX32_D}, "txTrieRoot must not use multiple aliases"),
        ({"accountStateRoot": "0x" + "ee" * 32}, "accountStateRoot must not use multiple aliases"),
        ({"parentBlockId": TRON_PARENT_BLOCK_ID}, "parentBlockId must not use multiple aliases"),
        ({"witnessAddress": "0x41" + "11" * 20}, "witnessAddress must not use multiple aliases"),
        ({"headerVersion": "1"}, "headerVersion must not use multiple aliases"),
        ({"timestampMs": "1700000012345"}, "timestampMs must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_raw_block_header_bytes({**raw_header_input, **patch})
    with pytest.raises(TypeError, match="txTrieRoot must be canonical hex"):
        canonical_tron_raw_block_header_bytes(
            {
                "number": "12345",
                "tx_trie_root": HEX32_D + "\n",
                "account_state_root": "0x" + "ee" * 32,
                "parent_block_id": TRON_PARENT_BLOCK_ID,
                "witness_address": "0x41" + "11" * 20,
                "header_version": "1",
                "timestamp_ms": "1700000012345",
            }
        )
    assert tron_raw_block_header_hash(parent_raw_header) == TRON_PARENT_RAW_HEADER_HASH
    assert tron_raw_block_header_hash(raw_header) == TRON_RAW_HEADER_HASH
    assert (
        tron_block_id_from_raw_data_hash("12344", TRON_PARENT_RAW_HEADER_HASH)
        == TRON_PARENT_BLOCK_ID
    )
    assert (
        tron_block_id_from_raw_data_hash(12344, TRON_PARENT_RAW_HEADER_HASH)
        == TRON_PARENT_BLOCK_ID
    )
    assert tron_block_id_from_raw_data_hash("12345", TRON_RAW_HEADER_HASH) == TRON_BLOCK_ID
    for block_number in ["012345", "0x3039", "+12345", " 12345", 12345.0]:
        with pytest.raises(TypeError, match="number must be an unsigned integer"):
            tron_block_id_from_raw_data_hash(block_number, TRON_RAW_HEADER_HASH)
    with pytest.raises(ValueError, match="TRON 0x41-prefixed address"):
        canonical_tron_raw_block_header_bytes(
            {
                "number": "12346",
                "tx_trie_root": HEX32_D,
                "account_state_root": "0x" + "ee" * 32,
                "parent_block_id": TRON_BLOCK_ID,
                "witness_address": "0x41" + "00" * 20,
                "header_version": "1",
                "timestamp_ms": "1700000012346",
            }
        )
    solid_header_proof = {
        "raw_data": raw_header,
        "witness_signature": tron_header_signature(0),
        "parent_raw_data": parent_raw_header,
        "parent_witness_signature": tron_header_signature(27),
        "raw_data_hash": TRON_RAW_HEADER_HASH,
        "parent_raw_data_hash": TRON_PARENT_RAW_HEADER_HASH,
        "block_id": TRON_BLOCK_ID,
        "tx_trie_root": HEX32_D,
        "account_state_root": "0x" + "ee" * 32,
        "parent_block_id": TRON_PARENT_BLOCK_ID,
        "witness_address": "0x41" + "11" * 20,
        "timestamp_ms": "1700000012345",
        "header_version": "1",
    }
    assert len(canonical_tron_solid_block_header_proof_bytes(solid_header_proof)) == 650
    assert (
        tron_solid_block_header_proof_hash(solid_header_proof)
        == TRON_SOLID_BLOCK_HEADER_PROOF_HASH
    )
    for patch, pattern in [
        ({"rawData": raw_header}, "rawData must not use multiple aliases"),
        ({"witnessSignature": tron_header_signature(0)}, "witnessSignature must not use multiple aliases"),
        ({"parentRawData": parent_raw_header}, "parentRawData must not use multiple aliases"),
        (
            {"parentWitnessSignature": tron_header_signature(27)},
            "parentWitnessSignature must not use multiple aliases",
        ),
        ({"rawDataHash": TRON_RAW_HEADER_HASH}, "rawDataHash must not use multiple aliases"),
        (
            {"parentRawDataHash": TRON_PARENT_RAW_HEADER_HASH},
            "parentRawDataHash must not use multiple aliases",
        ),
        ({"blockId": TRON_BLOCK_ID}, "blockId must not use multiple aliases"),
        ({"txTrieRoot": HEX32_D}, "txTrieRoot must not use multiple aliases"),
        ({"accountStateRoot": "0x" + "ee" * 32}, "accountStateRoot must not use multiple aliases"),
        ({"parentBlockId": TRON_PARENT_BLOCK_ID}, "parentBlockId must not use multiple aliases"),
        ({"witnessAddress": "0x41" + "11" * 20}, "witnessAddress must not use multiple aliases"),
        ({"timestampMs": "1700000012345"}, "timestampMs must not use multiple aliases"),
        ({"headerVersion": "1"}, "headerVersion must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_tron_solid_block_header_proof_bytes(
                {**solid_header_proof, **patch}
            )
    with pytest.raises(TypeError, match="rawDataHash must match rawData"):
        canonical_tron_solid_block_header_proof_bytes(
            {**solid_header_proof, "raw_data_hash": HEX32_A}
        )
    overlong_key_raw_header = bytes([0x88, 0x00]) + raw_header[1:]
    overlong_key_raw_header_hash = tron_raw_block_header_hash(overlong_key_raw_header)
    with pytest.raises(TypeError, match="protobuf varint must be canonical"):
        canonical_tron_solid_block_header_proof_bytes(
            {
                **solid_header_proof,
                "raw_data": overlong_key_raw_header,
                "raw_data_hash": overlong_key_raw_header_hash,
                "block_id": tron_block_id_from_raw_data_hash(
                    "12345",
                    overlong_key_raw_header_hash,
                ),
            }
        )
    with pytest.raises(ValueError, match="TRON 0x41-prefixed address"):
        canonical_tron_solid_block_header_proof_bytes(
            {**solid_header_proof, "witness_address": "0x41" + "00" * 20}
        )
    with pytest.raises(ValueError, match="rawData and parentRawData must be at most"):
        canonical_tron_solid_block_header_proof_bytes(
            {**solid_header_proof, "raw_data": bytes([0xAA]) * (16 * 1024 + 1)}
        )
    with pytest.raises(ValueError, match="TRON header signatures must be canonical low-S"):
        canonical_tron_solid_block_header_proof_bytes(
            {**solid_header_proof, "witness_signature": bytes([0xAA]) * 65}
        )
    with pytest.raises(ValueError, match="TRON header signatures must be canonical low-S"):
        canonical_tron_solid_block_header_proof_bytes(
            {**solid_header_proof, "parent_witness_signature": tron_header_signature(4)}
        )
    zero_r_signature = bytearray(tron_header_signature(0))
    zero_r_signature[:32] = bytes(32)
    with pytest.raises(ValueError, match="TRON header signatures must be canonical low-S"):
        canonical_tron_solid_block_header_proof_bytes(
            {**solid_header_proof, "witness_signature": bytes(zero_r_signature)}
        )

    substrate_input = {
        "source_domain": SCCP_DOMAIN_SORA_KUSAMA,
        "source_event_digest": source_event_digest,
        "source_event_leaf_index": "0",
        "finalized_block_number": "31",
        "grandpa_set_id": "32",
        "block_hash": HEX32_A,
        "authority_set_hash": HEX32_C,
        "events_root": HEX32_B,
        "inclusion_branch": inclusion_branch,
    }
    assert len(canonical_substrate_sccp_storage_proof_bytes(substrate_input)) == 225
    with pytest.raises(TypeError, match="sourceEventDigest must not be zero"):
        canonical_substrate_sccp_storage_proof_bytes(
            {**substrate_input, "source_event_digest": SCCP_ZERO_HASH_V1}
        )
    for patch, pattern in [
        ({"sourceDomain": SCCP_DOMAIN_SORA_KUSAMA}, "sourceDomain must not use multiple aliases"),
        ({"sourceEventDigest": source_event_digest}, "sourceEventDigest must not use multiple aliases"),
        ({"leafIndex": "0"}, "sourceEventLeafIndex must not use multiple aliases"),
        ({"finalityHeight": "31"}, "finalizedBlockNumber must not use multiple aliases"),
        ({"grandpaSetId": "32"}, "grandpaSetId must not use multiple aliases"),
        ({"finalityBlockHash": HEX32_A}, "blockHash must not use multiple aliases"),
        ({"authoritySetHash": HEX32_C}, "authoritySetHash must not use multiple aliases"),
        ({"receiptOrMessageRoot": HEX32_B}, "eventsRoot must not use multiple aliases"),
        ({"inclusionBranch": inclusion_branch}, "inclusionBranch must not use multiple aliases"),
    ]:
        with pytest.raises(TypeError, match=pattern):
            canonical_substrate_sccp_storage_proof_bytes({**substrate_input, **patch})
    assert (
        substrate_sccp_storage_proof_hash(substrate_input)
        == "0x1ff06ab7e38182e9b276a4967fba604fd85ed81ddd6f2a97093031a13e701386"
    )
    substrate_runtime_storage_input = {
        **substrate_input,
        "source_trust_anchor_hash": HEX32_A,
        "consensus_verifier_hash": HEX32_B,
        "message_inclusion_verifier_hash": HEX32_C,
        "finality_policy_hash": HEX32_D,
        "source_state_verifier_hash": HEX32_F,
    }
    assert (
        canonical_substrate_sccp_runtime_storage_verification_statement_bytes(
            substrate_runtime_storage_input
        )
        == canonical_substrate_sccp_storage_proof_bytes(substrate_input)
    )
    runtime_storage_public_inputs_hash = (
        substrate_sccp_runtime_storage_proof_public_inputs_hash(
            substrate_runtime_storage_input
        )
    )
    assert runtime_storage_public_inputs_hash.startswith("0x")
    assert len(runtime_storage_public_inputs_hash) == 66
    runtime_storage_columns = substrate_sccp_runtime_storage_public_input_columns(
        substrate_runtime_storage_input
    )
    assert len(runtime_storage_columns) == 11
    assert runtime_storage_columns[6] == [substrate_sccp_storage_proof_hash(substrate_input)]
    assert runtime_storage_columns[8] == [
        "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7"
    ]
    assert runtime_storage_columns[10] == [runtime_storage_public_inputs_hash]
    runtime_storage_context = (
        canonical_substrate_sccp_runtime_storage_verification_context_bytes(
            substrate_runtime_storage_input
        )
    )
    assert runtime_storage_context != (
        canonical_substrate_sccp_runtime_storage_verification_context_bytes(
            {**substrate_runtime_storage_input, "source_state_verifier_hash": HEX32_G}
        )
    )
    runtime_storage_request = build_substrate_sccp_runtime_storage_proof_request(
        substrate_runtime_storage_input
    )
    assert_immutable_fastpq_proof_request(
        runtime_storage_request,
        ("statement_bytes", "verification_context_bytes", "schema_descriptor"),
    )
    assert (
        runtime_storage_request["circuit_id"]
        == SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert (
        runtime_storage_request["runtime_storage_proof_public_inputs_hash"]
        == runtime_storage_public_inputs_hash
    )
    assert runtime_storage_request["fastpq_public_inputs"]["slot"] == "31"
    assert [
        transition["key"] for transition in runtime_storage_request["fastpq_transitions"]
    ] == [
        "sccp:substrate:runtime-storage:v1:context",
        "sccp:substrate:runtime-storage:v1:statement",
        "sccp:substrate:runtime-storage:v1:storage-key",
    ]
    assert len(
        substrate_sccp_runtime_storage_open_verify_schema_descriptor(
            substrate_runtime_storage_input
        )
    ) == len(runtime_storage_request["schema_descriptor"])
    with pytest.raises(TypeError, match="storageProofHash must not use multiple aliases"):
        build_substrate_sccp_runtime_storage_proof_request(
            {
                **substrate_runtime_storage_input,
                "storage_proof_hash": substrate_sccp_storage_proof_hash(substrate_input),
                "storageProofHash": substrate_sccp_storage_proof_hash(substrate_input),
            }
        )
    with pytest.raises(TypeError, match="sourceVerifierMaterial must not use multiple aliases"):
        build_substrate_sccp_runtime_storage_proof_request(
            {
                **substrate_runtime_storage_input,
                "source_verifier_material": {"source_domain": SCCP_DOMAIN_SORA_KUSAMA},
                "sourceVerifierMaterial": {"source_domain": SCCP_DOMAIN_SORA_KUSAMA},
            }
        )
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        substrate_sccp_runtime_storage_open_verify_schema_descriptor(
            {
                "source_domain": SCCP_DOMAIN_SORA_KUSAMA,
                "sourceDomain": SCCP_DOMAIN_SORA_KUSAMA,
            }
        )
    with pytest.raises(TypeError, match="storageProofHash"):
        build_substrate_sccp_runtime_storage_proof_request(
            {**substrate_runtime_storage_input, "storage_proof_hash": HEX32_A}
        )
    with pytest.raises(TypeError, match="template verifier hash"):
        build_substrate_sccp_runtime_storage_proof_request(
            {
                **substrate_runtime_storage_input,
                "source_state_verifier_hash": (
                    "0xaf2d28b3e07447239f28e90ce4fdee7e6cd3778c087eaeda7170781eb4b76b9c"
                ),
            }
        )
    with pytest.raises(ValueError, match="sourceVerifierMaterial.sourceDomain"):
        build_substrate_sccp_runtime_storage_proof_request(
            {
                **substrate_runtime_storage_input,
                "source_verifier_material": {"source_domain": SCCP_DOMAIN_ETH},
            }
        )
    with pytest.raises(TypeError, match="sourceVerifierMaterial.sourceDomain"):
        build_substrate_sccp_runtime_storage_proof_request(
            {
                **substrate_runtime_storage_input,
                "source_verifier_material": {
                    "sourceDomain": SCCP_DOMAIN_SORA_KUSAMA,
                    "source_domain": SCCP_DOMAIN_SORA_KUSAMA,
                },
            }
        )
    assert substrate_sccp_storage_proof_hash(substrate_input) != (
        substrate_sccp_storage_proof_hash({**substrate_input, "inclusion_branch": changed_branch})
    )
    assert substrate_sccp_storage_proof_hash(substrate_input) != (
        substrate_sccp_storage_proof_hash(
            {**substrate_input, "source_event_leaf_index": "1"}
        )
    )

    with pytest.raises(TypeError, match="inclusionBranch\\[0\\] must be 32 bytes"):
        canonical_tron_sccp_receipt_proof_bytes(
            {**tron_input, "inclusion_branch": [bytes([1, 2, 3])]}
        )
    with pytest.raises(TypeError, match="receiptTrieProofNodes must not be empty"):
        canonical_tron_sccp_receipt_state_proof_bytes(
            {**tron_receipt_state_input, "receipt_trie_proof_nodes": []}
        )
    with pytest.raises(TypeError, match="sourceDomain must be a u32 domain id"):
        canonical_substrate_sccp_storage_proof_bytes(
            {**substrate_input, "source_domain": None}
        )


def test_derives_eth_sync_committee_transition_transcripts_from_ui_witness_material() -> None:
    def sync_committee_fixture(public_key_byte: int, pop_byte: int) -> dict[str, list[Any]]:
        public_keys = []
        pops = []
        for index in range(512):
            public_key = bytearray([public_key_byte] * 48)
            public_key[46:48] = index.to_bytes(2, "big")
            public_keys.append(bytes(public_key))
            pop = bytearray([pop_byte] * 96)
            pop[94:96] = index.to_bytes(2, "big")
            pops.append(bytes(pop))
        return {
            "sync_committee_public_keys": public_keys,
            "sync_committee_weights": ["1"] * 512,
            "sync_committee_pops": pops,
        }

    def signers_bitmap(count: int) -> bytes:
        bitmap = bytearray(64)
        for index in range(count):
            bitmap[index // 8] |= 1 << (index % 8)
        return bytes(bitmap)

    parent = {
        **sync_committee_fixture(0x11, 0xAA),
    }
    next_committee = {
        **sync_committee_fixture(0x33, 0xCC),
    }
    next_payload = canonical_eth_sync_committee_payload_bytes(next_committee)
    parent_hash = eth_sync_committee_hash(parent)
    next_hash = eth_sync_committee_hash_from_payload(next_payload)
    next_payload_hash = eth_sync_committee_payload_hash(next_payload)

    assert re.fullmatch(r"0x[0-9a-f]{64}", parent_hash)
    assert len(next_payload) == 81925
    assert re.fullmatch(r"0x[0-9a-f]{64}", next_hash)
    assert re.fullmatch(r"0x[0-9a-f]{64}", next_payload_hash)
    assert eth_mainnet_sync_committee_period_for_slot("19") == 0
    assert eth_mainnet_sync_committee_period_for_slot(8192) == 1
    with pytest.raises(ValueError, match="exactly 512"):
        canonical_eth_sync_committee_payload_bytes(
            {
                "sync_committee_public_keys": ["0x" + "11" * 48, "0x" + "22" * 48],
                "sync_committee_weights": ["1", "1"],
                "sync_committee_pops": ["0x" + "aa" * 96, "0x" + "bb" * 96],
            }
        )
    with pytest.raises(ValueError, match="must be 1"):
        canonical_eth_sync_committee_payload_bytes(
            {
                **parent,
                "sync_committee_weights": ["2"] + parent["sync_committee_weights"][1:],
            }
        )
    with pytest.raises(TypeError, match="syncCommitteePublicKeys must not use multiple aliases"):
        canonical_eth_sync_committee_payload_bytes(
            {
                **parent,
                "syncCommitteePublicKeys": parent["sync_committee_public_keys"],
            }
        )

    message_input = {
        "source_domain": SCCP_DOMAIN_ETH,
        "from_sync_period": "0",
        "to_sync_period": "1",
        "transition_slot": "19",
        "finalized_beacon_root": HEX32_A,
        "parent_sync_committee_hash": parent_hash,
        "next_sync_committee_hash": next_hash,
        "next_sync_committee_payload_hash": next_payload_hash,
        "next_sync_committee_branch_hash": "0x" + "be" * 32,
    }
    transition_message_hash = eth_sync_committee_transition_message_hash(message_input)
    assert len(canonical_eth_sync_committee_transition_message_bytes(message_input)) == 189
    assert re.fullmatch(r"0x[0-9a-f]{64}", transition_message_hash)
    with pytest.raises(ValueError, match="sourceDomain"):
        canonical_eth_sync_committee_transition_message_bytes(
            {**message_input, "source_domain": SCCP_DOMAIN_BSC}
        )
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        canonical_eth_sync_committee_transition_message_bytes(
            {**message_input, "sourceDomain": SCCP_DOMAIN_ETH}
        )
    with pytest.raises(TypeError, match="fromSyncPeriod must not use multiple aliases"):
        canonical_eth_sync_committee_transition_message_bytes(
            {**message_input, "fromSyncPeriod": "0"}
        )
    with pytest.raises(ValueError, match="toSyncPeriod"):
        canonical_eth_sync_committee_transition_message_bytes(
            {**message_input, "to_sync_period": "2"}
        )
    with pytest.raises(ValueError, match="transitionSlot must belong to fromSyncPeriod"):
        canonical_eth_sync_committee_transition_message_bytes(
            {**message_input, "from_sync_period": "1", "to_sync_period": "2"}
        )
    with pytest.raises(ValueError, match="transitionSlot must not be zero"):
        canonical_eth_sync_committee_transition_message_bytes(
            {**message_input, "transition_slot": "0"}
        )
    with pytest.raises(TypeError, match="nextSyncCommitteePayloadHash must not use multiple aliases"):
        canonical_eth_sync_committee_transition_message_bytes(
            {
                **message_input,
                "nextSyncCommitteePayloadHash": next_payload_hash,
            }
        )

    signature_input = {
        **message_input,
        "version": 1,
        "next_sync_committee_payload": next_payload,
        "transition_message_hash": transition_message_hash,
        "sync_committee_proof": {
            "version": 1,
            "total_weight": "512",
            "signed_weight": "342",
            "sync_committee_message_hash": transition_message_hash,
            **parent,
            "signers_bitmap": signers_bitmap(342),
            "aggregate_signature": "0x" + "ee" * 96,
        },
    }
    assert len(canonical_eth_sync_committee_transition_signature_bytes(signature_input)) > len(
        next_payload
    )
    assert re.fullmatch(
        r"0x[0-9a-f]{64}", eth_sync_committee_transition_signature_hash(signature_input)
    )
    with pytest.raises(TypeError, match="syncCommitteeProof must not use multiple aliases"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "syncCommitteeProof": signature_input["sync_committee_proof"],
            }
        )
    with pytest.raises(TypeError, match="nextSyncCommitteePayload must not use multiple aliases"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {**signature_input, "nextSyncCommitteePayload": next_payload}
        )
    with pytest.raises(TypeError, match="transitionMessageHash must not use multiple aliases"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "transitionMessageHash": transition_message_hash,
            }
        )
    with pytest.raises(TypeError, match="signersBitmap must not use multiple aliases"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "signersBitmap": signature_input["sync_committee_proof"]["signers_bitmap"],
                },
            }
        )
    with pytest.raises(
        ValueError,
        match="ETH sync-committee transition signature version",
    ):
        canonical_eth_sync_committee_transition_signature_bytes(
            {**signature_input, "version": 0}
        )
    with pytest.raises(ValueError, match="syncCommitteeProof.version"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "version": 0,
                },
            }
        )
    with pytest.raises(TypeError, match="syncCommitteeProof.version"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "version": None,
                },
            }
        )

    with pytest.raises(ValueError, match="exactly 512"):
        canonical_eth_sync_committee_payload_bytes(
            {
                "sync_committee_public_keys": ["0x" + "11" * 48] * 513,
                "sync_committee_weights": ["1"] * 513,
                "sync_committee_pops": ["0x" + "aa" * 96] * 513,
            }
        )
    with pytest.raises(ValueError, match="48 bytes"):
        malformed_public_keys = parent["sync_committee_public_keys"].copy()
        malformed_public_keys[0] = "0x" + "11" * 47
        canonical_eth_sync_committee_payload_bytes(
            {
                **parent,
                "sync_committee_public_keys": malformed_public_keys,
            }
        )
    with pytest.raises(ValueError, match="must not be zero"):
        zero_pops = parent["sync_committee_pops"].copy()
        zero_pops[0] = bytes(96)
        canonical_eth_sync_committee_payload_bytes(
            {
                **parent,
                "sync_committee_pops": zero_pops,
            }
        )
    with pytest.raises(ValueError, match="signersBitmap"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "signers_bitmap": bytes(65),
                },
            }
        )
    with pytest.raises(ValueError, match="select at least one"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "signers_bitmap": signers_bitmap(0),
                },
            }
        )
    with pytest.raises(ValueError, match="totalWeight"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "total_weight": "513",
                },
            }
        )
    with pytest.raises(ValueError, match="signedWeight"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "signed_weight": "341",
                },
            }
        )
    with pytest.raises(ValueError, match="greater than two thirds"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "signed_weight": "341",
                    "signers_bitmap": signers_bitmap(341),
                },
            }
        )
    with pytest.raises(ValueError, match="96 bytes"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "aggregate_signature": bytes(95),
                },
            }
        )
    with pytest.raises(ValueError, match="all zero"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {
                **signature_input,
                "sync_committee_proof": {
                    **signature_input["sync_committee_proof"],
                    "aggregate_signature": bytes(96),
                },
            }
        )
    with pytest.raises(ValueError, match=r"syncCommitteePublicKeys\[0\] must not be zero"):
        zero_public_keys = parent["sync_committee_public_keys"].copy()
        zero_public_keys[0] = "0x" + "00" * 48
        canonical_eth_sync_committee_payload_bytes(
            {
                **parent,
                "sync_committee_public_keys": zero_public_keys,
            }
        )
    with pytest.raises(TypeError, match="nextSyncCommitteePayloadHash must match"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {**signature_input, "next_sync_committee_payload_hash": HEX32_B}
        )
    with pytest.raises(TypeError, match="nextSyncCommitteeHash must match"):
        canonical_eth_sync_committee_transition_signature_bytes(
            {**signature_input, "next_sync_committee_hash": HEX32_B}
        )


def test_derives_eth_beacon_execution_payload_ssz_roots_from_ui_witness_material() -> None:
    header_rlp = _sample_eth_execution_header_rlp()
    execution_payload_root = eth_execution_payload_header_root_from_rlp(header_rlp)
    execution_payload_branch = [
        HEX32_E,
        "0x" + "ff" * 32,
        "0x" + "11" * 32,
        "0x" + "22" * 32,
    ]
    beacon_body_root = eth_beacon_body_root_from_execution_payload_branch(
        execution_payload_root,
        execution_payload_branch,
    )
    beacon_header_input = {
        "beacon_slot": "320",
        "beacon_proposer_index": "17",
        "beacon_parent_root": HEX32_A,
        "beacon_state_root": HEX32_B,
        "beacon_body_root": beacon_body_root,
    }
    beacon_header_root = eth_beacon_block_header_root(beacon_header_input)

    assert execution_payload_root == "0xc029dda492d2e41ad72bd83f1727a67e5331f413ec29d5c31de955d0bea24624"
    assert beacon_body_root == "0x431e6bef5e759e8fdf32d8e8ed1ff761933ddb4de24ec9ae8e2aa0d25fe861ba"
    assert beacon_header_root == "0xd54b406debae26e6ebaef512cc4f9e6bc12cf02af0d4476895383b37f682a179"
    with pytest.raises(TypeError, match="slot must not use multiple aliases"):
        eth_beacon_block_header_root({**beacon_header_input, "slot": "320"})
    with pytest.raises(TypeError, match="proposerIndex must not use multiple aliases"):
        eth_beacon_block_header_root({**beacon_header_input, "proposerIndex": "17"})
    with pytest.raises(TypeError, match="bodyRoot must not use multiple aliases"):
        eth_beacon_block_header_root({**beacon_header_input, "bodyRoot": beacon_body_root})
    assert (
        eth_beacon_body_root_from_execution_payload_branch(
            execution_payload_root,
            ["0x" + "ff" * 32, "0x" + "ff" * 32, "0x" + "11" * 32, "0x" + "22" * 32],
        )
        != beacon_body_root
    )
    with pytest.raises(TypeError, match="executionPayloadBranch"):
        eth_beacon_body_root_from_execution_payload_branch(execution_payload_root, [HEX32_E])
    with pytest.raises(ValueError, match="RLP list"):
        eth_execution_payload_header_root_from_rlp(b"\x80")


def test_derives_groth16_bn254_public_signal_words_for_destination_verifiers() -> None:
    public_inputs = {
        "version": 1,
        "message_id": "0x" + "11" * 32,
        "payload_hash": "0x" + "22" * 32,
        "target_domain": SCCP_DOMAIN_TRON,
        "commitment_root": "0x" + "33" * 32,
        "finality_height": "19",
        "finality_block_hash": "0x" + "44" * 32,
    }
    signals = sccp_groth16_bn254_public_signal_words(
        {
            "public_inputs": public_inputs,
            "source_domain": SCCP_DOMAIN_SORA,
            "statement_hash": "0x" + "55" * 32,
            "destination_binding_hash": "0x" + "66" * 32,
        }
    )

    assert signals == [
        "0x0ffdbc782e79d1dc508e08af01e87f16d93b6e58e4861a0b8155455e3ee7a683",
        "0x0c5398ea95021a790e276e3ece1592b32b85751dc77e50293c867a5f2e0131bb",
        "0x21aac4195d8db839756f61c0780675823e15456c92acf135c36e02367c8fd11f",
        "0x01c73f2f9156a52493a9beabeec73e62deed32fcef2e3e6fac86a79f0764f0bc",
        "0x0ca6bbc36d23183d027c8df09f06c39e64abbb0bb4d6a4c37369d2c36f41a888",
        "0x2b153d0fe1bc6e2a6d44e851523edb1511dac55443ca80c22cbe9cb7423886dc",
        "0x2697e4e42f34b673b4aa254c6a92de09304e84c1a667c7d266777775a231efb4",
        "0x16fbe0c1d659f142b3e7815b24df66da3cfd89cc42d051b04bc31aae6925c396",
        "0x1157cd422e2089145c9cf93794dd6a0a1c3b1a611c22a5fe999d0542f62535d8",
    ]
    changed_binding = sccp_groth16_bn254_public_signal_words(
        {
            "public_inputs": public_inputs,
            "source_domain": SCCP_DOMAIN_SORA,
            "statement_hash": "0x" + "55" * 32,
            "destination_binding_hash": "0x" + "67" * 32,
        }
    )
    assert changed_binding[:8] == signals[:8]
    assert changed_binding[8] != signals[8]
    for bad_public_inputs in ({}, False, 0, ""):
        with pytest.raises(TypeError, match="publicInputs"):
            sccp_groth16_bn254_public_signal_words(
                {
                    "public_inputs": bad_public_inputs,
                    "source_domain": SCCP_DOMAIN_SORA,
                    "statement_hash": "0x" + "55" * 32,
                    "destination_binding_hash": "0x" + "66" * 32,
                }
            )


def test_builds_deterministic_solana_sccp_proof_requests() -> None:
    request = build_solana_sccp_proof_request(sample_witness())

    assert request["version"] == 1
    assert request["backend"] == SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1
    assert request["source_domain"] == SCCP_DOMAIN_SOL
    assert request["public_inputs"]["message_id"] == HEX32_D
    assert request["public_inputs"]["bank_hash"] == HEX32_A
    assert request["public_inputs"]["transaction_status_root"] == HEX32_B
    assert request["public_inputs"]["message_proof_hash"] == HEX32_C
    assert request["public_inputs"]["parent_slot"] == "320"
    assert request["public_inputs"]["bank_signature_count"] == "8"
    assert request["public_inputs"]["accounts_lt_hash_proof_public_inputs_hash"] == (
        solana_sccp_accounts_lt_hash_proof_public_inputs_hash(request["witness"])
    )
    assert request["public_inputs"]["statement_hash"] == HEX32_G
    assert request["public_inputs"]["destination_binding_hash"] == HEX32_H
    assert request["source_state_verifier_id"] == SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1
    assert request["source_state_verifier_hash"] == SCCP_ZERO_HASH_V1
    assert (
        request["public_inputs"]["source_state_verifier_id"]
        == SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1
    )
    assert request["public_inputs"]["source_state_verifier_hash"] == SCCP_ZERO_HASH_V1
    assert request["public_inputs"]["source_adapter_deployment_hash"] == SCCP_ZERO_HASH_V1
    assert (
        request["public_inputs"]["source_adapter_deployment_receipt_hash"] == SCCP_ZERO_HASH_V1
    )
    assert request["source_adapter_deployment_binding"] == {
        "version": 1,
        "source_domain": SCCP_DOMAIN_SOL,
        "target_domain": SCCP_DOMAIN_SORA,
        "source_adapter_deployment_hash": SCCP_ZERO_HASH_V1,
        "source_adapter_deployment_receipt_hash": SCCP_ZERO_HASH_V1,
    }
    assert request["proof_context"] == {
        "version": 1,
        "statement_hash": HEX32_G,
        "destination_binding_hash": HEX32_H,
    }
    assert request["witness_hash"] == (
        "0x42db5036040f5ae4873123e3296f94df360a50d29634bb4ee3620667fadf9b61"
    )
    assert request["proof_context_hash"] == (
        "0x3301998ef4dccab62a62dc2be0b5f6b3e3a876344c67200dcbe6f751165d9679"
    )
    assert request["source_adapter_deployment_binding_hash"] == (
        "0x199859d1da5915d5d17df51a97c9df8ac9c375c0093230e4faac5476ab416e6a"
    )
    assert request["proof_context_hash"] == solana_sccp_proof_context_hash(
        request["proof_context"]
    )
    assert len(canonical_solana_sccp_proof_context_bytes(request["proof_context"])) > 0
    assert len(canonical_solana_sccp_accounts_lt_hash_proof_public_inputs_bytes(request["witness"])) > 250
    with pytest.raises(TypeError, match="immutable"):
        request["proof_context_hash"] = HEX32_A
    with pytest.raises(TypeError, match="immutable"):
        request["public_inputs"]["message_id"] = HEX32_A
    for bad_context in ({}, False, 0, ""):
        with pytest.raises(TypeError, match="Solana SCCP proof context|statementHash"):
            build_solana_sccp_proof_request(
                sample_witness(proof_context=bad_context)
            )
    for bad_blockhash in (b"", [], "", False):
        with pytest.raises(TypeError, match="blockhash"):
            normalize_solana_sccp_witness(
                sample_witness(blockhash_bytes=bad_blockhash)
            )
    with pytest.raises(TypeError, match=r"finalizedSlot.*multiple aliases"):
        build_solana_sccp_proof_request(sample_witness(finalizedSlot=321))
    with pytest.raises(TypeError, match=r"blockhash.*multiple aliases"):
        build_solana_sccp_proof_request(sample_witness(blockhashBytes=HEX32_A))
    with pytest.raises(TypeError, match=r"messageId.*multiple aliases"):
        build_solana_sccp_proof_request(sample_witness(messageId=HEX32_D))
    with pytest.raises(TypeError, match=r"statementHash.*multiple aliases"):
        build_solana_sccp_proof_request(
            sample_witness(
                proof_context={
                    "statement_hash": HEX32_G,
                    "statementHash": HEX32_G,
                    "destination_binding_hash": HEX32_H,
                }
            )
        )
    with pytest.raises(TypeError, match="inclusionBranch must be an array"):
        canonical_solana_sccp_witness_bytes(
            sample_witness(inclusion_branch=False)
        )


def test_solana_sccp_proof_request_requires_sora_target_domain() -> None:
    with pytest.raises(TypeError, match="statementHash must not be zero"):
        build_solana_sccp_proof_request(
            sample_witness(statement_hash=SCCP_ZERO_HASH_V1)
        )
    with pytest.raises(TypeError, match="destinationBindingHash must not be zero"):
        build_solana_sccp_proof_request(
            sample_witness(destination_binding_hash=SCCP_ZERO_HASH_V1)
        )
    with pytest.raises(ValueError, match="targetDomain must be SORA"):
        build_solana_sccp_proof_request(
            sample_witness(target_domain=SCCP_DOMAIN_TON)
        )


def test_rejects_unexpected_solana_source_state_verifier_profile() -> None:
    with pytest.raises(TypeError, match="AccountsDB verifier profile"):
        normalize_solana_sccp_witness(
            sample_witness(
                source_state_verifier_id="debug-solana-state-verifier",
                source_state_verifier_hash=HEX32_A,
            )
        )


def test_binds_source_adapter_deployment_context_for_ui_provers() -> None:
    binding = normalize_sccp_source_adapter_deployment_binding(
        {
            "source_domain": SCCP_DOMAIN_SOL,
            "target_domain": SCCP_DOMAIN_SORA,
            "source_adapter_deployment_hash": HEX32_A,
            "source_adapter_deployment_receipt_hash": HEX32_B,
        }
    )
    request = build_solana_sccp_proof_request(
        sample_witness(
            source_adapter_deployment_hash=HEX32_A,
            source_adapter_deployment_receipt_hash=HEX32_B,
        )
    )

    assert len(canonical_sccp_source_adapter_deployment_binding_bytes(binding)) == 73
    assert request["source_adapter_deployment_binding"] == binding
    assert request["source_adapter_deployment_binding_hash"] == (
        sccp_source_adapter_deployment_binding_hash(binding)
    )
    assert request["public_inputs"]["source_adapter_deployment_hash"] == HEX32_A
    assert request["public_inputs"]["source_adapter_deployment_receipt_hash"] == HEX32_B

    with pytest.raises(TypeError, match="must both be zero or both be non-zero"):
        normalize_sccp_source_adapter_deployment_binding(
            {
                "source_adapter_deployment_hash": HEX32_A,
                "source_adapter_deployment_receipt_hash": SCCP_ZERO_HASH_V1,
            }
        )
    with pytest.raises(TypeError, match="must differ from sourceAdapterDeploymentReceiptHash"):
        normalize_sccp_source_adapter_deployment_binding(
            {
                "source_adapter_deployment_hash": HEX32_A,
                "source_adapter_deployment_receipt_hash": HEX32_A,
            }
        )


def test_derives_source_adapter_verifier_vk_hashes_for_ui_tooling() -> None:
    vectors = {
        SCCP_DOMAIN_ETH: "0x2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46",
        SCCP_DOMAIN_BSC: "0x12536f25748a6520f10ebd42a7bcccd6ec181b9d53129795c8e186dc6e8b18cc",
        SCCP_DOMAIN_SOL: "0xe7bc29d06bf56184183c3fc59a0e934cd1d8e16751f1eda2efaaf88aa350b9d6",
        SCCP_DOMAIN_TON: "0xf03f70e8cb504e69b0611df224c2783d04d8f4ee93beae7a62e1cd0a49703bad",
        SCCP_DOMAIN_TRON: "0x0e12ad03def9d75887d4d6437e63539cef97c54db4769881eeda757a88826364",
        SCCP_DOMAIN_SORA_KUSAMA: "0xf7768653132995511594e6e7edb4af22f78bba615650d9dda72f14bb18984daf",
        SCCP_DOMAIN_SORA_POLKADOT: "0x4f8456bf8626436a16d763c40bf23dffb962232f0766c4ae33d6e594f8be1635",
        SCCP_DOMAIN_SORA2: "0x96bbfa08489249b28a1444d0dcb9d5b4023bd688091f31c0b435601dad48dbb4",
    }
    for source_domain, expected in vectors.items():
        assert sccp_source_adapter_verifier_vk_hash(source_domain) == expected
    with pytest.raises(ValueError, match="target_domain must be SORA"):
        sccp_source_adapter_verifier_vk_hash(SCCP_DOMAIN_TON, target_domain=SCCP_DOMAIN_TON)


def test_derives_native_destination_binding_hashes_for_ui_tooling() -> None:
    vectors = {
        SCCP_DOMAIN_SOL: (
            "sccp:0:3:sol:solana-program-v1:2",
            "0x078578f0aa27daa2972d6c19d1d26dbb6bf6ba1e8df84e283d7ef101fc46abf6",
        ),
        SCCP_DOMAIN_TON: (
            "sccp:0:4:ton:ton-contract-v1:3",
            "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799",
        ),
        SCCP_DOMAIN_SORA_KUSAMA: (
            "sccp:0:6:sora-kusama:substrate-runtime-v1:5",
            "0x2ee5c37634c3fab7e9086ea43af7553089fc24dc2ce27d76c46ef4c3da57bb56",
        ),
        SCCP_DOMAIN_SORA_POLKADOT: (
            "sccp:0:7:sora-polkadot:substrate-runtime-v1:5",
            "0x570ec340d4fee4a84eaa7a53b19baa53c9f4f8d7f64c3c43639fde0c6b3fdef0",
        ),
        SCCP_DOMAIN_SORA2: (
            "sccp:0:8:sora2:substrate-runtime-v1:5",
            "0xda5d48fe26518cd8cff6bdaa7cf8e37c7302d1e66469efed4ef2cf340c55b9e4",
        ),
    }
    for target_domain, (expected_key, expected_hash) in vectors.items():
        assert sccp_destination_binding_key(target_domain) == expected_key
        assert sccp_destination_binding_hash(target_domain) == expected_hash
    with pytest.raises(ValueError, match="native SCCP destination lane"):
        sccp_destination_binding_hash(SCCP_DOMAIN_ETH)


def test_derives_evm_and_tron_destination_bindings_for_ui_provers() -> None:
    evm_input = {
        "target_domain": SCCP_DOMAIN_ETH,
        "network_id": "0x" + "33" * 32,
        "verifier_address": "0x" + "11" * 20,
        "bridge_address": "0x" + "22" * 20,
        "verifier_code_hash": "0x" + "bb" * 32,
        "verifier_key_hash": "0x" + "cc" * 32,
    }
    evm_binding = evm_sccp_destination_binding(evm_input)
    assert evm_binding["key"] == (
        "evm:0:1:"
        + "33" * 32
        + ":0x"
        + "11" * 20
        + ":0x"
        + "22" * 20
        + ":0x"
        + "bb" * 32
        + ":0x"
        + "cc" * 32
    )
    assert evm_binding["binding_hash"] == (
        "0x3ad95ac3e5bc2892f768aae40a3b7ba673d561858b7d1318fbb9f6eba83207bf"
    )
    assert evm_sccp_destination_binding_hash(evm_input) == evm_binding["binding_hash"]
    evm_request = build_evm_sccp_proof_request(
        sample_evm_request_input(
            destination_binding=evm_binding,
            destination_binding_hash=None,
        )
    )
    assert evm_request["destination_binding_hash"] == evm_binding["binding_hash"]
    evm_submission_for_submit = build_evm_sccp_submission(
        {"proof_result": wrap_evm_sccp_proof_result(GROTH16_PROOF_BYTES, evm_request)}
    )
    evm_message_bundle = {
        "commitment": {"message_id": sample_evm_public_inputs()["message_id"]},
        "commitment_root": sample_evm_public_inputs()["commitment_root"],
    }
    evm_submit_payload = build_evm_sccp_bridge_proof_submit_payload(
        {
            "authority": "alice@sora",
            "publicKeyHex": "ed0123",
            "signatureB64": "sig",
            "messageBundle": evm_message_bundle,
            "submission": evm_submission_for_submit,
            "destinationBinding": evm_binding,
            "creationTimeMs": 123,
        }
    )
    assert evm_submit_payload["authority"] == "alice@sora"
    assert evm_submit_payload["public_key_hex"] == "ed0123"
    assert evm_submit_payload["signature_b64"] == "sig"
    assert evm_submit_payload["network_id_hex"] == evm_binding["network_id"]
    assert evm_submit_payload["verifier_address_hex"] == evm_binding["verifier_address"]
    assert evm_submit_payload["bridge_address_hex"] == evm_binding["bridge_address"]
    assert evm_submit_payload["verifier_code_hash_hex"] == evm_binding["verifier_code_hash"]
    assert evm_submit_payload["verifier_key_hash_hex"] == evm_binding["verifier_key_hash"]
    assert (
        evm_submit_payload["expected_destination_binding_hash_hex"]
        == evm_binding["binding_hash"]
    )
    assert evm_submit_payload["proof_bytes_hex"] == "0x" + GROTH16_PROOF_BYTES.hex()
    assert evm_submit_payload["creation_time_ms"] == 123
    with pytest.raises(
        TypeError,
        match=r"proofBytes\.messageId must match messageBundle\.commitment\.messageId",
    ):
        build_evm_sccp_bridge_proof_submit_payload(
            {
                "authority": "alice@sora",
                "message_bundle": {
                    **evm_message_bundle,
                    "commitment": {"message_id": HEX32_D},
                },
                "submission": evm_submission_for_submit,
                "destination_binding": evm_binding,
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofBytes\.commitmentRoot must match messageBundle\.commitmentRoot",
    ):
        build_evm_sccp_bridge_proof_submit_payload(
            {
                "authority": "alice@sora",
                "message_bundle": {
                    **evm_message_bundle,
                    "commitment_root": HEX32_D,
                },
                "submission": evm_submission_for_submit,
                "destination_binding": evm_binding,
            }
        )
    with pytest.raises(
        TypeError,
        match="submission destinationBindingHash must match destinationBinding",
    ):
        build_evm_sccp_bridge_proof_submit_payload(
            {
                "authority": "alice@sora",
                "message_bundle": {},
                "submission": {
                    **dict(evm_submission_for_submit),
                    "destination_binding_hash": HEX32_A,
                },
                "destination_binding": evm_binding,
            }
        )
    with pytest.raises(TypeError, match="submission must not use multiple aliases"):
        build_evm_sccp_bridge_proof_submit_payload(
            {
                "authority": "alice@sora",
                "message_bundle": {},
                "submission": evm_submission_for_submit,
                "sccp_submission": evm_submission_for_submit,
                "destination_binding": evm_binding,
            }
        )
    assert normalize_evm_sccp_proof_context(
        {
            "statement_hash": "0x" + "55" * 32,
            "destination_binding": evm_binding,
        }
    ) == {
        "version": 1,
        "statement_hash": "0x" + "55" * 32,
        "destination_binding_hash": evm_binding["binding_hash"],
    }
    with pytest.raises(TypeError, match="destinationBindingHash must match destinationBinding"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(
                destination_binding=evm_binding,
                destination_binding_hash=HEX32_A,
            )
        )
    with pytest.raises(TypeError, match="destinationBinding must not use multiple aliases"):
        build_evm_sccp_proof_request(
            {
                **sample_evm_request_input(
                    destination_binding=evm_binding,
                    destination_binding_hash=None,
                ),
                "destinationBinding": evm_binding,
            }
        )
    with pytest.raises(TypeError, match="destinationBindingHash must not use multiple aliases"):
        build_evm_sccp_proof_request(
            {
                **sample_evm_request_input(
                    destination_binding=evm_binding,
                    destination_binding_hash=evm_binding["binding_hash"],
                ),
                "destinationBindingHash": evm_binding["binding_hash"],
            }
        )
    with pytest.raises(TypeError, match=r"destinationBinding\.bindingHash must match"):
        evm_sccp_destination_binding({**evm_input, "binding_hash": HEX32_A})
    with pytest.raises(
        TypeError,
        match=r"destinationBinding\.networkId must not use multiple aliases",
    ):
        evm_sccp_destination_binding({**evm_input, "networkId": evm_input["network_id"]})
    with pytest.raises(
        TypeError,
        match=r"destinationBinding\.verifierAddress must not use multiple aliases",
    ):
        evm_sccp_destination_binding(
            {**evm_input, "verifierAddress": evm_input["verifier_address"]}
        )
    with pytest.raises(
        TypeError,
        match=r"destinationBinding\.bindingHash must not use multiple aliases",
    ):
        evm_sccp_destination_binding(
            {
                **evm_input,
                "binding_hash": evm_binding["binding_hash"],
                "bindingHash": evm_binding["binding_hash"],
            }
        )
    with pytest.raises(ValueError, match="verifierAddress must differ from bridgeAddress"):
        evm_sccp_destination_binding(
            {**evm_input, "bridge_address": evm_input["verifier_address"]}
        )
    for field, expected_error in (
        ("backend", "destinationBinding.verifierBackend"),
        ("proof_family", "destinationBinding.proofFamily"),
    ):
        for bad_value in ("", False, 0):
            with pytest.raises(TypeError, match=expected_error):
                evm_sccp_destination_binding({**evm_input, field: bad_value})

    tron_input = {
        "network_id": "0x" + "33" * 32,
        "verifier_address": "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        "verifier_code_hash": "0x" + "bb" * 32,
        "verifier_key_hash": "0x" + "cc" * 32,
    }
    tron_binding = tron_sccp_destination_binding(tron_input)
    assert tron_binding["key"] == (
        "tron:0:5:"
        + "33" * 32
        + ":TJRabPrwbZy45sbavfcjinPJC18kjpRTv8:0x"
        + "bb" * 32
        + ":0x"
        + "cc" * 32
    )
    assert tron_binding["binding_hash"] == (
        "0x17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f"
    )
    assert tron_sccp_destination_binding_hash(tron_input) == tron_binding["binding_hash"]
    with pytest.raises(TypeError, match=r"destinationBinding\.networkId must be canonical hex"):
        tron_sccp_destination_binding({**tron_input, "network_id": "\t" + tron_input["network_id"]})
    tron_request = build_tron_sccp_proof_request(
        sample_tron_request_input(
            destination_binding=tron_binding,
            destination_binding_hash=None,
        )
    )
    assert tron_request["destination_binding_hash"] == tron_binding["binding_hash"]
    tron_submission_for_submit = build_tron_sccp_submission(
        {"proof_result": wrap_tron_sccp_proof_result(GROTH16_PROOF_BYTES, tron_request)}
    )
    tron_message_bundle = {
        "commitment": {"message_id": sample_tron_public_inputs()["message_id"]},
        "commitment_root": sample_tron_public_inputs()["commitment_root"],
    }
    tron_submit_payload = build_tron_sccp_bridge_proof_submit_payload(
        {
            "authority": "alice@sora",
            "messageBundle": tron_message_bundle,
            "tronSccpSubmission": tron_submission_for_submit,
            "destinationBinding": tron_binding,
        }
    )
    assert tron_submit_payload["network_id_hex"] == tron_binding["network_id"]
    assert tron_submit_payload["tron_verifier_address"] == tron_binding["verifier_address"]
    assert "verifier_address_hex" not in tron_submit_payload
    assert "bridge_address_hex" not in tron_submit_payload
    assert tron_submit_payload["verifier_code_hash_hex"] == tron_binding["verifier_code_hash"]
    assert tron_submit_payload["verifier_key_hash_hex"] == tron_binding["verifier_key_hash"]
    assert (
        tron_submit_payload["expected_destination_binding_hash_hex"]
        == tron_binding["binding_hash"]
    )
    assert tron_submit_payload["proof_bytes_hex"] == "0x" + GROTH16_PROOF_BYTES.hex()
    with pytest.raises(
        TypeError,
        match=r"proofBytes\.commitmentRoot must match messageBundle\.commitmentRoot",
    ):
        build_tron_sccp_bridge_proof_submit_payload(
            {
                "authority": "alice@sora",
                "message_bundle": {
                    **tron_message_bundle,
                    "commitment_root": HEX32_D,
                },
                "submission": tron_submission_for_submit,
                "destination_binding": tron_binding,
            }
        )
    with pytest.raises(TypeError, match="submission targetDomain must match destinationBinding"):
        build_tron_sccp_bridge_proof_submit_payload(
            {
                "authority": "alice@sora",
                "message_bundle": {},
                "submission": {
                    **dict(tron_submission_for_submit),
                    "target_domain": SCCP_DOMAIN_ETH,
                },
                "destination_binding": tron_binding,
            }
        )
    with pytest.raises(TypeError, match="destinationBindingHash must match destinationBinding"):
        build_tron_sccp_proof_request(
            sample_tron_request_input(
                destination_binding=tron_binding,
                destination_binding_hash=HEX32_A,
            )
        )
    with pytest.raises(TypeError, match="destinationBinding must not use multiple aliases"):
        build_tron_sccp_proof_request(
            {
                **sample_tron_request_input(
                    destination_binding=tron_binding,
                    destination_binding_hash=None,
                ),
                "destinationBinding": tron_binding,
            }
        )
    with pytest.raises(TypeError, match=r"destinationBinding\.bindingHash must match"):
        tron_sccp_destination_binding({**tron_input, "binding_hash": HEX32_A})
    with pytest.raises(
        TypeError,
        match=r"destinationBinding\.networkId must not use multiple aliases",
    ):
        tron_sccp_destination_binding({**tron_input, "networkId": tron_input["network_id"]})
    with pytest.raises(
        TypeError,
        match=r"destinationBinding\.verifierAddress must not use multiple aliases",
    ):
        tron_sccp_destination_binding(
            {**tron_input, "verifierAddress": tron_input["verifier_address"]}
        )
    with pytest.raises(
        TypeError,
        match=r"destinationBinding\.verifierBackend must not use multiple aliases",
    ):
        tron_sccp_destination_binding(
            {
                **tron_input,
                "backend": SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1,
                "verifierBackend": SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1,
            }
        )
    with pytest.raises(TypeError, match="base58check checksum"):
        tron_sccp_destination_binding(
            {**tron_input, "verifier_address": "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9"}
        )
    with pytest.raises(TypeError, match="canonical base58check"):
        tron_sccp_destination_binding(
            {
                **tron_input,
                "verifier_address": " TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            }
        )
    for field, expected_error in (
        ("backend", "destinationBinding.verifierBackend"),
        ("proof_family", "destinationBinding.proofFamily"),
    ):
        for bad_value in ("", False, 0):
            with pytest.raises(TypeError, match=expected_error):
                tron_sccp_destination_binding({**tron_input, field: bad_value})


def sample_source_record_input(source_domain: int) -> Dict[str, Any]:
    input_value: Dict[str, Any] = {
        "source_domain": source_domain,
        "source_trust_anchor_hash": "0x" + "44" * 32,
        "consensus_verifier_hash": "0x" + "55" * 32,
        "message_inclusion_verifier_hash": "0x" + "66" * 32,
        "finality_policy_hash": "0x" + "88" * 32,
        "deployment_receipt_hash": "0x" + "aa" * 32,
    }
    if source_domain in (SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON):
        input_value["bridge_address"] = "0x" + "11" * 20
        input_value["source_bridge_emitter_code_hash"] = "0x" + "77" * 32
    if source_domain in (
        SCCP_DOMAIN_SOL,
        SCCP_DOMAIN_TON,
        SCCP_DOMAIN_SORA_KUSAMA,
        SCCP_DOMAIN_SORA_POLKADOT,
        SCCP_DOMAIN_SORA2,
    ):
        input_value["source_state_verifier_hash"] = "0x" + "77" * 32
    if source_domain == SCCP_DOMAIN_TRON:
        input_value["network_id"] = "0x" + "33" * 32
        input_value["owner_address"] = "0x" + "22" * 20
        input_value["config_hash"] = (
            "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d"
        )
    return input_value


def test_derives_source_material_and_deployment_record_hashes_for_ui_tooling() -> None:
    material_vectors = {
        SCCP_DOMAIN_ETH: "0x035c5a35f6412d45ed10389741016d067bd6d0b874a38cd744922c599e0a2fdd",
        SCCP_DOMAIN_BSC: "0x1630e4d75e2676cc443e07b0477303240ae4cff13bdf9fe61725b4a9a4ee959a",
        SCCP_DOMAIN_SOL: "0x499a7363142d5fcfe3a79b11a29ae2ad897e853649e80e39a162b8942f908331",
        SCCP_DOMAIN_TON: "0x08b11177113ac2d9f612abdf767a017de560d805e965b3dc32e28c8748ea2ebc",
        SCCP_DOMAIN_TRON: "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8",
        SCCP_DOMAIN_SORA_KUSAMA: "0x012c66498a85190d6075c441fad30fe01816796ee1713838fe8bb97f2ad1c924",
        SCCP_DOMAIN_SORA_POLKADOT: "0x40cd55d64e92d688b839242e170f1722485cddf2e42b4ff22e53c5e7723e570d",
        SCCP_DOMAIN_SORA2: "0x6fc968441106993502dd05ebeadea1dbfee0f7814680f1ad006d4584c99a8a2d",
    }
    deployment_vectors = {
        SCCP_DOMAIN_ETH: "0xd08e3344760aabfb4ba891990c852846d04a5735647174ce6e3ab0f2cad57f4d",
        SCCP_DOMAIN_BSC: "0x7d47ade779a5bddb3a5f283600af677db8605b75a00516a4328f3823ff28fb2d",
        SCCP_DOMAIN_SOL: "0xcdb2a81cb31e58d9bc1f4292d33c3f4990b2d2008dda1b9b1275aaac087461cc",
        SCCP_DOMAIN_TON: "0x5c4e226c1f4619311762a9c889f8e3b99ea6f020317c2e8a0c76a08d7a70f887",
        SCCP_DOMAIN_TRON: "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
        SCCP_DOMAIN_SORA_KUSAMA: "0xda47a31715813ef5bff0882cd0e0e8b0cc89d426e005e37e0f94a2bdba2043cd",
        SCCP_DOMAIN_SORA_POLKADOT: "0x2a57fe4beb69e8201299f2c01259a025cafc8388bb38e2a727c2fc872893e13a",
        SCCP_DOMAIN_SORA2: "0xdac819bff0aa57f7596f06297dfec39027aaab63213497020b772c355a6eaecb",
    }
    for source_domain, expected_material_hash in material_vectors.items():
        input_value = sample_source_record_input(source_domain)
        material = normalize_sccp_source_verifier_material(input_value)
        deployment = normalize_sccp_source_adapter_engine_deployment(input_value)
        assert material["placeholder_material"] is False
        assert len(canonical_sccp_source_verifier_material_bytes(material)) > 0
        assert len(canonical_sccp_source_adapter_engine_deployment_bytes(deployment)) > 0
        assert sccp_source_verifier_material_hash(input_value) == expected_material_hash
        assert (
            sccp_source_adapter_engine_deployment_hash(input_value)
            == deployment_vectors[source_domain]
        )
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        normalize_sccp_source_verifier_material(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "sourceDomain": SCCP_DOMAIN_TON,
            }
        )
    with pytest.raises(TypeError, match="sourceStateVerifierHash must not use multiple aliases"):
        normalize_sccp_source_verifier_material(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "sourceStateVerifierHash": "0x" + "99" * 32,
            }
        )
    with pytest.raises(TypeError, match="sourceBridgeNetworkId must not use multiple aliases"):
        normalize_sccp_source_verifier_material(
            {
                **sample_source_record_input(SCCP_DOMAIN_TRON),
                "sourceBridgeNetworkId": "0x" + "99" * 32,
            }
        )
    with pytest.raises(TypeError, match="sourceStateVerifierHash is not used for sourceDomain"):
        normalize_sccp_source_verifier_material(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "source_state_verifier_hash": "0x" + "77" * 32,
            }
        )
    with pytest.raises(
        TypeError,
        match="sourceBridgeEmitterAddress is not used for sourceDomain",
    ):
        normalize_sccp_source_verifier_material(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "bridge_address": "0x" + "11" * 20,
            }
        )
    with pytest.raises(TypeError, match="sourceBridgeNetworkId is not used for sourceDomain"):
        normalize_sccp_source_verifier_material(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "network_id": "0x" + "33" * 32,
            }
        )
    with pytest.raises(TypeError, match="targetDomain must not use multiple aliases"):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "targetDomain": SCCP_DOMAIN_SORA,
                "target_domain": SCCP_DOMAIN_SORA,
            }
        )
    with pytest.raises(
        TypeError,
        match="solanaTowerReplayVerifierHash must not use multiple aliases",
    ):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "solanaTowerReplayVerifierHash": "0x" + "bb" * 32,
                "solana_tower_replay_verifier_hash": "0x" + "bc" * 32,
            }
        )
    with pytest.raises(
        TypeError,
        match="tonMasterchainConfigVerifierHash must not use multiple aliases",
    ):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_TON),
                "tonMasterchainConfigVerifierHash": "0x" + "bb" * 32,
                "ton_masterchain_config_verifier_hash": "0x" + "bc" * 32,
            }
        )
    with pytest.raises(TypeError, match="solanaTowerReplayVerifierHash"):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "solana_tower_replay_verifier_hash": None,
            }
        )
    with pytest.raises(TypeError, match="tonMasterchainConfigVerifierHash"):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_TON),
                "ton_masterchain_config_verifier_hash": None,
            }
        )
    with pytest.raises(TypeError, match="deploymentReceiptHash must not use multiple aliases"):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "deploymentReceiptHash": "0x" + "99" * 32,
            }
        )
    for field, template_hash in TON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.items():
        with pytest.raises(TypeError, match=r"TON template (verifier|component) hash"):
            normalize_sccp_source_verifier_material(
                {
                    **sample_source_record_input(SCCP_DOMAIN_TON),
                    field: template_hash,
                }
            )
    for field, template_hash in TRON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.items():
        with pytest.raises(TypeError, match=r"TRON template component hash"):
            normalize_sccp_source_verifier_material(
                {
                    **sample_source_record_input(SCCP_DOMAIN_TRON),
                    field: template_hash,
                }
            )
    for field, template_hash in SOLANA_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.items():
        with pytest.raises(TypeError, match=r"Solana template (verifier|component) hash"):
            normalize_sccp_source_verifier_material(
                {
                    **sample_source_record_input(SCCP_DOMAIN_SOL),
                    field: template_hash,
                }
            )
    with pytest.raises(TypeError, match=r"TRON source bridge config fields"):
        normalize_sccp_source_verifier_material(
            {
                **sample_source_record_input(SCCP_DOMAIN_TRON),
                "config_hash": "0x" + "99" * 32,
            }
        )
    with pytest.raises(ValueError, match="role-separated"):
        normalize_sccp_source_verifier_material(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "consensus_verifier_hash": "0x" + "44" * 32,
            }
        )
    with pytest.raises(ValueError, match="role-separated"):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "deployment_receipt_hash": sccp_source_adapter_verifier_vk_hash(
                    SCCP_DOMAIN_ETH,
                    target_domain=SCCP_DOMAIN_SORA,
                ),
            }
        )
    with pytest.raises(TypeError, match="adapterProofFamily"):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "adapter_proof_family": "",
            }
        )
    with pytest.raises(TypeError, match="adapterProofFamily"):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "adapter_proof_family": None,
            }
        )
    with pytest.raises(TypeError, match="targetDomain must be a u32 domain id"):
        normalize_sccp_source_adapter_engine_deployment(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "target_domain": None,
            }
        )

    audited_solana_deployment = {
        **sample_source_record_input(SCCP_DOMAIN_SOL),
        "solana_tower_replay_verifier_hash": "0x" + "bb" * 32,
        "solana_full_accountsdb_lattice_verifier_hash": "0x" + "cc" * 32,
        "solana_bank_fork_choice_verifier_hash": "0x" + "dd" * 32,
    }
    assert (
        sccp_source_adapter_engine_deployment_hash(audited_solana_deployment)
        == "0x97e5c4196aff6387b9d973e663de3ce9345e1d8c3de89d22505b2197e282dc61"
    )
    assert (
        sccp_solana_full_light_client_gate_hash(audited_solana_deployment)
        == "0x2c94b86a665bb68708b762c678661f5e9879bd588627e93a640796eeaef970f9"
    )
    with pytest.raises(ValueError, match="audited Solana -> SORA deployment"):
        sccp_solana_full_light_client_gate_hash(
            sample_source_record_input(SCCP_DOMAIN_SOL)
        )
    with pytest.raises(ValueError, match="Solana audit verifier hashes"):
        sccp_source_adapter_engine_deployment_hash(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "solana_tower_replay_verifier_hash": "0x" + "bb" * 32,
            }
        )
    with pytest.raises(ValueError, match="role-separated"):
        sccp_source_adapter_engine_deployment_hash(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "solana_tower_replay_verifier_hash": "0x" + "bb" * 32,
                "solana_full_accountsdb_lattice_verifier_hash": "0x" + "bb" * 32,
                "solana_bank_fork_choice_verifier_hash": "0x" + "dd" * 32,
            }
        )
    with pytest.raises(ValueError, match="source_state_verifier_hash"):
        sccp_solana_full_light_client_gate_hash(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "solana_tower_replay_verifier_hash": "0x" + "77" * 32,
                "solana_full_accountsdb_lattice_verifier_hash": "0x" + "cc" * 32,
                "solana_bank_fork_choice_verifier_hash": "0x" + "dd" * 32,
            }
        )
    for role_field in ("adapter_verifier_vk_hash", "deployment_receipt_hash"):
        source_record = sample_source_record_input(SCCP_DOMAIN_SOL)
        reused_hash = (
            sccp_source_adapter_verifier_vk_hash(
                SCCP_DOMAIN_SOL,
                target_domain=SCCP_DOMAIN_SORA,
            )
            if role_field == "adapter_verifier_vk_hash"
            else source_record[role_field]
        )
        with pytest.raises(ValueError, match=role_field):
            sccp_solana_full_light_client_gate_hash(
                {
                    **source_record,
                    "solana_tower_replay_verifier_hash": reused_hash,
                    "solana_full_accountsdb_lattice_verifier_hash": "0x" + "cc" * 32,
                    "solana_bank_fork_choice_verifier_hash": "0x" + "dd" * 32,
                }
            )
    with pytest.raises(ValueError, match="template material"):
        sccp_solana_full_light_client_gate_hash(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "solana_tower_replay_verifier_hash": (
                    SOLANA_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES[
                        "source_trust_anchor_hash"
                    ]
                ),
                "solana_full_accountsdb_lattice_verifier_hash": "0x" + "cc" * 32,
                "solana_bank_fork_choice_verifier_hash": "0x" + "dd" * 32,
            }
        )
    with pytest.raises(ValueError, match="only used for Solana deployments"):
        sccp_source_adapter_engine_deployment_hash(
            {
                **sample_source_record_input(SCCP_DOMAIN_TON),
                "solana_tower_replay_verifier_hash": "0x" + "bb" * 32,
                "solana_full_accountsdb_lattice_verifier_hash": "0x" + "cc" * 32,
                "solana_bank_fork_choice_verifier_hash": "0x" + "dd" * 32,
            }
        )

    audited_ton_deployment = {
        **sample_source_record_input(SCCP_DOMAIN_TON),
        "ton_masterchain_config_verifier_hash": "0x" + "bb" * 32,
        "ton_validator_set_transition_verifier_hash": "0x" + "cc" * 32,
        "ton_shard_accounts_dictionary_verifier_hash": "0x" + "dd" * 32,
    }
    assert (
        sccp_source_adapter_engine_deployment_hash(audited_ton_deployment)
        == "0x61e5d710ccbc902be00a38a5a80d05c19de97105605a3f93d4f8067862d81f07"
    )
    assert (
        sccp_ton_full_light_client_gate_hash(audited_ton_deployment)
        == "0xc32d8cfc2e273646abb00911b9a15e7ee0ab1721b04a6e89a060422dd3cc4596"
    )
    with pytest.raises(ValueError, match="audited TON -> SORA deployment"):
        sccp_ton_full_light_client_gate_hash(sample_source_record_input(SCCP_DOMAIN_TON))
    with pytest.raises(ValueError, match="TON audit verifier hashes"):
        sccp_source_adapter_engine_deployment_hash(
            {
                **sample_source_record_input(SCCP_DOMAIN_TON),
                "ton_masterchain_config_verifier_hash": "0x" + "bb" * 32,
            }
        )
    with pytest.raises(ValueError, match="only used for TON deployments"):
        sccp_source_adapter_engine_deployment_hash(
            {
                **sample_source_record_input(SCCP_DOMAIN_SOL),
                "ton_masterchain_config_verifier_hash": "0x" + "bb" * 32,
                "ton_validator_set_transition_verifier_hash": "0x" + "cc" * 32,
                "ton_shard_accounts_dictionary_verifier_hash": "0x" + "dd" * 32,
            }
        )

    with pytest.raises(ValueError, match="canonical source-adapter verifier profile"):
        sccp_source_adapter_engine_deployment_hash(
            {
                **sample_source_record_input(SCCP_DOMAIN_ETH),
                "adapter_verifier_vk_hash": "0x" + "99" * 32,
            }
        )


def sample_ton_full_light_client_audit_proof_input(**overrides: Any) -> Dict[str, Any]:
    config_leaf = {
        "source_domain": SCCP_DOMAIN_TON,
        "masterchain_seqno": "19",
        "masterchain_block_hash": HEX32_A,
        "shard_state_root": TON_SHARD_STATE_ROOT_HASH,
        "validator_set_hash": TON_VALIDATOR_SET_HASH,
        "validator_set_payload_hash": TON_VALIDATOR_SET_PAYLOAD_HASH,
    }
    config_leaf_hash = ton_masterchain_config_leaf_hash(config_leaf)
    config_proof = {
        **config_leaf,
        "config_root": TON_MASTERCHAIN_CONFIG_ROOT,
        "config_leaf_hash": config_leaf_hash,
        "config_leaf_index": str(SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM),
        "config_value_hash": TON_MASTERCHAIN_CONFIG_VALUE_HASH,
        "config_dictionary_proof_boc": TON_MASTERCHAIN_CONFIG_PROOF_BOC,
        "config_inclusion_branch": [],
    }
    value = {
        **sample_source_record_input(SCCP_DOMAIN_TON),
        "source_state_verifier_hash": "0x" + "d4" * 32,
        "source_trust_anchor_hash": TON_VALIDATOR_SET_HASH,
        "consensus_verifier_hash": "0x" + "b2" * 32,
        "message_inclusion_verifier_hash": "0x" + "c3" * 32,
        "finality_policy_hash": "0x" + "c4" * 32,
        "ton_masterchain_config_verifier_hash": "0x" + "b1" * 32,
        "ton_validator_set_transition_verifier_hash": "0x" + "c2" * 32,
        "ton_shard_accounts_dictionary_verifier_hash": "0x" + "d3" * 32,
        "masterchain_seqno": "19",
        "masterchain_workchain_id": -1,
        "masterchain_shard": str(0x8000_0000_0000_0000),
        "masterchain_block_hash": HEX32_A,
        "masterchain_file_hash": "0x" + "a5" * 32,
        "validator_set_hash": TON_VALIDATOR_SET_HASH,
        "masterchain_config_root": TON_MASTERCHAIN_CONFIG_ROOT,
        "masterchain_config_proof_hash": ton_masterchain_config_proof_hash(config_proof),
        "validator_set_payload_hash": TON_VALIDATOR_SET_PAYLOAD_HASH,
        "config_leaf_hash": config_leaf_hash,
        "config_value_hash": TON_MASTERCHAIN_CONFIG_VALUE_HASH,
        "shard_workchain_id": 0,
        "shard_shard": str(0x8000_0000_0000_0000),
        "shard_seqno": "7",
        "shard_block_hash": HEX32_B,
        "shard_file_hash": "0x" + "bc" * 32,
        "shard_state_root": TON_SHARD_STATE_ROOT_HASH,
        "transaction_root": TON_HASHMAP_E_VALUE_HASH,
        "transaction_lt": "7",
        "shard_state_proof_boc": TON_SHARD_STATE_PROOF_BOC,
        "shard_state_dictionary_root": TON_SHARD_ACCOUNTS_ROOT_HASH,
        "shard_state_dictionary_key_bit_len": "256",
        "shard_state_dictionary_key": TON_SHARD_ACCOUNT_KEY,
        "shard_state_dictionary_proof_boc": TON_SHARD_ACCOUNTS_BOC,
        "masterchain_signature_hash": TON_MASTERCHAIN_SIGNATURES_HASH,
        "shard_proof_hash": "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf",
        "config_dictionary_proof_boc": TON_MASTERCHAIN_CONFIG_PROOF_BOC,
        "validator_set_transition_proofs": [],
        "shard_state_verification_proof": {
            "version": 1,
            "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
            "circuit_id": SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
            "proof_bytes": b"\x11\x22\x33\x44",
        },
    }
    value.update(overrides)
    return value


def test_builds_ton_full_light_client_audit_role_proof_requests() -> None:
    input_value = sample_ton_full_light_client_audit_proof_input()
    requests = build_ton_sccp_full_light_client_audit_proof_requests(input_value)
    shard_state_public_inputs_hash = ton_shard_state_proof_public_inputs_hash(input_value)
    shard_state_proof_hash = ton_sccp_shard_state_verification_proof_hash(
        input_value["shard_state_verification_proof"]
    )
    assert canonical_ton_sccp_source_state_verification_proof_bytes(
        input_value["shard_state_verification_proof"]
    )

    assert list(requests) == [
        "masterchain_config",
        "validator_set_transition",
        "shard_accounts_dictionary",
    ]
    with pytest.raises(TypeError, match="immutable"):
        requests["masterchain_config"] = {}
    assert requests["masterchain_config"]["circuit_id"] == (
        SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert requests["validator_set_transition"]["circuit_id"] == (
        SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert requests["shard_accounts_dictionary"]["circuit_id"] == (
        SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert len({request["circuit_id"] for request in requests.values()}) == 3
    assert len(canonical_ton_sccp_full_light_client_audit_statement_bytes(
        input_value,
        "masterchain_config",
    )) > 0
    for request in requests.values():
        assert_immutable_fastpq_proof_request(
            request,
            ("statement_bytes", "verification_context_bytes", "schema_descriptor"),
        )
        assert request["version"] == 1
        assert request["proof_family"] == SCCP_STARK_FRI_PROOF_FAMILY_V1
        assert request["parameter_set"] == "fastpq-lane-balanced"
        assert request["source_domain"] == SCCP_DOMAIN_TON
        assert request["masterchain_seqno"] == "19"
        assert request["shard_seqno"] == "7"
        assert request["source_state_verifier_id"] == SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1
        assert request["full_light_client_gate_hash"] == sccp_ton_full_light_client_gate_hash(input_value)
        assert request["shard_state_proof_public_inputs_hash"] == shard_state_public_inputs_hash
        assert request["shard_state_verification_proof_hash"] == shard_state_proof_hash
        assert request["audit_statement_hash"] == (
            ton_sccp_full_light_client_audit_statement_hash(input_value, request["role"])
        )
        assert request["schema_descriptor"] == (
            ton_sccp_full_light_client_audit_open_verify_schema_descriptor(
                input_value,
                request["role"],
            )
        )
        assert request["public_input_columns"] == (
            ton_sccp_full_light_client_audit_public_input_columns(
                input_value,
                request["role"],
            )
        )
        expected_column_count = 16 if request["role"] == "validator_set_transition" else 17
        assert len(request["public_input_columns"]) == expected_column_count
        assert len(request["fastpq_transitions"]) == 3
        assert request["fastpq_transitions"] == sorted(
            request["fastpq_transitions"],
            key=lambda item: item["key"],
        )
        assert all(transition["key"].startswith("0x") for transition in request["fastpq_transitions"])
    assert requests["masterchain_config"]["fastpq_public_inputs"]["old_root"] == (
        TON_MASTERCHAIN_CONFIG_ROOT
    )
    assert requests["validator_set_transition"]["fastpq_public_inputs"]["old_root"] == (
        TON_VALIDATOR_SET_HASH
    )
    assert requests["shard_accounts_dictionary"]["fastpq_public_inputs"]["new_root"] == (
        TON_HASHMAP_E_VALUE_HASH
    )
    with pytest.raises(ValueError, match="role-separated"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                ton_validator_set_transition_verifier_hash="0x" + "b1" * 32,
            )
        )
    with pytest.raises(ValueError, match="must not reuse"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                ton_masterchain_config_verifier_hash="0x" + "d4" * 32,
            )
        )
    request_hash_replay_input = sample_ton_full_light_client_audit_proof_input()
    request_hash_replay = ton_sccp_full_light_client_audit_statement_hash(
        request_hash_replay_input,
        "masterchain_config",
    )
    with pytest.raises(ValueError, match="request-bound hashes"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                ton_masterchain_config_verifier_hash=request_hash_replay,
            )
        )
    with pytest.raises(ValueError, match="built-in template material"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                ton_masterchain_config_verifier_hash=TON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES[
                    "source_trust_anchor_hash"
                ],
            )
        )
    with pytest.raises(TypeError, match="shardStateVerificationProofHash"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                shard_state_verification_proof_hash=HEX32_A,
            )
        )
    hash_only_input = sample_ton_full_light_client_audit_proof_input(
        shard_state_verification_proof_hash=shard_state_proof_hash,
    )
    del hash_only_input["shard_state_verification_proof"]
    with pytest.raises(TypeError, match="shardStateVerificationProof is required"):
        build_ton_sccp_full_light_client_audit_proof_requests(hash_only_input)
    with pytest.raises(TypeError, match="TON source-state"):
        canonical_ton_sccp_source_state_verification_proof_bytes(
            {
                **input_value["shard_state_verification_proof"],
                "circuit_id": SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
            }
        )
    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        canonical_ton_sccp_source_state_verification_proof_bytes(
            {
                **input_value["shard_state_verification_proof"],
                "proof_bytes": b"\0\0\0",
            }
        )
    with pytest.raises(
        TypeError,
        match=r"sourceStateProof\.proofBase64 must not use multiple aliases",
    ):
        canonical_ton_sccp_source_state_verification_proof_bytes(
            {
                **input_value["shard_state_verification_proof"],
                "proof_base64": base64.b64encode(
                    input_value["shard_state_verification_proof"]["proof_bytes"]
                ).decode("ascii"),
                "proofBase64": base64.b64encode(
                    input_value["shard_state_verification_proof"]["proof_bytes"]
                ).decode("ascii"),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"sourceStateProof\.circuitId must not use multiple aliases",
    ):
        canonical_ton_sccp_source_state_verification_proof_bytes(
            {
                **input_value["shard_state_verification_proof"],
                "circuitId": input_value["shard_state_verification_proof"][
                    "circuit_id"
                ],
            }
        )
    with pytest.raises(
        TypeError,
        match=r"sourceStateProof\.proofBytes must not use multiple aliases",
    ):
        canonical_ton_sccp_source_state_verification_proof_bytes(
            {
                **input_value["shard_state_verification_proof"],
                "proofBytes": input_value["shard_state_verification_proof"][
                    "proof_bytes"
                ],
            }
        )
    with pytest.raises(TypeError, match="masterchainConfigProofHash"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                masterchain_config_proof_hash=HEX32_A,
            )
        )
    with pytest.raises(TypeError, match="validatorSetPayloadHash must not use multiple aliases"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                validatorSetPayloadHash=TON_VALIDATOR_SET_PAYLOAD_HASH,
            )
        )
    with pytest.raises(
        TypeError,
        match="configLeafHash must not use top-level and masterchainConfigProof aliases",
    ):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                masterchainConfigProof={
                    "configLeafHash": input_value["config_leaf_hash"],
                },
            )
        )
    with pytest.raises(TypeError, match="sourceVerifierMaterial must not use multiple aliases"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                sourceVerifierMaterial={},
                source_verifier_material={},
            )
        )
    with pytest.raises(TypeError, match="sourceAdapterDeployment must not use multiple aliases"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                sourceAdapterDeployment={},
                source_adapter_deployment={},
            )
        )
    with pytest.raises(TypeError, match="masterchainConfigProof must not use multiple aliases"):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                masterchainConfigProof={},
                masterchain_config_proof={},
            )
        )
    with pytest.raises(
        TypeError,
        match="shardStateProofPublicInputsHash must not use multiple aliases",
    ):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                shardStateProofPublicInputsHash=shard_state_public_inputs_hash,
                shard_state_proof_public_inputs_hash=shard_state_public_inputs_hash,
            )
        )
    with pytest.raises(
        TypeError,
        match="shardStateVerificationProofHash must not use multiple aliases",
    ):
        build_ton_sccp_full_light_client_audit_proof_requests(
            sample_ton_full_light_client_audit_proof_input(
                shardStateVerificationProofHash=shard_state_proof_hash,
                shard_state_verification_proof_hash=shard_state_proof_hash,
            )
        )


def test_ton_source_state_prover_wraps_shard_and_full_light_audit_role_proofs() -> None:
    input_value = sample_ton_full_light_client_audit_proof_input()
    shard_request = build_ton_shard_state_proof_request(input_value)
    wrapped_shard = wrap_ton_sccp_source_state_verification_proof(
        b"\x09\x08\x07",
        shard_request,
    )
    assert wrapped_shard["circuit_id"] == SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1
    assert wrapped_shard["proof_base64"] == "CQgH"
    assert canonical_ton_sccp_source_state_verification_proof_bytes(wrapped_shard)

    audit_requests = build_ton_sccp_full_light_client_audit_proof_requests(input_value)
    wrapped_audit = wrap_ton_sccp_source_state_verification_proof(
        b"\x01\x02\x03",
        audit_requests["masterchain_config"],
    )
    assert wrapped_audit["circuit_id"] == SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1
    assert canonical_ton_sccp_source_state_verification_proof_bytes(wrapped_audit)
    with pytest.raises(TypeError, match="all zero"):
        wrap_ton_sccp_source_state_verification_proof(b"\0\0", shard_request)
    tampered_shard_request = mutable_proof_request(shard_request)
    tampered_shard_request["fastpq_transitions"][0]["new_value"] = "0x00"
    with pytest.raises(TypeError, match="canonical TON source-state request"):
        wrap_ton_sccp_source_state_verification_proof(b"\x09\x08\x07", tampered_shard_request)
    tampered_shard_hash_request = mutable_proof_request(shard_request)
    tampered_shard_hash_request["shard_state_proof_public_inputs_hash"] = HEX32_A
    with pytest.raises(TypeError, match="statementBytes"):
        wrap_ton_sccp_source_state_verification_proof(b"\x09\x08\x07", tampered_shard_hash_request)
    tampered_shard_dsid_request = mutable_proof_request(shard_request)
    tampered_shard_dsid_request["fastpq_public_inputs"]["dsid"] = "0x" + "00" * 16
    with pytest.raises(TypeError, match="fastpqPublicInputs.dsid"):
        wrap_ton_sccp_source_state_verification_proof(b"\x09\x08\x07", tampered_shard_dsid_request)
    duplicate_shard_alias_request = mutable_proof_request(shard_request)
    duplicate_shard_alias_request["sourceDomain"] = duplicate_shard_alias_request["source_domain"]
    with pytest.raises(TypeError, match="multiple aliases"):
        wrap_ton_sccp_source_state_verification_proof(b"\x09\x08\x07", duplicate_shard_alias_request)
    duplicate_shard_fastpq_alias_request = mutable_proof_request(shard_request)
    duplicate_shard_fastpq_alias_request["fastpq_public_inputs"]["txSetHash"] = (
        duplicate_shard_fastpq_alias_request["fastpq_public_inputs"]["tx_set_hash"]
    )
    with pytest.raises(TypeError, match="multiple aliases"):
        wrap_ton_sccp_source_state_verification_proof(
            b"\x09\x08\x07",
            duplicate_shard_fastpq_alias_request,
        )
    tampered_audit_request = mutable_proof_request(audit_requests["masterchain_config"])
    tampered_audit_request["fastpq_transitions"][0]["new_value"] = "0x00"
    with pytest.raises(TypeError, match="canonical TON source-state request"):
        wrap_ton_sccp_source_state_verification_proof(b"\x09\x08\x07", tampered_audit_request)
    tampered_audit_hash_request = mutable_proof_request(audit_requests["masterchain_config"])
    tampered_audit_hash_request["audit_statement_hash"] = HEX32_A
    with pytest.raises(TypeError, match="statementBytes"):
        wrap_ton_sccp_source_state_verification_proof(b"\x09\x08\x07", tampered_audit_hash_request)
    tampered_audit_tx_request = mutable_proof_request(audit_requests["masterchain_config"])
    tampered_audit_tx_request["fastpq_public_inputs"]["tx_set_hash"] = HEX32_A
    with pytest.raises(TypeError, match="fastpqPublicInputs.txSetHash"):
        wrap_ton_sccp_source_state_verification_proof(b"\x09\x08\x07", tampered_audit_tx_request)

    preflight_callback_invoked = False

    def preflight_prove(
        _request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Mapping[str, Any]:
        nonlocal preflight_callback_invoked
        preflight_callback_invoked = True
        return {"proof_bytes": b"\x09\x08\x07"}

    preflight_prover = TonSccpSourceStateProver(prove=preflight_prove)
    with pytest.raises(TypeError, match="canonical TON source-state request"):
        asyncio.run(preflight_prover.prove_request(tampered_shard_request))
    assert not preflight_callback_invoked
    with pytest.raises(TypeError, match="canonical TON source-state request"):
        asyncio.run(preflight_prover.prove_request(tampered_audit_request))
    assert not preflight_callback_invoked

    roles = []

    def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> Mapping[str, Any]:
        roles.append(request.get("role", "shard_state"))
        result: dict[str, Any] = {
            "proof_bytes": b"\x09\x08\x07",
            "version": 1,
            "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
            "circuit_id": request["circuit_id"],
            "proof_base64": "CQgH",
            "parameter_set": request["parameter_set"],
            "source_domain": request["source_domain"],
            "masterchain_seqno": request["masterchain_seqno"],
            "shard_seqno": request["shard_seqno"],
            "source_state_verifier_id": request["source_state_verifier_id"],
            "source_state_verifier_hash": request["source_state_verifier_hash"],
            "shard_state_proof_public_inputs_hash": (
                request["shard_state_proof_public_inputs_hash"]
            ),
            "public_input_columns": request["public_input_columns"],
            "fastpq_public_inputs": request["fastpq_public_inputs"],
            "fastpq_transitions": request["fastpq_transitions"],
            "statement_bytes": request["statement_bytes"],
            "verification_context_bytes": request["verification_context_bytes"],
            "schema_descriptor": request["schema_descriptor"],
        }
        if "role" in request:
            result.update({
                "role": request["role"],
                "role_code": request["role_code"],
                "verifier_id": request["verifier_id"],
                "verifier_hash": request["verifier_hash"],
                "source_verifier_material_hash": request["source_verifier_material_hash"],
                "source_adapter_deployment_hash": request["source_adapter_deployment_hash"],
                "full_light_client_gate_hash": request["full_light_client_gate_hash"],
                "shard_state_verification_proof_hash": (
                    request["shard_state_verification_proof_hash"]
                ),
                "audit_statement_hash": request["audit_statement_hash"],
            })
        else:
            result["witness_commitment_bytes"] = request["witness_commitment_bytes"]
        return result

    prover = TonSccpSourceStateProver(prove=prove)
    shard_proof = asyncio.run(prover.prove_shard_state(input_value))
    audit_proofs = asyncio.run(prover.prove_full_light_client_audit(input_value))

    assert shard_proof["circuit_id"] == SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1
    assert list(audit_proofs) == [
        "masterchain_config",
        "validator_set_transition",
        "shard_accounts_dictionary",
    ]
    assert roles == [
        "shard_state",
        "masterchain_config",
        "validator_set_transition",
        "shard_accounts_dictionary",
    ]
    assert audit_proofs["validator_set_transition"]["circuit_id"] == (
        SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert audit_proofs["shard_accounts_dictionary"]["circuit_id"] == (
        SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1
    )
    assert audit_proofs["shard_accounts_dictionary"]["proof_base64"] == "CQgH"

    with pytest.raises(TonSccpSourceStateProverUnavailableError) as exc:
        asyncio.run(TonSccpSourceStateProver().prove_request(shard_request))
    assert exc.value.code == "ERR_SCCP_TON_SOURCE_STATE_PROVER_UNAVAILABLE"
    with pytest.raises(TypeError, match=r"result\.proofFamily"):
        asyncio.run(
            TonSccpSourceStateProver(
                prove=lambda _request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "proof_family": "debug-proof-family",
                }
            ).prove_shard_state(input_value)
        )
    with pytest.raises(
        TypeError,
        match=r"source-state prover result\.statementBytes must match request\.statementBytes",
    ):
        asyncio.run(
            TonSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "statement_bytes": bytes([request["statement_bytes"][0] ^ 0xFF]),
                }
            ).prove_shard_state(input_value)
        )
    with pytest.raises(TypeError, match=r"result\.proofBase64"):
        asyncio.run(
            TonSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "circuit_id": request["circuit_id"],
                    "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
                    "proof_base64": "AAAA",
                    "version": 1,
                }
            ).prove_shard_state(input_value)
        )
    with pytest.raises(
        TypeError,
        match=(
            r"source-state prover result\.masterchainSeqno must match "
            r"request\.masterchainSeqno"
        ),
    ):
        asyncio.run(
            TonSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "masterchain_seqno": str(int(request["masterchain_seqno"]) + 1),
                }
            ).prove_shard_state(input_value)
        )
    with pytest.raises(
        TypeError,
        match=(
            r"source-state prover result\.shardStateVerificationProofHash "
            r"must match request\.shardStateVerificationProofHash"
        ),
    ):
        asyncio.run(
            TonSccpSourceStateProver(
                prove=lambda request, _options: {
                    "proof_bytes": b"\x01\x02\x03",
                    "shard_state_verification_proof_hash": (
                        HEX32_B
                        if request["shard_state_verification_proof_hash"] == HEX32_A
                        else HEX32_A
                    ),
                }
            ).prove_full_light_client_audit(input_value)
        )


def test_ton_source_state_prover_snapshots_mutable_callback_requests() -> None:
    built_request = build_ton_shard_state_proof_request(
        sample_ton_full_light_client_audit_proof_input()
    )
    mutable_request = mutable_proof_request(built_request)

    def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> bytes:
        assert request is not mutable_request
        with pytest.raises(TypeError, match="immutable"):
            request["statement_bytes"] = b""  # type: ignore[index]
        mutable_request["statement_bytes"] = b""
        mutable_request["circuit_id"] = SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1
        return b"\x04\x05\x06"

    proof = asyncio.run(TonSccpSourceStateProver(prove=prove).prove_request(mutable_request))

    assert proof["circuit_id"] == SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1
    assert proof["proof_base64"] == "BAUG"


def test_builds_ton_sccp_message_body_submission_boc() -> None:
    with pytest.raises(TypeError, match="proofResult must be a wrapped TON SCCP proof result"):
        build_ton_sccp_message_body_boc(sample_ton_message_body_input())
    with pytest.raises(TypeError, match="proofResult must be a wrapped TON SCCP proof result"):
        build_ton_sccp_submission(sample_ton_message_body_input())

    body = build_ton_sccp_message_body_boc(sample_ton_message_body_input_with_result())

    assert body[:4] == bytes([0xB5, 0xEE, 0x9C, 0x72])
    assert len(body) > len(
        canonical_sccp_message_transparent_public_inputs_bytes(sample_ton_public_inputs())
    )
    assert ton_sccp_submission_query_id(sample_ton_public_inputs()) == int("dd" * 8, 16)
    assert len(ton_boc_single_root_hash(body)) == 66

    submission = build_ton_sccp_submission(sample_ton_message_body_input_with_result())
    assert submission["version"] == 1
    assert submission["envelope_encoding"] == SCCP_TON_MESSAGE_BODY_BOC_V1
    assert submission["submission_kind"] == "internal_message"
    assert submission["verifier_entrypoint"] == "op::submit_sccp_message_proof"
    assert submission["message_body_boc"] == body
    assert submission["message_body_boc_hex"] == submission["envelope_hex"]
    assert submission["arguments"][0]["key"] == "message_body_boc"
    assert submission["arguments"][0]["encoding"] == "ton_boc"
    assert submission["arguments"][0]["bytes"] == submission["message_body_boc_hex"]
    with pytest.raises(TypeError, match="immutable"):
        submission["message_body_boc_hex"] = HEX32_A
    with pytest.raises(TypeError, match="immutable"):
        submission["arguments"].append({"key": "tampered"})

    manifest = {
        "version": 1,
        "local_domain": SCCP_DOMAIN_SORA,
        "counterparty_domain": SCCP_DOMAIN_TON,
        "security_model": "RecursiveZk",
        "anchor_governance": "CryptographicProof",
        "verifier_target": "TonContract",
        "verifier_backend_family": "TonContract",
        "proof_family": SCCP_STARK_FRI_PROOF_FAMILY_V1,
        "verifier_backend_key": SCCP_TON_CONTRACT_PROOF_BACKEND_V1,
        "message_backend": "sccp-message-v1",
        "registry_backend": "sccp-registry-v1",
        "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
    }
    destination_binding = {"key": "sora:ton", "binding_hash": HEX32_H}
    metadata = canonical_sccp_ton_submission_metadata_bytes(
        {
            "manifest": manifest,
            "destination_binding": destination_binding,
            "destination_binding_hash": HEX32_H,
            "public_inputs": sample_ton_public_inputs(),
            "statement_hash": HEX32_B,
        }
    )
    assert len(metadata) > len(
        canonical_sccp_message_transparent_public_inputs_bytes(sample_ton_public_inputs())
    )
    manifest_with_binding = {**manifest, "destination_binding": destination_binding}
    assert (
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest_with_binding,
                "destination_binding": destination_binding,
                "destination_binding_hash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
        == metadata
    )
    with pytest.raises(TypeError, match="destinationBinding must not use multiple aliases"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest_with_binding,
                "destination_binding": destination_binding,
                "destinationBinding": destination_binding,
                "destination_binding_hash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match=r"destinationBinding\.bindingHash must not use multiple aliases"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest_with_binding,
                "destination_binding": {
                    **destination_binding,
                    "bindingHash": HEX32_H,
                },
                "destination_binding_hash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match="destinationBindingHash must not use multiple aliases"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest_with_binding,
                "destination_binding": destination_binding,
                "destination_binding_hash": HEX32_H,
                "destinationBindingHash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match="publicInputs must not use multiple aliases"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest_with_binding,
                "destination_binding": destination_binding,
                "destination_binding_hash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "publicInputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match="statementHash must not use multiple aliases"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest_with_binding,
                "destination_binding": destination_binding,
                "destination_binding_hash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
                "statementHash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match=r"manifest\.localDomain must not use multiple aliases"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": {**manifest_with_binding, "localDomain": SCCP_DOMAIN_SORA},
                "destination_binding": destination_binding,
                "destination_binding_hash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match="messageBackend must not use multiple aliases"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": {
                    **manifest_with_binding,
                    "messageBackend": manifest_with_binding["message_backend"],
                },
                "destination_binding": destination_binding,
                "destination_binding_hash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match="verifierBackendKey must not use multiple aliases"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": {
                    **manifest_with_binding,
                    "verifierBackend": {"key": SCCP_TON_CONTRACT_PROOF_BACKEND_V1},
                },
                "destination_binding": destination_binding,
                "destination_binding_hash": HEX32_H,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match=r"destinationBindingHash must match destinationBinding\.bindingHash"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest_with_binding,
                "destination_binding": destination_binding,
                "destination_binding_hash": HEX32_A,
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    for bad_manifest in ({}, False, 0, ""):
        with pytest.raises(TypeError, match="TON SCCP manifest|localDomain"):
            canonical_sccp_ton_submission_metadata_bytes(
                {
                    "manifest": bad_manifest,
                    "destination_binding": destination_binding,
                    "public_inputs": sample_ton_public_inputs(),
                    "statement_hash": HEX32_B,
                }
            )
    for bad_manifest, expected in (
        ({**manifest, "local_domain": SCCP_DOMAIN_TON}, "manifest.localDomain must be SORA"),
        (
            {**manifest, "counterparty_domain": SCCP_DOMAIN_SOL},
            "manifest.counterpartyDomain must be TON",
        ),
        ({**manifest, "proof_family": "debug-proof"}, "proofFamily must be stark-fri-v1"),
        (
            {**manifest, "verifier_backend_key": "debug-ton-contract"},
            "verifierBackendKey must be ton-contract-v1",
        ),
    ):
        with pytest.raises(TypeError, match=expected):
            canonical_sccp_ton_submission_metadata_bytes(
                {
                    "manifest": bad_manifest,
                    "destination_binding": destination_binding,
                    "public_inputs": sample_ton_public_inputs(),
                    "statement_hash": HEX32_B,
                }
            )
    with pytest.raises(TypeError, match=r"destinationBinding must match manifest\.destinationBinding"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest_with_binding,
                "destination_binding": {**destination_binding, "binding_hash": HEX32_A},
                "public_inputs": sample_ton_public_inputs(),
                "statement_hash": HEX32_B,
            }
        )
    with pytest.raises(TypeError, match=r"publicInputs\.targetDomain must be TON"):
        canonical_sccp_ton_submission_metadata_bytes(
            {
                "manifest": manifest,
                "destination_binding": destination_binding,
                "public_inputs": sample_ton_public_inputs(target_domain=SCCP_DOMAIN_SOL),
                "statement_hash": HEX32_B,
            }
        )
    for bad_backend_key in ("", False, 0):
        with pytest.raises(TypeError, match="verifierBackendKey"):
            canonical_sccp_ton_submission_metadata_bytes(
                {
                    "manifest": {
                        **manifest,
                        "verifier_backend_key": bad_backend_key,
                    },
                    "destination_binding": destination_binding,
                    "public_inputs": sample_ton_public_inputs(),
                    "statement_hash": HEX32_B,
                }
            )
    assert build_ton_sccp_message_body_boc(
        sample_ton_message_body_input_with_result(
            metadata_bytes=None,
            manifest=manifest,
            destination_binding=destination_binding,
            destination_binding_hash=HEX32_H,
        )
    ) != body
    with pytest.raises(TypeError, match=r"destinationBindingHash must match destinationBinding\.bindingHash"):
        build_ton_sccp_message_body_boc(
            sample_ton_message_body_input_with_result(
                metadata_bytes=None,
                manifest=manifest_with_binding,
                destination_binding_hash=HEX32_G,
            )
        )

    with pytest.raises(TypeError, match="bundleBytes must not be empty"):
        build_ton_sccp_submission(sample_ton_message_body_input_with_result(bundle_bytes=b""))
    with pytest.raises(TypeError, match="bundleBytes must not be all zero"):
        build_ton_sccp_submission(
            sample_ton_message_body_input_with_result(bundle_bytes=b"\x00\x00")
        )
    with pytest.raises(TypeError, match="bundleBytes must be at most"):
        build_ton_sccp_submission(
            sample_ton_message_body_input_with_result(
                bundle_bytes=bytes([1]) * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1)
            )
        )
    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        build_ton_sccp_submission(sample_ton_message_body_input_with_result(proof_bytes=b"\x00\x00"))

    request = build_ton_sccp_proof_request(
        sample_ton_request_input(
            statement_hash=HEX32_B,
            destination_binding_hash=HEX32_G,
            source_adapter_deployment_hash=HEX32_A,
            source_adapter_deployment_receipt_hash=HEX32_B,
        )
    )
    proof_result = wrap_ton_sccp_proof_result(bytes([1, 2, 3, 4]), request)
    oversized_ton_message_result = wrap_ton_sccp_proof_result(
        bytes([1]) * (
            sccp_module._SCCP_TON_MAX_BOC_CELLS
            * sccp_module._SCCP_TON_MAX_CELL_DATA_BYTES
        ),
        request,
    )
    with pytest.raises(ValueError, match="TON BOC contains too many cells"):
        build_ton_sccp_submission(
            {
                "proof_result": oversized_ton_message_result,
                "bundle_bytes": bytes([5, 6, 7]),
                "metadata_bytes": bytes([8, 9]),
            }
        )
    proof_result_submission = build_ton_sccp_submission(
        {
            "proof_result": proof_result,
            "bundle_bytes": bytes([5, 6, 7]),
            "metadata_bytes": bytes([8, 9]),
        }
    )
    assert proof_result_submission["envelope_hex"] == submission["envelope_hex"]
    assert proof_result["bundle_bytes"] == bytes([5, 6, 7])
    assert proof_result["source_proof_bytes"] == bytes([9, 10])
    with pytest.raises(TypeError, match="proofResult must not use multiple aliases"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "proofResult": proof_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"proofResult\.proofBytes must not use multiple aliases"):
        build_ton_sccp_submission(
            {
                "proof_result": {
                    **proof_result,
                    "proofBytes": proof_result["proof_bytes"],
                },
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"proofResult\.requestHash must not use multiple aliases"):
        build_ton_sccp_submission(
            {
                "proof_result": {
                    **proof_result,
                    "requestHash": proof_result["request_hash"],
                },
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"proofResult\.proofContext must not use multiple aliases"):
        build_ton_sccp_submission(
            {
                "proof_result": {
                    **proof_result,
                    "proofContext": proof_result["proof_context"],
                },
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="bundleBytes must not use multiple aliases"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "bundle_bytes": bytes([5, 6, 7]),
                "bundleBytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="publicInputs must be an object"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "public_inputs": None,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="proofBytes must be bytes"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "proof_bytes": None,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="statementHash"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "statement_hash": None,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="destinationBindingHash"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "destination_binding_hash": None,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    omitted_source_proof_result = wrap_ton_sccp_proof_result(
        bytes([1, 2, 3, 4]),
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                source_proof_bytes=b"",
                statement_hash=HEX32_B,
                destination_binding_hash=HEX32_G,
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        ),
    )
    omitted_source_proof_submission = build_ton_sccp_submission(
        {
            "proof_result": omitted_source_proof_result,
            "bundle_bytes": bytes([5, 6, 7]),
            "metadata_bytes": bytes([8, 9]),
        }
    )
    assert omitted_source_proof_result["source_proof_bytes"] == b""
    assert omitted_source_proof_submission["envelope_hex"] == submission["envelope_hex"]
    with pytest.raises(TypeError, match=r"bundleBytes must match proofResult\.bundleBytes"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "bundle_bytes": bytes([5, 6, 8]),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.requestHash must match bundleBytes and sourceProofBytes",
    ):
        build_ton_sccp_submission(
            {
                "proof_result": {
                    **proof_result,
                    "bundle_bytes": bytes([5, 6, 8]),
                },
                "bundle_bytes": bytes([5, 6, 8]),
            }
        )
    with pytest.raises(TypeError, match=r"proofBytes must match proofResult\.proofBytes"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "proof_bytes": bytes([4, 3, 2, 1]),
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"publicInputs must match proofResult\.publicInputs"):
        build_ton_sccp_submission(
            {
                "proof_result": proof_result,
                "public_inputs": sample_ton_public_inputs(message_id=HEX32_A),
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.envelopeHash must match wrapped proof bytes",
    ):
        build_ton_sccp_submission(
            {
                "proof_result": {**proof_result, "envelope_hash": HEX32_A},
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.sourceStateVerifierHash must not be zero",
    ):
        build_ton_sccp_submission(
            {
                "proof_result": {
                    **proof_result,
                    "source_state_verifier_hash": SCCP_ZERO_HASH_V1,
                },
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.sourceAdapterDeploymentBinding\.targetDomain must be SORA",
    ):
        build_ton_sccp_submission(
            {
                "proof_result": {
                    **proof_result,
                    "source_adapter_deployment_binding": {
                        **proof_result["source_adapter_deployment_binding"],
                        "target_domain": SCCP_DOMAIN_TON,
                    },
                },
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"publicInputs\.targetDomain must be TON"):
        build_ton_sccp_submission(
            sample_ton_message_body_input_with_result(
                public_inputs=sample_ton_public_inputs(target_domain=SCCP_DOMAIN_SOL)
            )
        )
    for field, label in (
        ("statement_hash", "statementHash"),
        ("destination_binding_hash", "destinationBindingHash"),
    ):
        for bad_value in ("", False, 0):
            tampered_result = dict(proof_result)
            tampered_result[field] = bad_value
            with pytest.raises(TypeError, match=label):
                build_ton_sccp_submission(
                    {
                        "proof_result": tampered_result,
                        "bundle_bytes": bytes([5, 6, 7]),
                    }
                )
    tampered_context = dict(proof_result)
    tampered_context["proof_context"] = False
    with pytest.raises(TypeError, match=r"proofResult\.proofContext must be an object"):
        build_ton_sccp_submission(
            {
                "proof_result": tampered_context,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    tampered_context = dict(proof_result)
    tampered_context["proof_context"] = None
    with pytest.raises(TypeError, match=r"proofResult\.proofContext must be an object"):
        build_ton_sccp_submission(
            {
                "proof_result": tampered_context,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"cells\[0\]\.data must be bytes"):
        sccp_module._ton_encode_boc_single_root([{"data": False, "refs": []}])
    with pytest.raises(TypeError, match=r"cells\[0\]\.refs must be a sequence"):
        sccp_module._ton_encode_boc_single_root([{"data": b"", "refs": False}])


def test_builds_ton_sccp_proof_request_with_relay_and_deployment_binding() -> None:
    request = build_ton_sccp_proof_request(
        sample_ton_request_input(
            source_adapter_deployment_binding={
                "source_adapter_deployment_hash": HEX32_A,
                "source_adapter_deployment_receipt_hash": HEX32_B,
            }
        )
    )

    assert request["version"] == 1
    assert request["backend"] == SCCP_TON_CONTRACT_PROOF_BACKEND_V1
    assert request["source_domain"] == SCCP_DOMAIN_TON
    assert request["target_domain"] == SCCP_DOMAIN_TON
    assert request["source_state_verifier_id"] == SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1
    assert request["source_state_verifier_hash"] == HEX32_C
    assert request["public_inputs"] == {
        "version": 1,
        "message_id": HEX32_D,
        "payload_hash": HEX32_E,
        "target_domain": SCCP_DOMAIN_TON,
        "commitment_root": HEX32_F,
        "finality_height": "19",
        "finality_block_hash": HEX32_A,
    }
    assert request["bundle_bytes"] == bytes([5, 6, 7])
    assert request["source_proof_bytes"] == bytes([9, 10])
    assert request["proof_context"] == {
        "version": 1,
        "statement_hash": HEX32_G,
        "destination_binding_hash": HEX32_H,
    }
    assert normalize_ton_sccp_proof_context(
        {
            "statement_hash": HEX32_G,
            "destination_binding": {"binding_hash": HEX32_H},
        }
    ) == request["proof_context"]
    assert request["source_adapter_deployment_binding"] == {
        "version": 1,
        "source_domain": SCCP_DOMAIN_TON,
        "target_domain": SCCP_DOMAIN_SORA,
        "source_adapter_deployment_hash": HEX32_A,
        "source_adapter_deployment_receipt_hash": HEX32_B,
    }
    assert request["source_adapter_deployment_binding_hash"] == (
        sccp_source_adapter_deployment_binding_hash(
            request["source_adapter_deployment_binding"]
        )
    )
    assert len(request["public_inputs_bytes"]) == 141
    assert len(request["request_hash"]) == 66
    with pytest.raises(TypeError, match="immutable"):
        request["request_hash"] = HEX32_A
    with pytest.raises(TypeError, match="immutable"):
        request["proof_context"]["statement_hash"] = HEX32_A

    duplicate_public_inputs_alias = sample_ton_request_input(
        source_adapter_deployment_hash=HEX32_A,
        source_adapter_deployment_receipt_hash=HEX32_B,
    )
    duplicate_public_inputs_alias["publicInputs"] = duplicate_public_inputs_alias[
        "public_inputs"
    ]
    with pytest.raises(TypeError, match="publicInputs must not use multiple aliases"):
        build_ton_sccp_proof_request(duplicate_public_inputs_alias)

    duplicate_context_alias = sample_ton_request_input(
        proof_context={
            "statementHash": HEX32_G,
            "statement_hash": HEX32_G,
            "destination_binding_hash": HEX32_H,
        },
        source_adapter_deployment_hash=HEX32_A,
        source_adapter_deployment_receipt_hash=HEX32_B,
    )
    with pytest.raises(TypeError, match="statementHash must not use multiple aliases"):
        build_ton_sccp_proof_request(duplicate_context_alias)

    mismatched_context_binding = sample_ton_request_input(
        proof_context={
            "statement_hash": HEX32_G,
            "destination_binding_hash": HEX32_H,
            "destination_binding": {"binding_hash": HEX32_A},
        },
        source_adapter_deployment_hash=HEX32_A,
        source_adapter_deployment_receipt_hash=HEX32_B,
    )
    with pytest.raises(
        TypeError,
        match=r"destinationBindingHash must match destinationBinding\.bindingHash",
    ):
        build_ton_sccp_proof_request(mismatched_context_binding)

    duplicate_deployment_alias = sample_ton_request_input(
        source_adapter_deployment_binding={
            "source_adapter_deployment_hash": HEX32_A,
            "sourceAdapterDeploymentHash": HEX32_A,
            "source_adapter_deployment_receipt_hash": HEX32_B,
        }
    )
    with pytest.raises(
        TypeError,
        match="sourceAdapterDeploymentHash must not use multiple aliases",
    ):
        build_ton_sccp_proof_request(duplicate_deployment_alias)

    mismatched_deployment_binding = sample_ton_request_input(
        source_adapter_deployment_hash=HEX32_A,
        source_adapter_deployment_receipt_hash=HEX32_B,
        source_adapter_deployment_binding={
            "source_adapter_deployment_hash": HEX32_C,
            "source_adapter_deployment_receipt_hash": HEX32_B,
        },
    )
    with pytest.raises(
        TypeError,
        match=r"sourceAdapterDeploymentHash must match sourceAdapterDeploymentBinding\.sourceAdapterDeploymentHash",
    ):
        build_ton_sccp_proof_request(mismatched_deployment_binding)

    changed = build_ton_sccp_proof_request(
        sample_ton_request_input(
            source_adapter_deployment_hash=HEX32_C,
            source_adapter_deployment_receipt_hash=HEX32_D,
        )
    )
    assert changed["request_hash"] != request["request_hash"]
    changed_source_state = build_ton_sccp_proof_request(
        sample_ton_request_input(
            source_state_verifier_hash=HEX32_D,
            source_adapter_deployment_hash=HEX32_A,
            source_adapter_deployment_receipt_hash=HEX32_B,
        )
    )
    assert changed_source_state["request_hash"] != request["request_hash"]
    shifted_split = build_ton_sccp_proof_request(
        sample_ton_request_input(
            bundle_bytes=bytes([5, 6, 7, 9]),
            source_proof_bytes=bytes([10]),
            source_adapter_deployment_hash=HEX32_A,
            source_adapter_deployment_receipt_hash=HEX32_B,
        )
    )
    assert shifted_split["request_hash"] != request["request_hash"]

    with pytest.raises(TypeError, match="sourceStateVerifierId must match TON"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                source_state_verifier_id="debug-ton-state-verifier",
                source_state_verifier_hash=HEX32_C,
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match="sourceStateVerifierHash must not be zero"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                source_state_verifier_hash=SCCP_ZERO_HASH_V1,
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match="TON template verifier hash"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                source_state_verifier_hash=TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH,
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match="must both be zero or both be non-zero"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(source_adapter_deployment_hash=HEX32_A)
        )
    with pytest.raises(TypeError, match="requires non-zero source adapter deployment binding"):
        build_ton_sccp_proof_request(sample_ton_request_input())
    with pytest.raises(TypeError, match="deployment binding targetDomain must be SORA"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                source_adapter_deployment_binding={
                    "source_domain": SCCP_DOMAIN_TON,
                    "target_domain": SCCP_DOMAIN_TON,
                    "source_adapter_deployment_hash": HEX32_A,
                    "source_adapter_deployment_receipt_hash": HEX32_B,
                }
            )
        )
    with pytest.raises(TypeError, match="bundleBytes must not be empty"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                bundle_bytes=b"",
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match="bundleBytes must not be all zero"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                bundle_bytes=b"\x00\x00",
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match="bundleBytes must be at most"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                bundle_bytes=bytes([1]) * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1),
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match="sourceProofBytes must be at most"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                source_proof_bytes=bytes([1]) * (SCCP_SOURCE_STATE_MAX_PROOF_BYTES + 1),
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match=r"publicInputs\.targetDomain must be TON"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                public_inputs=sample_ton_public_inputs(target_domain=SCCP_DOMAIN_SOL),
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match="backend must be ton-contract-v1"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                backend="debug-ton-backend",
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    for bad_backend in ("", False, 0):
        with pytest.raises(TypeError, match="backend must be ton-contract-v1"):
            build_ton_sccp_proof_request(
                sample_ton_request_input(
                    backend=bad_backend,
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
    for bad_verifier_id in ("", False, 0):
        with pytest.raises(TypeError, match="sourceStateVerifierId"):
            build_ton_sccp_proof_request(
                sample_ton_request_input(
                    source_state_verifier_id=bad_verifier_id,
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
    for bad_context in ({}, False, 0, ""):
        with pytest.raises(TypeError, match="TON SCCP proof context|statementHash"):
            build_ton_sccp_proof_request(
                sample_ton_request_input(
                    proof_context=bad_context,
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
    for bad_binding in (False, 0, ""):
        with pytest.raises(TypeError, match="sourceAdapterDeploymentBinding"):
            build_ton_sccp_proof_request(
                sample_ton_request_input(
                    source_adapter_deployment_binding=bad_binding,
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
    for bad_deployment_hash in ("", False, 0):
        with pytest.raises(TypeError, match="sourceAdapterDeploymentHash"):
            build_ton_sccp_proof_request(
                sample_ton_request_input(
                    source_adapter_deployment_hash=bad_deployment_hash,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )


def test_ton_sccp_proof_request_hash_matches_cross_sdk_vector() -> None:
    public_inputs = {
        "version": 1,
        "message_id": "0x" + "11" * 32,
        "payload_hash": "0x" + "22" * 32,
        "target_domain": SCCP_DOMAIN_TON,
        "commitment_root": "0x" + "33" * 32,
        "finality_height": 123456789,
        "finality_block_hash": "0x" + "44" * 32,
    }
    request = build_ton_sccp_proof_request(
        {
            "public_inputs": public_inputs,
            "bundle_bytes": bytes([1, 2, 3, 4, 5, 6, 7, 8, 9]),
            "source_proof_bytes": bytes([0x51, 0x52, 0x53]),
            "statement_hash": "0x" + "55" * 32,
            "destination_binding_hash": "0x" + "66" * 32,
            "source_state_verifier_hash": "0x" + "42" * 32,
            "source_adapter_deployment_binding": {
                "source_domain": SCCP_DOMAIN_TON,
                "target_domain": SCCP_DOMAIN_SORA,
                "source_adapter_deployment_hash": HEX32_A,
                "source_adapter_deployment_receipt_hash": HEX32_B,
            },
        }
    )

    assert (
        "0x" + canonical_sccp_message_transparent_public_inputs_bytes(public_inputs).hex()
        == "0x011111111111111111111111111111111111111111111111111111111111111111222222222222222222222222222222222222222222222222222222222222222204000000333333333333333333333333333333333333333333333333333333333333333315cd5b07000000004444444444444444444444444444444444444444444444444444444444444444"
    )
    assert (
        request["source_adapter_deployment_binding_hash"]
        == "0x7d35b186e3d49aed31693e33d33355fa8fa9032160c929f2c7fe260094f6ccdf"
    )
    assert (
        request["request_hash"]
        == "0xb3a61f09923efd639a0263de6b45eec6ddd5de679bfaab1b6ec1c591fd1b1d1b"
    )

    proof_result = wrap_ton_sccp_proof_result(
        bytes([0x91, 0x92, 0x93, 0x94, 0x95]),
        request,
    )
    assert (
        proof_result["envelope_hash"]
        == "0xa2bc6697b237fd4b2dd3f60f187a184793104a99372dcdf60c7ec585ef32f5ab"
    )


def test_builds_tron_sccp_groth16_proof_request_with_public_signals() -> None:
    request = build_tron_sccp_proof_request(sample_tron_request_input())

    assert request["version"] == 1
    assert request["backend"] == SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1
    assert request["source_domain"] == SCCP_DOMAIN_SORA
    assert request["target_domain"] == SCCP_DOMAIN_TRON
    assert request["public_inputs"] == sample_tron_public_inputs()
    assert request["bundle_bytes"] == bytes([5, 6, 7])
    assert request["source_proof_bytes"] == bytes([9, 10])
    assert request["proof_context"] == {
        "version": 1,
        "statement_hash": "0x" + "55" * 32,
        "destination_binding_hash": "0x" + "66" * 32,
    }
    assert normalize_tron_sccp_proof_context(
        {
            "statement_hash": "0x" + "55" * 32,
            "destination_binding": {"binding_hash": "0x" + "66" * 32},
        }
    ) == request["proof_context"]
    assert request["public_signal_words"] == sccp_groth16_bn254_public_signal_words(
        {
            "public_inputs": request["public_inputs"],
            "source_domain": SCCP_DOMAIN_SORA,
            "statement_hash": "0x" + "55" * 32,
            "destination_binding_hash": "0x" + "66" * 32,
        }
    )
    with pytest.raises(TypeError, match="immutable"):
        request["public_signal_words"].append(HEX32_A)
    assert request["request_hash"] == (
        "0x853cdaff01db27620f607dfb54cc7a580b4733849354b18867e5d5ca129d40cd"
    )
    assert request["request_hash"] != build_tron_sccp_proof_request(
        sample_tron_request_input(
            bundle_bytes=bytes([5, 6, 7, 9]),
            source_proof_bytes=bytes([10]),
        )
    )["request_hash"]

    changed = build_tron_sccp_proof_request(
        sample_tron_request_input(destination_binding_hash="0x" + "67" * 32)
    )
    assert changed["public_signal_words"][:8] == request["public_signal_words"][:8]
    assert changed["public_signal_words"][8] != request["public_signal_words"][8]
    assert changed["request_hash"] != request["request_hash"]

    with pytest.raises(TypeError, match="publicInputs must not use multiple aliases"):
        build_tron_sccp_proof_request(
            {
                **sample_tron_request_input(),
                "publicInputs": sample_tron_public_inputs(),
            }
        )
    with pytest.raises(TypeError, match="bundleBytes must not use multiple aliases"):
        build_tron_sccp_proof_request(
            {
                **sample_tron_request_input(),
                "bundleBytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="sourceProofBytes must not use multiple aliases"):
        build_tron_sccp_proof_request(
            {
                **sample_tron_request_input(),
                "sourceProofBytes": bytes([9, 10]),
            }
        )
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        build_tron_sccp_proof_request(
            {
                **sample_tron_request_input(),
                "sourceDomain": SCCP_DOMAIN_SORA,
            }
        )
    with pytest.raises(TypeError, match="proofContext must not use multiple aliases"):
        build_tron_sccp_proof_request(
            {
                **sample_tron_request_input(
                    proof_context={
                        "statement_hash": "0x" + "55" * 32,
                        "destination_binding_hash": "0x" + "66" * 32,
                    }
                ),
                "proofContext": {
                    "statement_hash": "0x" + "55" * 32,
                    "destination_binding_hash": "0x" + "66" * 32,
                },
            }
        )

    with pytest.raises(TypeError, match="payloadHash must not be zero"):
        build_tron_sccp_proof_request(
            sample_tron_request_input(
                public_inputs=sample_tron_public_inputs(payload_hash=SCCP_ZERO_HASH_V1)
            )
        )
    with pytest.raises(TypeError, match=r"publicInputs\.payloadHash must be canonical hex"):
        build_tron_sccp_proof_request(
            sample_tron_request_input(
                public_inputs=sample_tron_public_inputs(payload_hash=" " + "0x" + "22" * 32)
            )
        )
    with pytest.raises(ValueError, match="sourceDomain must be SORA"):
        build_tron_sccp_proof_request(sample_tron_request_input(source_domain=SCCP_DOMAIN_ETH))
    with pytest.raises(ValueError, match="publicInputs.targetDomain must be TRON"):
        build_tron_sccp_proof_request(
            sample_tron_request_input(
                public_inputs=sample_tron_public_inputs(target_domain=SCCP_DOMAIN_TON)
            )
        )
    with pytest.raises(TypeError, match="statementHash must not be zero"):
        build_tron_sccp_proof_request(sample_tron_request_input(statement_hash=SCCP_ZERO_HASH_V1))
    with pytest.raises(TypeError, match="statementHash must be canonical hex"):
        build_tron_sccp_proof_request(
            sample_tron_request_input(statement_hash="0x" + "55" * 32 + "\n")
        )
    with pytest.raises(TypeError, match="bundleBytes must not be empty"):
        build_tron_sccp_proof_request(sample_tron_request_input(bundle_bytes=b""))
    with pytest.raises(TypeError, match="backend must be tron-groth16-bn254-v1"):
        build_tron_sccp_proof_request(
            sample_tron_request_input(backend="debug-tron-backend")
        )
    for bad_backend in ("", False, 0):
        with pytest.raises(TypeError, match="backend must be tron-groth16-bn254-v1"):
            build_tron_sccp_proof_request(
                sample_tron_request_input(backend=bad_backend)
            )
    for bad_context in ({}, False, 0, ""):
        with pytest.raises(TypeError, match="proof context|statementHash"):
            build_tron_sccp_proof_request(
                sample_tron_request_input(proof_context=bad_context)
            )


def test_builds_evm_family_sccp_groth16_proof_request_with_public_signals() -> None:
    request = build_evm_sccp_proof_request(sample_evm_request_input())

    assert request["version"] == 1
    assert request["backend"] == SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
    assert request["source_domain"] == SCCP_DOMAIN_SORA
    assert request["target_domain"] == SCCP_DOMAIN_ETH
    assert request["public_inputs"] == sample_evm_public_inputs()
    assert request["bundle_bytes"] == bytes([5, 6, 7])
    assert request["source_proof_bytes"] == bytes([9, 10])
    assert request["proof_context"] == {
        "version": 1,
        "statement_hash": "0x" + "55" * 32,
        "destination_binding_hash": "0x" + "66" * 32,
    }
    assert normalize_evm_sccp_proof_context(
        {
            "statement_hash": "0x" + "55" * 32,
            "destination_binding": {"binding_hash": "0x" + "66" * 32},
        }
    ) == request["proof_context"]
    assert request["public_signal_words"] == sccp_groth16_bn254_public_signal_words(
        {
            "public_inputs": request["public_inputs"],
            "source_domain": SCCP_DOMAIN_SORA,
            "statement_hash": "0x" + "55" * 32,
            "destination_binding_hash": "0x" + "66" * 32,
        }
    )
    with pytest.raises(TypeError, match="immutable"):
        request["public_signal_words"].append(HEX32_A)
    assert request["request_hash"] == (
        "0xc784b3c223200182c9a52017eaba9a1d9225ed11ae3d99c35b17a1b0083cdfad"
    )
    assert request["request_hash"] != build_evm_sccp_proof_request(
        sample_evm_request_input(
            bundle_bytes=bytes([5, 6, 7, 9]),
            source_proof_bytes=bytes([10]),
        )
    )["request_hash"]

    bsc_request = build_evm_sccp_proof_request(
        sample_evm_request_input(
            public_inputs=sample_evm_public_inputs(target_domain=SCCP_DOMAIN_BSC)
        )
    )
    assert bsc_request["target_domain"] == SCCP_DOMAIN_BSC
    assert bsc_request["public_signal_words"][2] != request["public_signal_words"][2]
    assert bsc_request["request_hash"] != request["request_hash"]

    changed = build_evm_sccp_proof_request(
        sample_evm_request_input(destination_binding_hash="0x" + "67" * 32)
    )
    assert changed["public_signal_words"][:8] == request["public_signal_words"][:8]
    assert changed["public_signal_words"][8] != request["public_signal_words"][8]
    assert changed["request_hash"] != request["request_hash"]

    with pytest.raises(TypeError, match="publicInputs must not use multiple aliases"):
        build_evm_sccp_proof_request(
            {
                **sample_evm_request_input(),
                "publicInputs": sample_evm_public_inputs(),
            }
        )
    with pytest.raises(TypeError, match=r"publicInputs\.messageId.*multiple aliases"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(
                public_inputs={
                    **sample_evm_public_inputs(),
                    "messageId": sample_evm_public_inputs()["message_id"],
                },
            )
        )
    with pytest.raises(TypeError, match="publicInputs must not use multiple aliases"):
        sccp_groth16_bn254_public_signal_words(
            {
                "public_inputs": sample_evm_public_inputs(),
                "publicInputs": sample_evm_public_inputs(),
                "source_domain": SCCP_DOMAIN_SORA,
                "statement_hash": "0x" + "55" * 32,
                "destination_binding_hash": "0x" + "66" * 32,
            }
        )
    with pytest.raises(TypeError, match="bundleBytes must not use multiple aliases"):
        build_evm_sccp_proof_request(
            {
                **sample_evm_request_input(),
                "bundleBytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="sourceProofBytes must not use multiple aliases"):
        build_evm_sccp_proof_request(
            {
                **sample_evm_request_input(),
                "sourceProofBytes": bytes([9, 10]),
            }
        )
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        build_evm_sccp_proof_request(
            {
                **sample_evm_request_input(),
                "sourceDomain": SCCP_DOMAIN_SORA,
            }
        )
    with pytest.raises(TypeError, match="proofContext must not use multiple aliases"):
        build_evm_sccp_proof_request(
            {
                **sample_evm_request_input(
                    proof_context={
                        "statement_hash": "0x" + "55" * 32,
                        "destination_binding_hash": "0x" + "66" * 32,
                    }
                ),
                "proofContext": {
                    "statement_hash": "0x" + "55" * 32,
                    "destination_binding_hash": "0x" + "66" * 32,
                },
            }
        )

    with pytest.raises(ValueError, match="finalityHeight must not be zero"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(
                public_inputs=sample_evm_public_inputs(finality_height="0")
            )
        )
    with pytest.raises(ValueError, match="sourceDomain must be SORA"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(
                public_inputs=sample_evm_public_inputs(target_domain=SCCP_DOMAIN_ETH),
                source_domain=SCCP_DOMAIN_ETH,
            )
        )
    with pytest.raises(ValueError, match="sourceDomain must be SORA"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(source_domain=SCCP_DOMAIN_TON)
        )
    with pytest.raises(TypeError, match="sourceDomain must be a u32 domain id"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(source_domain=None)
        )
    with pytest.raises(ValueError, match="publicInputs.targetDomain must be ETH or BSC"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(
                public_inputs=sample_evm_public_inputs(target_domain=SCCP_DOMAIN_TON)
            )
        )
    with pytest.raises(TypeError, match="destinationBindingHash must not be zero"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(destination_binding_hash=SCCP_ZERO_HASH_V1)
        )
    with pytest.raises(TypeError, match="bundleBytes must not be empty"):
        build_evm_sccp_proof_request(sample_evm_request_input(bundle_bytes=b""))
    with pytest.raises(TypeError, match="backend must be evm-groth16-bn254-v1"):
        build_evm_sccp_proof_request(sample_evm_request_input(backend="debug-evm-backend"))
    for bad_backend in ("", False, 0):
        with pytest.raises(TypeError, match="backend must be evm-groth16-bn254-v1"):
            build_evm_sccp_proof_request(
                sample_evm_request_input(backend=bad_backend)
            )
    for bad_context in ({}, False, 0, ""):
        with pytest.raises(TypeError, match="proof context|statementHash"):
            build_evm_sccp_proof_request(
                sample_evm_request_input(proof_context=bad_context)
            )


def test_builds_substrate_sccp_runtime_proof_request() -> None:
    request = build_substrate_sccp_proof_request(sample_substrate_request_input())

    assert request["version"] == 1
    assert request["backend"] == SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1
    assert request["source_domain"] == SCCP_DOMAIN_SORA
    assert request["target_domain"] == SCCP_DOMAIN_SORA2
    assert request["public_inputs"] == sample_substrate_public_inputs()
    assert request["bundle_bytes"] == bytes([5, 6, 7])
    assert request["source_proof_bytes"] == bytes([9, 10])
    assert request["proof_context"] == {
        "version": 1,
        "statement_hash": "0x" + "55" * 32,
        "destination_binding_hash": "0x" + "66" * 32,
    }
    assert len(request["public_inputs_bytes"]) == 141
    assert len(request["request_hash"]) == 66
    with pytest.raises(TypeError, match="immutable"):
        request["source_proof_bytes"] = b""

    kusama_request = build_substrate_sccp_proof_request(
        sample_substrate_request_input(
            public_inputs=sample_substrate_public_inputs(
                target_domain=SCCP_DOMAIN_SORA_KUSAMA
            )
        )
    )
    assert kusama_request["target_domain"] == SCCP_DOMAIN_SORA_KUSAMA
    assert kusama_request["request_hash"] != request["request_hash"]

    assert request["request_hash"] != build_substrate_sccp_proof_request(
        sample_substrate_request_input(
            bundle_bytes=bytes([5, 6, 7, 9]),
            source_proof_bytes=bytes([10]),
        )
    )["request_hash"]
    with pytest.raises(TypeError, match="publicInputs must not use multiple aliases"):
        build_substrate_sccp_proof_request(
            {
                **sample_substrate_request_input(),
                "publicInputs": sample_substrate_public_inputs(),
            }
        )
    with pytest.raises(TypeError, match="bundleBytes must not use multiple aliases"):
        build_substrate_sccp_proof_request(
            {
                **sample_substrate_request_input(),
                "bundleBytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="sourceProofBytes must not use multiple aliases"):
        build_substrate_sccp_proof_request(
            {
                **sample_substrate_request_input(),
                "sourceProofBytes": bytes([9, 10]),
            }
        )
    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        build_substrate_sccp_proof_request(
            {
                **sample_substrate_request_input(),
                "sourceDomain": SCCP_DOMAIN_SORA,
            }
        )
    with pytest.raises(TypeError, match="proofContext must not use multiple aliases"):
        build_substrate_sccp_proof_request(
            {
                **sample_substrate_request_input(
                    proof_context={
                        "statement_hash": "0x" + "55" * 32,
                        "destination_binding_hash": "0x" + "66" * 32,
                    }
                ),
                "proofContext": {
                    "statement_hash": "0x" + "55" * 32,
                    "destination_binding_hash": "0x" + "66" * 32,
                },
            }
        )
    with pytest.raises(TypeError, match="statementHash must not use multiple aliases"):
        build_substrate_sccp_proof_request(
            {
                **sample_substrate_request_input(),
                "statementHash": "0x" + "55" * 32,
            }
        )
    with pytest.raises(
        TypeError,
        match=r"destinationBinding\.bindingHash must not use multiple aliases",
    ):
        build_substrate_sccp_proof_request(
            {
                **sample_substrate_request_input(destination_binding_hash=None),
                "destination_binding": {
                    "binding_hash": "0x" + "66" * 32,
                    "bindingHash": "0x" + "66" * 32,
                },
            }
        )
    with pytest.raises(ValueError, match="sourceDomain must be SORA"):
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(source_domain=SCCP_DOMAIN_TRON)
        )

    with pytest.raises(ValueError, match="Substrate-family SCCP domain"):
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(
                public_inputs=sample_substrate_public_inputs(target_domain=SCCP_DOMAIN_TON)
            )
        )
    with pytest.raises(ValueError, match="sourceDomain must be SORA"):
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(source_domain=SCCP_DOMAIN_SORA2)
        )
    with pytest.raises(TypeError, match="destinationBindingHash must not be zero"):
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(destination_binding_hash=SCCP_ZERO_HASH_V1)
        )
    with pytest.raises(TypeError, match="bundleBytes must not be empty"):
        build_substrate_sccp_proof_request(sample_substrate_request_input(bundle_bytes=b""))
    with pytest.raises(TypeError, match="bundleBytes must not be all zero"):
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(bundle_bytes=b"\x00\x00")
        )
    with pytest.raises(TypeError, match="bundleBytes must be at most"):
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(
                bundle_bytes=b"\x01" * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1)
            )
        )
    with pytest.raises(TypeError, match="backend must be substrate-runtime-v1"):
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(backend="debug-substrate-backend")
        )
    for bad_backend in ("", False, 0):
        with pytest.raises(TypeError, match="backend must be substrate-runtime-v1"):
            build_substrate_sccp_proof_request(
                sample_substrate_request_input(backend=bad_backend)
            )
    for bad_context in ({}, False, 0, ""):
        with pytest.raises(TypeError, match="proof context|statementHash"):
            build_substrate_sccp_proof_request(
                sample_substrate_request_input(proof_context=bad_context)
            )


def test_builds_substrate_sccp_runtime_call_submission() -> None:
    request = build_substrate_sccp_proof_request(sample_substrate_request_input())
    proof_result = wrap_substrate_sccp_proof_result(bytes([1, 2, 3, 4]), request)
    submission = build_substrate_sccp_submission({"proof_result": proof_result})

    assert submission["version"] == 1
    assert submission["proof_family"] == SCCP_STARK_FRI_PROOF_FAMILY_V1
    assert submission["verifier_backend"] == SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1
    assert submission["platform_payload"] == "substrate_runtime_call"
    assert submission["envelope_encoding"] == SCCP_SUBSTRATE_RUNTIME_CALL_SCALE_V1
    assert submission["submission_kind"] == "runtime_call"
    assert (
        submission["verifier_entrypoint"]
        == SCCP_SUBSTRATE_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1
    )
    assert submission["source_domain"] == SCCP_DOMAIN_SORA
    assert submission["target_domain"] == SCCP_DOMAIN_SORA2
    assert submission["request_hash"] == request["request_hash"]
    assert [argument["key"] for argument in submission["arguments"]] == [
        "proof_bytes",
        "public_inputs",
        "bundle_bytes",
    ]
    assert submission["runtime_call"] == submission["envelope_bytes"]
    assert submission["runtime_call_hex"] == submission["envelope_hex"]
    assert submission["runtime_call_hex"].startswith(
        "0x7c" + SCCP_SUBSTRATE_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1.encode().hex() + "10"
    )
    assert submission["proof_bytes"] == bytes([1, 2, 3, 4])
    assert submission["public_inputs_bytes"] == request["public_inputs_bytes"]
    assert submission["bundle_bytes"] == bytes([5, 6, 7])
    with pytest.raises(TypeError, match="immutable"):
        submission["bundle_bytes"] = b""
    with pytest.raises(TypeError, match="proofResult must not use multiple aliases"):
        build_substrate_sccp_submission(
            {
                "proof_result": proof_result,
                "proofResult": proof_result,
            }
        )
    with pytest.raises(
        TypeError,
        match="proofResult must be a wrapped Substrate SCCP proof result",
    ):
        build_substrate_sccp_submission(
            {
                "proof_result": None,
                "proof_bytes": bytes([1, 2, 3, 4]),
                "public_inputs": sample_substrate_public_inputs(),
                "bundle_bytes": bytes([5, 6, 7]),
                "source_proof_bytes": bytes([9, 10]),
                "statement_hash": "0x" + "55" * 32,
                "destination_binding_hash": "0x" + "66" * 32,
            }
        )
    with pytest.raises(TypeError, match=r"proofResult\.requestHash.*multiple aliases"):
        build_substrate_sccp_submission(
            {
                "proof_result": {
                    **dict(proof_result),
                    "requestHash": proof_result["request_hash"],
                },
            }
        )
    with pytest.raises(TypeError, match=r"proofResult\.envelopeHash.*multiple aliases"):
        build_substrate_sccp_submission(
            {
                "proof_result": {
                    **dict(proof_result),
                    "envelopeHash": proof_result["envelope_hash"],
                },
            }
        )
    with pytest.raises(TypeError, match="bundleBytes must not use multiple aliases"):
        build_substrate_sccp_submission(
            {
                "proof_result": proof_result,
                "bundle_bytes": bytes([5, 6, 7]),
                "bundleBytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"proofContext\.statementHash.*multiple aliases"):
        build_substrate_sccp_submission(
            {
                "proof_result": {
                    **dict(proof_result),
                    "proof_context": {
                        **dict(proof_result["proof_context"]),
                        "statementHash": proof_result["statement_hash"],
                    },
                },
            }
        )

    explicit_submission = build_substrate_sccp_submission(
        {
            "proof_bytes": bytes([1, 2, 3, 4]),
            "public_inputs": sample_substrate_public_inputs(),
            "bundle_bytes": bytes([5, 6, 7]),
            "source_proof_bytes": b"",
            "statement_hash": "0x" + "55" * 32,
            "destination_binding_hash": "0x" + "66" * 32,
        }
    )
    assert explicit_submission["runtime_call"] == submission["runtime_call"]

    with pytest.raises(
        TypeError,
        match="sourceProofBytes requires proofResult for request-bound submission",
    ):
        build_substrate_sccp_submission(
            {
                "proof_bytes": bytes([1, 2, 3, 4]),
                "public_inputs": sample_substrate_public_inputs(),
                "bundle_bytes": bytes([5, 6, 7]),
                "source_proof_bytes": bytes([9, 10]),
                "statement_hash": "0x" + "55" * 32,
                "destination_binding_hash": "0x" + "66" * 32,
            }
        )

    with pytest.raises(TypeError, match="bundleBytes must match proofResult.bundleBytes"):
        build_substrate_sccp_submission(
            {"proof_result": proof_result, "bundle_bytes": bytes([5, 6, 8])}
        )
    with pytest.raises(TypeError, match="bundleBytes must not be all zero"):
        build_substrate_sccp_submission(
            {
                "proof_bytes": bytes([1, 2, 3, 4]),
                "public_inputs": sample_substrate_public_inputs(),
                "bundle_bytes": b"\x00\x00",
                "source_proof_bytes": bytes([9, 10]),
                "statement_hash": "0x" + "55" * 32,
                "destination_binding_hash": "0x" + "66" * 32,
            }
        )
    with pytest.raises(TypeError, match="envelopeHash must match request"):
        build_substrate_sccp_submission(
            {"proof_result": {**proof_result, "envelope_hash": HEX32_A}}
        )
    with pytest.raises(
        TypeError,
        match="publicInputsBytes must match canonical SCCP transparent public inputs",
    ):
        build_substrate_sccp_submission(
            {"proof_result": proof_result, "public_inputs_bytes": bytes([1, 2, 3])}
        )


def test_sccp_proof_requests_reject_all_zero_source_proof_bytes() -> None:
    with pytest.raises(TypeError, match="sourceProofBytes must not be all zero"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                source_proof_bytes=b"\x00\x00",
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )
    with pytest.raises(TypeError, match="sourceProofBytes must not be all zero"):
        build_tron_sccp_proof_request(
            sample_tron_request_input(source_proof_bytes=b"\x00\x00")
        )
    with pytest.raises(TypeError, match="sourceProofBytes must not be all zero"):
        build_evm_sccp_proof_request(
            sample_evm_request_input(source_proof_bytes=b"\x00\x00")
        )
    with pytest.raises(TypeError, match="sourceProofBytes must not be all zero"):
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(source_proof_bytes=b"\x00\x00")
        )

    assert (
        build_evm_sccp_proof_request(
            sample_evm_request_input(source_proof_bytes=b"")
        )["source_proof_bytes"]
        == b""
    )
    assert (
        build_tron_sccp_proof_request(
            sample_tron_request_input(source_proof_bytes=b"")
        )["source_proof_bytes"]
        == b""
    )
    assert (
        build_substrate_sccp_proof_request(
            sample_substrate_request_input(source_proof_bytes=b"")
        )["source_proof_bytes"]
        == b""
    )
    assert (
        build_ton_sccp_proof_request(
            sample_ton_request_input(
                source_proof_bytes=b"",
                source_adapter_deployment_hash=HEX32_A,
                source_adapter_deployment_receipt_hash=HEX32_B,
            )
        )["source_proof_bytes"]
        == b""
    )
    assert (
        wrap_evm_sccp_proof_result(
            GROTH16_PROOF_BYTES,
            build_evm_sccp_proof_request(
                sample_evm_production_request_input(source_proof_bytes=b"")
            ),
        )["source_proof_bytes"]
        == b""
    )
    assert (
        wrap_tron_sccp_proof_result(
            GROTH16_PROOF_BYTES,
            build_tron_sccp_proof_request(
                sample_tron_production_request_input(source_proof_bytes=b"")
            ),
        )["source_proof_bytes"]
        == b""
    )
    assert (
        wrap_substrate_sccp_proof_result(
            bytes([1, 2, 3, 4]),
            build_substrate_sccp_proof_request(
                sample_substrate_request_input(source_proof_bytes=b"")
            ),
        )["source_proof_bytes"]
        == b""
    )
    assert (
        wrap_ton_sccp_proof_result(
            bytes([1, 2, 3, 4]),
            build_ton_sccp_proof_request(
                sample_ton_request_input(
                    source_proof_bytes=b"",
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            ),
        )["source_proof_bytes"]
        == b""
    )


def test_builds_solana_sccp_program_instruction_submission_data() -> None:
    solana_destination_binding_hash = sccp_destination_binding_hash(SCCP_DOMAIN_SOL)
    proof_request = build_solana_sccp_proof_request(
        sample_production_witness(destination_binding_hash=solana_destination_binding_hash)
    )
    proof_result = wrap_solana_sccp_proof_result(bytes([1, 2, 3, 4]), proof_request)
    transparent_public_inputs = {
        "message_id": proof_request["public_inputs"]["message_id"],
        "payload_hash": proof_request["public_inputs"]["payload_hash"],
        "target_domain": SCCP_DOMAIN_SOL,
        "commitment_root": proof_request["public_inputs"]["commitment_root"],
        "finality_height": proof_request["public_inputs"]["finalized_slot"],
        "finality_block_hash": proof_request["public_inputs"]["bank_hash"],
    }
    submission = build_solana_sccp_submission(
        {
            "public_inputs": transparent_public_inputs,
            "proof_result": proof_result,
            "bundle_bytes": bytes([5, 6, 7]),
        }
    )

    assert submission["envelope_encoding"] == SCCP_SOLANA_BORSH_INSTRUCTION_V1
    assert submission["submission_kind"] == "program_instruction"
    assert submission["verifier_entrypoint"] == "submit_sccp_message_proof"
    with pytest.raises(TypeError, match="proofBytes must not be empty"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "proof_bytes": b"",
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="statementHash"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "statement_hash": "",
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match="proofContextHash"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "proof_context_hash": "",
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    assert [argument["key"] for argument in submission["arguments"]] == [
        "proof_bytes",
        "public_inputs",
        "bundle_bytes",
        "statement_hash",
        "destination_binding_hash",
        "proof_context_hash",
    ]
    assert len(submission["public_inputs_bytes"]) == 141
    assert submission["proof_context_hash"] == solana_sccp_proof_context_hash(
        {
            "statement_hash": HEX32_G,
            "destination_binding_hash": solana_destination_binding_hash,
        }
    )
    assert submission["instruction_data_hex"] == submission["envelope_hex"]
    assert submission["instruction_data"][4:29].decode("utf-8") == "submit_sccp_message_proof"
    with pytest.raises(TypeError, match="immutable"):
        submission["arguments"].append({"key": "tampered"})
    with pytest.raises(TypeError, match="immutable"):
        submission["public_inputs"]["message_id"] = HEX32_A
    with pytest.raises(TypeError, match="requires transparent publicInputs"):
        build_solana_sccp_submission(
            {
                "proof_result": proof_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    proof_result_without_envelope = dict(proof_result)
    del proof_result_without_envelope["envelope_hash"]
    with pytest.raises(TypeError, match=r"proofResult\.envelopeHash must be non-zero"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result_without_envelope,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    tampered_envelope_result = dict(proof_result)
    tampered_envelope_result["envelope_hash"] = HEX32_A
    with pytest.raises(
        TypeError,
        match=r"proofResult\.envelopeHash must match wrapped proof bytes",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": tampered_envelope_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofBytes must match proofResult\.proofBytes",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "proof_bytes": bytes([9]),
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    proof_result_with_bad_version = dict(proof_result)
    proof_result_with_bad_version["version"] = 2
    with pytest.raises(TypeError, match=r"proofResult\.version must be 1"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result_with_bad_version,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    proof_result_with_bad_base64 = dict(proof_result)
    proof_result_with_bad_base64["proof_base64"] = "AAAA"
    with pytest.raises(
        TypeError,
        match=r"proofResult\.proofBase64 must match proofResult\.proofBytes",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result_with_bad_base64,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    duplicate_proof_bytes_result = dict(proof_result)
    duplicate_proof_bytes_result["proofBytes"] = proof_result["proof_bytes"]
    with pytest.raises(
        TypeError,
        match=r"proofResult\.proofBytes.*multiple aliases",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": duplicate_proof_bytes_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    duplicate_proof_context_hash_result = dict(proof_result)
    duplicate_proof_context_hash_result["proofContextHash"] = proof_result[
        "proof_context_hash"
    ]
    with pytest.raises(
        TypeError,
        match=r"proofResult\.proofContextHash.*multiple aliases",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": duplicate_proof_context_hash_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    duplicate_public_inputs_result = dict(proof_result)
    duplicate_public_inputs_result["public_inputs"] = dict(proof_result["public_inputs"])
    duplicate_public_inputs_result["public_inputs"]["bankHash"] = proof_result[
        "public_inputs"
    ]["bank_hash"]
    with pytest.raises(
        TypeError,
        match=r"proofResult\.publicInputs\.bankHash.*multiple aliases",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": duplicate_public_inputs_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    proof_result_with_zero_witness = dict(proof_result)
    proof_result_with_zero_witness["witness_hash"] = SCCP_ZERO_HASH_V1
    with pytest.raises(
        TypeError,
        match=r"proofResult\.witnessHash must not be zero",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result_with_zero_witness,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    mismatched_proof_context_result = dict(proof_result)
    mismatched_proof_context_result["proof_context_hash"] = HEX32_C
    with pytest.raises(TypeError, match="proofContextHash must match"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": mismatched_proof_context_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    proof_result_with_bad_context_version = dict(proof_result)
    proof_result_with_bad_context_version["proof_context"] = {
        **proof_result["proof_context"],
        "version": 2,
    }
    with pytest.raises(
        TypeError,
        match=r"proofResult\.proofContext\.version must be 1",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result_with_bad_context_version,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"proofContext must be an object"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "proof_context": None,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"proofBytes must be bytes"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "proof_bytes": None,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"statementHash"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "statement_hash": None,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"proofContextHash"):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "proof_context_hash": None,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(TypeError, match=r"publicInputs"):
        build_solana_sccp_submission(
            {
                "public_inputs": None,
                "transparent_public_inputs": transparent_public_inputs,
                "proof_result": proof_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    missing_deployment_binding_result = dict(proof_result)
    del missing_deployment_binding_result["source_adapter_deployment_binding"]
    with pytest.raises(
        TypeError,
        match=r"proofResult\.sourceAdapterDeploymentBinding is required",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": missing_deployment_binding_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    proof_result_with_bad_binding_version = dict(proof_result)
    proof_result_with_bad_binding_version["source_adapter_deployment_binding"] = {
        **proof_result["source_adapter_deployment_binding"],
        "version": 2,
    }
    with pytest.raises(
        TypeError,
        match=r"proofResult\.sourceAdapterDeploymentBinding\.version must be 1",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": proof_result_with_bad_binding_version,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    tampered_deployment_binding_result = dict(proof_result)
    tampered_deployment_binding_result["source_adapter_deployment_binding"] = {
        **proof_result["source_adapter_deployment_binding"],
        "source_adapter_deployment_hash": HEX32_C,
    }
    with pytest.raises(
        TypeError,
        match="sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": tampered_deployment_binding_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    mismatched_deployment_public_inputs_result = dict(proof_result)
    mismatched_deployment_public_inputs_result["public_inputs"] = {
        **proof_result["public_inputs"],
        "source_adapter_deployment_hash": HEX32_C,
    }
    with pytest.raises(
        TypeError,
        match=(
            r"proofResult\.publicInputs\.sourceAdapterDeploymentHash must match "
            "sourceAdapterDeploymentBinding"
        ),
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": mismatched_deployment_public_inputs_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    mismatched_source_verifier_id_result = dict(proof_result)
    mismatched_source_verifier_id_result["public_inputs"] = {
        **proof_result["public_inputs"],
        "source_state_verifier_id": "sccp:solana:wrong-source-state-verifier:v1",
    }
    with pytest.raises(
        TypeError,
        match=(
            r"proofResult\.publicInputs\.sourceStateVerifierId must match "
            r"proofResult\.sourceStateVerifierId"
        ),
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": mismatched_source_verifier_id_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    mismatched_source_verifier_hash_result = dict(proof_result)
    mismatched_source_verifier_hash_result["public_inputs"] = {
        **proof_result["public_inputs"],
        "source_state_verifier_hash": HEX32_D,
    }
    with pytest.raises(
        TypeError,
        match=(
            r"proofResult\.publicInputs\.sourceStateVerifierHash must match "
            r"proofResult\.sourceStateVerifierHash"
        ),
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": mismatched_source_verifier_hash_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    bad_parent_slot_result = dict(proof_result)
    bad_parent_slot_result["public_inputs"] = {
        **proof_result["public_inputs"],
        "parent_slot": proof_result["public_inputs"]["finalized_slot"],
    }
    with pytest.raises(
        TypeError,
        match=r"proofResult\.publicInputs\.parentSlot must be the direct parent",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": bad_parent_slot_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    zero_message_proof_result = dict(proof_result)
    zero_message_proof_result["public_inputs"] = {
        **proof_result["public_inputs"],
        "message_proof_hash": SCCP_ZERO_HASH_V1,
    }
    with pytest.raises(
        TypeError,
        match=r"proofResult\.publicInputs\.messageProofHash must not be zero",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_result": zero_message_proof_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.publicInputs\.messageId must match",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": {
                    **transparent_public_inputs,
                    "message_id": HEX32_A,
                },
                "proof_result": proof_result,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(
        TypeError,
        match="proofResult must be a wrapped Solana SCCP proof result",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": transparent_public_inputs,
                "proof_bytes": proof_result["proof_bytes"],
                "bundle_bytes": bytes([8]),
                "statement_hash": HEX32_G,
                "destination_binding_hash": solana_destination_binding_hash,
            }
        )

    mismatched_public_inputs_bytes = bytearray(
        canonical_sccp_message_transparent_public_inputs_bytes(
            submission["public_inputs"]
        )
    )
    mismatched_public_inputs_bytes[5] ^= 1
    with pytest.raises(TypeError, match=r"publicInputs\.targetDomain must be Solana"):
        build_solana_sccp_submission(
            {
                "public_inputs": {
                    **submission["public_inputs"],
                    "target_domain": SCCP_DOMAIN_SORA,
                },
                "proof_result": proof_result,
                "proof_bytes": [1, 2],
                "bundle_bytes": [5, 6, 7],
                "statement_hash": HEX32_G,
                "destination_binding_hash": solana_destination_binding_hash,
            }
        )

    with pytest.raises(TypeError, match="publicInputsBytes must match canonical"):
        build_solana_sccp_submission(
            {
                "public_inputs": submission["public_inputs"],
                "public_inputs_bytes": bytes(mismatched_public_inputs_bytes),
                "proof_result": proof_result,
                "proof_bytes": [1, 2],
                "bundle_bytes": [5, 6, 7],
                "statement_hash": HEX32_G,
                "destination_binding_hash": solana_destination_binding_hash,
            }
        )

    with pytest.raises(
        TypeError,
        match="destinationBindingHash must match canonical Solana destination binding",
    ):
        build_solana_sccp_submission(
            {
                "public_inputs": submission["public_inputs"],
                "proof_result": proof_result,
                "proof_bytes": [1],
                "bundle_bytes": [2],
                "statement_hash": HEX32_G,
                "destination_binding_hash": HEX32_H,
            }
        )

    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        build_solana_sccp_submission(
            {
                "public_inputs": submission["public_inputs"],
                "proof_result": proof_result,
                "proof_bytes": [0, 0],
                "bundle_bytes": [2],
                "statement_hash": HEX32_G,
                "destination_binding_hash": HEX32_H,
            }
        )

    with pytest.raises(TypeError, match="bundleBytes must not be all zero"):
        build_solana_sccp_submission(
            {
                "public_inputs": submission["public_inputs"],
                "proof_result": proof_result,
                "proof_bytes": [1],
                "bundle_bytes": [0, 0],
                "statement_hash": HEX32_G,
                "destination_binding_hash": solana_destination_binding_hash,
            }
        )

    with pytest.raises(TypeError, match="bundleBytes must be at most"):
        build_solana_sccp_submission(
            {
                "public_inputs": submission["public_inputs"],
                "proof_result": proof_result,
                "proof_bytes": [1],
                "bundle_bytes": b"\x01" * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1),
                "statement_hash": HEX32_G,
                "destination_binding_hash": solana_destination_binding_hash,
            }
        )

    with pytest.raises(TypeError, match="proofContextHash must match"):
        build_solana_sccp_submission(
            {
                "public_inputs": submission["public_inputs"],
                "proof_result": proof_result,
                "proof_bytes": [1],
                "bundle_bytes": [2],
                "statement_hash": HEX32_G,
                "destination_binding_hash": solana_destination_binding_hash,
                "proof_context_hash": HEX32_C,
            }
        )


def test_solana_sccp_prover_requires_linked_engine() -> None:
    prover = SolanaSccpProver()

    with pytest.raises(SolanaSccpProverUnavailableError) as exc:
        asyncio.run(prover.prove(sample_witness()))

    assert exc.value.code == "ERR_SCCP_SOLANA_PROVER_UNAVAILABLE"


def test_solana_sccp_prover_wraps_externally_generated_proof_bytes() -> None:
    production_witness = sample_production_witness()
    callback_request = None

    async def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> Dict[str, Any]:
        nonlocal callback_request
        callback_request = request
        assert request["backend"] == SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1
        assert request["proof_context"]["statement_hash"] == HEX32_G
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "proof_base64": base64.b64encode(GROTH16_PROOF_BYTES).decode("ascii"),
        }

    result = asyncio.run(SolanaSccpProver(prove=prove).prove(production_witness))
    request = build_solana_sccp_proof_request(production_witness)
    direct_result = wrap_solana_sccp_proof_result(GROTH16_PROOF_BYTES, request)

    assert callback_request is not request
    assert callback_request == request
    assert result["proof_bytes"] == GROTH16_PROOF_BYTES
    assert len(result["proof_base64"]) > 0
    assert result["proof_context_hash"] == request["proof_context_hash"]
    assert direct_result["envelope_hash"] == result["envelope_hash"]
    assert len(result["envelope_hash"]) == 66
    with pytest.raises(TypeError, match="at most"):
        wrap_solana_sccp_proof_result(
            b"\x01" * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1),
            request,
        )
    mutated_request = dict(request)
    mutated_request["witness_hash"] = HEX32_A
    with pytest.raises(TypeError, match="proof request must be canonical"):
        wrap_solana_sccp_proof_result([1], mutated_request)
    mutated_public_inputs = dict(request)
    mutated_public_inputs["public_inputs"] = dict(request["public_inputs"])
    mutated_public_inputs["public_inputs"]["bank_hash"] = HEX32_B
    with pytest.raises(TypeError, match="proof request must be canonical"):
        wrap_solana_sccp_proof_result([1], mutated_public_inputs)

    async def mismatched_public_inputs(
        request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        public_inputs = dict(request["public_inputs"])
        public_inputs["message_id"] = HEX32_A
        return {"proof_bytes": [1, 2, 3, 4], "public_inputs": public_inputs}

    with pytest.raises(TypeError, match=r"proofResult\.publicInputs must match request"):
        asyncio.run(SolanaSccpProver(prove=mismatched_public_inputs).prove(production_witness))

    async def mismatched_proof_base64(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [1, 2, 3, 4], "proof_base64": "AAAA"}

    with pytest.raises(TypeError, match=r"proofResult\.proofBase64"):
        asyncio.run(SolanaSccpProver(prove=mismatched_proof_base64).prove(production_witness))

    async def duplicate_proof_bytes(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": [1, 2, 3, 4],
            "proofBytes": [1, 2, 3, 4],
        }

    with pytest.raises(TypeError, match=r"proofResult\.proofBytes.*multiple aliases"):
        asyncio.run(SolanaSccpProver(prove=duplicate_proof_bytes).prove(production_witness))

    async def duplicate_source_verifier_id(
        request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": [1, 2, 3, 4],
            "source_state_verifier_id": request["source_state_verifier_id"],
            "sourceStateVerifierId": request["source_state_verifier_id"],
        }

    with pytest.raises(
        TypeError,
        match=r"proofResult\.sourceStateVerifierId.*multiple aliases",
    ):
        asyncio.run(
            SolanaSccpProver(prove=duplicate_source_verifier_id).prove(
                production_witness
            )
        )

    async def duplicate_public_input_alias(
        request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        public_inputs = dict(request["public_inputs"])
        public_inputs["bankHash"] = public_inputs["bank_hash"]
        return {"proof_bytes": [1, 2, 3, 4], "public_inputs": public_inputs}

    with pytest.raises(
        TypeError,
        match=r"proofResult\.publicInputs\.bankHash.*multiple aliases",
    ):
        asyncio.run(
            SolanaSccpProver(prove=duplicate_public_input_alias).prove(
                production_witness
            )
        )

    async def mismatched_proof_context(
        request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        proof_context = dict(request["proof_context"])
        proof_context["statement_hash"] = HEX32_A
        return {"proof_bytes": [1, 2, 3, 4], "proof_context": proof_context}

    with pytest.raises(TypeError, match=r"proofResult\.proofContext must match request"):
        asyncio.run(SolanaSccpProver(prove=mismatched_proof_context).prove(production_witness))

    async def zero_proof(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [0, 0]}

    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(SolanaSccpProver(prove=zero_proof).prove(production_witness))

    async def unexpected_prover(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        raise AssertionError("local prover should not be invoked")

    with pytest.raises(TypeError, match="mainnetGenesisHash"):
        asyncio.run(
            SolanaSccpProver(prove=unexpected_prover).prove(
                sample_production_witness(mainnet_genesis_hash="devnet")
            )
        )
    for bad_genesis_hash in ("", False, 0):
        with pytest.raises(TypeError, match="mainnetGenesisHash"):
            asyncio.run(
                SolanaSccpProver(prove=unexpected_prover).prove(
                    sample_production_witness(mainnet_genesis_hash=bad_genesis_hash)
                )
            )

    with pytest.raises(TypeError, match="accountsLtHash"):
        asyncio.run(
            SolanaSccpProver(prove=unexpected_prover).prove(
                sample_production_witness(accounts_lt_hash=None)
            )
        )

    with pytest.raises(TypeError, match="sourceStateVerifierHash must not be zero"):
        asyncio.run(SolanaSccpProver(prove=unexpected_prover).prove(sample_witness()))

    with pytest.raises(TypeError, match="Solana template verifier hash"):
        asyncio.run(
            SolanaSccpProver(prove=unexpected_prover).prove(
                sample_production_witness(
                    source_state_verifier_hash=(
                        SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1
                    )
                )
            )
        )

    with pytest.raises(TypeError, match="inclusionBranch must not be empty"):
        asyncio.run(
            SolanaSccpProver(prove=unexpected_prover).prove(
                sample_witness(
                    source_state_verifier_hash=HEX32_C,
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
        )


def test_ton_sccp_prover_requires_linked_engine() -> None:
    prover = TonSccpProver()

    with pytest.raises(TonSccpProverUnavailableError) as exc:
        asyncio.run(
            prover.prove(
                sample_ton_request_input(
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
        )

    assert exc.value.code == "ERR_SCCP_TON_PROVER_UNAVAILABLE"


def test_ton_sccp_prover_rejects_non_production_input_before_callback() -> None:
    invoked = False

    async def prove(_request: Mapping[str, Any], _options: Mapping[str, Any]) -> Dict[str, Any]:
        nonlocal invoked
        invoked = True
        return {"proof_bytes": b"\x01\x02\x03\x04"}

    with pytest.raises(TypeError, match="sourceStateVerifierHash"):
        asyncio.run(
            TonSccpProver(prove=prove).prove(
                sample_ton_request_input(
                    source_state_verifier_hash=SCCP_ZERO_HASH_V1,
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
        )

    assert not invoked


def test_sccp_provers_accept_callable_and_camel_case_witness_providers() -> None:
    evm_public_inputs = sample_evm_public_inputs()
    evm_bundle_bytes = [5, 6, 7]
    evm_input = sample_evm_production_request_input(
        public_inputs=evm_public_inputs,
        bundle_bytes=evm_bundle_bytes,
        source_proof_bytes=b"",
    )

    async def evm_witness_provider(
        input_value: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Mapping[str, Any]:
        assert options["portal"] is True
        assert input_value is not evm_input
        assert input_value["public_inputs"] is not evm_public_inputs
        assert input_value["bundle_bytes"] is not evm_bundle_bytes
        input_value["public_inputs"]["message_id"] = HEX32_B
        input_value["bundle_bytes"][0] = 0xFF
        witness = dict(evm_input)
        witness["source_proof_bytes"] = bytes([9, 10])
        return witness

    async def evm_prove(
        request: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        assert options["portal"] is True
        assert request["source_proof_bytes"] == bytes([9, 10])
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    evm_result = asyncio.run(
        EvmSccpProver(
            witness_provider=evm_witness_provider,
            prove=evm_prove,
        ).prove(evm_input, portal=True)
    )

    assert evm_result["source_proof_bytes"] == bytes([9, 10])
    assert evm_input["public_inputs"]["message_id"] == "0x" + "11" * 32
    assert evm_input["bundle_bytes"] == [5, 6, 7]

    class TonWitnessProvider:
        async def resolveWitness(
            self,
            input_value: Mapping[str, Any],
            options: Mapping[str, Any],
        ) -> Mapping[str, Any]:
            assert options["mobile"] is True
            witness = dict(input_value)
            witness["source_adapter_deployment_hash"] = HEX32_A
            witness["source_adapter_deployment_receipt_hash"] = HEX32_B
            return witness

    async def ton_prove(
        request: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        assert options["mobile"] is True
        assert request["source_adapter_deployment_binding"][
            "source_adapter_deployment_hash"
        ] == HEX32_A
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    ton_result = asyncio.run(
        TonSccpProver(
            witness_provider=TonWitnessProvider(),
            prove=ton_prove,
        ).prove(sample_ton_request_input(), mobile=True)
    )

    assert ton_result["source_adapter_deployment_binding"][
        "source_adapter_deployment_receipt_hash"
    ] == HEX32_B

    with pytest.raises(TypeError, match="resolve_witness/resolveWitness"):
        asyncio.run(
            EvmSccpProver(witness_provider=object()).build_request(
                sample_evm_request_input()
            )
        )

    class DuplicateResolverWitnessProvider:
        async def resolve_witness(
            self,
            input_value: Mapping[str, Any],
            options: Mapping[str, Any],
        ) -> Mapping[str, Any]:
            return input_value

        async def resolveWitness(
            self,
            input_value: Mapping[str, Any],
            options: Mapping[str, Any],
        ) -> Mapping[str, Any]:
            return input_value

    with pytest.raises(
        TypeError,
        match="witness_provider resolver must not use multiple aliases",
    ):
        asyncio.run(
            SolanaSccpProver(
                witness_provider=DuplicateResolverWitnessProvider(),
                prove=lambda request, options: {"proof_bytes": b"\x01\x02\x03\x04"},
            ).build_request(sample_production_witness())
        )


def test_sccp_witness_provider_snapshots_mutable_sequence_inputs() -> None:
    bundle_bytes = deque([5, 6, 7])
    source_proof_bytes = deque([9, 10])
    input_value = sample_evm_production_request_input(
        bundle_bytes=bundle_bytes,
        source_proof_bytes=source_proof_bytes,
    )

    async def evm_witness_provider(
        input_snapshot: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Mapping[str, Any]:
        assert input_snapshot is not input_value
        assert input_snapshot["bundle_bytes"] is not bundle_bytes
        assert input_snapshot["source_proof_bytes"] is not source_proof_bytes
        input_snapshot["bundle_bytes"][0] = 0xFF  # type: ignore[index]
        input_snapshot["source_proof_bytes"].append(0xFF)  # type: ignore[attr-defined]
        return {
            **input_snapshot,
            "bundle_bytes": [5, 6, 7],
            "source_proof_bytes": [9, 10],
        }

    request = asyncio.run(
        EvmSccpProver(witness_provider=evm_witness_provider).build_request(input_value)
    )

    assert list(bundle_bytes) == [5, 6, 7]
    assert list(source_proof_bytes) == [9, 10]
    assert request["bundle_bytes"] == bytes([5, 6, 7])
    assert request["source_proof_bytes"] == bytes([9, 10])


def test_sccp_provers_resolve_ui_witnesses_before_linked_provers() -> None:
    resolved_destination_binding_hash = sccp_destination_binding_hash(SCCP_DOMAIN_SOL)
    expected_solana_request = build_solana_sccp_proof_request(
        sample_production_witness(destination_binding_hash=resolved_destination_binding_hash)
    )
    solana_resolved = False

    async def solana_witness_provider(
        input_value: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Mapping[str, Any]:
        nonlocal solana_resolved
        assert options["portal"] is True
        assert input_value["destination_binding_hash"] == HEX32_H
        solana_resolved = True
        return {
            **input_value,
            "destination_binding_hash": resolved_destination_binding_hash,
        }

    async def solana_prove(
        request: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        assert options["portal"] is True
        assert solana_resolved
        assert (
            request["proof_context"]["destination_binding_hash"]
            == resolved_destination_binding_hash
        )
        assert request["proof_context_hash"] == expected_solana_request["proof_context_hash"]
        return {"proof_bytes": bytes([1, 2, 3, 4])}

    solana_result = asyncio.run(
        SolanaSccpProver(
            witness_provider=solana_witness_provider,
            prove=solana_prove,
        ).prove(sample_production_witness(), portal=True)
    )
    assert solana_result["witness_hash"] == expected_solana_request["witness_hash"]
    assert (
        solana_result["proof_context_hash"]
        == expected_solana_request["proof_context_hash"]
    )

    ton_resolved = False

    async def ton_witness_provider(
        input_value: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Mapping[str, Any]:
        nonlocal ton_resolved
        assert options["portal"] is True
        assert input_value["source_proof_bytes"] == b""
        ton_resolved = True
        return {
            **input_value,
            "source_proof_bytes": bytes([9, 10]),
            "source_adapter_deployment_hash": HEX32_A,
            "source_adapter_deployment_receipt_hash": HEX32_B,
        }

    async def ton_prove(
        request: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        assert options["portal"] is True
        assert ton_resolved
        assert request["source_proof_bytes"] == bytes([9, 10])
        return {"proof_bytes": bytes([1, 2, 3, 4])}

    ton_result = asyncio.run(
        TonSccpProver(
            witness_provider=ton_witness_provider,
            prove=ton_prove,
        ).prove(sample_ton_request_input(source_proof_bytes=b""), portal=True)
    )
    assert ton_result["source_proof_bytes"] == bytes([9, 10])

    tron_resolved = False

    async def tron_witness_provider(
        input_value: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Mapping[str, Any]:
        nonlocal tron_resolved
        assert options["portal"] is True
        assert input_value["source_proof_bytes"] == b""
        tron_resolved = True
        return {**input_value, "source_proof_bytes": bytes([9, 10])}

    async def tron_prove(
        request: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        assert options["portal"] is True
        assert tron_resolved
        assert request["source_proof_bytes"] == bytes([9, 10])
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    tron_result = asyncio.run(
        TronSccpProver(
            witness_provider=tron_witness_provider,
            prove=tron_prove,
            ).prove(sample_tron_production_request_input(source_proof_bytes=b""), portal=True)
        )
    assert tron_result["source_proof_bytes"] == bytes([9, 10])

    substrate_resolved = False

    class SubstrateWitnessProvider:
        async def resolve_witness(
            self,
            input_value: Mapping[str, Any],
            options: Mapping[str, Any],
        ) -> Mapping[str, Any]:
            nonlocal substrate_resolved
            assert options["portal"] is True
            assert input_value["source_proof_bytes"] == b""
            substrate_resolved = True
            return {**input_value, "source_proof_bytes": bytes([9, 10])}

    async def substrate_prove(
        request: Mapping[str, Any],
        options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        assert options["portal"] is True
        assert substrate_resolved
        assert request["source_proof_bytes"] == bytes([9, 10])
        return {"proof_bytes": bytes([1, 2, 3, 4])}

    substrate_result = asyncio.run(
        SubstrateSccpProver(
            witness_provider=SubstrateWitnessProvider(),
            prove=substrate_prove,
        ).prove(sample_substrate_request_input(source_proof_bytes=b""), portal=True)
    )
    assert substrate_result["source_proof_bytes"] == bytes([9, 10])


def test_ton_sccp_proof_request_requires_ton_source_domain() -> None:
    with pytest.raises(TypeError, match="sourceDomain must be TON"):
        build_ton_sccp_proof_request(
            sample_ton_request_input(source_domain=SCCP_DOMAIN_SOL)
        )


def test_ton_sccp_prover_wraps_externally_generated_proof_bytes() -> None:
    callback_request = None

    async def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> Dict[str, Any]:
        nonlocal callback_request
        callback_request = request
        assert request["backend"] == SCCP_TON_CONTRACT_PROOF_BACKEND_V1
        assert isinstance(request["bundle_bytes"], bytes)
        assert isinstance(request["source_proof_bytes"], bytes)
        assert request["bundle_bytes"] == bytes([5, 6, 7])
        assert request["source_proof_bytes"] == bytes([9, 10])
        assert request["proof_context"]["statement_hash"] == HEX32_G
        assert request["source_adapter_deployment_binding"]["source_domain"] == SCCP_DOMAIN_TON
        with pytest.raises(TypeError, match="immutable"):
            request["bundle_bytes"] = b"\x00"
        with pytest.raises(TypeError, match="immutable"):
            request["source_proof_bytes"] = b"\x00"
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    bundle_bytes = bytearray([5, 6, 7])
    source_proof_bytes = bytearray([9, 10])
    input_value = sample_ton_request_input(
        bundle_bytes=bundle_bytes,
        source_proof_bytes=source_proof_bytes,
        source_adapter_deployment_hash=HEX32_A,
        source_adapter_deployment_receipt_hash=HEX32_B,
    )
    result = asyncio.run(TonSccpProver(prove=prove).prove(input_value))
    request = build_ton_sccp_proof_request(input_value)
    direct_result = wrap_ton_sccp_proof_result(GROTH16_PROOF_BYTES, request)

    assert callback_request is not request
    assert callback_request == request
    assert result["proof_bytes"] == GROTH16_PROOF_BYTES
    assert len(result["proof_base64"]) > 0
    assert result["request_hash"] == request["request_hash"]
    assert direct_result["envelope_hash"] == result["envelope_hash"]
    assert result["bundle_bytes"] == request["bundle_bytes"]
    assert result["source_proof_bytes"] == request["source_proof_bytes"]
    assert result["statement_hash"] == HEX32_G
    assert result["destination_binding_hash"] == HEX32_H
    assert result["source_state_verifier_id"] == SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1
    assert result["source_state_verifier_hash"] == HEX32_C
    assert result["source_adapter_deployment_binding_hash"] == (
        request["source_adapter_deployment_binding_hash"]
    )
    assert len(result["envelope_hash"]) == 66

    async def duplicate_request_hash_aliases(
        linked_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        wrapped = wrap_ton_sccp_proof_result(GROTH16_PROOF_BYTES, linked_request)
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "request_hash": wrapped["request_hash"],
            "requestHash": wrapped["request_hash"],
        }

    with pytest.raises(TypeError, match="proofResult.requestHash.*multiple aliases"):
        asyncio.run(TonSccpProver(prove=duplicate_request_hash_aliases).prove(input_value))

    async def mismatched_public_inputs(
        _request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "public_inputs": sample_ton_public_inputs(message_id=HEX32_B),
        }

    with pytest.raises(TypeError, match=r"proofResult\.publicInputs must match request"):
        asyncio.run(TonSccpProver(prove=mismatched_public_inputs).prove(input_value))

    with pytest.raises(TypeError, match="at most"):
        wrap_ton_sccp_proof_result(
            b"\x01" * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1), request
        )


def test_evm_family_sccp_prover_requires_linked_engine() -> None:
    prover = EvmSccpProver()

    with pytest.raises(EvmSccpProverUnavailableError) as exc:
        asyncio.run(prover.prove(sample_evm_request_input()))

    assert exc.value.code == "ERR_SCCP_EVM_PROVER_UNAVAILABLE"


def test_evm_family_sccp_prover_wraps_externally_generated_proof_bytes() -> None:
    callback_request = None

    async def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> Dict[str, Any]:
        nonlocal callback_request
        callback_request = request
        assert request["backend"] == SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
        assert isinstance(request["bundle_bytes"], bytes)
        assert isinstance(request["source_proof_bytes"], bytes)
        assert request["bundle_bytes"] == bytes([5, 6, 7])
        assert request["source_proof_bytes"] == bytes([9, 10])
        assert request["proof_context"]["statement_hash"] == "0x" + "55" * 32
        assert request["target_domain"] == SCCP_DOMAIN_ETH
        assert len(request["public_signal_words"]) == 9
        with pytest.raises(TypeError, match="immutable"):
            request["bundle_bytes"] = b"\x00"
        with pytest.raises(TypeError, match="immutable"):
            request["source_proof_bytes"] = b"\x00"
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    bundle_bytes = bytearray([5, 6, 7])
    source_proof_bytes = bytearray([9, 10])
    input_value = sample_evm_production_request_input(
        bundle_bytes=bundle_bytes,
        source_proof_bytes=source_proof_bytes,
    )
    result = asyncio.run(EvmSccpProver(prove=prove).prove(input_value))
    request = build_evm_sccp_proof_request(input_value)
    direct_result = wrap_evm_sccp_proof_result(GROTH16_PROOF_BYTES, request)

    assert callback_request is not request
    assert callback_request == request
    async def prove_with_request_hash_aliases(
        linked_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "request_hash": linked_request["request_hash"],
            "requestHash": linked_request["request_hash"],
        }

    with pytest.raises(TypeError, match=r"proofResult\.requestHash.*multiple aliases"):
        asyncio.run(EvmSccpProver(prove=prove_with_request_hash_aliases).prove(input_value))

    async def prove_with_envelope_hash_aliases(
        linked_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        wrapped = wrap_evm_sccp_proof_result(GROTH16_PROOF_BYTES, linked_request)
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "envelope_hash": wrapped["envelope_hash"],
            "envelopeHash": wrapped["envelope_hash"],
        }

    with pytest.raises(TypeError, match=r"proofResult\.envelopeHash.*multiple aliases"):
        asyncio.run(EvmSccpProver(prove=prove_with_envelope_hash_aliases).prove(input_value))

    assert result["proof_bytes"] == GROTH16_PROOF_BYTES
    assert len(result["proof_base64"]) > 0
    assert result["request_hash"] == request["request_hash"]
    assert direct_result["envelope_hash"] == result["envelope_hash"]
    assert result["public_signal_words"] == request["public_signal_words"]
    assert result["bundle_bytes"] == bytes([5, 6, 7])
    assert result["source_proof_bytes"] == bytes([9, 10])
    assert result["statement_hash"] == "0x" + "55" * 32
    assert result["destination_binding_hash"] == input_value["destination_binding"]["binding_hash"]
    assert result["destination_binding"] == input_value["destination_binding"]
    assert len(result["envelope_hash"]) == 66
    with pytest.raises(TypeError, match="at most"):
        wrap_substrate_sccp_proof_result(
            b"\x01" * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1), request
        )


def test_ethereum_mainnet_sccp_facade_requires_chain_id_1_and_eth_target() -> None:
    raw_binding = {
        "verifier_address": "0x" + "11" * 20,
        "bridge_address": "0x" + "22" * 20,
        "verifier_code_hash": "0x" + "bb" * 32,
        "verifier_key_hash": "0x" + "cc" * 32,
    }
    binding = ethereum_mainnet_sccp_destination_binding(raw_binding)

    assert SCCP_ETH_MAINNET_EVM_CHAIN_ID == 1
    assert require_ethereum_mainnet_chain_id("1") == 1
    assert require_ethereum_mainnet_chain_id("0x1") == 1
    assert EthereumMainnetSccp.require_mainnet_chain_id(1) == 1
    with pytest.raises(ValueError, match="eth_chainId == 1"):
        require_ethereum_mainnet_chain_id(56)
    assert binding["target_domain"] == SCCP_DOMAIN_ETH
    assert binding["network_id"] == SCCP_ETH_MAINNET_NETWORK_ID
    assert (
        ethereum_mainnet_sccp_destination_binding_hash(
            {**raw_binding, "binding_hash": binding["binding_hash"]}
        )
        == binding["binding_hash"]
    )
    assert EthereumMainnetSccp.destination_binding(raw_binding) == binding
    assert (
        EthereumMainnetSccp.destination_binding_hash(raw_binding)
        == binding["binding_hash"]
    )

    input_value = sample_evm_request_input(
        public_inputs=sample_evm_public_inputs(target_domain=SCCP_DOMAIN_ETH),
        destination_binding=binding,
        destination_binding_hash=None,
    )
    request = build_ethereum_mainnet_sccp_destination_proof_request(input_value)
    facade_request = asyncio.run(
        EthereumMainnetSccp().build_outbound_proof_request(input_value)
    )
    with pytest.raises(
        TypeError,
        match="destinationBindingHash must match destinationBinding",
    ):
        asyncio.run(
            EthereumMainnetSccp().build_outbound_proof_request(
                {
                    **input_value,
                    "destination_binding_hash": "0x" + "99" * 32,
                }
            )
        )
    proof_result = wrap_ethereum_mainnet_sccp_destination_proof_result(
        GROTH16_PROOF_BYTES,
        request,
    )
    submission = build_ethereum_mainnet_sccp_destination_submission(
        {"proof_result": proof_result}
    )
    facade_submission = EthereumMainnetSccp().build_ethereum_calldata(
        {"proofResult": proof_result}
    )

    assert request["target_domain"] == SCCP_DOMAIN_ETH
    assert request["request_hash"] == facade_request["request_hash"]
    assert proof_result["destination_binding_hash"] == binding["binding_hash"]
    assert submission["target_domain"] == SCCP_DOMAIN_ETH
    assert facade_submission["destination_binding_hash"] == binding["binding_hash"]

    async def submit_outbound(
        callback_submission: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> str:
        assert callback_submission["target_domain"] == SCCP_DOMAIN_ETH
        assert callback_submission["call_data_hex"] == submission["call_data_hex"]
        return "eth-submitted"

    submitted = asyncio.run(
        EthereumMainnetSccp(
            submit_outbound_to_ethereum=submit_outbound
        ).submit_outbound_to_ethereum({"proof_result": proof_result})
    )
    assert submitted == "eth-submitted"

    class WrongChainProvider:
        async def request(self, method: str, params: list[Any]) -> Any:
            assert method == "eth_chainId"
            assert params == []
            return "0x38"

    guarded_submit_called = False

    async def guarded_submit(
        _callback_submission: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> str:
        nonlocal guarded_submit_called
        guarded_submit_called = True
        return "wrong-chain"

    with pytest.raises(ValueError, match="eth_chainId == 1"):
        asyncio.run(
            EthereumMainnetSccp(
                execution_provider=WrongChainProvider(),
                submit_outbound_to_ethereum=guarded_submit,
            ).submit_outbound_to_ethereum({"proof_result": proof_result})
        )
    assert guarded_submit_called is False

    with pytest.raises(EvmSccpProverUnavailableError, match="outbound submitter"):
        asyncio.run(
            EthereumMainnetSccp().submit_outbound_to_ethereum(
                {"proof_result": proof_result}
            )
        )

    async def prove(
        callback_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        assert callback_request["target_domain"] == SCCP_DOMAIN_ETH
        assert (
            callback_request["destination_binding"]["network_id"]
            == SCCP_ETH_MAINNET_NETWORK_ID
        )
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    async_result = asyncio.run(
        EthereumMainnetSccp(prove=prove).prove_outbound_to_ethereum(input_value)
    )
    assert async_result["destination_binding_hash"] == binding["binding_hash"]

    outbound_prover_called = False

    async def reject_bsc_prove(
        _callback_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        nonlocal outbound_prover_called
        outbound_prover_called = True
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    with pytest.raises((TypeError, ValueError), match="request route|target ETH|Ethereum mainnet"):
        asyncio.run(
            EthereumMainnetSccp(prove=reject_bsc_prove).prove_outbound_to_ethereum(
                sample_evm_request_input(
                    public_inputs=sample_evm_public_inputs(target_domain=SCCP_DOMAIN_BSC),
                    destination_binding=binding,
                    destination_binding_hash=None,
                )
            )
        )
    assert not outbound_prover_called

    with pytest.raises(ValueError, match="chain id 1"):
        ethereum_mainnet_sccp_destination_binding(
            {**raw_binding, "network_id": "0x" + "aa" * 32}
        )

    with pytest.raises(
        (TypeError, ValueError),
        match="request route|target ETH|Ethereum mainnet",
    ):
        build_ethereum_mainnet_sccp_destination_proof_request(
            sample_evm_request_input(
                public_inputs=sample_evm_public_inputs(target_domain=SCCP_DOMAIN_BSC),
                destination_binding=binding,
                destination_binding_hash=None,
            )
        )

    with pytest.raises((TypeError, ValueError), match="proof result"):
        build_ethereum_mainnet_sccp_destination_submission(
            {
                "public_inputs": sample_evm_public_inputs(target_domain=SCCP_DOMAIN_ETH),
                "proof_bytes": GROTH16_PROOF_BYTES,
                "statement_hash": HEX32_G,
                "destination_binding_hash": binding["binding_hash"],
            }
        )

    with pytest.raises((TypeError, ValueError), match="chain id 1|targetDomain"):
        build_ethereum_mainnet_sccp_destination_submission(
            {
                "proof_result": {
                    **proof_result,
                    "destination_binding": sample_evm_destination_binding(),
                }
            }
        )


def test_ethereum_mainnet_sccp_builds_local_admission_submission() -> None:
    input_value = {
        "source_domain": SCCP_DOMAIN_ETH,
        "target_domain": SCCP_DOMAIN_SORA,
        "proof_bytes": b"\x01\x02\x03",
        "public_inputs_bytes": b"\x04\x05\x06",
        "bundle_bytes": b"\x07\x08\x09",
        "envelope_bytes": b"\x0a\x0b\x0c",
        "statement_hash": "0x" + "66" * 32,
        "source_verifier_material_hash": "0x" + "77" * 32,
        "source_adapter_engine_deployment_hash": "0x" + "88" * 32,
    }
    submission = build_ethereum_mainnet_sccp_local_admission_submission(input_value)
    facade_submission = EthereumMainnetSccp().build_local_admission_submission(
        input_value
    )

    assert submission["platform_payload"] == SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1
    assert submission["envelope_encoding"] == SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1
    assert submission["verifier_entrypoint"] == SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1
    assert submission["source_domain"] == SCCP_DOMAIN_ETH
    assert submission["target_domain"] == SCCP_DOMAIN_SORA
    assert submission["arguments"] == []
    assert submission["proof_bytes"] == b"\x01\x02\x03"
    assert submission["public_inputs_bytes"] == b"\x04\x05\x06"
    assert submission["bundle_bytes"] == b"\x07\x08\x09"
    assert submission["envelope_bytes"] == b"\x0a\x0b\x0c"
    assert submission["local_admission"]["proof_bytes"] == b"\x01\x02\x03"
    assert facade_submission["envelope_hex"] == submission["envelope_hex"]

    with pytest.raises(ValueError, match="ETH -> SORA"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "source_domain": SCCP_DOMAIN_BSC}
        )
    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "proof_bytes": b"\x00\x00"}
        )
    with pytest.raises(TypeError, match="publicInputsBytes must not be all zero"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "public_inputs_bytes": b"\x00\x00"}
        )
    with pytest.raises(TypeError, match="bundleBytes must not be all zero"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "bundle_bytes": b"\x00\x00"}
        )
    with pytest.raises(TypeError, match="envelopeBytes must not be empty"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "envelope_bytes": b""}
        )
    with pytest.raises(TypeError, match="envelopeBytes must not be all zero"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "envelope_bytes": b"\x00\x00"}
        )
    with pytest.raises(TypeError, match="statementHash must not be zero"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "statement_hash": "0x" + "00" * 32}
        )
    with pytest.raises(TypeError, match="sourceVerifierMaterialHash must not be zero"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "source_verifier_material_hash": "0x" + "00" * 32}
        )
    with pytest.raises(TypeError, match="sourceAdapterEngineDeploymentHash must not be zero"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "source_adapter_engine_deployment_hash": "0x" + "00" * 32}
        )
    with pytest.raises(TypeError, match="metadata is not canonical"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "envelope_encoding": "abi_tuple_v1"}
        )
    with pytest.raises(TypeError, match="metadata is not canonical"):
        build_ethereum_mainnet_sccp_local_admission_submission(
            {**input_value, "proof_family": "debug-proof-family"}
        )


def test_ethereum_mainnet_sccp_facade_collects_inbound_receipts_and_copies_proofs() -> None:
    class ExecutionProvider:
        def __init__(self) -> None:
            self.calls: list[tuple[str, list[Any]]] = []

        async def request(self, method: str, params: list[Any]) -> Any:
            self.calls.append((method, params))
            if method == "eth_chainId":
                return "0x1"
            if method == "eth_getTransactionReceipt":
                assert params == [HEX32_A]
                return {
                    "transactionHash": HEX32_A,
                    "blockHash": HEX32_B,
                    "blockNumber": "0x1234",
                    "status": "0x1",
                    "logs": [source_event_log()],
                }
            if method == "eth_getBlockByHash":
                assert params == [HEX32_B, False]
                return {
                    "hash": HEX32_B,
                    "number": "0x1234",
                    "receiptsRoot": HEX32_C,
                }
            raise AssertionError(f"unexpected method {method}")

    class ConsensusProvider:
        async def collect_finality_evidence(
            self,
            evidence: Mapping[str, Any],
            _options: Mapping[str, Any],
        ) -> Mapping[str, Any]:
            assert evidence["transaction_hash"] == HEX32_A
            assert evidence["receipt"]["blockHash"] == HEX32_B
            assert evidence["block"]["receiptsRoot"] == HEX32_C
            return ethereum_beacon_finality()

    receipt_proof = {
        "source_domain": SCCP_DOMAIN_ETH,
        "source_event_digest": "0x" + "34" * 32,
        "beacon_slot": "11",
        "execution_block_number": "4660",
        "execution_block_hash": HEX32_B,
        "execution_receipts_root": HEX32_C,
        "beacon_finalized_root": HEX32_D,
        "sync_committee_root": HEX32_E,
        "receipt_root_index": "0",
        "receipt_trie_proof_nodes": [EVM_RECEIPT_STATE_MPT_NODE_HEX],
        "inclusion_branch": [HEX32_E],
    }
    receipt_proof_hash = evm_sccp_receipt_proof_hash(receipt_proof)

    async def prove_inbound(
        evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        assert evidence["source_domain"] == SCCP_DOMAIN_ETH
        assert evidence["target_domain"] == SCCP_DOMAIN_SORA
        assert evidence["beacon_finality"]["execution_block_number"] == "4660"
        assert evidence["beacon_finality"]["execution_block_hash"] == HEX32_B
        assert evidence["beacon_finality"]["execution_receipts_root"] == HEX32_C
        assert evidence["beacon_finality"]["finality_branch"] == ETHEREUM_FINALITY_BRANCH
        assert evidence["beacon_finality"]["sync_committee_participation"] == "342"
        assert evidence["receipt_proof_hash"] == receipt_proof_hash
        assert evidence["receipt_proof"]["execution_block_hash"] == HEX32_B
        assert evidence["source_event_digest"] == SOURCE_EVENT_DIGEST
        return b"\x07\x08\x09"

    submitted: list[bytes] = []

    async def submit_inbound(proof_bytes: bytes, _options: Mapping[str, Any]) -> str:
        submitted.append(proof_bytes)
        return "submitted"

    provider = ExecutionProvider()
    sdk = EthereumMainnetSccp(
        execution_provider=provider,
        consensus_provider=ConsensusProvider(),
        source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS,
        prove_inbound=prove_inbound,
        submit_inbound_to_iroha=submit_inbound,
    )

    evidence = asyncio.run(
        sdk.collect_inbound_evidence_from_receipt({"transaction_hash": HEX32_A})
    )
    assert evidence["source_domain"] == SCCP_DOMAIN_ETH
    assert evidence["target_domain"] == SCCP_DOMAIN_SORA
    assert evidence["transaction_hash"] == HEX32_A
    assert evidence["beacon_finality"]["finalized_header_root"] == HEX32_D
    assert evidence["beacon_finality"]["execution_block_number"] == "4660"

    proof = asyncio.run(
        sdk.prove_inbound_to_sora(
            {"transaction_hash": HEX32_A, "receipt_proof": receipt_proof}
        )
    )
    assert proof == b"\x07\x08\x09"

    async def prove_oversized_inbound(
        _evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        return b"\x01" * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1)

    oversized_sdk = EthereumMainnetSccp(
        execution_provider=provider,
        consensus_provider=ConsensusProvider(),
        source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS,
        prove_inbound=prove_oversized_inbound,
    )
    with pytest.raises(TypeError, match="proofBytes must be at most"):
        asyncio.run(
            oversized_sdk.prove_inbound_to_sora(
                {"transaction_hash": HEX32_A, "receipt_proof": receipt_proof}
            )
        )

    with pytest.raises(TypeError, match="receipt source event validation"):
        asyncio.run(
            EthereumMainnetSccp(prove_inbound=prove_inbound).prove_inbound_to_sora(
                {
                    "receipt": {
                        "transactionHash": HEX32_A,
                        "blockHash": HEX32_B,
                        "blockNumber": "0x1234",
                        "status": "0x1",
                    },
                    "block": {
                        "hash": HEX32_B,
                        "number": "0x1234",
                        "receiptsRoot": HEX32_C,
                    },
                    "beacon_finality": ethereum_beacon_finality(),
                    "receipt_proof": receipt_proof,
                }
            )
        )

    mutable_proof = bytearray(b"\x0a\x0b\x0c")
    assert asyncio.run(sdk.submit_inbound_to_iroha(mutable_proof)) == "submitted"
    mutable_proof[0] = 0x99
    assert submitted == [b"\x0a\x0b\x0c"]

    with pytest.raises(TypeError, match="proofBytes must not be empty"):
        asyncio.run(sdk.submit_inbound_to_iroha(b""))
    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(sdk.submit_inbound_to_iroha(b"\x00\x00"))
    with pytest.raises(TypeError, match="proofBytes must be at most"):
        asyncio.run(
            sdk.submit_inbound_to_iroha(
                b"\x01" * (SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1)
            )
        )
    assert submitted == [b"\x0a\x0b\x0c"]

    receipt_proof_evidence = asyncio.run(
        EthereumMainnetSccp().collect_inbound_evidence_from_receipt(
            {
                "receipt_proof": receipt_proof,
                "receipt_proof_hash": receipt_proof_hash,
            }
        )
    )
    assert receipt_proof_evidence["receipt_proof_hash"] == receipt_proof_hash
    assert receipt_proof_evidence["receipt_proof"]["source_domain"] == SCCP_DOMAIN_ETH
    with pytest.raises(ValueError, match="receiptProofHash must match receiptProof"):
        asyncio.run(
            EthereumMainnetSccp().collect_inbound_evidence_from_receipt(
                {
                    "receipt_proof": receipt_proof,
                    "receipt_proof_hash": HEX32_A,
                }
            )
        )

    source_receipt = {
        "transactionHash": HEX32_A,
        "blockHash": HEX32_B,
        "blockNumber": "0x1234",
        "status": "0x1",
        "logs": [
            {
                "address": "0x" + "00" * 20,
                "topics": ["0x" + "00" * 32],
                "data": "0x1234",
            },
            source_event_log(),
        ],
    }
    source_evidence = asyncio.run(
        EthereumMainnetSccp(
            source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
        ).collect_inbound_evidence_from_receipt({"receipt": source_receipt})
    )
    assert source_evidence["source_event_digest"] == SOURCE_EVENT_DIGEST
    assert source_evidence["source_bridge_emitter_address"] == SOURCE_BRIDGE_ADDRESS
    explicit_source_evidence = asyncio.run(
        EthereumMainnetSccp(
            source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
        ).collect_inbound_evidence_from_receipt(
            {"receipt": source_receipt, "sourceEventDigest": SOURCE_EVENT_DIGEST}
        )
    )
    assert explicit_source_evidence["source_event_digest"] == SOURCE_EVENT_DIGEST

    with pytest.raises(TypeError, match="sourceBridgeEmitterAddress is required"):
        asyncio.run(
            EthereumMainnetSccp().collect_inbound_evidence_from_receipt(
                {"receipt": source_receipt, "sourceEventDigest": SOURCE_EVENT_DIGEST}
            )
        )
    with pytest.raises(TypeError, match="expected SCCP source event"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address="0x" + "45" * 20
            ).collect_inbound_evidence_from_receipt({"receipt": source_receipt})
        )
    with pytest.raises(TypeError, match="expected SCCP source event"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        **source_receipt,
                        "logs": [
                            source_event_log(
                                topics=[HEX32_A, SOURCE_EVENT_DIGEST]
                            )
                        ],
                    }
                }
            )
        )
    with pytest.raises(TypeError, match="exactly one matching"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        **source_receipt,
                        "logs": [source_event_log(), source_event_log()],
                    }
                }
            )
        )
    with pytest.raises(TypeError, match="removed logs"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        **source_receipt,
                        "logs": [source_event_log(removed=True)],
                    }
                }
            )
        )
    with pytest.raises(TypeError, match=r"receipt\.logs\[0\] must be an object"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        **source_receipt,
                        "logs": ["not-a-log"],
                    }
                }
            )
        )
    missing_data_log = source_event_log()
    del missing_data_log["data"]
    with pytest.raises(TypeError, match=r"receipt\.logs\[0\]\.data is required"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        **source_receipt,
                        "logs": [missing_data_log],
                    }
                }
            )
        )
    with pytest.raises(TypeError, match=r"receipt\.logs transactionHash"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        **source_receipt,
                        "logs": [source_event_log(transactionHash=HEX32_D)],
                    }
                }
            )
        )
    with pytest.raises(TypeError, match=r"receipt\.logs blockHash"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        **source_receipt,
                        "logs": [source_event_log(blockHash=HEX32_D)],
                    }
                }
            )
        )
    with pytest.raises(TypeError, match=r"receipt\.logs blockNumber"):
        asyncio.run(
            EthereumMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        **source_receipt,
                        "logs": [source_event_log(blockNumber="0x1235")],
                    }
                }
            )
        )


def test_ethereum_mainnet_sccp_collects_immutable_evidence_snapshot_from_mutable_inputs() -> None:
    mutable_log_topics = [evm_sccp_source_event_topic(), SOURCE_EVENT_DIGEST]
    receipt_logs = [source_event_log(topics=mutable_log_topics)]
    receipt = {
        "transactionHash": HEX32_A,
        "blockHash": HEX32_B,
        "blockNumber": "0x1234",
        "status": "0x1",
        "logs": receipt_logs,
    }
    block_witness = {
        "branch": [HEX32_E],
        "bytes": bytearray(b"\xbb"),
    }
    block = {
        "hash": HEX32_B,
        "number": "0x1234",
        "receiptsRoot": HEX32_C,
        "mutableWitness": block_witness,
    }
    finality_branch = list(ETHEREUM_FINALITY_BRANCH)
    finality_witness = {
        "branch": finality_branch,
        "bytes": bytearray(b"\xcc"),
    }
    mutable_payload = bytearray(b"\xaa")

    class ConsensusProvider:
        async def collect_finality_evidence(
            self,
            evidence: Mapping[str, Any],
            _options: Mapping[str, Any],
        ) -> Mapping[str, Any]:
            assert evidence["mutable_payload"] == b"\xaa"
            assert evidence["receipt"]["logs"][0]["topics"][1] == SOURCE_EVENT_DIGEST
            assert evidence["block"]["mutableWitness"]["bytes"] == b"\xbb"
            with pytest.raises(TypeError, match="immutable"):
                evidence["mutable_payload"] = b"\x00"  # type: ignore[index]
            with pytest.raises(TypeError, match="immutable"):
                evidence["receipt"]["logs"].append({})  # type: ignore[attr-defined]
            with pytest.raises(TypeError, match="immutable"):
                evidence["block"]["mutableWitness"]["branch"].append(HEX32_A)  # type: ignore[attr-defined]

            receipt_logs.append(source_event_log())
            mutable_log_topics[1] = HEX32_A
            block_witness["branch"].append(HEX32_A)
            block_witness["bytes"][0] = 0x7C
            mutable_payload[0] = 0x7D
            return ethereum_beacon_finality(
                finalityBranch=finality_branch,
                mutableWitness=finality_witness,
            )

    evidence = asyncio.run(
        EthereumMainnetSccp(
            consensus_provider=ConsensusProvider(),
            source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS,
        ).collect_inbound_evidence_from_receipt(
            {
                "receipt": receipt,
                "block": block,
                "mutable_payload": mutable_payload,
            }
        )
    )

    finality_branch[0] = HEX32_A
    finality_witness["branch"].append(HEX32_A)
    finality_witness["bytes"][0] = 0x7E

    with pytest.raises(TypeError, match="immutable"):
        evidence["receipt"] = {}  # type: ignore[index]
    with pytest.raises(TypeError, match="immutable"):
        evidence["receipt"]["logs"].append({})  # type: ignore[attr-defined]
    with pytest.raises(TypeError, match="immutable"):
        evidence["beacon_finality"]["mutableWitness"]["branch"].append(HEX32_A)  # type: ignore[attr-defined]

    assert evidence["mutable_payload"] == b"\xaa"
    assert len(evidence["receipt"]["logs"]) == 1
    assert evidence["receipt"]["logs"][0]["topics"][1] == SOURCE_EVENT_DIGEST
    assert evidence["block"]["mutableWitness"]["branch"] == [HEX32_E]
    assert evidence["block"]["mutableWitness"]["bytes"] == b"\xbb"
    assert (
        evidence["beacon_finality"]["finality_branch"][0]
        == ETHEREUM_FINALITY_BRANCH[0]
    )
    assert evidence["beacon_finality"]["mutableWitness"]["branch"] == ETHEREUM_FINALITY_BRANCH
    assert evidence["beacon_finality"]["mutableWitness"]["bytes"] == b"\xcc"


def test_ethereum_mainnet_sccp_inbound_prover_receives_immutable_evidence_snapshot() -> None:
    mutable_log_topics = [evm_sccp_source_event_topic(), SOURCE_EVENT_DIGEST]
    receipt_logs = [source_event_log(topics=mutable_log_topics)]
    receipt = {
        "transactionHash": HEX32_A,
        "blockHash": HEX32_B,
        "blockNumber": "0x1234",
        "status": "0x1",
        "logs": receipt_logs,
    }
    block_witness = {
        "branch": [HEX32_E],
        "bytes": bytearray(b"\xbb"),
    }
    block = {
        "hash": HEX32_B,
        "number": "0x1234",
        "receiptsRoot": HEX32_C,
        "mutableWitness": block_witness,
    }
    finality_branch = list(ETHEREUM_FINALITY_BRANCH)
    finality_witness = {
        "branch": finality_branch,
        "bytes": bytearray(b"\xcc"),
    }
    beacon_finality = ethereum_beacon_finality(
        finalityBranch=finality_branch,
        mutableWitness=finality_witness,
    )
    mutable_receipt_proof_nodes = [EVM_RECEIPT_STATE_MPT_NODE_HEX]
    mutable_inclusion_branch = [HEX32_E]
    receipt_proof = {
        "source_domain": SCCP_DOMAIN_ETH,
        "source_event_digest": SOURCE_EVENT_DIGEST,
        "beacon_slot": "11",
        "execution_block_number": "4660",
        "execution_block_hash": HEX32_B,
        "execution_receipts_root": HEX32_C,
        "beacon_finalized_root": HEX32_D,
        "sync_committee_root": HEX32_E,
        "receipt_root_index": "0",
        "receipt_trie_proof_nodes": mutable_receipt_proof_nodes,
        "inclusion_branch": mutable_inclusion_branch,
    }
    mutable_payload = bytearray(b"\xaa")
    callback_evidence: Mapping[str, Any] | None = None

    async def prove_inbound(
        evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        nonlocal callback_evidence
        callback_evidence = evidence
        assert evidence["mutable_payload"] == b"\xaa"
        assert evidence["receipt"]["logs"][0]["topics"][1] == SOURCE_EVENT_DIGEST
        assert evidence["block"]["mutableWitness"]["bytes"] == b"\xbb"
        assert evidence["beacon_finality"]["mutableWitness"]["bytes"] == b"\xcc"
        assert (
            evidence["receipt_proof"]["receipt_trie_proof_nodes"][0]
            == EVM_RECEIPT_STATE_MPT_NODE_HEX
        )

        with pytest.raises(TypeError, match="immutable"):
            evidence["mutable_payload"] = b"\x00"  # type: ignore[index]
        with pytest.raises(TypeError, match="immutable"):
            evidence["receipt"]["logs"].append({})  # type: ignore[attr-defined]
        with pytest.raises(TypeError, match="immutable"):
            evidence["beacon_finality"]["finality_branch"].append(HEX32_A)  # type: ignore[attr-defined]
        with pytest.raises(TypeError, match="immutable"):
            evidence["receipt_proof"]["receipt_trie_proof_nodes"].append(HEX32_A)  # type: ignore[attr-defined]

        receipt_logs.append(source_event_log())
        mutable_log_topics[1] = HEX32_A
        block_witness["branch"].append(HEX32_A)
        block_witness["bytes"][0] = 0x7C
        finality_branch[0] = HEX32_A
        finality_witness["bytes"][0] = 0x7D
        mutable_receipt_proof_nodes[0] = "0x" + "99" * 32
        mutable_inclusion_branch.append(HEX32_A)
        mutable_payload[0] = 0x7E
        return b"\x01\x02\x03"

    proof = asyncio.run(
        EthereumMainnetSccp(
            source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS,
            prove_inbound=prove_inbound,
        ).prove_inbound_to_sora(
            {
                "receipt": receipt,
                "block": block,
                "beacon_finality": beacon_finality,
                "receipt_proof": receipt_proof,
                "mutable_payload": mutable_payload,
            }
        )
    )

    assert proof == b"\x01\x02\x03"
    assert callback_evidence is not None
    assert callback_evidence["mutable_payload"] == b"\xaa"
    assert len(callback_evidence["receipt"]["logs"]) == 1
    assert callback_evidence["receipt"]["logs"][0]["topics"][1] == SOURCE_EVENT_DIGEST
    assert callback_evidence["block"]["mutableWitness"]["branch"] == [HEX32_E]
    assert callback_evidence["block"]["mutableWitness"]["bytes"] == b"\xbb"
    assert (
        callback_evidence["beacon_finality"]["finality_branch"][0]
        == ETHEREUM_FINALITY_BRANCH[0]
    )
    assert callback_evidence["beacon_finality"]["mutableWitness"]["bytes"] == b"\xcc"
    assert (
        callback_evidence["receipt_proof"]["receipt_trie_proof_nodes"][0]
        == EVM_RECEIPT_STATE_MPT_NODE_HEX
    )
    assert callback_evidence["receipt_proof"]["inclusion_branch"] == [HEX32_E]


def test_ethereum_mainnet_sccp_facade_rejects_adversarial_inbound_evidence() -> None:
    class Provider:
        def __init__(
            self,
            receipt: Mapping[str, Any],
            block: Mapping[str, Any],
            *,
            chain_id: str = "0x1",
        ) -> None:
            self.receipt = receipt
            self.block = block
            self.chain_id = chain_id

        def request(self, method: str, _params: list[Any]) -> Any:
            if method == "eth_chainId":
                return self.chain_id
            if method == "eth_getTransactionReceipt":
                return self.receipt
            if method == "eth_getBlockByHash":
                return self.block
            raise AssertionError(f"unexpected method {method}")

    good_receipt = {
        "transactionHash": HEX32_A,
        "blockHash": HEX32_B,
        "blockNumber": "0x1234",
        "status": "0x1",
    }
    good_block = {"hash": HEX32_B, "number": "0x1234", "receiptsRoot": HEX32_C}
    good_finality = ethereum_beacon_finality()
    good_receipt_proof = {
        "source_domain": SCCP_DOMAIN_ETH,
        "source_event_digest": HEX32_D,
        "beacon_slot": "11",
        "execution_block_number": "4660",
        "execution_block_hash": HEX32_B,
        "execution_receipts_root": HEX32_C,
        "beacon_finalized_root": HEX32_D,
        "sync_committee_root": HEX32_E,
        "receipt_root_index": "0",
        "receipt_trie_proof_nodes": [EVM_RECEIPT_STATE_MPT_NODE_HEX],
        "inclusion_branch": [HEX32_E],
    }
    receipt_without_block_number = dict(good_receipt)
    del receipt_without_block_number["blockNumber"]
    block_without_number = dict(good_block)
    del block_without_number["number"]

    for chain_id in ("1", 1, "0x38", "0x01", "0X1"):
        with pytest.raises((TypeError, ValueError), match="eth_chainId|quantity"):
            asyncio.run(
                EthereumMainnetSccp(
                    execution_provider=Provider(
                        good_receipt, good_block, chain_id=chain_id
                    )
                ).collect_inbound_evidence_from_receipt({"transaction_hash": HEX32_A})
            )

    with pytest.raises(TypeError, match="execution provider is required"):
        asyncio.run(
            EthereumMainnetSccp().collect_inbound_evidence_from_receipt(
                {"transaction_hash": HEX32_A}
            )
        )

    with pytest.raises((TypeError, ValueError), match="transactionHash"):
        asyncio.run(
            EthereumMainnetSccp().collect_inbound_evidence_from_receipt(
                {
                    "transaction_hash": HEX32_A,
                    "receipt": {**good_receipt, "transactionHash": HEX32_D},
                    "block": good_block,
                    "beacon_finality": good_finality,
                }
            )
        )

    cases = (
        (
            {**good_receipt, "status": "0x0"},
            good_block,
            good_finality,
            "receipt status",
        ),
        (
            {**good_receipt, "transactionHash": HEX32_A.upper()},
            good_block,
            good_finality,
            "canonical lowercase",
        ),
        (receipt_without_block_number, good_block, good_finality, "receipt.blockNumber"),
        (
            {**good_receipt, "blockNumber": "0x0"},
            good_block,
            good_finality,
            "receipt.blockNumber",
        ),
        (
            good_receipt,
            {**good_block, "hash": HEX32_D},
            good_finality,
            "block.hash",
        ),
        (good_receipt, block_without_number, good_finality, "block.number"),
        (
            good_receipt,
            {**good_block, "number": "0x0"},
            good_finality,
            "block.number",
        ),
        (
            good_receipt,
            {**good_block, "number": "0x1235"},
            good_finality,
            "block.number",
        ),
        (
            good_receipt,
            {**good_block, "receiptsRoot": "0x" + "00" * 32},
            good_finality,
            "receiptsRoot",
        ),
        (
            good_receipt,
            good_block,
            {**good_finality, "executionBlockHash": HEX32_D},
            "beaconFinality.executionBlockHash",
        ),
        (
            good_receipt,
            good_block,
            {**good_finality, "executionBlockNumber": "0x1235"},
            "beaconFinality.executionBlockNumber",
        ),
        (
            good_receipt,
            good_block,
            {**good_finality, "executionReceiptsRoot": HEX32_D},
            "beaconFinality.executionReceiptsRoot",
        ),
        (
            good_receipt,
            good_block,
            {
                key: value
                for key, value in good_finality.items()
                if key != "finalityBranch"
            },
            "beaconFinality.finalityBranch",
        ),
        (
            good_receipt,
            good_block,
            {**good_finality, "finalityBranch": ETHEREUM_FINALITY_BRANCH[:5]},
            "finalityBranch must contain 6 siblings",
        ),
        (
            good_receipt,
            good_block,
            {**good_finality, "syncCommitteeBits": LOW_ETHEREUM_SYNC_COMMITTEE_BITS},
            "beaconFinality.syncCommitteeBits",
        ),
        (
            good_receipt,
            good_block,
            {**good_finality, "syncCommitteeParticipation": "341"},
            "beaconFinality.syncCommitteeParticipation",
        ),
        (
            good_receipt,
            good_block,
            {**good_finality, "syncSignatureSlot": "10"},
            "beaconFinality.syncSignatureSlot",
        ),
        (
            good_receipt,
            good_block,
            {**good_finality, "syncCommitteeSignature": "0x" + "00" * 96},
            "beaconFinality.syncCommitteeSignature",
        ),
        (
            good_receipt,
            good_block,
            {},
            "beaconFinality.executionBlockNumber",
        ),
    )

    for receipt, block, finality, expected_message in cases:
        with pytest.raises((TypeError, ValueError), match=expected_message):
            asyncio.run(
                EthereumMainnetSccp().collect_inbound_evidence_from_receipt(
                    {
                        "receipt": receipt,
                        "block": block,
                        "beacon_finality": finality,
                    }
                )
            )

    called = False

    async def prove_inbound(
        _evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        nonlocal called
        called = True
        return b"\x01\x02"

    with pytest.raises(TypeError, match="beaconFinality"):
        asyncio.run(
            EthereumMainnetSccp(prove_inbound=prove_inbound).prove_inbound_to_sora(
                {"receipt": good_receipt, "block": good_block}
            )
        )
    assert not called

    with pytest.raises(TypeError, match="receiptProof"):
        asyncio.run(
            EthereumMainnetSccp(prove_inbound=prove_inbound).prove_inbound_to_sora(
                {
                    "receipt": good_receipt,
                    "block": good_block,
                    "beacon_finality": good_finality,
                    "receipt_proof_hash": HEX32_A,
                }
            )
        )
    assert not called

    with pytest.raises(ValueError, match="receiptProof.executionReceiptsRoot"):
        asyncio.run(
            EthereumMainnetSccp(prove_inbound=prove_inbound).prove_inbound_to_sora(
                {
                    "receipt": good_receipt,
                    "block": good_block,
                    "beacon_finality": good_finality,
                    "receipt_proof": {
                        **good_receipt_proof,
                        "execution_receipts_root": HEX32_D,
                    },
                }
            )
        )
    assert not called

    with pytest.raises(ValueError, match="receiptProof.beaconFinalizedRoot"):
        asyncio.run(
            EthereumMainnetSccp(prove_inbound=prove_inbound).prove_inbound_to_sora(
                {
                    "receipt": good_receipt,
                    "block": good_block,
                    "beacon_finality": good_finality,
                    "receipt_proof": {
                        **good_receipt_proof,
                        "beacon_finalized_root": HEX32_A,
                    },
                }
            )
        )
    assert not called

    with pytest.raises(ValueError, match="receiptProof.syncCommitteeRoot"):
        asyncio.run(
            EthereumMainnetSccp(prove_inbound=prove_inbound).prove_inbound_to_sora(
                {
                    "receipt": good_receipt,
                    "block": good_block,
                    "beacon_finality": good_finality,
                    "receipt_proof": {
                        **good_receipt_proof,
                        "sync_committee_root": HEX32_A,
                    },
                }
            )
        )
    assert not called

    with pytest.raises(ValueError, match="receiptProof.beaconSlot"):
        asyncio.run(
            EthereumMainnetSccp(prove_inbound=prove_inbound).prove_inbound_to_sora(
                {
                    "receipt": good_receipt,
                    "block": good_block,
                    "beacon_finality": good_finality,
                    "receipt_proof": {
                        **good_receipt_proof,
                        "beacon_slot": "12",
                    },
                }
            )
        )
    assert not called

    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        asyncio.run(
            EthereumMainnetSccp().collect_inbound_evidence_from_receipt(
                {
                    "sourceDomain": SCCP_DOMAIN_ETH,
                    "source_domain": SCCP_DOMAIN_ETH,
                    "receipt_proof_hash": HEX32_A,
                }
            )
        )


def test_bsc_mainnet_sccp_facade_requires_chain_id_56_and_bsc_target() -> None:
    raw_binding = {
        "verifier_address": "0x" + "11" * 20,
        "bridge_address": "0x" + "22" * 20,
        "verifier_code_hash": "0x" + "bb" * 32,
        "verifier_key_hash": "0x" + "cc" * 32,
    }
    assert require_bsc_mainnet_chain_id(56) == 56
    binding = bsc_mainnet_sccp_destination_binding(raw_binding)

    assert SCCP_BSC_MAINNET_EVM_CHAIN_ID == 56
    assert binding["target_domain"] == SCCP_DOMAIN_BSC
    assert binding["network_id"] == SCCP_BSC_MAINNET_NETWORK_ID
    assert (
        bsc_mainnet_sccp_destination_binding_hash(
            {**raw_binding, "binding_hash": binding["binding_hash"]}
        )
        == binding["binding_hash"]
    )

    input_value = sample_evm_request_input(
        public_inputs=sample_evm_public_inputs(target_domain=SCCP_DOMAIN_BSC),
        destination_binding=binding,
        destination_binding_hash=None,
    )
    request = build_bsc_mainnet_sccp_destination_proof_request(input_value)
    proof_result = wrap_bsc_mainnet_sccp_destination_proof_result(
        GROTH16_PROOF_BYTES,
        request,
    )
    with pytest.raises(TypeError, match="canonical|destinationBinding"):
        wrap_bsc_mainnet_sccp_destination_proof_result(
            GROTH16_PROOF_BYTES,
            {**request, "destination_binding_hash": "0x" + "99" * 32},
        )
    submission = build_bsc_mainnet_sccp_destination_submission(
        {"proof_result": proof_result}
    )

    assert request["target_domain"] == SCCP_DOMAIN_BSC
    assert request["destination_binding"]["network_id"] == SCCP_BSC_MAINNET_NETWORK_ID
    assert proof_result["destination_binding_hash"] == binding["binding_hash"]
    assert submission["target_domain"] == SCCP_DOMAIN_BSC

    async def prove(
        callback_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        assert callback_request["target_domain"] == SCCP_DOMAIN_BSC
        assert (
            callback_request["destination_binding"]["network_id"]
            == SCCP_BSC_MAINNET_NETWORK_ID
        )
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    async_result = asyncio.run(BscMainnetSccpProver(prove=prove).prove(input_value))
    assert async_result["destination_binding_hash"] == binding["binding_hash"]

    async def zero_prove(
        _callback_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        return {"proof_bytes": bytes(len(GROTH16_PROOF_BYTES))}

    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(BscMainnetSccpProver(prove=zero_prove).prove(input_value))

    facade = BscMainnetSccp(prove=prove)
    facade_request = asyncio.run(facade.build_outbound_proof_request(input_value))
    facade_result = asyncio.run(facade.prove_outbound_to_bsc(input_value))
    facade_submission = facade.build_bsc_calldata({"proof_result": facade_result})
    assert facade.require_mainnet_chain_id(56) == 56
    assert facade.destination_binding(raw_binding)["binding_hash"] == binding["binding_hash"]
    assert facade.destination_binding_hash(raw_binding) == binding["binding_hash"]
    assert facade_request["target_domain"] == SCCP_DOMAIN_BSC
    assert facade_result["destination_binding_hash"] == binding["binding_hash"]
    assert facade_submission["target_domain"] == SCCP_DOMAIN_BSC

    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(BscMainnetSccp(prove=zero_prove).prove_outbound_to_bsc(input_value))

    async def submit_outbound(
        callback_submission: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> str:
        assert callback_submission["target_domain"] == SCCP_DOMAIN_BSC
        assert callback_submission["call_data_hex"] == submission["call_data_hex"]
        assert callback_submission["destination_binding_hash"] == binding["binding_hash"]
        return "bsc-submitted"

    submitted = asyncio.run(
        BscMainnetSccp(
            submit_outbound_to_bsc=submit_outbound
        ).submit_outbound_to_bsc({"proof_result": proof_result})
    )
    assert submitted == "bsc-submitted"
    with pytest.raises(EvmSccpProverUnavailableError, match="outbound submitter"):
        asyncio.run(
            BscMainnetSccp().submit_outbound_to_bsc(
                {"proof_result": proof_result}
            )
        )

    with pytest.raises(ValueError, match="chain id 56"):
        require_bsc_mainnet_chain_id(1)

    with pytest.raises(ValueError, match="chain id 56"):
        bsc_mainnet_sccp_destination_binding(
            {**raw_binding, "network_id": "0x" + "aa" * 32}
        )

    with pytest.raises((TypeError, ValueError), match="target BSC|BSC mainnet"):
        build_bsc_mainnet_sccp_destination_proof_request(
            sample_evm_production_request_input()
        )

    with pytest.raises((TypeError, ValueError), match="chain id 56|targetDomain"):
        build_bsc_mainnet_sccp_destination_submission(
            {
                "proof_result": {
                    **proof_result,
                    "destination_binding": sample_evm_destination_binding(),
                }
            }
        )


def test_bsc_mainnet_sccp_local_admission_submission_wraps_native_output() -> None:
    input_value = {
        "source_domain": SCCP_DOMAIN_BSC,
        "target_domain": SCCP_DOMAIN_SORA,
        "proof_bytes": bytearray(b"\x01\x02\x03"),
        "public_inputs_bytes": b"\x04\x05\x06",
        "bundle_bytes": b"\x07\x08\x09",
        "envelope_bytes": b"\x0a\x0b\x0c",
        "statement_hash": "0x" + "66" * 32,
        "source_verifier_material_hash": "0x" + "77" * 32,
        "source_adapter_engine_deployment_hash": "0x" + "88" * 32,
    }
    submission = build_bsc_mainnet_sccp_local_admission_submission(input_value)
    facade_submission = BscMainnetSccp().build_local_admission_submission(input_value)

    assert submission["platform_payload"] == SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1
    assert submission["envelope_encoding"] == SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1
    assert submission["verifier_entrypoint"] == SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1
    assert submission["source_domain"] == SCCP_DOMAIN_BSC
    assert submission["target_domain"] == SCCP_DOMAIN_SORA
    assert submission["arguments"] == []
    assert submission["proof_bytes"] == b"\x01\x02\x03"
    assert submission["public_inputs_bytes"] == b"\x04\x05\x06"
    assert submission["bundle_bytes"] == b"\x07\x08\x09"
    assert submission["envelope_bytes"] == b"\x0a\x0b\x0c"
    assert submission["local_admission"]["proof_bytes"] == b"\x01\x02\x03"
    assert facade_submission["envelope_hex"] == submission["envelope_hex"]

    input_value["proof_bytes"][0] = 0x99
    assert submission["proof_bytes"] == b"\x01\x02\x03"

    with pytest.raises(ValueError, match="BSC -> SORA"):
        build_bsc_mainnet_sccp_local_admission_submission(
            {**input_value, "source_domain": SCCP_DOMAIN_ETH}
        )
    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        build_bsc_mainnet_sccp_local_admission_submission(
            {**input_value, "proof_bytes": b"\x00\x00"}
        )
    with pytest.raises(TypeError, match="envelopeBytes must not be empty"):
        build_bsc_mainnet_sccp_local_admission_submission(
            {**input_value, "envelope_bytes": b""}
        )
    with pytest.raises(TypeError, match="metadata is not canonical"):
        build_bsc_mainnet_sccp_local_admission_submission(
            {**input_value, "envelope_encoding": "abi_tuple_v1"}
        )
    with pytest.raises(TypeError, match="metadata is not canonical"):
        build_bsc_mainnet_sccp_local_admission_submission(
            {**input_value, "proof_family": "debug-proof-family"}
        )


def test_bsc_mainnet_sccp_facade_collects_inbound_receipts_and_copies_proofs() -> None:
    class ExecutionProvider:
        def __init__(self) -> None:
            self.calls: list[tuple[str, list[Any]]] = []

        async def request(self, method: str, params: list[Any]) -> Any:
            self.calls.append((method, params))
            if method == "eth_chainId":
                return "0x38"
            if method == "eth_getTransactionReceipt":
                assert params == [HEX32_A]
                return {
                    "transactionHash": HEX32_A,
                    "blockHash": HEX32_B,
                    "blockNumber": "0x1234",
                    "status": "0x1",
                    "logs": [source_event_log()],
                }
            if method == "eth_getBlockByHash":
                assert params == [HEX32_B, False]
                return {
                    "hash": HEX32_B,
                    "number": "0x1234",
                    "receiptsRoot": HEX32_C,
                }
            raise AssertionError(f"unexpected method {method}")

    class ConsensusProvider:
        async def collect_finality_evidence(
            self,
            evidence: Mapping[str, Any],
            _options: Mapping[str, Any],
        ) -> Mapping[str, Any]:
            assert evidence["transaction_hash"] == HEX32_A
            assert evidence["receipt"]["blockHash"] == HEX32_B
            assert evidence["block"]["receiptsRoot"] == HEX32_C
            return {
                "execution_block_number": "0x1234",
                "execution_block_hash": HEX32_B,
                "execution_receipts_root": HEX32_C,
                "validator_epoch": "0x24",
                "commit_seal_hash": HEX32_D,
            }

    async def prove_inbound(
        evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        assert evidence["source_domain"] == SCCP_DOMAIN_BSC
        assert evidence["target_domain"] == SCCP_DOMAIN_SORA
        assert evidence["parlia_finality"]["commit_seal_hash"] == HEX32_D
        assert evidence["receipt_proof"]["block_hash"] == HEX32_B
        assert evidence["source_event_digest"] == SOURCE_EVENT_DIGEST
        assert evidence["source_bridge_emitter_address"] == SOURCE_BRIDGE_ADDRESS
        return b"\x01\x02\x03"

    submitted: list[bytes] = []

    async def submit_inbound(proof_bytes: bytes, _options: Mapping[str, Any]) -> str:
        submitted.append(proof_bytes)
        return "submitted"

    provider = ExecutionProvider()
    sdk = BscMainnetSccp(
        execution_provider=provider,
        consensus_provider=ConsensusProvider(),
        prove_inbound=prove_inbound,
        submit_inbound_to_iroha=submit_inbound,
        source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS,
    )

    evidence = asyncio.run(
        sdk.collect_inbound_evidence_from_receipt({"transaction_hash": HEX32_A})
    )
    assert evidence["source_domain"] == SCCP_DOMAIN_BSC
    assert evidence["target_domain"] == SCCP_DOMAIN_SORA
    assert evidence["transaction_hash"] == HEX32_A
    assert evidence["parlia_finality"]["execution_block_number"] == "4660"
    assert evidence["parlia_finality"]["execution_block_hash"] == HEX32_B
    assert evidence["parlia_finality"]["execution_receipts_root"] == HEX32_C
    assert evidence["parlia_finality"]["validator_epoch"] == "0x24"
    assert evidence["source_event_digest"] == SOURCE_EVENT_DIGEST
    assert evidence["source_bridge_emitter_address"] == SOURCE_BRIDGE_ADDRESS

    receipt_proof = {
        "source_domain": SCCP_DOMAIN_BSC,
        "source_event_digest": "0x" + "34" * 32,
        "validator_epoch": "36",
        "block_number": "4660",
        "block_hash": HEX32_B,
        "receipts_root": HEX32_C,
        "validator_set_hash": HEX32_E,
        "commit_seal_hash": HEX32_D,
        "receipt_root_index": "0",
        "receipt_trie_proof_nodes": [EVM_RECEIPT_STATE_MPT_NODE_HEX],
        "inclusion_branch": [HEX32_E],
    }
    receipt_proof_hash = bsc_sccp_receipt_proof_hash(receipt_proof)
    proof = asyncio.run(
        sdk.prove_inbound_to_sora(
            {"transaction_hash": HEX32_A, "receipt_proof": receipt_proof}
        )
    )
    assert proof == b"\x01\x02\x03"
    receipt_proof_evidence = asyncio.run(
        BscMainnetSccp().collect_inbound_evidence_from_receipt(
            {
                "receipt_proof": receipt_proof,
                "receipt_proof_hash": receipt_proof_hash,
                "parlia_finality": {
                    "execution_block_number": "0x1234",
                    "execution_block_hash": HEX32_B,
                    "execution_receipts_root": HEX32_C,
                },
            }
        )
    )
    assert receipt_proof_evidence["receipt_proof"]["block_hash"] == HEX32_B
    assert receipt_proof_evidence["receipt_proof_hash"] == receipt_proof_hash

    called_without_source_event = False

    async def prove_without_source_event(
        _evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        nonlocal called_without_source_event
        called_without_source_event = True
        return b"\x01"

    with pytest.raises(TypeError, match="requires receipt source event validation"):
        asyncio.run(
            BscMainnetSccp(prove_inbound=prove_without_source_event).prove_inbound_to_sora(
                {
                    "receipt_proof": receipt_proof,
                    "receipt_proof_hash": receipt_proof_hash,
                    "parlia_finality": {
                        "execution_block_number": "0x1234",
                        "execution_block_hash": HEX32_B,
                        "execution_receipts_root": HEX32_C,
                    },
                }
            )
        )
    assert called_without_source_event is False

    called_with_unvalidated_source_event = False

    async def prove_with_unvalidated_source_event(
        _evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        nonlocal called_with_unvalidated_source_event
        called_with_unvalidated_source_event = True
        return b"\x01"

    with pytest.raises(TypeError, match="source event validation requires receipt logs"):
        asyncio.run(
            BscMainnetSccp(
                prove_inbound=prove_with_unvalidated_source_event
            ).prove_inbound_to_sora(
                {
                    "receipt_proof": receipt_proof,
                    "receipt_proof_hash": receipt_proof_hash,
                    "source_event_digest": SOURCE_EVENT_DIGEST,
                    "parlia_finality": {
                        "execution_block_number": "0x1234",
                        "execution_block_hash": HEX32_B,
                        "execution_receipts_root": HEX32_C,
                    },
                }
            )
        )
    assert called_with_unvalidated_source_event is False

    with pytest.raises(ValueError, match="receiptProof.sourceEventDigest"):
        asyncio.run(
            BscMainnetSccp().collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        "transactionHash": HEX32_A,
                        "blockHash": HEX32_B,
                        "blockNumber": "0x1234",
                        "status": "0x1",
                        "logs": [
                            source_event_log(
                                topics=[evm_sccp_source_event_topic(), HEX32_D]
                            )
                        ],
                    },
                    "block": {
                        "hash": HEX32_B,
                        "number": "0x1234",
                        "receiptsRoot": HEX32_C,
                    },
                    "source_bridge_emitter_address": SOURCE_BRIDGE_ADDRESS,
                    "receipt_proof": receipt_proof,
                    "receipt_proof_hash": receipt_proof_hash,
                    "parlia_finality": {
                        "execution_block_number": "0x1234",
                        "execution_block_hash": HEX32_B,
                        "execution_receipts_root": HEX32_C,
                    },
                }
            )
        )

    malformed_source_log_cases = (
        (
            [source_event_log(topics=[evm_sccp_source_event_topic(), SOURCE_EVENT_DIGEST, HEX32_F])],
            "exactly 2 topics",
        ),
        ([source_event_log(data="0x01")], "data must be 0x"),
        (
            [source_event_log(topics=[evm_sccp_source_event_topic(), "0x" + "00" * 32])],
            "digest must not be zero",
        ),
        ([source_event_log(), source_event_log()], "exactly one matching"),
        ([source_event_log(removed=True)], "removed logs"),
    )
    for logs, message in malformed_source_log_cases:
        with pytest.raises((TypeError, ValueError), match=message):
            asyncio.run(
                BscMainnetSccp(
                    source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
                ).collect_inbound_evidence_from_receipt(
                    {
                        "receipt": {
                            "transactionHash": HEX32_A,
                            "blockHash": HEX32_B,
                            "blockNumber": "0x1234",
                            "status": "0x1",
                            "logs": logs,
                        },
                        "block": {
                            "hash": HEX32_B,
                            "number": "0x1234",
                            "receiptsRoot": HEX32_C,
                        },
                    }
                )
            )

    missing_transaction_hash_log = source_event_log()
    del missing_transaction_hash_log["transactionHash"]
    with pytest.raises(TypeError, match=r"receipt\.logs\[0\]\.transactionHash"):
        asyncio.run(
            BscMainnetSccp(
                source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS
            ).collect_inbound_evidence_from_receipt(
                {
                    "receipt": {
                        "transactionHash": HEX32_A,
                        "blockHash": HEX32_B,
                        "blockNumber": "0x1234",
                        "status": "0x1",
                        "logs": [missing_transaction_hash_log],
                    },
                    "block": {
                        "hash": HEX32_B,
                        "number": "0x1234",
                        "receiptsRoot": HEX32_C,
                    },
                }
            )
        )

    called_without_finality = False

    async def prove_without_finality(
        _evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        nonlocal called_without_finality
        called_without_finality = True
        return b"\x01"

    sdk_without_finality = BscMainnetSccp(prove_inbound=prove_without_finality)
    with pytest.raises(TypeError, match="requires parliaFinality"):
        asyncio.run(
            sdk_without_finality.prove_inbound_to_sora({"receipt_proof_hash": HEX32_A})
        )
    with pytest.raises(TypeError, match="requires parliaFinality"):
        asyncio.run(
            sdk_without_finality.prove_inbound_to_sora(
                {"receipt_proof_hash": HEX32_A, "parlia_finality": {}}
            )
        )
    assert called_without_finality is False

    called_with_hash_only = False

    async def prove_hash_only(
        _evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        nonlocal called_with_hash_only
        called_with_hash_only = True
        return b"\x01"

    with pytest.raises(TypeError, match="requires receiptProof"):
        asyncio.run(
            BscMainnetSccp(prove_inbound=prove_hash_only).prove_inbound_to_sora(
                {
                    "receipt_proof_hash": HEX32_A,
                    "parlia_finality": {
                        "execution_block_number": "0x1234",
                        "execution_block_hash": HEX32_B,
                        "execution_receipts_root": HEX32_C,
                    },
                }
            )
        )
    assert called_with_hash_only is False

    mutable_proof = bytearray(b"\x04\x05\x06")
    assert asyncio.run(sdk.submit_inbound_to_iroha(mutable_proof)) == "submitted"
    mutable_proof[0] = 0x99
    assert submitted == [b"\x04\x05\x06"]

    with pytest.raises(TypeError, match="proofBytes must not be empty"):
        asyncio.run(sdk.submit_inbound_to_iroha(b""))
    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(sdk.submit_inbound_to_iroha(b"\x00\x00"))


def test_bsc_mainnet_sccp_collects_immutable_evidence_snapshot_from_mutable_inputs() -> None:
    mutable_log_topics = [evm_sccp_source_event_topic(), SOURCE_EVENT_DIGEST]
    receipt_logs = [source_event_log(topics=mutable_log_topics)]
    receipt = {
        "transactionHash": HEX32_A,
        "blockHash": HEX32_B,
        "blockNumber": "0x1234",
        "status": "0x1",
        "logs": receipt_logs,
    }
    block_witness = {
        "branch": [HEX32_E],
        "bytes": bytearray(b"\xbb"),
    }
    block = {
        "hash": HEX32_B,
        "number": "0x1234",
        "receiptsRoot": HEX32_C,
        "mutableWitness": block_witness,
    }
    finality_witness = {
        "branch": [HEX32_E],
        "bytes": bytearray(b"\xcc"),
    }
    mutable_payload = bytearray(b"\xaa")

    class ConsensusProvider:
        async def collect_finality_evidence(
            self,
            evidence: Mapping[str, Any],
            _options: Mapping[str, Any],
        ) -> Mapping[str, Any]:
            assert evidence["mutable_payload"] == b"\xaa"
            assert evidence["receipt"]["logs"][0]["topics"][1] == SOURCE_EVENT_DIGEST
            assert evidence["block"]["mutableWitness"]["bytes"] == b"\xbb"
            with pytest.raises(TypeError, match="immutable"):
                evidence["mutable_payload"] = b"\x00"  # type: ignore[index]
            with pytest.raises(TypeError, match="immutable"):
                evidence["receipt"]["logs"].append({})  # type: ignore[attr-defined]
            with pytest.raises(TypeError, match="immutable"):
                evidence["block"]["mutableWitness"]["branch"].append(HEX32_A)  # type: ignore[attr-defined]

            receipt_logs.append(source_event_log())
            mutable_log_topics[1] = HEX32_A
            block_witness["branch"].append(HEX32_A)
            block_witness["bytes"][0] = 0x7C
            mutable_payload[0] = 0x7D
            return {
                "execution_block_number": "0x1234",
                "execution_block_hash": HEX32_B,
                "execution_receipts_root": HEX32_C,
                "validator_epoch": "0x24",
                "commit_seal_hash": HEX32_D,
                "mutableWitness": finality_witness,
            }

    evidence = asyncio.run(
        BscMainnetSccp(
            consensus_provider=ConsensusProvider(),
            source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS,
        ).collect_inbound_evidence_from_receipt(
            {
                "receipt": receipt,
                "block": block,
                "mutable_payload": mutable_payload,
            }
        )
    )

    finality_witness["branch"].append(HEX32_A)
    finality_witness["bytes"][0] = 0x7E

    with pytest.raises(TypeError, match="immutable"):
        evidence["receipt"] = {}  # type: ignore[index]
    with pytest.raises(TypeError, match="immutable"):
        evidence["receipt"]["logs"].append({})  # type: ignore[attr-defined]
    with pytest.raises(TypeError, match="immutable"):
        evidence["parlia_finality"]["mutableWitness"]["branch"].append(HEX32_A)  # type: ignore[attr-defined]

    assert evidence["mutable_payload"] == b"\xaa"
    assert len(evidence["receipt"]["logs"]) == 1
    assert evidence["receipt"]["logs"][0]["topics"][1] == SOURCE_EVENT_DIGEST
    assert evidence["block"]["mutableWitness"]["branch"] == [HEX32_E]
    assert evidence["block"]["mutableWitness"]["bytes"] == b"\xbb"
    assert evidence["parlia_finality"]["mutableWitness"]["branch"] == [HEX32_E]
    assert evidence["parlia_finality"]["mutableWitness"]["bytes"] == b"\xcc"


def test_bsc_mainnet_sccp_inbound_prover_receives_immutable_evidence_snapshot() -> None:
    mutable_log_topics = [evm_sccp_source_event_topic(), SOURCE_EVENT_DIGEST]
    receipt_logs = [source_event_log(topics=mutable_log_topics)]
    receipt = {
        "transactionHash": HEX32_A,
        "blockHash": HEX32_B,
        "blockNumber": "0x1234",
        "status": "0x1",
        "logs": receipt_logs,
    }
    block_witness = {
        "branch": [HEX32_E],
        "bytes": bytearray(b"\xbb"),
    }
    block = {
        "hash": HEX32_B,
        "number": "0x1234",
        "receiptsRoot": HEX32_C,
        "mutableWitness": block_witness,
    }
    finality_witness = {
        "branch": [HEX32_E],
        "bytes": bytearray(b"\xcc"),
    }
    parlia_finality = {
        "execution_block_number": "0x1234",
        "execution_block_hash": HEX32_B,
        "execution_receipts_root": HEX32_C,
        "validator_epoch": "0x24",
        "validator_set_hash": HEX32_E,
        "commit_seal_hash": HEX32_D,
        "mutableWitness": finality_witness,
    }
    mutable_receipt_proof_nodes = [EVM_RECEIPT_STATE_MPT_NODE_HEX]
    mutable_inclusion_branch = [HEX32_E]
    receipt_proof = {
        "source_domain": SCCP_DOMAIN_BSC,
        "source_event_digest": SOURCE_EVENT_DIGEST,
        "validator_epoch": "36",
        "block_number": "4660",
        "block_hash": HEX32_B,
        "receipts_root": HEX32_C,
        "validator_set_hash": HEX32_E,
        "commit_seal_hash": HEX32_D,
        "receipt_root_index": "0",
        "receipt_trie_proof_nodes": mutable_receipt_proof_nodes,
        "inclusion_branch": mutable_inclusion_branch,
    }
    mutable_payload = bytearray(b"\xaa")
    callback_evidence: Mapping[str, Any] | None = None

    async def prove_inbound(
        evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        nonlocal callback_evidence
        callback_evidence = evidence
        assert evidence["mutable_payload"] == b"\xaa"
        assert evidence["receipt"]["logs"][0]["topics"][1] == SOURCE_EVENT_DIGEST
        assert evidence["block"]["mutableWitness"]["bytes"] == b"\xbb"
        assert evidence["parlia_finality"]["mutableWitness"]["bytes"] == b"\xcc"
        assert (
            evidence["receipt_proof"]["receipt_trie_proof_nodes"][0]
            == EVM_RECEIPT_STATE_MPT_NODE_HEX
        )

        with pytest.raises(TypeError, match="immutable"):
            evidence["mutable_payload"] = b"\x00"  # type: ignore[index]
        with pytest.raises(TypeError, match="immutable"):
            evidence["receipt"]["logs"].append({})  # type: ignore[attr-defined]
        with pytest.raises(TypeError, match="immutable"):
            evidence["parlia_finality"]["mutableWitness"]["branch"].append(HEX32_A)  # type: ignore[attr-defined]
        with pytest.raises(TypeError, match="immutable"):
            evidence["receipt_proof"]["receipt_trie_proof_nodes"].append(HEX32_A)  # type: ignore[attr-defined]

        receipt_logs.append(source_event_log())
        mutable_log_topics[1] = HEX32_A
        block_witness["branch"].append(HEX32_A)
        block_witness["bytes"][0] = 0x7C
        finality_witness["branch"].append(HEX32_A)
        finality_witness["bytes"][0] = 0x7D
        mutable_receipt_proof_nodes[0] = "0x" + "99" * 32
        mutable_inclusion_branch.append(HEX32_A)
        mutable_payload[0] = 0x7E
        return b"\x04\x05\x06"

    proof = asyncio.run(
        BscMainnetSccp(
            source_bridge_emitter_address=SOURCE_BRIDGE_ADDRESS,
            prove_inbound=prove_inbound,
        ).prove_inbound_to_sora(
            {
                "receipt": receipt,
                "block": block,
                "parlia_finality": parlia_finality,
                "receipt_proof": receipt_proof,
                "mutable_payload": mutable_payload,
            }
        )
    )

    assert proof == b"\x04\x05\x06"
    assert callback_evidence is not None
    assert callback_evidence["mutable_payload"] == b"\xaa"
    assert len(callback_evidence["receipt"]["logs"]) == 1
    assert callback_evidence["receipt"]["logs"][0]["topics"][1] == SOURCE_EVENT_DIGEST
    assert callback_evidence["block"]["mutableWitness"]["branch"] == [HEX32_E]
    assert callback_evidence["block"]["mutableWitness"]["bytes"] == b"\xbb"
    assert callback_evidence["parlia_finality"]["mutableWitness"]["branch"] == [HEX32_E]
    assert callback_evidence["parlia_finality"]["mutableWitness"]["bytes"] == b"\xcc"
    assert (
        callback_evidence["receipt_proof"]["receipt_trie_proof_nodes"][0]
        == EVM_RECEIPT_STATE_MPT_NODE_HEX
    )
    assert callback_evidence["receipt_proof"]["inclusion_branch"] == [HEX32_E]


def test_bsc_mainnet_sccp_facade_rejects_adversarial_inbound_evidence() -> None:
    class Provider:
        def __init__(
            self,
            receipt: Mapping[str, Any],
            block: Mapping[str, Any],
            *,
            chain_id: str = "0x38",
        ) -> None:
            self.receipt = receipt
            self.block = block
            self.chain_id = chain_id

        def request(self, method: str, _params: list[Any]) -> Any:
            if method == "eth_chainId":
                return self.chain_id
            if method == "eth_getTransactionReceipt":
                return self.receipt
            if method == "eth_getBlockByHash":
                return self.block
            raise AssertionError(f"unexpected method {method}")

    good_receipt = {
        "transactionHash": HEX32_A,
        "blockHash": HEX32_B,
        "blockNumber": "0x1234",
        "status": "0x1",
    }
    good_block = {"hash": HEX32_B, "number": "0x1234", "receiptsRoot": HEX32_C}
    receipt_without_block_number = dict(good_receipt)
    del receipt_without_block_number["blockNumber"]
    block_without_number = dict(good_block)
    del block_without_number["number"]

    for chain_id in ("56", 56, "0x1", "0x038", "0X38"):
        with pytest.raises((TypeError, ValueError), match="chain id 56|quantity"):
            asyncio.run(
                BscMainnetSccp(
                    execution_provider=Provider(good_receipt, good_block, chain_id=chain_id)
                ).collect_inbound_evidence_from_receipt({"transaction_hash": HEX32_A})
            )

    cases = (
        (
            {**good_receipt, "status": "0x0"},
            good_block,
            "receipt status",
        ),
        (
            {**good_receipt, "transactionHash": HEX32_D},
            good_block,
            "transactionHash",
        ),
        (
            {**good_receipt, "transactionHash": HEX32_A.upper()},
            good_block,
            "canonical lowercase",
        ),
        (
            receipt_without_block_number,
            good_block,
            "receipt.blockNumber",
        ),
        (
            {**good_receipt, "blockNumber": "0x0"},
            good_block,
            "receipt.blockNumber",
        ),
        (
            good_receipt,
            {**good_block, "hash": HEX32_D},
            "block.hash",
        ),
        (
            good_receipt,
            block_without_number,
            "block.number",
        ),
        (
            good_receipt,
            {**good_block, "number": "0x0"},
            "block.number",
        ),
        (
            good_receipt,
            {**good_block, "number": "0x1235"},
            "block.number",
        ),
        (
            good_receipt,
            {**good_block, "receiptsRoot": "0x" + "00" * 32},
            "receiptsRoot",
        ),
    )

    for receipt, block, expected_message in cases:
        with pytest.raises((TypeError, ValueError), match=expected_message):
            asyncio.run(
                BscMainnetSccp(
                    execution_provider=Provider(receipt, block)
                ).collect_inbound_evidence_from_receipt({"transaction_hash": HEX32_A})
            )

    with pytest.raises(TypeError, match="sourceDomain must not use multiple aliases"):
        asyncio.run(
            BscMainnetSccp().collect_inbound_evidence_from_receipt(
                {
                    "sourceDomain": SCCP_DOMAIN_BSC,
                    "source_domain": SCCP_DOMAIN_BSC,
                    "receipt_proof_hash": HEX32_A,
                }
            )
        )

    async def prove_inbound(
        _evidence: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> bytes:
        raise AssertionError("prover callback must not run without Parlia finality")

    with pytest.raises(TypeError, match="requires parliaFinality"):
        asyncio.run(
            BscMainnetSccp(prove_inbound=prove_inbound).prove_inbound_to_sora(
                {"source_domain": SCCP_DOMAIN_BSC, "target_domain": SCCP_DOMAIN_SORA, "receipt_proof_hash": HEX32_A}
            )
        )

    good_finality = {
        "execution_block_number": "0x1234",
        "execution_block_hash": HEX32_B,
        "execution_receipts_root": HEX32_C,
        "validator_epoch": "0x24",
        "commit_seal_hash": HEX32_D,
    }
    finality_drift_cases = (
        (
            {**good_finality, "execution_block_hash": HEX32_D},
            "parliaFinality.executionBlockHash",
        ),
        (
            {**good_finality, "execution_block_number": "0x1235"},
            "parliaFinality.executionBlockNumber",
        ),
        (
            {**good_finality, "execution_receipts_root": HEX32_D},
            "parliaFinality.executionReceiptsRoot",
        ),
    )
    for finality, expected_message in finality_drift_cases:
        with pytest.raises((TypeError, ValueError), match=expected_message):
            asyncio.run(
                BscMainnetSccp().collect_inbound_evidence_from_receipt(
                    {
                        "receipt": good_receipt,
                        "block": good_block,
                        "parlia_finality": finality,
                    }
                )
            )


def test_substrate_sccp_prover_requires_linked_engine() -> None:
    prover = SubstrateSccpProver()

    with pytest.raises(SubstrateSccpProverUnavailableError) as exc:
        asyncio.run(prover.prove(sample_substrate_request_input()))

    assert exc.value.code == "ERR_SCCP_SUBSTRATE_PROVER_UNAVAILABLE"


def test_substrate_sccp_prover_wraps_externally_generated_proof_bytes() -> None:
    callback_request = None

    async def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> Dict[str, Any]:
        nonlocal callback_request
        callback_request = request
        assert request["backend"] == SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1
        assert isinstance(request["bundle_bytes"], bytes)
        assert isinstance(request["source_proof_bytes"], bytes)
        assert request["bundle_bytes"] == bytes([5, 6, 7])
        assert request["source_proof_bytes"] == bytes([9, 10])
        assert request["proof_context"]["statement_hash"] == "0x" + "55" * 32
        assert request["target_domain"] == SCCP_DOMAIN_SORA2
        with pytest.raises(TypeError, match="immutable"):
            request["bundle_bytes"] = b"\x00"
        with pytest.raises(TypeError, match="immutable"):
            request["source_proof_bytes"] = b"\x00"
        return {"proof_bytes": [1, 2, 3, 4]}

    bundle_bytes = bytearray([5, 6, 7])
    source_proof_bytes = bytearray([9, 10])
    input_value = sample_substrate_request_input(
        bundle_bytes=bundle_bytes,
        source_proof_bytes=source_proof_bytes,
    )
    result = asyncio.run(SubstrateSccpProver(prove=prove).prove(input_value))
    request = build_substrate_sccp_proof_request(input_value)
    direct_result = wrap_substrate_sccp_proof_result([1, 2, 3, 4], request)

    assert callback_request is not request
    assert callback_request == request
    async def prove_with_request_hash_aliases(
        linked_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": [1, 2, 3, 4],
            "request_hash": linked_request["request_hash"],
            "requestHash": linked_request["request_hash"],
        }

    with pytest.raises(TypeError, match=r"proofResult\.requestHash.*multiple aliases"):
        asyncio.run(
            SubstrateSccpProver(prove=prove_with_request_hash_aliases).prove(input_value)
        )

    async def prove_with_envelope_hash_aliases(
        linked_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        wrapped = wrap_substrate_sccp_proof_result([1, 2, 3, 4], linked_request)
        return {
            "proof_bytes": [1, 2, 3, 4],
            "envelope_hash": wrapped["envelope_hash"],
            "envelopeHash": wrapped["envelope_hash"],
        }

    with pytest.raises(TypeError, match=r"proofResult\.envelopeHash.*multiple aliases"):
        asyncio.run(
            SubstrateSccpProver(prove=prove_with_envelope_hash_aliases).prove(input_value)
        )

    assert result["proof_bytes"] == bytes([1, 2, 3, 4])
    assert result["proof_base64"] == "AQIDBA=="
    assert result["request_hash"] == request["request_hash"]
    assert direct_result["envelope_hash"] == result["envelope_hash"]
    assert result["bundle_bytes"] == bytes([5, 6, 7])
    assert result["source_proof_bytes"] == bytes([9, 10])
    assert result["statement_hash"] == "0x" + "55" * 32
    assert result["destination_binding_hash"] == "0x" + "66" * 32
    assert len(result["envelope_hash"]) == 66


def test_tron_sccp_prover_requires_linked_engine() -> None:
    prover = TronSccpProver()

    with pytest.raises(TronSccpProverUnavailableError) as exc:
        asyncio.run(prover.prove(sample_tron_request_input()))

    assert exc.value.code == "ERR_SCCP_TRON_PROVER_UNAVAILABLE"


def test_tron_sccp_prover_wraps_externally_generated_proof_bytes() -> None:
    callback_request = None

    async def prove(request: Mapping[str, Any], _options: Mapping[str, Any]) -> Dict[str, Any]:
        nonlocal callback_request
        callback_request = request
        assert request["backend"] == SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1
        assert isinstance(request["bundle_bytes"], bytes)
        assert isinstance(request["source_proof_bytes"], bytes)
        assert request["bundle_bytes"] == bytes([5, 6, 7])
        assert request["source_proof_bytes"] == bytes([9, 10])
        assert request["proof_context"]["statement_hash"] == "0x" + "55" * 32
        assert len(request["public_signal_words"]) == 9
        with pytest.raises(TypeError, match="immutable"):
            request["request_hash"] = HEX32_A
        with pytest.raises(TypeError, match="immutable"):
            request["source_proof_bytes"] = b"\x00"
        with pytest.raises(TypeError, match="immutable"):
            request["public_signal_words"].append(HEX32_B)
        with pytest.raises(TypeError, match="immutable"):
            request["proof_context"]["statement_hash"] = HEX32_A
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    bundle_bytes = bytearray([5, 6, 7])
    source_proof_bytes = bytearray([9, 10])
    input_value = sample_tron_production_request_input(
        bundle_bytes=bundle_bytes,
        source_proof_bytes=source_proof_bytes,
    )
    result = asyncio.run(TronSccpProver(prove=prove).prove(input_value))
    request = build_tron_sccp_proof_request(input_value)
    direct_result = wrap_tron_sccp_proof_result(GROTH16_PROOF_BYTES, request)

    assert callback_request is not request
    assert callback_request == request
    async def prove_with_envelope_hash_aliases(
        linked_request: Mapping[str, Any],
        _options: Mapping[str, Any],
    ) -> Dict[str, Any]:
        wrapped = wrap_tron_sccp_proof_result(GROTH16_PROOF_BYTES, linked_request)
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "envelope_hash": wrapped["envelope_hash"],
            "envelopeHash": wrapped["envelope_hash"],
        }

    with pytest.raises(TypeError, match=r"proofResult\.envelopeHash.*multiple aliases"):
        asyncio.run(TronSccpProver(prove=prove_with_envelope_hash_aliases).prove(input_value))

    assert result["proof_bytes"] == GROTH16_PROOF_BYTES
    assert len(result["proof_base64"]) > 0
    assert result["request_hash"] == request["request_hash"]
    assert direct_result["envelope_hash"] == result["envelope_hash"]
    assert result["public_signal_words"] == request["public_signal_words"]
    assert request["bundle_bytes"] == bytes([5, 6, 7])
    assert result["bundle_bytes"] == bytes([5, 6, 7])
    assert result["source_proof_bytes"] == bytes([9, 10])
    assert result["statement_hash"] == "0x" + "55" * 32
    assert result["destination_binding_hash"] == input_value["destination_binding"]["binding_hash"]
    assert result["destination_binding"] == input_value["destination_binding"]
    assert len(result["envelope_hash"]) == 66
    with pytest.raises(TypeError, match="immutable"):
        result["proof_bytes"] = bytes([0])
    with pytest.raises(TypeError, match="immutable"):
        result["proof_context"]["statement_hash"] = HEX32_A


def test_builds_evm_and_tron_groth16_contract_call_submissions() -> None:
    assert SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1 == (
        "0x" + _keccak_256(SCCP_SUBMIT_MESSAGE_PROOF_ABI_V1.encode("utf-8"))[:4].hex()
    )

    evm_request = build_evm_sccp_proof_request(sample_evm_production_request_input())
    evm_result = wrap_evm_sccp_proof_result(GROTH16_PROOF_BYTES, evm_request)
    evm_submission = build_evm_sccp_submission({"proof_result": evm_result})
    evm_words = sccp_message_transparent_public_input_abi_words(sample_evm_public_inputs())

    assert evm_submission["version"] == 1
    assert evm_submission["submission_kind"] == "contract_call"
    assert evm_submission["platform_payload"] == "evm_groth16_contract_call"
    assert evm_submission["envelope_encoding"] == SCCP_EVM_CONTRACT_CALL_ABI_TUPLE_V1
    assert evm_submission["verifier_backend"] == SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
    assert evm_submission["function_selector"] == SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1
    assert evm_submission["contract_method"] == SCCP_SUBMIT_MESSAGE_PROOF_ABI_V1
    assert evm_submission["source_domain"] == SCCP_DOMAIN_SORA
    assert evm_submission["target_domain"] == SCCP_DOMAIN_ETH
    assert evm_submission["public_inputs"] == sample_evm_public_inputs()
    assert evm_submission["public_input_words"] == evm_words
    assert evm_submission["public_signal_words"] == evm_result["public_signal_words"]
    assert evm_submission["statement_hash"] == evm_result["statement_hash"]
    assert evm_submission["destination_binding_hash"] == evm_result["destination_binding_hash"]
    assert evm_submission["proof_bytes"] == GROTH16_PROOF_BYTES
    assert evm_submission["envelope_bytes"] == evm_submission["call_data"]
    assert evm_submission["envelope_hex"] == evm_submission["call_data_hex"]
    assert evm_submission["call_data_hex"].startswith(SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1)
    assert len(evm_submission["call_data"]) == 676
    assert evm_submission["call_data"][4:36].hex() == ("00" * 30 + "0100")
    assert evm_submission["call_data"][260:292].hex() == ("00" * 30 + "0180")
    assert evm_submission["call_data"][292:] == GROTH16_PROOF_BYTES
    assert evm_submission["call_data"] == sccp_submit_message_proof_call_data(
        GROTH16_PROOF_BYTES,
        sample_evm_public_inputs(),
        evm_result["statement_hash"],
    )
    with pytest.raises(TypeError, match="proofResult must not use multiple aliases"):
        build_evm_sccp_submission(
            {
                "proof_result": evm_result,
                "proofResult": evm_result,
            }
        )
    with pytest.raises(
        TypeError,
        match="proofResult must be a wrapped Groth16 SCCP proof result",
    ):
        build_evm_sccp_submission(
            {
                "proof_result": None,
                "proof_bytes": GROTH16_PROOF_BYTES,
                "public_inputs": sample_evm_public_inputs(),
                "statement_hash": HEX32_G,
                "destination_binding_hash": HEX32_H,
            }
        )
    with pytest.raises(TypeError, match=r"proofResult\.requestHash.*multiple aliases"):
        build_evm_sccp_submission(
            {
                "proof_result": {
                    **dict(evm_result),
                    "requestHash": evm_result["request_hash"],
                },
            }
        )
    with pytest.raises(TypeError, match=r"proofResult\.envelopeHash.*multiple aliases"):
        build_evm_sccp_submission(
            {
                "proof_result": {
                    **dict(evm_result),
                    "envelopeHash": evm_result["envelope_hash"],
                },
            }
        )
    with pytest.raises(TypeError, match="bundleBytes must not use multiple aliases"):
        build_evm_sccp_submission(
            {
                "proof_result": evm_result,
                "bundle_bytes": bytes([5, 6, 7]),
                "bundleBytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.proofContext\.statementHash.*multiple aliases",
    ):
        build_evm_sccp_submission(
            {
                "proof_result": {
                    **dict(evm_result),
                    "proof_context": {
                        **dict(evm_result["proof_context"]),
                        "statementHash": evm_result["statement_hash"],
                    },
                },
            }
        )
    omitted_evm_result = wrap_evm_sccp_proof_result(
        GROTH16_PROOF_BYTES,
        build_evm_sccp_proof_request(
                sample_evm_production_request_input(source_proof_bytes=b"")
        ),
    )
    omitted_evm_submission = build_evm_sccp_submission(
        {"proof_result": omitted_evm_result}
    )
    assert omitted_evm_result["source_proof_bytes"] == b""
    assert omitted_evm_submission["proof_bytes"] == GROTH16_PROOF_BYTES
    assert evm_submission["public_input_words_bytes"] == bytes.fromhex(
        "".join(word.removeprefix("0x") for word in evm_words)
    )
    assert evm_submission["arguments"][0]["key"] == "proof_bytes"
    assert evm_submission["arguments"][1]["encoding"] == "abi_bytes32x6"
    assert evm_submission["arguments"][2]["bytes"] == evm_result["statement_hash"]
    with pytest.raises(TypeError, match="immutable"):
        evm_submission["arguments"].append({})

    bsc_submission = build_evm_sccp_submission(
        {
            "proof_result": wrap_evm_sccp_proof_result(
                GROTH16_PROOF_BYTES,
                build_evm_sccp_proof_request(
                    sample_evm_production_request_input(
                        public_inputs=sample_evm_public_inputs(
                            target_domain=SCCP_DOMAIN_BSC
                        ),
                        destination_binding=sample_evm_destination_binding(
                            target_domain=SCCP_DOMAIN_BSC
                        ),
                        destination_binding_hash=None,
                    )
                ),
            )
        }
    )
    assert bsc_submission["target_domain"] == SCCP_DOMAIN_BSC
    assert bsc_submission["public_input_words"][2] != evm_submission["public_input_words"][2]

    with pytest.raises(TypeError, match="proofResult.backend"):
        build_evm_sccp_submission(
            {"proof_result": {**dict(evm_result), "backend": "debug-evm-backend"}}
        )
    with pytest.raises(TypeError, match="proofBytes must match"):
        build_evm_sccp_submission(
            {
                "proof_result": evm_result,
                "proof_bytes": groth16_proof_bytes(
                    words={
                        11: abi_word(
                            int(
                                "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd45",
                                16,
                            )
                        )
                    }
                ),
            }
        )
    with pytest.raises(TypeError, match="publicInputs must be an object"):
        build_evm_sccp_submission({"proof_result": evm_result, "public_inputs": None})
    with pytest.raises(TypeError, match="proofBytes must be bytes"):
        build_evm_sccp_submission({"proof_result": evm_result, "proof_bytes": None})
    with pytest.raises(TypeError, match="statementHash"):
        build_evm_sccp_submission({"proof_result": evm_result, "statement_hash": None})
    with pytest.raises(TypeError, match="destinationBindingHash"):
        build_evm_sccp_submission(
            {"proof_result": evm_result, "destination_binding_hash": None}
        )
    with pytest.raises(TypeError, match="publicSignalWords must contain 9 words"):
        build_evm_sccp_submission(
            {"proof_result": evm_result, "public_signal_words": None}
        )
    with pytest.raises(TypeError, match="bundleBytes must be bytes"):
        build_evm_sccp_submission({"proof_result": evm_result, "bundle_bytes": None})
    wrong_signals = list(evm_result["public_signal_words"])
    wrong_signals[0] = HEX32_A
    with pytest.raises(TypeError, match="publicSignalWords must match"):
        build_evm_sccp_submission(
            {"proof_result": evm_result, "public_signal_words": wrong_signals}
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.envelopeHash must match wrapped proof bytes",
    ):
        build_evm_sccp_submission(
            {"proof_result": {**dict(evm_result), "envelope_hash": HEX32_A}}
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.proofBase64 must match proofResult\.proofBytes",
    ):
        build_evm_sccp_submission(
            {"proof_result": {**dict(evm_result), "proof_base64": "AAAA"}}
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.requestHash must match bundleBytes and sourceProofBytes",
    ):
        build_evm_sccp_submission(
            {"proof_result": {**dict(evm_result), "bundle_bytes": bytes([5, 6, 8])}}
        )
    wrong_evm_context = dict(evm_result["proof_context"])
    wrong_evm_context["statement_hash"] = HEX32_A
    with pytest.raises(TypeError, match="proofResult.proofContext"):
        build_evm_sccp_submission(
            {"proof_result": {**dict(evm_result), "proof_context": wrong_evm_context}}
        )
    with pytest.raises(ValueError, match="ETH or BSC"):
        build_evm_sccp_submission(
            {
                "proof_bytes": GROTH16_PROOF_BYTES,
                "public_inputs": sample_evm_public_inputs(target_domain=SCCP_DOMAIN_TON),
                "statement_hash": "0x" + "55" * 32,
                "destination_binding_hash": "0x" + "66" * 32,
            }
        )

    tron_request = build_tron_sccp_proof_request(sample_tron_production_request_input())
    tron_result = wrap_tron_sccp_proof_result(GROTH16_PROOF_BYTES, tron_request)
    tron_submission = build_tron_sccp_submission({"proof_result": tron_result})
    tron_words = sccp_message_transparent_public_input_abi_words(
        sample_tron_public_inputs()
    )

    assert tron_submission["submission_kind"] == "contract_call"
    assert tron_submission["platform_payload"] == "tron_contract_call"
    assert tron_submission["envelope_encoding"] == SCCP_TRON_CONTRACT_CALL_ABI_TUPLE_V1
    assert tron_submission["verifier_backend"] == SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1
    assert tron_submission["function_selector"] == SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1
    assert tron_submission["target_domain"] == SCCP_DOMAIN_TRON
    assert tron_submission["public_input_words"] == tron_words
    assert tron_submission["public_signal_words"] == tron_result["public_signal_words"]
    assert tron_submission["call_data"] == sccp_submit_message_proof_call_data(
        GROTH16_PROOF_BYTES,
        sample_tron_public_inputs(),
        tron_result["statement_hash"],
    )
    omitted_tron_result = wrap_tron_sccp_proof_result(
        GROTH16_PROOF_BYTES,
        build_tron_sccp_proof_request(
                sample_tron_production_request_input(source_proof_bytes=b"")
        ),
    )
    omitted_tron_submission = build_tron_sccp_submission(
        {"proof_result": omitted_tron_result}
    )
    assert omitted_tron_result["source_proof_bytes"] == b""
    assert omitted_tron_submission["proof_bytes"] == GROTH16_PROOF_BYTES
    with pytest.raises(
        TypeError,
        match="bundleBytes requires proofResult for request-bound submission",
    ):
        build_evm_sccp_submission(
            {
                "proof_bytes": GROTH16_PROOF_BYTES,
                "public_inputs": sample_evm_public_inputs(),
                "statement_hash": HEX32_G,
                "destination_binding_hash": HEX32_H,
                "bundle_bytes": bytes([5, 6, 7]),
            }
        )
    with pytest.raises(
        TypeError,
        match="sourceProofBytes requires proofResult for request-bound submission",
    ):
        build_tron_sccp_submission(
            {
                "proof_bytes": GROTH16_PROOF_BYTES,
                "public_inputs": sample_tron_public_inputs(),
                "statement_hash": HEX32_G,
                "destination_binding_hash": HEX32_H,
                "source_proof_bytes": bytes([9, 10]),
            }
        )
    assert len(tron_submission["call_data"]) == 676
    assert tron_submission["call_data"][292:] == GROTH16_PROOF_BYTES
    with pytest.raises(TypeError, match=r"proofResult\.requestHash.*multiple aliases"):
        build_tron_sccp_submission(
            {
                "proof_result": {
                    **dict(tron_result),
                    "requestHash": tron_result["request_hash"],
                },
            }
        )
    with pytest.raises(
        TypeError,
        match="proofResult must be a wrapped Groth16 SCCP proof result",
    ):
        build_tron_sccp_submission(
            {
                "proof_result": None,
                "proof_bytes": GROTH16_PROOF_BYTES,
                "public_inputs": sample_tron_public_inputs(),
                "statement_hash": HEX32_G,
                "destination_binding_hash": HEX32_H,
            }
        )

    with pytest.raises(TypeError, match="destinationBindingHash"):
        build_tron_sccp_submission(
            {"proof_result": tron_result, "destination_binding_hash": HEX32_A}
        )
    with pytest.raises(TypeError, match="publicInputs must match"):
        build_tron_sccp_submission(
            {
                "proof_result": tron_result,
                "public_inputs": sample_tron_public_inputs(commitment_root=HEX32_A),
            }
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.envelopeHash must match wrapped proof bytes",
    ):
        build_tron_sccp_submission(
            {"proof_result": {**dict(tron_result), "envelope_hash": HEX32_A}}
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.proofBase64 must match proofResult\.proofBytes",
    ):
        build_tron_sccp_submission(
            {"proof_result": {**dict(tron_result), "proof_base64": "AAAA"}}
        )
    with pytest.raises(
        TypeError,
        match=r"proofResult\.requestHash must match bundleBytes and sourceProofBytes",
    ):
        build_tron_sccp_submission(
            {"proof_result": {**dict(tron_result), "bundle_bytes": bytes([5, 6, 8])}}
        )
    wrong_tron_context = dict(tron_result["proof_context"])
    wrong_tron_context["destination_binding_hash"] = HEX32_A
    with pytest.raises(TypeError, match="proofResult.proofContext"):
        build_tron_sccp_submission(
            {"proof_result": {**dict(tron_result), "proof_context": wrong_tron_context}}
        )
    with pytest.raises(ValueError, match="TRON"):
        build_tron_sccp_submission(
            {
                "proof_bytes": GROTH16_PROOF_BYTES,
                "public_inputs": sample_tron_public_inputs(target_domain=SCCP_DOMAIN_ETH),
                "statement_hash": "0x" + "55" * 32,
                "destination_binding_hash": "0x" + "66" * 32,
            }
        )


def test_rejects_malformed_evm_and_tron_groth16_proof_tuples() -> None:
    evm_request = build_evm_sccp_proof_request(sample_evm_production_request_input())
    with pytest.raises(TypeError, match=r"proofBytes\.version must be 1"):
        wrap_evm_sccp_proof_result(
            groth16_proof_bytes(words={0: abi_word(2)}),
            evm_request,
        )

    tron_request = build_tron_sccp_proof_request(sample_tron_production_request_input())
    with pytest.raises(TypeError, match=r"proofBytes\.a\.x must be a BN254 base-field element"):
        wrap_tron_sccp_proof_result(
            groth16_proof_bytes(words={4: bytes([0xFF]) * 32}),
            tron_request,
        )
    with pytest.raises(TypeError, match=r"proofBytes\.a must not be zero"):
        wrap_tron_sccp_proof_result(
            groth16_proof_bytes(words={4: bytes(32), 5: bytes(32)}),
            tron_request,
        )
    with pytest.raises(TypeError, match=r"proofBytes\.b must not be zero"):
        wrap_tron_sccp_proof_result(
            groth16_proof_bytes(
                words={
                    6: bytes(32),
                    7: bytes(32),
                    8: bytes(32),
                    9: bytes(32),
                }
            ),
            tron_request,
        )
    with pytest.raises(TypeError, match=r"proofBytes\.c must not be zero"):
        wrap_tron_sccp_proof_result(
            groth16_proof_bytes(words={10: bytes(32), 11: bytes(32)}),
            tron_request,
        )
    with pytest.raises(TypeError, match=r"proofBytes\.c must be a BN254 G1 point"):
        wrap_evm_sccp_proof_result(
            groth16_proof_bytes(words={11: abi_word(3)}),
            evm_request,
        )
    off_curve_b = bytearray(groth16_proof_bytes())
    off_curve_b[6 * 32 + 31] ^= 0x01
    with pytest.raises(TypeError, match=r"proofBytes\.b must be a BN254 G2 point"):
        wrap_tron_sccp_proof_result(bytes(off_curve_b), tron_request)
    with pytest.raises(TypeError, match=r"proofBytes\.b must be a BN254 G2 point"):
        wrap_evm_sccp_proof_result(
            groth16_proof_bytes(
                words={
                    6: abi_word(0),
                    7: abi_word(1),
                    8: bytes.fromhex(
                        "0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8"
                    ),
                    9: bytes.fromhex(
                        "07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2"
                    ),
                }
            ),
            evm_request,
        )
    with pytest.raises(TypeError, match=r"proofBytes\.b must be a BN254 G2 point"):
        wrap_tron_sccp_proof_result(
            groth16_proof_bytes(
                words={
                    6: abi_word(0),
                    7: abi_word(1),
                    8: bytes.fromhex(
                        "0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8"
                    ),
                    9: bytes.fromhex(
                        "07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2"
                    ),
                }
            ),
            tron_request,
        )
    with pytest.raises(
        TypeError,
        match=r"proofBytes\.messageId must match publicInputs\.messageId",
    ):
        wrap_evm_sccp_proof_result(
            groth16_proof_bytes(words={1: bytes([0x22]) * 32}),
            evm_request,
        )
    with pytest.raises(
        TypeError,
        match=r"proofBytes\.sourceDomain must match sourceDomain",
    ):
        wrap_tron_sccp_proof_result(
            groth16_proof_bytes(words={2: abi_word(999)}),
            tron_request,
        )
    with pytest.raises(
        TypeError,
        match=r"proofBytes\.sourceDomain must match sourceDomain",
    ):
        sccp_submit_message_proof_call_data(
            groth16_proof_bytes(words={2: abi_word(SCCP_DOMAIN_ETH)}),
            sample_tron_public_inputs(),
            HEX32_G,
        )
    with pytest.raises(ValueError, match="sourceDomain must be SORA"):
        sccp_submit_message_proof_call_data(
            groth16_proof_bytes(words={2: abi_word(SCCP_DOMAIN_ETH)}),
            sample_tron_public_inputs(),
            HEX32_G,
            SCCP_DOMAIN_ETH,
        )
    with pytest.raises(
        TypeError,
        match=r"proofBytes\.commitmentRoot must match publicInputs\.commitmentRoot",
    ):
        build_evm_sccp_submission(
            {
                "proof_bytes": groth16_proof_bytes(words={3: bytes([0x44]) * 32}),
                "public_inputs": sample_evm_public_inputs(),
                "source_domain": SCCP_DOMAIN_SORA,
                "statement_hash": HEX32_G,
                "destination_binding_hash": HEX32_H,
            }
        )


def test_evm_and_tron_sccp_provers_reject_all_zero_groth16_proof_bytes() -> None:
    async def zero_proof(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [0, 0]}

    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(EvmSccpProver(prove=zero_proof).prove(sample_evm_production_request_input()))

    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(TronSccpProver(prove=zero_proof).prove(sample_tron_production_request_input()))


def test_ton_and_substrate_sccp_provers_reject_all_zero_proof_bytes() -> None:
    async def zero_proof(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [0, 0]}

    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(
            TonSccpProver(prove=zero_proof).prove(
                sample_ton_request_input(
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
        )

    with pytest.raises(TypeError, match="proofBytes must not be all zero"):
        asyncio.run(
            SubstrateSccpProver(prove=zero_proof).prove(sample_substrate_request_input())
        )


def test_evm_and_tron_sccp_provers_reject_noncanonical_groth16_proof_lengths() -> None:
    async def short_proof(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [1, 2, 3, 4]}

    with pytest.raises(TypeError, match="proofBytes must be 384 bytes"):
        asyncio.run(EvmSccpProver(prove=short_proof).prove(sample_evm_production_request_input()))

    with pytest.raises(TypeError, match="proofBytes must be 384 bytes"):
        asyncio.run(TronSccpProver(prove=short_proof).prove(sample_tron_production_request_input()))


def test_sccp_provers_reject_results_bound_to_different_request_contexts() -> None:
    async def wrong_solana_context(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [1, 2, 3, 4], "proof_context_hash": HEX32_A}

    with pytest.raises(TypeError, match="proofContextHash must match request"):
        asyncio.run(
            SolanaSccpProver(prove=wrong_solana_context).prove(
                sample_production_witness()
            )
        )

    async def wrong_ton_deployment(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": [1, 2, 3, 4],
            "source_adapter_deployment_binding_hash": HEX32_C,
        }

    with pytest.raises(
        TypeError, match="sourceAdapterDeploymentBindingHash must match request"
    ):
        asyncio.run(
            TonSccpProver(prove=wrong_ton_deployment).prove(
                sample_ton_request_input(
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
        )

    async def wrong_ton_proof_base64(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [1, 2, 3, 4], "proof_base64": "AAAA"}

    with pytest.raises(TypeError, match=r"proofResult\.proofBase64"):
        asyncio.run(
            TonSccpProver(prove=wrong_ton_proof_base64).prove(
                sample_ton_request_input(
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
        )

    async def padded_ton_proof_base64(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": [1, 2, 3, 4],
            "proof_base64": " "
            + base64.b64encode(bytes([1, 2, 3, 4])).decode("ascii")
            + " ",
        }

    with pytest.raises(TypeError, match=r"proofResult\.proofBase64"):
        asyncio.run(
            TonSccpProver(prove=padded_ton_proof_base64).prove(
                sample_ton_request_input(
                    source_adapter_deployment_hash=HEX32_A,
                    source_adapter_deployment_receipt_hash=HEX32_B,
                )
            )
        )

    async def wrong_evm_request(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": GROTH16_PROOF_BYTES, "request_hash": HEX32_A}

    with pytest.raises(TypeError, match="requestHash must match request"):
        asyncio.run(EvmSccpProver(prove=wrong_evm_request).prove(sample_evm_production_request_input()))

    async def wrong_evm_proof_base64(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": GROTH16_PROOF_BYTES, "proof_base64": "AAAA"}

    with pytest.raises(TypeError, match=r"proofResult\.proofBase64"):
        asyncio.run(
            EvmSccpProver(prove=wrong_evm_proof_base64).prove(
                sample_evm_production_request_input()
            )
        )

    async def padded_evm_proof_base64(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "proof_base64": (
                " " + base64.b64encode(GROTH16_PROOF_BYTES).decode("ascii") + " "
            ),
        }

    with pytest.raises(TypeError, match=r"proofResult\.proofBase64"):
        asyncio.run(
            EvmSccpProver(prove=padded_evm_proof_base64).prove(
                sample_evm_production_request_input()
            )
        )

    async def wrong_evm_public_inputs(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "public_inputs": sample_evm_public_inputs(commitment_root=HEX32_A),
        }

    with pytest.raises(TypeError, match=r"proofResult\.publicInputs"):
        asyncio.run(
            EvmSccpProver(prove=wrong_evm_public_inputs).prove(
                sample_evm_production_request_input()
            )
        )

    async def duplicate_evm_public_inputs(
        request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "publicInputs": request["public_inputs"],
            "public_inputs": request["public_inputs"],
        }

    with pytest.raises(TypeError, match=r"proofResult\.publicInputs"):
        asyncio.run(
            EvmSccpProver(prove=duplicate_evm_public_inputs).prove(
                sample_evm_production_request_input()
            )
        )

    async def null_evm_public_inputs(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": GROTH16_PROOF_BYTES, "public_inputs": None}

    with pytest.raises(TypeError, match="publicInputs"):
        asyncio.run(
            EvmSccpProver(prove=null_evm_public_inputs).prove(
                sample_evm_production_request_input()
            )
        )

    async def wrong_tron_backend(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "backend": SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
            "proof_bytes": GROTH16_PROOF_BYTES,
        }

    with pytest.raises(TypeError, match="backend must match request"):
        asyncio.run(TronSccpProver(prove=wrong_tron_backend).prove(sample_tron_production_request_input()))

    async def wrong_tron_proof_base64(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": GROTH16_PROOF_BYTES, "proofBase64": "AAAA"}

    with pytest.raises(TypeError, match=r"proofResult\.proofBase64"):
        asyncio.run(
            TronSccpProver(prove=wrong_tron_proof_base64).prove(
                sample_tron_production_request_input()
            )
        )

    async def duplicate_tron_proof_base64(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        proof_base64 = base64.b64encode(GROTH16_PROOF_BYTES).decode("ascii")
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "proofBase64": proof_base64,
            "proof_base64": proof_base64,
        }

    with pytest.raises(TypeError, match=r"proofResult\.proofBase64"):
        asyncio.run(
            TronSccpProver(prove=duplicate_tron_proof_base64).prove(
                sample_tron_production_request_input()
            )
        )

    async def wrong_tron_proof_context(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": GROTH16_PROOF_BYTES,
            "proof_context": {
                "version": 1,
                "statement_hash": "0x" + "55" * 32,
                "destination_binding_hash": HEX32_A,
            },
        }

    with pytest.raises(TypeError, match=r"proofResult\.proofContext"):
        asyncio.run(
            TronSccpProver(prove=wrong_tron_proof_context).prove(
                sample_tron_production_request_input()
            )
        )

    async def null_tron_request_hash(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": GROTH16_PROOF_BYTES, "request_hash": None}

    with pytest.raises(TypeError, match="requestHash"):
        asyncio.run(
            TronSccpProver(prove=null_tron_request_hash).prove(
                sample_tron_production_request_input()
            )
        )

    async def wrong_substrate_request(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [1, 2, 3, 4], "request_hash": HEX32_A}

    with pytest.raises(TypeError, match="requestHash must match request"):
        asyncio.run(
            SubstrateSccpProver(prove=wrong_substrate_request).prove(
                sample_substrate_request_input()
            )
        )

    async def wrong_substrate_proof_base64(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {"proof_bytes": [1, 2, 3, 4], "proof_base64": "AAAA"}

    with pytest.raises(TypeError, match=r"proofResult\.proofBase64"):
        asyncio.run(
            SubstrateSccpProver(prove=wrong_substrate_proof_base64).prove(
                sample_substrate_request_input()
            )
        )

    async def wrong_substrate_public_inputs(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": [1, 2, 3, 4],
            "public_inputs": sample_substrate_public_inputs(commitment_root=HEX32_A),
        }

    with pytest.raises(TypeError, match=r"proofResult\.publicInputs"):
        asyncio.run(
            SubstrateSccpProver(prove=wrong_substrate_public_inputs).prove(
                sample_substrate_request_input()
            )
        )

    async def wrong_substrate_proof_context(
        _request: Mapping[str, Any], _options: Mapping[str, Any]
    ) -> Dict[str, Any]:
        return {
            "proof_bytes": [1, 2, 3, 4],
            "proof_context": {
                "version": 1,
                "statement_hash": "0x" + "55" * 32,
                "destination_binding_hash": HEX32_A,
            },
        }

    with pytest.raises(TypeError, match=r"proofResult\.proofContext"):
        asyncio.run(
            SubstrateSccpProver(prove=wrong_substrate_proof_context).prove(
                sample_substrate_request_input()
            )
        )


def test_evm_tron_and_substrate_wrappers_reject_mutated_proof_requests() -> None:
    hash_only_evm_request = build_evm_sccp_proof_request(sample_evm_request_input())
    with pytest.raises(
        TypeError,
        match="EVM-family SCCP production proofs must include destinationBinding deployment material",
    ):
        wrap_evm_sccp_proof_result(GROTH16_PROOF_BYTES, hash_only_evm_request)

    evm_request = dict(build_evm_sccp_proof_request(sample_evm_production_request_input()))
    evm_request["request_hash"] = HEX32_A
    with pytest.raises(
        TypeError,
        match="EVM-family SCCP proof request must be canonical",
    ):
        wrap_evm_sccp_proof_result(GROTH16_PROOF_BYTES, evm_request)

    hash_only_tron_request = build_tron_sccp_proof_request(sample_tron_request_input())
    with pytest.raises(
        TypeError,
        match="TRON SCCP production proofs must include destinationBinding deployment material",
    ):
        wrap_tron_sccp_proof_result(GROTH16_PROOF_BYTES, hash_only_tron_request)

    tron_request = dict(build_tron_sccp_proof_request(sample_tron_production_request_input()))
    tron_signals = list(tron_request["public_signal_words"])
    tron_signals[0] = HEX32_A
    tron_request["public_signal_words"] = tron_signals
    with pytest.raises(TypeError, match="TRON SCCP proof request must be canonical"):
        wrap_tron_sccp_proof_result(GROTH16_PROOF_BYTES, tron_request)

    substrate_request = dict(
        build_substrate_sccp_proof_request(sample_substrate_request_input())
    )
    substrate_context = dict(substrate_request["proof_context"])
    substrate_context["destination_binding_hash"] = HEX32_A
    substrate_request["proof_context"] = substrate_context
    with pytest.raises(
        TypeError,
        match="Substrate SCCP proof request must be canonical",
    ):
        wrap_substrate_sccp_proof_result([1, 2, 3, 4], substrate_request)


def test_evm_tron_and_substrate_provers_reject_non_production_input_before_callback() -> None:
    invoked = False

    async def prove(_request: Mapping[str, Any], _options: Mapping[str, Any]) -> Dict[str, Any]:
        nonlocal invoked
        invoked = True
        return {"proof_bytes": GROTH16_PROOF_BYTES}

    with pytest.raises(ValueError, match="ETH or BSC"):
        asyncio.run(
            EvmSccpProver(prove=prove).prove(
                sample_evm_request_input(
                    public_inputs=sample_evm_public_inputs(target_domain=SCCP_DOMAIN_TON)
                )
            )
        )
    assert not invoked

    with pytest.raises(ValueError, match="targetDomain must be TRON"):
        asyncio.run(
            TronSccpProver(prove=prove).prove(
                sample_tron_request_input(
                    public_inputs=sample_tron_public_inputs(target_domain=SCCP_DOMAIN_ETH)
                )
            )
        )
    assert not invoked

    with pytest.raises(ValueError, match="Substrate-family SCCP domain"):
        asyncio.run(
            SubstrateSccpProver(prove=prove).prove(
                sample_substrate_request_input(
                    public_inputs=sample_substrate_public_inputs(
                        target_domain=SCCP_DOMAIN_TON
                    )
                )
            )
        )
    assert not invoked
