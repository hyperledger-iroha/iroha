#!/usr/bin/env python3
"""Build payload-free SoraFS AI pre-screening rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_ai_prescreen_rollout_evidence import (  # noqa: E402
    ALLOWED_PRESCREEN_VERDICTS,
    FORBIDDEN_SUBJECT_REFERENCE_MARKERS,
    FORBIDDEN_WORKFLOW_ID_MARKERS,
    KIND_BY_NAME,
    REQUIRED_E2E_STEPS,
    REQUIRED_GOVERNANCE_EDGE_COUNT,
    REQUIRED_GOVERNANCE_PRODUCERS,
    REQUIRED_OPERATOR_ROUTES,
    REQUIRED_OPERATOR_SCHEMAS,
    REQUIRED_TRANSPARENCY_SOURCE_KINDS,
    SUBJECT_REFERENCE_ERROR,
    SUBJECT_REFERENCE_PATTERN,
    WORKFLOW_ID_ERROR,
    WORKFLOW_ID_PATTERN,
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
from sorafs_path_identity import path_diagnostic_label  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)
from sorafs_evidence_validation import is_archive_portable_artifact_path  # noqa: E402
from sorafs_runner_preflight import runner_url_arg_is_plan_safe  # noqa: E402


CANARY_KINDS = tuple(KIND_BY_NAME)
RUNNER_BINDING_KINDS = ("runner", "committee")
WORKFLOW_DIGEST_KINDS = (
    "operator_workflow",
    "notification_transport",
    "commit_reveal_executor",
    "transparency_publication",
    "governance_dag",
    "end_to_end_workflow",
)
HEX32_LEN = 32
HEX64_LEN = 64
RUNNER_STATUS_KINDS = {"runner", "committee"}
CANARY_URL_ARG_ERROR = (
    "SoraFS AI pre-screen canary URL arguments must not contain userinfo, "
    "query strings, fragments, control characters, encoded traversal, "
    "separators, drive prefixes, URI-scheme-like path tokens, or "
    "secret-looking host/path components"
)
CANARY_PATH_ARG_ERROR = (
    "SoraFS AI pre-screen canary path arguments must be archive-relative without "
    "absolute, empty, current, parent, encoded, URI-scheme-like, or "
    "platform-specific segments"
)


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into canonical strings."""

    items: list[str] = []
    for value in values:
        for item in value.split(","):
            stripped = item.strip()
            if stripped:
                items.append(stripped)
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


def validate_verdict(value: str, *, option: str, errors: list[str]) -> None:
    """Require a shipped moderation screening verdict label."""

    if value not in ALLOWED_PRESCREEN_VERDICTS:
        allowed = ", ".join(ALLOWED_PRESCREEN_VERDICTS[:-1])
        errors.append(
            f"{option} must be {allowed}, or {ALLOWED_PRESCREEN_VERDICTS[-1]}"
        )


def validate_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    kind: str,
    count_option: str,
    errors: list[str],
) -> list[str]:
    """Return reviewed unique inventory labels whose count matches a CLI count."""

    items = list(values)
    if not items:
        errors.append(f"{option} is required for {kind}")
    for index, item in enumerate(items):
        validate_canonical_string(item, label=f"{option}[{index}]", errors=errors)
    unique_items = set(items)
    if len(unique_items) != len(items):
        errors.append(f"{option} must not contain duplicates")
    if len(unique_items) != expected_count:
        errors.append(f"{option} unique values must match {count_option}")
    return items


def validate_governance_edges(
    values: Iterable[str],
    *,
    expected_count: int,
    errors: list[str],
) -> list[dict[str, str]]:
    """Return reviewed governance DAG edge rows from PRODUCER:NAME values."""

    items = list(values)
    if not items:
        errors.append("--governance-edge is required for governance_dag")
    records: list[dict[str, str]] = []
    names: list[str] = []
    producers: list[str] = []
    for index, item in enumerate(items):
        if ":" not in item:
            errors.append("--governance-edge values must use <producer>:<name>")
            continue
        producer, name = item.split(":", 1)
        producer = producer.strip()
        name = name.strip()
        validate_canonical_string(
            producer,
            label=f"--governance-edge[{index}].producer",
            errors=errors,
        )
        validate_canonical_string(
            name,
            label=f"--governance-edge[{index}].name",
            errors=errors,
        )
        if producer and producer not in REQUIRED_GOVERNANCE_PRODUCERS:
            errors.append("--governance-edge producer must be a required producer")
        if producer and name:
            records.append({"producer": producer, "name": name})
            names.append(name)
            producers.append(producer)
    if len(set(names)) != len(names):
        errors.append("--governance-edge names must not contain duplicates")
    if len(set(names)) != expected_count:
        errors.append("--governance-edge unique names must match --edge-count")
    missing_producers = [
        producer
        for producer in REQUIRED_GOVERNANCE_PRODUCERS
        if producer not in set(producers)
    ]
    if missing_producers:
        errors.append("--governance-edge must include every required producer")
    return records


def validate_output_path(path: Path, errors: list[str]) -> None:
    """Reject unsafe output targets before writing a canary artifact."""

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


def validate_hex(
    value: str | None,
    *,
    length: int,
    option: str,
    errors: list[str],
) -> None:
    """Validate an exact lowercase hex string."""

    if (
        not isinstance(value, str)
        or len(value) != length
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{option} must be exact lowercase {length}-character hex")


def validate_canonical_string(value: str | None, *, label: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control characters."""

    if (
        not isinstance(value, str)
        or not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{label} must be a non-empty canonical string")


def validate_canary_url(
    value: str | None,
    *,
    label: str,
    errors: list[str],
    required: bool,
) -> None:
    """Require a canary URL to be canonical and safe for evidence payloads."""

    if value is None and not required:
        return
    previous_error_count = len(errors)
    validate_canonical_string(value, label=label, errors=errors)
    if len(errors) != previous_error_count:
        return
    if not runner_url_arg_is_plan_safe(value):
        if CANARY_URL_ARG_ERROR not in errors:
            errors.append(CANARY_URL_ARG_ERROR)


def validate_canary_path_label(
    value: str | None,
    *,
    label: str,
    errors: list[str],
) -> None:
    """Require a canary path label to be canonical and archive-portable."""

    previous_error_count = len(errors)
    validate_canonical_string(value, label=label, errors=errors)
    if len(errors) != previous_error_count:
        return
    if not is_archive_portable_artifact_path(value):
        if CANARY_PATH_ARG_ERROR not in errors:
            errors.append(CANARY_PATH_ARG_ERROR)


def validate_workflow_id_arg(value: str | None, *, errors: list[str]) -> None:
    """Require a reviewed lowercase SFM-4a workflow identifier."""

    validate_canonical_string(value, label="--workflow-id", errors=errors)
    if not isinstance(value, str):
        return
    if WORKFLOW_ID_PATTERN.fullmatch(value) is None:
        errors.append(WORKFLOW_ID_ERROR.replace("workflow_id", "--workflow-id"))
        return
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_WORKFLOW_ID_MARKERS
        if marker in value.split("-")
    )
    if forbidden:
        errors.append(
            f"--workflow-id must not contain non-production markers {forbidden}"
        )


def validate_subject_arg(value: str | None, *, errors: list[str]) -> None:
    """Require a reviewed lowercase payload-free content subject reference."""

    validate_canonical_string(value, label="--subject", errors=errors)
    if not isinstance(value, str):
        return
    if SUBJECT_REFERENCE_PATTERN.fullmatch(value) is None:
        errors.append(SUBJECT_REFERENCE_ERROR.replace("subject", "--subject"))
        return
    subject_tokens = frozenset(
        token for token in re.split(r"[^a-z0-9]+", value) if token
    )
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_SUBJECT_REFERENCE_MARKERS
        if marker in subject_tokens
    )
    if forbidden:
        errors.append(f"--subject must not contain non-production markers {forbidden}")


def require_kind_options(
    args: argparse.Namespace,
    errors: list[str],
    required: Sequence[tuple[str, Any]],
) -> None:
    """Require kind-specific options by stable CLI flag."""

    for option, value in required:
        if value is None:
            errors.append(f"{option} is required for {args.kind}")


def require_2xx(value: int, *, option: str, errors: list[str]) -> None:
    """Require a successful HTTP status for generated probe rows."""

    if value < 200 or value > 299:
        errors.append(f"{option} must be a 2xx HTTP status code")


def common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by AI pre-screen canary payloads."""

    status = "verified" if args.kind in RUNNER_STATUS_KINDS else "passed"
    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": status,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
    }


def build_operator_route_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free operator workflow route probe records."""

    return [
        {
            "name": route,
            "method": "GET",
            "path": f"/{route}",
            "url": f"{args.operator_url.rstrip('/')}/{route}",
            "status_code": args.route_status_code,
            "schema": REQUIRED_OPERATOR_SCHEMAS.get(route),
            "body_blake3_hex": args.body_digest_hex,
            "body_bytes": args.body_bytes,
            "payload_bytes_included": False,
            "private_payloads_included": False,
        }
        for route in args.operator_routes
    ]


def build_notification_probes(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free notification transport probes."""

    return [
        {
            "delivery_id": f"notify-{index}",
            "dedup_key": f"sorafs-moderation-juror:notify-{index}",
            "action": "commit" if index % 2 else "reveal",
            "case_id": args.case_id,
            "round_id": args.round_id,
            "juror_id": f"juror-{index}@moderation",
            "notification_bytes": args.notification_bytes,
            "notification_body_blake3": args.body_digest_hex,
            "response_status": args.notification_status_code,
            "response_success": True,
            "response_bytes": args.response_bytes,
            "response_body_blake3": args.body_digest_hex,
            "payload_bytes_included": False,
            "private_payloads_included": False,
        }
        for index in range(1, args.probe_count + 1)
    ]


def build_executor_artifacts(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free commit/reveal executor bundle artifact records."""

    return [
        {
            "name": "executor.env",
            "kind": "env",
            "path": "executor.env",
            "exists": True,
            "bytes": args.bundle_artifact_bytes,
            "body_blake3": args.body_digest_hex,
            "passed": True,
            "checks": [{"name": "payload-free", "passed": True}],
            "payload_bytes_included": False,
            "private_payloads_included": False,
        },
        {
            "name": "run.sh",
            "kind": "script",
            "path": "run.sh",
            "exists": True,
            "bytes": args.bundle_artifact_bytes,
            "body_blake3": args.body_digest_hex,
            "passed": True,
            "checks": [{"name": "executable", "passed": True}],
            "payload_bytes_included": False,
            "private_payloads_included": False,
        },
    ]


def build_transparency_probes(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free moderation transparency publication probes."""

    return [
        {
            "source_kind": source_kind,
            "payload_path": f"{source_kind}.json",
            "request_bytes": args.request_bytes,
            "request_body_blake3": args.body_digest_hex,
            "response_status": args.publication_status_code,
            "response_success": True,
            "response_bytes": args.response_bytes,
            "response_body_blake3": args.body_digest_hex,
            "payload_bytes_included": False,
            "private_payloads_included": False,
            "response_body_included": False,
        }
        for source_kind in args.transparency_source_kinds
    ]


def build_workflow_steps(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build end-to-end workflow step records."""

    return [{"name": step, "passed": True} for step in args.workflow_steps]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free AI pre-screen rollout canary payload."""

    payload = common_payload(args)
    if args.kind == "runner":
        payload.update(
            {
                "source": "sorafs_cli",
                "runner_url": args.runner_url,
                "status_url": args.runner_status_url
                or f"{args.runner_url.rstrip('/')}/v1/sorafs/moderation/runner/status",
                "screen_url": args.runner_screen_url
                or f"{args.runner_url.rstrip('/')}/v1/sorafs/moderation/runner/screen",
                "manifest_id_hex": args.manifest_id_hex,
                "runner_hash_hex": args.runner_hash_hex,
                "subject": args.subject,
                "subject_digest_hex": args.subject_digest_hex,
                "screened_at_unix": args.screened_at_unix or args.generated_at_unix,
                "checked_at_unix": args.checked_at_unix or args.generated_at_unix,
                "combined_score_bps": args.score_bps,
                "verdict": args.verdict,
            }
        )
        if args.evidence_digest_hex is not None:
            payload["evidence_digest_hex"] = args.evidence_digest_hex
        if args.policy_digest_hex is not None:
            payload["policy_digest_hex"] = args.policy_digest_hex
    elif args.kind == "committee":
        payload.update(
            {
                "source": "sorafs_cli",
                "committee_url": args.committee_url,
                "status_url": args.committee_status_url
                or f"{args.committee_url.rstrip('/')}/v1/sorafs/moderation/committee/status",
                "aggregate_url": args.committee_aggregate_url
                or f"{args.committee_url.rstrip('/')}/v1/sorafs/moderation/committee/aggregate",
                "manifest_id_hex": args.manifest_id_hex,
                "runner_hash_hex": args.runner_hash_hex,
                "quorum": args.quorum,
                "aggregation": "median_score_bps",
                "result_count": args.result_count,
                "results": [{"name": name} for name in args.committee_results],
                "subject": args.subject,
                "subject_digest_hex": args.subject_digest_hex,
                "aggregated_score_bps": args.score_bps,
                "verdict": args.verdict,
                "checked_at_unix": args.checked_at_unix or args.generated_at_unix,
            }
        )
    elif args.kind == "operator_workflow":
        routes = build_operator_route_records(args)
        payload.update(
            {
                "source": "iroha_cli",
                "operator_url": args.operator_url,
                "workflow_digest_hex": args.workflow_digest_hex,
                "quarantine_id_hex": args.quarantine_id_hex,
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "payload_bytes_included": False,
                "private_payloads_included": False,
                "routes": routes,
            }
        )
    elif args.kind == "notification_transport":
        probes = build_notification_probes(args)
        payload.update(
            {
                "source": "juror-notifications",
                "manifest_path": args.manifest_path,
                "workflow_digest_hex": args.workflow_digest_hex,
                "manifest_body_blake3": args.body_digest_hex,
                "webhook_url": args.webhook_url,
                "probe_count": len(probes),
                "accepted_count": len(probes),
                "payload_bytes_included": False,
                "private_payloads_included": False,
                "probes": probes,
            }
        )
    elif args.kind == "commit_reveal_executor":
        artifacts = build_executor_artifacts(args)
        payload.update(
            {
                "source": "executor-bundle",
                "bundle_dir": args.bundle_dir,
                "workflow_digest_hex": args.workflow_digest_hex,
                "bundle_metadata_bytes": args.bundle_metadata_bytes,
                "bundle_metadata_blake3": args.body_digest_hex,
                "service_name": args.service_name,
                "interval_secs": args.interval_secs,
                "artifact_count": len(artifacts),
                "passed_artifact_count": len(artifacts),
                "execution_summary_present": True,
                "execution_summary_digest_hex": args.body_digest_hex,
                "execution_summary": {
                    "passed": True,
                    "path": args.execution_summary_path,
                    "bytes": args.execution_summary_bytes,
                    "body_blake3": args.body_digest_hex,
                    "action_count": args.action_count,
                    "commit_action_count": args.commit_action_count,
                    "reveal_action_count": args.reveal_action_count,
                    "tally_action_count": args.tally_action_count,
                    "payload_bytes_included": False,
                    "private_payloads_included": False,
                },
                "payload_bytes_included": False,
                "private_payloads_included": False,
                "private_payload_files_copied": False,
                "artifacts": artifacts,
            }
        )
    elif args.kind == "transparency_publication":
        probes = build_transparency_probes(args)
        payload.update(
            {
                "source": "iroha_cli",
                "workflow_digest_hex": args.workflow_digest_hex,
                "probe_count": len(probes),
                "passed_probe_count": len(probes),
                "source_entry_probe_count": len(probes),
                "payload_bytes_included": False,
                "private_payloads_included": False,
                "response_bodies_included": False,
                "probes": probes,
            }
        )
    elif args.kind == "governance_dag":
        producers = [{"name": name} for name in args.governance_producers]
        payload.update(
            {
                "source": "iroha_config",
                "workflow_digest_hex": args.workflow_digest_hex,
                "governance_dag_bound": True,
                "live_producers_bound": True,
                "transparency_source_entries_bound": True,
                "screening_ingest_bound": True,
                "quarantine_escalation_bound": True,
                "role_provisioning_recorded": True,
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
                "producer_count": len(producers),
                "edge_count": args.edge_count,
                "producers": producers,
                "edges": args.governance_edges,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            }
        )
    elif args.kind == "end_to_end_workflow":
        steps = build_workflow_steps(args)
        payload.update(
            {
                "source": "release-workflow",
                "workflow_id": args.workflow_id,
                "workflow_digest_hex": args.workflow_digest_hex,
                "deployed_services": True,
                "runner_committee_live": True,
                "ingest_quarantine_release_path_passed": True,
                "appeal_path_passed": True,
                "transparency_publication_passed": True,
                "role_gate_checks_passed": True,
                "encrypted_object_api_checks_passed": True,
                "step_count": len(steps),
                "passed_step_count": len(steps),
                "steps": steps,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            }
        )
    return payload


def validate_common_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate inputs shared by every generated canary."""

    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    validate_hex(args.body_digest_hex, length=HEX64_LEN, option="--body-digest-hex", errors=errors)
    if args.policy_digest_hex is not None:
        validate_hex(
            args.policy_digest_hex,
            length=HEX64_LEN,
            option="--policy-digest-hex",
            errors=errors,
        )
    require_2xx(args.route_status_code, option="--route-status-code", errors=errors)
    require_2xx(
        args.notification_status_code,
        option="--notification-status-code",
        errors=errors,
    )
    require_2xx(
        args.publication_status_code,
        option="--publication-status-code",
        errors=errors,
    )


def validate_runner_binding_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate manifest/hash/subject binding inputs."""

    require_kind_options(
        args,
        errors,
        (
            ("--manifest-id-hex", args.manifest_id_hex),
            ("--runner-hash-hex", args.runner_hash_hex),
            ("--subject", args.subject),
            ("--subject-digest-hex", args.subject_digest_hex),
        ),
    )
    validate_hex(
        args.manifest_id_hex,
        length=HEX32_LEN,
        option="--manifest-id-hex",
        errors=errors,
    )
    validate_hex(
        args.runner_hash_hex,
        length=HEX64_LEN,
        option="--runner-hash-hex",
        errors=errors,
    )
    validate_hex(
        args.subject_digest_hex,
        length=HEX64_LEN,
        option="--subject-digest-hex",
        errors=errors,
    )
    validate_subject_arg(args.subject, errors=errors)
    validate_verdict(args.verdict, option="--verdict", errors=errors)
    if args.evidence_digest_hex is not None:
        validate_hex(
            args.evidence_digest_hex,
            length=HEX64_LEN,
            option="--evidence-digest-hex",
            errors=errors,
        )


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    if args.kind == "runner":
        require_kind_options(args, errors, (("--runner-url", args.runner_url),))
        validate_canary_url(
            args.runner_url,
            label="--runner-url",
            errors=errors,
            required=True,
        )
        validate_canary_url(
            args.runner_status_url,
            label="--runner-status-url",
            errors=errors,
            required=False,
        )
        validate_canary_url(
            args.runner_screen_url,
            label="--runner-screen-url",
            errors=errors,
            required=False,
        )
        validate_runner_binding_inputs(args, errors)
    elif args.kind == "committee":
        require_kind_options(args, errors, (("--committee-url", args.committee_url),))
        validate_canary_url(
            args.committee_url,
            label="--committee-url",
            errors=errors,
            required=True,
        )
        validate_canary_url(
            args.committee_status_url,
            label="--committee-status-url",
            errors=errors,
            required=False,
        )
        validate_canary_url(
            args.committee_aggregate_url,
            label="--committee-aggregate-url",
            errors=errors,
            required=False,
        )
        validate_runner_binding_inputs(args, errors)
        if args.result_count < args.quorum:
            errors.append("--result-count must be >= --quorum")
        args.committee_results = validate_reviewed_inventory(
            split_csv_values(args.committee_result),
            expected_count=args.result_count,
            option="--committee-result",
            kind="committee",
            count_option="--result-count",
            errors=errors,
        )
    elif args.kind == "operator_workflow":
        require_kind_options(
            args,
            errors,
            (
                ("--operator-url", args.operator_url),
                ("--workflow-digest-hex", args.workflow_digest_hex),
                ("--quarantine-id-hex", args.quarantine_id_hex),
            ),
        )
        validate_canary_url(
            args.operator_url,
            label="--operator-url",
            errors=errors,
            required=True,
        )
        validate_hex(
            args.workflow_digest_hex,
            length=HEX64_LEN,
            option="--workflow-digest-hex",
            errors=errors,
        )
        validate_hex(
            args.quarantine_id_hex,
            length=HEX32_LEN,
            option="--quarantine-id-hex",
            errors=errors,
        )
        args.operator_routes = validate_name_set(
            split_csv_values(args.operator_route),
            allowed=REQUIRED_OPERATOR_ROUTES,
            option="--operator-route",
            errors=errors,
        )
    elif args.kind == "notification_transport":
        require_kind_options(
            args,
            errors,
            (
                ("--workflow-digest-hex", args.workflow_digest_hex),
                ("--webhook-url", args.webhook_url),
            ),
        )
        validate_hex(
            args.workflow_digest_hex,
            length=HEX64_LEN,
            option="--workflow-digest-hex",
            errors=errors,
        )
        validate_canary_url(
            args.webhook_url,
            label="--webhook-url",
            errors=errors,
            required=True,
        )
        validate_canary_path_label(
            args.manifest_path,
            label="--manifest-path",
            errors=errors,
        )
    elif args.kind == "commit_reveal_executor":
        require_kind_options(
            args,
            errors,
            (("--workflow-digest-hex", args.workflow_digest_hex),),
        )
        validate_hex(
            args.workflow_digest_hex,
            length=HEX64_LEN,
            option="--workflow-digest-hex",
            errors=errors,
        )
        validate_canary_path_label(
            args.execution_summary_path,
            label="--execution-summary-path",
            errors=errors,
        )
    elif args.kind == "transparency_publication":
        require_kind_options(
            args,
            errors,
            (("--workflow-digest-hex", args.workflow_digest_hex),),
        )
        validate_hex(
            args.workflow_digest_hex,
            length=HEX64_LEN,
            option="--workflow-digest-hex",
            errors=errors,
        )
        args.transparency_source_kinds = validate_name_set(
            split_csv_values(args.transparency_source_kind),
            allowed=REQUIRED_TRANSPARENCY_SOURCE_KINDS,
            option="--transparency-source-kind",
            errors=errors,
        )
    elif args.kind == "governance_dag":
        require_kind_options(
            args,
            errors,
            (
                ("--workflow-digest-hex", args.workflow_digest_hex),
                ("--policy-digest-hex", args.policy_digest_hex),
            ),
        )
        validate_hex(
            args.workflow_digest_hex,
            length=HEX64_LEN,
            option="--workflow-digest-hex",
            errors=errors,
        )
        args.governance_producers = validate_name_set(
            split_csv_values(args.governance_producer),
            allowed=REQUIRED_GOVERNANCE_PRODUCERS,
            option="--governance-producer",
            errors=errors,
        )
        if args.edge_count != REQUIRED_GOVERNANCE_EDGE_COUNT:
            errors.append("--edge-count must match required governance producer inventory")
        args.governance_edges = validate_governance_edges(
            split_csv_values(args.governance_edge),
            expected_count=args.edge_count,
            errors=errors,
        )
    elif args.kind == "end_to_end_workflow":
        require_kind_options(
            args,
            errors,
            (
                ("--workflow-digest-hex", args.workflow_digest_hex),
                ("--workflow-id", args.workflow_id),
            ),
        )
        validate_hex(
            args.workflow_digest_hex,
            length=HEX64_LEN,
            option="--workflow-digest-hex",
            errors=errors,
        )
        validate_workflow_id_arg(args.workflow_id, errors=errors)
        args.workflow_steps = validate_name_set(
            split_csv_values(args.workflow_step),
            allowed=REQUIRED_E2E_STEPS,
            option="--workflow-step",
            errors=errors,
        )


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_common_inputs(args, errors)
    validate_kind_inputs(args, errors)
    return errors


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the AI pre-screen gate contract."""

    kind, errors = validate_evidence_payload(payload)
    if kind != args.kind:
        errors.append(f"generated canary must validate as {args.kind}")
    return errors


def write_payload_atomic(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write the canary JSON atomically without following output symlinks."""

    text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    parent = path.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        del error
        return [f"--out parent `{path_diagnostic_label(parent)}` cannot be created"]
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
        del error
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
        return [f"--out `{path_diagnostic_label(path)}` cannot be written"]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SFM-4a AI pre-screen canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--body-digest-hex", required=True)
    parser.add_argument("--manifest-id-hex")
    parser.add_argument("--runner-hash-hex")
    parser.add_argument("--subject")
    parser.add_argument("--subject-digest-hex")
    parser.add_argument("--workflow-digest-hex")
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--evidence-digest-hex")
    parser.add_argument("--runner-url")
    parser.add_argument("--runner-status-url")
    parser.add_argument("--runner-screen-url")
    parser.add_argument("--committee-url")
    parser.add_argument("--committee-status-url")
    parser.add_argument("--committee-aggregate-url")
    parser.add_argument("--operator-url")
    parser.add_argument("--webhook-url")
    parser.add_argument("--workflow-id")
    parser.add_argument("--quarantine-id-hex")
    parser.add_argument("--screened-at-unix", type=positive_int_arg)
    parser.add_argument("--checked-at-unix", type=positive_int_arg)
    parser.add_argument("--score-bps", type=non_negative_int_arg, default=7250)
    parser.add_argument("--verdict", default="quarantine")
    parser.add_argument("--quorum", type=positive_int_arg, default=2)
    parser.add_argument("--result-count", type=positive_int_arg, default=3)
    parser.add_argument("--committee-result", action="append", default=[])
    parser.add_argument("--operator-route", action="append", default=[])
    parser.add_argument("--transparency-source-kind", action="append", default=[])
    parser.add_argument("--governance-producer", action="append", default=[])
    parser.add_argument("--governance-edge", action="append", default=[])
    parser.add_argument("--workflow-step", action="append", default=[])
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--notification-status-code", type=positive_int_arg, default=202)
    parser.add_argument("--publication-status-code", type=positive_int_arg, default=201)
    parser.add_argument("--probe-count", type=positive_int_arg, default=1)
    parser.add_argument("--body-bytes", type=positive_int_arg, default=128)
    parser.add_argument("--request-bytes", type=positive_int_arg, default=128)
    parser.add_argument("--response-bytes", type=positive_int_arg, default=16)
    parser.add_argument("--notification-bytes", type=positive_int_arg, default=256)
    parser.add_argument("--case-id", default="case-1")
    parser.add_argument("--round-id", default="round-1")
    parser.add_argument("--manifest-path", default="juror-notifications.json")
    parser.add_argument("--bundle-dir", default="/tmp/sorafs-ai-prescreen-executor")
    parser.add_argument("--bundle-metadata-bytes", type=positive_int_arg, default=128)
    parser.add_argument("--bundle-artifact-bytes", type=positive_int_arg, default=128)
    parser.add_argument(
        "--service-name",
        default="sorafs-moderation-ballots-executor",
    )
    parser.add_argument("--interval-secs", type=positive_int_arg, default=60)
    parser.add_argument("--execution-summary-path", default="execution.json")
    parser.add_argument("--execution-summary-bytes", type=positive_int_arg, default=512)
    parser.add_argument("--action-count", type=positive_int_arg, default=3)
    parser.add_argument("--commit-action-count", type=positive_int_arg, default=1)
    parser.add_argument("--reveal-action-count", type=positive_int_arg, default=1)
    parser.add_argument("--tally-action-count", type=positive_int_arg, default=1)
    parser.add_argument(
        "--edge-count",
        type=positive_int_arg,
        default=REQUIRED_GOVERNANCE_EDGE_COUNT,
    )
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
            "ERROR: SoraFS AI pre-screen canary inputs are incomplete:",
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
