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
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
from typing import Any, Callable, ContextManager, Iterator, Sequence

REPO_ROOT = Path(__file__).resolve().parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts import kagemusha_source_tree_seal as source_seal
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
# and rustc themselves. The authenticated projection v1 does not carry
# identities for a C/C++ compiler, archiver, linker, SDK, package-config root,
# or compiler cache, so none of those controls may survive into this build.
# Any future exception must first become an exact, content-pinned member of the
# authenticated projection/tool contract; it must never be added as an ambient
# allow-list entry here.
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
    try:
        requested.relative_to(root)
    except ValueError:
        pass
    else:
        raise CandidateBuildError("target directory must be outside the source repository")
    if requested.exists() or requested.is_symlink():
        raise CandidateBuildError("target directory must be a fresh nonexistent path")
    try:
        requested.mkdir(mode=0o700)
        target_dir = requested.resolve(strict=True)
        metadata = requested.lstat()
    except OSError as error:
        raise CandidateBuildError("could not create fresh external target directory") from error
    if (
        target_dir != requested
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077 != 0
        or any(requested.iterdir())
    ):
        raise CandidateBuildError("fresh external target directory is not exact and empty")
    return target_dir


def _is_ambient_native_toolchain_control(name: str) -> bool:
    """Return whether an environment name can redirect a native build helper."""

    normalized = name.upper()
    return (
        normalized in _REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT
        or normalized.startswith(_REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT_PREFIXES)
        or normalized.endswith(_REMOVED_NATIVE_TOOLCHAIN_ENVIRONMENT_SUFFIXES)
    )


def _sanitized_build_environment(cargo_home: Path, rustc: AdmittedExecutable) -> dict[str, str]:
    """Remove ambient Rust and unauthenticated native-toolchain controls."""

    environment = {
        key: value
        for key, value in os.environ.items()
        if key not in _REMOVED_BUILD_ENVIRONMENT
        and not key.startswith(_REMOVED_BUILD_ENVIRONMENT_PREFIXES)
        and not _is_ambient_native_toolchain_control(key)
    }
    # Empty wrapper/flag values override user Cargo configuration as well as
    # inherited environment settings without selecting another executable.
    environment.update(
        {
            "CARGO_HOME": str(cargo_home),
            "CARGO_NET_OFFLINE": "true",
            "CARGO_ENCODED_RUSTFLAGS": "",
            "HOME": "/var/empty",
            "PATH": "/usr/bin:/bin",
            "RUSTC": rustc.command_path,
            "RUSTC_WRAPPER": "",
            "RUSTC_WORKSPACE_WRAPPER": "",
            "RUSTFLAGS": "",
        }
    )
    if any(_is_ambient_native_toolchain_control(key) for key in environment):
        raise CandidateBuildError(
            "unauthenticated native-toolchain control survived environment sanitization"
        )
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
        {"cargo", "execution_policy_sha256", "schema"},
        "projection outer policy",
    )
    if outer["schema"] != SOURCE_SEAL_OUTER_POLICY_SCHEMA:
        raise CandidateBuildError("authenticated source-seal outer policy schema differs")
    execution_policy_sha256 = _nonzero_lower_hex(
        outer["execution_policy_sha256"], 64, "projection execution-policy SHA-256"
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
            "custom_build_packages",
            "custom_build_units",
            "iroha_core_units",
            "normalization",
            "packages",
            "sha256",
            "size_bytes",
            "units",
        },
        "projection Cargo unit graph",
    )
    # TODO: Recompute this normalized unit graph when the sealed stable Cargo
    # toolchain exposes a supported unit-graph interface. Cargo 1.93 still
    # gates `--unit-graph` behind unstable options, so manufacturing a
    # metadata-based approximation here would authenticate the wrong graph.
    if unit_graph["normalization"] != SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION:
        raise CandidateBuildError("authenticated source-seal unit-graph normalization differs")
    unit_graph_sha256 = _nonzero_lower_hex(
        unit_graph["sha256"], 64, "projection Cargo unit-graph SHA-256"
    )
    unit_graph_size = _bounded_integer(
        unit_graph["size_bytes"], 1, 16 * 1024 * 1024, "projection unit-graph size"
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
        "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_UNITS": str(custom_build_units),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_IROHA_CORE_UNITS": str(iroha_core_units),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_PACKAGES": str(package_count),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_SHA256": unit_graph_sha256,
        "KAGEMUSHA_BUILD_UNIT_GRAPH_SIZE_BYTES": str(unit_graph_size),
        "KAGEMUSHA_BUILD_UNIT_GRAPH_UNITS": str(unit_count),
        "SOURCE_DATE_EPOCH": str(source_date_epoch),
    }


def _binary_sha256(path: Path) -> tuple[str, int]:
    """Hash one newly built owner-controlled regular executable."""

    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
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


def build_candidate_bundle(
    root: Path,
    cargo: str = "cargo",
    *,
    target_dir: Path,
    reviewed_source_closure: Path,
    reviewed_source_closure_sha256: str,
    authenticated_source_seal_projection: Path,
    authenticated_source_seal_projection_sha256: str,
    rustc: str,
    cargo_sha256: str,
    rustc_sha256: str,
    cargo_home: Path,
    identity_reader: Callable[[Path, str, str], source_seal.SourceIdentity] = (
        source_seal.compute_identity
    ),
    command_runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run,
    physical_memory_reader: Callable[[], int] = _physical_memory_bytes,
    build_lock: Callable[[], ContextManager[object]] = _shared_memory_heavy_build_lock,
) -> dict[str, object]:
    """Build once and prove the reviewed source and Cargo/rustc binaries stayed exact."""

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
    environment = _sanitized_build_environment(cargo_home, rustc_tool)
    environment.update(sealed_build_environment)
    environment.update(
        {
            "CARGO": cargo_tool.command_path,
            "KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256": cargo_tool.sha256,
            "KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256": rustc_tool.sha256,
        }
    )
    command = [
        cargo_tool.command_path,
        "build",
        "--release",
        "--locked",
        "--target-dir",
        str(target_dir),
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
    with build_lock():
        _revalidate_admitted_executable(cargo_tool, "Cargo")
        _revalidate_admitted_executable(rustc_tool, "rustc")
        try:
            completed = command_runner(
                command,
                cwd=root,
                env=environment,
                check=False,
                stdout=subprocess.PIPE,
            )
        except OSError as error:
            raise CandidateBuildError(f"could not start Cargo: {error}") from error
    if completed.returncode != 0:
        raise CandidateBuildError(
            f"sealed candidate Cargo build failed with status {completed.returncode}"
        )
    _revalidate_admitted_executable(cargo_tool, "Cargo")
    _revalidate_admitted_executable(rustc_tool, "rustc")
    if _admit_cargo_home(root, cargo_home) != cargo_home:
        raise CandidateBuildError("Cargo home changed during the candidate build")
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

    if not isinstance(completed.stdout, bytes):
        raise CandidateBuildError("Cargo did not return binary build metadata")
    binary = _built_binary_from_cargo_messages(completed.stdout, root, target_dir)
    sha256, size_bytes = _binary_sha256(binary)
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
    _revalidate_admitted_executable(cargo_tool, "Cargo")
    _revalidate_admitted_executable(rustc_tool, "rustc")
    if _admit_cargo_home(root, cargo_home) != cargo_home:
        raise CandidateBuildError("Cargo home changed while sealing the candidate binary")
    return {
        "authenticated_source_seal_projection_sha256": projection_sha256,
        "binary_path": str(binary),
        "binary_sha256": sha256,
        "binary_size_bytes": size_bytes,
        "build_profile": "release",
        "reviewed_cargo_binary_sha256": cargo_tool.sha256,
        "minimum_build_physical_memory_bytes": (
            MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES
        ),
        "physical_memory_bytes_at_admission": physical_memory_bytes,
        "schema": "iroha.kagemusha.sealed_candidate_build.v1",
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
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    parser.add_argument("--cargo", required=True)
    parser.add_argument("--cargo-sha256", required=True)
    parser.add_argument("--rustc", required=True)
    parser.add_argument("--rustc-sha256", required=True)
    parser.add_argument("--cargo-home", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--reviewed-source-closure", type=Path, required=True)
    parser.add_argument("--reviewed-source-closure-sha256", required=True)
    parser.add_argument(
        "--authenticated-source-seal-projection", type=Path, required=True
    )
    parser.add_argument(
        "--authenticated-source-seal-projection-sha256", required=True
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Build and print canonical JSON identifying the exact resulting binary."""

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
            rustc=args.rustc,
            cargo_sha256=args.cargo_sha256,
            rustc_sha256=args.rustc_sha256,
            cargo_home=args.cargo_home,
        )
    except (CandidateBuildError, OSError, source_seal.SourceSealError) as error:
        print(f"sealed Kagemusha candidate build failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, ensure_ascii=True, separators=(",", ":"), sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
