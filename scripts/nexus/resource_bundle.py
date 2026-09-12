"""Descriptor-based physical admission for complete budgeted resource bundles.

Callers independently authenticate the experiment ledger and control-file hashes.
Raw captures have one exact typed namespace; their JSON, Clock, process identity,
transaction effects and metric reductions still require their respective replay
owners. Keep this context open across those checks, then call ``verify`` before
accepting evidence. There is no unsampled or inferred-allocation input form.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import stat

from resource_evidence_budget import (
    BudgetError, CAPTURE_MANIFEST_BYTES, EvidenceBudget, MAX_CONTROL_FILES, MAX_FILE_BYTES,
    MAX_TOTAL_BYTES, select_run_budget,
)

_COMPONENT = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}")
_DIGEST = re.compile(r"[0-9a-f]{64}")
_MEMBER = re.compile(
    r"(preflight|sample)-([0-9]{10})(?:-peer-([0-9]{4})-(status|metrics)\.body|\.json)"
)
_DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
_FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
MAX_ROOT_BYTES = 4096
MAX_ROOT_COMPONENTS = 64


class BundleError(ValueError):
    """A static physical-bundle failure code without runtime secret material."""


def _require(condition, code):
    if not condition:
        raise BundleError(code)


def _relative(value):
    _require(type(value) is str and 0 < len(value) <= 1024, 'path_invalid')
    parts = tuple(value.split('/'))
    _require(len(parts) <= 8 and all(_COMPONENT.fullmatch(part) for part in parts)
             and parts[0] != 'resources', 'path_invalid')
    return parts


def _root_path(value):
    _require(type(value) is type(Path('/')) and value.is_absolute()
             and str(value) == os.path.abspath(value)
             and len(os.fsencode(value)) <= MAX_ROOT_BYTES
             and len(value.parts) - 1 <= MAX_ROOT_COMPONENTS, 'root_path_invalid')
    return str(value)


def _identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _directory_owner(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid)


@dataclass(frozen=True, slots=True)
class ControlBinding:
    """One exact physical control file, bound to a ledger label and trusted hash."""

    label: str
    path: str
    sha256: str

    def __post_init__(self):
        _require(type(self.label) is str and 0 < len(self.label) <= 128, 'label_invalid')
        _relative(self.path)
        _require(type(self.sha256) is str and _DIGEST.fullmatch(self.sha256), 'digest_invalid')


@dataclass(frozen=True, slots=True)
class BundleSnapshot:
    """Bounded aggregate census; raw member metadata is not retained in memory."""

    control_files: int
    resource_files: int
    directories: int
    control_bytes: int
    resource_bytes: int
    census_sha256: str

    @property
    def total_bytes(self):
        """All physical bytes, without compression or namespace discounts."""
        return self.control_bytes + self.resource_bytes


def _read_file(parent, name, metadata, cap, digest_expected, uid, capture):
    _require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1,
             'regular_single_link_file_required')
    _require(metadata.st_uid == uid, 'file_owner_invalid')
    _require(0 <= metadata.st_size <= cap <= MAX_FILE_BYTES, 'file_allocation_exceeded')
    if capture:
        _require(metadata.st_size > 0 and stat.S_IMODE(metadata.st_mode) == 0o600,
                 'capture_file_invalid')
    fd = os.open(name, _FILE_FLAGS, dir_fd=parent)
    try:
        _require(_identity(os.fstat(fd)) == _identity(metadata), 'file_changed')
        digest = hashlib.sha256()
        offset = 0
        while offset < metadata.st_size:
            chunk = os.pread(fd, min(65536, metadata.st_size - offset), offset)
            _require(bool(chunk), 'file_truncated')
            offset += len(chunk)
            digest.update(chunk)
        actual = digest.hexdigest()
        _require(digest_expected is None or actual == digest_expected, 'control_digest_mismatch')
        _require(_identity(os.fstat(fd)) == _identity(metadata)
                 and _identity(os.stat(name, dir_fd=parent, follow_symlinks=False))
                 == _identity(metadata), 'file_changed')
        return actual
    finally:
        os.close(fd)


class BudgetedBundle:
    """Retain an admitted root across independent semantic replay and final census.

    ``reported=False`` requires the reserved report to be absent; ``True``
    requires it present and hash bound like every other control. Report space is
    reserved in both stages. All remaining allocations must map one-to-one to
    actual controls. Captures live at resources/pair-NN/{one_lane,four_lane}.
    Each new context requires its own semantic replay before ``verify``; a new
    reported census cannot authenticate replay performed in an earlier context.
    """

    def __init__(self, root: Path, budget: EvidenceBudget,
                 controls: tuple[ControlBinding, ...], *, reported: bool):
        self._chain = []
        self._closed = False
        self._control_read_failed = False
        self._control_read_admission = None
        _require(type(budget) is EvidenceBudget, 'experiment_budget_required')
        # EvidenceBudget is publicly constructible: never trust derived fields.
        rebuilt = select_run_budget(budget, 1, 'one_lane').experiment
        _require(type(reported) is bool, 'stage_invalid')
        _require(type(controls) is tuple and len(controls) <= MAX_CONTROL_FILES
                 and all(type(item) is ControlBinding for item in controls), 'controls_invalid')
        for item in controls:
            item.__post_init__()
        allocations = {item.label: (item.max_bytes, False)
                       for item in (*budget.control_budgets,
                                    *(item for run in budget.runs for item in run.files))}
        allocations.update({item.label: (item.size_bytes, True) for item in budget.static_files})
        self._report_label = budget.control_budgets[1].label
        if not reported:
            del allocations[self._report_label]
        _require(len(controls) == len(allocations)
                 and {item.label for item in controls} == set(allocations), 'control_mapping_invalid')
        _require(len({item.path for item in controls}) == len(controls), 'duplicate_control_path')
        self._controls = {item.path: (item, *allocations[item.label]) for item in controls}
        self._children = {'': {}}
        for item in controls:
            prefix = ''
            parts = _relative(item.path)
            for index, component in enumerate(parts):
                child = f'{prefix}/{component}' if prefix else component
                is_directory = index + 1 < len(parts)
                previous = self._children.setdefault(prefix, {}).get(component)
                _require(previous is None or previous == is_directory, 'path_kind_conflict')
                self._children[prefix][component] = is_directory
                if is_directory:
                    self._children.setdefault(child, {})
                prefix = child
        self._children['']['resources'] = True
        self._children['resources'] = {}
        self._resource_roots = {}
        for run in budget.runs:
            pair = f'pair-{run.pair_index:02}'
            self._children['resources'][pair] = True
            parent = f'resources/{pair}'
            self._children.setdefault(parent, {})[run.variant] = True
            self._resource_roots[f'{parent}/{run.variant}'] = run
        self.budget = rebuilt
        self._control_inputs = controls
        self._reported = reported
        _root_path(root)
        self.root = root
        self._scope_pin = self._scope_identity()

        def retain(name, parent=None):
            fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent)
            try:
                owner = _directory_owner(os.fstat(fd))
                self._chain.append((fd, name if parent is not None else None, owner))
            except BaseException:
                # Ownership has not transferred if metadata or append fails.
                os.close(fd)
                raise

        try:
            # Hold every ancestor and authenticate the named edges again later.
            retain('/')
            for part in root.parts[1:]:
                retain(part, self._chain[-1][0])
            self._root_fd = self._chain[-1][0]
            self._uid = os.getuid()
            _require(os.fstat(self._root_fd).st_uid == self._uid, 'root_owner_invalid')
            self.snapshot = self._scan()
        except BaseException:
            self.close()
            raise

    def _scope_identity(self):
        # Revalidate every type/length/constructor before encoding. The bounded
        # ledger and <=256 control bindings cannot expand with raw capture count.
        admitted = select_run_budget(self.budget, 1, 'one_lane').experiment
        _require(type(self._control_inputs) is tuple and len(self._control_inputs) <= MAX_CONTROL_FILES
                 and all(type(item) is ControlBinding for item in self._control_inputs), 'controls_invalid')
        for item in self._control_inputs:
            item.__post_init__()
        _require(type(self._reported) is bool, 'stage_invalid')
        return repr((admitted, self._control_inputs, self._reported, _root_path(self.root)))

    def _validate_root(self):
        _require(not self._closed and bool(self._chain), 'bundle_closed')
        _require(not self._control_read_failed, 'control_read_failed')
        try:
            current_scope = self._scope_identity()
        except (BudgetError, BundleError):
            raise BundleError('bundle_scope_changed') from None
        _require(current_scope == self._scope_pin, 'bundle_scope_changed')
        for index, (fd, name, owner) in enumerate(self._chain):
            _require(_directory_owner(os.fstat(fd)) == owner, 'directory_owner_changed')
            if index:
                named = os.stat(name, dir_fd=self._chain[index - 1][0], follow_symlinks=False)
                _require(_directory_owner(named) == owner, 'directory_owner_changed')

    def _scan(self):
        self._validate_root()
        census = hashlib.sha256()
        counts = [0, 0, 0, 0, 0]
        # Only <=256 controls and their <=8-component ancestors survive admission.
        # Raw file metadata continues to contribute only to the existing census.
        control_files, control_directories = {}, {}
        # At most the admitted member count plus <=256 controls. Directory entry
        # buffering is bounded per run; no full raw-path/hash/stat table survives.
        inodes = set()

        def file(parent, name, relative, metadata, cap, digest, capture):
            inode = (metadata.st_dev, metadata.st_ino)
            _require(inode not in inodes, 'inode_alias')
            inodes.add(inode)
            actual = _read_file(parent, name, metadata, cap, digest, self._uid, capture)
            if not capture:
                binding, allocation, exact = self._controls[relative]
                control_files[relative] = (binding, binding.label, binding.path, binding.sha256,
                                           allocation, exact, _identity(metadata))
            counts[1 if capture else 0] += 1
            counts[4 if capture else 3] += metadata.st_size
            _require(counts[0] <= len(self._controls)
                     and counts[1] <= self.budget.resource_member_count, 'file_count_exceeded')
            _require(counts[3] + counts[4] <= self.budget.total_bytes <= MAX_TOTAL_BYTES,
                     'total_allocation_exceeded')
            census.update(repr((relative, _identity(metadata), actual)).encode('utf-8') + b'\n')

        def visit(fd, prefix, capture_root=False):
            before = os.fstat(fd)
            _require(stat.S_ISDIR(before.st_mode) and before.st_uid == self._uid,
                     'directory_invalid')
            if prefix != 'resources' and not prefix.startswith('resources/'):
                control_directories[prefix] = _identity(before)
            if capture_root:
                _require(stat.S_IMODE(before.st_mode) == 0o700, 'capture_directory_invalid')
            expected = self._children.get(prefix, {})
            maximum = self.budget.members_per_run if capture_root else len(expected)
            names = []
            with os.scandir(fd) as entries:
                for entry in entries:
                    _require(_COMPONENT.fullmatch(entry.name) is not None, 'entry_name_invalid')
                    names.append(entry.name)
                    _require(len(names) <= maximum, 'directory_member_count_exceeded')
            names.sort()
            if not capture_root:
                _require(set(names) == set(expected), 'directory_members_mismatch')
            else:
                _require(len(names) == maximum, 'capture_members_mismatch')
            capture_totals = {}
            run_bytes = 0
            for name in names:
                relative = f'{prefix}/{name}' if prefix else name
                metadata = os.stat(name, dir_fd=fd, follow_symlinks=False)
                if capture_root:
                    match = _MEMBER.fullmatch(name)
                    _require(match is not None, 'capture_member_name_invalid')
                    kind, number, peer, role = match.groups()
                    sequence = int(number)
                    _require((kind == 'preflight' and sequence == 0)
                             or (kind == 'sample' and 1 <= sequence <= self.budget.geometry.sample_count),
                             'capture_sequence_invalid')
                    cap = CAPTURE_MANIFEST_BYTES
                    if peer is not None:
                        _require(int(peer) < self.budget.geometry.peers, 'capture_peer_invalid')
                        cap = (self.budget.policy.status_body_bytes if role == 'status'
                               else self.budget.policy.metrics_body_bytes)
                    file(fd, name, relative, metadata, cap, None, True)
                    key = (kind, sequence)
                    capture_totals[key] = capture_totals.get(key, 0) + metadata.st_size
                    _require(capture_totals[key] <= self.budget.bytes_per_capture,
                             'capture_allocation_exceeded')
                    run_bytes += metadata.st_size
                    _require(run_bytes <= self.budget.resource_bytes_per_run, 'run_allocation_exceeded')
                elif expected[name]:
                    _require(stat.S_ISDIR(metadata.st_mode), 'directory_required')
                    child = os.open(name, _DIRECTORY_FLAGS, dir_fd=fd)
                    try:
                        _require(_identity(os.fstat(child)) == _identity(metadata), 'directory_changed')
                        counts[2] += 1
                        visit(child, relative, relative in self._resource_roots)
                        _require(_identity(os.stat(name, dir_fd=fd, follow_symlinks=False))
                                 == _identity(metadata), 'directory_changed')
                    finally:
                        os.close(child)
                else:
                    binding, cap, exact = self._controls[relative]
                    if exact:
                        _require(metadata.st_size == cap, 'static_size_mismatch')
                    file(fd, name, relative, metadata, cap, binding.sha256, False)
            _require(_identity(os.fstat(fd)) == _identity(before), 'directory_changed')
            census.update(repr((prefix, _identity(before))).encode('utf-8') + b'\n')

        visit(self._root_fd, '')
        self._validate_root()
        _require(counts[0] == len(self._controls)
                 and counts[1] == self.budget.resource_member_count, 'file_count_mismatch')
        if self._control_read_admission is None:
            self._control_read_admission = (control_files, control_directories)
        return BundleSnapshot(*counts, census.hexdigest())

    def read_control(self, binding: ControlBinding, *, max_bytes: int) -> bytes:
        """Read one exact admitted control under an independent semantic byte cap.

        ``binding`` must be the original object supplied to this context, with
        its admitted label, path and digest unchanged. Equal but unbound records,
        raw captures and a cap exceeding its allocation are rejected. No path or
        descriptor is returned. The caller owns strict JSON/text decoding and
        must still complete semantic replay and the final ``verify`` census.
        Every failed read permanently poisons this context, including exceptions
        which the caller catches; a later census cannot erase that failure.
        """
        try:
            return self._read_control(binding, max_bytes)
        except BaseException as error:
            self._control_read_failed = True
            if isinstance(error, OSError):
                # File names and runtime OS error material are not evidence output.
                raise BundleError('control_read_io') from None
            raise

    def _read_control(self, binding, max_bytes):
        self._validate_root()
        _require(type(binding) is ControlBinding, 'control_binding_unbound')
        binding.__post_init__()
        records, admitted_directories = self._control_read_admission
        record = records.get(binding.path)
        _require(record is not None and binding is record[0]
                 and (binding.label, binding.path, binding.sha256) == record[1:4],
                 'control_binding_unbound')
        _, _, relative, expected_digest, allocation, exact, expected_file = record
        _require(type(max_bytes) is int and 0 <= max_bytes <= allocation <= MAX_FILE_BYTES,
                 'control_read_cap_invalid')
        # The initial census authenticated this exact size; reject a narrower
        # semantic cap before opening the file or allocating its returned bytes.
        _require(expected_file[6] <= max_bytes, 'control_read_cap_exceeded')
        parts = _relative(relative)
        opened = []

        def check_directories():
            _require(_identity(os.fstat(self._root_fd)) == admitted_directories[''],
                     'control_directory_changed')
            for fd, parent, name, expected in opened:
                _require(_identity(os.fstat(fd)) == expected
                         and _identity(os.stat(name, dir_fd=parent, follow_symlinks=False))
                         == expected, 'control_directory_changed')

        try:
            check_directories()
            parent = self._root_fd
            prefix = ''
            for name in parts[:-1]:
                prefix = f'{prefix}/{name}' if prefix else name
                expected = admitted_directories[prefix]
                metadata = os.stat(name, dir_fd=parent, follow_symlinks=False)
                _require(_identity(metadata) == expected and stat.S_ISDIR(metadata.st_mode)
                         and metadata.st_uid == self._uid, 'control_directory_changed')
                child = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent)
                try:
                    _require(_identity(os.fstat(child)) == expected, 'control_directory_changed')
                    opened.append((child, parent, name, expected))
                except BaseException:
                    os.close(child)
                    raise
                parent = child
            name = parts[-1]
            metadata = os.stat(name, dir_fd=parent, follow_symlinks=False)
            _require(_identity(metadata) == expected_file, 'control_file_changed')
            _require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1
                     and metadata.st_uid == self._uid, 'control_file_invalid')
            _require(0 <= metadata.st_size <= max_bytes
                     and (not exact or metadata.st_size == allocation), 'control_read_cap_exceeded')
            fd = os.open(name, _FILE_FLAGS, dir_fd=parent)
            try:
                _require(_identity(os.fstat(fd)) == expected_file, 'control_file_changed')
                check_directories()
                chunks, offset, digest = [], 0, hashlib.sha256()
                while offset < metadata.st_size:
                    count = min(65536, metadata.st_size - offset)
                    chunk = os.pread(fd, count, offset)
                    _require(0 < len(chunk) <= count, 'control_file_truncated')
                    chunks.append(chunk)
                    digest.update(chunk)
                    offset += len(chunk)
                raw = b''.join(chunks)
                _require(digest.hexdigest() == expected_digest, 'control_digest_mismatch')
                _require(_identity(os.fstat(fd)) == expected_file
                         and _identity(os.stat(name, dir_fd=parent, follow_symlinks=False))
                         == expected_file, 'control_file_changed')
                check_directories()
                self._validate_root()
                return raw
            finally:
                os.close(fd)
        finally:
            for fd, _, _, _ in reversed(opened):
                os.close(fd)

    def verify(self):
        """Repeat the complete bounded census after all semantic validators finish."""
        _require(self._scan() == self.snapshot, 'bundle_changed')
        return self.snapshot

    def close(self):
        """Release only this scanner's retained descriptors; never mutate evidence."""
        if not self._closed:
            self._closed = True
            for fd, _, _ in reversed(self._chain):
                os.close(fd)
            self._chain.clear()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
