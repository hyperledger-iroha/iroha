#!/usr/bin/env python3
"""Verify ISO 20022 operator adapter receipts.

Purpose:
  This CI/operator canary gate validates receipts written by
  ``iso_audit_notary_adapter.py`` and ``iso_rail_gateway_adapter.py``. It
  recomputes each receipt digest, checks success/status policy, verifies HTTPS
  endpoint policy by default, rejects leaked authorization material, and can
  cross-check referenced source XML or notary anchor files.

Prerequisites:
  Python 3.11+. No third party Python packages are required.

Safety:
  The verifier is read-only. It never contacts Torii, rail gateways, or notary
  services and never deletes receipt/source files.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import urllib.parse
from pathlib import Path
from typing import Any


ANCHOR_DIGEST_FIELD = "anchor_sha256"
INDEX_DIGEST_FIELD = "index_sha256"
RECEIPT_DIGEST_FIELD = "receipt_sha256"
RECEIPT_VERSION = 1
SUMMARY_DIGEST_FIELD = "summary_sha256"
SUPPORTED_KINDS = {"iso-audit-notary", "iso-rail-gateway"}
LEGACY_RAIL_MESSAGE_TYPES = {"colr.007"}


class ReceiptError(RuntimeError):
    """Raised when an operator receipt is invalid."""


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def _canonical_summary_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _is_lower_hex_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _load_json(path: Path) -> Any:
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except FileNotFoundError as error:
        raise ReceiptError(f"{path} does not exist") from error
    except json.JSONDecodeError as error:
        raise ReceiptError(f"{path} is not valid JSON: {error}") from error


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise ReceiptError(f"JSON object contains duplicate key {key!r}")
        seen.add(key)
        result[key] = value
    return result


def digest_without_field(obj: dict[str, Any], digest_field: str) -> str:
    """Compute the canonical object digest with one digest field removed."""

    if digest_field not in obj:
        raise ReceiptError(f"missing {digest_field}")
    body = dict(obj)
    body.pop(digest_field)
    return sha256_hex(_canonical_json_bytes(body))


def require_digest_matches(obj: dict[str, Any], digest_field: str, label: str) -> str:
    """Validate and return an embedded digest field."""

    expected = obj.get(digest_field)
    if not _is_lower_hex_sha256(expected):
        raise ReceiptError(f"{label} has missing or non-canonical {digest_field}")
    actual = digest_without_field(obj, digest_field)
    if actual != expected:
        raise ReceiptError(f"{label} {digest_field} mismatch: expected {expected}, got {actual}")
    return expected


def _check_no_secret_material(receipt: dict[str, Any], path: Path) -> None:
    forbidden = {
        "authorization",
        "bearer_token",
        "token",
        "private_key",
        "secret",
        "x-iroha-signature",
    }
    for key, value in receipt.items():
        lowered = key.lower()
        if lowered in forbidden or any(part in lowered for part in ("authorization", "token", "secret", "private_key")):
            raise ReceiptError(f"{path} contains forbidden secret-looking field {key}")
        if isinstance(value, str) and value.lower().startswith("bearer "):
            raise ReceiptError(f"{path} contains bearer-token material in field {key}")


def _reject_url_control_chars(url: str, label: str) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in url):
        raise ReceiptError(f"{label} must not contain control characters")


def _require_https(url: str, *, allow_insecure_http: bool, label: str) -> None:
    _reject_url_control_chars(url, label)
    try:
        parsed = urllib.parse.urlparse(url)
        hostname = parsed.hostname
    except ValueError as error:
        raise ReceiptError(f"{label} URL {url} is not valid: {error}") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise ReceiptError(f"{label} uses insecure HTTP URL {url}")
        raise ReceiptError(f"{label} must use http or https URL, got {url}")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise ReceiptError(f"{label} URL {url} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise ReceiptError(f"{label} URL {url} must not contain credentials")
    if parsed.params or parsed.query or parsed.fragment:
        raise ReceiptError(f"{label} URL {url} must not contain params, query, or fragment")


def _check_status(receipt: dict[str, Any], path: Path, *, allow_failed: bool) -> None:
    ok = receipt.get("ok")
    status_code = receipt.get("status_code")
    if not isinstance(ok, bool):
        raise ReceiptError(f"{path} ok must be boolean")
    if status_code is not None and (not isinstance(status_code, int) or status_code < 100):
        raise ReceiptError(f"{path} status_code must be null or an HTTP status integer")
    success = ok and isinstance(status_code, int) and 200 <= status_code <= 299
    if not allow_failed and not success:
        raise ReceiptError(f"{path} is not a successful 2xx receipt")


def _verify_anchor_source(receipt: dict[str, Any], path: Path, *, require_source_files: bool) -> None:
    anchor_sha256 = receipt.get(ANCHOR_DIGEST_FIELD)
    index_sha256 = receipt.get(INDEX_DIGEST_FIELD)
    if not _is_lower_hex_sha256(anchor_sha256):
        raise ReceiptError(f"{path} has invalid anchor_sha256")
    if not _is_lower_hex_sha256(index_sha256):
        raise ReceiptError(f"{path} has invalid index_sha256")
    record_count = receipt.get("record_count")
    if not isinstance(record_count, int) or record_count < 0:
        raise ReceiptError(f"{path} record_count must be a non-negative integer")

    anchor_path_raw = receipt.get("anchor_path")
    if not isinstance(anchor_path_raw, str) or not anchor_path_raw:
        raise ReceiptError(f"{path} anchor_path must be a non-empty string")
    anchor_path = Path(anchor_path_raw)
    if not anchor_path.exists():
        if require_source_files:
            raise ReceiptError(f"{path} references missing anchor_path {anchor_path}")
        return

    anchor = _load_json(anchor_path)
    if not isinstance(anchor, dict):
        raise ReceiptError(f"{anchor_path} must contain a JSON object")
    if require_digest_matches(anchor, ANCHOR_DIGEST_FIELD, str(anchor_path)) != anchor_sha256:
        raise ReceiptError(f"{path} anchor_sha256 does not match source anchor")
    if anchor.get(INDEX_DIGEST_FIELD) != index_sha256:
        raise ReceiptError(f"{path} index_sha256 does not match source anchor")
    if anchor.get("record_count") != record_count:
        raise ReceiptError(f"{path} record_count does not match source anchor")


def _verify_rail_source(
    receipt: dict[str, Any],
    path: Path,
    *,
    require_source_files: bool,
    allow_legacy_colr007: bool,
) -> None:
    payload_sha256 = receipt.get("payload_sha256")
    if not _is_lower_hex_sha256(payload_sha256):
        raise ReceiptError(f"{path} has invalid payload_sha256")
    message_type = receipt.get("message_type")
    if not isinstance(message_type, str) or not message_type:
        raise ReceiptError(f"{path} message_type must be a non-empty string")
    if message_type in LEGACY_RAIL_MESSAGE_TYPES and not allow_legacy_colr007:
        raise ReceiptError(
            f"{path} uses legacy rail message_type {message_type!r}; "
            "production evidence must use colr.012"
        )
    profile = receipt.get("profile")
    if profile is not None and (not isinstance(profile, str) or not profile):
        raise ReceiptError(f"{path} profile must be null or a non-empty string")

    xml_path_raw = receipt.get("xml_path")
    if not isinstance(xml_path_raw, str) or not xml_path_raw:
        raise ReceiptError(f"{path} xml_path must be a non-empty string")
    xml_path = Path(xml_path_raw)
    if not xml_path.exists():
        if require_source_files:
            raise ReceiptError(f"{path} references missing xml_path {xml_path}")
        return

    actual = sha256_hex(xml_path.read_bytes())
    if actual != payload_sha256:
        raise ReceiptError(f"{path} payload_sha256 does not match source XML {xml_path}")


def verify_receipt_file(
    path: Path,
    *,
    allow_failed: bool,
    allow_insecure_http: bool,
    allow_legacy_colr007: bool,
    require_source_files: bool,
) -> dict[str, Any]:
    """Verify one operator receipt and return its parsed JSON object."""

    receipt = _load_json(path)
    if not isinstance(receipt, dict):
        raise ReceiptError(f"{path} must contain a JSON object")
    if receipt.get("version") != RECEIPT_VERSION:
        raise ReceiptError(f"{path} has unsupported receipt version")
    kind = receipt.get("receipt_kind")
    if kind not in SUPPORTED_KINDS:
        raise ReceiptError(f"{path} has unsupported receipt_kind {kind!r}")
    require_digest_matches(receipt, RECEIPT_DIGEST_FIELD, str(path))
    _check_no_secret_material(receipt, path)
    _check_status(receipt, path, allow_failed=allow_failed)

    if kind == "iso-audit-notary":
        endpoint = receipt.get("endpoint")
        if not isinstance(endpoint, str) or not endpoint:
            raise ReceiptError(f"{path} endpoint must be a non-empty string")
        _require_https(endpoint, allow_insecure_http=allow_insecure_http, label=str(path))
        _verify_anchor_source(receipt, path, require_source_files=require_source_files)
    elif kind == "iso-rail-gateway":
        endpoint_url = receipt.get("endpoint_url")
        if not isinstance(endpoint_url, str) or not endpoint_url:
            raise ReceiptError(f"{path} endpoint_url must be a non-empty string")
        _require_https(endpoint_url, allow_insecure_http=allow_insecure_http, label=str(path))
        _verify_rail_source(
            receipt,
            path,
            require_source_files=require_source_files,
            allow_legacy_colr007=allow_legacy_colr007,
        )
    else:  # pragma: no cover - guarded above, kept explicit for future kinds.
        raise ReceiptError(f"{path} has unsupported receipt_kind {kind!r}")

    return receipt


def discover_receipts(receipt_dir: Path) -> list[Path]:
    """Return receipt files in deterministic order."""

    if not receipt_dir.is_dir():
        raise ReceiptError(f"{receipt_dir} is not a directory")
    receipts = sorted(receipt_dir.glob("*.receipt.json"))
    if not receipts:
        raise ReceiptError(f"{receipt_dir} has no *.receipt.json files")
    return receipts


def _reject_duplicate_paths(paths: list[Path]) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(path.resolve())
        if key in seen:
            raise ReceiptError(
                f"receipt[{offset}] duplicates receipt[{seen[key]}]: {key}"
            )
        seen[key] = offset


def _receipt_metadata(path: Path, receipt: dict[str, Any]) -> dict[str, Any]:
    metadata: dict[str, Any] = {
        "path": str(path),
        "receipt_kind": receipt["receipt_kind"],
        "receipt_sha256": receipt[RECEIPT_DIGEST_FIELD],
        "ok": receipt.get("ok"),
        "status_code": receipt.get("status_code"),
    }
    if receipt["receipt_kind"] == "iso-audit-notary":
        metadata.update(
            {
                "anchor_sha256": receipt.get(ANCHOR_DIGEST_FIELD),
                "index_sha256": receipt.get(INDEX_DIGEST_FIELD),
                "record_count": receipt.get("record_count"),
            }
        )
    elif receipt["receipt_kind"] == "iso-rail-gateway":
        metadata.update(
            {
                "message_type": receipt.get("message_type"),
                "payload_sha256": receipt.get("payload_sha256"),
                "profile": receipt.get("profile"),
            }
        )
    return metadata


def run(args: argparse.Namespace) -> int:
    paths = list(args.receipt)
    for receipt_dir in args.receipt_dir:
        paths.extend(discover_receipts(receipt_dir))
    if not paths:
        raise ReceiptError("provide at least one --receipt or --receipt-dir")
    _reject_duplicate_paths(paths)

    verified: list[dict[str, Any]] = []
    receipt_entries: list[dict[str, Any]] = []
    seen_receipt_digests: dict[str, Path] = {}
    for path in paths:
        receipt = verify_receipt_file(
            path,
            allow_failed=args.allow_failed,
            allow_insecure_http=args.allow_insecure_http,
            allow_legacy_colr007=args.allow_legacy_colr007,
            require_source_files=args.require_source_files,
        )
        receipt_digest = receipt[RECEIPT_DIGEST_FIELD]
        if receipt_digest in seen_receipt_digests:
            raise ReceiptError(
                f"{path} {RECEIPT_DIGEST_FIELD} duplicates "
                f"{seen_receipt_digests[receipt_digest]}: {receipt_digest}"
            )
        seen_receipt_digests[receipt_digest] = path
        verified.append(receipt)
        receipt_entries.append(_receipt_metadata(path, receipt))

    summary = {
        "verified_receipts": len(verified),
        "receipt_kind": sorted({receipt["receipt_kind"] for receipt in verified}),
        "allow_failed": args.allow_failed,
        "allow_insecure_http": args.allow_insecure_http,
        "allow_legacy_colr007": args.allow_legacy_colr007,
        "require_source_files": args.require_source_files,
        "receipts": receipt_entries,
    }
    summary[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_summary_json_bytes(summary))
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify ISO 20022 operator rail/notary adapter receipts."
    )
    parser.add_argument(
        "--receipt",
        action="append",
        default=[],
        type=Path,
        help="Receipt JSON file to verify; repeatable.",
    )
    parser.add_argument(
        "--receipt-dir",
        action="append",
        default=[],
        type=Path,
        help="Directory containing *.receipt.json files to verify.",
    )
    parser.add_argument(
        "--allow-failed",
        action="store_true",
        help="Allow receipts whose remote submission/publication failed.",
    )
    parser.add_argument(
        "--allow-insecure-http",
        action="store_true",
        help="Allow http:// endpoints in receipts for local tests.",
    )
    parser.add_argument(
        "--allow-legacy-colr007",
        action="store_true",
        help="Allow legacy local colr.007 rail receipts; production evidence should use colr.012.",
    )
    parser.add_argument(
        "--require-source-files",
        action="store_true",
        help="Require referenced source XML/anchor files to exist and match receipt digests.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except ReceiptError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
