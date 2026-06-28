#!/usr/bin/env python3
"""Probe official ISO 20022 pending XSD download endpoints.

This utility records bounded reachability evidence for the official ISO
download URLs tracked by ``iso_xsd_fixture_verify``. It intentionally reads only
small byte ranges and does not import schema bytes into the repository.
"""

from __future__ import annotations

import argparse
import datetime as dt
import http.client
import json
import re
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
MAX_TIMEOUT_SECS = 300.0
DEFAULT_MAX_BYTES = 4096
MAX_PROBE_BYTES = 65536
MAX_CONTENT_TYPE_CHARS = 256
SUMMARY_DIGEST_FIELD = "summary_sha256"
ASCII_FLOAT_RE = re.compile(
    r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?(?:0|[1-9][0-9]*))?"
)
ASCII_INT_RE = re.compile(r"0|[1-9][0-9]*")
XSD_ROOT_OPEN_RE = re.compile(
    rb"\A(?:\xef\xbb\xbf)?[ \t\r\n]*"
    rb"(?:<\?xml\b[^>]*\?>[ \t\r\n]*)?"
    rb"(?:(?:<!--.*?-->|<\?[A-Za-z_][^?]*\?>)[ \t\r\n]*)*"
    rb"(?:"
    rb"<xs:schema(?:[ \t\r\n/>])"
    rb"(?=[^>]*\bxmlns:xs\s*=\s*['\"]http://www\.w3\.org/2001/XMLSchema['\"])"
    rb"|<xsd:schema(?:[ \t\r\n/>])"
    rb"(?=[^>]*\bxmlns:xsd\s*=\s*['\"]http://www\.w3\.org/2001/XMLSchema['\"])"
    rb"|<schema(?:[ \t\r\n/>])"
    rb"(?=[^>]*\bxmlns\s*=\s*['\"]http://www\.w3\.org/2001/XMLSchema['\"])"
    rb")",
    re.DOTALL,
)


class ProbeError(RuntimeError):
    """Raised when probe inputs are invalid."""


def _plain_text(value: str, label: str) -> str:
    try:
        return str.__str__(value)
    except Exception:
        raise ProbeError(f"{label} must be valid text") from None


def _normalise_cli_argv(argv: list[str] | None) -> list[str]:
    if argv is None:
        raw_sys_argv = sys.argv
        if type(raw_sys_argv) is not list:
            raise ProbeError("sys.argv must be a plain argument list")
        raw_args = raw_sys_argv[1:]
    else:
        raw_args = argv
    if type(raw_args) is not list:
        raise ProbeError("argv must be a plain argument list")
    normalised: list[str] = []
    for index, value in enumerate(raw_args):
        if not isinstance(value, str):
            raise ProbeError(f"argv[{index}] must be a string")
        normalised.append(_plain_text(value, f"argv[{index}]"))
    return normalised


def _require_plain_namespace(args: argparse.Namespace) -> argparse.Namespace:
    if type(args) is not argparse.Namespace:
        raise ProbeError("args must be an argparse.Namespace")
    return args


UrlOpen = Callable[..., Any]


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        allow_nan=False,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    import hashlib

    return hashlib.sha256(data).hexdigest()


def _ascii_cli_number_text(value: Any, label: str) -> Any:
    if not isinstance(value, str):
        return value
    value = _plain_text(value, label)
    if not value:
        raise ProbeError(f"{label} must not be empty")
    if value != value.strip():
        raise ProbeError(f"{label} must not have surrounding whitespace")
    try:
        value.encode("ascii")
    except UnicodeEncodeError as error:
        raise ProbeError(f"{label} must use ASCII digits") from error
    return value


def _positive_float(value: Any, label: str) -> float:
    if isinstance(value, bool):
        raise ProbeError(f"{label} must be a positive finite number")
    value = _ascii_cli_number_text(value, label)
    if isinstance(value, str):
        if ASCII_FLOAT_RE.fullmatch(value) is None:
            raise ProbeError(f"{label} must be a positive finite number")
        parsed = float(value)
    elif type(value) in (int, float):
        parsed = float(value)
    else:
        raise ProbeError(f"{label} must be a positive finite number")
    if not (parsed > 0.0 and parsed < float("inf")):
        raise ProbeError(f"{label} must be a positive finite number")
    if parsed > MAX_TIMEOUT_SECS:
        raise ProbeError(f"{label} must be no larger than {MAX_TIMEOUT_SECS:g}")
    return parsed


def _positive_int(value: Any, label: str) -> int:
    if isinstance(value, bool):
        raise ProbeError(f"{label} must be a positive integer")
    value = _ascii_cli_number_text(value, label)
    if isinstance(value, str):
        if ASCII_INT_RE.fullmatch(value) is None:
            raise ProbeError(f"{label} must be a positive integer")
        parsed = int(value)
    elif type(value) is int:
        parsed = value
    else:
        raise ProbeError(f"{label} must be a positive integer")
    if parsed <= 0:
        raise ProbeError(f"{label} must be a positive integer")
    if parsed > MAX_PROBE_BYTES:
        raise ProbeError(f"{label} must be no larger than {MAX_PROBE_BYTES}")
    return parsed


def _preflight_probe_limit_cli_values(argv: list[str] | None) -> None:
    raw_args = _normalise_cli_argv(argv)
    validators = {
        "--max-bytes": _positive_int,
        "--timeout-secs": _positive_float,
    }
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise ProbeError("argument terminator is not supported")
        matched = False
        for flag, validator in validators.items():
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise ProbeError(f"{flag} requires a numeric value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise ProbeError(f"{flag} requires a numeric value")
                validator(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise ProbeError(f"{flag} requires a numeric value")
                validator(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _selected_message_ids(values: Any) -> list[str]:
    known = xsd.KNOWN_PENDING_SCHEMA_SOURCE_METADATA
    if values is None:
        return sorted(known)
    if isinstance(values, (str, bytes)) or type(values) not in (list, tuple):
        raise ProbeError("--message-def-id must be a repeatable string list")
    if len(values) == 0:
        return sorted(known)
    selected = list(values)
    result: list[str] = []
    seen: dict[str, int] = {}
    for offset, value in enumerate(selected):
        if not isinstance(value, str):
            raise ProbeError(f"--message-def-id[{offset}] must be a string")
        value = _plain_text(value, f"--message-def-id[{offset}]")
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
    return XSD_ROOT_OPEN_RE.search(sample) is not None


def _content_type(headers: Any) -> str | None:
    if headers is None:
        return None
    value = None
    try:
        get_content_type = getattr(headers, "get_content_type", None)
    except Exception:
        get_content_type = None
    if callable(get_content_type):
        try:
            value = get_content_type()
        except Exception:
            value = None
    if value is None:
        try:
            get = getattr(headers, "get", None)
        except Exception:
            get = None
        if callable(get):
            try:
                value = get("content-type") or get("Content-Type")
            except Exception:
                value = None
    if not isinstance(value, str) or not value:
        return None
    try:
        if len(value) > MAX_CONTENT_TYPE_CHARS:
            return None
        if value != value.strip():
            return None
        if xsd._contains_control_character(value):
            return None
        if any(ord(ch) > 0x7E for ch in value):
            return None
        if xsd._contains_secret_material(value) or xsd._contains_secret_identifier_material(value):
            return None
    except Exception:
        return None
    return value


def _response_headers(response: Any) -> Any:
    try:
        return getattr(response, "headers", None)
    except Exception:
        return None


def _http_status_code(value: Any) -> int | None:
    if type(value) is not int:
        return None
    if 100 <= value <= 599:
        return value
    return None


def _response_status(response: Any) -> int | None:
    try:
        raw_status = getattr(response, "status", None)
    except Exception:
        raw_status = None
    status = _http_status_code(raw_status)
    if status is not None:
        return status
    try:
        getcode = getattr(response, "getcode", None)
    except Exception:
        getcode = None
    if callable(getcode):
        try:
            return _http_status_code(getcode())
        except Exception:
            return None
    return None


def _http_error_status(error: Any) -> int | None:
    try:
        code = getattr(error, "code", None)
    except Exception:
        code = None
    return _http_status_code(code)


def _enter_probe_response(response_context: Any) -> tuple[Any, Any | None] | None:
    try:
        enter = getattr(response_context, "__enter__", None)
    except Exception:
        return None
    if enter is None:
        return response_context, None
    if not callable(enter):
        return None
    try:
        exit_func = getattr(response_context, "__exit__", None)
    except Exception:
        exit_func = None
    try:
        response = enter()
    except Exception:
        return None
    return response, exit_func


def _close_probe_response(response_context: Any, exit_func: Any | None) -> None:
    if callable(exit_func):
        try:
            exit_func(None, None, None)
        except Exception:
            return
        return
    try:
        close = getattr(response_context, "close", None)
    except Exception:
        return
    if not callable(close):
        return
    try:
        close()
    except Exception:
        return


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
        response_context = opener(request, timeout=timeout_secs)
        entered = _enter_probe_response(response_context)
        if entered is None:
            return {
                **base,
                "status": "network_error",
                "http_status": None,
                "content_type": None,
                "downloaded_bytes": 0,
                "sample_sha256": None,
                "truncated": False,
                "looks_like_xsd": False,
                "error_kind": "NetworkError",
            }
        response, exit_func = entered
        try:
            status = _response_status(response)
            if status is None:
                return {
                    **base,
                    "status": "network_error",
                    "http_status": None,
                    "content_type": None,
                    "downloaded_bytes": 0,
                    "sample_sha256": None,
                    "truncated": False,
                    "looks_like_xsd": False,
                    "error_kind": "NetworkError",
                }
            if 400 <= status <= 599:
                return {
                    **base,
                    "status": "http_error",
                    "http_status": status,
                    "content_type": _content_type(_response_headers(response)),
                    "downloaded_bytes": 0,
                    "sample_sha256": None,
                    "truncated": False,
                    "looks_like_xsd": False,
                    "error_kind": "HTTPError",
                }
            try:
                data = response.read(max_bytes + 1)
            except (http.client.HTTPException, OSError, RuntimeError, TypeError, ValueError):
                return {
                    **base,
                    "status": "network_error",
                    "http_status": None,
                    "content_type": None,
                    "downloaded_bytes": 0,
                    "sample_sha256": None,
                    "truncated": False,
                    "looks_like_xsd": False,
                    "error_kind": "NetworkError",
                }
            if type(data) not in (bytes, bytearray, memoryview):
                return {
                    **base,
                    "status": "network_error",
                    "http_status": None,
                    "content_type": None,
                    "downloaded_bytes": 0,
                    "sample_sha256": None,
                    "truncated": False,
                    "looks_like_xsd": False,
                    "error_kind": "NetworkError",
            }
            try:
                if type(data) is memoryview:
                    byte_data = data.cast("B")
                    total_bytes = byte_data.nbytes
                    downloaded = min(total_bytes, max_bytes)
                    sample = byte_data[:downloaded].tobytes()
                elif type(data) is bytes:
                    total_bytes = len(data)
                    downloaded = min(total_bytes, max_bytes)
                    sample = data[:downloaded]
                else:
                    total_bytes = len(data)
                    downloaded = min(total_bytes, max_bytes)
                    sample = bytes(data[:downloaded])
            except (TypeError, ValueError):
                return {
                    **base,
                    "status": "network_error",
                    "http_status": None,
                    "content_type": None,
                    "downloaded_bytes": 0,
                    "sample_sha256": None,
                    "truncated": False,
                    "looks_like_xsd": False,
                    "error_kind": "NetworkError",
                }
            looks_like_xsd = _looks_like_xsd(sample)
            if not downloaded or looks_like_xsd and not (200 <= status <= 299):
                return {
                    **base,
                    "status": "network_error",
                    "http_status": None,
                    "content_type": None,
                    "downloaded_bytes": 0,
                    "sample_sha256": None,
                    "truncated": False,
                    "looks_like_xsd": False,
                    "error_kind": "NetworkError",
                }
            return {
                **base,
                "status": (
                    "reachable"
                    if 200 <= status <= 299
                    and downloaded
                    and looks_like_xsd
                    else "unexpected"
                ),
                "http_status": status,
                "content_type": _content_type(_response_headers(response)),
                "downloaded_bytes": downloaded,
                "sample_sha256": sha256_hex(sample) if sample else None,
                "truncated": total_bytes > max_bytes,
                "looks_like_xsd": looks_like_xsd,
                "error_kind": None,
            }
        finally:
            _close_probe_response(response_context, exit_func)
    except urllib.error.HTTPError as error:
        try:
            status = _http_error_status(error)
            if status is None or not (400 <= status <= 599):
                return {
                    **base,
                    "status": "network_error",
                    "http_status": None,
                    "content_type": None,
                    "downloaded_bytes": 0,
                    "sample_sha256": None,
                    "truncated": False,
                    "looks_like_xsd": False,
                    "error_kind": "NetworkError",
                }
            return {
                **base,
                "status": "http_error",
                "http_status": status,
                "content_type": _content_type(_response_headers(error)),
                "downloaded_bytes": 0,
                "sample_sha256": None,
                "truncated": False,
                "looks_like_xsd": False,
                "error_kind": "HTTPError",
            }
        finally:
            _close_probe_response(error, None)
    except TimeoutError:
        return {
            **base,
            "status": "timeout",
            "http_status": None,
            "content_type": None,
            "downloaded_bytes": 0,
            "sample_sha256": None,
            "truncated": False,
            "looks_like_xsd": False,
            "error_kind": "TimeoutError",
        }
    except (urllib.error.URLError, socket.timeout, OSError):
        return {
            **base,
            "status": "network_error",
            "http_status": None,
            "content_type": None,
            "downloaded_bytes": 0,
            "sample_sha256": None,
            "truncated": False,
            "looks_like_xsd": False,
            "error_kind": "NetworkError",
        }
    except (RuntimeError, TypeError, ValueError):
        return {
            **base,
            "status": "network_error",
            "http_status": None,
            "content_type": None,
            "downloaded_bytes": 0,
            "sample_sha256": None,
            "truncated": False,
            "looks_like_xsd": False,
            "error_kind": "NetworkError",
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
    args = _require_plain_namespace(args)
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
    text = json.dumps(summary, allow_nan=False, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        xsd._write_text_output(args.summary_out, text, display_label="summary_out")
    print(text, end="")
    return 0 if summary["ok"] else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog=Path(__file__).name,
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
    try:
        normalised_argv = _normalise_cli_argv(argv)
        parser = build_parser()
        xsd._preflight_raw_cli_secrets(
            normalised_argv,
            {
                "--max-bytes",
                "--message-def-id",
                "--summary-out",
                "--timeout-secs",
            },
        )
        _preflight_probe_limit_cli_values(normalised_argv)
        xsd._preflight_output_cli_paths(normalised_argv, {"--summary-out"})
        args = parser.parse_args(normalised_argv)
        return run(args)
    except ProbeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    except xsd.FixtureManifestError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
