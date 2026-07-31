#!/usr/bin/env python3
"""Build payload-free SoraFS reference SDK release evidence artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import secrets
import sys
from collections.abc import Callable, Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_reference_sdk_release_evidence import (  # noqa: E402
    ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_SMOKE_DURATION_SECS,
    DEFAULT_MIN_DOWNSTREAM_PACKAGES,
    DEFAULT_MIN_RELEASE_TARGETS,
    KIND_BY_NAME,
    RELEASE_MANIFEST_BOUND_KINDS,
    REQUIRED_DOWNSTREAM_PACKAGES,
    REQUIRED_RELEASE_TARGETS,
    ValidationOptions,
    validate_evidence_payload,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    fsync_checker_output_parent,
    write_all_checker_summary_bytes,
    validate_checker_output_parent,
)
from sorafs_path_identity import (  # noqa: E402
    diagnostic_text_is_canonical,
    error_diagnostic_label,
    path_diagnostic_label,
)
from sorafs_evidence_validation import (  # noqa: E402
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    positive_int_arg,
)
from sorafs_reference_sdk_supply_chain import (  # noqa: E402
    DEFAULT_SOURCE_ARTIFACT_PATHS,
    SupplyChainSourceResult,
    validate_supply_chain_sources,
)
from sccp_release_common import verify_ed25519  # noqa: E402


CANARY_KINDS = tuple(KIND_BY_NAME)
HEX64_LEN = 64
POLICY_DIGEST_KINDS = ("signed_manifest", "governance_approval")


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into exact strings."""

    items: list[str] = []
    for value in values:
        items.extend(value.split(","))
    return items


def validate_name_set(
    values: Iterable[str],
    *,
    allowed: Sequence[str],
    option: str,
    errors: list[str],
) -> list[str]:
    """Return allowed-order values, requiring complete known non-duplicate coverage."""

    values = tuple(values)
    allowed_set = frozenset(allowed)
    value_set = frozenset(values)
    if len(value_set) != len(values):
        errors.append(f"{option} must not contain duplicates")
    if any(name not in allowed_set for name in value_set):
        errors.append(f"{option} contains an unknown value")
    missing = [name for name in allowed if name not in value_set]
    if missing:
        errors.append(f"{option} must include every required value")
    return [name for name in allowed if name in value_set]


def validate_output_path(path: Path, errors: list[str]) -> None:
    """Reject unsafe output targets before writing a release artifact."""

    if not isinstance(path, Path):
        errors.append(f"--out `{path_diagnostic_label(path)}` must be a path")
        return
    try:
        if path.is_symlink():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a symlink")
            return
        if path.exists() and path.is_dir():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a directory")
            return
    except (OSError, RuntimeError) as error:
        del error
        errors.append(f"--out `{path_diagnostic_label(path)}` cannot be inspected")
        return
    validate_checker_output_parent(path, errors, label="--out")


def validate_hex64(value: str | None, *, option: str, errors: list[str]) -> None:
    """Validate an exact lowercase 32-byte digest hex string."""

    if (
        not isinstance(value, str)
        or len(value) != HEX64_LEN
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{option} must be exact lowercase 32-byte hex")


def decode_ed25519_public_key(
    value: str | None,
    *,
    option: str,
    errors: list[str],
) -> bytes | None:
    """Decode one non-zero raw Ed25519 public key without echoing it."""

    validate_hex64(value, option=option, errors=errors)
    if not isinstance(value, str) or len(value) != HEX64_LEN:
        return None
    try:
        public_key = bytes.fromhex(value)
    except ValueError:
        return None
    if not any(public_key):
        errors.append(f"{option} must not be the all-zero Ed25519 public key")
        return None
    return public_key


def provenance_receipt_authenticator(
    public_key: bytes,
) -> tuple[str, Callable[[str, bytes, bytes], bool]]:
    """Bind receipt authentication to one reviewed Ed25519 public key."""

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


def validate_canonical_string(value: str | None, *, label: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control/format text."""

    if not diagnostic_text_is_canonical(value):
        errors.append(f"{label} must be a non-empty canonical string")


def validate_signature_algorithm(value: str | None, *, errors: list[str]) -> None:
    """Require a governed release manifest signature algorithm label."""

    validate_canonical_string(value, label="--signature-algorithm", errors=errors)
    if value not in ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS:
        allowed = " or ".join(
            f"`{algorithm}`" for algorithm in ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS
        )
        errors.append(f"--signature-algorithm must be {allowed}")


def require_kind_options(
    args: argparse.Namespace,
    errors: list[str],
    required: Sequence[tuple[str, Any]],
) -> None:
    """Require kind-specific options by stable CLI flag."""

    for option, value in required:
        if value is None:
            errors.append(f"{option} is required for {args.kind}")


def validate_supply_chain_inputs(
    args: argparse.Namespace,
    errors: list[str],
) -> None:
    """Validate and retain the source-derived hard-cut supply-chain result."""

    local_errors: list[str] = []
    if args.target:
        local_errors.append(
            "--target is retired for supply_chain; targets are source-derived"
        )
    require_kind_options(
        args,
        local_errors,
        (
            ("--supply-chain-source-root", args.supply_chain_source_root),
            (
                "--provenance-certificate-identity",
                args.provenance_certificate_identity,
            ),
            ("--provenance-oidc-issuer", args.provenance_oidc_issuer),
            (
                "--provenance-verification-public-key-hex",
                args.provenance_verification_public_key_hex,
            ),
        ),
    )
    validate_canonical_string(
        args.provenance_certificate_identity,
        label="--provenance-certificate-identity",
        errors=local_errors,
    )
    validate_canonical_string(
        args.provenance_oidc_issuer,
        label="--provenance-oidc-issuer",
        errors=local_errors,
    )
    public_key = decode_ed25519_public_key(
        args.provenance_verification_public_key_hex,
        option="--provenance-verification-public-key-hex",
        errors=local_errors,
    )
    if local_errors:
        errors.extend(local_errors)
        return
    assert args.supply_chain_source_root is not None
    assert args.provenance_certificate_identity is not None
    assert args.provenance_oidc_issuer is not None
    assert public_key is not None

    fingerprint, authenticator = provenance_receipt_authenticator(public_key)
    result, source_errors = validate_supply_chain_sources(
        args.supply_chain_source_root,
        expected_deployment_id=args.deployment_id,
        expected_environment=args.environment,
        expected_release_manifest_digest_hex=args.release_manifest_digest_hex,
        expected_certificate_identity=args.provenance_certificate_identity,
        expected_verification_key_fingerprint_hex=fingerprint,
        verification_receipt_authenticator=authenticator,
        now_unix=args.now_unix,
        expected_oidc_issuer=args.provenance_oidc_issuer,
        release_rehearsal_path=args.release_rehearsal_path,
        sbom_index_path=args.sbom_index_path,
        vulnerability_report_path=args.vulnerability_report_path,
        provenance_bundle_path=args.provenance_bundle_path,
    )
    errors.extend(f"supply-chain source: {error}" for error in source_errors)
    if result is None:
        if not source_errors:
            errors.append("supply-chain source did not return a validated result")
        return
    if not isinstance(result, SupplyChainSourceResult):
        errors.append("supply-chain source returned an invalid result")
        return
    if args.generated_at_unix != result.generated_at_unix:
        errors.append(
            "--generated-at-unix must equal the oldest validated "
            "supply-chain source timestamp"
        )
        return
    args.supply_chain_source_result = result
    args.provenance_verification_public_key = public_key
    args.provenance_verification_key_fingerprint_hex = fingerprint


def common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by SF-11 release evidence payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "generated_at_unix": args.generated_at_unix,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free reference SDK release evidence payload."""

    payload = common_payload(args)
    if args.kind == "release_archive":
        payload.update(
            {
                "packaging_helper_used": True,
                "deterministic_archive_verified": True,
                "archive_checksums_published": True,
                "binary_checksums_published": True,
                "dist_gitkeep_only_tracked": True,
                "target_count": len(args.targets),
                "targets": args.targets,
                "archive_index_digest_hex": args.archive_index_digest_hex,
                "release_manifest_digest_hex": args.release_manifest_digest_hex,
                "raw_archives_included": False,
            }
        )
    elif args.kind == "signed_manifest":
        payload.update(
            {
                "manifest_signed": True,
                "manifest_signature_verified": True,
                "manifest_sha256_published": True,
                "governed_release_key_used": True,
                "public_key_fingerprint_recorded": True,
                "private_key_absent": True,
                "signature_algorithm": args.signature_algorithm,
                "signing_provider": args.signing_provider,
                "signing_provider_revision": args.signing_provider_revision,
                "hsm_signature_verified": True,
                "manifest_digest_hex": args.manifest_digest_hex,
                "policy_digest_hex": args.policy_digest_hex,
                "public_key_fingerprint_hex": args.public_key_fingerprint_hex,
                "raw_manifest_included": False,
            }
        )
    elif args.kind == "supply_chain":
        source_result = args.supply_chain_source_result
        assert isinstance(source_result, SupplyChainSourceResult)
        payload.update(
            {
                "generated_at_unix": source_result.generated_at_unix,
                "target_count": len(source_result.target_results),
                "target_results": [
                    target_result.to_dict()
                    for target_result in source_result.target_results
                ],
                "source_artifacts": [
                    artifact.to_dict()
                    for artifact in source_result.source_artifacts
                ],
                "release_manifest_digest_hex": (
                    source_result.release_manifest_digest_hex
                ),
                "sbom_index_digest_hex": source_result.sbom_index_digest_hex,
                "vulnerability_report_digest_hex": (
                    source_result.vulnerability_report_digest_hex
                ),
                "provenance_bundle_digest_hex": (
                    source_result.provenance_bundle_digest_hex
                ),
                "provenance_certificate_identity": (
                    args.provenance_certificate_identity
                ),
                "provenance_oidc_issuer": args.provenance_oidc_issuer,
                "provenance_verification_key_fingerprint_hex": (
                    args.provenance_verification_key_fingerprint_hex
                ),
                "raw_sboms_included": False,
                "raw_vulnerability_reports_included": False,
                "raw_provenance_included": False,
            }
        )
    elif args.kind == "downstream_bindings":
        payload.update(
            {
                "packages": args.packages,
                "package_count": len(args.packages),
                "sdk_exports_verified": True,
                "validation_outcome_contract_verified": True,
                "version_alignment_verified": True,
                "native_bridge_header_bound": True,
                "published_package_digests_recorded": True,
                "release_manifest_digest_hex": args.release_manifest_digest_hex,
                "package_index_digest_hex": args.package_index_digest_hex,
                "raw_packages_included": False,
            }
        )
    elif args.kind == "cookbook_smoke":
        payload.update(
            {
                "published_archive_smoke_passed": True,
                "cookbook_replay_passed": True,
                "fixture_bundle_validation_passed": True,
                "manifest_car_replay_passed": True,
                "validation_outcomes_emitted": True,
                "smoke_duration_seconds": args.smoke_duration_seconds,
                "release_manifest_digest_hex": args.release_manifest_digest_hex,
                "smoke_output_digest_hex": args.smoke_output_digest_hex,
                "raw_smoke_outputs_included": False,
            }
        )
    elif args.kind == "ffi_header_contract":
        payload.update(
            {
                "ci_guard_passed": True,
                "rust_exports_match_header": True,
                "selector_constants_match": True,
                "c_signatures_match": True,
                "bridge_bindings_verified": True,
                "release_manifest_digest_hex": args.release_manifest_digest_hex,
                "header_digest_hex": args.header_digest_hex,
                "ffi_contract_digest_hex": args.ffi_contract_digest_hex,
                "raw_header_included": False,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "approved": True,
                "governance_vote_recorded": True,
                "release_key_roster_bound": True,
                "release_targets_bound": True,
                "downstream_packages_bound": True,
                "smoke_evidence_bound": True,
                "governance_source": "governed_release",
                "release_manifest_digest_hex": args.release_manifest_digest_hex,
                "policy_digest_hex": args.policy_digest_hex,
                "public_key_fingerprint_hex": args.public_key_fingerprint_hex,
            }
        )
    return payload


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building release evidence."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    require_rollout_deployment_id(
        {"--deployment-id": args.deployment_id},
        errors,
        field="--deployment-id",
    )
    require_rollout_environment(
        {"--environment": args.environment},
        errors,
        field="--environment",
    )
    if args.kind == "signed_manifest":
        validate_hex64(
            args.manifest_digest_hex,
            option="--manifest-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.public_key_fingerprint_hex,
            option="--public-key-fingerprint-hex",
            errors=errors,
        )
        validate_signature_algorithm(args.signature_algorithm, errors=errors)
        require_kind_options(
            args,
            errors,
            (
                ("--signing-provider", args.signing_provider),
                ("--signing-provider-revision", args.signing_provider_revision),
            ),
        )
        if args.signing_provider != "external_ed25519_hsm":
            errors.append("--signing-provider must be `external_ed25519_hsm`")
    if args.kind == "governance_approval":
        require_kind_options(
            args,
            errors,
            (("--public-key-fingerprint-hex", args.public_key_fingerprint_hex),),
        )
        validate_hex64(
            args.public_key_fingerprint_hex,
            option="--public-key-fingerprint-hex",
            errors=errors,
        )
    if args.kind in RELEASE_MANIFEST_BOUND_KINDS:
        validate_hex64(
            args.release_manifest_digest_hex,
            option="--release-manifest-digest-hex",
            errors=errors,
        )

    if args.kind == "release_archive":
        args.targets = validate_name_set(
            split_csv_values(args.target),
            allowed=REQUIRED_RELEASE_TARGETS,
            option="--target",
            errors=errors,
        )
    if args.kind == "release_archive":
        validate_hex64(
            args.archive_index_digest_hex,
            option="--archive-index-digest-hex",
            errors=errors,
        )
    elif args.kind == "supply_chain":
        validate_supply_chain_inputs(args, errors)
    elif args.kind == "downstream_bindings":
        args.packages = validate_name_set(
            split_csv_values(args.package),
            allowed=REQUIRED_DOWNSTREAM_PACKAGES,
            option="--package",
            errors=errors,
        )
        validate_hex64(
            args.package_index_digest_hex,
            option="--package-index-digest-hex",
            errors=errors,
        )
    elif args.kind == "cookbook_smoke":
        if args.smoke_duration_seconds > DEFAULT_MAX_SMOKE_DURATION_SECS:
            errors.append(
                f"--smoke-duration-seconds must be <= {DEFAULT_MAX_SMOKE_DURATION_SECS}"
            )
        validate_hex64(
            args.smoke_output_digest_hex,
            option="--smoke-output-digest-hex",
            errors=errors,
        )
    elif args.kind == "ffi_header_contract":
        validate_hex64(
            args.header_digest_hex,
            option="--header-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.ffi_contract_digest_hex,
            option="--ffi-contract-digest-hex",
            errors=errors,
        )
    if args.kind in POLICY_DIGEST_KINDS:
        require_kind_options(
            args,
            errors,
            (("--policy-digest-hex", args.policy_digest_hex),),
        )
        validate_hex64(
            args.policy_digest_hex,
            option="--policy-digest-hex",
            errors=errors,
        )
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate generated release evidence."""

    return ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        min_release_targets=DEFAULT_MIN_RELEASE_TARGETS,
        min_downstream_packages=DEFAULT_MIN_DOWNSTREAM_PACKAGES,
        max_smoke_duration_secs=DEFAULT_MAX_SMOKE_DURATION_SECS,
        supply_chain_source_root=(
            args.supply_chain_source_root
            if args.kind == "supply_chain"
            else None
        ),
        provenance_certificate_identity=(
            args.provenance_certificate_identity
            if args.kind == "supply_chain"
            else None
        ),
        provenance_oidc_issuer=(
            args.provenance_oidc_issuer
            if args.kind == "supply_chain"
            else None
        ),
        provenance_verification_public_key=(
            args.provenance_verification_public_key
            if args.kind == "supply_chain"
            else None
        ),
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate generated release evidence through the SF-11 gate contract."""

    kind, errors = validate_evidence_payload(payload, validation_options(args))
    if kind != args.kind:
        errors.append(f"generated release evidence must validate as {args.kind}")
    return errors


def write_payload_atomic(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write the release evidence JSON atomically without following symlinks."""

    text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    parent = path.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        parent_label = path_diagnostic_label(parent)
        return [
            f"--out parent `{parent_label}` cannot be created: "
            f"{error_diagnostic_label(error, path_label=parent_label)}"
        ]
    tmp_name = f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    tmp_path = parent / tmp_name
    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        if nofollow:
            flags |= nofollow
        fd = os.open(tmp_path, flags, 0o600)
        write_all_checker_summary_bytes(fd, text.encode("utf-8"))
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.replace(tmp_path, path)
        parent_sync_errors = fsync_checker_output_parent(path, label="--out")
        if parent_sync_errors:
            return parent_sync_errors
    except (OSError, RuntimeError) as error:
        path_label = path_diagnostic_label(path)
        try:
            if fd >= 0:
                os.close(fd)
        finally:
            try:
                tmp_path.unlink()
            except FileNotFoundError:
                pass
            except (OSError, RuntimeError):
                pass
        return [
            f"--out `{path_label}` cannot be written: "
            f"{error_diagnostic_label(error, path_label=path_label)}"
        ]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SF-11 reference SDK release evidence.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
    parser.add_argument("--release-manifest-digest-hex")
    parser.add_argument("--manifest-digest-hex")
    parser.add_argument("--archive-index-digest-hex")
    parser.add_argument("--package-index-digest-hex")
    parser.add_argument("--smoke-output-digest-hex")
    parser.add_argument("--header-digest-hex")
    parser.add_argument("--ffi-contract-digest-hex")
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--public-key-fingerprint-hex")
    parser.add_argument("--signature-algorithm", default="ed25519")
    parser.add_argument("--signing-provider")
    parser.add_argument("--signing-provider-revision", type=positive_int_arg)
    parser.add_argument("--target", action="append", default=[])
    parser.add_argument("--package", action="append", default=[])
    parser.add_argument("--smoke-duration-seconds", type=positive_int_arg, default=600)
    parser.add_argument("--supply-chain-source-root", type=Path)
    parser.add_argument(
        "--release-rehearsal-path",
        default=DEFAULT_SOURCE_ARTIFACT_PATHS["release_rehearsal"],
        help="Exact relative v1 release-rehearsal source path.",
    )
    parser.add_argument(
        "--sbom-index-path",
        default=DEFAULT_SOURCE_ARTIFACT_PATHS["sbom_index"],
        help="Exact relative v1 SBOM-index source path.",
    )
    parser.add_argument(
        "--vulnerability-report-path",
        default=DEFAULT_SOURCE_ARTIFACT_PATHS["vulnerability_report"],
        help="Exact relative v1 vulnerability-report source path.",
    )
    parser.add_argument(
        "--provenance-bundle-path",
        default=DEFAULT_SOURCE_ARTIFACT_PATHS["provenance_bundle"],
        help="Exact relative v1 provenance-bundle source path.",
    )
    parser.add_argument("--provenance-certificate-identity")
    parser.add_argument("--provenance-oidc-issuer")
    parser.add_argument("--provenance-verification-public-key-hex")
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
        return parser.parse_args(expanded_args)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    errors = validate_inputs(args)
    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS reference SDK release evidence inputs are incomplete:",
            errors,
        )
        return 2

    payload = build_payload(args)
    payload_errors = validate_generated_payload(payload, args)
    if payload_errors:
        emit_checker_error_lines(payload_errors)
        return 2

    write_errors = write_payload_atomic(args.out, payload)
    if write_errors:
        emit_checker_error_lines(write_errors)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
