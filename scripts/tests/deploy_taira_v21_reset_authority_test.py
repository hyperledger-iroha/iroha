"""Deploy-authority execution tests for the Taira reset controller."""

from __future__ import annotations

import argparse
import contextlib
import hashlib
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts.tests.deploy_taira_v21_reset_test_support import (
    DPN_VALIDATOR_RELEASE_COMMIT,
    MODULE,
)


def _kagemusha_bundle(
    *, configured: bool, material_present: bool
) -> SimpleNamespace:
    external = SimpleNamespace(
        bounded_material_present=material_present,
        expected_policy_sha256="2" * 64,
        manifest_directory_digests=("3" * 64, "4" * 64),
        manifest_directory_inventory_sha256="5" * 64,
        qualification_seal_sha256="6" * 64,
    )
    return SimpleNamespace(
        kagemusha_config_projection_sha256=("1" * 64 if configured else None),
        kagemusha_external_release=external if configured else None,
    )


@pytest.mark.parametrize(
    ("configured", "material_present", "exact", "status"),
    (
        (False, False, False, "not-configured"),
        (True, False, False, "blocked-external-release-unavailable"),
        (True, True, False, "blocked-exact-installed-binary-config-pending"),
        (True, True, True, "exact-installed-binary-config-verified"),
    ),
)
def test_authority_helper_reports_one_exact_kagemusha_state(
    configured: bool,
    material_present: bool,
    exact: bool,
    status: str,
) -> None:
    fields = MODULE._DEPLOY_AUTHORITY.report_fields(
        _kagemusha_bundle(
            configured=configured,
            material_present=material_present,
        ),
        exact_binary_config_verified=exact,
    )

    assert fields["kagemusha_external_release_status"] == status
    assert fields["kagemusha_external_release_material_present"] is (
        configured and material_present
    )
    assert fields["kagemusha_exact_binary_config_verified"] is (
        configured and material_present and exact
    )
    assert fields["kagemusha_external_release_verified"] is (
        configured and material_present and exact
    )


def test_authority_subject_propagates_the_complete_exact_projection() -> None:
    bundle = _kagemusha_bundle(configured=True, material_present=True)
    bundle.manifest_sha256 = "7" * 64
    bundle.peers = (
        SimpleNamespace(slug="val0", config_sha256="8" * 64),
        SimpleNamespace(slug="val1", config_sha256="9" * 64),
    )
    admission = SimpleNamespace(
        archive_sha256="a" * 64,
        artifact_handoff_sha256="b" * 64,
        boi_qualified_inventory_sha256="c" * 64,
        receipt_id="d" * 64,
        release_manifest_sha256="e" * 64,
        reset_manifest_sha256="f" * 64,
        cargo_lock_sha256="0" * 64,
        source_commit="1" * 40,
        dpn_validator_release_commit="2" * 40,
        workspace_source_manifest_sha256="3" * 64,
        restart_generation="4" * 64,
    )
    sources = SimpleNamespace(binary_sha256="5" * 64, supervisor_sha256="6" * 64)

    subject = MODULE._DEPLOY_AUTHORITY.subject(
        admission,
        bundle,
        sources,
        exact_binary_config_verified=True,
    )

    assert subject["bundle"]["kagemusha"] == {
        "configured": True,
        "config_projection_sha256": "1" * 64,
        "bounded_material_present": True,
        "exact_binary_config_verified": True,
        "external_release_verified": True,
        "manifest_directory_digests": ["3" * 64, "4" * 64],
        "manifest_directory_inventory_sha256": "5" * 64,
        "policy_sha256": "2" * 64,
        "qualification_seal_sha256": "6" * 64,
    }
    assert subject["bundle"]["peer_config_sha256"] == {
        "val0": "8" * 64,
        "val1": "9" * 64,
    }


def test_controller_projections_are_owned_by_the_single_authority_helper() -> None:
    for projection in (
        MODULE._kagemusha_authority_subject,
        MODULE._kagemusha_authority_artifacts,
        MODULE._kagemusha_report_fields,
        MODULE._deploy_authority_subject,
        MODULE._deploy_authority_artifacts,
        MODULE._deploy_result_sha256,
    ):
        assert projection.__self__ is MODULE._DEPLOY_AUTHORITY

def test_dry_run_execute_never_calls_apply(monkeypatch: pytest.MonkeyPatch) -> None:
    events: list[str] = []
    admission = SimpleNamespace(
        archive_sha256="0" * 64,
        boi_artifact_inventory_sha256="2" * 64,
        boi_qualified_inventory_sha256="3" * 64,
        boi_qualification_receipt_id="4" * 64,
        receipt_id="f" * 64,
        reset_manifest_sha256="1" * 64,
        binary_sha256="a" * 64,
        supervisor_sha256="b" * 64,
        source_commit="c" * 40,
        dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        restart_generation="9" * 64,
    )
    bundle = SimpleNamespace(
        root=Path("/bundle"),
        bundle_bytes=1,
        free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        fsync_latency_ms=1.0,
    )
    sources = SimpleNamespace(
        binary_sha256="a" * 64,
        supervisor_sha256="b" * 64,
    )
    cohort = tuple(
        SimpleNamespace(
            path=Path(f"/Library/LaunchDaemons/{label}.plist"),
            managed=SimpleNamespace(child_was_present=True),
        )
        for label in MODULE.LABELS
    )
    monkeypatch.setattr(MODULE, "validate_bundle", lambda *args, **kwargs: bundle)
    monkeypatch.setattr(MODULE, "validate_sources", lambda *args, **kwargs: sources)
    monkeypatch.setattr(
        MODULE,
        "verify_deployment_admission",
        lambda _args: events.append("admission-verify") or admission,
    )
    monkeypatch.setattr(MODULE, "require_inputs_match_admission", lambda *args: None)
    monkeypatch.setattr(
        MODULE,
        "require_mutable_bundle_identities",
        lambda *_args, phase: events.append(f"bundle-recheck:{phase}"),
    )
    monkeypatch.setattr(
        MODULE,
        "require_admission_archive_unchanged",
        lambda _admission: events.append("archive-recheck"),
    )
    monkeypatch.setattr(
        MODULE,
        "consume_admission_receipt",
        lambda *_args: pytest.fail("dry run consumed an admission receipt"),
    )
    monkeypatch.setattr(
        MODULE,
        "capture_old_cohort",
        lambda _ops, *, allow_absent_child: events.append("capture") or cohort,
    )
    monkeypatch.setattr(
        MODULE,
        "apply_reset",
        lambda *args, **kwargs: pytest.fail("dry run called apply_reset"),
    )
    dry_run_authority = MODULE.taira_authority_client.AuthorityResult(
        role="deploy-issuance",
        operation_id="7" * 64,
        run_id="8" * 64,
        status="verified",
        authority_envelope={},
        durable_receipt={},
    )
    monkeypatch.setattr(
        MODULE,
        "_authorize_deploy_lease",
        lambda *_args, apply, **_kwargs: (
            events.append(f"authority:{apply}") or dry_run_authority
        ),
    )
    monkeypatch.setattr(
        MODULE.taira_authority_client,
        "verify_receipt",
        lambda *_args, **_kwargs: pytest.fail("dry run historically verified a lease"),
    )
    monkeypatch.setattr(
        MODULE,
        "_finalize_deploy_lease",
        lambda *_args, **_kwargs: pytest.fail("dry run finalized a lease"),
    )
    monkeypatch.setattr(
        MODULE,
        "exclusive_deployment_lock",
        lambda: pytest.fail("dry run acquired the deployment lock"),
    )
    args = argparse.Namespace(
        bundle=Path("/bundle"),
        binary=Path("/binary"),
        supervisor=Path("/supervisor"),
        admission_archive=Path("/candidate.tar.gz"),
        admission_authority_dir=Path("/authority"),
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
        expected_source_commit="c" * 40,
        expected_dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        expected_cargo_lock_sha256="d" * 64,
        expected_workspace_source_manifest_sha256="e" * 64,
        expected_receipt_id="f" * 64,
        expected_artifact_handoff_sha256="9" * 64,
        expected_production_reset_manifest_sha256="a" * 64,
        trusted_signing_fingerprint="1" * 64,
        trusted_boi_qualification_signing_fingerprint="3" * 64,
        release_manifest_verifier=Path("/sorafs-validate"),
        trusted_release_manifest_verifier_sha256="2" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        allow_absent_old_child=False,
        apply=False,
    )

    report = MODULE._execute_after_provisioned_authority_contracts(
        args, ops=MODULE.SystemOps()
    )
    assert report["mode"] == "verified-read-only-dry-run"
    assert report["applied"] is False
    assert report["admission_receipt_consumed"] is False
    assert report["boi_artifact_inventory_sha256"] == "2" * 64
    assert report["boi_qualified_inventory_sha256"] == "3" * 64
    assert report["boi_qualification_receipt_id"] == "4" * 64
    assert events == [
        "admission-verify",
        "capture",
        "archive-recheck",
        "bundle-recheck:immediately before dry-run authority",
        "authority:False",
        "bundle-recheck:immediately after dry-run authority",
    ]


def test_apply_lock_spans_old_cohort_capture_and_rollout(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    events: list[str] = []
    admission = SimpleNamespace(
        archive_sha256="0" * 64,
        boi_artifact_inventory_sha256="2" * 64,
        boi_qualified_inventory_sha256="3" * 64,
        boi_qualification_receipt_id="4" * 64,
        receipt_id="f" * 64,
        reset_manifest_sha256="1" * 64,
        binary_sha256="a" * 64,
        supervisor_sha256="b" * 64,
        source_commit="c" * 40,
        dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        restart_generation="9" * 64,
    )
    bundle = SimpleNamespace()
    sources = SimpleNamespace()
    cohort = tuple(object() for _ in range(MODULE.PEER_COUNT))
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_UID_ENV, "41")
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_GID_ENV, "42")
    monkeypatch.setattr(MODULE, "validate_bundle", lambda *args, **kwargs: bundle)
    monkeypatch.setattr(MODULE, "validate_sources", lambda *args, **kwargs: sources)
    monkeypatch.setattr(
        MODULE,
        "require_authenticated_lifecycle_node_ids",
        lambda _bundle: {},
    )
    monkeypatch.setattr(
        MODULE,
        "verify_deployment_admission",
        lambda _args: events.append("admission-verify") or admission,
    )
    monkeypatch.setattr(
        MODULE,
        "require_inputs_match_admission",
        lambda *args: events.append("bind-inputs"),
    )
    monkeypatch.setattr(
        MODULE,
        "require_admission_bound_inputs_unchanged",
        lambda *args: events.append("recheck-inputs"),
    )
    monkeypatch.setattr(
        MODULE,
        "require_admission_archive_unchanged",
        lambda *_args: events.append("recheck-admission-evidence"),
    )
    monkeypatch.setattr(
        MODULE,
        "capture_old_cohort",
        lambda _ops, *, allow_absent_child: (
            events.append(f"capture:{allow_absent_child}") or cohort
        ),
    )

    def apply(*_args, **kwargs):
        events.append("apply")
        kwargs["rollout_starter"]()
        return {"applied": True}

    monkeypatch.setattr(MODULE, "apply_reset", apply)

    consumed_lease = MODULE.taira_authority_client.AuthorityResult(
        role="deploy-issuance",
        operation_id="7" * 64,
        run_id="8" * 64,
        status="authorized",
        authority_envelope={"schema": "test-deploy-envelope"},
        durable_receipt={"schema": "test-deploy-receipt"},
    )
    finalization = MODULE.taira_authority_client.AuthorityResult(
        role="deploy-issuance",
        operation_id="7" * 64,
        run_id="8" * 64,
        status="finalized",
        authority_envelope={"schema": "test-final-envelope"},
        durable_receipt={"schema": "test-final-receipt"},
    )
    monkeypatch.setattr(
        MODULE,
        "_authorize_deploy_lease",
        lambda *_args, apply, **_kwargs: (
            events.append(f"authority:{apply}") or consumed_lease
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "_finalize_deploy_lease",
        lambda *_args, outcome, **_kwargs: (
            events.append(f"finalize:{outcome}") or finalization
        ),
    )

    @contextlib.contextmanager
    def consume(_admission):
        events.append("consume-enter")
        transaction = SimpleNamespace(
            mark_rollout_started=lambda: events.append("rollout-start")
        )
        try:
            yield transaction
        finally:
            events.append("consume-exit")

    @contextlib.contextmanager
    def lock():
        events.append("lock-enter")
        try:
            yield
        finally:
            events.append("lock-exit")

    monkeypatch.setattr(MODULE, "exclusive_deployment_lock", lock)
    monkeypatch.setattr(MODULE, "consume_admission_receipt", consume)
    monkeypatch.setattr(MODULE, "build_operator_http_getter", lambda *_args: object())
    args = argparse.Namespace(
        bundle=Path("/bundle"),
        binary=Path("/binary"),
        supervisor=Path("/supervisor"),
        admission_archive=Path("/candidate.tar.gz"),
        admission_authority_dir=Path("/authority"),
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
        expected_source_commit="c" * 40,
        expected_dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        expected_cargo_lock_sha256="d" * 64,
        expected_workspace_source_manifest_sha256="e" * 64,
        expected_receipt_id="f" * 64,
        expected_artifact_handoff_sha256="9" * 64,
        expected_production_reset_manifest_sha256="a" * 64,
        trusted_signing_fingerprint="1" * 64,
        trusted_boi_qualification_signing_fingerprint="3" * 64,
        release_manifest_verifier=Path("/sorafs-validate"),
        trusted_release_manifest_verifier_sha256="2" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        allow_absent_old_child=True,
        operator_network_id="taira", operator_private_key_file=Path("/operator.key"),
        apply=True,
    )

    assert MODULE._execute_after_provisioned_authority_contracts(args, ops=MODULE.SystemOps()) == {
        "admission_archive_sha256": "0" * 64,
        "admission_receipt_consumed": True,
        "admission_receipt_id": "f" * 64,
        "applied": True,
        "boi_artifact_inventory_sha256": "2" * 64,
        "boi_qualified_inventory_sha256": "3" * 64,
        "boi_qualification_receipt_id": "4" * 64,
        "deploy_authority_final_status": "finalized",
        "deploy_authority_operation_id": "7" * 64,
        "deploy_authority_result_receipt_sha256": hashlib.sha256(
            finalization.durable_receipt_bytes
        ).hexdigest(),
        "deploy_authority_status": "authorized",
    }
    assert events == [
        "admission-verify",
        "bind-inputs",
        "lock-enter",
        "admission-verify",
        "recheck-admission-evidence",
        "recheck-inputs",
        "capture:True",
        "recheck-inputs",
        "recheck-admission-evidence",
        "authority:True",
        "consume-enter",
        "apply",
        "rollout-start",
        "consume-exit",
        "finalize:success",
        "lock-exit",
    ]
