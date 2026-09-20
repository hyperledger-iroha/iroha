#!/usr/bin/env python3
"""Pin ordinary Cargo outputs for capture without changing their cache aliases.

Callers must hold their maintained Cargo profile and lane locks. Single-link
inputs retain the generic custody contract. A published debug/release binary may
also have exactly its conventional deps/<crate_name>-<16hex> hardlink. Both names and
parent directories remain pinned and revalidated; captured evidence stays
single-link. This module never modifies a Cargo file or consumes signing inputs.
"""
from __future__ import annotations

import contextlib
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import stat

import release_artifact_contract as contract


@dataclass(frozen=True)
class CargoFile(contract.StableFile):
    """Bind the exact two-name Cargo topology in addition to content identity."""
    alias: str
    profile_identity: tuple[int, ...]
    deps_identity: tuple[int, ...]


def _need(value, message):
    if not value:
        raise contract.ReleaseArtifactError(message)


def _identity(info):
    return tuple(getattr(info, field) for field in (
        'st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid', 'st_nlink',
        'st_size', 'st_mtime_ns', 'st_ctime_ns'))


def _directory(info):
    _need(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid()
          and not info.st_mode & 0o022, 'Cargo alias directory custody differs')
    return tuple(getattr(info, field) for field in ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid'))


@contextlib.contextmanager
def _pair(path: Path, max_size: int):
    _need(path.is_absolute() and path.parent.name in ('debug', 'release')
          and re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]{0,127}', path.name),
          'Cargo hardlink source must be the canonical published debug/release binary')
    with contextlib.ExitStack() as held:
        source, named, profile = contract._open_anchored_regular(path.parent, path.name)
        held.callback(os.close, source); held.callback(os.close, profile)
        before = os.fstat(source)
        _need(_identity(before) == _identity(named) and stat.S_ISREG(before.st_mode)
              and before.st_uid == os.geteuid() and before.st_mode & stat.S_IXUSR
              and not before.st_mode & 0o022 and before.st_nlink == 2
              and 0 < before.st_size <= max_size,
              'Cargo source must be a bounded owner-held executable with exactly two aliases')
        profile_identity = _directory(os.fstat(profile))
        deps = os.open('deps', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=profile)
        held.callback(os.close, deps)
        deps_identity = _directory(os.fstat(deps))
        _need(deps_identity == _directory(os.stat('deps', dir_fd=profile, follow_symlinks=False)),
              'Cargo deps directory changed while opening')
        entries = os.listdir(deps)
        _need(len(entries) <= 65536, 'Cargo deps directory exceeds alias inventory bound')
        aliases = []
        # rustc crate names normalize hyphens in Cargo binary names.
        crate_name = path.name.replace('-', '_')
        for name in entries:
            if not re.fullmatch(re.escape(crate_name) + r'-[0-9a-f]{16}', name):
                continue
            info = os.stat(name, dir_fd=deps, follow_symlinks=False)
            if (info.st_dev, info.st_ino) == (before.st_dev, before.st_ino):
                aliases.append(name)
        _need(len(aliases) == 1, 'Cargo source lacks its exact hashed deps alias')
        alias = aliases[0]
        other = os.open(alias, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=deps)
        held.callback(os.close, other)
        _need(_identity(os.fstat(other)) == _identity(before), 'Cargo alias changed while opening')
        try:
            yield source, before, alias, profile_identity, deps_identity
        finally:
            for current in (os.fstat(source), os.fstat(other),
                            os.stat(path.name, dir_fd=profile, follow_symlinks=False),
                            os.stat(alias, dir_fd=deps, follow_symlinks=False)):
                _need(_identity(current) == _identity(before), 'Cargo alias or source changed during capture')
            _need(_directory(os.fstat(profile)) == profile_identity
                  and _directory(os.fstat(deps)) == deps_identity
                  and _directory(os.stat('deps', dir_fd=profile, follow_symlinks=False)) == deps_identity,
                  'Cargo alias directory changed during capture')
            # Reopen every pathname from its absolute anchor, not just held
            # directory descriptors: renamed/replaced ancestors must also fail.
            for relative, parent_identity in ((path.name, profile_identity), ('deps/' + alias, deps_identity)):
                reopened, current_named, parent = contract._open_anchored_regular(path.parent, relative)
                try:
                    _need(_identity(os.fstat(reopened)) == _identity(before)
                          and _identity(current_named) == _identity(before)
                          and _directory(os.fstat(parent)) == parent_identity,
                          'Cargo source path changed during capture')
                finally:
                    os.close(reopened); os.close(parent)


def _digest(source, size):
    os.lseek(source, 0, os.SEEK_SET)
    digest, total = hashlib.sha256(), 0
    while block := os.read(source, 1024 * 1024):
        total += len(block)
        _need(total <= size, 'Cargo source grew during capture')
        digest.update(block)
    _need(total == size, 'Cargo source size changed during capture')
    return digest.hexdigest()


def cargo_hash_path(path: Path, *, max_size: int):
    """Hash a single-link source or the exact closed ordinary Cargo alias pair."""
    path = Path(path)
    if path.lstat().st_nlink == 1:
        return contract.stable_hash_path(path, max_size=max_size)
    with _pair(path, max_size) as (source, info, alias, profile, deps):
        digest = _digest(source, info.st_size)
        _need(_digest(source, info.st_size) == digest, 'Cargo source content changed during capture')
        return CargoFile(sha256=digest, size=info.st_size, mode=stat.S_IMODE(info.st_mode),
            device=info.st_dev, inode=info.st_ino, mtime_ns=info.st_mtime_ns,
            ctime_ns=info.st_ctime_ns, link_count=2, alias=alias,
            profile_identity=profile, deps_identity=deps)


@contextlib.contextmanager
def cargo_open_relative(root: Path, relative: str, *, expected):
    """Pin the previously hashed Cargo topology and rehash after descriptor copy."""
    relative = contract.canonical_relative_path(relative)
    if expected.link_count == 1:
        with contract.stable_open_relative(root, relative, expected=expected) as source:
            yield source
        return
    _need(isinstance(expected, CargoFile) and expected.link_count == 2,
          'Cargo two-alias capture requires its complete pinned identity')
    with _pair(Path(root) / relative, expected.size) as (source, info, alias, profile, deps):
        _need((info.st_dev, info.st_ino, info.st_size, stat.S_IMODE(info.st_mode),
               info.st_mtime_ns, info.st_ctime_ns, alias, profile, deps)
              == (expected.device, expected.inode, expected.size, expected.mode,
                  expected.mtime_ns, expected.ctime_ns, expected.alias,
                  expected.profile_identity, expected.deps_identity),
              'Cargo source identity or alias topology changed before copy')
        _need(_digest(source, expected.size) == expected.sha256, 'Cargo source content changed before copy')
        os.lseek(source, 0, os.SEEK_SET)
        try:
            yield source
        finally:
            _need(_digest(source, expected.size) == expected.sha256, 'Cargo source content changed during copy')
