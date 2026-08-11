#!/usr/bin/env python3
"""Collect and verify SoraFS reference SDK release evidence."""

from __future__ import annotations

import argparse
import hashlib
import secrets
import sys
import unicodedata
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

BUNDLED_VERIFIER = SCRIPT_DIR / "check_sorafs_reference_sdk_release_evidence.py"

from check_sorafs_reference_sdk_release_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_SMOKE_DURATION_SECS,
    DEFAULT_MIN_DOWNSTREAM_PACKAGES,
    DEFAULT_MIN_RELEASE_TARGETS,
    DEFAULT_REQUIRED_KINDS,
    EVIDENCE_REQUIRED_FIELDS,
    INDEPENDENT_VERIFICATION_KEYS_ERROR,
    KIND_BY_NAME,
    SUMMARY_SCHEMA,
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
from sorafs_runner_preflight import (  # noqa: E402
    canonical_runner_plan_string,
    emit_runner_error_block,
    emit_runner_error_lines,
    emit_runner_exception,
    run_command_plan,
    require_existing_files,
    require_no_unrequired_evidence,
    require_runner_non_negative_int,
    require_runner_positive_int,
    validate_runner_evidence_plan,
    validate_runner_output_dir,
    validate_runner_plan_steps,
    validate_runner_preflight,
    write_runner_plan,
)


from sorafs_topology_qualification import (  # noqa: E402
    add_signed_topology_qualification_arguments,
)

PLAN_SCHEMA = "sorafs.reference_sdk.release_evidence_collection_plan.v1"
PLAN_FIELDS = frozenset(
    {
        "schema",
        "verifier_summary_schema",
        "required_kinds",
        "thresholds",
        "external_evidence",
        "evidence_contract",
        "supply_chain_source",
        "topology_qualification",
        "steps",
    }
)
SUPPLY_CHAIN_SOURCE_PLAN_FIELDS = frozenset(
    {
        "required",
        "source_root",
        "provenance_certificate_identity",
        "provenance_oidc_issuer",
        "provenance_verification_key_fingerprint_hex",
    }
)
TOPOLOGY_QUALIFICATION_PLAN_FIELDS = frozenset(
    {
        "summary_path",
        "envelope_path",
        "verification_public_key_fingerprint_hex",
        "signer_service_id",
        "signer_administrator_id",
        "signer_key_revision",
        "signer_policy_revision",
        "signer_policy_digest_hex",
    }
)
PLAN_REQUIRED_THRESHOLD_FIELDS = frozenset(
    {
        "max_evidence_age_secs",
        "max_topology_qualification_review_age_secs",
        "min_release_targets",
        "min_downstream_packages",
        "max_smoke_duration_secs",
        "now_unix",
    }
)
PLAN_POSITIVE_THRESHOLD_FIELDS = frozenset(
    {
        "min_release_targets",
        "min_downstream_packages",
        "max_smoke_duration_secs",
        "now_unix",
    }
)
PLAN_NON_NEGATIVE_THRESHOLD_FIELDS = frozenset(
    {
        "max_evidence_age_secs",
        "max_topology_qualification_review_age_secs",
    }
)


@dataclass(frozen=True)
class CommandPlan:
    """One reference SDK release evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




EVIDENCE_OPTIONS_BY_KIND = {
    "release_archive": "release_archive_evidence",
    "signed_manifest": "signed_manifest_evidence",
    "supply_chain": "supply_chain_evidence",
    "downstream_bindings": "downstream_bindings_evidence",
    "cookbook_smoke": "cookbook_smoke_evidence",
    "ffi_header_contract": "ffi_header_contract_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "release_archive": "--release-archive-evidence",
    "signed_manifest": "--signed-manifest-evidence",
    "supply_chain": "--supply-chain-evidence",
    "downstream_bindings": "--downstream-bindings-evidence",
    "cookbook_smoke": "--cookbook-smoke-evidence",
    "ffi_header_contract": "--ffi-header-contract-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied release evidence paths keyed by SF-11 evidence kind."""

    return {
        kind: list(getattr(args, option))
        for kind, option in EVIDENCE_OPTIONS_BY_KIND.items()
    }


def supply_chain_source_required(args: argparse.Namespace) -> bool:
    """Return whether this plan must re-open the SF-11 source bundle."""

    return "supply_chain" in args.required_kinds


def provenance_verification_key_fingerprint(
    public_key_hex: Any,
) -> str | None:
    """Return the trusted raw Ed25519 public-key fingerprint when canonical."""

    if (
        not isinstance(public_key_hex, str)
        or len(public_key_hex) != 64
        or any(character not in "0123456789abcdef" for character in public_key_hex)
    ):
        return None
    public_key = bytes.fromhex(public_key_hex)
    if not any(public_key):
        return None
    return hashlib.sha256(public_key).hexdigest()


def canonical_nonzero_sha256(value: Any) -> str | None:
    """Return one canonical non-zero SHA-256 digest without echoing failures."""

    if (
        not isinstance(value, str)
        or len(value) != 64
        or value != value.lower()
    ):
        return None
    try:
        digest = bytes.fromhex(value)
    except ValueError:
        return None
    return value if any(digest) else None


def topology_qualification_plan(args: argparse.Namespace) -> dict[str, object]:
    """Return the payload-free signed-topology trust binding for a runner plan."""

    return {
        "summary_path": str(args.topology_qualification_summary),
        "envelope_path": str(args.topology_qualification_envelope),
        "verification_public_key_fingerprint_hex": (
            provenance_verification_key_fingerprint(
                args.topology_qualification_verification_public_key_hex
            )
        ),
        "signer_service_id": args.topology_qualification_signer_service_id,
        "signer_administrator_id": (
            args.topology_qualification_signer_administrator_id
        ),
        "signer_key_revision": args.topology_qualification_signer_key_revision,
        "signer_policy_revision": (
            args.topology_qualification_signer_policy_revision
        ),
        "signer_policy_digest_hex": (
            args.topology_qualification_signer_policy_digest_hex
        ),
    }


def supply_chain_source_plan(args: argparse.Namespace) -> dict[str, object]:
    """Return the payload-free source trust configuration for the dry-run plan."""

    required = supply_chain_source_required(args)
    fingerprint = provenance_verification_key_fingerprint(
        args.provenance_verification_public_key_hex
    )
    return {
        "required": required,
        "source_root": (
            str(args.supply_chain_source_root)
            if required and args.supply_chain_source_root is not None
            else None
        ),
        "provenance_certificate_identity": (
            args.provenance_certificate_identity if required else None
        ),
        "provenance_oidc_issuer": (
            args.provenance_oidc_issuer if required else None
        ),
        "provenance_verification_key_fingerprint_hex": (
            fingerprint if required else None
        ),
    }


def validate_supply_chain_source_inputs(
    args: argparse.Namespace,
    errors: list[str],
) -> None:
    """Require source-root and provenance trust inputs exactly when SF-11 needs them."""

    required = supply_chain_source_required(args)
    configured_values = (
        args.supply_chain_source_root,
        args.provenance_certificate_identity,
        args.provenance_oidc_issuer,
        args.provenance_verification_public_key_hex,
    )
    if not required:
        if any(value is not None for value in configured_values):
            errors.append(
                "supply-chain source inputs require the `supply_chain` evidence kind"
            )
        return

    if args.supply_chain_source_root is None:
        errors.append("--supply-chain-source-root is required for supply_chain")
    else:
        validate_runner_output_dir(
            args.supply_chain_source_root,
            errors,
            label="--supply-chain-source-root",
            require_exists=True,
        )
    if canonical_runner_plan_string(args.provenance_certificate_identity) is None:
        errors.append(
            "--provenance-certificate-identity is required and must be canonical"
        )
    if canonical_runner_plan_string(args.provenance_oidc_issuer) is None:
        errors.append("--provenance-oidc-issuer is required and must be canonical")
    if (
        provenance_verification_key_fingerprint(
            args.provenance_verification_public_key_hex
        )
        is None
    ):
        errors.append(
            "--provenance-verification-public-key-hex must be a non-zero "
            "raw 32-byte Ed25519 public key in lowercase hex"
        )


def validate_topology_qualification_inputs(
    args: argparse.Namespace,
    errors: list[str],
) -> None:
    """Require the independent topology signer trust tuple without echoing it."""

    signer_ids = (
        ("service-id", args.topology_qualification_signer_service_id),
        ("administrator-id", args.topology_qualification_signer_administrator_id),
    )
    for option, signer_id in signer_ids:
        if (
            canonical_runner_plan_string(signer_id) is None
            or len(signer_id.encode("utf-8", "surrogatepass")) > 128
            or signer_id != unicodedata.normalize("NFC", signer_id)
        ):
            errors.append(f"--topology-qualification-signer-{option} must be canonical")
    if signer_ids[0][1] == signer_ids[1][1]:
        errors.append("topology signer service-id and administrator-id must differ")
    topology_key_fingerprint = provenance_verification_key_fingerprint(
        args.topology_qualification_verification_public_key_hex
    )
    if topology_key_fingerprint is None:
        errors.append(
            "--topology-qualification-verification-public-key-hex must be a "
            "non-zero raw 32-byte Ed25519 public key in lowercase hex"
        )
    if (
        canonical_nonzero_sha256(
            args.topology_qualification_signer_policy_digest_hex
        )
        is None
    ):
        errors.append(
            "--topology-qualification-signer-policy-digest-hex must be a "
            "canonical non-zero SHA-256 digest"
        )
    provenance_key_fingerprint = provenance_verification_key_fingerprint(
        args.provenance_verification_public_key_hex
    )
    if (
        topology_key_fingerprint is not None
        and provenance_key_fingerprint is not None
        and secrets.compare_digest(
            topology_key_fingerprint,
            provenance_key_fingerprint,
        )
    ):
        errors.append(INDEPENDENT_VERIFICATION_KEYS_ERROR)


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(
        args,
        summary_filename="release-summary.json",
        bundled_verifier=BUNDLED_VERIFIER,
    )
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    paths_by_kind = evidence_paths_by_kind(args)
    for kind in args.required_kinds:
        paths = paths_by_kind[kind]
        if not paths:
            errors.append(
                "missing required release evidence input"
            )
    require_no_unrequired_evidence(
        paths_by_kind,
        args.required_kinds,
        errors,
        diagnostic="release evidence supplied for unrequired kind",
    )

    for kind, paths in paths_by_kind.items():
        errors.extend(require_existing_files(paths, EVIDENCE_FLAGS_BY_KIND[kind], seen=seen_input_files))

    errors.extend(
        require_existing_files(
            [args.topology_qualification_summary],
            "--topology-qualification-summary",
            seen=seen_input_files,
        )
    )
    errors.extend(
        require_existing_files(
            [args.topology_qualification_envelope],
            "--topology-qualification-envelope",
            seen=seen_input_files,
        )
    )

    require_runner_positive_int(args, "now_unix", errors)
    require_runner_non_negative_int(args, "max_evidence_age_secs", errors)
    require_runner_non_negative_int(
        args,
        "max_topology_qualification_review_age_secs",
        errors,
    )
    require_runner_positive_int(args, "min_release_targets", errors)
    require_runner_positive_int(args, "min_downstream_packages", errors)
    require_runner_positive_int(args, "max_smoke_duration_secs", errors)
    require_runner_positive_int(
        args,
        "topology_qualification_signer_key_revision",
        errors,
    )
    validate_topology_qualification_inputs(args, errors)
    validate_supply_chain_source_inputs(args, errors)
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    summary_out = args.summary_out or args.out_dir / "release-summary.json"
    verifier_command = [
        sys.executable,
        str(BUNDLED_VERIFIER),
    ]
    for paths in evidence_paths_by_kind(args).values():
        for path in paths:
            verifier_command.extend(["--evidence", str(path)])
    for required_kind in args.required_kinds:
        verifier_command.extend(["--require-kind", required_kind])
    verifier_command.extend(
        [
            "--summary-out",
            str(summary_out),
            "--topology-qualification-summary",
            str(args.topology_qualification_summary),
            "--topology-qualification-envelope",
            str(args.topology_qualification_envelope),
            "--topology-qualification-verification-public-key-hex",
            args.topology_qualification_verification_public_key_hex,
            "--topology-qualification-signer-service-id",
            args.topology_qualification_signer_service_id,
            "--topology-qualification-signer-administrator-id",
            args.topology_qualification_signer_administrator_id,
            "--topology-qualification-signer-key-revision",
            str(args.topology_qualification_signer_key_revision),
            "--topology-qualification-signer-policy-revision",
            str(args.topology_qualification_signer_policy_revision),
            "--topology-qualification-signer-policy-digest-hex",
            args.topology_qualification_signer_policy_digest_hex,
            "--max-topology-qualification-review-age-secs",
            str(args.max_topology_qualification_review_age_secs),
            "--max-evidence-age-secs",
            str(args.max_evidence_age_secs),
            "--min-release-targets",
            str(args.min_release_targets),
            "--min-downstream-packages",
            str(args.min_downstream_packages),
            "--max-smoke-duration-secs",
            str(args.max_smoke_duration_secs),
        ]
    )
    verifier_command.extend(["--now-unix", str(args.now_unix)])
    if supply_chain_source_required(args):
        verifier_command.extend(
            [
                "--supply-chain-source-root",
                str(args.supply_chain_source_root),
                "--provenance-certificate-identity",
                args.provenance_certificate_identity,
                "--provenance-oidc-issuer",
                args.provenance_oidc_issuer,
                "--provenance-verification-public-key-hex",
                args.provenance_verification_public_key_hex,
            ]
        )

    return [CommandPlan("release_evidence_gate", summary_out, verifier_command)]


def threshold_values(args: argparse.Namespace) -> dict[str, int]:
    """Return threshold values rendered in dry-run plans."""

    thresholds: dict[str, int] = {
        "max_evidence_age_secs": args.max_evidence_age_secs,
        "max_topology_qualification_review_age_secs": (
            args.max_topology_qualification_review_age_secs
        ),
        "min_release_targets": args.min_release_targets,
        "min_downstream_packages": args.min_downstream_packages,
        "max_smoke_duration_secs": args.max_smoke_duration_secs,
        "now_unix": args.now_unix,
    }
    return thresholds


def external_evidence(args: argparse.Namespace) -> dict[str, list[str]]:
    """Return reviewed external evidence paths rendered in dry-run plans."""

    return {
        kind: [str(path) for path in paths]
        for kind, paths in evidence_paths_by_kind(args).items()
        if paths
    }


def evidence_contract(args: argparse.Namespace) -> dict[str, dict[str, object]]:
    """Return the checker-backed evidence contract rendered in dry-run plans."""

    return {
        kind: {
            "schema": KIND_BY_NAME[kind].schema,
            "required_payload_fields": list(EVIDENCE_REQUIRED_FIELDS[kind]),
        }
        for kind in args.required_kinds
    }


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    return {
        "schema": PLAN_SCHEMA,
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "required_kinds": list(args.required_kinds),
        "thresholds": threshold_values(args),
        "external_evidence": external_evidence(args),
        "evidence_contract": evidence_contract(args),
        "supply_chain_source": supply_chain_source_plan(args),
        "topology_qualification": topology_qualification_plan(args),
        "steps": [
            {
                "label": step.label,
                "artifact": None if step.artifact is None else str(step.artifact),
                "command": list(step.command),
            }
            for step in plan
        ],
    }


def validate_plan_json(
    rendered: object,
    plan: Sequence[CommandPlan],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the SF-11 collection-plan envelope before use."""

    errors = validate_runner_evidence_plan(
        rendered,
        plan,
        diagnostic_prefix="reference SDK release runner plan",
        plan_schema=PLAN_SCHEMA,
        plan_fields=PLAN_FIELDS,
        summary_schema=SUMMARY_SCHEMA,
        required_kinds=args.required_kinds,
        known_kinds=KIND_BY_NAME,
        thresholds=threshold_values(args),
        required_threshold_fields=PLAN_REQUIRED_THRESHOLD_FIELDS,
        positive_threshold_fields=PLAN_POSITIVE_THRESHOLD_FIELDS,
        non_negative_threshold_fields=PLAN_NON_NEGATIVE_THRESHOLD_FIELDS,
        external_evidence=external_evidence(args),
        evidence_contract=evidence_contract(args),
        evidence_required_fields=EVIDENCE_REQUIRED_FIELDS,
    )
    if not isinstance(rendered, Mapping):
        return errors
    topology = rendered.get("topology_qualification")
    if not isinstance(topology, Mapping):
        errors.append(
            "reference SDK release runner plan topology_qualification "
            "must be an object"
        )
    else:
        if any(canonical_runner_plan_string(field) is None for field in topology):
            errors.append(
                "reference SDK release runner plan topology_qualification "
                "fields must be canonical strings"
            )
        if set(topology) != TOPOLOGY_QUALIFICATION_PLAN_FIELDS:
            errors.append(
                "reference SDK release runner plan topology_qualification "
                "fields must match the schema-closed contract"
            )
        for field in (
            "summary_path",
            "envelope_path",
            "signer_service_id",
            "signer_administrator_id",
        ):
            if canonical_runner_plan_string(topology.get(field)) is None:
                errors.append(
                    "reference SDK release runner plan topology_qualification "
                    "string fields must be canonical"
                )
                break
        if (
            provenance_verification_key_fingerprint(
                args.topology_qualification_verification_public_key_hex
            )
            is None
            or canonical_nonzero_sha256(
                topology.get("verification_public_key_fingerprint_hex")
            )
            is None
        ):
            errors.append(
                "reference SDK release runner plan topology_qualification "
                "verification key fingerprint must be canonical"
            )
        if (
            canonical_nonzero_sha256(
                topology.get("signer_policy_digest_hex")
            )
            is None
        ):
            errors.append(
                "reference SDK release runner plan topology_qualification "
                "signer policy digest must be canonical"
            )
        for field in ("signer_key_revision", "signer_policy_revision"):
            revision = topology.get(field)
            if not isinstance(revision, int) or isinstance(revision, bool) or revision <= 0:
                errors.append(
                    "reference SDK release runner plan topology_qualification "
                    f"{field.replace('_', ' ')} must be positive"
                )
        if topology != topology_qualification_plan(args):
            errors.append(
                "reference SDK release runner plan topology_qualification "
                "must match args"
            )
    source = rendered.get("supply_chain_source")
    if not isinstance(source, Mapping):
        errors.append(
            "reference SDK release runner plan supply_chain_source must be an object"
        )
    else:
        if any(canonical_runner_plan_string(field) is None for field in source):
            errors.append(
                "reference SDK release runner plan supply_chain_source fields "
                "must be canonical strings"
            )
        if set(source) != SUPPLY_CHAIN_SOURCE_PLAN_FIELDS:
            errors.append(
                "reference SDK release runner plan supply_chain_source fields "
                "must match the schema-closed contract"
            )
        if not isinstance(source.get("required"), bool):
            errors.append(
                "reference SDK release runner plan supply_chain_source.required "
                "must be boolean"
            )
        expected = supply_chain_source_plan(args)
        if source != expected:
            errors.append(
                "reference SDK release runner plan supply_chain_source must match args"
            )
    return errors


def run_plan(plan: Sequence[CommandPlan], out_dir: Path) -> int:
    return run_command_plan(plan, out_dir)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Collect and verify SoraFS SF-11 reference SDK release evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=BUNDLED_VERIFIER,
        help="Bundled release evidence verifier path; substitutions are rejected.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        required=True,
        help="Directory where the verifier summary will be written.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional verifier summary path. Defaults under --out-dir.",
    )
    parser.add_argument(
        "--require-kind",
        action="append",
        default=[],
        help=(
            "Required evidence kind, or comma-separated kinds. "
            "Defaults to every SF-11 release kind."
        ),
    )
    for kind, flag in EVIDENCE_FLAGS_BY_KIND.items():
        parser.add_argument(
            flag,
            dest=EVIDENCE_OPTIONS_BY_KIND[kind],
            action="append",
            type=Path,
            default=[],
            help=f"Existing JSON artifact for `{kind}` release evidence.",
        )
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )
    parser.add_argument("--min-release-targets", type=positive_int_arg, default=DEFAULT_MIN_RELEASE_TARGETS)
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
        help="Root containing the exact source artifacts bound by the supply-chain canary.",
    )
    parser.add_argument(
        "--provenance-certificate-identity",
        help="Expected OIDC certificate identity for supply-chain provenance.",
    )
    parser.add_argument(
        "--provenance-oidc-issuer",
        help="Expected OIDC issuer for supply-chain provenance.",
    )
    parser.add_argument(
        "--provenance-verification-public-key-hex",
        help="Trusted raw Ed25519 key authenticating provenance verification receipts.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the verifier command plan without executing it.",
    )
    add_signed_topology_qualification_arguments(parser)
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    args = parser.parse_args(expanded_args)
    try:
        args.required_kinds = parse_required_evidence_kinds(
            args.require_kind,
            allowed_kinds=KIND_BY_NAME,
            default_required=DEFAULT_REQUIRED_KINDS,
        )
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    return args


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1
    errors = validate_inputs(args)
    if errors:
        emit_runner_error_lines(errors)
        return 2

    plan = build_command_plan(args)
    rendered_plan = plan_json(plan, args)
    plan_errors = validate_plan_json(rendered_plan, plan, args)
    if plan_errors:
        emit_runner_error_lines(plan_errors)
        return 2
    if args.dry_run:
        plan_errors = write_runner_plan(rendered_plan)
        if plan_errors:
            emit_runner_error_lines(plan_errors)
            return 2
        return 0

    return run_plan(plan, args.out_dir)


if __name__ == "__main__":
    raise SystemExit(main())
