"""Admission, receipt-ledger, and rollback components for reset tests."""

from __future__ import annotations

import argparse
import copy
import hashlib
import os
import stat
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts.tests.deploy_taira_v21_reset_test_support import (
    DPN_VALIDATOR_RELEASE_COMMIT,
    MODULE,
)
from scripts.tests.taira_receipt_signer_test_support import (
    receipt_signer_map as _receipt_signer_map,
)


def _write(path: Path, body: bytes) -> None:
    """Write one owner-private test fixture."""

    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(body)
    path.chmod(0o600)


def test_deployment_admission_requires_and_binds_qualified_boi_result(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive_state = SimpleNamespace(sha256="a" * 64)
    source = {
        "cargo_lock_sha256": "d" * 64,
        "commit": "c" * 40,
        "dpn_validator_release_commit": DPN_VALIDATOR_RELEASE_COMMIT,
        "workspace_source_manifest_sha256": "e" * 64,
    }
    result = {
        "artifact_handoff_sha256": "9" * 64,
        "archive_sha256": "a" * 64,
        "boi_artifact_inventory_sha256": "b" * 64,
        "deployment_performed": False,
        "linux_authority_manifest_sha256": "3" * 64,
        "macos_end_block_hash": "4" * 64,
        "macos_end_height": 42,
        "peer_count": MODULE.PEER_COUNT,
        "privacy_protocol_receipt_id": "5" * 64,
        "receipt_id": "f" * 64,
        "release_manifest_sha256": "6" * 64,
        "release_manifest_verifier_sha256": "2" * 64,
        "receipt_signers": _receipt_signer_map(),
        "reset_manifest_sha256": "7" * 64,
        "restart_generation": "8" * 64,
        "schema": MODULE.rollout_admission.VERIFICATION_SCHEMA,
        "schema_version": MODULE.rollout_admission.VERIFICATION_SCHEMA_VERSION,
        "signer_fingerprint_sha256": "1" * 64,
        "source": source,
        "supervisor_sha256": "0" * 64,
        "validator_binary_sha256": "a" * 64,
        "validator_config_sha256": {
            slug: f"{index}" * 64
            for index, slug in enumerate(MODULE.SLUGS, start=1)
        },
        "verified": True,
    }
    snapshot = SimpleNamespace(
        boi_inventory_sha256="b" * 64,
        candidate_archive_sha256="a" * 64,
        candidate_boi_artifact_inventory_sha256="b" * 64,
        candidate_release_manifest_sha256="6" * 64,
        qualification_receipt_id="7" * 64,
        source=source,
    )
    seen: list[Path] = []
    monkeypatch.setattr(MODULE, "canonical_path", lambda path, _label: path)
    monkeypatch.setattr(MODULE, "require_protected_replay_ledger", lambda _path: None)
    monkeypatch.setattr(MODULE, "_stable_admission_file", lambda *_args: archive_state)
    monkeypatch.setattr(
        MODULE.rollout_admission, "verify_admission", lambda **_kwargs: result
    )
    monkeypatch.setattr(
        MODULE.rollout_admission,
        "scan_inventory_paths",
        lambda _root: list(MODULE.rollout_admission.FINAL_AUTHORITY_FILES),
    )
    monkeypatch.setattr(
        MODULE.rollout_admission,
        "stable_hash_relative",
        lambda _root, relative: SimpleNamespace(sha256=relative, size=1),
    )

    def verify_boi(root: Path, **_kwargs):
        seen.append(root)
        return snapshot

    monkeypatch.setattr(
        MODULE.boi_handoff, "verify_qualified_boi_handoff", verify_boi
    )
    args = argparse.Namespace(
        admission_archive=Path("/candidate.tar.gz"),
        admission_authority_dir=Path("/authority"),
        boi_qualified_handoff_root=Path("/qualified-boi"),
        expected_source_commit="c" * 40,
        expected_dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        expected_cargo_lock_sha256="d" * 64,
        expected_workspace_source_manifest_sha256="e" * 64,
        expected_receipt_id="f" * 64,
        expected_artifact_handoff_sha256="9" * 64,
        trusted_signing_fingerprint="1" * 64,
        trusted_boi_qualification_public_key=Path("/qualification.pub"),
        trusted_boi_qualification_signing_fingerprint="3" * 64,
        expected_boi_qualification_host_id="boi-host-v1",
        expected_boi_qualification_installation_id="boi-installation-v1",
        expected_boi_qualification_controller_digest="4" * 64,
        expected_workflow_run_id=101,
        expected_workflow_run_attempt=2,
        release_manifest_verifier=Path("/verifier"),
        trusted_release_manifest_verifier_sha256="2" * 64,
    )

    plan = MODULE.verify_deployment_admission(args)

    assert seen == [Path("/qualified-boi")]
    assert plan.boi_artifact_inventory_sha256 == "b" * 64
    assert plan.boi_qualified_inventory_sha256 == "b" * 64
    assert plan.boi_qualification_receipt_id == "7" * 64
    assert plan.privacy_protocol_receipt_id == "5" * 64
    assert plan.release_manifest_sha256 == "6" * 64

    snapshot.candidate_archive_sha256 = "0" * 64
    with pytest.raises(MODULE.DeploymentError, match="differs from the exact signed"):
        MODULE.verify_deployment_admission(args)


@pytest.mark.parametrize("apply", [False, True], ids=("dry-run", "apply"))
def test_admission_failure_precedes_every_deployment_preflight(
    monkeypatch: pytest.MonkeyPatch,
    apply: bool,
) -> None:
    events: list[str] = []
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_UID_ENV, "41")
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_GID_ENV, "42")

    def reject_admission(_args):
        events.append("admission-verify")
        raise MODULE.DeploymentError("injected admission refusal")

    monkeypatch.setattr(MODULE, "verify_deployment_admission", reject_admission)
    monkeypatch.setattr(
        MODULE,
        "validate_bundle",
        lambda *_args, **_kwargs: pytest.fail("bundle preflight preceded admission"),
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
        operator_network_id="taira", operator_private_key_file=Path("/operator.key"),
        apply=apply,
    )

    with pytest.raises(MODULE.DeploymentError, match="admission refusal"):
        MODULE._execute_after_provisioned_authority_contracts(args, ops=MODULE.SystemOps())

    assert events == ["admission-verify"]




def _receipt_transaction_plan(tmp_path: Path) -> MODULE.AdmissionPlan:
    archive = tmp_path / "candidate.tar.gz"
    _write(archive, b"signed candidate archive")
    ledger = tmp_path / "rollout-admission-replay-v1.json"
    ledger.write_bytes(MODULE.rollout_admission.canonical_replay_ledger_bytes([]))
    return MODULE.AdmissionPlan(
        archive=archive,
        archive_state=MODULE._stable_admission_file(archive, "test archive"),
        authority_dir=tmp_path,
        authority_state=(),
        boi_qualified_handoff=SimpleNamespace(),
        replay_ledger=ledger,
        receipt_id="a" * 64,
        artifact_handoff_sha256="9" * 64,
        boi_artifact_inventory_sha256="0" * 64,
        boi_qualified_inventory_sha256="1" * 64,
        boi_qualification_receipt_id="4" * 64,
        archive_sha256=hashlib.sha256(archive.read_bytes()).hexdigest(),
        privacy_protocol_receipt_id="2" * 64,
        release_manifest_sha256="3" * 64,
        source_commit="b" * 40,
        dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        cargo_lock_sha256="c" * 64,
        workspace_source_manifest_sha256="d" * 64,
        reset_manifest_sha256="e" * 64,
        binary_sha256="f" * 64,
        supervisor_sha256="1" * 64,
        validator_config_sha256=tuple(
            (slug, f"{index}" * 64) for index, slug in enumerate(MODULE.SLUGS, start=2)
        ),
        receipt_signers=MODULE.require_receipt_signer_map(
            _receipt_signer_map(),
            "test receipt signer map",
        ),
        restart_generation="6" * 64,
        signer_fingerprint_sha256="7" * 64,
        release_manifest_verifier_sha256="8" * 64,
    )


def _use_unprivileged_transaction_ledger(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        MODULE,
        "require_protected_replay_ledger",
        MODULE.rollout_admission.load_replay_ledger,
    )
    monkeypatch.setattr(
        MODULE,
        "atomic_replace_owned",
        lambda path, body, **_kwargs: path.write_bytes(body),
    )
    monkeypatch.setattr(
        MODULE,
        "require_admission_archive_unchanged",
        lambda _admission: None,
    )


def _transaction_receipt_ids(admission: MODULE.AdmissionPlan) -> tuple[str, str]:
    return tuple(
        sorted((admission.receipt_id, admission.boi_qualification_receipt_id))
    )


def test_receipt_consumption_restores_exact_ledger_when_rollout_does_not_begin(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    prior = admission.replay_ledger.read_bytes()

    with pytest.raises(MODULE.DeploymentError, match="injected pre-cutover failure"):
        with MODULE.consume_admission_receipt(admission):
            assert (
                admission.receipt_id
                in MODULE.rollout_admission.load_replay_ledger(
                    admission.replay_ledger
                ).consumed_receipt_ids
            )
            assert (
                admission.boi_qualification_receipt_id
                in MODULE.rollout_admission.load_replay_ledger(
                    admission.replay_ledger
                ).consumed_receipt_ids
            )
            raise MODULE.DeploymentError("injected pre-cutover failure")

    assert admission.replay_ledger.read_bytes() == prior


def test_receipt_consumption_remains_committed_after_rollout_begins(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)

    with pytest.raises(MODULE.DeploymentError, match="injected post-cutover failure"):
        with MODULE.consume_admission_receipt(admission) as transaction:
            transaction.mark_rollout_started()
            raise MODULE.DeploymentError("injected post-cutover failure")

    consumed = MODULE.rollout_admission.load_replay_ledger(
        admission.replay_ledger
    ).consumed_receipt_ids
    assert consumed == _transaction_receipt_ids(admission)


def test_successful_receipt_transaction_rechecks_committed_ledger(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)

    with MODULE.consume_admission_receipt(admission) as transaction:
        transaction.mark_rollout_started()

    assert MODULE.rollout_admission.load_replay_ledger(
        admission.replay_ledger
    ).consumed_receipt_ids == _transaction_receipt_ids(admission)


def test_receipt_consumption_cannot_succeed_without_rollout_start(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    prior = admission.replay_ledger.read_bytes()

    with pytest.raises(MODULE.DeploymentError, match="without beginning"):
        with MODULE.consume_admission_receipt(admission):
            pass

    assert admission.replay_ledger.read_bytes() == prior


def test_rollout_start_rejects_removed_receipt_and_preserves_prior_ledger(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    prior = admission.replay_ledger.read_bytes()

    with pytest.raises(MODULE.DeploymentError, match="changed before rollout"):
        with MODULE.consume_admission_receipt(admission) as transaction:
            admission.replay_ledger.write_bytes(prior)
            transaction.mark_rollout_started()

    assert admission.replay_ledger.read_bytes() == prior


def test_unstarted_receipt_rollback_refuses_foreign_ledger_change(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    foreign_receipt = "b" * 64

    with pytest.raises(MODULE.DeploymentError, match="receipt rollback failed"):
        with MODULE.consume_admission_receipt(admission):
            admission.replay_ledger.write_bytes(
                MODULE.rollout_admission.canonical_replay_ledger_bytes(
                    [*_transaction_receipt_ids(admission), foreign_receipt]
                )
            )
            raise MODULE.DeploymentError("injected failure after foreign mutation")

    assert MODULE.rollout_admission.load_replay_ledger(
        admission.replay_ledger
    ).consumed_receipt_ids == tuple(
        sorted((*_transaction_receipt_ids(admission), foreign_receipt))
    )


@pytest.mark.parametrize(
    "replayed_field", ["receipt_id", "boi_qualification_receipt_id"]
)
def test_receipt_consumption_rejects_replay_under_lock(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, replayed_field: str
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    admission.replay_ledger.write_bytes(
        MODULE.rollout_admission.canonical_replay_ledger_bytes(
            [getattr(admission, replayed_field)]
        )
    )
    _use_unprivileged_transaction_ledger(monkeypatch)

    with pytest.raises(MODULE.DeploymentError, match="already consumed"):
        with MODULE.consume_admission_receipt(admission):
            pytest.fail("replayed receipt entered deployment transaction")


def test_receipt_consumption_rejects_ledger_capacity_before_publication(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    prior = admission.replay_ledger.read_bytes()
    consumed = MODULE.rollout_admission.canonical_replay_ledger_bytes(
        list(_transaction_receipt_ids(admission))
    )
    assert len(prior) < len(consumed)
    monkeypatch.setattr(
        MODULE.rollout_admission,
        "MAX_JSON_BYTES",
        len(consumed) - 1,
    )

    with pytest.raises(MODULE.DeploymentError, match="no capacity"):
        with MODULE.consume_admission_receipt(admission):
            pytest.fail("oversized replay ledger was published")

    assert admission.replay_ledger.read_bytes() == prior


def test_archive_substitution_is_rejected_before_rollout(tmp_path: Path) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    replacement = tmp_path / "replacement.tar.gz"
    _write(replacement, b"signed candidate archive")
    os.replace(replacement, admission.archive)

    with pytest.raises(MODULE.DeploymentError, match="substituted"):
        MODULE.require_admission_archive_unchanged(admission)


def test_production_config_may_differ_from_secret_free_qualification(tmp_path: Path) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    peers = tuple(
        SimpleNamespace(slug=slug, config_sha256=digest)
        for slug, digest in admission.validator_config_sha256
    )
    peers = (
        SimpleNamespace(slug=peers[0].slug, config_sha256="9" * 64),
        *peers[1:],
    )
    bundle = SimpleNamespace(
        manifest_sha256=admission.reset_manifest_sha256,
        manifest={
            "source_commit": admission.source_commit,
            "dpn_validator_release_commit": (
                admission.dpn_validator_release_commit
            ),
            "receipt_signers": MODULE.receipt_signer_public_map(
                admission.receipt_signers
            ),
        },
        peers=peers,
        receipt_signers=admission.receipt_signers,
    )
    sources = SimpleNamespace(
        binary_sha256=admission.binary_sha256,
        supervisor_sha256=admission.supervisor_sha256,
    )

    MODULE.require_inputs_match_admission(bundle, sources, admission)

    sources.binary_sha256 = "0" * 64
    with pytest.raises(MODULE.DeploymentError, match="do not match"):
        MODULE.require_inputs_match_admission(bundle, sources, admission)


def test_under_lock_recheck_rejects_python_runtime_identity_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    binary = Path("/candidate/iroha3d")
    supervisor = Path("/candidate/supervisor.py")
    runtime = Path("/Library/Developer/CommandLineTools/Python.app/Python")
    stable = SimpleNamespace(
        st_dev=1,
        st_ino=2,
        st_mode=stat.S_IFREG | 0o555,
        st_uid=0,
        st_gid=0,
        st_nlink=1,
        st_size=3,
        st_mtime_ns=4,
        st_ctime_ns=5,
    )
    changed = copy.copy(stable)
    changed.st_ino = 9
    sources = MODULE.SourcePlan(
        binary=binary,
        binary_sha256="a" * 64,
        supervisor=supervisor,
        supervisor_sha256="b" * 64,
        python=runtime,
        python_identity=MODULE.metadata_identity(stable),
    )
    admission = SimpleNamespace(
        binary_sha256=sources.binary_sha256,
        supervisor_sha256=sources.supervisor_sha256,
    )
    monkeypatch.setattr(
        MODULE, "require_bundle_runtime_unchanged", lambda _bundle: None
    )
    monkeypatch.setattr(
        MODULE,
        "sha256_regular",
        lambda path, _maximum: (
            sources.binary_sha256 if path == binary else sources.supervisor_sha256,
            stable,
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "require_root_controlled_file",
        lambda path, *, executable: changed,
    )

    with pytest.raises(MODULE.DeploymentError, match="Python changed"):
        MODULE.require_admission_bound_inputs_unchanged(
            SimpleNamespace(), sources, admission
        )


def test_exclusive_deployment_lock_refuses_contention(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    lock_path = tmp_path / "deploy.lock"
    _write(lock_path, b"")
    real_fstat = os.fstat

    def root_fstat(descriptor: int) -> SimpleNamespace:
        info = real_fstat(descriptor)
        return SimpleNamespace(
            st_mode=info.st_mode,
            st_nlink=info.st_nlink,
            st_uid=0,
            st_gid=0,
        )

    def contended_flock(_descriptor: int, operation: int) -> None:
        if operation & MODULE.fcntl.LOCK_NB:
            raise BlockingIOError

    monkeypatch.setattr(MODULE, "DEPLOYMENT_LOCK", lock_path)
    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(MODULE.os, "fstat", root_fstat)
    monkeypatch.setattr(MODULE.fcntl, "flock", contended_flock)

    with pytest.raises(MODULE.DeploymentError, match="holds the deployment lock"):
        with MODULE.exclusive_deployment_lock():
            pytest.fail("contended lock was acquired")


def test_headroom_is_required_on_every_distinct_filesystem(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    first = SimpleNamespace(
        stat=lambda: SimpleNamespace(st_dev=11),
        name="first",
    )
    second = SimpleNamespace(
        stat=lambda: SimpleNamespace(st_dev=22),
        name="second",
    )
    roots = {Path("/first"): first, Path("/second"): second}
    monkeypatch.setattr(MODULE, "existing_ancestor", lambda path: roots[path])
    monkeypatch.setattr(
        MODULE.shutil,
        "disk_usage",
        lambda path: SimpleNamespace(free=20_000 if path is first else 9_999),
    )

    with pytest.raises(MODULE.DeploymentError, match="device 22"):
        MODULE.require_filesystem_headroom([Path("/first"), Path("/second")], 10_000)


class _RollbackOps:
    def __init__(
        self,
        snapshots: tuple[MODULE.PlistSnapshot, ...],
        *,
        fail_bootout_label: str | None = None,
    ) -> None:
        self.loaded = set(MODULE.LABELS)
        self.calls: list[tuple[str, str]] = []
        self.fail_bootout_label = fail_bootout_label
        self.supervisor_pids = {
            snapshot.path.stem: 40 + index for index, snapshot in enumerate(snapshots)
        }
        self.processes: dict[int, MODULE.ProcessInfo] = {}
        for index, snapshot in enumerate(snapshots):
            supervisor_pid = self.supervisor_pids[snapshot.path.stem]
            child_pid = 140 + index
            self.processes[supervisor_pid] = MODULE.ProcessInfo(
                pid=supervisor_pid,
                ppid=1,
                uid=snapshot.managed.supervisor_uid,
                argv=snapshot.managed.supervisor_argv,
            )
            self.processes[child_pid] = MODULE.ProcessInfo(
                pid=child_pid,
                ppid=supervisor_pid,
                uid=snapshot.managed.child_uid,
                argv=snapshot.managed.child_argv,
            )

    def launchd_print(self, label: str) -> str | None:
        return (
            f"\tpid = {self.supervisor_pids[label]}\n" if label in self.loaded else None
        )

    def bootout(self, label: str) -> None:
        self.calls.append(("bootout", label))
        self.loaded.discard(label)
        if label == self.fail_bootout_label:
            raise MODULE.DeploymentError("injected bootout failure")

    def bootstrap(self, path: Path) -> None:
        self.calls.append(("bootstrap", path.stem))
        self.loaded.add(path.stem)

    def inspect_process(self, pid: int) -> MODULE.ProcessInfo:
        return self.processes[pid]

    def child_pids(self, parent_pid: int) -> tuple[int, ...]:
        return tuple(
            sorted(
                process.pid
                for process in self.processes.values()
                if process.ppid == parent_pid
            )
        )


def _rollback_snapshots(tmp_path: Path) -> tuple[MODULE.PlistSnapshot, ...]:
    snapshots: list[MODULE.PlistSnapshot] = []
    for index, label in enumerate(MODULE.LABELS):
        pid_file = tmp_path / f"{label}.pid"
        _write(pid_file, f"{140 + index}\n".encode())
        binary = f"/old/bin/iroha3d-{index}"
        config = f"/old/config-{index}.toml"
        supervisor_argv = (
            "/usr/bin/python3",
            "/old/taira_peer_supervisor.py",
            "--binary",
            binary,
            "--config",
            config,
            "--pid-file",
            str(pid_file),
        )
        managed = MODULE.OldManagedIdentity(
            supervisor_uid=os.getuid(),
            supervisor_argv=supervisor_argv,
            child_uid=os.getuid(),
            child_argv=(binary, "--sora", "--config", config),
            pid_file=pid_file,
            pid_file_gid=os.getgid(),
            child_was_present=True,
        )
        snapshots.append(
            MODULE.PlistSnapshot(
                path=tmp_path / f"{label}.plist",
                body=f"old-{label}".encode(),
                mode=0o644,
                uid=0,
                gid=0,
                managed=managed,
            )
        )
    return tuple(snapshots)


def test_rollback_unloads_and_restores_the_whole_four_job_cohort(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    snapshots = _rollback_snapshots(tmp_path)
    restored: list[str] = []
    monkeypatch.setattr(
        MODULE,
        "atomic_replace_owned",
        lambda path, body, **kwargs: restored.append(path.stem),
    )
    ops = _RollbackOps(snapshots)

    MODULE.rollback_cohort(snapshots, ops)  # type: ignore[arg-type]

    assert restored == list(MODULE.LABELS)
    assert [label for action, label in ops.calls if action == "bootout"] == list(
        MODULE.LABELS
    )
    assert [label for action, label in ops.calls if action == "bootstrap"] == list(
        MODULE.LABELS
    )
    assert ops.loaded == set(MODULE.LABELS)


def test_rollback_attempts_full_restore_after_injected_bootout_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    snapshots = _rollback_snapshots(tmp_path)
    restored: list[str] = []
    monkeypatch.setattr(
        MODULE,
        "atomic_replace_owned",
        lambda path, body, **kwargs: restored.append(path.stem),
    )
    ops = _RollbackOps(snapshots, fail_bootout_label=MODULE.LABELS[1])

    with pytest.raises(MODULE.DeploymentError, match="rollback was incomplete"):
        MODULE.rollback_cohort(snapshots, ops)  # type: ignore[arg-type]

    assert restored == list(MODULE.LABELS)
    assert [label for action, label in ops.calls if action == "bootstrap"] == list(
        MODULE.LABELS
    )


def test_cli_defaults_match_the_audited_operator_contract() -> None:
    argv = [
            "--bundle",
            "/bundle",
            "--binary",
            "/binary",
            "--supervisor",
            "/supervisor",
            "--admission-archive",
            "/candidate.tar.gz",
            "--admission-authority-dir",
            "/authority",
            "--boi-qualified-handoff-root",
            "/qualified-boi",
            "--expected-source-commit",
            "c" * 40,
            "--expected-dpn-validator-release-commit",
            DPN_VALIDATOR_RELEASE_COMMIT,
            "--expected-cargo-lock-sha256",
            "d" * 64,
            "--expected-workspace-source-manifest-sha256",
            "e" * 64,
                "--expected-receipt-id",
                "f" * 64,
                "--expected-artifact-handoff-sha256",
                "9" * 64,
                "--expected-production-reset-manifest-sha256",
                "a" * 64,
            "--trusted-signing-fingerprint",
            "1" * 64,
            "--trusted-boi-qualification-public-key",
            "/qualification.pub",
            "--trusted-boi-qualification-signing-fingerprint",
            "3" * 64,
            "--expected-boi-qualification-host-id",
            "boi-host-v1",
            "--expected-boi-qualification-installation-id",
            "boi-installation-v1",
            "--expected-boi-qualification-controller-digest",
            "4" * 64,
            "--expected-workflow-run-id",
            "101",
            "--expected-workflow-run-attempt",
            "2",
            "--release-manifest-verifier",
            "/sorafs-validate",
            "--trusted-release-manifest-verifier-sha256",
            "2" * 64,
        ]
    args = MODULE.build_parser().parse_args(argv)
    assert args.health_timeout_seconds == 240
    assert args.minimum_free_bytes == 17_179_869_184
    assert args.maximum_fsync_latency_ms == 250
    assert args.supervisor_python == MODULE.DEFAULT_SUPERVISOR_PYTHON
    assert args.boi_qualified_handoff_root == Path("/qualified-boi")
    assert not hasattr(args, "restart_generation")
    assert not hasattr(args, "expected_binary_sha256")
    assert not hasattr(args, "expected_supervisor_sha256")
    assert args.allow_absent_old_child is False
    assert args.apply is False
    missing_boi = list(argv)
    index = missing_boi.index("--boi-qualified-handoff-root")
    del missing_boi[index : index + 2]
    with pytest.raises(SystemExit):
        MODULE.build_parser().parse_args(missing_boi)


def test_release_and_boi_qualification_signers_must_be_distinct() -> None:
    assert MODULE.require_distinct_signing_fingerprints("1" * 64, "2" * 64) == (
        "1" * 64,
        "2" * 64,
    )
    with pytest.raises(MODULE.DeploymentError, match="must be distinct"):
        MODULE.require_distinct_signing_fingerprints("1" * 64, "1" * 64)


@pytest.mark.parametrize("apply", [False, True], ids=("dry-run", "apply"))
def test_deploy_issuance_barrier_precedes_identity_paths_admission_and_state(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    apply: bool,
) -> None:
    calls: list[str] = []

    def forbidden(name: str):
        def call(*_args, **_kwargs):
            calls.append(name)
            raise AssertionError(f"deploy barrier reached forbidden operation: {name}")

        return call

    for name in (
        "require_sealed_external_tool_identity",
        "validate_arguments",
        "verify_deployment_admission",
        "validate_bundle",
        "validate_sources",
        "exclusive_deployment_lock",
        "consume_admission_receipt",
        "apply_reset",
    ):
        monkeypatch.setattr(MODULE, name, forbidden(name))
    monkeypatch.setattr(MODULE.os, "geteuid", forbidden("geteuid"))
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_UID_ENV, "41")
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_GID_ENV, "42")
    state = tmp_path / "deployment-state"
    state.write_bytes(b"unchanged\n")

    with pytest.raises(MODULE.DeploymentError) as error:
        MODULE.execute(argparse.Namespace(apply=apply), ops=object())

    assert MODULE.DEPLOY_AUTHENTICATED_RUN_NONCE_CONTRACT in str(error.value)
    assert MODULE.COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT in str(error.value)
    assert calls == []
    assert state.read_bytes() == b"unchanged\n"


@pytest.mark.parametrize(
    ("raw_uid", "raw_gid", "message"),
    [
        (None, "41", "incomplete"),
        ("41", None, "incomplete"),
        ("0", "41", "positive canonical"),
        ("41", "0", "positive canonical"),
        ("041", "42", "positive canonical"),
        ("+41", "42", "noncanonical"),
        ("41 ", "42", "noncanonical"),
        ("４１", "42", "noncanonical"),
    ],
)
def test_sealed_external_tool_identity_rejects_malformed_ids(
    monkeypatch: pytest.MonkeyPatch,
    raw_uid: str | None,
    raw_gid: str | None,
    message: str,
) -> None:
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setattr(MODULE.os, "getegid", lambda: 0)
    for name, value in (
        (MODULE.EXTERNAL_TOOL_UID_ENV, raw_uid),
        (MODULE.EXTERNAL_TOOL_GID_ENV, raw_gid),
    ):
        if value is None:
            monkeypatch.delenv(name, raising=False)
        else:
            monkeypatch.setenv(name, value)

    with pytest.raises(MODULE.DeploymentError, match=message):
        MODULE.require_sealed_external_tool_identity()


def test_sealed_external_tool_identity_is_exact_for_root_and_non_root(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_UID_ENV, "41")
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_GID_ENV, "42")
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setattr(MODULE.os, "getegid", lambda: 0)
    assert MODULE.require_sealed_external_tool_identity() == (41, 42)

    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 41)
    monkeypatch.setattr(MODULE.os, "getegid", lambda: 42)
    assert MODULE.require_sealed_external_tool_identity() is None

    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_UID_ENV, "43")
    with pytest.raises(MODULE.DeploymentError, match="differs from the current identity"):
        MODULE.require_sealed_external_tool_identity()


EXPORTED_TESTS = (
    test_deployment_admission_requires_and_binds_qualified_boi_result,
    test_admission_failure_precedes_every_deployment_preflight,
    test_receipt_consumption_restores_exact_ledger_when_rollout_does_not_begin,
    test_receipt_consumption_remains_committed_after_rollout_begins,
    test_successful_receipt_transaction_rechecks_committed_ledger,
    test_receipt_consumption_cannot_succeed_without_rollout_start,
    test_rollout_start_rejects_removed_receipt_and_preserves_prior_ledger,
    test_unstarted_receipt_rollback_refuses_foreign_ledger_change,
    test_receipt_consumption_rejects_replay_under_lock,
    test_receipt_consumption_rejects_ledger_capacity_before_publication,
    test_archive_substitution_is_rejected_before_rollout,
    test_production_config_may_differ_from_secret_free_qualification,
    test_under_lock_recheck_rejects_python_runtime_identity_drift,
    test_exclusive_deployment_lock_refuses_contention,
    test_headroom_is_required_on_every_distinct_filesystem,
    test_rollback_unloads_and_restores_the_whole_four_job_cohort,
    test_rollback_attempts_full_restore_after_injected_bootout_failure,
    test_cli_defaults_match_the_audited_operator_contract,
    test_release_and_boi_qualification_signers_must_be_distinct,
    test_deploy_issuance_barrier_precedes_identity_paths_admission_and_state,
    test_sealed_external_tool_identity_rejects_malformed_ids,
    test_sealed_external_tool_identity_is_exact_for_root_and_non_root,
)
