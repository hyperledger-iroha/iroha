from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "render_swift_dashboards.sh"


def test_render_without_output_directory_works_under_bash_nounset(
    tmp_path: Path,
) -> None:
    true_bin = shutil.which("true")
    assert true_bin is not None
    env = os.environ.copy()
    env.pop("SWIFT_DASHBOARD_OUTPUT_DIR", None)
    env["SWIFT_BIN"] = true_bin
    env["SWIFT_MODULECACHE_PATH"] = str(tmp_path / "module-cache")

    result = subprocess.run(
        [
            "/bin/bash",
            str(SCRIPT),
            str(REPO_ROOT / "dashboards/data/mobile_parity.sample.json"),
            str(REPO_ROOT / "dashboards/data/mobile_ci.sample.json"),
            str(REPO_ROOT / "dashboards/data/mobile_pipeline_metadata.sample.json"),
        ],
        cwd=REPO_ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "=== Swift Norito Parity ===" in result.stdout
    assert "=== Swift CI Health ===" in result.stdout
    assert "=== Swift Pipeline Metadata ===" in result.stdout


def test_render_with_output_directory_reports_all_summaries(tmp_path: Path) -> None:
    true_bin = shutil.which("true")
    assert true_bin is not None
    output_dir = tmp_path / "output"
    env = os.environ.copy()
    env["SWIFT_BIN"] = true_bin
    env["SWIFT_MODULECACHE_PATH"] = str(tmp_path / "module-cache")
    env["SWIFT_DASHBOARD_OUTPUT_DIR"] = str(output_dir)

    result = subprocess.run(
        ["/bin/bash", str(SCRIPT)],
        cwd=REPO_ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert f"summary written to {output_dir}/mobile_parity.txt" in result.stdout
    assert f"summary written to {output_dir}/mobile_ci.txt" in result.stdout
    assert (
        f"summary written to {output_dir}/mobile_pipeline_metadata.txt"
        in result.stdout
    )
