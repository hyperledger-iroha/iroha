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


def test_liveness_rows_reject_retired_consensus_dimensions() -> None:
    rows = LIVENESS.parse_rows("baseline:1024:300")
    assert rows == [LIVENESS.MatrixRow("baseline", 1024, 300)]

    try:
        LIVENESS.parse_rows("legacy:1024:1:300:2:2")
    except ValueError as error:
        assert "native matrix rows" in str(error)
    else:
        raise AssertionError("retired collector dimensions must fail closed")

    try:
        LIVENESS.parse_rows("retired_scan:1024:1:300")
    except ValueError:
        pass
    else:
        raise AssertionError("retired proposal scan tuning must fail closed")
    source = LIVENESS_SCRIPT.read_text(encoding="utf-8")
    assert "--sumeragi-proposal-queue-scan-multiplier" not in source
    assert "--sumeragi-collectors-k" not in source
    assert "--sumeragi-inline-block-created-backup-rbc" not in source


def test_liveness_rows_pass_on_the_runner_strict_interval(tmp_path: Path) -> None:
    import argparse

    run_dir = tmp_path / "cap1024_pipe300"
    run_dir.mkdir()
    (run_dir / "runner.log").write_text(
        "2026-09-30T00:00:00Z INFO izanami::progress: strict_min_height=40 "
        "ingress_accepted=1200 offered=1210 strict block height advanced\n"
        "2026-09-30T00:01:00Z INFO izanami::summary: offered=1210 "
        "ingress_accepted=1200 final_strict_min_height=Some(41) "
        "final_strict_min_txs_approved=Some(1200) "
        "final_quorum_block_interval_p95_ms=Some(900) "
        "final_strict_block_interval_p95_ms=Some(1800) "
        "sumeragi_status_delta=Some(SumeragiStatusDigest { status: SumeragiStatus { "
        "protocol_version: 1, height: 42, view: 1, stage: 0, level: 0, "
        "committed_height: 41, applied_height: 41, halted: None }, "
        "committed_height_advance: 40 }) izanami run complete\n"
    )
    args = argparse.Namespace(
        duration=60, progress_interval_s=5, strict_interval_p95_threshold_ms=3_000
    )
    row = LIVENESS.MatrixRow("cap1024_pipe300", 1024, 300)

    result = LIVENESS.collect_result(args, row, tmp_path, 0)
    assert result["row_pass"] is True
    assert result["runner_strict_interval_p95_ms"] == 1800
    assert result["runner_quorum_interval_p95_ms"] == 900
    assert result["committed_tps"] == "20.00"
    assert (result["committed_height_advance"], result["view"], result["halted"]) == (
        "40",
        "1",
        "None",
    )

    args.strict_interval_p95_threshold_ms = 1_000
    assert LIVENESS.collect_result(args, row, tmp_path, 0)["row_pass"] is False


def test_liveness_matrix_accepts_only_native_committee_geometry() -> None:
    assert [
        peers
        for peers in range(1, 33)
        if LIVENESS.is_admitted_committee_size(peers)
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


def test_matrix_rejects_retired_packet_loss_execution_surface(tmp_path: Path) -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    assert "--fault-enable-network-packet-loss" not in source
    assert "--fault-network-packet-loss-percent" not in source
    assert "--packet-loss-sweep" not in source
    assert "faults_network_partition=(" in source
    assert "--fault-enable-network-partition=true" in source

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--out",
            str(tmp_path / "matrix"),
            "--only",
            "packet-loss",
            "--izanami-cmd",
            "true",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "unsupported scenario: packet-loss" in result.stderr


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


def test_stopping_matrix_stays_within_sumeragi_fault_budget(tmp_path: Path) -> None:
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
        "sumeragi_status_delta=Some(SumeragiStatusDigest { status: SumeragiStatus { "
        "protocol_version: 1, config_fingerprint: Hash(00), "
        "beacon_horizon: Some(BeaconHorizonStatusV1 { epoch_length_blocks: 100, "
        "next_required_pulse_height: Some(99), active_session_id: None, "
        "session_covers_next_pulse: false, local_provider_ready: true }), "
        "instance: [7, 7], height: 2, view: 5, stage: 1, leader: None, "
        "proxy_tail: None, high_qc_view: Some(4), level: 2, start_level: 1, "
        "t_retx_ms: 2000, committed_height: 1, applied_height: 0, awaiting: false, "
        "signer: None, unanchored: false, abstaining: false, halted: None, "
        "footprint: SumeragiFootprint { votes: 3, timeouts: 1 } }, "
        "committed_height_advance: 0 })'\n"
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
        "driver-saturated,consensus-stalled,apply-pending",
        "degraded",
    ]
    evidence_lines = (out_dir / "evidence.tsv").read_text().splitlines()
    header = evidence_lines[0].split("\t")
    row = evidence_lines[1].split("\t")
    assert len(row) == len(header)
    evidence = evidence_lines[1]
    assert "status=driver-saturated" in evidence
    assert "driver-saturated,consensus-stalled,apply-pending" in evidence
    assert "\tapply-lag\t" in evidence
    assert row[header.index("protocol_version")] == "1"
    assert row[header.index("height")] == "2"
    assert row[header.index("view")] == "5"
    assert row[header.index("high_qc_view")] == "4"
    assert row[header.index("committed_height")] == "1"
    assert row[header.index("applied_height")] == "0"
    assert row[header.index("halted")] == "none"
    assert "offered_ratio=" in evidence
    assert "accepted_tps=104.25" in evidence
    log = (out_dir / "permissioned-targeted-load.log").read_text()
    assert "--tps 20000" in log
    assert "--max-inflight 20000" in log
    assert "--diagnostic-dir" in log


def test_stress_matrix_rejects_foreign_protocol_status_digest(tmp_path: Path) -> None:
    out_dir = tmp_path / "matrix"
    fake_izanami = tmp_path / "fake_foreign_izanami.sh"
    fake_izanami.write_text(
        "#!/usr/bin/env bash\n"
        "echo '2026-04-29T00:00:00Z INFO izanami::summary: izanami run complete "
        "offered=83399 ingress_accepted=83399 submit_plans_started=83399 "
        "final_quorum_min_height=Some(1) final_strict_min_height=Some(1) "
        "sumeragi_status_delta=Some(SumeragiStatusDigest { status: SumeragiStatus { "
        "protocol_version: 2, height: 2, view: 5, stage: 1, level: 2, "
        "committed_height: 1, applied_height: 0, awaiting: true, halted: None }, "
        "committed_height_advance: 0 })'\n"
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
    assert row[header.index("protocol_version")] == "2"
    assert row[header.index("view")] == ""
    assert row[header.index("awaiting")] == ""


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
