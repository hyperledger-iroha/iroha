"""Focused fake-process contracts for the two Nexus PR helper scripts."""

from __future__ import annotations

import atexit
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


ROOT_DIR = Path(__file__).resolve().parents[2]
LOCALNET_HELPER = ROOT_DIR / "ci" / "check_nexus_cross_dataspace_localnet.sh"
LANE_HELPER = ROOT_DIR / "ci" / "check_nexus_cross_lane_proofs.sh"
NEXUS_LAUNCHER = ROOT_DIR / "scripts" / "run_nexus_cross_dataspace_atomic_swap.sh"
PROCESS_POLICY = ROOT_DIR / "scripts" / "sumeragi_v2_release_process_policy.sh"
PREBUILT_SHELL = ROOT_DIR / "scripts" / "sumeragi_v2_prebuilt_bundle.sh"
SOURCE_MANIFEST = "a" * 64
PREBUILT_MANIFEST = "b" * 64
_EXTERNAL_ROOTS: list[Path] = []


@atexit.register
def _cleanup_external_roots() -> None:
    for root in _EXTERNAL_ROOTS:
        shutil.rmtree(root, ignore_errors=True)


def _write_executable(path: Path, source: str) -> None:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o700)


def _fixture(
    tmp_path: Path,
    helper: Path,
) -> tuple[Path, dict[str, str], Path, Path]:
    repo = tmp_path / "repo"
    ci = repo / "ci"
    scripts = repo / "scripts"
    fake_bin = tmp_path / "bin"
    ci.mkdir(parents=True)
    scripts.mkdir()
    fake_bin.mkdir()
    copied_helper = ci / helper.name
    shutil.copy2(helper, copied_helper)
    shutil.copy2(PROCESS_POLICY, scripts / PROCESS_POLICY.name)
    shutil.copy2(PREBUILT_SHELL, scripts / PREBUILT_SHELL.name)
    _write_executable(
        ci / "check_sumeragi_v2_multilane_release_inventory.sh",
        "#!/bin/sh\nexit 0\n",
    )

    external_root = Path(
        tempfile.mkdtemp(prefix="iroha-nexus-pr-helper-test-", dir="/private/tmp")
    )
    _EXTERNAL_ROOTS.append(external_root)
    target = external_root / "target"
    artifacts = external_root / "artifacts"
    target.mkdir(mode=0o700)
    artifacts.mkdir(mode=0o700)
    bundle = (
        artifacts
        / "sumeragi-v2-release"
        / SOURCE_MANIFEST
        / "programs"
        / "invocation.fixture"
    )

    python = fake_bin / "python3"
    _write_executable(
        python,
        f"""#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" == *compute_workspace_source_manifest.py* ]]; then
  printf '%s\n' '{SOURCE_MANIFEST}'
  exit 0
fi
if [[ "$*" == *sumeragi_v2_prebuilt_bundle.py* ]]; then
  exit 0
fi
exec "$NEXUS_HELPER_REAL_PYTHON3" "$@"
""",
    )
    _write_executable(
        fake_bin / "ps",
        "#!/bin/sh\nprintf '%s\n' '  PID ELAPSED COMMAND'\n",
    )

    capture = tmp_path / "capture.tsv"
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{fake_bin}{os.pathsep}{env['PATH']}",
            "NEXUS_HELPER_REAL_PYTHON3": sys.executable,
            "IROHA_RELEASE_POLICY_PYTHON": sys.executable,
            "CARGO_TARGET_DIR": str(target),
            "IROHA_RELEASE_ARTIFACT_ROOT": str(artifacts),
            "IROHA_RELEASE_CANCEL_REQUEST_PATH": str(
                external_root / "cancel-request.json"
            ),
            "IROHA_RELEASE_SOURCE_MANIFEST_SHA256": SOURCE_MANIFEST,
            "IROHA_RELEASE_PREBUILT_MANIFEST_SHA256": PREBUILT_MANIFEST,
            "IROHA_TEST_TARGET_DIR": str(bundle),
            "NEXUS_HELPER_CAPTURE": str(capture),
        }
    )
    return copied_helper, env, capture, bundle


def _run(helper: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(helper)],
        cwd=helper.parents[1],
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )


def _job(source: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        source,
    )
    assert match is not None, name
    return match.group("body")


def test_nexus_pr_helpers_and_jobs_are_pinned_to_shared_policy() -> None:
    localnet = LOCALNET_HELPER.read_text(encoding="utf-8")
    lane = LANE_HELPER.read_text(encoding="utf-8")
    launcher = NEXUS_LAUNCHER.read_text(encoding="utf-8")
    workflow = (ROOT_DIR / ".github" / "workflows" / "pr.yml").read_text(
        encoding="utf-8"
    )

    for source in (localnet, lane):
        assert "sumeragi_v2_release_process_policy.sh" in source
        assert "sumeragi_v2_prebuilt_bundle.sh" in source
        assert source.count("require_disjoint_release_roots") == 1
        assert "must be supplied all-or-none" in source
        assert "export IROHA_TEST_SKIP_BUILD=1" in source
        assert "export IROHA_TEST_ALLOW_REENTRANT_BUILD=0" in source
        assert "--no-skip-build" not in source
        assert "IROHA_TEST_ALLOW_REENTRANT_BUILD=1" not in source
        assert re.search(r"(?m)^\s*(?:command\s+)?cargo(?:\s|$)", source) is None
    assert lane.count("run_cargo test --locked --offline") == 4
    extras_end = launcher.index("done\n# Test processes must consume")
    for assignment in (
        'ENV_VARS+=("IROHA_TEST_SKIP_BUILD=1")',
        'ENV_VARS+=("IROHA_TEST_ALLOW_REENTRANT_BUILD=0")',
    ):
        assert launcher.count(assignment) == 1
        assert extras_end < launcher.index(assignment)
    assert "--no-skip-build" not in launcher
    assert re.search(r"(?m)^SKIP_BUILD=", launcher) is None
    for arguments in (
        ("--no-skip-build",),
        ("--env", "IROHA_TEST_SKIP_BUILD=0"),
        ("--env", "IROHA_TEST_ALLOW_REENTRANT_BUILD=1"),
    ):
        rejected = subprocess.run(
            ["bash", str(NEXUS_LAUNCHER), *arguments],
            cwd=ROOT_DIR,
            check=False,
            capture_output=True,
            text=True,
        )
        assert rejected.returncode == 2

    for name in ("nexus_cross_dataspace_localnet", "nexus_cross_lane_proofs"):
        job = _job(workflow, name)
        assert "timeout-minutes:" not in job
        assert "uses: Swatinem/rust-cache@" not in job
        assert "uses: actions-rust-lang/setup-rust-toolchain@" not in job


def test_cross_dataspace_helper_reuses_attested_prebuilt_bundle(
    tmp_path: Path,
) -> None:
    helper, env, capture, bundle = _fixture(tmp_path, LOCALNET_HELPER)
    launcher = helper.parents[1] / "scripts" / "run_nexus_cross_dataspace_atomic_swap.sh"
    _write_executable(
        launcher,
        """#!/usr/bin/env bash
set -euo pipefail
printf 'args\t%s\n' "$*" >"$NEXUS_HELPER_CAPTURE"
printf 'skip\t%s\n' "$IROHA_TEST_SKIP_BUILD" >>"$NEXUS_HELPER_CAPTURE"
printf 'reentrant\t%s\n' "$IROHA_TEST_ALLOW_REENTRANT_BUILD" >>"$NEXUS_HELPER_CAPTURE"
printf 'target\t%s\n' "$CARGO_TARGET_DIR" >>"$NEXUS_HELPER_CAPTURE"
printf 'artifacts\t%s\n' "$IROHA_RELEASE_ARTIFACT_ROOT" >>"$NEXUS_HELPER_CAPTURE"
printf 'irohad\t%s\n' "$TEST_NETWORK_BIN_IROHAD" >>"$NEXUS_HELPER_CAPTURE"
""",
    )

    partial_env = env.copy()
    partial_env.pop("IROHA_RELEASE_ARTIFACT_ROOT")
    partial_env.pop("IROHA_RELEASE_CANCEL_REQUEST_PATH")
    partial = _run(helper, partial_env)
    assert partial.returncode == 2
    assert "must be supplied all-or-none" in partial.stderr
    assert not capture.exists()

    nested_artifacts = Path(env["CARGO_TARGET_DIR"]) / "nested-artifacts"
    nested_artifacts.mkdir(mode=0o700)
    nested_env = env.copy()
    nested_env["IROHA_RELEASE_ARTIFACT_ROOT"] = str(nested_artifacts)
    nested = _run(helper, nested_env)
    assert nested.returncode == 2
    assert "must be disjoint" in nested.stderr
    assert not capture.exists()

    result = _run(helper, env)

    assert result.returncode == 0, result.stderr
    fields = dict(
        line.split("\t", 1)
        for line in capture.read_text(encoding="utf-8").splitlines()
    )
    assert "--capture" in fields["args"]
    assert "--no-skip-build" not in fields["args"]
    assert "--env IROHA_TEST_ALLOW_REENTRANT_BUILD=0" not in fields["args"]
    assert fields["skip"] == "1"
    assert fields["reentrant"] == "0"
    assert fields["target"] == env["CARGO_TARGET_DIR"]
    assert fields["artifacts"] == env["IROHA_RELEASE_ARTIFACT_ROOT"]
    assert fields["irohad"] == str(bundle / "release" / "iroha3d")


def test_cross_lane_helper_routes_all_filters_through_pinned_cargo(
    tmp_path: Path,
) -> None:
    helper, env, capture, bundle = _fixture(tmp_path, LANE_HELPER)
    cargo = Path(env["PATH"].split(os.pathsep, 1)[0]) / "cargo"
    _write_executable(
        cargo,
        """#!/usr/bin/env bash
set -euo pipefail
printf '%s\t%s\t%s\t%s\n' \
  "$*" \
  "${IROHA_TEST_SKIP_BUILD-<unset>}" \
  "${IROHA_TEST_ALLOW_REENTRANT_BUILD-<unset>}" \
  "${TEST_NETWORK_BIN_IROHAD-<unset>}" \
  >>"$NEXUS_HELPER_CAPTURE"
""",
    )

    result = _run(helper, env)

    assert result.returncode == 0, result.stderr
    rows = [
        line.split("\t")
        for line in capture.read_text(encoding="utf-8").splitlines()
    ]
    assert len(rows) == 4
    assert all(
        row[0].startswith("+1.93.1 test -j1 --locked --offline ")
        for row in rows
    )
    assert [
        "batch_verification_",
        "get_sumeragi_status_wire_rejects_",
        "get_cross_lane_transfer_proofs_",
        "sumeragi_status_json_endpoint_decodes_to_wire_end_to_end",
    ] == [
        next(filter_name for filter_name in expected if filter_name in row[0])
        for row, expected in zip(
            rows,
            (
                ("batch_verification_",),
                ("get_sumeragi_status_wire_rejects_",),
                ("get_cross_lane_transfer_proofs_",),
                ("sumeragi_status_json_endpoint_decodes_to_wire_end_to_end",),
            ),
        )
    ]
    assert all(row[1:3] == ["1", "0"] for row in rows)
    assert all(row[3] == str(bundle / "release" / "iroha3d") for row in rows)
