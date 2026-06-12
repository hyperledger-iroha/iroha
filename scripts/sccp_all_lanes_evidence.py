#!/usr/bin/env python3
"""Validate SCCP all-lanes rollout evidence bundles."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import importlib.util
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from typing import Any, Callable

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python < 3.11 fallback
    tomllib = None  # type: ignore[assignment]


SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_ETH = 1
SCCP_DOMAIN_BSC = 2
SCCP_DOMAIN_SOL = 3
SCCP_DOMAIN_TON = 4
SCCP_DOMAIN_TRON = 5
SOLANA_UPGRADEABLE_LOADER_ID = "BPFLoaderUpgradeab1e11111111111111111111111"
SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG = 2
SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG = 3
SOLANA_UPGRADEABLE_PROGRAM_ACCOUNT_LEN = 36
SOLANA_PROGRAMDATA_METADATA_LEN = 45
EVM_EXPECTED_RPC_CHAIN_IDS = {
    SCCP_DOMAIN_ETH: 1,
    SCCP_DOMAIN_BSC: 56,
}
SCCP_CORE_REMOTE_DOMAINS = (
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
)
SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS = (
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
)
SCCP_UNSUPPORTED_LAUNCH_REMOTE_DOMAINS: tuple[int, ...] = ()
SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID = "sccp-source-adapter-v1"
SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1"
ROUTE_CANARY_EVIDENCE_SOURCE_BY_DOMAIN = {
    SCCP_DOMAIN_ETH: "evm_message_proof_accepted_transaction",
    SCCP_DOMAIN_BSC: "evm_message_proof_accepted_transaction",
    SCCP_DOMAIN_SOL: "solana_live_programdata_snapshot",
    SCCP_DOMAIN_TON: "ton_live_account_snapshot",
    SCCP_DOMAIN_TRON: "tron_message_proof_accepted_transaction",
}


@dataclass(frozen=True)
class LaneProfile:
    """Static SCCP v1 production profile for one remote lane."""

    domain: int
    chain: str
    source_proof_plan: str
    finality_model: str
    destination_verifier_plan: str
    source_trust_anchor_id: str
    consensus_verifier_id: str
    message_inclusion_verifier_id: str
    finality_policy_id: str
    destination_anchor_id: str
    route_allowlist_id: str
    source_state_verifier_id: str = ""
    source_bridge_emitter_id: str = ""
    destination_verifier_key_hash_required: bool = False
    eth_source_bridge_config_required: bool = False
    evm_source_gate_required: bool = False
    tron_source_bridge_config_required: bool = False
    solana_full_light_client_audit_required: bool = False
    ton_full_light_client_audit_required: bool = False


LANE_PROFILES: dict[int, LaneProfile] = {
    SCCP_DOMAIN_ETH: LaneProfile(
        domain=SCCP_DOMAIN_ETH,
        chain="eth",
        source_proof_plan="EthereumBeaconReceiptProof",
        finality_model="EthereumBeaconExecution",
        destination_verifier_plan="EvmGroth16Bn254Adapter",
        source_trust_anchor_id=(
            "sccp:eth:source-trust-anchor:"
            "ethereum-mainnet-beacon-finalized-checkpoint:v1"
        ),
        consensus_verifier_id=(
            "sccp:eth:consensus-verifier:"
            "beacon-sync-committee-execution-header-mainnet:v1"
        ),
        message_inclusion_verifier_id=(
            "sccp:eth:message-inclusion-verifier:"
            "execution-receipt-trie-branch-mainnet:v1"
        ),
        finality_policy_id=(
            "sccp:eth:finality-policy:beacon-finalized-checkpoint-mainnet:v1"
        ),
        destination_anchor_id="sccp:eth:destination-anchor:ethereum-mainnet:v1",
        route_allowlist_id="sccp:eth:route-allowlist:ethereum-mainnet:v1",
        source_bridge_emitter_id="sccp:eth:source-bridge-emitter:ethereum-mainnet:v1",
        destination_verifier_key_hash_required=True,
        eth_source_bridge_config_required=True,
        evm_source_gate_required=True,
    ),
    SCCP_DOMAIN_BSC: LaneProfile(
        domain=SCCP_DOMAIN_BSC,
        chain="bsc",
        source_proof_plan="BscValidatorSetReceiptProof",
        finality_model="BscValidatorSet",
        destination_verifier_plan="EvmGroth16Bn254Adapter",
        source_trust_anchor_id="sccp:bsc:source-trust-anchor:bsc-mainnet-validator-set:v1",
        consensus_verifier_id="sccp:bsc:consensus-verifier:validator-set-seal-mainnet:v1",
        message_inclusion_verifier_id=(
            "sccp:bsc:message-inclusion-verifier:receipt-trie-branch-mainnet:v1"
        ),
        finality_policy_id="sccp:bsc:finality-policy:validator-set-finality-mainnet:v1",
        destination_anchor_id="sccp:bsc:destination-anchor:bsc-mainnet:v1",
        route_allowlist_id="sccp:bsc:route-allowlist:bsc-mainnet:v1",
        source_bridge_emitter_id="sccp:bsc:source-bridge-emitter:bsc-mainnet:v1",
        destination_verifier_key_hash_required=True,
        evm_source_gate_required=True,
    ),
    SCCP_DOMAIN_SOL: LaneProfile(
        domain=SCCP_DOMAIN_SOL,
        chain="sol",
        source_proof_plan="SolanaFinalizedTransactionProof",
        finality_model="SolanaFinalizedSlot",
        destination_verifier_plan="SolanaProgramNativeRecursive",
        source_trust_anchor_id="sccp:sol:source-trust-anchor:solana-mainnet-beta-genesis:v1",
        consensus_verifier_id=(
            "sccp:sol:consensus-verifier:finalized-slot-bankhash-mainnet-beta:v1"
        ),
        message_inclusion_verifier_id=(
            "sccp:sol:message-inclusion-verifier:transaction-status-root-branch:v1"
        ),
        finality_policy_id="sccp:sol:finality-policy:finalized-slot-mainnet-beta:v1",
        destination_anchor_id="sccp:sol:destination-anchor:solana-mainnet-beta:v1",
        route_allowlist_id="sccp:sol:route-allowlist:solana-mainnet-beta:v1",
        source_state_verifier_id=(
            "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1"
        ),
        solana_full_light_client_audit_required=True,
    ),
    SCCP_DOMAIN_TON: LaneProfile(
        domain=SCCP_DOMAIN_TON,
        chain="ton",
        source_proof_plan="TonMasterchainShardProof",
        finality_model="TonMasterchain",
        destination_verifier_plan="TonContractNativeRecursive",
        source_trust_anchor_id="sccp:ton:source-trust-anchor:ton-mainnet-masterchain:v1",
        consensus_verifier_id="sccp:ton:consensus-verifier:masterchain-block-proof:v1",
        message_inclusion_verifier_id=(
            "sccp:ton:message-inclusion-verifier:shard-transaction-branch:v1"
        ),
        finality_policy_id="sccp:ton:finality-policy:masterchain-finality:v1",
        destination_anchor_id="sccp:ton:destination-anchor:ton-mainnet:v1",
        route_allowlist_id="sccp:ton:route-allowlist:ton-mainnet:v1",
        source_state_verifier_id=(
            "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1"
        ),
        ton_full_light_client_audit_required=True,
    ),
    SCCP_DOMAIN_TRON: LaneProfile(
        domain=SCCP_DOMAIN_TRON,
        chain="tron",
        source_proof_plan="TronDposReceiptProof",
        finality_model="TronDpos",
        destination_verifier_plan="TronContractGroth16Bn254",
        source_trust_anchor_id="sccp:tron:source-trust-anchor:mainnet-witness-schedule:v1",
        consensus_verifier_id="sccp:tron:consensus-verifier:dpos-solid-block-mainnet:v1",
        message_inclusion_verifier_id=(
            "sccp:tron:message-inclusion-verifier:transaction-source-mainnet:v1"
        ),
        finality_policy_id="sccp:tron:finality-policy:solid-block-mainnet:v1",
        destination_anchor_id="sccp:tron:destination-anchor:tron-mainnet:v1",
        route_allowlist_id="sccp:tron:route-allowlist:tron-mainnet:v1",
        source_bridge_emitter_id="sccp:tron:source-bridge-emitter:tron-mainnet:v1",
        destination_verifier_key_hash_required=True,
        tron_source_bridge_config_required=True,
    ),
}

SECTION_NAMES = (
    "sccp_source_verifier_materials",
    "sccp_source_adapter_engine_deployments",
    "sccp_destination_rollouts",
    "sccp_route_allowlists",
)

SOLANA_FULL_LIGHT_CLIENT_AUDIT_FIELDS = (
    "solana_tower_replay_verifier_hash",
    "solana_full_accountsdb_lattice_verifier_hash",
    "solana_bank_fork_choice_verifier_hash",
    "solana_full_light_client_gate_hash",
)
TON_FULL_LIGHT_CLIENT_AUDIT_FIELDS = (
    "ton_masterchain_config_verifier_hash",
    "ton_validator_set_transition_verifier_hash",
    "ton_shard_accounts_dictionary_verifier_hash",
    "ton_full_light_client_gate_hash",
)
EVM_SOURCE_GATE_FIELDS = ("evm_source_gate_hash",)
TRON_DPOS_SOURCE_GATE_FIELDS = ("tron_dpos_source_gate_hash",)
EVM_SOURCE_BRIDGE_LIVE_COMMENT_FIELDS = (
    "_comment_evm_source_rpc_chain_id",
    "_comment_evm_source_block_tag",
    "_comment_evm_source_bridge_address",
    "_comment_evm_source_bridge_code_hash",
    "_comment_evm_source_bridge_runtime_bytecode_hex",
    "_comment_eth_source_bridge_network_id",
    "_comment_eth_source_bridge_config_hash",
    "_comment_evm_source_deployment_transaction_hash",
    "_comment_evm_source_deployment_transaction_block_hash",
    "_comment_evm_source_deployment_transaction_block_number",
    "_comment_evm_source_deployment_transaction_input_sha256",
    "_comment_evm_source_deployment_receipt_status",
    "_comment_evm_source_deployment_contract_address",
    "_comment_evm_source_deployment_block_hash",
    "_comment_evm_source_deployment_block_number",
    "_comment_evm_source_deployment_block_receipts_root",
)
TRON_SOURCE_BRIDGE_LIVE_COMMENT_FIELDS = (
    "_comment_tron_source_bridge_address",
    "_comment_tron_source_bridge_code_hash",
    "_comment_tron_source_bridge_runtime_bytecode_hex",
    "_comment_tron_source_bridge_config_hash",
)
EVM_DESTINATION_BRIDGE_BINDING_FIELDS = (
    "destination_bridge_address",
    "_comment_destination_bridge_address",
)
EVM_TRON_DESTINATION_NETWORK_BINDING_FIELDS = (
    "destination_network_id",
    "_comment_destination_network_id",
)
EVM_DESTINATION_VERIFIER_LIVE_COMMENT_FIELDS = (
    "_comment_evm_rpc_chain_id",
    "_comment_evm_block_tag",
    "_comment_evm_bridge_code_hash",
    "_comment_evm_bridge_runtime_bytecode_hex",
    "_comment_evm_verifier_code_hash",
    "_comment_evm_verifier_runtime_bytecode_hex",
    "_comment_evm_verifier_key_hash",
    "_comment_evm_verifier_backend_hash",
    "_comment_evm_proof_family_hash",
)
SOLANA_DESTINATION_LIVE_FIELDS = (
    "solana_rpc_commitment",
    "solana_program_owner",
    "solana_programdata_owner",
    "solana_program_immutable",
    "solana_program_account_data_base64",
    "solana_programdata_address",
    "solana_programdata_slot",
    "solana_expected_programdata_slot",
    "solana_program_account_context_slot",
    "solana_programdata_account_context_slot",
    "solana_programdata_metadata_blake2b256",
    "solana_programdata_metadata_base64",
    "solana_programdata_executable_blake2b256",
    "solana_programdata_executable_base64",
    "_comment_solana_rpc_commitment",
    "_comment_solana_program_owner",
    "_comment_solana_programdata_owner",
    "_comment_solana_program_immutable",
    "_comment_solana_program_account_data_len",
    "_comment_solana_program_account_data_base64",
    "_comment_solana_programdata_address",
    "_comment_solana_programdata_slot",
    "_comment_solana_expected_programdata_slot",
    "_comment_solana_program_account_context_slot",
    "_comment_solana_programdata_account_context_slot",
    "_comment_solana_programdata_metadata_blake2b256",
    "_comment_solana_programdata_metadata_base64",
    "_comment_solana_programdata_code_hash",
    "_comment_solana_programdata_executable_base64",
)
TON_DESTINATION_LIVE_FIELDS = (
    "ton_account_status",
    "ton_account_state_hash",
    "ton_last_transaction_lt",
    "ton_last_transaction_hash",
    "ton_verifier_code_boc_root_hash",
    "ton_verifier_code_boc",
    "_comment_ton_account_status",
    "_comment_ton_account_state_hash",
    "_comment_ton_last_transaction_lt",
    "_comment_ton_last_transaction_hash",
    "_comment_ton_code_hash",
    "_comment_ton_code_boc_root_hash",
    "_comment_ton_code_boc_base64",
    "_comment_ton_code_boc_hash_matches",
)
TRON_DESTINATION_VERIFIER_LIVE_COMMENT_FIELDS = (
    "_comment_tron_destination_verifier_address",
    "_comment_tron_destination_verifier_code_hash",
    "_comment_tron_destination_verifier_runtime_bytecode_hex",
    "_comment_tron_destination_verifier_key_hash",
    "_comment_tron_destination_verifier_backend_hash",
    "_comment_tron_destination_proof_family_hash",
)
SOURCE_VERIFIER_MATERIAL_ROLE_HASH_FIELDS = (
    "source_trust_anchor_hash",
    "consensus_verifier_hash",
    "message_inclusion_verifier_hash",
    "finality_policy_hash",
    "source_state_verifier_hash",
    "source_bridge_emitter_code_hash",
    "source_bridge_network_id",
    "source_bridge_config_hash",
)
SOURCE_ADAPTER_DEPLOYMENT_ROLE_HASH_FIELDS = (
    *SOURCE_VERIFIER_MATERIAL_ROLE_HASH_FIELDS,
    "adapter_verifier_vk_hash",
    "deployment_receipt_hash",
)
EVM_SOURCE_GATE_ROLE_HASH_FIELDS = ("evm_source_gate_hash",)
SOLANA_FULL_LIGHT_CLIENT_AUDIT_ROLE_HASH_FIELDS = (
    "solana_tower_replay_verifier_hash",
    "solana_full_accountsdb_lattice_verifier_hash",
    "solana_bank_fork_choice_verifier_hash",
)
TON_FULL_LIGHT_CLIENT_AUDIT_ROLE_HASH_FIELDS = (
    "ton_masterchain_config_verifier_hash",
    "ton_validator_set_transition_verifier_hash",
    "ton_shard_accounts_dictionary_verifier_hash",
)
SOURCE_MATERIAL_FIELDS = frozenset(
    (
        "version",
        "source_domain",
        "source_chain",
        "source_proof_plan",
        "finality_model",
        "adapter_circuit_id",
        "source_trust_anchor_id",
        "source_trust_anchor_hash",
        "consensus_verifier_id",
        "consensus_verifier_hash",
        "message_inclusion_verifier_id",
        "message_inclusion_verifier_hash",
        "source_state_verifier_id",
        "source_state_verifier_hash",
        "finality_policy_id",
        "finality_policy_hash",
        "source_bridge_emitter_id",
        "source_bridge_emitter_address",
        "source_bridge_emitter_code_hash",
        "source_bridge_network_id",
        "source_bridge_owner_address",
        "source_bridge_config_hash",
        "_comment_source_verifier_material_hash",
        "_comment_evm_source_rpc_chain_id",
        "_comment_evm_source_block_tag",
        "_comment_evm_source_bridge_address",
        "_comment_evm_source_bridge_code_hash",
        "_comment_evm_source_bridge_runtime_bytecode_hex",
        "_comment_eth_source_bridge_network_id",
        "_comment_eth_source_bridge_config_hash",
        "_comment_evm_source_deployment_transaction_hash",
        "_comment_evm_source_deployment_transaction_block_hash",
        "_comment_evm_source_deployment_transaction_block_number",
        "_comment_evm_source_deployment_transaction_input_sha256",
        "_comment_evm_source_deployment_receipt_status",
        "_comment_evm_source_deployment_contract_address",
        "_comment_evm_source_deployment_block_hash",
        "_comment_evm_source_deployment_block_number",
        "_comment_evm_source_deployment_block_receipts_root",
        "_comment_tron_source_bridge_address",
        "_comment_tron_source_bridge_code_hash",
        "_comment_tron_source_bridge_runtime_bytecode_hex",
        "_comment_tron_source_bridge_config_hash",
        "placeholder_material",
    )
)
SOURCE_DEPLOYMENT_FIELDS = frozenset(
    field for field in SOURCE_MATERIAL_FIELDS if field != "placeholder_material"
) | frozenset(
    (
        "target_domain",
        "adapter_proof_family",
        "adapter_verifier_vk_hash",
        "deployment_receipt_hash",
        "_comment_source_adapter_engine_deployment_hash",
        *SOLANA_FULL_LIGHT_CLIENT_AUDIT_FIELDS,
        *TON_FULL_LIGHT_CLIENT_AUDIT_FIELDS,
        *EVM_SOURCE_GATE_FIELDS,
        *TRON_DPOS_SOURCE_GATE_FIELDS,
    )
)
DESTINATION_ROLLOUT_FIELDS = frozenset(
    (
        "version",
        "domain",
        "chain",
        "verifier_plan",
        "verifier_identity",
        "verifier_code_hash",
        "verifier_key_hash",
        "immutable_verifier_ready",
        "anchors_ready",
        "anchor_id",
        "blockers",
        "destination_network_id",
        "destination_bridge_address",
        "destination_binding_key",
        "destination_binding_hash",
        "solana_rpc_commitment",
        "solana_program_owner",
        "solana_programdata_owner",
        "solana_program_immutable",
        "solana_program_account_data_base64",
        "solana_programdata_address",
        "solana_programdata_slot",
        "solana_expected_programdata_slot",
        "solana_program_account_context_slot",
        "solana_programdata_account_context_slot",
        "solana_programdata_metadata_blake2b256",
        "solana_programdata_metadata_base64",
        "solana_programdata_executable_blake2b256",
        "solana_programdata_executable_base64",
        "ton_account_status",
        "ton_account_state_hash",
        "ton_last_transaction_lt",
        "ton_last_transaction_hash",
        "ton_verifier_code_boc_root_hash",
        "ton_verifier_code_boc",
        "_comment_destination_network_id",
        "_comment_destination_bridge_address",
        "_comment_destination_binding_key",
        "_comment_destination_binding_hash",
        "_comment_evm_rpc_chain_id",
        "_comment_evm_block_tag",
        "_comment_evm_bridge_code_hash",
        "_comment_evm_bridge_runtime_bytecode_hex",
        "_comment_evm_verifier_code_hash",
        "_comment_evm_verifier_runtime_bytecode_hex",
        "_comment_evm_verifier_key_hash",
        "_comment_evm_verifier_backend_hash",
        "_comment_evm_proof_family_hash",
        "_comment_solana_rpc_commitment",
        "_comment_solana_program_owner",
        "_comment_solana_programdata_owner",
        "_comment_solana_program_immutable",
        "_comment_solana_program_account_data_len",
        "_comment_solana_program_account_data_base64",
        "_comment_solana_programdata_address",
        "_comment_solana_programdata_slot",
        "_comment_solana_expected_programdata_slot",
        "_comment_solana_program_account_context_slot",
        "_comment_solana_programdata_account_context_slot",
        "_comment_solana_programdata_metadata_blake2b256",
        "_comment_solana_programdata_metadata_base64",
        "_comment_solana_programdata_code_hash",
        "_comment_solana_programdata_executable_base64",
        "_comment_ton_account_status",
        "_comment_ton_account_state_hash",
        "_comment_ton_last_transaction_lt",
        "_comment_ton_last_transaction_hash",
        "_comment_ton_code_hash",
        "_comment_ton_code_boc_root_hash",
        "_comment_ton_code_boc_base64",
        "_comment_ton_code_boc_hash_matches",
        "_comment_tron_destination_verifier_address",
        "_comment_tron_destination_verifier_code_hash",
        "_comment_tron_destination_verifier_runtime_bytecode_hex",
        "_comment_tron_destination_verifier_key_hash",
        "_comment_tron_destination_verifier_backend_hash",
        "_comment_tron_destination_proof_family_hash",
    )
)
ROUTE_ALLOWLIST_FIELDS = frozenset(
    (
        "version",
        "domain",
        "chain",
        "activation_policy",
        "route_allowlist_id",
        "route_allowlist_hash",
        "route_canary_status",
        "route_canary_evidence_hash",
        "route_canary_route_allowlist_hash",
        "route_canary_destination_binding_hash",
        "evm_route_canary_transaction_hash",
        "evm_route_canary_transaction_block_number",
        "evm_route_canary_transaction_block_hash",
        "evm_route_canary_log_index",
        "evm_route_canary_receipt_block_number",
        "evm_route_canary_receipt_block_hash",
        "evm_route_canary_block_receipts_root",
        "evm_route_canary_call_data_sha256",
        "evm_route_canary_message_id",
        "evm_route_canary_payload_hash",
        "evm_route_canary_target_domain",
        "evm_route_canary_statement_hash",
        "evm_route_canary_commitment_root",
        "evm_route_canary_finality_height",
        "evm_route_canary_finality_block_hash",
        "evm_route_canary_proof_version",
        "evm_route_canary_proof_source_domain",
        "evm_route_canary_used_message_proof",
        "evm_route_canary_receipt_block_finalized",
        "tron_route_canary_transaction_id",
        "tron_route_canary_transaction_owner_address",
        "tron_route_canary_block_number",
        "tron_route_canary_block_timestamp",
        "tron_route_canary_log_index",
        "tron_route_canary_message_id",
        "tron_route_canary_call_data_sha256",
        "tron_route_canary_payload_hash",
        "tron_route_canary_target_domain",
        "tron_route_canary_statement_hash",
        "tron_route_canary_commitment_root",
        "tron_route_canary_finality_height",
        "tron_route_canary_finality_block_hash",
        "tron_route_canary_proof_version",
        "tron_route_canary_proof_source_domain",
        "tron_route_canary_used_message_proof",
        "tron_route_canary_raw_data_owner_matches_transaction",
        "tron_route_canary_signature_sha256",
        "tron_route_canary_signature_recovered_address",
        "tron_route_canary_signature_recovers_to_owner",
        "ton_route_canary_account_state_hash",
        "ton_route_canary_last_transaction_lt",
        "ton_route_canary_last_transaction_hash",
        "routes_allowlisted",
        "blockers",
        "_comment_route_canary_status",
        "_comment_route_canary_evidence_hash",
        "_comment_route_canary_route_allowlist_hash",
        "_comment_route_canary_destination_binding_hash",
        "_comment_evm_route_canary_transaction_hash",
        "_comment_evm_route_canary_transaction_block_number",
        "_comment_evm_route_canary_transaction_block_hash",
        "_comment_evm_route_canary_log_index",
        "_comment_evm_route_canary_receipt_block_number",
        "_comment_evm_route_canary_receipt_block_hash",
        "_comment_evm_route_canary_block_receipts_root",
        "_comment_evm_route_canary_call_data_sha256",
        "_comment_evm_route_canary_message_id",
        "_comment_evm_route_canary_payload_hash",
        "_comment_evm_route_canary_target_domain",
        "_comment_evm_route_canary_statement_hash",
        "_comment_evm_route_canary_commitment_root",
        "_comment_evm_route_canary_finality_height",
        "_comment_evm_route_canary_finality_block_hash",
        "_comment_evm_route_canary_proof_version",
        "_comment_evm_route_canary_proof_source_domain",
        "_comment_evm_route_canary_used_message_proof",
        "_comment_evm_route_canary_receipt_block_finalized",
        "_comment_ton_route_canary_account_state_hash",
        "_comment_ton_route_canary_last_transaction_lt",
        "_comment_ton_route_canary_last_transaction_hash",
        "_comment_tron_route_canary_transaction_id",
        "_comment_tron_route_canary_transaction_owner_address",
        "_comment_tron_route_canary_block_number",
        "_comment_tron_route_canary_block_timestamp",
        "_comment_tron_route_canary_log_index",
        "_comment_tron_route_canary_message_id",
        "_comment_tron_route_canary_call_data_sha256",
        "_comment_tron_route_canary_payload_hash",
        "_comment_tron_route_canary_target_domain",
        "_comment_tron_route_canary_statement_hash",
        "_comment_tron_route_canary_commitment_root",
        "_comment_tron_route_canary_finality_height",
        "_comment_tron_route_canary_finality_block_hash",
        "_comment_tron_route_canary_proof_version",
        "_comment_tron_route_canary_proof_source_domain",
        "_comment_tron_route_canary_used_message_proof",
        "_comment_tron_route_canary_raw_data_owner_matches_transaction",
        "_comment_tron_route_canary_signature_sha256",
        "_comment_tron_route_canary_signature_recovered_address",
        "_comment_tron_route_canary_signature_recovers_to_owner",
    )
)

DESTINATION_ROLLOUT_COMMENT_KEYS = {
    "sccp_evm_block_tag": "_comment_evm_block_tag",
    "sccp_evm_destination_network_id": "_comment_destination_network_id",
    "sccp_evm_destination_bridge_address": "_comment_destination_bridge_address",
    "sccp_evm_destination_binding_key": "_comment_destination_binding_key",
    "sccp_evm_destination_binding_hash": "_comment_destination_binding_hash",
    "sccp_evm_rpc_chain_id": "_comment_evm_rpc_chain_id",
    "sccp_evm_bridge_runtime_code_hash": "_comment_evm_bridge_code_hash",
    "sccp_evm_bridge_runtime_bytecode_hex": "_comment_evm_bridge_runtime_bytecode_hex",
    "sccp_evm_verifier_runtime_code_hash": "_comment_evm_verifier_code_hash",
    "sccp_evm_verifier_runtime_bytecode_hex": (
        "_comment_evm_verifier_runtime_bytecode_hex"
    ),
    "sccp_evm_verifier_key_hash": "_comment_evm_verifier_key_hash",
    "sccp_evm_verifier_backend_hash": "_comment_evm_verifier_backend_hash",
    "sccp_evm_proof_family_hash": "_comment_evm_proof_family_hash",
    "sccp_solana_destination_binding_hash": "_comment_destination_binding_hash",
    "sccp_solana_rpc_commitment": "_comment_solana_rpc_commitment",
    "sccp_solana_program_owner": "_comment_solana_program_owner",
    "sccp_solana_programdata_owner": "_comment_solana_programdata_owner",
    "sccp_solana_program_immutable": "_comment_solana_program_immutable",
    "sccp_solana_program_account_data_len": (
        "_comment_solana_program_account_data_len"
    ),
    "sccp_solana_program_account_data_base64": (
        "_comment_solana_program_account_data_base64"
    ),
    "sccp_solana_programdata_address": "_comment_solana_programdata_address",
    "sccp_solana_programdata_slot": "_comment_solana_programdata_slot",
    "sccp_solana_expected_programdata_slot": "_comment_solana_expected_programdata_slot",
    "sccp_solana_program_account_context_slot": (
        "_comment_solana_program_account_context_slot"
    ),
    "sccp_solana_programdata_account_context_slot": (
        "_comment_solana_programdata_account_context_slot"
    ),
    "sccp_solana_programdata_metadata_blake2b256": (
        "_comment_solana_programdata_metadata_blake2b256"
    ),
    "sccp_solana_programdata_metadata_base64": (
        "_comment_solana_programdata_metadata_base64"
    ),
    "sccp_solana_programdata_executable_blake2b256": (
        "_comment_solana_programdata_code_hash"
    ),
    "sccp_solana_programdata_executable_base64": (
        "_comment_solana_programdata_executable_base64"
    ),
    "sccp_ton_destination_binding_hash": "_comment_destination_binding_hash",
    "sccp_ton_account_status": "_comment_ton_account_status",
    "sccp_ton_account_state_hash": "_comment_ton_account_state_hash",
    "sccp_ton_last_transaction_lt": "_comment_ton_last_transaction_lt",
    "sccp_ton_last_transaction_hash": "_comment_ton_last_transaction_hash",
    "sccp_ton_code_hash": "_comment_ton_code_hash",
    "sccp_ton_code_boc_root_hash": "_comment_ton_code_boc_root_hash",
    "sccp_ton_code_boc_base64": "_comment_ton_code_boc_base64",
    "sccp_ton_code_boc_hash_matches": "_comment_ton_code_boc_hash_matches",
    "sccp_tron_destination_binding_hash": "_comment_destination_binding_hash",
    "sccp_tron_destination_binding_key": "_comment_destination_binding_key",
    "sccp_tron_destination_verifier_address": (
        "_comment_tron_destination_verifier_address"
    ),
    "sccp_tron_destination_verifier_runtime_code_hash": (
        "_comment_tron_destination_verifier_code_hash"
    ),
    "sccp_tron_destination_verifier_runtime_bytecode_hex": (
        "_comment_tron_destination_verifier_runtime_bytecode_hex"
    ),
    "sccp_tron_destination_verifier_key_hash": (
        "_comment_tron_destination_verifier_key_hash"
    ),
    "sccp_tron_destination_verifier_backend_hash": (
        "_comment_tron_destination_verifier_backend_hash"
    ),
    "sccp_tron_destination_proof_family_hash": (
        "_comment_tron_destination_proof_family_hash"
    ),
}
SOURCE_RECORD_COMMENT_KEYS = {
    "sccp_eth_source_verifier_material_hash": "_comment_source_verifier_material_hash",
    "sccp_bsc_source_verifier_material_hash": "_comment_source_verifier_material_hash",
    "sccp_solana_source_verifier_material_hash": (
        "_comment_source_verifier_material_hash"
    ),
    "sccp_ton_source_verifier_material_hash": "_comment_source_verifier_material_hash",
    "sccp_tron_source_verifier_material_hash": "_comment_source_verifier_material_hash",
    "sccp_evm_source_rpc_chain_id": "_comment_evm_source_rpc_chain_id",
    "sccp_evm_source_block_tag": "_comment_evm_source_block_tag",
    "sccp_evm_source_bridge_address": "_comment_evm_source_bridge_address",
    "sccp_evm_source_bridge_runtime_code_hash": (
        "_comment_evm_source_bridge_code_hash"
    ),
    "sccp_evm_source_bridge_runtime_bytecode_hex": (
        "_comment_evm_source_bridge_runtime_bytecode_hex"
    ),
    "sccp_eth_source_bridge_network_id": "_comment_eth_source_bridge_network_id",
    "sccp_eth_source_bridge_config_hash": "_comment_eth_source_bridge_config_hash",
    "sccp_evm_source_deployment_transaction_hash": (
        "_comment_evm_source_deployment_transaction_hash"
    ),
    "sccp_evm_source_deployment_transaction_block_hash": (
        "_comment_evm_source_deployment_transaction_block_hash"
    ),
    "sccp_evm_source_deployment_transaction_block_number": (
        "_comment_evm_source_deployment_transaction_block_number"
    ),
    "sccp_evm_source_deployment_transaction_input_sha256": (
        "_comment_evm_source_deployment_transaction_input_sha256"
    ),
    "sccp_evm_source_deployment_receipt_status": (
        "_comment_evm_source_deployment_receipt_status"
    ),
    "sccp_evm_source_deployment_contract_address": (
        "_comment_evm_source_deployment_contract_address"
    ),
    "sccp_evm_source_deployment_block_hash": (
        "_comment_evm_source_deployment_block_hash"
    ),
    "sccp_evm_source_deployment_block_number": (
        "_comment_evm_source_deployment_block_number"
    ),
    "sccp_evm_source_deployment_block_receipts_root": (
        "_comment_evm_source_deployment_block_receipts_root"
    ),
    "sccp_tron_source_bridge_address": "_comment_tron_source_bridge_address",
    "sccp_tron_source_bridge_runtime_code_hash": (
        "_comment_tron_source_bridge_code_hash"
    ),
    "sccp_tron_source_bridge_runtime_bytecode_hex": (
        "_comment_tron_source_bridge_runtime_bytecode_hex"
    ),
    "sccp_tron_source_bridge_config_hash": (
        "_comment_tron_source_bridge_config_hash"
    ),
}
SOURCE_DEPLOYMENT_COMMENT_KEYS = {
    "sccp_eth_source_adapter_engine_deployment_hash": (
        "_comment_source_adapter_engine_deployment_hash"
    ),
    "sccp_bsc_source_adapter_engine_deployment_hash": (
        "_comment_source_adapter_engine_deployment_hash"
    ),
    "sccp_solana_source_adapter_engine_deployment_hash": (
        "_comment_source_adapter_engine_deployment_hash"
    ),
    "sccp_ton_source_adapter_engine_deployment_hash": (
        "_comment_source_adapter_engine_deployment_hash"
    ),
    "sccp_tron_source_adapter_engine_deployment_hash": (
        "_comment_source_adapter_engine_deployment_hash"
    ),
}

ROUTE_ALLOWLIST_COMMENT_KEYS = {
    "sccp_route_canary_status": "_comment_route_canary_status",
    "sccp_route_canary_evidence_hash": "_comment_route_canary_evidence_hash",
    "sccp_route_canary_route_allowlist_hash": (
        "_comment_route_canary_route_allowlist_hash"
    ),
    "sccp_route_canary_destination_binding_hash": (
        "_comment_route_canary_destination_binding_hash"
    ),
    "sccp_evm_route_canary_transaction_hash": (
        "_comment_evm_route_canary_transaction_hash"
    ),
    "sccp_evm_route_canary_transaction_block_number": (
        "_comment_evm_route_canary_transaction_block_number"
    ),
    "sccp_evm_route_canary_transaction_block_hash": (
        "_comment_evm_route_canary_transaction_block_hash"
    ),
    "sccp_evm_route_canary_log_index": "_comment_evm_route_canary_log_index",
    "sccp_evm_route_canary_receipt_block_number": (
        "_comment_evm_route_canary_receipt_block_number"
    ),
    "sccp_evm_route_canary_receipt_block_hash": (
        "_comment_evm_route_canary_receipt_block_hash"
    ),
    "sccp_evm_route_canary_block_receipts_root": (
        "_comment_evm_route_canary_block_receipts_root"
    ),
    "sccp_evm_route_canary_call_data_sha256": (
        "_comment_evm_route_canary_call_data_sha256"
    ),
    "sccp_evm_route_canary_message_id": "_comment_evm_route_canary_message_id",
    "sccp_evm_route_canary_payload_hash": "_comment_evm_route_canary_payload_hash",
    "sccp_evm_route_canary_target_domain": "_comment_evm_route_canary_target_domain",
    "sccp_evm_route_canary_statement_hash": (
        "_comment_evm_route_canary_statement_hash"
    ),
    "sccp_evm_route_canary_commitment_root": (
        "_comment_evm_route_canary_commitment_root"
    ),
    "sccp_evm_route_canary_finality_height": (
        "_comment_evm_route_canary_finality_height"
    ),
    "sccp_evm_route_canary_finality_block_hash": (
        "_comment_evm_route_canary_finality_block_hash"
    ),
    "sccp_evm_route_canary_proof_version": (
        "_comment_evm_route_canary_proof_version"
    ),
    "sccp_evm_route_canary_proof_source_domain": (
        "_comment_evm_route_canary_proof_source_domain"
    ),
    "sccp_evm_route_canary_used_message_proof": (
        "_comment_evm_route_canary_used_message_proof"
    ),
    "sccp_evm_route_canary_receipt_block_finalized": (
        "_comment_evm_route_canary_receipt_block_finalized"
    ),
    "sccp_ton_route_canary_account_state_hash": (
        "_comment_ton_route_canary_account_state_hash"
    ),
    "sccp_ton_route_canary_last_transaction_lt": (
        "_comment_ton_route_canary_last_transaction_lt"
    ),
    "sccp_ton_route_canary_last_transaction_hash": (
        "_comment_ton_route_canary_last_transaction_hash"
    ),
    "sccp_tron_route_canary_transaction_id": (
        "_comment_tron_route_canary_transaction_id"
    ),
    "sccp_tron_route_canary_transaction_owner_address": (
        "_comment_tron_route_canary_transaction_owner_address"
    ),
    "sccp_tron_route_canary_block_number": (
        "_comment_tron_route_canary_block_number"
    ),
    "sccp_tron_route_canary_block_timestamp": (
        "_comment_tron_route_canary_block_timestamp"
    ),
    "sccp_tron_route_canary_log_index": "_comment_tron_route_canary_log_index",
    "sccp_tron_route_canary_message_id": "_comment_tron_route_canary_message_id",
    "sccp_tron_route_canary_call_data_sha256": (
        "_comment_tron_route_canary_call_data_sha256"
    ),
    "sccp_tron_route_canary_payload_hash": "_comment_tron_route_canary_payload_hash",
    "sccp_tron_route_canary_target_domain": "_comment_tron_route_canary_target_domain",
    "sccp_tron_route_canary_statement_hash": (
        "_comment_tron_route_canary_statement_hash"
    ),
    "sccp_tron_route_canary_commitment_root": (
        "_comment_tron_route_canary_commitment_root"
    ),
    "sccp_tron_route_canary_finality_height": (
        "_comment_tron_route_canary_finality_height"
    ),
    "sccp_tron_route_canary_finality_block_hash": (
        "_comment_tron_route_canary_finality_block_hash"
    ),
    "sccp_tron_route_canary_proof_version": (
        "_comment_tron_route_canary_proof_version"
    ),
    "sccp_tron_route_canary_proof_source_domain": (
        "_comment_tron_route_canary_proof_source_domain"
    ),
    "sccp_tron_route_canary_used_message_proof": (
        "_comment_tron_route_canary_used_message_proof"
    ),
    "sccp_tron_route_canary_raw_data_owner_matches_transaction": (
        "_comment_tron_route_canary_raw_data_owner_matches_transaction"
    ),
    "sccp_tron_route_canary_signature_sha256": (
        "_comment_tron_route_canary_signature_sha256"
    ),
    "sccp_tron_route_canary_signature_recovered_address": (
        "_comment_tron_route_canary_signature_recovered_address"
    ),
    "sccp_tron_route_canary_signature_recovers_to_owner": (
        "_comment_tron_route_canary_signature_recovers_to_owner"
    ),
}

_SIBLING_MODULES: dict[str, Any] = {}


def _strip_0x(value: str) -> str:
    return value[2:] if value.lower().startswith("0x") else value


def _canonical_hex_text(value: str) -> str | None:
    if value.startswith("0X"):
        return None
    text = value[2:] if value.startswith("0x") else value
    if text != text.lower():
        return None
    return text


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _push_u8(out: bytearray, value: int) -> None:
    out.append(value)


def _push_u32(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(4, "little"))


def _push_u64(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(8, "little"))


def _push_vec(out: bytearray, value: bytes) -> None:
    _push_u32(out, len(value))
    out.extend(value)


def _prefixed_blake2b(prefix: bytes, payload: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    hasher.update(prefix)
    hasher.update(payload)
    return hasher.digest()


def _hex_bytes(value: Any, *, byte_length: int) -> bytes | None:
    if not isinstance(value, str) or value != value.strip():
        return None
    text = _canonical_hex_text(value)
    if text is None:
        return None
    if any(symbol.isspace() for symbol in text):
        return None
    if len(text) != byte_length * 2:
        return None
    try:
        return bytes.fromhex(text)
    except ValueError:
        return None


def _required_hex_bytes(record: dict[str, Any], field: str, *, byte_length: int) -> bytes:
    raw = _hex_bytes(record.get(field), byte_length=byte_length)
    if raw is None:
        raise ValueError(f"{field} must be a {byte_length}-byte hex value")
    return raw


def _exact_hex_bytes(value: Any, *, byte_length: int) -> bytes | None:
    if not isinstance(value, str) or value != value.strip():
        return None
    text = _canonical_hex_text(value)
    if text is None:
        return None
    if any(symbol.isspace() for symbol in text):
        return None
    if len(text) != byte_length * 2:
        return None
    try:
        return bytes.fromhex(text)
    except ValueError:
        return None


def _required_exact_hex_bytes(
    record: dict[str, Any],
    field: str,
    *,
    byte_length: int,
) -> bytes:
    raw = _exact_hex_bytes(record.get(field), byte_length=byte_length)
    if raw is None:
        raise ValueError(f"{field} must be an exact {byte_length}-byte hex value")
    return raw


def _parse_exact_runtime_bytecode(
    module: Any,
    value: Any,
    *,
    label: str,
) -> bytes:
    if not isinstance(value, str):
        raise argparse.ArgumentTypeError(f"{label} must be hex")
    if value != value.strip():
        raise argparse.ArgumentTypeError(
            f"{label} must not contain surrounding whitespace"
        )
    if value.startswith("0X"):
        raise argparse.ArgumentTypeError(f"{label} must use lowercase 0x prefix")
    text = value[2:] if value.startswith("0x") else value
    if text != text.lower():
        raise argparse.ArgumentTypeError(f"{label} must use lowercase hex")
    if any(symbol.isspace() for symbol in text):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    return module.parse_runtime_bytecode_hex(value, label=label)


def _nonzero_hex(value: Any, *, byte_length: int) -> bool:
    raw = _hex_bytes(value, byte_length=byte_length)
    return raw is not None and any(raw)


def _empty_hex_or_absent(value: Any, *, byte_length: int) -> bool:
    if value in (None, ""):
        return True
    raw = _hex_bytes(value, byte_length=byte_length)
    return raw is not None and not any(raw)


def _is_nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _decode_canonical_base64(value: str, *, label: str) -> bytes:
    try:
        raw = base64.b64decode(value, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise ValueError(f"{label} must be base64") from exc
    if base64.b64encode(raw).decode("ascii") != value:
        raise ValueError(f"{label} must be canonical base64")
    return raw


def _blocker_list_errors(record: dict[str, Any], label: str) -> list[str]:
    blockers = record.get("blockers", [])
    if not isinstance(blockers, list):
        return [f"{label} blockers must be a list of non-empty canonical strings"]
    errors: list[str] = []
    for index, blocker in enumerate(blockers):
        if (
            not isinstance(blocker, str)
            or not blocker
            or blocker.strip() != blocker
        ):
            errors.append(
                f"{label} blockers[{index}] must be a non-empty canonical string"
            )
    if blockers:
        errors.append(f"{label} blockers must be empty")
    return errors


def _canonical_blocker_list(
    value: Any,
    label: str,
) -> tuple[list[str], list[str]]:
    if not isinstance(value, list):
        return [], [f"{label} blockers must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    errors: list[str] = []
    for index, blocker in enumerate(value):
        if (
            not isinstance(blocker, str)
            or not blocker
            or blocker.strip() != blocker
        ):
            errors.append(
                f"{label} blockers[{index}] must be a non-empty canonical string"
            )
        else:
            blockers.append(blocker)
    return blockers, errors


def _load_sibling_module(filename: str) -> Any:
    cached = _SIBLING_MODULES.get(filename)
    if cached is not None:
        return cached
    path = Path(__file__).with_name(filename)
    module_name = f"_sccp_all_lanes_{filename.removesuffix('.py')}"
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    _SIBLING_MODULES[filename] = module
    return module


def _expect(errors: list[str], record: dict[str, Any], field: str, expected: Any) -> None:
    if record.get(field) != expected:
        errors.append(f"{field} must be {expected!r}")


def _expect_nonzero_hex(
    errors: list[str],
    record: dict[str, Any],
    field: str,
    *,
    byte_length: int = 32,
) -> None:
    if not _nonzero_hex(record.get(field), byte_length=byte_length):
        errors.append(f"{field} must be a non-zero {byte_length}-byte hex value")


def _expect_empty_hex_or_absent(
    errors: list[str],
    record: dict[str, Any],
    field: str,
    *,
    byte_length: int = 32,
) -> None:
    if not _empty_hex_or_absent(record.get(field), byte_length=byte_length):
        errors.append(f"{field} must be empty for this lane")


def _expect_role_hash_fields_are_distinct(
    errors: list[str],
    record: dict[str, Any],
    fields: tuple[str, ...],
    *,
    label: str,
) -> None:
    seen: dict[bytes, str] = {}
    for field in fields:
        raw = _hex_bytes(record.get(field), byte_length=32)
        if raw is None or not any(raw):
            continue
        previous_field = seen.get(raw)
        if previous_field is not None:
            errors.append(f"{label} {field} must not reuse {previous_field}")
        else:
            seen[raw] = field


def _expect_distinct_byte_values(
    errors: list[str],
    fields: tuple[tuple[str, bytes | None], ...],
    *,
    label: str,
) -> None:
    seen: dict[bytes, str] = {}
    for field, raw in fields:
        if raw is None or not any(raw):
            continue
        previous_field = seen.get(raw)
        if previous_field is not None:
            errors.append(f"{label} {field} must not reuse {previous_field}")
        else:
            seen[raw] = field


def _reject_unknown_fields(
    errors: list[str],
    record: dict[str, Any],
    allowed_fields: frozenset[str],
) -> None:
    for field in sorted(record):
        if field not in allowed_fields:
            errors.append(f"unexpected field {field}")


def _reject_lane_foreign_fields(
    errors: list[str],
    record: dict[str, Any],
    fields: tuple[str, ...],
    *,
    actual_chain: str,
    allowed_chain: str | None = None,
    allowed_chains: tuple[str, ...] | None = None,
    label: str,
) -> None:
    allowed = allowed_chains if allowed_chains is not None else (allowed_chain,)
    if actual_chain in allowed:
        return
    for field in fields:
        if record.get(field) not in (None, ""):
            errors.append(f"{field} is only valid for {label}")


def _records_by_domain(
    records: Any,
    domain_field: str,
    *,
    allowed_domains: set[int] | None = None,
) -> tuple[dict[int, dict[str, Any]], list[str]]:
    by_domain: dict[int, dict[str, Any]] = {}
    errors: list[str] = []
    if records is None:
        return by_domain, errors
    if not isinstance(records, list):
        return by_domain, ["records must be a list"]
    seen: dict[int, int] = {}
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            errors.append(f"record {index} must be a table")
            continue
        domain = record.get(domain_field)
        if type(domain) is not int:
            errors.append(f"record {index} missing integer {domain_field}")
            continue
        if allowed_domains is not None and domain not in allowed_domains:
            errors.append(f"record {index} uses unsupported {domain_field} {domain}")
            continue
        seen[domain] = seen.get(domain, 0) + 1
        by_domain.setdefault(domain, record)
    for domain, count in sorted(seen.items()):
        if count > 1:
            errors.append(f"duplicate records for domain {domain}")
    return by_domain, errors


def _load_toml(text: str, *, label: str) -> dict[str, Any]:
    if tomllib is None:
        return _load_toml_minimal(text, label=label)
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as exc:  # type: ignore[union-attr]
        raise ValueError(f"{label}: invalid TOML") from exc


def _parse_minimal_toml_value(value: str, *, label: str, line_number: int) -> Any:
    text = value.strip()
    if text in ("true", "false"):
        return text == "true"
    if text.startswith('"'):
        try:
            return json.loads(text)
        except json.JSONDecodeError as exc:
            raise ValueError(f"{label}:{line_number}: invalid string") from exc
    if text.startswith("["):
        try:
            parsed = json.loads(text)
        except json.JSONDecodeError as exc:
            raise ValueError(f"{label}:{line_number}: invalid array") from exc
        if not isinstance(parsed, list) or not all(
            isinstance(item, str) for item in parsed
        ):
            raise ValueError(f"{label}:{line_number}: only string arrays are supported")
        return parsed
    digits = text[1:] if text.startswith("-") else text
    if (
        not digits
        or not digits.isascii()
        or not digits.isdecimal()
        or text.startswith("+")
        or (text.startswith("-") and digits == "0")
        or (len(digits) > 1 and digits.startswith("0"))
    ):
        raise ValueError(f"{label}:{line_number}: unsupported TOML value")
    return int(text, 10)


def _is_canonical_decimal_text(value: object, *, positive: bool) -> bool:
    if not isinstance(value, str):
        return False
    if not value or not value.isascii() or not value.isdecimal():
        return False
    if len(value) > 1 and value.startswith("0"):
        return False
    return not positive or int(value, 10) > 0


def _load_toml_minimal(text: str, *, label: str) -> dict[str, Any]:
    document: dict[str, Any] = {"zk": {name: [] for name in SECTION_NAMES}}
    current: dict[str, Any] | None = None
    for line_number, raw_line in enumerate(text.splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[[") and line.endswith("]]"):
            section = line[2:-2].strip()
            if not section.startswith("zk."):
                raise ValueError(f"{label}:{line_number}: expected [[zk.*]] section")
            name = section.removeprefix("zk.")
            if name not in SECTION_NAMES:
                raise ValueError(f"{label}:{line_number}: unsupported zk section {name}")
            current = {}
            document["zk"][name].append(current)
            continue
        if current is None:
            raise ValueError(f"{label}:{line_number}: key appears before a section")
        if "=" not in line:
            raise ValueError(f"{label}:{line_number}: expected key = value")
        key, value = line.split("=", 1)
        key = key.strip()
        if key in current:
            raise ValueError(f"{label}:{line_number}: duplicate key {key}")
        current[key] = _parse_minimal_toml_value(
            value,
            label=label,
            line_number=line_number,
        )
    return document


def _comment_toml_value(value: str, *, label: str, line_number: int) -> str:
    try:
        parsed = json.loads(value.strip())
    except json.JSONDecodeError as exc:
        raise ValueError(f"{label}:{line_number}: invalid metadata comment") from exc
    if not isinstance(parsed, str):
        raise ValueError(f"{label}:{line_number}: metadata comment must be a string")
    return parsed


def _section_comment_metadata(
    text: str,
    *,
    label: str,
    section_header: str,
    comment_keys: dict[str, str],
) -> list[dict[str, str]]:
    metadata: list[dict[str, str]] = []
    pending: dict[str, str] = {}
    for line_number, raw_line in enumerate(text.splitlines(), start=1):
        line = raw_line.strip()
        if not line:
            continue
        if line.startswith("#"):
            comment = line[1:].strip()
            if "=" not in comment:
                continue
            key, value = comment.split("=", 1)
            field = comment_keys.get(key.strip())
            if field is not None:
                if field in pending:
                    raise ValueError(
                        f"{label}:{line_number}: duplicate metadata comment "
                        f"for {field}"
                    )
                pending[field] = _comment_toml_value(
                    value,
                    label=label,
                    line_number=line_number,
                )
            continue
        if line == section_header:
            metadata.append(pending)
            pending = {}
            continue
        if line.startswith("[["):
            pending = {}
            continue
        if pending:
            pending = {}
    return metadata


def _destination_rollout_comment_metadata(
    text: str,
    *,
    label: str,
) -> list[dict[str, str]]:
    return _section_comment_metadata(
        text,
        label=label,
        section_header="[[zk.sccp_destination_rollouts]]",
        comment_keys=DESTINATION_ROLLOUT_COMMENT_KEYS,
    )


def _source_material_comment_metadata(
    text: str,
    *,
    label: str,
) -> list[dict[str, str]]:
    return _section_comment_metadata(
        text,
        label=label,
        section_header="[[zk.sccp_source_verifier_materials]]",
        comment_keys=SOURCE_RECORD_COMMENT_KEYS,
    )


def _source_deployment_comment_metadata(
    text: str,
    *,
    label: str,
) -> list[dict[str, str]]:
    return _section_comment_metadata(
        text,
        label=label,
        section_header="[[zk.sccp_source_adapter_engine_deployments]]",
        comment_keys=SOURCE_DEPLOYMENT_COMMENT_KEYS,
    )


def _route_allowlist_comment_metadata(
    text: str,
    *,
    label: str,
) -> list[dict[str, str]]:
    return _section_comment_metadata(
        text,
        label=label,
        section_header="[[zk.sccp_route_allowlists]]",
        comment_keys=ROUTE_ALLOWLIST_COMMENT_KEYS,
    )


def load_evidence_bundle(paths: list[Path]) -> dict[str, list[dict[str, Any]]]:
    """Load and merge SCCP evidence TOML snippets."""

    merged: dict[str, list[dict[str, Any]]] = {name: [] for name in SECTION_NAMES}
    for path in paths:
        label = str(path)
        text = path.read_text(encoding="utf-8")
        destination_metadata = _destination_rollout_comment_metadata(text, label=label)
        source_material_metadata = _source_material_comment_metadata(
            text,
            label=label,
        )
        source_deployment_metadata = _source_deployment_comment_metadata(
            text,
            label=label,
        )
        route_allowlist_metadata = _route_allowlist_comment_metadata(
            text,
            label=label,
        )
        document = _load_toml(text, label=label)
        zk = document.get("zk", {})
        if not isinstance(zk, dict):
            raise ValueError(f"{label}: [zk] must be a TOML table")
        for section in sorted(zk):
            if section not in SECTION_NAMES:
                raise ValueError(f"{label}: unsupported zk section {section}")
        for section in SECTION_NAMES:
            records = zk.get(section, [])
            if records is None:
                continue
            if not isinstance(records, list) or not all(
                isinstance(record, dict) for record in records
            ):
                raise ValueError(f"{label}: zk.{section} must be an array of tables")
            if section == "sccp_source_verifier_materials":
                annotated = []
                for index, record in enumerate(records):
                    item = dict(record)
                    if index < len(source_material_metadata):
                        item.update(source_material_metadata[index])
                    annotated.append(item)
                merged[section].extend(annotated)
            elif section == "sccp_source_adapter_engine_deployments":
                annotated = []
                for index, record in enumerate(records):
                    item = dict(record)
                    if index < len(source_deployment_metadata):
                        item.update(source_deployment_metadata[index])
                    annotated.append(item)
                merged[section].extend(annotated)
            elif section == "sccp_destination_rollouts":
                annotated = []
                for index, record in enumerate(records):
                    item = dict(record)
                    if index < len(destination_metadata):
                        item.update(destination_metadata[index])
                    annotated.append(item)
                merged[section].extend(annotated)
            elif section == "sccp_route_allowlists":
                annotated = []
                for index, record in enumerate(records):
                    item = dict(record)
                    if index < len(route_allowlist_metadata):
                        item.update(route_allowlist_metadata[index])
                    annotated.append(item)
                merged[section].extend(annotated)
            else:
                merged[section].extend(records)
    return merged


def _check_source_material(profile: LaneProfile, record: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    _reject_unknown_fields(errors, record, SOURCE_MATERIAL_FIELDS)
    _reject_lane_foreign_fields(
        errors,
        record,
        EVM_SOURCE_BRIDGE_LIVE_COMMENT_FIELDS,
        actual_chain=profile.chain,
        allowed_chains=("eth", "bsc"),
        label="EVM source bridge live evidence",
    )
    _reject_lane_foreign_fields(
        errors,
        record,
        TRON_SOURCE_BRIDGE_LIVE_COMMENT_FIELDS,
        actual_chain=profile.chain,
        allowed_chain="tron",
        label="TRON source bridge live evidence",
    )
    _expect(errors, record, "version", 1)
    _expect(errors, record, "source_domain", profile.domain)
    _expect(errors, record, "source_chain", profile.chain)
    _expect(errors, record, "source_proof_plan", profile.source_proof_plan)
    _expect(errors, record, "finality_model", profile.finality_model)
    _expect(errors, record, "adapter_circuit_id", SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID)
    _expect(errors, record, "source_trust_anchor_id", profile.source_trust_anchor_id)
    _expect(errors, record, "consensus_verifier_id", profile.consensus_verifier_id)
    _expect(
        errors,
        record,
        "message_inclusion_verifier_id",
        profile.message_inclusion_verifier_id,
    )
    _expect(errors, record, "finality_policy_id", profile.finality_policy_id)
    _expect(errors, record, "placeholder_material", False)
    for field in (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "finality_policy_hash",
    ):
        _expect_nonzero_hex(errors, record, field)

    if profile.source_state_verifier_id:
        _expect(
            errors,
            record,
            "source_state_verifier_id",
            profile.source_state_verifier_id,
        )
        _expect_nonzero_hex(errors, record, "source_state_verifier_hash")
    else:
        if record.get("source_state_verifier_id") not in (None, ""):
            errors.append("source_state_verifier_id must be empty for this lane")
        _expect_empty_hex_or_absent(errors, record, "source_state_verifier_hash")

    if profile.source_bridge_emitter_id:
        _expect(
            errors,
            record,
            "source_bridge_emitter_id",
            profile.source_bridge_emitter_id,
        )
        _expect_nonzero_hex(
            errors,
            record,
            "source_bridge_emitter_address",
            byte_length=20,
        )
        _expect_nonzero_hex(errors, record, "source_bridge_emitter_code_hash")
        if profile.chain in ("eth", "bsc"):
            errors.extend(_check_evm_live_source_bridge_evidence(profile, record))
    else:
        if record.get("source_bridge_emitter_id") not in (None, ""):
            errors.append("source_bridge_emitter_id must be empty for this lane")
        _expect_empty_hex_or_absent(
            errors,
            record,
            "source_bridge_emitter_address",
            byte_length=20,
        )
        _expect_empty_hex_or_absent(errors, record, "source_bridge_emitter_code_hash")

    if profile.eth_source_bridge_config_required:
        _expect_nonzero_hex(errors, record, "source_bridge_network_id")
        _expect_empty_hex_or_absent(
            errors,
            record,
            "source_bridge_owner_address",
            byte_length=20,
        )
        _expect_nonzero_hex(errors, record, "source_bridge_config_hash")
        errors.extend(_check_eth_source_bridge_config_hash(material=record))
    elif profile.tron_source_bridge_config_required:
        _expect_nonzero_hex(errors, record, "source_bridge_network_id")
        _expect_nonzero_hex(
            errors,
            record,
            "source_bridge_owner_address",
            byte_length=20,
        )
        _expect_nonzero_hex(errors, record, "source_bridge_config_hash")
        errors.extend(_check_tron_live_source_bridge_evidence(record))
    else:
        _expect_empty_hex_or_absent(errors, record, "source_bridge_network_id")
        _expect_empty_hex_or_absent(
            errors,
            record,
            "source_bridge_owner_address",
            byte_length=20,
        )
        _expect_empty_hex_or_absent(errors, record, "source_bridge_config_hash")
    _expect_role_hash_fields_are_distinct(
        errors,
        record,
        SOURCE_VERIFIER_MATERIAL_ROLE_HASH_FIELDS,
        label="source verifier material role hash",
    )
    return errors


def _check_tron_live_source_bridge_evidence(record: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
    address = record.get("_comment_tron_source_bridge_address")
    material_address = _exact_hex_bytes(
        record.get("source_bridge_emitter_address"),
        byte_length=20,
    )
    try:
        observed_address = (
            module.parse_tron_address(address, label="TRON source bridge")
            if _is_nonempty_string(address)
            else None
        )
    except (argparse.ArgumentTypeError, ValueError):
        errors.append("TRON source bridge address metadata is invalid")
        observed_address = None
    if observed_address is None or not any(observed_address):
        errors.append(
            "TRON source bridge address metadata must be a non-zero "
            "20-byte address"
        )
    elif material_address != observed_address:
        errors.append(
            "TRON source bridge address metadata must match "
            "source_bridge_emitter_address"
        )

    bridge_code_hash = _exact_hex_bytes(
        record.get("_comment_tron_source_bridge_code_hash"),
        byte_length=32,
    )
    material_code_hash = _exact_hex_bytes(
        record.get("source_bridge_emitter_code_hash"),
        byte_length=32,
    )
    if bridge_code_hash is None or not any(bridge_code_hash):
        errors.append(
            "TRON source bridge runtime code hash metadata must be a non-zero "
            "32-byte hex value"
        )
    elif material_code_hash != bridge_code_hash:
        errors.append(
            "TRON source bridge runtime code hash metadata must match "
            "source_bridge_emitter_code_hash"
        )
    bridge_runtime_bytecode = record.get(
        "_comment_tron_source_bridge_runtime_bytecode_hex"
    )
    if not _is_nonempty_string(bridge_runtime_bytecode):
        errors.append("TRON source bridge runtime bytecode metadata must be present")
    else:
        try:
            runtime = _parse_exact_runtime_bytecode(
                module,
                bridge_runtime_bytecode,
                label="TRON source bridge runtime bytecode metadata",
            )
            derived_hash = module.runtime_bytecode_hash(runtime)
        except (argparse.ArgumentTypeError, ValueError):
            errors.append("TRON source bridge runtime bytecode metadata is invalid")
        else:
            if bridge_code_hash is not None and derived_hash != bridge_code_hash:
                errors.append(
                    "TRON source bridge runtime bytecode hash must match "
                    "runtime code hash metadata"
                )
            if material_code_hash is not None and derived_hash != material_code_hash:
                errors.append(
                    "TRON source bridge runtime bytecode hash must match "
                    "source_bridge_emitter_code_hash"
                )

    bridge_config_hash = _exact_hex_bytes(
        record.get("_comment_tron_source_bridge_config_hash"),
        byte_length=32,
    )
    material_config_hash = _exact_hex_bytes(
        record.get("source_bridge_config_hash"),
        byte_length=32,
    )
    if bridge_config_hash is None or not any(bridge_config_hash):
        errors.append(
            "TRON source bridge config hash metadata must be a non-zero "
            "32-byte hex value"
        )
    elif material_config_hash != bridge_config_hash:
        errors.append(
            "TRON source bridge config hash metadata must match "
            "source_bridge_config_hash"
        )
    return errors


def _check_evm_live_source_bridge_evidence(
    profile: LaneProfile,
    record: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    module = _load_sibling_module(
        "sccp_eth_source_bridge_evidence.py"
        if profile.domain == SCCP_DOMAIN_ETH
        else "sccp_bsc_source_bridge_evidence.py"
    )
    rpc_chain_id = record.get("_comment_evm_source_rpc_chain_id")
    expected_chain_id = EVM_EXPECTED_RPC_CHAIN_IDS[profile.domain]
    if not _is_canonical_decimal_text(rpc_chain_id, positive=True):
        errors.append(
            "EVM source live RPC chain-id metadata is required from "
            "sccp_evm_source_live_evidence.py"
        )
    elif int(rpc_chain_id, 10) != expected_chain_id:
        errors.append(
            f"EVM source live RPC chain-id must be canonical for {profile.chain}: "
            f"expected {expected_chain_id}"
        )
    if (
        profile.domain == SCCP_DOMAIN_ETH
        and record.get("_comment_evm_source_block_tag") != "finalized"
    ):
        errors.append("Ethereum source live block-tag metadata must be finalized")

    bridge_address = _hex_bytes(
        record.get("_comment_evm_source_bridge_address"),
        byte_length=20,
    )
    material_address = _hex_bytes(
        record.get("source_bridge_emitter_address"),
        byte_length=20,
    )
    if bridge_address is None or not any(bridge_address):
        errors.append(
            "EVM source bridge address metadata must be a non-zero 20-byte hex value"
        )
    elif material_address != bridge_address:
        errors.append(
            "EVM source bridge address metadata must match "
            "source_bridge_emitter_address"
        )

    bridge_code_hash = _hex_bytes(
        record.get("_comment_evm_source_bridge_code_hash"),
        byte_length=32,
    )
    material_code_hash = _hex_bytes(
        record.get("source_bridge_emitter_code_hash"),
        byte_length=32,
    )
    if bridge_code_hash is None or not any(bridge_code_hash):
        errors.append(
            "EVM source bridge runtime code hash metadata must be a non-zero "
            "32-byte hex value"
        )
    elif material_code_hash != bridge_code_hash:
        errors.append(
            "EVM source bridge runtime code hash metadata must match "
            "source_bridge_emitter_code_hash"
        )
    bridge_runtime_bytecode = record.get(
        "_comment_evm_source_bridge_runtime_bytecode_hex"
    )
    if not _is_nonempty_string(bridge_runtime_bytecode):
        errors.append("EVM source bridge runtime bytecode metadata must be present")
    else:
        try:
            bridge_runtime = _parse_exact_runtime_bytecode(
                module,
                bridge_runtime_bytecode,
                label="EVM source bridge runtime bytecode metadata",
            )
            derived_bridge_code_hash = module.runtime_bytecode_hash(bridge_runtime)
        except (argparse.ArgumentTypeError, ValueError):
            errors.append("EVM source bridge runtime bytecode metadata is invalid")
        else:
            if (
                bridge_code_hash is not None
                and derived_bridge_code_hash != bridge_code_hash
            ):
                errors.append(
                    "EVM source bridge runtime bytecode hash must match "
                    "bridge runtime code hash metadata"
                )
            if (
                material_code_hash is not None
                and derived_bridge_code_hash != material_code_hash
            ):
                errors.append(
                    "EVM source bridge runtime bytecode hash must match "
                    "source_bridge_emitter_code_hash"
                )

    receipt_tx = _hex_bytes(
        record.get("_comment_evm_source_deployment_transaction_hash"),
        byte_length=32,
    )
    if receipt_tx is None or not any(receipt_tx):
        errors.append(
            "EVM source deployment transaction hash metadata must be a non-zero "
            "32-byte hex value"
        )
    transaction_block_hash = _hex_bytes(
        record.get("_comment_evm_source_deployment_transaction_block_hash"),
        byte_length=32,
    )
    if transaction_block_hash is None or not any(transaction_block_hash):
        errors.append(
            "EVM source deployment transaction block hash metadata must be a "
            "non-zero 32-byte hex value"
        )
    transaction_block_number = record.get(
        "_comment_evm_source_deployment_transaction_block_number"
    )
    if not _is_canonical_decimal_text(transaction_block_number, positive=True):
        errors.append(
            "EVM source deployment transaction block number metadata must be a "
            "positive integer"
        )
    transaction_input_sha256 = _hex_bytes(
        record.get("_comment_evm_source_deployment_transaction_input_sha256"),
        byte_length=32,
    )
    if transaction_input_sha256 is None or not any(transaction_input_sha256):
        errors.append(
            "EVM source deployment transaction input SHA-256 metadata must be a "
            "non-zero 32-byte hex value"
        )
    receipt_status = record.get("_comment_evm_source_deployment_receipt_status")
    if receipt_status != "0x1":
        errors.append("EVM source deployment receipt status metadata must be 0x1")
    receipt_contract = record.get("_comment_evm_source_deployment_contract_address")
    contract_address = _hex_bytes(receipt_contract, byte_length=20)
    if contract_address is None or not any(contract_address):
        errors.append(
            "EVM source deployment contract address metadata must be a non-zero "
            "20-byte hex value"
        )
    elif contract_address != material_address:
        errors.append(
            "EVM source deployment contract address metadata must match "
            "source_bridge_emitter_address"
        )
    receipt_block_hash = record.get("_comment_evm_source_deployment_block_hash")
    block_hash = _hex_bytes(receipt_block_hash, byte_length=32)
    if block_hash is None or not any(block_hash):
        errors.append(
            "EVM source deployment block hash metadata must be a non-zero "
            "32-byte hex value"
        )
    elif (
        transaction_block_hash is not None
        and any(transaction_block_hash)
        and transaction_block_hash != block_hash
    ):
        errors.append(
            "EVM source deployment transaction block hash metadata must match "
            "deployment receipt block hash metadata"
        )
    receipt_block_number = record.get("_comment_evm_source_deployment_block_number")
    if not _is_canonical_decimal_text(receipt_block_number, positive=True):
        errors.append(
            "EVM source deployment block number metadata must be a positive integer"
        )
    elif int(receipt_block_number, 10) <= 0:
        errors.append(
            "EVM source deployment block number metadata must be a positive integer"
        )
    elif (
        _is_canonical_decimal_text(transaction_block_number, positive=True)
        and transaction_block_number != receipt_block_number
    ):
        errors.append(
            "EVM source deployment transaction block number metadata must match "
            "deployment receipt block number metadata"
        )
    receipt_block_receipts_root = record.get(
        "_comment_evm_source_deployment_block_receipts_root"
    )
    receipts_root = _hex_bytes(receipt_block_receipts_root, byte_length=32)
    if receipts_root is None or not any(receipts_root):
        errors.append(
            "EVM source deployment block receiptsRoot metadata must be a "
            "non-zero 32-byte hex value"
        )
    return errors


def _check_deployment(
    profile: LaneProfile,
    material: dict[str, Any],
    record: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    _reject_unknown_fields(errors, record, SOURCE_DEPLOYMENT_FIELDS)
    _expect(errors, record, "version", 1)
    _expect(errors, record, "source_domain", profile.domain)
    _expect(errors, record, "target_domain", SCCP_DOMAIN_SORA)
    _expect(errors, record, "adapter_proof_family", SCCP_PROOF_FAMILY_STARK_FRI)
    _expect_nonzero_hex(errors, record, "adapter_verifier_vk_hash")
    _expect_nonzero_hex(errors, record, "deployment_receipt_hash")

    shared_fields = (
        "source_chain",
        "source_proof_plan",
        "finality_model",
        "adapter_circuit_id",
        "source_trust_anchor_id",
        "source_trust_anchor_hash",
        "consensus_verifier_id",
        "consensus_verifier_hash",
        "message_inclusion_verifier_id",
        "message_inclusion_verifier_hash",
        "finality_policy_id",
        "finality_policy_hash",
    )
    for field in shared_fields:
        if record.get(field) != material.get(field):
            errors.append(f"{field} must match source verifier material")

    optional_shared = (
        "source_state_verifier_id",
        "source_state_verifier_hash",
        "source_bridge_emitter_id",
        "source_bridge_emitter_address",
        "source_bridge_emitter_code_hash",
        "source_bridge_network_id",
        "source_bridge_owner_address",
        "source_bridge_config_hash",
    )
    for field in optional_shared:
        material_value = material.get(field, "")
        deployment_value = record.get(field, "")
        if deployment_value != material_value:
            errors.append(f"{field} must match source verifier material")

    if profile.solana_full_light_client_audit_required:
        for field in SOLANA_FULL_LIGHT_CLIENT_AUDIT_FIELDS:
            _expect_nonzero_hex(errors, record, field)
        errors.extend(_check_solana_full_light_client_gate(material, record))
    else:
        for field in SOLANA_FULL_LIGHT_CLIENT_AUDIT_FIELDS:
            _expect_empty_hex_or_absent(errors, record, field)

    if profile.ton_full_light_client_audit_required:
        for field in TON_FULL_LIGHT_CLIENT_AUDIT_FIELDS:
            _expect_nonzero_hex(errors, record, field)
        errors.extend(_check_ton_full_light_client_gate(material, record))
    else:
        for field in TON_FULL_LIGHT_CLIENT_AUDIT_FIELDS:
            _expect_empty_hex_or_absent(errors, record, field)

    if profile.eth_source_bridge_config_required:
        errors.extend(_check_eth_source_bridge_config_hash(material))
    if profile.evm_source_gate_required:
        for field in EVM_SOURCE_GATE_FIELDS:
            _expect_nonzero_hex(errors, record, field)
        errors.extend(_check_evm_source_gate(profile, material, record))
    else:
        for field in EVM_SOURCE_GATE_FIELDS:
            _expect_empty_hex_or_absent(errors, record, field)

    if profile.tron_source_bridge_config_required:
        errors.extend(_check_tron_source_bridge_config_hash(material))
        for field in TRON_DPOS_SOURCE_GATE_FIELDS:
            _expect_nonzero_hex(errors, record, field)
        errors.extend(_check_tron_dpos_source_gate(material, record))
    else:
        for field in TRON_DPOS_SOURCE_GATE_FIELDS:
            _expect_empty_hex_or_absent(errors, record, field)

    deployment_role_hash_fields = SOURCE_ADAPTER_DEPLOYMENT_ROLE_HASH_FIELDS
    if profile.evm_source_gate_required:
        deployment_role_hash_fields += EVM_SOURCE_GATE_ROLE_HASH_FIELDS
    if profile.solana_full_light_client_audit_required:
        deployment_role_hash_fields += SOLANA_FULL_LIGHT_CLIENT_AUDIT_ROLE_HASH_FIELDS
    if profile.ton_full_light_client_audit_required:
        deployment_role_hash_fields += TON_FULL_LIGHT_CLIENT_AUDIT_ROLE_HASH_FIELDS
    _expect_role_hash_fields_are_distinct(
        errors,
        record,
        deployment_role_hash_fields,
        label="source adapter deployment role hash",
    )
    errors.extend(_check_chain_specific_source_evidence(profile, material, record))
    errors.extend(_check_source_record_hash_comments(profile, material, record))
    return errors


def _source_adapter_args(
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> SimpleNamespace:
    return SimpleNamespace(
        source_domain=material["source_domain"],
        target_domain=deployment["target_domain"],
        source_trust_anchor_hash=_required_hex_bytes(
            material,
            "source_trust_anchor_hash",
            byte_length=32,
        ),
        consensus_verifier_hash=_required_hex_bytes(
            material,
            "consensus_verifier_hash",
            byte_length=32,
        ),
        message_inclusion_verifier_hash=_required_hex_bytes(
            material,
            "message_inclusion_verifier_hash",
            byte_length=32,
        ),
        source_state_verifier_hash=_required_hex_bytes(
            material,
            "source_state_verifier_hash",
            byte_length=32,
        ),
        finality_policy_hash=_required_hex_bytes(
            material,
            "finality_policy_hash",
            byte_length=32,
        ),
        adapter_verifier_vk_hash=_required_hex_bytes(
            deployment,
            "adapter_verifier_vk_hash",
            byte_length=32,
        ),
        deployment_receipt_hash=_required_hex_bytes(
            deployment,
            "deployment_receipt_hash",
            byte_length=32,
        ),
    )


def _evm_source_bridge_args(
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> SimpleNamespace:
    return SimpleNamespace(
        source_domain=material["source_domain"],
        target_domain=deployment["target_domain"],
        source_trust_anchor_hash=_required_hex_bytes(
            material,
            "source_trust_anchor_hash",
            byte_length=32,
        ),
        consensus_verifier_hash=_required_hex_bytes(
            material,
            "consensus_verifier_hash",
            byte_length=32,
        ),
        message_inclusion_verifier_hash=_required_hex_bytes(
            material,
            "message_inclusion_verifier_hash",
            byte_length=32,
        ),
        finality_policy_hash=_required_hex_bytes(
            material,
            "finality_policy_hash",
            byte_length=32,
        ),
        bridge_address=_required_hex_bytes(
            material,
            "source_bridge_emitter_address",
            byte_length=20,
        ),
        source_bridge_emitter_code_hash=_required_hex_bytes(
            material,
            "source_bridge_emitter_code_hash",
            byte_length=32,
        ),
        adapter_verifier_vk_hash=_required_hex_bytes(
            deployment,
            "adapter_verifier_vk_hash",
            byte_length=32,
        ),
        deployment_receipt_hash=_required_hex_bytes(
            deployment,
            "deployment_receipt_hash",
            byte_length=32,
        ),
        expected_source_verifier_material_hash=None,
        expected_source_adapter_engine_deployment_hash=None,
    )

def _tron_source_bridge_args(
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> SimpleNamespace:
    return SimpleNamespace(
        source_domain=material["source_domain"],
        target_domain=deployment["target_domain"],
        source_trust_anchor_hash=_required_exact_hex_bytes(
            material,
            "source_trust_anchor_hash",
            byte_length=32,
        ),
        consensus_verifier_hash=_required_exact_hex_bytes(
            material,
            "consensus_verifier_hash",
            byte_length=32,
        ),
        message_inclusion_verifier_hash=_required_exact_hex_bytes(
            material,
            "message_inclusion_verifier_hash",
            byte_length=32,
        ),
        bridge_address=_required_exact_hex_bytes(
            material,
            "source_bridge_emitter_address",
            byte_length=20,
        ),
        source_bridge_emitter_code_hash=_required_exact_hex_bytes(
            material,
            "source_bridge_emitter_code_hash",
            byte_length=32,
        ),
        network_id=_required_exact_hex_bytes(
            material,
            "source_bridge_network_id",
            byte_length=32,
        ),
        owner_address=_required_exact_hex_bytes(
            material,
            "source_bridge_owner_address",
            byte_length=20,
        ),
        finality_policy_hash=_required_exact_hex_bytes(
            material,
            "finality_policy_hash",
            byte_length=32,
        ),
        adapter_verifier_vk_hash=_required_exact_hex_bytes(
            deployment,
            "adapter_verifier_vk_hash",
            byte_length=32,
        ),
        deployment_receipt_hash=_required_exact_hex_bytes(
            deployment,
            "deployment_receipt_hash",
            byte_length=32,
        ),
        expected_source_verifier_material_hash=None,
        expected_source_adapter_engine_deployment_hash=None,
    )


def _chain_specific_source_args(
    profile: LaneProfile,
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> SimpleNamespace:
    if profile.chain in ("eth", "bsc"):
        return _evm_source_bridge_args(material, deployment)
    if profile.chain in ("sol", "ton"):
        return _source_adapter_args(material, deployment)
    if profile.chain == "tron":
        return _tron_source_bridge_args(material, deployment)
    raise ValueError(f"unsupported lane chain {profile.chain!r}")


def _check_chain_specific_source_evidence(
    profile: LaneProfile,
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> list[str]:
    try:
        args = _chain_specific_source_args(profile, material, deployment)
        if profile.chain == "eth":
            module = _load_sibling_module("sccp_eth_source_bridge_evidence.py")
            module._validate_eth_source_evidence_args(args)
        elif profile.chain == "bsc":
            module = _load_sibling_module("sccp_bsc_source_bridge_evidence.py")
            module._validate_bsc_source_evidence_args(args)
        elif profile.chain == "sol":
            module = _load_sibling_module("sccp_solana_source_state_evidence.py")
            _add_solana_light_client_args(args, deployment)
            module._validate_solana_evidence(args)
        elif profile.chain == "ton":
            module = _load_sibling_module("sccp_ton_source_state_evidence.py")
            _add_ton_light_client_args(args, deployment)
            module._validate_ton_source_evidence_args(args)
        elif profile.chain == "tron":
            module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
            module.apply_source_adapter_verifier_vk_hash(args)
            module._require_tron_sora_production_lane(args, "all-lanes")
            module._require_live_source_component_hashes(args)
            config_hash = _required_hex_bytes(
                material,
                "source_bridge_config_hash",
                byte_length=32,
            )
            module.tron_source_verifier_material_record_hash(args, config_hash)
            module.tron_source_adapter_engine_deployment_record_hash(
                args,
                config_hash,
            )
        else:
            return [f"unsupported lane chain {profile.chain!r}"]
    except (SystemExit, ValueError, RuntimeError):
        return [f"{profile.chain} source evidence rejected by canonical validator"]
    return []


def _add_solana_light_client_args(
    args: SimpleNamespace,
    deployment: dict[str, Any],
) -> None:
    args.tower_replay_verifier_hash = _required_hex_bytes(
        deployment,
        "solana_tower_replay_verifier_hash",
        byte_length=32,
    )
    args.full_accountsdb_lattice_verifier_hash = _required_hex_bytes(
        deployment,
        "solana_full_accountsdb_lattice_verifier_hash",
        byte_length=32,
    )
    args.bank_fork_choice_verifier_hash = _required_hex_bytes(
        deployment,
        "solana_bank_fork_choice_verifier_hash",
        byte_length=32,
    )
    args.expected_full_light_client_gate_hash = _required_hex_bytes(
        deployment,
        "solana_full_light_client_gate_hash",
        byte_length=32,
    )


def _add_ton_light_client_args(
    args: SimpleNamespace,
    deployment: dict[str, Any],
) -> None:
    args.masterchain_config_verifier_hash = _required_hex_bytes(
        deployment,
        "ton_masterchain_config_verifier_hash",
        byte_length=32,
    )
    args.validator_set_transition_verifier_hash = _required_hex_bytes(
        deployment,
        "ton_validator_set_transition_verifier_hash",
        byte_length=32,
    )
    args.shard_accounts_dictionary_verifier_hash = _required_hex_bytes(
        deployment,
        "ton_shard_accounts_dictionary_verifier_hash",
        byte_length=32,
    )
    args.expected_full_light_client_gate_hash = _required_hex_bytes(
        deployment,
        "ton_full_light_client_gate_hash",
        byte_length=32,
    )


def _canonical_source_record_hashes(
    profile: LaneProfile,
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> dict[str, str]:
    args = _chain_specific_source_args(profile, material, deployment)
    if profile.chain == "eth":
        module = _load_sibling_module("sccp_eth_source_bridge_evidence.py")
        material_hash = module.eth_source_verifier_material_record_hash(args)
        deployment_hash = module.eth_source_adapter_engine_deployment_record_hash(args)
    elif profile.chain == "bsc":
        module = _load_sibling_module("sccp_bsc_source_bridge_evidence.py")
        material_hash = module.bsc_source_verifier_material_record_hash(args)
        deployment_hash = module.bsc_source_adapter_engine_deployment_record_hash(args)
    elif profile.chain == "sol":
        module = _load_sibling_module("sccp_solana_source_state_evidence.py")
        _add_solana_light_client_args(args, deployment)
        material_hash = module.solana_source_verifier_material_record_hash(args)
        deployment_hash = module.solana_source_adapter_engine_deployment_record_hash(
            args,
        )
    elif profile.chain == "ton":
        module = _load_sibling_module("sccp_ton_source_state_evidence.py")
        _add_ton_light_client_args(args, deployment)
        material_hash = module.ton_source_verifier_material_record_hash(args)
        deployment_hash = module.ton_source_adapter_engine_deployment_record_hash(args)
    elif profile.chain == "tron":
        module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
        config_hash = _required_hex_bytes(
            material,
            "source_bridge_config_hash",
            byte_length=32,
        )
        material_hash = module.tron_source_verifier_material_record_hash(
            args,
            config_hash,
        )
        deployment_hash = module.tron_source_adapter_engine_deployment_record_hash(
            args,
            config_hash,
        )
    else:
        raise ValueError(f"unsupported lane chain {profile.chain!r}")
    return {
        "source_verifier_material_hash": _hex(material_hash),
        "source_adapter_engine_deployment_hash": _hex(deployment_hash),
    }


def route_allowlist_hash_for_lane_evidence(
    profile: LaneProfile,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes:
    """Return the canonical route allowlist hash for exact lane evidence."""

    for label, value in (
        ("source_verifier_material_hash", source_verifier_material_hash),
        (
            "source_adapter_engine_deployment_hash",
            source_adapter_engine_deployment_hash,
        ),
        ("destination_binding_hash", destination_binding_hash),
    ):
        if len(value) != 32 or not any(value):
            raise ValueError(f"{label} must be a non-zero 32-byte value")
    seen_hash_roles: dict[bytes, str] = {}
    for label, value in (
        ("source_verifier_material_hash", source_verifier_material_hash),
        (
            "source_adapter_engine_deployment_hash",
            source_adapter_engine_deployment_hash,
        ),
        ("destination_binding_hash", destination_binding_hash),
    ):
        previous = seen_hash_roles.get(value)
        if previous is not None:
            raise ValueError(
                "route allowlist evidence hashes must be distinct: "
                f"{label} matches {previous}"
            )
        seen_hash_roles[value] = label

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, profile.domain)
    _push_vec(payload, profile.chain.encode("utf-8"))
    _push_vec(payload, b"GovernanceAllowlist")
    _push_vec(payload, profile.route_allowlist_id.encode("utf-8"))
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    payload.extend(destination_binding_hash)
    return _prefixed_blake2b(b"sccp:route-allowlist:lane-evidence:v1", payload)


def _expected_route_allowlist_hash(
    profile: LaneProfile,
    source_record_hashes: dict[str, str],
    destination_binding: dict[str, Any],
) -> bytes:
    source_verifier_material_hash = _hex_bytes(
        source_record_hashes.get("source_verifier_material_hash"),
        byte_length=32,
    )
    if source_verifier_material_hash is None or not any(source_verifier_material_hash):
        raise ValueError(
            "source_verifier_material_hash must be a non-zero 32-byte hex value"
        )
    source_adapter_engine_deployment_hash = _hex_bytes(
        source_record_hashes.get("source_adapter_engine_deployment_hash"),
        byte_length=32,
    )
    if source_adapter_engine_deployment_hash is None or not any(
        source_adapter_engine_deployment_hash
    ):
        raise ValueError(
            "source_adapter_engine_deployment_hash must be a non-zero 32-byte hex value"
        )
    destination_binding_hash = _hex_bytes(
        destination_binding.get("expected_destination_binding_hash"),
        byte_length=32,
    )
    if destination_binding_hash is None or not any(destination_binding_hash):
        raise ValueError(
            "expected_destination_binding_hash must be a non-zero 32-byte hex value"
        )

    return route_allowlist_hash_for_lane_evidence(
        profile,
        source_verifier_material_hash,
        source_adapter_engine_deployment_hash,
        destination_binding_hash,
    )


def _check_evm_source_gate(
    profile: LaneProfile,
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> list[str]:
    try:
        module_name = (
            "sccp_eth_source_bridge_evidence.py"
            if profile.domain == SCCP_DOMAIN_ETH
            else "sccp_bsc_source_bridge_evidence.py"
        )
        module = _load_sibling_module(module_name)
        args = _evm_source_bridge_args(material, deployment)
        expected = (
            module.eth_source_gate_hash(args)
            if profile.domain == SCCP_DOMAIN_ETH
            else module.bsc_source_gate_hash(args)
        )
        configured = _required_hex_bytes(
            deployment,
            "evm_source_gate_hash",
            byte_length=32,
        )
    except (SystemExit, ValueError, RuntimeError):
        return ["EVM source gate cannot be recomputed"]
    if expected != configured:
        return ["evm_source_gate_hash does not match source and deployment material"]
    return []


def _check_solana_full_light_client_gate(
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> list[str]:
    try:
        module = _load_sibling_module("sccp_solana_source_state_evidence.py")
        args = _source_adapter_args(material, deployment)
        args.tower_replay_verifier_hash = _required_hex_bytes(
            deployment,
            "solana_tower_replay_verifier_hash",
            byte_length=32,
        )
        args.full_accountsdb_lattice_verifier_hash = _required_hex_bytes(
            deployment,
            "solana_full_accountsdb_lattice_verifier_hash",
            byte_length=32,
        )
        args.bank_fork_choice_verifier_hash = _required_hex_bytes(
            deployment,
            "solana_bank_fork_choice_verifier_hash",
            byte_length=32,
        )
        expected = module.solana_full_light_client_gate_hash(args)
        configured = _required_hex_bytes(
            deployment,
            "solana_full_light_client_gate_hash",
            byte_length=32,
        )
    except (SystemExit, ValueError, RuntimeError):
        return ["Solana full light-client gate cannot be recomputed"]
    if expected != configured:
        return ["solana_full_light_client_gate_hash does not match source and audit material"]
    return []


def _check_ton_full_light_client_gate(
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> list[str]:
    try:
        module = _load_sibling_module("sccp_ton_source_state_evidence.py")
        args = _source_adapter_args(material, deployment)
        args.masterchain_config_verifier_hash = _required_hex_bytes(
            deployment,
            "ton_masterchain_config_verifier_hash",
            byte_length=32,
        )
        args.validator_set_transition_verifier_hash = _required_hex_bytes(
            deployment,
            "ton_validator_set_transition_verifier_hash",
            byte_length=32,
        )
        args.shard_accounts_dictionary_verifier_hash = _required_hex_bytes(
            deployment,
            "ton_shard_accounts_dictionary_verifier_hash",
            byte_length=32,
        )
        expected = module.ton_full_light_client_gate_hash(args)
        configured = _required_hex_bytes(
            deployment,
            "ton_full_light_client_gate_hash",
            byte_length=32,
        )
    except (SystemExit, ValueError, RuntimeError):
        return ["TON full light-client gate cannot be recomputed"]
    if expected != configured:
        return ["ton_full_light_client_gate_hash does not match source and audit material"]
    return []


def _check_tron_dpos_source_gate(
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> list[str]:
    try:
        module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
        args = _tron_source_bridge_args(material, deployment)
        config_hash = _required_exact_hex_bytes(
            material,
            "source_bridge_config_hash",
            byte_length=32,
        )
        expected = module.tron_dpos_source_gate_hash(args, config_hash)
        configured = _required_exact_hex_bytes(
            deployment,
            "tron_dpos_source_gate_hash",
            byte_length=32,
        )
    except (SystemExit, ValueError, RuntimeError):
        return ["TRON DPoS source gate cannot be recomputed"]
    if expected != configured:
        return ["tron_dpos_source_gate_hash does not match source and deployment material"]
    return []

def _source_adapter_gate_summary(
    profile: LaneProfile,
    material: dict[str, Any] | None,
    deployment: dict[str, Any] | None,
    route_record: dict[str, Any] | None,
    source_record_hashes: dict[str, str],
    destination_binding: dict[str, Any],
    route_allowlist_summary: dict[str, Any],
) -> dict[str, Any]:
    required_fields: tuple[str, ...]
    gate_field: str
    gate_checker: Callable[[dict[str, Any], dict[str, Any]], list[str]]
    if profile.evm_source_gate_required:
        required_fields = EVM_SOURCE_GATE_FIELDS
        gate_field = "evm_source_gate_hash"
        gate_checker = lambda material, deployment: _check_evm_source_gate(
            profile,
            material,
            deployment,
        )
    elif profile.solana_full_light_client_audit_required:
        required_fields = SOLANA_FULL_LIGHT_CLIENT_AUDIT_FIELDS
        gate_field = "solana_full_light_client_gate_hash"
        gate_checker = _check_solana_full_light_client_gate
    elif profile.ton_full_light_client_audit_required:
        required_fields = TON_FULL_LIGHT_CLIENT_AUDIT_FIELDS
        gate_field = "ton_full_light_client_gate_hash"
        gate_checker = _check_ton_full_light_client_gate
    elif profile.tron_source_bridge_config_required:
        required_fields = TRON_DPOS_SOURCE_GATE_FIELDS
        gate_field = "tron_dpos_source_gate_hash"
        gate_checker = _check_tron_dpos_source_gate
    else:
        return {
            "required": False,
            "ready": True,
            "gate_hash": "",
            "audit_hashes": {},
            "blockers": [],
        }

    blockers: list[str] = []
    audit_hashes: dict[str, str] = {}
    gate_hash = ""
    if deployment is not None:
        for field in required_fields:
            value = deployment.get(field)
            if isinstance(value, str):
                audit_hashes[field] = value
            if field == gate_field and isinstance(value, str):
                gate_hash = value
            parsed = _hex_bytes(value, byte_length=32)
            if parsed is None or not any(parsed):
                blockers.append(f"{field} must be a non-zero 32-byte hex value")

    if material is None:
        blockers.append("missing source verifier material")
    if deployment is None:
        blockers.append("missing source adapter deployment")
    if material is not None and deployment is not None and not blockers:
        blockers.extend(gate_checker(material, deployment))
    source_material_hash = source_record_hashes.get("source_verifier_material_hash")
    if source_material_hash is None and material is not None:
        source_material_hash = material.get("_comment_source_verifier_material_hash")
    source_deployment_hash = source_record_hashes.get(
        "source_adapter_engine_deployment_hash"
    )
    if source_deployment_hash is None and deployment is not None:
        source_deployment_hash = deployment.get(
            "_comment_source_adapter_engine_deployment_hash"
        )
    route_canary_evidence_hash = route_allowlist_summary.get("route_canary", {}).get(
        "evidence_hash"
    )
    if route_canary_evidence_hash is None and route_record is not None:
        route_canary_evidence_hash = route_record.get(
            "route_canary_evidence_hash",
            route_record.get("_comment_route_canary_evidence_hash"),
        )
    role_fields: list[tuple[str, bytes | None]] = [
        (
            "source_verifier_material_hash",
            _hex_bytes(
                source_material_hash,
                byte_length=32,
            ),
        ),
        (
            "source_adapter_engine_deployment_hash",
            _hex_bytes(
                source_deployment_hash,
                byte_length=32,
            ),
        ),
        (
            "destination_binding_hash",
            _hex_bytes(
                destination_binding.get("destination_binding_hash"),
                byte_length=32,
            ),
        ),
        (
            "route_allowlist_hash",
            _hex_bytes(
                route_allowlist_summary.get("route_allowlist_hash"),
                byte_length=32,
            ),
        ),
        (
            "route_canary_evidence_hash",
            _hex_bytes(
                route_canary_evidence_hash,
                byte_length=32,
            ),
        ),
    ]
    role_fields.extend(
        (f"audit_hashes.{field}", _hex_bytes(value, byte_length=32))
        for field, value in sorted(audit_hashes.items())
    )
    _expect_distinct_byte_values(
        blockers,
        tuple(role_fields),
        label="source_adapter_gate hash role",
    )

    return {
        "required": True,
        "ready": not blockers,
        "gate_hash": gate_hash,
        "audit_hashes": audit_hashes,
        "blockers": blockers,
    }


def _check_tron_source_bridge_config_hash(material: dict[str, Any]) -> list[str]:
    try:
        module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
        expected = module.tron_source_bridge_config_hash(
            bridge_address=_required_exact_hex_bytes(
                material,
                "source_bridge_emitter_address",
                byte_length=20,
            ),
            network_id=_required_exact_hex_bytes(
                material,
                "source_bridge_network_id",
                byte_length=32,
            ),
            source_domain=material["source_domain"],
            target_domain=SCCP_DOMAIN_SORA,
            owner_address=_required_exact_hex_bytes(
                material,
                "source_bridge_owner_address",
                byte_length=20,
            ),
        )
        configured = _required_exact_hex_bytes(
            material,
            "source_bridge_config_hash",
            byte_length=32,
        )
    except (ValueError, RuntimeError):
        return ["TRON source bridge config hash cannot be recomputed"]
    if expected != configured:
        return ["source_bridge_config_hash does not match TRON bridge address, network id, and owner"]
    return []


def _check_eth_source_bridge_config_hash(material: dict[str, Any]) -> list[str]:
    try:
        module = _load_sibling_module("sccp_eth_source_bridge_evidence.py")
        expected = module.eth_source_bridge_config_hash(
            bridge_address=_required_exact_hex_bytes(
                material,
                "source_bridge_emitter_address",
                byte_length=20,
            ),
            source_bridge_code_hash=_required_exact_hex_bytes(
                material,
                "source_bridge_emitter_code_hash",
                byte_length=32,
            ),
            network_id=_required_exact_hex_bytes(
                material,
                "source_bridge_network_id",
                byte_length=32,
            ),
            source_domain=material["source_domain"],
            target_domain=SCCP_DOMAIN_SORA,
        )
        configured = _required_exact_hex_bytes(
            material,
            "source_bridge_config_hash",
            byte_length=32,
        )
    except (ValueError, RuntimeError):
        return ["ETH source bridge config hash cannot be recomputed"]
    if expected != configured:
        return [
            "source_bridge_config_hash does not match ETH bridge address, "
            "network id, and runtime code hash"
        ]
    return []


def _check_required_hash_comment(
    errors: list[str],
    record: dict[str, Any],
    comment_field: str,
    expected_hash: str,
    *,
    label: str,
) -> None:
    expected = _hex_bytes(expected_hash, byte_length=32)
    observed = _hex_bytes(record.get(comment_field), byte_length=32)
    if observed is None or not any(observed):
        errors.append(f"{label} metadata must be a non-zero 32-byte hex value")
    elif expected is not None and observed != expected:
        errors.append(f"{label} metadata must match the canonical record hash")


def _check_source_record_hash_comments(
    profile: LaneProfile,
    material: dict[str, Any],
    deployment: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    try:
        hashes = _canonical_source_record_hashes(profile, material, deployment)
    except (SystemExit, ValueError, RuntimeError):
        return [f"{profile.chain} source record hash metadata cannot be recomputed"]
    _check_required_hash_comment(
        errors,
        material,
        "_comment_source_verifier_material_hash",
        hashes["source_verifier_material_hash"],
        label="source verifier material hash",
    )
    _check_required_hash_comment(
        errors,
        deployment,
        "_comment_source_adapter_engine_deployment_hash",
        hashes["source_adapter_engine_deployment_hash"],
        label="source adapter deployment hash",
    )
    return errors


def _first_record_value(record: dict[str, Any], *fields: str) -> Any:
    for field in fields:
        value = record.get(field)
        if value not in (None, ""):
            return value
    return None


def _check_hex_comment_matches_record(
    errors: list[str],
    record: dict[str, Any],
    field: str,
    comment_field: str,
    *,
    label: str,
    byte_length: int,
) -> None:
    value = record.get(field)
    comment = record.get(comment_field)
    if value in (None, "") or comment in (None, ""):
        return

    raw = _hex_bytes(value, byte_length=byte_length)
    if raw is None:
        return
    comment_raw = _hex_bytes(comment, byte_length=byte_length)
    if comment_raw is None:
        errors.append(f"{label} comment must be a {byte_length}-byte hex value")
    elif comment_raw != raw:
        errors.append(f"{label} comment must match {field}")


def _check_string_comment_matches_record(
    errors: list[str],
    record: dict[str, Any],
    field: str,
    comment_field: str,
    *,
    label: str,
) -> None:
    value = record.get(field)
    comment = record.get(comment_field)
    if value in (None, "") or comment in (None, ""):
        return
    if not _is_nonempty_string(value):
        return
    if not _is_nonempty_string(comment):
        errors.append(f"{label} comment must be a non-empty string")
    elif comment != value:
        errors.append(f"{label} comment must match {field}")


def _required_metadata_hex_bytes(
    record: dict[str, Any],
    *fields: str,
    label: str,
    byte_length: int,
) -> bytes:
    value = _first_record_value(record, *fields)
    raw = _hex_bytes(value, byte_length=byte_length)
    if raw is None:
        raise ValueError(f"{label} must be a {byte_length}-byte hex value")
    if not any(raw):
        raise ValueError(f"{label} must be non-zero")
    return raw


def _expected_destination_binding(
    profile: LaneProfile,
    material: dict[str, Any] | None,
    destination: dict[str, Any],
) -> dict[str, Any]:
    if profile.chain == "eth" or profile.chain == "bsc":
        module = _load_sibling_module("sccp_evm_destination_evidence.py")
        network_id = _required_metadata_hex_bytes(
            destination,
            "destination_network_id",
            "network_id",
            "_comment_destination_network_id",
            label="destination_network_id",
            byte_length=32,
        )
        bridge_address = _required_metadata_hex_bytes(
            destination,
            "destination_bridge_address",
            "bridge_address",
            "_comment_destination_bridge_address",
            label="destination_bridge_address",
            byte_length=20,
        )
        binding_hash = module.evm_destination_binding_hash(
            network_id=network_id,
            source_domain=SCCP_DOMAIN_SORA,
            target_domain=profile.domain,
            verifier_address=module.parse_evm_address(
                destination["verifier_identity"],
                label="verifier_identity",
            ),
            bridge_address=bridge_address,
            verifier_code_hash=_required_hex_bytes(
                destination,
                "verifier_code_hash",
                byte_length=32,
            ),
            verifier_key_hash=_required_hex_bytes(
                destination,
                "verifier_key_hash",
                byte_length=32,
            ),
        )
        binding_key = module.evm_destination_binding_key(
            network_id=network_id,
            source_domain=SCCP_DOMAIN_SORA,
            target_domain=profile.domain,
            verifier_address=module.parse_evm_address(
                destination["verifier_identity"],
                label="verifier_identity",
            ),
            bridge_address=bridge_address,
            verifier_code_hash=_required_hex_bytes(
                destination,
                "verifier_code_hash",
                byte_length=32,
            ),
            verifier_key_hash=_required_hex_bytes(
                destination,
                "verifier_key_hash",
                byte_length=32,
            ),
        )
        return {
            "destination_binding_key": binding_key,
            "destination_binding_hash": _hex(binding_hash),
            "destination_network_id": _hex(network_id),
            "destination_bridge_address": _hex(bridge_address),
            "recomputed": True,
        }
    if profile.chain == "sol":
        module = _load_sibling_module("sccp_solana_destination_evidence.py")
        return {
            "destination_binding_key": module.solana_destination_binding_key(),
            "destination_binding_hash": _hex(module.solana_destination_binding_hash()),
            "recomputed": True,
        }
    if profile.chain == "ton":
        module = _load_sibling_module("sccp_ton_destination_evidence.py")
        return {
            "destination_binding_key": module.ton_destination_binding_key(),
            "destination_binding_hash": _hex(module.ton_destination_binding_hash()),
            "recomputed": True,
        }
    if profile.chain == "tron":
        if material is None:
            raise ValueError("TRON source material is required")
        module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
        network_id = _required_exact_hex_bytes(
            material,
            "source_bridge_network_id",
            byte_length=32,
        )
        verifier_code_hash = _required_exact_hex_bytes(
            destination,
            "verifier_code_hash",
            byte_length=32,
        )
        verifier_key_hash = _required_exact_hex_bytes(
            destination,
            "verifier_key_hash",
            byte_length=32,
        )
        args = {
            "network_id": network_id,
            "source_domain": SCCP_DOMAIN_SORA,
            "target_domain": SCCP_DOMAIN_TRON,
            "verifier_address": destination["verifier_identity"],
            "verifier_code_hash": verifier_code_hash,
            "verifier_key_hash": verifier_key_hash,
            "proof_family": SCCP_PROOF_FAMILY_STARK_FRI,
        }
        return {
            "destination_binding_key": module.tron_destination_binding_key(**args),
            "destination_binding_hash": _hex(
                module.tron_destination_binding_hash(**args),
            ),
            "destination_network_id": _hex(network_id),
            "recomputed": True,
        }
    raise ValueError(f"unsupported destination chain {profile.chain!r}")


def _check_destination_binding(
    profile: LaneProfile,
    material: dict[str, Any] | None,
    destination: dict[str, Any],
) -> tuple[list[str], dict[str, Any]]:
    errors: list[str] = []
    summary: dict[str, Any] = {}
    _check_hex_comment_matches_record(
        errors,
        destination,
        "destination_binding_hash",
        "_comment_destination_binding_hash",
        label="destination_binding_hash",
        byte_length=32,
    )
    _check_string_comment_matches_record(
        errors,
        destination,
        "destination_binding_key",
        "_comment_destination_binding_key",
        label="destination_binding_key",
    )
    _check_hex_comment_matches_record(
        errors,
        destination,
        "destination_network_id",
        "_comment_destination_network_id",
        label="destination_network_id",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        destination,
        "destination_bridge_address",
        "_comment_destination_bridge_address",
        label="destination_bridge_address",
        byte_length=20,
    )
    supplied = _first_record_value(
        destination,
        "destination_binding_hash",
        "_comment_destination_binding_hash",
    )
    supplied_raw = _hex_bytes(supplied, byte_length=32)
    if supplied_raw is None or not any(supplied_raw):
        errors.append(
            "destination_binding_hash must be supplied as a non-zero 32-byte hex value"
        )
        return errors, summary
    supplied_hash = _hex(supplied_raw)
    summary["destination_binding_hash"] = supplied_hash

    try:
        expected = _expected_destination_binding(profile, material, destination)
    except (argparse.ArgumentTypeError, SystemExit, ValueError, RuntimeError):
        errors.append("destination binding cannot be recomputed")
        return errors, summary

    expected_hash = expected["destination_binding_hash"]
    for key, value in expected.items():
        if key == "destination_binding_hash":
            summary["expected_destination_binding_hash"] = value
        else:
            summary[key] = value

    expected_network_id = expected.get("destination_network_id")
    if expected_network_id is not None:
        supplied_network_id = _first_record_value(
            destination,
            "destination_network_id",
            "_comment_destination_network_id",
        )
        supplied_network_id_raw = _hex_bytes(supplied_network_id, byte_length=32)
        if supplied_network_id_raw is None or not any(supplied_network_id_raw):
            errors.append(
                "destination_network_id must be supplied as a non-zero 32-byte hex value"
            )
        elif _hex(supplied_network_id_raw) != expected_network_id:
            errors.append(
                "destination_network_id does not match canonical "
                f"SORA -> {profile.chain} destination binding"
            )

    summary["expected_destination_binding_hash_matches"] = (
        supplied_hash == expected_hash
    )
    if not summary["expected_destination_binding_hash_matches"]:
        errors.append(
            "destination_binding_hash does not match canonical "
            f"SORA -> {profile.chain} destination binding"
        )

    expected_key = expected.get("destination_binding_key")
    supplied_key = _first_record_value(
        destination,
        "destination_binding_key",
        "_comment_destination_binding_key",
    )
    if expected_key is not None:
        if not _is_nonempty_string(supplied_key):
            errors.append(
                "destination_binding_key must be supplied for canonical "
                f"SORA -> {profile.chain} destination binding"
            )
        elif supplied_key != expected_key:
            errors.append(
                "destination_binding_key does not match canonical "
                f"SORA -> {profile.chain} destination binding"
            )
        else:
            summary["destination_binding_key"] = supplied_key
    elif _is_nonempty_string(supplied_key):
        summary["destination_binding_key"] = supplied_key
    return errors, summary


def _check_destination_rollout(profile: LaneProfile, record: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    _reject_unknown_fields(errors, record, DESTINATION_ROLLOUT_FIELDS)
    _reject_lane_foreign_fields(
        errors,
        record,
        EVM_DESTINATION_BRIDGE_BINDING_FIELDS,
        actual_chain=profile.chain,
        allowed_chains=("eth", "bsc"),
        label="EVM destination bridge binding evidence",
    )
    _reject_lane_foreign_fields(
        errors,
        record,
        EVM_TRON_DESTINATION_NETWORK_BINDING_FIELDS,
        actual_chain=profile.chain,
        allowed_chains=("eth", "bsc", "tron"),
        label="EVM/TRON destination network binding evidence",
    )
    _reject_lane_foreign_fields(
        errors,
        record,
        EVM_DESTINATION_VERIFIER_LIVE_COMMENT_FIELDS,
        actual_chain=profile.chain,
        allowed_chains=("eth", "bsc"),
        label="EVM destination verifier live evidence",
    )
    _reject_lane_foreign_fields(
        errors,
        record,
        SOLANA_DESTINATION_LIVE_FIELDS,
        actual_chain=profile.chain,
        allowed_chain="sol",
        label="Solana destination live evidence",
    )
    _reject_lane_foreign_fields(
        errors,
        record,
        TON_DESTINATION_LIVE_FIELDS,
        actual_chain=profile.chain,
        allowed_chain="ton",
        label="TON destination live evidence",
    )
    _reject_lane_foreign_fields(
        errors,
        record,
        TRON_DESTINATION_VERIFIER_LIVE_COMMENT_FIELDS,
        actual_chain=profile.chain,
        allowed_chain="tron",
        label="TRON destination verifier live evidence",
    )
    _expect(errors, record, "version", 1)
    _expect(errors, record, "domain", profile.domain)
    _expect(errors, record, "chain", profile.chain)
    _expect(errors, record, "verifier_plan", profile.destination_verifier_plan)
    _expect(errors, record, "immutable_verifier_ready", True)
    _expect(errors, record, "anchors_ready", True)
    _expect(errors, record, "anchor_id", profile.destination_anchor_id)
    if not _is_nonempty_string(record.get("verifier_identity")):
        errors.append("verifier_identity must be present")
    else:
        errors.extend(_check_destination_verifier_identity(profile, record))
    _expect_nonzero_hex(errors, record, "verifier_code_hash")
    if profile.destination_verifier_key_hash_required:
        _expect_nonzero_hex(errors, record, "verifier_key_hash")
    elif record.get("verifier_key_hash") not in (None, ""):
        errors.append("verifier_key_hash must be absent for this lane")
    if profile.chain in ("eth", "bsc"):
        errors.extend(_check_evm_live_bridge_evidence(profile, record))
    if profile.chain == "tron":
        errors.extend(_check_tron_live_destination_verifier_evidence(record))
    if profile.chain == "sol":
        errors.extend(_check_solana_live_programdata_evidence(record))
    if profile.chain == "ton":
        errors.extend(_check_ton_live_account_evidence(record))
    errors.extend(_blocker_list_errors(record, "destination rollout"))
    return errors


def _check_tron_live_destination_verifier_evidence(
    record: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
    address = record.get("_comment_tron_destination_verifier_address")
    try:
        observed_address = (
            module.parse_tron_address(address, label="TRON destination verifier")
            if _is_nonempty_string(address)
            else None
        )
        expected_address = module.parse_tron_address(
            record.get("verifier_identity"),
            label="verifier_identity",
        )
    except (argparse.ArgumentTypeError, ValueError):
        errors.append("TRON destination verifier address metadata is invalid")
        observed_address = None
        expected_address = None
    if observed_address is None or not any(observed_address):
        errors.append(
            "TRON destination verifier address metadata must be a non-zero "
            "20-byte address"
        )
    elif expected_address != observed_address:
        errors.append(
            "TRON destination verifier address metadata must match verifier_identity"
        )

    verifier_code_hash = _exact_hex_bytes(
        record.get("_comment_tron_destination_verifier_code_hash"),
        byte_length=32,
    )
    configured_code_hash = _exact_hex_bytes(
        record.get("verifier_code_hash"),
        byte_length=32,
    )
    if verifier_code_hash is None or not any(verifier_code_hash):
        errors.append(
            "TRON destination verifier runtime code hash metadata must be a "
            "non-zero 32-byte hex value"
        )
    elif configured_code_hash != verifier_code_hash:
        errors.append(
            "TRON destination verifier runtime code hash metadata must match "
            "verifier_code_hash"
        )
    verifier_runtime_bytecode = record.get(
        "_comment_tron_destination_verifier_runtime_bytecode_hex"
    )
    if not _is_nonempty_string(verifier_runtime_bytecode):
        errors.append(
            "TRON destination verifier runtime bytecode metadata must be present"
        )
    else:
        try:
            runtime = _parse_exact_runtime_bytecode(
                module,
                verifier_runtime_bytecode,
                label="TRON destination verifier runtime bytecode metadata",
            )
            derived_hash = module.runtime_bytecode_hash(runtime)
        except (argparse.ArgumentTypeError, ValueError):
            errors.append(
                "TRON destination verifier runtime bytecode metadata is invalid"
            )
        else:
            if verifier_code_hash is not None and derived_hash != verifier_code_hash:
                errors.append(
                    "TRON destination verifier runtime bytecode hash must match "
                    "runtime code hash metadata"
                )
            if configured_code_hash is not None and derived_hash != configured_code_hash:
                errors.append(
                    "TRON destination verifier runtime bytecode hash must match "
                    "verifier_code_hash"
                )
    verifier_key_hash = _exact_hex_bytes(
        record.get("_comment_tron_destination_verifier_key_hash"),
        byte_length=32,
    )
    configured_key_hash = _exact_hex_bytes(
        record.get("verifier_key_hash"),
        byte_length=32,
    )
    if verifier_key_hash is None or not any(verifier_key_hash):
        errors.append(
            "TRON destination verifier key hash metadata must be a non-zero "
            "32-byte hex value"
        )
    elif configured_key_hash != verifier_key_hash:
        errors.append(
            "TRON destination verifier key hash metadata must match "
            "verifier_key_hash"
        )
    verifier_backend_hash = _exact_hex_bytes(
        record.get("_comment_tron_destination_verifier_backend_hash"),
        byte_length=32,
    )
    expected_backend_hash = module._keccak_256(
        module.TRON_GROTH16_BACKEND.encode("utf-8")
    )
    if verifier_backend_hash is None or not any(verifier_backend_hash):
        errors.append(
            "TRON destination verifier backend hash metadata must be a non-zero "
            "32-byte hex value"
        )
    elif verifier_backend_hash != expected_backend_hash:
        errors.append(
            "TRON destination verifier backend hash metadata must match "
            "tron-groth16-bn254-v1"
        )
    proof_family_hash = _exact_hex_bytes(
        record.get("_comment_tron_destination_proof_family_hash"),
        byte_length=32,
    )
    expected_proof_family_hash = module._keccak_256(
        module.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")
    )
    if proof_family_hash is None or not any(proof_family_hash):
        errors.append(
            "TRON destination proof family hash metadata must be a non-zero "
            "32-byte hex value"
        )
    elif proof_family_hash != expected_proof_family_hash:
        errors.append(
            "TRON destination proof family hash metadata must match stark-fri-v1"
        )
    return errors


def _check_evm_live_bridge_evidence(
    profile: LaneProfile,
    record: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    module = _load_sibling_module("sccp_evm_destination_evidence.py")
    verifier_address = _hex_bytes(record.get("verifier_identity"), byte_length=20)
    bridge_address = _hex_bytes(
        record.get("destination_bridge_address"),
        byte_length=20,
    )
    if (
        verifier_address is not None
        and bridge_address is not None
        and verifier_address == bridge_address
    ):
        errors.append(
            "EVM destination verifier_identity must differ from destination_bridge_address"
        )

    rpc_chain_id = record.get("_comment_evm_rpc_chain_id")
    expected_chain_id = EVM_EXPECTED_RPC_CHAIN_IDS[profile.domain]
    if not _is_canonical_decimal_text(rpc_chain_id, positive=True):
        errors.append(
            "EVM live RPC chain-id metadata is required from "
            "sccp_evm_live_evidence.py"
        )
    elif int(rpc_chain_id, 10) != expected_chain_id:
        errors.append(
            f"EVM live RPC chain-id must be canonical for {profile.chain}: "
            f"expected {expected_chain_id}"
        )
    if (
        profile.domain == SCCP_DOMAIN_ETH
        and record.get("_comment_evm_block_tag") != "finalized"
    ):
        errors.append("Ethereum destination live block-tag metadata must be finalized")

    bridge_code_hash = _hex_bytes(
        record.get("_comment_evm_bridge_code_hash"),
        byte_length=32,
    )
    if bridge_code_hash is None or not any(bridge_code_hash):
        errors.append(
            "EVM bridge runtime code hash metadata must be a non-zero "
            "32-byte hex value"
        )
    bridge_runtime_bytecode = record.get("_comment_evm_bridge_runtime_bytecode_hex")
    if not _is_nonempty_string(bridge_runtime_bytecode):
        errors.append("EVM bridge runtime bytecode metadata must be present")
    else:
        try:
            bridge_runtime = _parse_exact_runtime_bytecode(
                module,
                bridge_runtime_bytecode,
                label="EVM bridge runtime bytecode metadata",
            )
            derived_bridge_code_hash = module.runtime_bytecode_hash(bridge_runtime)
        except (argparse.ArgumentTypeError, ValueError):
            errors.append("EVM bridge runtime bytecode metadata is invalid")
        else:
            if (
                bridge_code_hash is not None
                and derived_bridge_code_hash != bridge_code_hash
            ):
                errors.append(
                    "EVM bridge runtime bytecode hash must match bridge code hash metadata"
                )

    verifier_code_hash = _hex_bytes(
        record.get("_comment_evm_verifier_code_hash"),
        byte_length=32,
    )
    configured_verifier_code_hash = _hex_bytes(
        record.get("verifier_code_hash"),
        byte_length=32,
    )
    if verifier_code_hash is None or not any(verifier_code_hash):
        errors.append(
            "EVM verifier runtime code hash metadata must be a non-zero "
            "32-byte hex value"
        )
    elif configured_verifier_code_hash != verifier_code_hash:
        errors.append(
            "EVM verifier runtime code hash metadata must match verifier_code_hash"
        )
    verifier_runtime_bytecode = record.get("_comment_evm_verifier_runtime_bytecode_hex")
    if not _is_nonempty_string(verifier_runtime_bytecode):
        errors.append("EVM verifier runtime bytecode metadata must be present")
    else:
        try:
            verifier_runtime = _parse_exact_runtime_bytecode(
                module,
                verifier_runtime_bytecode,
                label="EVM verifier runtime bytecode metadata",
            )
            derived_verifier_code_hash = module.runtime_bytecode_hash(verifier_runtime)
        except (argparse.ArgumentTypeError, ValueError):
            errors.append("EVM verifier runtime bytecode metadata is invalid")
        else:
            if (
                verifier_code_hash is not None
                and derived_verifier_code_hash != verifier_code_hash
            ):
                errors.append(
                    "EVM verifier runtime bytecode hash must match verifier code hash metadata"
                )
            if (
                configured_verifier_code_hash is not None
                and derived_verifier_code_hash != configured_verifier_code_hash
            ):
                errors.append(
                    "EVM verifier runtime bytecode hash must match verifier_code_hash"
                )

    verifier_key_hash = _hex_bytes(
        record.get("_comment_evm_verifier_key_hash"),
        byte_length=32,
    )
    configured_verifier_key_hash = _hex_bytes(
        record.get("verifier_key_hash"),
        byte_length=32,
    )
    if verifier_key_hash is None or not any(verifier_key_hash):
        errors.append(
            "EVM verifier key hash metadata must be a non-zero 32-byte hex value"
        )
    elif configured_verifier_key_hash != verifier_key_hash:
        errors.append("EVM verifier key hash metadata must match verifier_key_hash")

    verifier_backend_hash = _hex_bytes(
        record.get("_comment_evm_verifier_backend_hash"),
        byte_length=32,
    )
    expected_backend_hash = module.evm_verifier_backend_hash()
    if verifier_backend_hash is None or not any(verifier_backend_hash):
        errors.append(
            "EVM verifier backend hash metadata must be a non-zero 32-byte hex value"
        )
    elif verifier_backend_hash != expected_backend_hash:
        errors.append(
            "EVM verifier backend hash metadata must match evm-groth16-bn254-v1"
        )

    proof_family_hash = _hex_bytes(
        record.get("_comment_evm_proof_family_hash"),
        byte_length=32,
    )
    expected_proof_family_hash = module.evm_proof_family_hash()
    if proof_family_hash is None or not any(proof_family_hash):
        errors.append(
            "EVM proof family hash metadata must be a non-zero 32-byte hex value"
        )
    elif proof_family_hash != expected_proof_family_hash:
        errors.append("EVM proof family hash metadata must match stark-fri-v1")
    return errors


def _check_solana_live_programdata_evidence(record: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    module = _load_sibling_module("sccp_solana_destination_evidence.py")

    def parse_positive_decimal(field: str, label: str) -> int | None:
        value = record.get(field)
        if not _is_canonical_decimal_text(value, positive=True):
            errors.append(f"{label} must be a positive decimal string")
            return None
        parsed = int(value, 10)
        if parsed <= 0:
            errors.append(f"{label} must be a positive decimal string")
            return None
        return parsed

    def decode_comment_base64(field: str, label: str) -> bytes | None:
        value = record.get(field)
        if not _is_nonempty_string(value):
            errors.append(f"{label} must be present")
            return None
        try:
            return _decode_canonical_base64(value, label=label)
        except ValueError:
            errors.append(f"{label} is invalid")
            return None

    def require_matching_string(actual_key: str, comment_key: str, label: str) -> None:
        actual = record.get(actual_key)
        comment = record.get(comment_key)
        if actual is not None and comment is not None and actual != comment:
            errors.append(f"{label} field must match {comment_key} comment")

    def require_matching_bool(actual_key: str, comment_key: str, label: str) -> None:
        actual = record.get(actual_key)
        comment = record.get(comment_key)
        if actual is not None and comment is not None and (actual is True) != (comment == "true"):
            errors.append(f"{label} field must match {comment_key} comment")

    for actual_key, comment_key, label in (
        ("solana_rpc_commitment", "_comment_solana_rpc_commitment", "Solana RPC commitment"),
        ("solana_program_owner", "_comment_solana_program_owner", "Solana program owner"),
        (
            "solana_programdata_owner",
            "_comment_solana_programdata_owner",
            "Solana ProgramData owner",
        ),
        (
            "solana_program_account_data_base64",
            "_comment_solana_program_account_data_base64",
            "Solana Program account data",
        ),
        (
            "solana_programdata_address",
            "_comment_solana_programdata_address",
            "Solana ProgramData address",
        ),
        (
            "solana_programdata_slot",
            "_comment_solana_programdata_slot",
            "Solana ProgramData slot",
        ),
        (
            "solana_expected_programdata_slot",
            "_comment_solana_expected_programdata_slot",
            "Solana expected ProgramData slot",
        ),
        (
            "solana_program_account_context_slot",
            "_comment_solana_program_account_context_slot",
            "Solana program account context slot",
        ),
        (
            "solana_programdata_account_context_slot",
            "_comment_solana_programdata_account_context_slot",
            "Solana ProgramData account context slot",
        ),
        (
            "solana_programdata_metadata_blake2b256",
            "_comment_solana_programdata_metadata_blake2b256",
            "Solana ProgramData metadata hash",
        ),
        (
            "solana_programdata_metadata_base64",
            "_comment_solana_programdata_metadata_base64",
            "Solana ProgramData metadata",
        ),
        (
            "solana_programdata_executable_blake2b256",
            "_comment_solana_programdata_code_hash",
            "Solana ProgramData executable hash",
        ),
        (
            "solana_programdata_executable_base64",
            "_comment_solana_programdata_executable_base64",
            "Solana ProgramData executable",
        ),
    ):
        require_matching_string(actual_key, comment_key, label)
    require_matching_bool(
        "solana_program_immutable",
        "_comment_solana_program_immutable",
        "Solana program immutable",
    )

    if record.get("_comment_solana_rpc_commitment") != "finalized":
        errors.append("Solana live RPC commitment metadata must be finalized")
    if record.get("_comment_solana_program_owner") != SOLANA_UPGRADEABLE_LOADER_ID:
        errors.append(
            "Solana verifier program owner metadata must be the BPF upgradeable loader"
        )
    if record.get("_comment_solana_programdata_owner") != SOLANA_UPGRADEABLE_LOADER_ID:
        errors.append(
            "Solana ProgramData owner metadata must be the BPF upgradeable loader"
        )
    if record.get("_comment_solana_program_immutable") != "true":
        errors.append("Solana verifier program immutable metadata must be true")
    program_account_data_len = parse_positive_decimal(
        "_comment_solana_program_account_data_len",
        "Solana Program account data length metadata",
    )
    if (
        program_account_data_len is not None
        and program_account_data_len != SOLANA_UPGRADEABLE_PROGRAM_ACCOUNT_LEN
    ):
        errors.append("Solana Program account data length metadata must be 36 bytes")

    programdata_address = record.get("_comment_solana_programdata_address")
    programdata_raw: bytes | None = None
    if not _is_nonempty_string(programdata_address):
        errors.append(
            "Solana live ProgramData account metadata is required from "
            "sccp_solana_live_evidence.py"
        )
    else:
        try:
            module._require_solana_program_id(
                programdata_address,
                label="Solana ProgramData account",
            )
            programdata_raw = module.decode_solana_base58(
                programdata_address,
                label="Solana ProgramData account",
            )
            if programdata_address == record.get("verifier_identity"):
                errors.append(
                    "Solana ProgramData account metadata must differ from verifier_identity"
                )
        except (argparse.ArgumentTypeError, ValueError):
            errors.append("Solana ProgramData account is not canonical")

    program_account_data = decode_comment_base64(
        "_comment_solana_program_account_data_base64",
        "Solana Program account data base64 metadata",
    )
    if program_account_data is not None:
        if len(program_account_data) != SOLANA_UPGRADEABLE_PROGRAM_ACCOUNT_LEN:
            errors.append(
                "Solana Program account data base64 metadata must decode to 36 bytes"
            )
        else:
            program_tag = int.from_bytes(program_account_data[:4], "little")
            if program_tag != SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG:
                errors.append(
                    "Solana Program account data metadata must be an upgradeable Program"
                )
            if (
                programdata_raw is not None
                and program_account_data[4:36] != programdata_raw
            ):
                errors.append(
                    "Solana Program account data metadata must reference "
                    "ProgramData account metadata"
                )

    parsed_programdata_slot = parse_positive_decimal(
        "_comment_solana_programdata_slot",
        "Solana ProgramData slot metadata",
    )
    parsed_expected_programdata_slot = parse_positive_decimal(
        "_comment_solana_expected_programdata_slot",
        "Solana expected ProgramData slot metadata",
    )
    parsed_program_context_slot = parse_positive_decimal(
        "_comment_solana_program_account_context_slot",
        "Solana program account RPC context slot metadata",
    )
    parsed_programdata_context_slot = parse_positive_decimal(
        "_comment_solana_programdata_account_context_slot",
        "Solana ProgramData account RPC context slot metadata",
    )

    if (
        parsed_programdata_slot is not None
        and parsed_expected_programdata_slot is not None
        and parsed_programdata_slot != parsed_expected_programdata_slot
    ):
        errors.append("Solana ProgramData slot metadata must match expected ProgramData slot")
    if (
        parsed_programdata_slot is not None
        and parsed_program_context_slot is not None
        and parsed_program_context_slot < parsed_programdata_slot
    ):
        errors.append(
            "Solana program account RPC context slot must be at or after "
            "ProgramData deployment slot"
        )
    if (
        parsed_programdata_slot is not None
        and parsed_programdata_context_slot is not None
        and parsed_programdata_context_slot < parsed_programdata_slot
    ):
        errors.append(
            "Solana ProgramData account RPC context slot must be at or after "
            "ProgramData deployment slot"
        )

    programdata_metadata_hash = _hex_bytes(
        record.get("_comment_solana_programdata_metadata_blake2b256"),
        byte_length=32,
    )
    if programdata_metadata_hash is None or not any(programdata_metadata_hash):
        errors.append(
            "Solana ProgramData metadata hash must be a non-zero 32-byte hex value"
        )
    programdata_metadata = decode_comment_base64(
        "_comment_solana_programdata_metadata_base64",
        "Solana ProgramData metadata base64 metadata",
    )
    if programdata_metadata is not None:
        if len(programdata_metadata) != SOLANA_PROGRAMDATA_METADATA_LEN:
            errors.append(
                "Solana ProgramData metadata base64 metadata must decode to 45 bytes"
            )
        else:
            derived_metadata_hash = hashlib.blake2b(
                programdata_metadata,
                digest_size=32,
            ).digest()
            if (
                programdata_metadata_hash is not None
                and derived_metadata_hash != programdata_metadata_hash
            ):
                errors.append(
                    "Solana ProgramData metadata base64 hash must match "
                    "ProgramData metadata hash"
                )
            metadata_tag = int.from_bytes(programdata_metadata[:4], "little")
            if metadata_tag != SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG:
                errors.append(
                    "Solana ProgramData metadata must encode a ProgramData account"
                )
            metadata_slot = int.from_bytes(programdata_metadata[4:12], "little")
            if (
                parsed_programdata_slot is not None
                and metadata_slot != parsed_programdata_slot
            ):
                errors.append(
                    "Solana ProgramData metadata slot must match ProgramData slot"
                )
            if programdata_metadata[12] != 0 or any(programdata_metadata[13:45]):
                errors.append(
                    "Solana ProgramData metadata must encode no upgrade authority"
                )

    programdata_code_hash = _hex_bytes(
        record.get("_comment_solana_programdata_code_hash"),
        byte_length=32,
    )
    if programdata_code_hash is None or not any(programdata_code_hash):
        errors.append(
            "Solana ProgramData executable hash metadata must be a non-zero "
            "32-byte hex value"
        )
    else:
        verifier_code_hash = _hex_bytes(record.get("verifier_code_hash"), byte_length=32)
        if verifier_code_hash != programdata_code_hash:
            errors.append(
                "Solana ProgramData executable hash must match verifier_code_hash"
            )
    programdata_executable_base64 = record.get(
        "_comment_solana_programdata_executable_base64"
    )
    if not _is_nonempty_string(programdata_executable_base64):
        errors.append("Solana ProgramData executable base64 metadata must be present")
    else:
        try:
            programdata_executable = module.parse_program_bytes_base64(
                programdata_executable_base64,
                label="Solana ProgramData executable base64 metadata",
            )
            derived_programdata_hash = module.solana_verifier_program_code_hash(
                programdata_executable
            )
        except (argparse.ArgumentTypeError, ValueError):
            errors.append(
                "Solana ProgramData executable base64 metadata is invalid"
            )
        else:
            verifier_code_hash = _hex_bytes(
                record.get("verifier_code_hash"),
                byte_length=32,
            )
            if (
                programdata_code_hash is not None
                and derived_programdata_hash != programdata_code_hash
            ):
                errors.append(
                    "Solana ProgramData executable base64 hash must match "
                    "ProgramData executable hash metadata"
                )
            if (
                verifier_code_hash is not None
                and derived_programdata_hash != verifier_code_hash
            ):
                errors.append(
                    "Solana ProgramData executable base64 hash must match "
                    "verifier_code_hash"
                )
    return errors


def _check_ton_live_account_evidence(record: dict[str, Any]) -> list[str]:
    errors: list[str] = []

    def require_matching_comment(
        actual_key: str,
        comment_key: str,
        *,
        label: str,
    ) -> None:
        actual = record.get(actual_key)
        comment = record.get(comment_key)
        if comment in (None, ""):
            errors.append(f"TON {label} comment must be present")
        elif actual != comment:
            errors.append(f"TON {label} comment must match {actual_key}")

    account_status = record.get("ton_account_status")
    require_matching_comment(
        "ton_account_status",
        "_comment_ton_account_status",
        label="live account status",
    )
    if account_status != "active":
        errors.append("TON live account status metadata must be active")

    account_state_hash = _hex_bytes(
        record.get("ton_account_state_hash"),
        byte_length=32,
    )
    require_matching_comment(
        "ton_account_state_hash",
        "_comment_ton_account_state_hash",
        label="account state hash",
    )
    if account_state_hash is None or not any(account_state_hash):
        errors.append(
            "TON account state hash metadata must be a non-zero 32-byte hex value"
        )

    last_transaction_lt = record.get("ton_last_transaction_lt")
    require_matching_comment(
        "ton_last_transaction_lt",
        "_comment_ton_last_transaction_lt",
        label="last transaction LT",
    )
    if (
        not _is_canonical_decimal_text(last_transaction_lt, positive=True)
    ):
        errors.append("TON last transaction LT metadata must be a positive decimal string")

    last_transaction_hash = _hex_bytes(
        record.get("ton_last_transaction_hash"),
        byte_length=32,
    )
    require_matching_comment(
        "ton_last_transaction_hash",
        "_comment_ton_last_transaction_hash",
        label="last transaction hash",
    )
    if last_transaction_hash is None or not any(last_transaction_hash):
        errors.append(
            "TON last transaction hash metadata must be a non-zero 32-byte hex value"
        )

    verifier_code_hash = _hex_bytes(record.get("verifier_code_hash"), byte_length=32)
    code_hash = _hex_bytes(record.get("_comment_ton_code_hash"), byte_length=32)
    if code_hash is None or not any(code_hash):
        errors.append(
            "TON code hash metadata must be a non-zero 32-byte hex value"
        )
    elif verifier_code_hash != code_hash:
        errors.append("TON code hash metadata must match verifier_code_hash")

    code_boc_root_hash = _hex_bytes(
        record.get("ton_verifier_code_boc_root_hash"),
        byte_length=32,
    )
    require_matching_comment(
        "ton_verifier_code_boc_root_hash",
        "_comment_ton_code_boc_root_hash",
        label="code BoC root hash",
    )
    if code_boc_root_hash is None or not any(code_boc_root_hash):
        errors.append(
            "TON code BoC root hash metadata must be a non-zero 32-byte hex value"
        )
    elif verifier_code_hash is not None and code_boc_root_hash != verifier_code_hash:
        errors.append("TON code BoC root hash metadata must match verifier_code_hash")

    if record.get("_comment_ton_code_boc_hash_matches") != "true":
        errors.append("TON code BoC hash match metadata must be true")

    code_boc_hex = record.get("ton_verifier_code_boc")
    if not _is_nonempty_string(code_boc_hex):
        errors.append("TON verifier code BoC metadata must be present")
    else:
        try:
            ton_module = _load_sibling_module("sccp_ton_destination_evidence.py")
            code_boc_bytes = ton_module.parse_code_boc_hex(
                code_boc_hex,
                label="TON verifier code BoC metadata",
            )
            derived_code_boc_root_hash = ton_module.ton_boc_single_root_hash(
                code_boc_bytes
            )
        except (argparse.ArgumentTypeError, ValueError):
            errors.append("TON verifier code BoC metadata is invalid")
        else:
            if (
                code_boc_root_hash is not None
                and derived_code_boc_root_hash != code_boc_root_hash
            ):
                errors.append("TON verifier code BoC root must match root metadata")
            if verifier_code_hash is not None and derived_code_boc_root_hash != verifier_code_hash:
                errors.append("TON verifier code BoC root must match verifier_code_hash")

            code_boc_base64 = record.get("_comment_ton_code_boc_base64")
            if not _is_nonempty_string(code_boc_base64):
                errors.append("TON code BoC base64 metadata must be present")
            else:
                try:
                    comment_code_boc = ton_module.parse_code_boc_base64(
                        code_boc_base64,
                        label="TON code BoC base64 metadata",
                    )
                    comment_root = ton_module.ton_boc_single_root_hash(comment_code_boc)
                except (argparse.ArgumentTypeError, ValueError):
                    errors.append("TON code BoC base64 metadata is invalid")
                else:
                    if comment_code_boc != code_boc_bytes:
                        errors.append("TON code BoC base64 metadata must match verifier code BoC")
                    if comment_root != derived_code_boc_root_hash:
                        errors.append("TON code BoC base64 root hash must match verifier code BoC")
    return errors

def _check_destination_verifier_identity(
    profile: LaneProfile,
    record: dict[str, Any],
) -> list[str]:
    identity = record["verifier_identity"]
    try:
        if profile.chain in ("eth", "bsc"):
            module = _load_sibling_module("sccp_evm_destination_evidence.py")
            module.parse_evm_address(identity, label="verifier_identity")
        elif profile.chain == "sol":
            module = _load_sibling_module("sccp_solana_destination_evidence.py")
            module._require_solana_program_id(identity, label="verifier_identity")
        elif profile.chain == "ton":
            module = _load_sibling_module("sccp_ton_destination_evidence.py")
            module._require_ton_raw_address(identity, label="verifier_identity")
        elif profile.chain == "tron":
            module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
            module.normalize_tron_base58check_address(
                identity,
                label="verifier_identity",
            )
        else:
            return [f"unsupported destination chain {profile.chain!r}"]
    except (argparse.ArgumentTypeError, SystemExit, ValueError, RuntimeError):
        return [f"verifier_identity is not canonical for {profile.chain}"]
    return []


def _check_route_allowlist(
    profile: LaneProfile,
    record: dict[str, Any],
    source_record_hashes: dict[str, str],
    destination_binding: dict[str, Any],
    destination_record: dict[str, Any] | None,
) -> tuple[list[str], dict[str, Any]]:
    errors: list[str] = []
    summary: dict[str, Any] = {}
    _reject_unknown_fields(errors, record, ROUTE_ALLOWLIST_FIELDS)
    _expect(errors, record, "version", 1)
    _expect(errors, record, "domain", profile.domain)
    _expect(errors, record, "chain", profile.chain)
    _expect(errors, record, "activation_policy", "GovernanceAllowlist")
    _expect(errors, record, "route_allowlist_id", profile.route_allowlist_id)
    _expect(errors, record, "routes_allowlisted", True)
    _expect_nonzero_hex(errors, record, "route_allowlist_hash")
    errors.extend(_blocker_list_errors(record, "route allowlist"))

    supplied_hash_raw = _hex_bytes(record.get("route_allowlist_hash"), byte_length=32)
    if supplied_hash_raw is not None and any(supplied_hash_raw):
        summary["route_allowlist_hash"] = _hex(supplied_hash_raw)
    try:
        expected_hash = _expected_route_allowlist_hash(
            profile,
            source_record_hashes,
            destination_binding,
        )
    except ValueError:
        errors.append("route_allowlist_hash cannot be recomputed")
        return errors, summary

    expected_hash_hex = _hex(expected_hash)
    summary["expected_route_allowlist_hash"] = expected_hash_hex
    summary["expected_route_allowlist_hash_matches"] = (
        supplied_hash_raw == expected_hash
    )
    if not summary["expected_route_allowlist_hash_matches"]:
        errors.append(
            "route_allowlist_hash does not match canonical source, deployment, "
            f"and destination evidence for SORA -> {profile.chain}"
        )
    errors.extend(
        _check_route_canary_evidence(
            profile,
            record,
            supplied_hash_raw=supplied_hash_raw,
            source_record_hashes=source_record_hashes,
            destination_binding=destination_binding,
            destination_record=destination_record,
            summary=summary,
        )
    )
    return errors, summary


def _canonical_route_canary_log_index(value: Any) -> int | None:
    if isinstance(value, int) and not isinstance(value, bool):
        if 0 <= value <= 0xFFFFFFFF:
            return value
        return None
    if not _is_canonical_decimal_text(value, positive=False):
        return None
    parsed = int(value, 10)
    if parsed > 0xFFFFFFFF:
        return None
    return parsed


def _canonical_decimal_int(value: Any, *, positive: bool) -> int | None:
    if isinstance(value, int) and not isinstance(value, bool):
        if value < 0 or (positive and value == 0):
            return None
        return value
    if not _is_canonical_decimal_text(value, positive=positive):
        return None
    return int(value, 10)


def _check_route_canary_decimal_comment_matches_record(
    errors: list[str],
    record: dict[str, Any],
    field: str,
    comment_field: str,
    *,
    label: str,
    positive: bool,
) -> None:
    value = record.get(field)
    comment = record.get(comment_field)
    if value in (None, "") or comment in (None, ""):
        return

    parsed = _canonical_decimal_int(value, positive=positive)
    if parsed is None:
        return
    comment_parsed = _canonical_decimal_int(comment, positive=positive)
    if comment_parsed is None:
        qualifier = "positive " if positive else ""
        errors.append(f"{label} comment must be a canonical {qualifier}decimal")
    elif comment_parsed != parsed:
        errors.append(f"{label} comment must match {field}")


def _route_canary_used_message_proof_value(value: Any) -> bool | None:
    if isinstance(value, bool):
        return value
    if value == "true":
        return True
    if value == "false":
        return False
    return None


def _check_route_canary_log_index_comment_matches_record(
    errors: list[str],
    record: dict[str, Any],
    field: str,
    comment_field: str,
    *,
    label: str,
) -> None:
    value = record.get(field)
    comment = record.get(comment_field)
    if value in (None, "") or comment in (None, ""):
        return

    parsed = _canonical_route_canary_log_index(value)
    if parsed is None:
        return
    comment_parsed = _canonical_route_canary_log_index(comment)
    if comment_parsed is None:
        errors.append(f"{label} comment must be a canonical u32")
    elif comment_parsed != parsed:
        errors.append(f"{label} comment must match {field}")


def _check_route_canary_bool_comment_matches_record(
    errors: list[str],
    record: dict[str, Any],
    field: str,
    comment_field: str,
    *,
    label: str,
) -> None:
    value = record.get(field)
    comment = record.get(comment_field)
    if value in (None, "") or comment in (None, ""):
        return

    parsed = _route_canary_used_message_proof_value(value)
    if parsed is None:
        return
    comment_parsed = _route_canary_used_message_proof_value(comment)
    if comment_parsed is None:
        errors.append(f"{label} comment must be true or false")
    elif comment_parsed != parsed:
        errors.append(f"{label} comment must match {field}")


def _check_evm_route_canary_transaction_evidence(
    record: dict[str, Any],
    *,
    destination_record: dict[str, Any] | None,
    source_record_hashes: dict[str, str],
    evidence_hash: bytes | None,
    route_allowlist_hash: bytes | None,
    destination_binding_hash: bytes | None,
    canary: dict[str, Any],
) -> list[str]:
    fields = (
        "evm_route_canary_transaction_hash",
        "evm_route_canary_transaction_block_number",
        "evm_route_canary_transaction_block_hash",
        "evm_route_canary_log_index",
        "evm_route_canary_receipt_block_number",
        "evm_route_canary_receipt_block_hash",
        "evm_route_canary_block_receipts_root",
        "evm_route_canary_call_data_sha256",
        "evm_route_canary_message_id",
        "evm_route_canary_payload_hash",
        "evm_route_canary_target_domain",
        "evm_route_canary_statement_hash",
        "evm_route_canary_commitment_root",
        "evm_route_canary_finality_height",
        "evm_route_canary_finality_block_hash",
        "evm_route_canary_proof_version",
        "evm_route_canary_proof_source_domain",
        "evm_route_canary_used_message_proof",
        "_comment_evm_route_canary_transaction_hash",
        "_comment_evm_route_canary_transaction_block_number",
        "_comment_evm_route_canary_transaction_block_hash",
        "_comment_evm_route_canary_log_index",
        "_comment_evm_route_canary_receipt_block_number",
        "_comment_evm_route_canary_receipt_block_hash",
        "_comment_evm_route_canary_block_receipts_root",
        "_comment_evm_route_canary_call_data_sha256",
        "_comment_evm_route_canary_message_id",
        "_comment_evm_route_canary_payload_hash",
        "_comment_evm_route_canary_target_domain",
        "_comment_evm_route_canary_statement_hash",
        "_comment_evm_route_canary_commitment_root",
        "_comment_evm_route_canary_finality_height",
        "_comment_evm_route_canary_finality_block_hash",
        "_comment_evm_route_canary_proof_version",
        "_comment_evm_route_canary_proof_source_domain",
        "_comment_evm_route_canary_used_message_proof",
    )
    if not any(record.get(field) not in (None, "") for field in fields):
        return ["EVM route canary transaction metadata must be present"]
    errors: list[str] = []
    if destination_record is None:
        return ["EVM route canary transaction evidence requires destination rollout"]
    for field, comment_field, label in (
        (
            "evm_route_canary_transaction_hash",
            "_comment_evm_route_canary_transaction_hash",
            "EVM route canary transaction hash",
        ),
        (
            "evm_route_canary_call_data_sha256",
            "_comment_evm_route_canary_call_data_sha256",
            "EVM route canary call data SHA-256",
        ),
        (
            "evm_route_canary_receipt_block_hash",
            "_comment_evm_route_canary_receipt_block_hash",
            "EVM route canary receipt block hash",
        ),
        (
            "evm_route_canary_transaction_block_hash",
            "_comment_evm_route_canary_transaction_block_hash",
            "EVM route canary transaction block hash",
        ),
        (
            "evm_route_canary_block_receipts_root",
            "_comment_evm_route_canary_block_receipts_root",
            "EVM route canary block receiptsRoot",
        ),
        (
            "evm_route_canary_message_id",
            "_comment_evm_route_canary_message_id",
            "EVM route canary message id",
        ),
        (
            "evm_route_canary_payload_hash",
            "_comment_evm_route_canary_payload_hash",
            "EVM route canary payload hash",
        ),
        (
            "evm_route_canary_statement_hash",
            "_comment_evm_route_canary_statement_hash",
            "EVM route canary statement hash",
        ),
        (
            "evm_route_canary_commitment_root",
            "_comment_evm_route_canary_commitment_root",
            "EVM route canary commitment root",
        ),
        (
            "evm_route_canary_finality_height",
            "_comment_evm_route_canary_finality_height",
            "EVM route canary finality height",
        ),
        (
            "evm_route_canary_finality_block_hash",
            "_comment_evm_route_canary_finality_block_hash",
            "EVM route canary finality block hash",
        ),
    ):
        _check_hex_comment_matches_record(
            errors,
            record,
            field,
            comment_field,
            label=label,
            byte_length=32,
        )
    for field, comment_field, label in (
        (
            "evm_route_canary_log_index",
            "_comment_evm_route_canary_log_index",
            "EVM route canary log index",
        ),
        (
            "evm_route_canary_target_domain",
            "_comment_evm_route_canary_target_domain",
            "EVM route canary target domain",
        ),
        (
            "evm_route_canary_proof_version",
            "_comment_evm_route_canary_proof_version",
            "EVM route canary proof version",
        ),
        (
            "evm_route_canary_proof_source_domain",
            "_comment_evm_route_canary_proof_source_domain",
            "EVM route canary proof source domain",
        ),
    ):
        _check_route_canary_log_index_comment_matches_record(
            errors,
            record,
            field,
            comment_field,
            label=label,
        )
    _check_route_canary_decimal_comment_matches_record(
        errors,
        record,
        "evm_route_canary_receipt_block_number",
        "_comment_evm_route_canary_receipt_block_number",
        label="EVM route canary receipt block number",
        positive=True,
    )
    _check_route_canary_decimal_comment_matches_record(
        errors,
        record,
        "evm_route_canary_transaction_block_number",
        "_comment_evm_route_canary_transaction_block_number",
        label="EVM route canary transaction block number",
        positive=True,
    )
    _check_route_canary_bool_comment_matches_record(
        errors,
        record,
        "evm_route_canary_used_message_proof",
        "_comment_evm_route_canary_used_message_proof",
        label="EVM route canary usedMessageProofs",
    )
    _check_route_canary_bool_comment_matches_record(
        errors,
        record,
        "evm_route_canary_receipt_block_finalized",
        "_comment_evm_route_canary_receipt_block_finalized",
        label="EVM route canary receipt block finalized",
    )
    transaction_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_transaction_hash",
            "_comment_evm_route_canary_transaction_hash",
        ),
        byte_length=32,
    )
    if transaction_hash is None or not any(transaction_hash):
        errors.append(
            "EVM route canary transaction hash metadata must be a non-zero bytes32"
        )
    transaction_block_number = _canonical_decimal_int(
        _first_record_value(
            record,
            "evm_route_canary_transaction_block_number",
            "_comment_evm_route_canary_transaction_block_number",
        ),
        positive=True,
    )
    if transaction_block_number is None:
        errors.append(
            "EVM route canary transaction block number metadata must be a "
            "canonical positive decimal"
        )
    transaction_block_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_transaction_block_hash",
            "_comment_evm_route_canary_transaction_block_hash",
        ),
        byte_length=32,
    )
    if transaction_block_hash is None or not any(transaction_block_hash):
        errors.append(
            "EVM route canary transaction block hash metadata must be a non-zero bytes32"
        )
    log_index = _canonical_route_canary_log_index(
        _first_record_value(
            record,
            "evm_route_canary_log_index",
            "_comment_evm_route_canary_log_index",
        )
    )
    if log_index is None:
        errors.append("EVM route canary log index metadata must be a canonical u32")
    receipt_block_number = _canonical_decimal_int(
        _first_record_value(
            record,
            "evm_route_canary_receipt_block_number",
            "_comment_evm_route_canary_receipt_block_number",
        ),
        positive=True,
    )
    if receipt_block_number is None:
        errors.append(
            "EVM route canary receipt block number metadata must be a canonical positive decimal"
        )
    receipt_block_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_receipt_block_hash",
            "_comment_evm_route_canary_receipt_block_hash",
        ),
        byte_length=32,
    )
    if receipt_block_hash is None or not any(receipt_block_hash):
        errors.append(
            "EVM route canary receipt block hash metadata must be a non-zero bytes32"
        )
    elif (
        transaction_block_hash is not None
        and any(transaction_block_hash)
        and transaction_block_hash != receipt_block_hash
    ):
        errors.append(
            "EVM route canary transaction block hash metadata must match "
            "receipt block hash metadata"
        )
    if (
        transaction_block_number is not None
        and receipt_block_number is not None
        and transaction_block_number != receipt_block_number
    ):
        errors.append(
            "EVM route canary transaction block number metadata must match "
            "receipt block number metadata"
        )
    block_receipts_root = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_block_receipts_root",
            "_comment_evm_route_canary_block_receipts_root",
        ),
        byte_length=32,
    )
    if block_receipts_root is None or not any(block_receipts_root):
        errors.append(
            "EVM route canary block receiptsRoot metadata must be a non-zero bytes32"
        )
    call_data_sha256 = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_call_data_sha256",
            "_comment_evm_route_canary_call_data_sha256",
        ),
        byte_length=32,
    )
    if call_data_sha256 is None or not any(call_data_sha256):
        errors.append(
            "EVM route canary call data SHA-256 metadata must be a non-zero bytes32"
        )
    message_id = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_message_id",
            "_comment_evm_route_canary_message_id",
        ),
        byte_length=32,
    )
    if message_id is None or not any(message_id):
        errors.append("EVM route canary message id metadata must be a non-zero bytes32")
    payload_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_payload_hash",
            "_comment_evm_route_canary_payload_hash",
        ),
        byte_length=32,
    )
    if payload_hash is None or not any(payload_hash):
        errors.append(
            "EVM route canary payload hash metadata must be a non-zero bytes32"
        )
    target_domain = _canonical_route_canary_log_index(
        _first_record_value(
            record,
            "evm_route_canary_target_domain",
            "_comment_evm_route_canary_target_domain",
        )
    )
    if target_domain not in (SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC):
        errors.append("EVM route canary target domain metadata must be ETH or BSC")
    statement_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_statement_hash",
            "_comment_evm_route_canary_statement_hash",
        ),
        byte_length=32,
    )
    if statement_hash is None or not any(statement_hash):
        errors.append(
            "EVM route canary statement hash metadata must be a non-zero bytes32"
        )
    commitment_root = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_commitment_root",
            "_comment_evm_route_canary_commitment_root",
        ),
        byte_length=32,
    )
    if commitment_root is None or not any(commitment_root):
        errors.append(
            "EVM route canary commitment root metadata must be a non-zero bytes32"
        )
    finality_height = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_finality_height",
            "_comment_evm_route_canary_finality_height",
        ),
        byte_length=32,
    )
    if finality_height is None or not any(finality_height):
        errors.append(
            "EVM route canary finality height metadata must be a non-zero bytes32"
        )
    finality_block_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "evm_route_canary_finality_block_hash",
            "_comment_evm_route_canary_finality_block_hash",
        ),
        byte_length=32,
    )
    if finality_block_hash is None or not any(finality_block_hash):
        errors.append(
            "EVM route canary finality block hash metadata must be a non-zero bytes32"
        )
    proof_version = _canonical_route_canary_log_index(
        _first_record_value(
            record,
            "evm_route_canary_proof_version",
            "_comment_evm_route_canary_proof_version",
        )
    )
    if proof_version != 1:
        errors.append("EVM route canary proof version metadata must be 1")
    proof_source_domain = _canonical_route_canary_log_index(
        _first_record_value(
            record,
            "evm_route_canary_proof_source_domain",
            "_comment_evm_route_canary_proof_source_domain",
        )
    )
    if proof_source_domain != SCCP_DOMAIN_SORA:
        errors.append("EVM route canary proof source domain metadata must be SORA")
    used_message_proof = _route_canary_used_message_proof_value(
        _first_record_value(
            record,
            "evm_route_canary_used_message_proof",
            "_comment_evm_route_canary_used_message_proof",
        )
    )
    if used_message_proof is not True:
        errors.append("EVM route canary usedMessageProofs metadata must be true")
    receipt_block_finalized = _route_canary_used_message_proof_value(
        _first_record_value(
            record,
            "evm_route_canary_receipt_block_finalized",
            "_comment_evm_route_canary_receipt_block_finalized",
        )
    )
    if receipt_block_finalized is not True:
        errors.append("EVM route canary receipt block finalized metadata must be true")
    _expect_distinct_byte_values(
        errors,
        (
            ("evm_route_canary_transaction_hash", transaction_hash),
            ("evm_route_canary_receipt_block_hash", receipt_block_hash),
            ("evm_route_canary_block_receipts_root", block_receipts_root),
            ("evm_route_canary_call_data_sha256", call_data_sha256),
            ("evm_route_canary_message_id", message_id),
            ("evm_route_canary_payload_hash", payload_hash),
            ("evm_route_canary_statement_hash", statement_hash),
            ("evm_route_canary_commitment_root", commitment_root),
            ("evm_route_canary_finality_height", finality_height),
            ("evm_route_canary_finality_block_hash", finality_block_hash),
        ),
        label="EVM route canary transcript hash",
    )
    if evidence_hash is None or route_allowlist_hash is None:
        errors.append("EVM route canary transaction evidence requires canary and route hashes")
    if destination_binding_hash is None:
        errors.append("EVM route canary transaction evidence requires destination binding hash")
    source_material_hash = _hex_bytes(
        source_record_hashes.get("source_verifier_material_hash"),
        byte_length=32,
    )
    source_deployment_hash = _hex_bytes(
        source_record_hashes.get("source_adapter_engine_deployment_hash"),
        byte_length=32,
    )
    if source_material_hash is None or source_deployment_hash is None:
        errors.append("EVM route canary transaction evidence requires source record hashes")
    _expect_distinct_byte_values(
        errors,
        (
            ("route_allowlist_hash", route_allowlist_hash),
            ("destination_binding_hash", destination_binding_hash),
            ("source_verifier_material_hash", source_material_hash),
            ("source_adapter_engine_deployment_hash", source_deployment_hash),
            ("evm_route_canary_transaction_hash", transaction_hash),
            ("evm_route_canary_receipt_block_hash", receipt_block_hash),
            ("evm_route_canary_block_receipts_root", block_receipts_root),
            ("evm_route_canary_call_data_sha256", call_data_sha256),
            ("evm_route_canary_message_id", message_id),
            ("evm_route_canary_payload_hash", payload_hash),
            ("evm_route_canary_statement_hash", statement_hash),
            ("evm_route_canary_commitment_root", commitment_root),
            ("evm_route_canary_finality_height", finality_height),
            ("evm_route_canary_finality_block_hash", finality_block_hash),
        ),
        label="EVM route canary hash role",
    )

    module = _load_sibling_module("sccp_evm_destination_evidence.py")
    bridge_address = _exact_hex_bytes(
        _first_record_value(
            destination_record,
            "destination_bridge_address",
            "_comment_destination_bridge_address",
        ),
        byte_length=20,
    )
    if bridge_address is None or not any(bridge_address):
        errors.append("EVM route canary bridge address metadata must be a non-zero address")
    network_id = _exact_hex_bytes(
        _first_record_value(
            destination_record,
            "destination_network_id",
            "_comment_destination_network_id",
        ),
        byte_length=32,
    )
    if network_id is None or not any(network_id):
        errors.append("EVM route canary network id metadata must be a non-zero bytes32")
    destination_domain = _canonical_route_canary_log_index(destination_record.get("domain"))
    if destination_domain not in (SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC):
        errors.append("EVM route canary destination domain metadata must be ETH or BSC")
    if target_domain is not None and destination_domain is not None:
        if target_domain != destination_domain:
            errors.append(
                "EVM route canary target domain metadata must match destination rollout"
            )
    verifier_backend_hash = _exact_hex_bytes(
        destination_record.get("_comment_evm_verifier_backend_hash"),
        byte_length=32,
    )
    if verifier_backend_hash is None or not any(verifier_backend_hash):
        errors.append(
            "EVM route canary verifier backend hash metadata must be a non-zero bytes32"
        )
    proof_family_hash = _exact_hex_bytes(
        destination_record.get("_comment_evm_proof_family_hash"),
        byte_length=32,
    )
    if proof_family_hash is None or not any(proof_family_hash):
        errors.append(
            "EVM route canary proof family hash metadata must be a non-zero bytes32"
        )
    if errors:
        return errors
    assert transaction_hash is not None
    assert log_index is not None
    assert receipt_block_number is not None
    assert receipt_block_hash is not None
    assert block_receipts_root is not None
    assert call_data_sha256 is not None
    assert message_id is not None
    assert payload_hash is not None
    assert target_domain is not None
    assert statement_hash is not None
    assert commitment_root is not None
    assert finality_height is not None
    assert finality_block_hash is not None
    assert proof_version is not None
    assert proof_source_domain is not None
    assert evidence_hash is not None
    assert route_allowlist_hash is not None
    assert destination_binding_hash is not None
    assert bridge_address is not None
    assert network_id is not None
    assert verifier_backend_hash is not None
    assert proof_family_hash is not None
    expected_hash = module.evm_route_canary_transaction_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        bridge_address=bridge_address,
        transaction_hash=transaction_hash,
        log_index=log_index,
        receipt_block_number=receipt_block_number,
        receipt_block_hash=receipt_block_hash,
        block_receipts_root=block_receipts_root,
        call_data_sha256=call_data_sha256,
        message_id=message_id,
        payload_hash=payload_hash,
        source_domain=SCCP_DOMAIN_SORA,
        target_domain=target_domain,
        commitment_root=commitment_root,
        finality_height=finality_height,
        finality_block_hash=finality_block_hash,
        statement_hash=statement_hash,
        proof_version=proof_version,
        proof_source_domain=proof_source_domain,
        destination_binding_hash=destination_binding_hash,
        verifier_backend_hash=verifier_backend_hash,
        proof_family_hash=proof_family_hash,
        network_id=network_id,
        used_message_proof=used_message_proof,
        receipt_block_finalized=receipt_block_finalized,
    )
    if evidence_hash != expected_hash:
        errors.append(
            "EVM route canary evidence hash must match "
            "MessageProofAccepted transaction metadata"
        )
    else:
        canary["evidence_source"] = "evm_message_proof_accepted_transaction"
        canary["transaction_hash"] = _hex(transaction_hash)
        canary["log_index"] = log_index
        canary["receipt_block_number"] = receipt_block_number
        canary["receipt_block_hash"] = _hex(receipt_block_hash)
        canary["block_receipts_root"] = _hex(block_receipts_root)
        canary["call_data_sha256"] = _hex(call_data_sha256)
        canary["message_id"] = _hex(message_id)
        canary["payload_hash"] = _hex(payload_hash)
        canary["target_domain"] = target_domain
        canary["statement_hash"] = _hex(statement_hash)
        canary["commitment_root"] = _hex(commitment_root)
        canary["finality_height"] = _hex(finality_height)
        canary["finality_block_hash"] = _hex(finality_block_hash)
        canary["proof_version"] = proof_version
        canary["proof_source_domain"] = proof_source_domain
        canary["message_proof_used"] = True
        canary["receipt_block_finalized"] = True
    return errors


def _check_tron_route_canary_transaction_evidence(
    record: dict[str, Any],
    *,
    destination_record: dict[str, Any] | None,
    source_record_hashes: dict[str, str],
    evidence_hash: bytes | None,
    route_allowlist_hash: bytes | None,
    destination_binding_hash: bytes | None,
    canary: dict[str, Any],
) -> list[str]:
    fields = (
        "tron_route_canary_transaction_id",
        "tron_route_canary_transaction_owner_address",
        "tron_route_canary_block_number",
        "tron_route_canary_block_timestamp",
        "tron_route_canary_log_index",
        "tron_route_canary_message_id",
        "tron_route_canary_call_data_sha256",
        "tron_route_canary_payload_hash",
        "tron_route_canary_target_domain",
        "tron_route_canary_statement_hash",
        "tron_route_canary_commitment_root",
        "tron_route_canary_finality_height",
        "tron_route_canary_finality_block_hash",
        "tron_route_canary_proof_version",
        "tron_route_canary_proof_source_domain",
        "tron_route_canary_used_message_proof",
        "tron_route_canary_raw_data_owner_matches_transaction",
        "tron_route_canary_signature_sha256",
        "tron_route_canary_signature_recovered_address",
        "tron_route_canary_signature_recovers_to_owner",
        "_comment_tron_route_canary_transaction_id",
        "_comment_tron_route_canary_transaction_owner_address",
        "_comment_tron_route_canary_block_number",
        "_comment_tron_route_canary_block_timestamp",
        "_comment_tron_route_canary_log_index",
        "_comment_tron_route_canary_message_id",
        "_comment_tron_route_canary_call_data_sha256",
        "_comment_tron_route_canary_payload_hash",
        "_comment_tron_route_canary_target_domain",
        "_comment_tron_route_canary_statement_hash",
        "_comment_tron_route_canary_commitment_root",
        "_comment_tron_route_canary_finality_height",
        "_comment_tron_route_canary_finality_block_hash",
        "_comment_tron_route_canary_proof_version",
        "_comment_tron_route_canary_proof_source_domain",
        "_comment_tron_route_canary_used_message_proof",
        "_comment_tron_route_canary_raw_data_owner_matches_transaction",
        "_comment_tron_route_canary_signature_sha256",
        "_comment_tron_route_canary_signature_recovered_address",
        "_comment_tron_route_canary_signature_recovers_to_owner",
    )
    if not any(record.get(field) not in (None, "") for field in fields):
        return ["TRON route canary transaction metadata must be present"]
    errors: list[str] = []
    if destination_record is None:
        return ["TRON route canary transaction evidence requires destination rollout"]
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_transaction_id",
        "_comment_tron_route_canary_transaction_id",
        label="TRON route canary transaction id",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_transaction_owner_address",
        "_comment_tron_route_canary_transaction_owner_address",
        label="TRON route canary transaction owner address",
        byte_length=21,
    )
    _check_route_canary_decimal_comment_matches_record(
        errors,
        record,
        "tron_route_canary_block_number",
        "_comment_tron_route_canary_block_number",
        label="TRON route canary block number",
        positive=True,
    )
    _check_route_canary_decimal_comment_matches_record(
        errors,
        record,
        "tron_route_canary_block_timestamp",
        "_comment_tron_route_canary_block_timestamp",
        label="TRON route canary block timestamp",
        positive=False,
    )
    _check_route_canary_log_index_comment_matches_record(
        errors,
        record,
        "tron_route_canary_log_index",
        "_comment_tron_route_canary_log_index",
        label="TRON route canary log index",
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_message_id",
        "_comment_tron_route_canary_message_id",
        label="TRON route canary message id",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_call_data_sha256",
        "_comment_tron_route_canary_call_data_sha256",
        label="TRON route canary call data SHA-256",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_payload_hash",
        "_comment_tron_route_canary_payload_hash",
        label="TRON route canary payload hash",
        byte_length=32,
    )
    _check_route_canary_log_index_comment_matches_record(
        errors,
        record,
        "tron_route_canary_target_domain",
        "_comment_tron_route_canary_target_domain",
        label="TRON route canary target domain",
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_statement_hash",
        "_comment_tron_route_canary_statement_hash",
        label="TRON route canary statement hash",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_commitment_root",
        "_comment_tron_route_canary_commitment_root",
        label="TRON route canary commitment root",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_finality_height",
        "_comment_tron_route_canary_finality_height",
        label="TRON route canary finality height",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "tron_route_canary_finality_block_hash",
        "_comment_tron_route_canary_finality_block_hash",
        label="TRON route canary finality block hash",
        byte_length=32,
    )
    _check_route_canary_log_index_comment_matches_record(
        errors,
        record,
        "tron_route_canary_proof_version",
        "_comment_tron_route_canary_proof_version",
        label="TRON route canary proof version",
    )
    _check_route_canary_log_index_comment_matches_record(
        errors,
        record,
        "tron_route_canary_proof_source_domain",
        "_comment_tron_route_canary_proof_source_domain",
        label="TRON route canary proof source domain",
    )
    _check_route_canary_bool_comment_matches_record(
        errors,
        record,
        "tron_route_canary_used_message_proof",
        "_comment_tron_route_canary_used_message_proof",
        label="TRON route canary usedMessageProofs",
    )
    _check_route_canary_bool_comment_matches_record(
        errors,
        record,
        "tron_route_canary_raw_data_owner_matches_transaction",
        "_comment_tron_route_canary_raw_data_owner_matches_transaction",
        label="TRON route canary raw_data owner binding",
    )
    transaction_id = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_transaction_id",
            "_comment_tron_route_canary_transaction_id",
        ),
        byte_length=32,
    )
    if transaction_id is None or not any(transaction_id):
        errors.append(
            "TRON route canary transaction id metadata must be a non-zero bytes32"
        )
    transaction_owner_address = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_transaction_owner_address",
            "_comment_tron_route_canary_transaction_owner_address",
        ),
        byte_length=21,
    )
    if (
        transaction_owner_address is None
        or transaction_owner_address[0] != 0x41
        or not any(transaction_owner_address[1:])
    ):
        errors.append(
            "TRON route canary transaction owner address metadata must be a "
            "non-zero 0x41-prefixed TRON address"
        )
    block_number = _canonical_decimal_int(
        _first_record_value(
            record,
            "tron_route_canary_block_number",
            "_comment_tron_route_canary_block_number",
        ),
        positive=True,
    )
    if block_number is None:
        errors.append(
            "TRON route canary block number metadata must be a canonical positive decimal"
        )
    block_timestamp = _canonical_decimal_int(
        _first_record_value(
            record,
            "tron_route_canary_block_timestamp",
            "_comment_tron_route_canary_block_timestamp",
        ),
        positive=False,
    )
    if block_timestamp is None:
        errors.append(
            "TRON route canary block timestamp metadata must be a canonical decimal"
        )
    log_index = _canonical_route_canary_log_index(
        _first_record_value(
            record,
            "tron_route_canary_log_index",
            "_comment_tron_route_canary_log_index",
        )
    )
    if log_index is None:
        errors.append(
            "TRON route canary log index metadata must be a canonical u32"
        )
    message_id = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_message_id",
            "_comment_tron_route_canary_message_id",
        ),
        byte_length=32,
    )
    if message_id is None or not any(message_id):
        errors.append("TRON route canary message id metadata must be a non-zero bytes32")
    call_data_sha256 = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_call_data_sha256",
            "_comment_tron_route_canary_call_data_sha256",
        ),
        byte_length=32,
    )
    if call_data_sha256 is None or not any(call_data_sha256):
        errors.append(
            "TRON route canary call data hash metadata must be a non-zero bytes32"
        )
    payload_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_payload_hash",
            "_comment_tron_route_canary_payload_hash",
        ),
        byte_length=32,
    )
    if payload_hash is None or not any(payload_hash):
        errors.append(
            "TRON route canary payload hash metadata must be a non-zero bytes32"
        )
    target_domain = _canonical_route_canary_log_index(
        _first_record_value(
            record,
            "tron_route_canary_target_domain",
            "_comment_tron_route_canary_target_domain",
        )
    )
    if target_domain != SCCP_DOMAIN_TRON:
        errors.append("TRON route canary target domain metadata must be TRON")
    statement_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_statement_hash",
            "_comment_tron_route_canary_statement_hash",
        ),
        byte_length=32,
    )
    if statement_hash is None or not any(statement_hash):
        errors.append(
            "TRON route canary statement hash metadata must be a non-zero bytes32"
        )
    commitment_root = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_commitment_root",
            "_comment_tron_route_canary_commitment_root",
        ),
        byte_length=32,
    )
    if commitment_root is None or not any(commitment_root):
        errors.append(
            "TRON route canary commitment root metadata must be a non-zero bytes32"
        )
    finality_height = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_finality_height",
            "_comment_tron_route_canary_finality_height",
        ),
        byte_length=32,
    )
    if finality_height is None or not any(finality_height):
        errors.append(
            "TRON route canary finality height metadata must be a non-zero bytes32"
        )
    finality_block_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_finality_block_hash",
            "_comment_tron_route_canary_finality_block_hash",
        ),
        byte_length=32,
    )
    if finality_block_hash is None or not any(finality_block_hash):
        errors.append(
            "TRON route canary finality block hash metadata must be a non-zero bytes32"
        )
    proof_version = _canonical_route_canary_log_index(
        _first_record_value(
            record,
            "tron_route_canary_proof_version",
            "_comment_tron_route_canary_proof_version",
        )
    )
    if proof_version != 1:
        errors.append("TRON route canary proof version metadata must be 1")
    proof_source_domain = _canonical_route_canary_log_index(
        _first_record_value(
            record,
            "tron_route_canary_proof_source_domain",
            "_comment_tron_route_canary_proof_source_domain",
        )
    )
    if proof_source_domain != SCCP_DOMAIN_SORA:
        errors.append("TRON route canary proof source domain metadata must be SORA")
    used_message_proof = _route_canary_used_message_proof_value(
        _first_record_value(
            record,
            "tron_route_canary_used_message_proof",
            "_comment_tron_route_canary_used_message_proof",
        )
    )
    if used_message_proof is not True:
        errors.append(
            "TRON route canary usedMessageProofs metadata must be true"
        )
    raw_data_owner_matches_transaction = _route_canary_used_message_proof_value(
        _first_record_value(
            record,
            "tron_route_canary_raw_data_owner_matches_transaction",
            "_comment_tron_route_canary_raw_data_owner_matches_transaction",
        )
    )
    if raw_data_owner_matches_transaction is not True:
        errors.append(
            "TRON route canary raw_data owner binding metadata must be true"
        )
    signature_sha256 = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_signature_sha256",
            "_comment_tron_route_canary_signature_sha256",
        ),
        byte_length=32,
    )
    if signature_sha256 is None or not any(signature_sha256):
        errors.append(
            "TRON route canary signature hash metadata must be a non-zero bytes32"
        )
    signature_recovered_address = _exact_hex_bytes(
        _first_record_value(
            record,
            "tron_route_canary_signature_recovered_address",
            "_comment_tron_route_canary_signature_recovered_address",
        ),
        byte_length=21,
    )
    if (
        signature_recovered_address is None
        or signature_recovered_address[0] != 0x41
        or not any(signature_recovered_address[1:])
    ):
        errors.append(
            "TRON route canary signature recovered address metadata must be a "
            "non-zero 0x41-prefixed TRON address"
        )
    signature_recovers_to_owner = _route_canary_used_message_proof_value(
        _first_record_value(
            record,
            "tron_route_canary_signature_recovers_to_owner",
            "_comment_tron_route_canary_signature_recovers_to_owner",
        )
    )
    if signature_recovers_to_owner is not True:
        errors.append(
            "TRON route canary signature recovery metadata must be true"
        )
    if (
        transaction_owner_address is not None
        and signature_recovered_address is not None
        and transaction_owner_address != signature_recovered_address
    ):
        errors.append(
            "TRON route canary signature recovered address must match transaction owner"
        )
    _expect_distinct_byte_values(
        errors,
        (
            ("tron_route_canary_transaction_id", transaction_id),
            ("tron_route_canary_message_id", message_id),
            ("tron_route_canary_call_data_sha256", call_data_sha256),
            ("tron_route_canary_payload_hash", payload_hash),
            ("tron_route_canary_statement_hash", statement_hash),
            ("tron_route_canary_commitment_root", commitment_root),
            ("tron_route_canary_finality_height", finality_height),
            ("tron_route_canary_finality_block_hash", finality_block_hash),
            ("tron_route_canary_signature_sha256", signature_sha256),
        ),
        label="TRON route canary transcript hash",
    )
    if evidence_hash is None or route_allowlist_hash is None:
        errors.append(
            "TRON route canary transaction evidence requires canary and route hashes"
        )
    if destination_binding_hash is None:
        errors.append(
            "TRON route canary transaction evidence requires destination binding hash"
        )
    source_material_hash = _hex_bytes(
        source_record_hashes.get("source_verifier_material_hash"),
        byte_length=32,
    )
    source_deployment_hash = _hex_bytes(
        source_record_hashes.get("source_adapter_engine_deployment_hash"),
        byte_length=32,
    )
    if source_material_hash is None or source_deployment_hash is None:
        errors.append("TRON route canary transaction evidence requires source record hashes")
    _expect_distinct_byte_values(
        errors,
        (
            ("route_allowlist_hash", route_allowlist_hash),
            ("destination_binding_hash", destination_binding_hash),
            ("source_verifier_material_hash", source_material_hash),
            ("source_adapter_engine_deployment_hash", source_deployment_hash),
            ("tron_route_canary_transaction_id", transaction_id),
            ("tron_route_canary_message_id", message_id),
            ("tron_route_canary_call_data_sha256", call_data_sha256),
            ("tron_route_canary_payload_hash", payload_hash),
            ("tron_route_canary_statement_hash", statement_hash),
            ("tron_route_canary_commitment_root", commitment_root),
            ("tron_route_canary_finality_height", finality_height),
            ("tron_route_canary_finality_block_hash", finality_block_hash),
            ("tron_route_canary_signature_sha256", signature_sha256),
        ),
        label="TRON route canary hash role",
    )

    module = _load_sibling_module("sccp_tron_source_bridge_evidence.py")
    try:
        verifier_address = module.parse_tron_address(
            _first_record_value(
                destination_record,
                "_comment_tron_destination_verifier_address",
                "verifier_identity",
            ),
            label="TRON destination verifier address",
        )
    except (argparse.ArgumentTypeError, ValueError):
        errors.append("TRON route canary verifier address metadata is invalid")
        verifier_address = None
    network_id = _exact_hex_bytes(
        _first_record_value(
            destination_record,
            "destination_network_id",
            "_comment_destination_network_id",
        ),
        byte_length=32,
    )
    if network_id is None or not any(network_id):
        errors.append("TRON route canary network id metadata must be a non-zero bytes32")
    verifier_backend_hash = _exact_hex_bytes(
        destination_record.get("_comment_tron_destination_verifier_backend_hash"),
        byte_length=32,
    )
    if verifier_backend_hash is None or not any(verifier_backend_hash):
        errors.append(
            "TRON route canary verifier backend hash metadata must be a non-zero bytes32"
        )
    proof_family_hash = _exact_hex_bytes(
        destination_record.get("_comment_tron_destination_proof_family_hash"),
        byte_length=32,
    )
    if proof_family_hash is None or not any(proof_family_hash):
        errors.append(
            "TRON route canary proof family hash metadata must be a non-zero bytes32"
        )
    if errors:
        return errors
    assert transaction_id is not None
    assert transaction_owner_address is not None
    assert block_number is not None
    assert block_timestamp is not None
    assert log_index is not None
    assert verifier_address is not None
    assert message_id is not None
    assert call_data_sha256 is not None
    assert payload_hash is not None
    assert target_domain == SCCP_DOMAIN_TRON
    assert statement_hash is not None
    assert commitment_root is not None
    assert finality_height is not None
    assert finality_block_hash is not None
    assert proof_version == 1
    assert proof_source_domain == SCCP_DOMAIN_SORA
    assert used_message_proof is True
    assert raw_data_owner_matches_transaction is True
    assert signature_sha256 is not None
    assert signature_recovered_address is not None
    assert signature_recovers_to_owner is True
    assert route_allowlist_hash is not None
    assert destination_binding_hash is not None
    assert verifier_backend_hash is not None
    assert proof_family_hash is not None
    assert network_id is not None
    payload = bytearray()
    _push_u8(payload, 3)
    payload.extend(route_allowlist_hash)
    payload.extend(b"\x41" + verifier_address)
    payload.extend(transaction_id)
    payload.extend(transaction_owner_address)
    _push_u64(payload, block_number)
    _push_u64(payload, block_timestamp)
    _push_u32(payload, log_index)
    payload.extend(call_data_sha256)
    payload.extend(message_id)
    _push_u32(payload, SCCP_DOMAIN_SORA)
    _push_u32(payload, target_domain)
    payload.extend(payload_hash)
    payload.extend(commitment_root)
    payload.extend(finality_height)
    payload.extend(finality_block_hash)
    payload.extend(statement_hash)
    _push_u32(payload, proof_version)
    _push_u32(payload, proof_source_domain)
    payload.extend(destination_binding_hash)
    payload.extend(verifier_backend_hash)
    payload.extend(proof_family_hash)
    payload.extend(network_id)
    _push_u8(payload, 1 if used_message_proof is True else 0)
    _push_u8(payload, 1 if raw_data_owner_matches_transaction is True else 0)
    payload.extend(signature_sha256)
    payload.extend(signature_recovered_address)
    _push_u8(payload, 1 if signature_recovers_to_owner is True else 0)
    expected_hash = _prefixed_blake2b(
        b"iroha:sccp:tron-route-canary-evidence:v3",
        payload,
    )
    if evidence_hash != expected_hash:
        errors.append(
            "TRON route canary evidence hash must match "
            "MessageProofAccepted transaction metadata"
        )
    else:
        canary["evidence_source"] = "tron_message_proof_accepted_transaction"
        canary["transaction_id"] = _hex(transaction_id)
        canary["transaction_owner_address"] = _hex(transaction_owner_address)
        canary["block_number"] = block_number
        canary["block_timestamp"] = block_timestamp
        canary["log_index"] = log_index
        canary["message_id"] = _hex(message_id)
        canary["call_data_sha256"] = _hex(call_data_sha256)
        canary["payload_hash"] = _hex(payload_hash)
        canary["target_domain"] = target_domain
        canary["statement_hash"] = _hex(statement_hash)
        canary["commitment_root"] = _hex(commitment_root)
        canary["finality_height"] = _hex(finality_height)
        canary["finality_block_hash"] = _hex(finality_block_hash)
        canary["proof_version"] = proof_version
        canary["proof_source_domain"] = proof_source_domain
        canary["message_proof_used"] = True
        canary["raw_data_owner_matches_transaction"] = True
        canary["signature_sha256"] = _hex(signature_sha256)
        canary["signature_recovered_address"] = _hex(signature_recovered_address)
        canary["signature_recovers_to_owner"] = True
    return errors


def _check_ton_route_canary_live_account_evidence(
    record: dict[str, Any],
    *,
    destination_record: dict[str, Any] | None,
    source_record_hashes: dict[str, str],
    evidence_hash: bytes | None,
    route_allowlist_hash: bytes | None,
    destination_binding_hash: bytes | None,
    canary: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    for field, comment, label, byte_length in (
        (
            "ton_route_canary_account_state_hash",
            "_comment_ton_route_canary_account_state_hash",
            "ton_route_canary_account_state_hash",
            32,
        ),
        (
            "ton_route_canary_last_transaction_hash",
            "_comment_ton_route_canary_last_transaction_hash",
            "ton_route_canary_last_transaction_hash",
            32,
        ),
    ):
        _check_hex_comment_matches_record(
            errors,
            record,
            field,
            comment,
            label=label,
            byte_length=byte_length,
        )
    _check_string_comment_matches_record(
        errors,
        record,
        "ton_route_canary_last_transaction_lt",
        "_comment_ton_route_canary_last_transaction_lt",
        label="ton_route_canary_last_transaction_lt",
    )
    if destination_record is None:
        return ["TON route canary live-account evidence requires destination rollout"]
    if evidence_hash is None or route_allowlist_hash is None:
        errors.append("TON route canary evidence requires canary and route hashes")
    if destination_binding_hash is None:
        errors.append("TON route canary evidence requires destination binding hash")

    account_status = _first_record_value(
        destination_record,
        "ton_account_status",
        "_comment_ton_account_status",
    )
    if account_status != "active":
        errors.append(
            "TON route canary account status must match active destination rollout"
        )
    account_state_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "ton_route_canary_account_state_hash",
            "_comment_ton_route_canary_account_state_hash",
        ),
        byte_length=32,
    )
    if account_state_hash is None or not any(account_state_hash):
        errors.append(
            "TON route canary account state hash must be a non-zero bytes32"
        )
    last_transaction_lt = _first_record_value(
        record,
        "ton_route_canary_last_transaction_lt",
        "_comment_ton_route_canary_last_transaction_lt",
    )
    if not _is_canonical_decimal_text(last_transaction_lt, positive=True):
        errors.append(
            "TON route canary last transaction LT must be a canonical positive decimal"
        )
    last_transaction_hash = _exact_hex_bytes(
        _first_record_value(
            record,
            "ton_route_canary_last_transaction_hash",
            "_comment_ton_route_canary_last_transaction_hash",
        ),
        byte_length=32,
    )
    if last_transaction_hash is None or not any(last_transaction_hash):
        errors.append(
            "TON route canary last transaction hash must be a non-zero bytes32"
        )
    if (
        account_state_hash is not None
        and last_transaction_hash is not None
        and account_state_hash == last_transaction_hash
    ):
        errors.append(
            "TON route canary account state hash must differ from last transaction hash"
        )

    destination_account_state_hash = _exact_hex_bytes(
        _first_record_value(
            destination_record,
            "ton_account_state_hash",
            "_comment_ton_account_state_hash",
        ),
        byte_length=32,
    )
    if destination_account_state_hash != account_state_hash:
        errors.append(
            "TON route canary account state hash must match destination rollout live account state"
        )
    destination_last_transaction_lt = _first_record_value(
        destination_record,
        "ton_last_transaction_lt",
        "_comment_ton_last_transaction_lt",
    )
    if destination_last_transaction_lt != last_transaction_lt:
        errors.append(
            "TON route canary last transaction LT must match destination rollout live account LT"
        )
    destination_last_transaction_hash = _exact_hex_bytes(
        _first_record_value(
            destination_record,
            "ton_last_transaction_hash",
            "_comment_ton_last_transaction_hash",
        ),
        byte_length=32,
    )
    if destination_last_transaction_hash != last_transaction_hash:
        errors.append(
            "TON route canary last transaction hash must match destination rollout live account transaction"
        )

    module = _load_sibling_module("sccp_ton_destination_evidence.py")
    try:
        verifier_identity = module.normalize_ton_raw_address(
            str(destination_record.get("verifier_identity")),
            label="TON route canary verifier identity",
        )
    except (argparse.ArgumentTypeError, ValueError):
        errors.append("TON route canary verifier identity is invalid")
        verifier_identity = None
    verifier_code_hash = _exact_hex_bytes(
        destination_record.get("verifier_code_hash"),
        byte_length=32,
    )
    if verifier_code_hash is None or not any(verifier_code_hash):
        errors.append("TON route canary verifier code hash must be a non-zero bytes32")
    verifier_code_boc_root_hash = _exact_hex_bytes(
        _first_record_value(
            destination_record,
            "ton_verifier_code_boc_root_hash",
            "_comment_ton_code_boc_root_hash",
        ),
        byte_length=32,
    )
    if verifier_code_boc_root_hash is None or not any(verifier_code_boc_root_hash):
        errors.append(
            "TON route canary verifier code BoC root hash must be a non-zero bytes32"
        )
    elif verifier_code_hash is not None and verifier_code_boc_root_hash != verifier_code_hash:
        errors.append(
            "TON route canary verifier code BoC root hash must match verifier_code_hash"
        )
    source_material_hash = _hex_bytes(
        source_record_hashes.get("source_verifier_material_hash"),
        byte_length=32,
    )
    source_deployment_hash = _hex_bytes(
        source_record_hashes.get("source_adapter_engine_deployment_hash"),
        byte_length=32,
    )
    if source_material_hash is None or source_deployment_hash is None:
        errors.append("TON route canary evidence requires source record hashes")
    _expect_distinct_byte_values(
        errors,
        (
            ("route_allowlist_hash", route_allowlist_hash),
            ("destination_binding_hash", destination_binding_hash),
            ("source_verifier_material_hash", source_material_hash),
            ("source_adapter_engine_deployment_hash", source_deployment_hash),
            ("verifier_code_hash", verifier_code_hash),
            ("ton_route_canary_account_state_hash", account_state_hash),
            ("ton_route_canary_last_transaction_hash", last_transaction_hash),
        ),
        label="TON route canary hash role",
    )
    if errors:
        return errors

    assert evidence_hash is not None
    assert route_allowlist_hash is not None
    assert destination_binding_hash is not None
    assert source_material_hash is not None
    assert source_deployment_hash is not None
    assert verifier_identity is not None
    assert verifier_code_hash is not None
    assert account_status == "active"
    assert account_state_hash is not None
    assert isinstance(last_transaction_lt, str)
    assert last_transaction_hash is not None
    assert verifier_code_boc_root_hash is not None
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_SORA)
    _push_u32(payload, SCCP_DOMAIN_TON)
    payload.extend(route_allowlist_hash)
    payload.extend(destination_binding_hash)
    payload.extend(source_material_hash)
    payload.extend(source_deployment_hash)
    _push_vec(payload, verifier_identity.encode("utf-8"))
    payload.extend(verifier_code_hash)
    _push_vec(payload, account_status.encode("ascii"))
    payload.extend(account_state_hash)
    _push_vec(payload, last_transaction_lt.encode("ascii"))
    payload.extend(last_transaction_hash)
    payload.extend(verifier_code_boc_root_hash)
    expected_hash = _prefixed_blake2b(
        b"iroha:sccp:ton-route-canary-live-account:v1",
        payload,
    )
    if evidence_hash != expected_hash:
        errors.append(
            "TON route canary evidence hash must match live account route canary metadata"
        )
    else:
        canary["evidence_source"] = "ton_live_account_snapshot"
        canary["ton_account_state_hash"] = _hex(account_state_hash)
        canary["ton_last_transaction_lt"] = last_transaction_lt
        canary["ton_last_transaction_hash"] = _hex(last_transaction_hash)
    return errors


def _check_solana_route_canary_live_program_evidence(
    record: dict[str, Any],
    *,
    destination_record: dict[str, Any] | None,
    source_record_hashes: dict[str, str],
    evidence_hash: bytes | None,
    route_allowlist_hash: bytes | None,
    destination_binding_hash: bytes | None,
    canary: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    if destination_record is None:
        return ["Solana route canary live-program evidence requires destination rollout"]
    if evidence_hash is None or route_allowlist_hash is None:
        errors.append("Solana route canary evidence requires canary and route hashes")
    if destination_binding_hash is None:
        errors.append("Solana route canary evidence requires destination binding hash")

    source_material_hash = _hex_bytes(
        source_record_hashes.get("source_verifier_material_hash"),
        byte_length=32,
    )
    source_deployment_hash = _hex_bytes(
        source_record_hashes.get("source_adapter_engine_deployment_hash"),
        byte_length=32,
    )
    if source_material_hash is None or source_deployment_hash is None:
        errors.append("Solana route canary evidence requires source record hashes")

    verifier_code_hash = _hex_bytes(
        destination_record.get("verifier_code_hash"),
        byte_length=32,
    )
    if verifier_code_hash is None or not any(verifier_code_hash):
        errors.append(
            "Solana route canary verifier code hash must be a non-zero bytes32"
        )

    def parse_positive_u64(field: str, label: str) -> int | None:
        value = destination_record.get(field)
        if not _is_canonical_decimal_text(value, positive=True):
            errors.append(f"{label} must be a canonical positive decimal")
            return None
        return int(value, 10)

    def decode_base64(field: str, label: str) -> bytes | None:
        value = destination_record.get(field)
        if not _is_nonempty_string(value):
            errors.append(f"{label} must be present")
            return None
        try:
            return _decode_canonical_base64(value, label=label)
        except ValueError:
            errors.append(f"{label} is invalid")
            return None

    programdata_slot = parse_positive_u64(
        "_comment_solana_programdata_slot",
        "Solana route canary ProgramData slot",
    )
    expected_programdata_slot = parse_positive_u64(
        "_comment_solana_expected_programdata_slot",
        "Solana route canary expected ProgramData slot",
    )
    program_account_context_slot = parse_positive_u64(
        "_comment_solana_program_account_context_slot",
        "Solana route canary program account context slot",
    )
    programdata_account_context_slot = parse_positive_u64(
        "_comment_solana_programdata_account_context_slot",
        "Solana route canary ProgramData account context slot",
    )
    program_account_data = decode_base64(
        "_comment_solana_program_account_data_base64",
        "Solana route canary Program account data",
    )
    programdata_metadata = decode_base64(
        "_comment_solana_programdata_metadata_base64",
        "Solana route canary ProgramData metadata",
    )
    module = _load_sibling_module("sccp_solana_destination_evidence.py")
    programdata_executable: bytes | None = None
    programdata_executable_base64 = destination_record.get(
        "_comment_solana_programdata_executable_base64"
    )
    if not _is_nonempty_string(programdata_executable_base64):
        errors.append("Solana route canary ProgramData executable must be present")
    else:
        try:
            programdata_executable = module.parse_program_bytes_base64(
                programdata_executable_base64,
                label="Solana route canary ProgramData executable",
            )
        except (argparse.ArgumentTypeError, ValueError):
            errors.append("Solana route canary ProgramData executable is invalid")

    _expect_distinct_byte_values(
        errors,
        (
            ("route_allowlist_hash", route_allowlist_hash),
            ("destination_binding_hash", destination_binding_hash),
            ("source_verifier_material_hash", source_material_hash),
            ("source_adapter_engine_deployment_hash", source_deployment_hash),
            ("verifier_code_hash", verifier_code_hash),
        ),
        label="Solana route canary hash role",
    )

    if errors:
        return errors

    assert evidence_hash is not None
    assert route_allowlist_hash is not None
    assert destination_binding_hash is not None
    assert source_material_hash is not None
    assert source_deployment_hash is not None
    assert verifier_code_hash is not None
    assert programdata_slot is not None
    assert expected_programdata_slot is not None
    assert program_account_context_slot is not None
    assert programdata_account_context_slot is not None
    assert program_account_data is not None
    assert programdata_metadata is not None
    assert programdata_executable is not None

    try:
        expected_hash = module.solana_route_canary_evidence_hash(
            route_allowlist_hash=route_allowlist_hash,
            destination_binding_hash=destination_binding_hash,
            source_verifier_material_hash=source_material_hash,
            source_adapter_engine_deployment_hash=source_deployment_hash,
            verifier_program_id=str(destination_record.get("verifier_identity")),
            verifier_code_hash=verifier_code_hash,
            rpc_commitment=str(destination_record.get("_comment_solana_rpc_commitment")),
            program_owner=str(destination_record.get("_comment_solana_program_owner")),
            programdata_owner=str(
                destination_record.get("_comment_solana_programdata_owner")
            ),
            program_immutable=(
                destination_record.get("_comment_solana_program_immutable") == "true"
            ),
            program_account_data=program_account_data,
            programdata_address=str(
                destination_record.get("_comment_solana_programdata_address")
            ),
            programdata_slot=programdata_slot,
            expected_programdata_slot=expected_programdata_slot,
            program_account_context_slot=program_account_context_slot,
            programdata_account_context_slot=programdata_account_context_slot,
            programdata_metadata=programdata_metadata,
            programdata_executable=programdata_executable,
        )
    except (argparse.ArgumentTypeError, ValueError):
        errors.append("Solana route canary live program metadata is invalid")
        return errors
    if evidence_hash != expected_hash:
        errors.append(
            "Solana route canary evidence hash must match live program metadata"
        )
    else:
        canary["evidence_source"] = "solana_live_programdata_snapshot"
        canary["solana_programdata_address"] = str(
            destination_record.get("_comment_solana_programdata_address")
        )
        canary["solana_programdata_slot"] = str(programdata_slot)
    return errors

def _check_route_canary_evidence(
    profile: LaneProfile,
    record: dict[str, Any],
    *,
    supplied_hash_raw: bytes | None,
    source_record_hashes: dict[str, str],
    destination_binding: dict[str, Any],
    destination_record: dict[str, Any] | None,
    summary: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    canary: dict[str, Any] = {}
    _check_string_comment_matches_record(
        errors,
        record,
        "route_canary_status",
        "_comment_route_canary_status",
        label="route_canary_status",
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "route_canary_evidence_hash",
        "_comment_route_canary_evidence_hash",
        label="route_canary_evidence_hash",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "route_canary_route_allowlist_hash",
        "_comment_route_canary_route_allowlist_hash",
        label="route_canary_route_allowlist_hash",
        byte_length=32,
    )
    _check_hex_comment_matches_record(
        errors,
        record,
        "route_canary_destination_binding_hash",
        "_comment_route_canary_destination_binding_hash",
        label="route_canary_destination_binding_hash",
        byte_length=32,
    )

    status = record.get(
        "route_canary_status",
        record.get("_comment_route_canary_status"),
    )
    canary["status"] = status
    if status != "passed":
        errors.append("route canary status metadata must be passed")

    evidence_hash = _hex_bytes(
        record.get(
            "route_canary_evidence_hash",
            record.get("_comment_route_canary_evidence_hash"),
        ),
        byte_length=32,
    )
    if evidence_hash is None or not any(evidence_hash):
        errors.append(
            "route canary evidence hash metadata must be a non-zero bytes32"
        )
    else:
        canary["evidence_hash"] = _hex(evidence_hash)

    canary_route_hash = _hex_bytes(
        record.get(
            "route_canary_route_allowlist_hash",
            record.get("_comment_route_canary_route_allowlist_hash"),
        ),
        byte_length=32,
    )
    if canary_route_hash is None or not any(canary_route_hash):
        errors.append(
            "route canary route allowlist hash metadata must be a non-zero bytes32"
        )
    else:
        canary["route_allowlist_hash"] = _hex(canary_route_hash)
        if supplied_hash_raw is not None and canary_route_hash != supplied_hash_raw:
            errors.append(
                "route canary route allowlist hash must match route_allowlist_hash"
            )

    canary_destination_binding_hash = _hex_bytes(
        record.get(
            "route_canary_destination_binding_hash",
            record.get("_comment_route_canary_destination_binding_hash"),
        ),
        byte_length=32,
    )
    expected_destination_binding_hash = _hex_bytes(
        destination_binding.get("destination_binding_hash"),
        byte_length=32,
    )
    if canary_destination_binding_hash is None or not any(
        canary_destination_binding_hash
    ):
        errors.append(
            "route canary destination binding hash metadata must be a non-zero bytes32"
        )
    else:
        canary["destination_binding_hash"] = _hex(canary_destination_binding_hash)
        if (
            expected_destination_binding_hash is not None
            and canary_destination_binding_hash != expected_destination_binding_hash
        ):
            errors.append(
                "route canary destination binding hash must match destination_binding_hash"
            )

    if evidence_hash is not None and any(evidence_hash):
        source_material_hash = _hex_bytes(
            source_record_hashes.get("source_verifier_material_hash"),
            byte_length=32,
        )
        if source_material_hash is not None and evidence_hash == source_material_hash:
            errors.append(
                "route canary evidence hash must be distinct from "
                "source_verifier_material_hash"
            )
        source_deployment_hash = _hex_bytes(
            source_record_hashes.get("source_adapter_engine_deployment_hash"),
            byte_length=32,
        )
        if (
            source_deployment_hash is not None
            and evidence_hash == source_deployment_hash
        ):
            errors.append(
                "route canary evidence hash must be distinct from "
                "source_adapter_engine_deployment_hash"
            )
        if canary_route_hash is not None and evidence_hash == canary_route_hash:
            errors.append(
                "route canary evidence hash must be distinct from route_allowlist_hash"
            )
        if (
            canary_destination_binding_hash is not None
            and evidence_hash == canary_destination_binding_hash
        ):
            errors.append(
                "route canary evidence hash must be distinct from destination_binding_hash"
            )

    if profile.chain in ("eth", "bsc"):
        errors.extend(
            _check_evm_route_canary_transaction_evidence(
                record,
                destination_record=destination_record,
                source_record_hashes=source_record_hashes,
                evidence_hash=evidence_hash,
                route_allowlist_hash=canary_route_hash,
                destination_binding_hash=canary_destination_binding_hash,
                canary=canary,
            )
        )
    elif profile.chain == "tron":
        errors.extend(
            _check_tron_route_canary_transaction_evidence(
                record,
                destination_record=destination_record,
                source_record_hashes=source_record_hashes,
                evidence_hash=evidence_hash,
                route_allowlist_hash=canary_route_hash,
                destination_binding_hash=canary_destination_binding_hash,
                canary=canary,
            )
        )
    elif profile.chain == "ton":
        errors.extend(
            _check_ton_route_canary_live_account_evidence(
                record,
                destination_record=destination_record,
                source_record_hashes=source_record_hashes,
                evidence_hash=evidence_hash,
                route_allowlist_hash=canary_route_hash,
                destination_binding_hash=canary_destination_binding_hash,
                canary=canary,
            )
        )
    elif profile.chain == "sol":
        errors.extend(
            _check_solana_route_canary_live_program_evidence(
                record,
                destination_record=destination_record,
                source_record_hashes=source_record_hashes,
                evidence_hash=evidence_hash,
                route_allowlist_hash=canary_route_hash,
                destination_binding_hash=canary_destination_binding_hash,
                canary=canary,
            )
        )
    for field in (
        "ton_route_canary_account_state_hash",
        "ton_route_canary_last_transaction_lt",
        "ton_route_canary_last_transaction_hash",
        "_comment_ton_route_canary_account_state_hash",
        "_comment_ton_route_canary_last_transaction_lt",
        "_comment_ton_route_canary_last_transaction_hash",
    ):
        if profile.chain != "ton" and record.get(field) not in (None, ""):
            errors.append(f"{field} is only valid for TON route canary evidence")
    for field in (
        "tron_route_canary_transaction_id",
        "tron_route_canary_transaction_owner_address",
        "tron_route_canary_log_index",
        "tron_route_canary_message_id",
        "tron_route_canary_call_data_sha256",
        "tron_route_canary_payload_hash",
        "tron_route_canary_target_domain",
        "tron_route_canary_statement_hash",
        "tron_route_canary_commitment_root",
        "tron_route_canary_finality_height",
        "tron_route_canary_finality_block_hash",
        "tron_route_canary_proof_version",
        "tron_route_canary_proof_source_domain",
        "tron_route_canary_used_message_proof",
        "tron_route_canary_raw_data_owner_matches_transaction",
        "tron_route_canary_signature_sha256",
        "tron_route_canary_signature_recovered_address",
        "tron_route_canary_signature_recovers_to_owner",
        "_comment_tron_route_canary_transaction_id",
        "_comment_tron_route_canary_transaction_owner_address",
        "_comment_tron_route_canary_log_index",
        "_comment_tron_route_canary_message_id",
        "_comment_tron_route_canary_call_data_sha256",
        "_comment_tron_route_canary_payload_hash",
        "_comment_tron_route_canary_target_domain",
        "_comment_tron_route_canary_statement_hash",
        "_comment_tron_route_canary_commitment_root",
        "_comment_tron_route_canary_finality_height",
        "_comment_tron_route_canary_finality_block_hash",
        "_comment_tron_route_canary_proof_version",
        "_comment_tron_route_canary_proof_source_domain",
        "_comment_tron_route_canary_used_message_proof",
        "_comment_tron_route_canary_raw_data_owner_matches_transaction",
        "_comment_tron_route_canary_signature_sha256",
        "_comment_tron_route_canary_signature_recovered_address",
        "_comment_tron_route_canary_signature_recovers_to_owner",
    ):
        if profile.chain != "tron" and record.get(field) not in (None, ""):
            errors.append(f"{field} is only valid for TRON route canary evidence")
    for field in (
        "evm_route_canary_transaction_hash",
        "evm_route_canary_transaction_block_number",
        "evm_route_canary_transaction_block_hash",
        "evm_route_canary_log_index",
        "evm_route_canary_receipt_block_number",
        "evm_route_canary_receipt_block_hash",
        "evm_route_canary_block_receipts_root",
        "evm_route_canary_call_data_sha256",
        "evm_route_canary_message_id",
        "evm_route_canary_payload_hash",
        "evm_route_canary_target_domain",
        "evm_route_canary_statement_hash",
        "evm_route_canary_commitment_root",
        "evm_route_canary_finality_height",
        "evm_route_canary_finality_block_hash",
        "evm_route_canary_proof_version",
        "evm_route_canary_proof_source_domain",
        "evm_route_canary_used_message_proof",
        "_comment_evm_route_canary_transaction_hash",
        "_comment_evm_route_canary_transaction_block_number",
        "_comment_evm_route_canary_transaction_block_hash",
        "_comment_evm_route_canary_log_index",
        "_comment_evm_route_canary_receipt_block_number",
        "_comment_evm_route_canary_receipt_block_hash",
        "_comment_evm_route_canary_block_receipts_root",
        "_comment_evm_route_canary_call_data_sha256",
        "_comment_evm_route_canary_message_id",
        "_comment_evm_route_canary_payload_hash",
        "_comment_evm_route_canary_target_domain",
        "_comment_evm_route_canary_statement_hash",
        "_comment_evm_route_canary_commitment_root",
        "_comment_evm_route_canary_finality_height",
        "_comment_evm_route_canary_finality_block_hash",
        "_comment_evm_route_canary_proof_version",
        "_comment_evm_route_canary_proof_source_domain",
        "_comment_evm_route_canary_used_message_proof",
    ):
        if profile.chain not in ("eth", "bsc") and record.get(field) not in (None, ""):
            errors.append(f"{field} is only valid for EVM route canary evidence")

    canary["evidence_bound"] = not errors
    summary["route_canary"] = canary
    return errors


def _check_route_canary_evidence_hashes_unique(
    routes: dict[int, dict[str, Any]],
) -> list[str]:
    errors: list[str] = []
    seen: dict[bytes, int] = {}
    for domain, record in sorted(routes.items()):
        evidence_hash = _hex_bytes(
            record.get(
                "route_canary_evidence_hash",
                record.get("_comment_route_canary_evidence_hash"),
            ),
            byte_length=32,
        )
        if evidence_hash is None or not any(evidence_hash):
            continue
        previous_domain = seen.get(evidence_hash)
        if previous_domain is not None:
            errors.append(
                "route canary evidence hash for domain "
                f"{domain} must be distinct from domain {previous_domain}"
            )
        else:
            seen[evidence_hash] = domain
    return errors


def _check_route_canary_evidence_hashes_do_not_reuse_governed_hashes(
    lanes: list[dict[str, Any]],
) -> list[str]:
    errors: list[str] = []
    governed_hashes: dict[bytes, tuple[int, str]] = {}
    for lane in lanes:
        domain = lane["domain"]
        for field, value in lane.get("source_record_hashes", {}).items():
            raw = _hex_bytes(value, byte_length=32)
            if raw is not None and any(raw):
                governed_hashes.setdefault(raw, (domain, field))
        destination_binding = lane.get("destination_binding", {})
        raw_destination = _hex_bytes(
            destination_binding.get("destination_binding_hash"),
            byte_length=32,
        )
        if raw_destination is not None and any(raw_destination):
            governed_hashes.setdefault(
                raw_destination,
                (domain, "destination_binding_hash"),
            )
        route_allowlist = lane.get("route_allowlist", {})
        raw_route = _hex_bytes(
            route_allowlist.get("route_allowlist_hash"),
            byte_length=32,
        )
        if raw_route is not None and any(raw_route):
            governed_hashes.setdefault(
                raw_route,
                (domain, "route_allowlist_hash"),
            )

    for lane in lanes:
        domain = lane["domain"]
        route = lane.get("route_allowlist", {})
        canary = route.get("route_canary", {})
        evidence_hash = _hex_bytes(canary.get("evidence_hash"), byte_length=32)
        if evidence_hash is None or not any(evidence_hash):
            continue
        governed = governed_hashes.get(evidence_hash)
        if governed is None:
            continue
        governed_domain, governed_field = governed
        if governed_domain != domain:
            errors.append(
                "route canary evidence hash for domain "
                f"{domain} must be distinct from {governed_field} for "
                f"domain {governed_domain}"
            )
    return errors


def _route_canary_error_target_domain(error: str) -> int | None:
    prefix = "route canary evidence hash for domain "
    if not error.startswith(prefix):
        return None
    domain_text = error[len(prefix) :].split(" ", 1)[0]
    if not _is_canonical_decimal_text(domain_text, positive=False):
        return None
    return int(domain_text, 10)


def _attach_route_canary_cross_lane_errors(
    lanes: list[dict[str, Any]],
    errors: list[str],
) -> list[str]:
    lanes_by_domain = {lane["domain"]: lane for lane in lanes}
    unmatched: list[str] = []
    for error in errors:
        target_domain = _route_canary_error_target_domain(error)
        lane = lanes_by_domain.get(target_domain)
        if lane is None:
            unmatched.append(error)
        else:
            lane["blockers"].append(error)
    return unmatched


def _release_checklist_item(
    item_id: str,
    title: str,
    blockers: list[str],
) -> dict[str, Any]:
    return {
        "id": item_id,
        "title": title,
        "ready": not blockers,
        "blockers": blockers,
    }


def _source_adapter_gate_requirements(
    domain: Any,
) -> tuple[str, tuple[str, ...]]:
    if not isinstance(domain, int):
        return "", ()
    profile = LANE_PROFILES.get(domain)
    if profile is None:
        return "", ()
    if profile.evm_source_gate_required:
        return "evm_source_gate_hash", EVM_SOURCE_GATE_FIELDS
    if profile.solana_full_light_client_audit_required:
        return "solana_full_light_client_gate_hash", SOLANA_FULL_LIGHT_CLIENT_AUDIT_FIELDS
    if profile.ton_full_light_client_audit_required:
        return "ton_full_light_client_gate_hash", TON_FULL_LIGHT_CLIENT_AUDIT_FIELDS
    if profile.tron_source_bridge_config_required:
        return "tron_dpos_source_gate_hash", TRON_DPOS_SOURCE_GATE_FIELDS
    return "", ()


def _source_adapter_gate_release_metadata_blockers(
    lane_label: str,
    lane: dict[str, Any],
    source_adapter_gate: dict[str, Any],
) -> list[str]:
    gate_field, expected_audit_fields = _source_adapter_gate_requirements(
        lane.get("domain")
    )
    gate_required = source_adapter_gate.get("required")
    blockers: list[str] = []
    if gate_required != bool(expected_audit_fields):
        blockers.append(
            f"{lane_label}: source adapter gate required flag must match lane policy"
        )

    gate_hash = source_adapter_gate.get("gate_hash", "")
    audit_hashes = source_adapter_gate.get("audit_hashes", {})
    if gate_required is not True:
        if gate_hash not in ("", None):
            blockers.append(
                f"{lane_label}: source adapter gate hash must be empty when not required"
            )
        if not isinstance(audit_hashes, dict):
            blockers.append(
                f"{lane_label}: source adapter gate audit hashes must be empty when not required"
            )
        elif audit_hashes:
            blockers.append(
                f"{lane_label}: source adapter gate audit hashes must be empty when not required"
            )
        return blockers

    parsed_gate_hash = _hex_bytes(gate_hash, byte_length=32)
    if parsed_gate_hash is None or not any(parsed_gate_hash):
        blockers.append(
            f"{lane_label}: source adapter gate hash must be a canonical non-zero bytes32 when required"
        )

    if not isinstance(audit_hashes, dict):
        blockers.append(
            f"{lane_label}: source adapter gate audit hashes must be an object when required"
        )
        return blockers

    if not audit_hashes:
        blockers.append(
            f"{lane_label}: source adapter gate audit hashes must not be empty when required"
        )
    for field in sorted(set(audit_hashes) - set(expected_audit_fields)):
        blockers.append(
            f"{lane_label}: source adapter gate audit hashes contains unexpected field: {field}"
        )
    for field in expected_audit_fields:
        value = audit_hashes.get(field)
        parsed = _hex_bytes(value, byte_length=32)
        if parsed is None or not any(parsed):
            blockers.append(
                f"{lane_label}: source adapter gate audit hashes {field} must be a canonical non-zero bytes32"
            )
    if (
        gate_field
        and parsed_gate_hash is not None
        and any(parsed_gate_hash)
        and audit_hashes.get(gate_field) != gate_hash
    ):
        blockers.append(
            f"{lane_label}: source adapter gate hash must match audit_hashes.{gate_field}"
        )
    source_record_hashes = lane.get("source_record_hashes")
    if not isinstance(source_record_hashes, dict):
        source_record_hashes = {}
    destination_binding = lane.get("destination_binding")
    if not isinstance(destination_binding, dict):
        destination_binding = {}
    route_summary = lane.get("route_allowlist")
    if not isinstance(route_summary, dict):
        route_summary = {}
    route_canary = route_summary.get("route_canary")
    if not isinstance(route_canary, dict):
        route_canary = {}
    role_fields: list[tuple[str, bytes | None]] = [
        (
            "source_verifier_material_hash",
            _hex_bytes(
                source_record_hashes.get("source_verifier_material_hash"),
                byte_length=32,
            ),
        ),
        (
            "source_adapter_engine_deployment_hash",
            _hex_bytes(
                source_record_hashes.get("source_adapter_engine_deployment_hash"),
                byte_length=32,
            ),
        ),
        (
            "destination_binding_hash",
            _hex_bytes(
                destination_binding.get("destination_binding_hash"),
                byte_length=32,
            ),
        ),
        (
            "route_allowlist_hash",
            _hex_bytes(
                route_summary.get("route_allowlist_hash"),
                byte_length=32,
            ),
        ),
        (
            "route_canary_evidence_hash",
            _hex_bytes(
                route_canary.get("evidence_hash"),
                byte_length=32,
            ),
        ),
    ]
    role_fields.extend(
        (f"audit_hashes.{field}", _hex_bytes(value, byte_length=32))
        for field, value in sorted(audit_hashes.items())
    )
    _expect_distinct_byte_values(
        blockers,
        tuple(role_fields),
        label=f"{lane_label}: source adapter gate hash role",
    )
    return blockers


def _release_checklist(
    lanes: list[dict[str, Any]],
    all_blockers: list[str],
) -> dict[str, Any]:
    """Return operator-facing release gates derived from lane evidence."""

    record_labels = {
        "source_verifier_material": "source verifier material",
        "source_adapter_deployment": "source adapter deployment",
        "destination_rollout": "destination rollout",
        "route_allowlist": "route allowlist",
    }

    records_blockers: list[str] = []
    deployment_blockers: list[str] = []
    route_blockers: list[str] = []
    canary_blockers: list[str] = []
    unresolved_blockers = list(all_blockers)

    def append_unresolved(blocker: str) -> None:
        if blocker not in unresolved_blockers:
            unresolved_blockers.append(blocker)

    for lane in lanes:
        lane_label = f"domain {lane.get('domain')} ({lane.get('chain')})"
        lane_blockers, lane_blocker_schema_errors = _canonical_blocker_list(
            lane.get("blockers", []),
            f"{lane_label}: lane",
        )
        if lane_blocker_schema_errors:
            canary_blockers.extend(lane_blocker_schema_errors)
            for blocker in lane_blocker_schema_errors:
                append_unresolved(blocker)
        else:
            for item in lane_blockers:
                append_unresolved(f"{lane_label}: {item}")

        records = lane.get("records", {})
        if not isinstance(records, dict):
            records_blockers.append(f"{lane_label}: lane record summary is malformed")
            records = {}
        missing_records = [
            label
            for key, label in record_labels.items()
            if records.get(key) is not True
        ]
        if missing_records:
            records_blockers.append(
                f"{lane_label}: missing {', '.join(missing_records)}"
            )

        if records.get("source_adapter_deployment") is not True:
            deployment_blockers.append(
                f"{lane_label}: source adapter deployment evidence is missing"
            )
        source_adapter_gate = lane.get("source_adapter_gate", {})
        if not isinstance(source_adapter_gate, dict):
            deployment_blockers.append(
                f"{lane_label}: source adapter gate summary is malformed"
            )
        else:
            gate_required = source_adapter_gate.get("required")
            gate_ready = source_adapter_gate.get("ready")
            if type(gate_required) is not bool:
                deployment_blockers.append(
                    f"{lane_label}: source adapter gate required flag must be boolean"
                )
            elif type(gate_ready) is not bool:
                deployment_blockers.append(
                    f"{lane_label}: source adapter gate ready flag must be boolean"
                )
            else:
                deployment_blockers.extend(
                    _source_adapter_gate_release_metadata_blockers(
                        lane_label,
                        lane,
                        source_adapter_gate,
                    )
                )
                if gate_required is True and gate_ready is not True:
                    gate_errors, gate_schema_errors = _canonical_blocker_list(
                        source_adapter_gate.get("blockers", []),
                        f"{lane_label}: source adapter gate",
                    )
                    if gate_schema_errors:
                        deployment_blockers.extend(gate_schema_errors)
                    elif gate_errors:
                        deployment_blockers.extend(
                            f"{lane_label}: {error}" for error in gate_errors
                        )
                    else:
                        deployment_blockers.append(
                            f"{lane_label}: source adapter gate is not ready"
                        )
        if records.get("destination_rollout") is not True:
            deployment_blockers.append(
                f"{lane_label}: destination rollout evidence is missing"
            )
        destination_binding = lane.get("destination_binding", {})
        if not isinstance(destination_binding, dict):
            deployment_blockers.append(
                f"{lane_label}: destination binding summary is malformed"
            )
            destination_binding = {}
        if (
            records.get("destination_rollout") is True
            and destination_binding.get("expected_destination_binding_hash_matches")
            is not True
        ):
            deployment_blockers.append(
                f"{lane_label}: destination binding hash is not bound to rollout evidence"
            )

        route_summary = lane.get("route_allowlist", {})
        if not isinstance(route_summary, dict):
            route_blockers.append(f"{lane_label}: route allowlist summary is malformed")
            route_summary = {}
        if records.get("route_allowlist") is not True:
            route_blockers.append(f"{lane_label}: route allowlist evidence is missing")
        elif route_summary.get("expected_route_allowlist_hash_matches") is not True:
            route_blockers.append(
                f"{lane_label}: route allowlist hash is not bound to source and destination evidence"
            )

        canary = route_summary.get("route_canary", {})
        if not isinstance(canary, dict):
            canary_blockers.append(f"{lane_label}: route canary summary is malformed")
            canary = {}
        canary_status = canary.get("status")
        if canary_status in (None, ""):
            canary_blockers.append(
                f"{lane_label}: route canary status is not passed"
            )
        elif (
            not isinstance(canary_status, str)
            or canary_status.strip() != canary_status
        ):
            canary_blockers.append(
                f"{lane_label}: route canary status must be a non-empty canonical string"
            )
        elif canary_status != "passed":
            canary_blockers.append(
                f"{lane_label}: route canary status is not passed"
            )
        canary_evidence_hash = canary.get("evidence_hash")
        parsed_canary_evidence_hash = _hex_bytes(
            canary_evidence_hash,
            byte_length=32,
        )
        if canary_evidence_hash in (None, ""):
            canary_blockers.append(
                f"{lane_label}: route canary evidence hash is missing"
            )
        elif parsed_canary_evidence_hash is None or not any(
            parsed_canary_evidence_hash
        ):
            canary_blockers.append(
                f"{lane_label}: route canary evidence hash must be a canonical non-zero bytes32"
            )
        expected_evidence_source = ROUTE_CANARY_EVIDENCE_SOURCE_BY_DOMAIN.get(
            lane["domain"]
        )
        canary_evidence_source = canary.get("evidence_source")
        if canary_evidence_source in (None, ""):
            canary_blockers.append(
                f"{lane_label}: live route canary evidence source is missing"
            )
        elif (
            not isinstance(canary_evidence_source, str)
            or canary_evidence_source.strip() != canary_evidence_source
        ):
            canary_blockers.append(
                f"{lane_label}: live route canary evidence source must be a non-empty canonical string"
            )
        elif canary_evidence_source != expected_evidence_source:
            canary_blockers.append(
                f"{lane_label}: live route canary evidence source must be {expected_evidence_source}"
            )
        lane_canary_blockers = [
            item for item in lane_blockers if "route canary" in item
        ]
        if canary.get("evidence_bound") is not True and not lane_canary_blockers:
            lane_canary_blockers.append("route canary evidence is not bound")
        canary_blockers.extend(f"{lane_label}: {item}" for item in lane_canary_blockers)

    items = [
        _release_checklist_item(
            "all_required_lane_records",
            "All advertised SCCP lanes have the required source, deployment, destination, and route records",
            records_blockers,
        ),
        _release_checklist_item(
            "governed_deployment_evidence",
            "Source-adapter deployments and destination rollouts are governed and hash-bound",
            deployment_blockers,
        ),
        _release_checklist_item(
            "route_allowlist_binding",
            "Route allowlists bind the governed source and destination evidence",
            route_blockers,
        ),
        _release_checklist_item(
            "live_route_canary_evidence",
            "Post-deploy route canary evidence is live, passed, and bound to the route",
            canary_blockers,
        ),
        _release_checklist_item(
            "no_unresolved_blockers",
            "No SCCP all-lanes preflight blockers remain",
            unresolved_blockers,
        ),
    ]
    return {
        "ready": all(item["ready"] is True for item in items),
        "items": items,
    }


def _evm_live_metadata_summary(
    profile: LaneProfile,
    material: dict[str, Any] | None,
    destination: dict[str, Any] | None,
) -> dict[str, Any]:
    """Return public EVM live-read metadata carried by lane evidence."""

    if profile.chain not in ("eth", "bsc"):
        return {
            "required": False,
            "ready": True,
            "source_rpc_chain_id": "",
            "source_block_tag": "",
            "destination_rpc_chain_id": "",
            "destination_block_tag": "",
        }
    source_rpc_chain_id = (
        str(material.get("_comment_evm_source_rpc_chain_id"))
        if material is not None
        and material.get("_comment_evm_source_rpc_chain_id") is not None
        else ""
    )
    source_block_tag = (
        str(material.get("_comment_evm_source_block_tag"))
        if material is not None and material.get("_comment_evm_source_block_tag") is not None
        else ""
    )
    destination_rpc_chain_id = (
        str(destination.get("_comment_evm_rpc_chain_id"))
        if destination is not None
        and destination.get("_comment_evm_rpc_chain_id") is not None
        else ""
    )
    destination_block_tag = (
        str(destination.get("_comment_evm_block_tag"))
        if destination is not None and destination.get("_comment_evm_block_tag") is not None
        else ""
    )
    expected_chain_id = EVM_EXPECTED_RPC_CHAIN_IDS[profile.domain]
    source_chain_id_ready = (
        _is_canonical_decimal_text(source_rpc_chain_id, positive=True)
        and int(source_rpc_chain_id, 10) == expected_chain_id
    )
    destination_chain_id_ready = (
        _is_canonical_decimal_text(destination_rpc_chain_id, positive=True)
        and int(destination_rpc_chain_id, 10) == expected_chain_id
    )
    if profile.domain == SCCP_DOMAIN_ETH:
        ready = (
            source_chain_id_ready
            and destination_chain_id_ready
            and source_block_tag == "finalized"
            and destination_block_tag == "finalized"
        )
    else:
        ready = (
            source_chain_id_ready
            and destination_chain_id_ready
            and bool(source_block_tag and destination_block_tag)
        )
    return {
        "required": True,
        "ready": ready,
        "source_rpc_chain_id": source_rpc_chain_id,
        "source_block_tag": source_block_tag,
        "destination_rpc_chain_id": destination_rpc_chain_id,
        "destination_block_tag": destination_block_tag,
    }


def _evidence_bundle_root_errors(records: Any) -> tuple[dict[str, Any], list[str]]:
    """Return a dict-shaped evidence root plus root-level validation blockers."""

    if not isinstance(records, dict):
        return {}, ["evidence bundle root must be an object"]

    errors: list[str] = []
    for section in sorted(records, key=lambda item: str(item)):
        if not isinstance(section, str):
            errors.append(f"evidence section name must be a string: {section!r}")
        elif section not in SECTION_NAMES:
            errors.append(f"unsupported evidence section {section}")
    return records, errors


def validate_evidence_bundle(records: dict[str, list[dict[str, Any]]] | Any) -> dict[str, Any]:
    """Return a production-readiness summary for a merged SCCP evidence bundle."""

    records, global_errors = _evidence_bundle_root_errors(records)
    materials, material_errors = _records_by_domain(
        records.get("sccp_source_verifier_materials", []),
        "source_domain",
        allowed_domains=set(SCCP_CORE_REMOTE_DOMAINS),
    )
    deployments, deployment_errors = _records_by_domain(
        records.get("sccp_source_adapter_engine_deployments", []),
        "source_domain",
        allowed_domains=set(SCCP_CORE_REMOTE_DOMAINS),
    )
    destinations, destination_errors = _records_by_domain(
        records.get("sccp_destination_rollouts", []),
        "domain",
        allowed_domains=set(SCCP_CORE_REMOTE_DOMAINS),
    )
    routes, route_errors = _records_by_domain(
        records.get("sccp_route_allowlists", []),
        "domain",
        allowed_domains=set(SCCP_CORE_REMOTE_DOMAINS),
    )
    route_canary_cross_lane_errors = _check_route_canary_evidence_hashes_unique(routes)

    section_errors = {
        "sccp_source_verifier_materials": material_errors,
        "sccp_source_adapter_engine_deployments": deployment_errors,
        "sccp_destination_rollouts": destination_errors,
        "sccp_route_allowlists": route_errors,
    }

    lanes = []
    all_blockers: list[str] = []
    for domain in SCCP_CORE_REMOTE_DOMAINS:
        profile = LANE_PROFILES[domain]
        blockers: list[str] = []
        material = materials.get(domain)
        deployment = deployments.get(domain)
        destination = destinations.get(domain)
        route = routes.get(domain)

        destination_binding: dict[str, Any] = {}
        route_allowlist_summary: dict[str, Any] = {}
        if material is None:
            blockers.append("missing source verifier material")
        else:
            blockers.extend(_check_source_material(profile, material))

        if deployment is None:
            blockers.append("missing source adapter deployment")
        elif material is not None:
            blockers.extend(_check_deployment(profile, material, deployment))

        source_record_hashes: dict[str, str] = {}
        if material is not None and deployment is not None and not blockers:
            try:
                source_record_hashes = _canonical_source_record_hashes(
                    profile,
                    material,
                    deployment,
                )
            except (SystemExit, ValueError, RuntimeError):
                blockers.append("source record hashes cannot be recomputed")

        if destination is None:
            blockers.append("missing destination rollout")
        else:
            blockers.extend(_check_destination_rollout(profile, destination))
            binding_errors, destination_binding = _check_destination_binding(
                profile,
                material,
                destination,
            )
            blockers.extend(binding_errors)

        if route is None:
            blockers.append("missing route allowlist")
        else:
            route_errors, route_allowlist_summary = _check_route_allowlist(
                profile,
                route,
                source_record_hashes,
                destination_binding,
                destination,
            )
            blockers.extend(route_errors)

        for section, errors in section_errors.items():
            for error in errors:
                if f"domain {domain}" in error:
                    blockers.append(f"{section}: {error}")

        source_adapter_gate = _source_adapter_gate_summary(
            profile,
            material,
            deployment,
            route,
            source_record_hashes,
            destination_binding,
            route_allowlist_summary,
        )
        evm_live_metadata = _evm_live_metadata_summary(
            profile,
            material,
            destination,
        )
        if source_adapter_gate.get("required") is True:
            gate_blockers, gate_schema_errors = _canonical_blocker_list(
                source_adapter_gate.get("blockers", []),
                "source adapter gate",
            )
            for gate_blocker in [*gate_schema_errors, *gate_blockers]:
                if gate_blocker not in blockers:
                    blockers.append(gate_blocker)
        lanes.append(
            {
                "domain": domain,
                "chain": profile.chain,
                "production_ready": False,
                "records": {
                    "source_verifier_material": material is not None,
                    "source_adapter_deployment": deployment is not None,
                    "destination_rollout": destination is not None,
                    "route_allowlist": route is not None,
                },
                "source_record_hashes": source_record_hashes,
                "source_adapter_gate": source_adapter_gate,
                "evm_live_metadata": evm_live_metadata,
                "destination_binding": destination_binding,
                "route_allowlist": route_allowlist_summary,
                "blockers": blockers,
            }
        )

    route_canary_cross_lane_errors.extend(
        _check_route_canary_evidence_hashes_do_not_reuse_governed_hashes(
            lanes
        )
    )
    global_errors.extend(
        _attach_route_canary_cross_lane_errors(
            lanes,
            route_canary_cross_lane_errors,
        )
    )
    for lane in lanes:
        lane["production_ready"] = not lane["blockers"]
        all_blockers.extend(
            f"domain {lane['domain']} ({lane['chain']}): {item}"
            for item in lane["blockers"]
        )
    for section, errors in section_errors.items():
        for error in errors:
            if not any(f"domain {domain}" in error for domain in SCCP_CORE_REMOTE_DOMAINS):
                all_blockers.append(f"{section}: {error}")
    all_blockers.extend(global_errors)
    release_checklist = _release_checklist(lanes, all_blockers)

    return {
        "production_ready": not all_blockers,
        "required_domains": list(SCCP_CORE_REMOTE_DOMAINS),
        "supported_launch_domains": list(SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS),
        "unsupported_launch_domains": list(SCCP_UNSUPPORTED_LAUNCH_REMOTE_DOMAINS),
        "lanes": lanes,
        "blockers": all_blockers,
        "release_checklist": release_checklist,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Validate that SCCP source, deployment, destination, and route "
            "evidence covers every advertised remote lane."
        ),
    )
    parser.add_argument(
        "toml",
        nargs="+",
        type=Path,
        help="TOML evidence snippet or full config containing [zk] SCCP records.",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Only return the readiness exit code.",
    )
    return parser


SENSITIVE_CLI_ERROR_MARKERS = (
    "secret-token",
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer ",
    "authorization",
    "access-key",
    "access_key",
    "api-key",
    "api_key",
    "client-secret",
    "client_secret",
    "session=",
    "token=",
)


def _cli_error_detail(exc: BaseException, *, fallback: str) -> str:
    if isinstance(exc, OSError):
        return fallback
    text = str(exc)
    if not text:
        return fallback
    lowered = text.lower()
    if any(marker in lowered for marker in SENSITIVE_CLI_ERROR_MARKERS):
        return fallback
    if any((ord(ch) < 0x20 and ch not in "\n\t") or ord(ch) == 0x7F for ch in text):
        return fallback
    return text


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        records = load_evidence_bundle(args.toml)
        summary = validate_evidence_bundle(records)
    except (OSError, RuntimeError, ValueError) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP all-lanes evidence validation failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")

    if not args.quiet:
        print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if summary["production_ready"] is True else 1


if __name__ == "__main__":
    raise SystemExit(main())
