#!/usr/bin/env python3
"""Build, package, and verify the isolated native zk-X.509 worker.

The release package is content-addressed by the worker executable SHA-256 and
binds the exact X5PW protocol, reviewed Git signature policy, Cargo lock,
source-closure and whole-workspace manifests, protocol/KAT/expectation/resource
evidence, compiled profile, isolation package, target, frozen build command,
every effective environment value, resolved build tools, Rust component
closures, Cargo configuration, and artifact bytes. Externally supplied
artifacts are permanently candidate-only; release readiness is available only
to the authenticated source-build path after identical post-build recapture.
Candidate packages are allowed so incomplete review work can be inspected, but
``--require-release-ready`` rejects them and also requires an out-of-band
trusted digest of the complete canonical manifest. This script never accepts
or opens a signer seed, certificate witness, or secret bundle.

External command execution is deliberately Linux-only. Every command runs
below a trusted init in fresh user and PID namespaces; if that boundary or its
exact identity maps cannot be established, the command fails before target
exec. There is no macOS, process-group-only, or descendant-scan fallback.
Only the trusted namespace init is generically non-dumpable; the source-closed
worker separately re-establishes and attests its own post-exec non-dumpability.
"""

from __future__ import annotations

import argparse
import contextlib
import ctypes
import errno
import fcntl
import hashlib
import hmac
import json
import os
import re
import secrets
import selectors
import shutil
import signal
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import types
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Callable, Mapping, NoReturn, Sequence

_PACKAGING_SCRIPT_RELATIVE = Path("scripts/package_zk_x509_prover_worker.py")
_WORKSPACE_MANIFEST_HELPER_RELATIVE = Path(
    "scripts/compute_workspace_source_manifest.py"
)
_EXACT_LAUNCH_SOURCE_RELATIVE = Path(
    "python/iroha_python/src/iroha_python/privacy_wallet_worker.py"
)
_AUTHENTICATED_HELPER_RELATIVES = (
    _PACKAGING_SCRIPT_RELATIVE,
    _WORKSPACE_MANIFEST_HELPER_RELATIVE,
    _EXACT_LAUNCH_SOURCE_RELATIVE,
)


SCHEMA = "iroha.privacy.zk_x509_prover_worker_package.v5"
SCHEMA_VERSION = 5
ARTIFACT_FILE = "iroha_zk_x509_prover_worker"
PROTOCOL_ID = "iroha-zk-x509-stark-p256-v0"
PROTOCOL_VERSION = 1
PUBLIC_REQUEST_SCHEMA_VERSION = 1
SOURCE_CLOSURE_SCHEMA = (
    "path-and-length-framed-sha256("
    "ci/privacy_zk_x509_worker_source_closure_v1.txt):v3"
)
SOURCE_CLOSURE_MANIFEST = Path(
    "ci/privacy_zk_x509_worker_source_closure_v1.txt"
)
RELEASE_TARGET = "aarch64-unknown-linux-gnu"
AUTHENTICATED_SOURCE_BUILD_V2 = "cargo-direct-frozen-signed-snapshot-v3"
PREBUILT_CANDIDATE_BUILD_V1 = "prebuilt-artifact-candidate-v1"
QUALIFIED_ISOLATION_CONTRACT = (
    "iroha.zk-x509.qualified-linux-aarch64-launcher.v1"
)
UNAVAILABLE_ISOLATION_CONTRACT = QUALIFIED_ISOLATION_CONTRACT + ":unavailable"
ISOLATION_PACKAGE_DOMAIN_V1 = (
    b"iroha.privacy.zk-x509.qualified-linux-launcher-package.v1"
)
ISOLATION_POLICY_V1 = (
    b"target=aarch64-unknown-linux-gnu;kernel-min=6.3;static-elf=true;"
    b"openat2=resolve-beneath+no-symlinks+no-magiclinks;"
    b"executable-memfd=mfd-exec+seal-exec+seal-write+seal-grow+seal-shrink+seal-seal;"
    b"attestation-memfd=mfd-noexec-seal+seal-exec+seal-write+seal-grow+seal-shrink+seal-seal;"
    b"uid=nonzero-equal-real+effective+saved+fs;"
    b"capabilities=effective+permitted+inheritable+ambient-zero;"
    b"landlock-abi-min=3;seccomp-tsync=true;seccomp-future-syscalls=deny;"
    b"seccomp-pidfd+privileged=deny;no-new-privs=true;dumpable=false;"
    b"cgroup-v2=true;memory-max=12884901888;memory-swap-max=0;"
    b"memory-oom-group=1;pids-max=6;cpu-max=max+period-100000;"
    b"rlimit-as=34359738368;rlimit-core=0;"
    b"fd-closure=stdio+64-72-bootstrap+stdio+one-data-runtime;"
    b"wall-ms=300000"
)

_FRAME_MAGIC = b"X5PW"
_FRAME_PROTOCOL_VERSION = 1
_IDENTITY_COMMAND = 1
_RESPONSE_OK = 0
_AUTH_KEY_BYTES = 32
_AUTH_TAG_BYTES = 32
_MAX_FRAME_BYTES = 12 * 1024 * 1024
_MAX_IDENTITY_BYTES = 64 * 1024
_MAX_ARTIFACT_BYTES = 512 * 1024 * 1024
_MAX_MANIFEST_BYTES = 64 * 1024
_MAX_POLICY_BYTES = 16 * 1024 * 1024
_MAX_KAT_PROOF_BYTES = 8_212_538
_SOURCE_CLOSURE_DOMAIN = b"iroha.privacy.zk-x509.worker-source-closure.v3"
_RELEASE_EVIDENCE_DOMAIN = b"iroha.privacy.zk-x509.worker-release-evidence.v1"
_AUTHENTICATED_PACKAGE_ROOT_DOMAIN = (
    b"iroha.privacy.zk-x509.worker-authenticated-package-root.v1"
)
_BUILD_COMMAND_DOMAIN = b"iroha.privacy.zk-x509.worker-build-command.v1"
_BUILD_ENVIRONMENT_DOMAIN = b"iroha.privacy.zk-x509.worker-build-environment.v2"
_BUILD_TOOLCHAIN_DOMAIN = b"iroha.privacy.zk-x509.worker-build-toolchain.v3"
_BUILD_PROVENANCE_SCHEMA = "iroha.privacy.zk-x509.worker-build-provenance.v2"
_BUILD_TOOLCHAIN_SCHEMA = "iroha.privacy.zk-x509.worker-build-toolchain.v3"
_CARGO_CACHE_TREE_DOMAIN = b"iroha.privacy.zk-x509.cargo-cache-tree.v1"
_RUST_COMPONENT_CLOSURE_DOMAIN = (
    b"iroha.privacy.zk-x509.worker-rust-component-closure.v1"
)
_SYSTEM_GIT = "/usr/bin/git"
_SYSTEM_SSH_KEYGEN = "/usr/bin/ssh-keygen"
_SIGNED_MANIFEST_TOKEN = "@SIGNED_SOURCE_SNAPSHOT@/Cargo.toml"

_MAX_TOOL_FILE_BYTES = 512 * 1024 * 1024
_MAX_TOOL_OUTPUT_BYTES = 64 * 1024
_MAX_BUILD_OUTPUT_BYTES = 256 * 1024 * 1024
_MAX_BUILD_SECONDS = 4 * 60 * 60
_MAX_SIGNED_SOURCE_ARCHIVE_BYTES = 20 * 1024 * 1024 * 1024
_MAX_CARGO_CACHE_FILE_BYTES = 4 * 1024 * 1024 * 1024
_MAX_CARGO_CACHE_TOTAL_BYTES = 128 * 1024 * 1024 * 1024
_MAX_CARGO_CACHE_ENTRIES = 2_000_000

_SHA256_RE = re.compile(r"[0-9a-f]{64}")
_COMMIT_RE = re.compile(r"[0-9a-f]{40}")
_SSH_FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}")
_TARGET_RE = re.compile(r"[a-z0-9][a-z0-9._+-]{0,127}")
_ENVIRONMENT_NAME_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]{0,127}")
_INHERITED_BUILD_ENVIRONMENT_NAMES = (
    "CARGO_HOME",
    "CARGO_TARGET_DIR",
    "HOME",
    "PATH",
    "RUSTUP_HOME",
    "SCCACHE_DIR",
    "TMPDIR",
)
_BUILD_TOOL_ROLES = (
    "archiver",
    "cargo",
    "dirname",
    "env",
    "git",
    "grep",
    "lscpu",
    "linker",
    "linker_driver",
    "python",
    "rustc",
    "rustc_wrapper",
    "shell",
    "tr",
    "uname",
)
_RUST_COMPONENT_ROLES = ("cargo", "rust_std", "rustc")


class ZkX509WorkerPackageError(RuntimeError):
    """One fail-closed worker-package validation error."""


class _BoundedProcessError(RuntimeError):
    """One process violated its bounded, OS-contained contract."""


_CONTAINMENT_ENVIRONMENT_MAX_BYTES = 4 * 1024 * 1024
_CONTAINMENT_BOOTSTRAP_ENVIRONMENT = {
    "LANG": "C",
    "LC_ALL": "C",
    "PATH": "/usr/bin:/bin",
    "PYTHONCOERCECLOCALE": "0",
    "PYTHONUTF8": "0",
}
_LINUX_PID_NAMESPACE_SUPERVISOR = r"""
import ctypes
import fcntl
import json
import os
import signal
import struct
import sys

CLONE_NEWUSER = 0x10000000
CLONE_NEWPID = 0x20000000
PR_SET_PDEATHSIG = 1
PR_SET_DUMPABLE = 4
PR_CAPBSET_DROP = 24
PR_SET_SECUREBITS = 28
PR_SET_NO_NEW_PRIVS = 38
PR_GET_NO_NEW_PRIVS = 39
LINUX_CAPABILITY_VERSION_3 = 0x20080522
TARGET_SECUREBITS = 1 | 2 | 4 | 8 | 32 | 64 | 128
MAX_ENVIRONMENT_BYTES = 4 * 1024 * 1024
FAILURE = 125

class CapabilityHeader(ctypes.Structure):
    _fields_ = [("version", ctypes.c_uint32), ("pid", ctypes.c_int)]

class CapabilityData(ctypes.Structure):
    _fields_ = [
        ("effective", ctypes.c_uint32),
        ("permitted", ctypes.c_uint32),
        ("inheritable", ctypes.c_uint32),
    ]

def fail(message):
    try:
        os.write(2, ("zk-X509 containment supervisor: " + message + "\n").encode("ascii"))
    finally:
        os._exit(FAILURE)

try:
    signal.pthread_sigmask(signal.SIG_SETMASK, set())
except (AttributeError, OSError, ValueError):
    fail("trusted supervisor signal mask could not be normalized")

def write_once(path, payload):
    descriptor = os.open(path, os.O_WRONLY | getattr(os, "O_CLOEXEC", 0))
    try:
        if os.write(descriptor, payload) != len(payload):
            fail("namespace identity map write was incomplete")
    finally:
        os.close(descriptor)

def write_all(descriptor, payload):
    view = memoryview(payload)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            fail("namespace status write made no progress")
        view = view[written:]

def read_exact(descriptor, size):
    payload = bytearray()
    while len(payload) < size:
        try:
            chunk = os.read(descriptor, size - len(payload))
        except InterruptedError:
            continue
        if not chunk:
            break
        payload.extend(chunk)
    return bytes(payload)

def target_fail(descriptor, message):
    try:
        os.write(descriptor, b"1")
    except OSError:
        pass
    fail(message)

def normalize_target_signals(error_descriptor):
    for name in ("SIGPIPE", "SIGXFZ", "SIGXFSZ"):
        target_signal = getattr(signal, name, None)
        if target_signal is None:
            continue
        try:
            signal.signal(target_signal, signal.SIG_DFL)
        except (OSError, ValueError):
            target_fail(error_descriptor, "target signal dispositions could not be normalized")

def drop_target_privileges(error_descriptor):
    try:
        descriptor = os.open(
            "/proc/sys/kernel/cap_last_cap",
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            cap_last_cap = int(os.read(descriptor, 32).strip())
        finally:
            os.close(descriptor)
    except (OSError, ValueError):
        target_fail(error_descriptor, "Linux capability ceiling is unavailable")
    if not 0 <= cap_last_cap <= 63:
        target_fail(error_descriptor, "Linux capability ceiling is unsupported")
    if prctl(PR_SET_SECUREBITS, TARGET_SECUREBITS, 0, 0, 0) != 0:
        target_fail(error_descriptor, "target securebits could not be locked")
    for capability in range(cap_last_cap + 1):
        if prctl(PR_CAPBSET_DROP, capability, 0, 0, 0) != 0:
            target_fail(error_descriptor, "target capability bounding set could not be cleared")
    header = CapabilityHeader(LINUX_CAPABILITY_VERSION_3, 0)
    data = (CapabilityData * 2)()
    if capset(ctypes.byref(header), data) != 0:
        target_fail(error_descriptor, "target capabilities could not be cleared")
    if prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0:
        target_fail(error_descriptor, "target no-new-privileges mode could not be established")
    observed = (CapabilityData * 2)()
    if capget(ctypes.byref(header), observed) != 0 or any(
        item.effective or item.permitted or item.inheritable for item in observed
    ):
        target_fail(error_descriptor, "target capability clearance could not be verified")
    if prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1:
        target_fail(error_descriptor, "target no-new-privileges mode could not be verified")

if len(sys.argv) < 6:
    fail("bootstrap arguments are invalid")
try:
    ready_descriptor = int(sys.argv[1])
    environment_descriptor = int(sys.argv[2])
    launch_authorization_descriptor = int(sys.argv[3])
    expected_controller_pid = int(sys.argv[4])
except ValueError:
    fail("bootstrap descriptors are invalid")
arguments = sys.argv[5:]
if expected_controller_pid <= 1 or not arguments or any(not item or "\0" in item for item in arguments):
    fail("target argv is invalid")

try:
    required_seals = (
        fcntl.F_SEAL_SEAL
        | fcntl.F_SEAL_SHRINK
        | fcntl.F_SEAL_GROW
        | fcntl.F_SEAL_WRITE
    )
    if fcntl.fcntl(environment_descriptor, fcntl.F_GET_SEALS) & required_seals != required_seals:
        fail("target environment is not sealed")
    os.lseek(environment_descriptor, 0, os.SEEK_SET)
    environment_payload = bytearray()
    while len(environment_payload) <= MAX_ENVIRONMENT_BYTES:
        chunk = os.read(
            environment_descriptor,
            min(1024 * 1024, MAX_ENVIRONMENT_BYTES + 1 - len(environment_payload)),
        )
        if not chunk:
            break
        environment_payload.extend(chunk)
    if not environment_payload or len(environment_payload) > MAX_ENVIRONMENT_BYTES:
        fail("target environment is empty or too large")
    target_environment = json.loads(environment_payload)
    if (
        not isinstance(target_environment, dict)
        or any(
            not isinstance(name, str)
            or not isinstance(value, str)
            or not name
            or "=" in name
            or "\0" in name
            or "\0" in value
            for name, value in target_environment.items()
        )
    ):
        fail("target environment is invalid")
finally:
    os.close(environment_descriptor)

library = ctypes.CDLL(None, use_errno=True)
try:
    unshare = library.unshare
    prctl = library.prctl
    capset = library.capset
    capget = library.capget
except AttributeError:
    fail("required Linux namespace APIs are unavailable")
unshare.argtypes = [ctypes.c_int]
unshare.restype = ctypes.c_int
prctl.argtypes = [
    ctypes.c_int,
    ctypes.c_ulong,
    ctypes.c_ulong,
    ctypes.c_ulong,
    ctypes.c_ulong,
]
prctl.restype = ctypes.c_int
capset.argtypes = [ctypes.POINTER(CapabilityHeader), ctypes.POINTER(CapabilityData)]
capset.restype = ctypes.c_int
capget.argtypes = [ctypes.POINTER(CapabilityHeader), ctypes.POINTER(CapabilityData)]
capget.restype = ctypes.c_int

namespace_init = -1
teardown_requested = False

def request_teardown(_signal_number, _frame):
    global teardown_requested
    teardown_requested = True
    if namespace_init > 0:
        try:
            os.kill(namespace_init, signal.SIGKILL)
        except ProcessLookupError:
            pass

try:
    signal.signal(signal.SIGTERM, request_teardown)
except (OSError, ValueError):
    fail("trusted teardown handler could not be installed")

if prctl(PR_SET_PDEATHSIG, signal.SIGKILL, 0, 0, 0) != 0:
    fail("controller death binding is unavailable")
if os.getppid() != expected_controller_pid:
    fail("controller exited during bootstrap")
try:
    launch_authorization = read_exact(launch_authorization_descriptor, 1)
finally:
    os.close(launch_authorization_descriptor)
if launch_authorization != b"1":
    fail("controller did not authorize target launch")

outer_uid = os.geteuid()
outer_gid = os.getegid()
if unshare(CLONE_NEWUSER | CLONE_NEWPID) != 0:
    fail("fresh user and PID namespaces are unavailable")
try:
    try:
        write_once("/proc/self/setgroups", b"deny\n")
    except FileNotFoundError:
        pass
    write_once("/proc/self/uid_map", f"{outer_uid} {outer_uid} 1\n".encode("ascii"))
    write_once("/proc/self/gid_map", f"{outer_gid} {outer_gid} 1\n".encode("ascii"))
except OSError:
    fail("fresh user namespace identity mapping failed")

lifecycle_read, lifecycle_write = os.pipe2(getattr(os, "O_CLOEXEC", 0))
status_read, status_write = os.pipe2(getattr(os, "O_CLOEXEC", 0))
try:
    namespace_init = os.fork()
except OSError:
    fail("PID namespace init could not be created")
if namespace_init == 0:
    signal.signal(signal.SIGTERM, signal.SIG_DFL)
    os.close(lifecycle_write)
    os.close(status_read)
    if os.getpid() != 1:
        fail("trusted init is not PID 1 in its private namespace")
    if prctl(PR_SET_PDEATHSIG, signal.SIGKILL, 0, 0, 0) != 0:
        fail("namespace supervisor death binding is unavailable")
    if prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) != 0:
        fail("namespace init could not be made non-dumpable")
    os.set_blocking(lifecycle_read, False)
    try:
        if os.read(lifecycle_read, 1) == b"":
            fail("namespace supervisor exited during bootstrap")
    except BlockingIOError:
        pass
    finally:
        os.close(lifecycle_read)
    exec_error_read, exec_error_write = os.pipe2(getattr(os, "O_CLOEXEC", 0))
    try:
        target_pid = os.fork()
    except OSError:
        fail("contained target could not be created")
    if target_pid == 0:
        os.close(exec_error_read)
        os.close(status_write)
        os.close(ready_descriptor)
        normalize_target_signals(exec_error_write)
        drop_target_privileges(exec_error_write)
        try:
            os.execve(arguments[0], arguments, target_environment)
        except OSError:
            target_fail(exec_error_write, "target exec failed")
    os.close(exec_error_write)
    launch_error = read_exact(exec_error_read, 1)
    os.close(exec_error_read)
    if launch_error:
        fail("contained target launch failed")
    try:
        if os.write(ready_descriptor, b"1") != 1:
            fail("containment readiness write was incomplete")
    finally:
        os.close(ready_descriptor)
    try:
        while True:
            try:
                _, target_status = os.waitpid(target_pid, 0)
                break
            except InterruptedError:
                continue
        write_all(status_write, struct.pack("=I", target_status))
    finally:
        os.close(status_write)
    os._exit(0)

if teardown_requested:
    try:
        os.kill(namespace_init, signal.SIGKILL)
    except ProcessLookupError:
        pass
os.close(lifecycle_read)
os.close(status_write)
os.close(ready_descriptor)
try:
    while True:
        try:
            _, init_status = os.waitpid(namespace_init, 0)
            break
        except InterruptedError:
            continue
finally:
    os.close(lifecycle_write)
try:
    status_payload = read_exact(status_read, 4)
finally:
    os.close(status_read)
if not os.WIFEXITED(init_status) or os.WEXITSTATUS(init_status) != 0:
    fail("namespace init did not complete cleanly")
if len(status_payload) != 4:
    fail("namespace init did not report target status")
target_status = struct.unpack("=I", status_payload)[0]
if os.WIFEXITED(target_status):
    os._exit(os.WEXITSTATUS(target_status))
if os.WIFSIGNALED(target_status):
    termination_signal = os.WTERMSIG(target_status)
    if termination_signal not in (signal.SIGKILL, signal.SIGSTOP):
        signal.signal(termination_signal, signal.SIG_DFL)
    os.kill(os.getpid(), termination_signal)
    os._exit(128 + termination_signal)
fail("target returned an invalid wait status")
"""
def _sealed_containment_environment(environment: Mapping[str, str]) -> int:
    """Return one sealed descriptor containing the exact target environment."""

    if not all(
        type(name) is str
        and type(value) is str
        and name
        and "=" not in name
        and "\0" not in name
        and "\0" not in value
        for name, value in environment.items()
    ):
        raise _BoundedProcessError("bounded subprocess environment is invalid")
    payload = json.dumps(
        dict(environment), ensure_ascii=True, sort_keys=True, separators=(",", ":")
    ).encode("ascii")
    if not 1 <= len(payload) <= _CONTAINMENT_ENVIRONMENT_MAX_BYTES:
        raise _BoundedProcessError("bounded subprocess environment exceeds its size ceiling")
    if not sys.platform.startswith("linux"):
        raise _BoundedProcessError(
            "OS-enforced descendant containment requires Linux user and PID namespaces"
        )
    if not all(
        hasattr(owner, name)
        for owner, name in (
            (os, "memfd_create"),
            (os, "MFD_ALLOW_SEALING"),
            (fcntl, "F_ADD_SEALS"),
            (fcntl, "F_GET_SEALS"),
            (fcntl, "F_SEAL_SEAL"),
            (fcntl, "F_SEAL_SHRINK"),
            (fcntl, "F_SEAL_GROW"),
            (fcntl, "F_SEAL_WRITE"),
        )
    ):
        raise _BoundedProcessError(
            "OS-enforced descendant containment lacks sealed-memory support"
        )
    descriptor = os.memfd_create(
        "iroha-zk-x509-contained-environment",
        os.MFD_ALLOW_SEALING | getattr(os, "MFD_CLOEXEC", 0),
    )
    try:
        offset = 0
        while offset < len(payload):
            offset += os.write(descriptor, payload[offset:])
        os.fchmod(descriptor, 0o400)
        required_seals = (
            fcntl.F_SEAL_SEAL
            | fcntl.F_SEAL_SHRINK
            | fcntl.F_SEAL_GROW
            | fcntl.F_SEAL_WRITE
        )
        fcntl.fcntl(descriptor, fcntl.F_ADD_SEALS, required_seals)
        if fcntl.fcntl(descriptor, fcntl.F_GET_SEALS) & required_seals != required_seals:
            raise _BoundedProcessError(
                "OS-enforced descendant containment could not seal its environment"
            )
        os.lseek(descriptor, 0, os.SEEK_SET)
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _fail(message: str) -> NoReturn:
    raise ZkX509WorkerPackageError(message)


def _request_namespace_teardown_and_reap(process: subprocess.Popen[bytes]) -> None:
    """Ask the trusted supervisor to drain its namespace, then reap it."""

    previous_signal_mask = signal.pthread_sigmask(
        signal.SIG_BLOCK, _controller_catchable_signals()
    )
    try:
        _request_namespace_teardown_and_reap_masked(process)
    finally:
        signal.pthread_sigmask(signal.SIG_SETMASK, previous_signal_mask)


def _request_namespace_teardown_and_reap_masked(
    process: subprocess.Popen[bytes],
) -> None:
    deferred_error: BaseException | None = None
    while process.poll() is None:
        try:
            process.terminate()
            break
        except ProcessLookupError:
            break
        except InterruptedError:
            continue
        except OSError:
            # Waiting forever is safer than returning while teardown is unproved.
            break
        except BaseException as error:
            if deferred_error is None:
                deferred_error = error
            continue
    while process.poll() is None:
        try:
            process.wait()
        except InterruptedError:
            continue
        except BaseException as error:
            # A catchable controller signal must not release control while the
            # namespace is still live. Preserve it until teardown is proved.
            if deferred_error is None:
                deferred_error = error
            continue
    if deferred_error is not None:
        raise deferred_error


def _controller_catchable_signals() -> set[int]:
    """Return signals whose Python handlers could unwind process creation."""

    result: set[int] = set()
    for candidate in signal.valid_signals():
        try:
            handler = signal.getsignal(candidate)
        except (OSError, ValueError):
            continue
        if callable(handler):
            result.add(int(candidate))
    return result


def _write_process_sink(descriptor: int, payload: bytes) -> None:
    view = memoryview(payload)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise _BoundedProcessError("bounded subprocess sink made no progress")
        view = view[written:]


def _atomic_rename_noreplace(
    source: str,
    destination: str,
    *,
    source_dir_fd: int,
    destination_dir_fd: int,
    label: str,
) -> None:
    """Atomically publish one name without replacing any existing entry."""

    if (
        not source
        or not destination
        or "/" in source
        or "/" in destination
        or "\0" in source
        or "\0" in destination
    ):
        _fail(f"{label} has an invalid publication name")
    library = ctypes.CDLL(None, use_errno=True)
    encoded_source = os.fsencode(source)
    encoded_destination = os.fsencode(destination)
    if sys.platform.startswith("linux"):
        operation = getattr(library, "renameat2", None)
        flag = 1  # RENAME_NOREPLACE
    elif sys.platform == "darwin":
        operation = getattr(library, "renameatx_np", None)
        flag = 0x00000004  # RENAME_EXCL
    else:
        operation = None
        flag = 0
    if operation is None:
        _fail(f"{label} requires atomic no-replace rename support")
    operation.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    ]
    operation.restype = ctypes.c_int
    ctypes.set_errno(0)
    result = operation(
        source_dir_fd,
        encoded_source,
        destination_dir_fd,
        encoded_destination,
        flag,
    )
    if result == 0:
        return
    observed_errno = ctypes.get_errno()
    if observed_errno in (errno.EEXIST, errno.ENOTEMPTY):
        _fail(f"{label} destination already exists")
    if observed_errno in (errno.ENOSYS, errno.ENOTSUP, errno.EINVAL):
        _fail(f"{label} atomic no-replace rename is unsupported")
    raise ZkX509WorkerPackageError(
        f"{label} atomic no-replace rename failed: {os.strerror(observed_errno)}"
    )


def _run_bounded_process(
    arguments: Sequence[str],
    *,
    cwd: Path | str,
    environment: Mapping[str, str],
    timeout: float,
    stdout_limit: int,
    stderr_limit: int,
    input_data: bytes | bytearray | None = None,
    pass_fds: Sequence[int] = (),
    stdout_sink: int | None = None,
    stderr_sink: int | None = None,
    capture_stdout: bool = True,
    capture_stderr: bool = True,
) -> subprocess.CompletedProcess[bytes]:
    """Stream bounded output under OS-enforced descendant containment."""

    if (
        os.name != "posix"
        or not arguments
        or any(type(item) is not str or not item for item in arguments)
        or timeout <= 0
        or stdout_limit < 0
        or stderr_limit < 0
        or (input_data is not None and len(input_data) > 64 * 1024)
        or len(set(pass_fds)) != len(tuple(pass_fds))
    ):
        raise _BoundedProcessError("bounded subprocess contract is invalid")
    if not sys.platform.startswith("linux"):
        raise _BoundedProcessError(
            "OS-enforced descendant containment requires Linux user and PID namespaces"
        )
    for sink in (stdout_sink, stderr_sink):
        if sink is not None and not stat.S_ISREG(os.fstat(sink).st_mode):
            raise _BoundedProcessError("bounded subprocess sink must be a regular file")
    for descriptor in pass_fds:
        os.fstat(descriptor)
    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    environment_descriptor = -1
    containment_read = -1
    containment_write = -1
    launch_authorization_read = -1
    launch_authorization_write = -1
    containment_status = bytearray()
    stdout = bytearray()
    stderr = bytearray()
    counts = {"stdout": 0, "stderr": 0}
    limits = {"stdout": stdout_limit, "stderr": stderr_limit}
    sinks = {"stdout": stdout_sink, "stderr": stderr_sink}
    buffers = {
        "stdout": stdout if capture_stdout else None,
        "stderr": stderr if capture_stderr else None,
    }
    deadline = time.monotonic() + timeout
    try:
        environment_descriptor = _sealed_containment_environment(environment)
        containment_read, containment_write = os.pipe()
        launch_authorization_read, launch_authorization_write = os.pipe()
        os.set_inheritable(containment_read, False)
        os.set_inheritable(containment_write, False)
        os.set_inheritable(launch_authorization_read, False)
        os.set_inheritable(launch_authorization_write, False)
        os.set_blocking(containment_read, False)
        supervisor_arguments = [
            "/proc/self/exe",
            "-I",
            "-S",
            "-c",
            _LINUX_PID_NAMESPACE_SUPERVISOR,
            str(containment_write),
            str(environment_descriptor),
            str(launch_authorization_read),
            str(os.getpid()),
            *arguments,
        ]
        controller_catchable_signals = _controller_catchable_signals()
        previous_signal_mask = signal.pthread_sigmask(
            signal.SIG_BLOCK, controller_catchable_signals
        )
        try:
            process = subprocess.Popen(
                supervisor_arguments,
                cwd=cwd,
                env=_CONTAINMENT_BOOTSTRAP_ENVIRONMENT,
                stdin=subprocess.PIPE if input_data is not None else subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                close_fds=True,
                pass_fds=tuple(pass_fds)
                + (
                    containment_write,
                    environment_descriptor,
                    launch_authorization_read,
                ),
                start_new_session=True,
            )
            os.close(launch_authorization_read)
            launch_authorization_read = -1
            try:
                if controller_catchable_signals.intersection(
                    int(item) for item in signal.sigpending()
                ):
                    raise _BoundedProcessError(
                        "controller signal interrupted containment before launch authorization"
                    )
                if os.write(launch_authorization_write, b"1") != 1:
                    raise _BoundedProcessError(
                        "OS-enforced descendant containment launch authorization was incomplete"
                    )
            finally:
                os.close(launch_authorization_write)
                launch_authorization_write = -1
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_signal_mask)
        os.close(containment_write)
        containment_write = -1
        os.close(environment_descriptor)
        environment_descriptor = -1
        selector.register(containment_read, selectors.EVENT_READ, "containment")
        assert process.stdout is not None and process.stderr is not None
        for name, stream in (("stdout", process.stdout), ("stderr", process.stderr)):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        input_view = memoryview(input_data) if input_data is not None else None
        input_offset = 0
        if input_view is not None:
            assert process.stdin is not None
            os.set_blocking(process.stdin.fileno(), False)
            selector.register(process.stdin, selectors.EVENT_WRITE, "stdin")
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise _BoundedProcessError("bounded subprocess timed out")
            events = selector.select(min(remaining, 0.25))
            for key, _ in events:
                name = key.data
                stream = key.fileobj
                if name == "containment":
                    try:
                        chunk = os.read(int(stream), 2)
                    except BlockingIOError:
                        continue
                    if chunk:
                        containment_status.extend(chunk)
                        if containment_status != b"1":
                            raise _BoundedProcessError(
                                "OS-enforced descendant containment returned invalid readiness"
                            )
                    else:
                        selector.unregister(stream)
                        os.close(int(stream))
                        containment_read = -1
                        if containment_status != b"1":
                            raise _BoundedProcessError(
                                "OS-enforced descendant containment could not be established"
                            )
                    continue
                if name == "stdin":
                    assert input_view is not None
                    try:
                        written = os.write(
                            stream.fileno(), input_view[input_offset : input_offset + 64 * 1024]
                        )
                    except BlockingIOError:
                        continue
                    except BrokenPipeError:
                        written = 0
                    input_offset += written
                    if written == 0 or input_offset == len(input_view):
                        selector.unregister(stream)
                        stream.close()
                    continue
                allowed = limits[name] - counts[name]
                try:
                    chunk = os.read(stream.fileno(), min(64 * 1024, allowed + 1))
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(stream)
                    stream.close()
                    continue
                counts[name] += len(chunk)
                if counts[name] > limits[name]:
                    raise _BoundedProcessError(
                        f"bounded subprocess {name} exceeded its byte ceiling"
                    )
                if buffers[name] is not None:
                    buffers[name].extend(chunk)
                if sinks[name] is not None:
                    _write_process_sink(int(sinks[name]), chunk)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise _BoundedProcessError("bounded subprocess timed out before reap")
        try:
            returncode = process.wait(timeout=remaining)
        except subprocess.TimeoutExpired as error:
            raise _BoundedProcessError("bounded subprocess timed out before reap") from error
        if containment_status != b"1":
            raise _BoundedProcessError(
                "OS-enforced descendant containment did not attest readiness"
            )
        return subprocess.CompletedProcess(
            list(arguments), returncode, bytes(stdout), bytes(stderr)
        )
    except BaseException:
        if process is not None:
            _request_namespace_teardown_and_reap(process)
        raise
    finally:
        selector.close()
        for descriptor in (
            containment_read,
            containment_write,
            environment_descriptor,
            launch_authorization_read,
            launch_authorization_write,
        ):
            if descriptor >= 0:
                try:
                    os.close(descriptor)
                except OSError:
                    pass
        if process is not None:
            for stream in (process.stdin, process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()


@dataclass(frozen=True)
class StableFileV1:
    path: Path
    device: int
    inode: int
    mode: int
    owner: int
    links: int
    size: int
    modified_ns: int
    sha256: str


@dataclass(frozen=True)
class ArtifactSnapshotV1:
    """One held private artifact inode used across every package boundary."""

    path: Path
    descriptor: int
    record: StableFileV1
    snapshot_identity: tuple[int, ...]


@dataclass(frozen=True)
class WorkerIdentityV2:
    cargo_lock_sha256: str
    compiled_profile_sha256: str | None
    expectations_json_sha256: str | None
    expectations_norito_sha256: str | None
    isolation_contract: str
    isolation_package_sha256: str | None
    kat_proof_bytes: int
    kat_proof_sha256: str | None
    production_profile_ready: bool
    protocol_id: str
    protocol_profile_sha256: str
    protocol_version: int
    public_request_schema_version: int
    qualified_isolation_ready: bool
    release_evidence_ready: bool
    release_evidence_sha256: str | None
    resource_certificate_sha256: str | None
    soundness_certificate_sha256: str | None
    source_allowed_signers_sha256: str
    source_closure_schema: str
    source_commit: str
    source_revocation_sha256: str
    source_sha256: str
    workspace_source_manifest_sha256: str


@dataclass(frozen=True)
class SourceEvidenceV1:
    allowed_signers_sha256: str
    cargo_lock_sha256: str
    commit: str
    raw_commit_sha256: str
    revocation_sha256: str
    signer_fingerprint: str
    signer_principal: str
    source_sha256: str
    source_date_epoch: int
    workspace_source_manifest_sha256: str


@dataclass(frozen=True)
class AuthenticatedBuildCorridorV2:
    """Exact effective inputs used for one authenticated Cargo build."""

    cargo: Path
    environment: dict[str, str]
    provenance: dict[str, object]


@dataclass(frozen=True)
class SignedSourceSnapshotV1:
    """One read-only Git-archive export held by an open directory descriptor."""

    root: Path
    descriptor: int


@dataclass(frozen=True)
class ClosedCargoHomeV1:
    """Fresh Cargo home with descriptor-anchored read-only cache roots."""

    root: Path
    cache_descriptors: tuple[int, ...]


def _require_lower_hex(value: object, digits: int, label: str) -> str:
    if (
        type(value) is not str
        or len(value) != digits
        or any(character not in "0123456789abcdef" for character in value)
    ):
        _fail(f"{label} must be exactly {digits} lowercase hexadecimal digits")
    return value


def _require_sha256(value: object, label: str) -> str:
    digest = _require_lower_hex(value, 64, label)
    if digest == "0" * 64:
        _fail(f"{label} must be nonzero")
    return digest


def _optional_sha256(value: object, label: str) -> str | None:
    if value is None:
        return None
    return _require_sha256(value, label)


def _release_evidence_sha256(
    *,
    protocol_profile_sha256: str,
    kat_proof_bytes: int,
    kat_proof_sha256: str,
    expectations_norito_sha256: str,
    expectations_json_sha256: str,
    soundness_certificate_sha256: str,
    resource_certificate_sha256: str,
) -> str:
    protocol_label = PROTOCOL_ID.encode("ascii")
    if len(protocol_label) > (1 << 16) - 1:
        _fail("zk-X509 worker protocol label is too long")
    digest = hashlib.sha256()
    digest.update(_RELEASE_EVIDENCE_DOMAIN)
    digest.update(len(protocol_label).to_bytes(2, "big"))
    digest.update(protocol_label)
    digest.update(bytes((PROTOCOL_VERSION, PUBLIC_REQUEST_SCHEMA_VERSION)))
    digest.update(bytes.fromhex(protocol_profile_sha256))
    digest.update(kat_proof_bytes.to_bytes(4, "big"))
    digest.update(bytes.fromhex(kat_proof_sha256))
    digest.update(bytes.fromhex(expectations_norito_sha256))
    digest.update(bytes.fromhex(expectations_json_sha256))
    digest.update(bytes.fromhex(soundness_certificate_sha256))
    digest.update(bytes.fromhex(resource_certificate_sha256))
    return digest.hexdigest()


def _qualified_isolation_package_sha256(artifact_sha256: str) -> str:
    """Bind the exact worker/launcher image to the reviewed isolation policy."""

    artifact = _require_sha256(artifact_sha256, "isolation artifact SHA-256")
    digest = hashlib.sha256()
    digest.update(ISOLATION_PACKAGE_DOMAIN_V1)
    digest.update(bytes.fromhex(artifact))
    digest.update(hashlib.sha256(ISOLATION_POLICY_V1).digest())
    return digest.hexdigest()


def _validate_static_aarch64_elf_bytes(payload: bytes) -> None:
    """Enforce the same static AArch64 policy at build and package verification."""

    label = "zk-X509 worker release image"
    if len(payload) < 64 or payload[:4] != b"\x7fELF":
        _fail(f"{label} is not an ELF image")
    if payload[4:7] != bytes((2, 1, 1)):
        _fail(f"{label} must be ELF64 little-endian version 1")
    try:
        header = struct.unpack_from("<16sHHIQQQIHHHHHH", payload, 0)
    except struct.error as error:
        raise ZkX509WorkerPackageError(f"{label} has a truncated ELF header") from error
    (
        _,
        elf_type,
        machine,
        version,
        _,
        program_offset,
        _,
        _,
        header_size,
        entry_size,
        entry_count,
        _,
        _,
        _,
    ) = header
    if elf_type not in (2, 3) or machine != 183 or version != 1 or header_size != 64:
        _fail(f"{label} is not a canonical Linux AArch64 executable")
    if entry_size != 56 or not 1 <= entry_count <= 4_096:
        _fail(f"{label} has an invalid program-header inventory")
    table_end = program_offset + entry_size * entry_count
    if program_offset < 64 or table_end > len(payload):
        _fail(f"{label} has a truncated program-header table")
    has_load = False
    for index in range(entry_count):
        offset = program_offset + index * entry_size
        (
            segment_type,
            flags,
            file_offset,
            _,
            _,
            file_size,
            memory_size,
            _,
        ) = struct.unpack_from("<IIQQQQQQ", payload, offset)
        if file_size > memory_size or file_offset + file_size > len(payload):
            _fail(f"{label} has an out-of-range program segment")
        if segment_type == 1:
            has_load = True
        if segment_type == 3:
            _fail(f"{label} contains PT_INTERP")
        if flags & 0x1 and flags & 0x2:
            _fail(f"{label} contains a writable executable program segment")
        if segment_type == 0x6474E551 and flags & 0x1:
            _fail(f"{label} contains an executable GNU stack")
        if segment_type == 2:
            if file_size % 16:
                _fail(f"{label} has a malformed PT_DYNAMIC segment")
            saw_null = False
            for dynamic_offset in range(file_offset, file_offset + file_size, 16):
                tag, _ = struct.unpack_from("<qQ", payload, dynamic_offset)
                if tag == 0:
                    saw_null = True
                    break
                if tag == 1:
                    _fail(f"{label} contains DT_NEEDED")
            if not saw_null:
                _fail(f"{label} PT_DYNAMIC has no DT_NULL terminator")
    if not has_load:
        _fail(f"{label} has no PT_LOAD segment")


def _validate_static_aarch64_elf(path: Path) -> None:
    payload = _stable_bytes(
        path,
        label="zk-X509 worker release image",
        maximum=_MAX_ARTIFACT_BYTES,
    )
    _validate_static_aarch64_elf_bytes(payload)


def _require_commit(value: object, label: str) -> str:
    commit = _require_lower_hex(value, 40, label)
    if commit == "0" * 40:
        _fail(f"{label} must be nonzero")
    return commit


def _require_signer_principal(value: object, label: str) -> str:
    if (
        type(value) is not str
        or not value
        or len(value.encode("utf-8")) > 1024
        or "\0" in value
        or "\n" in value
        or "\r" in value
    ):
        _fail(f"{label} is not canonical")
    return value


def _require_ssh_fingerprint(value: object, label: str) -> str:
    if type(value) is not str or _SSH_FINGERPRINT_RE.fullmatch(value) is None:
        _fail(f"{label} is not a canonical OpenSSH SHA-256 fingerprint")
    return value


def _cargo_build_command(target: str) -> tuple[str, ...]:
    """Return the sole source-authenticated worker build command."""

    if _TARGET_RE.fullmatch(target) is None:
        _fail("zk-X509 worker target is not canonical")
    return (
        "cargo",
        "build",
        "--manifest-path",
        _SIGNED_MANIFEST_TOKEN,
        "--frozen",
        "--profile",
        "release",
        "--package",
        "iroha_core",
        "--bin",
        ARTIFACT_FILE,
        "--features",
        "privacy-release-evidence",
        "--target",
        target,
    )


def _build_command_sha256(target: str) -> str:
    """Hash the exact argv with unambiguous item and length framing."""

    arguments = _cargo_build_command(target)
    digest = hashlib.sha256()
    digest.update(_BUILD_COMMAND_DOMAIN)
    digest.update(len(arguments).to_bytes(4, "big"))
    for argument in arguments:
        encoded = argument.encode("utf-8")
        digest.update(len(encoded).to_bytes(4, "big"))
        digest.update(encoded)
    return digest.hexdigest()


def _build_environment_values(source: SourceEvidenceV1) -> dict[str, str]:
    """Return the complete semantic environment for a frozen worker build."""

    if type(source.source_date_epoch) is not int or source.source_date_epoch <= 0:
        _fail("source date epoch must be a positive integer")
    return {
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "CARGO_ENCODED_RUSTFLAGS": "-C\x1ftarget-feature=+crt-static",
        "IROHA_ZK_X509_ALLOWED_SIGNERS_SHA256": source.allowed_signers_sha256,
        "IROHA_ZK_X509_CARGO_LOCK_SHA256": source.cargo_lock_sha256,
        "IROHA_ZK_X509_REVOCATION_SHA256": source.revocation_sha256,
        "IROHA_ZK_X509_RAW_SOURCE_COMMIT_SHA256": source.raw_commit_sha256,
        "IROHA_ZK_X509_SIGNER_FINGERPRINT": source.signer_fingerprint,
        "IROHA_ZK_X509_SIGNER_PRINCIPAL": source.signer_principal,
        "IROHA_ZK_X509_SIGNED_SOURCE_COMMIT": source.commit,
        "IROHA_ZK_X509_SOURCE_SHA256": source.source_sha256,
        "IROHA_ZK_X509_WORKSPACE_SOURCE_MANIFEST_SHA256": (
            source.workspace_source_manifest_sha256
        ),
        "LANG": "C",
        "LC_ALL": "C",
        "SCCACHE_CLIENT_SIDE": "1",
        "SOURCE_DATE_EPOCH": str(source.source_date_epoch),
        "VERGEN_GIT_SHA": source.commit,
    }


def _canonical_build_environment(
    value: object,
    *,
    source: SourceEvidenceV1 | None = None,
    target: str | None = None,
) -> dict[str, str]:
    environment = _plain_object(value, "zk-X509 worker build environment")
    canonical: dict[str, str] = {}
    for name, item in environment.items():
        if (
            _ENVIRONMENT_NAME_RE.fullmatch(name) is None
            or type(item) is not str
            or not item
            or len(item.encode("utf-8")) > 32 * 1024
            or "\0" in item
        ):
            _fail("zk-X509 worker build environment is not canonical")
        canonical[name] = item
    if not canonical.get("HOME") or not canonical.get("PATH"):
        _fail("zk-X509 worker build environment requires HOME and PATH")
    if source is not None:
        semantic = _build_environment_values(source)
        if any(canonical.get(name) != item for name, item in semantic.items()):
            _fail("zk-X509 worker semantic build environment is not exact")
    if target is not None:
        if _TARGET_RE.fullmatch(target) is None:
            _fail("zk-X509 worker target is not canonical")
        cargo_suffix = target.upper().replace("-", "_").replace(".", "_")
        cc_suffix = target.replace("-", "_").replace(".", "_")
        required_derived = {
            "AR",
            f"AR_{cc_suffix}",
            "CC",
            f"CC_{cc_suffix}",
            f"CARGO_TARGET_{cargo_suffix}_LINKER",
            "RUSTC",
            "RUSTC_WRAPPER",
        }
        allowed = (
            set(_INHERITED_BUILD_ENVIRONMENT_NAMES)
            | set(_build_environment_values(source))
            | required_derived
        ) if source is not None else set(canonical)
        if set(canonical) - allowed or not required_derived <= set(canonical):
            _fail("zk-X509 worker effective build environment inventory is not exact")
    return dict(sorted(canonical.items()))


def _build_environment_sha256(environment: Mapping[str, str]) -> str:
    canonical = _canonical_build_environment(dict(environment))
    digest = hashlib.sha256()
    digest.update(_BUILD_ENVIRONMENT_DOMAIN)
    digest.update(len(canonical).to_bytes(4, "big"))
    for name, item in canonical.items():
        encoded_name = name.encode("utf-8")
        encoded_item = item.encode("utf-8")
        digest.update(len(encoded_name).to_bytes(2, "big"))
        digest.update(encoded_name)
        digest.update(len(encoded_item).to_bytes(4, "big"))
        digest.update(encoded_item)
    return digest.hexdigest()


def _canonical_record_path(value: object, label: str) -> str:
    if type(value) is not str or not value or "\0" in value:
        _fail(f"{label} path is invalid")
    path = Path(value)
    if not path.is_absolute() or os.path.normpath(value) != value:
        _fail(f"{label} path is not canonical")
    return value


def _validate_build_file_record(value: object, label: str) -> dict[str, object]:
    record = _plain_object(value, label)
    if set(record) != {"mode", "owner", "path", "sha256", "size"}:
        _fail(f"{label} field inventory is not exact")
    path = _canonical_record_path(record["path"], label)
    digest = _require_sha256(record["sha256"], f"{label} SHA-256")
    size = record["size"]
    mode = record["mode"]
    owner = record["owner"]
    if (
        type(size) is not int
        or not 1 <= size <= _MAX_TOOL_FILE_BYTES
        or type(mode) is not int
        or not 0 <= mode <= 0o7777
        or mode & (stat.S_IWGRP | stat.S_IWOTH)
        or type(owner) is not int
        or not 0 <= owner <= (1 << 31) - 1
    ):
        _fail(f"{label} metadata is invalid")
    return {
        "mode": mode,
        "owner": owner,
        "path": path,
        "sha256": digest,
        "size": size,
    }


def _validate_component_record(value: object, label: str) -> dict[str, object]:
    record = _plain_object(value, label)
    if set(record) != {
        "closure_sha256",
        "file_count",
        "manifest_path",
        "manifest_sha256",
        "total_bytes",
    }:
        _fail(f"{label} field inventory is not exact")
    manifest_path = _canonical_record_path(record["manifest_path"], label)
    manifest_sha256 = _require_sha256(
        record["manifest_sha256"], f"{label} manifest SHA-256"
    )
    closure_sha256 = _require_sha256(
        record["closure_sha256"], f"{label} closure SHA-256"
    )
    file_count = record["file_count"]
    total_bytes = record["total_bytes"]
    if (
        type(file_count) is not int
        or not 1 <= file_count <= 100_000
        or type(total_bytes) is not int
        or not 1 <= total_bytes <= 16 * 1024 * 1024 * 1024
    ):
        _fail(f"{label} bounds are invalid")
    return {
        "closure_sha256": closure_sha256,
        "file_count": file_count,
        "manifest_path": manifest_path,
        "manifest_sha256": manifest_sha256,
        "total_bytes": total_bytes,
    }


def _validate_cargo_cache_root_record(value: object, role: str) -> dict[str, object]:
    record = _plain_object(value, f"Cargo {role} cache root")
    if set(record) != {
        "device",
        "entry_count",
        "inode",
        "mode",
        "owner",
        "path",
        "role",
        "total_file_bytes",
        "tree_sha256",
    }:
        _fail(f"Cargo {role} cache-root field inventory is not exact")
    if record["role"] != role:
        _fail(f"Cargo {role} cache-root role is invalid")
    path = _canonical_record_path(record["path"], f"Cargo {role} cache root")
    integers = {
        name: record[name]
        for name in (
            "device",
            "entry_count",
            "inode",
            "mode",
            "owner",
            "total_file_bytes",
        )
    }
    if any(type(item) is not int or item < 0 for item in integers.values()):
        _fail(f"Cargo {role} cache-root metadata is invalid")
    if (
        not 0 <= integers["mode"] <= 0o7777
        or integers["mode"] & (stat.S_IWGRP | stat.S_IWOTH)
        or integers["entry_count"] > _MAX_CARGO_CACHE_ENTRIES
        or integers["total_file_bytes"] > _MAX_CARGO_CACHE_TOTAL_BYTES
    ):
        _fail(f"Cargo {role} cache-root bounds are invalid")
    return {
        "device": integers["device"],
        "entry_count": integers["entry_count"],
        "inode": integers["inode"],
        "mode": integers["mode"],
        "owner": integers["owner"],
        "path": path,
        "role": role,
        "total_file_bytes": integers["total_file_bytes"],
        "tree_sha256": _require_sha256(
            record["tree_sha256"], f"Cargo {role} cache-tree SHA-256"
        ),
    }


def _canonical_build_toolchain(value: object, target: str) -> dict[str, object]:
    toolchain = _plain_object(value, "zk-X509 worker build toolchain")
    if set(toolchain) != {
        "cargo_cache_roots",
        "cargo_configuration",
        "cargo_version_sha256",
        "components",
        "host",
        "rustc_version_sha256",
        "schema",
        "sysroot",
        "target",
        "tools",
    }:
        _fail("zk-X509 worker build toolchain field inventory is not exact")
    if (
        toolchain["schema"] != _BUILD_TOOLCHAIN_SCHEMA
        or toolchain["target"] != target
        or type(toolchain["host"]) is not str
        or _TARGET_RE.fullmatch(toolchain["host"]) is None
    ):
        _fail("zk-X509 worker build toolchain identity is invalid")
    sysroot = _canonical_record_path(toolchain["sysroot"], "Rust sysroot")
    tools = _plain_object(toolchain["tools"], "zk-X509 worker build tools")
    if set(tools) != set(_BUILD_TOOL_ROLES):
        _fail("zk-X509 worker build tool inventory is not exact")
    canonical_tools = {
        role: _validate_build_file_record(tools[role], f"build tool {role}")
        for role in _BUILD_TOOL_ROLES
    }
    if any(not int(record["mode"]) & stat.S_IXUSR for record in canonical_tools.values()):
        _fail("zk-X509 worker captured build tools must be owner-executable")
    cargo_configuration = toolchain["cargo_configuration"]
    if type(cargo_configuration) is not list or len(cargo_configuration) > 64:
        _fail("zk-X509 worker Cargo configuration inventory is invalid")
    canonical_cargo_configuration = [
        _validate_build_file_record(item, "Cargo configuration")
        for item in cargo_configuration
    ]
    configuration_paths = [item["path"] for item in canonical_cargo_configuration]
    if configuration_paths != sorted(set(configuration_paths)):
        _fail("zk-X509 worker Cargo configuration inventory is not canonical")
    cargo_cache_roots = _plain_object(
        toolchain["cargo_cache_roots"], "zk-X509 Cargo cache roots"
    )
    if not set(cargo_cache_roots) <= {"git", "registry"}:
        _fail("zk-X509 worker Cargo cache-root inventory is invalid")
    canonical_cargo_cache_roots = {
        role: _validate_cargo_cache_root_record(cargo_cache_roots[role], role)
        for role in sorted(cargo_cache_roots)
    }
    components = _plain_object(toolchain["components"], "zk-X509 Rust components")
    if set(components) != set(_RUST_COMPONENT_ROLES):
        _fail("zk-X509 worker Rust component inventory is not exact")
    canonical_components = {
        role: _validate_component_record(components[role], f"Rust component {role}")
        for role in _RUST_COMPONENT_ROLES
    }
    expected_manifests = {
        "cargo": os.fspath(
            Path(sysroot) / "lib" / "rustlib" / f"manifest-cargo-{toolchain['host']}"
        ),
        "rust_std": os.fspath(
            Path(sysroot) / "lib" / "rustlib" / f"manifest-rust-std-{target}"
        ),
        "rustc": os.fspath(
            Path(sysroot) / "lib" / "rustlib" / f"manifest-rustc-{toolchain['host']}"
        ),
    }
    if any(
        canonical_components[role]["manifest_path"] != expected_path
        for role, expected_path in expected_manifests.items()
    ):
        _fail("zk-X509 worker Rust component paths are not toolchain-bound")
    cargo_version = _require_sha256(
        toolchain["cargo_version_sha256"], "Cargo version output SHA-256"
    )
    rustc_version = _require_sha256(
        toolchain["rustc_version_sha256"], "rustc version output SHA-256"
    )
    return {
        "cargo_cache_roots": canonical_cargo_cache_roots,
        "cargo_configuration": canonical_cargo_configuration,
        "cargo_version_sha256": cargo_version,
        "components": canonical_components,
        "host": toolchain["host"],
        "rustc_version_sha256": rustc_version,
        "schema": _BUILD_TOOLCHAIN_SCHEMA,
        "sysroot": sysroot,
        "target": target,
        "tools": canonical_tools,
    }


def _build_toolchain_sha256(toolchain: object, target: str) -> str:
    canonical = _canonical_build_toolchain(toolchain, target)
    digest = hashlib.sha256()
    digest.update(_BUILD_TOOLCHAIN_DOMAIN)
    digest.update(_canonical_json_bytes(canonical))
    return digest.hexdigest()


def _build_provenance_v2(
    environment: Mapping[str, str],
    toolchain: object,
    *,
    source: SourceEvidenceV1,
    target: str,
) -> dict[str, object]:
    canonical_environment = _canonical_build_environment(
        dict(environment), source=source, target=target
    )
    canonical_toolchain = _canonical_build_toolchain(toolchain, target)
    cargo_suffix = target.upper().replace("-", "_").replace(".", "_")
    cc_suffix = target.replace("-", "_").replace(".", "_")
    tools = canonical_toolchain["tools"]
    assert isinstance(tools, dict)
    sysroot = Path(str(canonical_toolchain["sysroot"]))
    if (
        tools["cargo"]["path"] != os.fspath(sysroot / "bin" / "cargo")
        or tools["rustc"]["path"] != os.fspath(sysroot / "bin" / "rustc")
    ):
        _fail("zk-X509 worker Cargo/rustc paths are not sysroot-bound")
    if (
        canonical_environment["RUSTC"] != tools["rustc"]["path"]
        or canonical_environment["RUSTC_WRAPPER"]
        != tools["rustc_wrapper"]["path"]
    ):
        _fail("zk-X509 worker RUSTC does not match the captured toolchain")
    if (
        canonical_environment["CC"] != tools["linker_driver"]["path"]
        or canonical_environment[f"CC_{cc_suffix}"]
        != tools["linker_driver"]["path"]
        or canonical_environment[f"CARGO_TARGET_{cargo_suffix}_LINKER"]
        != tools["linker_driver"]["path"]
        or canonical_environment["AR"] != tools["archiver"]["path"]
        or canonical_environment[f"AR_{cc_suffix}"] != tools["archiver"]["path"]
    ):
        _fail("zk-X509 worker compiler/linker environment is not tool-bound")
    return {
        "environment": canonical_environment,
        "environment_sha256": _build_environment_sha256(canonical_environment),
        "schema": _BUILD_PROVENANCE_SCHEMA,
        "target": target,
        "toolchain": canonical_toolchain,
        "toolchain_sha256": _build_toolchain_sha256(canonical_toolchain, target),
    }


def _validate_build_provenance_v2(
    value: object,
    *,
    source: SourceEvidenceV1,
    target: str,
) -> dict[str, object]:
    provenance = _plain_object(value, "zk-X509 worker build provenance")
    if set(provenance) != {
        "environment",
        "environment_sha256",
        "schema",
        "target",
        "toolchain",
        "toolchain_sha256",
    } or provenance.get("schema") != _BUILD_PROVENANCE_SCHEMA:
        _fail("zk-X509 worker build provenance field inventory is not exact")
    canonical = _build_provenance_v2(
        _plain_object(provenance["environment"], "build environment"),
        provenance["toolchain"],
        source=source,
        target=target,
    )
    if provenance["target"] != target:
        _fail("zk-X509 worker build provenance target is invalid")
    if not hmac.compare_digest(
        _require_sha256(
            provenance["environment_sha256"], "build environment SHA-256"
        ),
        canonical["environment_sha256"],
    ) or not hmac.compare_digest(
        _require_sha256(
            provenance["toolchain_sha256"], "build toolchain SHA-256"
        ),
        canonical["toolchain_sha256"],
    ):
        _fail("zk-X509 worker authenticated build provenance is invalid")
    return canonical


def _plain_object(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        _fail(f"{label} must be a JSON object")
    return value


def _reject_duplicate_pairs(
    pairs: list[tuple[str, object]],
) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            _fail(f"zk-X509 worker package contains duplicate key {key!r}")
        value[key] = item
    return value


def _canonical_json_bytes(value: object) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=True,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")


def _canonical_absolute_path(value: Path, label: str) -> Path:
    if not value.is_absolute() or value != value.resolve(strict=False):
        _fail(f"{label} must be a canonical absolute path")
    return value


def _read_stable_file(
    path: Path,
    *,
    label: str,
    maximum: int,
    allow_empty: bool = False,
    require_executable: bool = False,
    require_owner: bool = False,
    capture_payload: bool = False,
) -> tuple[StableFileV1, bytes | None]:
    try:
        before = path.lstat()
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} is unavailable: {path}") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or not (0 if allow_empty else 1) <= before.st_size <= maximum
        or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or (require_executable and not before.st_mode & stat.S_IXUSR)
        or (require_owner and before.st_uid != os.geteuid())
    ):
        _fail(f"{label} must be one bounded owner-controlled regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} could not be opened") from error
    digest = hashlib.sha256()
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            _fail(f"{label} changed while it was opened")
        remaining = opened.st_size
        payload = bytearray() if capture_payload else None
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                _fail(f"{label} was truncated while it was hashed")
            digest.update(chunk)
            if payload is not None:
                payload.extend(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail(f"{label} grew while it was hashed")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    before_identity = (
        opened.st_dev,
        opened.st_ino,
        opened.st_mode,
        opened.st_uid,
        opened.st_nlink,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
    )
    after_identity = (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_uid,
        after.st_nlink,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    )
    if before_identity != after_identity:
        _fail(f"{label} changed while it was hashed")
    return (
        StableFileV1(
            path=path,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=opened.st_mode,
            owner=opened.st_uid,
            links=opened.st_nlink,
            size=opened.st_size,
            modified_ns=opened.st_mtime_ns,
            sha256=digest.hexdigest(),
        ),
        bytes(payload) if payload is not None else None,
    )


def _stable_file(
    path: Path,
    *,
    label: str,
    maximum: int,
    allow_empty: bool = False,
    require_executable: bool = False,
    require_owner: bool = False,
) -> StableFileV1:
    identity, _ = _read_stable_file(
        path,
        label=label,
        maximum=maximum,
        allow_empty=allow_empty,
        require_executable=require_executable,
        require_owner=require_owner,
    )
    return identity


def _stable_bytes(path: Path, *, label: str, maximum: int) -> bytes:
    identity, payload = _read_stable_file(
        path,
        label=label,
        maximum=maximum,
        capture_payload=True,
    )
    if payload is None or len(payload) != identity.size:
        _fail(f"{label} could not be read completely")
    return payload


def _file_open_identity(details: os.stat_result) -> tuple[int, ...]:
    return (
        details.st_dev,
        details.st_ino,
        details.st_mode,
        details.st_uid,
        details.st_nlink,
        details.st_size,
        details.st_mtime_ns,
        details.st_ctime_ns,
    )


def _snapshot_digest(descriptor: int, size: int) -> str:
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(descriptor, min(1024 * 1024, size - offset), offset)
        if not chunk:
            _fail("immutable zk-X509 artifact snapshot was truncated")
        digest.update(chunk)
        offset += len(chunk)
    if os.pread(descriptor, 1, size):
        _fail("immutable zk-X509 artifact snapshot grew")
    return digest.hexdigest()


def _revalidate_artifact_snapshot(
    snapshot: ArtifactSnapshotV1, expected_sha256: str
) -> None:
    expected_sha256 = _require_sha256(expected_sha256, "artifact snapshot SHA-256")
    before = os.fstat(snapshot.descriptor)
    if _file_open_identity(before) != snapshot.snapshot_identity:
        _fail("immutable zk-X509 artifact snapshot metadata changed")
    named = snapshot.path.lstat()
    if _file_open_identity(named) != snapshot.snapshot_identity:
        _fail("immutable zk-X509 artifact snapshot path changed")
    observed = _snapshot_digest(snapshot.descriptor, before.st_size)
    after = os.fstat(snapshot.descriptor)
    if (
        _file_open_identity(after) != snapshot.snapshot_identity
        or not hmac.compare_digest(observed, expected_sha256)
    ):
        _fail("immutable zk-X509 artifact snapshot bytes changed")


def _artifact_snapshot_payload(snapshot: ArtifactSnapshotV1) -> bytes:
    _revalidate_artifact_snapshot(snapshot, snapshot.record.sha256)
    payload = bytearray()
    offset = 0
    while offset < snapshot.record.size:
        chunk = os.pread(
            snapshot.descriptor,
            min(1024 * 1024, snapshot.record.size - offset),
            offset,
        )
        if not chunk:
            _fail("immutable zk-X509 artifact snapshot was truncated")
        payload.extend(chunk)
        offset += len(chunk)
    if not hmac.compare_digest(
        hashlib.sha256(payload).hexdigest(), snapshot.record.sha256
    ):
        _fail("immutable zk-X509 artifact snapshot payload changed")
    _revalidate_artifact_snapshot(snapshot, snapshot.record.sha256)
    return bytes(payload)


@contextlib.contextmanager
def _immutable_artifact_snapshot(
    path: Path,
    label: str,
    *,
    source_dir_fd: int | None = None,
    source_name: str | None = None,
):
    """Copy one opened artifact inode once, then retain that exact read-only copy."""

    path = _canonical_absolute_path(path, label)
    if (source_dir_fd is None) is not (source_name is None):
        _fail(f"{label} descriptor anchor is incomplete")
    try:
        named_before = (
            path.lstat()
            if source_dir_fd is None
            else os.stat(
                str(source_name),
                dir_fd=source_dir_fd,
                follow_symlinks=False,
            )
        )
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} is unavailable") from error
    if (
        not stat.S_ISREG(named_before.st_mode)
        or stat.S_ISLNK(named_before.st_mode)
        or named_before.st_nlink != 1
        or named_before.st_uid != os.geteuid()
        or named_before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or not named_before.st_mode & stat.S_IXUSR
        or not 1 <= named_before.st_size <= _MAX_ARTIFACT_BYTES
    ):
        _fail(f"{label} must be one bounded owner-controlled executable")
    source = os.open(
        path if source_dir_fd is None else str(source_name),
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        **({} if source_dir_fd is None else {"dir_fd": source_dir_fd}),
    )
    root: Path | None = None
    root_descriptor = -1
    snapshot_path: Path | None = None
    writer = -1
    snapshot_descriptor = -1
    try:
        root = Path(
            tempfile.mkdtemp(
                prefix="iroha-zk-x509-artifact-",
                dir=_validated_temporary_parent(label),
            )
        ).resolve(strict=True)
        root.chmod(0o700)
        root_descriptor = _open_directory_descriptor(root, f"{label} snapshot root")
        snapshot_path = root / ARTIFACT_FILE
        opened = os.fstat(source)
        source_identity = _file_open_identity(opened)
        if source_identity != _file_open_identity(named_before):
            _fail(f"{label} changed while it was opened")
        writer = os.open(
            ARTIFACT_FILE,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o500,
            dir_fd=root_descriptor,
        )
        digest = hashlib.sha256()
        copied = 0
        while copied < opened.st_size:
            chunk = os.pread(
                source, min(1024 * 1024, opened.st_size - copied), copied
            )
            if not chunk:
                _fail(f"{label} was truncated while snapshotted")
            digest.update(chunk)
            _write_process_sink(writer, chunk)
            copied += len(chunk)
        if os.pread(source, 1, copied):
            _fail(f"{label} grew while snapshotted")
        os.fchmod(writer, 0o500)
        os.fsync(writer)
        source_after = os.fstat(source)
        path_after = (
            path.lstat()
            if source_dir_fd is None
            else os.stat(
                str(source_name),
                dir_fd=source_dir_fd,
                follow_symlinks=False,
            )
        )
        if (
            _file_open_identity(source_after) != source_identity
            or _file_open_identity(path_after) != source_identity
            or copied != opened.st_size
        ):
            _fail(f"{label} changed while its immutable snapshot was created")
        record = StableFileV1(
            path=path,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=opened.st_mode,
            owner=opened.st_uid,
            links=opened.st_nlink,
            size=opened.st_size,
            modified_ns=opened.st_mtime_ns,
            sha256=digest.hexdigest(),
        )
        os.close(writer)
        writer = -1
        snapshot_descriptor = os.open(
            ARTIFACT_FILE,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=root_descriptor,
        )
        snapshot_details = os.fstat(snapshot_descriptor)
        snapshot_identity = _file_open_identity(snapshot_details)
        if (
            not stat.S_ISREG(snapshot_details.st_mode)
            or snapshot_details.st_uid != os.geteuid()
            or snapshot_details.st_nlink != 1
            or stat.S_IMODE(snapshot_details.st_mode) != 0o500
            or snapshot_details.st_size != record.size
            or not hmac.compare_digest(
                _snapshot_digest(snapshot_descriptor, snapshot_details.st_size),
                record.sha256,
            )
        ):
            _fail(f"{label} immutable snapshot is invalid")
        os.fchmod(root_descriptor, 0o500)
        assert snapshot_path is not None
        snapshot = ArtifactSnapshotV1(
            path=snapshot_path,
            descriptor=snapshot_descriptor,
            record=record,
            snapshot_identity=snapshot_identity,
        )
        _revalidate_artifact_snapshot(snapshot, record.sha256)
        yield snapshot
        _revalidate_artifact_snapshot(snapshot, record.sha256)
    finally:
        if writer >= 0:
            os.close(writer)
        os.close(source)
        if root_descriptor >= 0 and root is not None:
            try:
                os.fchmod(root_descriptor, 0o700)
                if snapshot_descriptor >= 0:
                    held = os.fstat(snapshot_descriptor)
                    try:
                        named = os.stat(
                            ARTIFACT_FILE,
                            dir_fd=root_descriptor,
                            follow_symlinks=False,
                        )
                    except OSError:
                        named = None
                    if named is not None and (named.st_dev, named.st_ino) == (
                        held.st_dev,
                        held.st_ino,
                    ):
                        os.unlink(ARTIFACT_FILE, dir_fd=root_descriptor)
            finally:
                if snapshot_descriptor >= 0:
                    os.close(snapshot_descriptor)
                root_held = os.fstat(root_descriptor)
                os.close(root_descriptor)
                try:
                    root_named = root.lstat()
                    if (root_named.st_dev, root_named.st_ino) == (
                        root_held.st_dev,
                        root_held.st_ino,
                    ):
                        root.rmdir()
                except OSError:
                    pass


def _source_closure_paths(source_root: Path) -> tuple[PurePosixPath, ...]:
    manifest_path = source_root / SOURCE_CLOSURE_MANIFEST
    payload = _stable_bytes(
        manifest_path,
        label="zk-X509 worker source-closure manifest",
        maximum=64 * 1024,
    )
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 worker source-closure manifest is not UTF-8"
        ) from error
    if not text.endswith("\n") or "\r" in text:
        _fail("zk-X509 worker source-closure manifest is not canonical LF text")
    raw_paths = text.splitlines()
    paths: list[PurePosixPath] = []
    for raw in raw_paths:
        path = PurePosixPath(raw)
        if (
            not raw
            or raw.startswith("/")
            or path.as_posix() != raw
            or any(part in ("", ".", "..") for part in path.parts)
        ):
            _fail("zk-X509 worker source-closure manifest has a non-canonical path")
        paths.append(path)
    if len(paths) != len(set(paths)) or paths != sorted(paths, key=lambda item: item.as_posix()):
        _fail("zk-X509 worker source-closure paths must be unique and sorted")

    x509_root = source_root / "crates/iroha_core/src/privacy_engines/zk_x509"
    try:
        actual_x509 = {
            PurePosixPath(path.relative_to(source_root).as_posix())
            for path in x509_root.rglob("*")
            if path.is_file() and not path.is_symlink()
        }
    except OSError as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 source inventory could not be enumerated"
        ) from error
    listed_x509 = {
        path
        for path in paths
        if path.as_posix().startswith(
            "crates/iroha_core/src/privacy_engines/zk_x509/"
        )
    }
    if listed_x509 != actual_x509:
        missing = sorted(actual_x509 - listed_x509, key=lambda item: item.as_posix())
        extra = sorted(listed_x509 - actual_x509, key=lambda item: item.as_posix())
        detail = ", ".join(
            [*(f"missing:{path}" for path in missing), *(f"extra:{path}" for path in extra)]
        )
        _fail("zk-X509 worker source closure is not exhaustive" + (f": {detail}" if detail else ""))
    return tuple(paths)


def source_closure_sha256(source_root: Path) -> str:
    """Hash the canonical worker source closure with paths and lengths."""

    source_root = source_root.resolve(strict=True)
    manifest_payload = _stable_bytes(
        source_root / SOURCE_CLOSURE_MANIFEST,
        label="zk-X509 worker source-closure manifest",
        maximum=64 * 1024,
    )
    paths = _source_closure_paths(source_root)
    digest = hashlib.sha256()
    digest.update(_SOURCE_CLOSURE_DOMAIN)
    digest.update(len(manifest_payload).to_bytes(8, "big"))
    digest.update(manifest_payload)
    digest.update(len(paths).to_bytes(4, "big"))
    for path in paths:
        encoded_path = path.as_posix().encode("utf-8")
        if len(encoded_path) > (1 << 16) - 1:
            _fail("zk-X509 worker source-closure path is too long")
        payload = _stable_bytes(
            source_root.joinpath(*path.parts),
            label=f"zk-X509 worker source {path}",
            maximum=64 * 1024 * 1024,
        )
        digest.update(len(encoded_path).to_bytes(2, "big"))
        digest.update(encoded_path)
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)
    return digest.hexdigest()


def _git_environment() -> dict[str, str]:
    # Git accepts several environment variables which can redirect the object
    # database, index, work tree, configuration, or even the repository itself.
    # None of those caller-controlled redirects are admissible while proving the
    # identity of the explicitly resolved source tree.
    # Do not carry caller-controlled PATH, dynamic-loader, locale-plugin, SSH,
    # or home-directory state into source authentication.  Both programs are
    # named by their system paths below; this PATH is only for their own
    # system subprocesses.
    environment = {
        "HOME": os.path.abspath(os.sep),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }
    environment["GIT_CONFIG_GLOBAL"] = os.devnull
    environment["GIT_CONFIG_NOSYSTEM"] = "1"
    environment["GIT_NO_LAZY_FETCH"] = "1"
    environment["GIT_NO_REPLACE_OBJECTS"] = "1"
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    return environment


def _git(source_root: Path, arguments: Sequence[str], *, timeout: int = 60) -> str:
    try:
        result = _run_bounded_process(
            [_SYSTEM_GIT, "-C", os.fspath(source_root), *arguments],
            cwd=source_root,
            environment=_git_environment(),
            timeout=timeout,
            stdout_limit=4 * 1024 * 1024,
            stderr_limit=_MAX_TOOL_OUTPUT_BYTES,
        )
    except (OSError, _BoundedProcessError) as error:
        raise ZkX509WorkerPackageError("Git source authentication failed") from error
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip()
        _fail("Git source authentication failed" + (f": {detail}" if detail else ""))
    try:
        return result.stdout.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ZkX509WorkerPackageError("Git source output is not UTF-8") from error


def _git_bytes(
    source_root: Path, arguments: Sequence[str], *, timeout: int = 60
) -> bytes:
    """Run one byte-preserving Git plumbing query in the closed environment."""

    try:
        result = _run_bounded_process(
            [_SYSTEM_GIT, "-C", os.fspath(source_root), *arguments],
            cwd=source_root,
            environment=_git_environment(),
            timeout=timeout,
            stdout_limit=4 * 1024 * 1024,
            stderr_limit=_MAX_TOOL_OUTPUT_BYTES,
        )
    except (OSError, _BoundedProcessError) as error:
        raise ZkX509WorkerPackageError("Git source authentication failed") from error
    if result.returncode != 0:
        _fail("Git source authentication failed")
    return result.stdout


def _require_source_checkout_identity_v1(source_root: Path) -> None:
    """Require this script and ``--source-root`` to name the same checkout."""

    source_root = source_root.resolve(strict=True)
    try:
        running_script = Path(__file__).resolve(strict=True)
        expected_script = (source_root / _PACKAGING_SCRIPT_RELATIVE).resolve(
            strict=True
        )
    except OSError as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 packaging script checkout identity is unavailable"
        ) from error
    if running_script != expected_script:
        _fail("zk-X509 packaging script checkout does not equal --source-root")
    top_level_text = _git(source_root, ("rev-parse", "--show-toplevel")).strip()
    try:
        top_level = Path(top_level_text).resolve(strict=True)
    except OSError as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 --source-root Git checkout identity is invalid"
        ) from error
    if not top_level_text or top_level != source_root:
        _fail("zk-X509 --source-root must be the exact Git checkout root")


def _authenticate_source_helpers_v1(source_root: Path, commit: str) -> None:
    """Bind every executed packaging helper to its signed source revision."""

    source_root = source_root.resolve(strict=True)
    commit = _require_commit(commit, "source commit")
    for relative in _AUTHENTICATED_HELPER_RELATIVES:
        label = f"authenticated source helper {relative.as_posix()}"
        working_bytes = _stable_bytes(
            source_root / relative,
            label=label,
            maximum=4 * 1024 * 1024,
        )
        committed_bytes = _git_bytes(
            source_root,
            ("cat-file", "blob", f"{commit}:{relative.as_posix()}"),
        )
        if not committed_bytes or not hmac.compare_digest(
            hashlib.sha256(working_bytes).digest(),
            hashlib.sha256(committed_bytes).digest(),
        ) or working_bytes != committed_bytes:
            _fail(f"{label} does not match the signed source revision")


def _parse_commit_headers(raw_commit: bytes) -> list[tuple[bytes, bytes]]:
    """Parse the raw commit header block without normalizing signature bytes."""

    separator = raw_commit.find(b"\n\n")
    if separator < 0:
        _fail("raw commit object has no header/message boundary")
    headers: list[tuple[bytes, bytes]] = []
    current_name: bytes | None = None
    current_value = bytearray()
    for line in raw_commit[:separator].split(b"\n"):
        if line.startswith(b" "):
            if current_name is None:
                _fail("raw commit object has an orphan continuation header")
            current_value.extend(b"\n")
            current_value.extend(line[1:])
            continue
        if current_name is not None:
            headers.append((current_name, bytes(current_value)))
        name, marker, value = line.partition(b" ")
        if not marker or not name or re.fullmatch(rb"[a-z0-9-]+", name) is None:
            _fail("raw commit object contains a malformed header")
        current_name = name
        current_value = bytearray(value)
    if current_name is not None:
        headers.append((current_name, bytes(current_value)))
    return headers


def _require_exact_one_ssh_signature(raw_commit: bytes) -> bytes:
    headers = _parse_commit_headers(raw_commit)
    signatures = [value for name, value in headers if name == b"gpgsig"]
    if len(signatures) != 1 or any(name.startswith(b"gpgsig-") for name, _ in headers):
        _fail("source commit must contain exactly one ordinary gpgsig header")
    signature = signatures[0]
    if (
        not signature.startswith(b"-----BEGIN SSH SIGNATURE-----\n")
        or not signature.endswith(b"\n-----END SSH SIGNATURE-----")
        or signature.count(b"-----BEGIN SSH SIGNATURE-----") != 1
        or signature.count(b"-----END SSH SIGNATURE-----") != 1
        or b"PGP SIGNATURE" in signature
    ):
        _fail("the sole source-commit signature must be canonical SSH armor")
    return signature


def _validated_temporary_parent(label: str) -> Path:
    try:
        parent = Path(tempfile.gettempdir()).resolve(strict=True)
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} temporary root is unavailable") from error
    for directory in (parent, *parent.parents):
        details = directory.lstat()
        controlled_owner = details.st_uid in (0, os.geteuid())
        writable = bool(details.st_mode & (stat.S_IWGRP | stat.S_IWOTH))
        sticky = bool(details.st_mode & stat.S_ISVTX)
        if (
            not stat.S_ISDIR(details.st_mode)
            or stat.S_ISLNK(details.st_mode)
            or not controlled_owner
            or (writable and not sticky)
        ):
            _fail(f"{label} temporary ancestor is not owner/root controlled")
    return parent


def _verify_source_signature(
    source_root: Path,
    commit: str,
    allowed_signers: bytes,
    revocation: bytes,
) -> tuple[str, str, str]:
    """Authenticate one raw commit and return its OOB-comparable signer identity."""

    raw_commit = _git_bytes(source_root, ("cat-file", "commit", commit))
    _require_exact_one_ssh_signature(raw_commit)
    raw_object_id = hashlib.sha1(
        b"commit " + str(len(raw_commit)).encode("ascii") + b"\0" + raw_commit
    ).hexdigest()
    if not hmac.compare_digest(raw_object_id, commit):
        _fail("raw commit bytes do not hash to the resolved source commit")
    with tempfile.TemporaryDirectory(
        prefix="iroha-zk-x509-ssh-policy-",
        dir=_validated_temporary_parent("SSH policy"),
    ) as temporary:
        root = Path(temporary).resolve(strict=True)
        root.chmod(0o700)
        root_metadata = root.lstat()
        if (
            root_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(root_metadata.st_mode) != 0o700
        ):
            _fail("private SSH policy directory is not owner-private")
        policy_paths: list[Path] = []
        for name, payload in (
            ("allowed-signers", allowed_signers),
            ("revocation", revocation),
        ):
            path = root / name
            descriptor = os.open(
                path,
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
                0o400,
            )
            try:
                offset = 0
                while offset < len(payload):
                    offset += os.write(descriptor, payload[offset:])
                os.fchmod(descriptor, 0o400)
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
            policy_paths.append(path)
        private_allowed, private_revocation = policy_paths
        try:
            completed = _run_bounded_process(
                [
                    _SYSTEM_GIT,
                    "-C",
                    os.fspath(source_root),
                    "-c",
                    "gpg.format=ssh",
                    "-c",
                    f"gpg.ssh.program={_SYSTEM_SSH_KEYGEN}",
                    "-c",
                    f"gpg.ssh.allowedSignersFile={private_allowed}",
                    "-c",
                    f"gpg.ssh.revocationFile={private_revocation}",
                    "verify-commit",
                    "--raw",
                    commit,
                ],
                cwd=source_root,
                environment=_git_environment(),
                timeout=60,
                stdout_limit=_MAX_TOOL_OUTPUT_BYTES,
                stderr_limit=_MAX_TOOL_OUTPUT_BYTES,
            )
        except (OSError, _BoundedProcessError) as error:
            raise ZkX509WorkerPackageError("SSH source authentication failed") from error
        repeated_allowed, repeated_allowed_payload = _read_stable_file(
            private_allowed,
            label="private SSH allowed-signers policy",
            maximum=_MAX_POLICY_BYTES,
            capture_payload=True,
        )
        repeated_revocation, repeated_revocation_payload = _read_stable_file(
            private_revocation,
            label="private SSH revocation policy",
            maximum=_MAX_POLICY_BYTES,
            allow_empty=True,
            capture_payload=True,
        )
        if (
            repeated_allowed_payload != allowed_signers
            or repeated_revocation_payload != revocation
            or repeated_allowed.owner != os.geteuid()
            or repeated_revocation.owner != os.geteuid()
            or stat.S_IMODE(repeated_allowed.mode) != 0o400
            or stat.S_IMODE(repeated_revocation.mode) != 0o400
        ):
            _fail("private SSH policy changed during source authentication")
    if (
        completed.returncode != 0
        or len(completed.stdout) > _MAX_TOOL_OUTPUT_BYTES
        or len(completed.stderr) > _MAX_TOOL_OUTPUT_BYTES
    ):
        _fail("SSH source authentication failed")
    report = (completed.stdout + b"\n" + completed.stderr).decode(
        "utf-8", "replace"
    )
    matches = re.findall(
        r'^Good "git" signature for (.+) with ([A-Za-z0-9_-]+) key '
        r'(SHA256:[A-Za-z0-9+/]{43})$',
        report,
        re.MULTILINE,
    )
    if len(matches) != 1:
        _fail("Git did not report exactly one good SSH source-commit signature")
    principal, _key_type, fingerprint = matches[0]
    return hashlib.sha256(raw_commit).hexdigest(), principal, fingerprint


def _signed_source_helper_bytes(source_root: Path, commit: str) -> bytes:
    """Read the exact helper blob reached through the authenticated commit tree."""

    raw_commit = _git_bytes(source_root, ("cat-file", "commit", commit))
    tree_headers = [
        value for name, value in _parse_commit_headers(raw_commit) if name == b"tree"
    ]
    if len(tree_headers) != 1 or re.fullmatch(rb"[0-9a-f]{40}", tree_headers[0]) is None:
        _fail("signed source helper commit tree is invalid")
    inventory = _signed_tree_inventory(source_root, tree_headers[0].decode("ascii"))
    relative = PurePosixPath(_WORKSPACE_MANIFEST_HELPER_RELATIVE.as_posix())
    entry = inventory.get(relative)
    if entry is None or entry[0] not in {"100644", "100755"}:
        _fail("signed source identity helper is not one regular committed blob")
    payload = _git_bytes(source_root, ("cat-file", "blob", entry[1]))
    observed = hashlib.sha1(
        b"blob " + str(len(payload)).encode("ascii") + b"\0" + payload
    ).hexdigest()
    if not payload or not hmac.compare_digest(observed, entry[1]):
        _fail("signed source identity helper bytes do not hash to their Git blob")
    return payload


def _raw_release_source_identity(
    source_root: Path, signed_helper_payload: bytes
) -> dict[str, object]:
    """Run signed helper bytes from an unlinked read-only descriptor."""

    if (
        type(signed_helper_payload) is not bytes
        or not signed_helper_payload
        or len(signed_helper_payload) > 4 * 1024 * 1024
    ):
        _fail("signed source identity helper payload is invalid")
    environment = {
        "HOME": os.path.abspath(os.sep),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }
    with tempfile.TemporaryDirectory(
        prefix="iroha-zk-x509-source-helper-",
        dir=_validated_temporary_parent("signed source helper"),
    ) as temporary:
        root = Path(temporary).resolve(strict=True)
        root.chmod(0o700)
        helper = root / "source-identity.py"
        writer = os.open(
            helper,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o400,
        )
        try:
            offset = 0
            while offset < len(signed_helper_payload):
                offset += os.write(writer, signed_helper_payload[offset:])
            os.fchmod(writer, 0o400)
            os.fsync(writer)
        finally:
            os.close(writer)
        descriptor = os.open(
            helper,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            opened = os.fstat(descriptor)
            if (
                not stat.S_ISREG(opened.st_mode)
                or opened.st_uid != os.geteuid()
                or stat.S_IMODE(opened.st_mode) != 0o400
                or opened.st_nlink != 1
                or opened.st_size != len(signed_helper_payload)
            ):
                _fail("signed source identity helper snapshot is not owner-read-only")
            observed_payload = os.pread(descriptor, opened.st_size, 0)
            if observed_payload != signed_helper_payload:
                _fail("signed source identity helper snapshot bytes changed")
            os.unlink(helper)
            unlinked = os.fstat(descriptor)
            if unlinked.st_nlink != 0:
                _fail("signed source identity helper snapshot remained path-addressable")
            invocation = _descriptor_path(
                descriptor, "signed source identity helper snapshot"
            )
            try:
                completed = _run_bounded_process(
                    [
                        sys.executable,
                        "-I",
                        "-S",
                        os.fspath(invocation),
                        "--root",
                        os.fspath(source_root),
                        "--release-identity-json",
                    ],
                    cwd=os.path.abspath(os.sep),
                    environment=environment,
                    timeout=600,
                    pass_fds=(descriptor,),
                    stdout_limit=64 * 1024,
                    stderr_limit=_MAX_TOOL_OUTPUT_BYTES,
                )
            except (OSError, _BoundedProcessError) as error:
                raise ZkX509WorkerPackageError(
                    "raw clean-source identity capture failed"
                ) from error
            after = os.fstat(descriptor)
            if (
                (
                    after.st_dev,
                    after.st_ino,
                    after.st_mode,
                    after.st_uid,
                    after.st_nlink,
                    after.st_size,
                    after.st_mtime_ns,
                )
                != (
                    unlinked.st_dev,
                    unlinked.st_ino,
                    unlinked.st_mode,
                    unlinked.st_uid,
                    unlinked.st_nlink,
                    unlinked.st_size,
                    unlinked.st_mtime_ns,
                )
                or os.pread(descriptor, after.st_size, 0) != signed_helper_payload
            ):
                _fail("signed source identity helper changed while it executed")
        finally:
            os.close(descriptor)
    if completed.returncode != 0 or not 1 <= len(completed.stdout) <= 64 * 1024:
        _fail("raw clean-source identity capture failed")
    try:
        text = completed.stdout.decode("ascii")
        value = json.loads(text, object_pairs_hook=_reject_duplicate_pairs)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ZkX509WorkerPackageError(
            "raw clean-source identity is not canonical JSON"
        ) from error
    identity = _plain_object(value, "raw clean-source identity")
    if completed.stdout != _canonical_json_bytes(identity):
        _fail("raw clean-source identity is not canonical JSON")
    return identity


def collect_source_evidence(
    source_root: Path,
    *,
    allowed_signers: Path,
    expected_allowed_signers_sha256: str,
    revocation: Path,
    expected_revocation_sha256: str,
    expected_signer_principal: str,
    expected_signer_fingerprint: str,
) -> SourceEvidenceV1:
    """Authenticate one clean signed source revision and both SSH policies."""

    source_root = source_root.resolve(strict=True)
    _require_source_checkout_identity_v1(source_root)
    allowed_signers = _canonical_absolute_path(allowed_signers, "allowed_signers")
    revocation = _canonical_absolute_path(revocation, "revocation")
    allowed, allowed_payload = _read_stable_file(
        allowed_signers,
        label="SSH allowed-signers policy",
        maximum=_MAX_POLICY_BYTES,
        capture_payload=True,
    )
    revoked, revocation_payload = _read_stable_file(
        revocation,
        label="SSH revocation policy",
        maximum=_MAX_POLICY_BYTES,
        allow_empty=True,
        capture_payload=True,
    )
    if allowed_payload is None or revocation_payload is None:
        raise AssertionError("SSH policy capture did not return bytes")
    if not hmac.compare_digest(
        allowed.sha256,
        _require_sha256(expected_allowed_signers_sha256, "allowed-signers SHA-256"),
    ):
        _fail("SSH allowed-signers policy does not match its trusted SHA-256")
    if not hmac.compare_digest(
        revoked.sha256,
        _require_sha256(expected_revocation_sha256, "revocation SHA-256"),
    ):
        _fail("SSH revocation policy does not match its trusted SHA-256")

    expected_signer_principal = _require_signer_principal(
        expected_signer_principal, "expected SSH signer principal"
    )
    expected_signer_fingerprint = _require_ssh_fingerprint(
        expected_signer_fingerprint, "expected SSH signer fingerprint"
    )
    commit = _require_commit(
        _git(source_root, ("rev-parse", "--verify", "HEAD^{commit}")).strip(),
        "source commit",
    )
    raw_commit_sha256, signer_principal, signer_fingerprint = (
        _verify_source_signature(source_root, commit, allowed_payload, revocation_payload)
    )
    if (
        not hmac.compare_digest(signer_principal, expected_signer_principal)
        or not hmac.compare_digest(signer_fingerprint, expected_signer_fingerprint)
    ):
        _fail("SSH source signer principal or fingerprint differs from its OOB pin")
    _authenticate_source_helpers_v1(source_root, commit)
    signed_source_helper = _signed_source_helper_bytes(source_root, commit)

    try:
        source_identity = _raw_release_source_identity(source_root, signed_source_helper)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 worker packages require one raw-verified clean source tree"
        ) from error
    reported_commit = _require_commit(source_identity.get("head_commit"), "source commit")
    if not hmac.compare_digest(reported_commit, commit):
        _fail("raw clean-source identity does not match the signed source commit")
    workspace_digest = _require_sha256(
        source_identity.get("workspace_source_manifest_sha256"),
        "workspace source-manifest SHA-256",
    )
    cargo_lock_sha256 = _require_sha256(
        source_identity.get("cargo_lock_sha256"), "Cargo.lock SHA-256"
    )
    source_date_epoch_text = _git(
        source_root, ("show", "-s", "--format=%ct", commit)
    ).strip()
    if not source_date_epoch_text.isascii() or not source_date_epoch_text.isdigit():
        _fail("signed source commit timestamp is not canonical")
    source_date_epoch = int(source_date_epoch_text)
    if source_date_epoch <= 0:
        _fail("signed source commit timestamp must be positive")
    closure_digest = source_closure_sha256(source_root)
    try:
        repeated_source_identity = _raw_release_source_identity(
            source_root, signed_source_helper
        )
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        raise ZkX509WorkerPackageError(
            "authenticated source changed while worker evidence was collected"
        ) from error
    repeated_allowed = _stable_file(
        allowed_signers,
        label="SSH allowed-signers policy",
        maximum=_MAX_POLICY_BYTES,
    )
    repeated_revoked = _stable_file(
        revocation,
        label="SSH revocation policy",
        maximum=_MAX_POLICY_BYTES,
        allow_empty=True,
    )
    repeated_signature = _verify_source_signature(
        source_root, commit, allowed_payload, revocation_payload
    )
    _authenticate_source_helpers_v1(source_root, commit)
    if (
        repeated_source_identity != source_identity
        or repeated_allowed != allowed
        or repeated_revoked != revoked
        or repeated_signature
        != (raw_commit_sha256, signer_principal, signer_fingerprint)
        or _git(source_root, ("show", "-s", "--format=%ct", commit)).strip()
        != source_date_epoch_text
    ):
        _fail("source or SSH policy changed while worker evidence was collected")
    return SourceEvidenceV1(
        allowed_signers_sha256=allowed.sha256,
        cargo_lock_sha256=cargo_lock_sha256,
        commit=commit,
        raw_commit_sha256=_require_sha256(
            raw_commit_sha256, "raw source commit SHA-256"
        ),
        revocation_sha256=revoked.sha256,
        signer_fingerprint=signer_fingerprint,
        signer_principal=signer_principal,
        source_sha256=closure_digest,
        source_date_epoch=source_date_epoch,
        workspace_source_manifest_sha256=workspace_digest,
    )


def _encode_identity_frame(sequence: int, auth_key: bytes) -> bytes:
    authenticated = b"".join(
        (
            _FRAME_MAGIC,
            bytes((_FRAME_PROTOCOL_VERSION, _IDENTITY_COMMAND)),
            sequence.to_bytes(8, "big"),
            (0).to_bytes(4, "big"),
        )
    )
    tag = hmac.new(auth_key, authenticated, hashlib.sha256).digest()
    frame = authenticated + tag
    return len(frame).to_bytes(4, "big") + frame


def _decode_identity_frame(encoded: bytes, sequence: int, auth_key: bytes) -> bytes:
    if len(encoded) < 4:
        _fail("X5PW identity response is truncated")
    declared = int.from_bytes(encoded[:4], "big")
    if declared != len(encoded) - 4 or not 50 <= declared <= _MAX_FRAME_BYTES:
        _fail("X5PW identity response length is invalid")
    authenticated = encoded[4:-_AUTH_TAG_BYTES]
    tag = encoded[-_AUTH_TAG_BYTES:]
    if not hmac.compare_digest(
        tag, hmac.new(auth_key, authenticated, hashlib.sha256).digest()
    ):
        _fail("X5PW identity response authentication failed")
    if (
        authenticated[:4] != _FRAME_MAGIC
        or authenticated[4] != _FRAME_PROTOCOL_VERSION
        or authenticated[5] != _IDENTITY_COMMAND
        or int.from_bytes(authenticated[6:14], "big") != sequence
        or int.from_bytes(authenticated[14:18], "big") != len(authenticated[18:])
    ):
        _fail("X5PW identity response does not match the request")
    return authenticated[18:]


def _script_checkout_root_v1() -> Path:
    try:
        return Path(__file__).resolve(strict=True).parents[1]
    except (OSError, IndexError) as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 packaging script checkout is unavailable"
        ) from error


def _load_exact_worker_launch_module(source_root: Path | None = None):
    """Load exact-launch code from authenticated, stably captured source bytes."""

    checkout = (
        _script_checkout_root_v1()
        if source_root is None
        else source_root
    )
    if not checkout.is_absolute() or not checkout.is_dir():
        _fail("zk-X509 exact-launch source root is not descriptor-anchored")
    launch_source = checkout / _EXACT_LAUNCH_SOURCE_RELATIVE
    payload = _stable_bytes(
        launch_source,
        label="zk-X509 exact launch helper",
        maximum=4 * 1024 * 1024,
    )
    source_sha256 = hashlib.sha256(payload).hexdigest()
    module_name = f"_iroha_privacy_zk_x509_package_launch_{source_sha256}"
    loaded = sys.modules.get(module_name)
    if loaded is not None:
        return loaded
    module = types.ModuleType(module_name)
    module.__file__ = os.fspath(launch_source)
    module.__package__ = ""
    sys.modules[module_name] = module
    try:
        code = compile(
            payload,
            os.fspath(launch_source),
            "exec",
            dont_inherit=True,
        )
        exec(code, module.__dict__)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


def _parse_identity(payload: bytes) -> WorkerIdentityV2:
    if not payload or payload[0] != _RESPONSE_OK or len(payload) > _MAX_IDENTITY_BYTES:
        _fail("X5PW worker did not return a successful bounded identity")
    try:
        raw = payload[1:].decode("utf-8")
        parsed = json.loads(raw, object_pairs_hook=_reject_duplicate_pairs)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ZkX509WorkerPackageError("X5PW worker identity is not JSON") from error
    identity = _plain_object(parsed, "X5PW worker identity")
    expected_keys = {
        "artifact_self_hash_required",
        "cargo_lock_sha256",
        "compiled_profile_sha256",
        "expectations_json_sha256",
        "expectations_norito_sha256",
        "isolation_contract",
        "isolation_package_sha256",
        "kat_proof_bytes",
        "kat_proof_sha256",
        "operation",
        "production_profile_ready",
        "protocol_id",
        "protocol_profile_sha256",
        "protocol_version",
        "public_request_schema_version",
        "qualified_isolation_ready",
        "release_evidence_ready",
        "release_evidence_sha256",
        "resource_certificate_sha256",
        "schema",
        "schema_version",
        "soundness_certificate_sha256",
        "source_allowed_signers_sha256",
        "source_closure_schema",
        "source_commit",
        "source_revocation_sha256",
        "source_sha256",
        "workspace_source_manifest_sha256",
    }
    if set(identity) != expected_keys:
        _fail("X5PW worker identity field inventory is not exact")
    canonical_identity = json.dumps(
        identity,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
    ).encode("utf-8")
    if canonical_identity != payload[1:]:
        _fail("X5PW worker identity is not canonical JSON")
    if (
        identity["artifact_self_hash_required"] is not True
        or identity["operation"] != "prove-and-sign-zk-x509-action-v1"
        or identity["protocol_id"] != PROTOCOL_ID
        or identity["protocol_version"] != PROTOCOL_VERSION
        or identity["public_request_schema_version"] != PUBLIC_REQUEST_SCHEMA_VERSION
        or identity["schema"] != "iroha.privacy.zk_x509_worker_identity"
        or identity["schema_version"] != 2
        or identity["source_closure_schema"] != SOURCE_CLOSURE_SCHEMA
        or type(identity["production_profile_ready"]) is not bool
        or type(identity["qualified_isolation_ready"]) is not bool
        or type(identity["release_evidence_ready"]) is not bool
    ):
        _fail("X5PW worker identity does not match the closed protocol")
    protocol_profile = _require_sha256(
        identity["protocol_profile_sha256"], "protocol profile SHA-256"
    )
    compiled = _optional_sha256(
        identity["compiled_profile_sha256"], "compiled profile SHA-256"
    )
    kat_proof_bytes = identity["kat_proof_bytes"]
    if (
        type(kat_proof_bytes) is not int
        or not 0 <= kat_proof_bytes <= _MAX_KAT_PROOF_BYTES
    ):
        _fail("X5PW worker KAT proof length is invalid")
    kat_proof = _optional_sha256(
        identity["kat_proof_sha256"], "KAT proof SHA-256"
    )
    expectations_norito = _optional_sha256(
        identity["expectations_norito_sha256"],
        "expectations Norito SHA-256",
    )
    expectations_json = _optional_sha256(
        identity["expectations_json_sha256"], "expectations JSON SHA-256"
    )
    soundness = _optional_sha256(
        identity["soundness_certificate_sha256"],
        "soundness certificate SHA-256",
    )
    resource = _optional_sha256(
        identity["resource_certificate_sha256"],
        "resource certificate SHA-256",
    )
    release_evidence = _optional_sha256(
        identity["release_evidence_sha256"], "release evidence SHA-256"
    )
    evidence_components_complete = (
        kat_proof_bytes > 0
        and kat_proof is not None
        and expectations_norito is not None
        and expectations_json is not None
        and not hmac.compare_digest(expectations_norito, expectations_json)
        and soundness is not None
        and resource is not None
    )
    expected_release_evidence = None
    if evidence_components_complete:
        assert kat_proof is not None
        assert expectations_norito is not None
        assert expectations_json is not None
        assert soundness is not None
        assert resource is not None
        expected_release_evidence = _release_evidence_sha256(
            protocol_profile_sha256=protocol_profile,
            kat_proof_bytes=kat_proof_bytes,
            kat_proof_sha256=kat_proof,
            expectations_norito_sha256=expectations_norito,
            expectations_json_sha256=expectations_json,
            soundness_certificate_sha256=soundness,
            resource_certificate_sha256=resource,
        )
    evidence_ready = (
        expected_release_evidence is not None
        and release_evidence is not None
        and hmac.compare_digest(release_evidence, expected_release_evidence)
    )
    if expected_release_evidence is None and release_evidence is not None:
        _fail("X5PW worker release-evidence digest has incomplete constituents")
    if expected_release_evidence is not None and not evidence_ready:
        _fail("X5PW worker release-evidence digest does not match its constituents")
    if identity["release_evidence_ready"] is not evidence_ready:
        _fail("X5PW worker release-evidence identity is inconsistent")
    if compiled is not None and (
        not evidence_ready or not hmac.compare_digest(compiled, protocol_profile)
    ):
        _fail("X5PW worker compiled profile is not the evidence-complete protocol profile")
    source_commit = _require_commit(identity["source_commit"], "source commit")
    cargo_lock_sha256 = _require_sha256(
        identity["cargo_lock_sha256"], "Cargo.lock SHA-256"
    )
    source_allowed_signers_sha256 = _require_sha256(
        identity["source_allowed_signers_sha256"], "allowed-signers SHA-256"
    )
    source_revocation_sha256 = _require_sha256(
        identity["source_revocation_sha256"], "revocation SHA-256"
    )
    source_sha256 = _require_sha256(identity["source_sha256"], "source closure SHA-256")
    workspace_digest = _require_sha256(
        identity["workspace_source_manifest_sha256"],
        "workspace source-manifest SHA-256",
    )
    qualified = identity["qualified_isolation_ready"]
    isolation = identity["isolation_contract"]
    isolation_package = _optional_sha256(
        identity["isolation_package_sha256"], "isolation package SHA-256"
    )
    if (
        type(isolation) is not str
        or (qualified and isolation != QUALIFIED_ISOLATION_CONTRACT)
        or (not qualified and isolation != UNAVAILABLE_ISOLATION_CONTRACT)
        or (qualified and isolation_package is None)
        or (not qualified and isolation_package is not None)
    ):
        _fail("X5PW worker isolation identity is not canonical")
    expected_production_ready = compiled is not None and evidence_ready and qualified
    if identity["production_profile_ready"] is not expected_production_ready:
        _fail("X5PW worker overstates or understates production readiness")
    return WorkerIdentityV2(
        cargo_lock_sha256=cargo_lock_sha256,
        compiled_profile_sha256=compiled,
        expectations_json_sha256=expectations_json,
        expectations_norito_sha256=expectations_norito,
        isolation_contract=isolation,
        isolation_package_sha256=isolation_package,
        kat_proof_bytes=kat_proof_bytes,
        kat_proof_sha256=kat_proof,
        production_profile_ready=identity["production_profile_ready"],
        protocol_id=PROTOCOL_ID,
        protocol_profile_sha256=protocol_profile,
        protocol_version=PROTOCOL_VERSION,
        public_request_schema_version=PUBLIC_REQUEST_SCHEMA_VERSION,
        qualified_isolation_ready=qualified,
        release_evidence_ready=evidence_ready,
        release_evidence_sha256=release_evidence,
        resource_certificate_sha256=resource,
        soundness_certificate_sha256=soundness,
        source_allowed_signers_sha256=source_allowed_signers_sha256,
        source_closure_schema=SOURCE_CLOSURE_SCHEMA,
        source_commit=source_commit,
        source_revocation_sha256=source_revocation_sha256,
        source_sha256=source_sha256,
        workspace_source_manifest_sha256=workspace_digest,
    )


def _probe_worker_identity_snapshot(
    snapshot: ArtifactSnapshotV1,
    expected_artifact_sha256: str,
    *,
    source_root: Path | None = None,
) -> WorkerIdentityV2:
    """Execute and authenticate the exact artifact snapshot already admitted."""

    expected_artifact_sha256 = _require_sha256(
        expected_artifact_sha256, "expected worker artifact SHA-256"
    )
    if not hmac.compare_digest(snapshot.record.sha256, expected_artifact_sha256):
        _fail("worker artifact snapshot differs from its caller-supplied digest")
    _revalidate_artifact_snapshot(snapshot, expected_artifact_sha256)
    auth_key = bytearray(secrets.token_bytes(_AUTH_KEY_BYTES))
    if len(auth_key) != _AUTH_KEY_BYTES or not any(auth_key):
        _fail("secure X5PW authentication entropy is unavailable")
    sequence = int.from_bytes(secrets.token_bytes(8), "big") or 1
    request = bytearray(auth_key)
    request.extend(_encode_identity_frame(sequence, bytes(auth_key)))
    launch = None
    try:
        launch_module = _load_exact_worker_launch_module(source_root)
        try:
            launch = launch_module._prepare_verified_worker_launch_v1(
                snapshot.path,
                expected_artifact_sha256,
            )
        except (OSError, ValueError) as error:
            raise ZkX509WorkerPackageError(
                "zk-X509 worker exact authenticated launch failed"
            ) from error
        try:
            completed = _run_bounded_process(
                [os.fspath(launch.invocation)],
                cwd=os.path.abspath(os.sep),
                environment={},
                timeout=30,
                stdout_limit=_MAX_FRAME_BYTES + 4,
                stderr_limit=_MAX_TOOL_OUTPUT_BYTES,
                input_data=request,
                pass_fds=launch.pass_fds,
            )
            launch.authenticate()
        except (OSError, ValueError, _BoundedProcessError) as error:
            raise ZkX509WorkerPackageError(
                "native zk-X509 worker identity probe failed"
            ) from error
        _revalidate_artifact_snapshot(snapshot, expected_artifact_sha256)
        if completed.returncode != 0:
            _fail("native zk-X509 worker changed or failed during identity probe")
        response = _decode_identity_frame(completed.stdout, sequence, bytes(auth_key))
        return _parse_identity(response)
    finally:
        if launch is not None:
            launch.close()
        for index in range(len(auth_key)):
            auth_key[index] = 0
        for index in range(len(request)):
            request[index] = 0


def probe_worker_identity(
    artifact: Path,
    *,
    expected_artifact_sha256: str,
    source_root: Path | None = None,
) -> WorkerIdentityV2:
    """Authenticate one native worker through one private immutable snapshot."""

    expected = _require_sha256(
        expected_artifact_sha256, "expected worker artifact SHA-256"
    )
    with _immutable_artifact_snapshot(artifact, "worker artifact") as snapshot:
        if not hmac.compare_digest(snapshot.record.sha256, expected):
            _fail("worker artifact differs from its caller-supplied digest")
        return _probe_worker_identity_snapshot(
            snapshot,
            expected,
            source_root=source_root,
        )


def _validated_build_command_sha256(
    method: object, digest: object, target: str
) -> tuple[str, str | None]:
    if method == PREBUILT_CANDIDATE_BUILD_V1:
        if digest is not None:
            _fail("prebuilt zk-X509 worker candidates cannot claim a build-command pin")
        return PREBUILT_CANDIDATE_BUILD_V1, None
    if method != AUTHENTICATED_SOURCE_BUILD_V2:
        _fail("zk-X509 worker artifact build method is not recognized")
    observed = _require_sha256(digest, "artifact build-command SHA-256")
    expected = _build_command_sha256(target)
    if not hmac.compare_digest(observed, expected):
        _fail("zk-X509 worker artifact build command does not match the closed corridor")
    return AUTHENTICATED_SOURCE_BUILD_V2, observed


def _validated_build_provenance(
    method: str,
    value: object,
    *,
    source: SourceEvidenceV1,
    target: str,
) -> dict[str, object] | None:
    if method == PREBUILT_CANDIDATE_BUILD_V1:
        if value is not None:
            _fail("prebuilt zk-X509 worker candidates cannot claim build provenance")
        return None
    return _validate_build_provenance_v2(value, source=source, target=target)


def _release_ready(
    identity: WorkerIdentityV2,
    target: str,
    artifact_build_method: str,
    artifact_build_command_sha256: str | None,
    artifact_build_provenance: dict[str, object] | None,
) -> bool:
    return (
        target == RELEASE_TARGET
        and artifact_build_method == AUTHENTICATED_SOURCE_BUILD_V2
        and artifact_build_command_sha256 == _build_command_sha256(target)
        and artifact_build_provenance is not None
        and artifact_build_provenance.get("target") == target
        and identity.compiled_profile_sha256 is not None
        and identity.compiled_profile_sha256 == identity.protocol_profile_sha256
        and identity.production_profile_ready
        and identity.qualified_isolation_ready
        and identity.isolation_contract == QUALIFIED_ISOLATION_CONTRACT
        and identity.isolation_package_sha256 is not None
        and identity.release_evidence_ready
        and identity.release_evidence_sha256 is not None
    )


def build_manifest(
    *,
    artifact: StableFileV1,
    identity: WorkerIdentityV2,
    source: SourceEvidenceV1,
    target: str,
    artifact_build_method: str = PREBUILT_CANDIDATE_BUILD_V1,
    artifact_build_command_sha256: str | None = None,
    artifact_build_provenance: dict[str, object] | None = None,
) -> dict[str, object]:
    """Build one exact candidate or release package manifest."""

    if _TARGET_RE.fullmatch(target) is None:
        _fail("zk-X509 worker target is not canonical")
    artifact_build_method, artifact_build_command_sha256 = (
        _validated_build_command_sha256(
            artifact_build_method, artifact_build_command_sha256, target
        )
    )
    artifact_build_provenance = _validated_build_provenance(
        artifact_build_method,
        artifact_build_provenance,
        source=source,
        target=target,
    )
    artifact_build_environment_sha256 = (
        artifact_build_provenance["environment_sha256"]
        if artifact_build_provenance is not None
        else None
    )
    artifact_build_toolchain_sha256 = (
        artifact_build_provenance["toolchain_sha256"]
        if artifact_build_provenance is not None
        else None
    )
    if (
        identity.cargo_lock_sha256 != source.cargo_lock_sha256
        or identity.source_allowed_signers_sha256 != source.allowed_signers_sha256
        or identity.source_commit != source.commit
        or identity.source_revocation_sha256 != source.revocation_sha256
        or identity.source_sha256 != source.source_sha256
        or identity.workspace_source_manifest_sha256
        != source.workspace_source_manifest_sha256
    ):
        _fail("worker identity does not match the authenticated source")
    if identity.qualified_isolation_ready and not hmac.compare_digest(
        str(identity.isolation_package_sha256),
        _qualified_isolation_package_sha256(artifact.sha256),
    ):
        _fail("worker isolation package does not bind the packaged launcher image")
    return validate_manifest(
        {
            "artifact_build_command_sha256": artifact_build_command_sha256,
            "artifact_build_environment_sha256": artifact_build_environment_sha256,
            "artifact_build_method": artifact_build_method,
            "artifact_build_provenance": artifact_build_provenance,
            "artifact_build_toolchain_sha256": artifact_build_toolchain_sha256,
            "artifact_file": ARTIFACT_FILE,
            "artifact_sha256": artifact.sha256,
            "artifact_size": artifact.size,
            "cargo_lock_sha256": identity.cargo_lock_sha256,
            "compiled_profile_sha256": identity.compiled_profile_sha256,
            "expectations_json_sha256": identity.expectations_json_sha256,
            "expectations_norito_sha256": identity.expectations_norito_sha256,
            "isolation_contract": identity.isolation_contract,
            "isolation_package_sha256": identity.isolation_package_sha256,
            "kat_proof_bytes": identity.kat_proof_bytes,
            "kat_proof_sha256": identity.kat_proof_sha256,
            "production_profile_ready": identity.production_profile_ready,
            "protocol_id": identity.protocol_id,
            "protocol_profile_sha256": identity.protocol_profile_sha256,
            "protocol_version": identity.protocol_version,
            "public_request_schema_version": identity.public_request_schema_version,
            "qualified_isolation_ready": identity.qualified_isolation_ready,
            "release_evidence_ready": identity.release_evidence_ready,
            "release_evidence_sha256": identity.release_evidence_sha256,
            "release_ready": _release_ready(
                identity,
                target,
                artifact_build_method,
                artifact_build_command_sha256,
                artifact_build_provenance,
            ),
            "resource_certificate_sha256": identity.resource_certificate_sha256,
            "schema": SCHEMA,
            "schema_version": SCHEMA_VERSION,
            "source_allowed_signers_sha256": identity.source_allowed_signers_sha256,
            "source_closure_schema": identity.source_closure_schema,
            "source_commit": source.commit,
            "source_commit_raw_sha256": source.raw_commit_sha256,
            "source_commit_signature_verified": True,
            "source_revocation_sha256": identity.source_revocation_sha256,
            "source_signer_fingerprint": source.signer_fingerprint,
            "source_signer_principal": source.signer_principal,
            "source_sha256": source.source_sha256,
            "source_date_epoch": source.source_date_epoch,
            "source_tree_clean": True,
            "soundness_certificate_sha256": identity.soundness_certificate_sha256,
            "target": target,
            "workspace_source_manifest_sha256": source.workspace_source_manifest_sha256,
        }
    )


def validate_manifest(value: object) -> dict[str, object]:
    """Validate the exact immutable package-manifest schema."""

    manifest = _plain_object(value, "zk-X509 worker package manifest")
    expected_keys = {
        "artifact_build_command_sha256",
        "artifact_build_environment_sha256",
        "artifact_build_method",
        "artifact_build_provenance",
        "artifact_build_toolchain_sha256",
        "artifact_file",
        "artifact_sha256",
        "artifact_size",
        "cargo_lock_sha256",
        "compiled_profile_sha256",
        "expectations_json_sha256",
        "expectations_norito_sha256",
        "isolation_contract",
        "isolation_package_sha256",
        "kat_proof_bytes",
        "kat_proof_sha256",
        "production_profile_ready",
        "protocol_id",
        "protocol_profile_sha256",
        "protocol_version",
        "public_request_schema_version",
        "qualified_isolation_ready",
        "release_evidence_ready",
        "release_evidence_sha256",
        "release_ready",
        "resource_certificate_sha256",
        "schema",
        "schema_version",
        "source_allowed_signers_sha256",
        "source_closure_schema",
        "source_commit",
        "source_commit_raw_sha256",
        "source_commit_signature_verified",
        "source_revocation_sha256",
        "source_signer_fingerprint",
        "source_signer_principal",
        "source_sha256",
        "source_date_epoch",
        "source_tree_clean",
        "soundness_certificate_sha256",
        "target",
        "workspace_source_manifest_sha256",
    }
    if set(manifest) != expected_keys:
        _fail("zk-X509 worker package manifest field inventory is not exact")
    if (
        manifest["schema"] != SCHEMA
        or manifest["schema_version"] != SCHEMA_VERSION
        or manifest["artifact_file"] != ARTIFACT_FILE
        or manifest["protocol_id"] != PROTOCOL_ID
        or manifest["protocol_version"] != PROTOCOL_VERSION
        or manifest["public_request_schema_version"] != PUBLIC_REQUEST_SCHEMA_VERSION
        or manifest["source_closure_schema"] != SOURCE_CLOSURE_SCHEMA
        or manifest["source_commit_signature_verified"] is not True
        or manifest["source_tree_clean"] is not True
    ):
        _fail("zk-X509 worker package manifest protocol or source state is invalid")
    artifact_sha256 = _require_sha256(manifest["artifact_sha256"], "artifact SHA-256")
    artifact_size = manifest["artifact_size"]
    if type(artifact_size) is not int or not 1 <= artifact_size <= _MAX_ARTIFACT_BYTES:
        _fail("zk-X509 worker artifact size is invalid")
    target = manifest["target"]
    if type(target) is not str or _TARGET_RE.fullmatch(target) is None:
        _fail("zk-X509 worker target is invalid")
    artifact_build_method, artifact_build_command_sha256 = (
        _validated_build_command_sha256(
            manifest["artifact_build_method"],
            manifest["artifact_build_command_sha256"],
            target,
        )
    )
    commit = _require_commit(manifest["source_commit"], "source commit")
    raw_commit_sha256 = _require_sha256(
        manifest["source_commit_raw_sha256"], "raw source commit SHA-256"
    )
    signer_fingerprint = _require_ssh_fingerprint(
        manifest["source_signer_fingerprint"], "source signer fingerprint"
    )
    signer_principal = _require_signer_principal(
        manifest["source_signer_principal"], "source signer principal"
    )
    source_digests: dict[str, str] = {}
    for field, label in (
        ("cargo_lock_sha256", "Cargo.lock SHA-256"),
        ("source_allowed_signers_sha256", "allowed-signers SHA-256"),
        ("source_revocation_sha256", "revocation SHA-256"),
        ("source_sha256", "source closure SHA-256"),
        ("workspace_source_manifest_sha256", "workspace source-manifest SHA-256"),
    ):
        source_digests[field] = _require_sha256(manifest[field], label)
    source_date_epoch = manifest["source_date_epoch"]
    if type(source_date_epoch) is not int or source_date_epoch <= 0:
        _fail("zk-X509 worker source date epoch is invalid")
    source = SourceEvidenceV1(
        allowed_signers_sha256=source_digests["source_allowed_signers_sha256"],
        cargo_lock_sha256=source_digests["cargo_lock_sha256"],
        commit=commit,
        raw_commit_sha256=raw_commit_sha256,
        revocation_sha256=source_digests["source_revocation_sha256"],
        signer_fingerprint=signer_fingerprint,
        signer_principal=signer_principal,
        source_sha256=source_digests["source_sha256"],
        source_date_epoch=source_date_epoch,
        workspace_source_manifest_sha256=source_digests[
            "workspace_source_manifest_sha256"
        ],
    )
    artifact_build_provenance = _validated_build_provenance(
        artifact_build_method,
        manifest["artifact_build_provenance"],
        source=source,
        target=target,
    )
    environment_digest = manifest["artifact_build_environment_sha256"]
    toolchain_digest = manifest["artifact_build_toolchain_sha256"]
    if artifact_build_method == PREBUILT_CANDIDATE_BUILD_V1:
        if environment_digest is not None or toolchain_digest is not None:
            _fail("prebuilt zk-X509 worker candidates cannot claim build provenance")
    else:
        assert artifact_build_provenance is not None
        if not hmac.compare_digest(
            _require_sha256(environment_digest, "artifact build-environment SHA-256"),
            artifact_build_provenance["environment_sha256"],
        ) or not hmac.compare_digest(
            _require_sha256(toolchain_digest, "artifact build-toolchain SHA-256"),
            artifact_build_provenance["toolchain_sha256"],
        ):
            _fail("zk-X509 worker authenticated build provenance is invalid")
    protocol_profile = _require_sha256(
        manifest["protocol_profile_sha256"], "protocol profile SHA-256"
    )
    compiled = _optional_sha256(
        manifest["compiled_profile_sha256"], "compiled profile SHA-256"
    )
    kat_proof_bytes = manifest["kat_proof_bytes"]
    if (
        type(kat_proof_bytes) is not int
        or not 0 <= kat_proof_bytes <= _MAX_KAT_PROOF_BYTES
    ):
        _fail("zk-X509 worker package KAT proof length is invalid")
    kat_proof = _optional_sha256(
        manifest["kat_proof_sha256"], "KAT proof SHA-256"
    )
    expectations_norito = _optional_sha256(
        manifest["expectations_norito_sha256"],
        "expectations Norito SHA-256",
    )
    expectations_json = _optional_sha256(
        manifest["expectations_json_sha256"], "expectations JSON SHA-256"
    )
    soundness = _optional_sha256(
        manifest["soundness_certificate_sha256"],
        "soundness certificate SHA-256",
    )
    resource = _optional_sha256(
        manifest["resource_certificate_sha256"],
        "resource certificate SHA-256",
    )
    evidence_digest = _optional_sha256(
        manifest["release_evidence_sha256"], "release evidence SHA-256"
    )
    evidence_components_complete = (
        kat_proof_bytes > 0
        and kat_proof is not None
        and expectations_norito is not None
        and expectations_json is not None
        and not hmac.compare_digest(expectations_norito, expectations_json)
        and soundness is not None
        and resource is not None
    )
    expected_evidence_digest = None
    if evidence_components_complete:
        assert kat_proof is not None
        assert expectations_norito is not None
        assert expectations_json is not None
        assert soundness is not None
        assert resource is not None
        expected_evidence_digest = _release_evidence_sha256(
            protocol_profile_sha256=protocol_profile,
            kat_proof_bytes=kat_proof_bytes,
            kat_proof_sha256=kat_proof,
            expectations_norito_sha256=expectations_norito,
            expectations_json_sha256=expectations_json,
            soundness_certificate_sha256=soundness,
            resource_certificate_sha256=resource,
        )
    evidence_ready = manifest["release_evidence_ready"]
    expected_evidence_ready = (
        expected_evidence_digest is not None
        and evidence_digest is not None
        and hmac.compare_digest(evidence_digest, expected_evidence_digest)
    )
    if expected_evidence_digest is None and evidence_digest is not None:
        _fail("zk-X509 worker package release-evidence digest has incomplete constituents")
    if expected_evidence_digest is not None and not expected_evidence_ready:
        _fail("zk-X509 worker package release-evidence digest does not match its constituents")
    if type(evidence_ready) is not bool or evidence_ready is not expected_evidence_ready:
        _fail("zk-X509 worker package release-evidence identity is inconsistent")
    if compiled is not None and (
        not evidence_ready or not hmac.compare_digest(compiled, protocol_profile)
    ):
        _fail("zk-X509 worker package compiled profile is not evidence-complete")
    production = manifest["production_profile_ready"]
    qualified = manifest["qualified_isolation_ready"]
    release = manifest["release_ready"]
    isolation = manifest["isolation_contract"]
    isolation_package = _optional_sha256(
        manifest["isolation_package_sha256"], "isolation package SHA-256"
    )
    if (
        type(production) is not bool
        or type(qualified) is not bool
        or type(release) is not bool
    ):
        _fail("zk-X509 worker readiness fields must be booleans")
    if (
        type(isolation) is not str
        or (qualified and isolation != QUALIFIED_ISOLATION_CONTRACT)
        or (not qualified and isolation != UNAVAILABLE_ISOLATION_CONTRACT)
        or (qualified and isolation_package is None)
        or (not qualified and isolation_package is not None)
        or (
            qualified
            and not hmac.compare_digest(
                str(isolation_package),
                _qualified_isolation_package_sha256(artifact_sha256),
            )
        )
        or production is not (compiled is not None and evidence_ready and qualified)
    ):
        _fail("zk-X509 worker readiness evidence is inconsistent")
    identity = WorkerIdentityV2(
        cargo_lock_sha256=str(manifest["cargo_lock_sha256"]),
        compiled_profile_sha256=compiled,
        expectations_json_sha256=expectations_json,
        expectations_norito_sha256=expectations_norito,
        isolation_contract=isolation,
        isolation_package_sha256=isolation_package,
        kat_proof_bytes=kat_proof_bytes,
        kat_proof_sha256=kat_proof,
        production_profile_ready=production,
        protocol_id=PROTOCOL_ID,
        protocol_profile_sha256=protocol_profile,
        protocol_version=PROTOCOL_VERSION,
        public_request_schema_version=PUBLIC_REQUEST_SCHEMA_VERSION,
        qualified_isolation_ready=qualified,
        release_evidence_ready=evidence_ready,
        release_evidence_sha256=evidence_digest,
        resource_certificate_sha256=resource,
        soundness_certificate_sha256=soundness,
        source_allowed_signers_sha256=str(
            manifest["source_allowed_signers_sha256"]
        ),
        source_closure_schema=SOURCE_CLOSURE_SCHEMA,
        source_commit=commit,
        source_revocation_sha256=str(manifest["source_revocation_sha256"]),
        source_sha256=str(manifest["source_sha256"]),
        workspace_source_manifest_sha256=str(
            manifest["workspace_source_manifest_sha256"]
        ),
    )
    if release is not _release_ready(
        identity,
        target,
        artifact_build_method,
        artifact_build_command_sha256,
        artifact_build_provenance,
    ):
        _fail("zk-X509 worker release-ready claim is inconsistent")
    # Keep local names live so type and canonical checks above cannot be
    # accidentally optimized away during future mechanical refactors.
    if not artifact_sha256 or artifact_size == 0:
        _fail("zk-X509 worker artifact identity is empty")
    return dict(manifest)


def canonical_manifest_bytes(value: object) -> bytes:
    return _canonical_json_bytes(validate_manifest(value))


def authenticated_package_root_sha256(value: object) -> str:
    """Commit to the complete canonical manifest for out-of-band approval."""

    payload = canonical_manifest_bytes(value)
    digest = hashlib.sha256()
    digest.update(_AUTHENTICATED_PACKAGE_ROOT_DOMAIN)
    digest.update(len(payload).to_bytes(8, "big"))
    digest.update(payload)
    return digest.hexdigest()


def _require_trusted_package_root_v1(
    manifest: dict[str, object], trusted_package_root_sha256: str | None
) -> None:
    if trusted_package_root_sha256 is None:
        _fail(
            "release-ready verification requires an externally trusted "
            "package-root SHA-256"
        )
    trusted = _require_sha256(
        trusted_package_root_sha256, "trusted package-root SHA-256"
    )
    observed = authenticated_package_root_sha256(manifest)
    if not hmac.compare_digest(observed, trusted):
        _fail("zk-X509 worker package does not match the trusted package root")


def _load_manifest_bytes(payload: bytes) -> dict[str, object]:
    try:
        value = json.loads(payload, object_pairs_hook=_reject_duplicate_pairs)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 worker package manifest is not JSON"
        ) from error
    manifest = validate_manifest(value)
    if payload != canonical_manifest_bytes(manifest):
        _fail("zk-X509 worker package manifest is not canonical JSON")
    return manifest


def load_manifest(path: Path) -> dict[str, object]:
    payload = _stable_bytes(
        path,
        label="zk-X509 worker package manifest",
        maximum=_MAX_MANIFEST_BYTES,
    )
    return _load_manifest_bytes(payload)


def _package_file_record_at(
    directory_descriptor: int,
    name: str,
    *,
    label: str,
    maximum: int,
    expected_mode: int,
    capture_payload: bool,
) -> tuple[dict[str, object], bytes]:
    """Read and bind one package member through its held parent directory."""

    try:
        before = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
        descriptor = os.open(
            name,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=directory_descriptor,
        )
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} is unavailable") from error
    try:
        opened = os.fstat(descriptor)
        if stat.S_IMODE(before.st_mode) != expected_mode:
            _fail(f"{label} must have mode {expected_mode:04o}")
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid != os.geteuid()
            or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
            or not 1 <= before.st_size <= maximum
            or _file_open_identity(opened) != _file_open_identity(before)
        ):
            _fail(f"{label} is not one exact owner-controlled package file")
        payload = bytearray() if capture_payload else None
        digest = hashlib.sha256()
        remaining = opened.st_size
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                _fail(f"{label} was truncated while read")
            digest.update(chunk)
            if payload is not None:
                payload.extend(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail(f"{label} grew while read")
        after = os.fstat(descriptor)
        if _file_open_identity(after) != _file_open_identity(opened):
            _fail(f"{label} changed while read")
        return (
            {
                "device": opened.st_dev,
                "inode": opened.st_ino,
                "links": opened.st_nlink,
                "mode": stat.S_IMODE(opened.st_mode),
                "owner": opened.st_uid,
                "sha256": digest.hexdigest(),
                "size": opened.st_size,
            },
            b"" if payload is None else bytes(payload),
        )
    finally:
        os.close(descriptor)


def _package_inventory(
    directory_descriptor: int,
) -> tuple[tuple[int, ...], dict[str, dict[str, object]], dict[str, bytes]]:
    """Return the complete exact inventory of one held package directory."""

    root_before = os.fstat(directory_descriptor)
    root_identity = (
        root_before.st_dev,
        root_before.st_ino,
        root_before.st_mode,
        root_before.st_uid,
        root_before.st_nlink,
    )
    if (
        not stat.S_ISDIR(root_before.st_mode)
        or root_before.st_uid != os.geteuid()
        or stat.S_IMODE(root_before.st_mode) != 0o500
    ):
        _fail("zk-X509 worker package root is not owner-controlled mode 0500")
    try:
        names = set(os.listdir(directory_descriptor))
    except OSError as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 worker package inventory is unreadable"
        ) from error
    if names != {ARTIFACT_FILE, "manifest.json"}:
        _fail("zk-X509 worker package file inventory is not exact")
    records: dict[str, dict[str, object]] = {}
    payloads: dict[str, bytes] = {}
    for name, maximum, mode, label in (
        (ARTIFACT_FILE, _MAX_ARTIFACT_BYTES, 0o500, "packaged zk-X509 worker artifact"),
        ("manifest.json", _MAX_MANIFEST_BYTES, 0o400, "zk-X509 worker package manifest"),
    ):
        records[name], payloads[name] = _package_file_record_at(
            directory_descriptor,
            name,
            label=label,
            maximum=maximum,
            expected_mode=mode,
            capture_payload=name == "manifest.json",
        )
    root_after = os.fstat(directory_descriptor)
    if (
        root_after.st_dev,
        root_after.st_ino,
        root_after.st_mode,
        root_after.st_uid,
        root_after.st_nlink,
    ) != root_identity:
        _fail("zk-X509 worker package root changed during inventory")
    return root_identity, records, payloads


def _copy_artifact(
    source: ArtifactSnapshotV1,
    destination: Path,
    *,
    destination_dir_fd: int | None = None,
) -> None:
    destination_flags = (
        os.O_CREAT
        | os.O_EXCL
        | os.O_WRONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    destination_fd = (
        os.open(destination, destination_flags, 0o500)
        if destination_dir_fd is None
        else os.open(
            destination.name,
            destination_flags,
            0o500,
            dir_fd=destination_dir_fd,
        )
    )
    digest = hashlib.sha256()
    size = 0
    try:
        _revalidate_artifact_snapshot(source, source.record.sha256)
        while size < source.record.size:
            chunk = os.pread(
                source.descriptor,
                min(1024 * 1024, source.record.size - size),
                size,
            )
            if not chunk:
                _fail("zk-X509 worker artifact snapshot was truncated while copied")
            digest.update(chunk)
            size += len(chunk)
            offset = 0
            while offset < len(chunk):
                offset += os.write(destination_fd, chunk[offset:])
        os.fchmod(destination_fd, 0o500)
        os.fsync(destination_fd)
    finally:
        os.close(destination_fd)
    _revalidate_artifact_snapshot(source, source.record.sha256)
    if (
        size != source.record.size
        or not hmac.compare_digest(digest.hexdigest(), source.record.sha256)
    ):
        _fail("zk-X509 worker artifact changed while it was packaged")


def _fsync_directory(path: Path, label: str) -> None:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} could not be opened") from error
    try:
        if not stat.S_ISDIR(os.fstat(descriptor).st_mode):
            _fail(f"{label} is not a directory")
        os.fsync(descriptor)
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} could not be synchronized") from error
    finally:
        os.close(descriptor)


def _clear_held_directory(directory_descriptor: int) -> None:
    """Remove only descendants reached through one already-held directory."""

    os.fchmod(directory_descriptor, 0o700)
    for name in os.listdir(directory_descriptor):
        details = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
        if stat.S_ISDIR(details.st_mode):
            child = os.open(
                name,
                os.O_RDONLY
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_NOFOLLOW", 0),
                dir_fd=directory_descriptor,
            )
            try:
                opened = os.fstat(child)
                if (opened.st_dev, opened.st_ino) != (
                    details.st_dev,
                    details.st_ino,
                ):
                    _fail("package cleanup directory changed while opened")
                _clear_held_directory(child)
            finally:
                os.close(child)
            os.rmdir(name, dir_fd=directory_descriptor)
        else:
            os.unlink(name, dir_fd=directory_descriptor)


def write_package(
    *,
    artifact_path: Path,
    manifest: dict[str, object],
    output_root: Path,
    artifact_snapshot: ArtifactSnapshotV1 | None = None,
) -> Path:
    """Write one fresh content-addressed package without overwriting files."""

    manifest = validate_manifest(manifest)
    if artifact_snapshot is None:
        with _immutable_artifact_snapshot(
            artifact_path, "zk-X509 worker artifact"
        ) as snapshot:
            return write_package(
                artifact_path=artifact_path,
                manifest=manifest,
                output_root=output_root,
                artifact_snapshot=snapshot,
            )
    _revalidate_artifact_snapshot(
        artifact_snapshot, str(manifest["artifact_sha256"])
    )
    output_root = _canonical_absolute_path(output_root, "output_root")
    try:
        root_metadata = output_root.lstat()
    except OSError as error:
        raise ZkX509WorkerPackageError("output_root is unavailable") from error
    if (
        not stat.S_ISDIR(root_metadata.st_mode)
        or stat.S_ISLNK(root_metadata.st_mode)
        or root_metadata.st_uid != os.geteuid()
        or root_metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or root_metadata.st_mode & 0o700 != 0o700
    ):
        _fail("output_root must be one owner-controlled real existing directory")
    package = output_root / str(manifest["artifact_sha256"])
    temporary = output_root / (
        f".zk-x509-worker-{os.getpid()}-{secrets.token_hex(8)}"
    )
    output_descriptor = _open_directory_descriptor(
        output_root, "zk-X509 worker package output root"
    )
    if (
        os.fstat(output_descriptor).st_dev,
        os.fstat(output_descriptor).st_ino,
    ) != (root_metadata.st_dev, root_metadata.st_ino):
        os.close(output_descriptor)
        _fail("output_root changed while it was opened")
    try:
        os.stat(package.name, dir_fd=output_descriptor, follow_symlinks=False)
    except FileNotFoundError:
        pass
    else:
        os.close(output_descriptor)
        _fail("zk-X509 worker package output must be fresh")
    os.mkdir(temporary.name, 0o700, dir_fd=output_descriptor)
    temporary_descriptor = os.open(
        temporary.name,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        dir_fd=output_descriptor,
    )
    active_leaf = temporary.name
    try:
        artifact = artifact_snapshot.record
        if (
            artifact.sha256 != manifest["artifact_sha256"]
            or artifact.size != manifest["artifact_size"]
        ):
            _fail("zk-X509 worker artifact does not match its package manifest")
        packaged_artifact = temporary / ARTIFACT_FILE
        _copy_artifact(
            artifact_snapshot,
            packaged_artifact,
            destination_dir_fd=temporary_descriptor,
        )
        manifest_path = temporary / "manifest.json"
        descriptor = os.open(
            manifest_path.name,
            os.O_CREAT
            | os.O_EXCL
            | os.O_WRONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o400,
            dir_fd=temporary_descriptor,
        )
        try:
            payload = canonical_manifest_bytes(manifest)
            offset = 0
            while offset < len(payload):
                offset += os.write(descriptor, payload[offset:])
            os.fchmod(descriptor, 0o400)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        os.fchmod(temporary_descriptor, 0o500)
        os.fsync(temporary_descriptor)
        expected_root, expected_inventory, expected_payloads = _package_inventory(
            temporary_descriptor
        )
        if (
            expected_payloads["manifest.json"] != payload
            or expected_inventory[ARTIFACT_FILE]["sha256"]
            != manifest["artifact_sha256"]
        ):
            _fail("zk-X509 worker staged package inventory is inconsistent")
        _atomic_rename_noreplace(
            temporary.name,
            package.name,
            source_dir_fd=output_descriptor,
            destination_dir_fd=output_descriptor,
            label="zk-X509 worker package publication",
        )
        active_leaf = package.name
        held = os.fstat(temporary_descriptor)
        published_descriptor = os.open(
            package.name,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=output_descriptor,
        )
        try:
            moved = os.fstat(published_descriptor)
            if (moved.st_dev, moved.st_ino) != (held.st_dev, held.st_ino):
                _fail("zk-X509 worker package root changed while finalized")
            published_root, published_inventory, published_payloads = _package_inventory(
                published_descriptor
            )
            if (
                published_root != expected_root
                or published_inventory != expected_inventory
                or published_payloads != expected_payloads
            ):
                _fail("zk-X509 worker published package inventory changed")
            named = os.stat(
                package.name,
                dir_fd=output_descriptor,
                follow_symlinks=False,
            )
            if (named.st_dev, named.st_ino) != (held.st_dev, held.st_ino):
                _fail("zk-X509 worker published package pathname changed")
            os.fsync(output_descriptor)
            final_root, final_inventory, final_payloads = _package_inventory(
                published_descriptor
            )
            final_named = os.stat(
                package.name,
                dir_fd=output_descriptor,
                follow_symlinks=False,
            )
            if (
                final_root != expected_root
                or final_inventory != expected_inventory
                or final_payloads != expected_payloads
                or (final_named.st_dev, final_named.st_ino)
                != (held.st_dev, held.st_ino)
            ):
                _fail("zk-X509 worker package changed before publication completed")
        finally:
            os.close(published_descriptor)
    except BaseException:
        try:
            named = os.stat(active_leaf, dir_fd=output_descriptor, follow_symlinks=False)
            held = os.fstat(temporary_descriptor)
            if (
                stat.S_ISDIR(named.st_mode)
                and (named.st_dev, named.st_ino) == (held.st_dev, held.st_ino)
            ):
                _clear_held_directory(temporary_descriptor)
                os.rmdir(active_leaf, dir_fd=output_descriptor)
                os.fsync(output_descriptor)
        except OSError:
            pass
        raise
    finally:
        os.close(temporary_descriptor)
        os.close(output_descriptor)
    return package


def _identity_matches_manifest(
    identity: WorkerIdentityV2, manifest: dict[str, object]
) -> bool:
    return (
        identity.cargo_lock_sha256 == manifest["cargo_lock_sha256"]
        and identity.compiled_profile_sha256 == manifest["compiled_profile_sha256"]
        and identity.expectations_json_sha256
        == manifest["expectations_json_sha256"]
        and identity.expectations_norito_sha256
        == manifest["expectations_norito_sha256"]
        and identity.isolation_contract == manifest["isolation_contract"]
        and identity.isolation_package_sha256
        == manifest["isolation_package_sha256"]
        and identity.kat_proof_bytes == manifest["kat_proof_bytes"]
        and identity.kat_proof_sha256 == manifest["kat_proof_sha256"]
        and identity.production_profile_ready == manifest["production_profile_ready"]
        and identity.protocol_id == manifest["protocol_id"]
        and identity.protocol_profile_sha256
        == manifest["protocol_profile_sha256"]
        and identity.protocol_version == manifest["protocol_version"]
        and identity.public_request_schema_version
        == manifest["public_request_schema_version"]
        and identity.qualified_isolation_ready
        == manifest["qualified_isolation_ready"]
        and identity.release_evidence_ready
        == manifest["release_evidence_ready"]
        and identity.release_evidence_sha256
        == manifest["release_evidence_sha256"]
        and identity.resource_certificate_sha256
        == manifest["resource_certificate_sha256"]
        and identity.soundness_certificate_sha256
        == manifest["soundness_certificate_sha256"]
        and identity.source_allowed_signers_sha256
        == manifest["source_allowed_signers_sha256"]
        and identity.source_closure_schema == manifest["source_closure_schema"]
        and identity.source_commit == manifest["source_commit"]
        and identity.source_revocation_sha256
        == manifest["source_revocation_sha256"]
        and identity.source_sha256 == manifest["source_sha256"]
        and identity.workspace_source_manifest_sha256
        == manifest["workspace_source_manifest_sha256"]
    )


def verify_package(
    package: Path,
    *,
    identity_probe: Callable[..., WorkerIdentityV2] | None = None,
    require_release_ready: bool = False,
    trusted_package_root_sha256: str | None = None,
) -> dict[str, object]:
    """Re-authenticate verifier helpers, artifact, manifest, and native identity."""

    # ``identity_probe`` is a unit-test seam. The CLI and every release caller
    # use the native path, which must authenticate the executing package and
    # exact-launch helpers against the package's signed source commit before
    # any worker image is executed.
    native_identity_probe = (
        identity_probe is None or identity_probe is probe_worker_identity
    )
    if identity_probe is None:
        identity_probe = probe_worker_identity
    verifier_source_root: Path | None = None
    if native_identity_probe:
        verifier_source_root = _script_checkout_root_v1()
        _require_source_checkout_identity_v1(verifier_source_root)

    package = _canonical_absolute_path(package, "package")
    try:
        package_metadata = package.lstat()
    except OSError as error:
        raise ZkX509WorkerPackageError("zk-X509 worker package is unavailable") from error
    if (
        not stat.S_ISDIR(package_metadata.st_mode)
        or stat.S_ISLNK(package_metadata.st_mode)
        or package_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(package_metadata.st_mode) != 0o500
    ):
        _fail(
            "zk-X509 worker package directory must be owner-controlled mode 0500"
        )
    try:
        package_descriptor = os.open(
            package,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
    except OSError as error:
        raise ZkX509WorkerPackageError(
            "zk-X509 worker package could not be opened"
        ) from error
    try:
        held_root = os.fstat(package_descriptor)
        if (held_root.st_dev, held_root.st_ino) != (
            package_metadata.st_dev,
            package_metadata.st_ino,
        ):
            _fail("zk-X509 worker package changed while opened")
        initial_root, initial_inventory, initial_payloads = _package_inventory(
            package_descriptor
        )
        manifest = _load_manifest_bytes(initial_payloads["manifest.json"])
        if verifier_source_root is not None:
            _authenticate_source_helpers_v1(
                verifier_source_root,
                str(manifest["source_commit"]),
            )
        if package.name != manifest["artifact_sha256"]:
            _fail("zk-X509 worker package directory is not content-addressed")
        artifact_record = initial_inventory[ARTIFACT_FILE]
        if (
            artifact_record["sha256"] != manifest["artifact_sha256"]
            or artifact_record["size"] != manifest["artifact_size"]
        ):
            _fail("packaged zk-X509 worker artifact does not match its manifest")
        artifact_path = package / ARTIFACT_FILE
        with _immutable_artifact_snapshot(
            artifact_path,
            "packaged zk-X509 worker artifact",
            source_dir_fd=package_descriptor,
            source_name=ARTIFACT_FILE,
        ) as snapshot:
            if not hmac.compare_digest(
                snapshot.record.sha256, str(manifest["artifact_sha256"])
            ):
                _fail("packaged zk-X509 worker snapshot differs from its manifest")
            if manifest["target"] == RELEASE_TARGET:
                _validate_static_aarch64_elf_bytes(
                    _artifact_snapshot_payload(snapshot)
                )
            if native_identity_probe:
                identity = _probe_worker_identity_snapshot(
                    snapshot,
                    str(manifest["artifact_sha256"]),
                    source_root=verifier_source_root,
                )
            else:
                assert identity_probe is not None
                identity = identity_probe(
                    snapshot.path,
                    expected_artifact_sha256=str(manifest["artifact_sha256"]),
                )
            _revalidate_artifact_snapshot(
                snapshot, str(manifest["artifact_sha256"])
            )
        if not _identity_matches_manifest(identity, manifest):
            _fail("packaged zk-X509 worker identity does not match its manifest")
        if require_release_ready and manifest["release_ready"] is not True:
            _fail(
                "zk-X509 worker package is a non-release candidate: reviewed profile "
                "pins or qualified Linux isolation are unavailable"
            )
        if require_release_ready or trusted_package_root_sha256 is not None:
            _require_trusted_package_root_v1(manifest, trusted_package_root_sha256)
        if verifier_source_root is not None:
            _authenticate_source_helpers_v1(
                verifier_source_root,
                str(manifest["source_commit"]),
            )
        final_root, final_inventory, final_payloads = _package_inventory(
            package_descriptor
        )
        named_after = package.lstat()
        if (
            final_root != initial_root
            or final_inventory != initial_inventory
            or final_payloads != initial_payloads
            or (named_after.st_dev, named_after.st_ino)
            != (held_root.st_dev, held_root.st_ino)
        ):
            _fail("zk-X509 worker package changed during verification")
        return manifest
    finally:
        os.close(package_descriptor)


def _collect_from_args(args: argparse.Namespace) -> SourceEvidenceV1:
    return collect_source_evidence(
        args.source_root,
        allowed_signers=args.allowed_signers,
        expected_allowed_signers_sha256=args.allowed_signers_sha256,
        revocation=args.revocation,
        expected_revocation_sha256=args.revocation_sha256,
        expected_signer_principal=args.signer_principal,
        expected_signer_fingerprint=args.signer_fingerprint,
    )


def _create_package(
    args: argparse.Namespace,
    artifact_path: Path,
    source: SourceEvidenceV1,
    *,
    artifact_build_method: str,
    artifact_build_command_sha256: str | None,
    artifact_build_provenance: dict[str, object] | None,
    helper_source_root: Path | None = None,
) -> Path:
    source_root = args.source_root.resolve(strict=True)
    _require_source_checkout_identity_v1(source_root)
    _authenticate_source_helpers_v1(source_root, source.commit)
    output_root = args.output_root.resolve(strict=True)
    try:
        output_root.relative_to(source_root)
    except ValueError:
        pass
    else:
        _fail("zk-X509 worker package output must be outside the source tree")
    artifact_path = artifact_path.resolve(strict=True)
    with _immutable_artifact_snapshot(
        artifact_path, "zk-X509 worker artifact"
    ) as artifact_snapshot:
        artifact = artifact_snapshot.record
        if args.target == RELEASE_TARGET:
            _validate_static_aarch64_elf_bytes(
                _artifact_snapshot_payload(artifact_snapshot)
            )
        identity = _probe_worker_identity_snapshot(
            artifact_snapshot,
            artifact.sha256,
            source_root=(
                source_root if helper_source_root is None else helper_source_root
            ),
        )
        _authenticate_source_helpers_v1(source_root, source.commit)
        manifest = build_manifest(
            artifact=artifact,
            identity=identity,
            source=source,
            target=args.target,
            artifact_build_method=artifact_build_method,
            artifact_build_command_sha256=artifact_build_command_sha256,
            artifact_build_provenance=artifact_build_provenance,
        )
        if args.require_release_ready and manifest["release_ready"] is not True:
            _fail(
                "zk-X509 worker release package requires reviewed profile pins and "
                "qualified Linux isolation"
            )
        if args.require_release_ready:
            _require_trusted_package_root_v1(
                manifest,
                args.trusted_package_root_sha256,
            )
        package = write_package(
            artifact_path=artifact_path,
            manifest=manifest,
            output_root=output_root,
            artifact_snapshot=artifact_snapshot,
        )
    verify_package(
        package,
        require_release_ready=args.require_release_ready,
        trusted_package_root_sha256=args.trusted_package_root_sha256,
    )
    _authenticate_source_helpers_v1(source_root, source.commit)
    return package


def _stable_build_input_record(
    path: Path,
    *,
    label: str,
    require_executable: bool = False,
) -> dict[str, object]:
    """Hash one root- or process-owner-controlled tool without rejecting hardlinks."""

    path = path.resolve(strict=True)
    try:
        before = path.lstat()
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink < 1
        or not 1 <= before.st_size <= _MAX_TOOL_FILE_BYTES
        or before.st_uid not in (0, os.geteuid())
        or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or (require_executable and not before.st_mode & stat.S_IXUSR)
    ):
        _fail(f"{label} is not an admissible build input")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} could not be opened safely") from error
    digest = hashlib.sha256()
    size = 0
    try:
        opened = os.fstat(descriptor)
        expected = (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_uid,
            before.st_nlink,
            before.st_size,
            before.st_mtime_ns,
        )
        if (
            opened.st_dev,
            opened.st_ino,
            opened.st_mode,
            opened.st_uid,
            opened.st_nlink,
            opened.st_size,
            opened.st_mtime_ns,
        ) != expected:
            _fail(f"{label} changed before it was opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            size += len(chunk)
            if size > _MAX_TOOL_FILE_BYTES:
                _fail(f"{label} exceeds its size bound")
            digest.update(chunk)
        after = os.fstat(descriptor)
        if (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_uid,
            after.st_nlink,
            after.st_size,
            after.st_mtime_ns,
        ) != expected:
            _fail(f"{label} changed while it was read")
    finally:
        os.close(descriptor)
    if size != before.st_size:
        _fail(f"{label} changed while it was read")
    return _validate_build_file_record(
        {
            "mode": stat.S_IMODE(before.st_mode),
            "owner": before.st_uid,
            "path": os.fspath(path),
            "sha256": digest.hexdigest(),
            "size": size,
        },
        label,
    )


def _resolve_build_executable(
    name: str,
    environment: Mapping[str, str],
    *,
    label: str,
) -> Path:
    path_value = environment.get("PATH")
    if not path_value:
        _fail(f"{label} cannot be resolved without PATH")
    try:
        located = shutil.which(name, path=path_value)
    except (OSError, ValueError) as error:
        raise ZkX509WorkerPackageError(f"{label} could not be resolved") from error
    if located is None:
        _fail(f"{label} is unavailable on the frozen PATH")
    try:
        resolved = Path(located).resolve(strict=True)
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} could not be resolved") from error
    _stable_build_input_record(resolved, label=label, require_executable=True)
    return resolved


def _locate_build_executable(
    name: str,
    environment: Mapping[str, str],
    *,
    label: str,
) -> Path:
    """Return the PATH spelling, preserving dispatch-sensitive symlink argv[0]."""

    path_value = environment.get("PATH")
    located = shutil.which(name, path=path_value) if path_value else None
    if located is None:
        _fail(f"{label} is unavailable on the frozen PATH")
    invocation = Path(located)
    if not invocation.is_absolute():
        _fail(f"{label} did not resolve to an absolute invocation path")
    _stable_build_input_record(
        invocation.resolve(strict=True),
        label=label,
        require_executable=True,
    )
    return invocation


def _run_build_tool(
    executable: Path,
    arguments: Sequence[str],
    *,
    source_root: Path,
    environment: Mapping[str, str],
    label: str,
) -> bytes:
    try:
        completed = _run_bounded_process(
            [os.fspath(executable), *arguments],
            cwd=source_root,
            environment=environment,
            timeout=30,
            stdout_limit=_MAX_TOOL_OUTPUT_BYTES,
            stderr_limit=_MAX_TOOL_OUTPUT_BYTES,
        )
    except (OSError, _BoundedProcessError) as error:
        raise ZkX509WorkerPackageError(f"{label} probe failed") from error
    if (
        completed.returncode != 0
        or not 1 <= len(completed.stdout) <= _MAX_TOOL_OUTPUT_BYTES
        or len(completed.stderr) > _MAX_TOOL_OUTPUT_BYTES
    ):
        _fail(f"{label} probe failed")
    return completed.stdout


def _rust_component_closure_record(
    sysroot: Path,
    manifest_name: str,
    *,
    label: str,
) -> dict[str, object]:
    manifest = sysroot / "lib" / "rustlib" / manifest_name
    manifest_payload = _stable_bytes(
        manifest,
        label=f"{label} manifest",
        maximum=4 * 1024 * 1024,
    )
    try:
        text = manifest_payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ZkX509WorkerPackageError(f"{label} manifest is not UTF-8") from error
    if not text.endswith("\n") or "\r" in text:
        _fail(f"{label} manifest is not canonical LF text")
    relative_paths: list[PurePosixPath] = []
    for line in text.splitlines():
        if not line.startswith("file:"):
            _fail(f"{label} manifest contains an unsupported entry")
        raw_path = line[5:]
        relative = PurePosixPath(raw_path)
        if (
            not raw_path
            or raw_path.startswith("/")
            or relative.as_posix() != raw_path
            or any(part in ("", ".", "..") for part in relative.parts)
        ):
            _fail(f"{label} manifest contains a non-canonical path")
        relative_paths.append(relative)
    if len(relative_paths) != len(set(relative_paths)) or not relative_paths:
        _fail(f"{label} manifest inventory is invalid")
    digest = hashlib.sha256()
    digest.update(_RUST_COMPONENT_CLOSURE_DOMAIN)
    encoded_name = manifest_name.encode("utf-8")
    digest.update(len(encoded_name).to_bytes(2, "big"))
    digest.update(encoded_name)
    digest.update(len(manifest_payload).to_bytes(8, "big"))
    digest.update(manifest_payload)
    total_bytes = len(manifest_payload)
    for relative in relative_paths:
        candidate = sysroot.joinpath(*relative.parts)
        try:
            resolved = candidate.resolve(strict=True)
            resolved.relative_to(sysroot)
        except (OSError, ValueError) as error:
            raise ZkX509WorkerPackageError(
                f"{label} file escapes the Rust sysroot"
            ) from error
        if resolved != candidate:
            _fail(f"{label} contains a symlinked component")
        record = _stable_build_input_record(resolved, label=f"{label} file")
        encoded_path = relative.as_posix().encode("utf-8")
        digest.update(len(encoded_path).to_bytes(2, "big"))
        digest.update(encoded_path)
        digest.update(int(record["size"]).to_bytes(8, "big"))
        digest.update(bytes.fromhex(str(record["sha256"])))
        total_bytes += int(record["size"])
    return _validate_component_record(
        {
            "closure_sha256": digest.hexdigest(),
            "file_count": len(relative_paths),
            "manifest_path": os.fspath(manifest),
            "manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
            "total_bytes": total_bytes,
        },
        label,
    )


def _path_with_build_tools_first(original: str, *directories: Path) -> str:
    parts = original.split(os.pathsep)
    if any(not part or not Path(part).is_absolute() for part in parts):
        _fail("frozen zk-X509 worker PATH must contain only absolute entries")
    prefixed: list[str] = []
    for item in (*map(os.fspath, directories), *parts):
        if item not in prefixed:
            prefixed.append(item)
    return os.pathsep.join(prefixed)


def _target_compiler_names(host: str, target: str) -> tuple[tuple[str, ...], tuple[str, ...]]:
    if host == target:
        return ("cc", "clang", "gcc"), ("ar", "llvm-ar")
    return (
        (f"{target}-gcc", f"{target}-cc"),
        (f"{target}-ar", "llvm-ar"),
    )


def _first_resolved_build_executable(
    names: Sequence[str],
    environment: Mapping[str, str],
    *,
    label: str,
) -> Path:
    for name in names:
        if shutil.which(name, path=environment.get("PATH")) is not None:
            return _resolve_build_executable(name, environment, label=label)
    _fail(f"{label} is unavailable on the frozen PATH")


def _descriptor_path(descriptor: int, label: str) -> Path:
    """Return a magic-link spelling that remains anchored to ``descriptor``."""

    opened = os.fstat(descriptor)
    for directory in (Path("/proc/self/fd"), Path("/dev/fd")):
        candidate = directory / str(descriptor)
        try:
            observed = candidate.stat()
        except OSError:
            continue
        if (
            observed.st_ino == opened.st_ino
            and stat.S_IFMT(observed.st_mode) == stat.S_IFMT(opened.st_mode)
            and observed.st_uid == opened.st_uid
            and (
                observed.st_dev == opened.st_dev
                or directory == Path("/dev/fd")
            )
        ):
            return candidate
    _fail(f"{label} cannot be named through an open descriptor")


def _open_directory_descriptor(path: Path, label: str) -> int:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} could not be opened") from error
    if not stat.S_ISDIR(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        _fail(f"{label} is not a directory")
    return descriptor


def _require_owner_build_root(path: Path, label: str) -> Path:
    try:
        resolved = path.resolve(strict=True)
        metadata = resolved.lstat()
    except OSError as error:
        raise ZkX509WorkerPackageError(f"{label} is unavailable") from error
    if (
        not path.is_absolute()
        or path != resolved
        or not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or metadata.st_mode & 0o700 != 0o700
    ):
        _fail(f"{label} must be one canonical owner-private existing directory")
    return resolved


def _open_snapshot_parent(root_descriptor: int, parts: Sequence[str]) -> int:
    descriptor = os.dup(root_descriptor)
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        for part in parts:
            try:
                os.mkdir(part, 0o700, dir_fd=descriptor)
            except FileExistsError:
                pass
            next_descriptor = os.open(part, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = next_descriptor
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _seal_snapshot_tree(root_descriptor: int) -> None:
    """Make every directory read-only through the held snapshot root inode."""

    def seal(directory_descriptor: int) -> None:
        before = os.fstat(directory_descriptor)
        for name in os.listdir(directory_descriptor):
            details = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
            if stat.S_ISDIR(details.st_mode):
                child = os.open(
                    name,
                    os.O_RDONLY
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_DIRECTORY", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                    dir_fd=directory_descriptor,
                )
                try:
                    opened = os.fstat(child)
                    if (opened.st_dev, opened.st_ino, opened.st_uid) != (
                        details.st_dev,
                        details.st_ino,
                        details.st_uid,
                    ):
                        _fail("signed source snapshot directory changed while opened")
                    seal(child)
                finally:
                    os.close(child)
            elif stat.S_ISREG(details.st_mode):
                if stat.S_IMODE(details.st_mode) not in (0o400, 0o500):
                    _fail("signed source snapshot file mode is not sealed")
            elif not stat.S_ISLNK(details.st_mode):
                _fail("signed source snapshot contains a special file")
        os.fchmod(directory_descriptor, 0o500)
        after = os.fstat(directory_descriptor)
        if (
            (after.st_dev, after.st_ino, after.st_uid, after.st_nlink, after.st_mtime_ns)
            != (before.st_dev, before.st_ino, before.st_uid, before.st_nlink, before.st_mtime_ns)
            or stat.S_IMODE(after.st_mode) != 0o500
        ):
            _fail("signed source snapshot directory changed while sealed")

    seal(root_descriptor)


def _snapshot_member_path(name: str) -> PurePosixPath:
    spelling = name[:-1] if name.endswith("/") else name
    relative = PurePosixPath(spelling)
    if (
        not spelling
        or spelling.startswith("/")
        or relative.as_posix() != spelling
        or any(part in ("", ".", "..") for part in relative.parts)
    ):
        _fail("signed source archive contains a non-canonical path")
    return relative


def _signed_tree_inventory(
    source_root: Path,
    root_tree: str,
) -> dict[PurePosixPath, tuple[str, str]]:
    """Recompute every raw Git tree object linking the signed root to its leaves."""

    root_tree = _require_commit(root_tree, "signed source root tree")
    expected: dict[PurePosixPath, tuple[str, str]] = {}
    visited_entries = 0

    def walk(tree_id: str, prefix: PurePosixPath, ancestors: frozenset[str]) -> None:
        nonlocal visited_entries
        if tree_id in ancestors or len(ancestors) >= 128:
            _fail("signed source tree nesting is cyclic or exceeds its depth ceiling")
        payload = _git_bytes(source_root, ("cat-file", "tree", tree_id))
        observed_id = hashlib.sha1(
            b"tree " + str(len(payload)).encode("ascii") + b"\0" + payload
        ).hexdigest()
        if not hmac.compare_digest(observed_id, tree_id):
            _fail("raw signed source tree bytes do not hash to their object ID")
        offset = 0
        names: set[str] = set()
        while offset < len(payload):
            space = payload.find(b" ", offset)
            nul = payload.find(b"\0", space + 1 if space >= 0 else offset)
            if space <= offset or nul <= space + 1 or nul + 21 > len(payload):
                _fail("raw signed source tree entry is truncated")
            raw_mode = payload[offset:space]
            raw_name = payload[space + 1 : nul]
            object_id = payload[nul + 1 : nul + 21].hex()
            offset = nul + 21
            try:
                mode = raw_mode.decode("ascii")
                name = raw_name.decode("utf-8")
            except UnicodeError as error:
                raise ZkX509WorkerPackageError(
                    "signed source tree contains a non-UTF-8 path or mode"
                ) from error
            if (
                mode not in {"40000", "100644", "100755", "120000", "160000"}
                or not name
                or name in {".", ".."}
                or "/" in name
                or "\0" in name
                or name in names
            ):
                _fail("raw signed source tree contains an unsupported entry")
            names.add(name)
            visited_entries += 1
            if visited_entries > _MAX_CARGO_CACHE_ENTRIES:
                _fail("signed source tree exceeds its entry ceiling")
            relative = prefix / name
            if mode == "40000":
                walk(tree_id=object_id, prefix=relative, ancestors=ancestors | {tree_id})
            else:
                if relative in expected:
                    _fail("signed source tree contains a duplicate leaf")
                expected[relative] = (mode, object_id)
        if offset != len(payload):
            _fail("raw signed source tree has trailing bytes")

    walk(root_tree, PurePosixPath(), frozenset())
    if not expected:
        _fail("signed source tree inventory is empty")
    return expected


def _export_signed_source_snapshot(
    source_root: Path,
    commit: str,
    destination: Path,
) -> SignedSourceSnapshotV1:
    """Export ``commit`` without consuming mutable checkout file bytes."""

    source_root = source_root.resolve(strict=True)
    commit = _require_commit(commit, "signed source snapshot commit")
    if not destination.is_absolute() or destination != destination.resolve(strict=False):
        _fail("signed source snapshot destination must be canonical and absolute")
    if destination.exists() or destination.is_symlink():
        _fail("signed source snapshot destination must be fresh")
    destination.mkdir(mode=0o700)
    destination.chmod(0o700)
    root_descriptor = _open_directory_descriptor(
        destination, "signed source snapshot root"
    )
    archive_file = None
    try:
        raw_commit = _git_bytes(source_root, ("cat-file", "commit", commit))
        raw_commit_id = hashlib.sha1(
            b"commit " + str(len(raw_commit)).encode("ascii") + b"\0" + raw_commit
        ).hexdigest()
        tree_headers = [
            value for name, value in _parse_commit_headers(raw_commit) if name == b"tree"
        ]
        if (
            not hmac.compare_digest(raw_commit_id, commit)
            or len(tree_headers) != 1
            or re.fullmatch(rb"[0-9a-f]{40}", tree_headers[0]) is None
        ):
            _fail("signed source snapshot commit/tree identity is invalid")
        root_tree = tree_headers[0].decode("ascii")
        expected = _signed_tree_inventory(source_root, root_tree)
        expected_directories: set[PurePosixPath] = set()
        for relative in expected:
            current = relative.parent
            while current.parts:
                expected_directories.add(current)
                current = current.parent
        # An unnamed temporary file prevents a pathname replacement between Git
        # production and descriptor-based tar consumption.
        archive_file = tempfile.TemporaryFile(mode="w+b", dir=destination.parent)
        archive_descriptor = archive_file.fileno()
        try:
            completed = _run_bounded_process(
                [_SYSTEM_GIT, "-C", os.fspath(source_root), "archive", "--format=tar", root_tree],
                cwd=source_root,
                environment=_git_environment(),
                timeout=600,
                stdout_limit=_MAX_SIGNED_SOURCE_ARCHIVE_BYTES,
                stderr_limit=_MAX_TOOL_OUTPUT_BYTES,
                stdout_sink=archive_descriptor,
                capture_stdout=False,
            )
        except (OSError, _BoundedProcessError) as error:
            raise ZkX509WorkerPackageError("signed source snapshot export failed") from error
        if completed.returncode != 0 or len(completed.stderr) > _MAX_TOOL_OUTPUT_BYTES:
            _fail("signed source snapshot export failed")
        os.lseek(archive_descriptor, 0, os.SEEK_SET)
        seen: set[PurePosixPath] = set()
        signed_seen: set[PurePosixPath] = set()
        total_bytes = 0
        with os.fdopen(os.dup(archive_descriptor), "rb") as archive_stream:
            with tarfile.open(fileobj=archive_stream, mode="r|") as archive:
                for member in archive:
                    relative = _snapshot_member_path(member.name)
                    if relative in seen:
                        _fail("signed source archive contains a duplicate path")
                    seen.add(relative)
                    parent = _open_snapshot_parent(root_descriptor, relative.parts[:-1])
                    try:
                        leaf = relative.parts[-1]
                        if member.isdir():
                            if relative not in expected_directories and not (
                                relative in expected and expected[relative][0] == "160000"
                            ):
                                _fail("signed source archive contains an untracked directory")
                            if relative in expected:
                                signed_seen.add(relative)
                            try:
                                os.mkdir(leaf, 0o700, dir_fd=parent)
                            except FileExistsError:
                                existing = os.stat(leaf, dir_fd=parent, follow_symlinks=False)
                                if not stat.S_ISDIR(existing.st_mode):
                                    _fail("signed source archive directory collides with a file")
                            continue
                        if member.issym():
                            tree_entry = expected.get(relative)
                            if tree_entry is None or tree_entry[0] != "120000":
                                _fail("signed source archive symlink is not in the signed tree")
                            target = PurePosixPath(member.linkname)
                            depth = len(relative.parent.parts)
                            for part in target.parts:
                                if part == "..":
                                    depth -= 1
                                elif part not in ("", "."):
                                    depth += 1
                                if depth < 0:
                                    break
                            if not member.linkname or member.linkname.startswith("/") or depth < 0:
                                _fail("signed source archive contains an escaping symlink")
                            link_payload = member.linkname.encode("utf-8")
                            link_object_id = hashlib.sha1(
                                b"blob "
                                + str(len(link_payload)).encode("ascii")
                                + b"\0"
                                + link_payload
                            ).hexdigest()
                            if not hmac.compare_digest(link_object_id, tree_entry[1]):
                                _fail("signed source archive symlink differs from its Git blob")
                            os.symlink(member.linkname, leaf, dir_fd=parent)
                            signed_seen.add(relative)
                            continue
                        if not member.isfile() or member.size < 0 or member.size > _MAX_TOOL_FILE_BYTES:
                            _fail("signed source archive contains an unsupported member")
                        tree_entry = expected.get(relative)
                        expected_mode = "100755" if member.mode & 0o111 else "100644"
                        if tree_entry is None or tree_entry[0] != expected_mode:
                            _fail("signed source archive file mode is not in the signed tree")
                        total_bytes += member.size
                        if total_bytes > 16 * 1024 * 1024 * 1024:
                            _fail("signed source archive exceeds its total size ceiling")
                        source = archive.extractfile(member)
                        if source is None:
                            _fail("signed source archive file payload is unavailable")
                        descriptor = os.open(
                            leaf,
                            os.O_WRONLY
                            | os.O_CREAT
                            | os.O_EXCL
                            | getattr(os, "O_CLOEXEC", 0)
                            | getattr(os, "O_NOFOLLOW", 0),
                            0o600,
                            dir_fd=parent,
                        )
                        copied = 0
                        blob_digest = hashlib.sha1(
                            b"blob " + str(member.size).encode("ascii") + b"\0"
                        )
                        try:
                            while True:
                                chunk = source.read(1024 * 1024)
                                if not chunk:
                                    break
                                copied += len(chunk)
                                blob_digest.update(chunk)
                                offset = 0
                                while offset < len(chunk):
                                    offset += os.write(descriptor, chunk[offset:])
                            if copied != member.size:
                                _fail("signed source archive file was truncated")
                            if not hmac.compare_digest(
                                blob_digest.hexdigest(), tree_entry[1]
                            ):
                                _fail("signed source archive file differs from its Git blob")
                            os.fchmod(descriptor, 0o500 if member.mode & 0o111 else 0o400)
                            os.fsync(descriptor)
                            signed_seen.add(relative)
                        finally:
                            os.close(descriptor)
                            source.close()
                    finally:
                        os.close(parent)
        if set(expected) != signed_seen:
            _fail("signed source archive inventory is not the exact signed Git tree")
        if not (destination / "Cargo.toml").is_file():
            _fail("signed source snapshot lacks Cargo.toml")
        _seal_snapshot_tree(root_descriptor)
        anchored = os.fstat(root_descriptor)
        named = destination.lstat()
        if (anchored.st_dev, anchored.st_ino) != (named.st_dev, named.st_ino):
            _fail("signed source snapshot root changed during export")
        return SignedSourceSnapshotV1(destination, root_descriptor)
    except BaseException:
        os.close(root_descriptor)
        raise
    finally:
        if archive_file is not None:
            archive_file.close()


def _closed_cargo_home(
    destination: Path,
    base_environment: Mapping[str, str],
) -> ClosedCargoHomeV1:
    """Create a config-free Cargo home with descriptor-anchored offline caches."""

    if destination.exists() or destination.is_symlink():
        _fail("closed Cargo home must be fresh")
    destination.mkdir(mode=0o700)
    destination.chmod(0o700)
    inherited = Path(
        base_environment.get(
            "CARGO_HOME", os.fspath(Path(base_environment["HOME"]) / ".cargo")
        )
    )
    if not inherited.is_absolute():
        _fail("inherited Cargo cache home must be absolute")
    inherited = inherited.resolve(strict=True)
    descriptors: list[int] = []
    try:
        for name in ("registry", "git"):
            cache_root = inherited / name
            if not cache_root.exists():
                continue
            descriptor = _open_directory_descriptor(cache_root, f"Cargo {name} cache")
            metadata = os.fstat(descriptor)
            if (
                metadata.st_uid not in (0, os.geteuid())
                or metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
            ):
                os.close(descriptor)
                _fail(f"Cargo {name} cache is not owner-controlled")
            # Cargo must consume the exact inode that is sealed below, even if
            # the inherited cache pathname is concurrently replaced.
            os.symlink(
                os.fspath(_descriptor_path(descriptor, f"Cargo {name} cache")),
                destination / name,
            )
            descriptors.append(descriptor)
        return ClosedCargoHomeV1(destination, tuple(descriptors))
    except BaseException:
        for descriptor in descriptors:
            os.close(descriptor)
        raise


def _require_cache_path_identity(
    root_descriptor: int,
    root_path: Path,
    role: str,
    expected_record: Mapping[str, object] | None = None,
) -> os.stat_result:
    """Require one durable cache pathname to name the held root inode."""

    if role not in {"git", "registry"} or not root_path.is_absolute():
        _fail("Cargo cache path identity request is invalid")
    try:
        named = root_path.lstat()
        canonical = root_path.resolve(strict=True)
        held = os.fstat(root_descriptor)
    except OSError as error:
        raise ZkX509WorkerPackageError(
            f"Cargo {role} durable cache path is unavailable"
        ) from error
    if (
        canonical != root_path
        or stat.S_ISLNK(named.st_mode)
        or not stat.S_ISDIR(named.st_mode)
        or (named.st_dev, named.st_ino) != (held.st_dev, held.st_ino)
        or named.st_mode != held.st_mode
        or named.st_uid != held.st_uid
    ):
        _fail(f"Cargo {role} durable cache path differs from its held root")
    if expected_record is not None and (
        expected_record.get("path") != os.fspath(root_path)
        or expected_record.get("device") != held.st_dev
        or expected_record.get("inode") != held.st_ino
        or expected_record.get("mode") != stat.S_IMODE(held.st_mode)
        or expected_record.get("owner") != held.st_uid
    ):
        _fail(f"Cargo {role} durable cache identity differs from its provenance")
    return held


def _cargo_cache_tree_record(
    root_descriptor: int,
    root_path: Path,
    role: str,
) -> dict[str, object]:
    """Seal one Cargo cache tree through a held root directory descriptor."""

    if role not in {"git", "registry"}:
        _fail("Cargo cache role is invalid")
    try:
        root_path = root_path.resolve(strict=True)
    except OSError as error:
        raise ZkX509WorkerPackageError(
            f"Cargo {role} durable cache path cannot be resolved"
        ) from error
    _require_cache_path_identity(root_descriptor, root_path, role)
    root_before = os.fstat(root_descriptor)
    if (
        not stat.S_ISDIR(root_before.st_mode)
        or root_before.st_uid not in (0, os.geteuid())
        or root_before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
    ):
        _fail(f"Cargo {role} cache root is not owner-controlled")
    entry_count = 0
    total_file_bytes = 0
    entries: list[dict[str, object]] = []

    def identity(details: os.stat_result) -> tuple[int, ...]:
        return (
            details.st_dev,
            details.st_ino,
            details.st_mode,
            details.st_uid,
            details.st_nlink,
            details.st_size,
            details.st_mtime_ns,
            details.st_ctime_ns,
        )

    def safe_symlink(relative: PurePosixPath, target: str) -> None:
        target_path = PurePosixPath(target)
        if not target or "\0" in target or target_path.is_absolute():
            _fail(f"Cargo {role} cache contains an unsafe symlink")
        stack = list(relative.parent.parts)
        for part in target_path.parts:
            if part in ("", "."):
                continue
            if part == "..":
                if not stack:
                    _fail(f"Cargo {role} cache symlink escapes its root")
                stack.pop()
            else:
                stack.append(part)

    def walk(directory_descriptor: int, prefix: PurePosixPath) -> None:
        nonlocal entry_count, total_file_bytes
        directory_before = os.fstat(directory_descriptor)
        try:
            names = sorted(os.listdir(directory_descriptor))
        except OSError as error:
            raise ZkX509WorkerPackageError(
                f"Cargo {role} cache inventory is unreadable"
            ) from error
        if any(name in ("", ".", "..") or "/" in name or "\0" in name for name in names):
            _fail(f"Cargo {role} cache contains a non-canonical entry name")
        for name in names:
            entry_count += 1
            if entry_count > _MAX_CARGO_CACHE_ENTRIES:
                _fail(f"Cargo {role} cache exceeds its entry ceiling")
            relative = prefix / name
            try:
                before = os.stat(
                    name,
                    dir_fd=directory_descriptor,
                    follow_symlinks=False,
                )
            except OSError as error:
                raise ZkX509WorkerPackageError(
                    f"Cargo {role} cache entry is unavailable: {relative}"
                ) from error
            mode = stat.S_IMODE(before.st_mode)
            if stat.S_ISDIR(before.st_mode):
                if (
                    before.st_uid not in (0, os.geteuid())
                    or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
                ):
                    _fail(f"Cargo {role} cache contains an uncontrolled directory")
                entries.append(
                    {
                        "kind": "directory",
                        "mode": mode,
                        "owner": before.st_uid,
                        "path": relative.as_posix(),
                    }
                )
                child = os.open(
                    name,
                    os.O_RDONLY
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_DIRECTORY", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                    dir_fd=directory_descriptor,
                )
                try:
                    if identity(os.fstat(child)) != identity(before):
                        _fail(f"Cargo {role} cache directory changed while opened")
                    walk(child, relative)
                    if identity(os.fstat(child)) != identity(before):
                        _fail(f"Cargo {role} cache directory changed while sealed")
                finally:
                    os.close(child)
            elif stat.S_ISREG(before.st_mode):
                if (
                    before.st_nlink != 1
                    or before.st_uid not in (0, os.geteuid())
                    or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
                    or not 0 <= before.st_size <= _MAX_CARGO_CACHE_FILE_BYTES
                ):
                    _fail(f"Cargo {role} cache contains an uncontrolled file")
                descriptor = os.open(
                    name,
                    os.O_RDONLY
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                    dir_fd=directory_descriptor,
                )
                digest = hashlib.sha256()
                try:
                    opened = os.fstat(descriptor)
                    if identity(opened) != identity(before):
                        _fail(f"Cargo {role} cache file changed while opened")
                    remaining = opened.st_size
                    while remaining:
                        chunk = os.read(descriptor, min(1024 * 1024, remaining))
                        if not chunk:
                            _fail(f"Cargo {role} cache file was truncated")
                        digest.update(chunk)
                        remaining -= len(chunk)
                    if os.read(descriptor, 1) or identity(os.fstat(descriptor)) != identity(before):
                        _fail(f"Cargo {role} cache file changed while sealed")
                finally:
                    os.close(descriptor)
                total_file_bytes += before.st_size
                if total_file_bytes > _MAX_CARGO_CACHE_TOTAL_BYTES:
                    _fail(f"Cargo {role} cache exceeds its byte ceiling")
                entries.append(
                    {
                        "kind": "file",
                        "mode": mode,
                        "owner": before.st_uid,
                        "path": relative.as_posix(),
                        "sha256": digest.hexdigest(),
                        "size": before.st_size,
                    }
                )
            elif stat.S_ISLNK(before.st_mode):
                try:
                    target = os.readlink(name, dir_fd=directory_descriptor)
                    followed = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=True)
                    after = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
                except OSError as error:
                    raise ZkX509WorkerPackageError(
                        f"Cargo {role} cache contains a dangling symlink"
                    ) from error
                safe_symlink(relative, target)
                if identity(after) != identity(before) or not (
                    stat.S_ISREG(followed.st_mode) or stat.S_ISDIR(followed.st_mode)
                ):
                    _fail(f"Cargo {role} cache symlink changed or names a special file")
                entries.append(
                    {
                        "kind": "symlink",
                        "owner": before.st_uid,
                        "path": relative.as_posix(),
                        "target": target,
                    }
                )
            else:
                _fail(f"Cargo {role} cache contains a special file")
        if identity(os.fstat(directory_descriptor)) != identity(directory_before):
            _fail(f"Cargo {role} cache directory changed during traversal")

    walk(root_descriptor, PurePosixPath())
    root_after = os.fstat(root_descriptor)
    if identity(root_after) != identity(root_before):
        _fail(f"Cargo {role} cache root changed while it was sealed")
    _require_cache_path_identity(root_descriptor, root_path, role)
    digest = hashlib.sha256()
    digest.update(_CARGO_CACHE_TREE_DOMAIN)
    encoded_role = role.encode("ascii")
    digest.update(len(encoded_role).to_bytes(2, "big"))
    digest.update(encoded_role)
    payload = _canonical_json_bytes(entries)
    digest.update(len(payload).to_bytes(8, "big"))
    digest.update(payload)
    return _validate_cargo_cache_root_record(
        {
            "device": root_before.st_dev,
            "entry_count": entry_count,
            "inode": root_before.st_ino,
            "mode": stat.S_IMODE(root_before.st_mode),
            "owner": root_before.st_uid,
            "path": os.fspath(root_path),
            "role": role,
            "total_file_bytes": total_file_bytes,
            "tree_sha256": digest.hexdigest(),
        },
        role,
    )


def _materialize_durable_cargo_cache_links(
    cargo_home: ClosedCargoHomeV1,
    cache_records: Mapping[str, object],
) -> None:
    """Replace build-only fd links with provenance-checkable durable targets."""

    home_descriptor = _open_directory_descriptor(
        cargo_home.root, "closed Cargo home"
    )
    try:
        descriptor_index = 0
        for role in ("registry", "git"):
            record = cache_records.get(role)
            if record is None:
                continue
            if descriptor_index >= len(cargo_home.cache_descriptors):
                raise AssertionError("closed Cargo cache descriptor inventory drift")
            cache_descriptor = cargo_home.cache_descriptors[descriptor_index]
            record = _validate_cargo_cache_root_record(record, role)
            durable_path = Path(str(record["path"]))
            _require_cache_path_identity(
                cache_descriptor, durable_path, role, record
            )
            expected_link = os.fspath(
                _descriptor_path(cache_descriptor, f"Cargo {role} cache")
            )
            details = os.stat(role, dir_fd=home_descriptor, follow_symlinks=False)
            if not stat.S_ISLNK(details.st_mode) or os.readlink(
                role, dir_fd=home_descriptor
            ) != expected_link:
                _fail(f"Cargo {role} descriptor link changed during the build")
            os.unlink(role, dir_fd=home_descriptor)
            os.symlink(os.fspath(durable_path), role, dir_fd=home_descriptor)
            followed = os.stat(role, dir_fd=home_descriptor, follow_symlinks=True)
            held = os.fstat(cache_descriptor)
            if (followed.st_dev, followed.st_ino) != (held.st_dev, held.st_ino):
                _fail(f"Cargo {role} durable cache link names a different inode")
            _require_cache_path_identity(
                cache_descriptor, durable_path, role, record
            )
            descriptor_index += 1
        if descriptor_index != len(cargo_home.cache_descriptors):
            raise AssertionError("closed Cargo cache descriptor inventory drift")
        os.fsync(home_descriptor)
    finally:
        os.close(home_descriptor)


def _cargo_configuration_records(
    invocation_root: Path,
    environment: Mapping[str, str],
) -> list[dict[str, object]]:
    cargo_home = Path(
        environment.get("CARGO_HOME", os.fspath(Path(environment["HOME"]) / ".cargo"))
    )
    if not cargo_home.is_absolute():
        _fail("CARGO_HOME must be absolute when it is inherited")
    candidates: set[Path] = set()
    for directory in (invocation_root, *invocation_root.parents, cargo_home):
        cargo_directory = directory if directory == cargo_home else directory / ".cargo"
        for name in ("config", "config.toml"):
            path = cargo_directory / name
            if path.exists() or path.is_symlink():
                if path.is_symlink():
                    _fail("Cargo configuration cannot be a symlink")
                candidates.add(path.resolve(strict=True))
    if candidates:
        _fail("closed Cargo invocation unexpectedly discovered a configuration file")
    return []


def _prepare_authenticated_build_corridor_v2(
    source_root: Path,
    source: SourceEvidenceV1,
    target: str,
    base_environment: Mapping[str, str],
    cargo_cache_roots: Mapping[str, object],
) -> AuthenticatedBuildCorridorV2:
    """Resolve and bind every ambient input retained by the build process."""

    # Toolchain identity probes have no reason to inspect the source tree.  A
    # fixed root cwd also prevents configuration discovery from depending on a
    # replaceable pathname spelling of the signed snapshot.
    del source_root
    probe_root = Path(os.sep)
    base = _canonical_build_environment(dict(base_environment))
    cargo_home_text = base.get("CARGO_HOME")
    if not cargo_home_text:
        _fail("authenticated build requires one explicit fresh CARGO_HOME")
    cargo_home = Path(cargo_home_text)
    try:
        cargo_home_metadata = cargo_home.lstat()
    except OSError as error:
        raise ZkX509WorkerPackageError("closed Cargo home is unavailable") from error
    if (
        not cargo_home.is_absolute()
        or cargo_home != cargo_home.resolve(strict=True)
        or not stat.S_ISDIR(cargo_home_metadata.st_mode)
        or stat.S_ISLNK(cargo_home_metadata.st_mode)
        or cargo_home_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(cargo_home_metadata.st_mode) != 0o700
    ):
        _fail("authenticated build CARGO_HOME is not one closed fresh directory")
    rustc_dispatcher = _locate_build_executable(
        "rustc", base, label="rustc dispatcher"
    )
    sysroot_output = _run_build_tool(
        rustc_dispatcher,
        ("--print", "sysroot"),
        source_root=probe_root,
        environment=base,
        label="rustc sysroot",
    )
    try:
        sysroot_text = sysroot_output.decode("utf-8").strip()
        sysroot = Path(sysroot_text).resolve(strict=True)
    except (UnicodeError, OSError) as error:
        raise ZkX509WorkerPackageError("Rust sysroot is invalid") from error
    if (
        not sysroot_text
        or not Path(sysroot_text).is_absolute()
        or os.fspath(sysroot) != sysroot_text
    ):
        _fail("Rust sysroot must be one canonical absolute path")
    cargo = (sysroot / "bin" / "cargo").resolve(strict=True)
    rustc = (sysroot / "bin" / "rustc").resolve(strict=True)
    _stable_build_input_record(cargo, label="Cargo", require_executable=True)
    _stable_build_input_record(rustc, label="rustc", require_executable=True)
    effective = dict(base)
    effective["PATH"] = _path_with_build_tools_first(
        effective["PATH"], cargo.parent
    )
    effective["RUSTC"] = os.fspath(rustc)
    rustc_wrapper = _resolve_build_executable(
        "sccache", effective, label="sccache rustc wrapper"
    )
    effective["RUSTC_WRAPPER"] = os.fspath(rustc_wrapper)
    rustc_version = _run_build_tool(
        rustc,
        ("-vV",),
        source_root=probe_root,
        environment=effective,
        label="rustc version",
    )
    try:
        rustc_version_text = rustc_version.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ZkX509WorkerPackageError("rustc version is not UTF-8") from error
    host_matches = re.findall(
        r"^host: ([a-z0-9][a-z0-9._+-]{0,127})$", rustc_version_text, re.M
    )
    if len(host_matches) != 1:
        _fail("rustc host identity is invalid")
    host = host_matches[0]
    compiler_names, archiver_names = _target_compiler_names(host, target)
    linker_driver = _first_resolved_build_executable(
        compiler_names, effective, label="target linker driver"
    )
    archiver = _first_resolved_build_executable(
        archiver_names, effective, label="target archiver"
    )
    linker_output = _run_build_tool(
        linker_driver,
        ("-print-prog-name=ld",),
        source_root=probe_root,
        environment=effective,
        label="target linker resolution",
    )
    try:
        linker_name = linker_output.decode("utf-8").strip()
    except UnicodeDecodeError as error:
        raise ZkX509WorkerPackageError("target linker path is not UTF-8") from error
    if not linker_name or "\0" in linker_name or "\n" in linker_name:
        _fail("target linker path is invalid")
    if Path(linker_name).is_absolute():
        linker = Path(linker_name).resolve(strict=True)
    else:
        linker = _resolve_build_executable(
            linker_name, effective, label="target linker"
        )
    cargo_suffix = target.upper().replace("-", "_").replace(".", "_")
    cc_suffix = target.replace("-", "_").replace(".", "_")
    effective.update(
        {
            "AR": os.fspath(archiver),
            f"AR_{cc_suffix}": os.fspath(archiver),
            "CC": os.fspath(linker_driver),
            f"CC_{cc_suffix}": os.fspath(linker_driver),
            f"CARGO_TARGET_{cargo_suffix}_LINKER": os.fspath(linker_driver),
        }
    )
    effective = _canonical_build_environment(
        effective, source=source, target=target
    )
    tool_names = {
        "archiver": archiver,
        "cargo": cargo,
        "dirname": _resolve_build_executable("dirname", effective, label="dirname"),
        "env": _resolve_build_executable("env", effective, label="env"),
        "git": _resolve_build_executable("git", effective, label="Git"),
        "grep": _resolve_build_executable("grep", effective, label="grep"),
        "lscpu": _resolve_build_executable("lscpu", effective, label="lscpu"),
        "linker": linker,
        "linker_driver": linker_driver,
        "python": Path(sys.executable).resolve(strict=True),
        "rustc": rustc,
        "rustc_wrapper": rustc_wrapper,
        "shell": _resolve_build_executable("bash", effective, label="Bash"),
        "tr": _resolve_build_executable("tr", effective, label="tr"),
        "uname": _resolve_build_executable("uname", effective, label="uname"),
    }
    tools = {
        role: _stable_build_input_record(
            path,
            label=f"build tool {role}",
            require_executable=True,
        )
        for role, path in tool_names.items()
    }
    cargo_version = _run_build_tool(
        cargo,
        ("--version", "--verbose"),
        source_root=probe_root,
        environment=effective,
        label="Cargo version",
    )
    components = {
        "cargo": _rust_component_closure_record(
            sysroot,
            f"manifest-cargo-{host}",
            label="Cargo Rust component",
        ),
        "rust_std": _rust_component_closure_record(
            sysroot,
            f"manifest-rust-std-{target}",
            label="target Rust standard library component",
        ),
        "rustc": _rust_component_closure_record(
            sysroot,
            f"manifest-rustc-{host}",
            label="rustc component",
        ),
    }
    toolchain: dict[str, object] = {
        "cargo_cache_roots": dict(cargo_cache_roots),
        "cargo_configuration": _cargo_configuration_records(Path(os.sep), effective),
        "cargo_version_sha256": hashlib.sha256(cargo_version).hexdigest(),
        "components": components,
        "host": host,
        "rustc_version_sha256": hashlib.sha256(rustc_version).hexdigest(),
        "schema": _BUILD_TOOLCHAIN_SCHEMA,
        "sysroot": os.fspath(sysroot),
        "target": target,
        "tools": tools,
    }
    provenance = _build_provenance_v2(
        effective,
        toolchain,
        source=source,
        target=target,
    )
    return AuthenticatedBuildCorridorV2(
        cargo=cargo,
        environment=effective,
        provenance=provenance,
    )


def _cargo_target_directory(
    source_root: Path,
    cargo: Path,
    environment: dict[str, str],
) -> Path:
    del cargo
    target_text = environment.get("CARGO_TARGET_DIR")
    if not target_text:
        _fail("authenticated build requires one explicit external CARGO_TARGET_DIR")
    target = Path(target_text)
    if not target.is_absolute() or target != target.resolve(strict=False):
        _fail("authenticated build CARGO_TARGET_DIR must be canonical and absolute")
    try:
        target.relative_to(source_root)
    except ValueError:
        return target
    _fail("authenticated build CARGO_TARGET_DIR must be outside the source snapshot")


def _frozen_build_environment(source: SourceEvidenceV1) -> dict[str, str]:
    """Drop caller compiler/config injection before allocating fresh build roots."""

    environment: dict[str, str] = {}
    for name in _INHERITED_BUILD_ENVIRONMENT_NAMES:
        value = os.environ.get(name)
        if value is not None:
            environment[name] = value
    if not environment.get("HOME") or not environment.get("PATH"):
        _fail("frozen zk-X509 worker build requires explicit HOME and PATH")
    environment.update(_build_environment_values(source))
    return _canonical_build_environment(environment)


def _build(args: argparse.Namespace) -> Path:
    source = _collect_from_args(args)
    base_environment = _frozen_build_environment(source)
    source_root = args.source_root.resolve(strict=True)
    external_root = _require_owner_build_root(
        args.external_build_root, "external package-build root"
    )
    try:
        external_root.relative_to(source_root)
    except ValueError:
        pass
    else:
        _fail("external package-build root must be outside the source checkout")
    lane = external_root / (
        f"zk-x509-worker-signed-build-{os.getpid()}-{secrets.token_hex(12)}"
    )
    lane.mkdir(mode=0o700)
    lane.chmod(0o700)
    snapshot = _export_signed_source_snapshot(
        source_root, source.commit, lane / "signed-source"
    )
    try:
        cargo_home = _closed_cargo_home(lane / "cargo-home", base_environment)
    except BaseException:
        os.close(snapshot.descriptor)
        raise
    pass_descriptors = (snapshot.descriptor, *cargo_home.cache_descriptors)
    fresh_roots = {
        name: lane / leaf
        for name, leaf in (
            ("HOME", "home"),
            ("CARGO_TARGET_DIR", "cargo-target"),
            ("SCCACHE_DIR", "sccache"),
            ("TMPDIR", "tmp"),
        )
    }
    for name, path in fresh_roots.items():
        try:
            path.mkdir(mode=0o700)
            path.chmod(0o700)
        except OSError as error:
            for descriptor in pass_descriptors:
                os.close(descriptor)
            raise ZkX509WorkerPackageError(
                f"fresh authenticated build {name} root could not be created"
            ) from error
    def cache_records() -> dict[str, object]:
        records: dict[str, object] = {}
        descriptor_index = 0
        for role in ("registry", "git"):
            link = cargo_home.root / role
            if not link.is_symlink():
                continue
            if descriptor_index >= len(cargo_home.cache_descriptors):
                raise AssertionError("closed Cargo cache descriptor inventory drift")
            target = Path(os.readlink(link))
            records[role] = _cargo_cache_tree_record(
                cargo_home.cache_descriptors[descriptor_index], target, role
            )
            descriptor_index += 1
        if descriptor_index != len(cargo_home.cache_descriptors):
            raise AssertionError("closed Cargo cache descriptor inventory drift")
        return records

    try:
        cache_before = cache_records()
    except BaseException:
        for descriptor in pass_descriptors:
            os.close(descriptor)
        raise
    try:
        effective_base = dict(base_environment)
        effective_base["CARGO_HOME"] = os.fspath(cargo_home.root)
        effective_base.update(
            {name: os.fspath(path) for name, path in fresh_roots.items()}
        )
        descriptor_root = _descriptor_path(
            snapshot.descriptor, "signed source snapshot"
        )
        corridor = _prepare_authenticated_build_corridor_v2(
            descriptor_root,
            source,
            args.target,
            effective_base,
            cache_before,
        )
        target_directory = _cargo_target_directory(
            descriptor_root,
            corridor.cargo,
            corridor.environment,
        )
        try:
            target_directory.relative_to(source_root)
        except ValueError:
            pass
        else:
            _fail("CARGO_TARGET_DIR must be outside the authenticated source checkout")
        command = _cargo_build_command(args.target)
        actual_command = [
            os.fspath(corridor.cargo),
            *(
                os.fspath(descriptor_root / "Cargo.toml")
                if argument == _SIGNED_MANIFEST_TOKEN
                else argument
                for argument in command[1:]
            ),
        ]
        result: subprocess.CompletedProcess[bytes] | None = None
        build_error: BaseException | None = None
        with tempfile.TemporaryFile(
            mode="w+b", dir=fresh_roots["TMPDIR"]
        ) as stdout_spool, tempfile.TemporaryFile(
            mode="w+b", dir=fresh_roots["TMPDIR"]
        ) as stderr_spool:
            try:
                result = _run_bounded_process(
                    actual_command,
                    cwd=os.path.abspath(os.sep),
                    environment=corridor.environment,
                    timeout=_MAX_BUILD_SECONDS,
                    stdout_limit=_MAX_BUILD_OUTPUT_BYTES,
                    stderr_limit=_MAX_BUILD_OUTPUT_BYTES,
                    pass_fds=pass_descriptors,
                    stdout_sink=stdout_spool.fileno(),
                    stderr_sink=stderr_spool.fileno(),
                    capture_stdout=False,
                    capture_stderr=False,
                )
            except (OSError, _BoundedProcessError) as error:
                build_error = error
            for spool, fallback, stream in (
                (
                    stdout_spool,
                    b"" if result is None else result.stdout,
                    sys.stdout.buffer,
                ),
                (
                    stderr_spool,
                    b"" if result is None else result.stderr,
                    sys.stderr.buffer,
                ),
            ):
                spool.flush()
                spool.seek(0)
                emitted = False
                while True:
                    chunk = spool.read(1024 * 1024)
                    if not chunk:
                        break
                    emitted = True
                    stream.write(chunk)
                if not emitted and fallback:
                    stream.write(fallback)
                stream.flush()
        if build_error is not None:
            raise ZkX509WorkerPackageError(
                "zk-X509 worker Cargo build failed"
            ) from build_error
        assert result is not None
        if result.returncode != 0:
            _fail("zk-X509 worker Cargo build failed")
        after = _collect_from_args(args)
        if after != source:
            _fail("authenticated source changed while the zk-X509 worker was built")
        repeated_corridor = _prepare_authenticated_build_corridor_v2(
            descriptor_root,
            source,
            args.target,
            effective_base,
            cache_before,
        )
        cache_after = cache_records()
        if repeated_corridor != corridor or cache_after != cache_before:
            _fail("zk-X509 worker build inputs changed during compilation")
        _materialize_durable_cargo_cache_links(cargo_home, cache_before)
        artifact = target_directory / args.target / "release" / ARTIFACT_FILE
        if args.target == RELEASE_TARGET:
            _validate_static_aarch64_elf(artifact)
        return _create_package(
            args,
            artifact,
            source,
            artifact_build_method=AUTHENTICATED_SOURCE_BUILD_V2,
            artifact_build_command_sha256=_build_command_sha256(args.target),
            artifact_build_provenance=corridor.provenance,
            helper_source_root=descriptor_root,
        )
    finally:
        for descriptor in pass_descriptors:
            os.close(descriptor)


def _add_source_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--allowed-signers", type=Path, required=True)
    parser.add_argument("--allowed-signers-sha256", required=True)
    parser.add_argument("--revocation", type=Path, required=True)
    parser.add_argument("--revocation-sha256", required=True)
    parser.add_argument("--signer-principal", required=True)
    parser.add_argument("--signer-fingerprint", required=True)


def _add_release_trust_argument(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--trusted-package-root-sha256",
        help=(
            "out-of-band trusted digest printed by verify --print-package-root; "
            "required with --require-release-ready"
        ),
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    package = commands.add_parser("package", help="package an existing native artifact")
    _add_source_arguments(package)
    package.add_argument("--artifact", type=Path, required=True)
    package.add_argument("--target", required=True)
    package.add_argument("--output-root", type=Path, required=True)
    package.add_argument("--require-release-ready", action="store_true")
    _add_release_trust_argument(package)

    build = commands.add_parser("build", help="build from a signed read-only source snapshot")
    _add_source_arguments(build)
    build.add_argument("--target", required=True)
    build.add_argument("--output-root", type=Path, required=True)
    build.add_argument("--external-build-root", type=Path, required=True)
    build.add_argument("--require-release-ready", action="store_true")
    _add_release_trust_argument(build)

    verify = commands.add_parser("verify", help="verify one installed package")
    verify.add_argument("--package", type=Path, required=True)
    verify.add_argument("--require-release-ready", action="store_true")
    verify.add_argument("--print-package-root", action="store_true")
    _add_release_trust_argument(verify)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "package":
            source = _collect_from_args(args)
            output = _create_package(
                args,
                args.artifact,
                source,
                artifact_build_method=PREBUILT_CANDIDATE_BUILD_V1,
                artifact_build_command_sha256=None,
                artifact_build_provenance=None,
            )
            print(output)
        elif args.command == "build":
            print(_build(args))
        else:
            manifest = verify_package(
                args.package,
                require_release_ready=args.require_release_ready,
                trusted_package_root_sha256=args.trusted_package_root_sha256,
            )
            if args.print_package_root:
                print(authenticated_package_root_sha256(manifest))
            else:
                print(_canonical_json_bytes(manifest).decode("ascii"), end="")
    except ZkX509WorkerPackageError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
