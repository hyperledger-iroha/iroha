"""Bounded offline consistency checks for the fixed ten-run public archive.

This module never accepts a release, constructs a live experiment/authority,
reads a historical process, or checks a current deadline. Native canonical
verification and authenticated original-parent attribution remain mandatory in
its caller. Expected hashes are comparison inputs, never authority credentials.
"""
from __future__ import annotations

import base64
from dataclasses import asdict, dataclass, fields, is_dataclass
import hashlib
import fcntl
import os
from pathlib import Path
import re
import stat

from resource_bundle import ControlBinding, _directory_owner, _identity, _root_path
from resource_evidence_budget import (EvidenceBudget, RUN_FILE_FIELDS, CAPTURE_MANIFEST_BYTES,
    select_run_budget, parse_run_budget, run_budget_inputs)
from resource_process import ProcessIdentity
from resource_replay import (ReplayGeometry, ExpectedPeer, replay, _json, _peer_identity)
from scaling_canonical_proof import CanonicalReplayReceipt, _digest as _native_digest
from scaling_completed_authority import PublicRequest, ResourceSnapshot
from scaling_experiment_final_projection import (RunManifest, manifest_bytes, report_bytes,
    MAX_PROJECTION_BYTES, MANIFEST_SCHEMA)
from scaling_experiment_plan import ExperimentPlan, RUN_KEYS, admit_plan, encode
from scaling_measurements import MeasurementRun, ExperimentMeasurements, measure_data_experiment
from scaling_public_files import PublicFile, _PATHS, _SOURCE_ROLES
from scaling_readiness import ReadyReceipt, MAX_ATTESTATION_BYTES
from scaling_readiness_inputs import _hash as _identity_hash, _ED25519, _BLS
from scaling_trial_captures import CaptureCensus
from scaling_worker_sources import SOURCE_NAMES, MAX_SOURCE_BYTES

_READ = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
_DIRECTORY = _READ | os.O_DIRECTORY
_OBLIGATIONS = ('canonical_native_cryptographic_replay', 'authenticated_original_parent_execution')
_PREFIX = 'iroha.sumeragi_v2.multilane_scaling.'


class ArchiveDataError(ValueError):
    """Closed archive-data failure with no evidence contents or authority claim."""


def _require(value):
    if not value:
        raise ArchiveDataError('scaling_archive_data_invalid')


def _object(value, names):
    _require(type(value) is dict and set(value) == set(names))
    return value


def _integer(value, minimum=0, maximum=(1 << 64) - 1):
    _require(type(value) is int and minimum <= value <= maximum)
    return value


def _text(value, maximum=512):
    _require(type(value) is str and 0 < len(value) <= maximum
             and all(32 <= ord(char) < 127 for char in value))
    return value


def _digest(value):
    _require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None)
    return value


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _canonical(raw, maximum):
    value = _json(raw, maximum)
    _require(encode(value, maximum) == raw)
    return value


@dataclass(frozen=True, slots=True)
class ArchiveBindings:
    """Expected byte commitments; their authenticated provenance is external."""
    manifest_sha256: str
    report_sha256: str
    inventory_sha256: str


@dataclass(frozen=True, slots=True)
class ArchiveInventory:
    """Portable lexical path/size/content census, without historical metadata."""
    files: int
    bytes: int
    sha256: str


@dataclass(frozen=True, slots=True)
class NativeArchiveRun:
    """Bounded recorded native inputs awaiting the canonical Rust verifier.

    File bindings and this receipt projection describe unverified archived data.
    A native adapter must independently retain/recheck its exact bounded inputs,
    reconstruct the actual TrustedRunPlan using its canonical owners, and replay
    canonical Norito. This record is neither a verification result nor authority.
    """
    pair_index: int
    variant: str
    files: tuple[PublicFile, ...]
    generation_json: bytes
    canonical_receipt_json: bytes
    readiness_json: bytes
    measurement: MeasurementRun


@dataclass(frozen=True, slots=True)
class ArchiveDataChecks:
    """Data consistency only; deliberately has no PASS or release verdict field."""
    bindings: ArchiveBindings
    inventory: ArchiveInventory
    measurements: ExperimentMeasurements
    native_runs: tuple[NativeArchiveRun, ...]
    identity_json: bytes
    source_closure_json: bytes
    required_obligations: tuple[str, ...] = _OBLIGATIONS


def _layout(budget):
    """Derive the complete namespace from the existing admitted budget."""
    files = {'manifest.json': budget.control_budgets[0].max_bytes,
             'report.json': budget.control_budgets[1].max_bytes}
    files.update((f'inputs/{item.label}.json', item.size_bytes) for item in budget.static_files)
    for pair, variant in RUN_KEYS:
        allocation = select_run_budget(budget, pair, variant)
        prefix = f'runs/pair-{pair:02}/{variant}'
        files.update((prefix + '/' + _PATHS[role], getattr(allocation.run, role).max_bytes)
                     for role in RUN_FILE_FIELDS)
        capture = f'resources/pair-{pair:02}/{variant}'
        for sequence in range(allocation.geometry.sample_count + 1):
            name = f'{"preflight" if sequence == 0 else "sample"}-{sequence:010}'
            files[capture + '/' + name + '.json'] = CAPTURE_MANIFEST_BYTES
            for peer in range(4):
                for role in ('metrics', 'status'):
                    files[f'{capture}/{name}-peer-{peer:04}-{role}.body'] = getattr(allocation.policy, role + '_body_bytes')
    _require(len(files) == budget.resource_member_count + budget.control_file_count)
    return files


def _descriptor_pin(fd):
    """Current physical handle plus access, status and inheritance flags."""
    return (_directory_owner(os.fstat(fd)), fcntl.fcntl(fd, fcntl.F_GETFL),
            fcntl.fcntl(fd, fcntl.F_GETFD))


def _close_original_descriptor(fd, pin):
    """Close only a still-matching original slot; report observed substitution."""
    try:
        if _descriptor_pin(fd) == pin:
            os.close(fd)
            return True
    except OSError:
        pass
    return False


class _Tree:
    """A current archive read scope; never adoption of original trial custody."""
    def __init__(self, root, caps, total):
        self.root = Path(_root_path(root))
        self.caps, self.total = caps, total
        self.chain, self.directories, self.states, self.rows = [], {}, {}, {}
        self.handles, self.closed = [], False
        self.members = {'': set()}
        for name in caps:
            parts = name.split('/')
            for index, part in enumerate(parts):
                parent = '/'.join(parts[:index])
                self.members.setdefault(parent, set()).add(part)
                if index + 1 < len(parts):
                    self.members.setdefault('/'.join(parts[:index + 1]), set())
        try:
            parent = None
            for part in self.root.parts:
                fd = os.open(part, _DIRECTORY, dir_fd=parent)
                self._retain_directory(fd)
                info = os.fstat(fd)
                _require(_directory_owner(info) == _directory_owner(os.stat(part, dir_fd=parent, follow_symlinks=False)))
                self.chain.append((fd, parent, part, _directory_owner(info)))
                parent = fd
            self.directories[''] = self.chain[-1][0]
            self._open('', self.directories[''])
        except BaseException:
            self.close(); raise

    def _retain_directory(self, fd):
        pin = _descriptor_pin(fd)
        self.handles.append((fd, pin))
        _require(pin[1] & os.O_ACCMODE == os.O_RDONLY
                 and pin[1] & os.O_NONBLOCK and pin[2] == fcntl.FD_CLOEXEC)

    def _open(self, relative, fd):
        info = os.fstat(fd)
        _require(stat.S_IMODE(info.st_mode) == 0o700 and info.st_uid == os.geteuid())
        self.states[relative] = _identity(info)
        names = []
        with os.scandir(fd) as entries:
            for entry in entries:
                _require(len(names) < len(self.members[relative]))
                names.append(entry.name)
        _require(set(names) == self.members[relative] and len(names) == len(set(names)))
        for name in sorted(names):
            child = f'{relative}/{name}' if relative else name
            if child in self.members:
                opened = os.open(name, _DIRECTORY, dir_fd=fd)
                self._retain_directory(opened)
                self.directories[child] = opened
                self._open(child, opened)
        self.check()

    def check(self, selected=None):
        _require(not self.closed)
        for fd, pin in self.handles:
            _require(_descriptor_pin(fd) == pin)
        for fd, parent, name, expected in self.chain:
            _require(_directory_owner(os.fstat(fd)) == expected
                == _directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)))
        for relative, fd in self.directories.items():
            if selected is not None and relative and selected != relative and not selected.startswith(relative + '/'):
                continue
            if relative not in self.states:
                continue
            _require(_identity(os.fstat(fd)) == self.states[relative])
            if relative:
                parent, _, name = relative.rpartition('/')
                _require(_identity(os.stat(name, dir_fd=self.directories[parent], follow_symlinks=False)) == self.states[relative])

    def read(self, name, *, retain=False, cap=None):
        _require(not self.closed and name in self.caps)
        maximum = self.caps[name] if cap is None else min(self.caps[name], cap)
        parent, _, leaf = name.rpartition('/')
        directory = self.directories[parent]
        self.check(parent)
        fd = os.open(leaf, _READ, dir_fd=directory)
        pin, completed = None, False
        try:
            pin = _descriptor_pin(fd)
            _require(pin[1] & os.O_ACCMODE == os.O_RDONLY
                     and pin[1] & os.O_NONBLOCK and pin[2] == fcntl.FD_CLOEXEC)
            info = os.fstat(fd); identity = _identity(info)
            _require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_uid == os.geteuid()
                and stat.S_IMODE(info.st_mode) in (0o400, 0o600) and 0 < info.st_size <= maximum)
            _require(identity == _identity(os.stat(leaf, dir_fd=directory, follow_symlinks=False)))
            digest, chunks, count = hashlib.sha256(), [], 0
            while count < info.st_size:
                raw = os.read(fd, min(65536, info.st_size - count))
                _require(raw); count += len(raw); digest.update(raw)
                if retain: chunks.append(raw)
            _require(not os.read(fd, 1) and _identity(os.fstat(fd)) == identity
                == _identity(os.stat(leaf, dir_fd=directory, follow_symlinks=False)))
            row = (info.st_size, digest.hexdigest(), identity)
            if name in self.rows:
                _require(row == self.rows[name])
            result = b''.join(chunks) if retain else row
            self.check(parent)
            # Materialize the result and run directory checks before the final
            # reader fence. A later hash callback must not retarget an earlier FD.
            _require(_descriptor_pin(fd) == pin and _identity(os.fstat(fd)) == identity
                     == _identity(os.stat(leaf, dir_fd=directory, follow_symlinks=False)))
            self.rows[name] = row
            completed = True
            return result
        finally:
            if pin is not None:
                closed = _close_original_descriptor(fd, pin)
                if completed: _require(closed)

    def scan(self):
        total = 0; digest = hashlib.sha256()
        for name in sorted(self.caps):
            size, content, _ = self.read(name)
            total += size; _require(total <= self.total)
            # The versioned domain tag and exact canonical lines are the portable
            # contract. No dev/inode/uid/timestamps or caller bool enters it.
            digest.update(encode([name, size, content], 4096) + b'\n')
        self.check_metadata()
        return ArchiveInventory(len(self.caps), total, _sha(b'iroha.scaling.archive.inventory.v1\n' + digest.digest()))

    def check_metadata(self):
        self.check()
        for name, (_, _, expected) in self.rows.items():
            parent, _, leaf = name.rpartition('/')
            _require(_identity(os.stat(leaf, dir_fd=self.directories[parent], follow_symlinks=False)) == expected)
        self.check()

    def close(self):
        if self.closed: return
        self.closed = True
        for fd, pin in reversed(self.handles):
            _close_original_descriptor(fd, pin)
        self.handles.clear()
        self.directories.clear()
        self.chain.clear()


def inventory_archive(root: Path, plan: ExperimentPlan, budget: EvidenceBudget) -> ArchiveInventory:
    """Compute current content inventory; only the original parent can attribute it."""
    tree = None
    try:
        _, budget, _ = admit_plan(plan, budget)
        tree = _Tree(root, _layout(budget), budget.total_bytes)
        return tree.scan()
    except Exception:
        raise ArchiveDataError('scaling_archive_data_invalid') from None
    finally:
        if tree is not None: tree.close()


def _controls(identity, source, plan_sha256):
    _object(identity, ('schema', 'hardware', 'software'))
    _require(identity['schema'] == _PREFIX + 'fixed_identity.v1')
    hardware = _object(identity['hardware'], ('machine_id', 'cpu_model', 'storage_model', 'physical_cores', 'logical_cores', 'memory_bytes'))
    for name in ('machine_id', 'cpu_model', 'storage_model'): _text(hardware[name])
    for name in ('physical_cores', 'logical_cores', 'memory_bytes'): _integer(hardware[name], 1, (1 << 63) - 1)
    _require(hardware['physical_cores'] <= hardware['logical_cores'] <= 65536)
    software = _object(identity['software'], ('os', 'kernel', 'architecture', 'python_version', 'rustc_version',
        'source_revision', 'workspace_source_sha256', 'plan_sha256', 'irohad_sha256', 'iroha_cli_sha256'))
    for name in ('os', 'kernel', 'architecture', 'python_version', 'rustc_version'): _text(software[name])
    _require(type(software['source_revision']) is str and re.fullmatch('(?:[0-9a-f]{40}|[0-9a-f]{64})', software['source_revision']))
    for name in ('workspace_source_sha256', 'plan_sha256', 'irohad_sha256', 'iroha_cli_sha256'): _digest(software[name])
    _require(software['plan_sha256'] == plan_sha256)
    _object(source, ('schema', 'source_revision', 'workspace_source_sha256', 'executable_images', 'resource_sources', 'scope'))
    _require(source['schema'] == _PREFIX + 'source_inputs.v1'
        and source['scope'] == 'main_executable_images_and_fixed_resource_sources'
        and source['source_revision'] == software['source_revision']
        and source['workspace_source_sha256'] == software['workspace_source_sha256'])
    images = _object(source['executable_images'], ('kagami', 'cli', 'daemon', 'resource_program'))
    for digest in images.values(): _digest(digest)
    _require(images['daemon'] == software['irohad_sha256'] and images['cli'] == software['iroha_cli_sha256'])
    rows = source['resource_sources']
    _require(type(rows) is list and len(rows) == len(SOURCE_NAMES))
    for name, row in zip(SOURCE_NAMES, rows, strict=True):
        _object(row, ('name', 'sha256', 'bytes')); _require(row['name'] == name)
        _digest(row['sha256']); _integer(row['bytes'], 1, MAX_SOURCE_BYTES)
    return images


def _manifest(value, plan, budget, raw_plan, cap):
    _object(value, ('schema', 'original_deadline_ns', 'plan_sha256', 'budget', 'inputs', 'runs'))
    _require(value['schema'] == MANIFEST_SCHEMA and value['plan_sha256'] == _sha(raw_plan))
    selected = parse_run_budget({'experiment': value['budget'], 'pair_index': 1, 'variant': 'one_lane'})
    _require(run_budget_inputs(selected) == run_budget_inputs(select_run_budget(budget, 1, 'one_lane')))
    _require(type(value['inputs']) is list and len(value['inputs']) == 3)
    controls = tuple(ControlBinding(**_object(row, ('label', 'path', 'sha256'))) for row in value['inputs'])
    _require(type(value['runs']) is list and len(value['runs']) == len(RUN_KEYS))
    runs = []
    for row in value['runs']:
        _object(row, ('pair_index', 'variant', 'original_deadline_ns', 'files', 'captures'))
        _require(type(row['files']) is list and len(row['files']) == len(RUN_FILE_FIELDS))
        items = []
        for item in row['files']:
            _object(item, ('role', 'label', 'path', 'sha256', 'bytes', 'max_bytes'))
            items.append(PublicFile(item['role'], ControlBinding(item['label'], item['path'], item['sha256']), item['bytes'], item['max_bytes']))
        census = _object(row['captures'], ('directory', 'files', 'bytes', 'census_sha256', 'metadata_sha256'))
        _require(census['directory'] == f'resources/pair-{row["pair_index"]:02}/{row["variant"]}')
        runs.append(RunManifest(row['pair_index'], row['variant'], row['original_deadline_ns'], tuple(items),
            CaptureCensus(*(census[name] for name in ('files', 'bytes', 'census_sha256', 'metadata_sha256')))))
    raw = manifest_bytes(plan, budget, value['original_deadline_ns'], controls, tuple(runs), cap)
    return raw, controls, tuple(runs)


def _process(value):
    # Pure descriptor parsing only. Never invoke DarwinProcessReader here.
    return ProcessIdentity(**_peer_identity(value))


def _receipt(value, run, trial, images):
    _object(value, ('schema', 'pair_index', 'variant', 'original_deadline_ns', 'raw_run_sha256',
        'generation', 'readiness', 'canonical', 'load_process', 'peers', 'artifacts', 'proof_commands'))
    _require(value['schema'] == _PREFIX + 'run_receipt.v1'
        and type(value['pair_index']) is int and (value['pair_index'], value['variant']) == (run.pair_index, run.variant)
        and type(value['original_deadline_ns']) is int and value['original_deadline_ns'] == run.original_deadline_ns)
    bindings = {item.role: item for item in run.files}
    _require(value['raw_run_sha256'] == bindings['raw_run'].binding.sha256)
    generation = _object(value['generation'], ('network_id', 'genesis_hash', 'context_id', 'genesis_public_key',
        'chain_discriminant', 'anchors_sha256', 'generator_sha256', 'process', 'accounts'))
    for name in ('network_id', 'genesis_hash', 'context_id'): _identity_hash(generation[name])
    for name in ('anchors_sha256', 'generator_sha256'): _digest(generation[name])
    _require(generation['network_id'] == generation['genesis_hash']
        and type(generation['genesis_public_key']) is str and _ED25519.fullmatch(generation['genesis_public_key']))
    _integer(generation['chain_discriminant'], 0, 65535)
    _require(generation['anchors_sha256'] == bindings['genesis_anchors'].binding.sha256
        and generation['generator_sha256'] == images['kagami']
        and _process(generation['process']).executable_sha256 == images['kagami'])
    accounts = generation['accounts']; _require(type(accounts) is list and len(accounts) == trial.generator.account_count)
    for index, row in enumerate(accounts):
        _object(row, ('index', 'account_id')); _require(type(row['index']) is int and row['index'] == index)
        _text(row['account_id'], 2048)
    _require(type(value['peers']) is list and len(value['peers']) == 4)
    peers = tuple(_process(row) for row in value['peers'])
    _require(all(peer.executable_sha256 == images['daemon'] for peer in peers))
    _require(_process(value['load_process']).executable_sha256 == images['cli'])
    ready = value['readiness']; _require(type(ready) is list and len(ready) == 4)
    for index, row in enumerate(ready):
        _object(row, (field.name for field in fields(ReadyReceipt)))
        _require(row['peer_id'] == f'peer{index}' and _process(row['process']) == peers[index])
        for name in ('network_id', 'genesis_hash', 'context_id', 'anchors_sha256'):
            _require(row[name] == generation[name])
        for name in ('challenge', 'cli_sha256', 'client_config_sha256', 'report_sha256'): _digest(row[name])
        _require(type(row['node_id']) is str and _BLS.fullmatch(row['node_id']))
        _require(row['cli_sha256'] == images['cli'] and _process(row['cli_process']).executable_sha256 == images['cli'])
        _require(type(row['attestation']) is str and 0 < len(row['attestation']) <= ((MAX_ATTESTATION_BYTES + 2) // 3) * 4)
        raw = base64.b64decode(row['attestation'], validate=True)
        _require(0 < len(raw) <= MAX_ATTESTATION_BYTES and base64.b64encode(raw).decode('ascii') == row['attestation'])
    canonical = _object(value['canonical'], (field.name for field in fields(CanonicalReplayReceipt)))
    for name in ('invocation_id', 'request_sha256', 'proof_sha256', 'proof_iroha_hash', 'reply_sha256', 'joined_rows_sha256', 'verifier_sha256'): _digest(canonical[name])
    _native_digest(canonical['proof_iroha_hash'], marked=True)
    _integer(canonical['proof_bytes'], 1, bindings['canonical_proof'].max_bytes)
    _integer(canonical['row_count'], 1, 65536)
    _integer(canonical['reply_bytes'], 1, trial.replay_reply_max_bytes)
    _require(canonical['request_sha256'] == bindings['native_request'].binding.sha256
        and canonical['proof_sha256'] == bindings['canonical_proof'].binding.sha256
        and canonical['proof_bytes'] == bindings['canonical_proof'].bytes
        and canonical['verifier_sha256'] == images['kagami']
        and _process(canonical['verifier_process']).executable_sha256 == images['kagami'])
    commands = value['proof_commands']; _require(type(commands) is list and len(commands) == 2)
    for role, row in zip(('proof-prepare', 'proof-export'), commands, strict=True):
        _object(row, ('role', 'process')); _require(row['role'] == role and _process(row['process']).executable_sha256 == images['kagami'])
    artifacts = [dict(role=role, label=bindings[role].binding.label, path=bindings[role].binding.path,
        sha256=bindings[role].binding.sha256, bytes=bindings[role].bytes, max_bytes=bindings[role].max_bytes) for role in _SOURCE_ROLES]
    _require(value['artifacts'] == artifacts and encode(value['artifacts'], MAX_PROJECTION_BYTES) == encode(artifacts, MAX_PROJECTION_BYTES))
    return tuple(row['account_id'] for row in accounts), tuple(ExpectedPeer(f'peer{i}', peer) for i, peer in enumerate(peers))


def _data(value):
    if is_dataclass(value): return {field.name: _data(getattr(value, field.name)) for field in fields(value)}
    if type(value) is tuple: return [_data(item) for item in value]
    return value


def _run(tree, run, trial, budget, images):
    files = {item.role: item for item in run.files}
    for item in run.files:
        size, digest, _ = tree.rows[item.binding.path]
        _require(size == item.bytes and digest == item.binding.sha256)
    receipt_file = files['run_receipt']; raw = tree.read(receipt_file.binding.path, retain=True)
    receipt = _canonical(raw, receipt_file.max_bytes)
    accounts, peers = _receipt(receipt, run, trial, images)
    load = trial.load
    geometry = ReplayGeometry(load.warmup_ns, load.measurement_ns, load.drain_ns,
        load.preparation_ahead_ms * 1000000, load.resource_interval_ms * 1000000,
        load.resource_timeout_ms * 1000000, load.resource_max_start_lag_ms * 1000000)
    allocation = select_run_budget(budget, run.pair_index, run.variant)
    directory = f'resources/pair-{run.pair_index:02}/{run.variant}'
    observed = replay(tree.root / directory, tree.root / files['collector_journal'].binding.path,
        files['collector_journal'].binding.sha256, peers, geometry,
        expected_policy=allocation.policy, allocation=allocation)
    tree.check_metadata()
    _require(observed.capture_file_count == run.census.files and observed.capture_bytes == run.census.bytes)
    requests = []
    for signed, applied in zip(observed.signed_requests, observed.applied_requests, strict=True):
        requests.append(PublicRequest(signed.index, *(getattr(signed.plan, name) for name in
            ('cohort', 'sequence', 'logical_id', 'scheduled_offset_ns', 'account_index')),
            signed.hash, signed.canonical_sha256, len(signed.canonical_bytes),
            *(getattr(applied, field.name) for field in fields(type(applied))[2:])))
    _require(receipt['canonical']['row_count'] == len(requests))
    resources = {name: _data(getattr(observed, name)) for name in ResourceSnapshot._fields}
    projected = dict(schema=_PREFIX + 'raw_run.v1', pair_index=run.pair_index, variant=run.variant,
        load=asdict(load), geometry=asdict(geometry), resources=resources,
        requests=[dict(row._asdict()) for row in requests])
    raw_file = files['raw_run']
    _require(tree.read(raw_file.binding.path, retain=True) == encode(projected, raw_file.max_bytes))
    measured = MeasurementRun(run.pair_index, run.variant, trial, accounts, tuple(peer.identity.pid for peer in peers),
        geometry, observed.preflight, observed.samples, tuple(requests))
    native = NativeArchiveRun(run.pair_index, run.variant, tuple(files[role] for role in _SOURCE_ROLES),
        encode(receipt['generation'], receipt_file.max_bytes), encode(receipt['canonical'], receipt_file.max_bytes),
        encode(receipt['readiness'], receipt_file.max_bytes), measured)
    return measured, native


def inspect_archive(root: Path, plan: ExperimentPlan, budget: EvidenceBudget,
                    expected: ArchiveBindings) -> ArchiveDataChecks:
    """Recompute exact archive data against independently supplied commitments.

    The caller must authenticate expected commitments using the original parent
    receipt and separately perform native crypto replay. This function never
    calls a live owner constructor or treats recorded success flags as proof.
    """
    tree = None
    try:
        _require(type(expected) is ArchiveBindings)
        expected = ArchiveBindings(*(_digest(getattr(expected, field.name)) for field in fields(ArchiveBindings)))
        plan, budget, raw_plan = admit_plan(plan, budget)
        tree = _Tree(root, _layout(budget), budget.total_bytes)
        inventory = tree.scan(); _require(inventory.sha256 == expected.inventory_sha256)
        manifest_raw = tree.read('manifest.json', retain=True, cap=MAX_PROJECTION_BYTES)
        report_raw = tree.read('report.json', retain=True, cap=MAX_PROJECTION_BYTES)
        _require(_sha(manifest_raw) == expected.manifest_sha256 and _sha(report_raw) == expected.report_sha256)
        value = _canonical(manifest_raw, MAX_PROJECTION_BYTES)
        canonical, controls, runs = _manifest(value, plan, budget, raw_plan, budget.control_budgets[0].max_bytes)
        _require(canonical == manifest_raw)
        inputs = {}
        for control, static in zip(controls, budget.static_files, strict=True):
            raw = tree.read(control.path, retain=True, cap=MAX_PROJECTION_BYTES)
            _require(len(raw) == static.size_bytes and _sha(raw) == control.sha256)
            inputs[control.label] = raw
        _require(inputs['plan'] == raw_plan)
        images = _controls(_canonical(inputs['identity'], MAX_PROJECTION_BYTES),
                           _canonical(inputs['source_closure'], MAX_PROJECTION_BYTES), _sha(raw_plan))
        data, native = [], []
        for run, trial in zip(runs, plan.trials, strict=True):
            measured, pending = _run(tree, run, trial, budget, images)
            data.append(measured); native.append(pending)
        measurements = measure_data_experiment(plan, tuple(data))
        _require(report_raw == report_bytes(expected.manifest_sha256, measurements, budget.control_budgets[1].max_bytes))
        _require(tree.scan() == inventory)
        return ArchiveDataChecks(expected, inventory, measurements, tuple(native),
                                 inputs['identity'], inputs['source_closure'])
    except Exception:
        raise ArchiveDataError('scaling_archive_data_invalid') from None
    finally:
        if tree is not None: tree.close()
