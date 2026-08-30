from __future__ import annotations

import hashlib
import importlib.util
import json
import fcntl
import os
import signal
import stat
import struct
import subprocess
import sys
import time
from pathlib import Path

import pytest


ROOT = Path(__file__).parents[2]
SCRIPT = ROOT / "scripts/package_zk_x509_prover_worker.py"
SPEC = importlib.util.spec_from_file_location("package_zk_x509_worker", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
package = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = package
SPEC.loader.exec_module(package)

CONTROLLER_SOURCE = (
    ROOT
    / "python/iroha_python/src/iroha_python/privacy_zk_x509_worker.py"
)
CONTROLLER_SPEC = importlib.util.spec_from_file_location(
    "package_test_privacy_zk_x509_worker_controller",
    CONTROLLER_SOURCE,
)
assert CONTROLLER_SPEC is not None and CONTROLLER_SPEC.loader is not None
controller = importlib.util.module_from_spec(CONTROLLER_SPEC)
sys.modules[CONTROLLER_SPEC.name] = controller
CONTROLLER_SPEC.loader.exec_module(controller)

def make_static_aarch64_elf(
    *,
    interpreter: bool = False,
    writable_executable: bool = False,
    needed: bool = False,
    executable_stack: bool = False,
    no_load: bool = False,
) -> bytes:
    dynamic = struct.pack("<qQqQ", 1, 1, 0, 0) if needed else b""
    program_count = 1 + int(needed) + int(executable_stack)
    total = 64 + 56 * program_count + len(dynamic)
    header = struct.pack(
        "<16sHHIQQQIHHHHHH",
        b"\x7fELF" + bytes((2, 1, 1)) + bytes(9),
        2,
        183,
        1,
        0,
        64,
        0,
        0,
        64,
        56,
        program_count,
        0,
        0,
        0,
    )
    first_type = 3 if interpreter else (0 if no_load else 1)
    first_flags = 7 if writable_executable else 5
    segments = [
        struct.pack("<IIQQQQQQ", first_type, first_flags, 0, 0, 0, total, total, 4096)
    ]
    if needed:
        dynamic_offset = 64 + 56 * program_count
        segments.append(
            struct.pack(
                "<IIQQQQQQ", 2, 4, dynamic_offset, 0, 0, len(dynamic), len(dynamic), 8
            )
        )
    if executable_stack:
        segments.append(struct.pack("<IIQQQQQQ", 0x6474E551, 1, 0, 0, 0, 0, 0, 16))
    return header + b"".join(segments) + dynamic


DIGESTS = tuple(f"{index:02x}" * 32 for index in range(1, 20))
COMMIT = "a" * 40
SOURCE_DATE_EPOCH = 1_700_000_000
FIXTURE_ARTIFACT_BYTES = make_static_aarch64_elf()
FIXTURE_ARTIFACT_SHA256 = hashlib.sha256(FIXTURE_ARTIFACT_BYTES).hexdigest()
RAW_COMMIT_SHA256 = DIGESTS[17]
SIGNER_PRINCIPAL = "release@example"
SIGNER_FINGERPRINT = "SHA256:" + "A" * 43


def _bounded_test_command(source: str, *arguments: Path | str) -> list[str]:
    return [
        sys.executable,
        "-I",
        "-S",
        "-c",
        source,
        *(os.fspath(argument) for argument in arguments),
    ]


def _bounded_test_environment() -> dict[str, str]:
    return {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"}


def test_cleanup_defers_controller_exception_until_supervisor_reap() -> None:
    class FakeProcess:
        pid = 424242

        def __init__(self) -> None:
            self.returncode: int | None = None
            self.terminate_count = 0
            self.wait_count = 0

        def poll(self) -> int | None:
            return self.returncode

        def terminate(self) -> None:
            self.terminate_count += 1
            if self.terminate_count == 1:
                raise InterruptedError

        def wait(self) -> int:
            self.wait_count += 1
            if self.wait_count == 1:
                raise InterruptedError
            if self.wait_count == 2:
                raise KeyboardInterrupt
            self.returncode = -signal.SIGKILL
            return self.returncode

    process = FakeProcess()
    with pytest.raises(KeyboardInterrupt):
        package._request_namespace_teardown_and_reap(process)  # type: ignore[arg-type]
    assert process.returncode == -signal.SIGKILL
    assert process.wait_count == 3
    assert process.terminate_count == 2


@pytest.mark.parametrize(("descriptor", "stream"), ((1, "stdout"), (2, "stderr")))
@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="bounded process execution is intentionally Linux-only",
)
def test_bounded_process_rejects_stream_flood(
    tmp_path: Path, descriptor: int, stream: str
) -> None:
    command = _bounded_test_command(
        f"import os; os.write({descriptor}, b'x' * 8192)"
    )
    with pytest.raises(package._BoundedProcessError, match=f"{stream} exceeded"):
        package._run_bounded_process(
            command,
            cwd=tmp_path,
            environment=_bounded_test_environment(),
            timeout=5,
            stdout_limit=1024,
            stderr_limit=1024,
        )


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="bounded process execution is intentionally Linux-only",
)
@pytest.mark.parametrize("parent_status", (0, 23))
def test_bounded_process_contains_setsid_descendant_after_parent_exit(
    tmp_path: Path, parent_status: int,
) -> None:
    escape_marker = tmp_path / "escaped-descendant"
    command = _bounded_test_command(
        "\n".join(
            (
                "import os, sys, time",
                "pid = os.fork()",
                "if pid == 0:",
                "    os.setsid()",
                "    for descriptor in (0, 1, 2):",
                "        try:",
                "            os.close(descriptor)",
                "        except OSError:",
                "            pass",
                "    time.sleep(0.25)",
                "    fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                "    os.write(fd, b'escaped')",
                "    os.close(fd)",
                "    os._exit(0)",
                "os._exit(int(sys.argv[2]))",
            )
        ),
        escape_marker,
        str(parent_status),
    )
    completed = package._run_bounded_process(
        command,
        cwd=tmp_path,
        environment=_bounded_test_environment(),
        timeout=2,
        stdout_limit=1024,
        stderr_limit=4096,
    )
    assert completed.returncode == parent_status
    time.sleep(0.5)
    assert not escape_marker.exists(), (
        "a detached subprocess descendant outlived the bounded runner"
    )


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the direct-target namespace teardown attack is Linux-specific",
)
def test_bounded_process_contains_direct_target_after_timeout(tmp_path: Path) -> None:
    armed = tmp_path / "armed"
    trigger = tmp_path / "trigger"
    escape_marker = tmp_path / "escaped-target"
    command = _bounded_test_command(
        "\n".join(
            (
                "import ctypes, os, sys, time",
                "if ctypes.CDLL(None).prctl(1, 0, 0, 0, 0) != 0:",
                "    os._exit(91)",
                "os.setsid()",
                "fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                "os.write(fd, b'armed')",
                "os.close(fd)",
                "for descriptor in (0, 1, 2):",
                "    try:",
                "        os.close(descriptor)",
                "    except OSError:",
                "        pass",
                "while not os.path.exists(sys.argv[2]):",
                "    time.sleep(0.01)",
                "fd = os.open(sys.argv[3], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                "os.write(fd, b'escaped')",
                "os.close(fd)",
            )
        ),
        armed,
        trigger,
        escape_marker,
    )
    with pytest.raises(package._BoundedProcessError, match="timed out"):
        package._run_bounded_process(
            command,
            cwd=tmp_path,
            environment=_bounded_test_environment(),
            timeout=0.5,
            stdout_limit=1024,
            stderr_limit=1024,
        )
    assert armed.exists(), "the direct target did not reach its escape posture"
    trigger.write_bytes(b"go")
    time.sleep(0.25)
    assert not escape_marker.exists(), (
        "a direct target escaped namespace teardown after timeout"
    )


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the detached output-overflow attack is Linux-specific",
)
def test_bounded_process_tears_down_namespace_on_output_overflow(
    tmp_path: Path,
) -> None:
    trigger = tmp_path / "trigger"
    escape_marker = tmp_path / "escaped-descendant"
    command = _bounded_test_command(
        "\n".join(
            (
                "import os, sys, time",
                "pid = os.fork()",
                "if pid == 0:",
                "    os.setsid()",
                "    for descriptor in (0, 1, 2):",
                "        try:",
                "            os.close(descriptor)",
                "        except OSError:",
                "            pass",
                "    while not os.path.exists(sys.argv[1]):",
                "        time.sleep(0.01)",
                "    fd = os.open(sys.argv[2], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                "    os.write(fd, b'escaped')",
                "    os.close(fd)",
                "    os._exit(0)",
                "os.write(1, b'x' * 8192)",
                "time.sleep(60)",
            )
        ),
        trigger,
        escape_marker,
    )
    with pytest.raises(package._BoundedProcessError, match="stdout exceeded"):
        package._run_bounded_process(
            command,
            cwd=tmp_path,
            environment=_bounded_test_environment(),
            timeout=5,
            stdout_limit=1024,
            stderr_limit=1024,
        )
    trigger.write_bytes(b"go")
    time.sleep(0.25)
    assert not escape_marker.exists(), (
        "an output-overflow descendant escaped namespace teardown"
    )


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the PID-namespace controller-death binding is Linux-specific",
)
def test_bounded_process_tears_down_namespace_on_controller_death(
    tmp_path: Path,
) -> None:
    armed = tmp_path / "armed"
    trigger = tmp_path / "trigger"
    escape_marker = tmp_path / "escaped-target"
    target_source = "\n".join(
        (
            "import ctypes, os, sys, time",
            "ctypes.CDLL(None).prctl(1, 0, 0, 0, 0)",
            "os.setsid()",
            "fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
            "os.write(fd, b'armed')",
            "os.close(fd)",
            "for descriptor in (0, 1, 2):",
            "    try:",
            "        os.close(descriptor)",
            "    except OSError:",
            "        pass",
            "while not os.path.exists(sys.argv[2]):",
            "    time.sleep(0.01)",
            "fd = os.open(sys.argv[3], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
            "os.write(fd, b'escaped')",
            "os.close(fd)",
        )
    )
    controller_source = "\n".join(
        (
            "import importlib.util, sys",
            "from pathlib import Path",
            "spec = importlib.util.spec_from_file_location('controller_death_package', Path(sys.argv[1]))",
            "module = importlib.util.module_from_spec(spec)",
            "sys.modules[spec.name] = module",
            "spec.loader.exec_module(module)",
            "module._run_bounded_process(",
            "    [sys.executable, '-I', '-S', '-c', sys.argv[6], *sys.argv[3:6]],",
            "    cwd=Path(sys.argv[2]),",
            "    environment={'LANG': 'C', 'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'},",
            "    timeout=30, stdout_limit=1024, stderr_limit=1024,",
            ")",
        )
    )
    controller = subprocess.Popen(
        _bounded_test_command(
            controller_source,
            SCRIPT,
            tmp_path,
            armed,
            trigger,
            escape_marker,
            target_source,
        ),
        cwd=tmp_path,
        env=_bounded_test_environment(),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    deadline = time.monotonic() + 5
    while not armed.exists() and controller.poll() is None and time.monotonic() < deadline:
        time.sleep(0.01)
    if not armed.exists():
        if controller.poll() is None:
            controller.kill()
        controller.wait(timeout=5)
        pytest.fail("the controller-death target did not reach its escape posture")
    os.kill(controller.pid, signal.SIGKILL)
    controller.wait(timeout=5)
    time.sleep(0.1)
    trigger.write_bytes(b"go")
    time.sleep(0.25)
    assert not escape_marker.exists(), "a target survived controller death"


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the pre-PDEATH controller race is Linux-specific",
)
def test_bounded_process_fails_closed_if_controller_dies_before_pdeath_binding(
    tmp_path: Path,
) -> None:
    bootstrap_armed = tmp_path / "bootstrap-armed"
    target_marker = tmp_path / "target-ran"
    controller_source = "\n".join(
        (
            "import importlib.util, os, sys",
            "from pathlib import Path",
            "spec = importlib.util.spec_from_file_location('bootstrap_race_package', Path(sys.argv[1]))",
            "module = importlib.util.module_from_spec(spec)",
            "sys.modules[spec.name] = module",
            "spec.loader.exec_module(module)",
            "needle = 'if prctl(PR_SET_PDEATHSIG, signal.SIGKILL, 0, 0, 0) != 0:'",
            "injection = (",
            "    f'descriptor = os.open({sys.argv[3]!r}, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)\\n'",
            "    + 'os.write(descriptor, b\"armed\"); os.close(descriptor)\\n'",
            "    + 'import time; time.sleep(1)\\n'",
            ")",
            "module._LINUX_PID_NAMESPACE_SUPERVISOR = module._LINUX_PID_NAMESPACE_SUPERVISOR.replace(needle, injection + needle, 1)",
            "target = 'import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b\"ran\")'",
            "module._run_bounded_process(",
            "    [sys.executable, '-I', '-S', '-c', target, sys.argv[4]],",
            "    cwd=Path(sys.argv[2]), environment={}, timeout=5,",
            "    stdout_limit=1024, stderr_limit=1024,",
            ")",
        )
    )
    controller = subprocess.Popen(
        _bounded_test_command(
            controller_source,
            SCRIPT,
            tmp_path,
            bootstrap_armed,
            target_marker,
        ),
        cwd=tmp_path,
        env=_bounded_test_environment(),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    deadline = time.monotonic() + 5
    while (
        not bootstrap_armed.exists()
        and controller.poll() is None
        and time.monotonic() < deadline
    ):
        time.sleep(0.01)
    if not bootstrap_armed.exists():
        if controller.poll() is None:
            controller.kill()
        controller.wait(timeout=5)
        pytest.fail("the containment supervisor did not enter the pre-PDEATH window")
    os.kill(controller.pid, signal.SIGKILL)
    controller.wait(timeout=5)
    time.sleep(1.25)
    assert not target_marker.exists(), "the target ran after its controller was orphaned"


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the controller spawn-signal handshake is Linux-specific",
)
def test_bounded_process_defers_spawn_signal_until_handle_is_retained(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    target_marker = tmp_path / "target-ran"
    real_popen = package.subprocess.Popen
    spawned: list[subprocess.Popen[bytes]] = []

    def interrupted_popen(*args: object, **kwargs: object):
        process = real_popen(*args, **kwargs)
        spawned.append(process)
        os.kill(os.getpid(), signal.SIGINT)
        return process

    monkeypatch.setattr(package.subprocess, "Popen", interrupted_popen)
    with pytest.raises(KeyboardInterrupt):
        package._run_bounded_process(
            _bounded_test_command(
                "import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b'ran')",
                target_marker,
            ),
            cwd=tmp_path,
            environment=_bounded_test_environment(),
            timeout=5,
            stdout_limit=1024,
            stderr_limit=4096,
        )
    time.sleep(0.25)
    assert not target_marker.exists(), "target exec raced a controller signal during spawn"
    assert len(spawned) == 1
    assert spawned[0].poll() is not None, "containment supervisor was not reaped"


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the launch-authorization handshake is Linux-specific",
)
def test_bounded_process_pre_authorization_exception_never_execs_and_reaps_supervisor(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    target_marker = tmp_path / "target-ran"
    real_popen = package.subprocess.Popen
    real_write = package.os.write
    spawned: list[subprocess.Popen[bytes]] = []

    def retained_popen(*args: object, **kwargs: object):
        process = real_popen(*args, **kwargs)
        spawned.append(process)
        return process

    def interrupted_authorization(descriptor: int, payload: bytes) -> int:
        if payload == b"1":
            raise KeyboardInterrupt
        return real_write(descriptor, payload)

    monkeypatch.setattr(package.subprocess, "Popen", retained_popen)
    monkeypatch.setattr(package.os, "write", interrupted_authorization)
    with pytest.raises(KeyboardInterrupt):
        package._run_bounded_process(
            _bounded_test_command(
                "import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b'ran')",
                target_marker,
            ),
            cwd=tmp_path,
            environment=_bounded_test_environment(),
            timeout=5,
            stdout_limit=1024,
            stderr_limit=4096,
        )
    assert not target_marker.exists(), "target exec bypassed launch authorization"
    assert len(spawned) == 1
    assert spawned[0].poll() is not None, "containment supervisor was not reaped"


@pytest.mark.parametrize(
    "termination_signal", (signal.SIGINT, signal.SIGTERM, signal.SIGKILL)
)
@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="bounded process execution is intentionally Linux-only",
)
def test_bounded_process_preserves_signal_status(
    tmp_path: Path, termination_signal: signal.Signals
) -> None:
    completed = package._run_bounded_process(
        _bounded_test_command(
            f"import os; os.kill(os.getpid(), {int(termination_signal)})"
        ),
        cwd=tmp_path,
        environment=_bounded_test_environment(),
        timeout=5,
        stdout_limit=1024,
        stderr_limit=1024,
    )
    assert completed.returncode == -int(termination_signal)


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the supervisor target-signal normalization is Linux-specific",
)
def test_bounded_process_restores_sigpipe_default_before_exec(tmp_path: Path) -> None:
    completed = package._run_bounded_process(
        ["/bin/sh", "-c", "kill -PIPE $$; exit 77"],
        cwd=tmp_path,
        environment=_bounded_test_environment(),
        timeout=5,
        stdout_limit=1024,
        stderr_limit=1024,
    )
    assert completed.returncode == -signal.SIGPIPE


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the post-exec process-state probe is Linux-specific",
)
def test_bounded_process_exec_target_state_is_not_overclaimed(tmp_path: Path) -> None:
    completed = package._run_bounded_process(
        _bounded_test_command(
            "\n".join(
                (
                    "import ctypes, json",
                    "fields = {}",
                    "for line in open('/proc/self/status', encoding='ascii'):",
                    "    if ':' in line:",
                    "        name, value = line.split(':', 1)",
                    "        fields[name] = value.strip()",
                    "dumpable = ctypes.CDLL(None).prctl(3, 0, 0, 0, 0)",
                    "print(json.dumps({",
                    "    'dumpable': dumpable,",
                    "    'no_new_privs': fields.get('NoNewPrivs'),",
                    "    'cap_eff': fields.get('CapEff'),",
                    "    'cap_prm': fields.get('CapPrm'),",
                    "    'cap_inh': fields.get('CapInh'),",
                    "    'cap_amb': fields.get('CapAmb'),",
                    "}, sort_keys=True))",
                )
            )
        ),
        cwd=tmp_path,
        environment=_bounded_test_environment(),
        timeout=5,
        stdout_limit=4096,
        stderr_limit=4096,
    )
    state = json.loads(completed.stdout)
    # Linux resets an ordinary target to dumpable at exec. The generic runner
    # promises a non-dumpable trusted PID 1, not a non-dumpable arbitrary PID 2.
    assert state["dumpable"] == 1
    assert state["no_new_privs"] == "1"
    assert {
        state["cap_eff"],
        state["cap_prm"],
        state["cap_inh"],
        state["cap_amb"],
    } == {"0000000000000000"}


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the exec handshake is implemented by the Linux containment supervisor",
)
def test_bounded_process_rejects_failed_exec_handshake(tmp_path: Path) -> None:
    with pytest.raises(
        package._BoundedProcessError,
        match="OS-enforced descendant containment",
    ):
        package._run_bounded_process(
            ["/definitely/missing-zk-x509-target"],
            cwd=tmp_path,
            environment=_bounded_test_environment(),
            timeout=5,
            stdout_limit=1024,
            stderr_limit=1024,
        )


def test_bounded_process_fails_before_exec_on_unsupported_host(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    marker = tmp_path / "ran"
    monkeypatch.setattr(sys, "platform", "unsupported-zk-x509-host")
    with pytest.raises(
        package._BoundedProcessError,
        match="requires Linux user and PID namespaces",
    ):
        package._run_bounded_process(
            _bounded_test_command(
                "import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b'ran')",
                marker,
            ),
            cwd=tmp_path,
            environment=_bounded_test_environment(),
            timeout=5,
            stdout_limit=1024,
            stderr_limit=1024,
        )
    assert not marker.exists()


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="the unavailable namespace bootstrap is Linux-specific",
)
def test_bounded_process_never_execs_if_linux_containment_bootstrap_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    marker = tmp_path / "ran"
    monkeypatch.setattr(
        package,
        "_LINUX_PID_NAMESPACE_SUPERVISOR",
        "import os; os._exit(125)",
    )
    with pytest.raises(
        package._BoundedProcessError,
        match="could not be established",
    ):
        package._run_bounded_process(
            _bounded_test_command(
                "import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b'ran')",
                marker,
            ),
            cwd=tmp_path,
            environment=_bounded_test_environment(),
            timeout=5,
            stdout_limit=1024,
            stderr_limit=1024,
        )
    assert not marker.exists()


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="bounded process execution is intentionally Linux-only",
)
def test_bounded_process_interleaves_stdin_with_both_output_streams(
    tmp_path: Path,
) -> None:
    input_data = b"i" * (64 * 1024)
    command = _bounded_test_command(
        "\n".join(
            (
                "import os",
                "total = 0",
                "while True:",
                "    chunk = os.read(0, 256)",
                "    if not chunk:",
                "        break",
                "    total += len(chunk)",
                "    os.write(1, b'o' * 1024)",
                "    os.write(2, b'e' * 1024)",
                "os.write(1, str(total).encode('ascii'))",
            )
        )
    )
    completed = package._run_bounded_process(
        command,
        cwd=tmp_path,
        environment=_bounded_test_environment(),
        timeout=5,
        stdout_limit=300 * 1024,
        stderr_limit=300 * 1024,
        input_data=input_data,
    )
    assert completed.returncode == 0
    assert completed.stdout.endswith(str(len(input_data)).encode("ascii"))
    assert len(completed.stdout) == 256 * 1024 + len(str(len(input_data)))
    assert completed.stderr == b"e" * (256 * 1024)


def candidate_identity() -> package.WorkerIdentityV2:
    return package.WorkerIdentityV2(
        cargo_lock_sha256=DIGESTS[5],
        compiled_profile_sha256=None,
        expectations_json_sha256=None,
        expectations_norito_sha256=None,
        isolation_contract=package.UNAVAILABLE_ISOLATION_CONTRACT,
        isolation_package_sha256=None,
        kat_proof_bytes=0,
        kat_proof_sha256=None,
        production_profile_ready=False,
        protocol_id=package.PROTOCOL_ID,
        protocol_profile_sha256=DIGESTS[2],
        protocol_version=package.PROTOCOL_VERSION,
        public_request_schema_version=package.PUBLIC_REQUEST_SCHEMA_VERSION,
        qualified_isolation_ready=False,
        release_evidence_ready=False,
        release_evidence_sha256=None,
        resource_certificate_sha256=None,
        soundness_certificate_sha256=DIGESTS[3],
        source_allowed_signers_sha256=DIGESTS[4],
        source_closure_schema=package.SOURCE_CLOSURE_SCHEMA,
        source_commit=COMMIT,
        source_revocation_sha256=DIGESTS[6],
        source_sha256=DIGESTS[0],
        workspace_source_manifest_sha256=DIGESTS[1],
    )


def release_identity() -> package.WorkerIdentityV2:
    kat_proof_bytes = 1_234_567
    kat_proof_sha256 = DIGESTS[7]
    expectations_norito_sha256 = DIGESTS[8]
    expectations_json_sha256 = DIGESTS[9]
    resource_certificate_sha256 = DIGESTS[10]
    release_evidence_sha256 = package._release_evidence_sha256(
        protocol_profile_sha256=DIGESTS[2],
        kat_proof_bytes=kat_proof_bytes,
        kat_proof_sha256=kat_proof_sha256,
        expectations_norito_sha256=expectations_norito_sha256,
        expectations_json_sha256=expectations_json_sha256,
        soundness_certificate_sha256=DIGESTS[3],
        resource_certificate_sha256=resource_certificate_sha256,
    )
    return package.WorkerIdentityV2(
        cargo_lock_sha256=DIGESTS[5],
        compiled_profile_sha256=DIGESTS[2],
        expectations_json_sha256=expectations_json_sha256,
        expectations_norito_sha256=expectations_norito_sha256,
        isolation_contract=package.QUALIFIED_ISOLATION_CONTRACT,
        isolation_package_sha256=package._qualified_isolation_package_sha256(
            FIXTURE_ARTIFACT_SHA256
        ),
        kat_proof_bytes=kat_proof_bytes,
        kat_proof_sha256=kat_proof_sha256,
        production_profile_ready=True,
        protocol_id=package.PROTOCOL_ID,
        protocol_profile_sha256=DIGESTS[2],
        protocol_version=package.PROTOCOL_VERSION,
        public_request_schema_version=package.PUBLIC_REQUEST_SCHEMA_VERSION,
        qualified_isolation_ready=True,
        release_evidence_ready=True,
        release_evidence_sha256=release_evidence_sha256,
        resource_certificate_sha256=resource_certificate_sha256,
        soundness_certificate_sha256=DIGESTS[3],
        source_allowed_signers_sha256=DIGESTS[4],
        source_closure_schema=package.SOURCE_CLOSURE_SCHEMA,
        source_commit=COMMIT,
        source_revocation_sha256=DIGESTS[6],
        source_sha256=DIGESTS[0],
        workspace_source_manifest_sha256=DIGESTS[1],
    )


def source_evidence() -> package.SourceEvidenceV1:
    return package.SourceEvidenceV1(
        allowed_signers_sha256=DIGESTS[4],
        cargo_lock_sha256=DIGESTS[5],
        commit=COMMIT,
        raw_commit_sha256=RAW_COMMIT_SHA256,
        revocation_sha256=DIGESTS[6],
        signer_fingerprint=SIGNER_FINGERPRINT,
        signer_principal=SIGNER_PRINCIPAL,
        source_sha256=DIGESTS[0],
        source_date_epoch=SOURCE_DATE_EPOCH,
        workspace_source_manifest_sha256=DIGESTS[1],
    )


def fake_build_file(role: str, path: str) -> dict[str, object]:
    return {
        "mode": 0o500,
        "owner": 0,
        "path": path,
        "sha256": hashlib.sha256(role.encode()).hexdigest(),
        "size": len(role) + 1,
    }


def fake_build_provenance(
    source: package.SourceEvidenceV1,
    *,
    path: str = "/tools:/usr/bin:/bin",
    wrapper_digest_role: str = "rustc_wrapper",
    rust_component_role: str = "rustc-component",
    target: str = package.RELEASE_TARGET,
) -> dict[str, object]:
    cargo_suffix = target.upper().replace("-", "_").replace(".", "_")
    cc_suffix = target.replace("-", "_").replace(".", "_")
    environment = package._build_environment_values(source)
    environment.update(
        {
            "AR": "/tools/ar",
            f"AR_{cc_suffix}": "/tools/ar",
            "CC": "/tools/cc",
            f"CC_{cc_suffix}": "/tools/cc",
            f"CARGO_TARGET_{cargo_suffix}_LINKER": "/tools/cc",
            "HOME": "/home/release",
            "PATH": path,
            "CARGO_HOME": "/cargo-home",
            "CARGO_TARGET_DIR": "/target",
            "RUSTC": "/rust/bin/rustc",
            "RUSTC_WRAPPER": "/tools/sccache",
        }
    )
    tool_paths = {
        "archiver": "/tools/ar",
        "cargo": "/rust/bin/cargo",
        "dirname": "/usr/bin/dirname",
        "env": "/usr/bin/env",
        "git": "/usr/bin/git",
        "grep": "/usr/bin/grep",
        "lscpu": "/usr/bin/lscpu",
        "linker": "/tools/ld",
        "linker_driver": "/tools/cc",
        "python": "/usr/bin/python3",
        "rustc": "/rust/bin/rustc",
        "rustc_wrapper": "/tools/sccache",
        "shell": "/bin/bash",
        "tr": "/usr/bin/tr",
        "uname": "/usr/bin/uname",
    }
    tools = {
        role: fake_build_file(
            wrapper_digest_role if role == "rustc_wrapper" else role,
            tool_paths[role],
        )
        for role in package._BUILD_TOOL_ROLES
    }

    def component(role: str) -> dict[str, object]:
        digest_role = rust_component_role if role == "rustc" else role
        manifest_names = {
            "cargo": f"manifest-cargo-{target}",
            "rust_std": f"manifest-rust-std-{target}",
            "rustc": f"manifest-rustc-{target}",
        }
        return {
            "closure_sha256": hashlib.sha256(
                f"closure:{digest_role}".encode()
            ).hexdigest(),
            "file_count": 1,
            "manifest_path": f"/rust/lib/rustlib/{manifest_names[role]}",
            "manifest_sha256": hashlib.sha256(
                f"manifest:{digest_role}".encode()
            ).hexdigest(),
            "total_bytes": 1024,
        }

    toolchain = {
        "cargo_cache_roots": {
            "registry": {
                "device": 1,
                "entry_count": 1,
                "inode": 2,
                "mode": 0o755,
                "owner": 0,
                "path": "/cargo-cache/registry",
                "role": "registry",
                "total_file_bytes": 1,
                "tree_sha256": hashlib.sha256(b"registry-cache").hexdigest(),
            }
        },
        "cargo_configuration": [],
        "cargo_version_sha256": hashlib.sha256(b"cargo-version").hexdigest(),
        "components": {
            role: component(role) for role in package._RUST_COMPONENT_ROLES
        },
        "host": target,
        "rustc_version_sha256": hashlib.sha256(b"rustc-version").hexdigest(),
        "schema": package._BUILD_TOOLCHAIN_SCHEMA,
        "sysroot": "/rust",
        "target": target,
        "tools": tools,
    }
    return package._build_provenance_v2(
        environment,
        toolchain,
        source=source,
        target=target,
    )


def executable(tmp_path: Path) -> package.StableFileV1:
    artifact = tmp_path / package.ARTIFACT_FILE
    artifact.write_bytes(FIXTURE_ARTIFACT_BYTES)
    artifact.chmod(0o700)
    return package._stable_file(
        artifact,
        label="fixture worker",
        maximum=package._MAX_ARTIFACT_BYTES,
        require_executable=True,
        require_owner=True,
    )


def manifest(
    tmp_path: Path,
    *,
    identity: package.WorkerIdentityV2 | None = None,
    target: str = "aarch64-apple-darwin",
    authenticated_build: bool = False,
) -> tuple[dict[str, object], package.StableFileV1]:
    artifact = executable(tmp_path)
    build_method = (
        package.AUTHENTICATED_SOURCE_BUILD_V2
        if authenticated_build
        else package.PREBUILT_CANDIDATE_BUILD_V1
    )
    build_command_sha256 = (
        package._build_command_sha256(target) if authenticated_build else None
    )
    build_provenance = (
        fake_build_provenance(source_evidence(), target=target)
        if authenticated_build
        else None
    )
    return (
        package.build_manifest(
            artifact=artifact,
            identity=identity or candidate_identity(),
            source=source_evidence(),
            target=target,
            artifact_build_method=build_method,
            artifact_build_command_sha256=build_command_sha256,
            artifact_build_provenance=build_provenance,
        ),
        artifact,
    )


def test_checked_in_source_closure_is_exhaustive_and_deterministic() -> None:
    paths = package._source_closure_paths(ROOT)
    assert paths == tuple(sorted(paths, key=lambda item: item.as_posix()))
    assert len(paths) == len(set(paths))
    assert package.PurePosixPath(
        "python/iroha_python/src/iroha_python/privacy_wallet_worker.py"
    ) in paths
    assert package.PurePosixPath(
        "python/iroha_python/src/iroha_python/privacy_zk_x509_worker.py"
    ) in paths
    assert package.source_closure_sha256(ROOT) == package.source_closure_sha256(ROOT)
    assert package.source_closure_sha256(ROOT) != "0" * 64


def test_linux_launcher_is_source_closed_and_matches_packaging_policy() -> None:
    worker_source = (
        ROOT / "crates/iroha_core/src/bin/iroha_zk_x509_prover_worker.rs"
    ).read_text()
    launcher_source = (
        ROOT
        / "crates/iroha_core/src/bin/iroha_zk_x509_prover_worker/linux_isolation.rs"
    ).read_text()
    closure = (ROOT / package.SOURCE_CLOSURE_MANIFEST).read_text().splitlines()
    assert (
        "crates/iroha_core/src/bin/iroha_zk_x509_prover_worker/linux_isolation.rs"
        in closure
    )
    assert "crates/iroha_core/src/bin/iroha_zk_x509_prover_worker.rs" in closure
    assert '#[path = "iroha_zk_x509_prover_worker/linux_isolation.rs"]' in worker_source
    assert (
        "rustix::process::set_dumpable_behavior("
        "rustix::process::DumpableBehavior::NotDumpable)"
    ) in worker_source
    assert worker_source.index("if harden_process().is_err()") < worker_source.index(
        "let args = env::args()"
    )
    assert "fn process_is_nondumpable_v1() -> bool" in launcher_source
    assert "|| !process_is_nondumpable_v1()" in launcher_source
    assert "rustix::fs::MemfdFlags::EXEC" in launcher_source
    assert "rustix::fs::MemfdFlags::NOEXEC_SEAL" in launcher_source
    assert "rustix::fs::SealFlags::EXEC" in launcher_source
    assert "rustix::fs::ResolveFlags::BENEATH" in launcher_source
    assert "SYS_LANDLOCK_RESTRICT_SELF" in launcher_source
    assert "SECCOMP_FILTER_FLAG_TSYNC" in launcher_source
    assert "SYS_CLOSE_RANGE" in launcher_source
    assert "SECCOMP_RET_ERRNO_ENOSYS" in launcher_source
    assert "SYS_CLONE3" in launcher_source
    assert "close_bootstrap_fds_v1()?" in launcher_source
    assert 'join("cgroup.kill")' in launcher_source
    assert "initialize_privacy_release_rayon_pool_v1" in worker_source
    assert "require_release_rayon_pool_v1()?" in worker_source
    rust_policy = launcher_source.split(
        'const ISOLATION_POLICY_V1: &[u8] = b"', 1
    )[1].split('";', 1)[0].encode("ascii")
    assert rust_policy == package.ISOLATION_POLICY_V1
    assert controller._QUALIFIED_ISOLATION_CONTRACT_V1 == (
        package.QUALIFIED_ISOLATION_CONTRACT
    )
    assert controller._UNAVAILABLE_ISOLATION_CONTRACT_V1 == (
        package.UNAVAILABLE_ISOLATION_CONTRACT
    )
    assert controller._ISOLATION_PACKAGE_DOMAIN_V1 == (
        package.ISOLATION_PACKAGE_DOMAIN_V1
    )
    assert controller._ISOLATION_POLICY_V1 == package.ISOLATION_POLICY_V1
    assert controller._qualified_isolation_package_sha256(
        FIXTURE_ARTIFACT_SHA256
    ) == package._qualified_isolation_package_sha256(FIXTURE_ARTIFACT_SHA256)


def test_static_aarch64_elf_gate_is_shared_strict_policy() -> None:
    package._validate_static_aarch64_elf_bytes(make_static_aarch64_elf())
    for payload, message in (
        (make_static_aarch64_elf(interpreter=True), "PT_INTERP"),
        (make_static_aarch64_elf(needed=True), "DT_NEEDED"),
        (
            make_static_aarch64_elf(writable_executable=True),
            "writable executable",
        ),
        (make_static_aarch64_elf(executable_stack=True), "executable GNU stack"),
        (make_static_aarch64_elf(no_load=True), "no PT_LOAD"),
    ):
        with pytest.raises(package.ZkX509WorkerPackageError, match=message):
            package._validate_static_aarch64_elf_bytes(payload)
    wrong_machine = bytearray(make_static_aarch64_elf())
    wrong_machine[18:20] = (62).to_bytes(2, "little")
    with pytest.raises(package.ZkX509WorkerPackageError, match="AArch64"):
        package._validate_static_aarch64_elf_bytes(bytes(wrong_machine))


def test_explicitly_pinned_empty_revocation_policy_is_supported(tmp_path: Path) -> None:
    revocation = tmp_path / "revocation"
    revocation.write_bytes(b"")
    identity = package._stable_file(
        revocation,
        label="revocation fixture",
        maximum=package._MAX_POLICY_BYTES,
        allow_empty=True,
    )
    assert identity.size == 0
    assert identity.sha256 == hashlib.sha256(b"").hexdigest()


def test_git_environment_drops_inherited_repository_redirectors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("GIT_DIR", "/attacker/repository")
    monkeypatch.setenv("GIT_WORK_TREE", "/attacker/worktree")
    monkeypatch.setenv("GIT_OBJECT_DIRECTORY", "/attacker/objects")
    monkeypatch.setenv("git_alternate_object_directories", "/attacker/alternates")
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", "/attacker/config")
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "0")
    monkeypatch.setenv("GIT_OPTIONAL_LOCKS", "1")

    environment = package._git_environment()

    assert "GIT_DIR" not in environment
    assert "GIT_WORK_TREE" not in environment
    assert "GIT_OBJECT_DIRECTORY" not in environment
    assert "git_alternate_object_directories" not in environment
    assert environment["GIT_CONFIG_GLOBAL"] == os.devnull
    assert environment["GIT_CONFIG_NOSYSTEM"] == "1"
    assert environment["GIT_OPTIONAL_LOCKS"] == "0"
    assert environment["PATH"] == "/usr/bin:/bin"
    assert environment["GIT_NO_LAZY_FETCH"] == "1"
    assert environment["GIT_NO_REPLACE_OBJECTS"] == "1"
    assert "DYLD_INSERT_LIBRARIES" not in environment
    assert "LD_PRELOAD" not in environment


def test_raw_release_identity_subprocess_uses_a_closed_environment(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source_root = tmp_path / "source"
    source_root.mkdir()
    identity = {
        "cargo_lock_sha256": DIGESTS[5],
        "head_commit": COMMIT,
        "head_tree": "b" * 40,
        "index_tree": "b" * 40,
        "schema_version": 1,
        "workspace_source_manifest_sha256": DIGESTS[1],
    }
    calls: list[tuple[list[str], dict[str, object]]] = []
    signed_helper = b"raise SystemExit('the test subprocess is mocked')\n"

    def fake_run(args: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
        descriptors = kwargs["pass_fds"]
        assert isinstance(descriptors, tuple) and len(descriptors) == 1
        descriptor = descriptors[0]
        assert fcntl.fcntl(descriptor, fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY
        assert os.fstat(descriptor).st_nlink == 0
        assert os.pread(descriptor, len(signed_helper), 0) == signed_helper
        calls.append((args, kwargs))
        return subprocess.CompletedProcess(
            args,
            0,
            package._canonical_json_bytes(identity),
            b"",
        )

    monkeypatch.setattr(package, "_run_bounded_process", fake_run)
    assert (
        package._raw_release_source_identity(source_root.resolve(), signed_helper)
        == identity
    )
    assert len(calls) == 1
    args, kwargs = calls[0]
    assert args[:3] == [sys.executable, "-I", "-S"]
    assert args[3].startswith(("/proc/self/fd/", "/dev/fd/"))
    assert str(source_root / package._WORKSPACE_MANIFEST_HELPER_RELATIVE) not in args
    assert args[-3:] == ["--root", str(source_root.resolve()), "--release-identity-json"]
    assert kwargs["environment"] == {
        "HOME": "/",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }
    assert kwargs["cwd"] == "/"


def test_packaging_script_must_come_from_exact_source_checkout(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source_root = tmp_path / "source"
    script = source_root / package._PACKAGING_SCRIPT_RELATIVE
    script.parent.mkdir(parents=True)
    script.write_bytes(b"# authenticated packaging helper\n")
    monkeypatch.setattr(package, "__file__", str(script))
    monkeypatch.setattr(
        package,
        "_git",
        lambda _root, _arguments: f"{source_root.resolve()}\n",
    )

    package._require_source_checkout_identity_v1(source_root)

    other_root = tmp_path / "other"
    other_script = other_root / package._PACKAGING_SCRIPT_RELATIVE
    other_script.parent.mkdir(parents=True)
    other_script.write_bytes(script.read_bytes())
    with pytest.raises(
        package.ZkX509WorkerPackageError,
        match="does not equal --source-root",
    ):
        package._require_source_checkout_identity_v1(other_root)


def test_executed_packaging_helpers_must_match_signed_commit_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source_root = tmp_path / "source"
    committed: dict[str, bytes] = {}
    for relative in package._AUTHENTICATED_HELPER_RELATIVES:
        path = source_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        payload = f"authenticated:{relative.as_posix()}\n".encode()
        path.write_bytes(payload)
        committed[relative.as_posix()] = payload
    monkeypatch.setattr(
        package,
        "_git_bytes",
        lambda _root, arguments: committed[arguments[-1].split(":", 1)[1]],
    )

    package._authenticate_source_helpers_v1(source_root, COMMIT)

    changed = source_root / package._WORKSPACE_MANIFEST_HELPER_RELATIVE
    changed.write_bytes(b"changed helper\n")
    with pytest.raises(
        package.ZkX509WorkerPackageError,
        match="does not match the signed source revision",
    ):
        package._authenticate_source_helpers_v1(source_root, COMMIT)


def test_source_evidence_uses_raw_clean_identity_and_rechecks_policies(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source_root = tmp_path / "source"
    source_root.mkdir()
    allowed_signers = tmp_path / "allowed_signers"
    allowed_signers.write_bytes(b"release@example ssh-ed25519 fixture\n")
    revocation = tmp_path / "revocation"
    revocation.write_bytes(b"")
    allowed_digest = hashlib.sha256(allowed_signers.read_bytes()).hexdigest()
    revocation_digest = hashlib.sha256(b"").hexdigest()
    raw_identity = {
        "schema_version": 1,
        "head_commit": COMMIT,
        "head_tree": "b" * 40,
        "index_tree": "b" * 40,
        "workspace_source_manifest_sha256": DIGESTS[1],
        "cargo_lock_sha256": DIGESTS[5],
    }
    observed_signatures: list[tuple[Path, str, bytes, bytes]] = []
    monkeypatch.setattr(
        package,
        "_raw_release_source_identity",
        lambda root, _helper: dict(raw_identity),
    )
    monkeypatch.setattr(package, "_signed_source_helper_bytes", lambda *_args: b"signed")
    monkeypatch.setattr(package, "source_closure_sha256", lambda _root: DIGESTS[0])
    monkeypatch.setattr(
        package,
        "_git",
        lambda _root, arguments: (
            f"{COMMIT}\n"
            if arguments[0] == "rev-parse"
            else f"{SOURCE_DATE_EPOCH}\n"
        ),
    )
    monkeypatch.setattr(package, "_require_source_checkout_identity_v1", lambda _root: None)
    monkeypatch.setattr(package, "_authenticate_source_helpers_v1", lambda *_args: None)
    monkeypatch.setattr(
        package,
        "_verify_source_signature",
        lambda root, commit, allowed, revoked: (
            observed_signatures.append((root, commit, allowed, revoked))
            or (RAW_COMMIT_SHA256, SIGNER_PRINCIPAL, SIGNER_FINGERPRINT)
        ),
    )

    evidence = package.collect_source_evidence(
        source_root,
        allowed_signers=allowed_signers.resolve(),
        expected_allowed_signers_sha256=allowed_digest,
        revocation=revocation.resolve(),
        expected_revocation_sha256=revocation_digest,
        expected_signer_principal=SIGNER_PRINCIPAL,
        expected_signer_fingerprint=SIGNER_FINGERPRINT,
    )

    assert evidence == package.SourceEvidenceV1(
        allowed_signers_sha256=allowed_digest,
        cargo_lock_sha256=DIGESTS[5],
        commit=COMMIT,
        raw_commit_sha256=RAW_COMMIT_SHA256,
        revocation_sha256=revocation_digest,
        signer_fingerprint=SIGNER_FINGERPRINT,
        signer_principal=SIGNER_PRINCIPAL,
        source_sha256=DIGESTS[0],
        source_date_epoch=SOURCE_DATE_EPOCH,
        workspace_source_manifest_sha256=DIGESTS[1],
    )
    assert observed_signatures == [
        (
            source_root.resolve(),
            COMMIT,
            allowed_signers.read_bytes(),
            revocation.read_bytes(),
        ),
        (
            source_root.resolve(),
            COMMIT,
            allowed_signers.read_bytes(),
            revocation.read_bytes(),
        ),
    ]


def test_source_identity_change_during_evidence_collection_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source_root = tmp_path / "source"
    source_root.mkdir()
    allowed_signers = tmp_path / "allowed_signers"
    allowed_signers.write_bytes(b"release@example ssh-ed25519 fixture\n")
    revocation = tmp_path / "revocation"
    revocation.write_bytes(b"")
    first = {
        "head_commit": COMMIT,
        "workspace_source_manifest_sha256": DIGESTS[1],
        "cargo_lock_sha256": DIGESTS[5],
    }
    second = {**first, "workspace_source_manifest_sha256": DIGESTS[2]}
    identities = iter((first, second))
    monkeypatch.setattr(
        package, "_raw_release_source_identity", lambda _root, _helper: next(identities)
    )
    monkeypatch.setattr(package, "_signed_source_helper_bytes", lambda *_args: b"signed")
    monkeypatch.setattr(package, "source_closure_sha256", lambda _root: DIGESTS[0])
    monkeypatch.setattr(
        package,
        "_git",
        lambda _root, arguments: (
            f"{COMMIT}\n"
            if arguments[0] == "rev-parse"
            else f"{SOURCE_DATE_EPOCH}\n"
        ),
    )
    monkeypatch.setattr(package, "_require_source_checkout_identity_v1", lambda _root: None)
    monkeypatch.setattr(package, "_authenticate_source_helpers_v1", lambda *_args: None)
    monkeypatch.setattr(
        package,
        "_verify_source_signature",
        lambda *_args: (RAW_COMMIT_SHA256, SIGNER_PRINCIPAL, SIGNER_FINGERPRINT),
    )

    with pytest.raises(package.ZkX509WorkerPackageError, match="source or SSH policy changed"):
        package.collect_source_evidence(
            source_root,
            allowed_signers=allowed_signers.resolve(),
            expected_allowed_signers_sha256=hashlib.sha256(
                allowed_signers.read_bytes()
            ).hexdigest(),
            revocation=revocation.resolve(),
            expected_revocation_sha256=hashlib.sha256(b"").hexdigest(),
            expected_signer_principal=SIGNER_PRINCIPAL,
            expected_signer_fingerprint=SIGNER_FINGERPRINT,
        )


def test_package_source_auth_requires_exactly_one_canonical_ssh_signature() -> None:
    armor = (
        b"-----BEGIN SSH SIGNATURE-----\n"
        b"AAAA\n"
        b"-----END SSH SIGNATURE-----"
    )
    raw = (
        b"tree "
        + b"0" * 40
        + b"\ngpgsig "
        + armor.replace(b"\n", b"\n ")
        + b"\n\nmessage\n"
    )
    assert package._require_exact_one_ssh_signature(raw) == armor
    duplicated = raw.replace(
        b"\n\nmessage",
        b"\ngpgsig " + armor.replace(b"\n", b"\n ") + b"\n\nmessage",
    )
    with pytest.raises(package.ZkX509WorkerPackageError, match="exactly one"):
        package._require_exact_one_ssh_signature(duplicated)
    with pytest.raises(package.ZkX509WorkerPackageError, match="fingerprint"):
        package._require_ssh_fingerprint("SHA256:not-canonical", "fixture fingerprint")


def test_frozen_build_environment_drops_compiler_and_loader_injection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("RUSTFLAGS", "-C linker=/attacker/linker")
    monkeypatch.setenv("CARGO_ENCODED_RUSTFLAGS", "--cfg\x1fattacker")
    monkeypatch.setenv("RUSTC", "/attacker/rustc")
    monkeypatch.setenv("RUSTC_WRAPPER", "/attacker/wrapper")
    monkeypatch.setenv("LD_PRELOAD", "/attacker/library")
    monkeypatch.setenv("DYLD_INSERT_LIBRARIES", "/attacker/library")
    monkeypatch.setenv("CARGO_PROFILE_RELEASE_OPT_LEVEL", "0")
    monkeypatch.setenv("CARGO_TARGET_DIR", "/external/target")

    environment = package._frozen_build_environment(source_evidence())

    for forbidden in (
        "RUSTFLAGS",
        "RUSTC",
        "RUSTC_WRAPPER",
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
        "CARGO_PROFILE_RELEASE_OPT_LEVEL",
    ):
        assert forbidden not in environment
    assert environment["CARGO_ENCODED_RUSTFLAGS"] == (
        "-C\x1ftarget-feature=+crt-static"
    )
    for name, value in package._build_environment_values(source_evidence()).items():
        assert environment[name] == value


def test_candidate_manifest_is_canonical_but_not_release_ready(tmp_path: Path) -> None:
    value, _ = manifest(tmp_path)
    assert value["compiled_profile_sha256"] is None
    assert value["artifact_build_method"] == package.PREBUILT_CANDIDATE_BUILD_V1
    assert value["artifact_build_command_sha256"] is None
    assert value["artifact_build_environment_sha256"] is None
    assert value["artifact_build_provenance"] is None
    assert value["artifact_build_toolchain_sha256"] is None
    assert value["source_date_epoch"] == SOURCE_DATE_EPOCH
    assert value["protocol_profile_sha256"] == DIGESTS[2]
    assert value["kat_proof_bytes"] == 0
    assert value["kat_proof_sha256"] is None
    assert value["release_evidence_ready"] is False
    assert value["release_evidence_sha256"] is None
    assert value["qualified_isolation_ready"] is False
    assert value["isolation_package_sha256"] is None
    assert value["release_ready"] is False
    encoded = package.canonical_manifest_bytes(value)
    assert encoded.endswith(b"\n")
    assert json.loads(encoded) == value


def test_release_manifest_requires_profile_isolation_and_exact_target(
    tmp_path: Path,
) -> None:
    value, _ = manifest(
        tmp_path,
        identity=release_identity(),
        target=package.RELEASE_TARGET,
        authenticated_build=True,
    )
    assert value["release_ready"] is True
    (tmp_path / "wrong-target").mkdir()
    wrong_target, _ = manifest(
        tmp_path / "wrong-target",
        identity=release_identity(),
        target="x86_64-unknown-linux-gnu",
        authenticated_build=True,
    )
    assert package.validate_manifest(wrong_target)["release_ready"] is False
    overstated = dict(wrong_target)
    overstated["release_ready"] = True
    with pytest.raises(package.ZkX509WorkerPackageError, match="release-ready claim"):
        package.validate_manifest(overstated)


def test_release_manifest_binds_isolation_package_to_exact_launcher_image(
    tmp_path: Path,
) -> None:
    identity = release_identity()
    mismatched = package.WorkerIdentityV2(
        **{
            **identity.__dict__,
            "isolation_package_sha256": package._qualified_isolation_package_sha256(
                DIGESTS[18]
            ),
        }
    )
    with pytest.raises(
        package.ZkX509WorkerPackageError,
        match="isolation package does not bind",
    ):
        manifest(
            tmp_path,
            identity=mismatched,
            target=package.RELEASE_TARGET,
            authenticated_build=True,
        )


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("artifact_sha256", "0" * 64, "artifact SHA-256"),
        ("artifact_size", 0, "artifact size"),
        ("artifact_build_method", "unknown", "build method"),
        ("artifact_build_command_sha256", DIGESTS[14], "cannot claim"),
        ("artifact_build_environment_sha256", DIGESTS[15], "cannot claim"),
        ("artifact_build_toolchain_sha256", DIGESTS[16], "cannot claim"),
        ("source_date_epoch", 0, "source date epoch"),
        ("protocol_id", "retired-x509-alias", "protocol or source state"),
        ("protocol_version", 2, "protocol or source state"),
        ("protocol_profile_sha256", "0" * 64, "protocol profile SHA-256"),
        ("kat_proof_bytes", -1, "KAT proof length"),
        ("release_evidence_sha256", DIGESTS[12], "incomplete constituents"),
        ("isolation_package_sha256", DIGESTS[13], "readiness evidence"),
        ("source_commit", "b" * 39, "source commit"),
        ("source_commit", "0" * 40, "source commit"),
        ("source_commit_raw_sha256", "0" * 64, "raw source commit"),
        ("source_signer_fingerprint", "MD5:legacy", "fingerprint"),
        ("source_signer_principal", "release@example\nother", "principal"),
        ("source_sha256", "0" * 64, "source closure"),
        ("source_tree_clean", False, "protocol or source state"),
        ("source_commit_signature_verified", False, "protocol or source state"),
        ("workspace_source_manifest_sha256", "z" * 64, "workspace"),
    ],
)
def test_manifest_rejects_every_critical_pin_mutation(
    tmp_path: Path,
    field: str,
    value: object,
    message: str,
) -> None:
    canonical, _ = manifest(tmp_path)
    mutated = dict(canonical)
    mutated[field] = value
    with pytest.raises(package.ZkX509WorkerPackageError, match=message):
        package.validate_manifest(mutated)


def test_manifest_loader_rejects_duplicate_and_noncanonical_json(tmp_path: Path) -> None:
    value, _ = manifest(tmp_path)
    path = tmp_path / "manifest.json"
    encoded = package.canonical_manifest_bytes(value)
    path.write_bytes(encoded.replace(b'"schema":', b'"schema":"duplicate","schema":'))
    with pytest.raises(package.ZkX509WorkerPackageError, match="duplicate key"):
        package.load_manifest(path)
    path.write_bytes(json.dumps(value, indent=2).encode("utf-8"))
    with pytest.raises(package.ZkX509WorkerPackageError, match="not canonical JSON"):
        package.load_manifest(path)


def test_prebuilt_artifact_cannot_be_promoted_by_a_ready_identity(tmp_path: Path) -> None:
    value, _ = manifest(
        tmp_path,
        identity=release_identity(),
        target=package.RELEASE_TARGET,
    )
    assert value["production_profile_ready"] is True
    assert value["qualified_isolation_ready"] is True
    assert value["artifact_build_method"] == package.PREBUILT_CANDIDATE_BUILD_V1
    assert value["release_ready"] is False
    promoted = dict(value)
    promoted["release_ready"] = True
    with pytest.raises(package.ZkX509WorkerPackageError, match="release-ready claim"):
        package.validate_manifest(promoted)


def test_authenticated_build_environment_pin_cannot_be_mutated(tmp_path: Path) -> None:
    value, _ = manifest(
        tmp_path,
        identity=release_identity(),
        target=package.RELEASE_TARGET,
        authenticated_build=True,
    )
    mutated = dict(value)
    mutated["artifact_build_environment_sha256"] = DIGESTS[18]
    with pytest.raises(package.ZkX509WorkerPackageError, match="provenance"):
        package.validate_manifest(mutated)


def test_effective_path_and_inherited_environment_drift_changes_provenance() -> None:
    source = source_evidence()
    first = fake_build_provenance(source)
    changed_path = fake_build_provenance(
        source,
        path="/different-tools:/usr/bin:/bin",
    )
    assert first["environment_sha256"] != changed_path["environment_sha256"]

    environment = dict(first["environment"])
    environment["CARGO_TARGET_DIR"] = "/different-target"
    changed_inherited = package._build_provenance_v2(
        environment,
        first["toolchain"],
        source=source,
        target=package.RELEASE_TARGET,
    )
    assert first["environment_sha256"] != changed_inherited["environment_sha256"]


def test_wrapper_and_rust_toolchain_drift_change_provenance() -> None:
    source = source_evidence()
    first = fake_build_provenance(source)
    wrapper_drift = fake_build_provenance(
        source,
        wrapper_digest_role="replaced-rustc-wrapper",
    )
    toolchain_drift = fake_build_provenance(
        source,
        rust_component_role="replaced-rustc-component",
    )
    assert first["toolchain_sha256"] != wrapper_drift["toolchain_sha256"]
    assert first["toolchain_sha256"] != toolchain_drift["toolchain_sha256"]


def test_path_resolution_and_tool_byte_drift_are_observed(tmp_path: Path) -> None:
    first_dir = tmp_path / "first"
    second_dir = tmp_path / "second"
    first_dir.mkdir()
    second_dir.mkdir()
    first_wrapper = first_dir / "sccache"
    second_wrapper = second_dir / "sccache"
    first_wrapper.write_bytes(b"#!/bin/sh\nexit 0\n")
    second_wrapper.write_bytes(b"#!/bin/sh\nexit 1\n")
    first_wrapper.chmod(0o700)
    second_wrapper.chmod(0o700)
    first_path = package._resolve_build_executable(
        "sccache",
        {"PATH": f"{first_dir}:{second_dir}"},
        label="fixture wrapper",
    )
    second_path = package._resolve_build_executable(
        "sccache",
        {"PATH": f"{second_dir}:{first_dir}"},
        label="fixture wrapper",
    )
    first_record = package._stable_build_input_record(
        first_path,
        label="fixture wrapper",
        require_executable=True,
    )
    second_record = package._stable_build_input_record(
        second_path,
        label="fixture wrapper",
        require_executable=True,
    )
    assert first_record["path"] != second_record["path"]
    assert first_record["sha256"] != second_record["sha256"]


def _fixture_git_commit(repository: Path) -> str:
    environment = {
        "HOME": str(repository),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "GIT_AUTHOR_NAME": "Fixture",
        "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
        "GIT_AUTHOR_DATE": "1700000000 +0000",
        "GIT_COMMITTER_NAME": "Fixture",
        "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
        "GIT_COMMITTER_DATE": "1700000000 +0000",
    }
    subprocess.run(["/usr/bin/git", "init", "-q", str(repository)], check=True, env=environment)
    subprocess.run(["/usr/bin/git", "-C", str(repository), "add", "--all"], check=True, env=environment)
    subprocess.run(
        ["/usr/bin/git", "-C", str(repository), "commit", "-q", "-m", "fixture"],
        check=True,
        env=environment,
    )
    return subprocess.run(
        ["/usr/bin/git", "-C", str(repository), "rev-parse", "HEAD"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
        env=environment,
    ).stdout.strip()


def _make_snapshot_writable(root: Path) -> None:
    for directory, names, files in os.walk(root, topdown=False, followlinks=False):
        base = Path(directory)
        for name in files:
            path = base / name
            if not path.is_symlink():
                path.chmod(0o600)
        for name in names:
            path = base / name
            if not path.is_symlink():
                path.chmod(0o700)
    root.chmod(0o700)


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="authenticated Git snapshot execution is intentionally Linux-only",
)
def test_signed_source_snapshot_consumes_exact_committed_tree_not_worktree(
    tmp_path: Path,
) -> None:
    repository = tmp_path / "repository"
    repository.mkdir()
    (repository / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    (repository / "payload").write_bytes(b"signed bytes")
    subdirectory = repository / "subdirectory"
    subdirectory.mkdir()
    (subdirectory / "link").symlink_to("../payload")
    executable_file = repository / "tool"
    executable_file.write_bytes(b"#!/bin/sh\nexit 0\n")
    executable_file.chmod(0o700)
    commit = _fixture_git_commit(repository)
    (repository / "payload").write_bytes(b"mutable worktree bytes")

    destination = tmp_path / "snapshot"
    snapshot = package._export_signed_source_snapshot(repository, commit, destination)
    try:
        assert (snapshot.root / "payload").read_bytes() == b"signed bytes"
        assert (snapshot.root / "subdirectory/link").readlink() == Path("../payload")
        assert stat.S_IMODE((snapshot.root / "payload").stat().st_mode) == 0o400
        assert stat.S_IMODE((snapshot.root / "tool").stat().st_mode) == 0o500
        assert stat.S_IMODE(snapshot.root.stat().st_mode) == 0o500
        anchored = os.fstat(snapshot.descriptor)
        named = snapshot.root.lstat()
        assert (anchored.st_ino, anchored.st_uid) == (named.st_ino, named.st_uid)
    finally:
        os.close(snapshot.descriptor)
        _make_snapshot_writable(destination)


@pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="authenticated Git snapshot execution is intentionally Linux-only",
)
def test_signed_source_snapshot_rejects_git_archive_exclusion(tmp_path: Path) -> None:
    repository = tmp_path / "repository"
    repository.mkdir()
    (repository / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    (repository / ".gitattributes").write_text("payload export-ignore\n", encoding="utf-8")
    (repository / "payload").write_text("$Format:%H$\n", encoding="utf-8")
    commit = _fixture_git_commit(repository)
    destination = tmp_path / "snapshot"
    with pytest.raises(package.ZkX509WorkerPackageError, match="inventory is not the exact"):
        package._export_signed_source_snapshot(repository, commit, destination)
    _make_snapshot_writable(destination)


def test_cargo_configuration_inventory_fails_closed_on_any_effective_file(
    tmp_path: Path,
) -> None:
    invocation = tmp_path / "invocation"
    invocation.mkdir()
    cargo_home = tmp_path / "cargo-home"
    cargo_home.mkdir()
    assert package._cargo_configuration_records(
        invocation, {"CARGO_HOME": str(cargo_home), "HOME": str(tmp_path)}
    ) == []
    config_directory = invocation / ".cargo"
    config_directory.mkdir()
    (config_directory / "config.toml").write_text("[net]\noffline=true\n", encoding="utf-8")
    with pytest.raises(package.ZkX509WorkerPackageError, match="unexpectedly discovered"):
        package._cargo_configuration_records(
            invocation, {"CARGO_HOME": str(cargo_home), "HOME": str(tmp_path)}
        )


def test_cargo_cache_tree_seal_is_descriptor_anchored_and_content_exact(
    tmp_path: Path,
) -> None:
    cache = tmp_path / "registry"
    nested = cache / "src"
    nested.mkdir(parents=True)
    payload = nested / "crate.rs"
    payload.write_bytes(b"authenticated cache bytes")
    (nested / "alias.rs").symlink_to("crate.rs")
    descriptor = package._open_directory_descriptor(cache, "fixture Cargo cache")
    try:
        first = package._cargo_cache_tree_record(descriptor, cache, "registry")
        assert first["entry_count"] == 3
        assert first["total_file_bytes"] == len(b"authenticated cache bytes")
        payload.write_bytes(b"mutated cache bytes")
        second = package._cargo_cache_tree_record(descriptor, cache, "registry")
        assert second["tree_sha256"] != first["tree_sha256"]
    finally:
        os.close(descriptor)


def test_cargo_cache_tree_seal_rejects_escape_symlink(tmp_path: Path) -> None:
    cache = tmp_path / "git"
    cache.mkdir()
    outside = tmp_path / "outside"
    outside.write_bytes(b"ambient")
    (cache / "escape").symlink_to("../outside")
    descriptor = package._open_directory_descriptor(cache, "fixture Cargo cache")
    try:
        with pytest.raises(package.ZkX509WorkerPackageError, match="escapes"):
            package._cargo_cache_tree_record(descriptor, cache, "git")
    finally:
        os.close(descriptor)


def test_closed_cargo_home_uses_fd_links_then_materializes_durable_seals(
    tmp_path: Path,
) -> None:
    inherited = tmp_path / "inherited"
    registry = inherited / "registry"
    registry.mkdir(parents=True)
    (registry / "crate").write_bytes(b"sealed dependency")
    destination = tmp_path / "closed-home"
    cargo_home = package._closed_cargo_home(
        destination,
        {"CARGO_HOME": str(inherited), "HOME": str(tmp_path)},
    )
    try:
        descriptor_link = os.readlink(destination / "registry")
        assert descriptor_link == str(
            package._descriptor_path(
                cargo_home.cache_descriptors[0], "fixture registry cache"
            )
        )
        record = package._cargo_cache_tree_record(
            cargo_home.cache_descriptors[0], registry, "registry"
        )
        package._materialize_durable_cargo_cache_links(
            cargo_home, {"registry": record}
        )
        assert os.readlink(destination / "registry") == str(registry)
    finally:
        for descriptor in cargo_home.cache_descriptors:
            os.close(descriptor)


def test_cargo_cache_materialization_rejects_durable_path_inode_swap(
    tmp_path: Path,
) -> None:
    inherited = tmp_path / "inherited"
    registry = inherited / "registry"
    registry.mkdir(parents=True)
    (registry / "crate").write_bytes(b"sealed dependency")
    cargo_home = package._closed_cargo_home(
        tmp_path / "closed-home",
        {"CARGO_HOME": str(inherited), "HOME": str(tmp_path)},
    )
    moved = inherited / "registry-held-aside"
    try:
        record = package._cargo_cache_tree_record(
            cargo_home.cache_descriptors[0], registry, "registry"
        )
        registry.rename(moved)
        registry.mkdir()
        (registry / "crate").write_bytes(b"attacker replacement")
        with pytest.raises(
            package.ZkX509WorkerPackageError,
            match="durable cache path differs from its held root",
        ):
            package._materialize_durable_cargo_cache_links(
                cargo_home, {"registry": record}
            )
    finally:
        for descriptor in cargo_home.cache_descriptors:
            os.close(descriptor)


def test_ssh_policy_temp_parent_rejects_nonsticky_writable_directory(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    unsafe = tmp_path / "unsafe-temp"
    unsafe.mkdir(mode=0o777)
    unsafe.chmod(0o777)
    monkeypatch.setattr(package.tempfile, "gettempdir", lambda: str(unsafe))
    with pytest.raises(package.ZkX509WorkerPackageError, match="temporary ancestor"):
        package._validated_temporary_parent("fixture policy")


def test_rust_component_file_drift_changes_closure(tmp_path: Path) -> None:
    sysroot = tmp_path / "rust"
    manifests = sysroot / "lib" / "rustlib"
    manifests.mkdir(parents=True)
    driver = sysroot / "lib" / "driver.so"
    driver.write_bytes(b"first rustc driver")
    manifest_path = manifests / "manifest-rustc-fixture"
    manifest_path.write_text("file:lib/driver.so\n", encoding="utf-8")
    first = package._rust_component_closure_record(
        sysroot,
        "manifest-rustc-fixture",
        label="fixture rustc component",
    )
    driver.write_bytes(b"second rustc driver")
    second = package._rust_component_closure_record(
        sysroot,
        "manifest-rustc-fixture",
        label="fixture rustc component",
    )
    assert first["closure_sha256"] != second["closure_sha256"]


@pytest.mark.parametrize("mutation", ("path", "wrapper", "rust_component"))
def test_release_manifest_rejects_unrepinned_build_input_drift(
    tmp_path: Path,
    mutation: str,
) -> None:
    value, _ = manifest(
        tmp_path,
        identity=release_identity(),
        target=package.RELEASE_TARGET,
        authenticated_build=True,
    )
    changed = json.loads(json.dumps(value))
    provenance = changed["artifact_build_provenance"]
    if mutation == "path":
        provenance["environment"]["PATH"] = "/attacker:/usr/bin:/bin"
    elif mutation == "wrapper":
        provenance["toolchain"]["tools"]["rustc_wrapper"]["sha256"] = "f" * 64
    else:
        provenance["toolchain"]["components"]["rustc"]["closure_sha256"] = "f" * 64
    with pytest.raises(package.ZkX509WorkerPackageError, match="provenance"):
        package.validate_manifest(changed)


def test_prebuilt_candidate_cannot_carry_authenticated_build_evidence(
    tmp_path: Path,
) -> None:
    value, _ = manifest(tmp_path)
    provenance = fake_build_provenance(source_evidence())
    value["artifact_build_provenance"] = provenance
    value["artifact_build_environment_sha256"] = provenance["environment_sha256"]
    value["artifact_build_toolchain_sha256"] = provenance["toolchain_sha256"]
    with pytest.raises(package.ZkX509WorkerPackageError, match="cannot claim"):
        package.validate_manifest(value)


@pytest.mark.parametrize(
    "drift",
    ("environment", "path", "wrapper", "rust_component"),
)
def test_build_rejects_post_build_corridor_drift(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    drift: str,
) -> None:
    source_root = tmp_path / "source"
    source_root.mkdir()
    external_root = tmp_path / "external"
    external_root.mkdir(mode=0o700)
    snapshot_root = tmp_path / "snapshot"
    snapshot_root.mkdir()
    cargo_home_root = tmp_path / "cargo-home"
    cargo_home_root.mkdir(mode=0o700)
    source = source_evidence()
    first_provenance = fake_build_provenance(source)
    if drift == "path":
        repeated_provenance = fake_build_provenance(
            source,
            path="/different-tools:/usr/bin:/bin",
        )
    elif drift == "wrapper":
        repeated_provenance = fake_build_provenance(
            source,
            wrapper_digest_role="replaced-rustc-wrapper",
        )
    elif drift == "rust_component":
        repeated_provenance = fake_build_provenance(
            source,
            rust_component_role="replaced-rustc-component",
        )
    else:
        changed_environment = dict(first_provenance["environment"])
        changed_environment["CARGO_TARGET_DIR"] = "/different-target"
        repeated_provenance = package._build_provenance_v2(
            changed_environment,
            first_provenance["toolchain"],
            source=source,
            target=package.RELEASE_TARGET,
        )
    first = package.AuthenticatedBuildCorridorV2(
        cargo=Path("/rust/bin/cargo"),
        environment=dict(first_provenance["environment"]),
        provenance=first_provenance,
    )
    repeated = package.AuthenticatedBuildCorridorV2(
        cargo=Path("/rust/bin/cargo"),
        environment=dict(repeated_provenance["environment"]),
        provenance=repeated_provenance,
    )
    corridors = iter((first, repeated))
    monkeypatch.setattr(package, "_collect_from_args", lambda _args: source)
    monkeypatch.setattr(
        package,
        "_frozen_build_environment",
        lambda _source: {
            "CARGO_TARGET_DIR": str(tmp_path / "target"),
            "HOME": "/home/release",
            "PATH": "/tools",
        },
    )
    monkeypatch.setattr(
        package,
        "_export_signed_source_snapshot",
        lambda *_args: package.SignedSourceSnapshotV1(
            snapshot_root, package._open_directory_descriptor(snapshot_root, "snapshot")
        ),
    )
    monkeypatch.setattr(
        package,
        "_closed_cargo_home",
        lambda *_args: package.ClosedCargoHomeV1(cargo_home_root, ()),
    )
    monkeypatch.setattr(
        package,
        "_prepare_authenticated_build_corridor_v2",
        lambda *_args: next(corridors),
    )
    monkeypatch.setattr(
        package,
        "_cargo_target_directory",
        lambda *_args: tmp_path / "target",
    )
    monkeypatch.setattr(
        package,
        "_run_bounded_process",
        lambda arguments, **_kwargs: subprocess.CompletedProcess(
            list(arguments), 0, b"", b""
        ),
    )
    args = package.argparse.Namespace(
        source_root=source_root,
        external_build_root=external_root.resolve(),
        target=package.RELEASE_TARGET,
    )

    with pytest.raises(
        package.ZkX509WorkerPackageError,
        match="build inputs changed during compilation",
    ):
        package._build(args)


def test_content_addressed_package_verifies_exact_artifact_and_identity(
    tmp_path: Path,
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    assert packaged.name == artifact.sha256
    assert packaged.stat().st_mode & 0o777 == 0o500
    assert (packaged / package.ARTIFACT_FILE).stat().st_mode & 0o777 == 0o500
    assert (packaged / "manifest.json").stat().st_mode & 0o777 == 0o400

    def bound_identity_probe(
        snapshot: Path, *, expected_artifact_sha256: str
    ) -> package.WorkerIdentityV2:
        assert expected_artifact_sha256 == artifact.sha256
        assert hashlib.sha256(snapshot.read_bytes()).hexdigest() == artifact.sha256
        assert snapshot != packaged / package.ARTIFACT_FILE
        return candidate_identity()

    verified = package.verify_package(
        packaged,
        identity_probe=bound_identity_probe,
    )
    assert verified == value
    with pytest.raises(package.ZkX509WorkerPackageError, match="non-release candidate"):
        package.verify_package(
            packaged,
            identity_probe=lambda _path, **_kwargs: candidate_identity(),
            require_release_ready=True,
        )
    packaged.chmod(0o700)
    (packaged / package.ARTIFACT_FILE).chmod(0o700)
    (packaged / "manifest.json").chmod(0o600)


def test_default_package_verifier_authenticates_its_launch_helpers_against_source_commit(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    events: list[tuple[str, object]] = []
    verifier_root = tmp_path / "reviewed-source"
    verifier_root.mkdir()

    monkeypatch.setattr(
        package,
        "_script_checkout_root_v1",
        lambda: verifier_root,
    )
    monkeypatch.setattr(
        package,
        "_require_source_checkout_identity_v1",
        lambda root: events.append(("checkout", root)),
    )
    monkeypatch.setattr(
        package,
        "_authenticate_source_helpers_v1",
        lambda root, commit: events.append(("helpers", (root, commit))),
    )

    def probe(snapshot, expected_artifact_sha256, *, source_root=None):
        events.append(
            (
                "probe",
                (snapshot.record.sha256, expected_artifact_sha256, source_root),
            )
        )
        return candidate_identity()

    monkeypatch.setattr(package, "_probe_worker_identity_snapshot", probe)
    assert package.verify_package(packaged) == value
    assert events == [
        ("checkout", verifier_root),
        ("helpers", (verifier_root, COMMIT)),
        (
            "probe",
            (artifact.sha256, artifact.sha256, verifier_root),
        ),
        ("helpers", (verifier_root, COMMIT)),
    ]

    packaged.chmod(0o700)
    (packaged / package.ARTIFACT_FILE).chmod(0o700)
    (packaged / "manifest.json").chmod(0o600)


def test_default_package_verifier_refuses_before_worker_exec_when_helpers_drift(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    monkeypatch.setattr(
        package,
        "_script_checkout_root_v1",
        lambda: tmp_path / "reviewed-source",
    )
    monkeypatch.setattr(
        package,
        "_require_source_checkout_identity_v1",
        lambda _root: None,
    )
    monkeypatch.setattr(
        package,
        "_authenticate_source_helpers_v1",
        lambda _root, _commit: package._fail(
            "authenticated launch helper differs from signed source"
        ),
    )
    worker_executed = False

    def probe(*_args, **_kwargs):
        nonlocal worker_executed
        worker_executed = True
        return candidate_identity()

    monkeypatch.setattr(package, "_probe_worker_identity_snapshot", probe)
    with pytest.raises(
        package.ZkX509WorkerPackageError,
        match="launch helper differs from signed source",
    ):
        package.verify_package(packaged)
    assert worker_executed is False

    packaged.chmod(0o700)
    (packaged / package.ARTIFACT_FILE).chmod(0o700)
    (packaged / "manifest.json").chmod(0o600)


def test_package_copy_uses_held_artifact_after_source_path_replacement(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    real_copy = package._copy_artifact

    def replace_then_copy(snapshot, destination, **kwargs):
        replacement = tmp_path / "replacement-worker"
        replacement.write_bytes(b"different executable bytes")
        replacement.chmod(0o700)
        os.replace(replacement, artifact.path)
        return real_copy(snapshot, destination, **kwargs)

    monkeypatch.setattr(package, "_copy_artifact", replace_then_copy)
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    assert (packaged / package.ARTIFACT_FILE).read_bytes() == FIXTURE_ARTIFACT_BYTES
    assert hashlib.sha256((packaged / package.ARTIFACT_FILE).read_bytes()).hexdigest() == value[
        "artifact_sha256"
    ]


def test_package_publication_collision_never_replaces_attacker_entry(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    real_publish = package._atomic_rename_noreplace

    def collide(source, destination, **kwargs):
        attacker = output / destination
        attacker.mkdir()
        (attacker / "survivor").write_bytes(b"must remain")
        real_publish(source, destination, **kwargs)

    monkeypatch.setattr(package, "_atomic_rename_noreplace", collide)
    with pytest.raises(package.ZkX509WorkerPackageError, match="already exists"):
        package.write_package(
            artifact_path=artifact.path,
            manifest=value,
            output_root=output,
        )
    assert (output / str(value["artifact_sha256"]) / "survivor").read_bytes() == b"must remain"


def test_package_publication_rejects_post_rename_inventory_injection(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    real_publish = package._atomic_rename_noreplace

    def inject(source, destination, **kwargs):
        real_publish(source, destination, **kwargs)
        published = output / destination
        published.chmod(0o700)
        (published / "injected").write_bytes(b"attacker")
        published.chmod(0o500)

    monkeypatch.setattr(package, "_atomic_rename_noreplace", inject)
    with pytest.raises(package.ZkX509WorkerPackageError, match="inventory"):
        package.write_package(
            artifact_path=artifact.path,
            manifest=value,
            output_root=output,
        )


def test_release_package_requires_exact_out_of_band_full_manifest_root(
    tmp_path: Path,
) -> None:
    value, artifact = manifest(
        tmp_path,
        identity=release_identity(),
        target=package.RELEASE_TARGET,
        authenticated_build=True,
    )
    output = tmp_path / "packages"
    output.mkdir()
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    expected_root = package.authenticated_package_root_sha256(value)

    with pytest.raises(
        package.ZkX509WorkerPackageError,
        match="externally trusted package-root",
    ):
        package.verify_package(
            packaged,
            identity_probe=lambda _path, **_kwargs: release_identity(),
            require_release_ready=True,
        )
    with pytest.raises(
        package.ZkX509WorkerPackageError,
        match="does not match the trusted package root",
    ):
        package.verify_package(
            packaged,
            identity_probe=lambda _path, **_kwargs: release_identity(),
            require_release_ready=True,
            trusted_package_root_sha256="f" * 64,
        )
    assert package.verify_package(
        packaged,
        identity_probe=lambda _path, **_kwargs: release_identity(),
        require_release_ready=True,
        trusted_package_root_sha256=expected_root,
    ) == value

    packaged.chmod(0o700)
    (packaged / package.ARTIFACT_FILE).chmod(0o700)
    (packaged / "manifest.json").chmod(0o600)


@pytest.mark.parametrize(
    ("member", "mode", "message"),
    [
        ("directory", 0o700, "directory must be owner-controlled mode 0500"),
        ("artifact", 0o700, "artifact must have mode 0500"),
        ("manifest", 0o600, "manifest must have mode 0400"),
    ],
)
def test_package_verifier_rejects_mode_tampering(
    tmp_path: Path,
    member: str,
    mode: int,
    message: str,
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    target = {
        "directory": packaged,
        "artifact": packaged / package.ARTIFACT_FILE,
        "manifest": packaged / "manifest.json",
    }[member]
    target.chmod(mode)
    with pytest.raises(package.ZkX509WorkerPackageError, match=message):
        package.verify_package(
            packaged,
            identity_probe=lambda _path, **_kwargs: candidate_identity(),
        )
    packaged.chmod(0o700)
    (packaged / package.ARTIFACT_FILE).chmod(0o700)
    (packaged / "manifest.json").chmod(0o600)


def test_package_verifier_rejects_non_owner_package(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    real_euid = os.geteuid()
    with monkeypatch.context() as context:
        context.setattr(package.os, "geteuid", lambda: real_euid + 1)
        with pytest.raises(
            package.ZkX509WorkerPackageError,
            match="directory must be owner-controlled mode 0500",
        ):
            package.verify_package(
                packaged,
                identity_probe=lambda _path, **_kwargs: candidate_identity(),
            )
    packaged.chmod(0o700)
    (packaged / package.ARTIFACT_FILE).chmod(0o700)
    (packaged / "manifest.json").chmod(0o600)


def test_packaged_artifact_mutation_is_rejected(tmp_path: Path) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    packaged.chmod(0o700)
    packaged_artifact = packaged / package.ARTIFACT_FILE
    packaged_artifact.chmod(0o700)
    packaged_artifact.write_bytes(b"substituted worker")
    packaged_artifact.chmod(0o500)
    packaged.chmod(0o500)
    with pytest.raises(package.ZkX509WorkerPackageError, match="does not match"):
        package.verify_package(
            packaged,
            identity_probe=lambda _path, **_kwargs: candidate_identity(),
        )
    packaged.chmod(0o700)
    packaged_artifact.chmod(0o700)
    (packaged / "manifest.json").chmod(0o600)


def test_package_verification_rejects_same_bytes_artifact_inode_swap(
    tmp_path: Path,
) -> None:
    value, artifact = manifest(tmp_path)
    output = tmp_path / "packages"
    output.mkdir()
    packaged = package.write_package(
        artifact_path=artifact.path,
        manifest=value,
        output_root=output,
    )
    packaged_artifact = packaged / package.ARTIFACT_FILE

    def replacing_probe(_snapshot: Path, **_kwargs) -> package.WorkerIdentityV2:
        packaged.chmod(0o700)
        old = packaged / "old-worker"
        packaged_artifact.rename(old)
        packaged_artifact.write_bytes(FIXTURE_ARTIFACT_BYTES)
        packaged_artifact.chmod(0o500)
        old.unlink()
        packaged.chmod(0o500)
        return candidate_identity()

    with pytest.raises(package.ZkX509WorkerPackageError, match="changed during verification"):
        package.verify_package(packaged, identity_probe=replacing_probe)


def identity_payload(
    *,
    value: package.WorkerIdentityV2 | None = None,
    overrides: dict[str, object] | None = None,
) -> bytes:
    worker = value or candidate_identity()
    identity = {
        "artifact_self_hash_required": True,
        "cargo_lock_sha256": worker.cargo_lock_sha256,
        "compiled_profile_sha256": worker.compiled_profile_sha256,
        "expectations_json_sha256": worker.expectations_json_sha256,
        "expectations_norito_sha256": worker.expectations_norito_sha256,
        "operation": "prove-and-sign-zk-x509-action-v1",
        "production_profile_ready": worker.production_profile_ready,
        "protocol_id": package.PROTOCOL_ID,
        "protocol_profile_sha256": worker.protocol_profile_sha256,
        "protocol_version": package.PROTOCOL_VERSION,
        "public_request_schema_version": package.PUBLIC_REQUEST_SCHEMA_VERSION,
        "qualified_isolation_ready": worker.qualified_isolation_ready,
        "isolation_contract": worker.isolation_contract,
        "isolation_package_sha256": worker.isolation_package_sha256,
        "kat_proof_bytes": worker.kat_proof_bytes,
        "kat_proof_sha256": worker.kat_proof_sha256,
        "release_evidence_ready": worker.release_evidence_ready,
        "release_evidence_sha256": worker.release_evidence_sha256,
        "resource_certificate_sha256": worker.resource_certificate_sha256,
        "schema": "iroha.privacy.zk_x509_worker_identity",
        "schema_version": 2,
        "soundness_certificate_sha256": worker.soundness_certificate_sha256,
        "source_allowed_signers_sha256": worker.source_allowed_signers_sha256,
        "source_closure_schema": package.SOURCE_CLOSURE_SCHEMA,
        "source_commit": COMMIT,
        "source_revocation_sha256": worker.source_revocation_sha256,
        "source_sha256": DIGESTS[0],
        "workspace_source_manifest_sha256": DIGESTS[1],
    }
    if overrides:
        identity.update(overrides)
    encoded = json.dumps(identity, separators=(",", ":")).encode("utf-8")
    return bytes((package._RESPONSE_OK,)) + encoded


def test_identity_parser_accepts_candidate_and_rejects_overstated_readiness() -> None:
    assert package._parse_identity(identity_payload()) == candidate_identity()
    assert package._parse_identity(identity_payload(value=release_identity())) == release_identity()
    with pytest.raises(package.ZkX509WorkerPackageError, match="overstates"):
        package._parse_identity(
            identity_payload(overrides={"production_profile_ready": True})
        )
    with pytest.raises(package.ZkX509WorkerPackageError, match="compiled profile"):
        package._parse_identity(
            identity_payload(overrides={"compiled_profile_sha256": DIGESTS[2]})
        )
    with pytest.raises(package.ZkX509WorkerPackageError, match="does not match"):
        package._parse_identity(
            identity_payload(
                value=release_identity(),
                overrides={"release_evidence_sha256": DIGESTS[12]},
            )
        )
    with pytest.raises(package.ZkX509WorkerPackageError, match="isolation identity"):
        package._parse_identity(
            identity_payload(
                value=release_identity(),
                overrides={"isolation_package_sha256": None},
            )
        )


def test_source_identity_mismatch_cannot_be_packaged(tmp_path: Path) -> None:
    artifact = executable(tmp_path)
    mismatched = candidate_identity()
    mismatched = package.WorkerIdentityV2(
        **{**mismatched.__dict__, "source_sha256": DIGESTS[15]}
    )
    with pytest.raises(package.ZkX509WorkerPackageError, match="authenticated source"):
        package.build_manifest(
            artifact=artifact,
            identity=mismatched,
            source=source_evidence(),
            target="aarch64-apple-darwin",
        )


def test_package_script_has_no_secret_input_surface() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    assert 'add_argument("--signer-principal", required=True)' in source
    assert 'add_argument("--signer-fingerprint", required=True)' in source
    assert 'add_argument("--witness' not in source
    assert 'add_argument("--secret-bundle' not in source
    assert '_SIGNED_MANIFEST_TOKEN = "@SIGNED_SOURCE_SNAPSHOT@/Cargo.toml"' in source
    assert '"cargo",\n        "build",' in source
    assert '"iroha-fast"' not in source
    assert '"gpg.format=ssh"' in source
    assert 'f"gpg.ssh.allowedSignersFile={private_allowed}"' in source
    assert 'f"gpg.ssh.revocationFile={private_revocation}"' in source
    assert 'f"gpg.ssh.program={_SYSTEM_SSH_KEYGEN}"' in source
    assert '"verify-commit"' in source
    assert "_raw_release_source_identity(source_root, signed_source_helper)" in source
    assert "signed source identity helper snapshot remained path-addressable" in source
    assert "AUTHENTICATED_SOURCE_BUILD_V2" in source
    assert "PREBUILT_CANDIDATE_BUILD_V1" in source
    assert "_prepare_verified_worker_launch_v1" in source
    assert "pass_fds=launch.pass_fds" in source
    assert "launch.authenticate()" in source
