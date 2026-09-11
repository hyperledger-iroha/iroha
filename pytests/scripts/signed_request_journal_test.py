"""Independent retention protocol and real raw ResourceReplay adverse controls.

Opaque bytes deliberately do not claim canonical/signature validity. The exact
retained bytes are an input to the compiled verifier, never its replacement.
"""
from dataclasses import FrozenInstanceError
import copy
import hashlib
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / 'scripts/nexus'), str(ROOT / 'scripts/tests')]
import signed_request_journal as signed
from signed_request_fixture import command
from resource_replay_test import Fixture
import resource_replay as replay


def plan(cohort='measurement', sequence=1):
    return dict(cohort=cohort, sequence=sequence,
                logical_id=hashlib.sha256(f'{cohort}:{sequence}'.encode()).hexdigest(),
                scheduled_offset_ns=-10 if cohort == 'warmup' else 0, account_index=0)


def rows(raw=b'opaque retained bytes'):
    p, tx_hash = plan(), '1' * 64
    return ([dict(event='plan', scheduled_requests=1), dict(event='scheduled', index=0, plan=p),
             dict(event='clock_started', initial_offset_ns=-100)] + command(0, p, tx_hash, raw) +
            [dict(event='prepared', index=0, hash=tx_hash, offset_ns=-1),
             dict(event='offer', index=0, hash=tx_hash, offset_ns=0),
             dict(event='request_final', plan=p, hash=tx_hash, offer_offset_ns=0),
             dict(event='collection_finished', passed=True, failure=None)])


def run(events, limit=4 * 1024 * 1024):
    owner = signed.SignedRequestReader(limit)
    for event in events:
        assert owner.consume(event) is None
    return owner, owner.finish()


def rejected(events, match='signed_', limit=4 * 1024 * 1024):
    owner = signed.SignedRequestReader(limit)
    with pytest.raises(signed.SignedRequestError, match=match):
        for event in events: owner.consume(event)
        owner.finish()
    with pytest.raises(signed.SignedRequestError): owner.finish()
    with pytest.raises(signed.SignedRequestError): owner.consume(dict(event='plan', scheduled_requests=1))
    assert owner._active is None
    assert owner._requests == {}


@pytest.mark.parametrize('length', [1, 4095, 4096, 4097, 1024 * 1024])
def test_exact_chunk_and_request_boundaries_return_only_complete_immutable_bytes(length):
    raw = bytes(range(256)) * (length // 256) + bytes(range(length % 256))
    owner, result = run(rows(raw))
    assert len(result) == 1
    item = result[0]
    assert type(result) is tuple and type(item) is signed.RetainedRequest
    assert type(item.canonical_bytes) is bytes and item.canonical_bytes == raw
    assert item.canonical_sha256 == hashlib.sha256(raw).hexdigest()
    assert item.index == 0 and item.hash == '1' * 64
    assert item.plan == signed.RequestPlan(**plan())
    with pytest.raises(FrozenInstanceError): item.index = 1
    with pytest.raises(FrozenInstanceError): item.plan.account_index = 1
    with pytest.raises(signed.SignedRequestError): owner.finish()
    assert not hasattr(item, 'signature_verified')


def test_concurrent_preparation_can_complete_in_reverse_index_order_with_exact_cohorts():
    plans = [plan('warmup'), plan()]
    events = [dict(event='plan', scheduled_requests=2)]
    events += [dict(event='scheduled', index=i, plan=p) for i, p in enumerate(plans)]
    events.append(dict(event='clock_started', initial_offset_ns=-100))
    for index in (1, 0):
        events += command(index, plans[index], str(index + 1) * 63 + '1', bytes([index + 1]) * 4097)
        events.append(dict(event='prepared', index=index, hash=str(index + 1) * 63 + '1', offset_ns=-20))
    for index in (0, 1):
        tx_hash = str(index + 1) * 63 + '1'
        offset = plans[index]['scheduled_offset_ns']
        events += [dict(event='offer', index=index, hash=tx_hash, offset_ns=offset),
                   dict(event='request_final', plan=plans[index], hash=tx_hash, offer_offset_ns=offset)]
    events.append(dict(event='collection_finished', passed=True, failure=None))
    _, result = run(events)
    assert tuple(row.index for row in result) == (0, 1)
    assert tuple(row.plan.cohort for row in result) == ('warmup', 'measurement')
    assert result[0].canonical_bytes != result[1].canonical_bytes


@pytest.mark.parametrize('value', [True, 1.0, None, 0, -1, 256 * 1024 * 1024 + 1])
def test_constructor_rejects_unbounded_or_untyped_journal_cap(value):
    with pytest.raises(signed.SignedRequestError): signed.SignedRequestReader(value)


def test_total_canonical_storage_is_at_most_half_admitted_journal_bytes():
    raw = b'123456789'
    _, result = run(rows(raw), limit=18)
    assert sum(len(row.canonical_bytes) for row in result) == 18 // 2
    rejected(rows(raw), match='signed_cumulative_limit', limit=17)


@pytest.mark.parametrize('event', ['signed_request_begin', 'signed_request_chunk', 'signed_request_retained'])
@pytest.mark.parametrize('mutation', ['missing', 'extra'])
def test_every_retention_object_has_exact_required_fields(event, mutation):
    events = rows()
    row = next(row for row in events if row['event'] == event)
    if mutation == 'missing': row.pop('index')
    else: row['compatibility'] = True
    rejected(events)


@pytest.mark.parametrize('field', ['index', 'byte_length', 'chunk_count'])
@pytest.mark.parametrize('value', [True, 1.0, -1, None, 1 << 128])
@pytest.mark.parametrize('event', ['signed_request_begin', 'signed_request_retained'])
def test_begin_and_terminal_numbers_are_exact_bounded_integers(event, field, value):
    events = rows(); next(row for row in events if row['event'] == event)[field] = value
    rejected(events)


@pytest.mark.parametrize('field', ['index', 'chunk_index', 'offset'])
@pytest.mark.parametrize('value', [True, 1.0, -1, None, 1 << 128])
def test_chunk_coordinates_are_exact_bounded_integers(field, value):
    events = rows(); next(row for row in events if row['event'] == 'signed_request_chunk')[field] = value
    rejected(events)


@pytest.mark.parametrize('event', ['signed_request_begin', 'signed_request_retained'])
@pytest.mark.parametrize('field', ['hash', 'canonical_sha256'])
@pytest.mark.parametrize('value', [None, 1, 'A' * 64, 'a' * 63, 'a' * 65, 'a' * 63 + '\n'])
def test_hash_and_digest_text_is_strict_without_untrusted_errors(event, field, value):
    events = rows(); next(row for row in events if row['event'] == event)[field] = value
    rejected(events)


@pytest.mark.parametrize('mutation', ['encoding', 'max_plus_one', 'zero_bytes', 'zero_chunks', 'wrong_count',
    'wrong_plan', 'bool_plan', 'extra_plan', 'chunk_owner', 'chunk_skip', 'chunk_offset', 'short_chunk',
    'long_chunk', 'uppercase_hex', 'hex_space', 'hex_unicode', 'hex_empty', 'hex_nonstring', 'body_changed',
    'terminal_hash', 'terminal_digest', 'terminal_length', 'terminal_count', 'hash_marker'])
def test_independent_rehashed_protocol_corruptions_poison_reader(mutation):
    events = rows(b'a' * 4097)
    begin = next(r for r in events if r['event'] == 'signed_request_begin')
    chunk = next(r for r in events if r['event'] == 'signed_request_chunk')
    terminal = next(r for r in events if r['event'] == 'signed_request_retained')
    if mutation == 'encoding': begin['encoding'] = 'http.versioned.envelope'
    if mutation == 'max_plus_one': begin['byte_length'] = 1024 * 1024 + 1
    if mutation == 'zero_bytes': begin['byte_length'] = 0
    if mutation == 'zero_chunks': begin['chunk_count'] = 0
    if mutation == 'wrong_count': begin['chunk_count'] = 1
    if mutation == 'wrong_plan': begin['plan']['logical_id'] = '2' * 64
    if mutation == 'bool_plan': begin['plan']['sequence'] = True
    if mutation == 'extra_plan': begin['plan']['lane'] = 1
    if mutation == 'chunk_owner': chunk['index'] = 1
    if mutation == 'chunk_skip': chunk['chunk_index'] = 1
    if mutation == 'chunk_offset': chunk['offset'] = 4096
    if mutation == 'short_chunk': chunk['bytes_hex'] = chunk['bytes_hex'][:-2]
    if mutation == 'long_chunk': chunk['bytes_hex'] += '61'
    if mutation == 'uppercase_hex': chunk['bytes_hex'] = 'AA' * 4096
    if mutation == 'hex_space': chunk['bytes_hex'] = '  ' + chunk['bytes_hex'][2:]
    if mutation == 'hex_unicode': chunk['bytes_hex'] = 'éé' + chunk['bytes_hex'][2:]
    if mutation == 'hex_empty': chunk['bytes_hex'] = ''
    if mutation == 'hex_nonstring': chunk['bytes_hex'] = ['61']
    if mutation == 'body_changed': chunk['bytes_hex'] = '62' + chunk['bytes_hex'][2:]
    if mutation == 'terminal_hash': terminal['hash'] = '3' * 64
    if mutation == 'terminal_digest': terminal['canonical_sha256'] = '3' * 64
    if mutation == 'terminal_length': terminal['byte_length'] -= 1
    if mutation == 'terminal_count': terminal['chunk_count'] = 1
    if mutation == 'hash_marker': begin['hash'] = '2' * 64
    rejected(events)


@pytest.mark.parametrize('boundary', ['before_chunk', 'before_terminal'])
@pytest.mark.parametrize('interloper', ['prepared', 'offer', 'scheduled', 'resource_request', 'signed_request_begin', 'collection_finished'])
def test_no_other_row_can_interleave_a_retention_command(boundary, interloper):
    events = rows()
    index = next(i for i, r in enumerate(events) if r['event'] == ('signed_request_chunk' if boundary == 'before_chunk' else 'signed_request_retained'))
    events.insert(index, dict(event=interloper))
    rejected(events, match='signed_command_interleaved')


@pytest.mark.parametrize('mutation', ['omit_schedule', 'duplicate_schedule', 'wrong_schedule_index', 'schedule_after_clock',
    'clock_missing', 'before_clock', 'chunk_missing', 'terminal_missing', 'duplicate_begin', 'prepared_missing',
    'prepared_before_retention', 'prepared_duplicate', 'prepared_hash', 'offer_missing', 'offer_before_prepared',
    'offer_duplicate', 'offer_hash', 'offer_time', 'final_missing', 'final_duplicate', 'final_hash', 'final_plan',
    'final_offer', 'collection_missing', 'collection_false', 'after_finished', 'signed_unknown'])
def test_complete_schedule_retention_prepared_offer_and_final_coverage(mutation):
    events = rows()
    def row(name): return next(r for r in events if r['event'] == name)
    if mutation == 'omit_schedule': events.remove(row('scheduled'))
    if mutation == 'duplicate_schedule': events.insert(2, copy.deepcopy(row('scheduled')))
    if mutation == 'wrong_schedule_index': row('scheduled')['index'] = 1
    if mutation == 'schedule_after_clock': events.insert(3, events.pop(1))
    if mutation == 'clock_missing': events.remove(row('clock_started'))
    if mutation == 'before_clock': events.insert(3, events.pop(2))
    if mutation == 'chunk_missing': events.remove(row('signed_request_chunk'))
    if mutation == 'terminal_missing': events.remove(row('signed_request_retained'))
    if mutation == 'duplicate_begin':
        group = [copy.deepcopy(r) for r in events if r['event'] in signed.EVENTS]
        events[6:6] = group
    if mutation == 'prepared_missing': events.remove(row('prepared'))
    if mutation == 'prepared_before_retention': events.insert(3, events.pop(events.index(row('prepared'))))
    if mutation == 'prepared_duplicate': events.insert(events.index(row('offer')), copy.deepcopy(row('prepared')))
    if mutation == 'prepared_hash': row('prepared')['hash'] = '3' * 64
    if mutation == 'offer_missing': events.remove(row('offer'))
    if mutation == 'offer_before_prepared':
        i, j = events.index(row('prepared')), events.index(row('offer')); events[i], events[j] = events[j], events[i]
    if mutation == 'offer_duplicate': events.insert(events.index(row('request_final')), copy.deepcopy(row('offer')))
    if mutation == 'offer_hash': row('offer')['hash'] = '3' * 64
    if mutation == 'offer_time': row('offer')['offset_ns'] = -2
    if mutation == 'final_missing': events.remove(row('request_final'))
    if mutation == 'final_duplicate': events.insert(-1, copy.deepcopy(row('request_final')))
    if mutation == 'final_hash': row('request_final')['hash'] = '3' * 64
    if mutation == 'final_plan': row('request_final')['plan'] = plan(sequence=2)
    if mutation == 'final_offer': row('request_final')['offer_offset_ns'] = 1
    if mutation == 'collection_missing': events.pop()
    if mutation == 'collection_false': row('collection_finished')['passed'] = False
    if mutation == 'after_finished': events.append(dict(event='ignored'))
    if mutation == 'signed_unknown': events.insert(3, dict(event='signed_request_legacy'))
    rejected(events)


def test_failed_finish_and_external_abort_never_expose_partial_or_reusable_request():
    owner = signed.SignedRequestReader(1024)
    for row in rows()[:-1]: owner.consume(row)
    with pytest.raises(signed.SignedRequestError): owner.finish()
    with pytest.raises(signed.SignedRequestError): owner.consume(rows()[-1])
    other = signed.SignedRequestReader(1024)
    for row in rows(): other.consume(row)
    other.abort()
    with pytest.raises(signed.SignedRequestError): other.finish()


@pytest.mark.parametrize('mutation', ['missing_all', 'missing_chunk', 'bad_digest', 'interleaved_resource',
    'terminal_then_offer', 'changed_prepared_hash', 'changed_final_hash', 'duplicate_index', 'unknown_field'])
def test_actual_rehashed_journal_resource_replay_requires_complete_retention(tmp_path, mutation):
    fixture = Fixture(tmp_path)
    positive = fixture.run()
    assert len(positive.signed_requests) == 1
    assert positive.signed_requests[0].hash == next(r['hash'] for r in fixture.events if r['event'] == 'request_final')
    events = fixture.events
    def row(name): return next(r for r in events if r['event'] == name)
    if mutation == 'missing_all': events[:] = [r for r in events if r['event'] not in signed.EVENTS]
    if mutation == 'missing_chunk': events.remove(row('signed_request_chunk'))
    if mutation == 'bad_digest': row('signed_request_chunk')['bytes_hex'] = '00' + row('signed_request_chunk')['bytes_hex'][2:]
    if mutation == 'interleaved_resource': events.insert(events.index(row('signed_request_chunk')), copy.deepcopy(row('resource_request')))
    if mutation == 'terminal_then_offer': events.remove(row('prepared'))
    if mutation == 'changed_prepared_hash': row('prepared')['hash'] = '3' * 64
    if mutation == 'changed_final_hash': row('request_final')['hash'] = '3' * 64
    if mutation == 'duplicate_index': events.insert(events.index(row('offer')), copy.deepcopy(row('prepared')))
    if mutation == 'unknown_field': row('signed_request_begin')['legacy'] = True
    with pytest.raises(replay.ReplayError, match='signed_'): fixture.run()


def test_hash_refreshed_opaque_bytes_are_exposed_for_compiled_authentication_not_claimed_valid():
    events = rows(b'not a canonical signed transaction')
    _, result = run(events)
    assert result[0].canonical_bytes == b'not a canonical signed transaction'
    assert result[0].hash == '1' * 64
    assert not hasattr(result[0], 'canonical_verified')


@pytest.mark.parametrize('mutation', ['none', 'cumulative', 'duplicate_hash', 'duplicate_logical', 'duplicate_identity'])
def test_independent_multi_request_totals_and_unique_owners(mutation):
    plans = [plan(sequence=1), plan(sequence=2)]
    events = [dict(event='plan', scheduled_requests=2)]
    events += [dict(event='scheduled', index=i, plan=copy.deepcopy(p)) for i, p in enumerate(plans)]
    events.append(dict(event='clock_started', initial_offset_ns=-100))
    for index, p in enumerate(plans):
        tx_hash = ('1' if mutation == 'duplicate_hash' else str(index * 2 + 1)) * 64
        events += command(index, p, tx_hash, b'12345678')
        events += [dict(event='prepared', index=index, hash=tx_hash, offset_ns=-1),
                   dict(event='offer', index=index, hash=tx_hash, offset_ns=0),
                   dict(event='request_final', plan=p, hash=tx_hash, offer_offset_ns=0)]
    events.append(dict(event='collection_finished', passed=True, failure=None))
    if mutation == 'duplicate_logical': events[2]['plan']['logical_id'] = events[1]['plan']['logical_id']
    if mutation == 'duplicate_identity': events[2]['plan']['sequence'] = 1
    if mutation == 'none':
        _, result = run(events, limit=32)
        assert len(result) == 2 and sum(len(item.canonical_bytes) for item in result) == 16
    else:
        rejected(events, limit=31 if mutation == 'cumulative' else 32)


def test_caller_json_mutation_after_consumption_cannot_change_retained_plan():
    events = rows()
    owner = signed.SignedRequestReader(1024)
    original = copy.deepcopy(events[1]['plan'])
    for row in events:
        owner.consume(row)
        if row['event'] == 'scheduled': row['plan'] = plan(sequence=99)
        if row['event'] == 'signed_request_begin': row['plan']['logical_id'] = '3' * 64
    result = owner.finish()
    assert result[0].plan == signed.RequestPlan(**original)


# The frozen predecessor's168 cases/assertions above remain byte-for-byte intact.
def producer_fidelity_fixture(root, case):
    """Actual capture owner plus a fully consistent journal for one local shape."""
    from signed_request_fixture import add_retention
    fixture = Fixture(root)
    original = next(r for r in fixture.events if r['event'] == 'request_final')
    shapes = {
        'measurement_only': [('measurement', 1)],
        'account63': [('measurement', 1)],
        'account64': [('measurement', 1)],
        'sequence_starts2': [('measurement', 2)],
        'two_measurement': [('measurement', 1), ('measurement', 2)],
        'sequence_gap': [('measurement', 1), ('measurement', 3)],
        'warmup_measurement': [('warmup', 1), ('measurement', 1)],
        'warmup_after_measurement': [('measurement', 1), ('warmup', 1)],
        'two_warmup_then_measurement': [('warmup', 1), ('warmup', 2), ('measurement', 1)],
        'warmup_reenters': [('warmup', 1), ('measurement', 1), ('warmup', 2)],
    }
    final = []
    for index, (cohort, sequence) in enumerate(shapes[case]):
        row = copy.deepcopy(original)
        row['plan'] = plan(cohort, sequence)
        offset = -10 + sequence - 1 if cohort == 'warmup' else sequence - 1
        row['plan']['scheduled_offset_ns'] = offset
        if case in ('account63', 'account64'): row['plan']['account_index'] = int(case[-2:])
        row['hash'] = f'{2 * index + 1:064x}'
        row.update(offer_offset_ns=offset, acknowledgment_offset_ns=offset + 1,
                   applied_offset_ns=-1 if cohort == 'warmup' else fixture.geometry.final)
        final.append(row)
    events = [r for r in fixture.events if r['event'] not in signed.EVENTS |
              {'scheduled', 'prepared', 'offer', 'request_final', 'collection_finished'}]
    events[0]['scheduled_requests'] = len(final)
    events[0]['accounts'] = [{'authority': f'synthetic-account-{i}'} for i in range(64)]
    fixture.events = add_retention(events + final + [dict(event='collection_finished', passed=True, failure=None)])
    fixture.save()
    return fixture


@pytest.mark.parametrize('case', ['measurement_only', 'account63', 'two_measurement',
                                 'warmup_measurement', 'two_warmup_then_measurement'])
def test_actual_resource_replay_accepts_producer_local_boundaries_and_cohorts(tmp_path, case):
    fixture = producer_fidelity_fixture(tmp_path, case)
    result = fixture.run()
    planned = [r for r in fixture.events if r['event'] == 'scheduled']
    assert len(result.signed_requests) == len(planned)
    assert tuple(r.index for r in result.signed_requests) == tuple(range(len(planned)))
    assert all(r.plan == signed.RequestPlan(**p['plan']) for r, p in zip(result.signed_requests, planned, strict=True))
    if case == 'account63': assert result.signed_requests[0].plan.account_index == 63


@pytest.mark.parametrize('case,reason', [('account64', 'signed_integer_invalid'),
    ('sequence_starts2', 'signed_schedule_cohort_order'), ('sequence_gap', 'signed_schedule_cohort_order'),
    ('warmup_after_measurement', 'signed_schedule_cohort_order'), ('warmup_reenters', 'signed_schedule_cohort_order')])
def test_actual_rehashed_resource_replay_rejects_writer_impossible_plan(case, reason, tmp_path):
    # A same-component valid journal succeeds before the consistently rewritten
    # malformed one; every capture and source digest is the actual owner output.
    good = tmp_path / 'valid'; good.mkdir()
    bad = tmp_path / 'invalid'; bad.mkdir()
    valid = producer_fidelity_fixture(good, 'two_warmup_then_measurement')
    assert len(valid.run().signed_requests) == 3
    fixture = producer_fidelity_fixture(bad, case)
    assert hashlib.sha256(fixture.journal.read_bytes()).hexdigest() == fixture.digest
    with pytest.raises(replay.ReplayError, match=reason): fixture.run()
