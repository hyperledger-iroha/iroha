#!/usr/bin/env python3
"""Validate protected, path-free Sumeragi v2 release approvals.

An approval is an operator decision record, not a digital signature and not a
claim about the approver's cryptographic identity.  This module authenticates
the protected local file contract and its exact canonical contents.  The
bootstrap, standalone validator, and receipt writer compare those contents
with the exact planned invocation and retain the separately administered
operator handoff.

Raw approval bytes may contain the exact candidate-relative command arguments.
The sanitized archive projection never contains those arguments or a source
pathname; it carries only stable semantic identifiers and canonical digests.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from enum import Enum
import hashlib
import json
import os
from pathlib import Path
import re
import stat
from types import MappingProxyType
from typing import Any, Mapping, Sequence


APPROVAL_FORMAT = "iroha-sumeragi-v2-release-approval"
APPROVAL_SCHEMA_VERSION = 1
APPROVAL_ARCHIVE_FORMAT = "iroha-sumeragi-v2-release-approval-attestation"
APPROVAL_SET_ARCHIVE_FORMAT = (
    "iroha-sumeragi-v2-release-approval-set-attestation"
)
MAX_APPROVAL_BYTES = 256 * 1024
MAX_APPROVAL_OPERATIONS = 256
MAX_COMMAND_ARGUMENTS = 256
MAX_COMMAND_ARGUMENT_BYTES = 4096
MAX_EXPECTED_DURATION_SECONDS = 31 * 24 * 60 * 60
PROTECTED_APPROVAL_MODE = 0o400


class ReleaseApprovalClass(str, Enum):
    """The four independently authorized first-release execution classes."""

    OFFLINE_TOOLCHAIN_SDK = "offline-toolchain-sdk"
    FORMAL_PROOF_TOOLS = "formal-proof-tools"
    NETWORK_SCALE_SOAK = "network-scale-soak"
    FINAL_BOOTSTRAP_PUBLICATION = "final-bootstrap-publication"


APPROVAL_CLASS_ORDER = tuple(ReleaseApprovalClass)
APPROVAL_CLASS_IDS = tuple(value.value for value in APPROVAL_CLASS_ORDER)
APPROVAL_FILENAMES = {
    approval_class: f"{approval_class.value}.approval.v1.json"
    for approval_class in APPROVAL_CLASS_ORDER
}
APPROVAL_ARCHIVE_IDS = {
    approval_class: f"release-approval.{approval_class.value}.v1"
    for approval_class in APPROVAL_CLASS_ORDER
}

_TOP_LEVEL_FIELDS = frozenset(
    {
        "approval_id",
        "approved_at",
        "candidate_oid",
        "candidate_tree",
        "class_id",
        "evidence_root_id",
        "expected_duration_seconds",
        "format",
        "operations",
        "profile",
        "protected_tool_manifest_sha256",
        "schema_version",
    }
)
_OPERATION_FIELDS = frozenset(
    {"arguments", "operation_id", "ordinal", "tool_id"}
)
_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_OBJECT_ID_RE = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_IDENTIFIER_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:+-]{0,127}")
_UTC_SECONDS_RE = re.compile(
    r"[0-9]{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12][0-9]|3[01])"
    r"T(?:[01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]Z"
)
_WINDOWS_ABSOLUTE_RE = re.compile(r"[A-Za-z]:[\\/]")


class ReleaseApprovalError(RuntimeError):
    """A release approval failed its protected-file or semantic contract."""


@dataclass(frozen=True)
class ApprovalOperation:
    """One exact ordered operation and its protected-tool command vector."""

    ordinal: int
    operation_id: str
    tool_id: str
    arguments: tuple[str, ...]

    def value(self) -> dict[str, Any]:
        """Return the exact canonical JSON value for this operation."""

        return {
            "arguments": list(self.arguments),
            "operation_id": self.operation_id,
            "ordinal": self.ordinal,
            "tool_id": self.tool_id,
        }

    @property
    def record_sha256(self) -> str:
        """Digest the complete canonical operation record."""

        return hashlib.sha256(_canonical_json(self.value())).hexdigest()

    @property
    def command(self) -> tuple[str, ...]:
        """Return the exact path-free tool identifier and ordered arguments."""

        return (self.tool_id, *self.arguments)


@dataclass(frozen=True)
class PlannedApprovalOperation:
    """One immutable normalized command template for an approval class.

    Template arguments are canonical relative arguments or stable semantic
    identifiers.  The four braced binding slots are replaced only by
    :func:`build_release_approval_expectations`; they are never accepted from
    an approval file.  In particular, a plan never contains an original
    checkout, tool, cache, or evidence pathname.
    """

    operation_id: str
    tool_id: str
    arguments: tuple[str, ...]


@dataclass(frozen=True)
class ReleaseApprovalExpectation:
    """Exact execution inputs a consumer must compare with one approval.

    Bootstrap integration must construct each instance from that class's own
    planned operation inventory; it must not reuse one generic command list
    across approval classes.
    """

    class_id: ReleaseApprovalClass
    candidate_oid: str
    candidate_tree: str
    profile: str
    operations: tuple[ApprovalOperation, ...]
    protected_tool_manifest_sha256: str
    evidence_root_id: str
    expected_duration_seconds: int


@dataclass(frozen=True)
class SanitizedApprovalArchive:
    """Path-free canonical projection ready for protected evidence archiving."""

    value: dict[str, Any]
    canonical_bytes: bytes
    sha256: str


@dataclass(frozen=True)
class ValidatedReleaseApproval:
    """One stable protected approval snapshot with no retained source path."""

    approval_id: str
    class_id: ReleaseApprovalClass
    candidate_oid: str
    candidate_tree: str
    profile: str
    operations: tuple[ApprovalOperation, ...]
    protected_tool_manifest_sha256: str
    evidence_root_id: str
    expected_duration_seconds: int
    approved_at: str
    canonical_bytes: bytes
    approval_sha256: str
    size_bytes: int
    source_mode: int
    source_nlink: int
    source_owner_uid: int

    def sanitized_archive(self) -> SanitizedApprovalArchive:
        """Project stable approval evidence without paths or raw command values."""

        projection = {
            "approval_id": self.approval_id,
            "approved_at": self.approved_at,
            "archive_id": APPROVAL_ARCHIVE_IDS[self.class_id],
            "candidate_oid": self.candidate_oid,
            "candidate_tree": self.candidate_tree,
            "class_id": self.class_id.value,
            "evidence_root_id": self.evidence_root_id,
            "expected_duration_seconds": self.expected_duration_seconds,
            "format": APPROVAL_ARCHIVE_FORMAT,
            "ordered_operations": [
                {
                    "operation_id": operation.operation_id,
                    "ordinal": operation.ordinal,
                    "record_sha256": operation.record_sha256,
                    "tool_id": operation.tool_id,
                }
                for operation in self.operations
            ],
            "profile": self.profile,
            "protected_approval": {
                "mode": f"{self.source_mode:04o}",
                "nlink": self.source_nlink,
                "owner_contract": "release-host-effective-uid",
                "sha256": self.approval_sha256,
                "size_bytes": self.size_bytes,
            },
            "protected_tool_manifest_sha256": self.protected_tool_manifest_sha256,
            "schema_version": APPROVAL_SCHEMA_VERSION,
        }
        canonical = _canonical_json(projection)
        return SanitizedApprovalArchive(
            value=projection,
            canonical_bytes=canonical,
            sha256=hashlib.sha256(canonical).hexdigest(),
        )


@dataclass(frozen=True)
class _DirectoryIdentity:
    path: Path
    device: int
    inode: int
    owner_uid: int
    mode: int


def _canonical_json(value: Any) -> bytes:
    try:
        text = json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        )
    except (TypeError, ValueError) as error:
        raise ReleaseApprovalError("approval value cannot be encoded canonically") from error
    return (text + "\n").encode("ascii")


def _reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ReleaseApprovalError("release approval contains a duplicate JSON field")
        value[key] = item
    return value


def _reject_nonfinite(value: str) -> None:
    del value
    raise ReleaseApprovalError("release approval contains a non-finite JSON value")


def _reject_float(value: str) -> None:
    del value
    raise ReleaseApprovalError("release approval contains a floating-point JSON value")


def _exact_dict(value: Any, fields: frozenset[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != fields:
        raise ReleaseApprovalError(f"{label} has the wrong exact schema")
    return value


def _identifier(value: Any, label: str) -> str:
    if not isinstance(value, str) or _IDENTIFIER_RE.fullmatch(value) is None:
        raise ReleaseApprovalError(f"{label} must be one path-free identifier")
    return value


def _digest(value: Any, label: str) -> str:
    if not isinstance(value, str) or _DIGEST_RE.fullmatch(value) is None:
        raise ReleaseApprovalError(f"{label} must be one lowercase SHA-256 digest")
    return value


def _object_id(value: Any, label: str) -> str:
    if not isinstance(value, str) or _OBJECT_ID_RE.fullmatch(value) is None:
        raise ReleaseApprovalError(f"{label} must be one lowercase Git object ID")
    return value


def _approval_class(value: Any, label: str) -> ReleaseApprovalClass:
    if not isinstance(value, str):
        raise ReleaseApprovalError(f"{label} must name one approval class")
    try:
        return ReleaseApprovalClass(value)
    except ValueError as error:
        raise ReleaseApprovalError(f"{label} must name one approval class") from error


def _approved_at(value: Any) -> str:
    if not isinstance(value, str) or _UTC_SECONDS_RE.fullmatch(value) is None:
        raise ReleaseApprovalError("approved_at must be UTC with exact whole-second syntax")
    try:
        parsed = datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(
            tzinfo=timezone.utc
        )
    except ValueError as error:
        raise ReleaseApprovalError("approved_at is not a valid UTC instant") from error
    if parsed.strftime("%Y-%m-%dT%H:%M:%SZ") != value:
        raise ReleaseApprovalError("approved_at is not canonical UTC")
    return value


def _command_argument(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > MAX_COMMAND_ARGUMENT_BYTES
        or any(ord(character) < 0x20 or ord(character) > 0x7E for character in value)
    ):
        raise ReleaseApprovalError(f"{label} must be bounded printable ASCII")
    normalized = value.replace("\\", "/")
    option_value = normalized.split("=", 1)[1] if "=" in normalized else normalized
    if (
        normalized.startswith(("/", "~/", "file://", "\\\\"))
        or option_value.startswith(("/", "~/", "file://", "//"))
        or _WINDOWS_ABSOLUTE_RE.match(normalized) is not None
        or _WINDOWS_ABSOLUTE_RE.match(option_value) is not None
        or ".." in normalized.split("/")
    ):
        raise ReleaseApprovalError(f"{label} discloses an original or escaping path")
    return value


_CANDIDATE_OID_SLOT = "{candidate_oid}"
_CANDIDATE_TREE_SLOT = "{candidate_tree}"
_TOOL_MANIFEST_SHA256_SLOT = "{protected_tool_manifest_sha256}"
_EVIDENCE_ROOT_ID_SLOT = "{evidence_root_id}"
_PLAN_BINDING_SLOTS = frozenset(
    {
        _CANDIDATE_OID_SLOT,
        _CANDIDATE_TREE_SLOT,
        _TOOL_MANIFEST_SHA256_SLOT,
        _EVIDENCE_ROOT_ID_SLOT,
    }
)


def _planned(
    operation_id: str, tool_id: str, *arguments: str
) -> PlannedApprovalOperation:
    return PlannedApprovalOperation(operation_id, tool_id, tuple(arguments))


_OFFLINE_TOOLCHAIN_SDK_PLANS = (
    _planned("offline-rustc-version", "rustc", "--version"),
    _planned("offline-cargo-version", "cargo", "--version"),
    _planned(
        "offline-workspace-build",
        "cargo",
        "build",
        "--locked",
        "--offline",
        "--workspace",
    ),
    _planned(
        "g-unit-production-864",
        "release-runner",
        "operation:g-unit-production-864.v1",
    ),
    _planned(
        "g-unit-focused-530",
        "release-runner",
        "operation:g-unit-focused-530.v1",
    ),
    _planned(
        "offline-workspace-clippy",
        "cargo",
        "clippy",
        "--locked",
        "--offline",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ),
    _planned(
        "offline-workspace-format",
        "cargo",
        "fmt",
        "--all",
        "--",
        "--check",
    ),
    _planned(
        "offline-no-legacy-codec",
        "bash",
        "scripts/check_no_legacy_codec.sh",
    ),
    _planned(
        "sdk-rust-regeneration-first",
        "cargo",
        "run",
        "--locked",
        "--offline",
        "-p",
        "iroha_data_model",
        "--features",
        "dev-tools",
        "--bin",
        "sumeragi_v2_wire_fixtures",
        "--",
        "--out-dir-id",
        "archive:release-sdk.rust-regeneration-first.v1",
    ),
    _planned(
        "sdk-rust-regeneration-second",
        "cargo",
        "run",
        "--locked",
        "--offline",
        "-p",
        "iroha_data_model",
        "--features",
        "dev-tools",
        "--bin",
        "sumeragi_v2_wire_fixtures",
        "--",
        "--out-dir-id",
        "archive:release-sdk.rust-regeneration-second.v1",
    ),
    _planned(
        "sdk-regeneration-byte-identity",
        "python3",
        "-I",
        "-S",
        "ci/resolve_sumeragi_v2_sdk_source_closure.py",
        "--suite",
        "native-amx-v2-grouped",
        "--check-regeneration",
        "rust-fixtures",
        "--first-output-root-id",
        "archive:release-sdk.rust-regeneration-first.v1",
        "--second-output-root-id",
        "archive:release-sdk.rust-regeneration-second.v1",
    ),
    *tuple(
        _planned(
            f"sdk-grouped-{surface}",
            "bash",
            "ci/run_native_amx_v2_grouped_sdk_parity.sh",
            surface,
        )
        for surface in ("openapi", "python", "javascript", "swift", "kotlin", "java")
    ),
    _planned(
        "sdk-diagnostics-rust",
        "cargo",
        "test",
        "--locked",
        "--offline",
        "-p",
        "iroha",
        "--lib",
        "client::tests::get_sumeragi_",
        "--",
        "--test-threads=1",
    ),
    *tuple(
        _planned(
            f"sdk-diagnostics-{surface}",
            "bash",
            "ci/run_sumeragi_v2_sdk_diagnostics.sh",
            surface,
        )
        for surface in ("python", "javascript", "swift", "kotlin", "java")
    ),
)

_FORMAL_MUTATION_COMMANDS = (
    ("service-rank", "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh"),
    ("productive", "scripts/formal/run_sumeragi_v2_productive_mutation.sh"),
    ("candidate-restart", "scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh"),
    (
        "commit-import-provenance",
        "scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh",
    ),
    (
        "restart-locked-fetch-order",
        "scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh",
    ),
    (
        "persist-install-generation",
        "scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh",
    ),
    (
        "persist-install-validation",
        "scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh",
    ),
    ("apply-authority", "scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh"),
    (
        "replay-locked-body-carrier",
        "scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh",
    ),
    (
        "certificate-ref-recovery",
        "scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh",
    ),
    (
        "certified-response-source-lineage",
        "scripts/formal/run_sumeragi_v2_certified_response_source_lineage_mutation.sh",
    ),
    (
        "certified-response-identity-separation",
        "scripts/formal/run_sumeragi_v2_certified_response_identity_separation_mutation.sh",
    ),
    ("progress", "scripts/formal/run_sumeragi_v2_progress_mutations.sh"),
    (
        "begin-timeout-ready",
        "scripts/formal/run_sumeragi_v2_begin_timeout_ready_mutation.sh",
    ),
    (
        "command-execution-ready",
        "scripts/formal/run_sumeragi_v2_command_execution_ready_mutation.sh",
    ),
    (
        "post-decision-timeout",
        "scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh",
    ),
    (
        "decision-recovery-lifecycle",
        "scripts/formal/run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh",
    ),
    (
        "certified-response-registration",
        "scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh",
    ),
    (
        "effect-capacity-ownership",
        "scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh",
    ),
    (
        "applied-phase-admission",
        "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh",
    ),
    (
        "ingress-causal-freshness",
        "scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh",
    ),
    (
        "liveness-ownership",
        "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh",
    ),
    (
        "serve-scheduler-ordinal",
        "scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh",
    ),
    (
        "indexed-service-activation",
        "scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh",
    ),
    (
        "adequate-leader-readiness",
        "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh",
    ),
    ("indexed-height", "scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh"),
    (
        "item-carrier-typing",
        "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh",
    ),
    (
        "reply-writer-deadline",
        "scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh",
    ),
    (
        "historical-discovery-occurrence-rank",
        "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh",
    ),
    (
        "typed-rollover-handoff",
        "scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh",
    ),
)

_FORMAL_PROOF_TOOLS_PLANS = (
    _planned(
        "formal-proof-ledger",
        "python3",
        "-I",
        "-S",
        "scripts/formal/check_sumeragi_v2_proof_ledger.py",
    ),
    _planned("formal-tlaps", "bash", "scripts/formal/run_sumeragi_v2_tlaps.sh"),
    *tuple(
        _planned(f"formal-mutation-{name}", "bash", relative)
        for name, relative in _FORMAL_MUTATION_COMMANDS
    ),
    _planned(
        "formal-tlc-positive-and-mutations",
        "bash",
        "scripts/formal/run_sumeragi_v2_tlc.sh",
        "ci",
    ),
    _planned(
        "formal-apalache-refinement",
        "bash",
        "scripts/formal/run_sumeragi_v2_multilane_apalache.sh",
    ),
    _planned(
        "formal-production-trace-replay",
        "bash",
        "scripts/formal/check_sumeragi_v2_replay_trace.sh",
    ),
    _planned(
        "formal-rust-verus-correspondence",
        "bash",
        "scripts/verify_sumeragi_v2.sh",
    ),
    _planned(
        "formal-verus-evidence-validation",
        "python3",
        "-I",
        "-S",
        "scripts/formal/sumeragi_v2_verus_evidence.py",
        "validate",
        "--evidence-id",
        "archive:release-formal.verus-evidence.v1",
    ),
    _planned(
        "formal-cross-tool-evidence",
        "python3",
        "-I",
        "-S",
        "scripts/formal/check_sumeragi_v2_proof_ledger.py",
        "--release",
        "--evidence-id",
        "archive:release-formal.proof-evidence.v1",
        "--verus-evidence-id",
        "archive:release-formal.verus-evidence.v1",
        "--write-cross-tool-evidence-id",
        "archive:release-formal.cross-tool-evidence.v1",
    ),
)

_NETWORK_SCALE_SOAK_PLANS = (
    _planned(
        "network-release-seed-matrix",
        "bash",
        "scripts/run_sumeragi_v2_seed_matrix.sh",
        "--release",
    ),
    _planned(
        "network-g4p-mandatory-cases",
        "bash",
        "scripts/run_nexus_cross_dataspace_atomic_swap.sh",
        "--release",
        "--capture",
        "--test-threads",
        "1",
        "--multilane-four-peer-release",
    ),
    _planned(
        "network-g12p-ten-seeds",
        "bash",
        "scripts/run_nexus_cross_dataspace_atomic_swap.sh",
        "--release",
        "--capture",
        "--test-threads",
        "1",
    ),
    _planned(
        "network-g12p-rotating-fault-soak",
        "bash",
        "scripts/run_nexus_cross_dataspace_atomic_swap.sh",
        "--release",
        "--capture",
        "--test-threads",
        "1",
        "--cross-dataspace-fault-soak",
        "--cross-dataspace-seed",
        "nexus-cross-dataspace-v1-seed-00",
        "--cross-dataspace-soak-duration-secs",
        "7200",
    ),
    _planned(
        "scale-five-paired-trials",
        "python3",
        "-I",
        "-S",
        "scripts/nexus/run_multilane_scaling_gate.py",
        "--operation-id",
        "release-scaling.five-paired-trials.v1",
        "--hardware-identity-id",
        "archive:release-scaling.hardware-identity.v1",
        "--configuration-id",
        "archive:release-scaling.configuration.v1",
        "--trial-harness-id",
        "archive:release-scaling.trial-harness.v1",
        "--evidence-root-id",
        _EVIDENCE_ROOT_ID_SLOT,
    ),
    _planned(
        "scale-evidence-validation",
        "python3",
        "-I",
        "-S",
        "scripts/nexus/validate_multilane_scaling_evidence.py",
        "archive:release-scaling.evidence-manifest.v1",
        "--expected-source-revision",
        _CANDIDATE_OID_SLOT,
        "--quiet",
    ),
    _planned(
        "network-chaos-100000-height",
        "bash",
        "scripts/run_sumeragi_v2_100k_chaos.sh",
    ),
    _planned(
        "network-taira-24h-soak",
        "bash",
        "scripts/run_taira_v2_24h_soak.sh",
    ),
)

_FINAL_BOOTSTRAP_PUBLICATION_PLANS = (
    _planned(
        "final-protected-bootstrap",
        "python3",
        "-I",
        "-S",
        "scripts/bootstrap_sumeragi_v2_release.py",
        "--candidate-archive-id",
        "archive:release-candidate.signed-immutable.v1",
        "--candidate-oid",
        _CANDIDATE_OID_SLOT,
        "--candidate-tree",
        _CANDIDATE_TREE_SLOT,
        "--tool-manifest-archive-id",
        "archive:release-tools.protected-manifest.v1",
        "--tool-manifest-sha256",
        _TOOL_MANIFEST_SHA256_SLOT,
        "--evidence-root-id",
        _EVIDENCE_ROOT_ID_SLOT,
    ),
    _planned(
        "final-release-runner",
        "bash",
        "scripts/run_sumeragi_v2_release_gates.sh",
        "--release",
    ),
    _planned(
        "final-canonical-receipt-publication",
        "python3",
        "-I",
        "-S",
        "scripts/write_sumeragi_v2_release_receipt.py",
        "--operation-id",
        "release-receipt.publish-canonical-0400-single-link.v1",
        "--output-archive-id",
        "archive:release-receipt.canonical.v1",
    ),
    _planned(
        "final-no-clobber-validator-acknowledgment",
        "receipt-validator",
        "release-receipt.validate-and-ack-no-clobber.v1",
        "--validator-archive-id",
        "archive:release-bootstrap.receipt-validator.v1",
        "--receipt-archive-id",
        "archive:release-receipt.canonical.v1",
        "--ack-archive-id",
        "archive:release-retained.receipt-validation-ack.v3",
    ),
    _planned(
        "final-private-state-prune",
        "release-runtime-helper",
        "operation:release-private-state.prune-after-ack.v1",
        "--invocation-archive-id",
        "archive:release-retained.invocation.v1",
    ),
    _planned(
        "final-retained-inventory-and-result-publication",
        "release-runtime-helper",
        "operation:release-retained.publish-inventory-and-result.v1",
        "--inventory-archive-id",
        "archive:release-retained.inventory.v2",
        "--result-archive-id",
        "archive:release-retained.result.v1",
    ),
    _planned(
        "final-bootstrap-independent-authentication",
        "release-bootstrap",
        "operation:release-retained.authenticate-receipt-tools-gates.v1",
        "--source-archive-id",
        "archive:release-retained.source.v1",
        "--receipt-archive-id",
        "archive:release-receipt.canonical.v1",
        "--evidence-root-id",
        _EVIDENCE_ROOT_ID_SLOT,
    ),
    _planned(
        "final-external-completion-publication",
        "release-bootstrap",
        "operation:release-completion.publish-after-authentication.v1",
        "--completion-archive-id",
        "archive:release-bootstrap.external-completion.v1",
    ),
)


APPROVAL_OPERATION_PLANS: Mapping[
    ReleaseApprovalClass, tuple[PlannedApprovalOperation, ...]
] = MappingProxyType(
    {
        ReleaseApprovalClass.OFFLINE_TOOLCHAIN_SDK: _OFFLINE_TOOLCHAIN_SDK_PLANS,
        ReleaseApprovalClass.FORMAL_PROOF_TOOLS: _FORMAL_PROOF_TOOLS_PLANS,
        ReleaseApprovalClass.NETWORK_SCALE_SOAK: _NETWORK_SCALE_SOAK_PLANS,
        ReleaseApprovalClass.FINAL_BOOTSTRAP_PUBLICATION: (
            _FINAL_BOOTSTRAP_PUBLICATION_PLANS
        ),
    }
)
APPROVAL_OPERATION_PLAN_SHA256: Mapping[ReleaseApprovalClass, str] = (
    MappingProxyType(
        {
            approval_class: hashlib.sha256(
                _canonical_json(
                    [
                        {
                            "arguments": list(plan.arguments),
                            "operation_id": plan.operation_id,
                            "ordinal": ordinal,
                            "tool_id": plan.tool_id,
                        }
                        for ordinal, plan in enumerate(plans)
                    ]
                )
            ).hexdigest()
            for approval_class, plans in APPROVAL_OPERATION_PLANS.items()
        }
    )
)

# Keep the required protected consumer APIs and actions source-bound here so
# focused contract tests detect any consumer that stops independently loading,
# binding, sanitizing, or replaying the four approval records.
APPROVAL_REQUIRED_CONSUMER_APIS: Mapping[str, tuple[str, ...]] = MappingProxyType(
    {
        "scripts/bootstrap_sumeragi_v2_release.py": (
            "build_release_approval_expectations",
            "load_protected_release_approval_set",
            "sanitized_release_approval_set_archive",
        ),
        "scripts/validate_sumeragi_v2_release_bootstrap.py": (
            "build_release_approval_expectations",
            "load_protected_release_approval_set",
            "sanitized_release_approval_set_archive",
        ),
        "scripts/write_sumeragi_v2_release_receipt.py": (
            "require_release_approval_binding",
            "sanitized_release_approval_set_archive",
        ),
    }
)
APPROVAL_REQUIRED_CONSUMER_ACTIONS: Mapping[str, tuple[str, ...]] = (
    MappingProxyType(
        {
            "scripts/bootstrap_sumeragi_v2_release.py": (
                "import-component",
                "protected-load-and-exact-bind",
                "publish-sanitized-archive",
            ),
            "scripts/validate_sumeragi_v2_release_bootstrap.py": (
                "import-component",
                "independently-replay-sanitized-archive",
            ),
            "scripts/write_sumeragi_v2_release_receipt.py": (
                "import-component",
                "retain-sanitized-archive-and-digests",
            ),
        }
    )
)


def _bounded_duration(value: Any, label: str) -> int:
    if (
        type(value) is not int
        or value <= 0
        or value > MAX_EXPECTED_DURATION_SECONDS
    ):
        raise ReleaseApprovalError(f"{label} is outside its bound")
    return value


def _materialize_approval_operations(
    approval_class: ReleaseApprovalClass,
    bindings: Mapping[str, str],
) -> tuple[ApprovalOperation, ...]:
    plans = APPROVAL_OPERATION_PLANS[approval_class]
    operations: list[ApprovalOperation] = []
    for ordinal, plan in enumerate(plans):
        operation_id = _identifier(plan.operation_id, "planned operation_id")
        tool_id = _identifier(plan.tool_id, "planned tool_id")
        arguments = tuple(
            _command_argument(
                bindings.get(argument, argument),
                f"planned operation {ordinal} argument {index}",
            )
            for index, argument in enumerate(plan.arguments)
        )
        operations.append(
            ApprovalOperation(
                ordinal=ordinal,
                operation_id=operation_id,
                tool_id=tool_id,
                arguments=arguments,
            )
        )
    if not operations or len(operations) > MAX_APPROVAL_OPERATIONS:
        raise ReleaseApprovalError("planned approval operation count is invalid")
    if len({operation.operation_id for operation in operations}) != len(operations):
        raise ReleaseApprovalError("planned approval repeats an operation_id")
    unresolved = {
        argument
        for operation in operations
        for argument in operation.arguments
        if argument in _PLAN_BINDING_SLOTS
    }
    if unresolved:
        raise ReleaseApprovalError("planned approval has an unresolved binding")
    return tuple(operations)


def build_release_approval_expectations(
    *,
    candidate_oid: str,
    candidate_tree: str,
    protected_tool_manifest_sha256: str,
    evidence_root_id: str,
    offline_toolchain_sdk_duration_seconds: int,
    formal_proof_tools_duration_seconds: int,
    network_scale_soak_duration_seconds: int,
    final_bootstrap_publication_duration_seconds: int,
) -> dict[ReleaseApprovalClass, ReleaseApprovalExpectation]:
    """Build the exact class-specific approval expectations for one release.

    The result is ordered by :data:`APPROVAL_CLASS_ORDER` and can be passed
    directly to :func:`load_protected_release_approval_set`.  Command records
    use only relative repository arguments and normalized archive/evidence
    identifiers.  They are approval comparison records, not a substitute CLI
    and not a claim about an approver's cryptographic identity.
    """

    normalized_oid = _object_id(candidate_oid, "expected candidate_oid")
    normalized_tree = _object_id(candidate_tree, "expected candidate_tree")
    if len(normalized_oid) != len(normalized_tree):
        raise ReleaseApprovalError("release approval expectation mixes object formats")
    normalized_tool_manifest = _digest(
        protected_tool_manifest_sha256,
        "expected protected tool manifest",
    )
    normalized_evidence_root = _identifier(
        evidence_root_id,
        "expected evidence_root_id",
    )
    durations = {
        ReleaseApprovalClass.OFFLINE_TOOLCHAIN_SDK: _bounded_duration(
            offline_toolchain_sdk_duration_seconds,
            "offline-toolchain-sdk expected duration",
        ),
        ReleaseApprovalClass.FORMAL_PROOF_TOOLS: _bounded_duration(
            formal_proof_tools_duration_seconds,
            "formal-proof-tools expected duration",
        ),
        ReleaseApprovalClass.NETWORK_SCALE_SOAK: _bounded_duration(
            network_scale_soak_duration_seconds,
            "network-scale-soak expected duration",
        ),
        ReleaseApprovalClass.FINAL_BOOTSTRAP_PUBLICATION: _bounded_duration(
            final_bootstrap_publication_duration_seconds,
            "final-bootstrap-publication expected duration",
        ),
    }
    bindings = {
        _CANDIDATE_OID_SLOT: normalized_oid,
        _CANDIDATE_TREE_SLOT: normalized_tree,
        _TOOL_MANIFEST_SHA256_SLOT: normalized_tool_manifest,
        _EVIDENCE_ROOT_ID_SLOT: normalized_evidence_root,
    }
    return {
        approval_class: ReleaseApprovalExpectation(
            class_id=approval_class,
            candidate_oid=normalized_oid,
            candidate_tree=normalized_tree,
            profile="production",
            operations=_materialize_approval_operations(approval_class, bindings),
            protected_tool_manifest_sha256=normalized_tool_manifest,
            evidence_root_id=normalized_evidence_root,
            expected_duration_seconds=durations[approval_class],
        )
        for approval_class in APPROVAL_CLASS_ORDER
    }


def _decode_operation(value: Any, ordinal: int) -> ApprovalOperation:
    record = _exact_dict(value, _OPERATION_FIELDS, f"operation {ordinal}")
    if type(record["ordinal"]) is not int or record["ordinal"] != ordinal:
        raise ReleaseApprovalError("approval operation ordinals must be contiguous and ordered")
    arguments = record["arguments"]
    if not isinstance(arguments, list) or len(arguments) > MAX_COMMAND_ARGUMENTS:
        raise ReleaseApprovalError("approval operation arguments exceed their exact bound")
    return ApprovalOperation(
        ordinal=ordinal,
        operation_id=_identifier(
            record["operation_id"], f"operation {ordinal} operation_id"
        ),
        tool_id=_identifier(record["tool_id"], f"operation {ordinal} tool_id"),
        arguments=tuple(
            _command_argument(argument, f"operation {ordinal} argument {index}")
            for index, argument in enumerate(arguments)
        ),
    )


def _decode_approval(
    data: bytes, expected_class: ReleaseApprovalClass
) -> dict[str, Any]:
    try:
        value = json.loads(
            data.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_pairs,
            parse_constant=_reject_nonfinite,
            parse_float=_reject_float,
        )
    except ReleaseApprovalError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseApprovalError("release approval is not canonical UTF-8 JSON") from error
    document = _exact_dict(value, _TOP_LEVEL_FIELDS, "release approval")
    if data != _canonical_json(document):
        raise ReleaseApprovalError("release approval is not canonical UTF-8 JSON")
    if document["format"] != APPROVAL_FORMAT:
        raise ReleaseApprovalError("release approval has the wrong format")
    if (
        type(document["schema_version"]) is not int
        or document["schema_version"] != APPROVAL_SCHEMA_VERSION
    ):
        raise ReleaseApprovalError("release approval must use schema version 1")
    class_id = _approval_class(document["class_id"], "class_id")
    if class_id is not expected_class:
        raise ReleaseApprovalError("release approval has the wrong approval class")
    candidate_oid = _object_id(document["candidate_oid"], "candidate_oid")
    candidate_tree = _object_id(document["candidate_tree"], "candidate_tree")
    if len(candidate_oid) != len(candidate_tree):
        raise ReleaseApprovalError("release approval mixes Git object formats")
    operations_value = document["operations"]
    if (
        not isinstance(operations_value, list)
        or not operations_value
        or len(operations_value) > MAX_APPROVAL_OPERATIONS
    ):
        raise ReleaseApprovalError("release approval has an invalid operation count")
    operations = tuple(
        _decode_operation(operation, ordinal)
        for ordinal, operation in enumerate(operations_value)
    )
    if len({operation.operation_id for operation in operations}) != len(operations):
        raise ReleaseApprovalError("release approval repeats an operation_id")
    duration = document["expected_duration_seconds"]
    if (
        type(duration) is not int
        or duration <= 0
        or duration > MAX_EXPECTED_DURATION_SECONDS
    ):
        raise ReleaseApprovalError("expected_duration_seconds is outside its bound")
    return {
        "approval_id": _identifier(document["approval_id"], "approval_id"),
        "approved_at": _approved_at(document["approved_at"]),
        "candidate_oid": candidate_oid,
        "candidate_tree": candidate_tree,
        "class_id": class_id,
        "evidence_root_id": _identifier(
            document["evidence_root_id"], "evidence_root_id"
        ),
        "expected_duration_seconds": duration,
        "operations": operations,
        "profile": _identifier(document["profile"], "profile"),
        "protected_tool_manifest_sha256": _digest(
            document["protected_tool_manifest_sha256"],
            "protected_tool_manifest_sha256",
        ),
    }


def _capture_trusted_ancestors(
    path: Path, expected_owner_uid: int
) -> tuple[_DirectoryIdentity, ...]:
    identities: list[_DirectoryIdentity] = []
    for ancestor in (path.parent, *path.parent.parents):
        try:
            metadata = ancestor.lstat()
        except OSError as error:
            raise ReleaseApprovalError("approval ancestry is unavailable") from error
        mode = stat.S_IMODE(metadata.st_mode)
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid not in {0, expected_owner_uid}
            or mode & 0o022
        ):
            raise ReleaseApprovalError(
                "approval has a writable, symlinked, or untrusted ancestor"
            )
        identities.append(
            _DirectoryIdentity(
                path=ancestor,
                device=metadata.st_dev,
                inode=metadata.st_ino,
                owner_uid=metadata.st_uid,
                mode=mode,
            )
        )
    return tuple(identities)


def _revalidate_trusted_ancestors(
    identities: Sequence[_DirectoryIdentity], expected_owner_uid: int
) -> None:
    for expected in identities:
        try:
            metadata = expected.path.lstat()
        except OSError as error:
            raise ReleaseApprovalError("approval ancestry changed while reading") from error
        observed = (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_uid,
            stat.S_IMODE(metadata.st_mode),
        )
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid not in {0, expected_owner_uid}
            or stat.S_IMODE(metadata.st_mode) & 0o022
            or observed
            != (
                expected.device,
                expected.inode,
                expected.owner_uid,
                expected.mode,
            )
        ):
            raise ReleaseApprovalError("approval ancestry changed while reading")


def _read_protected_approval(
    path: Path, expected_owner_uid: int
) -> tuple[bytes, os.stat_result]:
    if (
        not isinstance(path, Path)
        or not path.is_absolute()
        or Path(os.path.abspath(path)) != path
    ):
        raise ReleaseApprovalError("approval path must be absolute and normalized")
    if type(expected_owner_uid) is not int or expected_owner_uid < 0:
        raise ReleaseApprovalError("approval owner UID is invalid")
    try:
        if path.resolve(strict=True) != path:
            raise ReleaseApprovalError("approval path must not contain symlinks")
        before_path = path.lstat()
    except ReleaseApprovalError:
        raise
    except OSError as error:
        raise ReleaseApprovalError("approval file is unavailable") from error
    ancestors = _capture_trusted_ancestors(path, expected_owner_uid)
    if (
        stat.S_ISLNK(before_path.st_mode)
        or not stat.S_ISREG(before_path.st_mode)
        or before_path.st_uid != expected_owner_uid
        or stat.S_IMODE(before_path.st_mode) != PROTECTED_APPROVAL_MODE
        or before_path.st_nlink != 1
        or before_path.st_size > MAX_APPROVAL_BYTES
    ):
        raise ReleaseApprovalError(
            "approval must be an owner-held 0400 single-link bounded regular file"
        )
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if not nofollow:
        raise ReleaseApprovalError("approval no-follow file opening is unavailable")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | nofollow
    descriptor = -1
    try:
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        stable = (
            opened.st_dev,
            opened.st_ino,
            opened.st_mode,
            opened.st_uid,
            opened.st_nlink,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
        )
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino)
            != (before_path.st_dev, before_path.st_ino)
            or opened.st_uid != expected_owner_uid
            or stat.S_IMODE(opened.st_mode) != PROTECTED_APPROVAL_MODE
            or opened.st_nlink != 1
            or opened.st_size > MAX_APPROVAL_BYTES
        ):
            raise ReleaseApprovalError("approval changed while opening")
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(descriptor, min(64 * 1024, MAX_APPROVAL_BYTES + 1 - total))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > MAX_APPROVAL_BYTES:
                raise ReleaseApprovalError("approval exceeds its canonical byte bound")
        after = os.fstat(descriptor)
        linked_after = path.lstat()
        if stable != (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_uid,
            after.st_nlink,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        ) or (after.st_dev, after.st_ino) != (
            linked_after.st_dev,
            linked_after.st_ino,
        ):
            raise ReleaseApprovalError("approval changed while reading")
        _revalidate_trusted_ancestors(ancestors, expected_owner_uid)
        return b"".join(chunks), after
    except OSError as error:
        raise ReleaseApprovalError("approval could not be read safely") from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def require_release_approval_binding(
    approval: ValidatedReleaseApproval,
    expectation: ReleaseApprovalExpectation,
) -> None:
    """Require an approval to equal the consumer's exact planned invocation."""

    if not isinstance(expectation, ReleaseApprovalExpectation):
        raise ReleaseApprovalError("release approval expectation has the wrong type")
    candidate_oid = _object_id(expectation.candidate_oid, "expected candidate_oid")
    candidate_tree = _object_id(expectation.candidate_tree, "expected candidate_tree")
    if len(candidate_oid) != len(candidate_tree):
        raise ReleaseApprovalError("release approval expectation mixes object formats")
    profile = _identifier(expectation.profile, "expected profile")
    tool_manifest = _digest(
        expectation.protected_tool_manifest_sha256,
        "expected protected tool manifest",
    )
    evidence_root_id = _identifier(
        expectation.evidence_root_id, "expected evidence_root_id"
    )
    expected_duration_seconds = expectation.expected_duration_seconds
    if (
        type(expected_duration_seconds) is not int
        or expected_duration_seconds <= 0
        or expected_duration_seconds > MAX_EXPECTED_DURATION_SECONDS
    ):
        raise ReleaseApprovalError(
            "expected release duration is outside its bound"
        )
    expected_operations = tuple(expectation.operations)
    for ordinal, operation in enumerate(expected_operations):
        if not isinstance(operation, ApprovalOperation) or operation.ordinal != ordinal:
            raise ReleaseApprovalError(
                "release approval expectation has unordered operations"
            )
        _identifier(operation.operation_id, "expected operation_id")
        _identifier(operation.tool_id, "expected tool_id")
        for index, argument in enumerate(operation.arguments):
            _command_argument(argument, f"expected operation argument {index}")
    if len({operation.operation_id for operation in expected_operations}) != len(
        expected_operations
    ):
        raise ReleaseApprovalError("release approval expectation repeats an operation_id")
    if (
        approval.class_id is not expectation.class_id
        or approval.candidate_oid != candidate_oid
        or approval.candidate_tree != candidate_tree
        or approval.profile != profile
        or approval.operations != expected_operations
        or approval.protected_tool_manifest_sha256 != tool_manifest
        or approval.evidence_root_id != evidence_root_id
        or approval.expected_duration_seconds != expected_duration_seconds
    ):
        raise ReleaseApprovalError(
            "release approval does not bind the exact planned invocation"
        )


def load_protected_release_approval(
    path: Path,
    *,
    expected_class: ReleaseApprovalClass,
    expectation: ReleaseApprovalExpectation | None = None,
    expected_owner_uid: int | None = None,
) -> ValidatedReleaseApproval:
    """Read, validate, and optionally bind one protected approval file."""

    if not isinstance(expected_class, ReleaseApprovalClass):
        raise ReleaseApprovalError("expected approval class has the wrong type")
    owner_uid = os.geteuid() if expected_owner_uid is None else expected_owner_uid
    data, metadata = _read_protected_approval(path, owner_uid)
    fields = _decode_approval(data, expected_class)
    approval = ValidatedReleaseApproval(
        **fields,
        canonical_bytes=data,
        approval_sha256=hashlib.sha256(data).hexdigest(),
        size_bytes=len(data),
        source_mode=stat.S_IMODE(metadata.st_mode),
        source_nlink=metadata.st_nlink,
        source_owner_uid=metadata.st_uid,
    )
    if expectation is not None:
        if expectation.class_id is not expected_class:
            raise ReleaseApprovalError("release approval expectation has the wrong class")
        require_release_approval_binding(approval, expectation)
    return approval


def _class_mapping(
    values: Mapping[ReleaseApprovalClass | str, Any], label: str
) -> dict[ReleaseApprovalClass, Any]:
    if not isinstance(values, Mapping):
        raise ReleaseApprovalError(f"{label} must map the four approval classes")
    normalized: dict[ReleaseApprovalClass, Any] = {}
    for raw_class, value in values.items():
        approval_class = _approval_class(raw_class, f"{label} class")
        if approval_class in normalized:
            raise ReleaseApprovalError(f"{label} repeats an approval class")
        normalized[approval_class] = value
    if set(normalized) != set(APPROVAL_CLASS_ORDER):
        raise ReleaseApprovalError(f"{label} must contain exactly four approval classes")
    return normalized


def load_protected_release_approval_set(
    paths: Mapping[ReleaseApprovalClass | str, Path],
    *,
    expectations: Mapping[
        ReleaseApprovalClass | str, ReleaseApprovalExpectation
    ] | None = None,
    expected_owner_uid: int | None = None,
) -> tuple[ValidatedReleaseApproval, ...]:
    """Load the exact four independent approvals for one immutable candidate.

    When expectations are supplied, the mapping requires a distinct
    class-typed expectation for every approval class, including its exact
    ordered operations and duration.
    """

    normalized_paths = _class_mapping(paths, "approval paths")
    normalized_expectations = (
        None
        if expectations is None
        else _class_mapping(expectations, "approval expectations")
    )
    approvals = tuple(
        load_protected_release_approval(
            normalized_paths[approval_class],
            expected_class=approval_class,
            expectation=(
                None
                if normalized_expectations is None
                else normalized_expectations[approval_class]
            ),
            expected_owner_uid=expected_owner_uid,
        )
        for approval_class in APPROVAL_CLASS_ORDER
    )
    if len({approval.approval_id for approval in approvals}) != len(approvals):
        raise ReleaseApprovalError("release approval set repeats an approval_id")
    candidate_pairs = {
        (approval.candidate_oid, approval.candidate_tree) for approval in approvals
    }
    if len(candidate_pairs) != 1:
        raise ReleaseApprovalError("release approval set names more than one candidate")
    return approvals


def sanitized_release_approval_set_archive(
    approvals: Sequence[ValidatedReleaseApproval],
) -> SanitizedApprovalArchive:
    """Project the exact four approval attestations into one path-free index."""

    by_class: dict[ReleaseApprovalClass, ValidatedReleaseApproval] = {}
    for approval in approvals:
        if not isinstance(approval, ValidatedReleaseApproval):
            raise ReleaseApprovalError("approval archive set contains an invalid value")
        if approval.class_id in by_class:
            raise ReleaseApprovalError("approval archive set repeats a class")
        by_class[approval.class_id] = approval
    if set(by_class) != set(APPROVAL_CLASS_ORDER):
        raise ReleaseApprovalError("approval archive set must contain exactly four classes")
    ordered = tuple(by_class[value] for value in APPROVAL_CLASS_ORDER)
    if len({(value.candidate_oid, value.candidate_tree) for value in ordered}) != 1:
        raise ReleaseApprovalError("approval archive set names more than one candidate")
    projections = tuple(value.sanitized_archive() for value in ordered)
    projection = {
        "approvals": [
            {
                "approval_id": approval.approval_id,
                "approval_sha256": approval.approval_sha256,
                "archive_id": APPROVAL_ARCHIVE_IDS[approval.class_id],
                "class_id": approval.class_id.value,
                "projection_sha256": sanitized.sha256,
            }
            for approval, sanitized in zip(ordered, projections)
        ],
        "candidate_oid": ordered[0].candidate_oid,
        "candidate_tree": ordered[0].candidate_tree,
        "format": APPROVAL_SET_ARCHIVE_FORMAT,
        "schema_version": APPROVAL_SCHEMA_VERSION,
    }
    canonical = _canonical_json(projection)
    return SanitizedApprovalArchive(
        value=projection,
        canonical_bytes=canonical,
        sha256=hashlib.sha256(canonical).hexdigest(),
    )


__all__ = (
    "APPROVAL_ARCHIVE_FORMAT",
    "APPROVAL_ARCHIVE_IDS",
    "APPROVAL_CLASS_IDS",
    "APPROVAL_CLASS_ORDER",
    "APPROVAL_FILENAMES",
    "APPROVAL_FORMAT",
    "APPROVAL_OPERATION_PLANS",
    "APPROVAL_OPERATION_PLAN_SHA256",
    "APPROVAL_REQUIRED_CONSUMER_ACTIONS",
    "APPROVAL_REQUIRED_CONSUMER_APIS",
    "APPROVAL_SCHEMA_VERSION",
    "ApprovalOperation",
    "MAX_APPROVAL_BYTES",
    "MAX_EXPECTED_DURATION_SECONDS",
    "PlannedApprovalOperation",
    "PROTECTED_APPROVAL_MODE",
    "ReleaseApprovalClass",
    "ReleaseApprovalError",
    "ReleaseApprovalExpectation",
    "SanitizedApprovalArchive",
    "ValidatedReleaseApproval",
    "build_release_approval_expectations",
    "load_protected_release_approval",
    "load_protected_release_approval_set",
    "require_release_approval_binding",
    "sanitized_release_approval_set_archive",
)
