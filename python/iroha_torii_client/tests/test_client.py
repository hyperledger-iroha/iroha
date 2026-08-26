from __future__ import annotations

import base64
import copy
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, Callable, Dict, List, Mapping, Optional, Union, get_args, get_type_hints
from urllib.parse import quote

import pytest
import requests

from sumeragi_exact_json_test_support import (
    RecordingSession,
    StubResponse,
    sumeragi_exact_json_response_cases,
)
from client_test_support import (
    CANONICAL_OWNER,
    CANONICAL_OWNER_HEADER,
    app_api_transaction_draft as _app_api_transaction_draft,
    authority_fee_payment as _authority_fee_payment,
    canonical_hash as _canonical_hash,
    sponsor_fee_payment as _sponsor_fee_payment,
)

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from offline_test_support import (  # noqa: E402
    OFFLINE_NETWORK_ID,
    OFFLINE_OPERATION_BYTES,
    OFFLINE_OPERATION_ID,
    OFFLINE_OTHER_NETWORK_ID,
    OFFLINE_STATUS_URI,
    OFFLINE_REDEEM_REQUEST_FRAME,
    OFFLINE_SUBMITTED_AT_MS,
    OFFLINE_TOP_UP_REQUEST_FRAME,
    OFFLINE_TRANSACTION_HASH,
    offline_applied_top_up_status as _offline_applied_top_up_status,
    offline_capability_payload as _offline_capability_payload,
    offline_fixed_bytes as _offline_fixed_bytes,
    offline_norito_frame as _offline_norito_frame,
    offline_operation_reference as _offline_operation_reference,
    offline_norito_request_frame as _offline_norito_request_frame,
    offline_redeem_request as _offline_redeem_request,
    offline_rejected_status as _offline_rejected_status,
    offline_top_up_anchor as _offline_top_up_anchor,
    offline_top_up_finality_proof as _offline_top_up_finality_proof,
    offline_top_up_request as _offline_top_up_request,
)

import iroha_torii_client as torii_module  # noqa: E402
import iroha_torii_client.client as client_module  # noqa: E402
import iroha_torii_client.mock as mock_module  # noqa: E402
from iroha_torii_client import (  # noqa: E402  (import depends on sys.path mutation)
    ContractCallResponse,
    ContractOperationReceipt,
    ExplorerAccountQr,
    GovernanceContractResponse,
    GovernanceLockCustody,
    GovernanceLockRecord,
    KagemushaRedeemRequestV4,
    KagemushaTopUpRequestV4,
    MultisigResponse,
    NetworkTimeSnapshot,
    NetworkTimeStatus,
    OfflineAppliedOperation,
    OfflineAssetScale,
    OfflinePendingOperation,
    OfflineRejectedOperation,
    OfflineStatus,
    OfflineTopUpAnchor,
    OfflineTopUpFinalityProof,
    SumeragiDiagnosticsStatus,
    SumeragiV2Status,
    ToriiCanonicalRequestAuth,
    ToriiClient,
    ToriiLocalSigningContext,
    ToriiOperatorSigningContext,
    VpnQuoteCreateRequest,
    VpnReceiptSubmitRequest,
    VpnSessionCreateRequest,
    build_canonical_request_headers,
    canonical_network_request_signature_message,
    decode_pdp_commitment_header,
)
from iroha_torii_client.mock import ToriiMockServer  # noqa: E402
from iroha_torii_client.native_amx import (  # noqa: E402
    compute_native_amx_descriptor_hash,
    compute_native_amx_participant_settlement_hash,
    compute_native_amx_proposal_hash,
    compute_native_amx_validator_set_hash,
)

CANONICAL_LARGE_FRACTION = "18446744073709551616.25"
CANONICAL_ASSET_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
CANONICAL_ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
CHECKSUM_INVALID_ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF2"
CHECKSUM_VALID_NON_UUID_V4_ASSET_DEFINITION_ID = "7EAD8EFYV3tk2BtyQaGhqhATjFy7"
CHECKSUM_VALID_NON_RFC4122_ASSET_DEFINITION_ID = "7EAD8EFYUx1bhNP18PQmxXsySxi6"
_NATIVE_AMX_VALIDATOR_SET = [
    "ea013094D37A1FCA72E8734CAAD4163678D82C36FE2CA70B80F5626E6591709E0D44831BE86CBA9BD0471C6D0D73FF9C4B54E0",
    "ea01309988FA1336476987EF7F91C3EA728B7EA0556698AA0F1A294147C8D5CD43BB24C4BCD14FAE23A384D721CBF1F6A16DF7",
    "ea013099BA3FACE165941434D3238C4D5767059EBFFFB4120A9885A4EB2BAC9CD868F690660D2936B03C0214FBDAD36034D578",
    "ea0130B921EAC90D1A99EC9DA3FF8C8A29EBEE19DD1B659A4C6FC21BC8046EA30DE566668EDCCEAE4CB5932F4F860606A1E0E3",
]


def _offline_bound_client(session: Any) -> ToriiClient:
    return ToriiClient(
        "http://node.test",
        session=session,
        local_signing_context=ToriiLocalSigningContext(OFFLINE_NETWORK_ID),
    )


def _contract_operation_receipt(
    *,
    entrypoint: str = "ping",
    gas_limit: int = 5000,
    fee_payment: Optional[Dict[str, Any]] = None,
    contract_alias: Optional[str] = "router::universal",
    contract_address: Optional[str] = (
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
    ),
) -> Dict[str, Any]:
    return {
        "operation_kind": "contract_call",
        "status": "pending_signature",
        "transport": "torii",
        "dataspace": "universal",
        "contract_alias": contract_alias,
        "contract_address": contract_address,
        "code_hash_hex": "22" * 32,
        "abi_hash_hex": "33" * 32,
        "tx_hash_hex": None,
        "entrypoint": entrypoint,
        "entrypoint_hash_hex": None,
        "gas_limit": gas_limit,
        "gas_used": None,
        "fee_payment": fee_payment or _sponsor_fee_payment(gas_limit),
        "payload_digest_hex": "66" * 32,
    }


def _contract_call_draft(
    *,
    entrypoint: str = "ping",
    contract_alias: Optional[str] = "router::universal",
    contract_address: Optional[str] = (
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
    ),
    fee_payment: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    transaction_payload = b"\x01\x02\x03"
    signing_message = bytearray(hashlib.blake2b(transaction_payload, digest_size=32).digest())
    signing_message[-1] |= 1
    return {
        "ok": True,
        "submitted": False,
        "dataspace": "universal",
        "code_hash_hex": "22" * 32,
        "abi_hash_hex": "33" * 32,
        "creation_time_ms": 42,
        "contract_address": contract_address,
        "tx_hash_hex": None,
        "pipeline_status": None,
        "entrypoint": entrypoint,
        "transaction_ttl_ms": 60_000,
        "entrypoint_hash_hex": None,
        "transaction_payload_b64": base64.b64encode(transaction_payload).decode("ascii"),
        "signing_message_b64": base64.b64encode(signing_message).decode("ascii"),
        "operation_receipt": _contract_operation_receipt(
            entrypoint=entrypoint,
            fee_payment=fee_payment,
            contract_alias=contract_alias,
            contract_address=contract_address,
        ),
    }


GOVERNANCE_NETWORK_ID = _canonical_hash(0xA5)


def _operator_context(captured: Optional[List[bytes]] = None) -> ToriiOperatorSigningContext:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return b"\x55" * 64

    return ToriiOperatorSigningContext(
        network_id=GOVERNANCE_NETWORK_ID,
        public_key="ed0120" + "66" * 32,
        signer=signer,
    )


def _governance_auth(captured: Optional[List[bytes]] = None) -> ToriiCanonicalRequestAuth:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return b"\x44" * 64

    return ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=signer,
        timestamp_ms=4_102_444_801_000,
        nonce="low-python-governance-test",
    )


def _fee_quote_transaction_payload(
    fee_payment: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """Return the exact closed Norito JSON shape of ``TransactionPayload``."""

    return {
        "domain": {"kind": "network", "value": GOVERNANCE_NETWORK_ID},
        "authority": CANONICAL_OWNER,
        "creation_time_ms": 1_725_000_000_123,
        "instructions": {"Instructions": []},
        "time_to_live_ms": 100_000,
        "nonce": None,
        "fee_payment": fee_payment or _authority_fee_payment(),
        "admission_intent": {"intent": "ordinary", "value": None},
        "metadata": {},
        "attachments": None,
    }


_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT = (
    "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
)


def _sumeragi_v2_status_payload() -> Dict[str, Any]:
    subject = {
        "parent_block_hash": _canonical_hash(0x31),
        "block_hash": _canonical_hash(0x32),
        "payload_hash": _canonical_hash(0x33),
    }
    execution_commitment = {
        "parent_state_root": _canonical_hash(0x34),
        "post_state_root": _canonical_hash(0x35),
        "ordinary_writes_root": _canonical_hash(0x36),
        "topup_anchor_count": 0,
        "native_amx_application_manifest_version": 1,
        "native_amx_application_manifest_root": _NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT,
        "native_amx_application_manifest_count": 0,
        "lane_finality_manifest": None,
        "merge_carrier": None,
        "executed_block_wire_len": 123,
        "executed_block_wire_hash": _canonical_hash(0x37),
    }
    return {
        "protocol_version": 4,
        "node_fingerprint": _canonical_hash(0x11),
        "build_fingerprint": _canonical_hash(0x12),
        "config_fingerprint": _canonical_hash(0x13),
        "restart_required": False,
        "height_context_id": [_canonical_hash(0x14)],
        "height": 10,
        "view": 2,
        "phase": {"phase": "prepare", "details": None},
        "leader": 1,
        "locked_prepare_qc": None,
        "highest_prepare_qc": None,
        "last_timeout_certificate": None,
        "body_state": {"state": "validated", "details": None},
        "pending_persistence_id": None,
        "last_committed_height": 9,
        "last_committed_subject": subject,
        "height_context": {
            "epoch": 1,
            "epoch_end_height": 20,
            "mode": {"mode": "permissioned", "details": None},
            "epoch_seed": bytes(range(32)).hex().upper(),
            "validator_count": 4,
            "quorum": {"min_signers": 3, "total_power": 4},
        },
        "last_commit_qc": {
            "certificate": {
                "round": {
                    "context_id": [_canonical_hash(0x41)],
                    "height": 9,
                    "view": 1,
                },
                "proposal_round": {
                    "context_id": [_canonical_hash(0x41)],
                    "height": 9,
                    "view": 1,
                },
                "phase": {"phase": "commit", "details": None},
                "subject": dict(subject),
                "execution_commitment": execution_commitment,
            },
            "validator_count": 4,
            "signer_count": 3,
            "min_signers": 3,
            "signed_power": 3,
            "total_power": 4,
        },
        "liveness": {
            "generation": 2,
            "prepare_quorums": [
                {
                    "round": {
                        "context_id": [_canonical_hash(0x14)],
                        "height": 10,
                        "view": 1,
                    },
                    "proposal_round": {
                        "context_id": [_canonical_hash(0x14)],
                        "height": 10,
                        "view": 1,
                    },
                    "subject": dict(subject),
                    "execution_commitment": dict(execution_commitment),
                    "signer_count": 2,
                    "signed_power": 2,
                    "min_signers": 3,
                    "total_power": 4,
                }
            ],
            "commit_quorums": [],
            "timeout_quorums": [],
            "outbound_intents": [
                {
                    "kind": {"kind": "proposal", "details": None},
                    "round": {
                        "context_id": [_canonical_hash(0x14)],
                        "height": 10,
                        "view": 1,
                    },
                    "proposal_round": {
                        "context_id": [_canonical_hash(0x14)],
                        "height": 10,
                        "view": 1,
                    },
                    "subject": dict(subject),
                    "stage": {"stage": "sent", "details": None},
                }
            ],
            "work": {
                "candidate": {"stage": "idle", "details": None},
                "body_recovery": {"stage": "idle", "details": None},
                "body_store": {"stage": "idle", "details": None},
                "validation": {"stage": "complete", "details": None},
                "application": {"stage": "idle", "details": None},
                "successor_height": {"stage": "idle", "details": None},
            },
            "queues": [
                {
                    "queue": {"queue": "network_ingress", "details": None},
                    "depth": 1,
                    "capacity": 4,
                    "oldest_age_ms": 17,
                    "service_debt": 2,
                }
            ],
            "last_progress": {
                "generation": 2,
                "round": {
                    "context_id": [_canonical_hash(0x14)],
                    "height": 10,
                    "view": 1,
                },
                "transition": {
                    "transition": "prepare_vote_admitted",
                    "details": None,
                },
                "age_ms": 19,
            },
            "no_progress_age_ms": 19,
            "blocker": {"blocker": "prepare_quorum_missing", "details": None},
            "ignore_counts": [
                {
                    "reason": {"reason": "duplicate", "details": None},
                    "count": 2,
                }
            ],
        },
    }


def _sumeragi_diagnostics_payload() -> Dict[str, Any]:
    return {
        "pipeline_execution": {
            "tx_vertices_total": 1,
            "tx_edges_total": 0,
            "overlay_count_total": 1,
            "overlay_instr_total": 2,
            "overlay_bytes_total": 128,
            "rbc_chunks_total": 1,
            "rbc_bytes_total": 256,
            "detached_prepared_total": 1,
            "detached_merged_total": 1,
            "detached_fallback_total": 0,
            "detached_fallback_fee_postprocessing_total": 0,
            "detached_fallback_user_executor_total": 0,
            "detached_fallback_durable_state_total": 0,
            "detached_fallback_unsupported_instruction_total": 0,
            "detached_fallback_rejected_eval_total": 0,
            "detached_fallback_overlay_error_total": 0,
            "quarantine_executed_total": 0,
        },
        "tx_queue_depth": 3,
        "tx_queue_capacity": 32,
        "tx_queue_retained_bytes": 4096,
        "tx_queue_max_retained_bytes": 65536,
        "tx_queue_saturated": False,
        "tx_queue_saturated_by_count": False,
        "tx_queue_saturated_by_bytes": False,
        "tx_queue_saturated_by_age": False,
        "tx_queue_oldest_queued_age_ms": 25,
        "npos": None,
        "lane_commitments": [],
        "dataspace_commitments": [],
        "lane_settlement_commitments": [],
        "lane_relay_envelopes": [],
        "lane_payload_ownerships": [],
        "committed_lane_blocks": [],
        "lane_block_sessions": [],
        "lane_governance_sealed_total": 0,
        "lane_governance_sealed_aliases": [],
        "lane_governance": [],
        "native_amx_participant_applications": [],
        "autonomous_lane_executions": [],
    }


def _native_amx_participant_application_payload(
    *,
    lane_id: int = 3,
    state: Any = "durably_applied",
) -> Dict[str, Any]:
    return {
        "lane_id": lane_id,
        "dataspace_id": 8,
        "lane_incarnation": _canonical_hash(0x65),
        "participant_height": 8,
        "participant_view": 1,
        "predecessor_height": 7,
        "predecessor_descriptor_hash": _canonical_hash(0x68),
        "descriptor_hash": _canonical_hash(0x73),
        "proposal_hash": _canonical_hash(0x69),
        "settlement_hash": _canonical_hash(0x6B),
        "source_count": 2,
        "application_block_height": 10,
        "application_block_hash": _canonical_hash(0x79),
        "state": state,
    }


def _set_native_amx_application_without_block(
    row: Dict[str, Any], state: str
) -> None:
    row["state"] = state
    row.pop("application_block_height")
    row.pop("application_block_hash")


def _autonomous_lane_execution_payload() -> Dict[str, Any]:
    return {
        "lane_id": 3,
        "dataspace_id": 8,
        "lane_incarnation": _canonical_hash(0x65),
        "lane_block_height": 8,
        "lane_block_view": 1,
        "proposal_height": 10,
        "proposal_view": 2,
        "reservation_owner_hash": _canonical_hash(0x66),
        "proposal_identity_hash": _canonical_hash(0x67),
        "reservation_group_hash": _canonical_hash(0x68),
        "proposal_hash": _canonical_hash(0x69),
        "descriptor_hash": _canonical_hash(0x73),
        "executable_payload_hash": _canonical_hash(0x74),
        "source_bundle_hash": _canonical_hash(0x75),
        "merge_entry_hash": _canonical_hash(0x76),
        "application_block_height": 12,
        "application_block_hash": _canonical_hash(0x77),
        "reservation_count": 2,
        "transaction_count": 2,
        "highest_durable_stage": "kura_wsv_application_receipt_durable",
        "stuck_reason": "queue_finalization_unverifiable",
    }


def _lane_settlement_payload() -> Dict[str, Any]:
    return {
        "block_height": 9,
        "lane_id": 2,
        "lane_incarnation": _canonical_hash(0x51),
        "dataspace_id": 7,
        "tx_count": 1,
        "total_local_amount": "10",
        "total_xor_due": "5",
        "total_xor_after_haircut": "4",
        "total_xor_variance": "1",
        "swap_metadata": {
            "epsilon_bps": 5,
            "twap_window_seconds": 60,
            "liquidity_profile": {"profile": "Tier1", "state": None},
            "twap_local_per_xor": "2.5",
            "volatility_class": {"bucket": "Stable", "state": None},
        },
        "receipts": [
            {
                "source_id": "52" * 32,
                "local_amount": "10",
                "xor_due": "5",
                "xor_after_haircut": "4",
                "xor_variance": "1",
                "timestamp_ms": 1700,
            }
        ],
        "nexus_fee_receipts": [],
        "native_amx_receipts": [],
    }


def _nexus_fee_receipt_payload() -> Dict[str, Any]:
    return {
        "version": 1,
        "source_id": "A1" * 32,
        "dataspace_id": 7,
        "lane_id": 2,
        "block_height": 9,
        "payer_account_id": CANONICAL_OWNER,
        "fee_asset_id": "xor#universal",
        "fee_amount": CANONICAL_LARGE_FRACTION,
        "schedule": {
            "tx_bytes_len": 128,
            "instruction_count": 2,
            "gas_used": 3,
            "base_fee": "1",
            "per_byte_fee": "0.5",
            "per_instruction_fee": "2",
            "per_gas_unit_fee": "0",
        },
    }


def _seal_native_amx_receipt_payload(receipt: Dict[str, Any]) -> Dict[str, Any]:
    for leg in receipt["legs"]:
        descriptor = leg["participant_proposal"]["descriptor"]
        descriptor["validator_set_hash"] = (
            compute_native_amx_validator_set_hash(
                descriptor["validator_set"]
            )
        )
        descriptor["descriptor_hash"] = compute_native_amx_descriptor_hash(
            descriptor
        )
        leg["participant_proposal"]["proposal_hash"] = (
            compute_native_amx_proposal_hash(descriptor)
        )
        leg["participant_settlement_hash"] = (
            compute_native_amx_participant_settlement_hash(
                leg["participant_settlement"]
            )
        )
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["validator_set_hash"] = descriptor["validator_set_hash"]
            qc["body"]["participant_validator_set_hash"] = descriptor[
                "validator_set_hash"
            ]
            qc["body"]["participant_proposal_hash"] = leg[
                "participant_proposal"
            ]["proposal_hash"]
            qc["body"]["participant_settlement_commitment"] = leg[
                "participant_settlement_hash"
            ]
    return receipt


def _native_amx_receipt_payload(source_index: int = 0) -> Dict[str, Any]:
    transaction_hashes = [_canonical_hash(0x61), _canonical_hash(0x74)]
    source_ids = ["AB" * 32, "CD" * 32]
    transaction_hash = transaction_hashes[source_index]
    source_id = source_ids[source_index]
    previous_descriptor_hash = _canonical_hash(0x68)
    participant_proposal_hash = _canonical_hash(0x69)
    participant_settlement_hash = _canonical_hash(0x6B)
    common_body = {
        "round": {
            "context_id": [_canonical_hash(0x62)],
            "height": 10,
            "view": 2,
        },
        "epoch": 1,
        "network_id": _canonical_hash(0x63),
        "source_id": source_id,
        "tx_entrypoint_hash": transaction_hash,
        "plan_digest": _canonical_hash(0x64),
        "phase": {"phase": "prepare", "detail": None},
        "coordinator_lane_id": 2,
        "coordinator_dataspace_id": 7,
        "coordinator_lane_incarnation": _canonical_hash(0x51),
        "participant_lane_id": 3,
        "participant_dataspace_id": 8,
        "participant_lane_incarnation": _canonical_hash(0x65),
        "participant_previous_block_height": 7,
        "participant_previous_block_descriptor_hash": previous_descriptor_hash,
        "participant_lane_block_height": 8,
        "participant_lane_block_view": 1,
        "participant_proposal_hash": participant_proposal_hash,
        "participant_settlement_commitment": participant_settlement_hash,
        "participant_validator_set_hash": _canonical_hash(0x66),
        "participant_validator_count": 4,
        "participant_min_quorum": 3,
        "authority_context_height": 10,
        "planned_coordinator_block_height": 9,
        "coordinator_lane_block_view": 2,
        "coordinator_proposal_hash": _canonical_hash(0x67),
    }

    def qc(phase: str) -> Dict[str, Any]:
        body = json.loads(json.dumps(common_body))
        body["phase"]["phase"] = phase
        return {
            "body": body,
            "validator_set_hash_version": 1,
            "validator_set_hash": _canonical_hash(0x66),
            "validator_set": list(_NATIVE_AMX_VALIDATOR_SET),
            "validator_set_pops": [[1] * 96 for _ in range(4)],
            "signers_bitmap": [0x07],
            "bls_aggregate_signature": [2] * 96,
        }

    return _seal_native_amx_receipt_payload({
        "version": 2,
        "source_id": source_id,
        "network_id": _canonical_hash(0x63),
        "plan_digest": _canonical_hash(0x64),
        "lane_id": 2,
        "dataspace_id": 7,
        "lane_incarnation": _canonical_hash(0x51),
        "authority_context_height": 10,
        "lane_block_height": 9,
        "lane_block_view": 2,
        "coordinator_proposal_hash": _canonical_hash(0x67),
        "legs": [
            {
                "lane_id": 3,
                "dataspace_id": 8,
                "participant_proposal": {
                    "descriptor": {
                        "lane_id": 3,
                        "dataspace_id": 8,
                        "lane_incarnation": _canonical_hash(0x65),
                        "proposal_height": 10,
                        "previous_lane_block_height": 7,
                        "previous_lane_block_descriptor_hash": previous_descriptor_hash,
                        "lane_block_height": 8,
                        "lane_block_view": 1,
                        "subject_hash": _canonical_hash(0x6D),
                        "payload_ownership_hash": _canonical_hash(0x6F),
                        "rbc_instance_hash": _canonical_hash(0x71),
                        "accepted_candidate_indices": [0, 1],
                        "accepted_transaction_hashes": transaction_hashes,
                        "validator_set_hash_version": 1,
                        "validator_set_hash": _canonical_hash(0x66),
                        "validator_set": list(_NATIVE_AMX_VALIDATOR_SET),
                        "validator_count": 4,
                        "min_quorum": 3,
                        "qc_mode_tag": "permissioned:native-amx-v2",
                        "descriptor_hash": _canonical_hash(0x73),
                    },
                    "proposal_hash": participant_proposal_hash,
                    "payload_block_hint": None,
                },
                "participant_settlement": {
                    "block_height": 8,
                    "lane_id": 3,
                    "lane_incarnation": _canonical_hash(0x65),
                    "dataspace_id": 8,
                    "tx_count": 2,
                    "total_local_amount": "0",
                    "total_xor_due": "0",
                    "total_xor_after_haircut": "0",
                    "total_xor_variance": "0",
                    "swap_metadata": None,
                    "receipts": [
                        {
                            "source_id": source_ids[0],
                            "local_amount": "0",
                            "xor_due": "0",
                            "xor_after_haircut": "0",
                            "xor_variance": "0",
                            "timestamp_ms": 10,
                        },
                        {
                            "source_id": "CD" * 32,
                            "local_amount": "0",
                            "xor_due": "0",
                            "xor_after_haircut": "0",
                            "xor_variance": "0",
                            "timestamp_ms": 10,
                        },
                    ],
                    "nexus_fee_receipts": [],
                    "native_amx_receipts": [],
                },
                "participant_settlement_hash": participant_settlement_hash,
                "prepare_qc": qc("prepare"),
                "commit_qc": qc("commit"),
            }
        ],
    })


def _native_amx_receipt_group() -> List[Dict[str, Any]]:
    return [_native_amx_receipt_payload(0), _native_amx_receipt_payload(1)]


def _get_sumeragi_status(payload: Mapping[str, Any]) -> SumeragiV2Status:
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    return ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    ).get_sumeragi_status()


def _get_sumeragi_diagnostics(
    payload: Mapping[str, Any],
) -> SumeragiDiagnosticsStatus:
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    return ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    ).get_sumeragi_diagnostics()


def _canonical_signature_base64_fixture() -> str:
    return base64.b64encode(bytes([1]) * 64).decode("ascii")


def _noncanonical_standard_base64_pad_bit_alias(encoded: str) -> str:
    assert encoded.endswith("==")
    alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    chars = list(encoded)
    index = len(chars) - 3
    chars[index] = alphabet[alphabet.index(chars[index]) ^ 0x01]
    return "".join(chars)


def _sample_sorafs_orderbook_payloads() -> Dict[str, Any]:
    def fixed(seed: int) -> List[int]:
        return [seed] * 32

    cursor = {"height": 42, "block_hash": fixed(0xA0)}
    order = {
        "order_id": fixed(0x11),
        "owner": "alice@wonderland",
        "canonical_order": base64.b64encode(b"canonical-order").decode("ascii"),
        "admitted_policy_digest": fixed(0x12),
        "admitted_at_unix": 1_700_000_000,
        "admission_sequence": 7,
        "remaining_gib": 2,
        "status": {"status": "open", "value": None},
        "updated_at_unix": 1_700_000_001,
        "canonical_cancel": None,
        "cancelled_at_unix": None,
        "cancelled_policy_digest": None,
    }
    trade = {
        "trade_id": fixed(0x22),
        "maker_order_id": fixed(0x11),
        "taker_order_id": fixed(0x13),
        "trade_sequence": 3,
        "canonical_trade": base64.b64encode(b"canonical-trade").decode("ascii"),
        "channel_id": fixed(0x33),
        "book_revision": 9,
        "recorded_at_unix": 1_700_000_100,
    }
    channel = {
        "channel_id": fixed(0x33),
        "trade_id": fixed(0x22),
        "buyer": "alice@wonderland",
        "provider": "provider@storage",
        "provider_id": fixed(0x55),
        "settlement_authority": "settlement@governance",
        "total_bytes": 2_147_483_648,
        "remaining_bytes": 1_073_741_824,
        "initial_xor_locked": "340282366920938463463374607431768211456.000000001",
        "remaining_xor_locked": "1.000000001",
        "status": {"status": "open", "value": None},
        "opened_at_unix": 1_700_000_101,
        "expires_at_unix": 1_800_000_000,
        "updated_at_unix": 1_700_000_102,
    }
    receipt = {
        "receipt_id": fixed(0x44),
        "channel_id": fixed(0x33),
        "trade_id": fixed(0x22),
        "canonical_receipt": base64.b64encode(b"canonical-receipt").decode("ascii"),
        "admitted_policy_digest": fixed(0x12),
        "admitted_at_unix": 1_700_000_103,
        "recorded_by": "settlement@governance",
    }
    finalized_event = {
        "sequence": 9,
        "block_height": 42,
        "block_hash": fixed(0xA0),
        "event_index": 2,
        "event": {
            "kind": {"kind": "receipt_recorded", "detail": None},
            "order_id": None,
            "trade_id": fixed(0x22),
            "channel_id": fixed(0x33),
            "receipt_id": fixed(0x44),
            "provider_id": fixed(0x55),
            "book_revision": 10,
            "authority": "settlement@governance",
            "occurred_at_unix_ms": 1_700_000_104_000,
        },
    }
    status = {
        "open_orders": 1,
        "partially_filled_orders": 0,
        "filled_orders": 1,
        "cancelled_orders": 0,
        "expired_orders": 0,
        "trades": 1,
        "settlement_receipts": 1,
        "settlement_channels": 1,
        "open_settlement_channels": 1,
        "book_revision": 10,
        "next_admission_sequence": 8,
        "next_trade_sequence": 4,
        "updated_at_unix": 1_700_000_104,
    }
    submission_receipt = {
        "payload": {
            "entrypoint_hash": _canonical_hash(0x72),
            "signed_transaction_hash": _canonical_hash(0x73),
            "submitted_at_ms": 1_700_000_200_000,
            "submitted_at_height": 42,
            "signer": "ed0120ABCDEF",
        },
        "signature": "AB" * 64,
    }
    return {
        "fixed": fixed,
        "cursor": cursor,
        "order": order,
        "trade": trade,
        "channel": channel,
        "receipt": receipt,
        "finalized_event": finalized_event,
        "status": status,
        "submission_receipt": submission_receipt,
    }


def test_sorafs_orderbook_read_helpers_build_paths_and_normalize_payloads() -> None:
    payloads = _sample_sorafs_orderbook_payloads()
    cursor = payloads["cursor"]
    fixed = payloads["fixed"]
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "source": "finalized_chain",
                "status": payloads["status"],
                "orders": {
                    "finalized_cursor": cursor,
                    "orders": [payloads["order"]],
                    "has_more": True,
                    "next_after_order_id": fixed(0x11),
                },
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "source": "finalized_chain",
                "trades": {
                    "finalized_cursor": cursor,
                    "trades": [payloads["trade"]],
                    "has_more": False,
                    "next_after_trade_id": None,
                },
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "source": "finalized_chain",
                "channels": {
                    "finalized_cursor": cursor,
                    "channels": [payloads["channel"]],
                    "has_more": False,
                    "next_after_channel_id": None,
                },
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "source": "finalized_chain",
                "receipts": {
                    "finalized_cursor": cursor,
                    "receipts": [payloads["receipt"]],
                    "has_more": False,
                    "next_after_receipt_id": None,
                },
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "source": "finalized_chain",
                "events": {
                    "finalized_cursor": cursor,
                    "events": [payloads["finalized_event"]],
                    "has_more": True,
                    "next_after": {
                        "sequence": 9,
                        "block_height": 42,
                        "block_hash": fixed(0xA0),
                        "event_index": 2,
                    },
                },
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    anchor_hex = "a0" * 32
    book = client.get_sorafs_orderbook(
        expected_finalized_height=42,
        expected_finalized_block_hash_hex=anchor_hex,
        after_id_hex="10" * 32,
        limit=25,
        headers={"X-Trace": "book"},
    )
    assert book["source"] == "finalized_chain"
    assert book["status"]["book_revision"] == 10
    assert book["orders"]["orders"][0]["order_id"] == fixed(0x11)
    assert book["orders"]["finalized_cursor"] == cursor
    assert session.calls[0]["method"] == "GET"
    assert session.calls[0]["url"].endswith("/v1/sorafs/orderbook/book")
    assert session.calls[0]["params"] == {
        "expected_finalized_height": 42,
        "expected_finalized_block_hash_hex": anchor_hex,
        "after_id_hex": "10" * 32,
        "limit": 25,
    }
    assert session.calls[0]["headers"]["X-Trace"] == "book"
    trades = client.list_sorafs_orderbook_trades()
    assert trades["trades"]["trades"][0]["trade_id"] == fixed(0x22)
    assert session.calls[1]["url"].endswith("/v1/sorafs/orderbook/trades")
    channels = client.list_sorafs_orderbook_channels()
    assert channels["channels"]["channels"][0]["provider_id"] == fixed(0x55)
    assert channels["channels"]["channels"][0]["status"] == {
        "status": "open",
        "value": None,
    }
    assert session.calls[2]["url"].endswith("/v1/sorafs/orderbook/channels")
    receipts = client.list_sorafs_orderbook_receipts()
    assert receipts["receipts"]["receipts"][0]["receipt_id"] == fixed(0x44)
    assert session.calls[3]["url"].endswith("/v1/sorafs/orderbook/receipts")
    events = client.list_sorafs_orderbook_events(
        expected_finalized_height=42,
        expected_finalized_block_hash_hex=anchor_hex,
        after_sequence=8,
        after_block_height=41,
        after_block_hash_hex="9f" * 32,
        after_event_index=1,
        limit=10,
        if_none_match='"old-events"',
    )
    assert events is not None
    event = events["events"]["events"][0]
    assert event["event"]["kind"] == {"kind": "receipt_recorded", "detail": None}
    assert event["event"]["receipt_id"] == fixed(0x44)
    assert session.calls[4]["url"].endswith("/v1/sorafs/orderbook/events")
    assert session.calls[4]["params"] == {
        "expected_finalized_height": 42,
        "expected_finalized_block_hash_hex": anchor_hex,
        "after_sequence": 8,
        "after_block_height": 41,
        "after_block_hash_hex": "9f" * 32,
        "after_event_index": 1,
        "limit": 10,
    }
    assert session.calls[4]["headers"]["If-None-Match"] == '"old-events"'


def test_sorafs_orderbook_read_helpers_validate_options_and_cache_status() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())

    with pytest.raises(ValueError, match="1..=500"):
        client.list_sorafs_orderbook_events(limit=0)
    with pytest.raises(ValueError, match="1..=500"):
        client.list_sorafs_orderbook_events(limit=501)
    with pytest.raises(ValueError, match="requires expected_finalized_height"):
        client.get_sorafs_orderbook(expected_finalized_height=7)
    with pytest.raises(ValueError, match="lowercase hexadecimal"):
        client.get_sorafs_orderbook(
            expected_finalized_height=7,
            expected_finalized_block_hash_hex="AA" * 32,
        )
    with pytest.raises(ValueError, match="all-zero"):
        client.get_sorafs_orderbook(
            expected_finalized_height=7,
            expected_finalized_block_hash_hex="00" * 32,
        )
    with pytest.raises(ValueError, match="all four finalized event cursor"):
        client.list_sorafs_orderbook_events(after_sequence=1)
    with pytest.raises(TypeError, match="unexpected keyword argument 'since'"):
        client.list_sorafs_orderbook_events(since=0)  # type: ignore[call-arg]
    with pytest.raises(TypeError, match="unexpected keyword argument 'etag'"):
        client.list_sorafs_orderbook_events(etag='"old"')  # type: ignore[call-arg]
    with pytest.raises(TypeError, match="headers must be a mapping"):
        client.get_sorafs_orderbook(headers="not-a-mapping")  # type: ignore[arg-type]

    session = RecordingSession()
    session.queue(StubResponse(status_code=304))
    cached_client = ToriiClient("http://node.test", session=session)

    assert cached_client.list_sorafs_orderbook_events(if_none_match='"same"') is None
    assert session.calls[0]["headers"]["If-None-Match"] == '"same"'


@pytest.mark.parametrize(
    ("parser", "payload_key", "retired_field"),
    [
        (
            ToriiClient._parse_sorafs_orderbook_order_record,
            "order",
            "price_per_gib_micro_xor",
        ),
        (
            ToriiClient._parse_sorafs_orderbook_trade_record,
            "trade",
            "maker_fee_micro_xor",
        ),
        (
            ToriiClient._parse_sorafs_orderbook_channel_record,
            "channel",
            "xor_locked_micro",
        ),
        (
            ToriiClient._parse_sorafs_orderbook_receipt_record,
            "receipt",
            "provider_credit_micro",
        ),
    ],
)
def test_sorafs_orderbook_exact_records_reject_legacy_duplicate_fields(
    parser: Callable[..., Dict[str, Any]],
    payload_key: str,
    retired_field: str,
) -> None:
    record = dict(_sample_sorafs_orderbook_payloads()[payload_key])
    record[retired_field] = "1"

    with pytest.raises(ValueError, match="unknown or retired"):
        parser(record, context=payload_key)


def test_sorafs_orderbook_exact_records_reject_unknown_fields() -> None:
    order = dict(_sample_sorafs_orderbook_payloads()["order"])
    order["unexpected_amount"] = "1"

    with pytest.raises(ValueError, match="unexpected_amount"):
        ToriiClient._parse_sorafs_orderbook_order_record(order, context="order")


def test_sorafs_orderbook_native_parsers_reject_noncanonical_wire_values() -> None:
    payloads = _sample_sorafs_orderbook_payloads()

    order = copy.deepcopy(payloads["order"])
    order["order_id"][0] = True
    with pytest.raises(TypeError, match="integer byte"):
        ToriiClient._parse_sorafs_orderbook_order_record(order, context="order")

    order = copy.deepcopy(payloads["order"])
    order["canonical_order"] = "YQ"
    with pytest.raises(ValueError, match="canonical"):
        ToriiClient._parse_sorafs_orderbook_order_record(order, context="order")

    order = copy.deepcopy(payloads["order"])
    order["status"] = {"status": "open"}
    with pytest.raises(ValueError, match="missing value"):
        ToriiClient._parse_sorafs_orderbook_order_record(order, context="order")

    page = {
        "finalized_cursor": payloads["cursor"],
        "orders": [payloads["order"]],
        "has_more": True,
        "next_after_order_id": None,
    }
    with pytest.raises(ValueError, match="presence must match has_more"):
        ToriiClient._parse_sorafs_orderbook_order_page(page, context="orders")


def test_expect_status_surfaces_error_envelope_details() -> None:
    response = StubResponse(
        429,
        {
            "code": "queue_full",
            "message": "transaction queue is at capacity",
            "details": {
                "reject_code": "TX_QUEUE_FULL",
                "retry_after_seconds": 1,
                "queue": {
                    "state": "saturated",
                    "queued": 128,
                    "capacity": 128,
                    "saturated": True,
                },
            },
        },
    )

    with pytest.raises(RuntimeError) as exc:
        ToriiClient._expect_status(response, (200,))

    message = str(exc.value)
    assert "transaction queue is at capacity" in message
    assert "reject_code=TX_QUEUE_FULL" in message


def test_expect_status_ignores_adversarial_non_string_reject_code() -> None:
    response = StubResponse(
        400,
        {
            "code": "bad_request",
            "message": "bad request",
            "details": {
                "reject_code": {"unexpected": "object"},
                "axt": {"code": ["array"]},
            },
        },
    )

    with pytest.raises(RuntimeError) as exc:
        ToriiClient._expect_status(response, (200,))

    message = str(exc.value)
    assert "bad request" in message
    assert "reject_code=" not in message
    assert "object" not in message
    assert "array" not in message


VPN_ACCOUNT = "vpn-user@paynet"
VPN_OPERATOR = "vpn-operator@paynet"
VPN_ESCROW = "vpn-escrow@paynet"
VPN_QUOTE_ID = "11" * 32
VPN_QUOTE_SESSION_ID = "44" * 16
VPN_PAYMENT_HASH = "22" * 32
VPN_METERING_KEY = "33" * 32
VPN_LEASE_ID = VPN_QUOTE_ID
VPN_HELPER_TICKET_HEX = "5356504e48543100" + "00" * 688
VPN_RELAY_ID_HEX = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"


def _vpn_trust_fields(spki: str = "ab" * 32) -> Dict[str, str]:
    return {
        "relay_id_hex": VPN_RELAY_ID_HEX,
        "descriptor_commit_hex": "cd" * 32,
        "tls_server_name": "relay.example",
        "relay_tls_spki_sha256_hex": spki,
        "relay_certificate_sha256_hex": "ef" * 32,
        "directory_snapshot_digest_hex": "42" * 32,
    }


def _vpn_instruction(wire_id: str = "OpenVpnLeaseEscrow") -> Dict[str, str]:
    return {"wire_id": wire_id, "payload_hex": "ab" * 8}


def _vpn_profile_payload() -> Dict[str, Any]:
    return {
        "available": True,
        "relay_endpoint": "/dns4/relay.example/udp/443/quic",
        "supported_exit_classes": ["standard", "low-latency", "high-security"],
        "default_exit_class": "standard",
        "lease_secs": 3600,
        "dns_push_interval_secs": 60,
        "meter_family": "soranet.vpn.v1",
        "route_pushes": ["0.0.0.0/0"],
        "excluded_routes": ["10.0.0.0/8"],
        "dns_servers": ["1.1.1.1"],
        "tunnel_addresses": ["10.208.0.2/32"],
        "mtu_bytes": 1280,
        "display_billing_label": "standard - soranet.vpn.v1 - 100.25 XOR",
        "operator_account_id": VPN_OPERATOR,
        "lease_fee": "100.25",
        "settlement_grace_secs": 300,
        "flow_label_bits": 24,
        "padding_budget_ms": 250,
        **_vpn_trust_fields(),
    }


def _vpn_quote_payload() -> Dict[str, Any]:
    payload = _vpn_profile_payload()
    return {
        "quote_id": VPN_QUOTE_ID,
        "lease_id_hex": VPN_LEASE_ID,
        "session_id_hex": VPN_QUOTE_SESSION_ID,
        "payment_reference": VPN_QUOTE_ID,
        "account_id": VPN_ACCOUNT,
        "exit_class": "standard",
        "relay_endpoint": payload["relay_endpoint"],
        "lease_secs": payload["lease_secs"],
        "quote_expires_at_ms": 1_700_000_000_000,
        "fee_asset_id": "xor#universal",
        "escrow_account_id": VPN_ESCROW,
        "operator_account_id": VPN_OPERATOR,
        "lease_fee": payload["lease_fee"],
        "route_pushes": payload["route_pushes"],
        "excluded_routes": payload["excluded_routes"],
        "dns_servers": payload["dns_servers"],
        "tunnel_addresses": payload["tunnel_addresses"],
        "mtu_bytes": payload["mtu_bytes"],
        "meter_family": payload["meter_family"],
        "flow_label_bits": payload["flow_label_bits"],
        "padding_budget_ms": payload["padding_budget_ms"],
        **_vpn_trust_fields(payload["relay_tls_spki_sha256_hex"]),
        "metering_public_key_hex": VPN_METERING_KEY,
        "open_lease_instruction": _vpn_instruction(),
    }


def _vpn_session_payload() -> Dict[str, Any]:
    quote_payload = _vpn_quote_payload()
    return {
        "session_id": VPN_QUOTE_ID,
        "account_id": VPN_ACCOUNT,
        "exit_class": quote_payload["exit_class"],
        "relay_endpoint": quote_payload["relay_endpoint"],
        "lease_secs": quote_payload["lease_secs"],
        "expires_at_ms": quote_payload["quote_expires_at_ms"],
        "connected_at_ms": 1_699_999_999_000,
        "meter_family": quote_payload["meter_family"],
        "quote_id": VPN_QUOTE_ID,
        "payment_reference": VPN_QUOTE_ID,
        "payment_tx_hash": VPN_PAYMENT_HASH,
        "fee_asset_id": quote_payload["fee_asset_id"],
        "escrow_account_id": VPN_ESCROW,
        "operator_account_id": VPN_OPERATOR,
        "lease_fee": quote_payload["lease_fee"],
        "flow_label_bits": quote_payload["flow_label_bits"],
        "padding_budget_ms": quote_payload["padding_budget_ms"],
        **_vpn_trust_fields(quote_payload["relay_tls_spki_sha256_hex"]),
        "route_pushes": quote_payload["route_pushes"],
        "excluded_routes": quote_payload["excluded_routes"],
        "dns_servers": quote_payload["dns_servers"],
        "tunnel_addresses": quote_payload["tunnel_addresses"],
        "mtu_bytes": quote_payload["mtu_bytes"],
        "helper_ticket_hex": VPN_HELPER_TICKET_HEX,
        "bytes_in": 0,
        "bytes_out": 0,
        "status": "active",
    }


def _vpn_receipt_payload(status: str = "settled") -> Dict[str, Any]:
    session_payload = _vpn_session_payload()
    return {
        "session_id": VPN_QUOTE_ID,
        "account_id": VPN_ACCOUNT,
        "exit_class": session_payload["exit_class"],
        "relay_endpoint": session_payload["relay_endpoint"],
        "meter_family": session_payload["meter_family"],
        "connected_at_ms": session_payload["connected_at_ms"],
        "disconnected_at_ms": session_payload["connected_at_ms"] + 60_000,
        "duration_ms": 60_000,
        "bytes_in": 1024,
        "bytes_out": 2048,
        "status": status,
        "receipt_source": "relay" if status == "settled" else "torii",
        "quote_id": VPN_QUOTE_ID,
        "payment_tx_hash": VPN_PAYMENT_HASH,
        "fee_asset_id": session_payload["fee_asset_id"],
        "escrow_account_id": VPN_ESCROW,
        "operator_account_id": VPN_OPERATOR,
        "lease_fee": session_payload["lease_fee"],
        "earned_fee": "25.125",
        "refunded_fee": "75.125",
        "lease_id_hex": VPN_LEASE_ID,
        "settle_lease_instruction": _vpn_instruction("SettleVpnLease"),
    }


def _vpn_auth(captured: List[bytes]) -> ToriiCanonicalRequestAuth:
    def signer(message: bytes) -> bytes:
        captured.append(message)
        return b"\x7a" * 64

    return ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id=VPN_ACCOUNT,
        signer=signer,
        timestamp_ms=1_700_000_001_000,
        nonce="vpn-test-nonce",
    )


def test_signed_vpn_methods_require_canonical_auth_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    omitted_auth_calls = [
        lambda: client.create_vpn_quote(
            VpnQuoteCreateRequest(metering_public_key_hex=VPN_METERING_KEY)
        ),
        lambda: client.create_vpn_session(
            VpnSessionCreateRequest(
                quote_id=VPN_QUOTE_ID,
                payment_tx_hash=VPN_PAYMENT_HASH,
                metering_public_key_hex=VPN_METERING_KEY,
            )
        ),
        lambda: client.get_vpn_session(VPN_QUOTE_ID),
        lambda: client.delete_vpn_session(VPN_QUOTE_ID),
        lambda: client.submit_vpn_receipt(
            VpnReceiptSubmitRequest(
                relay_receipt_hex="abcd",
                client_voucher_hex="beef",
            )
        ),
        client.list_vpn_receipts,
    ]

    for invoke in omitted_auth_calls:
        with pytest.raises(TypeError, match=r"canonical_auth"):
            invoke()

    explicit_none_calls = [
        lambda: client.create_vpn_quote(
            VpnQuoteCreateRequest(metering_public_key_hex=VPN_METERING_KEY),
            canonical_auth=None,  # type: ignore[arg-type]
        ),
        lambda: client.create_vpn_session(
            VpnSessionCreateRequest(
                quote_id=VPN_QUOTE_ID,
                payment_tx_hash=VPN_PAYMENT_HASH,
                metering_public_key_hex=VPN_METERING_KEY,
            ),
            canonical_auth=None,  # type: ignore[arg-type]
        ),
        lambda: client.get_vpn_session(
            VPN_QUOTE_ID,
            canonical_auth=None,  # type: ignore[arg-type]
        ),
        lambda: client.delete_vpn_session(
            VPN_QUOTE_ID,
            canonical_auth=None,  # type: ignore[arg-type]
        ),
        lambda: client.submit_vpn_receipt(
            VpnReceiptSubmitRequest(
                relay_receipt_hex="abcd",
                client_voucher_hex="beef",
            ),
            canonical_auth=None,  # type: ignore[arg-type]
        ),
        lambda: client.list_vpn_receipts(
            canonical_auth=None,  # type: ignore[arg-type]
        ),
    ]
    for invoke in explicit_none_calls:
        with pytest.raises(ValueError, match=r"canonical_auth is required"):
            invoke()
    assert session.calls == []


def test_vpn_request_mappings_reject_unknown_fields_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    auth = _vpn_auth([])
    calls = [
        lambda: client.create_vpn_quote(
            {"metering_public_key_hex": VPN_METERING_KEY, "unexpected": True},
            canonical_auth=auth,
        ),
        lambda: client.create_vpn_session(
            {
                "quote_id": VPN_QUOTE_ID,
                "payment_tx_hash": VPN_PAYMENT_HASH,
                "metering_public_key_hex": VPN_METERING_KEY,
                "unexpected": True,
            },
            canonical_auth=auth,
        ),
        lambda: client.submit_vpn_receipt(
            {
                "relay_receipt_hex": "abcd",
                "client_voucher_hex": "beef",
                "unexpected": True,
            },
            canonical_auth=auth,
        ),
    ]

    for invoke in calls:
        with pytest.raises(RuntimeError, match=r"unsupported fields: unexpected"):
            invoke()
    assert session.calls == []


def test_vpn_requests_keep_openapi_allowed_prefixed_mixed_case_hex() -> None:
    metering_key = "ab" * 32
    payment_hash = "cd" * 32
    lease_id = "ef" * 32
    assert ToriiClient._normalize_vpn_quote_request(
        {"metering_public_key_hex": "0X" + ("aB" * 32)}
    )["metering_public_key_hex"] == metering_key
    session_payload = ToriiClient._normalize_vpn_session_request(
        {
            "quote_id": VPN_QUOTE_ID,
            "payment_tx_hash": "0x" + ("cD" * 32),
            "metering_public_key_hex": "0X" + ("aB" * 32),
        }
    )
    assert session_payload["payment_tx_hash"] == payment_hash
    assert session_payload["metering_public_key_hex"] == metering_key
    receipt_payload = ToriiClient._normalize_vpn_receipt_request(
        {
            "relay_receipt_hex": "0XABCD",
            "client_voucher_hex": "0xBEEF",
            "lease_id_hex": "0X" + ("eF" * 32),
        }
    )
    assert receipt_payload == {
        "relay_receipt_hex": "abcd",
        "client_voucher_hex": "beef",
        "lease_id_hex": lease_id,
    }
    with pytest.raises(RuntimeError, match=r"quote_id must be an exact lowercase"):
        ToriiClient._normalize_vpn_session_request(
            {
                "quote_id": "0X" + VPN_QUOTE_ID,
                "payment_tx_hash": payment_hash,
                "metering_public_key_hex": metering_key,
            }
        )
    with pytest.raises(RuntimeError, match=r"exit_class must be one of"):
        ToriiClient._normalize_vpn_quote_request(
            {
                "exit_class": "fastest",
                "metering_public_key_hex": metering_key,
            }
        )
    with pytest.raises(RuntimeError, match=r"exit_class must be one of"):
        ToriiClient._normalize_vpn_session_request(
            {
                "exit_class": "fastest",
                "quote_id": VPN_QUOTE_ID,
                "payment_tx_hash": payment_hash,
                "metering_public_key_hex": metering_key,
            }
        )


def test_create_vpn_quote_signs_body_and_parses_open_lease_instruction() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=201, payload=_vpn_quote_payload()))
    captured: List[bytes] = []
    auth = _vpn_auth(captured)
    client = ToriiClient("https://node.test", session=session)

    quote = client.create_vpn_quote(
        VpnQuoteCreateRequest(
            metering_public_key_hex=bytes.fromhex(VPN_METERING_KEY),
            exit_class="standard",
        ),
        canonical_auth=auth,
    )

    body = session.calls[0]["data"]
    assert body == (
        b'{"exit_class":"standard","metering_public_key_hex":"'
        + VPN_METERING_KEY.encode("ascii")
        + b'"}'
    )
    assert captured == [
        canonical_network_request_signature_message(
            auth.network_id,
            "POST",
            "/v1/vpn/quotes",
            body,
            timestamp_ms=auth.timestamp_ms or 0,
            nonce=auth.nonce or "",
        )
    ]
    headers = session.calls[0]["headers"]
    assert headers["X-Iroha-Account"] == VPN_ACCOUNT
    assert headers["X-Iroha-Signature"] == base64.b64encode(b"\x7a" * 64).decode("ascii")
    assert headers["X-Iroha-Timestamp-Ms"] == str(auth.timestamp_ms)
    assert headers["X-Iroha-Nonce"] == auth.nonce
    assert quote.lease_id_hex == VPN_LEASE_ID
    assert quote.open_lease_instruction.wire_id == "OpenVpnLeaseEscrow"
    assert quote.open_lease_instruction.payload_hex == "ab" * 8


def test_canonical_request_auth_rejects_padded_fields_before_send() -> None:
    def signer(message: bytes) -> bytes:
        return b"\x7a" * 64

    with pytest.raises(ValueError, match="surrounding whitespace"):
        canonical_network_request_signature_message(
            GOVERNANCE_NETWORK_ID,
            "POST",
            "/v1/vpn/quotes",
            b"{}",
            timestamp_ms=1,
            nonce=" nonce",
        )
    with pytest.raises(ValueError, match="printable ASCII"):
        canonical_network_request_signature_message(
            GOVERNANCE_NETWORK_ID,
            "POST",
            "/v1/vpn/quotes",
            b"{}",
            timestamp_ms=1,
            nonce="nonce value",
        )
    with pytest.raises(ValueError, match="printable ASCII"):
        canonical_network_request_signature_message(
            GOVERNANCE_NETWORK_ID,
            "POST",
            "/v1/vpn/quotes",
            b"{}",
            timestamp_ms=1,
            nonce="nönce",
        )
    with pytest.raises(ValueError, match="at most 256"):
        canonical_network_request_signature_message(
            GOVERNANCE_NETWORK_ID,
            "POST",
            "/v1/vpn/quotes",
            b"{}",
            timestamp_ms=1,
            nonce="n" * 257,
        )
    with pytest.raises((TypeError, ValueError), match="unsigned 64-bit"):
        canonical_network_request_signature_message(
            GOVERNANCE_NETWORK_ID,
            "POST",
            "/v1/vpn/quotes",
            b"{}",
            timestamp_ms=-1,
            nonce="nonce",
        )
    with pytest.raises(ValueError, match="non-empty string"):
        build_canonical_request_headers(
            network_id=GOVERNANCE_NETWORK_ID,
            account_id=VPN_ACCOUNT,
            signer=signer,
            method="POST",
            path="/v1/vpn/quotes",
            body=b"{}",
            timestamp_ms=1,
            nonce="",
        )
    with pytest.raises(ValueError, match="surrounding whitespace"):
        build_canonical_request_headers(
            network_id=GOVERNANCE_NETWORK_ID,
            account_id=f"{VPN_ACCOUNT} ",
            signer=signer,
            method="POST",
            path="/v1/vpn/quotes",
            body=b"{}",
            timestamp_ms=1,
            nonce="nonce",
        )
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(ValueError, match="surrounding whitespace"):
        client.create_vpn_quote(
            VpnQuoteCreateRequest(
                metering_public_key_hex=bytes.fromhex(VPN_METERING_KEY),
                exit_class="standard",
            ),
            canonical_auth=ToriiCanonicalRequestAuth(
                network_id=GOVERNANCE_NETWORK_ID,
                account_id=VPN_ACCOUNT,
                signer=signer,
                timestamp_ms=1,
                nonce="nonce ",
            ),
        )
    assert session.calls == []


def test_vpn_session_accepts_exact_lowercase_696_byte_helper_ticket() -> None:
    parsed = ToriiClient._parse_vpn_session(
        _vpn_session_payload(),
        context="vpn session response",
    )

    assert parsed.helper_ticket_hex == VPN_HELPER_TICKET_HEX
    assert len(parsed.helper_ticket_hex) == 1392


@pytest.mark.parametrize(
    "helper_ticket_hex",
    [
        "0x" + VPN_HELPER_TICKET_HEX,
        VPN_HELPER_TICKET_HEX.upper(),
        VPN_HELPER_TICKET_HEX[:-1],
        VPN_HELPER_TICKET_HEX[:-2],
    ],
    ids=["prefix", "uppercase", "odd-length", "wrong-even-length"],
)
def test_vpn_session_rejects_noncanonical_helper_ticket(helper_ticket_hex: str) -> None:
    payload = _vpn_session_payload()
    payload["helper_ticket_hex"] = helper_ticket_hex

    with pytest.raises(
        RuntimeError,
        match=r"helper_ticket_hex must contain exactly 1392 lowercase hexadecimal characters",
    ):
        ToriiClient._parse_vpn_session(payload, context="vpn session response")


def test_vpn_response_parsers_reject_unknown_fields() -> None:
    cases = [
        (ToriiClient._parse_vpn_profile, _vpn_profile_payload(), "vpn profile"),
        (ToriiClient._parse_vpn_quote, _vpn_quote_payload(), "vpn quote"),
        (ToriiClient._parse_vpn_session, _vpn_session_payload(), "vpn session"),
        (ToriiClient._parse_vpn_receipt, _vpn_receipt_payload(), "vpn receipt"),
        (
            ToriiClient._parse_vpn_receipt_list,
            {"items": [_vpn_receipt_payload()], "total": 1},
            "vpn receipts",
        ),
    ]
    for parser, payload, context in cases:
        payload["unexpected"] = True
        with pytest.raises(RuntimeError, match=r"unsupported fields: unexpected"):
            parser(payload, context=context)

    nested = _vpn_quote_payload()
    nested["open_lease_instruction"]["unexpected"] = True
    with pytest.raises(RuntimeError, match=r"unsupported fields: unexpected"):
        ToriiClient._parse_vpn_quote(nested, context="vpn quote")


def test_vpn_response_parsers_require_all_openapi_fields() -> None:
    cases = [
        (
            ToriiClient._parse_vpn_profile,
            _vpn_profile_payload,
            "relay_tls_spki_sha256_hex",
            "vpn profile",
        ),
        (
            ToriiClient._parse_vpn_quote,
            _vpn_quote_payload,
            "open_lease_instruction",
            "vpn quote",
        ),
        (
            ToriiClient._parse_vpn_session,
            _vpn_session_payload,
            "route_pushes",
            "vpn session",
        ),
        (
            ToriiClient._parse_vpn_receipt,
            _vpn_receipt_payload,
            "settle_lease_instruction",
            "vpn receipt",
        ),
        (
            ToriiClient._parse_vpn_receipt_list,
            lambda: {"items": [_vpn_receipt_payload()], "total": 1},
            "total",
            "vpn receipts",
        ),
    ]
    for parser, payload_factory, missing_field, context in cases:
        payload = payload_factory()
        payload.pop(missing_field)
        with pytest.raises(RuntimeError, match=rf"missing required fields: {missing_field}"):
            parser(payload, context=context)

    nested = _vpn_quote_payload()
    nested["open_lease_instruction"].pop("payload_hex")
    with pytest.raises(RuntimeError, match=r"missing required fields: payload_hex"):
        ToriiClient._parse_vpn_quote(nested, context="vpn quote")

    session = _vpn_session_payload()
    session["route_pushes"] = None
    with pytest.raises(RuntimeError, match=r"route_pushes must be a list"):
        ToriiClient._parse_vpn_session(session, context="vpn session")


def test_vpn_response_parsers_reject_empty_min_length_strings() -> None:
    cases = [
        (
            ToriiClient._parse_vpn_profile,
            _vpn_profile_payload,
            "vpn profile",
            (
                "relay_endpoint",
                "meter_family",
                "display_billing_label",
                "operator_account_id",
            ),
        ),
        (
            ToriiClient._parse_vpn_quote,
            _vpn_quote_payload,
            "vpn quote",
            (
                "payment_reference",
                "account_id",
                "relay_endpoint",
                "fee_asset_id",
                "escrow_account_id",
                "operator_account_id",
                "meter_family",
            ),
        ),
        (
            ToriiClient._parse_vpn_session,
            _vpn_session_payload,
            "vpn session",
            (
                "account_id",
                "relay_endpoint",
                "meter_family",
                "payment_reference",
                "fee_asset_id",
                "escrow_account_id",
                "operator_account_id",
            ),
        ),
        (
            ToriiClient._parse_vpn_receipt,
            _vpn_receipt_payload,
            "vpn receipt",
            (
                "account_id",
                "relay_endpoint",
                "meter_family",
                "fee_asset_id",
                "escrow_account_id",
                "operator_account_id",
            ),
        ),
    ]
    for parser, payload_factory, context, fields in cases:
        for field in fields:
            payload = payload_factory()
            payload[field] = ""
            with pytest.raises(RuntimeError, match=field):
                parser(payload, context=context)

    instruction = _vpn_quote_payload()
    instruction["open_lease_instruction"]["wire_id"] = ""
    with pytest.raises(RuntimeError, match=r"wire_id"):
        ToriiClient._parse_vpn_quote(instruction, context="vpn quote")


def test_vpn_response_parsers_enforce_openapi_enums_and_bounds() -> None:
    cases = [
        (
            "profile exit set",
            ToriiClient._parse_vpn_profile,
            _vpn_profile_payload(),
            lambda payload: payload.__setitem__(
                "supported_exit_classes",
                ["standard", "standard", "high-security"],
            ),
            "supported_exit_classes",
            "vpn profile",
        ),
        (
            "profile lease lower bound",
            ToriiClient._parse_vpn_profile,
            _vpn_profile_payload(),
            lambda payload: payload.__setitem__("lease_secs", 0),
            "lease_secs",
            "vpn profile",
        ),
        (
            "profile settlement lower bound",
            ToriiClient._parse_vpn_profile,
            _vpn_profile_payload(),
            lambda payload: payload.__setitem__("settlement_grace_secs", 0),
            "settlement_grace_secs",
            "vpn profile",
        ),
        (
            "retired quote instruction array",
            ToriiClient._parse_vpn_quote,
            _vpn_quote_payload(),
            lambda payload: payload.__setitem__("tx_instructions", []),
            "tx_instructions",
            "vpn quote",
        ),
        (
            "quote exit enum",
            ToriiClient._parse_vpn_quote,
            _vpn_quote_payload(),
            lambda payload: payload.__setitem__("exit_class", "fastest"),
            "exit_class",
            "vpn quote",
        ),
        (
            "session mtu constant",
            ToriiClient._parse_vpn_session,
            _vpn_session_payload(),
            lambda payload: payload.__setitem__("mtu_bytes", 1500),
            "mtu_bytes",
            "vpn session",
        ),
        (
            "session flow label constant",
            ToriiClient._parse_vpn_session,
            _vpn_session_payload(),
            lambda payload: payload.__setitem__("flow_label_bits", 20),
            "flow_label_bits",
            "vpn session",
        ),
        (
            "session padding lower bound",
            ToriiClient._parse_vpn_session,
            _vpn_session_payload(),
            lambda payload: payload.__setitem__("padding_budget_ms", 0),
            "padding_budget_ms",
            "vpn session",
        ),
        (
            "session status constant",
            ToriiClient._parse_vpn_session,
            _vpn_session_payload(),
            lambda payload: payload.__setitem__("status", "connected"),
            "status",
            "vpn session",
        ),
        (
            "receipt status enum",
            ToriiClient._parse_vpn_receipt,
            _vpn_receipt_payload(),
            lambda payload: payload.__setitem__("status", "active"),
            "status",
            "vpn receipt",
        ),
        (
            "receipt source enum",
            ToriiClient._parse_vpn_receipt,
            _vpn_receipt_payload(),
            lambda payload: payload.__setitem__("receipt_source", "client"),
            "receipt_source",
            "vpn receipt",
        ),
        (
            "receipt instruction count",
            ToriiClient._parse_vpn_receipt,
            _vpn_receipt_payload(),
            lambda payload: payload.__setitem__(
                "tx_instructions",
                [_vpn_instruction(), _vpn_instruction()],
            ),
            "tx_instructions",
            "vpn receipt",
        ),
        (
            "receipt list item count",
            ToriiClient._parse_vpn_receipt_list,
            {"items": [_vpn_receipt_payload()] * 25, "total": 24},
            lambda payload: None,
            "items",
            "vpn receipts",
        ),
        (
            "receipt list total",
            ToriiClient._parse_vpn_receipt_list,
            {"items": [], "total": 25},
            lambda payload: None,
            "total",
            "vpn receipts",
        ),
    ]
    for _case_name, parser, payload, mutate, expected_field, context in cases:
        mutate(payload)
        with pytest.raises(RuntimeError, match=expected_field):
            parser(payload, context=context)


def test_vpn_response_parsers_require_json_uint64_integers() -> None:
    cases = [
        (
            ToriiClient._parse_vpn_profile,
            _vpn_profile_payload(),
            "dns_push_interval_secs",
            "30",
            "vpn profile",
        ),
        (
            ToriiClient._parse_vpn_quote,
            _vpn_quote_payload(),
            "quote_expires_at_ms",
            True,
            "vpn quote",
        ),
        (
            ToriiClient._parse_vpn_session,
            _vpn_session_payload(),
            "bytes_in",
            -1,
            "vpn session",
        ),
        (
            ToriiClient._parse_vpn_receipt,
            _vpn_receipt_payload(),
            "duration_ms",
            1 << 64,
            "vpn receipt",
        ),
    ]
    for parser, payload, field, invalid_value, context in cases:
        payload[field] = invalid_value
        with pytest.raises(RuntimeError, match=field):
            parser(payload, context=context)


@pytest.mark.parametrize(
    ("parser", "payload", "field", "value", "context"),
    [
        (
            ToriiClient._parse_vpn_profile,
            _vpn_profile_payload(),
            "relay_tls_spki_sha256_hex",
            "AC" * 32,
            "vpn profile",
        ),
        (
            ToriiClient._parse_vpn_quote,
            _vpn_quote_payload(),
            "quote_id",
            "AB" * 32,
            "vpn quote",
        ),
        (
            ToriiClient._parse_vpn_quote,
            _vpn_quote_payload(),
            "session_id_hex",
            "0x" + VPN_QUOTE_SESSION_ID,
            "vpn quote",
        ),
        (
            ToriiClient._parse_vpn_quote,
            _vpn_quote_payload(),
            "metering_public_key_hex",
            "CD" * 32,
            "vpn quote",
        ),
        (
            ToriiClient._parse_vpn_session,
            _vpn_session_payload(),
            "session_id",
            "0X" + VPN_QUOTE_ID,
            "vpn session",
        ),
        (
            ToriiClient._parse_vpn_session,
            _vpn_session_payload(),
            "payment_tx_hash",
            "EF" * 32,
            "vpn session",
        ),
        (
            ToriiClient._parse_vpn_receipt,
            _vpn_receipt_payload(),
            "lease_id_hex",
            "0x" + VPN_LEASE_ID,
            "vpn receipt",
        ),
    ],
    ids=[
        "profile-uppercase-spki",
        "quote-uppercase-id",
        "quote-prefixed-session-id",
        "quote-uppercase-metering-key",
        "session-prefixed-id",
        "session-uppercase-payment-hash",
        "receipt-prefixed-lease-id",
    ],
)
def test_vpn_response_parsers_reject_noncanonical_ids_and_hashes(
    parser: Callable[..., Any],
    payload: Dict[str, Any],
    field: str,
    value: str,
    context: str,
) -> None:
    payload[field] = value

    with pytest.raises(RuntimeError, match=r"exact lowercase"):
        parser(payload, context=context)


def test_vpn_session_delete_and_receipt_listing_use_native_receipts() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=201, payload=_vpn_session_payload()))
    session.queue(StubResponse(payload=_vpn_session_payload()))
    session.queue(StubResponse(status_code=200, payload=_vpn_receipt_payload("disconnected")))
    session.queue(
        StubResponse(
            payload={
                "items": [_vpn_receipt_payload("disconnected")],
                "total": 1,
            }
        )
    )
    session.queue(StubResponse(status_code=404))
    client = ToriiClient("https://node.test", session=session)
    captured: List[bytes] = []
    auth = _vpn_auth(captured)

    created = client.create_vpn_session(
        VpnSessionCreateRequest(
            quote_id=VPN_QUOTE_ID,
            payment_tx_hash=VPN_PAYMENT_HASH,
            metering_public_key_hex=VPN_METERING_KEY,
        ),
        canonical_auth=auth,
    )
    fetched = client.get_vpn_session(VPN_QUOTE_ID, canonical_auth=auth)
    deleted = client.delete_vpn_session(VPN_QUOTE_ID, canonical_auth=auth)
    receipts = client.list_vpn_receipts(canonical_auth=auth)
    missing = client.get_vpn_session(VPN_QUOTE_ID, canonical_auth=auth)

    assert created.session_id == VPN_QUOTE_ID
    assert fetched is not None and fetched.payment_tx_hash == VPN_PAYMENT_HASH
    assert deleted is not None and deleted.status == "disconnected"
    assert deleted.settle_lease_instruction is not None
    assert deleted.settle_lease_instruction.wire_id == "SettleVpnLease"
    assert receipts.total == 1
    assert receipts.items[0].refunded_fee == "75.125"
    assert missing is None
    assert [call["method"] for call in session.calls] == ["POST", "GET", "DELETE", "GET", "GET"]


def test_submit_vpn_receipt_parses_settlement_instruction() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=201, payload=_vpn_receipt_payload()))
    client = ToriiClient("https://node.test", session=session)
    captured: List[bytes] = []

    receipt = client.submit_vpn_receipt(
        VpnReceiptSubmitRequest(
            relay_receipt_hex="aa" * 12,
            client_voucher_hex="bb" * 12,
            lease_id_hex=VPN_LEASE_ID,
        ),
        canonical_auth=_vpn_auth(captured),
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload == {
        "client_voucher_hex": "bb" * 12,
        "lease_id_hex": VPN_LEASE_ID,
        "relay_receipt_hex": "aa" * 12,
    }
    assert receipt.status == "settled"
    assert receipt.earned_fee == "25.125"
    assert receipt.refunded_fee == "75.125"
    assert receipt.settle_lease_instruction is not None
    assert receipt.settle_lease_instruction.wire_id == "SettleVpnLease"


def test_list_peers_returns_typed_records() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=[
                {"address": "127.0.0.1:1337", "id": {"public_key": "ed01"}},
                {"address": "[::1]:1337", "id": {"public_key": "ed02"}},
            ]
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )
    peers = client.list_peers()
    assert len(peers) == 2
    assert peers[0].address == "127.0.0.1:1337"
    assert peers[0].public_key_hex == "ed01"
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"] == "http://node.test/v1/peers"
    assert call["params"] == {}
    assert call["data"] is None
    assert call["allow_redirects"] is False
    assert call["stream"] is False
    for header in (
        "X-Iroha-Operator-Public-Key",
        "X-Iroha-Operator-Timestamp-Ms",
        "X-Iroha-Operator-Nonce",
        "X-Iroha-Operator-Signature",
    ):
        assert call["headers"][header]


def test_fee_quote_posts_exact_payload_with_authority_signature() -> None:
    session = RecordingSession()
    quote = {
        "intent": _authority_fee_payment(),
        "observation": {
            "ledger_time_ms": 10,
            "next_block_height": 4,
            "route_dataspace_id": 0,
        },
        "components": [],
        "capacities": [],
        "decision": {"status": "accepted", "value": {"debit_source": {}}},
    }
    session.queue(StubResponse(payload=quote))
    signed_messages: List[bytes] = []
    auth = ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=lambda message: signed_messages.append(message) or b"signature",
        timestamp_ms=123,
        nonce="fee-quote-nonce",
    )
    client = ToriiClient("https://node.test", session=session)
    draft = _fee_quote_transaction_payload()
    original_draft = copy.deepcopy(draft)
    expected_body = ToriiClient._encode_json_body({"payload": draft})

    assert client.quote_fees(draft, canonical_auth=auth) == quote

    call = session.calls[0]
    assert call["url"] == "https://node.test/v1/fees/quote"
    assert call["data"] == expected_body
    assert json.loads(call["data"].decode("utf-8")) == {"payload": draft}
    assert draft == original_draft
    assert call["headers"]["X-Iroha-Account"] == CANONICAL_OWNER_HEADER
    assert call["headers"]["X-Iroha-Timestamp-Ms"] == "123"
    assert call["headers"]["X-Iroha-Nonce"] == "fee-quote-nonce"
    assert len(signed_messages) == 1


def test_fee_quote_rejects_authority_substitution_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)
    auth = ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id="another-account",
        signer=lambda _message: b"signature",
    )

    with pytest.raises(ValueError, match="must equal the exact payload authority"):
        client.quote_fees(
            _fee_quote_transaction_payload(),
            canonical_auth=auth,
        )

    assert session.calls == []


@pytest.mark.parametrize(
    "requested, quoted",
    [
        (_authority_fee_payment(100), _authority_fee_payment(101)),
        (_sponsor_fee_payment(100), _authority_fee_payment(100)),
        (
            _sponsor_fee_payment(100),
            {
                **_sponsor_fee_payment(100),
                "value": {
                    **_sponsor_fee_payment(100)["value"],
                    "program_revision": 4,
                },
            },
        ),
    ],
)
def test_fee_quote_rejects_substituted_selection(
    requested: Dict[str, Any],
    quoted: Dict[str, Any],
) -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "intent": quoted,
                "observation": {},
                "components": [],
                "capacities": [],
                "decision": {},
            }
        )
    )
    client = ToriiClient("https://node.test", session=session)
    auth = ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=lambda _message: b"signature",
    )

    with pytest.raises(RuntimeError, match="changed the requested payer"):
        client.quote_fees(
            _fee_quote_transaction_payload(requested),
            canonical_auth=auth,
        )


@pytest.mark.parametrize(
    "domain",
    [
        None,
        {"kind": "genesis", "value": None},
        {"kind": "network", "value": GOVERNANCE_NETWORK_ID[5:69].lower()},
        {"kind": "network", "value": GOVERNANCE_NETWORK_ID.lower()},
        {
            "kind": "network",
            "value": GOVERNANCE_NETWORK_ID[:-1]
            + ("0" if GOVERNANCE_NETWORK_ID[-1] != "0" else "1"),
        },
        {"kind": "network", "value": _canonical_hash(0xA7)},
        {
            "kind": "network",
            "value": GOVERNANCE_NETWORK_ID,
            "chain": "legacy",
        },
    ],
    ids=(
        "not-an-object",
        "genesis-domain",
        "raw-network-id",
        "lowercase-marked-network-id",
        "bad-network-id-checksum",
        "foreign-network-id",
        "domain-extra-field",
    ),
)
def test_fee_quote_rejects_noncanonical_or_foreign_domain_before_dispatch(
    domain: Any,
) -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    draft = _fee_quote_transaction_payload()
    draft["domain"] = domain

    with pytest.raises((TypeError, ValueError)):
        client.quote_fees(draft, canonical_auth=_governance_auth())

    assert session.calls == []


def test_fee_quote_rejects_missing_domain_and_unknown_payload_fields() -> None:
    client = ToriiClient("https://node.test", session=RecordingSession())
    missing_domain = _fee_quote_transaction_payload()
    missing_domain.pop("domain")
    with pytest.raises(ValueError, match="missing domain"):
        client.quote_fees(missing_domain, canonical_auth=_governance_auth())

    unknown = _fee_quote_transaction_payload()
    unknown["future_identity"] = "forbidden"
    with pytest.raises(ValueError, match="unexpected future_identity"):
        client.quote_fees(unknown, canonical_auth=_governance_auth())


@pytest.mark.parametrize(
    "field",
    ["chain", "chain_id", "chainId", "genesis_hash", "genesisHash"],
)
def test_fee_quote_rejects_retired_transaction_identity_aliases(field: str) -> None:
    client = ToriiClient("https://node.test", session=RecordingSession())
    draft = _fee_quote_transaction_payload()
    draft[field] = "legacy"

    with pytest.raises(ValueError, match="retired transaction identity fields"):
        client.quote_fees(draft, canonical_auth=_governance_auth())


def test_fee_sponsor_program_lookup_is_account_signed_and_exact() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "id": {"sponsor": CANONICAL_OWNER, "name": "retail"},
                "payout_account": CANONICAL_OWNER,
                "lifecycle": "active",
            }
        )
    )
    auth = ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=lambda _message: b"signature",
        timestamp_ms=124,
        nonce="program-lookup-nonce",
    )
    client = ToriiClient("https://node.test", session=session)

    result = client.get_fee_sponsor_program(
        f"{CANONICAL_OWNER}/retail",
        canonical_auth=auth,
    )

    assert result["lifecycle"] == "active"
    assert result["payout_account"] == CANONICAL_OWNER
    assert json.loads(session.calls[0]["data"].decode("utf-8")) == {
        "program_id": f"{CANONICAL_OWNER}/retail"
    }
    assert session.calls[0]["headers"]["X-Iroha-Account"] == CANONICAL_OWNER_HEADER


def test_fee_sponsor_program_lookup_rejects_substituted_response_id() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "id": {"sponsor": CANONICAL_OWNER, "name": "other"},
                "lifecycle": "active",
            }
        )
    )
    client = ToriiClient("https://node.test", session=session)
    auth = ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=lambda _message: b"signature",
    )

    with pytest.raises(RuntimeError, match="does not match the requested program"):
        client.get_fee_sponsor_program(
            f"{CANONICAL_OWNER}/retail",
            canonical_auth=auth,
        )


def test_call_contract_posts_selector_payload_and_parses_response() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=200,
            payload=_contract_call_draft(
                fee_payment=_authority_fee_payment(5000),
            ),
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.prepare_contract_call(
        authority=CANONICAL_OWNER,
        contract_alias="router::universal",
        entrypoint="ping",
        payload={"value": 1, "labels": ["alpha"]},
        fee_payment=_authority_fee_payment(5000),
    )

    assert isinstance(result, ContractCallResponse)
    assert result.entrypoint == "ping"
    assert result.creation_time_ms == 42
    assert result.transaction_ttl_ms == 60_000
    assert result.entrypoint_hash_hex is None
    assert isinstance(result.operation_receipt, ContractOperationReceipt)
    assert result.operation_receipt.gas_limit == 5000
    assert result.operation_receipt.payload_digest_hex == "66" * 32
    assert result.submitted is False
    assert result.pipeline_status is None
    assert result.transaction_payload_b64 == base64.b64encode(b"\x01\x02\x03").decode("ascii")
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload == {
        "authority": CANONICAL_OWNER,
        "contract_alias": "router::universal",
        "entrypoint": "ping",
        "payload": {"value": 1, "labels": ["alpha"]},
        "fee_payment": _authority_fee_payment(5000),
    }


def test_pipeline_status_parser_exposes_only_public_metadata() -> None:
    transaction_hash = "ab" * 32
    parsed = ToriiClient._parse_pipeline_status_response(
        {
            "hash": transaction_hash,
            "status": {"kind": "Applied", "block_height": 7},
            "scope": "global",
            "resolved_from": "state",
        },
        context="pipeline status",
    )

    assert parsed.hash == transaction_hash
    assert parsed.status.kind == "Applied"
    assert parsed.status.block_height == 7
    assert parsed.scope == "global"
    assert parsed.resolved_from == "state"
    assert not hasattr(parsed, "diagnostics")
    assert not hasattr(parsed, "summary")
    assert not hasattr(parsed, "raw")


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("summary", "Rejected: secret"),
        ("diagnostics", [{"message": "secret"}]),
        ("trigger_completions", []),
        ("batch_transfer_outcomes", []),
    ],
)
def test_pipeline_status_parser_rejects_retired_detail_fields(
    field: str,
    value: object,
) -> None:
    payload = {
        "hash": "cd" * 32,
        "status": {"kind": "Rejected"},
        "scope": "global",
        "resolved_from": "state",
        field: value,
    }

    with pytest.raises(RuntimeError, match="unsupported fields"):
        ToriiClient._parse_pipeline_status_response(
            payload,
            context="pipeline status",
        )


@pytest.mark.parametrize("block_height", [None, 0, -1, True, 1.5])
def test_pipeline_status_parser_rejects_non_positive_or_non_integer_height(
    block_height: object,
) -> None:
    payload = {
        "hash": "cd" * 32,
        "status": {"kind": "Applied", "block_height": block_height},
        "scope": "global",
        "resolved_from": "state",
    }

    with pytest.raises(RuntimeError, match="block_height"):
        ToriiClient._parse_pipeline_status_response(
            payload,
            context="pipeline status",
        )


@pytest.mark.parametrize("transaction_hash", ["ab", "AB" * 32, "gg" * 32, " ab" * 32])
def test_pipeline_status_parser_rejects_noncanonical_hash(transaction_hash: str) -> None:
    payload = {
        "hash": transaction_hash,
        "status": {"kind": "Applied", "block_height": 1},
        "scope": "global",
        "resolved_from": "state",
    }

    with pytest.raises(RuntimeError, match="exact lowercase 32-byte hex"):
        ToriiClient._parse_pipeline_status_response(
            payload,
            context="pipeline status",
        )


def test_call_contract_preserves_shared_rust_argument_record_fixture() -> None:
    fixture_path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "kotodama"
        / "entrypoint_argument_record_v1.json"
    )
    fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
    assert fixture["codec"] == "EntrypointArgumentRecordV1"
    assert fixture["generator"] == "ivm::encode_argument_record_from_json"
    assert re.fullmatch(
        r"[0-9a-f]{64}",
        fixture["entrypoint_argument_schema_v1"]["schema_hash_hex"],
    )
    assert re.fullmatch(
        r"(?:[0-9a-f]{2})+",
        fixture["entrypoint_argument_record_v1"]["norito_hex"],
    )

    boundary = fixture["torii_boundary"]
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=200,
            payload=_contract_call_draft(
                entrypoint=boundary["entrypoint"],
                contract_alias=boundary["contract_alias"],
                fee_payment=boundary["fee_payment"],
            ),
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.prepare_contract_call(
        authority=boundary["authority"],
        contract_alias=boundary["contract_alias"],
        entrypoint=boundary["entrypoint"],
        payload=boundary["payload"],
        fee_payment=boundary["fee_payment"],
    )

    submitted = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert submitted == {
        "authority": boundary["authority"],
        "contract_alias": boundary["contract_alias"],
        "entrypoint": boundary["entrypoint"],
        "payload": boundary["payload"],
        "fee_payment": boundary["fee_payment"],
    }
    assert "argument_record" not in submitted
    assert "argument_record_norito_hex" not in submitted


def test_call_contract_posts_exact_sponsor_program_and_rejects_adversarial_sponsor() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=200,
            payload=_contract_call_draft(
                contract_alias="router::is",
                fee_payment=_sponsor_fee_payment(5000),
            ),
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.prepare_contract_call(
        authority=CANONICAL_OWNER,
        contract_alias="router::is",
        entrypoint="ping",
        payload={},
        fee_payment=_sponsor_fee_payment(5000),
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["fee_payment"] == _sponsor_fee_payment(5000)
    assert payload["contract_alias"] == "router::is"

    adversarial = _sponsor_fee_payment(5000)
    adversarial["value"]["program_id"]["sponsor"] = "bad sponsor"
    with pytest.raises(ValueError, match="prepare_contract_call.fee_payment.*sponsor"):
        client.prepare_contract_call(
            authority=CANONICAL_OWNER,
            contract_alias="router::is",
            entrypoint="ping",
            fee_payment=adversarial,
        )


def test_call_contract_rejects_missing_entrypoint_and_non_positive_gas_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    for entrypoint in ("", "   "):
        with pytest.raises(ValueError, match="prepare_contract_call.entrypoint"):
            client.prepare_contract_call(
                authority=CANONICAL_OWNER,
                contract_alias="router::universal",
                entrypoint=entrypoint,
                fee_payment=_authority_fee_payment(1),
            )
    for gas_limit in (None, 0, -1):
        with pytest.raises(ValueError, match="prepare_contract_call.fee_payment.*gas_limit"):
            client.prepare_contract_call(
                authority=CANONICAL_OWNER,
                contract_alias="router::universal",
                entrypoint="ping",
                fee_payment=_authority_fee_payment(gas_limit),
            )

    assert session.calls == []


def test_call_contract_response_requires_operation_receipt() -> None:
    payload = {
        "ok": True,
        "submitted": True,
        "dataspace": "universal",
        "code_hash_hex": "22" * 32,
        "abi_hash_hex": "33" * 32,
        "creation_time_ms": 42,
        "entrypoint": "ping",
    }

    with pytest.raises(RuntimeError, match="operation_receipt response must be a JSON object"):
        ToriiClient._parse_contract_call_response(payload, context="contract call response")


def test_propose_multisig_posts_native_norito_instruction_payloads() -> None:
    session = RecordingSession()
    instruction = b"\x01\x02\x03\x04"
    proposal_id = "aa" * 32
    draft = _app_api_transaction_draft()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "resolved_multisig_account_id": CANONICAL_OWNER,
                "submitted": False,
                "proposal_id": proposal_id,
                "instructions_hash": proposal_id,
                "tx_hash_hex": None,
                "executed_tx_hash_hex": None,
                "creation_time_ms": 123,
                "transaction_payload_b64": draft["transaction_payload_b64"],
                "signing_message_b64": draft["signing_message_b64"],
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)
    result = client.propose_multisig(
        multisig_account_alias="cbdc@banka",
        signer_account_id=CANONICAL_OWNER,
        instructions=[instruction],
        creation_time_ms=123,
        fee_payment=_sponsor_fee_payment(),
    )
    assert isinstance(result, MultisigResponse)
    assert result.ok is True
    assert result.resolved_multisig_account_id == CANONICAL_OWNER
    assert result.submitted is False
    assert result.instructions_hash == proposal_id
    assert result.transaction_payload_b64 == draft["transaction_payload_b64"]
    assert result.signing_message_b64 == draft["signing_message_b64"]
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"] == "http://node.test/v1/multisig/propose"
    assert call["headers"]["Content-Type"] == "application/json"
    payload = json.loads(call["data"].decode("utf-8"))
    assert payload == {
        "signer_account_id": CANONICAL_OWNER,
        "instructions": [base64.b64encode(instruction).decode("ascii")],
        "multisig_account_alias": "cbdc@banka",
        "creation_time_ms": 123,
        "fee_payment": _sponsor_fee_payment(),
    }


def test_multisig_instruction_b64_validates_inputs() -> None:
    assert ToriiClient.multisig_instruction_b64(b"\x01\x02") == "AQI="
    assert ToriiClient.multisig_instruction_b64("AQI=") == "AQI="
    with pytest.raises((RuntimeError, ValueError), match="valid base64|exact standard-base64"):
        ToriiClient.multisig_instruction_b64("not base64")
    with pytest.raises(RuntimeError, match="must not be empty"):
        ToriiClient.multisig_instruction_b64(b"")


def test_propose_multisig_rejects_adversarial_request_shapes() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())
    kwargs = {
        "signer_account_id": CANONICAL_OWNER,
        "instructions": [b"\x01"],
        "fee_payment": _authority_fee_payment(),
    }
    with pytest.raises(ValueError, match="exactly one"):
        client.propose_multisig(
            multisig_account_id=CANONICAL_OWNER,
            multisig_account_alias="cbdc@banka",
            **kwargs,
        )
    with pytest.raises(ValueError, match="exactly one"):
        client.propose_multisig(**kwargs)
    with pytest.raises(TypeError, match="sequence"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=b"\x01\x02",
            fee_payment=_authority_fee_payment(),
        )
    with pytest.raises(ValueError, match="must not be empty"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[],
            fee_payment=_authority_fee_payment(),
        )
    with pytest.raises((RuntimeError, ValueError), match="valid base64|exact standard-base64"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
            signature_b64="not base64",
        )
    canonical_signature = _canonical_signature_base64_fixture()
    for signature_b64 in (
        canonical_signature.rstrip("="),
        _noncanonical_standard_base64_pad_bit_alias(canonical_signature),
    ):
        with pytest.raises((RuntimeError, ValueError), match="valid base64|exact standard-base64"):
            client.propose_multisig(
                multisig_account_alias="cbdc@banka",
                signer_account_id=CANONICAL_OWNER,
                instructions=[b"\x01"],
                fee_payment=_authority_fee_payment(),
                signature_b64=signature_b64,
            )
    with pytest.raises(RuntimeError, match="64 hex"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
            public_key_hex="aa",
        )
    with pytest.raises(ValueError, match="non-negative"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
            creation_time_ms=-1,
        )


def test_propose_multisig_rejects_malformed_response_fields() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": False,
                "resolved_multisig_account_id": CANONICAL_OWNER,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises(RuntimeError, match="ok"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
        )

    for resolved_account_id in (
        f"{CANONICAL_OWNER} ",
        "multisig@banka",
        "multisig",
    ):
        session = RecordingSession()
        session.queue(
            StubResponse(
                payload={
                    "ok": True,
                    "resolved_multisig_account_id": resolved_account_id,
                }
            )
        )
        client = ToriiClient("http://node.test", session=session)
        with pytest.raises(ValueError, match="resolved_multisig_account_id"):
            client.propose_multisig(
                multisig_account_alias="cbdc@banka",
                signer_account_id=CANONICAL_OWNER,
                instructions=[b"\x01"],
                fee_payment=_authority_fee_payment(),
            )

    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "resolved_multisig_account_id": CANONICAL_OWNER,
                "submitted": "false",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises(TypeError, match="submitted"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
        )

    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "resolved_multisig_account_id": CANONICAL_OWNER,
                "instructions_hash": "aa",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises(RuntimeError, match="64 hex"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
        )

    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "resolved_multisig_account_id": CANONICAL_OWNER,
                **_app_api_transaction_draft(),
                "signing_message_b64": "not base64",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises((RuntimeError, ValueError), match="valid base64|exact standard-base64"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
        )
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "resolved_multisig_account_id": CANONICAL_OWNER,
                **_app_api_transaction_draft(),
                "signing_message_b64": "",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises((RuntimeError, ValueError), match="empty bytes|non-empty"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
        )

    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "resolved_multisig_account_id": CANONICAL_OWNER,
                "creation_time_ms": -1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises(RuntimeError, match="non-negative"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            fee_payment=_authority_fee_payment(),
        )


def test_call_contract_rejects_ambiguous_selector() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())

    with pytest.raises(ValueError, match="exactly one of contract_address or contract_alias"):
        client.prepare_contract_call(
            authority=CANONICAL_OWNER,
            contract_address="irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
            contract_alias="router::universal",
            entrypoint="ping",
            fee_payment=_authority_fee_payment(1),
        )


def test_call_contract_rejects_padded_selectors_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="prepare_contract_call\\.contract_address must not contain surrounding whitespace"):
        client.prepare_contract_call(
            authority=CANONICAL_OWNER,
            contract_address=" irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
            entrypoint="ping",
            fee_payment=_authority_fee_payment(1),
        )

    with pytest.raises(ValueError, match="prepare_contract_call\\.contract_alias must not contain surrounding whitespace"):
        client.prepare_contract_call(
            authority=CANONICAL_OWNER,
            contract_alias="router::universal ",
            entrypoint="ping",
            fee_payment=_authority_fee_payment(1),
        )

    assert session.calls == []


def test_get_governance_contract_parses_response() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "found": True,
                "contract_address": "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
                "dataspace": "universal",
                "code_hash_hex": "22" * 32,
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.get_governance_contract(
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
        canonical_auth=_governance_auth(),
    )

    assert isinstance(result, GovernanceContractResponse)
    assert result.found is True
    assert result.code_hash_hex == "22" * 32
    assert session.calls[0]["url"] == (
        "http://node.test/v1/gov/contracts/"
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
    )


@pytest.mark.parametrize(
    "selector",
    [
        "",
        ".",
        ".hidden",
        "selector/alias",
        "selector%2Falias",
        "selector alias",
        "selector\nalias",
        "sélector",
        "a" * 129,
    ],
)
def test_governance_selectors_reject_before_transport(selector: str) -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    operations = [
        lambda: client.get_governance_locks(selector, canonical_auth=_governance_auth()),
        lambda: client.get_governance_referendum(selector, canonical_auth=_governance_auth()),
        lambda: client.get_governance_tally(selector, canonical_auth=_governance_auth()),
        lambda: client.submit_plain_ballot(
            authority=CANONICAL_OWNER,
            network_id=GOVERNANCE_NETWORK_ID,
            referendum_id=selector,
            owner=CANONICAL_OWNER,
            amount="1",
            duration_blocks=1,
            direction="Aye",
            canonical_auth=_governance_auth(),
        ),
        lambda: client.submit_zk_ballot_v1(
            authority=CANONICAL_OWNER,
            network_id=GOVERNANCE_NETWORK_ID,
            election_id=selector,
            backend="halo2/ipa",
            envelope_b64="AAAA",
            canonical_auth=_governance_auth(),
        ),
    ]
    for operation in operations:
        with pytest.raises(RuntimeError, match="canonical governance selector V1"):
            operation()
    assert session.calls == []


def test_governance_selector_accepts_exact_boundaries() -> None:
    for selector in ("a", "a" * 128, "A9_selector~with.dots"):
        assert (
            ToriiClient._require_governance_selector_v1(
                selector,
                context="selector",
            )
            == selector
        )


@pytest.mark.parametrize(
    "proposal_id",
    ["a" * 63, "A" * 64, "0x" + "a" * 64, "a" * 63 + "/"],
)
def test_governance_proposal_ids_reject_before_transport(proposal_id: str) -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    for operation in [
        lambda: client.get_governance_proposal(
            proposal_id, canonical_auth=_governance_auth()
        ),
        lambda: client.enact_proposal(
            proposal_id=proposal_id, canonical_auth=_governance_auth()
        ),
        lambda: client.finalize_referendum(
            referendum_id="a" * 64,
            proposal_id=proposal_id,
        ),
        lambda: client.finalize_referendum(
            referendum_id=proposal_id,
            proposal_id="a" * 64,
        ),
    ]:
        with pytest.raises(RuntimeError, match="lowercase 32-byte hex"):
            operation()
    assert session.calls == []


def _governance_locks_payload(amount: Any) -> Dict[str, Any]:
    return {
        "found": True,
        "referendum_id": "ref-1",
        "locks": {
            CANONICAL_OWNER: {
                "owner": CANONICAL_OWNER,
                "amount": amount,
                "slashed": "0.25",
                "expiry_height": 10,
                "direction": 1,
                "duration_blocks": 5,
                "custody": {
                    "escrowed": True,
                    "asset_definition_id": "xor#wonderland",
                    "bond_escrow_account": CANONICAL_OWNER,
                    "slash_receiver_account": CANONICAL_OWNER,
                },
            }
        },
    }


def test_get_governance_locks_returns_typed_lossless_quantity() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(payload=_governance_locks_payload(CANONICAL_LARGE_FRACTION))
    )
    result = ToriiClient(
        "http://node.test",
        session=session,
    ).get_governance_locks("ref-1", canonical_auth=_governance_auth())

    record = result.locks[CANONICAL_OWNER] if result.locks is not None else None
    assert isinstance(record, GovernanceLockRecord)
    assert record.amount == CANONICAL_LARGE_FRACTION
    assert record.slashed == "0.25"
    assert record.custody is not None
    assert isinstance(record.custody, GovernanceLockCustody)
    assert record.custody.escrowed is True
    assert record.custody.asset_definition_id == "xor#wonderland"


def test_get_governance_locks_accepts_explicit_null_custody() -> None:
    payload = _governance_locks_payload("1")
    payload["locks"][CANONICAL_OWNER]["custody"] = None
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    result = ToriiClient("http://node.test", session=session).get_governance_locks(
        "ref-1", canonical_auth=_governance_auth()
    )

    record = result.locks[CANONICAL_OWNER] if result.locks is not None else None
    assert isinstance(record, GovernanceLockRecord)
    assert record.custody is None


def test_get_governance_locks_requires_strict_nullable_custody() -> None:
    missing = _governance_locks_payload("1")
    del missing["locks"][CANONICAL_OWNER]["custody"]
    session = RecordingSession()
    session.queue(StubResponse(payload=missing))
    with pytest.raises(RuntimeError, match="custody"):
        ToriiClient("http://node.test", session=session).get_governance_locks(
            "ref-1", canonical_auth=_governance_auth()
        )

    extra = _governance_locks_payload("1")
    extra["locks"][CANONICAL_OWNER]["custody"]["legacy"] = True
    session = RecordingSession()
    session.queue(StubResponse(payload=extra))
    with pytest.raises(RuntimeError, match="exactly"):
        ToriiClient("http://node.test", session=session).get_governance_locks(
            "ref-1", canonical_auth=_governance_auth()
        )

    incomplete = _governance_locks_payload("1")
    del incomplete["locks"][CANONICAL_OWNER]["custody"]["bond_escrow_account"]
    session = RecordingSession()
    session.queue(StubResponse(payload=incomplete))
    with pytest.raises(RuntimeError, match="exactly"):
        ToriiClient("http://node.test", session=session).get_governance_locks(
            "ref-1", canonical_auth=_governance_auth()
        )

    wrong = _governance_locks_payload("1")
    wrong["locks"][CANONICAL_OWNER]["custody"]["escrowed"] = 1
    session = RecordingSession()
    session.queue(StubResponse(payload=wrong))
    with pytest.raises(RuntimeError, match="escrowed"):
        ToriiClient("http://node.test", session=session).get_governance_locks(
            "ref-1", canonical_auth=_governance_auth()
        )

    padded = _governance_locks_payload("1")
    padded["locks"][CANONICAL_OWNER]["custody"]["asset_definition_id"] = (
        "xor#wonderland "
    )
    session = RecordingSession()
    session.queue(StubResponse(payload=padded))
    with pytest.raises(RuntimeError, match="whitespace"):
        ToriiClient("http://node.test", session=session).get_governance_locks(
            "ref-1", canonical_auth=_governance_auth()
        )


@pytest.mark.parametrize(
    "amount",
    [1, 1.5, "+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", "9" * 155],
)
def test_get_governance_locks_rejects_noncanonical_quantity(amount: Any) -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=_governance_locks_payload(amount)))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="quantity|Quantity"):
        client.get_governance_locks("ref-1", canonical_auth=_governance_auth())


@pytest.mark.parametrize(
    "slashed",
    [1, 1.5, "+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", "9" * 155],
)
def test_get_governance_locks_rejects_noncanonical_slashed_quantity(
    slashed: Any,
) -> None:
    payload = _governance_locks_payload("1")
    payload["locks"][CANONICAL_OWNER]["slashed"] = slashed
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="quantity"):
        client.get_governance_locks("ref-1", canonical_auth=_governance_auth())


@pytest.mark.parametrize(
    "alias",
    ["", "zk", "plain", "ZK", "PLAIN", " Zk", "Plain ", "quadratic"],
)
def test_propose_contract_deploy_rejects_noncanonical_voting_mode(alias: str) -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="exactly 'Zk' or 'Plain'"):
        client.propose_contract_deploy(
            canonical_auth=_governance_auth(),
            contract_alias="router::universal",
            abi_version="1",
            code_hash="22" * 32,
            abi_hash="33" * 32,
            mode=alias,
        )

    assert session.calls == []


def test_list_telemetry_peers_info_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=[
                {
                    "url": "https://peer-1.example",
                    "connected": True,
                    "telemetry_unsupported": False,
                    "config": {
                        "public_key": "ed011122",
                        "queue_capacity": 8,
                        "network_block_gossip_size": 32,
                        "network_block_gossip_period": {"ms": 150},
                        "network_tx_gossip_size": 16,
                        "network_tx_gossip_period": {"ms": 50},
                    },
                    "location": {"lat": 35.0, "lon": 139.7, "country": "JP", "city": "Tokyo"},
                    "connected_peers": ["peer-A", "peer-B"],
                }
            ]
        )
    )
    client = ToriiClient("http://node.test", session=session)
    peers = client.list_telemetry_peers_info()

    assert len(peers) == 1
    peer = peers[0]
    assert peer.url == "https://peer-1.example"
    assert peer.connected is True
    assert peer.telemetry_unsupported is False
    assert peer.config is not None
    assert peer.config.queue_capacity == 8
    assert peer.config.network_block_gossip_period_ms == 150
    assert peer.location is not None
    assert peer.location.country == "JP"
    assert peer.connected_peers == ["peer-A", "peer-B"]
    assert session.calls[0]["headers"] == {"Accept": "application/json"}


def test_list_telemetry_peers_info_rejects_non_list_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"not": "a list"}))
    client = ToriiClient("http://node.test", session=session)

    try:
        client.list_telemetry_peers_info()
    except RuntimeError as exc:
        assert "/v1/telemetry/peers-info response must be a list" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for invalid telemetry response")


def test_list_telemetry_peers_info_rejects_camelcase_config_fields() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=[
                {
                    "url": "https://peer-2.example",
                    "connected": True,
                    "telemetry_unsupported": False,
                    "config": {
                        "publicKey": "ed011122",
                    },
                }
            ]
        )
    )
    client = ToriiClient("http://node.test", session=session)

    try:
        client.list_telemetry_peers_info()
    except RuntimeError as exc:
        assert "missing `public_key`" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for camelCase telemetry config")


def test_get_health_status_returns_plain_text() -> None:
    session = RecordingSession()
    session.queue(StubResponse(text="Healthy"))
    client = ToriiClient("http://node.test", session=session)

    assert client.get_health_status() == "Healthy"
    assert session.calls[0]["url"].endswith("/v1/health")
    assert session.calls[0]["method"] == "GET"


def test_runtime_manifest_rejects_alias_fields() -> None:
    try:
        ToriiClient._normalize_runtime_manifest_payload(
            {
                "name": "upgrade-1",
                "description": "First upgrade",
                "abiVersion": 1,
                "abi_hash": "0" * 64,
                "start_height": 1,
                "end_height": 2,
            },
            context="runtime upgrade manifest",
        )
    except RuntimeError as exc:
        assert "abi_version is required" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for alias manifest fields")


def test_runtime_manifest_rejects_non_v1_abi_version() -> None:
    with pytest.raises(RuntimeError, match="abi_version must be 1"):
        ToriiClient._normalize_runtime_manifest_payload(
            {
                "name": "upgrade-1",
                "description": "First upgrade",
                "abi_version": 2,
                "abi_hash": "0" * 64,
                "start_height": 1,
                "end_height": 2,
            },
            context="runtime upgrade manifest",
        )


def test_runtime_manifest_rejects_non_empty_added_surfaces() -> None:
    with pytest.raises(RuntimeError, match="added_syscalls must be empty"):
        ToriiClient._normalize_runtime_manifest_payload(
            {
                "name": "upgrade-1",
                "description": "First upgrade",
                "abi_version": 1,
                "abi_hash": "0" * 64,
                "added_syscalls": [512],
                "start_height": 1,
                "end_height": 2,
            },
            context="runtime upgrade manifest",
        )


def test_get_node_version_returns_string() -> None:
    session = RecordingSession()
    session.queue(StubResponse(text="2.1.0-dev"))
    client = ToriiClient("http://node.test", session=session)

    assert client.get_node_version() == "2.1.0-dev"
    assert session.calls[0]["url"].endswith("/v1/version")
    assert session.calls[0]["method"] == "GET"


def test_get_time_now_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "now": 1_700_000,
                "offset_ms": -4,
                "confidence_ms": 9,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_time_now()

    assert isinstance(snapshot, NetworkTimeSnapshot)
    assert snapshot.now_ms == 1_700_000
    assert snapshot.offset_ms == -4
    assert snapshot.confidence_ms == 9
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/time/now")
    assert call["headers"]["Accept"] == "application/json"


def test_get_time_status_parses_histogram_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "peers": 3,
                "samples": [
                    {"peer": "peer-1", "last_offset_ms": -2, "last_rtt_ms": 7, "count": 5}
                ],
                "rtt": {
                    "buckets": [
                        {"le": 5, "count": 10},
                        {"count": 2},
                    ],
                    "sum_ms": 42,
                    "count": 12,
                },
                "note": "ok",
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    status = client.get_time_status()

    assert isinstance(status, NetworkTimeStatus)
    assert status.peers == 3
    assert len(status.samples) == 1
    sample = status.samples[0]
    assert sample.peer == "peer-1"
    assert sample.last_offset_ms == -2
    assert sample.last_rtt_ms == 7
    assert sample.count == 5
    assert len(status.rtt_buckets) == 2
    first_bucket, second_bucket = status.rtt_buckets
    assert first_bucket.upper_bound_ms == 5
    assert first_bucket.count == 10
    assert second_bucket.upper_bound_ms is None
    assert second_bucket.count == 2
    assert status.rtt_sum_ms == 42
    assert status.rtt_count == 12
    assert status.note == "ok"
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/time/status")
    assert call["headers"]["Accept"] == "application/json"


def test_get_explorer_account_qr_parses_payload_and_params() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "canonical_id": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
                "literal": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
                "network_prefix": 26,
                "error_correction": "quartile",
                "modules": 33,
                "qr_version": 5,
                "svg": "<svg></svg>",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    qr = client.get_explorer_account_qr("sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6")

    assert qr == ExplorerAccountQr(
        canonical_id="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        literal="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        network_prefix=26,
        error_correction="quartile",
        modules=33,
        qr_version=5,
        svg="<svg></svg>",
    )
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith(f"/v1/explorer/accounts/{quote(CANONICAL_OWNER, safe='')}/qr")
    assert call["params"] == {}
    assert call["headers"]["Accept"] == "application/json"


def test_get_explorer_account_qr_accepts_account_alias_path_literal() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "canonical_id": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
                "literal": "operator@banka.universal",
                "network_prefix": 26,
                "error_correction": "quartile",
                "modules": 33,
                "qr_version": 5,
                "svg": "<svg></svg>",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    qr = client.get_explorer_account_qr("operator@banka.universal")

    assert qr.literal == "operator@banka.universal"
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/explorer/accounts/operator%40banka.universal/qr")
    assert call["params"] == {}


def test_get_explorer_account_qr_normalizes_payload_variants() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "canonicalId": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
                "literal": "sorabobacct",
                "networkPrefix": 27,
                "errorCorrection": "medium",
                "modules": 41,
                "qrVersion": 7,
                "svg": "<svg/>",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    qr = client.get_explorer_account_qr("sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE")

    assert qr == ExplorerAccountQr(
        canonical_id="sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        literal="sorabobacct",
        network_prefix=27,
        error_correction="medium",
        modules=41,
        qr_version=7,
        svg="<svg/>",
    )
    call = session.calls[0]
    assert call["params"] == {}
    assert call["headers"]["Accept"] == "application/json"


def test_get_node_capabilities_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "abi_version": 1,
                "data_model_version": 1,
                "crypto": {
                    "sm": {
                        "enabled": True,
                        "default_hash": "sm3",
                        "allowed_signing": ["sm2"],
                        "sm2_distid_default": "soranet",
                        "openssl_preview": False,
                        "acceleration": {
                            "scalar": True,
                            "neon_sm3": True,
                            "neon_sm4": False,
                            "policy": "scalar",
                        },
                    },
                    "curves": {
                        "registry_version": 2,
                        "allowed_curve_ids": [1, 15],
                        "allowed_curve_bitmap": [32770],
                    },
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    capabilities = client.get_node_capabilities(canonical_auth=_governance_auth())

    assert capabilities.abi_version == 1
    assert capabilities.data_model_version == 1
    assert capabilities.crypto.sm.allowed_signing == ["sm2"]
    assert capabilities.crypto.sm.acceleration.neon_sm3 is True
    assert capabilities.crypto.curves.registry_version == 2
    assert capabilities.crypto.curves.allowed_curve_bitmap == [32770]


def test_contract_helpers_against_mock_server() -> None:
    server = ToriiMockServer().start()
    contract_address = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
    try:
        response = requests.post(
            f"{server.base_url.rstrip('/')}/__mock__/gov/config",
            json={
                "gov_contracts": {
                    contract_address: {
                        "found": True,
                        "dataspace": "universal",
                        "code_hash_hex": "22" * 32,
                    }
                },
                "contract_call_response": _contract_call_draft(
                    contract_alias=None,
                    contract_address=contract_address,
                    fee_payment=_authority_fee_payment(5000),
                ),
            },
            timeout=5.0,
        )
        response.raise_for_status()

        client = ToriiClient(server.base_url)
        call = client.prepare_contract_call(
            authority=CANONICAL_OWNER,
            contract_address=contract_address,
            entrypoint="ping",
            payload={"value": 1},
            fee_payment=_authority_fee_payment(5000),
        )
        governed = client.get_governance_contract(
            contract_address, canonical_auth=_governance_auth()
        )

        assert call.contract_address == contract_address
        assert governed.contract_address == contract_address
        assert governed.code_hash_hex == "22" * 32
    finally:
        server.stop()


def test_mock_server_advertises_current_data_model_version() -> None:
    server = ToriiMockServer().start()
    try:
        response = requests.get(
            f"{server.base_url.rstrip('/')}/v1/node/capabilities",
            timeout=5.0,
        )
        response.raise_for_status()

        assert response.json()["data_model_version"] == 4
    finally:
        server.stop()


def test_mock_pipeline_status_emits_only_the_current_exact_public_shape() -> None:
    queued = mock_module._MockState._make_status_payload(  # noqa: SLF001
        "11" * 32,
        {
            "kind": "Queued",
            "block_height": None,
            "rejection_reason": {"message": "retired"},
            "summary": "retired",
            "diagnostics": [{"message": "retired"}],
            "scope": "local",
            "resolved_from": "queue",
        },
    )
    assert queued == {
        "hash": "11" * 32,
        "status": {"kind": "Queued"},
        "scope": "local",
        "resolved_from": "queue",
    }

    applied = mock_module._MockState._make_status_payload(  # noqa: SLF001
        "13" * 32,
        {"kind": "Applied", "block_height": 42},
    )
    assert applied == {
        "hash": "13" * 32,
        "status": {"kind": "Applied", "block_height": 42},
        "scope": "global",
        "resolved_from": "state",
    }


def test_mock_server_seeds_sumeragi_status_snapshot() -> None:
    server = ToriiMockServer().start()
    try:
        response = requests.get(f"{server.base_url.rstrip('/')}/v1/sumeragi/status", timeout=5.0)
        response.raise_for_status()

        payload = response.json()

        assert payload["protocol_version"] == 4
        assert payload["restart_required"] is False
        assert payload["leader"] == 1
        assert payload["height_context"]["validator_count"] == 4
        assert payload["liveness"]["generation"] == 2
        assert "lane_settlement_commitments" not in payload

        diagnostics = requests.get(
            f"{server.base_url.rstrip('/')}/v1/sumeragi/diagnostics", timeout=5.0
        )
        diagnostics.raise_for_status()
        diagnostics_payload = diagnostics.json()
        assert diagnostics_payload["tx_queue_capacity"] == 32
        assert diagnostics_payload["committed_lane_blocks"] == []
    finally:
        server.stop()


def test_mock_server_allows_sumeragi_fixture_override() -> None:
    server = ToriiMockServer().start()
    try:
        base_url = server.base_url.rstrip("/")
        fixtures = {
            "status": {"protocol_version": 4, "height": 42},
            "leader": {"leader_index": 2},
            "telemetry": {"availability": {"total_votes_ingested": 7}},
        }
        response = requests.post(
            f"{base_url}/__mock__/sumeragi/config",
            json=fixtures,
            timeout=5.0,
        )
        response.raise_for_status()

        for endpoint, expected in fixtures.items():
            response = requests.get(f"{base_url}/v1/sumeragi/{endpoint}", timeout=5.0)
            response.raise_for_status()
            assert response.json() == expected

        response = requests.post(
            f"{base_url}/__mock__/sumeragi/config",
            json={"status": {"height": 99}, "leader": []},
            timeout=5.0,
        )
        assert response.status_code == 400
        response = requests.get(f"{base_url}/v1/sumeragi/status", timeout=5.0)
        response.raise_for_status()
        assert response.json() == fixtures["status"]
    finally:
        server.stop()


def test_get_sumeragi_status_parses_authoritative_v2_snapshot() -> None:
    payload = _sumeragi_v2_status_payload()
    status = _get_sumeragi_status(payload)

    assert type(status) is SumeragiV2Status
    assert SumeragiV2Status is not SumeragiDiagnosticsStatus
    assert status.protocol_version == 4
    assert status.restart_required is False
    assert status.height == 10
    assert status.phase == "prepare"
    assert status.height_context.mode == "permissioned"
    assert status.height_context.min_signers == 3
    assert status.last_commit_qc is not None
    assert status.last_commit_qc.certificate.round.height == 9
    assert status.last_commit_qc.certificate.proposal_round.view == 1
    assert (
        status.last_commit_qc.certificate.execution_commitment
        .native_amx_application_manifest_root
        == _NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT
    )
    assert status.last_commit_qc.signed_power == 3
    assert status.liveness.generation == 2
    assert status.liveness.work.validation == "complete"
    assert status.liveness.prepare_quorums[0].signer_count == 2
    assert (
        status.liveness.prepare_quorums[0].proposal_round
        == status.liveness.prepare_quorums[0].round
    )
    assert (
        status.liveness.outbound_intents[0].proposal_round
        == status.liveness.outbound_intents[0].round
    )
    assert status.liveness.queues[0].queue == "network_ingress"
    assert status.liveness.last_progress is not None
    assert status.liveness.last_progress.transition == "prepare_vote_admitted"
    assert status.liveness.blocker == "prepare_quorum_missing"
    assert not hasattr(status, "lane_settlement_commitments")
    assert not hasattr(status, "operator")


def test_get_sumeragi_status_accepts_nonempty_native_manifest() -> None:
    payload = _sumeragi_v2_status_payload()
    commitment = payload["last_commit_qc"]["certificate"]["execution_commitment"]
    commitment["native_amx_application_manifest_root"] = _canonical_hash(0x38)
    commitment["native_amx_application_manifest_count"] = 1

    status = _get_sumeragi_status(payload)

    assert status.last_commit_qc is not None
    assert (
        status.last_commit_qc.certificate.execution_commitment
        .native_amx_application_manifest_root
        == _canonical_hash(0x38)
    )
    assert (
        status.last_commit_qc.certificate.execution_commitment
        .native_amx_application_manifest_count
        == 1
    )


@pytest.mark.parametrize(
    ("mutate", "error"),
    [
        (
            lambda commitment: commitment.update(
                native_amx_application_manifest_version=2
            ),
            "native_amx_application_manifest_version must equal 1",
        ),
        (
            lambda commitment: commitment.update(
                native_amx_application_manifest_count=1025
            ),
            "native_amx_application_manifest_count",
        ),
        (
            lambda commitment: commitment.update(
                native_amx_application_manifest_root=_canonical_hash(0x38)
            ),
            "must be zero exactly for the canonical empty root",
        ),
        (
            lambda commitment: commitment.update(
                native_amx_application_manifest_count=1
            ),
            "must be zero exactly for the canonical empty root",
        ),
    ],
)
def test_get_sumeragi_status_rejects_invalid_native_manifest(
    mutate, error: str
) -> None:
    payload = _sumeragi_v2_status_payload()
    mutate(payload["last_commit_qc"]["certificate"]["execution_commitment"])

    with pytest.raises(RuntimeError, match=error):
        _get_sumeragi_status(payload)


def test_get_sumeragi_status_requires_exact_merge_carrier_projection() -> None:
    payload = _sumeragi_v2_status_payload()
    commitment = payload["last_commit_qc"]["certificate"]["execution_commitment"]
    status = _get_sumeragi_status(payload)
    assert status.last_commit_qc is not None
    assert status.last_commit_qc.certificate.execution_commitment.merge_carrier is None

    commitment["merge_carrier"] = {
        "version": 1,
        "entry_hash": _canonical_hash(0x39),
    }
    status = _get_sumeragi_status(payload)
    assert status.last_commit_qc is not None
    carrier = status.last_commit_qc.certificate.execution_commitment.merge_carrier
    assert carrier is not None
    assert carrier.version == 1
    assert carrier.entry_hash == _canonical_hash(0x39)

    invalid_carriers = [
        "missing",
        "malformed",
        "wrong_version",
        "missing_version",
        "missing_entry_hash",
        "bad_hash",
        "unknown_field",
    ]
    for case in invalid_carriers:
        candidate = _sumeragi_v2_status_payload()
        candidate_commitment = candidate["last_commit_qc"]["certificate"][
            "execution_commitment"
        ]
        if case == "missing":
            del candidate_commitment["merge_carrier"]
        elif case == "malformed":
            candidate_commitment["merge_carrier"] = "carrier"
        elif case == "wrong_version":
            candidate_commitment["merge_carrier"] = {
                "version": 2,
                "entry_hash": _canonical_hash(0x39),
            }
        elif case == "missing_version":
            candidate_commitment["merge_carrier"] = {
                "entry_hash": _canonical_hash(0x39),
            }
        elif case == "missing_entry_hash":
            candidate_commitment["merge_carrier"] = {"version": 1}
        elif case == "bad_hash":
            candidate_commitment["merge_carrier"] = {
                "version": 1,
                "entry_hash": "not-a-hash",
            }
        else:
            candidate_commitment["merge_carrier"] = {
                "version": 1,
                "entry_hash": _canonical_hash(0x39),
                "future": True,
            }
        with pytest.raises(RuntimeError):
            _get_sumeragi_status(candidate)


@pytest.mark.parametrize("invalid", [None, True, 0, -1, 1 << 64, "123"])
def test_get_sumeragi_status_requires_exact_executed_wire_len(invalid: Any) -> None:
    payload = _sumeragi_v2_status_payload()
    commitment = payload["last_commit_qc"]["certificate"]["execution_commitment"]
    status = _get_sumeragi_status(payload)
    assert status.last_commit_qc is not None
    assert (
        status.last_commit_qc.certificate.execution_commitment.executed_block_wire_len
        == 123
    )

    commitment["executed_block_wire_len"] = invalid
    with pytest.raises(RuntimeError, match="executed_block_wire_len"):
        _get_sumeragi_status(payload)

    del commitment["executed_block_wire_len"]
    with pytest.raises(RuntimeError, match="executed_block_wire_len"):
        _get_sumeragi_status(payload)


def test_get_sumeragi_status_preserves_exact_proposal_rounds() -> None:
    payload = _sumeragi_v2_status_payload()
    commit_quorum = copy.deepcopy(payload["liveness"]["prepare_quorums"][0])
    commit_quorum["round"]["view"] = 2
    commit_quorum["proposal_round"]["view"] = 2
    payload["liveness"]["commit_quorums"] = [commit_quorum]

    commit_intent = copy.deepcopy(payload["liveness"]["outbound_intents"][0])
    commit_intent["kind"]["kind"] = "commit_vote"
    commit_intent["round"]["view"] = 2
    commit_intent["proposal_round"]["view"] = 2
    commit_intent["execution_commitment"] = copy.deepcopy(
        commit_quorum["execution_commitment"]
    )
    payload["liveness"]["outbound_intents"] = [commit_intent]
    payload["last_commit_qc"]["certificate"]["round"]["view"] = 2
    payload["last_commit_qc"]["certificate"]["proposal_round"]["view"] = 2

    status = _get_sumeragi_status(payload)

    assert status.liveness.commit_quorums[0].round.view == 2
    assert status.liveness.commit_quorums[0].proposal_round.view == 2
    assert status.liveness.outbound_intents[0].round.view == 2
    assert status.liveness.outbound_intents[0].proposal_round is not None
    assert status.liveness.outbound_intents[0].proposal_round.view == 2
    assert status.last_commit_qc is not None
    assert status.last_commit_qc.certificate.proposal_round.view == 2

    later_commit_payload = _sumeragi_v2_status_payload()
    later_commit_intent = later_commit_payload["liveness"]["outbound_intents"][0]
    later_commit_intent["kind"]["kind"] = "commit_qc"
    later_commit_intent["round"]["view"] = 3
    later_commit_intent["proposal_round"]["view"] = 3
    later_commit_intent["execution_commitment"] = copy.deepcopy(
        later_commit_payload["last_commit_qc"]["certificate"][
            "execution_commitment"
        ]
    )
    later_commit_status = _get_sumeragi_status(later_commit_payload)
    assert later_commit_status.liveness.outbound_intents[0].round.view == 3
    assert (
        later_commit_status.liveness.outbound_intents[0].proposal_round.view == 3
    )

    timeout_payload = _sumeragi_v2_status_payload()
    timeout_intent = timeout_payload["liveness"]["outbound_intents"][0]
    timeout_intent["kind"]["kind"] = "timeout_certificate"
    del timeout_intent["proposal_round"]
    del timeout_intent["subject"]
    timeout_status = _get_sumeragi_status(timeout_payload)
    assert timeout_status.liveness.outbound_intents[0].proposal_round is None


def test_get_sumeragi_status_enforces_vote_quorum_proposal_geometry() -> None:
    missing_origin = _sumeragi_v2_status_payload()
    del missing_origin["liveness"]["prepare_quorums"][0]["proposal_round"]
    with pytest.raises(RuntimeError, match="proposal_round"):
        _get_sumeragi_status(missing_origin)

    prepare_reproposal = _sumeragi_v2_status_payload()
    prepare_reproposal["liveness"]["prepare_quorums"][0]["proposal_round"][
        "view"
    ] = 0
    with pytest.raises(RuntimeError, match="proposal_round must equal round"):
        _get_sumeragi_status(prepare_reproposal)

    future_commit_origin = _sumeragi_v2_status_payload()
    commit_quorum = copy.deepcopy(
        future_commit_origin["liveness"]["prepare_quorums"][0]
    )
    commit_quorum["proposal_round"]["view"] = 2
    future_commit_origin["liveness"]["commit_quorums"] = [commit_quorum]
    with pytest.raises(RuntimeError, match="proposal_round must equal round"):
        _get_sumeragi_status(future_commit_origin)

    foreign_origin = _sumeragi_v2_status_payload()
    foreign_origin["liveness"]["prepare_quorums"][0]["proposal_round"][
        "context_id"
    ] = [_canonical_hash(0x55)]
    with pytest.raises(RuntimeError, match="proposal_round.*active height context"):
        _get_sumeragi_status(foreign_origin)

    wrong_height = _sumeragi_v2_status_payload()
    wrong_height["liveness"]["prepare_quorums"][0]["proposal_round"]["height"] = 9
    with pytest.raises(RuntimeError, match="proposal_round.*active height context"):
        _get_sumeragi_status(wrong_height)


def test_get_sumeragi_status_enforces_outbound_intent_proposal_geometry() -> None:
    missing_origin = _sumeragi_v2_status_payload()
    del missing_origin["liveness"]["outbound_intents"][0]["proposal_round"]
    with pytest.raises(RuntimeError, match="inconsistent proposal_round"):
        _get_sumeragi_status(missing_origin)

    timeout_with_origin = _sumeragi_v2_status_payload()
    timeout_intent = timeout_with_origin["liveness"]["outbound_intents"][0]
    timeout_intent["kind"]["kind"] = "timeout_vote"
    timeout_intent["subject"] = None
    with pytest.raises(RuntimeError, match="inconsistent proposal_round"):
        _get_sumeragi_status(timeout_with_origin)

    prepare_reproposal = _sumeragi_v2_status_payload()
    prepare_intent = prepare_reproposal["liveness"]["outbound_intents"][0]
    prepare_intent["kind"]["kind"] = "prepare_vote"
    prepare_intent["execution_commitment"] = copy.deepcopy(
        prepare_reproposal["last_commit_qc"]["certificate"][
            "execution_commitment"
        ]
    )
    prepare_intent["round"]["view"] = 2
    with pytest.raises(RuntimeError, match="proposal_round must equal round"):
        _get_sumeragi_status(prepare_reproposal)

    future_commit_origin = _sumeragi_v2_status_payload()
    commit_intent = future_commit_origin["liveness"]["outbound_intents"][0]
    commit_intent["kind"]["kind"] = "commit_vote"
    commit_intent["execution_commitment"] = copy.deepcopy(
        future_commit_origin["last_commit_qc"]["certificate"][
            "execution_commitment"
        ]
    )
    commit_intent["proposal_round"]["view"] = 2
    with pytest.raises(RuntimeError, match="proposal_round must equal round"):
        _get_sumeragi_status(future_commit_origin)

    foreign_origin = _sumeragi_v2_status_payload()
    foreign_origin["liveness"]["outbound_intents"][0]["proposal_round"][
        "context_id"
    ] = [_canonical_hash(0x55)]
    with pytest.raises(RuntimeError, match="proposal_round.*active height context"):
        _get_sumeragi_status(foreign_origin)


def test_get_sumeragi_status_accepts_local_control_pending_liveness_blocker() -> None:
    payload = _sumeragi_v2_status_payload()
    payload["liveness"]["blocker"] = {
        "blocker": "local_control_pending",
        "details": None,
    }

    status = _get_sumeragi_status(payload)

    assert status.liveness.blocker == "local_control_pending"


def test_get_sumeragi_status_accepts_successor_activation_pending_liveness_blocker() -> None:
    payload = _sumeragi_v2_status_payload()
    payload["liveness"]["blocker"] = {
        "blocker": "successor_activation_pending",
        "details": None,
    }

    status = _get_sumeragi_status(payload)

    assert status.liveness.blocker == "successor_activation_pending"


def test_get_sumeragi_status_accepts_unsafe_proposal_ignore_reason() -> None:
    payload = _sumeragi_v2_status_payload()
    payload["liveness"]["ignore_counts"] = [
        {
            "reason": {"reason": "unsafe_proposal", "details": None},
            "count": 3,
        }
    ]

    status = _get_sumeragi_status(payload)

    assert [(entry.reason, entry.count) for entry in status.liveness.ignore_counts] == [
        ("unsafe_proposal", 3)
    ]


def test_get_sumeragi_status_accepts_all_twelve_ignore_reasons_at_the_bound() -> None:
    reasons = [
        "wrong_height",
        "wrong_view",
        "stale_generation",
        "busy",
        "duplicate",
        "no_matching_work",
        "observer",
        "view_closed",
        "already_decided",
        "recovery_pending",
        "irrelevant_view",
        "unsafe_proposal",
    ]
    payload = _sumeragi_v2_status_payload()
    payload["liveness"]["ignore_counts"] = [
        {
            "reason": {"reason": reason, "details": None},
            "count": index,
        }
        for index, reason in enumerate(reasons, start=1)
    ]

    status = _get_sumeragi_status(payload)

    assert [entry.reason for entry in status.liveness.ignore_counts] == reasons

    payload["liveness"]["ignore_counts"].append(
        copy.deepcopy(payload["liveness"]["ignore_counts"][-1])
    )
    with pytest.raises(RuntimeError, match="ignore_counts exceeds its protocol item bound"):
        _get_sumeragi_status(payload)


def test_get_sumeragi_status_accepts_all_ten_liveness_queue_kinds() -> None:
    payload = _sumeragi_v2_status_payload()
    queue_template = payload["liveness"]["queues"][0]
    queue_kinds = [
        "ingress",
        "deferred_normal",
        "deferred_progress",
        "deferred_completion",
        "runtime_normal",
        "runtime_progress",
        "runtime_completion",
        "effect_completion",
        "network_ingress",
        "effect_dispatch",
    ]
    payload["liveness"]["queues"] = [
        {
            **copy.deepcopy(queue_template),
            "queue": {"queue": queue, "details": None},
        }
        for queue in queue_kinds
    ]

    status = _get_sumeragi_status(payload)
    assert [queue.queue for queue in status.liveness.queues] == queue_kinds

    payload["liveness"]["queues"].append(copy.deepcopy(queue_template))
    with pytest.raises(RuntimeError, match="queues exceeds its protocol item bound"):
        _get_sumeragi_status(payload)


def test_get_sumeragi_status_rejects_operational_diagnostics_fields() -> None:
    payload = _sumeragi_v2_status_payload()
    payload["lane_settlement_commitments"] = []
    with pytest.raises(
        RuntimeError, match="unknown field lane_settlement_commitments"
    ):
        _get_sumeragi_status(payload)


def test_sumeragi_endpoint_methods_reject_swapped_payload_contracts() -> None:
    status_session = RecordingSession()
    status_session.queue(StubResponse(payload=_sumeragi_diagnostics_payload()))
    status_client = ToriiClient(
        "http://node.test",
        session=status_session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(RuntimeError, match="sumeragi status contains unknown field"):
        status_client.get_sumeragi_status()
    assert status_session.calls[0]["url"].endswith("/v1/sumeragi/status")

    diagnostics_session = RecordingSession()
    diagnostics_session.queue(StubResponse(payload=_sumeragi_v2_status_payload()))
    diagnostics_client = ToriiClient(
        "http://node.test",
        session=diagnostics_session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(
        RuntimeError, match="sumeragi diagnostics contains unknown field"
    ):
        diagnostics_client.get_sumeragi_diagnostics()
    assert diagnostics_session.calls[0]["url"].endswith(
        "/v1/sumeragi/diagnostics"
    )

    for endpoint, response, error_type, message in sumeragi_exact_json_response_cases():
        session = RecordingSession()
        session.queue(response)
        client = ToriiClient(
            "http://node.test",
            session=session,
            operator_signing_context=_operator_context(),
        )
        with pytest.raises(error_type, match=message):
            getattr(client, f"get_sumeragi_{endpoint}")()
        assert response.was_closed is True, endpoint
        assert session.calls[0]["url"].endswith(f"/v1/sumeragi/{endpoint}")
        assert session.calls[0]["headers"]["Accept"] == "application/json"
        for header in (
            "X-Iroha-Operator-Public-Key",
            "X-Iroha-Operator-Timestamp-Ms",
            "X-Iroha-Operator-Nonce",
            "X-Iroha-Operator-Signature",
        ):
            assert session.calls[0]["headers"][header]
        assert session.calls[0]["allow_redirects"] is False
        assert session.calls[0]["data"] is None
        assert session.calls[0]["stream"] is True


def test_get_sumeragi_diagnostics_parses_exact_nested_fee_and_native_amx_receipts() -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    settlement["nexus_fee_receipts"] = [_nexus_fee_receipt_payload()]
    settlement["native_amx_receipts"] = _native_amx_receipt_group()
    payload["lane_settlement_commitments"] = [settlement]

    parsed = _get_sumeragi_diagnostics(payload).lane_settlement_commitments[0]

    assert (
        parsed["nexus_fee_receipts"][0]["fee_amount"]
        == CANONICAL_LARGE_FRACTION
    )
    assert parsed["nexus_fee_receipts"][0]["schedule"]["per_byte_fee"] == "0.5"
    native = parsed["native_amx_receipts"][0]
    assert native["version"] == 2
    assert native["legs"][0]["prepare_qc"]["body"]["phase"] == {
        "phase": "prepare",
        "detail": None,
    }
    assert len(native["legs"][0]["commit_qc"]["bls_aggregate_signature"]) == 96
    leg = native["legs"][0]
    assert (
        leg["participant_proposal"]["proposal_hash"]
        == leg["prepare_qc"]["body"]["participant_proposal_hash"]
    )
    assert leg["participant_proposal"]["payload_block_hint"] is None
    assert (
        leg["participant_settlement_hash"]
        == leg["commit_qc"]["body"]["participant_settlement_commitment"]
    )
    assert leg["participant_settlement"]["block_height"] == 8
    assert len(leg["participant_settlement"]["receipts"]) == 2
    assert leg["prepare_qc"]["body"]["source_id"] == "AB" * 32
    assert leg["prepare_qc"]["body"]["tx_entrypoint_hash"] == _canonical_hash(0x61)


def test_get_sumeragi_diagnostics_accepts_first_native_amx_participant_block() -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native_group = _native_amx_receipt_group()
    for native in native_group:
        leg = native["legs"][0]
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["body"]["participant_previous_block_height"] = 0
            qc["body"]["participant_previous_block_descriptor_hash"] = None
            qc["body"]["participant_lane_block_height"] = 1
        descriptor = leg["participant_proposal"]["descriptor"]
        descriptor["previous_lane_block_height"] = 0
        del descriptor["previous_lane_block_descriptor_hash"]
        descriptor["lane_block_height"] = 1
        leg["participant_settlement"]["block_height"] = 1
        _seal_native_amx_receipt_payload(native)
    settlement["native_amx_receipts"] = native_group
    payload["lane_settlement_commitments"] = [settlement]

    parsed_leg = _get_sumeragi_diagnostics(payload).lane_settlement_commitments[0][
        "native_amx_receipts"
    ][0]["legs"][0]

    assert parsed_leg["prepare_qc"]["body"]["participant_previous_block_descriptor_hash"] is None
    assert (
        "previous_lane_block_descriptor_hash"
        not in parsed_leg["participant_proposal"]["descriptor"]
    )


def test_get_sumeragi_diagnostics_accepts_mixed_role_proposal_without_current_entrypoint() -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native_group = _native_amx_receipt_group()
    native = native_group[0]
    leg = native["legs"][0]
    descriptor = leg["participant_proposal"]["descriptor"]
    descriptor["accepted_candidate_indices"] = [1]
    descriptor["accepted_transaction_hashes"] = [_canonical_hash(0x74)]
    _seal_native_amx_receipt_payload(native)
    settlement["native_amx_receipts"] = native_group
    payload["lane_settlement_commitments"] = [settlement]

    parsed_leg = _get_sumeragi_diagnostics(payload).lane_settlement_commitments[0][
        "native_amx_receipts"
    ][0]["legs"][0]

    assert parsed_leg["requires_mixed_role_anchor_validation"] is True


def test_get_sumeragi_diagnostics_rejects_native_amx_group_shape_drift() -> None:
    missing_outer_source = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native_group = _native_amx_receipt_group()
    native_group.pop()
    settlement["native_amx_receipts"] = native_group
    missing_outer_source["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="exact ordered source group"):
        _get_sumeragi_diagnostics(missing_outer_source)

    unordered_outer_sources = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native_group = _native_amx_receipt_group()
    native_group.reverse()
    settlement["native_amx_receipts"] = native_group
    unordered_outer_sources["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="strictly ordered"):
        _get_sumeragi_diagnostics(unordered_outer_sources)

    unordered_participant_sources = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native_group = _native_amx_receipt_group()
    native_group[0]["legs"][0]["participant_settlement"]["receipts"].reverse()
    settlement["native_amx_receipts"] = native_group
    unordered_participant_sources["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="canonical commitment"):
        _get_sumeragi_diagnostics(unordered_participant_sources)

    oversized_outer_group = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    settlement["native_amx_receipts"] = [{}] * 4097
    oversized_outer_group["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="native_amx_receipts exceeds"):
        _get_sumeragi_diagnostics(oversized_outer_group)


def test_get_sumeragi_diagnostics_rejects_same_route_native_identity_drift() -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native_group = _native_amx_receipt_group()
    leg = native_group[0]["legs"][0]
    leg["lane_id"] = 2
    leg["dataspace_id"] = 7
    for qc in (leg["prepare_qc"], leg["commit_qc"]):
        qc["body"]["participant_lane_id"] = 2
        qc["body"]["participant_dataspace_id"] = 7
        qc["body"]["participant_lane_incarnation"] = _canonical_hash(0x51)
    descriptor = leg["participant_proposal"]["descriptor"]
    descriptor["lane_id"] = 2
    descriptor["dataspace_id"] = 7
    descriptor["lane_incarnation"] = _canonical_hash(0x51)
    leg["participant_settlement"]["lane_id"] = 2
    leg["participant_settlement"]["dataspace_id"] = 7
    leg["participant_settlement"]["lane_incarnation"] = _canonical_hash(0x51)
    _seal_native_amx_receipt_payload(native_group[0])
    settlement["native_amx_receipts"] = native_group
    payload["lane_settlement_commitments"] = [settlement]

    with pytest.raises(RuntimeError, match="mismatched signed identities"):
        _get_sumeragi_diagnostics(payload)


def test_get_sumeragi_diagnostics_keeps_global_and_coordinator_views_independent() -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native_group = _native_amx_receipt_group()
    for native in native_group:
        native["lane_block_view"] = 9
        for qc in (native["legs"][0]["prepare_qc"], native["legs"][0]["commit_qc"]):
            assert qc["body"]["round"]["view"] == 2
            qc["body"]["coordinator_lane_block_view"] = 9
    settlement["native_amx_receipts"] = native_group
    payload["lane_settlement_commitments"] = [settlement]

    parsed = _get_sumeragi_diagnostics(payload)

    body = parsed.lane_settlement_commitments[0]["native_amx_receipts"][0][
        "legs"
    ][0]["prepare_qc"]["body"]
    assert body["round"]["view"] == 2
    assert body["coordinator_lane_block_view"] == 9


def test_get_sumeragi_diagnostics_rejects_unordered_native_qc_validator_set() -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    validators = native["legs"][0]["prepare_qc"]["validator_set"]
    validators[0], validators[1] = validators[1], validators[0]
    settlement["native_amx_receipts"] = [native]
    payload["lane_settlement_commitments"] = [settlement]

    with pytest.raises(
        RuntimeError, match="strictly ordered by canonical validator id"
    ):
        _get_sumeragi_diagnostics(payload)


def test_get_sumeragi_diagnostics_parses_ordered_native_application_evidence() -> None:
    payload = _sumeragi_diagnostics_payload()
    payload["native_amx_participant_applications"] = [
        _native_amx_participant_application_payload()
    ]

    diagnostics = _get_sumeragi_diagnostics(payload)
    applications = diagnostics.native_amx_participant_applications

    assert type(diagnostics) is SumeragiDiagnosticsStatus
    assert applications[0].participant_height == 8
    assert applications[0].state == "durably_applied"


@pytest.mark.parametrize(
    ("state", "has_application_block"),
    [
        pytest.param("certified_pending_carrier", False, id="certified"),
        pytest.param("committed_evidence_pending", True, id="committed"),
        pytest.param("durably_applied", True, id="durably-applied"),
        pytest.param("conflict", False, id="conflict"),
    ],
)
def test_get_sumeragi_diagnostics_accepts_native_application_state_geometry(
    state: str, has_application_block: bool
) -> None:
    payload = _sumeragi_diagnostics_payload()
    application = _native_amx_participant_application_payload(state=state)
    if not has_application_block:
        _set_native_amx_application_without_block(application, state)
    payload["native_amx_participant_applications"] = [application]

    parsed = _get_sumeragi_diagnostics(
        payload
    ).native_amx_participant_applications[0]

    assert parsed.state == state
    assert (parsed.application_block_height is not None) is has_application_block
    assert (parsed.application_block_hash is not None) is has_application_block


@pytest.mark.parametrize(
    ("mutate", "error"),
    [
        pytest.param(
            lambda row: row.update(state="applied"),
            "state has an unknown variant",
            id="unknown-state",
        ),
        pytest.param(
            lambda row: row.update(state=" conflict "),
            "state has an unknown variant",
            id="padded-state",
        ),
        pytest.param(
            lambda row: row.update(
                state={"state": "durably_applied", "details": None}
            ),
            "state has an unknown variant",
            id="status-style-tagged-state",
        ),
        pytest.param(
            lambda row: row.pop("state"),
            "missing required field state",
            id="missing-state",
        ),
        pytest.param(
            lambda row: row.update(legacy_phase="commit"),
            "unknown field legacy_phase",
            id="unknown-field",
        ),
        pytest.param(
            lambda row: row.pop("application_block_hash"),
            "application block height and hash must appear together",
            id="unpaired-application-block",
        ),
        pytest.param(
            lambda row: row.update(state="certified_pending_carrier"),
            "state and application block identity disagree",
            id="certified-with-application-block",
        ),
        pytest.param(
            lambda row: row.update(state="conflict"),
            "state and application block identity disagree",
            id="conflict-with-application-block",
        ),
        pytest.param(
            lambda row: _set_native_amx_application_without_block(
                row, "committed_evidence_pending"
            ),
            "state and application block identity disagree",
            id="committed-without-application-block",
        ),
        pytest.param(
            lambda row: _set_native_amx_application_without_block(
                row, "durably_applied"
            ),
            "state and application block identity disagree",
            id="durably-applied-without-application-block",
        ),
        pytest.param(
            lambda row: row.update(source_count="2"),
            "source_count must be an integer",
            id="quoted-source-count",
        ),
        pytest.param(
            lambda row: row.update(source_count=4_097),
            "source_count exceeds its protocol bound",
            id="source-count-overflow",
        ),
        pytest.param(
            lambda row: row.update(descriptor_hash="73" * 32),
            "descriptor_hash must be a canonical hash literal",
            id="malformed-hash",
        ),
    ],
)
def test_get_sumeragi_diagnostics_rejects_invalid_native_application_shapes(
    mutate: Callable[[Dict[str, Any]], Any],
    error: str,
) -> None:
    payload = _sumeragi_diagnostics_payload()
    application = _native_amx_participant_application_payload()
    mutate(application)
    payload["native_amx_participant_applications"] = [application]

    with pytest.raises(RuntimeError, match=error):
        _get_sumeragi_diagnostics(payload)


def test_get_sumeragi_diagnostics_enforces_native_application_bound_and_order() -> None:
    bounded = _sumeragi_diagnostics_payload()
    bounded["native_amx_participant_applications"] = [
        _native_amx_participant_application_payload(lane_id=lane_id)
        for lane_id in range(1_024)
    ]
    bounded_applications = _get_sumeragi_diagnostics(
        bounded
    ).native_amx_participant_applications
    assert len(bounded_applications) == 1_024

    oversized = _sumeragi_diagnostics_payload()
    oversized["native_amx_participant_applications"] = [None] * 1_025
    with pytest.raises(
        RuntimeError,
        match="native_amx_participant_applications exceeds its protocol item bound",
    ):
        _get_sumeragi_diagnostics(oversized)

    unordered = _sumeragi_diagnostics_payload()
    unordered["native_amx_participant_applications"] = [
        _native_amx_participant_application_payload(lane_id=4),
        _native_amx_participant_application_payload(lane_id=3),
    ]
    with pytest.raises(RuntimeError, match="strictly ordered by route and incarnation"):
        _get_sumeragi_diagnostics(unordered)

    duplicate = _sumeragi_diagnostics_payload()
    application = _native_amx_participant_application_payload()
    duplicate["native_amx_participant_applications"] = [
        application,
        copy.deepcopy(application),
    ]
    with pytest.raises(RuntimeError, match="strictly ordered by route and incarnation"):
        _get_sumeragi_diagnostics(duplicate)


def test_get_sumeragi_diagnostics_parses_autonomous_stage_and_conflict() -> None:
    payload = _sumeragi_diagnostics_payload()
    row = _autonomous_lane_execution_payload()
    payload["autonomous_lane_executions"] = [row]
    parsed = _get_sumeragi_diagnostics(payload).autonomous_lane_executions[0]
    assert parsed.merge_entry_hash == _canonical_hash(0x76)
    assert parsed.application_block_height == 12
    assert parsed.reservation_owner_hash == _canonical_hash(0x66)
    assert parsed.proposal_identity_hash == _canonical_hash(0x67)
    assert parsed.reservation_group_hash == _canonical_hash(0x68)

    payload["autonomous_lane_executions"] = [row, dict(row)]
    with pytest.raises(RuntimeError, match="strictly ordered"):
        _get_sumeragi_diagnostics(payload)
    payload["autonomous_lane_executions"] = [row]
    row["reservation_count"] = 1
    with pytest.raises(RuntimeError, match="reservation and transaction counts disagree"):
        _get_sumeragi_diagnostics(payload)
    row["highest_durable_stage"] = "conflict"
    row["stuck_reason"] = "evidence_conflict"
    assert _get_sumeragi_diagnostics(payload).autonomous_lane_executions[0].stuck_reason == (
        "evidence_conflict"
    )
    row["stuck_reason"] = "awaiting_merge_selection"
    with pytest.raises(RuntimeError, match="stage and stuck reason disagree"):
        _get_sumeragi_diagnostics(payload)


@pytest.mark.parametrize(
    "field",
    ["reservation_owner_hash", "proposal_identity_hash", "reservation_group_hash"],
)
def test_get_sumeragi_diagnostics_requires_provisional_identity_hashes(
    field: str,
) -> None:
    for mutation in ("missing", "zero", "type", "bare-lowercase"):
        row = _autonomous_lane_execution_payload()
        if mutation == "missing":
            del row[field]
        elif mutation == "zero":
            row[field] = "hash:" + ("00" * 32) + "#6A0A"
        elif mutation == "type":
            row[field] = [1] * 32
        else:
            row[field] = "ab" * 32
        payload = _sumeragi_diagnostics_payload()
        payload["autonomous_lane_executions"] = [row]
        with pytest.raises(RuntimeError, match=field):
            _get_sumeragi_diagnostics(payload)


def test_get_sumeragi_diagnostics_enforces_reservation_only_geometry() -> None:
    row = _autonomous_lane_execution_payload()
    row.update(
        highest_durable_stage="reservations_durable",
        stuck_reason="awaiting_executable_payload",
    )
    for field in (
        "proposal_view",
        "proposal_hash",
        "descriptor_hash",
        "executable_payload_hash",
        "source_bundle_hash",
        "merge_entry_hash",
        "application_block_height",
        "application_block_hash",
    ):
        del row[field]
    payload = _sumeragi_diagnostics_payload()
    payload["autonomous_lane_executions"] = [row]
    parsed = _get_sumeragi_diagnostics(payload).autonomous_lane_executions[0]
    assert parsed.proposal_hash is None
    assert parsed.descriptor_hash is None
    assert parsed.proposal_view is None
    assert parsed.stuck_reason == "awaiting_executable_payload"

    for field in (
        "proposal_hash",
        "executable_payload_hash",
        "source_bundle_hash",
        "merge_entry_hash",
        "application_block_height",
    ):
        invalid = copy.deepcopy(row)
        if field == "proposal_hash":
            invalid["proposal_hash"] = _canonical_hash(0x79)
            invalid["descriptor_hash"] = _canonical_hash(0x7A)
        elif field == "application_block_height":
            invalid[field] = 12
            invalid["application_block_hash"] = _canonical_hash(0x7B)
        else:
            invalid[field] = _canonical_hash(0x7C)
        payload["autonomous_lane_executions"] = [invalid]
        with pytest.raises(RuntimeError, match="finalized identity|evidence"):
            _get_sumeragi_diagnostics(payload)

    wrong_reason = copy.deepcopy(row)
    wrong_reason["stuck_reason"] = "awaiting_payload_availability"
    payload["autonomous_lane_executions"] = [wrong_reason]
    with pytest.raises(RuntimeError, match="stage and stuck reason disagree"):
        _get_sumeragi_diagnostics(payload)

    mismatched_counts = copy.deepcopy(row)
    mismatched_counts["reservation_count"] = 1
    payload["autonomous_lane_executions"] = [mismatched_counts]
    with pytest.raises(RuntimeError, match="reservation and transaction counts disagree"):
        _get_sumeragi_diagnostics(payload)

    null_view = copy.deepcopy(row)
    null_view["proposal_view"] = None
    payload["autonomous_lane_executions"] = [null_view]
    assert _get_sumeragi_diagnostics(
        payload
    ).autonomous_lane_executions[0].proposal_view is None

    present_view = copy.deepcopy(row)
    present_view["proposal_view"] = 0
    payload["autonomous_lane_executions"] = [present_view]
    with pytest.raises(RuntimeError, match="proposal view disagrees"):
        _get_sumeragi_diagnostics(payload)


def test_get_sumeragi_diagnostics_enforces_finalized_identity_pair_and_order() -> None:
    payload = _sumeragi_diagnostics_payload()
    for missing_field in ("proposal_hash", "descriptor_hash"):
        row = _autonomous_lane_execution_payload()
        del row[missing_field]
        payload["autonomous_lane_executions"] = [row]
        with pytest.raises(RuntimeError, match="must appear together"):
            _get_sumeragi_diagnostics(payload)

    row = _autonomous_lane_execution_payload()
    row["proposal_hash"] = None
    row["descriptor_hash"] = None
    payload["autonomous_lane_executions"] = [row]
    with pytest.raises(RuntimeError, match="finalized identity disagrees"):
        _get_sumeragi_diagnostics(payload)

    missing_view = _autonomous_lane_execution_payload()
    del missing_view["proposal_view"]
    payload["autonomous_lane_executions"] = [missing_view]
    assert _get_sumeragi_diagnostics(
        payload
    ).autonomous_lane_executions[0].proposal_view is None

    first = _autonomous_lane_execution_payload()
    same_provisional_identity = copy.deepcopy(first)
    same_provisional_identity["proposal_hash"] = _canonical_hash(0x7D)
    same_provisional_identity["descriptor_hash"] = _canonical_hash(0x7E)
    payload["autonomous_lane_executions"] = [first, same_provisional_identity]
    with pytest.raises(RuntimeError, match="strictly ordered"):
        _get_sumeragi_diagnostics(payload)

    first["proposal_identity_hash"] = _canonical_hash(0x90)
    ordering_drift = copy.deepcopy(first)
    ordering_drift["proposal_identity_hash"] = _canonical_hash(0x80)
    ordering_drift["proposal_hash"] = _canonical_hash(0x91)
    ordering_drift["descriptor_hash"] = _canonical_hash(0x92)
    payload["autonomous_lane_executions"] = [first, ordering_drift]
    with pytest.raises(RuntimeError, match="strictly ordered"):
        _get_sumeragi_diagnostics(payload)


def test_get_sumeragi_diagnostics_parses_npos_windows_and_byte_seed() -> None:
    payload = _sumeragi_diagnostics_payload()
    payload["npos"] = {
        "epoch_length_blocks": 100,
        "vrf_commit_deadline_offset": 20,
        "vrf_reveal_deadline_offset": 40,
        "epoch_seed": [1] * 32,
        "prf_height": 10,
        "prf_view": 2,
        "vrf_penalty_epoch": 1,
        "vrf_committed_no_reveal_total": 0,
        "vrf_no_participation_total": 0,
        "vrf_late_reveals_total": 0,
    }

    npos = _get_sumeragi_diagnostics(payload).npos

    assert npos is not None
    assert npos.epoch_seed == (1,) * 32


def test_get_sumeragi_diagnostics_rejects_native_amx_participant_finality_tampering() -> None:
    def extra_leg_field(leg: Dict[str, Any]) -> None:
        leg["future_leg_field"] = 1

    def missing_settlement_hash(leg: Dict[str, Any]) -> None:
        del leg["participant_settlement_hash"]

    def wrong_proposal_type(leg: Dict[str, Any]) -> None:
        leg["participant_proposal"] = []

    def wrong_settlement_hash_type(leg: Dict[str, Any]) -> None:
        leg["participant_settlement_hash"] = 7

    def set_phase_string(leg: Dict[str, Any]) -> None:
        leg["prepare_qc"]["body"]["phase"] = "prepare"

    def missing_body_field(leg: Dict[str, Any]) -> None:
        del leg["prepare_qc"]["body"]["participant_lane_block_height"]

    def extra_body_field(leg: Dict[str, Any]) -> None:
        leg["prepare_qc"]["body"]["future_participant_field"] = 1

    def wrong_body_type(leg: Dict[str, Any]) -> None:
        leg["prepare_qc"]["body"]["participant_lane_block_view"] = "1"

    def mismatch_commit_identity(leg: Dict[str, Any]) -> None:
        leg["commit_qc"]["body"]["participant_proposal_hash"] = _canonical_hash(0x75)

    def mismatch_proposal_hash(leg: Dict[str, Any]) -> None:
        leg["participant_proposal"]["proposal_hash"] = _canonical_hash(0x75)

    def missing_payload_hint(leg: Dict[str, Any]) -> None:
        del leg["participant_proposal"]["payload_block_hint"]

    def nonnull_payload_hint(leg: Dict[str, Any]) -> None:
        leg["participant_proposal"]["payload_block_hint"] = {}

    def extra_proposal_field(leg: Dict[str, Any]) -> None:
        leg["participant_proposal"]["future_proposal_field"] = None

    def missing_descriptor_field(leg: Dict[str, Any]) -> None:
        del leg["participant_proposal"]["descriptor"]["subject_hash"]

    def extra_descriptor_field(leg: Dict[str, Any]) -> None:
        leg["participant_proposal"]["descriptor"]["future_descriptor_field"] = 1

    def missing_predecessor(leg: Dict[str, Any]) -> None:
        del leg["participant_proposal"]["descriptor"]["previous_lane_block_descriptor_hash"]

    def null_non_genesis_predecessor(leg: Dict[str, Any]) -> None:
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["body"]["participant_previous_block_descriptor_hash"] = None

    def nonnull_genesis_predecessor(leg: Dict[str, Any]) -> None:
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["body"]["participant_previous_block_height"] = 0
            qc["body"]["participant_lane_block_height"] = 1

    def explicit_null_genesis_descriptor(leg: Dict[str, Any]) -> None:
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["body"]["participant_previous_block_height"] = 0
            qc["body"]["participant_previous_block_descriptor_hash"] = None
            qc["body"]["participant_lane_block_height"] = 1
        descriptor = leg["participant_proposal"]["descriptor"]
        descriptor["previous_lane_block_height"] = 0
        descriptor["previous_lane_block_descriptor_hash"] = None
        descriptor["lane_block_height"] = 1
        leg["participant_settlement"]["block_height"] = 1

    def mismatch_proposal_route(leg: Dict[str, Any]) -> None:
        leg["participant_proposal"]["descriptor"]["lane_id"] = 99

    def mismatch_proposal_height(leg: Dict[str, Any]) -> None:
        leg["participant_proposal"]["descriptor"]["proposal_height"] = 11

    def mismatch_settlement_hash(leg: Dict[str, Any]) -> None:
        leg["participant_settlement_hash"] = _canonical_hash(0x79)

    def mismatch_settlement_route(leg: Dict[str, Any]) -> None:
        leg["participant_settlement"]["lane_id"] = 99

    def nonzero_participant_effect(leg: Dict[str, Any]) -> None:
        leg["participant_settlement"]["total_local_amount"] = "1"

    def mismatch_settlement_source(leg: Dict[str, Any]) -> None:
        leg["participant_settlement"]["receipts"][0]["source_id"] = "EF" * 32

    def duplicate_settlement_source(leg: Dict[str, Any]) -> None:
        leg["participant_settlement"]["receipts"][1]["source_id"] = "AB" * 32

    def wrong_settlement_tx_count(leg: Dict[str, Any]) -> None:
        leg["participant_settlement"]["tx_count"] = 1

    def empty_settlement(leg: Dict[str, Any]) -> None:
        leg["participant_settlement"]["tx_count"] = 0
        leg["participant_settlement"]["receipts"] = []

    def oversized_settlement(leg: Dict[str, Any]) -> None:
        receipt = copy.deepcopy(leg["participant_settlement"]["receipts"][0])
        leg["participant_settlement"]["tx_count"] = 4097
        leg["participant_settlement"]["receipts"] = [receipt] * 4097

    def recursive_settlement(leg: Dict[str, Any]) -> None:
        leg["participant_settlement"]["native_amx_receipts"] = [{}]

    mutations = (
        extra_leg_field,
        missing_settlement_hash,
        wrong_proposal_type,
        wrong_settlement_hash_type,
        set_phase_string,
        missing_body_field,
        extra_body_field,
        wrong_body_type,
        mismatch_commit_identity,
        mismatch_proposal_hash,
        missing_payload_hint,
        nonnull_payload_hint,
        extra_proposal_field,
        missing_descriptor_field,
        extra_descriptor_field,
        missing_predecessor,
        null_non_genesis_predecessor,
        nonnull_genesis_predecessor,
        explicit_null_genesis_descriptor,
        mismatch_proposal_route,
        mismatch_proposal_height,
        mismatch_settlement_hash,
        mismatch_settlement_route,
        nonzero_participant_effect,
        mismatch_settlement_source,
        duplicate_settlement_source,
        wrong_settlement_tx_count,
        empty_settlement,
        oversized_settlement,
        recursive_settlement,
    )
    for mutate in mutations:
        payload = _sumeragi_diagnostics_payload()
        settlement = _lane_settlement_payload()
        native = _native_amx_receipt_payload()
        mutate(native["legs"][0])
        settlement["native_amx_receipts"] = [native]
        payload["lane_settlement_commitments"] = [settlement]
        with pytest.raises(RuntimeError, match="."):
            _get_sumeragi_diagnostics(payload)


@pytest.mark.parametrize(
    "invalid",
    [
        7,
        True,
        "01",
        "1.0",
        "1.",
        "-1",
        "1e3",
        "not-a-quantity",
        "0.00000000000000000000000000001",
        str(1 << 511),
        "1" * 156,
    ],
)
def test_get_sumeragi_diagnostics_rejects_noncanonical_quantity_json(invalid: Any) -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    settlement["total_local_amount"] = invalid
    payload["lane_settlement_commitments"] = [settlement]

    with pytest.raises(
        RuntimeError,
        match="total_local_amount.*(?:quantity|canonical|length|512-bit)",
    ):
        _get_sumeragi_diagnostics(payload)


def test_get_sumeragi_diagnostics_preserves_exact_quantity_boundaries() -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    maximum = str((1 << 511) - 1)
    scale_28_maximum = f"{maximum[:126]}.{maximum[126:]}"
    assert len(scale_28_maximum) == 155
    settlement["total_local_amount"] = scale_28_maximum
    settlement["total_xor_due"] = "0.0000000000000000000000000001"
    settlement["total_xor_after_haircut"] = "123.000000001"
    settlement["total_xor_variance"] = "0"
    settlement["receipts"][0].update(
        {
            "local_amount": maximum,
            "xor_due": "0.0000000000000000000000000001",
            "xor_after_haircut": "123.000000001",
            "xor_variance": "0",
        }
    )
    payload["lane_settlement_commitments"] = [settlement]

    parsed = _get_sumeragi_diagnostics(payload).lane_settlement_commitments[0]

    assert parsed["total_local_amount"] == scale_28_maximum
    assert parsed["total_xor_due"] == "0.0000000000000000000000000001"
    assert parsed["receipts"][0]["xor_after_haircut"] == "123.000000001"


@pytest.mark.parametrize(
    "retired_field",
    [
        "total_local_micro",
        "total_xor_due_micro",
        "total_xor_after_haircut_micro",
        "total_xor_variance_micro",
    ],
)
def test_get_sumeragi_diagnostics_rejects_retired_settlement_fields(
    retired_field: str,
) -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    settlement[retired_field] = "0"
    payload["lane_settlement_commitments"] = [settlement]

    with pytest.raises(RuntimeError, match=f"unknown field {retired_field}"):
        _get_sumeragi_diagnostics(payload)


@pytest.mark.parametrize(
    "retired_field",
    [
        "local_amount_micro",
        "xor_due_micro",
        "xor_after_haircut_micro",
        "xor_variance_micro",
    ],
)
def test_get_sumeragi_diagnostics_rejects_retired_settlement_receipt_fields(
    retired_field: str,
) -> None:
    payload = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    settlement["receipts"][0][retired_field] = "0"
    payload["lane_settlement_commitments"] = [settlement]

    with pytest.raises(RuntimeError, match=f"unknown field {retired_field}"):
        _get_sumeragi_diagnostics(payload)


def test_get_sumeragi_diagnostics_rejects_noncanonical_fixed_hex_and_nested_unknown_fields() -> None:
    lowercase = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    fee = _nexus_fee_receipt_payload()
    fee["source_id"] = "ab" * 32
    settlement["nexus_fee_receipts"] = [fee]
    lowercase["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="source_id.*uppercase"):
        _get_sumeragi_diagnostics(lowercase)

    unknown_fee = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    fee = _nexus_fee_receipt_payload()
    fee["schedule"]["legacy_rate"] = "1"
    settlement["nexus_fee_receipts"] = [fee]
    unknown_fee["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="schedule contains unknown field legacy_rate"):
        _get_sumeragi_diagnostics(unknown_fee)

    overflowing = "9" * 155
    for invalid in [
        1,
        1.5,
        "+1",
        "01",
        "1.0",
        "1.2300",
        " 1",
        "1 ",
        "-1",
        overflowing,
    ]:
        invalid_fee = _sumeragi_diagnostics_payload()
        settlement = _lane_settlement_payload()
        fee = _nexus_fee_receipt_payload()
        fee["fee_amount"] = invalid
        settlement["nexus_fee_receipts"] = [fee]
        invalid_fee["lane_settlement_commitments"] = [settlement]
        with pytest.raises(RuntimeError, match="fee_amount.*(?:quantity|512-bit)"):
            _get_sumeragi_diagnostics(invalid_fee)

    invalid_schedule = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    fee = _nexus_fee_receipt_payload()
    fee["schedule"]["base_fee"] = "2.0"
    settlement["nexus_fee_receipts"] = [fee]
    invalid_schedule["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="base_fee.*quantity"):
        _get_sumeragi_diagnostics(invalid_schedule)

    unknown_amx = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"][0]["prepare_qc"]["body"]["legacy_round"] = 1
    settlement["native_amx_receipts"] = [native]
    unknown_amx["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="body contains unknown field legacy_round"):
        _get_sumeragi_diagnostics(unknown_amx)


def test_get_sumeragi_diagnostics_rejects_nested_receipt_coordinate_and_qc_tampering() -> None:
    wrong_coordinate = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    fee = _nexus_fee_receipt_payload()
    fee["block_height"] = 8
    settlement["nexus_fee_receipts"] = [fee]
    wrong_coordinate["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="receipt coordinates do not match"):
        _get_sumeragi_diagnostics(wrong_coordinate)
    for bitmap in ([0x03], [0x0F]):
        invalid_quorum = _sumeragi_diagnostics_payload()
        settlement = _lane_settlement_payload()
        native = _native_amx_receipt_payload()
        native["legs"][0]["prepare_qc"]["signers_bitmap"] = bitmap
        settlement["native_amx_receipts"] = [native]
        invalid_quorum["lane_settlement_commitments"] = [settlement]
        with pytest.raises(RuntimeError, match="signers_bitmap does not carry the exact quorum"):
            _get_sumeragi_diagnostics(invalid_quorum)
    malformed_pop = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"][0]["commit_qc"]["validator_set_pops"][0] = [1] * 95
    settlement["native_amx_receipts"] = [native]
    malformed_pop["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match=r"validator_set_pops\[0\].*96"):
        _get_sumeragi_diagnostics(malformed_pop)

    mismatched_phase_identity = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"][0]["commit_qc"]["body"]["plan_digest"] = _canonical_hash(0x70)
    settlement["native_amx_receipts"] = [native]
    mismatched_phase_identity["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="prepare and commit identities differ"):
        _get_sumeragi_diagnostics(mismatched_phase_identity)


def test_get_sumeragi_diagnostics_rejects_bounded_vector_overflow_before_nested_decode() -> None:
    too_many_settlements = _sumeragi_diagnostics_payload()
    too_many_settlements["lane_settlement_commitments"] = [{}] * 129
    with pytest.raises(RuntimeError, match="lane_settlement_commitments exceeds"):
        _get_sumeragi_diagnostics(too_many_settlements)

    too_many_relays = _sumeragi_diagnostics_payload()
    too_many_relays["lane_relay_envelopes"] = [{}] * 65
    with pytest.raises(RuntimeError, match="lane_relay_envelopes exceeds"):
        _get_sumeragi_diagnostics(too_many_relays)

    too_many_legs = _sumeragi_diagnostics_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"] = native["legs"] * 256
    settlement["native_amx_receipts"] = [native]
    too_many_legs["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="legs exceeds"):
        _get_sumeragi_diagnostics(too_many_legs)


def test_get_sumeragi_status_rejects_protocol_context_and_commit_tampering() -> None:
    legacy_field = _sumeragi_v2_status_payload()
    legacy_field["mode_tag"] = "retired"
    with pytest.raises(RuntimeError, match="unknown field mode_tag"):
        _get_sumeragi_status(legacy_field)

    wrong_version = _sumeragi_v2_status_payload()
    wrong_version["protocol_version"] = 3
    with pytest.raises(RuntimeError, match="protocol_version must equal 4"):
        _get_sumeragi_status(wrong_version)

    missing_restart_required = _sumeragi_v2_status_payload()
    del missing_restart_required["restart_required"]
    with pytest.raises(RuntimeError, match="restart_required must be a boolean"):
        _get_sumeragi_status(missing_restart_required)

    invalid_restart_required = _sumeragi_v2_status_payload()
    invalid_restart_required["restart_required"] = 0
    with pytest.raises(RuntimeError, match="restart_required must be a boolean"):
        _get_sumeragi_status(invalid_restart_required)

    quoted_height = _sumeragi_v2_status_payload()
    quoted_height["height"] = "10"
    with pytest.raises(RuntimeError, match="height must be an integer"):
        _get_sumeragi_status(quoted_height)

    wrong_quorum = _sumeragi_v2_status_payload()
    wrong_quorum["height_context"]["quorum"]["min_signers"] = 2
    with pytest.raises(RuntimeError, match="quorum is not canonical"):
        _get_sumeragi_status(wrong_quorum)

    missing_enum_details = _sumeragi_v2_status_payload()
    del missing_enum_details["phase"]["details"]
    with pytest.raises(RuntimeError, match="phase.details must be explicitly null"):
        _get_sumeragi_status(missing_enum_details)

    wrong_leader = _sumeragi_v2_status_payload()
    wrong_leader["leader"] = 4
    with pytest.raises(RuntimeError, match="leader must index"):
        _get_sumeragi_status(wrong_leader)

    wrong_subject = _sumeragi_v2_status_payload()
    wrong_subject["last_commit_qc"]["certificate"]["subject"]["block_hash"] = (
        _canonical_hash(0x77)
    )
    with pytest.raises(RuntimeError, match="does not certify the committed subject"):
        _get_sumeragi_status(wrong_subject)

    missing_proposal_round = _sumeragi_v2_status_payload()
    del missing_proposal_round["last_commit_qc"]["certificate"]["proposal_round"]
    with pytest.raises(RuntimeError, match="proposal_round"):
        _get_sumeragi_status(missing_proposal_round)

    foreign_proposal_round = _sumeragi_v2_status_payload()
    foreign_proposal_round["last_commit_qc"]["certificate"]["proposal_round"][
        "context_id"
    ] = [_canonical_hash(0x42)]
    with pytest.raises(RuntimeError, match="proposal_round must match round context"):
        _get_sumeragi_status(foreign_proposal_round)

    wrong_proposal_height = _sumeragi_v2_status_payload()
    wrong_proposal_height["last_commit_qc"]["certificate"]["proposal_round"][
        "height"
    ] = 8
    with pytest.raises(RuntimeError, match="proposal_round must match round context"):
        _get_sumeragi_status(wrong_proposal_height)

    future_proposal_round = _sumeragi_v2_status_payload()
    future_proposal_round["last_commit_qc"]["certificate"]["proposal_round"][
        "view"
    ] = 2
    with pytest.raises(RuntimeError, match="proposal_round must equal round"):
        _get_sumeragi_status(future_proposal_round)
    underpowered = _sumeragi_v2_status_payload()
    underpowered["last_commit_qc"]["signed_power"] = 2
    with pytest.raises(RuntimeError, match="exact frozen certificate quorum"):
        _get_sumeragi_status(underpowered)
    overcomplete = _sumeragi_v2_status_payload()
    overcomplete["last_commit_qc"].update(signer_count=4, signed_power=4)
    with pytest.raises(RuntimeError, match="exact frozen certificate quorum"):
        _get_sumeragi_status(overcomplete)
    weighted_npos = _sumeragi_v2_status_payload()
    weighted_npos["height_context"]["mode"] = {"mode": "npos", "details": None}
    weighted_npos["height_context"]["quorum"]["total_power"] = 5
    with pytest.raises(RuntimeError, match="quorum is not canonical"):
        _get_sumeragi_status(weighted_npos)
    invalid_geometry = _sumeragi_v2_status_payload()
    invalid_geometry["height_context"]["validator_count"] = 5
    invalid_geometry["height_context"]["quorum"]["min_signers"] = 4
    invalid_geometry["height_context"]["quorum"]["total_power"] = 5
    with pytest.raises(RuntimeError, match="quorum is not canonical"):
        _get_sumeragi_status(invalid_geometry)


def test_get_sumeragi_status_allows_authenticated_bootstrap_without_commit_details() -> None:
    payload = _sumeragi_v2_status_payload()
    payload["last_committed_subject"] = None
    payload["last_commit_qc"] = None

    status = _get_sumeragi_status(payload)

    assert status.last_committed_height == 9
    assert status.last_committed_subject is None
    assert status.last_commit_qc is None


def test_get_sumeragi_diagnostics_rejects_impossible_queue_bounds() -> None:
    depth_overflow = _sumeragi_diagnostics_payload()
    depth_overflow["tx_queue_depth"] = 33
    with pytest.raises(RuntimeError, match="queue depth exceeds capacity"):
        _get_sumeragi_diagnostics(depth_overflow)

    byte_overflow = _sumeragi_diagnostics_payload()
    byte_overflow["tx_queue_retained_bytes"] = 65537
    with pytest.raises(RuntimeError, match="retained queue bytes exceed"):
        _get_sumeragi_diagnostics(byte_overflow)


@pytest.mark.parametrize(
    "field",
    [
        "lane_settlement_commitments",
        "lane_relay_envelopes",
        "lane_payload_ownerships",
        "committed_lane_blocks",
        "lane_block_sessions",
        "autonomous_lane_executions",
    ],
)
def test_get_sumeragi_diagnostics_requires_all_canonical_lane_arrays(
    field: str,
) -> None:
    payload = _sumeragi_diagnostics_payload()
    del payload[field]

    with pytest.raises(RuntimeError, match=rf"missing required field {field}"):
        _get_sumeragi_diagnostics(payload)


@pytest.mark.parametrize(
    ("method", "path"),
    (
        ("GET", "/v1/sumeragi/rbc"),
        ("GET", "/v1/sumeragi/rbc/delivered/1/0"),
        ("GET", "/v1/sumeragi/rbc/sessions"),
        ("POST", "/v1/sumeragi/rbc/sample"),
        ("GET", "/v1/sumeragi/collectors"),
    ),
)
def test_mock_server_rejects_retired_global_sumeragi_routes(method: str, path: str) -> None:
    server = ToriiMockServer().start()
    try:
        response = requests.request(
            method,
            f"{server.base_url.rstrip('/')}{path}",
            json={} if method == "POST" else None,
            timeout=5.0,
        )

        assert response.status_code == 404
    finally:
        server.stop()


def test_get_runtime_abi_active_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "abi_version": 1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_runtime_abi_active(canonical_auth=_governance_auth())

    assert snapshot.abi_version == 1


def test_get_runtime_abi_hash_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "policy": "V1",
                "abi_hash_hex": "aa" * 32,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.get_runtime_abi_hash()

    assert result.policy == "V1"
    assert result.abi_hash_hex == "aa" * 32


def test_get_runtime_metrics_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "abi_version": 1,
                "upgrade_events_total": {"proposed": 5, "activated": 3, "canceled": 1},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    metrics = client.get_runtime_metrics(canonical_auth=_governance_auth())

    assert metrics.abi_version == 1
    assert metrics.upgrade_events_total.proposed == 5
    assert metrics.upgrade_events_total.activated == 3
    assert metrics.upgrade_events_total.canceled == 1


def test_list_runtime_upgrades_parses_records() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "id_hex": "aa" * 32,
                        "record": {
                            "manifest": {
                                "name": "ABI v1 refresh",
                                "description": "scheduled rollout",
                                "abi_version": 1,
                                "abi_hash": "11" * 32,
                                "added_syscalls": [],
                                "added_pointer_types": [],
                                "start_height": 10,
                                "end_height": 20,
                            },
                            "status": {"ActivatedAt": 12},
                            "proposer": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
                            "created_height": 8,
                        },
                    },
                    {
                        "id_hex": "bb" * 32,
                        "record": {
                            "manifest": {
                                "name": "ABI v1 maintenance",
                                "description": "next window",
                                "abi_version": 1,
                                "abi_hash": "22" * 32,
                                "added_syscalls": [],
                                "added_pointer_types": [],
                                "start_height": 30,
                                "end_height": 40,
                            },
                            "status": {"Proposed": None},
                            "proposer": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
                            "created_height": 25,
                        },
                    },
                ]
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    upgrades = client.list_runtime_upgrades()

    assert len(upgrades) == 2
    assert upgrades[0].record.status.kind == "ActivatedAt"
    assert upgrades[0].record.status.activated_height == 12
    assert upgrades[1].record.status.kind == "Proposed"


def test_propose_runtime_upgrade_posts_manifest() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [{"wire_id": "ProposeRuntimeUpgrade", "payload_hex": "aa" * 32}],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.propose_runtime_upgrade(
        {
            "name": "ABI v1 maintenance",
            "description": "roll out refreshed binaries",
            "abi_version": 1,
            "abi_hash": "ff" * 32,
            "start_height": 50,
            "end_height": 60,
            "added_syscalls": [],
            "added_pointer_types": [],
        }
    )

    assert result.ok is True
    assert result.tx_instructions[0].wire_id == "ProposeRuntimeUpgrade"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades/propose")
    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert body["abi_version"] == 1
    assert body["abi_hash"] == "ff" * 32


def test_activate_runtime_upgrade_posts_identifier() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [{"wire_id": "ActivateRuntimeUpgrade", "payload_hex": "cc" * 32}],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    response = client.activate_runtime_upgrade("0x" + "bb" * 32)

    assert response.tx_instructions[0].wire_id == "ActivateRuntimeUpgrade"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades/activate/0x" + "bb" * 32)


def test_cancel_runtime_upgrade_posts_identifier() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [{"wire_id": "CancelRuntimeUpgrade", "payload_hex": "dd" * 32}],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    response = client.cancel_runtime_upgrade("aa" * 32)

    assert response.tx_instructions[0].wire_id == "CancelRuntimeUpgrade"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades/cancel/0x" + "aa" * 32)


def test_get_uaid_portfolio_parses_payload() -> None:
    uaid_literal = "UAID:" + "AB" * 32
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "uaid": uaid_literal,
                "totals": {"accounts": 2, "positions": 3},
                "dataspaces": [
                    {
                        "dataspace_id": 7,
                        "dataspace_alias": "treasury",
                        "accounts": [
                            {
                                "account_id": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
                                "label": "primary",
                                "assets": [
                                    {
                                        "asset_id": CANONICAL_ASSET_ID,
                                        "asset_definition_id": CANONICAL_ASSET_ID.split("#", 1)[0],
                                        "quantity": "42",
                                    }
                                ],
                            }
                        ],
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    response = client.get_uaid_portfolio(uaid_literal)

    assert response.uaid == "uaid:" + "ab" * 32
    assert response.totals.accounts == 2
    assert response.dataspaces[0].accounts[0].assets[0].quantity == "42"
    expected_suffix = "/v1/accounts/uaid%3A" + "ab" * 32 + "/portfolio"
    assert session.calls[0]["url"].endswith(expected_suffix)


def test_get_uaid_portfolio_rejects_padded_literal_before_dispatch() -> None:
    uaid_hex = "ab" * 32
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    for literal in [
        f" uaid:{uaid_hex}",
        f"uaid:{uaid_hex} ",
        f"uaid: {uaid_hex}",
    ]:
        with pytest.raises(ValueError, match="uaid must not contain surrounding whitespace"):
            client.get_uaid_portfolio(literal)

    assert session.calls == []


def test_get_uaid_portfolio_encodes_asset_id_filter() -> None:
    uaid_literal = "uaid:" + "ab" * 32
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "uaid": uaid_literal,
                "totals": {"accounts": 0, "positions": 0},
                "dataspaces": [],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.get_uaid_portfolio(uaid_literal, asset_id=CANONICAL_ASSET_ID)

    assert session.calls[0]["params"]["asset_id"] == CANONICAL_ASSET_ID


def test_get_uaid_portfolio_rejects_padded_asset_id_before_dispatch() -> None:
    uaid_literal = "uaid:" + "ab" * 32
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="uaid portfolio asset_id must not contain surrounding whitespace"):
        client.get_uaid_portfolio(uaid_literal, asset_id=f" {CANONICAL_ASSET_ID}")

    with pytest.raises(ValueError, match="uaid portfolio asset_id must not contain surrounding whitespace"):
        client.get_uaid_portfolio(uaid_literal, asset_id=f"{CANONICAL_ASSET_ID} ")

    assert session.calls == []


def test_get_uaid_portfolio_rejects_invalid_lsb() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())
    invalid = "uaid:" + "10" * 32
    with pytest.raises(RuntimeError, match="least significant bit"):
        client.get_uaid_portfolio(invalid)


def test_get_uaid_bindings_fetches_dataspace_accounts() -> None:
    uaid_literal = "uaid:" + "bb" * 32
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "uaid": uaid_literal,
                "dataspaces": [
                    {
                        "dataspace_id": 9,
                        "dataspace_alias": "alpha",
                        "accounts": ["sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6", " sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE "],
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    bindings = client.get_uaid_bindings(uaid_literal)

    assert bindings.dataspaces[0].accounts == ["sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6", "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"]
    assert session.calls[0]["params"] == {}


def test_get_uaid_manifests_parses_payload_and_filters() -> None:
    uaid_literal = "uaid:" + "cd" * 32
    manifest_hash = "0x" + "dd" * 32
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "uaid": uaid_literal,
                "manifests": [
                    {
                        "dataspace_id": 5,
                        "dataspace_alias": "lane-5",
                        "manifest_hash": manifest_hash,
                        "status": "Active",
                        "lifecycle": {
                            "activated_epoch": 12,
                            "revocation": {"epoch": 44, "reason": "duplicate"},
                        },
                        "accounts": ["sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"],
                        "manifest": {
                            "version": "1.0",
                            "uaid": uaid_literal,
                            "dataspace": 5,
                            "issued_ms": 123,
                            "activation_epoch": 12,
                            "entries": [
                                {
                                    "scope": {"accounts": ["sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"]},
                                    "effect": {"action": "allow"},
                                    "notes": "demo",
                                }
                            ],
                        },
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    manifests = client.get_uaid_manifests(
        uaid_literal,
        dataspace_id=9,
    )

    assert len(manifests.manifests) == 1
    record = manifests.manifests[0]
    assert record.manifest_hash == manifest_hash.lower()
    assert record.lifecycle.revocation is not None
    assert record.manifest.entries[0].notes == "demo"
    assert session.calls[0]["params"] == {"dataspace": 9}


def test_get_configuration_returns_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "public_key": "ed0123",
                "logger": {"level": "Info", "filter": None},
                "network": {
                    "block_gossip_size": 32,
                    "block_gossip_period_ms": 150,
                    "transaction_gossip_size": 16,
                    "transaction_gossip_period_ms": 75,
                },
                "queue": {"capacity": 1024},
                "confidential_gas": {
                    "proof_base": 10,
                    "per_public_input": 2,
                    "per_proof_byte": 3,
                    "per_nullifier": 4,
                    "per_commitment": 5,
                },
                "transport": {
                    "norito_rpc": {
                        "enabled": True,
                        "stage": "ga",
                        "require_mtls": True,
                        "canary_allowlist_size": 3,
                    },
                    "streaming": {
                        "soranet": {
                            "enabled": True,
                            "stream_tag": "norito",
                            "exit_multiaddr": "/dns/torii/udp/9443/quic",
                            "padding_budget_ms": 25,
                            "access_kind": "authenticated",
                            "gar_category": "soranet-auth",
                            "channel_salt": "salt-123",
                            "provision_spool_dir": "./storage/streaming/soranet_routes",
                            "provision_window_segments": 4,
                            "provision_queue_capacity": 256,
                        }
                    },
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_configuration()

    assert snapshot.public_key_hex == "ed0123"
    assert snapshot.logger.level == "Info"
    assert snapshot.logger.filter is None
    assert snapshot.queue is not None and snapshot.queue.capacity == 1024
    assert snapshot.confidential_gas is not None
    assert snapshot.confidential_gas.per_nullifier == 4
    transport = snapshot.transport
    assert transport is not None
    assert transport.norito_rpc is not None
    assert transport.norito_rpc.stage == "ga"
    assert transport.norito_rpc.canary_allowlist_size == 3
    assert transport.streaming is not None
    assert transport.streaming.soranet is not None
    assert transport.streaming.soranet.padding_budget_ms == 25
    assert transport.streaming.soranet.provision_queue_capacity == 256


def test_update_configuration_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=202))
    client = ToriiClient("http://node.test", session=session)

    result = client.update_configuration({"logger": {"level": "Info", "filter": "net=debug"}})

    assert result == {}
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"].endswith("/v1/configuration")
    assert json.loads(session.calls[0]["data"]) == {"logger": {"level": "Info", "filter": "net=debug"}}


def test_get_sumeragi_qc_parses_authoritative_v2_references() -> None:
    highest = copy.deepcopy(_sumeragi_v2_status_payload()["last_commit_qc"]["certificate"])
    highest["phase"] = {"phase": "prepare", "details": None}
    locked = copy.deepcopy(highest)
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "highest_prepare_qc": highest,
                "locked_prepare_qc": locked,
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    snapshot = client.get_sumeragi_qc()

    assert snapshot.highest_prepare_qc is not None
    assert snapshot.highest_prepare_qc.round.height == 9
    assert snapshot.highest_prepare_qc.phase == "prepare"
    assert snapshot.locked_prepare_qc is not None
    assert snapshot.locked_prepare_qc.subject.block_hash == _canonical_hash(0x32)
    assert session.calls[0]["url"].endswith("/v1/sumeragi/qc")


def test_get_sumeragi_qc_rejects_pre_release_snapshot_shape() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "highest_qc": {"height": 10, "view": 2, "subject_block_hash": "aa11"},
                "locked_qc": {"height": 9, "view": 1, "subject_block_hash": None},
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(RuntimeError, match="unknown field highest_qc"):
        client.get_sumeragi_qc()


def test_get_sumeragi_qc_requires_both_nullable_slots() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"highest_prepare_qc": None}))
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(RuntimeError, match=r"locked_prepare_qc is required"):
        client.get_sumeragi_qc()


def test_get_status_snapshot_parses_payload_and_computes_metrics() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=_status_payload(queue_size=4, da_total=1, approved=3, rejected=1, views=2)))
    session.queue(StubResponse(payload=_status_payload(queue_size=9, da_total=4, approved=5, rejected=2, views=5)))
    client = ToriiClient("http://node.test", session=session)

    first = client.get_status_snapshot()
    second = client.get_status_snapshot()

    assert first.status.queue_size == 4
    assert first.status.queue_queued == 2
    assert first.status.queue_inflight == 2
    assert first.metrics.time_since_last_non_empty_block_ms == 1_000
    assert first.status.is_queue_stalled(999) is True
    assert first.status.is_queue_stalled(1_000) is False
    assert first.metrics.queue_delta == 0
    assert first.metrics.has_activity is False
    assert first.status.lane_commitments[0].lane_id == 7
    assert first.status.dataspace_commitments[0].dataspace_id == 9
    assert first.status.dataspace_catalog[0].alias == "alpha"

    assert second.status.queue_size == 9
    assert second.metrics.queue_queued == 7
    assert second.metrics.queue_inflight == 2
    assert second.metrics.queue_delta == 5
    assert second.metrics.da_reschedule_delta == 3
    assert second.metrics.tx_approved_delta == 2
    assert second.metrics.tx_rejected_delta == 1
    assert second.metrics.view_change_delta == 3
    assert second.metrics.has_activity is True
    lane_gov = second.status.lane_governance[0]
    assert lane_gov.alias == "lane-alpha"
    assert lane_gov.runtime_upgrade is not None
    assert lane_gov.runtime_upgrade.allowed_ids == ["alpha"]
    activation = second.status.governance.recent_manifest_activations[0]
    assert (
        activation.contract_address
        == "xorc1qyqqqqqqqqqqqq9a5v7f58jgm40m0w7esnqg2pxj68d3f8a2l9ja3s"
    )
    assert second.status.lane_governance_sealed_aliases == ["sealed-one"]
    assert second.status.require_dataspace("alpha").sealed is True
    assert second.status.require_dataspace(9).manifest_required is True
    assert "peers" in second.status.raw


def test_get_pipeline_preflight_parses_payload_and_liveness_helper() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "schema_version": 1,
                "chain_height": 42,
                "sumeragi": {
                    "block_time_ms": 1_000,
                    "commit_time_ms": 2_000,
                    "stall_threshold_ms": 6_000,
                },
                "admission": {
                    "max_signatures": 32,
                    "max_instructions": 4096,
                    "max_tx_bytes": 1_048_576,
                    "max_decompressed_bytes": 1_048_576,
                    "max_metadata_depth": 16,
                },
                "block": {"max_transactions": 512},
                "pipeline": {
                    "signature_batch_max": 0,
                    "signature_batch_max_ed25519": 64,
                    "signature_batch_max_secp256k1": 16,
                    "signature_batch_max_pqc": 8,
                    "signature_batch_max_bls": 16,
                    "overlay_max_instructions": 0,
                    "ivm_max_decoded_instructions": 1_048_576,
                },
                "queue": {"size": 2, "queued": 1, "inflight": 1},
                "fees": {
                    "fee_asset_id": "xor#sora",
                    "fee_sink_account_id": "fees@system",
                    "base_fee": "0",
                    "per_byte_fee": "0",
                    "per_instruction_fee": "0",
                    "per_gas_unit_fee": "0",
                    "sponsor_vault_custody_account_id": "vault@system",
                    "settlement_mode": "direct",
                    "successful_claim_fee_exempt_authorities": ["authority@system"],
                },
            }
        )
    )
    status_payload = _status_payload(
        queue_size=2,
        da_total=0,
        approved=0,
        rejected=0,
        views=0,
    )
    status_payload["time_since_last_non_empty_block_ms"] = 6_001
    session.queue(StubResponse(payload=status_payload))
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    preflight = client.get_pipeline_preflight()
    status = client.get_status_snapshot().status

    assert preflight.schema_version == 1
    assert preflight.chain_height == 42
    assert preflight.sumeragi.stall_threshold_ms == 6_000
    assert preflight.admission.max_tx_bytes == 1_048_576
    assert preflight.pipeline.signature_batch_max_ed25519 == 64
    assert preflight.queue.queued == 1
    assert preflight.fees.base_fee == "0"
    assert preflight.fees.sponsor_vault_custody_account_id == "vault@system"
    assert preflight.fees.successful_claim_fee_exempt_authorities == ["authority@system"]
    assert preflight.is_status_stalled(status) is True
    assert session.calls[0]["url"].endswith("/v1/pipeline/preflight")


def _status_payload(
    *,
    queue_size: int,
    da_total: int,
    approved: int,
    rejected: int,
    views: int,
) -> Dict[str, Any]:
    governance = {
        "proposals": {
            "proposed": 1,
            "approved": 2,
            "rejected": 3,
            "enacted": 4,
        },
        "protected_namespace": {
            "total_checks": 4,
            "allowed": 3,
            "rejected": 1,
        },
        "manifest_admission": {
            "total_checks": 5,
            "allowed": 4,
            "missing_manifest": 1,
            "non_validator_authority": 0,
            "quorum_rejected": 0,
            "protected_namespace_rejected": 0,
            "runtime_hook_rejected": 0,
        },
        "manifest_quorum": {
            "total_checks": 3,
            "satisfied": 2,
            "rejected": 1,
        },
        "recent_manifest_activations": [
            {
                "contract_address": "xorc1qyqqqqqqqqqqqq9a5v7f58jgm40m0w7esnqg2pxj68d3f8a2l9ja3s",
                "code_hash_hex": "deadbeef",
                "abi_hash_hex": "cafebabe",
                "height": 42,
                "activated_at_ms": 1_111,
            }
        ],
    }
    lane_commitments = [
        {
            "block_height": 10,
            "lane_id": 7,
            "tx_count": 2,
            "total_chunks": 4,
            "rbc_bytes_total": 64,
            "teu_total": 128,
            "block_hash": "hash-lane",
        }
    ]
    dataspace_commitments = [
        {
            "block_height": 10,
            "lane_id": 7,
            "dataspace_id": 9,
            "tx_count": 2,
            "total_chunks": 4,
            "rbc_bytes_total": 64,
            "teu_total": 128,
            "block_hash": "hash-dataspace",
        }
    ]
    lane_governance = [
        {
            "lane_id": 7,
            "alias": "lane-alpha",
            "dataspace_id": 9,
            "visibility": "public",
            "storage_profile": "balanced",
            "governance": None,
            "manifest_required": True,
            "manifest_ready": False,
            "manifest_path": None,
            "validator_ids": ["val#1"],
            "quorum": 1,
            "protected_namespaces": ["alpha"],
            "runtime_upgrade": {
                "allow": True,
                "require_metadata": True,
                "metadata_key": "manifest",
                "allowed_ids": ["alpha"],
            },
        }
    ]
    dataspace_catalog = [
        {
            "lane_id": 7,
            "lane_alias": "lane-alpha",
            "dataspace_id": 9,
            "alias": "alpha",
            "visibility": "restricted",
            "storage_profile": "balanced",
            "manifest_required": True,
            "manifest_ready": False,
            "sealed": True,
            "manifest_path": None,
            "protected_namespaces": ["alpha"],
        }
    ]
    return {
        "observed_at_ms": 10_000,
        "peers": 5,
        "queue_size": queue_size,
        "queue_queued": max(0, queue_size - 2),
        "queue_inflight": min(queue_size, 2),
        "last_block_committed_at_ms": 9_900,
        "last_non_empty_block_committed_at_ms": 9_000,
        "time_since_last_block_ms": 100,
        "time_since_last_non_empty_block_ms": 1_000,
        "commit_time_ms": 250,
        "da_reschedule_total": da_total,
        "txs_approved": approved,
        "txs_rejected": rejected,
        "view_changes": views,
        "governance": governance,
        "lane_commitments": lane_commitments,
        "dataspace_commitments": dataspace_commitments,
        "lane_governance": lane_governance,
        "dataspace_catalog": dataspace_catalog,
        "lane_governance_sealed_total": 1,
        "lane_governance_sealed_aliases": ["sealed-one"],
    }


def test_get_sumeragi_pacemaker_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "backoff_ms": 50,
                "rtt_floor_ms": 10,
                "jitter_ms": 5,
                "backoff_multiplier": 2,
                "rtt_floor_multiplier": 3,
                "max_backoff_ms": 120,
                "jitter_frac_permille": 25,
                "round_elapsed_ms": 40,
                "view_timeout_target_ms": 90,
                "view_timeout_remaining_ms": 12,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    pacemaker = client.get_sumeragi_pacemaker()

    assert pacemaker.backoff_ms == 50
    assert pacemaker.view_timeout_remaining_ms == 12
    assert session.calls[0]["url"].endswith("/v1/sumeragi/pacemaker")


def test_get_sumeragi_leader_parses_prf() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "leader_index": 3,
                "prf": {"height": 100, "view": 4, "epoch_seed": "ff00"},
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    leader = client.get_sumeragi_leader()

    assert leader.leader_index == 3
    assert leader.prf.epoch_seed == "ff00"
    assert session.calls[0]["url"].endswith("/v1/sumeragi/leader")


def test_retired_global_sumeragi_rbc_and_collectors_surfaces_are_absent() -> None:
    retired_methods = (
        "get_sumeragi_rbc",
        "get_sumeragi_rbc_sessions",
        "get_sumeragi_rbc_delivered",
        "sample_rbc_chunks",
        "get_sumeragi_collectors",
    )
    for name in retired_methods:
        assert not hasattr(ToriiClient, name), name

    retired_models = (
        "SumeragiRbcSnapshot",
        "SumeragiRbcSession",
        "SumeragiRbcSessionsSnapshot",
        "SumeragiRbcDeliveryStatus",
        "SumeragiCollectorEntry",
        "SumeragiCollectorsSnapshot",
        "RbcSample",
        "RbcChunkSample",
        "RbcMerkleProof",
    )
    for name in retired_models:
        assert not hasattr(client_module, name), name
        assert name not in client_module.__all__, name
        assert not hasattr(torii_module, name), name
        assert name not in torii_module.__all__, name


def test_get_sumeragi_params_parses_flags() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "block_time_ms": 2000,
                "commit_time_ms": 500,
                "max_clock_drift_ms": 20,
                "collectors_k": 3,
                "redundant_send_r": 1,
                "da_enabled": True,
                "next_mode": None,
                "mode_activation_height": 1200,
                "chain_height": 777,
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    params = client.get_sumeragi_params()

    assert params.da_enabled is True
    assert params.mode_activation_height == 1200
    assert session.calls[0]["url"].endswith("/v1/sumeragi/params")


def test_get_sumeragi_bls_keys_parses_map() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ed01": "ff00",
                "ed02": None,
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    mapping = client.get_sumeragi_bls_keys()

    assert mapping["ed01"] == "ff00"
    assert mapping["ed02"] is None
    assert session.calls[0]["url"].endswith("/v1/sumeragi/bls-keys")


def test_get_sumeragi_evidence_count_returns_int() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"count": 42}))
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    count = client.get_sumeragi_evidence_count()

    assert count == 42
    assert session.calls[0]["url"].endswith("/v1/sumeragi/evidence/count")


def _sumeragi_evidence_common(*, admitted: Optional[int] = None) -> Dict[str, Any]:
    return {
        "recorded_height": 40,
        "recorded_view": 2,
        "recorded_ms": 1_700_000_000_000,
        "consensus_admitted_height": admitted,
    }


def _sumeragi_v2_equivocation_record(
    *, evidence_class: str = "phase_vote"
) -> Dict[str, Any]:
    return {
        "kind": "SumeragiV2Equivocation",
        "class": evidence_class,
        "height": 31,
        "view": 4,
        "epoch": 2,
        "signer": 3,
        "context_id": "11" * 32,
        "artifact_hash_1": "22" * 32,
        "artifact_hash_2": "33" * 32,
        **_sumeragi_evidence_common(admitted=41),
    }


def _sumeragi_censorship_record() -> Dict[str, Any]:
    return {
        "kind": "Censorship",
        "tx_hash": "44" * 32,
        "receipt_count": 2,
        "signers": ["alice@test", "bob@test"],
        "submitted_at_height_min": 20,
        "submitted_at_height_max": 22,
        **_sumeragi_evidence_common(),
    }


@pytest.mark.parametrize("evidence_class", ["proposal", "phase_vote", "timeout_vote"])
def test_sumeragi_v2_equivocation_accepts_exact_classes(evidence_class: str) -> None:
    parsed = ToriiClient._parse_sumeragi_evidence_record(
        _sumeragi_v2_equivocation_record(evidence_class=evidence_class),
        context="evidence",
    )

    assert isinstance(parsed, client_module.SumeragiV2EquivocationEvidenceRecord)
    assert parsed.class_ == evidence_class


def test_sumeragi_evidence_rejects_unknown_record_kind() -> None:
    record = {"kind": "UnknownEvidence", **_sumeragi_evidence_common()}

    with pytest.raises(RuntimeError, match=r"kind must be one of"):
        ToriiClient._parse_sumeragi_evidence_record(record, context="evidence")


@pytest.mark.parametrize(
    "alias",
    [
        "min_height",
        "max_height",
        "minHeight",
        "maxHeight",
        "submittedAtHeightMin",
        "submittedAtHeightMax",
    ],
)
def test_sumeragi_censorship_rejects_retired_height_aliases(alias: str) -> None:
    record = _sumeragi_censorship_record()
    record[alias] = 20

    with pytest.raises(RuntimeError, match=rf"unexpected {alias}"):
        ToriiClient._parse_sumeragi_evidence_record(record, context="evidence")


@pytest.mark.parametrize(
    ("field", "value", "match"),
    [
        ("class", "Prepare", r"class must be one of"),
        ("signer", "3", r"signer must be a non-negative JSON integer"),
        ("signer", True, r"signer must be a non-negative JSON integer"),
        ("signer", 0x1_0000_0000, r"signer must be <= 4294967295"),
        ("context_id", "AA" * 32, r"exact lowercase 32-byte hex"),
        ("artifact_hash_2", "22" * 32, r"distinct artifacts"),
    ],
)
def test_sumeragi_v2_equivocation_rejects_noncanonical_fields(
    field: str, value: Any, match: str
) -> None:
    record = _sumeragi_v2_equivocation_record()
    record[field] = value

    with pytest.raises(RuntimeError, match=match):
        ToriiClient._parse_sumeragi_evidence_record(record, context="evidence")


@pytest.mark.parametrize(("field", "match"), [("context_id", "missing context_id")])
def test_sumeragi_v2_equivocation_rejects_missing_fields(
    field: str, match: str
) -> None:
    record = _sumeragi_v2_equivocation_record()
    del record[field]

    with pytest.raises(RuntimeError, match=match):
        ToriiClient._parse_sumeragi_evidence_record(record, context="evidence")


@pytest.mark.parametrize(
    ("receipt_count", "signers", "height_min", "height_max", "match"),
    [
        (2, ["alice@test"], 20, 22, r"receipt_count must equal len\(signers\)"),
        (2, ["alice@test", "bob@test"], 23, 22, r"submitted_at_height_min"),
    ],
)
def test_sumeragi_censorship_rejects_inconsistent_receipt_metadata(
    receipt_count: int,
    signers: List[str],
    height_min: int,
    height_max: int,
    match: str,
) -> None:
    record = _sumeragi_censorship_record()
    record.update(
        receipt_count=receipt_count,
        signers=signers,
        submitted_at_height_min=height_min,
        submitted_at_height_max=height_max,
    )

    with pytest.raises(RuntimeError, match=match):
        ToriiClient._parse_sumeragi_evidence_record(record, context="evidence")


def test_list_sumeragi_evidence_validates_limit() -> None:
    client = ToriiClient("http://node.test")

    try:
        client.list_sumeragi_evidence(limit=2000)
    except RuntimeError as exc:
        assert "limit must be <= 1000" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for oversized limit")


def test_confidential_gas_schedule_has_no_runtime_setter() -> None:
    assert not hasattr(ToriiClient, "set_confidential_gas_schedule")


def test_configuration_update_rejects_confidential_gas_before_request() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="confidential_gas is read-only"):
        client.update_configuration(
            {
                "logger": {"level": "INFO", "filter": None},
                "confidential_gas": {
                    "proof_base": 1,
                    "per_public_input": 2,
                    "per_proof_byte": 3,
                    "per_nullifier": 4,
                    "per_commitment": 5,
                },
            }
        )

    assert session.calls == []


def test_get_time_now_parses_snapshot_alt_values() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "now": 123456789,
                "offset_ms": -5,
                "confidence_ms": 42,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_time_now()

    assert snapshot.now_ms == 123456789
    assert snapshot.offset_ms == -5
    assert snapshot.confidence_ms == 42


def test_get_time_status_parses_diagnostics() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "peers": 2,
                "samples": [
                    {"peer": "peer-a", "last_offset_ms": 1, "last_rtt_ms": 10, "count": 5},
                    {"peer": "peer-b", "last_offset_ms": -2, "last_rtt_ms": 15, "count": 7},
                ],
                "rtt": {
                    "buckets": [{"le": 25, "count": 3}, {"le": 50, "count": 4}],
                    "sum_ms": 28,
                    "count": 9,
                },
                "note": "NTS running",
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    status = client.get_time_status()

    assert status.peers == 2
    assert len(status.samples) == 2
    assert status.samples[0].peer == "peer-a"
    assert status.rtt_buckets[1].upper_bound_ms == 50
    assert status.rtt_sum_ms == 28
    assert status.note == "NTS running"


def test_finalize_referendum_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [
                    {"wire_id": "FinalizeReferendum", "payload_hex": "AA"}
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    proposal_id = "a" * 64
    draft = client.finalize_referendum(
        referendum_id=proposal_id,
        proposal_id=proposal_id,
    )

    assert draft.ok is True
    assert len(draft.tx_instructions) == 1
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/gov/finalize")
    assert json.loads(call["data"]) == {
        "referendum_id": proposal_id,
        "proposal_id": proposal_id,
    }


def test_enact_proposal_supports_preimage_and_window() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [{"wire_id": "EnactReferendum", "payload_hex": "BB"}],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    draft = client.enact_proposal(
        canonical_auth=_governance_auth(),
        proposal_id="b" * 64,
        preimage_hash="c" * 64,
        window=(10, 20),
    )

    assert draft.ok is True
    assert draft.tx_instructions[0].wire_id == "EnactReferendum"
    call = session.calls[0]
    assert call["url"].endswith("/v1/gov/enact")
    assert json.loads(call["data"]) == {
        "proposal_id": "b" * 64,
        "preimage_hash": "c" * 64,
        "window": {"lower": 10, "upper": 20},
    }


def test_connect_app_registry_and_policy_helpers() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "app_id": "demo.wallet",
                        "display_name": "Demo Wallet",
                        "namespaces": ["wallets"],
                        "metadata": {"category": "wallet"},
                        "policy": {"relay_enabled": True},
                    }
                ],
                "total": 1,
                "next_cursor": "cursor-1",
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "policy": {
                    "relay_enabled": False,
                    "ws_max_sessions": 16,
                    "heartbeat_interval_ms": 15000,
                }
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "policy": {
                    "relay_enabled": True,
                    "heartbeat_interval_ms": 12000,
                }
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_connect_apps(limit=5, cursor="start")
    assert page.total == 1
    assert page.items[0].app_id == "demo.wallet"
    assert page.next_cursor == "cursor-1"

    policy = client.get_connect_app_policy()
    assert policy.relay_enabled is False
    assert policy.heartbeat_interval_ms == 15000

    updated = client.update_connect_app_policy({"relay_enabled": True, "heartbeat_interval_ms": 12000})
    assert updated.relay_enabled is True
    assert updated.heartbeat_interval_ms == 12000
    assert json.loads(session.calls[2]["data"]) == {
        "relay_enabled": True,
        "heartbeat_interval_ms": 12000,
    }


def test_iterate_connect_apps_pages_and_limit() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {"app_id": "demo.wallet", "namespaces": [], "metadata": {}, "policy": {}},
                    {"app_id": "demo.market", "namespaces": [], "metadata": {}, "policy": {}},
                ],
                "next_cursor": "c2",
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {"app_id": "demo.bridge", "namespaces": [], "metadata": {}, "policy": {}},
                ],
                "next_cursor": None,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    apps = list(client.iterate_connect_apps(limit=2))

    assert [app.app_id for app in apps] == ["demo.wallet", "demo.market"]
    # Only the first page is fetched because limit was satisfied.
    assert len(session.calls) == 1
    assert session.calls[0]["params"]["limit"] == 2


def test_iterate_connect_apps_consumes_all_pages_when_unbounded() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {"app_id": "app-1", "namespaces": [], "metadata": {}, "policy": {}},
                ],
                "next_cursor": "c2",
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {"app_id": "app-2", "namespaces": [], "metadata": {}, "policy": {}},
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    apps = list(client.iterate_connect_apps(page_size=1))

    assert [app.app_id for app in apps] == ["app-1", "app-2"]
    assert len(session.calls) == 2
    assert session.calls[0]["params"]["limit"] == 1
    assert session.calls[1]["params"]["cursor"] == "c2"
def test_connect_admission_manifest_helpers() -> None:
    session = RecordingSession()
    manifest_payload = {
        "version": 2,
        "manifest_hash": "abcd",
        "entries": [
            {
                "app_id": "demo.wallet",
                "namespaces": ["wallets"],
                "metadata": {"region": "global"},
                "policy": {"relay_enabled": True},
            }
        ],
    }
    session.queue(StubResponse(payload=manifest_payload))
    session.queue(StubResponse(payload=manifest_payload))
    client = ToriiClient("http://node.test", session=session)

    manifest = client.get_connect_admission_manifest()
    assert manifest.version == 2
    assert manifest.entries[0].namespaces == ["wallets"]

    updated = client.set_connect_admission_manifest(manifest_payload)
    assert updated.manifest_hash == "abcd"
    put_call = session.calls[1]
    assert put_call["method"] == "PUT"
    assert json.loads(put_call["data"]) == manifest_payload


def test_trigger_listing_and_lookup_roundtrip() -> None:
    session = RecordingSession()
    list_payload: Dict[str, Any] = {
        "items": [
            {
                "id": "daily-airdrop",
                "action": {"Mint": {"params": {"asset_id": CANONICAL_ASSET_ID}}},
                "metadata": {"cron": "0 0 * * *"},
            }
        ],
        "total": 1,
    }
    session.queue(StubResponse(payload=list_payload))
    session.queue(StubResponse(payload=list_payload["items"][0]))
    session.queue(StubResponse(status_code=404))
    client = ToriiClient("http://node.test", session=session)

    page = client.list_triggers(namespace="core", authority=CANONICAL_OWNER, limit=5, offset=10)
    trigger = client.get_trigger("daily-airdrop")
    missing = client.get_trigger("unknown-trigger")

    assert page.total == 1
    assert page.items[0].id == "daily-airdrop"
    assert trigger is not None and trigger.metadata["cron"] == "0 0 * * *"
    assert missing is None

    assert session.calls[0]["params"] == {
        "namespace": "core",
        "authority": CANONICAL_OWNER,
        "limit": 5,
        "offset": 10,
    }
    assert session.calls[0]["url"].endswith("/v1/triggers")
    assert session.calls[1]["url"].endswith("/v1/triggers/daily-airdrop")
    assert session.calls[2]["url"].endswith("/v1/triggers/unknown-trigger")


def test_trigger_registration_deletion_and_query() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=201, payload={"ok": True}))
    session.queue(StubResponse(status_code=204))
    session.queue(StubResponse(status_code=404))
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "id": "hook",
                        "action": {"Grant": {"params": {}}},
                        "metadata": {},
                    }
                ],
                "total": 1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.register_trigger({"id": "hook", "action": {"Grant": {}}})
    deleted = client.delete_trigger("hook")
    deleted_missing = client.delete_trigger("missing")
    page = client.query_triggers(filter={"id": {"$eq": "hook"}}, fetch_size=1, query_name="named_query")

    assert result["ok"] is True
    assert deleted is True
    assert deleted_missing is False
    assert page.total == 1
    assert page.items[0].id == "hook"

    post_call = session.calls[0]
    assert post_call["method"] == "POST"
    assert post_call["url"].endswith("/v1/triggers")
    assert json.loads(post_call["data"].decode("utf-8")) == {"id": "hook", "action": {"Grant": {}}}

    query_call = session.calls[-1]
    assert query_call["url"].endswith("/v1/triggers/query")
    assert json.loads(query_call["data"].decode("utf-8")) == {
        "filter": {"id": {"$eq": "hook"}},
        "fetch_size": 1,
        "query_name": "named_query",
    }


def test_offline_public_request_annotations_are_closed_first_release_types() -> None:
    assert get_type_hints(ToriiClient.submit_kagemusha_top_up)["request"] is KagemushaTopUpRequestV4
    assert get_type_hints(ToriiClient.submit_kagemusha_redeem)["request"] is KagemushaRedeemRequestV4
    assert get_args(OfflineAssetScale) == tuple(range(29))
    assert "next_zero_leaf_index" in (
        client_module.OfflineRecursiveSpendStatementJson.__required_keys__
    )
    assert "network_id" in client_module.OfflineSpendableNoteJson.__required_keys__
    assert "chain_id" not in client_module.OfflineSpendableNoteJson.__required_keys__
    assert tuple(client_module.OfflineErrorEnvelope.__dataclass_fields__) == ("code", "message")
    for retired in (
        "OfflineQueueErrorDetails",
        "OfflineAxtErrorDetails",
        "OfflineErrorDetails",
    ):
        assert not hasattr(client_module, retired), retired
        assert retired not in client_module.__all__, retired
        assert not hasattr(torii_module, retired), retired
        assert retired not in torii_module.__all__, retired
    for code, message in (
        ("future_rejection", "rejected"),
        ("offline_operation_rejected", ""),
        ("offline_operation_rejected", "\U0001f600" * 1025),
    ):
        with pytest.raises(RuntimeError):
            client_module.OfflineErrorEnvelope(code=code, message=message)


def test_offline_finality_execution_commitment_requires_executed_wire_identity() -> None:
    payload = {
        "parent_state_root": _canonical_hash(0x91),
        "post_state_root": _canonical_hash(0x92),
        "ordinary_writes_root": _canonical_hash(0x93),
        "topup_anchor_count": 0,
        "native_amx_application_manifest_version": 1,
        "native_amx_application_manifest_root": _NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT,
        "native_amx_application_manifest_count": 0,
        "lane_finality_manifest": None,
        "merge_carrier": None,
        "executed_block_wire_len": 321,
        "executed_block_wire_hash": _canonical_hash(0x94),
    }
    commitment = client_module._offline_top_up_finality_execution_commitment(
        payload,
        "test.execution_commitment",
        require_topup=False,
    )
    assert commitment.executed_block_wire_len == payload["executed_block_wire_len"]
    assert commitment.executed_block_wire_hash == payload["executed_block_wire_hash"]

    payload["native_amx_application_manifest_root"] = _canonical_hash(0x95)
    payload["native_amx_application_manifest_count"] = 1
    nonempty_commitment = (
        client_module._offline_top_up_finality_execution_commitment(
            payload,
            "test.execution_commitment",
            require_topup=False,
        )
    )
    assert nonempty_commitment.native_amx_application_manifest_count == 1

    for invalid in (None, True, 0, -1, 1 << 64, "321"):
        invalid_payload = dict(payload)
        invalid_payload["executed_block_wire_len"] = invalid
        with pytest.raises(RuntimeError, match="executed_block_wire_len"):
            client_module._offline_top_up_finality_execution_commitment(
                invalid_payload,
                "test.execution_commitment",
                require_topup=False,
            )

    missing_len = dict(payload)
    del missing_len["executed_block_wire_len"]
    with pytest.raises(RuntimeError, match="executed_block_wire_len"):
        client_module._offline_top_up_finality_execution_commitment(
            missing_len,
            "test.execution_commitment",
            require_topup=False,
        )

    del payload["executed_block_wire_hash"]
    with pytest.raises(RuntimeError, match="executed_block_wire_hash"):
        client_module._offline_top_up_finality_execution_commitment(
            payload,
            "test.execution_commitment",
            require_topup=False,
        )


def test_offline_finality_execution_commitment_requires_exact_merge_carrier() -> None:
    def payload() -> Dict[str, Any]:
        return {
            "parent_state_root": _canonical_hash(0x91),
            "post_state_root": _canonical_hash(0x92),
            "ordinary_writes_root": _canonical_hash(0x93),
            "topup_anchor_count": 0,
            "native_amx_application_manifest_version": 1,
            "native_amx_application_manifest_root": _NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT,
            "native_amx_application_manifest_count": 0,
            "lane_finality_manifest": None,
            "merge_carrier": None,
            "executed_block_wire_len": 321,
            "executed_block_wire_hash": _canonical_hash(0x94),
        }

    parsed = client_module._offline_top_up_finality_execution_commitment(
        payload(), "test.execution_commitment", require_topup=False
    )
    assert parsed.merge_carrier is None

    carrier_payload = payload()
    carrier_payload["merge_carrier"] = {
        "version": 1,
        "entry_hash": _canonical_hash(0x95),
    }
    parsed = client_module._offline_top_up_finality_execution_commitment(
        carrier_payload, "test.execution_commitment", require_topup=False
    )
    assert parsed.merge_carrier is not None
    assert parsed.merge_carrier.entry_hash == _canonical_hash(0x95)

    invalid_payloads = []
    missing = payload()
    del missing["merge_carrier"]
    invalid_payloads.append(missing)
    malformed = payload()
    malformed["merge_carrier"] = []
    invalid_payloads.append(malformed)
    wrong_version = payload()
    wrong_version["merge_carrier"] = {
        "version": 2,
        "entry_hash": _canonical_hash(0x95),
    }
    invalid_payloads.append(wrong_version)
    missing_version = payload()
    missing_version["merge_carrier"] = {
        "entry_hash": _canonical_hash(0x95),
    }
    invalid_payloads.append(missing_version)
    missing_entry_hash = payload()
    missing_entry_hash["merge_carrier"] = {"version": 1}
    invalid_payloads.append(missing_entry_hash)
    bad_hash = payload()
    bad_hash["merge_carrier"] = {"version": 1, "entry_hash": "bad"}
    invalid_payloads.append(bad_hash)
    unknown = payload()
    unknown["merge_carrier"] = {
        "version": 1,
        "entry_hash": _canonical_hash(0x95),
        "future": 1,
    }
    invalid_payloads.append(unknown)

    for invalid in invalid_payloads:
        with pytest.raises(RuntimeError):
            client_module._offline_top_up_finality_execution_commitment(
                invalid,
                "test.execution_commitment",
                require_topup=False,
            )


@pytest.mark.parametrize(
    ("mutate", "error"),
    [
        (
            lambda payload: payload.update(
                native_amx_application_manifest_version=2
            ),
            "native_amx_application_manifest_version must equal 1",
        ),
        (
            lambda payload: payload.update(
                native_amx_application_manifest_count=1025
            ),
            "native_amx_application_manifest_count",
        ),
        (
            lambda payload: payload.update(
                native_amx_application_manifest_root=_canonical_hash(0x95)
            ),
            "must be zero exactly for the canonical empty root",
        ),
        (
            lambda payload: payload.update(
                native_amx_application_manifest_count=1
            ),
            "must be zero exactly for the canonical empty root",
        ),
    ],
)
def test_offline_finality_execution_commitment_rejects_invalid_native_manifest(
    mutate, error: str
) -> None:
    payload = {
        "parent_state_root": _canonical_hash(0x91),
        "post_state_root": _canonical_hash(0x92),
        "ordinary_writes_root": _canonical_hash(0x93),
        "topup_anchor_count": 0,
        "native_amx_application_manifest_version": 1,
        "native_amx_application_manifest_root": _NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT,
        "native_amx_application_manifest_count": 0,
        "lane_finality_manifest": None,
        "merge_carrier": None,
        "executed_block_wire_len": 321,
        "executed_block_wire_hash": _canonical_hash(0x94),
    }
    mutate(payload)

    with pytest.raises(RuntimeError, match=error):
        client_module._offline_top_up_finality_execution_commitment(
            payload,
            "test.execution_commitment",
            require_topup=False,
        )


def test_get_offline_capability_is_asset_neutral_and_exact() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=_offline_capability_payload()))
    client = ToriiClient("http://node.test", session=session)

    capability = client.get_offline_capability()

    assert isinstance(capability, OfflineStatus)
    assert capability.mandatory is False
    assert capability.cash_handoff_capability == "cash_handoff_v1"
    assert capability.required_bridge_abi_version == 22
    assert capability.max_hops == 8
    assert capability.ready is False
    assert capability.assets == ()
    assert [blocker.code for blocker in capability.blockers] == [
        "offline_cash_authenticated_release_unavailable",
        "offline_cash_eligible_asset_unavailable",
        "offline_cash_proof_backend_unavailable",
    ]
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/offline/readiness")
    assert call["params"] == {}
    assert call["headers"]["Accept"] == "application/json"
    assert not hasattr(ToriiClient, "get_kagemusha_readiness")


def test_get_offline_capability_rejects_non_universal_claims() -> None:
    payloads = [
        _offline_capability_payload(mandatory=True),
        _offline_capability_payload(cash_handoff_capability="cash_handoff_v2"),
        _offline_capability_payload(required_bridge_abi_version=20),
        _offline_capability_payload(max_hops=7),
        _offline_capability_payload(ready="false"),
        _offline_capability_payload(ready=True),
        _offline_capability_payload(blockers=[]),
        _offline_capability_payload(assets=[{"asset_definition_id": "asset-specific"}]),
        _offline_capability_payload(
            blockers=[
                {"code": "backend_gate", "message": "blocked"},
                {"code": "backend_gate", "message": "duplicate"},
            ]
        ),
        _offline_capability_payload(
            blockers=list(reversed(_offline_capability_payload()["blockers"]))
        ),
        _offline_capability_payload(unexpected_field=True),
    ]
    missing_field = _offline_capability_payload()
    missing_field.pop("cash_handoff_capability")
    payloads.append(missing_field)

    for payload in payloads:
        session = RecordingSession()
        session.queue(StubResponse(payload=payload))
        with pytest.raises(RuntimeError, match="offline capability response"):
            ToriiClient("http://node.test", session=session).get_offline_capability()


def test_offline_asset_definition_id_validation_matches_canonical_rust_codec() -> None:
    assert client_module._offline_canonical_asset_definition_id(
        CANONICAL_ASSET_DEFINITION_ID,
        "asset_definition_id",
    ) == CANONICAL_ASSET_DEFINITION_ID

    for invalid in (
        CHECKSUM_INVALID_ASSET_DEFINITION_ID,
        CHECKSUM_VALID_NON_UUID_V4_ASSET_DEFINITION_ID,
        CHECKSUM_VALID_NON_RFC4122_ASSET_DEFINITION_ID,
    ):
        with pytest.raises(RuntimeError, match="checksummed UUIDv4"):
            client_module._offline_canonical_asset_definition_id(
                invalid,
                "asset_definition_id",
            )


def test_submit_kagemusha_top_up_sends_exact_norito_and_idempotency_key() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(),
            headers={"Location": OFFLINE_STATUS_URI, "Retry-After": "1"},
        )
    )
    client = _offline_bound_client(session)

    request = _offline_top_up_request()
    reference = client.submit_kagemusha_top_up(request)

    assert reference.operation_id == OFFLINE_OPERATION_ID
    assert reference.kind.kind == "top_up"
    assert reference.state.state == "pending"
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/offline/top-up")
    assert call["headers"] == {
        "Accept": "application/json",
        "Content-Type": "application/x-norito",
        "Idempotency-Key": OFFLINE_OPERATION_ID,
    }
    assert call["data"] is request.norito


def test_submit_kagemusha_redeem_uses_only_the_final_route() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(kind={"kind": "redeem", "value": None}),
            headers={"Location": OFFLINE_STATUS_URI, "Retry-After": "1"},
        )
    )
    client = _offline_bound_client(session)

    request = _offline_redeem_request()
    reference = client.submit_kagemusha_redeem(request)

    assert reference.kind.kind == "redeem"
    assert session.calls[0]["url"].endswith("/v1/offline/redeem")
    assert session.calls[0]["headers"]["Content-Type"] == "application/x-norito"
    assert session.calls[0]["data"] is request.norito


def test_kagemusha_command_validation_rejects_noncanonical_inputs_before_network() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)
    for malformed in ({}, b"request", "request", bytearray(b"request"), memoryview(b"request")):
        with pytest.raises(TypeError):
            client.submit_kagemusha_top_up(malformed)  # type: ignore[arg-type]
        with pytest.raises(TypeError):
            client.submit_kagemusha_redeem(malformed)  # type: ignore[arg-type]

    for request_type, maximum_bytes, framed, wrong_schema in (
        (
            KagemushaTopUpRequestV4,
            512 * 1024,
            OFFLINE_TOP_UP_REQUEST_FRAME,
            OFFLINE_REDEEM_REQUEST_FRAME,
        ),
        (
            KagemushaRedeemRequestV4,
            48 * 1024 * 1024,
            OFFLINE_REDEEM_REQUEST_FRAME,
            OFFLINE_TOP_UP_REQUEST_FRAME,
        ),
    ):
        with pytest.raises(ValueError, match="must not be empty"):
            request_type(norito=b"", operation_id=OFFLINE_OPERATION_ID)
        with pytest.raises(ValueError, match="exceeds"):
            request_type(norito=b"x" * (maximum_bytes + 1), operation_id=OFFLINE_OPERATION_ID)
        for norito in (bytearray(b"x"), memoryview(b"x"), "x"):
            with pytest.raises(TypeError, match="immutable bytes"):
                request_type(norito=norito, operation_id=OFFLINE_OPERATION_ID)  # type: ignore[arg-type]
        for norito in (b"x", b"NRT0" + bytes(35), b"XXXX" + bytes(36), wrong_schema):
            with pytest.raises(ValueError, match="canonical compact"):
                request_type(norito=norito, operation_id=OFFLINE_OPERATION_ID)
        corrupt_checksum = bytearray(framed)
        corrupt_checksum[-1] ^= 1
        with pytest.raises(ValueError, match="canonical compact"):
            request_type(norito=bytes(corrupt_checksum), operation_id=OFFLINE_OPERATION_ID)
        noncanonical_flags = bytearray(framed)
        noncanonical_flags[39] = 0
        with pytest.raises(ValueError, match="canonical compact"):
            request_type(norito=bytes(noncanonical_flags), operation_id=OFFLINE_OPERATION_ID)
        for operation_id in (
            "0" * 64,
            "11" * 31,
            "11" * 33,
            "AA" * 32,
            "gg" * 32,
            f" {OFFLINE_OPERATION_ID}",
        ):
            with pytest.raises(RuntimeError, match="operation_id"):
                request_type(norito=framed, operation_id=operation_id)
    assert session.calls == []


def test_kagemusha_request_archives_derive_signed_public_bindings() -> None:
    top_up = KagemushaTopUpRequestV4(OFFLINE_TOP_UP_REQUEST_FRAME)
    redeem = KagemushaRedeemRequestV4(OFFLINE_REDEEM_REQUEST_FRAME)

    for request in (top_up, redeem):
        assert request.operation_id == OFFLINE_OPERATION_ID
        assert request.issued_at_ms == OFFLINE_SUBMITTED_AT_MS
        assert request.network_id == OFFLINE_NETWORK_ID

    with pytest.raises(ValueError, match="signed Norito request body"):
        KagemushaTopUpRequestV4(
            OFFLINE_TOP_UP_REQUEST_FRAME,
            operation_id="33" * 32,
        )
    with pytest.raises(ValueError, match="operation ids must match"):
        KagemushaRedeemRequestV4(
            _offline_norito_request_frame(
                "redeem",
                authorization_operation_id="33" * 32,
            )
        )
    with pytest.raises(ValueError, match="issued_at_ms must be at least 1"):
        KagemushaTopUpRequestV4(
            _offline_norito_request_frame("top_up", issued_at_ms=0)
        )
    with pytest.raises(ValueError, match="version must be exactly 4"):
        KagemushaTopUpRequestV4(
            _offline_norito_request_frame("top_up", version=3)
        )

    payload = OFFLINE_TOP_UP_REQUEST_FRAME[48:]
    with pytest.raises(ValueError, match="length is overlong"):
        KagemushaTopUpRequestV4(
            _offline_norito_frame("top_up", b"\x82\x00" + payload[1:])
        )
    with pytest.raises(ValueError, match="trailing or unknown bytes"):
        KagemushaTopUpRequestV4(
            _offline_norito_frame("top_up", payload + b"\x00")
        )


@pytest.mark.parametrize(
    ("kind", "request_type"),
    (
        ("top_up", KagemushaTopUpRequestV4),
        ("redeem", KagemushaRedeemRequestV4),
    ),
)
def test_kagemusha_submission_binds_signed_request_network_before_dispatch(
    kind: str,
    request_type: type,
) -> None:
    foreign_request = request_type(
        _offline_norito_request_frame(kind, network_id=OFFLINE_OTHER_NETWORK_ID)
    )
    foreign_session = RecordingSession()
    foreign_client = _offline_bound_client(foreign_session)
    submit = (
        foreign_client.submit_kagemusha_top_up
        if kind == "top_up"
        else foreign_client.submit_kagemusha_redeem
    )
    with pytest.raises(ValueError, match="does not match the local_signing_context"):
        submit(foreign_request)
    assert foreign_session.calls == []

    missing_context_session = RecordingSession()
    missing_context_client = ToriiClient(
        "http://node.test",
        session=missing_context_session,
    )
    local_request = request_type(_offline_norito_request_frame(kind))
    submit_without_context = (
        missing_context_client.submit_kagemusha_top_up
        if kind == "top_up"
        else missing_context_client.submit_kagemusha_redeem
    )
    with pytest.raises(ValueError, match="requires an immutable local_signing_context"):
        submit_without_context(local_request)
    assert missing_context_session.calls == []


def test_kagemusha_acceptance_binds_signed_request_time() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(
                submitted_at_ms=OFFLINE_SUBMITTED_AT_MS + 1
            ),
            headers={"Location": OFFLINE_STATUS_URI, "Retry-After": "1"},
        )
    )

    with pytest.raises(RuntimeError, match="does not match the signed V4 request"):
        _offline_bound_client(session).submit_kagemusha_top_up(
            KagemushaTopUpRequestV4(OFFLINE_TOP_UP_REQUEST_FRAME)
        )
    assert len(session.calls) == 1


def test_kagemusha_signing_network_bytes_format_as_the_exact_json_literal() -> None:
    assert client_module._offline_hash_literal_from_bytes(
        bytes.fromhex("91" * 32),
        "network_id",
    ) == OFFLINE_NETWORK_ID
    with pytest.raises(RuntimeError, match="marker bit"):
        client_module._offline_hash_literal_from_bytes(bytes.fromhex("92" * 32), "network_id")


def test_offline_acceptance_cross_checks_reference_and_location() -> None:
    cases = [
        (_offline_operation_reference(operation_id="33" * 32), OFFLINE_STATUS_URI),
        (
            _offline_operation_reference(kind={"kind": "redeem", "value": None}),
            OFFLINE_STATUS_URI,
        ),
        (
            _offline_operation_reference(status_uri="/v1/offline/operations/not-a-digest"),
            OFFLINE_STATUS_URI,
        ),
        (_offline_operation_reference(), f"/v1/offline/operations/{'44' * 32}"),
        (_offline_operation_reference(), None),
        (_offline_operation_reference(unexpected=True), OFFLINE_STATUS_URI),
        (_offline_operation_reference(kind={"kind": "top_up"}), OFFLINE_STATUS_URI),
        (_offline_operation_reference(state={"state": "pending"}), OFFLINE_STATUS_URI),
    ]
    for payload, location in cases:
        session = RecordingSession()
        headers = ({"Location": location} if location is not None else {}) | {
            "Retry-After": "1"
        }
        session.queue(StubResponse(status_code=202, payload=payload, headers=headers))
        client = _offline_bound_client(session)
        with pytest.raises(RuntimeError):
            client.submit_kagemusha_top_up(_offline_top_up_request())

    wrong_media_session = RecordingSession()
    wrong_media_session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(),
            headers={
                "Content-Type": "text/plain",
                "Location": OFFLINE_STATUS_URI,
                "Retry-After": "1",
            },
        )
    )
    wrong_media_client = _offline_bound_client(wrong_media_session)
    with pytest.raises(RuntimeError, match="Content-Type application/json"):
        wrong_media_client.submit_kagemusha_top_up(_offline_top_up_request())

    for retry_after in (
        None,
        "0",
        "01",
        "+1",
        "1\u0661",
        "1, 1",
        "18446744073709551616",
    ):
        session = RecordingSession()
        headers = {"Location": OFFLINE_STATUS_URI}
        if retry_after is not None:
            headers["Retry-After"] = retry_after
        session.queue(
            StubResponse(
                status_code=202,
                payload=_offline_operation_reference(),
                headers=headers,
            )
        )
        with pytest.raises(RuntimeError, match="Retry-After"):
            _offline_bound_client(session).submit_kagemusha_top_up(
                _offline_top_up_request()
            )

    maximum_session = RecordingSession()
    maximum_session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(),
            headers={
                "Location": OFFLINE_STATUS_URI,
                "Retry-After": "18446744073709551615",
            },
        )
    )
    maximum = _offline_bound_client(maximum_session).submit_kagemusha_top_up(
        _offline_top_up_request()
    )
    assert maximum.operation_id == OFFLINE_OPERATION_ID

    zero_time_session = RecordingSession()
    zero_time_session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(submitted_at_ms=0),
            headers={"Location": OFFLINE_STATUS_URI, "Retry-After": "1"},
        )
    )
    with pytest.raises(RuntimeError, match="submitted_at_ms"):
        _offline_bound_client(zero_time_session).submit_kagemusha_top_up(
            _offline_top_up_request()
        )

    class DuplicateRawHeaders:
        def __init__(self, duplicate_name: str) -> None:
            self.headers = self
            self.duplicate_name = duplicate_name.lower()

        def getlist(self, name: str) -> List[str]:
            value = OFFLINE_STATUS_URI if name.lower() == "location" else "1"
            return [value, value] if name.lower() == self.duplicate_name else [value]

    for duplicate_name in ("Location", "Retry-After"):
        duplicate_response = StubResponse(
            status_code=202,
            payload=_offline_operation_reference(),
            headers={"Location": OFFLINE_STATUS_URI, "Retry-After": "1"},
        )
        duplicate_response.raw = DuplicateRawHeaders(duplicate_name)
        duplicate_session = RecordingSession()
        duplicate_session.queue(duplicate_response)
        with pytest.raises(RuntimeError, match="exactly one"):
            _offline_bound_client(duplicate_session).submit_kagemusha_top_up(
                _offline_top_up_request()
            )


def test_offline_transaction_carriers_reject_unmarked_iroha_hashes() -> None:
    unmarked_hash = "22" * 32
    reference_session = RecordingSession()
    reference_session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(transaction_hash=unmarked_hash),
            headers={"Location": OFFLINE_STATUS_URI, "Retry-After": "1"},
        )
    )
    with pytest.raises(RuntimeError, match="marker bit"):
        _offline_bound_client(reference_session).submit_kagemusha_top_up(
            _offline_top_up_request()
        )

    statuses = [
        {
            "state": "pending",
            "value": {
                "operation_id": OFFLINE_OPERATION_ID,
                "kind": {"kind": "top_up", "value": None},
                "transaction_hash": unmarked_hash,
                "submitted_at_ms": 10,
            },
        },
        {
            "state": "applied",
            "value": {
                "operation_id": OFFLINE_OPERATION_ID,
                "result": {
                    "kind": "redeem",
                    "result": {
                        "transaction_hash": unmarked_hash,
                        "finalized_block_height": 12,
                        "server_time_ms": 13,
                    },
                },
            },
        },
        _offline_rejected_status(
            {
                "code": "offline_operation_rejected",
                "message": "rejected",
            }
        ),
    ]
    statuses[-1]["value"]["transaction_hash"] = unmarked_hash
    for payload in statuses:
        session = RecordingSession()
        session.queue(StubResponse(payload=payload))
        with pytest.raises(RuntimeError, match="marker bit"):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_get_kagemusha_operation_status_parses_all_tagged_states() -> None:
    statuses = [
        (
            {
                "state": "pending",
                "value": {
                    "operation_id": OFFLINE_OPERATION_ID,
                    "kind": {"kind": "top_up", "value": None},
                    "transaction_hash": OFFLINE_TRANSACTION_HASH,
                    "submitted_at_ms": 10,
                },
            },
            OfflinePendingOperation,
        ),
        (
            {
                "state": "applied",
                "value": {
                    "operation_id": OFFLINE_OPERATION_ID,
                    "result": {
                        "kind": "top_up",
                        "result": {
                            "transaction_hash": OFFLINE_TRANSACTION_HASH,
                            "finalized_block_height": 12,
                            "server_time_ms": 13,
                            "anchor": _offline_top_up_anchor(),
                            "finality_proof": _offline_top_up_finality_proof(),
                        },
                    },
                },
            },
            OfflineAppliedOperation,
        ),
        (
            {
                "state": "rejected",
                "value": {
                    "operation_id": OFFLINE_OPERATION_ID,
                    "kind": {"kind": "redeem", "value": None},
                    "transaction_hash": OFFLINE_TRANSACTION_HASH,
                    "error": {
                        "code": "offline_operation_rejected",
                        "message": "rejected",
                    },
                },
            },
            OfflineRejectedOperation,
        ),
    ]
    for payload, expected_type in statuses:
        session = RecordingSession()
        session.queue(StubResponse(payload=payload))
        client = ToriiClient("http://node.test", session=session)
        status = client.get_kagemusha_operation_status(OFFLINE_OPERATION_ID)
        assert isinstance(status, expected_type)
        assert status.operation_id == OFFLINE_OPERATION_ID
        assert session.calls[0]["url"].endswith(OFFLINE_STATUS_URI)


def test_kagemusha_top_up_anchor_is_closed_typed_and_cross_checked() -> None:
    unknown_anchor = _offline_top_up_anchor(unknown_member={"attacker_controlled": True})
    session = RecordingSession()
    session.queue(StubResponse(payload=_offline_applied_top_up_status(unknown_anchor)))
    with pytest.raises(RuntimeError, match="first-release contract"):
        ToriiClient(
            "http://node.test", session=session
        ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)

    exact_session = RecordingSession()
    exact_session.queue(
        StubResponse(payload=_offline_applied_top_up_status(_offline_top_up_anchor()))
    )

    status = ToriiClient(
        "http://node.test", session=exact_session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)

    assert isinstance(status, OfflineAppliedOperation)
    assert status.result.kind == "top_up"
    typed_anchor = status.result.result.anchor
    assert isinstance(typed_anchor, OfflineTopUpAnchor)
    # Kagemusha V4 promotes the finalized anchor and its authenticated artifact
    # binding atomically to the V4 wire contract.
    assert typed_anchor.version == 4
    assert typed_anchor.network_id == OFFLINE_NETWORK_ID
    assert typed_anchor.amount.scale == 4
    assert typed_anchor.shield_leaf_index == 7
    assert typed_anchor.shield_verifier_id.backend == "halo2/ipa"
    assert typed_anchor.artifact_binding.version == 4
    assert typed_anchor.artifact_binding.generation == "generation-1"
    assert typed_anchor.artifact_binding.manifest_sha256 == tuple(
        _offline_fixed_bytes(0x81)
    )
    assert typed_anchor.topup_operation_id == tuple(OFFLINE_OPERATION_BYTES)


def test_offline_top_up_finality_proof_is_closed_and_direct_typed() -> None:
    proof = _offline_top_up_finality_proof()
    session = RecordingSession()
    session.queue(
        StubResponse(payload=_offline_applied_top_up_status(finality_proof=proof))
    )

    status = ToriiClient(
        "http://node.test", session=session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)

    assert isinstance(status, OfflineAppliedOperation)
    assert status.result.kind == "top_up"
    typed_proof = status.result.result.finality_proof
    assert isinstance(typed_proof, OfflineTopUpFinalityProof)
    assert typed_proof.version == 1
    assert typed_proof.anchor.topup_operation_id == tuple(OFFLINE_OPERATION_BYTES)
    assert typed_proof.anchor.anchor_digest == tuple(_offline_fixed_bytes(0x71))
    assert typed_proof.commit_qc.height_context.protocol_version == 4
    assert (
        typed_proof.commit_qc.height_context.da_layout.encoding.encoding
        == "reed_solomon16"
    )
    assert typed_proof.commit_qc.height_context.da_layout.data_shards == 1
    assert typed_proof.commit_qc.height_context.da_layout.parity_shards == 1
    assert typed_proof.commit_qc.certificate.round.height == 12
    assert (
        typed_proof.commit_qc.certificate.proposal_round
        == typed_proof.commit_qc.certificate.round
    )
    assert typed_proof.commit_qc.height_context.snapshot_bootstrap is None
    assert typed_proof.anchor_path.leaf_count == 1


def test_offline_top_up_public_parser_types_snapshot_and_omitted_genesis_authorities() -> None:
    snapshot_proof = _offline_top_up_finality_proof()
    snapshot_context = snapshot_proof["commit_qc"]["height_context"]
    snapshot_context["parent_commit_qc"] = None
    snapshot_context["snapshot_bootstrap"] = {
        "snapshot_height": 11,
        "snapshot_block_hash": _canonical_hash(0xA6),
        "snapshot_block_creation_time_ms": 1_000,
        "snapshot_state_hash": _canonical_hash(0xA7),
    }
    snapshot_session = RecordingSession()
    snapshot_session.queue(
        StubResponse(
            payload=_offline_applied_top_up_status(finality_proof=snapshot_proof)
        )
    )

    snapshot_status = ToriiClient(
        "http://node.test", session=snapshot_session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)

    assert isinstance(snapshot_status, OfflineAppliedOperation)
    snapshot_bootstrap = (
        snapshot_status.result.result.finality_proof.commit_qc.height_context.snapshot_bootstrap
    )
    assert snapshot_bootstrap is not None
    assert snapshot_bootstrap.snapshot_height == 11
    assert snapshot_bootstrap.snapshot_block_creation_time_ms == 1_000

    genesis_anchor = _offline_top_up_anchor(finalized_height=1)
    genesis_proof = _offline_top_up_finality_proof(
        genesis_anchor,
        finalized_height=1,
    )
    genesis_context = genesis_proof["commit_qc"]["height_context"]
    for optional_field in (
        "next_epoch_snapshot",
        "parent_commit_qc",
        "snapshot_bootstrap",
    ):
        genesis_context.pop(optional_field)
    genesis_session = RecordingSession()
    genesis_session.queue(
        StubResponse(
            payload=_offline_applied_top_up_status(
                genesis_anchor,
                finalized_block_height=1,
                finality_proof=genesis_proof,
            )
        )
    )

    genesis_status = ToriiClient(
        "http://node.test", session=genesis_session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)

    assert isinstance(genesis_status, OfflineAppliedOperation)
    genesis_height_context = (
        genesis_status.result.result.finality_proof.commit_qc.height_context
    )
    assert genesis_height_context.next_epoch_snapshot is None
    assert genesis_height_context.parent_commit_qc is None
    assert genesis_height_context.snapshot_bootstrap is None


def test_offline_top_up_public_parser_rejects_noncanonical_da_layouts() -> None:
    invalid_proofs = []

    missing_encoding = copy.deepcopy(_offline_top_up_finality_proof())
    missing_encoding["commit_qc"]["height_context"]["da_layout"].pop("encoding")
    invalid_proofs.append(
        (missing_encoding, r"da_layout\.encoding is required")
    )

    missing_variant = copy.deepcopy(_offline_top_up_finality_proof())
    missing_variant["commit_qc"]["height_context"]["da_layout"]["encoding"] = {
        "details": None,
    }
    invalid_proofs.append(
        (missing_variant, r"da_layout\.encoding\.encoding is required")
    )

    for retired_or_unknown in ("plain", "rs16"):
        invalid_encoding = copy.deepcopy(_offline_top_up_finality_proof())
        layout = invalid_encoding["commit_qc"]["height_context"]["da_layout"]
        layout["encoding"]["encoding"] = retired_or_unknown
        invalid_proofs.append(
            (
                invalid_encoding,
                r"da_layout\.encoding\.encoding must be reed_solomon16",
            )
        )

    for field in ("data_shards", "parity_shards"):
        zero_shard = copy.deepcopy(_offline_top_up_finality_proof())
        zero_shard["commit_qc"]["height_context"]["da_layout"][field] = 0
        invalid_proofs.append(
            (zero_shard, rf"da_layout\.{field} must be between 1 and 65535")
        )

    for proof, expected_error in invalid_proofs:
        session = RecordingSession()
        session.queue(
            StubResponse(
                payload=_offline_applied_top_up_status(finality_proof=proof)
            )
        )
        with pytest.raises(RuntimeError, match=expected_error):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_top_up_public_parser_rejects_unknown_finality_projection_fields() -> None:
    def next_epoch_snapshot() -> Dict[str, Any]:
        return {
            "epoch": 1,
            "epoch_end_height": 100,
            "mode": {"mode": "permissioned", "details": None},
            "roster": [{"validator": _NATIVE_AMX_VALIDATOR_SET[0], "power": 1}],
            "validator_set_pops": [[1] * 96],
            "quorum": {"min_signers": 1, "total_power": 1},
            "leader_seed": _offline_fixed_bytes(0xA5),
        }

    invalid_proofs: List[tuple[Dict[str, Any], str]] = []

    def reject_unknown(
        proof: Dict[str, Any], target: Dict[str, Any], field: str, context: str
    ) -> None:
        target[field] = "retired-extension"
        invalid_proofs.append((proof, rf"{context}\.{field} is not part"))

    proof = _offline_top_up_finality_proof()
    height_context = proof["commit_qc"]["height_context"]
    reject_unknown(proof, height_context, "future_context", r"height_context")

    proof = _offline_top_up_finality_proof()
    certificate = proof["commit_qc"]["certificate"]
    reject_unknown(proof, certificate, "future_certificate", r"certificate")

    for component in ("round", "proposal_round", "phase", "subject"):
        proof = _offline_top_up_finality_proof()
        nested = proof["commit_qc"]["certificate"][component]
        reject_unknown(proof, nested, f"future_{component}", rf"certificate\.{component}")

    proof = _offline_top_up_finality_proof()
    execution = proof["commit_qc"]["certificate"]["execution_commitment"]
    reject_unknown(
        proof,
        execution,
        "future_execution",
        r"certificate\.execution_commitment",
    )

    proof = _offline_top_up_finality_proof()
    mode = proof["commit_qc"]["height_context"]["mode"]
    reject_unknown(proof, mode, "future_mode", r"height_context\.mode")

    proof = _offline_top_up_finality_proof()
    reject_unknown(proof, proof["anchor_path"], "future_path", r"anchor_path")

    proof = _offline_top_up_finality_proof()
    height_context = proof["commit_qc"]["height_context"]
    height_context["epoch_end_height"] = 12
    height_context["next_epoch_snapshot"] = next_epoch_snapshot()
    reject_unknown(
        proof,
        height_context["next_epoch_snapshot"],
        "future_snapshot",
        r"next_epoch_snapshot",
    )

    proof = _offline_top_up_finality_proof()
    height_context = proof["commit_qc"]["height_context"]
    height_context["epoch_end_height"] = 12
    height_context["next_epoch_snapshot"] = next_epoch_snapshot()
    reject_unknown(
        proof,
        height_context["next_epoch_snapshot"]["roster"][0],
        "future_validator",
        r"roster\[0\]",
    )

    proof = _offline_top_up_finality_proof()
    height_context = proof["commit_qc"]["height_context"]
    height_context["epoch_end_height"] = 12
    height_context["next_epoch_snapshot"] = next_epoch_snapshot()
    reject_unknown(
        proof,
        height_context["next_epoch_snapshot"]["quorum"],
        "future_quorum",
        r"next_epoch_snapshot\.quorum",
    )

    proof = _offline_top_up_finality_proof()
    height_context = proof["commit_qc"]["height_context"]
    height_context["parent_commit_qc"] = None
    height_context["snapshot_bootstrap"] = {
        "snapshot_height": 11,
        "snapshot_block_hash": _canonical_hash(0xA6),
        "snapshot_block_creation_time_ms": 1_000,
        "snapshot_state_hash": _canonical_hash(0xA7),
    }
    reject_unknown(
        proof,
        height_context["snapshot_bootstrap"],
        "future_bootstrap",
        r"snapshot_bootstrap",
    )

    for proof, expected_error in invalid_proofs:
        session = RecordingSession()
        session.queue(
            StubResponse(
                payload=_offline_applied_top_up_status(finality_proof=proof)
            )
        )
        with pytest.raises(RuntimeError, match=expected_error):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_top_up_finality_proof_rejects_missing_mismatched_and_type_confused_fields() -> None:
    missing = _offline_applied_top_up_status()
    del missing["value"]["result"]["result"]["finality_proof"]

    def mutated(*path_and_value: Any) -> Dict[str, Any]:
        *path, value = path_and_value
        proof = copy.deepcopy(_offline_top_up_finality_proof())
        cursor: Dict[str, Any] = proof
        for component in path[:-1]:
            cursor = cursor[component]
        cursor[path[-1]] = value
        return _offline_applied_top_up_status(finality_proof=proof)

    invalid = [
        missing,
        _offline_applied_top_up_status(finality_proof="bm90LWEtZGlyZWN0LXByb29m"),
        mutated("version", 2),
        mutated("anchor", "topup_operation_id", _offline_fixed_bytes(0x12)),
        mutated("anchor", "anchor_digest", _offline_fixed_bytes(0x72)),
        mutated("commit_qc", []),
        mutated("commit_qc", "height_context", []),
        mutated("commit_qc", "height_context", "height", 11),
        mutated("commit_qc", "certificate", []),
        mutated("commit_qc", "certificate", "round", []),
        mutated("commit_qc", "certificate", "round", "height", 13),
        mutated("commit_qc", "certificate", "proposal_round", "view", 1),
        mutated("anchor_path", []),
    ]
    for payload in invalid:
        session = RecordingSession()
        session.queue(StubResponse(payload=payload))
        with pytest.raises(RuntimeError):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_redeem_result_rejects_every_top_up_only_field() -> None:
    for field in ("anchor", "finality_proof"):
        result = {
            "transaction_hash": OFFLINE_TRANSACTION_HASH,
            "finalized_block_height": 12,
            "server_time_ms": 13,
            field: {},
        }
        payload = {
            "state": "applied",
            "value": {
                "operation_id": OFFLINE_OPERATION_ID,
                "result": {"kind": "redeem", "result": result},
            },
        }
        session = RecordingSession()
        session.queue(StubResponse(payload=payload))
        with pytest.raises(RuntimeError, match=field):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_top_up_anchor_preserves_full_width_amounts_and_heights() -> None:
    amount = {"atomic_units": (1 << 128) - 1, "scale": 28}
    finalized_height = (1 << 64) - 1
    anchor = _offline_top_up_anchor(amount=amount, finalized_height=finalized_height)
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=_offline_applied_top_up_status(
                anchor,
                finalized_block_height=finalized_height,
            )
        )
    )

    status = ToriiClient(
        "http://node.test", session=session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)

    assert isinstance(status, OfflineAppliedOperation)
    assert status.result.kind == "top_up"
    assert status.result.result.anchor.amount.atomic_units == (1 << 128) - 1
    assert status.result.result.anchor.finalized_height == finalized_height


def test_offline_top_up_anchor_rejects_malformed_and_cross_resource_conflicts() -> None:
    missing_digest = _offline_top_up_anchor()
    missing_digest.pop("anchor_digest")
    invalid = [
        missing_digest,
        _offline_top_up_anchor(chain_id="wonderland"),
        _offline_top_up_anchor(network_id="wonderland"),
        _offline_top_up_anchor(version=1),
        _offline_top_up_anchor(asset_scale=29),
        _offline_top_up_anchor(asset_scale=3),
        _offline_top_up_anchor(finalized_root=_offline_fixed_bytes(0x10)),
        _offline_top_up_anchor(shield_leaf_index=-1),
        _offline_top_up_anchor(shield_leaf_index=1 << 16),
        _offline_top_up_anchor(topup_operation_id=_offline_fixed_bytes(0x12)),
        _offline_top_up_anchor(finalized_height=11),
        _offline_top_up_anchor(finalized_tx_hash=_offline_fixed_bytes(0x25)),
        _offline_top_up_anchor(anchor_digest=_offline_fixed_bytes(0)),
        _offline_top_up_anchor(
            shield_verifier_id={"backend": "", "name": "asset-topup-shield-v2"}
        ),
        _offline_top_up_anchor(
            shield_verifier_id={"backend": "halo2/ipa", "name": "v" * 257}
        ),
        _offline_top_up_anchor(shield_verifier_commitment=_offline_fixed_bytes(0)),
        _offline_top_up_anchor(
            artifact_binding={
                "version": 4,
                "generation": "é" * 65,
                "manifest_sha256": _offline_fixed_bytes(0x81),
            }
        ),
        _offline_top_up_anchor(
            artifact_binding={
                "version": 4,
                "generation": "generation-1",
                "manifest_sha256": _offline_fixed_bytes(0),
            }
        ),
        _offline_top_up_anchor(
            artifact_binding={"generation": "generation-1"}
        ),
        _offline_top_up_anchor(
            current_note={
                "network_id": OFFLINE_NETWORK_ID,
                "asset": CANONICAL_ASSET_ID,
                "note_commitment": _offline_fixed_bytes(0x41),
                "spend_nullifier": _offline_fixed_bytes(0x41),
                "amount": {"atomic_units": 17, "scale": 4},
            }
        ),
        _offline_top_up_anchor(
            current_note={
                "network_id": OFFLINE_OTHER_NETWORK_ID,
                "asset": CANONICAL_ASSET_ID,
                "note_commitment": _offline_fixed_bytes(0x41),
                "spend_nullifier": _offline_fixed_bytes(0x51),
                "amount": {"atomic_units": 17, "scale": 4},
            }
        ),
        _offline_top_up_anchor(
            current_note={
                "network_id": OFFLINE_NETWORK_ID,
                "asset": "different-asset",
                "note_commitment": _offline_fixed_bytes(0x41),
                "spend_nullifier": _offline_fixed_bytes(0x51),
                "amount": {"atomic_units": 17, "scale": 4},
            }
        ),
        _offline_top_up_anchor(
            current_note={
                "network_id": OFFLINE_NETWORK_ID,
                "asset": CANONICAL_ASSET_ID,
                "note_commitment": _offline_fixed_bytes(0x41),
                "spend_nullifier": _offline_fixed_bytes(0x51),
                "amount": {"atomic_units": 18, "scale": 4},
            }
        ),
    ]

    for anchor in invalid:
        session = RecordingSession()
        session.queue(StubResponse(payload=_offline_applied_top_up_status(anchor)))
        with pytest.raises(RuntimeError):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_applied_status_rejects_zero_finality_fields() -> None:
    for kind in ("top_up", "redeem"):
        for field in ("finalized_block_height", "server_time_ms"):
            result: Dict[str, Any] = {
                "transaction_hash": OFFLINE_TRANSACTION_HASH,
                "finalized_block_height": 1,
                "server_time_ms": 1,
            }
            result[field] = 0
            if kind == "top_up":
                anchor = _offline_top_up_anchor(finalized_height=result["finalized_block_height"])
                result["anchor"] = anchor
                result["finality_proof"] = _offline_top_up_finality_proof(
                    anchor,
                    finalized_height=result["finalized_block_height"],
                )
            payload = {
                "state": "applied",
                "value": {
                    "operation_id": OFFLINE_OPERATION_ID,
                    "result": {"kind": kind, "result": result},
                },
            }
            session = RecordingSession()
            session.queue(StubResponse(payload=payload))
            with pytest.raises(RuntimeError, match=field):
                ToriiClient(
                    "http://node.test", session=session
                ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_pending_status_rejects_zero_submission_time() -> None:
    payload = {
        "state": "pending",
        "value": {
            "operation_id": OFFLINE_OPERATION_ID,
            "kind": {"kind": "top_up", "value": None},
            "transaction_hash": OFFLINE_TRANSACTION_HASH,
            "submitted_at_ms": 0,
        },
    }
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    with pytest.raises(RuntimeError, match="submitted_at_ms"):
        ToriiClient(
            "http://node.test", session=session
        ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_applied_top_up_rejects_self_consistent_foreign_network() -> None:
    payload = _offline_applied_top_up_status()
    result = payload["value"]["result"]["result"]
    result["anchor"]["network_id"] = OFFLINE_OTHER_NETWORK_ID
    result["anchor"]["current_note"]["network_id"] = OFFLINE_OTHER_NETWORK_ID
    result["finality_proof"]["commit_qc"]["height_context"][
        "network_id"
    ] = OFFLINE_OTHER_NETWORK_ID

    transport_only_session = RecordingSession()
    transport_only_session.queue(StubResponse(payload=copy.deepcopy(payload)))
    transport_only = ToriiClient(
        "http://node.test", session=transport_only_session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)
    assert isinstance(transport_only, OfflineAppliedOperation)

    bound_session = RecordingSession()
    bound_session.queue(StubResponse(payload=payload))
    with pytest.raises(RuntimeError, match="configured client network"):
        ToriiClient(
            "http://node.test",
            session=bound_session,
            local_signing_context=ToriiLocalSigningContext(OFFLINE_NETWORK_ID),
        ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_rejection_requires_the_exact_error_code() -> None:
    for code in ("1_future_code", "rejected", "", "_leading_underscore", "a" * 65):
        session = RecordingSession()
        session.queue(
            StubResponse(
                payload=_offline_rejected_status({"code": code, "message": "invalid code"})
            )
        )
        with pytest.raises(RuntimeError):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_error_messages_require_bounded_unicode_scalar_text() -> None:
    maximum_astral_message = "\U0001f600" * 1024
    accepted_session = RecordingSession()
    accepted_session.queue(
        StubResponse(
            payload=_offline_rejected_status(
                {
                    "code": "offline_operation_rejected",
                    "message": maximum_astral_message,
                }
            )
        )
    )
    accepted = ToriiClient(
        "http://node.test", session=accepted_session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)
    assert isinstance(accepted, OfflineRejectedOperation)
    assert accepted.error.message == maximum_astral_message

    for message in (
        "",
        " leading",
        "trailing ",
        "line\nbreak",
        "control\u0085",
        "\ud800",
        "\udc00",
        "\U0001f600" * 1025,
    ):
        session = RecordingSession()
        session.queue(
            StubResponse(
                payload=_offline_rejected_status(
                    {"code": "offline_operation_rejected", "message": message}
                )
            )
        )
        with pytest.raises(RuntimeError):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_rejection_requires_details_to_be_absent() -> None:
    for details in (None, {}, {"layer": "torii"}):
        session = RecordingSession()
        session.queue(
            StubResponse(
                payload=_offline_rejected_status(
                    {
                        "code": "offline_operation_rejected",
                        "message": "rejected",
                        "details": details,
                    }
                )
            )
        )
        with pytest.raises(RuntimeError, match="details"):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_json_decoder_rejects_duplicates_non_finite_depth_and_size() -> None:
    valid = json.dumps(_offline_capability_payload())
    duplicate = valid.replace('"ready": false', '"ready": false, "ready": false')
    assert duplicate != valid, "duplicate-key fixture must actually introduce a duplicate"
    non_finite = valid.replace('"max_hops": 8', '"max_hops": NaN')
    infinity = valid.replace('"max_hops": 8', '"max_hops": Infinity')
    deep_value = "0"
    for _ in range(130):
        deep_value = f"[{deep_value}]"
    deep = valid[:-1] + f', "unknown": {deep_value}}}'
    oversized = valid[:-1] + f', "unknown": "{"x" * (256 * 1024)}"}}'

    for body in (duplicate, non_finite, infinity, deep, oversized):
        session = RecordingSession()
        session.queue(
            StubResponse(
                text=body,
                headers={"Content-Type": "application/json"},
            )
        )
        with pytest.raises(RuntimeError):
            ToriiClient("http://node.test", session=session).get_offline_capability()


def test_offline_status_rejects_noncanonical_paths_and_adversarial_envelopes() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)
    for operation_id in (
        "AB" * 32,
        "00" * 32,
        "11",
        f"{OFFLINE_OPERATION_ID}/extra",
    ):
        with pytest.raises(RuntimeError):
            client.get_kagemusha_operation_status(operation_id)
    assert session.calls == []

    invalid_statuses = [
        {"state": "unknown", "value": {"operation_id": OFFLINE_OPERATION_ID}},
        {
            "state": "pending",
            "value": {
                "operation_id": OFFLINE_OPERATION_ID,
                "kind": {"kind": "top_up", "value": None},
                "transaction_hash": OFFLINE_TRANSACTION_HASH,
                "submitted_at_ms": 1,
            },
            "unexpected": True,
        },
        {
            "state": "pending",
            "value": {
                "operation_id": OFFLINE_OPERATION_ID,
                "kind": {"kind": "top_up", "value": None},
                "transaction_hash": OFFLINE_TRANSACTION_HASH,
                "submitted_at_ms": 1,
                "unexpected": True,
            },
        },
        _offline_applied_top_up_status(unexpected=True),
        {
            "state": "pending",
            "value": {
                "operation_id": "33" * 32,
                "kind": {"kind": "top_up"},
                "transaction_hash": OFFLINE_TRANSACTION_HASH,
                "submitted_at_ms": 1,
            },
        },
        {
            "state": "pending",
            "value": {
                "operation_id": OFFLINE_OPERATION_ID,
                "kind": {"kind": "top_up", "value": {}},
                "transaction_hash": OFFLINE_TRANSACTION_HASH,
                "submitted_at_ms": 1,
            },
        },
        {
            "state": "applied",
            "value": {
                "operation_id": OFFLINE_OPERATION_ID,
                "result": {
                    "kind": "redeem",
                    "result": {
                        "transaction_hash": OFFLINE_TRANSACTION_HASH,
                        "finalized_block_height": 1,
                        "server_time_ms": 2,
                        "anchor": {},
                    },
                },
            },
        },
        {
            "state": "rejected",
            "value": {
                "operation_id": OFFLINE_OPERATION_ID,
                "kind": {"kind": "redeem"},
                "transaction_hash": OFFLINE_TRANSACTION_HASH,
                "error": {"code": "INVALID-CODE", "message": "no"},
            },
        },
    ]
    for payload in invalid_statuses:
        invalid_session = RecordingSession()
        invalid_session.queue(StubResponse(payload=payload))
        invalid_client = ToriiClient("http://node.test", session=invalid_session)
        with pytest.raises(RuntimeError):
            invalid_client.get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_status_snapshot_parses_mode_and_consensus_caps() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "mode_tag": "iroha2-consensus::permissioned-sumeragi@v2",
                "staged_mode_tag": "iroha2-consensus::npos-sumeragi@v2",
                "staged_mode_activation_height": 10,
                "mode_activation_lag_blocks": 2,
                "consensus_caps": {
                    "collectors_k": 2,
                    "redundant_send_r": 1,
                    "da_enabled": True,
                    "rbc_chunk_max_bytes": 1024,
                    "rbc_session_ttl_ms": 5000,
                    "rbc_store_max_sessions": 64,
                    "rbc_store_soft_sessions": 32,
                    "rbc_store_max_bytes": 4096,
                    "rbc_store_soft_bytes": 2048,
                },
                "peers": 1,
                "queue_size": 2,
                "commit_time_ms": 3,
                "da_reschedule_total": 4,
                "txs_approved": 5,
                "txs_rejected": 6,
                "view_changes": 7,
                "lane_commitments": [],
                "dataspace_commitments": [],
                "lane_governance": [],
                "lane_governance_sealed_total": 0,
                "lane_governance_sealed_aliases": [],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_status_snapshot()

    assert snapshot.status.mode_tag == "iroha2-consensus::permissioned-sumeragi@v2"
    assert snapshot.status.staged_mode_tag == "iroha2-consensus::npos-sumeragi@v2"
    assert snapshot.status.staged_mode_activation_height == 10
    assert snapshot.status.mode_activation_lag_blocks == 2
    assert snapshot.status.consensus_caps is not None
    assert snapshot.status.consensus_caps.collectors_k == 2
    assert snapshot.status.consensus_caps.rbc_chunk_max_bytes == 1024
