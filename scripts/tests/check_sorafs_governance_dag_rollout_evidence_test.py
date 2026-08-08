"""Tests for scripts/check_sorafs_governance_dag_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "check_sorafs_governance_dag_rollout_evidence.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_governance_dag_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TEST_DIR = Path(__file__).resolve().parent
if str(TEST_DIR) not in sys.path:
    sys.path.insert(0, str(TEST_DIR))
from sorafs_rollout_runner_test_support import TopologyBoundChecker  # noqa: E402


NOW_UNIX = 1_800_300_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32
DIGEST_3 = "ef" * 32
DIGEST_4 = "12" * 32
DIGEST_5 = "34" * 32
DEPLOYMENT_ID = "governance-dag-production-a"
ENVIRONMENT = "production"
CHECKER = TopologyBoundChecker(
    MODULE.main,
    deployment_id=DEPLOYMENT_ID,
    environment=ENVIRONMENT,
    name="governance-dag-checker",
)


def payload_kinds() -> list[str]:
    return list(MODULE.REQUIRED_PAYLOAD_KINDS)


def block_refs(count: int) -> list[str]:
    return [f"governance-dag-block-{index:02d}" for index in range(count)]


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
    }


def ingest_service() -> dict:
    payload = base("sorafs.governance_dag.ingest_service_canary.v1")
    payload.update(
        {
            "daemonized": True,
            "payload_validation_enabled": True,
            "publisher_signature_verified": True,
            "dedupe_by_digest_enabled": True,
            "quarantine_invalid_blocks": True,
            "source_count": 8,
            "payload_kinds": payload_kinds(),
            "payload_bytes_included": False,
        }
    )
    return payload


def publisher_service(*, head_age: int = 300, block_count: int = 6) -> dict:
    payload = base("sorafs.governance_dag.publisher_service_canary.v1")
    payload.update(
        {
            "dag_builder_daemonized": True,
            "kubo_unixfs_profile": MODULE.KUBO_UNIXFS_PROFILE,
            "unixfs_chunk_size_bytes": MODULE.KUBO_UNIXFS_CHUNK_SIZE_BYTES,
            "unixfs_raw_leaves": True,
            "unixfs_balanced_layout": True,
            "unixfs_max_links_per_node": MODULE.KUBO_UNIXFS_MAX_LINKS_PER_NODE,
            "cid_version": MODULE.KUBO_CID_VERSION,
            "cid_multihash": MODULE.KUBO_CID_MULTIHASH,
            "locally_derived_cids_verified": True,
            "signed_http_head_cas_enabled": True,
            "strong_single_etag_verified": True,
            "conditional_cas_readback_verified": True,
            "signed_head_verified": True,
            "parent_chain_verified": True,
            "objects_pinned": True,
            "authenticated_ingress_qualified": True,
            "ingress_enforcement": MODULE.INGRESS_ENFORCEMENT,
            "replay_posture": MODULE.REPLAY_POSTURE,
            "ingress_scope_binding_verified": True,
            "receiver_policy_digest_hex": DIGEST,
            "replay_namespace_digest_hex": DIGEST_2,
            "replica_set_digest_hex": DIGEST_3,
            "kubo_ingress_binding_digest_hex": DIGEST_4,
            "signed_head_ingress_binding_digest_hex": DIGEST_5,
            "public_head_cid_hex": DIGEST,
            "policy_digest_hex": DIGEST,
            "pin_lag_seconds": 120,
            "head_age_seconds": head_age,
            "block_count": block_count,
            "block_refs": block_refs(block_count),
            "payload_kind_count": 8,
            "payload_kinds": payload_kinds(),
            "raw_head_included": False,
        }
    )
    return payload


def mirror_datastore(*, drift: bool = False) -> dict:
    payload = base("sorafs.governance_dag.mirror_datastore_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "sealed_typed_store_enabled": True,
            "query_service_enabled": True,
            "mirror_index_verified": True,
            "head_lookup_verified": True,
            "block_lookup_verified": True,
            "node_lookup_verified": True,
            "digest_lookup_verified": True,
            "retention_max_entries": MODULE.MIRROR_RETENTION_MAX_ENTRIES,
            "retention_max_bytes": MODULE.MIRROR_RETENTION_MAX_BYTES,
            "exact_retained_source_suffix_verified": True,
            "fresh_checkpoint_coherent_reads_verified": True,
            "liveness_bound_reader_verified": True,
            "mirror_drift_detected": drift,
            "missing_block_count": 0,
            "raw_blocks_included": False,
        }
    )
    return payload


def operator_recovery() -> dict:
    payload = base("sorafs.governance_dag.operator_recovery_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "live_head_fetch_verified": True,
            "public_checkpoint_published": True,
            "checkpoint_recovery_verified": True,
            "derived_mirror_recovery_verified": True,
            "recovered_head_matches_public_head": True,
            "post_loss_repair_verified": True,
            "head_object_repaired_with_same_cid": True,
            "block_object_repaired_with_same_cid": True,
            "public_head_unchanged_during_repair": True,
            "checkpoint_digest_hex": DIGEST,
            "raw_checkpoint_included": False,
        }
    )
    return payload


def dashboard_api(*, latency_ms: int = 200, passed: bool = True) -> dict:
    routes = [
        {
            "name": name,
            "passed": passed,
            "status_code": 200,
            "body_blake3_hex": DIGEST,
            "latency_ms": latency_ms,
            "publisher_identity_present": True,
            "verification_valid": True,
        }
        for name in (
            "dashboard",
            "head",
            "block_lookup",
            "node_lookup",
            "digest_lookup",
            "checkpoint",
        )
    ]
    payload = base("sorafs.governance_dag.dashboard_api_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "route_count": len(routes),
            "passed_route_count": len(routes) if passed else 0,
            "service_mirror_capability_installed": True,
            "fresh_checkpoint_coherent_reads_verified": True,
            "liveness_bound_reader_verified": True,
            "unready_reader_rejected": True,
            "reader_withdrawal_verified": True,
            "response_bodies_included": False,
            "routes": routes,
        }
    )
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.governance_dag.observability_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "publication_metrics_present": True,
            "first_full_audit_verified": True,
            "readiness_withheld_until_full_audit": True,
            "bounded_rotating_audit_verified": True,
            "audit_max_entries_per_poll": MODULE.STEADY_AUDIT_MAX_ENTRIES_PER_POLL,
            "audit_max_bytes_per_poll": MODULE.STEADY_AUDIT_MAX_BYTES_PER_POLL,
            "critical_alerts_firing": critical,
            "metrics": [
                "sorafs_governance_dag_publish_total",
                "sorafs_governance_dag_published_bytes_total",
                "sorafs_governance_dag_last_publish_timestamp_seconds",
                "sorafs_governance_dag_backlog",
                "sorafs_governance_dag_head_age_seconds",
                "sorafs_governance_dag_ipfs_pin_lag_seconds",
                "sorafs_governance_dag_validation_failure_total",
                "sorafs_governance_dag_mirror_drift",
            ],
            "metric_count": len(MODULE.REQUIRED_METRICS),
            "response_bodies_included": False,
        }
    )
    return payload


def publication_e2e(*, block_count: int = 6) -> dict:
    payload = base("sorafs.governance_dag.publication_e2e_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "local_kubo_tests_passed": True,
            "deterministic_unixfs_profile_verified": True,
            "signed_http_head_resolved": True,
            "strong_single_etag_cas_verified": True,
            "authenticated_ingress_qualification_verified": True,
            "replay_attack_rejected": True,
            "block_replay_verified": True,
            "duplicate_payload_rejected": True,
            "invalid_parent_quarantined": True,
            "post_loss_same_cid_repair_verified": True,
            "bounded_rotating_audit_verified": True,
            "fresh_torii_reads_verified": True,
            "stopped_service_reads_rejected": True,
            "publisher_key_failure_tested": True,
            "block_count": block_count,
            "block_refs": block_refs(block_count),
            "payload_kind_count": 8,
            "payload_kinds": payload_kinds(),
            "raw_blocks_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.governance_dag.governance_approval.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "publisher_keys_governed": True,
            "signed_http_head_endpoint_governed": True,
            "ingress_receiver_policy_governed": True,
            "replay_namespace_governed": True,
            "fixed_retention_contract_bound": True,
            "receiver_policy_digest_hex": DIGEST,
            "replay_namespace_digest_hex": DIGEST_2,
            "replica_set_digest_hex": DIGEST_3,
            "kubo_ingress_binding_digest_hex": DIGEST_4,
            "signed_head_ingress_binding_digest_hex": DIGEST_5,
            "retention_max_entries": MODULE.MIRROR_RETENTION_MAX_ENTRIES,
            "retention_max_bytes": MODULE.MIRROR_RETENTION_MAX_BYTES,
            "config_source": "iroha_config",
            "policy_digest_hex": DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "ingest-service.json", ingest_service())
    write_json(root / "publisher-service.json", publisher_service())
    write_json(root / "mirror-datastore.json", mirror_datastore())
    write_json(root / "operator-recovery.json", operator_recovery())
    write_json(root / "dashboard-api.json", dashboard_api())
    write_json(root / "observability.json", observability())
    write_json(root / "publication-e2e.json", publication_e2e())
    write_json(root / "governance-approval.json", governance_approval())


PUBLIC_HEAD_BOUND_FIXTURES = (
    ("mirror_datastore", "mirror-datastore.json", mirror_datastore),
    ("operator_recovery", "operator-recovery.json", operator_recovery),
    ("dashboard_api", "dashboard-api.json", dashboard_api),
    ("observability", "observability.json", observability),
    ("publication_e2e", "publication-e2e.json", publication_e2e),
    ("governance_approval", "governance-approval.json", governance_approval),
)

POLICY_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)


def run_gate(root: Path, *extra: str) -> int:
    return CHECKER(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.governance_dag.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["publisher_service"]["valid"] is True
    assert payload["valid_checkpoint_digests"] == [DIGEST]
    assert payload["valid_public_head_cids"] == [DIGEST]
    assert payload["valid_policy_digests"] == [DIGEST]
    assert (
        payload["thresholds"]["min_payload_kinds"]
        == MODULE.DEFAULT_MIN_PAYLOAD_KINDS
    )
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    observability_artifact = payload["required"]["observability"]["artifacts"][0]
    assert observability_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert observability_artifact["fingerprint"]["metrics"] == list(
        MODULE.REQUIRED_METRICS
    )


def test_evidence_payloads_are_schema_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["ipns_head_publication_enabled"] = True
    write_json(tmp_path / "publisher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = result["required"]["publisher_service"]["artifacts"][0]["errors"]
    assert (
        "publisher_service payload contains unknown fields: "
        "ipns_head_publication_enabled"
    ) in errors


def test_dashboard_route_rows_are_schema_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = dashboard_api()
    payload["routes"][0]["cached_body"] = True
    write_json(tmp_path / "dashboard-api.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = result["required"]["dashboard_api"]["artifacts"][0]["errors"]
    assert "routes[0] contains unknown fields: cached_body" in errors


def test_publisher_requires_fixed_unixfs_and_signed_http_contract(
    tmp_path: Path,
) -> None:
    cases = (
        ("kubo_unixfs_profile", "legacy-kubo-default"),
        ("unixfs_chunk_size_bytes", MODULE.KUBO_UNIXFS_CHUNK_SIZE_BYTES // 2),
        ("unixfs_max_links_per_node", MODULE.KUBO_UNIXFS_MAX_LINKS_PER_NODE - 1),
        ("cid_version", 0),
        ("cid_multihash", "blake3-256"),
        ("unixfs_raw_leaves", False),
        ("unixfs_balanced_layout", False),
        ("locally_derived_cids_verified", False),
        ("signed_http_head_cas_enabled", False),
        ("strong_single_etag_verified", False),
        ("conditional_cas_readback_verified", False),
    )
    for field, invalid in cases:
        case_dir = tmp_path / field
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = publisher_service()
        payload[field] = invalid
        write_json(case_dir / "publisher-service.json", payload)

        assert run_gate(case_dir) == 1


def test_publisher_requires_qualified_exclusive_ingress_and_shared_replay(
    tmp_path: Path,
) -> None:
    cases = (
        ("authenticated_ingress_qualified", False),
        ("ingress_enforcement", "shared_receiver"),
        ("replay_posture", "process_local_cache"),
        ("ingress_scope_binding_verified", False),
        ("receiver_policy_digest_hex", "not-a-digest"),
        ("replay_namespace_digest_hex", "not-a-digest"),
        ("replica_set_digest_hex", "not-a-digest"),
        ("kubo_ingress_binding_digest_hex", "not-a-digest"),
        ("signed_head_ingress_binding_digest_hex", "not-a-digest"),
    )
    for field, invalid in cases:
        case_dir = tmp_path / field
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = publisher_service()
        payload[field] = invalid
        write_json(case_dir / "publisher-service.json", payload)

        assert run_gate(case_dir) == 1


def test_governance_approval_must_bind_publisher_ingress_qualification(
    tmp_path: Path,
) -> None:
    for field in (
        "receiver_policy_digest_hex",
        "replay_namespace_digest_hex",
        "replica_set_digest_hex",
        "kubo_ingress_binding_digest_hex",
        "signed_head_ingress_binding_digest_hex",
    ):
        case_dir = tmp_path / field
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = governance_approval()
        payload[field] = "56" * 32
        write_json(case_dir / "governance-approval.json", payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        errors = result["required"]["governance_approval"]["artifacts"][0][
            "errors"
        ]
        assert (
            f"governance_approval {field} must match a valid "
            f"publisher_service {field}"
        ) in errors


def test_governance_only_gate_requires_publisher_policy_and_ingress_anchors(
    tmp_path: Path,
) -> None:
    write_json(tmp_path / "governance-approval.json", governance_approval())
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-kind",
            "governance_approval",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_approval"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "governance_approval policy_digest_hex requires a valid "
        "publisher_service policy_digest_hex"
    ) in artifact["errors"]
    for field in (
        "receiver_policy_digest_hex",
        "replay_namespace_digest_hex",
        "replica_set_digest_hex",
        "kubo_ingress_binding_digest_hex",
        "signed_head_ingress_binding_digest_hex",
    ):
        assert (
            f"governance_approval {field} requires a valid "
            f"publisher_service {field}"
        ) in artifact["errors"]


def test_optional_bound_artifact_cannot_escape_missing_publisher_anchor(
    tmp_path: Path,
) -> None:
    write_json(tmp_path / "ingest-service.json", ingest_service())
    write_json(tmp_path / "governance-approval.json", governance_approval())
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-kind",
            "ingest_service",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    approval = next(
        artifact
        for artifact in result["recognized_artifacts"]
        if artifact["kind"] == "governance_approval"
    )
    assert approval["valid"] is False
    assert (
        "governance_approval public_head_cid_hex requires a valid "
        "publisher_service public_head_cid_hex"
    ) in approval["errors"]


def test_publisher_security_identity_digests_reject_zero(tmp_path: Path) -> None:
    for field in (
        "policy_digest_hex",
        "receiver_policy_digest_hex",
        "replay_namespace_digest_hex",
        "replica_set_digest_hex",
        "kubo_ingress_binding_digest_hex",
        "signed_head_ingress_binding_digest_hex",
    ):
        case_dir = tmp_path / field
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = publisher_service()
        payload[field] = "0" * 64
        write_json(case_dir / "publisher-service.json", payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1
        result = json.loads(summary.read_text(encoding="utf-8"))
        errors = result["required"]["publisher_service"]["artifacts"][0][
            "errors"
        ]
        assert f"{field} must not be the zero digest" in errors


def test_fixed_retention_contract_is_exact_across_mirror_and_approval(
    tmp_path: Path,
) -> None:
    cases = (
        (
            "mirror_datastore",
            "mirror-datastore.json",
            mirror_datastore,
            "retention_max_entries",
            MODULE.MIRROR_RETENTION_MAX_ENTRIES - 1,
        ),
        (
            "mirror_datastore",
            "mirror-datastore.json",
            mirror_datastore,
            "retention_max_bytes",
            MODULE.MIRROR_RETENTION_MAX_BYTES + 1,
        ),
        (
            "governance_approval",
            "governance-approval.json",
            governance_approval,
            "retention_max_entries",
            MODULE.MIRROR_RETENTION_MAX_ENTRIES + 1,
        ),
        (
            "governance_approval",
            "governance-approval.json",
            governance_approval,
            "retention_max_bytes",
            MODULE.MIRROR_RETENTION_MAX_BYTES - 1,
        ),
    )
    for kind, filename, factory, field, invalid in cases:
        case_dir = tmp_path / f"{kind}-{field}"
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload[field] = invalid
        write_json(case_dir / filename, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        errors = result["required"][kind]["artifacts"][0]["errors"]
        assert any(error.startswith(f"{field} must equal") for error in errors)


def test_rollout_requires_repair_audit_and_live_fresh_read_guarantees(
    tmp_path: Path,
) -> None:
    cases = (
        (
            "operator_recovery",
            "operator-recovery.json",
            operator_recovery,
            "post_loss_repair_verified",
        ),
        (
            "operator_recovery",
            "operator-recovery.json",
            operator_recovery,
            "head_object_repaired_with_same_cid",
        ),
        (
            "mirror_datastore",
            "mirror-datastore.json",
            mirror_datastore,
            "exact_retained_source_suffix_verified",
        ),
        (
            "mirror_datastore",
            "mirror-datastore.json",
            mirror_datastore,
            "fresh_checkpoint_coherent_reads_verified",
        ),
        (
            "dashboard_api",
            "dashboard-api.json",
            dashboard_api,
            "reader_withdrawal_verified",
        ),
        (
            "publication_e2e",
            "publication-e2e.json",
            publication_e2e,
            "stopped_service_reads_rejected",
        ),
    )
    for kind, filename, factory, field in cases:
        case_dir = tmp_path / f"{kind}-{field}"
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload[field] = False
        write_json(case_dir / filename, payload)

        assert run_gate(case_dir) == 1


def test_rotating_audit_budgets_are_fixed_v1_values(tmp_path: Path) -> None:
    for field, invalid in (
        ("audit_max_entries_per_poll", MODULE.STEADY_AUDIT_MAX_ENTRIES_PER_POLL + 1),
        ("audit_max_bytes_per_poll", MODULE.STEADY_AUDIT_MAX_BYTES_PER_POLL - 1),
    ):
        case_dir = tmp_path / field
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = observability()
        payload[field] = invalid
        write_json(case_dir / "observability.json", payload)

        assert run_gate(case_dir) == 1


def test_retired_ipns_publication_schema_is_not_recognized(tmp_path: Path) -> None:
    path = write_json(
        tmp_path / "retired.json",
        base("sorafs.governance_dag.ipfs_ipns_e2e_canary.v1"),
    )

    assert CHECKER(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in PUBLIC_HEAD_BOUND_FIXTURES)
        == MODULE.PUBLIC_HEAD_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in POLICY_BOUND_FIXTURES)
        == MODULE.POLICY_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert set(payload_kinds()) == MODULE.REQUIRED_PAYLOAD_KIND_SET
    assert tuple(route["name"] for route in dashboard_api()["routes"]) == (
        MODULE.REQUIRED_DASHBOARD_ROUTES
    )


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "governance-dag.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert CHECKER([f"@{args}"]) == 0


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "ingest-service.json",
            "ingest_service",
            ingest_service,
            ("payload_bytes_included",),
        ),
        (
            "publisher-service.json",
            "publisher_service",
            publisher_service,
            ("raw_head_included",),
        ),
        (
            "mirror-datastore.json",
            "mirror_datastore",
            mirror_datastore,
            ("mirror_drift_detected", "raw_blocks_included"),
        ),
        (
            "operator-recovery.json",
            "operator_recovery",
            operator_recovery,
            ("raw_checkpoint_included",),
        ),
        (
            "dashboard-api.json",
            "dashboard_api",
            dashboard_api,
            ("response_bodies_included",),
        ),
        (
            "observability.json",
            "observability",
            observability,
            ("critical_alerts_firing", "response_bodies_included"),
        ),
        (
            "publication-e2e.json",
            "publication_e2e",
            publication_e2e,
            ("raw_blocks_included",),
        ),
    )

    for artifact_file, kind, make_payload, fields in cases:
        for field in fields:
            case_dir = tmp_path / kind / field
            case_dir.mkdir(parents=True)
            write_complete_evidence(case_dir)
            payload = make_payload()
            payload.pop(field)
            write_json(case_dir / artifact_file, payload)
            summary = case_dir / "summary.json"

            assert run_gate(case_dir, "--summary-out", str(summary)) == 1

            result = json.loads(summary.read_text(encoding="utf-8"))
            artifact = result["required"][kind]["artifacts"][0]
            assert artifact["valid"] is False
            assert f"{field} must be false" in artifact["errors"]


def test_missing_publisher_service_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "publisher-service.json").unlink()

    assert run_gate(tmp_path) == 1


def test_dashboard_requires_public_head_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = dashboard_api()
    payload.pop("public_head_cid_hex")
    write_json(tmp_path / "dashboard-api.json", payload)

    assert run_gate(tmp_path) == 1


def test_publication_e2e_public_head_binding_must_match_publisher(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = publication_e2e()
    payload["public_head_cid_hex"] = DIGEST_2
    write_json(tmp_path / "publication-e2e.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["publication_e2e"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "publication_e2e public_head_cid_hex must match a valid "
        "publisher_service public_head_cid_hex"
    ]


def test_all_public_head_bound_artifacts_reject_publisher_head_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in PUBLIC_HEAD_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["public_head_cid_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} public_head_cid_hex must match a valid "
            "publisher_service public_head_cid_hex"
        ) in artifact["errors"]


def test_publisher_policy_digest_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload.pop("policy_digest_hex")
    write_json(tmp_path / "publisher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publisher_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]
    assert payload["valid_policy_digests"] == []


def test_operator_recovery_checkpoint_digest_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_recovery()
    payload.pop("checkpoint_digest_hex")
    write_json(tmp_path / "operator-recovery.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_recovery"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "checkpoint_digest_hex must be a non-empty string" in artifact["errors"]
    assert payload["valid_checkpoint_digests"] == []


def test_governance_approval_policy_digest_must_match_publisher(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert payload["valid_policy_digests"] == [DIGEST]
    assert (
        "governance_approval policy_digest_hex must match a valid "
        "publisher_service policy_digest_hex"
    ) in artifact["errors"]


def test_all_policy_bound_artifacts_reject_publisher_policy_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in POLICY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["policy_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_policy_digests"] == [DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} policy_digest_hex must match a valid "
            "publisher_service policy_digest_hex"
        ) in artifact["errors"]


def test_multiple_valid_public_head_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["public_head_cid_hex"] = DIGEST_3
    write_json(tmp_path / "publisher-service-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_public_head_cids"] == []
    assert (
        "valid_public_head_cids must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_policy_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["policy_digest_hex"] = DIGEST_3
    write_json(tmp_path / "publisher-service-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_policy_digests"] == []
    assert (
        "valid_policy_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_checkpoint_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_recovery()
    payload["checkpoint_digest_hex"] = DIGEST_3
    write_json(tmp_path / "operator-recovery-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_checkpoint_digests"] == []
    assert (
        "valid_checkpoint_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_stale_publisher_head_does_not_anchor_bound_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = publisher_service()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "publisher-service.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["mirror_datastore"]
    artifact = required["artifacts"][0]
    assert payload["valid_public_head_cids"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "mirror_datastore public_head_cid_hex requires a valid "
        "publisher_service public_head_cid_hex"
    ]


def test_stale_ingest_service_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ingest_service()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "ingest-service.json", payload)

    assert run_gate(tmp_path) == 1


def test_ingest_source_count_must_match_unique_payload_kinds(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ingest_service()
    payload["source_count"] += 1
    write_json(tmp_path / "ingest-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["ingest_service"]["artifacts"][0]
    assert "source_count must match unique payload_kinds count" in artifact["errors"]


def test_ingest_payload_kinds_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ingest_service()
    payload["payload_kinds"].append(payload["payload_kinds"][0])
    payload["source_count"] = len(payload["payload_kinds"])
    write_json(tmp_path / "ingest-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["ingest_service"]["artifacts"][0]
    assert "payload_kinds must not contain duplicate values" in artifact["errors"]
    assert "source_count must match unique payload_kinds count" in artifact["errors"]


def test_ingest_payload_kinds_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = ingest_service()
    payload["payload_kinds"].append("unknown-governance-payload")
    payload["source_count"] = len(payload["payload_kinds"])
    write_json(tmp_path / "ingest-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["ingest_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "payload_kinds must not include unknown values" in artifact["errors"]


def test_publisher_block_count_must_match_unique_block_refs(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["block_count"] += 1
    write_json(tmp_path / "publisher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publisher_service"]["artifacts"][0]
    assert "block_count must match unique block_refs count" in artifact["errors"]


def test_publisher_block_refs_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["block_refs"].append(payload["block_refs"][0])
    payload["block_count"] = len(payload["block_refs"])
    write_json(tmp_path / "publisher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publisher_service"]["artifacts"][0]
    assert "block_refs must not contain duplicate values" in artifact["errors"]
    assert "block_count must match unique block_refs count" in artifact["errors"]


def test_publisher_block_refs_must_use_production_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["block_refs"][0] = "governance-block-00"
    write_json(tmp_path / "publisher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publisher_service"]["artifacts"][0]
    assert MODULE.BLOCK_REF_LABEL_ERROR in artifact["errors"]


def test_publisher_block_refs_reject_placeholder_marker(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["block_refs"][0] = "governance-dag-block-placeholder"
    write_json(tmp_path / "publisher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publisher_service"]["artifacts"][0]
    assert (
        "block_refs[0] must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_publisher_payload_kind_count_must_match_inventory(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["payload_kind_count"] += 1
    write_json(tmp_path / "publisher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publisher_service"]["artifacts"][0]
    assert "payload_kind_count must match unique payload_kinds count" in artifact[
        "errors"
    ]


def test_publisher_payload_kinds_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["payload_kinds"].append("unknown-governance-payload")
    payload["payload_kind_count"] = len(payload["payload_kinds"])
    write_json(tmp_path / "publisher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publisher_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "payload_kinds must not include unknown values" in artifact["errors"]


def test_publication_block_refs_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_e2e()
    payload["block_refs"].append(payload["block_refs"][0])
    payload["block_count"] = len(payload["block_refs"])
    write_json(tmp_path / "publication-e2e.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publication_e2e"]["artifacts"][0]
    assert "block_refs must not contain duplicate values" in artifact["errors"]
    assert "block_count must match unique block_refs count" in artifact["errors"]


def test_publication_block_refs_must_use_production_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_e2e()
    payload["block_refs"][0] = "governance-block-00"
    write_json(tmp_path / "publication-e2e.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publication_e2e"]["artifacts"][0]
    assert MODULE.BLOCK_REF_LABEL_ERROR in artifact["errors"]


def test_publication_payload_kinds_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_e2e()
    payload["payload_kinds"].append(payload["payload_kinds"][0])
    payload["payload_kind_count"] = len(payload["payload_kinds"])
    write_json(tmp_path / "publication-e2e.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publication_e2e"]["artifacts"][0]
    assert "payload_kinds must not contain duplicate values" in artifact["errors"]
    assert "payload_kind_count must match unique payload_kinds count" in artifact[
        "errors"
    ]


def test_publication_payload_kinds_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_e2e()
    payload["payload_kinds"].append("unknown-governance-payload")
    payload["payload_kind_count"] = len(payload["payload_kinds"])
    write_json(tmp_path / "publication-e2e.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publication_e2e"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "payload_kinds must not include unknown values" in artifact["errors"]


def test_payload_kinds_reject_trim_normalized_and_unicode_variants(
    tmp_path: Path,
) -> None:
    cases = (
        ("ingest_service", "ingest-service.json", ingest_service, "source_count"),
        (
            "publisher_service",
            "publisher-service.json",
            publisher_service,
            "payload_kind_count",
        ),
        ("publication_e2e", "publication-e2e.json", publication_e2e, "payload_kind_count"),
    )
    suffixes = (" ", "\u200d", "\u202e")

    for artifact_kind, filename, factory, count_field in cases:
        for suffix_index, suffix in enumerate(suffixes):
            root = tmp_path / f"{artifact_kind}-{suffix_index}"
            root.mkdir()
            write_complete_evidence(root)
            payload = factory()
            bad_value = payload["payload_kinds"][0] + suffix
            payload["payload_kinds"].append(bad_value)
            payload[count_field] = len(payload["payload_kinds"])
            write_json(root / filename, payload)
            summary = root / "summary.json"

            assert run_gate(root, "--summary-out", str(summary)) == 1

            summary_payload = json.loads(summary.read_text(encoding="utf-8"))
            artifact = summary_payload["required"][artifact_kind]["artifacts"][0]
            errors = artifact["errors"]
            rendered_errors = json.dumps(errors, ensure_ascii=True)
            escaped_value = bad_value.encode("unicode_escape").decode("ascii")
            assert artifact["valid"] is False
            assert "payload_kinds must not include unknown values" in errors
            assert bad_value not in rendered_errors
            assert escaped_value not in rendered_errors


def test_raw_payload_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["raw_block"] = {"cid": "leaked"}
    write_json(tmp_path / "publisher-service.json", payload)

    assert run_gate(tmp_path) == 1


def test_public_head_age_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "publisher-service.json", publisher_service(head_age=10_000))

    assert run_gate(tmp_path) == 1


def test_dashboard_route_failure_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "dashboard-api.json", dashboard_api(passed=False))

    assert run_gate(tmp_path) == 1


def test_dashboard_route_latency_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "dashboard-api.json", dashboard_api(latency_ms=10_000))

    assert run_gate(tmp_path) == 1


def test_rollout_timing_evidence_must_be_integer_units(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    publisher = publisher_service()
    publisher["pin_lag_seconds"] = 12.5
    publisher["head_age_seconds"] = 120.5
    write_json(tmp_path / "publisher-service.json", publisher)
    dashboard = dashboard_api()
    dashboard["routes"][0]["latency_ms"] = 12.5
    write_json(tmp_path / "dashboard-api.json", dashboard)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    publisher_errors = payload["required"]["publisher_service"]["artifacts"][0][
        "errors"
    ]
    dashboard_errors = payload["required"]["dashboard_api"]["artifacts"][0]["errors"]
    assert "pin_lag_seconds must be a non-negative integer" in publisher_errors
    assert "head_age_seconds must be a non-negative integer" in publisher_errors
    assert "routes[0].latency_ms must be a non-negative integer" in dashboard_errors


def test_dashboard_route_count_must_match_unique_routes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = dashboard_api()
    payload["route_count"] += 1
    payload["passed_route_count"] = payload["route_count"]
    write_json(tmp_path / "dashboard-api.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["dashboard_api"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_dashboard_routes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = dashboard_api()
    payload["routes"].append(dict(payload["routes"][0]))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "dashboard-api.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["dashboard_api"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_dashboard_routes_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = dashboard_api()
    unknown = dict(payload["routes"][0])
    unknown["name"] = "shadow_dashboard_route"
    payload["routes"].append(unknown)
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "dashboard-api.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["dashboard_api"]["artifacts"][0]
    assert "routes must not include unknown values" in artifact["errors"]


def test_dashboard_route_body_hash_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = dashboard_api()
    del payload["routes"][0]["body_blake3_hex"]
    write_json(tmp_path / "dashboard-api.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["dashboard_api"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[0].body_blake3_hex must be a non-empty string"
        in artifact["errors"]
    )


def test_mirror_drift_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "mirror-datastore.json", mirror_datastore(drift=True))

    assert run_gate(tmp_path) == 1


def test_publication_e2e_requires_minimum_blocks(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "publication-e2e.json", publication_e2e(block_count=2))

    assert run_gate(tmp_path) == 1


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_observability_metrics_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = observability()
    payload["metrics"].append(payload["metrics"][0])
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert "metrics must not contain duplicate values" in artifact["errors"]
    assert "metric_count must match unique metrics count" in artifact["errors"]


def test_observability_metrics_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = observability()
    payload["metrics"].append("sorafs_governance_dag_shadow_metric")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["observability"]["artifacts"][0]
    assert "metrics must not include unknown values" in artifact["errors"]


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.governance_dag.unknown.v1"})

    assert CHECKER(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "ingest-service.json", ingest_service())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.governance_dag.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "ingest_service") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "ingest-service.json", ingest_service())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "ingest_service") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert CHECKER(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
