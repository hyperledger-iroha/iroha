#!/usr/bin/env python3
"""Aggregate ISO 20022 production-readiness evidence.

Purpose:
  This offline release gate combines the ISO XSD fixture preflight and operator
  production-evidence summaries into one digest-bound readiness report. It
  fails closed on missing strict XSD closure, local-test evidence overrides,
  plan-only or partial canary evidence, weak receipt evidence, non-production
  trust policy, and provider/environment drift.

Prerequisites:
  Python 3.11+. No third party Python packages are required. XSD summaries
  should come from ``iso_xsd_fixture_verify.py`` and evidence summaries should
  come from ``iso_operator_evidence_verify.py``.

Safety:
  The script is read-only unless ``--summary-out`` is supplied. It does not
  contact Torii, rail gateways, notaries, PKI endpoints, OCSP, CRL, or remote
  schema repositories.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


READINESS_VERSION = 1
EVIDENCE_VERSION = 1
SUMMARY_DIGEST_FIELD = "summary_sha256"
REQUIRED_CANARY_STAGES = {"rail", "notary", "verify"}
REQUIRED_RECEIPT_KINDS = {"iso-audit-notary", "iso-rail-gateway"}
REQUIRE_VERIFIED = "require-verified"
PRODUCTION_FALSE_POLICY_FLAGS = {
    "allow_plan_only",
    "allow_dry_run",
    "allow_insecure_http",
    "allow_legacy_colr007",
    "allow_default_profile",
    "allow_failed_receipts",
    "allow_partial_canary",
    "allow_receipt_source_missing",
    "allow_record_only_trust",
    "allow_synthetic_trust",
    "allow_missing_trust_source",
}


class ReadinessError(RuntimeError):
    """Raised when a readiness input is malformed or digest-tampered."""


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
        raise ReadinessError(f"{path} does not exist") from error
    except json.JSONDecodeError as error:
        raise ReadinessError(f"{path} is not valid JSON: {error}") from error


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ReadinessError(f"{label} must be a JSON object")
    return value


def _require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise ReadinessError(f"{label} must be a JSON array")
    return value


def _require_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise ReadinessError(f"{label}.{key} must be a non-empty string")
    return raw.strip()


def _optional_bool(value: dict[str, Any], key: str, label: str, default: bool = False) -> bool:
    raw = value.get(key, default)
    if not isinstance(raw, bool):
        raise ReadinessError(f"{label}.{key} must be a boolean")
    return raw


def _require_bool(value: dict[str, Any], key: str, label: str) -> bool:
    raw = value.get(key)
    if not isinstance(raw, bool):
        raise ReadinessError(f"{label}.{key} must be a boolean")
    return raw


def _require_positive_int(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw <= 0:
        raise ReadinessError(f"{label}.{key} must be a positive integer")
    return raw


def _require_nonnegative_int(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw < 0:
        raise ReadinessError(f"{label}.{key} must be a non-negative integer")
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
        raise ReadinessError(f"{label} has missing or non-canonical {SUMMARY_DIGEST_FIELD}")
    body = dict(summary)
    body.pop(SUMMARY_DIGEST_FIELD)
    actual = sha256_hex(_canonical_json_bytes(body))
    if actual != expected:
        raise ReadinessError(
            f"{label} {SUMMARY_DIGEST_FIELD} mismatch: expected {expected}, got {actual}"
        )
    return expected


def _blocker(blockers: list[dict[str, Any]], code: str, message: str, path: Path) -> None:
    blockers.append({"code": code, "message": message, "path": str(path)})


def _verify_receipt_summary(
    receipt_obj: dict[str, Any],
    label: str,
    path: Path,
    blockers: list[dict[str, Any]],
    *,
    missing_kinds_code: str,
    allow_failed_code: str,
    allow_insecure_code: str,
    allow_legacy_code: str,
    source_files_code: str,
    count_mismatch_code: str,
    digest_missing_code: str,
    kind_entry_mismatch_code: str,
) -> dict[str, Any]:
    digest = _require_summary_digest(receipt_obj, label)
    verified_receipts = _require_positive_int(
        receipt_obj,
        "verified_receipts",
        label,
    )
    receipt_kind_raw = _require_list(
        receipt_obj.get("receipt_kind"),
        f"{label}.receipt_kind",
    )
    if not all(isinstance(item, str) for item in receipt_kind_raw):
        raise ReadinessError(f"{label}.receipt_kind must contain strings")
    receipt_kind_set = set(receipt_kind_raw)
    missing = sorted(REQUIRED_RECEIPT_KINDS - receipt_kind_set)
    if missing:
        _blocker(
            blockers,
            missing_kinds_code,
            "receipt verification is missing kinds: " + ", ".join(missing),
            path,
        )
    unsupported = sorted(receipt_kind_set - REQUIRED_RECEIPT_KINDS)
    if unsupported:
        _blocker(
            blockers,
            kind_entry_mismatch_code,
            "receipt verification contains unsupported kinds: " + ", ".join(unsupported),
            path,
        )
    if _optional_bool(receipt_obj, "allow_failed", label):
        _blocker(
            blockers,
            allow_failed_code,
            "receipt verifier evidence allowed failed receipts",
            path,
        )
    if _optional_bool(receipt_obj, "allow_insecure_http", label):
        _blocker(
            blockers,
            allow_insecure_code,
            "receipt verifier evidence allowed insecure HTTP endpoints",
            path,
        )
    allow_legacy_colr007 = _optional_bool(receipt_obj, "allow_legacy_colr007", label)
    if allow_legacy_colr007:
        _blocker(
            blockers,
            allow_legacy_code,
            "receipt verifier evidence allowed legacy colr.007 rail receipts",
            path,
        )
    if not _optional_bool(receipt_obj, "require_source_files", label):
        _blocker(
            blockers,
            source_files_code,
            "receipt verifier evidence did not require source files",
            path,
        )

    receipts_raw = _require_list(receipt_obj.get("receipts"), f"{label}.receipts")
    if len(receipts_raw) != verified_receipts:
        _blocker(
            blockers,
            count_mismatch_code,
            "receipt verification count does not match receipts[] entries",
            path,
        )
    receipts: list[dict[str, Any]] = []
    receipt_entry_kinds: set[str] = set()
    for offset, receipt_raw in enumerate(receipts_raw):
        entry_label = f"{label}.receipts[{offset}]"
        receipt = _require_object(receipt_raw, entry_label)
        receipt_path = _require_string(receipt, "path", entry_label)
        receipt_kind = _require_string(receipt, "receipt_kind", entry_label)
        receipt_sha256 = receipt.get("receipt_sha256")
        if not _is_lower_sha256(receipt_sha256):
            _blocker(
                blockers,
                digest_missing_code,
                f"{entry_label}.receipt_sha256 is missing or non-canonical",
                path,
            )
        receipts.append(dict(receipt))
        receipt_entry_kinds.add(receipt_kind)
    if receipt_kind_set != receipt_entry_kinds:
        _blocker(
            blockers,
            kind_entry_mismatch_code,
            "receipt_kind does not match receipts[].receipt_kind",
            path,
        )

    return {
        "verified_receipts": verified_receipts,
        "receipt_kind": sorted(receipt_kind_set),
        "allow_failed": _optional_bool(receipt_obj, "allow_failed", label),
        "allow_insecure_http": _optional_bool(receipt_obj, "allow_insecure_http", label),
        "allow_legacy_colr007": allow_legacy_colr007,
        "require_source_files": _optional_bool(receipt_obj, "require_source_files", label),
        "receipts": receipts,
        "summary_sha256": digest,
    }


def verify_xsd_summary(
    path: Path,
    *,
    allow_reviewed_xsd_gaps: bool,
    blockers: list[dict[str, Any]],
    warnings: list[dict[str, Any]],
) -> dict[str, Any]:
    """Verify one XSD preflight summary and append production blockers."""

    summary = _require_object(_load_json(path), str(path))
    digest = _require_summary_digest(summary, str(path))
    verified_schemas = _require_positive_int(summary, "verified_schemas", str(path))
    verified_fixtures = _require_positive_int(summary, "verified_fixtures", str(path))
    schema_backed_fixtures = summary.get("schema_backed_fixtures")
    if (
        isinstance(schema_backed_fixtures, bool)
        or not isinstance(schema_backed_fixtures, int)
        or schema_backed_fixtures < 0
    ):
        raise ReadinessError(f"{path}.schema_backed_fixtures must be a non-negative integer")
    missing_schema_fixtures = _require_list(
        summary.get("missing_schema_fixtures"),
        f"{path}.missing_schema_fixtures",
    )
    schema_only_entries = _require_list(
        summary.get("schema_only_entries"),
        f"{path}.schema_only_entries",
    )
    strict = _require_object(summary.get("strict"), f"{path}.strict")
    require_schema_backed = _optional_bool(
        strict,
        "require_schema_backed_fixtures",
        f"{path}.strict",
    )
    require_fixture_for_schema = _optional_bool(
        strict,
        "require_fixture_for_schema",
        f"{path}.strict",
    )
    if schema_backed_fixtures > verified_fixtures:
        raise ReadinessError(f"{path}.schema_backed_fixtures exceeds verified_fixtures")

    if not require_schema_backed:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.strict_schema_backed_not_proven",
                "message": "XSD summary was not produced with --require-schema-backed-fixtures",
                "path": str(path),
            }
        )
    if not require_fixture_for_schema:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.strict_fixture_for_schema_not_proven",
                "message": "XSD summary was not produced with --require-fixture-for-schema",
                "path": str(path),
            }
        )
    if missing_schema_fixtures:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.missing_schema_fixtures",
                "message": f"{len(missing_schema_fixtures)} XML fixtures are not schema-backed",
                "path": str(path),
                "entries": missing_schema_fixtures,
            }
        )
    if schema_only_entries:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.schema_only_entries",
                "message": f"{len(schema_only_entries)} XSDs have no standalone XML fixture",
                "path": str(path),
                "entries": schema_only_entries,
            }
        )
    return {
        "path": str(path),
        "verified_schemas": verified_schemas,
        "verified_fixtures": verified_fixtures,
        "schema_backed_fixtures": schema_backed_fixtures,
        "missing_schema_fixture_count": len(missing_schema_fixtures),
        "schema_only_count": len(schema_only_entries),
        "strict": {
            "require_schema_backed_fixtures": require_schema_backed,
            "require_fixture_for_schema": require_fixture_for_schema,
        },
        "summary_sha256": digest,
    }


def _verify_policy(summary: dict[str, Any], path: Path, blockers: list[dict[str, Any]]) -> None:
    policy = _require_object(summary.get("policy"), f"{path}.policy")
    for flag in sorted(PRODUCTION_FALSE_POLICY_FLAGS):
        if _require_bool(policy, flag, f"{path}.policy"):
            _blocker(
                blockers,
                f"evidence.policy.{flag}",
                f"Evidence summary was produced with non-production policy {flag}=true",
                path,
            )


def _verify_canary(
    canary: dict[str, Any],
    label: str,
    path: Path,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any]:
    provider = _require_string(canary, "provider", label)
    environment = _require_string(canary, "environment", label)
    plan_only = _optional_bool(canary, "plan_only", label)
    if args.provider is not None and provider != args.provider:
        _blocker(
            blockers,
            "evidence.provider_mismatch",
            f"canary provider is {provider!r}, expected {args.provider!r}",
            path,
        )
    if args.environment is not None and environment != args.environment:
        _blocker(
            blockers,
            "evidence.environment_mismatch",
            f"canary environment is {environment!r}, expected {args.environment!r}",
            path,
        )
    if plan_only:
        _blocker(blockers, "evidence.plan_only", "canary summary is plan-only", path)
    stage_names_raw = _require_list(canary.get("stage_names"), f"{label}.stage_names")
    if not all(isinstance(item, str) for item in stage_names_raw):
        raise ReadinessError(f"{label}.stage_names must contain strings")
    stage_names = set(stage_names_raw)
    missing_stages = sorted(REQUIRED_CANARY_STAGES - stage_names)
    if missing_stages:
        _blocker(
            blockers,
            "evidence.missing_canary_stages",
            "canary summary is missing stages: " + ", ".join(missing_stages),
            path,
        )
    receipt_summary = _verify_receipt_summary(
        _require_object(canary.get("receipt_summary"), f"{label}.receipt_summary"),
        f"{label}.receipt_summary",
        path,
        blockers,
        missing_kinds_code="evidence.missing_receipt_kinds",
        allow_failed_code="evidence.receipts_allow_failed",
        allow_insecure_code="evidence.receipts_allow_insecure_http",
        allow_legacy_code="evidence.receipts_allow_legacy_colr007",
        source_files_code="evidence.receipts_source_files_not_required",
        count_mismatch_code="evidence.receipt_count_mismatch",
        digest_missing_code="evidence.receipt_digest_missing",
        kind_entry_mismatch_code="evidence.receipt_kind_entry_mismatch",
    )
    return {
        "provider": provider,
        "environment": environment,
        "plan_only": plan_only,
        "stage_names": sorted(stage_names),
        "verified_receipts": receipt_summary["verified_receipts"],
        "receipt_kind": receipt_summary["receipt_kind"],
        "receipt_summary": receipt_summary,
    }


def _verify_trust_profile(
    profile: dict[str, Any],
    label: str,
    path: Path,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any]:
    profile_id = _require_string(profile, "profile_id", label)
    rail = _require_string(profile, "rail", label)
    environment = _require_string(profile, "environment", label)
    policy = _require_string(profile, "embedded_signature_policy", label)
    signature_pin_count = _require_nonnegative_int(
        profile,
        "signature_public_key_pin_count",
        label,
    )
    x509_pin_count = _require_nonnegative_int(
        profile,
        "x509_trust_anchor_pin_count",
        label,
    )
    if signature_pin_count + x509_pin_count <= 0:
        _blocker(
            blockers,
            "trust.no_signature_or_x509_pins",
            f"trust profile {profile_id!r} has no public-key or X.509 pins",
            path,
        )
    if args.environment is not None and environment != args.environment:
        _blocker(
            blockers,
            "trust.environment_mismatch",
            f"trust profile {profile_id!r} environment is {environment!r}, expected {args.environment!r}",
            path,
        )
    if policy != REQUIRE_VERIFIED:
        _blocker(
            blockers,
            "trust.policy_not_require_verified",
            f"trust profile {profile_id!r} uses {policy!r}",
            path,
        )
    return {
        "profile_id": profile_id,
        "rail": rail,
        "environment": environment,
        "embedded_signature_policy": policy,
        "signature_public_key_pin_count": signature_pin_count,
        "x509_trust_anchor_pin_count": x509_pin_count,
    }


def _verify_archive_receipts(
    summary: dict[str, Any],
    path: Path,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any] | None:
    receipt_summary = summary.get("receipt_verification")
    if receipt_summary is None:
        if args.allow_canary_stage_receipts_only:
            return None
        _blocker(
            blockers,
            "evidence.archive_receipts_not_reverified",
            "evidence summary does not include direct receipt archive verification",
            path,
        )
        return None
    receipt_obj = _require_object(receipt_summary, f"{path}.receipt_verification")
    return _verify_receipt_summary(
        receipt_obj,
        f"{path}.receipt_verification",
        path,
        blockers,
        missing_kinds_code="evidence.archive_receipt_kinds_missing",
        allow_failed_code="evidence.archive_receipts_allow_failed",
        allow_insecure_code="evidence.archive_receipts_insecure_http",
        allow_legacy_code="evidence.archive_receipts_allow_legacy_colr007",
        source_files_code="evidence.archive_receipts_source_files_not_required",
        count_mismatch_code="evidence.archive_receipt_count_mismatch",
        digest_missing_code="evidence.archive_receipt_digest_missing",
        kind_entry_mismatch_code="evidence.archive_receipt_kind_entry_mismatch",
    )


def verify_evidence_summary(
    path: Path,
    *,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any]:
    """Verify one aggregate operator-evidence summary and append blockers."""

    summary = _require_object(_load_json(path), str(path))
    digest = _require_summary_digest(summary, str(path))
    version = summary.get("version")
    if version != EVIDENCE_VERSION:
        raise ReadinessError(f"{path}.version must be {EVIDENCE_VERSION}")
    if not _optional_bool(summary, "ok", str(path)):
        _blocker(blockers, "evidence.summary_not_ok", "evidence summary is not ok", path)
    _verify_policy(summary, path, blockers)

    canary_summaries = _require_list(summary.get("canary_summaries"), f"{path}.canary_summaries")
    trust_summaries = _require_list(summary.get("trust_summaries"), f"{path}.trust_summaries")
    if not canary_summaries:
        _blocker(blockers, "evidence.no_canary_summaries", "no canary summaries recorded", path)
    if not trust_summaries:
        _blocker(blockers, "evidence.no_trust_summaries", "no trust summaries recorded", path)
    archive_receipts = _verify_archive_receipts(summary, path, args, blockers)

    canaries = [
        _verify_canary(
            _require_object(canary, f"{path}.canary_summaries[{offset}]"),
            f"{path}.canary_summaries[{offset}]",
            path,
            args,
            blockers,
        )
        for offset, canary in enumerate(canary_summaries)
    ]
    trust_outputs: list[dict[str, Any]] = []
    for offset, trust in enumerate(trust_summaries):
        label = f"{path}.trust_summaries[{offset}]"
        trust_obj = _require_object(trust, label)
        verified_bundles = _require_positive_int(trust_obj, "verified_bundles", label)
        profiles_raw = _require_list(trust_obj.get("profiles"), f"{label}.profiles")
        if not profiles_raw:
            _blocker(blockers, "trust.no_profiles", "trust summary has no profiles", path)
        profiles = [
            _verify_trust_profile(
                _require_object(profile, f"{label}.profiles[{profile_offset}]"),
                f"{label}.profiles[{profile_offset}]",
                path,
                args,
                blockers,
            )
            for profile_offset, profile in enumerate(profiles_raw)
        ]
        trust_outputs.append(
            {
                "path": trust_obj.get("path"),
                "verified_bundles": verified_bundles,
                "profiles": profiles,
                "summary_sha256": trust_obj.get("summary_sha256"),
            }
        )
    return {
        "path": str(path),
        "canary_summaries": canaries,
        "trust_summaries": trust_outputs,
        "receipt_verification": archive_receipts,
        "summary_sha256": digest,
    }


def run(args: argparse.Namespace) -> int:
    if not args.xsd_summary:
        raise ReadinessError("provide at least one --xsd-summary")
    if not args.evidence_summary:
        raise ReadinessError("provide at least one --evidence-summary")

    blockers: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    xsd_summaries = [
        verify_xsd_summary(
            path.resolve(),
            allow_reviewed_xsd_gaps=args.allow_reviewed_xsd_gaps,
            blockers=blockers,
            warnings=warnings,
        )
        for path in args.xsd_summary
    ]
    evidence_summaries = [
        verify_evidence_summary(path.resolve(), args=args, blockers=blockers)
        for path in args.evidence_summary
    ]
    output: dict[str, Any] = {
        "version": READINESS_VERSION,
        "checked_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "ok": not blockers,
        "blockers": blockers,
        "warnings": warnings,
        "xsd_summaries": xsd_summaries,
        "evidence_summaries": evidence_summaries,
        "policy": {
            "provider": args.provider,
            "environment": args.environment,
            "allow_reviewed_xsd_gaps": args.allow_reviewed_xsd_gaps,
            "allow_canary_stage_receipts_only": args.allow_canary_stage_receipts_only,
        },
    }
    output[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(output))
    text = json.dumps(output, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        args.summary_out.parent.mkdir(parents=True, exist_ok=True)
        args.summary_out.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if output["ok"] else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Aggregate ISO 20022 production-readiness evidence summaries."
    )
    parser.add_argument(
        "--xsd-summary",
        action="append",
        default=[],
        type=Path,
        help="Digest-bound summary JSON from iso_xsd_fixture_verify.py; repeatable.",
    )
    parser.add_argument(
        "--evidence-summary",
        action="append",
        default=[],
        type=Path,
        help="Digest-bound summary JSON from iso_operator_evidence_verify.py; repeatable.",
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
        help="Optional path to write the production-readiness summary JSON.",
    )
    parser.add_argument(
        "--allow-reviewed-xsd-gaps",
        action="store_true",
        help="Downgrade reviewed XSD missing-schema/schema-only gaps to warnings for local audits.",
    )
    parser.add_argument(
        "--allow-canary-stage-receipts-only",
        action="store_true",
        help="Do not require final evidence summaries to include direct receipt archive verification.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except ReadinessError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
