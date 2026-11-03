"""Controlled HTTP negatives for the real-peer smoke assertions and startup wait."""

from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from threading import Thread
from types import SimpleNamespace

import pytest

from . import test_network_smoke as smoke


@contextmanager
def _peer(replies):
    """Serve exact ordered HTTP responses on an ephemeral loopback socket."""
    seen = []

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append((self.path, self.headers.get("Accept")))
            status, content_type, body = replies[self.path][0]
            if len(replies[self.path]) > 1:
                replies[self.path].pop(0)
            body = body.encode()
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Location", "http://127.0.0.1:1/never-follow")
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, kwargs={"poll_interval": 0.01})
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}", seen
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        assert not thread.is_alive()


def _replies():
    schema = dict.fromkeys(smoke.SCHEMA_KINDS, 0)
    schema.update(types_total=1, structs=1, type_names_sample=["Example"])
    status = {
        "blocks": 1,
        "blocks_non_empty": 1,
        "peers": 3,
        "txs_approved": 0,
        "txs_rejected": 0,
        "observed_at_ms": 1,
        "build": {
            "version": "fixture",
            "target_triple": "fixture",
            "wire_schema_hash": "ab" * 32,
        },
    }
    return {
        "/health": [(200, "text/plain", "Healthy")],
        "/v1/api/version": [(200, "text/plain; charset=utf-8", "1")],
        "/status": [(200, "application/json", json.dumps(status))],
        "/v1/schema": [(200, "application/json", json.dumps(schema))],
    }


@pytest.mark.parametrize(
    "check,requests",
    [
        (smoke.test_peer_health, [("/health", "text/plain")]),
        (
            smoke.test_peer_canonical_block_wire_version,
            [("/v1/api/version", "text/plain, application/json")],
        ),
        (
            smoke.test_peer_status,
            [
                ("/v1/api/version", "text/plain, application/json"),
                ("/status", "application/json"),
            ],
        ),
        (smoke.test_peer_schema, [("/v1/schema", "application/json")]),
    ],
)
def test_smoke_uses_the_current_routes(check, requests):
    with _peer(_replies()) as (url, seen):
        check(url)
    assert seen == requests


@pytest.mark.parametrize(
    "check,path,reply",
    [
        (smoke.test_peer_health, "/health", (302, "text/plain", "Healthy")),
        (smoke.test_peer_health, "/health", (200, "text/plain", "not healthy")),
        (smoke.test_peer_health, "/health", (200, "application/json", "Healthy")),
        (
            smoke.test_peer_canonical_block_wire_version,
            "/v1/api/version",
            (200, "text/plain", "2"),
        ),
        (
            smoke.test_peer_canonical_block_wire_version,
            "/v1/api/version",
            (503, "text/plain", "storage unavailable"),
        ),
        (
            smoke.test_peer_canonical_block_wire_version,
            "/v1/api/version",
            (503, "application/json", '{"code":"another_failure"}'),
        ),
        (
            smoke.test_peer_canonical_block_wire_version,
            "/v1/api/version",
            (200, "application/json", "1"),
        ),
        (smoke.test_peer_status, "/status", (200, "application/json", "{}")),
        (smoke.test_peer_schema, "/v1/schema", (404, "application/json", "{}")),
    ],
)
def test_smoke_refuses_wrong_status_body_or_media_type(check, path, reply):
    replies = _replies()
    replies[path] = [reply]
    with _peer(replies) as (url, _seen):
        with pytest.raises((AssertionError, KeyError)):
            check(url)


@pytest.mark.parametrize(
    "path,field,value,check",
    [
        ("/status", "blocks", 0, smoke.test_peer_status),
        ("/status", "peers", 4, smoke.test_peer_status),
        ("/status", "blocks", True, smoke.test_peer_status),
        ("/v1/schema", "types_total", 2, smoke.test_peer_schema),
        ("/v1/schema", "type_names_sample", [], smoke.test_peer_schema),
    ],
)
def test_smoke_refuses_invalid_committed_or_schema_values(path, field, value, check):
    replies = _replies()
    payload = json.loads(replies[path][0][2])
    payload[field] = value
    replies[path] = [(200, "application/json", json.dumps(payload))]
    with _peer(replies) as (url, _seen):
        with pytest.raises(AssertionError):
            check(url)


def _unavailable_version():
    """The existing full Rust router test specifies this canonical startup reply."""
    return (
        503,
        "application/json; charset=utf-8",
        json.dumps(
            {
                "code": "service_unavailable",
                "message": "The service is temporarily unavailable.",
            }
        ),
    )


def test_only_canonical_version_unavailable_is_retried():
    replies = _replies()
    replies["/v1/api/version"].insert(0, _unavailable_version())
    with _peer(replies) as (url, seen):
        smoke.test_peer_canonical_block_wire_version(url)
    assert [path for path, _accept in seen] == ["/v1/api/version"] * 2


def test_startup_wait_does_not_reset_its_deadline(monkeypatch):
    replies = {"/v1/api/version": [_unavailable_version()]}
    with _peer(replies) as (url, seen):
        ticks = iter((0, 0, 31, 31))
        with monkeypatch.context() as clock:
            clock.setattr(
                smoke,
                "time",
                SimpleNamespace(
                    monotonic=lambda: next(ticks), sleep=lambda _delay: None
                ),
            )
            with pytest.raises(
                AssertionError, match="version was not available within 30s"
            ):
                smoke.test_peer_canonical_block_wire_version(url)
    assert len(seen) == 1


def test_all_four_generated_peers_are_selected():
    assert smoke.TORII_URLS == tuple(
        f"http://127.0.0.1:{port}" for port in (8080, 8081, 8082, 8083)
    )


def test_missing_peer_is_a_hard_network_failure():
    server = ThreadingHTTPServer(("127.0.0.1", 0), BaseHTTPRequestHandler)
    url = f"http://127.0.0.1:{server.server_port}"
    server.server_close()
    with pytest.raises(smoke.requests.exceptions.ConnectionError):
        smoke.test_peer_health(url)
