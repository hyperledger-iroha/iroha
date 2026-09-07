"""Execute formal/chaos directory setup without starting build or proof tools."""

from __future__ import annotations

import os
from pathlib import Path
import re
import shutil
import stat
import subprocess

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]


def _workflow_layout(workflow: str, job: str, step: str) -> str:
    """Read the actual shell setup from one named workflow step."""
    source = (ROOT_DIR / ".github" / "workflows" / workflow).read_text()
    job_match = re.search(
        rf"(?ms)^  {re.escape(job)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        source,
    )
    assert job_match is not None
    step_match = re.search(
        rf"(?m)^        id: {re.escape(step)}\n"
        r"        shell: bash\n        run: \|\n((?:          .*\n)+)",
        job_match.group(1),
    )
    assert step_match is not None
    return "".join(line[10:] for line in step_match.group(1).splitlines(True))


def _launcher_layout() -> str:
    """Read the standalone launcher's default environment setup only."""
    source = (ROOT_DIR / "scripts" / "run_sumeragi_v2_formal_release.sh").read_text()
    start = source.index('if [[ -z "${CARGO_TARGET_DIR:-}"')
    end = source.index('require_external_cargo_target_dir "$repo_root"', start)
    return "set -euo pipefail\n" + source[start:end] + (
        "printf 'invocation_root=%s\\n' \"$formal_invocation_root\" "
        ">\"$GITHUB_OUTPUT\"\n"
        "printf 'CARGO_TARGET_DIR=%s\\n' \"$CARGO_TARGET_DIR\" >\"$GITHUB_ENV\"\n"
        "printf 'IROHA_RELEASE_ARTIFACT_ROOT=%s\\n' \"$IROHA_RELEASE_ARTIFACT_ROOT\" "
        ">>\"$GITHUB_ENV\"\n"
        "printf 'IROHA_RELEASE_CANCEL_REQUEST_PATH=%s\\n' "
        "\"$IROHA_RELEASE_CANCEL_REQUEST_PATH\" >>\"$GITHUB_ENV\"\n"
    )


@pytest.mark.parametrize(
    ("workflow", "job", "step"),
    (
        ("nightly_sumeragi_formal.yml", "sumeragi-v2-formal", "formal_layout"),
        ("nightly_sumeragi_formal.yml", "sumeragi-v2-chaos-100k", "chaos_layout"),
        ("pr.yml", "sumeragi_formal", "formal_layout"),
        (None, None, None),
    ),
    ids=("nightly-formal", "nightly-chaos", "pr-formal", "standalone-formal"),
)
def test_invocation_layout_is_fresh_canonical_private_and_outside_source(
    tmp_path: Path, workflow: str | None, job: str | None, step: str | None
) -> None:
    """Host temp aliases and caller TMPDIR never move evidence into the checkout."""
    if workflow is None:
        script = _launcher_layout()
    else:
        assert job is not None and step is not None
        script = _workflow_layout(workflow, job, step)
    source = tmp_path / "checkout with spaces"
    source.mkdir()
    roots: list[Path] = []
    try:
        for attempt in range(2):
            output = tmp_path / f"output-{attempt}"
            environment = tmp_path / f"environment-{attempt}"
            result = subprocess.run(
                ["bash", "-c", script],
                cwd=source,
                env={
                    "PATH": os.environ["PATH"],
                    "TMPDIR": str(source),
                    "GITHUB_OUTPUT": str(output),
                    "GITHUB_ENV": str(environment),
                },
                capture_output=True,
                text=True,
                check=False,
            )
            assert result.returncode == 0, result.stderr
            fields = dict(
                line.split("=", 1) for line in output.read_text().splitlines()
            )
            root = Path(fields["invocation_root"])
            assert root.is_absolute() and root == root.resolve()
            assert root.parent == Path("/tmp").resolve()
            assert root not in roots
            roots.append(root)
            assert source.resolve() not in root.parents
            for directory in (root, *root.rglob("*")):
                assert directory.is_dir() and not directory.is_symlink()
                metadata = directory.stat()
                assert metadata.st_uid == os.getuid()
                assert stat.S_IMODE(metadata.st_mode) == 0o700
            exported = dict(
                line.split("=", 1) for line in environment.read_text().splitlines()
            )
            for name, path in exported.items():
                assert root in Path(path).parents, name
            assert exported["CARGO_TARGET_DIR"] == str(root / "target")
            assert exported["IROHA_RELEASE_ARTIFACT_ROOT"] == str(root / "artifacts")
            assert exported["IROHA_RELEASE_CANCEL_REQUEST_PATH"] == str(
                root / "cancel-request.json"
            )
    finally:
        for root in roots:
            shutil.rmtree(root)
