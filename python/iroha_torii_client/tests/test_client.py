from __future__ import annotations

import base64
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Mapping, Optional, Union
from urllib.parse import quote

import pytest
import requests
from requests.structures import CaseInsensitiveDict

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import (  # noqa: E402  (import depends on sys.path mutation)
    ContractCallResponse,
    ContractDeployResponse,
    ExplorerAccountQr,
    GovernanceContractResponse,
    MultisigResponse,
    NetworkTimeSnapshot,
    NetworkTimeStatus,
    ToriiCanonicalRequestAuth,
    ToriiClient,
    VpnQuoteCreateRequest,
    VpnReceiptSubmitRequest,
    VpnSessionCreateRequest,
    canonical_request_signature_message,
    decode_pdp_commitment_header,
    inspect_i105_network_prefix,
)
from iroha_torii_client.client import _decode_i105_string  # noqa: E402
from iroha_torii_client.mock import ToriiMockServer  # noqa: E402

CANONICAL_OWNER = "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"
CANONICAL_ASSET_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
CANONICAL_ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
SCCP_TEST_MESSAGE_ID = "11" * 32
SCCP_TEST_COMMITMENT_ROOT = "33" * 32
SCCP_TEST_MESSAGE_BUNDLE = {
    "version": 1,
    "commitment_root": SCCP_TEST_COMMITMENT_ROOT,
    "commitment": {
        "version": 1,
        "kind": "Transfer",
        "target_domain": 5,
        "message_id": SCCP_TEST_MESSAGE_ID,
        "payload_hash": "22" * 32,
    },
}
SCCP_TEST_EVM_NETWORK_ID = "aa" * 32
SCCP_TEST_EVM_VERIFIER_ADDRESS = "bb" * 20
SCCP_TEST_EVM_BRIDGE_ADDRESS = "cc" * 20
SCCP_TEST_EVM_VERIFIER_CODE_HASH = "dd" * 32
SCCP_TEST_EVM_VERIFIER_KEY_HASH = "ee" * 32
SCCP_TEST_TRON_NETWORK_ID = "71" * 32
SCCP_TEST_TRON_VERIFIER_ADDRESS = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
SCCP_TEST_TRON_VERIFIER_CODE_HASH = "72" * 32
SCCP_TEST_TRON_VERIFIER_KEY_HASH = "73" * 32


def _sample_sccp_evm_destination_binding_hash(
    network_id: str = SCCP_TEST_EVM_NETWORK_ID,
    verifier_address: str = SCCP_TEST_EVM_VERIFIER_ADDRESS,
    bridge_address: str = SCCP_TEST_EVM_BRIDGE_ADDRESS,
    verifier_code_hash: str = SCCP_TEST_EVM_VERIFIER_CODE_HASH,
    verifier_key_hash: str = SCCP_TEST_EVM_VERIFIER_KEY_HASH,
) -> str:
    from iroha_torii_client.sccp import evm_sccp_destination_binding_hash

    return evm_sccp_destination_binding_hash(
        {
            "network_id_hex": f"0x{network_id}",
            "verifier_address_hex": f"0x{verifier_address}",
            "bridge_address_hex": f"0x{bridge_address}",
            "verifier_code_hash_hex": f"0x{verifier_code_hash}",
            "verifier_key_hash_hex": f"0x{verifier_key_hash}",
        }
    ).removeprefix("0x")


def _sample_sccp_tron_destination_binding_hash() -> str:
    from iroha_torii_client.sccp import tron_sccp_destination_binding_hash

    return tron_sccp_destination_binding_hash(
        {
            "network_id_hex": f"0x{SCCP_TEST_TRON_NETWORK_ID}",
            "verifier_address": SCCP_TEST_TRON_VERIFIER_ADDRESS,
            "verifier_code_hash_hex": f"0x{SCCP_TEST_TRON_VERIFIER_CODE_HASH}",
            "verifier_key_hash_hex": f"0x{SCCP_TEST_TRON_VERIFIER_KEY_HASH}",
        }
    ).removeprefix("0x")


def _abi_word(value: int) -> bytes:
    return value.to_bytes(32, "big")


SCCP_TEST_BN254_G2_GENERATOR_WORDS = (
    _abi_word(
        int("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed", 16)
    ),
    _abi_word(
        int("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2", 16)
    ),
    _abi_word(
        int("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa", 16)
    ),
    _abi_word(
        int("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b", 16)
    ),
)


def _sample_sccp_groth16_proof_bytes(
    *,
    message_id: str = SCCP_TEST_MESSAGE_ID,
    source_domain: int = 0,
    commitment_root: str = SCCP_TEST_COMMITMENT_ROOT,
) -> bytes:
    return b"".join(
        (
            _abi_word(1),
            bytes.fromhex(message_id),
            _abi_word(source_domain),
            bytes.fromhex(commitment_root),
            _abi_word(1),
            _abi_word(2),
            *SCCP_TEST_BN254_G2_GENERATOR_WORDS,
            _abi_word(1),
            _abi_word(2),
        )
    )


SCCP_TEST_GROTH16_PROOF_BYTES = _sample_sccp_groth16_proof_bytes()
SCCP_TEST_GROTH16_PROOF_HEX = SCCP_TEST_GROTH16_PROOF_BYTES.hex()


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
                        "upgraded": False,
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
                "init_calls": [],
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
                "transaction_scaffold_b64": "AQID",
                "signed_transaction_b64": "BAUG",
                "signing_message_b64": "BwgJ",
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
            gas_limit=5000,
            fee_sponsor="bad sponsor",
        )


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
    with pytest.raises(RuntimeError, match="valid base64"):
        client.propose_multisig(
            multisig_account_alias="cbdc@banka",
            signer_account_id=CANONICAL_OWNER,
            instructions=[b"\x01"],
            signature_b64="not base64",
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
            gas_limit=1,
        )


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


def test_get_sccp_capabilities_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "local_domain": 0,
                "local_chain": "sora",
                "proof_family": "stark-fri-v1",
                "burn_bundle_path": "/v1/sccp/proofs/burn/{message_id}",
                "message_bundle_path": "/v1/sccp/proofs/message/{message_id}",
                "message_proof_path": "/v1/sccp/artifacts/message/{message_id}",
                "message_job_path": "/v1/sccp/jobs/message/{message_id}",
                "proof_manifest_path": "/v1/sccp/manifests",
                "burn_registry_backend": "bridge/sccp/burn-v1",
                "proof_submit_path": "/v1/bridge/proofs/submit",
                "message_submit_path": "/v1/bridge/messages",
                "message_payload_kinds": [
                    "asset_register",
                    "route_activate",
                    "transfer",
                    "token_add",
                    "token_pause",
                    "token_resume",
                ],
                "codecs": [
                    {
                        "id": 4,
                        "key": "ton_raw",
                        "description": "Canonical TON raw addresses in workchain:account_hex form.",
                    }
                ],
                "counterparties": [
                    {
                        "domain": 4,
                        "chain": "ton",
                        "message_backend": "sccp/stark-fri-v1/ton",
                        "registry_backend": "bridge/sccp/stark-fri-v1/ton",
                        "counterparty_account_codec": 4,
                        "counterparty_account_codec_key": "ton_raw",
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    capabilities = client.get_sccp_capabilities()

    assert capabilities.local_domain == 0
    assert capabilities.local_chain == "sora"
    assert capabilities.message_proof_path == "/v1/sccp/artifacts/message/{message_id}"
    assert capabilities.message_job_path == "/v1/sccp/jobs/message/{message_id}"
    assert capabilities.proof_manifest_path == "/v1/sccp/manifests"
    assert capabilities.burn_registry_backend == "bridge/sccp/burn-v1"
    assert capabilities.message_payload_kinds == [
        "asset_register",
        "route_activate",
        "transfer",
        "token_add",
        "token_pause",
        "token_resume",
    ]
    assert capabilities.codecs[0].key == "ton_raw"
    assert capabilities.counterparties[0].message_backend == "sccp/stark-fri-v1/ton"


def test_get_sccp_proof_manifests_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "local_domain": 0,
                "local_chain": "sora",
                "proof_family": "stark-fri-v1",
                "manifests": [
                    {
                        "version": 1,
                        "local_domain": 0,
                        "local_chain": "sora",
                        "counterparty_domain": 1,
                        "chain": "eth",
                        "proof_family": "stark-fri-v1",
                        "message_backend": "sccp/stark-fri-v1/eth",
                        "registry_backend": "bridge/sccp/stark-fri-v1/eth",
                        "counterparty_account_codec": 2,
                        "counterparty_account_codec_key": "evm_hex",
                        "finality_model": "EthereumBeaconExecution",
                        "verifier_target": "EvmContract",
                        "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:eth",
                        "required_public_inputs": [
                            "message_id",
                            "payload_hash",
                            "target_domain",
                            "commitment_root",
                            "finality_height",
                            "finality_block_hash",
                        ],
                        "message_payload_kinds": ["asset_register", "route_activate", "transfer"],
                        "submission_template": {
                            "version": 1,
                            "encoding": "abi_tuple_v1",
                            "submission_kind": "contract_call",
                            "verifier_entrypoint": (
                                "submitSccpMessageProof(bytes proof_bytes, bytes public_inputs, "
                                "bytes bundle_bytes)"
                            ),
                            "required_arguments": [
                                {
                                    "key": "proof_bytes",
                                    "description": (
                                        "Transparent SCCP proof bytes emitted by the prover "
                                        "backend."
                                    ),
                                },
                                {
                                    "key": "public_inputs",
                                    "description": (
                                        "ABI-encoded SCCP public inputs in manifest order."
                                    ),
                                },
                                {
                                    "key": "bundle_bytes",
                                    "description": (
                                        "ABI-encoded Nexus SCCP message bundle passed to the "
                                        "verifier contract."
                                    ),
                                },
                            ],
                        },
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    manifests = client.get_sccp_proof_manifests()

    assert manifests.local_domain == 0
    assert manifests.proof_family == "stark-fri-v1"
    assert manifests.manifests[0].chain == "eth"
    assert manifests.manifests[0].verifier_target == "EvmContract"
    assert manifests.manifests[0].required_public_inputs[-1] == "finality_block_hash"
    assert manifests.manifests[0].submission_template.encoding == "abi_tuple_v1"
    assert manifests.manifests[0].submission_template.required_arguments[0].key == "proof_bytes"


def test_get_sccp_proof_manifests_rejects_unknown_verifier_target() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "local_domain": 0,
                "local_chain": "sora",
                "proof_family": "stark-fri-v1",
                "manifests": [
                    {
                        "version": 1,
                        "local_domain": 0,
                        "local_chain": "sora",
                        "counterparty_domain": 4,
                        "chain": "ton",
                        "proof_family": "stark-fri-v1",
                        "message_backend": "sccp/stark-fri-v1/ton",
                        "registry_backend": "bridge/sccp/stark-fri-v1/ton",
                        "counterparty_account_codec": 4,
                        "counterparty_account_codec_key": "ton_raw",
                        "finality_model": "TonMasterchain",
                        "verifier_target": "UnknownVerifier",
                        "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                        "required_public_inputs": ["message_id"],
                        "message_payload_kinds": ["transfer"],
                        "submission_template": {
                            "version": 1,
                            "encoding": "ton_cell_v1",
                            "submission_kind": "internal_message",
                            "verifier_entrypoint": "op::submit_sccp_message_proof",
                            "required_arguments": [
                                {
                                    "key": "proof_cell",
                                    "description": (
                                        "Transparent SCCP proof cell emitted by the TON prover "
                                        "backend."
                                    ),
                                }
                            ],
                        },
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="verifier_target must be one of"):
        client.get_sccp_proof_manifests()


def test_get_sccp_message_proof_artifact_parses_typed_snapshot() -> None:
    session = RecordingSession()
    message_id = "11" * 32
    payload_hash = "22" * 32
    commitment_root = "33" * 32
    finality_block_hash = "44" * 32
    session.queue(
        StubResponse(
            payload={
                "version": 1,
                "local_domain": 0,
                "counterparty_domain": 4,
                "proof_family": "stark-fri-v1",
                "message_backend": "sccp/stark-fri-v1/ton",
                "registry_backend": "bridge/sccp/stark-fri-v1/ton",
                "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                "finality_model": "TonMasterchain",
                "verifier_target": "TonContract",
                "submission_template": {
                    "version": 1,
                    "encoding": "ton_cell_v1",
                    "submission_kind": "internal_message",
                    "verifier_entrypoint": "op::submit_sccp_message_proof",
                    "required_arguments": [
                        {
                            "key": "proof_cell",
                            "description": (
                                "Transparent SCCP proof cell emitted by the TON prover backend."
                            ),
                        },
                        {
                            "key": "public_inputs_cell",
                            "description": "Cell-encoded SCCP public inputs in manifest order.",
                        },
                        {
                            "key": "bundle_cell",
                            "description": (
                                "Cell-encoded Nexus SCCP message bundle for the TON bridge "
                                "contract."
                            ),
                        },
                    ],
                },
                "submission_package": {
                    "version": 1,
                    "proof_family": "stark-fri-v1",
                    "verifier_backend": {"version": 1, "key": "ton-contract-v1"},
                    "envelope_encoding": "ton_message_body_v1",
                    "submission_kind": "internal_message",
                    "verifier_entrypoint": "op::submit_sccp_message_proof",
                    "platform_payload": {
                        "platform": "ton_internal_message",
                        "payload": {
                            "proof_cell": "aa55",
                            "public_inputs_cell": "cc77",
                            "bundle_cell": "dd88",
                        },
                    },
                    "arguments": [
                        {"key": "proof_cell", "encoding": "raw_bytes", "bytes": "aa55"},
                        {"key": "public_inputs_cell", "encoding": "raw_bytes", "bytes": "cc77"},
                        {"key": "bundle_cell", "encoding": "raw_bytes", "bytes": "dd88"},
                    ],
                    "envelope_bytes": "ee99",
                },
                "public_inputs": {
                    "version": 1,
                    "message_id": message_id,
                    "payload_hash": payload_hash,
                    "target_domain": 4,
                    "commitment_root": commitment_root,
                    "finality_height": "19",
                    "finality_block_hash": finality_block_hash,
                },
                "proof_bytes": "aa55",
                "submission_package": {
                    "version": 1,
                    "proof_family": "stark-fri-v1",
                    "verifier_backend": {"version": 1, "key": "ton-contract-v1"},
                    "envelope_encoding": "ton_message_body_v1",
                    "submission_kind": "internal_message",
                    "verifier_entrypoint": "op::submit_sccp_message_proof",
                    "platform_payload": {
                        "platform": "ton_internal_message",
                        "payload": {
                            "proof_cell": "aa55",
                            "public_inputs_cell": "cc77",
                            "bundle_cell": "dd88",
                        },
                    },
                    "arguments": [
                        {"key": "proof_cell", "encoding": "raw_bytes", "bytes": "aa55"},
                        {"key": "public_inputs_cell", "encoding": "raw_bytes", "bytes": "cc77"},
                        {"key": "bundle_cell", "encoding": "raw_bytes", "bytes": "dd88"},
                    ],
                    "envelope_bytes": "ee99",
                },
                "bundle": {
                    "version": 1,
                    "commitment_root": commitment_root,
                    "commitment": {
                        "version": 1,
                        "kind": "Transfer",
                        "target_domain": 4,
                        "message_id": message_id,
                        "payload_hash": payload_hash,
                    },
                    "merkle_proof": {
                        "steps": [
                            {
                                "sibling_hash": "55" * 32,
                                "sibling_is_left": False,
                            }
                        ]
                    },
                    "payload": {
                        "Transfer": {
                            "version": 1,
                            "source_domain": 0,
                            "dest_domain": 4,
                            "nonce": "21",
                            "asset_home_domain": 0,
                            "asset_id_codec": 1,
                            "asset_id": "786f7223756e6976657273616c",
                            "amount": "77",
                            "sender_codec": 1,
                            "sender": "6e657875733a736f726173776170",
                            "recipient_codec": 4,
                            "recipient": (
                                "303a3031323334353637383961626364656630313233343536373839616263646566"
                                "3031323334353637383961626364656630313233343536373839616263646566"
                            ),
                            "route_id_codec": 1,
                            "route_id": "6e657875733a746f6e3a786f72",
                        }
                    },
                    "finality_proof": "bb66",
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    binding_hash = _sample_sccp_evm_destination_binding_hash()
    artifact = client.get_sccp_message_proof_artifact(
        f"0x{message_id}",
        network_id_hex="0x" + SCCP_TEST_EVM_NETWORK_ID,
        verifier_address_hex="0x" + SCCP_TEST_EVM_VERIFIER_ADDRESS,
        bridge_address_hex="0x" + SCCP_TEST_EVM_BRIDGE_ADDRESS,
        verifier_code_hash_hex="0x" + SCCP_TEST_EVM_VERIFIER_CODE_HASH,
        verifier_key_hash_hex="0x" + SCCP_TEST_EVM_VERIFIER_KEY_HASH,
        expected_destination_binding_hash_hex="0x" + binding_hash,
        proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
    )

    assert artifact.counterparty_domain == 4
    assert artifact.finality_model == "TonMasterchain"
    assert artifact.public_inputs.message_id == message_id
    assert artifact.submission_package.platform_payload.kind == "ton_internal_message"
    assert artifact.submission_package.platform_payload.value["proof_cell"] == "aa55"
    assert artifact.bundle.payload.kind == "Transfer"
    assert artifact.bundle.payload.value["amount"] == "77"
    assert session.calls[0]["url"] == f"http://node.test/v1/sccp/artifacts/message/{message_id}"
    assert session.calls[0]["params"] == {
        "network_id_hex": SCCP_TEST_EVM_NETWORK_ID,
        "verifier_address_hex": SCCP_TEST_EVM_VERIFIER_ADDRESS,
        "bridge_address_hex": SCCP_TEST_EVM_BRIDGE_ADDRESS,
        "verifier_code_hash_hex": SCCP_TEST_EVM_VERIFIER_CODE_HASH,
        "verifier_key_hash_hex": SCCP_TEST_EVM_VERIFIER_KEY_HASH,
        "expected_destination_binding_hash_hex": binding_hash,
        "proof_bytes_hex": SCCP_TEST_GROTH16_PROOF_HEX,
    }


def test_get_sccp_message_proof_artifact_rejects_proof_message_id_mismatch() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match=r"proof_bytes_hex\.message_id must match message_id"):
        client.get_sccp_message_proof_artifact(
            "11" * 32,
            network_id_hex="0x" + "71" * 32,
            verifier_code_hash_hex="0x" + "72" * 32,
            verifier_key_hash_hex="0x" + "73" * 32,
            expected_destination_binding_hash_hex="0x" + "74" * 32,
            tron_verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            proof_bytes_hex=_sample_sccp_groth16_proof_bytes(message_id="22" * 32),
        )

    assert session.calls == []


def test_sccp_destination_params_include_tron_proof_material() -> None:
    binding_hash = _sample_sccp_tron_destination_binding_hash()
    params = ToriiClient._normalize_sccp_evm_destination_params(
        network_id_hex=bytes.fromhex(SCCP_TEST_TRON_NETWORK_ID),
        verifier_address_hex=None,
        bridge_address_hex=None,
        verifier_code_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_CODE_HASH}",
        verifier_key_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_KEY_HASH}",
        expected_destination_binding_hash_hex=f"0x{binding_hash}",
        tron_verifier_address=SCCP_TEST_TRON_VERIFIER_ADDRESS,
        proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
        context="sccp message proof artifact",
    )

    assert params == {
        "network_id_hex": SCCP_TEST_TRON_NETWORK_ID,
        "verifier_code_hash_hex": SCCP_TEST_TRON_VERIFIER_CODE_HASH,
        "verifier_key_hash_hex": SCCP_TEST_TRON_VERIFIER_KEY_HASH,
        "expected_destination_binding_hash_hex": binding_hash,
        "tron_verifier_address": SCCP_TEST_TRON_VERIFIER_ADDRESS,
        "proof_bytes_hex": SCCP_TEST_GROTH16_PROOF_HEX,
    }


def test_sccp_destination_params_reject_tron_binding_hash_mismatch() -> None:
    with pytest.raises(RuntimeError, match="canonical TRON destination binding"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex=f"0x{SCCP_TEST_TRON_NETWORK_ID}",
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_CODE_HASH}",
            verifier_key_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_KEY_HASH}",
            expected_destination_binding_hash_hex="0x" + "74" * 32,
            tron_verifier_address=SCCP_TEST_TRON_VERIFIER_ADDRESS,
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
            context="sccp message proof artifact",
        )


def test_sccp_destination_params_reject_evm_binding_hash_mismatch() -> None:
    with pytest.raises(RuntimeError, match="canonical EVM destination binding"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex=f"0x{SCCP_TEST_EVM_NETWORK_ID}",
            verifier_address_hex=f"0x{SCCP_TEST_EVM_VERIFIER_ADDRESS}",
            bridge_address_hex=f"0x{SCCP_TEST_EVM_BRIDGE_ADDRESS}",
            verifier_code_hash_hex=f"0x{SCCP_TEST_EVM_VERIFIER_CODE_HASH}",
            verifier_key_hash_hex=f"0x{SCCP_TEST_EVM_VERIFIER_KEY_HASH}",
            expected_destination_binding_hash_hex="0x" + "74" * 32,
            tron_verifier_address=None,
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
            context="sccp message proof artifact",
        )


@pytest.mark.parametrize(
    "tron_verifier_address",
    [
        " TJRabPrwbZy45sbavfcjinPJC18kjpRTv8 ",
        "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9",
        "not-base58",
        "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
    ],
)
def test_sccp_destination_params_reject_invalid_tron_verifier_address(
    tron_verifier_address: str,
) -> None:
    with pytest.raises(RuntimeError, match="tron_verifier_address"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex=None,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex=None,
            verifier_key_hash_hex=None,
            expected_destination_binding_hash_hex=None,
            tron_verifier_address=tron_verifier_address,
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
            context="sccp message proof job",
        )


def test_sccp_normalized_codec_value_normalizes_tron_payload() -> None:
    expected_payload = "4174472e7d35395a6b5add427eecb7f4b62ad2b071"

    from_address = ToriiClient._parse_sccp_normalized_codec_value(
        {"TronBase58Check": {"payload": "TLa2f6VPqDgRE67v1736s7bJ8Ray5wYjU7"}},
        context="codec",
    )
    from_hex = ToriiClient._parse_sccp_normalized_codec_value(
        {"TronBase58Check": {"payload": f"0x{expected_payload}"}},
        context="codec",
    )

    assert from_address.kind == "TronBase58Check"
    assert from_address.value == expected_payload
    assert from_hex.kind == "TronBase58Check"
    assert from_hex.value == expected_payload


@pytest.mark.parametrize(
    "payload",
    [
        " TLa2f6VPqDgRE67v1736s7bJ8Ray5wYjU7 ",
        "not-base58",
        "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
        "0x41" + "00" * 20,
    ],
)
def test_sccp_normalized_codec_value_rejects_invalid_tron_payload(payload: str) -> None:
    with pytest.raises(RuntimeError, match="TRON Base58Check"):
        ToriiClient._parse_sccp_normalized_codec_value(
            {"TronBase58Check": {"payload": payload}},
            context="codec",
        )


@pytest.mark.parametrize(
    ("proof_bytes_hex", "message"),
    [
        (b"", "non-empty hex string"),
        (b"\x00\x00", "must not be all zero"),
        ("0x0000", "must not be all zero"),
        (b"\x01\x02\x03", "384-byte hex string"),
    ],
)
def test_sccp_destination_params_reject_placeholder_proof_bytes(
    proof_bytes_hex: Union[str, bytes],
    message: str,
) -> None:
    with pytest.raises(RuntimeError, match=message):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex=None,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex=None,
            verifier_key_hash_hex=None,
            expected_destination_binding_hash_hex=None,
            tron_verifier_address=None,
            proof_bytes_hex=proof_bytes_hex,
            context="sccp message proof job",
        )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("network_id_hex", " 0x" + "71" * 32),
        ("verifier_code_hash_hex", "0x" + "72" * 16 + " " + "72" * 16),
        ("expected_destination_binding_hash_hex", "0x" + "74" * 32 + "\n"),
        ("proof_bytes_hex", " " + SCCP_TEST_GROTH16_PROOF_HEX),
    ],
)
def test_sccp_destination_params_reject_padded_inline_hex_material(
    field: str,
    value: str,
) -> None:
    kwargs = {
        "network_id_hex": "0x" + "71" * 32,
        "verifier_address_hex": None,
        "bridge_address_hex": None,
        "verifier_code_hash_hex": "0x" + "72" * 32,
        "verifier_key_hash_hex": "0x" + "73" * 32,
        "expected_destination_binding_hash_hex": "0x" + "74" * 32,
        "tron_verifier_address": "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        "proof_bytes_hex": SCCP_TEST_GROTH16_PROOF_BYTES,
        "context": "sccp message proof job",
    }
    kwargs[field] = value

    with pytest.raises(RuntimeError, match=f"{field}.*canonical hex"):
        ToriiClient._normalize_sccp_evm_destination_params(**kwargs)


def test_sccp_destination_params_reject_off_curve_groth16_proof_bytes() -> None:
    off_curve_c = bytearray(SCCP_TEST_GROTH16_PROOF_BYTES)
    off_curve_c[11 * 32 + 31] = 3
    with pytest.raises(RuntimeError, match=r"proof_bytes_hex\.c must be a BN254 G1 point"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex="0x" + "71" * 32,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex=None,
            verifier_key_hash_hex=None,
            expected_destination_binding_hash_hex="0x" + "74" * 32,
            tron_verifier_address=None,
            proof_bytes_hex=bytes(off_curve_c),
            context="sccp message proof job",
        )

    off_curve_b = bytearray(SCCP_TEST_GROTH16_PROOF_BYTES)
    off_curve_b[6 * 32 + 31] ^= 0x01
    with pytest.raises(RuntimeError, match=r"proof_bytes_hex\.b must be a BN254 G2 point"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex="0x" + "71" * 32,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex=None,
            verifier_key_hash_hex=None,
            expected_destination_binding_hash_hex="0x" + "74" * 32,
            tron_verifier_address=None,
            proof_bytes_hex=bytes(off_curve_b),
            context="sccp message proof job",
        )


def test_sccp_destination_params_reject_wrong_source_domain_groth16_proof_bytes() -> None:
    with pytest.raises(RuntimeError, match=r"proof_bytes_hex\.source_domain must be SORA"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex="0x" + "71" * 32,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex="0x" + "72" * 32,
            verifier_key_hash_hex="0x" + "73" * 32,
            expected_destination_binding_hash_hex="0x" + "74" * 32,
            tron_verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            proof_bytes_hex=_sample_sccp_groth16_proof_bytes(source_domain=5),
            context="sccp message proof job",
        )


def test_sccp_destination_params_reject_message_id_mismatch_groth16_proof_bytes() -> None:
    with pytest.raises(RuntimeError, match=r"proof_bytes_hex\.message_id must match message_id"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex="0x" + "71" * 32,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex="0x" + "72" * 32,
            verifier_key_hash_hex="0x" + "73" * 32,
            expected_destination_binding_hash_hex="0x" + "74" * 32,
            tron_verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            proof_bytes_hex=_sample_sccp_groth16_proof_bytes(message_id="22" * 32),
            context="sccp message proof job",
            expected_message_id_hex="11" * 32,
        )


def test_sccp_destination_params_reject_invalid_expected_binding_hash() -> None:
    with pytest.raises(RuntimeError, match="expected_destination_binding_hash_hex"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex=None,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex=None,
            verifier_key_hash_hex=None,
            expected_destination_binding_hash_hex="0x11",
            tron_verifier_address=None,
            proof_bytes_hex=None,
            context="sccp message proof job",
        )


@pytest.mark.parametrize(
    ("field", "byte_length"),
    [
        ("network_id_hex", 32),
        ("verifier_address_hex", 20),
        ("bridge_address_hex", 20),
        ("verifier_code_hash_hex", 32),
        ("verifier_key_hash_hex", 32),
        ("expected_destination_binding_hash_hex", 32),
    ],
)
def test_sccp_destination_params_reject_all_zero_evm_destination_material(
    field: str,
    byte_length: int,
) -> None:
    kwargs = {
        "network_id_hex": None,
        "verifier_address_hex": None,
        "bridge_address_hex": None,
        "verifier_code_hash_hex": None,
        "verifier_key_hash_hex": None,
        "expected_destination_binding_hash_hex": None,
        "tron_verifier_address": None,
        "proof_bytes_hex": SCCP_TEST_GROTH16_PROOF_BYTES,
        "context": "sccp message proof job",
    }
    kwargs[field] = "0x" + "00" * byte_length

    with pytest.raises(RuntimeError, match=f"{field}.*all zero"):
        ToriiClient._normalize_sccp_evm_destination_params(**kwargs)


def test_sccp_destination_params_reject_missing_proof_bytes() -> None:
    with pytest.raises(RuntimeError, match="proof_bytes_hex is required"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex="0x" + "71" * 32,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex=None,
            verifier_key_hash_hex=None,
            expected_destination_binding_hash_hex="0x" + "74" * 32,
            tron_verifier_address=None,
            proof_bytes_hex=None,
            context="sccp message proof job",
        )


def test_sccp_destination_params_reject_proof_without_destination_material() -> None:
    with pytest.raises(RuntimeError, match="deployment destination fields are required"):
        ToriiClient._normalize_sccp_evm_destination_params(
            network_id_hex=None,
            verifier_address_hex=None,
            bridge_address_hex=None,
            verifier_code_hash_hex=None,
            verifier_key_hash_hex=None,
            expected_destination_binding_hash_hex=None,
            tron_verifier_address=None,
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
            context="sccp message proof job",
        )


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        (
            {
                "network_id_hex": "0x" + "71" * 32,
                "expected_destination_binding_hash_hex": "0x" + "74" * 32,
            },
            "complete EVM or TRON",
        ),
        (
            {
                "network_id_hex": "0x" + "71" * 32,
                "verifier_address_hex": "0x" + "22" * 20,
                "verifier_code_hash_hex": "0x" + "72" * 32,
                "verifier_key_hash_hex": "0x" + "73" * 32,
                "expected_destination_binding_hash_hex": "0x" + "74" * 32,
            },
            "complete EVM",
        ),
        (
            {
                "network_id_hex": "0x" + "71" * 32,
                "verifier_address_hex": "0x" + "22" * 20,
                "bridge_address_hex": "0x" + "33" * 20,
                "verifier_code_hash_hex": "0x" + "72" * 32,
                "verifier_key_hash_hex": "0x" + "73" * 32,
                "expected_destination_binding_hash_hex": "0x" + "74" * 32,
                "tron_verifier_address": "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            },
            "cannot be mixed",
        ),
    ],
)
def test_sccp_destination_params_reject_partial_or_mixed_deployment_tuple(
    kwargs: Mapping[str, Any], message: str
) -> None:
    params = {
        "network_id_hex": None,
        "verifier_address_hex": None,
        "bridge_address_hex": None,
        "verifier_code_hash_hex": None,
        "verifier_key_hash_hex": None,
        "expected_destination_binding_hash_hex": None,
        "tron_verifier_address": None,
    }
    params.update(kwargs)
    with pytest.raises(RuntimeError, match=message):
        ToriiClient._normalize_sccp_evm_destination_params(
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
            context="sccp message proof job",
            **params,
        )


def test_bridge_submit_rejects_context_wrong_sccp_groth16_proof_bytes() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())
    binding_hash = _sample_sccp_evm_destination_binding_hash(
        network_id="11" * 32,
        verifier_address="22" * 20,
        bridge_address="33" * 20,
        verifier_code_hash="44" * 32,
        verifier_key_hash="55" * 32,
    )
    destination_material = {
        "network_id_hex": "0x" + "11" * 32,
        "verifier_address_hex": "0x" + "22" * 20,
        "bridge_address_hex": "0x" + "33" * 20,
        "verifier_code_hash_hex": "0x" + "44" * 32,
        "verifier_key_hash_hex": "0x" + "55" * 32,
        "expected_destination_binding_hash_hex": "0x" + binding_hash,
    }

    with pytest.raises(RuntimeError, match="proof_bytes_hex.message_id"):
        client.submit_bridge_proof(
            authority="alice@sora",
            message_bundle=SCCP_TEST_MESSAGE_BUNDLE,
            **destination_material,
            proof_bytes_hex=_sample_sccp_groth16_proof_bytes(message_id="44" * 32),
        )

    with pytest.raises(RuntimeError, match="proof_bytes_hex.source_domain must be SORA"):
        client.submit_bridge_message(
            authority="alice@sora",
            message_bundle=SCCP_TEST_MESSAGE_BUNDLE,
            **destination_material,
            proof_bytes_hex=_sample_sccp_groth16_proof_bytes(source_domain=5),
        )

    with pytest.raises(RuntimeError, match="proof_bytes_hex.commitment_root"):
        client.submit_bridge_message(
            authority="alice@sora",
            message_bundle=SCCP_TEST_MESSAGE_BUNDLE,
            **destination_material,
            proof_bytes_hex=_sample_sccp_groth16_proof_bytes(commitment_root="55" * 32),
        )


def test_submit_bridge_proof_posts_tron_proof_material() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            200,
            {
                "ok": True,
                "submitted": False,
                "proof_kind": "message",
                "counterparty_chain": "tron",
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)
    binding_hash = _sample_sccp_tron_destination_binding_hash()

    response = client.submit_bridge_proof(
        authority=" alice@sora ",
        message_bundle=SCCP_TEST_MESSAGE_BUNDLE,
        network_id_hex=bytes.fromhex(SCCP_TEST_TRON_NETWORK_ID),
        verifier_code_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_CODE_HASH}",
        verifier_key_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_KEY_HASH}",
        expected_destination_binding_hash_hex=f"0x{binding_hash}",
        tron_verifier_address=SCCP_TEST_TRON_VERIFIER_ADDRESS,
        proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
        creation_time_ms="1779660000000",
    )

    assert response == {
        "ok": True,
        "submitted": False,
        "proof_kind": "message",
        "counterparty_chain": "tron",
    }
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"] == "http://node.test/v1/bridge/proofs/submit"
    assert session.calls[0]["headers"]["Content-Type"] == "application/json"
    assert json.loads(session.calls[0]["data"].decode("utf-8")) == {
        "authority": "alice@sora",
        "message_bundle": SCCP_TEST_MESSAGE_BUNDLE,
        "network_id_hex": SCCP_TEST_TRON_NETWORK_ID,
        "verifier_code_hash_hex": SCCP_TEST_TRON_VERIFIER_CODE_HASH,
        "verifier_key_hash_hex": SCCP_TEST_TRON_VERIFIER_KEY_HASH,
        "expected_destination_binding_hash_hex": binding_hash,
        "tron_verifier_address": SCCP_TEST_TRON_VERIFIER_ADDRESS,
        "proof_bytes_hex": SCCP_TEST_GROTH16_PROOF_HEX,
        "creation_time_ms": "1779660000000",
    }


def test_submit_bridge_proof_rejects_ambiguous_bundle_selection_before_request() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="exactly one of burn_bundle or message_bundle"):
        client.submit_bridge_proof(authority="alice@sora")

    with pytest.raises(RuntimeError, match="exactly one of burn_bundle or message_bundle"):
        client.submit_bridge_proof(
            authority="alice@sora",
            burn_bundle={"version": 1},
            message_bundle={"version": 1},
        )

    assert session.calls == []


def test_submit_bridge_proof_rejects_destination_tuple_on_burn_bundle() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)
    binding_hash = _sample_sccp_tron_destination_binding_hash()

    with pytest.raises(RuntimeError, match="message_bundle submissions"):
        client.submit_bridge_proof(
            authority="alice@sora",
            burn_bundle={"version": 1},
            network_id_hex=bytes.fromhex(SCCP_TEST_TRON_NETWORK_ID),
            verifier_code_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_CODE_HASH}",
            verifier_key_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_KEY_HASH}",
            expected_destination_binding_hash_hex=f"0x{binding_hash}",
            tron_verifier_address=SCCP_TEST_TRON_VERIFIER_ADDRESS,
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
        )

    assert session.calls == []


def test_submit_bridge_proof_rejects_proof_bytes_without_message_commitment_context() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)
    binding_hash = _sample_sccp_tron_destination_binding_hash()

    with pytest.raises(
        RuntimeError,
        match=r"message_bundle\.commitment\.message_id is required",
    ):
        client.submit_bridge_proof(
            authority="alice@sora",
            message_bundle={"version": 1},
            network_id_hex=bytes.fromhex(SCCP_TEST_TRON_NETWORK_ID),
            verifier_code_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_CODE_HASH}",
            verifier_key_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_KEY_HASH}",
            expected_destination_binding_hash_hex=f"0x{binding_hash}",
            tron_verifier_address=SCCP_TEST_TRON_VERIFIER_ADDRESS,
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
        )

    assert session.calls == []


def test_submit_bridge_message_posts_tron_proof_material() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            200,
            {
                "ok": True,
                "submitted": False,
                "message_kind": "transfer",
                "counterparty_chain": "tron",
            },
        )
    )
    client = ToriiClient("http://node.test", session=session)
    binding_hash = _sample_sccp_tron_destination_binding_hash()

    response = client.submit_bridge_message(
        authority=" alice@sora ",
        message_bundle=SCCP_TEST_MESSAGE_BUNDLE,
        network_id_hex=bytes.fromhex(SCCP_TEST_TRON_NETWORK_ID),
        verifier_code_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_CODE_HASH}",
        verifier_key_hash_hex=f"0x{SCCP_TEST_TRON_VERIFIER_KEY_HASH}",
        expected_destination_binding_hash_hex=f"0x{binding_hash}",
        tron_verifier_address=SCCP_TEST_TRON_VERIFIER_ADDRESS,
        proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
        receipt_lane=7,
        settlement={"route": "xor"},
        creation_time_ms="1779660000000",
    )

    assert response == {
        "ok": True,
        "submitted": False,
        "message_kind": "transfer",
        "counterparty_chain": "tron",
    }
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"] == "http://node.test/v1/bridge/messages"
    assert session.calls[0]["headers"]["Content-Type"] == "application/json"
    assert json.loads(session.calls[0]["data"].decode("utf-8")) == {
        "authority": "alice@sora",
        "message_bundle": SCCP_TEST_MESSAGE_BUNDLE,
        "network_id_hex": SCCP_TEST_TRON_NETWORK_ID,
        "verifier_code_hash_hex": SCCP_TEST_TRON_VERIFIER_CODE_HASH,
        "verifier_key_hash_hex": SCCP_TEST_TRON_VERIFIER_KEY_HASH,
        "expected_destination_binding_hash_hex": binding_hash,
        "tron_verifier_address": SCCP_TEST_TRON_VERIFIER_ADDRESS,
        "proof_bytes_hex": SCCP_TEST_GROTH16_PROOF_HEX,
        "receipt_lane": 7,
        "settlement": {"route": "xor"},
        "creation_time_ms": "1779660000000",
    }


@pytest.mark.parametrize(
    "tron_verifier_address",
    [
        " TJRabPrwbZy45sbavfcjinPJC18kjpRTv8 ",
        "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9",
        "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
    ],
)
def test_submit_bridge_message_rejects_invalid_tron_verifier_address_before_request(
    tron_verifier_address: str,
) -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="tron_verifier_address"):
        client.submit_bridge_message(
            authority="alice@sora",
            message_bundle=SCCP_TEST_MESSAGE_BUNDLE,
            tron_verifier_address=tron_verifier_address,
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
        )

    assert session.calls == []


def test_submit_bridge_message_rejects_all_zero_evm_destination_material_before_request() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="verifier_address_hex.*all zero"):
        client.submit_bridge_message(
            authority="alice@sora",
            message_bundle=SCCP_TEST_MESSAGE_BUNDLE,
            verifier_address_hex="0x" + "00" * 20,
            proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
        )

    assert session.calls == []


def test_get_sccp_message_proof_artifact_preserves_solana_submission_context() -> None:
    session = RecordingSession()
    message_id = "11" * 32
    payload_hash = "22" * 32
    commitment_root = "33" * 32
    binding_hash = "56" * 32
    statement_hash = "99" * 32
    proof_context_hash = "ab" * 32
    session.queue(
        StubResponse(
            payload={
                "version": 1,
                "local_domain": 0,
                "counterparty_domain": 3,
                "proof_family": "stark-fri-v1",
                "message_backend": "sccp/stark-fri-v1/sol",
                "registry_backend": "bridge/sccp/stark-fri-v1/sol",
                "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:sol",
                "finality_model": "SolanaFinalizedSlot",
                "verifier_target": "SolanaProgram",
                "public_inputs": {
                    "version": 1,
                    "message_id": message_id,
                    "payload_hash": payload_hash,
                    "target_domain": 3,
                    "commitment_root": commitment_root,
                    "finality_height": "321",
                    "finality_block_hash": "44" * 32,
                },
                "proof_bytes": "aa55",
                "submission_package": {
                    "version": 1,
                    "proof_family": "stark-fri-v1",
                    "verifier_backend": {"version": 1, "key": "solana-program-v1"},
                    "envelope_encoding": "borsh_instruction_v1",
                    "submission_kind": "program_instruction",
                    "verifier_entrypoint": "submit_sccp_message_proof",
                    "platform_payload": {
                        "platform": "solana_program_instruction",
                        "payload": {
                            "proof_bytes": "aa55",
                            "public_inputs_bytes": "cc77",
                            "bundle_bytes": "dd88",
                            "destination_binding": {
                                "version": 1,
                                "key": "sccp:sol:governed-recursive-zk:v1",
                                "binding_hash": binding_hash,
                            },
                            "destination_binding_hash": binding_hash,
                            "statement_hash": statement_hash,
                            "proof_context_hash": proof_context_hash,
                        },
                    },
                    "arguments": [
                        {"key": "proof_bytes", "encoding": "raw_bytes", "bytes": "aa55"},
                        {"key": "public_inputs", "encoding": "raw_bytes", "bytes": "cc77"},
                        {"key": "bundle_bytes", "encoding": "raw_bytes", "bytes": "dd88"},
                        {"key": "statement_hash", "encoding": "raw_bytes", "bytes": statement_hash},
                        {
                            "key": "destination_binding_hash",
                            "encoding": "raw_bytes",
                            "bytes": binding_hash,
                        },
                        {
                            "key": "proof_context_hash",
                            "encoding": "raw_bytes",
                            "bytes": proof_context_hash,
                        },
                    ],
                    "envelope_bytes": "ee99",
                },
                "bundle": {
                    "version": 1,
                    "commitment_root": commitment_root,
                    "commitment": {
                        "version": 1,
                        "kind": "Transfer",
                        "target_domain": 3,
                        "message_id": message_id,
                        "payload_hash": payload_hash,
                    },
                    "merkle_proof": {"steps": []},
                    "payload": {"Transfer": {"version": 1}},
                    "finality_proof": "bb66",
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    artifact = client.get_sccp_message_proof_artifact(f"0x{message_id}")
    payload = artifact.submission_package.platform_payload

    assert payload.kind == "solana_program_instruction"
    assert payload.value["proof_bytes"] == "aa55"
    assert payload.value["public_inputs_bytes"] == "cc77"
    assert payload.value["bundle_bytes"] == "dd88"
    assert payload.value["destination_binding"] == {
        "version": 1,
        "key": "sccp:sol:governed-recursive-zk:v1",
        "binding_hash": binding_hash,
    }
    assert payload.value["destination_binding_hash"] == binding_hash
    assert payload.value["statement_hash"] == statement_hash
    assert payload.value["proof_context_hash"] == proof_context_hash


def test_get_sccp_message_proof_artifact_preserves_ton_message_body_payload() -> None:
    session = RecordingSession()
    message_id = "11" * 32
    payload_hash = "22" * 32
    commitment_root = "33" * 32
    binding_hash = "58" * 32
    statement_hash = "99" * 32
    session.queue(
        StubResponse(
            payload={
                "version": 1,
                "local_domain": 0,
                "counterparty_domain": 4,
                "proof_family": "stark-fri-v1",
                "message_backend": "sccp/stark-fri-v1/ton",
                "registry_backend": "bridge/sccp/stark-fri-v1/ton",
                "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                "finality_model": "TonMasterchain",
                "verifier_target": "TonContract",
                "public_inputs": {
                    "version": 1,
                    "message_id": message_id,
                    "payload_hash": payload_hash,
                    "target_domain": 4,
                    "commitment_root": commitment_root,
                    "finality_height": "19",
                    "finality_block_hash": "44" * 32,
                },
                "proof_bytes": "aa55",
                "submission_package": {
                    "version": 1,
                    "proof_family": "stark-fri-v1",
                    "verifier_backend": {"version": 1, "key": "ton-contract-v1"},
                    "envelope_encoding": "ton_message_body_boc_v1",
                    "submission_kind": "internal_message",
                    "verifier_entrypoint": "op::submit_sccp_message_proof",
                    "platform_payload": {
                        "platform": "ton_internal_message",
                        "payload": {
                            "message_body_boc": "b5ee9c72",
                            "query_id": "7",
                            "destination_binding": {
                                "version": 1,
                                "key": "sccp:ton:governed-recursive-zk:v1",
                                "binding_hash": binding_hash,
                            },
                            "destination_binding_hash": binding_hash,
                            "proof_bytes": "aa55",
                            "public_inputs_bytes": "cc77",
                            "bundle_bytes": "dd88",
                            "statement_hash": statement_hash,
                        },
                    },
                    "arguments": [
                        {"key": "message_body_boc", "encoding": "ton_boc", "bytes": "b5ee9c72"}
                    ],
                    "envelope_bytes": "b5ee9c72",
                },
                "bundle": {
                    "version": 1,
                    "commitment_root": commitment_root,
                    "commitment": {
                        "version": 1,
                        "kind": "Transfer",
                        "target_domain": 4,
                        "message_id": message_id,
                        "payload_hash": payload_hash,
                    },
                    "merkle_proof": {"steps": []},
                    "payload": {"Transfer": {"version": 1}},
                    "finality_proof": "bb66",
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    artifact = client.get_sccp_message_proof_artifact(message_id)
    payload = artifact.submission_package.platform_payload

    assert payload.kind == "ton_internal_message"
    assert payload.value["message_body_boc"] == "b5ee9c72"
    assert payload.value["query_id"] == 7
    assert payload.value["destination_binding"] == {
        "version": 1,
        "key": "sccp:ton:governed-recursive-zk:v1",
        "binding_hash": binding_hash,
    }
    assert payload.value["destination_binding_hash"] == binding_hash
    assert payload.value["proof_bytes"] == "aa55"
    assert payload.value["public_inputs_bytes"] == "cc77"
    assert payload.value["bundle_bytes"] == "dd88"
    assert payload.value["statement_hash"] == statement_hash


def test_get_sccp_message_proof_artifact_rejects_mismatched_public_inputs() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "version": 1,
                "local_domain": 0,
                "counterparty_domain": 1,
                "proof_family": "stark-fri-v1",
                "message_backend": "sccp/stark-fri-v1/eth",
                "registry_backend": "bridge/sccp/stark-fri-v1/eth",
                "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:eth",
                "finality_model": "EthereumBeaconExecution",
                "verifier_target": "EvmContract",
                "public_inputs": {
                    "version": 1,
                    "message_id": "11" * 32,
                    "payload_hash": "22" * 32,
                    "target_domain": 1,
                    "commitment_root": "33" * 32,
                    "finality_height": "7",
                    "finality_block_hash": "44" * 32,
                },
                "proof_bytes": "aa55",
                "submission_package": {
                    "version": 1,
                    "proof_family": "stark-fri-v1",
                    "verifier_backend": {"version": 1, "key": "evm-secp256k1-keccak-v1"},
                    "envelope_encoding": "abi_tuple_v1",
                    "submission_kind": "contract_call",
                    "verifier_entrypoint": (
                        "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, "
                        "bytes32 statement_hash)"
                    ),
                    "platform_payload": {
                        "platform": "evm_contract_call",
                        "payload": {
                            "proof_bytes": "aa55",
                            "public_inputs": {
                                "message_id": "11" * 32,
                                "payload_hash": "22" * 32,
                                "target_domain_word": "00" * 31 + "01",
                                "commitment_root": "33" * 32,
                                "finality_height_word": "00" * 31 + "07",
                                "finality_block_hash": "44" * 32,
                            },
                            "public_inputs_hash": "88" * 32,
                            "statement_hash": "55" * 32,
                            "attestation": {
                                "version": 1,
                                "message_id": "11" * 32,
                                "source_domain": 0,
                                "commitment_root": "33" * 32,
                                "native_proof_hash": "99" * 32,
                                "signatures": [
                                    {
                                        "signer_address": "12" * 20,
                                        "signature_bytes": "34" * 65,
                                    }
                                ],
                            },
                        },
                    },
                    "arguments": [
                        {"key": "proof_bytes", "encoding": "raw_bytes", "bytes": "aa55"},
                        {"key": "public_inputs", "encoding": "abi_bytes32x6", "bytes": "66" * (32 * 6)},
                        {"key": "statement_hash", "encoding": "abi_bytes32", "bytes": "55" * 32},
                    ],
                    "envelope_bytes": "77",
                },
                "bundle": {
                    "version": 1,
                    "commitment_root": "33" * 32,
                    "commitment": {
                        "version": 1,
                        "kind": "Transfer",
                        "target_domain": 1,
                        "message_id": "99" * 32,
                        "payload_hash": "22" * 32,
                    },
                    "merkle_proof": {"steps": []},
                    "payload": {"Transfer": {"version": 1}},
                    "finality_proof": "bb66",
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="message_id"):
        client.get_sccp_message_proof_artifact("11" * 32)


def test_get_sccp_message_proof_artifact_against_mock_server() -> None:
    server = ToriiMockServer().start()
    message_id = "11" * 32
    try:
        response = requests.post(
            f"{server.base_url.rstrip('/')}/__mock__/sccp/config",
            json={
                "message_artifacts": {
                    message_id: {
                        "version": 1,
                        "local_domain": 0,
                        "counterparty_domain": 4,
                        "proof_family": "stark-fri-v1",
                        "message_backend": "sccp/stark-fri-v1/ton",
                        "registry_backend": "bridge/sccp/stark-fri-v1/ton",
                        "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                        "finality_model": "TonMasterchain",
                        "verifier_target": "TonContract",
                        "public_inputs": {
                            "version": 1,
                            "message_id": message_id,
                            "payload_hash": "22" * 32,
                            "target_domain": 4,
                            "commitment_root": "33" * 32,
                            "finality_height": "19",
                            "finality_block_hash": "44" * 32,
                        },
                        "proof_bytes": "aa55",
                        "submission_package": {
                            "version": 1,
                            "proof_family": "stark-fri-v1",
                            "verifier_backend": {"version": 1, "key": "ton-contract-v1"},
                            "envelope_encoding": "ton_message_body_v1",
                            "submission_kind": "internal_message",
                            "verifier_entrypoint": "op::submit_sccp_message_proof",
                            "platform_payload": {
                                "platform": "ton_internal_message",
                                "payload": {
                                    "proof_cell": "aa55",
                                    "public_inputs_cell": "cc77",
                                    "bundle_cell": "dd88",
                                },
                            },
                            "arguments": [
                                {"key": "proof_cell", "encoding": "raw_bytes", "bytes": "aa55"},
                                {"key": "public_inputs_cell", "encoding": "raw_bytes", "bytes": "cc77"},
                                {"key": "bundle_cell", "encoding": "raw_bytes", "bytes": "dd88"},
                            ],
                            "envelope_bytes": "ee99",
                        },
                        "bundle": {
                            "version": 1,
                            "commitment_root": "33" * 32,
                            "commitment": {
                                "version": 1,
                                "kind": "Transfer",
                                "target_domain": 4,
                                "message_id": message_id,
                                "payload_hash": "22" * 32,
                            },
                            "merkle_proof": {"steps": []},
                            "payload": {"Transfer": {"version": 1, "amount": "77"}},
                            "finality_proof": "bb66",
                        },
                    }
                }
            },
            timeout=5.0,
        )
        response.raise_for_status()

        client = ToriiClient(server.base_url)
        artifact = client.get_sccp_message_proof_artifact(message_id)

        assert artifact.bundle.payload.kind == "Transfer"
        assert artifact.bundle.payload.value["amount"] == "77"
        assert artifact.message_backend == "sccp/stark-fri-v1/ton"
        assert artifact.submission_package.platform_payload.kind == "ton_internal_message"
    finally:
        server.stop()


def test_get_sccp_message_proof_job_parses_typed_snapshot() -> None:
    session = RecordingSession()
    message_id = "11" * 32
    payload_hash = "22" * 32
    commitment_root = "33" * 32
    finality_block_hash = "44" * 32
    session.queue(
        StubResponse(
            payload={
                "version": 1,
                "chain_family": "Ton",
                "chain": "ton",
                "local_domain": 0,
                "counterparty_domain": 4,
                "proof_family": "stark-fri-v1",
                "message_backend": "sccp/stark-fri-v1/ton",
                "registry_backend": "bridge/sccp/stark-fri-v1/ton",
                "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                "finality_model": "TonMasterchain",
                "verifier_target": "TonContract",
                "submission_template": {
                    "version": 1,
                    "encoding": "ton_cell_v1",
                    "submission_kind": "internal_message",
                    "verifier_entrypoint": "op::submit_sccp_message_proof",
                    "required_arguments": [
                        {
                            "key": "proof_cell",
                            "description": (
                                "Transparent SCCP proof cell emitted by the TON prover backend."
                            ),
                        },
                        {
                            "key": "public_inputs_cell",
                            "description": "Cell-encoded SCCP public inputs in manifest order.",
                        },
                        {
                            "key": "bundle_cell",
                            "description": (
                                "Cell-encoded Nexus SCCP message bundle for the TON bridge "
                                "contract."
                            ),
                        },
                    ],
                },
                "submission_package": {
                    "version": 1,
                    "proof_family": "stark-fri-v1",
                    "verifier_backend": {"version": 1, "key": "ton-contract-v1"},
                    "envelope_encoding": "ton_message_body_v1",
                    "submission_kind": "internal_message",
                    "verifier_entrypoint": "op::submit_sccp_message_proof",
                    "platform_payload": {
                        "platform": "ton_internal_message",
                        "payload": {
                            "proof_cell": "aa55",
                            "public_inputs_cell": "cc77",
                            "bundle_cell": "dd88",
                        },
                    },
                    "arguments": [
                        {"key": "proof_cell", "encoding": "raw_bytes", "bytes": "aa55"},
                        {"key": "public_inputs_cell", "encoding": "raw_bytes", "bytes": "cc77"},
                        {"key": "bundle_cell", "encoding": "raw_bytes", "bytes": "dd88"},
                    ],
                    "envelope_bytes": "ee99",
                },
                "public_inputs": {
                    "version": 1,
                    "message_id": message_id,
                    "payload_hash": payload_hash,
                    "target_domain": 4,
                    "commitment_root": commitment_root,
                    "finality_height": "19",
                    "finality_block_hash": finality_block_hash,
                },
                "payload_kind": "transfer",
                "payload_projection": {
                    "Transfer": {
                        "version": 1,
                        "source_domain": 0,
                        "dest_domain": 4,
                        "nonce": "21",
                        "asset_home_domain": 0,
                        "asset_id": {"TextUtf8": {"value": "xor#universal"}},
                        "amount": "77",
                        "sender": {"TextUtf8": {"value": "nexus:soraswap"}},
                        "recipient": {
                            "TonRaw": {
                                "workchain": 0,
                                "account": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                            }
                        },
                        "route_id": {"TextUtf8": {"value": "nexus:ton:xor"}},
                    }
                },
                "bundle": {
                    "version": 1,
                    "commitment_root": commitment_root,
                    "commitment": {
                        "version": 1,
                        "kind": "Transfer",
                        "target_domain": 4,
                        "message_id": message_id,
                        "payload_hash": payload_hash,
                    },
                    "merkle_proof": {"steps": []},
                    "payload": {"Transfer": {"version": 1, "amount": "77"}},
                    "finality_proof": "bb66",
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    binding_hash = _sample_sccp_evm_destination_binding_hash()
    job = client.get_sccp_message_proof_job(
        f"0x{message_id}",
        network_id_hex="0x" + SCCP_TEST_EVM_NETWORK_ID,
        verifier_address_hex="0x" + SCCP_TEST_EVM_VERIFIER_ADDRESS,
        bridge_address_hex="0x" + SCCP_TEST_EVM_BRIDGE_ADDRESS,
        verifier_code_hash_hex="0x" + SCCP_TEST_EVM_VERIFIER_CODE_HASH,
        verifier_key_hash_hex="0x" + SCCP_TEST_EVM_VERIFIER_KEY_HASH,
        expected_destination_binding_hash_hex="0x" + binding_hash,
        proof_bytes_hex=SCCP_TEST_GROTH16_PROOF_BYTES,
    )

    assert job.chain_family == "Ton"
    assert job.chain == "ton"
    assert job.payload_kind == "transfer"
    assert job.payload_projection.kind == "Transfer"
    assert job.payload_projection.value["amount"] == 77
    assert job.payload_projection.value["recipient"].kind == "TonRaw"
    assert job.payload_projection.value["recipient"].value["workchain"] == 0
    assert job.submission_template.encoding == "ton_cell_v1"
    assert job.submission_template.required_arguments[0].key == "proof_cell"
    assert job.submission_package.platform_payload.kind == "ton_internal_message"
    assert session.calls[0]["url"] == f"http://node.test/v1/sccp/jobs/message/{message_id}"
    assert session.calls[0]["params"] == {
        "network_id_hex": SCCP_TEST_EVM_NETWORK_ID,
        "verifier_address_hex": SCCP_TEST_EVM_VERIFIER_ADDRESS,
        "bridge_address_hex": SCCP_TEST_EVM_BRIDGE_ADDRESS,
        "verifier_code_hash_hex": SCCP_TEST_EVM_VERIFIER_CODE_HASH,
        "verifier_key_hash_hex": SCCP_TEST_EVM_VERIFIER_KEY_HASH,
        "expected_destination_binding_hash_hex": binding_hash,
        "proof_bytes_hex": SCCP_TEST_GROTH16_PROOF_HEX,
    }


def test_get_sccp_message_proof_job_against_mock_server() -> None:
    server = ToriiMockServer().start()
    message_id = "11" * 32
    try:
        response = requests.post(
            f"{server.base_url.rstrip('/')}/__mock__/sccp/config",
            json={
                "message_jobs": {
                    message_id: {
                        "version": 1,
                        "chain_family": "Ton",
                        "chain": "ton",
                        "local_domain": 0,
                        "counterparty_domain": 4,
                        "proof_family": "stark-fri-v1",
                        "message_backend": "sccp/stark-fri-v1/ton",
                        "registry_backend": "bridge/sccp/stark-fri-v1/ton",
                        "manifest_seed": "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                        "finality_model": "TonMasterchain",
                        "verifier_target": "TonContract",
                        "submission_template": {
                            "version": 1,
                            "encoding": "ton_cell_v1",
                            "submission_kind": "internal_message",
                            "verifier_entrypoint": "op::submit_sccp_message_proof",
                            "required_arguments": [
                                {
                                    "key": "proof_cell",
                                    "description": (
                                        "Transparent SCCP proof cell emitted by the TON prover "
                                        "backend."
                                    ),
                                },
                                {
                                    "key": "public_inputs_cell",
                                    "description": (
                                        "Cell-encoded SCCP public inputs in manifest order."
                                    ),
                                },
                                {
                                    "key": "bundle_cell",
                                    "description": (
                                        "Cell-encoded Nexus SCCP message bundle for the TON "
                                        "bridge contract."
                                    ),
                                },
                            ],
                        },
                        "submission_package": {
                            "version": 1,
                            "proof_family": "stark-fri-v1",
                            "verifier_backend": {"version": 1, "key": "ton-contract-v1"},
                            "envelope_encoding": "ton_message_body_v1",
                            "submission_kind": "internal_message",
                            "verifier_entrypoint": "op::submit_sccp_message_proof",
                            "platform_payload": {
                                "platform": "ton_internal_message",
                                "payload": {
                                    "proof_cell": "aa55",
                                    "public_inputs_cell": "cc77",
                                    "bundle_cell": "dd88",
                                },
                            },
                            "arguments": [
                                {"key": "proof_cell", "encoding": "raw_bytes", "bytes": "aa55"},
                                {"key": "public_inputs_cell", "encoding": "raw_bytes", "bytes": "cc77"},
                                {"key": "bundle_cell", "encoding": "raw_bytes", "bytes": "dd88"},
                            ],
                            "envelope_bytes": "ee99",
                        },
                        "public_inputs": {
                            "version": 1,
                            "message_id": message_id,
                            "payload_hash": "22" * 32,
                            "target_domain": 4,
                            "commitment_root": "33" * 32,
                            "finality_height": "19",
                            "finality_block_hash": "44" * 32,
                        },
                        "payload_kind": "transfer",
                        "payload_projection": {
                            "Transfer": {
                                "version": 1,
                                "source_domain": 0,
                                "dest_domain": 4,
                                "nonce": "21",
                                "asset_home_domain": 0,
                                "asset_id": {"TextUtf8": {"value": "xor#universal"}},
                                "amount": "77",
                                "sender": {"TextUtf8": {"value": "nexus:soraswap"}},
                                "recipient": {
                                    "TonRaw": {
                                        "workchain": 0,
                                        "account": (
                                            "0123456789abcdef0123456789abcdef0123456789abcdef"
                                            "0123456789abcdef"
                                        ),
                                    }
                                },
                                "route_id": {"TextUtf8": {"value": "nexus:ton:xor"}},
                            }
                        },
                        "bundle": {
                            "version": 1,
                            "commitment_root": "33" * 32,
                            "commitment": {
                                "version": 1,
                                "kind": "Transfer",
                                "target_domain": 4,
                                "message_id": message_id,
                                "payload_hash": "22" * 32,
                            },
                            "merkle_proof": {"steps": []},
                            "payload": {"Transfer": {"version": 1, "amount": "77"}},
                            "finality_proof": "bb66",
                        },
                    }
                }
            },
            timeout=5.0,
        )
        response.raise_for_status()

        client = ToriiClient(server.base_url)
        job = client.get_sccp_message_proof_job(message_id)

        assert job.chain == "ton"
        assert job.payload_projection.kind == "Transfer"
        assert job.payload_projection.value["amount"] == 77
        assert job.submission_package.platform_payload.kind == "ton_internal_message"
        assert job.submission_template.verifier_entrypoint == "op::submit_sccp_message_proof"
    finally:
        server.stop()


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
                            "upgraded": False,
                            "dataspace": "universal",
                            "deploy_nonce": 1,
                            "tx_hash_hex": "11" * 32,
                            "code_hash_hex": "22" * 32,
                            "abi_hash_hex": "33" * 32,
                            "status": "submitted",
                        }
                    ],
                    "init_calls": [],
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
                    "transaction_scaffold_b64": "AQID",
                    "signed_transaction_b64": "BAUG",
                    "signing_message_b64": "BwgJ",
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

        assert payload["leader_index"] == 2
        assert payload["gossip_fallback_total"] == 1
        assert payload["rbc_store"]["persist_drops_total"] == 2
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
                    "completed_stages": ["plan", "deploy", "init_calls", "assertions"],
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
            "init_calls": [
                {
                    "id": "seed",
                    "contract_alias": "greeter::universal",
                    "entrypoint": "init",
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
        assert dry_run_payload["init_calls"][0]["status"] == "pending"

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
            "init_calls",
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


def test_get_sumeragi_collectors_parses_entries() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "consensus_mode": "Permissioned",
                "mode": "Permissioned",
                "topology_len": 7,
                "min_votes_for_commit": 5,
                "proxy_tail_index": 2,
                "height": 11,
                "view": 3,
                "collectors_k": 4,
                "redundant_send_r": 1,
                "epoch_seed": "abcd",
                "collectors": [{"index": 0, "peer_id": "peer#0"}, {"index": 1, "peer_id": "peer#1"}],
                "prf": {"height": 11, "view": 3, "epoch_seed": None},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    collectors = client.get_sumeragi_collectors()

    assert collectors.collectors_k == 4
    assert collectors.collectors[1].peer_id == "peer#1"
    assert session.calls[0]["url"].endswith("/v1/sumeragi/collectors")


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
    assert session.calls[0]["url"].endswith("/v1/sumeragi/bls_keys")


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


def test_get_sumeragi_rbc_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "sessions_active": 2,
                "sessions_pruned_total": 7,
                "ready_broadcasts_total": 11,
                "deliver_broadcasts_total": 13,
                "payload_bytes_delivered_total": 1024,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_sumeragi_rbc()

    assert snapshot.sessions_active == 2
    assert snapshot.payload_bytes_delivered_total == 1024
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/sumeragi/rbc")


def test_get_sumeragi_rbc_sessions_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "sessions_active": 1,
                "items": [
                    {
                        "block_hash": "AA55",
                        "height": 42,
                        "view": 3,
                        "total_chunks": 8,
                        "received_chunks": 4,
                        "ready_count": 2,
                        "delivered": True,
                        "invalid": False,
                        "payload_hash": "FF",
                        "recovered": False,
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    sessions = client.get_sumeragi_rbc_sessions()

    assert sessions.sessions_active == 1
    assert len(sessions.items) == 1
    assert sessions.items[0].block_hash == "AA55"
    assert sessions.items[0].delivered is True
    assert session.calls[0]["url"].endswith("/v1/sumeragi/rbc/sessions")


def test_get_sumeragi_rbc_delivered_flow() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=404))
    session.queue(
        StubResponse(
            payload={
                "height": 5,
                "view": 1,
                "delivered": True,
                "present": True,
                "block_hash": "DEADBEEF",
                "ready_count": 7,
                "received_chunks": 8,
                "total_chunks": 10,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    assert client.get_sumeragi_rbc_delivered(5, 1) is None
    status = client.get_sumeragi_rbc_delivered(height="5", view="1")

    assert status is not None
    assert status.block_hash == "DEADBEEF"
    assert status.ready_count == 7
    assert session.calls[1]["url"].endswith("/v1/sumeragi/rbc/delivered/5/1")


def test_sample_rbc_chunks_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "block_hash": "AA",
                "height": 9,
                "view": 0,
                "total_chunks": 16,
                "chunk_root": "BB",
                "payload_hash": None,
                "samples": [
                    {
                        "index": 0,
                        "chunk_hex": "CC",
                        "digest_hex": "DD",
                        "proof": {
                            "leaf_index": 0,
                            "depth": 2,
                            "audit_path": ["11", None],
                        },
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    sample = client.sample_rbc_chunks(
        block_hash="AA",
        height=9,
        view=0,
        count=2,
        seed="10",
        api_token="secret-token",
    )

    assert sample is not None
    assert sample.block_hash == "AA"
    assert sample.samples[0].proof.audit_path[0] == "11"

    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/sumeragi/rbc/sample")
    assert call["headers"]["X-API-Token"] == "secret-token"
    assert json.loads(call["data"]) == {
        "block_hash": "AA",
        "height": 9,
        "view": 0,
        "count": 2,
        "seed": 10,
    }


def test_sample_rbc_chunks_requires_token() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=401))
    client = ToriiClient("http://node.test", session=session)

    try:
        client.sample_rbc_chunks(block_hash="AA", height=1, view=0)
    except RuntimeError as exc:
        assert "requires a valid X-API-Token" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for missing RBC token")


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


def test_get_offline_readiness_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "offline_note": True,
                "offline_one_use_keys": True,
                "offline_recursive_note_proof": False,
                "offline_fountain_qr": True,
                "offline_sync_optional": True,
                "offline_telemetry": True,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    readiness = client.get_offline_readiness()

    assert readiness.offline_note is True
    assert readiness.offline_one_use_keys is True
    assert readiness.offline_recursive_note_proof is False
    assert readiness.offline_fountain_qr is True
    assert readiness.offline_sync_optional is True
    assert readiness.offline_telemetry is True
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/offline/readiness")


def test_status_snapshot_parses_mode_and_consensus_caps() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "mode_tag": "iroha2-consensus::permissioned-sumeragi@v1",
                "staged_mode_tag": "iroha2-consensus::npos-sumeragi@v1",
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

    assert snapshot.status.mode_tag == "iroha2-consensus::permissioned-sumeragi@v1"
    assert snapshot.status.staged_mode_tag == "iroha2-consensus::npos-sumeragi@v1"
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
