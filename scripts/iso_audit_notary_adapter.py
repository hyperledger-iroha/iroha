#!/usr/bin/env python3
"""Publish ISO 20022 audit export anchors to external archival/notary services.

Purpose:
  This operator-side adapter consumes the digest-bound preimages written by
  Torii's ``iso_bridge.audit_export_dir``. It verifies the local
  ``messages.index.json`` and ``*.notary.json`` preimages before publishing them
  to configured HTTPS endpoints, then writes bounded local receipts.

Prerequisites:
  Python 3.11+ and a populated ISO audit export directory from Torii. No third
  party Python packages are required.

Safety:
  The script never mutates Torii state and never deletes files. Plain HTTP
  endpoints are rejected unless ``--allow-insecure-http`` is supplied for local
  tests. Bearer tokens are read from a runtime-only file and are never persisted
  into receipts.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ANCHOR_DIGEST_FIELD = "anchor_sha256"
ANCHOR_DIR = "anchors"
ANCHOR_VERSION = 1
DEFAULT_RESPONSE_LIMIT_BYTES = 64 * 1024
INDEX_DIGEST_FIELD = "index_sha256"
INDEX_FILE = "messages.index.json"
LATEST_ANCHOR_FILE = "latest.notary.json"
RECEIPT_DIGEST_FIELD = "receipt_sha256"
RECEIPT_VERSION = 1


class AdapterError(RuntimeError):
    """Raised when an audit preimage or publication response is invalid."""


@dataclass(frozen=True)
class VerifiedAnchor:
    """Verified anchor bytes and selected metadata ready for publication."""

    path: Path
    payload: dict[str, Any]
    raw: bytes
    index_sha256: str
    anchor_sha256: str
    record_count: int


@dataclass(frozen=True)
class PublishResult:
    """Publication outcome for one endpoint."""

    endpoint: str
    status_code: int | None
    ok: bool
    response_body_sha256: str | None
    response_body_preview: str | None
    error: str | None = None


def _load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise AdapterError(f"{path} does not exist") from error
    except json.JSONDecodeError as error:
        raise AdapterError(f"{path} is not valid JSON: {error}") from error


def _load_json_bytes(path: Path) -> tuple[Any, bytes]:
    try:
        raw = path.read_bytes()
    except FileNotFoundError as error:
        raise AdapterError(f"{path} does not exist") from error
    try:
        return json.loads(raw.decode("utf-8")), raw
    except UnicodeDecodeError as error:
        raise AdapterError(f"{path} is not UTF-8 JSON") from error
    except json.JSONDecodeError as error:
        raise AdapterError(f"{path} is not valid JSON: {error}") from error


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _is_lower_hex_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def digest_without_field(obj: dict[str, Any], digest_field: str) -> str:
    """Compute the same JSON-object digest shape used by Torii export code."""

    if digest_field not in obj:
        raise AdapterError(f"missing {digest_field}")
    body = dict(obj)
    body.pop(digest_field)
    return sha256_hex(_canonical_json_bytes(body))


def require_digest_matches(obj: dict[str, Any], digest_field: str, label: str) -> str:
    """Validate and return an embedded digest field."""

    expected = obj.get(digest_field)
    if not _is_lower_hex_sha256(expected):
        raise AdapterError(f"{label} has missing or non-canonical {digest_field}")
    actual = digest_without_field(obj, digest_field)
    if actual != expected:
        raise AdapterError(f"{label} {digest_field} mismatch: expected {expected}, got {actual}")
    return expected


def verify_audit_index(index: Any) -> dict[str, Any]:
    """Verify the exported audit index digest and basic record-count shape."""

    if not isinstance(index, dict):
        raise AdapterError("audit index must be a JSON object")
    require_digest_matches(index, INDEX_DIGEST_FIELD, "audit index")
    record_count = index.get("record_count")
    records = index.get("records")
    if not isinstance(record_count, int) or record_count < 0:
        raise AdapterError("audit index record_count must be a non-negative integer")
    if not isinstance(records, list):
        raise AdapterError("audit index records must be an array")
    if len(records) != record_count:
        raise AdapterError(
            f"audit index record_count {record_count} does not match records length {len(records)}"
        )
    for offset, record in enumerate(records):
        if not isinstance(record, dict):
            raise AdapterError(f"audit index record {offset} must be an object")
        if not isinstance(record.get("message_id"), str) or not record["message_id"]:
            raise AdapterError(f"audit index record {offset} has invalid message_id")
        if not isinstance(record.get("filename"), str) or not record["filename"]:
            raise AdapterError(f"audit index record {offset} has invalid filename")
        if not _is_lower_hex_sha256(record.get("record_sha256")):
            raise AdapterError(f"audit index record {offset} has invalid record_sha256")
    return index


def verify_anchor_file(export_dir: Path, anchor_path: Path) -> VerifiedAnchor:
    """Verify one notary anchor against the export directory index file."""

    anchor_value, raw = _load_json_bytes(anchor_path)
    if not isinstance(anchor_value, dict):
        raise AdapterError(f"{anchor_path} must contain a JSON object")
    if anchor_value.get("version") != ANCHOR_VERSION:
        raise AdapterError(f"{anchor_path} has unsupported anchor version")
    anchor_sha256 = require_digest_matches(anchor_value, ANCHOR_DIGEST_FIELD, str(anchor_path))

    audit_index = verify_audit_index(anchor_value.get("audit_index"))
    index_sha256 = anchor_value.get(INDEX_DIGEST_FIELD)
    embedded_index_sha256 = audit_index.get(INDEX_DIGEST_FIELD)
    if index_sha256 != embedded_index_sha256:
        raise AdapterError(
            f"{anchor_path} index_sha256 does not match embedded audit index digest"
        )
    if anchor_value.get("record_count") != audit_index.get("record_count"):
        raise AdapterError(f"{anchor_path} record_count does not match embedded audit index")

    index_file = export_dir / INDEX_FILE
    exported_index = verify_audit_index(_load_json(index_file))
    if exported_index != audit_index:
        raise AdapterError(f"{anchor_path} embedded audit index differs from {index_file}")

    anchors_dir = export_dir / ANCHOR_DIR
    try:
        relative_anchor = anchor_path.resolve().relative_to(anchors_dir.resolve())
    except ValueError:
        relative_anchor = None
    if relative_anchor is not None:
        expected_name = f"{index_sha256}.notary.json"
        if relative_anchor.name != expected_name:
            raise AdapterError(
                f"{anchor_path} filename must be digest-addressed as {expected_name}"
            )

    latest = export_dir / LATEST_ANCHOR_FILE
    if anchor_path.resolve() == latest.resolve():
        digest_anchor = anchors_dir / f"{index_sha256}.notary.json"
        if not digest_anchor.exists():
            raise AdapterError(f"{latest} has no digest-addressed peer {digest_anchor}")
        if digest_anchor.read_bytes() != raw:
            raise AdapterError(f"{latest} differs from digest-addressed peer {digest_anchor}")

    return VerifiedAnchor(
        path=anchor_path,
        payload=anchor_value,
        raw=raw,
        index_sha256=index_sha256,
        anchor_sha256=anchor_sha256,
        record_count=anchor_value["record_count"],
    )


def discover_anchor_paths(export_dir: Path, all_anchors: bool) -> list[Path]:
    """Return anchor paths to publish in deterministic order."""

    if all_anchors:
        anchors = sorted((export_dir / ANCHOR_DIR).glob("*.notary.json"))
        if not anchors:
            raise AdapterError(f"{export_dir / ANCHOR_DIR} has no *.notary.json anchors")
        return anchors
    return [export_dir / LATEST_ANCHOR_FILE]


def _endpoint_sha256(endpoint: str) -> str:
    return sha256_hex(endpoint.encode("utf-8"))


def _reject_url_control_chars(url: str, label: str) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in url):
        raise AdapterError(f"{label} must not contain control characters")


def _validate_endpoint(endpoint: str, allow_insecure_http: bool) -> None:
    _reject_url_control_chars(endpoint, "endpoint")
    try:
        parsed = urllib.parse.urlparse(endpoint)
        hostname = parsed.hostname
    except ValueError as error:
        raise AdapterError(f"endpoint {endpoint} is not a valid URL: {error}") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise AdapterError(
                f"refusing insecure HTTP endpoint {endpoint}; pass --allow-insecure-http for local tests"
            )
        raise AdapterError(f"endpoint {endpoint} must use http or https")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise AdapterError(f"endpoint {endpoint} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise AdapterError(f"endpoint {endpoint} must not contain credentials")
    if parsed.params or parsed.query or parsed.fragment:
        raise AdapterError(
            f"endpoint {endpoint} must not contain params, query, or fragment"
        )


def _load_bearer_token(path: Path | None) -> str | None:
    if path is None:
        return None
    token = path.read_text(encoding="utf-8").strip()
    if not token:
        raise AdapterError(f"bearer token file {path} is empty")
    return token


def publish_anchor(
    anchor: VerifiedAnchor,
    endpoint: str,
    *,
    timeout_secs: float,
    response_limit_bytes: int,
    bearer_token: str | None,
) -> PublishResult:
    """POST a verified anchor preimage and return a bounded outcome."""

    headers = {
        "Content-Type": "application/json",
        "X-Iroha-Iso-Anchor-Sha256": anchor.anchor_sha256,
        "X-Iroha-Iso-Index-Sha256": anchor.index_sha256,
    }
    if bearer_token is not None:
        headers["Authorization"] = f"Bearer {bearer_token}"
    request = urllib.request.Request(endpoint, data=anchor.raw, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(request, timeout=timeout_secs) as response:
            body = response.read(response_limit_bytes + 1)
            if len(body) > response_limit_bytes:
                raise AdapterError(
                    f"{endpoint} response exceeded {response_limit_bytes} byte limit"
                )
            status_code = int(response.status)
    except urllib.error.HTTPError as error:
        try:
            body = error.read(response_limit_bytes + 1)
        finally:
            error.close()
        if len(body) > response_limit_bytes:
            raise AdapterError(f"{endpoint} error response exceeded {response_limit_bytes} byte limit")
        return PublishResult(
            endpoint=endpoint,
            status_code=int(error.code),
            ok=False,
            response_body_sha256=sha256_hex(body),
            response_body_preview=_response_preview(body),
            error=f"HTTP {error.code}",
        )
    except urllib.error.URLError as error:
        return PublishResult(
            endpoint=endpoint,
            status_code=None,
            ok=False,
            response_body_sha256=None,
            response_body_preview=None,
            error=str(error.reason),
        )

    ok = 200 <= status_code <= 299
    return PublishResult(
        endpoint=endpoint,
        status_code=status_code,
        ok=ok,
        response_body_sha256=sha256_hex(body),
        response_body_preview=_response_preview(body),
        error=None if ok else f"HTTP {status_code}",
    )


def _response_preview(body: bytes) -> str:
    return body[:4096].decode("utf-8", errors="replace")


def receipt_value(anchor: VerifiedAnchor, result: PublishResult) -> dict[str, Any]:
    """Build a receipt JSON object for one publication attempt."""

    receipt: dict[str, Any] = {
        "version": RECEIPT_VERSION,
        "receipt_kind": "iso-audit-notary",
        "published_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "endpoint": result.endpoint,
        "endpoint_sha256": _endpoint_sha256(result.endpoint),
        "anchor_path": str(anchor.path),
        "anchor_sha256": anchor.anchor_sha256,
        "index_sha256": anchor.index_sha256,
        "record_count": anchor.record_count,
        "status_code": result.status_code,
        "ok": result.ok,
        "response_body_sha256": result.response_body_sha256,
        "response_body_preview": result.response_body_preview,
        "error": result.error,
    }
    receipt[RECEIPT_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(receipt))
    return receipt


def write_receipt(receipt_dir: Path, anchor: VerifiedAnchor, result: PublishResult) -> Path:
    """Write one receipt and return its path."""

    receipt_dir.mkdir(parents=True, exist_ok=True)
    receipt = receipt_value(anchor, result)
    path = receipt_dir / (
        f"{anchor.index_sha256}.{_endpoint_sha256(result.endpoint)}.receipt.json"
    )
    path.write_text(json.dumps(receipt, indent=2, sort_keys=False) + "\n", encoding="utf-8")
    return path


def run(args: argparse.Namespace) -> int:
    export_dir = args.export_dir.resolve()
    receipt_dir = (args.receipt_dir or export_dir / "receipts").resolve()
    endpoints = list(args.endpoint)
    for endpoint in endpoints:
        _validate_endpoint(endpoint, args.allow_insecure_http)
    bearer_token = _load_bearer_token(args.bearer_token_file)

    anchors = [
        verify_anchor_file(export_dir, anchor_path)
        for anchor_path in discover_anchor_paths(export_dir, args.all)
    ]
    if args.dry_run:
        summary = {
            "validated_anchors": len(anchors),
            "index_sha256": [anchor.index_sha256 for anchor in anchors],
            "record_count": [anchor.record_count for anchor in anchors],
            "dry_run": True,
        }
        print(json.dumps(summary, indent=2, sort_keys=True))
        return 0
    if not endpoints:
        raise AdapterError("at least one --endpoint is required unless --dry-run is set")

    failures = 0
    receipts: list[str] = []
    for anchor in anchors:
        for endpoint in endpoints:
            result = publish_anchor(
                anchor,
                endpoint,
                timeout_secs=args.timeout_secs,
                response_limit_bytes=args.response_limit_bytes,
                bearer_token=bearer_token,
            )
            receipts.append(str(write_receipt(receipt_dir, anchor, result)))
            if not result.ok:
                failures += 1

    summary = {
        "published_anchors": len(anchors),
        "endpoint_count": len(endpoints),
        "receipts": receipts,
        "failures": failures,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 1 if failures else 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify and publish ISO 20022 audit_export_dir notary anchors."
    )
    parser.add_argument(
        "--export-dir",
        required=True,
        type=Path,
        help="Torii iso_bridge.audit_export_dir containing messages.index.json and anchors/.",
    )
    parser.add_argument(
        "--endpoint",
        action="append",
        default=[],
        help="HTTPS archival/notary endpoint to POST each verified anchor to; repeatable.",
    )
    parser.add_argument(
        "--receipt-dir",
        type=Path,
        help="Directory for local publication receipts (default: <export-dir>/receipts).",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Publish all digest-addressed anchors instead of latest.notary.json.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Only verify preimages and print a validation summary; do not publish.",
    )
    parser.add_argument(
        "--allow-insecure-http",
        action="store_true",
        help="Allow http:// endpoints for local tests; production endpoints should use HTTPS.",
    )
    parser.add_argument(
        "--bearer-token-file",
        type=Path,
        help="Runtime-only file containing a bearer token for endpoint Authorization.",
    )
    parser.add_argument(
        "--timeout-secs",
        type=float,
        default=10.0,
        help="HTTP timeout in seconds per publication attempt.",
    )
    parser.add_argument(
        "--response-limit-bytes",
        type=int,
        default=DEFAULT_RESPONSE_LIMIT_BYTES,
        help="Maximum response body bytes retained in a receipt.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except AdapterError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
