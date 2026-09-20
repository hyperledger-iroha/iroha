"""Retain the fixed fifteen public control files for one scaling trial.

This is physical custody, not a successful-trial capability. The fixed experiment
owner authenticates the actual trial and its native results before supplying
original descriptors here, and retains that authority independently. Neither
file hashes nor this owner's return values authorize a scaling verdict.

TODO: wire this owner into FixedExperimentCustody's actual-trial handoff and
same-owner manifest/report validation before opening the public release gate.
"""
from __future__ import annotations

from scaling_structural_identity import pin_run_budget
from dataclasses import dataclass
import fcntl
import hashlib
import os
from pathlib import Path
import re
import stat

from resource_bundle import ControlBinding, _directory_owner, _identity, _root_path
from scaling_publication import _WRITE, _close_owned, _flags, _file_identity, digest_file, publish_file
from resource_evidence_budget import (
    PerRunResourceBudget, canonical_run_budget_bytes, parse_run_budget, run_budget_inputs,
)

_READ = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
_DIRECTORY = _READ | os.O_DIRECTORY
_HEX = re.compile(r'[0-9a-f]{64}')
_EXISTING = {
    'collector_journal': 'collector.jsonl', 'transaction_trace': 'trace.json',
    'native_finality': 'native/finality.nrt', 'native_queries': 'native/queries.nrt',
    'native_facts': 'native/facts.nrt', 'native_request': 'native/request.nrt',
    'native_bundle': 'native/bundle.nrt', 'canonical_proof': 'native/proof.nrt',
}
_GENESIS = {
    'genesis_manifest': 'genesis/genesis.json', 'signed_genesis': 'genesis/genesis.signed.nrt',
    'genesis_context': 'genesis/genesis-context.nrt',
    'genesis_network_record': 'genesis/genesis.expected_hash',
    'genesis_anchors': 'genesis/genesis-anchors.json',
}
_SUMMARIES = {'raw_run': 'raw_samples.json', 'run_receipt': 'run_receipt.json'}
_PATHS = _EXISTING | _GENESIS | _SUMMARIES
_SOURCE_ROLES = tuple(_EXISTING) + tuple(_GENESIS)


class PublicFileError(ValueError):
    """Static physical-custody failure, without paths or evidence contents."""


def _require(condition):
    if not condition:
        raise PublicFileError('public_file_custody_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise PublicFileError('public_file_custody_failed') from None








@dataclass(frozen=True, slots=True)
class PublicFile:
    """Physical binding only; native and experiment authority remain elsewhere."""
    role: str
    binding: ControlBinding
    bytes: int
    max_bytes: int


@dataclass(slots=True)
class _File:
    fd: int
    identity: tuple
    flags: int
    public: PublicFile
    pin: tuple
    readonly: bool


class RunPublicFiles:
    """Original file/ancestor custody from before the trial through final reads.

    All source admissions and ``seal_sources`` use the caller's unchanged timed
    guard. The caller may close private trial owners only after successful seal
    and its own final deadline/authority check. Later physical verification has
    no private callback or deadline, and cannot re-admit files or issue a verdict.
    """

    def __init__(self, directory: Path, allocation: PerRunResourceBudget):
        self._chain, self._directories, self._files = [], {}, {}
        self._failed, self._closed, self._busy = False, False, False
        self._sealed, self._published = False, False
        self._states = None
        try:
            _require(type(allocation) is PerRunResourceBudget)
            # Own all nested primitive values; re-admission alone can alias them.
            self._allocation = parse_run_budget(run_budget_inputs(allocation))
            self._budget = canonical_run_budget_bytes(self._allocation)
            self._budget_identity = pin_run_budget(self._allocation, self._budget)
            self._directory = Path(_root_path(directory))
            run = self._allocation.run
            self._prefix = f'runs/pair-{run.pair_index:02}/{run.variant}'
            _require(tuple(self._directory.parts[-3:]) == tuple(self._prefix.split('/')))
            self._caps = {role: (getattr(run, role).label, getattr(run, role).max_bytes)
                          for role in _PATHS}
            self._scope = (str(self._directory), self._prefix, tuple(self._caps.items()), self._budget)
            parent = None
            for index, part in enumerate(self._directory.parts):
                name = '/' if index == 0 else part
                fd = os.open(name, _DIRECTORY, dir_fd=parent)
                try:
                    info = os.fstat(fd)
                    _require(stat.S_ISDIR(info.st_mode))
                    identity = _directory_owner(info)
                    _require(_directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity)
                    self._chain.append((fd, parent, name, identity))
                except BaseException:
                    os.close(fd)
                    raise
                parent = fd
            self._root = self._chain[-1][0]
            info = os.fstat(self._root)
            _require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
            _require(self._names(self._root, 0) == ())
            self._root_original = _directory_owner(info)
            self._check()
        except BaseException as error:
            self._failed = True
            self.close()
            _failure(error)

    @staticmethod
    def _names(fd, cap):
        names = []
        with os.scandir(fd) as entries:
            for entry in entries:
                _require(len(names) < cap)
                names.append(entry.name)
        return tuple(sorted(names))

    @property
    def directory(self) -> Path:
        """Return the original public root inside its unchanged physical scope."""
        try:
            self._check()
            return self._directory
        except BaseException as error:
            self._failed = True
            _failure(error)

    @property
    def allocation(self) -> PerRunResourceBudget:
        """Return an owned copy of the exact admitted canonical allocation."""
        try:
            self._check()
            return parse_run_budget(run_budget_inputs(self._allocation))
        except BaseException as error:
            self._failed = True
            _failure(error)

    def _check(self):
        _require(not self._failed and not self._closed)
        _require((str(self._directory), self._prefix, tuple(self._caps.items()),
                  self._budget_identity.checked_bytes(self._allocation, self._budget)) == self._scope)
        for fd, parent, name, identity in self._chain:
            _flags(fd, readonly=True)
            _require(_directory_owner(os.fstat(fd)) == identity
                     and _directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity)
        for name, (fd, identity) in self._directories.items():
            _flags(fd, readonly=True)
            _require(_directory_owner(os.fstat(fd)) == identity
                     and _directory_owner(os.stat(name, dir_fd=self._root, follow_symlinks=False)) == identity)
        self._check_files()
        if self._states is not None:
            _require(self._directory_states() == self._states)

    def _check_files(self):
        for role, item in self._files.items():
            public = item.public
            _require((public.role, public.binding.label, public.binding.path,
                      public.binding.sha256, public.bytes, public.max_bytes) == item.pin)
            identity, flags = _file_identity(item.fd, public.max_bytes, readonly=item.readonly)
            parent, name = self._parent(role)
            _require(identity == item.identity and flags == item.flags
                     and _identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == item.identity)

    def _parent(self, role):
        parts = _PATHS[role].split('/')
        return (self._root, parts[0]) if len(parts) == 1 else (self._directories[parts[0]][0], parts[1])

    def _directory_states(self):
        return (_identity(os.fstat(self._root)),
                tuple((name, _identity(os.fstat(fd))) for name, (fd, _) in sorted(self._directories.items())))

    def _enter(self, guard):
        _require(callable(guard) and not self._busy)
        self._busy = True
        self._guard(guard)

    def _guard(self, guard):
        self._check()
        guard()
        self._check()

    def _digest(self, fd, identity, guard):
        return digest_file(fd, identity, lambda: self._guard(guard))

    def _duplicate(self, fd, identity):
        _flags(fd, readonly=True)
        _require(_identity(os.fstat(fd)) == identity)
        duplicate = os.dup(fd)
        try:
            _flags(duplicate, readonly=True)
            _require(_identity(os.fstat(duplicate)) == identity and _identity(os.fstat(fd)) == identity)
            return duplicate
        except BaseException:
            _close_owned(duplicate, identity[:2])
            raise

    def _native_directory(self, fd):
        flags = _flags(fd, readonly=True)
        info = os.fstat(fd)
        _require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid()
                 and stat.S_IMODE(info.st_mode) == 0o700)
        identity = _identity(info)
        _require(_identity(os.stat('native', dir_fd=self._root, follow_symlinks=False)) == identity)
        if 'native' in self._directories:
            original, _ = self._directories['native']
            _require(_identity(os.fstat(original)) == identity and fcntl.fcntl(original, fcntl.F_GETFL) == flags)
        else:
            duplicate = self._duplicate(fd, identity)
            self._directories['native'] = (duplicate, _directory_owner(info))

    def _record(self, role, fd, identity, digest, readonly):
        label, cap = self._caps[role]
        binding = ControlBinding(label, f'{self._prefix}/{_PATHS[role]}', digest)
        public = PublicFile(role, binding, identity[6], cap)
        pin = (role, binding.label, binding.path, binding.sha256, public.bytes, cap)
        self._files[role] = _File(fd, identity, _flags(fd, readonly=readonly), public, pin, readonly)

    def adopt_existing(self, role: str, descriptor: int, sha256: str, size: int,
                       *, native_parent: int | None, guard):
        """Duplicate one of eight original load/native FDs under timed authority.

        The exact completed native parent's original FD is mandatory for native
        outputs and forbidden for journal/trace. No file is reopened by pathname.
        """
        duplicate, identity = None, None
        try:
            self._enter(guard)
            _require(not self._sealed and type(role) is str and role in _EXISTING and role not in self._files)
            _require(type(sha256) is str and _HEX.fullmatch(sha256)
                     and type(size) is int and 0 < size <= self._caps[role][1])
            if _EXISTING[role].startswith('native/'):
                self._native_directory(native_parent)
            else:
                _require(native_parent is None)
            identity, _ = _file_identity(descriptor, self._caps[role][1], readonly=True)
            _require(identity[6] == size and identity[:2] not in {item.identity[:2] for item in self._files.values()})
            parent, name = self._parent(role)
            before = _identity(os.fstat(parent))
            _require(_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity)
            duplicate = self._duplicate(descriptor, identity)
            _require(self._digest(duplicate, identity, guard) == sha256)
            self._guard(guard)
            _require(_identity(os.fstat(descriptor)) == identity
                     and _identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity
                     and _identity(os.fstat(parent)) == before)
            self._record(role, duplicate, identity, sha256, True)
            duplicate = None
            self._guard(guard)
            self._busy = False
        except BaseException as error:
            self._failed = True
            if duplicate is not None: _close_owned(duplicate, identity[:2])
            _failure(error)

    def _genesis_directory(self):
        if 'genesis' in self._directories: return
        os.mkdir('genesis', mode=0o700, dir_fd=self._root)
        fd = os.open('genesis', _DIRECTORY, dir_fd=self._root)
        original = None
        try:
            info = os.fstat(fd)
            original = _identity(info)[:2]
            _require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700
                     and _identity(os.stat('genesis', dir_fd=self._root, follow_symlinks=False)) == _identity(info)
                     and self._names(fd, 0) == ())
            self._directories['genesis'] = (fd, _directory_owner(info))
            os.fsync(self._root)
        except BaseException:
            if original is not None: _close_owned(fd, original)
            raise

    def _publish(self, role, size, expected_sha, read_chunk, check_source, guard):
        parent, name = self._parent(role)
        expected_names = {self._parent(existing)[1] for existing in self._files if self._parent(existing)[0] == parent}
        if parent == self._root:
            expected_names.update(self._directories)
        created = publish_file(parent, name, size, self._caps[role][1], expected_sha,
                               read_chunk, check_source, lambda: self._guard(guard), frozenset(expected_names))
        try:
            self._record(role, created.fd, created.identity, expected_sha, False)
        except BaseException:
            _close_owned(created.fd, created.identity[:2])
            raise

    def copy_genesis(self, role: str, descriptor: int, sha256: str, size: int, *, guard):
        """Copy exactly one public genesis original; private TOMLs have no role."""
        duplicate, identity = None, None
        try:
            self._enter(guard)
            _require(not self._sealed and type(role) is str and role in _GENESIS and role not in self._files)
            _require(type(sha256) is str and _HEX.fullmatch(sha256)
                     and type(size) is int and 0 < size <= self._caps[role][1])
            identity, flags = _file_identity(descriptor, self._caps[role][1], readonly=True)
            _require(identity[6] == size and identity[:2] not in {item.identity[:2] for item in self._files.values()})
            duplicate = self._duplicate(descriptor, identity)
            def source():
                _require(_identity(os.fstat(descriptor)) == identity
                         and _identity(os.fstat(duplicate)) == identity
                         and _flags(descriptor, readonly=True) == flags
                         and _flags(duplicate, readonly=True) == flags)
            self._genesis_directory()
            self._publish(role, size, sha256, lambda offset, count: os.pread(duplicate, count, offset), source, guard)
            self._guard(guard); source()
            _close_owned(duplicate, identity[:2]); duplicate = None
            self._busy = False
        except BaseException as error:
            self._failed = True
            if duplicate is not None: _close_owned(duplicate, identity[:2])
            _failure(error)

    def _namespace(self):
        root = {'native', 'genesis', 'collector.jsonl', 'trace.json'}
        root.update(_SUMMARIES[role] for role in _SUMMARIES if role in self._files)
        _require(set(self._names(self._root, 6)) == root)
        for directory, roles in (('native', tuple(role for role in _EXISTING if _EXISTING[role].startswith('native/'))),
                                 ('genesis', tuple(_GENESIS))):
            fd = self._directories[directory][0]
            _require(set(self._names(fd, 6)) == {_PATHS[role].split('/')[1] for role in roles})

    def _verify(self, guard):
        self._guard(guard)
        before = self._directory_states()
        for role, item in self._files.items():
            _require(self._digest(item.fd, item.identity, guard) == item.public.binding.sha256)
        # Hashing later large files cannot hide an earlier in-place change.
        self._guard(guard)
        self._check_files()
        _require(self._directory_states() == before)
        self._namespace()
        self._check()

    def seal_sources(self, *, guard) -> tuple[PublicFile, ...]:
        """Require thirteen original/copy controls before private custody closes."""
        try:
            self._enter(guard)
            _require(not self._sealed and set(self._files) == set(_SOURCE_ROLES))
            self._verify(guard)
            self._states = self._directory_states()
            self._guard(guard)
            self._sealed = True
            self._busy = False
            return tuple(self._files[role].public for role in _SOURCE_ROLES)
        except BaseException as error:
            self._failed = True
            _failure(error)

    def publish_summary(self, role: str, raw: bytes, *, guard):
        """Bounded publication of one outer-owner-derived public JSON encoding.

        The outer fixed publisher owns serialization and semantic reconciliation.
        This physical writer never accepts a claimed success field as authority.
        """
        try:
            self._enter(guard)
            _require(self._sealed and not self._published and type(role) is str and role in _SUMMARIES
                     and role not in self._files and type(raw) is bytes and 0 < len(raw) <= self._caps[role][1])
            expected_role = 'raw_run' if 'raw_run' not in self._files else 'run_receipt'
            _require(role == expected_role)
            self._verify(guard)
            previous = self._states
            self._states = None
            self._publish(role, len(raw), hashlib.sha256(raw).hexdigest(),
                          lambda offset, count: raw[offset:offset + count], lambda: None, guard)
            # Only the exact root leaf changed; nested native/genesis parents and
            # every earlier control retain their complete pre-publication state.
            _require(self._directory_states()[1] == previous[1])
            self._states = self._directory_states()
            self._verify(guard)
            self._published = set(self._files) == set(_PATHS)
            self._busy = False
        except BaseException as error:
            self._failed = True
            _failure(error)

    def verify(self) -> tuple[PublicFile, ...]:
        """Recheck the same source admission without invoking expired trial code."""
        try:
            self._enter(lambda: None)
            _require(self._sealed)
            self._verify(lambda: None)
            self._busy = False
            return tuple(self._files[role].public for role in _PATHS if role in self._files)
        except BaseException as error:
            self._failed = True
            _failure(error)

    def read_control(self, binding: ControlBinding, *, max_bytes: int) -> bytes:
        """Read an original bound control under a narrower semantic byte limit."""
        try:
            self._enter(lambda: None)
            _require(self._sealed and type(binding) is ControlBinding
                     and type(max_bytes) is int and max_bytes >= 0)
            items = tuple(item for item in self._files.values() if item.public.binding is binding)
            _require(len(items) == 1)
            item = items[0]
            _require(item.public.bytes <= max_bytes <= item.public.max_bytes)
            before = self._directory_states()
            chunks, offset = [], 0
            while offset < item.public.bytes:
                self._check()
                count = min(65536, item.public.bytes - offset)
                raw = os.pread(item.fd, count, offset)
                _require(0 < len(raw) <= count)
                chunks.append(raw); offset += len(raw)
            raw = b''.join(chunks)
            _require(hashlib.sha256(raw).hexdigest() == item.public.binding.sha256)
            self._check()
            _require(self._directory_states() == before)
            self._busy = False
            return raw
        except BaseException as error:
            self._failed = True
            _failure(error)

    def close(self):
        """Release only this owner's FDs; leave originals and diagnostic files."""
        if self._closed: return
        self._closed, self._failed = True, True
        owned = [(item.fd, item.identity[:2]) for item in self._files.values()]
        owned.extend((fd, identity[:2]) for fd, identity in self._directories.values())
        owned.extend((fd, identity[:2]) for fd, _, _, identity in reversed(self._chain))
        for fd, inode in owned: _close_owned(fd, inode)
        self._files.clear(); self._directories.clear(); self._chain.clear()

    def __enter__(self): return self

    def __exit__(self, *_): self.close()
