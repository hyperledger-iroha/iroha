"""Real owned verifier processes and descriptor-bound private executable snapshots."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat
import sys
import time

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_verifier_process as PROCESS


@pytest.fixture
def private_root(tmp_path: Path) -> Path:
    root = tmp_path.resolve()
    root.chmod(0o700)
    return root


def run(root: Path, code: str, limit: int = 0, *, expected_stderr: bytes) -> bytes:
    return PROCESS.run_verifier(
        [sys.executable, "-c", code], root,
        max_stdout_bytes=limit, expected_stderr=expected_stderr,
    )


def test_exact_stdout_limit_and_silent_success(private_root: Path) -> None:
    assert run(private_root, "pass", expected_stderr=b"") == b""
    assert run(private_root, "import os; os.write(1, b'{}')", 2, expected_stderr=b"") == b"{}"
    assert run(private_root, "import os; os.write(1, b'x'*16384)", 16384, expected_stderr=b"") == b"x" * 16384


@pytest.mark.parametrize("frame", [b"Verified OK\n", b"x" * 16384])
def test_exact_stderr_frame_is_verified_without_becoming_stdout(
    private_root: Path, frame: bytes,
) -> None:
    code = f"import os; os.write(2, {frame!r}); os.write(1, b'ok')"
    assert run(private_root, code, 2, expected_stderr=frame) == b"ok"


def test_expected_stderr_accepts_arbitrary_read_boundaries(
    private_root: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    original_read = PROCESS.os.read
    nonempty_reads = 0

    def fragmented_read(descriptor: int, size: int) -> bytes:
        nonlocal nonempty_reads
        chunk = original_read(descriptor, min(size, 2))
        nonempty_reads += bool(chunk)
        return chunk

    monkeypatch.setattr(PROCESS.os, "read", fragmented_read)
    assert run(
        private_root, "import os; os.write(2, b'Verified OK\\n')",
        expected_stderr=b"Verified OK\n",
    ) == b""
    assert nonempty_reads >= 6


@pytest.mark.parametrize("actual", [
    b"", b"Verified", b"Verified OK\nextra-secret", b"VerifXed OK\n",
    b"Verified OK\nVerified OK\n", b"Verified OK\r\n", b"\xffsecret",
])
def test_stderr_requires_the_complete_exact_frame_without_leaking(
    private_root: Path, actual: bytes, capfd: pytest.CaptureFixture[str],
) -> None:
    with pytest.raises(ValueError, match="^verifier output rejected$"):
        run(
            private_root, f"import os; os.write(2, {actual!r})",
            expected_stderr=b"Verified OK\n",
        )
    captured = capfd.readouterr()
    assert captured.out == captured.err == ""


@pytest.mark.parametrize("exit_code", [1, 7])
def test_matching_stderr_never_accepts_nonzero_exit(
    private_root: Path, exit_code: int,
) -> None:
    with pytest.raises(ValueError, match="^verifier output rejected$"):
        run(
            private_root,
            f"import os; os.write(2, b'Verified OK\\n'); raise SystemExit({exit_code})",
            expected_stderr=b"Verified OK\n",
        )


def test_matching_stderr_keeps_zero_stdout_required(private_root: Path) -> None:
    with pytest.raises(ValueError, match="^verifier output rejected$"):
        run(
            private_root, "import os; os.write(2, b'Verified OK\\n'); os.write(1, b'secret')",
            expected_stderr=b"Verified OK\n",
        )


@pytest.mark.parametrize("frame", [None, "Verified OK\n", bytearray(b"ok"), True, b"x" * 16385])
def test_expected_stderr_is_bounded_bytes_before_process_creation(
    private_root: Path, monkeypatch: pytest.MonkeyPatch, frame: object,
) -> None:
    def forbidden(*args, **kwargs):
        raise AssertionError("invalid stderr frame reached process creation")

    monkeypatch.setattr(PROCESS.subprocess, "Popen", forbidden)
    with pytest.raises(ValueError, match="^invalid verifier invocation$"):
        run(private_root, "pass", expected_stderr=frame)


def test_expected_stderr_keyword_has_no_compatibility_default(
    private_root: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    def forbidden(*args, **kwargs):
        raise AssertionError("missing stderr frame reached process creation")

    monkeypatch.setattr(PROCESS.subprocess, "Popen", forbidden)
    with pytest.raises(TypeError, match="expected_stderr"):
        PROCESS.run_verifier([sys.executable, "-c", "pass"], private_root, max_stdout_bytes=0)


@pytest.mark.parametrize("code,limit", [
    ("import os; os.write(1,b'sensitive-stdout')", 0),
    ("import os; os.write(1,b'sensitive-stdout')", 3),
    ("import os; os.write(2,b'sensitive-stderr')", 16384),
    ("raise SystemExit(7)", 16384),
])
def test_rejected_output_is_never_in_diagnostics(private_root: Path, code: str, limit: int) -> None:
    with pytest.raises(ValueError, match="^verifier output rejected$"):
        run(private_root, code, limit, expected_stderr=b"")


def test_stdout_cap_is_absolute_and_checked_before_start(private_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    def forbidden(*args, **kwargs):
        raise AssertionError("invalid invocation reached process creation")
    monkeypatch.setattr(PROCESS.subprocess, "Popen", forbidden)
    for limit in [-1, True, 16385, "1"]:
        with pytest.raises(ValueError, match="^invalid verifier invocation$"):
            run(private_root, "pass", limit, expected_stderr=b"")


def test_environment_stdin_cwd_and_umask_are_fixed(private_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SENSITIVE_VERIFIER_TEST_SECRET", "never-forward")
    code = (
        "import os,sys; from pathlib import Path; "
        "assert 'SENSITIVE_VERIFIER_TEST_SECRET' not in os.environ; "
        "assert os.environ['PATH']==os.defpath; assert sys.stdin.read()==''; "
        "Path('private-result').write_bytes(b'x'); os.write(1,b'ok')"
    )
    assert run(private_root, code, 2, expected_stderr=b"") == b"ok"
    assert stat.S_IMODE((private_root / "private-result").stat().st_mode) == 0o600


def test_silent_timeout_reaps_owned_process(private_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    pid_path = private_root / "pid"
    monkeypatch.setattr(PROCESS, "VERIFIER_TIMEOUT_SECS", 0.2)
    start = time.monotonic()
    with pytest.raises(OSError, match="^verifier process unavailable$"):
        run(private_root, "import os,time; from pathlib import Path; Path('pid').write_text(str(os.getpid())); time.sleep(60)", expected_stderr=b"")
    assert time.monotonic() - start < 2
    with pytest.raises(ProcessLookupError):
        os.kill(int(pid_path.read_text()), 0)


@pytest.mark.parametrize("inherit_pipes", [True, False])
def test_left_descendants_are_rejected_and_owned_group_is_reaped(private_root: Path, monkeypatch: pytest.MonkeyPatch, inherit_pipes: bool) -> None:
    monkeypatch.setattr(PROCESS, "VERIFIER_TIMEOUT_SECS", 0.4)
    redirect = "" if inherit_pipes else ", stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL"
    code = (
        "import os,sys,subprocess; from pathlib import Path; "
        "child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']"
        + redirect + "); Path('pids').write_text(f'{os.getpgrp()} {child.pid}')"
    )
    with pytest.raises(OSError, match="^verifier process unavailable$"):
        run(private_root, code, expected_stderr=b"")
    group, child = map(int, (private_root / "pids").read_text().split())
    for _ in range(100):
        try:
            os.killpg(group, 0)
        except ProcessLookupError:
            break
        time.sleep(0.01)
    with pytest.raises(ProcessLookupError):
        os.killpg(group, 0)
    with pytest.raises(ProcessLookupError):
        os.kill(child, 0)


def test_private_staging_is_exact_readonly_and_exclusive(private_root: Path) -> None:
    path = private_root / "input"
    PROCESS.write_private_input(path, b"exact bytes")
    assert path.read_bytes() == b"exact bytes"
    assert stat.S_IMODE(path.stat().st_mode) == 0o400
    with pytest.raises(OSError, match="^verifier private input could not be staged$"):
        PROCESS.write_private_input(path, b"replacement")
    assert path.read_bytes() == b"exact bytes"
    executable = private_root / "executable"
    PROCESS.write_private_input(executable, b"executable bytes", 0o500)
    assert stat.S_IMODE(executable.stat().st_mode) == 0o500


def test_partial_writes_are_completed_and_failed_writes_stay_nonexecutable(private_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    original = PROCESS.os.write
    monkeypatch.setattr(PROCESS.os, "write", lambda fd, data: original(fd, data[:2]))
    path = private_root / "partial"
    PROCESS.write_private_input(path, b"complete exact bytes", 0o500)
    assert path.read_bytes() == b"complete exact bytes"
    monkeypatch.setattr(PROCESS.os, "write", lambda fd, data: 0)
    failed = private_root / "failed"
    with pytest.raises(OSError, match="^verifier private input could not be staged$"):
        PROCESS.write_private_input(failed, b"incomplete", 0o500)
    assert stat.S_IMODE(failed.stat().st_mode) == 0o600


def test_unsupported_process_and_staging_boundaries_fail_closed(private_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    source, _, digest = source_file(private_root)
    destination = private_root / "unsupported"
    with monkeypatch.context() as patch:
        patch.setattr(PROCESS.os, "name", "nt")
        with pytest.raises(OSError, match="^verifier private staging is unavailable$"):
            PROCESS.write_private_input(destination, b"bytes")
        with pytest.raises(OSError, match="^verifier executable snapshot unavailable$"):
            PROCESS.snapshot_executable(source, destination, digest)
        with pytest.raises(OSError, match="^verifier process groups are unavailable$"):
            run(private_root, "pass", expected_stderr=b"")
    assert not destination.exists()


def test_staging_rejects_public_parents_links_and_unsafe_modes(private_root: Path) -> None:
    for mode in (0o644, 0o755, 0o600):
        with pytest.raises(ValueError, match="^invalid verifier private input$"):
            PROCESS.write_private_input(private_root / "invalid", b"bytes", mode)
    outside = private_root / "outside"
    outside.write_bytes(b"unchanged")
    link = private_root / "link"
    link.symlink_to(outside)
    with pytest.raises(OSError):
        PROCESS.write_private_input(link, b"replacement")
    assert outside.read_bytes() == b"unchanged"
    private_root.chmod(0o755)
    with pytest.raises(OSError):
        PROCESS.write_private_input(private_root / "public", b"bytes")
    assert not (private_root / "public").exists()


def source_file(root: Path, content: bytes = b"#!/bin/sh\nexit 0\n") -> tuple[Path, bytes, str]:
    source = root / "source"
    source.write_bytes(content)
    source.chmod(0o700)
    return source, content, hashlib.sha256(content).hexdigest()


def test_executable_snapshot_streams_exact_pinned_bytes(private_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    source, content, digest = source_file(private_root, b"x" * (2 * PROCESS.COPY_CHUNK_BYTES + 11))
    destination = private_root / "copy"
    original_read = PROCESS.os.read
    requested = []
    def bounded_read(fd, size):
        requested.append(size)
        return original_read(fd, size)
    monkeypatch.setattr(PROCESS.os, "read", bounded_read)
    PROCESS.snapshot_executable(source, destination, digest)
    assert destination.read_bytes() == content
    assert max(requested) <= PROCESS.COPY_CHUNK_BYTES
    assert len(requested) >= 4
    assert stat.S_IMODE(destination.stat().st_mode) == 0o500
    assert destination.stat().st_ino != source.stat().st_ino


@pytest.mark.parametrize("failure", ["pin", "public", "nonexec", "hardlink", "symlink", "oversize", "owner"])
def test_unsafe_or_unpinned_executable_never_publishes_execute_permission(private_root: Path, monkeypatch: pytest.MonkeyPatch, failure: str) -> None:
    source, content, digest = source_file(private_root)
    destination = private_root / "copy"
    if failure == "pin":
        digest = "00" * 32
    elif failure == "public":
        source.chmod(0o722)
    elif failure == "nonexec":
        source.chmod(0o600)
    elif failure == "hardlink":
        os.link(source, private_root / "another")
    elif failure == "symlink":
        actual = private_root / "actual"
        source.rename(actual)
        source.symlink_to(actual)
    elif failure == "oversize":
        monkeypatch.setattr(PROCESS, "MAX_VERIFIER_EXECUTABLE_BYTES", len(content) - 1)
    else:
        owner = os.geteuid()
        monkeypatch.setattr(PROCESS.os, "geteuid", lambda: owner + 1)
    with pytest.raises((ValueError, OSError)) as error:
        PROCESS.snapshot_executable(source, destination, digest)
    assert str(source) not in str(error.value)
    assert not destination.exists() or not destination.stat().st_mode & 0o111


@pytest.mark.parametrize("mutation", ["bytes", "leaf", "parent"])
def test_source_descriptor_and_path_substitution_during_copy_are_rejected(private_root: Path, monkeypatch: pytest.MonkeyPatch, mutation: str) -> None:
    source_root = private_root / "source-parent"
    source_root.mkdir(mode=0o700)
    source, _content, digest = source_file(source_root)
    original_read = PROCESS.os.read
    changed = False
    def racing_read(fd, size):
        nonlocal changed
        chunk = original_read(fd, size)
        if chunk and not changed:
            changed = True
            if mutation == "bytes":
                source.write_bytes(b"same descriptor changed")
            elif mutation == "leaf":
                source.rename(source_root / "old")
                source.write_bytes(b"replacement leaf")
            else:
                source_root.rename(private_root / "old-parent")
                source_root.mkdir(mode=0o700)
                source.write_bytes(b"replacement parent")
        return chunk
    monkeypatch.setattr(PROCESS.os, "read", racing_read)
    destination = private_root / "copy"
    with pytest.raises(ValueError, match="^verifier executable snapshot rejected$"):
        PROCESS.snapshot_executable(source, destination, digest)
    assert not destination.stat().st_mode & 0o111


@pytest.mark.parametrize("mutation", ["mode", "path"])
def test_destination_parent_must_remain_private_and_at_the_original_path(private_root: Path, monkeypatch: pytest.MonkeyPatch, mutation: str) -> None:
    source, _, digest = source_file(private_root)
    destination_root = private_root / "destination"
    destination_root.mkdir(mode=0o700)
    destination = destination_root / "copy"
    original_write = PROCESS._write_all
    def racing_write(fd, payload):
        original_write(fd, payload)
        if mutation == "mode":
            destination_root.chmod(0o755)
        else:
            destination_root.rename(private_root / "old-destination")
            destination_root.mkdir(mode=0o700)
    monkeypatch.setattr(PROCESS, "_write_all", racing_write)
    with pytest.raises(OSError, match="^verifier executable snapshot unavailable$"):
        PROCESS.snapshot_executable(source, destination, digest)
    if mutation == "path":
        destination = private_root / "old-destination/copy"
    assert not destination.stat().st_mode & 0o111
