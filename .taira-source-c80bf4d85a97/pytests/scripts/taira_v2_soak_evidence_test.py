"""Adversarial tests for durable Taira v2 soak evidence validation."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import re

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "check_taira_v2_soak_evidence.py"


def load_module():
    spec = importlib.util.spec_from_file_location("taira_soak_evidence", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def valid_status_snapshot(validator_index: int, blocker: str | None = None) -> dict:
    snapshot = {
        "validator_index": validator_index,
        "status": {
            "protocol_version": 3,
            "restart_required": False,
            "height": 10,
            "view": 0,
            "leader": 0,
            "liveness": {
                "generation": 1,
                "prepare_quorums": [],
                "commit_quorums": [],
                "timeout_quorums": [],
                "outbound_intents": [],
                "work": {},
                "queues": [{}],
                "no_progress_age_ms": 0,
                "ignore_counts": [],
            },
        },
    }
    if blocker is not None:
        snapshot["status"]["liveness"]["blocker"] = {
            "blocker": blocker,
            "details": None,
        }
    return snapshot


def valid_summary(module, tmp_path: Path) -> tuple[dict, Path, Path]:
    build_root = tmp_path / "build"
    build_root.mkdir()
    binaries = {}
    for name in ("daemon", "kagami", "test"):
        release_root = build_root / module.EXPECTED_BINARY_SUBDIRECTORIES[name]
        release_root.mkdir(parents=True, exist_ok=True)
        path = release_root / name
        path.write_bytes(f"{name}-binary".encode())
        binaries[f"{name}_binary_path"] = str(path)
        binaries[f"{name}_binary_blake2b_256"] = module._file_digest(path)

    artifact_root = tmp_path / "target" / "taira-localnet" / "localnet"
    artifact_root.mkdir(parents=True)
    (artifact_root / "peer0.toml").write_text("chain = 'taira'\n", encoding="utf-8")
    source_manifest = "a" * 64
    summary = {
        **module.EXPECTED_PROFILE,
        **binaries,
        "git_revision": "revision",
        "workspace_source_manifest_sha256": source_manifest,
        "localnet_artifact_path": str(artifact_root),
        "generated_config_blake2b_256": module._generated_config_digest(artifact_root),
        "duration_secs": 86_400,
        "soak_overrun_secs": 0.0,
        "expected_process_churn_cycles": 287,
        "expected_membership_churn_cycles": 288,
        "process_churn_cycles": 259,
        "process_churn_lagged_cycles": 0,
        "membership_join_cycles": 130,
        "membership_leave_cycles": 130,
        "membership_churn_lagged_cycles": 0,
        "membership_churn_warning_cycles": 2,
        "membership_cleanup_leave": False,
        "churn_paused_secs": 8_640.0,
        "churn_paused_ratio": 0.1,
        "scheduled_tps": 5.0,
        "submitted_tps": 4.9,
        "committed_tps": 4.8,
        "tx_sent": 423_360,
        "tx_attempted": 432_000,
        "tx_submit_errors": 8_640,
        "committed_txs_min_delta": 414_720,
        "max_height_skew_observed": 1,
        "view_changes_start": 10,
        "view_changes_end": 10,
        "view_change_rate_per_sec": 0.0,
        "saturated_samples": 0,
        "total_samples": 100,
        "unclassified_no_progress_intervals": 0,
        "no_progress_intervals": [],
        "initial_status_snapshots": [valid_status_snapshot(index) for index in range(3)],
        "final_status_snapshots": [valid_status_snapshot(index) for index in range(3)],
    }
    return summary, build_root, artifact_root


def stub_repository_identity(module, monkeypatch) -> None:
    monkeypatch.setattr(module, "_current_git_revision", lambda _root: "revision")
    monkeypatch.setattr(
        module, "_current_workspace_source_manifest", lambda _root: "a" * 64
    )


def test_valid_evidence_is_source_and_artifact_bound(tmp_path: Path, monkeypatch) -> None:
    module = load_module()
    summary, build_root, _ = valid_summary(module, tmp_path)
    stub_repository_identity(module, monkeypatch)
    module.validate_evidence(
        summary,
        source_manifest_sha256="a" * 64,
        build_root=build_root,
        repo_root=tmp_path,
    )


def test_duplicate_json_keys_are_rejected_recursively() -> None:
    module = load_module()
    with pytest.raises(module.EvidenceError, match="duplicate JSON object key"):
        module._decode_evidence(b'{"outer":{"seed":"a","seed":"b"}}')


def test_non_finite_json_numbers_are_rejected_recursively() -> None:
    module = load_module()
    for payload in (
        b'{"outer":{"rate":NaN}}',
        b'{"outer":{"rate":1e10000}}',
    ):
        with pytest.raises(module.EvidenceError, match="non-finite JSON number"):
            module._decode_evidence(payload)


def test_checker_recomputes_the_canonical_workspace_manifest(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    summary, build_root, _ = valid_summary(module, tmp_path)
    stub_repository_identity(module, monkeypatch)
    monkeypatch.setattr(
        module, "_current_workspace_source_manifest", lambda _root: "b" * 64
    )
    with pytest.raises(module.EvidenceError, match="workspace source manifest drifted"):
        module.validate_evidence(
            summary,
            source_manifest_sha256="a" * 64,
            build_root=build_root,
            repo_root=tmp_path,
        )


def test_manifest_recomputation_delegates_to_the_canonical_workspace_helper(
    monkeypatch,
) -> None:
    module = load_module()
    repo_root = ROOT_DIR.resolve()
    expected_command = [
        module.sys.executable,
        str(repo_root / "scripts" / "compute_workspace_source_manifest.py"),
        "--root",
        str(repo_root),
    ]

    def fake_run(command, **kwargs):
        assert command == expected_command
        assert kwargs == {
            "cwd": repo_root,
            "check": True,
            "capture_output": True,
            "text": True,
        }
        return type("Result", (), {"stdout": f"{'a' * 64}\n"})()

    monkeypatch.setattr(module.subprocess, "run", fake_run)
    assert module._current_workspace_source_manifest(repo_root) == "a" * 64


def test_debug_profile_binary_is_rejected(tmp_path: Path, monkeypatch) -> None:
    module = load_module()
    summary, build_root, _ = valid_summary(module, tmp_path)
    debug_binary = build_root / "programs" / "debug" / "daemon"
    debug_binary.parent.mkdir(parents=True)
    debug_binary.write_bytes(b"daemon-binary")
    summary["daemon_binary_path"] = str(debug_binary)
    summary["daemon_binary_blake2b_256"] = module._file_digest(debug_binary)
    stub_repository_identity(module, monkeypatch)
    with pytest.raises(module.EvidenceError, match="pinned release-profile"):
        module.validate_evidence(
            summary,
            source_manifest_sha256="a" * 64,
            build_root=build_root,
            repo_root=tmp_path,
        )


@pytest.mark.parametrize(
    "classification",
    ["missing_proposal", "local_control_pending"],
)
def test_classified_interval_is_bound_to_authoritative_statuses(
    tmp_path: Path, monkeypatch, classification: str
) -> None:
    module = load_module()
    summary, build_root, _ = valid_summary(module, tmp_path)
    summary["no_progress_intervals"] = [
        {
            "start_elapsed_ms": 1_000,
            "end_elapsed_ms": 2_000,
            "classifications": [classification],
            "classified": True,
            "status_snapshots": [
                valid_status_snapshot(index, classification) for index in range(3)
            ],
        }
    ]
    stub_repository_identity(module, monkeypatch)
    module.validate_evidence(
        summary,
        source_manifest_sha256="a" * 64,
        build_root=build_root,
        repo_root=tmp_path,
    )


def test_evidence_schema_matches_rust_summary_exactly() -> None:
    module = load_module()
    source = (ROOT_DIR / "integration_tests/tests/taira_public_localnet.rs").read_text(
        encoding="utf-8"
    )
    body = source.split("impl SimulationSummary {", 1)[1].split(
        "#[derive(Clone, Debug)]\nstruct NoProgressInterval", 1
    )[0]
    rust_fields = set(re.findall(r'^\s*"([a-z0-9_]+)":', body, flags=re.MULTILINE))
    assert rust_fields == module.EXPECTED_SUMMARY_FIELDS


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        (lambda summary: summary.__setitem__("build_profile", "debug"), "build_profile"),
        (
            lambda summary: summary.__setitem__("cargo_net_offline", 1),
            "cargo_net_offline",
        ),
        (lambda summary: summary.__setitem__("duration_secs", 30), "24 wall-clock hours"),
        (lambda summary: summary.__setitem__("soak_overrun_secs", 901), "overrun"),
        (lambda summary: summary.__setitem__("process_churn_cycles", 1), "process churn"),
        (lambda summary: summary.__setitem__("churn_paused_ratio", 0.99), "too much"),
        (
            lambda summary: summary.__setitem__("unclassified_no_progress_intervals", 1),
            "unclassified",
        ),
    ],
)
def test_weakened_evidence_is_rejected(tmp_path: Path, monkeypatch, mutation, message) -> None:
    module = load_module()
    summary, build_root, _ = valid_summary(module, tmp_path)
    mutation(summary)
    stub_repository_identity(module, monkeypatch)
    with pytest.raises(module.EvidenceError, match=message):
        module.validate_evidence(
            summary,
            source_manifest_sha256="a" * 64,
            build_root=build_root,
            repo_root=tmp_path,
        )


def test_binary_or_config_tampering_is_rejected(tmp_path: Path, monkeypatch) -> None:
    module = load_module()
    summary, build_root, artifact_root = valid_summary(module, tmp_path)
    stub_repository_identity(module, monkeypatch)
    Path(summary["daemon_binary_path"]).write_bytes(b"tampered")
    with pytest.raises(module.EvidenceError, match="daemon binary digest mismatch"):
        module.validate_evidence(
            summary,
            source_manifest_sha256="a" * 64,
            build_root=build_root,
            repo_root=tmp_path,
        )

    Path(summary["daemon_binary_path"]).write_bytes(b"daemon-binary")
    (artifact_root / "peer0.toml").write_text("chain = 'tampered'\n", encoding="utf-8")
    with pytest.raises(module.EvidenceError, match="configuration digest mismatch"):
        module.validate_evidence(
            summary,
            source_manifest_sha256="a" * 64,
            build_root=build_root,
            repo_root=tmp_path,
        )


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        (lambda summary: summary.__setitem__("unexpected", True), "summary fields"),
        (lambda summary: summary.__setitem__("scheduled_tps", 5.1), "scheduled_tps"),
        (lambda summary: summary.__setitem__("churn_paused_secs", 1.0), "churn_paused_ratio"),
        (lambda summary: summary.__setitem__("tx_attempted", 432_001), "transaction accounting"),
        (
            lambda summary: summary.__setitem__("committed_txs_min_delta", 414_719),
            "committed_tps",
        ),
        (lambda summary: summary.__setitem__("view_changes_end", 9), "counter regressed"),
        (lambda summary: summary.__setitem__("total_samples", 0), "total_samples"),
        (
            lambda summary: summary.__setitem__(
                "final_status_snapshots",
                [valid_status_snapshot(index) for index in range(2)],
            ),
            "valid quorum",
        ),
        (
            lambda summary: summary.__setitem__(
                "no_progress_intervals",
                [
                    {
                        "start_elapsed_ms": 1,
                        "end_elapsed_ms": 2,
                        "classifications": ["view_changed"],
                        "classified": True,
                        "status_snapshots": [],
                    }
                ],
            ),
            "invalid watchdog classification",
        ),
    ],
)
def test_internally_inconsistent_evidence_is_rejected(
    tmp_path: Path, monkeypatch, mutation, message
) -> None:
    module = load_module()
    summary, build_root, _ = valid_summary(module, tmp_path)
    mutation(summary)
    stub_repository_identity(module, monkeypatch)
    with pytest.raises(module.EvidenceError, match=message):
        module.validate_evidence(
            summary,
            source_manifest_sha256="a" * 64,
            build_root=build_root,
            repo_root=tmp_path,
        )


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        (
            lambda summary: summary.__setitem__(
                "initial_status_snapshots", [valid_status_snapshot(0) for _ in range(3)]
            ),
            "distinct validator",
        ),
        (
            lambda summary: summary["final_status_snapshots"][0]["status"].__setitem__(
                "restart_required", True
            ),
            "fail-stopped",
        ),
        (
            lambda summary: summary["final_status_snapshots"][0]["status"].pop(
                "liveness"
            ),
            "required Sumeragi fields",
        ),
        (
            lambda summary: summary["final_status_snapshots"][0]["status"][
                "liveness"
            ].__setitem__("queues", []),
            "bounded-queue evidence",
        ),
    ],
)
def test_status_snapshot_evidence_is_authoritative(
    tmp_path: Path, monkeypatch, mutation, message
) -> None:
    module = load_module()
    summary, build_root, _ = valid_summary(module, tmp_path)
    mutation(summary)
    stub_repository_identity(module, monkeypatch)
    with pytest.raises(module.EvidenceError, match=message):
        module.validate_evidence(
            summary,
            source_manifest_sha256="a" * 64,
            build_root=build_root,
            repo_root=tmp_path,
        )


@pytest.mark.parametrize(
    ("classifications", "blocker", "message"),
    [
        (["missing_proposal"], None, "lacks a watchdog blocker"),
        (["missing_proposal"], "body_unavailable", "disagree"),
        (
            ["missing_proposal", "missing_proposal"],
            "missing_proposal",
            "unique and canonical",
        ),
    ],
)
def test_no_progress_interval_cannot_forge_its_classification(
    tmp_path: Path, monkeypatch, classifications, blocker, message
) -> None:
    module = load_module()
    summary, build_root, _ = valid_summary(module, tmp_path)
    summary["no_progress_intervals"] = [
        {
            "start_elapsed_ms": 1_000,
            "end_elapsed_ms": 2_000,
            "classifications": classifications,
            "classified": True,
            "status_snapshots": [
                valid_status_snapshot(index, blocker) for index in range(3)
            ],
        }
    ]
    stub_repository_identity(module, monkeypatch)
    with pytest.raises(module.EvidenceError, match=message):
        module.validate_evidence(
            summary,
            source_manifest_sha256="a" * 64,
            build_root=build_root,
            repo_root=tmp_path,
        )
