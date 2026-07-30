#!/usr/bin/env python3
"""Validate one holistic SoraFS L1 resilience and disaster-recovery receipt.

The receipt is a qualification attachment bound to the existing
four-validator topology; it is deliberately neither an eighteenth readiness
lane nor a tenth foundational prerequisite ID. Local receipts remain
non-promotable. Only a receipt authenticated by an operator-trusted Ed25519
public key may become evidence-qualified for promotion review.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_production_readiness import (  # noqa: E402
    canonical_string,
    is_production_ready_environment,
    require_production_deployment_id_value,
)
from sccp_release_common import verify_ed25519  # noqa: E402
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    emit_checker_notice,
    render_and_write_checker_summary,
    validate_checker_preflight,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_paths import (  # noqa: E402
    inspect_evidence_directory,
    resolve_evidence_path,
)
from sorafs_evidence_sensitivity import visit_sensitive_fields  # noqa: E402
from sorafs_evidence_validation import (  # noqa: E402
    is_archive_portable_artifact_path,
    require_recent_timestamp,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
)
from sorafs_topology_qualification import (  # noqa: E402
    load_topology_qualification_binding,
    validate_topology_binding_object,
)


RECEIPT_SCHEMA = "sorafs.l1.resilience_qualification.v1"
ARTIFACT_SCHEMA = "sorafs.l1.resilience_qualification.artifact.v1"
SUMMARY_SCHEMA = "sorafs.l1.resilience_qualification.summary.v1"
SIGNATURE_DOMAIN = b"iroha:sorafs:l1-resilience-qualification:v1\x00"
MAX_RECEIPT_BYTES = 512 * 1024
MAX_ARTIFACT_BYTES = 256 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 24 * 60 * 60
MAX_TIMESTAMP = (1 << 63) - 1
READINESS_LANE_COUNT_DELTA = 0
IDENTIFIER_PATTERN = re.compile(r"^[a-z0-9]+(?:[.-][a-z0-9]+)*\Z")

REQUIRED_REQUIREMENTS = (
    "network_partition_recovery",
    "consensus_view_change",
    "validator_restart",
    "torii_restart",
    "provider_restart",
    "simultaneous_peer_submission",
    "signer_rotation",
    "root_rotation",
    "catalog_rotation",
    "gateway_failover",
    "governance_dag_failover",
    "stale_fork_rejection",
    "crash_recovery",
    "identical_post_recovery_peer_state",
    "repair_outcome",
    "settlement_outcome",
    "backup_restore",
    "release_rollback",
    "package_yank",
)

RECEIPT_FIELDS = frozenset(
    {
        "schema",
        "deployment",
        "topology_qualification",
        "generated_at_unix",
        "artifacts",
        "authentication",
    }
)
DEPLOYMENT_FIELDS = frozenset({"deployment_id", "environment"})
ARTIFACT_ROW_FIELDS = frozenset(
    {"requirement", "artifact_path", "artifact_sha256", "captured_at_unix"}
)
ARTIFACT_FIELDS = frozenset(
    {
        "schema",
        "requirement",
        "deployment",
        "topology_qualification",
        "captured_at_unix",
        "result",
        "observation_count",
        "payload_included",
        "peer_state_digests",
    }
)
PEER_STATE_FIELDS = frozenset({"validator_id", "finalized_state_sha256"})
AUTHENTICATION_FIELDS = frozenset(
    {
        "kind",
        "algorithm",
        "public_key_fingerprint_sha256",
        "signature_hex",
    }
)
SUMMARY_FIELDS = frozenset(
    {
        "schema",
        "status",
        "qualification_scope",
        "live_evidence_recognized",
        "externally_authenticated",
        "promotion_eligible",
        "readiness_lane_count_delta",
        "receipt_sha256",
        "canonical_receipt_sha256",
        "receipt_generated_at_unix",
        "receipt_authentication",
        "deployment",
        "topology_qualification",
        "required_requirements",
        "recognized_requirement_count",
        "artifact_bindings",
        "earliest_capture_unix",
        "latest_capture_unix",
        "errors",
    }
)


def _closed_object(
    value: Any,
    fields: frozenset[str],
    label: str,
    errors: list[str],
) -> Mapping[str, Any] | None:
    """Return one schema-closed mapping or record a stable diagnostic."""

    if not isinstance(value, Mapping):
        errors.append(f"{label} must be an object")
        return None
    if set(value) != fields:
        errors.append(f"{label} fields must match the schema-closed contract")
    return value


def _row_sequence(
    value: Any,
    label: str,
    errors: list[str],
) -> Sequence[Any] | None:
    """Return one non-string sequence or record a stable diagnostic."""

    if (
        isinstance(value, (str, bytes, bytearray, Mapping))
        or not isinstance(value, Sequence)
    ):
        errors.append(f"{label} must be an array")
        return None
    return value


def _canonical_nonzero_sha256(
    value: Any,
    label: str,
    errors: list[str],
) -> str | None:
    """Return one canonical, non-zero SHA-256 digest."""

    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{label} must be canonical lowercase SHA-256")
        return None
    decoded = bytes.fromhex(value)
    if not any(decoded):
        errors.append(f"{label} must not be zero")
        return None
    return value


def _canonical_signature_hex(
    value: Any,
    label: str,
    errors: list[str],
) -> bytes | None:
    """Decode one exact non-zero raw Ed25519 signature."""

    if (
        not isinstance(value, str)
        or len(value) != 128
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{label} must be exactly 64 bytes of lowercase hex")
        return None
    signature = bytes.fromhex(value)
    if not any(signature):
        errors.append(f"{label} must not be the all-zero signature")
        return None
    return signature


def _canonical_identifier(
    value: Any,
    label: str,
    errors: list[str],
) -> str | None:
    """Return one bounded lowercase identifier."""

    if (
        not isinstance(value, str)
        or canonical_string(value) is None
        or len(value) > 128
        or IDENTIFIER_PATTERN.fullmatch(value) is None
    ):
        errors.append(
            f"{label} must be a canonical lowercase identifier of at most 128 bytes"
        )
        return None
    return value


def _positive_count(value: Any, label: str, errors: list[str]) -> int:
    """Return one positive bounded observation count."""

    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or not 1 <= value <= (1 << 31) - 1
    ):
        errors.append(f"{label} must be an integer in 1..2147483647")
        return 0
    return value


def _deployment_context(
    value: Any,
    *,
    expected_deployment_id: str,
    expected_environment: str,
    label: str,
    errors: list[str],
) -> dict[str, str] | None:
    """Validate one exact production deployment context."""

    deployment = _closed_object(value, DEPLOYMENT_FIELDS, label, errors)
    if deployment is None:
        return None
    deployment_errors: list[str] = []
    deployment_id = require_production_deployment_id_value(
        deployment.get("deployment_id"),
        deployment_errors,
        f"{label}.deployment_id",
    )
    errors.extend(deployment_errors)
    environment = deployment.get("environment")
    if not is_production_ready_environment(environment):
        errors.append(f"{label}.environment must be production")
        return None
    if deployment_id != expected_deployment_id:
        errors.append(f"{label}.deployment_id must match the reviewed deployment")
    if environment != expected_environment:
        errors.append(f"{label}.environment must match the reviewed deployment")
    if deployment_id != expected_deployment_id or environment != expected_environment:
        return None
    return {
        "deployment_id": deployment_id,
        "environment": str(environment),
    }


def canonical_receipt_sha256(payload: Mapping[str, Any]) -> str:
    """Hash the canonical JSON encoding of a resilience receipt."""

    encoded = json.dumps(
        payload,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def resilience_signing_payload(payload: Mapping[str, Any]) -> bytes:
    """Build the domain-separated canonical bytes authenticated externally."""

    unsigned = dict(payload)
    authentication = unsigned.get("authentication")
    if isinstance(authentication, Mapping):
        unsigned_authentication = dict(authentication)
        unsigned_authentication.pop("signature_hex", None)
        unsigned["authentication"] = unsigned_authentication
    return SIGNATURE_DOMAIN + json.dumps(
        unsigned,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def parse_trusted_public_key(
    value: Any,
    errors: list[str],
) -> bytes | None:
    """Decode an optional operator-trusted raw Ed25519 public key."""

    if value is None:
        return None
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(
            "--trusted-public-key-hex must be exactly 32 bytes of lowercase hex"
        )
        return None
    public_key = bytes.fromhex(value)
    if not any(public_key):
        errors.append("--trusted-public-key-hex must not be the all-zero key")
        return None
    return public_key


def _sensitivity_view(payload: Mapping[str, Any]) -> dict[str, Any]:
    """Return a copy that keeps signature bytes out of secret-value heuristics."""

    candidate = dict(payload)
    authentication = candidate.get("authentication")
    if isinstance(authentication, Mapping):
        sanitized_authentication = dict(authentication)
        sanitized_authentication["signature_hex"] = None
        candidate["authentication"] = sanitized_authentication
    return candidate


def _validate_authentication(
    receipt: Mapping[str, Any],
    trusted_public_key: bytes | None,
    errors: list[str],
) -> bool:
    """Validate local mode or an operator-trusted external Ed25519 signature."""

    authentication = _closed_object(
        receipt.get("authentication"),
        AUTHENTICATION_FIELDS,
        "authentication",
        errors,
    )
    if authentication is None:
        return False
    kind = authentication.get("kind")
    if kind == "local":
        if authentication.get("algorithm") is not None:
            errors.append("local authentication.algorithm must be null")
        if authentication.get("public_key_fingerprint_sha256") is not None:
            errors.append(
                "local authentication.public_key_fingerprint_sha256 must be null"
            )
        if authentication.get("signature_hex") is not None:
            errors.append("local authentication.signature_hex must be null")
        if trusted_public_key is not None:
            errors.append(
                "--trusted-public-key-hex must only be supplied for external-ed25519 authentication"
            )
        return False
    if kind != "external-ed25519":
        errors.append(
            "authentication.kind must be `local` or `external-ed25519`"
        )
        return False
    if authentication.get("algorithm") != "ed25519":
        errors.append("external authentication.algorithm must be `ed25519`")
    fingerprint = _canonical_nonzero_sha256(
        authentication.get("public_key_fingerprint_sha256"),
        "authentication.public_key_fingerprint_sha256",
        errors,
    )
    signature = _canonical_signature_hex(
        authentication.get("signature_hex"),
        "authentication.signature_hex",
        errors,
    )
    if trusted_public_key is None:
        errors.append(
            "external authentication requires --trusted-public-key-hex"
        )
        return False
    expected_fingerprint = hashlib.sha256(trusted_public_key).hexdigest()
    if fingerprint is not None and fingerprint != expected_fingerprint:
        errors.append(
            "authentication public key fingerprint must match the operator-trusted key"
        )
    if signature is None or fingerprint != expected_fingerprint:
        return False
    if not verify_ed25519(
        trusted_public_key,
        signature,
        resilience_signing_payload(receipt),
    ):
        errors.append("resilience receipt signature verification failed")
        return False
    return True


def _validate_peer_state_digests(
    value: Any,
    *,
    required: bool,
    errors: list[str],
) -> None:
    """Require four matching peer-state digests only for the recovery check."""

    rows = _row_sequence(value, "peer_state_digests", errors)
    if rows is None:
        return
    if not required:
        if rows:
            errors.append(
                "peer_state_digests must be empty outside identical post-recovery state evidence"
            )
        return
    if len(rows) != 4:
        errors.append(
            "peer_state_digests must contain exactly four voting-validator states"
        )
    validator_ids: list[str] = []
    state_digests: list[str] = []
    for index, item in enumerate(rows):
        label = f"peer_state_digests[{index}]"
        row = _closed_object(item, PEER_STATE_FIELDS, label, errors)
        if row is None:
            continue
        validator_id = _canonical_identifier(
            row.get("validator_id"),
            f"{label}.validator_id",
            errors,
        )
        state_digest = _canonical_nonzero_sha256(
            row.get("finalized_state_sha256"),
            f"{label}.finalized_state_sha256",
            errors,
        )
        if validator_id is not None:
            validator_ids.append(validator_id)
        if state_digest is not None:
            state_digests.append(state_digest)
    if len(validator_ids) == 4 and len(set(validator_ids)) != 4:
        errors.append("peer_state_digests validator identities must be unique")
    if len(state_digests) == 4 and len(set(state_digests)) != 1:
        errors.append(
            "peer_state_digests must prove identical post-recovery finalized state"
        )


def _validate_artifact_payload(
    payload: Mapping[str, Any],
    *,
    expected_requirement: str,
    expected_captured_at_unix: int,
    expected_deployment_id: str,
    expected_environment: str,
    topology_qualification: Mapping[str, str],
    now_unix: int,
    max_evidence_age_secs: int,
) -> list[str]:
    """Validate one payload-free, topology-bound resilience observation."""

    errors: list[str] = []
    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys=(),
        evidence_label="SoraFS L1 resilience qualification artifact",
    )
    artifact = _closed_object(
        payload,
        ARTIFACT_FIELDS,
        f"{expected_requirement} artifact",
        errors,
    )
    if artifact is None:
        return errors
    if artifact.get("schema") != ARTIFACT_SCHEMA:
        errors.append(f"{expected_requirement} artifact schema must match the contract")
    if artifact.get("requirement") != expected_requirement:
        errors.append(
            f"{expected_requirement} artifact requirement identity must match its receipt row"
        )
    _deployment_context(
        artifact.get("deployment"),
        expected_deployment_id=expected_deployment_id,
        expected_environment=expected_environment,
        label=f"{expected_requirement} artifact deployment",
        errors=errors,
    )
    errors.extend(
        validate_topology_binding_object(
            artifact.get("topology_qualification"),
            expected=topology_qualification,
            path=f"{expected_requirement} artifact topology_qualification",
        )
    )
    captured_at_unix = require_recent_timestamp(
        artifact,
        "captured_at_unix",
        errors,
        now_unix=now_unix,
        max_age_secs=max_evidence_age_secs,
        path=f"{expected_requirement} artifact captured_at_unix",
    )
    if captured_at_unix != expected_captured_at_unix:
        errors.append(
            f"{expected_requirement} artifact captured_at_unix must match its receipt row"
        )
    if artifact.get("result") != "passed":
        errors.append(f"{expected_requirement} artifact result must be `passed`")
    _positive_count(
        artifact.get("observation_count"),
        f"{expected_requirement} artifact observation_count",
        errors,
    )
    if artifact.get("payload_included") is not False:
        errors.append(f"{expected_requirement} artifact payload_included must be false")
    _validate_peer_state_digests(
        artifact.get("peer_state_digests"),
        required=expected_requirement == "identical_post_recovery_peer_state",
        errors=errors,
    )
    return errors


def _validate_artifact_rows(
    value: Any,
    *,
    artifact_root: Path,
    expected_deployment_id: str,
    expected_environment: str,
    topology_qualification: Mapping[str, str],
    generated_at_unix: int,
    now_unix: int,
    max_evidence_age_secs: int,
    errors: list[str],
) -> tuple[list[dict[str, Any]], list[int]]:
    """Validate the canonical artifact inventory and exact referenced bytes."""

    rows = _row_sequence(value, "artifacts", errors)
    if rows is None:
        return [], []
    if len(rows) != len(REQUIRED_REQUIREMENTS):
        errors.append(
            f"artifacts must contain exactly {len(REQUIRED_REQUIREMENTS)} requirements"
        )
    bindings: list[dict[str, Any]] = []
    captures: list[int] = []
    observed_requirements: list[str] = []
    observed_digests: list[str] = []
    observed_paths: list[str] = []
    path_identities: set[Path] = set()

    for index, item in enumerate(rows):
        expected_requirement = (
            REQUIRED_REQUIREMENTS[index]
            if index < len(REQUIRED_REQUIREMENTS)
            else "<unexpected>"
        )
        label = f"artifacts[{index}]"
        before = len(errors)
        row = _closed_object(item, ARTIFACT_ROW_FIELDS, label, errors)
        if row is None:
            continue
        requirement = row.get("requirement")
        if requirement != expected_requirement:
            errors.append(
                "artifacts must match the canonical resilience requirement order"
            )
        if isinstance(requirement, str):
            observed_requirements.append(requirement)
        artifact_path = row.get("artifact_path")
        if (
            not isinstance(artifact_path, str)
            or not is_archive_portable_artifact_path(artifact_path)
        ):
            errors.append(
                f"{label}.artifact_path must be a portable archive-relative path"
            )
            continue
        observed_paths.append(artifact_path)
        expected_digest = _canonical_nonzero_sha256(
            row.get("artifact_sha256"),
            f"{label}.artifact_sha256",
            errors,
        )
        if expected_digest is not None:
            observed_digests.append(expected_digest)
        captured_at_unix = require_recent_timestamp(
            row,
            "captured_at_unix",
            errors,
            now_unix=now_unix,
            max_age_secs=max_evidence_age_secs,
            path=f"{label}.captured_at_unix",
        )
        if generated_at_unix and captured_at_unix > generated_at_unix:
            errors.append(f"{label}.captured_at_unix must not follow generated_at_unix")

        artifact_file = artifact_root.joinpath(*artifact_path.split("/"))
        loaded = load_evidence_json_with_sha256_or_record_error(
            artifact_file,
            MAX_ARTIFACT_BYTES,
            errors,
        )
        identity_errors: list[str] = []
        identity = resolve_evidence_path(
            artifact_file,
            identity_errors,
            label="resilience artifact path",
        )
        if identity_errors or identity is None:
            errors.append(f"{label}.artifact_path cannot be resolved safely")
        elif identity in path_identities:
            errors.append("artifacts must bind unique canonical file identities")
        else:
            path_identities.add(identity)
        if loaded is None:
            continue
        artifact_payload, actual_digest = loaded
        if expected_digest is not None and actual_digest != expected_digest:
            errors.append(f"{label}.artifact_sha256 does not match exact artifact bytes")
        if requirement not in REQUIRED_REQUIREMENTS:
            errors.append(f"{label}.requirement is not part of the closed contract")
        elif isinstance(captured_at_unix, int) and captured_at_unix > 0:
            errors.extend(
                _validate_artifact_payload(
                    artifact_payload,
                    expected_requirement=requirement,
                    expected_captured_at_unix=captured_at_unix,
                    expected_deployment_id=expected_deployment_id,
                    expected_environment=expected_environment,
                    topology_qualification=topology_qualification,
                    now_unix=now_unix,
                    max_evidence_age_secs=max_evidence_age_secs,
                )
            )
        if len(errors) == before:
            binding = {
                "requirement": requirement,
                "artifact_path": artifact_path,
                "artifact_sha256": actual_digest,
                "captured_at_unix": captured_at_unix,
            }
            bindings.append(binding)
            captures.append(captured_at_unix)

    if observed_requirements != list(REQUIRED_REQUIREMENTS):
        errors.append(
            "artifacts must cover every resilience requirement exactly once in canonical order"
        )
    if len(observed_requirements) != len(set(observed_requirements)):
        errors.append("artifacts must not contain duplicate requirements")
    if len(observed_paths) != len(set(observed_paths)):
        errors.append("artifacts must not contain duplicate artifact paths")
    if len(observed_digests) != len(set(observed_digests)):
        errors.append("artifacts must not contain duplicate artifact digests")
    return bindings, captures


def validate_receipt(
    payload: dict[str, Any],
    receipt_sha256: str,
    *,
    artifact_root: Path,
    expected_deployment_id: str,
    expected_environment: str,
    topology_qualification: Mapping[str, str],
    now_unix: int,
    max_evidence_age_secs: int,
    trusted_public_key: bytes | None,
) -> tuple[dict[str, Any], list[str]]:
    """Validate one holistic receipt and build its payload-free summary."""

    errors: list[str] = []
    visit_sensitive_fields(
        _sensitivity_view(payload),
        "",
        errors,
        sensitive_keys=(),
        evidence_label="SoraFS L1 resilience qualification receipt",
    )
    receipt = _closed_object(
        payload,
        RECEIPT_FIELDS,
        "resilience qualification receipt",
        errors,
    )
    deployment: dict[str, str] | None = None
    generated_at_unix = 0
    bindings: list[dict[str, Any]] = []
    captures: list[int] = []
    externally_authenticated = False

    if receipt is not None:
        if receipt.get("schema") != RECEIPT_SCHEMA:
            errors.append("resilience qualification schema must match the contract")
        deployment = _deployment_context(
            receipt.get("deployment"),
            expected_deployment_id=expected_deployment_id,
            expected_environment=expected_environment,
            label="deployment",
            errors=errors,
        )
        errors.extend(
            validate_topology_binding_object(
                receipt.get("topology_qualification"),
                expected=topology_qualification,
                path="topology_qualification",
            )
        )
        generated_at_unix = require_recent_timestamp(
            receipt,
            "generated_at_unix",
            errors,
            now_unix=now_unix,
            max_age_secs=max_evidence_age_secs,
        )
        bindings, captures = _validate_artifact_rows(
            receipt.get("artifacts"),
            artifact_root=artifact_root,
            expected_deployment_id=expected_deployment_id,
            expected_environment=expected_environment,
            topology_qualification=topology_qualification,
            generated_at_unix=generated_at_unix,
            now_unix=now_unix,
            max_evidence_age_secs=max_evidence_age_secs,
            errors=errors,
        )
        externally_authenticated = _validate_authentication(
            receipt,
            trusted_public_key,
            errors,
        )

    qualified = not errors
    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": (
            "evidence-qualified"
            if qualified and externally_authenticated
            else "configuration-qualified"
            if qualified
            else "blocked"
        ),
        "qualification_scope": "holistic-deployment-resilience",
        "live_evidence_recognized": qualified and externally_authenticated,
        "externally_authenticated": qualified and externally_authenticated,
        "promotion_eligible": qualified and externally_authenticated,
        "readiness_lane_count_delta": READINESS_LANE_COUNT_DELTA,
        "receipt_sha256": receipt_sha256,
        "canonical_receipt_sha256": canonical_receipt_sha256(payload),
        "receipt_generated_at_unix": generated_at_unix,
        "receipt_authentication": (
            dict(receipt.get("authentication"))
            if receipt is not None
            and isinstance(receipt.get("authentication"), Mapping)
            else None
        ),
        "deployment": deployment
        or {"deployment_id": None, "environment": None},
        "topology_qualification": dict(topology_qualification),
        "required_requirements": list(REQUIRED_REQUIREMENTS),
        "recognized_requirement_count": len(bindings),
        "artifact_bindings": bindings,
        "earliest_capture_unix": min(captures) if captures else None,
        "latest_capture_unix": max(captures) if captures else None,
        "errors": errors,
    }
    assert set(summary) == SUMMARY_FIELDS
    return summary, errors


def _blocked_summary(
    errors: list[str],
    topology_qualification: Mapping[str, str] | None,
) -> dict[str, Any]:
    """Build the stable blocked result used before a receipt can be decoded."""

    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": "blocked",
        "qualification_scope": "holistic-deployment-resilience",
        "live_evidence_recognized": False,
        "externally_authenticated": False,
        "promotion_eligible": False,
        "readiness_lane_count_delta": READINESS_LANE_COUNT_DELTA,
        "receipt_sha256": None,
        "canonical_receipt_sha256": None,
        "receipt_generated_at_unix": None,
        "receipt_authentication": None,
        "deployment": {"deployment_id": None, "environment": None},
        "topology_qualification": (
            dict(topology_qualification)
            if topology_qualification is not None
            else None
        ),
        "required_requirements": list(REQUIRED_REQUIREMENTS),
        "recognized_requirement_count": 0,
        "artifact_bindings": [],
        "earliest_capture_unix": None,
        "latest_capture_unix": None,
        "errors": errors,
    }
    assert set(summary) == SUMMARY_FIELDS
    return summary


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse resilience-qualification checker arguments."""

    parser = EvidenceArgumentParser(
        description=(
            "Validate one topology-bound SoraFS L1 resilience and DR receipt."
        ),
    )
    parser.add_argument(
        "--receipt",
        dest="evidence",
        action="append",
        type=Path,
        default=[],
        help="Exactly one schema-closed holistic resilience receipt.",
    )
    parser.add_argument(
        "--artifact-root",
        required=True,
        type=Path,
        help="Directory against which receipt artifact paths are resolved.",
    )
    parser.add_argument(
        "--topology-qualification-summary",
        required=True,
        type=Path,
        help="Exact existing four-validator L1 topology qualification summary.",
    )
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--now-unix", required=True, type=int)
    parser.add_argument(
        "--max-evidence-age-secs",
        type=int,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )
    parser.add_argument(
        "--trusted-public-key-hex",
        help=(
            "Operator-trusted raw Ed25519 public key; required only for an "
            "external-ed25519 receipt."
        ),
    )
    parser.add_argument("--summary-out", type=Path)
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error
    return parser.parse_args(expanded_args)


def main(argv: list[str] | None = None) -> int:
    """Run the holistic L1 resilience and disaster-recovery checker."""

    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    preflight_errors = validate_checker_preflight(args)
    if len(args.evidence) != 1:
        preflight_errors.append("provide exactly one --receipt input")
    deployment_errors: list[str] = []
    deployment_id = require_production_deployment_id_value(
        args.deployment_id,
        deployment_errors,
        "--deployment-id",
    )
    preflight_errors.extend(deployment_errors)
    if not is_production_ready_environment(args.environment):
        preflight_errors.append("--environment must be production")
    if (
        not isinstance(args.now_unix, int)
        or isinstance(args.now_unix, bool)
        or not 0 <= args.now_unix <= MAX_TIMESTAMP
    ):
        preflight_errors.append("--now-unix must be a non-negative integer timestamp")
    if (
        not isinstance(args.max_evidence_age_secs, int)
        or isinstance(args.max_evidence_age_secs, bool)
        or not 0 <= args.max_evidence_age_secs <= MAX_TIMESTAMP
    ):
        preflight_errors.append(
            "--max-evidence-age-secs must be a non-negative integer"
        )
    artifact_root_errors: list[str] = []
    root_is_directory = inspect_evidence_directory(
        args.artifact_root,
        artifact_root_errors,
    )
    preflight_errors.extend(artifact_root_errors)
    if root_is_directory is False:
        preflight_errors.append("--artifact-root must exist and be a directory")
    trusted_public_key = parse_trusted_public_key(
        args.trusted_public_key_hex,
        preflight_errors,
    )
    topology_qualification, topology_errors = load_topology_qualification_binding(
        args.topology_qualification_summary,
        expected_deployment_id=deployment_id,
        expected_environment=args.environment,
    )
    preflight_errors.extend(topology_errors)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    load_errors: list[str] = []
    loaded = load_evidence_json_with_sha256_or_record_error(
        args.evidence[0],
        MAX_RECEIPT_BYTES,
        load_errors,
    )
    if loaded is None:
        summary = _blocked_summary(load_errors, topology_qualification)
        _, render_errors = render_and_write_checker_summary(args.summary_out, summary)
        if render_errors:
            emit_checker_error_lines(render_errors)
            return 2
        emit_checker_error_block(
            "SoraFS L1 resilience qualification is blocked:",
            load_errors,
        )
        return 1

    payload, receipt_sha256 = loaded
    assert topology_qualification is not None
    summary, errors = validate_receipt(
        payload,
        receipt_sha256,
        artifact_root=args.artifact_root,
        expected_deployment_id=deployment_id,
        expected_environment=args.environment,
        topology_qualification=topology_qualification,
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        trusted_public_key=trusted_public_key,
    )
    _, render_errors = render_and_write_checker_summary(args.summary_out, summary)
    if render_errors:
        emit_checker_error_lines(render_errors)
        return 2
    if errors:
        emit_checker_error_block(
            "SoraFS L1 resilience qualification is blocked:",
            errors,
        )
        return 1
    if summary["externally_authenticated"]:
        emit_checker_notice(
            "SoraFS L1 resilience receipt is externally authenticated and evidence-qualified."
        )
    else:
        emit_checker_notice(
            "SoraFS L1 resilience receipt is configuration-qualified only; "
            "it is non-promotable and recognizes no live evidence."
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
