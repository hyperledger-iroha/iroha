"""SoraNet privacy-event feed schema tests."""

from __future__ import annotations

import json

import pytest
import requests

from iroha_python.privacy import (
    PrivacyEventGarAbuseCategory,
    PrivacyEventKind,
    fetch_privacy_events,
    load_privacy_events_from_ndjson,
    parse_privacy_event,
    stream_privacy_events,
)


def _gar_event(category_hash: object) -> dict[str, object]:
    return {
        "timestamp_unix": 1_723_456_789,
        "mode": "exit",
        "kind": "GarAbuseCategory",
        "payload": {"category_hash": category_hash},
    }


def test_gar_event_parses_only_the_fixed_hash() -> None:
    event = parse_privacy_event(_gar_event([0xA5] * 8))

    assert event.kind is PrivacyEventKind.GAR_ABUSE_CATEGORY
    assert event.payload == PrivacyEventGarAbuseCategory(category_hash=bytes([0xA5] * 8))


@pytest.mark.parametrize(
    "category_hash",
    (
        [1] * 7,
        [1] * 9,
        [1] * 7 + [-1],
        [1] * 7 + [256],
        [1] * 7 + [True],
        "0101010101010101",
    ),
)
def test_gar_event_rejects_noncanonical_hashes(category_hash: object) -> None:
    with pytest.raises(TypeError, match="category_hash"):
        parse_privacy_event(_gar_event(category_hash))


def test_gar_event_rejects_retired_raw_label_payload() -> None:
    event = _gar_event(None)
    event["payload"] = {"label": "policy.secret"}

    with pytest.raises(TypeError, match="category_hash"):
        parse_privacy_event(event)


class _Response(requests.Response):
    def __init__(self, body: bytes, status: int = 200) -> None:
        super().__init__()
        self.status_code = status
        self._content = body
        self.closed_by_client = False

    def close(self) -> None:
        self.closed_by_client = True
        super().close()


class _Session(requests.Session):
    def __init__(self, response: requests.Response) -> None:
        super().__init__()
        self.response = response
        self.calls: list[dict[str, object]] = []
        self.close_count = 0

    def get(self, url: str, **kwargs: object) -> requests.Response:
        self.calls.append({"url": url, **kwargs})
        self.response.url = url
        return self.response

    def close(self) -> None:
        self.close_count += 1
        super().close()


def _gar_line() -> bytes:
    return json.dumps(_gar_event([0xA5] * 8), separators=(",", ":")).encode("utf-8")


def test_fetch_is_origin_locked_bounded_and_does_not_follow_redirects() -> None:
    response = _Response(_gar_line() + b"\n")
    session = _Session(response)

    events = fetch_privacy_events(
        "https://relay.example",
        session=session,
        maximum_response_bytes=1024,
        maximum_line_bytes=512,
    )

    assert len(events) == 1
    assert response.closed_by_client is True
    assert session.close_count == 0
    assert session.calls == [
        {
            "url": "https://relay.example/privacy/events",
            "timeout": 10.0,
            "headers": {"Accept": "application/x-ndjson"},
            "allow_redirects": False,
            "stream": True,
        }
    ]


def test_fetch_and_stream_reject_oversized_input() -> None:
    fetch_response = _Response(_gar_line())
    with pytest.raises(ValueError, match="10-byte limit"):
        fetch_privacy_events(
            "https://relay.example/privacy/events",
            session=_Session(fetch_response),
            maximum_response_bytes=10,
        )
    assert fetch_response.closed_by_client is True

    stream_response = _Response(_gar_line() + b"\n")
    stream_session = _Session(stream_response)
    stream = stream_privacy_events(
        "https://relay.example",
        session=stream_session,
        maximum_line_bytes=10,
        chunk_size=4,
    )
    assert stream_session.calls == []
    with pytest.raises(ValueError, match="10-byte limit"):
        next(stream)
    assert stream_response.closed_by_client is True

    declared_oversize = _Response(b"")
    declared_oversize.headers["Content-Length"] = "11"
    with pytest.raises(ValueError, match="10-byte limit"):
        fetch_privacy_events(
            "https://relay.example",
            session=_Session(declared_oversize),
            maximum_response_bytes=10,
        )
    assert declared_oversize.closed_by_client is True


def test_privacy_feed_rejects_ambiguous_urls_and_numeric_coercions() -> None:
    with pytest.raises(ValueError, match="credentials"):
        fetch_privacy_events("https://user:secret@relay.example")
    with pytest.raises(ValueError, match="base_url path"):
        fetch_privacy_events("https://relay.example/unrelated")
    with pytest.raises(ValueError, match="origin-relative"):
        stream_privacy_events("https://relay.example", path="//attacker.example/feed")
    with pytest.raises(ValueError, match="control characters"):
        fetch_privacy_events("https://relay.exa\nmple")
    with pytest.raises(ValueError, match="backslashes"):
        stream_privacy_events("https://relay.example", path="/\\attacker")

    for timestamp in (True, "1", 1.5, -1):
        event = _gar_event([0xA5] * 8)
        event["timestamp_unix"] = timestamp
        with pytest.raises((TypeError, ValueError), match="non-negative integer"):
            parse_privacy_event(event)


def test_ndjson_parser_enforces_line_and_event_limits() -> None:
    text = (_gar_line() + b"\n" + _gar_line()).decode("utf-8")
    with pytest.raises(ValueError, match="1-event limit"):
        load_privacy_events_from_ndjson(text, maximum_events=1)
    with pytest.raises(ValueError, match="10-byte limit"):
        load_privacy_events_from_ndjson(text, maximum_line_bytes=10)
