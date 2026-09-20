"""Fixed native stopped-tip and facts publication under original input custody.

The caller keeps the original peer3 cleanly stopped, peer0 available for vector
collection, and all generated/runtime/store owners alive through final replay.
Native Rust alone authenticates Norito, signatures, journal semantics and state.
A facts receipt is an intermediate artifact, never a completed execution proof.
"""
from dataclasses import dataclass, fields
import fcntl
import hashlib
import os
from pathlib import Path
import secrets
import stat
import time
from typing import Callable

from resource_bundle import _directory_owner, _identity
from resource_process import ExecutableImage, ProcessIdentity
from scaling_command import BoundedCommand
from scaling_canonical_proof import MAX_TRIAL_NS, _digest, _integer, _object, _path
from scaling_native_outputs import NativeOutputs, PublishedIdentity, RetainedOutput
from scaling_readiness_inputs import ReadinessInputs
from scaling_proof_sequence import StoppedReader
from scaling_native_facts_inputs import (
    MAX_BYTES, FactsBudget, FactsJournalPlan, JournalInput, ReaderBudget,
    budget_snapshot, journal_input_snapshot, journal_snapshot, reader_snapshot,
    stopped_reader,
)

_READ = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
_DIR = _READ | os.O_DIRECTORY
_TIP_FIELDS = frozenset(('version', 'operation', 'invocation_id', 'genesis_sha256',
                         'genesis_bytes', 'committed_height'))
_FACTS_FIELDS = frozenset(('version', 'operation', 'invocation_id', 'facts_sha256', 'facts_bytes'))
_READER_FLAGS = ('first-height', 'last-height', 'max-committed-blocks', 'max-store-data-bytes',
    'max-carrier-bytes', 'max-merge-log-bytes', 'max-merge-frames', 'reader-max-output-bytes',
    'max-decode-allocation-bytes', 'owner-uid')


class NativeFactsError(ValueError):
    """Closed public failure without private journal/config, paths or native stderr."""


def _require(value, code='native_facts_invalid'):
    if not value: raise NativeFactsError(code)


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    if type(error) is NativeFactsError and str(error).startswith('native_facts_'):
        raise NativeFactsError(str(error)) from None
    raise NativeFactsError('native_facts_failed') from None


@dataclass(frozen=True, slots=True)
class StoppedTipReceipt:
    """Bounded original native durable height; later vector/facts checks still apply."""
    reader: StoppedReader
    invocation_id: str
    process: ProcessIdentity
    kagami_sha256: str
    original_anchor_sha256: str
    genesis_sha256: str
    genesis_bytes: int


@dataclass(frozen=True, slots=True)
class FactsReceipt:
    """Native-authenticated intermediate facts with all original input identities."""
    invocation_id: str
    process: ProcessIdentity
    kagami_sha256: str
    original_anchor_sha256: str
    stopped_height: int
    journal_sha256: str
    finality_sha256: str
    queries_sha256: str
    facts: RetainedOutput


class _Journal:
    """Retain one original private journal and each named ancestor through facts."""
    def __init__(self, original, guard):
        self.path, self.digest, self.size, self.maximum = original
        self.chain, self.fd, self.identity, self.flags = [], None, None, None
        try:
            parent = None
            for name in ('/', *self.path.parts[1:-1]):
                fd = os.open(name, _DIR, dir_fd=parent)
                try:
                    info = os.fstat(fd)
                    _require(stat.S_ISDIR(info.st_mode), 'native_facts_journal_parent')
                    identity = _directory_owner(info)
                    if parent is not None:
                        _require(_directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity,
                                 'native_facts_journal_parent')
                    self.chain.append((fd, parent, name, identity))
                    parent = fd
                except BaseException:
                    os.close(fd)
                    raise
            _require(parent is not None and os.fstat(parent).st_uid == os.geteuid()
                     and stat.S_IMODE(os.fstat(parent).st_mode) == 0o700,
                     'native_facts_journal_parent')
            self.fd = os.open(self.path.name, _READ, dir_fd=parent)
            info = os.fstat(self.fd)
            self.identity = _identity(info)
            self.flags = fcntl.fcntl(self.fd, fcntl.F_GETFL)
            _require(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid()
                     and stat.S_IMODE(info.st_mode) == 0o600 and info.st_nlink == 1
                     and info.st_size == self.size and 0 < self.size <= self.maximum,
                     'native_facts_journal_original')
            self.validate()
            digest, offset = hashlib.sha256(), 0
            while offset < self.size:
                guard(); self.validate()
                data = os.pread(self.fd, min(65536, self.size - offset), offset)
                _require(bool(data), 'native_facts_journal_truncated')
                digest.update(data); offset += len(data)
            _require(digest.hexdigest() == self.digest, 'native_facts_journal_digest')
            self.validate(); guard()
        except BaseException:
            self.close()
            raise

    def validate(self):
        _require(self.fd is not None and bool(self.chain), 'native_facts_journal_closed')
        for fd, parent, name, identity in self.chain:
            _require(_directory_owner(os.fstat(fd)) == identity, 'native_facts_journal_parent')
            if parent is not None:
                _require(_directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity,
                         'native_facts_journal_parent')
        parent = self.chain[-1][0]
        before = _identity(os.fstat(parent))
        _require(_identity(os.fstat(self.fd)) == self.identity
                 and fcntl.fcntl(self.fd, fcntl.F_GETFL) == self.flags
                 and self.flags & os.O_ACCMODE == os.O_RDONLY
                 and _identity(os.stat(self.path.name, dir_fd=parent, follow_symlinks=False)) == self.identity,
                 'native_facts_journal_changed')
        _require(_identity(os.fstat(parent)) == before, 'native_facts_journal_parent')
        for fd, parent, name, identity in self.chain:
            _require(_directory_owner(os.fstat(fd)) == identity, 'native_facts_journal_parent')
            if parent is not None:
                _require(_directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity,
                         'native_facts_journal_parent')

    def close(self):
        owned = [] if self.fd is None else [(self.fd, self.identity[:2] if self.identity else None)]
        owned.extend((fd, identity[:2]) for fd, _, _, identity in reversed(self.chain))
        for fd, identity in owned:
            try:
                if identity is None or _identity(os.fstat(fd))[:2] == identity: os.close(fd)
            except OSError: pass
        self.fd, self.chain = None, []


def _original_snapshot(inputs):
    inputs.validate()
    generation = inputs.generation
    directory = Path(_path(inputs.input_directory))
    accounts = tuple(row.account_id for row in generation.accounts)
    artifacts = tuple((row.path, _digest(row.sha256), _integer(row.bytes, 1, 64 * 1024 * 1024))
                      for row in generation.artifacts)
    roles = tuple((row.peer_id, Path(_path(row.node_config)), row.node_public_key,
                   Path(_path(row.primary_block_store)), Path(_path(row.primary_merge_log)))
                  for row in inputs.roles)
    _require(len(roles) == 4 and tuple(row[0] for row in roles) == ('peer0', 'peer1', 'peer2', 'peer3'))
    _require(len({row[2] for row in roles}) == 4 and len({row[0] for row in artifacts}) == len(artifacts))
    _integer(generation.chain_discriminant, 0, 65535)
    _require(type(generation.genesis_public_key) is str and 0 < len(generation.genesis_public_key) <= 2048)
    return (directory, inputs.anchors_sha256, inputs.network_id, generation.chain_id,
            generation.lane_count, generation.genesis_public_key, generation.chain_discriminant,
            accounts, artifacts, roles)


class NativeFacts:
    """Single-use stopped-tip then facts stages sharing one original deadline.

    Call observe_tip after original peer3 is cleanly stopped. Its reader goes to
    NativeVectorCollection. After vector collection completes into these same
    NativeOutputs, call produce_facts. Keep this owner and its original inputs
    alive until proof replay and final experiment validation finish. Cleanup only
    reaps owned commands; close only releases the journal descriptors.
    """
    def __init__(self, inputs: ReadinessInputs, outputs: NativeOutputs,
                 journal: JournalInput, reader_budget: ReaderBudget, plan: FactsJournalPlan,
                 budget: FactsBudget, image: ExecutableImage, reader, trial_deadline_ns: int,
                 verify_original_runtime: Callable[[], None]):
        _require(not hasattr(self, '_phase'), 'native_facts_readmission')
        self._phase, self._journal, self._commands = 'admitting', None, None
        try:
            _require(type(inputs) is ReadinessInputs and type(outputs) is NativeOutputs
                     and isinstance(image, ExecutableImage) and callable(getattr(reader, 'sample', None))
                     and callable(verify_original_runtime), 'native_facts_owner_invalid')
            _integer(trial_deadline_ns, 1)
            _require(0 < trial_deadline_ns - time.monotonic_ns() <= MAX_TRIAL_NS,
                     'native_facts_deadline_invalid')
            self._inputs, self._outputs, self._image, self._process_reader = inputs, outputs, image, reader
            self._end, self._guard = trial_deadline_ns, verify_original_runtime
            self._admitted_end = trial_deadline_ns
            self._binding = (image.path, image.fd, image.identity, image.sha256, image.uuids)
            self._original = _original_snapshot(inputs)
            self._limits = reader_snapshot(reader_budget)
            _require(self._limits[-1] == os.geteuid(), 'native_facts_owner_uid')
            self._plan, self._counts = journal_snapshot(plan, self._original[7])
            self._budget = budget_snapshot(budget)
            self._journal_input = journal_input_snapshot(journal)
            self._height, self._tip, self._invocations = None, None, set()
            self._admit_originals()
            self._verify()
            self._journal = _Journal(self._journal_input, self._verify)
            self._commands = BoundedCommand(image, reader, trial_deadline_ns, self._verify)
            self._verify()
            self._phase = 'admitted'
        except BaseException as error:
            self._phase = 'failed'
            if self._journal is not None: self._journal.close()
            _failure(error)

    def _admit_originals(self):
        original, budget, outputs = self._original, self._budget, self._outputs
        _require(outputs.directory != original[0]
                 and original[0] not in outputs.directory.parents
                 and outputs.directory not in original[0].parents,
                 'native_facts_output_namespace')
        _require(self._image.path != outputs.directory
                 and outputs.directory not in self._image.path.parents,
                 'native_facts_output_namespace')
        names = ('genesis.json', 'genesis.signed.nrt', 'peer0.toml', 'peer1.toml',
                 'peer2.toml', 'peer3.toml', 'genesis-context.nrt')
        records = {name: (digest, size) for name, digest, size in original[8]}
        caps = (budget[0], budget[1], *(budget[2] for _ in range(4)), budget[3])
        _require(all(name in records and records[name][1] <= cap for name, cap in zip(names, caps, strict=True)),
                 'native_facts_original_reservation')
        self._files = tuple((original[0] / name, *records[name], cap)
                            for name, cap in zip(names, caps, strict=True))
        _require(all(original[9][index][1] == self._files[index + 2][0] for index in range(4)),
                 'native_facts_peer_config_binding')
        self._vector_caps = outputs.allocation('finality'), outputs.allocation('queries')
        _require(sum(caps) + self._journal_input[3] + sum(self._vector_caps) <= budget[4]
                 and outputs.allocation('facts') == budget[5]
                 and self._plan[15] <= budget[12], 'native_facts_total_reservation')
        _require(self._journal_input[0] not in {row[0] for row in self._files}, 'native_facts_journal_alias')
        self._store, self._merge = original[9][3][3:5]
        _require(self._store != self._merge, 'native_facts_store_alias')

    def _verify(self):
        phase = self._phase
        _require(phase in ('admitting', 'admitted', 'busy', 'tip-ready', 'complete')
                 and self._end == self._admitted_end and time.monotonic_ns() < self._end, 'native_facts_phase_or_deadline')
        image = self._image
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._binding,
                 'native_facts_image_changed')
        image.validate(); self._outputs.validate()
        _require(_original_snapshot(self._inputs) == self._original, 'native_facts_original_changed')
        if self._journal is not None: self._journal.validate()
        self._guard()
        _require(self._phase == phase and time.monotonic_ns() < self._end,
                 'native_facts_phase_or_deadline')
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._binding,
                 'native_facts_image_changed')
        image.validate(); self._outputs.validate()
        _require(_original_snapshot(self._inputs) == self._original, 'native_facts_original_changed')
        if self._journal is not None: self._journal.validate()
        _require(self._phase == phase and self._end == self._admitted_end
                 and time.monotonic_ns() < self._end, 'native_facts_phase_or_deadline')

    def _reader_argv(self, height):
        return tuple(part for flag, value in zip(_READER_FLAGS, (1, height, *self._limits), strict=True)
                     for part in ('--' + flag, str(value)))

    def _call(self, operation, arguments, reply_operation, expected):
        invocation = _digest(secrets.token_hex(32))
        _require(invocation != '0' * 64 and invocation not in self._invocations,
                 'native_facts_invocation_invalid')
        self._invocations.add(invocation)
        argv = (str(self._image.path), '--ui-mode', 'plain', 'advanced', 'kura', 'scaling-evidence',
                operation, '--invocation-id', invocation, *arguments, '--reply-max-bytes', '512')
        result = self._commands.run('native-' + operation, argv, (self._journal.fd,), 512)
        self._verify()
        raw = result.stdout
        _require(type(raw) is bytes and 1 < len(raw) <= 512 and raw.endswith(b'\n'),
                 'native_facts_reply_framing')
        reply = _object(raw[:-1], expected)
        _require(type(reply['version']) is int and reply['version'] == 1
                 and reply['operation'] == reply_operation and reply['invocation_id'] == invocation,
                 'native_facts_reply_operation')
        return invocation, result.process, reply

    def observe_tip(self) -> StoppedTipReceipt:
        """Observe only the original peer3 durable marker under genesis pins."""
        try:
            _require(self._phase == 'admitted', 'native_facts_tip_phase')
            self._phase = 'busy'; self._verify()
            path, digest, size, cap = self._files[1]
            invocation, process, reply = self._call('stopped-tip', (
                '--signed-genesis', str(path), '--signed-genesis-sha256', digest,
                '--signed-genesis-max-bytes', str(cap), '--network-id', self._original[2],
                '--block-store', str(self._store), '--merge-log', str(self._merge),
                *self._reader_argv(1)), 'stopped_tip', _TIP_FIELDS)
            _require(reply['genesis_sha256'] == digest and type(reply['genesis_bytes']) is int
                     and reply['genesis_bytes'] == size, 'native_facts_tip_genesis_changed')
            height = _integer(reply['committed_height'], 1, min(self._limits[0], self._budget[11]))
            self._verify()
            self._height, self._tip = height, (invocation, process, digest, size)
            self._phase = 'tip-ready'
            return StoppedTipReceipt(stopped_reader(self._store, self._merge, height, self._limits),
                invocation, process, self._binding[3], self._original[1], digest, size)
        except BaseException as error:
            self._phase = 'failed'; _failure(error)

    def produce_facts(self) -> FactsReceipt:
        """Authenticate all originals and vectors, then capture the complete facts file."""
        try:
            _require(self._phase == 'tip-ready' and self._height is not None, 'native_facts_publication_phase')
            self._phase = 'busy'; self._verify()
            vectors = tuple(self._outputs.artifact(role) for role in ('finality', 'queries'))
            _require(tuple(row.max_bytes for row in vectors) == self._vector_caps,
                     'native_facts_vector_reservation_changed')
            arguments = []
            def add(flag, *values): arguments.extend(('--' + flag, *(str(value) for value in values)))
            for flag, index in (('manifest', 0), ('signed-genesis', 1), ('context', 6)):
                path, digest, _, cap = self._files[index]
                add(flag, path); add(flag + '-sha256', digest); add(flag + '-max-bytes', cap)
            add('peer-config', *(row[0] for row in self._files[2:6]))
            add('peer-config-sha256', *(row[1] for row in self._files[2:6]))
            add('peer-config-max-bytes', self._budget[2])
            path, digest, _, cap = self._journal_input
            add('journal', path); add('journal-sha256', digest); add('journal-max-bytes', cap)
            for row in vectors:
                add(row.role, row.path); add(row.role + '-sha256', row.sha256); add(row.role + '-max-bytes', row.max_bytes)
            add('chain-id', self._original[3]); add('network-id', self._original[2])
            add('genesis-public-key', self._original[5]); add('chain-discriminant', self._original[6])
            add('validator', *(row[2] for row in self._original[9]))
            add('lanes', self._original[4]); add('account', *self._original[7])
            for field, value in zip(fields(FactsJournalPlan), self._plan, strict=True):
                add(field.name.replace('_', '-'), value)
            for field, value in zip(fields(FactsBudget)[4:], self._budget[4:], strict=True):
                add(field.name.replace('_', '-'), value)
            arguments.extend(self._reader_argv(self._height))
            add('block-store', self._store); add('merge-log', self._merge)
            add('facts-output', self._outputs.path('facts'))
            self._outputs.begin('facts')
            invocation, process, reply = self._call('facts', tuple(arguments), 'facts', _FACTS_FIELDS)
            artifact, = self._outputs.complete((PublishedIdentity('facts', _digest(reply['facts_sha256']),
                _integer(reply['facts_bytes'], 1, self._budget[5])),))
            self._verify()
            self._phase = 'complete'
            return FactsReceipt(invocation, process, self._binding[3], self._original[1], self._height,
                                digest, vectors[0].sha256, vectors[1].sha256, artifact)
        except BaseException as error:
            self._phase = 'failed'; _failure(error)

    def validate(self):
        """Keep original journal/generation/store guards active through final replay."""
        try: self._verify()
        except BaseException as error:
            self._phase = 'failed'; _failure(error)

    def cleanup(self, deadline_ns):
        """Reap retained native children without releasing original evidence custody."""
        self._phase = 'failed'
        return () if self._commands is None else self._commands.cleanup(deadline_ns)

    def close(self):
        """Release only owned original journal descriptors after children are reaped."""
        try:
            if self._commands is not None:
                for child in self._commands._children:
                    self._commands._bound(child)
                    _require(child.process.poll() is not None, 'native_facts_child_not_reaped')
                    self._commands._bound(child)
            self._phase = 'failed'
            if self._journal is not None: self._journal.close()
        except BaseException as error:
            self._phase = 'failed'; _failure(error)
