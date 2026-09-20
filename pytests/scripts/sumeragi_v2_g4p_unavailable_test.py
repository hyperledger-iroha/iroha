"""G-4P refuses missing native qualification before build or publication."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
LAUNCHER = ROOT_DIR / "scripts/run_nexus_cross_dataspace_atomic_swap.sh"
RECREATION = (
    "nexus_autoscale_native_four_peer_recreates_lane_and_rejects_stale_artifacts"
)
RECOVERY = "nexus_autoscale_native_recovers_missing_execution_evidence_after_restart"
UNAVAILABLE = (
    "G-4P unavailable: current-native lane recreation and execution-evidence "
    "restart recovery qualification is not implemented"
)


@pytest.mark.parametrize(
    ("source", "missing"),
    (
        ("", RECREATION),
        (f"#[test]\nfn {RECREATION}() {{}}\n", RECOVERY),
        (f"// fn {RECREATION}() {{}}\n", RECREATION),
        (f"fn {RECREATION}() {{}}\n" * 2, RECREATION),
    ),
)
def test_g4p_launcher_refuses_missing_native_qualification_before_setup(
    tmp_path: Path, source: str, missing: str
) -> None:
    repo = tmp_path / "repo"
    scripts = repo / "scripts"
    sources = repo / "integration_tests/tests/nexus"
    scripts.mkdir(parents=True)
    sources.mkdir(parents=True)
    launcher = scripts / LAUNCHER.name
    shutil.copy2(LAUNCHER, launcher)
    (sources / "autoscale_localnet.rs").write_text(source, encoding="utf-8")
    target = tmp_path / "target"
    evidence = tmp_path / "evidence"
    completion = tmp_path / "completion-pointer"
    env = os.environ.copy()
    env["IROHA_MULTILANE_FOUR_PEER_COMPLETION_PATH_FILE"] = str(completion)

    # No process policy or Cargo executable is installed in this fixture.
    # The qualification refusal must precede all build and artifact setup.
    result = subprocess.run(
        [
            "bash", str(launcher), "--release", "--multilane-four-peer-release",
            "--target-dir", str(target), "--evidence-dir", str(evidence),
        ],
        cwd=repo,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 1
    assert result.stderr.strip() == (
        f"{UNAVAILABLE}: nexus::autoscale_localnet::{missing}"
    )
    assert not result.stdout
    assert not target.exists()
    assert not evidence.exists()
    assert not completion.exists()


def test_current_g4p_source_inventory_reports_unavailable_qualification() -> None:
    result = subprocess.run(
        ["bash", str(ROOT_DIR / "ci/check_sumeragi_v2_multilane_release_inventory.sh")],
        cwd=ROOT_DIR,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 1
    assert f"{UNAVAILABLE}: {RECREATION}" in result.stderr
    assert "found 0" in result.stderr
