"""Hostile qualification for the native authenticated-tool controller."""

from __future__ import annotations

import os
from pathlib import Path
import resource
import signal
import stat
import subprocess
import sys
import time

import pytest


ROOT = Path(__file__).resolve().parents[2]
SOURCE = (
    ROOT
    / "crates"
    / "iroha_kagami"
    / "src"
    / "bin"
    / "iroha_authenticated_tool_controller.rs"
)
CONTRACT = "iroha.authenticated-tool-os-isolation.v1"


@pytest.fixture(scope="module")
def controller(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Compile the dependency-free controller source as one authenticated file."""

    output = tmp_path_factory.mktemp("authenticated-tool-controller") / "controller"
    subprocess.run(
        [
            "rustc",
            "--edition",
            "2024",
            "-D",
            "warnings",
            "-D",
            "unsafe-code",
            str(SOURCE),
            "-o",
            str(output),
        ],
        cwd=ROOT,
        check=True,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return output.resolve(strict=True)


def exact_environment(temporary: Path) -> dict[str, str]:
    return {
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "TMPDIR": str(temporary),
    }


def controller_command(
    controller: Path,
    *,
    working_directory: Path,
    tool: list[str],
    writable: tuple[str, int] | None = None,
    cumulative_write_limit: int | None = None,
    maximum_live_write_root: int | None = None,
    readable_files: tuple[Path, ...] = (),
    readable_directories: tuple[Path, ...] = (),
    wall_time: str = "2",
    stdout_limit: int = 4096,
    stderr_limit: int = 4096,
) -> list[str]:
    command = [
        str(controller),
        "run-v1",
        "--contract",
        CONTRACT,
        "--platform",
        "macos",
        "--expected-runtime-uid",
        str(os.getuid()),
        "--expected-runtime-gid",
        str(os.getgid()),
        "--working-directory",
        str(working_directory),
        "--use-attested-runtime-identity",
        "--no-new-privileges",
        "--close-inherited-fds",
        "--forward-tool-exit-status",
        "--exact-tool-stdio",
        "--deny-network",
        "--deny-tool-process-spawn",
        "--deny-read-outside-allowlist",
    ]
    if writable is None:
        command.extend(
            [
                "--deny-all-writes",
                "--account-unlinked-write-bytes",
                "--require-empty-process-tree",
                "--cumulative-write-limit-bytes",
                "0",
                "--maximum-live-write-root-bytes",
                "0",
            ]
        )
    else:
        name, maximum = writable
        cumulative = (
            maximum if cumulative_write_limit is None else cumulative_write_limit
        )
        live = maximum if maximum_live_write_root is None else maximum_live_write_root
        command.extend(
            [
                "--deny-write-outside-allowlist",
                "--deny-link-rename-unlink",
                "--deny-symlink",
                "--deny-special-files",
                "--account-unlinked-write-bytes",
                "--require-empty-process-tree",
                "--cumulative-write-limit-bytes",
                str(cumulative),
                "--maximum-live-write-root-bytes",
                str(live),
                "--writable-file",
                f"{name}:{maximum}",
            ]
        )
    for path in readable_files:
        command.extend(("--readable-file", str(path)))
    for path in readable_directories:
        command.extend(("--readable-directory", str(path)))
    command.extend(
        [
            "--wall-time-seconds",
            wall_time,
            "--stdout-limit-bytes",
            str(stdout_limit),
            "--stderr-limit-bytes",
            str(stderr_limit),
            "--",
            *tool,
        ]
    )
    return command


def run_controller(
    controller: Path,
    command: list[str],
    temporary: Path,
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        command,
        cwd="/",
        env=exact_environment(temporary),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=10,
    )


pytestmark = pytest.mark.skipif(
    sys.platform != "darwin" or not Path("/usr/bin/sandbox-exec").is_file(),
    reason="hostile Seatbelt qualification requires macOS sandbox-exec",
)


def test_controller_preserves_exact_output_status_and_allowlisted_write(
    controller: Path, tmp_path: Path
) -> None:
    tmp_path.chmod(0o700)
    output = tmp_path / "allowed"
    command = controller_command(
        controller,
        working_directory=tmp_path.resolve(),
        writable=(output.name, 128),
        tool=[
            str(controller),
            "qualification-probe-v1",
            "success",
            str(output),
        ],
    )
    completed = run_controller(controller, command, tmp_path)
    assert completed.returncode == 0, completed.stderr.decode("utf-8", "replace")
    assert completed.stdout == b"qualified stdout\n"
    assert completed.stderr == b"qualified stderr\n"
    assert output.read_bytes() == b"qualified\n"
    assert stat.S_IMODE(output.stat().st_mode) == 0o600

    create_new = controller_command(
        controller,
        working_directory=tmp_path.resolve(),
        writable=(output.name, 128),
        tool=[
            str(controller),
            "qualification-probe-v1",
            "create-new",
            str(output),
        ],
    )
    incompatible = run_controller(controller, create_new, tmp_path)
    assert incompatible.returncode == 125
    assert b"qualification create-new signer incompatible" in incompatible.stderr

    exit_command = controller_command(
        controller,
        working_directory=Path("/"),
        tool=[str(controller), "qualification-probe-v1", "exit", "17"],
    )
    exited = run_controller(controller, exit_command, tmp_path)
    assert exited.returncode == 17
    assert exited.stdout == b""
    assert exited.stderr == b""

    cumulative_root = tmp_path / "cumulative-quota"
    cumulative_root.mkdir(mode=0o700)
    cumulative_output = cumulative_root / "allowed"
    cumulative_command = controller_command(
        controller,
        working_directory=cumulative_root.resolve(),
        writable=(cumulative_output.name, 128),
        cumulative_write_limit=32,
        maximum_live_write_root=128,
        tool=[
            str(controller),
            "qualification-probe-v1",
            "write-bytes",
            str(cumulative_output),
            "64",
        ],
    )
    cumulative = run_controller(controller, cumulative_command, tmp_path)
    assert cumulative.returncode == 124
    assert b"cumulative write quota was exceeded" in cumulative.stderr

    live_root = tmp_path / "live-quota"
    live_root.mkdir(mode=0o700)
    protected = live_root / "protected"
    protected.write_bytes(b"p" * 100)
    protected.chmod(0o600)
    live_output = live_root / "allowed"
    live_command = controller_command(
        controller,
        working_directory=live_root.resolve(),
        writable=(live_output.name, 128),
        cumulative_write_limit=128,
        maximum_live_write_root=128,
        tool=[
            str(controller),
            "qualification-probe-v1",
            "write-bytes",
            str(live_output),
            "64",
        ],
    )
    live = run_controller(controller, live_command, tmp_path)
    assert live.returncode == 124
    assert b"maximum live write-root quota was exceeded" in live.stderr


def test_controller_allows_only_exact_read_paths(
    controller: Path, tmp_path: Path
) -> None:
    root = tmp_path / "reads"
    root.mkdir(mode=0o700)
    allowed = root / "allowed"
    forbidden = root / "forbidden"
    allowed.write_bytes(b"allowed bytes\n")
    forbidden.write_bytes(b"private bytes\n")
    allowed.chmod(0o600)
    forbidden.chmod(0o600)

    allowed_command = controller_command(
        controller,
        working_directory=Path("/"),
        readable_files=(allowed.resolve(),),
        tool=[str(controller), "qualification-probe-v1", "read", str(allowed)],
    )
    completed = run_controller(controller, allowed_command, tmp_path)
    assert completed.returncode == 0, completed.stderr.decode("utf-8", "replace")
    assert completed.stdout == b"allowed bytes\n"

    forbidden_command = controller_command(
        controller,
        working_directory=Path("/"),
        readable_files=(allowed.resolve(),),
        tool=[str(controller), "qualification-probe-v1", "read", str(forbidden)],
    )
    denied = run_controller(controller, forbidden_command, tmp_path)
    assert denied.returncode == 125
    assert b"qualification read denied" in denied.stderr
    assert b"private bytes" not in denied.stdout
    assert b"private bytes" not in denied.stderr

    metadata_command = controller_command(
        controller,
        working_directory=Path("/"),
        readable_files=(allowed.resolve(),),
        tool=[
            str(controller),
            "qualification-probe-v1",
            "metadata",
            str(forbidden),
        ],
    )
    metadata_denied = run_controller(controller, metadata_command, tmp_path)
    assert metadata_denied.returncode == 125
    assert b"qualification metadata read denied" in metadata_denied.stderr


def test_controller_denies_ambient_system_secret_reads(
    controller: Path, tmp_path: Path
) -> None:
    command = controller_command(
        controller,
        working_directory=Path("/"),
        tool=[
            str(controller),
            "qualification-probe-v1",
            "read",
            "/etc/passwd",
        ],
    )
    denied = run_controller(controller, command, tmp_path)
    assert denied.returncode == 125
    assert denied.stdout == b""
    assert b"qualification read denied" in denied.stderr


def test_controller_rejects_incomplete_environment(
    controller: Path, tmp_path: Path
) -> None:
    command = controller_command(
        controller,
        working_directory=Path("/"),
        tool=[str(controller), "qualification-probe-v1", "exit", "0"],
    )
    environment = exact_environment(tmp_path)
    environment.pop("TMPDIR")
    denied = subprocess.run(
        command,
        cwd="/",
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=10,
    )
    assert denied.returncode == 125
    assert b"environment TMPDIR is missing" in denied.stderr

    mismatched = command.copy()
    uid_index = mismatched.index("--expected-runtime-uid") + 1
    mismatched[uid_index] = str(os.geteuid() + 1)
    identity_denied = run_controller(controller, mismatched, tmp_path)
    assert identity_denied.returncode == 125
    assert b"runtime identity differs from its exact attested credentials" in (
        identity_denied.stderr
    )

    linux = command.copy()
    linux[linux.index("--platform") + 1] = "linux"
    unsupported = run_controller(controller, linux, tmp_path)
    assert unsupported.returncode == 125
    assert b"Linux isolation is unavailable" in unsupported.stderr


def test_controller_rejects_inherited_fd_above_lowered_soft_limit(
    controller: Path, tmp_path: Path
) -> None:
    command = controller_command(
        controller,
        working_directory=Path("/"),
        tool=[str(controller), "qualification-probe-v1", "exit", "0"],
    )
    source = os.open(tmp_path, os.O_RDONLY)
    inherited = 128
    try:
        os.dup2(source, inherited, inheritable=True)

        def lower_descriptor_limit() -> None:
            _, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
            resource.setrlimit(resource.RLIMIT_NOFILE, (4, hard))

        denied = subprocess.run(
            command,
            cwd="/",
            env=exact_environment(tmp_path),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            pass_fds=(inherited,),
            preexec_fn=lower_descriptor_limit,
            check=False,
            timeout=10,
        )
    finally:
        os.close(inherited)
        os.close(source)
    assert denied.returncode == 125
    assert b"controller inherited unexpected descriptor 128" in denied.stderr


@pytest.mark.parametrize("action", ["network", "spawn", "fork", "setsid"])
def test_controller_denies_network_and_process_creation(
    controller: Path, tmp_path: Path, action: str
) -> None:
    command = controller_command(
        controller,
        working_directory=Path("/"),
        tool=[str(controller), "qualification-probe-v1", action],
    )
    completed = run_controller(controller, command, tmp_path)
    assert completed.returncode == 125
    assert f"qualification {action} denied".encode() in completed.stderr


def test_controller_denies_ambient_sysctl_disclosure(
    controller: Path, tmp_path: Path
) -> None:
    command = controller_command(
        controller,
        working_directory=Path("/"),
        tool=[str(controller), "qualification-probe-v1", "ambient-sysctl"],
    )
    completed = run_controller(controller, command, tmp_path)
    assert completed.returncode == 125
    assert b"qualification ambient sysctl denied" in completed.stderr


@pytest.mark.skipif(os.geteuid() == 0, reason="root already has the requested identity")
def test_controller_non_setid_image_cannot_gain_root(
    controller: Path, tmp_path: Path
) -> None:
    command = controller_command(
        controller,
        working_directory=Path("/"),
        tool=[str(controller), "qualification-probe-v1", "setuid-root"],
    )
    completed = run_controller(controller, command, tmp_path)
    assert completed.returncode == 125
    assert b"qualification privilege escalation denied" in completed.stderr


@pytest.mark.parametrize(
    ("action", "diagnostic", "initial", "arguments"),
    [
        ("outside", "write", True, ("write", "other")),
        ("unlink", "unlink", True, ("unlink", "allowed")),
        ("rename", "rename", True, ("rename", "allowed", "other")),
        ("hardlink", "hardlink", True, ("hardlink", "allowed", "other")),
        ("symlink", "symlink", False, ("symlink", "/private/tmp", "allowed")),
        ("fifo", "FIFO", False, ("fifo", "allowed")),
    ],
)
def test_controller_denies_filesystem_escape_classes(
    controller: Path,
    tmp_path: Path,
    action: str,
    diagnostic: str,
    initial: bool,
    arguments: tuple[str, ...],
) -> None:
    root = tmp_path / action
    root.mkdir(mode=0o700)
    allowed = root / "allowed"
    if initial:
        allowed.touch(mode=0o600)
    rendered_arguments = [
        str(root / value) if value in {"allowed", "other"} else value
        for value in arguments
    ]
    command = controller_command(
        controller,
        working_directory=root.resolve(),
        writable=("allowed", 128),
        tool=[
            str(controller),
            "qualification-probe-v1",
            *rendered_arguments,
        ],
    )
    completed = run_controller(controller, command, root)
    assert completed.returncode == 125
    assert f"qualification {diagnostic}".encode() in completed.stderr
    assert not (root / "other").exists()
    assert allowed.is_file() and not allowed.is_symlink()
    assert allowed.stat().st_size == 0


def test_controller_enforces_output_and_wall_time(
    controller: Path, tmp_path: Path
) -> None:
    output_command = controller_command(
        controller,
        working_directory=Path("/"),
        stdout_limit=32,
        tool=[
            str(controller),
            "qualification-probe-v1",
            "stdout-overflow",
            "4096",
        ],
    )
    output = run_controller(controller, output_command, tmp_path)
    assert output.returncode == 124
    assert output.stdout == b""
    assert output.stderr.endswith(b"tool output exceeded its bound\n")

    timeout_command = controller_command(
        controller,
        working_directory=Path("/"),
        wall_time="0.1",
        tool=[str(controller), "qualification-probe-v1", "sleep"],
    )
    started = time.monotonic()
    timeout = run_controller(controller, timeout_command, tmp_path)
    assert time.monotonic() - started < 2
    assert timeout.returncode == 124
    assert timeout.stderr.endswith(b"tool exceeded its wall-time bound\n")


def test_controller_watchdog_cleans_job_after_forced_controller_death(
    controller: Path, tmp_path: Path
) -> None:
    command = controller_command(
        controller,
        working_directory=Path("/"),
        wall_time="60",
        tool=[str(controller), "qualification-probe-v1", "sleep"],
    )
    process = subprocess.Popen(
        command,
        cwd="/",
        env=exact_environment(tmp_path),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    children: set[int] = set()
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline and len(children) < 2:
        rows = subprocess.run(
            ["/bin/ps", "-axo", "pid=,ppid="],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        ).stdout.splitlines()
        children = {
            int(fields[0])
            for row in rows
            if len(fields := row.split()) == 2 and int(fields[1]) == process.pid
        }
        if len(children) < 2:
            time.sleep(0.02)
    assert len(children) == 2
    os.kill(process.pid, signal.SIGKILL)
    process.wait(timeout=2)
    deadline = time.monotonic() + 3
    remaining = set(children)
    while time.monotonic() < deadline and remaining:
        remaining = {
            pid
            for pid in remaining
            if subprocess.run(
                ["/bin/ps", "-p", str(pid), "-o", "pid="],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            ).stdout.strip()
        }
        if remaining:
            time.sleep(0.02)
    assert not remaining
