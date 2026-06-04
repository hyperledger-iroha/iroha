#!/usr/bin/env python3
"""Verify archived ISO 20022 operator production evidence.

Purpose:
  This offline gate validates operator canary and trust-bundle summaries before
  they are archived as production evidence. It rejects plan-only canaries,
  dry-run child commands, insecure HTTP overrides, default-profile fallbacks,
  synthetic trust DER, record-only signature policy, digest tampering, skipped
  stages, failed stages, and obvious secret leakage.

Prerequisites:
  Python 3.11+. No third party Python packages are required. Canary summaries
  should be produced by ``iso_operator_canary.py`` and trust summaries should be
  produced by ``iso_trust_bundle_verify.py``.

Safety:
  The verifier is read-only. It never contacts provider, Torii, notary, OCSP,
  CRL, or rail endpoints. If ``--receipt`` or ``--receipt-dir`` is supplied it
  invokes the local receipt verifier in read-only mode.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import ipaddress
import json
import re
import subprocess
import sys
import urllib.parse
from pathlib import Path
from typing import Any


EVIDENCE_VERSION = 1
REQUIRE_VERIFIED = "require-verified"
REQUIRED_CANARY_STAGES = {"rail", "notary", "verify"}
REQUIRED_RECEIPT_KINDS = {"iso-audit-notary", "iso-rail-gateway"}
SUMMARY_DIGEST_FIELD = "summary_sha256"
SCRIPT_DIR = Path(__file__).resolve().parent

EXPECTED_STAGE_SCRIPTS = {
    "rail": "iso_rail_gateway_adapter.py",
    "notary": "iso_audit_notary_adapter.py",
    "verify": "iso_operator_receipt_verify.py",
}
EXPECTED_STAGE_FLAGS = {
    "rail": {
        "--allow-default-profile",
        "--allow-insecure-http",
        "--allow-legacy-colr007",
        "--bearer-token-file",
        "--dry-run",
        "--inbox-dir",
        "--max-payload-bytes",
        "--message",
        "--receipt-dir",
        "--response-limit-bytes",
        "--timeout-secs",
        "--torii-base-url",
    },
    "notary": {
        "--all",
        "--allow-insecure-http",
        "--bearer-token-file",
        "--dry-run",
        "--endpoint",
        "--export-dir",
        "--receipt-dir",
        "--response-limit-bytes",
        "--timeout-secs",
    },
    "verify": {
        "--allow-failed",
        "--allow-insecure-http",
        "--allow-legacy-colr007",
        "--receipt",
        "--receipt-dir",
        "--require-source-files",
    },
}
COMMAND_URL_FLAGS = {"--endpoint", "--torii-base-url"}

SECRET_KEY_FRAGMENTS = (
    "authorization",
    "private_key",
    "x-iroha-signature",
)
SECRET_KEY_EXACT = {
    "bearer",
    "bearer_token",
    "secret",
    "token",
}
SECRET_VALUE_PATTERNS = [
    re.compile(r"\bauthorization\s*:", re.IGNORECASE),
    re.compile(r"\bbearer\s+[A-Za-z0-9._~+/=-]+", re.IGNORECASE),
    re.compile(r"\b(?:token|secret|private[_-]?key)\s*[:=]\s*\S+", re.IGNORECASE),
    re.compile(r"\bx-iroha-signature\s*:", re.IGNORECASE),
]


class EvidenceError(RuntimeError):
    """Raised when archived ISO operator evidence is unsafe or malformed."""


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _load_json(path: Path) -> Any:
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except FileNotFoundError as error:
        raise EvidenceError(f"{path} does not exist") from error
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{path} is not valid JSON: {error}") from error


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError(f"duplicate key {key!r} in JSON object")
        result[key] = value
    return result


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} must be a JSON object")
    return value


def _require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise EvidenceError(f"{label} must be a JSON array")
    return value


def _required_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise EvidenceError(f"{label}.{key} must be a non-empty string")
    return raw.strip()


def _required_cli_string(value: str | None, label: str) -> str:
    if value is None or not value.strip():
        raise EvidenceError(f"provide {label}")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise EvidenceError(f"{label} must not contain control characters")
    return value.strip()


def _required_positive_cli_int(value: int | None, label: str) -> int:
    if value is None:
        raise EvidenceError(f"provide {label}")
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise EvidenceError(f"{label} must be a positive integer")
    return value


def _reject_duplicate_paths(paths: list[Path], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(path)
        if key in seen:
            raise EvidenceError(
                f"{label}[{offset}] duplicates {label}[{seen[key]}]: {key}"
            )
        seen[key] = offset


def _reject_duplicate_summary_digests(summaries: list[dict[str, Any]], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, summary in enumerate(summaries):
        digest = summary[SUMMARY_DIGEST_FIELD]
        if digest in seen:
            raise EvidenceError(
                f"{label}[{offset}].{SUMMARY_DIGEST_FIELD} duplicates "
                f"{label}[{seen[digest]}].{SUMMARY_DIGEST_FIELD}: {digest}"
            )
        seen[digest] = offset


def _required_timestamp(value: dict[str, Any], key: str, label: str) -> tuple[str, dt.datetime]:
    raw = _required_string(value, key, label)
    return raw, _parse_timestamp(raw, f"{label}.{key}")


def _reject_stale_timestamp(
    timestamp: dt.datetime,
    *,
    max_age_days: int,
    label: str,
) -> None:
    cutoff = dt.datetime.now(dt.UTC) - dt.timedelta(days=max_age_days)
    if timestamp < cutoff:
        raise EvidenceError(f"{label} is older than the {max_age_days}-day freshness budget")


def _required_bool(value: dict[str, Any], key: str, label: str) -> bool:
    raw = value.get(key)
    if not isinstance(raw, bool):
        raise EvidenceError(f"{label}.{key} must be a boolean")
    return raw


def _required_nonnegative_int(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw < 0:
        raise EvidenceError(f"{label}.{key} must be a non-negative integer")
    return raw


def _is_lower_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _require_summary_digest(summary: dict[str, Any], label: str) -> str:
    expected = summary.get(SUMMARY_DIGEST_FIELD)
    if not _is_lower_sha256(expected):
        raise EvidenceError(f"{label} has missing or non-canonical {SUMMARY_DIGEST_FIELD}")
    body = dict(summary)
    body.pop(SUMMARY_DIGEST_FIELD)
    actual = sha256_hex(_canonical_json_bytes(body))
    if actual != expected:
        raise EvidenceError(
            f"{label} {SUMMARY_DIGEST_FIELD} mismatch: expected {expected}, got {actual}"
        )
    return expected


def _verify_receipt_verifier_summary(
    receipt_obj: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    digest = _require_summary_digest(receipt_obj, label)
    _check_no_secret_material(receipt_obj, label)
    verified_receipts = receipt_obj.get("verified_receipts")
    if (
        isinstance(verified_receipts, bool)
        or not isinstance(verified_receipts, int)
        or verified_receipts <= 0
    ):
        raise EvidenceError(f"{label}.verified_receipts must be positive")
    allow_failed = _required_bool(receipt_obj, "allow_failed", label)
    if allow_failed and not args.allow_failed_receipts:
        raise EvidenceError(f"{label} allowed failed receipts")
    allow_insecure_http = _required_bool(receipt_obj, "allow_insecure_http", label)
    if allow_insecure_http and not args.allow_insecure_http:
        raise EvidenceError(f"{label} allowed insecure HTTP receipts")
    allow_legacy_colr007 = _required_bool(receipt_obj, "allow_legacy_colr007", label)
    if allow_legacy_colr007 and not args.allow_legacy_colr007:
        raise EvidenceError(f"{label} allowed legacy colr.007 receipts")
    require_source_files = _required_bool(
        receipt_obj,
        "require_source_files",
        label,
    )
    if not require_source_files and not args.allow_receipt_source_missing:
        raise EvidenceError(f"{label} did not require receipt source files")

    receipt_kind = _require_list(
        receipt_obj.get("receipt_kind"),
        f"{label}.receipt_kind",
    )
    if not receipt_kind or not all(isinstance(item, str) for item in receipt_kind):
        raise EvidenceError(f"{label}.receipt_kind must contain strings")
    receipt_kind_set = set(receipt_kind)
    if args.allow_partial_canary:
        if not (receipt_kind_set & REQUIRED_RECEIPT_KINDS):
            raise EvidenceError(f"{label} has no rail/notary receipt kinds")
    else:
        missing = sorted(REQUIRED_RECEIPT_KINDS - receipt_kind_set)
        if missing:
            raise EvidenceError(f"{label} is missing receipt kinds: {', '.join(missing)}")

    receipt_entries_raw = _require_list(receipt_obj.get("receipts"), f"{label}.receipts")
    if len(receipt_entries_raw) != verified_receipts:
        raise EvidenceError(f"{label}.receipts length does not match verified_receipts")
    receipt_entries: list[dict[str, Any]] = []
    receipt_entry_kinds: set[str] = set()
    seen_receipt_paths: dict[str, int] = {}
    seen_receipt_digests: dict[str, int] = {}
    for offset, receipt_entry_raw in enumerate(receipt_entries_raw):
        entry_label = f"{label}.receipts[{offset}]"
        receipt_entry = _require_object(receipt_entry_raw, entry_label)
        receipt_path = _required_string(receipt_entry, "path", entry_label)
        if receipt_path in seen_receipt_paths:
            raise EvidenceError(
                f"{entry_label}.path duplicates "
                f"{label}.receipts[{seen_receipt_paths[receipt_path]}].path: {receipt_path}"
            )
        seen_receipt_paths[receipt_path] = offset
        entry_kind = _required_string(receipt_entry, "receipt_kind", entry_label)
        if entry_kind not in REQUIRED_RECEIPT_KINDS:
            raise EvidenceError(f"{entry_label}.receipt_kind is unsupported: {entry_kind!r}")
        receipt_sha256 = receipt_entry.get("receipt_sha256")
        if not _is_lower_sha256(receipt_sha256):
            raise EvidenceError(f"{entry_label}.receipt_sha256 must be a canonical SHA-256")
        if receipt_sha256 in seen_receipt_digests:
            raise EvidenceError(
                f"{entry_label}.receipt_sha256 duplicates "
                f"{label}.receipts[{seen_receipt_digests[receipt_sha256]}].receipt_sha256: "
                f"{receipt_sha256}"
            )
        seen_receipt_digests[receipt_sha256] = offset
        receipt_entry_kinds.add(entry_kind)
        receipt_entries.append(dict(receipt_entry))
    if receipt_kind_set != receipt_entry_kinds:
        raise EvidenceError(f"{label}.receipt_kind does not match receipts[].receipt_kind")

    return {
        "verified_receipts": verified_receipts,
        "receipt_kind": sorted(receipt_kind_set),
        "allow_failed": allow_failed,
        "allow_insecure_http": allow_insecure_http,
        "allow_legacy_colr007": allow_legacy_colr007,
        "require_source_files": require_source_files,
        "receipts": receipt_entries,
        "summary_sha256": digest,
    }


def _reject_secret_string(value: str, label: str) -> None:
    for pattern in SECRET_VALUE_PATTERNS:
        if pattern.search(value):
            raise EvidenceError(f"{label} contains secret-looking material")


def _check_no_secret_material(value: Any, label: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            lowered = str(key).lower()
            if lowered in SECRET_KEY_EXACT or any(
                fragment in lowered for fragment in SECRET_KEY_FRAGMENTS
            ):
                raise EvidenceError(f"{label}.{key} is a forbidden secret-looking field")
            _check_no_secret_material(child, f"{label}.{key}")
    elif isinstance(value, list):
        for offset, child in enumerate(value):
            _check_no_secret_material(child, f"{label}[{offset}]")
    elif isinstance(value, str):
        _reject_secret_string(value, label)


def _command_has_script(command: list[str], script_name: str) -> bool:
    return any(Path(item).name == script_name for item in command)


def _command_has_flag(command: list[str], flag: str) -> bool:
    return any(item == flag or item.startswith(flag + "=") for item in command)


def _command_has_http_url(command: list[str]) -> bool:
    return any(item.startswith("http://") for item in command)


def _check_command_urls(
    command: list[str],
    label: str,
    *,
    allow_insecure_http: bool,
) -> None:
    for offset, item in enumerate(command):
        if item in COMMAND_URL_FLAGS:
            if offset + 1 >= len(command):
                raise EvidenceError(f"{label}.command has {item} without a value")
            _check_clean_http_url(
                command[offset + 1],
                f"{label}.command[{offset + 1}]",
                allow_insecure_http=allow_insecure_http,
            )
            continue
        for flag in COMMAND_URL_FLAGS:
            prefix = flag + "="
            if item.startswith(prefix):
                _check_clean_http_url(
                    item[len(prefix):],
                    f"{label}.command[{offset}]",
                    allow_insecure_http=allow_insecure_http,
                )
                break
        else:
            if item.startswith(("http://", "https://")):
                _check_clean_http_url(
                    item,
                    f"{label}.command[{offset}]",
                    allow_insecure_http=allow_insecure_http,
                )


def _check_redacted_bearer_files(command: list[str], label: str) -> None:
    for offset, item in enumerate(command):
        if item == "--bearer-token-file":
            if offset + 1 >= len(command):
                raise EvidenceError(f"{label} has --bearer-token-file without a value")
            if command[offset + 1] != "<runtime-token-file>":
                raise EvidenceError(f"{label} contains an unredacted bearer-token file path")
            continue
        prefix = "--bearer-token-file="
        if item.startswith(prefix) and item[len(prefix) :] != "<runtime-token-file>":
            raise EvidenceError(f"{label} contains an unredacted bearer-token file path")


def _check_command_policy(
    command: list[str],
    label: str,
    *,
    allow_dry_run: bool,
    allow_insecure_http: bool,
    allow_default_profile: bool,
    allow_failed_receipts: bool,
    allow_legacy_colr007: bool,
) -> None:
    if not command:
        raise EvidenceError(f"{label}.command must not be empty")
    if not all(isinstance(item, str) and item for item in command):
        raise EvidenceError(f"{label}.command must contain non-empty strings")
    _check_redacted_bearer_files(command, label)
    if _command_has_flag(command, "--dry-run") and not allow_dry_run:
        raise EvidenceError(f"{label} used --dry-run")
    if (
        _command_has_flag(command, "--allow-insecure-http") or _command_has_http_url(command)
    ) and not allow_insecure_http:
        raise EvidenceError(f"{label} used insecure HTTP evidence")
    _check_command_urls(command, label, allow_insecure_http=allow_insecure_http)
    if _command_has_flag(command, "--allow-default-profile") and not allow_default_profile:
        raise EvidenceError(f"{label} used --allow-default-profile")
    if _command_has_flag(command, "--allow-failed") and not allow_failed_receipts:
        raise EvidenceError(f"{label} allowed failed receipts")
    if _command_has_flag(command, "--allow-legacy-colr007") and not allow_legacy_colr007:
        raise EvidenceError(f"{label} used --allow-legacy-colr007")


def _check_stage_script(stage_name: str, command: list[str], label: str) -> None:
    expected = EXPECTED_STAGE_SCRIPTS.get(stage_name)
    if expected is None:
        raise EvidenceError(f"{label}.name has unsupported canary stage {stage_name!r}")
    if not _command_has_script(command, expected):
        raise EvidenceError(f"{label}.command does not invoke {expected}")


def _check_stage_command_flags(stage_name: str, command: list[str], label: str) -> None:
    allowed = EXPECTED_STAGE_FLAGS.get(stage_name)
    if allowed is None:
        raise EvidenceError(f"{label}.name has unsupported canary stage {stage_name!r}")
    for offset, item in enumerate(command):
        if not item.startswith("--"):
            continue
        flag = item.split("=", 1)[0]
        if flag not in allowed:
            raise EvidenceError(f"{label}.command[{offset}] uses unsupported flag {flag!r}")


def _verify_receipt_stdout(
    stage: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    if _required_bool(stage, "stdout_truncated", label):
        raise EvidenceError(f"{label}.stdout_preview is truncated")
    stdout = stage.get("stdout_preview")
    if not isinstance(stdout, str) or not stdout.strip():
        raise EvidenceError(f"{label}.stdout_preview must contain receipt verifier JSON")
    try:
        receipt_summary = json.loads(
            stdout,
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{label}.stdout_preview is not valid receipt verifier JSON") from error
    receipt_obj = _require_object(receipt_summary, f"{label}.stdout_preview")
    return _verify_receipt_verifier_summary(receipt_obj, f"{label}.stdout_preview", args)


def _stage_summary(
    stage: dict[str, Any],
    label: str,
    args: argparse.Namespace,
    *,
    canary_started_at: dt.datetime,
    canary_finished_at: dt.datetime,
) -> dict[str, Any]:
    name = _required_string(stage, "name", label)
    started_at_raw, started_at = _required_timestamp(stage, "started_at", label)
    finished_at_raw, finished_at = _required_timestamp(stage, "finished_at", label)
    if finished_at < started_at:
        raise EvidenceError(f"{label}.finished_at must not be before started_at")
    if started_at < canary_started_at or finished_at > canary_finished_at:
        raise EvidenceError(f"{label} timestamp window must be inside canary window")
    skipped = _required_bool(stage, "skipped", label)
    if skipped:
        raise EvidenceError(f"{label} was skipped")
    returncode = stage.get("returncode")
    if isinstance(returncode, bool) or not isinstance(returncode, int):
        raise EvidenceError(f"{label}.returncode must be an integer")
    if returncode != 0:
        raise EvidenceError(f"{label} failed with returncode {returncode}")
    command = _require_list(stage.get("command"), f"{label}.command")
    if not all(isinstance(item, str) for item in command):
        raise EvidenceError(f"{label}.command must contain strings")
    _check_command_policy(
        command,
        label,
        allow_dry_run=args.allow_dry_run,
        allow_insecure_http=args.allow_insecure_http,
        allow_default_profile=args.allow_default_profile,
        allow_failed_receipts=args.allow_failed_receipts,
        allow_legacy_colr007=args.allow_legacy_colr007,
    )
    _check_stage_script(name, command, label)
    _check_stage_command_flags(name, command, label)
    if name in {"rail", "notary"}:
        receipt_dir = stage.get("receipt_dir")
        if not isinstance(receipt_dir, str) or not receipt_dir.strip():
            raise EvidenceError(f"{label}.receipt_dir must be recorded")
    if name == "verify":
        result = {
            "name": name,
            "_started_at": started_at,
            "_finished_at": finished_at,
            "started_at": started_at_raw,
            "finished_at": finished_at_raw,
        }
        if (
            not _command_has_flag(command, "--require-source-files")
            and not args.allow_receipt_source_missing
        ):
            raise EvidenceError(f"{label} did not require receipt source files")
        result["receipt_summary"] = _verify_receipt_stdout(stage, label, args)
        return result
    return {
        "name": name,
        "_started_at": started_at,
        "_finished_at": finished_at,
        "started_at": started_at_raw,
        "finished_at": finished_at_raw,
    }


def _planned_stage_summary(
    stage: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> str:
    name = _required_string(stage, "name", label)
    dry_run = _required_bool(stage, "dry_run", label)
    if dry_run and not args.allow_dry_run:
        raise EvidenceError(f"{label} planned a dry-run stage")
    command = _require_list(stage.get("command"), f"{label}.command")
    if not all(isinstance(item, str) for item in command):
        raise EvidenceError(f"{label}.command must contain strings")
    _check_command_policy(
        command,
        label,
        allow_dry_run=args.allow_dry_run,
        allow_insecure_http=args.allow_insecure_http,
        allow_default_profile=args.allow_default_profile,
        allow_failed_receipts=args.allow_failed_receipts,
        allow_legacy_colr007=args.allow_legacy_colr007,
    )
    _check_stage_script(name, command, label)
    _check_stage_command_flags(name, command, label)
    return name


def verify_canary_summary(path: Path, args: argparse.Namespace) -> dict[str, Any]:
    """Verify one archived canary summary and return compact evidence metadata."""

    summary = _require_object(_load_json(path), str(path))
    digest = _require_summary_digest(summary, str(path))
    _check_no_secret_material(summary)

    provider = _required_string(summary, "provider", str(path))
    environment = _required_string(summary, "environment", str(path))
    started_at_raw, started_at = _required_timestamp(summary, "started_at", str(path))
    finished_at_raw, finished_at = _required_timestamp(summary, "finished_at", str(path))
    if finished_at < started_at:
        raise EvidenceError(f"{path}.finished_at must not be before started_at")
    _reject_stale_timestamp(
        finished_at,
        max_age_days=args.max_canary_age_days,
        label=f"{path}.finished_at",
    )
    if args.provider is not None and provider != args.provider:
        raise EvidenceError(f"{path}.provider is {provider!r}, expected {args.provider!r}")
    if args.environment is not None and environment != args.environment:
        raise EvidenceError(
            f"{path}.environment is {environment!r}, expected {args.environment!r}"
        )

    ok = _required_bool(summary, "ok", str(path))
    if not ok:
        raise EvidenceError(f"{path} is not an ok canary summary")
    plan_only = _required_bool(summary, "plan_only", str(path))
    if plan_only and not args.allow_plan_only:
        raise EvidenceError(f"{path} is plan-only evidence")
    policy = _require_object(summary.get("policy"), f"{path}.policy")
    require_explicit_policy = _required_bool(
        policy,
        "require_explicit_policy",
        f"{path}.policy",
    )
    if not require_explicit_policy:
        raise EvidenceError(f"{path} was not produced with --require-explicit-policy")

    stage_results: list[dict[str, Any]] = []
    if plan_only:
        stages = _require_list(summary.get("planned_stages"), f"{path}.planned_stages")
        stage_names = [
            _planned_stage_summary(
                _require_object(stage, f"{path}.planned_stages[{offset}]"),
                f"{path}.planned_stages[{offset}]",
                args,
            )
            for offset, stage in enumerate(stages)
        ]
    else:
        stages = _require_list(summary.get("stages"), f"{path}.stages")
        stage_results = [
            _stage_summary(
                _require_object(stage, f"{path}.stages[{offset}]"),
                f"{path}.stages[{offset}]",
                args,
                canary_started_at=started_at,
                canary_finished_at=finished_at,
            )
            for offset, stage in enumerate(stages)
        ]
        stage_names = [stage["name"] for stage in stage_results]

    if len(stage_names) != len(set(stage_names)):
        raise EvidenceError(f"{path} contains duplicate canary stages")
    previous_finished_at: dt.datetime | None = None
    for offset, stage in enumerate(stage_results):
        if previous_finished_at is not None and stage["_started_at"] < previous_finished_at:
            raise EvidenceError(
                f"{path}.stages[{offset}].started_at must not be before previous stage finished_at"
            )
        previous_finished_at = stage["_finished_at"]
    stage_name_set = set(stage_names)
    if args.allow_partial_canary:
        if "verify" not in stage_name_set:
            raise EvidenceError(f"{path} is missing verify stage")
        if not ({"rail", "notary"} & stage_name_set):
            raise EvidenceError(f"{path} must include rail or notary stage")
    else:
        missing = sorted(REQUIRED_CANARY_STAGES - stage_name_set)
        if missing:
            raise EvidenceError(
                f"{path} is missing required canary stages: {', '.join(missing)}"
            )
    receipt_summary = next(
        (
            stage["receipt_summary"]
            for stage in stage_results
            if stage["name"] == "verify" and "receipt_summary" in stage
        ),
        None,
    )

    return {
        "path": str(path),
        "provider": provider,
        "environment": environment,
        "started_at": started_at_raw,
        "finished_at": finished_at_raw,
        "plan_only": plan_only,
        "require_explicit_policy": require_explicit_policy,
        "stage_names": stage_names,
        "stage_windows": [
            {
                "name": stage["name"],
                "started_at": stage["started_at"],
                "finished_at": stage["finished_at"],
            }
            for stage in stage_results
        ],
        "receipt_summary": receipt_summary,
        "summary_sha256": digest,
    }


def _check_clean_http_url(
    url: str,
    label: str,
    *,
    allow_insecure_http: bool,
    reject_local_hosts: bool = False,
) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in url):
        raise EvidenceError(f"{label} must not contain control characters")
    try:
        parsed = urllib.parse.urlparse(url)
        hostname = parsed.hostname
    except ValueError as error:
        raise EvidenceError(f"{label} is not a valid URL: {error}") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise EvidenceError(f"{label} uses insecure HTTP URL")
        raise EvidenceError(f"{label} must use HTTPS URL")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise EvidenceError(f"{label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise EvidenceError(f"{label} must not contain credentials")
    if parsed.params or parsed.query or parsed.fragment:
        raise EvidenceError(f"{label} must not contain params, query, or fragment")
    hostname = hostname.strip().lower()
    if reject_local_hosts and not allow_insecure_http:
        if hostname == "localhost" or hostname.endswith(".localhost"):
            raise EvidenceError(f"{label} must not use localhost")
        try:
            address = ipaddress.ip_address(hostname)
        except ValueError:
            return
        if not address.is_global:
            raise EvidenceError(f"{label} must not use local, private, or reserved IP addresses")


def _check_https_url(url: str, label: str, *, allow_insecure_http: bool) -> None:
    _check_clean_http_url(
        url,
        label,
        allow_insecure_http=allow_insecure_http,
        reject_local_hosts=True,
    )


def _parse_timestamp(value: Any, label: str) -> dt.datetime:
    if not isinstance(value, str):
        raise EvidenceError(f"{label} must be recorded")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise EvidenceError(f"{label} must not contain control characters")
    text = value.strip()
    if not text:
        raise EvidenceError(f"{label} must be recorded")
    normalized = text[:-1] + "+00:00" if text.endswith("Z") else text
    try:
        parsed = dt.datetime.fromisoformat(normalized)
    except ValueError as error:
        raise EvidenceError(f"{label} must be an ISO 8601 timestamp") from error
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise EvidenceError(f"{label} must include a timezone offset")
    parsed_utc = parsed.astimezone(dt.UTC)
    if parsed_utc > dt.datetime.now(dt.UTC):
        raise EvidenceError(f"{label} must not be in the future")
    return parsed_utc


def _check_retrieved_at(value: str, label: str, args: argparse.Namespace) -> None:
    retrieved_at = _parse_timestamp(value, label)
    _reject_stale_timestamp(
        retrieved_at,
        max_age_days=args.max_trust_source_age_days,
        label=label,
    )


def _check_trust_bundle(
    bundle: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    profile_id = _required_string(bundle, "profile_id", label)
    rail = _required_string(bundle, "rail", label)
    environment = _required_string(bundle, "environment", label)
    if args.environment is not None and environment != args.environment:
        raise EvidenceError(
            f"{label}.environment is {environment!r}, expected {args.environment!r}"
        )
    policy = _required_string(bundle, "embedded_signature_policy", label)
    if policy != REQUIRE_VERIFIED and not args.allow_record_only_trust:
        raise EvidenceError(f"{label}.embedded_signature_policy is {policy!r}")

    source = bundle.get("source")
    if source is None:
        if not args.allow_missing_trust_source:
            raise EvidenceError(f"{label}.source is required for production evidence")
    else:
        source_obj = _require_object(source, f"{label}.source")
        url = source_obj.get("url")
        if not isinstance(url, str) or not url.strip():
            raise EvidenceError(f"{label}.source.url must be recorded")
        _check_https_url(
            url.strip(),
            f"{label}.source.url",
            allow_insecure_http=args.allow_insecure_http,
        )
        retrieved_at = source_obj.get("retrieved_at")
        if not isinstance(retrieved_at, str):
            raise EvidenceError(f"{label}.source.retrieved_at must be recorded")
        _check_retrieved_at(
            retrieved_at,
            f"{label}.source.retrieved_at",
            args,
        )

    material = _require_object(bundle.get("material"), f"{label}.material")
    signature_pin_count = _required_nonnegative_int(
        material,
        "signature_public_key_pin_count",
        f"{label}.material",
    )
    x509_anchor_pin_count = _required_nonnegative_int(
        material,
        "x509_trust_anchor_pin_count",
        f"{label}.material",
    )
    if signature_pin_count + x509_anchor_pin_count == 0:
        raise EvidenceError(f"{label} has no signature public-key or X.509 trust pins")

    profile_overrides = _require_object(
        bundle.get("profile_overrides"),
        f"{label}.profile_overrides",
    )
    crl_required = _required_bool(
        profile_overrides,
        "x509_require_crl_revocation_check",
        f"{label}.profile_overrides",
    )
    ocsp_required = _required_bool(
        profile_overrides,
        "x509_require_ocsp_revocation_check",
        f"{label}.profile_overrides",
    )
    x509_crl_count = _required_nonnegative_int(
        material,
        "x509_crl_count",
        f"{label}.material",
    )
    x509_ocsp_response_count = _required_nonnegative_int(
        material,
        "x509_ocsp_response_count",
        f"{label}.material",
    )
    if crl_required and x509_crl_count == 0:
        raise EvidenceError(f"{label} requires CRL revocation checking but has no CRLs")
    if ocsp_required and x509_ocsp_response_count == 0:
        raise EvidenceError(f"{label} requires OCSP revocation checking but has no OCSP responses")

    return {
        "profile_id": profile_id,
        "rail": rail,
        "environment": environment,
        "embedded_signature_policy": policy,
        "signature_public_key_pin_count": signature_pin_count,
        "x509_trust_anchor_pin_count": x509_anchor_pin_count,
        "x509_require_crl_revocation_check": crl_required,
        "x509_crl_count": x509_crl_count,
        "x509_require_ocsp_revocation_check": ocsp_required,
        "x509_ocsp_response_count": x509_ocsp_response_count,
    }


def verify_trust_summary(path: Path, args: argparse.Namespace) -> dict[str, Any]:
    """Verify one archived trust-bundle summary and return compact metadata."""

    summary = _require_object(_load_json(path), str(path))
    digest = _require_summary_digest(summary, str(path))
    _check_no_secret_material(summary)
    verified_at_raw, verified_at = _required_timestamp(summary, "verified_at", str(path))
    _reject_stale_timestamp(
        verified_at,
        max_age_days=args.max_trust_age_days,
        label=f"{path}.verified_at",
    )

    allow_synthetic_der = _required_bool(summary, "allow_synthetic_der", str(path))
    allow_record_only = _required_bool(summary, "allow_record_only", str(path))
    allow_insecure_source_url = _required_bool(summary, "allow_insecure_source_url", str(path))
    profile_json_emittable = _required_bool(summary, "profile_json_emittable", str(path))
    if allow_synthetic_der and not args.allow_synthetic_trust:
        raise EvidenceError(f"{path} was verified with --allow-synthetic-der")
    if allow_record_only and not args.allow_record_only_trust:
        raise EvidenceError(f"{path} was verified with --allow-record-only")
    if allow_insecure_source_url and not args.allow_insecure_http:
        raise EvidenceError(f"{path} was verified with --allow-insecure-source-url")
    if not profile_json_emittable and not args.allow_synthetic_trust:
        raise EvidenceError(f"{path} cannot emit production profile JSON")

    verified_bundles = summary.get("verified_bundles")
    if isinstance(verified_bundles, bool) or not isinstance(verified_bundles, int) or verified_bundles <= 0:
        raise EvidenceError(f"{path}.verified_bundles must be a positive integer")
    bundles = _require_list(summary.get("bundles"), f"{path}.bundles")
    if len(bundles) != verified_bundles:
        raise EvidenceError(f"{path}.bundles length does not match verified_bundles")
    bundle_summaries = [
        _check_trust_bundle(
            _require_object(bundle, f"{path}.bundles[{offset}]"),
            f"{path}.bundles[{offset}]",
            args,
        )
        for offset, bundle in enumerate(bundles)
    ]
    seen_profile_ids: dict[str, int] = {}
    for offset, bundle in enumerate(bundle_summaries):
        profile_id = bundle["profile_id"]
        if profile_id in seen_profile_ids:
            raise EvidenceError(
                f"{path}.bundles[{offset}].profile_id duplicates "
                f"{path}.bundles[{seen_profile_ids[profile_id]}].profile_id: {profile_id}"
            )
        seen_profile_ids[profile_id] = offset
    return {
        "path": str(path),
        "verified_at": verified_at_raw,
        "verified_bundles": verified_bundles,
        "profiles": bundle_summaries,
        "summary_sha256": digest,
    }


def verify_receipts(args: argparse.Namespace) -> dict[str, Any] | None:
    """Optionally invoke the existing receipt verifier in read-only mode."""

    if not args.receipt and not args.receipt_dir:
        return None
    command = [sys.executable, str(SCRIPT_DIR / "iso_operator_receipt_verify.py")]
    for receipt in args.receipt:
        command.extend(["--receipt", str(receipt)])
    for receipt_dir in args.receipt_dir:
        command.extend(["--receipt-dir", str(receipt_dir)])
    if args.allow_failed_receipts:
        command.append("--allow-failed")
    if args.allow_insecure_http:
        command.append("--allow-insecure-http")
    if args.allow_legacy_colr007:
        command.append("--allow-legacy-colr007")
    if not args.allow_receipt_source_missing:
        command.append("--require-source-files")
    completed = subprocess.run(
        command,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise EvidenceError(
            "receipt verification failed: "
            + completed.stderr.strip()[:4096]
        )
    try:
        receipt_summary = json.loads(
            completed.stdout,
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError("receipt verifier emitted invalid JSON") from error
    receipt_obj = _require_object(receipt_summary, "receipt verifier summary")
    return _verify_receipt_verifier_summary(receipt_obj, "receipt verifier summary", args)


def run(args: argparse.Namespace) -> int:
    if not args.canary_summary:
        raise EvidenceError("provide at least one --canary-summary")
    if not args.trust_summary:
        raise EvidenceError("provide at least one --trust-summary")
    args.provider = _required_cli_string(args.provider, "--provider")
    args.environment = _required_cli_string(args.environment, "--environment")
    args.max_canary_age_days = _required_positive_cli_int(
        args.max_canary_age_days,
        "--max-canary-age-days",
    )
    args.max_trust_age_days = _required_positive_cli_int(
        args.max_trust_age_days,
        "--max-trust-age-days",
    )
    args.max_trust_source_age_days = _required_positive_cli_int(
        args.max_trust_source_age_days,
        "--max-trust-source-age-days",
    )

    canary_paths = [path.resolve() for path in args.canary_summary]
    trust_paths = [path.resolve() for path in args.trust_summary]
    _reject_duplicate_paths(canary_paths, "--canary-summary")
    _reject_duplicate_paths(trust_paths, "--trust-summary")

    canaries = [verify_canary_summary(path, args) for path in canary_paths]
    trusts = [verify_trust_summary(path, args) for path in trust_paths]
    _reject_duplicate_summary_digests(canaries, "canary_summaries")
    _reject_duplicate_summary_digests(trusts, "trust_summaries")
    receipt_summary = verify_receipts(args)

    output: dict[str, Any] = {
        "version": EVIDENCE_VERSION,
        "verified_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "ok": True,
        "canary_summaries": canaries,
        "trust_summaries": trusts,
        "receipt_verification": receipt_summary,
        "policy": {
            "provider": args.provider,
            "environment": args.environment,
            "allow_plan_only": args.allow_plan_only,
            "allow_dry_run": args.allow_dry_run,
            "allow_insecure_http": args.allow_insecure_http,
            "allow_legacy_colr007": args.allow_legacy_colr007,
            "allow_default_profile": args.allow_default_profile,
            "allow_failed_receipts": args.allow_failed_receipts,
            "allow_partial_canary": args.allow_partial_canary,
            "allow_receipt_source_missing": args.allow_receipt_source_missing,
            "allow_record_only_trust": args.allow_record_only_trust,
            "allow_synthetic_trust": args.allow_synthetic_trust,
            "allow_missing_trust_source": args.allow_missing_trust_source,
            "max_canary_age_days": args.max_canary_age_days,
            "max_trust_age_days": args.max_trust_age_days,
            "max_trust_source_age_days": args.max_trust_source_age_days,
        },
    }
    output[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(output))
    text = json.dumps(output, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        args.summary_out.parent.mkdir(parents=True, exist_ok=True)
        args.summary_out.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify archived ISO 20022 operator canary and trust evidence."
    )
    parser.add_argument(
        "--canary-summary",
        action="append",
        default=[],
        type=Path,
        help="Canary summary JSON produced by iso_operator_canary.py; repeatable.",
    )
    parser.add_argument(
        "--trust-summary",
        action="append",
        default=[],
        type=Path,
        help="Trust summary JSON produced by iso_trust_bundle_verify.py; repeatable.",
    )
    parser.add_argument(
        "--receipt",
        action="append",
        default=[],
        type=Path,
        help="Optional receipt JSON file to re-verify; repeatable.",
    )
    parser.add_argument(
        "--receipt-dir",
        action="append",
        default=[],
        type=Path,
        help="Optional directory of *.receipt.json files to re-verify; repeatable.",
    )
    parser.add_argument(
        "--provider",
        help="Expected canary provider value.",
    )
    parser.add_argument(
        "--environment",
        help="Expected canary and trust environment value.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path to write the evidence verification summary JSON.",
    )
    parser.add_argument(
        "--max-canary-age-days",
        type=int,
        help="Maximum age in days for canary finished_at timestamps.",
    )
    parser.add_argument(
        "--max-trust-age-days",
        type=int,
        help="Maximum age in days for trust-bundle summary verified_at timestamps.",
    )
    parser.add_argument(
        "--max-trust-source-age-days",
        type=int,
        help="Maximum age in days for trust source retrieved_at timestamps.",
    )
    parser.add_argument(
        "--allow-plan-only",
        action="store_true",
        help="Allow plan-only canary summaries for local dry audits.",
    )
    parser.add_argument(
        "--allow-dry-run",
        action="store_true",
        help="Allow child canary commands that include --dry-run.",
    )
    parser.add_argument(
        "--allow-insecure-http",
        action="store_true",
        help="Allow http:// URLs and insecure HTTP overrides for local tests.",
    )
    parser.add_argument(
        "--allow-default-profile",
        action="store_true",
        help="Allow rail gateway canaries that use --allow-default-profile.",
    )
    parser.add_argument(
        "--allow-failed-receipts",
        action="store_true",
        help="Allow receipt verifier runs configured with --allow-failed.",
    )
    parser.add_argument(
        "--allow-legacy-colr007",
        action="store_true",
        help="Allow local diagnostic evidence that used legacy colr.007 rail receipts.",
    )
    parser.add_argument(
        "--allow-partial-canary",
        action="store_true",
        help="Allow canaries with only rail or only notary plus verify.",
    )
    parser.add_argument(
        "--allow-receipt-source-missing",
        action="store_true",
        help="Do not require receipt verifier commands to use --require-source-files.",
    )
    parser.add_argument(
        "--allow-record-only-trust",
        action="store_true",
        help="Allow trust summaries produced with record-only signature policy.",
    )
    parser.add_argument(
        "--allow-synthetic-trust",
        action="store_true",
        help="Allow trust summaries produced with --allow-synthetic-der.",
    )
    parser.add_argument(
        "--allow-missing-trust-source",
        action="store_true",
        help="Allow trust bundle summaries without provenance source metadata.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except EvidenceError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
