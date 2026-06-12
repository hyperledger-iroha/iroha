from __future__ import annotations

import base64
import json
from types import SimpleNamespace
from typing import Any, Dict, Optional, Union

import pytest
import requests
from requests.structures import CaseInsensitiveDict

import iroha_python.client as client_module
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


def test_get_transaction_status_defaults_to_global_scope() -> None:
    client, session = _client_with_session()
    session._response = StubResponse(payload={"status": "Committed"})
    tx_hash = "aa" * 32

    payload = client.get_transaction_status(tx_hash)

    assert payload == {"status": "Committed"}
    assert session.calls[0]["method"] == "GET"
    assert session.calls[0]["url"] == "http://localhost:8080/v1/pipeline/transactions/status"
    assert session.calls[0]["params"] == {"hash": tx_hash, "scope": "global"}


def test_wait_for_transaction_status_forwards_explicit_scope() -> None:
    client, session = _client_with_session()
    session._response = StubResponse(payload={"status": "Committed"})
    tx_hash = "bb" * 32

    payload = client.wait_for_transaction_status(
        tx_hash,
        interval=0,
        timeout=1,
        scope="local",
    )

    assert payload == {"status": "Committed"}
    assert session.calls[0]["params"] == {"hash": tx_hash, "scope": "local"}


def test_transaction_status_scope_rejects_auto_and_injected_values() -> None:
    client, session = _client_with_session()
    tx_hash = "cc" * 32

    for scope in ("auto", "global&scope=local", "local,global", "../global"):
        with pytest.raises(ValueError, match="must be one of: local, global"):
            client.get_transaction_status(tx_hash, scope=scope)

    with pytest.raises(ValueError, match="must be one of: local, global"):
        client.wait_for_transaction_status(
            tx_hash,
            interval=0,
            timeout=0,
            scope="GLOBAL\nscope=local",
        )

    assert session.calls == []


def test_build_and_submit_transaction_forwards_wait_scope(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _session = _client_with_session()
    envelope = SimpleNamespace(hash=b"\xaa" * 32)
    captured: Dict[str, Any] = {}

    class FakeCrypto:
        @staticmethod
        def build_signed_transaction(*_args: Any, **_kwargs: Any) -> Any:
            return envelope

    def fake_submit_transaction_envelope_and_wait(
        submitted_envelope: Any,
        **kwargs: Any,
    ) -> Dict[str, str]:
        captured["envelope"] = submitted_envelope
        captured.update(kwargs)
        return {"status": "Committed"}

    monkeypatch.setattr(client_module, "_require_crypto", lambda: FakeCrypto)
    monkeypatch.setattr(
        client,
        "submit_transaction_envelope_and_wait",
        fake_submit_transaction_envelope_and_wait,
    )

    envelope_out, result = client.build_and_submit_transaction(
        "00000000-0000-0000-0000-000000000000",
        "testu-authority",
        b"\x11" * 32,
        scope="local",
    )

    assert envelope_out is envelope
    assert result == {"status": "Committed"}
    assert captured["envelope"] is envelope
    assert captured["scope"] == "local"


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


def test_propose_multisig_inherited_helper_rejects_false_ok_response() -> None:
    session = RecordingSession()
    session._response = StubResponse(
        payload={
            "ok": False,
            "resolved_multisig_account_id": "ops@universal",
        }
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="ok"):
        client.propose_multisig(
            multisig_account_alias="ops@universal",
            signer_account_id="signer@universal",
            instructions=[b"\x01"],
        )


def test_propose_multisig_inherited_helper_rejects_empty_signing_message() -> None:
    session = RecordingSession()
    session._response = StubResponse(
        payload={
            "ok": True,
            "resolved_multisig_account_id": "ops@universal",
            "signing_message_b64": "",
        }
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="empty bytes"):
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


@pytest.mark.parametrize(
    ("algorithm", "message"),
    [
        ("", "non-empty string"),
        ("   ", "non-empty string"),
        (" ed25519", "surrounding whitespace"),
        ("ed25519 ", "surrounding whitespace"),
    ],
)
def test_account_address_rejects_blank_or_padded_signing_algorithm_aliases(
    algorithm: str, message: str
) -> None:
    with pytest.raises(AccountAddressError, match=message):
        AccountAddress.from_account(
            domain="wonderland",
            public_key=bytes([0x11] * 32),
            algorithm=algorithm,
        )


@pytest.mark.parametrize("algorithm", [0, False, b"ed25519", ["ed25519"]])
def test_account_address_rejects_non_string_signing_algorithm_aliases(algorithm: object) -> None:
    with pytest.raises(AccountAddressError, match="signing algorithm must be a string"):
        AccountAddress.from_account(
            domain="wonderland",
            public_key=bytes([0x11] * 32),
            algorithm=algorithm,  # type: ignore[arg-type]
        )


@pytest.mark.parametrize(
    "algorithm",
    [
        "future-curve",
        "ed\t25519",
        "ed\u200b25519",
        "\u0435d25519",
        "ml\uff0ddsa",
        "gost256\u0430",
    ],
)
def test_account_address_rejects_confusable_signing_algorithm_aliases(algorithm: str) -> None:
    with pytest.raises(AccountAddressError, match="unsupported signing algorithm"):
        AccountAddress.from_account(
            domain="wonderland",
            public_key=bytes([0x11] * 32),
            algorithm=algorithm,
        )


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
