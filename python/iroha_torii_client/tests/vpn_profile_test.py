from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

import pytest
import requests
from requests.structures import CaseInsensitiveDict

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import ToriiClient  # noqa: E402

VPN_OPERATOR = "vpn-operator@paynet"
VPN_RELAY_ID_HEX = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
VPN_RELAY_MLDSA65_PUBLIC_KEY_HEX = "55" * 1_952


class StubResponse(requests.Response):
    def __init__(
        self,
        status_code: int = 200,
        payload: Optional[Any] = None,
    ) -> None:
        super().__init__()
        self.status_code = status_code
        self._payload = payload
        self.headers = CaseInsensitiveDict({"Content-Type": "application/json"})
        self._content = json.dumps(payload).encode("utf-8")
        self.encoding = "utf-8"

    def json(self, **kwargs: Any) -> Any:
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
        self.calls.append(
            {
                "method": method,
                "url": url,
                "params": kwargs.get("params") or {},
                "headers": kwargs.get("headers") or {},
                "data": kwargs.get("data"),
                "allow_redirects": kwargs.get("allow_redirects"),
            }
        )
        if not self._responses:
            raise AssertionError("no queued responses")
        return self._responses.pop(0)

    def send(
        self,
        request: requests.PreparedRequest,
        **kwargs: Any,
    ) -> requests.Response:
        return self.request(
            request.method or "",
            request.url or "",
            headers=dict(request.headers),
            data=request.body,
            **kwargs,
        )


def _vpn_trust_fields() -> Dict[str, str]:
    return {
        "relay_id_hex": VPN_RELAY_ID_HEX,
        "relay_mldsa65_public_key_hex": VPN_RELAY_MLDSA65_PUBLIC_KEY_HEX,
        "descriptor_commit_hex": "cd" * 32,
        "tls_server_name": "relay.example",
        "relay_tls_spki_sha256_hex": "ab" * 32,
        "relay_certificate_sha256_hex": "ef" * 32,
        "directory_snapshot_digest_hex": "42" * 32,
    }


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


def test_vpn_profile_deserializes_native_lease_fields() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=_vpn_profile_payload()))
    client = ToriiClient("https://node.test", session=session)

    profile = client.get_vpn_profile()

    assert profile.lease_fee == "100.25"
    assert profile.operator_account_id == VPN_OPERATOR
    assert profile.route_pushes == ["0.0.0.0/0"]
    assert profile.relay_mldsa65_public_key_hex == VPN_RELAY_MLDSA65_PUBLIC_KEY_HEX
    assert session.calls[0]["url"] == "https://node.test/v1/vpn/profile"
    assert session.calls[0]["headers"] == {"Accept": "application/json"}
    assert session.calls[0]["allow_redirects"] is False


def test_vpn_profile_rejects_insecure_transport_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="HTTPS"):
        client.get_vpn_profile()

    assert session.calls == []


def test_unavailable_vpn_profile_accepts_only_the_explicit_empty_trust_tuple() -> None:
    payload = _vpn_profile_payload()
    payload["available"] = False
    for field in (
        "relay_endpoint",
        "relay_id_hex",
        "relay_mldsa65_public_key_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
    ):
        payload[field] = ""

    profile = ToriiClient._parse_vpn_profile(payload, context="vpn profile")

    assert profile.available is False
    assert profile.relay_endpoint == ""
    assert profile.relay_id_hex == ""
    assert profile.relay_mldsa65_public_key_hex == ""


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("relay_id_hex", "00" * 32),
        ("relay_mldsa65_public_key_hex", ""),
        ("relay_mldsa65_public_key_hex", "55" * 1_951),
        ("relay_mldsa65_public_key_hex", "AA" * 1_952),
        ("relay_mldsa65_public_key_hex", "55" * 1_951 + "5\n"),
        ("relay_mldsa65_public_key_hex", "00" * 1_952),
        ("descriptor_commit_hex", "00" * 32),
        ("descriptor_commit_hex", "0x" + "cd" * 32),
        ("tls_server_name", "Relay.Example"),
        ("tls_server_name", "-relay.example"),
        ("relay_endpoint", "/dns4/Relay.Example/udp/443/quic"),
        ("relay_endpoint", "/dns4/relay.example/udp/0443/quic"),
        ("relay_endpoint", "/dns4/relay.example/tcp/443/quic"),
    ],
)
def test_available_vpn_profile_rejects_malformed_trust_tuple(
    field: str,
    value: str,
) -> None:
    payload = _vpn_profile_payload()
    payload[field] = value

    with pytest.raises(RuntimeError):
        ToriiClient._parse_vpn_profile(payload, context="vpn profile")


@pytest.mark.parametrize("invalid_fee", [100, "01", "-1", "1.0"])
def test_vpn_profile_rejects_noncanonical_quantity_fee(invalid_fee: Any) -> None:
    payload = _vpn_profile_payload()
    payload["lease_fee"] = invalid_fee
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    client = ToriiClient("https://node.test", session=session)

    with pytest.raises(RuntimeError, match="lease_fee"):
        client.get_vpn_profile()


@pytest.mark.parametrize("dns_push_interval_secs", [None, 0, 29], ids=["missing", "zero", "below-minimum"])
def test_vpn_profile_requires_dns_push_interval_of_at_least_30(
    dns_push_interval_secs: Optional[int],
) -> None:
    payload = _vpn_profile_payload()
    if dns_push_interval_secs is None:
        payload.pop("dns_push_interval_secs")
    else:
        payload["dns_push_interval_secs"] = dns_push_interval_secs

    with pytest.raises(RuntimeError, match="dns_push_interval_secs"):
        ToriiClient._parse_vpn_profile(payload, context="vpn profile")
