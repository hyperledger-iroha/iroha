"""Real small probe controls; no native candidate or platform qualification."""
from __future__ import annotations

import ctypes
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import time
import types

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("bounded_native_probe_under_test", ROOT / "scripts/check_native_sdk_artifact.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def command(source):
    return (sys.executable, "-I", "-B", "-c", source)


def run(source, **kwargs):
    return MODULE._run_bounded_probe(command(source), stdout_limit=kwargs.pop("stdout_limit", 8192),
                                    stderr_limit=kwargs.pop("stderr_limit", 8192),
                                    timeout_seconds=kwargs.pop("timeout_seconds", 3), **kwargs)


def track_children(monkeypatch):
    original = subprocess.Popen
    children = []
    def create(*args, **kwargs):
        result = original(*args, **kwargs); children.append(result); return result
    monkeypatch.setattr(MODULE.subprocess, "Popen", create)
    return children


def assert_reaped(children):
    assert len(children) == 1
    assert children[0].poll() is not None
    assert children[0].stdout.closed and children[0].stderr.closed


@pytest.mark.parametrize("code", (0, 7))
def test_actual_streams_and_nonzero_exit_are_preserved(monkeypatch, code):
    children = track_children(monkeypatch)
    result = run(f"import os,sys; os.write(1,bytes((50,51,13,10))); os.write(2,b'detail'); sys.exit({code})")
    assert result.stdout == b"23\r\n" and result.stderr == b"detail" and result.returncode == code
    assert_reaped(children)


def test_both_streams_accept_exact_byte_ceiling():
    result = run("import os; os.write(1,b'x'*8192); os.write(2,b'y'*8192)")
    assert result.stdout == b"x" * 8192 and result.stderr == b"y" * 8192


@pytest.mark.parametrize("fd", (1, 2))
@pytest.mark.parametrize("size", (8193, 65536))
def test_actual_flood_refuses_during_drain_and_reaps(monkeypatch, fd, size):
    children = track_children(monkeypatch)
    started = time.monotonic()
    source = f"import os\nwhile True: os.write({fd},b'x'*{size})"
    with pytest.raises(MODULE._ProbeOutputLimitExceeded, match="stdout" if fd == 1 else "stderr"):
        run(source)
    assert time.monotonic() - started < 2
    assert_reaped(children)


def test_zero_output_bounds_accept_empty_process():
    result = run("pass", stdout_limit=0, stderr_limit=0)
    assert result.stdout == result.stderr == b""


@pytest.mark.parametrize("fd", (1, 2))
def test_zero_bound_rejects_first_byte(fd):
    with pytest.raises(MODULE._ProbeOutputLimitExceeded):
        run(f"import os; os.write({fd},b'x')", stdout_limit=0, stderr_limit=0)


def test_actual_wall_deadline_kills_and_reaps(monkeypatch):
    children = track_children(monkeypatch)
    started = time.monotonic()
    with pytest.raises(subprocess.TimeoutExpired):
        run("import time; time.sleep(30)", timeout_seconds=0.15)
    assert time.monotonic() - started < 2
    assert_reaped(children)


@pytest.mark.skipif(os.name != "posix", reason="POSIX session signal control")
def test_ignored_sigterm_still_reaps_own_process(monkeypatch):
    children = track_children(monkeypatch)
    started = time.monotonic()
    with pytest.raises(subprocess.TimeoutExpired):
        run("import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(30)", timeout_seconds=0.15)
    assert time.monotonic() - started < 2
    assert_reaped(children)



def test_read_failure_reaps_and_closes_owned_pipes(monkeypatch):
    children = track_children(monkeypatch)
    def fail(*_args): raise OSError("injected read failure")
    monkeypatch.setattr(MODULE, "_read_probe_pipe", fail)
    with pytest.raises(OSError, match="injected"):
        run("import time; time.sleep(30)")
    assert_reaped(children)


@pytest.mark.parametrize("value", (-1, True, MODULE.MAX_SYMBOL_TOOL_OUTPUT_BYTES + 1))
def test_invalid_byte_bound_never_spawns(monkeypatch, value):
    children = track_children(monkeypatch)
    with pytest.raises(ValueError): run("pass", stdout_limit=value)
    assert not children


@pytest.mark.parametrize("value", (0, -1, float("inf"), float("nan"), True, 31))
def test_invalid_deadline_never_spawns(monkeypatch, value):
    children = track_children(monkeypatch)
    with pytest.raises(ValueError): run("pass", timeout_seconds=value)
    assert not children


@pytest.mark.parametrize("body,expected", (("print(23)", 23), ("print(' 23'+chr(13)+chr(10))", 23), ("print(0)", 0)))
def test_abi_decimal_success_semantics(body, expected):
    assert MODULE._probe_subprocess(command(body), label="inert ABI control") == expected


@pytest.mark.parametrize("body", ("print('23x')", "print('-1')", "print('23 23')", "print('２３')", "pass"))
def test_abi_malformed_decimal_still_refuses(body):
    with pytest.raises(MODULE.ArtifactContractError, match="noncanonical ABI"):
        MODULE._probe_subprocess(command(body), label="inert ABI control")


def test_abi_nonzero_exit_preserves_diagnostic():
    with pytest.raises(MODULE.ArtifactContractError, match="inert ABI control failed: detail"):
        MODULE._probe_subprocess(command("import sys; print('detail',file=sys.stderr); sys.exit(7)"), label="inert ABI control")


@pytest.mark.parametrize("fd", (1, 2))
def test_abi_helper_uses_live_stream_bounds(fd):
    with pytest.raises(MODULE.ArtifactContractError, match="byte limit"):
        MODULE._probe_subprocess(command(f"import os; os.write({fd},b'x'*4097)"), label="inert ABI control")


def test_symbol_tool_live_success_keeps_existing_parser(monkeypatch):
    monkeypatch.setattr(MODULE, "_symbol_tool_commands", lambda _path: (("inert", ("-I", "-B", "-c", "print('symbol_one'); print('symbol_two')"), "lines"),))
    monkeypatch.setattr(MODULE.shutil, "which", lambda _tool: sys.executable)
    assert MODULE.inspect_exported_symbols(Path("unused"), required=True) == ("symbol_one", "symbol_two")


def test_symbol_tool_flood_hits_bound_before_optional_fallback(monkeypatch):
    monkeypatch.setattr(MODULE, "MAX_SYMBOL_TOOL_OUTPUT_BYTES", 8192)
    monkeypatch.setattr(MODULE, "_symbol_tool_commands", lambda _path: (("inert", ("-I", "-B", "-c", "import os; os.write(1,b'x'*8193)"), "lines"),))
    monkeypatch.setattr(MODULE.shutil, "which", lambda _tool: sys.executable)
    with pytest.raises(MODULE.ArtifactContractError, match="stdout exceeded its byte limit"):
        MODULE.inspect_exported_symbols(Path("unused"), required=True)
    assert MODULE.inspect_exported_symbols(Path("unused"), required=False) is None


def test_node_and_python_probe_keep_same_shared_owner(monkeypatch, tmp_path):
    calls = []
    def observed(argv, **kwargs):
        calls.append((tuple(argv), kwargs))
        return subprocess.CompletedProcess(argv, 0, b"24", b"")
    monkeypatch.setattr(MODULE, "_run_bounded_probe", observed)
    assert MODULE.probe_node_abi(tmp_path / "inert.node", (), node="node") == 24
    assert MODULE.probe_python_abi(tmp_path / "inert.so", (), python=sys.executable) == 24
    assert calls[0][0][0:2] == ("node", "--eval")
    assert calls[1][0][0:4] == (sys.executable, "-I", "-B", "-c")
    assert all(options == {"stdout_limit": 4096, "stderr_limit": 4096} for _, options in calls)


@pytest.mark.parametrize("available,expected", ((None, b""), (0, None), (200, b"abcd")))
def test_windows_reader_uses_only_available_bounded_bytes(monkeypatch, available, expected):
    calls = []
    monkeypatch.setattr(MODULE.os, "name", "nt")
    monkeypatch.setattr(MODULE, "_windows_pipe_available", lambda _fd: available)
    def read(fd, size): calls.append((fd, size)); return b"abcd"
    monkeypatch.setattr(MODULE.os, "read", read)
    assert MODULE._read_probe_pipe(17, 4) == expected
    assert calls == ([(17, 4)] if available else [])


@pytest.mark.parametrize("available,error,expected", ((8, 0, 8), (0, 0, 0), (0, 109, None), (0, 233, None)))
def test_windows_peek_owned_pipe_contract(monkeypatch, available, error, expected):
    class Peek:
        def __call__(self, handle, _buffer, size, _read, count, _remaining):
            assert handle == 1017 and size == 0
            count._obj.value = available
            return not error
    kernel = types.SimpleNamespace(PeekNamedPipe=Peek())
    monkeypatch.setitem(sys.modules, "msvcrt", types.SimpleNamespace(get_osfhandle=lambda fd: fd + 1000))
    monkeypatch.setattr(MODULE.ctypes, "WinDLL", lambda *args, **kwargs: kernel, raising=False)
    monkeypatch.setattr(MODULE.ctypes, "get_last_error", lambda: error, raising=False)
    assert MODULE._windows_pipe_available(17) == expected


def test_windows_peek_unknown_failure_is_not_eof(monkeypatch):
    class Peek:
        def __call__(self, *_args): return False
    kernel = types.SimpleNamespace(PeekNamedPipe=Peek())
    monkeypatch.setitem(sys.modules, "msvcrt", types.SimpleNamespace(get_osfhandle=lambda _fd: 1017))
    monkeypatch.setattr(MODULE.ctypes, "WinDLL", lambda *args, **kwargs: kernel, raising=False)
    monkeypatch.setattr(MODULE.ctypes, "get_last_error", lambda: 5, raising=False)
    monkeypatch.setattr(MODULE.ctypes, "WinError", lambda code: OSError(code, "access denied"), raising=False)
    with pytest.raises(OSError, match="access denied"): MODULE._windows_pipe_available(17)


def test_explicit_environment_and_closed_stdin_are_preserved():
    result = run("import os,sys; print(os.environ['IROHA_INERT_CONTROL']); print(len(sys.stdin.buffer.read()))",
                 env={"IROHA_INERT_CONTROL": "original"})
    assert result.stdout == b"original\n0\n"


def test_abi_retains_maximum_stderr_diagnostic_on_failure():
    source = "import os,sys; os.write(2,b'x'*4096); sys.exit(7)"
    with pytest.raises(MODULE.ArtifactContractError) as caught:
        MODULE._probe_subprocess(command(source), label="inert ABI control")
    assert str(caught.value) == "inert ABI control failed: " + "x" * 4096


@pytest.mark.skipif(os.name != "posix", reason="actual POSIX outer process owner")
@pytest.mark.parametrize("reason", ("timeout", "cancel"))
@pytest.mark.parametrize("ignore_term", (False, True))
def test_actual_outer_owner_stops_nested_probe_without_session_escape(tmp_path, monkeypatch, reason, ignore_term):
    sys.path.insert(0, str(ROOT / "scripts"))
    import sorafs_python_process as outer
    for name in ("home", "temporary"):
        (tmp_path / name).mkdir()
    ready, late = tmp_path / "nested-ready", tmp_path / "late-side-effect"
    nested = ("import os,time; from pathlib import Path; "
              + ("import signal; signal.signal(signal.SIGTERM,signal.SIG_IGN); " if ignore_term else "")
              + f"Path({str(ready)!r}).write_text(str(os.getpid())+' '+str(os.getpgrp())+' '+str(os.getsid(0))); "
              + f"time.sleep(1.5); Path({str(late)!r}).write_text('escaped')")
    checker = ("import importlib.util,sys; "
               + f"spec=importlib.util.spec_from_file_location('nested_checker',{str(ROOT / 'scripts/check_native_sdk_artifact.py')!r}); "
               + "module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); "
               + f"module._run_bounded_probe((sys.executable,'-I','-B','-c',{nested!r}),stdout_limit=1024,stderr_limit=1024,timeout_seconds=3)")
    children = track_children(monkeypatch)
    if reason == "cancel":
        original_selector = outer.selectors.DefaultSelector
        class CancelWhenNestedStarts:
            def __init__(self): self.delegate = original_selector()
            def __getattr__(self, name): return getattr(self.delegate, name)
            def select(self, timeout):
                if ready.exists(): raise KeyboardInterrupt("actual nested cancellation")
                return self.delegate.select(timeout)
        monkeypatch.setattr(outer.selectors, "DefaultSelector", CancelWhenNestedStarts)
    expected = outer.ProcessError if reason == "timeout" else KeyboardInterrupt
    started = time.monotonic()
    with pytest.raises(expected):
        outer.run_python_process(command(checker), cwd=tmp_path,
            stdout_path=tmp_path / "stdout", stderr_path=tmp_path / "stderr",
            home=tmp_path / "home", temporary=tmp_path / "temporary",
            stdout_limit=8192, stderr_limit=8192, timeout_seconds=0.35 if reason == "timeout" else 3)
    assert ready.exists(), "nested process never started; timeout control is not meaningful"
    nested_pid, nested_group, nested_session = map(int, ready.read_text().split())
    assert len(children) == 1 and children[0].poll() is not None
    assert nested_pid != children[0].pid
    assert nested_group == nested_session == children[0].pid, "nested probe escaped the outer owner"
    gone_deadline = time.monotonic() + 1
    while time.monotonic() < gone_deadline:
        try: os.kill(nested_pid, 0)
        except ProcessLookupError: break
        time.sleep(0.01)
    else: raise AssertionError("owned outer group left the observed nested PID alive")
    time.sleep(max(0, started + 1.7 - time.monotonic()))
    assert not late.exists(), "nested process continued after outer timeout/cancel"
    assert children[0].stdout.closed and children[0].stderr.closed
