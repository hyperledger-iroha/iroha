"""Independent dual-scope collector rows; no native or state qualification."""
import copy
from dataclasses import FrozenInstanceError
from itertools import permutations
from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/nexus'))
import applied_request_journal as applied


def rows(cohort='measurement', order=('local_status', 'status', 'accepted')):
    offer = -10 if cohort == 'warmup' else 0
    plan = dict(cohort=cohort, sequence=1, logical_id='b' * 64,
                scheduled_offset_ns=offer, account_index=0)
    tx_hash = 'c' * 63 + '1'
    events = [dict(event='plan', local_applied_required=True, poll_interval_ns=1, scheduled_requests=1),
              dict(event='scheduled', index=0, plan=copy.deepcopy(plan)),
              dict(event='clock_started'), dict(event='offer', index=0, hash=tx_hash, offset_ns=offer)]
    offsets = {}
    for position, name in enumerate(order, 1):
        offsets[name] = offer + position
        if name == 'accepted':
            event = dict(event=name, index=0, hash=tx_hash, offset_ns=offer + position)
        else:
            scope = 'local' if name == 'local_status' else 'global'
            event = dict(event=name, index=0, expected_hash=tx_hash, offset_ns=offer + position,
                         hash_matches=True, resolved_from='state', status='Applied', block_height=7,
                         **{f'{scope}_scope_matches': True})
        events.append(event)
    events.extend([dict(event='resource_collection_finished'),
                   dict(event='request_final', plan=plan, hash=tx_hash, offer_offset_ns=offer,
                        acknowledgment_offset_ns=offsets['accepted'], applied_offset_ns=offsets['status'],
                        block_height=7, status_attempts=1, local_applied_offset_ns=offsets['local_status'],
                        local_block_height=7, local_status_attempts=1, submission_finished=True, failure=None),
                   dict(event='collection_finished', passed=True, failure=None)])
    return events


def selected(events, event):
    return next(row for row in events if row['event'] == event)


def run(events):
    reader = applied.AppliedRequestReader(100)
    for row in events:
        assert reader.consume(row) is None
    assert reader.finish() == tuple(
        applied.RetainedApplication(index, row['hash'], row['offer_offset_ns'],
            row['acknowledgment_offset_ns'], row['applied_offset_ns'], row['block_height'],
            row['status_attempts'], row['local_applied_offset_ns'], row['local_block_height'],
            row['local_status_attempts'])
        for index, row in enumerate(row for row in events if row['event'] == 'request_final')
    )
    return reader


def test_finished_projection_uses_original_schedule_order_despite_reverse_completion():
    events = two_requests()
    # Complete the second request first, while physical scheduled order remains
    # the authoritative join with the independently retained signed requests.
    events[4:12] = events[8:12] + events[4:8]
    reader = applied.AppliedRequestReader(100)
    for row in events:
        assert reader.consume(row) is None
    result = reader.finish()
    assert type(result) is tuple
    assert result == (
        applied.RetainedApplication(0, 'c' * 63 + '1', 0, 3, 2, 7, 1, 1, 7, 1),
        applied.RetainedApplication(1, 'e' * 63 + '1', 0, 3, 2, 7, 1, 1, 7, 1),
    )
    with pytest.raises(FrozenInstanceError):
        result[0].block_height = 8
    with pytest.raises(TypeError):
        result[0] = result[1]
    assert not hasattr(result[0], '__dict__')
    # Neither mutable source rows nor the reader's no-longer-active counters
    # remain aliases of the successfully returned immutable observation.
    for row in events:
        if 'block_height' in row:
            row['block_height'] = 999
    reader._requests[0].global_scope.applied = (99, 999)
    assert result[0].block_height == 7 and result[0].applied_offset_ns == 2


def test_successful_finish_is_single_use_and_later_failure_does_not_mutate_result():
    reader = applied.AppliedRequestReader(100)
    for row in rows():
        reader.consume(row)
    original = reader.finish()
    with pytest.raises(applied.AppliedJournalError, match='applied_reader_incomplete'):
        reader.finish()
    with pytest.raises(applied.AppliedJournalError, match='applied_reader_unavailable'):
        reader.consume(dict(event='collection_finished', passed=True, failure=None))
    assert original == (applied.RetainedApplication(0, 'c' * 63 + '1', 0, 3, 2, 7, 1, 1, 7, 1),)


@pytest.mark.parametrize('boundary', ['empty', 'one_complete_request', 'all_final_rows'])
def test_early_finish_never_exposes_a_prefix_and_cannot_resume(boundary):
    events = two_requests()
    count = 0 if boundary == 'empty' else (
        events.index(selected(events, 'request_final')) + 1
        if boundary == 'one_complete_request' else len(events) - 1)
    reader = applied.AppliedRequestReader(100)
    for row in events[:count]:
        assert reader.consume(row) is None
    with pytest.raises(applied.AppliedJournalError, match='applied_reader_incomplete'):
        reader.finish()
    with pytest.raises(applied.AppliedJournalError, match='applied_reader_unavailable'):
        reader.consume(events[count])
    with pytest.raises(applied.AppliedJournalError, match='applied_reader_incomplete'):
        reader.finish()


def reject(events, reason='applied_'):
    reader = applied.AppliedRequestReader(100)
    with pytest.raises(applied.AppliedJournalError, match=reason):
        for row in events:
            reader.consume(row)
        reader.finish()
    with pytest.raises(applied.AppliedJournalError):
        reader.finish()
    with pytest.raises(applied.AppliedJournalError):
        reader.consume(dict(event='plan', local_applied_required=True, poll_interval_ns=1, scheduled_requests=1))


@pytest.mark.parametrize('cohort', ['warmup', 'measurement'])
@pytest.mark.parametrize('order', list(permutations(('local_status', 'status', 'accepted'))))
def test_local_global_and_acknowledgment_complete_in_every_order(cohort, order):
    events = rows(cohort, order)
    original = copy.deepcopy(events)
    reader = run(events)
    assert events == original
    assert reader._requests[0].global_scope.attempts == reader._requests[0].local_scope.attempts == 1


@pytest.mark.parametrize('local', [False, True])
def test_pending_missing_cache_queue_and_committed_rows_count_only_their_scope(local):
    events = rows()
    name = 'local_status' if local else 'status'
    terminal = selected(events, name)
    at = events.index(terminal)
    pending = []
    for source, status in [('cache', 'Queued'), ('queue', 'Approved'), ('state', 'Committed'),
                           ('cache', 'Applied'), ('queue', 'Rejected'), ('cache', 'Expired')]:
        row = copy.deepcopy(terminal)
        row.update(offset_ns=len(pending), resolved_from=source, status=status, block_height=None)
        pending.append(row)
    pending.append(dict(event=name + '_missing', index=0, offset_ns=len(pending), hash=terminal['expected_hash']))
    terminal['offset_ns'] = len(pending)
    selected(events, 'request_final')['local_applied_offset_ns' if local else 'applied_offset_ns'] = len(pending)
    events[at:at] = pending
    field = 'local_status_attempts' if local else 'status_attempts'
    selected(events, 'request_final')[field] = len(pending) + 1
    reader = run(events)
    scopes = reader._requests[0].local_scope, reader._requests[0].global_scope
    assert tuple(scope.attempts for scope in scopes) == ((8, 1) if local else (1, 8))


@pytest.mark.parametrize('local', [False, True])
@pytest.mark.parametrize('field,value', [
    ('expected_hash', 'd' * 63 + '1'), ('expected_hash', 'c' * 64), ('expected_hash', 'C' * 63 + '1'),
    ('hash_matches', False), ('hash_matches', 1), ('scope', False), ('scope', 1),
    ('index', True), ('index', 1), ('index', -1), ('index', None),
    ('offset_ns', True), ('offset_ns', -1), ('offset_ns', 0), ('offset_ns', 101), ('offset_ns', 1 << 63),
    ('resolved_from', 'unknown'), ('resolved_from', ''), ('resolved_from', None),
    ('status', 'unknown'), ('status', 'StateApplied'), ('status', 'Rejected'), ('status', 'Expired'),
    ('block_height', None), ('block_height', 0), ('block_height', -1), ('block_height', True),
    ('block_height', 7.0), ('block_height', '7'), ('block_height', 1 << 64), ('block_height', 8),
])
def test_each_scope_rejects_wrong_identity_type_terminal_source_height_and_window(local, field, value):
    events = rows()
    name = 'local_status' if local else 'status'
    if field == 'scope': field = 'local_scope_matches' if local else 'global_scope_matches'
    selected(events, name)[field] = value
    reject(events)


@pytest.mark.parametrize('value', [None, False, 1, 'true'])
def test_local_requirement_is_unconditionally_true(value):
    events = rows()
    events[0]['local_applied_required'] = value
    reject(events, 'applied_local_requirement_missing')


def test_predecessor_global_only_journal_fails_without_compatibility_inference():
    events = [row for row in rows() if row['event'] != 'local_status']
    del events[0]['local_applied_required']
    final = selected(events, 'request_final')
    for name in ('local_applied_offset_ns', 'local_block_height', 'local_status_attempts'):
        del final[name]
    reject(events)


@pytest.mark.parametrize('event', ['offer', 'accepted', 'status', 'local_status', 'request_final', 'collection_finished'])
@pytest.mark.parametrize('mutation', ['missing', 'duplicate', 'extra_field'])
def test_every_required_event_has_exact_presence_and_schema(event, mutation):
    events = rows()
    row = selected(events, event)
    if mutation == 'missing': events.remove(row)
    elif mutation == 'duplicate': events.insert(events.index(row), copy.deepcopy(row))
    else: row['compatibility'] = True
    reject(events)


@pytest.mark.parametrize('event', ['status', 'local_status'])
@pytest.mark.parametrize('source', ['cache', 'queue'])
def test_nonstate_applied_cannot_fill_a_terminal_slot(event, source):
    events = rows()
    selected(events, event)['resolved_from'] = source
    reject(events, 'applied_local_or_global_missing')


@pytest.mark.parametrize('local', [False, True])
@pytest.mark.parametrize('mutation', ['wrong_hash', 'wrong_index', 'missing_field', 'extra_field', 'after_terminal', 'no_terminal', 'counter_drift'])
def test_missing_observations_are_exact_pending_attempts(local, mutation):
    events = rows()
    name = 'local_status' if local else 'status'
    terminal = selected(events, name)
    missing = dict(event=name + '_missing', index=0, hash=terminal['expected_hash'], offset_ns=0)
    field = 'local_status_attempts' if local else 'status_attempts'
    selected(events, 'request_final')[field] = 2
    at = events.index(terminal)
    events.insert(at + (mutation == 'after_terminal'), missing)
    if mutation == 'wrong_hash': missing['hash'] = 'd' * 63 + '1'
    elif mutation == 'wrong_index': missing['index'] = 1
    elif mutation == 'missing_field': del missing['hash']
    elif mutation == 'extra_field': missing['status'] = 'Applied'
    elif mutation == 'no_terminal': events.remove(terminal)
    elif mutation == 'counter_drift': selected(events, 'request_final')[field] = 1
    reject(events)


@pytest.mark.parametrize('field', ['offer_offset_ns', 'acknowledgment_offset_ns', 'applied_offset_ns',
    'block_height', 'status_attempts', 'local_applied_offset_ns', 'local_block_height', 'local_status_attempts'])
@pytest.mark.parametrize('mutation', ['changed', 'missing', 'bool', 'float', 'null'])
def test_summary_cannot_invent_terminals_offsets_heights_or_attempt_counts(field, mutation):
    events = rows()
    final = selected(events, 'request_final')
    if mutation == 'changed': final[field] += 1
    elif mutation == 'missing': del final[field]
    elif mutation == 'bool': final[field] = bool(final[field])
    elif mutation == 'float': final[field] = float(final[field])
    else: final[field] = None
    reject(events)


@pytest.mark.parametrize('cohort,offset,valid', [('measurement', 100, True), ('measurement', 101, False),
                                               ('warmup', -1, True), ('warmup', 0, False)])
@pytest.mark.parametrize('event,field', [('local_status', 'local_applied_offset_ns'),
    ('status', 'applied_offset_ns'), ('accepted', 'acknowledgment_offset_ns')])
def test_terminal_deadline_is_unchanged_by_later_resource_finish(cohort, offset, valid, event, field):
    events = rows(cohort)
    selected(events, event)['offset_ns'] = offset
    selected(events, 'request_final')[field] = offset
    if valid: run(events)
    else: reject(events, 'applied_transaction_drain_extended')


@pytest.mark.parametrize('event', ['status', 'local_status', 'accepted'])
def test_observations_must_follow_offer_and_precede_postconditions(event):
    for boundary in ('offer', 'workload_postconditions_started'):
        events = rows()
        row = selected(events, event)
        events.remove(row)
        if boundary == 'workload_postconditions_started':
            at = events.index(selected(events, 'resource_collection_finished')) + 1
            events.insert(at, dict(event=boundary))
        at = events.index(selected(events, boundary))
        events.insert(at + (boundary != 'offer'), row)
        reject(events)


def test_global_and_local_may_share_a_positive_u64_height_above_signed_range():
    events = rows()
    for name in ('status', 'local_status'):
        selected(events, name)['block_height'] = (1 << 64) - 1
    selected(events, 'request_final').update(block_height=(1 << 64) - 1, local_block_height=(1 << 64) - 1)
    run(events)


def test_unknown_local_event_cannot_be_silently_ignored():
    events = rows()
    events.insert(4, dict(event='local_status_cached_success'))
    reject(events, 'applied_event_unknown')


def test_schedule_is_copied_before_callers_mutate_their_dictionary():
    events = rows()
    reader = applied.AppliedRequestReader(100)
    for row in events:
        reader.consume(row)
        if row['event'] == 'scheduled': row['plan']['sequence'] = 2
    reader.finish()


@pytest.mark.parametrize('value', [True, None, 0, -1, 1.0, 1 << 63])
def test_deadline_is_a_positive_bounded_exact_integer(value):
    with pytest.raises(applied.AppliedJournalError): applied.AppliedRequestReader(value)


@pytest.mark.parametrize('value', [True, None, 0, -1, 1.0, 1_000_001])
def test_retained_request_state_is_bounded_by_exact_count(value):
    events = rows()
    events[0]['scheduled_requests'] = value
    reject(events)


@pytest.mark.parametrize('value', [True, None, 0, -1, 1.0, 1 << 63])
def test_retry_delay_is_a_positive_bounded_exact_integer(value):
    events = rows()
    events[0]['poll_interval_ns'] = value
    reject(events)


@pytest.mark.parametrize('local', [False, True])
@pytest.mark.parametrize('delay,valid', [(0, False), (4, False), (5, True), (6, True)])
def test_each_scope_retry_completion_respects_previous_completion_plus_poll_interval(local, delay, valid):
    events = rows()
    events[0]['poll_interval_ns'] = 5
    terminal = selected(events, 'local_status' if local else 'status')
    pending = copy.deepcopy(terminal)
    pending.update(offset_ns=0, resolved_from='cache')
    events.insert(events.index(terminal), pending)
    terminal['offset_ns'] = delay
    final = selected(events, 'request_final')
    final['local_applied_offset_ns' if local else 'applied_offset_ns'] = delay
    final['local_status_attempts' if local else 'status_attempts'] = 2
    if valid: run(events)
    else: reject(events)


def two_requests():
    """Distinct retained hash owners whose observation timing may overlap."""
    first, second = rows(), rows()
    second_plan = selected(second, 'scheduled')['plan']
    second_plan.update(sequence=2, logical_id='d' * 64)
    selected(second, 'request_final')['plan'] = copy.deepcopy(second_plan)
    for row in second:
        if 'index' in row: row['index'] = 1
        if 'hash' in row: row['hash'] = 'e' * 63 + '1'
        if 'expected_hash' in row: row['expected_hash'] = 'e' * 63 + '1'
    first[0]['scheduled_requests'] = 2
    return [first[0], first[1], second[1], first[2],
            *first[3:7], *second[3:7], first[7], first[8], second[8], first[9]]


def test_two_distinct_hashes_keep_independent_slot_owners():
    reader = run(two_requests())
    assert set(reader._requests) == {0, 1}
    assert reader._requests[0].hash != reader._requests[1].hash


@pytest.mark.parametrize('kind', ['local_index', 'local_hash', 'global_index', 'duplicate_offer_hash',
                                 'wrong_final_hash', 'duplicate_final_plan'])
def test_another_request_cannot_supply_the_original_hash_local_slot(kind):
    events = two_requests()
    local = selected(events, 'local_status')
    if kind == 'local_index': local['index'] = 1
    elif kind == 'local_hash': local['expected_hash'] = 'e' * 63 + '1'
    elif kind == 'global_index': selected(events, 'status')['index'] = 1
    elif kind == 'duplicate_offer_hash':
        [row for row in events if row['event'] == 'offer'][1]['hash'] = selected(events, 'offer')['hash']
    elif kind == 'wrong_final_hash': selected(events, 'request_final')['hash'] = 'e' * 63 + '1'
    else:
        finals = [row for row in events if row['event'] == 'request_final']
        finals[1]['plan'] = copy.deepcopy(finals[0]['plan'])
    reject(events)


@pytest.mark.parametrize('event,field', [('local_status', 'local_applied_offset_ns'),
    ('status', 'applied_offset_ns'), ('accepted', 'acknowledgment_offset_ns')])
@pytest.mark.parametrize('offset,valid', [(100, True), (101, False)])
def test_independent_resource_finish_does_not_replace_the_exact_completion_deadline(event, field, offset, valid):
    events = rows()
    completion = selected(events, event)
    completion['offset_ns'] = offset
    selected(events, 'request_final')[field] = offset
    events.remove(completion)
    at = events.index(selected(events, 'resource_collection_finished')) + 1
    events.insert(at, completion)
    if valid: run(events)
    else: reject(events, 'applied_transaction_drain_extended')


def test_any_response_after_first_final_is_closed_even_for_another_request():
    events = two_requests()
    last_local = [row for row in events if row['event'] == 'local_status'][1]
    events.remove(last_local)
    events.insert(events.index(selected(events, 'request_final')) + 1, last_local)
    reject(events, 'applied_observation_order')
