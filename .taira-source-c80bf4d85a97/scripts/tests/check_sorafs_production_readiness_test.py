"""Tests for scripts/check_sorafs_production_readiness.py."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_production_readiness.py"
SPEC = importlib.util.spec_from_file_location("check_sorafs_production_readiness", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

import sccp_release_common as RELEASE_CRYPTO  # noqa: E402

NOW_UNIX = 1_800_800_000
GENERATED_AT = NOW_UNIX - 120
SHA256 = "ab" * 32
PREDECESSOR_CATALOG_SHA256 = "ac" * 32
SNAPSHOT_ID = "cd" * 16
DEPLOYMENT_ID = "sorafs-mainnet-2026-06"
ENVIRONMENT = "production"
FOUNDATIONAL_RELEASE_SEQUENCE = 7
FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256 = hashlib.sha256(
    b"test-only-foundational-predecessor"
).hexdigest()
FOUNDATIONAL_SIGNING_SEED = bytes.fromhex("1f" * 32)


def ed25519_public_key_from_seed(seed: bytes) -> bytes:
    """Derive a deterministic test-only Ed25519 public key."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    return RELEASE_CRYPTO._ed_encode(  # noqa: SLF001 - deterministic test fixture
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            scalar,
        )
    )


def ed25519_sign(seed: bytes, message: bytes) -> bytes:
    """Create a deterministic test-only Ed25519 signature."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    prefix = digest[32:]
    public_key = ed25519_public_key_from_seed(seed)
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little")
    nonce %= RELEASE_CRYPTO._ED_L  # noqa: SLF001
    encoded_r = RELEASE_CRYPTO._ed_encode(  # noqa: SLF001
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            nonce,
        )
    )
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(),
        "little",
    ) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    encoded_s = (
        (nonce + challenge * scalar) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    ).to_bytes(32, "little")
    return encoded_r + encoded_s


FOUNDATIONAL_SIGNER_PUBLIC_KEY = ed25519_public_key_from_seed(
    FOUNDATIONAL_SIGNING_SEED
)
ORIGINAL_VERIFY_ED25519 = MODULE.verify_ed25519
FOUNDATIONAL_SIGNATURE_VERIFICATION_CACHE: dict[
    tuple[bytes, bytes, bytes], bool
] = {}


def cached_verify_ed25519(
    public_key: bytes,
    signature: bytes,
    message: bytes,
) -> bool:
    """Avoid repeating the same expensive pure-Python verification in fixtures."""

    key = (public_key, signature, hashlib.sha256(message).digest())
    if key not in FOUNDATIONAL_SIGNATURE_VERIFICATION_CACHE:
        FOUNDATIONAL_SIGNATURE_VERIFICATION_CACHE[key] = ORIGINAL_VERIFY_ED25519(
            public_key,
            signature,
            message,
        )
    return FOUNDATIONAL_SIGNATURE_VERIFICATION_CACHE[key]


MODULE.verify_ed25519 = cached_verify_ed25519


def resign_foundational_summary(
    payload: dict,
    *,
    seed: bytes = FOUNDATIONAL_SIGNING_SEED,
) -> None:
    """Refresh the test envelope fingerprint and signature after a mutation."""

    public_key = ed25519_public_key_from_seed(seed)
    payload.setdefault("signature", {})[
        "public_key_fingerprint_sha256"
    ] = hashlib.sha256(public_key).hexdigest()
    payload["signature"]["signature_hex"] = "00" * 64
    payload["signature"]["signature_hex"] = ed25519_sign(
        seed,
        MODULE.foundational_signing_payload(payload),
    ).hex()


def foundational_summary(
    *,
    generated_at_unix: int = GENERATED_AT,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    lane_summary_sha256: dict[str, str] | None = None,
) -> dict:
    """Return a complete signed foundational prerequisite envelope."""

    lane_summary_sha256 = lane_summary_sha256 or {
        gate_name: hashlib.sha256(
            f"{gate_name}:reviewed-lane-summary".encode("ascii")
        ).hexdigest()
        for gate_name in MODULE.DEFAULT_REQUIRED_GATES
    }
    payload = {
        "schema": MODULE.FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "status": "verified",
        "deployment": {
            "deployment_id": deployment_id,
            "environment": environment,
        },
        "generated_at_unix": generated_at_unix,
        "release_sequence": FOUNDATIONAL_RELEASE_SEQUENCE,
        "previous_envelope_sha256": FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256,
        "prerequisites": [
            {
                "id": prerequisite_id,
                "status": "verified",
                "evidence_anchor_sha256": hashlib.sha256(
                    f"{prerequisite_id}:production-evidence".encode("ascii")
                ).hexdigest(),
                "evidence_generated_at_unix": generated_at_unix - 1,
            }
            for prerequisite_id in MODULE.FOUNDATIONAL_PREREQUISITE_IDS
        ],
        "lane_summaries": [
            {
                "gate": gate_name,
                "sha256": lane_summary_sha256[gate_name],
            }
            for gate_name in MODULE.DEFAULT_REQUIRED_GATES
        ],
        "signature": {
            "algorithm": "ed25519",
            "public_key_fingerprint_sha256": "00" * 32,
            "signature_hex": "00" * 64,
        },
    }
    resign_foundational_summary(payload)
    return payload


def lane_summary_digests(root: Path) -> dict[str, str]:
    """Return exact lane-summary byte digests present below one evidence root."""

    digests = {
        gate_name: hashlib.sha256(
            f"{gate_name}:reviewed-lane-summary".encode("ascii")
        ).hexdigest()
        for gate_name in MODULE.DEFAULT_REQUIRED_GATES
    }
    for path in sorted(root.rglob("*.json")):
        if path.name.startswith("foundational_prerequisite"):
            continue
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError):
            continue
        gate = MODULE.SCHEMA_TO_GATE.get(payload.get("schema"))
        if gate is not None:
            digests[gate.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    return digests


def write_foundational_summary(root: Path, payload: dict | None = None) -> Path:
    """Write the signed prerequisite fixture when it is not already present."""

    path = root / "foundational_prerequisites.json"
    if payload is not None or not path.exists():
        write_json(
            path,
            (
                foundational_summary(
                    lane_summary_sha256=lane_summary_digests(root),
                )
                if payload is None
                else payload
            ),
        )
    return path


def foundational_cli_args() -> list[str]:
    """Return the reviewed trust/continuity arguments for fixture envelopes."""

    return [
        "--foundational-prerequisite-signer-public-key-hex",
        FOUNDATIONAL_SIGNER_PUBLIC_KEY.hex(),
        "--foundational-prerequisite-release-sequence",
        str(FOUNDATIONAL_RELEASE_SEQUENCE),
        "--foundational-prerequisite-previous-envelope-sha256",
        FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256,
    ]


def production_validation_options(**overrides: object):
    """Return aggregate options including reviewed foundational trust anchors."""

    values = {
        "now_unix": NOW_UNIX,
        "max_summary_artifact_age_secs": (
            MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS
        ),
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "foundational_signer_public_key": FOUNDATIONAL_SIGNER_PUBLIC_KEY,
        "foundational_release_sequence": FOUNDATIONAL_RELEASE_SEQUENCE,
        "foundational_previous_envelope_sha256": (
            FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256
        ),
    }
    values.update(overrides)
    return MODULE.ValidationOptions(**values)


LANE_FIXTURE_TESTS = {
    "ai_prescreen": "check_sorafs_ai_prescreen_rollout_evidence_test.py",
    "appeal_finance": "check_sorafs_appeal_finance_rollout_evidence_test.py",
    "gateway_compliance": "check_sorafs_gateway_compliance_rollout_evidence_test.py",
    "gateway_load": "check_sorafs_gateway_load_rollout_evidence_test.py",
    "governance_dag": "check_sorafs_governance_dag_rollout_evidence_test.py",
    "hedging_billing": "check_sorafs_hedging_rollout_evidence_test.py",
    "moderation_panel": "check_sorafs_moderation_panel_rollout_evidence_test.py",
    "orderbook": "check_sorafs_orderbook_rollout_evidence_test.py",
    "pdp": "check_sorafs_pdp_rollout_evidence_test.py",
    "pop_credentials": "check_sorafs_pop_credentials_rollout_evidence_test.py",
    "por": "check_sorafs_por_rollout_evidence_test.py",
    "potr": "check_sorafs_potr_rollout_evidence_test.py",
    "reference_sdk_release": "check_sorafs_reference_sdk_release_evidence_test.py",
    "repair": "check_sorafs_repair_rollout_evidence_test.py",
    "reputation": "check_sorafs_reputation_rollout_evidence_test.py",
    "reserve_rent": "check_sorafs_reserve_rent_rollout_evidence_test.py",
    "transparency": "check_sorafs_transparency_rollout_evidence_test.py",
}


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def default_gate_metadata(
    gate_name: str,
    *,
    generated_at_unix: int,
    deployment_id: str,
    environment: str,
) -> tuple[dict[str, object], dict[str, object]]:
    metadata: dict[str, object] = {}
    fingerprints: dict[str, object] = {}

    def add_hex_list(field: str, fingerprint_field: str, value: str = SHA256) -> None:
        metadata[field] = [value]
        fingerprints[fingerprint_field] = value

    def add_policy() -> None:
        add_hex_list("valid_policy_digests", "policy_digest_hex")

    if gate_name == "ai_prescreen":
        metadata["deployment_context"] = {
            "deployment_id": deployment_id,
            "environment": environment,
        }
        metadata["valid_runner_bindings"] = [
            {
                "manifest_id_hex": "12" * 16,
                "runner_hash_hex": SHA256,
                "subject_digest_hex": SHA256,
            }
        ]
        fingerprints.update(
            {
                "manifest_id_hex": "12" * 16,
                "runner_hash_hex": SHA256,
                "subject_digest_hex": SHA256,
            }
        )
        add_hex_list("valid_executor_summary_digests", "execution_summary_digest_hex")
        add_hex_list("valid_notification_manifest_digests", "manifest_body_blake3_hex")
        add_hex_list("valid_workflow_digests", "workflow_digest_hex")
        add_policy()
    elif gate_name == "appeal_finance":
        metadata["metric_count_values"] = [len(MODULE.APPEAL_FINANCE_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.APPEAL_FINANCE_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.APPEAL_FINANCE_REQUIRED_METRICS),
                "metrics": list(MODULE.APPEAL_FINANCE_REQUIRED_METRICS),
            }
        )
        add_hex_list("valid_config_digests", "config_digest_hex")
        add_policy()
        metadata["valid_multi_peer_runs"] = [
            {
                "case_count": 1,
                "config_digest_hex": SHA256,
                "deployment_id": deployment_id,
                "environment": environment,
                "generated_at_unix": generated_at_unix,
                "peer_count": 4,
                "validator_count": 4,
            }
        ]
        fingerprints.update({"case_count": 1, "peer_count": 4, "validator_count": 4})
    elif gate_name == "gateway_compliance":
        metadata["metric_count_values"] = [
            len(MODULE.GATEWAY_COMPLIANCE_REQUIRED_METRICS)
        ]
        metadata["metrics"] = sorted(MODULE.GATEWAY_COMPLIANCE_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.GATEWAY_COMPLIANCE_REQUIRED_METRICS),
                "metrics": list(MODULE.GATEWAY_COMPLIANCE_REQUIRED_METRICS),
            }
        )
        add_hex_list("valid_catalog_digests", "catalog_digest_hex")
        metadata["valid_catalog_history_bindings"] = [
            {
                "catalog_digest_hex": SHA256,
                "catalog_sequence": 2,
                "predecessor_catalog_digest_hex": PREDECESSOR_CATALOG_SHA256,
                "predecessor_catalog_sequence": 1,
            }
        ]
        fingerprints.update(
            {
                "catalog_sequence": 2,
                "predecessor_catalog_digest_hex": PREDECESSOR_CATALOG_SHA256,
                "predecessor_catalog_sequence": 1,
            }
        )
        add_policy()
    elif gate_name == "gateway_load":
        metadata["metric_count_values"] = [len(MODULE.GATEWAY_LOAD_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.GATEWAY_LOAD_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.GATEWAY_LOAD_REQUIRED_METRICS),
                "metrics": list(MODULE.GATEWAY_LOAD_REQUIRED_METRICS),
            }
        )
        add_policy()
        add_hex_list("valid_staging_report_digests", "staging_report_digest_hex")
        add_hex_list("valid_suite_report_digests", "suite_report_digest_hex")
    elif gate_name == "governance_dag":
        metadata["metric_count_values"] = [len(MODULE.GOVERNANCE_DAG_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.GOVERNANCE_DAG_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.GOVERNANCE_DAG_REQUIRED_METRICS),
                "metrics": list(MODULE.GOVERNANCE_DAG_REQUIRED_METRICS),
            }
        )
        add_hex_list("valid_checkpoint_digests", "checkpoint_digest_hex")
        add_policy()
        add_hex_list("valid_public_head_cids", "public_head_cid_hex")
    elif gate_name == "hedging_billing":
        metadata["metric_count_values"] = [len(MODULE.HEDGING_BILLING_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.HEDGING_BILLING_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.HEDGING_BILLING_REQUIRED_METRICS),
                "metrics": list(MODULE.HEDGING_BILLING_REQUIRED_METRICS),
            }
        )
        metadata["valid_billing_cycles"] = [
            {
                "cycle_id": "billing-cycle-1",
                "cycle_index": 1,
                "deployment_id": deployment_id,
                "environment": environment,
                "generated_at_unix": generated_at_unix,
                "policy_digest_hex": SHA256,
                "reconciliation_digest_hex": SHA256,
                "reference_decision_id_hex": SHA256,
                "statement_bundle_digest_hex": SHA256,
                "statement_count": 1,
            }
        ]
        metadata["valid_cycle_bindings"] = [
            {
                "statement_bundle_digest_hex": SHA256,
                "reconciliation_digest_hex": SHA256,
            }
        ]
        fingerprints.update(
            {
                "cycle_id": "billing-cycle-1",
                "cycle_index": 1,
                "policy_digest_hex": SHA256,
                "reference_decision_id_hex": SHA256,
                "statement_bundle_digest_hex": SHA256,
                "reconciliation_digest_hex": SHA256,
                "statement_count": 1,
            }
        )
        add_policy()
        add_hex_list("valid_reference_decision_ids", "decision_id_hex")
    elif gate_name == "moderation_panel":
        metadata["metric_count_values"] = [len(MODULE.MODERATION_PANEL_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.MODERATION_PANEL_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.MODERATION_PANEL_REQUIRED_METRICS),
                "metrics": list(MODULE.MODERATION_PANEL_REQUIRED_METRICS),
            }
        )
        metadata["deployment_context"] = {
            "deployment_id": deployment_id,
            "environment": environment,
        }
        add_hex_list("valid_case_digests", "case_digest_hex")
        metadata["valid_e2e_runs"] = [
            {
                "case_count": 1,
                "case_digest_hex": SHA256,
                "deployment_id": deployment_id,
                "environment": environment,
                "generated_at_unix": generated_at_unix,
                "peer_count": 4,
                "roster_hash_hex": SHA256,
                "tally_digest_hex": SHA256,
                "validator_count": 4,
            }
        ]
        metadata["valid_evidence_viewer_digest_sets"] = [
            {
                "catalog_digest_hex": SHA256,
                "case_digest_hex": SHA256,
                "roster_hash_hex": SHA256,
                "session_manifest_digest_hex": SHA256,
                "watermark_metadata_digest_hex": SHA256,
                "access_log_digest_hex": SHA256,
                "legal_hold_receipt_digest_hex": SHA256,
                "transparency_report_digest_hex": SHA256,
                "audit_digest_hex": SHA256,
            }
        ]
        add_policy()
        metadata["valid_roster_bindings"] = [
            {"case_digest_hex": SHA256, "roster_hash_hex": SHA256}
        ]
        metadata["valid_tally_bindings"] = [
            {
                "case_digest_hex": SHA256,
                "roster_hash_hex": SHA256,
                "tally_digest_hex": SHA256,
            }
        ]
        fingerprints.update({"roster_hash_hex": SHA256, "tally_digest_hex": SHA256})
        fingerprints.update(
            {
                "catalog_digest_hex": SHA256,
                "session_manifest_digest_hex": SHA256,
                "watermark_metadata_digest_hex": SHA256,
                "access_log_digest_hex": SHA256,
                "legal_hold_receipt_digest_hex": SHA256,
                "transparency_report_digest_hex": SHA256,
                "audit_digest_hex": SHA256,
            }
        )
        fingerprints.update({"case_count": 1, "peer_count": 4, "validator_count": 4})
    elif gate_name == "orderbook":
        metadata["metric_count_values"] = [len(MODULE.ORDERBOOK_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.ORDERBOOK_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.ORDERBOOK_REQUIRED_METRICS),
                "metrics": list(MODULE.ORDERBOOK_REQUIRED_METRICS),
            }
        )
        add_hex_list("valid_contract_digests", "contract_digest_hex")
        add_policy()
    elif gate_name == "pdp":
        metadata["metric_count_values"] = [len(MODULE.PDP_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.PDP_REQUIRED_METRICS)
        metadata["valid_repair_handoff_digests"] = [SHA256]
        fingerprints.update(
            {
                "metric_count": len(MODULE.PDP_REQUIRED_METRICS),
                "metrics": list(MODULE.PDP_REQUIRED_METRICS),
                "repair_handoff_digest_hex": SHA256,
            }
        )
        add_policy()
        add_hex_list("valid_proof_summary_digests", "proof_summary_digest_hex")
        add_hex_list("valid_provider_roster_digests", "provider_roster_digest_hex")
    elif gate_name == "pop_credentials":
        metadata["metric_count_values"] = [len(MODULE.POP_CREDENTIALS_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.POP_CREDENTIALS_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.POP_CREDENTIALS_REQUIRED_METRICS),
                "metrics": list(MODULE.POP_CREDENTIALS_REQUIRED_METRICS),
            }
        )
        metadata["valid_juror_sync_bindings"] = [
            {
                "synced_root_digest_hex": SHA256,
                "synced_revocation_list_digest_hex": SHA256,
            }
        ]
        fingerprints.update(
            {
                "synced_root_digest_hex": SHA256,
                "synced_revocation_list_digest_hex": SHA256,
            }
        )
        add_policy()
        add_hex_list("valid_pop_snapshot_digests", "pop_snapshot_digest_hex")
        add_hex_list("valid_revocation_list_digests", "revocation_list_digest_hex")
        add_hex_list("valid_root_digests", "root_digest_hex")
    elif gate_name == "por":
        metadata["archive_backends"] = ["parquet"]
        metadata["valid_governance_archive_handoff_digests"] = [SHA256]
        metadata["metric_count_values"] = [len(MODULE.POR_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.POR_REQUIRED_METRICS)
        fingerprints.update(
            {
                "archive_backend": "parquet",
                "governance_archive_handoff_digest_hex": SHA256,
                "metric_count": len(MODULE.POR_REQUIRED_METRICS),
                "metrics": list(MODULE.POR_REQUIRED_METRICS),
            }
        )
        add_policy()
        add_hex_list("valid_seed_replay_digests", "seed_replay_digest_hex")
    elif gate_name == "potr":
        metadata["metric_count_values"] = [len(MODULE.POTR_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.POTR_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.POTR_REQUIRED_METRICS),
                "metrics": list(MODULE.POTR_REQUIRED_METRICS),
            }
        )
        add_policy()
        add_hex_list("valid_pq_key_roster_digests", "pq_key_roster_digest_hex")
        add_hex_list("valid_receipt_summary_digests", "receipt_summary_digest_hex")
        add_hex_list(
            "valid_reputation_weight_policy_digests",
            "reputation_weight_policy_digest_hex",
        )
    elif gate_name == "reference_sdk_release":
        metadata["signature_algorithms"] = ["ed25519"]
        add_hex_list("valid_archive_index_digests", "archive_index_digest_hex")
        add_hex_list("valid_ffi_contract_digests", "ffi_contract_digest_hex")
        add_hex_list("valid_header_digests", "header_digest_hex")
        add_hex_list("valid_package_index_digests", "package_index_digest_hex")
        add_policy()
        add_hex_list(
            "valid_release_key_fingerprints",
            "public_key_fingerprint_hex",
        )
        add_hex_list("valid_release_manifest_digests", "manifest_digest_hex")
        add_hex_list(
            "valid_release_manifest_reference_digests",
            "release_manifest_digest_hex",
        )
        add_hex_list("valid_smoke_output_digests", "smoke_output_digest_hex")
        fingerprints["signature_algorithm"] = "ed25519"
    elif gate_name == "repair":
        metadata["metric_count_values"] = [len(MODULE.REPAIR_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.REPAIR_REQUIRED_METRICS)
        fingerprints.update(
            {
                "metric_count": len(MODULE.REPAIR_REQUIRED_METRICS),
                "metrics": list(MODULE.REPAIR_REQUIRED_METRICS),
            }
        )
        add_hex_list("valid_failure_bundle_digests", "evidence_bundle_digest_hex")
        add_hex_list("valid_handoff_digests", "handoff_digest_hex")
        add_policy()
        add_hex_list("valid_roster_digests", "roster_digest_hex")
    elif gate_name == "reputation":
        metadata["metric_count_values"] = [len(MODULE.REPUTATION_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.REPUTATION_REQUIRED_METRICS)
        metadata["merkle_root_hex"] = SHA256
        metadata["provider_count_values"] = [1]
        metadata["provider_ids"] = ["provider-a"]
        metadata["snapshot_id_hex"] = SNAPSHOT_ID
        metadata["valid_reputation_weight_digests"] = [SHA256]
        metadata["valid_snapshot_bindings"] = [
            {"snapshot_id_hex": SNAPSHOT_ID, "merkle_root_hex": SHA256}
        ]
        fingerprints.update(
            {
                "merkle_root_hex": SHA256,
                "metric_count": len(MODULE.REPUTATION_REQUIRED_METRICS),
                "metrics": list(MODULE.REPUTATION_REQUIRED_METRICS),
                "provider_count": 1,
                "provider_id": "provider-a",
                "snapshot_id_hex": SNAPSHOT_ID,
                "weights_digest_hex": SHA256,
            }
        )
    elif gate_name == "reserve_rent":
        metadata["metric_count_values"] = [len(MODULE.RESERVE_RENT_REQUIRED_METRICS)]
        metadata["metrics"] = sorted(MODULE.RESERVE_RENT_REQUIRED_METRICS)
        add_policy()
        metadata["valid_policy_matrix_bindings"] = [
            {"policy_digest_hex": SHA256, "matrix_digest_hex": SHA256}
        ]
        metadata["valid_policy_matrix_ledger_bindings"] = [
            {
                "policy_digest_hex": SHA256,
                "matrix_digest_hex": SHA256,
                "ledger_digest_hex": SHA256,
            }
        ]
        metadata["valid_provider_bakes"] = [
            {
                "bake_id": "reserve-bake-001",
                "completed_at_unix": generated_at_unix,
                "deployment_id": deployment_id,
                "environment": environment,
                "ledger_digest_hex": SHA256,
                "matrix_digest_hex": SHA256,
                "policy_digest_hex": SHA256,
                "provider_count": 1,
                "scheduled_lifecycle_canary_defaulted_provider_count": 1,
                "scheduled_lifecycle_canary_last_tick_at_unix": generated_at_unix - 30,
                "scheduled_lifecycle_canary_tick_count": 2,
                "started_at_unix": generated_at_unix - 60,
            }
        ]
        fingerprints.update(
            {
                "bake_id": "reserve-bake-001",
                "completed_at_unix": generated_at_unix,
                "ledger_digest_hex": SHA256,
                "matrix_digest_hex": SHA256,
                "metric_count": len(MODULE.RESERVE_RENT_REQUIRED_METRICS),
                "metrics": list(MODULE.RESERVE_RENT_REQUIRED_METRICS),
                "provider_count": 1,
                "scheduled_lifecycle_canary_defaulted_provider_count": 1,
                "scheduled_lifecycle_canary_last_tick_at_unix": generated_at_unix - 30,
                "scheduled_lifecycle_canary_tick_count": 2,
                "started_at_unix": generated_at_unix - 60,
            }
        )
    elif gate_name == "transparency":
        add_hex_list("valid_cycle_digests", "cycle_digest_hex")
        metadata["valid_publication_bindings"] = [
            {
                "source_batch_digest_hex": SHA256,
                "cycle_digest_hex": SHA256,
            }
        ]
        add_hex_list("valid_source_batch_digests", "source_batch_digest_hex")

    return metadata, fingerprints


def gate_summary(
    gate_name: str,
    *,
    status: str = "ready",
    errors: list[str] | None = None,
    generated_at_unix: int = GENERATED_AT,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    raw_response: bool = False,
    required_kinds: list[str] | None = None,
) -> dict:
    gate = MODULE.GATE_BY_NAME[gate_name]
    gate_required_kinds = (
        list(gate.required_kinds) if required_kinds is None else required_kinds
    )
    required_kind_schemas = MODULE.GATE_REQUIRED_KIND_SCHEMAS.get(gate_name, {})
    required_rows = {}
    for kind_name in gate_required_kinds:
        kind_schema = required_kind_schemas.get(kind_name, f"{gate.schema}.{kind_name}")
        required_rows[kind_name] = {
            "schema": kind_schema,
            "present": True,
            "valid": True,
            "artifact_count": 1,
            "artifacts": [
                {
                    "path": f"artifacts/{gate_name}/{kind_name}.json",
                    "sha256": SHA256,
                    "schema": kind_schema,
                    "status": "passed",
                    "fingerprint": {
                        "generated_at_unix": generated_at_unix,
                        "deployment_id": deployment_id,
                        "environment": environment,
                        "deployment_context_reviewed": True,
                    },
                    "valid": True,
                    "errors": [],
                }
            ],
            "errors": [],
        }
    payload = {
        "schema": gate.schema,
        "status": status,
        "required_kinds": gate_required_kinds,
        "thresholds": {"max_evidence_bytes": 2_097_152},
        "evidence_file_count": len(gate_required_kinds),
        "recognized_artifact_count": len(gate_required_kinds),
        "recognized_artifacts": recognized_artifacts_from_required(
            {"required": required_rows}
        ),
        "required": required_rows,
        "errors": [] if errors is None else errors,
    }
    metadata, fingerprint_metadata = default_gate_metadata(
        gate_name,
        generated_at_unix=generated_at_unix,
        deployment_id=deployment_id,
        environment=environment,
    )
    payload.update(metadata)
    for row in payload["required"].values():
        for artifact in row["artifacts"]:
            artifact["fingerprint"].update(fingerprint_metadata)
    for artifact in payload["recognized_artifacts"]:
        artifact["fingerprint"].update(fingerprint_metadata)
    if raw_response:
        payload["response_body"] = "leaked"
    return payload


def write_gate(root: Path, gate_name: str, **kwargs: object) -> Path:
    return write_json(root / f"{gate_name}.json", gate_summary(gate_name, **kwargs))


def write_all_gates(root: Path) -> None:
    for gate_name in MODULE.DEFAULT_REQUIRED_GATES:
        write_gate(root, gate_name)


def recognized_artifacts_from_required(payload: dict) -> list[dict]:
    artifacts = []
    for kind_name, row in payload["required"].items():
        for required_artifact in row["artifacts"]:
            artifact = dict(required_artifact)
            artifact["kind"] = kind_name
            artifacts.append(artifact)
    return artifacts


def add_fingerprint_metadata(
    payload: dict,
    *,
    kind_name: str | None = None,
    **metadata: object,
) -> None:
    for row_name, row in payload["required"].items():
        if kind_name is not None and row_name != kind_name:
            continue
        for artifact in row["artifacts"]:
            artifact.setdefault("fingerprint", {}).update(metadata)
    for artifact in payload["recognized_artifacts"]:
        if kind_name is not None and artifact.get("kind") != kind_name:
            continue
        artifact.setdefault("fingerprint", {}).update(metadata)


def remove_fingerprint_metadata(
    payload: dict,
    *field_names: str,
    kind_name: str | None = None,
) -> None:
    for row_name, row in payload["required"].items():
        if kind_name is not None and row_name != kind_name:
            continue
        for artifact in row["artifacts"]:
            fingerprint = artifact.setdefault("fingerprint", {})
            for field_name in field_names:
                fingerprint.pop(field_name, None)
    for artifact in payload["recognized_artifacts"]:
        if kind_name is not None and artifact.get("kind") != kind_name:
            continue
        fingerprint = artifact.setdefault("fingerprint", {})
        for field_name in field_names:
            fingerprint.pop(field_name, None)


def append_required_artifact(
    payload: dict,
    kind_name: str,
    *,
    suffix: str,
    sha256: str,
    **fingerprint_metadata: object,
) -> None:
    row = payload["required"][kind_name]
    artifact = copy.deepcopy(row["artifacts"][0])
    artifact["path"] = f"{artifact['path'].removesuffix('.json')}-{suffix}.json"
    artifact["sha256"] = sha256
    artifact.setdefault("fingerprint", {}).update(fingerprint_metadata)
    row["artifacts"].append(artifact)
    row["artifact_count"] = len(row["artifacts"])

    recognized_artifact = copy.deepcopy(artifact)
    recognized_artifact["kind"] = kind_name
    payload["recognized_artifacts"].append(recognized_artifact)
    payload["recognized_artifact_count"] = len(payload["recognized_artifacts"])
    payload["evidence_file_count"] = len(payload["recognized_artifacts"])


def append_hedging_billing_cycle(
    payload: dict,
    *,
    cycle_id: str,
    cycle_index: int,
    artifact_sha256: str,
    policy_digest_hex: str = SHA256,
    reference_decision_id_hex: str = SHA256,
    statement_bundle_digest_hex: str = SHA256,
    reconciliation_digest_hex: str = SHA256,
) -> None:
    cycle = copy.deepcopy(payload["valid_billing_cycles"][0])
    cycle.update(
        {
            "cycle_id": cycle_id,
            "cycle_index": cycle_index,
            "policy_digest_hex": policy_digest_hex,
            "reference_decision_id_hex": reference_decision_id_hex,
            "statement_bundle_digest_hex": statement_bundle_digest_hex,
            "reconciliation_digest_hex": reconciliation_digest_hex,
        }
    )
    payload["valid_billing_cycles"].append(cycle)
    append_required_artifact(
        payload,
        "billing_cycle",
        suffix=cycle_id,
        sha256=artifact_sha256,
        cycle_id=cycle_id,
        cycle_index=cycle_index,
        policy_digest_hex=policy_digest_hex,
        reference_decision_id_hex=reference_decision_id_hex,
        statement_bundle_digest_hex=statement_bundle_digest_hex,
        reconciliation_digest_hex=reconciliation_digest_hex,
    )


def run_gate(root: Path, *extra: str) -> int:
    write_foundational_summary(root)
    return MODULE.main(
        [
            "--evidence-dir",
            str(root),
            "--now-unix",
            str(NOW_UNIX),
            "--deployment-id",
            DEPLOYMENT_ID,
            "--environment",
            ENVIRONMENT,
            *foundational_cli_args(),
            *extra,
        ]
    )


def run_foundational_case(
    root: Path,
    payload: dict,
    *extra: str,
) -> tuple[int, dict]:
    """Run one signed prerequisite mutation beside a valid lane summary."""

    root.mkdir()
    write_gate(root, "gateway_load")
    write_foundational_summary(root, payload)
    summary = root / "aggregate.json"
    exit_code = run_gate(
        root,
        "--require-gate",
        "gateway_load",
        "--summary-out",
        str(summary),
        *extra,
    )
    return exit_code, json.loads(summary.read_text(encoding="utf-8"))


def test_duplicate_required_gate_fails_before_validation(capsys) -> None:
    assert (
        MODULE.main(
            [
                "--require-gate",
                "gateway_load",
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "duplicate required evidence kind" in captured.err
    assert "gateway_load" not in captured.err


def test_unknown_required_gate_fails_before_validation(capsys) -> None:
    unknown_gate = "private-key-placeholder"

    assert (
        MODULE.main(["--require-gate", unknown_gate, "--now-unix", str(NOW_UNIX)])
        == 2
    )

    captured = capsys.readouterr()
    assert "unknown required evidence kind" in captured.err
    assert unknown_gate not in captured.err


def test_malformed_required_gate_fails_before_validation(capsys) -> None:
    malformed_gate = "gateway_load,"

    assert (
        MODULE.main(["--require-gate", malformed_gate, "--now-unix", str(NOW_UNIX)])
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "--require-kind entries must be non-empty canonical strings"
        in captured.err
    )
    assert malformed_gate not in captured.err


def test_malformed_integer_arguments_fail_before_validation(capsys) -> None:
    cases = [
        ("--now-unix", "private-key-01", "must be an integer"),
        ("--now-unix", "0", "must be positive"),
        (
            "--max-summary-artifact-age-secs",
            "private-key-02",
            "must be an integer",
        ),
        ("--max-summary-artifact-age-secs", "-1", "must be non-negative"),
    ]

    for flag, value, diagnostic in cases:
        assert MODULE.main([flag, value]) == 2

        captured = capsys.readouterr()
        assert diagnostic in captured.err
        assert value not in captured.err
        assert captured.out == ""


def load_lane_fixture_module(gate_name: str):
    fixture_name = LANE_FIXTURE_TESTS[gate_name]
    fixture_path = Path(__file__).resolve().parent / fixture_name
    spec = importlib.util.spec_from_file_location(
        f"{fixture_path.stem}_aggregate_fixture",
        fixture_path,
    )
    fixture_module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader  # pragma: no cover - defensive
    sys.modules[spec.name] = fixture_module
    spec.loader.exec_module(fixture_module)
    return fixture_module


def write_complete_lane_fixture_summary(
    gate_name: str,
    root: Path,
) -> tuple[dict, int]:
    fixture_module = load_lane_fixture_module(gate_name)
    evidence_root = root / gate_name
    evidence_root.mkdir()
    summary = root / f"{gate_name}.json"

    if gate_name == "pop_credentials":
        evidence_dir = fixture_module.write_complete_evidence(evidence_root)
        exit_code = fixture_module.MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(fixture_module.NOW),
            ]
        )
    elif gate_name == "transparency":
        fixture_module.write_complete_evidence(evidence_root)
        exit_code = fixture_module.MODULE.main(
            [
                "--evidence-dir",
                str(evidence_root),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(fixture_module.NOW_UNIX),
            ]
        )
    elif gate_name == "reputation":
        fixture_module.write_complete_evidence(evidence_root)
        exit_code = fixture_module.run_gate(
            evidence_root,
            "--require-provider",
            "provider-a",
            "--summary-out",
            str(summary),
        )
    elif gate_name == "gateway_compliance":
        fixture_module.write_complete_evidence(evidence_root)
        exit_code = fixture_module.MODULE.main(
            [
                "--evidence-dir",
                str(evidence_root),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(fixture_module.NOW),
            ]
        )
    else:
        fixture_module.write_complete_evidence(evidence_root)
        exit_code = fixture_module.run_gate(
            evidence_root,
            "--summary-out",
            str(summary),
        )

    assert exit_code == 0
    now_unix = getattr(
        fixture_module,
        "NOW_UNIX",
        getattr(fixture_module, "NOW", NOW_UNIX),
    )
    return json.loads(summary.read_text(encoding="utf-8")), now_unix


def lane_summary_deployment_context(payload: dict) -> tuple[str, str]:
    contexts = set()
    for row in payload["required"].values():
        for artifact in row["artifacts"]:
            fingerprint = artifact["fingerprint"]
            contexts.add(
                (
                    fingerprint.get("deployment_id"),
                    fingerprint.get("environment"),
                )
            )

    assert contexts
    assert len(contexts) == 1
    deployment_id, environment = next(iter(contexts))
    assert isinstance(deployment_id, str) and deployment_id
    assert isinstance(environment, str) and environment
    return deployment_id, environment


def normalize_deployment_context(value: object, deployment_id: str, environment: str) -> None:
    if isinstance(value, dict):
        if "deployment_id" in value:
            value["deployment_id"] = deployment_id
        if "environment" in value:
            value["environment"] = environment
        for child in value.values():
            normalize_deployment_context(child, deployment_id, environment)
    elif isinstance(value, list):
        for item in value:
            normalize_deployment_context(item, deployment_id, environment)


def write_normalized_complete_lane_summaries(
    fixture_root: Path,
    summary_root: Path,
) -> int:
    now_values = []
    for gate_name in MODULE.DEFAULT_REQUIRED_GATES:
        payload, now_unix = write_complete_lane_fixture_summary(gate_name, fixture_root)
        normalize_deployment_context(payload, DEPLOYMENT_ID, ENVIRONMENT)
        write_json(summary_root / f"{gate_name}.json", payload)
        now_values.append(now_unix)
    assert now_values
    return max(now_values)


def test_payload_free_summary_metadata_fields_are_derived_from_gate_contracts() -> None:
    expected = frozenset().union(*MODULE.GATE_METADATA_FIELDS.values())

    assert MODULE.PAYLOAD_FREE_SUMMARY_METADATA_FIELDS == expected
    assert MODULE.PAYLOAD_FREE_SUMMARY_FIELDS == (
        MODULE.PAYLOAD_FREE_SUMMARY_CORE_FIELDS | expected
    )
    assert set(MODULE.GATE_METADATA_FIELDS) == set(MODULE.GATE_BY_NAME)


def test_required_kind_schema_contracts_cover_gate_contracts() -> None:
    assert set(MODULE.GATE_REQUIRED_KIND_SCHEMAS) == set(MODULE.GATE_BY_NAME)

    for gate in MODULE.GATE_SUMMARY_KINDS:
        schemas = MODULE.GATE_REQUIRED_KIND_SCHEMAS[gate.name]
        assert set(schemas) == set(gate.required_kinds)
        for kind_name, schema in schemas.items():
            assert MODULE.canonical_string(kind_name) == kind_name
            assert MODULE.canonical_string(schema) == schema


def test_canonical_string_rejects_unicode_controls() -> None:
    assert MODULE.canonical_string("gateway_load") == "gateway_load"

    for value in (
        "",
        " gateway_load",
        "gateway_load ",
        "gateway\nload",
        "gateway\u200dload",
        "gateway\u202eload",
        "gateway\ue000load",
    ):
        assert MODULE.canonical_string(value) is None


def test_payload_free_summary_metadata_fields_have_validator_coverage() -> None:
    metadata_fields = set(MODULE.PAYLOAD_FREE_SUMMARY_METADATA_FIELDS)
    valid_fields = {field for field in metadata_fields if field.startswith("valid_")}
    hex_list_fields = set(MODULE.PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS)
    binding_list_fields = set(MODULE.PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS)
    object_list_fields = set(MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS)
    positive_int_list_fields = set(
        MODULE.PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS
    )
    string_list_fields = set(MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS)
    string_array_list_fields = set(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS
    )
    scalar_hex_fields = set(MODULE.PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS)
    object_fields = set(MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS)

    assert valid_fields == hex_list_fields | binding_list_fields | object_list_fields
    for field in valid_fields:
        assert (
            sum(
                field in validator_fields
                for validator_fields in (
                    hex_list_fields,
                    binding_list_fields,
                    object_list_fields,
                )
            )
            == 1
        )

    assert metadata_fields == (
        hex_list_fields
        | binding_list_fields
        | object_list_fields
        | positive_int_list_fields
        | string_list_fields
        | string_array_list_fields
        | scalar_hex_fields
        | object_fields
    )
    assert set(MODULE.PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS) == (
        valid_fields
        | positive_int_list_fields
        | string_list_fields
        | string_array_list_fields
    )
    assert set(MODULE.PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS) == (
        hex_list_fields
        | binding_list_fields
        | positive_int_list_fields
        | string_list_fields
        | string_array_list_fields
    )
    assert set(MODULE.PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS) == scalar_hex_fields
    assert set(MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS) == (
        scalar_hex_fields
    )
    assert {
        field
        for _gate_name, field in (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS
        )
    } == scalar_hex_fields
    assert {
        field
        for _gate_name, field in (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS
        )
    } == string_list_fields
    assert {
        field
        for _gate_name, field in (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS
        )
    } == string_array_list_fields
    assert {
        field
        for _gate_name, field in MODULE.PAYLOAD_FREE_SUMMARY_ALLOWED_STRING_LIST_VALUES
    } <= (string_list_fields | string_array_list_fields)
    assert {
        field
        for _gate_name, field in MODULE.PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES
    } <= (string_list_fields | string_array_list_fields)
    assert {
        field
        for _gate_name, field in MODULE.PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS
    } <= (string_list_fields | string_array_list_fields)
    assert {
        field
        for _gate_name, field in (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS
        )
    } == positive_int_list_fields
    assert {
        field
        for _gate_name, field in (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS
        )
    } == binding_list_fields
    assert set(MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS) == (
        object_list_fields
    )
    assert {
        field
        for _gate_name, field in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS
    } == object_list_fields
    assert set(MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_FINGERPRINT_HEX_BINDINGS) == (
        object_list_fields
    )


def test_declared_metadata_without_validator_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    field = "future_metadata"
    gate_fields = dict(MODULE.GATE_METADATA_FIELDS)
    gate_fields["gateway_load"] = frozenset(
        set(gate_fields["gateway_load"]) | {field}
    )
    metadata_fields = frozenset(set(MODULE.PAYLOAD_FREE_SUMMARY_METADATA_FIELDS) | {field})
    monkeypatch.setattr(MODULE, "GATE_METADATA_FIELDS", gate_fields)
    monkeypatch.setattr(MODULE, "PAYLOAD_FREE_SUMMARY_METADATA_FIELDS", metadata_fields)
    monkeypatch.setattr(
        MODULE,
        "PAYLOAD_FREE_SUMMARY_FIELDS",
        MODULE.PAYLOAD_FREE_SUMMARY_CORE_FIELDS | metadata_fields,
    )

    payload = gate_summary("gateway_load")
    payload[field] = "future-value"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"{field} validator is not configured for `gateway_load`" in errors


def test_complete_aggregate_readiness_passes(tmp_path: Path) -> None:
    write_all_gates(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == MODULE.SUMMARY_SCHEMA
    assert payload["status"] == "ready"
    assert payload["recognized_summary_count"] == len(MODULE.DEFAULT_REQUIRED_GATES)
    assert payload["deployment"] == {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    assert payload["foundational_prerequisites"]["valid"] is True
    assert payload["foundational_prerequisites"]["required_ids"] == list(
        MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    )
    assert payload["foundational_prerequisites"]["prerequisite_count"] == len(
        MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    )
    assert "signature" not in payload["foundational_prerequisites"]
    assert payload["required"]["gateway_load"]["valid"] is True
    assert payload["required"]["gateway_load"]["path"] == "gateway_load.json"
    assert payload["required"]["gateway_load"]["thresholds"] == {
        "max_evidence_bytes": 2_097_152,
    }


def test_full_aggregate_rejects_lane_summary_bytes_swapped_after_signing(
    tmp_path: Path,
) -> None:
    """The HSM envelope must bind the exact reviewed lane summary byte set."""

    write_all_gates(tmp_path)
    write_foundational_summary(tmp_path)
    gateway_summary_path = tmp_path / "gateway_load.json"
    gateway_summary_path.write_bytes(gateway_summary_path.read_bytes() + b"\n")
    summary_path = tmp_path / "aggregate.json"

    assert run_gate(tmp_path, "--summary-out", str(summary_path)) == 1
    result = json.loads(summary_path.read_text(encoding="utf-8"))
    diagnostics = "\n".join(result["errors"])
    assert result["status"] == "blocked"
    assert result["recognized_summary_count"] == len(MODULE.DEFAULT_REQUIRED_GATES)
    assert (
        "foundational prerequisite lane summary binding for gateway_load does "
        "not match the supplied readiness summary"
        in diagnostics
    )
    assert result["foundational_prerequisites"]["valid"] is False


def test_foundational_prerequisite_schema_inventories_are_closed() -> None:
    assert MODULE.FOUNDATIONAL_PREREQUISITE_FIELDS == {
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
    assert MODULE.FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS == {
        "deployment_id",
        "environment",
    }
    assert MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS == {
        "algorithm",
        "public_key_fingerprint_sha256",
        "signature_hex",
    }
    assert MODULE.FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS == {
        "gate",
        "sha256",
    }


def test_foundational_prerequisites_reject_schema_set_freshness_and_context_attacks(
    tmp_path: Path,
    capsys,
) -> None:
    """Exercise signed semantic attacks independently of signature forgery."""

    def signed_mutation(mutator) -> dict:
        payload = foundational_summary()
        mutator(payload)
        resign_foundational_summary(payload)
        return payload

    cases: list[tuple[str, dict, str]] = []

    cases.append(
        (
            "missing-id",
            signed_mutation(lambda payload: payload["prerequisites"].pop()),
            "foundational prerequisites are missing required ids",
        )
    )

    def add_unknown_id(payload: dict) -> None:
        row = copy.deepcopy(payload["prerequisites"][-1])
        row["id"] = "SF-99"
        row["evidence_anchor_sha256"] = hashlib.sha256(b"SF-99").hexdigest()
        payload["prerequisites"].append(row)

    cases.append(
        (
            "extra-id",
            signed_mutation(add_unknown_id),
            "foundational prerequisites contain unknown ids",
        )
    )
    cases.append(
        (
            "duplicate-id",
            signed_mutation(
                lambda payload: payload["prerequisites"][-1].__setitem__(
                    "id", payload["prerequisites"][0]["id"]
                )
            ),
            "foundational prerequisites must not contain duplicate ids",
        )
    )
    cases.append(
        (
            "reordered-ids",
            signed_mutation(lambda payload: payload["prerequisites"].reverse()),
            "foundational prerequisites must match the exact required set and canonical order",
        )
    )
    cases.append(
        (
            "duplicate-anchor",
            signed_mutation(
                lambda payload: payload["prerequisites"][1].__setitem__(
                    "evidence_anchor_sha256",
                    payload["prerequisites"][0]["evidence_anchor_sha256"],
                )
            ),
            "foundational prerequisites must use unique evidence anchors",
        )
    )
    cases.append(
        (
            "zero-anchor",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_anchor_sha256", "00" * 32
                )
            ),
            "evidence_anchor_sha256 must not be zero",
        )
    )
    cases.append(
        (
            "uppercase-anchor",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_anchor_sha256",
                    payload["prerequisites"][0]["evidence_anchor_sha256"].upper(),
                )
            ),
            "evidence_anchor_sha256 must be canonical lowercase SHA-256",
        )
    )
    cases.append(
        (
            "failed-row",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "status", "failed"
                )
            ),
            ".status must be `verified`",
        )
    )
    cases.append(
        (
            "failed-envelope",
            signed_mutation(lambda payload: payload.__setitem__("status", "failed")),
            "foundational prerequisite status must be `verified`",
        )
    )
    cases.append(
        (
            "stale-envelope",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "generated_at_unix",
                    NOW_UNIX - MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS - 1,
                )
            ),
            "foundational prerequisite generated_at_unix exceeds max summary artifact age",
        )
    )
    cases.append(
        (
            "future-envelope",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "generated_at_unix", NOW_UNIX + 1
                )
            ),
            "foundational prerequisite generated_at_unix must not be future",
        )
    )
    cases.append(
        (
            "stale-evidence",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_generated_at_unix",
                    NOW_UNIX - MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS - 1,
                )
            ),
            "evidence_generated_at_unix exceeds max summary artifact age",
        )
    )
    cases.append(
        (
            "future-evidence",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_generated_at_unix", NOW_UNIX + 1
                )
            ),
            "evidence_generated_at_unix must not be future",
        )
    )
    cases.append(
        (
            "post-envelope-evidence",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_generated_at_unix", GENERATED_AT + 1
                )
            ),
            "evidence_generated_at_unix must not be later than the signed envelope",
        )
    )
    cases.append(
        (
            "mixed-deployment",
            signed_mutation(
                lambda payload: payload["deployment"].__setitem__(
                    "deployment_id", "sorafs-mainnet-2026-07"
                )
            ),
            "foundational prerequisite deployment_id must match --deployment-id",
        )
    )
    cases.append(
        (
            "mixed-environment",
            signed_mutation(
                lambda payload: payload["deployment"].__setitem__(
                    "environment", "prod"
                )
            ),
            "foundational prerequisite environment must match --environment",
        )
    )
    cases.append(
        (
            "unicode-control-id",
            signed_mutation(
                lambda payload: payload["prerequisites"][2].__setitem__(
                    "id", "SF-\u202e2"
                )
            ),
            ".id must be a canonical string",
        )
    )
    cases.append(
        (
            "unicode-homoglyph-id",
            signed_mutation(
                lambda payload: payload["prerequisites"][2].__setitem__(
                    "id", "S\uff26-2"
                )
            ),
            "foundational prerequisites contain unknown ids",
        )
    )
    cases.append(
        (
            "boolean-sequence",
            signed_mutation(
                lambda payload: payload.__setitem__("release_sequence", True)
            ),
            "foundational prerequisite release_sequence must be an integer in 1..2^63-1",
        )
    )
    cases.append(
        (
            "zero-predecessor-after-genesis",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "previous_envelope_sha256", "00" * 32
                )
            ),
            "foundational prerequisite sequence after 1 must use a non-zero predecessor",
        )
    )
    cases.append(
        (
            "rollback-sequence",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "release_sequence", FOUNDATIONAL_RELEASE_SEQUENCE - 1
                )
            ),
            "release_sequence must match the operator-reviewed expected value",
        )
    )
    cases.append(
        (
            "replay-wrong-predecessor",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "previous_envelope_sha256", "22" * 32
                )
            ),
            "previous_envelope_sha256 must match the operator-reviewed expected digest",
        )
    )
    cases.append(
        (
            "wrong-algorithm",
            signed_mutation(
                lambda payload: payload["signature"].__setitem__(
                    "algorithm", "ed25519ph"
                )
            ),
            "signature algorithm must be `ed25519`",
        )
    )

    secret_path = "../../runtime-only-private-key-material"

    def add_path_payload(payload: dict) -> None:
        payload["prerequisites"][0]["evidence_path"] = secret_path

    cases.append(
        (
            "traversal-payload-field",
            signed_mutation(add_path_payload),
            "fields must match the schema-closed contract",
        )
    )

    def add_raw_secret(payload: dict) -> None:
        payload["raw_payload"] = "runtime-only-secret-payload"

    cases.append(
        (
            "raw-secret-field",
            signed_mutation(add_raw_secret),
            "is not allowed",
        )
    )

    for index, (name, payload, expected_error) in enumerate(cases):
        exit_code, result = run_foundational_case(
            tmp_path / f"foundation-semantic-{index:02d}-{name}",
            payload,
        )
        assert exit_code == 1, name
        diagnostics = "\n".join(result["errors"])
        assert expected_error in diagnostics, name
        assert result["status"] == "blocked", name
        assert result["foundational_prerequisites"]["valid"] is False, name
        assert "signature" not in result["foundational_prerequisites"], name
        captured = capsys.readouterr()
        rendered = diagnostics + captured.err + captured.out
        assert secret_path not in rendered, name
        assert "runtime-only-secret-payload" not in rendered, name


def test_foundational_prerequisites_reject_signature_digest_and_trust_attacks(
    tmp_path: Path,
) -> None:
    """Reject forgeries, self-selected signers, and non-canonical signatures."""

    cases: list[tuple[str, dict, str]] = []

    forged_signature = foundational_summary()
    signature_hex = forged_signature["signature"]["signature_hex"]
    forged_signature["signature"]["signature_hex"] = (
        ("0" if signature_hex[0] != "0" else "1") + signature_hex[1:]
    )
    cases.append(
        (
            "forged-signature",
            forged_signature,
            "foundational prerequisite signature verification failed",
        )
    )

    forged_digest = foundational_summary()
    forged_digest["prerequisites"][0]["evidence_anchor_sha256"] = "33" * 32
    cases.append(
        (
            "forged-digest",
            forged_digest,
            "foundational prerequisite signature verification failed",
        )
    )

    malleable_signature = foundational_summary()
    signature = bytes.fromhex(malleable_signature["signature"]["signature_hex"])
    scalar = int.from_bytes(signature[32:], "little") + RELEASE_CRYPTO._ED_L  # noqa: SLF001
    malleable_signature["signature"]["signature_hex"] = (
        signature[:32] + scalar.to_bytes(32, "little")
    ).hex()
    cases.append(
        (
            "non-canonical-scalar",
            malleable_signature,
            "foundational prerequisite signature verification failed",
        )
    )

    alternate_signer = foundational_summary()
    resign_foundational_summary(alternate_signer, seed=bytes.fromhex("2f" * 32))
    cases.append(
        (
            "self-selected-signer",
            alternate_signer,
            "signer fingerprint must match the operator-trusted key",
        )
    )

    wrong_fingerprint = foundational_summary()
    wrong_fingerprint["signature"]["public_key_fingerprint_sha256"] = "44" * 32
    wrong_fingerprint["signature"]["signature_hex"] = "00" * 64
    wrong_fingerprint["signature"]["signature_hex"] = ed25519_sign(
        FOUNDATIONAL_SIGNING_SEED,
        MODULE.foundational_signing_payload(wrong_fingerprint),
    ).hex()
    cases.append(
        (
            "forged-fingerprint",
            wrong_fingerprint,
            "signer fingerprint must match the operator-trusted key",
        )
    )

    zero_signature = foundational_summary()
    zero_signature["signature"]["signature_hex"] = "00" * 64
    cases.append(
        (
            "zero-signature",
            zero_signature,
            "signature must be a non-zero canonical Ed25519 signature",
        )
    )

    uppercase_signature = foundational_summary()
    uppercase_signature["signature"]["signature_hex"] = uppercase_signature[
        "signature"
    ]["signature_hex"].upper()
    cases.append(
        (
            "uppercase-signature",
            uppercase_signature,
            "signature must be a non-zero canonical Ed25519 signature",
        )
    )

    for index, (name, payload, expected_error) in enumerate(cases):
        exit_code, result = run_foundational_case(
            tmp_path / f"foundation-signature-{index:02d}-{name}",
            payload,
        )
        assert exit_code == 1, name
        diagnostics = "\n".join(result["errors"])
        assert expected_error in diagnostics, name
        assert result["status"] == "blocked", name
        assert result["foundational_prerequisites"]["valid"] is False, name


def test_foundational_prerequisite_missing_duplicate_and_untrusted_inputs_block(
    tmp_path: Path,
) -> None:
    missing_root = tmp_path / "missing-foundation"
    missing_root.mkdir()
    lane_path = write_json(
        missing_root / "gateway_load.json",
        gate_summary("gateway_load"),
    )
    missing_summary = missing_root / "aggregate.json"
    assert (
        MODULE.main(
            [
                "--evidence",
                str(lane_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *foundational_cli_args(),
                "--summary-out",
                str(missing_summary),
            ]
        )
        == 1
    )
    missing = json.loads(missing_summary.read_text(encoding="utf-8"))
    assert missing["foundational_prerequisites"] == {
        "schema": MODULE.FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "present": False,
        "valid": False,
        "errors": ["missing required foundational prerequisite summary"],
    }
    assert missing["status"] == "blocked"

    duplicate_root = tmp_path / "duplicate-foundation"
    duplicate_root.mkdir()
    write_gate(duplicate_root, "gateway_load")
    write_json(
        duplicate_root / "foundational_prerequisites_copy.json",
        foundational_summary(),
    )
    duplicate_summary = duplicate_root / "aggregate.json"
    assert (
        run_gate(
            duplicate_root,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(duplicate_summary),
        )
        == 1
    )
    duplicate = json.loads(duplicate_summary.read_text(encoding="utf-8"))
    assert "duplicate foundational prerequisite summary" in duplicate["errors"]
    assert duplicate["foundational_prerequisites"]["valid"] is False

    untrusted_root = tmp_path / "untrusted-foundation"
    untrusted_root.mkdir()
    write_gate(untrusted_root, "gateway_load")
    write_foundational_summary(untrusted_root)
    untrusted_summary = untrusted_root / "aggregate.json"
    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(untrusted_root),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(untrusted_summary),
            ]
        )
        == 1
    )
    untrusted = json.loads(untrusted_summary.read_text(encoding="utf-8"))
    diagnostics = "\n".join(untrusted["errors"])
    assert "operator-trusted Ed25519 public key" in diagnostics
    assert "operator-reviewed expected value" in diagnostics
    assert "operator-reviewed expected digest" in diagnostics


def test_foundational_prerequisite_path_policy_rejects_symlink_and_traversal(
    tmp_path: Path,
    capsys,
) -> None:
    root = tmp_path / "path-policy"
    root.mkdir()
    lane = write_json(root / "gateway_load.json", gate_summary("gateway_load"))
    target = write_json(root / "foundation-target.json", foundational_summary())
    symlink = root / "foundation-link.json"
    symlink.symlink_to(target)

    assert (
        MODULE.main(
            [
                "--evidence",
                str(lane),
                "--evidence",
                str(symlink),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *foundational_cli_args(),
            ]
        )
        == 1
    )
    captured = capsys.readouterr()
    assert "evidence file must not be a symlink" in captured.err
    assert "foundation-target" not in captured.err

    (root / "nested").mkdir()
    traversal = root / "nested" / ".." / "foundation-target.json"
    assert (
        MODULE.main(
            [
                "--evidence",
                str(lane),
                "--evidence",
                str(traversal),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *foundational_cli_args(),
            ]
        )
        == 2
    )
    captured = capsys.readouterr()
    assert "checker-rendered paths" in captured.err


def test_foundational_prerequisite_cli_trust_values_fail_closed_without_echo(
    tmp_path: Path,
    capsys,
) -> None:
    write_gate(tmp_path, "gateway_load")
    malformed_values = (
        (
            "--foundational-prerequisite-signer-public-key-hex",
            "private-key-runtime-only",
            "must be exactly 32 bytes of lowercase hex",
        ),
        (
            "--foundational-prerequisite-signer-public-key-hex",
            "00" * 32,
            "must not be the all-zero key",
        ),
        (
            "--foundational-prerequisite-previous-envelope-sha256",
            "SECRET-PREDECESSOR",
            "must be canonical lowercase SHA-256",
        ),
        (
            "--foundational-prerequisite-release-sequence",
            str(1 << 63),
            "must be in 1..2^63-1",
        ),
    )
    for flag, value, expected_error in malformed_values:
        assert (
            MODULE.main(
                [
                    "--evidence-dir",
                    str(tmp_path),
                    "--require-gate",
                    "gateway_load",
                    "--now-unix",
                    str(NOW_UNIX),
                    "--deployment-id",
                    DEPLOYMENT_ID,
                    "--environment",
                    ENVIRONMENT,
                    *foundational_cli_args(),
                    flag,
                    value,
                ]
            )
            == 2
        )
        captured = capsys.readouterr()
        assert expected_error in captured.err
        assert value not in captured.err
        assert captured.out == ""


def test_direct_checker_requires_explicit_deployment_context(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(payload["errors"])
    assert (
        "aggregate production readiness requires --deployment-id and --environment"
        in errors
    )
    assert payload["status"] == "blocked"
    assert payload["deployment"] == {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    assert payload["required"]["gateway_load"]["valid"] is True


def test_complete_lane_fixture_summaries_pass_aggregate_contract(
    tmp_path: Path,
) -> None:
    failures = {}

    for gate_name in MODULE.DEFAULT_REQUIRED_GATES:
        payload, now_unix = write_complete_lane_fixture_summary(gate_name, tmp_path)
        deployment_id, environment = lane_summary_deployment_context(payload)
        row, validation_errors = MODULE.validate_gate_summary(
            MODULE.GATE_BY_NAME[gate_name],
            payload,
            MODULE.ValidationOptions(
                now_unix=now_unix,
                max_summary_artifact_age_secs=(
                    MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS
                ),
                deployment_id=deployment_id,
                environment=environment,
            ),
        )
        if validation_errors:
            failures[gate_name] = validation_errors
            continue
        assert row["present"] is True
        assert row["valid"] is True
        assert row["recognized_artifact_count"] == payload["recognized_artifact_count"]
        assert row["expected_required_kinds"] == list(
            MODULE.GATE_BY_NAME[gate_name].required_kinds
        )

    assert failures == {}


def test_complete_lane_fixture_summaries_pass_full_aggregate_cli(
    tmp_path: Path,
) -> None:
    fixture_root = tmp_path / "fixtures"
    summary_root = tmp_path / "summaries"
    fixture_root.mkdir()
    summary_root.mkdir()
    now_unix = write_normalized_complete_lane_summaries(fixture_root, summary_root)
    write_foundational_summary(
        summary_root,
        foundational_summary(
            generated_at_unix=now_unix - 1,
            lane_summary_sha256=lane_summary_digests(summary_root),
        ),
    )
    summary = tmp_path / "aggregate.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(summary_root),
                "--now-unix",
                str(now_unix),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *foundational_cli_args(),
                "--summary-out",
                str(summary),
            ]
        )
        == 0
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == MODULE.SUMMARY_SCHEMA
    assert payload["status"] == "ready"
    assert payload["recognized_summary_count"] == len(MODULE.DEFAULT_REQUIRED_GATES)
    assert payload["deployment"] == {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    assert set(payload["required"]) == set(MODULE.DEFAULT_REQUIRED_GATES)
    for gate_name, row in payload["required"].items():
        assert row["present"] is True
        assert row["valid"] is True
        assert row["deployment_id"] == DEPLOYMENT_ID
        assert row["environment"] == ENVIRONMENT
        assert row["expected_required_kinds"] == list(
            MODULE.GATE_BY_NAME[gate_name].required_kinds
        )


def test_aggregate_lane_summary_paths_are_archive_relative(tmp_path: Path) -> None:
    nested = tmp_path / "release" / "summaries"
    nested.mkdir(parents=True)
    write_gate(nested, "gateway_load")
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 0
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["required"]["gateway_load"]["path"] == (
        "release/summaries/gateway_load.json"
    )


def test_explicit_lane_summary_path_uses_safe_basename(tmp_path: Path) -> None:
    gateway_load = write_gate(tmp_path, "gateway_load")
    foundations = write_foundational_summary(tmp_path)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence",
                str(gateway_load),
                "--evidence",
                str(foundations),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *foundational_cli_args(),
                "--summary-out",
                str(summary),
            ]
        )
        == 0
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["required"]["gateway_load"]["path"] == "gateway_load.json"


def test_aggregate_summary_path_label_falls_back_when_identity_resolution_fails(
    tmp_path: Path, monkeypatch
) -> None:
    gateway_load = write_gate(tmp_path, "gateway_load")
    original_resolve = Path.resolve

    def resolve(path: Path, *args, **kwargs):
        if path == gateway_load:
            raise RuntimeError("resolver failure")
        return original_resolve(path, *args, **kwargs)

    monkeypatch.setattr(Path, "resolve", resolve)

    assert MODULE.aggregate_summary_path_label(gateway_load, [tmp_path]) == (
        "gateway_load.json"
    )


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    write_foundational_summary(tmp_path)
    args = tmp_path / "aggregate.args"
    args.write_text(
        "\n".join(
            [
                f"--evidence-dir {tmp_path}",
                "--require-gate gateway_load",
                f"--now-unix {NOW_UNIX}",
                f"--deployment-id {DEPLOYMENT_ID}",
                f"--environment {ENVIRONMENT}",
                (
                    "--foundational-prerequisite-signer-public-key-hex "
                    f"{FOUNDATIONAL_SIGNER_PUBLIC_KEY.hex()}"
                ),
                (
                    "--foundational-prerequisite-release-sequence "
                    f"{FOUNDATIONAL_RELEASE_SEQUENCE}"
                ),
                (
                    "--foundational-prerequisite-previous-envelope-sha256 "
                    f"{FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256}"
                ),
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_response_file_symlink_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    target = tmp_path / "private-key-args"
    target.write_text("--require-gate gateway_load\n", encoding="utf-8")
    symlink = tmp_path / "aggregate.args"
    symlink.symlink_to(target)

    assert MODULE.main([f"@{symlink}"]) == 2

    captured = capsys.readouterr()
    assert "@ARGFILE must not be a symlink" in captured.err
    assert "private-key-args" not in captured.err
    assert "aggregate.args" not in captured.err
    assert captured.out == ""


def test_response_file_malformed_line_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    args = tmp_path / "aggregate.args"
    args.write_text("--require-gate 'private-key-placeholder\n", encoding="utf-8")

    assert MODULE.main([f"@{args}"]) == 2

    captured = capsys.readouterr()
    assert "@ARGFILE line 1:" in captured.err
    assert "private-key-placeholder" not in captured.err
    assert "aggregate.args" not in captured.err
    assert captured.out == ""


def test_summary_out_symlink_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    write_gate(tmp_path, "gateway_load")
    target = tmp_path / "private-key-summary.json"
    target.write_text("{}", encoding="utf-8")
    summary = tmp_path / "summary.json"
    summary.symlink_to(target)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(summary),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--summary-out" in captured.err
    assert "must not be a symlink" in captured.err
    assert "private-key-summary" not in captured.err
    assert captured.out == ""


def test_summary_out_parent_symlink_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    write_gate(tmp_path, "gateway_load")
    target = tmp_path / "private-key-summary-parent"
    target.mkdir()
    parent = tmp_path / "summary-parent"
    parent.symlink_to(target, target_is_directory=True)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(parent / "summary.json"),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--summary-out parent" in captured.err
    assert "must not be a symlink" in captured.err
    assert "private-key-summary-parent" not in captured.err
    assert "SoraFS production readiness is blocked" not in captured.err
    assert captured.out == ""


def test_summary_out_directory_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    write_gate(tmp_path, "gateway_load")
    summary = tmp_path / "summary-dir"
    summary.mkdir()
    (summary / "marker.txt").write_text("private-key-placeholder", encoding="utf-8")

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(summary),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--summary-out" in captured.err
    assert "must not be a directory" in captured.err
    assert "private-key-placeholder" not in captured.err
    assert "SoraFS production readiness is blocked" not in captured.err
    assert captured.out == ""


def test_summary_out_parent_file_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    write_gate(tmp_path, "gateway_load")
    parent = tmp_path / "summary-parent"
    parent.write_text("private-key-placeholder", encoding="utf-8")

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(parent / "summary.json"),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--summary-out parent" in captured.err
    assert "must be a directory when it exists" in captured.err
    assert "private-key-placeholder" not in captured.err
    assert "SoraFS production readiness is blocked" not in captured.err
    assert captured.out == ""


def test_summary_out_unsafe_path_fails_before_validation_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    write_gate(tmp_path, "gateway_load")

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(tmp_path / "private%26%2395%3Bkey-summary.json"),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "SoraFS checker-rendered paths must not contain secret-looking"
        in captured.err
    )
    assert "private%26%2395%3Bkey" not in captured.err
    assert "private&#95;key" not in captured.err
    assert "private_key" not in captured.err
    assert "SoraFS production readiness is blocked" not in captured.err
    assert captured.out == ""


def test_summary_out_same_as_explicit_evidence_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    evidence = write_gate(tmp_path, "gateway_load")

    assert (
        MODULE.main(
            [
                "--evidence",
                str(evidence),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(evidence),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--summary-out" in captured.err
    assert "must not be the same path as --evidence" in captured.err
    assert "SoraFS production readiness is blocked" not in captured.err
    assert captured.out == ""


def test_summary_out_discovered_from_evidence_dir_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    summary = write_gate(tmp_path, "gateway_load")

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(summary),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "evidence file conflicts with reserved output" in captured.err
    assert "gateway_load.json" not in captured.err
    assert "SoraFS production readiness is blocked" not in captured.err
    assert captured.out == ""


def test_evidence_dir_unsafe_path_fails_before_validation_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path / "bearer&#95;token_bundle"),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "SoraFS checker-rendered paths must not contain secret-looking"
        in captured.err
    )
    assert "bearer&#95;token" not in captured.err
    assert "bearer_token" not in captured.err
    assert "SoraFS production readiness is blocked" not in captured.err
    assert captured.out == ""


def test_explicit_evidence_unsafe_path_fails_before_validation_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    assert (
        MODULE.main(
            [
                "--evidence",
                str(tmp_path / "private%26%2395%3Bkey.json"),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "SoraFS checker-rendered paths must not contain secret-looking"
        in captured.err
    )
    assert "private%26%2395%3Bkey" not in captured.err
    assert "private&#95;key" not in captured.err
    assert "private_key" not in captured.err
    assert "SoraFS production readiness is blocked" not in captured.err
    assert captured.out == ""


def test_explicit_evidence_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    target = write_gate(tmp_path, "gateway_load")
    symlink = tmp_path / "linked-summary.json"
    symlink.symlink_to(target)

    assert (
        MODULE.main(
            [
                "--evidence",
                str(symlink),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence file must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-summary" not in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["status"] == "blocked"


def test_explicit_evidence_broken_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    symlink = tmp_path / "linked-summary.json"
    symlink.symlink_to(tmp_path / "missing-target.json")

    assert (
        MODULE.main(
            [
                "--evidence",
                str(symlink),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence file must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-summary" not in diagnostics
    assert "missing-target" not in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_evidence_dir_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    target_dir = tmp_path / "target"
    target_dir.mkdir()
    write_gate(target_dir, "gateway_load")
    symlink_dir = tmp_path / "linked-evidence"
    symlink_dir.symlink_to(target_dir, target_is_directory=True)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(symlink_dir),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence directory must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-evidence" not in diagnostics
    assert "target" not in diagnostics
    assert payload["status"] == "blocked"


def test_evidence_dir_broken_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    symlink_dir = tmp_path / "linked-evidence"
    symlink_dir.symlink_to(tmp_path / "missing-target", target_is_directory=True)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(symlink_dir),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence directory must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-evidence" not in diagnostics
    assert "missing-target" not in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_explicit_evidence_parent_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    target_parent = tmp_path / "target-parent"
    target_parent.mkdir()
    write_gate(target_parent, "gateway_load")
    linked_parent = tmp_path / "linked-parent"
    linked_parent.symlink_to(target_parent, target_is_directory=True)

    assert (
        MODULE.main(
            [
                "--evidence",
                str(linked_parent / "gateway_load.json"),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence file parent must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-parent" not in diagnostics
    assert "target-parent" not in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_explicit_evidence_broken_parent_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    linked_parent = tmp_path / "linked-broken-parent"
    linked_parent.symlink_to(tmp_path / "missing-target", target_is_directory=True)

    assert (
        MODULE.main(
            [
                "--evidence",
                str(linked_parent / "gateway_load.json"),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence file parent must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-broken-parent" not in diagnostics
    assert "missing-target" not in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_evidence_dir_parent_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    target_parent = tmp_path / "target-parent"
    target_dir = target_parent / "summaries"
    target_dir.mkdir(parents=True)
    write_gate(target_dir, "gateway_load")
    linked_parent = tmp_path / "linked-parent"
    linked_parent.symlink_to(target_parent, target_is_directory=True)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(linked_parent / "summaries"),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence directory parent must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-parent" not in diagnostics
    assert "target-parent" not in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_evidence_dir_broken_parent_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    linked_parent = tmp_path / "linked-broken-parent"
    linked_parent.symlink_to(tmp_path / "missing-target", target_is_directory=True)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(linked_parent / "summaries"),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence directory parent must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-broken-parent" not in diagnostics
    assert "missing-target" not in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_discovered_evidence_file_symlink_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    target = write_gate(tmp_path, "gateway_load")
    symlink = tmp_path / "linked-discovered-summary.json"
    symlink.symlink_to(target)
    target.unlink()

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence file must not be a symlink" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "linked-discovered-summary" not in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_duplicate_explicit_evidence_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    evidence = write_gate(tmp_path, "gateway_load")

    assert (
        MODULE.main(
            [
                "--evidence",
                str(evidence),
                "--evidence",
                str(evidence),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "duplicate explicit evidence file" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_explicit_and_directory_evidence_overlap_fails_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    evidence = write_gate(tmp_path, "gateway_load")

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--evidence",
                str(evidence),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "evidence file provided by multiple evidence sources" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_overlapping_evidence_dirs_fail_closed_without_path_leak(
    tmp_path: Path,
    capsys,
) -> None:
    nested = tmp_path / "nested"
    nested.mkdir()
    write_gate(nested, "gateway_load")

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--evidence-dir",
                str(nested),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = "\n".join([*payload["errors"], captured.err])
    assert "duplicate evidence file" in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "gateway_load.json" not in diagnostics
    assert "nested" not in diagnostics
    assert payload["summary_file_count"] == 0
    assert payload["status"] == "blocked"


def test_evidence_source_conflicts_fail_closed_from_config(
    tmp_path: Path,
    capsys,
) -> None:
    cases = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases

    for index, gate_name in enumerate(cases):
        for suffix, expected_error in (
            ("duplicate_explicit", "duplicate explicit evidence file"),
            ("explicit_directory_overlap", "evidence file provided by multiple evidence sources"),
            ("overlapping_directories", "duplicate evidence file"),
        ):
            root = tmp_path / f"{index}_{gate_name}_{suffix}"
            root.mkdir()
            secret_dir = root / f"runtime-only-private-source-{index:03d}-{suffix}"
            if suffix == "overlapping_directories":
                secret_dir.mkdir()
                evidence = write_gate(secret_dir, gate_name)
                evidence_args = [
                    "--evidence-dir",
                    str(root),
                    "--evidence-dir",
                    str(secret_dir),
                ]
            else:
                evidence = write_gate(root, gate_name)
                evidence_args = (
                    ["--evidence", str(evidence), "--evidence", str(evidence)]
                    if suffix == "duplicate_explicit"
                    else ["--evidence-dir", str(root), "--evidence", str(evidence)]
                )

            assert (
                MODULE.main(
                    [
                        *evidence_args,
                        "--require-gate",
                        gate_name,
                        "--now-unix",
                        str(NOW_UNIX),
                        "--deployment-id",
                        DEPLOYMENT_ID,
                        "--environment",
                        ENVIRONMENT,
                    ]
                )
                == 1
            )

            captured = capsys.readouterr()
            payload = json.loads(captured.out)
            diagnostics = "\n".join([*payload["errors"], captured.err])
            assert expected_error in diagnostics
            assert (
                f"missing required {gate_name} production readiness summary"
                in diagnostics
            )
            assert payload["summary_file_count"] == 0
            assert payload["status"] == "blocked"
            assert evidence.name not in diagnostics
            assert str(evidence) not in diagnostics
            assert "runtime-only-private-source" not in diagnostics


def test_missing_required_gate_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")

    assert run_gate(tmp_path) == 1


def test_missing_required_summary_rows_fail_closed_from_config(tmp_path: Path) -> None:
    options = production_validation_options()

    for index, gate in enumerate(MODULE.GATE_SUMMARY_KINDS):
        evidence_root = tmp_path / f"{index}_{gate.name}"
        evidence_root.mkdir()
        write_foundational_summary(evidence_root)
        summary, build_errors = MODULE.build_summary(
            [evidence_root],
            [],
            (gate.name,),
            options,
            None,
        )
        missing_error = f"missing required {gate.name} production readiness summary"
        assert summary["status"] == "blocked"
        assert summary["summary_file_count"] == 0
        assert summary["recognized_summary_count"] == 0
        assert build_errors == [missing_error]
        assert summary["errors"] == [missing_error]
        assert summary["required"][gate.name] == {
            "schema": gate.schema,
            "present": False,
            "valid": False,
            "errors": [missing_error],
        }

        row_errors: list[str] = []
        MODULE.validate_aggregate_required_row_output(
            gate,
            summary["required"][gate.name],
            row_errors,
        )
        assert row_errors == []

        summary_errors: list[str] = []
        MODULE.validate_aggregate_summary_output(
            summary,
            (gate.name,),
            summary_errors,
        )
        assert summary_errors == []


def test_explicit_unrequired_gate_summary_fails(tmp_path: Path) -> None:
    gateway_load = write_gate(tmp_path, "gateway_load")
    reputation = write_gate(tmp_path, "reputation")
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence",
                str(gateway_load),
                "--evidence",
                str(reputation),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "explicit production readiness summary belongs to unrequired gate"
        in errors
    )
    assert (
        result["errors"].count(
            "explicit production readiness summary belongs to unrequired gate"
        )
        == 1
    )
    assert MODULE.GATE_BY_NAME["reputation"].schema not in errors
    assert "reputation` gate" not in errors

    drift_errors: list[str] = []
    MODULE.validate_disallowed_summary_diagnostics(
        drift_errors,
        unknown_schema_count=0,
        explicit_unrequired_count=1,
    )
    assert (
        "aggregate summary unrequired-gate diagnostics must match explicit unrequired summaries"
        in "\n".join(drift_errors)
    )


def test_unknown_sorafs_schema_in_summary_dir_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    unknown_schema = "sorafs.unknown.private-key-placeholder.v1"
    unknown_path = tmp_path / "unknown.json"
    write_json(
        unknown_path,
        {
            "schema": unknown_schema,
            "status": "ready",
            "errors": [],
        },
    )
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(payload["errors"])
    assert "unknown SoraFS readiness summary schema" in errors
    assert payload["errors"].count("unknown SoraFS readiness summary schema") == 1
    assert unknown_schema not in errors
    assert str(unknown_path) not in errors

    drift_errors: list[str] = []
    MODULE.validate_disallowed_summary_diagnostics(
        drift_errors,
        unknown_schema_count=1,
        explicit_unrequired_count=0,
    )
    assert (
        "aggregate summary unknown-schema diagnostics must match discovered unknown summaries"
        in "\n".join(drift_errors)
    )


def test_unknown_sorafs_schema_fails_closed_from_config(tmp_path: Path) -> None:
    gate_names = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert gate_names

    for index, gate_name in enumerate(gate_names):
        root = tmp_path / f"{index}_{gate_name}"
        root.mkdir()
        write_gate(root, gate_name)
        unknown_schema = f"sorafs.unknown.private-key-placeholder.{index:03d}.v1"
        unknown_path = root / f"runtime-only-private-unknown-{index:03d}.json"
        write_json(
            unknown_path,
            {
                "schema": unknown_schema,
                "status": "ready",
                "errors": [],
            },
        )
        summary = root / "summary.json"

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        payload = json.loads(summary.read_text(encoding="utf-8"))
        errors = "\n".join(payload["errors"])
        assert "unknown SoraFS readiness summary schema" in errors
        assert payload["errors"].count("unknown SoraFS readiness summary schema") == 1
        assert unknown_schema not in errors
        assert str(unknown_path) not in errors
        assert unknown_path.name not in errors


def test_duplicate_sensitive_json_key_load_error_does_not_echo(
    tmp_path: Path,
    capsys,
) -> None:
    evidence = tmp_path / "malformed-summary.json"
    secret_value = "runtime-only-private-key-material"
    evidence.write_text(
        '{"schema":"sorafs.gateway_load.rollout_evidence_gate.v1",'
        f'"private_key":"{secret_value}",'
        f'"private_key":"{secret_value}-shadow"}}',
        encoding="utf-8",
    )
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence",
                str(evidence),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(summary.read_text(encoding="utf-8"))
    diagnostics = "\n".join([*payload["errors"], captured.err, captured.out])
    assert (
        "failed to load evidence JSON: evidence JSON object contains duplicate key "
        "`<sensitive-key>`"
    ) in diagnostics
    assert "missing required gateway_load production readiness summary" in diagnostics
    assert "private_key" not in diagnostics
    assert secret_value not in diagnostics
    assert "malformed-summary.json" not in diagnostics


def test_unknown_non_sorafs_schema_in_summary_dir_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    write_json(
        tmp_path / "unrelated.json",
        {
            "schema": "unrelated.summary.v1",
            "status": "ready",
            "errors": [],
        },
    )

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_blocked_lane_summary_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load", status="blocked", errors=["lane failed"])

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_lane_summary_with_load_errors_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["load_errors"] = ["failed to parse skipped-evidence.json"]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_lane_summary_load_errors_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["load_errors"] = "failed to parse skipped-evidence.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_lane_summary_status_and_load_errors_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases

    for index, gate_name in enumerate(cases):
        for suffix, mutate, expected_errors in (
            (
                "blocked_status",
                lambda payload, secret: (
                    payload.__setitem__("status", "blocked"),
                    payload.__setitem__("errors", [secret]),
                ),
                ("status must be `ready`", "errors must be empty"),
            ),
            (
                "nonempty_load_errors",
                lambda payload, secret: payload.__setitem__(
                    "load_errors",
                    [secret],
                ),
                ("load_errors must be empty",),
            ),
            (
                "malformed_load_errors",
                lambda payload, secret: payload.__setitem__("load_errors", secret),
                ("load_errors must be an empty error list",),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            secret = f"runtime-only-private-lane-error-{index:03d}-{suffix}"
            mutate(payload, secret)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            for expected_error in expected_errors:
                assert f"{gate_name}: {expected_error}" in errors
            assert result["required"][gate_name]["valid"] is False
            assert secret not in result_text


def test_malformed_lane_summary_thresholds_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = ["max_artifact_age_secs=86400"]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_missing_lane_summary_thresholds_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    del payload["thresholds"]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_empty_lane_summary_thresholds_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_lane_summary_threshold_keys_must_be_canonical(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {" max_evidence_age_secs": 86_400}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_lane_summary_threshold_values_must_be_non_negative_int(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {"max_evidence_age_secs": {"seconds": 86_400}}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_lane_summary_thresholds_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases

    for index, gate_name in enumerate(cases):
        for suffix, mutate, expected_errors, forbidden_values in (
            (
                "missing",
                lambda payload, _secret: payload.pop("thresholds"),
                ("thresholds must be present",),
                (),
            ),
            (
                "scalar",
                lambda payload, secret: payload.__setitem__("thresholds", secret),
                ("thresholds must be an object",),
                (f"runtime-only-private-threshold-{index:03d}-scalar",),
            ),
            (
                "empty",
                lambda payload, _secret: payload.__setitem__("thresholds", {}),
                ("thresholds must not be empty",),
                (),
            ),
            (
                "noncanonical_key",
                lambda payload, _secret: payload.__setitem__(
                    "thresholds",
                    {"bad\nkey": False},
                ),
                (
                    "thresholds keys must be canonical strings",
                    "thresholds.<invalid> must be a non-negative integer",
                ),
                ("bad\nkey",),
            ),
            (
                "sensitive_key",
                lambda payload, _secret: payload.__setitem__(
                    "thresholds",
                    {"private_key": 1},
                ),
                ("thresholds.<sensitive-key> must not be present",),
                ("private_key",),
            ),
            (
                "noninteger_value",
                lambda payload, secret: payload.__setitem__(
                    "thresholds",
                    {"max_evidence_age_secs": {"seconds": secret}},
                ),
                ("thresholds.max_evidence_age_secs must be a non-negative integer",),
                (f"runtime-only-private-threshold-{index:03d}-noninteger_value",),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            secret = f"runtime-only-private-threshold-{index:03d}-{suffix}"
            mutate(payload, secret)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            for expected_error in expected_errors:
                assert f"{gate_name}: {expected_error}" in errors
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text
            assert result["required"][gate_name]["valid"] is False


def test_malformed_threshold_key_value_diagnostic_is_sanitized(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {"bad\nkey": False}
    write_json(tmp_path / "gateway_load.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "thresholds.<invalid> must be a non-negative integer" in errors
    assert "bad\nkey" not in errors


def test_malformed_threshold_entries_are_not_carried_into_summary(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {
        "max_evidence_bytes": 2_097_152,
        " bad_key": 5,
        "nested": {"value": 1},
    }
    write_json(tmp_path / "gateway_load.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["required"]["gateway_load"]["thresholds"] == {
        "max_evidence_bytes": 2_097_152,
    }
    errors = "\n".join(result["errors"])
    assert "thresholds keys must be canonical strings" in errors
    assert "thresholds.nested must be a non-negative integer" in errors


def test_stale_artifact_timestamp_fails(tmp_path: Path) -> None:
    summary = tmp_path / "summary.json"
    write_gate(
        tmp_path,
        "gateway_load",
        generated_at_unix=NOW_UNIX - MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS - 1,
    )

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert ".fingerprint.generated_at_unix exceeds max summary artifact age" in errors
    assert ".artifacts[0].generated_at_unix exceeds max summary artifact age" not in errors
    assert "recognized_artifacts[0].generated_at_unix exceeds max summary artifact age" not in errors


def test_future_artifact_timestamp_fails_with_fingerprint_path(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load", generated_at_unix=NOW_UNIX + 1)
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert ".fingerprint.generated_at_unix must not be future" in errors
    assert ".artifacts[0].generated_at_unix must not be future" not in errors
    assert "recognized_artifacts[0].generated_at_unix must not be future" not in errors


def test_artifact_freshness_timestamps_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases
    stale_generated_at = (
        NOW_UNIX - MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS - 1
    )
    future_generated_at = NOW_UNIX + 1

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, generated_at, expected_error in (
            (
                "stale",
                stale_generated_at,
                ".fingerprint.generated_at_unix exceeds max summary artifact age",
            ),
            (
                "future",
                future_generated_at,
                ".fingerprint.generated_at_unix must not be future",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload["required"][kind_name]["artifacts"][0]["fingerprint"][
                "generated_at_unix"
            ] = generated_at
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"]
                == payload["required"][kind_name]["artifacts"][0]["path"]
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result = json.loads(summary.read_text(encoding="utf-8"))
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0]{expected_error}"
                in errors
            )
            assert f"recognized_artifacts[{recognized_index}]{expected_error}" in errors
            assert "artifacts[0].generated_at_unix" not in errors
            assert f"recognized_artifacts[{recognized_index}].generated_at_unix" not in errors
            assert result["required"][gate_name]["valid"] is False


def test_artifact_generated_at_shape_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        malformed_values = (
            ("missing", None, ()),
            ("boolean", False, ()),
            ("zero", 0, ()),
            ("negative", -1, ()),
            (
                "string",
                f"runtime-only-generated-at-{index:03d}\nprivate_key",
                (f"runtime-only-generated-at-{index:03d}\nprivate_key", "private_key"),
            ),
            (
                "object",
                {"private_key": f"runtime-only-generated-at-{index:03d}"},
                ("private_key", f"runtime-only-generated-at-{index:03d}"),
            ),
        )
        for suffix, generated_at, forbidden_values in malformed_values:
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            fingerprint = payload["required"][kind_name]["artifacts"][0][
                "fingerprint"
            ]
            if suffix == "missing":
                fingerprint.pop("generated_at_unix")
            else:
                fingerprint["generated_at_unix"] = generated_at
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"]
                == payload["required"][kind_name]["artifacts"][0]["path"]
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].fingerprint."
                "generated_at_unix must be positive"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].fingerprint."
                "generated_at_unix must be positive"
                in errors
            )
            assert result["required"][gate_name]["valid"] is False
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_malformed_required_artifact_digest_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["artifacts"][0]["sha256"] = "AB" * 32
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_and_recognized_artifact_digests_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, forged_digest in (
            ("uppercase", "AB" * 32),
            ("malformed", f"runtime-only-digest-{index:03d}"),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload["required"][kind_name]["artifacts"][0]["sha256"] = forged_digest
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"]
                == payload["required"][kind_name]["artifacts"][0]["path"]
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].sha256 "
                "must be canonical lowercase SHA-256"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].sha256 "
                "must be canonical lowercase SHA-256"
                in errors
            )
            assert forged_digest not in result_text


def test_required_and_recognized_artifact_digest_shapes_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        malformed_values = (
            ("missing", None, ()),
            ("boolean", True, ()),
            (
                "object",
                {"private_key": f"runtime-only-digest-{index:03d}"},
                ("private_key", f"runtime-only-digest-{index:03d}"),
            ),
        )
        for suffix, malformed_digest, forbidden_values in malformed_values:
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}_digest"
            root.mkdir()
            payload = gate_summary(gate_name)
            required_artifact = payload["required"][kind_name]["artifacts"][0]
            if suffix == "missing":
                required_artifact.pop("sha256")
            else:
                required_artifact["sha256"] = malformed_digest
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"] == required_artifact["path"]
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].sha256 "
                "must be canonical lowercase SHA-256"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].sha256 "
                "must be canonical lowercase SHA-256"
                in errors
            )
            assert result["required"][gate_name]["valid"] is False
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_malformed_required_artifact_metadata_label_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["artifacts"][0]["schema"] = "\ninvalid"
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_and_recognized_artifact_optional_labels_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for field in ("schema", "status"):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{field}"
            root.mkdir()
            forged_label = f"runtime-only-{field}-{index:03d}\nsecret"
            payload = gate_summary(gate_name)
            payload["required"][kind_name]["artifacts"][0][field] = forged_label
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"]
                == payload["required"][kind_name]["artifacts"][0]["path"]
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            if field == "schema":
                assert (
                    f"{gate_name}.required.{kind_name}.artifacts[0].schema "
                    "must be canonical"
                    in errors
                )
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].{field} "
                "must be canonical when present"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].{field} "
                "must be canonical when present"
                in errors
            )
            assert forged_label not in result_text


def test_required_and_recognized_artifact_schema_shapes_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        object_schema = {"private_key": f"runtime-only-schema-{index:03d}"}
        for surface, shape, mutate, expected_error, forbidden_values in (
            (
                "required",
                "missing",
                lambda payload: payload["required"][kind_name]["artifacts"][
                    0
                ].pop("schema"),
                f"{gate_name}.required.{kind_name}.artifacts[0].schema "
                "must be canonical",
                (),
            ),
            (
                "required",
                "object",
                lambda payload: payload["required"][kind_name]["artifacts"][
                    0
                ].__setitem__(
                    "schema",
                    object_schema,
                ),
                f"{gate_name}.required.{kind_name}.artifacts[0].schema "
                "must be canonical",
                ("private_key", object_schema["private_key"]),
            ),
            (
                "recognized",
                "missing",
                lambda payload: payload["recognized_artifacts"][
                    next(
                        artifact_index
                        for artifact_index, artifact in enumerate(
                            payload["recognized_artifacts"]
                        )
                        if artifact["kind"] == kind_name
                    )
                ].pop("schema"),
                "recognized_artifacts[{index}].schema must match the required "
                "artifact metadata",
                (),
            ),
            (
                "recognized",
                "object",
                lambda payload: payload["recognized_artifacts"][
                    next(
                        artifact_index
                        for artifact_index, artifact in enumerate(
                            payload["recognized_artifacts"]
                        )
                        if artifact["kind"] == kind_name
                    )
                ].__setitem__(
                    "schema",
                    object_schema,
                ),
                "recognized_artifacts[{index}].schema must be canonical when present",
                ("private_key", object_schema["private_key"]),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{surface}_{shape}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload)
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            if surface == "recognized":
                expected = expected_error.format(index=recognized_index)
            else:
                expected = expected_error
            assert expected in errors
            assert result["required"][gate_name]["valid"] is False
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_non_object_artifact_fingerprint_reports_single_sanitized_error(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    secret_fingerprint = "runtime-only-private-key-material"
    payload["required"][first_required]["artifacts"][0][
        "fingerprint"
    ] = secret_fingerprint
    payload["recognized_artifacts"][0]["fingerprint"] = secret_fingerprint
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert errors.count(".fingerprint must be an object") == 2
    assert ".fingerprint.generated_at_unix" not in errors
    assert ".fingerprint.deployment_id" not in errors
    assert ".fingerprint.environment" not in errors
    assert secret_fingerprint not in errors


def test_sensitive_artifact_fingerprint_key_is_rejected_without_echo(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    secret_value = "runtime-only-private-key-material"
    payload["required"][first_required]["artifacts"][0]["fingerprint"][
        "private_key"
    ] = secret_value
    payload["recognized_artifacts"][0]["fingerprint"]["private_key"] = secret_value
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert ".fingerprint.<sensitive-key> must not be present" in errors
    assert "private_key" not in errors
    assert secret_value not in errors


def test_artifact_fingerprint_shape_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, mutate, expected_error, forbidden_values in (
            (
                "non_object",
                lambda artifact, secret: artifact.__setitem__("fingerprint", secret),
                ".fingerprint must be an object",
                (f"runtime-only-private-fingerprint-{index:03d}-non_object",),
            ),
            (
                "sensitive_key",
                lambda artifact, secret: artifact["fingerprint"].__setitem__(
                    "private_key",
                    secret,
                ),
                ".fingerprint.<sensitive-key> must not be present",
                (
                    "private_key",
                    f"runtime-only-private-fingerprint-{index:03d}-sensitive_key",
                ),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            secret = f"runtime-only-private-fingerprint-{index:03d}-{suffix}"
            mutate(payload["required"][kind_name]["artifacts"][0], secret)
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"]
                == payload["required"][kind_name]["artifacts"][0]["path"]
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            if suffix == "sensitive_key":
                assert (
                    f"required.{kind_name}.artifacts[0]{expected_error} "
                    "in SoraFS production readiness summary"
                    in errors
                )
                assert (
                    f"recognized_artifacts[{recognized_index}]{expected_error} "
                    "in SoraFS production readiness summary"
                    in errors
                )
            else:
                assert (
                    f"{gate_name}.required.{kind_name}.artifacts[0]{expected_error}"
                    in errors
                )
                assert (
                    f"recognized_artifacts[{recognized_index}]{expected_error}"
                    in errors
                )
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text
            if suffix == "non_object":
                assert ".fingerprint.generated_at_unix" not in errors
                assert ".fingerprint.deployment_id" not in errors
                assert ".fingerprint.environment" not in errors
            assert result["required"][gate_name]["valid"] is False


def test_artifact_fingerprint_deployment_context_rejects_nonproduction_without_echo(
    tmp_path: Path,
) -> None:
    cases = [
        (
            {"deployment_id": "gateway-staging-a"},
            "fingerprint.deployment_id must not contain "
            "non-production deployment markers ['staging']",
            "gateway-staging-a",
        ),
        (
            {"environment": "qa"},
            "fingerprint.environment must be production",
            "qa",
        ),
    ]

    for index, (fingerprint_metadata, expected_error, raw_label) in enumerate(cases):
        root = tmp_path / f"artifact_context_{index}"
        root.mkdir()
        payload = gate_summary("gateway_load")
        add_fingerprint_metadata(payload, **fingerprint_metadata)
        summary = root / "summary.json"
        write_json(root / "gateway_load.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                "gateway_load",
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert expected_error in errors
        assert raw_label not in errors


def test_artifact_fingerprint_deployment_context_rejects_nonproduction_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases
    assert set(cases) == {
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in MODULE.GATE_BY_NAME[gate.name].required_kinds
    }

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, fingerprint_metadata, expected_error, raw_label in (
            (
                "deployment_id",
                {"deployment_id": f"sorafs-staging-artifact-{index:03d}"},
                "fingerprint.deployment_id must not contain "
                "non-production deployment markers ['staging']",
                f"sorafs-staging-artifact-{index:03d}",
            ),
            (
                "environment",
                {"environment": f"runtime-env-secret-{index:03d}"},
                "fingerprint.environment must be production",
                f"runtime-env-secret-{index:03d}",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            add_fingerprint_metadata(
                payload,
                kind_name=kind_name,
                **fingerprint_metadata,
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert expected_error in errors
            assert raw_label not in result_text


def test_artifact_fingerprint_deployment_context_shape_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, fingerprint_metadata, expected_error, forbidden_values in (
            (
                "deployment_id",
                {"deployment_id": f"runtime-only-deployment-{index:03d}\nsecret"},
                "fingerprint.deployment_id must be canonical",
                (f"runtime-only-deployment-{index:03d}\nsecret",),
            ),
            (
                "environment",
                {"environment": f"runtime-only-environment-{index:03d}\nsecret"},
                "fingerprint.environment must be canonical",
                (f"runtime-only-environment-{index:03d}\nsecret",),
            ),
            (
                "reviewed",
                {"deployment_context_reviewed": False},
                "fingerprint.deployment_context_reviewed must be true",
                (),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}_shape"
            root.mkdir()
            payload = gate_summary(gate_name)
            add_fingerprint_metadata(
                payload,
                kind_name=kind_name,
                **fingerprint_metadata,
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].{expected_error}"
                in errors
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"]
                == payload["required"][kind_name]["artifacts"][0]["path"]
            )
            assert f"recognized_artifacts[{recognized_index}].{expected_error}" in errors
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_artifact_deployment_context_required_fields_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for field, expected_error in (
            ("deployment_id", "fingerprint.deployment_id must be canonical"),
            ("environment", "fingerprint.environment must be canonical"),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{field}_missing"
            root.mkdir()
            payload = gate_summary(gate_name)
            required_path = payload["required"][kind_name]["artifacts"][0]["path"]
            remove_fingerprint_metadata(payload, field, kind_name=kind_name)
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"] == required_path
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result = json.loads(summary.read_text(encoding="utf-8"))
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].{expected_error}"
                in errors
            )
            assert f"recognized_artifacts[{recognized_index}].{expected_error}" in errors
            assert result["required"][gate_name]["valid"] is False


def test_artifact_deployment_context_reviewed_shape_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, reviewed_value, forbidden_values in (
            ("missing", None, ()),
            (
                "string",
                f"runtime-only-reviewed-{index:03d}\nprivate_key",
                (f"runtime-only-reviewed-{index:03d}\nprivate_key", "private_key"),
            ),
            (
                "object",
                {"private_key": f"runtime-only-reviewed-{index:03d}"},
                ("private_key", f"runtime-only-reviewed-{index:03d}"),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}_reviewed"
            root.mkdir()
            payload = gate_summary(gate_name)
            fingerprint = payload["required"][kind_name]["artifacts"][0][
                "fingerprint"
            ]
            if suffix == "missing":
                fingerprint.pop("deployment_context_reviewed")
            else:
                fingerprint["deployment_context_reviewed"] = reviewed_value
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"]
                == payload["required"][kind_name]["artifacts"][0]["path"]
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].fingerprint."
                "deployment_context_reviewed must be true"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].fingerprint."
                "deployment_context_reviewed must be true"
                in errors
            )
            assert result["required"][gate_name]["valid"] is False
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_artifact_paths_must_be_archive_portable(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    absolute_path = "/tmp/private-sorafs-evidence.json"
    parent_path = "../private-sorafs-evidence.json"
    encoded_parent_path = "nested/%2e%2e/private_key.json"
    encoded_separator_path = "nested/bad%2Fprivate_key.json"
    encoded_drive_path = "nested/C%3A/private_key.json"
    html_separator_path = "nested/bad&#47;private_key.json"
    html_drive_path = "nested/C&#58;/private_key.json"
    payload["required"][first_required]["artifacts"][0]["path"] = absolute_path
    payload["recognized_artifacts"][0]["path"] = absolute_path
    payload["recognized_artifacts"][1]["path"] = parent_path
    payload["recognized_artifacts"][2]["path"] = encoded_parent_path
    payload["recognized_artifacts"][3]["path"] = encoded_separator_path
    payload["recognized_artifacts"][4]["path"] = encoded_drive_path
    html_separator_artifact = copy.deepcopy(payload["recognized_artifacts"][0])
    html_separator_artifact["path"] = html_separator_path
    payload["recognized_artifacts"].append(html_separator_artifact)
    html_drive_artifact = copy.deepcopy(payload["recognized_artifacts"][0])
    html_drive_artifact["path"] = html_drive_path
    payload["recognized_artifacts"].append(html_drive_artifact)
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        ".path must be archive-relative without absolute, empty, current, "
        "parent, encoded, URI-scheme-like, platform-specific, or secret-looking segments"
        in errors
    )
    assert "recognized_artifacts[2].path" in errors
    assert "recognized_artifacts[3].path" in errors
    assert "recognized_artifacts[4].path" in errors
    assert "recognized_artifacts[5].path" in errors
    assert "recognized_artifacts[6].path" in errors
    assert absolute_path not in errors
    assert parent_path not in errors
    assert encoded_parent_path not in errors
    assert encoded_separator_path not in errors
    assert encoded_drive_path not in errors
    assert html_separator_path not in errors
    assert html_drive_path not in errors


def test_required_and_recognized_artifact_paths_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases
    expected_error_suffix = (
        "path must be archive-relative without absolute, empty, current, "
        "parent, encoded, URI-scheme-like, platform-specific, or secret-looking "
        "segments"
    )

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, forged_path in (
            (
                "absolute",
                f"/tmp/runtime-only/sorafs/{gate_name}/{kind_name}/private_key.json",
            ),
            (
                "encoded_parent",
                f"artifacts/{gate_name}/%2e%2e/{kind_name}/private_key.json",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload["required"][kind_name]["artifacts"][0]["path"] = forged_path
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name and artifact["path"] == forged_path
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0]."
                f"{expected_error_suffix}"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].{expected_error_suffix}"
                in errors
            )
            assert forged_path not in result_text
            assert "private_key" not in result_text


def test_required_and_recognized_artifact_path_shapes_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        malformed_values = (
            ("missing", None, ()),
            ("boolean", False, ()),
            (
                "object",
                {"private_key": f"runtime-only-path-{index:03d}"},
                ("private_key", f"runtime-only-path-{index:03d}"),
            ),
        )
        for suffix, malformed_path, forbidden_values in malformed_values:
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}_path"
            root.mkdir()
            payload = gate_summary(gate_name)
            required_artifact = payload["required"][kind_name]["artifacts"][0]
            if suffix == "missing":
                required_artifact.pop("path")
            else:
                required_artifact["path"] = malformed_path
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].path "
                "must be canonical"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].path must be canonical"
                in errors
            )
            assert result["required"][gate_name]["valid"] is False
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_required_and_recognized_artifact_path_variants_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases
    expected_error_suffix = (
        "path must be archive-relative without absolute, empty, current, "
        "parent, encoded, URI-scheme-like, platform-specific, or secret-looking "
        "segments"
    )

    def forged_paths(gate_name: str, kind_name: str) -> tuple[tuple[str, str], ...]:
        return (
            (
                "parent",
                f"artifacts/{gate_name}/../{kind_name}/runtime-only-report.json",
            ),
            (
                "encoded_separator",
                f"artifacts/{gate_name}/bad%2F{kind_name}-runtime-only.json",
            ),
            (
                "html_separator",
                f"artifacts/{gate_name}/bad&#47;{kind_name}-runtime-only.json",
            ),
            (
                "empty_segment",
                f"artifacts/{gate_name}//{kind_name}-runtime-only.json",
            ),
            (
                "platform",
                f"C:\\runtime-only\\sorafs\\{gate_name}\\{kind_name}.json",
            ),
            (
                "sensitive_label",
                f"artifacts/{gate_name}/private_key.json",
            ),
        )

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, forged_path in forged_paths(gate_name, kind_name):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload["required"][kind_name]["artifacts"][0]["path"] = forged_path
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name and artifact["path"] == forged_path
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0]."
                f"{expected_error_suffix}"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].{expected_error_suffix}"
                in errors
            )
            assert forged_path not in result_text
            assert "private_key" not in result_text


def test_required_and_recognized_artifact_path_scheme_and_current_segments_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases
    expected_error_suffix = (
        "path must be archive-relative without absolute, empty, current, "
        "parent, encoded, URI-scheme-like, platform-specific, or secret-looking "
        "segments"
    )

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, forged_path in (
            (
                "uri_scheme",
                f"artifacts/{gate_name}/file:runtime-only-{kind_name}.json",
            ),
            (
                "current_segment",
                f"artifacts/{gate_name}/./{kind_name}-runtime-only.json",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload["required"][kind_name]["artifacts"][0]["path"] = forged_path
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name and artifact["path"] == forged_path
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0]."
                f"{expected_error_suffix}"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].{expected_error_suffix}"
                in errors
            )
            assert forged_path not in result_text


def test_artifact_paths_reject_platform_specific_segments(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    windows_path = "C:\\runtime-only\\sorafs-evidence.json"
    empty_segment_path = "artifacts//sorafs-evidence.json"
    payload["required"][first_required]["artifacts"][0]["path"] = windows_path
    payload["recognized_artifacts"][0]["path"] = windows_path
    payload["recognized_artifacts"][1]["path"] = empty_segment_path
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        ".path must be archive-relative without absolute, empty, current, "
        "parent, encoded, URI-scheme-like, platform-specific, or secret-looking segments"
        in errors
    )
    assert windows_path not in errors
    assert empty_segment_path not in errors


def test_artifact_paths_reject_secret_looking_segments(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    secret_path = "nested/private&#95;key.json"
    encoded_html_secret_path = "nested/private%26%2395%3Bkey.json"
    proof_token_path = "nested/proof-token-report.json"
    payload["recognized_artifacts"][0]["path"] = secret_path
    payload["recognized_artifacts"][1]["path"] = encoded_html_secret_path
    payload["recognized_artifacts"][2]["path"] = proof_token_path
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "recognized_artifacts[0].path must be archive-relative without absolute, "
        "empty, current, parent, encoded, URI-scheme-like, platform-specific, "
        "or secret-looking segments"
        in errors
    )
    assert (
        "recognized_artifacts[1].path must be archive-relative without absolute, "
        "empty, current, parent, encoded, URI-scheme-like, platform-specific, "
        "or secret-looking segments"
        in errors
    )
    assert "recognized_artifacts[2].path must be archive-relative" not in errors
    assert secret_path not in errors
    assert encoded_html_secret_path not in errors
    assert proof_token_path not in errors


def test_portable_artifact_path_rejects_token_alias_segments() -> None:
    assert MODULE.is_archive_portable_artifact_path("nested/api-token.json") is False
    assert MODULE.is_archive_portable_artifact_path("nested/auth%2Dtoken.json") is False
    assert MODULE.is_archive_portable_artifact_path("nested/auth&#45;token.json") is False
    assert (
        MODULE.is_archive_portable_artifact_path("nested/auth%26%2345%3Btoken.json")
        is False
    )
    assert MODULE.is_archive_portable_artifact_path("nested/id-token.json") is False
    assert MODULE.is_archive_portable_artifact_path("nested/jwt.json") is False
    assert MODULE.is_archive_portable_artifact_path("nested/oauth-token.json") is False
    assert (
        MODULE.is_archive_portable_artifact_path("nested/refresh%2Dtoken.json")
        is False
    )
    assert (
        MODULE.is_archive_portable_artifact_path("nested/session%255Ftoken.json")
        is False
    )
    assert MODULE.is_archive_portable_artifact_path("nested/set-cookie.txt") is False
    assert MODULE.is_archive_portable_artifact_path("nested/x-api-token.txt") is False
    assert MODULE.is_archive_portable_artifact_path("nested/password.txt") is False
    assert MODULE.is_archive_portable_artifact_path("nested/response_body.json") is False
    assert MODULE.is_archive_portable_artifact_path(
        "nested/test_sensitive_response_body_f0/report.json"
    )
    assert MODULE.is_archive_portable_artifact_path("nested/proof-token-report.json")


def test_malformed_required_row_schema_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["schema"] = " padded-schema "
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_duplicate_required_artifact_identities_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    duplicate = dict(payload["required"][first_required]["artifacts"][0])
    payload["required"][first_required]["artifacts"].append(duplicate)
    payload["required"][first_required]["artifact_count"] = 2
    payload["recognized_artifact_count"] += 1
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_deployment_mismatch_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    write_gate(tmp_path, "reputation", deployment_id="different-deployment")

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--require-gate",
            "reputation",
        )
        == 1
    )


def test_staging_environment_cannot_promote_production_readiness(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load", environment="staging")
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "aggregate environment must be production" in errors
    assert result["status"] == "blocked"
    assert result["deployment"] == {
        "deployment_id": DEPLOYMENT_ID,
        "environment": "staging",
    }


def test_unreviewed_deployment_id_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    unreviewed_deployment = "gateway-notproductionready-a"
    write_gate(tmp_path, "gateway_load", deployment_id=unreviewed_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment markers "
        "['notproductionready']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['notproductionready']"
        in errors
    )
    assert result["status"] == "blocked"
    assert unreviewed_deployment not in errors


def test_staging_deployment_id_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    staging_deployment = "gateway-staging-a"
    write_gate(tmp_path, "gateway_load", deployment_id=staging_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-production deployment markers "
        "['staging']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert result["status"] == "blocked"
    assert staging_deployment not in errors


def test_numbered_staging_deployment_id_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    staging_deployment = "gateway-staging1-a"
    write_gate(tmp_path, "gateway_load", deployment_id=staging_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-production deployment markers "
        "['staging']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert result["status"] == "blocked"
    assert staging_deployment not in errors


def test_compact_staging_deployment_id_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    staging_deployment = "gateway-stagingready-a"
    write_gate(tmp_path, "gateway_load", deployment_id=staging_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-production deployment markers "
        "['staging']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert result["status"] == "blocked"
    assert staging_deployment not in errors


def test_joined_nonproduction_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    joined_deployment = "gateway-stageproduction-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=joined_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['stage']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['stage']"
        in errors
    )
    assert result["status"] == "blocked"
    assert joined_deployment not in errors


def test_prerelease_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    prerelease_deployment = "gateway-releasecandidate-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=prerelease_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['releasecandidate']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['releasecandidate']"
        in errors
    )
    assert result["status"] == "blocked"
    assert prerelease_deployment not in errors


def test_tokenized_prerelease_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    tokenized_deployment = "gateway-production-candidate-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=tokenized_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['candidate']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['candidate']"
        in errors
    )
    assert result["status"] == "blocked"
    assert tokenized_deployment not in errors


def test_preview_prerelease_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    preview_deployment = "gateway-preprodrelease-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=preview_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['preprod']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['preprod']"
        in errors
    )
    assert result["status"] == "blocked"
    assert preview_deployment not in errors


def test_canary_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    canary_deployment = "gateway-prod-canary-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=canary_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['canary']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['canary']"
        in errors
    )
    assert result["status"] == "blocked"
    assert canary_deployment not in errors


def test_stg_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    stg_deployment = "gateway-stgproduction-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=stg_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['stg']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['stg']"
        in errors
    )
    assert result["status"] == "blocked"
    assert stg_deployment not in errors


def test_poc_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    poc_deployment = "gateway-prod-poc-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=poc_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['poc']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['poc']"
        in errors
    )
    assert result["status"] == "blocked"
    assert poc_deployment not in errors


def test_smoke_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    smoke_deployment = "gateway-production-smoke-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=smoke_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['smoke']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['smoke']"
        in errors
    )
    assert result["status"] == "blocked"
    assert smoke_deployment not in errors


def test_stress_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    stress_deployment = "gateway-prod-stress-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=stress_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['stress']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['stress']"
        in errors
    )
    assert result["status"] == "blocked"
    assert stress_deployment not in errors


def test_shadow_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    shadow_deployment = "gateway-prod-shadow-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=shadow_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['shadow']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['shadow']"
        in errors
    )
    assert result["status"] == "blocked"
    assert shadow_deployment not in errors


def test_cutover_alias_cannot_promote_production_readiness(
    tmp_path: Path,
) -> None:
    cutover_deployment = "gateway-prod-cutover-202606"
    write_gate(tmp_path, "gateway_load", deployment_id=cutover_deployment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "aggregate deployment_id must not contain non-reviewed deployment "
        "markers ['cutover']"
        in errors
    )
    assert (
        "gateway_load aggregate row deployment_id must not contain "
        "non-reviewed deployment markers ['cutover']"
        in errors
    )
    assert result["status"] == "blocked"
    assert cutover_deployment not in errors


def test_explicit_nonproduction_environment_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    write_gate(tmp_path, "gateway_load", environment="staging")

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                "staging",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--environment must be production for this gate" in captured.err
    assert "staging" not in captured.err


def test_explicit_unreviewed_deployment_id_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    unreviewed_deployment = "gateway-notproductionready-a"
    write_gate(tmp_path, "gateway_load", deployment_id=unreviewed_deployment)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                unreviewed_deployment,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "--deployment-id must not contain non-reviewed deployment markers "
        "['notproductionready']"
        in captured.err
    )
    assert unreviewed_deployment not in captured.err


def test_explicit_staging_deployment_id_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    staging_deployment = "gateway-staging-a"
    write_gate(tmp_path, "gateway_load", deployment_id=staging_deployment)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                staging_deployment,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "--deployment-id must not contain non-production deployment markers "
        "['staging']"
        in captured.err
    )
    assert staging_deployment not in captured.err


def test_sensitive_summary_payload_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load", raw_response=True)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_sensitive_summary_key_diagnostic_is_sanitized(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["private\nkey"] = "runtime-only-private-key"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "<sensitive-key> must not be present" in errors
    assert "private\nkey" not in errors


def test_sensitive_summary_key_diagnostic_sanitizes_canonical_key(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["private_key"] = "runtime-only-key-material"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "<sensitive-key> must not be present" in errors
    assert "<sensitive-key> is not allowed in payload-free lane summary" in errors
    assert "private_key" not in errors
    assert "runtime-only-key-material" not in errors


def test_sensitive_threshold_key_is_not_carried_into_summary(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"]["private_key"] = 1
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "thresholds.<sensitive-key> must not be present" in errors
    assert result["required"]["gateway_load"]["thresholds"] == {
        "max_evidence_bytes": 2_097_152,
    }
    assert "private_key" not in errors


def test_sensitive_required_and_artifact_field_diagnostics_are_sanitized(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["private_key"] = "row-key-material"
    payload["required"][first_kind]["artifacts"][0][
        "private_key"
    ] = "artifact-key-material"
    payload["recognized_artifacts"][0]["private_key"] = "recognized-key-material"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        ".<sensitive-key> is not allowed in payload-free required row"
        in errors
    )
    assert (
        ".<sensitive-key> is not allowed in payload-free artifact summary"
        in errors
    )
    assert "private_key" not in errors
    assert "row-key-material" not in errors
    assert "artifact-key-material" not in errors
    assert "recognized-key-material" not in errors


def test_extra_top_level_lane_summary_field_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["debug_report"] = {"note": "not part of the payload-free contract"}
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        "debug_report is not allowed in payload-free lane summary"
        in "\n".join(result["errors"])
    )


def test_payload_free_summary_fields_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases

    for index, gate_name in enumerate(cases):
        for suffix, key, value, expected_errors, forbidden_values in (
            (
                "extra_canonical",
                f"debug_report_{index:03d}",
                {"note": f"runtime-only-private-debug-{index:03d}"},
                (f"debug_report_{index:03d} is not allowed in payload-free lane summary",),
                (f"runtime-only-private-debug-{index:03d}",),
            ),
            (
                "sensitive_canonical",
                "private_key",
                f"runtime-only-private-summary-key-{index:03d}",
                (
                    "<sensitive-key> must not be present",
                    "<sensitive-key> is not allowed in payload-free lane summary",
                ),
                ("private_key", f"runtime-only-private-summary-key-{index:03d}"),
            ),
            (
                "sensitive_noncanonical",
                "private\nkey",
                f"runtime-only-private-summary-newline-key-{index:03d}",
                (
                    "<sensitive-key> must not be present",
                    "summary keys must be canonical strings",
                ),
                ("private\nkey", f"runtime-only-private-summary-newline-key-{index:03d}"),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload[key] = value
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            for expected_error in expected_errors:
                assert f"{gate_name}: {expected_error}" in errors
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text
            assert result["required"][gate_name]["valid"] is False


def test_allowed_top_level_lane_metadata_shape_is_validated(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_suite_report_digests"] = {"digest": SHA256}
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        "valid_suite_report_digests must be a payload-free metadata list"
        in "\n".join(result["errors"])
    )


def test_allowed_top_level_lane_metadata_rejects_nested_raw_payload(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_suite_report_digests"] = [{"raw": "leak"}]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_suite_report_digests[0].<sensitive-key> must not be present"
        in errors
    )
    assert "valid_suite_report_digests[0].raw must not be present" not in errors


def test_allowed_top_level_lane_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_suite_report_digests"] = [SHA256]
    add_fingerprint_metadata(payload, suite_report_digest_hex=SHA256)
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 0


def test_required_top_level_lane_metadata_must_be_present(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    del payload["valid_policy_digests"]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_policy_digests is required for `gateway_load` lane metadata" in errors


def test_required_top_level_lane_metadata_lists_must_not_be_empty(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_policy_digests"] = []
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "valid_policy_digests must not be empty for `gateway_load` lane metadata" in errors


def test_required_lane_metadata_fields_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in sorted(MODULE.GATE_METADATA_FIELDS[gate.name])
    ]
    assert cases

    for gate_name, field in cases:
        root = tmp_path / f"{gate_name}_{field}_missing"
        root.mkdir()
        payload = gate_summary(gate_name)
        assert field in payload
        del payload[field]
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{field} is required for `{gate_name}` lane metadata" in errors


def test_list_lane_metadata_fields_require_list_shape_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in sorted(
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS
        )
    ]
    assert cases

    for gate_name, field in cases:
        for suffix, value, expected_error in (
            (
                "non_list",
                {"runtime_key": "runtime-only-private-key"},
                f"{field} must be a payload-free metadata list",
            ),
            (
                "empty",
                [],
                f"{field} must not be empty for `{gate_name}` lane metadata",
            ),
        ):
            root = tmp_path / f"{gate_name}_{field}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload[field] = value
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert expected_error in errors
            assert "runtime-only-private-key" not in result_text


def test_cross_lane_metadata_fields_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases: list[tuple[str, str]] = []
    for field in sorted(MODULE.PAYLOAD_FREE_SUMMARY_METADATA_FIELDS):
        target_gate = next(
            gate
            for gate in MODULE.GATE_SUMMARY_KINDS
            if field not in MODULE.GATE_METADATA_FIELDS[gate.name]
        )
        cases.append((target_gate.name, field))

    covered_fields = {field for _, field in cases}
    assert covered_fields == MODULE.PAYLOAD_FREE_SUMMARY_METADATA_FIELDS

    secret = "runtime-only-private-key-cross-lane"
    for gate_name, field in cases:
        root = tmp_path / f"{gate_name}_{field}_disallowed"
        root.mkdir()
        payload = gate_summary(gate_name)
        assert field not in MODULE.GATE_METADATA_FIELDS[gate_name]
        payload[field] = {"private_key": secret}
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert f"{field} is not allowed for `{gate_name}` lane metadata" in errors
        assert "private_key" not in result_text
        assert secret not in result_text


def test_hex_list_metadata_must_match_recognized_artifact_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_suite_report_digests"] = [SHA256]
    remove_fingerprint_metadata(payload, "suite_report_digest_hex")
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_suite_report_digests must match recognized artifact fingerprints"
        in errors
    )
    assert SHA256 not in errors


def test_every_hex_list_metadata_field_has_owner_kind_tether() -> None:
    expected = {
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in (
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS.keys()
        )
    }
    configured = set(MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS)
    assert configured == expected

    required_kinds = {
        gate.name: set(gate.required_kinds) for gate in MODULE.GATE_SUMMARY_KINDS
    }
    for gate_name, metadata_field in sorted(configured):
        source_kinds = MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS[
            (gate_name, metadata_field)
        ]
        assert isinstance(source_kinds, tuple)
        assert source_kinds
        assert len(source_kinds) == len(set(source_kinds))
        assert set(source_kinds) <= required_kinds[gate_name]


def test_hex_list_metadata_without_owner_kind_tether_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = gate_summary("gateway_load")
    monkeypatch.delitem(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS,
        ("gateway_load", "valid_suite_report_digests"),
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_suite_report_digests source-kind tether is not configured "
        "for `gateway_load`"
    ) in errors


def test_anchor_hex_list_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    manual_cases = [
        (
            "ai_prescreen",
            "valid_executor_summary_digests",
            ("commit_reveal_executor",),
            "execution_summary_digest_hex",
        ),
        (
            "ai_prescreen",
            "valid_notification_manifest_digests",
            ("notification_transport",),
            "manifest_body_blake3_hex",
        ),
        ("ai_prescreen", "valid_policy_digests", ("runner",), "policy_digest_hex"),
        (
            "ai_prescreen",
            "valid_workflow_digests",
            ("end_to_end_workflow",),
            "workflow_digest_hex",
        ),
        (
            "appeal_finance",
            "valid_config_digests",
            ("pricing_config",),
            "config_digest_hex",
        ),
        (
            "appeal_finance",
            "valid_policy_digests",
            ("pricing_config",),
            "policy_digest_hex",
        ),
        (
            "gateway_compliance",
            "valid_catalog_digests",
            ("catalog_promotion",),
            "catalog_digest_hex",
        ),
        (
            "gateway_compliance",
            "valid_policy_digests",
            ("catalog_promotion",),
            "policy_digest_hex",
        ),
        (
            "gateway_load",
            "valid_policy_digests",
            ("staging_load",),
            "policy_digest_hex",
        ),
        (
            "gateway_load",
            "valid_staging_report_digests",
            ("staging_load",),
            "staging_report_digest_hex",
        ),
        (
            "gateway_load",
            "valid_suite_report_digests",
            ("local_conformance",),
            "suite_report_digest_hex",
        ),
        (
            "governance_dag",
            "valid_checkpoint_digests",
            ("operator_recovery",),
            "checkpoint_digest_hex",
        ),
        (
            "governance_dag",
            "valid_policy_digests",
            ("publisher_service",),
            "policy_digest_hex",
        ),
        (
            "governance_dag",
            "valid_public_head_cids",
            ("publisher_service",),
            "public_head_cid_hex",
        ),
        (
            "hedging_billing",
            "valid_policy_digests",
            ("billing_cycle",),
            "policy_digest_hex",
        ),
        (
            "hedging_billing",
            "valid_reference_decision_ids",
            ("reference_price",),
            "decision_id_hex",
        ),
        (
            "moderation_panel",
            "valid_case_digests",
            ("appeal_intake",),
            "case_digest_hex",
        ),
        (
            "moderation_panel",
            "valid_policy_digests",
            ("e2e_panel",),
            "policy_digest_hex",
        ),
        (
            "orderbook",
            "valid_contract_digests",
            ("contract_surface",),
            "contract_digest_hex",
        ),
        (
            "orderbook",
            "valid_policy_digests",
            ("contract_surface",),
            "policy_digest_hex",
        ),
        ("pdp", "valid_policy_digests", ("proof_generation",), "policy_digest_hex"),
        (
            "pdp",
            "valid_proof_summary_digests",
            ("proof_generation",),
            "proof_summary_digest_hex",
        ),
        (
            "pdp",
            "valid_provider_roster_digests",
            ("proof_generation",),
            "provider_roster_digest_hex",
        ),
        (
            "pdp",
            "valid_repair_handoff_digests",
            ("governance_repair",),
            "repair_handoff_digest_hex",
        ),
        (
            "pop_credentials",
            "valid_policy_digests",
            ("verifier_service",),
            "policy_digest_hex",
        ),
        (
            "pop_credentials",
            "valid_pop_snapshot_digests",
            ("moderation_integration",),
            "pop_snapshot_digest_hex",
        ),
        (
            "pop_credentials",
            "valid_revocation_list_digests",
            ("issuer_bundle", "revocation_registry"),
            "revocation_list_digest_hex",
        ),
        (
            "pop_credentials",
            "valid_root_digests",
            ("issuer_bundle", "commitment_root"),
            "root_digest_hex",
        ),
        ("por", "valid_policy_digests", ("randomness",), "policy_digest_hex"),
        (
            "por",
            "valid_seed_replay_digests",
            ("randomness",),
            "seed_replay_digest_hex",
        ),
        (
            "por",
            "valid_governance_archive_handoff_digests",
            ("reporting_archive",),
            "governance_archive_handoff_digest_hex",
        ),
        (
            "potr",
            "valid_policy_digests",
            ("governance_approval",),
            "policy_digest_hex",
        ),
        (
            "potr",
            "valid_pq_key_roster_digests",
            ("governance_approval",),
            "pq_key_roster_digest_hex",
        ),
        (
            "potr",
            "valid_receipt_summary_digests",
            ("multi_provider_probe",),
            "receipt_summary_digest_hex",
        ),
        (
            "potr",
            "valid_reputation_weight_policy_digests",
            ("governance_approval",),
            "reputation_weight_policy_digest_hex",
        ),
        (
            "reference_sdk_release",
            "valid_policy_digests",
            ("signed_manifest",),
            "policy_digest_hex",
        ),
        (
            "reference_sdk_release",
            "valid_archive_index_digests",
            ("release_archive",),
            "archive_index_digest_hex",
        ),
        (
            "reference_sdk_release",
            "valid_ffi_contract_digests",
            ("ffi_header_contract",),
            "ffi_contract_digest_hex",
        ),
        (
            "reference_sdk_release",
            "valid_header_digests",
            ("ffi_header_contract",),
            "header_digest_hex",
        ),
        (
            "reference_sdk_release",
            "valid_package_index_digests",
            ("downstream_bindings",),
            "package_index_digest_hex",
        ),
        (
            "reference_sdk_release",
            "valid_release_key_fingerprints",
            ("signed_manifest",),
            "public_key_fingerprint_hex",
        ),
        (
            "reference_sdk_release",
            "valid_release_manifest_digests",
            ("signed_manifest",),
            "manifest_digest_hex",
        ),
        (
            "reference_sdk_release",
            "valid_release_manifest_reference_digests",
            (
                "release_archive",
                "downstream_bindings",
                "cookbook_smoke",
                "ffi_header_contract",
                "governance_approval",
            ),
            "release_manifest_digest_hex",
        ),
        (
            "reference_sdk_release",
            "valid_smoke_output_digests",
            ("cookbook_smoke",),
            "smoke_output_digest_hex",
        ),
        (
            "reputation",
            "valid_reputation_weight_digests",
            ("publish", "latest"),
            "weights_digest_hex",
        ),
        (
            "repair",
            "valid_failure_bundle_digests",
            ("failure_capture",),
            "evidence_bundle_digest_hex",
        ),
        (
            "repair",
            "valid_handoff_digests",
            ("governance_handoff",),
            "handoff_digest_hex",
        ),
        (
            "repair",
            "valid_policy_digests",
            ("governance_handoff",),
            "policy_digest_hex",
        ),
        ("repair", "valid_roster_digests", ("auditor_roster",), "roster_digest_hex"),
        (
            "reserve_rent",
            "valid_policy_digests",
            ("policy_config",),
            "policy_digest_hex",
        ),
        ("transparency", "valid_cycle_digests", ("publication",), "cycle_digest_hex"),
        (
            "transparency",
            "valid_source_batch_digests",
            ("source_entry",),
            "source_batch_digest_hex",
        ),
    ]

    configured_cases = {
        (
            gate_name,
            metadata_field,
            owner_kinds,
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS[
                metadata_field
            ],
        )
        for (
            gate_name,
            metadata_field,
        ), owner_kinds in MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS.items()
    }
    manual_case_set = set(manual_cases)
    assert len(manual_cases) == len(manual_case_set)
    assert manual_case_set == configured_cases

    for gate_name, metadata_field, owner_kinds, fingerprint_field in sorted(
        configured_cases
    ):
        root = tmp_path / f"{gate_name}_{metadata_field}_{fingerprint_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        for owner_kind in owner_kinds:
            remove_fingerprint_metadata(
                payload,
                fingerprint_field,
                kind_name=owner_kind,
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert (
            f"{metadata_field} must match recognized artifact fingerprints"
            in errors
        )


def test_digest_list_metadata_entries_are_validated(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    bad_digest = "not-a-digest"
    payload["valid_suite_report_digests"] = [bad_digest]
    payload["valid_staging_report_digests"] = ["AB" * 32, {"digest": SHA256}]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_suite_report_digests[0] must be 64 lowercase hex characters"
        in errors
    )
    assert (
        "valid_staging_report_digests[0] must be 64 lowercase hex characters"
        in errors
    )
    assert (
        "valid_staging_report_digests[1] must be 64 lowercase hex characters"
        in errors
    )
    assert bad_digest not in errors
    assert "AB" * 32 not in errors


def test_hex_list_metadata_entries_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in sorted(
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS
        )
    ]
    assert cases

    for index, (gate_name, field) in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}_{field}_hex_entries"
        root.mkdir()
        bad_label = f"private-key-{field}"
        bad_uppercase = "AB" * 32
        bad_object = {
            "digest": SHA256,
            "private_key": f"runtime-only-{field}-key",
        }
        payload = gate_summary(gate_name)
        payload[field] = [bad_label, bad_uppercase, bad_object]
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert f"{field}[0] must be 64 lowercase hex characters" in errors
        assert f"{field}[1] must be 64 lowercase hex characters" in errors
        assert f"{field}[2] must be 64 lowercase hex characters" in errors
        assert bad_label not in result_text
        assert bad_uppercase not in result_text
        assert "private_key" not in result_text
        assert bad_object["private_key"] not in result_text


def test_digest_list_metadata_entries_must_be_unique_and_sorted(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    lower_digest = "00" * 32
    payload["valid_suite_report_digests"] = [SHA256, lower_digest]
    payload["valid_staging_report_digests"] = [SHA256, SHA256]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_suite_report_digests must be sorted in canonical order" in errors
    assert (
        "valid_staging_report_digests must not contain duplicate metadata entries"
        in errors
    )
    assert lower_digest not in errors
    assert SHA256 not in errors


def test_hex_binding_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("ai_prescreen")
    payload["valid_runner_bindings"] = [
        {
            "manifest_id_hex": "12" * 16,
            "runner_hash_hex": SHA256,
            "subject_digest_hex": SHA256,
        }
    ]
    payload["valid_workflow_digests"] = [SHA256]
    payload["valid_notification_manifest_digests"] = [SHA256]
    payload["valid_executor_summary_digests"] = [SHA256]
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(
        payload,
        manifest_id_hex="12" * 16,
        runner_hash_hex=SHA256,
        subject_digest_hex=SHA256,
        workflow_digest_hex=SHA256,
        manifest_body_blake3_hex=SHA256,
        execution_summary_digest_hex=SHA256,
        policy_digest_hex=SHA256,
    )
    write_json(tmp_path / "ai_prescreen.json", payload)

    assert run_gate(tmp_path, "--require-gate", "ai_prescreen") == 0


def test_ai_prescreen_runner_bound_artifacts_must_match_runner_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("ai_prescreen")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="committee",
        subject_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "ai_prescreen.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "ai_prescreen",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "ai_prescreen runner-bound artifact fingerprints must match "
        "valid_runner_bindings"
    ) in errors


def test_ai_prescreen_workflow_bound_artifacts_must_match_workflow_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("ai_prescreen")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="notification_transport",
        workflow_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "ai_prescreen.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "ai_prescreen",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "ai_prescreen workflow-bound artifact fingerprints must match "
        "valid_workflow_digests"
    ) in errors


def test_ai_prescreen_policy_bound_artifacts_must_match_policy_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("ai_prescreen")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_dag",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "ai_prescreen.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "ai_prescreen",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "ai_prescreen policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_gateway_compliance_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("gateway_compliance")
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, policy_digest_hex=SHA256)
    write_json(tmp_path / "gateway_compliance.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_compliance") == 0


def test_gateway_catalog_history_metadata_must_match_promotion_fingerprint(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_compliance")
    payload["valid_catalog_history_bindings"][0][
        "predecessor_catalog_digest_hex"
    ] = "cd" * 32
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_compliance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_compliance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_catalog_history_bindings entries must match recognized artifact "
        "fingerprints"
    ) in errors


def test_gateway_predecessor_bound_artifacts_must_match_full_history_tuple(
    tmp_path: Path,
) -> None:
    cases = (
        ("controller_runtime", "predecessor_catalog_digest_hex", "cd" * 32),
        ("gateway_reload", "predecessor_catalog_sequence", 7),
    )
    for index, (kind_name, fingerprint_field, forged_value) in enumerate(cases):
        root = tmp_path / f"{index}_{kind_name}_{fingerprint_field}"
        root.mkdir()
        payload = gate_summary("gateway_compliance")
        add_fingerprint_metadata(
            payload,
            kind_name=kind_name,
            **{fingerprint_field: forged_value},
        )
        summary = root / "summary.json"
        write_json(root / "gateway_compliance.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                "gateway_compliance",
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(
            json.loads(summary.read_text(encoding="utf-8"))["errors"]
        )
        assert (
            "gateway_compliance predecessor-bound artifact fingerprints must "
            "match valid_catalog_history_bindings"
        ) in errors


def test_gateway_compliance_legacy_bundle_anchor_is_rejected(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_compliance")
    payload["valid_bundle_digests"] = payload.pop("valid_catalog_digests")
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_compliance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_compliance",
            "--summary-out",
            str(summary),
        )
        == 1
    )
    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "gateway_compliance: valid_bundle_digests is not allowed in payload-free "
        "lane summary"
    ) in errors


def test_gateway_compliance_catalog_bound_artifacts_must_match_catalog_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_compliance")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="controller_runtime",
        catalog_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_compliance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_compliance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "gateway_compliance catalog-bound artifact fingerprints must match "
        "valid_catalog_digests"
    ) in errors


def test_gateway_compliance_policy_bound_artifacts_must_match_policy_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_compliance")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_compliance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_compliance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "gateway_compliance policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_gateway_compliance_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_compliance")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_compliance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_compliance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_gateway_compliance_metrics_metadata_must_cover_required_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_compliance")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_compliance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_compliance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_appeal_finance_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("appeal_finance")
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, policy_digest_hex=SHA256)
    write_json(tmp_path / "appeal_finance.json", payload)

    assert run_gate(tmp_path, "--require-gate", "appeal_finance") == 0


def test_appeal_finance_config_bound_artifacts_must_match_pricing_config(
    tmp_path: Path,
) -> None:
    payload = gate_summary("appeal_finance")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="quote_api",
        config_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "appeal_finance config-bound artifact fingerprints must match "
        "valid_config_digests"
    ) in errors


def test_appeal_finance_policy_bound_artifacts_must_match_pricing_policy(
    tmp_path: Path,
) -> None:
    payload = gate_summary("appeal_finance")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "appeal_finance policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_appeal_finance_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("appeal_finance")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="dashboard_metrics",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_appeal_finance_metrics_metadata_must_cover_required_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("appeal_finance")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="dashboard_metrics",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_gateway_load_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, policy_digest_hex=SHA256)
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 0


def test_gateway_load_suite_bound_artifacts_must_match_suite_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="staging_load",
        suite_report_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "gateway_load suite-bound artifact fingerprints must match "
        "valid_suite_report_digests"
    ) in errors


def test_gateway_load_staging_bound_artifacts_must_match_staging_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="telemetry_slo",
        staging_report_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "gateway_load staging-bound artifact fingerprints must match "
        "valid_staging_report_digests"
    ) in errors


def test_gateway_load_policy_bound_artifacts_must_match_policy_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "gateway_load policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_orderbook_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("orderbook")
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, policy_digest_hex=SHA256)
    write_json(tmp_path / "orderbook.json", payload)

    assert run_gate(tmp_path, "--require-gate", "orderbook") == 0


def test_orderbook_contract_bound_artifacts_must_match_contract_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("orderbook")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="matcher_service",
        contract_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "orderbook.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "orderbook",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "orderbook contract-bound artifact fingerprints must match "
        "valid_contract_digests"
    ) in errors


def test_orderbook_policy_bound_artifacts_must_match_policy_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("orderbook")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "orderbook.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "orderbook",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "orderbook policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_orderbook_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("orderbook")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "orderbook.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "orderbook",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_orderbook_metrics_metadata_must_cover_required_values(tmp_path: Path) -> None:
    payload = gate_summary("orderbook")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "orderbook.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "orderbook",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_pdp_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("pdp")
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, policy_digest_hex=SHA256)
    write_json(tmp_path / "pdp.json", payload)

    assert run_gate(tmp_path, "--require-gate", "pdp") == 0


def test_pdp_provider_roster_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("pdp")
    payload["valid_provider_roster_digests"] = [SHA256]
    add_fingerprint_metadata(payload, provider_roster_digest_hex=SHA256)
    write_json(tmp_path / "pdp.json", payload)

    assert run_gate(tmp_path, "--require-gate", "pdp") == 0


def test_pdp_repair_handoff_metadata_must_match_governance_repair_fingerprint(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pdp")
    remove_fingerprint_metadata(
        payload,
        "repair_handoff_digest_hex",
        kind_name="governance_repair",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pdp.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pdp",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_repair_handoff_digests must match recognized artifact fingerprints"
        in errors
    )


def test_pdp_repair_handoff_metadata_rejects_malformed_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pdp")
    payload["valid_repair_handoff_digests"] = ["not-a-digest"]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pdp.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pdp",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_repair_handoff_digests[0] must be 64 lowercase hex characters"
        in errors
    )


def test_pdp_proof_summary_bound_artifacts_must_match_proof_summary_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pdp")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="validator_replay",
        proof_summary_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pdp.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pdp",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "pdp proof-summary-bound artifact fingerprints must match "
        "valid_proof_summary_digests"
    ) in errors


def test_pdp_policy_bound_artifacts_must_match_policy_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pdp")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pdp.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pdp",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "pdp policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_pdp_provider_roster_bound_artifacts_must_match_roster_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pdp")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        provider_roster_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pdp.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pdp",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "pdp provider-roster-bound artifact fingerprints must match "
        "valid_provider_roster_digests"
    ) in errors


def test_pdp_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pdp")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pdp.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pdp",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_pdp_metrics_metadata_must_cover_required_values(tmp_path: Path) -> None:
    payload = gate_summary("pdp")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pdp.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pdp",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_por_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("por")
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, policy_digest_hex=SHA256)
    write_json(tmp_path / "por.json", payload)

    assert run_gate(tmp_path, "--require-gate", "por") == 0


def test_por_archive_backend_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("por")
    write_json(tmp_path / "por.json", payload)

    assert run_gate(tmp_path, "--require-gate", "por") == 0


def test_por_seed_replay_bound_artifacts_must_match_randomness_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("por")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="scheduler_runtime",
        seed_replay_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "por.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "por",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "por seed-replay-bound artifact fingerprints must match "
        "valid_seed_replay_digests"
    ) in errors


def test_por_policy_bound_artifacts_must_match_randomness_policy(
    tmp_path: Path,
) -> None:
    payload = gate_summary("por")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "por.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "por",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "por policy-bound artifact fingerprints must match valid_policy_digests"
        in errors
    )


def test_por_archive_backend_metadata_must_match_reporting_archive_fingerprint(
    tmp_path: Path,
) -> None:
    payload = gate_summary("por")
    remove_fingerprint_metadata(
        payload,
        "archive_backend",
        kind_name="reporting_archive",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "por.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "por",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "archive_backends must match recognized artifact fingerprints" in errors


def test_por_governance_handoff_digest_metadata_must_match_reporting_archive_fingerprint(
    tmp_path: Path,
) -> None:
    payload = gate_summary("por")
    remove_fingerprint_metadata(
        payload,
        "governance_archive_handoff_digest_hex",
        kind_name="reporting_archive",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "por.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "por",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_governance_archive_handoff_digests must match recognized "
        "artifact fingerprints"
    ) in errors


def test_por_governance_handoff_digest_metadata_rejects_malformed_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("por")
    payload["valid_governance_archive_handoff_digests"] = ["not-a-digest"]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "por.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "por",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_governance_archive_handoff_digests[0] must be 64 lowercase "
        "hex characters"
    ) in errors


def test_por_archive_backend_metadata_rejects_unknown_values(tmp_path: Path) -> None:
    payload = gate_summary("por")
    payload["archive_backends"] = ["object-store"]
    add_fingerprint_metadata(
        payload,
        archive_backend="object-store",
        kind_name="reporting_archive",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "por.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "por",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "archive_backends must not include unknown metadata values" in errors


def test_pop_credentials_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("pop_credentials")
    payload["valid_juror_sync_bindings"] = [
        {
            "synced_root_digest_hex": SHA256,
            "synced_revocation_list_digest_hex": SHA256,
        }
    ]
    payload["valid_policy_digests"] = [SHA256]
    payload["valid_pop_snapshot_digests"] = [SHA256]
    add_fingerprint_metadata(
        payload,
        policy_digest_hex=SHA256,
        pop_snapshot_digest_hex=SHA256,
        synced_root_digest_hex=SHA256,
        synced_revocation_list_digest_hex=SHA256,
    )
    write_json(tmp_path / "pop_credentials.json", payload)

    assert run_gate(tmp_path, "--require-gate", "pop_credentials") == 0


def test_pop_credentials_bound_fingerprint_field_inventories_match_lane_contract() -> None:
    assert MODULE.POP_CREDENTIALS_ROOT_BOUND_FINGERPRINT_FIELDS == tuple(
        (
            kind_name,
            (
                "synced_root_digest_hex"
                if kind_name == "juror_client"
                else "root_digest_hex"
            ),
        )
        for kind_name in MODULE.POP_CREDENTIALS_ROOT_BOUND_KINDS
    )
    assert MODULE.POP_CREDENTIALS_REVOCATION_BOUND_FINGERPRINT_FIELDS == tuple(
        (
            kind_name,
            (
                "synced_revocation_list_digest_hex"
                if kind_name == "juror_client"
                else "revocation_list_digest_hex"
            ),
        )
        for kind_name in MODULE.POP_CREDENTIALS_REVOCATION_BOUND_KINDS
    )
    root_bound_kinds = {
        kind
        for kind, _field in MODULE.POP_CREDENTIALS_ROOT_BOUND_FINGERPRINT_FIELDS
    }
    assert root_bound_kinds == set(MODULE.POP_CREDENTIALS_ROOT_BOUND_KINDS)
    assert {
        kind
        for kind, _field in MODULE.POP_CREDENTIALS_REVOCATION_BOUND_FINGERPRINT_FIELDS
    } == set(MODULE.POP_CREDENTIALS_REVOCATION_BOUND_KINDS)


def test_pop_credentials_juror_sync_root_must_match_valid_roots(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pop_credentials")
    other_digest = "cd" * 32
    payload["valid_juror_sync_bindings"][0]["synced_root_digest_hex"] = other_digest
    add_fingerprint_metadata(
        payload,
        kind_name="juror_client",
        synced_root_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pop_credentials.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pop_credentials",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "valid_juror_sync_bindings roots must match valid_root_digests" in errors


def test_pop_credentials_juror_sync_revocation_must_match_valid_revocations(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pop_credentials")
    other_digest = "cd" * 32
    payload["valid_juror_sync_bindings"][0][
        "synced_revocation_list_digest_hex"
    ] = other_digest
    add_fingerprint_metadata(
        payload,
        kind_name="juror_client",
        synced_revocation_list_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pop_credentials.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pop_credentials",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_juror_sync_bindings revocations must match "
        "valid_revocation_list_digests"
    ) in errors


def test_pop_credentials_root_bound_artifacts_must_match_valid_roots(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pop_credentials")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="verifier_service",
        root_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pop_credentials.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pop_credentials",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "pop_credentials root-bound artifact fingerprints must match "
        "valid_root_digests"
    ) in errors


def test_pop_credentials_revocation_bound_artifacts_must_match_valid_revocations(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pop_credentials")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="metrics_alerts",
        revocation_list_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pop_credentials.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pop_credentials",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "pop_credentials revocation-bound artifact fingerprints must match "
        "valid_revocation_list_digests"
    ) in errors


def test_pop_credentials_policy_bound_artifacts_must_match_verifier_policy(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pop_credentials")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pop_credentials.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pop_credentials",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "pop_credentials policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_pop_credentials_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pop_credentials")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="metrics_alerts",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pop_credentials.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pop_credentials",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_pop_credentials_metrics_metadata_must_cover_required_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("pop_credentials")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="metrics_alerts",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "pop_credentials.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "pop_credentials",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_repair_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("repair")
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, policy_digest_hex=SHA256)
    write_json(tmp_path / "repair.json", payload)

    assert run_gate(tmp_path, "--require-gate", "repair") == 0


def test_repair_roster_bound_artifacts_must_match_auditor_roster(
    tmp_path: Path,
) -> None:
    payload = gate_summary("repair")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="auditor_api",
        roster_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "repair.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "repair",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "repair roster-bound artifact fingerprints must match "
        "valid_roster_digests"
    ) in errors


def test_repair_failure_bound_artifacts_must_match_failure_capture(
    tmp_path: Path,
) -> None:
    payload = gate_summary("repair")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="worker_lifecycle",
        evidence_bundle_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "repair.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "repair",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "repair failure-bound artifact fingerprints must match "
        "valid_failure_bundle_digests"
    ) in errors


def test_repair_handoff_bound_artifacts_must_match_governance_handoff(
    tmp_path: Path,
) -> None:
    payload = gate_summary("repair")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        handoff_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "repair.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "repair",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "repair handoff-bound artifact fingerprints must match "
        "valid_handoff_digests"
    ) in errors


def test_repair_policy_bound_artifacts_must_match_handoff_policy(
    tmp_path: Path,
) -> None:
    payload = gate_summary("repair")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "repair.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "repair",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "repair policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_repair_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("repair")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "repair.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "repair",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_repair_metrics_metadata_must_cover_required_values(tmp_path: Path) -> None:
    payload = gate_summary("repair")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "repair.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "repair",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_reference_sdk_release_policy_metadata_for_gate_passes(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reference_sdk_release")
    payload["valid_archive_index_digests"] = [SHA256]
    payload["valid_ffi_contract_digests"] = [SHA256]
    payload["valid_header_digests"] = [SHA256]
    payload["valid_package_index_digests"] = [SHA256]
    payload["valid_policy_digests"] = [SHA256]
    payload["valid_release_key_fingerprints"] = [SHA256]
    payload["valid_release_manifest_digests"] = [SHA256]
    payload["valid_release_manifest_reference_digests"] = [SHA256]
    payload["valid_smoke_output_digests"] = [SHA256]
    add_fingerprint_metadata(
        payload,
        archive_index_digest_hex=SHA256,
        ffi_contract_digest_hex=SHA256,
        header_digest_hex=SHA256,
        manifest_digest_hex=SHA256,
        package_index_digest_hex=SHA256,
        policy_digest_hex=SHA256,
        public_key_fingerprint_hex=SHA256,
        release_manifest_digest_hex=SHA256,
        smoke_output_digest_hex=SHA256,
    )
    write_json(tmp_path / "reference_sdk_release.json", payload)

    assert run_gate(tmp_path, "--require-gate", "reference_sdk_release") == 0


def test_reference_sdk_release_manifest_bound_artifacts_must_match_signed_manifest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reference_sdk_release")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="release_archive",
        release_manifest_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reference_sdk_release.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reference_sdk_release",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reference_sdk_release manifest-bound artifact fingerprints must match "
        "valid_release_manifest_digests"
    ) in errors


def test_reference_sdk_release_policy_bound_artifacts_must_match_signed_manifest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reference_sdk_release")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reference_sdk_release.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reference_sdk_release",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reference_sdk_release policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_reference_sdk_release_governance_key_fingerprint_must_match_signed_manifest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reference_sdk_release")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        public_key_fingerprint_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reference_sdk_release.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reference_sdk_release",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reference_sdk_release governance approval release-key fingerprints "
        "must match valid_release_key_fingerprints"
    ) in errors


def test_reference_sdk_release_signature_algorithm_metadata_for_gate_passes(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reference_sdk_release")
    write_json(tmp_path / "reference_sdk_release.json", payload)

    assert run_gate(tmp_path, "--require-gate", "reference_sdk_release") == 0


def test_reference_sdk_release_signature_algorithm_must_match_signed_manifest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reference_sdk_release")
    remove_fingerprint_metadata(
        payload,
        "signature_algorithm",
        kind_name="signed_manifest",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reference_sdk_release.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reference_sdk_release",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "signature_algorithms must match recognized artifact fingerprints" in errors


def test_reference_sdk_release_signature_algorithm_rejects_unknown_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reference_sdk_release")
    payload["signature_algorithms"] = ["rsa-sha256"]
    add_fingerprint_metadata(
        payload,
        signature_algorithm="rsa-sha256",
        kind_name="signed_manifest",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reference_sdk_release.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reference_sdk_release",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "signature_algorithms must not include unknown metadata values" in errors


def test_closed_set_string_metadata_unknown_values_do_not_echo(
    tmp_path: Path,
) -> None:
    cases = (
        (
            "por",
            "archive_backends",
            "reporting_archive",
            "archive_backend",
            "private-key-placeholder",
            "archive_backends must not include unknown metadata values",
        ),
        (
            "reference_sdk_release",
            "signature_algorithms",
            "signed_manifest",
            "signature_algorithm",
            "private-key-placeholder",
            "signature_algorithms must not include unknown metadata values",
        ),
    )

    for gate_name, metadata_field, kind_name, fingerprint_field, raw_value, error in cases:
        root = tmp_path / gate_name
        root.mkdir()
        payload = gate_summary(gate_name)
        payload[metadata_field] = [raw_value]
        add_fingerprint_metadata(
            payload,
            kind_name=kind_name,
            **{fingerprint_field: raw_value},
        )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert error in errors
        assert raw_value not in result_text


def test_transparency_publication_binding_metadata_for_gate_passes(
    tmp_path: Path,
) -> None:
    payload = gate_summary("transparency")
    write_json(tmp_path / "transparency.json", payload)

    assert run_gate(tmp_path, "--require-gate", "transparency") == 0


def test_transparency_publication_binding_must_match_publication_fingerprint(
    tmp_path: Path,
) -> None:
    payload = gate_summary("transparency")
    remove_fingerprint_metadata(
        payload,
        "source_batch_digest_hex",
        "cycle_digest_hex",
        kind_name="publication",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "transparency.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "transparency",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "valid_publication_bindings must match recognized artifact fingerprints" in errors


def test_transparency_cycle_digest_must_match_publication_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("transparency")
    other_digest = "cd" * 32
    payload["valid_cycle_digests"] = [other_digest]
    add_fingerprint_metadata(
        payload,
        cycle_digest_hex=other_digest,
        kind_name="publication",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "transparency.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "transparency",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_cycle_digests must match valid_publication_bindings cycle digests"
        in errors
    )


def test_transparency_publication_binding_source_must_match_source_entry(
    tmp_path: Path,
) -> None:
    payload = gate_summary("transparency")
    other_digest = "cd" * 32
    payload["valid_publication_bindings"] = [
        {
            "source_batch_digest_hex": other_digest,
            "cycle_digest_hex": SHA256,
        }
    ]
    add_fingerprint_metadata(
        payload,
        source_batch_digest_hex=other_digest,
        cycle_digest_hex=SHA256,
        kind_name="publication",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "transparency.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "transparency",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_publication_bindings source batches must match valid_source_batch_digests"
        in errors
    )


def test_transparency_source_bound_artifacts_must_match_source_batch_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("transparency")
    other_digest = "cd" * 32
    payload["valid_publication_bindings"] = [
        {
            "source_batch_digest_hex": other_digest,
            "cycle_digest_hex": SHA256,
        }
    ]
    add_fingerprint_metadata(
        payload,
        source_batch_digest_hex=other_digest,
        cycle_digest_hex=SHA256,
        kind_name="publication",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "transparency.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "transparency",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "transparency source-bound artifact fingerprints must match "
        "valid_source_batch_digests"
    ) in errors


def test_transparency_cycle_bound_artifacts_must_match_publication_cycle(
    tmp_path: Path,
) -> None:
    payload = gate_summary("transparency")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="privacy_aggregate",
        cycle_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "transparency.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "transparency",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "transparency cycle-bound artifact fingerprints must match "
        "valid_cycle_digests"
    ) in errors


def test_potr_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("potr")
    payload["valid_policy_digests"] = [SHA256]
    payload["valid_pq_key_roster_digests"] = [SHA256]
    payload["valid_receipt_summary_digests"] = [SHA256]
    payload["valid_reputation_weight_policy_digests"] = [SHA256]
    add_fingerprint_metadata(
        payload,
        policy_digest_hex=SHA256,
        pq_key_roster_digest_hex=SHA256,
        receipt_summary_digest_hex=SHA256,
        reputation_weight_policy_digest_hex=SHA256,
    )
    write_json(tmp_path / "potr.json", payload)

    assert run_gate(tmp_path, "--require-gate", "potr") == 0


def test_potr_receipt_summary_bound_artifacts_must_match_receipt_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("potr")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="receipt_validation",
        receipt_summary_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "potr.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "potr",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "potr receipt-summary-bound artifact fingerprints must match "
        "valid_receipt_summary_digests"
    ) in errors


def test_potr_pq_key_roster_bound_artifacts_must_match_governance_roster(
    tmp_path: Path,
) -> None:
    payload = gate_summary("potr")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="receipt_validation",
        pq_key_roster_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "potr.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "potr",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "potr pq-key-roster-bound artifact fingerprints must match "
        "valid_pq_key_roster_digests"
    ) in errors


def test_potr_reputation_weight_bound_artifacts_must_match_governance_policy(
    tmp_path: Path,
) -> None:
    payload = gate_summary("potr")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="reputation_integration",
        reputation_weight_policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "potr.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "potr",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "potr reputation-weight-bound artifact fingerprints must match "
        "valid_reputation_weight_policy_digests"
    ) in errors


def test_potr_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("potr")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "potr.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "potr",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_potr_metrics_metadata_must_cover_required_values(tmp_path: Path) -> None:
    payload = gate_summary("potr")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "potr.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "potr",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_hex_binding_metadata_must_match_recognized_artifact_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("ai_prescreen")
    payload["valid_runner_bindings"] = [
        {
            "manifest_id_hex": "12" * 16,
            "runner_hash_hex": SHA256,
            "subject_digest_hex": SHA256,
        }
    ]
    remove_fingerprint_metadata(
        payload,
        "manifest_id_hex",
        "runner_hash_hex",
        "subject_digest_hex",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "ai_prescreen.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "ai_prescreen",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_runner_bindings must match recognized artifact fingerprints"
        in errors
    )
    assert SHA256 not in errors


def test_every_hex_binding_metadata_field_has_owner_kind_tether() -> None:
    expected = {
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in (
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_FIELDS.keys()
        )
    }
    configured = set(MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS)
    assert configured == expected

    required_kinds = {
        gate.name: set(gate.required_kinds) for gate in MODULE.GATE_SUMMARY_KINDS
    }
    for gate_name, metadata_field in sorted(configured):
        source_kinds = MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS[
            (gate_name, metadata_field)
        ]
        assert isinstance(source_kinds, tuple)
        assert source_kinds
        assert len(source_kinds) == len(set(source_kinds))
        assert set(source_kinds) <= required_kinds[gate_name]


def test_hex_binding_metadata_without_owner_kind_tether_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = gate_summary("ai_prescreen")
    monkeypatch.delitem(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS,
        ("ai_prescreen", "valid_runner_bindings"),
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "ai_prescreen.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "ai_prescreen",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_runner_bindings source-kind tether is not configured "
        "for `ai_prescreen`"
    ) in errors


def test_hex_binding_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    manual_cases = [
        (
            "ai_prescreen",
            "valid_runner_bindings",
            ("runner",),
            (
                "manifest_id_hex",
                "runner_hash_hex",
                "subject_digest_hex",
            ),
        ),
        (
            "hedging_billing",
            "valid_cycle_bindings",
            ("billing_cycle",),
            (
                "statement_bundle_digest_hex",
                "reconciliation_digest_hex",
            ),
        ),
        (
            "moderation_panel",
            "valid_roster_bindings",
            ("sortition_roster",),
            (
                "case_digest_hex",
                "roster_hash_hex",
            ),
        ),
        (
            "moderation_panel",
            "valid_tally_bindings",
            ("commit_reveal",),
            (
                "case_digest_hex",
                "roster_hash_hex",
                "tally_digest_hex",
            ),
        ),
        (
            "pop_credentials",
            "valid_juror_sync_bindings",
            ("juror_client",),
            (
                "synced_root_digest_hex",
                "synced_revocation_list_digest_hex",
            ),
        ),
        (
            "reputation",
            "valid_snapshot_bindings",
            ("publish", "latest"),
            (
                "snapshot_id_hex",
                "merkle_root_hex",
            ),
        ),
        (
            "reserve_rent",
            "valid_policy_matrix_bindings",
            ("quote_matrix",),
            (
                "policy_digest_hex",
                "matrix_digest_hex",
            ),
        ),
        (
            "reserve_rent",
            "valid_policy_matrix_ledger_bindings",
            ("ledger_digest",),
            (
                "policy_digest_hex",
                "matrix_digest_hex",
                "ledger_digest_hex",
            ),
        ),
        (
            "transparency",
            "valid_publication_bindings",
            ("publication",),
            (
                "source_batch_digest_hex",
                "cycle_digest_hex",
            ),
        ),
    ]

    configured_cases = {
        (
            gate_name,
            metadata_field,
            owner_kinds,
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_FIELDS[
                metadata_field
            ],
        )
        for (
            gate_name,
            metadata_field,
        ), owner_kinds in MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS.items()
    }
    manual_case_set = set(manual_cases)
    assert len(manual_cases) == len(manual_case_set)
    assert manual_case_set == configured_cases

    for gate_name, metadata_field, owner_kinds, fingerprint_fields in sorted(
        configured_cases
    ):
        fingerprint_suffix = "_".join(fingerprint_fields)
        root = tmp_path / f"{gate_name}_{metadata_field}_{fingerprint_suffix}"
        root.mkdir()
        payload = gate_summary(gate_name)
        for owner_kind in owner_kinds:
            remove_fingerprint_metadata(
                payload,
                *fingerprint_fields,
                kind_name=owner_kind,
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{metadata_field} must match recognized artifact fingerprints" in errors


def test_hex_binding_metadata_entries_are_validated(tmp_path: Path) -> None:
    payload = gate_summary("reputation")
    bad_snapshot_id = "AB" * 16
    payload["valid_snapshot_bindings"] = [
        {
            "snapshot_id_hex": bad_snapshot_id,
            "merkle_root_hex": SHA256,
            "raw_response": "runtime-only-body",
        },
        {"snapshot_id_hex": SNAPSHOT_ID},
        "not-a-binding",
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_snapshot_bindings[0].snapshot_id_hex must be 32 lowercase hex characters"
        in errors
    )
    assert (
        "valid_snapshot_bindings[0].<sensitive-key> is not allowed in payload-free binding metadata"
        in errors
    )
    assert (
        "valid_snapshot_bindings[1].merkle_root_hex must be 64 lowercase hex characters"
        in errors
    )
    assert "valid_snapshot_bindings[2] must be a payload-free binding object" in errors
    assert bad_snapshot_id not in errors
    assert "raw_response" not in errors
    assert "runtime-only-body" not in errors
    assert "not-a-binding" not in errors


def test_hex_binding_metadata_entries_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, field, MODULE.PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS[field])
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in sorted(
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS.keys()
        )
    ]
    assert cases

    for index, (gate_name, field, binding_fields) in enumerate(cases):
        base_item = copy.deepcopy(gate_summary(gate_name)[field][0])
        for key, expected_hex_length in sorted(binding_fields.items()):
            root = tmp_path / f"{index}_{gate_name}_{field}_{key}_binding_entries"
            root.mkdir()
            uppercase_value = "AB" * (expected_hex_length // 2)
            bad_item = copy.deepcopy(base_item)
            bad_item[key] = uppercase_value
            bad_item["private_key"] = f"runtime-only-{field}-{key}-key"
            missing_item = copy.deepcopy(base_item)
            del missing_item[key]
            non_object = f"private-key-{field}-{key}"
            payload = gate_summary(gate_name)
            payload[field] = [bad_item, missing_item, non_object]
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert (
                f"{field}[0].{key} must be {expected_hex_length} lowercase hex characters"
                in errors
            )
            assert (
                f"{field}[1].{key} must be {expected_hex_length} lowercase hex characters"
                in errors
            )
            assert f"{field}[2] must be a payload-free binding object" in errors
            assert (
                f"{field}[0].<sensitive-key> is not allowed in payload-free binding metadata"
                in errors
            )
            assert uppercase_value not in result_text
            assert "private_key" not in result_text
            assert bad_item["private_key"] not in result_text
            assert non_object not in result_text


def test_binding_metadata_entries_must_be_unique_and_sorted(
    tmp_path: Path,
) -> None:
    high_binding = {
        "snapshot_id_hex": "ff" * 16,
        "merkle_root_hex": SHA256,
    }
    low_binding = {
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": SHA256,
    }
    payload = gate_summary("reputation")
    payload["valid_snapshot_bindings"] = [high_binding, low_binding, low_binding]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_snapshot_bindings must be sorted in canonical order" in errors
    assert (
        "valid_snapshot_bindings must not contain duplicate metadata entries"
        in errors
    )
    assert "ff" * 16 not in errors
    assert SNAPSHOT_ID not in errors


def test_reputation_snapshot_binding_must_match_scalar_snapshot_metadata(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    other_merkle_root = "ef" * 32
    payload["valid_snapshot_bindings"][0]["merkle_root_hex"] = other_merkle_root
    add_fingerprint_metadata(
        payload,
        kind_name="latest",
        merkle_root_hex=other_merkle_root,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_snapshot_bindings must match snapshot_id_hex and merkle_root_hex"
        in errors
    )


def test_reputation_snapshot_bound_artifacts_must_match_snapshot_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    other_merkle_root = "ef" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="provider",
        merkle_root_hex=other_merkle_root,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reputation snapshot-bound artifact fingerprints must match "
        "valid_snapshot_bindings"
    ) in errors


def test_reputation_weight_metadata_must_match_publish_latest_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    for kind_name in ("publish", "latest"):
        remove_fingerprint_metadata(
            payload,
            "weights_digest_hex",
            kind_name=kind_name,
        )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "valid_reputation_weight_digests must match recognized artifact fingerprints" in errors
    assert (
        "reputation publish/latest artifact fingerprints must match "
        "valid_reputation_weight_digests"
    ) in errors


def test_reputation_weight_metadata_rejects_malformed_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    bad_digest = "AB" * 32
    payload["valid_reputation_weight_digests"] = [bad_digest]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_reputation_weight_digests[0] must be 64 lowercase hex characters"
        in errors
    )
    assert bad_digest not in errors


def test_reputation_publish_latest_weight_artifacts_must_match_metadata(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    other_digest = "ef" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="latest",
        weights_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reputation publish/latest artifact fingerprints must match "
        "valid_reputation_weight_digests"
    ) in errors
    assert other_digest not in errors


def test_reference_decision_id_metadata_entries_are_validated(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    bad_decision_id = "decision-private-key-placeholder"
    payload["valid_reference_decision_ids"] = [SHA256, bad_decision_id, "AB" * 32]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_reference_decision_ids[1] must be 64 lowercase hex characters"
        in errors
    )
    assert (
        "valid_reference_decision_ids[2] must be 64 lowercase hex characters"
        in errors
    )
    assert bad_decision_id not in errors
    assert "AB" * 32 not in errors


def test_public_head_cid_metadata_entries_are_validated(tmp_path: Path) -> None:
    payload = gate_summary("governance_dag")
    bad_public_head = "cid-private-key-placeholder"
    payload["valid_public_head_cids"] = [SHA256, bad_public_head, "AB" * 32]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "governance_dag.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "governance_dag",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_public_head_cids[1] must be 64 lowercase hex characters" in errors
    assert "valid_public_head_cids[2] must be 64 lowercase hex characters" in errors
    assert bad_public_head not in errors
    assert "AB" * 32 not in errors


def test_governance_dag_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("governance_dag")
    payload["valid_checkpoint_digests"] = [SHA256]
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(
        payload,
        checkpoint_digest_hex=SHA256,
        policy_digest_hex=SHA256,
    )
    write_json(tmp_path / "governance_dag.json", payload)

    assert run_gate(tmp_path, "--require-gate", "governance_dag") == 0


def test_governance_dag_public_head_bound_artifacts_must_match_public_head(
    tmp_path: Path,
) -> None:
    payload = gate_summary("governance_dag")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="mirror_datastore",
        public_head_cid_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "governance_dag.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "governance_dag",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "governance_dag public-head-bound artifact fingerprints must match "
        "valid_public_head_cids"
    ) in errors


def test_governance_dag_policy_bound_artifacts_must_match_policy_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("governance_dag")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "governance_dag.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "governance_dag",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "governance_dag policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_hedging_billing_policy_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("hedging_billing")
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, policy_digest_hex=SHA256)
    write_json(tmp_path / "hedging_billing.json", payload)

    assert run_gate(tmp_path, "--require-gate", "hedging_billing") == 0


def test_hedging_cycle_bound_artifacts_must_match_billing_cycles(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="statement_publication",
        statement_bundle_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "hedging_billing cycle-bound artifact fingerprints must match "
        "valid_cycle_bindings"
    ) in errors


def test_hedging_policy_bound_artifacts_must_match_billing_cycles(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "hedging_billing policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_hedging_cycle_bindings_must_match_billing_cycles(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    other_digest = "cd" * 32
    append_hedging_billing_cycle(
        payload,
        cycle_id="billing-cycle-2",
        cycle_index=2,
        artifact_sha256="ef" * 32,
        statement_bundle_digest_hex=other_digest,
        reconciliation_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_cycle_bindings must match valid_billing_cycles cycle tuples" in errors
    )


def test_hedging_policy_digests_must_match_billing_cycles(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    append_hedging_billing_cycle(
        payload,
        cycle_id="billing-cycle-2",
        cycle_index=2,
        artifact_sha256="ef" * 32,
        policy_digest_hex="cd" * 32,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_policy_digests must match valid_billing_cycles policy digests"
        in errors
    )


def test_hedging_billing_cycle_reference_must_match_reference_decision(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    other_digest = "cd" * 32
    payload["valid_billing_cycles"][0]["reference_decision_id_hex"] = other_digest
    add_fingerprint_metadata(
        payload,
        kind_name="billing_cycle",
        reference_decision_id_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_billing_cycles reference decisions must match "
        "valid_reference_decision_ids"
    ) in errors


def test_hedging_billing_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="metrics_alerts",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_hedging_billing_metrics_metadata_must_cover_required_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="metrics_alerts",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_provider_count_values_metadata_entries_are_positive_ints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    payload["provider_count_values"] = [2, 0, -1, True, "3"]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "provider_count_values[1] must be a positive integer" in errors
    assert "provider_count_values[2] must be a positive integer" in errors
    assert "provider_count_values[3] must be a positive integer" in errors
    assert "provider_count_values[4] must be a positive integer" in errors
    assert '"3"' not in errors


def test_positive_int_list_metadata_entries_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in sorted(
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS
        )
    ]
    assert cases

    for index, (gate_name, field) in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}_{field}_positive_int_entries"
        root.mkdir()
        bad_string = f"private-key-{field}"
        bad_object = {"private_key": f"runtime-only-{field}-key"}
        payload = gate_summary(gate_name)
        payload[field] = [1, 0, -1, True, bad_string, bad_object]
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        for item_index in range(1, 6):
            assert f"{field}[{item_index}] must be a positive integer" in errors
        assert bad_string not in result_text
        assert "private_key" not in result_text
        assert bad_object["private_key"] not in result_text


def test_provider_count_values_metadata_must_be_unique_and_sorted(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    payload["provider_count_values"] = [3, 2, 2]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "provider_count_values must be sorted in canonical order" in errors
    assert "provider_count_values must not contain duplicate metadata entries" in errors


def test_provider_ids_metadata_entries_are_canonical_strings(tmp_path: Path) -> None:
    payload = gate_summary("reputation")
    payload["provider_ids"] = ["provider-a", "", True, 7]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "provider_ids[1] must be a canonical string" in errors
    assert "provider_ids[2] must be a canonical string" in errors
    assert "provider_ids[3] must be a canonical string" in errors
    assert '""' not in errors


def test_reputation_provider_metadata_must_match_recognized_artifact_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    remove_fingerprint_metadata(payload, "provider_id", "provider_count")
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "provider_ids must match recognized artifact fingerprints" in errors
    assert "provider_count_values must match recognized artifact fingerprints" in errors


def test_provider_ids_secret_like_metadata_does_not_echo(tmp_path: Path) -> None:
    raw_provider_id = "provider-private-key-placeholder"
    payload = gate_summary("reputation")
    payload["provider_ids"] = [raw_provider_id]
    add_fingerprint_metadata(
        payload,
        kind_name="provider",
        provider_id=raw_provider_id,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result_text = summary.read_text(encoding="utf-8")
    errors = "\n".join(json.loads(result_text)["errors"])
    assert (
        "provider_ids[0] must not contain non-production markers "
        "['placeholder', 'private']"
    ) in errors
    assert raw_provider_id not in result_text


def test_provider_ids_compact_non_production_marker_does_not_echo(
    tmp_path: Path,
) -> None:
    raw_provider_id = "provider-prodplaceholderreview"
    payload = gate_summary("reputation")
    payload["provider_ids"] = [raw_provider_id]
    add_fingerprint_metadata(
        payload,
        kind_name="provider",
        provider_id=raw_provider_id,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result_text = summary.read_text(encoding="utf-8")
    errors = "\n".join(json.loads(result_text)["errors"])
    assert (
        "provider_ids[0] must not contain non-production markers ['placeholder']"
        in errors
    )
    assert raw_provider_id not in result_text


def test_every_string_list_metadata_field_has_owner_kind_tether() -> None:
    expected = {
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in (
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS.keys()
        )
    }
    configured = set(MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS)
    assert configured == expected

    required_kinds = {
        gate.name: set(gate.required_kinds) for gate in MODULE.GATE_SUMMARY_KINDS
    }
    for gate_name, metadata_field in sorted(configured):
        source_kinds = MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS[
            (gate_name, metadata_field)
        ]
        assert isinstance(source_kinds, tuple)
        assert source_kinds
        assert len(source_kinds) == len(set(source_kinds))
        assert set(source_kinds) <= required_kinds[gate_name]


def test_every_string_array_list_metadata_field_has_owner_kind_tether() -> None:
    expected = {
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in (
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS.keys()
        )
    }
    configured = set(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS
    )
    assert configured == expected

    required_kinds = {
        gate.name: set(gate.required_kinds) for gate in MODULE.GATE_SUMMARY_KINDS
    }
    for gate_name, metadata_field in sorted(configured):
        source_kinds = (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS[
                (gate_name, metadata_field)
            ]
        )
        assert isinstance(source_kinds, tuple)
        assert source_kinds
        assert len(source_kinds) == len(set(source_kinds))
        assert set(source_kinds) <= required_kinds[gate_name]


def test_string_list_metadata_without_owner_kind_tether_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = gate_summary("reputation")
    monkeypatch.delitem(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS,
        ("reputation", "provider_ids"),
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "provider_ids source-kind tether is not configured for `reputation`" in errors


def test_string_array_list_metadata_without_owner_kind_tether_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = gate_summary("reputation")
    monkeypatch.delitem(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS,
        ("reputation", "metrics"),
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics source-kind tether is not configured for `reputation`" in errors


def test_payload_free_metadata_owner_kind_tethers_fail_closed_from_config(
    tmp_path: Path,
    monkeypatch,
) -> None:
    source_kind_maps = {
        "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS": dict(
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS
        ),
        "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS": dict(
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS
        ),
        "PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS": dict(
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS
        ),
        "PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS": dict(
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS
        ),
        "PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS": dict(
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS
        ),
        "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS": dict(
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS
        ),
    }
    assert all(source_kind_maps.values())

    for map_name, original_tethers in source_kind_maps.items():
        for gate_name, field in sorted(original_tethers):
            root = tmp_path / f"{map_name}_{gate_name}_{field}_missing"
            root.mkdir()
            for reset_map_name, reset_tethers in source_kind_maps.items():
                monkeypatch.setattr(
                    MODULE,
                    reset_map_name,
                    dict(reset_tethers),
                )
            monkeypatch.setattr(
                MODULE,
                map_name,
                {
                    key: value
                    for key, value in original_tethers.items()
                    if key != (gate_name, field)
                },
            )
            payload = gate_summary(gate_name)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            errors = "\n".join(
                json.loads(summary.read_text(encoding="utf-8"))["errors"]
            )
            assert (
                f"{field} source-kind tether is not configured for `{gate_name}`"
                in errors
            )


def test_string_list_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    cases = [
        (
            gate_name,
            metadata_field,
            owner_kinds,
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS[
                metadata_field
            ],
        )
        for (
            gate_name,
            metadata_field,
        ), owner_kinds in MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS.items()
    ]

    for gate_name, metadata_field, owner_kinds, fingerprint_field in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        for owner_kind in owner_kinds:
            remove_fingerprint_metadata(
                payload,
                fingerprint_field,
                kind_name=owner_kind,
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{metadata_field} must match recognized artifact fingerprints" in errors


def test_string_array_list_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    cases = [
        (
            gate_name,
            metadata_field,
            owner_kinds,
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS[
                metadata_field
            ],
        )
        for (
            gate_name,
            metadata_field,
        ), owner_kinds in MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS.items()
    ]

    for gate_name, metadata_field, owner_kinds, fingerprint_field in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}_{fingerprint_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        for owner_kind in owner_kinds:
            remove_fingerprint_metadata(
                payload,
                fingerprint_field,
                kind_name=owner_kind,
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{metadata_field} must match recognized artifact fingerprints" in errors


def test_string_list_metadata_entries_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate_name, field)
        for source_kind_map in (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS,
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS,
        )
        for gate_name, field in source_kind_map
    ]
    assert cases

    for index, (gate_name, field) in enumerate(sorted(cases)):
        root = tmp_path / f"{index}_{gate_name}_{field}_string_entries"
        root.mkdir()
        bad_string = f" private-key-{field}"
        bad_object = {"private_key": f"runtime-only-{field}-key"}
        payload = gate_summary(gate_name)
        payload[field] = [bad_string, True, bad_object]
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert f"{field}[0] must be a canonical string" in errors
        assert f"{field}[1] must be a canonical string" in errors
        assert f"{field}[2] must be a canonical string" in errors
        assert bad_string not in result_text
        assert "private_key" not in result_text
        assert bad_object["private_key"] not in result_text


def test_required_string_list_metadata_must_cover_configured_values(
    tmp_path: Path,
) -> None:
    cases = [
        (gate_name, metadata_field, tuple(required_values))
        for (
            gate_name,
            metadata_field,
        ), required_values in MODULE.PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES.items()
    ]

    for gate_name, metadata_field, required_values in sorted(cases):
        assert required_values
        root = tmp_path / f"{gate_name}_{metadata_field}_missing"
        root.mkdir()
        payload = gate_summary(gate_name)
        missing = required_values[0]
        payload[metadata_field] = [
            value for value in payload[metadata_field] if value != missing
        ]
        count_field = MODULE.PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS.get(
            (gate_name, metadata_field)
        )
        if count_field is not None:
            count_value = len(set(payload[metadata_field]))
            payload[count_field] = [count_value]
            fingerprint_field = (
                MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS[
                    count_field
                ]
            )
            for owner_kind in (
                MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS[
                    (gate_name, count_field)
                ]
            ):
                add_fingerprint_metadata(
                    payload,
                    kind_name=owner_kind,
                    **{fingerprint_field: count_value},
                )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{metadata_field} must include metadata value `{missing}`" in errors


def test_required_string_list_metadata_rejects_unknown_values(
    tmp_path: Path,
) -> None:
    cases = [
        (gate_name, metadata_field, tuple(required_values))
        for (
            gate_name,
            metadata_field,
        ), required_values in MODULE.PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES.items()
    ]

    for gate_name, metadata_field, _required_values in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}_unknown"
        root.mkdir()
        payload = gate_summary(gate_name)
        unknown = f"sorafs_{gate_name}_{metadata_field}_unknown_total"
        payload[metadata_field] = sorted([*payload[metadata_field], unknown])
        count_field = MODULE.PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS.get(
            (gate_name, metadata_field)
        )
        fingerprint_metadata = {}
        if count_field is not None:
            count_value = len(set(payload[metadata_field]))
            payload[count_field] = [count_value]
            count_fingerprint_field = (
                MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS[
                    count_field
                ]
            )
            fingerprint_metadata[count_fingerprint_field] = count_value
        if (
            gate_name,
            metadata_field,
        ) in MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS:
            source_kinds = (
                MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS[
                    (gate_name, metadata_field)
                ]
            )
            fingerprint_field = (
                MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS[
                    metadata_field
                ]
            )
            fingerprint_metadata[fingerprint_field] = payload[metadata_field]
        else:
            source_kinds = (
                MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS[
                    (gate_name, metadata_field)
                ]
            )
            fingerprint_field = (
                MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS[
                    metadata_field
                ]
            )
            fingerprint_metadata[fingerprint_field] = unknown
        for source_kind in source_kinds:
            append_required_artifact(
                payload,
                source_kind,
                suffix=f"{metadata_field}-unknown",
                sha256="bc" * 32,
                **fingerprint_metadata,
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{metadata_field} must not include unknown metadata values" in errors
        assert unknown not in errors


def test_allowed_string_list_metadata_rejects_unknown_values(
    tmp_path: Path,
) -> None:
    cases = [
        (gate_name, metadata_field, tuple(allowed_values))
        for (
            gate_name,
            metadata_field,
        ), allowed_values in MODULE.PAYLOAD_FREE_SUMMARY_ALLOWED_STRING_LIST_VALUES.items()
    ]

    for gate_name, metadata_field, _allowed_values in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}_allowed_unknown"
        root.mkdir()
        payload = gate_summary(gate_name)
        unknown = f"{metadata_field}-unknown"
        payload[metadata_field] = sorted([*payload[metadata_field], unknown])
        fingerprint_field = (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS[
                metadata_field
            ]
        )
        for source_kind in (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS[
                (gate_name, metadata_field)
            ]
        ):
            append_required_artifact(
                payload,
                source_kind,
                suffix=f"{metadata_field}-unknown",
                sha256="bc" * 32,
                **{fingerprint_field: unknown},
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{metadata_field} must not include unknown metadata values" in errors
        assert unknown not in errors


def test_string_list_count_bindings_must_match_configured_unique_counts(
    tmp_path: Path,
) -> None:
    cases = [
        (gate_name, metadata_field, count_field)
        for (
            gate_name,
            metadata_field,
        ), count_field in MODULE.PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS.items()
    ]

    for gate_name, metadata_field, count_field in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}_{count_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        count_value = len(set(payload[metadata_field])) + 1
        payload[count_field] = [count_value]
        fingerprint_field = (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS[
                count_field
            ]
        )
        for owner_kind in (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS[
                (gate_name, count_field)
            ]
        ):
            add_fingerprint_metadata(
                payload,
                kind_name=owner_kind,
                **{fingerprint_field: count_value},
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{count_field} must include the unique {metadata_field} count" in errors


def test_ordered_list_metadata_must_be_unique_and_sorted_from_config(
    tmp_path: Path,
) -> None:
    def hex_value_before(value: str) -> str:
        candidate = "00" * (len(value) // 2)
        if candidate < value:
            return candidate
        return "ff" * (len(value) // 2)

    def string_value_before(value: str) -> str:
        candidate = "aaa-ordered-probe"
        if candidate < value:
            return candidate
        return "zzz-ordered-probe"

    def binding_value_before(field: str, item: dict) -> dict:
        binding_fields = MODULE.PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS[field]
        candidate = copy.deepcopy(item)
        first_field, expected_hex_length = next(iter(binding_fields.items()))
        current = candidate[first_field]
        lower = "00" * (expected_hex_length // 2)
        candidate[first_field] = (
            lower if lower < current else "ff" * (expected_hex_length // 2)
        )
        return candidate

    def duplicate_and_unsorted_items(field: str, payload: dict) -> tuple[list, list]:
        first = copy.deepcopy(payload[field][0])
        duplicate = [copy.deepcopy(first), copy.deepcopy(first)]
        if field in MODULE.PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS:
            other = binding_value_before(field, first)
            unsorted = [first, other] if first != other else duplicate
        elif field in MODULE.PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS:
            unsorted = [2, 1]
        elif field in MODULE.PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS:
            other = hex_value_before(first)
            unsorted = [first, other] if other < first else [other, first]
        else:
            other = string_value_before(first)
            unsorted = [first, other] if other < first else [other, first]
        assert duplicate[0] == duplicate[1]
        if field in MODULE.PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS:
            assert unsorted[0] != unsorted[1]
        else:
            assert unsorted != sorted(unsorted)
        return duplicate, unsorted

    cases = [
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in sorted(
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS
        )
    ]
    assert cases

    for gate_name, field in cases:
        for suffix, expected_error, mutation_index in (
            ("duplicate", f"{field} must not contain duplicate metadata entries", 0),
            ("unsorted", f"{field} must be sorted in canonical order", 1),
        ):
            root = tmp_path / f"{gate_name}_{field}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload[field] = duplicate_and_unsorted_items(field, payload)[
                mutation_index
            ]
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
            assert expected_error in errors


def test_reputation_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    remove_fingerprint_metadata(payload, "metric_count", "metrics", kind_name="metrics")
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_reputation_metrics_metadata_must_cover_required_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="metrics",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_reputation_metrics_metadata_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    payload["metrics"] = sorted(
        [*MODULE.REPUTATION_REQUIRED_METRICS, "sorafs_reputation_unknown_total"]
    )
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="metrics",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must not include unknown metadata values" in errors


def test_reputation_metrics_metadata_must_be_unique_sorted_and_counted(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    payload["metrics"] = [
        MODULE.REPUTATION_REQUIRED_METRICS[1],
        MODULE.REPUTATION_REQUIRED_METRICS[0],
        MODULE.REPUTATION_REQUIRED_METRICS[0],
    ]
    payload["metric_count_values"] = [len(MODULE.REPUTATION_REQUIRED_METRICS) + 1]
    add_fingerprint_metadata(
        payload,
        metric_count=payload["metric_count_values"][0],
        metrics=payload["metrics"],
        kind_name="metrics",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must not contain duplicate metadata entries" in errors
    assert "metrics must be sorted in canonical order" in errors
    assert "metric_count_values must include the unique metrics count" in errors


def test_governance_dag_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("governance_dag")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "governance_dag.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "governance_dag",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_governance_dag_metrics_metadata_must_cover_required_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("governance_dag")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "governance_dag.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "governance_dag",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_gateway_load_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="telemetry_slo",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_por_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("por")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="observability",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "por.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "por",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_every_positive_int_list_metadata_field_has_owner_kind_tether() -> None:
    expected = {
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in (
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS.keys()
        )
    }
    configured = set(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS
    )
    assert configured == expected

    required_kinds = {
        gate.name: set(gate.required_kinds) for gate in MODULE.GATE_SUMMARY_KINDS
    }
    for gate_name, metadata_field in sorted(configured):
        source_kinds = (
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS[
                (gate_name, metadata_field)
            ]
        )
        assert isinstance(source_kinds, tuple)
        assert source_kinds
        assert len(source_kinds) == len(set(source_kinds))
        assert set(source_kinds) <= required_kinds[gate_name]


def test_positive_int_list_metadata_without_owner_kind_tether_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = gate_summary("reputation")
    monkeypatch.delitem(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS,
        ("reputation", "provider_count_values"),
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "provider_count_values source-kind tether is not configured for `reputation`"
        in errors
    )


def test_positive_int_list_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    cases = [
        (
            gate_name,
            metadata_field,
            owner_kinds,
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS[
                metadata_field
            ],
        )
        for (
            gate_name,
            metadata_field,
        ), owner_kinds in MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS.items()
    ]

    for gate_name, metadata_field, owner_kinds, fingerprint_field in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        for owner_kind in owner_kinds:
            remove_fingerprint_metadata(
                payload,
                fingerprint_field,
                kind_name=owner_kind,
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{metadata_field} must match recognized artifact fingerprints" in errors


def test_object_list_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("appeal_finance")
    payload["valid_multi_peer_runs"] = [
        {
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
            "generated_at_unix": GENERATED_AT,
            "peer_count": 4,
            "validator_count": 4,
            "case_count": 2,
            "config_digest_hex": SHA256,
        }
    ]
    payload["valid_config_digests"] = [SHA256]
    add_fingerprint_metadata(
        payload,
        case_count=2,
        config_digest_hex=SHA256,
        peer_count=4,
        validator_count=4,
    )
    write_json(tmp_path / "appeal_finance.json", payload)

    assert run_gate(tmp_path, "--require-gate", "appeal_finance") == 0


def test_appeal_finance_multi_peer_run_config_must_match_valid_config(
    tmp_path: Path,
) -> None:
    payload = gate_summary("appeal_finance")
    other_digest = "cd" * 32
    payload["valid_multi_peer_runs"][0]["config_digest_hex"] = other_digest
    add_fingerprint_metadata(
        payload,
        kind_name="multi_peer_reconciliation",
        config_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_multi_peer_runs config digests must match valid_config_digests"
        in errors
    )


def test_object_list_metadata_must_match_required_artifact_count(
    tmp_path: Path,
) -> None:
    cases = [
        (
            gate_name,
            metadata_field,
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS[
                metadata_field
            ],
        )
        for (
            gate_name,
            metadata_field,
        ) in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS
    ]

    for gate_name, metadata_field, required_kind in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        payload[metadata_field] = []
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert (
            f"{metadata_field} length must match `{required_kind}` required artifact count"
            in errors
        )


def test_every_object_list_metadata_field_has_owner_kind_tether() -> None:
    expected = {
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in (
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.keys()
        )
    }
    configured = set(MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS)
    assert configured == expected

    required_kinds = {
        gate.name: set(gate.required_kinds) for gate in MODULE.GATE_SUMMARY_KINDS
    }
    for gate_name, metadata_field in sorted(configured):
        source_kind = MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS[
            (gate_name, metadata_field)
        ]
        assert isinstance(source_kind, str)
        assert source_kind
        assert source_kind in required_kinds[gate_name]
        assert (
            source_kind
            == MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS[
                metadata_field
            ]
        )


def test_every_object_list_metadata_field_has_domain_identity() -> None:
    assert set(MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS) == set(
        MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS
    )

    for field, identity_fields in sorted(
        MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS.items()
    ):
        schema = MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
        allowed_fields = (
            set(schema.get("strings", frozenset()))
            | set(schema.get("positive_ints", frozenset()))
            | set(schema.get("hex", {}))
        )
        assert identity_fields
        assert set(identity_fields) <= allowed_fields


def test_object_list_metadata_without_owner_kind_tether_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = gate_summary("reserve_rent")
    monkeypatch.delitem(
        MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS,
        ("reserve_rent", "valid_provider_bakes"),
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_provider_bakes source-kind tether is not configured for `reserve_rent`"
        in errors
    )


def test_object_list_metadata_owner_kind_tether_drift_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = gate_summary("reserve_rent")
    monkeypatch.setitem(
        MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS,
        ("reserve_rent", "valid_provider_bakes"),
        "policy_config",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_provider_bakes source-kind tether must match required artifact "
        "count kind for `reserve_rent`"
    ) in errors


def test_object_list_metadata_missing_owner_kind_tethers_fail_closed_from_config(
    tmp_path: Path,
    monkeypatch,
) -> None:
    original_tethers = dict(MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS)
    assert original_tethers

    for gate_name, field in sorted(original_tethers):
        root = tmp_path / f"{gate_name}_{field}_missing_tether"
        root.mkdir()
        monkeypatch.setattr(
            MODULE,
            "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS",
            {
                key: value
                for key, value in original_tethers.items()
                if key != (gate_name, field)
            },
        )
        payload = gate_summary(gate_name)
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{field} source-kind tether is not configured for `{gate_name}`" in errors


def test_object_list_metadata_owner_kind_tether_drift_fails_closed_from_config(
    tmp_path: Path,
    monkeypatch,
) -> None:
    original_tethers = dict(MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS)
    cases = []
    for gate_name, field in sorted(original_tethers):
        expected_kind = MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS[
            field
        ]
        alternate_kind = next(
            (
                kind
                for kind in MODULE.GATE_BY_NAME[gate_name].required_kinds
                if kind != expected_kind
            ),
            None,
        )
        assert alternate_kind is not None
        cases.append((gate_name, field, alternate_kind))
    assert cases

    for gate_name, field, alternate_kind in cases:
        root = tmp_path / f"{gate_name}_{field}_drifted_tether"
        root.mkdir()
        drifted_tethers = dict(original_tethers)
        drifted_tethers[(gate_name, field)] = alternate_kind
        monkeypatch.setattr(
            MODULE,
            "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS",
            drifted_tethers,
        )
        payload = gate_summary(gate_name)
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert (
            f"{field} source-kind tether must match required artifact count "
            f"kind for `{gate_name}`"
        ) in errors
        assert alternate_kind not in errors


def test_object_list_metadata_must_match_recognized_artifact_fingerprints(
    tmp_path: Path,
) -> None:
    cases = [
        (
            gate_name,
            metadata_field,
            source_kind,
            fingerprint_field,
        )
        for (
            gate_name,
            metadata_field,
        ), source_kind in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS.items()
        for fingerprint_field in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_FINGERPRINT_HEX_BINDINGS[
            metadata_field
        ]
    ]

    for gate_name, metadata_field, required_kind, fingerprint_field in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}_{fingerprint_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        remove_fingerprint_metadata(
            payload,
            fingerprint_field,
            kind_name=required_kind,
        )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert (
            f"{metadata_field}.{fingerprint_field} must match recognized artifact fingerprints"
            in errors
        )


def test_object_list_metadata_non_digest_details_must_match_fingerprints(
    tmp_path: Path,
) -> None:
    def replacement_value(metadata_field: str, detail_field: str, current: object) -> object:
        if detail_field == "cycle_id":
            return "billing-cycle-2"
        if detail_field == "bake_id":
            return "reserve-bake-002"
        if detail_field == "generated_at_unix":
            assert isinstance(current, int)
            return current - 1
        if isinstance(current, int):
            return current + 1
        raise AssertionError(f"unhandled {metadata_field}.{detail_field}")

    cases = [
        (
            gate_name,
            metadata_field,
            detail_field,
        )
        for (
            gate_name,
            metadata_field,
        ) in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS
        for detail_field in sorted(
            (
                set(
                    MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[
                        metadata_field
                    ].get("strings", ())
                )
                - {"deployment_id", "environment"}
            )
            | set(
                MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[
                    metadata_field
                ].get("positive_ints", ())
            )
        )
    ]

    for index, (gate_name, metadata_field, detail_field) in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}_{detail_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        replacement = replacement_value(
            metadata_field,
            detail_field,
            payload[metadata_field][0][detail_field],
        )
        payload[metadata_field][0][detail_field] = replacement
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert (
            f"{metadata_field} entries must match recognized artifact fingerprints"
            in errors
        )


def test_object_list_identity_labels_replay_lane_policies_without_echo(
    tmp_path: Path,
) -> None:
    def invalid_labels(
        gate_name: str,
        metadata_field: str,
        key_field: str,
        policy: dict,
    ) -> list[tuple[str, str]]:
        current = gate_summary(gate_name)[metadata_field][0][key_field]
        assert isinstance(current, str)
        marker = (
            "placeholder"
            if "placeholder" in policy["forbidden_markers"]
            else sorted(policy["forbidden_markers"])[0]
        )
        forbidden_error = (
            f"{metadata_field}[0].{key_field} must not contain "
            f"non-production markers {str([marker])}"
        )
        prefix = current.rsplit("-", 1)[0]
        forbidden_labels = [
            f"{prefix}-prod{marker}review",
            f"{current}-{marker}",
        ]
        for label in forbidden_labels:
            assert policy["pattern"].fullmatch(label) is not None

        pattern_label = f"{key_field}-prod-private-key"
        assert policy["pattern"].fullmatch(pattern_label) is None
        pattern_error = f"{metadata_field}[0].{key_field} {policy['pattern_error']}"
        return [
            *((label, forbidden_error) for label in forbidden_labels),
            (pattern_label, pattern_error),
        ]

    cases = [
        (
            gate_name,
            metadata_field,
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS[
                (gate_name, metadata_field)
            ],
            key_field,
            invalid_label,
            expected_error,
        )
        for (
            gate_name,
            metadata_field,
            key_field,
        ), policy in sorted(
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_STRING_FIELD_POLICIES.items()
        )
        for invalid_label, expected_error in invalid_labels(
            gate_name,
            metadata_field,
            key_field,
            policy,
        )
    ]

    for index, (
        gate_name,
        metadata_field,
        source_kind,
        key_field,
        invalid_label,
        expected_error,
    ) in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}_{key_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        payload[metadata_field][0][key_field] = invalid_label
        add_fingerprint_metadata(
            payload,
            kind_name=source_kind,
            **{key_field: invalid_label},
        )
        if gate_name == "reserve_rent":
            add_fingerprint_metadata(
                payload,
                kind_name="governance_approval",
                bake_id=invalid_label,
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert expected_error in errors
        assert invalid_label not in errors


def test_object_list_metadata_entries_are_validated(tmp_path: Path) -> None:
    payload = gate_summary("appeal_finance")
    bad_digest = "AB" * 32
    payload["valid_multi_peer_runs"] = [
        {
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
            "generated_at_unix": GENERATED_AT,
            "peer_count": 0,
            "validator_count": True,
            "case_count": "2",
            "config_digest_hex": bad_digest,
            "private_key": "runtime-only-key-material",
        },
        "not-a-run",
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_multi_peer_runs[0].peer_count must be a positive integer" in errors
    assert (
        "valid_multi_peer_runs[0].validator_count must be a positive integer"
        in errors
    )
    assert "valid_multi_peer_runs[0].case_count must be a positive integer" in errors
    assert (
        "valid_multi_peer_runs[0].config_digest_hex must be 64 lowercase hex characters"
        in errors
    )
    assert (
        "valid_multi_peer_runs[0].<sensitive-key> is not allowed in payload-free object metadata"
        in errors
    )
    assert "valid_multi_peer_runs[1] must be a payload-free metadata object" in errors
    assert bad_digest not in errors
    assert "private_key" not in errors
    assert "runtime-only-key-material" not in errors
    assert "not-a-run" not in errors


def test_object_list_metadata_schema_fields_fail_closed_from_config() -> None:
    cases = [
        (
            gate_name,
            field,
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field],
        )
        for gate_name, field in sorted(
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS
        )
    ]
    assert cases
    assert {field for _, field, _ in cases} == set(
        MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS
    )

    def validate(field: str, schema: dict, item: object) -> str:
        errors: list[str] = []
        MODULE.validate_payload_free_object_list_metadata(
            field,
            [item],
            schema,
            errors,
        )
        return "\n".join(errors)

    for gate_name, field, schema in cases:
        base_item = copy.deepcopy(gate_summary(gate_name)[field][0])
        secret = f" private_key-runtime-{field}"
        non_object_errors = validate(field, schema, secret)
        assert f"{field}[0] must be a payload-free metadata object" in non_object_errors
        assert secret not in non_object_errors

        item_with_secret_key = copy.deepcopy(base_item)
        item_with_secret_key["private_key"] = f"runtime-only-{field}-key"
        secret_key_errors = validate(field, schema, item_with_secret_key)
        assert (
            f"{field}[0].<sensitive-key> is not allowed in payload-free object metadata"
            in secret_key_errors
        )
        assert "private_key" not in secret_key_errors
        assert f"runtime-only-{field}-key" not in secret_key_errors

        for key in sorted(schema.get("strings", frozenset())):
            item = copy.deepcopy(base_item)
            item[key] = secret
            errors = validate(field, schema, item)
            assert f"{field}[0].{key} must be a canonical string" in errors
            assert secret not in errors

        for key in sorted(schema.get("positive_ints", frozenset())):
            item = copy.deepcopy(base_item)
            item[key] = True
            errors = validate(field, schema, item)
            assert f"{field}[0].{key} must be a positive integer" in errors

        for key, expected_hex_length in sorted(schema.get("hex", {}).items()):
            item = copy.deepcopy(base_item)
            bad_hex = "AB" * (expected_hex_length // 2)
            item[key] = bad_hex
            errors = validate(field, schema, item)
            assert (
                f"{field}[0].{key} must be {expected_hex_length} lowercase hex characters"
                in errors
            )
            assert bad_hex not in errors

        for start_key, end_key in schema.get("ordered_int_pairs", ()):
            item = copy.deepcopy(base_item)
            item[start_key] = 2
            item[end_key] = 1
            errors = validate(field, schema, item)
            assert f"{field}[0].{end_key} must be >= {start_key}" in errors


def test_object_list_metadata_entries_must_not_duplicate_from_config() -> None:
    cases = [
        (
            gate_name,
            field,
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field],
        )
        for gate_name, field in sorted(
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS
        )
    ]
    assert cases
    assert {field for _, field, _ in cases} == set(
        MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS
    )

    for gate_name, field, schema in cases:
        original = copy.deepcopy(gate_summary(gate_name)[field][0])
        errors: list[str] = []
        MODULE.validate_payload_free_object_list_metadata(
            field,
            [copy.deepcopy(original), copy.deepcopy(original)],
            schema,
            errors,
        )

        result_text = "\n".join(errors)
        assert f"{field} must not contain duplicate metadata entries" in result_text
        for value in original.values():
            if isinstance(value, str):
                assert value not in result_text


def test_evidence_viewer_digest_set_metadata_requires_every_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    del payload["valid_evidence_viewer_digest_sets"][0]["access_log_digest_hex"]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_evidence_viewer_digest_sets[0].access_log_digest_hex "
        "must be 64 lowercase hex characters"
    ) in errors


def test_evidence_viewer_digest_set_requires_catalog_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    del payload["valid_evidence_viewer_digest_sets"][0]["catalog_digest_hex"]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_evidence_viewer_digest_sets[0].catalog_digest_hex "
        "must be 64 lowercase hex characters"
    ) in errors


def test_evidence_viewer_catalog_digest_must_match_artifact_fingerprint(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    payload["valid_evidence_viewer_digest_sets"][0][
        "catalog_digest_hex"
    ] = "cd" * 32
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_evidence_viewer_digest_sets.catalog_digest_hex must match "
        "recognized artifact fingerprints"
    ) in errors


def test_joint_gateway_moderation_catalog_binding_passes(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_compliance")
    write_gate(tmp_path, "moderation_panel")

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_compliance",
            "--require-gate",
            "moderation_panel",
        )
        == 0
    )


def test_joint_gateway_moderation_rejects_individually_valid_foreign_catalog(
    tmp_path: Path,
) -> None:
    moderation = gate_summary("moderation_panel")
    foreign_catalog_digest = "cd" * 32
    moderation["valid_evidence_viewer_digest_sets"][0][
        "catalog_digest_hex"
    ] = foreign_catalog_digest
    add_fingerprint_metadata(
        moderation,
        kind_name="evidence_viewer",
        catalog_digest_hex=foreign_catalog_digest,
    )
    write_json(tmp_path / "moderation_panel.json", moderation)

    assert run_gate(tmp_path, "--require-gate", "moderation_panel") == 0

    write_gate(tmp_path, "gateway_compliance")
    summary = tmp_path / "summary.json"
    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_compliance",
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["required"]["gateway_compliance"]["valid"] is True
    assert result["required"]["moderation_panel"]["valid"] is True
    assert result["errors"].count(
        MODULE.GATEWAY_MODERATION_CATALOG_MISMATCH_ERROR
    ) == 1
    assert foreign_catalog_digest not in "\n".join(result["errors"])


def test_object_list_metadata_entries_must_not_duplicate(tmp_path: Path) -> None:
    payload = gate_summary("appeal_finance")
    run = {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "generated_at_unix": GENERATED_AT,
        "peer_count": 4,
        "validator_count": 4,
        "case_count": 2,
        "config_digest_hex": SHA256,
    }
    payload["valid_multi_peer_runs"] = [dict(run), dict(run)]
    payload["valid_config_digests"] = [SHA256]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_multi_peer_runs must not contain duplicate metadata entries"
        in errors
    )
    assert SHA256 not in errors


def test_object_list_metadata_domain_identities_must_not_duplicate() -> None:
    def replacement_value(field: str, detail_field: str, current: object) -> object:
        schema = MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
        if detail_field in schema.get("positive_ints", frozenset()):
            assert isinstance(current, int)
            return current + 1
        if detail_field in schema.get("hex", {}):
            expected_hex_length = schema["hex"][detail_field]
            replacement = "bc" * (expected_hex_length // 2)
            if replacement != current:
                return replacement
            return "cd" * (expected_hex_length // 2)
        if detail_field in schema.get("strings", frozenset()):
            assert isinstance(current, str)
            return f"{current}-variant"
        raise AssertionError(f"unhandled {field}.{detail_field}")

    def duplicate_domain_entries(gate_name: str, field: str) -> list[dict]:
        schema = MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
        identity_fields = set(
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS[field]
        )
        candidate_fields = sorted(
            (
                set(schema.get("positive_ints", frozenset()))
                | set(schema.get("hex", {}))
                | set(schema.get("strings", frozenset()))
            )
            - identity_fields
        )
        assert candidate_fields

        original = copy.deepcopy(gate_summary(gate_name)[field][0])
        duplicate_entry = copy.deepcopy(original)
        detail_field = candidate_fields[0]
        duplicate_entry[detail_field] = replacement_value(
            field,
            detail_field,
            duplicate_entry[detail_field],
        )
        assert duplicate_entry != original
        return [original, duplicate_entry]

    cases = [
        (
            gate_name,
            field,
            duplicate_domain_entries(gate_name, field),
        )
        for (gate_name, field) in sorted(
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS
        )
    ]

    for _gate_name, field, entries in cases:
        errors: list[str] = []
        MODULE.validate_payload_free_object_list_metadata(
            field,
            entries,
            MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field],
            errors,
        )

        assert f"{field} must not contain duplicate metadata identities" in errors
        assert f"{field} must not contain duplicate metadata entries" not in errors


def test_provider_bake_metadata_completed_at_must_not_precede_start(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    payload["valid_policy_digests"] = [SHA256]
    payload["valid_provider_bakes"] = [
        {
            "bake_id": "reserve-bake-001",
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
            "policy_digest_hex": SHA256,
            "matrix_digest_hex": SHA256,
            "ledger_digest_hex": SHA256,
            "started_at_unix": GENERATED_AT,
            "completed_at_unix": GENERATED_AT - 1,
            "provider_count": 3,
            "scheduled_lifecycle_canary_defaulted_provider_count": 1,
            "scheduled_lifecycle_canary_last_tick_at_unix": GENERATED_AT - 30,
            "scheduled_lifecycle_canary_tick_count": 2,
        }
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        "valid_provider_bakes[0].completed_at_unix must be >= started_at_unix"
        in "\n".join(result["errors"])
    )


def test_provider_bake_scheduler_metadata_is_validated(tmp_path: Path) -> None:
    payload = gate_summary("reserve_rent")
    payload["valid_policy_digests"] = [SHA256]
    payload["valid_provider_bakes"][0][
        "scheduled_lifecycle_canary_tick_count"
    ] = 0
    payload["valid_provider_bakes"][0][
        "scheduled_lifecycle_canary_defaulted_provider_count"
    ] = True
    payload["valid_provider_bakes"][0][
        "scheduled_lifecycle_canary_last_tick_at_unix"
    ] = GENERATED_AT + 1
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_provider_bakes[0].scheduled_lifecycle_canary_tick_count "
        "must be a positive integer"
    ) in errors
    assert (
        "valid_provider_bakes[0].scheduled_lifecycle_canary_defaulted_provider_count "
        "must be a positive integer"
    ) in errors
    assert (
        "valid_provider_bakes[0].completed_at_unix must be >= "
        "scheduled_lifecycle_canary_last_tick_at_unix"
    ) in errors


def test_reserve_rent_matrix_policy_must_match_policy_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    other_digest = "cd" * 32
    payload["valid_policy_matrix_bindings"][0]["policy_digest_hex"] = other_digest
    payload["valid_policy_matrix_ledger_bindings"][0][
        "policy_digest_hex"
    ] = other_digest
    payload["valid_provider_bakes"][0]["policy_digest_hex"] = other_digest
    add_fingerprint_metadata(
        payload,
        kind_name="quote_matrix",
        policy_digest_hex=other_digest,
    )
    add_fingerprint_metadata(
        payload,
        kind_name="ledger_digest",
        policy_digest_hex=other_digest,
    )
    add_fingerprint_metadata(
        payload,
        kind_name="provider_bake",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_policy_matrix_bindings policies must match valid_policy_digests"
        in errors
    )


def test_reserve_rent_ledger_pair_must_match_matrix_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    other_digest = "cd" * 32
    payload["valid_policy_matrix_ledger_bindings"][0][
        "matrix_digest_hex"
    ] = other_digest
    payload["valid_provider_bakes"][0]["matrix_digest_hex"] = other_digest
    add_fingerprint_metadata(
        payload,
        kind_name="ledger_digest",
        matrix_digest_hex=other_digest,
    )
    add_fingerprint_metadata(
        payload,
        kind_name="provider_bake",
        matrix_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_policy_matrix_ledger_bindings matrix pairs must match "
        "valid_policy_matrix_bindings"
    ) in errors


def test_reserve_rent_provider_bake_tuple_must_match_ledger_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    other_digest = "cd" * 32
    payload["valid_provider_bakes"][0]["ledger_digest_hex"] = other_digest
    add_fingerprint_metadata(
        payload,
        kind_name="provider_bake",
        ledger_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_provider_bakes ledger tuples must match "
        "valid_policy_matrix_ledger_bindings"
    ) in errors


def test_reserve_rent_governance_approval_requires_bake_id_fingerprint(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    remove_fingerprint_metadata(
        payload,
        "bake_id",
        kind_name="governance_approval",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reserve_rent governance approval bake_id fingerprints must match "
        "valid_provider_bakes"
    ) in errors


def test_reserve_rent_governance_approval_bake_id_must_match_provider_bakes(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        bake_id="reserve-bake-002",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reserve_rent governance approval bake_id fingerprints must match "
        "valid_provider_bakes"
    ) in errors
    assert "reserve-bake-002" not in errors


def test_reserve_rent_policy_bound_artifacts_must_match_policy_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="quote_matrix",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reserve_rent policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_reserve_rent_matrix_bound_artifacts_must_match_matrix_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="lifecycle_service",
        matrix_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reserve_rent matrix-bound artifact fingerprints must match "
        "valid_policy_matrix_bindings"
    ) in errors


def test_reserve_rent_ledger_bound_artifacts_must_match_ledger_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="lifecycle_service",
        ledger_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "reserve_rent ledger-bound artifact fingerprints must match "
        "valid_policy_matrix_ledger_bindings"
    ) in errors


def test_reserve_rent_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="metrics_alerts",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_reserve_rent_metrics_metadata_must_cover_required_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="metrics_alerts",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_moderation_panel_roster_binding_case_must_match_case_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    other_digest = "cd" * 32
    payload["valid_roster_bindings"] = [
        {"case_digest_hex": other_digest, "roster_hash_hex": SHA256}
    ]
    payload["valid_tally_bindings"] = [
        {
            "case_digest_hex": other_digest,
            "roster_hash_hex": SHA256,
            "tally_digest_hex": SHA256,
        }
    ]
    payload["valid_e2e_runs"][0]["case_digest_hex"] = other_digest
    payload["valid_evidence_viewer_digest_sets"][0][
        "case_digest_hex"
    ] = other_digest
    add_fingerprint_metadata(
        payload,
        case_digest_hex=other_digest,
        roster_hash_hex=SHA256,
        kind_name="sortition_roster",
    )
    add_fingerprint_metadata(
        payload,
        case_digest_hex=other_digest,
        roster_hash_hex=SHA256,
        tally_digest_hex=SHA256,
        kind_name="commit_reveal",
    )
    add_fingerprint_metadata(
        payload,
        case_digest_hex=other_digest,
        roster_hash_hex=SHA256,
        tally_digest_hex=SHA256,
        kind_name="e2e_panel",
    )
    add_fingerprint_metadata(
        payload,
        case_digest_hex=other_digest,
        roster_hash_hex=SHA256,
        kind_name="evidence_viewer",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "valid_roster_bindings case digests must match valid_case_digests" in errors


def test_moderation_panel_tally_binding_must_match_roster_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    other_digest = "cd" * 32
    payload["valid_tally_bindings"] = [
        {
            "case_digest_hex": other_digest,
            "roster_hash_hex": SHA256,
            "tally_digest_hex": SHA256,
        }
    ]
    add_fingerprint_metadata(
        payload,
        case_digest_hex=other_digest,
        roster_hash_hex=SHA256,
        tally_digest_hex=SHA256,
        kind_name="commit_reveal",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "valid_tally_bindings roster pairs must match valid_roster_bindings" in errors


def test_moderation_panel_e2e_run_must_match_tally_binding(tmp_path: Path) -> None:
    payload = gate_summary("moderation_panel")
    other_digest = "cd" * 32
    payload["valid_e2e_runs"][0]["tally_digest_hex"] = other_digest
    add_fingerprint_metadata(
        payload,
        tally_digest_hex=other_digest,
        kind_name="e2e_panel",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "valid_e2e_runs tally bindings must match valid_tally_bindings" in errors


def test_moderation_panel_evidence_viewer_must_match_roster_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    other_digest = "cd" * 32
    payload["valid_evidence_viewer_digest_sets"][0][
        "roster_hash_hex"
    ] = other_digest
    add_fingerprint_metadata(
        payload,
        roster_hash_hex=other_digest,
        kind_name="evidence_viewer",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "valid_evidence_viewer_digest_sets roster pairs must match valid_roster_bindings"
        in errors
    )


def test_moderation_panel_case_bound_artifacts_must_match_case_digest(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="operator_workflow",
        case_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "moderation_panel case-bound artifact fingerprints must match "
        "valid_case_digests"
    ) in errors


def test_moderation_panel_roster_bound_artifacts_must_match_roster_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="juror_notifications",
        roster_hash_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "moderation_panel roster-bound artifact fingerprints must match "
        "valid_roster_bindings"
    ) in errors


def test_moderation_panel_tally_bound_artifacts_must_match_tally_binding(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="decision_publication",
        tally_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "moderation_panel tally-bound artifact fingerprints must match "
        "valid_tally_bindings"
    ) in errors


def test_moderation_panel_policy_bound_artifacts_must_match_e2e_policy(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    other_digest = "cd" * 32
    add_fingerprint_metadata(
        payload,
        kind_name="governance_approval",
        policy_digest_hex=other_digest,
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "moderation_panel policy-bound artifact fingerprints must match "
        "valid_policy_digests"
    ) in errors


def test_deployment_context_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("moderation_panel")
    payload["deployment_context"] = {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    payload["valid_case_digests"] = [SHA256]
    payload["valid_policy_digests"] = [SHA256]
    add_fingerprint_metadata(payload, case_digest_hex=SHA256, policy_digest_hex=SHA256)
    write_json(tmp_path / "moderation_panel.json", payload)

    assert run_gate(tmp_path, "--require-gate", "moderation_panel") == 0


def test_moderation_panel_metrics_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    remove_fingerprint_metadata(
        payload,
        "metric_count",
        "metrics",
        kind_name="metrics_alerts",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "metrics must match recognized artifact fingerprints" in errors
    assert "metric_count_values must match recognized artifact fingerprints" in errors


def test_moderation_panel_metrics_metadata_must_cover_required_values(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    missing = payload["metrics"].pop()
    payload["metric_count_values"] = [len(payload["metrics"])]
    add_fingerprint_metadata(
        payload,
        metric_count=len(payload["metrics"]),
        metrics=payload["metrics"],
        kind_name="metrics_alerts",
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert f"metrics must include metadata value `{missing}`" in errors


def test_deployment_context_metadata_mismatch_fails(tmp_path: Path) -> None:
    payload = gate_summary("moderation_panel")
    mismatched_deployment = "moderation-panel-staging-a"
    payload["deployment_context"] = {
        "deployment_id": mismatched_deployment,
        "environment": ENVIRONMENT,
    }
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "moderation_panel deployment context must match across artifacts and metadata"
        in errors
    )
    assert mismatched_deployment not in errors


def test_deployment_context_metadata_entries_are_validated(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    payload["deployment_context"] = {
        "deployment_id": " runtime-only-deployment",
        "private_key": "runtime-only-key-material",
    }
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "deployment_context.deployment_id must be a canonical string" in errors
    assert "deployment_context.environment must be a canonical string" in errors
    assert (
        "deployment_context.<sensitive-key> is not allowed in payload-free object metadata"
        in errors
    )
    assert "runtime-only-deployment" not in errors
    assert "private_key" not in errors
    assert "runtime-only-key-material" not in errors


def test_object_metadata_fields_fail_closed_from_config(tmp_path: Path) -> None:
    cases = [
        (gate.name, field, tuple(sorted(object_fields)))
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field, object_fields in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS.items()
        if field in MODULE.GATE_METADATA_FIELDS[gate.name]
    ]
    assert cases
    assert {field for _, field, _ in cases} == set(
        MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS
    )

    for index, (gate_name, field, object_fields) in enumerate(cases):
        invalid_value = f"runtime-only-{field}-secret"
        root = tmp_path / f"{index}_{gate_name}_{field}_non_object"
        root.mkdir()
        payload = gate_summary(gate_name)
        payload[field] = invalid_value
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert f"{field} must be a payload-free metadata object" in errors
        assert invalid_value not in result_text

        invalid_required_value = f" runtime-only-{field}-deployment"
        private_value = f"runtime-only-{field}-key-material"
        root = tmp_path / f"{index}_{gate_name}_{field}_invalid_entries"
        root.mkdir()
        payload = gate_summary(gate_name)
        payload[field] = {
            object_fields[0]: invalid_required_value,
            f"bad\n{field}": f"runtime-only-{field}-control-key",
            "private_key": private_value,
        }
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert f"{field} keys must be canonical strings" in errors
        assert (
            f"{field}.<sensitive-key> is not allowed in payload-free object metadata"
            in errors
        )
        for object_field in object_fields:
            assert f"{field}.{object_field} must be a canonical string" in errors
        assert invalid_required_value.strip() not in result_text
        assert private_value not in result_text
        assert "private_key" not in result_text


def test_payload_free_deployment_context_rejects_nonproduction_without_echo(
    tmp_path: Path,
) -> None:
    cases = [
        (
            "moderation_panel",
            "moderation-panel-staging-a",
            ENVIRONMENT,
            "deployment_context.deployment_id must not contain "
            "non-production deployment markers ['staging']",
            "moderation-panel-staging-a",
        ),
        (
            "appeal_finance",
            "appeal-finance-staging-a",
            ENVIRONMENT,
            "valid_multi_peer_runs[0].deployment_id must not contain "
            "non-production deployment markers ['staging']",
            "appeal-finance-staging-a",
        ),
        (
            "moderation_panel",
            "moderation-panel-stagingready-a",
            ENVIRONMENT,
            "deployment_context.deployment_id must not contain "
            "non-production deployment markers ['staging']",
            "moderation-panel-stagingready-a",
        ),
        (
            "moderation_panel",
            "moderation-panel-staging-b",
            ENVIRONMENT,
            "valid_e2e_runs[0].deployment_id must not contain "
            "non-production deployment markers ['staging']",
            "moderation-panel-staging-b",
        ),
        (
            "hedging_billing",
            DEPLOYMENT_ID,
            "dev",
            "valid_billing_cycles[0].environment must be production",
            "dev",
        ),
        (
            "reserve_rent",
            "reserve-notproductionready-a",
            ENVIRONMENT,
            "valid_provider_bakes[0].deployment_id must not contain "
            "non-reviewed deployment markers ['notproductionready']",
            "reserve-notproductionready-a",
        ),
    ]

    for index, (
        gate_name,
        deployment_id,
        environment,
        expected_error,
        raw_label,
    ) in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}"
        root.mkdir()
        payload = gate_summary(
            gate_name,
            deployment_id=deployment_id,
            environment=environment,
        )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert expected_error in errors
        assert raw_label not in errors


def test_payload_free_deployment_context_surfaces_reject_nonproduction_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, field, False)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field, object_fields in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS.items()
        if field in MODULE.GATE_METADATA_FIELDS[gate.name]
        and {"deployment_id", "environment"} <= set(object_fields)
    ]
    cases.extend(
        (gate.name, field, True)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field, schema in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.items()
        if field in MODULE.GATE_METADATA_FIELDS[gate.name]
        and {"deployment_id", "environment"} <= set(schema.get("strings", frozenset()))
    )
    assert cases

    for index, (gate_name, field, is_object_list) in enumerate(cases):
        path = f"{field}[0]" if is_object_list else field
        for suffix, key, value, expected_error in (
            (
                "deployment_id",
                "deployment_id",
                f"{gate_name.replace('_', '-')}-staging-a",
                f"{path}.deployment_id must not contain "
                "non-production deployment markers ['staging']",
            ),
            (
                "environment",
                "environment",
                "dev",
                f"{path}.environment must be production",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{field}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            metadata = payload[field][0] if is_object_list else payload[field]
            assert isinstance(metadata, dict)
            metadata[key] = value
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert expected_error in errors
            assert value not in result_text


def test_payload_free_deployment_context_surfaces_must_match_artifacts_from_config(
    tmp_path: Path,
) -> None:
    object_field_cases = [
        (gate.name, field, False)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field, object_fields in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS.items()
        if field in MODULE.GATE_METADATA_FIELDS[gate.name]
        and {"deployment_id", "environment"} <= set(object_fields)
    ]
    object_list_field_cases = [
        (gate.name, field, True)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field, schema in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.items()
        if field in MODULE.GATE_METADATA_FIELDS[gate.name]
        and {"deployment_id", "environment"} <= set(schema.get("strings", frozenset()))
    ]
    cases = object_field_cases + object_list_field_cases
    assert cases
    assert {field for _, field, _ in object_field_cases} == {
        field
        for field, object_fields in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS.items()
        if {"deployment_id", "environment"} <= set(object_fields)
    }
    assert {field for _, field, _ in object_list_field_cases} == {
        field
        for field, schema in MODULE.PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.items()
        if {"deployment_id", "environment"} <= set(schema.get("strings", frozenset()))
    }

    for index, (gate_name, field, is_object_list) in enumerate(cases):
        mismatched_deployment_id = (
            f"sorafs-mainnet-{index:02d}-{gate_name.replace('_', '-')}"
        )
        root = tmp_path / f"{index}_{gate_name}_{field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        metadata = payload[field][0] if is_object_list else payload[field]
        assert isinstance(metadata, dict)
        metadata["deployment_id"] = mismatched_deployment_id
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert (
            f"{gate_name} deployment context must match across artifacts and metadata"
            in errors
        )
        assert mismatched_deployment_id not in result_text


def test_object_list_metadata_deployment_context_mismatch_fails(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    mismatched_environment = "staging"
    payload["valid_billing_cycles"] = [
        {
            "deployment_id": DEPLOYMENT_ID,
            "environment": mismatched_environment,
            "cycle_id": "billing-cycle-a",
            "cycle_index": 1,
            "generated_at_unix": GENERATED_AT,
            "policy_digest_hex": SHA256,
            "statement_count": 3,
            "reference_decision_id_hex": SHA256,
            "statement_bundle_digest_hex": SHA256,
            "reconciliation_digest_hex": SHA256,
        }
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "hedging_billing deployment context must match across artifacts and metadata"
        in errors
    )
    assert mismatched_environment not in errors


def test_multi_peer_run_metadata_deployment_context_mismatch_fails(
    tmp_path: Path,
) -> None:
    payload = gate_summary("appeal_finance")
    mismatched_deployment = "appeal-finance-staging-a"
    payload["valid_multi_peer_runs"] = [
        {
            "deployment_id": mismatched_deployment,
            "environment": ENVIRONMENT,
            "generated_at_unix": GENERATED_AT,
            "peer_count": 4,
            "validator_count": 4,
            "case_count": 2,
            "config_digest_hex": SHA256,
        }
    ]
    payload["valid_config_digests"] = [SHA256]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "appeal_finance deployment context must match across artifacts and metadata"
        in errors
    )
    assert mismatched_deployment not in errors


def test_provider_bake_metadata_deployment_context_mismatch_fails(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    mismatched_environment = "staging"
    payload["valid_policy_digests"] = [SHA256]
    payload["valid_provider_bakes"] = [
        {
            "bake_id": "reserve-bake-001",
            "deployment_id": DEPLOYMENT_ID,
            "environment": mismatched_environment,
            "policy_digest_hex": SHA256,
            "matrix_digest_hex": SHA256,
            "ledger_digest_hex": SHA256,
            "started_at_unix": GENERATED_AT - 3_600,
            "completed_at_unix": GENERATED_AT,
            "provider_count": 3,
            "scheduled_lifecycle_canary_defaulted_provider_count": 1,
            "scheduled_lifecycle_canary_last_tick_at_unix": GENERATED_AT - 60,
            "scheduled_lifecycle_canary_tick_count": 2,
        }
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "reserve_rent deployment context must match across artifacts and metadata"
        in errors
    )
    assert mismatched_environment not in errors


def test_reputation_top_level_hex_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("reputation")
    payload["snapshot_id_hex"] = SNAPSHOT_ID
    payload["merkle_root_hex"] = SHA256
    add_fingerprint_metadata(
        payload,
        snapshot_id_hex=SNAPSHOT_ID,
        merkle_root_hex=SHA256,
    )
    write_json(tmp_path / "reputation.json", payload)

    assert run_gate(tmp_path, "--require-gate", "reputation") == 0


def test_scalar_metadata_must_match_recognized_artifact_fingerprints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    payload["snapshot_id_hex"] = SNAPSHOT_ID
    payload["merkle_root_hex"] = SHA256
    remove_fingerprint_metadata(payload, "snapshot_id_hex", "merkle_root_hex")
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "snapshot_id_hex must match recognized artifact fingerprints" in errors
    assert "merkle_root_hex must match recognized artifact fingerprints" in errors
    assert SNAPSHOT_ID not in errors
    assert SHA256 not in errors


def test_every_scalar_metadata_field_has_owner_kind_tether() -> None:
    expected = {
        (gate.name, field)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in (
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS.keys()
        )
    }
    configured = set(MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS)
    assert configured == expected

    required_kinds = {
        gate.name: set(gate.required_kinds) for gate in MODULE.GATE_SUMMARY_KINDS
    }
    for gate_name, metadata_field in sorted(configured):
        source_kinds = MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS[
            (gate_name, metadata_field)
        ]
        assert isinstance(source_kinds, tuple)
        assert source_kinds
        assert len(source_kinds) == len(set(source_kinds))
        assert set(source_kinds) <= required_kinds[gate_name]


def test_scalar_metadata_without_owner_kind_tether_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = gate_summary("reputation")
    monkeypatch.delitem(
        MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS,
        ("reputation", "snapshot_id_hex"),
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert "snapshot_id_hex source-kind tether is not configured for `reputation`" in errors


def test_scalar_hex_metadata_entries_fail_closed_from_config(tmp_path: Path) -> None:
    cases = [
        (
            gate.name,
            field,
            MODULE.PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS[field],
        )
        for gate in MODULE.GATE_SUMMARY_KINDS
        for field in sorted(
            MODULE.GATE_METADATA_FIELDS[gate.name]
            & MODULE.PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS
        )
    ]
    assert cases
    assert {field for _, field, _ in cases} == set(
        MODULE.PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS
    )

    for index, (gate_name, field, expected_hex_length) in enumerate(cases):
        for suffix, value in (
            ("malformed", f"private-key-{field}"),
            ("uppercase", "AB" * (expected_hex_length // 2)),
            ("object", {"private_key": f"runtime-only-{field}-key"}),
        ):
            root = tmp_path / f"{index}_{gate_name}_{field}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload[field] = value
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert (
                f"{field} must be {expected_hex_length} lowercase hex characters"
                in errors
            )
            if isinstance(value, str):
                assert value not in result_text
            else:
                assert "private_key" not in result_text
                assert value["private_key"] not in result_text


def test_scalar_metadata_must_match_owner_kind_fingerprints(
    tmp_path: Path,
) -> None:
    cases = [
        (
            gate_name,
            metadata_field,
            owner_kinds,
            MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS[
                metadata_field
            ],
        )
        for (
            gate_name,
            metadata_field,
        ), owner_kinds in MODULE.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS.items()
    ]

    for gate_name, metadata_field, owner_kinds, fingerprint_field in sorted(cases):
        root = tmp_path / f"{gate_name}_{metadata_field}_{fingerprint_field}"
        root.mkdir()
        payload = gate_summary(gate_name)
        for owner_kind in owner_kinds:
            remove_fingerprint_metadata(
                payload,
                fingerprint_field,
                kind_name=owner_kind,
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
        assert f"{metadata_field} must match recognized artifact fingerprints" in errors


def test_reputation_top_level_hex_metadata_is_validated(tmp_path: Path) -> None:
    payload = gate_summary("reputation")
    bad_snapshot_id = "not-a-snapshot-id"
    bad_merkle_root = "AB" * 32
    payload["snapshot_id_hex"] = bad_snapshot_id
    payload["merkle_root_hex"] = bad_merkle_root
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "snapshot_id_hex must be 32 lowercase hex characters" in errors
    assert "merkle_root_hex must be 64 lowercase hex characters" in errors
    assert bad_snapshot_id not in errors
    assert bad_merkle_root not in errors


def test_cross_lane_top_level_lane_metadata_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_snapshot_bindings"] = [
        {
            "snapshot_id_hex": SHA256,
            "merkle_root_hex": SHA256,
        }
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        "valid_snapshot_bindings is not allowed for `gateway_load` lane metadata"
        in "\n".join(result["errors"])
    )


def test_narrowed_lane_summary_fails(tmp_path: Path) -> None:
    gate = MODULE.GATE_BY_NAME["gateway_load"]
    write_gate(tmp_path, "gateway_load", required_kinds=["local_conformance"])
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "required_kinds missing full-contract kinds" in errors
    for missing_kind in set(gate.required_kinds) - {"local_conformance"}:
        assert missing_kind not in errors


def test_extra_required_kind_labels_are_payload_free(tmp_path: Path) -> None:
    gate = MODULE.GATE_BY_NAME["gateway_load"]
    hidden_kind = "shadow_optional_row"
    required_kinds = list(gate.required_kinds) + [hidden_kind, hidden_kind]
    payload = gate_summary("gateway_load", required_kinds=required_kinds)
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "required_kinds contains duplicate kind" in errors
    assert "required_kinds contains unknown full-contract kinds" in errors
    assert hidden_kind not in errors


def test_extra_required_row_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    extra_row = dict(payload["required"][first_required])
    extra_row["schema"] = f"{payload['schema']}.hidden_optional"
    payload["required"]["hidden_optional"] = extra_row
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_extra_required_row_label_is_payload_free(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    hidden_row = "shadow_optional_row"
    extra_row = dict(payload["required"][first_required])
    extra_row["schema"] = f"{payload['schema']}.hidden_optional"
    payload["required"][hidden_row] = extra_row
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "required contains rows outside the full `gateway_load` gate contract"
        in errors
    )
    assert hidden_row not in errors


def test_malformed_extra_required_row_label_is_sanitized(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    extra_row = dict(payload["required"][first_required])
    extra_row["schema"] = f"{payload['schema']}.hidden_optional"
    payload["required"]["hidden\noptional"] = extra_row
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "required row labels must be canonical strings" in errors
    assert (
        "required contains rows outside the full `gateway_load` gate contract"
        in errors
    )
    assert "hidden\noptional" not in errors


def test_required_kind_inventory_fails_closed_from_config(tmp_path: Path) -> None:
    cases = [(gate.name, gate.required_kinds) for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases

    for index, (gate_name, required_kinds) in enumerate(cases):
        hidden_kind = f"runtime_only_hidden_kind_{index:03d}"
        noncanonical_kind = f"runtime-only-private-kind-{index:03d}\nshadow"
        for suffix, mutate, expected_errors, forbidden_values in (
            (
                "scalar_required_kinds",
                lambda payload: payload.__setitem__(
                    "required_kinds",
                    f"runtime-only-private-required-kinds-{index:03d}",
                ),
                ("required_kinds must be a non-empty array",),
                (f"runtime-only-private-required-kinds-{index:03d}",),
            ),
            (
                "noncanonical_required_kind",
                lambda payload: payload["required_kinds"].__setitem__(
                    0,
                    noncanonical_kind,
                ),
                (
                    "required_kinds[0] must be canonical",
                    f"required_kinds must match the full `{gate_name}` gate contract",
                    "required_kinds missing full-contract kinds",
                ),
                (noncanonical_kind,),
            ),
            (
                "narrowed_required_kinds",
                lambda payload: payload.__setitem__(
                    "required_kinds",
                    list(required_kinds[:-1]),
                ),
                (
                    f"required_kinds must match the full `{gate_name}` gate contract",
                    "required_kinds missing full-contract kinds",
                ),
                (),
            ),
            (
                "duplicate_unknown_required_kind",
                lambda payload: payload.__setitem__(
                    "required_kinds",
                    [*payload["required_kinds"], hidden_kind, hidden_kind],
                ),
                (
                    "required_kinds contains duplicate kind",
                    f"required_kinds must match the full `{gate_name}` gate contract",
                    "required_kinds contains unknown full-contract kinds",
                ),
                (hidden_kind,),
            ),
            (
                "scalar_required",
                lambda payload: payload.__setitem__(
                    "required",
                    f"runtime-only-private-required-{index:03d}",
                ),
                ("required must be an object",),
                (f"runtime-only-private-required-{index:03d}",),
            ),
            (
                "extra_required_row",
                lambda payload: payload["required"].__setitem__(
                    hidden_kind,
                    copy.deepcopy(payload["required"][required_kinds[0]]),
                ),
                (
                    f"required contains rows outside the full `{gate_name}` gate contract",
                ),
                (hidden_kind,),
            ),
            (
                "noncanonical_required_row",
                lambda payload: payload["required"].__setitem__(
                    noncanonical_kind,
                    copy.deepcopy(payload["required"][required_kinds[0]]),
                ),
                (
                    "required row labels must be canonical strings",
                    f"required contains rows outside the full `{gate_name}` gate contract",
                ),
                (noncanonical_kind,),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            for expected_error in expected_errors:
                assert expected_error in errors
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_extra_required_row_field_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["payload"] = {"raw": "leak"}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_row_state_fields_fail_closed_from_config(tmp_path: Path) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for field, expected_error in (
            ("present", f"{gate_name}.required.{kind_name}.present must be true"),
            ("valid", f"{gate_name}.required.{kind_name}.valid must be true"),
            (
                "artifact_count",
                f"{gate_name}.required.{kind_name}.artifact_count must be positive",
            ),
        ):
            forged_value = f"runtime-only-required-row-{field}-{index:03d}"
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{field}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload["required"][kind_name][field] = forged_value
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert expected_error in errors
            assert forged_value not in result_text


def test_required_row_presence_and_shape_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, mutate, forged_value in (
            (
                "missing",
                lambda payload: payload["required"].pop(kind_name),
                None,
            ),
            (
                "non_object",
                lambda payload: payload["required"].__setitem__(
                    kind_name,
                    f"runtime-only-required-row-object-{index:03d}",
                ),
                f"runtime-only-required-row-object-{index:03d}",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert f"{gate_name}.required.{kind_name} must be an object" in errors
            if forged_value is not None:
                assert forged_value not in result_text


def test_required_and_recognized_error_lists_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, mutate, expected_error in (
            (
                "row",
                lambda payload, error_text: payload["required"][kind_name].__setitem__(
                    "errors",
                    [error_text],
                ),
                f"{gate_name}.required.{kind_name}.errors must be empty",
            ),
            (
                "required_artifact",
                lambda payload, error_text: payload["required"][kind_name][
                    "artifacts"
                ][0].__setitem__(
                    "errors",
                    [error_text],
                ),
                f"{gate_name}.required.{kind_name}.artifacts[0].errors must be empty",
            ),
            (
                "recognized_artifact",
                lambda payload, error_text: payload["recognized_artifacts"][
                    next(
                        artifact_index
                        for artifact_index, artifact in enumerate(
                            payload["recognized_artifacts"]
                        )
                        if artifact["kind"] == kind_name
                    )
                ].__setitem__(
                    "errors",
                    [error_text],
                ),
                "recognized_artifacts[{index}].errors must be empty",
            ),
        ):
            forged_error = f"runtime-only-private-key-material-{index:03d}-{suffix}"
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload, forged_error)
            recognized_index = next(
                (
                    artifact_index
                    for artifact_index, artifact in enumerate(
                        payload["recognized_artifacts"]
                    )
                    if isinstance(artifact, dict) and artifact.get("kind") == kind_name
                ),
                None,
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            if suffix == "recognized_artifact":
                assert recognized_index is not None
                expected = expected_error.format(index=recognized_index)
            else:
                expected = expected_error
            assert expected in errors
            assert forged_error not in result_text


def test_required_and_recognized_malformed_error_lists_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    def set_required_row_errors(
        payload: dict,
        kind_name: str,
        value: object,
    ) -> int | None:
        payload["required"][kind_name]["errors"] = value
        return None

    def set_required_artifact_errors(
        payload: dict,
        kind_name: str,
        value: object,
    ) -> int | None:
        payload["required"][kind_name]["artifacts"][0]["errors"] = value
        return None

    def set_recognized_artifact_errors(
        payload: dict,
        kind_name: str,
        value: object,
    ) -> int:
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
        )
        payload["recognized_artifacts"][recognized_index]["errors"] = value
        return recognized_index

    for index, (gate_name, kind_name) in enumerate(cases):
        for surface, mutate, expected_path in (
            (
                "row",
                set_required_row_errors,
                f"{gate_name}.required.{kind_name}.errors",
            ),
            (
                "required_artifact",
                set_required_artifact_errors,
                f"{gate_name}.required.{kind_name}.artifacts[0].errors",
            ),
            (
                "recognized_artifact",
                set_recognized_artifact_errors,
                "recognized_artifacts[{index}].errors",
            ),
        ):
            for shape, malformed_value, expected_suffix in (
                (
                    "scalar",
                    f"runtime-only-private-key-error-list-{index:03d}-{surface}",
                    "must be an empty error list",
                ),
                (
                    "noncanonical",
                    [
                        f"runtime-only-private-key-error-list-{index:03d}-{surface}\nforged"
                    ],
                    "must contain only canonical strings",
                ),
            ):
                root = tmp_path / f"{index}_{gate_name}_{kind_name}_{surface}_{shape}"
                root.mkdir()
                payload = gate_summary(gate_name)
                recognized_index = mutate(payload, kind_name, malformed_value)
                summary = root / "summary.json"
                write_json(root / f"{gate_name}.json", payload)

                assert (
                    run_gate(
                        root,
                        "--require-gate",
                        gate_name,
                        "--summary-out",
                        str(summary),
                    )
                    == 1
                )

                result_text = summary.read_text(encoding="utf-8")
                errors = "\n".join(json.loads(result_text)["errors"])
                if surface == "recognized_artifact":
                    assert recognized_index is not None
                    path = expected_path.format(index=recognized_index)
                else:
                    path = expected_path
                assert f"{path} {expected_suffix}" in errors
                if isinstance(malformed_value, list):
                    assert malformed_value[0] not in result_text
                else:
                    assert malformed_value not in result_text


def test_required_and_recognized_error_list_shapes_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    def remove_required_row_errors(payload: dict, kind_name: str) -> int | None:
        payload["required"][kind_name].pop("errors")
        return None

    def set_required_row_errors_object(
        payload: dict,
        kind_name: str,
        value: object,
    ) -> int | None:
        payload["required"][kind_name]["errors"] = value
        return None

    def remove_required_artifact_errors(
        payload: dict,
        kind_name: str,
    ) -> int | None:
        payload["required"][kind_name]["artifacts"][0].pop("errors")
        return None

    def set_required_artifact_errors_object(
        payload: dict,
        kind_name: str,
        value: object,
    ) -> int | None:
        payload["required"][kind_name]["artifacts"][0]["errors"] = value
        return None

    def remove_recognized_artifact_errors(payload: dict, kind_name: str) -> int:
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
        )
        payload["recognized_artifacts"][recognized_index].pop("errors")
        return recognized_index

    def set_recognized_artifact_errors_object(
        payload: dict,
        kind_name: str,
        value: object,
    ) -> int:
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
        )
        payload["recognized_artifacts"][recognized_index]["errors"] = value
        return recognized_index

    for index, (gate_name, kind_name) in enumerate(cases):
        malformed_object = {
            "private_key": f"runtime-only-error-list-{index:03d}",
        }
        for surface, shape, mutate, expected_path, forbidden_values in (
            (
                "row",
                "missing",
                lambda payload, _value: remove_required_row_errors(
                    payload,
                    kind_name,
                ),
                f"{gate_name}.required.{kind_name}.errors",
                (),
            ),
            (
                "row",
                "object",
                lambda payload, value: set_required_row_errors_object(
                    payload,
                    kind_name,
                    value,
                ),
                f"{gate_name}.required.{kind_name}.errors",
                ("private_key", malformed_object["private_key"]),
            ),
            (
                "required_artifact",
                "missing",
                lambda payload, _value: remove_required_artifact_errors(
                    payload,
                    kind_name,
                ),
                f"{gate_name}.required.{kind_name}.artifacts[0].errors",
                (),
            ),
            (
                "required_artifact",
                "object",
                lambda payload, value: set_required_artifact_errors_object(
                    payload,
                    kind_name,
                    value,
                ),
                f"{gate_name}.required.{kind_name}.artifacts[0].errors",
                ("private_key", malformed_object["private_key"]),
            ),
            (
                "recognized_artifact",
                "missing",
                lambda payload, _value: remove_recognized_artifact_errors(
                    payload,
                    kind_name,
                ),
                "recognized_artifacts[{index}].errors",
                (),
            ),
            (
                "recognized_artifact",
                "object",
                lambda payload, value: set_recognized_artifact_errors_object(
                    payload,
                    kind_name,
                    value,
                ),
                "recognized_artifacts[{index}].errors",
                ("private_key", malformed_object["private_key"]),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{surface}_{shape}"
            root.mkdir()
            payload = gate_summary(gate_name)
            recognized_index = mutate(
                payload,
                malformed_object if shape == "object" else None,
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            if surface == "recognized_artifact":
                assert recognized_index is not None
                path = expected_path.format(index=recognized_index)
            else:
                path = expected_path
            assert f"{path} must be an empty error list" in errors
            assert result["required"][gate_name]["valid"] is False
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_recognized_artifact_count_mismatch_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["recognized_artifact_count"] += 1
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_recognized_artifact_inventory_shape_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases

    for index, gate_name in enumerate(cases):
        for suffix, mutate, expected_errors in (
            (
                "missing",
                lambda payload, _secret: payload.pop("recognized_artifacts"),
                ("recognized_artifacts must be present",),
            ),
            (
                "scalar",
                lambda payload, secret: payload.__setitem__(
                    "recognized_artifacts",
                    secret,
                ),
                ("recognized_artifacts must be a non-empty array",),
            ),
            (
                "empty",
                lambda payload, _secret: payload.__setitem__(
                    "recognized_artifacts",
                    [],
                ),
                ("recognized_artifacts must be a non-empty array",),
            ),
            (
                "count",
                lambda payload, _secret: payload.__setitem__(
                    "recognized_artifact_count",
                    len(payload["recognized_artifacts"]) + 1,
                ),
                (
                    "recognized_artifacts length must match recognized_artifact_count",
                    "recognized_artifact_count must match recognized artifact object count",
                ),
            ),
        ):
            secret = f"runtime-only-private-inventory-{index:03d}-{suffix}"
            root = tmp_path / f"{index}_{gate_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload, secret)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            for expected_error in expected_errors:
                assert expected_error in errors
            assert secret not in result_text


def test_required_artifact_inventory_shape_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        base_path = f"{gate_name}.required.{kind_name}"
        for suffix, mutate, expected_errors, forbidden_values in (
            (
                "missing",
                lambda payload, _secret: payload["required"][kind_name].pop(
                    "artifacts"
                ),
                (
                    f"{base_path}.artifacts must be a non-empty array",
                    f"{base_path}.artifact_count must match artifact object count",
                ),
                (),
            ),
            (
                "scalar",
                lambda payload, secret: payload["required"][kind_name].__setitem__(
                    "artifacts",
                    secret,
                ),
                (
                    f"{base_path}.artifacts must be a non-empty array",
                    f"{base_path}.artifact_count must match artifact object count",
                ),
                ("{secret}",),
            ),
            (
                "empty",
                lambda payload, _secret: payload["required"][kind_name].__setitem__(
                    "artifacts",
                    [],
                ),
                (
                    f"{base_path}.artifacts must be a non-empty array",
                    f"{base_path}.artifact_count must match artifact object count",
                ),
                (),
            ),
            (
                "non_object",
                lambda payload, secret: payload["required"][kind_name][
                    "artifacts"
                ].__setitem__(0, secret),
                (
                    f"{base_path}.artifact_count must match artifact object count",
                    f"{base_path}.artifacts[0] must be an object",
                ),
                ("{secret}",),
            ),
            (
                "count_drift",
                lambda payload, _secret: payload["required"][kind_name].__setitem__(
                    "artifact_count",
                    payload["required"][kind_name]["artifact_count"] + 1,
                ),
                (f"{base_path}.artifact_count must match artifact object count",),
                (),
            ),
        ):
            secret = f"runtime-only-required-artifact-inventory-{index:03d}-{suffix}"
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload, secret)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            for expected_error in expected_errors:
                assert expected_error in errors
            for forbidden_value in forbidden_values:
                assert forbidden_value.format(secret=secret) not in result_text


def test_required_artifact_count_mismatch_reports_observed_aggregate_count(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    observed_count = sum(
        len(row["artifacts"]) for row in payload["required"].values()
    )
    payload["required"][first_required]["artifact_count"] += 1
    payload["recognized_artifact_count"] = observed_count + 1
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    row = result["required"]["gateway_load"]
    diagnostics = "\n".join(result["errors"] + row["errors"])
    assert row["artifact_count"] == observed_count
    assert row["recognized_artifact_count"] == observed_count
    assert (
        f"gateway_load.required.{first_required}.artifact_count must match artifact object count"
        in diagnostics
    )
    assert (
        "recognized_artifact_count must match recognized artifact object count"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row recognized_artifact_count must match artifact_count"
        not in diagnostics
    )


def test_non_object_required_artifact_does_not_inflate_aggregate_count(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["artifacts"].append("payload-shaped-entry")
    payload["required"][first_required]["artifact_count"] += 1
    observed_count = sum(
        sum(1 for artifact in row["artifacts"] if isinstance(artifact, dict))
        for row in payload["required"].values()
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    row = result["required"]["gateway_load"]
    diagnostics = "\n".join(result["errors"] + row["errors"])
    assert row["artifact_count"] == observed_count
    assert (
        f"gateway_load.required.{first_required}.artifact_count must match artifact object count"
        in diagnostics
    )
    assert (
        f"gateway_load.required.{first_required}.artifacts[1] must be an object"
        in diagnostics
    )


def test_non_object_required_artifact_does_not_inflate_recognized_expected_count(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["recognized_artifacts"] = recognized_artifacts_from_required(payload)
    payload["required"][first_required]["artifacts"].append("payload-shaped-entry")
    payload["required"][first_required]["artifact_count"] += 1
    payload["recognized_artifact_count"] = len(payload["recognized_artifacts"])
    payload["evidence_file_count"] = len(
        {artifact["path"] for artifact in payload["recognized_artifacts"]}
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    diagnostics = "\n".join(result["errors"] + result["required"]["gateway_load"]["errors"])
    assert (
        f"gateway_load.required.{first_required}.artifacts[1] must be an object"
        in diagnostics
    )
    assert (
        f"gateway_load.required.{first_required}.artifact_count must match artifact object count"
        in diagnostics
    )
    assert "recognized_artifacts must match required artifact counts" not in diagnostics


def test_non_object_recognized_artifact_does_not_inflate_aggregate_count(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    recognized_object_count = sum(
        1 for artifact in payload["recognized_artifacts"] if isinstance(artifact, dict)
    )
    payload["recognized_artifacts"].append("payload-shaped-entry")
    non_object_index = len(payload["recognized_artifacts"]) - 1
    payload["recognized_artifact_count"] = recognized_object_count + 1
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    row = result["required"]["gateway_load"]
    diagnostics = "\n".join(result["errors"] + row["errors"])
    assert row["recognized_artifact_count"] == recognized_object_count
    assert row["artifact_count"] == recognized_object_count
    assert f"recognized_artifacts[{non_object_index}] must be an object" in diagnostics
    assert (
        "recognized_artifact_count must match recognized artifact object count"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row recognized_artifact_count must match artifact_count"
        not in diagnostics
    )


def test_non_object_artifact_entries_do_not_inflate_aggregate_counts_from_config(
    tmp_path: Path,
) -> None:
    required_cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    recognized_cases = [(gate.name, None) for gate in MODULE.GATE_SUMMARY_KINDS]
    cases = [
        ("required", gate_name, kind_name)
        for gate_name, kind_name in required_cases
    ] + [
        ("recognized", gate_name, kind_name)
        for gate_name, kind_name in recognized_cases
    ]
    assert cases

    for index, (surface, gate_name, kind_name) in enumerate(cases):
        secret = f"runtime-only-non-object-artifact-entry-{index:03d}-{surface}"
        root = tmp_path / f"{index}_{gate_name}_{kind_name or 'recognized'}_{surface}"
        root.mkdir()
        payload = gate_summary(gate_name)
        expected_object_count = sum(
            sum(
                1
                for artifact in row["artifacts"]
                if isinstance(artifact, dict)
            )
            for row in payload["required"].values()
        )
        if surface == "required":
            assert kind_name is not None
            artifacts = payload["required"][kind_name]["artifacts"]
            non_object_index = len(artifacts)
            artifacts.append(secret)
            payload["required"][kind_name]["artifact_count"] += 1
            expected_errors = (
                f"{gate_name}.required.{kind_name}.artifact_count "
                "must match artifact object count",
                f"{gate_name}.required.{kind_name}.artifacts[{non_object_index}] "
                "must be an object",
            )
        else:
            non_object_index = len(payload["recognized_artifacts"])
            payload["recognized_artifacts"].append(secret)
            payload["recognized_artifact_count"] = expected_object_count + 1
            expected_errors = (
                f"recognized_artifacts[{non_object_index}] must be an object",
                "recognized_artifact_count must match recognized artifact object count",
            )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        result = json.loads(result_text)
        row = result["required"][gate_name]
        diagnostics = "\n".join(result["errors"] + row["errors"])
        assert row["artifact_count"] == expected_object_count
        assert row["recognized_artifact_count"] == expected_object_count
        for expected_error in expected_errors:
            assert expected_error in diagnostics
        assert (
            f"{gate_name} aggregate invalid row recognized_artifact_count "
            "must match artifact_count"
        ) not in diagnostics
        assert secret not in result_text


def test_empty_required_artifacts_do_not_inflate_aggregate_count(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["artifacts"] = []
    observed_count = sum(
        sum(
            1
            for artifact in row.get("artifacts", [])
            if isinstance(artifact, dict)
        )
        for row in payload["required"].values()
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    row = result["required"]["gateway_load"]
    diagnostics = "\n".join(result["errors"] + row["errors"])
    assert row["artifact_count"] == observed_count
    assert (
        f"gateway_load.required.{first_required}.artifacts must be a non-empty array"
        in diagnostics
    )
    assert (
        f"gateway_load.required.{first_required}.artifact_count must match artifact object count"
        in diagnostics
    )


def test_malformed_required_artifact_inventories_do_not_inflate_counts_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    def recognized_required_artifact_objects(payload: dict) -> list[dict]:
        artifacts = []
        for required_kind, row in payload["required"].items():
            row_artifacts = row.get("artifacts")
            if not isinstance(row_artifacts, list):
                continue
            for required_artifact in row_artifacts:
                if not isinstance(required_artifact, dict):
                    continue
                artifact = copy.deepcopy(required_artifact)
                artifact["kind"] = required_kind
                artifacts.append(artifact)
        return artifacts

    for index, (gate_name, kind_name) in enumerate(cases):
        base_path = f"{gate_name}.required.{kind_name}"
        for suffix, mutate, expected_errors, forbidden_values in (
            (
                "missing",
                lambda payload, _secret: payload["required"][kind_name].pop(
                    "artifacts"
                ),
                (
                    f"{base_path}.artifacts must be a non-empty array",
                    f"{base_path}.artifact_count must match artifact object count",
                ),
                (),
            ),
            (
                "scalar",
                lambda payload, secret: payload["required"][kind_name].__setitem__(
                    "artifacts",
                    secret,
                ),
                (
                    f"{base_path}.artifacts must be a non-empty array",
                    f"{base_path}.artifact_count must match artifact object count",
                ),
                ("{secret}",),
            ),
            (
                "empty",
                lambda payload, _secret: payload["required"][kind_name].__setitem__(
                    "artifacts",
                    [],
                ),
                (
                    f"{base_path}.artifacts must be a non-empty array",
                    f"{base_path}.artifact_count must match artifact object count",
                ),
                (),
            ),
        ):
            secret = f"runtime-only-required-artifact-bucket-{index:03d}-{suffix}"
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload, secret)
            recognized_artifacts = recognized_required_artifact_objects(payload)
            expected_object_count = len(recognized_artifacts)
            payload["recognized_artifacts"] = recognized_artifacts
            payload["recognized_artifact_count"] = expected_object_count
            payload["evidence_file_count"] = len(
                {artifact["path"] for artifact in recognized_artifacts}
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            row = result["required"][gate_name]
            diagnostics = "\n".join(result["errors"] + row["errors"])
            assert row["artifact_count"] == expected_object_count
            assert row["recognized_artifact_count"] == expected_object_count
            for expected_error in expected_errors:
                assert expected_error in diagnostics
            assert "recognized_artifact_count must match required row artifact total" not in diagnostics
            assert "recognized_artifacts must match required artifact counts" not in diagnostics
            assert (
                f"{gate_name} aggregate invalid row recognized_artifact_count "
                "must match artifact_count"
            ) not in diagnostics
            for forbidden_value in forbidden_values:
                assert forbidden_value.format(secret=secret) not in result_text


def test_evidence_file_count_exceeds_artifacts_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["evidence_file_count"] = payload["recognized_artifact_count"] + 1
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_evidence_file_count_must_match_recognized_artifact_paths(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["evidence_file_count"] = 1
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_evidence_file_count_ignores_unsafe_recognized_artifact_paths(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    unsafe_path = "/tmp/runtime-only/sorafs-evidence.json"
    payload["recognized_artifacts"][0]["path"] = unsafe_path
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    diagnostics = "\n".join(result["errors"])
    assert "evidence_file_count must match recognized artifact path count" in diagnostics
    assert (
        ".path must be archive-relative without absolute, empty, current, "
        "parent, encoded, URI-scheme-like, platform-specific, or secret-looking segments"
        in diagnostics
    )
    assert unsafe_path not in diagnostics


def test_unsafe_recognized_artifact_paths_do_not_satisfy_evidence_count_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    expected_path_suffix = (
        "path must be archive-relative without absolute, empty, current, "
        "parent, encoded, URI-scheme-like, platform-specific, or secret-looking "
        "segments"
    )

    for index, (gate_name, kind_name) in enumerate(cases):
        unsafe_path = (
            f"/tmp/runtime-only/sorafs/{gate_name}/{kind_name}/private_key.json"
        )
        root = tmp_path / f"{index}_{gate_name}_{kind_name}"
        root.mkdir()
        payload = gate_summary(gate_name)
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
        )
        payload["recognized_artifacts"][recognized_index]["path"] = unsafe_path
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        result = json.loads(result_text)
        row = result["required"][gate_name]
        diagnostics = "\n".join(result["errors"] + row["errors"])
        assert (
            "evidence_file_count must match recognized artifact path count"
            in diagnostics
        )
        assert f"recognized_artifacts[{recognized_index}].{expected_path_suffix}" in diagnostics
        assert (
            f"{gate_name} aggregate invalid row recognized_artifact_count "
            "must match artifact_count"
        ) not in diagnostics
        assert unsafe_path not in result_text
        assert "private_key" not in result_text


def test_malformed_top_level_counts_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["evidence_file_count"] = 0
    payload["recognized_artifact_count"] = True
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_top_level_count_invariants_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases

    for index, gate_name in enumerate(cases):
        for suffix, mutate, expected_errors in (
            (
                "malformed_evidence_file_count",
                lambda payload, secret: payload.__setitem__(
                    "evidence_file_count",
                    secret,
                ),
                ("evidence_file_count must be a positive integer",),
            ),
            (
                "malformed_recognized_artifact_count",
                lambda payload, secret: payload.__setitem__(
                    "recognized_artifact_count",
                    secret,
                ),
                ("recognized_artifact_count must be a positive integer",),
            ),
            (
                "evidence_exceeds_recognized",
                lambda payload, _secret: payload.__setitem__(
                    "evidence_file_count",
                    payload["recognized_artifact_count"] + 1,
                ),
                (
                    "evidence_file_count must not exceed recognized_artifact_count",
                    "evidence_file_count must match recognized artifact path count",
                ),
            ),
            (
                "evidence_path_count_mismatch",
                lambda payload, _secret: payload.__setitem__(
                    "evidence_file_count",
                    payload["evidence_file_count"] - 1,
                ),
                ("evidence_file_count must match recognized artifact path count",),
            ),
            (
                "recognized_required_total_mismatch",
                lambda payload, _secret: payload.__setitem__(
                    "recognized_artifact_count",
                    payload["recognized_artifact_count"] + 1,
                ),
                (
                    "recognized_artifact_count must match required row artifact total",
                    "recognized_artifacts length must match recognized_artifact_count",
                    "recognized_artifact_count must match recognized artifact object count",
                ),
            ),
        ):
            secret = f"runtime-only-private-count-{index:03d}-{suffix}"
            root = tmp_path / f"{index}_{gate_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload, secret)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            for expected_error in expected_errors:
                assert expected_error in errors
            assert secret not in result_text


def test_invalid_top_level_recognized_artifacts_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["recognized_artifacts"][0]["valid"] = False
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_and_recognized_artifact_status_must_be_successful(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_kind = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_kind]["artifacts"][0]["status"] = "failed"
    payload["recognized_artifacts"][0]["status"] = "failed"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        f"gateway_load.required.{first_kind}.artifacts[0].status "
        "must be a successful status"
        in errors
    )
    assert "recognized_artifacts[0].status must be a successful status" in errors
    assert "failed" not in errors


def test_required_and_recognized_artifact_statuses_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        forged_status = f"runtime-only-status-{index:03d}"
        root = tmp_path / f"{index}_{gate_name}_{kind_name}"
        root.mkdir()
        payload = gate_summary(gate_name)
        payload["required"][kind_name]["artifacts"][0]["status"] = forged_status
        payload["recognized_artifacts"] = recognized_artifacts_from_required(payload)
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
            and artifact["path"]
            == payload["required"][kind_name]["artifacts"][0]["path"]
        )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert (
            f"{gate_name}.required.{kind_name}.artifacts[0].status "
            "must be a successful status"
            in errors
        )
        assert (
            f"recognized_artifacts[{recognized_index}].status "
            "must be a successful status"
            in errors
        )
        assert forged_status not in result_text


def test_required_and_recognized_artifact_status_shapes_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for surface, shape in (
            ("required", "missing"),
            ("required", "object"),
            ("recognized", "missing"),
            ("recognized", "object"),
        ):
            hostile_status = {"private_key": f"runtime-only-status-{index:03d}"}
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{surface}_{shape}"
            root.mkdir()
            payload = gate_summary(gate_name)
            target_artifact = payload["required"][kind_name]["artifacts"][0]
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"] == target_artifact["path"]
            )
            if surface == "required":
                artifact = target_artifact
                expected_error = (
                    f"{gate_name}.required.{kind_name}.artifacts[0].status "
                    "must be canonical"
                )
            else:
                artifact = payload["recognized_artifacts"][recognized_index]
                expected_error = (
                    f"recognized_artifacts[{recognized_index}].status "
                    "must be canonical"
                )
            if shape == "missing":
                artifact.pop("status")
            else:
                artifact["status"] = hostile_status
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert expected_error in errors
            assert result["required"][gate_name]["valid"] is False
            assert "private_key" not in result_text
            assert hostile_status["private_key"] not in result_text


def test_required_and_recognized_artifact_valid_markers_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        forged_valid = f"runtime-only-valid-{index:03d}"
        root = tmp_path / f"{index}_{gate_name}_{kind_name}"
        root.mkdir()
        payload = gate_summary(gate_name)
        payload["required"][kind_name]["artifacts"][0]["valid"] = forged_valid
        payload["recognized_artifacts"] = recognized_artifacts_from_required(payload)
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
            and artifact["path"]
            == payload["required"][kind_name]["artifacts"][0]["path"]
        )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert (
            f"{gate_name}.required.{kind_name}.artifacts[0].valid must be true"
            in errors
        )
        assert f"recognized_artifacts[{recognized_index}].valid must be true" in errors
        assert forged_valid not in result_text


def test_required_and_recognized_artifact_valid_marker_shapes_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        malformed_values = (
            ("missing", None, ()),
            ("false", False, ()),
            (
                "object",
                {"private_key": f"runtime-only-valid-{index:03d}"},
                ("private_key", f"runtime-only-valid-{index:03d}"),
            ),
        )
        for suffix, malformed_valid, forbidden_values in malformed_values:
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}_valid"
            root.mkdir()
            payload = gate_summary(gate_name)
            required_artifact = payload["required"][kind_name]["artifacts"][0]
            if suffix == "missing":
                required_artifact.pop("valid")
            else:
                required_artifact["valid"] = malformed_valid
            payload["recognized_artifacts"] = recognized_artifacts_from_required(
                payload
            )
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"] == required_artifact["path"]
            )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            result = json.loads(result_text)
            errors = "\n".join(result["errors"])
            assert f"{gate_name} production readiness summary is invalid" in errors
            assert (
                f"{gate_name}.required.{kind_name}.artifacts[0].valid "
                "must be true"
                in errors
            )
            assert (
                f"recognized_artifacts[{recognized_index}].valid must be true"
                in errors
            )
            assert result["required"][gate_name]["valid"] is False
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_required_artifact_extra_fields_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["artifacts"][0]["payload"] = {"raw": "leak"}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_and_recognized_artifact_extra_fields_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    def add_required_artifact_private_key(
        payload: dict,
        kind_name: str,
        secret: str,
    ) -> int | None:
        payload["required"][kind_name]["artifacts"][0]["private_key"] = secret
        return None

    def add_recognized_artifact_private_key(
        payload: dict,
        kind_name: str,
        secret: str,
    ) -> int:
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
        )
        payload["recognized_artifacts"][recognized_index]["private_key"] = secret
        return recognized_index

    for index, (gate_name, kind_name) in enumerate(cases):
        for surface, mutate, expected_path in (
            (
                "required_artifact",
                add_required_artifact_private_key,
                f"{gate_name}.required.{kind_name}.artifacts[0].<sensitive-key>",
            ),
            (
                "recognized_artifact",
                add_recognized_artifact_private_key,
                "recognized_artifacts[{index}].<sensitive-key>",
            ),
        ):
            secret = f"runtime-only-private-key-artifact-field-{index:03d}-{surface}"
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{surface}"
            root.mkdir()
            payload = gate_summary(gate_name)
            recognized_index = mutate(payload, kind_name, secret)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            if surface == "recognized_artifact":
                assert recognized_index is not None
                path = expected_path.format(index=recognized_index)
            else:
                path = expected_path
            assert (
                f"{path} is not allowed in payload-free artifact summary" in errors
            )
            assert "private_key" not in result_text
            assert secret not in result_text


def test_required_row_schema_must_match_required_evidence_schema(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["required"]["local_conformance"]["schema"] = "sorafs.shadow.schema.v1"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "gateway_load.required.local_conformance.schema must match required evidence schema"
        in errors
    )
    assert "sorafs.shadow.schema.v1" not in errors


def test_required_artifact_schema_must_match_required_evidence_schema(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["required"]["local_conformance"]["artifacts"][0]["schema"] = (
        "sorafs.shadow.schema.v1"
    )
    payload["recognized_artifacts"] = recognized_artifacts_from_required(payload)
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    errors = "\n".join(json.loads(summary.read_text(encoding="utf-8"))["errors"])
    assert (
        "gateway_load.required.local_conformance.artifacts[0].schema "
        "must match required evidence schema"
    ) in errors
    assert "sorafs.shadow.schema.v1" not in errors


def test_required_row_schema_canonicality_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for suffix, forged_schema, forbidden_values in (
            (
                "padded",
                " padded-row-schema ",
                (" padded-row-schema ",),
            ),
            (
                "newline",
                f"runtime-only-row-schema-{index:03d}\nprivate_key",
                (f"runtime-only-row-schema-{index:03d}\nprivate_key", "private_key"),
            ),
            (
                "object",
                {"private_key": f"runtime-only-row-schema-{index:03d}"},
                ("private_key", f"runtime-only-row-schema-{index:03d}"),
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            payload["required"][kind_name]["schema"] = forged_schema
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert (
                f"{gate_name}.required.{kind_name}.schema must be canonical"
                in errors
            )
            for forbidden_value in forbidden_values:
                assert forbidden_value not in result_text


def test_required_kind_schema_bindings_fail_closed_from_config(tmp_path: Path) -> None:
    cases = [
        (gate.name, kind_name, schema)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name, schema in MODULE.GATE_REQUIRED_KIND_SCHEMAS[gate.name].items()
    ]
    assert cases
    assert set(cases) == {
        (
            gate.name,
            kind_name,
            MODULE.GATE_REQUIRED_KIND_SCHEMAS[gate.name][kind_name],
        )
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    }

    for index, (gate_name, kind_name, _schema) in enumerate(cases):
        forged_schema = f"sorafs.private.runtime.schema.{index:03d}.v1"
        for suffix, mutate, expected_error in (
            (
                "row",
                lambda payload: payload["required"][kind_name].__setitem__(
                    "schema",
                    forged_schema,
                ),
                f"{gate_name}.required.{kind_name}.schema "
                "must match required evidence schema",
            ),
            (
                "artifact",
                lambda payload: payload["required"][kind_name]["artifacts"][
                    0
                ].__setitem__(
                    "schema",
                    forged_schema,
                ),
                f"{gate_name}.required.{kind_name}.artifacts[0].schema "
                "must match required evidence schema",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload)
            if suffix == "artifact":
                payload["recognized_artifacts"] = recognized_artifacts_from_required(
                    payload
                )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert expected_error in errors
            assert forged_schema not in result_text


def test_required_artifact_kind_mismatch_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["artifacts"][0]["kind"] = "wrong_kind"
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_and_recognized_artifact_kind_canonicality_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        forged_kind = f"runtime-only-kind-{index:03d}\nprivate_key"
        for suffix, mutate, expected_error in (
            (
                "required",
                lambda payload: payload["required"][kind_name]["artifacts"][
                    0
                ].__setitem__(
                    "kind",
                    forged_kind,
                ),
                f"{gate_name}.required.{kind_name}.artifacts[0].kind "
                "must be canonical when present",
            ),
            (
                "recognized",
                lambda payload: payload["recognized_artifacts"][
                    next(
                        artifact_index
                        for artifact_index, artifact in enumerate(
                            payload["recognized_artifacts"]
                        )
                        if artifact["kind"] == kind_name
                    )
                ].__setitem__(
                    "kind",
                    forged_kind,
                ),
                "recognized_artifacts[{index}].kind must be canonical",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
            )
            mutate(payload)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            if suffix == "recognized":
                expected = expected_error.format(index=recognized_index)
            else:
                expected = expected_error
            assert expected in errors
            assert forged_kind not in result_text
            assert "private_key" not in result_text


def test_required_and_recognized_artifact_kind_labels_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        forged_kind = f"runtime_only_kind_{index:03d}"
        for suffix, mutate, expected_error in (
            (
                "required",
                lambda payload: payload["required"][kind_name]["artifacts"][
                    0
                ].__setitem__(
                    "kind",
                    forged_kind,
                ),
                f"{gate_name}.required.{kind_name}.artifacts[0].kind "
                "must match required row kind",
            ),
            (
                "recognized",
                lambda payload: payload["recognized_artifacts"][
                    next(
                        artifact_index
                        for artifact_index, artifact in enumerate(
                            payload["recognized_artifacts"]
                        )
                        if artifact["kind"] == kind_name
                    )
                ].__setitem__(
                    "kind",
                    forged_kind,
                ),
                "kind must be part of the full "
                f"`{gate_name}` gate contract",
            ),
        ):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{suffix}"
            root.mkdir()
            payload = gate_summary(gate_name)
            mutate(payload)
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert expected_error in errors
            assert forged_kind not in result_text


def test_required_artifact_duplicate_paths_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    first_artifact = payload["required"][first_kind]["artifacts"][0]
    duplicate_path_artifact = dict(first_artifact)
    duplicate_path_artifact["sha256"] = "cd" * 32
    duplicate_path_artifact["fingerprint"] = dict(first_artifact["fingerprint"])
    duplicate_path_artifact["fingerprint"]["generated_at_unix"] = GENERATED_AT - 1
    payload["required"][first_kind]["artifacts"].append(duplicate_path_artifact)
    payload["required"][first_kind]["artifact_count"] = 2
    payload["recognized_artifact_count"] += 1
    payload["recognized_artifacts"] = recognized_artifacts_from_required(payload)
    payload["evidence_file_count"] = len(
        {artifact["path"] for artifact in payload["recognized_artifacts"]}
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert ".artifacts must not duplicate artifact paths" in errors
    assert "recognized_artifacts must not duplicate artifact paths" in errors


def test_required_artifact_duplicate_paths_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}_{kind_name}"
        root.mkdir()
        payload = gate_summary(gate_name)
        duplicate = copy.deepcopy(payload["required"][kind_name]["artifacts"][0])
        duplicate["sha256"] = "cd" * 32
        duplicate["fingerprint"] = copy.deepcopy(duplicate["fingerprint"])
        duplicate["fingerprint"]["generated_at_unix"] = GENERATED_AT - 1
        payload["required"][kind_name]["artifacts"].append(duplicate)
        payload["required"][kind_name]["artifact_count"] = len(
            payload["required"][kind_name]["artifacts"]
        )
        payload["recognized_artifacts"] = recognized_artifacts_from_required(payload)
        payload["recognized_artifact_count"] = len(payload["recognized_artifacts"])
        payload["evidence_file_count"] = len(
            {
                artifact["path"]
                for artifact in payload["recognized_artifacts"]
                if isinstance(artifact, dict)
            }
        )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert (
            f"{gate_name}.required.{kind_name}.artifacts "
            "must not duplicate artifact paths"
            in errors
        )
        assert "recognized_artifacts must not duplicate artifact paths" in errors
        assert (
            f"{gate_name}.required.{kind_name}.artifacts "
            "must not duplicate artifact identities"
            not in errors
        )
        assert "cd" * 32 not in result_text


def test_required_artifact_duplicate_identities_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}_{kind_name}"
        root.mkdir()
        payload = gate_summary(gate_name)
        duplicate = copy.deepcopy(payload["required"][kind_name]["artifacts"][0])
        payload["required"][kind_name]["artifacts"].append(duplicate)
        payload["required"][kind_name]["artifact_count"] = len(
            payload["required"][kind_name]["artifacts"]
        )
        payload["recognized_artifacts"] = recognized_artifacts_from_required(payload)
        payload["recognized_artifact_count"] = len(payload["recognized_artifacts"])
        payload["evidence_file_count"] = len(
            {
                artifact["path"]
                for artifact in payload["recognized_artifacts"]
                if isinstance(artifact, dict)
            }
        )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        errors = "\n".join(result["errors"])
        assert (
            f"{gate_name}.required.{kind_name}.artifacts "
            "must not duplicate artifact paths"
            in errors
        )
        assert (
            f"{gate_name}.required.{kind_name}.artifacts "
            "must not duplicate artifact identities"
            in errors
        )


def test_recognized_artifact_extra_fields_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["recognized_artifacts"][0]["payload"] = {"raw": "leak"}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_missing_top_level_recognized_artifacts_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    del payload["recognized_artifacts"]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_recognized_artifact_metadata_label_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["recognized_artifacts"][0]["status"] = " padded-status "
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_recognized_artifacts_must_match_required_kind_counts(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["recognized_artifacts"][0]["kind"]
    replaced_kind = payload["recognized_artifacts"][-1]["kind"]
    artifacts = payload["recognized_artifacts"]
    artifacts[-1] = dict(artifacts[0])
    payload["recognized_artifacts"] = artifacts
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "recognized_artifacts must match required artifact counts" in errors
    assert "{'required':" not in errors
    assert first_kind not in errors
    assert replaced_kind not in errors


def test_recognized_artifact_kind_counts_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name, gate.required_kinds)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name, required_kinds) in enumerate(cases):
        donor_kind = next(candidate for candidate in required_kinds if candidate != kind_name)
        root = tmp_path / f"{index}_{gate_name}_{kind_name}"
        root.mkdir()
        payload = gate_summary(gate_name)
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
        )
        payload["recognized_artifacts"][recognized_index]["kind"] = donor_kind
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        errors = "\n".join(result["errors"])
        assert "recognized_artifacts must match required artifact counts" in errors
        assert "recognized_artifacts must match required artifact identities" in errors
        assert "{'required':" not in errors


def test_recognized_artifacts_must_match_required_artifact_identities(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    artifacts = payload["recognized_artifacts"]
    artifacts[0] = dict(artifacts[0])
    artifacts[0]["sha256"] = "cd" * 32
    payload["recognized_artifacts"] = artifacts
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_recognized_artifact_identity_drift_fails_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}_{kind_name}"
        root.mkdir()
        payload = gate_summary(gate_name)
        required_artifact = payload["required"][kind_name]["artifacts"][0]
        recognized_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
            and artifact["path"] == required_artifact["path"]
            and artifact["sha256"] == required_artifact["sha256"]
        )
        forged_digest = f"{(index + 1) % 256:02x}" * 32
        if forged_digest == required_artifact["sha256"]:
            forged_digest = "cd" * 32
        payload["recognized_artifacts"][recognized_index]["sha256"] = forged_digest
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert "recognized_artifacts must match required artifact identities" in errors
        assert forged_digest not in result_text


def test_recognized_artifacts_must_match_required_artifact_metadata(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    artifacts = payload["recognized_artifacts"]
    artifacts[0] = dict(artifacts[0])
    artifacts[0]["fingerprint"] = dict(artifacts[0]["fingerprint"])
    artifacts[0]["fingerprint"]["generated_at_unix"] = GENERATED_AT + 1
    payload["recognized_artifacts"] = artifacts
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_recognized_artifact_metadata_bindings_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name) in enumerate(cases):
        for metadata_field in ("schema", "status", "fingerprint"):
            root = tmp_path / f"{index}_{gate_name}_{kind_name}_{metadata_field}"
            root.mkdir()
            payload = gate_summary(gate_name)
            required_artifact = payload["required"][kind_name]["artifacts"][0]
            recognized_index = next(
                artifact_index
                for artifact_index, artifact in enumerate(
                    payload["recognized_artifacts"]
                )
                if artifact["kind"] == kind_name
                and artifact["path"] == required_artifact["path"]
                and artifact["sha256"] == required_artifact["sha256"]
            )
            recognized_artifact = payload["recognized_artifacts"][recognized_index]
            if metadata_field == "schema":
                forged_value = f"sorafs.private.runtime.schema.{index:03d}.v1"
                recognized_artifact["schema"] = forged_value
            elif metadata_field == "status":
                forged_value = "verified"
                assert required_artifact["status"] != forged_value
                recognized_artifact["status"] = forged_value
            else:
                forged_value = f"runtime-only-fingerprint-{index:03d}"
                recognized_artifact["fingerprint"] = dict(
                    recognized_artifact["fingerprint"]
                )
                recognized_artifact["fingerprint"]["aggregate_binding_nonce"] = (
                    forged_value
                )
            summary = root / "summary.json"
            write_json(root / f"{gate_name}.json", payload)

            assert (
                run_gate(
                    root,
                    "--require-gate",
                    gate_name,
                    "--summary-out",
                    str(summary),
                )
                == 1
            )

            result_text = summary.read_text(encoding="utf-8")
            errors = "\n".join(json.loads(result_text)["errors"])
            assert (
                f"recognized_artifacts[{recognized_index}].{metadata_field} "
                "must match the required artifact metadata"
                in errors
            )
            assert forged_value not in result_text


def test_recognized_artifacts_duplicate_paths_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    artifacts = payload["recognized_artifacts"]
    artifacts[0] = dict(artifacts[0])
    artifacts[0]["path"] = artifacts[1]["path"]
    artifacts[0]["sha256"] = artifacts[1]["sha256"]
    artifacts[0]["fingerprint"] = dict(artifacts[1]["fingerprint"])
    payload["recognized_artifacts"] = artifacts
    payload["evidence_file_count"] = len({artifact["path"] for artifact in artifacts})
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert "recognized_artifacts must not duplicate artifact paths" in "\n".join(
        result["errors"]
    )


def test_recognized_artifact_duplicate_paths_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [
        (gate.name, kind_name, gate.required_kinds)
        for gate in MODULE.GATE_SUMMARY_KINDS
        for kind_name in gate.required_kinds
    ]
    assert cases

    for index, (gate_name, kind_name, required_kinds) in enumerate(cases):
        donor_kind = next(candidate for candidate in required_kinds if candidate != kind_name)
        root = tmp_path / f"{index}_{gate_name}_{kind_name}"
        root.mkdir()
        payload = gate_summary(gate_name)
        target_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == kind_name
        )
        donor_index = next(
            artifact_index
            for artifact_index, artifact in enumerate(payload["recognized_artifacts"])
            if artifact["kind"] == donor_kind
        )
        donor_path = payload["recognized_artifacts"][donor_index]["path"]
        payload["recognized_artifacts"][target_index]["path"] = donor_path
        payload["evidence_file_count"] = len(
            {artifact["path"] for artifact in payload["recognized_artifacts"]}
        )
        summary = root / "summary.json"
        write_json(root / f"{gate_name}.json", payload)

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result_text = summary.read_text(encoding="utf-8")
        errors = "\n".join(json.loads(result_text)["errors"])
        assert "recognized_artifacts must not duplicate artifact paths" in errors
        assert "recognized_artifacts must match required artifact identities" in errors
        assert donor_path not in result_text


def test_artifact_fingerprint_metadata_must_be_payload_free(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["artifacts"][0]["fingerprint"]["optional"] = None
    payload["recognized_artifacts"][0]["fingerprint"]["optional"] = None
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        ".fingerprint.optional must contain only payload-free canonical metadata"
        in "\n".join(result["errors"])
    )


def test_aggregate_output_field_inventories_are_schema_closed() -> None:
    field_sets = {
        "PAYLOAD_FREE_ARTIFACT_FIELDS": MODULE.PAYLOAD_FREE_ARTIFACT_FIELDS,
        "PAYLOAD_FREE_REQUIRED_ROW_FIELDS": MODULE.PAYLOAD_FREE_REQUIRED_ROW_FIELDS,
        "AGGREGATE_REQUIRED_GATE_ROW_FIELDS": (
            MODULE.AGGREGATE_REQUIRED_GATE_ROW_FIELDS
        ),
        "AGGREGATE_MISSING_GATE_ROW_FIELDS": (
            MODULE.AGGREGATE_MISSING_GATE_ROW_FIELDS
        ),
        "AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS": (
            MODULE.AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS
        ),
        "AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS": (
            MODULE.AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS
        ),
        "AGGREGATE_SUMMARY_FIELDS": MODULE.AGGREGATE_SUMMARY_FIELDS,
    }

    assert MODULE.PAYLOAD_FREE_ARTIFACT_FIELDS == frozenset(
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
    assert MODULE.PAYLOAD_FREE_REQUIRED_ROW_FIELDS == frozenset(
        {"schema", "present", "valid", "artifact_count", "artifacts", "errors"}
    )
    assert MODULE.AGGREGATE_MISSING_GATE_ROW_FIELDS == frozenset(
        {"schema", "present", "valid", "errors"}
    )
    assert MODULE.AGGREGATE_REQUIRED_GATE_ROW_FIELDS == frozenset(
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
    assert MODULE.AGGREGATE_SUMMARY_FIELDS == frozenset(
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

    assert MODULE.AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS == (
        frozenset({"schema", "present", "valid", "errors"})
    )
    assert MODULE.AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS == frozenset(
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

    assert (
        MODULE.AGGREGATE_MISSING_GATE_ROW_FIELDS
        < MODULE.AGGREGATE_REQUIRED_GATE_ROW_FIELDS
    )
    for fields in field_sets.values():
        assert fields
        assert all(MODULE.canonical_string(field) == field for field in fields)


def test_aggregate_gate_row_output_shape_is_validated() -> None:
    gate = MODULE.GATE_BY_NAME["gateway_load"]
    payload = gate_summary("gateway_load")
    row, validation_errors = MODULE.validate_gate_summary(
        gate,
        payload,
        MODULE.ValidationOptions(
            now_unix=NOW_UNIX,
            max_summary_artifact_age_secs=MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
            deployment_id=DEPLOYMENT_ID,
            environment=ENVIRONMENT,
        ),
    )
    assert validation_errors == []
    row["path"] = "gateway_load.json"
    row["sha256"] = SHA256

    errors: list[str] = []
    MODULE.validate_aggregate_gate_row_output(gate, row, errors)
    assert errors == []

    row["private_key"] = "runtime-only-key-material"
    row["path"] = "/tmp/runtime-only/gateway_load.json"
    row["sha256"] = "AB" * 32
    row["required_kind_count"] = 0
    row["expected_required_kind_count"] = len(gate.required_kinds) + 1
    row["artifact_count"] = 1
    row["recognized_artifact_count"] = 2
    row["evidence_file_count"] = 3
    row["expected_required_kinds"] = list(reversed(row["expected_required_kinds"]))
    row["newest_generated_at_unix"] = row["oldest_generated_at_unix"] - 1
    row["environment"] = "staging"
    row["errors"] = ["row drifted"]
    MODULE.validate_aggregate_gate_row_output(gate, row, errors)
    diagnostics = "\n".join(errors)
    assert (
        "gateway_load aggregate row fields must match the schema-closed output contract"
        in diagnostics
    )
    assert "gateway_load aggregate row <sensitive-key> is not allowed" in diagnostics
    assert (
        "gateway_load aggregate row path must be archive-relative without "
        "absolute, empty, current, parent, or platform-specific segments"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row sha256 must be canonical lowercase SHA-256"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row expected_required_kinds must match gate contract"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row required_kind_count must match gate contract"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row expected_required_kind_count must match gate contract"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row recognized_artifact_count must match artifact_count"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row evidence_file_count must not exceed recognized_artifact_count"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row newest_generated_at_unix must be >= oldest_generated_at_unix"
        in diagnostics
    )
    assert "gateway_load aggregate row environment must be production" in diagnostics
    assert "gateway_load aggregate row errors must be empty" in diagnostics
    assert "private_key" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "/tmp/runtime-only" not in diagnostics
    assert "AB" * 32 not in diagnostics


def test_aggregate_required_row_output_contracts_fail_closed_from_config() -> None:
    cases = [(gate.name, gate) for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases

    for index, (gate_name, gate) in enumerate(cases):
        payload = gate_summary(gate_name)
        row, validation_errors = MODULE.validate_gate_summary(
            gate,
            payload,
            MODULE.ValidationOptions(
                now_unix=NOW_UNIX,
                max_summary_artifact_age_secs=(
                    MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS
                ),
                deployment_id=DEPLOYMENT_ID,
                environment=ENVIRONMENT,
            ),
        )
        assert validation_errors == []
        row["path"] = f"{gate_name}.json"
        row["sha256"] = SHA256

        errors: list[str] = []
        MODULE.validate_aggregate_required_row_output(gate, row, errors)
        assert errors == []

        secret = f"runtime-only-private-row-key-{index:03d}"
        row["private_key"] = secret
        row["path"] = f"/tmp/runtime-only/{gate_name}.json"
        row["sha256"] = "AB" * 32
        row["required_kind_count"] = 0
        row["expected_required_kind_count"] = len(gate.required_kinds) + 1
        row["artifact_count"] = 1
        row["recognized_artifact_count"] = 2
        row["evidence_file_count"] = 3
        row["expected_required_kinds"] = list(reversed(row["expected_required_kinds"]))
        row["newest_generated_at_unix"] = row["oldest_generated_at_unix"] - 1
        row["environment"] = "staging"
        row["errors"] = ["row drifted"]
        errors = []
        MODULE.validate_aggregate_required_row_output(gate, row, errors)
        diagnostics = "\n".join(errors)
        assert (
            f"{gate_name} aggregate row fields must match the schema-closed "
            "output contract"
            in diagnostics
        )
        assert f"{gate_name} aggregate row <sensitive-key> is not allowed" in diagnostics
        assert (
            f"{gate_name} aggregate row required_kind_count must match gate contract"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate row expected_required_kind_count must match "
            "gate contract"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate row expected_required_kinds must match gate contract"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate row recognized_artifact_count must match "
            "artifact_count"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate row evidence_file_count must not exceed "
            "recognized_artifact_count"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate row newest_generated_at_unix must be >= "
            "oldest_generated_at_unix"
            in diagnostics
        )
        assert f"{gate_name} aggregate row environment must be production" in diagnostics
        assert f"{gate_name} aggregate row errors must be empty" in diagnostics
        assert "private_key" not in diagnostics
        assert secret not in diagnostics
        assert "/tmp/runtime-only" not in diagnostics
        assert "AB" * 32 not in diagnostics

        invalid_row = copy.deepcopy(row)
        invalid_row.pop("private_key")
        invalid_row["valid"] = False
        invalid_row["errors"] = ["canonical invalid row diagnostic"]
        invalid_row["evidence_file_count"] = -1
        invalid_row["recognized_artifact_count"] = 2
        invalid_row["artifact_count"] = 1
        invalid_row["required_kind_count"] = 0
        invalid_row["expected_required_kind_count"] = len(gate.required_kinds) + 1
        invalid_row["expected_required_kinds"] = []
        invalid_row["newest_generated_at_unix"] = invalid_row[
            "oldest_generated_at_unix"
        ] - 1
        errors = []
        MODULE.validate_aggregate_required_row_output(gate, invalid_row, errors)
        diagnostics = "\n".join(errors)
        assert (
            f"{gate_name} aggregate invalid row evidence_file_count must be a "
            "non-negative integer"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate invalid row recognized_artifact_count must "
            "match artifact_count"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate invalid row required_kind_count must match "
            "gate contract"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate invalid row expected_required_kind_count must "
            "match gate contract"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate invalid row expected_required_kinds must match "
            "gate contract"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate invalid row newest_generated_at_unix must be "
            ">= oldest_generated_at_unix"
            in diagnostics
        )


def test_aggregate_summary_output_shape_is_validated(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    write_foundational_summary(tmp_path)
    options = production_validation_options()
    summary, build_errors = MODULE.build_summary(
        [tmp_path],
        [],
        ("gateway_load",),
        options,
        None,
    )
    assert build_errors == []
    assert summary["status"] == "ready"

    errors: list[str] = []
    MODULE.validate_aggregate_summary_output(summary, ("gateway_load",), errors)
    assert errors == []

    summary["status"] = "blocked"
    MODULE.validate_aggregate_summary_output(summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert "aggregate summary status must match aggregate diagnostics" in diagnostics

    errors = []
    summary["status"] = "ready"
    summary["errors"] = ["drifted aggregate diagnostic"]
    MODULE.validate_aggregate_summary_output(summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert "aggregate summary status must match aggregate diagnostics" in diagnostics

    errors = []
    summary["status"] = "ready"
    summary["errors"] = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["deployment"] = {}
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary ready deployment must include deployment_id and environment"
        in diagnostics
    )

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["deployment"]["environment"] = "staging"
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert "aggregate summary environment must be production" in diagnostics

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["deployment"]["deployment_id"] = "gateway-staging-a"
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary deployment_id must not contain non-production "
        "deployment markers ['staging']"
        in diagnostics
    )

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["deployment"]["deployment_id"] = "gateway-stagingready-a"
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary deployment_id must not contain non-production "
        "deployment markers ['staging']"
        in diagnostics
    )

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["required"]["gateway_load"]["deployment_id"] = (
        "sorafs-mainnet-2026-07"
    )
    ready_summary["required"]["gateway_load"]["environment"] = "prod"
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "gateway_load aggregate required row deployment_id must match "
        "aggregate deployment_id"
        in diagnostics
    )
    assert (
        "gateway_load aggregate required row environment must match "
        "aggregate environment"
        in diagnostics
    )

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["recognized_summary_count"] = 0
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary ready recognized_summary_count must match required gate count"
        in diagnostics
    )

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["summary_file_count"] = 2
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary ready summary_file_count must match required gate count"
        in diagnostics
    )

    errors = []
    drifted_threshold_summary = copy.deepcopy(summary)
    drifted_threshold_summary["thresholds"]["shadow_threshold"] = 1
    MODULE.validate_aggregate_summary_output(
        drifted_threshold_summary,
        ("gateway_load",),
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary thresholds must contain only max_summary_artifact_age_secs"
        in diagnostics
    )
    assert "shadow_threshold" not in diagnostics

    errors = []
    drifted_required_gates_summary = copy.deepcopy(summary)
    drifted_required_gates_summary["required_gates"] = [
        "gateway_load",
        "gateway_load",
        "shadow_gate",
        "bad\ngate",
    ]
    MODULE.validate_aggregate_summary_output(
        drifted_required_gates_summary,
        ("gateway_load",),
        errors,
    )
    diagnostics = "\n".join(errors)
    assert "aggregate summary required_gates must match requested gates" in diagnostics
    assert (
        "aggregate summary required_gates must contain canonical strings"
        in diagnostics
    )
    assert (
        "aggregate summary required_gates must not contain duplicate gates"
        in diagnostics
    )
    assert (
        "aggregate summary required_gates must use known gate names" in diagnostics
    )
    assert "bad\ngate" not in diagnostics
    assert "shadow_gate" not in diagnostics

    errors = []
    drifted_required_gates_summary = copy.deepcopy(summary)
    drifted_required_gates_summary["required_gates"] = "gateway_load"
    MODULE.validate_aggregate_summary_output(
        drifted_required_gates_summary,
        ("gateway_load",),
        errors,
    )
    diagnostics = "\n".join(errors)
    assert "aggregate summary required_gates must be a list" in diagnostics
    assert "gateway_load" not in diagnostics

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["required"]["gateway_load"]["valid"] = False
    ready_summary["required"]["gateway_load"]["errors"] = ["invalid required row"]
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert "aggregate summary ready rows must all be present and valid" in diagnostics

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["foundational_prerequisites"]["valid"] = False
    ready_summary["foundational_prerequisites"]["errors"] = [
        "invalid foundational row"
    ]
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary ready foundational prerequisites must be present and valid"
        in diagnostics
    )

    errors = []
    ready_summary = copy.deepcopy(summary)
    runtime_secret = "runtime-only-foundational-private-key"
    ready_summary["foundational_prerequisites"]["private_key"] = runtime_secret
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate foundational prerequisites fields must match the schema-closed output contract"
        in diagnostics
    )
    assert (
        "aggregate foundational prerequisites <sensitive-key> is not allowed"
        in diagnostics
    )
    assert runtime_secret not in diagnostics
    assert "private_key" not in diagnostics

    errors = []
    duplicate_error_summary = copy.deepcopy(summary)
    duplicate_error_summary["status"] = "blocked"
    duplicate_error_summary["errors"] = [
        "drifted aggregate diagnostic",
        "drifted aggregate diagnostic",
    ]
    MODULE.validate_aggregate_summary_output(
        duplicate_error_summary,
        ("gateway_load",),
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary errors must not contain duplicate diagnostics"
        in diagnostics
    )

    errors = []
    summary["private_key"] = "runtime-only-key-material"
    summary["status"] = "done"
    summary["required_gates"] = ["gateway_load", "shadow_gate"]
    summary["recognized_summary_count"] = 0
    summary["deployment"] = {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "private_key": "runtime-only-key-material",
    }
    summary["errors"] = ["bad\nerror", "bad\u200derror", "bad\u202eerror"]
    MODULE.validate_aggregate_summary_output(summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary fields must match the schema-closed output contract"
        in diagnostics
    )
    assert "aggregate summary <sensitive-key> is not allowed" in diagnostics
    assert "aggregate summary status must be ready, failed, or blocked" in diagnostics
    assert "aggregate summary required_gates must match requested gates" in diagnostics
    assert (
        "aggregate summary recognized_summary_count must match present required rows"
        in diagnostics
    )
    assert (
        "aggregate summary deployment fields must be deployment_id and environment"
        in diagnostics
    )
    assert "aggregate summary deployment <sensitive-key> is not allowed" in diagnostics
    assert "aggregate summary errors must contain canonical strings" in diagnostics
    assert "private_key" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "bad\nerror" not in diagnostics
    assert "bad\u200derror" not in diagnostics
    assert "bad\u202eerror" not in diagnostics


def test_aggregate_summary_output_contracts_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    cases = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert cases
    options = production_validation_options()

    for index, gate_name in enumerate(cases):
        root = tmp_path / f"{index}_{gate_name}"
        root.mkdir()
        write_gate(root, gate_name)
        write_foundational_summary(root)
        summary, build_errors = MODULE.build_summary(
            [root],
            [],
            (gate_name,),
            options,
            None,
        )
        assert build_errors == []
        assert summary["status"] == "ready"

        errors: list[str] = []
        MODULE.validate_aggregate_summary_output(summary, (gate_name,), errors)
        assert errors == []

        hidden_gate = f"runtime_only_shadow_gate_{index:03d}"
        bad_gate = f"bad\n{index}"
        drifted_required_gates = copy.deepcopy(summary)
        drifted_required_gates["required_gates"] = [
            gate_name,
            gate_name,
            hidden_gate,
            bad_gate,
        ]
        errors = []
        MODULE.validate_aggregate_summary_output(
            drifted_required_gates,
            (gate_name,),
            errors,
        )
        diagnostics = "\n".join(errors)
        assert "aggregate summary required_gates must match requested gates" in diagnostics
        assert (
            "aggregate summary required_gates must contain canonical strings"
            in diagnostics
        )
        assert (
            "aggregate summary required_gates must not contain duplicate gates"
            in diagnostics
        )
        assert (
            "aggregate summary required_gates must use known gate names" in diagnostics
        )
        assert hidden_gate not in diagnostics
        assert bad_gate not in diagnostics

        count_drift = copy.deepcopy(summary)
        count_drift["recognized_summary_count"] = (
            count_drift["summary_file_count"] + 1
        )
        errors = []
        MODULE.validate_aggregate_summary_output(count_drift, (gate_name,), errors)
        diagnostics = "\n".join(errors)
        assert (
            "aggregate summary recognized_summary_count must not exceed summary_file_count"
            in diagnostics
        )
        assert (
            "aggregate summary recognized_summary_count must not exceed required gate count"
            in diagnostics
        )
        assert (
            "aggregate summary ready recognized_summary_count must match required gate count"
            in diagnostics
        )
        assert (
            "aggregate summary recognized_summary_count must match present required rows"
            in diagnostics
        )

        malformed_count = copy.deepcopy(summary)
        malformed_count["summary_file_count"] = True
        malformed_count["recognized_summary_count"] = -1
        errors = []
        MODULE.validate_aggregate_summary_output(malformed_count, (gate_name,), errors)
        diagnostics = "\n".join(errors)
        assert (
            "aggregate summary summary_file_count must be a non-negative integer"
            in diagnostics
        )
        assert (
            "aggregate summary recognized_summary_count must be a non-negative integer"
            in diagnostics
        )

        secret = f"runtime-only-private-summary-{index:03d}"
        sensitive_drift = copy.deepcopy(summary)
        sensitive_drift["private_key"] = secret
        sensitive_drift["deployment"]["private_key"] = secret
        errors = []
        MODULE.validate_aggregate_summary_output(
            sensitive_drift,
            (gate_name,),
            errors,
        )
        diagnostics = "\n".join(errors)
        assert (
            "aggregate summary fields must match the schema-closed output contract"
            in diagnostics
        )
        assert "aggregate summary <sensitive-key> is not allowed" in diagnostics
        assert (
            "aggregate summary deployment fields must be deployment_id and environment"
            in diagnostics
        )
        assert (
            "aggregate summary deployment <sensitive-key> is not allowed"
            in diagnostics
        )
        assert "private_key" not in diagnostics
        assert secret not in diagnostics

        deployment_drift = copy.deepcopy(summary)
        deployment_drift["deployment"] = {
            "deployment_id": f"sorafs-staging-{index:03d}",
            "environment": "staging",
        }
        errors = []
        MODULE.validate_aggregate_summary_output(
            deployment_drift,
            (gate_name,),
            errors,
        )
        diagnostics = "\n".join(errors)
        assert (
            "aggregate summary deployment_id must not contain non-production "
            "deployment markers ['staging']"
            in diagnostics
        )
        assert "aggregate summary environment must be production" in diagnostics
        assert f"sorafs-staging-{index:03d}" not in diagnostics

        row_drift = copy.deepcopy(summary)
        row_drift["required"][gate_name]["deployment_id"] = (
            f"sorafs-mainnet-{index:03d}"
        )
        row_drift["required"][gate_name]["environment"] = "prod"
        errors = []
        MODULE.validate_aggregate_summary_output(row_drift, (gate_name,), errors)
        diagnostics = "\n".join(errors)
        assert (
            f"{gate_name} aggregate required row deployment_id must match "
            "aggregate deployment_id"
            in diagnostics
        )
        assert (
            f"{gate_name} aggregate required row environment must match "
            "aggregate environment"
            in diagnostics
        )
        assert f"sorafs-mainnet-{index:03d}" not in diagnostics


def test_aggregate_summary_output_rejects_overcounted_ready_inventory(
    tmp_path: Path,
) -> None:
    write_gate(tmp_path, "gateway_load")
    write_foundational_summary(tmp_path)
    options = production_validation_options()
    summary, build_errors = MODULE.build_summary(
        [tmp_path],
        [],
        ("gateway_load",),
        options,
        None,
    )
    assert build_errors == []
    assert summary["status"] == "ready"

    errors: list[str] = []
    over_file_count = copy.deepcopy(summary)
    over_file_count["recognized_summary_count"] = (
        over_file_count["summary_file_count"] + 1
    )
    MODULE.validate_aggregate_summary_output(
        over_file_count,
        ("gateway_load",),
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary recognized_summary_count must not exceed summary_file_count"
        in diagnostics
    )
    assert (
        "aggregate summary ready recognized_summary_count must match required gate count"
        in diagnostics
    )

    errors = []
    over_required_count = copy.deepcopy(summary)
    over_required_count["summary_file_count"] = 3
    over_required_count["recognized_summary_count"] = 2
    MODULE.validate_aggregate_summary_output(
        over_required_count,
        ("gateway_load",),
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary recognized_summary_count must not exceed required gate count"
        in diagnostics
    )

    errors = []
    unknown_row = copy.deepcopy(summary)
    unknown_row["required"]["shadow_gate"] = copy.deepcopy(
        unknown_row["required"]["gateway_load"]
    )
    MODULE.validate_aggregate_summary_output(
        unknown_row,
        ("gateway_load",),
        errors,
    )
    diagnostics = "\n".join(errors)
    assert "aggregate summary required rows must match requested gates" in diagnostics
    assert "aggregate summary required rows must use known gate names" in diagnostics
    assert "shadow_gate" not in diagnostics


def test_aggregate_required_row_output_shape_is_validated(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    write_foundational_summary(tmp_path)
    options = production_validation_options()
    summary, build_errors = MODULE.build_summary(
        [tmp_path],
        [],
        ("gateway_load", "reputation"),
        options,
        None,
    )
    assert summary["status"] == "blocked"
    assert build_errors == [
        "missing required reputation production readiness summary",
    ]

    errors: list[str] = []
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["gateway_load"],
        summary["required"]["gateway_load"],
        errors,
    )
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["reputation"],
        summary["required"]["reputation"],
        errors,
    )
    assert errors == []
    valid_gateway_row = copy.deepcopy(summary["required"]["gateway_load"])

    missing_row = dict(summary["required"]["reputation"])
    missing_row["errors"] = [
        "missing required reputation production readiness summary",
        "private-key-placeholder drift",
    ]
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["reputation"],
        missing_row,
        errors,
    )

    summary["required"]["gateway_load"]["private_key"] = "runtime-only-key-material"
    summary["required"]["gateway_load"]["valid"] = False
    summary["required"]["gateway_load"]["errors"] = []
    summary["required"]["gateway_load"]["sha256"] = "AB" * 32
    summary["required"]["gateway_load"]["thresholds"] = {"bad\u200dkey": False}
    summary["required"]["gateway_load"]["evidence_file_count"] = None
    summary["required"]["gateway_load"]["recognized_artifact_count"] = "1"
    summary["required"]["gateway_load"]["artifact_count"] = False
    summary["required"]["gateway_load"]["required_kind_count"] = 0
    summary["required"]["gateway_load"]["expected_required_kind_count"] = 999
    summary["required"]["gateway_load"]["oldest_generated_at_unix"] = 0
    summary["required"]["gateway_load"]["newest_generated_at_unix"] = False
    summary["required"]["gateway_load"]["deployment_id"] = " runtime-only-deployment"
    summary["required"]["gateway_load"]["environment"] = "prod\u202esecret"
    summary["required"]["reputation"]["present"] = True
    summary["required"]["reputation"]["private_key"] = "runtime-only-key-material"
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["gateway_load"],
        summary["required"]["gateway_load"],
        errors,
    )
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["reputation"],
        summary["required"]["reputation"],
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "gateway_load aggregate required row fields must match the schema-closed output contract"
        in diagnostics
    )
    assert (
        "gateway_load aggregate required row <sensitive-key> is not allowed"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row sha256 must be canonical lowercase SHA-256"
        in diagnostics
    )
    assert "gateway_load aggregate invalid row errors must not be empty" in diagnostics
    assert (
        "gateway_load aggregate invalid row evidence_file_count must be a non-negative integer"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row recognized_artifact_count must be a non-negative integer"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row artifact_count must be a non-negative integer"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row oldest_generated_at_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row newest_generated_at_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row required_kind_count must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row required_kind_count must match gate contract"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row expected_required_kind_count must match gate contract"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row thresholds.<invalid> must be a non-negative integer"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row deployment_id must be a non-empty canonical string"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row environment must be canonical when present"
        in diagnostics
    )
    assert (
        "reputation aggregate required row fields must match the schema-closed output contract"
        in diagnostics
    )
    assert (
        "reputation aggregate required row <sensitive-key> is not allowed"
        in diagnostics
    )
    assert "bad\u200dkey" not in diagnostics
    assert "prod\u202esecret" not in diagnostics
    assert (
        "reputation aggregate missing row errors must match the deterministic missing summary diagnostic"
        in diagnostics
    )
    assert "private_key" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "private-key-placeholder drift" not in diagnostics
    assert "AB" * 32 not in diagnostics
    assert "runtime-only-deployment" not in diagnostics
    assert "prod\nsecret" not in diagnostics

    errors = []
    invalid_count_row = copy.deepcopy(valid_gateway_row)
    invalid_count_row["valid"] = False
    invalid_count_row["errors"] = ["invalid production readiness summary"]
    invalid_count_row["artifact_count"] = 1
    invalid_count_row["recognized_artifact_count"] = 2
    invalid_count_row["evidence_file_count"] = 3
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["gateway_load"],
        invalid_count_row,
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "gateway_load aggregate invalid row recognized_artifact_count must match artifact_count"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row evidence_file_count must not exceed recognized_artifact_count"
        in diagnostics
    )

    errors = []
    invalid_row = copy.deepcopy(summary["required"]["gateway_load"])
    invalid_row["private_key"] = "runtime-only-key-material"
    invalid_row["valid"] = False
    invalid_row["errors"] = ["invalid production readiness summary"]
    invalid_row["environment"] = "staging"
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["gateway_load"],
        invalid_row,
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "gateway_load aggregate invalid row environment must be production when present"
        in diagnostics
    )
    assert "staging" not in diagnostics

    errors = []
    invalid_row = copy.deepcopy(summary["required"]["gateway_load"])
    invalid_row["valid"] = False
    invalid_row["errors"] = [
        "invalid production readiness summary",
        "invalid production readiness summary",
    ]
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["gateway_load"],
        invalid_row,
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "gateway_load aggregate invalid row errors must not contain duplicate diagnostics"
        in diagnostics
    )


def test_duplicate_gate_summary_fails(tmp_path: Path) -> None:
    first = write_gate(tmp_path, "gateway_load")
    second = tmp_path / "gateway_load_duplicate.json"
    second.write_text(first.read_text(encoding="utf-8"), encoding="utf-8")
    third = tmp_path / "gateway_load_duplicate_2.json"
    third.write_text(first.read_text(encoding="utf-8"), encoding="utf-8")
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    row_errors = result["required"]["gateway_load"]["errors"]
    assert row_errors.count("duplicate gateway_load production readiness summary") == 1
    assert (
        result["errors"].count("duplicate gateway_load production readiness summary")
        == 2
    )

    errors: list[str] = []
    result["required"]["gateway_load"]["errors"] = [
        "duplicate gateway_load production readiness summary",
        "duplicate gateway_load production readiness summary",
    ]
    MODULE.validate_duplicate_summary_diagnostics(
        result["required"],
        {"gateway_load"},
        2,
        errors,
    )
    assert (
        "gateway_load duplicate summary row errors must contain the deterministic duplicate summary diagnostic exactly once"
        in "\n".join(errors)
    )
    errors = []
    result["required"]["gateway_load"]["errors"] = [
        "duplicate gateway_load production readiness summary"
    ]
    MODULE.validate_duplicate_summary_diagnostics(
        result["required"],
        {"gateway_load"},
        3,
        errors,
    )
    assert (
        "aggregate summary duplicate-summary diagnostics must match duplicate summary inputs"
        in "\n".join(errors)
    )


def test_duplicate_and_unrequired_summaries_fail_closed_from_config(
    tmp_path: Path,
) -> None:
    gate_names = [gate.name for gate in MODULE.GATE_SUMMARY_KINDS]
    assert len(gate_names) > 1

    for index, gate_name in enumerate(gate_names):
        root = tmp_path / f"{index}_{gate_name}_duplicate"
        root.mkdir()
        first = write_gate(root, gate_name)
        for duplicate_index in (1, 2):
            duplicate = root / f"{gate_name}_duplicate_{duplicate_index}.json"
            duplicate.write_text(first.read_text(encoding="utf-8"), encoding="utf-8")
        summary = root / "summary.json"

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        duplicate_error = f"duplicate {gate_name} production readiness summary"
        row_errors = result["required"][gate_name]["errors"]
        assert row_errors.count(duplicate_error) == 1
        assert result["errors"].count(duplicate_error) == 2
        assert f"{gate_name}_duplicate_" not in "\n".join(result["errors"])

    for index, gate_name in enumerate(gate_names):
        unrequired_gate = gate_names[(index + 1) % len(gate_names)]
        root = tmp_path / f"{index}_{gate_name}_unrequired"
        root.mkdir()
        required_summary = write_gate(root, gate_name)
        unrequired_summary = write_gate(root, unrequired_gate)
        summary = root / "summary.json"

        assert (
            MODULE.main(
                [
                    "--evidence",
                    str(required_summary),
                    "--evidence",
                    str(unrequired_summary),
                    "--require-gate",
                    gate_name,
                    "--now-unix",
                    str(NOW_UNIX),
                    "--deployment-id",
                    DEPLOYMENT_ID,
                    "--environment",
                    ENVIRONMENT,
                    "--summary-out",
                    str(summary),
                ]
            )
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        errors = "\n".join(result["errors"])
        assert (
            result["errors"].count(
                "explicit production readiness summary belongs to unrequired gate"
            )
            == 1
        )
        assert MODULE.GATE_BY_NAME[unrequired_gate].schema not in errors
        assert f"{unrequired_gate}` gate" not in errors
