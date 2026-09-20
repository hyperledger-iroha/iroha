"""Join a fixed native proof replay to the original complete Applied workload.

The compiled Kagami verifier authenticates canonical Norito, signed requests,
genesis authority, contiguous finality, routing, incarnations and useful effects.
This owner invokes that verifier and joins every projected row to independently
admitted accounts, the seeded schedule and both Applied observations. A parsed
JSON row alone is never a verification result. Original file and runtime custody
belongs to the caller's mandatory verifier, retained through the command and join.

TODO: compose this owner with the fixed trial entrypoint and retained preparation
outputs before opening the public evidence validator's release gate.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import time
from typing import Callable

from resource_process import ExecutableImage, ProcessIdentity
from scaling_command import BoundedCommand

MAX_BYTES = 256 * 1024 * 1024
MAX_CHUNK_BYTES = 65536
MAX_OBJECT_BYTES = 1024
MAX_TRIAL_NS = 7200 * 1_000_000_000
MAX_U64 = (1 << 64) - 1
_HEX = re.compile(r'[0-9a-f]{64}')
_NUMBER = re.compile(r'0|[1-9][0-9]{0,19}')
_HEADER_FIELDS = frozenset(('version', 'operation', 'invocation_id', 'request_sha256',
    'input_sha256', 'proof_sha256', 'proof_iroha_hash', 'proof_bytes'))
_ROW_FIELDS = frozenset(('logical_id', 'phase', 'sequence', 'authority', 'entrypoint_hash',
    'carrier_height', 'carrier_hash', 'merge_entry_hash', 'merge_epoch', 'leaf_index',
    'lane_id', 'dataspace_id', 'incarnation'))
_MARKER = b',"rows":'


class CanonicalProofError(ValueError):
    """Closed public failure code without private input, argv or native stderr."""


def _require(value, code):
    if not value:
        raise CanonicalProofError(code)


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    if type(error) is CanonicalProofError: raise CanonicalProofError(str(error)) from None
    raise CanonicalProofError('canonical_replay_failed') from None


def _integer(value, minimum=0, maximum=MAX_U64):
    _require(type(value) is int and minimum <= value <= maximum, 'canonical_integer_invalid')
    return value


def _digest(value, *, marked=False):
    _require(type(value) is str and _HEX.fullmatch(value) is not None,
             'canonical_digest_invalid')
    if marked:
        _require(int(value[-1], 16) & 1 == 1, 'canonical_digest_invalid')
    return value


def _path(value):
    _require(type(value) is type(Path('/')) and value.is_absolute()
             and str(value) == os.path.abspath(value) and '\x00' not in str(value)
             and len(os.fsencode(value)) <= 4096 and len(value.parts) <= 65,
             'canonical_path_invalid')
    return str(value)


@dataclass(frozen=True, slots=True)
class AppliedObservation:
    """One original signed external request and its two journal-replayed heights."""
    transaction_hash: str
    global_height: int
    local_height: int


@dataclass(frozen=True, slots=True)
class ReplayPlan:
    """Independent fixed workload inputs, never populated from proof result rows.

    Accounts retain the native generator's order. Observations retain complete
    warmup then measurement order after strict collector/resource/trace replay.
    The native request separately authenticates this plan and the signed effects.
    """
    seed: str
    lane_count: int
    accounts: tuple[str, ...]
    warmup_requests: int
    measurement_requests: int
    last_height: int
    observations: tuple[AppliedObservation, ...]


def _plan_snapshot(plan):
    _require(type(plan) is ReplayPlan, 'canonical_plan_invalid')
    seed = _digest(plan.seed)
    lanes = _integer(plan.lane_count, 1, 4)
    _require(lanes in (1, 4), 'canonical_plan_invalid')
    accounts = plan.accounts
    _require(type(accounts) is tuple and 4 <= len(accounts) <= 64 and len(accounts) % 4 == 0
             and all(type(a) is str and 0 < len(a) <= 2048
                     and all(33 <= ord(c) <= 126 for c in a) for a in accounts)
             and len(set(accounts)) == len(accounts), 'canonical_accounts_invalid')
    warmup = _integer(plan.warmup_requests, 0, 64 * 1024)
    measurement = _integer(plan.measurement_requests, 1, 64 * 1024)
    _require(warmup % len(accounts) == measurement % len(accounts) == 0
             and warmup + measurement <= len(accounts) * 1024, 'canonical_schedule_invalid')
    height = _integer(plan.last_height, 1, 1_000_000)
    observations = plan.observations
    _require(type(observations) is tuple and len(observations) == warmup + measurement,
             'canonical_observation_count_invalid')
    rows, seen = [], set()
    for row in observations:
        _require(type(row) is AppliedObservation, 'canonical_observation_invalid')
        tx_hash = _digest(row.transaction_hash, marked=True)
        global_height = _integer(row.global_height, 1, height)
        _require(_integer(row.local_height, 1, height) == global_height,
                 'canonical_applied_height_mismatch')
        _require(tx_hash not in seen, 'canonical_duplicate_request')
        seen.add(tx_hash)
        rows.append((tx_hash, global_height))
    # Own built-in immutable values, not the caller's constructible dataclasses.
    return seed, lanes, tuple(accounts), warmup, measurement, height, tuple(rows)


@dataclass(frozen=True, slots=True)
class ReplayBindings:
    """Original native preparation/export identities and independent byte caps.

    The caller retains every original file and ancestor and verifies it through
    the whole invocation. These declarations alone do not provide file custody.
    Proof identity comes from the completed native export, not a replay response.
    """
    request: Path
    request_sha256: str
    request_max_bytes: int
    proof: Path
    proof_sha256: str
    proof_iroha_hash: str
    proof_bytes: int
    proof_max_bytes: int
    reply_max_bytes: int


def _bindings_snapshot(bindings):
    _require(type(bindings) is ReplayBindings, 'canonical_bindings_invalid')
    request, proof = _path(bindings.request), _path(bindings.proof)
    _require(request != proof, 'canonical_paths_alias')
    request_cap = _integer(bindings.request_max_bytes, 1, MAX_BYTES)
    proof_cap = _integer(bindings.proof_max_bytes, 1, MAX_BYTES)
    size = _integer(bindings.proof_bytes, 1, proof_cap)
    reply_cap = _integer(bindings.reply_max_bytes, 1, MAX_BYTES)
    return (request, _digest(bindings.request_sha256), request_cap,
            proof, _digest(bindings.proof_sha256),
            _digest(bindings.proof_iroha_hash, marked=True), size, proof_cap, reply_cap)


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        _require(key not in result, 'canonical_duplicate_key')
        result[key] = value
    return result


def _number(token):
    _require(_NUMBER.fullmatch(token) is not None, 'canonical_number_invalid')
    return _integer(int(token))


def _invalid_number(_):
    raise CanonicalProofError('canonical_number_invalid')


def _object_end(raw):
    """Find one bounded flat object without allocating or parsing nested values."""
    _require(bool(raw) and raw[0] == 123, 'canonical_object_framing')
    quoted = escaped = False
    for index in range(1, min(len(raw), MAX_OBJECT_BYTES + 1)):
        byte = raw[index]
        if quoted:
            if escaped: escaped = False
            elif byte == 92: escaped = True
            elif byte == 34: quoted = False
        elif byte == 34: quoted = True
        elif byte == 125: return index + 1
        elif byte in (123, 91, 93): raise CanonicalProofError('canonical_nested_value')
    _require(len(raw) < MAX_OBJECT_BYTES, 'canonical_object_bound')
    return None


def _object(raw, fields):
    _require(type(raw) is bytes and 1 < len(raw) <= MAX_OBJECT_BYTES
             and _object_end(raw) == len(raw) and b'\n' not in raw and b'\r' not in raw,
             'canonical_object_framing')
    value = json.loads(raw.decode('utf-8'), object_pairs_hook=_pairs, parse_int=_number,
                       parse_float=_invalid_number, parse_constant=_invalid_number)
    _require(type(value) is dict and value.keys() == fields, 'canonical_fields_invalid')
    return value


class _ProjectionJoiner:
    """Incremental agreement only; this private parser cannot authenticate proofs."""

    def __init__(self, plan, bindings, invocation_id):
        self._plan, self._bindings = plan, bindings
        self._invocation = _digest(invocation_id)
        # Admit the complete native worst-case projection before spawning a child.
        _require(MAX_OBJECT_BYTES + 4 + len(plan[6]) * (MAX_OBJECT_BYTES + 1) <= bindings[8],
                 'canonical_projection_reservation')
        self._phase, self._poisoned = 'header', False
        self._buffer = bytearray()
        self._count = self._bytes = 0
        self._raw_hash, self._row_hash = hashlib.sha256(), hashlib.sha256()
        self._account_offset = int.from_bytes(hashlib.sha256(
            f'gscale-account-offset-v1:{plan[0]}'.encode('ascii')).digest()[:8], 'little') % len(plan[2])

    def consume(self, chunk):
        """Consume one bounded pipe chunk, retaining at most one partial object."""
        try:
            _require(not self._poisoned and self._phase not in ('complete', 'finished'),
                     'canonical_projection_unavailable')
            self._poisoned = True
            _require(type(chunk) is bytes and 0 < len(chunk) <= MAX_CHUNK_BYTES,
                     'canonical_chunk_invalid')
            self._bytes += len(chunk)
            _require(self._bytes <= self._bindings[8], 'canonical_reply_bound')
            self._raw_hash.update(chunk)
            self._buffer.extend(chunk)
            self._consume_available()
            _require(len(self._buffer) <= MAX_OBJECT_BYTES, 'canonical_object_bound')
            self._poisoned = False
        except BaseException as error:
            self._poisoned = True
            self._buffer.clear()
            _failure(error)

    def _consume_available(self):
        while self._buffer:
            if self._phase == 'header':
                end = self._buffer.find(_MARKER, 0, MAX_OBJECT_BYTES)
                if end < 0:
                    _require(len(self._buffer) < MAX_OBJECT_BYTES, 'canonical_header_bound')
                    return
                header = _object(bytes(self._buffer[:end]) + b'}', _HEADER_FIELDS)
                expected = {'version': 1, 'operation': 'replay', 'invocation_id': self._invocation,
                    'request_sha256': self._bindings[1], 'input_sha256': self._bindings[4],
                    'proof_sha256': self._bindings[4], 'proof_iroha_hash': self._bindings[5],
                    'proof_bytes': self._bindings[6]}
                _require(all(type(header[k]) is type(v) and header[k] == v for k, v in expected.items()),
                         'canonical_reply_binding')
                del self._buffer[:end + len(_MARKER)]
                self._phase = 'array'
            elif self._phase == 'array':
                _require(self._buffer[0] == 91, 'canonical_array_framing')
                del self._buffer[:1]
                self._phase = 'row'
            elif self._phase == 'row':
                end = _object_end(self._buffer)
                if end is None: return
                raw = bytes(self._buffer[:end])
                row = _object(raw, _ROW_FIELDS)
                self._join(row)
                self._row_hash.update(len(raw).to_bytes(4, 'big'))
                self._row_hash.update(raw)
                self._count += 1
                del self._buffer[:end]
                self._phase = 'separator'
            elif self._phase == 'separator':
                last = self._count == len(self._plan[6])
                _require(self._buffer[0] == (93 if last else 44), 'canonical_row_count_or_order')
                del self._buffer[:1]
                self._phase = 'footer' if last else 'row'
            elif self._phase == 'footer':
                if len(self._buffer) < 2:
                    _require(self._buffer[0] == 125, 'canonical_footer_invalid')
                    return
                _require(self._buffer == b'}\n', 'canonical_footer_invalid')
                self._buffer.clear()
                self._phase = 'complete'
            else:
                raise CanonicalProofError('canonical_trailing_bytes')

    def _join(self, row):
        seed, lanes, accounts, warmup, _, _, observations = self._plan
        _require(self._count < len(observations), 'canonical_extra_row')
        is_warmup = self._count < warmup
        phase = 'warmup' if is_warmup else 'measurement'
        sequence = self._count + 1 if is_warmup else self._count - warmup + 1
        account = (sequence - 1 + self._account_offset) % len(accounts)
        tx_hash, height = observations[self._count]
        expected = {'logical_id': hashlib.sha256(f'{seed}:{phase}:{sequence}'.encode('ascii')).hexdigest(),
            'phase': phase, 'sequence': sequence, 'authority': accounts[account],
            # SignedTransaction::hash and an External entrypoint's hash have the
            # same raw value in the canonical data-model implementation.
            'entrypoint_hash': tx_hash, 'carrier_height': height,
            'lane_id': account % lanes, 'dataspace_id': 0}
        _require(all(type(row[k]) is type(v) and row[k] == v for k, v in expected.items()),
                 'canonical_row_join_mismatch')
        for field in ('carrier_hash', 'merge_entry_hash', 'incarnation'):
            _digest(row[field], marked=True)
        _integer(row['merge_epoch'])
        _integer(row['leaf_index'], 0, (1 << 32) - 1)
        # Carrier/QC/merge/leaf/incarnation authenticity is established by the
        # native verifier against original genesis authority and signed requests.
        # Python never learns expected authority from these projected fields.

    def finish(self):
        """Complete parser agreement; the caller must first require native exit zero."""
        available = not self._poisoned
        self._poisoned = True
        _require(available and self._phase == 'complete' and not self._buffer
                 and self._count == len(self._plan[6]), 'canonical_projection_incomplete')
        self._phase = 'finished'
        return self._count, self._bytes, self._raw_hash.hexdigest(), self._row_hash.hexdigest()


@dataclass(frozen=True, slots=True)
class CanonicalReplayReceipt:
    """Complete native invocation and trace agreement; not a whole-release receipt."""
    invocation_id: str
    request_sha256: str
    proof_sha256: str
    proof_iroha_hash: str
    proof_bytes: int
    row_count: int
    reply_bytes: int
    reply_sha256: str
    joined_rows_sha256: str
    verifier_sha256: str
    verifier_process: ProcessIdentity


class CanonicalReplay:
    """One fixed native replay with retained inputs and the original trial deadline.

    The launcher keeps this owner until cleanup. Only run() obtains native bytes;
    callers cannot supply a claimed verified flag or a pre-parsed result. Returned
    receipts remain usable only inside the caller's retained original input scope.
    """

    def __init__(self, plan: ReplayPlan, bindings: ReplayBindings, image: ExecutableImage,
                 reader, trial_deadline_ns: int, verify_inputs: Callable[[], None]):
        _require(not hasattr(self, '_plan'), 'canonical_readmission')
        _require(isinstance(image, ExecutableImage) and callable(getattr(reader, 'sample', None))
                 and callable(verify_inputs), 'canonical_owner_invalid')
        _integer(trial_deadline_ns, 1)
        _require(0 < trial_deadline_ns - time.monotonic_ns() <= MAX_TRIAL_NS,
                 'canonical_deadline_invalid')
        self._plan, self._bindings = _plan_snapshot(plan), _bindings_snapshot(bindings)
        image.validate()
        self._image = image
        self._image_binding = (image.path, image.fd, image.identity, image.sha256, image.uuids)
        self._end, self._guard = trial_deadline_ns, verify_inputs
        self._phase = 'admitted'
        self._commands = BoundedCommand(image, reader, trial_deadline_ns, self._verify)

    def _verify(self):
        _require(self._phase == 'busy', 'canonical_phase_invalid')
        _require(time.monotonic_ns() < self._end, 'canonical_deadline_exceeded')
        image = self._image
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._image_binding,
                 'canonical_image_binding_changed')
        image.validate()
        self._guard()
        _require(self._phase == 'busy', 'canonical_phase_invalid')
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._image_binding,
                 'canonical_image_binding_changed')
        image.validate()
        _require(time.monotonic_ns() < self._end, 'canonical_deadline_exceeded')

    def run(self) -> CanonicalReplayReceipt:
        """Require bounded full streaming replay, successful native reap and exact join."""
        try:
            _require(self._phase == 'admitted', 'canonical_phase_invalid')
            self._phase = 'busy'
            self._verify()
            invocation = secrets.token_hex(32)
            _require(_digest(invocation) != '0' * 64, 'canonical_invocation_invalid')
            joiner = _ProjectionJoiner(self._plan, self._bindings, invocation)
            request, request_hash, request_cap, proof, proof_hash, iroha_hash, _, proof_cap, reply_cap = self._bindings
            argv = (str(self._image.path), '--ui-mode', 'plain', 'advanced', 'kura', 'scaling-evidence', 'replay',
                '--invocation-id', invocation, '--request', request, '--request-sha256', request_hash,
                '--request-max-bytes', str(request_cap), '--input', proof, '--input-sha256', proof_hash,
                '--input-max-bytes', str(proof_cap), '--reply-max-bytes', str(reply_cap),
                '--proof-iroha-hash', iroha_hash)
            result = self._commands.run_stream('canonical-replay', argv, (), reply_cap, joiner.consume)
            self._verify()
            count, size, digest, joined_digest = joiner.finish()
            _require(type(result.stdout_bytes) is int and result.stdout_bytes == size
                     and type(result.stdout_sha256) is str and result.stdout_sha256 == digest,
                     'canonical_stream_receipt_mismatch')
            _require(type(result.process) is ProcessIdentity
                     and result.process.executable_sha256 == self._image_binding[3],
                     'canonical_process_receipt_mismatch')
            self._verify()
            receipt = CanonicalReplayReceipt(invocation, request_hash, proof_hash, iroha_hash,
                self._bindings[6], count, size, digest, joined_digest, self._image_binding[3], result.process)
            self._phase = 'verified'
            return receipt
        except BaseException as error:
            self._phase = 'failed'
            _failure(error)

    def cleanup(self, deadline_ns: int) -> tuple[str, ...]:
        """Fail permanently and retain/reap only this owner's original native children."""
        self._phase = 'failed'
        return self._commands.cleanup(deadline_ns)
