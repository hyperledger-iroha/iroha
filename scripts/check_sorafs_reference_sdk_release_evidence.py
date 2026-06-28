#!/usr/bin/env python3
"""Validate SoraFS reference SDK release evidence artifacts."""

from __future__ import annotations

import argparse
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    emit_checker_notice,
    render_and_write_checker_summary,
    validate_checker_preflight,
)
from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    evidence_path_identities,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_validation import (  # noqa: E402
    build_evidence_artifact,
    count_evidence_artifacts,
    count_evidence_files,
    evidence_gate_status,
    evidence_artifact_is_valid,
    evidence_artifact_fingerprint,
    evidence_schema_by_kind,
    init_evidence_artifact_buckets,
    build_required_evidence_summary,
    record_explicit_evidence_validation_errors,
    record_evidence_artifact,
    record_evidence_validation_errors,
    validate_bound_evidence_digest_references,
    require_bool_true,
    require_false,
    require_hex,
    require_governance_approval,
    validate_standard_evidence_payload,
    require_maximum_number,
    require_minimum_int,
    require_object,
    required_evidence_kind_names,
    require_passed_status,
    require_policy_digest,
    require_positive_int,
    require_recent_timestamp,
    require_string,
    require_string_coverage,
    require_string_equal,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_evidence_kinds,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


SUMMARY_SCHEMA = "sorafs.reference_sdk.release_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 14 * 24 * 60 * 60
DEFAULT_MIN_RELEASE_TARGETS = 4
DEFAULT_MIN_DOWNSTREAM_PACKAGES = 5
DEFAULT_MAX_SMOKE_DURATION_SECS = 30 * 60
HEX64_LEN = 64

REQUIRED_RELEASE_TARGETS = (
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
)
REQUIRED_DOWNSTREAM_PACKAGES = (
    "javascript",
    "python",
    "kotlin_jvm",
    "java_android",
    "swift",
)
RELEASE_MANIFEST_BOUND_KINDS = (
    "release_archive",
    "downstream_bindings",
    "cookbook_smoke",
    "ffi_header_contract",
    "governance_approval",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "evidence_json",
    "manifest_signing_key",
    "mnemonic",
    "norito_bytes",
    "package_bytes",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_archive",
    "raw_archives",
    "raw_binary",
    "raw_evidence",
    "raw_manifest",
    "raw_package",
    "raw_packages",
    "raw_request",
    "raw_response",
    "raw_smoke_output",
    "raw_smoke_outputs",
    "release_private_key",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SF-11 release evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("release_archive", "sorafs.reference_sdk.release_archive_canary.v1"),
    EvidenceKind("signed_manifest", "sorafs.reference_sdk.signed_manifest_canary.v1"),
    EvidenceKind("downstream_bindings", "sorafs.reference_sdk.downstream_bindings_canary.v1"),
    EvidenceKind("cookbook_smoke", "sorafs.reference_sdk.cookbook_smoke_canary.v1"),
    EvidenceKind("ffi_header_contract", "sorafs.reference_sdk.ffi_header_contract_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.reference_sdk.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SF-11 release evidence gate."""

    now_unix: int
    max_evidence_age_secs: int
    min_release_targets: int
    min_downstream_packages: int
    max_smoke_duration_secs: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "manifest_digest_hex",
    "release_manifest_digest_hex",
)


def validate_release_archive(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_bool_true(payload, "packaging_helper_used", errors)
    require_bool_true(payload, "deterministic_archive_verified", errors)
    require_bool_true(payload, "archive_checksums_published", errors)
    require_bool_true(payload, "binary_checksums_published", errors)
    require_bool_true(payload, "dist_gitkeep_only_tracked", errors)
    require_minimum_int(payload, "target_count", options.min_release_targets, errors)
    require_string_coverage(payload, "targets", "", REQUIRED_RELEASE_TARGETS, errors)
    require_hex(payload, "archive_index_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "release_manifest_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_archives_included", errors)


def validate_signed_manifest(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "manifest_signed", errors)
    require_bool_true(payload, "manifest_signature_verified", errors)
    require_bool_true(payload, "manifest_sha256_published", errors)
    require_bool_true(payload, "governed_release_key_used", errors)
    require_bool_true(payload, "public_key_fingerprint_recorded", errors)
    require_bool_true(payload, "private_key_absent", errors)
    require_string(payload, "signature_algorithm", errors)
    require_hex(payload, "manifest_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "public_key_fingerprint_hex", HEX64_LEN, errors)
    require_false(payload, "raw_manifest_included", errors)


def validate_downstream_bindings(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_string_coverage(payload, "packages", "", REQUIRED_DOWNSTREAM_PACKAGES, errors)
    require_minimum_int(
        payload,
        "package_count",
        options.min_downstream_packages,
        errors,
    )
    require_bool_true(payload, "sdk_exports_verified", errors)
    require_bool_true(payload, "validation_outcome_contract_verified", errors)
    require_bool_true(payload, "version_alignment_verified", errors)
    require_bool_true(payload, "native_bridge_header_bound", errors)
    require_bool_true(payload, "published_package_digests_recorded", errors)
    require_hex(payload, "release_manifest_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "package_index_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_packages_included", errors)


def validate_cookbook_smoke(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_bool_true(payload, "published_archive_smoke_passed", errors)
    require_bool_true(payload, "cookbook_replay_passed", errors)
    require_bool_true(payload, "fixture_bundle_validation_passed", errors)
    require_bool_true(payload, "manifest_car_replay_passed", errors)
    require_bool_true(payload, "validation_outcomes_emitted", errors)
    require_maximum_number(
        payload,
        "smoke_duration_seconds",
        options.max_smoke_duration_secs,
        errors,
    )
    require_hex(payload, "release_manifest_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "smoke_output_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_smoke_outputs_included", errors)


def validate_ffi_header_contract(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "ci_guard_passed", errors)
    require_bool_true(payload, "rust_exports_match_header", errors)
    require_bool_true(payload, "selector_constants_match", errors)
    require_bool_true(payload, "c_signatures_match", errors)
    require_bool_true(payload, "bridge_bindings_verified", errors)
    require_hex(payload, "release_manifest_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "header_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "ffi_contract_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_header_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_governance_approval(payload, errors)
    require_bool_true(payload, "release_key_roster_bound", errors)
    require_bool_true(payload, "release_targets_bound", errors)
    require_bool_true(payload, "downstream_packages_bound", errors)
    require_bool_true(payload, "smoke_evidence_bound", errors)
    require_string_equal(payload, "governance_source", "governed_release", errors)
    require_hex(payload, "release_manifest_digest_hex", HEX64_LEN, errors)
    require_policy_digest(payload, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_passed_status(payload, errors)
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_evidence_age_secs,
    )

    if kind.name == "release_archive":
        validate_release_archive(payload, errors, options)
    elif kind.name == "signed_manifest":
        validate_signed_manifest(payload, errors)
    elif kind.name == "downstream_bindings":
        validate_downstream_bindings(payload, errors, options)
    elif kind.name == "cookbook_smoke":
        validate_cookbook_smoke(payload, errors, options)
    elif kind.name == "ffi_header_contract":
        validate_ffi_header_contract(payload, errors)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SF-11 release artifact",
        SENSITIVE_KEYS,
        "release evidence",
        lambda kind, checked_payload, errors: validate_kind_specific(
            kind, checked_payload, errors, options
        ),
        require_reviewed_deployment_context=True,
    )



def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []


    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_release_manifest_digests: set[str] = set()
    valid_release_manifest_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    files = discover_evidence_files(
        evidence_dirs,
        evidence_files,
        errors,
        reserved_output_paths=() if summary_out is None else (summary_out,),
    )
    explicit = evidence_path_identities(evidence_files, errors)

    for path in files:
        loaded = load_evidence_json_with_sha256_or_record_error(
            path, MAX_EVIDENCE_BYTES, errors
        )
        if loaded is None:
            continue
        payload, digest = loaded
        kind_name, validation_errors = validate_evidence_payload(payload, options)
        if kind_name is None:
            record_explicit_evidence_validation_errors(
                path, explicit, validation_errors, errors
            )
            continue
        artifact = build_evidence_artifact(
            path,
            digest,
            payload,
            validation_errors,
            FINGERPRINT_FIELDS,
        )
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            if kind_name == "signed_manifest":
                digest = fingerprint.get("manifest_digest_hex")
                if isinstance(digest, str):
                    valid_release_manifest_digests.add(digest.lower())
            elif kind_name in RELEASE_MANIFEST_BOUND_KINDS:
                valid_release_manifest_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("signed_manifest",),
        bound_artifacts=valid_release_manifest_bound_artifacts,
        valid_anchor_digests=valid_release_manifest_digests,
        digest_field="release_manifest_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} release_manifest_digest_hex must reference a valid "
            "signed_manifest manifest_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} release_manifest_digest_hex requires a valid "
            "signed_manifest manifest_digest_hex"
        ),
    )

    required = build_required_evidence_summary(
        required_kinds,
        artifacts_by_kind,
        evidence_schema_by_kind(KIND_BY_NAME),
        errors,
        evidence_label="release",
    )

    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": evidence_gate_status(errors),
        "required_kinds": required_evidence_kind_names(required_kinds),
        "thresholds": {
            "max_evidence_age_secs": options.max_evidence_age_secs,
            "min_release_targets": options.min_release_targets,
            "min_downstream_packages": options.min_downstream_packages,
            "max_smoke_duration_secs": options.max_smoke_duration_secs,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_release_manifest_digests": sorted(valid_release_manifest_digests),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SF-11 reference SDK release evidence artifacts."
    )
    parser.add_argument(
        "--evidence-dir",
        action="append",
        type=Path,
        default=[],
        help="Directory containing release evidence JSON artifacts.",
    )
    parser.add_argument(
        "--evidence",
        action="append",
        type=Path,
        default=[],
        help="Explicit release evidence JSON artifact.",
    )
    parser.add_argument(
        "--require-kind",
        action="append",
        default=[],
        help="Required evidence kind, or comma-separated kinds. Defaults to all SF-11 kinds.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        default=int(time.time()),
        help="Validator clock used for age checks. Defaults to current Unix time.",
    )
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )
    parser.add_argument(
        "--min-release-targets",
        type=positive_int_arg,
        default=DEFAULT_MIN_RELEASE_TARGETS,
    )
    parser.add_argument(
        "--min-downstream-packages",
        type=positive_int_arg,
        default=DEFAULT_MIN_DOWNSTREAM_PACKAGES,
    )
    parser.add_argument(
        "--max-smoke-duration-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_SMOKE_DURATION_SECS,
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_exception(error)
        return 2
    try:
        args = parser.parse_args(expanded_args)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    try:
        required_kinds = parse_required_evidence_kinds(
            args.require_kind,
            allowed_kinds=KIND_BY_NAME,
            default_required=DEFAULT_REQUIRED_KINDS,
        )
    except ValueError as error:
        emit_checker_exception(error)
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        min_release_targets=args.min_release_targets,
        min_downstream_packages=args.min_downstream_packages,
        max_smoke_duration_secs=args.max_smoke_duration_secs,
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir, args.evidence, required_kinds, options, args.summary_out
    )
    rendered_summary, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2

    if errors:
        emit_checker_error_block("ERROR: SoraFS reference SDK release evidence is incomplete:", errors)
        return 1

    emit_checker_notice(
        "SoraFS reference SDK release evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
