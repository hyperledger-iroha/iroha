#!/usr/bin/env python3
"""Validate SoraFS hedging and billing fixture manifest wiring."""

from __future__ import annotations

import argparse
import hashlib
import re
import shlex
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_lines,
    emit_checker_exception,
    render_checker_summary,
    resolve_checker_preflight_path,
    validate_checker_summary_output,
    write_checker_summary,
)
from sorafs_evidence_json import (  # noqa: E402
    EvidenceFileTooLargeError,
    load_evidence_json_with_sha256,
    read_evidence_bytes,
)
from sorafs_path_identity import (  # noqa: E402
    error_diagnostic_label,
    path_diagnostic_label,
)


REPO_ROOT = SCRIPT_DIR.parent
DEFAULT_MANIFEST = (
    REPO_ROOT / "fixtures" / "sorafs_manifest" / "hedging" / "fixture_manifest.json"
)
SUMMARY_SCHEMA = "sorafs.hedging_billing.fixture_manifest_check.v1"
EXPECTED_SCHEMA_VERSION = 1
EXPECTED_FAMILY = "sorafs_hedging_billing"
EXPECTED_SCOPE = "generated_bytes_required"
MAX_JSON_BYTES = 1024 * 1024
MAX_NORITO_BYTES = 4 * 1024 * 1024
DEFAULT_VALIDATOR_TIMEOUT_SECONDS = 30
MAX_BASIS_POINTS = 10_000
NAME_RE = re.compile(r"^[a-z0-9_]+_v1$")
HEX_RE = re.compile(r"^[0-9a-f]+$")
U128_DECIMAL_RE = re.compile(r"^(0|[1-9][0-9]*)$")
MAX_U128_DECIMAL = "340282366920938463463374607431768211455"
KIND_FLAGS = {
    "billing-line-item": "--line",
    "billing-statement": "--statement",
    "price-feed": "--feed",
    "reference-price-decision": "--decision",
}
EXPECTED_STATUSES = {"accepted", "rejected"}
HEDGING_FIXTURE_ROOT = Path("fixtures") / "sorafs_manifest" / "hedging"
HEDGING_FIXTURE_MANIFEST_PATH = HEDGING_FIXTURE_ROOT / "fixture_manifest.json"
VALIDATED_NORITO_PATH = "_validated_norito_path"
VALIDATED_JSON_PATH = "_validated_json_path"
JSON_SIDE_CAR_KEYS = {
    "billing-line-item": {
        "direction",
        "kind",
        "line_id_hex",
        "norito_bytes_hex",
        "note",
        "quantity_units",
        "source_id",
        "usd_micros",
        "version",
        "xor_amount_micro",
    },
    "billing-statement": {
        "account_id",
        "account_id_hex",
        "due_at_unix",
        "lines",
        "net_due_usd_micros",
        "net_due_xor_micro",
        "norito_bytes_hex",
        "period_end_unix",
        "period_start_unix",
        "previous_statement_id_hex",
        "reference_price",
        "statement_id_hex",
        "total_credit_usd_micros",
        "total_credit_xor_micro",
        "total_debit_usd_micros",
        "total_debit_xor_micro",
        "version",
    },
    "price-feed": {
        "evidence_digest_hex",
        "feed_id",
        "norito_bytes_hex",
        "observed_at_unix",
        "source",
        "status",
        "version",
        "weight_bps",
        "xor_usd_micros",
    },
    "reference-price-decision": {
        "decision_id_hex",
        "degradation_reasons",
        "degraded",
        "effective_at_unix",
        "feeds",
        "max_divergence_bps",
        "max_feed_age_secs",
        "norito_bytes_hex",
        "version",
        "xor_usd_micros",
    },
}
REQUIRED_FIXTURES = {
    "billing_line_egress_v1",
    "billing_line_incentive_credit_v1",
    "billing_line_storage_v1",
    "billing_statement_v1",
    "line_usd_mismatch_statement_v1",
    "price_feed_primary_v1",
    "price_feed_secondary_v1",
    "reference_price_decision_v1",
    "stale_reference_price_decision_v1",
    "tampered_totals_statement_v1",
}
EXPECTED_NEGATIVE_CASES = {
    "line_usd_mismatch_statement_v1": "line_usd_mismatch",
    "stale_reference_price_decision_v1": "stale_feed",
    "tampered_totals_statement_v1": "totals_mismatch",
}


def _path_label(path: Any) -> str:
    """Return a canonical operator diagnostic label for a path-like value."""

    return path_diagnostic_label(path)


def _error_label(error: BaseException, *, path_label: str | None = None) -> str:
    """Return a canonical operator diagnostic label for an exception."""

    return error_diagnostic_label(error, path_label=path_label)


def main(argv: list[str] | None = None) -> int:
    """Run the fixture manifest checker."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="Path to the hedging fixture manifest JSON.",
    )
    parser.add_argument(
        "--manifest-only",
        action="store_true",
        help="Validate manifest structure without requiring generated fixture files.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path for a JSON summary.",
    )
    parser.add_argument(
        "--validator-bin",
        default="sorafs-validate",
        help="sorafs-validate binary used to verify accepted/rejected outcomes.",
    )
    parser.add_argument(
        "--validator-timeout-seconds",
        type=int,
        default=DEFAULT_VALIDATOR_TIMEOUT_SECONDS,
        help="Timeout for each validator invocation.",
    )
    args = parser.parse_args(argv)

    preflight_errors = validate_fixture_manifest_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    errors: list[str] = []
    manifest_path = args.manifest
    manifest, manifest_sha256 = load_manifest(manifest_path, errors)
    entries = validate_manifest(manifest, errors)
    checked_entries: list[dict[str, Any]] = []
    manifest_valid = not errors

    if not args.manifest_only:
        if manifest_valid:
            validate_generated_inventory(entries, errors)
            for entry in entries:
                checked_entries.append(
                    validate_generated_entry(
                        entry,
                        args.validator_bin,
                        args.validator_timeout_seconds,
                        errors,
                    )
                )
        else:
            checked_entries = not_checked_entries(entries)
    else:
        checked_entries = not_checked_entries(entries)

    status = "ok" if not errors else "blocked"
    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": status,
        "manifest": str(manifest_path),
        "manifest_sha256": manifest_sha256,
        "manifest_only": args.manifest_only,
        "entry_count": len(entries),
        "entries": checked_entries,
        "errors": errors,
    }
    rendered_summary = render_checker_summary(summary)
    summary_errors = write_checker_summary(args.summary_out, rendered_summary)
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2

    if errors:
        sys.stderr.write(rendered_summary)
        return 1
    sys.stdout.write(rendered_summary)
    return 0


def validate_fixture_manifest_preflight(args: argparse.Namespace) -> list[str]:
    """Validate fixture checker output targets before reading the manifest."""

    errors: list[str] = []
    manifest = getattr(args, "manifest", None)
    summary_out = getattr(args, "summary_out", None)
    if summary_out is None:
        return errors
    if not isinstance(summary_out, Path):
        return [f"--summary-out `{_path_label(summary_out)}` must be a path"]
    if not validate_checker_summary_output(summary_out, errors):
        return errors
    manifest_identity = None
    if isinstance(manifest, Path):
        manifest_identity = resolve_checker_preflight_path(
            manifest,
            errors,
            label="--manifest",
        )
    summary_identity = resolve_checker_preflight_path(
        summary_out,
        errors,
        label="--summary-out",
    )
    if (
        manifest_identity is not None
        and summary_identity is not None
        and manifest_identity == summary_identity
    ):
        errors.append(
            "--summary-out `{}` must not be the same path as --manifest `{}`".format(
                _path_label(summary_out), _path_label(manifest)
            )
        )
    return errors


def not_checked_entries(entries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Return summary entries for fixtures skipped before generated-byte checks."""

    return [
        {
            "name": entry.get("name"),
            "kind": entry.get("kind"),
            "expected_status": entry.get("expected_status"),
            "generated": "not_checked",
        }
        for entry in entries
    ]


def inspect_regular_file(
    path: Path,
    label: str,
    errors: list[str],
) -> bool | None:
    """Return whether `path` is a regular file, recording inspection failures."""

    if not isinstance(path, Path):
        errors.append(f"{label} `{_path_label(path)}` must be a path")
        return None
    path_label = _path_label(path)
    try:
        return path.is_file()
    except (OSError, RuntimeError) as error:
        errors.append(
            f"failed to inspect {label} `{path_label}`: "
            f"{_error_label(error, path_label=path_label)}"
        )
        return None


def inspect_directory(
    path: Path,
    label: str,
    errors: list[str],
) -> bool | None:
    """Return whether `path` is a directory, recording inspection failures."""

    if not isinstance(path, Path):
        errors.append(f"{label} `{_path_label(path)}` must be a path")
        return None
    path_label = _path_label(path)
    try:
        return path.is_dir()
    except (OSError, RuntimeError) as error:
        errors.append(
            f"failed to inspect {label} `{path_label}`: "
            f"{_error_label(error, path_label=path_label)}"
        )
        return None


def read_file_bytes(
    path: Path,
    label: str,
    errors: list[str],
    *,
    max_bytes: int,
) -> bytes | None:
    """Read file bytes, recording filesystem failures as checker errors."""

    path_label = _path_label(path)
    try:
        return read_evidence_bytes(path, max_bytes)
    except (OSError, RuntimeError) as error:
        errors.append(
            f"failed to read {label} `{path_label}`: "
            f"{_error_label(error, path_label=path_label)}"
        )
    except EvidenceFileTooLargeError as error:
        errors.append(f"{label} exceeds {error.max_bytes} bytes: {path_label}")
    except ValueError as error:
        errors.append(
            f"failed to read {label} `{path_label}`: "
            f"{_error_label(error, path_label=path_label)}"
        )
    return None


def load_manifest(path: Path, errors: list[str]) -> tuple[dict[str, Any], str | None]:
    """Load a manifest JSON object and digest the same bytes."""

    is_file = inspect_regular_file(path, "manifest", errors)
    if is_file is None:
        return {}, None
    if not is_file:
        errors.append(f"manifest does not exist: {_path_label(path)}")
        return {}, None
    try:
        return load_evidence_json_with_sha256(path, MAX_JSON_BYTES)
    except (OSError, RuntimeError) as error:
        path_label = _path_label(path)
        errors.append(
            f"failed to read manifest `{path_label}`: "
            f"{_error_label(error, path_label=path_label)}"
        )
    except (UnicodeDecodeError, ValueError) as error:
        path_label = _path_label(path)
        errors.append(
            "manifest is not valid bounded JSON object: "
            f"{_error_label(error, path_label=path_label)}"
        )
    return {}, None


def validate_manifest(manifest: dict[str, Any], errors: list[str]) -> list[dict[str, Any]]:
    """Validate manifest structure and return fixture entries."""

    if manifest.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        errors.append(f"schema_version must be {EXPECTED_SCHEMA_VERSION}")
    if manifest.get("fixture_family") != EXPECTED_FAMILY:
        errors.append(f"fixture_family must be `{EXPECTED_FAMILY}`")
    if manifest.get("validation_scope") != EXPECTED_SCOPE:
        errors.append(f"validation_scope must be `{EXPECTED_SCOPE}`")
    generator = manifest.get("generator")
    if generator != "cargo run -p sorafs_manifest --bin generate_hedging_fixtures":
        errors.append("generator must be the checked-in hedging fixture command")

    raw_entries = manifest.get("fixtures")
    if not isinstance(raw_entries, list) or not raw_entries:
        errors.append("fixtures must be a non-empty array")
        return []
    entries: list[dict[str, Any]] = []
    seen_names: set[str] = set()
    for index, raw_entry in enumerate(raw_entries):
        path = f"fixtures[{index}]"
        if not isinstance(raw_entry, dict):
            errors.append(f"{path} must be an object")
            continue
        entry = dict(raw_entry)
        name = require_string(entry, "name", path, errors)
        if name and not NAME_RE.fullmatch(name):
            errors.append(f"{path}.name has invalid shape: {name}")
        if name in seen_names:
            errors.append(f"{path}.name duplicates {name}")
        seen_names.add(name)

        kind = require_string(entry, "kind", path, errors)
        if kind and kind not in KIND_FLAGS:
            errors.append(f"{path}.kind is unsupported: {kind}")
        expected_status = require_string(entry, "expected_status", path, errors)
        if expected_status and expected_status not in EXPECTED_STATUSES:
            errors.append(f"{path}.expected_status is unsupported: {expected_status}")
        negative_case = ""
        if expected_status == "rejected":
            negative_case = require_string(entry, "negative_case", path, errors)
            if not negative_case:
                errors.append(f"{path}.negative_case is required for rejected fixtures")
        if expected_status == "accepted" and "negative_case" in entry:
            errors.append(f"{path}.negative_case must be absent for accepted fixtures")

        norito_path = require_fixture_path(entry, "norito_path", ".to", path, errors)
        json_path = require_fixture_path(entry, "json_path", ".json", path, errors)
        if norito_path is not None:
            entry[VALIDATED_NORITO_PATH] = norito_path
        if json_path is not None:
            entry[VALIDATED_JSON_PATH] = json_path
        if name and norito_path and not norito_path.name == f"{name}.to":
            errors.append(f"{path}.norito_path must end with {name}.to")
        if name and json_path and not json_path.name == f"{name}.json":
            errors.append(f"{path}.json_path must end with {name}.json")
        validate_status_path_contract(
            name,
            expected_status,
            negative_case,
            norito_path,
            json_path,
            path,
            errors,
        )

        command = require_string(entry, "validation_command", path, errors)
        if kind in KIND_FLAGS and norito_path is not None:
            expected_command = (
                f"sorafs-validate hedging {KIND_FLAGS[kind]} {norito_path.as_posix()}"
            )
            if command != expected_command:
                errors.append(f"{path}.validation_command must be `{expected_command}`")
        entries.append(entry)

    missing = REQUIRED_FIXTURES.difference(seen_names)
    extra = seen_names.difference(REQUIRED_FIXTURES)
    if missing:
        errors.append(f"fixtures missing required names: {sorted(missing)}")
    if extra:
        errors.append(f"fixtures include unexpected names: {sorted(extra)}")
    return entries


def validate_status_path_contract(
    name: str,
    expected_status: str,
    negative_case: str,
    norito_path: Path | None,
    json_path: Path | None,
    path: str,
    errors: list[str],
) -> None:
    """Validate accepted/rejected fixture path and negative-case contracts."""

    if not name or expected_status not in EXPECTED_STATUSES:
        return
    expected_negative_case = EXPECTED_NEGATIVE_CASES.get(name)
    is_known_negative = expected_negative_case is not None
    paths = [fixture_path for fixture_path in (norito_path, json_path) if fixture_path]
    under_negative = [
        "negative" in fixture_path.parts[len(HEDGING_FIXTURE_ROOT.parts) :]
        for fixture_path in paths
    ]
    if is_known_negative and expected_status != "rejected":
        errors.append(f"{path}.expected_status must be rejected for negative fixture {name}")
    if not is_known_negative and expected_status == "rejected":
        errors.append(f"{path}.name is not in the reviewed rejected fixture set: {name}")
    if expected_status == "rejected":
        if is_known_negative and negative_case and negative_case != expected_negative_case:
            errors.append(
                f"{path}.negative_case must be `{expected_negative_case}` for {name}"
            )
        if paths and not all(under_negative):
            errors.append(f"{path} rejected fixture paths must both stay under negative/")
    if expected_status == "accepted" and any(under_negative):
        errors.append(f"{path} accepted fixture paths must not use negative/")


def require_string(
    entry: dict[str, Any],
    field: str,
    path: str,
    errors: list[str],
) -> str:
    """Return a non-empty string field or record an error."""

    value = entry.get(field)
    if isinstance(value, str) and value.strip() == value and value:
        return value
    errors.append(f"{path}.{field} must be a non-empty trimmed string")
    return ""


def require_fixture_path(
    entry: dict[str, Any],
    field: str,
    suffix: str,
    path: str,
    errors: list[str],
) -> Path | None:
    """Validate a repository-relative fixture path."""

    raw = require_string(entry, field, path, errors)
    if not raw:
        return None
    return parse_fixture_path(raw, suffix, f"{path}.{field}", errors)


def parse_fixture_path(
    raw: str,
    suffix: str,
    field_path: str,
    errors: list[str],
) -> Path | None:
    """Return a safe repository-relative fixture path, or record an error."""

    fixture_path = Path(raw)
    valid = True
    if fixture_path.is_absolute() or ".." in fixture_path.parts:
        errors.append(
            f"{field_path} must be a repository-relative path without parent segments"
        )
        valid = False
    if not fixture_path.as_posix().startswith(HEDGING_FIXTURE_ROOT.as_posix() + "/"):
        errors.append(f"{field_path} must stay under {HEDGING_FIXTURE_ROOT.as_posix()}")
        valid = False
    if fixture_path.suffix != suffix:
        errors.append(f"{field_path} must end in {suffix}")
        valid = False
    if not valid:
        return None
    return fixture_path


def validate_generated_entry(
    entry: dict[str, Any],
    validator_bin: str,
    validator_timeout_seconds: int,
    errors: list[str],
) -> dict[str, Any]:
    """Validate generated `.to` and `.json` files for one manifest entry."""

    name = entry.get("name")
    norito_rel_path = entry.get(VALIDATED_NORITO_PATH)
    json_rel_path = entry.get(VALIDATED_JSON_PATH)
    result: dict[str, Any] = {
        "name": name,
        "kind": entry.get("kind"),
        "expected_status": entry.get("expected_status"),
        "norito_path": entry.get("norito_path"),
        "json_path": entry.get("json_path"),
        "generated": "checked",
        "validator": "not_checked",
    }
    if not isinstance(norito_rel_path, Path) or not isinstance(json_rel_path, Path):
        result["generated"] = "not_checked"
        return result

    norito_path = REPO_ROOT / norito_rel_path
    json_path = REPO_ROOT / json_rel_path

    norito_bytes = read_generated_bytes(norito_path, MAX_NORITO_BYTES, "Norito", errors)
    json_loaded = load_generated_json_sidecar(json_path, errors)
    if norito_bytes is None or json_loaded is None:
        return result
    json_value, json_sha256 = json_loaded
    result["norito_sha256"] = hashlib.sha256(norito_bytes).hexdigest()
    result["json_sha256"] = json_sha256

    norito_hex = json_value.get("norito_bytes_hex")
    if (
        not isinstance(norito_hex, str)
        or len(norito_hex) % 2 != 0
        or not HEX_RE.fullmatch(norito_hex)
    ):
        errors.append(
            f"{_path_label(json_path)} must contain lowercase even-length hex norito_bytes_hex"
        )
        return result
    try:
        sidecar_norito_bytes = bytes.fromhex(norito_hex)
    except ValueError as error:
        errors.append(
            f"{_path_label(json_path)} norito_bytes_hex is not valid hex: "
            f"{_error_label(error)}"
        )
        return result
    if sidecar_norito_bytes != norito_bytes:
        errors.append(
            f"{_path_label(json_path)} norito_bytes_hex does not match "
            f"{_path_label(norito_path)}"
        )
        return result
    if not validate_json_sidecar(entry, json_value, json_path, errors):
        return result
    validate_expected_status(
        entry,
        norito_rel_path,
        validator_bin,
        validator_timeout_seconds,
        result,
        errors,
    )
    return result


def load_generated_json_sidecar(
    path: Path,
    errors: list[str],
) -> tuple[dict[str, Any], str] | None:
    """Load a generated JSON sidecar and digest the same bytes."""

    file_label = "generated JSON fixture"
    is_file = inspect_regular_file(path, file_label, errors)
    if is_file is None:
        return None
    if not is_file:
        errors.append(f"missing {file_label}: {_path_label(path)}")
        return None
    try:
        return load_evidence_json_with_sha256(path, MAX_JSON_BYTES)
    except (OSError, RuntimeError) as error:
        path_label = _path_label(path)
        errors.append(
            f"failed to read {file_label} `{path_label}`: "
            f"{_error_label(error, path_label=path_label)}"
        )
    except (UnicodeDecodeError, ValueError) as error:
        path_label = _path_label(path)
        errors.append(
            f"{path_label} is not a valid bounded JSON object: "
            f"{_error_label(error, path_label=path_label)}"
        )
    return None


def validate_json_sidecar(
    entry: dict[str, Any],
    json_value: dict[str, Any],
    json_path: Path,
    errors: list[str],
) -> bool:
    """Validate generated JSON sidecar fields for the manifest entry kind."""

    kind = entry.get("kind")
    if kind not in JSON_SIDE_CAR_KEYS:
        return False
    expected_keys = sidecar_keys(kind, include_norito_bytes=True)
    actual_keys = set(json_value)
    missing = sorted(expected_keys.difference(actual_keys))
    extra = sorted(actual_keys.difference(expected_keys))
    if missing:
        errors.append(f"{json_path} missing required JSON sidecar fields: {missing}")
    if extra:
        errors.append(f"{json_path} has unexpected JSON sidecar fields: {extra}")

    valid = not missing and not extra
    if kind == "price-feed":
        valid &= validate_price_feed_sidecar(json_value, json_path, errors)
    elif kind == "reference-price-decision":
        valid &= validate_reference_price_sidecar(json_value, json_path, errors)
    elif kind == "billing-line-item":
        valid &= validate_billing_line_sidecar(json_value, json_path, errors)
    elif kind == "billing-statement":
        valid &= require_json_version(json_value, json_path, errors)
        valid &= require_json_hex(json_value, "statement_id_hex", json_path, errors)
        valid &= require_json_string(json_value, "account_id", json_path, errors)
        valid &= require_json_hex(json_value, "account_id_hex", json_path, errors, length=None)
        valid &= require_json_int(json_value, "period_start_unix", json_path, errors)
        valid &= require_json_int(json_value, "period_end_unix", json_path, errors)
        valid &= require_json_int(json_value, "due_at_unix", json_path, errors)
        valid &= require_statement_window(json_value, json_path, errors)
        if require_json_object(json_value, "reference_price", json_path, errors):
            valid &= validate_nested_object_keys(
                json_value["reference_price"],
                "reference-price-decision",
                f"{json_path} reference_price",
                errors,
            )
            valid &= validate_reference_price_sidecar(
                json_value["reference_price"],
                f"{json_path} reference_price",
                errors,
            )
        else:
            valid = False
        if require_json_non_empty_array(json_value, "lines", json_path, errors):
            valid &= require_unique_nested_strings(
                json_value["lines"],
                "line_id_hex",
                f"{json_path} lines",
                errors,
            )
            for index, line in enumerate(json_value["lines"]):
                label = f"{json_path} lines[{index}]"
                if not isinstance(line, dict):
                    errors.append(f"{label} must be an object")
                    valid = False
                    continue
                valid &= validate_nested_object_keys(
                    line,
                    "billing-line-item",
                    label,
                    errors,
                )
                valid &= validate_billing_line_sidecar(line, label, errors)
        else:
            valid = False
        for field in (
            "total_debit_xor_micro",
            "total_credit_xor_micro",
            "net_due_xor_micro",
            "total_debit_usd_micros",
            "total_credit_usd_micros",
            "net_due_usd_micros",
        ):
            valid &= require_json_decimal_string(json_value, field, json_path, errors)
        previous = json_value.get("previous_statement_id_hex")
        if previous is not None:
            valid &= require_hex_string(previous, "previous_statement_id_hex", json_path, errors)
        valid &= require_account_hex_matches_string(json_value, json_path, errors)
    return valid


def sidecar_keys(kind: str, *, include_norito_bytes: bool) -> set[str]:
    """Return expected sidecar keys for a payload kind."""

    keys = set(JSON_SIDE_CAR_KEYS[kind])
    if not include_norito_bytes:
        keys.discard("norito_bytes_hex")
    return keys


def validate_nested_object_keys(
    value: dict[str, Any],
    kind: str,
    label: str,
    errors: list[str],
) -> bool:
    """Validate the exact key set for a nested generated sidecar object."""

    expected_keys = sidecar_keys(kind, include_norito_bytes=False)
    actual_keys = set(value)
    missing = sorted(expected_keys.difference(actual_keys))
    extra = sorted(actual_keys.difference(expected_keys))
    valid = True
    if missing:
        errors.append(f"{label} missing required JSON sidecar fields: {missing}")
        valid = False
    if extra:
        errors.append(f"{label} has unexpected JSON sidecar fields: {extra}")
        valid = False
    return valid


def validate_price_feed_sidecar(
    json_value: dict[str, Any],
    label: Any,
    errors: list[str],
) -> bool:
    """Validate common price-feed sidecar fields."""

    valid = True
    valid &= require_json_version(json_value, label, errors)
    valid &= require_json_string(json_value, "feed_id", label, errors)
    valid &= require_json_string(json_value, "source", label, errors)
    valid &= require_json_positive_int(json_value, "observed_at_unix", label, errors)
    valid &= require_json_positive_int(json_value, "xor_usd_micros", label, errors)
    valid &= require_json_bps(json_value, "weight_bps", label, errors)
    valid &= require_json_hex(json_value, "evidence_digest_hex", label, errors)
    status = json_value.get("status")
    if status not in {"degraded", "ok", "rejected"}:
        errors.append(f"{label} status must be ok, degraded, or rejected")
        valid = False
    return valid


def validate_reference_price_sidecar(
    json_value: dict[str, Any],
    label: Any,
    errors: list[str],
) -> bool:
    """Validate common reference-price decision sidecar fields."""

    valid = True
    valid &= require_json_version(json_value, label, errors)
    valid &= require_json_hex(json_value, "decision_id_hex", label, errors)
    valid &= require_json_positive_int(json_value, "effective_at_unix", label, errors)
    valid &= require_json_positive_int(json_value, "xor_usd_micros", label, errors)
    valid &= require_json_positive_int(json_value, "max_feed_age_secs", label, errors)
    valid &= require_json_bps(json_value, "max_divergence_bps", label, errors)
    valid &= require_json_bool(json_value, "degraded", label, errors)
    valid &= require_json_array(json_value, "degradation_reasons", label, errors)
    if require_json_non_empty_array(json_value, "feeds", label, errors):
        valid &= require_unique_nested_strings(
            json_value["feeds"],
            "feed_id",
            f"{label} feeds",
            errors,
        )
        for index, feed in enumerate(json_value["feeds"]):
            feed_label = f"{label} feeds[{index}]"
            if not isinstance(feed, dict):
                errors.append(f"{feed_label} must be an object")
                valid = False
                continue
            valid &= validate_nested_object_keys(feed, "price-feed", feed_label, errors)
            valid &= validate_price_feed_sidecar(feed, feed_label, errors)
    else:
        valid = False
    return valid


def validate_billing_line_sidecar(
    json_value: dict[str, Any],
    label: Any,
    errors: list[str],
) -> bool:
    """Validate common billing-line sidecar fields."""

    valid = True
    valid &= require_json_version(json_value, label, errors)
    valid &= require_json_hex(json_value, "line_id_hex", label, errors)
    line_kind = json_value.get("kind")
    if line_kind not in {
        "adjustment",
        "egress",
        "incentive_credit",
        "penalty",
        "reserve_rent",
        "settlement_fee",
        "storage",
    }:
        errors.append(f"{label} kind has invalid billing-line value")
        valid = False
    direction = json_value.get("direction")
    if direction not in {"credit", "debit"}:
        errors.append(f"{label} direction must be debit or credit")
        valid = False
    valid &= require_json_string(json_value, "source_id", label, errors)
    valid &= require_json_positive_decimal_string(
        json_value,
        "xor_amount_micro",
        label,
        errors,
    )
    valid &= require_json_positive_decimal_string(
        json_value,
        "usd_micros",
        label,
        errors,
    )
    valid &= require_json_decimal_string(json_value, "quantity_units", label, errors)
    note = json_value.get("note")
    if note is not None and not isinstance(note, str):
        errors.append(f"{label} note must be a string or null")
        valid = False
    return valid


def require_json_version(
    value: dict[str, Any],
    path: Any,
    errors: list[str],
) -> bool:
    """Require the generated fixture schema version to be V1."""

    if value.get("version") == 1:
        return True
    errors.append(f"{path} version must be 1")
    return False


def require_unique_nested_strings(
    values: list[Any],
    field: str,
    label: str,
    errors: list[str],
) -> bool:
    """Require a nested array to have unique string identifiers."""

    seen: set[str] = set()
    valid = True
    for index, item in enumerate(values):
        if not isinstance(item, dict):
            continue
        field_value = item.get(field)
        if not isinstance(field_value, str):
            continue
        if field_value in seen:
            errors.append(f"{label} duplicate {field}: {field_value}")
            valid = False
        seen.add(field_value)
    return valid


def require_statement_window(
    value: dict[str, Any],
    path: Any,
    errors: list[str],
) -> bool:
    """Require statement period and due timestamps to be ordered."""

    start = value.get("period_start_unix")
    end = value.get("period_end_unix")
    due = value.get("due_at_unix")
    if not all(
        isinstance(item, int) and not isinstance(item, bool)
        for item in (start, end, due)
    ):
        return False
    if start >= end:
        errors.append(f"{path} period_start_unix must be before period_end_unix")
        return False
    if due < end:
        errors.append(f"{path} due_at_unix must be at or after period_end_unix")
        return False
    return True


def require_account_hex_matches_string(
    value: dict[str, Any],
    path: Any,
    errors: list[str],
) -> bool:
    """Require account_id_hex to decode to account_id."""

    account_id = value.get("account_id")
    account_hex = value.get("account_id_hex")
    if not isinstance(account_id, str) or not isinstance(account_hex, str):
        return False
    try:
        decoded = bytes.fromhex(account_hex).decode("utf-8")
    except (UnicodeDecodeError, ValueError):
        errors.append(f"{path} account_id_hex must decode as UTF-8")
        return False
    if decoded != account_id:
        errors.append(f"{path} account_id_hex must decode to account_id")
        return False
    return True


def require_json_string(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require a non-empty string JSON field."""

    field_value = value.get(field)
    if isinstance(field_value, str) and field_value:
        return True
    errors.append(f"{path} {field} must be a non-empty string")
    return False


def require_json_decimal_string(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require a canonical unsigned u128 decimal string JSON field."""

    field_value = value.get(field)
    if isinstance(field_value, str) and is_u128_decimal_string(field_value):
        return True
    errors.append(f"{path} {field} must be an unsigned u128 decimal string")
    return False


def require_json_positive_decimal_string(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require a positive canonical unsigned u128 decimal string JSON field."""

    field_value = value.get(field)
    if (
        isinstance(field_value, str)
        and is_u128_decimal_string(field_value)
        and field_value != "0"
    ):
        return True
    errors.append(f"{path} {field} must be a positive unsigned u128 decimal string")
    return False


def is_u128_decimal_string(value: str) -> bool:
    """Return whether a string is a canonical ASCII u128 decimal value."""

    if not U128_DECIMAL_RE.fullmatch(value):
        return False
    return (
        len(value) < len(MAX_U128_DECIMAL)
        or (len(value) == len(MAX_U128_DECIMAL) and value <= MAX_U128_DECIMAL)
    )


def require_json_int(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require a non-negative integer JSON field."""

    field_value = value.get(field)
    if isinstance(field_value, int) and not isinstance(field_value, bool) and field_value >= 0:
        return True
    errors.append(f"{path} {field} must be a non-negative integer")
    return False


def require_json_positive_int(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require a positive integer JSON field."""

    field_value = value.get(field)
    if isinstance(field_value, int) and not isinstance(field_value, bool) and field_value > 0:
        return True
    errors.append(f"{path} {field} must be a positive integer")
    return False


def require_json_bps(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require an integer basis-point field in 1..=10_000."""

    field_value = value.get(field)
    if (
        isinstance(field_value, int)
        and not isinstance(field_value, bool)
        and 1 <= field_value <= MAX_BASIS_POINTS
    ):
        return True
    errors.append(f"{path} {field} must be an integer in 1..={MAX_BASIS_POINTS}")
    return False


def require_json_bool(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require a boolean JSON field."""

    if isinstance(value.get(field), bool):
        return True
    errors.append(f"{path} {field} must be a boolean")
    return False


def require_json_array(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require an array JSON field."""

    if isinstance(value.get(field), list):
        return True
    errors.append(f"{path} {field} must be an array")
    return False


def require_json_non_empty_array(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require a non-empty array JSON field."""

    field_value = value.get(field)
    if isinstance(field_value, list) and field_value:
        return True
    errors.append(f"{path} {field} must be a non-empty array")
    return False


def require_json_object(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
) -> bool:
    """Require an object JSON field."""

    if isinstance(value.get(field), dict):
        return True
    errors.append(f"{path} {field} must be an object")
    return False


def require_json_hex(
    value: dict[str, Any],
    field: str,
    path: Any,
    errors: list[str],
    *,
    length: int | None = 64,
) -> bool:
    """Require a lowercase hex string JSON field."""

    return require_hex_string(value.get(field), field, path, errors, length=length)


def require_hex_string(
    value: Any,
    field: str,
    path: Any,
    errors: list[str],
    *,
    length: int | None = 64,
) -> bool:
    """Require a lowercase hex string."""

    if isinstance(value, str) and HEX_RE.fullmatch(value) and (
        length is None or len(value) == length
    ):
        return True
    if length is None:
        errors.append(f"{path} {field} must be a lowercase hex string")
    else:
        errors.append(f"{path} {field} must be a lowercase {length}-hex string")
    return False


def validate_generated_inventory(
    entries: list[dict[str, Any]],
    errors: list[str],
) -> None:
    """Reject generated fixture files that are not pinned by the manifest."""

    fixture_root = REPO_ROOT / HEDGING_FIXTURE_ROOT
    root_is_dir = inspect_directory(fixture_root, "generated fixture root", errors)
    if not root_is_dir:
        return
    expected_paths = {
        fixture_path.as_posix()
        for entry in entries
        for fixture_path in (
            entry.get(VALIDATED_NORITO_PATH),
            entry.get(VALIDATED_JSON_PATH),
        )
        if isinstance(fixture_path, Path)
    }
    actual_paths: set[str] = set()
    try:
        for path in fixture_root.rglob("*"):
            is_file = inspect_regular_file(path, "generated fixture candidate", errors)
            if not is_file or path.suffix not in {".json", ".to"}:
                continue
            relative_path = path.relative_to(REPO_ROOT)
            if relative_path == HEDGING_FIXTURE_MANIFEST_PATH:
                continue
            actual_paths.add(relative_path.as_posix())
    except (OSError, RuntimeError) as error:
        fixture_root_label = _path_label(fixture_root)
        errors.append(
            f"failed to scan generated fixture root `{fixture_root_label}`: "
            f"{_error_label(error, path_label=fixture_root_label)}"
        )
        return
    extra_paths = sorted(actual_paths.difference(expected_paths))
    if extra_paths:
        errors.append(f"unmanifested generated hedging fixtures: {extra_paths}")


def validate_expected_status(
    entry: dict[str, Any],
    norito_rel_path: Path,
    validator_bin: str,
    validator_timeout_seconds: int,
    result: dict[str, Any],
    errors: list[str],
) -> None:
    """Run the pinned validator command and compare the expected status."""

    command = entry.get("validation_command")
    kind = entry.get("kind")
    expected_status = entry.get("expected_status")
    name = entry.get("name")
    if not isinstance(command, str) or kind not in KIND_FLAGS:
        return
    flag = KIND_FLAGS[kind]
    expected_tokens = [
        "sorafs-validate",
        "hedging",
        flag,
        norito_rel_path.as_posix(),
    ]
    try:
        command_tokens = shlex.split(command)
    except ValueError as error:
        errors.append(
            f"{name} validation_command is not shell-tokenizable: "
            f"{_error_label(error)}"
        )
        return
    if command_tokens != expected_tokens:
        errors.append(
            f"{name} validation_command tokens must be {expected_tokens}, got {command_tokens}"
        )
        return

    validator_path = resolve_validator_bin(validator_bin)
    if validator_path is None:
        errors.append(f"validator binary not found: {validator_bin}")
        return
    if validator_timeout_seconds <= 0:
        errors.append("validator-timeout-seconds must be positive")
        return

    argv = [validator_path, "hedging", flag, norito_rel_path.as_posix()]
    try:
        completed = subprocess.run(
            argv,
            cwd=REPO_ROOT,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=validator_timeout_seconds,
        )
    except OSError as error:
        errors.append(f"{name} validator execution failed: {_error_label(error)}")
        return
    except subprocess.TimeoutExpired:
        errors.append(
            f"{name} validator timed out after {validator_timeout_seconds} seconds"
        )
        return

    result["validator"] = "checked"
    result["validator_exit_code"] = completed.returncode
    actual_status = "accepted" if completed.returncode == 0 else "rejected"
    if completed.returncode not in {0, 2}:
        actual_status = "error"
    result["validator_actual_status"] = actual_status
    if expected_status not in EXPECTED_STATUSES:
        return
    if expected_status != actual_status:
        errors.append(
            f"{name} expected validator status {expected_status}, got {actual_status}"
        )


def resolve_validator_bin(validator_bin: str) -> str | None:
    """Resolve a validator executable without invoking a shell."""

    validator_path = Path(validator_bin)
    if validator_path.is_absolute():
        return str(validator_path) if validator_path.is_file() else None
    if len(validator_path.parts) > 1:
        repo_relative = REPO_ROOT / validator_path
        return str(repo_relative) if repo_relative.is_file() else None
    return shutil.which(validator_bin)


def read_generated_bytes(
    path: Path,
    max_bytes: int,
    label: str,
    errors: list[str],
) -> bytes | None:
    """Read a generated fixture file with size bounds."""

    file_label = f"generated {label} fixture"
    is_file = inspect_regular_file(path, file_label, errors)
    if is_file is None:
        return None
    if not is_file:
        errors.append(f"missing generated {label} fixture: {_path_label(path)}")
        return None
    raw = read_file_bytes(path, file_label, errors, max_bytes=max_bytes)
    if raw is None:
        return None
    if not raw:
        errors.append(f"generated {label} fixture is empty: {_path_label(path)}")
        return None
    return raw


if __name__ == "__main__":
    raise SystemExit(main())
