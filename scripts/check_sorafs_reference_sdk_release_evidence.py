#!/usr/bin/env python3
"""Validate SoraFS reference SDK release evidence artifacts."""

from __future__ import annotations

import argparse
import hashlib
import secrets
import sys
from collections.abc import Callable
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
    archive_artifact_path_label,
    build_evidence_artifact,
    count_evidence_artifacts,
    recognized_evidence_artifacts,
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
    require_maximum_int,
    require_minimum_int,
    require_non_negative_int,
    require_object,
    required_evidence_kind_names,
    require_passed_status,
    require_policy_digest,
    require_positive_int,
    require_recent_timestamp,
    require_string,
    require_string_coverage,
    require_string_equal,
    require_string_in,
    require_string_inventory_count_match,
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
from sorafs_topology_qualification import (  # noqa: E402
    add_topology_qualification_argument,
    bind_lane_summary_to_topology,
)
from sccp_release_common import verify_ed25519  # noqa: E402
from sorafs_reference_sdk_supply_chain import (  # noqa: E402
    SOURCE_ARTIFACT_KINDS,
    SupplyChainSourceResult,
    validate_supply_chain_sources,
)

SUMMARY_SCHEMA = "sorafs.reference_sdk.release_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 14 * 24 * 60 * 60
DEFAULT_MIN_RELEASE_TARGETS = 5
DEFAULT_MIN_DOWNSTREAM_PACKAGES = 6
DEFAULT_MAX_SMOKE_DURATION_SECS = 30 * 60
HEX64_LEN = 64

MANDATORY_RELEASE_TARGETS = (
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
)
ADDITIONAL_RELEASE_TARGETS = ("x86_64-pc-windows-msvc",)
REQUIRED_RELEASE_TARGETS = MANDATORY_RELEASE_TARGETS + ADDITIONAL_RELEASE_TARGETS
REQUIRED_DOWNSTREAM_PACKAGES = (
    "javascript",
    "python",
    "kotlin_jvm",
    "java_android",
    "swift",
    "csharp",
)
RELEASE_MANIFEST_BOUND_KINDS = (
    "release_archive",
    "supply_chain",
    "downstream_bindings",
    "cookbook_smoke",
    "ffi_header_contract",
    "governance_approval",
)
POLICY_BOUND_KINDS = ("governance_approval",)
RELEASE_KEY_BOUND_KINDS = ("governance_approval",)
ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS = ("ed25519",)
REQUIRED_SIGNING_PROVIDER = "external_ed25519_hsm"
SUPPLY_CHAIN_TARGET_RESULT_FIELDS = frozenset(
    {
        "target",
        "binary_smoke_passed",
        "deterministic_archive_replay_passed",
        "installation_verified",
        "rollback_verified",
        "yank_verified",
        "sbom_generated",
        "critical_vulnerability_count",
        "high_vulnerability_count",
        "oidc_identity_verified",
        "cosign_provenance_verified",
    }
)
SUPPLY_CHAIN_SOURCE_ARTIFACT_FIELDS = frozenset(
    {"kind", "artifact_path", "sha256"}
)
SUPPLY_CHAIN_DERIVED_FIELDS = (
    "generated_at_unix",
    "deployment_id",
    "environment",
    "release_manifest_digest_hex",
    "source_artifacts",
    "target_count",
    "target_results",
    "sbom_index_digest_hex",
    "vulnerability_report_digest_hex",
    "provenance_bundle_digest_hex",
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


def require_only_required_values(
    payload: dict[str, Any],
    array_field: str,
    field: str,
    required_values: tuple[str, ...],
    errors: list[str],
) -> None:
    """Reject reviewed inventory rows outside a required closed string set."""

    values = payload.get(array_field)
    if not isinstance(values, list):
        return
    allowed = frozenset(required_values)
    for item in values:
        if field:
            if not isinstance(item, dict):
                continue
            value = item.get(field)
        else:
            value = item
        if not isinstance(value, str) or value not in allowed:
            errors.append(f"{array_field} must not include unknown values")
            return


@dataclass(frozen=True)
class EvidenceKind:
    """One SF-11 release evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("release_archive", "sorafs.reference_sdk.release_archive_canary.v1"),
    EvidenceKind("signed_manifest", "sorafs.reference_sdk.signed_manifest_canary.v1"),
    EvidenceKind("supply_chain", "sorafs.reference_sdk.supply_chain_canary.v1"),
    EvidenceKind("downstream_bindings", "sorafs.reference_sdk.downstream_bindings_canary.v1"),
    EvidenceKind("cookbook_smoke", "sorafs.reference_sdk.cookbook_smoke_canary.v1"),
    EvidenceKind("ffi_header_contract", "sorafs.reference_sdk.ffi_header_contract_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.reference_sdk.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)
COMMON_EVIDENCE_REQUIRED_FIELDS: tuple[str, ...] = (
    "schema",
    "status",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
)
EVIDENCE_REQUIRED_FIELDS: dict[str, tuple[str, ...]] = {
    "release_archive": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "packaging_helper_used",
        "deterministic_archive_verified",
        "archive_checksums_published",
        "binary_checksums_published",
        "dist_gitkeep_only_tracked",
        "target_count",
        "targets",
        "archive_index_digest_hex",
        "release_manifest_digest_hex",
        "raw_archives_included",
    ),
    "signed_manifest": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "manifest_signed",
        "manifest_signature_verified",
        "manifest_sha256_published",
        "governed_release_key_used",
        "public_key_fingerprint_recorded",
        "private_key_absent",
        "signature_algorithm",
        "signing_provider",
        "signing_provider_revision",
        "hsm_signature_verified",
        "manifest_digest_hex",
        "policy_digest_hex",
        "public_key_fingerprint_hex",
        "raw_manifest_included",
    ),
    "supply_chain": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "target_count",
        "target_results",
        "source_artifacts",
        "release_manifest_digest_hex",
        "sbom_index_digest_hex",
        "vulnerability_report_digest_hex",
        "provenance_bundle_digest_hex",
        "provenance_certificate_identity",
        "provenance_oidc_issuer",
        "provenance_verification_key_fingerprint_hex",
        "raw_sboms_included",
        "raw_vulnerability_reports_included",
        "raw_provenance_included",
    ),
    "downstream_bindings": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "packages",
        "package_count",
        "sdk_exports_verified",
        "validation_outcome_contract_verified",
        "version_alignment_verified",
        "native_bridge_header_bound",
        "published_package_digests_recorded",
        "release_manifest_digest_hex",
        "package_index_digest_hex",
        "raw_packages_included",
    ),
    "cookbook_smoke": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "published_archive_smoke_passed",
        "cookbook_replay_passed",
        "fixture_bundle_validation_passed",
        "manifest_car_replay_passed",
        "validation_outcomes_emitted",
        "smoke_duration_seconds",
        "release_manifest_digest_hex",
        "smoke_output_digest_hex",
        "raw_smoke_outputs_included",
    ),
    "ffi_header_contract": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "ci_guard_passed",
        "rust_exports_match_header",
        "selector_constants_match",
        "c_signatures_match",
        "bridge_bindings_verified",
        "release_manifest_digest_hex",
        "header_digest_hex",
        "ffi_contract_digest_hex",
        "raw_header_included",
    ),
    "governance_approval": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "approved",
        "governance_vote_recorded",
        "release_key_roster_bound",
        "release_targets_bound",
        "downstream_packages_bound",
        "smoke_evidence_bound",
        "governance_source",
        "release_manifest_digest_hex",
        "policy_digest_hex",
        "public_key_fingerprint_hex",
    ),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SF-11 release evidence gate."""

    now_unix: int
    max_evidence_age_secs: int
    min_release_targets: int
    min_downstream_packages: int
    max_smoke_duration_secs: int
    supply_chain_source_root: Path | None = None
    provenance_certificate_identity: str | None = None
    provenance_oidc_issuer: str | None = None
    provenance_verification_public_key: bytes | None = None


FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "archive_index_digest_hex",
    "ffi_contract_digest_hex",
    "header_digest_hex",
    "manifest_digest_hex",
    "package_index_digest_hex",
    "release_manifest_digest_hex",
    "public_key_fingerprint_hex",
    "smoke_output_digest_hex",
    "sbom_index_digest_hex",
    "vulnerability_report_digest_hex",
    "provenance_bundle_digest_hex",
    "source_artifacts",
    "provenance_certificate_identity",
    "provenance_oidc_issuer",
    "provenance_verification_key_fingerprint_hex",
    "policy_digest_hex",
)


def validated_signature_algorithm_fingerprint_values(
    kind_name: str,
    payload: dict[str, Any],
) -> dict[str, str]:
    """Return signature algorithm fingerprint fields after closed-set validation."""

    if kind_name != "signed_manifest":
        return {}
    signature_algorithm = payload.get("signature_algorithm")
    if (
        isinstance(signature_algorithm, str)
        and signature_algorithm in ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS
    ):
        return {"signature_algorithm": signature_algorithm}
    return {}


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
    require_only_required_values(payload, "targets", "", REQUIRED_RELEASE_TARGETS, errors)
    require_string_inventory_count_match(payload, "targets", "target_count", errors)
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
    require_string_in(
        payload,
        "signature_algorithm",
        ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS,
        errors,
    )
    require_string_equal(payload, "signing_provider", REQUIRED_SIGNING_PROVIDER, errors)
    require_positive_int(payload, "signing_provider_revision", errors)
    require_bool_true(payload, "hsm_signature_verified", errors)
    require_hex(payload, "manifest_digest_hex", HEX64_LEN, errors)
    require_policy_digest(payload, errors)
    require_hex(payload, "public_key_fingerprint_hex", HEX64_LEN, errors)
    require_false(payload, "raw_manifest_included", errors)


def decode_ed25519_public_key(value: str | None) -> bytes | None:
    """Decode one non-zero raw Ed25519 public key without echoing it."""

    if (
        not isinstance(value, str)
        or len(value) != HEX64_LEN
        or any(character not in "0123456789abcdef" for character in value)
    ):
        return None
    public_key = bytes.fromhex(value)
    return public_key if any(public_key) else None


def provenance_receipt_authenticator(
    public_key: bytes,
) -> tuple[str, Callable[[str, bytes, bytes], bool]]:
    """Bind source receipt authentication to one operator-trusted key."""

    fingerprint = hashlib.sha256(public_key).hexdigest()

    def authenticate(
        claimed_fingerprint: str,
        message: bytes,
        signature: bytes,
    ) -> bool:
        return secrets.compare_digest(
            claimed_fingerprint,
            fingerprint,
        ) and verify_ed25519(public_key, signature, message)

    return fingerprint, authenticate


def validate_supply_chain_source_artifacts(
    payload: dict[str, Any],
    errors: list[str],
) -> dict[str, str] | None:
    """Validate and return the four payload-declared source paths by kind."""

    source_artifacts = payload.get("source_artifacts")
    if not isinstance(source_artifacts, list):
        errors.append("source_artifacts must be an array")
        return None
    if len(source_artifacts) != len(SOURCE_ARTIFACT_KINDS):
        errors.append("source_artifacts must contain exactly four bindings")

    observed_kinds: list[str] = []
    source_paths: dict[str, str] = {}
    for index, artifact in enumerate(source_artifacts):
        path = f"source_artifacts[{index}]"
        if not isinstance(artifact, dict):
            errors.append(f"{path} must be an object")
            continue
        if set(artifact) != SUPPLY_CHAIN_SOURCE_ARTIFACT_FIELDS:
            errors.append(f"{path} fields must match the schema-closed contract")
        kind = require_string(artifact, "kind", errors)
        artifact_path = require_string(artifact, "artifact_path", errors)
        require_hex(
            artifact,
            "sha256",
            HEX64_LEN,
            errors,
            path=f"{path}.sha256",
        )
        if kind:
            observed_kinds.append(kind)
            if (
                kind in SOURCE_ARTIFACT_KINDS
                and kind not in source_paths
                and artifact_path
            ):
                source_paths[kind] = artifact_path
    if observed_kinds != list(SOURCE_ARTIFACT_KINDS):
        errors.append(
            "source_artifacts kinds must match the canonical four-source order"
        )
    if set(source_paths) != frozenset(SOURCE_ARTIFACT_KINDS):
        return None
    return source_paths


def validate_supply_chain_trust_metadata(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> tuple[
    Path,
    str,
    str,
    str,
    Callable[[str, bytes, bytes], bool],
] | None:
    """Require payload trust metadata to match all operator-trusted inputs."""

    certificate_identity = require_string(
        payload,
        "provenance_certificate_identity",
        errors,
    )
    oidc_issuer = require_string(payload, "provenance_oidc_issuer", errors)
    verification_key_fingerprint = require_hex(
        payload,
        "provenance_verification_key_fingerprint_hex",
        HEX64_LEN,
        errors,
    )

    inputs_missing = False
    if options.supply_chain_source_root is None:
        errors.append(
            "supply_chain validation requires --supply-chain-source-root"
        )
        inputs_missing = True
    if not options.provenance_certificate_identity:
        errors.append(
            "supply_chain validation requires "
            "--provenance-certificate-identity"
        )
        inputs_missing = True
    if not options.provenance_oidc_issuer:
        errors.append(
            "supply_chain validation requires --provenance-oidc-issuer"
        )
        inputs_missing = True
    public_key = options.provenance_verification_public_key
    if (
        not isinstance(public_key, bytes)
        or len(public_key) != 32
        or not any(public_key)
    ):
        errors.append(
            "supply_chain validation requires a non-zero raw Ed25519 "
            "--provenance-verification-public-key-hex"
        )
        inputs_missing = True
    if inputs_missing:
        return None

    assert options.supply_chain_source_root is not None
    assert options.provenance_certificate_identity is not None
    assert options.provenance_oidc_issuer is not None
    assert isinstance(public_key, bytes)
    expected_fingerprint, authenticator = provenance_receipt_authenticator(
        public_key
    )
    if certificate_identity != options.provenance_certificate_identity:
        errors.append(
            "provenance_certificate_identity must match the operator-trusted "
            "identity"
        )
    if oidc_issuer != options.provenance_oidc_issuer:
        errors.append(
            "provenance_oidc_issuer must match the operator-trusted issuer"
        )
    if verification_key_fingerprint != expected_fingerprint:
        errors.append(
            "provenance_verification_key_fingerprint_hex must match the "
            "operator-trusted key"
        )
    return (
        options.supply_chain_source_root,
        options.provenance_certificate_identity,
        options.provenance_oidc_issuer,
        expected_fingerprint,
        authenticator,
    )


def validate_supply_chain(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    """Require complete per-target release, security, and provenance evidence."""

    require_string_inventory_count_match(
        payload,
        "target_results",
        "target_count",
        errors,
        field="target",
        allow_scalar_items=False,
    )
    target_results = payload.get("target_results")
    if not isinstance(target_results, list):
        errors.append("target_results must be an array")
    observed_targets: list[str] = []
    if isinstance(target_results, list):
        if len(target_results) != len(REQUIRED_RELEASE_TARGETS):
            errors.append("target_results must cover exactly five release targets")
        for index, result in enumerate(target_results):
            path = f"target_results[{index}]"
            if not isinstance(result, dict):
                errors.append(f"{path} must be an object")
                continue
            if set(result) != SUPPLY_CHAIN_TARGET_RESULT_FIELDS:
                errors.append(f"{path} fields must match the schema-closed contract")
            target = require_string(result, "target", errors)
            if target:
                observed_targets.append(target)
            for field in (
                "binary_smoke_passed",
                "deterministic_archive_replay_passed",
                "installation_verified",
                "rollback_verified",
                "yank_verified",
                "sbom_generated",
                "oidc_identity_verified",
                "cosign_provenance_verified",
            ):
                require_bool_true(result, field, errors)
            for field in (
                "critical_vulnerability_count",
                "high_vulnerability_count",
            ):
                value = require_non_negative_int(result, field, errors)
                if value is not None and value != 0:
                    errors.append(f"{field} must be zero")
    if observed_targets != list(REQUIRED_RELEASE_TARGETS):
        errors.append(
            "target_results targets must match the canonical five-target order"
        )
    source_paths = validate_supply_chain_source_artifacts(payload, errors)
    require_hex(payload, "release_manifest_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "sbom_index_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "vulnerability_report_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "provenance_bundle_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_sboms_included", errors)
    require_false(payload, "raw_vulnerability_reports_included", errors)
    require_false(payload, "raw_provenance_included", errors)

    trust = validate_supply_chain_trust_metadata(payload, errors, options)
    if source_paths is None or trust is None:
        return
    (
        source_root,
        certificate_identity,
        oidc_issuer,
        verification_key_fingerprint,
        authenticator,
    ) = trust
    source_result, source_errors = validate_supply_chain_sources(
        source_root,
        expected_deployment_id=payload.get("deployment_id"),
        expected_environment=payload.get("environment"),
        expected_release_manifest_digest_hex=payload.get(
            "release_manifest_digest_hex"
        ),
        expected_certificate_identity=certificate_identity,
        expected_oidc_issuer=oidc_issuer,
        expected_verification_key_fingerprint_hex=verification_key_fingerprint,
        verification_receipt_authenticator=authenticator,
        now_unix=options.now_unix,
        max_source_age_secs=options.max_evidence_age_secs,
        release_rehearsal_path=source_paths["release_rehearsal"],
        sbom_index_path=source_paths["sbom_index"],
        vulnerability_report_path=source_paths["vulnerability_report"],
        provenance_bundle_path=source_paths["provenance_bundle"],
    )
    errors.extend(f"supply-chain source: {error}" for error in source_errors)
    if source_result is None:
        if not source_errors:
            errors.append(
                "supply-chain source validation did not return a validated result"
            )
        return
    if not isinstance(source_result, SupplyChainSourceResult):
        errors.append(
            "supply-chain source validation returned an invalid result"
        )
        return
    derived = source_result.to_dict()
    for field in SUPPLY_CHAIN_DERIVED_FIELDS:
        if payload.get(field) != derived[field]:
            errors.append(f"{field} must equal the source-derived value")


def validate_downstream_bindings(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_string_coverage(payload, "packages", "", REQUIRED_DOWNSTREAM_PACKAGES, errors)
    require_only_required_values(payload, "packages", "", REQUIRED_DOWNSTREAM_PACKAGES, errors)
    require_string_inventory_count_match(payload, "packages", "package_count", errors)
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
    require_maximum_int(
        payload,
        "smoke_duration_seconds",
        options.max_smoke_duration_secs,
        errors,
        minimum=1,
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
    require_hex(payload, "public_key_fingerprint_hex", HEX64_LEN, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    if set(payload) != frozenset(EVIDENCE_REQUIRED_FIELDS[kind.name]):
        errors.append(
            f"{kind.name} evidence fields must match the schema-closed contract"
        )
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
    elif kind.name == "supply_chain":
        validate_supply_chain(payload, errors, options)
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
    sensitivity_payload = payload
    if (
        isinstance(payload, dict)
        and payload.get("schema") == KIND_BY_NAME["supply_chain"].schema
    ):
        sensitivity_payload = dict(payload)
        for field, trusted_value in (
            (
                "provenance_certificate_identity",
                options.provenance_certificate_identity,
            ),
            ("provenance_oidc_issuer", options.provenance_oidc_issuer),
        ):
            if (
                isinstance(trusted_value, str)
                and sensitivity_payload.get(field) == trusted_value
            ):
                sensitivity_payload[field] = "<public-provenance-metadata>"
    return validate_standard_evidence_payload(
        sensitivity_payload,
        SCHEMA_TO_KIND,
        "SoraFS SF-11 release artifact",
        SENSITIVE_KEYS,
        "release evidence",
        lambda kind, _checked_payload, errors: validate_kind_specific(
            kind, payload, errors, options
        ),
        require_reviewed_deployment_context=True,
    )


def require_single_active_digest(
    digests: set[str],
    errors: list[str],
    *,
    label: str,
) -> set[str]:
    """Return one active release digest or fail closed on mixed anchors."""

    if len(digests) <= 1:
        return digests
    errors.append(f"{label} must contain exactly one active digest")
    return set()


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
    valid_release_manifest_reference_digests: set[str] = set()
    valid_policy_digests: set[str] = set()
    valid_archive_index_digests: set[str] = set()
    valid_ffi_contract_digests: set[str] = set()
    valid_header_digests: set[str] = set()
    valid_package_index_digests: set[str] = set()
    valid_provenance_bundle_digests: set[str] = set()
    valid_release_key_fingerprints: set[str] = set()
    valid_sbom_index_digests: set[str] = set()
    valid_smoke_output_digests: set[str] = set()
    valid_vulnerability_report_digests: set[str] = set()
    signature_algorithms: set[str] = set()
    valid_release_manifest_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_policy_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_release_key_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
            archive_artifact_path_label(path, evidence_dirs),
            digest,
            payload,
            validation_errors,
            FINGERPRINT_FIELDS,
        )
        fingerprint_values = validated_signature_algorithm_fingerprint_values(
            kind_name,
            payload,
        )
        if fingerprint_values:
            evidence_artifact_fingerprint(artifact).update(fingerprint_values)
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            if kind_name == "release_archive":
                archive_index_digest = fingerprint.get("archive_index_digest_hex")
                if isinstance(archive_index_digest, str):
                    valid_archive_index_digests.add(archive_index_digest)
            elif kind_name == "signed_manifest":
                digest = fingerprint.get("manifest_digest_hex")
                if isinstance(digest, str):
                    valid_release_manifest_digests.add(digest)
                policy_digest = fingerprint.get("policy_digest_hex")
                if isinstance(policy_digest, str):
                    valid_policy_digests.add(policy_digest)
                release_key_fingerprint = fingerprint.get("public_key_fingerprint_hex")
                if isinstance(release_key_fingerprint, str):
                    valid_release_key_fingerprints.add(
                        release_key_fingerprint
                    )
                signature_algorithm = fingerprint.get("signature_algorithm")
                if isinstance(signature_algorithm, str):
                    signature_algorithms.add(signature_algorithm)
            elif kind_name == "supply_chain":
                sbom_index_digest = fingerprint.get("sbom_index_digest_hex")
                if isinstance(sbom_index_digest, str):
                    valid_sbom_index_digests.add(sbom_index_digest)
                vulnerability_report_digest = fingerprint.get(
                    "vulnerability_report_digest_hex"
                )
                if isinstance(vulnerability_report_digest, str):
                    valid_vulnerability_report_digests.add(
                        vulnerability_report_digest
                    )
                provenance_bundle_digest = fingerprint.get(
                    "provenance_bundle_digest_hex"
                )
                if isinstance(provenance_bundle_digest, str):
                    valid_provenance_bundle_digests.add(provenance_bundle_digest)
            elif kind_name == "downstream_bindings":
                package_index_digest = fingerprint.get("package_index_digest_hex")
                if isinstance(package_index_digest, str):
                    valid_package_index_digests.add(package_index_digest)
            elif kind_name == "cookbook_smoke":
                smoke_output_digest = fingerprint.get("smoke_output_digest_hex")
                if isinstance(smoke_output_digest, str):
                    valid_smoke_output_digests.add(smoke_output_digest)
            elif kind_name == "ffi_header_contract":
                ffi_contract_digest = fingerprint.get("ffi_contract_digest_hex")
                if isinstance(ffi_contract_digest, str):
                    valid_ffi_contract_digests.add(ffi_contract_digest)
                header_digest = fingerprint.get("header_digest_hex")
                if isinstance(header_digest, str):
                    valid_header_digests.add(header_digest)
            if kind_name in RELEASE_MANIFEST_BOUND_KINDS:
                valid_release_manifest_bound_artifacts.append((kind_name, artifact))
            if kind_name in POLICY_BOUND_KINDS:
                valid_policy_bound_artifacts.append((kind_name, artifact))
            if kind_name in RELEASE_KEY_BOUND_KINDS:
                valid_release_key_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)

    valid_release_manifest_digests = require_single_active_digest(
        valid_release_manifest_digests,
        errors,
        label="valid_release_manifest_digests",
    )
    valid_policy_digests = require_single_active_digest(
        valid_policy_digests,
        errors,
        label="valid_policy_digests",
    )
    valid_release_key_fingerprints = require_single_active_digest(
        valid_release_key_fingerprints,
        errors,
        label="valid_release_key_fingerprints",
    )

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
    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("signed_manifest",),
        bound_artifacts=valid_policy_bound_artifacts,
        valid_anchor_digests=valid_policy_digests,
        digest_field="policy_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex must reference a valid "
            "signed_manifest policy_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} policy_digest_hex requires a valid "
            "signed_manifest policy_digest_hex"
        ),
    )
    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("signed_manifest",),
        bound_artifacts=valid_release_key_bound_artifacts,
        valid_anchor_digests=valid_release_key_fingerprints,
        digest_field="public_key_fingerprint_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} public_key_fingerprint_hex must reference a valid "
            "signed_manifest public_key_fingerprint_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} public_key_fingerprint_hex requires a valid "
            "signed_manifest public_key_fingerprint_hex"
        ),
    )

    for kind_name in RELEASE_MANIFEST_BOUND_KINDS:
        for artifact in artifacts_by_kind.get(kind_name, []):
            if not evidence_artifact_is_valid(artifact):
                continue
            fingerprint = evidence_artifact_fingerprint(artifact)
            digest = fingerprint.get("release_manifest_digest_hex")
            if isinstance(digest, str):
                valid_release_manifest_reference_digests.add(digest)

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
        "recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind),
        "valid_archive_index_digests": sorted(valid_archive_index_digests),
        "valid_ffi_contract_digests": sorted(valid_ffi_contract_digests),
        "valid_header_digests": sorted(valid_header_digests),
        "valid_package_index_digests": sorted(valid_package_index_digests),
        "valid_provenance_bundle_digests": sorted(
            valid_provenance_bundle_digests
        ),
        "valid_release_manifest_digests": sorted(valid_release_manifest_digests),
        "valid_release_manifest_reference_digests": sorted(
            valid_release_manifest_reference_digests
        ),
        "valid_release_key_fingerprints": sorted(valid_release_key_fingerprints),
        "valid_sbom_index_digests": sorted(valid_sbom_index_digests),
        "valid_smoke_output_digests": sorted(valid_smoke_output_digests),
        "valid_vulnerability_report_digests": sorted(
            valid_vulnerability_report_digests
        ),
        "valid_policy_digests": sorted(valid_policy_digests),
        "signature_algorithms": sorted(signature_algorithms),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SF-11 reference SDK release evidence artifacts."
    )
    add_topology_qualification_argument(parser)
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
        required=True,
        help="Required reviewed validator clock used for age checks.",
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
    parser.add_argument(
        "--supply-chain-source-root",
        type=Path,
        help=(
            "Root containing source artifacts named by supply_chain "
            "source_artifacts bindings."
        ),
    )
    parser.add_argument(
        "--provenance-certificate-identity",
        help="Operator-trusted provenance certificate identity.",
    )
    parser.add_argument(
        "--provenance-oidc-issuer",
        help="Operator-trusted provenance OIDC issuer.",
    )
    parser.add_argument(
        "--provenance-verification-public-key-hex",
        help="Operator-trusted non-zero raw Ed25519 verification public key.",
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
        supply_chain_source_root=args.supply_chain_source_root,
        provenance_certificate_identity=args.provenance_certificate_identity,
        provenance_oidc_issuer=args.provenance_oidc_issuer,
        provenance_verification_public_key=decode_ed25519_public_key(
            args.provenance_verification_public_key_hex
        ),
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir, args.evidence, required_kinds, options, args.summary_out
    )
    errors.extend(
        bind_lane_summary_to_topology(
            summary, args.topology_qualification_summary
        )
    )
    summary["status"] = evidence_gate_status(errors)
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
