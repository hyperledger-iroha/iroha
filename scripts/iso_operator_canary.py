#!/usr/bin/env python3
"""Run a provider ISO 20022 operator canary from a checked JSON runbook.

Purpose:
  This operator-side runner ties together the live rail file-drop adapter,
  audit-notary adapter, and receipt verifier. It executes the same CLI scripts
  operators run manually, captures bounded stage output, and emits a single
  JSON summary that can be archived by CI or an operations runbook.

Prerequisites:
  Python 3.11+. No third party Python packages are required. The configured
  rail inbox, audit export directory, endpoints, and optional bearer-token files
  must already exist.

Safety:
  The runner never deletes inputs and never mutates repository files unless
  ``--summary-out`` points at a file to write. Plain HTTP remains disabled by
  default in the underlying adapters and verifier. Bearer-token file paths are
  passed through to child scripts, but token contents are never read or persisted
  by this runner.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import subprocess
import sys
import urllib.parse
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_OUTPUT_LIMIT_BYTES = 64 * 1024
SCRIPT_DIR = Path(__file__).resolve().parent

TOP_LEVEL_KEYS = {"provider", "environment", "rail", "notary", "verify"}
RAIL_KEYS = {
    "inbox_dir",
    "message",
    "torii_base_url",
    "receipt_dir",
    "dry_run",
    "allow_default_profile",
    "allow_insecure_http",
    "bearer_token_file",
    "max_payload_bytes",
    "timeout_secs",
    "response_limit_bytes",
}
NOTARY_KEYS = {
    "export_dir",
    "endpoints",
    "receipt_dir",
    "all",
    "dry_run",
    "allow_insecure_http",
    "bearer_token_file",
    "timeout_secs",
    "response_limit_bytes",
}
VERIFY_KEYS = {
    "enabled",
    "receipts",
    "receipt_dirs",
    "include_stage_receipts",
    "allow_failed",
    "allow_insecure_http",
    "require_source_files",
    "skip_on_stage_failure",
}


class CanaryError(RuntimeError):
    """Raised when a canary runbook is invalid."""


@dataclass(frozen=True)
class StagePlan:
    """One subprocess stage planned by the canary runner."""

    name: str
    argv: list[str]
    receipt_dir: Path | None = None
    dry_run: bool = False


@dataclass(frozen=True)
class StageResult:
    """Bounded subprocess result for one canary stage."""

    name: str
    returncode: int
    command: list[str]
    stdout_preview: str
    stderr_preview: str
    stdout_truncated: bool
    stderr_truncated: bool
    receipt_dir: str | None
    skipped: bool = False
    reason: str | None = None


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
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise CanaryError(f"{path} does not exist") from error
    except json.JSONDecodeError as error:
        raise CanaryError(f"{path} is not valid JSON: {error}") from error


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise CanaryError(f"{label} must be a JSON object")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise CanaryError(f"{label} contains unknown keys: {', '.join(unknown)}")


def _required_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise CanaryError(f"{label}.{key} must be a non-empty string")
    result = raw.strip()
    _reject_control_chars(result, f"{label}.{key}")
    return result


def _optional_string(value: dict[str, Any], key: str, label: str) -> str | None:
    raw = value.get(key)
    if raw is None:
        return None
    if not isinstance(raw, str) or not raw.strip():
        raise CanaryError(f"{label}.{key} must be a non-empty string when provided")
    result = raw.strip()
    _reject_control_chars(result, f"{label}.{key}")
    return result


def _reject_control_chars(value: str, label: str) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise CanaryError(f"{label} must not contain control characters")


def _optional_bool(
    value: dict[str, Any], key: str, label: str, *, default: bool = False
) -> bool:
    raw = value.get(key, default)
    if not isinstance(raw, bool):
        raise CanaryError(f"{label}.{key} must be a boolean")
    return raw


def _optional_positive_int(
    value: dict[str, Any], key: str, label: str
) -> int | None:
    raw = value.get(key)
    if raw is None:
        return None
    if isinstance(raw, bool) or not isinstance(raw, int) or raw <= 0:
        raise CanaryError(f"{label}.{key} must be a positive integer")
    return raw


def _optional_positive_number(
    value: dict[str, Any], key: str, label: str
) -> float | None:
    raw = value.get(key)
    if raw is None:
        return None
    if isinstance(raw, bool) or not isinstance(raw, (int, float)) or raw <= 0:
        raise CanaryError(f"{label}.{key} must be a positive number")
    return float(raw)


def _string_list(value: dict[str, Any], key: str, label: str) -> list[str]:
    raw = value.get(key, [])
    if not isinstance(raw, list):
        raise CanaryError(f"{label}.{key} must be an array of strings")
    result: list[str] = []
    for offset, item in enumerate(raw):
        if not isinstance(item, str) or not item.strip():
            raise CanaryError(f"{label}.{key}[{offset}] must be a non-empty string")
        value = item.strip()
        _reject_control_chars(value, f"{label}.{key}[{offset}]")
        result.append(value)
    return result


def _path_from_config(config_dir: Path, raw: str, label: str) -> Path:
    path = Path(raw).expanduser()
    if path.is_absolute():
        return path
    resolved = (config_dir / path).resolve()
    if not resolved.is_relative_to(config_dir.resolve()):
        raise CanaryError(f"{label} relative paths must stay under {config_dir.resolve()}")
    return resolved


def _validate_endpoint_url(
    url: str,
    label: str,
    *,
    allow_insecure_http: bool,
) -> None:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "https" and not (parsed.scheme == "http" and allow_insecure_http):
        raise CanaryError(f"{label} must use HTTPS")
    if not parsed.netloc or parsed.hostname is None:
        raise CanaryError(f"{label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise CanaryError(f"{label} must not contain credentials")
    if parsed.params or parsed.query or parsed.fragment:
        raise CanaryError(f"{label} must not contain params, query, or fragment")


def _script(name: str) -> str:
    return str(SCRIPT_DIR / name)


def _append_path(argv: list[str], flag: str, path: Path | None) -> None:
    if path is not None:
        argv.extend([flag, str(path)])


def _append_bool(argv: list[str], flag: str, enabled: bool) -> None:
    if enabled:
        argv.append(flag)


def _append_value(argv: list[str], flag: str, value: object | None) -> None:
    if value is not None:
        argv.extend([flag, str(value)])


def _build_rail_stage(config_dir: Path, raw: Any) -> StagePlan:
    rail = _require_object(raw, "rail")
    _reject_unknown_keys(rail, RAIL_KEYS, "rail")

    inbox_dir = _path_from_config(
        config_dir,
        _required_string(rail, "inbox_dir", "rail"),
        "rail.inbox_dir",
    )
    message = _optional_string(rail, "message", "rail")
    message_path = (
        _path_from_config(config_dir, message, "rail.message")
        if message is not None
        else None
    )
    torii_base_url = _required_string(rail, "torii_base_url", "rail")
    allow_insecure_http = _optional_bool(rail, "allow_insecure_http", "rail")
    _validate_endpoint_url(
        torii_base_url,
        "rail.torii_base_url",
        allow_insecure_http=allow_insecure_http,
    )
    receipt_dir_raw = _optional_string(rail, "receipt_dir", "rail")
    receipt_dir = (
        _path_from_config(config_dir, receipt_dir_raw, "rail.receipt_dir")
        if receipt_dir_raw is not None
        else (inbox_dir / "receipts").resolve()
    )
    bearer_raw = _optional_string(rail, "bearer_token_file", "rail")
    bearer_token_file = (
        _path_from_config(config_dir, bearer_raw, "rail.bearer_token_file")
        if bearer_raw is not None
        else None
    )
    dry_run = _optional_bool(rail, "dry_run", "rail")

    argv = [
        sys.executable,
        _script("iso_rail_gateway_adapter.py"),
        "--inbox-dir",
        str(inbox_dir),
        "--torii-base-url",
        torii_base_url,
        "--receipt-dir",
        str(receipt_dir),
    ]
    _append_path(argv, "--message", message_path)
    _append_bool(argv, "--dry-run", dry_run)
    _append_bool(argv, "--allow-default-profile", _optional_bool(rail, "allow_default_profile", "rail"))
    _append_bool(argv, "--allow-insecure-http", allow_insecure_http)
    _append_path(argv, "--bearer-token-file", bearer_token_file)
    _append_value(argv, "--max-payload-bytes", _optional_positive_int(rail, "max_payload_bytes", "rail"))
    _append_value(argv, "--timeout-secs", _optional_positive_number(rail, "timeout_secs", "rail"))
    _append_value(
        argv,
        "--response-limit-bytes",
        _optional_positive_int(rail, "response_limit_bytes", "rail"),
    )
    return StagePlan("rail", argv, receipt_dir=receipt_dir, dry_run=dry_run)


def _build_notary_stage(config_dir: Path, raw: Any) -> StagePlan:
    notary = _require_object(raw, "notary")
    _reject_unknown_keys(notary, NOTARY_KEYS, "notary")

    export_dir = _path_from_config(
        config_dir,
        _required_string(notary, "export_dir", "notary"),
        "notary.export_dir",
    )
    endpoints = _string_list(notary, "endpoints", "notary")
    dry_run = _optional_bool(notary, "dry_run", "notary")
    if not dry_run and not endpoints:
        raise CanaryError("notary.endpoints must contain at least one endpoint unless dry_run is true")
    allow_insecure_http = _optional_bool(notary, "allow_insecure_http", "notary")
    for offset, endpoint in enumerate(endpoints):
        _validate_endpoint_url(
            endpoint,
            f"notary.endpoints[{offset}]",
            allow_insecure_http=allow_insecure_http,
        )
    receipt_dir_raw = _optional_string(notary, "receipt_dir", "notary")
    receipt_dir = (
        _path_from_config(config_dir, receipt_dir_raw, "notary.receipt_dir")
        if receipt_dir_raw is not None
        else (export_dir / "receipts").resolve()
    )
    bearer_raw = _optional_string(notary, "bearer_token_file", "notary")
    bearer_token_file = (
        _path_from_config(config_dir, bearer_raw, "notary.bearer_token_file")
        if bearer_raw is not None
        else None
    )

    argv = [
        sys.executable,
        _script("iso_audit_notary_adapter.py"),
        "--export-dir",
        str(export_dir),
        "--receipt-dir",
        str(receipt_dir),
    ]
    for endpoint in endpoints:
        argv.extend(["--endpoint", endpoint])
    _append_bool(argv, "--all", _optional_bool(notary, "all", "notary"))
    _append_bool(argv, "--dry-run", dry_run)
    _append_bool(
        argv,
        "--allow-insecure-http",
        allow_insecure_http,
    )
    _append_path(argv, "--bearer-token-file", bearer_token_file)
    _append_value(argv, "--timeout-secs", _optional_positive_number(notary, "timeout_secs", "notary"))
    _append_value(
        argv,
        "--response-limit-bytes",
        _optional_positive_int(notary, "response_limit_bytes", "notary"),
    )
    return StagePlan("notary", argv, receipt_dir=receipt_dir, dry_run=dry_run)


def _build_verify_stage(
    config_dir: Path,
    raw: Any,
    stage_receipt_dirs: list[Path],
    *,
    prior_failure: bool,
) -> StagePlan | None:
    verify = {} if raw is None else _require_object(raw, "verify")
    _reject_unknown_keys(verify, VERIFY_KEYS, "verify")
    if not _optional_bool(verify, "enabled", "verify", default=True):
        return None
    skip_on_failure = _optional_bool(
        verify, "skip_on_stage_failure", "verify", default=True
    )
    if prior_failure and skip_on_failure:
        return StagePlan(
            "verify",
            [],
            receipt_dir=None,
            dry_run=False,
        )

    include_stage_receipts = _optional_bool(
        verify, "include_stage_receipts", "verify", default=True
    )
    receipt_dirs = [
        _path_from_config(config_dir, item, "verify.receipt_dirs")
        for item in _string_list(verify, "receipt_dirs", "verify")
    ]
    if include_stage_receipts:
        receipt_dirs.extend(stage_receipt_dirs)
    receipts = [
        _path_from_config(config_dir, item, "verify.receipts")
        for item in _string_list(verify, "receipts", "verify")
    ]
    if not receipt_dirs and not receipts:
        raise CanaryError("verify requires generated stage receipts or explicit receipts/receipt_dirs")

    argv = [
        sys.executable,
        _script("iso_operator_receipt_verify.py"),
    ]
    for receipt in receipts:
        argv.extend(["--receipt", str(receipt)])
    for receipt_dir in receipt_dirs:
        argv.extend(["--receipt-dir", str(receipt_dir)])
    _append_bool(argv, "--allow-failed", _optional_bool(verify, "allow_failed", "verify"))
    _append_bool(
        argv,
        "--allow-insecure-http",
        _optional_bool(verify, "allow_insecure_http", "verify"),
    )
    _append_bool(
        argv,
        "--require-source-files",
        _optional_bool(verify, "require_source_files", "verify", default=True),
    )
    return StagePlan("verify", argv)


def _limit_text(text: str, limit_bytes: int) -> tuple[str, bool]:
    encoded = text.encode("utf-8", errors="replace")
    if len(encoded) <= limit_bytes:
        return text, False
    limited = encoded[:limit_bytes].decode("utf-8", errors="replace")
    return limited, True


def _redacted_command(argv: list[str]) -> list[str]:
    redacted: list[str] = []
    redact_next = False
    for item in argv:
        if redact_next:
            redacted.append("<runtime-token-file>")
            redact_next = False
            continue
        prefix = "--bearer-token-file="
        if item.startswith(prefix):
            redacted.append(prefix + "<runtime-token-file>")
            continue
        redacted.append(item)
        if item == "--bearer-token-file":
            redact_next = True
    return redacted


def _run_stage(stage: StagePlan, output_limit_bytes: int) -> StageResult:
    completed = subprocess.run(
        stage.argv,
        capture_output=True,
        text=True,
        check=False,
    )
    stdout, stdout_truncated = _limit_text(completed.stdout, output_limit_bytes)
    stderr, stderr_truncated = _limit_text(completed.stderr, output_limit_bytes)
    return StageResult(
        name=stage.name,
        returncode=completed.returncode,
        command=_redacted_command(stage.argv),
        stdout_preview=stdout,
        stderr_preview=stderr,
        stdout_truncated=stdout_truncated,
        stderr_truncated=stderr_truncated,
        receipt_dir=str(stage.receipt_dir) if stage.receipt_dir is not None else None,
    )


def _skipped_verify_result(reason: str) -> StageResult:
    return StageResult(
        name="verify",
        returncode=0,
        command=[],
        stdout_preview="",
        stderr_preview="",
        stdout_truncated=False,
        stderr_truncated=False,
        receipt_dir=None,
        skipped=True,
        reason=reason,
    )


def _result_to_json(result: StageResult) -> dict[str, Any]:
    return {
        "name": result.name,
        "returncode": result.returncode,
        "command": result.command,
        "stdout_preview": result.stdout_preview,
        "stderr_preview": result.stderr_preview,
        "stdout_truncated": result.stdout_truncated,
        "stderr_truncated": result.stderr_truncated,
        "receipt_dir": result.receipt_dir,
        "skipped": result.skipped,
        "reason": result.reason,
    }


def _plan_to_json(stage: StagePlan) -> dict[str, Any]:
    return {
        "name": stage.name,
        "command": _redacted_command(stage.argv),
        "receipt_dir": str(stage.receipt_dir) if stage.receipt_dir is not None else None,
        "dry_run": stage.dry_run,
    }


def build_stage_plans(config_path: Path, config: dict[str, Any]) -> tuple[str, str, list[StagePlan], Any]:
    """Validate a runbook and return provider metadata plus non-verify stages."""

    _reject_unknown_keys(config, TOP_LEVEL_KEYS, "config")
    provider = _required_string(config, "provider", "config")
    environment = _required_string(config, "environment", "config")
    config_dir = config_path.resolve().parent

    stages: list[StagePlan] = []
    if "rail" in config:
        stages.append(_build_rail_stage(config_dir, config["rail"]))
    if "notary" in config:
        stages.append(_build_notary_stage(config_dir, config["notary"]))
    if not stages:
        raise CanaryError("configure at least one of rail or notary")
    return provider, environment, stages, config.get("verify")


def run(args: argparse.Namespace) -> int:
    config_path = args.config.resolve()
    config = _require_object(_load_json(config_path), "config")
    provider, environment, stages, verify_config = build_stage_plans(config_path, config)
    if args.output_limit_bytes <= 0:
        raise CanaryError("--output-limit-bytes must be positive")

    started_at = dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat()
    if args.plan_only:
        stage_receipt_dirs = [
            stage.receipt_dir
            for stage in stages
            if stage.receipt_dir is not None and not stage.dry_run
        ]
        verify_stage = _build_verify_stage(
            config_path.resolve().parent,
            verify_config,
            stage_receipt_dirs,
            prior_failure=False,
        )
        planned_stages = [_plan_to_json(stage) for stage in stages]
        if verify_stage is not None:
            planned_stages.append(_plan_to_json(verify_stage))
        summary: dict[str, Any] = {
            "provider": provider,
            "environment": environment,
            "config_path": str(config_path),
            "started_at": started_at,
            "finished_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
            "ok": True,
            "plan_only": True,
            "planned_stages": planned_stages,
        }
        summary["summary_sha256"] = sha256_hex(_canonical_json_bytes(summary))
        text = json.dumps(summary, indent=2, sort_keys=True) + "\n"
        if args.summary_out is not None:
            args.summary_out.parent.mkdir(parents=True, exist_ok=True)
            args.summary_out.write_text(text, encoding="utf-8")
        print(text, end="")
        return 0

    results: list[StageResult] = []
    stage_receipt_dirs: list[Path] = []
    prior_failure = False
    for stage in stages:
        result = _run_stage(stage, args.output_limit_bytes)
        results.append(result)
        if stage.receipt_dir is not None and not stage.dry_run:
            stage_receipt_dirs.append(stage.receipt_dir)
        if result.returncode != 0:
            prior_failure = True

    verify_stage = _build_verify_stage(
        config_path.resolve().parent,
        verify_config,
        stage_receipt_dirs,
        prior_failure=prior_failure,
    )
    if verify_stage is not None:
        if not verify_stage.argv:
            results.append(_skipped_verify_result("skipped because an earlier stage failed"))
        else:
            verify_result = _run_stage(verify_stage, args.output_limit_bytes)
            results.append(verify_result)
            if verify_result.returncode != 0:
                prior_failure = True

    summary: dict[str, Any] = {
        "provider": provider,
        "environment": environment,
        "config_path": str(config_path),
        "started_at": started_at,
        "finished_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "ok": not prior_failure,
        "plan_only": False,
        "stages": [_result_to_json(result) for result in results],
    }
    summary["summary_sha256"] = sha256_hex(_canonical_json_bytes(summary))
    text = json.dumps(summary, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        args.summary_out.parent.mkdir(parents=True, exist_ok=True)
        args.summary_out.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if summary["ok"] else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run ISO 20022 rail/notary adapters and verify canary receipts."
    )
    parser.add_argument(
        "--config",
        required=True,
        type=Path,
        help="JSON runbook with provider, environment, and rail/notary/verify sections.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path to write the canary summary JSON.",
    )
    parser.add_argument(
        "--plan-only",
        action="store_true",
        help="Validate the runbook and print redacted planned child commands without executing them.",
    )
    parser.add_argument(
        "--output-limit-bytes",
        type=int,
        default=DEFAULT_OUTPUT_LIMIT_BYTES,
        help="Maximum stdout/stderr bytes retained per child stage in the summary.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except CanaryError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
