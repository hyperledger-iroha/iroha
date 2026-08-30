#!/usr/bin/env python3
"""Capture owner-private native zk-X509 *candidate* evidence.

This controller is intentionally incapable of installing fixtures, changing
source pins, committing, signing, uploading, or publishing.  It consumes one
clean SSH-signed source checkout and one immutable authenticated-source worker
package, builds two release-runner images in fresh provenance-keyed directories
outside the checkout, captures four create-new fixture candidates, and validates
them with both a separately built runner and an independent Python resource
certificate implementation.

Security boundary: the envelope measures local tools and the authenticated EC2
IID, but it is not remote attestation.  A privileged host, compromised kernel,
compiler, dynamic loader, Python/OpenSSL runtime, firmware, or hardware can
still falsify observations.  The output therefore remains candidate-only until
manual review and a later clean, separately SSH-signed pin commit.

The capture corridor is Linux-only. Every external command runs below a
trusted init in fresh user and PID namespaces, and capture fails before target
exec when that OS boundary cannot be established. There is no process-group or
descendant-enumeration fallback. The generic target is not claimed to remain
non-dumpable across Linux exec; that property applies to the trusted init.
"""

from __future__ import annotations

import argparse
import contextlib
import ctypes
from dataclasses import dataclass
import errno
import fcntl
import hashlib
import hmac
import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import selectors
import shutil
import signal
import stat
import struct
import subprocess
import sys
import tempfile
import time
import types
from typing import Any, Mapping, NoReturn, Sequence


TARGET = "aarch64-unknown-linux-gnu"
WORKER_NAME = "iroha_zk_x509_prover_worker"
RUNNER_NAME = "taira_privacy_release_runner"
PACKAGE_SCHEMA = "iroha.privacy.zk_x509_prover_worker_package.v5"
PACKAGE_BUILD_METHOD = "cargo-direct-frozen-signed-snapshot-v3"
PACKAGE_ROOT_DOMAIN = b"iroha.privacy.zk-x509.worker-authenticated-package-root.v1"
CANDIDATE_KEY_DOMAIN = b"iroha.privacy.zk-x509.native-candidate-build-key.v1"
CANDIDATE_ROOT_DOMAIN = b"iroha.privacy.zk-x509.native-capture-candidate-root.v1"
HASH_FRAME_DOMAIN = b"iroha.zk-x509.sha256.frame.v1"
RESOURCE_CERTIFICATE_DOMAIN = b"iroha.zk-x509.native-resource-certificate.payload.v1"
RESOURCE_CERTIFICATE_FIELD_COUNT = 60
EXPECTED_SCHEMA_VERSION = 1
EXPECTED_STAGE_COUNT = 48
MAX_KAT_PROOF_BYTES = 8_212_538
MAX_FILE_BYTES = 1024 * 1024 * 1024
MAX_TOOL_BYTES = 2 * 1024 * 1024 * 1024
MAX_JSON_BYTES = 64 * 1024 * 1024
CAPTURE_ELAPSED_CEILING = 300_000
CAPTURE_RSS_CEILING = 12 * 1024 * 1024 * 1024
CAPTURE_ADDRESS_SPACE_CEILING = 32 * 1024 * 1024 * 1024
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
COMMIT_SHA1 = re.compile(r"^[0-9a-f]{40}$")
SSH_FINGERPRINT = re.compile(r"^SHA256:[A-Za-z0-9+/]{43}$")
ACCOUNT_ID = re.compile(r"^[0-9]{12}$")
IMAGE_ID = re.compile(r"^ami-[0-9a-f]{8,32}$")

EXACT12_RELATIVE = Path("fixtures/privacy/exact12_v1.tsv")
PROFILE_RELATIVE = Path("crates/iroha_core/src/privacy_engines/zk_x509/profile.rs")
READINESS_RELATIVE = Path(
    "crates/iroha_core/src/privacy_engines/zk_x509/profile/readiness_certificates.rs"
)
PACKAGE_SCRIPT_RELATIVE = Path("scripts/package_zk_x509_prover_worker.py")
SOURCE_HELPER_RELATIVE = Path("scripts/compute_workspace_source_manifest.py")
IID_VERIFIER_RELATIVE = Path("scripts/verify_ec2_instance_identity.py")
TOOLCHAIN_HASHER_RELATIVE = Path("scripts/hash_taira_rust_toolchain.py")
CONTROLLER_RELATIVE = Path("scripts/capture_zk_x509_native_candidate.py")
HOST_CHECKER_RELATIVE = Path("ci/check_taira_privacy_native_host.sh")
HOST_PROBE_RELATIVE = Path("ci/taira_privacy_native_host_probe.c")
RUNNER_RELATIVE = Path(
    "crates/iroha_test_network/src/bin/taira_privacy_release_runner.rs"
)
RUNNER_MODULE_RELATIVES = (
    Path("crates/iroha_test_network/src/bin/taira_privacy_release_runner/expectation_pins.rs"),
    Path("crates/iroha_test_network/src/bin/taira_privacy_release_runner/process_resources.rs"),
    Path("crates/iroha_test_network/src/bin/taira_privacy_release_runner/resource_certificate.rs"),
    Path("crates/iroha_test_network/src/bin/taira_privacy_release_runner/proof_artifact_shape_tests.rs"),
)
SIGNED_CONTROLLER_FILES = (
    CONTROLLER_RELATIVE,
    IID_VERIFIER_RELATIVE,
    PACKAGE_SCRIPT_RELATIVE,
    SOURCE_HELPER_RELATIVE,
    TOOLCHAIN_HASHER_RELATIVE,
    HOST_CHECKER_RELATIVE,
    HOST_PROBE_RELATIVE,
    RUNNER_RELATIVE,
    *RUNNER_MODULE_RELATIVES,
    Path("crates/iroha_test_network/Cargo.toml"),
    PROFILE_RELATIVE,
    READINESS_RELATIVE,
    Path("Cargo.lock"),
)
FIXTURE_NAMES = (
    "native_release_expectations_v1.norito",
    "native_release_expectations_v1.json",
    "zk_x509_native_resource_v1.norito",
    "zk_x509_native_resource_v1.json",
)
PACKAGE_EXPECTED_KEYS = frozenset(
    {
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
)


EXPECTED_ENVIRONMENT: dict[str, object] = {
    "operating_system": "linux",
    "architecture": "aarch64",
    "endianness": "little",
    "kernel_minimum_major": 6,
    "kernel_minimum_minor": 3,
    "rustc_release": "1.93.1",
    "rustc_host": "aarch64-unknown-linux-gnu",
    "rustc_commit_hash": "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf",
    "rustc_commit_date": "2026-02-11",
    "instance_type": "c7g.4xlarge",
    "cpu_model": "Neoverse-V1",
    "logical_cpu_count": 16,
    "online_cpu_count": 16,
    "affinity_cpu_count": 16,
}
EXPECTED_PROCESS_LIMITS: dict[str, object] = {
    "elapsed_ceiling_millis": CAPTURE_ELAPSED_CEILING,
    "peak_rss_ceiling_bytes": CAPTURE_RSS_CEILING,
    "address_space_ceiling_bytes": CAPTURE_ADDRESS_SPACE_CEILING,
    "main_thread_stack_bytes": 8 * 1024 * 1024,
    "rayon_worker_stack_bytes": 8 * 1024 * 1024,
    "watchdog_thread_stack_bytes": 8 * 1024 * 1024,
    "rayon_worker_count": 4,
    "max_stage_tasks": 6,
    "max_stage_open_files": 4,
    "core_dump_bytes": 0,
    "landlock_abi_minimum": 3,
    "minimum_effective_memory_bytes": CAPTURE_RSS_CEILING,
    "cgroup_v2": True,
    "cpu_quota_unlimited": True,
    "landlock_restrict_self": True,
    "anchored_openat2": True,
    "memfd_exec": True,
    "memfd_seal_exec": True,
    "static_elf_only": True,
    "seccomp_tsync": True,
}
OBSERVATION_KEYS = frozenset(
    {
        "case_kind",
        "elapsed_millis",
        "peak_rss_bytes",
        "peak_address_space_bytes",
        "primary_units",
        "primary_ceiling",
        "secondary_units",
        "secondary_ceiling",
        "relation_depth",
        "relation_depth_ceiling",
    }
)
RESOURCE_KEYS = frozenset(
    {
        "schema_version",
        "protocol_id",
        "compiled_profile_digest",
        "environment",
        "expectations_norito_sha256",
        "expectations_json_sha256",
        "kat_proof_bytes",
        "kat_proof_sha256",
        "process_limits",
        "positive",
        "maximum",
        "certificate_sha256",
    }
)


class CandidateCaptureError(RuntimeError):
    """The candidate corridor cannot continue without weakening its boundary."""


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


def fail(message: str) -> NoReturn:
    raise CandidateCaptureError(message)


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


def _atomic_rename_noreplace(
    source: str,
    destination: str,
    *,
    source_dir_fd: int,
    destination_dir_fd: int,
    label: str,
) -> None:
    """Publish one directory name atomically without replacing an entry."""

    if (
        not source
        or not destination
        or "/" in source
        or "/" in destination
        or "\0" in source
        or "\0" in destination
    ):
        fail(f"{label} has an invalid publication name")
    library = ctypes.CDLL(None, use_errno=True)
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
        fail(f"{label} requires atomic no-replace rename support")
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
        os.fsencode(source),
        destination_dir_fd,
        os.fsencode(destination),
        flag,
    )
    if result == 0:
        return
    observed_errno = ctypes.get_errno()
    if observed_errno in (errno.EEXIST, errno.ENOTEMPTY):
        fail(f"{label} destination already exists")
    if observed_errno in (errno.ENOSYS, errno.ENOTSUP, errno.EINVAL):
        fail(f"{label} atomic no-replace rename is unsupported")
    raise CandidateCaptureError(
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
) -> subprocess.CompletedProcess[bytes]:
    """Drain both pipes under OS-enforced descendant containment."""

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


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def canonical_json(value: object) -> bytes:
    try:
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
    except (TypeError, ValueError, UnicodeError) as error:
        raise CandidateCaptureError("value cannot be represented as canonical JSON") from error


def reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            fail(f"JSON contains duplicate field {key!r}")
        result[key] = value
    return result


def strict_json(encoded: bytes, label: str) -> Any:
    try:
        return json.loads(
            encoded,
            object_pairs_hook=reject_duplicate_pairs,
            parse_constant=lambda token: fail(f"{label} contains {token}"),
        )
    except CandidateCaptureError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise CandidateCaptureError(f"{label} is not strict JSON") from error


@dataclass(frozen=True)
class FileRecord:
    path: str
    sha256: str
    size: int
    mode: int
    owner: int

    def json(self) -> dict[str, object]:
        return {
            "mode": self.mode,
            "owner": self.owner,
            "path": self.path,
            "sha256": self.sha256,
            "size": self.size,
        }


@dataclass(frozen=True)
class CaptureCargoHome:
    root: Path
    cache_descriptors: tuple[int, ...]
    cache_links: tuple[Path, ...]
    cache_roles: tuple[str, ...]


@dataclass(frozen=True)
class HeldExecutable:
    """One descriptor-bound executable retained across runtime measurement."""

    path: Path
    invocation: Path
    descriptor: int
    record: FileRecord
    identity: tuple[int, ...]


def canonical_path(path: Path, label: str, *, directory: bool = False) -> Path:
    if not path.is_absolute():
        fail(f"{label} must be an absolute path")
    try:
        details = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise CandidateCaptureError(f"{label} is unavailable: {path}") from error
    expected_kind = stat.S_ISDIR if directory else stat.S_ISREG
    if resolved != path or stat.S_ISLNK(details.st_mode) or not expected_kind(details.st_mode):
        kind = "directory" if directory else "regular file"
        fail(f"{label} must be one canonical non-symlink {kind}")
    return path


def descriptor_path(descriptor: int, label: str) -> Path:
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
            and (observed.st_dev == opened.st_dev or directory == Path("/dev/fd"))
        ):
            return candidate
    fail(f"{label} cannot be named through an open descriptor")


def require_no_effective_cargo_configuration(cargo_home: Path) -> None:
    """Recompute the entire config set for a Cargo invocation whose cwd is `/`."""

    candidates = (
        Path("/.cargo/config"),
        Path("/.cargo/config.toml"),
        cargo_home / "config",
        cargo_home / "config.toml",
    )
    if any(path.exists() or path.is_symlink() for path in candidates):
        fail("closed Cargo invocation discovered an unexpected configuration file")


def stable_file(
    path: Path,
    label: str,
    *,
    maximum: int = MAX_FILE_BYTES,
    allow_empty: bool = False,
    require_owner: bool = False,
    require_executable: bool = False,
) -> tuple[FileRecord, bytes]:
    path = canonical_path(path, label)
    before = path.lstat()
    if (
        before.st_nlink < 1
        or before.st_size < (0 if allow_empty else 1)
        or before.st_size > maximum
        or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or (require_owner and before.st_uid != os.geteuid())
        or (require_executable and not before.st_mode & stat.S_IXUSR)
    ):
        fail(f"{label} is not a bounded owner-controlled input")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        identity = (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_uid,
            before.st_size,
            before.st_mtime_ns,
        )
        if (
            opened.st_dev,
            opened.st_ino,
            opened.st_mode,
            opened.st_uid,
            opened.st_size,
            opened.st_mtime_ns,
        ) != identity:
            fail(f"{label} changed before it was opened")
        chunks: list[bytes] = []
        remaining = opened.st_size
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                fail(f"{label} was truncated while it was read")
            chunks.append(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            fail(f"{label} grew while it was read")
        after = os.fstat(descriptor)
        if (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_uid,
            after.st_size,
            after.st_mtime_ns,
        ) != identity:
            fail(f"{label} changed while it was read")
    finally:
        os.close(descriptor)
    payload = b"".join(chunks)
    return (
        FileRecord(
            path=str(path),
            sha256=sha256_bytes(payload),
            size=len(payload),
            mode=stat.S_IMODE(before.st_mode),
            owner=before.st_uid,
        ),
        payload,
    )


def _held_file_identity(details: os.stat_result) -> tuple[int, ...]:
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


def _descriptor_sha256(descriptor: int, size: int, label: str) -> str:
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(descriptor, min(1024 * 1024, size - offset), offset)
        if not chunk:
            fail(f"{label} was truncated while descriptor-bound")
        digest.update(chunk)
        offset += len(chunk)
    if os.pread(descriptor, 1, size):
        fail(f"{label} grew while descriptor-bound")
    return digest.hexdigest()


def _revalidate_held_executable(executable: HeldExecutable, label: str) -> None:
    try:
        held = os.fstat(executable.descriptor)
        named = executable.path.lstat()
    except OSError as error:
        raise CandidateCaptureError(f"{label} became unavailable") from error
    if (
        _held_file_identity(held) != executable.identity
        or _held_file_identity(named) != executable.identity
        or not hmac.compare_digest(
            _descriptor_sha256(executable.descriptor, held.st_size, label),
            executable.record.sha256,
        )
    ):
        fail(f"{label} changed during descriptor-bound use")


@contextlib.contextmanager
def hold_executable(
    path: Path,
    label: str,
    *,
    maximum: int,
    expected_sha256: str | None = None,
):
    """Hold, hash, and execute one exact input inode through its descriptor."""

    path = canonical_path(path, label)
    before = path.lstat()
    if (
        before.st_nlink < 1
        or before.st_uid not in (0, os.geteuid())
        or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or not before.st_mode & stat.S_IXUSR
        or not 1 <= before.st_size <= maximum
    ):
        fail(f"{label} is not one bounded controlled executable")
    descriptor = os.open(
        path,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        opened = os.fstat(descriptor)
        identity = _held_file_identity(opened)
        if identity != _held_file_identity(before):
            fail(f"{label} changed while opened")
        observed_sha256 = _descriptor_sha256(descriptor, opened.st_size, label)
        if expected_sha256 is not None and (
            not HEX_SHA256.fullmatch(expected_sha256)
            or not hmac.compare_digest(observed_sha256, expected_sha256)
        ):
            fail(f"{label} differs from its OOB digest")
        executable = HeldExecutable(
            path=path,
            invocation=descriptor_path(descriptor, label),
            descriptor=descriptor,
            record=FileRecord(
                path=os.fspath(path),
                sha256=observed_sha256,
                size=opened.st_size,
                mode=stat.S_IMODE(opened.st_mode),
                owner=opened.st_uid,
            ),
            identity=identity,
        )
        _revalidate_held_executable(executable, label)
        yield executable
        _revalidate_held_executable(executable, label)
    finally:
        os.close(descriptor)


def require_owner_directory(path: Path, label: str, *, exact_mode: int = 0o700) -> Path:
    path = canonical_path(path, label, directory=True)
    details = path.lstat()
    if details.st_uid != os.geteuid() or stat.S_IMODE(details.st_mode) != exact_mode:
        fail(f"{label} must be owned by the process user with mode {exact_mode:04o}")
    return path


def require_outside(path: Path, source_root: Path, label: str) -> None:
    try:
        path.relative_to(source_root)
    except ValueError:
        return
    fail(f"{label} must be outside the authenticated source checkout")


def write_create_new(path: Path, payload: bytes, mode: int = 0o600) -> None:
    if path.exists() or path.is_symlink():
        fail(f"candidate output must be create-new: {path}")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, mode)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fchmod(stream.fileno(), mode)
            os.fsync(stream.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def framed_digest(domain: bytes, values: Sequence[bytes]) -> str:
    digest = hashlib.sha256()
    digest.update(domain)
    digest.update(len(values).to_bytes(4, "big"))
    for value in values:
        digest.update(len(value).to_bytes(8, "big"))
        digest.update(value)
    return digest.hexdigest()


def closed_environment(extra: Mapping[str, str] | None = None) -> dict[str, str]:
    result = {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"}
    if extra:
        result.update(extra)
    return result


def run_checked(
    arguments: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    label: str,
    timeout: int,
    maximum_output: int = 16 * 1024 * 1024,
    pass_fds: Sequence[int] = (),
) -> tuple[dict[str, object], bytes, bytes]:
    if not arguments or any(not isinstance(item, str) or not item for item in arguments):
        fail(f"{label} command is not canonical")
    try:
        completed = _run_bounded_process(
            arguments,
            cwd=cwd,
            environment=environment,
            timeout=timeout,
            stdout_limit=maximum_output,
            stderr_limit=maximum_output,
            pass_fds=tuple(pass_fds),
        )
    except (OSError, _BoundedProcessError) as error:
        raise CandidateCaptureError(f"{label} could not complete") from error
    if completed.returncode != 0:
        diagnostic = completed.stderr.decode("utf-8", "replace").strip()[:1024]
        fail(f"{label} failed with code {completed.returncode}: {diagnostic}")
    record = {
        "argv": list(arguments),
        "cwd": str(cwd),
        "environment": dict(sorted(environment.items())),
        "passed_file_descriptors": list(pass_fds),
        "return_code": completed.returncode,
        "stdout_sha256": sha256_bytes(completed.stdout),
        "stderr_sha256": sha256_bytes(completed.stderr),
    }
    return record, completed.stdout, completed.stderr


def parse_commit_headers(raw_commit: bytes) -> tuple[list[tuple[bytes, bytes]], bytes]:
    separator = raw_commit.find(b"\n\n")
    if separator < 0:
        fail("raw commit object has no header/message boundary")
    raw_headers = raw_commit[:separator].split(b"\n")
    headers: list[tuple[bytes, bytes]] = []
    current_name: bytes | None = None
    current_value = bytearray()
    for line in raw_headers:
        if line.startswith(b" "):
            if current_name is None:
                fail("raw commit has an orphan continuation header")
            current_value.extend(b"\n")
            current_value.extend(line[1:])
            continue
        if current_name is not None:
            headers.append((current_name, bytes(current_value)))
        name, marker, value = line.partition(b" ")
        if not marker or not name or not re.fullmatch(rb"[a-z0-9-]+", name):
            fail("raw commit contains a malformed header")
        current_name = name
        current_value = bytearray(value)
    if current_name is not None:
        headers.append((current_name, bytes(current_value)))
    return headers, raw_commit[separator + 2 :]


def require_exact_one_ssh_signature(raw_commit: bytes) -> bytes:
    headers, _ = parse_commit_headers(raw_commit)
    signatures = [value for name, value in headers if name == b"gpgsig"]
    if len(signatures) != 1 or any(name.startswith(b"gpgsig-") for name, _ in headers):
        fail("commit must contain exactly one ordinary gpgsig header")
    signature = signatures[0]
    if (
        not signature.startswith(b"-----BEGIN SSH SIGNATURE-----\n")
        or not signature.endswith(b"\n-----END SSH SIGNATURE-----")
        or signature.count(b"-----BEGIN SSH SIGNATURE-----") != 1
        or signature.count(b"-----END SSH SIGNATURE-----") != 1
        or b"PGP SIGNATURE" in signature
    ):
        fail("the sole commit signature must be one canonical SSH signature armor")
    return signature


def git_object_sha1(kind: bytes, payload: bytes) -> str:
    return hashlib.sha1(kind + b" " + str(len(payload)).encode() + b"\0" + payload).hexdigest()


def validated_temporary_parent(label: str) -> Path:
    try:
        parent = Path(tempfile.gettempdir()).resolve(strict=True)
    except OSError as error:
        raise CandidateCaptureError(f"{label} temporary root is unavailable") from error
    for directory in (parent, *parent.parents):
        details = directory.lstat()
        if (
            not stat.S_ISDIR(details.st_mode)
            or stat.S_ISLNK(details.st_mode)
            or details.st_uid not in (0, os.geteuid())
            or (
                details.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
                and not details.st_mode & stat.S_ISVTX
            )
        ):
            fail(f"{label} temporary ancestor is not owner/root controlled")
    return parent


def run_signed_python_helper(
    python_path: Path,
    payload: bytes,
    arguments: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    label: str,
    timeout: int,
) -> tuple[dict[str, object], bytes, bytes]:
    """Run authenticated Python source from an unlinked read-only descriptor."""

    if not payload or len(payload) > 4 * 1024 * 1024:
        fail(f"{label} signed source payload is invalid")
    with tempfile.TemporaryDirectory(
        prefix="iroha-zk-x509-signed-helper-",
        dir=validated_temporary_parent(label),
    ) as temporary:
        root = Path(temporary).resolve(strict=True)
        root.chmod(0o700)
        helper = root / "helper.py"
        write_create_new(helper, payload, 0o400)
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
                or opened.st_size != len(payload)
                or os.pread(descriptor, opened.st_size, 0) != payload
            ):
                fail(f"{label} signed helper snapshot is not exact and read-only")
            os.unlink(helper)
            unlinked = os.fstat(descriptor)
            if unlinked.st_nlink != 0:
                fail(f"{label} signed helper snapshot remained path-addressable")
            invocation = descriptor_path(descriptor, f"{label} signed helper snapshot")
            command, stdout, stderr = run_checked(
                [str(python_path), "-I", "-S", str(invocation), *arguments],
                cwd=cwd,
                environment=environment,
                label=label,
                timeout=timeout,
                pass_fds=(descriptor,),
            )
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
                or os.pread(descriptor, after.st_size, 0) != payload
            ):
                fail(f"{label} signed helper snapshot changed while it executed")
            return command, stdout, stderr
        finally:
            os.close(descriptor)


def authenticate_source_commit(
    source_root: Path,
    *,
    expected_commit: str,
    allowed_signers: Path,
    allowed_signers_sha256: str,
    revocation: Path,
    revocation_sha256: str,
    expected_principal: str,
    expected_fingerprint: str,
    git_path: Path,
    ssh_keygen_path: Path,
    python_path: Path,
) -> tuple[dict[str, object], dict[str, object]]:
    if not COMMIT_SHA1.fullmatch(expected_commit):
        fail("expected source commit must be one nonzero SHA-1 object ID")
    if expected_commit == "0" * 40:
        fail("expected source commit must not be zero")
    if not expected_principal or "\n" in expected_principal or "\0" in expected_principal:
        fail("expected SSH signer principal is invalid")
    if not SSH_FINGERPRINT.fullmatch(expected_fingerprint):
        fail("expected SSH signer fingerprint is not canonical")
    for digest, label in (
        (allowed_signers_sha256, "allowed-signers SHA-256"),
        (revocation_sha256, "revocation-policy SHA-256"),
    ):
        if not HEX_SHA256.fullmatch(digest):
            fail(f"{label} is not canonical")
    allowed_record, allowed_payload = stable_file(
        allowed_signers,
        "allowed-signers policy",
        maximum=16 * 1024 * 1024,
    )
    revocation_record, revocation_payload = stable_file(
        revocation,
        "SSH revocation policy",
        maximum=16 * 1024 * 1024,
        allow_empty=True,
    )
    if allowed_record.sha256 != allowed_signers_sha256:
        fail("allowed-signers policy does not match its OOB digest")
    if revocation_record.sha256 != revocation_sha256:
        fail("SSH revocation policy does not match its OOB digest")
    git_record, _ = stable_file(
        git_path, "Git executable", maximum=MAX_TOOL_BYTES, require_executable=True
    )
    ssh_record, _ = stable_file(
        ssh_keygen_path,
        "ssh-keygen executable",
        maximum=MAX_TOOL_BYTES,
        require_executable=True,
    )
    python_record, _ = stable_file(
        python_path,
        "controller Python executable",
        maximum=MAX_TOOL_BYTES,
        require_executable=True,
    )
    git_environment = closed_environment(
        {
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_NOSYSTEM": "1",
            "HOME": "/nonexistent",
        }
    )
    head_command, head_stdout, _ = run_checked(
        [str(git_path), "-C", str(source_root), "rev-parse", "--verify", "HEAD^{commit}"],
        cwd=source_root,
        environment=git_environment,
        label="source HEAD resolution",
        timeout=30,
    )
    head = head_stdout.decode("ascii", "strict").strip()
    if head != expected_commit:
        fail("source HEAD does not equal the package source commit")
    raw_command, raw_commit, _ = run_checked(
        [str(git_path), "-C", str(source_root), "cat-file", "commit", expected_commit],
        cwd=source_root,
        environment=git_environment,
        label="raw commit read",
        timeout=30,
    )
    signature = require_exact_one_ssh_signature(raw_commit)
    if git_object_sha1(b"commit", raw_commit) != expected_commit:
        fail("raw commit bytes do not hash to the expected commit object ID")
    with tempfile.TemporaryDirectory(
        prefix="iroha-zk-x509-ssh-policy-",
        dir=validated_temporary_parent("SSH policy"),
    ) as temporary:
        policy_root = Path(temporary).resolve(strict=True)
        policy_root.chmod(0o700)
        policy_metadata = policy_root.lstat()
        if (
            policy_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(policy_metadata.st_mode) != 0o700
        ):
            fail("private SSH policy directory is not owner-private")
        private_allowed = policy_root / "allowed-signers"
        private_revocation = policy_root / "revocation"
        write_create_new(private_allowed, allowed_payload, 0o400)
        write_create_new(private_revocation, revocation_payload, 0o400)
        verify_arguments = [
            str(git_path),
            "-c",
            "gpg.format=ssh",
            "-c",
            f"gpg.ssh.allowedSignersFile={private_allowed}",
            "-c",
            f"gpg.ssh.revocationFile={private_revocation}",
            "-c",
            f"gpg.ssh.program={ssh_keygen_path}",
            "-C",
            str(source_root),
            "verify-commit",
            "--raw",
            expected_commit,
        ]
        verify_command, verify_stdout, verify_stderr = run_checked(
            verify_arguments,
            cwd=source_root,
            environment=git_environment,
            label="SSH commit authentication",
            timeout=30,
        )
        repeated_allowed, repeated_allowed_payload = stable_file(
            private_allowed,
            "private allowed-signers policy",
            maximum=16 * 1024 * 1024,
            require_owner=True,
        )
        repeated_revocation, repeated_revocation_payload = stable_file(
            private_revocation,
            "private SSH revocation policy",
            maximum=16 * 1024 * 1024,
            allow_empty=True,
            require_owner=True,
        )
        if (
            repeated_allowed_payload != allowed_payload
            or repeated_revocation_payload != revocation_payload
            or repeated_allowed.mode != 0o400
            or repeated_revocation.mode != 0o400
        ):
            fail("private SSH policy changed during source authentication")
    report = (verify_stdout + b"\n" + verify_stderr).decode("utf-8", "replace")
    matches = re.findall(
        r'^Good "git" signature for (.+) with ([A-Za-z0-9_-]+) key (SHA256:[A-Za-z0-9+/]{43})$',
        report,
        re.MULTILINE,
    )
    if len(matches) != 1:
        fail("Git did not report exactly one good SSH commit signature")
    observed_principal, key_type, observed_fingerprint = matches[0]
    if observed_principal != expected_principal or observed_fingerprint != expected_fingerprint:
        fail("SSH commit signer principal or fingerprint differs from the OOB pins")

    blob_records: list[dict[str, object]] = []
    source_helper_payload: bytes | None = None
    for relative in SIGNED_CONTROLLER_FILES:
        if relative.is_absolute() or any(part in ("", ".", "..") for part in relative.parts):
            raise AssertionError("signed controller path inventory is not canonical")
        checkout_record, checkout_bytes = stable_file(
            source_root / relative,
            f"signed controller file {relative.as_posix()}",
            maximum=MAX_FILE_BYTES,
        )
        _, committed_bytes, _ = run_checked(
            [
                str(git_path),
                "-C",
                str(source_root),
                "cat-file",
                "blob",
                f"{expected_commit}:{relative.as_posix()}",
            ],
            cwd=source_root,
            environment=git_environment,
            label=f"committed blob read for {relative.as_posix()}",
            timeout=30,
            maximum_output=MAX_FILE_BYTES,
        )
        if committed_bytes != checkout_bytes:
            fail(f"checked-out controller file differs from signed blob: {relative.as_posix()}")
        if relative == SOURCE_HELPER_RELATIVE:
            source_helper_payload = committed_bytes
        blob_records.append(
            {
                "path": relative.as_posix(),
                "sha256": checkout_record.sha256,
                "size": checkout_record.size,
            }
        )

    if source_helper_payload is None:
        raise AssertionError("signed source helper inventory drift")
    identity_command, identity_stdout, _ = run_signed_python_helper(
        python_path,
        source_helper_payload,
        [
            "--root",
            str(source_root),
            "--release-identity-json",
        ],
        cwd=Path(os.sep),
        environment=closed_environment({"PYTHONDONTWRITEBYTECODE": "1"}),
        label="clean source identity",
        timeout=600,
    )
    identity = strict_json(identity_stdout, "clean source identity")
    if not isinstance(identity, dict) or set(identity) != {
        "schema_version",
        "head_commit",
        "head_tree",
        "index_tree",
        "workspace_source_manifest_sha256",
        "cargo_lock_sha256",
    }:
        fail("clean source identity fields are not exact")
    if identity["schema_version"] != 1 or identity["head_commit"] != expected_commit:
        fail("clean source identity does not bind the expected commit")
    if identity["head_tree"] != identity["index_tree"]:
        fail("release source index tree does not equal its signed HEAD tree")
    authentication = {
        "schema": "iroha.zk-x509.raw-ssh-source-authentication.v1",
        "commit": expected_commit,
        "raw_commit_sha256": sha256_bytes(raw_commit),
        "ssh_signature_armor_sha256": sha256_bytes(signature),
        "signature_count": 1,
        "signer_principal": observed_principal,
        "signer_fingerprint": observed_fingerprint,
        "signer_key_type": key_type,
        "allowed_signers": allowed_record.json(),
        "revocation_policy": revocation_record.json(),
        "git": git_record.json(),
        "ssh_keygen": ssh_record.json(),
        "python": python_record.json(),
        "signed_controller_blobs": blob_records,
        "commands": [head_command, raw_command, verify_command, identity_command],
    }
    return authentication, identity


def validate_static_aarch64_elf(path: Path, label: str) -> dict[str, object]:
    record, payload = stable_file(
        path,
        label,
        maximum=MAX_FILE_BYTES,
        require_executable=True,
    )
    if len(payload) < 64 or payload[:4] != b"\x7fELF":
        fail(f"{label} is not an ELF image")
    if payload[4:7] != bytes((2, 1, 1)):
        fail(f"{label} must be ELF64 little-endian version 1")
    try:
        header = struct.unpack_from("<16sHHIQQQIHHHHHH", payload, 0)
    except struct.error as error:
        raise CandidateCaptureError(f"{label} has a truncated ELF header") from error
    _, elf_type, machine, version, _, program_offset, _, _, header_size, entry_size, count, _, _, _ = header
    if elf_type not in (2, 3) or machine != 183 or version != 1 or header_size != 64:
        fail(f"{label} is not a canonical Linux AArch64 executable")
    if entry_size != 56 or count < 1 or count > 4096:
        fail(f"{label} has an invalid program-header inventory")
    table_end = program_offset + entry_size * count
    if program_offset < 64 or table_end > len(payload):
        fail(f"{label} has a truncated program-header table")
    has_load = False
    dynamic_segments = 0
    for index in range(count):
        offset = program_offset + index * entry_size
        values = struct.unpack_from("<IIQQQQQQ", payload, offset)
        segment_type, flags, file_offset, _, _, file_size, memory_size, _ = values
        if file_size > memory_size or file_offset + file_size > len(payload):
            fail(f"{label} has an out-of-range program segment")
        if segment_type == 1:
            has_load = True
        if segment_type == 3:
            fail(f"{label} contains PT_INTERP")
        if flags & 0x1 and flags & 0x2:
            fail(f"{label} contains a writable executable program segment")
        if segment_type == 0x6474E551 and flags & 0x1:
            fail(f"{label} contains an executable GNU stack")
        if segment_type == 2:
            dynamic_segments += 1
            if file_size % 16:
                fail(f"{label} has a malformed PT_DYNAMIC segment")
            saw_null = False
            for dynamic_offset in range(file_offset, file_offset + file_size, 16):
                tag, _ = struct.unpack_from("<qQ", payload, dynamic_offset)
                if tag == 0:
                    saw_null = True
                    break
                if tag == 1:
                    fail(f"{label} contains DT_NEEDED")
            if not saw_null:
                fail(f"{label} PT_DYNAMIC has no DT_NULL terminator")
    if not has_load:
        fail(f"{label} has no PT_LOAD segment")
    return {
        **record.json(),
        "elf_class": 64,
        "elf_data": "little",
        "elf_machine": "AArch64",
        "program_header_count": count,
        "pt_interp_count": 0,
        "dt_needed_count": 0,
        "writable_executable_segment_count": 0,
        "dynamic_segment_count": dynamic_segments,
    }


def package_root_sha256(manifest: object) -> str:
    payload = canonical_json(manifest)
    digest = hashlib.sha256()
    digest.update(PACKAGE_ROOT_DOMAIN)
    digest.update(len(payload).to_bytes(8, "big"))
    digest.update(payload)
    return digest.hexdigest()


def load_candidate_package(package: Path) -> tuple[dict[str, Any], dict[str, object]]:
    package = canonical_path(package, "worker package", directory=True)
    details = package.lstat()
    if details.st_uid != os.geteuid() or stat.S_IMODE(details.st_mode) != 0o500:
        fail("worker package must be owner-controlled mode 0500")
    if {entry.name for entry in package.iterdir()} != {"manifest.json", WORKER_NAME}:
        fail("worker package inventory is not exact")
    manifest_record, manifest_bytes = stable_file(
        package / "manifest.json",
        "worker package manifest",
        maximum=64 * 1024,
        require_owner=True,
    )
    if manifest_record.mode != 0o400:
        fail("worker package manifest must have mode 0400")
    manifest = strict_json(manifest_bytes, "worker package manifest")
    if not isinstance(manifest, dict) or set(manifest) != PACKAGE_EXPECTED_KEYS:
        fail("worker package manifest fields are not exact")
    if canonical_json(manifest) != manifest_bytes:
        fail("worker package manifest is not canonical JSON")
    if (
        manifest["schema"] != PACKAGE_SCHEMA
        or manifest["schema_version"] != 5
        or manifest["artifact_file"] != WORKER_NAME
        or manifest["artifact_build_method"] != PACKAGE_BUILD_METHOD
        or manifest["target"] != TARGET
        or manifest["source_commit_signature_verified"] is not True
        or manifest["source_tree_clean"] is not True
    ):
        fail("worker package is outside the authenticated Linux release-build corridor")
    for field in (
        "artifact_sha256",
        "cargo_lock_sha256",
        "source_allowed_signers_sha256",
        "source_commit_raw_sha256",
        "source_revocation_sha256",
        "source_sha256",
        "workspace_source_manifest_sha256",
        "protocol_profile_sha256",
        "soundness_certificate_sha256",
        "artifact_build_command_sha256",
        "artifact_build_environment_sha256",
        "artifact_build_toolchain_sha256",
        "isolation_package_sha256",
    ):
        if not isinstance(manifest[field], str) or not HEX_SHA256.fullmatch(manifest[field]):
            fail(f"worker package {field} is not a nonzero SHA-256")
    if manifest["source_commit"] == "0" * 40 or not COMMIT_SHA1.fullmatch(manifest["source_commit"]):
        fail("worker package source commit is invalid")
    if (
        not isinstance(manifest["source_signer_principal"], str)
        or not manifest["source_signer_principal"]
        or "\0" in manifest["source_signer_principal"]
        or "\n" in manifest["source_signer_principal"]
        or "\r" in manifest["source_signer_principal"]
        or not isinstance(manifest["source_signer_fingerprint"], str)
        or not SSH_FINGERPRINT.fullmatch(manifest["source_signer_fingerprint"])
    ):
        fail("worker package SSH signer identity is invalid")
    zero_candidate_fields = {
        "compiled_profile_sha256": None,
        "expectations_json_sha256": None,
        "expectations_norito_sha256": None,
        "kat_proof_bytes": 0,
        "kat_proof_sha256": None,
        "production_profile_ready": False,
        "release_evidence_ready": False,
        "release_evidence_sha256": None,
        "release_ready": False,
        "resource_certificate_sha256": None,
    }
    for field, expected in zero_candidate_fields.items():
        if manifest[field] is not expected and manifest[field] != expected:
            fail(f"worker package is not an unpinned candidate: {field}")
    if (
        manifest["qualified_isolation_ready"] is not True
        or manifest["isolation_contract"]
        != "iroha.zk-x509.qualified-linux-aarch64-launcher.v1"
    ):
        fail("worker package lacks the reviewed qualified isolation contract")
    provenance = manifest["artifact_build_provenance"]
    if not isinstance(provenance, dict) or set(provenance) != {
        "environment",
        "environment_sha256",
        "schema",
        "target",
        "toolchain",
        "toolchain_sha256",
    }:
        fail("worker package build provenance fields are not exact")
    if (
        provenance["schema"] != "iroha.privacy.zk-x509.worker-build-provenance.v2"
        or provenance["target"] != TARGET
        or provenance["environment_sha256"] != manifest["artifact_build_environment_sha256"]
        or provenance["toolchain_sha256"] != manifest["artifact_build_toolchain_sha256"]
    ):
        fail("worker package build provenance identity is inconsistent")
    artifact_record, artifact_bytes = stable_file(
        package / WORKER_NAME,
        "packaged worker",
        maximum=512 * 1024 * 1024,
        require_owner=True,
        require_executable=True,
    )
    if artifact_record.mode != 0o500:
        fail("packaged worker must have mode 0500")
    if (
        artifact_record.sha256 != manifest["artifact_sha256"]
        or artifact_record.size != manifest["artifact_size"]
        or package.name != artifact_record.sha256
    ):
        fail("worker package is not content-addressed by its exact artifact")
    package_record = {
        "path": str(package),
        "manifest": manifest_record.json(),
        "artifact": artifact_record.json(),
        "package_root_sha256": package_root_sha256(manifest),
        "artifact_bytes_sha256_repeated": sha256_bytes(artifact_bytes),
    }
    return manifest, package_record


def require_zero_capture_pins(source_root: Path) -> dict[str, object]:
    profile_record, profile_bytes = stable_file(
        source_root / PROFILE_RELATIVE,
        "zk-X509 profile source",
        maximum=16 * 1024 * 1024,
    )
    readiness_record, readiness_bytes = stable_file(
        source_root / READINESS_RELATIVE,
        "zk-X509 readiness source",
        maximum=16 * 1024 * 1024,
    )
    try:
        profile = profile_bytes.decode("utf-8")
        readiness = readiness_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CandidateCaptureError("zk-X509 pin sources are not UTF-8") from error
    declarations = {
        "profile": (
            "pub(crate) const ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1: u32 = 0;",
            "pub(crate) const ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1: [u8; 32] = [0; 32];",
            "pub(crate) const ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1: [u8; 32] = [0; 32];",
            "pub(crate) const ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1: [u8; 32] = [0; 32];",
        ),
        "readiness": (
            "pub(crate) const ZK_X509_RESOURCE_POSITIVE_ELAPSED_MILLIS_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_POSITIVE_PEAK_RSS_BYTES_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_POSITIVE_PEAK_ADDRESS_SPACE_BYTES_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_MAXIMUM_PEAK_RSS_BYTES_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_MAXIMUM_PEAK_ADDRESS_SPACE_BYTES_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1: [u8; 32] = [0; 32];",
        ),
    }
    for declaration in declarations["profile"]:
        if profile.count(declaration) != 1:
            fail(f"zero-pin candidate profile declaration is not exact: {declaration}")
    for declaration in declarations["readiness"]:
        if readiness.count(declaration) != 1:
            fail(f"zero-pin candidate readiness declaration is not exact: {declaration}")
    for name in FIXTURE_NAMES:
        path = source_root / "fixtures" / "privacy" / name
        if path.exists() or path.is_symlink():
            fail(f"candidate source already contains capture-owned fixture: {name}")
    return {
        "capture_owned_pin_count": 11,
        "all_capture_owned_pins_zero": True,
        "capture_owned_fixture_files_absent": True,
        "profile_source": profile_record.json(),
        "readiness_source": readiness_record.json(),
    }


def validate_source_package_binding(
    manifest: Mapping[str, Any],
    source_identity: Mapping[str, Any],
    authentication: Mapping[str, Any],
) -> None:
    bindings = {
        "source_commit": authentication["commit"],
        "source_commit_raw_sha256": authentication["raw_commit_sha256"],
        "source_allowed_signers_sha256": authentication["allowed_signers"]["sha256"],
        "source_revocation_sha256": authentication["revocation_policy"]["sha256"],
        "source_signer_fingerprint": authentication["signer_fingerprint"],
        "source_signer_principal": authentication["signer_principal"],
        "workspace_source_manifest_sha256": source_identity[
            "workspace_source_manifest_sha256"
        ],
        "cargo_lock_sha256": source_identity["cargo_lock_sha256"],
    }
    for field, expected in bindings.items():
        if manifest[field] != expected:
            fail(f"worker package {field} does not bind the independently authenticated source")
    if manifest["protocol_id"] != "iroha-zk-x509-stark-p256-v0":
        fail("worker package protocol ID is not the reviewed zk-X509 protocol")
    if manifest["protocol_version"] != 1 or manifest["public_request_schema_version"] != 1:
        fail("worker package protocol versions are outside the reviewed corridor")


def load_authenticated_packaging_module(
    source_root: Path,
    authentication: Mapping[str, Any],
):
    """Compile the already authenticated package helper from captured bytes."""

    claimed = next(
        (
            item
            for item in authentication["signed_controller_blobs"]
            if item["path"] == PACKAGE_SCRIPT_RELATIVE.as_posix()
        ),
        None,
    )
    if not isinstance(claimed, dict):
        fail("source authentication omitted the package helper blob")
    record, payload = stable_file(
        source_root / PACKAGE_SCRIPT_RELATIVE,
        "authenticated package helper module",
        maximum=4 * 1024 * 1024,
    )
    if record.sha256 != claimed.get("sha256") or record.size != claimed.get("size"):
        fail("package helper bytes differ from the authenticated signed blob")
    module_name = f"_zk_x509_authenticated_package_{record.sha256}"
    module = types.ModuleType(module_name)
    module.__file__ = str(source_root / PACKAGE_SCRIPT_RELATIVE)
    module.__package__ = ""
    sys.modules[module_name] = module
    try:
        code = compile(payload, module.__file__, "exec", dont_inherit=True)
        exec(code, module.__dict__)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


def verify_package_with_signed_helper(
    source_root: Path,
    package: Path,
    expected_root: str,
    python_path: Path,
    *,
    pass_fds: Sequence[int] = (),
) -> dict[str, object]:
    command, stdout, _ = run_checked(
        [
            str(python_path),
            "-I",
            "-S",
            str(source_root / PACKAGE_SCRIPT_RELATIVE),
            "verify",
            "--package",
            str(package),
            "--print-package-root",
        ],
        cwd=source_root,
        environment=closed_environment({"PYTHONDONTWRITEBYTECODE": "1"}),
        label="signed worker package verification",
        timeout=600,
        pass_fds=pass_fds,
    )
    try:
        observed = stdout.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise CandidateCaptureError("package verifier root output is not ASCII") from error
    if observed != expected_root:
        fail("signed package verifier root differs from the controller's independent root")
    return command


def validate_toolchain(
    manifest: Mapping[str, Any],
    *,
    source_root: Path,
    packaging_module: types.ModuleType,
) -> tuple[dict[str, str], dict[str, object]]:
    provenance = manifest["artifact_build_provenance"]
    if not isinstance(provenance, dict):
        fail("worker package build provenance is not an object")
    environment = provenance["environment"]
    toolchain = provenance["toolchain"]
    if not isinstance(environment, dict) or not all(
        isinstance(name, str) and isinstance(value, str) and value
        for name, value in environment.items()
    ):
        fail("worker package build environment is not a string map")
    if not isinstance(toolchain, dict) or set(toolchain) != {
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
        fail("worker package toolchain field inventory is not exact")
    if (
        toolchain["schema"] != "iroha.privacy.zk-x509.worker-build-toolchain.v3"
        or toolchain["host"] != TARGET
        or toolchain["target"] != TARGET
    ):
        fail("worker package toolchain is not native Linux aarch64")
    sysroot = canonical_path(Path(toolchain["sysroot"]), "Rust sysroot", directory=True)
    tools = toolchain["tools"]
    expected_roles = {
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
    }
    if not isinstance(tools, dict) or set(tools) != expected_roles:
        fail("worker package tool inventory is not exact")
    observed_tools: dict[str, object] = {}
    for role in sorted(expected_roles):
        claimed = tools[role]
        if not isinstance(claimed, dict) or set(claimed) != {
            "mode",
            "owner",
            "path",
            "sha256",
            "size",
        }:
            fail(f"worker build tool {role} record is not exact")
        record, _ = stable_file(
            Path(claimed["path"]),
            f"worker build tool {role}",
            maximum=MAX_TOOL_BYTES,
            require_executable=True,
        )
        if record.json() != claimed:
            fail(f"worker build tool {role} changed since package construction")
        observed_tools[role] = record.json()
    cargo_configuration = toolchain["cargo_configuration"]
    if cargo_configuration != []:
        fail("closed Cargo configuration inventory must be exactly empty")
    raw_cargo_home = environment.get("CARGO_HOME")
    if not raw_cargo_home:
        fail("worker package build environment omitted its closed CARGO_HOME")
    packaged_cargo_home = canonical_path(
        Path(raw_cargo_home), "packaged closed Cargo home", directory=True
    )
    require_no_effective_cargo_configuration(packaged_cargo_home)
    observed_configuration: list[dict[str, object]] = []
    claimed_cache_roots = toolchain["cargo_cache_roots"]
    if not isinstance(claimed_cache_roots, dict) or not set(claimed_cache_roots) <= {
        "git",
        "registry",
    }:
        fail("packaged Cargo cache-root inventory is invalid")
    observed_cache_roots: dict[str, object] = {}
    for role in sorted(claimed_cache_roots):
        link = packaged_cargo_home / role
        if not link.is_symlink():
            fail(f"packaged Cargo {role} cache link is unavailable")
        try:
            target = link.resolve(strict=True)
            descriptor = packaging_module._open_directory_descriptor(
                target, f"packaged Cargo {role} cache"
            )
        except (OSError, RuntimeError) as error:
            raise CandidateCaptureError(
                f"packaged Cargo {role} cache could not be opened"
            ) from error
        try:
            observed = packaging_module._cargo_cache_tree_record(
                descriptor, target, role
            )
        finally:
            os.close(descriptor)
        if observed != claimed_cache_roots[role]:
            fail(f"packaged Cargo {role} cache changed since package construction")
        observed_cache_roots[role] = observed
    rustc_path = Path(tools["rustc"]["path"])
    cargo_path = Path(tools["cargo"]["path"])
    rustc_command, rustc_output, _ = run_checked(
        [str(rustc_path), "-vV"],
        cwd=source_root,
        environment=environment,
        label="pinned rustc version probe",
        timeout=30,
    )
    cargo_command, cargo_output, _ = run_checked(
        [str(cargo_path), "--version", "--verbose"],
        cwd=source_root,
        environment=environment,
        label="pinned Cargo version probe",
        timeout=30,
    )
    if sha256_bytes(rustc_output) != toolchain["rustc_version_sha256"]:
        fail("rustc version output changed since package construction")
    if sha256_bytes(cargo_output) != toolchain["cargo_version_sha256"]:
        fail("Cargo version output changed since package construction")
    rustc_text = rustc_output.decode("utf-8", "strict")
    expected_lines = {
        "release: 1.93.1",
        "commit-hash: 01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf",
        "commit-date: 2026-02-11",
        f"host: {TARGET}",
    }
    if not expected_lines <= set(rustc_text.splitlines()):
        fail("rustc does not match the frozen X.509 resource environment")
    return dict(environment), {
        "schema": toolchain["schema"],
        "sysroot": str(sysroot),
        "host": toolchain["host"],
        "target": toolchain["target"],
        "package_toolchain_sha256": provenance["toolchain_sha256"],
        "tools": observed_tools,
        "cargo_cache_roots": observed_cache_roots,
        "cargo_configuration": observed_configuration,
        "components": toolchain["components"],
        "commands": [rustc_command, cargo_command],
    }


def resolve_executable(name: str, environment: Mapping[str, str], label: str) -> Path:
    path_value = environment.get("PATH")
    found = shutil.which(name, path=path_value)
    if found is None:
        fail(f"{label} is unavailable on the frozen PATH")
    try:
        resolved = Path(found).resolve(strict=True)
    except OSError as error:
        raise CandidateCaptureError(f"{label} cannot be resolved") from error
    stable_file(resolved, label, maximum=MAX_TOOL_BYTES, require_executable=True)
    return resolved


def hash_directory_tree(root: Path, label: str) -> dict[str, object]:
    root = canonical_path(root, label, directory=True)
    entries: list[dict[str, object]] = []
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix()
        details = path.lstat()
        if stat.S_ISDIR(details.st_mode):
            entries.append(
                {"kind": "directory", "mode": stat.S_IMODE(details.st_mode), "path": relative}
            )
        elif stat.S_ISREG(details.st_mode):
            resolved = path.resolve(strict=True)
            if resolved != path:
                fail(f"{label} contains an unexpected hard path alias: {relative}")
            record, _ = stable_file(path, f"{label} file {relative}", maximum=MAX_TOOL_BYTES)
            entries.append(
                {
                    "kind": "file",
                    "mode": record.mode,
                    "path": relative,
                    "sha256": record.sha256,
                    "size": record.size,
                }
            )
        elif stat.S_ISLNK(details.st_mode):
            target = os.readlink(path)
            try:
                resolved = path.resolve(strict=True)
                resolved.relative_to(root)
            except (OSError, ValueError) as error:
                raise CandidateCaptureError(
                    f"{label} symlink escapes or dangles: {relative}"
                ) from error
            entries.append(
                {
                    "kind": "symlink",
                    "mode": stat.S_IMODE(details.st_mode),
                    "path": relative,
                    "target": target,
                }
            )
        else:
            fail(f"{label} contains a special file: {relative}")
    return {
        "path": str(root),
        "entry_count": len(entries),
        "tree_sha256": framed_digest(
            b"iroha.zk-x509.runtime-directory-tree.v1", [canonical_json(entries)]
        ),
        "entries": entries,
    }


def openssl_runtime_closure(
    openssl_path: Path | HeldExecutable,
    ldd_path: Path | HeldExecutable,
    *,
    cwd: Path,
) -> dict[str, object]:
    if isinstance(openssl_path, Path) and isinstance(ldd_path, Path):
        with contextlib.ExitStack() as inputs:
            held_openssl = inputs.enter_context(
                hold_executable(
                    openssl_path, "OpenSSL executable", maximum=MAX_TOOL_BYTES
                )
            )
            held_ldd = inputs.enter_context(
                hold_executable(ldd_path, "ldd executable", maximum=MAX_TOOL_BYTES)
            )
            return openssl_runtime_closure(held_openssl, held_ldd, cwd=cwd)
    if not isinstance(openssl_path, HeldExecutable) or not isinstance(
        ldd_path, HeldExecutable
    ):
        fail("OpenSSL runtime closure inputs must share one descriptor-bound form")
    openssl_input = openssl_path
    ldd_input = ldd_path
    _revalidate_held_executable(openssl_input, "OpenSSL executable")
    _revalidate_held_executable(ldd_input, "ldd executable")
    inherited = (openssl_input.descriptor, ldd_input.descriptor)
    environment = closed_environment({"OPENSSL_CONF": "/dev/null"})
    version_command, version_output, _ = run_checked(
        [str(openssl_input.invocation), "version", "-a"],
        cwd=cwd,
        environment=environment,
        label="OpenSSL version probe",
        timeout=30,
        pass_fds=inherited,
    )
    _revalidate_held_executable(openssl_input, "OpenSSL executable")
    _revalidate_held_executable(ldd_input, "ldd executable")
    modules_command, modules_output, _ = run_checked(
        [str(openssl_input.invocation), "version", "-m"],
        cwd=cwd,
        environment=environment,
        label="OpenSSL modules-directory probe",
        timeout=30,
        pass_fds=inherited,
    )
    _revalidate_held_executable(openssl_input, "OpenSSL executable")
    _revalidate_held_executable(ldd_input, "ldd executable")
    ldd_command, ldd_output, _ = run_checked(
        [str(ldd_input.invocation), str(openssl_input.invocation)],
        cwd=cwd,
        environment=closed_environment(),
        label="OpenSSL dynamic dependency resolution",
        timeout=30,
        pass_fds=inherited,
    )
    _revalidate_held_executable(openssl_input, "OpenSSL executable")
    _revalidate_held_executable(ldd_input, "ldd executable")
    dependencies: list[dict[str, object]] = []
    for raw_line in ldd_output.decode("utf-8", "strict").splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if "not found" in line:
            fail("OpenSSL has an unresolved dynamic dependency")
        if line.startswith("linux-vdso."):
            dependencies.append({"name": line.split()[0], "virtual": True})
            continue
        left, marker, right = line.partition(" => ")
        if marker:
            raw_path = right.split(" (", 1)[0]
            name = left.strip()
        else:
            raw_path = line.split(" (", 1)[0]
            name = Path(raw_path).name
        path = Path(raw_path)
        if not path.is_absolute():
            fail(f"OpenSSL ldd output is not understood: {line!r}")
        try:
            resolved = path.resolve(strict=True)
        except OSError as error:
            raise CandidateCaptureError(f"OpenSSL dependency is unavailable: {path}") from error
        record, _ = stable_file(
            resolved,
            f"OpenSSL runtime dependency {name}",
            maximum=MAX_TOOL_BYTES,
        )
        dependencies.append(
            {
                "name": name,
                "reported_path": str(path),
                "resolved_file": record.json(),
                "virtual": False,
            }
        )
    if not dependencies:
        fail("OpenSSL runtime dependency closure is empty or unrecognized")
    try:
        modules_text = modules_output.decode("utf-8", "strict").strip()
    except UnicodeDecodeError as error:
        raise CandidateCaptureError("OpenSSL modules-directory output is not UTF-8") from error
    match = re.fullmatch(r'MODULESDIR: "([^"\n]+)"', modules_text)
    if match is None:
        fail("OpenSSL modules-directory output is not canonical")
    modules = hash_directory_tree(Path(match.group(1)), "OpenSSL modules directory")
    loader_cache: dict[str, object] | None = None
    cache_path = Path("/etc/ld.so.cache")
    if cache_path.exists() and not cache_path.is_symlink():
        cache_record, _ = stable_file(
            cache_path,
            "dynamic-loader cache",
            maximum=256 * 1024 * 1024,
        )
        loader_cache = cache_record.json()
    semantic = {
        "openssl": openssl_input.record.json(),
        "ldd": ldd_input.record.json(),
        "version_output_sha256": sha256_bytes(version_output),
        "dependencies": dependencies,
        "modules": modules,
        "loader_cache": loader_cache,
        "openssl_conf": "/dev/null",
    }
    for command in (version_command, modules_command, ldd_command):
        command["argv"] = [
            os.fspath(openssl_input.path)
            if argument == os.fspath(openssl_input.invocation)
            else os.fspath(ldd_input.path)
            if argument == os.fspath(ldd_input.invocation)
            else argument
            for argument in command["argv"]
        ]
    _revalidate_held_executable(openssl_input, "OpenSSL executable")
    _revalidate_held_executable(ldd_input, "ldd executable")
    return {
        **semantic,
        "runtime_closure_sha256": framed_digest(
            b"iroha.zk-x509.openssl-runtime-closure.v1", [canonical_json(semantic)]
        ),
        "commands": [version_command, modules_command, ldd_command],
        "trust_limit": (
            "This hashes the loader resolution and provider directory observed by ldd/OpenSSL; "
            "it does not attest loader behavior, transitive runtime behavior, kernel state, or hardware."
        ),
    }


def uint(value: Any, bits: int, label: str) -> int:
    if type(value) is not int or value < 0 or value >= 1 << bits:
        fail(f"{label} must be one unsigned {bits}-bit integer")
    return value


def digest_bytes(value: Any, label: str, *, nonzero: bool = True) -> bytes:
    if (
        not isinstance(value, list)
        or len(value) != 32
        or any(type(item) is not int or not 0 <= item <= 255 for item in value)
    ):
        fail(f"{label} must be one exact 32-byte array")
    encoded = bytes(value)
    if nonzero and encoded == bytes(32):
        fail(f"{label} must not be the all-zero sentinel")
    return encoded


def require_exact_mapping(
    actual: Any, expected: Mapping[str, object], label: str
) -> dict[str, Any]:
    if not isinstance(actual, dict) or set(actual) != set(expected):
        fail(f"{label} must contain the exact canonical fields")
    for name, expected_value in expected.items():
        if type(actual[name]) is not type(expected_value) or actual[name] != expected_value:
            fail(f"{label}.{name} is not the canonical value")
    return actual


def validate_expectations_json(encoded: bytes) -> dict[str, Any]:
    payload = strict_json(encoded, "captured expectations JSON")
    if not isinstance(payload, dict) or set(payload) != {
        "schema_version",
        "stage_count",
        "stages",
    }:
        fail("captured expectations JSON must have the exact top-level fields")
    if uint(payload["schema_version"], 16, "expectations schema_version") != 1:
        fail("captured expectations JSON has an unexpected schema version")
    if uint(payload["stage_count"], 16, "expectations stage_count") != EXPECTED_STAGE_COUNT:
        fail("captured expectations JSON must declare exactly 48 stages")
    stages = payload["stages"]
    if not isinstance(stages, list) or len(stages) != EXPECTED_STAGE_COUNT:
        fail("captured expectations JSON must contain exactly 48 stages")
    if any(not isinstance(stage, dict) for stage in stages):
        fail("captured expectations JSON contains a non-object stage")
    return payload


def validate_observation(
    payload: Any,
    *,
    label: str,
    case_label: str,
    shape: tuple[int, int, int, int, int, int],
) -> dict[str, int]:
    if not isinstance(payload, dict) or set(payload) != OBSERVATION_KEYS:
        fail(f"{label} must contain the exact typed observation fields")
    require_exact_mapping(
        payload["case_kind"],
        {"case": case_label, "value": None},
        f"{label}.case_kind",
    )
    values = {
        name: uint(payload[name], 64, f"{label}.{name}")
        for name in OBSERVATION_KEYS
        if name != "case_kind"
    }
    shape_names = (
        "primary_units",
        "primary_ceiling",
        "secondary_units",
        "secondary_ceiling",
        "relation_depth",
        "relation_depth_ceiling",
    )
    if tuple(values[name] for name in shape_names) != shape:
        fail(f"{label} does not equal the canonical X.509 relation shape")
    for observed, ceiling in (
        ("elapsed_millis", "elapsed_ceiling_millis"),
        ("peak_rss_bytes", "peak_rss_ceiling_bytes"),
        ("peak_address_space_bytes", "address_space_ceiling_bytes"),
    ):
        if values[observed] == 0 or values[observed] > EXPECTED_PROCESS_LIMITS[ceiling]:
            fail(f"{label}.{observed} is outside its reviewed bound")
    return values


def resource_frame_digest(fields: list[bytes]) -> bytes:
    if len(fields) != RESOURCE_CERTIFICATE_FIELD_COUNT:
        raise AssertionError("resource-certificate field count drift")
    digest = hashlib.sha256()
    digest.update(HASH_FRAME_DOMAIN)
    digest.update(len(RESOURCE_CERTIFICATE_DOMAIN).to_bytes(2, "big"))
    digest.update(RESOURCE_CERTIFICATE_DOMAIN)
    digest.update(RESOURCE_CERTIFICATE_FIELD_COUNT.to_bytes(2, "big"))
    for field in fields:
        digest.update(len(field).to_bytes(8, "big"))
        digest.update(field)
    return digest.digest()


def resource_certificate_digest(
    payload: dict[str, Any],
    *,
    compiled_profile_digest: bytes,
    expectations_norito_digest: bytes,
    expectations_json_digest: bytes,
    kat_digest: bytes,
    positive: Mapping[str, int],
    maximum: Mapping[str, int],
) -> bytes:
    environment = payload["environment"]
    limits = payload["process_limits"]
    fields = [
        uint(payload["schema_version"], 16, "schema_version").to_bytes(2, "big"),
        compiled_profile_digest,
        environment["operating_system"].encode(),
        environment["architecture"].encode(),
        environment["endianness"].encode(),
        uint(environment["kernel_minimum_major"], 16, "kernel major").to_bytes(2, "big"),
        uint(environment["kernel_minimum_minor"], 16, "kernel minor").to_bytes(2, "big"),
        environment["rustc_release"].encode(),
        environment["rustc_host"].encode(),
        environment["rustc_commit_hash"].encode(),
        environment["rustc_commit_date"].encode(),
        environment["instance_type"].encode(),
        environment["cpu_model"].encode(),
        uint(environment["logical_cpu_count"], 16, "logical CPUs").to_bytes(2, "big"),
        uint(environment["online_cpu_count"], 16, "online CPUs").to_bytes(2, "big"),
        uint(environment["affinity_cpu_count"], 16, "affinity CPUs").to_bytes(2, "big"),
        expectations_norito_digest,
        expectations_json_digest,
        uint(payload["kat_proof_bytes"], 32, "kat_proof_bytes").to_bytes(4, "big"),
        kat_digest,
    ]
    for name in (
        "elapsed_ceiling_millis",
        "peak_rss_ceiling_bytes",
        "address_space_ceiling_bytes",
        "main_thread_stack_bytes",
        "rayon_worker_stack_bytes",
        "watchdog_thread_stack_bytes",
    ):
        fields.append(uint(limits[name], 64, f"process_limits.{name}").to_bytes(8, "big"))
    for name in ("rayon_worker_count", "max_stage_tasks", "max_stage_open_files"):
        fields.append(uint(limits[name], 16, f"process_limits.{name}").to_bytes(2, "big"))
    fields.append(uint(limits["core_dump_bytes"], 64, "core_dump_bytes").to_bytes(8, "big"))
    fields.append(
        uint(limits["landlock_abi_minimum"], 16, "landlock ABI minimum").to_bytes(2, "big")
    )
    fields.append(
        uint(limits["minimum_effective_memory_bytes"], 64, "minimum memory").to_bytes(8, "big")
    )
    for name in (
        "cgroup_v2",
        "cpu_quota_unlimited",
        "landlock_restrict_self",
        "anchored_openat2",
        "memfd_exec",
        "memfd_seal_exec",
        "static_elf_only",
        "seccomp_tsync",
    ):
        if type(limits[name]) is not bool:
            fail(f"process_limits.{name} must be boolean")
        fields.append(bytes([limits[name]]))
    observation_order = (
        "elapsed_millis",
        "peak_rss_bytes",
        "peak_address_space_bytes",
        "primary_units",
        "primary_ceiling",
        "secondary_units",
        "secondary_ceiling",
        "relation_depth",
        "relation_depth_ceiling",
    )
    for case_ordinal, observation in ((0, positive), (3, maximum)):
        fields.append(bytes([case_ordinal]))
        fields.extend(observation[name].to_bytes(8, "big") for name in observation_order)
    return resource_frame_digest(fields)


def validate_resource_json(
    encoded: bytes,
    expectations_norito_sha256: str,
    expectations_json_sha256: str,
) -> dict[str, Any]:
    payload = strict_json(encoded, "captured X.509 resource JSON")
    if not isinstance(payload, dict) or set(payload) != RESOURCE_KEYS:
        fail("captured X.509 resource JSON must contain the exact typed fields")
    if uint(payload["schema_version"], 16, "schema_version") != EXPECTED_SCHEMA_VERSION:
        fail("captured X.509 resource JSON has the wrong schema version")
    require_exact_mapping(
        payload["protocol_id"],
        {"protocol": "iroha-zk-x509-stark-p256-v0", "value": None},
        "protocol_id",
    )
    require_exact_mapping(payload["environment"], EXPECTED_ENVIRONMENT, "environment")
    require_exact_mapping(payload["process_limits"], EXPECTED_PROCESS_LIMITS, "process_limits")
    compiled_digest = digest_bytes(payload["compiled_profile_digest"], "compiled_profile_digest")
    expectations_norito_digest = digest_bytes(
        payload["expectations_norito_sha256"], "expectations_norito_sha256"
    )
    expectations_json_digest = digest_bytes(
        payload["expectations_json_sha256"], "expectations_json_sha256"
    )
    if expectations_norito_digest.hex() != expectations_norito_sha256:
        fail("resource certificate binds the wrong expectations Norito")
    if expectations_json_digest.hex() != expectations_json_sha256:
        fail("resource certificate binds the wrong expectations JSON")
    if expectations_norito_digest == expectations_json_digest:
        fail("resource certificate expectation digests must differ")
    kat_proof_bytes = uint(payload["kat_proof_bytes"], 32, "kat_proof_bytes")
    if not 0 < kat_proof_bytes <= MAX_KAT_PROOF_BYTES:
        fail("kat_proof_bytes is outside the canonical X5S1 bound")
    kat_digest = digest_bytes(payload["kat_proof_sha256"], "kat_proof_sha256")
    positive = validate_observation(
        payload["positive"],
        label="positive",
        case_label="positive-canonical-end-to-end",
        shape=(2, 3, 1, 4, 0, 64),
    )
    maximum = validate_observation(
        payload["maximum"],
        label="maximum",
        case_label="maximum-shape-resource",
        shape=(3, 3, 4, 4, 64, 64),
    )
    claimed_digest = digest_bytes(payload["certificate_sha256"], "certificate_sha256")
    calculated_digest = resource_certificate_digest(
        payload,
        compiled_profile_digest=compiled_digest,
        expectations_norito_digest=expectations_norito_digest,
        expectations_json_digest=expectations_json_digest,
        kat_digest=kat_digest,
        positive=positive,
        maximum=maximum,
    )
    if claimed_digest != calculated_digest:
        fail("resource certificate payload digest does not match")
    return {
        "certificate_sha256": calculated_digest.hex(),
        "compiled_profile_sha256": compiled_digest.hex(),
        "kat_proof_bytes": kat_proof_bytes,
        "kat_proof_sha256": kat_digest.hex(),
        "positive": positive,
        "maximum": maximum,
    }


def independently_validate_fixture_candidates(fixtures: Path) -> dict[str, object]:
    records: dict[str, FileRecord] = {}
    payloads: dict[str, bytes] = {}
    maximums = {
        FIXTURE_NAMES[0]: MAX_FILE_BYTES,
        FIXTURE_NAMES[1]: MAX_FILE_BYTES,
        FIXTURE_NAMES[2]: 64 * 1024,
        FIXTURE_NAMES[3]: 64 * 1024,
    }
    for name in FIXTURE_NAMES:
        record, payload = stable_file(
            fixtures / name,
            f"captured candidate {name}",
            maximum=maximums[name],
            require_owner=True,
        )
        records[name] = record
        payloads[name] = payload
    if len({record.sha256 for record in records.values()}) != len(records):
        fail("captured fixture candidate digests must be pairwise distinct")
    validate_expectations_json(payloads[FIXTURE_NAMES[1]])
    resource = validate_resource_json(
        payloads[FIXTURE_NAMES[3]],
        records[FIXTURE_NAMES[0]].sha256,
        records[FIXTURE_NAMES[1]].sha256,
    )
    derived_pins = {
        "kat_proof_bytes": resource["kat_proof_bytes"],
        "kat_proof_sha256": resource["kat_proof_sha256"],
        "expectations_norito_sha256": records[FIXTURE_NAMES[0]].sha256,
        "expectations_json_sha256": records[FIXTURE_NAMES[1]].sha256,
        "resource_certificate_sha256": resource["certificate_sha256"],
        "positive_elapsed_millis": resource["positive"]["elapsed_millis"],
        "positive_peak_rss_bytes": resource["positive"]["peak_rss_bytes"],
        "positive_peak_address_space_bytes": resource["positive"][
            "peak_address_space_bytes"
        ],
        "maximum_elapsed_millis": resource["maximum"]["elapsed_millis"],
        "maximum_peak_rss_bytes": resource["maximum"]["peak_rss_bytes"],
        "maximum_peak_address_space_bytes": resource["maximum"][
            "peak_address_space_bytes"
        ],
    }
    if len(derived_pins) != 11:
        raise AssertionError("candidate pin inventory drift")
    return {
        "fixtures": {
            name: {**records[name].json(), "path": f"fixtures/{name}"}
            for name in FIXTURE_NAMES
        },
        "resource_certificate": resource,
        "derived_unapproved_pin_candidates": derived_pins,
        "independent_python_validation": True,
        "pin_authority": False,
    }


def seal_toolchain_tree(
    source_root: Path,
    python_path: Path,
    sysroot: Path,
    output: Path,
    *,
    pass_fds: Sequence[int] = (),
) -> tuple[dict[str, object], dict[str, Any]]:
    command, stdout, _ = run_checked(
        [
            str(python_path),
            "-I",
            "-S",
            str(source_root / TOOLCHAIN_HASHER_RELATIVE),
            "--sysroot",
            str(sysroot),
            "--manifest-out",
            str(output),
        ],
        cwd=source_root,
        environment=closed_environment({"PYTHONDONTWRITEBYTECODE": "1"}),
        label="Rust toolchain tree sealing",
        timeout=1800,
        maximum_output=1024 * 1024,
        pass_fds=pass_fds,
    )
    record, payload = stable_file(
        output,
        "Rust toolchain tree manifest",
        maximum=64 * 1024 * 1024,
        require_owner=True,
    )
    manifest = strict_json(payload, "Rust toolchain tree manifest")
    if not isinstance(manifest, dict) or set(manifest) != {
        "entry_count",
        "root_path",
        "schema",
        "total_file_bytes",
        "tree_sha256",
        "tree_identity",
    }:
        fail("Rust toolchain tree manifest fields are not exact")
    if (
        manifest["schema"] != "iroha.taira.rust-toolchain-tree.v1"
        or manifest["root_path"] != str(sysroot)
        or not isinstance(manifest["tree_sha256"], str)
        or not HEX_SHA256.fullmatch(manifest["tree_sha256"])
    ):
        fail("Rust toolchain tree manifest identity is invalid")
    if stdout.decode("ascii", "strict").strip() != manifest["tree_sha256"]:
        fail("Rust toolchain hasher stdout differs from its manifest")
    return command, {**manifest, "manifest_file": record.json()}


def make_fresh_directory(path: Path, label: str) -> Path:
    if not path.is_absolute() or path != path.resolve(strict=False):
        fail(f"{label} must be a canonical absolute path")
    if path.exists() or path.is_symlink():
        fail(f"{label} must not already exist: {path}")
    path.mkdir(mode=0o700)
    path.chmod(0o700)
    return path


def make_capture_cargo_home(
    package_environment: Mapping[str, str],
    destination: Path,
    expected_cache_roots: Mapping[str, object],
    packaging_module: types.ModuleType,
) -> CaptureCargoHome:
    """Create a fresh config-free home from the package's durable cache roots."""

    raw_home = package_environment.get("CARGO_HOME")
    if not raw_home:
        fail("packaged build provenance omitted CARGO_HOME")
    package_home = canonical_path(Path(raw_home), "packaged closed Cargo home", directory=True)
    metadata = package_home.lstat()
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) != 0o700:
        fail("packaged closed Cargo home is not owner-private mode 0700")
    root = make_fresh_directory(destination, "fresh capture Cargo home")
    descriptors: list[int] = []
    links: list[Path] = []
    try:
        if not set(expected_cache_roots) <= {"registry", "git"}:
            fail("expected Cargo cache-root inventory is invalid")
        roles = tuple(sorted(expected_cache_roots))
        for name in roles:
            source_link = package_home / name
            if not source_link.is_symlink():
                if source_link.exists():
                    fail(f"packaged Cargo {name} cache root is not a symlink")
                continue
            raw_target = os.readlink(source_link)
            target = Path(raw_target)
            if not target.is_absolute():
                fail(f"packaged Cargo {name} cache target is not absolute")
            target = canonical_path(
                target.resolve(strict=True), f"packaged Cargo {name} cache", directory=True
            )
            target_metadata = target.lstat()
            if (
                target_metadata.st_uid not in (0, os.geteuid())
                or target_metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
            ):
                fail(f"packaged Cargo {name} cache is not owner-controlled")
            descriptor = os.open(
                target,
                os.O_RDONLY
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_NOFOLLOW", 0),
            )
            descriptors.append(descriptor)
            observed = packaging_module._cargo_cache_tree_record(
                descriptor, target, name
            )
            if observed != expected_cache_roots[name]:
                fail(f"packaged Cargo {name} cache differs from its provenance seal")
            packaging_module._require_cache_path_identity(
                descriptor, target, name, observed
            )
            link = root / name
            os.symlink(str(descriptor_path(descriptor, f"Cargo {name} cache")), link)
            links.append(link)
            followed = link.stat()
            held = os.fstat(descriptor)
            if (followed.st_dev, followed.st_ino) != (held.st_dev, held.st_ino):
                fail(f"fresh Cargo {name} cache link names a different inode")
            packaging_module._require_cache_path_identity(
                descriptor, target, name, observed
            )
        require_no_effective_cargo_configuration(root)
        return CaptureCargoHome(root, tuple(descriptors), tuple(links), roles)
    except BaseException:
        for link in links:
            try:
                link.unlink()
            except FileNotFoundError:
                pass
        for descriptor in descriptors:
            os.close(descriptor)
        raise


def runner_build_command(cargo_path: Path, manifest_path: Path) -> list[str]:
    return [
        str(cargo_path),
        "rustc",
        "--manifest-path",
        str(manifest_path),
        "--frozen",
        "--profile",
        "release",
        "--package",
        "iroha_test_network",
        "--bin",
        RUNNER_NAME,
        "--features",
        "privacy-release-evidence",
        "--target",
        TARGET,
        "--",
        "-C",
        "target-feature=+crt-static",
    ]


def build_runner(
    source_descriptor: int,
    base_environment: Mapping[str, str],
    expected_cache_roots: Mapping[str, object],
    packaging_module: types.ModuleType,
    cargo_path: Path,
    lane_root: Path,
    lane_name: str,
) -> tuple[Path, dict[str, object]]:
    target = make_fresh_directory(lane_root / "cargo-target", f"{lane_name} Cargo target")
    sccache = make_fresh_directory(lane_root / "sccache", f"{lane_name} sccache directory")
    temporary = make_fresh_directory(lane_root / "tmp", f"{lane_name} temporary directory")
    cargo_home = make_capture_cargo_home(
        base_environment,
        lane_root / "cargo-home",
        expected_cache_roots,
        packaging_module,
    )
    environment = dict(base_environment)
    environment.update(
        {
            "CARGO_TARGET_DIR": str(target),
            "CARGO_HOME": str(cargo_home.root),
            "SCCACHE_DIR": str(sccache),
            "TMPDIR": str(temporary),
            "CARGO_INCREMENTAL": "0",
            "CARGO_NET_OFFLINE": "true",
            "LANG": "C",
            "LC_ALL": "C",
        }
    )
    manifest_path = descriptor_path(source_descriptor, f"{lane_name} source snapshot") / "Cargo.toml"
    try:
        command, _, _ = run_checked(
            runner_build_command(cargo_path, manifest_path),
            cwd=Path(os.sep),
            environment=environment,
            label=f"{lane_name} release runner build",
            timeout=7200,
            maximum_output=64 * 1024 * 1024,
            pass_fds=(source_descriptor, *cargo_home.cache_descriptors),
        )
        for role, descriptor in zip(
            cargo_home.cache_roles, cargo_home.cache_descriptors, strict=True
        ):
            repeated = packaging_module._cargo_cache_tree_record(
                descriptor, Path(expected_cache_roots[role]["path"]), role
            )
            if repeated != expected_cache_roots[role]:
                fail(f"Cargo {role} cache changed during {lane_name} build")
    finally:
        for link in cargo_home.cache_links:
            try:
                link.unlink()
            except FileNotFoundError:
                pass
        for descriptor in cargo_home.cache_descriptors:
            os.close(descriptor)
    runner = target / TARGET / "release" / RUNNER_NAME
    runner_record = validate_static_aarch64_elf(runner, f"{lane_name} release runner")
    return runner, {
        "lane": lane_name,
        "lane_root": str(lane_root),
        "cargo_target_dir": str(target),
        "cargo_home": str(cargo_home.root),
        "sccache_dir": str(sccache),
        "tmpdir": str(temporary),
        "command": command,
        "runner": runner_record,
    }


def copy_executable_create_new(source: Path, destination: Path, label: str) -> dict[str, object]:
    source_record, payload = stable_file(
        source,
        label,
        maximum=MAX_FILE_BYTES,
        require_executable=True,
    )
    write_create_new(destination, payload, 0o500)
    copied = validate_static_aarch64_elf(destination, f"copied {label}")
    if copied["sha256"] != source_record.sha256:
        fail(f"copied {label} changed bytes")
    return copied


def capture_with_runner(
    runner: Path,
    exact12: Path,
    environment_path: Path,
    fixtures: Path,
    run_environment: Mapping[str, str],
    source_root: Path,
    *,
    pass_fds: Sequence[int] = (),
) -> dict[str, object]:
    outputs = [fixtures / name for name in FIXTURE_NAMES]
    if any(path.exists() or path.is_symlink() for path in outputs):
        fail("capture fixture outputs must all be absent")
    arguments = [
        str(runner),
        "capture-expectations",
        "--exact12-matrix",
        str(exact12),
        "--expectations-norito-out",
        str(outputs[0]),
        "--expectations-json-out",
        str(outputs[1]),
        "--x509-resource-host-metadata",
        str(environment_path),
        "--x509-resource-norito-out",
        str(outputs[2]),
        "--x509-resource-json-out",
        str(outputs[3]),
        "--elapsed-ceiling-ms",
        str(CAPTURE_ELAPSED_CEILING),
        "--peak-rss-ceiling-bytes",
        str(CAPTURE_RSS_CEILING),
        "--address-space-ceiling-bytes",
        str(CAPTURE_ADDRESS_SPACE_CEILING),
    ]
    command, _, _ = run_checked(
        arguments,
        cwd=source_root,
        environment=run_environment,
        label="native zk-X509 expectation capture",
        timeout=2400,
        maximum_output=16 * 1024 * 1024,
        pass_fds=pass_fds,
    )
    if any(not path.is_file() or path.is_symlink() for path in outputs):
        fail("native capture did not create exactly four regular fixture candidates")
    return command


def validate_with_fresh_runner(
    runner: Path,
    exact12: Path,
    fixtures: Path,
    run_environment: Mapping[str, str],
    source_root: Path,
    *,
    pass_fds: Sequence[int] = (),
) -> dict[str, object]:
    arguments = [
        str(runner),
        "validate-captured-fixtures",
        "--exact12-matrix",
        str(exact12),
        "--expectations-norito",
        str(fixtures / FIXTURE_NAMES[0]),
        "--expectations-json",
        str(fixtures / FIXTURE_NAMES[1]),
        "--x509-resource-norito",
        str(fixtures / FIXTURE_NAMES[2]),
        "--x509-resource-json",
        str(fixtures / FIXTURE_NAMES[3]),
    ]
    command, _, _ = run_checked(
        arguments,
        cwd=source_root,
        environment=run_environment,
        label="fresh native captured-fixture validation",
        timeout=2400,
        maximum_output=16 * 1024 * 1024,
        pass_fds=pass_fds,
    )
    return command


def read_source_identity_again(
    source_root: Path,
    python_path: Path,
    *,
    helper_root: Path | None = None,
    pass_fds: Sequence[int] = (),
) -> tuple[dict[str, object], dict[str, Any]]:
    helper_checkout = source_root if helper_root is None else helper_root
    command, stdout, _ = run_checked(
        [
            str(python_path),
            "-I",
            "-S",
            str(helper_checkout / SOURCE_HELPER_RELATIVE),
            "--root",
            str(source_root),
            "--release-identity-json",
        ],
        cwd=source_root,
        environment=closed_environment({"PYTHONDONTWRITEBYTECODE": "1"}),
        label="post-capture clean source identity",
        timeout=600,
        pass_fds=pass_fds,
    )
    value = strict_json(stdout, "post-capture clean source identity")
    if not isinstance(value, dict):
        fail("post-capture source identity is not an object")
    return command, value


def _directory_identity(details: os.stat_result) -> tuple[int, ...]:
    return (
        details.st_dev,
        details.st_ino,
        details.st_mode,
        details.st_uid,
        details.st_nlink,
        details.st_mtime_ns,
        details.st_ctime_ns,
    )


def _walk_candidate_tree(
    directory_descriptor: int,
    prefix: PurePosixPath,
    *,
    seal: bool,
    inventory: list[dict[str, object]] | None,
) -> None:
    directory_before = os.fstat(directory_descriptor)
    for name in sorted(os.listdir(directory_descriptor)):
        if name in ("", ".", "..") or "/" in name or "\0" in name:
            fail("candidate evidence contains a non-canonical entry name")
        relative = prefix / name
        details = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
        if stat.S_ISLNK(details.st_mode):
            fail(f"candidate evidence contains a symlink: {relative}")
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
                if _directory_identity(os.fstat(child)) != _directory_identity(details):
                    fail(f"candidate evidence directory changed while opened: {relative}")
                _walk_candidate_tree(
                    child,
                    relative,
                    seal=seal,
                    inventory=inventory,
                )
                if seal:
                    os.fchmod(child, 0o500)
                    os.fsync(child)
            finally:
                os.close(child)
            continue
        if (
            not stat.S_ISREG(details.st_mode)
            or details.st_nlink != 1
            or details.st_uid != os.geteuid()
            or details.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
            or not 1 <= details.st_size <= MAX_FILE_BYTES
        ):
            fail(f"candidate evidence contains an uncontrolled file: {relative}")
        descriptor = os.open(
            name,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=directory_descriptor,
        )
        try:
            opened = os.fstat(descriptor)
            if _directory_identity(opened) != _directory_identity(details):
                fail(f"candidate evidence file changed while opened: {relative}")
            if seal:
                os.fchmod(
                    descriptor,
                    0o500 if name in {"capture-runner", "validation-runner"} else 0o400,
                )
                os.fsync(descriptor)
                opened = os.fstat(descriptor)
            if inventory is not None:
                digest = hashlib.sha256()
                remaining = opened.st_size
                while remaining:
                    chunk = os.read(descriptor, min(1024 * 1024, remaining))
                    if not chunk:
                        fail(f"candidate evidence file was truncated: {relative}")
                    digest.update(chunk)
                    remaining -= len(chunk)
                if os.read(descriptor, 1):
                    fail(f"candidate evidence file grew: {relative}")
                inventory.append(
                    {
                        "mode": stat.S_IMODE(opened.st_mode),
                        "owner": opened.st_uid,
                        "path": relative.as_posix(),
                        "sha256": digest.hexdigest(),
                        "size": opened.st_size,
                    }
                )
            if _directory_identity(os.fstat(descriptor)) != _directory_identity(opened):
                fail(f"candidate evidence file changed while sealed: {relative}")
        finally:
            os.close(descriptor)
    directory_after = os.fstat(directory_descriptor)
    if not seal and _directory_identity(directory_after) != _directory_identity(directory_before):
        fail("candidate evidence directory changed during inventory")


def _complete_candidate_inventory(
    directory_descriptor: int,
    prefix: PurePosixPath = PurePosixPath(),
) -> tuple[tuple[int, ...], list[dict[str, object]]]:
    """Hash every file and bind every directory inode in a sealed candidate."""

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
        fail("candidate evidence directory is not owner-controlled mode 0500")
    records: list[dict[str, object]] = []
    for name in sorted(os.listdir(directory_descriptor)):
        if name in ("", ".", "..") or "/" in name or "\0" in name:
            fail("candidate evidence contains a non-canonical entry name")
        relative = prefix / name
        before = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
        if stat.S_ISDIR(before.st_mode):
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
                if _directory_identity(opened) != _directory_identity(before):
                    fail(f"candidate evidence directory changed while opened: {relative}")
                child_root, children = _complete_candidate_inventory(child, relative)
                records.append(
                    {
                        "device": child_root[0],
                        "inode": child_root[1],
                        "kind": "directory",
                        "links": child_root[4],
                        "mode": stat.S_IMODE(child_root[2]),
                        "owner": child_root[3],
                        "path": relative.as_posix(),
                    }
                )
                records.extend(children)
            finally:
                os.close(child)
            continue
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid != os.geteuid()
            or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
            or stat.S_IMODE(before.st_mode)
            != (0o500 if name in {"capture-runner", "validation-runner"} else 0o400)
            or not 1 <= before.st_size <= MAX_FILE_BYTES
        ):
            fail(f"candidate evidence file is not exactly sealed: {relative}")
        descriptor = os.open(
            name,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=directory_descriptor,
        )
        try:
            opened = os.fstat(descriptor)
            if _directory_identity(opened) != _directory_identity(before):
                fail(f"candidate evidence file changed while opened: {relative}")
            digest = hashlib.sha256()
            remaining = opened.st_size
            while remaining:
                chunk = os.read(descriptor, min(1024 * 1024, remaining))
                if not chunk:
                    fail(f"candidate evidence file was truncated: {relative}")
                digest.update(chunk)
                remaining -= len(chunk)
            if os.read(descriptor, 1) or _directory_identity(
                os.fstat(descriptor)
            ) != _directory_identity(opened):
                fail(f"candidate evidence file changed while inventoried: {relative}")
            records.append(
                {
                    "device": opened.st_dev,
                    "inode": opened.st_ino,
                    "kind": "file",
                    "links": opened.st_nlink,
                    "mode": stat.S_IMODE(opened.st_mode),
                    "owner": opened.st_uid,
                    "path": relative.as_posix(),
                    "sha256": digest.hexdigest(),
                    "size": opened.st_size,
                }
            )
        finally:
            os.close(descriptor)
    root_after = os.fstat(directory_descriptor)
    if (
        root_after.st_dev,
        root_after.st_ino,
        root_after.st_mode,
        root_after.st_uid,
        root_after.st_nlink,
    ) != root_identity:
        fail("candidate evidence directory changed during complete inventory")
    return root_identity, records


def finalize_candidate_directory(
    staging: Path,
    staging_descriptor: int,
    output_root: Path,
    output_descriptor: int,
    payload: dict[str, object],
) -> Path:
    anchored = os.fstat(staging_descriptor)
    named = os.stat(staging.name, dir_fd=output_descriptor, follow_symlinks=False)
    if _directory_identity(anchored) != _directory_identity(named):
        fail("candidate staging directory changed before finalization")
    _walk_candidate_tree(
        staging_descriptor, PurePosixPath(), seal=True, inventory=None
    )
    inventory: list[dict[str, object]] = []
    _walk_candidate_tree(
        staging_descriptor, PurePosixPath(), seal=False, inventory=inventory
    )
    payload["evidence_inventory"] = inventory
    payload_bytes = canonical_json(payload)
    candidate_root = framed_digest(CANDIDATE_ROOT_DOMAIN, [payload_bytes])
    envelope = {
        "schema": "iroha.privacy.zk-x509.native-capture-candidate-envelope.v1",
        "candidate_only": True,
        "candidate_root_sha256": candidate_root,
        "payload": payload,
        "promotion_authorized": False,
        "signed": False,
    }
    envelope_bytes = canonical_json(envelope)
    envelope_descriptor = os.open(
        "candidate-envelope-v1.json",
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o400,
        dir_fd=staging_descriptor,
    )
    try:
        offset = 0
        while offset < len(envelope_bytes):
            offset += os.write(envelope_descriptor, envelope_bytes[offset:])
        os.fchmod(envelope_descriptor, 0o400)
        os.fsync(envelope_descriptor)
    finally:
        os.close(envelope_descriptor)
    os.fchmod(staging_descriptor, 0o500)
    os.fsync(staging_descriptor)
    expected_root, expected_inventory = _complete_candidate_inventory(
        staging_descriptor
    )
    destination = output_root / f"zk-x509-native-candidate-{candidate_root}"
    try:
        os.stat(destination.name, dir_fd=output_descriptor, follow_symlinks=False)
    except FileNotFoundError:
        pass
    else:
        fail("candidate root output already exists")
    _atomic_rename_noreplace(
        staging.name,
        destination.name,
        source_dir_fd=output_descriptor,
        destination_dir_fd=output_descriptor,
        label="candidate evidence publication",
    )
    try:
        published_descriptor = os.open(
            destination.name,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=output_descriptor,
        )
        try:
            moved = os.fstat(published_descriptor)
            if (moved.st_dev, moved.st_ino) != (anchored.st_dev, anchored.st_ino):
                fail("candidate root changed while it was finalized")
            published_root, published_inventory = _complete_candidate_inventory(
                published_descriptor
            )
            if (
                published_root != expected_root
                or published_inventory != expected_inventory
            ):
                fail("published candidate evidence inventory changed")
            named = os.stat(
                destination.name,
                dir_fd=output_descriptor,
                follow_symlinks=False,
            )
            if (named.st_dev, named.st_ino) != (anchored.st_dev, anchored.st_ino):
                fail("published candidate evidence pathname changed")
            os.fsync(output_descriptor)
            final_root, final_inventory = _complete_candidate_inventory(
                published_descriptor
            )
            final_named = os.stat(
                destination.name,
                dir_fd=output_descriptor,
                follow_symlinks=False,
            )
            if (
                final_root != expected_root
                or final_inventory != expected_inventory
                or (final_named.st_dev, final_named.st_ino)
                != (anchored.st_dev, anchored.st_ino)
            ):
                fail("candidate evidence changed before publication completed")
        finally:
            os.close(published_descriptor)
    except BaseException:
        # Restore the staging name when the exact moved inode is still under
        # our held output root so outer failure cleanup targets the right leaf.
        restored = False
        try:
            moved = os.stat(
                destination.name,
                dir_fd=output_descriptor,
                follow_symlinks=False,
            )
            if (moved.st_dev, moved.st_ino) == (anchored.st_dev, anchored.st_ino):
                _atomic_rename_noreplace(
                    destination.name,
                    staging.name,
                    source_dir_fd=output_descriptor,
                    destination_dir_fd=output_descriptor,
                    label="candidate evidence rollback",
                )
                restored = True
        except (OSError, CandidateCaptureError):
            pass
        if not restored:
            _cleanup_staging(
                staging_descriptor, output_descriptor, destination.name
            )
        raise
    return destination


def _parse_arguments(arguments: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--allowed-signers", required=True, type=Path)
    parser.add_argument("--allowed-signers-sha256", required=True)
    parser.add_argument("--revocation", required=True, type=Path)
    parser.add_argument("--revocation-sha256", required=True)
    parser.add_argument("--signer-principal", required=True)
    parser.add_argument("--signer-fingerprint", required=True)
    parser.add_argument("--region", required=True)
    parser.add_argument("--expected-account-id", required=True)
    parser.add_argument("--expected-image-id", required=True)
    parser.add_argument("--iid-certificate", required=True, type=Path)
    parser.add_argument("--iid-certificate-sha256", required=True)
    parser.add_argument("--openssl", required=True, type=Path)
    parser.add_argument("--openssl-sha256", required=True)
    parser.add_argument("--git", default="/usr/bin/git", type=Path)
    parser.add_argument("--ssh-keygen", default="/usr/bin/ssh-keygen", type=Path)
    parser.add_argument("--ldd", default="/usr/bin/ldd", type=Path)
    parser.add_argument("--readelf", default="/usr/bin/readelf", type=Path)
    parser.add_argument("--external-build-root", required=True, type=Path)
    parser.add_argument("--candidate-output-root", required=True, type=Path)
    return parser.parse_args(arguments)


def _not_nested(left: Path, right: Path, label: str) -> None:
    for candidate, root in ((left, right), (right, left)):
        try:
            candidate.relative_to(root)
        except ValueError:
            continue
        fail(f"{label} must be disjoint, not nested")


def _cleanup_staging(
    staging_descriptor: int,
    output_descriptor: int,
    staging_leaf: str,
) -> None:
    """Remove only the held staging inode without following any symlink."""

    def clear(directory_descriptor: int) -> None:
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
                    clear(child)
                finally:
                    os.close(child)
                os.rmdir(name, dir_fd=directory_descriptor)
            else:
                os.unlink(name, dir_fd=directory_descriptor)

    try:
        named = os.stat(staging_leaf, dir_fd=output_descriptor, follow_symlinks=False)
        anchored = os.fstat(staging_descriptor)
        if (
            not stat.S_ISDIR(named.st_mode)
            or (named.st_dev, named.st_ino) != (anchored.st_dev, anchored.st_ino)
        ):
            return
        clear(staging_descriptor)
        os.rmdir(staging_leaf, dir_fd=output_descriptor)
        os.fsync(output_descriptor)
    except OSError:
        # If local tampering prevents exact anchored cleanup, leave the held
        # inode private rather than broadening the deletion target.
        pass


def capture_candidate(options: argparse.Namespace) -> Path:
    source_root = canonical_path(options.source_root, "authenticated source root", directory=True)
    package = canonical_path(options.package, "worker package", directory=True)
    build_root = require_owner_directory(options.external_build_root, "external build root")
    output_root = require_owner_directory(options.candidate_output_root, "candidate output root")
    require_outside(build_root, source_root, "external build root")
    require_outside(output_root, source_root, "candidate output root")
    _not_nested(build_root, output_root, "external build and candidate output roots")
    if not ACCOUNT_ID.fullmatch(options.expected_account_id):
        fail("--expected-account-id must be an OOB-pinned 12-digit AWS account ID")
    if not IMAGE_ID.fullmatch(options.expected_image_id):
        fail("--expected-image-id must be an OOB-pinned AMI ID")
    for digest, label in (
        (options.iid_certificate_sha256, "IID certificate SHA-256"),
        (options.openssl_sha256, "OpenSSL SHA-256"),
    ):
        if not HEX_SHA256.fullmatch(digest):
            fail(f"{label} must be 64 lowercase hexadecimal characters")
    for path, label in (
        (options.allowed_signers, "allowed-signers policy"),
        (options.revocation, "revocation policy"),
        (options.iid_certificate, "regional IID certificate"),
    ):
        canonical_path(path, label)
        require_outside(path, source_root, label)

    python_path = Path(sys.executable).resolve(strict=True)
    if not python_path.is_absolute():
        fail("controller Python executable is not absolute")
    git_path = canonical_path(options.git, "Git executable")
    ssh_keygen_path = canonical_path(options.ssh_keygen, "ssh-keygen executable")
    openssl_path = canonical_path(options.openssl, "OpenSSL executable")
    ldd_path = canonical_path(options.ldd, "ldd executable")
    readelf_path = canonical_path(options.readelf, "readelf executable")

    manifest, package_record = load_candidate_package(package)
    authentication, source_identity = authenticate_source_commit(
        source_root,
        expected_commit=manifest["source_commit"],
        allowed_signers=options.allowed_signers,
        allowed_signers_sha256=options.allowed_signers_sha256,
        revocation=options.revocation,
        revocation_sha256=options.revocation_sha256,
        expected_principal=options.signer_principal,
        expected_fingerprint=options.signer_fingerprint,
        git_path=git_path,
        ssh_keygen_path=ssh_keygen_path,
        python_path=python_path,
    )
    validate_source_package_binding(manifest, source_identity, authentication)
    packaging_module = load_authenticated_packaging_module(source_root, authentication)
    base_build_environment, toolchain_before = validate_toolchain(
        manifest,
        source_root=Path(os.sep),
        packaging_module=packaging_module,
    )
    if toolchain_before["tools"]["git"]["path"] != str(git_path):
        fail("source-authentication Git differs from the packaged build-toolchain Git")
    readelf_record, _ = stable_file(
        readelf_path,
        "readelf executable",
        maximum=MAX_TOOL_BYTES,
        require_executable=True,
    )
    ldd_record, _ = stable_file(
        ldd_path,
        "ldd executable",
        maximum=MAX_TOOL_BYTES,
        require_executable=True,
    )
    openssl_record, _ = stable_file(
        openssl_path,
        "OpenSSL executable",
        maximum=MAX_TOOL_BYTES,
        require_executable=True,
    )
    if openssl_record.sha256 != options.openssl_sha256:
        fail("OpenSSL executable differs from its OOB digest")
    certificate_record, _ = stable_file(
        options.iid_certificate,
        "regional AWS RSA-2048 certificate",
        maximum=64 * 1024,
    )
    if certificate_record.sha256 != options.iid_certificate_sha256:
        fail("regional AWS RSA-2048 certificate differs from its OOB digest")

    controller_blob = next(
        item
        for item in authentication["signed_controller_blobs"]
        if item["path"] == CONTROLLER_RELATIVE.as_posix()
    )
    candidate_key = framed_digest(
        CANDIDATE_KEY_DOMAIN,
        [
            manifest["source_commit"].encode(),
            source_identity["workspace_source_manifest_sha256"].encode(),
            source_identity["cargo_lock_sha256"].encode(),
            package_record["package_root_sha256"].encode(),
            manifest["artifact_build_toolchain_sha256"].encode(),
            controller_blob["sha256"].encode(),
            options.iid_certificate_sha256.encode(),
            options.openssl_sha256.encode(),
            options.region.encode(),
            options.expected_account_id.encode(),
            options.expected_image_id.encode(),
        ],
    )
    build_lane = make_fresh_directory(
        build_root / f"zk-x509-native-candidate-build-{candidate_key}",
        "provenance-keyed candidate build lane",
    )
    capture_lane = make_fresh_directory(build_lane / "capture-build", "capture build lane")
    validation_lane = make_fresh_directory(
        build_lane / "validation-build", "validation build lane"
    )
    run_temporary = make_fresh_directory(build_lane / "runtime-tmp", "candidate runtime temp")
    host_probe_root = make_fresh_directory(
        build_lane / "host-probe", "native host probe directory"
    )
    snapshots = []
    try:
        for name in ("control-source", "capture-source", "validation-source"):
            snapshots.append(
                packaging_module._export_signed_source_snapshot(
                    source_root, manifest["source_commit"], build_lane / name
                )
            )
    except BaseException as error:
        for snapshot in snapshots:
            os.close(snapshot.descriptor)
        raise CandidateCaptureError("signed source snapshot export failed") from error
    control_snapshot, capture_snapshot, validation_snapshot = snapshots
    control_descriptor_root = packaging_module._descriptor_path(
        control_snapshot.descriptor, "candidate control source snapshot"
    )
    capture_descriptor_root = packaging_module._descriptor_path(
        capture_snapshot.descriptor, "candidate capture source snapshot"
    )
    validation_descriptor_root = packaging_module._descriptor_path(
        validation_snapshot.descriptor, "candidate validation source snapshot"
    )
    package_verify_command = verify_package_with_signed_helper(
        control_descriptor_root,
        package,
        package_record["package_root_sha256"],
        python_path,
        pass_fds=(control_snapshot.descriptor,),
    )
    zero_pins = require_zero_capture_pins(control_descriptor_root)
    exact12_record, _ = stable_file(
        control_descriptor_root / EXACT12_RELATIVE,
        "exact12 matrix from signed source snapshot",
        maximum=64 * 1024,
    )

    staging_leaf = f".zk-x509-candidate-{os.getpid()}-{secrets.token_hex(12)}"
    staging = output_root / staging_leaf
    output_descriptor = os.open(
        output_root,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        os.mkdir(staging_leaf, 0o700, dir_fd=output_descriptor)
        staging_descriptor = os.open(
            staging_leaf,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=output_descriptor,
        )
    except BaseException:
        os.close(output_descriptor)
        for snapshot in snapshots:
            os.close(snapshot.descriptor)
        raise
    runtime_inputs = contextlib.ExitStack()
    try:
        openssl_input = runtime_inputs.enter_context(
            hold_executable(
                openssl_path,
                "OpenSSL executable",
                maximum=MAX_TOOL_BYTES,
                expected_sha256=options.openssl_sha256,
            )
        )
        ldd_input = runtime_inputs.enter_context(
            hold_executable(ldd_path, "ldd executable", maximum=MAX_TOOL_BYTES)
        )
        openssl_record = openssl_input.record
        ldd_record = ldd_input.record
        provenance_directory = make_fresh_directory(staging / "provenance", "provenance directory")
        fixtures_directory = make_fresh_directory(staging / "fixtures", "fixture directory")
        binaries_directory = make_fresh_directory(staging / "binaries", "runner directory")

        toolchain_before_command, tree_before = seal_toolchain_tree(
            control_descriptor_root,
            python_path,
            Path(toolchain_before["sysroot"]),
            provenance_directory / "rust-toolchain-before-v1.json",
            pass_fds=(control_snapshot.descriptor,),
        )
        openssl_before = openssl_runtime_closure(
            openssl_input, ldd_input, cwd=Path(os.sep)
        )

        iid_document_path = provenance_directory / "ec2-iid-document.json"
        iid_signature_path = provenance_directory / "ec2-iid-rsa2048.txt"
        iid_verified_path = provenance_directory / "ec2-iid-verification-v1.json"
        iid_command, _, _ = run_checked(
            [
                str(python_path),
                "-I",
                "-S",
                str(control_descriptor_root / IID_VERIFIER_RELATIVE),
                "--region",
                options.region,
                "--certificate",
                str(options.iid_certificate),
                "--certificate-sha256",
                options.iid_certificate_sha256,
                "--openssl",
                str(openssl_path),
                "--openssl-sha256",
                options.openssl_sha256,
                "--document-out",
                str(iid_document_path),
                "--signature-out",
                str(iid_signature_path),
                "--verified-out",
                str(iid_verified_path),
            ],
            cwd=control_descriptor_root,
            environment=closed_environment({"PYTHONDONTWRITEBYTECODE": "1"}),
            label="authenticated EC2 IID capture",
            timeout=60,
            pass_fds=(control_snapshot.descriptor,),
        )
        _revalidate_held_executable(openssl_input, "OpenSSL executable")
        _revalidate_held_executable(ldd_input, "ldd executable")
        iid_record, iid_bytes = stable_file(
            iid_verified_path,
            "verified EC2 IID",
            maximum=256 * 1024,
            require_owner=True,
        )
        iid = strict_json(iid_bytes, "verified EC2 IID")
        if not isinstance(iid, dict) or iid.get("verified") is not True:
            fail("IID verifier did not emit an authenticated identity")
        document = iid.get("document")
        if not isinstance(document, dict):
            fail("authenticated IID document is not an object")
        if document.get("accountId") != options.expected_account_id:
            fail("authenticated IID accountId differs from its OOB admission pin")
        if document.get("imageId") != options.expected_image_id:
            fail("authenticated IID imageId differs from its OOB admission pin")
        if document.get("region") != options.region:
            fail("authenticated IID region differs from its OOB admission pin")
        if document.get("instanceType") != "c7g.4xlarge" or document.get("architecture") != "arm64":
            fail("authenticated IID is not the required ARM64 c7g.4xlarge")
        iid_identity = {
            field: document[field]
            for field in (
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
            )
        }

        host_metadata_path = provenance_directory / "native-host-v1.json"
        resource_environment_path = provenance_directory / "x509-resource-environment-v1.json"
        shell_path = Path(toolchain_before["tools"]["shell"]["path"])
        cc_path = Path(toolchain_before["tools"]["linker_driver"]["path"])
        host_python_path = Path(toolchain_before["tools"]["python"]["path"])
        if host_python_path != python_path:
            fail("controller Python differs from the packaged host-checker Python")
        host_command, _, _ = run_checked(
            [
                str(shell_path),
                str(control_descriptor_root / HOST_CHECKER_RELATIVE),
                "--verified-iid",
                str(iid_verified_path),
                "--cc",
                str(cc_path),
                "--readelf",
                str(readelf_path),
                "--python",
                str(host_python_path),
                "--lscpu",
                str(toolchain_before["tools"]["lscpu"]["path"]),
                "--uname",
                str(toolchain_before["tools"]["uname"]["path"]),
                "--grep",
                str(toolchain_before["tools"]["grep"]["path"]),
                "--tr",
                str(toolchain_before["tools"]["tr"]["path"]),
                "--probe-root",
                str(host_probe_root),
                "--metadata-out",
                str(host_metadata_path),
                "--x509-environment-out",
                str(resource_environment_path),
            ],
            cwd=control_descriptor_root,
            environment=base_build_environment,
            label="native host qualification",
            timeout=180,
            pass_fds=(control_snapshot.descriptor,),
        )
        host_record, host_bytes = stable_file(
            host_metadata_path,
            "native host qualification",
            maximum=256 * 1024,
            require_owner=True,
        )
        host = strict_json(host_bytes, "native host qualification")
        if not isinstance(host, dict) or any(
            host.get(field) != expected
            for field, expected in {
                "schema": "iroha.taira.privacy-native-host.v1",
                "native_execution": True,
                "containerized": False,
                "architecture": "aarch64",
                "byte_order": "little",
                "instance_type": "c7g.4xlarge",
                "cpu_model": "Neoverse-V1",
                "logical_cpu_count": 16,
                "online_cpu_count": 16,
                "affinity_cpu_count": 16,
                "authenticated_iid_region": options.region,
                "authenticated_iid_instance_id": document["instanceId"],
                "authenticated_iid_document_sha256": iid["document_sha256"],
                "authenticated_iid_certificate_file_sha256": options.iid_certificate_sha256,
                "authenticated_iid_verification_sha256": iid_record.sha256,
            }.items()
        ):
            fail("native host qualification does not bind the authenticated IID and exact host")
        environment_record, environment_bytes = stable_file(
            resource_environment_path,
            "X.509 resource environment",
            maximum=64 * 1024,
            require_owner=True,
        )
        resource_environment = strict_json(environment_bytes, "X.509 resource environment")
        require_exact_mapping(resource_environment, EXPECTED_ENVIRONMENT, "resource environment")

        worker_elf = validate_static_aarch64_elf(package / WORKER_NAME, "packaged worker")
        if (
            worker_elf["sha256"] != manifest["artifact_sha256"]
            or worker_elf["size"] != manifest["artifact_size"]
        ):
            fail("independent packaged-worker ELF differs from the authenticated package")
        cargo_path = Path(toolchain_before["tools"]["cargo"]["path"])
        capture_runner_source, capture_build = build_runner(
            capture_snapshot.descriptor,
            base_build_environment,
            toolchain_before["cargo_cache_roots"],
            packaging_module,
            cargo_path,
            capture_lane,
            "capture",
        )
        capture_runner = binaries_directory / "capture-runner"
        capture_runner_record = copy_executable_create_new(
            capture_runner_source, capture_runner, "capture release runner"
        )
        run_environment = closed_environment(
            {"TMPDIR": str(run_temporary), "RUST_BACKTRACE": "0"}
        )
        capture_command = capture_with_runner(
            capture_runner,
            capture_descriptor_root / EXACT12_RELATIVE,
            resource_environment_path,
            fixtures_directory,
            run_environment,
            capture_descriptor_root,
            pass_fds=(capture_snapshot.descriptor,),
        )

        validation_runner_source, validation_build = build_runner(
            validation_snapshot.descriptor,
            base_build_environment,
            toolchain_before["cargo_cache_roots"],
            packaging_module,
            cargo_path,
            validation_lane,
            "validation",
        )
        validation_runner = binaries_directory / "validation-runner"
        validation_runner_record = copy_executable_create_new(
            validation_runner_source, validation_runner, "validation release runner"
        )
        if capture_runner_record["sha256"] != validation_runner_record["sha256"]:
            fail("fresh capture and validation runner builds are not byte-identical")
        validation_command = validate_with_fresh_runner(
            validation_runner,
            validation_descriptor_root / EXACT12_RELATIVE,
            fixtures_directory,
            run_environment,
            validation_descriptor_root,
            pass_fds=(validation_snapshot.descriptor,),
        )
        fixture_validation = independently_validate_fixture_candidates(fixtures_directory)

        toolchain_after_command, tree_after = seal_toolchain_tree(
            control_descriptor_root,
            python_path,
            Path(toolchain_before["sysroot"]),
            provenance_directory / "rust-toolchain-after-v1.json",
            pass_fds=(control_snapshot.descriptor,),
        )
        if tree_before["tree_sha256"] != tree_after["tree_sha256"]:
            fail("Rust toolchain tree changed during candidate capture")
        openssl_after = openssl_runtime_closure(
            openssl_input, ldd_input, cwd=Path(os.sep)
        )
        if openssl_before["runtime_closure_sha256"] != openssl_after["runtime_closure_sha256"]:
            fail("OpenSSL runtime closure changed during candidate capture")
        repeated_environment, toolchain_after = validate_toolchain(
            manifest,
            source_root=Path(os.sep),
            packaging_module=packaging_module,
        )
        if repeated_environment != base_build_environment or toolchain_after != toolchain_before:
            fail("packaged build toolchain changed during candidate capture")
        source_after_command, source_identity_after = read_source_identity_again(
            source_root,
            python_path,
            helper_root=control_descriptor_root,
            pass_fds=(control_snapshot.descriptor,),
        )
        if source_identity_after != source_identity:
            fail("authenticated source identity changed during candidate capture")
        cargo_lock_after, _ = stable_file(
            source_root / "Cargo.lock",
            "post-capture Cargo.lock",
            maximum=128 * 1024 * 1024,
        )
        if cargo_lock_after.sha256 != source_identity["cargo_lock_sha256"]:
            fail("Cargo.lock changed during candidate capture")

        payload: dict[str, object] = {
            "schema": "iroha.privacy.zk-x509.native-capture-candidate-payload.v1",
            "candidate_only": True,
            "promotion_authorized": False,
            "source_pin_write_performed": False,
            "fixture_install_performed": False,
            "commit_performed": False,
            "signature_performed": False,
            "upload_or_publish_performed": False,
            "candidate_build_key": candidate_key,
            "external_build_lane": str(build_lane),
            "source_authentication": authentication,
            "source_identity_before": source_identity,
            "source_identity_after": source_identity_after,
            "source_identity_repeated_equal": True,
            "zero_pin_source_state": zero_pins,
            "exact12_matrix": exact12_record.json(),
            "worker_package": {
                **package_record,
                "manifest_claims": manifest,
                "claims_treated_as_authority": False,
                "signed_helper_verify_command": package_verify_command,
                "independent_static_elf": worker_elf,
            },
            "toolchain": {
                "package_bound": toolchain_before,
                "tree_before": tree_before,
                "tree_after": tree_after,
                "tree_repeated_equal": True,
                "before_command": toolchain_before_command,
                "after_command": toolchain_after_command,
                "cargo_lock_after": cargo_lock_after.json(),
            },
            "openssl_runtime": {
                "before": openssl_before,
                "after_runtime_closure_sha256": openssl_after["runtime_closure_sha256"],
                "repeated_equal": True,
                "ldd": ldd_record.json(),
                "openssl": openssl_record.json(),
            },
            "iid": {
                "verification_file": iid_record.json(),
                "identity": iid_identity,
                "runtime_oob_admission_pins": {
                    "accountId": options.expected_account_id,
                    "imageId": options.expected_image_id,
                    "region": options.region,
                    "regional_certificate_file_sha256": options.iid_certificate_sha256,
                    "openssl_sha256": options.openssl_sha256,
                },
                "signed_recorded_manual_review_fields": [
                    "availabilityZone",
                    "instanceId",
                    "pendingTime",
                    "privateIp",
                ],
                "command": iid_command,
            },
            "native_host": {
                "qualification_file": host_record.json(),
                "resource_environment_file": environment_record.json(),
                "qualification": host,
                "command": host_command,
                "readelf": readelf_record.json(),
            },
            "runner": {
                "capture_build": capture_build,
                "validation_build": validation_build,
                "capture_runner": capture_runner_record,
                "validation_runner": validation_runner_record,
                "byte_reproducible": True,
                "capture_command": capture_command,
                "validation_command": validation_command,
                "capture_profile": {
                    "elapsed_ceiling_millis": CAPTURE_ELAPSED_CEILING,
                    "peak_rss_ceiling_bytes": CAPTURE_RSS_CEILING,
                    "address_space_ceiling_bytes": CAPTURE_ADDRESS_SPACE_CEILING,
                    "expected_stage_count": EXPECTED_STAGE_COUNT,
                    "fresh_validation_build": True,
                },
            },
            "fixture_validation": fixture_validation,
            "trust_limits": [
                "AWS RSA-2048 IID authenticates AWS-issued metadata, not the live kernel, process, filesystem, firmware, or hardware",
                "accountId, imageId, region, regional certificate digest, and OpenSSL digest are runtime OOB admission pins",
                "availabilityZone, instanceId, pendingTime, and privateIp are signed and bound for manual review but are not OOB admission pins",
                "IID supplies no verifier nonce or proof that the signed document is fresh for the current host",
                "tool and toolchain hashes measure local TCB bytes; they do not prove those bytes executed faithfully",
                "except for explicit descriptor-bound snapshots, path-dispatched build tools assume no concurrent same-owner swap-use-restore race",
                "OpenSSL executable, loader resolution, shared libraries, loader cache, and module directory are hashed, but runtime behavior remains unattested",
                "host prerequisite probes and resource measurements can be falsified by a privileged or compromised host",
                "the clean-source helper covers tracked and non-ignored source state; ignored filesystem state is outside that identity",
                "candidate fixture values have no release authority until a separate reviewed clean SSH-signed pin commit",
            ],
            "commands": {
                "post_capture_source_identity": source_after_command,
            },
        }
        destination = finalize_candidate_directory(
            staging,
            staging_descriptor,
            output_root,
            output_descriptor,
            payload,
        )
        return destination
    except BaseException:
        _cleanup_staging(staging_descriptor, output_descriptor, staging_leaf)
        raise
    finally:
        runtime_inputs.close()
        os.close(staging_descriptor)
        os.close(output_descriptor)
        for snapshot in snapshots:
            os.close(snapshot.descriptor)


def main(arguments: Sequence[str] | None = None) -> int:
    options = _parse_arguments(sys.argv[1:] if arguments is None else arguments)
    try:
        destination = capture_candidate(options)
    except (CandidateCaptureError, OSError, subprocess.SubprocessError) as error:
        print(f"zk-X509 candidate capture failed: {error}", file=sys.stderr)
        return 1
    print(destination)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
