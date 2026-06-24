from __future__ import annotations

import json
from typing import Any

import pytest
import requests
from requests.structures import CaseInsensitiveDict

from iroha_python import ToriiClient
from iroha_torii_client.client import ToriiCanonicalRequestAuth

from .helpers import StubResponse


class SequencedSession(requests.Session):
    """Capture outgoing requests and return responses in order."""

    def __init__(self, responses: list[requests.Response]) -> None:
        super().__init__()
        self._responses = list(responses)
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str | bytes, url: str | bytes, *args: Any, **kwargs: Any) -> requests.Response:
        self.calls.append(
            {
                "method": method,
                "url": url,
                "params": kwargs.get("params") or {},
                "headers": kwargs.get("headers") or {},
                "data": kwargs.get("data"),
                "stream": kwargs.get("stream"),
            }
        )
        if not self._responses:
            raise AssertionError("unexpected HTTP request")
        return self._responses.pop(0)

    def get(self, url: str | bytes, **kwargs: Any) -> requests.Response:
        return self.request("GET", url, **kwargs)


class SseStubResponse(StubResponse):
    """Minimal SSE response for `_stream_sse` tests."""

    def __init__(self, lines: list[str]) -> None:
        super().__init__(200, None)
        self.headers = CaseInsensitiveDict({"Content-Type": "text/event-stream"})
        self._lines = lines

    def iter_lines(self, decode_unicode: bool = False, **kwargs: Any):
        for line in self._lines:
            yield line if decode_unicode else line.encode("utf-8")


class FakeWebSocket:
    """Deterministic WebSocket stub for orderbook frame streaming tests."""

    def __init__(self, frames: list[str]) -> None:
        self.frames = list(frames)
        self.closed = False

    def recv(self) -> str:
        if not self.frames:
            raise AssertionError("unexpected WebSocket recv")
        return self.frames.pop(0)

    def close(self) -> None:
        self.closed = True


def _sample_orderbook_event() -> dict[str, Any]:
    return {
        "sequence": 9,
        "kind": "settlement_receipt_accepted",
        "generated_at_unix": 1_700_000_104,
        "order_id_hex": None,
        "trade_ids_hex": ["22" * 32],
        "settlement_channel_ids_hex": ["33" * 32],
        "receipt_id_hex": "44" * 32,
        "expired_order_ids_hex": ["11" * 32],
        "open_order_count": 1,
        "open_settlement_channel_count": 1,
        "settlement_receipt_count": 1,
    }


def _sample_orderbook_signature() -> dict[str, str]:
    return {
        "algorithm": "Ed25519",
        "public_key_hex": "aa" * 32,
        "signature_hex": "bb" * 64,
    }


def _sample_orderbook_order() -> dict[str, Any]:
    return {
        "version": 1,
        "order_id_hex": "11" * 32,
        "side": "bid",
        "tier": "hot",
        "price_per_gib_micro_xor": "1500000",
        "quantity_gib": 4,
        "remaining_gib": 2,
        "owner_account_hex": "cafe",
        "expiry_unix": 1_800_000_000,
        "nonce": 7,
        "maker_fee_bps": 25,
        "taker_fee_bps": 35,
        "signature": _sample_orderbook_signature(),
    }


def _sample_orderbook_trade() -> dict[str, Any]:
    return {
        "version": 1,
        "trade_id_hex": "22" * 32,
        "maker_order_id_hex": "11" * 32,
        "taker_order_id_hex": "77" * 32,
        "tier": "hot",
        "price_per_gib_micro_xor": "1500000",
        "filled_gib": 2,
        "maker_fee_micro_xor": "75000",
        "taker_fee_micro_xor": "105000",
        "timestamp_unix": 1_700_000_100,
    }


def _sample_orderbook_channel() -> dict[str, Any]:
    return {
        "version": 1,
        "channel_id_hex": "33" * 32,
        "trade_id_hex": "22" * 32,
        "buyer_account_hex": "face",
        "provider_id_hex": "55" * 32,
        "total_bytes": 2_147_483_648,
        "remaining_bytes": 1_073_741_824,
        "xor_locked_micro": "3000000",
        "status": "open",
        "opened_at_unix": 1_700_000_101,
        "updated_at_unix": 1_700_000_102,
    }


def _sample_orderbook_receipt() -> dict[str, Any]:
    return {
        "version": 1,
        "receipt_id_hex": "44" * 32,
        "channel_id_hex": "33" * 32,
        "trade_id_hex": "22" * 32,
        "range": {"start": 0, "end": 1024},
        "chunk_hash_hex": "66" * 32,
        "bytes_delivered": 1024,
        "xor_debited_micro": "1500",
        "provider_credit_micro": "1400",
        "fee_amount_micro": "100",
        "issued_at_unix": 1_700_000_103,
        "settlement_signature": _sample_orderbook_signature(),
    }


def test_sorafs_orderbook_read_helper_normalizes_events() -> None:
    event = _sample_orderbook_event()
    session = SequencedSession(
        [
            StubResponse(
                200,
                {
                    "since": 0,
                    "limit": 10,
                    "count": 1,
                    "next_since": 9,
                    "events": [event],
                },
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    events = client.list_sorafs_orderbook_events(
        since=0,
        limit="10",
        if_none_match='"old-events"',
    )

    assert events is not None
    assert events["events"][0]["kind"] == "settlement_receipt_accepted"
    assert events["events"][0]["receipt_id_hex"] == "44" * 32
    assert session.calls[0]["params"] == {"since": 0, "limit": 10}
    assert session.calls[0]["headers"]["If-None-Match"] == '"old-events"'


def test_sorafs_orderbook_submit_helpers_sign_exact_payload_bytes() -> None:
    order = _sample_orderbook_order()
    trade = _sample_orderbook_trade()
    channel = _sample_orderbook_channel()
    receipt = _sample_orderbook_receipt()
    session = SequencedSession(
        [
            StubResponse(
                200,
                {
                    "status": "accepted",
                    "sequence": 12,
                    "open_order_count": 1,
                    "accepted_order": order,
                    "fills": [
                        {
                            "trade": trade,
                            "maker_remaining_gib": 0,
                            "taker_remaining_gib": 2,
                            "gross_value_micro_xor": "3000000",
                        }
                    ],
                    "settlement_channels_opened": [channel],
                    "expired_order_ids_hex": ["11" * 32],
                },
            ),
            StubResponse(
                200,
                {
                    "status": "cancelled",
                    "reason": "owner_requested",
                    "open_order_count": 0,
                    "cancelled_order": order,
                },
            ),
            StubResponse(
                200,
                {
                    "status": "accepted",
                    "settlement_receipt_count": 1,
                    "open_settlement_channel_count": 1,
                    "accepted_receipt": receipt,
                    "updated_channel": channel,
                },
            ),
        ]
    )
    signed_messages: list[bytes] = []

    def signer(message: bytes) -> bytes:
        signed_messages.append(message)
        return b"signed-request"

    auth = ToriiCanonicalRequestAuth(
        account_id="alice@wonderland",
        signer=signer,
        timestamp_ms=1234,
        nonce="nonce-1",
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    submitted = client.submit_sorafs_orderbook_order(
        b"\x01\x02\x03",
        canonical_auth=auth,
        headers={"X-Trace": "order-submit"},
    )
    assert submitted["status"] == "accepted"
    assert submitted["fills"][0]["gross_value_micro_xor"] == "3000000"
    assert session.calls[0]["url"] == "http://torii.example/v1/sorafs/orderbook/orders"
    assert session.calls[0]["data"] == b"\x01\x02\x03"
    assert session.calls[0]["headers"]["Content-Type"] == "application/octet-stream"
    assert session.calls[0]["headers"]["X-Trace"] == "order-submit"
    assert session.calls[0]["headers"]["X-Iroha-Account"] == "alice@wonderland"
    assert session.calls[0]["headers"]["X-Iroha-Nonce"] == "nonce-1"
    assert signed_messages[0].startswith(b"POST\n/v1/sorafs/orderbook/orders\n")

    cancelled = client.submit_sorafs_orderbook_cancel([4, 5], canonical_auth=auth)
    assert cancelled["status"] == "cancelled"
    assert cancelled["cancelled_order"]["order_id_hex"] == "11" * 32
    assert session.calls[1]["data"] == b"\x04\x05"

    receipt_result = client.submit_sorafs_orderbook_receipt(
        bytearray([6]),
        canonical_auth=auth,
    )
    assert receipt_result["accepted_receipt"]["receipt_id_hex"] == "44" * 32
    assert session.calls[2]["url"] == "http://torii.example/v1/sorafs/orderbook/receipts"


def test_sorafs_orderbook_stream_helper_parses_and_normalizes_sse() -> None:
    event = _sample_orderbook_event()
    session = SequencedSession(
        [
            SseStubResponse(
                [
                    "id: 9",
                    "event: settlement_receipt_accepted",
                    f"data: {json.dumps(event)}",
                    "",
                ]
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    iterator = client.stream_sorafs_orderbook_events(
        since=8,
        limit=1,
        last_event_id="8",
        max_retries=0,
        with_metadata=True,
    )
    streamed = next(iterator)

    assert streamed.event == "settlement_receipt_accepted"
    assert streamed.id == "9"
    assert streamed.data["receipt_id_hex"] == "44" * 32
    assert session.calls[0]["params"] == {"since": 8, "limit": 1}
    assert session.calls[0]["headers"]["Last-Event-ID"] == "8"


def test_sorafs_orderbook_stream_helper_validates_inputs_before_request() -> None:
    client = ToriiClient("http://torii.example", session=SequencedSession([]), max_retries=0)

    with pytest.raises(ValueError, match="positive"):
        client.stream_sorafs_orderbook_events(limit=0)
    with pytest.raises(ValueError, match="canonical_auth is required"):
        client.submit_sorafs_orderbook_order(b"\x01", canonical_auth=None)  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="must not be empty"):
        client.submit_sorafs_orderbook_receipt(
            b"",
            canonical_auth=ToriiCanonicalRequestAuth(
                account_id="alice@wonderland",
                signer=lambda _message: b"sig",
            ),
        )


def test_sorafs_orderbook_websocket_helper_opens_and_normalizes_frames() -> None:
    event = _sample_orderbook_event()
    socket = FakeWebSocket(
        [
            json.dumps({"event": "settlement_receipt_accepted", "data": event}),
            json.dumps({"event": "lagged", "data": {"skipped": 2}}),
        ]
    )
    captured: dict[str, Any] = {}

    def factory(url: str, **kwargs: Any) -> FakeWebSocket:
        captured["url"] = url
        captured["kwargs"] = kwargs
        return socket

    client = ToriiClient("https://torii.example", session=SequencedSession([]), max_retries=0)
    assert (
        client.build_sorafs_orderbook_events_websocket_url(since=8, limit=1)
        == "wss://torii.example/v1/sorafs/orderbook/events/ws?since=8&limit=1"
    )

    iterator = client.stream_sorafs_orderbook_events_websocket(
        since=8,
        limit=1,
        subprotocols=["iroha.sorafs.orderbook.v1"],
        websocket_factory=factory,
        with_metadata=True,
    )
    first = next(iterator)
    assert first.event == "settlement_receipt_accepted"
    assert first.data["receipt_id_hex"] == "44" * 32
    assert captured["url"] == "wss://torii.example/v1/sorafs/orderbook/events/ws?since=8&limit=1"
    assert captured["kwargs"]["subprotocols"] == ["iroha.sorafs.orderbook.v1"]

    lagged = next(iterator)
    assert lagged.event == "lagged"
    assert lagged.data == {"skipped": 2}

    iterator.close()
    assert socket.closed is True


def test_sorafs_orderbook_websocket_helper_validates_inputs_before_connect() -> None:
    client = ToriiClient("http://torii.example", session=SequencedSession([]), max_retries=0)

    with pytest.raises(ValueError, match="positive"):
        client.build_sorafs_orderbook_events_websocket_url(limit=0)
    with pytest.raises(ValueError, match="must start with '/'"):
        client.connect_sorafs_orderbook_events_websocket(
            endpoint_path="v1/sorafs/orderbook/events/ws",
            websocket_factory=lambda *_args, **_kwargs: FakeWebSocket([]),
        )
