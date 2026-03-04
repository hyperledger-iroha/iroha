#!/usr/bin/env python3
"""Bulk SNS registration helper for SN-3b."""

from __future__ import annotations

import argparse
import csv
import json
import re
import subprocess
import sys
import tempfile
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Iterable, Sequence

CANONICAL_LABEL_RE = re.compile(r"^[a-z0-9\-_]+$")
DEFAULT_REQUIRED_COLUMNS = (
    "label",
    "suffix_id",
    "owner",
    "term_years",
    "payment_asset_id",
    "payment_gross",
    "payment_net",
    "settlement_tx",
    "payment_payer",
    "payment_signature",
)


class BulkOnboardError(Exception):
    """Raised when CSV parsing or validation fails."""


@dataclass(frozen=True)
class ColumnMap:
    controllers: str = "controllers"
    metadata: str = "metadata"
    governance: str = "governance"


@dataclass(frozen=True)
class ScriptOptions:
    require_governance: bool
    default_controller_policy: str
    columns: ColumnMap


def canonicalize_label(value: str, context: str) -> str:
    trimmed = value.strip()
    if not trimmed:
        raise BulkOnboardError(f"{context} label must not be empty")
    normalized = unicodedata.normalize("NFC", trimmed)
    try:
        ascii_label = normalized.encode("idna").decode("ascii")
    except UnicodeError as err:  # pragma: no cover - extremely rare
        raise BulkOnboardError(f"{context} label failed IDNA encoding: {err}") from err
    lowered = ascii_label.lower()
    if not CANONICAL_LABEL_RE.fullmatch(lowered):
        raise BulkOnboardError(
            f"{context} label '{value}' canonicalized to '{lowered}' "
            "which violates Norm v1 (letters, digits, '-' and '_' only)"
        )
    return lowered


def parse_int(
    raw: str,
    context: str,
    *,
    minimum: int,
    maximum: int,
) -> int:
    text = (raw or "").strip()
    if not text:
        raise BulkOnboardError(f"{context} is required")
    base = 16 if text.lower().startswith("0x") else 10
    try:
        value = int(text, base)
    except ValueError as err:
        raise BulkOnboardError(f"{context} must be an integer") from err
    if value < minimum or value > maximum:
        raise BulkOnboardError(
            f"{context} must be between {minimum} and {maximum}, got {value}"
        )
    return value


def load_jsonish(
    raw: str | None,
    base_dir: Path,
    context: str,
    *,
    expect_object: bool = True,
    allow_empty: bool = True,
    fallback_to_string: bool = False,
) -> Any:
    if raw is None:
        if expect_object and not allow_empty:
            raise BulkOnboardError(f"{context} is required")
        return {} if expect_object else None
    text = raw.strip()
    if not text:
        if expect_object and not allow_empty:
            raise BulkOnboardError(f"{context} is required")
        return {} if expect_object else None
    payload = text
    if text.startswith("@"):
        file_path = (base_dir / text[1:]).resolve()
        if not file_path.exists():
            raise BulkOnboardError(f"{context} references missing file {file_path}")
        payload = file_path.read_text(encoding="utf-8")
    try:
        parsed = json.loads(payload)
    except json.JSONDecodeError:
        if fallback_to_string:
            return payload
        raise BulkOnboardError(f"{context} must contain valid JSON")
    if expect_object and not isinstance(parsed, dict):
        raise BulkOnboardError(f"{context} must be a JSON object")
    return parsed


def parse_controllers(
    value: str | None,
    owner: str,
    policy: str,
    context: str,
) -> list[str]:
    if value:
        entries: list[str] = []
        for literal in re.split(r"[;,]", value):
            entry = literal.strip()
            if entry:
                entries.append(entry)
        if entries:
            return entries
    if policy == "owner":
        return [owner]
    if policy == "none":
        return []
    raise BulkOnboardError(f"{context} invalid controller policy {policy}")


def build_controller_payload(address: str) -> dict[str, Any]:
    return {
        "controller_type": {"kind": "Account"},
        "account_address": address,
        "resolver_template_id": None,
        "payload": {},
    }


def parse_payment(row: dict[str, str], base_dir: Path, row_desc: str) -> dict[str, Any]:
    asset_id = (row.get("payment_asset_id") or "").strip()
    if not asset_id:
        raise BulkOnboardError(f"{row_desc} payment_asset_id is required")
    gross_amount = parse_int(
        row.get("payment_gross", ""),
        f"{row_desc} payment_gross",
        minimum=0,
        maximum=2**63 - 1,
    )
    net_amount = parse_int(
        row.get("payment_net", ""),
        f"{row_desc} payment_net",
        minimum=0,
        maximum=2**63 - 1,
    )
    settlement_tx = load_jsonish(
        row.get("settlement_tx"),
        base_dir,
        f"{row_desc} settlement_tx",
        expect_object=False,
        allow_empty=False,
        fallback_to_string=True,
    )
    payer = (row.get("payment_payer") or "").strip()
    if not payer:
        raise BulkOnboardError(f"{row_desc} payment_payer is required")
    signature = load_jsonish(
        row.get("payment_signature"),
        base_dir,
        f"{row_desc} payment_signature",
        expect_object=False,
        allow_empty=False,
        fallback_to_string=True,
    )
    return {
        "asset_id": asset_id,
        "gross_amount": gross_amount,
        "net_amount": net_amount,
        "settlement_tx": settlement_tx,
        "payer": payer,
        "signature": signature,
    }


def parse_governance(
    value: str | None,
    base_dir: Path,
    row_desc: str,
    *,
    required: bool,
) -> dict[str, Any] | None:
    if not value and not required:
        return None
    document = load_jsonish(
        value,
        base_dir,
        f"{row_desc} governance",
        expect_object=True,
        allow_empty=not required,
    )
    if document is None and required:
        raise BulkOnboardError(f"{row_desc} governance evidence is required")
    return document


def build_request(
    row_index: int,
    row: dict[str, str],
    base_dir: Path,
    options: ScriptOptions,
) -> tuple[dict[str, Any], dict[str, Any]]:
    row_desc = f"row {row_index}"
    label = canonicalize_label(row.get("label", ""), f"{row_desc} label")
    suffix_id = parse_int(
        row.get("suffix_id", ""),
        f"{row_desc} suffix_id",
        minimum=0,
        maximum=0xFFFF,
    )
    owner = (row.get("owner") or "").strip()
    if not owner:
        raise BulkOnboardError(f"{row_desc} owner is required")
    term_years = parse_int(
        row.get("term_years", ""),
        f"{row_desc} term_years",
        minimum=1,
        maximum=255,
    )
    pricing_hint = row.get("pricing_class_hint")
    pricing_class_hint = (
        parse_int(
            pricing_hint,
            f"{row_desc} pricing_class_hint",
            minimum=0,
            maximum=255,
        )
        if pricing_hint and pricing_hint.strip()
        else None
    )
    controller_literals = parse_controllers(
        row.get(options.columns.controllers),
        owner,
        options.default_controller_policy,
        f"{row_desc} controllers",
    )
    controllers = [build_controller_payload(entry) for entry in controller_literals]
    metadata = load_jsonish(
        row.get(options.columns.metadata),
        base_dir,
        f"{row_desc} metadata",
        expect_object=True,
        allow_empty=True,
    )
    governance = parse_governance(
        row.get(options.columns.governance),
        base_dir,
        row_desc,
        required=options.require_governance,
    )
    payment = parse_payment(row, base_dir, row_desc)
    selector = {"version": 1, "suffix_id": suffix_id, "label": label}
    request = {
        "selector": selector,
        "owner": owner,
        "controllers": controllers,
        "term_years": term_years,
        "pricing_class_hint": pricing_class_hint,
        "payment": payment,
        "governance": governance,
        "metadata": metadata,
    }
    counters = {
        "suffix_id": suffix_id,
        "gross_amount": payment["gross_amount"],
        "net_amount": payment["net_amount"],
    }
    return request, counters


def summarize(counter_entries: Iterable[dict[str, Any]]) -> dict[str, Any]:
    total_gross = 0
    total_net = 0
    per_suffix: dict[int, int] = {}
    for entry in counter_entries:
        suffix_id = entry["suffix_id"]
        total_gross += entry["gross_amount"]
        total_net += entry["net_amount"]
        per_suffix[suffix_id] = per_suffix.get(suffix_id, 0) + 1
    return {
        "total_requests": sum(per_suffix.values()),
        "total_gross_amount": total_gross,
        "total_net_amount": total_net,
        "suffix_breakdown": per_suffix,
    }


def write_ndjson(path: Path, requests: Sequence[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for request in requests:
            handle.write(json.dumps(request, separators=(",", ":")))
            handle.write("\n")


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        raise BulkOnboardError(f"manifest {path} is not valid JSON: {err}") from err
    if "requests" not in data or not isinstance(data["requests"], list):
        raise BulkOnboardError("manifest JSON must contain a `requests` array")
    return data


def load_suffix_map(path: Path | None) -> dict[int, str]:
    if path is None:
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        raise BulkOnboardError(f"suffix map {path} is not valid JSON: {err}") from err
    mapping: dict[int, str] = {}
    for key, value in data.items():
        try:
            suffix_id = int(key, 0)
        except ValueError as err:
            raise BulkOnboardError(f"suffix map key `{key}` must be an integer") from err
        if not isinstance(value, str) or not value:
            raise BulkOnboardError(f"suffix map value for `{key}` must be a non-empty string")
        mapping[suffix_id] = value.lstrip(".")
    return mapping


def selector_key(request: dict[str, Any]) -> str:
    selector = request.get("selector", {})
    suffix_id = selector.get("suffix_id", "?")
    label = selector.get("label", "?")
    return f"{suffix_id}:{label}"


def selector_literal(
    request: dict[str, Any],
    suffix_map: dict[int, str],
) -> str | None:
    selector = request.get("selector", {})
    suffix_id = selector.get("suffix_id")
    label = selector.get("label")
    if suffix_id is None or label is None:
        return None
    suffix = suffix_map.get(int(suffix_id))
    if not suffix:
        return None
    return f"{label}.{suffix}"


def stringify_jsonish(value: Any) -> str:
    if isinstance(value, (dict, list, bool, int, float)):
        return json.dumps(value, separators=(",", ":"))
    return "" if value is None else str(value)


def write_submission_log(path: Path | None, entry: dict[str, Any]) -> None:
    if path is None:
        return
    entry = {"timestamp": datetime.now(timezone.utc).isoformat(), **entry}
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(entry, separators=(",", ":")))
        handle.write("\n")


def _parse_json_optional(body: str) -> dict[str, Any] | None:
    if not body:
        return None
    try:
        parsed = json.loads(body)
    except json.JSONDecodeError:
        return None
    if isinstance(parsed, dict):
        return parsed
    return None


def _record_snapshot(payload: dict[str, Any]) -> dict[str, Any]:
    snapshot: dict[str, Any] = {}
    record = payload.get("name_record")
    if isinstance(record, dict):
        target = record
    else:
        target = payload
    if isinstance(target, dict):
        selector = target.get("selector", {})
        if isinstance(selector, dict):
            suffix_id = selector.get("suffix_id")
            label = selector.get("label")
            if suffix_id is not None:
                snapshot["suffix_id"] = suffix_id
            if label is not None:
                snapshot["label"] = label
        for key in ("status", "pricing_class", "owner", "registered_at_ms", "expires_at_ms"):
            if key in target:
                snapshot[f"record_{key}"] = target[key]
    registry_event = payload.get("registry_event")
    if isinstance(registry_event, dict):
        version = registry_event.get("version")
        if version is not None:
            snapshot["registry_event_version"] = version
    return snapshot


class ToriiSubmitter:
    def __init__(
        self,
        url: str,
        token: str,
        timeout: float,
        poll_attempts: int,
        poll_interval: float,
        suffix_map: dict[int, str],
        log_path: Path | None,
        opener: Callable[..., Any] | None = None,
    ) -> None:
        self.url = url.rstrip("/")
        self.token = token
        self.timeout = timeout
        self.poll_attempts = poll_attempts
        self.poll_interval = poll_interval
        self.suffix_map = suffix_map
        self.log_path = log_path
        self._opener = opener or urllib.request.urlopen

    def submit(self, index: int, request_payload: dict[str, Any]) -> None:
        endpoint = f"{self.url}/v1/sns/registrations"
        data = json.dumps(request_payload, separators=(",", ":")).encode("utf-8")
        selector = selector_key(request_payload)
        headers = {
            "Authorization": f"Bearer {self.token}",
            "Content-Type": "application/json",
        }
        req = urllib.request.Request(endpoint, data=data, headers=headers, method="POST")
        try:
            response = self._opener(req, timeout=self.timeout)
            body = response.read().decode("utf-8") if hasattr(response, "read") else ""
            status = getattr(response, "status", getattr(response, "code", 0)) or 0
            success = 200 <= status < 300
            detail = body
            parsed = _parse_json_optional(body)
        except urllib.error.HTTPError as err:
            success = False
            status = err.code
            body = err.read().decode("utf-8", errors="replace")
            detail = body
            parsed = _parse_json_optional(body)
        except urllib.error.URLError as err:
            success = False
            status = 0
            detail = str(err.reason)
            parsed = None
        log_entry = {
            "mode": "torii",
            "index": index,
            "selector": selector,
            "status": status,
            "success": success,
            "detail": detail,
        }
        if parsed:
            log_entry.update(_record_snapshot(parsed))
        write_submission_log(self.log_path, log_entry)
        if not success:
            raise BulkOnboardError(
                f"Torii submission failed for {selector} (status={status}): {detail[:200]}"
            )
        if success and self.poll_attempts > 0 and self.suffix_map:
            literal = selector_literal(request_payload, self.suffix_map)
            if literal:
                self._poll_status(index, selector, literal)

    def _poll_status(self, index: int, selector: str, literal: str) -> None:
        encoded = urllib.parse.quote(literal, safe="")
        endpoint = f"{self.url}/v1/sns/registrations/{encoded}"
        for attempt in range(1, self.poll_attempts + 1):
            req = urllib.request.Request(endpoint, headers={"Authorization": f"Bearer {self.token}"})
            try:
                response = self._opener(req, timeout=self.timeout)
                body = response.read().decode("utf-8") if hasattr(response, "read") else ""
                status = getattr(response, "status", getattr(response, "code", 0)) or 0
                if 200 <= status < 300:
                    log_entry = {
                        "mode": "torii-poll",
                        "index": index,
                        "selector": selector,
                        "status": status,
                        "success": True,
                        "detail": body,
                        "attempt": attempt,
                    }
                    parsed = _parse_json_optional(body)
                    if parsed:
                        log_entry.update(_record_snapshot(parsed))
                    write_submission_log(self.log_path, log_entry)
                    return
                detail = body
                parsed = _parse_json_optional(body)
            except urllib.error.URLError as err:
                status = 0
                detail = str(err.reason)
                parsed = None
            log_entry = {
                "mode": "torii-poll",
                "index": index,
                "selector": selector,
                "status": status,
                "success": False,
                "detail": detail,
                "attempt": attempt,
            }
            if parsed:
                log_entry.update(_record_snapshot(parsed))
            write_submission_log(self.log_path, log_entry)
            time.sleep(self.poll_interval)


class CliSubmitter:
    def __init__(
        self,
        cli_path: str,
        cli_config: str | None,
        extra_args: list[str],
        log_path: Path | None,
        runner: Callable[..., Any] | None = None,
    ) -> None:
        self.cli_path = cli_path
        self.cli_config = cli_config
        self.extra_args = extra_args
        self.log_path = log_path
        self._runner = runner or subprocess.run

    def submit(self, index: int, request_payload: dict[str, Any]) -> None:
        selector = request_payload.get("selector", {})
        label = selector.get("label")
        suffix_id = selector.get("suffix_id")
        if label is None or suffix_id is None:
            raise BulkOnboardError("request is missing selector.label or selector.suffix_id")
        args = [
            self.cli_path,
            "sns",
            "register",
            "--label",
            str(label),
            "--suffix-id",
            str(suffix_id),
            "--term-years",
            str(request_payload.get("term_years", 1)),
        ]
        owner = request_payload.get("owner")
        if owner:
            args.extend(["--owner", str(owner)])
        pricing_hint = request_payload.get("pricing_class_hint")
        if pricing_hint is not None:
            args.extend(["--pricing-class", str(pricing_hint)])
        controllers = request_payload.get("controllers", [])
        for controller in controllers:
            controller_type = controller.get("controller_type", {})
            if controller_type.get("kind") != "Account":
                raise BulkOnboardError(
                    f"controller type `{controller_type}` is not supported for CLI submissions"
                )
            account_address = controller.get("account_address")
            if not account_address:
                raise BulkOnboardError("controller entry missing account_address")
            args.extend(["--controller", str(account_address)])
        payment = request_payload.get("payment")
        if not payment:
            raise BulkOnboardError("request is missing payment information")
        args.extend(
            [
                "--payment-asset-id",
                str(payment.get("asset_id", "")),
                "--payment-gross",
                str(payment.get("gross_amount", 0)),
                "--payment-net",
                str(payment.get("net_amount", 0)),
                "--payment-settlement",
                stringify_jsonish(payment.get("settlement_tx")),
                "--payment-payer",
                str(payment.get("payer", "")),
                "--payment-signature",
                stringify_jsonish(payment.get("signature")),
            ]
        )
        metadata = request_payload.get("metadata") or {}
        governance = request_payload.get("governance")

        if self.cli_config:
            args.extend(["--config", self.cli_config])
        args.extend(self.extra_args)

        with tempfile.TemporaryDirectory() as temp_dir:
            if metadata:
                metadata_path = Path(temp_dir) / "metadata.json"
                metadata_path.write_text(json.dumps(metadata, indent=2), encoding="utf-8")
                args.extend(["--metadata-json", str(metadata_path)])
            if governance:
                governance_path = Path(temp_dir) / "governance.json"
                governance_path.write_text(json.dumps(governance, indent=2), encoding="utf-8")
                args.extend(["--governance-json", str(governance_path)])

            result = self._runner(
                args,
                capture_output=True,
                text=True,
                check=False,
            )
        success = result.returncode == 0
        write_submission_log(
            self.log_path,
            {
                "mode": "cli",
                "index": index,
                "selector": selector_key(request_payload),
                "status": result.returncode,
                "success": success,
                "detail": result.stdout if success else result.stderr,
            },
        )
        if not success:
            raise BulkOnboardError(
                f"iroha CLI returned exit code {result.returncode} for selector {selector_key(request_payload)}"
            )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "SNS bulk onboarding toolkit: convert CSV rows to RegisterNameRequestV1 payloads "
            "and optionally submit them via Torii or the iroha CLI (roadmap SN-3b)."
        )
    )
    parser.add_argument(
        "csv",
        nargs="?",
        help="Input CSV file with SNS name registrations (omit when using --manifest).",
    )
    parser.add_argument(
        "--manifest",
        help="Existing manifest JSON produced by this tool. Skips CSV parsing.",
    )
    parser.add_argument(
        "--output",
        help="Path to write the aggregated JSON payload manifest (build mode only).",
    )
    parser.add_argument(
        "--ndjson",
        help="Optional path to write newline-delimited RegisterNameRequestV1 entries.",
    )
    parser.add_argument(
        "--require-governance",
        action="store_true",
        help="Fail rows that do not specify governance evidence.",
    )
    parser.add_argument(
        "--default-controllers",
        choices=("owner", "none"),
        default="owner",
        help="Controller fallback when the CSV does not specify controllers.",
    )
    parser.add_argument(
        "--controllers-column",
        default="controllers",
        help="CSV column containing controller IH58/hex literals.",
    )
    parser.add_argument(
        "--metadata-column",
        default="metadata",
        help="CSV column containing metadata JSON or @file references.",
    )
    parser.add_argument(
        "--governance-column",
        default="governance",
        help="CSV column with governance hook JSON or @file references.",
    )
    parser.add_argument(
        "--submit-torii-url",
        help="Torii base URL (e.g., https://torii.example.com) for automatic submissions.",
    )
    parser.add_argument(
        "--submit-token",
        help="Bearer token used for Torii submissions. Mutually exclusive with --submit-token-file.",
    )
    parser.add_argument(
        "--submit-token-file",
        help="Path to a file containing the Torii bearer token.",
    )
    parser.add_argument(
        "--submit-timeout",
        type=float,
        default=30.0,
        help="HTTP timeout (seconds) for Torii submissions and polling.",
    )
    parser.add_argument(
        "--poll-status",
        action="store_true",
        help="After each Torii submission, poll `/v1/sns/registrations/{selector}` for confirmation.",
    )
    parser.add_argument(
        "--poll-attempts",
        type=int,
        default=5,
        help="Maximum status polling attempts when --poll-status is enabled.",
    )
    parser.add_argument(
        "--poll-interval",
        type=float,
        default=2.0,
        help="Seconds to wait between polling attempts.",
    )
    parser.add_argument(
        "--suffix-map",
        help="JSON mapping of suffix_id → suffix string used when building selector literals during polling.",
    )
    parser.add_argument(
        "--submission-log",
        help="Path to append NDJSON submission receipts (Torii + CLI).",
    )
    parser.add_argument(
        "--submit-cli-path",
        help="Path to the `iroha` CLI binary for automated `sns register` submissions.",
    )
    parser.add_argument(
        "--submit-cli-config",
        help="Optional CLI config path passed to `iroha --config <path>`.",
    )
    parser.add_argument(
        "--submit-cli-extra-arg",
        action="append",
        default=[],
        help="Additional argument forwarded to the iroha CLI (repeatable).",
    )
    args = parser.parse_args(argv)

    if args.manifest and args.csv:
        print("error: --manifest cannot be combined with a CSV positional argument", file=sys.stderr)
        return 1

    manifest_data: dict[str, Any]
    requests: list[dict[str, Any]]
    csv_source: str | None = None

    if args.manifest:
        manifest_path = Path(args.manifest).expanduser().resolve()
        if not manifest_path.exists():
            print(f"error: manifest file {manifest_path} does not exist", file=sys.stderr)
            return 2
        try:
            manifest_data = load_manifest(manifest_path)
        except BulkOnboardError as err:
            print(f"error: {err}", file=sys.stderr)
            return 1
        requests = manifest_data.get("requests", [])
    else:
        if not args.csv:
            print("error: CSV path is required when --manifest is not supplied", file=sys.stderr)
            return 1
        if not args.output:
            print("error: --output is required when building a manifest from CSV", file=sys.stderr)
            return 1
        csv_path = Path(args.csv).expanduser().resolve()
        if not csv_path.exists():
            print(f"error: CSV file {csv_path} does not exist", file=sys.stderr)
            return 2
        options = ScriptOptions(
            require_governance=args.require_governance,
            default_controller_policy=args.default_controllers,
            columns=ColumnMap(
                controllers=args.controllers_column,
                metadata=args.metadata_column,
                governance=args.governance_column,
            ),
        )
        required_columns = set(DEFAULT_REQUIRED_COLUMNS)
        try:
            with csv_path.open("r", encoding="utf-8") as handle:
                reader = csv.DictReader(handle)
                if reader.fieldnames is None:
                    raise BulkOnboardError("CSV file is empty or missing a header row")
                missing = required_columns.difference(reader.fieldnames)
                if missing:
                    raise BulkOnboardError(
                        f"CSV header missing required columns: {', '.join(sorted(missing))}"
                    )
                requests = []
                counters: list[dict[str, Any]] = []
                for index, row in enumerate(reader, start=2):
                    request, stats = build_request(index, row, csv_path.parent, options)
                    requests.append(request)
                    counters.append(stats)
        except BulkOnboardError as err:
            print(f"error: {err}", file=sys.stderr)
            return 1
        manifest_data = {
            "schema_version": 1,
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "source_csv": str(csv_path),
            "requests": requests,
            "summary": summarize(counters),
        }
        output_path = Path(args.output).expanduser().resolve()
        output_path.write_text(json.dumps(manifest_data, indent=2) + "\n", encoding="utf-8")
        csv_source = str(csv_path)
        if args.ndjson:
            write_ndjson(Path(args.ndjson).expanduser().resolve(), requests)

    if args.manifest and args.ndjson:
        write_ndjson(Path(args.ndjson).expanduser().resolve(), requests)

    submission_targets: list[Callable[[int, dict[str, Any]], None]] = []
    suffix_map: dict[int, str] = {}
    if args.suffix_map:
        try:
            suffix_map = load_suffix_map(Path(args.suffix_map).expanduser().resolve())
        except BulkOnboardError as err:
            print(f"error: {err}", file=sys.stderr)
            return 1

    log_path = Path(args.submission_log).expanduser().resolve() if args.submission_log else None

    if args.submit_torii_url:
        token = args.submit_token
        if args.submit_token_file:
            token = Path(args.submit_token_file).expanduser().read_text(encoding="utf-8").strip()
        if not token:
            print("error: --submit-token or --submit-token-file is required for Torii submissions", file=sys.stderr)
            return 1
        torii_submitter = ToriiSubmitter(
            url=args.submit_torii_url,
            token=token,
            timeout=args.submit_timeout,
            poll_attempts=args.poll_attempts if args.poll_status else 0,
            poll_interval=args.poll_interval,
            suffix_map=suffix_map,
            log_path=log_path,
        )
        submission_targets.append(torii_submitter.submit)

    if args.submit_cli_path:
        cli_submitter = CliSubmitter(
            cli_path=args.submit_cli_path,
            cli_config=args.submit_cli_config,
            extra_args=args.submit_cli_extra_arg or [],
            log_path=log_path,
        )
        submission_targets.append(cli_submitter.submit)

    if submission_targets:
        for index, request in enumerate(requests, start=1):
            for target in submission_targets:
                try:
                    target(index, request)
                except BulkOnboardError as err:
                    print(f"error: submission failed for {selector_key(request)}: {err}", file=sys.stderr)
                    return 1
        if csv_source:
            print(f"Processed {len(requests)} requests from {csv_source}")
        else:
            print(f"Processed {len(requests)} requests from manifest")
    elif not args.manifest:
        print(f"Wrote manifest with {len(requests)} requests to {args.output}")
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
