#!/usr/bin/env python3
"""Validate aggregate SoraFS production-readiness gate summaries."""

from __future__ import annotations

import argparse
import re
import sys
import time
from collections import Counter
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
    numbered_rollout_marker_token,
    require_rollout_deployment_id,
)
from sorafs_path_identity import resolve_path_identity  # noqa: E402
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
)
from check_sorafs_appeal_finance_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as APPEAL_FINANCE_REQUIRED_KINDS,
    KIND_BY_NAME as APPEAL_FINANCE_KIND_BY_NAME,
)
from check_sorafs_gateway_compliance_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as GATEWAY_COMPLIANCE_REQUIRED_KINDS,
    KIND_BY_NAME as GATEWAY_COMPLIANCE_KIND_BY_NAME,
)
from check_sorafs_gateway_load_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as GATEWAY_LOAD_REQUIRED_KINDS,
    KIND_BY_NAME as GATEWAY_LOAD_KIND_BY_NAME,
)
from check_sorafs_governance_dag_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as GOVERNANCE_DAG_REQUIRED_KINDS,
    KIND_BY_NAME as GOVERNANCE_DAG_KIND_BY_NAME,
)
from check_sorafs_hedging_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as HEDGING_BILLING_REQUIRED_KINDS,
    KIND_BY_NAME as HEDGING_BILLING_KIND_BY_NAME,
)
from check_sorafs_moderation_panel_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as MODERATION_PANEL_REQUIRED_KINDS,
    KIND_BY_NAME as MODERATION_PANEL_KIND_BY_NAME,
)
from check_sorafs_orderbook_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as ORDERBOOK_REQUIRED_KINDS,
    KIND_BY_NAME as ORDERBOOK_KIND_BY_NAME,
)
from check_sorafs_pdp_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as PDP_REQUIRED_KINDS,
    KIND_BY_NAME as PDP_KIND_BY_NAME,
)
from check_sorafs_pop_credentials_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as POP_CREDENTIALS_REQUIRED_KINDS,
    KIND_BY_NAME as POP_CREDENTIALS_KIND_BY_NAME,
)
from check_sorafs_por_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as POR_REQUIRED_KINDS,
    KIND_BY_NAME as POR_KIND_BY_NAME,
)
from check_sorafs_potr_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as POTR_REQUIRED_KINDS,
    KIND_BY_NAME as POTR_KIND_BY_NAME,
)
from check_sorafs_reference_sdk_release_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as REFERENCE_SDK_REQUIRED_KINDS,
    KIND_BY_NAME as REFERENCE_SDK_KIND_BY_NAME,
)
from check_sorafs_repair_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as REPAIR_REQUIRED_KINDS,
    KIND_BY_NAME as REPAIR_KIND_BY_NAME,
)
from check_sorafs_reputation_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as REPUTATION_REQUIRED_KINDS,
    KIND_BY_NAME as REPUTATION_KIND_BY_NAME,
)
from check_sorafs_reserve_rent_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as RESERVE_RENT_REQUIRED_KINDS,
    KIND_BY_NAME as RESERVE_RENT_KIND_BY_NAME,
)
from check_sorafs_transparency_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as TRANSPARENCY_REQUIRED_KINDS,
    KIND_BY_NAME as TRANSPARENCY_KIND_BY_NAME,
)


SUMMARY_SCHEMA = "sorafs.production_readiness.aggregate_gate.v1"
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

GATE_METADATA_FIELDS: dict[str, frozenset[str]] = {
    "ai_prescreen": frozenset(
        {
            "valid_executor_summary_digests",
            "valid_notification_manifest_digests",
            "valid_policy_digests",
            "valid_runner_bindings",
            "valid_workflow_digests",
        }
    ),
    "appeal_finance": frozenset(
        {"valid_config_digests", "valid_multi_peer_runs", "valid_policy_digests"}
    ),
    "gateway_compliance": frozenset({"valid_bundle_digests", "valid_policy_digests"}),
    "gateway_load": frozenset(
        {
            "valid_policy_digests",
            "valid_staging_report_digests",
            "valid_suite_report_digests",
        }
    ),
    "governance_dag": frozenset(
        {
            "valid_checkpoint_digests",
            "valid_policy_digests",
            "valid_public_head_cids",
        }
    ),
    "hedging_billing": frozenset(
        {
            "valid_billing_cycles",
            "valid_cycle_bindings",
            "valid_policy_digests",
            "valid_reference_decision_ids",
        }
    ),
    "moderation_panel": frozenset(
        {
            "deployment_context",
            "valid_case_digests",
            "valid_e2e_runs",
            "valid_evidence_viewer_digest_sets",
            "valid_policy_digests",
            "valid_roster_bindings",
            "valid_tally_bindings",
        }
    ),
    "orderbook": frozenset({"valid_contract_digests", "valid_policy_digests"}),
    "pdp": frozenset(
        {
            "valid_policy_digests",
            "valid_proof_summary_digests",
            "valid_provider_roster_digests",
        }
    ),
    "pop_credentials": frozenset(
        {
            "valid_juror_sync_bindings",
            "valid_policy_digests",
            "valid_pop_snapshot_digests",
            "valid_revocation_list_digests",
            "valid_root_digests",
        }
    ),
    "por": frozenset({"valid_policy_digests", "valid_seed_replay_digests"}),
    "potr": frozenset(
        {
            "valid_policy_digests",
            "valid_pq_key_roster_digests",
            "valid_receipt_summary_digests",
            "valid_reputation_weight_policy_digests",
        }
    ),
    "reference_sdk_release": frozenset(
        {
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
            "valid_failure_bundle_digests",
            "valid_handoff_digests",
            "valid_policy_digests",
            "valid_roster_digests",
        }
    ),
    "reputation": frozenset(
        {
            "merkle_root_hex",
            "provider_count_values",
            "provider_ids",
            "snapshot_id_hex",
            "valid_snapshot_bindings",
        }
    ),
    "reserve_rent": frozenset(
        {
            "valid_policy_digests",
            "valid_policy_matrix_bindings",
            "valid_policy_matrix_ledger_bindings",
            "valid_provider_bakes",
        }
    ),
    "transparency": frozenset({"valid_cycle_digests", "valid_source_batch_digests"}),
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
) | frozenset({"provider_count_values", "provider_ids"})
PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS = frozenset(
    {
        "valid_bundle_digests",
        "valid_case_digests",
        "valid_checkpoint_digests",
        "valid_config_digests",
        "valid_contract_digests",
        "valid_cycle_digests",
        "valid_executor_summary_digests",
        "valid_failure_bundle_digests",
        "valid_handoff_digests",
        "valid_notification_manifest_digests",
        "valid_policy_digests",
        "valid_pop_snapshot_digests",
        "valid_pq_key_roster_digests",
        "valid_proof_summary_digests",
        "valid_provider_roster_digests",
        "valid_public_head_cids",
        "valid_receipt_summary_digests",
        "valid_reputation_weight_policy_digests",
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
    {"provider_count_values"}
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
            {"completed_at_unix", "provider_count", "started_at_unix"}
        ),
        "hex": {
            "ledger_digest_hex": 64,
            "matrix_digest_hex": 64,
            "policy_digest_hex": 64,
        },
        "ordered_int_pairs": (("started_at_unix", "completed_at_unix"),),
    },
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
    "valid_bundle_digests": "bundle_digest_hex",
    "valid_case_digests": "case_digest_hex",
    "valid_checkpoint_digests": "checkpoint_digest_hex",
    "valid_config_digests": "config_digest_hex",
    "valid_contract_digests": "contract_digest_hex",
    "valid_cycle_digests": "cycle_digest_hex",
    "valid_executor_summary_digests": "execution_summary_digest_hex",
    "valid_failure_bundle_digests": "evidence_bundle_digest_hex",
    "valid_handoff_digests": "handoff_digest_hex",
    "valid_notification_manifest_digests": "manifest_body_blake3",
    "valid_policy_digests": "policy_digest_hex",
    "valid_pop_snapshot_digests": "pop_snapshot_digest_hex",
    "valid_pq_key_roster_digests": "pq_key_roster_digest_hex",
    "valid_proof_summary_digests": "proof_summary_digest_hex",
    "valid_provider_roster_digests": "provider_roster_digest_hex",
    "valid_public_head_cids": "public_head_cid_hex",
    "valid_receipt_summary_digests": "receipt_summary_digest_hex",
    "valid_reputation_weight_policy_digests": "reputation_weight_policy_digest_hex",
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
    ("gateway_compliance", "valid_bundle_digests"): ("feed_promotion",),
    ("gateway_compliance", "valid_policy_digests"): ("feed_promotion",),
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
    ("pop_credentials", "valid_policy_digests"): ("verifier_service",),
    ("pop_credentials", "valid_pop_snapshot_digests"): ("moderation_integration",),
    ("pop_credentials", "valid_revocation_list_digests"): (
        "issuer_bundle",
        "revocation_registry",
    ),
    ("pop_credentials", "valid_root_digests"): ("issuer_bundle", "commitment_root"),
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
    "provider_ids": "provider_id",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS = {
    ("reputation", "provider_ids"): ("provider",),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS = {
    "provider_count_values": "provider_count",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS = {
    ("reputation", "provider_count_values"): ("publish", "latest"),
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
}
PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS = {
    "deployment_context": frozenset({"deployment_id", "environment"}),
}
PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS = (
    PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS
    | frozenset(PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS)
    | PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS
    | frozenset({"provider_ids"})
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


def canonical_string(value: Any) -> str | None:
    """Return a non-empty canonical string, or None."""

    if (
        isinstance(value, str)
        and value.strip()
        and value == value.strip()
        and not any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        return value
    return None


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
    tokens = [
        token for token in re.split(r"[._-]+", deployment_id.lower()) if token
    ]
    forbidden = sorted(
        {
            token
            for token in set(tokens)
            if token in FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS
        }
        | {
            marker
            for token in tokens
            if (
                marker := numbered_rollout_marker_token(
                    token,
                    FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS,
                )
            )
            is not None
        }
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


def is_archive_portable_artifact_path(path: str) -> bool:
    """Return whether an artifact label is portable inside release archives."""

    if canonical_string(path) is None:
        return False
    if path.startswith(("/", "\\")) or "\\" in path:
        return False
    if len(path) >= 2 and path[1] == ":" and path[0].isalpha():
        return False
    parts = path.split("/")
    return all(
        canonical_string(part) is not None and part not in {".", ".."}
        for part in parts
    )


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
            "current, parent, or platform-specific segments"
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
    for index, item in enumerate(value):
        item_path = f"{field}[{index}]"
        if not isinstance(item, dict):
            errors.append(f"{item_path} must be a payload-free metadata object")
            continue
        identity = payload_free_object_list_metadata_identity(item, schema)
        if identity is not None:
            identities.append(identity)
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


def validate_payload_free_summary_metadata(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    errors: list[str],
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
        object_list_schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.get(field)
        if object_list_schema is not None:
            validate_payload_free_object_list_metadata(
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
        if field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS:
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
            continue
        errors.append(f"{field} validator is not configured for `{gate.name}`")


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
    artifact: dict[str, Any],
    path: str,
    errors: list[str],
) -> int | None:
    """Return an artifact generation timestamp from its fingerprint."""

    fingerprint = artifact.get("fingerprint")
    if not isinstance(fingerprint, dict):
        errors.append(f"{path}.fingerprint must be an object")
        return None
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
) -> None:
    """Require artifact fingerprints to carry only payload-free metadata."""

    fingerprint = artifact.get("fingerprint")
    if not isinstance(fingerprint, dict):
        return
    validate_payload_free_metadata_value(fingerprint, f"{path}.fingerprint", errors)


def artifact_deployment_context(
    artifact: dict[str, Any],
    path: str,
    errors: list[str],
) -> tuple[str, str] | None:
    """Return the deployment context recorded in an artifact fingerprint."""

    fingerprint = artifact.get("fingerprint")
    if not isinstance(fingerprint, dict):
        errors.append(f"{path}.fingerprint must be an object")
        return None
    deployment_id = canonical_string(fingerprint.get("deployment_id"))
    environment = canonical_string(fingerprint.get("environment"))
    reviewed = fingerprint.get("deployment_context_reviewed")
    if deployment_id is None:
        errors.append(f"{path}.fingerprint.deployment_id must be canonical")
    if environment is None:
        errors.append(f"{path}.fingerprint.environment must be canonical")
    if reviewed is not True:
        errors.append(f"{path}.fingerprint.deployment_context_reviewed must be true")
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
    if status is not None and status not in SUCCESS_ARTIFACT_STATUSES:
        errors.append(f"{path}.status must be a successful status")
    if artifact.get("valid") is not True:
        errors.append(f"{path}.valid must be true")
    require_empty_error_list(
        artifact.get("errors"),
        f"{path}.errors",
        errors,
    )
    validate_payload_free_artifact_fingerprint(artifact, path, errors)
    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    generated_at = artifact_generated_at(artifact, path, errors)
    if generated_at is not None:
        if generated_at > options.now_unix:
            errors.append(f"{path}.generated_at_unix must not be future")
        elif (
            options.now_unix - generated_at
            > options.max_summary_artifact_age_secs
        ):
            errors.append(
                f"{path}.generated_at_unix exceeds max summary artifact age"
            )
        generated_times.append(generated_at)
    context = artifact_deployment_context(artifact, path, errors)
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
    validate_payload_free_summary_metadata(gate, payload, errors)
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
    summary_contexts: set[tuple[str, str]] = set()
    recognized_summaries = 0
    duplicate_summary_gates: set[str] = set()
    duplicate_summary_count = 0
    unknown_schema_count = 0
    explicit_unrequired_count = 0

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

    for name, row in required.items():
        if row.get("present") is False:
            errors.extend(row.get("errors", []))
        elif row.get("valid") is not True:
            errors.append(f"{name} production readiness summary is invalid")

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
        "summary_file_count": len(files),
        "recognized_summary_count": recognized_summaries,
        "deployment": deployment,
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
        default=int(time.time()),
        help="Validator clock used for artifact age checks. Defaults to current Unix time.",
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

    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_summary_artifact_age_secs=args.max_summary_artifact_age_secs,
        deployment_id=args.deployment_id,
        environment=args.environment,
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
        f"{len(required_gates)} required gate(s)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
