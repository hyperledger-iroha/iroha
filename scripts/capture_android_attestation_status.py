#!/usr/bin/env python3
"""Capture Google's Android Key Attestation status as governed offline evidence."""

from __future__ import annotations

import argparse
import datetime as dt
import email.utils
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import ssl
import stat
import sys
import time
from typing import Any, Iterable, Optional


STATUS_HOST = "android.googleapis.com"
STATUS_PATH = "/attestation/status"
STATUS_URL = f"https://{STATUS_HOST}{STATUS_PATH}"
CAPTURE_SCHEMA = "iroha.kagemusha.android_attestation_status_capture.v1"
SNAPSHOT_VERSION = 1
MAX_PAYLOAD_BYTES = 256 * 1024
MAX_NON_VALID_SERIALS = 4_096
MAX_SERIAL_HEX_BYTES = 40
MAX_CACHE_AGE_SECONDS = 86_400
HTTP_CLOCK_TOLERANCE_MS = 5 * 60 * 1_000
SERIAL_RE = re.compile(r"(?:0|[1-9a-f][0-9a-f]*)\Z")
NON_VALID_STATUSES = frozenset(("REVOKED", "SUSPENDED"))


class CaptureError(RuntimeError):
    """The status response cannot become governed evidence."""


def _strict_json_object(payload: bytes) -> dict[str, Any]:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise CaptureError(f"Android status payload repeats JSON key {key!r}")
            result[key] = value
        return result

    try:
        value = json.loads(
            payload.decode("utf-8"),
            object_pairs_hook=reject_duplicates,
            parse_constant=lambda token: (_ for _ in ()).throw(
                CaptureError(f"Android status payload contains non-finite {token}")
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CaptureError("Android status payload is not strict UTF-8 JSON") from error
    if not isinstance(value, dict):
        raise CaptureError("Android status payload must be a JSON object")
    return value


def _one_header(
    headers: Iterable[tuple[str, str]], name: str, *, required: bool = True
) -> Optional[str]:
    values = [value for key, value in headers if key.casefold() == name.casefold()]
    if len(values) > 1 or (required and not values):
        requirement = "exactly once" if required else "at most once"
        raise CaptureError(f"Android status response must contain {name} {requirement}")
    return values[0] if values else None


def _http_date_ms(value: str, label: str) -> int:
    try:
        parsed = email.utils.parsedate_to_datetime(value)
    except (TypeError, ValueError) as error:
        raise CaptureError(f"Android status {label} is not a valid HTTP date") from error
    if parsed is None or parsed.tzinfo is None:
        raise CaptureError(f"Android status {label} must include a timezone")
    parsed = parsed.astimezone(dt.timezone.utc)
    milliseconds = int(parsed.timestamp()) * 1_000
    if milliseconds <= 0:
        raise CaptureError(f"Android status {label} must be after the Unix epoch")
    return milliseconds


def _cache_max_age_seconds(value: str) -> int:
    directives = [directive.strip() for directive in value.split(",")]
    if any(not directive for directive in directives):
        raise CaptureError("Android status Cache-Control contains an empty directive")
    lowered = [directive.casefold() for directive in directives]
    if "public" not in lowered or any(
        directive in {"private", "no-cache", "no-store"} for directive in lowered
    ):
        raise CaptureError("Android status Cache-Control is not a public cache contract")
    max_age_values = [
        directive.split("=", 1)[1]
        for directive in lowered
        if directive.startswith("max-age=")
    ]
    if len(max_age_values) != 1 or re.fullmatch(r"(?:0|[1-9][0-9]*)", max_age_values[0]) is None:
        raise CaptureError("Android status Cache-Control must contain one canonical max-age")
    max_age = int(max_age_values[0])
    if not 1 <= max_age <= MAX_CACHE_AGE_SECONDS:
        raise CaptureError("Android status Cache-Control max-age is outside protocol bounds")
    return max_age


def _canonical_non_valid_serials(payload: bytes) -> list[str]:
    status = _strict_json_object(payload)
    entries = status.get("entries")
    if set(status) != {"entries"} or not isinstance(entries, dict):
        raise CaptureError("Android status payload must contain exactly an entries object")
    if len(entries) > MAX_NON_VALID_SERIALS:
        raise CaptureError("Android status payload exceeds the governed serial-count bound")
    serials: list[str] = []
    for serial, record in entries.items():
        if (
            not isinstance(serial, str)
            or len(serial) > MAX_SERIAL_HEX_BYTES
            or SERIAL_RE.fullmatch(serial) is None
            or not isinstance(record, dict)
            or record.get("status") not in NON_VALID_STATUSES
        ):
            raise CaptureError("Android status payload contains a malformed non-valid entry")
        serials.append(serial)
    return sorted(serials)


def build_capture(
    payload: bytes,
    headers: Iterable[tuple[str, str]],
    *,
    captured_at_ms: int,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Validate exact response bytes/headers and derive the consensus snapshot."""

    if not 1 <= len(payload) <= MAX_PAYLOAD_BYTES:
        raise CaptureError("Android status payload size is outside protocol bounds")
    header_list = list(headers)
    date_value = _one_header(header_list, "Date")
    age_value = _one_header(header_list, "Age")
    cache_control_value = _one_header(header_list, "Cache-Control")
    expires_value = _one_header(header_list, "Expires")
    last_modified_value = _one_header(header_list, "Last-Modified", required=False)
    content_encoding = _one_header(header_list, "Content-Encoding", required=False)
    if content_encoding is not None and content_encoding.casefold() != "identity":
        raise CaptureError("Android status response must not transform the exact payload bytes")
    if age_value is None or re.fullmatch(r"(?:0|[1-9][0-9]*)", age_value) is None:
        raise CaptureError("Android status Age must be a canonical non-negative integer")
    assert date_value is not None
    assert cache_control_value is not None
    assert expires_value is not None
    response_date_ms = _http_date_ms(date_value, "Date")
    cache_max_age_seconds = _cache_max_age_seconds(cache_control_value)
    fresh_until_ms = response_date_ms + cache_max_age_seconds * 1_000
    expires_ms = _http_date_ms(expires_value, "Expires")
    if expires_ms != fresh_until_ms:
        raise CaptureError("Android status Expires does not match Date plus max-age")
    last_modified_ms = (
        _http_date_ms(last_modified_value, "Last-Modified")
        if last_modified_value is not None
        else None
    )
    if last_modified_ms is not None and last_modified_ms > response_date_ms:
        raise CaptureError("Android status Last-Modified is later than Date")
    age_seconds = int(age_value)
    if age_seconds >= cache_max_age_seconds:
        raise CaptureError("Android status response is already stale according to Age")
    expected_capture_ms = response_date_ms + age_seconds * 1_000
    if abs(captured_at_ms - expected_capture_ms) > HTTP_CLOCK_TOLERANCE_MS:
        raise CaptureError("Android status Date/Age metadata disagrees with the capture clock")
    if captured_at_ms < response_date_ms or captured_at_ms >= fresh_until_ms:
        raise CaptureError("Android status response is not fresh at capture time")

    serials = _canonical_non_valid_serials(payload)
    payload_sha256 = hashlib.sha256(payload).digest()
    snapshot: dict[str, Any] = {
        "version": SNAPSHOT_VERSION,
        "payload_sha256": list(payload_sha256),
        "response_date_ms": response_date_ms,
        "last_modified_ms": last_modified_ms,
        "cache_max_age_seconds": cache_max_age_seconds,
        "non_valid_serials": serials,
    }
    receipt: dict[str, Any] = {
        "schema": CAPTURE_SCHEMA,
        "version": 1,
        "source_url": STATUS_URL,
        "captured_at_ms": captured_at_ms,
        "fresh_until_ms": fresh_until_ms,
        "status_payload_sha256": payload_sha256.hex(),
        "status_payload_size_bytes": len(payload),
        "response_headers": {
            "date": date_value,
            "age": age_value,
            "cache_control": cache_control_value,
            "expires": expires_value,
            "last_modified": last_modified_value,
        },
        "snapshot": snapshot,
    }
    return snapshot, receipt


def fetch_status() -> tuple[bytes, list[tuple[str, str]], int]:
    """Fetch only the fixed HTTPS endpoint without redirects or content decoding."""

    connection = http.client.HTTPSConnection(
        STATUS_HOST,
        port=443,
        timeout=30,
        context=ssl.create_default_context(),
    )
    try:
        connection.request(
            "GET",
            STATUS_PATH,
            headers={
                "Accept": "application/json",
                "Accept-Encoding": "identity",
                "Connection": "close",
                "User-Agent": "iroha-kagemusha-status-capture/1",
            },
        )
        response = connection.getresponse()
        captured_at_ms = time.time_ns() // 1_000_000
        if response.status != 200:
            raise CaptureError(
                f"Android status endpoint returned HTTP {response.status}; redirects are forbidden"
            )
        payload = response.read(MAX_PAYLOAD_BYTES + 1)
        if len(payload) > MAX_PAYLOAD_BYTES:
            raise CaptureError("Android status endpoint exceeded the payload bound")
        headers = response.getheaders()
        content_length = _one_header(headers, "Content-Length", required=False)
        if content_length is not None:
            if re.fullmatch(r"(?:0|[1-9][0-9]*)", content_length) is None:
                raise CaptureError("Android status Content-Length is not canonical")
            if int(content_length) != len(payload):
                raise CaptureError("Android status Content-Length does not match exact bytes")
        return payload, headers, captured_at_ms
    except (OSError, http.client.HTTPException) as error:
        raise CaptureError("Android status HTTPS fetch failed") from error
    finally:
        connection.close()


def _canonical_json(value: object) -> bytes:
    return (
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def _write_new_private(path: Path, payload: bytes) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise OSError("short status-capture write")
            offset += written
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(
        path,
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def publish_capture(
    output_directory: Path,
    payload: bytes,
    snapshot: dict[str, Any],
    receipt: dict[str, Any],
) -> None:
    """Publish one owner-only, create-new capture directory."""

    if not output_directory.is_absolute() or output_directory.name in {"", ".", ".."}:
        raise CaptureError("capture output directory must be an absolute new path")
    try:
        parent = output_directory.parent.resolve(strict=True)
        metadata = parent.stat()
    except OSError as error:
        raise CaptureError("capture output parent could not be resolved") from error
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid not in {0, os.geteuid()}
        or stat.S_IMODE(metadata.st_mode) & 0o022
    ):
        raise CaptureError("capture output parent must be owner-controlled and non-writable")
    target = parent / output_directory.name
    try:
        target.mkdir(mode=0o700)
    except OSError as error:
        raise CaptureError("capture output directory must not already exist") from error
    paths = [
        (target / "status.json", payload),
        (target / "snapshot.json", _canonical_json(snapshot)),
        (target / "capture-receipt.json", _canonical_json(receipt)),
    ]
    try:
        for path, contents in paths:
            _write_new_private(path, contents)
        _fsync_directory(target)
        _fsync_directory(parent)
    except (OSError, CaptureError) as error:
        raise CaptureError(
            "capture publication failed; inspect the create-new output directory"
        ) from error


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-directory", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        payload, headers, captured_at_ms = fetch_status()
        snapshot, receipt = build_capture(
            payload,
            headers,
            captured_at_ms=captured_at_ms,
        )
        publish_capture(args.output_directory, payload, snapshot, receipt)
    except CaptureError as error:
        print(f"[android-attestation-status] ERROR: {error}", file=sys.stderr)
        return 1
    print(f"[android-attestation-status] capture: {args.output_directory}")
    print(
        "[android-attestation-status] payload sha256: "
        f"{receipt['status_payload_sha256']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
