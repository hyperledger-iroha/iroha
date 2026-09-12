"""Exact collector/trace agreement; no canonical or resource replay substitution."""
import copy
import hashlib
import json

import pytest

from scaling_main_control_reader_test import main
from resource_replay import ReplayGeometry

GEOMETRY = ReplayGeometry(5_000_000_000, 20_000_000_000, 2_000_000_000,
                          1_000_000_000, 1_000_000_000, 100_000_000, 10_000_000)
SEED = 'a' * 64


def fixture():
    rows = []
    for cohort in ('warmup', 'measurement'):
        for index in range(4):
            sequence = index + 1
            offset = (index - 7 if cohort == 'warmup' else index) * 1_000_000_000
            txhash = f'{len(rows) + 1:063x}1'
            rows.append(dict(cohort=cohort, sequence=sequence,
                logical_id=hashlib.sha256(f'{SEED}:{cohort}:{sequence}'.encode()).hexdigest(),
                hash=txhash, scheduled_offset_ns=offset, offer_offset_ns=offset + 1,
                submission_lag_ns=1,
                acknowledgment={'offset_ns': offset + 10, 'hash': txhash, 'status': 'Accepted', 'rejection': None},
                applied={'offset_ns': offset + 20, 'hash': txhash, 'scope': 'global',
                         'resolved_from': 'state', 'status': 'Applied', 'block_height': len(rows) + 1}))
    authorities = tuple(f'synthetic-authority-{index}' for index in range(4))
    selector = int.from_bytes(hashlib.sha256(f'gscale-account-offset-v1:{SEED}'.encode()).digest()[:8], 'little') % 4
    def plan(row):
        return {name: row[name] for name in ('cohort', 'sequence', 'logical_id', 'scheduled_offset_ns')} | {
            'account_index': (row['sequence'] - 1 + selector) % 4}
    header = dict(event='plan', pair_index=1, variant='one_lane', seed=SEED, submission_lag_bound_ns=1,
        scheduled_requests=len(rows), accounts=[{'authority': authority} for authority in authorities],
        workload='self_owned_account_metadata_insert_v1', max_effects_per_account=1024,
        account_selection='(zero_based_cohort_sequence + first8le(sha256(gscale-account-offset-v1:seed))) modulo pool_length',
        **{name: getattr(GEOMETRY, name) for name in ('warmup_ns', 'measurement_ns', 'drain_ns', 'preparation_ahead_ns')})
    events = [header]
    events.extend(dict(event='scheduled', index=index, plan=plan(row)) for index, row in enumerate(rows))
    for index, authority in enumerate(authorities):
        events.append(dict(event='workload_account_preflight', authority=authority, account_index=index,
            expected_effects=2, expected_account_sha256=f'{index + 1:064x}', expected_account_frame_bytes=128))
    events.extend(({'event': 'clock_started'}, {'event': 'resource_collection_finished'},
                   {'event': 'workload_postconditions_started'}))
    for index, authority in enumerate(authorities):
        events.append(dict(event='workload_account_postcondition', authority=authority, account_index=index,
            verified_effects=2, account_sha256=f'{index + 1:064x}', read_source='signed_find_account_by_id_after_complete_drain'))
    for row in rows:
        events.append(dict(event='request_final', plan=plan(row), hash=row['hash'],
            offer_offset_ns=row['offer_offset_ns'], acknowledgment_offset_ns=row['acknowledgment']['offset_ns'],
            applied_offset_ns=row['applied']['offset_ns'], block_height=row['applied']['block_height'],
            status_attempts=1, submission_finished=True, failure=None))
    events.append(dict(event='collection_finished', passed=True, failure=None))
    return rows, events, authorities


def encoded(events):
    return b''.join(json.dumps(row, separators=(',', ':'), sort_keys=True).encode() + b'\n' for row in events)


def reconcile(events, rows):
    return main._reconcile_journal_trace(encoded(events), rows, pair_index=1,
        variant='one_lane', seed=SEED, geometry=GEOMETRY, submission_lag_bound_ns=1)


def test_full_collector_rows_bind_trace_and_observed_postconditions_without_mutation():
    rows, events, authorities = fixture()
    before = copy.deepcopy((rows, events))
    assert reconcile(events, rows) == authorities
    assert (rows, events) == before
    # Resource preflight may precede bulk scheduling in the admitted writer successor.
    events.insert(1, {'event': 'resource_preflight'})
    assert reconcile(events, rows) == authorities


@pytest.mark.parametrize('index', range(29))
def test_omitting_any_required_plan_schedule_preflight_final_or_terminal_row_fails(index):
    rows, events, _ = fixture()
    del events[index]
    with pytest.raises(main.EvidenceError): reconcile(events, rows)


@pytest.mark.parametrize('event_name,field,value', [
    ('plan', 'seed', 'b' * 64), ('plan', 'pair_index', True), ('plan', 'variant', 'four_lane'),
    ('plan', 'workload', 'log_only'), ('plan', 'scheduled_requests', 9),
    ('plan', 'account_selection', 'first'), ('plan', 'max_effects_per_account', 1025),
    ('plan', 'warmup_ns', 1), ('plan', 'measurement_ns', 1), ('plan', 'drain_ns', 1),
    ('plan', 'preparation_ahead_ns', 1), ('scheduled', 'index', True),
    ('workload_account_preflight', 'authority', 'other'), ('workload_account_preflight', 'expected_effects', 3),
    ('workload_account_preflight', 'expected_account_frame_bytes', 262145),
    ('workload_account_preflight', 'expected_account_sha256', 'A' * 64),
    ('workload_account_postcondition', 'authority', 'other'), ('workload_account_postcondition', 'account_index', True),
    ('workload_account_postcondition', 'verified_effects', 3), ('workload_account_postcondition', 'account_sha256', 'c' * 64),
    ('workload_account_postcondition', 'read_source', 'cached'), ('request_final', 'hash', 'c' * 63 + '1'),
    ('request_final', 'offer_offset_ns', 7), ('request_final', 'acknowledgment_offset_ns', 8),
    ('request_final', 'applied_offset_ns', 9), ('request_final', 'block_height', True),
    ('request_final', 'status_attempts', -1), ('request_final', 'submission_finished', 1),
    ('request_final', 'failure', 'rejected'), ('collection_finished', 'passed', 1),
    ('collection_finished', 'failure', 'unavailable'),
])
def test_native_workload_and_each_final_value_must_match_exactly(event_name, field, value):
    rows, events, _ = fixture()
    next(row for row in events if row['event'] == event_name)[field] = value
    with pytest.raises(main.EvidenceError): reconcile(events, rows)


@pytest.mark.parametrize('event_name,field', [('scheduled', 'sequence'), ('scheduled', 'account_index'),
    ('request_final', 'sequence'), ('request_final', 'account_index')])
def test_nested_plan_booleans_cannot_equal_integer_sequence_or_account_identity(event_name, field):
    rows, events, _ = fixture()
    selected = next(row for row in events if row['event'] == event_name and row['plan'][field] in (0, 1))
    selected['plan'][field] = bool(selected['plan'][field])
    with pytest.raises(main.EvidenceError): reconcile(events, rows)


@pytest.mark.parametrize('event_name', ['scheduled', 'workload_account_preflight',
    'workload_account_postcondition', 'request_final'])
def test_reordering_or_duplicating_same_content_rows_cannot_preserve_success(event_name):
    rows, events, _ = fixture()
    indices = [index for index, row in enumerate(events) if row['event'] == event_name]
    for duplicate in (False, True):
        changed = copy.deepcopy(events)
        left, right = indices[:2]
        if duplicate: changed[right] = copy.deepcopy(changed[left])
        else: changed[left], changed[right] = changed[right], changed[left]
        with pytest.raises(main.EvidenceError): reconcile(changed, rows)


@pytest.mark.parametrize('kind', ['duplicate', 'not_multiple_four', 'too_many', 'extra_field', 'oversize', 'empty'])
def test_account_pool_is_exact_bounded_and_has_no_duplicates(kind):
    rows, events, _ = fixture()
    pool = events[0]['accounts']
    if kind == 'duplicate': pool[1] = copy.deepcopy(pool[0])
    elif kind == 'not_multiple_four': pool.pop()
    elif kind == 'too_many': pool[:] = [{'authority': f'account-{i}'} for i in range(68)]
    elif kind == 'extra_field': pool[0]['alias'] = 'ignored'
    elif kind == 'oversize': pool[0]['authority'] = 'x' * 2049
    else: pool.clear()
    with pytest.raises(main.EvidenceError): reconcile(events, rows)


@pytest.mark.parametrize('kind', ['newline', 'blank', 'oversize_event', 'duplicate_key', 'after_finish', 'no_rows'])
def test_journal_raw_framing_and_complete_record_bounds_are_mandatory(kind):
    rows, events, _ = fixture()
    raw = encoded(events)
    if kind == 'newline': raw = raw[:-1]
    elif kind == 'blank': raw += b'\n'
    elif kind == 'oversize_event': raw = b'{"event":"' + b'x' * 16384 + b'"}\n' + raw
    elif kind == 'duplicate_key': raw = raw.replace(b'"event":"plan"', b'"event":"plan","event":"plan"', 1)
    elif kind == 'after_finish': raw += b'{"event":"prepared"}\n'
    else: rows = []
    with pytest.raises(main.EvidenceError):
        main._reconcile_journal_trace(raw, rows, pair_index=1, variant='one_lane', seed=SEED, geometry=GEOMETRY, submission_lag_bound_ns=1)


def test_complete_native_cohort_cannot_be_replaced_by_a_rejected_trace_row():
    rows, events, _ = fixture()
    rows[0]['acknowledgment']['status'] = 'Rejected'
    rows[0]['applied'] = None
    with pytest.raises(main.EvidenceError): reconcile(events, rows)


@pytest.mark.parametrize('declared', [None, True, 1.0, -1, 0, 2, 1 << 63, '1'])
def test_journal_submission_lag_must_equal_independent_schedule_with_exact_integer_type(declared):
    rows, events, _ = fixture()
    events[0]['submission_lag_bound_ns'] = declared
    with pytest.raises(main.EvidenceError): reconcile(events, rows)


@pytest.mark.parametrize('lag', [-1, 2])
def test_journal_and_trace_cannot_agree_on_an_offer_outside_the_independent_bound(lag):
    rows, events, _ = fixture()
    row = rows[0]
    row['offer_offset_ns'] = row['scheduled_offset_ns'] + lag
    row['submission_lag_ns'] = lag
    final = next(event for event in events if event['event'] == 'request_final')
    final['offer_offset_ns'] = row['offer_offset_ns']
    with pytest.raises(main.EvidenceError, match='collector offer exceeds'):
        reconcile(events, rows)


@pytest.mark.parametrize('lag', [True, 1.0, 0, 2])
def test_trace_lag_field_must_equal_the_exact_offer_minus_schedule(lag):
    rows, events, _ = fixture()
    rows[0]['submission_lag_ns'] = lag
    with pytest.raises(main.EvidenceError): reconcile(events, rows)


@pytest.mark.parametrize('bound', [None, True, 1.0, -1, 1 << 63, '1'])
def test_submission_lag_input_itself_requires_a_bounded_exact_integer(bound):
    rows, events, _ = fixture()
    with pytest.raises(main.EvidenceError, match='independent submission lag bound'):
        main._reconcile_journal_trace(encoded(events), rows, pair_index=1, variant='one_lane',
            seed=SEED, geometry=GEOMETRY, submission_lag_bound_ns=bound)
