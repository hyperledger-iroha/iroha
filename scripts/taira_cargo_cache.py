"""Admit source-bound Cargo fingerprints without deleting compiled artifacts.

Cargo 1.93 encodes files below its target directory relative to that directory,
before considering the package root. A fingerprint from another frozen source
can consequently remain fresh forever. Inspect those paths under Cargo's own
profile locks and retain foreign local-package fingerprints outside its lookup
namespace. Registry/git artifacts and compiled outputs remain available.
"""

from __future__ import annotations

import contextlib
import fcntl
import json
import os
from pathlib import Path
import re
import stat
import struct
import subprocess
import uuid


def dependency_paths(raw: bytes) -> list[tuple[int, Path]]:
    """Read the pinned Cargo 1.93 version-one dependency record, including EOF."""
    offset = 0

    def take(size: int) -> bytes:
        nonlocal offset
        if size < 0 or offset + size > len(raw):
            raise ValueError("truncated Cargo dependency record")
        value = raw[offset:offset + size]
        offset += size
        return value

    def number() -> int:
        return struct.unpack("<I", take(4))[0]

    def blob() -> bytes:
        return take(number())

    if take(6) != b"\x01\x00\x00\x00\xff\x01":
        raise ValueError("unsupported Cargo dependency record")
    paths = []
    for _ in range(number()):
        kind = take(1)[0]
        if kind not in (0, 1):
            raise ValueError("invalid Cargo dependency path kind")
        value = blob()
        if not value or b"\0" in value:
            raise ValueError("invalid Cargo dependency path")
        paths.append((kind, Path(os.fsdecode(value))))
        checksum = take(1)[0]
        if checksum == 1:
            take(8)
            blob()
        elif checksum != 0:
            raise ValueError("invalid Cargo dependency checksum flag")
    for _ in range(number()):
        blob()  # Names/values are never emitted into diagnostics.
        present = take(1)[0]
        if present == 1:
            blob()
        elif present != 0:
            raise ValueError("invalid Cargo dependency environment flag")
    if offset != len(raw):
        raise ValueError("trailing Cargo dependency record bytes")
    return paths


def local_package_names(source: Path, environment: dict[str, str]) -> set[str]:
    result = subprocess.run(
        [environment["CARGO"], "--config", str(source / ".cargo/config.toml"),
         "metadata", "--manifest-path", str(source / "Cargo.toml"),
         "--locked", "--offline", "--format-version=1"],
        cwd="/", env=environment, stdin=subprocess.DEVNULL, capture_output=True,
        check=True, timeout=60,
    )
    return {package["name"] for package in json.loads(result.stdout)["packages"]
            if package["source"] is None}


def private_regular(path: Path, maximum: int) -> bytes:
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        info = os.fstat(fd)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                or info.st_nlink != 1 or info.st_mode & 0o022 or info.st_size > maximum):
            raise ValueError("unsafe Cargo dependency record")
        raw = os.pread(fd, info.st_size + 1, 0)
        def identity(value):
            return (value.st_dev, value.st_ino, value.st_mode, value.st_uid, value.st_gid,
                    value.st_nlink, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
        if (len(raw) != info.st_size or identity(os.fstat(fd)) != identity(info)
                or identity(path.lstat()) != identity(info)):
            raise ValueError("Cargo dependency record changed")
        return raw
    finally:
        os.close(fd)


@contextlib.contextmanager
def source_fingerprints(source: Path, target: Path, triple: str,
                        packages: set[str], *, repair: bool = True, before_retire=None):
    """Hold Cargo's profile locks through admission and the caller's capture."""
    profiles = [target / profile for profile in ("debug", "release")]
    profiles += [target / triple / profile for profile in ("debug", "release")]
    profiles = [path for path in profiles if path.is_dir()]
    generated = [profile / name for profile in profiles for name in ("build", "deps")]
    stale: set[str] = set()
    directories: list[tuple[Path, str]] = []
    if not source.is_relative_to(target):
        raise ValueError("source admission requires the maintained capture below the Cargo target")
    with contextlib.ExitStack() as stack:
        for profile in sorted(profiles):
            if profile.resolve() != profile:
                raise ValueError("Cargo profile must not traverse symlinks")
            fd = os.open(profile / ".cargo-lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
            stack.callback(os.close, fd)
            info = os.fstat(fd)
            if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                    or info.st_nlink != 1 or info.st_mode & 0o022):
                raise ValueError("unsafe Cargo profile lock")
            fcntl.flock(fd, fcntl.LOCK_EX)
        for profile in profiles:
            fingerprints = profile / ".fingerprint"
            if not fingerprints.exists():
                continue
            if fingerprints.resolve() != fingerprints:
                raise ValueError("Cargo fingerprints must not traverse symlinks")
            for directory in sorted(fingerprints.iterdir()):
                match = re.fullmatch(r"(.+)-[0-9a-f]{16}", directory.name)
                if match is None or match[1] not in packages:
                    continue
                if directory.is_symlink() or not directory.is_dir():
                    raise ValueError("unsafe local Cargo fingerprint directory")
                name = match[1]
                directories.append((directory, name))
                for record in directory.glob("dep-*"):
                    try:
                        paths = dependency_paths(private_regular(record, 16 * 1024**2))
                    except ValueError:
                        stale.add(name)
                        continue
                    for kind, path in paths:
                        if kind == 0:
                            # This capture is below target. Cargo therefore
                            # encodes every captured source as build-relative;
                            # package-relative paths belong to another checkout.
                            stale.add(name)
                            continue
                        path = (target / path).resolve()
                        if path.is_relative_to(source) or any(path.is_relative_to(p) for p in generated):
                            continue
                        stale.add(name)
        if stale and not repair:
            raise ValueError("foreign Cargo source fingerprints after build: " + ", ".join(sorted(stale)))
        if stale:
            if before_retire is not None:
                before_retire()
            parent = target / "taira-release-cache-retired"
            parent.mkdir(mode=0o700, exist_ok=True)
            if parent.resolve() != parent or stat.S_IMODE(parent.stat().st_mode) != 0o700:
                raise ValueError("unsafe Cargo fingerprint archive")
            archive = parent / uuid.uuid4().hex
            archive.mkdir(mode=0o700)
            for directory, name in directories:
                if name not in stale:
                    continue
                destination = archive / directory.relative_to(target)
                destination.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
                os.rename(directory, destination)
            print("[taira-release] retained foreign source fingerprints for "
                  + str(len(stale)) + " local packages; compiled artifacts and dependency caches retained", flush=True)
        yield sorted(stale)


def admit_source_fingerprints(source: Path, target: Path, triple: str,
                              packages: set[str], *, repair: bool = True, before_retire=None) -> list[str]:
    """Retire only foreign local-package metadata; reject it after a build."""
    with source_fingerprints(source, target, triple, packages, repair=repair,
                             before_retire=before_retire) as stale:
        return stale
