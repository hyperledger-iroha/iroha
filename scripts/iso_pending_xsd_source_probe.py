#!/usr/bin/env python3
"""Probe official ISO 20022 pending XSD download endpoints.

This utility records bounded reachability evidence for the official ISO
download URLs tracked by ``iso_xsd_fixture_verify``. It intentionally reads only
small byte ranges and does not import schema bytes into the repository.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import socket
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import iso_xsd_fixture_verify as xsd


PROBE_SUMMARY_VERSION = 1
DEFAULT_TIMEOUT_SECS = 25.0
DEFAULT_MAX_BYTES = 4096
MAX_PROBE_BYTES = 65536
SUMMARY_DIGEST_FIELD = "summary_sha256"


class ProbeError(RuntimeError):
    """Raised when probe inputs are invalid."""


UrlOpen = Callable[..., Any]


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    import hashlib

    return hashlib.sha256(data).hexdigest()


def _positive_float(value: Any, label: str) -> float:
    if isinstance(value, bool):
        raise ProbeError(f"{label} must be a positive finite number")
    try:
        parsed = float(value)
    except (TypeError, ValueError) as error:
        raise ProbeError(f"{label} must be a positive finite number") from error
    if not (parsed > 0.0 and parsed < float("inf")):
        raise ProbeError(f"{label} must be a positive finite number")
    return parsed


def _positive_int(value: Any, label: str) -> int:
    if isinstance(value, bool):
        raise ProbeError(f"{label} must be a positive integer")
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise ProbeError(f"{label} must be a positive integer") from error
    if parsed <= 0:
        raise ProbeError(f"{label} must be a positive integer")
    if parsed > MAX_PROBE_BYTES:
        raise ProbeError(f"{label} must be no larger than {MAX_PROBE_BYTES}")
    return parsed


def _selected_message_ids(values: list[str] | None) -> list[str]:
    known = xsd.KNOWN_PENDING_SCHEMA_SOURCE_METADATA
    if not values:
        return sorted(known)
    result: list[str] = []
    seen: dict[str, int] = {}
    for offset, value in enumerate(values):
        if value not in known:
            raise ProbeError(f"--message-def-id[{offset}] is not a known pending schema")
        if value in seen:
            raise ProbeError(
                f"--message-def-id[{offset}] duplicates --message-def-id[{seen[value]}]"
            )
        seen[value] = offset
        result.append(value)
    return sorted(result)


def _looks_like_xsd(data: bytes) -> bool:
    sample = data[: min(len(data), 4096)].lstrip()
    return (
        sample.startswith(b"<?xml")
        or b"<xs:schema" in sample
        or b"<xsd:schema" in sample
    )


def _content_type(headers: Any) -> str | None:
    if headers is None:
        return None
    value = None
    get_content_type = getattr(headers, "get_content_type", None)
    if callable(get_content_type):
        value = get_content_type()
    if value is None:
        get = getattr(headers, "get", None)
        if callable(get):
            value = get("content-type") or get("Content-Type")
    return value if isinstance(value, str) and value else None


def _response_status(response: Any) -> int | None:
    status = getattr(response, "status", None)
    if isinstance(status, int):
        return status
    getcode = getattr(response, "getcode", None)
    if callable(getcode):
        code = getcode()
        if isinstance(code, int):
            return code
    return None


def _probe_download(
    *,
    message_def_id: str,
    metadata: dict[str, str],
    timeout_secs: float,
    max_bytes: int,
    opener: UrlOpen,
) -> dict[str, Any]:
    url = metadata["download_url"]
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/xml,text/xml,*/*;q=0.1",
            "Range": f"bytes=0-{max_bytes - 1}",
            "Referer": metadata["catalogue_url"],
            "User-Agent": "Mozilla/5.0 ISO20022ReadinessProbe/1.0",
        },
    )
    base = {
        "message_def_id": message_def_id,
        "message_name": metadata["message_name"],
        "submitting_organisation": metadata["submitting_organisation"],
        "catalogue_url": metadata["catalogue_url"],
        "download_url": metadata["download_url"],
    }
    try:
        with opener(request, timeout=timeout_secs) as response:
            status = _response_status(response)
            data = response.read(max_bytes + 1)
            downloaded = min(len(data), max_bytes)
            return {
                **base,
                "status": "reachable" if status is not None and 200 <= status <= 399 and downloaded else "unexpected",
                "http_status": status,
                "content_type": _content_type(getattr(response, "headers", None)),
                "downloaded_bytes": downloaded,
                "truncated": len(data) > max_bytes,
                "looks_like_xsd": _looks_like_xsd(data[:downloaded]),
                "error_kind": None,
            }
    except urllib.error.HTTPError as error:
        return {
            **base,
            "status": "http_error",
            "http_status": error.code,
            "content_type": _content_type(getattr(error, "headers", None)),
            "downloaded_bytes": 0,
            "truncated": False,
            "looks_like_xsd": False,
            "error_kind": "HTTPError",
        }
    except TimeoutError:
        return {
            **base,
            "status": "timeout",
            "http_status": None,
            "content_type": None,
            "downloaded_bytes": 0,
            "truncated": False,
            "looks_like_xsd": False,
            "error_kind": "TimeoutError",
        }
    except (urllib.error.URLError, socket.timeout, OSError) as error:
        return {
            **base,
            "status": "network_error",
            "http_status": None,
            "content_type": None,
            "downloaded_bytes": 0,
            "truncated": False,
            "looks_like_xsd": False,
            "error_kind": type(error).__name__,
        }


def build_summary(
    *,
    message_def_ids: list[str] | None = None,
    timeout_secs: float = DEFAULT_TIMEOUT_SECS,
    max_bytes: int = DEFAULT_MAX_BYTES,
    opener: UrlOpen = urllib.request.urlopen,
) -> dict[str, Any]:
    """Build a bounded pending-source probe summary."""

    timeout_secs = _positive_float(timeout_secs, "--timeout-secs")
    max_bytes = _positive_int(max_bytes, "--max-bytes")
    ids = _selected_message_ids(message_def_ids)
    probes = [
        _probe_download(
            message_def_id=message_def_id,
            metadata=xsd.KNOWN_PENDING_SCHEMA_SOURCE_METADATA[message_def_id],
            timeout_secs=timeout_secs,
            max_bytes=max_bytes,
            opener=opener,
        )
        for message_def_id in ids
    ]
    successful = [
        probe
        for probe in probes
        if probe["status"] == "reachable" and probe["looks_like_xsd"]
    ]
    summary = {
        "version": PROBE_SUMMARY_VERSION,
        "probed_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "ok": len(successful) == len(probes),
        "probe_count": len(probes),
        "successful_probe_count": len(successful),
        "timeout_secs": timeout_secs,
        "max_bytes": max_bytes,
        "probes": probes,
    }
    summary[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(summary))
    return summary


def run(args: argparse.Namespace) -> int:
    args.summary_out = xsd._optional_cli_path(
        getattr(args, "summary_out", None),
        "summary_out",
    )
    timeout_secs = _positive_float(getattr(args, "timeout_secs", None), "--timeout-secs")
    max_bytes = _positive_int(getattr(args, "max_bytes", None), "--max-bytes")
    message_def_ids = _selected_message_ids(getattr(args, "message_def_id", None))
    if args.summary_out is not None:
        xsd._reject_output_path_smuggling(args.summary_out, "summary_out")
        xsd._reject_repository_output_path(args.summary_out, "summary_out")
        xsd._ensure_text_output_target(
            args.summary_out,
            display_label="summary_out",
            create_parent=False,
        )
    summary = build_summary(
        message_def_ids=message_def_ids,
        timeout_secs=timeout_secs,
        max_bytes=max_bytes,
    )
    text = json.dumps(summary, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        xsd._write_text_output(args.summary_out, text, display_label="summary_out")
    print(text, end="")
    return 0 if summary["ok"] else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Probe official ISO 20022 pending XSD download endpoints.",
        allow_abbrev=False,
    )
    parser.add_argument(
        "--message-def-id",
        action="append",
        default=[],
        help="Known pending ISO message definition id to probe; repeatable.",
    )
    parser.add_argument(
        "--timeout-secs",
        default=DEFAULT_TIMEOUT_SECS,
        help="Per-endpoint timeout in seconds.",
    )
    parser.add_argument(
        "--max-bytes",
        default=DEFAULT_MAX_BYTES,
        help="Maximum bytes to read from each endpoint.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional JSON summary output path.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except ProbeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    except xsd.FixtureManifestError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
