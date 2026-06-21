from __future__ import annotations

import json
from typing import Any

import pytest
import requests
from requests.structures import CaseInsensitiveDict

from iroha_python import ToriiClient

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


def test_sorafs_reputation_rest_helpers_build_paths_headers_and_params() -> None:
    snapshot_id_hex = "ab" * 16
    merkle_root_hex = "cd" * 32
    provider_id = "provider:alpha"
    session = SequencedSession(
        [
            StubResponse(
                200,
                {
                    "snapshot_id_hex": snapshot_id_hex,
                    "generated_at_unix": 1_800_000_000,
                    "previous_snapshot_id_hex": None,
                    "merkle_root_hex": merkle_root_hex,
                    "provider_count": 1,
                    "alpha_bps": 2500,
                    "current_score_weight_bps": 7500,
                    "weights": {"por_success_bps": 5000},
                    "providers": [],
                },
            ),
            StubResponse(
                200,
                {
                    "snapshot_id_hex": snapshot_id_hex,
                    "generated_at_unix": 1_800_000_000,
                    "merkle_root_hex": merkle_root_hex,
                    "provider": {
                        "provider_id": provider_id,
                        "score_bps": 9800,
                        "degradation_flags": [],
                        "raw_metrics": {"version": 1},
                        "raw_metrics_hash_hex": "ef" * 32,
                    },
                    "proof": {"provider_id": provider_id, "leaf_index": 0, "siblings_hex": []},
                },
            ),
            StubResponse(304, None),
            StubResponse(
                200,
                {
                    "snapshot_id_hex": snapshot_id_hex,
                    "generated_at_unix": 1_800_000_000,
                    "alpha_bps": 2500,
                    "current_score_weight_bps": 7500,
                    "weights": {"pdp_success_bps": 4000},
                },
            ),
            StubResponse(
                200,
                {
                    "since": 0,
                    "limit": 2,
                    "count": 1,
                    "next_since": 8,
                    "events": [
                        {
                            "version": 1,
                            "sequence": 8,
                            "snapshot_id_hex": snapshot_id_hex,
                            "generated_at_unix": 1_800_000_000,
                            "merkle_root_hex": merkle_root_hex,
                            "provider_count": 1,
                            "previous_snapshot_id_hex": None,
                        }
                    ],
                },
            ),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    latest = client.get_sorafs_reputation_latest(if_none_match='"old"')
    assert latest["snapshot_id_hex"] == snapshot_id_hex
    assert str(session.calls[0]["url"]).endswith("/v1/sorafs/reputation/latest")
    assert session.calls[0]["headers"]["If-None-Match"] == '"old"'

    provider = client.get_sorafs_reputation_provider(provider_id)
    assert provider["provider"]["provider_id"] == provider_id
    assert str(session.calls[1]["url"]).endswith(
        "/v1/sorafs/reputation/providers/provider%3Aalpha"
    )

    snapshot = client.get_sorafs_reputation_snapshot(
        f"0x{snapshot_id_hex.upper()}",
        etag='"snapshot-etag"',
    )
    assert snapshot is None
    assert str(session.calls[2]["url"]).endswith(
        f"/v1/sorafs/reputation/snapshots/{snapshot_id_hex}"
    )
    assert session.calls[2]["headers"]["If-None-Match"] == '"snapshot-etag"'

    weights = client.get_sorafs_reputation_weights()
    assert weights["current_score_weight_bps"] == 7500

    events = client.list_sorafs_reputation_events(
        since=0,
        limit="2",
        if_none_match='"events-etag"',
    )
    assert events["count"] == 1
    assert session.calls[4]["params"] == {"since": 0, "limit": 2}
    assert session.calls[4]["headers"]["If-None-Match"] == '"events-etag"'


def test_sorafs_reputation_stream_helper_parses_sse() -> None:
    snapshot_id_hex = "ab" * 16
    session = SequencedSession(
        [
            SseStubResponse(
                [
                    "id: 8",
                    "event: reputation_snapshot",
                    f'data: {json.dumps({"sequence": 8, "snapshot_id_hex": snapshot_id_hex})}',
                    "",
                ]
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    iterator = client.stream_sorafs_reputation_events(
        since=7,
        limit=1,
        last_event_id="7",
        max_retries=0,
        with_metadata=True,
    )
    event = next(iterator)

    assert event.event == "reputation_snapshot"
    assert event.id == "8"
    assert event.data == {"sequence": 8, "snapshot_id_hex": snapshot_id_hex}
    assert session.calls[0]["params"] == {"since": 7, "limit": 1}
    assert session.calls[0]["headers"]["Last-Event-ID"] == "7"


def test_sorafs_reputation_helpers_validate_inputs_before_request() -> None:
    client = ToriiClient("http://torii.example", session=SequencedSession([]), max_retries=0)

    with pytest.raises(ValueError, match="only one of if_none_match or etag"):
        client.get_sorafs_reputation_latest(if_none_match='"a"', etag='"b"')
    with pytest.raises(ValueError, match="unsupported characters"):
        client.get_sorafs_reputation_provider("bad provider")
    with pytest.raises(ValueError, match="16-byte hex string"):
        client.get_sorafs_reputation_snapshot("ab" * 15)
    with pytest.raises(ValueError, match="positive"):
        client.list_sorafs_reputation_events(limit=0)
    with pytest.raises(ValueError, match="positive"):
        client.stream_sorafs_reputation_events(limit=0)
