import importlib.util
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run_izanami_communication_vulnerability_matrix.sh"
LIVENESS_SCRIPT = ROOT / "scripts" / "run_izanami_liveness_matrix.py"
LIVENESS_SPEC = importlib.util.spec_from_file_location(
    "run_izanami_liveness_matrix", LIVENESS_SCRIPT
)
assert LIVENESS_SPEC is not None and LIVENESS_SPEC.loader is not None
LIVENESS = importlib.util.module_from_spec(LIVENESS_SPEC)
sys.modules[LIVENESS_SPEC.name] = LIVENESS
LIVENESS_SPEC.loader.exec_module(LIVENESS)


def _classifier_degraded_pattern() -> str:
    source = SCRIPT.read_text()
    match = re.search(r"^acceptance_failure_regex='([^']+)'", source, re.MULTILINE)
    assert match is not None
    return match.group(1)


def test_liveness_rows_reject_retired_v1_consensus_dimensions() -> None:
    rows = LIVENESS.parse_rows("baseline:1024:1:300")
    assert rows == [LIVENESS.MatrixRow("baseline", 1024, 1, 300)]

    try:
        LIVENESS.parse_rows("legacy:1024:1:300:2:2")
    except ValueError as error:
        assert "revision-4" in str(error)
    else:
        raise AssertionError("retired collector dimensions must fail closed")

    source = LIVENESS_SCRIPT.read_text(encoding="utf-8")
    assert "--sumeragi-collectors-k" not in source
    assert "--sumeragi-inline-block-created-backup-rbc" not in source


def test_liveness_matrix_accepts_only_revision4_committee_geometry() -> None:
    assert [
        peers
        for peers in range(1, 33)
        if LIVENESS.is_revision4_committee_size(peers)
    ] == [4, 7, 10, 13, 16, 19, 22, 25, 28, 31]


def test_matrix_classifier_ignores_retryable_endpoint_refusals() -> None:
    pattern = _classifier_degraded_pattern()

    assert "Connection refused" not in pattern
    assert "connection closed before message completed" not in pattern


def test_matrix_classifier_keeps_final_liveness_failure_markers() -> None:
    pattern = _classifier_degraded_pattern()

    for marker in (
        "panic",
        "HTTP status 429",
        "429 Too Many Requests",
        "confirmation timeout",
        "sampled confirmation failed",
        "transaction did not reach",
        "transaction remained queued",
        "route_unavailable",
        "failures=[1-9][0-9]*",
        "confirmation_failed=[1-9][0-9]*",
    ):
        assert marker in pattern


def test_matrix_classifier_does_not_match_tolerated_fault_metadata(tmp_path: Path) -> None:
    pattern = _classifier_degraded_pattern()

    tolerated = tmp_path / "tolerated.log"
    tolerated.write_text(
        "progress tolerated_failures=5\n"
        "summary expected_failures=13 confirmation_failed=0\n"
        "summary expected_failures=3 failures=0 confirmation_failed=0\n"
    )
    actual = tmp_path / "actual.log"
    actual.write_text("summary failures=1 confirmation_failed=0\n")

    assert subprocess.run(["rg", "-q", pattern, str(tolerated)]).returncode == 1
    assert subprocess.run(["rg", "-q", pattern, str(actual)]).returncode == 0


def test_matrix_stress_mode_writes_paper_style_report(tmp_path: Path) -> None:
    out_dir = tmp_path / "matrix"

    subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--out",
            str(out_dir),
            "--mode",
            "stress-1200",
            "--only",
            "targeted-load",
            "--sumeragi-mode",
            "permissioned",
            "--izanami-cmd",
            "true",
        ],
        check=True,
        cwd=ROOT,
    )

    report = out_dir / "paper-style-final-report.md"
    summary = out_dir / "summary.tsv"
    evidence = out_dir / "evidence.tsv"
    assert report.exists()
    assert summary.exists()
    assert evidence.exists()
    assert "Mode: `stress-1200`" in report.read_text()
    assert "throughput_evidence" in evidence.read_text().splitlines()[0]
    assert "stress_labels" in evidence.read_text().splitlines()[0]
    assert "consensus_pressure" in evidence.read_text().splitlines()[0]
    assert "submit_latency_p95_ms" in evidence.read_text().splitlines()[0]
    assert (out_dir / "root-cause.md").exists()
    assert "--peers 19" in (out_dir / "permissioned-targeted-load.log").read_text()

    report.unlink()
    (out_dir / "summary.md").unlink()
    evidence.unlink()
    subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--out",
            str(out_dir),
            "--mode",
            "stress-1200",
            "--sumeragi-mode",
            "permissioned",
            "--report-only",
        ],
        check=True,
        cwd=ROOT,
    )

    assert report.exists()
    assert "Iroha (Sumeragi permissioned)" in report.read_text()
    assert "targeted-load" in summary.read_text()
    assert "throughput_evidence" in evidence.read_text().splitlines()[0]
    assert "stress_labels" in evidence.read_text().splitlines()[0]
    assert "submit_latency_p95_ms" in evidence.read_text().splitlines()[0]


def test_stopping_matrix_stays_within_revision4_fault_budget(tmp_path: Path) -> None:
    for mode, peers, faulty in (("quick", 4, 1), ("paper", 19, 6)):
        out_dir = tmp_path / mode
        subprocess.run(
            [
                "bash",
                str(SCRIPT),
                "--out",
                str(out_dir),
                "--mode",
                mode,
                "--only",
                "stopping",
                "--sumeragi-mode",
                "permissioned",
                "--izanami-cmd",
                "true",
            ],
            check=True,
            cwd=ROOT,
        )

        command_log = (out_dir / "permissioned-stopping.log").read_text()
        assert f"--peers {peers}" in command_log
        assert f"--faulty {faulty}" in command_log


def test_stress_matrix_marks_driver_saturation_and_consensus_stall(tmp_path: Path) -> None:
    out_dir = tmp_path / "matrix"
    fake_izanami = tmp_path / "fake_izanami.sh"
    fake_izanami.write_text(
        "#!/usr/bin/env bash\n"
        "echo '2026-04-29T00:00:00Z INFO izanami::summary: izanami run complete "
        "offered=83399 ingress_accepted=83399 submit_plans_started=83399 "
        "submit_latency_p50_ms=411 submit_latency_p95_ms=1882 "
        "submit_latency_p99_ms=3744 submit_latency_max_ms=10987 "
        "final_quorum_min_height=Some(1) final_strict_min_height=Some(1) "
        "final_max_peer_height_skew=Some(0) "
        "sumeragi_status_delta=Some(SumeragiStatusDigest { "
        "protocol_version: 4, height: 2, view: 5, phase: \"Prepare\", "
        "body_state: \"Missing\", last_committed_height: 1, "
        "committed_height_advance: 0, mode: \"Permissioned\", epoch: 0, "
        "epoch_end_height: 100, validator_count: 4, min_signers: 3, "
        "total_power: 4, commit_qc_present: true, commit_qc_signer_count: 3, "
        "commit_qc_min_signers: 3, commit_qc_signed_power: 3, "
        "commit_qc_total_power: 4, view_change_install_total: 5, "
        "busy_deferral_total: 7, adapter_ingress_keys: 1, "
        "adapter_ingress_capacity: 1024, adapter_deferred_completion: 0, "
        "adapter_deferred_progress: 0, adapter_deferred_progress_capacity: 512, "
        "adapter_deferred_normal: 0, adapter_deferred_normal_capacity: 512, "
        "tx_queue_tracked: 4096, tx_queue_depth: 4096, tx_queue_capacity: 4096, "
        "tx_queue_retained_bytes: 1048576, tx_queue_max_retained_bytes: 1048576, "
        "tx_queue_saturated: true, tx_queue_saturated_by_count: true, "
        "tx_queue_saturated_by_bytes: true, tx_queue_saturated_by_age: false, "
        "tx_queue_oldest_queued_age_ms: 500, lane_settlement_commitments: 1, "
        "lane_relay_envelopes: 1, lane_payload_ownerships: 1, "
        "committed_lane_blocks: 1, lane_block_sessions: 1, "
        "incomplete_lane_block_sessions: 0, blocked_committed_lane_blocks: 0, "
        "local_peer_removed: false })'\n"
    )
    fake_izanami.chmod(0o755)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--out",
            str(out_dir),
            "--mode",
            "stress-20000",
            "--only",
            "targeted-load",
            "--sumeragi-mode",
            "permissioned",
            "--izanami-cmd",
            str(fake_izanami),
        ],
        cwd=ROOT,
        check=False,
    )

    assert result.returncode == 1
    summary_rows = (out_dir / "summary.tsv").read_text().splitlines()
    assert summary_rows[1].split("\t")[3:5] == [
        "driver-saturated,consensus-stalled,overload-admission",
        "degraded",
    ]
    evidence_lines = (out_dir / "evidence.tsv").read_text().splitlines()
    header = evidence_lines[0].split("\t")
    row = evidence_lines[1].split("\t")
    assert len(row) == len(header)
    evidence = evidence_lines[1]
    assert "status=driver-saturated" in evidence
    assert "driver-saturated,consensus-stalled,overload-admission" in evidence
    assert "\tbusy-deferral\t" in evidence
    assert row[header.index("protocol_version")] == "4"
    assert row[header.index("commit_qc_present")] == "true"
    assert row[header.index("tx_queue_saturated_by_bytes")] == "true"
    assert "view_change_cause_total" not in header
    assert "pacemaker_backpressure_deferrals_total" not in header
    assert "offered_ratio=" in evidence
    assert "accepted_tps=104.25" in evidence
    log = (out_dir / "permissioned-targeted-load.log").read_text()
    assert "--tps 20000" in log
    assert "--max-inflight 20000" in log
    assert "--diagnostic-dir" in log


def test_stress_matrix_rejects_legacy_status_digest_evidence(tmp_path: Path) -> None:
    out_dir = tmp_path / "matrix"
    fake_izanami = tmp_path / "fake_legacy_izanami.sh"
    fake_izanami.write_text(
        "#!/usr/bin/env bash\n"
        "echo '2026-04-29T00:00:00Z INFO izanami::summary: izanami run complete "
        "offered=83399 ingress_accepted=83399 submit_plans_started=83399 "
        "final_quorum_min_height=Some(1) final_strict_min_height=Some(1) "
        "sumeragi_status_delta=Some(SumeragiStatusDigest { "
        "view_change_install_total: 5, view_change_cause_total: 5, "
        "view_change_last_cause: Some(\"quorum_timeout\"), "
        "tx_queue_depth: 4096, tx_queue_capacity: 4096, "
        "tx_queue_saturated: true, pacemaker_backpressure_deferrals_total: 7 })'\n"
    )
    fake_izanami.chmod(0o755)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--out",
            str(out_dir),
            "--mode",
            "stress-20000",
            "--only",
            "targeted-load",
            "--sumeragi-mode",
            "permissioned",
            "--izanami-cmd",
            str(fake_izanami),
        ],
        cwd=ROOT,
        check=False,
    )

    assert result.returncode == 1
    summary = (out_dir / "summary.tsv").read_text().splitlines()[1].split("\t")
    assert summary[3:5] == [
        "driver-saturated,consensus-stalled,diagnostic-incomplete",
        "degraded",
    ]
    evidence_lines = (out_dir / "evidence.tsv").read_text().splitlines()
    header = evidence_lines[0].split("\t")
    row = evidence_lines[1].split("\t")
    assert len(row) == len(header)
    assert row[header.index("protocol_version")] == ""
    assert row[header.index("view_change_install_total")] == ""
    assert row[header.index("tx_queue_saturated")] == ""


def test_sweep_aggregates_profile_and_seed(tmp_path: Path) -> None:
    out_dir = tmp_path / "sweep"
    sweep_script = ROOT / "scripts" / "run_izanami_communication_vulnerability_sweep.sh"

    subprocess.run(
        [
            "bash",
            str(sweep_script),
            "--out",
            str(out_dir),
            "--profiles",
            "quick",
            "--seed-list",
            "7",
            "--only",
            "targeted-load",
            "--sumeragi-mode",
            "permissioned",
            "--izanami-cmd",
            "true",
        ],
        check=True,
        cwd=ROOT,
    )

    summary_lines = (out_dir / "sweep-summary.tsv").read_text().splitlines()
    assert summary_lines[0].startswith("profile\tseed\t")
    assert any(line.startswith("quick\t7\tpermissioned\ttargeted-load\t") for line in summary_lines[1:])
    assert (out_dir / "sweep-evidence.tsv").exists()
    assert (out_dir / "sweep-report.md").exists()
