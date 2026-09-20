"""Completed fixed-trial authority under one original experiment token.

Requires Python 3.11+ and the actual completed FixedTrial plus retained public
file/capture owners. Admission and commit finish before that trial's unchanged
deadline. This owner reads no pathname, executes no native command, decodes no
Norito, and cannot accept receipt JSON or issue a release verdict. Its public
projection contains bounded immutable values, never private TOMLs, development
seeds or retained signed bodies. The outer owner retains the physical scope.
"""
from dataclasses import fields
import hashlib
import os
from pathlib import Path
import time
from typing import NamedTuple

from applied_request_journal import RetainedApplication
from resource_bundle import ControlBinding
from resource_evidence_budget import (
    PerRunResourceBudget, canonical_run_budget_bytes, parse_run_budget, run_budget_inputs,
)
from resource_process import ProcessIdentity
from resource_replay import ReplayResult, ReplayGeometry, Bracket, CaptureReduction
from scaling_canonical_proof import (
    CanonicalReplayReceipt, ReplayPlan, _plan_snapshot, _digest, _integer,
)
from scaling_command import BoundedCommand, MAX_COMMANDS
from scaling_fixed_trial import FixedTrial, TrialResult, TrialPlan
from scaling_generator import GenerationReceipt, GeneratorPlan, GeneratedPeer
from scaling_load_outputs import LoadArtifact
from scaling_native_facts import StoppedTipReceipt, FactsReceipt
from scaling_native_facts_inputs import ReaderBudget, FactsBudget, journal_snapshot
from scaling_native_load import NativeLoadPlan, NativeLoadReceipt
from scaling_native_outputs import NativeOutputBudget, RetainedOutput
from scaling_proof_sequence import StoppedReader
from scaling_public_files import RunPublicFiles, PublicFile, _PATHS, _SOURCE_ROLES
from scaling_readiness import ReadyReceipt, MAX_ATTESTATION_BYTES
from scaling_readiness_inputs import ReadinessRole, GeneratedAccount, GeneratedArtifact
from scaling_replayed_workload import build_replay_plan, _resource_scope
from scaling_trial_captures import CaptureCensus, TransferredCaptures
from scaling_vector_collection import CollectionLimits, VectorCollectionReceipt
from signed_request_journal import RetainedRequest, RequestPlan, MAX_REQUEST_BYTES


class CompletedAuthorityError(ValueError):
    """Closed authority failure with no runtime inputs or child output."""


def _require(value):
    if not value:
        raise CompletedAuthorityError('completed_authority_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise CompletedAuthorityError('completed_authority_failed') from None


_RECORDS = (TrialPlan, GeneratorPlan, NativeLoadPlan, ReaderBudget, FactsBudget,
    CollectionLimits, NativeOutputBudget, GenerationReceipt, GeneratedPeer,
    ReadinessRole, GeneratedAccount, GeneratedArtifact, ReadyReceipt,
    NativeLoadReceipt, LoadArtifact, StoppedTipReceipt, StoppedReader,
    VectorCollectionReceipt, FactsReceipt, CanonicalReplayReceipt, ProcessIdentity,
    RetainedOutput, ReplayGeometry, CaptureReduction, Bracket, CaptureCensus)
_PUBLIC_RECORDS = {record: NamedTuple(record.__name__ + 'Snapshot',
    [(field.name, object) for field in fields(record)]) for record in _RECORDS}
_RESOURCE_FIELDS = tuple(field.name for field in fields(ReplayResult)
    if field.name not in ('signed_requests', 'applied_requests', 'geometry', 'allocation'))
ResourceSnapshot = NamedTuple('ResourceSnapshot', [(name, object) for name in _RESOURCE_FIELDS])
_NATIVE_ROLES = (('native_finality', 'finality'), ('native_queries', 'queries'),
    ('native_facts', 'facts'), ('native_request', 'request'),
    ('native_bundle', 'bundle'), ('canonical_proof', 'proof'))
_GENESIS = (('genesis_manifest', 'genesis.json'), ('signed_genesis', 'genesis.signed.nrt'),
    ('genesis_context', 'genesis-context.nrt'), ('genesis_network_record', 'genesis.expected_hash'),
    ('genesis_anchors', 'genesis-anchors.json'))


class _Freeze:
    """Bounded explicit public-record projection; signed requests are excluded."""
    def __init__(self, requests, samples, receipt_cap):
        self.nodes = 4096 + requests * 48 + samples * 32
        self.members = max(128, requests, samples)
        self.bytes = min(4 * MAX_ATTESTATION_BYTES, receipt_cap)

    def __call__(self, value, depth=0):
        self.nodes -= 1
        _require(self.nodes >= 0 and depth <= 16)
        if type(value) is int:
            _require(-(1 << 63) <= value < 1 << 128)
            return value
        if type(value) is str:
            _require(len(value) <= 4096 and len(value.encode()) <= 16384)
            return value
        if type(value) is bytes:
            _require(0 < len(value) <= MAX_ATTESTATION_BYTES and len(value) <= self.bytes)
            self.bytes -= len(value)
            return value
        if type(value) is type(Path('/')):
            _require(value.is_absolute() and str(value) == os.path.abspath(value)
                     and len(os.fsencode(value)) <= 4096)
            return str(value)
        if type(value) is tuple:
            _require(len(value) <= self.members)
            return tuple(self(item, depth + 1) for item in value)
        _require(type(value) in _RECORDS)
        return _PUBLIC_RECORDS[type(value)](*(self(getattr(value, field.name), depth + 1)
                                             for field in fields(type(value))))


class PublicRequest(NamedTuple):
    """One original signed-byte commitment and complete timing/Applied identity."""
    index: int
    cohort: str
    sequence: int
    logical_id: str
    scheduled_offset_ns: int
    account_index: int
    transaction_hash: str
    canonical_sha256: str
    canonical_size_bytes: int
    offer_offset_ns: int
    acknowledgment_offset_ns: int
    applied_offset_ns: int
    block_height: int
    status_attempts: int
    local_applied_offset_ns: int
    local_block_height: int
    local_status_attempts: int


class AppliedSnapshot(NamedTuple):
    """Original native-plan observation, with both required matching heights."""
    transaction_hash: str
    global_height: int
    local_height: int


class ReplayPlanSnapshot(NamedTuple):
    """Complete immutable native plan, retaining original order and counts."""
    seed: str
    lane_count: int
    accounts: tuple
    warmup_requests: int
    measurement_requests: int
    last_height: int
    observations: tuple[AppliedSnapshot, ...]


class PublicArtifact(NamedTuple):
    """Exact fixed public destination bound to its original producer identity."""
    role: str
    label: str
    path: str
    sha256: str
    bytes: int
    max_bytes: int


class PublicCommand(NamedTuple):
    """Original completed native stage identity, without argv or child output."""
    role: str
    process: tuple


class PublicRunProjection(NamedTuple):
    """Owned public values; only their originating authority supplies provenance."""
    pair_index: int
    variant: str
    original_deadline_ns: int
    budget: bytes
    plan: tuple
    replay_plan: tuple
    generation: tuple
    readiness: tuple
    load: tuple
    stopped_tip: tuple
    vectors: tuple
    facts: tuple
    canonical: tuple
    proof_commands: tuple[PublicCommand, ...]
    peers: tuple
    geometry: tuple
    resources: ResourceSnapshot
    requests: tuple[PublicRequest, ...]
    artifacts: tuple[PublicArtifact, ...]


def _preflight(trial):
    _require(type(trial) is FixedTrial and trial._phase == 'complete'
             and type(trial._result) is TrialResult
             and type(trial._plan) is TrialPlan and type(trial._allocation) is PerRunResourceBudget)
    end = _integer(trial.original_deadline_ns, 1, (1 << 63) - 1)
    _require(trial._deadline == end == trial._result.original_deadline_ns
             and time.monotonic_ns() < end)
    count = _integer(trial._scheduled, 1, 64 * 1024)
    result = trial._result
    _require(type(result.resources) is ReplayResult and result.resources is trial._resources
             and type(result.resources.signed_requests) is tuple
             and type(result.resources.applied_requests) is tuple
             and len(result.resources.signed_requests) == len(result.resources.applied_requests) == count
             and type(trial._proof_plan) is ReplayPlan
             and type(trial._proof_plan.observations) is tuple
             and len(trial._proof_plan.observations) == count
             and type(result.readiness) is tuple and len(result.readiness) == 4
             and result.readiness is trial._ready_receipts
             and type(result.generation) is GenerationReceipt)
    for value, cap in ((result.generation.accounts, 64), (result.generation.artifacts, 78),
                       (result.generation.peers, 4)):
        _require(type(value) is tuple and 0 < len(value) <= cap)
    _require(len(result.generation.peers) == 4)
    for ready in result.readiness:
        _require(type(ready) is ReadyReceipt and type(ready.attestation) is bytes
                 and 0 < len(ready.attestation) <= MAX_ATTESTATION_BYTES)
    _require(type(result.resources.geometry) is ReplayGeometry)
    result.resources.geometry.validate()
    _require(type(result.resources.samples) is tuple
             and len(result.resources.samples) == result.resources.geometry.samples <= 100_000)
    return result, end, count


def _terminal(commands, role, process, deadline):
    _require(type(commands) is BoundedCommand and type(process) is ProcessIdentity
             and type(commands._children) is list and 0 < len(commands._children) <= MAX_COMMANDS
             and commands._phase == 'idle' and type(commands._end) is int
             and type(commands._admitted_end) is int
             and commands._end == commands._admitted_end == deadline)
    matches = []
    for child in commands._children:
        commands._bound(child)
        _require(child.pinned is not None and type(child.identity) is ProcessIdentity
                 and type(child.process.returncode) is int and child.process.returncode == 0)
        if child.role == role and child.identity == process:
            matches.append(child)
    _require(len(matches) == 1 and process.executable_sha256 == commands._binding[3])


def _commitments(replay, selected, proof, budget):
    """Check all signed/Applied rows while retaining only their public commitments."""
    _require(type(replay) is ReplayResult and canonical_run_budget_bytes(replay.allocation) == budget)
    journal_bytes = _resource_scope(replay, selected, proof[1])
    signed, applications = replay.signed_requests, replay.applied_requests
    count = proof[3] + proof[4]
    _require(type(signed) is tuple and type(applications) is tuple
             and len(signed) == len(applications) == count)
    result, seen, body_bytes = [], set(), 0
    for index, (request, application, expected) in enumerate(zip(signed, applications, proof[6], strict=True)):
        _require(type(request) is RetainedRequest and type(request.plan) is RequestPlan
                 and type(application) is RetainedApplication
                 and type(request.index) is int and type(application.index) is int
                 and request.index == application.index == index)
        tx_hash = _digest(request.hash, marked=True)
        _require(type(application.hash) is str and application.hash == tx_hash == expected[0]
                 and tx_hash not in seen)
        seen.add(tx_hash)
        digest = _digest(request.canonical_sha256)
        raw = request.canonical_bytes
        _require(type(raw) is bytes and 0 < len(raw) <= MAX_REQUEST_BYTES)
        body_bytes += len(raw)
        _require(body_bytes <= journal_bytes // 2 and hashlib.sha256(raw).hexdigest() == digest)
        plan = tuple(getattr(request.plan, field.name) for field in fields(RequestPlan))
        _require(type(plan[0]) is str and type(plan[2]) is str
                 and all(type(plan[i]) is int for i in (1, 3, 4)))
        _digest(plan[2])
        offsets = tuple(getattr(application, field.name) for field in fields(RetainedApplication)[2:])
        _require(all(type(value) is int and -(1 << 63) <= value < 1 << 63 for value in offsets)
                 and application.block_height == application.local_block_height == expected[1]
                 and 1 <= expected[1] <= proof[5])
        result.append(PublicRequest(index, *plan, tx_hash, digest, len(raw), *offsets))
    return tuple(result)


def _source(trial):
    result, end, count = _preflight(trial)
    allocation = parse_run_budget(run_budget_inputs(trial._allocation))
    budget = canonical_run_budget_bytes(allocation)
    _require(budget == trial._budget and (result.pair_index, result.variant)
             == (allocation.run.pair_index, allocation.run.variant)
             == (trial._paths.pair_index, trial._paths.variant))
    freeze = _Freeze(count, len(result.resources.samples), allocation.run.run_receipt.max_bytes)
    # Bound public records before the original owner's recursive fingerprint.
    plan = freeze(trial._plan)
    generation = freeze(result.generation)
    readiness = freeze(result.readiness)
    receipts = tuple(freeze(getattr(result, name))
                     for name in ('load', 'stopped_tip', 'vectors', 'facts', 'canonical'))
    trial.validate()
    _require(trial._result is result)
    native = trial._generated.inputs
    gen, loaded, tip, vectors, facts, canonical = (result.generation, result.load,
        result.stopped_tip, result.vectors, result.facts, result.canonical)
    _require(gen is trial._generated.receipt and loaded is trial._load._receipt
             and tuple(value for value, _ in trial._terminal_pins)
             == (gen, loaded, tip, vectors, facts, canonical)
             and len({ready.challenge for ready in result.readiness}) == 4
             and tuple(result.readiness) == tuple(trial._readiness._receipts))
    accounts = tuple(account.account_id for account in gen.accounts)
    selected, counts = journal_snapshot(trial._journal_plan(), accounts)
    proof = _plan_snapshot(trial._proof_plan)
    _require(proof == _plan_snapshot(build_replay_plan(result.resources, native,
             trial._journal_plan(), tip.reader))
             and proof[:6] == (loaded.plan.seed, gen.plan.lane_count, accounts, *counts, tip.reader.last_height)
             and sum(counts) == selected[15] == loaded.scheduled_requests == count
             and gen.plan == trial._plan.generator and loaded.plan == trial._plan.load
             and loaded.budget_sha256 == hashlib.sha256(budget).hexdigest())
    cli, kagami, daemon = (trial._runtime.cli.sha256, trial._runtime.kagami.sha256,
                           trial._runtime.daemon.sha256)
    _require(gen.generator_sha256 == tip.kagami_sha256 == facts.kagami_sha256
             == canonical.verifier_sha256 == kagami
             and loaded.cli_sha256 == vectors.cli_sha256 == cli)
    _require(gen.anchors_sha256 == native.anchors_sha256 == tip.original_anchor_sha256
             == facts.original_anchor_sha256 == vectors.anchors_sha256
             and (gen.genesis_hash, gen.context_id, gen.network_id, gen.genesis_public_key, gen.chain_discriminant)
             == (native.genesis_hash, native.context_id, native.network_id, native.genesis_public_key, native.chain_discriminant)
             and gen.accounts == native.generation.accounts and gen.artifacts == native.generation.artifacts
             and tuple(peer.role for peer in gen.peers) == native.roles
             and tip.reader.block_store == native.roles[3].primary_block_store
             and tip.reader.merge_log == native.roles[3].primary_merge_log
             and tip.reader.first_height == 1
             and tip.reader.last_height == vectors.stopped_height == facts.stopped_height == proof[5])
    _require(type(loaded.peers) is tuple and len(loaded.peers) == 4
             and loaded.peers == tuple(pin.identity for pin in trial._launch._pinned))
    terminals = []
    def terminal(commands, role, process):
        _terminal(commands, role, process, end)
        terminals.append((commands, role, process, freeze(process)))
    for index, ready in enumerate(result.readiness):
        role = native.roles[index]
        _require(ready.peer_id == role.peer_id == f'peer{index}' and ready.node_id == role.node_public_key
                 and ready.process == loaded.peers[index]
                 and ready.process.executable_sha256 == daemon
                 and (ready.genesis_hash, ready.context_id, ready.network_id)
                 == (gen.genesis_hash, gen.context_id, gen.network_id)
                 and ready.anchors_sha256 == gen.anchors_sha256
                 and ready.client_config_sha256 == role.client_config_sha256
                 and ready.cli_sha256 == cli)
        _digest(ready.challenge); _digest(ready.report_sha256)
        terminal(trial._readiness._commands, ready.peer_id, ready.cli_process)
    terminal(trial._generator._commands, 'generator', gen.process)
    terminal(trial._load._commands, 'native-load', loaded.process)
    terminal(trial._facts._commands, 'native-stopped-tip', tip.process)
    terminal(trial._facts._commands, 'native-facts', facts.process)
    terminal(trial._vectors._commands, 'vector-collection', vectors.process)
    terminal(trial._proof._replay._commands, 'canonical-replay', canonical.verifier_process)
    commands = trial._proof._commands
    _require(type(commands) is BoundedCommand and type(commands._children) is list
             and len(commands._children) == 2
             and tuple(child.role for child in commands._children) == ('proof-prepare', 'proof-export'))
    proof_children = tuple(commands._children)
    proof_commands = []
    for child in commands._children:
        terminal(commands, child.role, child.identity)
        proof_commands.append(PublicCommand(child.role, freeze(child.identity)))
    _require(trial._proof._phase == trial._proof._replay._phase == 'verified'
             and trial._proof._replay._plan == proof)
    output = {row.role: row for row in trial._outputs.finish()}
    source = []
    prefix = f'runs/pair-{result.pair_index:02}/{result.variant}'
    def artifact(role, digest, size):
        cap = getattr(allocation.run, role)
        _digest(digest); _integer(size, 1, cap.max_bytes)
        source.append(PublicArtifact(role, cap.label, prefix + '/' + _PATHS[role], digest, size, cap.max_bytes))
    for role in ('collector_journal', 'transaction_trace'):
        item = getattr(loaded, role)
        _require(item.path == getattr(trial._paths.load, role)
                 and item.label == getattr(allocation.run, role).label)
        artifact(role, item.sha256, item.bytes)
    for role, native_role in _NATIVE_ROLES:
        item = output[native_role]
        _require(item.path == trial._paths.native / f'{native_role}.nrt'
                 and item.max_bytes == getattr(allocation.run, role).max_bytes)
        artifact(role, item.sha256, item.bytes)
    originals = {item.path: item for item in gen.artifacts}
    _require(len(originals) == len(gen.artifacts))
    for role, name in _GENESIS:
        item = originals.get(name)
        digest, size = (gen.anchors_sha256, gen.anchors_bytes) if name == 'genesis-anchors.json' else (item.sha256, item.bytes)
        artifact(role, digest, size)
    _require(tuple(row[0] for row in source) == _SOURCE_ROLES
             and (tip.genesis_sha256, tip.genesis_bytes)
             == (originals['genesis.signed.nrt'].sha256, originals['genesis.signed.nrt'].bytes)
             and (vectors.context_sha256, vectors.context_bytes)
             == (originals['genesis-context.nrt'].sha256, originals['genesis-context.nrt'].bytes)
             and vectors.client_config_sha256 == native.roles[0].client_config_sha256
             and vectors.finality == output['finality'] and vectors.queries == output['queries']
             and facts.facts == output['facts']
             and facts.journal_sha256 == loaded.collector_journal.sha256 == result.resources.journal_sha256
             and facts.finality_sha256 == output['finality'].sha256
             and facts.queries_sha256 == output['queries'].sha256
             and canonical.request_sha256 == output['request'].sha256
             and (canonical.proof_sha256, canonical.proof_bytes) == (output['proof'].sha256, output['proof'].bytes)
             and canonical.row_count == count)
    bindings = trial._proof._replay._bindings
    _require((str(output['request'].path), canonical.request_sha256, output['request'].max_bytes,
              str(output['proof'].path), canonical.proof_sha256, canonical.proof_iroha_hash,
              canonical.proof_bytes, output['proof'].max_bytes, trial._plan.replay_reply_max_bytes) == bindings)
    commitments = _commitments(result.resources, selected, proof, budget)
    resource = ResourceSnapshot(*(freeze(getattr(result.resources, name)) for name in _RESOURCE_FIELDS))
    geometry = freeze(result.resources.geometry)
    census = freeze(result.capture_census)
    _require(result.capture_census == trial._captures.census
             and result.capture_census.files == result.resources.capture_file_count
             and result.capture_census.bytes == result.resources.capture_bytes)
    public_plan = ReplayPlanSnapshot(*proof[:6], tuple(AppliedSnapshot(tx, height, height)
                                                     for tx, height in proof[6]))
    payload = PublicRunProjection(result.pair_index, result.variant, end, budget, plan, public_plan,
        generation, readiness, *receipts, tuple(proof_commands), freeze(loaded.peers), geometry, resource, commitments, tuple(source))
    trial.validate()
    # The original validation callback has completed. Recheck every original
    # terminal handle with no callback, polling or long read before returning.
    _require(trial._proof._commands is commands and len(commands._children) == 2
             and all(actual is original for actual, original in zip(commands._children, proof_children, strict=True)))
    for command, role, process, pin in terminals:
        _terminal(command, role, process, end)
        _require(freeze(process) == pin)
    _require(time.monotonic_ns() < end and trial._result is result)
    return payload, commitments, selected, census, str(trial._paths.public), str(trial._paths.captures)


class CompletedRunAuthority:
    """Single-use timed admission, then token-bound retained public reconciliation."""
    def __init__(self, *args, **kwargs):
        raise CompletedAuthorityError('completed_authority_failed')

    @classmethod
    def admit(cls, trial: FixedTrial, experiment_token: object):
        """Admit only the actual completed trial; no public operation is yet authorized."""
        owner = None
        try:
            _require(cls is CompletedRunAuthority and type(experiment_token) is object)
            source = _source(trial)
            _require(_source(trial) == source)
            owner = object.__new__(cls)
            owner._token = experiment_token
            owner._trial, owner._source = trial, source
            owner._original_source = source
            owner._phase, owner._busy = 'pending', False
            owner._files = owner._captures = None
            return owner
        except BaseException as error:
            if owner is not None: owner._phase = 'failed'
            _failure(error)

    def _check(self, token, phases):
        _require(type(self) is CompletedRunAuthority and type(token) is object and token is self._token
                 and self._phase in phases and self._source is self._original_source)

    def check_provenance(self, token):
        """Cheap public callback guard; performs no recursive physical verification."""
        try:
            self._check(token, ('committed', 'reconciled'))
        except BaseException as error:
            self._phase = 'failed'; _failure(error)

    def _physical(self, files, captures, committing=False):
        payload, _, _, census, directory, capture_root = self._source
        _require(type(files) is RunPublicFiles and type(captures) is TransferredCaptures
                 and str(files.directory) == directory and str(captures.directory) == capture_root
                 and canonical_run_budget_bytes(files.allocation) == payload.budget
                 == canonical_run_budget_bytes(captures.allocation))
        public = files.verify()
        roles = tuple(item.role for item in public)
        _require(roles == _SOURCE_ROLES if committing else roles in
                 (_SOURCE_ROLES, (*_SOURCE_ROLES, 'raw_run'), (*_SOURCE_ROLES, 'raw_run', 'run_receipt')))
        for item, expected in zip(public[:13], payload.artifacts, strict=True):
            _require(type(item) is PublicFile and type(item.binding) is ControlBinding
                     and (item.role, item.binding.label, item.binding.path,
                          item.binding.sha256, item.bytes, item.max_bytes) == expected)
        observed = captures.verify()
        _require(type(observed) is CaptureCensus
                 and tuple(getattr(observed, field.name) for field in fields(CaptureCensus)) == census)
        # No callback or long read follows: later capture reads must not hide an
        # earlier public file mutation. The physical owner retains every source
        # file's complete metadata and the exact sealed parent states.
        files._check()
        captures.check_namespace()

    def commit(self, token, files: RunPublicFiles, captures: TransferredCaptures):
        """Join exact original public owners before the trial's unchanged deadline."""
        try:
            self._check(token, ('pending',)); _require(not self._busy
                and type(files) is RunPublicFiles and type(captures) is TransferredCaptures)
            self._busy = True
            trial = self._trial
            _require(_source(trial) == self._source)
            self._physical(files, captures, True)
            _require(_source(trial) == self._source)
            self._physical(files, captures, True)
            self._check(token, ('pending',))
            _require(self._busy and self._trial is trial
                     and time.monotonic_ns() < self._source[0].original_deadline_ns)
            self._files, self._captures = files, captures
            self._trial = None
            self._phase, self._busy = 'committed', False
        except BaseException as error:
            self._phase = 'failed'; _failure(error)

    def validate(self, token):
        """Verify the committed public scope without invoking expired private guards."""
        try:
            self._check(token, ('committed', 'reconciled')); _require(not self._busy)
            self._busy = True
            self._physical(self._files, self._captures)
            self._check(token, ('committed', 'reconciled'))
            self._busy = False
        except BaseException as error:
            self._phase = 'failed'; _failure(error)

    def projection(self, token) -> PublicRunProjection:
        """Return a detached public record; provenance stays in this open owner."""
        self.validate(token)
        return PublicRunProjection(*self._source[0])

    def reconcile(self, token, replay: ReplayResult) -> PublicRunProjection:
        """Join one complete public resource replay to the original native plan."""
        try:
            self._check(token, ('committed',)); self.validate(token)
            payload, expected, selected, _, _, _ = self._source
            _require(_commitments(replay, selected, payload.replay_plan, payload.budget) == expected)
            freeze = _Freeze(len(expected), len(replay.samples), 1)
            resource = ResourceSnapshot(*(freeze(getattr(replay, name)) for name in _RESOURCE_FIELDS))
            _require(resource == payload.resources)
            self.validate(token)
            self._phase = 'reconciled'
            return self.projection(token)
        except BaseException as error:
            self._phase = 'failed'; _failure(error)

    def close(self, token):
        """Drop only borrowed authority references; close no files or private owners."""
        try:
            _require(type(token) is object and token is self._token)
            self._phase = 'closed'
            self._trial = self._files = self._captures = None
        except BaseException as error:
            self._phase = 'failed'; _failure(error)
