from __future__ import annotations

import copy
from typing import Any, Callable, Optional

import pytest

from iroha_python.address import AccountAddress
from iroha_python.client import ToriiClient

from .helpers import RecordingSession, StubResponse


ACCOUNT_ID = AccountAddress.from_account(public_key=bytes([0x31]) * 32).to_i105(0x0171)


def _manifest_list_payload() -> dict[str, Any]:
    uaid = "uaid:" + "cd" * 32
    return {
        "uaid": uaid,
        "total": 1,
        "has_more": False,
        "count_mode": "exact",
        "manifests": [
            {
                "dataspace_id": 7,
                "dataspace_alias": "alpha",
                "manifest_hash": "ab" * 32,
                "status": "Active",
                "lifecycle": {
                    "activated_epoch": 10,
                    "expired_epoch": None,
                    "revocation": None,
                },
                "accounts": [ACCOUNT_ID],
                "manifest": {
                    "version": 1,
                    "uaid": uaid,
                    "dataspace": 7,
                    "issued_ms": 1_700_000_000_000,
                    "activation_epoch": 10,
                    "entries": [
                        {
                            "scope": {"dataspace": 7},
                            "effect": {"Allow": {"window": "PerSlot"}},
                        }
                    ],
                },
            }
        ],
    }


@pytest.mark.parametrize(
    ("asset_id", "expected_params"),
    [
        (None, {}),
        ("xor#wonderland", {"asset_id": "xor#wonderland"}),
    ],
)
def test_get_uaid_portfolio_emits_only_exact_optional_asset_id(
    asset_id: Optional[str], expected_params: dict[str, str]
) -> None:
    payload = {"uaid": "uaid:" + "ab" * 32, "totals": {}, "dataspaces": []}
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    result = client.get_uaid_portfolio(payload["uaid"], asset_id=asset_id)

    assert result == payload
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"] == "http://torii.example/v1/accounts/" + payload["uaid"] + "/portfolio"
    assert call["params"] == expected_params
    assert call["headers"] == {"Accept": "application/json"}


@pytest.mark.parametrize("asset_id", [" xor#wonderland", "xor#wonderland ", ""])
def test_get_uaid_portfolio_rejects_nonexact_asset_id_before_dispatch(
    asset_id: str,
) -> None:
    payload = {"uaid": "uaid:" + "ab" * 32, "totals": {}, "dataspaces": []}
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(ValueError):
        client.get_uaid_portfolio(payload["uaid"], asset_id=asset_id)

    assert session.calls == []


@pytest.mark.parametrize(
    "literal",
    [
        "ab" * 32,
        "UAID:" + "ab" * 32,
        "uaid:" + "AB" * 32,
        " uaid:" + "ab" * 32,
        "uaid:" + "ab" * 32 + " ",
    ],
)
def test_get_uaid_portfolio_rejects_noncanonical_literals_before_dispatch(
    literal: str,
) -> None:
    session = RecordingSession(StubResponse(200, {}))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(ValueError, match="exact canonical uaid"):
        client.get_uaid_portfolio(literal)

    assert session.calls == []


def test_typed_uaid_portfolio_rejects_noncanonical_response_literal() -> None:
    canonical = "uaid:" + "ab" * 32
    payload = {"uaid": canonical.upper(), "totals": {}, "dataspaces": []}
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(ValueError, match="exact canonical uaid"):
        client.get_uaid_portfolio_typed(canonical)


def test_typed_uaid_portfolio_requires_exact_response_fields() -> None:
    canonical = "uaid:" + "ab" * 32
    payload = {
        "uaid": canonical,
        "totals": {"accounts": 1, "positions": 0},
        "dataspaces": [
            {
                "dataspace_id": 7,
                "dataspace_alias": None,
                "accounts": [
                    {"account_id": ACCOUNT_ID, "label": None, "assets": []}
                ],
            }
        ],
    }
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    snapshot = client.get_uaid_portfolio_typed(canonical)

    assert snapshot.total_accounts == 1
    assert snapshot.dataspaces[0].accounts[0].account_id == ACCOUNT_ID

    payload["totals"]["legacy_positions"] = 0
    client = ToriiClient(
        "http://torii.example",
        session=RecordingSession(StubResponse(200, payload)),
        max_retries=0,
    )
    with pytest.raises(ValueError, match="unknown field"):
        client.get_uaid_portfolio_typed(canonical)


def test_typed_uaid_bindings_requires_accounts_and_rejects_padding() -> None:
    canonical = "uaid:" + "ab" * 32
    payload = {
        "uaid": canonical,
        "dataspaces": [
            {
                "dataspace_id": 7,
                "dataspace_alias": None,
                "accounts": [ACCOUNT_ID],
            }
        ],
    }
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.get_uaid_bindings_typed(canonical).dataspaces[0].accounts == [
        ACCOUNT_ID
    ]

    payload["dataspaces"][0]["accounts"] = [f" {ACCOUNT_ID}"]
    client = ToriiClient(
        "http://torii.example",
        session=RecordingSession(StubResponse(200, payload)),
        max_retries=0,
    )
    with pytest.raises(ValueError, match="surrounding whitespace"):
        client.get_uaid_bindings_typed(canonical)


def test_space_directory_manifest_query_and_response_are_exact_v1() -> None:
    payload = _manifest_list_payload()
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    result = client.list_space_directory_manifests_typed(
        payload["uaid"],
        dataspace=7,
        status="active",
        limit=25,
        offset=2,
        count_mode="exact",
    )

    assert result.total == 1
    assert result.has_more is False
    assert result.count_mode == "exact"
    assert result.manifests[0].manifest["version"] == 1
    assert session.calls[0]["params"] == {
        "dataspace": 7,
        "status": "active",
        "limit": 25,
        "offset": 2,
        "count_mode": "exact",
    }


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"dataspace": "7"}, "unsigned 64-bit integer"),
        ({"status": "Active"}, "status must be one of"),
        ({"status": " active"}, "surrounding whitespace"),
        ({"limit": 0}, "limit must be positive"),
        ({"offset": True}, "unsigned 64-bit integer"),
        ({"count_mode": "Exact"}, "count_mode must be"),
        ({"count_mode": "exact "}, "surrounding whitespace"),
    ],
)
def test_space_directory_manifest_query_rejects_noncanonical_options(
    kwargs: dict[str, Any],
    message: str,
) -> None:
    payload = _manifest_list_payload()
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises((TypeError, ValueError), match=message):
        client.list_space_directory_manifests(payload["uaid"], **kwargs)

    assert session.calls == []


@pytest.mark.parametrize(
    "mutation",
    [
        lambda payload: payload.pop("has_more"),
        lambda payload: payload.__setitem__("legacy_total", 1),
        lambda payload: payload["manifests"][0]["manifest"].__setitem__(
            "version", "1"
        ),
        lambda payload: payload["manifests"][0]["manifest"].__setitem__(
            "expiry_epoch", None
        ),
        lambda payload: payload["manifests"][0].__setitem__(
            "manifest_hash", "0x" + "ab" * 32
        ),
    ],
    ids=[
        "missing-pagination-field",
        "unknown-root-field",
        "string-version",
        "null-expiry",
        "prefixed-hash",
    ],
)
def test_space_directory_manifest_typed_response_rejects_legacy_shapes(
    mutation: Callable[[dict[str, Any]], Any],
) -> None:
    payload = copy.deepcopy(_manifest_list_payload())
    mutation(payload)
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises((TypeError, ValueError)):
        client.list_space_directory_manifests_typed(payload["uaid"])
