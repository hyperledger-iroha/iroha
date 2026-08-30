#!/usr/bin/env python3
"""Authenticate the local EC2 RSA-2048 instance identity document.

This verifier deliberately has no offline-fixture CLI.  Its production entry
point obtains both inputs from IMDSv2 and authenticates the signed content with
one operator-supplied, region-specific AWS RSA-2048 certificate whose raw file
digest is pinned out of band.  The certificate embedded in the CMS object is
never a trust root.

The resulting JSON is a narrow host-identity input for a *candidate* evidence
capture.  It is not remote attestation: EC2 instance identity authenticates
AWS-issued instance metadata, but it does not authenticate the current kernel,
userspace, process, filesystem, or capture outputs.

Production verification is Linux-only. OpenSSL runs below a trusted init in
fresh user and PID namespaces, and verification fails before OpenSSL exec if
that recursive-lifetime boundary cannot be established. The trusted init, not
the generic OpenSSL target after Linux exec, carries the non-dumpable claim.
"""

from __future__ import annotations

import argparse
import base64
import contextlib
import datetime as dt
import fcntl
import hashlib
import http.client
import ipaddress
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import sys
import tempfile
import time
from typing import Any, NoReturn


IMDS_ADDRESS = "169.254.169.254"
IMDS_TOKEN_PATH = "/latest/api/token"
IMDS_DOCUMENT_PATH = "/latest/dynamic/instance-identity/document"
IMDS_RSA2048_PATH = "/latest/dynamic/instance-identity/rsa2048"
MAX_DOCUMENT_BYTES = 64 * 1024
MAX_SIGNATURE_BYTES = 256 * 1024
MAX_CERTIFICATE_BYTES = 64 * 1024
MAX_TOOL_BYTES = 256 * 1024 * 1024
EXPECTED_INSTANCE_TYPE = "c7g.4xlarge"
EXPECTED_ARCHITECTURE = "arm64"
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
REGION = re.compile(r"^[a-z]{2}(?:-gov)?-[a-z]+-\d+$")
INSTANCE_ID = re.compile(r"^i-[0-9a-f]{8,32}$")
IMAGE_ID = re.compile(r"^ami-[0-9a-f]{8,32}$")
ACCOUNT_ID = re.compile(r"^[0-9]{12}$")
PRODUCT_CODE = re.compile(r"^[A-Za-z0-9._:+/-]{1,256}$")

REQUIRED_DOCUMENT_FIELDS = frozenset(
    {
        "accountId",
        "architecture",
        "availabilityZone",
        "imageId",
        "instanceId",
        "instanceType",
        "pendingTime",
        "privateIp",
        "region",
        "version",
    }
)
OPTIONAL_DOCUMENT_FIELDS = frozenset(
    {
        "billingProducts",
        "devpayProductCodes",
        "marketplaceProductCodes",
        "kernelId",
        "ramdiskId",
    }
)


class VerificationError(RuntimeError):
    """Fail-closed identity-verification error."""


class _BoundedProcessError(RuntimeError):
    """One child violated its streaming, OS-contained contract."""


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
def _sealed_containment_environment(environment: dict[str, str]) -> int:
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
        environment, ensure_ascii=True, sort_keys=True, separators=(",", ":")
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


def _die(message: str) -> NoReturn:
    raise VerificationError(message)


def _request_namespace_teardown_and_reap(process: subprocess.Popen[bytes]) -> None:
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


def _run_bounded_process(
    arguments: list[str],
    *,
    environment: dict[str, str],
    timeout: float,
    stdout_limit: int,
    stderr_limit: int,
    pass_fds: tuple[int, ...] = (),
) -> subprocess.CompletedProcess[bytes]:
    if (
        os.name != "posix"
        or not arguments
        or any(type(item) is not str or not item for item in arguments)
        or timeout <= 0
        or stdout_limit < 0
        or stderr_limit < 0
    ):
        raise _BoundedProcessError("bounded subprocess contract is invalid")
    if not sys.platform.startswith("linux"):
        raise _BoundedProcessError(
            "OS-enforced descendant containment requires Linux user and PID namespaces"
        )
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
    buffers = {"stdout": stdout, "stderr": stderr}
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
                env=_CONTAINMENT_BOOTSTRAP_ENVIRONMENT,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                close_fds=True,
                pass_fds=pass_fds
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
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise _BoundedProcessError("bounded subprocess timed out")
            for key, _ in selector.select(min(remaining, 0.25)):
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
                buffers[name].extend(chunk)
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
            arguments, returncode, bytes(stdout), bytes(stderr)
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
            for stream in (process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _die(f"duplicate JSON key in instance identity document: {key!r}")
        result[key] = value
    return result


def _require_plain_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value or any(ord(char) < 0x20 for char in value):
        _die(f"instance identity field {field!r} must be a non-empty plain string")
    return value


def _validate_product_codes(value: Any, field: str) -> None:
    if value is None:
        return
    if not isinstance(value, list) or len(value) > 64:
        _die(f"instance identity field {field!r} must be null or a bounded list")
    if not all(isinstance(item, str) and PRODUCT_CODE.fullmatch(item) for item in value):
        _die(f"instance identity field {field!r} contains an invalid product code")


def parse_and_validate_document(document: bytes, expected_region: str) -> dict[str, Any]:
    """Strictly decode the authenticated IID document and enforce release identity."""

    if not document or len(document) > MAX_DOCUMENT_BYTES:
        _die("instance identity document is empty or exceeds its size ceiling")
    try:
        text = document.decode("utf-8")
    except UnicodeDecodeError as error:
        raise VerificationError("instance identity document is not UTF-8") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=lambda token: _die(f"invalid JSON numeric constant: {token}"),
        )
    except (json.JSONDecodeError, RecursionError) as error:
        raise VerificationError("instance identity document is not strict JSON") from error
    if not isinstance(value, dict):
        _die("instance identity document must be a JSON object")
    keys = set(value)
    missing = REQUIRED_DOCUMENT_FIELDS - keys
    unexpected = keys - REQUIRED_DOCUMENT_FIELDS - OPTIONAL_DOCUMENT_FIELDS
    if missing:
        _die(f"instance identity document is missing fields: {sorted(missing)!r}")
    if unexpected:
        _die(f"instance identity document has unreviewed fields: {sorted(unexpected)!r}")

    account_id = _require_plain_string(value["accountId"], "accountId")
    if not ACCOUNT_ID.fullmatch(account_id):
        _die("instance identity accountId is not a 12-digit AWS account ID")
    architecture = _require_plain_string(value["architecture"], "architecture")
    if architecture != EXPECTED_ARCHITECTURE:
        _die(f"expected IID architecture {EXPECTED_ARCHITECTURE!r}, observed {architecture!r}")
    region = _require_plain_string(value["region"], "region")
    if region != expected_region:
        _die(f"expected IID region {expected_region!r}, observed {region!r}")
    availability_zone = _require_plain_string(value["availabilityZone"], "availabilityZone")
    if not availability_zone.startswith(region) or not re.fullmatch(
        re.escape(region) + r"[a-z]", availability_zone
    ):
        _die("IID availabilityZone is not a standard zone in the pinned region")
    image_id = _require_plain_string(value["imageId"], "imageId")
    if not IMAGE_ID.fullmatch(image_id):
        _die("IID imageId has an invalid syntax")
    instance_id = _require_plain_string(value["instanceId"], "instanceId")
    if not INSTANCE_ID.fullmatch(instance_id):
        _die("IID instanceId has an invalid syntax")
    instance_type = _require_plain_string(value["instanceType"], "instanceType")
    if instance_type != EXPECTED_INSTANCE_TYPE:
        _die(f"expected IID instanceType {EXPECTED_INSTANCE_TYPE!r}, observed {instance_type!r}")
    pending_time = _require_plain_string(value["pendingTime"], "pendingTime")
    try:
        parsed_time = dt.datetime.fromisoformat(pending_time.replace("Z", "+00:00"))
    except ValueError as error:
        raise VerificationError("IID pendingTime is not RFC3339") from error
    if parsed_time.tzinfo is None or parsed_time.utcoffset() is None:
        _die("IID pendingTime must include an RFC3339 UTC offset")
    private_ip = _require_plain_string(value["privateIp"], "privateIp")
    try:
        if ipaddress.ip_address(private_ip).version != 4:
            _die("IID privateIp must be IPv4")
    except ValueError as error:
        raise VerificationError("IID privateIp is invalid") from error
    _require_plain_string(value["version"], "version")

    for field in ("billingProducts", "devpayProductCodes", "marketplaceProductCodes"):
        if field in value:
            _validate_product_codes(value[field], field)
    for field in ("kernelId", "ramdiskId"):
        if field in value and value[field] is not None:
            identifier = _require_plain_string(value[field], field)
            prefix = "aki-" if field == "kernelId" else "ari-"
            if not re.fullmatch(re.escape(prefix) + r"[0-9a-f]{8,32}", identifier):
                _die(f"IID {field} has invalid syntax")
    return value


def _canonical_regular_file(path: Path, label: str, maximum: int) -> tuple[Path, bytes]:
    if not path.is_absolute():
        _die(f"{label} must be an absolute path")
    try:
        details = path.lstat()
    except OSError as error:
        raise VerificationError(f"cannot inspect {label}: {path}") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISREG(details.st_mode):
        _die(f"{label} must be a non-symlink regular file")
    canonical = path.resolve(strict=True)
    if canonical != path:
        _die(f"{label} must use its canonical physical path")
    if details.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        _die(f"{label} must not be group- or world-writable")
    if details.st_size <= 0 or details.st_size > maximum:
        _die(f"{label} is empty or exceeds its size ceiling")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (details.st_dev, details.st_ino):
            _die(f"{label} changed while it was opened")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(65536, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        if not data or len(data) > maximum:
            _die(f"{label} is empty or exceeds its size ceiling")
        after = os.fstat(descriptor)
        if (after.st_dev, after.st_ino, after.st_size) != (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
        ):
            _die(f"{label} changed while it was read")
    finally:
        os.close(descriptor)
    return canonical, data


def _validate_output_path(path: Path, label: str) -> Path:
    if not path.is_absolute():
        _die(f"{label} must be an absolute path")
    if path.exists() or path.is_symlink():
        _die(f"{label} must not already exist: {path}")
    parent = path.parent
    try:
        details = parent.lstat()
    except OSError as error:
        raise VerificationError(f"cannot inspect {label} parent: {parent}") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISDIR(details.st_mode):
        _die(f"{label} parent must be a non-symlink directory")
    if parent.resolve(strict=True) != parent:
        _die(f"{label} parent must use its canonical physical path")
    if details.st_uid != os.geteuid() or details.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        _die(f"{label} parent must be owner-controlled and not group/world writable")
    return path


def _write_create_new(path: Path, data: bytes, mode: int = 0o600) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, mode)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fchmod(stream.fileno(), mode)
            os.fsync(stream.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


@contextlib.contextmanager
def _immutable_input_snapshot(
    root: Path,
    name: str,
    payload: bytes,
    *,
    executable: bool,
):
    """Yield one private immutable input spelling and inherited descriptors."""

    mode = 0o500 if executable else 0o400
    if sys.platform.startswith("linux"):
        if not all(
            hasattr(item, name)
            for item, name in (
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
            _die("Linux lacks sealed-memory support for immutable verification inputs")
        flags = os.MFD_ALLOW_SEALING | getattr(os, "MFD_CLOEXEC", 0)
        if executable:
            flags |= getattr(os, "MFD_EXEC", 0)
        try:
            descriptor = os.memfd_create(name, flags)
        except OSError as error:
            raise VerificationError("cannot create immutable verification input") from error
        try:
            offset = 0
            while offset < len(payload):
                offset += os.write(descriptor, payload[offset:])
            os.fchmod(descriptor, mode)
            required_seals = (
                fcntl.F_SEAL_SEAL
                | fcntl.F_SEAL_SHRINK
                | fcntl.F_SEAL_GROW
                | fcntl.F_SEAL_WRITE
            )
            fcntl.fcntl(descriptor, fcntl.F_ADD_SEALS, required_seals)
            details = os.fstat(descriptor)
            if (
                not stat.S_ISREG(details.st_mode)
                or stat.S_IMODE(details.st_mode) != mode
                or details.st_size != len(payload)
                or fcntl.fcntl(descriptor, fcntl.F_GET_SEALS) & required_seals
                != required_seals
            ):
                _die("immutable verification input could not be sealed exactly")
            os.lseek(descriptor, 0, os.SEEK_SET)
            observed = bytearray()
            while len(observed) < len(payload):
                chunk = os.read(descriptor, min(1024 * 1024, len(payload) - len(observed)))
                if not chunk:
                    break
                observed.extend(chunk)
            if bytes(observed) != payload:
                _die("immutable verification input changed while it was sealed")
            descriptor_path = Path(f"/proc/self/fd/{descriptor}")
            if not descriptor_path.exists():
                _die("sealed verification descriptor filesystem is unavailable")
            yield descriptor_path, (descriptor,)
        except OSError as error:
            raise VerificationError("cannot seal immutable verification input") from error
        finally:
            os.close(descriptor)
        return

    # Pure snapshot helpers remain testable off Linux. Production verification
    # is Linux-only and always takes the sealed-memfd branch before OpenSSL runs.
    path = root / name
    _write_create_new(path, payload, mode)
    canonical, observed = _canonical_regular_file(
        path,
        f"private {name} snapshot",
        MAX_TOOL_BYTES if executable else MAX_CERTIFICATE_BYTES,
    )
    if observed != payload or stat.S_IMODE(canonical.stat().st_mode) != mode:
        _die("private verification input snapshot changed after creation")
    yield canonical, ()


def _run_openssl(
    openssl: Path,
    arguments: list[str],
    *,
    stdout_limit: int = 1024 * 1024,
    pass_fds: tuple[int, ...] = (),
    failure_label: str = "OpenSSL verification command",
) -> bytes:
    environment = {
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "OPENSSL_CONF": "/dev/null",
    }
    try:
        completed = _run_bounded_process(
            [str(openssl), *arguments],
            environment=environment,
            timeout=15,
            stdout_limit=stdout_limit,
            stderr_limit=64 * 1024,
            pass_fds=pass_fds,
        )
    except (OSError, _BoundedProcessError) as error:
        raise VerificationError(f"{failure_label} could not complete safely") from error
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", "replace").strip()[:512]
        _die(f"{failure_label} failed: {detail}")
    return completed.stdout


def verify_instance_identity(
    document: bytes,
    rsa2048_body: bytes,
    *,
    expected_region: str,
    certificate_path: Path,
    certificate_sha256: str,
    openssl_path: Path,
    openssl_sha256: str,
) -> dict[str, Any]:
    """Verify supplied bytes.  Kept importable solely for focused unit tests."""

    if not REGION.fullmatch(expected_region):
        _die("expected region has invalid syntax")
    for value, name in (
        (certificate_sha256, "certificate SHA-256"),
        (openssl_sha256, "OpenSSL SHA-256"),
    ):
        if not HEX_SHA256.fullmatch(value):
            _die(f"{name} must be 64 lowercase hexadecimal characters")
    certificate_path, certificate = _canonical_regular_file(
        certificate_path, "regional AWS RSA-2048 certificate", MAX_CERTIFICATE_BYTES
    )
    if sha256_bytes(certificate) != certificate_sha256:
        _die("regional AWS RSA-2048 certificate digest does not match the OOB pin")
    if certificate.count(b"-----BEGIN CERTIFICATE-----") != 1 or certificate.count(
        b"-----END CERTIFICATE-----"
    ) != 1:
        _die("regional AWS RSA-2048 trust file must contain exactly one PEM certificate")
    openssl_path, openssl_bytes = _canonical_regular_file(
        openssl_path, "OpenSSL executable", MAX_TOOL_BYTES
    )
    if not os.access(openssl_path, os.X_OK):
        _die("OpenSSL path is not executable")
    if sha256_bytes(openssl_bytes) != openssl_sha256:
        _die("OpenSSL executable digest does not match the OOB pin")
    if not rsa2048_body or len(rsa2048_body) > MAX_SIGNATURE_BYTES:
        _die("RSA-2048 CMS body is empty or exceeds its size ceiling")
    try:
        compact_signature = b"".join(rsa2048_body.split())
        cms_der = base64.b64decode(compact_signature, validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise VerificationError("RSA-2048 endpoint did not return strict base64") from error
    if not compact_signature:
        _die("RSA-2048 endpoint returned an empty CMS body")

    parsed_document = parse_and_validate_document(document, expected_region)
    signature_pem = (
        b"-----BEGIN PKCS7-----\n"
        + compact_signature
        + b"\n-----END PKCS7-----\n"
    )
    with tempfile.TemporaryDirectory(prefix="iroha-ec2-iid-") as temporary:
        os.chmod(temporary, 0o700)
        root = Path(temporary).resolve(strict=True)
        with contextlib.ExitStack() as immutable_inputs:
            openssl_invocation, openssl_descriptors = immutable_inputs.enter_context(
                _immutable_input_snapshot(
                    root,
                    "openssl-image",
                    openssl_bytes,
                    executable=True,
                )
            )
            certificate_snapshot, certificate_descriptors = immutable_inputs.enter_context(
                _immutable_input_snapshot(
                    root,
                    "regional-certificate.pem",
                    certificate,
                    executable=False,
                )
            )
            inherited_descriptors = openssl_descriptors + certificate_descriptors

            def openssl(arguments: list[str], **options: Any) -> bytes:
                return _run_openssl(
                    openssl_invocation,
                    arguments,
                    pass_fds=inherited_descriptors,
                    **options,
                )

            signature_file = root / "rsa2048.pem"
            recovered_file = root / "authenticated-document.json"
            _write_create_new(signature_file, signature_pem)
            cms_structure = openssl(
                ["cms", "-cmsout", "-print", "-inform", "PEM", "-in", str(signature_file)]
            )
            _, signer_marker, signer_section = cms_structure.partition(b"signerInfos:")
            signer_identities = re.findall(
                rb"(?m)^\s+d\.(?:issuerAndSerialNumber|subjectKeyIdentifier):\s*$",
                signer_section,
            )
            if (
                not signer_marker
                or len(signer_identities) != 1
                or re.search(
                    rb"digestAlgorithm:\s*\n\s*algorithm: sha256 \(",
                    signer_section,
                )
                is None
                or re.search(
                    rb"signatureAlgorithm:\s*\n\s*algorithm: "
                    rb"(?:rsaEncryption|sha256WithRSAEncryption) \(",
                    signer_section,
                )
                is None
            ):
                _die("RSA-2048 CMS must contain exactly one SHA-256/RSA signer")
            openssl(
                ["x509", "-in", str(certificate_snapshot), "-noout", "-checkend", "0"]
            )
            version = openssl(["version", "-a"])
            certificate_der = openssl(
                ["x509", "-in", str(certificate_snapshot), "-outform", "DER"]
            )
            certificate_text = openssl(
                ["x509", "-in", str(certificate_snapshot), "-noout", "-text"]
            )
            if (
                certificate_text.count(b"Public Key Algorithm: rsaEncryption") != 1
                or certificate_text.count(b"Public-Key: (2048 bit)") != 1
            ):
                _die("regional AWS IID certificate is not exactly an RSA-2048 certificate")
            public_key_pem = openssl(
                ["x509", "-in", str(certificate_snapshot), "-pubkey", "-noout"]
            )
            public_key_file = root / "certificate-public-key.pem"
            _write_create_new(public_key_file, public_key_pem)
            public_key_der = openssl(
                ["pkey", "-pubin", "-in", str(public_key_file), "-outform", "DER"]
            )
            openssl(
                [
                    "cms",
                    "-verify",
                    "-inform",
                    "PEM",
                    "-in",
                    str(signature_file),
                    "-certfile",
                    str(certificate_snapshot),
                    "-nointern",
                    "-noverify",
                    "-verify_retcode",
                    "-out",
                    str(recovered_file),
                ],
                failure_label="RSA-2048 CMS signature verification",
            )
            recovered = recovered_file.read_bytes()
            if recovered != document:
                _die("authenticated CMS content is not byte-for-byte equal to the IMDS document")

    return {
        "schema": "iroha.aws.ec2.instance-identity-verification.v1",
        "verified": True,
        "verification_method": "aws-rsa2048-cms-sha256",
        "trust_root_source": "operator-supplied-oob-regional-aws-rsa2048-certificate",
        "region_pin": expected_region,
        "document": parsed_document,
        "document_sha256": sha256_bytes(document),
        "rsa2048_body_sha256": sha256_bytes(rsa2048_body),
        "rsa2048_cms_der_sha256": sha256_bytes(cms_der),
        "rsa2048_cms_structure_output_sha256": sha256_bytes(cms_structure),
        "regional_certificate_path": str(certificate_path),
        "regional_certificate_file_sha256": certificate_sha256,
        "regional_certificate_der_sha256": sha256_bytes(certificate_der),
        "regional_certificate_spki_der_sha256": sha256_bytes(public_key_der),
        "openssl_path": str(openssl_path),
        "openssl_sha256": openssl_sha256,
        "openssl_version_output_sha256": sha256_bytes(version),
        "trust_limits": [
            "IID authenticates AWS-issued instance metadata, not the live kernel or userspace",
            "IID has no verifier nonce or current-host freshness proof; instanceId and pendingTime require independent review",
            "the regional certificate and its digest are operator-controlled out-of-band inputs",
            "the local OpenSSL executable and its digest are operator-controlled TCB inputs",
        ],
    }


def _imds_request(method: str, path: str, headers: dict[str, str], limit: int) -> bytes:
    connection = http.client.HTTPConnection(IMDS_ADDRESS, 80, timeout=3)
    try:
        connection.request(method, path, headers=headers)
        response = connection.getresponse()
        if response.status != 200:
            _die(f"IMDSv2 {method} {path} returned HTTP {response.status}")
        content_length = response.getheader("Content-Length")
        if content_length is not None:
            try:
                declared = int(content_length)
            except ValueError as error:
                raise VerificationError("IMDSv2 returned an invalid Content-Length") from error
            if declared < 0 or declared > limit:
                _die(f"IMDSv2 response for {path} exceeds its size ceiling")
        body = response.read(limit + 1)
        if not body or len(body) > limit:
            _die(f"IMDSv2 response for {path} is empty or exceeds its size ceiling")
        return body
    except (OSError, http.client.HTTPException) as error:
        raise VerificationError(f"IMDSv2 request failed for {path}") from error
    finally:
        connection.close()


def fetch_from_imdsv2() -> tuple[bytes, bytes]:
    token = _imds_request(
        "PUT",
        IMDS_TOKEN_PATH,
        {"X-aws-ec2-metadata-token-ttl-seconds": "60"},
        4096,
    )
    try:
        token_text = token.decode("ascii")
    except UnicodeDecodeError as error:
        raise VerificationError("IMDSv2 token is not ASCII") from error
    if not token_text or any(ord(char) < 0x21 or ord(char) > 0x7E for char in token_text):
        _die("IMDSv2 returned an invalid token")
    headers = {"X-aws-ec2-metadata-token": token_text}
    document = _imds_request("GET", IMDS_DOCUMENT_PATH, headers, MAX_DOCUMENT_BYTES)
    signature = _imds_request("GET", IMDS_RSA2048_PATH, headers, MAX_SIGNATURE_BYTES)
    return document, signature


def _parse_arguments(arguments: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--region", required=True, help="OOB-pinned AWS region")
    parser.add_argument("--certificate", required=True, type=Path)
    parser.add_argument("--certificate-sha256", required=True)
    parser.add_argument("--openssl", required=True, type=Path)
    parser.add_argument("--openssl-sha256", required=True)
    parser.add_argument("--document-out", required=True, type=Path)
    parser.add_argument("--signature-out", required=True, type=Path)
    parser.add_argument("--verified-out", required=True, type=Path)
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    options = _parse_arguments(sys.argv[1:] if arguments is None else arguments)
    try:
        outputs = [
            _validate_output_path(options.document_out, "--document-out"),
            _validate_output_path(options.signature_out, "--signature-out"),
            _validate_output_path(options.verified_out, "--verified-out"),
        ]
        if len(set(outputs)) != len(outputs):
            _die("IID output paths must be distinct")
        document, signature = fetch_from_imdsv2()
        verified = verify_instance_identity(
            document,
            signature,
            expected_region=options.region,
            certificate_path=options.certificate,
            certificate_sha256=options.certificate_sha256,
            openssl_path=options.openssl,
            openssl_sha256=options.openssl_sha256,
        )
        encoded = (json.dumps(verified, sort_keys=True, separators=(",", ":")) + "\n").encode()
        created: list[Path] = []
        try:
            for path, content in zip(outputs, (document, signature, encoded), strict=True):
                _write_create_new(path, content)
                created.append(path)
        except BaseException:
            for path in reversed(created):
                path.unlink(missing_ok=True)
            raise
    except (VerificationError, OSError, subprocess.SubprocessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        "authenticated EC2 IID: "
        f"{verified['document']['instanceId']} {verified['document']['instanceType']} "
        f"in {verified['document']['availabilityZone']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
