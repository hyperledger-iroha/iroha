"""Exact-network and one-shot admission tests for subscription commands."""

from __future__ import annotations

import inspect
import json
import sys
from pathlib import Path
from typing import List

import pytest
from client_test_support import CANONICAL_OWNER, app_api_transaction_draft, canonical_hash
from sumeragi_exact_json_test_support import RecordingSession, StubResponse

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import (  # noqa: E402
    ToriiCanonicalRequestAuth,
    ToriiClient,
    canonical_network_request_signature_message,
)

NETWORK_ID = canonical_hash(0xA5)
FOREIGN_NETWORK_ID = canonical_hash(0xA7)


def _auth(captured: List[bytes] | None = None) -> ToriiCanonicalRequestAuth:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return b"\x44" * 64

    return ToriiCanonicalRequestAuth(
        network_id=NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=signer,
        timestamp_ms=4_102_444_801_000,
        nonce="subscription-command-test",
    )


def test_subscription_plan_command_signs_exact_body_once_without_redirect() -> None:
    payload = app_api_transaction_draft()
    payload["plan_id"] = "fixed-plan"
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    captured: List[bytes] = []
    auth = _auth(captured)
    client = ToriiClient("https://node.test", session=session)

    result = client.create_subscription_plan(
        authority=CANONICAL_OWNER,
        plan_id="fixed-plan",
        plan={"provider": CANONICAL_OWNER},
        canonical_auth=auth,
    )

    assert result.plan_id == "fixed-plan"
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["allow_redirects"] is False
    assert call["headers"]["X-Iroha-Account"] == CANONICAL_OWNER
    assert json.loads(call["data"].decode("utf-8"))["authority"] == CANONICAL_OWNER
    expected = canonical_network_request_signature_message(
        NETWORK_ID,
        "POST",
        "/v1/subscriptions/plans",
        call["data"],
        timestamp_ms=auth.timestamp_ms or 0,
        nonce=auth.nonce or "",
    )
    assert captured == [expected]
    assert expected != canonical_network_request_signature_message(
        NETWORK_ID,
        "POST",
        "/v1/subscriptions/other",
        call["data"],
        timestamp_ms=auth.timestamp_ms or 0,
        nonce=auth.nonce or "",
    )
    assert expected != canonical_network_request_signature_message(
        FOREIGN_NETWORK_ID,
        "POST",
        "/v1/subscriptions/plans",
        call["data"],
        timestamp_ms=auth.timestamp_ms or 0,
        nonce=auth.nonce or "",
    )
    assert expected != canonical_network_request_signature_message(
        NETWORK_ID,
        "POST",
        "/v1/subscriptions/plans",
        call["data"] + b" ",
        timestamp_ms=auth.timestamp_ms or 0,
        nonce=auth.nonce or "",
    )


def test_subscription_command_rejects_principal_mismatch_before_transport() -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(ValueError, match="account_id must equal payload authority"):
        client.create_subscription_plan(
            authority="mallory@universal",
            plan_id="fixed-plan",
            plan={},
            canonical_auth=_auth(),
        )
    assert session.calls == []


def test_all_subscription_mutations_retire_unsigned_call_shapes() -> None:
    for method_name in (
        "create_subscription_plan",
        "create_subscription",
        "pause_subscription",
        "resume_subscription",
        "cancel_subscription",
        "keep_subscription",
        "charge_subscription_now",
        "record_subscription_usage",
    ):
        parameters = inspect.signature(getattr(ToriiClient, method_name)).parameters
        assert "private_key" not in parameters
        parameter = parameters["canonical_auth"]
        assert parameter.default is inspect.Parameter.empty
        assert parameter.kind is inspect.Parameter.KEYWORD_ONLY
    cancel_mode = inspect.signature(ToriiClient.cancel_subscription).parameters[
        "cancel_mode"
    ]
    assert cancel_mode.default is inspect.Parameter.empty


def test_subscription_mutations_never_send_private_keys_and_bind_cancel_mode() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "version": 1,
                "authority": CANONICAL_OWNER,
                "action": "create",
                "subscription_id": "subscription-1",
                "plan_id": "plan-1",
                "billing_trigger_id": "billing-1",
                "usage_trigger_id": None,
                "first_charge_ms": 7,
                "provider_usage_grant_included": False,
                "resulting_subscription": {},
                "tx_instructions": [{"wire_id": "register", "payload_hex": "00"}],
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "version": 1,
                "authority": CANONICAL_OWNER,
                "action": "cancel",
                "subscription_id": "subscription-1",
                "details": {},
                "tx_instructions": [{"wire_id": "set", "payload_hex": "00"}],
            }
        )
    )
    client = ToriiClient("https://node.test", session=session)

    client.create_subscription(
        authority=CANONICAL_OWNER,
        subscription_id="subscription-1",
        plan_id="plan-1",
        canonical_auth=_auth(),
    )
    client.cancel_subscription(
        "subscription-1",
        authority=CANONICAL_OWNER,
        cancel_mode="period_end",
        canonical_auth=_auth(),
    )

    create_payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    cancel_payload = json.loads(session.calls[1]["data"].decode("utf-8"))
    assert create_payload["authority"] == CANONICAL_OWNER
    assert "private_key" not in create_payload
    assert cancel_payload == {
        "authority": CANONICAL_OWNER,
        "cancel_mode": {"mode": "period_end", "value": None},
    }
    assert "private_key" not in inspect.signature(
        ToriiClient.create_subscription
    ).parameters
    assert "private_key" not in inspect.signature(
        ToriiClient.cancel_subscription
    ).parameters

    with pytest.raises(ValueError, match="cancel_mode must be immediate or period_end"):
        client.cancel_subscription(
            "subscription-1",
            authority=CANONICAL_OWNER,
            cancel_mode="next_week",
            canonical_auth=_auth(),
        )
    assert len(session.calls) == 2
