from __future__ import annotations

import base64
import copy
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, Dict, List, Mapping, Optional, Union, get_args, get_type_hints
from urllib.parse import quote

import pytest
import requests
from requests.structures import CaseInsensitiveDict

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

import iroha_torii_client as torii_module  # noqa: E402
import iroha_torii_client.client as client_module  # noqa: E402
from iroha_torii_client import (  # noqa: E402  (import depends on sys.path mutation)
    ContractCallResponse,
    ContractOperationReceipt,
    ContractDeployResponse,
    ExplorerAccountQr,
    GovernanceContractResponse,
    MultisigResponse,
    NetworkTimeSnapshot,
    NetworkTimeStatus,
    SumeragiV2Status,
    OfflineAppliedOperation,
    OfflineAssetScale,
    OfflinePendingOperation,
    KagemushaRedeemRequestV2,
    OfflineRejectedOperation,
    OfflineTopUpAnchor,
    OfflineTopUpFinalityProof,
    KagemushaTopUpRequestV2,
    ToriiCanonicalRequestAuth,
    ToriiClient,
    VpnQuoteCreateRequest,
    VpnReceiptSubmitRequest,
    VpnSessionCreateRequest,
    build_canonical_request_headers,
    canonical_request_signature_message,
    decode_pdp_commitment_header,
    encode_identifier_resolution_receipt_attestation,
    encode_identifier_resolution_receipt_payload,
    inspect_i105_network_prefix,
    verify_identifier_resolution_receipt,
)
from iroha_torii_client.client import _decode_i105_string  # noqa: E402
from iroha_torii_client.mock import ToriiMockServer  # noqa: E402

CANONICAL_OWNER = "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"
CANONICAL_ASSET_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
CANONICAL_ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"


def _contract_operation_receipt(
    *,
    entrypoint: str = "ping",
    gas_limit: int = 5000,
) -> Dict[str, Any]:
    return {
        "operation_kind": "contract_call",
        "status": "submitted",
        "transport": "torii",
        "dataspace": "universal",
        "contract_alias": "router::universal",
        "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
        "code_hash_hex": "22" * 32,
        "abi_hash_hex": "33" * 32,
        "tx_hash_hex": "44" * 32,
        "entrypoint": entrypoint,
        "entrypoint_hash_hex": "55" * 32,
        "gas_limit": gas_limit,
        "gas_used": 17,
        "gas_asset_id": "xor#wonderland",
        "fee_sponsor": CANONICAL_OWNER,
        "payload_digest_hex": "66" * 32,
    }


def _canonical_hash(seed: int) -> str:
    body_bytes = bytearray([seed & 0xFF] * 32)
    body_bytes[-1] |= 1
    body = body_bytes.hex().upper()
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return f"hash:{body}#{crc:04X}"


def _sumeragi_v2_status_payload() -> Dict[str, Any]:
    subject = {
        "parent_block_hash": _canonical_hash(0x31),
        "block_hash": _canonical_hash(0x32),
        "payload_hash": _canonical_hash(0x33),
    }
    return {
        "protocol_version": 2,
        "node_fingerprint": _canonical_hash(0x11),
        "build_fingerprint": _canonical_hash(0x12),
        "config_fingerprint": _canonical_hash(0x13),
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
                "phase": {"phase": "commit", "details": None},
                "subject": dict(subject),
            },
            "validator_count": 4,
            "signer_count": 3,
            "min_signers": 3,
            "signed_power": 3,
            "total_power": 4,
        },
        "safety_halt": {
            "active": False,
            "reason": None,
            "height": 0,
            "epoch": 0,
            "first_block_hash": None,
            "conflicting_block_hash": None,
            "first_parent_state_root": None,
            "first_post_state_root": None,
            "conflicting_parent_state_root": None,
            "conflicting_post_state_root": None,
        },
        "lane_settlement_commitments": [],
        "lane_relay_envelopes": [],
        "lane_payload_ownerships": [],
        "committed_lane_blocks": [],
        "lane_block_sessions": [],
        "local_peer_removed": False,
        "operator": {
            "view_change_install_total": 7,
            "busy_deferral_total": 3,
            "adapter_queues": {
                "ingress_keys": 2,
                "ingress_capacity": 16,
                "deferred_completion": 1,
                "deferred_progress": 2,
                "deferred_progress_capacity": 4,
                "deferred_normal": 3,
                "deferred_normal_capacity": 8,
            },
            "tx_queue": {
                "tracked_transactions": 5,
                "queued_transactions": 3,
                "capacity": 32,
                "retained_bytes": 4096,
                "max_retained_bytes": 65536,
                "oldest_queued_age_ms": 25,
                "saturated_by_count": False,
                "saturated_by_bytes": False,
                "saturated_by_age": False,
            },
        },
    }


def _lane_settlement_payload() -> Dict[str, Any]:
    return {
        "block_height": 9,
        "lane_id": 2,
        "lane_incarnation": _canonical_hash(0x51),
        "dataspace_id": 7,
        "tx_count": 1,
        "total_local_micro": "10",
        "total_xor_due_micro": "5",
        "total_xor_after_haircut_micro": "4",
        "total_xor_variance_micro": "1",
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
                "local_amount_micro": "10",
                "xor_due_micro": "5",
                "xor_after_haircut_micro": "4",
                "xor_variance_micro": "1",
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
        "fee_amount": "7.5",
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


def _native_amx_receipt_payload() -> Dict[str, Any]:
    transaction_hash = _canonical_hash(0x61)
    source_id = "AB" * 32
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
        "chain_id_hash": _canonical_hash(0x63),
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
            "validator_set": ["validator-a", "validator-b", "validator-c", "validator-d"],
            "validator_set_pops": [[1] * 96 for _ in range(4)],
            "signers_bitmap": [0x07],
            "bls_aggregate_signature": [2] * 96,
        }

    return {
        "version": 2,
        "source_id": source_id,
        "chain_id_hash": _canonical_hash(0x63),
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
                        "accepted_candidate_indices": [0],
                        "accepted_transaction_hashes": [transaction_hash],
                        "validator_set_hash_version": 1,
                        "validator_set_hash": _canonical_hash(0x66),
                        "validator_set": [
                            "validator-a",
                            "validator-b",
                            "validator-c",
                            "validator-d",
                        ],
                        "validator_count": 4,
                        "min_quorum": 3,
                        "qc_mode_tag": "permissioned:native-amx-v2",
                        "descriptor_hash": _canonical_hash(0x73),
                    },
                    "proposal_hash": participant_proposal_hash,
                },
                "participant_settlement": {
                    "block_height": 8,
                    "lane_id": 3,
                    "lane_incarnation": _canonical_hash(0x65),
                    "dataspace_id": 8,
                    "tx_count": 2,
                    "total_local_micro": "0",
                    "total_xor_due_micro": "0",
                    "total_xor_after_haircut_micro": "0",
                    "total_xor_variance_micro": "0",
                    "swap_metadata": None,
                    "receipts": [
                        {
                            "source_id": source_id,
                            "local_amount_micro": "0",
                            "xor_due_micro": "0",
                            "xor_after_haircut_micro": "0",
                            "xor_variance_micro": "0",
                            "timestamp_ms": 10,
                        },
                        {
                            "source_id": "CD" * 32,
                            "local_amount_micro": "0",
                            "xor_due_micro": "0",
                            "xor_after_haircut_micro": "0",
                            "xor_variance_micro": "0",
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
    }


def _get_sumeragi_status(payload: Mapping[str, Any]) -> SumeragiV2Status:
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    return ToriiClient("http://node.test", session=session).get_sumeragi_status()


def _canonical_signature_base64_fixture() -> str:
    return base64.b64encode(bytes([1]) * 64).decode("ascii")


def _noncanonical_standard_base64_pad_bit_alias(encoded: str) -> str:
    assert encoded.endswith("==")
    alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    chars = list(encoded)
    index = len(chars) - 3
    chars[index] = alphabet[alphabet.index(chars[index]) ^ 0x01]
    return "".join(chars)


def test_i105_decoder_rejects_out_of_range_numeric_discriminants() -> None:
    payload = CANONICAL_OWNER.removeprefix("sora")

    assert _decode_i105_string(f"n65535{payload}")
    prefix = inspect_i105_network_prefix(CANONICAL_OWNER, expected_chain_discriminant=0x02F1)
    assert prefix.sentinel == "sora"
    assert prefix.chain_discriminant == 0x02F1
    assert prefix.profile == "minamoto"
    numeric_prefix = inspect_i105_network_prefix(f"n65535{payload}")
    assert numeric_prefix.sentinel == "n65535"
    assert numeric_prefix.chain_discriminant == 65535
    assert numeric_prefix.profile is None
    with pytest.raises(ValueError, match="discriminant mismatch"):
        inspect_i105_network_prefix(CANONICAL_OWNER, expected_chain_discriminant=0x0171)
    for literal in (f"n65536{payload}", f"n70000{payload}"):
        with pytest.raises(ValueError, match="unsigned 16-bit"):
            _decode_i105_string(literal)


class StubResponse(requests.Response):
    def __init__(
        self,
        status_code: int = 200,
        payload: Optional[Any] = None,
        *,
        headers: Optional[Dict[str, str]] = None,
        text: Optional[str] = None,
    ) -> None:
        super().__init__()
        self.status_code = status_code
        self._payload = payload
        self.headers = CaseInsensitiveDict(headers or {})
        if payload is None:
            content = text.encode("utf-8") if text is not None else b""
        else:
            content = json.dumps(payload).encode("utf-8")
            if "Content-Type" not in self.headers:
                self.headers["Content-Type"] = "application/json"
        self._content = content
        self.encoding = "utf-8"

    def json(self, **kwargs: Any) -> Any:
        if self._payload is None:
            raise ValueError("no payload available")
        return json.loads(self.text)


class RecordingSession(requests.Session):
    def __init__(self) -> None:
        super().__init__()
        self.calls: List[Dict[str, Any]] = []
        self._responses: List[StubResponse] = []

    def queue(self, response: StubResponse) -> None:
        self._responses.append(response)

    def request(
        self,
        method: Union[str, bytes],
        url: Union[str, bytes],
        *args: Any,
        **kwargs: Any,
    ) -> requests.Response:
        params = kwargs.get("params") or {}
        headers = kwargs.get("headers") or {}
        data = kwargs.get("data")
        self.calls.append(
            {
                "method": method,
                "url": url,
                "params": params,
                "headers": headers,
                "data": data,
            }
        )
        if not self._responses:
            raise AssertionError("no queued responses")
        return self._responses.pop(0)


def _sample_sorafs_orderbook_payloads() -> Dict[str, Any]:
    order_id_hex = "11" * 32
    trade_id_hex = "22" * 32
    channel_id_hex = "33" * 32
    receipt_id_hex = "44" * 32
    provider_id_hex = "55" * 32
    chunk_hash_hex = "66" * 32
    signature = {
        "algorithm": "Ed25519",
        "public_key_hex": "AA" * 32,
        "signature_hex": "BB" * 64,
    }
    order = {
        "version": 1,
        "order_id_hex": f"0x{order_id_hex.upper()}",
        "side": "bid",
        "tier": "hot",
        "price_per_gib_micro_xor": "1500000",
        "quantity_gib": 4,
        "remaining_gib": 2,
        "owner_account_hex": "CAFE",
        "expiry_unix": 1_800_000_000,
        "nonce": 7,
        "maker_fee_bps": 25,
        "taker_fee_bps": 35,
        "signature": signature,
    }
    trade = {
        "version": 1,
        "trade_id_hex": trade_id_hex,
        "maker_order_id_hex": order_id_hex,
        "taker_order_id_hex": "77" * 32,
        "tier": "hot",
        "price_per_gib_micro_xor": "1500000",
        "filled_gib": 2,
        "maker_fee_micro_xor": "75000",
        "taker_fee_micro_xor": "105000",
        "timestamp_unix": 1_700_000_100,
    }
    channel = {
        "version": 1,
        "channel_id_hex": channel_id_hex,
        "trade_id_hex": trade_id_hex,
        "buyer_account_hex": "FACE",
        "provider_id_hex": provider_id_hex,
        "total_bytes": 2_147_483_648,
        "remaining_bytes": 1_073_741_824,
        "xor_locked_micro": "3000000",
        "status": "open",
        "opened_at_unix": 1_700_000_101,
        "updated_at_unix": 1_700_000_102,
    }
    receipt = {
        "version": 1,
        "receipt_id_hex": receipt_id_hex,
        "channel_id_hex": channel_id_hex,
        "trade_id_hex": trade_id_hex,
        "range": {"start": 0, "end": 1024},
        "chunk_hash_hex": chunk_hash_hex,
        "bytes_delivered": 1024,
        "xor_debited_micro": "1500",
        "provider_credit_micro": "1400",
        "fee_amount_micro": "100",
        "issued_at_unix": 1_700_000_103,
        "settlement_signature": signature,
    }
    event = {
        "sequence": 9,
        "kind": "settlement_receipt_accepted",
        "generated_at_unix": 1_700_000_104,
        "order_id_hex": None,
        "trade_ids_hex": [trade_id_hex],
        "settlement_channel_ids_hex": [channel_id_hex],
        "receipt_id_hex": receipt_id_hex,
        "expired_order_ids_hex": [order_id_hex],
        "open_order_count": 1,
        "open_settlement_channel_count": 1,
        "settlement_receipt_count": 1,
    }
    return {
        "order_id_hex": order_id_hex,
        "trade_id_hex": trade_id_hex,
        "channel_id_hex": channel_id_hex,
        "receipt_id_hex": receipt_id_hex,
        "provider_id_hex": provider_id_hex,
        "order": order,
        "trade": trade,
        "channel": channel,
        "receipt": receipt,
        "event": event,
    }


def test_sorafs_orderbook_read_helpers_build_paths_and_normalize_payloads() -> None:
    payloads = _sample_sorafs_orderbook_payloads()
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "schema": "sorafs.orderbook.local.v1",
                "source": "local",
                "generated_at_unix": 1_700_000_000,
                "next_sequence": 10,
                "open_order_count": 1,
                "trade_count": 1,
                "settlement_channel_count": 1,
                "settlement_receipt_count": 1,
                "depth": {
                    "hot_bid_gib": 2,
                    "hot_ask_gib": 0,
                    "warm_bid_gib": 0,
                    "warm_ask_gib": 0,
                    "archive_bid_gib": 0,
                    "archive_ask_gib": 0,
                },
                "open_orders": [{"sequence": 1, "order": payloads["order"]}],
                "trades": [payloads["trade"]],
                "settlement_channels": [payloads["channel"]],
                "settlement_receipts": [payloads["receipt"]],
                "expired_order_ids_hex": [payloads["order_id_hex"]],
            }
        )
    )
    session.queue(StubResponse(payload={"count": 1, "trades": [payloads["trade"]]}))
    session.queue(StubResponse(payload={"count": 1, "channels": [payloads["channel"]]}))
    session.queue(StubResponse(payload={"count": 1, "receipts": [payloads["receipt"]]}))
    session.queue(
        StubResponse(
            payload={
                "since": 0,
                "limit": 10,
                "count": 1,
                "next_since": 9,
                "events": [payloads["event"]],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    book = client.get_sorafs_orderbook(headers={"X-Trace": "book"})
    assert book["open_orders"][0]["order"]["order_id_hex"] == payloads["order_id_hex"]
    assert book["open_orders"][0]["order"]["owner_account_hex"] == "cafe"
    assert book["open_orders"][0]["order"]["signature"]["public_key_hex"] == "aa" * 32
    assert session.calls[0]["method"] == "GET"
    assert session.calls[0]["url"].endswith("/v1/sorafs/orderbook/book")
    assert session.calls[0]["headers"]["X-Trace"] == "book"

    trades = client.list_sorafs_orderbook_trades()
    assert trades["trades"][0]["trade_id_hex"] == payloads["trade_id_hex"]
    assert session.calls[1]["url"].endswith("/v1/sorafs/orderbook/trades")

    channels = client.list_sorafs_orderbook_channels()
    assert channels["channels"][0]["provider_id_hex"] == payloads["provider_id_hex"]
    assert channels["channels"][0]["status"] == "open"
    assert session.calls[2]["url"].endswith("/v1/sorafs/orderbook/channels")

    receipts = client.list_sorafs_orderbook_receipts()
    assert receipts["receipts"][0]["range"]["end"] == 1024
    assert receipts["receipts"][0]["settlement_signature"]["signature_hex"] == "bb" * 64
    assert session.calls[3]["url"].endswith("/v1/sorafs/orderbook/receipts")

    events = client.list_sorafs_orderbook_events(
        since=0,
        limit="10",
        if_none_match='"old-events"',
    )
    assert events is not None
    assert events["events"][0]["kind"] == "settlement_receipt_accepted"
    assert events["events"][0]["receipt_id_hex"] == payloads["receipt_id_hex"]
    assert session.calls[4]["url"].endswith("/v1/sorafs/orderbook/events")
    assert session.calls[4]["params"] == {"since": 0, "limit": 10}
    assert session.calls[4]["headers"]["If-None-Match"] == '"old-events"'


def test_sorafs_orderbook_read_helpers_validate_options_and_cache_status() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())

    with pytest.raises(ValueError, match="positive"):
        client.list_sorafs_orderbook_events(limit=0)
    with pytest.raises(ValueError, match="one of if_none_match or etag"):
        client.list_sorafs_orderbook_events(if_none_match='"a"', etag='"b"')
    with pytest.raises(TypeError, match="headers must be a mapping"):
        client.get_sorafs_orderbook(headers="not-a-mapping")  # type: ignore[arg-type]

    session = RecordingSession()
    session.queue(StubResponse(status_code=304))
    cached_client = ToriiClient("http://node.test", session=session)

    assert cached_client.list_sorafs_orderbook_events(etag='"same"') is None
    assert session.calls[0]["headers"]["If-None-Match"] == '"same"'


def test_sorafs_orderbook_submit_helpers_sign_exact_payload_bytes() -> None:
    payloads = _sample_sorafs_orderbook_payloads()
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "status": "accepted",
                "sequence": 12,
                "open_order_count": 1,
                "accepted_order": payloads["order"],
                "fills": [
                    {
                        "trade": payloads["trade"],
                        "maker_remaining_gib": 0,
                        "taker_remaining_gib": 2,
                        "gross_value_micro_xor": "3000000",
                    }
                ],
                "settlement_channels_opened": [payloads["channel"]],
                "expired_order_ids_hex": [payloads["order_id_hex"]],
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "status": "cancelled",
                "reason": "owner_requested",
                "open_order_count": 0,
                "cancelled_order": payloads["order"],
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "status": "accepted",
                "settlement_receipt_count": 1,
                "open_settlement_channel_count": 1,
                "accepted_receipt": payloads["receipt"],
                "updated_channel": payloads["channel"],
            }
        )
    )
    signed_messages: List[bytes] = []

    def signer(message: bytes) -> bytes:
        signed_messages.append(message)
        return b"signed-request"

    auth = ToriiCanonicalRequestAuth(
        account_id="alice@wonderland",
        signer=signer,
        timestamp_ms=1234,
        nonce="nonce-1",
    )
    client = ToriiClient("http://node.test", session=session)

    order_result = client.submit_sorafs_orderbook_order(
        b"\x01\x02\x03",
        canonical_auth=auth,
        headers={"X-Trace": "order-submit"},
    )
    assert order_result["status"] == "accepted"
    assert order_result["sequence"] == 12
    assert order_result["fills"][0]["gross_value_micro_xor"] == "3000000"
    order_call = session.calls[0]
    assert order_call["method"] == "POST"
    assert order_call["url"].endswith("/v1/sorafs/orderbook/orders")
    assert order_call["data"] == b"\x01\x02\x03"
    assert order_call["headers"]["Accept"] == "application/json"
    assert order_call["headers"]["Content-Type"] == "application/octet-stream"
    assert order_call["headers"]["X-Trace"] == "order-submit"
    assert order_call["headers"]["X-Iroha-Account"] == "alice@wonderland"
    assert order_call["headers"]["X-Iroha-Signature"] == base64.b64encode(
        b"signed-request"
    ).decode("ascii")
    assert signed_messages[0] == canonical_request_signature_message(
        "POST",
        "/v1/sorafs/orderbook/orders",
        b"\x01\x02\x03",
        timestamp_ms=1234,
        nonce="nonce-1",
    )

    cancel_result = client.submit_sorafs_orderbook_cancel([4, 5], canonical_auth=auth)
    assert cancel_result["status"] == "cancelled"
    assert cancel_result["cancelled_order"]["order_id_hex"] == payloads["order_id_hex"]
    assert session.calls[1]["url"].endswith("/v1/sorafs/orderbook/cancel")
    assert session.calls[1]["data"] == b"\x04\x05"

    receipt_result = client.submit_sorafs_orderbook_receipt(bytearray([6]), canonical_auth=auth)
    assert receipt_result["status"] == "accepted"
    assert receipt_result["accepted_receipt"]["receipt_id_hex"] == payloads["receipt_id_hex"]
    assert session.calls[2]["url"].endswith("/v1/sorafs/orderbook/receipts")
    assert session.calls[2]["data"] == b"\x06"


def test_sorafs_orderbook_submit_helpers_validate_inputs() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())
    auth = ToriiCanonicalRequestAuth(
        account_id="alice@wonderland",
        signer=lambda _message: b"signed-request",
    )

    with pytest.raises(ValueError, match="canonical_auth is required"):
        client.submit_sorafs_orderbook_order(b"\x01", canonical_auth=None)  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="must not be empty"):
        client.submit_sorafs_orderbook_receipt(b"", canonical_auth=auth)
    with pytest.raises(TypeError, match="headers must be a mapping"):
        client.submit_sorafs_orderbook_cancel(
            b"\x01",
            canonical_auth=auth,
            headers="not-a-mapping",  # type: ignore[arg-type]
        )


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
VPN_PAYMENT_HASH = "22" * 32
VPN_METERING_KEY = "33" * 32
VPN_LEASE_ID = VPN_QUOTE_ID
VPN_HELPER_TICKET_HEX = "5356504e48543100" + "00" * 248


def _vpn_instruction(wire_id: str = "OpenVpnLeaseEscrow") -> Dict[str, str]:
    return {"wire_id": wire_id, "payload_hex": "ab" * 8}


def _vpn_profile_payload() -> Dict[str, Any]:
    return {
        "available": True,
        "relay_endpoint": "/dns4/relay.example/tcp/443",
        "supported_exit_classes": ["standard", "low-latency"],
        "default_exit_class": "standard",
        "lease_secs": 3600,
        "dns_push_interval_secs": 60,
        "meter_family": "soranet.vpn.v1",
        "route_pushes": ["0.0.0.0/0"],
        "excluded_routes": ["10.0.0.0/8"],
        "dns_servers": ["1.1.1.1"],
        "tunnel_addresses": ["10.208.0.2/32"],
        "mtu_bytes": 1280,
        "display_billing_label": "standard - soranet.vpn.v1 - 100 nano-XOR",
        "fee_asset_id": "xor#universal",
        "escrow_account_id": VPN_ESCROW,
        "operator_account_id": VPN_OPERATOR,
        "lease_fee_nanos": 100,
        "settlement_grace_secs": 300,
        "flow_label_bits": 20,
        "padding_budget_ms": 250,
        "relay_tls_spki_sha256_hex": "44" * 32,
    }


def _vpn_quote_payload() -> Dict[str, Any]:
    payload = _vpn_profile_payload()
    return {
        "quote_id": VPN_QUOTE_ID,
        "lease_id_hex": VPN_LEASE_ID,
        "session_id_hex": VPN_QUOTE_ID,
        "payment_reference": VPN_QUOTE_ID,
        "account_id": VPN_ACCOUNT,
        "exit_class": "standard",
        "relay_endpoint": payload["relay_endpoint"],
        "lease_secs": payload["lease_secs"],
        "quote_expires_at_ms": 1_700_000_000_000,
        "fee_asset_id": payload["fee_asset_id"],
        "escrow_account_id": VPN_ESCROW,
        "operator_account_id": VPN_OPERATOR,
        "lease_fee_nanos": payload["lease_fee_nanos"],
        "route_pushes": payload["route_pushes"],
        "excluded_routes": payload["excluded_routes"],
        "dns_servers": payload["dns_servers"],
        "tunnel_addresses": payload["tunnel_addresses"],
        "mtu_bytes": payload["mtu_bytes"],
        "meter_family": payload["meter_family"],
        "flow_label_bits": payload["flow_label_bits"],
        "padding_budget_ms": payload["padding_budget_ms"],
        "relay_tls_spki_sha256_hex": payload["relay_tls_spki_sha256_hex"],
        "metering_public_key_hex": VPN_METERING_KEY,
        "open_lease_instruction": _vpn_instruction(),
        "tx_instructions": [_vpn_instruction()],
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
        "lease_fee_nanos": quote_payload["lease_fee_nanos"],
        "flow_label_bits": quote_payload["flow_label_bits"],
        "padding_budget_ms": quote_payload["padding_budget_ms"],
        "relay_tls_spki_sha256_hex": quote_payload["relay_tls_spki_sha256_hex"],
        "route_pushes": quote_payload["route_pushes"],
        "excluded_routes": quote_payload["excluded_routes"],
        "dns_servers": quote_payload["dns_servers"],
        "tunnel_addresses": quote_payload["tunnel_addresses"],
        "mtu_bytes": quote_payload["mtu_bytes"],
        "helper_ticket_hex": VPN_HELPER_TICKET_HEX,
        "bytes_in": 0,
        "bytes_out": 0,
        "status": "connected",
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
        "lease_fee_nanos": session_payload["lease_fee_nanos"],
        "earned_fee_nanos": 25,
        "refunded_fee_nanos": 75,
        "lease_id_hex": VPN_LEASE_ID,
        "settle_lease_instruction": _vpn_instruction("SettleVpnLease"),
        "tx_instructions": [_vpn_instruction("SettleVpnLease")],
    }


def _vpn_auth(captured: List[bytes]) -> ToriiCanonicalRequestAuth:
    def signer(message: bytes) -> bytes:
        captured.append(message)
        return b"\x7a" * 64

    return ToriiCanonicalRequestAuth(
        account_id=VPN_ACCOUNT,
        signer=signer,
        timestamp_ms=1_700_000_001_000,
        nonce="vpn-test-nonce",
    )


def test_vpn_profile_deserializes_native_lease_fields() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=_vpn_profile_payload()))
    client = ToriiClient("http://node.test", session=session)

    profile = client.get_vpn_profile()

    assert profile.fee_asset_id == "xor#universal"
    assert profile.lease_fee_nanos == 100
    assert profile.escrow_account_id == VPN_ESCROW
    assert profile.operator_account_id == VPN_OPERATOR
    assert profile.route_pushes == ["0.0.0.0/0"]
    assert session.calls[0]["url"] == "http://node.test/v1/vpn/profile"
    assert session.calls[0]["headers"] == {"Accept": "application/json"}


def test_create_vpn_quote_signs_body_and_parses_open_lease_instruction() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=201, payload=_vpn_quote_payload()))
    captured: List[bytes] = []
    auth = _vpn_auth(captured)
    client = ToriiClient("http://node.test", session=session)

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
        canonical_request_signature_message(
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
    assert quote.open_lease_instruction is not None
    assert quote.open_lease_instruction.wire_id == "OpenVpnLeaseEscrow"
    assert quote.tx_instructions[0].payload_hex == "ab" * 8


def test_canonical_request_auth_rejects_padded_fields_before_send() -> None:
    def signer(message: bytes) -> bytes:
        return b"\x7a" * 64

    with pytest.raises(ValueError, match="surrounding whitespace"):
        canonical_request_signature_message(
            "POST",
            "/v1/vpn/quotes",
            b"{}",
            timestamp_ms=1,
            nonce=" nonce",
        )
    with pytest.raises(ValueError, match="non-empty string"):
        build_canonical_request_headers(
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
            account_id=f"{VPN_ACCOUNT} ",
            signer=signer,
            method="POST",
            path="/v1/vpn/quotes",
            body=b"{}",
            timestamp_ms=1,
            nonce="nonce",
        )

    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises(ValueError, match="surrounding whitespace"):
        client.create_vpn_quote(
            VpnQuoteCreateRequest(
                metering_public_key_hex=bytes.fromhex(VPN_METERING_KEY),
                exit_class="standard",
            ),
            canonical_auth=ToriiCanonicalRequestAuth(
                account_id=VPN_ACCOUNT,
                signer=signer,
                timestamp_ms=1,
                nonce="nonce ",
            ),
        )
    assert session.calls == []


def test_identifier_resolution_receipt_matches_shared_vectors() -> None:
    pytest.importorskip("iroha_python.crypto")
    fixture = json.loads(
        (PACKAGE_ROOT.parent / "fixtures/soracloud/identifier_receipt_vectors_v1.json").read_text(
            encoding="utf-8"
        )
    )
    assert fixture["vector_set"] == "identifier-receipt-attestation-v1"

    payload_bytes = encode_identifier_resolution_receipt_payload(fixture["receipt"]["payload"])
    assert hashlib.sha256(payload_bytes).hexdigest().upper() == fixture["canonical_payload_sha256"]
    assert verify_identifier_resolution_receipt(fixture["receipt"], fixture["policy"]) is True

    for kind in (" signed", "signed ", "Signed"):
        non_exact_kind = json.loads(json.dumps(fixture["receipt"]["attestation"]))
        non_exact_kind["kind"] = kind
        with pytest.raises(ValueError, match="identifier receipt attestation.kind"):
            encode_identifier_resolution_receipt_attestation(non_exact_kind)

    padded_backend_payload = json.loads(json.dumps(fixture["receipt"]["payload"]))
    padded_backend_payload["execution"]["backend"] = " hkdf-sha3-512-prf-v1"
    with pytest.raises(ValueError, match="payload.execution.backend must not contain surrounding whitespace"):
        encode_identifier_resolution_receipt_payload(padded_backend_payload)

    padded_mode_payload = json.loads(json.dumps(fixture["receipt"]["payload"]))
    padded_mode_payload["execution"]["verification_mode"] = "signed "
    with pytest.raises(ValueError, match="payload.execution.verification_mode must not contain surrounding whitespace"):
        encode_identifier_resolution_receipt_payload(padded_mode_payload)

    for vector in fixture["attestation_vectors"]:
        encoded = encode_identifier_resolution_receipt_attestation(vector["attestation"])
        assert len(encoded) == vector["expected_attestation_bytes"], vector["name"]
        assert hashlib.sha256(encoded).hexdigest().upper() == vector["expected_attestation_sha256"]
        if vector["attestation"]["kind"] == "signed":
            for signature in (f" {vector['attestation']['signature']}", f"{vector['attestation']['signature']} "):
                padded_signature = json.loads(json.dumps(vector["attestation"]))
                padded_signature["signature"] = signature
                with pytest.raises(
                    ValueError,
                    match="identifier receipt attestation.signature must not contain surrounding whitespace",
                ):
                    encode_identifier_resolution_receipt_attestation(padded_signature)
        if vector["attestation"]["kind"] == "proof":
            padded_proof_backend = json.loads(json.dumps(vector["attestation"]))
            padded_proof_backend["proof_backend"] = f"{padded_proof_backend['proof_backend']} "
            with pytest.raises(ValueError, match="identifier receipt attestation.proof_backend must not contain surrounding whitespace"):
                encode_identifier_resolution_receipt_attestation(padded_proof_backend)

            malformed_proof_b64 = json.loads(json.dumps(vector["attestation"]))
            malformed_proof_b64["proof_b64"] = "@@@"
            with pytest.raises(ValueError, match="attestation.proof_b64 must be valid base64"):
                encode_identifier_resolution_receipt_attestation(malformed_proof_b64)

            for proof_b64 in (f" {vector['attestation']['proof_b64']}", f"{vector['attestation']['proof_b64']} "):
                padded_proof_b64 = json.loads(json.dumps(vector["attestation"]))
                padded_proof_b64["proof_b64"] = proof_b64
                with pytest.raises(
                    ValueError,
                    match="identifier receipt attestation.proof_b64 must not contain surrounding whitespace",
                ):
                    encode_identifier_resolution_receipt_attestation(padded_proof_b64)

            with pytest.raises(RuntimeError, match="proof attestations require an external verifier"):
                verify_identifier_resolution_receipt(
                    {
                        "payload": fixture["receipt"]["payload"],
                        "attestation": vector["attestation"],
                    },
                    fixture["policy"],
                )

    for opening_signature in (
        f" {fixture['receipt']['payload']['opening']['signature']}",
        f"{fixture['receipt']['payload']['opening']['signature']} ",
    ):
        padded_opening = json.loads(json.dumps(fixture["receipt"]))
        padded_opening["payload"]["opening"]["signature"] = opening_signature
        with pytest.raises(
            ValueError,
            match="payload.opening.signature must not contain surrounding whitespace",
        ):
            verify_identifier_resolution_receipt(padded_opening, fixture["policy"])

    for policy_id in (" phone#retail", "phone#retail ", "phone #retail", "phone# retail"):
        padded_policy_id = json.loads(json.dumps(fixture["receipt"]))
        padded_policy_id["payload"]["policy_id"] = policy_id
        with pytest.raises(ValueError, match="payload.policy_id"):
            verify_identifier_resolution_receipt(padded_policy_id, fixture["policy"])

    for program_id in (" identifier_lookup_retail", "identifier_lookup_retail "):
        padded_execution_program = json.loads(json.dumps(fixture["receipt"]))
        padded_execution_program["payload"]["execution"]["program_id"] = program_id
        with pytest.raises(ValueError, match="payload.execution.program_id"):
            verify_identifier_resolution_receipt(padded_execution_program, fixture["policy"])

        padded_opening_program = json.loads(json.dumps(fixture["receipt"]))
        padded_opening_program["payload"]["opening"]["payload"]["program_id"] = program_id
        with pytest.raises(ValueError, match="payload.opening.payload.program_id"):
            verify_identifier_resolution_receipt(padded_opening_program, fixture["policy"])

    for account_id in (
        f" {fixture['receipt']['payload']['account_id']}",
        f"{fixture['receipt']['payload']['account_id']} ",
    ):
        padded_account_id = json.loads(json.dumps(fixture["receipt"]))
        padded_account_id["payload"]["account_id"] = account_id
        with pytest.raises(ValueError, match="payload.account_id"):
            verify_identifier_resolution_receipt(padded_account_id, fixture["policy"])

    hash_exactness_cases = (
        ("payload.opaque_id", ("payload", "opaque_id"), fixture["receipt"]["payload"]["opaque_id"]),
        ("payload.receipt_hash", ("payload", "receipt_hash"), fixture["receipt"]["payload"]["receipt_hash"]),
        ("payload.uaid", ("payload", "uaid"), fixture["receipt"]["payload"]["uaid"]),
        (
            "payload.execution.program_digest",
            ("payload", "execution", "program_digest"),
            fixture["receipt"]["payload"]["execution"]["program_digest"],
        ),
        (
            "payload.opening.payload.input_ciphertext_hash",
            ("payload", "opening", "payload", "input_ciphertext_hash"),
            fixture["receipt"]["payload"]["opening"]["payload"]["input_ciphertext_hash"],
        ),
    )
    for context, path, value in hash_exactness_cases:
        for padded_value in (f" {value}", f"{value} "):
            padded_hash = json.loads(json.dumps(fixture["receipt"]))
            target = padded_hash
            for component in path[:-1]:
                target = target[component]
            target[path[-1]] = padded_value
            with pytest.raises(ValueError, match=context.replace(".", r"\.")):
                verify_identifier_resolution_receipt(padded_hash, fixture["policy"])

    timestamp_exactness_cases = (
        (
            "payload.execution.executed_at_ms",
            ("payload", "execution", "executed_at_ms"),
            fixture["receipt"]["payload"]["execution"]["executed_at_ms"],
        ),
        (
            "payload.execution.expires_at_ms",
            ("payload", "execution", "expires_at_ms"),
            fixture["receipt"]["payload"]["execution"]["expires_at_ms"],
        ),
        (
            "payload.opening.payload.opened_at_ms",
            ("payload", "opening", "payload", "opened_at_ms"),
            fixture["receipt"]["payload"]["opening"]["payload"]["opened_at_ms"],
        ),
        (
            "payload.opening.payload.expires_at_ms",
            ("payload", "opening", "payload", "expires_at_ms"),
            fixture["receipt"]["payload"]["opening"]["payload"]["expires_at_ms"],
        ),
    )
    for context, path, value in timestamp_exactness_cases:
        for padded_value in (f" {value}", f"{value} "):
            padded_timestamp = json.loads(json.dumps(fixture["receipt"]))
            target = padded_timestamp
            for component in path[:-1]:
                target = target[component]
            target[path[-1]] = padded_value
            with pytest.raises((TypeError, ValueError), match=context.replace(".", r"\.")):
                verify_identifier_resolution_receipt(padded_timestamp, fixture["policy"])

    for negative in fixture["negative_cases"]:
        receipt = json.loads(json.dumps(fixture["receipt"]))
        policy = json.loads(json.dumps(fixture["policy"]))
        if negative["mutation"] == "receipt.payload.execution.output_ciphertext_hash":
            receipt["payload"]["execution"]["output_ciphertext_hash"] = negative["value"]
        elif negative["mutation"] == "policy.resolver_public_key":
            policy["resolver_public_key"] = negative["value"]
        elif negative["mutation"] == "policy.policy_id":
            policy["policy_id"] = negative["value"]
        elif negative["mutation"] == "receipt.attestation.signature":
            receipt["attestation"]["signature"] = negative["value"]
        elif negative["mutation"] == "receipt.attestation":
            receipt["attestation"] = negative["value"]
        else:
            raise AssertionError(f"unhandled receipt vector mutation {negative['mutation']}")

        expected_error = negative.get("expected_error_contains")
        if expected_error:
            with pytest.raises((RuntimeError, ValueError), match=expected_error):
                verify_identifier_resolution_receipt(receipt, policy)
        else:
            assert (
                verify_identifier_resolution_receipt(receipt, policy)
                is negative["expected_result"]
            ), negative["name"]


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
    client = ToriiClient("http://node.test", session=session)
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
    assert deleted.tx_instructions[0].wire_id == "SettleVpnLease"
    assert receipts.total == 1
    assert receipts.items[0].refunded_fee_nanos == 75
    assert missing is None
    assert [call["method"] for call in session.calls] == ["POST", "GET", "DELETE", "GET", "GET"]


def test_submit_vpn_receipt_parses_settlement_instruction() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=201, payload=_vpn_receipt_payload()))
    client = ToriiClient("http://node.test", session=session)
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
    assert receipt.earned_fee_nanos == 25
    assert receipt.refunded_fee_nanos == 75
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
    client = ToriiClient("http://node.test", session=session)

    peers = client.list_peers()

    assert len(peers) == 2
    assert peers[0].address == "127.0.0.1:1337"
    assert peers[0].public_key_hex == "ed01"
    assert session.calls == [
        {
            "method": "GET",
            "url": "http://node.test/v1/peers",
            "params": {},
            "headers": {},
            "data": None,
        }
    ]


def test_deploy_contract_encodes_alias_first_payload_and_parses_response() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload={
                "ok": True,
                "bundle_name": "single-contract-deploy",
                "bundle_digest": "mock-bundle-digest",
                "chain_fingerprint": "mock-chain@height-0",
                "dry_run": False,
                "completed_stages": ["plan", "deploy"],
                "failure_point": None,
                "contracts": [
                    {
                        "name": "router::universal",
                        "contract_alias": "router::universal",
                        "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                        "previous_contract_address": None,
                        "kaizen": False,
                        "dataspace": "universal",
                        "deploy_nonce": 7,
                        "tx_hash_hex": "11" * 32,
                        "pipeline_status": {
                            "hash": "11" * 32,
                            "status": {
                                "kind": "Queued",
                                "block_height": None,
                                "rejection_reason": None,
                            },
                            "summary": "Queued",
                            "diagnostics": [],
                            "scope": "local",
                            "resolved_from": "queue",
                        },
                        "code_hash_hex": "22" * 32,
                        "abi_hash_hex": "33" * 32,
                        "status": "submitted",
                    }
                ],
                "hajimari_calls": [],
                "assertions": [],
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.deploy_contract(
        authority=CANONICAL_OWNER,
        private_key="00" * 32,
        code_b64="AQID",
        contract_alias="router::universal",
        lease_expiry_ms=1234,
    )

    assert isinstance(result, ContractDeployResponse)
    assert result is not None
    assert result.bundle_digest == "mock-bundle-digest"
    assert result.contracts[0].contract_alias == "router::universal"
    assert result.contracts[0].deploy_nonce == 7
    assert result.contracts[0].pipeline_status is not None
    assert result.contracts[0].pipeline_status.status.kind == "Queued"
    assert len(session.calls) == 1
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"] == "http://node.test/v1/contracts/deploy"
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload == {
        "authority": CANONICAL_OWNER,
        "private_key": "00" * 32,
        "code_b64": "AQID",
        "contract_alias": "router::universal",
        "lease_expiry_ms": 1234,
    }


def test_deploy_contract_rejects_retired_init_calls_response_field() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload={
                "ok": True,
                "bundle_name": "single-contract-deploy",
                "bundle_digest": "mock-bundle-digest",
                "chain_fingerprint": "mock-chain@height-0",
                "dry_run": False,
                "completed_stages": ["plan", "deploy"],
                "failure_point": None,
                "contracts": [],
                "init_calls": [],
                "assertions": [],
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match=r"contract deploy response\.hajimari_calls"):
        client.deploy_contract(
            authority=CANONICAL_OWNER,
            private_key="00" * 32,
            code_b64="AQID",
            contract_alias="router::universal",
        )


def test_deploy_contract_posts_fee_sponsor_metadata() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload={
                "ok": True,
                "bundle_name": "single-contract-deploy",
                "bundle_digest": "mock-bundle-digest",
                "chain_fingerprint": "mock-chain@height-0",
                "dry_run": False,
                "completed_stages": ["plan", "deploy"],
                "failure_point": None,
                "contracts": [],
                "hajimari_calls": [],
                "assertions": [],
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.deploy_contract(
        authority=CANONICAL_OWNER,
        private_key="00" * 32,
        code_b64="AQID",
        contract_alias="router::universal",
        gas_asset_id="xor#sora",
        fee_sponsor=CANONICAL_OWNER,
        gas_limit=10_000_000,
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["gas_asset_id"] == "xor#sora"
    assert payload["fee_sponsor"] == CANONICAL_OWNER
    assert payload["gas_limit"] == 10_000_000

    with pytest.raises(ValueError, match="deploy_contract.fee_sponsor"):
        client.deploy_contract(
            authority=CANONICAL_OWNER,
            private_key="00" * 32,
            code_b64="AQID",
            contract_alias="router::universal",
            fee_sponsor="bad sponsor",
        )


def test_call_contract_posts_selector_payload_and_parses_response() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload={
                "ok": True,
                "submitted": True,
                "dataspace": "universal",
                "code_hash_hex": "22" * 32,
                "abi_hash_hex": "33" * 32,
                "creation_time_ms": 42,
                "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                "tx_hash_hex": "44" * 32,
                "pipeline_status": {
                    "hash": "44" * 32,
                    "status": {
                        "kind": "Rejected",
                        "block_height": 12,
                        "rejection_reason": {
                            "Validation": "missing permission",
                        },
                    },
                    "summary": "Rejected: missing permission",
                    "diagnostics": [
                        {
                            "category": "validation",
                            "code": "validation",
                            "message": "missing permission",
                            "decoded_reason": "missing permission",
                            "raw_reason": "Validation(missing permission)",
                        }
                    ],
                    "scope": "local",
                    "resolved_from": "state",
                },
                "entrypoint": "ping",
                "transaction_ttl_ms": 60_000,
                "entrypoint_hash_hex": "55" * 32,
                "transaction_scaffold_b64": "AQID",
                "signed_transaction_b64": "BAUG",
                "signing_message_b64": "BwgJ",
                "operation_receipt": _contract_operation_receipt(),
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.call_contract(
        authority=CANONICAL_OWNER,
        private_key="00" * 32,
        contract_alias="router::universal",
        entrypoint="ping",
        payload={"value": 1, "labels": ["alpha"]},
        gas_asset_id="xor#wonderland",
        gas_limit=5000,
    )

    assert isinstance(result, ContractCallResponse)
    assert result.entrypoint == "ping"
    assert result.creation_time_ms == 42
    assert result.transaction_ttl_ms == 60_000
    assert result.entrypoint_hash_hex == "55" * 32
    assert isinstance(result.operation_receipt, ContractOperationReceipt)
    assert result.operation_receipt.gas_limit == 5000
    assert result.operation_receipt.payload_digest_hex == "66" * 32
    assert result.pipeline_status is not None
    assert result.pipeline_status.is_rejected
    assert result.pipeline_status.primary_diagnostic is not None
    assert result.pipeline_status.primary_diagnostic.decoded_reason == "missing permission"
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload == {
        "authority": CANONICAL_OWNER,
        "private_key": "00" * 32,
        "contract_alias": "router::universal",
        "entrypoint": "ping",
        "payload": {"value": 1, "labels": ["alpha"]},
        "gas_asset_id": "xor#wonderland",
        "gas_limit": 5000,
    }


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
            payload={
                "ok": True,
                "submitted": False,
                "dataspace": "universal",
                "code_hash_hex": "22" * 32,
                "abi_hash_hex": "33" * 32,
                "creation_time_ms": 1,
                "entrypoint": boundary["entrypoint"],
                "operation_receipt": _contract_operation_receipt(
                    entrypoint=boundary["entrypoint"],
                    gas_limit=boundary["gas_limit"],
                ),
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.call_contract(
        authority=boundary["authority"],
        private_key="fixture-private-key",
        contract_alias=boundary["contract_alias"],
        entrypoint=boundary["entrypoint"],
        payload=boundary["payload"],
        gas_limit=boundary["gas_limit"],
    )

    submitted = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert submitted == {
        "authority": boundary["authority"],
        "private_key": "fixture-private-key",
        "contract_alias": boundary["contract_alias"],
        "entrypoint": boundary["entrypoint"],
        "payload": boundary["payload"],
        "gas_limit": boundary["gas_limit"],
    }
    assert "argument_record" not in submitted
    assert "argument_record_norito_hex" not in submitted


def test_call_contract_posts_fee_sponsor_and_rejects_adversarial_sponsor() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload={
                "ok": True,
                "submitted": True,
                "dataspace": "universal",
                "code_hash_hex": "22" * 32,
                "abi_hash_hex": "33" * 32,
                "creation_time_ms": 42,
                "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                "tx_hash_hex": "44" * 32,
                "entrypoint": "ping",
                "operation_receipt": _contract_operation_receipt(),
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.call_contract(
        authority=CANONICAL_OWNER,
        private_key="00" * 32,
        contract_alias="router::is",
        entrypoint="ping",
        payload={},
        gas_limit=5000,
        fee_sponsor=CANONICAL_OWNER,
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["fee_sponsor"] == CANONICAL_OWNER
    assert payload["contract_alias"] == "router::is"

    with pytest.raises(ValueError, match="call_contract.fee_sponsor"):
        client.call_contract(
            authority=CANONICAL_OWNER,
            private_key="00" * 32,
            contract_alias="router::is",
            entrypoint="ping",
            gas_limit=5000,
            fee_sponsor="bad sponsor",
        )


def test_call_contract_rejects_missing_entrypoint_and_non_positive_gas_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    for entrypoint in ("", "   "):
        with pytest.raises(ValueError, match="call_contract.entrypoint"):
            client.call_contract(
                authority=CANONICAL_OWNER,
                private_key="00" * 32,
                contract_alias="router::universal",
                entrypoint=entrypoint,
                gas_limit=1,
            )
    for gas_limit in (0, -1):
        with pytest.raises(ValueError, match="call_contract.gas_limit must be positive"):
            client.call_contract(
                authority=CANONICAL_OWNER,
                private_key="00" * 32,
                contract_alias="router::universal",
                entrypoint="ping",
                gas_limit=gas_limit,
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
                "signing_message_b64": "AQID",
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.propose_multisig(
        multisig_account_alias="cbdc@banka",
        signer_account_id=CANONICAL_OWNER,
        instructions=[instruction],
        creation_time_ms=123,
        fee_sponsor=CANONICAL_OWNER,
    )

    assert isinstance(result, MultisigResponse)
    assert result.ok is True
    assert result.resolved_multisig_account_id == CANONICAL_OWNER
    assert result.submitted is False
    assert result.instructions_hash == proposal_id
    assert result.signing_message_b64 == "AQID"
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
        "fee_sponsor": CANONICAL_OWNER,
    }


def test_multisig_instruction_b64_validates_inputs() -> None:
    assert ToriiClient.multisig_instruction_b64(b"\x01\x02") == "AQI="
    assert ToriiClient.multisig_instruction_b64("AQI=") == "AQI="
    with pytest.raises(RuntimeError, match="valid base64"):
        ToriiClient.multisig_instruction_b64("not base64")
    with pytest.raises(RuntimeError, match="must not be empty"):
        ToriiClient.multisig_instruction_b64(b"")


def test_propose_multisig_rejects_adversarial_request_shapes() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())
    kwargs = {
        "signer_account_id": CANONICAL_OWNER,
        "instructions": [b"\x01"],
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
        )
    with pytest.raises(ValueError, match="must not be empty"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[],
        )
    with pytest.raises((RuntimeError, ValueError), match="valid base64|exact standard-base64"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
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
                signature_b64=signature_b64,
            )
    with pytest.raises(RuntimeError, match="64 hex"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            public_key_hex="aa",
        )
    with pytest.raises(ValueError, match="non-negative"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
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
        )

    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "resolved_multisig_account_id": CANONICAL_OWNER,
                "signing_message_b64": "not base64",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises(RuntimeError, match="valid base64"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
        )

    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "resolved_multisig_account_id": CANONICAL_OWNER,
                "signing_message_b64": "",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    with pytest.raises(RuntimeError, match="empty bytes"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
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
        )


def test_call_contract_rejects_ambiguous_selector() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())

    with pytest.raises(ValueError, match="exactly one of contract_address or contract_alias"):
        client.call_contract(
            authority=CANONICAL_OWNER,
            private_key="00" * 32,
            contract_address="tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
            contract_alias="router::universal",
            entrypoint="ping",
            gas_limit=1,
        )


def test_call_contract_rejects_padded_selectors_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="call_contract\\.contract_address must not contain surrounding whitespace"):
        client.call_contract(
            authority=CANONICAL_OWNER,
            private_key="00" * 32,
            contract_address=" tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
            entrypoint="ping",
            gas_limit=1,
        )

    with pytest.raises(ValueError, match="call_contract\\.contract_alias must not contain surrounding whitespace"):
        client.call_contract(
            authority=CANONICAL_OWNER,
            private_key="00" * 32,
            contract_alias="router::universal ",
            entrypoint="ping",
            gas_limit=1,
        )

    assert session.calls == []


def test_get_governance_contract_parses_response() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "found": True,
                "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                "dataspace": "universal",
                "code_hash_hex": "22" * 32,
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.get_governance_contract(
        "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
    )

    assert isinstance(result, GovernanceContractResponse)
    assert result.found is True
    assert result.code_hash_hex == "22" * 32
    assert session.calls[0]["url"] == (
        "http://node.test/v1/gov/contracts/"
        "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
    )


@pytest.mark.parametrize(
    "alias",
    ["", "zk", "plain", "ZK", "PLAIN", " Zk", "Plain ", "quadratic"],
)
def test_propose_contract_deploy_rejects_noncanonical_voting_mode(alias: str) -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="exactly 'Zk' or 'Plain'"):
        client.propose_contract_deploy(
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
    client = ToriiClient("http://node.test", session=session)

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

    capabilities = client.get_node_capabilities()

    assert capabilities.abi_version == 1
    assert capabilities.data_model_version == 1
    assert capabilities.crypto.sm.allowed_signing == ["sm2"]
    assert capabilities.crypto.sm.acceleration.neon_sm3 is True
    assert capabilities.crypto.curves.registry_version == 2
    assert capabilities.crypto.curves.allowed_curve_bitmap == [32770]



def test_contract_helpers_against_mock_server() -> None:
    server = ToriiMockServer().start()
    contract_address = "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
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
                "contract_deploy_response": {
                    "ok": True,
                    "bundle_name": "single-contract-deploy",
                    "bundle_digest": "mock-single-contract-digest",
                    "chain_fingerprint": "mock-chain@height-0",
                    "dry_run": False,
                    "completed_stages": ["plan", "deploy"],
                    "failure_point": None,
                    "contracts": [
                        {
                            "name": "router::universal",
                            "contract_alias": "router::universal",
                            "contract_address": contract_address,
                            "previous_contract_address": None,
                            "kaizen": False,
                            "dataspace": "universal",
                            "deploy_nonce": 1,
                            "tx_hash_hex": "11" * 32,
                            "code_hash_hex": "22" * 32,
                            "abi_hash_hex": "33" * 32,
                            "status": "submitted",
                        }
                    ],
                    "hajimari_calls": [],
                    "assertions": [],
                },
                "contract_call_response": {
                    "ok": True,
                    "submitted": True,
                    "dataspace": "universal",
                    "code_hash_hex": "22" * 32,
                    "abi_hash_hex": "33" * 32,
                    "creation_time_ms": 9,
                    "contract_address": contract_address,
                    "tx_hash_hex": "44" * 32,
                    "entrypoint": "ping",
                    "transaction_ttl_ms": 60_000,
                    "entrypoint_hash_hex": "55" * 32,
                    "transaction_scaffold_b64": "AQID",
                    "signed_transaction_b64": "BAUG",
                    "signing_message_b64": "BwgJ",
                    "operation_receipt": _contract_operation_receipt(),
                },
            },
            timeout=5.0,
        )
        response.raise_for_status()

        client = ToriiClient(server.base_url)
        deploy = client.deploy_contract(
            authority=CANONICAL_OWNER,
            private_key="00" * 32,
            code_b64="AQID",
            contract_alias="router::universal",
        )
        call = client.call_contract(
            authority=CANONICAL_OWNER,
            private_key="00" * 32,
            contract_address=contract_address,
            entrypoint="ping",
            payload={"value": 1},
            gas_limit=5000,
        )
        governed = client.get_governance_contract(contract_address)

        assert deploy is not None
        assert deploy.contracts[0].contract_address == contract_address
        assert call.contract_address == contract_address
        assert governed.contract_address == contract_address
        assert governed.code_hash_hex == "22" * 32
    finally:
        server.stop()


def test_mock_server_seeds_sumeragi_status_snapshot() -> None:
    server = ToriiMockServer().start()
    try:
        response = requests.get(f"{server.base_url.rstrip('/')}/v1/sumeragi/status", timeout=5.0)
        response.raise_for_status()

        payload = response.json()

        assert payload["protocol_version"] == 2
        assert payload["leader"] == 1
        assert payload["height_context"]["validator_count"] == 4
        assert payload["safety_halt"]["active"] is False
        assert payload["operator"]["tx_queue"]["capacity"] == 32
        assert payload["committed_lane_blocks"] == []
    finally:
        server.stop()


def test_get_sumeragi_status_parses_authoritative_v2_snapshot() -> None:
    payload = _sumeragi_v2_status_payload()
    payload["lane_settlement_commitments"] = [_lane_settlement_payload()]
    status = _get_sumeragi_status(payload)

    assert status.protocol_version == 2
    assert status.height == 10
    assert status.phase == "prepare"
    assert status.height_context.mode == "permissioned"
    assert status.height_context.min_signers == 3
    assert status.last_commit_qc is not None
    assert status.last_commit_qc.certificate.round.height == 9
    assert status.last_commit_qc.signed_power == 3
    assert status.safety_halt.active is False
    assert status.safety_halt.height == 0
    assert status.operator.view_change_install_total == 7
    assert status.operator.busy_deferral_total == 3
    assert status.operator.tx_queue.queued_transactions == 3
    assert status.lane_payload_ownerships == []
    settlement = status.lane_settlement_commitments[0]
    assert settlement["total_local_micro"] == "10"
    assert settlement["swap_metadata"]["liquidity_profile"]["profile"] == "Tier1"


def test_get_sumeragi_status_parses_and_validates_safety_halt() -> None:
    payload = _sumeragi_v2_status_payload()
    payload["safety_halt"] = {
        "active": True,
        "reason": "conflicting_commit_qc",
        "height": 9,
        "epoch": 1,
        "first_block_hash": _canonical_hash(0x71),
        "conflicting_block_hash": _canonical_hash(0x73),
    }
    safety_halt = _get_sumeragi_status(payload).safety_halt
    assert safety_halt.active is True
    assert safety_halt.reason == "conflicting_commit_qc"
    assert safety_halt.first_block_hash == _canonical_hash(0x71)
    assert safety_halt.conflicting_block_hash == _canonical_hash(0x73)

    missing = _sumeragi_v2_status_payload()
    del missing["safety_halt"]
    with pytest.raises(RuntimeError, match="safety_halt must be a JSON object"):
        _get_sumeragi_status(missing)

    unknown = _sumeragi_v2_status_payload()
    unknown["safety_halt"]["legacy_reason_code"] = 7
    with pytest.raises(RuntimeError, match="safety_halt contains unknown field"):
        _get_sumeragi_status(unknown)

    malformed_hash = _sumeragi_v2_status_payload()
    malformed_hash["safety_halt"]["first_block_hash"] = "not-a-canonical-hash"
    with pytest.raises(RuntimeError, match="first_block_hash must be a canonical hash"):
        _get_sumeragi_status(malformed_hash)


def test_get_sumeragi_status_parses_exact_nested_fee_and_native_amx_receipts() -> None:
    payload = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    settlement["nexus_fee_receipts"] = [_nexus_fee_receipt_payload()]
    settlement["native_amx_receipts"] = [_native_amx_receipt_payload()]
    payload["lane_settlement_commitments"] = [settlement]

    parsed = _get_sumeragi_status(payload).lane_settlement_commitments[0]

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
    assert (
        leg["participant_settlement_hash"]
        == leg["commit_qc"]["body"]["participant_settlement_commitment"]
    )
    assert leg["participant_settlement"]["block_height"] == 8
    assert len(leg["participant_settlement"]["receipts"]) == 2
    assert leg["prepare_qc"]["body"]["source_id"] == "AB" * 32
    assert leg["prepare_qc"]["body"]["tx_entrypoint_hash"] == _canonical_hash(0x61)


def test_get_sumeragi_status_accepts_first_native_amx_participant_block() -> None:
    payload = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
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
    settlement["native_amx_receipts"] = [native]
    payload["lane_settlement_commitments"] = [settlement]

    parsed_leg = _get_sumeragi_status(payload).lane_settlement_commitments[0][
        "native_amx_receipts"
    ][0]["legs"][0]

    assert parsed_leg["prepare_qc"]["body"]["participant_previous_block_descriptor_hash"] is None
    assert (
        "previous_lane_block_descriptor_hash"
        not in parsed_leg["participant_proposal"]["descriptor"]
    )


def test_get_sumeragi_status_accepts_mixed_role_proposal_without_current_entrypoint() -> None:
    payload = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    leg = native["legs"][0]
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
    descriptor["accepted_transaction_hashes"] = [_canonical_hash(0x77)]
    leg["participant_settlement"]["lane_id"] = 2
    leg["participant_settlement"]["dataspace_id"] = 7
    leg["participant_settlement"]["lane_incarnation"] = _canonical_hash(0x51)
    settlement["native_amx_receipts"] = [native]
    payload["lane_settlement_commitments"] = [settlement]

    parsed_leg = _get_sumeragi_status(payload).lane_settlement_commitments[0][
        "native_amx_receipts"
    ][0]["legs"][0]

    assert parsed_leg["participant_proposal"]["descriptor"][
        "accepted_transaction_hashes"
    ] == [_canonical_hash(0x77)]


def test_get_sumeragi_status_rejects_native_amx_participant_finality_tampering() -> None:
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

    def add_payload_hint(leg: Dict[str, Any]) -> None:
        leg["participant_proposal"]["payload_block_hint"] = None

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
        leg["participant_settlement"]["total_local_micro"] = "1"

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
        add_payload_hint,
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
        payload = _sumeragi_v2_status_payload()
        settlement = _lane_settlement_payload()
        native = _native_amx_receipt_payload()
        mutate(native["legs"][0])
        settlement["native_amx_receipts"] = [native]
        payload["lane_settlement_commitments"] = [settlement]
        with pytest.raises(RuntimeError, match="."):
            _get_sumeragi_status(payload)


@pytest.mark.parametrize("invalid", [7, "01", str(1 << 128)])
def test_get_sumeragi_status_rejects_noncanonical_u128_json(invalid: Any) -> None:
    payload = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    settlement["total_local_micro"] = invalid
    payload["lane_settlement_commitments"] = [settlement]

    with pytest.raises(RuntimeError, match="total_local_micro.*(?:canonical|u128)"):
        _get_sumeragi_status(payload)


def test_get_sumeragi_status_rejects_noncanonical_fixed_hex_and_nested_unknown_fields() -> None:
    lowercase = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    fee = _nexus_fee_receipt_payload()
    fee["source_id"] = "ab" * 32
    settlement["nexus_fee_receipts"] = [fee]
    lowercase["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="source_id.*uppercase"):
        _get_sumeragi_status(lowercase)

    unknown_fee = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    fee = _nexus_fee_receipt_payload()
    fee["schedule"]["legacy_rate"] = "1"
    settlement["nexus_fee_receipts"] = [fee]
    unknown_fee["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="schedule contains unknown field legacy_rate"):
        _get_sumeragi_status(unknown_fee)

    unknown_amx = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"][0]["prepare_qc"]["body"]["legacy_round"] = 1
    settlement["native_amx_receipts"] = [native]
    unknown_amx["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="body contains unknown field legacy_round"):
        _get_sumeragi_status(unknown_amx)


def test_get_sumeragi_status_rejects_nested_receipt_coordinate_and_qc_tampering() -> None:
    wrong_coordinate = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    fee = _nexus_fee_receipt_payload()
    fee["block_height"] = 8
    settlement["nexus_fee_receipts"] = [fee]
    wrong_coordinate["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="receipt coordinates do not match"):
        _get_sumeragi_status(wrong_coordinate)

    under_quorum = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"][0]["prepare_qc"]["signers_bitmap"] = [0x03]
    settlement["native_amx_receipts"] = [native]
    under_quorum["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="signers_bitmap does not meet quorum"):
        _get_sumeragi_status(under_quorum)

    malformed_pop = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"][0]["commit_qc"]["validator_set_pops"][0] = [1] * 95
    settlement["native_amx_receipts"] = [native]
    malformed_pop["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match=r"validator_set_pops\[0\].*96"):
        _get_sumeragi_status(malformed_pop)

    mismatched_phase_identity = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"][0]["commit_qc"]["body"]["plan_digest"] = _canonical_hash(0x70)
    settlement["native_amx_receipts"] = [native]
    mismatched_phase_identity["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="prepare and commit identities differ"):
        _get_sumeragi_status(mismatched_phase_identity)


def test_get_sumeragi_status_rejects_bounded_vector_overflow_before_nested_decode() -> None:
    too_many_settlements = _sumeragi_v2_status_payload()
    too_many_settlements["lane_settlement_commitments"] = [{}] * 129
    with pytest.raises(RuntimeError, match="lane_settlement_commitments exceeds"):
        _get_sumeragi_status(too_many_settlements)

    too_many_relays = _sumeragi_v2_status_payload()
    too_many_relays["lane_relay_envelopes"] = [{}] * 65
    with pytest.raises(RuntimeError, match="lane_relay_envelopes exceeds"):
        _get_sumeragi_status(too_many_relays)

    too_many_legs = _sumeragi_v2_status_payload()
    settlement = _lane_settlement_payload()
    native = _native_amx_receipt_payload()
    native["legs"] = native["legs"] * 256
    settlement["native_amx_receipts"] = [native]
    too_many_legs["lane_settlement_commitments"] = [settlement]
    with pytest.raises(RuntimeError, match="legs exceeds"):
        _get_sumeragi_status(too_many_legs)


def test_get_sumeragi_status_rejects_protocol_context_and_commit_tampering() -> None:
    legacy_field = _sumeragi_v2_status_payload()
    legacy_field["mode_tag"] = "retired"
    with pytest.raises(RuntimeError, match="unknown field mode_tag"):
        _get_sumeragi_status(legacy_field)

    wrong_version = _sumeragi_v2_status_payload()
    wrong_version["protocol_version"] = 1
    with pytest.raises(RuntimeError, match="protocol_version must equal 2"):
        _get_sumeragi_status(wrong_version)

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

    underpowered = _sumeragi_v2_status_payload()
    underpowered["last_commit_qc"]["signed_power"] = 2
    with pytest.raises(RuntimeError, match="does not satisfy its frozen dual quorum"):
        _get_sumeragi_status(underpowered)


def test_get_sumeragi_status_rejects_impossible_queue_bounds() -> None:
    adapter_overflow = _sumeragi_v2_status_payload()
    adapter_overflow["operator"]["adapter_queues"]["ingress_keys"] = 17
    with pytest.raises(RuntimeError, match="adapter_queues occupancy exceeds capacity"):
        _get_sumeragi_status(adapter_overflow)

    tx_overflow = _sumeragi_v2_status_payload()
    tx_overflow["operator"]["tx_queue"]["queued_transactions"] = 6
    with pytest.raises(RuntimeError, match="tx_queue occupancy exceeds capacity"):
        _get_sumeragi_status(tx_overflow)

    zero_capacity = _sumeragi_v2_status_payload()
    zero_capacity["operator"]["tx_queue"]["max_retained_bytes"] = 0
    with pytest.raises(RuntimeError, match="max_retained_bytes must be positive"):
        _get_sumeragi_status(zero_capacity)


@pytest.mark.parametrize(
    "field",
    [
        "lane_settlement_commitments",
        "lane_relay_envelopes",
        "lane_payload_ownerships",
        "committed_lane_blocks",
        "lane_block_sessions",
    ],
)
def test_get_sumeragi_status_requires_all_canonical_lane_arrays(field: str) -> None:
    payload = _sumeragi_v2_status_payload()
    del payload[field]

    with pytest.raises(RuntimeError, match=rf"{field} must be an array"):
        _get_sumeragi_status(payload)


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


def test_contract_bundle_helpers_against_mock_server() -> None:
    server = ToriiMockServer().start()
    try:
        response = requests.post(
            f"{server.base_url.rstrip('/')}/__mock__/contracts/config",
            json={
                "bundle_response": {
                    "bundle_digest": "mock-bundle-digest",
                    "completed_stages": ["plan", "deploy", "hajimari_calls", "assertions"],
                }
            },
            timeout=5.0,
        )
        response.raise_for_status()

        request_payload = {
            "bundle_name": "demo",
            "authority": CANONICAL_OWNER,
            "private_key": "00" * 32,
            "contracts": [
                {
                    "name": "demo.greeter",
                    "contract_alias": "greeter::universal",
                    "code_b64": "AQID",
                    "depends_on": [],
                }
            ],
            "hajimari_calls": [
                {
                    "id": "seed",
                    "contract_alias": "greeter::universal",
                    "entrypoint": "hajimari",
                    "gas_limit": 1000,
                }
            ],
            "assertions": [
                {
                    "id": "status",
                    "contract_alias": "greeter::universal",
                    "entrypoint": "status",
                    "gas_limit": 1000,
                    "expected_result": 7,
                }
            ],
        }

        dry_run = requests.post(
            f"{server.base_url.rstrip('/')}/v1/contracts/deploy-bundle?dry_run=true",
            json=request_payload,
            timeout=5.0,
        )
        dry_run.raise_for_status()
        dry_run_payload = dry_run.json()
        assert dry_run_payload["bundle_name"] == "demo"
        assert dry_run_payload["dry_run"] is True
        assert dry_run_payload["contracts"][0]["status"] == "planned"
        assert dry_run_payload["hajimari_calls"][0]["status"] == "pending"

        submit = requests.post(
            f"{server.base_url.rstrip('/')}/v1/contracts/deploy-bundle",
            json=request_payload,
            timeout=5.0,
        )
        submit.raise_for_status()
        submit_payload = submit.json()
        assert submit_payload["dry_run"] is False
        assert submit_payload["contracts"][0]["status"] == "deployed"
        assert submit_payload["contracts"][0]["tx_hash_hex"] == "01" * 32

        status = requests.get(
            f"{server.base_url.rstrip('/')}/v1/contracts/deploy-bundles/mock-bundle-digest",
            timeout=5.0,
        )
        status.raise_for_status()
        status_payload = status.json()
        assert status_payload["bundle_digest"] == "mock-bundle-digest"
        assert status_payload["completed_stages"] == [
            "plan",
            "deploy",
            "hajimari_calls",
            "assertions",
        ]
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

    snapshot = client.get_runtime_abi_active()

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

    metrics = client.get_runtime_metrics()

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


def test_publish_space_directory_manifest_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=202, payload={"queued": True}))
    client = ToriiClient("http://node.test", session=session)

    manifest: Dict[str, Any] = {
        "version": "V1",
        "uaid": "uaid:" + "11" * 32,
        "dataspace": 7,
        "entries": [{"scope": {"program": "cbdc.transfer"}, "effect": {"Allow": {"max_amount": "10"}}}],
    }
    response = client.publish_space_directory_manifest(
        authority=CANONICAL_OWNER,
        private_key="ed25519:AAAA",
        manifest=manifest,
        reason="demo",
    )

    assert response == {"queued": True}
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"].endswith("/v1/space-directory/manifests")
    assert session.calls[0]["headers"]["Content-Type"] == "application/json"
    body = json.loads(session.calls[0]["data"])
    assert body["authority"] == CANONICAL_OWNER
    assert body["reason"] == "demo"
    assert body["manifest"]["entries"][0]["scope"]["program"] == "cbdc.transfer"

    manifest["entries"][0]["scope"]["program"] = "mutated"
    assert body["manifest"]["entries"][0]["scope"]["program"] == "cbdc.transfer"


def test_revoke_space_directory_manifest_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=202))
    client = ToriiClient("http://node.test", session=session)

    result = client.revoke_space_directory_manifest(
        authority=CANONICAL_OWNER,
        private_key="ed25519:BBBB",
        uaid="UAID:" + "23" * 32,
        dataspace=3,
        revoked_epoch=4096,
        reason="audit",
    )

    assert result is None
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/space-directory/manifests/revoke")
    payload = json.loads(call["data"])
    assert payload["uaid"] == "uaid:" + "23" * 32
    assert payload["dataspace"] == 3
    assert payload["revoked_epoch"] == 4096
    assert payload["reason"] == "audit"


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


def test_get_sumeragi_qc_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "highest_qc": {"height": 10, "view": 2, "subject_block_hash": "aa11"},
                "locked_qc": {"height": 9, "view": 1, "subject_block_hash": None},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_sumeragi_qc()

    assert snapshot.highest_qc.height == 10
    assert snapshot.locked_qc.subject_block_hash is None
    assert session.calls[0]["url"].endswith("/v1/sumeragi/qc")


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
                    "sponsorship_enabled": False,
                    "sponsor_max_fee": "0",
                    "sponsor_verified_balance_safety_floor": "0",
                    "canonical_sponsor_account_id": None,
                    "fee_receipts_activation_height": 7,
                    "external_settlement_enabled": False,
                    "burn_from_unix_timestamp_ms": 0,
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
    client = ToriiClient("http://node.test", session=session)

    preflight = client.get_pipeline_preflight()
    status = client.get_status_snapshot().status

    assert preflight.schema_version == 1
    assert preflight.chain_height == 42
    assert preflight.sumeragi.stall_threshold_ms == 6_000
    assert preflight.admission.max_tx_bytes == 1_048_576
    assert preflight.pipeline.signature_batch_max_ed25519 == 64
    assert preflight.queue.queued == 1
    assert preflight.fees.base_fee == "0"
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


def test_get_sumeragi_phases_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "propose_ms": 1,
                "collect_da_ms": 2,
                "collect_prevote_ms": 3,
                "collect_precommit_ms": 4,
                "collect_aggregator_ms": 5,
                "commit_ms": 8,
                "pipeline_total_ms": 9,
                "collect_aggregator_gossip_total": 10,
                "block_created_dropped_by_lock_total": 11,
                "block_created_hint_mismatch_total": 12,
                "block_created_proposal_mismatch_total": 13,
                "ema_ms": {
                    "propose_ms": 14,
                    "collect_da_ms": 15,
                    "collect_prevote_ms": 16,
                    "collect_precommit_ms": 17,
                    "collect_aggregator_ms": 18,
                    "commit_ms": 21,
                    "pipeline_total_ms": 22,
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    phases = client.get_sumeragi_phases()

    assert phases.collect_aggregator_ms == 5
    assert phases.ema_ms.commit_ms == 21
    assert session.calls[0]["url"].endswith("/v1/sumeragi/phases")


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
    client = ToriiClient("http://node.test", session=session)

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
    client = ToriiClient("http://node.test", session=session)

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
    client = ToriiClient("http://node.test", session=session)

    mapping = client.get_sumeragi_bls_keys()

    assert mapping["ed01"] == "ff00"
    assert mapping["ed02"] is None
    assert session.calls[0]["url"].endswith("/v1/sumeragi/bls-keys")


def test_get_sumeragi_evidence_count_returns_int() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"count": 42}))
    client = ToriiClient("http://node.test", session=session)

    count = client.get_sumeragi_evidence_count()

    assert count == 42
    assert session.calls[0]["url"].endswith("/v1/sumeragi/evidence/count")


def test_list_sumeragi_evidence_parses_records() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 3,
                "items": [
                    {
                        "kind": "DoublePrepare",
                        "recorded_height": 1,
                        "recorded_view": 2,
                        "recorded_ms": 3,
                        "phase": "Prepare",
                        "height": 4,
                        "view": 5,
                        "epoch": 6,
                        "signer": "ed011122",
                        "block_hash_1": "aa11",
                        "block_hash_2": "bb22",
                    },
                    {
                        "kind": "InvalidProposal",
                        "recorded_height": 7,
                        "recorded_view": 8,
                        "recorded_ms": 9,
                        "height": 10,
                        "view": 11,
                        "epoch": 12,
                        "subject_block_hash": "cc33",
                        "payload_hash": "dd44",
                        "reason": "payload mismatch",
                    },
                    {
                        "kind": "UnknownEvidence",
                        "recorded_height": 13,
                        "recorded_view": 14,
                        "recorded_ms": 15,
                        "detail": "unknown entry",
                    },
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_sumeragi_evidence(limit=5, offset=1, kind="DoublePrepare")

    assert page.total == 3
    assert len(page.items) == 3
    prevote = page.items[0]
    assert prevote.kind == "DoublePrepare"
    assert prevote.phase == "Prepare"
    assert prevote.block_hash_1 == "aa11"
    assert prevote.block_hash_2 == "bb22"
    invalid_proposal = page.items[1]
    assert invalid_proposal.payload_hash == "dd44"
    assert invalid_proposal.reason == "payload mismatch"
    unknown = page.items[2]
    assert unknown.kind == "UnknownEvidence"
    assert unknown.detail == "unknown entry"
    call = session.calls[0]
    assert call["url"].endswith("/v1/sumeragi/evidence")
    assert call["params"] == {"limit": 5, "offset": 1, "kind": "DoublePrepare"}


def test_list_sumeragi_evidence_validates_limit() -> None:
    client = ToriiClient("http://node.test")

    try:
        client.list_sumeragi_evidence(limit=2000)
    except RuntimeError as exc:
        assert "limit must be <= 1000" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for oversized limit")


def test_set_confidential_gas_schedule_reuses_logger() -> None:
    session = RecordingSession()
    config_payload = {
        "public_key": "ed0123",
        "logger": {"level": "Info", "filter": "mod=warn"},
        "network": {
            "block_gossip_size": 32,
            "block_gossip_period_ms": 100,
            "transaction_gossip_size": 16,
            "transaction_gossip_period_ms": 50,
        },
        "queue": {"capacity": 2048},
        "confidential_gas": {
            "proof_base": 1,
            "per_public_input": 1,
            "per_proof_byte": 1,
            "per_nullifier": 1,
            "per_commitment": 1,
        },
    }
    session.queue(StubResponse(payload=config_payload))
    session.queue(StubResponse(status_code=202))
    client = ToriiClient("http://node.test", session=session)

    client.set_confidential_gas_schedule(
        proof_base=9,
        per_public_input=8,
        per_proof_byte=7,
        per_nullifier=6,
        per_commitment=5,
    )

    assert len(session.calls) == 2
    assert session.calls[1]["method"] == "POST"
    body = json.loads(session.calls[1]["data"])
    assert body["logger"] == {"level": "Info", "filter": "mod=warn"}
    assert body["confidential_gas"]["per_nullifier"] == 6


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
    client = ToriiClient("http://node.test", session=session)

    status = client.get_time_status()

    assert status.peers == 2
    assert len(status.samples) == 2
    assert status.samples[0].peer == "peer-a"
    assert status.rtt_buckets[1].upper_bound_ms == 50
    assert status.rtt_sum_ms == 28
    assert status.note == "NTS running"


def test_list_kaigi_relays_parses_summary() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 1,
                "items": [
                    {
                        "relay_id": "relay-alpha",
                        "domain": "kaigi.core",
                        "bandwidth_class": 3,
                        "hpke_fingerprint_hex": "ab" * 32,
                        "status": "healthy",
                        "reported_at_ms": 123,
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    summary = client.list_kaigi_relays()

    assert summary.total == 1
    assert len(summary.items) == 1
    relay = summary.items[0]
    assert relay.relay_id == "relay-alpha"
    assert relay.status == "healthy"
    assert session.calls[0]["url"].endswith("/v1/kaigi/relays")
    assert session.calls[0]["headers"]["Accept"] == "application/json"


def test_get_kaigi_relay_returns_detail_and_none_on_404() -> None:
    relay_id = CANONICAL_OWNER
    session = RecordingSession()
    session.queue(StubResponse(status_code=404))
    session.queue(
        StubResponse(
            payload={
                "relay": {
                    "relay_id": relay_id,
                    "domain": "kaigi.core",
                    "bandwidth_class": 3,
                    "hpke_fingerprint_hex": "cd" * 32,
                },
                "hpke_public_key_b64": "QUJDRA==",
                "reported_call": {"domain_id": "kaigi.core", "call_name": "register"},
                "reported_by": "ops@example",
                "notes": "Primary relay",
                "metrics": {
                    "domain": "kaigi.core",
                    "registrations_total": 5,
                    "manifest_updates_total": 7,
                    "failovers_total": 1,
                    "health_reports_total": 9,
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    assert client.get_kaigi_relay(relay_id) is None
    detail = client.get_kaigi_relay(relay_id)

    assert detail is not None
    assert detail.relay.domain == "kaigi.core"
    assert detail.metrics is not None and detail.metrics.failovers_total == 1
    assert detail.reported_call is not None
    assert detail.reported_call.call_name == "register"
    assert session.calls[1]["url"].endswith(f"/v1/kaigi/relays/{quote(relay_id, safe='')}")


def test_get_kaigi_relays_health_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "healthy_total": 2,
                "degraded_total": 1,
                "unavailable_total": 0,
                "reports_total": 5,
                "registrations_total": 7,
                "failovers_total": 1,
                "domains": [
                    {
                        "domain": "kaigi.core",
                        "registrations_total": 5,
                        "manifest_updates_total": 3,
                        "failovers_total": 1,
                        "health_reports_total": 4,
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_kaigi_relays_health()

    assert snapshot.healthy_total == 2
    assert snapshot.domains[0].domain == "kaigi.core"
    assert session.calls[0]["url"].endswith("/v1/kaigi/relays/health")


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

    draft = client.finalize_referendum(referendum_id="ref-1", proposal_id="a" * 64)

    assert draft.ok is True
    assert len(draft.tx_instructions) == 1
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/gov/finalize")
    assert json.loads(call["data"]) == {
        "referendum_id": "ref-1",
        "proposal_id": "a" * 64,
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


def test_get_connect_status_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "enabled": True,
                "sessions_total": 5,
                "sessions_active": 3,
                "per_ip_sessions": [{"ip": "192.0.2.1", "sessions": 2}],
                "buffered_sessions": 1,
                "total_buffer_bytes": 42,
                "dedupe_size": 7,
                "frames_in_total": 10,
                "frames_out_total": 11,
                "ciphertext_total": 12,
                "dedupe_drops_total": 0,
                "buffer_drops_total": 0,
                "plaintext_control_drops_total": 0,
                "monotonic_drops_total": 0,
                "sequence_violation_closes_total": 1,
                "role_direction_mismatch_total": 2,
                "ping_miss_total": 0,
                "p2p_rebroadcasts_total": 3,
                "p2p_rebroadcast_skipped_total": 4,
                "p2p_auth_failures_total": 5,
                "p2p_ttl_drops_total": 6,
                "p2p_unknown_session_drops_total": 7,
                "p2p_session_claims_in_total": 8,
                "p2p_session_claims_installed_total": 9,
                "p2p_session_claim_conflicts_total": 10,
                "p2p_role_consumed_total": 11,
                "p2p_session_terminated_total": 12,
                "policy": {
                    "relay_enabled": True,
                    "relay_strategy": "broadcast",
                    "relay_effective_strategy": "local_only",
                    "relay_p2p_attached": False,
                    "p2p_ttl_hops": 2,
                    "ws_max_sessions": 32,
                    "session_ttl_ms": 10000,
                    "heartbeat_interval_ms": 5000,
                    "heartbeat_miss_tolerance": 3,
                    "heartbeat_min_interval_ms": 1000,
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_connect_status()

    assert snapshot.enabled is True
    assert snapshot.sessions_total == 5
    assert snapshot.per_ip_sessions[0].ip == "192.0.2.1"
    assert snapshot.policy is not None
    assert snapshot.policy.ws_max_sessions == 32
    assert snapshot.policy.relay_strategy == "broadcast"
    assert snapshot.policy.p2p_ttl_hops == 2
    assert snapshot.sequence_violation_closes_total == 1
    assert snapshot.p2p_auth_failures_total == 5
    assert snapshot.p2p_session_claims_installed_total == 9
    assert snapshot.p2p_session_terminated_total == 12
    assert snapshot.policy.heartbeat_interval_ms == 5000
    assert session.calls[0]["url"].endswith("/v1/connect/status")


def test_create_and_delete_connect_session() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "sid": "abc",
                "wallet_uri": "iroha://wallet",
                "app_uri": "iroha://app",
                "token_app": "app-token",
                "token_wallet": "wallet-token",
                "token_management": "management-token",
                "token_relay": "relay-token",
                "ttl": 30,
            }
        )
    )
    session.queue(StubResponse(status_code=204))
    client = ToriiClient("http://node.test", session=session)

    session_info = client.create_connect_session({"scope": "demo"})
    deleted = client.delete_connect_session("abc", session_info.token_management)

    assert session_info.sid == "abc"
    assert session_info.token_relay == "relay-token"
    assert session_info.extra["ttl"] == 30
    assert deleted is True
    post_call = session.calls[0]
    assert post_call["method"] == "POST"
    assert post_call["url"].endswith("/v1/connect/session")
    assert json.loads(post_call["data"]) == {"scope": "demo"}
    delete_call = session.calls[1]
    assert delete_call["headers"] == {"Authorization": "Bearer management-token"}


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


def _offline_active_transfer_verifier(**overrides: Any) -> Dict[str, Any]:
    verifier = {
        "id": {"backend": "halo2/ipa", "name": "asset-transfer-v2"},
        "version": 7,
        "circuit_id": "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
        "commitment": "44" * 32,
        "public_inputs_schema_hash": "55" * 32,
        "max_proof_bytes": 4096,
        "activation_height": 1,
        "withdrawal_height": None,
    }
    verifier.update(overrides)
    return verifier


def _offline_active_topup_shield_verifier(**overrides: Any) -> Dict[str, Any]:
    verifier = _offline_active_transfer_verifier(
        id={"backend": "halo2/ipa", "name": "asset-topup-shield-v2"},
        circuit_id=(
            "halo2/pasta/ipa/"
            "kagemusha-topup-shield-merkle16-axiom-poseidon-v3"
        ),
        commitment="66" * 32,
        public_inputs_schema_hash="77" * 32,
    )
    verifier.update(overrides)
    return verifier


def _offline_active_unshield_verifier(**overrides: Any) -> Dict[str, Any]:
    verifier = _offline_active_transfer_verifier(
        id={"backend": "halo2/ipa", "name": "confidential_unshield_v3_verifier_record"},
        circuit_id=(
            "halo2/pasta/ipa/"
            "confidential-unshield-change-merkle16-axiom-poseidon-v4"
        ),
        commitment="88" * 32,
        public_inputs_schema_hash="89" * 32,
    )
    verifier.update(overrides)
    return verifier


def _offline_active_recursive_step_eq_verifier(**overrides: Any) -> Dict[str, Any]:
    verifier = _offline_active_transfer_verifier(
        id={
            "backend": "halo2/ipa",
            "name": "kagemusha_recursive_step_eq_v3_verifier_record",
        },
        circuit_id="kagemusha-recursive-spend-step-eq-two-parent-exact-state-v1",
        commitment="99" * 32,
        public_inputs_schema_hash="9a" * 32,
    )
    verifier.update(overrides)
    return verifier


def _offline_active_recursive_step_ep_verifier(**overrides: Any) -> Dict[str, Any]:
    verifier = _offline_active_transfer_verifier(
        id={
            "backend": "halo2/ipa",
            "name": "kagemusha_recursive_step_ep_v3_verifier_record",
        },
        circuit_id="kagemusha-recursive-spend-step-ep-two-parent-exact-state-v1",
        commitment="aa" * 32,
        public_inputs_schema_hash="ab" * 32,
    )
    verifier.update(overrides)
    return verifier


def _offline_readiness_payload(**overrides: Any) -> Dict[str, Any]:
    payload = {
        "required_bridge_abi_version": 19,
        "max_hops": 8,
        "asset_definition_id": CANONICAL_ASSET_DEFINITION_ID,
        "asset_scale": 4,
        "evaluated_block_height": 42,
        "evaluated_block_hash": "ab" * 32,
        "active_transfer_verifier": _offline_active_transfer_verifier(),
        "active_topup_shield_verifier": _offline_active_topup_shield_verifier(),
        "active_unshield_verifier": _offline_active_unshield_verifier(),
        "active_recursive_step_eq_verifier": (
            _offline_active_recursive_step_eq_verifier()
        ),
        "active_recursive_step_ep_verifier": _offline_active_recursive_step_ep_verifier(),
        "proof_backend_available": True,
        "recursive_lineage_supported": True,
        "ready": True,
        "blockers": [],
    }
    payload.update(overrides)
    return payload


OFFLINE_OPERATION_BYTES = [0x11] * 32
OFFLINE_OPERATION_ID = "11" * 32
OFFLINE_TRANSACTION_HASH = "22" * 32
OFFLINE_STATUS_URI = f"/v1/offline/operations/{OFFLINE_OPERATION_ID}"


def _offline_top_up_request(
    *,
    norito: bytes = b"kagemusha-top-up-v2\x00\x01\x02",
    operation_id: str = OFFLINE_OPERATION_ID,
) -> KagemushaTopUpRequestV2:
    return KagemushaTopUpRequestV2(norito=norito, operation_id=operation_id)


def _offline_redeem_request(
    *,
    norito: bytes = b"kagemusha-redeem-v2\x03\x04\x05",
    operation_id: str = OFFLINE_OPERATION_ID,
) -> KagemushaRedeemRequestV2:
    return KagemushaRedeemRequestV2(norito=norito, operation_id=operation_id)


def _offline_operation_reference(**overrides: Any) -> Dict[str, Any]:
    reference = {
        "operation_id": OFFLINE_OPERATION_ID,
        "kind": {"kind": "top_up", "value": None},
        "state": {"state": "pending", "value": None},
        "transaction_hash": OFFLINE_TRANSACTION_HASH,
        "status_uri": OFFLINE_STATUS_URI,
        "submitted_at_ms": 1_725_000_000_123,
    }
    reference.update(overrides)
    return reference


def _offline_fixed_bytes(byte: int) -> List[int]:
    return [byte] * 32


def _offline_top_up_anchor(**overrides: Any) -> Dict[str, Any]:
    amount = overrides.get("amount", {"atomic_units": 17, "scale": 4})
    current_note = overrides.get(
        "current_note",
        {
            "chain_id": "wonderland",
            "asset": CANONICAL_ASSET_ID,
            "note_commitment": _offline_fixed_bytes(0x41),
            "spend_nullifier": _offline_fixed_bytes(0x51),
            "amount": dict(amount),
        },
    )
    anchor = {
        "version": 2,
        "chain_id": "wonderland",
        "payer": CANONICAL_OWNER,
        "asset": CANONICAL_ASSET_ID,
        "asset_scale": amount["scale"],
        "amount": amount,
        "initial_root": _offline_fixed_bytes(0x10),
        "finalized_root": _offline_fixed_bytes(0x20),
        "shield_leaf_index": 7,
        "current_note": current_note,
        "topup_operation_id": list(OFFLINE_OPERATION_BYTES),
        "shield_verifier_id": {
            "backend": "halo2/ipa",
            "name": "asset-topup-shield-v2",
        },
        "shield_verifier_commitment": _offline_fixed_bytes(0x61),
        "artifact_binding": {
            "generation": "generation-1",
            "manifest_sha256": _offline_fixed_bytes(0x81),
        },
        "finalized_height": 12,
        "finalized_tx_hash": _offline_fixed_bytes(0x22),
        "anchor_digest": _offline_fixed_bytes(0x71),
    }
    anchor.update(overrides)
    return anchor


def _offline_top_up_finality_proof(
    anchor: Optional[Mapping[str, Any]] = None,
    *,
    finalized_height: int = 12,
    **overrides: Any,
) -> Dict[str, Any]:
    bound_anchor = anchor if anchor is not None else _offline_top_up_anchor()
    proof = {
        "version": 1,
        "anchor": {
            "topup_operation_id": list(
                bound_anchor.get("topup_operation_id", OFFLINE_OPERATION_BYTES)
            ),
            "anchor_digest": list(
                bound_anchor.get("anchor_digest", _offline_fixed_bytes(0x71))
            ),
        },
        "commit_qc": {
            "height_context": {
                "height": finalized_height,
                "opaque_context": {"protocol_version": 2},
            },
            "certificate": {
                "round": {"height": finalized_height, "view": 0},
                "opaque_certificate": [1, 2, 3],
            },
        },
        "anchor_path": {"leaf_index": 0, "leaf_count": 1, "siblings": []},
    }
    proof.update(overrides)
    return proof


def _offline_applied_top_up_status(
    anchor: Optional[Mapping[str, Any]] = None,
    **result_overrides: Any,
) -> Dict[str, Any]:
    finalized_height = result_overrides.get("finalized_block_height", 12)
    bound_anchor = dict(anchor if anchor is not None else _offline_top_up_anchor())
    result = {
        "transaction_hash": OFFLINE_TRANSACTION_HASH,
        "finalized_block_height": finalized_height,
        "server_time_ms": 13,
        "anchor": bound_anchor,
        "finality_proof": _offline_top_up_finality_proof(
            bound_anchor,
            finalized_height=finalized_height,
        ),
    }
    result.update(result_overrides)
    return {
        "state": "applied",
        "value": {
            "operation_id": OFFLINE_OPERATION_ID,
            "result": {"kind": "top_up", "result": result},
        },
    }


def _offline_rejected_status(error: Mapping[str, Any]) -> Dict[str, Any]:
    return {
        "state": "rejected",
        "value": {
            "operation_id": OFFLINE_OPERATION_ID,
            "kind": {"kind": "redeem", "value": None},
            "transaction_hash": OFFLINE_TRANSACTION_HASH,
            "error": dict(error),
        },
    }


def test_offline_public_request_annotations_are_closed_first_release_types() -> None:
    assert get_type_hints(ToriiClient.submit_kagemusha_top_up)["request"] is KagemushaTopUpRequestV2
    assert get_type_hints(ToriiClient.submit_kagemusha_redeem)["request"] is KagemushaRedeemRequestV2
    assert get_args(OfflineAssetScale) == tuple(range(29))


def test_get_kagemusha_readiness_sends_exact_asset_selector_and_parses_blockers() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=_offline_readiness_payload(
                ready=False,
                blockers=[
                    {
                        "code": "issuer_unavailable",
                        "message": "Issuer unavailable",
                    }
                ],
            )
        )
    )
    client = ToriiClient("http://node.test", session=session)

    readiness = client.get_kagemusha_readiness(CANONICAL_ASSET_DEFINITION_ID)

    assert readiness.asset_definition_id == CANONICAL_ASSET_DEFINITION_ID
    assert readiness.required_bridge_abi_version == 19
    assert readiness.max_hops == 8
    assert readiness.asset_scale == 4
    assert readiness.evaluated_block_height == 42
    assert readiness.evaluated_block_hash == "ab" * 32
    assert readiness.active_transfer_verifier is not None
    assert readiness.active_transfer_verifier.id.backend == "halo2/ipa"
    assert readiness.active_transfer_verifier.max_proof_bytes == 4096
    assert readiness.active_topup_shield_verifier is not None
    assert readiness.active_topup_shield_verifier.id.name == "asset-topup-shield-v2"
    assert readiness.active_unshield_verifier is not None
    assert readiness.active_recursive_step_eq_verifier is not None
    assert readiness.active_recursive_step_ep_verifier is not None
    assert readiness.proof_backend_available is True
    assert readiness.recursive_lineage_supported is True
    assert readiness.ready is False
    assert readiness.blockers[0].code == "issuer_unavailable"
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/offline/readiness")
    assert call["params"] == {"asset_definition_id": CANONICAL_ASSET_DEFINITION_ID}
    assert call["headers"]["Accept"] == "application/json"


def test_get_kagemusha_readiness_resolves_alias_to_canonical_asset_id() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=_offline_readiness_payload()))
    readiness = ToriiClient("http://node.test", session=session).get_kagemusha_readiness(
        "xor#sora"
    )

    assert readiness.asset_definition_id == CANONICAL_ASSET_DEFINITION_ID
    assert session.calls[0]["params"] == {"asset_definition_id": "xor#sora"}


def test_get_kagemusha_readiness_rejects_invalid_selector_before_network() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)
    for asset in (
        "",
        "different-asset",
        "XOR#sora",
        f" {CANONICAL_ASSET_DEFINITION_ID}",
        f"{CANONICAL_ASSET_DEFINITION_ID} ",
    ):
        with pytest.raises(RuntimeError, match="asset_definition_id"):
            client.get_kagemusha_readiness(asset)
    assert session.calls == []


def test_get_kagemusha_readiness_rejects_adversarial_snapshots() -> None:
    missing_hash = _offline_readiness_payload()
    missing_hash.pop("evaluated_block_hash")
    missing_scale = _offline_readiness_payload()
    missing_scale.pop("asset_scale")
    missing_verifier = _offline_readiness_payload()
    missing_verifier.pop("active_transfer_verifier")
    missing_topup_shield_verifier = _offline_readiness_payload()
    missing_topup_shield_verifier.pop("active_topup_shield_verifier")
    missing_unshield_verifier = _offline_readiness_payload()
    missing_unshield_verifier.pop("active_unshield_verifier")
    missing_recursive_step_eq_verifier = _offline_readiness_payload()
    missing_recursive_step_eq_verifier.pop("active_recursive_step_eq_verifier")
    missing_recursive_step_ep_verifier = _offline_readiness_payload()
    missing_recursive_step_ep_verifier.pop("active_recursive_step_ep_verifier")
    payloads = [
        missing_hash,
        missing_scale,
        missing_verifier,
        missing_topup_shield_verifier,
        missing_unshield_verifier,
        missing_recursive_step_eq_verifier,
        missing_recursive_step_ep_verifier,
        _offline_readiness_payload(unexpected_field="not-part-of-readiness"),
        _offline_readiness_payload(required_bridge_abi_version=17),
        _offline_readiness_payload(max_hops=9),
        _offline_readiness_payload(asset_definition_id="different-asset"),
        _offline_readiness_payload(
            ready=True,
            blockers=[{"code": "not_ready", "message": "no"}],
        ),
        _offline_readiness_payload(ready=False, blockers=[]),
        _offline_readiness_payload(evaluated_block_height=-1),
        _offline_readiness_payload(evaluated_block_height=1 << 64),
        _offline_readiness_payload(evaluated_block_hash="AB" * 32),
        _offline_readiness_payload(evaluated_block_hash="ab" * 31),
        _offline_readiness_payload(blockers=[{"code": "NOT-CANONICAL", "message": "no"}]),
        _offline_readiness_payload(
            ready=False, blockers=[{"code": "not_ready", "message": ""}]
        ),
        _offline_readiness_payload(
            ready=False, blockers=[{"code": "not_ready", "message": " leading"}]
        ),
        _offline_readiness_payload(
            ready=False, blockers=[{"code": "not_ready", "message": "line\nbreak"}]
        ),
        _offline_readiness_payload(asset_scale=-1),
        _offline_readiness_payload(asset_scale=1 << 32),
        _offline_readiness_payload(asset_scale=29),
        _offline_readiness_payload(
            asset_scale=None,
            ready=False,
            blockers=[{"code": "not_ready", "message": "missing scale"}],
        ),
        _offline_readiness_payload(active_transfer_verifier=None),
        _offline_readiness_payload(active_topup_shield_verifier=None),
        _offline_readiness_payload(active_unshield_verifier=None),
        _offline_readiness_payload(active_recursive_step_eq_verifier=None),
        _offline_readiness_payload(active_recursive_step_ep_verifier=None),
        _offline_readiness_payload(proof_backend_available=False),
        _offline_readiness_payload(recursive_lineage_supported=False),
        _offline_readiness_payload(
            active_unshield_verifier=_offline_active_unshield_verifier(
                circuit_id="kagemusha-recursive-spend-step-ep-two-parent-exact-state-v1"
            )
        ),
        _offline_readiness_payload(
            active_recursive_step_ep_verifier=_offline_active_recursive_step_ep_verifier(
                commitment="99" * 32
            )
        ),
        _offline_readiness_payload(
            active_transfer_verifier=_offline_active_transfer_verifier(
                max_proof_bytes=0
            )
        ),
        _offline_readiness_payload(
            active_transfer_verifier=_offline_active_transfer_verifier(
                activation_height=43
            )
        ),
        _offline_readiness_payload(
            active_transfer_verifier=_offline_active_transfer_verifier(
                withdrawal_height=42
            )
        ),
        _offline_readiness_payload(
            active_transfer_verifier=_offline_active_transfer_verifier(
                commitment="AA" * 32
            )
        ),
        _offline_readiness_payload(
            active_topup_shield_verifier=_offline_active_topup_shield_verifier(
                max_proof_bytes=0
            )
        ),
        _offline_readiness_payload(
            active_topup_shield_verifier=_offline_active_topup_shield_verifier(
                activation_height=43
            )
        ),
        _offline_readiness_payload(
            active_topup_shield_verifier=_offline_active_topup_shield_verifier(
                withdrawal_height=42
            )
        ),
        _offline_readiness_payload(
            ready=False,
            blockers=[
                {
                    "code": "topup_shield_verifier_unavailable",
                    "message": "top-up shield verifier unavailable",
                }
            ],
        ),
        _offline_readiness_payload(
            ready=False,
            blockers=[
                {"code": "not_ready", "message": "one"},
                {"code": "not_ready", "message": "two"},
            ],
        ),
    ]
    for payload in payloads:
        session = RecordingSession()
        session.queue(StubResponse(payload=payload))
        client = ToriiClient("http://node.test", session=session)
        with pytest.raises(RuntimeError):
            client.get_kagemusha_readiness(CANONICAL_ASSET_DEFINITION_ID)


def test_kagemusha_readiness_rejects_unknown_members_and_accepts_exact_unavailability() -> None:
    unknown_root = _offline_readiness_payload(unknown_member={"attacker_controlled": True})
    unknown_verifier = _offline_readiness_payload(
        active_transfer_verifier=_offline_active_transfer_verifier(
            ignored_verifier_field=True
        )
    )
    unknown_blocker = _offline_readiness_payload(
        ready=False,
        blockers=[
            {
                "code": "issuer_unavailable",
                "message": "issuer unavailable",
                "future": True,
            }
        ],
    )
    for payload in (unknown_root, unknown_verifier, unknown_blocker):
        session = RecordingSession()
        session.queue(StubResponse(payload=payload))
        with pytest.raises(RuntimeError, match="first-release contract"):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_readiness(CANONICAL_ASSET_DEFINITION_ID)

    exact_numeric_session = RecordingSession()
    exact_numeric_json = json.dumps(_offline_readiness_payload())[:-1]
    exact_numeric_json += ',"future_numeric":[1.25,1e400]}'
    exact_numeric_session.queue(
        StubResponse(text=exact_numeric_json, headers={"Content-Type": "application/json"})
    )
    with pytest.raises(RuntimeError, match="first-release contract"):
        ToriiClient(
            "http://node.test", session=exact_numeric_session
        ).get_kagemusha_readiness(CANONICAL_ASSET_DEFINITION_ID)

    expected_unavailable_session = RecordingSession()
    expected_unavailable_session.queue(
        StubResponse(
            payload=_offline_readiness_payload(
                asset_scale=29,
                ready=False,
                blockers=[
                    {
                        "code": "asset_scale_unsupported",
                        "message": "unsupported scale",
                    }
                ],
            )
        )
    )
    expected_unavailable = ToriiClient(
        "http://node.test", session=expected_unavailable_session
    ).get_kagemusha_readiness(CANONICAL_ASSET_DEFINITION_ID)
    assert expected_unavailable.asset_scale == 29
    assert expected_unavailable.active_transfer_verifier is not None

    topup_unavailable_session = RecordingSession()
    topup_unavailable_session.queue(
        StubResponse(
            payload=_offline_readiness_payload(
                active_topup_shield_verifier=None,
                ready=False,
                blockers=[
                    {
                        "code": "topup_shield_verifier_unavailable",
                        "message": "top-up shield verifier unavailable",
                    }
                ],
            )
        )
    )
    topup_unavailable = ToriiClient(
        "http://node.test", session=topup_unavailable_session
    ).get_kagemusha_readiness(CANONICAL_ASSET_DEFINITION_ID)
    assert topup_unavailable.active_topup_shield_verifier is None

    for code in ("", "_leading_underscore", "a" * 65):
        invalid_session = RecordingSession()
        invalid_session.queue(
            StubResponse(
                payload=_offline_readiness_payload(
                    ready=False,
                    blockers=[{"code": code, "message": "no"}],
                )
            )
        )
        with pytest.raises(RuntimeError):
            ToriiClient(
                "http://node.test", session=invalid_session
            ).get_kagemusha_readiness(CANONICAL_ASSET_DEFINITION_ID)


def test_submit_kagemusha_top_up_sends_exact_norito_and_idempotency_key() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(),
            headers={"Location": OFFLINE_STATUS_URI},
        )
    )
    client = ToriiClient("http://node.test", session=session)

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
            headers={"Location": OFFLINE_STATUS_URI},
        )
    )
    client = ToriiClient("http://node.test", session=session)

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

    for request_type in (KagemushaTopUpRequestV2, KagemushaRedeemRequestV2):
        with pytest.raises(ValueError, match="must not be empty"):
            request_type(norito=b"", operation_id=OFFLINE_OPERATION_ID)
        with pytest.raises(ValueError, match="exceeds"):
            request_type(norito=b"x" * (256 * 1024 + 1), operation_id=OFFLINE_OPERATION_ID)
        for norito in (bytearray(b"x"), memoryview(b"x"), "x"):
            with pytest.raises(TypeError, match="immutable bytes"):
                request_type(norito=norito, operation_id=OFFLINE_OPERATION_ID)  # type: ignore[arg-type]
        for operation_id in (
            "0" * 64,
            "11" * 31,
            "11" * 33,
            "AA" * 32,
            "gg" * 32,
            f" {OFFLINE_OPERATION_ID}",
        ):
            with pytest.raises(RuntimeError, match="operation_id"):
                request_type(norito=b"x", operation_id=operation_id)
    assert session.calls == []


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
    ]
    for payload, location in cases:
        session = RecordingSession()
        headers = {"Location": location} if location is not None else {}
        session.queue(StubResponse(status_code=202, payload=payload, headers=headers))
        client = ToriiClient("http://node.test", session=session)
        with pytest.raises(RuntimeError):
            client.submit_kagemusha_top_up(_offline_top_up_request())

    wrong_media_session = RecordingSession()
    wrong_media_session.queue(
        StubResponse(
            status_code=202,
            payload=_offline_operation_reference(),
            headers={"Content-Type": "text/plain", "Location": OFFLINE_STATUS_URI},
        )
    )
    wrong_media_client = ToriiClient("http://node.test", session=wrong_media_session)
    with pytest.raises(RuntimeError, match="Content-Type application/json"):
        wrong_media_client.submit_kagemusha_top_up(_offline_top_up_request())


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
                        "details": {"layer": "torii"},
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
    assert typed_anchor.version == 2
    assert typed_anchor.amount.scale == 4
    assert typed_anchor.shield_leaf_index == 7
    assert typed_anchor.shield_verifier_id.backend == "halo2/ipa"
    assert typed_anchor.artifact_binding.generation == "generation-1"
    assert typed_anchor.artifact_binding.manifest_sha256 == tuple(
        _offline_fixed_bytes(0x81)
    )
    assert typed_anchor.topup_operation_id == tuple(OFFLINE_OPERATION_BYTES)


def test_offline_top_up_finality_proof_is_direct_typed_and_preserves_opaque_internals() -> None:
    proof = _offline_top_up_finality_proof()
    proof["commit_qc"]["future_qc_field"] = {"opaque": [7, 8, 9]}
    proof["anchor_path"]["future_path_field"] = "preserved"
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
    assert typed_proof.commit_qc["future_qc_field"] == {"opaque": [7, 8, 9]}
    assert typed_proof.anchor_path["future_path_field"] == "preserved"

    proof["commit_qc"]["future_qc_field"]["opaque"][0] = 255
    assert typed_proof.commit_qc["future_qc_field"] == {"opaque": [7, 8, 9]}


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
        _offline_top_up_anchor(version=1),
        _offline_top_up_anchor(asset_scale=29),
        _offline_top_up_anchor(asset_scale=3),
        _offline_top_up_anchor(finalized_root=_offline_fixed_bytes(0x10)),
        _offline_top_up_anchor(shield_leaf_index=-1),
        _offline_top_up_anchor(shield_leaf_index=1 << 16),
        _offline_top_up_anchor(topup_operation_id=_offline_fixed_bytes(0x12)),
        _offline_top_up_anchor(finalized_height=11),
        _offline_top_up_anchor(finalized_tx_hash=_offline_fixed_bytes(0x23)),
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
                "generation": "é" * 65,
                "manifest_sha256": _offline_fixed_bytes(0x81),
            }
        ),
        _offline_top_up_anchor(
            artifact_binding={
                "generation": "generation-1",
                "manifest_sha256": _offline_fixed_bytes(0),
            }
        ),
        _offline_top_up_anchor(
            artifact_binding={"generation": "generation-1"}
        ),
        _offline_top_up_anchor(
            current_note={
                "chain_id": "wonderland",
                "asset": CANONICAL_ASSET_ID,
                "note_commitment": _offline_fixed_bytes(0x41),
                "spend_nullifier": _offline_fixed_bytes(0x41),
                "amount": {"atomic_units": 17, "scale": 4},
            }
        ),
        _offline_top_up_anchor(
            current_note={
                "chain_id": "other-chain",
                "asset": CANONICAL_ASSET_ID,
                "note_commitment": _offline_fixed_bytes(0x41),
                "spend_nullifier": _offline_fixed_bytes(0x51),
                "amount": {"atomic_units": 17, "scale": 4},
            }
        ),
        _offline_top_up_anchor(
            current_note={
                "chain_id": "wonderland",
                "asset": "different-asset",
                "note_commitment": _offline_fixed_bytes(0x41),
                "spend_nullifier": _offline_fixed_bytes(0x51),
                "amount": {"atomic_units": 17, "scale": 4},
            }
        ),
        _offline_top_up_anchor(
            current_note={
                "chain_id": "wonderland",
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
                anchor = _offline_top_up_anchor(
                    finalized_height=result["finalized_block_height"]
                )
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


def test_offline_error_codes_use_the_global_finite_grammar() -> None:
    accepted_session = RecordingSession()
    accepted_session.queue(
        StubResponse(
            payload=_offline_rejected_status(
                {"code": "1_future_code", "message": "future rejection"}
            )
        )
    )
    accepted = ToriiClient(
        "http://node.test", session=accepted_session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)
    assert isinstance(accepted, OfflineRejectedOperation)
    assert accepted.error.code == "1_future_code"

    for code in ("", "_leading_underscore", "a" * 65):
        session = RecordingSession()
        session.queue(
            StubResponse(
                payload=_offline_rejected_status(
                    {"code": code, "message": "invalid code"}
                )
            )
        )
        with pytest.raises(RuntimeError):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_error_messages_require_exact_non_control_text() -> None:
    for message in ("", " leading", "trailing ", "line\nbreak", "control\u0085"):
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


def test_offline_error_details_are_closed_and_typed() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=_offline_rejected_status(
                {
                    "code": "offline_operation_rejected",
                    "message": "rejected",
                    "unknown_envelope_member": "ignored",
                    "details": {
                        "layer": "torii",
                        "reject_code": "QUEUE_FULL",
                        "retry_after_seconds": 3,
                        "endpoint": "/v1/offline/redeem",
                        "field": "authorization",
                        "expected": "fresh",
                        "actual": "replayed",
                        "profile": "minamoto",
                        "chain_discriminant": 753,
                        "tx_hash": OFFLINE_TRANSACTION_HASH,
                        "last_status": "queued",
                        "hint": "retry later",
                        "unknown_detail": {"attacker_controlled": True},
                        "queue": {
                            "state": "saturated",
                            "queued": 5,
                            "capacity": 5,
                            "saturated": True,
                            "unknown_queue_member": "ignored",
                        },
                        "axt": {
                            "code": "handle_era_stale",
                            "reason": "stale handle era",
                            "snapshot_version": 7,
                            "dataspace": 8,
                            "lane": 9,
                            "next_min_handle_era": 10,
                            "next_min_sub_nonce": 11,
                            "unknown_axt_member": "ignored",
                        },
                    },
                }
            )
        )
    )
    status = ToriiClient(
        "http://node.test", session=session
    ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)
    assert isinstance(status, OfflineRejectedOperation)
    details = status.error.details
    assert details is not None
    assert details.layer == "torii"
    assert details.reject_code == "QUEUE_FULL"
    assert details.retry_after_seconds == 3
    assert details.chain_discriminant == 753
    assert details.queue is not None
    assert details.queue.queued == 5
    assert details.queue.saturated is True
    assert details.axt is not None
    assert details.axt.lane == 9
    assert details.axt.next_min_sub_nonce == 11
    assert not hasattr(details, "unknown_detail")
    assert not hasattr(details.queue, "unknown_queue_member")
    assert not hasattr(details.axt, "unknown_axt_member")


def test_offline_error_details_reject_malformed_nested_types_and_ranges() -> None:
    invalid_details = [
        {"queue": {"state": "healthy", "queued": 0, "capacity": 1}},
        {
            "queue": {
                "state": "healthy",
                "queued": -1,
                "capacity": 1,
                "saturated": False,
            }
        },
        {
            "queue": {
                "state": "healthy",
                "queued": 0,
                "capacity": 1,
                "saturated": "false",
            }
        },
        {"retry_after_seconds": -1},
        {"chain_discriminant": 65_536},
        {"axt": {"lane": 1 << 32}},
        {"axt": {"snapshot_version": "1"}},
        {"axt": []},
    ]
    for details in invalid_details:
        session = RecordingSession()
        session.queue(
            StubResponse(
                payload=_offline_rejected_status(
                    {"code": "rejected", "message": "no", "details": details}
                )
            )
        )
        with pytest.raises(RuntimeError):
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_operation_status(OFFLINE_OPERATION_ID)


def test_offline_json_decoder_rejects_duplicates_non_finite_depth_and_size() -> None:
    valid = json.dumps(_offline_readiness_payload())
    duplicate = valid.replace('"ready": true', '"ready": true, "ready": true')
    non_finite = valid.replace('"evaluated_block_height": 42', '"evaluated_block_height": NaN')
    infinity = valid.replace('"evaluated_block_height": 42', '"evaluated_block_height": Infinity')
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
            ToriiClient(
                "http://node.test", session=session
            ).get_kagemusha_readiness(CANONICAL_ASSET_DEFINITION_ID)


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


def test_decode_pdp_commitment_header_handles_mapping() -> None:
    payload = b"\x01\x02\x03"
    header_value = base64.b64encode(payload).decode("ascii")

    decoded = decode_pdp_commitment_header({"sora-pdp-commitment": header_value})

    assert decoded == payload


def test_decode_pdp_commitment_header_is_case_insensitive() -> None:
    payload = b"\xAA\xBB"
    header_value = base64.b64encode(payload).decode("ascii")

    decoded = decode_pdp_commitment_header({"Sora-PDP-Commitment": header_value})

    assert decoded == payload


def test_decode_pdp_commitment_header_rejects_invalid_payload() -> None:
    try:
        decode_pdp_commitment_header({"sora-pdp-commitment": "###"})
    except RuntimeError as exc:
        assert "Failed to decode" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for invalid header")


def test_decode_pdp_commitment_header_returns_none_when_missing() -> None:
    assert decode_pdp_commitment_header({}) is None
    assert decode_pdp_commitment_header(None) is None


def test_submit_zk_ballot_rejects_unsupported_public_inputs() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "accepted": True,
                "reason": None,
                "tx_instructions": [],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="durationBlocks"):
        client.submit_zk_ballot(
            authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
            chain_id="chain",
            election_id="election-1",
            proof_b64="AAAA",
            public={
                "owner": CANONICAL_OWNER,
                "amount": "100",
                "durationBlocks": 5,
            },
        )


def test_submit_zk_ballot_normalizes_public_inputs() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "accepted": True,
                "reason": None,
                "tx_instructions": [],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.submit_zk_ballot(
        authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        chain_id="chain",
        election_id="election-1",
        proof_b64="AAAA",
        public={
            "owner": CANONICAL_OWNER,
            "amount": "100",
            "duration_blocks": 5,
            "root_hint": f"0x{'Cc' * 32}",
            "nullifier": bytes.fromhex("DD" * 32),
        },
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    public = payload["public"]
    assert public["root_hint"] == "cc" * 32
    assert public["nullifier"] == "dd" * 32


def test_submit_zk_ballot_rejects_invalid_hex_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="root_hint"):
        client.submit_zk_ballot(
            authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
            chain_id="chain",
            election_id="election-1",
            proof_b64="AAAA",
            public={
                "owner": CANONICAL_OWNER,
                "amount": "100",
                "duration_blocks": 5,
                "root_hint": "not-hex",
            },
        )


def test_submit_zk_ballot_rejects_incomplete_lock_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="owner, amount, duration_blocks"):
        client.submit_zk_ballot(
            authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
            chain_id="chain",
            election_id="election-1",
            proof_b64="AAAA",
            public={"owner": CANONICAL_OWNER},
        )


def test_submit_zk_ballot_rejects_noncanonical_owner() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="canonical I105 account id"):
        client.submit_zk_ballot(
            authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
            chain_id="chain",
            election_id="election-1",
            proof_b64="AAAA",
            public={
                "owner": "soradead",
                "amount": "100",
                "duration_blocks": 5,
            },
        )


def test_submit_zk_ballot_v1_rejects_incomplete_lock_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="owner, amount, duration_blocks"):
        client.submit_zk_ballot_v1(
            authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
            chain_id="chain",
            election_id="election-1",
            backend="halo2/ipa",
            envelope_b64="AAAA",
            owner=CANONICAL_OWNER,
        )


def test_submit_zk_ballot_v1_rejects_noncanonical_owner() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="canonical I105 account id"):
        client.submit_zk_ballot_v1(
            authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
            chain_id="chain",
            election_id="election-1",
            backend="halo2/ipa",
            envelope_b64="AAAA",
            owner="soradead",
            amount="100",
            duration_blocks=5,
        )


def test_submit_zk_ballot_v1_normalizes_hex_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    client.submit_zk_ballot_v1(
        authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        chain_id="chain",
        election_id="election-1",
        backend="halo2/ipa",
        envelope_b64="AAAA",
        root_hint=f"0x{'Aa' * 32}",
        nullifier=f"blake2b32:{'BB' * 32}",
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["root_hint"] == "aa" * 32
    assert payload["nullifier"] == "bb" * 32


def test_submit_zk_ballot_v1_rejects_invalid_hex_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="root_hint"):
        client.submit_zk_ballot_v1(
            authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
            chain_id="chain",
            election_id="election-1",
            backend="halo2/ipa",
            envelope_b64="AAAA",
            root_hint="not-hex",
        )


def test_list_subscription_plans_encodes_params() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "plan_id": "plan#subs",
                        "plan": {"provider": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6", "pricing": {"kind": "fixed"}},
                    }
                ],
                "total": 1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_subscription_plans(provider="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6", limit=10, offset=5)

    assert page.total == 1
    assert page.items[0].plan_id == "plan#subs"
    assert page.items[0].plan["provider"] == "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"
    assert session.calls[0]["params"] == {
        "provider": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        "limit": 10,
        "offset": 5,
    }


def test_create_subscription_plan_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "plan_id": "plan#subs",
                "tx_hash_hex": "deadbeef",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.create_subscription_plan(
        authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        private_key="ed25519:priv",
        plan_id="plan#subs",
        plan={"provider": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"},
    )

    assert result.ok is True
    assert result.plan_id == "plan#subs"
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["authority"] == "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"
    assert payload["private_key"] == "ed25519:priv"
    assert payload["plan_id"] == "plan#subs"
    assert payload["plan"]["provider"] == "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"


def test_list_subscriptions_encodes_params() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "subscription_id": "sub-1$subscriptions",
                        "subscription": {"status": "active"},
                        "invoice": {"amount": "120"},
                        "plan": {"provider": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"},
                    }
                ],
                "total": 1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_subscriptions(
        owned_by="sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        provider="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        status="ACTIVE",
        limit=25,
        offset=0,
    )

    assert page.total == 1
    assert page.items[0].subscription_id == "sub-1$subscriptions"
    assert page.items[0].subscription["status"] == "active"
    assert session.calls[0]["params"] == {
        "owned_by": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        "provider": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        "status": "active",
        "limit": 25,
        "offset": 0,
    }


def test_list_subscriptions_rejects_invalid_status() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())

    with pytest.raises(ValueError, match="subscriptions.status"):
        client.list_subscriptions(status="unknown")


def test_create_subscription_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "subscription_id": "sub-1$subscriptions",
                "billing_trigger_id": "sub-bill",
                "usage_trigger_id": "sub-usage",
                "first_charge_ms": 1_704_067_200_000,
                "tx_hash_hex": "deadbeef",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.create_subscription(
        authority="sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        private_key="ed25519:priv",
        subscription_id="sub-1$subscriptions",
        plan_id="plan#subs",
        billing_trigger_id="sub-bill",
        usage_trigger_id="sub-usage",
        first_charge_ms=1_704_067_200_000,
        grant_usage_to_provider=True,
    )

    assert result.subscription_id == "sub-1$subscriptions"
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["authority"] == "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"
    assert payload["private_key"] == "ed25519:priv"
    assert payload["billing_trigger_id"] == "sub-bill"
    assert payload["usage_trigger_id"] == "sub-usage"
    assert payload["first_charge_ms"] == 1_704_067_200_000
    assert payload["grant_usage_to_provider"] is True


def test_get_subscription_encodes_path_and_parses_response() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "subscription_id": "sub-1$subscriptions",
                "subscription": {"status": "active"},
                "plan": {"provider": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    record = client.get_subscription("sub-1$subscriptions")

    assert record is not None
    assert record.subscription_id == "sub-1$subscriptions"
    assert session.calls[0]["url"].endswith("/v1/subscriptions/sub-1%24subscriptions")


def test_get_subscription_returns_none_on_404() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=404, payload=None))
    client = ToriiClient("http://node.test", session=session)

    assert client.get_subscription("sub-404$subscriptions") is None


def test_subscription_actions_post_payloads() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "a"}))
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "b"}))
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "c"}))
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "d"}))
    client = ToriiClient("http://node.test", session=session)

    client.pause_subscription("sub-1", authority="sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE", private_key="ed25519:priv")
    client.resume_subscription(
        "sub-1",
        authority="sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        private_key="ed25519:priv",
        charge_at_ms=1_704_067_200_000,
    )
    client.cancel_subscription("sub-1", authority="sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE", private_key="ed25519:priv")
    client.charge_subscription_now(
        "sub-1",
        authority="sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        private_key="ed25519:priv",
        charge_at_ms=1_704_067_200_000,
    )

    pause_body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert pause_body["authority"] == "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"
    resume_body = json.loads(session.calls[1]["data"].decode("utf-8"))
    assert resume_body["charge_at_ms"] == 1_704_067_200_000
    cancel_body = json.loads(session.calls[2]["data"].decode("utf-8"))
    assert cancel_body["private_key"] == "ed25519:priv"
    charge_body = json.loads(session.calls[3]["data"].decode("utf-8"))
    assert charge_body["charge_at_ms"] == 1_704_067_200_000


def test_record_subscription_usage_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "e"}))
    client = ToriiClient("http://node.test", session=session)

    result = client.record_subscription_usage(
        "sub-1",
        authority="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
        private_key="ed25519:priv",
        unit_key="compute_ms",
        delta=3600,
        usage_trigger_id="sub-usage",
    )

    assert result.ok is True
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["unit_key"] == "compute_ms"
    assert payload["delta"] == "3600"
    assert payload["usage_trigger_id"] == "sub-usage"
