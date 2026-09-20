"""Retain the six fixed native scaling outputs across preparation and replay.

Every allocation is admitted before creating a fresh private directory. Only the
current fixed command's exact stage/final names may appear. After native terminal
success, the caller supplies the independently checked reply identities and this
owner retains, hashes and seals every output in that command together. No JSON
reply or filesystem census substitutes for the native command's semantic checks.

TODO: compose with fixed collection/facts/prepare/export commands and the final
experiment ledger before permitting any public scaling qualification.
"""
from __future__ import annotations

from dataclasses import dataclass
import fcntl
import hashlib
import os
from pathlib import Path
import re
import stat

from resource_bundle import _directory_owner, _identity, _root_path

MAX_FILE_BYTES = 256 * 1024 * 1024
MAX_TOTAL_BYTES = 2 * 1024 * 1024 * 1024
_READ = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
_DIRECTORY = _READ | os.O_DIRECTORY
_HEX = re.compile(r'[0-9a-f]{64}')
_STEPS = (('collection', ('finality', 'queries')), ('facts', ('facts',)),
          ('prepare', ('request', 'bundle')), ('export', ('proof',)))
_ROLES = tuple(role for _, roles in _STEPS for role in roles)
_STAGES = {role: '.collecting' if role in ('finality', 'queries') else '.publishing' for role in _ROLES}


class NativeOutputError(ValueError):
    """Closed failure code with no original path, output content or credentials."""


class _ActiveRenameObservation(NativeOutputError):
    """A scan saw both fixed names; only a changed active namespace may retry."""


def _require(value, code):
    if not value:
        raise NativeOutputError(code)


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    if type(error) is NativeOutputError and str(error).startswith('native_output_'):
        raise NativeOutputError(str(error)) from None
    raise NativeOutputError('native_output_failed') from None


def _integer(value, minimum, maximum):
    _require(type(value) is int and minimum <= value <= maximum, 'native_output_integer_invalid')
    return value


@dataclass(frozen=True, slots=True)
class NativeOutputBudget:
    """Independent per-file caps and total allocation for one complete run."""
    finality: int
    queries: int
    facts: int
    request: int
    bundle: int
    proof: int
    total: int


def _budget(value):
    _require(type(value) is NativeOutputBudget, 'native_output_budget_invalid')
    caps = tuple(_integer(getattr(value, name), 1, MAX_FILE_BYTES) for name in _ROLES)
    total = _integer(value.total, 1, MAX_TOTAL_BYTES)
    _require(sum(caps) <= total, 'native_output_total_reservation')
    return dict(zip(_ROLES, caps, strict=True)), total


@dataclass(frozen=True, slots=True)
class PublishedIdentity:
    """One identity from the original successful native command's checked reply."""
    role: str
    sha256: str
    bytes: int


@dataclass(frozen=True, slots=True)
class RetainedOutput:
    """Public artifact identity; the originating owner keeps its original FD."""
    role: str
    path: Path
    sha256: str
    bytes: int
    max_bytes: int


@dataclass(slots=True)
class _File:
    fd: int
    identity: tuple
    flags: int
    sha256: str
    bytes: int


class NativeOutputs:
    """One fresh namespace and serial collection/facts/prepare/export publication.

    The caller keeps this owner open through native proof replay and final bundle
    validation, and reaps its own native commands before closing the owner. This
    class never starts, discovers, signals or adopts a process, and never deletes
    outputs. A failed operation permanently closes admission but retains its FDs
    until close(), including files captured before a later file failed.
    """

    def __init__(self, directory: Path, budget: NativeOutputBudget):
        _require(not hasattr(self, '_chain'), 'native_output_readmission')
        self._chain, self._files = [], {}
        self._failed, self._closed, self._busy = False, False, False
        self._active, self._step = None, 0
        try:
            self._caps, self._total = _budget(budget)
            self._directory = Path(_root_path(directory))
            _require(self._directory.parent != self._directory, 'native_output_root_invalid')
            self._retain_directory('/', None)
            for part in self._directory.parts[1:-1]:
                self._retain_directory(part, self._chain[-1][0])
            parent = self._chain[-1][0]
            os.mkdir(self._directory.name, mode=0o700, dir_fd=parent)
            self._retain_directory(self._directory.name, parent)
            self._root = self._chain[-1][0]
            info = os.fstat(self._root)
            _require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700,
                     'native_output_root_owner')
            self.validate()
        except BaseException as error:
            self._failed = True
            self.close()
            _failure(error)

    @property
    def directory(self):
        """Original absolute output namespace; never changed by a native reply."""
        return self._directory

    def _retain_directory(self, name, parent):
        fd = os.open(name, _DIRECTORY, dir_fd=parent)
        try:
            identity = _directory_owner(os.fstat(fd))
            _require(stat.S_ISDIR(identity[2]), 'native_output_directory_type')
            if parent is not None:
                _require(_directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity,
                         'native_output_directory_changed')
            self._chain.append((fd, parent, name, identity))
        except BaseException:
            os.close(fd)
            raise

    def _directories(self):
        _require(not self._closed and not self._failed and bool(self._chain), 'native_output_closed')
        for fd, parent, name, identity in self._chain:
            _require(_directory_owner(os.fstat(fd)) == identity,
                     'native_output_directory_changed')
            if parent is not None:
                _require(_directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity,
                         'native_output_directory_changed')

    def _name(self, role):
        _require(type(role) is str and role in self._caps, 'native_output_role_invalid')
        return role + '.nrt'

    def path(self, role: str) -> Path:
        """Derive one fixed original destination; no command-chosen path is accepted."""
        try:
            self.validate()
            return self._directory / self._name(role)
        except BaseException as error:
            self._failed = True
            _failure(error)

    def allocation(self, role: str) -> int:
        """Return the original cap for a fixed role before its writer begins."""
        try:
            self.validate()
            self._name(role)
            return self._caps[role]
        except BaseException as error:
            self._failed = True
            _failure(error)

    def _metadata(self, info, cap):
        _require(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid()
                 and stat.S_IMODE(info.st_mode) == 0o600 and info.st_nlink == 1,
                 'native_output_file_type_or_owner')
        _require(0 <= info.st_size <= cap, 'native_output_file_cap')

    def _sealed_files(self):
        _require(not self._closed and not self._failed, 'native_output_unavailable')
        for role, item in self._files.items():
            _require(_identity(os.fstat(item.fd)) == item.identity
                     and fcntl.fcntl(item.fd, fcntl.F_GETFL) == item.flags
                     and _identity(os.stat(self._name(role), dir_fd=self._root, follow_symlinks=False)) == item.identity,
                     'native_output_original_changed')

    def _scan(self):
        allowed = {self._name(role): role for role in self._files}
        active = () if self._active is None else self._active
        for role in active:
            allowed[self._name(role)] = role
            allowed[self._name(role) + _STAGES[role]] = role
        seen, sizes = set(), {}
        with os.scandir(self._root) as entries:
            for entry in entries:
                _require(entry.name in allowed and entry.name not in seen
                         and len(seen) < len(allowed), 'native_output_unallocated_entry')
                seen.add(entry.name)
                info = os.stat(entry.name, dir_fd=self._root, follow_symlinks=False)
                role = allowed[entry.name]
                self._metadata(info, self._caps[role])
                if role in sizes:
                    raise _ActiveRenameObservation('native_output_stage_and_final_coexist')
                sizes[role] = info.st_size
        _require(all(self._name(role) in seen for role in self._files), 'native_output_original_missing')
        _require(sum(sizes.values()) <= self._total, 'native_output_total_cap')
        return seen

    def _validate(self):
        self._directories()
        self._sealed_files()
        # Active native writers can atomically rename only their fixed names.
        # Retry a namespace-changing scan, never a sealed-file identity mismatch.
        # Completed publication has no concurrent writer and permits no retry.
        for _ in range(8 if self._active is not None else 1):
            before = _identity(os.fstat(self._root))
            try:
                self._scan()
            except (FileNotFoundError, _ActiveRenameObservation):
                _require(self._active is not None and _identity(os.fstat(self._root)) != before,
                         'native_output_namespace_changed')
                self._sealed_files()
                continue
            self._sealed_files()
            self._directories()
            after = _identity(os.fstat(self._root))
            if after == before:
                return
            _require(self._active is not None, 'native_output_namespace_changed')
        raise NativeOutputError('native_output_namespace_unstable')

    def _enter(self):
        _require(not self._failed and not self._closed and not self._busy,
                 'native_output_unavailable')
        self._busy = True

    def validate(self):
        """Recheck original namespace/FDs and every currently admitted byte cap."""
        try:
            self._enter()
            self._validate()
            self._busy = False
        except BaseException as error:
            self._failed = True
            _failure(error)

    def begin(self, step: str):
        """Open exactly the next native command's fixed publication slots."""
        try:
            self._enter()
            self._validate()
            _require(self._active is None and self._step < len(_STEPS)
                     and type(step) is str and step == _STEPS[self._step][0],
                     'native_output_step_order')
            self._active = _STEPS[self._step][1]
            self._validate()
            self._busy = False
        except BaseException as error:
            self._failed = True
            _failure(error)

    def _capture(self, role, digest, size):
        name = self._name(role)
        fd = os.open(name, _READ, dir_fd=self._root)
        try:
            info = os.fstat(fd)
            self._metadata(info, self._caps[role])
            _require(info.st_size == size, 'native_output_reply_length_mismatch')
            flags = fcntl.fcntl(fd, fcntl.F_GETFL)
            _require(flags & os.O_ACCMODE == os.O_RDONLY, 'native_output_descriptor_access')
            item = _File(fd, _identity(info), flags, digest, size)
            # Transfer ownership before any later read/callback can fail.
            self._files[role] = item
        except BaseException:
            os.close(fd)
            raise
        actual, offset = hashlib.sha256(), 0
        while offset < size:
            raw = os.pread(fd, min(65536, size - offset), offset)
            _require(bool(raw), 'native_output_truncated')
            actual.update(raw)
            offset += len(raw)
        _require(actual.hexdigest() == digest, 'native_output_reply_digest_mismatch')
        self._sealed_files()

    def complete(self, replies: tuple[PublishedIdentity, ...]) -> tuple[RetainedOutput, ...]:
        """Retain the entire successful command's outputs as one checked group.

        Call only after the fixed native owner has reaped terminal exit zero and
        validated its bounded identity reply. Any missing/stale stage, prefix or
        later-file failure makes every earlier file in the group unavailable.
        """
        try:
            self._enter()
            self._validate()
            active = self._active
            _require(active is not None and type(replies) is tuple and len(replies) == len(active)
                     and all(type(row) is PublishedIdentity for row in replies)
                     and tuple(row.role for row in replies) == active, 'native_output_reply_roles')
            inputs = []
            for row in replies:
                _require(type(row.sha256) is str and _HEX.fullmatch(row.sha256) is not None,
                         'native_output_reply_digest')
                inputs.append((row.role, row.sha256, _integer(row.bytes, 1, self._caps[row.role])))
            before = _identity(os.fstat(self._root))
            expected = {self._name(role) for role in (*self._files, *active)}
            _require(self._scan() == expected, 'native_output_publication_incomplete')
            for role, digest, size in inputs:
                self._capture(role, digest, size)
            self._sealed_files()
            _require(_identity(os.fstat(self._root)) == before,
                     'native_output_namespace_changed_during_capture')
            self._active = None
            self._step += 1
            self._validate()
            result = tuple(self._artifact(role) for role in active)
            self._busy = False
            return result
        except BaseException as error:
            self._failed = True
            _failure(error)

    def _artifact(self, role):
        item = self._files[role]
        return RetainedOutput(role, self._directory / self._name(role), item.sha256, item.bytes, self._caps[role])

    def artifact(self, role: str) -> RetainedOutput:
        """Read a completed artifact identity inside its still-retained scope."""
        try:
            self.validate()
            _require(type(role) is str and role in self._files, 'native_output_not_completed')
            return self._artifact(role)
        except BaseException as error:
            self._failed = True
            _failure(error)

    def descriptor(self, role: str) -> int:
        """Borrow, without transferring ownership, an original read-only descriptor."""
        self.artifact(role)
        return self._files[role].fd

    def directory_descriptor(self) -> int:
        """Borrow the original output parent only after all six outputs complete.

        The receiving owner duplicates this descriptor before this owner closes.
        It must still authenticate the exact named edge and all six original
        files; a directory descriptor alone establishes no native proof result.
        """
        self.finish()
        return self._root

    def finish(self) -> tuple[RetainedOutput, ...]:
        """Require all six artifacts while preserving their original custody."""
        try:
            self.validate()
            _require(self._active is None and self._step == len(_STEPS), 'native_output_pipeline_incomplete')
            return tuple(self._artifact(role) for role in _ROLES)
        except BaseException as error:
            self._failed = True
            _failure(error)

    def close(self):
        """Close retained descriptors only; preserve successful and failed outputs."""
        if self._closed: return
        self._closed, self._failed = True, True
        owned = [(item.fd, item.identity[:2]) for item in self._files.values()]
        owned.extend((fd, identity[:2]) for fd, _, _, identity in reversed(self._chain))
        for fd, identity in owned:
            try:
                if _identity(os.fstat(fd))[:2] == identity:
                    os.close(fd)
            except OSError:
                pass
        self._files.clear()
        self._chain.clear()

    def __enter__(self): return self

    def __exit__(self, *_): self.close()
