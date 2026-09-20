"""Actual private resource/journal replay with synthetic, unverified signed bytes."""
from dataclasses import fields, replace
from fractions import Fraction
import hashlib
import math
import os
from types import SimpleNamespace

import pytest

from resource_replay_test import Fixture
from scaling_readiness_fixture import seal_anchor, write_inputs
from signed_request_fixture import add_retention
from resource_evidence_budget import select_run_budget
from scaling_native_facts_inputs import FactsJournalPlan
from scaling_proof_sequence import StoppedReader
from scaling_readiness_inputs import ReadinessInputs
from scaling_canonical_proof import _plan_snapshot
import scaling_replayed_workload as adapter


def make_case(tmp_path, *, lanes=4, pool=4, zero_warmup=False, fractional=False):
    directory = (tmp_path / 'inputs').resolve()
    anchor = write_inputs(directory)
    anchor['lane_count'] = lanes
    for index in range(4, pool):
        name = f'workload-account-{index:02}.toml'
        raw = b'chain = "test-chain"\n'
        path = directory / name; path.write_bytes(raw); path.chmod(0o600)
        anchor['accounts'].append(dict(index=index, account_id=f'test-account-{index}', config=name))
        anchor['artifacts'].append(dict(path=name, sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw)))
    inputs = ReadinessInputs(directory, seal_anchor(directory, anchor), lanes)
    root = tmp_path / 'resource'; root.mkdir(mode=0o700)
    fixture = Fixture(root)
    if zero_warmup: fixture.geometry = replace(fixture.geometry, warmup_ns=0)
    fixture.allocation = select_run_budget(fixture.allocation.experiment, 1, 'one_lane' if lanes == 1 else 'four_lane')
    geometry = fixture.geometry
    numerator, denominator = (75, 2) if fractional else (pool * 10, 1)
    plan = FactsJournalPlan('a' * 64, 1, numerator, denominator,
        geometry.warmup_ns, geometry.measurement_ns, geometry.drain_ns, 0,
        2, 2, geometry.preparation_ahead_ns, 4, 32, 4, 1_000_000, 128,
        geometry.interval_ns, geometry.response_deadline_ns, geometry.max_start_lag_ns)
    rate = Fraction(numerator, denominator * 1_000_000_000)
    counts = (math.ceil(rate * geometry.warmup_ns), math.ceil(rate * geometry.measurement_ns))
    account_offset = int.from_bytes(hashlib.sha256(b'gscale-account-offset-v1:' + plan.workload_seed.encode()).digest()[:8], 'little') % pool
    rows = [row for row in fixture.events if row['event'] in (
        'plan', 'resource_preflight', 'clock_started', 'resource_request',
        'resource_observation', 'resource_collection_finished')]
    rows[0].update(seed=plan.workload_seed, accounts=[row.account_id for row in inputs.generation.accounts],
        pair_index=1, variant='one_lane' if lanes == 1 else 'four_lane',
        scheduled_requests=sum(counts), local_applied_required=True,
        poll_interval_ns=plan.poll_interval_ns, max_status_requests=plan.max_status_requests,
        **{name:getattr(geometry, name) for name in ('warmup_ns', 'measurement_ns', 'drain_ns', 'preparation_ahead_ns')})
    next(row for row in rows if row['event'] == 'clock_started')['initial_offset_ns'] = -(
        geometry.warmup_ns + geometry.drain_ns + geometry.preparation_ahead_ns)
    index = 0
    for cohort, count, start in (('warmup', counts[0], -(geometry.warmup_ns + geometry.drain_ns)),
                                  ('measurement', counts[1], 0)):
        for ordinal in range(count):
            offset = start + math.floor(Fraction(ordinal, 1) / rate)
            logical = hashlib.sha256(f'{plan.workload_seed}:{cohort}:{ordinal + 1}'.encode()).hexdigest()
            tx_hash = hashlib.sha256(f'opaque fixture {index}'.encode()).hexdigest()[:-1] + '1'
            final = geometry.final if cohort == 'measurement' and ordinal + 1 == count else offset + 2
            rows.append(dict(event='request_final', plan=dict(cohort=cohort, sequence=ordinal+1,
                logical_id=logical, scheduled_offset_ns=offset, account_index=(ordinal+account_offset)%pool),
                hash=tx_hash, offer_offset_ns=offset, acknowledgment_offset_ns=offset+3,
                applied_offset_ns=final, block_height=2+index, status_attempts=1,
                local_applied_offset_ns=offset+1, local_block_height=2+index, local_status_attempts=1,
                submission_finished=True, failure=None))
            index += 1
    rows.append(dict(event='collection_finished', passed=True, failure=None))
    fixture.events = add_retention(rows)
    result = fixture.run()
    peer = inputs.roles[3]
    stopped = StoppedReader(peer.primary_block_store, peer.primary_merge_log, 1, 100,
        1000, 8*1024*1024, 65536, 1024*1024, 1000, 1024*1024, 2*1024*1024, os.geteuid())
    return SimpleNamespace(inputs=inputs, fixture=fixture, result=result, plan=plan, stopped=stopped, counts=counts)


@pytest.fixture
def case(tmp_path):
    value = make_case(tmp_path)
    original = value.inputs
    yield value
    original.close()


def build(case):
    return adapter.build_replay_plan(case.result, case.inputs, case.plan, case.stopped)


def application(case, position=0, **changes):
    rows = list(case.result.applied_requests)
    rows[position] = replace(rows[position], **changes)
    case.result = replace(case.result, applied_requests=tuple(rows))


def signed(case, position=0, **changes):
    rows = list(case.result.signed_requests)
    rows[position] = replace(rows[position], **changes)
    case.result = replace(case.result, signed_requests=tuple(rows))


@pytest.mark.parametrize('lanes,pool,zero,fractional', [(1,4,False,False), (4,4,False,False),
    (4,8,False,False), (4,4,True,False), (4,4,False,True)])
def test_real_resource_replay_builds_complete_native_plan(tmp_path, lanes, pool, zero, fractional):
    case = make_case(tmp_path, lanes=lanes, pool=pool, zero_warmup=zero, fractional=fractional)
    try:
        plan = build(case)
        assert _plan_snapshot(plan)
        assert plan.lane_count == lanes
        assert plan.accounts == tuple(row.account_id for row in case.inputs.generation.accounts)
        assert (plan.warmup_requests, plan.measurement_requests) == case.counts
        assert plan.last_height == 100 and len(plan.observations) == sum(case.counts)
        assert tuple(row.transaction_hash for row in plan.observations) == tuple(row.hash for row in case.result.signed_requests)
        assert tuple(row.global_height for row in plan.observations) == tuple(range(2, 2+sum(case.counts)))
        assert all(row.local_height == row.global_height for row in plan.observations)
        assert case.result.applied_requests[-1].applied_offset_ns == case.fixture.geometry.final
        assert case.result.applied_requests[0].local_applied_offset_ns < case.result.applied_requests[0].acknowledgment_offset_ns
        if fractional:
            assert case.result.signed_requests[case.counts[0]+1].plan.scheduled_offset_ns == 26_666_666
    finally: case.inputs.close()


@pytest.mark.parametrize('field,bad', [('cohort','measurement'), ('sequence',True), ('sequence',2),
    ('logical_id','b'*64), ('scheduled_offset_ns',-109_999_999), ('account_index',True), ('account_index',63)])
def test_original_native_schedule_cannot_be_replaced(case, field, bad):
    original = case.result.signed_requests[0]
    signed(case, plan=replace(original.plan, **{field:bad}))
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('field,bad', [('index',True), ('index',1), ('hash','0'*64),
    ('canonical_sha256','secret'), ('canonical_bytes',b''), ('canonical_bytes',bytearray(b'x')),
    ('canonical_bytes',b'x'*(1024*1024+1))])
def test_retained_signed_owner_type_identity_and_bytes_are_bounded(case, field, bad):
    signed(case, **{field:bad})
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('field,bad', [('index',True), ('index',1), ('hash','f'*64),
    ('offer_offset_ns',True), ('offer_offset_ns',-110_000_001), ('offer_offset_ns',-109_999_999),
    ('acknowledgment_offset_ns',-110_000_001), ('acknowledgment_offset_ns',0),
    ('applied_offset_ns',-110_000_000), ('applied_offset_ns',0),
    ('local_applied_offset_ns',-110_000_000), ('local_applied_offset_ns',0),
    ('block_height',True), ('block_height',0), ('block_height',101), ('local_block_height',3),
    ('status_attempts',0), ('status_attempts',True), ('local_status_attempts',0), ('local_status_attempts',True)])
def test_both_applied_observations_must_join_the_original_offer_and_tip(case, field, bad):
    application(case, **{field:bad})
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('field', ['acknowledgment_offset_ns','applied_offset_ns','local_applied_offset_ns'])
def test_measurement_deadline_cannot_be_extended(case, field):
    application(case, len(case.result.applied_requests)-1, **{field:case.fixture.geometry.final+1})
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('field', ['signed_requests','applied_requests'])
@pytest.mark.parametrize('change', ['missing','extra','reordered','list','wrong_type'])
def test_complete_ordered_tuple_owners_are_mandatory(case, field, change):
    rows = getattr(case.result, field)
    changed = {'missing':rows[:-1], 'extra':rows+(rows[-1],), 'reordered':rows[1:]+rows[:1],
               'list':list(rows), 'wrong_type':(None,)+rows[1:]}[change]
    case.result = replace(case.result, **{field:changed})
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


def test_duplicate_signed_hashes_are_rejected_even_when_both_scopes_agree(case):
    first = case.result.signed_requests[0].hash
    signed(case, 1, hash=first); application(case, 1, hash=first)
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('field', [field.name for field in fields(adapter.ReplayGeometry)])
def test_retained_original_resource_geometry_must_equal_original_plan(case, field):
    geometry = case.result.geometry
    case.result = replace(case.result, geometry=replace(geometry, **{field:getattr(geometry,field)+1}))
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('pair,variant', [(2,'four_lane'),(1,'one_lane')])
def test_original_resource_allocation_cannot_select_another_trial(case, pair, variant):
    case.result = replace(case.result, allocation=select_run_budget(case.result.allocation.experiment,pair,variant))
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('field', ['admitted_resource_byte_limit','admitted_journal_byte_limit',
    'admitted_resource_file_count','admitted_experiment_resource_bytes','admitted_experiment_resource_file_count',
    'admitted_experiment_total_bytes','global_byte_limit','control_file_limit','capture_file_count'])
def test_retained_replay_reservations_cannot_be_relabelled(case, field):
    case.result = replace(case.result, **{field:getattr(case.result,field)+1})
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('kind', ['schedule','late','sequence','missing','finish','bytes'])
def test_resource_sample_coverage_and_bounds_remain_bound(case, kind):
    rows = list(case.result.samples)
    if kind == 'schedule': rows[1] = replace(rows[1],scheduled_offset_ns=rows[1].scheduled_offset_ns+1)
    elif kind == 'late': rows[1] = replace(rows[1],end_offset_ns=rows[1].start_offset_ns+case.plan.resource_response_deadline_ns)
    elif kind == 'sequence': rows[1] = replace(rows[1],capture=replace(rows[1].capture,sequence=99))
    elif kind == 'missing': rows.pop()
    elif kind == 'finish': case.result = replace(case.result,finish_start_ns=0)
    elif kind == 'bytes': case.result = replace(case.result,capture_bytes=case.result.capture_bytes+1)
    case.result = replace(case.result,samples=tuple(rows))
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('field,bad', [('first_height',2),('last_height',1),('owner_uid',4294967295),
    ('block_store',None),('merge_log',None)])
def test_only_the_original_stopped_primary_interval_is_accepted(case, field, bad):
    case.stopped = replace(case.stopped, **{field:bad})
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


def test_original_input_custody_is_required(case):
    case.inputs.roles[0].node_config.write_bytes(b'changed original private input')
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


def test_returned_plan_owns_values_after_caller_records_are_mutated(case):
    result = build(case); snapshot = _plan_snapshot(result)
    object.__setattr__(case.result.applied_requests[0], 'block_height', 99)
    object.__setattr__(case.result.signed_requests[0], 'hash', 'b'*64)
    object.__setattr__(case.plan, 'workload_seed', 'b'*64)
    object.__setattr__(case.stopped, 'last_height', 999)
    assert _plan_snapshot(result) == snapshot


@pytest.mark.parametrize('owner', ['result','inputs','plan','stopped'])
def test_untyped_documents_do_not_replace_original_owners(case, owner):
    setattr(case,owner,{})
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


@pytest.mark.parametrize('field,bad', [('workload_seed','b'*64), ('pair_index',2), ('rate_numerator',80),
    ('rate_denominator',True), ('journal_max_requests',1)])
def test_original_workload_selection_cannot_be_replaced(case, field, bad):
    case.plan = replace(case.plan, **{field:bad})
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)


def test_cumulative_signed_bytes_stay_within_original_journal_capacity(case):
    signed(case,canonical_bytes=b'x'*(case.result.journal_bytes//2+1))
    with pytest.raises(adapter.ReplayedWorkloadError): build(case)
