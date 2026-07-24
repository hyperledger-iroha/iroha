#!/usr/bin/env python3
"""Build the Kagemusha V4 candidate generator from one exact clean source tree."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
from typing import Callable, ContextManager, Iterator, Sequence

REPO_ROOT = Path(__file__).resolve().parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts import kagemusha_source_tree_seal as source_seal
from scripts.formal import run_sumeragi_v2_tlapm_guard as resource_guard


BINARY_NAME = "kagemusha_recursive_spend_v4_bundle"
SEALED_FEATURE = "kagemusha-candidate-source-seal"
# The measured single-rustc frontend high-water mark is about 11.466 GiB.
# Requiring 24 GiB of installed physical memory leaves slightly more than a
# two-times margin.  This is only build admission: it neither reduces compiler
# memory nor imposes a hard RSS limit once Cargo starts.
MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES = 24 * 1024 * 1024 * 1024
_REMOVED_BUILD_ENVIRONMENT = {
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_ENCODED_RUSTDOCFLAGS",
    "CARGO_HOME",
    "RUSTC",
    "RUSTC_BOOTSTRAP",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTDOC",
    "RUSTDOCFLAGS",
    "RUSTFLAGS",
    "RUSTUP_TOOLCHAIN",
}
_REMOVED_BUILD_ENVIRONMENT_PREFIXES = (
    "CARGO_BUILD_",
    "CARGO_PROFILE_",
    "CARGO_TARGET_",
)


class CandidateBuildError(RuntimeError):
    """A sealed candidate generator could not be built unambiguously."""


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


def _admitted_cargo_executable(cargo: str) -> str:
    """Resolve Cargo to one absolute, owner-controlled executable path."""

    requested = Path(cargo)
    if requested.parent == Path("."):
        located = shutil.which(cargo)
        if located is None:
            raise CandidateBuildError("Cargo executable was not found")
        requested = Path(located)
    elif not requested.is_absolute():
        requested = Path.cwd() / requested
    requested = requested.absolute()
    try:
        link_metadata = requested.lstat()
        resolved = requested.resolve(strict=True)
        executable_metadata = resolved.stat()
    except OSError as error:
        raise CandidateBuildError("Cargo executable is unavailable") from error
    if not (stat.S_ISREG(link_metadata.st_mode) or stat.S_ISLNK(link_metadata.st_mode)):
        raise CandidateBuildError("Cargo executable path has an unsafe file type")
    if link_metadata.st_uid not in (0, os.geteuid()):
        raise CandidateBuildError("Cargo executable path has an unsafe owner")
    if (
        not stat.S_ISREG(executable_metadata.st_mode)
        or executable_metadata.st_uid not in (0, os.geteuid())
        or executable_metadata.st_mode & 0o022 != 0
        or executable_metadata.st_mode & stat.S_IXUSR == 0
    ):
        raise CandidateBuildError("Cargo executable has unsafe metadata")
    return str(requested if stat.S_ISLNK(link_metadata.st_mode) else resolved)


def _sanitized_build_environment() -> dict[str, str]:
    """Remove ambient compiler, wrapper, target, profile, and flag controls."""

    environment = {
        key: value
        for key, value in os.environ.items()
        if key not in _REMOVED_BUILD_ENVIRONMENT
        and not key.startswith(_REMOVED_BUILD_ENVIRONMENT_PREFIXES)
    }
    # Empty wrapper/flag values override user Cargo configuration as well as
    # inherited environment settings without selecting another executable.
    environment.update(
        {
            "CARGO_ENCODED_RUSTFLAGS": "",
            "RUSTC_WRAPPER": "",
            "RUSTC_WORKSPACE_WRAPPER": "",
            "RUSTFLAGS": "",
        }
    )
    return environment


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
    identity_reader: Callable[[Path], source_seal.SourceIdentity] = (
        source_seal.compute_identity
    ),
    command_runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run,
    physical_memory_reader: Callable[[], int] = _physical_memory_bytes,
    build_lock: Callable[[], ContextManager[object]] = _shared_memory_heavy_build_lock,
) -> dict[str, object]:
    """Build once and prove the clean source identity stayed exact throughout."""

    root = root.resolve(strict=True)
    physical_memory_bytes = _admitted_physical_memory_bytes(physical_memory_reader)
    first = identity_reader(root)
    environment = _sanitized_build_environment()
    environment["KAGEMUSHA_BUILD_SOURCE_COMMIT"] = first.source_commit
    environment["KAGEMUSHA_BUILD_SOURCE_TREE_SHA256"] = first.source_tree_sha256
    environment["KAGEMUSHA_SOURCE_SEAL_PYTHON"] = sys.executable
    command = [
        _admitted_cargo_executable(cargo),
        "build",
        "--release",
        "--locked",
        "--target-dir",
        str(root / "target"),
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
    with build_lock():
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
    second = identity_reader(root)
    if second != first:
        raise CandidateBuildError("source identity changed during the candidate build")

    if not isinstance(completed.stdout, bytes):
        raise CandidateBuildError("Cargo did not return binary build metadata")
    target_dir = (root / "target").resolve(strict=True)
    binary = _built_binary_from_cargo_messages(completed.stdout, root, target_dir)
    sha256, size_bytes = _binary_sha256(binary)
    third = identity_reader(root)
    if third != first:
        raise CandidateBuildError("source identity changed while sealing the candidate binary")
    return {
        "binary_path": str(binary),
        "binary_sha256": sha256,
        "binary_size_bytes": size_bytes,
        "build_profile": "release",
        "minimum_build_physical_memory_bytes": (
            MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES
        ),
        "physical_memory_bytes_at_admission": physical_memory_bytes,
        "schema": "iroha.kagemusha.sealed_candidate_build.v1",
        "source_commit": first.source_commit,
        "source_tree_sha256": first.source_tree_sha256,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    parser.add_argument("--cargo", default="cargo")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Build and print canonical JSON identifying the exact resulting binary."""

    args = _parser().parse_args(argv)
    try:
        report = build_candidate_bundle(args.root, args.cargo)
    except (CandidateBuildError, OSError, source_seal.SourceSealError) as error:
        print(f"sealed Kagemusha candidate build failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, ensure_ascii=True, separators=(",", ":"), sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
