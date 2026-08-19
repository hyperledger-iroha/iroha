#!/usr/bin/env python3
"""Build the Kagemusha V4 candidate generator from one reviewed clean source closure."""

from __future__ import annotations

import sys

# The reviewed source closure rejects every ignored path and requires the tracked
# mode-100644 root Cargo.lock to match its separate V1 digest binding. Keep imports
# from creating __pycache__ entries that would otherwise make the post-build source
# identity differ from its pre-build seal.
sys.dont_write_bytecode = True

import argparse
from contextlib import contextmanager
import ctypes
from dataclasses import dataclass
import errno
import fcntl
import grp
import hashlib
import json
import os
from pathlib import Path
import plistlib
import pwd
import selectors
import signal
import socket
import stat
import subprocess
import time
from typing import Any, Callable, ContextManager, Iterator, Sequence

_NATIVE_REVIEWED_ROOT = os.environ.get("KAGEMUSHA_SEALED_BUILDER_REVIEWED_ROOT")
if _NATIVE_REVIEWED_ROOT is None:
    REPO_ROOT = Path(__file__).resolve().parent.parent
else:
    REPO_ROOT = Path(_NATIVE_REVIEWED_ROOT)
    if (
        not REPO_ROOT.is_absolute()
        or REPO_ROOT.resolve(strict=False) != REPO_ROOT
        or os.environ.get("KAGEMUSHA_SEALED_BUILDER_ENTRYPOINT_FD") != "12"
        or __file__ != "/dev/fd/12"
    ):
        raise SystemExit("sealed builder native entrypoint binding is malformed")
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts import kagemusha_source_tree_seal as source_seal
from scripts import profile_cargo_build as hermetic_build
from scripts.formal import run_sumeragi_v2_tlapm_guard as resource_guard


BINARY_NAME = "kagemusha_recursive_spend_v4_bundle"
SEALED_FEATURE = "kagemusha-candidate-source-seal"
CANDIDATE_EVIDENCE_FEATURE = "kagemusha-candidate-evidence-lab"
CANDIDATE_BUILD_FEATURES = (
    "iroha_core/dev-tools",
    f"iroha_core/{SEALED_FEATURE}",
    f"iroha_core/{CANDIDATE_EVIDENCE_FEATURE}",
)
AUTHENTICATED_SOURCE_SEAL_PROJECTION_SCHEMA = (
    "iroha.kagemusha.authenticated_source_seal_projection.v1"
)
SOURCE_SEAL_BUILD_SCRIPT_OBSERVED_SCHEMA = (
    "iroha.kagemusha.source_seal_build_script_observed.v1"
)
SOURCE_SEAL_OUTER_POLICY_SCHEMA = (
    "iroha.kagemusha.cprime_source_seal_outer_policy.v1"
)
MAX_AUTHENTICATED_SOURCE_SEAL_PROJECTION_BYTES = 16 * 1024
MAX_BUILD_INPUT_CLOSURE_BYTES = 8 * 1024
BUILD_INPUT_CLOSURE_SCHEMA = "iroha.kagemusha.build_input_closure.v1"
SEALED_DOUBLE_BUILD_REPORT_SCHEMA = (
    "iroha.kagemusha.sealed_candidate_double_build_report.v1"
)
NATIVE_SEALED_BUILDER_LAUNCH_CONTRACT = (
    "iroha.kagemusha.native-sealed-builder-launch.v1"
)
NATIVE_SEALED_BUILDER_ARGUMENT_CONTRACT = (
    "iroha.kagemusha.sealed-builder-exact-arguments.v1"
)
NATIVE_SEALED_BUILDER_ENVIRONMENT_CONTRACT = (
    "iroha.kagemusha.sealed-builder-exact-environment.v1"
)
NATIVE_SEALED_BUILDER_RECEIPT_FD_ENV = "KAGEMUSHA_SEALED_BUILDER_LAUNCH_FD"
NATIVE_SEALED_BUILDER_RECEIPT_SHA256_ENV = (
    "KAGEMUSHA_SEALED_BUILDER_LAUNCH_RECEIPT_SHA256"
)
NATIVE_SEALED_BUILDER_ENTRYPOINT_FD_ENV = "KAGEMUSHA_SEALED_BUILDER_ENTRYPOINT_FD"
NATIVE_SEALED_BUILDER_REVIEWED_ROOT_ENV = "KAGEMUSHA_SEALED_BUILDER_REVIEWED_ROOT"
NATIVE_SEALED_BUILDER_RECEIPT_MAX_BYTES = 16 * 1024
SEALED_CARGO_TIMEOUT_SECONDS = 30 * 60
SEALED_CARGO_STDOUT_MAX_BYTES = 128 * 1024 * 1024
SEALED_CARGO_STDERR_MAX_BYTES = 8 * 1024 * 1024
SOURCE_SEAL_CAPTURE_RECEIPT_SCHEMA = (
    "iroha.kagemusha.cargo_unit_graph_capture_receipt.v1"
)
SOURCE_SEAL_CAPTURE_RECEIPT_KEYS = {
    "build_inputs_sha256",
    "cargo_binary_sha256",
    "exit_status",
    "raw_stdout_sha256",
    "raw_stdout_size_bytes",
    "rustc_binary_sha256",
    "schema",
    "source_commit",
    "source_tree_sha256",
    "stderr_sha256",
    "stderr_size_bytes",
}
SOURCE_SEAL_PREFLIGHT_REPORT_KEYS = {
    "capture_exit_status",
    "capture_stderr_sha256",
    "capture_stderr_size_bytes",
    "controller_raw_sha256",
    "controller_raw_size_bytes",
    "custom_build_packages",
    "custom_build_units",
    "fresh_exit_status",
    "fresh_raw_sha256",
    "fresh_raw_size_bytes",
    "fresh_stderr_sha256",
    "fresh_stderr_size_bytes",
    "iroha_core_units",
    "normalization",
    "normalized_sha256",
    "normalized_size_bytes",
    "packages",
    "sha256",
    "size_bytes",
    "units",
}
REQUIRED_HOST_TOOL_PATHS = (
    "/bin/bash",
    "/bin/sh",
    "/bin/ps",
    "/usr/bin/ar",
    "/usr/bin/as",
    "/usr/bin/cc",
    "/usr/bin/clang",
    "/usr/bin/clang++",
    "/usr/bin/dsymutil",
    "/usr/bin/env",
    "/usr/bin/install_name_tool",
    "/usr/bin/ld",
    "/usr/bin/libtool",
    "/usr/bin/lipo",
    "/usr/bin/nm",
    "/usr/bin/otool",
    "/usr/bin/ranlib",
    "/usr/bin/sandbox-exec",
    "/usr/bin/strip",
    "/usr/bin/xcrun",
)
PRODUCTION_XCODE_DEVELOPER_DIR = Path(
    "/private/var/db/kagemusha/Xcode/Developer"
)
PRODUCTION_XCODE_SDKROOT = (
    PRODUCTION_XCODE_DEVELOPER_DIR
    / "Platforms/MacOSX.platform/Developer/SDKs/MacOSX26.2.sdk"
)
RUNTIME_IDENTITY_POLICY = "dedicated-nologin-no-concurrent-process-v1"
RUNTIME_ACCOUNT_NAME = "_iroha_kagemusha_build"
RUNTIME_GROUP_NAME = "_iroha_kagemusha_build"
RUNTIME_LOCK_ROOT = Path("/private/var/db/kagemusha/runtime-locks")
PRODUCTION_PYTHON_RUNTIME_PARENT = Path(
    "/private/var/db/iroha-kagemusha-python-runtime-v1"
)
MACOS_ACL_TYPE_EXTENDED = 0x00000100
MACOS_ACL_FIRST_ENTRY = 0
MACOS_XATTR_NOFOLLOW = 0x0001
if sys.platform == "darwin":
    _MACOS_LIBC = ctypes.CDLL(None, use_errno=True)
    _MACOS_LIBC.acl_get_fd_np.argtypes = [ctypes.c_int, ctypes.c_int]
    _MACOS_LIBC.acl_get_fd_np.restype = ctypes.c_void_p
    _MACOS_LIBC.acl_get_entry.argtypes = [
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.POINTER(ctypes.c_void_p),
    ]
    _MACOS_LIBC.acl_get_entry.restype = ctypes.c_int
    _MACOS_LIBC.acl_free.argtypes = [ctypes.c_void_p]
    _MACOS_LIBC.acl_free.restype = ctypes.c_int
    _MACOS_LIBC.flistxattr.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_int,
    ]
    _MACOS_LIBC.flistxattr.restype = ctypes.c_ssize_t
    _MACOS_LIBC.listxattr.argtypes = [
        ctypes.c_char_p,
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_int,
    ]
    _MACOS_LIBC.listxattr.restype = ctypes.c_ssize_t
else:
    _MACOS_LIBC = None
SOURCE_SEAL_TARGET = "aarch64-apple-darwin"
SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION = (
    "cargo-unit-graph-v1-package-root-relative-src-path-source-cache-"
    "placeholders-sorted-compact-lf-v1"
)
SOURCE_SEAL_RESOLVED_FEATURES = (
    "bls",
    "circuit-params",
    "default",
    "dev-tools",
    "json",
    "kagemusha-candidate-evidence-lab",
    "kagemusha-candidate-source-seal",
    "node",
    "proofs-halo2",
    "proofs-stark",
    "runtime",
    "zk-halo2",
    "zk-halo2-ipa",
    "zk-ipa-native",
    "zk-stark",
)
SOURCE_SEAL_EXPLICIT_FEATURES = (
    "iroha_core/dev-tools",
    "iroha_core/kagemusha-candidate-evidence-lab",
    "iroha_core/kagemusha-candidate-source-seal",
)
SOURCE_SEAL_SEMANTIC_ARGV = (
    "build",
    "--release",
    "--locked",
    "--offline",
    "--target",
    SOURCE_SEAL_TARGET,
    "--target-dir",
    "<EXTERNAL_TARGET_DIR>",
    "-p",
    "iroha_core",
    "--features",
    ",".join(CANDIDATE_BUILD_FEATURES),
    "--bin",
    BINARY_NAME,
    "--jobs",
    "1",
    "--message-format=json-render-diagnostics",
)
AUTHORIZED_SOURCE_PARENT_COMMIT = "5d41c784787ed496ccbd46379ee236cc992d9c65"
AUTHORIZED_SOURCE_PARENT_TREE = "f20ab04ddd65c2b7da71250e77e2cc1006aa38f2"
AUTHORIZED_SOURCE_PARENT_EPOCH = 1_786_749_503
# The measured single-rustc frontend high-water mark is about 11.466 GiB.
# Requiring 24 GiB of installed physical memory leaves slightly more than a
# two-times margin.  This is only build admission: it neither reduces compiler
# memory nor imposes a hard RSS limit once Cargo starts.
MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES = 24 * 1024 * 1024 * 1024
# Complete caller-supplied environment authenticated for unit-graph capture.
# Candidate compilation materializes this exact allowlist, substitutes only the
# the explicit path placeholders, and then adds projection-bound Kagemusha
# variables plus the already authenticated direct Cargo path. Arbitrary build
# scripts may interpret otherwise innocuous names, so a blacklist cannot close
# this input surface.
SOURCE_SEAL_CAPTURE_ENVIRONMENT = {
    "CARGO_ENCODED_RUSTFLAGS": "",
    "CARGO_HOME": "<OWNER_CONTROLLED_CACHE_ONLY_CARGO_HOME>",
    "CARGO_NET_OFFLINE": "true",
    "DEVELOPER_DIR": "<ROOT_CUSTODIED_DEVELOPER_DIR>",
    "HOME": "/var/empty",
    "LANG": "C",
    "LC_ALL": "C",
    "PATH": "<ROOT_CUSTODIED_HOST_TOOL_BIN>",
    "RUSTC": "<DIRECT_RUSTC>",
    "RUSTC_WRAPPER": "",
    "RUSTC_WORKSPACE_WRAPPER": "",
    "RUSTFLAGS": "",
    "SDKROOT": "<ROOT_CUSTODIED_SDKROOT>",
    "TMPDIR": "<FRESH_WRITABLE_BUILD_TMP>",
    "TZ": "UTC",
}
_UNIT_GRAPH_KEYS = {"roots", "units", "version"}
_UNIT_GRAPH_UNIT_KEYS = {
    "dependencies",
    "features",
    "mode",
    "pkg_id",
    "platform",
    "profile",
    "target",
}
_UNIT_GRAPH_TARGET_BASE_KEYS = {
    "crate_types",
    "doc",
    "doctest",
    "edition",
    "kind",
    "name",
    "src_path",
    "test",
}
_UNIT_GRAPH_TARGET_OPTIONAL_KEYS = {"required-features"}
_UNIT_GRAPH_PROFILE_KEYS = {
    "codegen_backend",
    "codegen_units",
    "debug_assertions",
    "debuginfo",
    "incremental",
    "lto",
    "name",
    "opt_level",
    "overflow_checks",
    "panic",
    "rpath",
    "split_debuginfo",
    "strip",
}
_UNIT_GRAPH_DEPENDENCY_KEYS = {"extern_crate_name", "index", "noprelude", "public"}
_UNIT_GRAPH_SOURCE_CACHE_MARKERS = ("/git/checkouts/", "/registry/src/")
_REMOVED_BUILD_ENVIRONMENT = {
    "CARGO",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_ENCODED_RUSTDOCFLAGS",
    "CARGO_HOME",
    "HOME",
    "PATH",
    "RUSTC",
    "RUSTC_BOOTSTRAP",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTDOC",
    "RUSTDOCFLAGS",
    "RUSTFLAGS",
    "RUSTUP_TOOLCHAIN",
    "RUSTUP_HOME",
    "SOURCE_DATE_EPOCH",
    "KAGEMUSHA_SOURCE_SEAL_PYTHON",
}
_REMOVED_BUILD_ENVIRONMENT_PREFIXES = (
    "CARGO_",
    "CARGO_ALIAS_",
    "CARGO_BUILD_",
    "CARGO_FEATURE_",
    "CARGO_HTTP_",
    "CARGO_NET_",
    "CARGO_PROFILE_",
    "CARGO_REGISTRIES_",
    "CARGO_REGISTRY_",
    "CARGO_TARGET_",
    "CARGO_TERM_",
    "DYLD_",
    "LD_",
    "KAGEMUSHA_BUILD_",
    "RUSTUP_",
    "RUST_",
    "RUSTC_",
)

# Native build helpers accept a much wider ambient control surface than Cargo
# and rustc themselves. The authenticated projection carries the exact helper,
# SDK, and tool-root identities; it never authorizes caller-selected redirects.
# None of these ambient controls may survive into this build. Any future
# exception must first become an exact, content-pinned member of the signed
# build-input contract; it must never be added as an ambient allow-list entry.
_REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT = frozenset(
    {
        "AR",
        "ARCHS",
        "ARFLAGS",
        "AS",
        "ASFLAGS",
        "BINDGEN_EXTRA_CLANG_ARGS",
        "CC",
        "CC_ENABLE_DEBUG_OUTPUT",
        "CC_FORCE_DISABLE",
        "CC_SHELL_ESCAPED_FLAGS",
        "CFLAGS",
        "CMAKE",
        "COMPILER_PATH",
        "CPATH",
        "CPP",
        "CPPFLAGS",
        "CRATE_CC_NO_DEFAULTS",
        "CUDACXX",
        "CUDA_HOME",
        "CUDA_PATH",
        "CPLUS_INCLUDE_PATH",
        "CXX",
        "CXXFLAGS",
        "C_INCLUDE_PATH",
        "DEVELOPER_DIR",
        "GCC_EXEC_PREFIX",
        "HOST_AR",
        "HOST_CC",
        "HOST_CFLAGS",
        "HOST_CPP",
        "HOST_CPPFLAGS",
        "HOST_CXX",
        "HOST_CXXFLAGS",
        "HOST_LD",
        "HOST_LDFLAGS",
        "HOST_RANLIB",
        "IPHONEOS_DEPLOYMENT_TARGET",
        "LD",
        "LDFLAGS",
        "LIBCLANG_PATH",
        "LIBRARY_PATH",
        "LIBTOOL",
        "LINKER",
        "LIPO",
        "MACOSX_DEPLOYMENT_TARGET",
        "MAKE",
        "MAKEFLAGS",
        "METAL",
        "MFLAGS",
        "NM",
        "NVCC",
        "NVCCFLAGS",
        "OBJC",
        "OBJCFLAGS",
        "OBJCXX",
        "OBJCXXFLAGS",
        "OBJC_INCLUDE_PATH",
        "OTOOL",
        "PKG_CONFIG",
        "PKG_CONFIG_ALLOW_CROSS",
        "PKG_CONFIG_ALLOW_SYSTEM_CFLAGS",
        "PKG_CONFIG_ALLOW_SYSTEM_LIBS",
        "PKG_CONFIG_LIBDIR",
        "PKG_CONFIG_PATH",
        "PKG_CONFIG_SYSROOT_DIR",
        "RANLIB",
        "RC_ARCHS",
        "SDKROOT",
        "STRIP",
        "TARGET_AR",
        "TARGET_CC",
        "TARGET_CFLAGS",
        "TARGET_CPP",
        "TARGET_CPPFLAGS",
        "TARGET_CXX",
        "TARGET_CXXFLAGS",
        "TARGET_LD",
        "TARGET_LDFLAGS",
        "TARGET_RANLIB",
        "TOOLCHAINS",
        "TVOS_DEPLOYMENT_TARGET",
        "WATCHOS_DEPLOYMENT_TARGET",
        "XCODE_DEFAULT_TOOLCHAIN_OVERRIDE",
        "XROS_DEPLOYMENT_TARGET",
        "ZERO_AR_DATE",
    }
)
_REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT_PREFIXES = (
    "ARFLAGS_",
    "AR_",
    "ASFLAGS_",
    "AS_",
    "BINDGEN_EXTRA_CLANG_ARGS_",
    "CCACHE_",
    "CC_",
    "CFLAGS_",
    "CMAKE_",
    "CONAN_",
    "CPPFLAGS_",
    "CPP_",
    "CUDA_",
    "CXXFLAGS_",
    "CXX_",
    "DISTCC_",
    "HOST_PKG_CONFIG_",
    "LDFLAGS_",
    "LD_",
    "MESON_",
    "NM_",
    "NVCC_",
    "OPENSSL_",
    "PKG_CONFIG_",
    "RANLIB_",
    "SCCACHE_",
    "STRIP_",
    "TARGET_PKG_CONFIG_",
    "VCPKG_",
    "XCODE_",
)
_REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT_SUFFIXES = (
    "_AR",
    "_ARFLAGS",
    "_AS",
    "_ASFLAGS",
    "_CC",
    "_CFLAGS",
    "_CPP",
    "_CPPFLAGS",
    "_CXX",
    "_CXXFLAGS",
    "_LD",
    "_LDFLAGS",
    "_LINKER",
    "_NM",
    "_RANLIB",
    "_STRIP",
)


class CandidateBuildError(RuntimeError):
    """A sealed candidate generator could not be built unambiguously."""


@dataclass(frozen=True)
class AdmittedExecutable:
    """One content-pinned direct tool executable and its launch path."""

    command_path: str
    resolved_path: Path
    selected_identity: tuple[int, ...]
    resolved_identity: tuple[int, ...]
    sha256: str


@dataclass(frozen=True)
class MaterializedBuildInputs:
    """Private build roots that are bound to the authenticated input closure."""

    source_root: Path
    verification_source_root: Path
    cargo_home: Path
    cargo: AdmittedExecutable
    rustc: AdmittedExecutable
    host_tool_bin: Path
    target_dir: Path
    verification_target_dir: Path
    unit_graph_target_dir: Path
    temporary_dir: Path
    verification_temporary_dir: Path
    unit_graph_temporary_dir: Path
    output_uid: int
    launch_prefix: tuple[str, ...]
    verification_launch_prefix: tuple[str, ...]
    unit_graph_launch_prefix: tuple[str, ...]
    drop_privileges: Callable[[], None]
    reset_cargo_lock: Callable[[], None]
    revalidate: Callable[[], None]


@dataclass(frozen=True)
class UnitGraphEvidence:
    """Controller-captured graph bytes independently normalized by the builder."""

    raw: bytes
    normalized: bytes
    summary: dict[str, int | str]
    capture_receipt: dict[str, object]


def _read_linux_meminfo() -> str:
    """Read Linux's installed-memory inventory for the sysconf fallback."""

    return Path("/proc/meminfo").read_text(encoding="ascii")


def _physical_memory_bytes(
    *,
    platform_name: str | None = None,
    inspection_runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
    sysconf_reader: Callable[[str], int] = os.sysconf,
    linux_meminfo_reader: Callable[[], str] = _read_linux_meminfo,
) -> int:
    """Return installed physical memory on macOS or Linux, failing closed."""

    platform_name = sys.platform if platform_name is None else platform_name
    if platform_name == "darwin":
        try:
            completed = inspection_runner(
                ["/usr/sbin/sysctl", "-n", "hw.memsize"],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="ascii",
                errors="strict",
                timeout=5,
            )
            if completed.returncode == 0:
                value = int(completed.stdout.strip())
                if value > 0:
                    return value
        except (OSError, ValueError, subprocess.TimeoutExpired):
            pass

    if platform_name == "darwin" or platform_name.startswith("linux"):
        try:
            pages = int(sysconf_reader("SC_PHYS_PAGES"))
            page_size = int(sysconf_reader("SC_PAGE_SIZE"))
            value = pages * page_size
            if pages > 0 and page_size > 0 and value > 0:
                return value
        except (KeyError, OSError, TypeError, ValueError):
            pass

    if platform_name.startswith("linux"):
        try:
            for line in linux_meminfo_reader().splitlines():
                fields = line.split()
                if len(fields) == 3 and fields[0] == "MemTotal:" and fields[2] == "kB":
                    value = int(fields[1]) * 1024
                    if value > 0:
                        return value
                    break
        except (OSError, UnicodeError, ValueError):
            pass

    raise CandidateBuildError(
        "could not determine installed physical memory on this macOS/Linux host"
    )


def _admitted_physical_memory_bytes(reader: Callable[[], int]) -> int:
    """Apply the non-bypassable installed-memory floor before Cargo starts."""

    try:
        physical_memory_bytes = reader()
    except CandidateBuildError:
        raise
    except (OSError, TypeError, ValueError) as error:
        raise CandidateBuildError(
            "could not determine installed physical memory"
        ) from error
    if (
        isinstance(physical_memory_bytes, bool)
        or not isinstance(physical_memory_bytes, int)
        or physical_memory_bytes <= 0
    ):
        raise CandidateBuildError("installed physical memory measurement is invalid")
    if physical_memory_bytes < MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES:
        raise CandidateBuildError(
            "sealed candidate build requires at least "
            f"{MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES} bytes of installed physical memory; "
            f"detected {physical_memory_bytes} bytes"
        )
    return physical_memory_bytes


@contextmanager
def _shared_memory_heavy_build_lock() -> Iterator[None]:
    """Serialize Cargo with other repository memory-heavy host jobs."""

    try:
        with resource_guard._host_lock(
            resource_guard.HEAVY_JOB_LOCK_PATH,
            description="memory-heavy job",
        ):
            yield
    except resource_guard.LockUnavailable as error:
        raise CandidateBuildError(
            "another guarded memory-heavy job owns the host lock"
        ) from error
    except resource_guard.GuardError as error:
        raise CandidateBuildError(
            f"could not acquire the memory-heavy host lock: {error}"
        ) from error


@contextmanager
def _dedicated_runtime_identity_lock(uid: int) -> Iterator[None]:
    """Serialize every cooperating build that can assume the dedicated UID."""

    if os.geteuid() != 0:
        raise CandidateBuildError("the dedicated runtime-identity lock requires root")
    _require_custodied_directory_ancestry(
        RUNTIME_LOCK_ROOT,
        "runtime-identity lock root",
        allowed_uids=(0,),
    )
    name = f"uid-{uid}.lock"
    root_fd = os.open(
        RUNTIME_LOCK_ROOT,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    descriptor = -1
    try:
        descriptor = os.open(
            name,
            os.O_RDWR
            | os.O_CREAT
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
            dir_fd=root_fd,
        )
        metadata = os.fstat(descriptor)
        named = os.stat(name, dir_fd=root_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or not os.path.samestat(metadata, named)
            or metadata.st_uid != 0
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise CandidateBuildError("runtime-identity lock file is not root-custodied")
        _require_no_extended_metadata_fd(descriptor, "runtime-identity lock file")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise CandidateBuildError(
                "another sealed build owns the authenticated runtime identity"
            ) from error
        yield
    finally:
        if descriptor >= 0:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_UN)
            finally:
                os.close(descriptor)
        os.close(root_fd)


def _require_no_process_for_runtime_uid(ps_path: Path, uid: int) -> None:
    """Prove the locked service UID has no process outside the next build child."""

    try:
        completed = subprocess.run(
            [str(ps_path), "-axo", "uid="],
            cwd=Path("/"),
            env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            close_fds=True,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CandidateBuildError("could not inspect dedicated runtime processes") from error
    if (
        completed.returncode != 0
        or completed.stderr
        or len(completed.stdout) > 16 * 1024 * 1024
    ):
        raise CandidateBuildError("dedicated runtime process inventory failed closed")
    try:
        observed = [int(line) for line in completed.stdout.decode("ascii").splitlines()]
    except (UnicodeError, ValueError) as error:
        raise CandidateBuildError("dedicated runtime process inventory is malformed") from error
    if uid in observed:
        raise CandidateBuildError(
            "the authenticated runtime UID already owns a concurrent process"
        )


def _validate_dedicated_runtime_identity(
    build_inputs: dict[str, Any], uid: int, gid: int, ps_path: Path
) -> None:
    """Validate the signed, locked, non-login service identity and idle state."""

    identity = build_inputs["runtime_identity"]
    if uid != identity["uid"] or gid != identity["gid"]:
        raise CandidateBuildError("runtime identity differs from the signed closure")
    try:
        account = pwd.getpwuid(uid)
        group = grp.getgrgid(gid)
        groups = grp.getgrall()
    except KeyError as error:
        raise CandidateBuildError(
            "the authenticated build-only account/group is not provisioned"
        ) from error
    if (
        account.pw_name != identity["account_name"]
        or account.pw_gid != gid
        or account.pw_dir != "/var/empty"
        or account.pw_shell != "/usr/bin/false"
        or group.gr_name != identity["group_name"]
        or group.gr_mem
        or any(account.pw_name in entry.gr_mem for entry in groups)
    ):
        raise CandidateBuildError(
            "the authenticated runtime identity is not one dedicated nologin account"
        )
    _require_no_process_for_runtime_uid(ps_path, uid)


def _tool_identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return the metadata fields retained for an admitted tool."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _require_no_extended_metadata_fd(descriptor: int, label: str) -> None:
    """Reject macOS ACL and xattr authority not represented by mode/tree hashes."""

    if _MACOS_LIBC is None:
        return
    ctypes.set_errno(0)
    acl = _MACOS_LIBC.acl_get_fd_np(descriptor, MACOS_ACL_TYPE_EXTENDED)
    if not acl:
        error_number = ctypes.get_errno()
        if error_number != errno.ENOENT:
            raise CandidateBuildError(f"could not inspect extended ACL on {label}")
    else:
        entry = ctypes.c_void_p()
        ctypes.set_errno(0)
        entry_status = _MACOS_LIBC.acl_get_entry(
            acl, MACOS_ACL_FIRST_ENTRY, ctypes.byref(entry)
        )
        entry_error = ctypes.get_errno()
        ctypes.set_errno(0)
        free_status = _MACOS_LIBC.acl_free(acl)
        free_error = ctypes.get_errno()
        if entry_status < 0:
            raise CandidateBuildError(
                f"could not inspect extended ACL on {label}: errno {entry_error}"
            )
        if free_status != 0:
            raise CandidateBuildError(
                f"could not release extended ACL on {label}: errno {free_error}"
            )
        if entry_status == 0:
            raise CandidateBuildError(f"{label} has an extended ACL")
    ctypes.set_errno(0)
    xattr_size = _MACOS_LIBC.flistxattr(descriptor, None, 0, 0)
    if xattr_size < 0:
        xattr_error = ctypes.get_errno()
        raise CandidateBuildError(
            f"could not inspect xattrs on {label}: errno {xattr_error}"
        )
    if xattr_size != 0:
        raise CandidateBuildError(f"{label} has unbound extended attributes")


def _require_no_symlink_xattrs(path: Path, label: str) -> None:
    """Reject xattrs on one symlink which cannot be inspected through a file fd."""

    if _MACOS_LIBC is None:
        return
    ctypes.set_errno(0)
    xattr_size = _MACOS_LIBC.listxattr(
        os.fsencode(path), None, 0, MACOS_XATTR_NOFOLLOW
    )
    if xattr_size < 0:
        xattr_error = ctypes.get_errno()
        raise CandidateBuildError(
            f"could not inspect xattrs on {label}: errno {xattr_error}"
        )
    if xattr_size != 0:
        raise CandidateBuildError(f"{label} has unbound extended attributes")


def _require_custodied_directory_ancestry(
    path: Path,
    label: str,
    *,
    allowed_uids: tuple[int, ...],
) -> None:
    """Open and verify every directory component without following symlinks."""

    for component in reversed((path, *path.parents)):
        try:
            before = component.lstat()
            descriptor = os.open(
                component,
                os.O_RDONLY
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_NOFOLLOW", 0),
            )
        except OSError as error:
            raise CandidateBuildError(
                f"{label} ancestry is unavailable: {component}"
            ) from error
        try:
            opened = os.fstat(descriptor)
            if (
                not stat.S_ISDIR(opened.st_mode)
                or not os.path.samestat(before, opened)
                or opened.st_uid not in allowed_uids
                or opened.st_mode & 0o022
            ):
                raise CandidateBuildError(
                    f"{label} ancestry is not exclusively custodied: {component}"
                )
            _require_no_extended_metadata_fd(
                descriptor, f"{label} ancestry component {component}"
            )
            after = component.lstat()
            if not os.path.samestat(opened, after):
                raise CandidateBuildError(
                    f"{label} ancestry changed during inspection: {component}"
                )
        finally:
            os.close(descriptor)


def _hash_regular_tool(path: Path) -> tuple[str, tuple[int, ...]]:
    """Hash one non-symlink executable through an exact retained descriptor."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        before = os.fstat(descriptor)
        digest = hashlib.sha256()
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if _tool_identity(before) != _tool_identity(after):
        raise CandidateBuildError("tool executable changed while it was hashed")
    return digest.hexdigest(), _tool_identity(after)


def _admit_direct_executable(path: str, label: str) -> AdmittedExecutable:
    """Admit one absolute owner-controlled non-rustup tool and pin its bytes."""

    requested = Path(path)
    if (
        not requested.is_absolute()
        or os.path.normpath(os.fspath(requested)) != os.fspath(requested)
    ):
        raise CandidateBuildError(f"{label} executable path must be absolute and normalized")
    try:
        link_metadata = requested.lstat()
        resolved = requested.resolve(strict=True)
        executable_metadata = resolved.stat()
    except OSError as error:
        raise CandidateBuildError(f"{label} executable is unavailable") from error
    if not (stat.S_ISREG(link_metadata.st_mode) or stat.S_ISLNK(link_metadata.st_mode)):
        raise CandidateBuildError(f"{label} executable path has an unsafe file type")
    if link_metadata.st_uid not in (0, os.geteuid()):
        raise CandidateBuildError(f"{label} executable path has an unsafe owner")
    if (
        not stat.S_ISREG(executable_metadata.st_mode)
        or executable_metadata.st_uid not in (0, os.geteuid())
        or executable_metadata.st_mode & 0o022 != 0
        or executable_metadata.st_mode & stat.S_IXUSR == 0
    ):
        raise CandidateBuildError(f"{label} executable has unsafe metadata")
    if resolved.name == "rustup":
        raise CandidateBuildError(
            f"{label} must name the direct toolchain binary, not a rustup proxy"
        )
    sha256, resolved_identity = _hash_regular_tool(resolved)
    if resolved_identity != _tool_identity(executable_metadata):
        raise CandidateBuildError(f"{label} executable changed during admission")
    return AdmittedExecutable(
        command_path=str(requested if stat.S_ISLNK(link_metadata.st_mode) else resolved),
        resolved_path=resolved,
        selected_identity=_tool_identity(link_metadata),
        resolved_identity=resolved_identity,
        sha256=sha256,
    )


def _revalidate_admitted_executable(tool: AdmittedExecutable, label: str) -> None:
    """Prove an admitted executable path and target still name the pinned bytes."""

    selected = Path(tool.command_path)
    try:
        selected_identity = _tool_identity(selected.lstat())
        resolved = selected.resolve(strict=True)
        sha256, resolved_identity = _hash_regular_tool(resolved)
    except OSError as error:
        raise CandidateBuildError(f"{label} executable became unavailable") from error
    if (
        selected_identity != tool.selected_identity
        or resolved != tool.resolved_path
        or resolved_identity != tool.resolved_identity
        or sha256 != tool.sha256
    ):
        raise CandidateBuildError(f"{label} executable changed after admission")


def _require_executable_digest_pin(
    tool: AdmittedExecutable, expected_sha256: str, label: str
) -> None:
    """Require an independently reviewed SHA-256 pin for one admitted tool."""

    if (
        len(expected_sha256) != 64
        or expected_sha256 == "0" * 64
        or any(character not in "0123456789abcdef" for character in expected_sha256)
    ):
        raise CandidateBuildError(f"{label} SHA-256 pin is not canonical")
    if tool.sha256 != expected_sha256:
        raise CandidateBuildError(f"{label} executable differs from its SHA-256 pin")


def _admitted_cargo_executable(cargo: str) -> str:
    """Compatibility wrapper returning one admitted direct Cargo path."""

    return _admit_direct_executable(cargo, "Cargo").command_path


def _admit_cargo_home(root: Path, requested: Path) -> Path:
    """Admit a canonical cache-only Cargo home outside the signed source tree."""

    if (
        not requested.is_absolute()
        or os.path.normpath(os.fspath(requested)) != os.fspath(requested)
    ):
        raise CandidateBuildError("Cargo home path must be absolute and normalized")
    try:
        metadata = requested.lstat()
        resolved = requested.resolve(strict=True)
    except OSError as error:
        raise CandidateBuildError("Cargo home is unavailable") from error
    if (
        resolved != requested
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o022 != 0
    ):
        raise CandidateBuildError(
            "Cargo home must be one canonical owner-controlled directory"
        )
    try:
        resolved.relative_to(root)
    except ValueError:
        pass
    else:
        raise CandidateBuildError("Cargo home must be outside the source repository")
    for name in ("config", "config.toml"):
        if os.path.lexists(resolved / name):
            raise CandidateBuildError(
                "Cargo home must not contain an ambient Cargo configuration"
            )
    for ancestor in root.parents:
        for name in ("config", "config.toml"):
            if os.path.lexists(ancestor / ".cargo" / name):
                raise CandidateBuildError(
                    "source ancestors must not inject an ambient Cargo configuration"
                )
    _require_custodied_directory_ancestry(
        resolved,
        "Cargo home",
        allowed_uids=tuple(sorted({0, os.geteuid()})),
    )
    return resolved


def _prepare_fresh_external_target_dir(root: Path, requested: Path) -> Path:
    """Create one empty canonical target directory strictly outside the source tree."""

    if not requested.is_absolute() or os.path.normpath(os.fspath(requested)) != os.fspath(
        requested
    ):
        raise CandidateBuildError("target directory path must be absolute and normalized")
    parent = requested.parent
    try:
        parent_resolved = parent.resolve(strict=True)
        parent_metadata = parent.lstat()
    except OSError as error:
        raise CandidateBuildError("target directory parent is unavailable") from error
    if (
        parent_resolved != parent
        or not stat.S_ISDIR(parent_metadata.st_mode)
        or parent_metadata.st_uid not in (0, os.geteuid())
        or parent_metadata.st_mode & 0o022 != 0
    ):
        raise CandidateBuildError(
            "target directory parent must be canonical, owner-controlled, and not a symlink"
        )
    _require_custodied_directory_ancestry(
        parent,
        "target directory parent",
        allowed_uids=tuple(sorted({0, os.geteuid()})),
    )
    try:
        requested.relative_to(root)
    except ValueError:
        pass
    else:
        raise CandidateBuildError("target directory must be outside the source repository")
    parent_fd = -1
    target_fd = -1
    try:
        parent_fd = os.open(
            parent,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        opened_parent = os.fstat(parent_fd)
        if not os.path.samestat(parent_metadata, opened_parent):
            raise CandidateBuildError("target directory parent changed before creation")
        try:
            os.stat(requested.name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise CandidateBuildError("target directory must be a fresh nonexistent path")
        os.mkdir(requested.name, mode=0o700, dir_fd=parent_fd)
        target_fd = os.open(
            requested.name,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent_fd,
        )
        metadata = os.fstat(target_fd)
        named = os.stat(requested.name, dir_fd=parent_fd, follow_symlinks=False)
        entries = os.listdir(target_fd)
    except OSError as error:
        raise CandidateBuildError("could not create fresh external target directory") from error
    finally:
        if target_fd >= 0:
            os.close(target_fd)
        if parent_fd >= 0:
            os.close(parent_fd)
    target_dir = requested
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or not os.path.samestat(metadata, named)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077 != 0
        or entries
    ):
        raise CandidateBuildError("fresh external target directory is not exact and empty")
    _require_custodied_directory_ancestry(
        target_dir,
        "fresh external target directory",
        allowed_uids=tuple(sorted({0, os.geteuid()})),
    )
    return target_dir


def _is_ambient_native_toolchain_control(name: str) -> bool:
    """Return whether an environment name can redirect a native build helper."""

    normalized = name.upper()
    return (
        normalized in _REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT
        or normalized.startswith(_REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT_PREFIXES)
        or normalized.endswith(_REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT_SUFFIXES)
    )


def _sanitized_build_environment(
    cargo_home: Path,
    rustc: AdmittedExecutable,
    build_inputs: dict[str, Any] | None = None,
    host_tool_bin: Path | None = None,
    temporary_dir: Path | None = None,
) -> dict[str, str]:
    """Materialize exactly the environment authenticated for graph capture."""

    environment = dict(SOURCE_SEAL_CAPTURE_ENVIRONMENT)
    environment["CARGO_HOME"] = str(cargo_home)
    environment["RUSTC"] = rustc.command_path
    if build_inputs is not None:
        environment["DEVELOPER_DIR"] = build_inputs["developer_dir"]["path"]
        environment["SDKROOT"] = build_inputs["sdkroot"]["path"]
    if host_tool_bin is not None:
        environment["PATH"] = str(host_tool_bin)
    if temporary_dir is not None:
        environment["TMPDIR"] = str(temporary_dir)
    return environment


def _reject_duplicate_json_members(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Reject duplicate object members while decoding authenticated JSON."""

    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise CandidateBuildError(
                f"authenticated source-seal projection repeats JSON member {key!r}"
            )
        value[key] = item
    return value


def _reject_nonfinite_json_number(value: str) -> None:
    """Reject JSON extensions such as NaN and Infinity."""

    raise CandidateBuildError(
        f"authenticated source-seal projection uses forbidden number {value}"
    )


def _canonical_json_line(value: Any) -> bytes:
    """Encode one strict canonical ASCII JSON line plus LF."""

    try:
        return (
            json.dumps(
                value,
                allow_nan=False,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, UnicodeError, ValueError) as error:
        raise CandidateBuildError(
            "authenticated source-seal projection is not canonical JSON"
        ) from error


def _strict_unit_graph_json(payload: bytes, label: str) -> dict[str, Any]:
    """Decode one duplicate-free Cargo unit graph without accepting extensions."""

    if not 1 <= len(payload) <= 16 * 1024 * 1024:
        raise CandidateBuildError(f"{label} violates its byte bound")
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_duplicate_json_members,
            parse_constant=_reject_nonfinite_json_number,
        )
    except CandidateBuildError:
        raise
    except (json.JSONDecodeError, UnicodeError, ValueError) as error:
        raise CandidateBuildError(f"{label} is not strict JSON") from error
    graph = _exact_object(value, _UNIT_GRAPH_KEYS, label)
    if graph["version"] != 1:
        raise CandidateBuildError(f"{label} version is not exact V1")
    if not isinstance(graph["roots"], list) or not isinstance(graph["units"], list):
        raise CandidateBuildError(f"{label} roots or units are not arrays")
    if not 1 <= len(graph["units"]) <= 100_000 or not graph["roots"]:
        raise CandidateBuildError(f"{label} is empty or exceeds its unit bound")
    return graph


def _raw_unit_graph_absolute_path(value: Any, label: str) -> str:
    """Require one canonical absolute POSIX path emitted by Cargo."""

    if (
        not isinstance(value, str)
        or not value.startswith("/")
        or len(value.encode("utf-8")) > 4096
        or not value.isascii()
        or "\\" in value
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
        or os.path.normpath(value) != value
        or any(component in ("", ".", "..") for component in value.split("/")[1:])
    ):
        raise CandidateBuildError(f"{label} is not one canonical absolute Cargo path")
    return value


def _raw_path_package_identity(package_id: Any, label: str) -> tuple[str, str]:
    """Parse a Cargo path package id and return its prefix and absolute root."""

    if (
        not isinstance(package_id, str)
        or not package_id
        or len(package_id.encode("utf-8")) > 4096
        or not package_id.isascii()
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in package_id)
    ):
        raise CandidateBuildError(f"{label} is not bounded canonical ASCII")
    marker = " (path+file://"
    if marker not in package_id:
        if (
            "path+file://" in package_id
            or "<PACKAGE_ROOT>" in package_id
            or "<SOURCE_ROOT>" in package_id
        ):
            raise CandidateBuildError(f"{label} has a malformed path package identity")
        return package_id, ""
    if package_id.count(marker) != 1 or not package_id.endswith(")"):
        raise CandidateBuildError(f"{label} has a malformed path package identity")
    prefix, raw_root = package_id[:-1].split(marker, 1)
    root = _raw_unit_graph_absolute_path(raw_root, f"{label} package root")
    return prefix, root


def _unit_graph_capture_source_root(raw: dict[str, Any]) -> str:
    """Derive the capture checkout root from the exact selected iroha_core unit."""

    roots = raw["roots"]
    if (
        len(roots) != 1
        or type(roots[0]) is not int
        or not 0 <= roots[0] < len(raw["units"])
    ):
        raise CandidateBuildError("raw Cargo unit graph has no exact selected root")
    root_unit = _exact_object(
        raw["units"][roots[0]],
        _UNIT_GRAPH_UNIT_KEYS,
        "raw Cargo selected root unit",
    )
    prefix, package_root = _raw_path_package_identity(
        root_unit["pkg_id"], "raw Cargo selected root pkg_id"
    )
    suffix = "/crates/iroha_core"
    if (
        not prefix.startswith("iroha_core ")
        or not package_root.endswith(suffix)
        or package_root == suffix
    ):
        raise CandidateBuildError(
            "raw Cargo selected root is not the iroha_core path package"
        )
    source_root = package_root[: -len(suffix)]
    return _raw_unit_graph_absolute_path(
        source_root, "raw Cargo capture source root"
    )


def _path_package_root(
    package_id: Any, label: str, capture_source_root: str
) -> tuple[str, str]:
    """Normalize a path package while preserving its repository-relative root."""

    prefix, root = _raw_path_package_identity(package_id, label)
    if not root:
        return prefix, ""
    source_prefix = f"{capture_source_root}/"
    if not root.startswith(source_prefix):
        raise CandidateBuildError(
            f"{label} package root is outside the selected capture source root"
        )
    relative_root = root[len(source_prefix) :]
    if any(component in ("", ".", "..") for component in relative_root.split("/")):
        raise CandidateBuildError(f"{label} package root is not repository-relative")
    marker = " (path+file://"
    return f"{prefix}{marker}<SOURCE_ROOT>/{relative_root})", root


def _normalized_unit_graph_source_path(
    raw_source: Any, raw_package_root: str, label: str
) -> str:
    """Remove capture-host package/cache roots from one Cargo target source."""

    source = _raw_unit_graph_absolute_path(raw_source, label)
    if raw_package_root:
        prefix = f"{raw_package_root}/"
        if not source.startswith(prefix):
            raise CandidateBuildError(
                f"{label} is outside its path-package root from pkg_id"
            )
        relative = source[len(prefix) :]
    else:
        matches = [marker for marker in _UNIT_GRAPH_SOURCE_CACHE_MARKERS if marker in source]
        if len(matches) != 1:
            raise CandidateBuildError(
                f"{label} is neither below a path package nor one Cargo source cache"
            )
        marker = matches[0]
        capture_root, suffix = source.split(marker, 1)
        _raw_unit_graph_absolute_path(capture_root, f"{label} source-cache root")
        relative = f"<SOURCE_CACHE>{marker}{suffix}"
    if (
        not relative
        or relative.startswith(("/", "\\"))
        or "\\" in relative
        or any(component in ("", ".", "..") for component in relative.split("/"))
    ):
        raise CandidateBuildError(f"{label} did not normalize to one relative path")
    return relative


def _sorted_unique_graph_strings(value: Any, label: str) -> list[str]:
    """Canonicalize one duplicate-free Cargo set-like string array."""

    if not isinstance(value, list) or any(
        not isinstance(item, str)
        or not item
        or not item.isascii()
        or len(item.encode("utf-8")) > 4096
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in item)
        for item in value
    ):
        raise CandidateBuildError(f"{label} is not a bounded string array")
    if len(set(value)) != len(value):
        raise CandidateBuildError(f"{label} repeats a set-like value")
    return sorted(value)


def normalize_source_seal_unit_graph(payload: bytes) -> bytes:
    """Apply the exact source-seal normalization to genuine Cargo V1 stdout."""

    raw = _strict_unit_graph_json(payload, "raw Cargo unit graph")
    roots = raw["roots"]
    if any(type(index) is not int for index in roots):
        raise CandidateBuildError("raw Cargo unit-graph roots are not integers")
    if len(set(roots)) != len(roots):
        raise CandidateBuildError("raw Cargo unit-graph roots repeat an index")
    capture_source_root = _unit_graph_capture_source_root(raw)
    normalized_units: list[dict[str, Any]] = []
    for unit_index, raw_unit in enumerate(raw["units"]):
        unit = _exact_object(
            raw_unit,
            _UNIT_GRAPH_UNIT_KEYS,
            f"raw Cargo unit {unit_index}",
        )
        package_id, package_root = _path_package_root(
            unit["pkg_id"],
            f"raw Cargo unit {unit_index} pkg_id",
            capture_source_root,
        )
        target = unit["target"]
        if not isinstance(target, dict) or not (
            _UNIT_GRAPH_TARGET_BASE_KEYS <= set(target)
            and set(target)
            <= _UNIT_GRAPH_TARGET_BASE_KEYS | _UNIT_GRAPH_TARGET_OPTIONAL_KEYS
        ):
            raise CandidateBuildError(
                f"raw Cargo unit {unit_index} target fields are not exact"
            )
        normalized_target = dict(target)
        for field in ("crate_types", "kind"):
            normalized_target[field] = _sorted_unique_graph_strings(
                target[field], f"raw Cargo unit {unit_index} target {field}"
            )
        if "required-features" in target:
            normalized_target["required-features"] = _sorted_unique_graph_strings(
                target["required-features"],
                f"raw Cargo unit {unit_index} target required-features",
            )
        normalized_target["src_path"] = _normalized_unit_graph_source_path(
            target["src_path"],
            package_root,
            f"raw Cargo unit {unit_index} target src_path",
        )
        _exact_object(
            unit["profile"],
            _UNIT_GRAPH_PROFILE_KEYS,
            f"raw Cargo unit {unit_index} profile",
        )
        dependencies = unit["dependencies"]
        if not isinstance(dependencies, list):
            raise CandidateBuildError(
                f"raw Cargo unit {unit_index} dependencies are not an array"
            )
        ordered_dependencies: dict[tuple[int, str], dict[str, Any]] = {}
        for dependency_index, raw_dependency in enumerate(dependencies):
            dependency = _exact_object(
                raw_dependency,
                _UNIT_GRAPH_DEPENDENCY_KEYS,
                f"raw Cargo unit {unit_index} dependency {dependency_index}",
            )
            key = (dependency["index"], dependency["extern_crate_name"])
            if (
                type(key[0]) is not int
                or not isinstance(key[1], str)
                or key in ordered_dependencies
            ):
                raise CandidateBuildError(
                    f"raw Cargo unit {unit_index} dependencies are ambiguous"
                )
            ordered_dependencies[key] = dict(dependency)
        normalized_units.append(
            {
                **unit,
                "dependencies": [
                    ordered_dependencies[key] for key in sorted(ordered_dependencies)
                ],
                "features": _sorted_unique_graph_strings(
                    unit["features"], f"raw Cargo unit {unit_index} features"
                ),
                "pkg_id": package_id,
                "target": normalized_target,
            }
        )
    normalized_roots = sorted(roots)
    if any(not 0 <= index < len(normalized_units) for index in normalized_roots):
        raise CandidateBuildError("raw Cargo unit-graph root index is invalid")
    return _canonical_json_line(
        {"roots": normalized_roots, "units": normalized_units, "version": 1}
    )


def _unit_graph_summary(payload: bytes) -> dict[str, int | str]:
    """Compute every projection count from exact canonical normalized bytes."""

    graph = _strict_unit_graph_json(payload, "normalized Cargo unit graph")
    if _canonical_json_line(graph) != payload:
        raise CandidateBuildError("normalized Cargo unit graph is not canonical JSON")
    roots = graph["roots"]
    if len(roots) != 1 or type(roots[0]) is not int or not 0 <= roots[0] < len(
        graph["units"]
    ):
        raise CandidateBuildError("normalized Cargo unit graph has no exact root")
    root_unit = _exact_object(
        graph["units"][roots[0]],
        _UNIT_GRAPH_UNIT_KEYS,
        "normalized Cargo root unit",
    )
    root_package = root_unit["pkg_id"]
    packages: set[str] = set()
    custom_build_packages: set[str] = set()
    custom_build_units = 0
    for unit_index, raw_unit in enumerate(graph["units"]):
        unit = _exact_object(
            raw_unit,
            _UNIT_GRAPH_UNIT_KEYS,
            f"normalized Cargo unit {unit_index}",
        )
        package = unit["pkg_id"]
        if not isinstance(package, str):
            raise CandidateBuildError("normalized Cargo package id is not text")
        packages.add(package)
        target = unit["target"]
        if not isinstance(target, dict) or not isinstance(target.get("kind"), list):
            raise CandidateBuildError("normalized Cargo target is malformed")
        if "custom-build" in target["kind"] or unit["mode"] == "run-custom-build":
            custom_build_units += 1
            custom_build_packages.add(package)
    return {
        "custom_build_packages": len(custom_build_packages),
        "custom_build_units": custom_build_units,
        "iroha_core_units": sum(
            unit["pkg_id"] == root_package for unit in graph["units"]
        ),
        "normalization": SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION,
        "packages": len(packages),
        "sha256": hashlib.sha256(payload).hexdigest(),
        "size_bytes": len(payload),
        "units": len(graph["units"]),
    }


def _nonzero_lower_hex(value: Any, length: int, label: str) -> str:
    """Require one nonzero lowercase hexadecimal string of an exact length."""

    if (
        not isinstance(value, str)
        or len(value) != length
        or value == "0" * length
        or any(byte not in "0123456789abcdef" for byte in value)
    ):
        raise CandidateBuildError(f"{label} is not exact nonzero lowercase hexadecimal")
    return value


def _bounded_integer(
    value: Any, minimum: int, maximum: int, label: str
) -> int:
    """Require one JSON integer inside an inclusive authenticated bound."""

    if type(value) is not int or not minimum <= value <= maximum:
        raise CandidateBuildError(f"{label} is outside its authenticated integer bound")
    return value


def _exact_object(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    """Require a JSON object with exactly the named members."""

    if not isinstance(value, dict) or set(value) != keys:
        raise CandidateBuildError(f"{label} JSON members are not exact")
    return value


def _portable_identifier(value: Any, maximum: int, label: str) -> str:
    """Require the portable identifier alphabet accepted by the build script."""

    if (
        not isinstance(value, str)
        or not value
        or len(value) > maximum
        or any(not (byte.isalnum() or byte in "._@+-") for byte in value)
        or any(ord(byte) > 0x7F for byte in value)
    ):
        raise CandidateBuildError(f"{label} is not one bounded portable identifier")
    return value


def _canonical_absolute_path(value: Any, label: str) -> str:
    """Require one bounded, canonical absolute POSIX path from signed JSON."""

    if (
        not isinstance(value, str)
        or not value.startswith("/")
        or len(value.encode("utf-8")) > 4096
        or any(ord(character) < 0x20 or ord(character) > 0x7E for character in value)
        or os.path.normpath(value) != value
        or any(component in ("", ".", "..") for component in value.split("/")[1:])
    ):
        raise CandidateBuildError(f"{label} is not one canonical absolute path")
    return value


def _validate_tree_identity(value: Any, label: str) -> dict[str, Any]:
    """Validate one bounded tree fingerprint produced by the snapshot helper."""

    tree = _exact_object(
        value,
        {"bytes", "files", "records", "sha256"},
        label,
    )
    _nonzero_lower_hex(tree["sha256"], 64, f"{label} SHA-256")
    records = _bounded_integer(tree["records"], 1, 250_000, f"{label} records")
    files = _bounded_integer(tree["files"], 1, records, f"{label} files")
    _bounded_integer(
        tree["bytes"],
        files,
        64 * 1024 * 1024 * 1024,
        f"{label} bytes",
    )
    return tree


def _validate_build_input_closure(value: Any) -> dict[str, Any]:
    """Validate the complete signed cache, sysroot, linker, and SDK closure."""

    closure = _exact_object(
        value,
        {
            "cargo_home",
            "cargo_toolchain",
            "developer_dir",
            "host_tools",
            "platform",
            "python_runtime",
            "rust_toolchain",
            "runtime_identity",
            "sandbox",
            "schema",
            "sdkroot",
        },
        "build-input closure",
    )
    if closure["schema"] != BUILD_INPUT_CLOSURE_SCHEMA:
        raise CandidateBuildError("build-input closure schema differs")
    if closure["platform"] != "darwin":
        raise CandidateBuildError("build-input closure platform must be darwin")

    cargo_home = _exact_object(
        closure["cargo_home"],
        {"roots", "tree"},
        "build-input Cargo home",
    )
    if cargo_home["roots"] != ["git", "registry"]:
        raise CandidateBuildError("build-input Cargo-home roots are not exact")
    _validate_tree_identity(cargo_home["tree"], "build-input Cargo-home tree")

    cargo_toolchain = _exact_object(
        closure["cargo_toolchain"],
        {"cargo_relative_path", "tree"},
        "build-input Cargo toolchain",
    )
    if cargo_toolchain["cargo_relative_path"] != "bin/cargo":
        raise CandidateBuildError("build-input Cargo tool path is not exact")
    _validate_tree_identity(
        cargo_toolchain["tree"], "build-input Cargo toolchain tree"
    )
    rust_toolchain = _exact_object(
        closure["rust_toolchain"],
        {"rustc_relative_path", "tree"},
        "build-input rustc toolchain",
    )
    if rust_toolchain["rustc_relative_path"] != "bin/rustc":
        raise CandidateBuildError("build-input rustc tool path is not exact")
    _validate_tree_identity(
        rust_toolchain["tree"], "build-input rustc toolchain tree"
    )
    python_runtime = _exact_object(
        closure["python_runtime"],
        {"interpreter_path", "interpreter_sha256", "root", "tree_sha256"},
        "build-input Python runtime",
    )
    python_runtime_root = Path(
        _canonical_absolute_path(
            python_runtime["root"], "build-input Python runtime root"
        )
    )
    python_interpreter = Path(
        _canonical_absolute_path(
            python_runtime["interpreter_path"],
            "build-input Python interpreter path",
        )
    )
    if (
        python_runtime_root.parent != PRODUCTION_PYTHON_RUNTIME_PARENT
        or python_interpreter != python_runtime_root / "bin/python3"
    ):
        raise CandidateBuildError(
            "build-input Python runtime is outside the fixed private runtime layout"
        )
    _nonzero_lower_hex(
        python_runtime["interpreter_sha256"],
        64,
        "build-input Python interpreter SHA-256",
    )
    _nonzero_lower_hex(
        python_runtime["tree_sha256"],
        64,
        "build-input Python runtime tree SHA-256",
    )

    developer = _exact_object(
        closure["developer_dir"], {"path", "tree"}, "build-input developer dir"
    )
    sdk = _exact_object(
        closure["sdkroot"], {"path", "tree"}, "build-input SDK root"
    )
    developer_path = Path(
        _canonical_absolute_path(
            developer["path"], "build-input developer-dir path"
        )
    )
    sdk_path = Path(
        _canonical_absolute_path(sdk["path"], "build-input SDK-root path")
    )
    if (
        developer_path != PRODUCTION_XCODE_DEVELOPER_DIR
        or sdk_path != PRODUCTION_XCODE_SDKROOT
    ):
        raise CandidateBuildError(
            "build-input developer/SDK paths are not the fixed private Xcode layout"
        )
    try:
        relative_sdk = sdk_path.relative_to(developer_path)
    except ValueError as error:
        raise CandidateBuildError(
            "build-input SDK root is outside the developer directory"
        ) from error
    if not relative_sdk.parts:
        raise CandidateBuildError(
            "build-input SDK root must be below the developer directory"
        )
    _validate_tree_identity(developer["tree"], "build-input developer-dir tree")
    _validate_tree_identity(sdk["tree"], "build-input SDK-root tree")

    host_tools = closure["host_tools"]
    if not isinstance(host_tools, list) or len(host_tools) != len(
        REQUIRED_HOST_TOOL_PATHS
    ):
        raise CandidateBuildError("build-input host-tool allowlist is not exact")
    observed_paths: list[str] = []
    for index, raw_entry in enumerate(host_tools):
        entry = _exact_object(
            raw_entry,
            {"binary_sha256", "binary_size_bytes", "path", "resolved_path"},
            f"build-input host tool {index}",
        )
        observed_paths.append(
            _canonical_absolute_path(
                entry["path"], f"build-input host tool {index} path"
            )
        )
        _canonical_absolute_path(
            entry["resolved_path"], f"build-input host tool {index} resolved path"
        )
        _nonzero_lower_hex(
            entry["binary_sha256"], 64, f"build-input host tool {index} SHA-256"
        )
        _bounded_integer(
            entry["binary_size_bytes"],
            1,
            512 * 1024 * 1024,
            f"build-input host tool {index} size",
        )
    if observed_paths != list(REQUIRED_HOST_TOOL_PATHS):
        raise CandidateBuildError("build-input host-tool paths are not exact and ordered")
    runtime_identity = _exact_object(
        closure["runtime_identity"],
        {"account_name", "gid", "group_name", "policy", "uid"},
        "build-input runtime identity",
    )
    if (
        runtime_identity["account_name"] != RUNTIME_ACCOUNT_NAME
        or runtime_identity["group_name"] != RUNTIME_GROUP_NAME
        or runtime_identity["policy"] != RUNTIME_IDENTITY_POLICY
        or type(runtime_identity["uid"]) is not int
        or type(runtime_identity["gid"]) is not int
        or not 1 <= runtime_identity["uid"] <= 2**31 - 1
        or not 1 <= runtime_identity["gid"] <= 2**31 - 1
    ):
        raise CandidateBuildError("build-input runtime identity is not exact")
    sandbox = _exact_object(
        closure["sandbox"],
        {"backend", "os_build", "profile_schema", "qualification", "xcode_build"},
        "build-input sandbox",
    )
    if (
        sandbox["backend"] != "macos-seatbelt-v1"
        or sandbox["profile_schema"]
        != "iroha.kagemusha.sealed_candidate_build_seatbelt.v1"
        or sandbox["qualification"]
        != [
            "deny-ambient-read-v1",
            "deny-ambient-write-v1",
            "deny-network-v1",
            "deny-unlisted-exec-v1",
            "fresh-cargo-rustc-link-v1",
        ]
    ):
        raise CandidateBuildError("build-input sandbox qualification is not exact")
    for field in ("os_build", "xcode_build"):
        _portable_identifier(sandbox[field], 64, f"build-input sandbox {field}")
    return closure


def _projection_build_input_closure(
    outer: dict[str, Any],
) -> tuple[bytes, dict[str, Any], str]:
    """Decode and validate the build-input closure carried by a projection."""

    closure_sha256 = _nonzero_lower_hex(
        outer["build_inputs_sha256"], 64, "projection build-input closure SHA-256"
    )
    closure_hex = outer["build_inputs_hex"]
    if (
        not isinstance(closure_hex, str)
        or not 2 <= len(closure_hex) <= 2 * MAX_BUILD_INPUT_CLOSURE_BYTES
        or len(closure_hex) % 2 != 0
        or any(character not in "0123456789abcdef" for character in closure_hex)
    ):
        raise CandidateBuildError("projection build-input closure hex is malformed")
    closure_payload = bytes.fromhex(closure_hex)
    if hashlib.sha256(closure_payload).hexdigest() != closure_sha256:
        raise CandidateBuildError("projection build-input closure digest differs")
    try:
        closure_value = json.loads(
            closure_payload,
            object_pairs_hook=_reject_duplicate_json_members,
            parse_constant=_reject_nonfinite_json_number,
        )
    except CandidateBuildError:
        raise
    except (json.JSONDecodeError, UnicodeError, ValueError) as error:
        raise CandidateBuildError("projection build-input closure is not strict JSON") from error
    if _canonical_json_line(closure_value) != closure_payload:
        raise CandidateBuildError("projection build-input closure is not canonical")
    return (
        closure_payload,
        _validate_build_input_closure(closure_value),
        closure_sha256,
    )


def _read_authenticated_source_seal_projection(
    path: Path, expected_sha256: str
) -> tuple[bytes, dict[str, Any], str]:
    """Read one owner-controlled projection through pinned path descriptors."""

    expected_sha256 = _nonzero_lower_hex(
        expected_sha256,
        64,
        "authenticated source-seal projection SHA-256 pin",
    )
    try:
        payload = source_seal._read_bounded_absolute_file(
            path,
            "authenticated source-seal projection",
            MAX_AUTHENTICATED_SOURCE_SEAL_PROJECTION_BYTES,
            allow_empty=False,
            owner_controlled=True,
        )
    except source_seal.SourceSealError as error:
        raise CandidateBuildError(str(error)) from error
    actual_sha256 = hashlib.sha256(payload).hexdigest()
    if actual_sha256 != expected_sha256:
        raise CandidateBuildError(
            "authenticated source-seal projection digest differs from its pin"
        )
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_duplicate_json_members,
            parse_constant=_reject_nonfinite_json_number,
        )
    except CandidateBuildError:
        raise
    except (json.JSONDecodeError, UnicodeError, ValueError) as error:
        raise CandidateBuildError(
            "authenticated source-seal projection is not strict JSON"
        ) from error
    if _canonical_json_line(value) != payload:
        raise CandidateBuildError(
            "authenticated source-seal projection bytes are not canonical"
        )
    return payload, _exact_object(
        value,
        {
            "build_script_observed",
            "outer_policy",
            "reviewed_source_closure_hex",
            "reviewed_source_closure_sha256",
            "schema",
            "source_authority",
            "source_commit",
            "source_date_epoch",
            "source_repo_dirty",
            "source_tree_sha256",
        },
        "authenticated source-seal projection",
    ), actual_sha256


def _projection_build_environment(
    projection: dict[str, Any],
    projection_payload: bytes,
    projection_sha256: str,
    identity: source_seal.SourceIdentity,
) -> dict[str, str]:
    """Validate the projection against the source identity and derive build inputs."""

    if projection["schema"] != AUTHENTICATED_SOURCE_SEAL_PROJECTION_SCHEMA:
        raise CandidateBuildError("authenticated source-seal projection schema differs")
    source_commit = _nonzero_lower_hex(
        projection["source_commit"], 40, "projection source commit"
    )
    source_tree_sha256 = _nonzero_lower_hex(
        projection["source_tree_sha256"], 64, "projection source-tree SHA-256"
    )
    if (
        projection["source_repo_dirty"] is not False
        or source_commit != identity.source_commit
        or source_tree_sha256 != identity.source_tree_sha256
        or identity.source_repo_dirty
    ):
        raise CandidateBuildError(
            "authenticated source-seal projection differs from the clean source identity"
        )
    source_date_epoch = _bounded_integer(
        projection["source_date_epoch"],
        AUTHORIZED_SOURCE_PARENT_EPOCH + 1,
        2**63 - 1,
        "projection source date epoch",
    )
    actual_authority = identity.source_authority
    if source_date_epoch != actual_authority.committer_epoch:
        raise CandidateBuildError(
            "projection source epoch differs from the verified HEAD committer epoch"
        )

    observed = _exact_object(
        projection["build_script_observed"],
        {
            "debug_assertions",
            "features",
            "host",
            "num_jobs",
            "opt_level",
            "profile",
            "schema",
            "target",
        },
        "projection build-script observation",
    )
    expected_observed = {
        "debug_assertions": False,
        "features": list(SOURCE_SEAL_RESOLVED_FEATURES),
        "host": SOURCE_SEAL_TARGET,
        "num_jobs": 1,
        "opt_level": "3",
        "profile": "release",
        "schema": SOURCE_SEAL_BUILD_SCRIPT_OBSERVED_SCHEMA,
        "target": SOURCE_SEAL_TARGET,
    }
    if observed != expected_observed:
        raise CandidateBuildError(
            "authenticated source-seal build-script observation is not exact"
        )

    outer = _exact_object(
        projection["outer_policy"],
        {
            "build_inputs_hex",
            "build_inputs_sha256",
            "cargo",
            "execution_policy_sha256",
            "schema",
            "toolchain",
        },
        "projection outer policy",
    )
    if outer["schema"] != SOURCE_SEAL_OUTER_POLICY_SCHEMA:
        raise CandidateBuildError("authenticated source-seal outer policy schema differs")
    execution_policy_sha256 = _nonzero_lower_hex(
        outer["execution_policy_sha256"], 64, "projection execution-policy SHA-256"
    )
    build_input_payload, _build_inputs, build_input_sha256 = (
        _projection_build_input_closure(outer)
    )
    cargo = _exact_object(
        outer["cargo"],
        {
            "binary",
            "explicit_features",
            "package",
            "profile",
            "semantic_argv",
            "target",
            "unit_graph",
        },
        "projection Cargo policy",
    )
    if {
        key: cargo[key]
        for key in (
            "binary",
            "explicit_features",
            "package",
            "profile",
            "semantic_argv",
            "target",
        )
    } != {
        "binary": BINARY_NAME,
        "explicit_features": list(SOURCE_SEAL_EXPLICIT_FEATURES),
        "package": "iroha_core",
        "profile": "release",
        "semantic_argv": list(SOURCE_SEAL_SEMANTIC_ARGV),
        "target": SOURCE_SEAL_TARGET,
    }:
        raise CandidateBuildError("authenticated source-seal Cargo policy is not exact")
    unit_graph = _exact_object(
        cargo["unit_graph"],
        {
            "capture_receipt",
            "custom_build_packages",
            "custom_build_units",
            "iroha_core_units",
            "normalization",
            "packages",
            "raw_sha256",
            "raw_size_bytes",
            "sha256",
            "size_bytes",
            "units",
        },
        "projection Cargo unit graph",
    )
    # The signed policy is produced by the separately pinned graph-capable
    # nightly Cargo. Manufacturing a metadata-based approximation here would
    # authenticate a different graph than the candidate build consumes.
    if unit_graph["normalization"] != SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION:
        raise CandidateBuildError("authenticated source-seal unit-graph normalization differs")
    unit_graph_sha256 = _nonzero_lower_hex(
        unit_graph["sha256"], 64, "projection Cargo unit-graph SHA-256"
    )
    unit_graph_size = _bounded_integer(
        unit_graph["size_bytes"], 1, 16 * 1024 * 1024, "projection unit-graph size"
    )
    raw_unit_graph_sha256 = _nonzero_lower_hex(
        unit_graph["raw_sha256"], 64, "projection raw Cargo unit-graph SHA-256"
    )
    raw_unit_graph_size = _bounded_integer(
        unit_graph["raw_size_bytes"],
        1,
        16 * 1024 * 1024,
        "projection raw unit-graph size",
    )
    unit_count = _bounded_integer(
        unit_graph["units"], 1, 100_000, "projection unit count"
    )
    package_count = _bounded_integer(
        unit_graph["packages"], 1, 100_000, "projection package count"
    )
    custom_build_units = _bounded_integer(
        unit_graph["custom_build_units"],
        0,
        unit_count,
        "projection custom-build unit count",
    )
    custom_build_packages = _bounded_integer(
        unit_graph["custom_build_packages"],
        0,
        package_count,
        "projection custom-build package count",
    )
    iroha_core_units = _bounded_integer(
        unit_graph["iroha_core_units"],
        1,
        unit_count,
        "projection iroha_core unit count",
    )
    capture_receipt = _exact_object(
        unit_graph["capture_receipt"],
        SOURCE_SEAL_CAPTURE_RECEIPT_KEYS,
        "projection Cargo capture receipt",
    )
    if capture_receipt["schema"] != SOURCE_SEAL_CAPTURE_RECEIPT_SCHEMA:
        raise CandidateBuildError("projection Cargo capture-receipt schema differs")
    capture_stderr_sha256 = _nonzero_lower_hex(
        capture_receipt["stderr_sha256"],
        64,
        "projection Cargo capture stderr SHA-256",
    )
    capture_stderr_size = _bounded_integer(
        capture_receipt["stderr_size_bytes"],
        0,
        16 * 1024 * 1024,
        "projection Cargo capture stderr size",
    )
    if (
        capture_receipt["exit_status"] != 0
        or capture_receipt["raw_stdout_sha256"] != raw_unit_graph_sha256
        or capture_receipt["raw_stdout_size_bytes"] != raw_unit_graph_size
        or capture_receipt["build_inputs_sha256"] != build_input_sha256
        or capture_receipt["source_commit"] != source_commit
        or capture_receipt["source_tree_sha256"] != source_tree_sha256
        or (capture_stderr_size == 0)
        != (capture_stderr_sha256 == hashlib.sha256(b"").hexdigest())
    ):
        raise CandidateBuildError(
            "projection Cargo capture receipt does not bind the signed graph execution"
        )
    toolchain = _exact_object(
        outer["toolchain"],
        {"cargo", "rustc"},
        "projection build toolchain",
    )
    tool_identities: dict[str, tuple[str, int]] = {}
    for tool in ("cargo", "rustc"):
        identity_value = _exact_object(
            toolchain[tool],
            {"binary_sha256", "binary_size_bytes"},
            f"projection {tool} identity",
        )
        tool_identities[tool] = (
            _nonzero_lower_hex(
                identity_value["binary_sha256"],
                64,
                f"projection {tool} binary SHA-256",
            ),
            _bounded_integer(
                identity_value["binary_size_bytes"],
                1,
                512 * 1024 * 1024,
                f"projection {tool} binary size",
            ),
        )
    if (
        capture_receipt["cargo_binary_sha256"] != tool_identities["cargo"][0]
        or capture_receipt["rustc_binary_sha256"] != tool_identities["rustc"][0]
    ):
        raise CandidateBuildError(
            "projection Cargo capture receipt does not bind the signed toolchain"
        )

    authority = _exact_object(
        projection["source_authority"],
        {
            "commit",
            "commit_object_sha256",
            "commit_object_size",
            "committer_epoch",
            "git_tree",
            "ordered_parents",
            "parent_commit",
            "parent_tree",
            "signature",
        },
        "projection source authority",
    )
    signature = _exact_object(
        authority["signature"],
        {
            "allowed_signers_sha256",
            "mechanism",
            "principal",
            "public_key_sha256",
            "revocation_sha256",
            "signature_namespace",
        },
        "projection source signature",
    )
    commit_object_sha256 = _nonzero_lower_hex(
        actual_authority.commit_object_sha256, 64, "verified commit-object SHA-256"
    )
    commit_object_size = _bounded_integer(
        actual_authority.commit_object_size,
        1,
        4096,
        "verified commit-object size",
    )
    source_git_tree = _nonzero_lower_hex(
        actual_authority.git_tree, 40, "verified source Git tree"
    )
    signer_principal = _portable_identifier(
        actual_authority.signature.principal,
        128,
        "verified source SSH signer principal",
    )
    ssh_public_key_sha256 = _nonzero_lower_hex(
        actual_authority.signature.public_key_sha256,
        64,
        "verified source SSH public-key SHA-256",
    )
    ssh_allowed_signers_sha256 = _nonzero_lower_hex(
        actual_authority.signature.allowed_signers_sha256,
        64,
        "verified source SSH allowed-signers SHA-256",
    )
    ssh_revocation_sha256 = _nonzero_lower_hex(
        actual_authority.signature.revocation_sha256,
        64,
        "verified source SSH revocation SHA-256",
    )
    if (
        actual_authority.commit != source_commit
        or actual_authority.committer_epoch != source_date_epoch
        or actual_authority.ordered_parents != (AUTHORIZED_SOURCE_PARENT_COMMIT,)
        or actual_authority.ordered_parent_trees != (AUTHORIZED_SOURCE_PARENT_TREE,)
    ):
        raise CandidateBuildError(
            "verified HEAD does not have the exact authorized source lineage"
        )
    expected_authority = {
        "commit": source_commit,
        "commit_object_sha256": commit_object_sha256,
        "commit_object_size": commit_object_size,
        "committer_epoch": source_date_epoch,
        "git_tree": source_git_tree,
        "ordered_parents": list(actual_authority.ordered_parents),
        "parent_commit": actual_authority.ordered_parents[0],
        "parent_tree": actual_authority.ordered_parent_trees[0],
        "signature": {
            "allowed_signers_sha256": ssh_allowed_signers_sha256,
            "mechanism": "git-commit-ssh-signature-v1",
            "principal": signer_principal,
            "public_key_sha256": ssh_public_key_sha256,
            "revocation_sha256": ssh_revocation_sha256,
            "signature_namespace": "git",
        },
    }
    if authority != expected_authority or signature != expected_authority["signature"]:
        raise CandidateBuildError(
            "authenticated projection source authority differs from verified HEAD"
        )

    closure_sha256 = _nonzero_lower_hex(
        projection["reviewed_source_closure_sha256"],
        64,
        "projection reviewed source-closure SHA-256",
    )
    closure_hex = projection["reviewed_source_closure_hex"]
    if (
        not isinstance(closure_hex, str)
        or not 2 <= len(closure_hex) <= 8192
        or len(closure_hex) % 2 != 0
        or any(byte not in "0123456789abcdef" for byte in closure_hex)
    ):
        raise CandidateBuildError("projection reviewed source-closure hex is malformed")
    closure_payload = bytes.fromhex(closure_hex)
    expected_closure_payload = _canonical_json_line(identity.reviewed_source_closure)
    if (
        closure_payload != expected_closure_payload
        or hashlib.sha256(closure_payload).hexdigest() != closure_sha256
        or closure_sha256 != identity.reviewed_source_closure_descriptor_sha256
    ):
        raise CandidateBuildError(
            "projection reviewed source closure differs from the pinned source identity"
        )

    return {
        "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_HEX": (
            projection_payload.hex()
        ),
        "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256": (
            projection_sha256
        ),
        "KAGEMUSHA_BUILD_EXECUTION_POLICY_SHA256": execution_policy_sha256,
        "KAGEMUSHA_BUILD_BUILD_INPUTS_HEX": build_input_payload.hex(),
        "KAGEMUSHA_BUILD_BUILD_INPUTS_SHA256": build_input_sha256,
        "KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256": tool_identities["cargo"][0],
        "KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SIZE_BYTES": str(
            tool_identities["cargo"][1]
        ),
        "KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256": tool_identities["rustc"][0],
        "KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SIZE_BYTES": str(
            tool_identities["rustc"][1]
        ),
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_HEX": closure_hex,
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256": closure_sha256,
        "KAGEMUSHA_BUILD_SOURCE_COMMIT": source_commit,
        "KAGEMUSHA_BUILD_SOURCE_COMMIT_OBJECT_SHA256": commit_object_sha256,
        "KAGEMUSHA_BUILD_SOURCE_COMMIT_OBJECT_SIZE": str(commit_object_size),
        "KAGEMUSHA_BUILD_SOURCE_DATE_EPOCH": str(source_date_epoch),
        "KAGEMUSHA_BUILD_SOURCE_GIT_TREE": source_git_tree,
        "KAGEMUSHA_BUILD_SOURCE_PARENT_COMMIT": AUTHORIZED_SOURCE_PARENT_COMMIT,
        "KAGEMUSHA_BUILD_SOURCE_PARENT_TREE": AUTHORIZED_SOURCE_PARENT_TREE,
        "KAGEMUSHA_BUILD_SOURCE_SSH_ALLOWED_SIGNERS_SHA256": (
            ssh_allowed_signers_sha256
        ),
        "KAGEMUSHA_BUILD_SOURCE_SSH_PUBLIC_KEY_SHA256": ssh_public_key_sha256,
        "KAGEMUSHA_BUILD_SOURCE_SSH_REVOCATION_SHA256": ssh_revocation_sha256,
        "KAGEMUSHA_BUILD_SOURCE_SSH_SIGNER_PRINCIPAL": signer_principal,
        "KAGEMUSHA_BUILD_SOURCE_TREE_SHA256": source_tree_sha256,
        "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_PACKAGES": str(
            custom_build_packages
        ),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_CAPTURE_EXIT_STATUS": str(
            capture_receipt["exit_status"]
        ),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_CAPTURE_STDERR_SHA256": capture_stderr_sha256,
        "KAGEMUSHA_BUILD_UNIT_GRAPH_CAPTURE_STDERR_SIZE_BYTES": str(
            capture_stderr_size
        ),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_UNITS": str(custom_build_units),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_IROHA_CORE_UNITS": str(iroha_core_units),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_PACKAGES": str(package_count),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_RAW_SHA256": raw_unit_graph_sha256,
        "KAGEMUSHA_BUILD_UNIT_GRAPH_RAW_SIZE_BYTES": str(raw_unit_graph_size),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_SHA256": unit_graph_sha256,
        "KAGEMUSHA_BUILD_UNIT_GRAPH_SIZE_BYTES": str(unit_graph_size),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_UNITS": str(unit_count),
        "SOURCE_DATE_EPOCH": str(source_date_epoch),
    }


def _read_unit_graph_evidence(
    projection: dict[str, Any],
    *,
    raw_path: Path,
    raw_sha256: str,
    normalized_path: Path,
    normalized_sha256: str,
) -> UnitGraphEvidence:
    """Read both controller artifacts and independently prove raw normalization."""

    pins = (
        (
            raw_path,
            _nonzero_lower_hex(raw_sha256, 64, "raw Cargo unit-graph SHA-256 pin"),
            "raw Cargo unit graph",
        ),
        (
            normalized_path,
            _nonzero_lower_hex(
                normalized_sha256,
                64,
                "normalized Cargo unit-graph SHA-256 pin",
            ),
            "normalized Cargo unit graph",
        ),
    )
    payloads: list[bytes] = []
    for path, expected_sha256, label in pins:
        try:
            payload = source_seal._read_bounded_absolute_file(
                path,
                label,
                16 * 1024 * 1024,
                allow_empty=False,
                owner_controlled=True,
            )
        except source_seal.SourceSealError as error:
            raise CandidateBuildError(str(error)) from error
        if hashlib.sha256(payload).hexdigest() != expected_sha256:
            raise CandidateBuildError(f"{label} differs from its external SHA-256 pin")
        payloads.append(payload)
    raw, normalized = payloads
    policy = projection["outer_policy"]["cargo"]["unit_graph"]
    if (
        hashlib.sha256(raw).hexdigest() != policy["raw_sha256"]
        or len(raw) != policy["raw_size_bytes"]
        or hashlib.sha256(normalized).hexdigest() != policy["sha256"]
        or len(normalized) != policy["size_bytes"]
    ):
        raise CandidateBuildError(
            "controller Cargo unit-graph artifacts differ from the authenticated projection"
        )
    independently_normalized = normalize_source_seal_unit_graph(raw)
    if independently_normalized != normalized:
        raise CandidateBuildError(
            "controller normalized Cargo unit graph differs from its raw capture"
        )
    summary = _unit_graph_summary(normalized)
    expected_summary = {
        key: policy[key]
        for key in (
            "custom_build_packages",
            "custom_build_units",
            "iroha_core_units",
            "normalization",
            "packages",
            "sha256",
            "size_bytes",
            "units",
        )
    }
    if summary != expected_summary:
        raise CandidateBuildError(
            "controller Cargo unit-graph counts differ from the authenticated projection"
        )
    return UnitGraphEvidence(
        raw=raw,
        normalized=normalized,
        summary=summary,
        capture_receipt=dict(policy["capture_receipt"]),
    )


def _validate_unit_graph_preflight_report(
    value: Any, evidence: UnitGraphEvidence
) -> dict[str, int | str]:
    """Require the canonical fresh-launch receipt carried by the build report."""

    report = _exact_object(
        value,
        SOURCE_SEAL_PREFLIGHT_REPORT_KEYS,
        "Cargo unit-graph preflight report",
    )
    capture = evidence.capture_receipt
    expected = {
        "capture_exit_status": capture["exit_status"],
        "capture_stderr_sha256": capture["stderr_sha256"],
        "capture_stderr_size_bytes": capture["stderr_size_bytes"],
        "controller_raw_sha256": hashlib.sha256(evidence.raw).hexdigest(),
        "controller_raw_size_bytes": len(evidence.raw),
        "fresh_exit_status": capture["exit_status"],
        "fresh_stderr_sha256": capture["stderr_sha256"],
        "fresh_stderr_size_bytes": capture["stderr_size_bytes"],
        "normalized_sha256": hashlib.sha256(evidence.normalized).hexdigest(),
        "normalized_size_bytes": len(evidence.normalized),
        **evidence.summary,
    }
    if any(report[key] != item for key, item in expected.items()):
        raise CandidateBuildError(
            "Cargo unit-graph preflight report differs from the authenticated evidence"
        )
    _nonzero_lower_hex(
        report["fresh_raw_sha256"], 64, "fresh Cargo unit-graph SHA-256"
    )
    _bounded_integer(
        report["fresh_raw_size_bytes"],
        1,
        16 * 1024 * 1024,
        "fresh Cargo unit-graph size",
    )
    return report


def _binary_sha256(path: Path, expected_uid: int | None = None) -> tuple[str, int]:
    """Hash one newly built owner-controlled regular executable."""

    if expected_uid is None:
        expected_uid = os.geteuid()
    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != expected_uid
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_mode & stat.S_IXUSR == 0
        or before.st_mode & 0o022 != 0
    ):
        raise CandidateBuildError("sealed candidate binary has unsafe metadata")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        _require_no_extended_metadata_fd(descriptor, "sealed candidate binary")
        digest = hashlib.sha256()
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    current = path.lstat()
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    if not identity(before) == identity(opened) == identity(after) == identity(current):
        raise CandidateBuildError("sealed candidate binary changed while it was hashed")
    return digest.hexdigest(), current.st_size


def _built_binary_from_cargo_messages(
    output: bytes, root: Path, target_dir: Path
) -> Path:
    """Select the one binary artifact Cargo says this build produced."""

    executables: list[Path] = []
    try:
        lines = output.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise CandidateBuildError("Cargo emitted non-UTF-8 build metadata") from error
    for line in lines:
        if not line:
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError as error:
            raise CandidateBuildError("Cargo emitted malformed JSON build metadata") from error
        if message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target")
        if not isinstance(target, dict):
            continue
        kinds = target.get("kind")
        if target.get("name") != BINARY_NAME or not isinstance(kinds, list) or "bin" not in kinds:
            continue
        executable = message.get("executable")
        if not isinstance(executable, str) or not executable:
            continue
        profile = message.get("profile")
        if not isinstance(profile, dict) or profile.get("test") is not False:
            raise CandidateBuildError("Cargo reported a non-release candidate artifact")
        if profile.get("debug_assertions") is not False:
            raise CandidateBuildError(
                "Cargo candidate artifact was compiled with debug assertions"
            )
        candidate = Path(executable)
        if not candidate.is_absolute():
            candidate = root / candidate
        try:
            metadata = candidate.lstat()
        except OSError as error:
            raise CandidateBuildError(
                "Cargo-reported candidate artifact is unavailable"
            ) from error
        if stat.S_ISLNK(metadata.st_mode):
            raise CandidateBuildError(
                "Cargo-reported candidate artifact must not be a symbolic link"
            )
        try:
            resolved = candidate.resolve(strict=True)
        except OSError as error:
            raise CandidateBuildError(
                "Cargo-reported candidate artifact could not be resolved"
            ) from error
        try:
            resolved.relative_to(target_dir)
        except ValueError as error:
            raise CandidateBuildError(
                "Cargo-reported candidate artifact is outside the fixed target directory"
            ) from error
        executables.append(resolved)
    if len(executables) != 1:
        raise CandidateBuildError(
            "Cargo did not report exactly one Kagemusha candidate binary artifact"
        )
    return executables[0]


def _tree_identity_matches(
    observed: hermetic_build.TreeFingerprint, expected: dict[str, Any]
) -> bool:
    """Compare a measured tree to the exact signed JSON representation."""

    return hermetic_build.tree_fingerprint_json(observed) == expected


def _require_bound_tree(
    root: Path,
    expected: dict[str, Any],
    label: str,
    *,
    roots: Sequence[str] | None = None,
    reject_hardlinks: bool = False,
) -> hermetic_build.TreeFingerprint:
    """Measure a tree and require equality with its signed fingerprint."""

    try:
        observed = hermetic_build.bounded_tree_fingerprint(
            root,
            roots,
            reject_hardlinks=reject_hardlinks,
        )
    except (OSError, ValueError) as error:
        raise CandidateBuildError(f"could not measure {label}: {error}") from error
    if not _tree_identity_matches(observed, expected):
        raise CandidateBuildError(f"{label} differs from the signed build-input closure")
    return observed


def _require_root_custodied_tree(root: Path, label: str) -> None:
    """Require root ownership and no delegated writes throughout one host tree."""

    if os.geteuid() != 0:
        raise CandidateBuildError(
            "production input snapshotting must run as root before the sealed build"
        )
    try:
        resolved = root.resolve(strict=True)
    except OSError as error:
        raise CandidateBuildError(f"{label} is unavailable") from error
    if resolved != root or root.is_symlink():
        raise CandidateBuildError(f"{label} must be one canonical non-symlink path")
    for ancestor in reversed((root, *root.parents)):
        metadata = ancestor.lstat()
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or stat.S_ISLNK(metadata.st_mode)
            or metadata.st_uid != 0
            or metadata.st_mode & 0o022
        ):
            raise CandidateBuildError(
                f"{label} ancestry is not exclusively root-custodied: {ancestor}"
            )
    try:
        for directory, directory_names, file_names, directory_fd in os.fwalk(
            root, topdown=True, follow_symlinks=False
        ):
            directory_metadata = os.fstat(directory_fd)
            if directory_metadata.st_uid != 0 or directory_metadata.st_mode & 0o022:
                raise CandidateBuildError(
                    f"{label} contains a delegated-writable directory: {directory}"
                )
            _require_no_extended_metadata_fd(
                directory_fd, f"{label} directory {directory}"
            )
            for name in (*directory_names, *file_names):
                metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                if metadata.st_uid != 0 or metadata.st_mode & 0o022:
                    raise CandidateBuildError(
                        f"{label} contains a non-root or delegated-writable entry: "
                        f"{Path(directory) / name}"
                    )
                entry_path = Path(directory) / name
                if stat.S_ISLNK(metadata.st_mode):
                    _require_no_symlink_xattrs(entry_path, f"{label} entry {entry_path}")
                    continue
                flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(
                    os, "O_NOFOLLOW", 0
                )
                if stat.S_ISDIR(metadata.st_mode):
                    flags |= getattr(os, "O_DIRECTORY", 0)
                descriptor = os.open(name, flags, dir_fd=directory_fd)
                try:
                    opened = os.fstat(descriptor)
                    if not os.path.samestat(metadata, opened):
                        raise CandidateBuildError(
                            f"{label} entry changed during custody inspection: {entry_path}"
                        )
                    _require_no_extended_metadata_fd(
                        descriptor, f"{label} entry {entry_path}"
                    )
                finally:
                    os.close(descriptor)
    except CandidateBuildError:
        raise
    except OSError as error:
        raise CandidateBuildError(f"could not inspect {label} custody") from error


def _write_all(descriptor: int, payload: bytes, label: str) -> None:
    """Write exact bytes to a newly created private descriptor."""

    view = memoryview(payload)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise CandidateBuildError(f"could not finish materializing {label}")
        view = view[written:]


def _make_root_read_only_tree(root: Path) -> None:
    """Expose a root-owned snapshot read-only to a distinct build UID."""

    if os.geteuid() != 0:
        raise CandidateBuildError("only root may seal production input snapshots")
    try:
        for directory, directory_names, file_names, directory_fd in os.fwalk(
            root, topdown=False, follow_symlinks=False
        ):
            for name in file_names:
                metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                if stat.S_ISLNK(metadata.st_mode):
                    continue
                if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != 0:
                    raise CandidateBuildError(
                        f"input snapshot contains an unsafe file: {Path(directory) / name}"
                    )
                os.chmod(
                    name,
                    0o555 if metadata.st_mode & 0o111 else 0o444,
                    dir_fd=directory_fd,
                    follow_symlinks=False,
                )
            for name in directory_names:
                metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                if stat.S_ISLNK(metadata.st_mode):
                    continue
                if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != 0:
                    raise CandidateBuildError(
                        f"input snapshot contains an unsafe directory: {Path(directory) / name}"
                    )
                os.chmod(name, 0o555, dir_fd=directory_fd, follow_symlinks=False)
        os.chmod(root, 0o555)
    except CandidateBuildError:
        raise
    except OSError as error:
        raise CandidateBuildError("could not seal the root-owned input snapshot") from error


def _copy_bound_host_tools(
    build_inputs: dict[str, Any], destination: Path
) -> tuple[AdmittedExecutable, ...]:
    """Copy the exact signed native helper allowlist into a private PATH."""

    destination.mkdir(mode=0o700)
    copied: list[AdmittedExecutable] = []
    for entry in build_inputs["host_tools"]:
        path = Path(entry["path"])
        admitted = _admit_direct_executable(str(path), f"host helper {path}")
        if (
            admitted.resolved_path != Path(entry["resolved_path"])
            or admitted.sha256 != entry["binary_sha256"]
            or admitted.resolved_identity[5] != entry["binary_size_bytes"]
        ):
            raise CandidateBuildError(
                f"host helper {path} differs from the signed build-input closure"
            )
        _require_custodied_directory_ancestry(
            admitted.resolved_path.parent,
            f"host helper {path}",
            allowed_uids=(0,),
        )
        source_fd = os.open(
            admitted.resolved_path,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        target = destination / path.name
        target_fd = os.open(
            target,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o700,
        )
        try:
            source_before = os.fstat(source_fd)
            _require_no_extended_metadata_fd(source_fd, f"host helper {path}")
            while True:
                chunk = os.read(source_fd, 1024 * 1024)
                if not chunk:
                    break
                _write_all(target_fd, chunk, f"host helper {path}")
            os.fsync(target_fd)
            if _tool_identity(source_before) != _tool_identity(os.fstat(source_fd)):
                raise CandidateBuildError(f"host helper changed while copied: {path}")
        finally:
            os.close(target_fd)
            os.close(source_fd)
        _revalidate_admitted_executable(admitted, f"host helper {path}")
        copied_tool = _admit_direct_executable(str(target), f"copied host helper {path}")
        if copied_tool.sha256 != admitted.sha256:
            raise CandidateBuildError(f"copied host helper differs: {path}")
        copied.append(copied_tool)
    _make_root_read_only_tree(destination)
    return tuple(copied)


def _drop_to_runtime_identity(uid: int, gid: int) -> None:
    """Drop every root credential before Cargo or any build script executes."""

    os.umask(0o077)
    os.setgroups([])
    os.setgid(gid)
    os.setuid(uid)


def _require_private_runtime_directory(path: Path, uid: int, gid: int, label: str) -> None:
    """Bind one writable build directory to only the authenticated service identity."""

    descriptor = os.open(
        path,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        metadata = os.fstat(descriptor)
        named = path.lstat()
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or not os.path.samestat(metadata, named)
            or metadata.st_uid != uid
            or metadata.st_gid != gid
            or stat.S_IMODE(metadata.st_mode) != 0o700
        ):
            raise CandidateBuildError(f"{label} is not private to the runtime identity")
        _require_no_extended_metadata_fd(descriptor, label)
    finally:
        os.close(descriptor)


def _require_empty_private_cargo_lock(path: Path, uid: int, gid: int) -> tuple[int, ...]:
    """Require the shared Cargo lock leaf to carry no cross-run bytes."""

    try:
        named = path.lstat()
        descriptor = os.open(
            path,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
    except OSError as error:
        raise CandidateBuildError("Cargo package-cache lock is unavailable") from error
    try:
        opened = os.fstat(descriptor)
        _require_no_extended_metadata_fd(descriptor, "Cargo package-cache lock")
        if (
            not os.path.samestat(named, opened)
            or not stat.S_ISREG(opened.st_mode)
            or opened.st_uid != uid
            or opened.st_gid != gid
            or stat.S_IMODE(opened.st_mode) != 0o600
            or opened.st_nlink != 1
            or opened.st_size != 0
        ):
            raise CandidateBuildError(
                "Cargo package-cache lock is not an empty private single-link file"
            )
        return _tool_identity(opened)
    finally:
        os.close(descriptor)


def _reset_private_cargo_lock(cargo_home: Path, uid: int, gid: int) -> None:
    """Detect lock-file state, then atomically install a fresh empty inode."""

    if os.geteuid() != 0:
        raise CandidateBuildError("only root may reset the Cargo package-cache lock")
    lock_path = cargo_home / ".package-cache"
    before = _require_empty_private_cargo_lock(lock_path, uid, gid)
    replacement = cargo_home / ".package-cache.kagemusha-next"
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(replacement, flags, 0o600)
    except OSError as error:
        raise CandidateBuildError(
            "could not create a fresh Cargo package-cache lock"
        ) from error
    try:
        os.fchown(descriptor, uid, gid)
        os.fchmod(descriptor, 0o600)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    try:
        os.replace(replacement, lock_path)
        directory_fd = os.open(
            cargo_home,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    except BaseException:
        try:
            replacement.unlink()
        except FileNotFoundError:
            pass
        raise
    after = _require_empty_private_cargo_lock(lock_path, uid, gid)
    if after[:2] == before[:2]:
        raise CandidateBuildError("Cargo package-cache lock inode was not replaced")


def _prove_runtime_cannot_mutate_snapshots(
    uid: int, gid: int, roots: Sequence[Path]
) -> None:
    """Use a real child identity to prove snapshot chmod/write attempts fail."""

    read_fd, write_fd = os.pipe()
    child = os.fork()
    if child == 0:
        os.close(read_fd)
        try:
            _drop_to_runtime_identity(uid, gid)
            for root in roots:
                denied = 0
                try:
                    os.chmod(root, 0o700)
                except PermissionError:
                    denied += 1
                try:
                    descriptor = os.open(root / ".hostile-write", os.O_WRONLY | os.O_CREAT, 0o600)
                except PermissionError:
                    denied += 1
                else:
                    os.close(descriptor)
                if denied != 2:
                    raise PermissionError("runtime identity can mutate a sealed input root")
            os.write(write_fd, b"ok")
            os._exit(0)
        except BaseException:
            os._exit(1)
    os.close(write_fd)
    try:
        proof = os.read(read_fd, 2)
    finally:
        os.close(read_fd)
    _, status = os.waitpid(child, 0)
    if proof != b"ok" or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
        raise CandidateBuildError(
            "the unprivileged build identity can mutate a sealed input snapshot"
        )


def _seatbelt_literal(path: Path) -> str:
    """Render one canonical path as a Seatbelt string literal."""

    return json.dumps(str(path), ensure_ascii=True)


def _sealed_build_seatbelt_profile(
    *,
    source_roots: Sequence[Path],
    cargo_home: Path,
    cargo_toolchain: Path,
    rustc_toolchain: Path,
    host_tools: Path,
    developer_dir: Path,
    sdkroot: Path,
    output_dirs: Sequence[Path],
    temporary_dirs: Sequence[Path],
) -> bytes:
    """Construct the fixed network-denying, read/exec/write-scoped build profile."""

    read_roots = (
        *source_roots,
        cargo_home,
        cargo_toolchain,
        rustc_toolchain,
        host_tools,
        developer_dir,
        sdkroot,
        Path("/System/Library"),
        Path("/usr/lib"),
        Path("/private/var/db/dyld"),
        *output_dirs,
        *temporary_dirs,
    )
    exec_roots = (
        cargo_toolchain,
        rustc_toolchain,
        host_tools,
        developer_dir,
        *output_dirs,
    )
    lines = [
        "(version 1)",
        "(deny default)",
        "(deny network*)",
        "(allow process-fork)",
        "(allow signal (target self))",
        "(allow file-read* (literal \"/dev/null\") (literal \"/dev/urandom\"))",
    ]
    lines.extend(
        f"(allow file-read* (subpath {_seatbelt_literal(path)}))"
        for path in read_roots
    )
    lines.extend(
        f"(allow process-exec (subpath {_seatbelt_literal(path)}))"
        for path in exec_roots
    )
    lines.extend(
        f"(allow file-write* (subpath {_seatbelt_literal(path)}))"
        for path in output_dirs
    )
    lines.extend(
        (
            *(
                f"(allow file-write* (subpath {_seatbelt_literal(path)}))"
                for path in temporary_dirs
            ),
            f"(allow file-write* (literal {_seatbelt_literal(cargo_home / '.package-cache')}))",
        )
    )
    return ("\n".join(lines) + "\n").encode("ascii")


def _write_root_read_only_file(path: Path, payload: bytes, label: str) -> None:
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
        _write_all(descriptor, payload, label)
        os.fchmod(descriptor, 0o444)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _verify_signed_host_versions(build_inputs: dict[str, Any]) -> None:
    """Bind the qualified Seatbelt/Xcode receipt to this exact macOS host."""

    try:
        with Path("/System/Library/CoreServices/SystemVersion.plist").open("rb") as source:
            system_version = plistlib.load(source)
        developer_dir = Path(build_inputs["developer_dir"]["path"])
        with (developer_dir.parent / "version.plist").open("rb") as source:
            xcode_version = plistlib.load(source)
    except (OSError, plistlib.InvalidFileException) as error:
        raise CandidateBuildError("could not read the qualified macOS/Xcode versions") from error
    if (
        system_version.get("ProductBuildVersion") != build_inputs["sandbox"]["os_build"]
        or xcode_version.get("ProductBuildVersion")
        != build_inputs["sandbox"]["xcode_build"]
    ):
        raise CandidateBuildError(
            "macOS or Xcode differs from the signed build-sandbox qualification"
        )


def _qualify_seatbelt_launch(
    launch_prefix: Sequence[str],
    source_root: Path,
    environment: dict[str, str],
    drop_privileges: Callable[[], None],
    cargo: AdmittedExecutable,
    rustc: AdmittedExecutable,
    host_tool_bin: Path,
) -> None:
    """Run positive tool probes and hostile ambient read/write/exec/network probes."""

    def run(arguments: Sequence[str]) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [*launch_prefix, *arguments],
            cwd=source_root,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            close_fds=True,
            timeout=30,
            preexec_fn=drop_privileges,
        )

    for tool, expected in (
        (cargo, b"cargo 1.93.0-nightly (6c1b61003 2025-10-28)"),
        (rustc, b"rustc 1.93.1 (01f6ddf75 2026-02-11)"),
    ):
        result = run((tool.command_path, "-Vv"))
        if result.returncode != 0 or result.stdout.splitlines()[:1] != [expected]:
            diagnostic = result.stderr[:4096].decode("utf-8", errors="replace").strip()
            raise CandidateBuildError(
                "qualified Seatbelt cannot execute the signed Rust tool "
                f"{tool.command_path} (status {result.returncode}; stderr={diagnostic!r})"
            )

    bash = str(host_tool_bin / "bash")
    hostile_commands = (
        (bash, "-c", "IFS= read -r _ </etc/hosts"),
        (bash, "-c", ": >/tmp/kagemusha-seatbelt-hostile-write"),
        (bash, "-c", "/usr/bin/true"),
    )
    if any(run(command).returncode == 0 for command in hostile_commands):
        raise CandidateBuildError(
            "qualified Seatbelt allowed an ambient read, write, or unlisted executable"
        )

    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        listener.settimeout(0.2)
        port = listener.getsockname()[1]
        result = run((bash, "-c", f"exec 3<>/dev/tcp/127.0.0.1/{port}"))
        try:
            connection, _ = listener.accept()
        except TimeoutError:
            connection = None
        if connection is not None:
            connection.close()
        if result.returncode == 0 or connection is not None:
            raise CandidateBuildError("qualified Seatbelt allowed a network connection")
    finally:
        listener.close()


def _prove_seatbelt_build_isolation(
    *,
    build_a_launch_prefix: Sequence[str],
    build_b_launch_prefix: Sequence[str],
    build_a_source_root: Path,
    build_b_source_root: Path,
    build_a_environment: dict[str, str],
    build_b_environment: dict[str, str],
    build_a_output_dir: Path,
    build_b_output_dir: Path,
    host_tool_bin: Path,
    drop_privileges: Callable[[], None],
) -> None:
    """Prove that neither sealed build can use its sibling as a data channel."""

    bash = str(host_tool_bin / "bash")
    source_probe = build_a_output_dir / ".seatbelt-build-a-read-probe"
    hostile_write = build_b_output_dir / ".seatbelt-build-a-hostile-write"
    hostile_copy = build_b_output_dir / ".seatbelt-build-b-hostile-copy"
    _write_root_read_only_file(source_probe, b"build-a-private\n", "Seatbelt isolation probe")

    def run(
        launch_prefix: Sequence[str],
        cwd: Path,
        environment: dict[str, str],
        arguments: Sequence[str],
    ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [*launch_prefix, *arguments],
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
            close_fds=True,
            timeout=30,
            preexec_fn=drop_privileges,
        )

    try:
        write_result = run(
            build_a_launch_prefix,
            build_a_source_root,
            build_a_environment,
            (bash, "-c", ': > "$1"', "kagemusha-isolation", str(hostile_write)),
        )
        copy_result = run(
            build_b_launch_prefix,
            build_b_source_root,
            build_b_environment,
            (
                bash,
                "-c",
                'IFS= read -r value < "$1" && printf "%s" "$value" > "$2"',
                "kagemusha-isolation",
                str(source_probe),
                str(hostile_copy),
            ),
        )
        if (
            write_result.returncode == 0
            or copy_result.returncode == 0
            or hostile_write.exists()
            or hostile_copy.exists()
        ):
            raise CandidateBuildError(
                "sealed build Seatbelt profiles do not isolate build A from build B"
            )
    finally:
        for path in (source_probe, hostile_write, hostile_copy):
            try:
                path.unlink()
            except FileNotFoundError:
                pass


def _preflight_unit_graph_launch(
    materialized: MaterializedBuildInputs,
    environment: dict[str, str],
    evidence: UnitGraphEvidence,
) -> dict[str, int | str]:
    """Run nightly Cargo and equal its graph to the signed controller evidence."""

    command = [
        *materialized.unit_graph_launch_prefix,
        materialized.cargo.command_path,
        "-Z",
        "unstable-options",
        "build",
        "--unit-graph",
        "--release",
        "--locked",
        "--offline",
        "--target",
        SOURCE_SEAL_TARGET,
        "--target-dir",
        str(materialized.unit_graph_target_dir),
        "-p",
        "iroha_core",
        "--features",
        ",".join(CANDIDATE_BUILD_FEATURES),
        "--bin",
        BINARY_NAME,
        "--jobs",
        "1",
    ]
    try:
        completed = _run_bounded_cargo_command(
            command,
            cwd=materialized.source_root,
            env=environment,
            drop_privileges=materialized.drop_privileges,
            timeout_seconds=300,
            stdout_max_bytes=16 * 1024 * 1024,
            stderr_max_bytes=16 * 1024 * 1024,
        )
    except (CandidateBuildError, OSError) as error:
        raise CandidateBuildError("could not run the exact Cargo unit-graph preflight") from error
    if (
        completed.returncode != 0
        or not 1 <= len(completed.stdout) <= 16 * 1024 * 1024
        or len(completed.stderr) > 16 * 1024 * 1024
    ):
        raise CandidateBuildError(
            "the exact pinned nightly Cargo cannot emit the required unit graph"
        )
    capture_receipt = evidence.capture_receipt
    fresh_stderr_sha256 = hashlib.sha256(completed.stderr).hexdigest()
    if (
        completed.returncode != capture_receipt["exit_status"]
        or len(completed.stderr) != capture_receipt["stderr_size_bytes"]
        or fresh_stderr_sha256 != capture_receipt["stderr_sha256"]
    ):
        raise CandidateBuildError(
            "fresh pinned-nightly Cargo exit/stderr differ from the signed capture receipt"
        )
    try:
        graph = json.loads(
            completed.stdout,
            object_pairs_hook=_reject_duplicate_json_members,
            parse_constant=_reject_nonfinite_json_number,
        )
    except (CandidateBuildError, json.JSONDecodeError, UnicodeError, ValueError) as error:
        raise CandidateBuildError("Cargo unit-graph preflight emitted invalid JSON") from error
    if (
        not isinstance(graph, dict)
        or set(graph) != {"roots", "units", "version"}
        or graph.get("version") != 1
        or not isinstance(graph.get("roots"), list)
        or not isinstance(graph.get("units"), list)
        or not graph["roots"]
        or not graph["units"]
    ):
        raise CandidateBuildError("Cargo unit-graph preflight shape is not exact V1")
    fresh_normalized = normalize_source_seal_unit_graph(completed.stdout)
    if fresh_normalized != evidence.normalized:
        raise CandidateBuildError(
            "fresh pinned-nightly Cargo unit graph differs from the controller-signed graph"
        )
    fresh_summary = _unit_graph_summary(fresh_normalized)
    if fresh_summary != evidence.summary:
        raise CandidateBuildError(
            "fresh pinned-nightly Cargo unit-graph counts differ from the signed projection"
        )
    return {
        "capture_exit_status": capture_receipt["exit_status"],
        "capture_stderr_sha256": capture_receipt["stderr_sha256"],
        "capture_stderr_size_bytes": capture_receipt["stderr_size_bytes"],
        "controller_raw_sha256": hashlib.sha256(evidence.raw).hexdigest(),
        "controller_raw_size_bytes": len(evidence.raw),
        "fresh_exit_status": completed.returncode,
        "fresh_raw_sha256": hashlib.sha256(completed.stdout).hexdigest(),
        "fresh_raw_size_bytes": len(completed.stdout),
        "fresh_stderr_sha256": fresh_stderr_sha256,
        "fresh_stderr_size_bytes": len(completed.stderr),
        "normalized_sha256": hashlib.sha256(fresh_normalized).hexdigest(),
        "normalized_size_bytes": len(fresh_normalized),
        **fresh_summary,
    }


def _materialize_authenticated_commit_tree(source_root: Path) -> None:
    """Populate a fresh worktree solely from blobs in its snapshotted Git index."""

    try:
        entries = source_seal._index_entries(source_root)
    except source_seal.SourceSealError as error:
        raise CandidateBuildError(
            "could not enumerate the snapshotted authenticated Git index"
        ) from error
    root_fd = os.open(
        source_root,
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        for entry in entries:
            relative = os.fsdecode(entry.path)
            parent_fd, name = hermetic_build._ensure_relative_parent_fd(
                root_fd, relative
            )
            try:
                if entry.mode == b"160000":
                    os.mkdir(name, 0o700, dir_fd=parent_fd)
                    continue
                try:
                    payload = source_seal._git(
                        source_root, "cat-file", "blob", entry.object_id.decode("ascii")
                    )
                except source_seal.SourceSealError as error:
                    raise CandidateBuildError(
                        f"could not read authenticated Git blob for {relative}"
                    ) from error
                actual_object_id = hashlib.sha1(
                    b"blob " + str(len(payload)).encode("ascii") + b"\0" + payload,
                    usedforsecurity=False,
                ).hexdigest().encode("ascii")
                if actual_object_id != entry.object_id:
                    raise CandidateBuildError(
                        f"snapshotted Git blob identity differs for {relative}"
                    )
                if entry.mode == b"120000":
                    os.symlink(os.fsdecode(payload), name, dir_fd=parent_fd)
                    continue
                if entry.mode not in (b"100644", b"100755"):
                    raise CandidateBuildError(
                        f"unsupported authenticated Git mode for {relative}"
                    )
                descriptor = os.open(
                    name,
                    os.O_WRONLY
                    | os.O_CREAT
                    | os.O_EXCL
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                    0o700 if entry.mode == b"100755" else 0o600,
                    dir_fd=parent_fd,
                )
                try:
                    _write_all(descriptor, payload, relative)
                    os.fsync(descriptor)
                finally:
                    os.close(descriptor)
            except FileExistsError as error:
                raise CandidateBuildError(
                    f"authenticated source snapshot path already exists: {relative}"
                ) from error
            finally:
                os.close(parent_fd)
    finally:
        os.close(root_fd)


def _materialize_sealed_build_inputs(
    root: Path,
    target_dir: Path,
    identity: source_seal.SourceIdentity,
    reviewed_source_closure: Path,
    reviewed_source_closure_sha256: str,
    cargo_home: Path,
    cargo: AdmittedExecutable,
    rustc: AdmittedExecutable,
    build_inputs: dict[str, Any],
    identity_reader: Callable[[Path, str, str], source_seal.SourceIdentity],
    runtime_uid: int,
    runtime_gid: int,
) -> MaterializedBuildInputs:
    """Create and verify private, no-replace source/cache/tool snapshots."""

    if os.geteuid() != 0:
        raise CandidateBuildError(
            "production input snapshotting must run as root before the sealed build"
        )
    if (
        type(runtime_uid) is not int
        or type(runtime_gid) is not int
        or not 1 <= runtime_uid <= 2**31 - 1
        or not 1 <= runtime_gid <= 2**31 - 1
    ):
        raise CandidateBuildError(
            "sealed build runtime UID/GID must be non-root positive 31-bit integers"
        )
    if sys.platform != "darwin":
        raise CandidateBuildError(
            "sealed Kagemusha candidate compilation requires the signed darwin host closure"
        )
    snapshot_parent = target_dir / ".kagemusha-sealed-inputs"
    try:
        snapshot_parent.mkdir(mode=0o700)
    except OSError as error:
        raise CandidateBuildError("could not create the private build-input root") from error

    def materialize_source(destination: Path) -> None:
        try:
            hermetic_build.copy_bounded_tree(
                root,
                destination,
                roots=(".git",),
                reject_source_hardlinks=False,
            )
            _materialize_authenticated_commit_tree(destination)
            snapshot_identity = identity_reader(
                destination,
                str(reviewed_source_closure),
                reviewed_source_closure_sha256,
            )
            if snapshot_identity != identity:
                raise CandidateBuildError(
                    "materialized commit source differs from the authenticated source identity"
                )
            _make_root_read_only_tree(destination)
            if (
                identity_reader(
                    destination,
                    str(reviewed_source_closure),
                    reviewed_source_closure_sha256,
                )
                != identity
            ):
                raise CandidateBuildError(
                    "read-only commit snapshot differs from the authenticated source identity"
                )
        except CandidateBuildError:
            raise
        except (OSError, ValueError, source_seal.SourceSealError) as error:
            raise CandidateBuildError(
                f"could not materialize the authenticated commit snapshot: {error}"
            ) from error

    source_snapshot = snapshot_parent / "source-a"
    verification_source_snapshot = snapshot_parent / "source-b"
    materialize_source(source_snapshot)
    materialize_source(verification_source_snapshot)
    _require_root_custodied_tree(source_snapshot, "authenticated source snapshot A")
    _require_root_custodied_tree(
        verification_source_snapshot, "authenticated source snapshot B"
    )

    cargo_roots = tuple(build_inputs["cargo_home"]["roots"])
    cargo_snapshot = snapshot_parent / "cargo-home"
    _require_root_custodied_tree(cargo_home, "Cargo dependency cache")
    try:
        cargo_tree = hermetic_build.copy_bounded_tree(
            cargo_home,
            cargo_snapshot,
            roots=cargo_roots,
            reject_source_hardlinks=False,
        )
    except (OSError, ValueError) as error:
        raise CandidateBuildError(f"could not snapshot the Cargo dependency cache: {error}") from error
    if not _tree_identity_matches(cargo_tree, build_inputs["cargo_home"]["tree"]):
        raise CandidateBuildError(
            "Cargo dependency snapshot differs from the signed build-input closure"
        )
    _make_root_read_only_tree(cargo_snapshot)
    _require_root_custodied_tree(cargo_snapshot, "Cargo dependency snapshot")
    os.chmod(cargo_snapshot, 0o755)
    cargo_lock = cargo_snapshot / ".package-cache"
    cargo_lock_fd = os.open(
        cargo_lock,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    os.close(cargo_lock_fd)
    os.chown(cargo_lock, runtime_uid, runtime_gid)
    os.chmod(cargo_snapshot, 0o555)
    os.chmod(cargo_lock, 0o600)

    cargo_relative = Path(build_inputs["cargo_toolchain"]["cargo_relative_path"])
    rustc_relative = Path(build_inputs["rust_toolchain"]["rustc_relative_path"])
    cargo_toolchain_root = cargo.resolved_path.parent.parent
    rustc_toolchain_root = rustc.resolved_path.parent.parent
    if (
        cargo.resolved_path.relative_to(cargo_toolchain_root) != cargo_relative
        or rustc.resolved_path.relative_to(rustc_toolchain_root) != rustc_relative
    ):
        raise CandidateBuildError("Cargo or rustc does not occupy its signed tool layout")
    _require_root_custodied_tree(cargo_toolchain_root, "Cargo toolchain")
    _require_root_custodied_tree(rustc_toolchain_root, "rustc toolchain")
    cargo_toolchain_snapshot = snapshot_parent / "cargo-toolchain"
    rustc_toolchain_snapshot = snapshot_parent / "rustc-toolchain"
    try:
        cargo_toolchain_tree = hermetic_build.copy_bounded_tree(
            cargo_toolchain_root,
            cargo_toolchain_snapshot,
            reject_source_hardlinks=False,
        )
        rustc_toolchain_tree = hermetic_build.copy_bounded_tree(
            rustc_toolchain_root,
            rustc_toolchain_snapshot,
            reject_source_hardlinks=False,
        )
    except (OSError, ValueError) as error:
        raise CandidateBuildError(f"could not snapshot the Cargo/rustc roots: {error}") from error
    if not _tree_identity_matches(
        cargo_toolchain_tree, build_inputs["cargo_toolchain"]["tree"]
    ):
        raise CandidateBuildError(
            "Cargo toolchain snapshot differs from the signed build-input closure"
        )
    if not _tree_identity_matches(
        rustc_toolchain_tree, build_inputs["rust_toolchain"]["tree"]
    ):
        raise CandidateBuildError(
            "rustc toolchain snapshot differs from the signed build-input closure"
        )
    _make_root_read_only_tree(cargo_toolchain_snapshot)
    _make_root_read_only_tree(rustc_toolchain_snapshot)
    _require_root_custodied_tree(cargo_toolchain_snapshot, "Cargo toolchain snapshot")
    _require_root_custodied_tree(rustc_toolchain_snapshot, "rustc toolchain snapshot")
    copied_cargo = _admit_direct_executable(
        str(cargo_toolchain_snapshot / cargo_relative), "snapshotted Cargo"
    )
    copied_rustc = _admit_direct_executable(
        str(rustc_toolchain_snapshot / rustc_relative), "snapshotted rustc"
    )
    if copied_cargo.sha256 != cargo.sha256 or copied_rustc.sha256 != rustc.sha256:
        raise CandidateBuildError("snapshotted Rust tools differ from their admitted bytes")

    host_specs = [
        (
            Path(build_inputs["developer_dir"]["path"]),
            build_inputs["developer_dir"]["tree"],
            "developer directory",
        ),
        (
            Path(build_inputs["sdkroot"]["path"]),
            build_inputs["sdkroot"]["tree"],
            "SDK root",
        ),
    ]
    developer_dir = host_specs[0][0]
    sdkroot = host_specs[1][0]
    try:
        sdkroot.relative_to(developer_dir)
    except ValueError as error:
        raise CandidateBuildError("signed SDK root is outside the developer directory") from error
    for host_root, expected_tree, label in host_specs:
        _require_root_custodied_tree(host_root, label)
        _require_bound_tree(host_root, expected_tree, label)

    host_tool_bin = snapshot_parent / "host-tools"
    copied_host_tools = _copy_bound_host_tools(build_inputs, host_tool_bin)
    _require_root_custodied_tree(host_tool_bin, "private host helper snapshot")
    copied_ps = next(
        (tool for tool in copied_host_tools if Path(tool.command_path).name == "ps"),
        None,
    )
    if copied_ps is None:
        raise CandidateBuildError("signed host helper closure has no private ps")
    _validate_dedicated_runtime_identity(
        build_inputs,
        runtime_uid,
        runtime_gid,
        Path(copied_ps.command_path),
    )
    output_dir = target_dir / "cargo-output-a"
    output_dir.mkdir(mode=0o700)
    os.chown(output_dir, runtime_uid, runtime_gid)
    verification_output_dir = target_dir / "cargo-output-b"
    verification_output_dir.mkdir(mode=0o700)
    os.chown(verification_output_dir, runtime_uid, runtime_gid)
    unit_graph_output_dir = target_dir / "cargo-unit-graph"
    unit_graph_output_dir.mkdir(mode=0o700)
    os.chown(unit_graph_output_dir, runtime_uid, runtime_gid)
    temporary_dir = target_dir / "build-tmp-a"
    temporary_dir.mkdir(mode=0o700)
    os.chown(temporary_dir, runtime_uid, runtime_gid)
    verification_temporary_dir = target_dir / "build-tmp-b"
    verification_temporary_dir.mkdir(mode=0o700)
    os.chown(verification_temporary_dir, runtime_uid, runtime_gid)
    unit_graph_temporary_dir = target_dir / "unit-graph-tmp"
    unit_graph_temporary_dir.mkdir(mode=0o700)
    os.chown(unit_graph_temporary_dir, runtime_uid, runtime_gid)
    for runtime_path, label in (
        (output_dir, "candidate Cargo output A"),
        (verification_output_dir, "candidate Cargo output B"),
        (unit_graph_output_dir, "Cargo unit-graph output"),
        (temporary_dir, "candidate build temporary A"),
        (verification_temporary_dir, "candidate build temporary B"),
        (unit_graph_temporary_dir, "Cargo unit-graph temporary"),
    ):
        _require_private_runtime_directory(
            runtime_path, runtime_uid, runtime_gid, label
        )
    _verify_signed_host_versions(build_inputs)
    profile_specs = (
        (
            "build-a",
            (source_snapshot,),
            (output_dir,),
            (temporary_dir,),
        ),
        (
            "build-b",
            (verification_source_snapshot,),
            (verification_output_dir,),
            (verification_temporary_dir,),
        ),
        (
            "unit-graph",
            (source_snapshot,),
            (unit_graph_output_dir,),
            (unit_graph_temporary_dir,),
        ),
    )
    sandbox_profiles: dict[str, tuple[Path, bytes]] = {}
    for name, source_roots, output_dirs, temporary_dirs in profile_specs:
        profile_bytes = _sealed_build_seatbelt_profile(
            source_roots=source_roots,
            cargo_home=cargo_snapshot,
            cargo_toolchain=cargo_toolchain_snapshot,
            rustc_toolchain=rustc_toolchain_snapshot,
            host_tools=host_tool_bin,
            developer_dir=developer_dir,
            sdkroot=sdkroot,
            output_dirs=output_dirs,
            temporary_dirs=temporary_dirs,
        )
        sandbox_profile = snapshot_parent / f"sealed-{name}.sb"
        _write_root_read_only_file(
            sandbox_profile, profile_bytes, f"sealed {name} Seatbelt profile"
        )
        sandbox_profiles[name] = (sandbox_profile, profile_bytes)
    os.chmod(snapshot_parent, 0o555)
    os.chmod(target_dir, 0o755)
    _prove_runtime_cannot_mutate_snapshots(
        runtime_uid,
        runtime_gid,
        (
            source_snapshot,
            verification_source_snapshot,
            cargo_snapshot,
            cargo_toolchain_snapshot,
            rustc_toolchain_snapshot,
            host_tool_bin,
        ),
    )
    sandbox_exec = host_tool_bin / "sandbox-exec"
    launch_prefix = (
        str(sandbox_exec),
        "-f",
        str(sandbox_profiles["build-a"][0]),
    )
    verification_launch_prefix = (
        str(sandbox_exec),
        "-f",
        str(sandbox_profiles["build-b"][0]),
    )
    unit_graph_launch_prefix = (
        str(sandbox_exec),
        "-f",
        str(sandbox_profiles["unit-graph"][0]),
    )
    build_a_probe_environment = _sanitized_build_environment(
        cargo_snapshot,
        copied_rustc,
        build_inputs,
        host_tool_bin,
        temporary_dir,
    )
    build_a_probe_environment["CARGO"] = copied_cargo.command_path
    build_b_probe_environment = _sanitized_build_environment(
        cargo_snapshot,
        copied_rustc,
        build_inputs,
        host_tool_bin,
        verification_temporary_dir,
    )
    build_b_probe_environment["CARGO"] = copied_cargo.command_path
    unit_graph_probe_environment = _sanitized_build_environment(
        cargo_snapshot,
        copied_rustc,
        build_inputs,
        host_tool_bin,
        unit_graph_temporary_dir,
    )
    unit_graph_probe_environment["CARGO"] = copied_cargo.command_path
    drop_privileges = lambda: _drop_to_runtime_identity(runtime_uid, runtime_gid)
    for prefix, source_root, probe_environment in (
        (launch_prefix, source_snapshot, build_a_probe_environment),
        (
            verification_launch_prefix,
            verification_source_snapshot,
            build_b_probe_environment,
        ),
        (unit_graph_launch_prefix, source_snapshot, unit_graph_probe_environment),
    ):
        _qualify_seatbelt_launch(
            prefix,
            source_root,
            probe_environment,
            drop_privileges,
            copied_cargo,
            copied_rustc,
            host_tool_bin,
        )
    _prove_seatbelt_build_isolation(
        build_a_launch_prefix=launch_prefix,
        build_b_launch_prefix=verification_launch_prefix,
        build_a_source_root=source_snapshot,
        build_b_source_root=verification_source_snapshot,
        build_a_environment=build_a_probe_environment,
        build_b_environment=build_b_probe_environment,
        build_a_output_dir=output_dir,
        build_b_output_dir=verification_output_dir,
        host_tool_bin=host_tool_bin,
        drop_privileges=drop_privileges,
    )

    def revalidate() -> None:
        for snapshot in (source_snapshot, verification_source_snapshot):
            if (
                identity_reader(
                    snapshot,
                    str(reviewed_source_closure),
                    reviewed_source_closure_sha256,
                )
                != identity
            ):
                raise CandidateBuildError(
                    "authenticated source snapshot changed during the candidate build"
                )
        _require_bound_tree(
            cargo_snapshot,
            build_inputs["cargo_home"]["tree"],
            "Cargo dependency snapshot",
            roots=cargo_roots,
            reject_hardlinks=True,
        )
        _require_bound_tree(
            cargo_toolchain_snapshot,
            build_inputs["cargo_toolchain"]["tree"],
            "Cargo toolchain snapshot",
            reject_hardlinks=True,
        )
        _require_bound_tree(
            rustc_toolchain_snapshot,
            build_inputs["rust_toolchain"]["tree"],
            "rustc toolchain snapshot",
            reject_hardlinks=True,
        )
        _revalidate_admitted_executable(copied_cargo, "snapshotted Cargo")
        _revalidate_admitted_executable(copied_rustc, "snapshotted rustc")
        for index, tool in enumerate(copied_host_tools):
            _revalidate_admitted_executable(tool, f"copied host helper {index}")
        _validate_dedicated_runtime_identity(
            build_inputs,
            runtime_uid,
            runtime_gid,
            Path(copied_ps.command_path),
        )
        for runtime_path, label in (
            (output_dir, "candidate Cargo output A"),
            (verification_output_dir, "candidate Cargo output B"),
            (unit_graph_output_dir, "Cargo unit-graph output"),
            (temporary_dir, "candidate build temporary A"),
            (verification_temporary_dir, "candidate build temporary B"),
            (unit_graph_temporary_dir, "Cargo unit-graph temporary"),
        ):
            _require_private_runtime_directory(
                runtime_path, runtime_uid, runtime_gid, label
            )
        for host_root, expected_tree, label in host_specs:
            _require_root_custodied_tree(host_root, label)
            _require_bound_tree(host_root, expected_tree, label)
        for name, (profile_path, profile_bytes) in sandbox_profiles.items():
            if profile_path.read_bytes() != profile_bytes:
                raise CandidateBuildError(f"sealed {name} Seatbelt profile changed")

    revalidate()
    return MaterializedBuildInputs(
        source_root=source_snapshot,
        verification_source_root=verification_source_snapshot,
        cargo_home=cargo_snapshot,
        cargo=copied_cargo,
        rustc=copied_rustc,
        host_tool_bin=host_tool_bin,
        target_dir=output_dir,
        verification_target_dir=verification_output_dir,
        unit_graph_target_dir=unit_graph_output_dir,
        temporary_dir=temporary_dir,
        verification_temporary_dir=verification_temporary_dir,
        unit_graph_temporary_dir=unit_graph_temporary_dir,
        output_uid=runtime_uid,
        launch_prefix=launch_prefix,
        verification_launch_prefix=verification_launch_prefix,
        unit_graph_launch_prefix=unit_graph_launch_prefix,
        drop_privileges=drop_privileges,
        reset_cargo_lock=lambda: _reset_private_cargo_lock(
            cargo_snapshot, runtime_uid, runtime_gid
        ),
        revalidate=revalidate,
    )


def build_candidate_bundle(
    root: Path,
    cargo: str = "cargo",
    **kwargs: Any,
) -> dict[str, object]:
    """Hold the global authenticated runtime lease for the complete build flow."""

    runtime_uid = kwargs.get("runtime_uid")
    if type(runtime_uid) is not int or not 1 <= runtime_uid <= 2**31 - 1:
        raise CandidateBuildError(
            "sealed build runtime UID must be one non-root positive 31-bit integer"
        )
    runtime_lock = kwargs.pop("runtime_lock", _dedicated_runtime_identity_lock)
    if not callable(runtime_lock):
        raise CandidateBuildError("sealed build runtime lock provider is invalid")
    with runtime_lock(runtime_uid):
        return _build_candidate_bundle_under_runtime_lock(root, cargo, **kwargs)


def _sweep_cargo_process_group(process: subprocess.Popen[bytes]) -> None:
    """Unconditionally terminate every member of one pinned Cargo process group."""

    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    except PermissionError:
        # Darwin reports EPERM when a process group contains only zombies.  A
        # sealed unprivileged leader cannot create a differently-owned member
        # of its private session, so this also proves that no live descendant
        # remains signalable.
        return
    grace_deadline = time.monotonic() + 0.25
    while time.monotonic() < grace_deadline:
        try:
            os.killpg(process.pid, 0)
        except (ProcessLookupError, PermissionError):
            return
        time.sleep(0.01)
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except PermissionError:
        # See the EPERM rationale above.
        pass


class _DarwinSiginfo(ctypes.Structure):
    """Exact Darwin ``siginfo_t`` prefix/layout used by ``waitid``."""

    _fields_ = [
        ("si_signo", ctypes.c_int),
        ("si_errno", ctypes.c_int),
        ("si_code", ctypes.c_int),
        ("si_pid", ctypes.c_int),
        ("si_uid", ctypes.c_uint),
        ("si_status", ctypes.c_int),
        ("si_addr", ctypes.c_void_p),
        ("si_value", ctypes.c_void_p),
        ("si_band", ctypes.c_long),
        ("_reserved", ctypes.c_ulong * 7),
    ]


def _darwin_waitid_wnowait(pid: int) -> bool:
    """Observe a Darwin child exit without releasing its process-group ID."""

    libc = ctypes.CDLL(None, use_errno=True)
    try:
        waitid = libc.waitid
    except AttributeError as error:
        raise CandidateBuildError("sealed Cargo requires Darwin waitid") from error
    waitid.argtypes = [
        ctypes.c_int,
        ctypes.c_uint,
        ctypes.POINTER(_DarwinSiginfo),
        ctypes.c_int,
    ]
    waitid.restype = ctypes.c_int
    info = _DarwinSiginfo()
    ctypes.set_errno(0)
    result = waitid(
        int(os.P_PID),
        pid,
        ctypes.byref(info),
        int(os.WEXITED | os.WNOWAIT | os.WNOHANG),
    )
    if result != 0:
        error_number = ctypes.get_errno()
        if error_number == errno.ECHILD:
            raise CandidateBuildError(
                "sealed Cargo leader was reaped before its process-group sweep"
            )
        raise CandidateBuildError(
            f"could not observe sealed Cargo leader without reaping: errno {error_number}"
        )
    if info.si_pid not in (0, pid):
        raise CandidateBuildError("Darwin waitid returned the wrong Cargo leader")
    return info.si_pid == pid


def _leader_exit_observed_without_reap(pid: int) -> bool:
    """Observe one direct child exit while retaining its zombie as the PGID pin."""

    required_constants = ("P_PID", "WEXITED", "WNOWAIT", "WNOHANG")
    if not all(hasattr(os, name) for name in required_constants):
        raise CandidateBuildError("sealed Cargo requires waitid WNOWAIT support")
    if not hasattr(os, "waitid"):
        if sys.platform == "darwin":
            return _darwin_waitid_wnowait(pid)
        raise CandidateBuildError("sealed Cargo requires waitid WNOWAIT support")
    try:
        result = os.waitid(
            os.P_PID,
            pid,
            os.WEXITED | os.WNOWAIT | os.WNOHANG,
        )
    except ChildProcessError as error:
        raise CandidateBuildError(
            "sealed Cargo leader was reaped before its process-group sweep"
        ) from error
    return result is not None and result.si_pid == pid


def _run_bounded_cargo_command(
    command: Sequence[str],
    *,
    cwd: Path,
    env: dict[str, str],
    drop_privileges: Callable[[], None],
    timeout_seconds: float | None = None,
    stdout_max_bytes: int | None = None,
    stderr_max_bytes: int | None = None,
    pipe_reader: Callable[[int, int], bytes] = os.read,
) -> subprocess.CompletedProcess[bytes]:
    """Run Cargo with finite time/output and kill/reap its private process group."""

    if timeout_seconds is None:
        timeout_seconds = SEALED_CARGO_TIMEOUT_SECONDS
    if stdout_max_bytes is None:
        stdout_max_bytes = SEALED_CARGO_STDOUT_MAX_BYTES
    if stderr_max_bytes is None:
        stderr_max_bytes = SEALED_CARGO_STDERR_MAX_BYTES
    if timeout_seconds <= 0 or stdout_max_bytes <= 0 or stderr_max_bytes <= 0:
        raise CandidateBuildError("sealed Cargo bounds must be positive")
    try:
        process = subprocess.Popen(
            list(command),
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            start_new_session=True,
            preexec_fn=drop_privileges,
        )
    except OSError as error:
        raise CandidateBuildError(f"could not start Cargo: {error}") from error
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    limits = {"stdout": stdout_max_bytes, "stderr": stderr_max_bytes}
    deadline = time.monotonic() + timeout_seconds
    violation: str | None = None
    leader_reaped = False
    try:
        for stream, name in ((process.stdout, "stdout"), (process.stderr, "stderr")):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        while selector.get_map() and violation is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                violation = "wall-clock timeout"
                break
            for key, _events in selector.select(min(remaining, 1.0)):
                try:
                    chunk = pipe_reader(key.fileobj.fileno(), 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                target = buffers[key.data]
                if len(target) + len(chunk) > limits[key.data]:
                    violation = f"{key.data} byte limit"
                    break
                target.extend(chunk)
        while violation is None and not _leader_exit_observed_without_reap(process.pid):
            if time.monotonic() >= deadline:
                violation = "wall-clock timeout"
                break
            time.sleep(0.01)
        _sweep_cargo_process_group(process)
        returncode = process.wait(timeout=10)
        leader_reaped = True
        if violation is not None:
            raise CandidateBuildError(f"sealed Cargo build exceeded its {violation}")
        return subprocess.CompletedProcess(
            list(command),
            returncode,
            stdout=bytes(buffers["stdout"]),
            stderr=bytes(buffers["stderr"]),
        )
    except BaseException:
        if not leader_reaped:
            try:
                _sweep_cargo_process_group(process)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    _sweep_cargo_process_group(process)
                    process.kill()
                    process.wait(timeout=10)
                leader_reaped = True
            except BaseException as cleanup_error:
                raise CandidateBuildError(
                    "could not kill and reap the complete sealed Cargo process group"
                ) from cleanup_error
        raise
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()


def _invoke_sealed_cargo(
    command: Sequence[str],
    *,
    cwd: Path,
    env: dict[str, str],
    drop_privileges: Callable[[], None],
    command_runner: Callable[..., subprocess.CompletedProcess[bytes]] | None,
) -> subprocess.CompletedProcess[bytes]:
    """Use the production bounded runner, retaining dependency injection for tests."""

    if command_runner is None:
        return _run_bounded_cargo_command(
            command, cwd=cwd, env=env, drop_privileges=drop_privileges
        )
    return command_runner(
        list(command),
        cwd=cwd,
        env=env,
        check=False,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        close_fds=True,
        timeout=SEALED_CARGO_TIMEOUT_SECONDS,
        preexec_fn=drop_privileges,
    )


def _build_candidate_bundle_under_runtime_lock(
    root: Path,
    cargo: str = "cargo",
    *,
    target_dir: Path,
    reviewed_source_closure: Path,
    reviewed_source_closure_sha256: str,
    authenticated_source_seal_projection: Path,
    authenticated_source_seal_projection_sha256: str,
    raw_unit_graph: Path,
    raw_unit_graph_sha256: str,
    normalized_unit_graph: Path,
    normalized_unit_graph_sha256: str,
    rustc: str,
    cargo_sha256: str,
    rustc_sha256: str,
    cargo_home: Path,
    runtime_uid: int,
    runtime_gid: int,
    identity_reader: Callable[[Path, str, str], source_seal.SourceIdentity] = (
        source_seal.compute_identity
    ),
    command_runner: Callable[..., subprocess.CompletedProcess[bytes]] | None = None,
    physical_memory_reader: Callable[[], int] = _physical_memory_bytes,
    build_lock: Callable[[], ContextManager[object]] = _shared_memory_heavy_build_lock,
    input_snapshotter: Callable[..., MaterializedBuildInputs] = (
        _materialize_sealed_build_inputs
    ),
    unit_graph_preflight: Callable[
        [MaterializedBuildInputs, dict[str, str], UnitGraphEvidence],
        dict[str, int | str],
    ] = _preflight_unit_graph_launch,
) -> dict[str, object]:
    """Build twice from private authenticated source, cache, tool, and SDK inputs."""

    root = root.resolve(strict=True)
    reviewed_source_closure = reviewed_source_closure.resolve(strict=True)
    physical_memory_bytes = _admitted_physical_memory_bytes(physical_memory_reader)
    cargo_tool = _admit_direct_executable(cargo, "Cargo")
    rustc_tool = _admit_direct_executable(rustc, "rustc")
    _require_executable_digest_pin(cargo_tool, cargo_sha256, "Cargo")
    _require_executable_digest_pin(rustc_tool, rustc_sha256, "rustc")
    cargo_home = _admit_cargo_home(root, cargo_home)
    target_dir = _prepare_fresh_external_target_dir(root, target_dir)
    first = identity_reader(
        root,
        str(reviewed_source_closure),
        reviewed_source_closure_sha256,
    )
    if (
        first.source_repo_dirty
        or first.reviewed_source_closure.get("source_repo_dirty") is not False
    ):
        raise CandidateBuildError(
            "candidate generation requires a clean signed source closure"
        )
    projection_payload, projection, projection_sha256 = (
        _read_authenticated_source_seal_projection(
            authenticated_source_seal_projection,
            authenticated_source_seal_projection_sha256,
        )
    )
    sealed_build_environment = _projection_build_environment(
        projection,
        projection_payload,
        projection_sha256,
        first,
    )
    unit_graph_evidence = _read_unit_graph_evidence(
        projection,
        raw_path=raw_unit_graph,
        raw_sha256=raw_unit_graph_sha256,
        normalized_path=normalized_unit_graph,
        normalized_sha256=normalized_unit_graph_sha256,
    )
    _, build_inputs, _ = _projection_build_input_closure(
        projection["outer_policy"]
    )
    runtime_identity = build_inputs["runtime_identity"]
    if (
        runtime_uid != runtime_identity["uid"]
        or runtime_gid != runtime_identity["gid"]
    ):
        raise CandidateBuildError(
            "requested runtime UID/GID differ from the authenticated build-input closure"
        )
    for tool, label, prefix in (
        (cargo_tool, "Cargo", "CARGO"),
        (rustc_tool, "rustc", "RUSTC"),
    ):
        policy_sha256 = sealed_build_environment[
            f"KAGEMUSHA_BUILD_REVIEWED_{prefix}_BINARY_SHA256"
        ]
        policy_size = int(
            sealed_build_environment[
                f"KAGEMUSHA_BUILD_REVIEWED_{prefix}_BINARY_SIZE_BYTES"
            ]
        )
        if tool.sha256 != policy_sha256 or tool.resolved_identity[5] != policy_size:
            raise CandidateBuildError(
                f"{label} executable differs from the authenticated execution policy"
            )
    materialized = input_snapshotter(
        root,
        target_dir,
        first,
        reviewed_source_closure,
        reviewed_source_closure_sha256,
        cargo_home,
        cargo_tool,
        rustc_tool,
        build_inputs,
        identity_reader,
        runtime_uid,
        runtime_gid,
    )
    environment = _sanitized_build_environment(
        materialized.cargo_home,
        materialized.rustc,
        build_inputs,
        materialized.host_tool_bin,
        materialized.temporary_dir,
    )
    environment.update(sealed_build_environment)
    environment["CARGO"] = materialized.cargo.command_path
    verification_environment = _sanitized_build_environment(
        materialized.cargo_home,
        materialized.rustc,
        build_inputs,
        materialized.host_tool_bin,
        materialized.verification_temporary_dir,
    )
    verification_environment.update(sealed_build_environment)
    verification_environment["CARGO"] = materialized.cargo.command_path
    unit_graph_environment = _sanitized_build_environment(
        materialized.cargo_home,
        materialized.rustc,
        build_inputs,
        materialized.host_tool_bin,
        materialized.unit_graph_temporary_dir,
    )
    unit_graph_environment.update(sealed_build_environment)
    unit_graph_environment["CARGO"] = materialized.cargo.command_path
    cargo_command = [
        materialized.cargo.command_path,
        "build",
        "--release",
        "--locked",
        "--offline",
        "--target",
        SOURCE_SEAL_TARGET,
        "--target-dir",
        str(materialized.target_dir),
        "-p",
        "iroha_core",
        "--features",
        ",".join(CANDIDATE_BUILD_FEATURES),
        "--bin",
        BINARY_NAME,
        "--jobs",
        "1",
        "--message-format=json-render-diagnostics",
    ]
    command = [*materialized.launch_prefix, *cargo_command]
    verification_cargo_command = list(cargo_command)
    verification_cargo_command[
        verification_cargo_command.index("--target-dir") + 1
    ] = str(materialized.verification_target_dir)
    verification_command = [
        *materialized.verification_launch_prefix,
        *verification_cargo_command,
    ]
    with build_lock():
        materialized.revalidate()
        materialized.reset_cargo_lock()
        materialized.revalidate()
        fresh_unit_graph = unit_graph_preflight(
            materialized, unit_graph_environment, unit_graph_evidence
        )
        fresh_unit_graph = _validate_unit_graph_preflight_report(
            fresh_unit_graph, unit_graph_evidence
        )
        materialized.revalidate()
        materialized.reset_cargo_lock()
        materialized.revalidate()
        try:
            completed = _invoke_sealed_cargo(
                command,
                cwd=materialized.source_root,
                env=environment,
                drop_privileges=materialized.drop_privileges,
                command_runner=command_runner,
            )
            if completed.returncode != 0:
                raise CandidateBuildError(
                    "sealed candidate Cargo build failed with status "
                    f"{completed.returncode}"
                )
            materialized.revalidate()
            materialized.reset_cargo_lock()
            materialized.revalidate()
            if not isinstance(completed.stdout, bytes):
                raise CandidateBuildError("Cargo did not return binary build metadata")
            binary = _built_binary_from_cargo_messages(
                completed.stdout,
                materialized.source_root,
                materialized.target_dir,
            )
            sha256, size_bytes = _binary_sha256(binary, materialized.output_uid)
            verification_completed = _invoke_sealed_cargo(
                verification_command,
                cwd=materialized.verification_source_root,
                env=verification_environment,
                drop_privileges=materialized.drop_privileges,
                command_runner=command_runner,
            )
        except OSError as error:
            raise CandidateBuildError(f"could not start Cargo: {error}") from error
    if verification_completed.returncode != 0:
        raise CandidateBuildError(
            "sealed candidate verification build failed with status "
            f"{verification_completed.returncode}"
        )
    materialized.revalidate()
    materialized.reset_cargo_lock()
    materialized.revalidate()
    second = identity_reader(
        root,
        str(reviewed_source_closure),
        reviewed_source_closure_sha256,
    )
    if second != first:
        raise CandidateBuildError("source identity changed during the candidate build")
    second_projection_payload, second_projection, second_projection_sha256 = (
        _read_authenticated_source_seal_projection(
            authenticated_source_seal_projection,
            authenticated_source_seal_projection_sha256,
        )
    )
    if (
        second_projection_payload != projection_payload
        or second_projection_sha256 != projection_sha256
        or _projection_build_environment(
            second_projection,
            second_projection_payload,
            second_projection_sha256,
            second,
        )
        != sealed_build_environment
    ):
        raise CandidateBuildError(
            "authenticated source-seal projection changed during the candidate build"
        )

    if not isinstance(verification_completed.stdout, bytes):
        raise CandidateBuildError(
            "verification Cargo build did not return binary build metadata"
        )
    verification_binary = _built_binary_from_cargo_messages(
        verification_completed.stdout,
        materialized.verification_source_root,
        materialized.verification_target_dir,
    )
    verification_sha256, verification_size_bytes = _binary_sha256(
        verification_binary, materialized.output_uid
    )
    if (
        verification_binary == binary
        or verification_sha256 != sha256
        or verification_size_bytes != size_bytes
        or _binary_sha256(binary, materialized.output_uid) != (sha256, size_bytes)
    ):
        raise CandidateBuildError(
            "two fresh sealed builds did not produce byte-identical candidate binaries"
        )
    third = identity_reader(
        root,
        str(reviewed_source_closure),
        reviewed_source_closure_sha256,
    )
    if third != first:
        raise CandidateBuildError("source identity changed while sealing the candidate binary")
    third_projection_payload, third_projection, third_projection_sha256 = (
        _read_authenticated_source_seal_projection(
            authenticated_source_seal_projection,
            authenticated_source_seal_projection_sha256,
        )
    )
    if (
        third_projection_payload != projection_payload
        or third_projection_sha256 != projection_sha256
        or _projection_build_environment(
            third_projection,
            third_projection_payload,
            third_projection_sha256,
            third,
        )
        != sealed_build_environment
    ):
        raise CandidateBuildError(
            "authenticated source-seal projection changed while sealing the candidate binary"
        )
    materialized.revalidate()
    common_build_identity = {
        "authenticated_source_seal_projection_sha256": projection_sha256,
        "build_inputs_sha256": sealed_build_environment[
            "KAGEMUSHA_BUILD_BUILD_INPUTS_SHA256"
        ],
        "cargo_binary_sha256": cargo_tool.sha256,
        "cargo_semantic_argv": list(SOURCE_SEAL_SEMANTIC_ARGV),
        "execution_policy_sha256": sealed_build_environment[
            "KAGEMUSHA_BUILD_EXECUTION_POLICY_SHA256"
        ],
        "normalized_unit_graph_sha256": unit_graph_evidence.summary["sha256"],
        "reviewed_source_closure_sha256": (
            first.reviewed_source_closure_descriptor_sha256
        ),
        "runtime_gid": runtime_gid,
        "runtime_uid": runtime_uid,
        "rustc_binary_sha256": rustc_tool.sha256,
        "source_commit": first.source_commit,
        "source_date_epoch": projection["source_date_epoch"],
        "source_tree_sha256": first.source_tree_sha256,
        "target": SOURCE_SEAL_TARGET,
    }
    builds: list[dict[str, object]] = []
    for ordinal, source_role, target_role, output_path, output_sha256, output_size in (
        (
            1,
            "authenticated-primary-source-snapshot-v1",
            "fresh-primary-target-v1",
            binary,
            sha256,
            size_bytes,
        ),
        (
            2,
            "authenticated-independent-source-snapshot-v1",
            "fresh-verification-target-v1",
            verification_binary,
            verification_sha256,
            verification_size_bytes,
        ),
    ):
        identity = {
            **common_build_identity,
            "ordinal": ordinal,
            "source_snapshot_role": source_role,
            "target_role": target_role,
        }
        builds.append(
            {
                "identity": identity,
                "identity_sha256": hashlib.sha256(
                    _canonical_json_line(identity)
                ).hexdigest(),
                "output": {
                    "binary_path": str(output_path),
                    "sha256": output_sha256,
                    "size_bytes": output_size,
                },
            }
        )
    return {
        "authenticated_source_seal_projection_sha256": projection_sha256,
        "binary_path": str(binary),
        "binary_sha256": sha256,
        "binary_size_bytes": size_bytes,
        "builds": builds,
        "build_profile": "release",
        "byte_equality": {
            "algorithm": "sha256-size-and-final-descriptor-rehash-v1",
            "equal": True,
            "sha256": sha256,
            "size_bytes": size_bytes,
        },
        "candidate_generator": {
            "selected_build_ordinal": 1,
            "sha256": sha256,
            "size_bytes": size_bytes,
        },
        "reproducible_build_count": 2,
        "reviewed_cargo_binary_sha256": cargo_tool.sha256,
        "minimum_build_physical_memory_bytes": (
            MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES
        ),
        "physical_memory_bytes_at_admission": physical_memory_bytes,
        "schema": SEALED_DOUBLE_BUILD_REPORT_SCHEMA,
        "reviewed_source_closure": first.reviewed_source_closure,
        "reviewed_source_closure_descriptor_sha256": (
            first.reviewed_source_closure_descriptor_sha256
        ),
        "reviewed_rustc_binary_sha256": rustc_tool.sha256,
        "source_commit": first.source_commit,
        "source_date_epoch": projection["source_date_epoch"],
        "source_repo_dirty": first.source_repo_dirty,
        "source_tree_sha256": first.source_tree_sha256,
        "target_dir": str(target_dir),
        "unit_graph_preflight": fresh_unit_graph,
        "verification_binary_path": str(verification_binary),
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    parser.add_argument("--cargo", required=True)
    parser.add_argument("--cargo-sha256", required=True)
    parser.add_argument("--rustc", required=True)
    parser.add_argument("--rustc-sha256", required=True)
    parser.add_argument("--cargo-home", type=Path, required=True)
    parser.add_argument("--runtime-uid", type=int, required=True)
    parser.add_argument("--runtime-gid", type=int, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--reviewed-source-closure", type=Path, required=True)
    parser.add_argument("--reviewed-source-closure-sha256", required=True)
    parser.add_argument(
        "--authenticated-source-seal-projection", type=Path, required=True
    )
    parser.add_argument(
        "--authenticated-source-seal-projection-sha256", required=True
    )
    parser.add_argument("--raw-unit-graph", type=Path, required=True)
    parser.add_argument("--raw-unit-graph-sha256", required=True)
    parser.add_argument("--normalized-unit-graph", type=Path, required=True)
    parser.add_argument("--normalized-unit-graph-sha256", required=True)
    return parser


def _native_builder_digest(contract: str, values: Sequence[str]) -> str:
    """Reproduce the native controller's NUL-framed contract digest."""

    digest = hashlib.sha256()
    digest.update(contract.encode("ascii"))
    digest.update(b"\0")
    for value in values:
        digest.update(value.encode("utf-8"))
        digest.update(b"\0")
    return digest.hexdigest()


def _validate_native_builder_launch(argv: Sequence[str] | None) -> dict[str, str]:
    """Require the inherited receipt from the pre-exec native controller.

    This check deliberately does not claim to authenticate Python: that decision
    happened before exec in the native controller.  It prevents the reviewed CLI
    from accidentally running outside that boundary and binds the child-visible
    arguments/environment back to the controller's receipt.  Only the native
    controller can wrap the resulting inner V1 payload in the promotion-admitted
    V2 report envelope.
    """

    descriptor_text = os.environ.get(NATIVE_SEALED_BUILDER_RECEIPT_FD_ENV, "")
    expected_sha256 = os.environ.get(
        NATIVE_SEALED_BUILDER_RECEIPT_SHA256_ENV, ""
    )
    if descriptor_text != "11" or re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None:
        raise CandidateBuildError(
            "sealed candidate builder requires its native pre-exec launch receipt"
        )
    descriptor = 11
    try:
        metadata = os.fstat(descriptor)
    except OSError as error:
        raise CandidateBuildError(
            "native pre-exec launch receipt descriptor is unavailable"
        ) from error
    if not stat.S_ISFIFO(metadata.st_mode):
        raise CandidateBuildError(
            "native pre-exec launch receipt is not an anonymous pipe"
        )
    payload = bytearray()
    try:
        while len(payload) <= NATIVE_SEALED_BUILDER_RECEIPT_MAX_BYTES:
            chunk = os.read(
                descriptor,
                min(
                    4096,
                    NATIVE_SEALED_BUILDER_RECEIPT_MAX_BYTES + 1 - len(payload),
                ),
            )
            if not chunk:
                break
            payload.extend(chunk)
    except OSError as error:
        raise CandidateBuildError(
            "native pre-exec launch receipt could not be read"
        ) from error
    finally:
        os.close(descriptor)
    receipt_bytes = bytes(payload)
    if (
        not receipt_bytes
        or len(receipt_bytes) > NATIVE_SEALED_BUILDER_RECEIPT_MAX_BYTES
        or hashlib.sha256(receipt_bytes).hexdigest() != expected_sha256
    ):
        raise CandidateBuildError(
            "native pre-exec launch receipt differs from its inherited digest"
        )
    try:
        receipt = json.loads(
            receipt_bytes,
            object_pairs_hook=_reject_duplicate_json_members,
            parse_constant=_reject_nonfinite_json_number,
        )
    except CandidateBuildError:
        raise
    except (UnicodeError, ValueError, json.JSONDecodeError) as error:
        raise CandidateBuildError(
            "native pre-exec launch receipt is not strict JSON"
        ) from error
    if _canonical_json_line(receipt) != receipt_bytes or set(receipt) != {"native_launch"}:
        raise CandidateBuildError(
            "native pre-exec launch receipt is not canonical and exact"
        )
    launch = receipt.get("native_launch")
    exact_fields = {
        "argument_contract",
        "argument_sha256",
        "builder_entrypoint_sha256",
        "contract",
        "controller_sha256",
        "environment_contract",
        "environment_sha256",
        "macos_build",
        "os_tcb_contract",
        "os_tcb_sha256",
        "python_interpreter_sha256",
        "python_runtime_tree_sha256",
        "report_publication_contract",
        "runtime_dependency_contract",
    }
    if (
        not isinstance(launch, dict)
        or set(launch) != exact_fields
        or launch.get("contract") != NATIVE_SEALED_BUILDER_LAUNCH_CONTRACT
        or launch.get("argument_contract")
        != NATIVE_SEALED_BUILDER_ARGUMENT_CONTRACT
        or launch.get("environment_contract")
        != NATIVE_SEALED_BUILDER_ENVIRONMENT_CONTRACT
    ):
        raise CandidateBuildError(
            "native pre-exec launch receipt contract is not exact"
        )
    for field in (
        "argument_sha256",
        "builder_entrypoint_sha256",
        "controller_sha256",
        "environment_sha256",
        "os_tcb_sha256",
        "python_interpreter_sha256",
        "python_runtime_tree_sha256",
    ):
        value = launch.get(field)
        if (
            not isinstance(value, str)
            or re.fullmatch(r"[0-9a-f]{64}", value) is None
            or value == "0" * 64
        ):
            raise CandidateBuildError(
                f"native pre-exec launch receipt {field} is malformed"
            )
    # The native controller receives and hashes the actual flat argv, not a
    # reconstructed parser view.
    actual_argv = list(sys.argv[1:] if argv is None else argv)
    if launch["argument_sha256"] != _native_builder_digest(
        NATIVE_SEALED_BUILDER_ARGUMENT_CONTRACT, actual_argv
    ):
        raise CandidateBuildError(
            "sealed-builder arguments differ from the native launch receipt"
        )
    base_environment = {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "PYTHONDONTWRITEBYTECODE": "1",
        "TMPDIR": "/private/var/tmp",
        "TZ": "UTC",
    }
    if set(os.environ) != set(base_environment) | {
        NATIVE_SEALED_BUILDER_RECEIPT_FD_ENV,
        NATIVE_SEALED_BUILDER_RECEIPT_SHA256_ENV,
        NATIVE_SEALED_BUILDER_ENTRYPOINT_FD_ENV,
        NATIVE_SEALED_BUILDER_REVIEWED_ROOT_ENV,
    } or any(os.environ.get(name) != value for name, value in base_environment.items()):
        raise CandidateBuildError(
            "sealed-builder environment differs from the native launch contract"
        )
    environment_values = [
        value
        for name in sorted(base_environment)
        for value in (name, base_environment[name])
    ]
    if launch["environment_sha256"] != _native_builder_digest(
        NATIVE_SEALED_BUILDER_ENVIRONMENT_CONTRACT, environment_values
    ):
        raise CandidateBuildError(
            "sealed-builder environment differs from the native launch receipt"
        )
    if (
        os.environ.get(NATIVE_SEALED_BUILDER_ENTRYPOINT_FD_ENV) != "12"
        or os.environ.get(NATIVE_SEALED_BUILDER_REVIEWED_ROOT_ENV) != str(REPO_ROOT)
        or __file__ != "/dev/fd/12"
    ):
        raise CandidateBuildError(
            "native builder entrypoint descriptor binding is not exact"
        )
    try:
        interpreter_sha256 = hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest()
        os.lseek(12, 0, os.SEEK_SET)
        builder_digest = hashlib.sha256()
        while True:
            chunk = os.read(12, 64 * 1024)
            if not chunk:
                break
            builder_digest.update(chunk)
        os.lseek(12, 0, os.SEEK_SET)
        builder_sha256 = builder_digest.hexdigest()
    except OSError as error:
        raise CandidateBuildError(
            "native-launched Python or builder entrypoint could not be rechecked"
        ) from error
    if (
        interpreter_sha256 != launch["python_interpreter_sha256"]
        or builder_sha256 != launch["builder_entrypoint_sha256"]
    ):
        raise CandidateBuildError(
            "native-launched Python or builder entrypoint differs from its receipt"
        )
    return launch


def main(argv: Sequence[str] | None = None) -> int:
    """Build and print canonical JSON identifying the exact resulting binary."""

    try:
        _validate_native_builder_launch(argv)
    except CandidateBuildError as error:
        print(f"sealed Kagemusha candidate build failed: {error}", file=sys.stderr)
        return 1
    args = _parser().parse_args(argv)
    try:
        report = build_candidate_bundle(
            args.root,
            args.cargo,
            target_dir=args.target_dir,
            reviewed_source_closure=args.reviewed_source_closure,
            reviewed_source_closure_sha256=args.reviewed_source_closure_sha256,
            authenticated_source_seal_projection=(
                args.authenticated_source_seal_projection
            ),
            authenticated_source_seal_projection_sha256=(
                args.authenticated_source_seal_projection_sha256
            ),
            raw_unit_graph=args.raw_unit_graph,
            raw_unit_graph_sha256=args.raw_unit_graph_sha256,
            normalized_unit_graph=args.normalized_unit_graph,
            normalized_unit_graph_sha256=args.normalized_unit_graph_sha256,
            rustc=args.rustc,
            cargo_sha256=args.cargo_sha256,
            rustc_sha256=args.rustc_sha256,
            cargo_home=args.cargo_home,
            runtime_uid=args.runtime_uid,
            runtime_gid=args.runtime_gid,
        )
    except (CandidateBuildError, OSError, source_seal.SourceSealError) as error:
        print(f"sealed Kagemusha candidate build failed: {error}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(_canonical_json_line(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
