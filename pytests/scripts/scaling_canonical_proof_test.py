"""Proof pipe/trace join regressions; no native cryptographic execution is implied."""
from __future__ import annotations

from dataclasses import replace
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace

import pytest

import scaling_canonical_proof as proof

SEED = '1' * 64
INVOCATION = '2' * 64
IMAGE_HASH = '3' * 64


def plan(*, warmup=4, measurement=4, lanes=4, accounts=4):
    # Account syntax is authenticated by native Kagami. These labels exercise
    # exact cross-owner equality without pretending to be signature fixtures.
    return proof.ReplayPlan(SEED, lanes, tuple(f'account-{i:02}' for i in range(accounts)),
        warmup, measurement, 100, tuple(proof.AppliedObservation(f'{i * 2 + 1:064x}',
            i // 4 + 2, i // 4 + 2) for i in range(warmup + measurement)))


def bindings(**overrides):
    return replace(proof.ReplayBindings(Path('/private/retained/request.nrt'), '4' * 64, 4096,
        Path('/private/retained/proof.nrt'), '5' * 64, '7' * 64, 1234, 8192,
        128 * 1024), **overrides)


def projection(run=None, files=None, invocation=INVOCATION):
    run, files = run or plan(), files or bindings()
    header = {'version': 1, 'operation': 'replay', 'invocation_id': invocation,
        'request_sha256': files.request_sha256, 'input_sha256': files.proof_sha256,
        'proof_sha256': files.proof_sha256, 'proof_iroha_hash': files.proof_iroha_hash,
        'proof_bytes': files.proof_bytes}
    offset = int.from_bytes(hashlib.sha256(f'gscale-account-offset-v1:{run.seed}'.encode()).digest()[:8],
                            'little') % len(run.accounts)
    rows = []
    for index, observation in enumerate(run.observations):
        warmup = index < run.warmup_requests
        phase = 'warmup' if warmup else 'measurement'
        sequence = index + 1 if warmup else index - run.warmup_requests + 1
        account = (sequence - 1 + offset) % len(run.accounts)
        rows.append({'logical_id': hashlib.sha256(f'{run.seed}:{phase}:{sequence}'.encode()).hexdigest(),
            'phase': phase, 'sequence': sequence, 'authority': run.accounts[account],
            'entrypoint_hash': observation.transaction_hash, 'carrier_height': observation.global_height,
            'carrier_hash': '9' * 64, 'merge_entry_hash': 'b' * 64, 'merge_epoch': 1,
            'leaf_index': index, 'lane_id': account % run.lane_count, 'dataspace_id': 0,
            'incarnation': 'd' * 64})
    return header, rows


def wire(header, rows):
    return (json.dumps(header, separators=(',', ':'))[:-1].encode() + b',"rows":'
            + json.dumps(rows, separators=(',', ':')).encode() + b'}\n')


def joiner(run=None, files=None):
    return proof._ProjectionJoiner(proof._plan_snapshot(run or plan()),
        proof._bindings_snapshot(files or bindings()), INVOCATION)


def feed(reader, raw, chunk=65536):
    for start in range(0, len(raw), chunk):
        reader.consume(raw[start:start + chunk])


@pytest.mark.parametrize('chunk', [1, 2, 3, 7, 31, 511, 1024, 65536])
@pytest.mark.parametrize('lanes,warmup', [(1, 4), (4, 4), (4, 0)])
def test_complete_ordered_projection_at_every_boundary(chunk, lanes, warmup):
    run = plan(lanes=lanes, warmup=warmup)
    reader = joiner(run)
    raw = wire(*projection(run))
    feed(reader, raw, chunk)
    count, size, digest, joined = reader.finish()
    assert count == len(run.observations)
    assert size == len(raw)
    assert digest == hashlib.sha256(raw).hexdigest()
    assert len(joined) == 64
    assert len(reader._buffer) == 0
    with pytest.raises(proof.CanonicalProofError): reader.finish()


@pytest.mark.parametrize('field,value', [
    ('version', True), ('version', 2), ('operation', 'export'),
    ('invocation_id', '0' * 64), ('request_sha256', '0' * 64), ('input_sha256', '0' * 64),
    ('proof_sha256', '0' * 64), ('proof_iroha_hash', 'f' * 64), ('proof_bytes', 1233),
    ('proof_bytes', True), ('extra', 0),
])
def test_header_binds_original_native_artifacts(field, value):
    header, rows = projection()
    header[field] = value
    reader = joiner()
    with pytest.raises(proof.CanonicalProofError): feed(reader, wire(header, rows))
    with pytest.raises(proof.CanonicalProofError): reader.finish()


@pytest.mark.parametrize('field,value', [
    ('logical_id', '0' * 64), ('phase', 'measurement'), ('sequence', 2), ('sequence', True),
    ('authority', 'another-account'), ('entrypoint_hash', 'f' * 64), ('carrier_height', 99),
    ('carrier_height', True), ('lane_id', 99), ('lane_id', True), ('dataspace_id', 1),
    ('dataspace_id', False), ('carrier_hash', '8' * 64), ('merge_entry_hash', 'b' * 63),
    ('incarnation', 'hash:' + 'D' * 64 + '#1234'), ('merge_epoch', -1),
    ('merge_epoch', 1.0), ('merge_epoch', True), ('leaf_index', 1 << 32),
    ('leaf_index', None), ('extra', 1),
])
def test_every_row_field_is_joined_or_strictly_typed(field, value):
    header, rows = projection()
    rows[0][field] = value
    reader = joiner()
    with pytest.raises(proof.CanonicalProofError): feed(reader, wire(header, rows))
    with pytest.raises(proof.CanonicalProofError): reader.finish()


@pytest.mark.parametrize('mutation', ['empty', 'prefix', 'extra', 'duplicate', 'reorder', 'late_bad'])
def test_no_successful_prefix_duplicate_or_reordering(mutation):
    header, rows = projection()
    if mutation == 'empty': rows = []
    elif mutation == 'prefix': rows.pop()
    elif mutation == 'extra': rows.append(rows[-1])
    elif mutation == 'duplicate': rows[1] = rows[0]
    elif mutation == 'reorder': rows[1], rows[2] = rows[2], rows[1]
    else: rows[-1]['carrier_height'] += 1
    reader = joiner()
    with pytest.raises(proof.CanonicalProofError): feed(reader, wire(header, rows), 31)
    with pytest.raises(proof.CanonicalProofError): reader.finish()


@pytest.mark.parametrize('suffix', [b'', b'}', b'}\r\n', b'}\n\n', b'}\n{}', b'}\nsecret'])
def test_terminal_framing_is_exact(suffix):
    raw = wire(*projection())[:-2] + suffix
    reader = joiner()
    with pytest.raises(proof.CanonicalProofError):
        feed(reader, raw)
        reader.finish()
    with pytest.raises(proof.CanonicalProofError): reader.finish()


def test_caught_incomplete_finish_cannot_resume():
    raw = wire(*projection())
    reader = joiner()
    feed(reader, raw[:-1])
    with pytest.raises(proof.CanonicalProofError): reader.finish()
    with pytest.raises(proof.CanonicalProofError): reader.consume(raw[-1:])


@pytest.mark.parametrize('raw', [
    b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}', b'{"x":1.0}', b'{"x":1e3}',
    b'{"x":-1}', b'{"x":18446744073709551616}', b'{"x":' + b'1' * 200 + b'}',
    b'{"x":{}}', b'{"x":[]}', b'{"x":"\xff"}', b'{"x":1}\n',
    b'{"x":1}extra', b'{"x":' + b'[' * 1000 + b']' * 1000 + b'}',
])
def test_bounded_flat_json_rejects_hostile_values(raw):
    with pytest.raises((proof.CanonicalProofError, ValueError)):
        proof._object(raw, frozenset(('x',)))


def test_flat_scan_respects_escaped_quotes_and_braces():
    raw = b'{"x":"a\\\"}b"}'
    assert proof._object(raw, frozenset(('x',))) == {'x': 'a"}b'}
    for size in range(1, len(raw)):
        assert proof._object_end(raw[:size]) is None


@pytest.mark.parametrize('raw', [b'', bytearray(b'a'), b'a' * 65537, b'{' + b'a' * 1024])
def test_chunk_and_partial_header_bounds(raw):
    reader = joiner()
    with pytest.raises(proof.CanonicalProofError): reader.consume(raw)
    with pytest.raises(proof.CanonicalProofError): feed(reader, wire(*projection()))


def test_oversized_row_rejected_before_json_decode(monkeypatch):
    header, rows = projection()
    rows[0]['authority'] = 'a' * 1025
    raw = wire(header, rows)
    real = proof._object
    lengths = []
    def bounded(raw, fields):
        lengths.append(len(raw))
        return real(raw, fields)
    monkeypatch.setattr(proof, '_object', bounded)
    reader = joiner()
    with pytest.raises(proof.CanonicalProofError): feed(reader, raw, 31)
    assert lengths and max(lengths) <= 1024


@pytest.mark.parametrize('field,value', [
    ('seed', 'bad'), ('lane_count', 2), ('lane_count', True), ('accounts', ['a'] * 4),
    ('accounts', ('a',) * 4), ('accounts', ('a', 'b', 'c', '\n')),
    ('warmup_requests', 1), ('measurement_requests', 0), ('measurement_requests', True),
    ('last_height', 1), ('observations', ()),
])
def test_independent_plan_is_admitted_before_a_command(field, value):
    with pytest.raises(proof.CanonicalProofError): proof._plan_snapshot(replace(plan(), **{field: value}))


@pytest.mark.parametrize('change', ['height', 'local_type', 'unmarked', 'duplicate', 'past_tip'])
def test_original_observation_identity_and_scope_are_mandatory(change):
    run = plan()
    rows = list(run.observations)
    if change == 'height': rows[0] = replace(rows[0], local_height=3)
    elif change == 'local_type': rows[0] = replace(rows[0], local_height=True)
    elif change == 'unmarked': rows[0] = replace(rows[0], transaction_hash='2' * 64)
    elif change == 'duplicate': rows[1] = rows[0]
    else: rows[0] = replace(rows[0], global_height=101, local_height=101)
    with pytest.raises(proof.CanonicalProofError): proof._plan_snapshot(replace(run, observations=tuple(rows)))


def test_complete_native_maximum_schedule_has_no_buffered_reply_cap():
    run = replace(plan(accounts=64, warmup=0, measurement=65536), last_height=20000)
    snapshot = proof._plan_snapshot(run)
    files = bindings(reply_max_bytes=128 * 1024 * 1024)
    reader = joiner(run, files)
    assert len(snapshot[6]) == 65536
    assert len(reader._buffer) == 0
    with pytest.raises(proof.CanonicalProofError): joiner(run, bindings())


def test_complete_65536_row_stream_keeps_only_a_partial_object():
    run = replace(plan(accounts=64, warmup=0, measurement=65536), last_height=20000)
    files = bindings(reply_max_bytes=128 * 1024 * 1024)
    reader = joiner(run, files)
    header, template = projection(files=files)
    reader.consume(json.dumps(header, separators=(',', ':'))[:-1].encode() + b',"rows":[')
    offset = int.from_bytes(hashlib.sha256(f'gscale-account-offset-v1:{SEED}'.encode()).digest()[:8],
                            'little') % 64
    for index, observation in enumerate(run.observations):
        sequence = index + 1
        account = (index + offset) % 64
        row = template[0] | {'logical_id': hashlib.sha256(f'{SEED}:measurement:{sequence}'.encode()).hexdigest(),
            'phase': 'measurement', 'sequence': sequence, 'authority': run.accounts[account],
            'entrypoint_hash': observation.transaction_hash, 'carrier_height': observation.global_height,
            'leaf_index': index, 'lane_id': account % 4}
        raw = (b',' if index else b'') + json.dumps(row, separators=(',', ':')).encode()
        reader.consume(raw)
        assert len(reader._buffer) <= proof.MAX_OBJECT_BYTES
    reader.consume(b']}\n')
    count, size, *_ = reader.finish()
    assert count == 65536
    assert size > 24 * 1024 * 1024


@pytest.mark.parametrize('field,value', [
    ('request', Path('relative')), ('request', Path('/private/../request')),
    ('proof', Path('/private/retained/request.nrt')), ('request_sha256', '0' * 63),
    ('proof_sha256', 'A' * 64), ('proof_iroha_hash', '2' * 64), ('proof_bytes', 0),
    ('proof_bytes', 8193), ('proof_max_bytes', 0), ('reply_max_bytes', 268435457),
    ('request_max_bytes', True),
])
def test_bound_native_inputs_reject_aliases_and_changed_reservations(field, value):
    with pytest.raises(proof.CanonicalProofError): proof._bindings_snapshot(bindings(**{field: value}))


class FakeImage:
    def __init__(self):
        self.path, self.fd, self.identity = Path('/private/bin/kagami'), 7, ('inode', 1)
        self.sha256, self.uuids = IMAGE_HASH, ('uuid',)
        self.validations = 0
    def validate(self): self.validations += 1


@pytest.fixture
def native(monkeypatch):
    run, files = plan(), bindings()
    image = FakeImage()
    identity = proof.ProcessIdentity(123, 501, 100, 1, 99, 'uuid', IMAGE_HASH)
    state = SimpleNamespace(calls=[], cleanups=[], guards=0, raw=None, fail=False, alter=None,
                            stream_size=None, stream_hash=None, process=identity, after=None)
    class Commands:
        def __init__(self, actual_image, reader, deadline, verify_inputs):
            assert actual_image is image
            self.guard, self.deadline = verify_inputs, deadline
        def run_stream(self, role, argv, fds, cap, consume):
            state.calls.append((role, argv, fds, cap))
            self.guard()
            raw = state.raw if state.raw is not None else wire(*projection(run, files))
            for start in range(0, len(raw), 31): consume(raw[start:start + 31])
            if state.after: state.after()
            if state.fail: raise RuntimeError('private secret in native failure')
            self.guard()
            return SimpleNamespace(stdout_bytes=state.stream_size if state.stream_size is not None else len(raw),
                stdout_sha256=state.stream_hash or hashlib.sha256(raw).hexdigest(), process=state.process)
        def cleanup(self, deadline):
            state.cleanups.append(deadline)
            return ('original-child',)
    monkeypatch.setattr(proof, 'ExecutableImage', FakeImage)
    monkeypatch.setattr(proof, 'BoundedCommand', Commands)
    monkeypatch.setattr(proof.time, 'monotonic_ns', lambda: 100)
    monkeypatch.setattr(proof.secrets, 'token_hex', lambda size: INVOCATION)
    def guard():
        state.guards += 1
        if state.alter: state.alter()
    owner = proof.CanonicalReplay(run, files, image, SimpleNamespace(sample=lambda: None), 100000, guard)
    return owner, state, image, run, files


def test_fixed_native_argv_and_complete_receipt(native):
    owner, state, image, run, files = native
    receipt = owner.run()
    assert receipt.row_count == 8
    assert receipt.proof_sha256 == files.proof_sha256
    assert receipt.verifier_process is state.process
    assert receipt.verifier_sha256 == IMAGE_HASH
    role, argv, fds, cap = state.calls[0]
    assert argv[:7] == (str(image.path), '--ui-mode', 'plain', 'advanced', 'kura', 'scaling-evidence', 'replay')
    assert dict(zip(argv[7::2], argv[8::2], strict=True)) == {
        '--invocation-id': INVOCATION, '--request': str(files.request), '--request-sha256': files.request_sha256,
        '--request-max-bytes': '4096', '--input': str(files.proof), '--input-sha256': files.proof_sha256,
        '--input-max-bytes': '8192', '--reply-max-bytes': str(cap), '--proof-iroha-hash': files.proof_iroha_hash}
    assert role == 'canonical-replay' and fds == () and cap == files.reply_max_bytes
    assert state.guards >= 4
    with pytest.raises(proof.CanonicalProofError): owner.run()


def test_complete_rows_followed_by_nonzero_native_exit_never_return_receipt(native):
    owner, state, *_ = native
    state.fail = True
    with pytest.raises(proof.CanonicalProofError, match='^canonical_replay_failed$'): owner.run()
    state.fail = False
    with pytest.raises(proof.CanonicalProofError): owner.run()
    assert owner.cleanup(200000) == ('original-child',)
    assert state.cleanups == [200000]


@pytest.mark.parametrize('field,value', [
    ('stream_size', 1), ('stream_size', True), ('stream_hash', '0' * 64),
    ('process', None),
])
def test_stream_terminal_receipt_must_match_exact_observed_bytes(native, field, value):
    owner, state, *_ = native
    setattr(state, field, value)
    with pytest.raises(proof.CanonicalProofError): owner.run()


def test_truncated_success_reply_is_failure(native):
    owner, state, *_ = native
    state.raw = wire(*projection())[:-1]
    with pytest.raises(proof.CanonicalProofError): owner.run()


def test_original_input_custody_failure_is_terminal_and_redacted(native):
    owner, state, *_ = native
    def change(): raise OSError('private key and path')
    state.alter = change
    with pytest.raises(proof.CanonicalProofError, match='^canonical_replay_failed$'): owner.run()
    assert state.calls == []


def test_verifier_image_change_during_guard_is_rejected(native):
    owner, state, image, *_ = native
    state.alter = lambda: setattr(image, 'sha256', 'f' * 64)
    with pytest.raises(proof.CanonicalProofError, match='canonical_image_binding_changed'): owner.run()
    assert state.calls == []


def test_deadline_is_not_extended_after_complete_stream(native, monkeypatch):
    owner, state, *_ = native
    state.after = lambda: monkeypatch.setattr(proof.time, 'monotonic_ns', lambda: 100001)
    with pytest.raises(proof.CanonicalProofError, match='canonical_deadline_exceeded'): owner.run()


def test_caller_plan_mutation_cannot_rewrite_owned_expectations(native):
    owner, state, _, run, files = native
    object.__setattr__(run.observations[0], 'global_height', 99)
    object.__setattr__(run, 'seed', '0' * 64)
    object.__setattr__(files, 'proof_sha256', '0' * 64)
    state.raw = wire(*projection())
    receipt = owner.run()
    assert receipt.proof_sha256 == '5' * 64


def test_reentrant_guard_poison_cannot_be_swallowed(native):
    owner, state, *_ = native
    def reenter():
        with pytest.raises(proof.CanonicalProofError): owner.run()
    state.alter = reenter
    with pytest.raises(proof.CanonicalProofError): owner.run()
    assert state.calls == []
