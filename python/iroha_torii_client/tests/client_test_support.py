"""Shared fixture builders for the low-level Torii client tests."""

from __future__ import annotations

import base64
import hashlib
from typing import Any, Dict, Optional
from urllib.parse import quote

CANONICAL_OWNER = "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"


def authority_fee_payment(gas_limit: Optional[int] = None) -> Dict[str, Any]:
    return {
        "payer": "authority",
        "value": {"charge_limits": [], "gas_limit": gas_limit},
    }


def sponsor_fee_payment(gas_limit: Optional[int] = None) -> Dict[str, Any]:
    return {
        "payer": "sponsor",
        "value": {
            "program_id": {"sponsor": CANONICAL_OWNER, "name": "retail"},
            "program_revision": 3,
            "charge_limits": [],
            "gas_limit": gas_limit,
        },
    }


def app_api_transaction_draft(payload: bytes = b"\x01\x02\x03") -> Dict[str, Any]:
    signing_message = bytearray(hashlib.blake2b(payload, digest_size=32).digest())
    signing_message[-1] |= 1
    return {
        "submitted": False,
        "transaction_payload_b64": base64.b64encode(payload).decode("ascii"),
        "signing_message_b64": base64.b64encode(signing_message).decode("ascii"),
    }


def canonical_hash(seed: int) -> str:
    """Return a canonical Norito hash literal derived from a fixture byte."""

    body_bytes = bytearray([seed & 0xFF] * 32)
    body_bytes[-1] |= 1
    body = body_bytes.hex().upper()
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return f"hash:{body}#{crc:04X}"


def connect_base64url(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode("ascii")


def connect_session_fixture(
    *,
    network_seed: int = 0xA5,
    app_seed: int = 0x41,
    nonce_seed: int = 0x51,
) -> tuple[Dict[str, str], Dict[str, Any]]:
    """Build a mutually consistent exact-identity Connect request and response."""

    network_id = canonical_hash(network_seed)
    network_bytes = bytes.fromhex(network_id[5:69])
    app_pk_bytes = bytes([app_seed]) * 32
    nonce_bytes = bytes([nonce_seed]) * 16
    sid_bytes = hashlib.blake2b(
        b"iroha-connect|sid|" + network_bytes + app_pk_bytes + nonce_bytes,
        digest_size=32,
    ).digest()
    sid = connect_base64url(sid_bytes)
    app_pk = connect_base64url(app_pk_bytes)
    nonce = connect_base64url(nonce_bytes)
    node = "node.example:443"
    token_app = connect_base64url(bytes([0x61]) * 32)
    token_wallet = connect_base64url(bytes([0x62]) * 32)
    token_management = connect_base64url(bytes([0x63]) * 32)
    token_relay = connect_base64url(bytes([0x64]) * 32)

    def role_uri(role: str, token: str) -> str:
        return (
            "iroha://connect"
            f"?sid={sid}&network_id={quote(network_id, safe='')}&app_pk={app_pk}"
            f"&nonce={nonce}&node={quote(node, safe='')}&v=1&role={role}"
            f"&token={token}&relay={token_relay}"
        )

    request = {
        "sid": sid,
        "network_id": network_id,
        "app_pk": app_pk,
        "nonce": nonce,
        "node": node,
    }
    response: Dict[str, Any] = {
        **request,
        "wallet_uri": role_uri("wallet", token_wallet),
        "app_uri": role_uri("app", token_app),
        "token_app": token_app,
        "token_wallet": token_wallet,
        "token_management": token_management,
        "token_relay": token_relay,
        "ttl": 30,
    }
    response.pop("node")
    return request, response
