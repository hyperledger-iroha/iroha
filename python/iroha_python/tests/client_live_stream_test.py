from __future__ import annotations

import inspect
import json
from typing import Any
from urllib.parse import urlsplit

import pytest
import requests
from requests.structures import CaseInsensitiveDict

import iroha_python
import iroha_python.client as client_module
from iroha_python import (
    EventCursor,
    NetworkId,
    OperatorSigningContext,
    SseStreamError,
    ToriiCanonicalRequestAuth,
    ToriiClient,
    canonical_network_request_signature_message,
)

from .helpers import StubResponse


class SequencedSession(requests.Session):
    """Capture streaming requests and return queued SSE responses."""

    def __init__(self, responses: list[requests.Response]) -> None:
        super().__init__()
        self._responses = list(responses)
        self.calls: list[dict[str, Any]] = []

    def request(
        self,
        method: str | bytes,
        url: str | bytes,
        **kwargs: Any,
    ) -> requests.Response:
        self.calls.append(
            {
                "url": url,
                "params": kwargs.get("params"),
                "headers": kwargs.get("headers") or {},
                "stream": kwargs.get("stream"),
                "allow_redirects": kwargs.get("allow_redirects"),
            }
        )
        if not self._responses:
            raise AssertionError("unexpected SSE request")
        response = self._responses.pop(0)
        response.url = str(url)
        return response

    def get(self, url: str | bytes, **kwargs: Any) -> requests.Response:
        return self.request("GET", url, **kwargs)

    def send(
        self,
        request: requests.PreparedRequest,
        **kwargs: Any,
    ) -> requests.Response:
        self.calls.append(
            {
                "url": request.url,
                "params": {},
                "headers": dict(request.headers),
                "stream": kwargs.get("stream"),
                "allow_redirects": kwargs.get("allow_redirects"),
            }
        )
        if not self._responses:
            raise AssertionError("unexpected prepared SSE request")
        response = self._responses.pop(0)
        response.request = request
        response.url = request.url
        return response


class SseStubResponse(StubResponse):
    """Minimal successful SSE response."""

    def __init__(self, lines: list[str]) -> None:
        super().__init__(200, None)
        self.headers = CaseInsensitiveDict({"Content-Type": "text/event-stream"})
        self._lines = lines

    def iter_content(self, chunk_size: int = 1, decode_unicode: bool = False):
        del chunk_size
        assert decode_unicode is False
        for line in self._lines:
            yield line.encode("utf-8") + b"\n"


class ChunkSseStubResponse(StubResponse):
    """SSE response that can emit a newline-free hostile chunk."""

    def __init__(self, chunks: list[bytes | Exception]) -> None:
        super().__init__(200, None)
        self.headers = CaseInsensitiveDict({"Content-Type": "text/event-stream"})
        self._chunks = chunks

    def iter_content(self, chunk_size: int = 1, decode_unicode: bool = False):
        del chunk_size
        assert decode_unicode is False
        for chunk in self._chunks:
            if isinstance(chunk, Exception):
                raise chunk
            yield chunk


class StubOperatorKeyPair:
    """Deterministic operator signer sufficient for transport-boundary tests."""

    public_key_multihash = "ed0120" + "11" * 32

    @staticmethod
    def sign(message: bytes) -> bytes:
        assert message
        return b"\x5a" * 64


def operator_context() -> OperatorSigningContext:
    """Return one immutable exact-network status-stream signer."""

    return OperatorSigningContext(
        NetworkId.from_bytes(bytes([0xA5]) * 32),
        StubOperatorKeyPair(),
    )


_LIVE_STREAM_HELPERS = (
    "stream_events",
    "stream_verifying_key_events",
    "stream_proof_events",
    "stream_trigger_events",
    "stream_pipeline_transactions",
    "stream_pipeline_blocks",
    "stream_pipeline_witnesses",
    "stream_pipeline_merges",
    "stream_sumeragi_status",
)


def test_live_stream_signatures_expose_no_replay_controls() -> None:
    forbidden = {"last_event_id", "resume", "cursor"}
    for name in (*_LIVE_STREAM_HELPERS, "stream_sorafs_reputation_events"):
        parameters = inspect.signature(getattr(ToriiClient, name)).parameters
        assert forbidden.isdisjoint(parameters), name

    orderbook_parameters = inspect.signature(
        ToriiClient.stream_sorafs_orderbook_events
    ).parameters
    assert forbidden.issubset(orderbook_parameters)

    client = ToriiClient(
        "http://torii.example",
        session=SequencedSession([]),
        max_retries=0,
    )
    with pytest.raises(TypeError, match="unexpected keyword argument 'last_event_id'"):
        client.stream_events(last_event_id="stale")  # type: ignore[call-arg]
    with pytest.raises(TypeError, match="unexpected keyword argument 'resume'"):
        client.stream_pipeline_transactions(resume=True)  # type: ignore[call-arg]


def test_sse_stream_has_a_mandatory_event_bound_and_never_redirects() -> None:
    session = SequencedSession([SseStubResponse(["data: 123456", ""])])
    client = ToriiClient("https://torii.example", session=session, max_retries=0)

    stream = client._stream_sse(
        "/v1/events/sse",
        maximum_event_bytes=8,
        max_retries=0,
        decode_json=False,
    )
    with pytest.raises(ValueError, match="8-byte size bound"):
        next(stream)
    assert session.calls[0]["allow_redirects"] is False

    with pytest.raises(ValueError, match="positive integer"):
        client._stream_sse(
            "/v1/events/sse",
            maximum_event_bytes=0,
            max_retries=0,
        )


def test_sse_stream_bounds_newline_free_chunks_and_normalizes_accept_header() -> None:
    session = SequencedSession([ChunkSseStubResponse([b"x" * 9])])
    client = ToriiClient(
        "https://torii.example",
        session=session,
        default_headers={"accept": "application/json"},
        max_retries=0,
    )

    stream = client._stream_sse(
        "/v1/events/sse",
        maximum_event_bytes=8,
        max_retries=0,
    )
    with pytest.raises(ValueError, match="8-byte size bound"):
        next(stream)

    headers = session.calls[0]["headers"]
    assert sum(name.lower() == "accept" for name in headers) == 1
    assert next(value for name, value in headers.items() if name.lower() == "accept") == (
        "text/event-stream"
    )


def test_sse_mid_body_failures_exhaust_the_retry_budget() -> None:
    session = SequencedSession(
        [
            ChunkSseStubResponse([requests.ConnectionError("first")]),
            ChunkSseStubResponse([requests.ConnectionError("second")]),
            SseStubResponse(["data: should-not-be-read", ""]),
        ]
    )
    client = ToriiClient("https://torii.example", session=session, max_retries=0)
    stream = client._stream_sse(
        "/v1/events/sse",
        max_retries=1,
        backoff_base=0,
    )

    with pytest.raises(requests.ConnectionError, match="second"):
        next(stream)
    assert len(session.calls) == 2


def test_sse_event_bound_counts_crlf_wire_bytes() -> None:
    session = SequencedSession([ChunkSseStubResponse([b":\r\n:\r\n\r\n"])])
    client = ToriiClient("https://torii.example", session=session, max_retries=0)
    stream = client._stream_sse(
        "/v1/events/sse",
        maximum_event_bytes=7,
        max_retries=0,
    )

    with pytest.raises(ValueError, match="7-byte size bound"):
        next(stream)


@pytest.mark.parametrize(
    "chunks",
    [
        [b"data: cr-only\r\r"],
        [b"data: cr-only\r", b"\r"],
        [b"data: crlf\r", b"\n\r", b"\n"],
    ],
)
def test_sse_stream_accepts_all_standard_line_endings(chunks: list[bytes]) -> None:
    session = SequencedSession([ChunkSseStubResponse(chunks)])
    client = ToriiClient("https://torii.example", session=session, max_retries=0)

    event = next(client._stream_sse("/v1/events/sse", max_retries=0))

    assert event.data in {"cr-only", "crlf"}
    assert len(session.calls) == 1


def test_sse_callback_request_errors_are_not_retried_or_checkpointed() -> None:
    session = SequencedSession(
        [
            SseStubResponse(["id: event-1", "data: payload", ""]),
            SseStubResponse(["data: duplicate", ""]),
        ]
    )
    client = ToriiClient("https://torii.example", session=session, max_retries=0)
    cursor = EventCursor()

    def fail_callback(_event: object) -> None:
        raise requests.ConnectionError("application callback failed")

    stream = client._stream_sse(
        "/v1/events/sse",
        allow_resume=True,
        cursor=cursor,
        max_retries=3,
        backoff_base=0,
        on_event=fail_callback,
    )

    with pytest.raises(requests.ConnectionError, match="application callback failed"):
        next(stream)
    assert len(session.calls) == 1
    assert cursor.last_event_id is None


def test_retired_sumeragi_new_view_surface_is_absent() -> None:
    retired_methods = (
        "get_sumeragi_new_view",
        "get_sumeragi_new_view_typed",
        "stream_sumeragi_new_view",
    )
    for name in retired_methods:
        assert not hasattr(ToriiClient, name), name

    retired_models = (
        "SumeragiNewViewReceipt",
        "SumeragiNewViewSnapshot",
    )
    for name in retired_models:
        assert not hasattr(client_module, name), name
        assert name not in client_module.__all__, name
        assert not hasattr(iroha_python, name), name
        assert name not in iroha_python.__all__, name


def test_specialized_live_stream_filters_normally_and_surfaces_terminal_error() -> None:
    event_payload = {"Pipeline": {"Transaction": {"status": "Queued"}}}
    terminal_payload = {
        "code": "stream_lagged",
        "message": "The event stream lost buffered events and cannot replay them.",
        "dropped_messages": 3,
        "replay_available": False,
    }
    session = SequencedSession(
        [
            SseStubResponse(
                [
                    f"data: {json.dumps(event_payload)}",
                    "",
                    "event: stream_error",
                    f"data: {json.dumps(terminal_payload)}",
                    "",
                ]
            )
        ]
    )
    observed: list[tuple[Any, Any]] = []
    client = ToriiClient(
        "http://torii.example",
        session=session,
        default_headers={"lAsT-EvEnT-Id": "must-not-leak"},
        max_retries=0,
    )

    events = client.stream_pipeline_transactions(
        status="Queued",
        max_retries=0,
        on_event=lambda payload, event_id: observed.append((payload, event_id)),
    )
    assert next(events) == event_payload
    with pytest.raises(SseStreamError) as raised:
        next(events)

    error = raised.value
    assert error.code == "stream_lagged"
    assert error.message == terminal_payload["message"]
    assert error.dropped_messages == 3
    assert error.replay_available is False
    assert error.payload == terminal_payload
    assert error.malformed_reason is None
    assert observed == [(event_payload, None)]

    call = session.calls[0]
    assert call["stream"] is True
    assert call["headers"]["Accept"] == "text/event-stream"
    assert all(name.lower() != "last-event-id" for name in call["headers"])
    assert all(not name.lower().startswith("x-iroha-") for name in call["headers"])
    encoded_filter = json.loads(call["params"]["filter"])
    assert encoded_filter["Pipeline"]["Transaction"]["status"] == "Queued"


def test_live_event_stream_optionally_signs_exact_final_uri() -> None:
    event_payload = {"Pipeline": {"Transaction": {"status": "Queued"}}}
    session = SequencedSession(
        [SseStubResponse([f"data: {json.dumps(event_payload)}", ""])]
    )
    captured: list[bytes] = []
    auth = ToriiCanonicalRequestAuth(
        network_id=(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        ),
        account_id=(
            "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP"
        ),
        signer=lambda message: captured.append(message) or b"\x5a" * 64,
        timestamp_ms=4_102_444_801_000,
        nonce="python-event-stream-final-uri",
    )
    client = ToriiClient(
        "https://torii.example",
        session=session,
        canonical_request_auth=auth,
        max_retries=3,
    )

    assert next(client.stream_pipeline_transactions(status="Queued", max_retries=0)) == event_payload

    call = session.calls[0]
    prepared = urlsplit(str(call["url"]))
    exact_target = prepared.path + (f"?{prepared.query}" if prepared.query else "")
    assert prepared.path == "/v1/events/sse"
    assert "filter=" in prepared.query
    assert captured == [
        canonical_network_request_signature_message(
            auth.network_id,
            "GET",
            exact_target,
            b"",
            timestamp_ms=auth.timestamp_ms or 0,
            nonce=auth.nonce or "",
        )
    ]
    assert call["stream"] is True
    assert call["headers"]["Accept"] == "text/event-stream"
    assert "X-Iroha-Account" in call["headers"]
    assert "X-Iroha-Signature" in call["headers"]


def test_sumeragi_status_stream_uses_fresh_one_shot_operator_auth() -> None:
    payload = {"view": 2}
    session = SequencedSession(
        [
            SseStubResponse([f"data: {json.dumps(payload)}", ""]),
            SseStubResponse([f"data: {json.dumps(payload)}", ""]),
        ]
    )
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=operator_context(),
        max_retries=3,
    )

    assert next(client.stream_sumeragi_status()) == payload
    assert next(client.stream_sumeragi_status()) == payload

    assert len(session.calls) == 2
    nonces = []
    for call in session.calls:
        headers = {name.lower(): value for name, value in call["headers"].items()}
        assert call["stream"] is True
        assert call["allow_redirects"] is False
        assert headers["accept"] == "text/event-stream"
        assert headers["x-iroha-operator-public-key"] == StubOperatorKeyPair.public_key_multihash
        assert headers["x-iroha-operator-signature"]
        nonces.append(headers["x-iroha-operator-nonce"])
    assert nonces[0] != nonces[1]


def test_sumeragi_status_stream_rejects_missing_signer_and_retries_before_dispatch() -> None:
    session = SequencedSession([])
    client = ToriiClient("https://torii.example", session=session, max_retries=3)

    with pytest.raises(ValueError, match="operator_signing_context"):
        client.stream_sumeragi_status()
    with pytest.raises(ValueError, match="max_retries must be zero"):
        ToriiClient(
            "https://torii.example",
            session=session,
            operator_signing_context=operator_context(),
        ).stream_sumeragi_status(max_retries=1)
    assert session.calls == []


@pytest.mark.parametrize(
    ("data", "reason"),
    [
        ("not-json", "data must be a JSON object"),
        (
            json.dumps(
                {
                    "code": "stream_lagged",
                    "message": "gap",
                    "dropped_messages": True,
                    "replay_available": False,
                }
            ),
            "dropped_messages must be a non-negative integer or null",
        ),
        (
            json.dumps(
                {
                    "code": "stream_lagged",
                    "message": "gap",
                    "dropped_messages": 1,
                }
            ),
            "replay_available is required",
        ),
    ],
)
def test_malformed_terminal_stream_error_is_typed(data: str, reason: str) -> None:
    session = SequencedSession(
        [SseStubResponse(["event: stream_error", f"data: {data}", ""])]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(SseStreamError) as raised:
        next(client.stream_events(max_retries=0))

    error = raised.value
    assert error.code == SseStreamError.MALFORMED_CODE
    assert error.dropped_messages is None
    assert error.replay_available is None
    assert error.malformed_reason == reason
    assert reason in str(error)


def test_stream_error_is_decoded_even_when_normal_payload_json_decode_is_disabled() -> None:
    payload = {
        "code": "stream_source_closed",
        "message": "The event source closed.",
        "dropped_messages": None,
        "replay_available": False,
    }
    session = SequencedSession(
        [
            SseStubResponse(
                ["event: stream_error", f"data: {json.dumps(payload)}", ""]
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(SseStreamError) as raised:
        next(client.stream_events(max_retries=0, decode_json=False))

    assert raised.value.code == "stream_source_closed"
    assert raised.value.replay_available is False
