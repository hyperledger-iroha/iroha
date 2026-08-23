"""Tests for the governed Android attestation status capture."""

from __future__ import annotations

import datetime as dt
import email.utils
import importlib.util
import json
import os
from pathlib import Path
import stat

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "capture_android_attestation_status.py"
SPEC = importlib.util.spec_from_file_location("capture_android_attestation_status", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
status_capture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(status_capture)


RESPONSE_DATE_MS = 1_800_000_000_000


def _http_date(milliseconds: int) -> str:
    value = dt.datetime.fromtimestamp(milliseconds / 1_000, tz=dt.timezone.utc)
    return email.utils.format_datetime(value, usegmt=True)


def _headers(**overrides: str) -> list[tuple[str, str]]:
    values = {
        "Date": _http_date(RESPONSE_DATE_MS),
        "Age": "30",
        "Cache-Control": "public, max-age=3600",
        "Expires": _http_date(RESPONSE_DATE_MS + 3_600_000),
        "Last-Modified": _http_date(RESPONSE_DATE_MS - 1_000),
        "Content-Encoding": "identity",
    }
    values.update(overrides)
    return list(values.items())


def _payload() -> bytes:
    return json.dumps(
        {
            "entries": {
                "a0": {"status": "SUSPENDED", "reason": "pending"},
                "1": {"status": "REVOKED", "reason": "compromised"},
            }
        },
        separators=(",", ":"),
    ).encode()


def test_capture_derives_exact_consensus_snapshot_and_header_receipt() -> None:
    payload = _payload()
    snapshot, receipt = status_capture.build_capture(
        payload,
        _headers(),
        captured_at_ms=RESPONSE_DATE_MS + 30_000,
    )

    assert snapshot["non_valid_serials"] == ["1", "a0"]
    assert snapshot["payload_sha256"] == list(__import__("hashlib").sha256(payload).digest())
    assert snapshot["last_modified_ms"] == RESPONSE_DATE_MS - 1_000
    assert receipt["source_url"] == status_capture.STATUS_URL
    assert receipt["snapshot"] == snapshot


@pytest.mark.parametrize(
    "headers,captured_at_ms",
    [
        (_headers(Age="3600"), RESPONSE_DATE_MS + 3_600_000),
        (_headers(Expires=_http_date(RESPONSE_DATE_MS + 3_599_000)), RESPONSE_DATE_MS + 30_000),
        (_headers(**{"Last-Modified": _http_date(RESPONSE_DATE_MS + 1_000)}), RESPONSE_DATE_MS + 30_000),
        (_headers(Age="030"), RESPONSE_DATE_MS + 30_000),
    ],
)
def test_capture_rejects_stale_or_inconsistent_http_metadata(
    headers: list[tuple[str, str]], captured_at_ms: int
) -> None:
    with pytest.raises(status_capture.CaptureError):
        status_capture.build_capture(_payload(), headers, captured_at_ms=captured_at_ms)


def test_capture_rejects_duplicate_headers_and_json_keys() -> None:
    with pytest.raises(status_capture.CaptureError):
        status_capture.build_capture(
            _payload(),
            [*_headers(), ("date", _http_date(RESPONSE_DATE_MS))],
            captured_at_ms=RESPONSE_DATE_MS + 30_000,
        )
    with pytest.raises(status_capture.CaptureError):
        status_capture.build_capture(
            b'{"entries":{},"entries":{}}',
            _headers(),
            captured_at_ms=RESPONSE_DATE_MS + 30_000,
        )


def test_fetch_status_uses_fixed_identity_encoded_https_request(monkeypatch) -> None:
    payload = _payload()
    response_headers = [("Content-Length", str(len(payload)))]
    context = object()
    calls: dict[str, object] = {}

    class FakeResponse:
        status = 200

        def read(self, limit: int) -> bytes:
            calls["read_limit"] = limit
            return payload

        def getheaders(self) -> list[tuple[str, str]]:
            return response_headers

    class FakeConnection:
        def __init__(self, host: str, *, port: int, timeout: int, context: object):
            calls["connection"] = (host, port, timeout, context)

        def request(self, method: str, path: str, *, headers: dict[str, str]) -> None:
            calls["request"] = (method, path, headers)

        def getresponse(self) -> FakeResponse:
            return FakeResponse()

        def close(self) -> None:
            calls["closed"] = True

    monkeypatch.setattr(status_capture.ssl, "create_default_context", lambda: context)
    monkeypatch.setattr(status_capture.http.client, "HTTPSConnection", FakeConnection)
    monkeypatch.setattr(status_capture.time, "time_ns", lambda: 1_800_000_030_000_000_000)

    actual_payload, actual_headers, captured_at_ms = status_capture.fetch_status()

    assert actual_payload == payload
    assert actual_headers == response_headers
    assert captured_at_ms == RESPONSE_DATE_MS + 30_000
    assert calls["connection"] == (
        status_capture.STATUS_HOST,
        443,
        30,
        context,
    )
    method, path, headers = calls["request"]
    assert (method, path) == ("GET", status_capture.STATUS_PATH)
    assert headers["Accept-Encoding"] == "identity"
    assert headers["Connection"] == "close"
    assert calls["read_limit"] == status_capture.MAX_PAYLOAD_BYTES + 1
    assert calls["closed"] is True


def test_private_writer_is_canonical_create_new(tmp_path: Path) -> None:
    path = tmp_path / "capture.json"
    encoded = status_capture._canonical_json({"z": 1, "a": 2})
    assert encoded == b'{"a":2,"z":1}\n'

    status_capture._write_new_private(path, encoded)

    assert path.read_bytes() == encoded
    assert stat.S_IMODE(path.stat().st_mode) == 0o600
    with pytest.raises(FileExistsError):
        status_capture._write_new_private(path, b"replacement\n")
    assert path.read_bytes() == encoded


def test_publish_capture_creates_owner_only_complete_directory(tmp_path: Path) -> None:
    os.chmod(tmp_path, 0o700)
    target = tmp_path / "capture"
    payload = _payload()
    snapshot = {"version": 1, "payload_sha256": [0] * 32}
    receipt = {"version": 1, "snapshot": snapshot}

    status_capture.publish_capture(target, payload, snapshot, receipt)

    assert stat.S_IMODE(target.stat().st_mode) == 0o700
    assert (target / "status.json").read_bytes() == payload
    assert (target / "snapshot.json").read_bytes() == status_capture._canonical_json(snapshot)
    assert (target / "capture-receipt.json").read_bytes() == status_capture._canonical_json(
        receipt
    )
    for path in target.iterdir():
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
    with pytest.raises(status_capture.CaptureError):
        status_capture.publish_capture(target, payload, snapshot, receipt)


def test_publish_capture_fsyncs_new_directory_entry_and_propagates_failure(
    monkeypatch, tmp_path: Path
) -> None:
    os.chmod(tmp_path, 0o700)
    target = tmp_path / "capture"
    calls: list[Path] = []

    def fsync_directory(path: Path) -> None:
        calls.append(path)
        if path == tmp_path:
            raise OSError("parent fsync failed")

    monkeypatch.setattr(status_capture, "_fsync_directory", fsync_directory)
    with pytest.raises(status_capture.CaptureError, match="capture publication failed"):
        status_capture.publish_capture(
            target,
            _payload(),
            {"version": 1, "payload_sha256": [0] * 32},
            {"version": 1},
        )
    assert calls == [target, tmp_path]


def test_main_publishes_only_validated_capture(monkeypatch, tmp_path: Path, capsys) -> None:
    payload = _payload()
    headers = _headers()
    snapshot = {"version": 1}
    receipt = {"status_payload_sha256": "ab" * 32}
    published: list[tuple[Path, bytes, dict, dict]] = []
    monkeypatch.setattr(
        status_capture,
        "fetch_status",
        lambda: (payload, headers, RESPONSE_DATE_MS + 30_000),
    )
    monkeypatch.setattr(
        status_capture,
        "build_capture",
        lambda actual_payload, actual_headers, *, captured_at_ms: (
            snapshot,
            receipt,
        )
        if (actual_payload, actual_headers, captured_at_ms)
        == (payload, headers, RESPONSE_DATE_MS + 30_000)
        else pytest.fail("main changed the captured response"),
    )
    monkeypatch.setattr(
        status_capture,
        "publish_capture",
        lambda output, actual_payload, actual_snapshot, actual_receipt: published.append(
            (output, actual_payload, actual_snapshot, actual_receipt)
        ),
    )
    output = tmp_path / "new-capture"

    assert status_capture.main(["--output-directory", str(output)]) == 0

    assert published == [(output, payload, snapshot, receipt)]
    stdout = capsys.readouterr().out
    assert str(output) in stdout
    assert receipt["status_payload_sha256"] in stdout


@pytest.mark.parametrize(
    "entries",
    [
        {"01": {"status": "REVOKED"}},
        {"A0": {"status": "REVOKED"}},
        {"a0": {"status": "GOOD"}},
    ],
)
def test_capture_rejects_noncanonical_or_non_deny_entries(entries: dict) -> None:
    payload = json.dumps({"entries": entries}, separators=(",", ":")).encode()
    with pytest.raises(status_capture.CaptureError):
        status_capture.build_capture(
            payload,
            _headers(),
            captured_at_ms=RESPONSE_DATE_MS + 30_000,
        )
