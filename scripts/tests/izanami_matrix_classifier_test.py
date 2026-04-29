from pathlib import Path
import re
import subprocess


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run_izanami_communication_vulnerability_matrix.sh"


def _classifier_degraded_pattern() -> str:
    source = SCRIPT.read_text()
    match = re.search(r"^acceptance_failure_regex='([^']+)'", source, re.MULTILINE)
    assert match is not None
    return match.group(1)


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
    assert "submit_latency_p95_ms" in evidence.read_text().splitlines()[0]

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
    assert "submit_latency_p95_ms" in evidence.read_text().splitlines()[0]


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
