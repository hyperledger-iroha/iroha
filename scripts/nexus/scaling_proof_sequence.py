"""Execute native facts preparation, proof export and replay as one retained chain.

The caller has already completed original vector collection and authenticated
native facts in NativeOutputs. It supplies the exact stopped original store and
reader geometry, the workload plan and original input/runtime custody. This owner
constructs fixed native commands and never exposes a prepared/exported prefix as
verified execution. The final result is still one run, not a scaling release gate.
"""
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import secrets
import time
from typing import Callable

from resource_process import ExecutableImage
from scaling_command import BoundedCommand
from scaling_canonical_proof import (
    MAX_BYTES, MAX_OBJECT_BYTES, MAX_TRIAL_NS, AppliedObservation, CanonicalReplay,
    ReplayBindings, ReplayPlan, _digest, _integer, _object, _path, _plan_snapshot,
)
from scaling_native_outputs import NativeOutputs, PublishedIdentity

_PREPARE_FIELDS = frozenset(('version', 'operation', 'invocation_id', 'facts_sha256', 'facts_bytes',
                            'request_sha256', 'request_bytes', 'bundle_sha256', 'bundle_bytes'))
_EXPORT_FIELDS = frozenset(('version', 'operation', 'invocation_id', 'request_sha256', 'input_sha256',
                           'proof_sha256', 'proof_iroha_hash', 'proof_bytes'))


class ProofSequenceError(ValueError):
    """Closed operation failure with no native stderr or original private inputs."""


def _require(value, code):
    if not value: raise ProofSequenceError(code)


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    if type(error) is ProofSequenceError and str(error).startswith('proof_sequence_'):
        raise ProofSequenceError(str(error)) from None
    raise ProofSequenceError('proof_sequence_failed') from None


@dataclass(frozen=True, slots=True)
class StoppedReader:
    """Original stopped-peer paths and independently allocated complete interval.

    The caller binds last_height to the original stopped-tip command and retains
    that same cleanly stopped peer's store throughout facts/export. It must keep
    first_height=1 so the original genesis context anchors the complete interval.
    """
    block_store: Path
    merge_log: Path
    first_height: int
    last_height: int
    max_committed_blocks: int
    max_store_data_bytes: int
    max_carrier_bytes: int
    max_merge_log_bytes: int
    max_merge_frames: int
    reader_max_output_bytes: int
    max_decode_allocation_bytes: int
    owner_uid: int


def _reader_snapshot(value):
    _require(type(value) is StoppedReader, 'proof_sequence_reader_invalid')
    store, merge = _path(value.block_store), _path(value.merge_log)
    _require(store != merge, 'proof_sequence_store_alias')
    _require(type(value.first_height) is int and value.first_height == 1,
             'proof_sequence_genesis_interval_required')
    last = _integer(value.last_height, 1, 1_000_000)
    maximum = _integer(value.max_committed_blocks, last, 1_000_000)
    # Core owns the additional format-specific limits. Every count here is still
    # explicit, positive and bounded before any command or directory publication.
    sizes = tuple(_integer(getattr(value, name), 1) for name in (
        'max_store_data_bytes', 'max_carrier_bytes', 'max_merge_log_bytes',
        'max_merge_frames', 'reader_max_output_bytes', 'max_decode_allocation_bytes'))
    uid = _integer(value.owner_uid, 0, (1 << 32) - 1)
    return store, merge, (1, last, maximum, *sizes, uid)


class NativeProofSequence:
    """One single-use native prepare/export/replay chain under the original clock."""

    def __init__(self, outputs: NativeOutputs, stopped: StoppedReader,
                 image: ExecutableImage, reader, trial_deadline_ns: int,
                 replay_reply_max_bytes: int, verify_original_inputs: Callable[[], None]):
        _require(not hasattr(self, '_outputs'), 'proof_sequence_readmission')
        _require(type(outputs) is NativeOutputs and isinstance(image, ExecutableImage)
                 and callable(getattr(reader, 'sample', None)) and callable(verify_original_inputs),
                 'proof_sequence_owner_invalid')
        _integer(trial_deadline_ns, 1)
        _require(0 < trial_deadline_ns - time.monotonic_ns() <= MAX_TRIAL_NS,
                 'proof_sequence_deadline_invalid')
        self._store, self._merge, self._reader_args = _reader_snapshot(stopped)
        self._reply_cap = _integer(replay_reply_max_bytes, 1, MAX_BYTES)
        self._outputs, self._image, self._reader = outputs, image, reader
        self._end, self._guard = trial_deadline_ns, verify_original_inputs
        self._binding = (image.path, image.fd, image.identity, image.sha256, image.uuids)
        self._phase, self._replay = 'admitted', None
        self._commands = BoundedCommand(image, reader, trial_deadline_ns, self._verify)
        self._invocations = set()

    def _verify(self):
        _require(self._phase == 'busy' and time.monotonic_ns() < self._end,
                 'proof_sequence_phase_or_deadline')
        image = self._image
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._binding,
                 'proof_sequence_image_changed')
        image.validate()
        self._outputs.validate()
        self._guard()
        _require(self._phase == 'busy' and time.monotonic_ns() < self._end,
                 'proof_sequence_phase_or_deadline')
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._binding,
                 'proof_sequence_image_changed')
        image.validate()
        self._outputs.validate()

    def _call(self, operation, arguments, fields):
        invocation = _digest(secrets.token_hex(32))
        _require(invocation != '0' * 64 and invocation not in self._invocations,
                 'proof_sequence_invocation_invalid')
        self._invocations.add(invocation)
        argv = (str(self._image.path), '--ui-mode', 'plain', 'advanced', 'kura',
                'scaling-evidence', operation, '--invocation-id', invocation, *arguments)
        result = self._commands.run('proof-' + operation, argv, (), 1024)
        self._verify()
        raw = result.stdout
        _require(type(raw) is bytes and raw.endswith(b'\n') and 1 < len(raw) <= 1024,
                 'proof_sequence_reply_framing')
        reply = _object(raw[:-1], fields)
        _require(type(reply['version']) is int and reply['version'] == 1
                 and reply['operation'] == operation and reply['invocation_id'] == invocation,
                 'proof_sequence_reply_operation')
        return reply

    def _prepare(self):
        outputs = self._outputs
        facts = outputs.artifact('facts')
        request_cap, bundle_cap = outputs.allocation('request'), outputs.allocation('bundle')
        total = facts.max_bytes + request_cap + bundle_cap
        _require(total <= MAX_BYTES, 'proof_sequence_prepare_reservation')
        request, bundle = outputs.path('request'), outputs.path('bundle')
        outputs.begin('prepare')
        reply = self._call('prepare', ('--facts', str(facts.path), '--facts-sha256', facts.sha256,
            '--facts-max-bytes', str(facts.max_bytes), '--request-output', str(request),
            '--bundle-output', str(bundle), '--request-max-bytes', str(request_cap),
            '--bundle-max-bytes', str(bundle_cap), '--total-max-bytes', str(total),
            '--reply-max-bytes', '1024'), _PREPARE_FIELDS)
        _require(reply['facts_sha256'] == facts.sha256 and type(reply['facts_bytes']) is int
                 and reply['facts_bytes'] == facts.bytes, 'proof_sequence_facts_changed')
        return outputs.complete(tuple(PublishedIdentity(role, _digest(reply[role + '_sha256']),
            _integer(reply[role + '_bytes'], 1, outputs.allocation(role))) for role in ('request', 'bundle')))

    def _export(self, request, bundle):
        outputs = self._outputs
        cap, path = outputs.allocation('proof'), outputs.path('proof')
        flags = ('first-height', 'last-height', 'max-committed-blocks', 'max-store-data-bytes',
            'max-carrier-bytes', 'max-merge-log-bytes', 'max-merge-frames', 'reader-max-output-bytes',
            'max-decode-allocation-bytes', 'owner-uid')
        reader_args = tuple(part for flag, value in zip(flags, self._reader_args, strict=True)
                            for part in ('--' + flag, str(value)))
        outputs.begin('export')
        reply = self._call('export', ('--request', str(request.path), '--request-sha256', request.sha256,
            '--request-max-bytes', str(request.max_bytes), '--input', str(bundle.path),
            '--input-sha256', bundle.sha256, '--input-max-bytes', str(bundle.max_bytes),
            '--reply-max-bytes', '1024', '--block-store', self._store, '--merge-log', self._merge,
            '--output', str(path), '--output-max-bytes', str(cap), *reader_args), _EXPORT_FIELDS)
        _require(reply['request_sha256'] == request.sha256 and reply['input_sha256'] == bundle.sha256,
                 'proof_sequence_export_input_changed')
        proof_hash = _digest(reply['proof_iroha_hash'], marked=True)
        artifact, = outputs.complete((PublishedIdentity('proof', _digest(reply['proof_sha256']),
            _integer(reply['proof_bytes'], 1, cap)),))
        return artifact, proof_hash

    def run(self, plan: ReplayPlan):
        """Return only after the exact retained proof replays and joins completely."""
        try:
            _require(self._phase == 'admitted' and type(plan) is ReplayPlan,
                     'proof_sequence_phase_invalid')
            # Own every plan field before any callback or native child can run.
            # Frozen caller dataclasses alone do not confer immutable ownership.
            snapshot = _plan_snapshot(plan)
            _require(snapshot[5] == self._reader_args[1],
                     'proof_sequence_stopped_tip_mismatch')
            _require(MAX_OBJECT_BYTES + 4 + len(snapshot[6]) * (MAX_OBJECT_BYTES + 1)
                     <= self._reply_cap, 'proof_sequence_projection_reservation')
            owned_plan = ReplayPlan(*snapshot[:6], tuple(AppliedObservation(tx, height, height)
                for tx, height in snapshot[6]))
            self._phase = 'busy'
            self._verify()
            request, bundle = self._prepare()
            artifact, proof_hash = self._export(request, bundle)
            bindings = ReplayBindings(request.path, request.sha256, request.max_bytes,
                artifact.path, artifact.sha256, proof_hash, artifact.bytes,
                artifact.max_bytes, self._reply_cap)
            self._replay = CanonicalReplay(owned_plan, bindings, self._image, self._reader, self._end, self._verify)
            receipt = self._replay.run()
            self._verify()
            self._outputs.finish()
            self._phase = 'verified'
            return receipt
        except BaseException as error:
            self._phase = 'failed'
            _failure(error)

    def cleanup(self, deadline_ns: int) -> tuple[str, ...]:
        """Reap only retained native handles; keep the output/input custody open."""
        self._phase = 'failed'
        pending = list(self._commands.cleanup(deadline_ns))
        if self._replay is not None: pending.extend(self._replay.cleanup(deadline_ns))
        return tuple(pending)
