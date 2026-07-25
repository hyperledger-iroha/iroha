#!/usr/bin/env python3
"""Validate aggregate SoraFS production-readiness gate summaries."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter
from dataclasses import dataclass
from html import unescape
from pathlib import Path
from typing import Any
from urllib.parse import unquote


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
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    evidence_path_identities,
    is_explicit_evidence_path,
)
from sorafs_evidence_sensitivity import (  # noqa: E402
    COMMON_SENSITIVE_KEYS,
    HIGH_RISK_SENSITIVE_KEY_FRAGMENTS,
    PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES,
    normalize_sensitive_key,
    visit_sensitive_fields,
)
from sorafs_evidence_validation import (  # noqa: E402
    evidence_schema_by_kind,
    evidence_gate_status,
    forbidden_non_production_markers,
    require_rollout_deployment_id,
)
from sorafs_path_identity import diagnostic_text_is_canonical, resolve_path_identity  # noqa: E402
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_gates,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)
from check_sorafs_ai_prescreen_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as AI_PRESCREEN_REQUIRED_KINDS,
    KIND_BY_NAME as AI_PRESCREEN_KIND_BY_NAME,
    POLICY_BOUND_KINDS as AI_PRESCREEN_POLICY_BOUND_KINDS,
    RUNNER_BOUND_KINDS as AI_PRESCREEN_RUNNER_BOUND_KINDS,
    WORKFLOW_BOUND_KINDS as AI_PRESCREEN_WORKFLOW_BOUND_KINDS,
)
from check_sorafs_appeal_finance_rollout_evidence import (  # noqa: E402
    CONFIG_BOUND_KINDS as APPEAL_FINANCE_CONFIG_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as APPEAL_FINANCE_REQUIRED_KINDS,
    KIND_BY_NAME as APPEAL_FINANCE_KIND_BY_NAME,
    POLICY_BOUND_KINDS as APPEAL_FINANCE_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as APPEAL_FINANCE_REQUIRED_METRICS,
)
from check_sorafs_gateway_compliance_rollout_evidence import (  # noqa: E402
    CATALOG_BOUND_KINDS as GATEWAY_COMPLIANCE_CATALOG_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as GATEWAY_COMPLIANCE_REQUIRED_KINDS,
    KIND_BY_NAME as GATEWAY_COMPLIANCE_KIND_BY_NAME,
    POLICY_BOUND_KINDS as GATEWAY_COMPLIANCE_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as GATEWAY_COMPLIANCE_REQUIRED_METRICS,
)
from check_sorafs_gateway_load_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as GATEWAY_LOAD_REQUIRED_KINDS,
    KIND_BY_NAME as GATEWAY_LOAD_KIND_BY_NAME,
    POLICY_BOUND_KINDS as GATEWAY_LOAD_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as GATEWAY_LOAD_REQUIRED_METRICS,
    STAGING_REPORT_BOUND_KINDS as GATEWAY_LOAD_STAGING_REPORT_BOUND_KINDS,
)
from check_sorafs_governance_dag_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as GOVERNANCE_DAG_REQUIRED_KINDS,
    KIND_BY_NAME as GOVERNANCE_DAG_KIND_BY_NAME,
    POLICY_BOUND_KINDS as GOVERNANCE_DAG_POLICY_BOUND_KINDS,
    PUBLIC_HEAD_BOUND_KINDS as GOVERNANCE_DAG_PUBLIC_HEAD_BOUND_KINDS,
    REQUIRED_METRICS as GOVERNANCE_DAG_REQUIRED_METRICS,
)
from check_sorafs_hedging_rollout_evidence import (  # noqa: E402
    CYCLE_ID_PATTERN as HEDGING_BILLING_CYCLE_ID_PATTERN,
    CYCLE_BOUND_KINDS as HEDGING_BILLING_CYCLE_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as HEDGING_BILLING_REQUIRED_KINDS,
    FORBIDDEN_CYCLE_ID_MARKERS as HEDGING_BILLING_FORBIDDEN_CYCLE_ID_MARKERS,
    KIND_BY_NAME as HEDGING_BILLING_KIND_BY_NAME,
    POLICY_BOUND_KINDS as HEDGING_BILLING_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as HEDGING_BILLING_REQUIRED_METRICS,
)
from check_sorafs_moderation_panel_rollout_evidence import (  # noqa: E402
    CASE_BOUND_KINDS as MODERATION_PANEL_CASE_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as MODERATION_PANEL_REQUIRED_KINDS,
    KIND_BY_NAME as MODERATION_PANEL_KIND_BY_NAME,
    POLICY_BOUND_KINDS as MODERATION_PANEL_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as MODERATION_PANEL_REQUIRED_METRICS,
    ROSTER_BOUND_KINDS as MODERATION_PANEL_ROSTER_BOUND_KINDS,
    TALLY_BOUND_KINDS as MODERATION_PANEL_TALLY_BOUND_KINDS,
)
from check_sorafs_orderbook_rollout_evidence import (  # noqa: E402
    CONTRACT_BOUND_KINDS as ORDERBOOK_CONTRACT_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as ORDERBOOK_REQUIRED_KINDS,
    KIND_BY_NAME as ORDERBOOK_KIND_BY_NAME,
    POLICY_BOUND_KINDS as ORDERBOOK_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as ORDERBOOK_REQUIRED_METRICS,
)
from check_sorafs_pdp_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as PDP_REQUIRED_KINDS,
    KIND_BY_NAME as PDP_KIND_BY_NAME,
    POLICY_BOUND_KINDS as PDP_POLICY_BOUND_KINDS,
    PROOF_SUMMARY_BOUND_KINDS as PDP_PROOF_SUMMARY_BOUND_KINDS,
    PROVIDER_ROSTER_BOUND_KINDS as PDP_PROVIDER_ROSTER_BOUND_KINDS,
    REQUIRED_METRICS as PDP_REQUIRED_METRICS,
)
from check_sorafs_pop_credentials_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as POP_CREDENTIALS_REQUIRED_KINDS,
    KIND_BY_NAME as POP_CREDENTIALS_KIND_BY_NAME,
    POLICY_BOUND_KINDS as POP_CREDENTIALS_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as POP_CREDENTIALS_REQUIRED_METRICS,
    REVOCATION_BOUND_KINDS as POP_CREDENTIALS_REVOCATION_BOUND_KINDS,
    ROOT_BOUND_KINDS as POP_CREDENTIALS_ROOT_BOUND_KINDS,
)
from check_sorafs_por_rollout_evidence import (  # noqa: E402
    ALLOWED_ARCHIVE_BACKENDS as POR_ALLOWED_ARCHIVE_BACKENDS,
    DEFAULT_REQUIRED_KINDS as POR_REQUIRED_KINDS,
    KIND_BY_NAME as POR_KIND_BY_NAME,
    POLICY_BOUND_KINDS as POR_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as POR_REQUIRED_METRICS,
    SEED_REPLAY_BOUND_KINDS as POR_SEED_REPLAY_BOUND_KINDS,
)
from check_sorafs_potr_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as POTR_REQUIRED_KINDS,
    KIND_BY_NAME as POTR_KIND_BY_NAME,
    PQ_KEY_ROSTER_BOUND_KINDS as POTR_PQ_KEY_ROSTER_BOUND_KINDS,
    RECEIPT_SUMMARY_BOUND_KINDS as POTR_RECEIPT_SUMMARY_BOUND_KINDS,
    REPUTATION_WEIGHT_BOUND_KINDS as POTR_REPUTATION_WEIGHT_BOUND_KINDS,
    REQUIRED_METRICS as POTR_REQUIRED_METRICS,
)
from check_sorafs_reference_sdk_release_evidence import (  # noqa: E402
    ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS as REFERENCE_SDK_ALLOWED_SIGNATURE_ALGORITHMS,
    DEFAULT_REQUIRED_KINDS as REFERENCE_SDK_REQUIRED_KINDS,
    KIND_BY_NAME as REFERENCE_SDK_KIND_BY_NAME,
    POLICY_BOUND_KINDS as REFERENCE_SDK_POLICY_BOUND_KINDS,
    RELEASE_MANIFEST_BOUND_KINDS as REFERENCE_SDK_RELEASE_MANIFEST_BOUND_KINDS,
)
from check_sorafs_repair_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as REPAIR_REQUIRED_KINDS,
    FAILURE_BOUND_KINDS as REPAIR_FAILURE_BOUND_KINDS,
    HANDOFF_BOUND_KINDS as REPAIR_HANDOFF_BOUND_KINDS,
    KIND_BY_NAME as REPAIR_KIND_BY_NAME,
    POLICY_BOUND_KINDS as REPAIR_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as REPAIR_REQUIRED_METRICS,
    ROSTER_BOUND_KINDS as REPAIR_ROSTER_BOUND_KINDS,
)
from check_sorafs_reputation_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as REPUTATION_REQUIRED_KINDS,
    FORBIDDEN_PROVIDER_ID_MARKERS as REPUTATION_FORBIDDEN_PROVIDER_ID_MARKERS,
    KIND_BY_NAME as REPUTATION_KIND_BY_NAME,
    PROVIDER_ID_PATTERN as REPUTATION_PROVIDER_ID_PATTERN,
    REQUIRED_METRICS as REPUTATION_REQUIRED_METRICS,
    SNAPSHOT_ANCHOR_KINDS as REPUTATION_SNAPSHOT_ANCHOR_KINDS,
    SNAPSHOT_BOUND_KINDS as REPUTATION_SNAPSHOT_BOUND_KINDS,
)
from check_sorafs_reserve_rent_rollout_evidence import (  # noqa: E402
    BAKE_ID_PATTERN as RESERVE_RENT_BAKE_ID_PATTERN,
    DEFAULT_REQUIRED_KINDS as RESERVE_RENT_REQUIRED_KINDS,
    FORBIDDEN_BAKE_ID_MARKERS as RESERVE_RENT_FORBIDDEN_BAKE_ID_MARKERS,
    KIND_BY_NAME as RESERVE_RENT_KIND_BY_NAME,
    LEDGER_BOUND_KINDS as RESERVE_RENT_LEDGER_BOUND_KINDS,
    MATRIX_BOUND_KINDS as RESERVE_RENT_MATRIX_BOUND_KINDS,
    POLICY_BOUND_KINDS as RESERVE_RENT_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as RESERVE_RENT_REQUIRED_METRICS,
)
from check_sorafs_transparency_rollout_evidence import (  # noqa: E402
    CYCLE_BOUND_KINDS as TRANSPARENCY_CYCLE_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as TRANSPARENCY_REQUIRED_KINDS,
    KIND_BY_NAME as TRANSPARENCY_KIND_BY_NAME,
    SOURCE_BOUND_KINDS as TRANSPARENCY_SOURCE_BOUND_KINDS,
)
from sccp_release_common import verify_ed25519  # noqa: E402


SUMMARY_SCHEMA = "sorafs.production_readiness.aggregate_gate.v1"
FOUNDATIONAL_PREREQUISITE_SCHEMA = (
    "sorafs.production_readiness.foundational_prerequisites.v1"
)
FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN = (
    b"iroha:sorafs:production-readiness:foundational-prerequisites:v1\x00"
)
FOUNDATIONAL_PREREQUISITE_IDS = (
    "SFM-1",
    "SF-1",
    "SF-2",
    "SF-2c",
    "SF-3",
    "SF-4",
    "SF-5b",
    "SF-6",
    "SF-8a",
)
MAX_FOUNDATIONAL_RELEASE_SEQUENCE = (1 << 63) - 1
FOUNDATIONAL_PREREQUISITE_FIELDS = frozenset(
    {
        "schema",
        "status",
        "deployment",
        "generated_at_unix",
        "release_sequence",
        "previous_envelope_sha256",
        "prerequisites",
        "lane_summaries",
        "signature",
    }
)
FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS = frozenset(
    {"deployment_id", "environment"}
)
FOUNDATIONAL_PREREQUISITE_ROW_FIELDS = frozenset(
    {
        "id",
        "status",
        "evidence_anchor_sha256",
        "evidence_generated_at_unix",
    }
)
FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS = frozenset({"gate", "sha256"})
FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS = frozenset(
    {"algorithm", "public_key_fingerprint_sha256", "signature_hex"}
)
AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS = frozenset(
    {
        "schema",
        "present",
        "valid",
        "required_ids",
        "prerequisite_count",
        "generated_at_unix",
        "oldest_evidence_generated_at_unix",
        "newest_evidence_generated_at_unix",
        "deployment_id",
        "environment",
        "release_sequence",
        "previous_envelope_sha256",
        "signer_public_key_fingerprint_sha256",
        "evidence_anchor_sha256",
        "lane_summary_sha256",
        "path",
        "sha256",
        "errors",
    }
)
AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS = frozenset(
    {"schema", "present", "valid", "errors"}
)
POP_CREDENTIALS_ROOT_BOUND_FINGERPRINT_FIELDS = tuple(
    (
        kind_name,
        "synced_root_digest_hex" if kind_name == "juror_client" else "root_digest_hex",
    )
    for kind_name in POP_CREDENTIALS_ROOT_BOUND_KINDS
)
POP_CREDENTIALS_REVOCATION_BOUND_FINGERPRINT_FIELDS = tuple(
    (
        kind_name,
        (
            "synced_revocation_list_digest_hex"
            if kind_name == "juror_client"
            else "revocation_list_digest_hex"
        ),
    )
    for kind_name in POP_CREDENTIALS_REVOCATION_BOUND_KINDS
)
MAX_SUMMARY_BYTES = 4 * 1024 * 1024
DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS = 14 * 24 * 60 * 60
LOWER_HEX_DIGITS = set("0123456789abcdef")
PRODUCTION_READY_ENVIRONMENTS = frozenset({"prod", "production"})
FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS = frozenset({"stage", "staging"})
SUCCESS_ARTIFACT_STATUSES = frozenset({"passed", "verified"})

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "evidence_json",
    "manifest_signing_key",
    "mnemonic",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_archive",
    "raw_body",
    "raw_evidence",
    "raw",
    "raw_payload",
    "raw_request",
    "raw_response",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_transaction",
    "token",
}
PATH_SENSITIVE_KEY_FRAGMENTS = HIGH_RISK_SENSITIVE_KEY_FRAGMENTS - frozenset(
    {"requestbody", "responsebody"}
)

GATE_METADATA_FIELDS: dict[str, frozenset[str]] = {
    "ai_prescreen": frozenset(
        {
            "valid_executor_summary_digests",
            "deployment_context",
            "valid_notification_manifest_digests",
            "valid_policy_digests",
            "valid_runner_bindings",
            "valid_workflow_digests",
        }
    ),
    "appeal_finance": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_config_digests",
            "valid_multi_peer_runs",
            "valid_policy_digests",
        }
    ),
    "gateway_compliance": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_catalog_digests",
            "valid_policy_digests",
        }
    ),
    "gateway_load": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_policy_digests",
            "valid_staging_report_digests",
            "valid_suite_report_digests",
        }
    ),
    "governance_dag": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_checkpoint_digests",
            "valid_policy_digests",
            "valid_public_head_cids",
        }
    ),
    "hedging_billing": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_billing_cycles",
            "valid_cycle_bindings",
            "valid_policy_digests",
            "valid_reference_decision_ids",
        }
    ),
    "moderation_panel": frozenset(
        {
            "deployment_context",
            "metric_count_values",
            "metrics",
            "valid_case_digests",
            "valid_e2e_runs",
            "valid_evidence_viewer_digest_sets",
            "valid_policy_digests",
            "valid_roster_bindings",
            "valid_tally_bindings",
        }
    ),
    "orderbook": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_contract_digests",
            "valid_policy_digests",
        }
    ),
    "pdp": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_policy_digests",
            "valid_proof_summary_digests",
            "valid_provider_roster_digests",
            "valid_repair_handoff_digests",
        }
    ),
    "pop_credentials": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_juror_sync_bindings",
            "valid_policy_digests",
            "valid_pop_snapshot_digests",
            "valid_revocation_list_digests",
            "valid_root_digests",
        }
    ),
    "por": frozenset(
        {
            "archive_backends",
            "metric_count_values",
            "metrics",
            "valid_governance_archive_handoff_digests",
            "valid_policy_digests",
            "valid_seed_replay_digests",
        }
    ),
    "potr": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_policy_digests",
            "valid_pq_key_roster_digests",
            "valid_receipt_summary_digests",
            "valid_reputation_weight_policy_digests",
        }
    ),
    "reference_sdk_release": frozenset(
        {
            "signature_algorithms",
            "valid_archive_index_digests",
            "valid_ffi_contract_digests",
            "valid_header_digests",
            "valid_package_index_digests",
            "valid_policy_digests",
            "valid_release_key_fingerprints",
            "valid_release_manifest_digests",
            "valid_release_manifest_reference_digests",
            "valid_smoke_output_digests",
        }
    ),
    "repair": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_failure_bundle_digests",
            "valid_handoff_digests",
            "valid_policy_digests",
            "valid_roster_digests",
        }
    ),
    "reputation": frozenset(
        {
            "metric_count_values",
            "metrics",
            "merkle_root_hex",
            "provider_count_values",
            "provider_ids",
            "snapshot_id_hex",
            "valid_reputation_weight_digests",
            "valid_snapshot_bindings",
        }
    ),
    "reserve_rent": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_policy_digests",
            "valid_policy_matrix_bindings",
            "valid_policy_matrix_ledger_bindings",
            "valid_provider_bakes",
        }
    ),
    "transparency": frozenset(
        {
            "valid_cycle_digests",
            "valid_publication_bindings",
            "valid_source_batch_digests",
        }
    ),
}
PAYLOAD_FREE_SUMMARY_CORE_FIELDS = frozenset(
    {
        "schema",
        "status",
        "required_kinds",
        "thresholds",
        "evidence_file_count",
        "recognized_artifact_count",
        "recognized_artifacts",
        "required",
        "errors",
        "load_errors",
    }
)
PAYLOAD_FREE_SUMMARY_METADATA_FIELDS = frozenset().union(
    *GATE_METADATA_FIELDS.values()
)
PAYLOAD_FREE_SUMMARY_FIELDS = (
    PAYLOAD_FREE_SUMMARY_CORE_FIELDS | PAYLOAD_FREE_SUMMARY_METADATA_FIELDS
)
PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS = frozenset(
    field
    for field in PAYLOAD_FREE_SUMMARY_METADATA_FIELDS
    if field.startswith("valid_")
) | frozenset(
    {
        "archive_backends",
        "metric_count_values",
        "metrics",
        "provider_count_values",
        "provider_ids",
        "signature_algorithms",
    }
)
PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS = frozenset(
    {
        "valid_catalog_digests",
        "valid_case_digests",
        "valid_checkpoint_digests",
        "valid_config_digests",
        "valid_contract_digests",
        "valid_cycle_digests",
        "valid_executor_summary_digests",
        "valid_failure_bundle_digests",
        "valid_governance_archive_handoff_digests",
        "valid_handoff_digests",
        "valid_notification_manifest_digests",
        "valid_policy_digests",
        "valid_pop_snapshot_digests",
        "valid_pq_key_roster_digests",
        "valid_proof_summary_digests",
        "valid_provider_roster_digests",
        "valid_public_head_cids",
        "valid_receipt_summary_digests",
        "valid_reputation_weight_digests",
        "valid_reputation_weight_policy_digests",
        "valid_repair_handoff_digests",
        "valid_reference_decision_ids",
        "valid_archive_index_digests",
        "valid_ffi_contract_digests",
        "valid_header_digests",
        "valid_package_index_digests",
        "valid_release_key_fingerprints",
        "valid_release_manifest_digests",
        "valid_release_manifest_reference_digests",
        "valid_smoke_output_digests",
        "valid_revocation_list_digests",
        "valid_root_digests",
        "valid_roster_digests",
        "valid_seed_replay_digests",
        "valid_source_batch_digests",
        "valid_staging_report_digests",
        "valid_suite_report_digests",
        "valid_workflow_digests",
    }
)
PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS = {
    "valid_cycle_bindings": {
        "statement_bundle_digest_hex": 64,
        "reconciliation_digest_hex": 64,
    },
    "valid_juror_sync_bindings": {
        "synced_root_digest_hex": 64,
        "synced_revocation_list_digest_hex": 64,
    },
    "valid_policy_matrix_bindings": {
        "policy_digest_hex": 64,
        "matrix_digest_hex": 64,
    },
    "valid_policy_matrix_ledger_bindings": {
        "policy_digest_hex": 64,
        "matrix_digest_hex": 64,
        "ledger_digest_hex": 64,
    },
    "valid_publication_bindings": {
        "source_batch_digest_hex": 64,
        "cycle_digest_hex": 64,
    },
    "valid_roster_bindings": {
        "case_digest_hex": 64,
        "roster_hash_hex": 64,
    },
    "valid_runner_bindings": {
        "manifest_id_hex": 32,
        "runner_hash_hex": 64,
        "subject_digest_hex": 64,
    },
    "valid_snapshot_bindings": {
        "snapshot_id_hex": 32,
        "merkle_root_hex": 64,
    },
    "valid_tally_bindings": {
        "case_digest_hex": 64,
        "roster_hash_hex": 64,
        "tally_digest_hex": 64,
    },
}
PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS = frozenset(
    {"metric_count_values", "provider_count_values"}
)
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS: dict[str, dict[str, Any]] = {
    "valid_billing_cycles": {
        "strings": frozenset({"cycle_id", "deployment_id", "environment"}),
        "positive_ints": frozenset(
            {"cycle_index", "generated_at_unix", "statement_count"}
        ),
        "hex": {
            "policy_digest_hex": 64,
            "reference_decision_id_hex": 64,
            "statement_bundle_digest_hex": 64,
            "reconciliation_digest_hex": 64,
        },
    },
    "valid_e2e_runs": {
        "strings": frozenset({"deployment_id", "environment"}),
        "positive_ints": frozenset(
            {"case_count", "generated_at_unix", "peer_count", "validator_count"}
        ),
        "hex": {
            "case_digest_hex": 64,
            "roster_hash_hex": 64,
            "tally_digest_hex": 64,
        },
    },
    "valid_evidence_viewer_digest_sets": {
        "strings": frozenset(),
        "positive_ints": frozenset(),
        "hex": {
            "case_digest_hex": 64,
            "roster_hash_hex": 64,
            "session_manifest_digest_hex": 64,
            "watermark_metadata_digest_hex": 64,
            "access_log_digest_hex": 64,
            "legal_hold_receipt_digest_hex": 64,
            "transparency_report_digest_hex": 64,
            "audit_digest_hex": 64,
        },
    },
    "valid_multi_peer_runs": {
        "strings": frozenset({"deployment_id", "environment"}),
        "positive_ints": frozenset(
            {"case_count", "generated_at_unix", "peer_count", "validator_count"}
        ),
        "hex": {"config_digest_hex": 64},
    },
    "valid_provider_bakes": {
        "strings": frozenset({"bake_id", "deployment_id", "environment"}),
        "positive_ints": frozenset(
            {
                "completed_at_unix",
                "provider_count",
                "scheduled_lifecycle_canary_defaulted_provider_count",
                "scheduled_lifecycle_canary_last_tick_at_unix",
                "scheduled_lifecycle_canary_tick_count",
                "started_at_unix",
            }
        ),
        "hex": {
            "ledger_digest_hex": 64,
            "matrix_digest_hex": 64,
            "policy_digest_hex": 64,
        },
        "ordered_int_pairs": (
            ("started_at_unix", "completed_at_unix"),
            ("scheduled_lifecycle_canary_last_tick_at_unix", "completed_at_unix"),
        ),
    },
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS = {
    "valid_billing_cycles": ("cycle_id",),
    "valid_e2e_runs": ("case_digest_hex", "roster_hash_hex", "tally_digest_hex"),
    "valid_evidence_viewer_digest_sets": ("case_digest_hex", "roster_hash_hex"),
    "valid_multi_peer_runs": ("deployment_id", "environment", "generated_at_unix"),
    "valid_provider_bakes": ("bake_id",),
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS = {
    "valid_billing_cycles": "billing_cycle",
    "valid_e2e_runs": "e2e_panel",
    "valid_evidence_viewer_digest_sets": "evidence_viewer",
    "valid_multi_peer_runs": "multi_peer_reconciliation",
    "valid_provider_bakes": "provider_bake",
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS = {
    ("appeal_finance", "valid_multi_peer_runs"): "multi_peer_reconciliation",
    ("hedging_billing", "valid_billing_cycles"): "billing_cycle",
    ("moderation_panel", "valid_e2e_runs"): "e2e_panel",
    ("moderation_panel", "valid_evidence_viewer_digest_sets"): "evidence_viewer",
    ("reserve_rent", "valid_provider_bakes"): "provider_bake",
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_STRING_FIELD_POLICIES = {
    ("hedging_billing", "valid_billing_cycles", "cycle_id"): {
        "forbidden_markers": HEDGING_BILLING_FORBIDDEN_CYCLE_ID_MARKERS,
        "pattern": HEDGING_BILLING_CYCLE_ID_PATTERN,
        "pattern_error": "must match canonical lowercase `billing-cycle-*`",
    },
    ("reserve_rent", "valid_provider_bakes", "bake_id"): {
        "forbidden_markers": RESERVE_RENT_FORBIDDEN_BAKE_ID_MARKERS,
        "pattern": RESERVE_RENT_BAKE_ID_PATTERN,
        "pattern_error": "must match canonical lowercase `reserve-bake-*`",
    },
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_FINGERPRINT_HEX_BINDINGS = {
    field: {
        metadata_field: metadata_field
        for metadata_field in schema.get("hex", {})
    }
    for field, schema in PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.items()
}
PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS = frozenset(
    {"merkle_root_hex", "snapshot_id_hex"}
)
PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS = {
    "merkle_root_hex": 64,
    "snapshot_id_hex": 32,
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS = {
    "valid_catalog_digests": "catalog_digest_hex",
    "valid_case_digests": "case_digest_hex",
    "valid_checkpoint_digests": "checkpoint_digest_hex",
    "valid_config_digests": "config_digest_hex",
    "valid_contract_digests": "contract_digest_hex",
    "valid_cycle_digests": "cycle_digest_hex",
    "valid_executor_summary_digests": "execution_summary_digest_hex",
    "valid_failure_bundle_digests": "evidence_bundle_digest_hex",
    "valid_governance_archive_handoff_digests": (
        "governance_archive_handoff_digest_hex"
    ),
    "valid_handoff_digests": "handoff_digest_hex",
    "valid_notification_manifest_digests": "manifest_body_blake3_hex",
    "valid_policy_digests": "policy_digest_hex",
    "valid_pop_snapshot_digests": "pop_snapshot_digest_hex",
    "valid_pq_key_roster_digests": "pq_key_roster_digest_hex",
    "valid_proof_summary_digests": "proof_summary_digest_hex",
    "valid_provider_roster_digests": "provider_roster_digest_hex",
    "valid_public_head_cids": "public_head_cid_hex",
    "valid_receipt_summary_digests": "receipt_summary_digest_hex",
    "valid_reputation_weight_digests": "weights_digest_hex",
    "valid_reputation_weight_policy_digests": "reputation_weight_policy_digest_hex",
    "valid_repair_handoff_digests": "repair_handoff_digest_hex",
    "valid_reference_decision_ids": "decision_id_hex",
    "valid_archive_index_digests": "archive_index_digest_hex",
    "valid_ffi_contract_digests": "ffi_contract_digest_hex",
    "valid_header_digests": "header_digest_hex",
    "valid_package_index_digests": "package_index_digest_hex",
    "valid_release_key_fingerprints": "public_key_fingerprint_hex",
    "valid_release_manifest_digests": "manifest_digest_hex",
    "valid_release_manifest_reference_digests": "release_manifest_digest_hex",
    "valid_smoke_output_digests": "smoke_output_digest_hex",
    "valid_revocation_list_digests": "revocation_list_digest_hex",
    "valid_root_digests": "root_digest_hex",
    "valid_roster_digests": "roster_digest_hex",
    "valid_seed_replay_digests": "seed_replay_digest_hex",
    "valid_source_batch_digests": "source_batch_digest_hex",
    "valid_staging_report_digests": "staging_report_digest_hex",
    "valid_suite_report_digests": "suite_report_digest_hex",
    "valid_workflow_digests": "workflow_digest_hex",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS = {
    ("ai_prescreen", "valid_executor_summary_digests"): ("commit_reveal_executor",),
    ("ai_prescreen", "valid_notification_manifest_digests"): (
        "notification_transport",
    ),
    ("ai_prescreen", "valid_policy_digests"): ("runner",),
    ("ai_prescreen", "valid_workflow_digests"): ("end_to_end_workflow",),
    ("appeal_finance", "valid_config_digests"): ("pricing_config",),
    ("appeal_finance", "valid_policy_digests"): ("pricing_config",),
    ("gateway_compliance", "valid_catalog_digests"): ("catalog_promotion",),
    ("gateway_compliance", "valid_policy_digests"): ("catalog_promotion",),
    ("gateway_load", "valid_policy_digests"): ("staging_load",),
    ("gateway_load", "valid_staging_report_digests"): ("staging_load",),
    ("gateway_load", "valid_suite_report_digests"): ("local_conformance",),
    ("governance_dag", "valid_checkpoint_digests"): ("operator_recovery",),
    ("governance_dag", "valid_policy_digests"): ("publisher_service",),
    ("governance_dag", "valid_public_head_cids"): ("publisher_service",),
    ("hedging_billing", "valid_policy_digests"): ("billing_cycle",),
    ("hedging_billing", "valid_reference_decision_ids"): ("reference_price",),
    ("moderation_panel", "valid_case_digests"): ("appeal_intake",),
    ("moderation_panel", "valid_policy_digests"): ("e2e_panel",),
    ("orderbook", "valid_contract_digests"): ("contract_surface",),
    ("orderbook", "valid_policy_digests"): ("contract_surface",),
    ("pdp", "valid_policy_digests"): ("proof_generation",),
    ("pdp", "valid_proof_summary_digests"): ("proof_generation",),
    ("pdp", "valid_provider_roster_digests"): ("proof_generation",),
    ("pdp", "valid_repair_handoff_digests"): ("governance_repair",),
    ("pop_credentials", "valid_policy_digests"): ("verifier_service",),
    ("pop_credentials", "valid_pop_snapshot_digests"): ("moderation_integration",),
    ("pop_credentials", "valid_revocation_list_digests"): (
        "issuer_bundle",
        "revocation_registry",
    ),
    ("pop_credentials", "valid_root_digests"): ("issuer_bundle", "commitment_root"),
    ("por", "valid_governance_archive_handoff_digests"): ("reporting_archive",),
    ("por", "valid_policy_digests"): ("randomness",),
    ("por", "valid_seed_replay_digests"): ("randomness",),
    ("potr", "valid_policy_digests"): ("governance_approval",),
    ("potr", "valid_pq_key_roster_digests"): ("governance_approval",),
    ("potr", "valid_receipt_summary_digests"): ("multi_provider_probe",),
    ("potr", "valid_reputation_weight_policy_digests"): ("governance_approval",),
    ("reference_sdk_release", "valid_policy_digests"): ("signed_manifest",),
    ("reference_sdk_release", "valid_archive_index_digests"): ("release_archive",),
    ("reference_sdk_release", "valid_ffi_contract_digests"): (
        "ffi_header_contract",
    ),
    ("reference_sdk_release", "valid_header_digests"): ("ffi_header_contract",),
    ("reference_sdk_release", "valid_package_index_digests"): (
        "downstream_bindings",
    ),
    ("reference_sdk_release", "valid_release_key_fingerprints"): (
        "signed_manifest",
    ),
    ("reference_sdk_release", "valid_release_manifest_digests"): (
        "signed_manifest",
    ),
    ("reference_sdk_release", "valid_release_manifest_reference_digests"): (
        "release_archive",
        "downstream_bindings",
        "cookbook_smoke",
        "ffi_header_contract",
        "governance_approval",
    ),
    ("reference_sdk_release", "valid_smoke_output_digests"): ("cookbook_smoke",),
    ("repair", "valid_failure_bundle_digests"): ("failure_capture",),
    ("repair", "valid_handoff_digests"): ("governance_handoff",),
    ("repair", "valid_policy_digests"): ("governance_handoff",),
    ("repair", "valid_roster_digests"): ("auditor_roster",),
    ("reputation", "valid_reputation_weight_digests"): ("publish", "latest"),
    ("reserve_rent", "valid_policy_digests"): ("policy_config",),
    ("transparency", "valid_cycle_digests"): ("publication",),
    ("transparency", "valid_source_batch_digests"): ("source_entry",),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS = {
    "merkle_root_hex": "merkle_root_hex",
    "snapshot_id_hex": "snapshot_id_hex",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS = {
    ("reputation", "merkle_root_hex"): ("publish", "latest"),
    ("reputation", "snapshot_id_hex"): ("publish", "latest"),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS = {
    "archive_backends": "archive_backend",
    "provider_ids": "provider_id",
    "signature_algorithms": "signature_algorithm",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS = {
    ("por", "archive_backends"): ("reporting_archive",),
    ("reference_sdk_release", "signature_algorithms"): ("signed_manifest",),
    ("reputation", "provider_ids"): ("provider",),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS = {
    "metrics": "metrics",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS = {
    ("appeal_finance", "metrics"): ("dashboard_metrics",),
    ("gateway_compliance", "metrics"): ("observability",),
    ("gateway_load", "metrics"): ("telemetry_slo",),
    ("governance_dag", "metrics"): ("observability",),
    ("hedging_billing", "metrics"): ("metrics_alerts",),
    ("moderation_panel", "metrics"): ("metrics_alerts",),
    ("orderbook", "metrics"): ("observability",),
    ("pdp", "metrics"): ("observability",),
    ("pop_credentials", "metrics"): ("metrics_alerts",),
    ("por", "metrics"): ("observability",),
    ("potr", "metrics"): ("observability",),
    ("repair", "metrics"): ("observability",),
    ("reputation", "metrics"): ("metrics",),
    ("reserve_rent", "metrics"): ("metrics_alerts",),
}
PAYLOAD_FREE_SUMMARY_ALLOWED_STRING_LIST_VALUES = {
    ("por", "archive_backends"): POR_ALLOWED_ARCHIVE_BACKENDS,
    (
        "reference_sdk_release",
        "signature_algorithms",
    ): REFERENCE_SDK_ALLOWED_SIGNATURE_ALGORITHMS,
}
PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES = {
    ("appeal_finance", "metrics"): APPEAL_FINANCE_REQUIRED_METRICS,
    ("gateway_compliance", "metrics"): GATEWAY_COMPLIANCE_REQUIRED_METRICS,
    ("gateway_load", "metrics"): GATEWAY_LOAD_REQUIRED_METRICS,
    ("governance_dag", "metrics"): GOVERNANCE_DAG_REQUIRED_METRICS,
    ("hedging_billing", "metrics"): HEDGING_BILLING_REQUIRED_METRICS,
    ("moderation_panel", "metrics"): MODERATION_PANEL_REQUIRED_METRICS,
    ("orderbook", "metrics"): ORDERBOOK_REQUIRED_METRICS,
    ("pdp", "metrics"): PDP_REQUIRED_METRICS,
    ("pop_credentials", "metrics"): POP_CREDENTIALS_REQUIRED_METRICS,
    ("por", "metrics"): POR_REQUIRED_METRICS,
    ("potr", "metrics"): POTR_REQUIRED_METRICS,
    ("repair", "metrics"): REPAIR_REQUIRED_METRICS,
    ("reputation", "metrics"): REPUTATION_REQUIRED_METRICS,
    ("reserve_rent", "metrics"): RESERVE_RENT_REQUIRED_METRICS,
}
PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS = {
    ("appeal_finance", "metrics"): "metric_count_values",
    ("gateway_compliance", "metrics"): "metric_count_values",
    ("gateway_load", "metrics"): "metric_count_values",
    ("governance_dag", "metrics"): "metric_count_values",
    ("hedging_billing", "metrics"): "metric_count_values",
    ("moderation_panel", "metrics"): "metric_count_values",
    ("orderbook", "metrics"): "metric_count_values",
    ("pdp", "metrics"): "metric_count_values",
    ("pop_credentials", "metrics"): "metric_count_values",
    ("por", "metrics"): "metric_count_values",
    ("potr", "metrics"): "metric_count_values",
    ("repair", "metrics"): "metric_count_values",
    ("reputation", "metrics"): "metric_count_values",
    ("reserve_rent", "metrics"): "metric_count_values",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS = {
    "metric_count_values": "metric_count",
    "provider_count_values": "provider_count",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS = {
    ("appeal_finance", "metric_count_values"): ("dashboard_metrics",),
    ("gateway_compliance", "metric_count_values"): ("observability",),
    ("gateway_load", "metric_count_values"): ("telemetry_slo",),
    ("governance_dag", "metric_count_values"): ("observability",),
    ("hedging_billing", "metric_count_values"): ("metrics_alerts",),
    ("moderation_panel", "metric_count_values"): ("metrics_alerts",),
    ("orderbook", "metric_count_values"): ("observability",),
    ("pdp", "metric_count_values"): ("observability",),
    ("pop_credentials", "metric_count_values"): ("metrics_alerts",),
    ("por", "metric_count_values"): ("observability",),
    ("potr", "metric_count_values"): ("observability",),
    ("repair", "metric_count_values"): ("observability",),
    ("reputation", "metric_count_values"): ("metrics",),
    ("reputation", "provider_count_values"): ("publish", "latest"),
    ("reserve_rent", "metric_count_values"): ("metrics_alerts",),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_FIELDS = {
    "valid_cycle_bindings": (
        "statement_bundle_digest_hex",
        "reconciliation_digest_hex",
    ),
    "valid_juror_sync_bindings": (
        "synced_root_digest_hex",
        "synced_revocation_list_digest_hex",
    ),
    "valid_policy_matrix_bindings": (
        "policy_digest_hex",
        "matrix_digest_hex",
    ),
    "valid_policy_matrix_ledger_bindings": (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
    ),
    "valid_publication_bindings": (
        "source_batch_digest_hex",
        "cycle_digest_hex",
    ),
    "valid_roster_bindings": (
        "case_digest_hex",
        "roster_hash_hex",
    ),
    "valid_runner_bindings": (
        "manifest_id_hex",
        "runner_hash_hex",
        "subject_digest_hex",
    ),
    "valid_snapshot_bindings": (
        "snapshot_id_hex",
        "merkle_root_hex",
    ),
    "valid_tally_bindings": (
        "case_digest_hex",
        "roster_hash_hex",
        "tally_digest_hex",
    ),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS = {
    ("ai_prescreen", "valid_runner_bindings"): ("runner",),
    ("hedging_billing", "valid_cycle_bindings"): ("billing_cycle",),
    ("moderation_panel", "valid_roster_bindings"): ("sortition_roster",),
    ("moderation_panel", "valid_tally_bindings"): ("commit_reveal",),
    ("pop_credentials", "valid_juror_sync_bindings"): ("juror_client",),
    ("reputation", "valid_snapshot_bindings"): ("publish", "latest"),
    ("reserve_rent", "valid_policy_matrix_bindings"): ("quote_matrix",),
    ("reserve_rent", "valid_policy_matrix_ledger_bindings"): ("ledger_digest",),
    ("transparency", "valid_publication_bindings"): ("publication",),
}
PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS = {
    "deployment_context": frozenset({"deployment_id", "environment"}),
}
PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS = (
    PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS
    | frozenset(PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS)
    | PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS
    | frozenset(
        {"archive_backends", "metrics", "provider_ids", "signature_algorithms"}
    )
)
MAX_SUMMARY_METADATA_DEPTH = 32
PAYLOAD_FREE_ARTIFACT_FIELDS = frozenset(
    {
        "kind",
        "path",
        "sha256",
        "schema",
        "status",
        "fingerprint",
        "valid",
        "errors",
    }
)
PAYLOAD_FREE_REQUIRED_ROW_FIELDS = frozenset(
    {
        "schema",
        "present",
        "valid",
        "artifact_count",
        "artifacts",
        "errors",
    }
)
AGGREGATE_REQUIRED_GATE_ROW_FIELDS = frozenset(
    {
        "schema",
        "present",
        "valid",
        "required_kind_count",
        "expected_required_kind_count",
        "evidence_file_count",
        "recognized_artifact_count",
        "artifact_count",
        "thresholds",
        "oldest_generated_at_unix",
        "newest_generated_at_unix",
        "deployment_id",
        "environment",
        "expected_required_kinds",
        "errors",
        "path",
        "sha256",
    }
)
AGGREGATE_MISSING_GATE_ROW_FIELDS = frozenset(
    {
        "schema",
        "present",
        "valid",
        "errors",
    }
)
AGGREGATE_SUMMARY_FIELDS = frozenset(
    {
        "schema",
        "status",
        "required_gates",
        "thresholds",
        "summary_file_count",
        "recognized_summary_count",
        "deployment",
        "foundational_prerequisites",
        "required",
        "errors",
    }
)


@dataclass(frozen=True)
class GateSummaryKind:
    """One SoraFS production-readiness lane summary."""

    name: str
    schema: str
    required_kinds: tuple[str, ...]


GATE_SUMMARY_KINDS: tuple[GateSummaryKind, ...] = (
    GateSummaryKind(
        "ai_prescreen",
        "sorafs.moderation.ai_prescreen.rollout_evidence_gate.v1",
        AI_PRESCREEN_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "appeal_finance",
        "sorafs.appeal_finance.rollout_evidence_gate.v1",
        APPEAL_FINANCE_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "gateway_compliance",
        "sorafs.gateway_compliance.rollout_evidence_gate.v1",
        GATEWAY_COMPLIANCE_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "gateway_load",
        "sorafs.gateway_load.rollout_evidence_gate.v1",
        GATEWAY_LOAD_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "governance_dag",
        "sorafs.governance_dag.rollout_evidence_gate.v1",
        GOVERNANCE_DAG_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "hedging_billing",
        "sorafs.hedging_billing.rollout_evidence_gate.v1",
        HEDGING_BILLING_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "moderation_panel",
        "sorafs.moderation_panel.rollout_evidence_gate.v1",
        MODERATION_PANEL_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "orderbook",
        "sorafs.orderbook.rollout_evidence_gate.v1",
        ORDERBOOK_REQUIRED_KINDS,
    ),
    GateSummaryKind("pdp", "sorafs.pdp.rollout_evidence_gate.v1", PDP_REQUIRED_KINDS),
    GateSummaryKind(
        "pop_credentials",
        "sorafs.pop_credentials.rollout_evidence_gate.v1",
        POP_CREDENTIALS_REQUIRED_KINDS,
    ),
    GateSummaryKind("por", "sorafs.por.rollout_evidence_gate.v1", POR_REQUIRED_KINDS),
    GateSummaryKind(
        "potr",
        "sorafs.potr.rollout_evidence_gate.v1",
        POTR_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "reference_sdk_release",
        "sorafs.reference_sdk.release_evidence_gate.v1",
        REFERENCE_SDK_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "repair",
        "sorafs.repair.rollout_evidence_gate.v1",
        REPAIR_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "reputation",
        "sorafs.reputation.rollout_evidence_gate.v1",
        REPUTATION_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "reserve_rent",
        "sorafs.reserve_rent.rollout_evidence_gate.v1",
        RESERVE_RENT_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "transparency",
        "sorafs.transparency.rollout_evidence_gate.v1",
        TRANSPARENCY_REQUIRED_KINDS,
    ),
)

SCHEMA_TO_GATE = {kind.schema: kind for kind in GATE_SUMMARY_KINDS}
GATE_BY_NAME = {kind.name: kind for kind in GATE_SUMMARY_KINDS}
DEFAULT_REQUIRED_GATES = tuple(kind.name for kind in GATE_SUMMARY_KINDS)
GATE_REQUIRED_KIND_SCHEMAS = {
    "ai_prescreen": evidence_schema_by_kind(AI_PRESCREEN_KIND_BY_NAME),
    "appeal_finance": evidence_schema_by_kind(APPEAL_FINANCE_KIND_BY_NAME),
    "gateway_compliance": evidence_schema_by_kind(GATEWAY_COMPLIANCE_KIND_BY_NAME),
    "gateway_load": evidence_schema_by_kind(GATEWAY_LOAD_KIND_BY_NAME),
    "governance_dag": evidence_schema_by_kind(GOVERNANCE_DAG_KIND_BY_NAME),
    "hedging_billing": evidence_schema_by_kind(HEDGING_BILLING_KIND_BY_NAME),
    "moderation_panel": evidence_schema_by_kind(MODERATION_PANEL_KIND_BY_NAME),
    "orderbook": evidence_schema_by_kind(ORDERBOOK_KIND_BY_NAME),
    "pdp": evidence_schema_by_kind(PDP_KIND_BY_NAME),
    "pop_credentials": evidence_schema_by_kind(POP_CREDENTIALS_KIND_BY_NAME),
    "por": evidence_schema_by_kind(POR_KIND_BY_NAME),
    "potr": evidence_schema_by_kind(POTR_KIND_BY_NAME),
    "reference_sdk_release": evidence_schema_by_kind(REFERENCE_SDK_KIND_BY_NAME),
    "repair": evidence_schema_by_kind(REPAIR_KIND_BY_NAME),
    "reputation": evidence_schema_by_kind(REPUTATION_KIND_BY_NAME),
    "reserve_rent": evidence_schema_by_kind(RESERVE_RENT_KIND_BY_NAME),
    "transparency": evidence_schema_by_kind(TRANSPARENCY_KIND_BY_NAME),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Aggregate SoraFS production-readiness thresholds."""

    now_unix: int
    max_summary_artifact_age_secs: int
    deployment_id: str | None
    environment: str | None
    foundational_signer_public_key: bytes | None = None
    foundational_release_sequence: int | None = None
    foundational_previous_envelope_sha256: str | None = None


def canonical_string(value: Any) -> str | None:
    """Return a non-empty canonical string, or None."""

    return value if diagnostic_text_is_canonical(value) else None


def is_production_ready_environment(value: Any) -> bool:
    """Return whether an environment label can promote final production readiness."""

    environment = canonical_string(value)
    return environment in PRODUCTION_READY_ENVIRONMENTS


def require_reviewed_deployment_id_value(
    value: Any,
    errors: list[str],
    path: str,
) -> str:
    """Return a reviewed deployment id value or record path-scoped diagnostics."""

    field_errors: list[str] = []
    deployment_id = require_rollout_deployment_id(
        {"deployment_id": value},
        field_errors,
    )
    for error in field_errors:
        errors.append(error.replace("deployment_id", path, 1))
    return deployment_id


def require_production_deployment_id_value(
    value: Any,
    errors: list[str],
    path: str,
) -> str:
    """Return a final-production deployment id or record path-scoped diagnostics."""

    deployment_id = require_reviewed_deployment_id_value(value, errors, path)
    if not deployment_id:
        return ""
    detected_markers = forbidden_non_production_markers(
        deployment_id,
        FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS,
    )
    forbidden = sorted(
        marker
        for marker in detected_markers
        if not any(marker != other and marker in other for other in detected_markers)
    )
    if forbidden:
        errors.append(
            f"{path} must not contain non-production deployment markers {forbidden}"
        )
        return ""
    return deployment_id


def is_payload_free_sensitive_reference(normalized_key: str) -> bool:
    """Return whether a sensitive-looking key is an allowed digest marker."""

    return any(
        normalized_key.endswith(suffix)
        for suffix in PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES
    )


def is_sensitive_diagnostic_key(key: str) -> bool:
    """Return whether a canonical key name should be hidden in diagnostics."""

    exact_keys = frozenset(key.lower() for key in SENSITIVE_KEYS) | COMMON_SENSITIVE_KEYS
    normalized_keys = frozenset(normalize_sensitive_key(key) for key in exact_keys)
    key_lower = key.lower()
    normalized_key = normalize_sensitive_key(key)
    return (
        key_lower in exact_keys
        or normalized_key in normalized_keys
        or any(
            fragment in normalized_key
            and not is_payload_free_sensitive_reference(normalized_key)
            for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
        )
    )


def payload_free_diagnostic_key_label(key: str) -> str:
    """Return a safe key label for aggregate readiness diagnostics."""

    return "<sensitive-key>" if is_sensitive_diagnostic_key(key) else key


def require_string_field(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> str:
    """Return a canonical string field or record an error."""

    value = canonical_string(payload.get(field))
    if value is None:
        errors.append(f"{field} must be a non-empty canonical string")
        return ""
    return value


def require_positive_int_field(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> int | None:
    """Return a positive integer field or record an error."""

    value = payload.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        errors.append(f"{field} must be a positive integer")
        return None
    return value


def canonical_int_value(value: Any) -> int | None:
    """Return an integer value while excluding booleans."""

    if isinstance(value, int) and not isinstance(value, bool):
        return value
    return None


def require_empty_error_list(value: Any, path: str, errors: list[str]) -> bool:
    """Require an empty list of canonical errors."""

    if not isinstance(value, list):
        errors.append(f"{path} must be an empty error list")
        return False
    for error in value:
        if canonical_string(error) is None:
            errors.append(f"{path} must contain only canonical strings")
            return False
    if value:
        errors.append(f"{path} must be empty")
        return False
    return True


def require_absent_or_empty_error_list(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> bool:
    """Require an optional diagnostic list to be absent or empty."""

    if field not in payload:
        return True
    return require_empty_error_list(payload.get(field), field, errors)


def require_threshold_map(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> dict[str, int]:
    """Return validated threshold metadata or record closed failures."""

    if field not in payload:
        errors.append(f"{field} must be present")
        return {}
    thresholds = payload.get(field)
    if not isinstance(thresholds, dict):
        errors.append(f"{field} must be an object")
        return {}
    if not thresholds:
        errors.append(f"{field} must not be empty")
        return {}
    valid_thresholds: dict[str, int] = {}
    for key, value in thresholds.items():
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{field} keys must be canonical strings")
        value_is_valid = (
            isinstance(value, int)
            and not isinstance(value, bool)
            and value >= 0
        )
        key_diagnostic_label = (
            payload_free_diagnostic_key_label(key_label)
            if key_label is not None
            else "<invalid>"
        )
        key_is_sensitive = key_label is not None and is_sensitive_diagnostic_key(
            key_label
        )
        if key_is_sensitive:
            errors.append(f"{field}.{key_diagnostic_label} must not be present")
        if not value_is_valid:
            errors.append(
                f"{field}.{key_diagnostic_label} must be a non-negative integer"
            )
        if key_label is not None and value_is_valid and not key_is_sensitive:
            valid_thresholds[key_label] = value
    return valid_thresholds


def decoded_text_variants(value: str) -> tuple[str, ...]:
    """Return raw plus repeatedly percent/HTML-decoded text variants."""

    variants = [value]
    seen = {value}
    current = value
    for _ in range(4):
        decoded = unescape(unquote(current))
        if decoded == current or decoded in seen:
            break
        variants.append(decoded)
        seen.add(decoded)
        current = decoded
    return tuple(variants)


def path_component_has_windows_drive_prefix(component: str) -> bool:
    """Return whether a path component starts with a Windows drive prefix."""

    return len(component) >= 2 and component[1] == ":" and component[0].isalpha()


def path_component_has_uri_scheme_prefix(component: str) -> bool:
    """Return whether a path component starts with a URI-like scheme."""

    head, separator, _tail = component.partition(":")
    if not separator:
        return False
    return re.fullmatch(r"[A-Za-z][A-Za-z0-9+.-]*", head) is not None


def path_component_has_sensitive_label(component: str) -> bool:
    """Return whether a path component looks like runtime-only secret material."""

    sensitive_key_labels = (
        frozenset(key.lower() for key in SENSITIVE_KEYS) | COMMON_SENSITIVE_KEYS
    )
    normalized_sensitive_keys = frozenset(
        normalize_sensitive_key(key) for key in sensitive_key_labels
    )
    for variant in decoded_text_variants(component):
        stem = variant.rsplit(".", 1)[0]
        normalized_values = {
            normalize_sensitive_key(variant),
            normalize_sensitive_key(stem),
        }
        if any(value in normalized_sensitive_keys for value in normalized_values):
            return True
        if any(
            fragment in value
            for value in normalized_values
            for fragment in PATH_SENSITIVE_KEY_FRAGMENTS
        ):
            return True
    return False


def archive_path_component_is_portable(component: str) -> bool:
    """Return whether an archive path component is safe in raw or decoded form."""

    for variant in decoded_text_variants(component):
        if (
            canonical_string(variant) is None
            or variant in {".", ".."}
            or "/" in variant
            or "\\" in variant
            or path_component_has_windows_drive_prefix(variant)
            or path_component_has_uri_scheme_prefix(variant)
            or path_component_has_sensitive_label(variant)
        ):
            return False
    return True


def is_archive_portable_artifact_path(path: str) -> bool:
    """Return whether an artifact label is portable inside release archives."""

    if canonical_string(path) is None:
        return False
    if path.startswith(("/", "\\")) or "\\" in path:
        return False
    if len(path) >= 2 and path[1] == ":" and path[0].isalpha():
        return False
    return all(archive_path_component_is_portable(part) for part in path.split("/"))


def aggregate_summary_path_label(path: Path, evidence_dirs: list[Path]) -> str:
    """Return an archive-portable label for a lane summary input."""

    for directory in evidence_dirs:
        resolution_errors: list[str] = []
        resolved_path = resolve_path_identity(
            path, resolution_errors, label="summary path"
        )
        resolved_directory = resolve_path_identity(
            directory, resolution_errors, label="evidence directory"
        )
        if resolved_path is None or resolved_directory is None:
            continue
        try:
            relative_path = resolved_path.relative_to(resolved_directory)
        except ValueError:
            continue
        label = relative_path.as_posix()
        if is_archive_portable_artifact_path(label):
            return label
    name_label = path.name
    if is_archive_portable_artifact_path(name_label):
        return name_label
    return "summary.json"


def require_artifact_identity_fields(
    artifact: dict[str, Any],
    path: str,
    errors: list[str],
) -> None:
    """Require canonical artifact path and digest metadata."""

    artifact_path = canonical_string(artifact.get("path"))
    if artifact_path is None:
        errors.append(f"{path}.path must be canonical")
    elif not is_archive_portable_artifact_path(artifact_path):
        errors.append(
            f"{path}.path must be archive-relative without absolute, empty, "
            "current, parent, encoded, URI-scheme-like, platform-specific, "
            "or secret-looking segments"
        )
    sha256 = artifact.get("sha256")
    if (
        not isinstance(sha256, str)
        or len(sha256) != 64
        or any(character not in LOWER_HEX_DIGITS for character in sha256)
    ):
        errors.append(f"{path}.sha256 must be canonical lowercase SHA-256")


def require_optional_artifact_label(
    artifact: dict[str, Any],
    field: str,
    path: str,
    errors: list[str],
) -> None:
    """Require optional artifact metadata labels to be canonical."""

    if field in artifact and canonical_string(artifact.get(field)) is None:
        errors.append(f"{path}.{field} must be canonical when present")


def require_payload_free_summary_fields(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Reject non-standard top-level lane summary fields."""

    for key in payload:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append("summary keys must be canonical strings")
        elif key_label not in PAYLOAD_FREE_SUMMARY_FIELDS:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{key_diagnostic_label} is not allowed in payload-free lane summary"
            )


def validate_payload_free_metadata_value(
    value: Any,
    path: str,
    errors: list[str],
    *,
    depth: int = 0,
) -> None:
    """Validate payload-free lane metadata carried beside required rows."""

    if depth > MAX_SUMMARY_METADATA_DEPTH:
        errors.append(f"{path} nesting exceeds {MAX_SUMMARY_METADATA_DEPTH} levels")
        return
    if isinstance(value, bool):
        return
    if isinstance(value, int):
        if value < 0:
            errors.append(f"{path} must contain only non-negative integers")
        return
    if isinstance(value, str):
        if canonical_string(value) is None:
            errors.append(f"{path} must contain only canonical strings")
        return
    if isinstance(value, list):
        for index, item in enumerate(value):
            validate_payload_free_metadata_value(
                item,
                f"{path}[{index}]",
                errors,
                depth=depth + 1,
            )
        return
    if isinstance(value, dict):
        for key, item in value.items():
            key_label = canonical_string(key)
            if key_label is None:
                errors.append(f"{path} keys must be canonical strings")
                continue
            key_path_label = payload_free_diagnostic_key_label(key_label)
            validate_payload_free_metadata_value(
                item,
                f"{path}.{key_path_label}",
                errors,
                depth=depth + 1,
            )
        return
    errors.append(f"{path} must contain only payload-free canonical metadata")


def payload_free_object_list_metadata_identity(
    item: Any,
    schema: dict[str, Any],
) -> tuple[tuple[str, Any], ...] | None:
    """Return an exact comparable identity for validated object-list metadata."""

    if not isinstance(item, dict):
        return None
    string_fields = schema.get("strings", frozenset())
    positive_int_fields = schema.get("positive_ints", frozenset())
    hex_fields = schema.get("hex", {})
    allowed_fields = set(string_fields) | set(positive_int_fields) | set(hex_fields)
    if set(item) != allowed_fields:
        return None
    identity: list[tuple[str, Any]] = []
    for key in sorted(string_fields):
        value = canonical_string(item.get(key))
        if value is None:
            return None
        identity.append((key, value))
    for key in sorted(positive_int_fields):
        value = item.get(key)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            return None
        identity.append((key, value))
    for key, expected_hex_length in sorted(hex_fields.items()):
        value = item.get(key)
        if (
            not isinstance(value, str)
            or len(value) != expected_hex_length
            or any(character not in LOWER_HEX_DIGITS for character in value)
        ):
            return None
        identity.append((key, value))
    for start_key, end_key in schema.get("ordered_int_pairs", ()):
        start_value = item.get(start_key)
        end_value = item.get(end_key)
        if (
            isinstance(start_value, int)
            and not isinstance(start_value, bool)
            and isinstance(end_value, int)
            and not isinstance(end_value, bool)
            and start_value > 0
            and end_value > 0
            and end_value < start_value
        ):
            return None
    return tuple(identity)


def payload_free_object_list_metadata_domain_identity(
    field: str,
    item: Any,
    schema: dict[str, Any],
) -> tuple[tuple[str, Any], ...] | None:
    """Return the domain identity for validated object-list metadata."""

    if payload_free_object_list_metadata_identity(item, schema) is None:
        return None
    identity_fields = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS.get(field)
    if identity_fields is None:
        return None
    string_fields = schema.get("strings", frozenset())
    positive_int_fields = schema.get("positive_ints", frozenset())
    hex_fields = schema.get("hex", {})
    identity: list[tuple[str, Any]] = []
    for key in identity_fields:
        if key in string_fields:
            value = canonical_string(item.get(key))
            if value is None:
                return None
        elif key in positive_int_fields:
            value = item.get(key)
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                return None
        elif key in hex_fields:
            value = item.get(key)
            expected_hex_length = hex_fields[key]
            if (
                not isinstance(value, str)
                or len(value) != expected_hex_length
                or any(character not in LOWER_HEX_DIGITS for character in value)
            ):
                return None
        else:
            return None
        identity.append((key, value))
    return tuple(identity)


def validate_payload_free_object_list_metadata(
    field: str,
    value: Any,
    schema: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate exact payload-free object-list metadata carried by lane summaries."""

    string_fields = schema.get("strings", frozenset())
    positive_int_fields = schema.get("positive_ints", frozenset())
    hex_fields = schema.get("hex", {})
    allowed_fields = set(string_fields) | set(positive_int_fields) | set(hex_fields)
    identities: list[tuple[tuple[str, Any], ...]] = []
    domain_identities: list[tuple[tuple[str, Any], ...]] = []
    for index, item in enumerate(value):
        item_path = f"{field}[{index}]"
        if not isinstance(item, dict):
            errors.append(f"{item_path} must be a payload-free metadata object")
            continue
        identity = payload_free_object_list_metadata_identity(item, schema)
        if identity is not None:
            identities.append(identity)
        domain_identity = payload_free_object_list_metadata_domain_identity(
            field,
            item,
            schema,
        )
        if domain_identity is not None:
            domain_identities.append(domain_identity)
        for key in item:
            key_label = canonical_string(key)
            if key_label is None:
                errors.append(f"{item_path} keys must be canonical strings")
            elif key_label not in allowed_fields:
                key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
                errors.append(
                    f"{item_path}.{key_diagnostic_label} is not allowed in payload-free object metadata"
                )
        for key in sorted(string_fields):
            if canonical_string(item.get(key)) is None:
                errors.append(f"{item_path}.{key} must be a canonical string")
        for key in sorted(positive_int_fields):
            item_value = item.get(key)
            if (
                not isinstance(item_value, int)
                or isinstance(item_value, bool)
                or item_value <= 0
            ):
                errors.append(f"{item_path}.{key} must be a positive integer")
        for key, expected_hex_length in sorted(hex_fields.items()):
            item_value = item.get(key)
            if (
                not isinstance(item_value, str)
                or len(item_value) != expected_hex_length
                or any(character not in LOWER_HEX_DIGITS for character in item_value)
            ):
                errors.append(
                    f"{item_path}.{key} must be {expected_hex_length} lowercase hex characters"
                )
        for start_key, end_key in schema.get("ordered_int_pairs", ()):
            start_value = item.get(start_key)
            end_value = item.get(end_key)
            if (
                isinstance(start_value, int)
                and not isinstance(start_value, bool)
                and isinstance(end_value, int)
                and not isinstance(end_value, bool)
                and start_value > 0
                and end_value > 0
                and end_value < start_value
            ):
                errors.append(f"{item_path}.{end_key} must be >= {start_key}")
    if len(set(identities)) != len(identities):
        errors.append(f"{field} must not contain duplicate metadata entries")
    if len(set(domain_identities)) != len(domain_identities):
        errors.append(f"{field} must not contain duplicate metadata identities")


def validate_payload_free_object_metadata(
    field: str,
    value: Any,
    allowed_fields: frozenset[str],
    errors: list[str],
) -> None:
    """Validate exact payload-free object metadata carried by lane summaries."""

    if not isinstance(value, dict):
        errors.append(f"{field} must be a payload-free metadata object")
        return
    for key in value:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{field} keys must be canonical strings")
        elif key_label not in allowed_fields:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{field}.{key_diagnostic_label} is not allowed in payload-free object metadata"
            )
    for key in sorted(allowed_fields):
        if canonical_string(value.get(key)) is None:
            errors.append(f"{field}.{key} must be a canonical string")


def payload_free_metadata_deployment_context(
    value: Any,
) -> tuple[str, str] | None:
    """Return a canonical deployment context carried by payload-free metadata."""

    if not isinstance(value, dict):
        return None
    deployment_id = canonical_string(value.get("deployment_id"))
    environment = canonical_string(value.get("environment"))
    if deployment_id is None or environment is None:
        return None
    return deployment_id, environment


def payload_free_summary_metadata_deployment_contexts(
    gate: GateSummaryKind,
    payload: dict[str, Any],
) -> set[tuple[str, str]]:
    """Return deployment contexts carried by validated top-level lane metadata."""

    allowed_metadata_fields = GATE_METADATA_FIELDS.get(gate.name, frozenset())
    contexts: set[tuple[str, str]] = set()
    if "deployment_context" in allowed_metadata_fields:
        context = payload_free_metadata_deployment_context(
            payload.get("deployment_context")
        )
        if context is not None:
            contexts.add(context)
    for field, schema in PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.items():
        if field not in allowed_metadata_fields:
            continue
        string_fields = schema.get("strings", frozenset())
        if not {"deployment_id", "environment"} <= set(string_fields):
            continue
        value = payload.get(field)
        if not isinstance(value, list):
            continue
        for item in value:
            context = payload_free_metadata_deployment_context(item)
            if context is not None:
                contexts.add(context)
    return contexts


def canonical_lower_hex(value: Any, expected_hex_length: int) -> str | None:
    """Return a lowercase hex string of the expected length, or None."""

    if (
        isinstance(value, str)
        and len(value) == expected_hex_length
        and all(character in LOWER_HEX_DIGITS for character in value)
    ):
        return value
    return None


def foundational_signing_payload(payload: dict[str, Any]) -> bytes:
    """Return the canonical, domain-separated prerequisite signature payload."""

    unsigned = dict(payload)
    signature = unsigned.get("signature")
    if isinstance(signature, dict):
        unsigned_signature = dict(signature)
        unsigned_signature.pop("signature_hex", None)
        unsigned["signature"] = unsigned_signature
    return FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN + json.dumps(
        unsigned,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def parse_foundational_signer_public_key(
    value: Any,
    errors: list[str],
    *,
    path: str,
) -> bytes | None:
    """Decode one exact, non-zero Ed25519 public key without echoing it."""

    public_key_hex = canonical_lower_hex(value, 64)
    if public_key_hex is None:
        errors.append(f"{path} must be exactly 32 bytes of lowercase hex")
        return None
    public_key = bytes.fromhex(public_key_hex)
    if not any(public_key):
        errors.append(f"{path} must not be the all-zero key")
        return None
    return public_key


def validate_foundational_exact_fields(
    value: Any,
    expected_fields: frozenset[str],
    path: str,
    errors: list[str],
) -> dict[str, Any] | None:
    """Require one schema-closed object and sanitize unknown-key diagnostics."""

    if not isinstance(value, dict):
        errors.append(f"{path} must be an object")
        return None
    observed_fields = set(value)
    if observed_fields != expected_fields:
        errors.append(f"{path} fields must match the schema-closed contract")
    for key in value:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{path} keys must be canonical strings")
        elif key_label not in expected_fields:
            errors.append(
                f"{path}.{payload_free_diagnostic_key_label(key_label)} is not allowed"
            )
    return value


def validate_foundational_timestamp(
    value: Any,
    path: str,
    options: ValidationOptions,
    errors: list[str],
) -> int | None:
    """Validate one reviewed prerequisite timestamp against the aggregate clock."""

    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        errors.append(f"{path} must be a positive integer")
        return None
    if value > options.now_unix:
        errors.append(f"{path} must not be future")
    elif options.now_unix - value > options.max_summary_artifact_age_secs:
        errors.append(f"{path} exceeds max summary artifact age")
    return value


def validate_foundational_lane_summary_rows(
    value: Any,
    errors: list[str],
) -> list[dict[str, str]]:
    """Validate the exact ordered lane-summary digests signed by the envelope."""

    rows: list[dict[str, str]] = []
    if not isinstance(value, list):
        errors.append("foundational lane_summaries must be an array")
        return rows
    for index, item in enumerate(value):
        path = f"foundational lane_summaries[{index}]"
        row = validate_foundational_exact_fields(
            item,
            FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS,
            path,
            errors,
        )
        if row is None:
            continue
        gate = canonical_string(row.get("gate"))
        if gate is None:
            errors.append(f"{path}.gate must be a canonical string")
        digest = canonical_lower_hex(row.get("sha256"), 64)
        if digest is None:
            errors.append(f"{path}.sha256 must be canonical lowercase SHA-256")
        elif not any(bytes.fromhex(digest)):
            errors.append(f"{path}.sha256 must not be zero")
        if gate is not None and digest is not None and any(bytes.fromhex(digest)):
            rows.append({"gate": gate, "sha256": digest})

    observed_gates = [row["gate"] for row in rows]
    expected_gates = list(DEFAULT_REQUIRED_GATES)
    if observed_gates != expected_gates:
        errors.append(
            "foundational lane_summaries must match all 17 readiness lanes in canonical order"
        )
        if len(set(observed_gates)) != len(observed_gates):
            errors.append("foundational lane_summaries must not contain duplicate gates")
        if set(expected_gates) - set(observed_gates):
            errors.append("foundational lane_summaries are missing required gates")
        if set(observed_gates) - set(expected_gates):
            errors.append("foundational lane_summaries contain unknown gates")
    digests = [row["sha256"] for row in rows]
    if len(set(digests)) != len(digests):
        errors.append("foundational lane_summaries must use unique summary digests")
    return rows


def validate_foundational_prerequisite_summary(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[dict[str, Any], list[str], tuple[str, str] | None]:
    """Validate the signed, payload-free foundational prerequisite envelope."""

    errors: list[str] = []
    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys=SENSITIVE_KEYS,
        evidence_label="SoraFS foundational prerequisite summary",
    )
    validate_foundational_exact_fields(
        payload,
        FOUNDATIONAL_PREREQUISITE_FIELDS,
        "foundational prerequisite summary",
        errors,
    )

    if payload.get("schema") != FOUNDATIONAL_PREREQUISITE_SCHEMA:
        errors.append("foundational prerequisite schema must match the contract")
    if payload.get("status") != "verified":
        errors.append("foundational prerequisite status must be `verified`")

    deployment = validate_foundational_exact_fields(
        payload.get("deployment"),
        FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS,
        "foundational prerequisite deployment",
        errors,
    )
    deployment_id: str | None = None
    environment: str | None = None
    if deployment is not None:
        deployment_field_errors: list[str] = []
        candidate_deployment_id = require_production_deployment_id_value(
            deployment.get("deployment_id"),
            deployment_field_errors,
            "foundational prerequisite deployment_id",
        )
        errors.extend(deployment_field_errors)
        candidate_environment = canonical_string(deployment.get("environment"))
        if candidate_environment is None:
            errors.append(
                "foundational prerequisite environment must be a canonical string"
            )
        elif not is_production_ready_environment(candidate_environment):
            errors.append("foundational prerequisite environment must be production")
        if candidate_deployment_id:
            deployment_id = candidate_deployment_id
        if candidate_environment is not None:
            environment = candidate_environment
        if (
            deployment_id is not None
            and options.deployment_id is not None
            and deployment_id != options.deployment_id
        ):
            errors.append(
                "foundational prerequisite deployment_id must match --deployment-id"
            )
        if (
            environment is not None
            and options.environment is not None
            and environment != options.environment
        ):
            errors.append(
                "foundational prerequisite environment must match --environment"
            )

    generated_at_unix = validate_foundational_timestamp(
        payload.get("generated_at_unix"),
        "foundational prerequisite generated_at_unix",
        options,
        errors,
    )

    release_sequence_value = payload.get("release_sequence")
    release_sequence: int | None = None
    if (
        not isinstance(release_sequence_value, int)
        or isinstance(release_sequence_value, bool)
        or release_sequence_value <= 0
        or release_sequence_value > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
    ):
        errors.append(
            "foundational prerequisite release_sequence must be an integer in 1..2^63-1"
        )
    else:
        release_sequence = release_sequence_value
        if options.foundational_release_sequence is None:
            errors.append(
                "foundational prerequisite release_sequence requires an operator-reviewed expected value"
            )
        elif release_sequence != options.foundational_release_sequence:
            errors.append(
                "foundational prerequisite release_sequence must match the operator-reviewed expected value"
            )

    previous_envelope_sha256 = canonical_lower_hex(
        payload.get("previous_envelope_sha256"),
        64,
    )
    if previous_envelope_sha256 is None:
        errors.append(
            "foundational prerequisite previous_envelope_sha256 must be canonical lowercase SHA-256"
        )
    else:
        is_zero_predecessor = not any(bytes.fromhex(previous_envelope_sha256))
        if release_sequence == 1 and not is_zero_predecessor:
            errors.append(
                "foundational prerequisite sequence 1 must use the zero predecessor"
            )
        if release_sequence is not None and release_sequence > 1 and is_zero_predecessor:
            errors.append(
                "foundational prerequisite sequence after 1 must use a non-zero predecessor"
            )
        expected_predecessor = options.foundational_previous_envelope_sha256
        if expected_predecessor is None:
            errors.append(
                "foundational prerequisite predecessor requires an operator-reviewed expected digest"
            )
        elif previous_envelope_sha256 != expected_predecessor:
            errors.append(
                "foundational prerequisite previous_envelope_sha256 must match the operator-reviewed expected digest"
            )

    prerequisites = payload.get("prerequisites")
    prerequisite_ids: list[str] = []
    anchors: list[str] = []
    evidence_generated_times: list[int] = []
    if not isinstance(prerequisites, list):
        errors.append("foundational prerequisites must be an array")
    else:
        for index, item in enumerate(prerequisites):
            path = f"foundational prerequisites[{index}]"
            row = validate_foundational_exact_fields(
                item,
                FOUNDATIONAL_PREREQUISITE_ROW_FIELDS,
                path,
                errors,
            )
            if row is None:
                continue
            prerequisite_id = canonical_string(row.get("id"))
            if prerequisite_id is None:
                errors.append(f"{path}.id must be a canonical string")
            else:
                prerequisite_ids.append(prerequisite_id)
            if row.get("status") != "verified":
                errors.append(f"{path}.status must be `verified`")
            anchor = canonical_lower_hex(row.get("evidence_anchor_sha256"), 64)
            if anchor is None:
                errors.append(
                    f"{path}.evidence_anchor_sha256 must be canonical lowercase SHA-256"
                )
            elif not any(bytes.fromhex(anchor)):
                errors.append(f"{path}.evidence_anchor_sha256 must not be zero")
            else:
                anchors.append(anchor)
            evidence_generated_at = validate_foundational_timestamp(
                row.get("evidence_generated_at_unix"),
                f"{path}.evidence_generated_at_unix",
                options,
                errors,
            )
            if evidence_generated_at is not None:
                evidence_generated_times.append(evidence_generated_at)
                if (
                    generated_at_unix is not None
                    and evidence_generated_at > generated_at_unix
                ):
                    errors.append(
                        f"{path}.evidence_generated_at_unix must not be later than the signed envelope"
                    )

    expected_prerequisite_ids = list(FOUNDATIONAL_PREREQUISITE_IDS)
    if prerequisite_ids != expected_prerequisite_ids:
        errors.append(
            "foundational prerequisites must match the exact required set and canonical order"
        )
        if len(set(prerequisite_ids)) != len(prerequisite_ids):
            errors.append("foundational prerequisites must not contain duplicate ids")
        if set(expected_prerequisite_ids) - set(prerequisite_ids):
            errors.append("foundational prerequisites are missing required ids")
        if set(prerequisite_ids) - set(expected_prerequisite_ids):
            errors.append("foundational prerequisites contain unknown ids")
    if len(set(anchors)) != len(anchors):
        errors.append("foundational prerequisites must use unique evidence anchors")

    lane_summary_rows = validate_foundational_lane_summary_rows(
        payload.get("lane_summaries"),
        errors,
    )

    signature = validate_foundational_exact_fields(
        payload.get("signature"),
        FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS,
        "foundational prerequisite signature",
        errors,
    )
    signer_fingerprint: str | None = None
    trusted_public_key = options.foundational_signer_public_key
    if (
        not isinstance(trusted_public_key, bytes)
        or len(trusted_public_key) != 32
        or not any(trusted_public_key)
    ):
        errors.append(
            "foundational prerequisite signature requires an operator-trusted Ed25519 public key"
        )
        trusted_public_key = None
    else:
        signer_fingerprint = hashlib.sha256(trusted_public_key).hexdigest()
    signature_bytes: bytes | None = None
    if signature is not None:
        if signature.get("algorithm") != "ed25519":
            errors.append(
                "foundational prerequisite signature algorithm must be `ed25519`"
            )
        declared_fingerprint = canonical_lower_hex(
            signature.get("public_key_fingerprint_sha256"),
            64,
        )
        if declared_fingerprint is None:
            errors.append(
                "foundational prerequisite signer fingerprint must be canonical lowercase SHA-256"
            )
        elif (
            signer_fingerprint is not None
            and declared_fingerprint != signer_fingerprint
        ):
            errors.append(
                "foundational prerequisite signer fingerprint must match the operator-trusted key"
            )
        signature_hex = canonical_lower_hex(signature.get("signature_hex"), 128)
        if signature_hex is None:
            errors.append(
                "foundational prerequisite signature must be a non-zero canonical Ed25519 signature"
            )
        elif not any(bytes.fromhex(signature_hex)):
            errors.append(
                "foundational prerequisite signature must be a non-zero canonical Ed25519 signature"
            )
        else:
            signature_bytes = bytes.fromhex(signature_hex)
    if trusted_public_key is not None and signature_bytes is not None:
        try:
            signature_valid = verify_ed25519(
                trusted_public_key,
                signature_bytes,
                foundational_signing_payload(payload),
            )
        except (TypeError, ValueError):
            signature_valid = False
        if not signature_valid:
            errors.append("foundational prerequisite signature verification failed")

    context = (
        (deployment_id, environment)
        if deployment_id is not None and environment is not None
        else None
    )
    summary = {
        "schema": FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "present": True,
        "valid": not errors,
        "required_ids": expected_prerequisite_ids,
        "prerequisite_count": len(prerequisite_ids),
        "generated_at_unix": generated_at_unix,
        "oldest_evidence_generated_at_unix": (
            min(evidence_generated_times) if evidence_generated_times else None
        ),
        "newest_evidence_generated_at_unix": (
            max(evidence_generated_times) if evidence_generated_times else None
        ),
        "deployment_id": deployment_id,
        "environment": environment,
        "release_sequence": release_sequence,
        "previous_envelope_sha256": previous_envelope_sha256,
        "signer_public_key_fingerprint_sha256": signer_fingerprint,
        "evidence_anchor_sha256": anchors,
        "lane_summary_sha256": lane_summary_rows,
        "errors": errors,
    }
    return summary, errors, context


def payload_free_summary_artifact_fingerprints(
    payload: dict[str, Any],
    *,
    kind_name: str | None = None,
    kind_names: tuple[str, ...] | None = None,
) -> list[dict[str, Any]]:
    """Return recognized artifact fingerprints that can anchor metadata."""

    artifacts = payload.get("recognized_artifacts")
    if not isinstance(artifacts, list):
        return []
    fingerprints: list[dict[str, Any]] = []
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        if kind_name is not None and artifact.get("kind") != kind_name:
            continue
        if kind_names is not None and artifact.get("kind") not in kind_names:
            continue
        fingerprint = artifact.get("fingerprint")
        if isinstance(fingerprint, dict):
            fingerprints.append(fingerprint)
    return fingerprints


def payload_free_hex_list_metadata_values(
    field: str,
    value: Any,
) -> set[str] | None:
    """Return canonical values from a top-level hex metadata list."""

    if not isinstance(value, list):
        return None
    values: set[str] = set()
    for item in value:
        identity = payload_free_list_metadata_identity(field, item)
        if not isinstance(identity, str):
            return None
        values.add(identity)
    return values


def payload_free_hex_binding_metadata_values(
    field: str,
    value: Any,
) -> set[tuple[str, ...]] | None:
    """Return canonical tuples from a top-level hex binding metadata list."""

    if not isinstance(value, list):
        return None
    values: set[tuple[str, ...]] = set()
    for item in value:
        identity = payload_free_list_metadata_identity(field, item)
        if not isinstance(identity, tuple):
            return None
        values.add(identity)
    return values


def payload_free_string_list_metadata_values(value: Any) -> set[str] | None:
    """Return canonical values from top-level string metadata lists."""

    if not isinstance(value, list):
        return None
    values: set[str] = set()
    for item in value:
        item_label = canonical_string(item)
        if item_label is None:
            return None
        values.add(item_label)
    return values


def payload_free_positive_int_list_metadata_values(value: Any) -> set[int] | None:
    """Return positive integers from top-level integer metadata lists."""

    if not isinstance(value, list):
        return None
    values: set[int] = set()
    for item in value:
        if not isinstance(item, int) or isinstance(item, bool) or item <= 0:
            return None
        values.add(item)
    return values


def payload_free_object_list_hex_metadata_values(
    field: str,
    value: Any,
) -> dict[str, set[str]] | None:
    """Return canonical hex values from top-level object-list metadata."""

    if not isinstance(value, list):
        return None
    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    values = {hex_field: set() for hex_field in schema.get("hex", {})}
    for item in value:
        if not isinstance(item, dict):
            return None
        for hex_field, expected_hex_length in schema.get("hex", {}).items():
            item_value = canonical_lower_hex(
                item.get(hex_field),
                expected_hex_length,
            )
            if item_value is None:
                return None
            values[hex_field].add(item_value)
    return values


def payload_free_object_list_hex_tuple_metadata_values(
    field: str,
    value: Any,
    tuple_fields: tuple[str, ...],
) -> set[tuple[str, ...]] | None:
    """Return selected canonical hex tuples from object-list metadata."""

    if not isinstance(value, list):
        return None
    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    hex_schema = schema.get("hex", {})
    if not set(tuple_fields) <= set(hex_schema):
        return None
    values: set[tuple[str, ...]] = set()
    for item in value:
        if not isinstance(item, dict):
            return None
        tuple_value = tuple(
            canonical_lower_hex(item.get(tuple_field), hex_schema[tuple_field])
            for tuple_field in tuple_fields
        )
        if not all(value is not None for value in tuple_value):
            return None
        values.add(tuple_value)
    return values


def payload_free_object_list_string_metadata_values(
    field: str,
    value: Any,
    string_field: str,
) -> set[str] | None:
    """Return selected canonical strings from object-list metadata."""

    if not isinstance(value, list):
        return None
    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    if string_field not in schema.get("strings", frozenset()):
        return None
    values: set[str] = set()
    for item in value:
        if payload_free_object_list_metadata_identity(item, schema) is None:
            return None
        item_value = canonical_string(item.get(string_field))
        if item_value is None:
            return None
        values.add(item_value)
    return values


def payload_free_object_list_metadata_identities(
    field: str,
    value: Any,
) -> set[tuple[tuple[str, Any], ...]] | None:
    """Return exact comparable identities from object-list metadata."""

    if not isinstance(value, list):
        return None
    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    identities: set[tuple[tuple[str, Any], ...]] = set()
    for item in value:
        identity = payload_free_object_list_metadata_identity(item, schema)
        if identity is None:
            return None
        identities.add(identity)
    return identities


def fingerprint_object_list_metadata_identities(
    field: str,
    fingerprints: list[dict[str, Any]],
) -> set[tuple[tuple[str, Any], ...]]:
    """Return exact object-list metadata identities from artifact fingerprints."""

    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    allowed_fields = (
        set(schema.get("strings", frozenset()))
        | set(schema.get("positive_ints", frozenset()))
        | set(schema.get("hex", {}))
    )
    identities: set[tuple[tuple[str, Any], ...]] = set()
    for fingerprint in fingerprints:
        item = {
            key: fingerprint[key]
            for key in allowed_fields
            if key in fingerprint
        }
        identity = payload_free_object_list_metadata_identity(item, schema)
        if identity is not None:
            identities.add(identity)
    return identities


def fingerprint_hex_values(
    fingerprints: list[dict[str, Any]],
    fingerprint_field: str,
    *,
    expected_hex_length: int = 64,
) -> set[str]:
    """Return canonical hex values carried by recognized artifact fingerprints."""

    return {
        value
        for fingerprint in fingerprints
        for value in [
            canonical_lower_hex(
                fingerprint.get(fingerprint_field),
                expected_hex_length,
            )
        ]
        if value is not None
    }


def fingerprint_string_values(
    fingerprints: list[dict[str, Any]],
    fingerprint_field: str,
) -> set[str]:
    """Return canonical string values carried by recognized artifact fingerprints."""

    return {
        value
        for fingerprint in fingerprints
        for value in [canonical_string(fingerprint.get(fingerprint_field))]
        if value is not None
    }


def fingerprint_string_array_values(
    fingerprints: list[dict[str, Any]],
    fingerprint_field: str,
) -> set[str]:
    """Return canonical string-array values from recognized artifact fingerprints."""

    values: set[str] = set()
    for fingerprint in fingerprints:
        items = fingerprint.get(fingerprint_field)
        if not isinstance(items, list):
            continue
        for item in items:
            item_label = canonical_string(item)
            if item_label is not None:
                values.add(item_label)
    return values


def fingerprint_positive_int_values(
    fingerprints: list[dict[str, Any]],
    fingerprint_field: str,
) -> set[int]:
    """Return positive integer values carried by recognized artifact fingerprints."""

    return {
        value
        for fingerprint in fingerprints
        for value in [fingerprint.get(fingerprint_field)]
        if isinstance(value, int) and not isinstance(value, bool) and value > 0
    }


def fingerprint_hex_binding_values(
    fingerprints: list[dict[str, Any]],
    field: str,
    fingerprint_fields: tuple[str, ...],
) -> set[tuple[str, ...]]:
    """Return canonical binding tuples carried by recognized artifact fingerprints."""

    binding_fields = PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS[field]
    expected_lengths = tuple(binding_fields.values())
    values: set[tuple[str, ...]] = set()
    for fingerprint in fingerprints:
        binding_values = tuple(
            canonical_lower_hex(
                fingerprint.get(fingerprint_field),
                expected_hex_length,
            )
            for fingerprint_field, expected_hex_length in zip(
                fingerprint_fields,
                expected_lengths,
            )
        )
        if all(value is not None for value in binding_values):
            values.add(binding_values)
    return values


def validate_payload_free_summary_metadata_fingerprint_tethers(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require top-level valid metadata claims to be backed by artifacts."""

    allowed_metadata_fields = GATE_METADATA_FIELDS.get(gate.name, frozenset())
    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_hex_list_metadata_values(
            field,
            payload.get(field),
        )
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS.get(
            (gate.name, field),
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_hex_values(
            source_fingerprints,
            fingerprint_field,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        expected_hex_length = PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS[field]
        metadata_value = canonical_lower_hex(payload.get(field), expected_hex_length)
        if metadata_value is None:
            continue
        source_kinds = PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS.get(
            (gate.name, field),
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_hex_values(
            source_fingerprints,
            fingerprint_field,
            expected_hex_length=expected_hex_length,
        )
        if metadata_value not in fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_string_list_metadata_values(payload.get(field))
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS.get(
            (gate.name, field),
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_string_values(
            source_fingerprints,
            fingerprint_field,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_string_list_metadata_values(payload.get(field))
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = (
            PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS.get(
                (gate.name, field),
            )
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_string_array_values(
            source_fingerprints,
            fingerprint_field,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_positive_int_list_metadata_values(
            payload.get(field),
        )
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = (
            PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS.get(
                (gate.name, field),
            )
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_positive_int_values(
            source_fingerprints,
            fingerprint_field,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_fields in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_FIELDS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_hex_binding_metadata_values(
            field,
            payload.get(field),
        )
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = (
            PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS.get(
                (gate.name, field),
            )
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_hex_binding_values(
            source_fingerprints,
            field,
            fingerprint_fields,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, field_bindings in (
        PAYLOAD_FREE_SUMMARY_OBJECT_LIST_FINGERPRINT_HEX_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values_by_field = payload_free_object_list_hex_metadata_values(
            field,
            payload.get(field),
        )
        if metadata_values_by_field is None:
            continue
        required_kind = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS[field]
        source_kind = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS.get(
            (gate.name, field),
        )
        if source_kind is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        if source_kind != required_kind:
            errors.append(
                f"{field} source-kind tether must match required artifact count "
                f"kind for `{gate.name}`"
            )
            continue
        kind_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_name=source_kind,
        )
        metadata_identities = payload_free_object_list_metadata_identities(
            field,
            payload.get(field),
        )
        if metadata_identities is not None:
            fingerprint_identities = fingerprint_object_list_metadata_identities(
                field,
                kind_fingerprints,
            )
            if not metadata_identities <= fingerprint_identities:
                errors.append(
                    f"{field} entries must match recognized artifact fingerprints"
                )
        for metadata_field, fingerprint_field in field_bindings.items():
            metadata_values = metadata_values_by_field.get(metadata_field, set())
            if not metadata_values:
                continue
            expected_hex_length = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[
                field
            ]["hex"][metadata_field]
            fingerprint_values = fingerprint_hex_values(
                kind_fingerprints,
                fingerprint_field,
                expected_hex_length=expected_hex_length,
            )
            if not metadata_values <= fingerprint_values:
                errors.append(
                    f"{field}.{metadata_field} must match recognized artifact fingerprints"
                )


def required_artifact_count_for_kind(
    required: dict[str, Any],
    kind_name: str,
) -> int | None:
    """Return the artifact count for a required row when it is well-formed."""

    row = required.get(kind_name)
    if not isinstance(row, dict):
        return None
    artifacts = row.get("artifacts")
    if not isinstance(artifacts, list):
        return None
    return len(artifacts)


def validate_payload_free_object_list_metadata_counts(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    required: dict[str, Any],
    errors: list[str],
) -> None:
    """Require detail metadata lists to match their required artifact rows."""

    allowed_metadata_fields = GATE_METADATA_FIELDS.get(gate.name, frozenset())
    for field, kind_name in (
        PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        value = payload.get(field)
        if not isinstance(value, list):
            continue
        expected_count = required_artifact_count_for_kind(required, kind_name)
        if expected_count is None:
            continue
        if len(value) != expected_count:
            errors.append(
                f"{field} length must match `{kind_name}` required artifact count"
            )


def payload_free_list_metadata_identity(field: str, item: Any) -> Any | None:
    """Return a comparable identity for sorted payload-free metadata lists."""

    binding_fields = PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS.get(field)
    if binding_fields is not None:
        if not isinstance(item, dict):
            return None
        identity = []
        for key, expected_hex_length in binding_fields.items():
            value = item.get(key)
            if (
                not isinstance(value, str)
                or len(value) != expected_hex_length
                or any(character not in LOWER_HEX_DIGITS for character in value)
            ):
                return None
            identity.append(value)
        return tuple(identity)
    if field in PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS:
        if isinstance(item, int) and not isinstance(item, bool) and item > 0:
            return item
        return None
    item_label = canonical_string(item)
    if item_label is None:
        return None
    if field in PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS:
        if len(item_label) != 64 or any(
            character not in LOWER_HEX_DIGITS for character in item_label
        ):
            return None
    return item_label


def validate_payload_free_ordered_list_metadata(
    field: str,
    value: list[Any],
    errors: list[str],
) -> None:
    """Require set-derived metadata lists to preserve canonical sorted order."""

    identities = [
        payload_free_list_metadata_identity(field, item)
        for item in value
    ]
    if any(identity is None for identity in identities):
        return
    if len(set(identities)) != len(identities):
        errors.append(f"{field} must not contain duplicate metadata entries")
    if identities != sorted(identities):
        errors.append(f"{field} must be sorted in canonical order")


def validate_payload_free_required_string_list_values(
    gate: GateSummaryKind,
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Require exact reviewed values for configured string-list metadata."""

    required_values = PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES.get(
        (gate.name, field)
    )
    if required_values is None or not isinstance(value, list):
        return
    metadata_values = payload_free_string_list_metadata_values(value)
    if metadata_values is None:
        return
    required_set = frozenset(required_values)
    for required_value in required_values:
        if required_value not in metadata_values:
            errors.append(f"{field} must include metadata value `{required_value}`")
    if not metadata_values <= required_set:
        errors.append(f"{field} must not include unknown metadata values")


def validate_payload_free_allowed_string_list_values(
    gate: GateSummaryKind,
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Reject values outside a reviewed closed set for string-list metadata."""

    allowed_values = PAYLOAD_FREE_SUMMARY_ALLOWED_STRING_LIST_VALUES.get(
        (gate.name, field)
    )
    if allowed_values is None or not isinstance(value, list):
        return
    metadata_values = payload_free_string_list_metadata_values(value)
    if metadata_values is None:
        return
    if not metadata_values <= frozenset(allowed_values):
        errors.append(f"{field} must not include unknown metadata values")


def validate_payload_free_provider_id_metadata_values(
    gate: GateSummaryKind,
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Reject forged aggregate provider-id labels outside the reputation policy."""

    if gate.name != "reputation" or field != "provider_ids" or not isinstance(value, list):
        return
    for index, item in enumerate(value):
        provider_id = canonical_string(item)
        if provider_id is None:
            continue
        path = f"{field}[{index}]"
        if REPUTATION_PROVIDER_ID_PATTERN.fullmatch(provider_id) is None:
            errors.append(f"{path} must match canonical lowercase `provider-*`")
            continue
        forbidden = forbidden_non_production_markers(
            provider_id,
            REPUTATION_FORBIDDEN_PROVIDER_ID_MARKERS,
        )
        if forbidden:
            errors.append(
                f"{path} must not contain non-production markers {forbidden}"
            )


def validate_payload_free_object_list_string_field_policies(
    gate: GateSummaryKind,
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Replay lane-owned string identity policies for aggregate object metadata."""

    if not isinstance(value, list):
        return
    policies = {
        key_field: policy
        for (gate_name, metadata_field, key_field), policy in (
            PAYLOAD_FREE_SUMMARY_OBJECT_LIST_STRING_FIELD_POLICIES.items()
        )
        if gate_name == gate.name and metadata_field == field
    }
    if not policies:
        return
    for index, item in enumerate(value):
        if not isinstance(item, dict):
            continue
        for key_field, policy in policies.items():
            metadata_value = canonical_string(item.get(key_field))
            if metadata_value is None:
                continue
            path = f"{field}[{index}].{key_field}"
            pattern = policy["pattern"]
            if pattern.fullmatch(metadata_value) is None:
                errors.append(f"{path} {policy['pattern_error']}")
                continue
            forbidden_markers = policy["forbidden_markers"]
            forbidden = forbidden_non_production_markers(
                metadata_value,
                forbidden_markers,
            )
            if forbidden:
                errors.append(
                    f"{path} must not contain non-production markers {forbidden}"
                )


def validate_payload_free_deployment_context_metadata(
    path: str,
    value: Any,
    errors: list[str],
) -> None:
    """Reject non-production deployment contexts in payload-free metadata."""

    if not isinstance(value, dict):
        return
    deployment_id = canonical_string(value.get("deployment_id"))
    if deployment_id is not None:
        require_production_deployment_id_value(
            deployment_id,
            errors,
            f"{path}.deployment_id",
        )
    environment = canonical_string(value.get("environment"))
    if environment is not None and not is_production_ready_environment(environment):
        errors.append(f"{path}.environment must be production")


def validate_payload_free_object_list_deployment_context_metadata(
    field: str,
    value: Any,
    schema: dict[str, Any],
    errors: list[str],
) -> None:
    """Reject non-production deployment contexts in object-list metadata."""

    string_fields = schema.get("strings", frozenset())
    if not {"deployment_id", "environment"} <= set(string_fields):
        return
    if not isinstance(value, list):
        return
    for index, item in enumerate(value):
        validate_payload_free_deployment_context_metadata(
            f"{field}[{index}]",
            item,
            errors,
        )


def validate_payload_free_string_list_count_binding(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Require configured string-list metadata counts to match companion counts."""

    count_field = PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS.get(
        (gate.name, field)
    )
    if count_field is None or not isinstance(value, list):
        return
    metadata_values = payload_free_string_list_metadata_values(value)
    count_values = payload_free_positive_int_list_metadata_values(
        payload.get(count_field)
    )
    if metadata_values is None or count_values is None:
        return
    if len(metadata_values) not in count_values:
        errors.append(f"{count_field} must include the unique {field} count")


def validate_transparency_publication_binding_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require transparency cycle claims to carry their source-publication edge."""

    source_values = payload_free_hex_list_metadata_values(
        "valid_source_batch_digests",
        payload.get("valid_source_batch_digests"),
    )
    cycle_values = payload_free_hex_list_metadata_values(
        "valid_cycle_digests",
        payload.get("valid_cycle_digests"),
    )
    publication_bindings = payload_free_hex_binding_metadata_values(
        "valid_publication_bindings",
        payload.get("valid_publication_bindings"),
    )
    if (
        source_values is None
        or cycle_values is None
        or publication_bindings is None
    ):
        return

    binding_source_values = {
        source_batch_digest
        for source_batch_digest, _cycle_digest in publication_bindings
    }
    binding_cycle_values = {
        cycle_digest for _source_batch_digest, cycle_digest in publication_bindings
    }
    if cycle_values != binding_cycle_values:
        errors.append(
            "valid_cycle_digests must match valid_publication_bindings cycle digests"
        )
    if not binding_source_values <= source_values:
        errors.append(
            "valid_publication_bindings source batches must match valid_source_batch_digests"
        )


def validate_transparency_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate transparency metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=TRANSPARENCY_SOURCE_BOUND_KINDS,
        metadata_field="valid_source_batch_digests",
        fingerprint_field="source_batch_digest_hex",
        error=(
            "transparency source-bound artifact fingerprints must match "
            "valid_source_batch_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=TRANSPARENCY_CYCLE_BOUND_KINDS,
        metadata_field="valid_cycle_digests",
        fingerprint_field="cycle_digest_hex",
        error=(
            "transparency cycle-bound artifact fingerprints must match "
            "valid_cycle_digests"
        ),
        errors=errors,
    )


def validate_hedging_billing_cycle_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require hedging billing-cycle metadata to preserve lane bindings."""

    cycle_bindings = payload_free_hex_binding_metadata_values(
        "valid_cycle_bindings",
        payload.get("valid_cycle_bindings"),
    )
    cycle_tuple_values = payload_free_object_list_hex_tuple_metadata_values(
        "valid_billing_cycles",
        payload.get("valid_billing_cycles"),
        ("statement_bundle_digest_hex", "reconciliation_digest_hex"),
    )
    cycle_hex_values = payload_free_object_list_hex_metadata_values(
        "valid_billing_cycles",
        payload.get("valid_billing_cycles"),
    )
    policy_values = payload_free_hex_list_metadata_values(
        "valid_policy_digests",
        payload.get("valid_policy_digests"),
    )
    reference_values = payload_free_hex_list_metadata_values(
        "valid_reference_decision_ids",
        payload.get("valid_reference_decision_ids"),
    )

    if cycle_bindings is not None and cycle_tuple_values is not None:
        if cycle_bindings != cycle_tuple_values:
            errors.append(
                "valid_cycle_bindings must match valid_billing_cycles cycle tuples"
            )
    if cycle_hex_values is None:
        return
    cycle_policy_values = cycle_hex_values.get("policy_digest_hex", set())
    if policy_values is not None and policy_values != cycle_policy_values:
        errors.append(
            "valid_policy_digests must match valid_billing_cycles policy digests"
        )
    cycle_reference_values = cycle_hex_values.get("reference_decision_id_hex", set())
    if reference_values is not None and not cycle_reference_values <= reference_values:
        errors.append(
            "valid_billing_cycles reference decisions must match valid_reference_decision_ids"
        )


def validate_hedging_billing_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate hedging billing metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=HEDGING_BILLING_CYCLE_BOUND_KINDS,
        metadata_field="valid_cycle_bindings",
        fingerprint_fields=(
            "statement_bundle_digest_hex",
            "reconciliation_digest_hex",
        ),
        error=(
            "hedging_billing cycle-bound artifact fingerprints must match "
            "valid_cycle_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=HEDGING_BILLING_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "hedging_billing policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_reserve_rent_policy_matrix_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require reserve-rent aggregate metadata to preserve policy/matrix chain."""

    policy_values = payload_free_hex_list_metadata_values(
        "valid_policy_digests",
        payload.get("valid_policy_digests"),
    )
    matrix_bindings = payload_free_hex_binding_metadata_values(
        "valid_policy_matrix_bindings",
        payload.get("valid_policy_matrix_bindings"),
    )
    ledger_bindings = payload_free_hex_binding_metadata_values(
        "valid_policy_matrix_ledger_bindings",
        payload.get("valid_policy_matrix_ledger_bindings"),
    )
    provider_bake_tuples = payload_free_object_list_hex_tuple_metadata_values(
        "valid_provider_bakes",
        payload.get("valid_provider_bakes"),
        ("policy_digest_hex", "matrix_digest_hex", "ledger_digest_hex"),
    )
    provider_bake_ids = payload_free_object_list_string_metadata_values(
        "valid_provider_bakes",
        payload.get("valid_provider_bakes"),
        "bake_id",
    )
    if provider_bake_ids is not None:
        for fingerprint in payload_free_summary_artifact_fingerprints(
            payload,
            kind_name="governance_approval",
        ):
            bake_id = canonical_string(fingerprint.get("bake_id"))
            if bake_id is None or bake_id not in provider_bake_ids:
                errors.append(
                    "reserve_rent governance approval bake_id fingerprints must "
                    "match valid_provider_bakes"
                )
                break

    if policy_values is not None and matrix_bindings is not None:
        matrix_policy_values = {
            policy_digest for policy_digest, _matrix_digest in matrix_bindings
        }
        if not matrix_policy_values <= policy_values:
            errors.append(
                "valid_policy_matrix_bindings policies must match valid_policy_digests"
            )
    if matrix_bindings is None or ledger_bindings is None:
        return
    ledger_matrix_pairs = {
        (policy_digest, matrix_digest)
        for policy_digest, matrix_digest, _ledger_digest in ledger_bindings
    }
    if not ledger_matrix_pairs <= matrix_bindings:
        errors.append(
            "valid_policy_matrix_ledger_bindings matrix pairs must match "
            "valid_policy_matrix_bindings"
        )
    if provider_bake_tuples is not None and not provider_bake_tuples <= ledger_bindings:
        errors.append(
            "valid_provider_bakes ledger tuples must match "
            "valid_policy_matrix_ledger_bindings"
        )


def validate_reserve_rent_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate reserve-rent metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=RESERVE_RENT_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "reserve_rent policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=RESERVE_RENT_MATRIX_BOUND_KINDS,
        metadata_field="valid_policy_matrix_bindings",
        fingerprint_fields=("policy_digest_hex", "matrix_digest_hex"),
        error=(
            "reserve_rent matrix-bound artifact fingerprints must match "
            "valid_policy_matrix_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=RESERVE_RENT_LEDGER_BOUND_KINDS,
        metadata_field="valid_policy_matrix_ledger_bindings",
        fingerprint_fields=(
            "policy_digest_hex",
            "matrix_digest_hex",
            "ledger_digest_hex",
        ),
        error=(
            "reserve_rent ledger-bound artifact fingerprints must match "
            "valid_policy_matrix_ledger_bindings"
        ),
        errors=errors,
    )


def validate_pop_credentials_sync_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require Pop juror sync bindings to reference valid roots and revocations."""

    root_values = payload_free_hex_list_metadata_values(
        "valid_root_digests",
        payload.get("valid_root_digests"),
    )
    revocation_values = payload_free_hex_list_metadata_values(
        "valid_revocation_list_digests",
        payload.get("valid_revocation_list_digests"),
    )
    sync_bindings = payload_free_hex_binding_metadata_values(
        "valid_juror_sync_bindings",
        payload.get("valid_juror_sync_bindings"),
    )
    if sync_bindings is None:
        return
    synced_root_values = {
        synced_root for synced_root, _synced_revocation in sync_bindings
    }
    if root_values is not None and not synced_root_values <= root_values:
        errors.append(
            "valid_juror_sync_bindings roots must match valid_root_digests"
        )
    synced_revocation_values = {
        synced_revocation for _synced_root, synced_revocation in sync_bindings
    }
    if (
        revocation_values is not None
        and not synced_revocation_values <= revocation_values
    ):
        errors.append(
            "valid_juror_sync_bindings revocations must match "
            "valid_revocation_list_digests"
        )


def validate_pop_credentials_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate Pop metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata_by_kind(
        payload,
        kind_fingerprint_fields=POP_CREDENTIALS_ROOT_BOUND_FINGERPRINT_FIELDS,
        metadata_field="valid_root_digests",
        error=(
            "pop_credentials root-bound artifact fingerprints must match "
            "valid_root_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata_by_kind(
        payload,
        kind_fingerprint_fields=POP_CREDENTIALS_REVOCATION_BOUND_FINGERPRINT_FIELDS,
        metadata_field="valid_revocation_list_digests",
        error=(
            "pop_credentials revocation-bound artifact fingerprints must match "
            "valid_revocation_list_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POP_CREDENTIALS_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "pop_credentials policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_reputation_snapshot_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require reputation scalar snapshot metadata to match binding tuples."""

    snapshot_id = canonical_lower_hex(payload.get("snapshot_id_hex"), 32)
    merkle_root = canonical_lower_hex(payload.get("merkle_root_hex"), 64)
    snapshot_bindings = payload_free_hex_binding_metadata_values(
        "valid_snapshot_bindings",
        payload.get("valid_snapshot_bindings"),
    )
    if snapshot_id is None or merkle_root is None or snapshot_bindings is None:
        return
    if snapshot_bindings != {(snapshot_id, merkle_root)}:
        errors.append(
            "valid_snapshot_bindings must match snapshot_id_hex and merkle_root_hex"
        )


def validate_moderation_panel_chain_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require moderation-panel aggregate metadata to preserve lane bindings."""

    case_values = payload_free_hex_list_metadata_values(
        "valid_case_digests",
        payload.get("valid_case_digests"),
    )
    roster_bindings = payload_free_hex_binding_metadata_values(
        "valid_roster_bindings",
        payload.get("valid_roster_bindings"),
    )
    tally_bindings = payload_free_hex_binding_metadata_values(
        "valid_tally_bindings",
        payload.get("valid_tally_bindings"),
    )
    e2e_tallies = payload_free_object_list_hex_tuple_metadata_values(
        "valid_e2e_runs",
        payload.get("valid_e2e_runs"),
        ("case_digest_hex", "roster_hash_hex", "tally_digest_hex"),
    )
    evidence_viewer_rosters = payload_free_object_list_hex_tuple_metadata_values(
        "valid_evidence_viewer_digest_sets",
        payload.get("valid_evidence_viewer_digest_sets"),
        ("case_digest_hex", "roster_hash_hex"),
    )
    if case_values is None or roster_bindings is None:
        return

    roster_case_values = {
        case_digest for case_digest, _roster_hash in roster_bindings
    }
    if not roster_case_values <= case_values:
        errors.append(
            "valid_roster_bindings case digests must match valid_case_digests"
        )
    if tally_bindings is None:
        return

    tally_roster_pairs = {
        (case_digest, roster_hash)
        for case_digest, roster_hash, _tally_digest in tally_bindings
    }
    if not tally_roster_pairs <= roster_bindings:
        errors.append(
            "valid_tally_bindings roster pairs must match valid_roster_bindings"
        )
    if e2e_tallies is not None and not e2e_tallies <= tally_bindings:
        errors.append(
            "valid_e2e_runs tally bindings must match valid_tally_bindings"
        )
    if (
        evidence_viewer_rosters is not None
        and not evidence_viewer_rosters <= roster_bindings
    ):
        errors.append(
            "valid_evidence_viewer_digest_sets roster pairs must match valid_roster_bindings"
        )


def fingerprint_hex_tuple_value(
    fingerprint: dict[str, Any],
    metadata_field: str,
    fingerprint_fields: tuple[str, ...],
) -> tuple[str, ...] | None:
    """Return one canonical hex tuple from an artifact fingerprint."""

    binding_fields = PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS[metadata_field]
    tuple_value = []
    for fingerprint_field in fingerprint_fields:
        expected_hex_length = binding_fields.get(fingerprint_field)
        if expected_hex_length is None:
            return None
        value = canonical_lower_hex(
            fingerprint.get(fingerprint_field),
            expected_hex_length,
        )
        if value is None:
            return None
        tuple_value.append(value)
    return tuple(tuple_value)


def bound_artifact_fingerprints_match_hex_list_metadata(
    payload: dict[str, Any],
    *,
    kind_names: tuple[str, ...],
    metadata_field: str,
    fingerprint_field: str,
    error: str,
    errors: list[str],
    expected_hex_length: int = 64,
) -> None:
    """Require every bound artifact fingerprint to carry a valid metadata value."""

    metadata_values = payload_free_hex_list_metadata_values(
        metadata_field,
        payload.get(metadata_field),
    )
    if metadata_values is None:
        return
    fingerprints = payload_free_summary_artifact_fingerprints(
        payload,
        kind_names=kind_names,
    )
    for fingerprint in fingerprints:
        value = canonical_lower_hex(
            fingerprint.get(fingerprint_field),
            expected_hex_length,
        )
        if value is None or value not in metadata_values:
            errors.append(error)
            return


def bound_artifact_fingerprints_match_hex_list_metadata_by_kind(
    payload: dict[str, Any],
    *,
    kind_fingerprint_fields: tuple[tuple[str, str], ...],
    metadata_field: str,
    error: str,
    errors: list[str],
    expected_hex_length: int = 64,
) -> None:
    """Require every bound artifact fingerprint to carry a valid metadata value."""

    metadata_values = payload_free_hex_list_metadata_values(
        metadata_field,
        payload.get(metadata_field),
    )
    if metadata_values is None:
        return
    for kind_name, fingerprint_field in kind_fingerprint_fields:
        fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_name=kind_name,
        )
        for fingerprint in fingerprints:
            value = canonical_lower_hex(
                fingerprint.get(fingerprint_field),
                expected_hex_length,
            )
            if value is None or value not in metadata_values:
                errors.append(error)
                return


def bound_artifact_fingerprints_match_hex_binding_metadata(
    payload: dict[str, Any],
    *,
    kind_names: tuple[str, ...],
    metadata_field: str,
    fingerprint_fields: tuple[str, ...],
    error: str,
    errors: list[str],
) -> None:
    """Require every bound artifact fingerprint to carry a valid metadata tuple."""

    metadata_values = payload_free_hex_binding_metadata_values(
        metadata_field,
        payload.get(metadata_field),
    )
    if metadata_values is None:
        return
    fingerprints = payload_free_summary_artifact_fingerprints(
        payload,
        kind_names=kind_names,
    )
    for fingerprint in fingerprints:
        tuple_value = fingerprint_hex_tuple_value(
            fingerprint,
            metadata_field,
            fingerprint_fields,
        )
        if tuple_value is None or tuple_value not in metadata_values:
            errors.append(error)
            return


def validate_moderation_panel_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate moderation-panel metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=MODERATION_PANEL_CASE_BOUND_KINDS,
        metadata_field="valid_case_digests",
        fingerprint_field="case_digest_hex",
        error=(
            "moderation_panel case-bound artifact fingerprints must match "
            "valid_case_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=MODERATION_PANEL_ROSTER_BOUND_KINDS,
        metadata_field="valid_roster_bindings",
        fingerprint_fields=("case_digest_hex", "roster_hash_hex"),
        error=(
            "moderation_panel roster-bound artifact fingerprints must match "
            "valid_roster_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=MODERATION_PANEL_TALLY_BOUND_KINDS,
        metadata_field="valid_tally_bindings",
        fingerprint_fields=(
            "case_digest_hex",
            "roster_hash_hex",
            "tally_digest_hex",
        ),
        error=(
            "moderation_panel tally-bound artifact fingerprints must match "
            "valid_tally_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=MODERATION_PANEL_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "moderation_panel policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_reputation_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate reputation metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPUTATION_SNAPSHOT_ANCHOR_KINDS,
        metadata_field="valid_reputation_weight_digests",
        fingerprint_field="weights_digest_hex",
        error=(
            "reputation publish/latest artifact fingerprints must match "
            "valid_reputation_weight_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=REPUTATION_SNAPSHOT_BOUND_KINDS,
        metadata_field="valid_snapshot_bindings",
        fingerprint_fields=("snapshot_id_hex", "merkle_root_hex"),
        error=(
            "reputation snapshot-bound artifact fingerprints must match "
            "valid_snapshot_bindings"
        ),
        errors=errors,
    )


def validate_appeal_finance_multi_peer_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require multi-peer run metadata to use lane-proven pricing configs."""

    config_values = payload_free_hex_list_metadata_values(
        "valid_config_digests",
        payload.get("valid_config_digests"),
    )
    run_hex_values = payload_free_object_list_hex_metadata_values(
        "valid_multi_peer_runs",
        payload.get("valid_multi_peer_runs"),
    )
    if config_values is None or run_hex_values is None:
        return

    run_config_values = run_hex_values.get("config_digest_hex", set())
    if not run_config_values <= config_values:
        errors.append(
            "valid_multi_peer_runs config digests must match valid_config_digests"
        )


def validate_appeal_finance_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate appeal finance metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=APPEAL_FINANCE_CONFIG_BOUND_KINDS,
        metadata_field="valid_config_digests",
        fingerprint_field="config_digest_hex",
        error=(
            "appeal_finance config-bound artifact fingerprints must match "
            "valid_config_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=APPEAL_FINANCE_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "appeal_finance policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_ai_prescreen_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate AI pre-screen metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=AI_PRESCREEN_RUNNER_BOUND_KINDS,
        metadata_field="valid_runner_bindings",
        fingerprint_fields=(
            "manifest_id_hex",
            "runner_hash_hex",
            "subject_digest_hex",
        ),
        error=(
            "ai_prescreen runner-bound artifact fingerprints must match "
            "valid_runner_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=AI_PRESCREEN_WORKFLOW_BOUND_KINDS,
        metadata_field="valid_workflow_digests",
        fingerprint_field="workflow_digest_hex",
        error=(
            "ai_prescreen workflow-bound artifact fingerprints must match "
            "valid_workflow_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=AI_PRESCREEN_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "ai_prescreen policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_governance_dag_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate Governance DAG metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GOVERNANCE_DAG_PUBLIC_HEAD_BOUND_KINDS,
        metadata_field="valid_public_head_cids",
        fingerprint_field="public_head_cid_hex",
        error=(
            "governance_dag public-head-bound artifact fingerprints must match "
            "valid_public_head_cids"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GOVERNANCE_DAG_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "governance_dag policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_gateway_load_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate gateway-load metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=("staging_load",),
        metadata_field="valid_suite_report_digests",
        fingerprint_field="suite_report_digest_hex",
        error=(
            "gateway_load suite-bound artifact fingerprints must match "
            "valid_suite_report_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GATEWAY_LOAD_STAGING_REPORT_BOUND_KINDS,
        metadata_field="valid_staging_report_digests",
        fingerprint_field="staging_report_digest_hex",
        error=(
            "gateway_load staging-bound artifact fingerprints must match "
            "valid_staging_report_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GATEWAY_LOAD_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "gateway_load policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_gateway_compliance_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate gateway-compliance metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GATEWAY_COMPLIANCE_CATALOG_BOUND_KINDS,
        metadata_field="valid_catalog_digests",
        fingerprint_field="catalog_digest_hex",
        error=(
            "gateway_compliance catalog-bound artifact fingerprints must match "
            "valid_catalog_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GATEWAY_COMPLIANCE_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "gateway_compliance policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_pdp_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate PDP metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=PDP_PROOF_SUMMARY_BOUND_KINDS,
        metadata_field="valid_proof_summary_digests",
        fingerprint_field="proof_summary_digest_hex",
        error=(
            "pdp proof-summary-bound artifact fingerprints must match "
            "valid_proof_summary_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=PDP_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "pdp policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=PDP_PROVIDER_ROSTER_BOUND_KINDS,
        metadata_field="valid_provider_roster_digests",
        fingerprint_field="provider_roster_digest_hex",
        error=(
            "pdp provider-roster-bound artifact fingerprints must match "
            "valid_provider_roster_digests"
        ),
        errors=errors,
    )


def validate_orderbook_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate orderbook metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=ORDERBOOK_CONTRACT_BOUND_KINDS,
        metadata_field="valid_contract_digests",
        fingerprint_field="contract_digest_hex",
        error=(
            "orderbook contract-bound artifact fingerprints must match "
            "valid_contract_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=ORDERBOOK_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "orderbook policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_potr_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate PoTR metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POTR_RECEIPT_SUMMARY_BOUND_KINDS,
        metadata_field="valid_receipt_summary_digests",
        fingerprint_field="receipt_summary_digest_hex",
        error=(
            "potr receipt-summary-bound artifact fingerprints must match "
            "valid_receipt_summary_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POTR_PQ_KEY_ROSTER_BOUND_KINDS,
        metadata_field="valid_pq_key_roster_digests",
        fingerprint_field="pq_key_roster_digest_hex",
        error=(
            "potr pq-key-roster-bound artifact fingerprints must match "
            "valid_pq_key_roster_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POTR_REPUTATION_WEIGHT_BOUND_KINDS,
        metadata_field="valid_reputation_weight_policy_digests",
        fingerprint_field="reputation_weight_policy_digest_hex",
        error=(
            "potr reputation-weight-bound artifact fingerprints must match "
            "valid_reputation_weight_policy_digests"
        ),
        errors=errors,
    )


def validate_por_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate PoR metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POR_SEED_REPLAY_BOUND_KINDS,
        metadata_field="valid_seed_replay_digests",
        fingerprint_field="seed_replay_digest_hex",
        error=(
            "por seed-replay-bound artifact fingerprints must match "
            "valid_seed_replay_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POR_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "por policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_repair_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate repair metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPAIR_ROSTER_BOUND_KINDS,
        metadata_field="valid_roster_digests",
        fingerprint_field="roster_digest_hex",
        error=(
            "repair roster-bound artifact fingerprints must match "
            "valid_roster_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPAIR_FAILURE_BOUND_KINDS,
        metadata_field="valid_failure_bundle_digests",
        fingerprint_field="evidence_bundle_digest_hex",
        error=(
            "repair failure-bound artifact fingerprints must match "
            "valid_failure_bundle_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPAIR_HANDOFF_BOUND_KINDS,
        metadata_field="valid_handoff_digests",
        fingerprint_field="handoff_digest_hex",
        error=(
            "repair handoff-bound artifact fingerprints must match "
            "valid_handoff_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPAIR_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "repair policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_reference_sdk_release_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate reference SDK release metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REFERENCE_SDK_RELEASE_MANIFEST_BOUND_KINDS,
        metadata_field="valid_release_manifest_digests",
        fingerprint_field="release_manifest_digest_hex",
        error=(
            "reference_sdk_release manifest-bound artifact fingerprints must match "
            "valid_release_manifest_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REFERENCE_SDK_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "reference_sdk_release policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=("governance_approval",),
        metadata_field="valid_release_key_fingerprints",
        fingerprint_field="public_key_fingerprint_hex",
        error=(
            "reference_sdk_release governance approval release-key fingerprints "
            "must match valid_release_key_fingerprints"
        ),
        errors=errors,
    )


def validate_payload_free_cross_metadata_bindings(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate lane-specific relationships between metadata fields."""

    if gate.name == "ai_prescreen":
        validate_ai_prescreen_bound_artifact_metadata(payload, errors)
    if gate.name == "appeal_finance":
        validate_appeal_finance_multi_peer_metadata(payload, errors)
        validate_appeal_finance_bound_artifact_metadata(payload, errors)
    if gate.name == "governance_dag":
        validate_governance_dag_bound_artifact_metadata(payload, errors)
    if gate.name == "gateway_load":
        validate_gateway_load_bound_artifact_metadata(payload, errors)
    if gate.name == "gateway_compliance":
        validate_gateway_compliance_bound_artifact_metadata(payload, errors)
    if gate.name == "hedging_billing":
        validate_hedging_billing_cycle_metadata(payload, errors)
        validate_hedging_billing_bound_artifact_metadata(payload, errors)
    if gate.name == "moderation_panel":
        validate_moderation_panel_chain_metadata(payload, errors)
        validate_moderation_panel_bound_artifact_metadata(payload, errors)
    if gate.name == "orderbook":
        validate_orderbook_bound_artifact_metadata(payload, errors)
    if gate.name == "pdp":
        validate_pdp_bound_artifact_metadata(payload, errors)
    if gate.name == "pop_credentials":
        validate_pop_credentials_sync_metadata(payload, errors)
        validate_pop_credentials_bound_artifact_metadata(payload, errors)
    if gate.name == "por":
        validate_por_bound_artifact_metadata(payload, errors)
    if gate.name == "potr":
        validate_potr_bound_artifact_metadata(payload, errors)
    if gate.name == "repair":
        validate_repair_bound_artifact_metadata(payload, errors)
    if gate.name == "reference_sdk_release":
        validate_reference_sdk_release_bound_artifact_metadata(payload, errors)
    if gate.name == "reputation":
        validate_reputation_snapshot_metadata(payload, errors)
        validate_reputation_bound_artifact_metadata(payload, errors)
    if gate.name == "reserve_rent":
        validate_reserve_rent_policy_matrix_metadata(payload, errors)
        validate_reserve_rent_bound_artifact_metadata(payload, errors)
    if gate.name == "transparency":
        validate_transparency_publication_binding_metadata(payload, errors)
        validate_transparency_bound_artifact_metadata(payload, errors)


def validate_payload_free_summary_metadata(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    errors: list[str],
    *,
    enforce_production_deployment_context: bool = False,
) -> None:
    """Validate required top-level lane metadata shapes."""

    allowed_metadata_fields = GATE_METADATA_FIELDS.get(gate.name, frozenset())
    for field in sorted(allowed_metadata_fields):
        if field not in payload:
            errors.append(f"{field} is required for `{gate.name}` lane metadata")
    for field in sorted(PAYLOAD_FREE_SUMMARY_METADATA_FIELDS):
        if field not in payload:
            continue
        if field not in allowed_metadata_fields:
            errors.append(f"{field} is not allowed for `{gate.name}` lane metadata")
            continue
        value = payload.get(field)
        if field in PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS and not isinstance(
            value,
            list,
        ):
            errors.append(f"{field} must be a payload-free metadata list")
            continue
        if field in PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS and not value:
            errors.append(f"{field} must not be empty for `{gate.name}` lane metadata")
        if field in PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS:
            validate_payload_free_ordered_list_metadata(field, value, errors)
        if (
            field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS
            or field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS
        ):
            validate_payload_free_allowed_string_list_values(
                gate,
                field,
                value,
                errors,
            )
            validate_payload_free_required_string_list_values(
                gate,
                field,
                value,
                errors,
            )
            validate_payload_free_provider_id_metadata_values(
                gate,
                field,
                value,
                errors,
            )
            validate_payload_free_string_list_count_binding(
                gate,
                payload,
                field,
                value,
                errors,
            )
        object_list_schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.get(field)
        if object_list_schema is not None:
            validate_payload_free_object_list_metadata(
                field,
                value,
                object_list_schema,
                errors,
            )
            validate_payload_free_object_list_string_field_policies(
                gate,
                field,
                value,
                errors,
            )
            if enforce_production_deployment_context:
                validate_payload_free_object_list_deployment_context_metadata(
                    field,
                    value,
                    object_list_schema,
                    errors,
                )
            continue
        binding_fields = PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS.get(field)
        if binding_fields is not None:
            for index, item in enumerate(value):
                item_path = f"{field}[{index}]"
                if not isinstance(item, dict):
                    errors.append(f"{item_path} must be a payload-free binding object")
                    continue
                for key in item:
                    key_label = canonical_string(key)
                    if key_label is None:
                        errors.append(f"{item_path} keys must be canonical strings")
                    elif key_label not in binding_fields:
                        key_diagnostic_label = payload_free_diagnostic_key_label(
                            key_label
                        )
                        errors.append(
                            f"{item_path}.{key_diagnostic_label} is not allowed in payload-free binding metadata"
                        )
                for key, expected_hex_length in binding_fields.items():
                    item_value = item.get(key)
                    if (
                        not isinstance(item_value, str)
                        or len(item_value) != expected_hex_length
                        or any(
                            character not in LOWER_HEX_DIGITS
                            for character in item_value
                        )
                    ):
                        errors.append(
                            f"{item_path}.{key} must be "
                            f"{expected_hex_length} lowercase hex characters"
                        )
            continue
        if field in PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS:
            for index, item in enumerate(value):
                if (
                    not isinstance(item, str)
                    or len(item) != 64
                    or any(character not in LOWER_HEX_DIGITS for character in item)
                ):
                    errors.append(
                        f"{field}[{index}] must be 64 lowercase hex characters"
                    )
            continue
        if field in PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS:
            for index, item in enumerate(value):
                if not isinstance(item, int) or isinstance(item, bool) or item <= 0:
                    errors.append(f"{field}[{index}] must be a positive integer")
            continue
        if (
            field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS
            or field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS
        ):
            for index, item in enumerate(value):
                if canonical_string(item) is None:
                    errors.append(f"{field}[{index}] must be a canonical string")
            continue
        if field in PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS:
            expected_hex_length = PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS[field]
            if (
                canonical_string(value) is None
                or not isinstance(value, str)
                or len(value) != expected_hex_length
                or any(character not in LOWER_HEX_DIGITS for character in value)
            ):
                errors.append(
                    f"{field} must be {expected_hex_length} lowercase hex characters"
                )
            continue
        object_fields = PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS.get(field)
        if object_fields is not None:
            validate_payload_free_object_metadata(field, value, object_fields, errors)
            if enforce_production_deployment_context:
                validate_payload_free_deployment_context_metadata(field, value, errors)
            continue
        errors.append(f"{field} validator is not configured for `{gate.name}`")
    validate_payload_free_cross_metadata_bindings(gate, payload, errors)


def require_payload_free_artifact_fields(
    artifact: dict[str, Any],
    path: str,
    errors: list[str],
) -> None:
    """Reject non-standard artifact fields before aggregate promotion."""

    for key in artifact:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{path} keys must be canonical strings")
        elif key_label not in PAYLOAD_FREE_ARTIFACT_FIELDS:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{path}.{key_diagnostic_label} is not allowed in payload-free artifact summary"
            )


def require_payload_free_required_row_fields(
    row: dict[str, Any],
    path: str,
    errors: list[str],
) -> None:
    """Reject non-standard required-row fields before aggregate promotion."""

    for key in row:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{path} keys must be canonical strings")
        elif key_label not in PAYLOAD_FREE_REQUIRED_ROW_FIELDS:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{path}.{key_diagnostic_label} is not allowed in payload-free required row"
            )


def artifact_identity(
    kind_name: str,
    artifact: dict[str, Any],
) -> tuple[str, str, str] | None:
    """Return a comparable artifact identity when path and digest are canonical."""

    artifact_path = canonical_string(artifact.get("path"))
    sha256 = artifact.get("sha256")
    if (
        artifact_path is None
        or not is_archive_portable_artifact_path(artifact_path)
        or not isinstance(sha256, str)
        or len(sha256) != 64
        or any(character not in LOWER_HEX_DIGITS for character in sha256)
    ):
        return None
    return kind_name, artifact_path, sha256


def artifact_generated_at(
    fingerprint: dict[str, Any],
    path: str,
    errors: list[str],
) -> int | None:
    """Return an artifact generation timestamp from its fingerprint."""

    generated_at = fingerprint.get("generated_at_unix")
    if (
        not isinstance(generated_at, int)
        or isinstance(generated_at, bool)
        or generated_at <= 0
    ):
        errors.append(f"{path}.fingerprint.generated_at_unix must be positive")
        return None
    return generated_at


def validate_payload_free_artifact_fingerprint(
    artifact: dict[str, Any],
    path: str,
    errors: list[str],
) -> dict[str, Any] | None:
    """Require artifact fingerprints to carry only payload-free metadata."""

    fingerprint = artifact.get("fingerprint")
    if not isinstance(fingerprint, dict):
        errors.append(f"{path}.fingerprint must be an object")
        return None
    validate_payload_free_metadata_value(fingerprint, f"{path}.fingerprint", errors)
    return fingerprint


def artifact_deployment_context(
    fingerprint: dict[str, Any],
    path: str,
    errors: list[str],
    *,
    enforce_production_deployment_context: bool = False,
) -> tuple[str, str] | None:
    """Return the deployment context recorded in an artifact fingerprint."""

    deployment_id = canonical_string(fingerprint.get("deployment_id"))
    environment = canonical_string(fingerprint.get("environment"))
    reviewed = fingerprint.get("deployment_context_reviewed")
    if deployment_id is None:
        errors.append(f"{path}.fingerprint.deployment_id must be canonical")
    if environment is None:
        errors.append(f"{path}.fingerprint.environment must be canonical")
    if reviewed is not True:
        errors.append(f"{path}.fingerprint.deployment_context_reviewed must be true")
    if enforce_production_deployment_context and deployment_id is not None:
        require_production_deployment_id_value(
            deployment_id,
            errors,
            f"{path}.fingerprint.deployment_id",
        )
    if (
        enforce_production_deployment_context
        and environment is not None
        and not is_production_ready_environment(environment)
    ):
        errors.append(f"{path}.fingerprint.environment must be production")
    if deployment_id is None or environment is None or reviewed is not True:
        return None
    return deployment_id, environment


def validate_summary_artifact(
    artifact: dict[str, Any],
    path: str,
    options: ValidationOptions,
    errors: list[str],
) -> tuple[list[int], set[tuple[str, str]]]:
    """Validate one payload-free artifact summary row."""

    require_payload_free_artifact_fields(artifact, path, errors)
    require_artifact_identity_fields(artifact, path, errors)
    require_optional_artifact_label(artifact, "schema", path, errors)
    require_optional_artifact_label(artifact, "status", path, errors)
    status = canonical_string(artifact.get("status"))
    if status is None:
        errors.append(f"{path}.status must be canonical")
    elif status not in SUCCESS_ARTIFACT_STATUSES:
        errors.append(f"{path}.status must be a successful status")
    if artifact.get("valid") is not True:
        errors.append(f"{path}.valid must be true")
    require_empty_error_list(
        artifact.get("errors"),
        f"{path}.errors",
        errors,
    )
    fingerprint = validate_payload_free_artifact_fingerprint(artifact, path, errors)
    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    if fingerprint is not None:
        generated_at = artifact_generated_at(fingerprint, path, errors)
        if generated_at is not None:
            if generated_at > options.now_unix:
                errors.append(f"{path}.fingerprint.generated_at_unix must not be future")
            elif (
                options.now_unix - generated_at
                > options.max_summary_artifact_age_secs
            ):
                errors.append(
                    f"{path}.fingerprint.generated_at_unix exceeds max summary artifact age"
                )
            generated_times.append(generated_at)
        context = artifact_deployment_context(
            fingerprint,
            path,
            errors,
            enforce_production_deployment_context=is_production_ready_environment(
                options.environment
            ),
        )
        if context is not None:
            deployment_contexts.add(context)
    return generated_times, deployment_contexts


def validate_required_row(
    gate_name: str,
    kind_name: str,
    expected_schema: str | None,
    row: Any,
    options: ValidationOptions,
    errors: list[str],
) -> tuple[int, list[int], set[tuple[str, str]]]:
    """Validate one required-kind row from a lane summary."""

    path = f"{gate_name}.required.{kind_name}"
    if not isinstance(row, dict):
        errors.append(f"{path} must be an object")
        return 0, [], set()
    require_payload_free_required_row_fields(row, path, errors)
    row_schema = canonical_string(row.get("schema"))
    if row_schema is None:
        errors.append(f"{path}.schema must be canonical")
    elif expected_schema is None:
        errors.append(f"{path}.schema gate contract is not configured")
    elif row_schema != expected_schema:
        errors.append(f"{path}.schema must match required evidence schema")
    if row.get("present") is not True:
        errors.append(f"{path}.present must be true")
    if row.get("valid") is not True:
        errors.append(f"{path}.valid must be true")
    artifact_count = row.get("artifact_count")
    if (
        not isinstance(artifact_count, int)
        or isinstance(artifact_count, bool)
        or artifact_count <= 0
    ):
        errors.append(f"{path}.artifact_count must be positive")
        artifact_count = 0
    require_empty_error_list(row.get("errors"), f"{path}.errors", errors)
    artifacts = row.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        errors.append(f"{path}.artifacts must be a non-empty array")
        if artifact_count:
            errors.append(f"{path}.artifact_count must match artifact object count")
        return 0, [], set()
    observed_artifact_count = sum(
        1 for artifact in artifacts if isinstance(artifact, dict)
    )
    if artifact_count and artifact_count != observed_artifact_count:
        errors.append(f"{path}.artifact_count must match artifact object count")
    artifact_count = observed_artifact_count

    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    artifact_paths: Counter[str] = Counter()
    artifact_identities: Counter[tuple[str, str, str]] = Counter()
    for index, artifact in enumerate(artifacts):
        artifact_path = f"{path}.artifacts[{index}]"
        if not isinstance(artifact, dict):
            errors.append(f"{artifact_path} must be an object")
            continue
        artifact_file_path = canonical_string(artifact.get("path"))
        if artifact_file_path is not None:
            artifact_paths[artifact_file_path] += 1
        if "kind" in artifact:
            artifact_kind = canonical_string(artifact.get("kind"))
            if artifact_kind is None:
                errors.append(f"{artifact_path}.kind must be canonical when present")
            elif artifact_kind != kind_name:
                errors.append(f"{artifact_path}.kind must match required row kind")
        artifact_schema = canonical_string(artifact.get("schema"))
        if artifact_schema is None:
            errors.append(f"{artifact_path}.schema must be canonical")
        elif expected_schema is None:
            errors.append(f"{artifact_path}.schema gate contract is not configured")
        elif artifact_schema != expected_schema:
            errors.append(f"{artifact_path}.schema must match required evidence schema")
        identity = artifact_identity(kind_name, artifact)
        if identity is not None:
            artifact_identities[identity] += 1
        row_times, row_contexts = validate_summary_artifact(
            artifact,
            artifact_path,
            options,
            errors,
        )
        generated_times.extend(row_times)
        deployment_contexts.update(row_contexts)
    if any(count > 1 for count in artifact_paths.values()):
        errors.append(f"{path}.artifacts must not duplicate artifact paths")
    if any(count > 1 for count in artifact_identities.values()):
        errors.append(f"{path}.artifacts must not duplicate artifact identities")
    return artifact_count, generated_times, deployment_contexts


def validate_recognized_artifacts(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    required: dict[str, Any],
    recognized_artifact_count: int | None,
    options: ValidationOptions,
    errors: list[str],
) -> tuple[list[int], set[tuple[str, str]], int | None, int | None]:
    """Validate required top-level recognized artifact inventory rows."""

    if "recognized_artifacts" not in payload:
        errors.append("recognized_artifacts must be present")
        return [], set(), None, None
    artifacts = payload.get("recognized_artifacts")
    path = "recognized_artifacts"
    if not isinstance(artifacts, list) or not artifacts:
        errors.append(f"{path} must be a non-empty array")
        return [], set(), None, None
    recognized_artifact_object_count = sum(
        1 for artifact in artifacts if isinstance(artifact, dict)
    )
    if (
        recognized_artifact_count is not None
        and len(artifacts) != recognized_artifact_count
    ):
        errors.append(
            "recognized_artifacts length must match recognized_artifact_count"
        )
    if (
        recognized_artifact_count is not None
        and recognized_artifact_object_count != recognized_artifact_count
    ):
        errors.append(
            "recognized_artifact_count must match recognized artifact object count"
        )

    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    recognized_artifact_paths: set[str] = set()
    recognized_artifact_path_counts: Counter[str] = Counter()
    expected_required_kinds = set(gate.required_kinds)
    expected_artifact_counts = {
        kind_name: (
            sum(
                1
                for artifact in row.get("artifacts")
                if isinstance(artifact, dict)
            )
            if isinstance(row, dict) and isinstance(row.get("artifacts"), list)
            else 0
        )
        for kind_name, row in required.items()
        if kind_name in expected_required_kinds
    }
    recognized_artifact_counts = {kind_name: 0 for kind_name in expected_required_kinds}
    expected_artifact_identities: Counter[tuple[str, str, str]] = Counter()
    required_artifacts_by_identity: dict[tuple[str, str, str], dict[str, Any]] = {}
    for kind_name, row in required.items():
        if kind_name not in expected_required_kinds or not isinstance(row, dict):
            continue
        required_artifacts = row.get("artifacts")
        if not isinstance(required_artifacts, list):
            continue
        for artifact in required_artifacts:
            if not isinstance(artifact, dict):
                continue
            identity = artifact_identity(kind_name, artifact)
            if identity is not None:
                expected_artifact_identities[identity] += 1
                required_artifacts_by_identity.setdefault(identity, artifact)
    recognized_artifact_identities: Counter[tuple[str, str, str]] = Counter()
    for index, artifact in enumerate(artifacts):
        artifact_path = f"{path}[{index}]"
        if not isinstance(artifact, dict):
            errors.append(f"{artifact_path} must be an object")
            continue
        artifact_file_path = canonical_string(artifact.get("path"))
        if artifact_file_path is not None and is_archive_portable_artifact_path(
            artifact_file_path
        ):
            recognized_artifact_paths.add(artifact_file_path)
            recognized_artifact_path_counts[artifact_file_path] += 1
        kind_name = canonical_string(artifact.get("kind"))
        if kind_name is None:
            errors.append(f"{artifact_path}.kind must be canonical")
        elif kind_name not in expected_required_kinds:
            errors.append(
                f"{artifact_path}.kind must be part of the full `{gate.name}` "
                "gate contract"
            )
        else:
            recognized_artifact_counts[kind_name] += 1
            identity = artifact_identity(kind_name, artifact)
            if identity is not None:
                recognized_artifact_identities[identity] += 1
                required_artifact = required_artifacts_by_identity.get(identity)
                if required_artifact is not None:
                    for metadata_field in ("schema", "status", "fingerprint"):
                        if artifact.get(metadata_field) != required_artifact.get(
                            metadata_field
                        ):
                            errors.append(
                                f"{artifact_path}.{metadata_field} must match "
                                "the required artifact metadata"
                            )
        row_times, row_contexts = validate_summary_artifact(
            artifact,
            artifact_path,
            options,
            errors,
        )
        generated_times.extend(row_times)
        deployment_contexts.update(row_contexts)
    mismatched_counts = [
        kind_name
        for kind_name in sorted(expected_required_kinds)
        if expected_artifact_counts.get(kind_name, 0)
        != recognized_artifact_counts.get(kind_name, 0)
    ]
    if mismatched_counts:
        errors.append("recognized_artifacts must match required artifact counts")
    if any(count > 1 for count in recognized_artifact_path_counts.values()):
        errors.append("recognized_artifacts must not duplicate artifact paths")
    missing_identities = expected_artifact_identities - recognized_artifact_identities
    unexpected_identities = recognized_artifact_identities - expected_artifact_identities
    if missing_identities or unexpected_identities:
        errors.append("recognized_artifacts must match required artifact identities")
    return (
        generated_times,
        deployment_contexts,
        recognized_artifact_object_count,
        len(recognized_artifact_paths),
    )


def validate_aggregate_gate_row_output(
    gate: GateSummaryKind,
    row: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate the schema-closed aggregate lane row emitted for release review."""

    row_fields = set(row)
    missing_fields = AGGREGATE_REQUIRED_GATE_ROW_FIELDS - row_fields
    extra_fields = row_fields - AGGREGATE_REQUIRED_GATE_ROW_FIELDS
    if missing_fields or extra_fields:
        errors.append(
            f"{gate.name} aggregate row fields must match the schema-closed output contract"
        )
    for key in extra_fields:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{gate.name} aggregate row keys must be canonical strings")
        else:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{gate.name} aggregate row {key_diagnostic_label} is not allowed"
            )
    if row.get("schema") != gate.schema:
        errors.append(f"{gate.name} aggregate row schema must match gate schema")
    if row.get("present") is not True:
        errors.append(f"{gate.name} aggregate row present must be true")
    if row.get("valid") is not True:
        errors.append(f"{gate.name} aggregate row valid must be true")
    for field in (
        "required_kind_count",
        "expected_required_kind_count",
        "evidence_file_count",
        "recognized_artifact_count",
        "artifact_count",
        "oldest_generated_at_unix",
        "newest_generated_at_unix",
    ):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            errors.append(f"{gate.name} aggregate row {field} must be a positive integer")
    evidence_file_count = canonical_int_value(row.get("evidence_file_count"))
    recognized_artifact_count = canonical_int_value(
        row.get("recognized_artifact_count")
    )
    artifact_count = canonical_int_value(row.get("artifact_count"))
    if (
        evidence_file_count is not None
        and recognized_artifact_count is not None
        and evidence_file_count > 0
        and recognized_artifact_count > 0
        and evidence_file_count > recognized_artifact_count
    ):
        errors.append(
            f"{gate.name} aggregate row evidence_file_count must not exceed recognized_artifact_count"
        )
    if (
        recognized_artifact_count is not None
        and artifact_count is not None
        and recognized_artifact_count > 0
        and artifact_count > 0
        and recognized_artifact_count != artifact_count
    ):
        errors.append(
            f"{gate.name} aggregate row recognized_artifact_count must match artifact_count"
        )
    oldest = row.get("oldest_generated_at_unix")
    newest = row.get("newest_generated_at_unix")
    if (
        isinstance(oldest, int)
        and not isinstance(oldest, bool)
        and isinstance(newest, int)
        and not isinstance(newest, bool)
        and oldest > 0
        and newest > 0
        and newest < oldest
    ):
        errors.append(
            f"{gate.name} aggregate row newest_generated_at_unix must be >= oldest_generated_at_unix"
        )
    if row.get("required_kind_count") != len(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate row required_kind_count must match gate contract"
        )
    if row.get("expected_required_kind_count") != len(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate row expected_required_kind_count must match gate contract"
        )
    thresholds_errors: list[str] = []
    require_threshold_map(row, "thresholds", thresholds_errors)
    for threshold_error in thresholds_errors:
        errors.append(f"{gate.name} aggregate row {threshold_error}")
    require_production_deployment_id_value(
        row.get("deployment_id"),
        errors,
        f"{gate.name} aggregate row deployment_id",
    )
    if canonical_string(row.get("environment")) is None:
        errors.append(f"{gate.name} aggregate row environment must be canonical")
    elif not is_production_ready_environment(row.get("environment")):
        errors.append(f"{gate.name} aggregate row environment must be production")
    expected_required_kinds = row.get("expected_required_kinds")
    if list(gate.required_kinds) != expected_required_kinds:
        errors.append(
            f"{gate.name} aggregate row expected_required_kinds must match gate contract"
        )
    require_empty_error_list(
        row.get("errors"),
        f"{gate.name} aggregate row errors",
        errors,
    )
    if canonical_string(row.get("path")) is None:
        errors.append(f"{gate.name} aggregate row path must be canonical")
    elif not is_archive_portable_artifact_path(row["path"]):
        errors.append(
            f"{gate.name} aggregate row path must be archive-relative without "
            "absolute, empty, current, parent, or platform-specific segments"
        )
    sha256 = row.get("sha256")
    if (
        not isinstance(sha256, str)
        or len(sha256) != 64
        or any(character not in LOWER_HEX_DIGITS for character in sha256)
    ):
        errors.append(
            f"{gate.name} aggregate row sha256 must be canonical lowercase SHA-256"
        )


def validate_aggregate_row_error_list(
    value: Any,
    path: str,
    errors: list[str],
    *,
    require_non_empty: bool,
) -> None:
    """Validate canonical aggregate row diagnostics without requiring emptiness."""

    if not isinstance(value, list):
        errors.append(f"{path} must be a list")
        return
    if require_non_empty and not value:
        errors.append(f"{path} must not be empty")
    seen_errors: set[str] = set()
    for error in value:
        error_label = canonical_string(error)
        if error_label is None:
            errors.append(f"{path} must contain canonical strings")
            return
        if error_label in seen_errors:
            errors.append(f"{path} must not contain duplicate diagnostics")
            return
        seen_errors.add(error_label)


def validate_aggregate_required_row_output(
    gate: GateSummaryKind,
    row: Any,
    errors: list[str],
) -> None:
    """Validate final aggregate required-row shape before summary emission."""

    if not isinstance(row, dict):
        errors.append(f"{gate.name} aggregate required row must be an object")
        return
    present = row.get("present")
    valid = row.get("valid")
    expected_fields = (
        AGGREGATE_REQUIRED_GATE_ROW_FIELDS
        if present is True
        else AGGREGATE_MISSING_GATE_ROW_FIELDS
    )
    row_fields = set(row)
    missing_fields = expected_fields - row_fields
    extra_fields = row_fields - expected_fields
    if missing_fields or extra_fields:
        errors.append(
            f"{gate.name} aggregate required row fields must match the schema-closed output contract"
        )
    for key in extra_fields:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(
                f"{gate.name} aggregate required row keys must be canonical strings"
            )
        else:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{gate.name} aggregate required row {key_diagnostic_label} is not allowed"
            )
    if row.get("schema") != gate.schema:
        errors.append(f"{gate.name} aggregate required row schema must match gate schema")
    if not isinstance(present, bool):
        errors.append(f"{gate.name} aggregate required row present must be boolean")
    if not isinstance(valid, bool):
        errors.append(f"{gate.name} aggregate required row valid must be boolean")
    if present is False and valid is not False:
        errors.append(f"{gate.name} aggregate missing row valid must be false")
    if present is False:
        validate_aggregate_row_error_list(
            row.get("errors"),
            f"{gate.name} aggregate missing row errors",
            errors,
            require_non_empty=True,
        )
        if row.get("errors") != [
            f"missing required {gate.name} production readiness summary"
        ]:
            errors.append(
                f"{gate.name} aggregate missing row errors must match the deterministic missing summary diagnostic"
            )
        return
    if present is not True:
        return
    if valid is True:
        validate_aggregate_gate_row_output(gate, row, errors)
        return
    for field in (
        "evidence_file_count",
        "recognized_artifact_count",
        "artifact_count",
    ):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(
                f"{gate.name} aggregate invalid row {field} must be a non-negative integer"
            )
    evidence_file_count = canonical_int_value(row.get("evidence_file_count"))
    recognized_artifact_count = canonical_int_value(
        row.get("recognized_artifact_count")
    )
    artifact_count = canonical_int_value(row.get("artifact_count"))
    if (
        evidence_file_count is not None
        and recognized_artifact_count is not None
        and evidence_file_count >= 0
        and recognized_artifact_count >= 0
        and evidence_file_count > recognized_artifact_count
    ):
        errors.append(
            f"{gate.name} aggregate invalid row evidence_file_count must not exceed recognized_artifact_count"
        )
    if (
        recognized_artifact_count is not None
        and artifact_count is not None
        and recognized_artifact_count >= 0
        and artifact_count >= 0
        and recognized_artifact_count != artifact_count
    ):
        errors.append(
            f"{gate.name} aggregate invalid row recognized_artifact_count must match artifact_count"
        )
    for field in ("oldest_generated_at_unix", "newest_generated_at_unix"):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            errors.append(
                f"{gate.name} aggregate invalid row {field} must be a positive integer"
            )
    for field in ("required_kind_count", "expected_required_kind_count"):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            errors.append(
                f"{gate.name} aggregate invalid row {field} must be a positive integer"
            )
    if row.get("required_kind_count") != len(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate invalid row required_kind_count must match gate contract"
        )
    if row.get("expected_required_kind_count") != len(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate invalid row expected_required_kind_count must match gate contract"
        )
    oldest = row.get("oldest_generated_at_unix")
    newest = row.get("newest_generated_at_unix")
    if (
        isinstance(oldest, int)
        and not isinstance(oldest, bool)
        and isinstance(newest, int)
        and not isinstance(newest, bool)
        and newest < oldest
    ):
        errors.append(
            f"{gate.name} aggregate invalid row newest_generated_at_unix must be >= oldest_generated_at_unix"
        )
    thresholds = row.get("thresholds")
    if thresholds is not None:
        thresholds_errors: list[str] = []
        require_threshold_map(row, "thresholds", thresholds_errors)
        for threshold_error in thresholds_errors:
            errors.append(f"{gate.name} aggregate invalid row {threshold_error}")
    if (
        row.get("deployment_id") is not None
    ):
        require_production_deployment_id_value(
            row.get("deployment_id"),
            errors,
            f"{gate.name} aggregate invalid row deployment_id",
        )
    environment = row.get("environment")
    if environment is not None:
        if canonical_string(environment) is None:
            errors.append(
                f"{gate.name} aggregate invalid row environment must be canonical when present"
            )
        elif not is_production_ready_environment(environment):
            errors.append(
                f"{gate.name} aggregate invalid row environment must be production when present"
            )
    if row.get("expected_required_kinds") != list(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate invalid row expected_required_kinds must match gate contract"
        )
    if canonical_string(row.get("path")) is None:
        errors.append(f"{gate.name} aggregate invalid row path must be canonical")
    elif not is_archive_portable_artifact_path(row["path"]):
        errors.append(
            f"{gate.name} aggregate invalid row path must be archive-relative without "
            "absolute, empty, current, parent, or platform-specific segments"
        )
    sha256 = row.get("sha256")
    if (
        not isinstance(sha256, str)
        or len(sha256) != 64
        or any(character not in LOWER_HEX_DIGITS for character in sha256)
    ):
        errors.append(
            f"{gate.name} aggregate invalid row sha256 must be canonical lowercase SHA-256"
        )
    validate_aggregate_row_error_list(
        row.get("errors"),
        f"{gate.name} aggregate invalid row errors",
        errors,
        require_non_empty=True,
    )


def validate_aggregate_foundational_prerequisite_output(
    row: Any,
    errors: list[str],
) -> None:
    """Validate the payload-free foundational row emitted by the aggregate gate."""

    path = "aggregate foundational prerequisites"
    if not isinstance(row, dict):
        errors.append(f"{path} must be an object")
        return
    present = row.get("present")
    valid = row.get("valid")
    expected_fields = (
        AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS
        if present is True
        else AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS
    )
    row_fields = set(row)
    if row_fields != expected_fields:
        errors.append(f"{path} fields must match the schema-closed output contract")
    for key in row:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{path} keys must be canonical strings")
        elif key_label not in expected_fields:
            errors.append(
                f"{path} {payload_free_diagnostic_key_label(key_label)} is not allowed"
            )
    if row.get("schema") != FOUNDATIONAL_PREREQUISITE_SCHEMA:
        errors.append(f"{path} schema must match the prerequisite contract")
    if not isinstance(present, bool):
        errors.append(f"{path} present must be boolean")
    if not isinstance(valid, bool):
        errors.append(f"{path} valid must be boolean")
    if present is not True:
        if valid is not False:
            errors.append(f"{path} missing row valid must be false")
        validate_aggregate_row_error_list(
            row.get("errors"),
            f"{path} missing row errors",
            errors,
            require_non_empty=True,
        )
        return

    required_ids = row.get("required_ids")
    if required_ids != list(FOUNDATIONAL_PREREQUISITE_IDS):
        errors.append(f"{path} required_ids must match the exact prerequisite set")
    prerequisite_count = row.get("prerequisite_count")
    if (
        not isinstance(prerequisite_count, int)
        or isinstance(prerequisite_count, bool)
        or prerequisite_count < 0
    ):
        errors.append(f"{path} prerequisite_count must be a non-negative integer")
    elif valid is True and prerequisite_count != len(FOUNDATIONAL_PREREQUISITE_IDS):
        errors.append(f"{path} prerequisite_count must match the prerequisite set")

    timestamp_fields = (
        "generated_at_unix",
        "oldest_evidence_generated_at_unix",
        "newest_evidence_generated_at_unix",
    )
    for field in timestamp_fields:
        value = row.get(field)
        if value is None and valid is not True:
            continue
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            errors.append(f"{path} {field} must be a positive integer")
    oldest = row.get("oldest_evidence_generated_at_unix")
    newest = row.get("newest_evidence_generated_at_unix")
    generated = row.get("generated_at_unix")
    if (
        isinstance(oldest, int)
        and not isinstance(oldest, bool)
        and isinstance(newest, int)
        and not isinstance(newest, bool)
        and newest < oldest
    ):
        errors.append(f"{path} newest evidence timestamp must be >= oldest")
    if (
        isinstance(newest, int)
        and not isinstance(newest, bool)
        and isinstance(generated, int)
        and not isinstance(generated, bool)
        and newest > generated
    ):
        errors.append(f"{path} evidence timestamps must not exceed envelope time")

    deployment_id = row.get("deployment_id")
    environment = row.get("environment")
    if deployment_id is not None or valid is True:
        require_production_deployment_id_value(
            deployment_id,
            errors,
            f"{path} deployment_id",
        )
    if environment is not None or valid is True:
        if canonical_string(environment) is None:
            errors.append(f"{path} environment must be canonical")
        elif not is_production_ready_environment(environment):
            errors.append(f"{path} environment must be production")

    release_sequence = row.get("release_sequence")
    if release_sequence is not None or valid is True:
        if (
            not isinstance(release_sequence, int)
            or isinstance(release_sequence, bool)
            or release_sequence <= 0
            or release_sequence > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
        ):
            errors.append(f"{path} release_sequence must be in 1..2^63-1")
    previous_digest = row.get("previous_envelope_sha256")
    if previous_digest is not None or valid is True:
        if canonical_lower_hex(previous_digest, 64) is None:
            errors.append(
                f"{path} previous_envelope_sha256 must be canonical lowercase SHA-256"
            )
    signer_fingerprint = row.get("signer_public_key_fingerprint_sha256")
    if signer_fingerprint is not None or valid is True:
        if canonical_lower_hex(signer_fingerprint, 64) is None:
            errors.append(
                f"{path} signer fingerprint must be canonical lowercase SHA-256"
            )

    anchors = row.get("evidence_anchor_sha256")
    if not isinstance(anchors, list):
        errors.append(f"{path} evidence anchors must be a list")
    else:
        canonical_anchors = [
            anchor
            for anchor in anchors
            if canonical_lower_hex(anchor, 64) is not None
            and any(bytes.fromhex(anchor))
        ]
        if len(canonical_anchors) != len(anchors):
            errors.append(f"{path} evidence anchors must be non-zero lowercase SHA-256")
        if len(set(canonical_anchors)) != len(canonical_anchors):
            errors.append(f"{path} evidence anchors must be unique")
        if valid is True and len(anchors) != len(FOUNDATIONAL_PREREQUISITE_IDS):
            errors.append(f"{path} evidence anchors must cover every prerequisite")

    lane_summary_rows = validate_foundational_lane_summary_rows(
        row.get("lane_summary_sha256"),
        errors,
    )
    if valid is True and len(lane_summary_rows) != len(DEFAULT_REQUIRED_GATES):
        errors.append(f"{path} lane summary digests must cover every readiness lane")

    artifact_path = row.get("path")
    if canonical_string(artifact_path) is None:
        errors.append(f"{path} path must be canonical")
    elif not is_archive_portable_artifact_path(artifact_path):
        errors.append(f"{path} path must be archive-relative and portable")
    if canonical_lower_hex(row.get("sha256"), 64) is None:
        errors.append(f"{path} sha256 must be canonical lowercase SHA-256")

    validate_aggregate_row_error_list(
        row.get("errors"),
        f"{path} errors",
        errors,
        require_non_empty=valid is not True,
    )
    if valid is True and row.get("errors") != []:
        errors.append(f"{path} valid row errors must be empty")


def validate_aggregate_summary_output(
    summary: dict[str, Any],
    required_gates: tuple[str, ...],
    errors: list[str],
) -> None:
    """Validate the schema-closed aggregate summary envelope before writing it."""

    summary_fields = set(summary)
    missing_fields = AGGREGATE_SUMMARY_FIELDS - summary_fields
    extra_fields = summary_fields - AGGREGATE_SUMMARY_FIELDS
    if missing_fields or extra_fields:
        errors.append(
            "aggregate summary fields must match the schema-closed output contract"
        )
    for key in extra_fields:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append("aggregate summary keys must be canonical strings")
        else:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(f"aggregate summary {key_diagnostic_label} is not allowed")
    if summary.get("schema") != SUMMARY_SCHEMA:
        errors.append("aggregate summary schema must match production readiness schema")
    if summary.get("status") not in {"ready", "failed", "blocked"}:
        errors.append("aggregate summary status must be ready, failed, or blocked")
    required_gates_value = summary.get("required_gates")
    if not isinstance(required_gates_value, list):
        errors.append("aggregate summary required_gates must be a list")
    else:
        seen_required_gates: set[str] = set()
        emitted_required_gate_diagnostics: set[str] = set()
        for gate_name in required_gates_value:
            gate_name_label = canonical_string(gate_name)
            if gate_name_label is None:
                diagnostic = (
                    "aggregate summary required_gates must contain canonical strings"
                )
                if diagnostic not in emitted_required_gate_diagnostics:
                    errors.append(diagnostic)
                    emitted_required_gate_diagnostics.add(diagnostic)
                continue
            if gate_name_label in seen_required_gates:
                diagnostic = (
                    "aggregate summary required_gates must not contain duplicate gates"
                )
                if diagnostic not in emitted_required_gate_diagnostics:
                    errors.append(diagnostic)
                    emitted_required_gate_diagnostics.add(diagnostic)
            else:
                seen_required_gates.add(gate_name_label)
            if gate_name_label not in GATE_BY_NAME:
                diagnostic = (
                    "aggregate summary required_gates must use known gate names"
                )
                if diagnostic not in emitted_required_gate_diagnostics:
                    errors.append(diagnostic)
                    emitted_required_gate_diagnostics.add(diagnostic)
    if list(required_gates) != required_gates_value:
        errors.append("aggregate summary required_gates must match requested gates")
    thresholds_errors: list[str] = []
    thresholds = require_threshold_map(summary, "thresholds", thresholds_errors)
    if "max_summary_artifact_age_secs" not in thresholds:
        thresholds_errors.append("thresholds.max_summary_artifact_age_secs must be present")
    if thresholds and set(thresholds) != {"max_summary_artifact_age_secs"}:
        thresholds_errors.append(
            "thresholds must contain only max_summary_artifact_age_secs"
        )
    for threshold_error in thresholds_errors:
        errors.append(f"aggregate summary {threshold_error}")
    for field in ("summary_file_count", "recognized_summary_count"):
        value = summary.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(f"aggregate summary {field} must be a non-negative integer")
    summary_file_count = summary.get("summary_file_count")
    recognized_summary_count = summary.get("recognized_summary_count")
    if (
        isinstance(summary_file_count, int)
        and not isinstance(summary_file_count, bool)
        and isinstance(recognized_summary_count, int)
        and not isinstance(recognized_summary_count, bool)
        and recognized_summary_count > summary_file_count
    ):
        errors.append(
            "aggregate summary recognized_summary_count must not exceed summary_file_count"
        )
    if (
        isinstance(recognized_summary_count, int)
        and not isinstance(recognized_summary_count, bool)
        and recognized_summary_count > len(required_gates)
    ):
        errors.append(
            "aggregate summary recognized_summary_count must not exceed required gate count"
        )
    deployment = summary.get("deployment")
    allowed_deployment_fields = {"deployment_id", "environment"}
    aggregate_deployment_id = None
    aggregate_environment = None
    if not isinstance(deployment, dict):
        errors.append("aggregate summary deployment must be an object")
    else:
        deployment_fields = set(deployment)
        if deployment_fields and deployment_fields != allowed_deployment_fields:
            errors.append(
                "aggregate summary deployment fields must be deployment_id and environment"
            )
        for key in deployment:
            key_label = canonical_string(key)
            if key_label is None:
                errors.append("aggregate summary deployment keys must be canonical strings")
            elif key_label not in allowed_deployment_fields:
                key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
                errors.append(
                    f"aggregate summary deployment {key_diagnostic_label} is not allowed"
                )
        if deployment_fields == allowed_deployment_fields:
            aggregate_deployment_id = require_production_deployment_id_value(
                deployment.get("deployment_id"),
                errors,
                "aggregate summary deployment_id",
            )
            aggregate_environment = canonical_string(deployment.get("environment"))
            if aggregate_environment is None:
                errors.append("aggregate summary environment must be a canonical string")
            elif not is_production_ready_environment(aggregate_environment):
                errors.append("aggregate summary environment must be production")
    required = summary.get("required")
    if not isinstance(required, dict):
        errors.append("aggregate summary required must be an object")
    else:
        if set(required) != set(required_gates):
            errors.append("aggregate summary required rows must match requested gates")
        if (
            isinstance(recognized_summary_count, int)
            and not isinstance(recognized_summary_count, bool)
            and recognized_summary_count
            != sum(
                1
                for row in required.values()
                if isinstance(row, dict) and row.get("present") is True
            )
        ):
            errors.append(
                "aggregate summary recognized_summary_count must match present required rows"
            )
        for gate_name, row in required.items():
            gate_name_label = canonical_string(gate_name)
            if gate_name_label is None:
                errors.append("aggregate summary required row keys must be canonical strings")
            elif gate_name_label not in GATE_BY_NAME:
                errors.append("aggregate summary required rows must use known gate names")
            if not isinstance(row, dict):
                errors.append("aggregate summary required rows must be objects")
            elif gate_name_label in GATE_BY_NAME:
                validate_aggregate_required_row_output(
                    GATE_BY_NAME[gate_name_label],
                    row,
                    errors,
                )
                if row.get("present") is True:
                    row_deployment_id = canonical_string(row.get("deployment_id"))
                    if (
                        aggregate_deployment_id is not None
                        and row_deployment_id is not None
                        and row_deployment_id != aggregate_deployment_id
                    ):
                        errors.append(
                            f"{gate_name_label} aggregate required row deployment_id must match aggregate deployment_id"
                        )
                    row_environment = canonical_string(row.get("environment"))
                    if (
                        aggregate_environment is not None
                        and row_environment is not None
                        and row_environment != aggregate_environment
                    ):
                        errors.append(
                            f"{gate_name_label} aggregate required row environment must match aggregate environment"
                        )
    foundational_prerequisites = summary.get("foundational_prerequisites")
    validate_aggregate_foundational_prerequisite_output(
        foundational_prerequisites,
        errors,
    )
    if isinstance(foundational_prerequisites, dict):
        foundational_deployment_id = canonical_string(
            foundational_prerequisites.get("deployment_id")
        )
        foundational_environment = canonical_string(
            foundational_prerequisites.get("environment")
        )
        if (
            aggregate_deployment_id is not None
            and foundational_deployment_id is not None
            and foundational_deployment_id != aggregate_deployment_id
        ):
            errors.append(
                "aggregate foundational prerequisite deployment_id must match aggregate deployment_id"
            )
        if (
            aggregate_environment is not None
            and foundational_environment is not None
            and foundational_environment != aggregate_environment
        ):
            errors.append(
                "aggregate foundational prerequisite environment must match aggregate environment"
            )
    if summary.get("status") == "ready":
        if not isinstance(deployment, dict) or set(deployment) != allowed_deployment_fields:
            errors.append(
                "aggregate summary ready deployment must include deployment_id and environment"
            )
        if (
            isinstance(summary_file_count, int)
            and not isinstance(summary_file_count, bool)
            and summary_file_count != len(required_gates)
        ):
            errors.append(
                "aggregate summary ready summary_file_count must match required gate count"
            )
        if (
            isinstance(recognized_summary_count, int)
            and not isinstance(recognized_summary_count, bool)
            and recognized_summary_count != len(required_gates)
        ):
            errors.append(
                "aggregate summary ready recognized_summary_count must match required gate count"
            )
        if not isinstance(required, dict) or any(
            not isinstance(row, dict)
            or row.get("present") is not True
            or row.get("valid") is not True
            for row in required.values()
        ):
            errors.append("aggregate summary ready rows must all be present and valid")
        if (
            not isinstance(foundational_prerequisites, dict)
            or foundational_prerequisites.get("present") is not True
            or foundational_prerequisites.get("valid") is not True
        ):
            errors.append(
                "aggregate summary ready foundational prerequisites must be present and valid"
            )
    error_values = summary.get("errors")
    if not isinstance(error_values, list):
        errors.append("aggregate summary errors must be a list")
    else:
        seen_errors: set[str] = set()
        for error in error_values:
            error_label = canonical_string(error)
            if error_label is None:
                errors.append("aggregate summary errors must contain canonical strings")
                break
            if error_label in seen_errors:
                errors.append(
                    "aggregate summary errors must not contain duplicate diagnostics"
                )
                break
            seen_errors.add(error_label)
        if summary.get("status") != evidence_gate_status(error_values):
            errors.append("aggregate summary status must match aggregate diagnostics")


def validate_gate_summary(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[dict[str, Any], list[str]]:
    """Validate one existing lane-gate summary."""

    errors: list[str] = []
    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys=SENSITIVE_KEYS,
        evidence_label="SoraFS production readiness summary",
    )
    require_payload_free_summary_fields(payload, errors)
    validate_payload_free_summary_metadata(
        gate,
        payload,
        errors,
        enforce_production_deployment_context=is_production_ready_environment(
            options.environment
        ),
    )
    require_string_field(payload, "schema", errors)
    if payload.get("schema") != gate.schema:
        errors.append(f"schema must be `{gate.schema}`")
    if payload.get("status") != "ready":
        errors.append("status must be `ready`")
    require_empty_error_list(payload.get("errors"), "errors", errors)
    require_absent_or_empty_error_list(payload, "load_errors", errors)
    thresholds = require_threshold_map(payload, "thresholds", errors)
    evidence_file_count = require_positive_int_field(
        payload,
        "evidence_file_count",
        errors,
    )
    recognized_artifact_count = require_positive_int_field(
        payload,
        "recognized_artifact_count",
        errors,
    )

    required_kinds_raw = payload.get("required_kinds")
    if not isinstance(required_kinds_raw, list) or not required_kinds_raw:
        errors.append("required_kinds must be a non-empty array")
        required_kinds: list[str] = []
    else:
        required_kinds = []
        for index, kind_name in enumerate(required_kinds_raw):
            kind_label = canonical_string(kind_name)
            if kind_label is None:
                errors.append(f"required_kinds[{index}] must be canonical")
                continue
            if kind_label in required_kinds:
                errors.append("required_kinds contains duplicate kind")
                continue
            required_kinds.append(kind_label)
    expected_required_kinds = list(gate.required_kinds)
    if required_kinds != expected_required_kinds:
        missing = sorted(set(expected_required_kinds) - set(required_kinds))
        extra = sorted(set(required_kinds) - set(expected_required_kinds))
        errors.append(
            f"required_kinds must match the full `{gate.name}` gate contract"
        )
        if missing:
            errors.append("required_kinds missing full-contract kinds")
        if extra:
            errors.append("required_kinds contains unknown full-contract kinds")

    required = payload.get("required")
    if not isinstance(required, dict):
        errors.append("required must be an object")
        required = {}
    else:
        noncanonical_required_rows = [
            row_label for row_label in required if canonical_string(row_label) is None
        ]
        if noncanonical_required_rows:
            errors.append("required row labels must be canonical strings")
        extra_required_rows = set(required) - set(expected_required_kinds)
        if extra_required_rows:
            errors.append(
                f"required contains rows outside the full `{gate.name}` gate contract"
            )

    artifact_total = 0
    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    required_kind_schemas = GATE_REQUIRED_KIND_SCHEMAS.get(gate.name, {})
    expected_required_kind_set = set(expected_required_kinds)
    for kind_name in required_kinds:
        if kind_name not in expected_required_kind_set:
            continue
        artifact_count, row_times, row_contexts = validate_required_row(
            gate.name,
            kind_name,
            required_kind_schemas.get(kind_name),
            required.get(kind_name),
            options,
            errors,
        )
        artifact_total += artifact_count
        generated_times.extend(row_times)
        deployment_contexts.update(row_contexts)

    if (
        recognized_artifact_count is not None
        and recognized_artifact_count != artifact_total
    ):
        errors.append(
            "recognized_artifact_count must match required row artifact total"
        )
    if (
        evidence_file_count is not None
        and recognized_artifact_count is not None
        and evidence_file_count > recognized_artifact_count
    ):
        errors.append(
            "evidence_file_count must not exceed recognized_artifact_count"
        )
    (
        recognized_times,
        recognized_contexts,
        recognized_artifact_object_count,
        recognized_artifact_path_count,
    ) = validate_recognized_artifacts(
        gate,
        payload,
        required,
        recognized_artifact_count,
        options,
        errors,
    )
    generated_times.extend(recognized_times)
    deployment_contexts.update(recognized_contexts)
    validate_payload_free_summary_metadata_fingerprint_tethers(
        gate,
        payload,
        errors,
    )
    validate_payload_free_object_list_metadata_counts(
        gate,
        payload,
        required,
        errors,
    )
    deployment_contexts.update(
        payload_free_summary_metadata_deployment_contexts(gate, payload)
    )
    if (
        evidence_file_count is not None
        and recognized_artifact_path_count is not None
        and evidence_file_count != recognized_artifact_path_count
    ):
        errors.append("evidence_file_count must match recognized artifact path count")

    if len(deployment_contexts) > 1:
        errors.append(
            f"{gate.name} deployment context must match across artifacts and metadata"
        )
    deployment_id = None
    environment = None
    if len(deployment_contexts) == 1:
        deployment_id, environment = next(iter(deployment_contexts))
        if options.deployment_id is not None and deployment_id != options.deployment_id:
            errors.append(f"{gate.name} deployment_id must match --deployment-id")
        if options.environment is not None and environment != options.environment:
            errors.append(f"{gate.name} environment must match --environment")
    summary_recognized_artifact_count = (
        recognized_artifact_object_count
        if recognized_artifact_object_count is not None
        else recognized_artifact_count
    )

    summary = {
        "schema": gate.schema,
        "present": True,
        "valid": not errors,
        "required_kind_count": len(required_kinds),
        "expected_required_kind_count": len(gate.required_kinds),
        "evidence_file_count": evidence_file_count,
        "recognized_artifact_count": summary_recognized_artifact_count,
        "artifact_count": artifact_total,
        "thresholds": thresholds,
        "oldest_generated_at_unix": min(generated_times) if generated_times else None,
        "newest_generated_at_unix": max(generated_times) if generated_times else None,
        "deployment_id": deployment_id,
        "environment": environment,
        "expected_required_kinds": expected_required_kinds,
        "errors": errors,
    }
    return summary, errors


def validate_duplicate_summary_diagnostics(
    required: dict[str, dict[str, Any]],
    duplicate_summary_gates: set[str],
    duplicate_summary_count: int,
    errors: list[str],
) -> None:
    """Pin deterministic duplicate-summary diagnostics in final aggregate rows."""

    counted_duplicate_errors = 0
    for gate_name in sorted(duplicate_summary_gates):
        row = required.get(gate_name)
        diagnostic = f"duplicate {gate_name} production readiness summary"
        counted_duplicate_errors += errors.count(diagnostic)
        if not isinstance(row, dict):
            errors.append(f"{gate_name} duplicate summary row must be an object")
            continue
        row_errors = row.get("errors")
        if not isinstance(row_errors, list) or row_errors.count(diagnostic) != 1:
            errors.append(
                f"{gate_name} duplicate summary row errors must contain the deterministic duplicate summary diagnostic exactly once"
            )
    if counted_duplicate_errors != duplicate_summary_count:
        errors.append(
            "aggregate summary duplicate-summary diagnostics must match duplicate summary inputs"
        )


def validate_disallowed_summary_diagnostics(
    errors: list[str],
    *,
    unknown_schema_count: int,
    explicit_unrequired_count: int,
) -> None:
    """Pin aggregate blockers for disallowed summary inputs."""

    if errors.count("unknown SoraFS readiness summary schema") != unknown_schema_count:
        errors.append(
            "aggregate summary unknown-schema diagnostics must match discovered unknown summaries"
        )
    if (
        errors.count("explicit production readiness summary belongs to unrequired gate")
        != explicit_unrequired_count
    ):
        errors.append(
            "aggregate summary unrequired-gate diagnostics must match explicit unrequired summaries"
        )


def validate_foundational_lane_summary_digest_bindings(
    foundational_prerequisites: dict[str, Any],
    observed_summary_sha256: dict[str, str],
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
        and canonical_string(row.get("gate")) is not None
        and canonical_lower_hex(row.get("sha256"), 64) is not None
    }
    row_errors = foundational_prerequisites.setdefault("errors", [])
    if not isinstance(row_errors, list):
        return
    for gate_name in DEFAULT_REQUIRED_GATES:
        observed = observed_summary_sha256.get(gate_name)
        if observed is None:
            continue
        if expected_by_gate.get(gate_name) == observed:
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


def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_gates: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    """Build the aggregate production-readiness summary."""

    errors: list[str] = []
    files = discover_evidence_files(
        evidence_dirs,
        evidence_files,
        errors,
        reserved_output_paths=() if summary_out is None else (summary_out,),
    )
    explicit = evidence_path_identities(evidence_files, errors)
    required: dict[str, dict[str, Any]] = {
        name: {
            "schema": GATE_BY_NAME[name].schema,
            "present": False,
            "valid": False,
            "errors": [f"missing required {name} production readiness summary"],
        }
        for name in required_gates
    }
    foundational_prerequisites: dict[str, Any] = {
        "schema": FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "present": False,
        "valid": False,
        "errors": ["missing required foundational prerequisite summary"],
    }
    summary_contexts: set[tuple[str, str]] = set()
    recognized_summaries = 0
    foundational_file_count = 0
    duplicate_foundational_summary = False
    duplicate_summary_gates: set[str] = set()
    duplicate_summary_count = 0
    unknown_schema_count = 0
    explicit_unrequired_count = 0
    observed_summary_sha256: dict[str, str] = {}

    for path in files:
        loaded = load_evidence_json_with_sha256_or_record_error(
            path,
            MAX_SUMMARY_BYTES,
            errors,
        )
        if loaded is None:
            continue
        payload, digest = loaded
        schema = payload.get("schema")
        if schema == FOUNDATIONAL_PREREQUISITE_SCHEMA:
            foundational_file_count += 1
            if foundational_prerequisites.get("present") is True:
                if not duplicate_foundational_summary:
                    duplicate_foundational_summary = True
                    duplicate_error = (
                        "duplicate foundational prerequisite summary"
                    )
                    foundational_prerequisites["valid"] = False
                    row_errors = foundational_prerequisites.setdefault("errors", [])
                    if duplicate_error not in row_errors:
                        row_errors.append(duplicate_error)
                    errors.append(duplicate_error)
                continue
            (
                foundation_summary,
                foundation_errors,
                foundation_context,
            ) = validate_foundational_prerequisite_summary(payload, options)
            foundation_summary["path"] = aggregate_summary_path_label(
                path,
                evidence_dirs,
            )
            foundation_summary["sha256"] = digest
            if foundation_summary.get("valid") is True:
                foundation_output_errors: list[str] = []
                validate_aggregate_foundational_prerequisite_output(
                    foundation_summary,
                    foundation_output_errors,
                )
                if foundation_output_errors:
                    foundation_summary["valid"] = False
                    foundation_summary["errors"].extend(foundation_output_errors)
                    foundation_errors.extend(foundation_output_errors)
            foundational_prerequisites = foundation_summary
            for error in foundation_errors:
                errors.append(f"foundational prerequisites: {error}")
            if foundation_context is not None:
                summary_contexts.add(foundation_context)
            continue
        gate = SCHEMA_TO_GATE.get(schema)
        if gate is None:
            unknown_schema_count += 1
            errors.append("unknown SoraFS readiness summary schema")
            continue
        if gate.name not in required:
            if is_explicit_evidence_path(path, explicit, errors):
                explicit_unrequired_count += 1
                errors.append(
                    "explicit production readiness summary belongs to unrequired gate"
                )
            continue
        if required[gate.name]["present"] is True:
            duplicate_summary_gates.add(gate.name)
            duplicate_summary_count += 1
            required[gate.name]["valid"] = False
            duplicate_error = f"duplicate {gate.name} production readiness summary"
            row_errors = required[gate.name].setdefault("errors", [])
            if duplicate_error not in row_errors:
                row_errors.append(duplicate_error)
            errors.append(duplicate_error)
            continue
        recognized_summaries += 1
        observed_summary_sha256[gate.name] = digest
        gate_summary, validation_errors = validate_gate_summary(gate, payload, options)
        gate_summary["path"] = aggregate_summary_path_label(path, evidence_dirs)
        gate_summary["sha256"] = digest
        if gate_summary.get("valid") is True:
            row_output_errors: list[str] = []
            validate_aggregate_gate_row_output(gate, gate_summary, row_output_errors)
            if row_output_errors:
                gate_summary["valid"] = False
                gate_summary["errors"].extend(row_output_errors)
                validation_errors.extend(row_output_errors)
        required[gate.name] = gate_summary
        for error in validation_errors:
            errors.append(f"{gate.name}: {error}")
        deployment_id = gate_summary.get("deployment_id")
        environment = gate_summary.get("environment")
        if isinstance(deployment_id, str) and isinstance(environment, str):
            summary_contexts.add((deployment_id, environment))

    validate_foundational_lane_summary_digest_bindings(
        foundational_prerequisites,
        observed_summary_sha256,
        required_gates,
        errors,
    )

    for name, row in required.items():
        if row.get("present") is False:
            errors.extend(row.get("errors", []))
        elif row.get("valid") is not True:
            errors.append(f"{name} production readiness summary is invalid")
    if foundational_prerequisites.get("present") is False:
        errors.extend(foundational_prerequisites.get("errors", []))
    elif foundational_prerequisites.get("valid") is not True:
        errors.append("foundational prerequisite summary is invalid")

    if options.deployment_id is None or options.environment is None:
        errors.append(
            "aggregate production readiness requires --deployment-id and --environment"
        )

    validate_duplicate_summary_diagnostics(
        required,
        duplicate_summary_gates,
        duplicate_summary_count,
        errors,
    )
    validate_disallowed_summary_diagnostics(
        errors,
        unknown_schema_count=unknown_schema_count,
        explicit_unrequired_count=explicit_unrequired_count,
    )

    if len(summary_contexts) > 1:
        errors.append("production readiness deployment context must match across gates")
    deployment: dict[str, str] = {}
    if len(summary_contexts) == 1:
        deployment_id, environment = next(iter(summary_contexts))
        deployment = {"deployment_id": deployment_id, "environment": environment}
        if options.deployment_id is not None and deployment_id != options.deployment_id:
            errors.append("aggregate deployment_id must match --deployment-id")
        if options.environment is not None and environment != options.environment:
            errors.append("aggregate environment must match --environment")
        require_production_deployment_id_value(
            deployment_id,
            errors,
            "aggregate deployment_id",
        )
        if not is_production_ready_environment(environment):
            errors.append("aggregate environment must be production")

    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": evidence_gate_status(errors),
        "required_gates": list(required_gates),
        "thresholds": {
            "max_summary_artifact_age_secs": options.max_summary_artifact_age_secs,
        },
        "summary_file_count": len(files) - foundational_file_count,
        "recognized_summary_count": recognized_summaries,
        "deployment": deployment,
        "foundational_prerequisites": foundational_prerequisites,
        "required": required,
        "errors": errors,
    }
    validate_aggregate_summary_output(summary, required_gates, errors)
    summary["status"] = evidence_gate_status(errors)
    return summary, errors


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Parse aggregate readiness checker arguments."""

    parser = EvidenceArgumentParser(
        description="Validate aggregate SoraFS production-readiness summaries.",
    )
    parser.add_argument(
        "--evidence-dir",
        action="append",
        type=Path,
        default=[],
        help="Directory containing per-lane readiness summary JSON files.",
    )
    parser.add_argument(
        "--evidence",
        action="append",
        type=Path,
        default=[],
        help="Explicit per-lane readiness summary JSON file.",
    )
    parser.add_argument(
        "--require-gate",
        action="append",
        default=[],
        help="Required aggregate gate name, or comma-separated names. Defaults to every SoraFS gate.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        required=True,
        help="Required reviewed validator clock used for artifact age checks.",
    )
    parser.add_argument(
        "--max-summary-artifact-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    )
    parser.add_argument(
        "--deployment-id",
        help=(
            "Required final deployment id shared by every lane summary artifact "
            "before production readiness can pass."
        ),
    )
    parser.add_argument(
        "--environment",
        help=(
            "Required final prod/production environment shared by every lane "
            "summary artifact before production readiness can pass."
        ),
    )
    parser.add_argument(
        "--foundational-prerequisite-signer-public-key-hex",
        dest="foundational_signer_public_key_hex",
        help=(
            "Required operator-trusted 32-byte Ed25519 public key for the signed "
            "foundational prerequisite envelope. The key is runtime-only input "
            "and is not copied into the aggregate summary."
        ),
    )
    parser.add_argument(
        "--foundational-prerequisite-release-sequence",
        dest="foundational_release_sequence",
        type=positive_int_arg,
        help=(
            "Required operator-reviewed monotonic release sequence expected in "
            "the foundational prerequisite envelope."
        ),
    )
    parser.add_argument(
        "--foundational-prerequisite-previous-envelope-sha256",
        dest="foundational_previous_envelope_sha256",
        help=(
            "Required operator-reviewed lowercase SHA-256 of the preceding "
            "foundational envelope (all zeroes only for sequence 1)."
        ),
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error
    return parser.parse_args(expanded_args)


def main(argv: list[str] | None = None) -> int:
    """Run the aggregate SoraFS production-readiness checker."""

    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    try:
        required_gates = parse_required_gates(
            args.require_gate,
            allowed_kinds=GATE_BY_NAME,
            default_required=DEFAULT_REQUIRED_GATES,
        )
    except ValueError as error:
        emit_checker_exception(error)
        return 2

    if args.deployment_id is not None:
        deployment_id_errors: list[str] = []
        require_production_deployment_id_value(
            args.deployment_id,
            deployment_id_errors,
            "--deployment-id",
        )
        if deployment_id_errors:
            emit_checker_error_lines(deployment_id_errors)
            return 2
    if args.environment is not None and canonical_string(args.environment) is None:
        emit_checker_error_lines(["--environment must be a non-empty canonical string"])
        return 2
    if args.environment is not None and not is_production_ready_environment(
        args.environment
    ):
        emit_checker_error_lines(["--environment must be production for this gate"])
        return 2

    foundational_signer_public_key: bytes | None = None
    if args.foundational_signer_public_key_hex is not None:
        foundational_key_errors: list[str] = []
        foundational_signer_public_key = parse_foundational_signer_public_key(
            args.foundational_signer_public_key_hex,
            foundational_key_errors,
            path="--foundational-prerequisite-signer-public-key-hex",
        )
        if foundational_key_errors:
            emit_checker_error_lines(foundational_key_errors)
            return 2
    foundational_previous_envelope_sha256 = None
    if (
        args.foundational_release_sequence is not None
        and args.foundational_release_sequence > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
    ):
        emit_checker_error_lines(
            [
                "--foundational-prerequisite-release-sequence must be in 1..2^63-1"
            ]
        )
        return 2
    if args.foundational_previous_envelope_sha256 is not None:
        foundational_previous_envelope_sha256 = canonical_lower_hex(
            args.foundational_previous_envelope_sha256,
            64,
        )
        if foundational_previous_envelope_sha256 is None:
            emit_checker_error_lines(
                [
                    "--foundational-prerequisite-previous-envelope-sha256 must be canonical lowercase SHA-256"
                ]
            )
            return 2

    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_summary_artifact_age_secs=args.max_summary_artifact_age_secs,
        deployment_id=args.deployment_id,
        environment=args.environment,
        foundational_signer_public_key=foundational_signer_public_key,
        foundational_release_sequence=args.foundational_release_sequence,
        foundational_previous_envelope_sha256=(
            foundational_previous_envelope_sha256
        ),
    )
    summary, errors = build_summary(
        args.evidence_dir,
        args.evidence,
        required_gates,
        options,
        args.summary_out,
    )
    _, render_errors = render_and_write_checker_summary(args.summary_out, summary)
    if render_errors:
        emit_checker_error_lines(render_errors)
        return 2
    if errors:
        emit_checker_error_block("SoraFS production readiness is blocked:", errors)
        return 1
    emit_checker_notice(
        "SoraFS production readiness validated for "
        f"{len(required_gates)} required gate(s) and "
        f"{len(FOUNDATIONAL_PREREQUISITE_IDS)} foundational prerequisite(s)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
