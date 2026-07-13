from __future__ import annotations

import inspect
import json
from typing import Any

import iroha_python
import iroha_python.client as client_module
import pytest
import requests
from iroha_python import SseStreamError, ToriiClient
from requests.structures import CaseInsensitiveDict

from .helpers import StubResponse


class SequencedSession(requests.Session):
    """Capture streaming requests and return queued SSE responses."""

    def __init__(self, responses: list[requests.Response]) -> None:
        super().__init__()
        self._responses = list(responses)
        self.calls: list[dict[str, Any]] = []

    def get(self, url: str | bytes, **kwargs: Any) -> requests.Response:
        self.calls.append(
            {
                "url": url,
                "params": kwargs.get("params"),
                "headers": kwargs.get("headers") or {},
                "stream": kwargs.get("stream"),
            }
        )
        if not self._responses:
            raise AssertionError("unexpected SSE request")
        return self._responses.pop(0)


class SseStubResponse(StubResponse):
    """Minimal successful SSE response."""

    def __init__(self, lines: list[str]) -> None:
        super().__init__(200, None)
        self.headers = CaseInsensitiveDict({"Content-Type": "text/event-stream"})
        self._lines = lines

    def iter_lines(self, decode_unicode: bool = False, **kwargs: Any):
        del kwargs
        for line in self._lines:
            yield line if decode_unicode else line.encode("utf-8")


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
    for name in _LIVE_STREAM_HELPERS:
        parameters = inspect.signature(getattr(ToriiClient, name)).parameters
        assert forbidden.isdisjoint(parameters), name

    for name in (
        "stream_sorafs_orderbook_events",
        "stream_sorafs_reputation_events",
    ):
        parameters = inspect.signature(getattr(ToriiClient, name)).parameters
        assert forbidden.issubset(parameters), name

    client = ToriiClient(
        "http://torii.example",
        session=SequencedSession([]),
        max_retries=0,
    )
    with pytest.raises(TypeError, match="unexpected keyword argument 'last_event_id'"):
        client.stream_events(last_event_id="stale")  # type: ignore[call-arg]
    with pytest.raises(TypeError, match="unexpected keyword argument 'resume'"):
        client.stream_pipeline_transactions(resume=True)  # type: ignore[call-arg]


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
    encoded_filter = json.loads(call["params"]["filter"])
    assert encoded_filter["Pipeline"]["Transaction"]["status"] == "Queued"


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
