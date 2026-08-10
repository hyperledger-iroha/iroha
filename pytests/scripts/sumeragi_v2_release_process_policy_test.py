from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


REPO_ROOT = Path(__file__).resolve().parents[2]
POLICY = REPO_ROOT / "scripts" / "sumeragi_v2_release_process_policy.sh"


def _write_executable(path: Path, source: str) -> None:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o700)


def _policy_environment(fake_bin: Path, **extra: str) -> dict[str, str]:
    environment = os.environ.copy()
    environment.update(
        {
            "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
            "IROHA_RELEASE_POLICY_PYTHON": sys.executable,
        }
    )
    environment.update(extra)
    return environment


def _run_policy(
    command: str,
    *,
    fake_bin: Path,
    environment: dict[str, str] | None = None,
    shell: str = "bash",
) -> subprocess.CompletedProcess[str]:
    merged = _policy_environment(fake_bin)
    if environment:
        merged.update(environment)
    return subprocess.run(
        [shell, "-c", f'source "{POLICY}"\n{command}'],
        cwd=REPO_ROOT,
        env=merged,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def test_wait_classifies_rustfmt_from_the_same_printed_snapshot(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    count_path = tmp_path / "ps-count"
    _write_executable(
        fake_bin / "ps",
        """#!/bin/sh
count=0
if [ -f "$PROCESS_SNAPSHOT_COUNT_FILE" ]; then
  IFS= read -r count < "$PROCESS_SNAPSHOT_COUNT_FILE"
fi
count=$((count + 1))
printf '%s\n' "$count" > "$PROCESS_SNAPSHOT_COUNT_FILE"
printf '%s\n' '  PID ELAPSED COMMAND'
if [ "$count" -eq 1 ]; then
  printf '%s\n' '   42   00:01 /toolchain/bin/rustfmt --edition 2024'
fi
""",
    )
    _write_executable(fake_bin / "sleep", "#!/bin/sh\nexit 0\n")

    result = _run_policy(
        "wait_for_external_cargo test-boundary",
        fake_bin=fake_bin,
        environment={"PROCESS_SNAPSHOT_COUNT_FILE": str(count_path)},
    )

    assert result.returncode == 0, result.stderr
    assert count_path.read_text(encoding="utf-8") == "2\n", (
        result.stdout,
        result.stderr,
    )
    assert result.stdout == ""
    assert result.stderr.count("PID ELAPSED COMMAND") == 2
    assert "rustfmt" in result.stderr


def test_run_cargo_pins_toolchain_jobs_and_locked_offline_flags(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    argv_path = tmp_path / "cargo-argv"
    rustup_policy_path = tmp_path / "rustup-auto-install"
    _write_executable(
        fake_bin / "ps",
        "#!/bin/sh\nprintf '%s\\n' '  PID ELAPSED COMMAND'\n",
    )
    _write_executable(
        fake_bin / "cargo",
        "#!/bin/sh\n"
        "printf '%s\\n' \"$@\" > \"$CARGO_ARGV_FILE\"\n"
        "printf '%s\\n' \"$RUSTUP_AUTO_INSTALL\" > \"$RUSTUP_POLICY_FILE\"\n",
    )

    result = _run_policy(
        "run_cargo test --locked --offline -p iroha_core --lib",
        fake_bin=fake_bin,
        environment={
            "CARGO_ARGV_FILE": str(argv_path),
            "RUSTUP_POLICY_FILE": str(rustup_policy_path),
        },
    )

    assert result.returncode == 0, result.stderr
    assert argv_path.read_text(encoding="utf-8").splitlines() == [
        "+1.93.1",
        "test",
        "-j1",
        "--locked",
        "--offline",
        "-p",
        "iroha_core",
        "--lib",
    ]
    assert rustup_policy_path.read_text(encoding="utf-8") == "0\n"


def test_run_cargo_preserves_nonzero_status_when_sourced_by_zsh(tmp_path: Path) -> None:
    zsh = shutil.which("zsh")
    if zsh is None:
        return

    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    argv_path = tmp_path / "cargo-argv"
    _write_executable(
        fake_bin / "ps",
        "#!/bin/sh\nprintf '%s\\n' '  PID ELAPSED COMMAND'\n",
    )
    _write_executable(
        fake_bin / "cargo",
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CARGO_ARGV_FILE\"\nexit 7\n",
    )

    result = _run_policy(
        "run_cargo test --locked --offline -p iroha_core --lib",
        fake_bin=fake_bin,
        environment={"CARGO_ARGV_FILE": str(argv_path)},
        shell=zsh,
    )

    assert result.returncode == 7, result.stderr
    assert "read-only variable: status" not in result.stderr
    assert argv_path.read_text(encoding="utf-8").splitlines()[:3] == [
        "+1.93.1",
        "test",
        "-j1",
    ]


def test_run_cargo_rejects_missing_or_caller_owned_policy_flags(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_executable(
        fake_bin / "ps",
        "#!/bin/sh\nprintf '%s\\n' '  PID ELAPSED COMMAND'\n",
    )
    _write_executable(fake_bin / "cargo", "#!/bin/sh\nexit 99\n")

    missing = _run_policy(
        "run_cargo test --locked -p iroha_core",
        fake_bin=fake_bin,
    )
    caller_jobs = _run_policy(
        "run_cargo test --locked --offline -j2 -p iroha_core",
        fake_bin=fake_bin,
    )
    fetch = _run_policy(
        "run_cargo fetch --locked --offline",
        fake_bin=fake_bin,
    )
    rustup_override = _run_policy(
        "run_cargo test --locked --offline -p iroha_core",
        fake_bin=fake_bin,
        environment={"RUSTUP_AUTO_INSTALL": "1"},
    )

    assert missing.returncode == 2
    assert "exactly one --locked and one --offline" in missing.stderr
    assert caller_jobs.returncode == 2
    assert "caller job flags are forbidden" in caller_jobs.stderr
    assert fetch.returncode == 2
    assert "rejects unsupported Cargo subcommand: fetch" in fetch.stderr
    assert rustup_override.returncode == 2
    assert "caller-owned rustup auto-install policy" in rustup_override.stderr


def test_cooperative_marker_is_checked_only_between_commands(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    marker = tmp_path / "cancel.json"
    marker.write_bytes(b'{"reason":"operator-request","schema_version":1}\n')
    marker.chmod(0o600)

    result = _run_policy(
        "release_gate_boundary focused-tests",
        fake_bin=fake_bin,
        environment={"IROHA_RELEASE_CANCEL_REQUEST_PATH": str(marker)},
        shell=shutil.which("zsh") or "bash",
    )

    assert result.returncode == 125
    assert "cooperative cancellation requested" in result.stderr


def test_malformed_or_non_private_marker_fails_closed(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    marker = tmp_path / "cancel.json"
    marker.write_text('{"schema_version":1}\n', encoding="utf-8")
    marker.chmod(0o600)
    malformed = _run_policy(
        "release_gate_boundary focused-tests",
        fake_bin=fake_bin,
        environment={"IROHA_RELEASE_CANCEL_REQUEST_PATH": str(marker)},
    )

    marker.write_bytes(b'{"reason":"operator-request","schema_version":1}\n')
    marker.chmod(0o644)
    public = _run_policy(
        "release_gate_boundary focused-tests",
        fake_bin=fake_bin,
        environment={"IROHA_RELEASE_CANCEL_REQUEST_PATH": str(marker)},
    )

    assert malformed.returncode == 2
    assert public.returncode == 2


def test_marker_parent_must_be_private(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    marker_parent = tmp_path / "public"
    marker_parent.mkdir(mode=0o700)
    marker = marker_parent / "cancel.json"
    marker.write_bytes(b'{"reason":"operator-request","schema_version":1}\n')
    marker.chmod(0o600)
    marker_parent.chmod(0o755)

    result = _run_policy(
        "release_gate_boundary focused-tests",
        fake_bin=fake_bin,
        environment={"IROHA_RELEASE_CANCEL_REQUEST_PATH": str(marker)},
    )

    assert result.returncode == 2
    assert "parent is not one canonical private owner directory" in result.stderr


def test_cargo_target_must_be_private_external_and_under_private_tmp(
    tmp_path: Path,
) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    with tempfile.TemporaryDirectory(
        prefix="iroha-policy-target-", dir="/private/tmp"
    ) as temporary_root:
        target = Path(temporary_root) / "target"
        target.mkdir(mode=0o700)
        accepted = _run_policy(
            f'require_external_cargo_target_dir "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={"CARGO_TARGET_DIR": str(target)},
        )
    rejected = _run_policy(
        f'require_external_cargo_target_dir "{REPO_ROOT}"',
        fake_bin=fake_bin,
        environment={"CARGO_TARGET_DIR": str(REPO_ROOT / "target")},
    )

    assert accepted.returncode == 0, accepted.stderr
    assert rejected.returncode == 2


def test_artifact_root_must_be_private_external_and_under_private_tmp(
    tmp_path: Path,
) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    with tempfile.TemporaryDirectory(
        prefix="iroha-policy-artifacts-", dir="/private/tmp"
    ) as temporary_root:
        invocation_root = Path(temporary_root)
        artifacts = invocation_root / "artifacts"
        target = invocation_root / "target"
        fixture_source = invocation_root / "source"
        artifacts.mkdir(mode=0o700)
        target.mkdir(mode=0o700)
        fixture_source.mkdir(mode=0o700)
        cancel = invocation_root / "cancel-request.json"
        cancel.write_bytes(b'{"reason":"operator-request","schema_version":1}\n')
        cancel.chmod(0o600)
        accepted = _run_policy(
            f'require_external_release_artifact_root "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={"IROHA_RELEASE_ARTIFACT_ROOT": str(artifacts)},
        )
        disjoint = _run_policy(
            f'require_disjoint_release_roots "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={
                "CARGO_TARGET_DIR": str(target),
                "IROHA_RELEASE_ARTIFACT_ROOT": str(artifacts),
                "IROHA_RELEASE_CANCEL_REQUEST_PATH": str(cancel),
            },
        )
        nested_artifacts = target / "nested-artifacts"
        nested_artifacts.mkdir(mode=0o700)
        nested = _run_policy(
            f'require_disjoint_release_roots "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={
                "CARGO_TARGET_DIR": str(target),
                "IROHA_RELEASE_ARTIFACT_ROOT": str(nested_artifacts),
                "IROHA_RELEASE_CANCEL_REQUEST_PATH": str(cancel),
            },
        )
        nested_target_cancel = _run_policy(
            f'require_disjoint_release_roots "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={
                "CARGO_TARGET_DIR": str(target),
                "IROHA_RELEASE_ARTIFACT_ROOT": str(artifacts),
                "IROHA_RELEASE_CANCEL_REQUEST_PATH": str(
                    target / "cancel-request.json"
                ),
            },
        )
        nested_artifact_cancel = _run_policy(
            f'require_disjoint_release_roots "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={
                "CARGO_TARGET_DIR": str(target),
                "IROHA_RELEASE_ARTIFACT_ROOT": str(artifacts),
                "IROHA_RELEASE_CANCEL_REQUEST_PATH": str(
                    artifacts / "cancel-request.json"
                ),
            },
        )
        nested_source_cancel = _run_policy(
            f'require_disjoint_release_roots "{fixture_source}"',
            fake_bin=fake_bin,
            environment={
                "CARGO_TARGET_DIR": str(target),
                "IROHA_RELEASE_ARTIFACT_ROOT": str(artifacts),
                "IROHA_RELEASE_CANCEL_REQUEST_PATH": str(
                    fixture_source / "cancel-request.json"
                ),
            },
        )
        noncanonical_parent = invocation_root / "nested"
        noncanonical_parent.mkdir(mode=0o700)
        noncanonical_cancel = _run_policy(
            f'require_disjoint_release_roots "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={
                "CARGO_TARGET_DIR": str(target),
                "IROHA_RELEASE_ARTIFACT_ROOT": str(artifacts),
                "IROHA_RELEASE_CANCEL_REQUEST_PATH": str(
                    noncanonical_parent / ".." / "cancel-request.json"
                ),
            },
        )
        relative_cancel = _run_policy(
            f'require_disjoint_release_roots "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={
                "CARGO_TARGET_DIR": str(target),
                "IROHA_RELEASE_ARTIFACT_ROOT": str(artifacts),
                "IROHA_RELEASE_CANCEL_REQUEST_PATH": "cancel-request.json",
            },
        )
        preserved_cancel = cancel.read_bytes()
    rejected = _run_policy(
        f'require_external_release_artifact_root "{REPO_ROOT}"',
        fake_bin=fake_bin,
        environment={"IROHA_RELEASE_ARTIFACT_ROOT": str(REPO_ROOT / "target")},
    )

    assert accepted.returncode == 0, accepted.stderr
    assert disjoint.returncode == 0, disjoint.stderr
    assert preserved_cancel == b'{"reason":"operator-request","schema_version":1}\n'
    assert nested.returncode == 2
    assert "must be disjoint" in nested.stderr
    for rejected_cancel in (
        nested_target_cancel,
        nested_artifact_cancel,
        nested_source_cancel,
    ):
        assert rejected_cancel.returncode == 2
        assert "outside source" in rejected_cancel.stderr
    assert noncanonical_cancel.returncode == 2
    assert "canonical private owner directory" in noncanonical_cancel.stderr
    assert relative_cancel.returncode == 2
    assert "must be absolute" in relative_cancel.stderr
    assert rejected.returncode == 2


def test_artifact_directory_must_remain_below_authenticated_root() -> None:
    with tempfile.TemporaryDirectory(
        prefix="iroha-policy-artifact-tree-", dir="/private/tmp"
    ) as temporary_root:
        root = Path(temporary_root) / "artifacts"
        root.mkdir(mode=0o700)
        nested = root / "run"
        nested.mkdir()
        fake_bin = root / "fake-bin"
        fake_bin.mkdir()
        future = root / "future" / "invocation"
        accepted_future = _run_policy(
            f'require_release_artifact_path "{future}"',
            fake_bin=fake_bin,
            environment={"IROHA_RELEASE_ARTIFACT_ROOT": str(root)},
        )
        accepted = _run_policy(
            f'require_release_artifact_directory "{nested}"',
            fake_bin=fake_bin,
            environment={"IROHA_RELEASE_ARTIFACT_ROOT": str(root)},
        )
        rejected = _run_policy(
            f'require_release_artifact_directory "{REPO_ROOT}"',
            fake_bin=fake_bin,
            environment={"IROHA_RELEASE_ARTIFACT_ROOT": str(root)},
        )
        escaped_future = REPO_ROOT / "future"
        rejected_future = _run_policy(
            f'require_release_artifact_path "{escaped_future}"',
            fake_bin=fake_bin,
            environment={"IROHA_RELEASE_ARTIFACT_ROOT": str(root)},
        )

    assert accepted_future.returncode == 0, accepted_future.stderr
    assert accepted.returncode == 0, accepted.stderr
    assert rejected.returncode == 2
    assert rejected_future.returncode == 2


def test_policy_source_contains_no_process_control_escape_hatch() -> None:
    source = POLICY.read_text(encoding="utf-8")
    forbidden = (
        "SIGSTOP",
        "SIGTERM",
        "SIGKILL",
        "killpg",
        "pkill",
        "renice",
        "start_new_session",
        ".terminate(",
        ".kill(",
    )
    assert all(token not in source for token in forbidden)
    assert source.count("ps -axo pid,etime,command") == 1
    assert 'executable == "rustfmt"' in source
    assert source.count('command cargo +1.93.1 "${pinned_arguments[@]}"') == 1
    assert 'pinned_arguments=("$subcommand" -j1)' in source
    assert 'pinned_arguments+=("$@")' in source
    assert "local status" not in source
    assert "build|test|run|clippy|verus)" in source
    assert "build|test|run|clippy|verus|fetch)" not in source
