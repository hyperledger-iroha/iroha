from __future__ import annotations

import base64
import json
from typing import Any, Dict, Optional, Union

import pytest
import requests
from requests.structures import CaseInsensitiveDict

from iroha_python import (
    MultisigResponse,
    ToriiClient,
    account_query_envelope,
    asset_holders_query_envelope,
)
from iroha_python.address import AccountAddress, AccountAddressError


class StubResponse(requests.Response):
    def __init__(self, payload: Optional[Dict[str, Any]] = None) -> None:
        super().__init__()
        self.status_code = 200
        self._payload = payload or {"items": [], "total": 0}
        self.headers = CaseInsensitiveDict({"Content-Type": "application/json"})
        self._content = json.dumps(self._payload).encode("utf-8")
        self.encoding = "utf-8"

    def json(self, **kwargs: Any) -> Any:
        return json.loads(self.content.decode("utf-8"))

    def close(self) -> None:
        return None

    def __enter__(self) -> "StubResponse":
        return self

    def __exit__(self, *args: object) -> None:
        self.close()
        return None


class RecordingSession(requests.Session):
    def __init__(self) -> None:
        super().__init__()
        self.calls: list[Dict[str, Any]] = []
        self._response = StubResponse()

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
        return self._response


def _client_with_session() -> tuple[ToriiClient, RecordingSession]:
    session = RecordingSession()
    client = ToriiClient("http://localhost:8080", session=session)
    return client, session


def test_account_query_envelope_omits_canonical_i105() -> None:
    payload = account_query_envelope()
    assert "canonical_i105" not in payload


def test_asset_holders_envelope_omits_canonical_i105() -> None:
    payload = asset_holders_query_envelope()
    assert "canonical_i105" not in payload


def test_list_accounts_omits_canonical_i105_param() -> None:
    client, session = _client_with_session()

    client.list_accounts()

    params = session.calls[0]["params"]
    assert "canonical_i105" not in params


def test_list_accounts_rejects_removed_canonical_i105_arg() -> None:
    client, _ = _client_with_session()

    with pytest.raises(TypeError):
        client.list_accounts(canonical_i105="i105")


def test_query_accounts_omits_canonical_i105() -> None:
    client, session = _client_with_session()

    client.query_accounts()

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert "canonical_i105" not in body


def test_list_asset_holders_omits_canonical_i105() -> None:
    client, session = _client_with_session()

    client.list_asset_holders("xor#wonderland")

    assert "canonical_i105" not in session.calls[0]["params"]


def test_query_asset_holders_omits_canonical_i105() -> None:
    client, session = _client_with_session()

    client.query_asset_holders("xor#wonderland")

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert "canonical_i105" not in body


def test_propose_multisig_inherited_helper_posts_native_instruction_payload() -> None:
    session = RecordingSession()
    session._response = StubResponse(
        payload={
            "ok": True,
            "resolved_multisig_account_id": "ops@universal",
            "submitted": False,
        }
    )
    client = ToriiClient("http://node.test", session=session)

    response = client.propose_multisig(
        multisig_account_alias="ops@universal",
        signer_account_id="signer@universal",
        instructions=[b"\x01\x02\x03"],
        creation_time_ms=0,
    )

    assert isinstance(response, MultisigResponse)
    assert response.ok is True
    assert response.submitted is False
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"] == "http://node.test/v1/multisig/propose"
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["multisig_account_alias"] == "ops@universal"
    assert payload["signer_account_id"] == "signer@universal"
    assert payload["creation_time_ms"] == 0
    assert payload["instructions"] == [base64.b64encode(b"\x01\x02\x03").decode("ascii")]


def test_propose_multisig_inherited_helper_rejects_bad_payload_shape() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())

    with pytest.raises(ValueError, match="exactly one"):
        client.propose_multisig(
            multisig_account_id="ops@universal",
            multisig_account_alias="ops@universal",
            signer_account_id="signer@universal",
            instructions=[b"\x01"],
        )
    with pytest.raises(RuntimeError, match="valid base64"):
        client.propose_multisig(
            multisig_account_alias="ops@universal",
            signer_account_id="signer@universal",
            instructions=["not base64"],
        )


def test_propose_multisig_inherited_helper_rejects_malformed_response() -> None:
    session = RecordingSession()
    session._response = StubResponse(
        payload={
            "ok": True,
            "resolved_multisig_account_id": "ops@universal",
            "signing_message_b64": "not base64",
        }
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="valid base64"):
        client.propose_multisig(
            multisig_account_alias="ops@universal",
            signer_account_id="signer@universal",
            instructions=[b"\x01"],
        )


def test_propose_multisig_inherited_helper_rejects_negative_response_time() -> None:
    session = RecordingSession()
    session._response = StubResponse(
        payload={
            "ok": True,
            "resolved_multisig_account_id": "ops@universal",
            "creation_time_ms": -1,
        }
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="non-negative"):
        client.propose_multisig(
            multisig_account_alias="ops@universal",
            signer_account_id="signer@universal",
            instructions=[b"\x01"],
        )


def test_i105_roundtrip_uses_halfwidth_iroha_poem_alphabet() -> None:
    address = AccountAddress.from_account(domain="wonderland", public_key=bytes([0x11] * 32))
    literal = address.to_i105(0x02F1)

    parsed = AccountAddress.parse_encoded(literal, expected_discriminant=0x02F1)

    payload = literal.removeprefix("sora")
    assert any(ch.isascii() and ch.isalnum() for ch in payload)
    assert any(ch in "ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ" for ch in payload)
    assert parsed.to_i105(0x02F1) == literal


def test_i105_parse_without_expected_discriminant_accepts_literal_prefix() -> None:
    address = AccountAddress.from_account(domain="wonderland", public_key=bytes([0x11] * 32))
    literal = address.to_i105(0x0171)

    parsed = AccountAddress.parse_encoded(literal)

    assert literal.startswith("test")
    assert parsed.to_i105(0x0171) == literal
    assert AccountAddress.from_i105(literal).to_i105(0x0171) == literal
    with pytest.raises(AccountAddressError, match="unexpected i105 chain discriminant"):
        AccountAddress.parse_encoded(literal, expected_discriminant=0x02F1)


def test_i105_numeric_discriminant_must_fit_u16() -> None:
    address = AccountAddress.from_account(domain="wonderland", public_key=bytes([0x11] * 32))
    valid = address.to_i105(0xFFFF)
    payload = address.to_i105(0x02F1).removeprefix("sora")

    assert valid.startswith("n65535")
    assert AccountAddress.parse_encoded(valid).to_i105(0xFFFF) == valid
    for discriminant in (-1, 0x10000, 70000):
        with pytest.raises(AccountAddressError, match="between 0 and 65535"):
            address.to_i105(discriminant)
    for literal in (f"n65536{payload}", f"n70000{payload}"):
        with pytest.raises(AccountAddressError, match="between 0 and 65535"):
            AccountAddress.parse_encoded(literal)


def test_i105_rejects_fullwidth_sentinel_literal() -> None:
    address = AccountAddress.from_account(domain="wonderland", public_key=bytes([0x11] * 32))
    literal = address.to_i105(0x02F1)
    noncanonical = literal.replace("sora", "ｓｏｒａ", 1)

    with pytest.raises(AccountAddressError, match="missing the expected"):
        AccountAddress.parse_encoded(noncanonical, expected_discriminant=0x02F1)


def test_i105_rejects_noncanonical_fullwidth_kana_payload() -> None:
    address = AccountAddress.from_account(domain="wonderland", public_key=bytes([0x11] * 32))
    literal = address.to_i105(0x02F1)
    noncanonical = literal
    for halfwidth, fullwidth in (("ﾛ", "ロ"), ("ﾊ", "ハ"), ("ﾆ", "ニ"), ("ﾎ", "ホ")):
        if halfwidth in noncanonical:
            noncanonical = noncanonical.replace(halfwidth, fullwidth, 1)
            break
    assert noncanonical != literal

    with pytest.raises(AccountAddressError, match="invalid i105 alphabet symbol"):
        AccountAddress.parse_encoded(noncanonical, expected_discriminant=0x02F1)


def test_query_asset_holders_rejects_removed_canonical_i105_arg() -> None:
    client, _ = _client_with_session()

    with pytest.raises(TypeError):
        client.query_asset_holders("xor#wonderland", canonical_i105="i105")
