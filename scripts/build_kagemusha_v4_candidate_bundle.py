#!/usr/bin/env python3
"""Build Kagemusha V4 from one root-published immutable production closure.

The privileged provisioner is intentionally separate. This consumer admits a
content-addressed, root-owned, entirely non-writable closure containing the
reviewed source, bootstrap, Rust sysroot/dynamic dependencies, GPG/keyring, and
canonical provenance, then builds into one fresh external target directory.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import json
import os
import pathlib
from pathlib import Path
import re
import stat
import subprocess
import sys
import types
from typing import Callable, ContextManager, Iterator, Sequence

REPO_ROOT = Path(__file__).resolve().parent.parent


def _load_repo_source_module(module_name: str, path: Path) -> types.ModuleType:
    """Compile stable source bytes directly, never consulting checkout bytecode."""

    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_size <= 0
        or before.st_size > 16 * 1024 * 1024
    ):
        raise RuntimeError(f"{path} is not a bounded regular source file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(os.fsencode(path), flags)
    try:
        opened_before = os.fstat(descriptor)
        chunks = bytearray()
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.extend(chunk)
            if len(chunks) > 16 * 1024 * 1024:
                raise RuntimeError(f"{path} exceeds the source-module size limit")
        opened_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = path.lstat()
    stable = lambda item: (
        item.st_dev,
        item.st_ino,
        item.st_mode,
        item.st_uid,
        item.st_nlink,
        item.st_size,
        item.st_mtime_ns,
        item.st_ctime_ns,
    )
    if not (
        stable(before)
        == stable(opened_before)
        == stable(opened_after)
        == stable(after)
    ):
        raise RuntimeError(f"{path} changed while its source bytes were captured")
    module = types.ModuleType(module_name)
    module.__file__ = str(path)
    module.__package__ = module_name.rpartition(".")[0]
    sys.modules[module_name] = module
    try:
        exec(compile(bytes(chunks), str(path), "exec"), module.__dict__)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


source_seal = _load_repo_source_module(
    "_kagemusha_source_tree_seal_source",
    REPO_ROOT / "scripts/kagemusha_source_tree_seal.py",
)
resource_guard = _load_repo_source_module(
    "_kagemusha_resource_guard_source",
    REPO_ROOT / "scripts/formal/run_sumeragi_v2_tlapm_guard.py",
)


BINARY_NAME = "kagemusha_recursive_spend_v4_bundle"
SEALED_FEATURE = "kagemusha-candidate-source-seal"
MATERIALIZED_SOURCE_DIR_NAME = ".kagemusha-reviewed-source"
MATERIALIZED_DESCRIPTOR_NAME = ".kagemusha-reviewed-source-closure.json"
SEALED_SOURCE_WORKSPACE_NAME = ".kagemusha-sealed-source-boundary"
PRODUCTION_CLOSURE_SCHEMA = "iroha.kagemusha.production_build_closure.v1"
PRODUCTION_PROVISIONING_PROTOCOL = (
    "iroha.kagemusha.root_private_atomic_publish.v1"
)
PRODUCTION_PROVENANCE_FILE_NAME = "production-build-closure.json"
PRODUCTION_CLOSURE_TREE_DOMAIN = b"iroha.kagemusha.production-build-closure.v1\0"
MAX_PRODUCTION_CLOSURE_ENTRIES = 1_000_000
# The measured single-rustc frontend high-water mark is about 11.466 GiB.
# Requiring 24 GiB of installed physical memory leaves slightly more than a
# two-times margin.  This is only build admission: it neither reduces compiler
# memory nor imposes a hard RSS limit once Cargo starts.
MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES = 24 * 1024 * 1024 * 1024
PRODUCTION_BUILD_USER_NAME = "boi-build"


class CandidateBuildError(RuntimeError):
    """A sealed candidate generator could not be built unambiguously."""


@dataclass(frozen=True)
class AdmittedRustToolchain:
    provenance: Path
    closure_root: Path
    closure_tree_sha256: str
    source_root: Path
    reviewed_source_closure: Path
    apple_developer_dir: Path
    apple_sdk: Path
    clang_resource_dir: Path
    cargo_home: Path
    cargo_vendor: Path
    cargo: Path
    rustc: Path
    rustc_sysroot: Path
    linker: Path
    git: Path
    git_exec_path: Path
    gpg: Path
    gnupghome: Path
    python: Path
    cargo_sha256: str
    rustc_sha256: str
    linker_sha256: str
    git_sha256: str
    gpg_sha256: str
    python_sha256: str
    source_signing_key_fingerprint: str
    provenance_sha256: str


def _stable_regular_sha256(path: Path) -> tuple[str, int]:
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_size < 0:
        raise CandidateBuildError("production closure file has unsafe metadata")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(os.fsencode(path), flags)
    try:
        opened_before = os.fstat(descriptor)
        digest = hashlib.sha256()
        size = 0
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
        opened_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = path.lstat()
    stable = lambda item: (
        item.st_dev,
        item.st_ino,
        item.st_mode,
        item.st_uid,
        item.st_gid,
        item.st_nlink,
        item.st_size,
        item.st_mtime_ns,
        item.st_ctime_ns,
    )
    if not (
        stable(before)
        == stable(opened_before)
        == stable(opened_after)
        == stable(after)
        and size == before.st_size
    ):
        raise CandidateBuildError("production closure file changed while hashed")
    return digest.hexdigest(), size


def _production_closure_tree_sha256(
    closure_root: Path,
    provenance_path: Path,
    *,
    trusted_owner_uid: int = 0,
    validate_parent_chain: bool = True,
) -> str:
    """Hash and validate one root-published, entirely non-writable closure tree."""

    try:
        root_metadata = closure_root.lstat()
        canonical_root = closure_root.resolve(strict=True)
        excluded = provenance_path.relative_to(closure_root)
    except (OSError, ValueError) as error:
        raise CandidateBuildError(
            "production closure root/provenance relationship is invalid"
        ) from error
    if (
        canonical_root != closure_root
        or not stat.S_ISDIR(root_metadata.st_mode)
        or root_metadata.st_uid != trusted_owner_uid
        or root_metadata.st_mode & 0o222 != 0
        or root_metadata.st_mode & 0o7000 != 0
    ):
        raise CandidateBuildError(
            "production closure root is not canonical, trusted-owner, and read-only"
        )
    if validate_parent_chain:
        ancestor = closure_root.parent
        while True:
            metadata = ancestor.lstat()
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or metadata.st_uid != trusted_owner_uid
                or metadata.st_mode & 0o022 != 0
                or ancestor.resolve(strict=True) != ancestor
            ):
                raise CandidateBuildError(
                    "production closure parent chain is not root-controlled"
                )
            if ancestor == ancestor.parent:
                break
            ancestor = ancestor.parent

    digest = hashlib.sha256()
    digest.update(PRODUCTION_CLOSURE_TREE_DOMAIN)
    entries_seen = 0
    excluded_seen = False

    def frame(payload: bytes) -> None:
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)

    stack: list[tuple[Path, bytes]] = [(closure_root, b"")]
    while stack:
        directory, relative_directory = stack.pop()
        try:
            entries = sorted(
                os.scandir(os.fsencode(directory)),
                key=lambda entry: entry.name,
                reverse=True,
            )
        except OSError as error:
            raise CandidateBuildError(
                "production closure directory could not be enumerated"
            ) from error
        for entry in entries:
            entries_seen += 1
            if entries_seen > MAX_PRODUCTION_CLOSURE_ENTRIES:
                raise CandidateBuildError("production closure inventory is oversized")
            name = entry.name
            assert isinstance(name, bytes)
            relative = name if not relative_directory else relative_directory + b"/" + name
            path = Path(os.fsdecode(entry.path))
            if Path(os.fsdecode(relative)) == excluded:
                excluded_seen = True
                continue
            metadata = entry.stat(follow_symlinks=False)
            if metadata.st_uid != trusted_owner_uid or metadata.st_mode & 0o7000 != 0:
                raise CandidateBuildError(
                    "production closure entry is not owned by the trusted provisioner"
                )
            mode = stat.S_IMODE(metadata.st_mode)
            if stat.S_ISDIR(metadata.st_mode):
                if mode & 0o222 != 0:
                    raise CandidateBuildError(
                        "production closure directory remains writable"
                    )
                digest.update(b"D")
                frame(relative)
                digest.update(mode.to_bytes(4, "big"))
                stack.append((path, relative))
            elif stat.S_ISREG(metadata.st_mode):
                if mode & 0o222 != 0 or metadata.st_nlink != 1:
                    raise CandidateBuildError(
                        "production closure file remains writable or multiply linked"
                    )
                file_sha256, size = _stable_regular_sha256(path)
                digest.update(b"F")
                frame(relative)
                digest.update(mode.to_bytes(4, "big"))
                digest.update(size.to_bytes(8, "big"))
                digest.update(bytes.fromhex(file_sha256))
            elif stat.S_ISLNK(metadata.st_mode):
                target = os.readlink(entry.path)
                assert isinstance(target, bytes)
                if target.startswith(b"/"):
                    raise CandidateBuildError(
                        "production closure symbolic link is absolute"
                    )
                lexical = pathlib.PurePosixPath(
                    os.fsdecode(relative_directory)
                ).joinpath(os.fsdecode(target))
                parts: list[str] = []
                for part in lexical.parts:
                    if part in ("", "."):
                        continue
                    if part == "..":
                        if not parts:
                            raise CandidateBuildError(
                                "production closure symbolic link escapes"
                            )
                        parts.pop()
                    else:
                        parts.append(part)
                digest.update(b"L")
                frame(relative)
                frame(target)
            else:
                raise CandidateBuildError(
                    "production closure contains a special filesystem entry"
                )
    if not excluded_seen:
        raise CandidateBuildError(
            "production closure provenance is not the one excluded tree entry"
        )
    return digest.hexdigest()


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


def _admitted_build_worker_identity() -> tuple[str, int]:
    """Bind the provisional build report to the supervisor-selected build UID."""

    user_name = os.environ.get("KAGEMUSHA_BUILD_USER_NAME", "")
    uid_text = os.environ.get("KAGEMUSHA_BUILD_UID", "")
    if (
        user_name != PRODUCTION_BUILD_USER_NAME
        or not uid_text.isascii()
        or not uid_text.isdigit()
        or (uid := int(uid_text)) < 1
        or uid != os.geteuid()
    ):
        raise CandidateBuildError(
            "production build must run under the supervisor-selected "
            "non-root boi-build UID"
        )
    return user_name, uid


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


def _canonical_offline_cargo_config(
    cargo_home: Path,
    cargo_vendor: Path,
) -> bytes:
    try:
        resolved_vendor = (cargo_home / "../cargo-vendor").resolve(strict=True)
    except OSError as error:
        raise CandidateBuildError(
            "production Cargo vendor directory is unavailable"
        ) from error
    if resolved_vendor != cargo_vendor:
        raise CandidateBuildError(
            "production Cargo vendor must be the canonical sibling of Cargo home"
        )
    return (
        "[net]\n"
        "offline = true\n\n"
        "[source.crates-io]\n"
        'replace-with = "vendored-sources"\n\n'
        "[source.vendored-sources]\n"
        'directory = "../cargo-vendor"\n'
    ).encode("ascii")


def _admitted_production_rust_toolchain(
    cargo: str,
    provenance_path: Path | None,
    provenance_sha256: str | None,
) -> AdmittedRustToolchain:
    """Admit one independently pinned, root-published production build closure."""

    if cargo != "cargo":
        raise CandidateBuildError(
            "sealed production builds do not accept a caller-selected Cargo executable"
        )
    if provenance_path is None or provenance_sha256 is None:
        raise CandidateBuildError(
            "sealed production builds require independently pinned closure provenance"
        )
    if (
        not provenance_path.is_absolute()
        or os.path.normpath(os.fspath(provenance_path)) != os.fspath(provenance_path)
        or re.fullmatch(r"[0-9a-f]{64}", provenance_sha256) is None
    ):
        raise CandidateBuildError("production closure provenance path or digest is malformed")
    try:
        resolved_provenance_path = provenance_path.resolve(strict=True)
    except OSError as error:
        raise CandidateBuildError("production closure provenance is unavailable") from error
    if resolved_provenance_path != provenance_path:
        raise CandidateBuildError("production closure provenance path is not canonical")
    before = provenance_path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != 0
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > 64 * 1024
        or before.st_mode & 0o222 != 0
        or before.st_mode & 0o7000 != 0
    ):
        raise CandidateBuildError("production closure provenance has unsafe metadata")
    payload_digest, _ = _stable_regular_sha256(provenance_path)
    payload = provenance_path.read_bytes()
    after = provenance_path.lstat()
    if (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_uid,
        before.st_nlink,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    ) != (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_uid,
        after.st_nlink,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ) or payload_digest != provenance_sha256 or hashlib.sha256(payload).hexdigest() != provenance_sha256:
        raise CandidateBuildError(
            "production closure provenance changed or differs from its pin"
        )
    def object_pairs(
        pairs: list[tuple[str, object]],
    ) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, item in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = item
        return result

    try:
        value = json.loads(payload, object_pairs_hook=object_pairs)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise CandidateBuildError("production closure provenance is not strict JSON") from error
    except ValueError as error:
        raise CandidateBuildError(
            "production closure provenance has duplicate JSON keys"
        ) from error
    expected_keys = {
        "apple_developer_dir_path",
        "apple_sdk_path",
        "cargo_path",
        "cargo_sha256",
        "cargo_home_path",
        "cargo_vendor_path",
        "closure_root",
        "closure_tree_sha256",
        "clang_resource_dir_path",
        "gnupghome_path",
        "gpg_path",
        "gpg_sha256",
        "git_exec_path",
        "git_path",
        "git_sha256",
        "linker_path",
        "linker_sha256",
        "provisioning_protocol",
        "python_path",
        "python_sha256",
        "reviewed_source_closure_path",
        "rustc_path",
        "rustc_sha256",
        "rustc_sysroot_path",
        "schema",
        "source_root",
        "source_signing_key_fingerprint",
    }
    canonical = (
        json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        + "\n"
    ).encode("ascii")
    if (
        not isinstance(value, dict)
        or set(value) != expected_keys
        or value.get("schema") != PRODUCTION_CLOSURE_SCHEMA
        or value.get("provisioning_protocol") != PRODUCTION_PROVISIONING_PROTOCOL
        or canonical != payload
    ):
        raise CandidateBuildError("production closure provenance is not canonical")

    raw_closure_root = value["closure_root"]
    expected_tree_sha256 = value["closure_tree_sha256"]
    if (
        not isinstance(raw_closure_root, str)
        or not isinstance(expected_tree_sha256, str)
        or re.fullmatch(r"[0-9a-f]{64}", expected_tree_sha256) is None
    ):
        raise CandidateBuildError("production closure identity fields are malformed")
    closure_root = Path(raw_closure_root)
    if (
        not closure_root.is_absolute()
        or os.path.normpath(raw_closure_root) != raw_closure_root
        or closure_root.name != expected_tree_sha256
        or provenance_path
        != closure_root / PRODUCTION_PROVENANCE_FILE_NAME
    ):
        raise CandidateBuildError(
            "production closure is not at its canonical content-addressed path"
        )
    observed_tree_sha256 = _production_closure_tree_sha256(
        closure_root,
        provenance_path,
    )
    if observed_tree_sha256 != expected_tree_sha256:
        raise CandidateBuildError(
            "production closure tree differs from its pinned content address"
        )

    admitted_paths: dict[str, Path] = {}
    observed_digests: dict[str, str] = {}
    for name in ("cargo", "rustc", "linker", "git", "gpg", "python"):
        raw_path = value[f"{name}_path"]
        expected_digest = value[f"{name}_sha256"]
        if (
            not isinstance(raw_path, str)
            or not isinstance(expected_digest, str)
            or re.fullmatch(r"[0-9a-f]{64}", expected_digest) is None
        ):
            raise CandidateBuildError(f"{name} closure fields are malformed")
        path = Path(raw_path)
        if not path.is_absolute() or os.path.normpath(raw_path) != raw_path:
            raise CandidateBuildError(f"{name} closure path is not canonical")
        try:
            metadata = path.lstat()
            resolved = path.resolve(strict=True)
        except OSError as error:
            raise CandidateBuildError(f"{name} closure tool is unavailable") from error
        try:
            path.relative_to(closure_root)
        except ValueError as error:
            raise CandidateBuildError(
                f"{name} tool is outside the production closure"
            ) from error
        if (
            resolved != path
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != 0
            or metadata.st_nlink != 1
            or metadata.st_mode & 0o222 != 0
            or metadata.st_mode & 0o7000 != 0
            or metadata.st_mode & 0o111 == 0
        ):
            raise CandidateBuildError(f"{name} closure tool has unsafe metadata")
        observed, _ = _binary_sha256_for_tool(path)
        if observed != expected_digest:
            raise CandidateBuildError(f"{name} differs from the pinned closure")
        admitted_paths[name] = path
        observed_digests[name] = observed

    directory_fields = {
        "apple_developer_dir": value["apple_developer_dir_path"],
        "apple_sdk": value["apple_sdk_path"],
        "cargo_home": value["cargo_home_path"],
        "cargo_vendor": value["cargo_vendor_path"],
        "clang_resource_dir": value["clang_resource_dir_path"],
        "git_exec_path": value["git_exec_path"],
        "gnupghome": value["gnupghome_path"],
        "rustc_sysroot": value["rustc_sysroot_path"],
        "source_root": value["source_root"],
    }
    admitted_directories: dict[str, Path] = {}
    for label, raw_path in directory_fields.items():
        if not isinstance(raw_path, str):
            raise CandidateBuildError(f"{label} closure path is malformed")
        path = Path(raw_path)
        if not path.is_absolute() or os.path.normpath(raw_path) != raw_path:
            raise CandidateBuildError(f"{label} closure path is not canonical")
        try:
            metadata = path.lstat()
            resolved = path.resolve(strict=True)
            path.relative_to(closure_root)
        except (OSError, ValueError) as error:
            raise CandidateBuildError(
                f"{label} closure directory is unavailable or outside the closure"
            ) from error
        if (
            resolved != path
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != 0
            or metadata.st_mode & 0o222 != 0
            or metadata.st_mode & 0o7000 != 0
        ):
            raise CandidateBuildError(
                f"{label} closure directory has unsafe metadata"
            )
        admitted_directories[label] = path

    apple_developer_dir = admitted_directories["apple_developer_dir"]
    apple_sdk = admitted_directories["apple_sdk"]
    clang_resource_dir = admitted_directories["clang_resource_dir"]
    try:
        admitted_paths["linker"].relative_to(apple_developer_dir)
        apple_sdk.relative_to(apple_developer_dir)
        clang_resource_dir.relative_to(apple_developer_dir)
    except ValueError as error:
        raise CandidateBuildError(
            "linker, SDK, or clang resource directory escapes Apple developer closure"
        ) from error
    apple_tool_bin = admitted_paths["linker"].parent
    if admitted_paths["linker"].name != "clang":
        raise CandidateBuildError("admitted Apple linker driver must be clang")
    for tool_name in (
        "clang++",
        "ar",
        "ranlib",
        "libtool",
        "ld",
        "nm",
        "otool",
        "strip",
    ):
        tool_path = apple_tool_bin / tool_name
        try:
            tool_metadata = tool_path.lstat()
            tool_resolved = tool_path.resolve(strict=True)
            resolved_metadata = tool_resolved.lstat()
            tool_resolved.relative_to(apple_developer_dir)
        except OSError as error:
            raise CandidateBuildError(
                f"Apple developer closure lacks {tool_name}"
            ) from error
        except ValueError as error:
            raise CandidateBuildError(
                f"Apple developer tool escapes the closure: {tool_name}"
            ) from error
        if stat.S_ISLNK(tool_metadata.st_mode):
            target = os.readlink(tool_path)
            if os.path.isabs(target):
                raise CandidateBuildError(
                    f"Apple developer tool has an absolute symlink: {tool_name}"
                )
        elif not stat.S_ISREG(tool_metadata.st_mode):
            raise CandidateBuildError(
                f"Apple developer tool is not a file/symlink: {tool_name}"
            )
        if (
            tool_metadata.st_uid != 0
            or not stat.S_ISREG(resolved_metadata.st_mode)
            or resolved_metadata.st_uid != 0
            or resolved_metadata.st_nlink != 1
            or resolved_metadata.st_mode & 0o222 != 0
            or resolved_metadata.st_mode & 0o111 == 0
        ):
            raise CandidateBuildError(
                f"Apple developer tool has unsafe metadata: {tool_name}"
            )

    cargo_config = admitted_directories["cargo_home"] / "config.toml"
    legacy_cargo_config = admitted_directories["cargo_home"] / "config"
    expected_cargo_config = _canonical_offline_cargo_config(
        admitted_directories["cargo_home"],
        admitted_directories["cargo_vendor"],
    )
    try:
        actual_cargo_config = cargo_config.read_bytes()
    except OSError as error:
        raise CandidateBuildError(
            "production Cargo home lacks canonical offline vendor configuration"
        ) from error
    if (
        legacy_cargo_config.exists()
        or legacy_cargo_config.is_symlink()
        or actual_cargo_config != expected_cargo_config
        or not any(admitted_directories["cargo_vendor"].iterdir())
    ):
        raise CandidateBuildError(
            "production Cargo home/vendor configuration is not exact and offline"
        )

    raw_reviewed_closure = value["reviewed_source_closure_path"]
    source_signing_key_fingerprint = value["source_signing_key_fingerprint"]
    if (
        not isinstance(raw_reviewed_closure, str)
        or not isinstance(source_signing_key_fingerprint, str)
        or re.fullmatch(
            r"(?:[0-9A-F]{40}|[0-9A-F]{64})",
            source_signing_key_fingerprint,
        )
        is None
    ):
        raise CandidateBuildError("signature-verifier provenance fields are malformed")
    reviewed_source_closure = Path(raw_reviewed_closure)
    try:
        reviewed_metadata = reviewed_source_closure.lstat()
        reviewed_resolved = reviewed_source_closure.resolve(strict=True)
        reviewed_source_closure.relative_to(closure_root)
    except OSError as error:
        raise CandidateBuildError(
            "reviewed source-closure descriptor is unavailable"
        ) from error
    except ValueError as error:
        raise CandidateBuildError(
            "reviewed source-closure descriptor is outside the production closure"
        ) from error
    if (
        reviewed_resolved != reviewed_source_closure
        or not stat.S_ISREG(reviewed_metadata.st_mode)
        or reviewed_metadata.st_uid != 0
        or reviewed_metadata.st_nlink != 1
        or reviewed_metadata.st_mode & 0o222 != 0
    ):
        raise CandidateBuildError(
            "reviewed source-closure descriptor has unsafe metadata"
        )
    _admit_running_python_runtime(
        closure_root,
        admitted_paths["python"],
    )
    _admit_macos_dynamic_tool_closure(
        closure_root,
        (
            *admitted_paths.values(),
            *(
                apple_tool_bin / tool_name
                for tool_name in (
                    "clang++",
                    "ar",
                    "ranlib",
                    "libtool",
                    "ld",
                    "nm",
                    "otool",
                    "strip",
                )
            ),
        ),
        otool=apple_tool_bin / "otool",
    )
    _verify_admitted_rustc_sysroot(
        admitted_paths["rustc"],
        admitted_directories["rustc_sysroot"],
    )
    _verify_admitted_git_builtins(
        admitted_paths["git"],
        admitted_directories["git_exec_path"],
    )
    return AdmittedRustToolchain(
        provenance=provenance_path,
        closure_root=closure_root,
        closure_tree_sha256=observed_tree_sha256,
        source_root=admitted_directories["source_root"],
        reviewed_source_closure=reviewed_source_closure,
        apple_developer_dir=apple_developer_dir,
        apple_sdk=apple_sdk,
        clang_resource_dir=clang_resource_dir,
        cargo_home=admitted_directories["cargo_home"],
        cargo_vendor=admitted_directories["cargo_vendor"],
        cargo=admitted_paths["cargo"],
        rustc=admitted_paths["rustc"],
        rustc_sysroot=admitted_directories["rustc_sysroot"],
        linker=admitted_paths["linker"],
        git=admitted_paths["git"],
        git_exec_path=admitted_directories["git_exec_path"],
        gpg=admitted_paths["gpg"],
        gnupghome=admitted_directories["gnupghome"],
        python=admitted_paths["python"],
        cargo_sha256=observed_digests["cargo"],
        rustc_sha256=observed_digests["rustc"],
        linker_sha256=observed_digests["linker"],
        git_sha256=observed_digests["git"],
        gpg_sha256=observed_digests["gpg"],
        python_sha256=observed_digests["python"],
        source_signing_key_fingerprint=source_signing_key_fingerprint,
        provenance_sha256=provenance_sha256,
    )


def _binary_sha256_for_tool(path: Path) -> tuple[str, int]:
    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid not in (0, os.geteuid())
        or before.st_nlink < 1
        or before.st_size <= 0
        or before.st_mode & 0o022 != 0
        or before.st_mode & 0o111 == 0
    ):
        raise CandidateBuildError(
            "toolchain executable has unsafe metadata"
        )
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(os.fsencode(path), flags)
    try:
        opened_before = os.fstat(descriptor)
        digest = hashlib.sha256()
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
        opened_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = path.lstat()
    stable = lambda item: (
        item.st_dev,
        item.st_ino,
        item.st_mode,
        item.st_uid,
        item.st_nlink,
        item.st_size,
        item.st_mtime_ns,
        item.st_ctime_ns,
    )
    if not (
        stable(before)
        == stable(opened_before)
        == stable(opened_after)
        == stable(after)
    ):
        raise CandidateBuildError("toolchain executable changed while hashed")
    return digest.hexdigest(), after.st_size


def _otool_output(
    path: Path,
    *arguments: str,
    executable: Path = Path("/usr/bin/otool"),
) -> str:
    if sys.platform != "darwin":
        raise CandidateBuildError(
            "root-published production closure admission is currently macOS-only"
        )
    try:
        completed = subprocess.run(
            [
                str(executable),
                "-arch",
                "arm64",
                *arguments,
                str(path),
            ],
            env={
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
            },
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="strict",
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired, UnicodeError) as error:
        raise CandidateBuildError("could not inspect production Mach-O closure") from error
    if completed.returncode != 0:
        raise CandidateBuildError(
            f"production executable has no inspectable arm64 Mach-O slice: {path}"
        )
    lines = completed.stdout.splitlines()
    exact_header = f"{path}:"
    architecture_headers = [
        line
        for line in lines
        if re.fullmatch(
            rf"{re.escape(str(path))} \(architecture [^)]+\):",
            line,
        )
        is not None
    ]
    if lines.count(exact_header) != 1 or architecture_headers:
        raise CandidateBuildError(
            "otool did not return exactly one selected arm64 slice"
        )
    return completed.stdout


def _macho_rpaths(
    path: Path,
    *,
    otool: Path = Path("/usr/bin/otool"),
) -> tuple[str, ...]:
    lines = _otool_output(path, "-l", executable=otool).splitlines()
    rpaths: list[str] = []
    for index, line in enumerate(lines):
        if line.strip() != "cmd LC_RPATH":
            continue
        for detail in lines[index + 1 : index + 5]:
            stripped = detail.strip()
            if stripped.startswith("path ") and " (offset " in stripped:
                rpaths.append(
                    stripped[len("path ") :].split(" (offset ", 1)[0]
                )
                break
        else:
            raise CandidateBuildError("Mach-O LC_RPATH command is malformed")
    return tuple(rpaths)


MACHO_MAGICS = frozenset(
    {
        b"\xca\xfe\xba\xbe",
        b"\xbe\xba\xfe\xca",
        b"\xca\xfe\xba\xbf",
        b"\xbf\xba\xfe\xca",
        b"\xfe\xed\xfa\xce",
        b"\xce\xfa\xed\xfe",
        b"\xfe\xed\xfa\xcf",
        b"\xcf\xfa\xed\xfe",
    }
)


def _is_macho_regular_file(path: Path) -> bool:
    """Identify one stable regular Mach-O image without following a symlink."""

    try:
        before = path.lstat()
    except OSError as error:
        raise CandidateBuildError(
            "could not inventory production runtime files"
        ) from error
    if not stat.S_ISREG(before.st_mode):
        return False
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(os.fsencode(path), flags)
        opened_before = os.fstat(descriptor)
        magic = os.read(descriptor, 4)
        opened_after = os.fstat(descriptor)
    except OSError as error:
        raise CandidateBuildError(
            "could not inspect a production runtime file"
        ) from error
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    try:
        after = path.lstat()
    except OSError as error:
        raise CandidateBuildError(
            "production runtime file disappeared during inventory"
        ) from error

    def identity(metadata: os.stat_result) -> tuple[int, ...]:
        return (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_mode,
            metadata.st_uid,
            metadata.st_nlink,
            metadata.st_size,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
        )

    if not (
        identity(before)
        == identity(opened_before)
        == identity(opened_after)
        == identity(after)
    ):
        raise CandidateBuildError(
            "production runtime file changed during Mach-O inventory"
        )
    return magic in MACHO_MAGICS


def _macho_regular_files_under(closure_root: Path) -> tuple[Path, ...]:
    """Return every regular Mach-O image anywhere in the published closure."""

    images: list[Path] = []
    for directory, directory_names, file_names in os.walk(
        closure_root,
        topdown=True,
        followlinks=False,
    ):
        directory_names.sort()
        file_names.sort()
        base = Path(directory)
        for file_name in file_names:
            path = base / file_name
            if _is_macho_regular_file(path):
                images.append(path.resolve(strict=True))
    return tuple(images)


def _is_canonical_system_dependency(load_name: str) -> bool:
    """Accept only normalized absolute load names beneath sealed system roots."""

    if (
        not load_name
        or "\x00" in load_name
        or not load_name.startswith("/")
        or os.path.normpath(load_name) != load_name
    ):
        return False
    parts = pathlib.PurePosixPath(load_name).parts
    if any(part in ("", ".", "..") for part in parts[1:]):
        return False
    return load_name.startswith(("/usr/lib/", "/System/Library/"))


def _admit_macos_dynamic_tool_closure(
    closure_root: Path,
    executable_paths: Sequence[Path],
    *,
    otool: Path = Path("/usr/bin/otool"),
) -> None:
    """Require every published Mach-O image dependency to stay in the closure."""

    root_executables = {
        path.resolve(strict=True)
        for path in executable_paths
    }
    published_images = set(_macho_regular_files_under(closure_root))
    pending = list(root_executables | published_images)
    observed: set[Path] = set()
    while pending:
        path = pending.pop()
        path = path.resolve(strict=True)
        if path in observed:
            continue
        observed.add(path)
        is_root_executable = path in root_executables
        lines = _otool_output(path, "-L", executable=otool).splitlines()
        if not lines:
            raise CandidateBuildError("otool returned no dependency inventory")
        raw_dependencies = []
        for line in lines[1:]:
            stripped = line.strip()
            if not stripped:
                continue
            raw_dependencies.append(stripped.split(" (compatibility ", 1)[0])
        install_name_lines = _otool_output(
            path,
            "-D",
            executable=otool,
        ).splitlines()[1:]
        install_names = {line.strip() for line in install_name_lines if line.strip()}
        raw_dependencies = [
            dependency
            for dependency in raw_dependencies
            if dependency not in install_names
        ]
        rpaths = _macho_rpaths(path, otool=otool)

        def expand_loader_or_absolute(raw: str) -> Path | None:
            if raw.startswith("@loader_path/"):
                return path.parent / raw[len("@loader_path/") :]
            if raw.startswith("/"):
                return Path(raw)
            return None

        expanded_rpaths: list[Path] = []
        for raw in rpaths:
            if raw.startswith(("@executable_path", "@rpath")):
                raise CandidateBuildError(
                    "Mach-O LC_RPATH uses unsupported inherited executable context"
                )
            expanded = expand_loader_or_absolute(raw)
            if expanded is None:
                raise CandidateBuildError("Mach-O LC_RPATH is not canonical")
            try:
                expanded.resolve(strict=True).relative_to(closure_root)
            except (OSError, ValueError) as error:
                raise CandidateBuildError(
                    "Mach-O LC_RPATH escapes the production closure"
                ) from error
            expanded_rpaths.append(expanded)
        for dependency in raw_dependencies:
            if _is_canonical_system_dependency(dependency):
                continue
            candidates: list[Path] = []
            if dependency.startswith("@rpath/"):
                if not is_root_executable:
                    raise CandidateBuildError(
                        "dependent Mach-O uses inherited @rpath context"
                    )
                suffix = dependency[len("@rpath/") :]
                candidates.extend(base / suffix for base in expanded_rpaths)
            elif dependency.startswith("@executable_path"):
                raise CandidateBuildError(
                    "Mach-O dependency uses unsupported @executable_path context"
                )
            else:
                expanded = expand_loader_or_absolute(dependency)
                if expanded is not None:
                    candidates.append(expanded)
            resolved_dependency = None
            for candidate in candidates:
                try:
                    resolved = candidate.resolve(strict=True)
                    resolved.relative_to(closure_root)
                except (OSError, ValueError):
                    continue
                resolved_dependency = resolved
                break
            if resolved_dependency is None:
                raise CandidateBuildError(
                    f"Mach-O dependency escapes the production closure: {dependency}"
                )
            pending.append(resolved_dependency)


def _verify_admitted_rustc_sysroot(
    rustc: Path,
    rustc_sysroot: Path,
) -> None:
    try:
        completed = subprocess.run(
            [str(rustc), "--print=sysroot"],
            env={
                "HOME": "/var/empty",
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "RUSTC_WRAPPER": "",
                "RUSTC_WORKSPACE_WRAPPER": "",
                "TZ": "UTC",
            },
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="strict",
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired, UnicodeError) as error:
        raise CandidateBuildError("could not inspect admitted rustc sysroot") from error
    if (
        completed.returncode != 0
        or completed.stdout != f"{rustc_sysroot}\n"
    ):
        raise CandidateBuildError(
            "admitted rustc does not use the pinned production sysroot"
        )


def _verify_admitted_git_builtins(
    git: Path,
    git_exec_path: Path,
) -> None:
    environment = {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_EXEC_PATH": str(git_exec_path),
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }
    try:
        completed = subprocess.run(
            [str(git), "--list-cmds=builtins"],
            env=environment,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="strict",
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired, UnicodeError) as error:
        raise CandidateBuildError("could not inspect admitted Git builtins") from error
    builtins = set(completed.stdout.split())
    if (
        completed.returncode != 0
        or not {"rev-parse", "verify-commit"}.issubset(builtins)
    ):
        raise CandidateBuildError(
            "admitted Git requires an unbound exec helper for source verification"
        )


def _admit_running_python_runtime(
    closure_root: Path,
    python: Path,
) -> None:
    """Bind the already-running bootstrap to isolated closure stdlib bytes."""

    hostile_environment = {
        name
        for name in os.environ
        if name.startswith(("DYLD_", "LD_"))
        or name
        in {
            "PYTHONHOME",
            "PYTHONINSPECT",
            "PYTHONPATH",
            "PYTHONSTARTUP",
            "PYTHONUSERBASE",
        }
    }
    if (
        sys.flags.isolated != 1
        or hostile_environment
        or Path(sys.executable).resolve(strict=True) != python
    ):
        raise CandidateBuildError(
            "production builder must run directly under admitted Python -I "
            "with no loader/Python injection environment"
        )
    for label, raw_path in (
        ("sys.prefix", sys.prefix),
        ("sys.base_prefix", sys.base_prefix),
        *((f"sys.path[{index}]", value) for index, value in enumerate(sys.path)),
    ):
        if not raw_path:
            raise CandidateBuildError(f"isolated Python exposes empty {label}")
        path = Path(raw_path)
        try:
            resolved = path.resolve(strict=True)
            resolved.relative_to(closure_root)
        except (OSError, ValueError) as error:
            raise CandidateBuildError(
                f"isolated Python {label} escapes the production closure"
            ) from error


def _recheck_admitted_toolchain(toolchain: AdmittedRustToolchain) -> None:
    for path, expected, label in (
        (toolchain.cargo, toolchain.cargo_sha256, "Cargo"),
        (toolchain.rustc, toolchain.rustc_sha256, "rustc"),
        (toolchain.linker, toolchain.linker_sha256, "linker"),
        (toolchain.git, toolchain.git_sha256, "Git"),
        (toolchain.gpg, toolchain.gpg_sha256, "GPG"),
        (toolchain.python, toolchain.python_sha256, "Python"),
    ):
        observed, _ = _binary_sha256_for_tool(path)
        if observed != expected:
            raise CandidateBuildError(f"{label} changed after admission")
    try:
        provenance = toolchain.provenance.read_bytes()
        gnupghome_metadata = toolchain.gnupghome.lstat()
    except OSError as error:
        raise CandidateBuildError(
            "toolchain/signature provenance became unavailable"
        ) from error
    if hashlib.sha256(provenance).hexdigest() != toolchain.provenance_sha256:
        raise CandidateBuildError("production closure provenance changed after admission")
    if (
        toolchain.gnupghome.resolve(strict=True) != toolchain.gnupghome
        or not stat.S_ISDIR(gnupghome_metadata.st_mode)
        or gnupghome_metadata.st_uid not in (0, os.geteuid())
        or gnupghome_metadata.st_mode & 0o022 != 0
    ):
        raise CandidateBuildError("GNUPGHOME changed after admission")
    observed_tree_sha256 = _production_closure_tree_sha256(
        toolchain.closure_root,
        toolchain.provenance,
    )
    if observed_tree_sha256 != toolchain.closure_tree_sha256:
        raise CandidateBuildError("production closure tree changed after admission")


def _verify_exact_signed_commit(
    root: Path,
    expected_commit: str,
    toolchain: AdmittedRustToolchain,
    *,
    command_runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run,
) -> None:
    """Verify one exact commit with the admitted GPG binary, keyring, and signer."""

    if re.fullmatch(r"[0-9a-f]{40}", expected_commit) is None:
        raise CandidateBuildError("signed source commit is not canonical")
    environment = {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_EXEC_PATH": str(toolchain.git_exec_path),
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GNUPGHOME": str(toolchain.gnupghome),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }

    def run(command: list[str]) -> subprocess.CompletedProcess[bytes]:
        try:
            return command_runner(
                command,
                cwd=root,
                env=environment,
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        except OSError as error:
            raise CandidateBuildError(
                "could not execute the admitted signed-commit verifier"
            ) from error

    head_command = [
        str(toolchain.git),
        "-c",
        "core.fileMode=true",
        "rev-parse",
        "--verify",
        "HEAD^{commit}",
    ]
    head_before = run(head_command)
    signature = run(
        [
            str(toolchain.git),
            "-c",
            "core.fileMode=true",
            "-c",
            f"gpg.program={toolchain.gpg}",
            "-c",
            "gpg.format=openpgp",
            "verify-commit",
            "--raw",
            expected_commit,
        ]
    )
    head_after = run(head_command)
    expected_head = f"{expected_commit}\n".encode("ascii")
    if (
        head_before.returncode != 0
        or head_before.stdout != expected_head
        or head_after.returncode != 0
        or head_after.stdout != expected_head
    ):
        raise CandidateBuildError(
            "HEAD changed from the exact signed source commit"
        )
    if signature.returncode != 0:
        raise CandidateBuildError(
            "the exact source commit does not have a valid admitted GPG signature"
        )
    prefix = b"[GNUPG:] VALIDSIG "
    fingerprints = []
    for line in signature.stderr.splitlines():
        if line.startswith(prefix):
            fields = line[len(prefix) :].split()
            if not fields:
                raise CandidateBuildError("GPG emitted malformed VALIDSIG status")
            fingerprints.append(fields[0])
    expected_fingerprint = toolchain.source_signing_key_fingerprint.encode("ascii")
    if fingerprints != [expected_fingerprint]:
        raise CandidateBuildError(
            "the exact source commit signer differs from the pinned fingerprint"
        )


def _admit_production_closure_binding(
    root: Path,
    reviewed_source_closure: Path,
    toolchain: AdmittedRustToolchain,
) -> None:
    """Bind this running bootstrap and requested source to the root closure."""

    expected_builder = root / "scripts/build_kagemusha_v4_candidate_bundle.py"
    expected_source_seal = root / "scripts/kagemusha_source_tree_seal.py"
    expected_resource_guard = (
        root / "scripts/formal/run_sumeragi_v2_tlapm_guard.py"
    )
    if (
        root != toolchain.source_root
        or reviewed_source_closure != toolchain.reviewed_source_closure
        or Path(__file__).resolve(strict=True) != expected_builder
        or Path(source_seal.__file__).resolve(strict=True) != expected_source_seal
        or Path(resource_guard.__file__).resolve(strict=True) != expected_resource_guard
        or not (root / ".git").is_dir()
        or (root / ".git").is_symlink()
        or (root / ".git/objects/info/alternates").exists()
        or (root / ".git/objects/info/alternates").is_symlink()
    ):
        raise CandidateBuildError(
            "production bootstrap/source is not wholly inside the admitted root closure"
        )


@contextmanager
def _use_root_published_source(
    root: Path,
    _workspace: Path,
    descriptor_path: str,
    descriptor_sha256: str,
    *,
    expected_identity: source_seal.SourceIdentity,
) -> Iterator[source_seal.SourceMaterialization]:
    """Use the pre-provisioned immutable source directly; never recopy it as a user."""

    descriptor = Path(descriptor_path)
    observed = source_seal.compute_identity(
        root,
        descriptor_path,
        descriptor_sha256,
    )
    if observed != expected_identity:
        raise CandidateBuildError(
            "root-published source changed before production use"
        )
    yield source_seal.SourceMaterialization(
        root=root,
        reviewed_source_closure=descriptor,
        identity=observed,
    )


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


def _sanitized_build_environment(
    target_dir: Path,
    rustc: Path,
    linker: Path,
    rustc_sysroot: Path,
    cargo_home: Path,
    apple_developer_dir: Path,
    apple_sdk: Path,
    clang_resource_dir: Path,
) -> dict[str, str]:
    """Create a minimal build environment with private HOME/CARGO_HOME."""

    private_home = target_dir / ".build-home"
    private_tmp = target_dir / ".tmp"
    for path in (private_home, private_tmp):
        path.mkdir(mode=0o700)
    apple_tool_bin = linker.parent
    apple_compile_flags = (
        f"-isysroot {apple_sdk} "
        f"-resource-dir {clang_resource_dir} "
        f"-B{apple_tool_bin}"
    )
    return {
        "CARGO_ENCODED_RUSTFLAGS": (
            f"-Clinker={linker}\x1f"
            f"--sysroot={rustc_sysroot}\x1f"
            f"-Clink-arg=-isysroot\x1f"
            f"-Clink-arg={apple_sdk}\x1f"
            f"-Clink-arg=-resource-dir\x1f"
            f"-Clink-arg={clang_resource_dir}\x1f"
            f"-Clink-arg=-B{apple_tool_bin}"
        ),
        "CARGO_HOME": str(cargo_home),
        "CARGO_NET_OFFLINE": "true",
        "CARGO_TERM_COLOR": "never",
        "AR": str(apple_tool_bin / "ar"),
        "BINDGEN_EXTRA_CLANG_ARGS": apple_compile_flags,
        "CC": str(linker),
        "CFLAGS": apple_compile_flags,
        "CXX": str(apple_tool_bin / "clang++"),
        "CXXFLAGS": apple_compile_flags,
        "DEVELOPER_DIR": str(apple_developer_dir),
        "HOME": str(private_home),
        "LANG": "C",
        "LIBTOOL": str(apple_tool_bin / "libtool"),
        "LC_ALL": "C",
        "NM": str(apple_tool_bin / "nm"),
        "PATH": f"{apple_tool_bin}:/usr/bin:/bin",
        "RANLIB": str(apple_tool_bin / "ranlib"),
        "RUSTC": str(rustc),
        "RUSTC_WRAPPER": "",
        "RUSTC_WORKSPACE_WRAPPER": "",
        "RUSTFLAGS": "",
        "SDKROOT": str(apple_sdk),
        "STRIP": str(apple_tool_bin / "strip"),
        "TMPDIR": str(private_tmp),
        "TZ": "UTC",
    }


def _reject_ambient_cargo_configs(source_root: Path) -> None:
    for parent in source_root.parents:
        cargo_directory = parent / ".cargo"
        for name in ("config", "config.toml"):
            candidate = cargo_directory / name
            if candidate.exists() or candidate.is_symlink():
                raise CandidateBuildError(
                    f"ambient Cargo configuration is forbidden: {candidate}"
                )


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
    toolchain_provenance: Path | None = None,
    toolchain_provenance_sha256: str | None = None,
    identity_reader: Callable[[Path, str, str], source_seal.SourceIdentity] = (
        source_seal.compute_identity
    ),
    source_materializer: Callable[..., ContextManager[source_seal.SourceMaterialization]] = (
        _use_root_published_source
    ),
    source_command_guard: Callable[..., list[str]] = (
        source_seal.write_denied_source_command
    ),
    toolchain_admitter: Callable[
        [str, Path | None, str | None],
        AdmittedRustToolchain,
    ] = (
        _admitted_production_rust_toolchain
    ),
    production_closure_admitter: Callable[
        [Path, Path, AdmittedRustToolchain],
        None,
    ] = _admit_production_closure_binding,
    source_git_configurer: Callable[[Path, Path], None] = (
        source_seal.configure_production_git
    ),
    toolchain_rechecker: Callable[
        [AdmittedRustToolchain],
        None,
    ] = _recheck_admitted_toolchain,
    signature_verifier: Callable[
        [Path, str, AdmittedRustToolchain],
        None,
    ] = _verify_exact_signed_commit,
    command_runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run,
    physical_memory_reader: Callable[[], int] = _physical_memory_bytes,
    build_identity_reader: Callable[[], tuple[str, int]] = (
        _admitted_build_worker_identity
    ),
    build_lock: Callable[[], ContextManager[object]] = _shared_memory_heavy_build_lock,
) -> dict[str, object]:
    """Build once and prove the pinned reviewed source closure stayed exact."""

    root = root.resolve(strict=True)
    reviewed_source_closure = reviewed_source_closure.resolve(strict=True)
    physical_memory_bytes = _admitted_physical_memory_bytes(physical_memory_reader)
    build_user_name, build_uid = build_identity_reader()
    if (
        build_user_name != PRODUCTION_BUILD_USER_NAME
        or isinstance(build_uid, bool)
        or not isinstance(build_uid, int)
        or build_uid < 1
    ):
        raise CandidateBuildError("production build worker identity is invalid")
    target_dir = _prepare_fresh_external_target_dir(root, target_dir)
    toolchain = toolchain_admitter(
        cargo,
        toolchain_provenance,
        toolchain_provenance_sha256,
    )
    production_closure_admitter(
        root,
        reviewed_source_closure,
        toolchain,
    )
    source_git_configurer(
        toolchain.git,
        toolchain.git_exec_path,
    )
    first = identity_reader(
        root,
        str(reviewed_source_closure),
        reviewed_source_closure_sha256,
    )
    try:
        materialization_context = source_materializer(
            root,
            target_dir / SEALED_SOURCE_WORKSPACE_NAME,
            str(reviewed_source_closure),
            reviewed_source_closure_sha256,
            expected_identity=first,
        )
        with materialization_context as materialization:
            materialized_root = materialization.root.resolve(strict=True)
            materialized_descriptor = (
                materialization.reviewed_source_closure.resolve(strict=True)
            )
            uses_root_published_source = (
                materialized_root == root
                and materialized_descriptor == reviewed_source_closure
            )
            if (materialized_root == root) != (
                materialized_descriptor == reviewed_source_closure
            ):
                raise CandidateBuildError(
                    "source root and descriptor must use one common production boundary"
                )
            if not uses_root_published_source:
                try:
                    materialized_root.relative_to(target_dir)
                    materialized_descriptor.relative_to(target_dir)
                except ValueError as error:
                    raise CandidateBuildError(
                        "sealed reviewed source materialization escaped the fixed "
                        "target directory"
                    ) from error
            second = identity_reader(
                materialized_root,
                str(materialized_descriptor),
                reviewed_source_closure_sha256,
            )
            if materialization.identity != first or second != first:
                raise CandidateBuildError(
                    "sealed reviewed source materialization is not the pinned closure"
                )
            signature_verifier(
                materialized_root,
                first.source_commit,
                toolchain,
            )
            _reject_ambient_cargo_configs(materialized_root)
            environment = _sanitized_build_environment(
                target_dir,
                toolchain.rustc,
                toolchain.linker,
                toolchain.rustc_sysroot,
                toolchain.cargo_home,
                toolchain.apple_developer_dir,
                toolchain.apple_sdk,
                toolchain.clang_resource_dir,
            )
            environment["KAGEMUSHA_BUILD_SOURCE_COMMIT"] = first.source_commit
            environment["KAGEMUSHA_BUILD_SOURCE_TREE_SHA256"] = (
                first.source_tree_sha256
            )
            environment["KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE"] = str(
                materialized_descriptor
            )
            environment["KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256"] = (
                first.reviewed_source_closure_descriptor_sha256
            )
            environment["KAGEMUSHA_SOURCE_SEAL_PYTHON"] = str(toolchain.python)
            environment["KAGEMUSHA_BUILD_GPG_EXECUTABLE"] = str(toolchain.gpg)
            environment["KAGEMUSHA_BUILD_GNUPGHOME"] = str(toolchain.gnupghome)
            environment["KAGEMUSHA_BUILD_GIT_EXECUTABLE"] = str(toolchain.git)
            environment["KAGEMUSHA_BUILD_GIT_EXEC_PATH"] = str(
                toolchain.git_exec_path
            )
            environment["KAGEMUSHA_SOURCE_SEAL_GIT_EXECUTABLE"] = str(
                toolchain.git
            )
            environment["KAGEMUSHA_SOURCE_SEAL_GIT_EXEC_PATH"] = str(
                toolchain.git_exec_path
            )
            environment["KAGEMUSHA_BUILD_SOURCE_SIGNING_KEY_FINGERPRINT"] = (
                toolchain.source_signing_key_fingerprint
            )
            cargo_command = [
                str(toolchain.cargo),
                "build",
                "--release",
                "--locked",
                "--offline",
                "--frozen",
                "--target-dir",
                str(target_dir),
                "-p",
                "iroha_core",
                "--features",
                SEALED_FEATURE,
                "--bin",
                BINARY_NAME,
                "--jobs",
                "1",
                "--message-format=json-render-diagnostics",
            ]
            try:
                command = source_command_guard(
                    materialized_root,
                    materialized_descriptor,
                    cargo_command,
                )
            except (OSError, RuntimeError, ValueError) as error:
                raise CandidateBuildError(
                    f"could not enforce sealed source write denial: {error}"
                ) from error
            with build_lock():
                try:
                    completed = command_runner(
                        command,
                        cwd=materialized_root,
                        env=environment,
                        check=False,
                        stdout=subprocess.PIPE,
                    )
                except OSError as error:
                    raise CandidateBuildError(
                        f"could not start Cargo: {error}"
                    ) from error
            if completed.returncode != 0:
                raise CandidateBuildError(
                    "sealed candidate Cargo build failed with status "
                    f"{completed.returncode}"
                )
            third = identity_reader(
                materialized_root,
                str(materialized_descriptor),
                reviewed_source_closure_sha256,
            )
            if third != first:
                raise CandidateBuildError(
                    "sealed source identity changed during the candidate build"
                )

            if not isinstance(completed.stdout, bytes):
                raise CandidateBuildError(
                    "Cargo did not return binary build metadata"
                )
            binary = _built_binary_from_cargo_messages(
                completed.stdout,
                materialized_root,
                target_dir,
            )
            sha256, size_bytes = _binary_sha256(binary)
            toolchain_rechecker(toolchain)
            fourth = identity_reader(
                materialized_root,
                str(materialized_descriptor),
                reviewed_source_closure_sha256,
            )
            if fourth != first:
                raise CandidateBuildError(
                    "sealed source identity changed while sealing the candidate binary"
                )
            return {
                "binary_path": str(binary),
                "binary_sha256": sha256,
                "binary_size_bytes": size_bytes,
                "build_profile": "release",
                "build_uid": build_uid,
                "build_user_name": build_user_name,
                "minimum_build_physical_memory_bytes": (
                    MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES
                ),
                "physical_memory_bytes_at_admission": physical_memory_bytes,
                "publication_status": "provisional_boi_build_worker_output",
                "schema": "iroha.kagemusha.sealed_candidate_build.v1",
                "reviewed_source_closure": first.reviewed_source_closure,
                "reviewed_source_closure_descriptor_sha256": (
                    first.reviewed_source_closure_descriptor_sha256
                ),
                "source_commit": first.source_commit,
                "source_repo_dirty": first.source_repo_dirty,
                "source_tree_sha256": first.source_tree_sha256,
                "target_dir": str(target_dir),
                "cargo_path": str(toolchain.cargo),
                "cargo_sha256": toolchain.cargo_sha256,
                "cargo_home_path": str(toolchain.cargo_home),
                "cargo_vendor_path": str(toolchain.cargo_vendor),
                "apple_developer_dir_path": str(toolchain.apple_developer_dir),
                "apple_sdk_path": str(toolchain.apple_sdk),
                "clang_resource_dir_path": str(toolchain.clang_resource_dir),
                "rustc_path": str(toolchain.rustc),
                "rustc_sha256": toolchain.rustc_sha256,
                "rustc_sysroot_path": str(toolchain.rustc_sysroot),
                "linker_path": str(toolchain.linker),
                "linker_sha256": toolchain.linker_sha256,
                "git_path": str(toolchain.git),
                "git_sha256": toolchain.git_sha256,
                "git_exec_path": str(toolchain.git_exec_path),
                "gpg_path": str(toolchain.gpg),
                "gpg_sha256": toolchain.gpg_sha256,
                "python_path": str(toolchain.python),
                "python_sha256": toolchain.python_sha256,
                "source_signing_key_fingerprint": (
                    toolchain.source_signing_key_fingerprint
                ),
                "production_closure_root": str(toolchain.closure_root),
                "production_closure_tree_sha256": (
                    toolchain.closure_tree_sha256
                ),
                "toolchain_provenance_sha256": toolchain.provenance_sha256,
            }
    except CandidateBuildError:
        raise
    except (OSError, RuntimeError, ValueError) as error:
        raise CandidateBuildError(
            f"sealed reviewed source boundary failed: {error}"
        ) from error


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=REPO_ROOT,
        help="root-owned non-writable source directory inside the production closure",
    )
    parser.add_argument(
        "--target-dir",
        type=Path,
        required=True,
        help="fresh nonexistent build directory outside the production closure",
    )
    parser.add_argument(
        "--reviewed-source-closure",
        type=Path,
        required=True,
        help="root-published reviewed-source descriptor named by closure provenance",
    )
    parser.add_argument(
        "--reviewed-source-closure-sha256",
        required=True,
        help="independently pinned descriptor SHA-256",
    )
    parser.add_argument(
        "--toolchain-provenance",
        type=Path,
        required=True,
        help=(
            "root-published production-build-closure.json binding source, "
            "bootstrap, toolchain/sysroot/dylibs, GPG, and keyring"
        ),
    )
    parser.add_argument(
        "--toolchain-provenance-sha256",
        required=True,
        help="independently pinned canonical production-closure provenance SHA-256",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Build and print canonical JSON identifying the exact resulting binary."""

    args = _parser().parse_args(argv)
    try:
        report = build_candidate_bundle(
            args.root,
            "cargo",
            target_dir=args.target_dir,
            reviewed_source_closure=args.reviewed_source_closure,
            reviewed_source_closure_sha256=args.reviewed_source_closure_sha256,
            toolchain_provenance=args.toolchain_provenance,
            toolchain_provenance_sha256=args.toolchain_provenance_sha256,
        )
    except (CandidateBuildError, OSError, source_seal.SourceSealError) as error:
        print(f"sealed Kagemusha candidate build failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, ensure_ascii=True, separators=(",", ":"), sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
