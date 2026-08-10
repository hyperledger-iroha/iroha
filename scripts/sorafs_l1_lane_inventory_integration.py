#!/usr/bin/env python3
"""Shared hard-cut integration for the signed SoraFS L1 lane inventory."""

from __future__ import annotations

import argparse
import hashlib
import unicodedata
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping, Sequence

import sorafs_l1_lane_evidence_inventory as lane_inventory
from sorafs_production_readiness_contract import (
    AGGREGATE_L1_LANE_EVIDENCE_INVENTORY_FIELDS,
    AGGREGATE_L1_LANE_EVIDENCE_INVENTORY_SCHEMA,
    DEFAULT_REQUIRED_GATES,
)


INVENTORY_ARGUMENT_PREFIX = "--l1-lane-evidence-inventory-"
VERIFICATION_FIELDS = frozenset(
    {
        "schema",
        "status",
        "signer_qualification",
        "inventory_sha256",
        "summary_file_count",
        "recognized_summary_count",
        "deployment",
        "anchors",
        "signer",
    }
)
VERIFICATION_SIGNER_FIELDS = lane_inventory.SIGNER_BASE_FIELDS
PLAN_FIELDS = frozenset(
    {
        "schema",
        "inventory",
        "inventory_sha256",
        "signer_backend",
        "signer_service_id",
        "signer_administrator_id",
        "signer_key_revision",
        "signer_policy_revision",
        "signer_policy_digest_sha256",
        "signer_public_key_fingerprint_sha256",
    }
)


@dataclass(frozen=True)
class VerifiedLaneInventory:
    """Authenticated inventory plus its internal exact-byte lane bindings."""

    verification: dict[str, Any]
    summary_sha256: dict[str, str]


def _positive_revision(value: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    if parsed <= 0 or parsed > lane_inventory.MAX_INTEGER:
        raise argparse.ArgumentTypeError("must be in 1..2^63-1")
    return parsed


def add_signed_lane_inventory_arguments(
    parser: argparse.ArgumentParser,
    *,
    summary_flag: str | None = None,
) -> None:
    """Add one mandatory signed inventory and its explicit public trust tuple."""

    parser.add_argument(
        "--l1-lane-evidence-inventory",
        required=True,
        type=Path,
        help="Exact signed payload-free inventory for the canonical 17 lanes.",
    )
    parser.add_argument(
        f"{INVENTORY_ARGUMENT_PREFIX}verification-public-key-hex",
        required=True,
    )
    parser.add_argument(
        f"{INVENTORY_ARGUMENT_PREFIX}signer-service-id",
        required=True,
    )
    parser.add_argument(
        f"{INVENTORY_ARGUMENT_PREFIX}signer-administrator-id",
        required=True,
    )
    parser.add_argument(
        f"{INVENTORY_ARGUMENT_PREFIX}signer-key-revision",
        required=True,
        type=_positive_revision,
    )
    parser.add_argument(
        f"{INVENTORY_ARGUMENT_PREFIX}signer-policy-revision",
        required=True,
        type=_positive_revision,
    )
    parser.add_argument(
        f"{INVENTORY_ARGUMENT_PREFIX}signer-policy-digest-sha256",
        required=True,
    )
    if summary_flag is not None:
        parser.add_argument(
            summary_flag,
            action="append",
            default=[],
            required=True,
            metavar="GATE=PATH",
            help="Repeat exactly 17 times in canonical lane order.",
        )


def expected_signer_from_args(args: argparse.Namespace) -> dict[str, Any]:
    """Validate and return the operator-trusted inventory signer binding."""

    return lane_inventory.trusted_signer_binding(
        args.l1_lane_evidence_inventory_verification_public_key_hex,
        service_id=args.l1_lane_evidence_inventory_signer_service_id,
        administrator_id=(
            args.l1_lane_evidence_inventory_signer_administrator_id
        ),
        key_revision=args.l1_lane_evidence_inventory_signer_key_revision,
        policy_revision=args.l1_lane_evidence_inventory_signer_policy_revision,
        policy_digest_sha256=(
            args.l1_lane_evidence_inventory_signer_policy_digest_sha256
        ),
    )


def inventory_trust_from_args(
    args: argparse.Namespace,
    topology: Mapping[str, Any],
    *,
    deployment_id: Any,
    environment: Any,
    now_unix: Any,
) -> dict[str, Any]:
    """Derive inventory anchors only from the authenticated topology binding."""

    return {
        "deployment_id": deployment_id,
        "environment": environment,
        "evaluation_now": now_unix,
        "verification_public_key_hex": (
            args.l1_lane_evidence_inventory_verification_public_key_hex
        ),
        "service_id": args.l1_lane_evidence_inventory_signer_service_id,
        "administrator_id": (
            args.l1_lane_evidence_inventory_signer_administrator_id
        ),
        "key_revision": args.l1_lane_evidence_inventory_signer_key_revision,
        "policy_revision": args.l1_lane_evidence_inventory_signer_policy_revision,
        "policy_digest_sha256": (
            args.l1_lane_evidence_inventory_signer_policy_digest_sha256
        ),
        "expected_topology_qualification_summary_sha256": topology.get(
            "qualification_summary_sha256"
        ),
        "expected_topology_manifest_sha256": topology.get("manifest_sha256"),
        "expected_topology_canonical_manifest_sha256": topology.get(
            "canonical_manifest_sha256"
        ),
        "expected_validator_ids_sha256": topology.get("validator_ids_sha256"),
    }


def verify_inventory_from_args(
    args: argparse.Namespace,
    summary_specs: Sequence[tuple[str, Path]],
    topology: Mapping[str, Any],
    *,
    deployment_id: Any,
    environment: Any,
    now_unix: Any,
) -> tuple[VerifiedLaneInventory | None, list[str]]:
    """Load, authenticate, and replay one inventory against exact lane bytes."""

    try:
        inventory, _raw = lane_inventory.load_canonical_inventory_file(
            args.l1_lane_evidence_inventory
        )
        verification = lane_inventory.verify_inventory(
            inventory,
            summary_specs,
            **inventory_trust_from_args(
                args,
                topology,
                deployment_id=deployment_id,
                environment=environment,
                now_unix=now_unix,
            ),
        )
        summary_sha256 = {
            row["lane"]: row["summary_sha256"]
            for row in inventory["summaries"]
        }
    except (OSError, RuntimeError, lane_inventory.InventoryError, ValueError) as error:
        return None, [f"signed L1 lane evidence inventory: {error}"]
    if tuple(summary_sha256) != DEFAULT_REQUIRED_GATES:
        return None, ["signed L1 lane evidence inventory lane order drifted"]
    return VerifiedLaneInventory(verification, summary_sha256), []


def inventory_plan_from_args(
    args: argparse.Namespace,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Bind one exact inventory file and reviewed signer tuple for runner plans."""

    try:
        inventory, raw = lane_inventory.load_canonical_inventory_file(
            args.l1_lane_evidence_inventory
        )
        expected_signer = expected_signer_from_args(args)
    except (OSError, RuntimeError, lane_inventory.InventoryError, ValueError) as error:
        return None, [f"signed L1 lane evidence inventory: {error}"]
    actual_signer = dict(inventory["signer"])
    actual_signer.pop("signature_hex", None)
    if actual_signer != expected_signer:
        return None, [
            "signed L1 lane evidence inventory signer must match the reviewed trust tuple"
        ]
    return {
        "schema": lane_inventory.INVENTORY_SCHEMA,
        "inventory": str(args.l1_lane_evidence_inventory),
        "inventory_sha256": hashlib.sha256(raw).hexdigest(),
        **{
            f"signer_{field}": expected_signer[field]
            for field in (
                "backend",
                "service_id",
                "administrator_id",
                "key_revision",
                "policy_revision",
                "policy_digest_sha256",
                "public_key_fingerprint_sha256",
            )
        },
    }, []


def _canonical_text(value: Any) -> str | None:
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or unicodedata.normalize("NFC", value) != value
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        return None
    return value


def _sha256(value: Any) -> str | None:
    if not isinstance(value, str) or len(value) != 64 or value != value.lower():
        return None
    try:
        decoded = bytes.fromhex(value)
    except ValueError:
        return None
    return value if any(decoded) else None


def validate_verification_binding(
    value: Any,
    errors: list[str],
    *,
    path: str,
) -> dict[str, Any] | None:
    """Validate the schema-closed payload-free inventory verification binding."""

    if not isinstance(value, dict) or set(value) != VERIFICATION_FIELDS:
        errors.append(f"{path} fields must match the schema-closed contract")
        return None
    if value.get("schema") != lane_inventory.VERIFICATION_SCHEMA:
        errors.append(f"{path} schema must match the inventory contract")
    if value.get("status") != "ready":
        errors.append(f"{path} status must be ready")
    if value.get("signer_qualification") != "software-key-qualified":
        errors.append(f"{path} must be software-key-qualified")
    if _sha256(value.get("inventory_sha256")) is None:
        errors.append(f"{path} inventory_sha256 must be non-zero lowercase SHA-256")
    for field in ("summary_file_count", "recognized_summary_count"):
        if value.get(field) != len(DEFAULT_REQUIRED_GATES):
            errors.append(f"{path} {field} must be exactly 17")
    deployment = value.get("deployment")
    if not isinstance(deployment, dict) or set(deployment) != lane_inventory.DEPLOYMENT_FIELDS:
        errors.append(f"{path} deployment fields must match the contract")
    elif (
        _canonical_text(deployment.get("deployment_id")) is None
        or deployment.get("environment") not in lane_inventory.PRODUCTION_ENVIRONMENTS
        or deployment.get("network") != lane_inventory.TAIRA_NETWORK
        or deployment.get("chain_id") != lane_inventory.TAIRA_CHAIN_ID
        or deployment.get("chain_discriminant")
        != lane_inventory.TAIRA_CHAIN_DISCRIMINANT
    ):
        errors.append(f"{path} deployment must bind canonical production Taira")
    anchors = value.get("anchors")
    if not isinstance(anchors, dict) or set(anchors) != lane_inventory.ANCHOR_FIELDS:
        errors.append(f"{path} anchors fields must match the contract")
    else:
        for field in (
            "topology_qualification_summary_sha256",
            "topology_manifest_sha256",
            "topology_canonical_manifest_sha256",
            "validator_ids_sha256",
        ):
            if _sha256(anchors.get(field)) is None:
                errors.append(f"{path} anchors.{field} must be non-zero SHA-256")
        oldest = anchors.get("oldest_evidence_generated_at_unix")
        newest = anchors.get("newest_evidence_generated_at_unix")
        if (
            not isinstance(oldest, int)
            or isinstance(oldest, bool)
            or oldest <= 0
            or not isinstance(newest, int)
            or isinstance(newest, bool)
            or newest < oldest
        ):
            errors.append(f"{path} evidence timestamps must be positive and ordered")
    signer = value.get("signer")
    if not isinstance(signer, dict) or set(signer) != VERIFICATION_SIGNER_FIELDS:
        errors.append(f"{path} signer fields must match the contract")
    else:
        if (
            signer.get("role") != lane_inventory.SIGNER_ROLE
            or signer.get("service_kind") != lane_inventory.SIGNER_KIND
            or signer.get("algorithm") != "ed25519"
            or signer.get("backend") != "software"
            or _canonical_text(signer.get("service_id")) is None
            or _canonical_text(signer.get("administrator_id")) is None
            or signer.get("service_id") == signer.get("administrator_id")
            or _sha256(signer.get("policy_digest_sha256")) is None
            or _sha256(signer.get("public_key_fingerprint_sha256")) is None
        ):
            errors.append(f"{path} signer must be an external software Ed25519 signer")
        for field in ("key_revision", "policy_revision"):
            revision = signer.get(field)
            if (
                not isinstance(revision, int)
                or isinstance(revision, bool)
                or revision <= 0
                or revision > lane_inventory.MAX_INTEGER
            ):
                errors.append(f"{path} signer {field} must be positive")
    return value


def aggregate_inventory_row(
    verified: VerifiedLaneInventory | None,
    validation_errors: Sequence[str],
) -> dict[str, Any]:
    """Build the aggregate's schema-closed payload-free inventory row."""

    row_errors = list(validation_errors)
    return {
        "schema": AGGREGATE_L1_LANE_EVIDENCE_INVENTORY_SCHEMA,
        "present": verified is not None,
        "valid": verified is not None and not row_errors,
        "binding": None if verified is None else verified.verification,
        "errors": row_errors,
    }


def validate_aggregate_inventory_row(
    value: Any,
    errors: list[str],
) -> dict[str, Any] | None:
    """Validate one aggregate inventory row and return its binding."""

    path = "aggregate L1 lane evidence inventory"
    if not isinstance(value, dict) or set(value) != AGGREGATE_L1_LANE_EVIDENCE_INVENTORY_FIELDS:
        errors.append(f"{path} fields must match the schema-closed contract")
        return None
    if value.get("schema") != AGGREGATE_L1_LANE_EVIDENCE_INVENTORY_SCHEMA:
        errors.append(f"{path} schema must match the contract")
    present, valid = value.get("present"), value.get("valid")
    if not isinstance(present, bool) or not isinstance(valid, bool):
        errors.append(f"{path} present and valid must be booleans")
    binding = None
    if value.get("binding") is not None:
        binding = validate_verification_binding(
            value.get("binding"), errors, path=f"{path} binding"
        )
    elif present is True or valid is True:
        errors.append(f"{path} present row binding must be an object")
    row_errors = value.get("errors")
    if not isinstance(row_errors, list) or any(
        _canonical_text(error) is None for error in row_errors
    ):
        errors.append(f"{path} errors must contain canonical strings")
    if valid is True and (present is not True or binding is None or row_errors != []):
        errors.append(f"{path} valid row must be present, bound, and error-free")
    if valid is not True and (not isinstance(row_errors, list) or not row_errors):
        errors.append(f"{path} invalid row must contain an error")
    return binding


def _signer_domain(value: Any) -> tuple[str | None, str | None, str | None]:
    if not isinstance(value, Mapping):
        return None, None, None
    nested = value.get("signer")
    if isinstance(nested, Mapping):
        value = nested
    service = value.get("signer_service_id", value.get("service_id"))
    administrator = value.get(
        "signer_administrator_id", value.get("administrator_id")
    )
    fingerprint = value.get(
        "signer_public_key_fingerprint_sha256",
        value.get("public_key_fingerprint_sha256"),
    )
    return (
        service if isinstance(service, str) else None,
        administrator if isinstance(administrator, str) else None,
        fingerprint if isinstance(fingerprint, str) else None,
    )


def validate_signer_independence(
    *domains: tuple[str, Any],
) -> list[str]:
    """Reject every key or service/administrator identity overlap pairwise."""

    errors: list[str] = []
    normalized = [(label, _signer_domain(value)) for label, value in domains]
    for index, (left_label, left) in enumerate(normalized):
        for right_label, right in normalized[index + 1 :]:
            if left[2] is not None and left[2] == right[2]:
                errors.append(
                    f"{left_label} and {right_label} signer public keys must differ"
                )
            left_identities = {value for value in left[:2] if value is not None}
            right_identities = {value for value in right[:2] if value is not None}
            if left_identities & right_identities:
                errors.append(
                    f"{left_label} and {right_label} signer service/administrator identities must not overlap"
                )
    return errors


def validate_foundational_inventory_digest(
    value: Any,
    verified_binding: Mapping[str, Any] | None,
    errors: list[str],
    *,
    path: str,
) -> str | None:
    """Validate and optionally match a foundational signed inventory digest."""

    digest = _sha256(value)
    if digest is None:
        errors.append(f"{path} must be non-zero canonical lowercase SHA-256")
    elif verified_binding is not None and digest != verified_binding.get(
        "inventory_sha256"
    ):
        errors.append(f"{path} must match the signed L1 lane evidence inventory")
    return digest


def validate_foundational_lane_summary_digest_bindings(
    foundational_prerequisites: dict[str, Any],
    observed_summary_sha256: Mapping[str, str],
    required_gates: tuple[str, ...],
    errors: list[str],
) -> None:
    """Bind a full promotion run to the exact 17 supplied lane summary bytes."""

    if (
        len(required_gates) != len(DEFAULT_REQUIRED_GATES)
        or set(required_gates) != set(DEFAULT_REQUIRED_GATES)
    ):
        return
    rows = foundational_prerequisites.get("lane_summary_sha256")
    if not isinstance(rows, list):
        return
    expected_by_gate = {
        row.get("gate"): row.get("sha256")
        for row in rows
        if isinstance(row, dict)
        and isinstance(row.get("gate"), str)
        and _sha256(row.get("sha256")) is not None
    }
    row_errors = foundational_prerequisites.setdefault("errors", [])
    if not isinstance(row_errors, list):
        return
    for gate_name in DEFAULT_REQUIRED_GATES:
        observed = observed_summary_sha256.get(gate_name)
        if observed is None or expected_by_gate.get(gate_name) == observed:
            continue
        diagnostic = (
            "foundational prerequisite lane summary binding for "
            f"{gate_name} does not match the supplied readiness summary"
        )
        if diagnostic not in row_errors:
            row_errors.append(diagnostic)
        prefixed = f"foundational prerequisites: {diagnostic}"
        if prefixed not in errors:
            errors.append(prefixed)
    if row_errors:
        foundational_prerequisites["valid"] = False


def validate_duplicate_summary_diagnostics(
    required: Mapping[str, Any],
    duplicate_summary_gates: set[str],
    duplicate_summary_count: int,
    errors: list[str],
) -> None:
    """Pin deterministic duplicate-summary diagnostics in aggregate rows."""

    counted = 0
    for gate_name in sorted(duplicate_summary_gates):
        row = required.get(gate_name)
        diagnostic = f"duplicate {gate_name} production readiness summary"
        counted += errors.count(diagnostic)
        if not isinstance(row, dict):
            errors.append(f"{gate_name} duplicate summary row must be an object")
            continue
        row_errors = row.get("errors")
        if not isinstance(row_errors, list) or row_errors.count(diagnostic) != 1:
            errors.append(
                f"{gate_name} duplicate summary row errors must contain the deterministic duplicate summary diagnostic exactly once"
            )
    if counted != duplicate_summary_count:
        errors.append(
            "aggregate summary duplicate-summary diagnostics must match duplicate summary inputs"
        )


def validate_disallowed_summary_diagnostics(
    errors: list[str],
    *,
    unknown_schema_count: int,
    explicit_unrequired_count: int,
) -> None:
    """Pin aggregate blockers for unknown and explicitly unrequired inputs."""

    if errors.count("unknown SoraFS readiness summary schema") != unknown_schema_count:
        errors.append(
            "aggregate summary unknown-schema diagnostics must match discovered unknown summaries"
        )
    if errors.count(
        "explicit production readiness summary belongs to unrequired gate"
    ) != explicit_unrequired_count:
        errors.append(
            "aggregate summary unrequired-gate diagnostics must match explicit unrequired summaries"
        )


def validate_inventory_lane_digest_bindings(
    expected_summary_sha256: Mapping[str, str],
    observed_summary_sha256: Mapping[str, str],
    errors: list[str],
) -> None:
    """Require checker evidence paths to be the inventory-replayed exact bytes."""

    for gate_name, observed in observed_summary_sha256.items():
        if expected_summary_sha256.get(gate_name) != observed:
            errors.append(
                "signed L1 lane evidence inventory summary binding for "
                f"{gate_name} does not match the supplied readiness summary"
            )


def validate_aggregate_foundational_lane_digest_bindings(
    foundational: Any,
    required: Any,
    errors: list[str],
) -> None:
    """Bind aggregate foundational rows to the emitted per-lane digests."""

    if (
        not isinstance(foundational, dict)
        or foundational.get("present") is not True
        or not isinstance(required, dict)
    ):
        return
    rows = foundational.get("lane_summary_sha256")
    if not isinstance(rows, list):
        return
    for row in rows:
        if not isinstance(row, dict):
            continue
        gate_name = _canonical_text(row.get("gate"))
        foundational_digest = _sha256(row.get("sha256"))
        required_row = required.get(gate_name) if gate_name is not None else None
        required_digest = (
            _sha256(required_row.get("sha256"))
            if isinstance(required_row, dict)
            else None
        )
        if (
            gate_name is not None
            and foundational_digest is not None
            and required_digest is not None
            and foundational_digest != required_digest
        ):
            errors.append(
                f"{gate_name} aggregate foundational lane digest must "
                "match required row sha256"
            )
