"""Inert fixed-child process/EOF controls; never execute Node or an addon."""
from __future__ import annotations

import base64
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_javascript_child_process as process


ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture
def tmp_path():
    """Keep every inert child and capture under this one checkout's target."""
    (ROOT / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sorafs-js-child-process-", dir=ROOT / "target") as name:
        yield Path(name)


def _write_script(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    path.chmod(0o600)


def _environment(tmp_path: Path) -> dict[str, str]:
    return {"PATH": "/usr/bin:/bin", "HOME": str(tmp_path),
            "TMPDIR": str(tmp_path), "LC_ALL": "C", "TZ": "UTC"}


def _request(tmp_path: Path) -> tuple[int, str]:
    path = tmp_path / "original-input.json"
    path.write_bytes(b"inert original fd3 input\n")
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC)
    return fd, hashlib.sha256(path.read_bytes()).hexdigest()


def _run(tmp_path: Path, body: str, *, timeout: float = 3) -> process.ChildProcessCapture:
    script = tmp_path / "fixed-child.py"
    _write_script(script, body)
    fd, digest = _request(tmp_path)
    try:
        return process._spawn_fixed(Path(sys.executable), script, digest, fd,
                                    _environment(tmp_path), timeout)
    finally:
        os.close(fd)


FRAME = '''import base64, hashlib, json, os, sys
assert len(sys.argv) == 2
raw = os.pread(3, 1024, 0)
assert hashlib.sha256(raw).hexdigest() == sys.argv[1]
payload = json.dumps({"fd3": raw.decode(), "argv": sys.argv[1:],
                      "environment": sorted(os.environ)}, sort_keys=True).encode() + b"\\n"
sys.stdout.write("SORAFS_JAVASCRIPT_CHILD_V1 " + base64.b64encode(payload).decode() + "\\n")
sys.stdout.flush()
'''


def test_exact_fd3_argv_scrubbed_environment_and_exit_after_eof(tmp_path):
    sentinel = tmp_path / "after-frame"
    capture = _run(tmp_path, FRAME + f'''\nimport time\ntime.sleep(0.1)\nopen({str(sentinel)!r}, "w").write("exited")\n''')
    assert sentinel.read_text() == "exited"
    assert capture.exit_code == 0 and capture.stderr == b""
    assert capture.stdout == process.FRAME_PREFIX + base64.b64encode(capture.observation) + b"\n"
    payload = json.loads(capture.observation)
    assert payload["fd3"] == "inert original fd3 input\n"
    # The macOS framework Python used only for this inert fixture may add its
    # own text-encoding key after exec; posix_spawn still receives only the
    # closed environment supplied by this owner.
    assert set(payload["environment"]) - set(_environment(tmp_path)) <= {
        "__CF_USER_TEXT_ENCODING"
    }
    assert set(_environment(tmp_path)) <= set(payload["environment"])
    assert "NODE_OPTIONS" not in payload["environment"]
    assert payload["argv"] == [hashlib.sha256(b"inert original fd3 input\n").hexdigest()]


@pytest.mark.parametrize("body,match", [
    ("import sys;sys.stdout.write('SORAFS_JAVASCRIPT_CHILD_V1 partial')", "complete final frame"),
    (FRAME + "sys.stdout.write('SORAFS_JAVASCRIPT_CHILD_V1 extra\\n')", "extra or malformed frame"),
    ("import sys;sys.stdout.write('early output\\n')", "complete final frame"),
    ("import sys;sys.exit(7)", "exited without success"),
])
def test_truncation_extra_output_and_nonzero_exit_refuse(tmp_path, body, match):
    with pytest.raises(process.ChildProcessError, match=match):
        _run(tmp_path, body)


def test_pipe_limits_refuse_before_unbounded_accumulation(tmp_path, monkeypatch):
    monkeypatch.setattr(process, "MAX_STDERR_BYTES", 1024)
    with pytest.raises(process.ChildProcessError, match="byte limit") as raised:
        _run(tmp_path, "import os;os.write(2,b'x'*4096)")
    assert len(raised.value.captured_stderr) <= 1024

    monkeypatch.setattr(process, "MAX_STDOUT_BYTES", 1024)
    with pytest.raises(process.ChildProcessError, match="byte limit") as raised:
        _run(tmp_path, "import os;os.write(1,b'x'*4096)")
    assert len(raised.value.captured_stdout) <= 1024


def test_successful_child_stderr_is_refused_but_its_bytes_are_retained(tmp_path):
    with pytest.raises(process.ChildProcessError, match="wrote stderr") as raised:
        _run(tmp_path, FRAME + "os.write(2,b'inert diagnostic')\n")
    assert raised.value.captured_stderr == b"inert diagnostic"
    assert raised.value.captured_stdout.startswith(process.FRAME_PREFIX)


def test_unrelated_inheritable_parent_descriptor_is_closed_in_child(tmp_path):
    extra_path = tmp_path / "unrelated"
    extra_path.write_bytes(b"unrelated")
    extra_fd = os.open(extra_path, os.O_RDONLY)
    os.set_inheritable(extra_fd, True)
    body = FRAME + f'''\ntry: os.fstat({extra_fd})\nexcept OSError: pass\nelse: raise AssertionError("unrelated descriptor escaped")\n'''
    try:
        capture = _run(tmp_path, body)
    finally:
        os.close(extra_fd)
    assert capture.exit_code == 0


def test_descriptor_census_bound_refuses_before_spawn(tmp_path, monkeypatch):
    monkeypatch.setattr(process, "MAX_PARENT_OPEN_FDS", 2)
    with pytest.raises(process.ChildProcessError, match="descriptor census"):
        _run(tmp_path, FRAME)


def test_selector_cleanup_failure_still_attempts_every_pipe_close(tmp_path, monkeypatch):
    original_pipe = os.pipe2
    original_selector = process.selectors.DefaultSelector
    created = []

    def observed_pipe(flags):
        pair = original_pipe(flags)
        created.extend(pair)
        return pair

    class FailingSelector:
        def __init__(self):
            self.inner = original_selector()

        def __getattr__(self, name):
            return getattr(self.inner, name)

        def close(self):
            self.inner.close()
            raise OSError("inert selector cleanup failure")

    monkeypatch.setattr(process.os, "pipe2", observed_pipe)
    monkeypatch.setattr(process.selectors, "DefaultSelector", FailingSelector)
    with pytest.raises(process.ChildProcessError, match="cleanup failed") as raised:
        _run(tmp_path, FRAME)
    assert len(raised.value.cleanup_errors) == 1
    assert len(created) == 4
    for fd in created:
        with pytest.raises(OSError):
            os.fstat(fd)


def test_write_pipe_close_failure_retains_all_ends_for_final_cleanup(tmp_path, monkeypatch):
    original_pipe = os.pipe2
    original_close = os.close
    created = []
    failed = False

    def observed_pipe(flags):
        pair = original_pipe(flags)
        created.extend(pair)
        return pair

    def fail_one_close(fd):
        nonlocal failed
        if len(created) == 4 and fd == created[1] and not failed:
            failed = True
            raise OSError("inert first write-close failure")
        return original_close(fd)

    monkeypatch.setattr(process.os, "pipe2", observed_pipe)
    monkeypatch.setattr(process.os, "close", fail_one_close)
    with pytest.raises(OSError, match="inert first write-close failure"):
        _run(tmp_path, FRAME)
    assert failed and len(created) == 4
    for fd in created:
        with pytest.raises(OSError):
            os.fstat(fd)


def test_timeout_stops_and_reaps_only_its_owned_child(tmp_path):
    pid_path = tmp_path / "child-pid"
    body = f'''import os,time\nopen({str(pid_path)!r}, "w").write(str(os.getpid()))\ntime.sleep(10)\n'''
    with pytest.raises(process.ChildProcessError, match="wall-clock limit"):
        _run(tmp_path, body, timeout=0.5)
    pid = int(pid_path.read_text())
    with pytest.raises(ProcessLookupError):
        os.kill(pid, 0)


def test_one_shot_owner_rechecks_both_originals_after_child_exit(tmp_path, monkeypatch):
    environment = tmp_path / "environment"
    temporary = tmp_path / "temporary"
    environment.mkdir(mode=0o700)
    temporary.mkdir(mode=0o700)
    script = environment / "qualification/tools/sorafs_javascript_child.mjs"
    sentinel = tmp_path / "child-exited"
    _write_script(script, FRAME + f'''\nopen({str(sentinel)!r}, "w").write("yes")\n''')
    raw = json.dumps({"schema": "sorafs.javascript.child_input.v1",
                      "environmentRoot": str(environment),
                      "temporaryRoot": str(temporary)}, sort_keys=True).encode() + b"\n"
    request = environment / "child-input.json"
    request.write_bytes(raw)

    class FakeInput:
        def __init__(self):
            self.fd = None
            self.checks = 0
            self.closed = False

        def __enter__(self):
            self.fd = os.open(request, os.O_RDONLY | os.O_CLOEXEC)
            return self

        def __exit__(self, kind, value, traceback):
            assert sentinel.read_text() == "yes"
            self.closed = True
            os.close(self.fd)

        @property
        def descriptor(self):
            assert self.fd is not None and not self.closed
            return self.fd

        @property
        def sha256(self):
            return hashlib.sha256(raw).hexdigest()

        def recheck(self):
            assert not self.closed and os.pread(self.fd, len(raw) + 1, 0) == raw
            self.checks += 1

    class FakeRuntime:
        def __init__(self):
            self.manifest = SimpleNamespace(selected_executable=sys.executable)
            self.checks = 0
            self.closed = False

        def __enter__(self):
            return self

        def __exit__(self, kind, value, traceback):
            assert sentinel.read_text() == "yes"
            self.closed = True

        def recheck(self):
            assert not self.closed
            self.checks += 1

    monkeypatch.setattr(process, "OriginalJavascriptChildInput", FakeInput)
    monkeypatch.setattr(process, "OriginalNodeRuntimeInputs", FakeRuntime)
    input_owner, runtime_owner = FakeInput(), FakeRuntime()
    owner = process.OriginalJavascriptChildProcess(input_owner, runtime_owner, timeout_seconds=3)
    capture = owner.run()
    assert capture.exit_code == 0 and input_owner.closed and runtime_owner.closed
    assert input_owner.checks == 2 and runtime_owner.checks == 2
    with pytest.raises(process.ChildProcessError, match="one-shot"):
        owner.run()


def test_failed_child_input_entry_closes_previously_acquired_runtime(monkeypatch):
    class FailingInput:
        def __enter__(self):
            raise process.ChildProcessError("inert child input refused")

        def __exit__(self, kind, value, traceback):
            raise AssertionError("failed entry has no matching exit")

    class HeldRuntime:
        def __init__(self):
            self.entered = False
            self.closed = False

        def __enter__(self):
            self.entered = True
            return self

        def __exit__(self, kind, value, traceback):
            assert kind is process.ChildProcessError
            self.closed = True

    monkeypatch.setattr(process, "OriginalJavascriptChildInput", FailingInput)
    monkeypatch.setattr(process, "OriginalNodeRuntimeInputs", HeldRuntime)
    runtime = HeldRuntime()
    owner = process.OriginalJavascriptChildProcess(FailingInput(), runtime,
                                                   timeout_seconds=3)
    with pytest.raises(process.ChildProcessError, match="inert child input refused"):
        owner.run()
    assert runtime.entered and runtime.closed
    with pytest.raises(process.ChildProcessError, match="one-shot"):
        owner.run()
