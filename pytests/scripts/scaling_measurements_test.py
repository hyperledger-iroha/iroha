"""Pure synthetic measurements; no signatures, native execution or authority."""
from dataclasses import fields,replace
from fractions import Fraction
import hashlib
from pathlib import Path
import sys

import pytest

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts/nexus'))
import scaling_measurements as measure
from resource_replay import Bracket,CaptureReduction,ReplayGeometry
from resource_experiment import ResourceMaxima
from resource_process import ProcessIdentity
from scaling_completed_authority import PublicRunProjection,PublicRequest,ResourceSnapshot,_PUBLIC_RECORDS
from scaling_experiment_plan import ExperimentPlan,ResourceLimits,RUN_KEYS,_RECORDS
from scaling_fixed_trial import TrialPlan
from scaling_generator import GeneratorPlan,GenerationReceipt
from scaling_native_load import NativeLoadPlan,NativeLoadReceipt
from scaling_readiness_inputs import GeneratedAccount

NS=1_000_000_000
LIMITS=ResourceLimits(100,1000,10000,100000)


def digest(value):return hashlib.sha256(value.encode()).hexdigest()


def snap(value):
    return _PUBLIC_RECORDS[type(value)](*(snap(getattr(value,field.name))
        if type(getattr(value,field.name)) in _PUBLIC_RECORDS else getattr(value,field.name)
        for field in fields(type(value))))


def record(record_type,**values):
    cls=_PUBLIC_RECORDS[record_type]
    return cls(**({field.name:() for field in fields(record_type)}|values))


def trial(pair=1,variant='one_lane',rate='100',measurement=NS,warmup=0,accounts=4):
    generator=GeneratorPlan('measurement-test',1 if variant=='one_lane' else 4,accounts,
        '127.0.0.1','127.0.0.1',8080,1337)
    load=NativeLoadPlan(pair,variant,digest(f'measurement-test:{pair}'),rate,warmup,measurement,
        200_000_000,1,16,2,10,16,128,8,1,16384,20,5,1)
    other=[kind(**{field.name:1 for field in fields(kind)}) for _,kind in _RECORDS[2:]]
    return TrialPlan(generator,load,*other,1024*1024,NS)


def run(declared=None,committed=None,latency=1_000_000,warmup_latency=None):
    declared=declared or trial();load=declared.load;rate=Fraction(load.offered_load_tps)
    counts=tuple(-(-(duration*rate).numerator//((duration*rate).denominator*NS))
                 for duration in (load.warmup_ns,load.measurement_ns))
    if committed is None:committed=counts[1]
    rotation=int.from_bytes(hashlib.sha256(f'gscale-account-offset-v1:{load.seed}'.encode()).digest()[:8],'little')%declared.generator.account_count
    requests=[]
    for index in range(sum(counts)):
        warm=index<counts[0];ordinal=index if warm else index-counts[0]
        cohort='warmup' if warm else 'measurement';start=-(load.warmup_ns+load.drain_ns) if warm else 0
        offered=start+ordinal*NS*rate.denominator//rate.numerator
        offset=latency if not callable(latency) else latency(ordinal)
        applied=offered+(warmup_latency if warm and warmup_latency is not None else offset)
        if not warm and ordinal>=committed:applied=load.measurement_ns+100_000_000
        tx=digest(f'tx:{load.pair_index}:{load.variant}:{cohort}:{ordinal}')[:-1]+'1'
        requests.append(PublicRequest(index,cohort,ordinal+1,digest(f'{load.seed}:{cohort}:{ordinal+1}'),
            offered,(ordinal+rotation)%declared.generator.account_count,tx,digest('canonical:'+tx),10,
            offered,offered,applied,7,1,applied,7,1))
    geometry=ReplayGeometry(load.warmup_ns,load.measurement_ns,load.drain_ns,
        load.preparation_ahead_ms*1_000_000,load.resource_interval_ms*1_000_000,
        load.resource_timeout_ms*1_000_000,load.resource_max_start_lag_ms*1_000_000)
    def capture(sequence):return record(CaptureReduction,sequence=sequence,rss_before_bytes=200,
        rss_after_bytes=250,queue_size_sum=4,queue_size_max=1,storage_bytes=400,
        represented_entries=50,raw_body_bytes=10,manifest_bytes=10,reported_wire_bytes=20)
    samples=tuple(record(Bracket,scheduled_offset_ns=i*geometry.interval_ns,
        start_offset_ns=i*geometry.interval_ns,end_offset_ns=i*geometry.interval_ns+1,
        capture=capture(i+1)) for i in range(geometry.samples))
    resources=ResourceSnapshot(**({name:0 for name in ResourceSnapshot._fields}|
        {'preflight':capture(0),'samples':samples}))
    generation=record(GenerationReceipt,plan=snap(declared.generator),accounts=tuple(
        record(GeneratedAccount,index=i,account_id=f'public-account-{i}',config=f'private-{i}.toml')
        for i in range(declared.generator.account_count)))
    values={name:() for name in PublicRunProjection._fields}
    return PublicRunProjection(**(values|{'pair_index':load.pair_index,'variant':load.variant,
        'plan':snap(declared),'generation':generation,'load':record(NativeLoadReceipt,plan=snap(load)),
        'peers':tuple(record(ProcessIdentity,pid=i+1,uid=1000,start_seconds=1,start_microseconds=0,
            start_abstime=1,image_uuid='a'*32,executable_sha256='b'*64) for i in range(4)),
        'requests':tuple(requests),'geometry':snap(geometry),'resources':resources}))


def matrix(rate='200',one_counts=None,four_counts=None,one_latency=1_000_000,four_latency=1_000_000):
    trials=tuple(trial(pair,variant,rate) for pair,variant in RUN_KEYS)
    plan=ExperimentPlan('measurement-test',trials,10*NS,120*NS,LIMITS)
    values=[]
    for declared in trials:
        count=(one_counts if declared.load.variant=='one_lane' else four_counts)
        latency=one_latency if declared.load.variant=='one_lane' else four_latency
        values.append(run(declared,None if count is None else count[declared.load.pair_index-1],latency))
    return plan,tuple(values)


def changed_request(value,position,**updates):
    rows=list(value.requests);rows[position]=rows[position]._replace(**updates)
    return value._replace(requests=tuple(rows))


def test_global_applied_measurement_boundary_and_complete_drain_latency():
    value=run();T=value.plan.load.measurement_ns;D=value.plan.load.drain_ns
    value=changed_request(value,0,applied_offset_ns=T-1,local_applied_offset_ns=T+D,
        acknowledgment_offset_ns=T+D)
    value=changed_request(value,1,applied_offset_ns=T,local_applied_offset_ns=20_000_000)
    value=changed_request(value,2,applied_offset_ns=T+D,local_applied_offset_ns=T+D)
    result=measure.measure_run(value,LIMITS)
    assert result.measurement_requests==100 and result.measurement_committed==98 and result.drain_committed==2
    assert result.committed_throughput_tps==Fraction(98,1)
    assert len(result.latencies_ns)==100 and result.latencies_ns[:3]==(T-1,T-10_000_000,T+D-20_000_000)


def test_late_acknowledgment_preserves_earlier_applied_latency_and_inclusive_drain():
    value=run();T=value.plan.load.measurement_ns;D=value.plan.load.drain_ns
    value=changed_request(value,0,acknowledgment_offset_ns=T+D)
    result=measure.measure_run(value,LIMITS)
    assert result.measurement_committed==100 and result.drain_committed==0
    assert result.latencies_ns[0]==1_000_000


def test_warmup_is_fully_drained_and_excluded_from_measurement_p95():
    value=run(trial(warmup=NS),latency=4,warmup_latency=100_000_000)
    result=measure.measure_run(value,LIMITS)
    assert result.warmup_requests==100 and result.warmup_p95_latency_ns==100_000_000
    assert result.p95_latency_ns==4 and result.measurement_requests==100 and len(result.latencies_ns)==100
    with pytest.raises(measure.MeasurementError):measure.measure_run(changed_request(value,0,applied_offset_ns=0),LIMITS)


def test_rational_offered_rate_and_throughput_are_exact():
    result=measure.measure_run(run(trial(rate='100.125',measurement=32*NS)),LIMITS)
    assert result.measurement_requests==3204
    assert result.offered_load_tps==result.committed_throughput_tps==Fraction(801,8)


def test_non_binary_throughput_fraction_is_never_rounded():
    result=measure.measure_run(run(trial(rate='33.33333333333333',measurement=3*NS)),LIMITS)
    assert type(result.committed_throughput_tps) is Fraction
    assert result.committed_throughput_tps==Fraction(100,3)
    assert result.offered_load_tps==Fraction(3333333333333333,100000000000000)


def test_nearest_rank_uses_exact_integer_ceiling_and_all_tail_samples():
    assert measure._p95(tuple(range(1,101)))==95
    assert measure._p95(tuple(range(1,102)))==96
    assert measure._p95(tuple(reversed(range(1,101))))==95
    value=run(latency=lambda i:1 if i<94 else 100_000_000)
    result=measure.measure_run(value,LIMITS)
    assert result.p95_latency_ns==100_000_000 and result.drain_committed==6


@pytest.mark.parametrize('field,number',[('measurement_committed',100),('drain_committed',0)])
def test_complete_zero_warmup_counts_are_explicit(field,number):
    result=measure.measure_run(run(),LIMITS)
    assert getattr(result,field)==number and result.warmup_requests==0 and result.warmup_p95_latency_ns is None


@pytest.mark.parametrize('kind',['missing','extra','reordered','index','cohort','sequence','logical','scheduled',
    'account','offer_before','offer_late','ack_before','ack_late','applied_before','applied_late',
    'local_before','local_late','wrong_height','zero_height','global_attempts','local_attempts','duplicate_hash',
    'unmarked_hash','canonical_digest','body_length','bool_offset'])
def test_complete_request_identity_schedule_and_bounds_fail_closed(kind):
    value=run();row=value.requests[0];T=value.plan.load.measurement_ns;D=value.plan.load.drain_ns
    if kind=='missing':value=value._replace(requests=value.requests[:-1])
    elif kind=='extra':value=value._replace(requests=(*value.requests,row))
    elif kind=='reordered':value=value._replace(requests=(value.requests[1],row,*value.requests[2:]))
    else:
        updates={'index':{'index':1},'cohort':{'cohort':'warmup'},'sequence':{'sequence':2},
            'logical':{'logical_id':'0'*64},'scheduled':{'scheduled_offset_ns':1},'account':{'account_index':99},
            'offer_before':{'offer_offset_ns':-1},'offer_late':{'offer_offset_ns':2},
            'ack_before':{'acknowledgment_offset_ns':-1},'ack_late':{'acknowledgment_offset_ns':T+D+1},
            'applied_before':{'applied_offset_ns':0},'applied_late':{'applied_offset_ns':T+D+1},
            'local_before':{'local_applied_offset_ns':0},'local_late':{'local_applied_offset_ns':T+D+1},
            'wrong_height':{'local_block_height':8},'zero_height':{'block_height':0},
            'global_attempts':{'status_attempts':0},'local_attempts':{'local_status_attempts':0},
            'duplicate_hash':{'transaction_hash':value.requests[1].transaction_hash},
            'unmarked_hash':{'transaction_hash':'0'*64},'canonical_digest':{'canonical_sha256':'x'*64},
            'body_length':{'canonical_size_bytes':0},'bool_offset':{'applied_offset_ns':True}}[kind]
        value=changed_request(value,0,**updates)
    with pytest.raises(measure.MeasurementError):measure.measure_run(value,LIMITS)


@pytest.mark.parametrize('invalid', ['1'*63, '1'*65, 'A'*63+'1', '1'*63+'0', 'g'*63+'1'])
def test_signed_transaction_hash_requires_exact_sdk_text_shape(invalid):
    value=changed_request(run(),0,transaction_hash=invalid)
    with pytest.raises(measure.MeasurementError):measure.measure_run(value,LIMITS)


def test_minimum_latency_sample_count_is_never_weakened():
    with pytest.raises(measure.MeasurementError):measure.measure_run(run(trial(rate='96')),LIMITS)
    with pytest.raises(measure.MeasurementError):measure.measure_run(run(),replace(LIMITS,min_latency_samples=101))
    with pytest.raises(measure.MeasurementError):measure.measure_run(run(),replace(LIMITS,min_latency_samples=99))


def test_resource_reduction_includes_preflight_final_tail_and_both_rss_edges():
    value=run();before=value.resources.preflight._replace(queue_size_sum=20,queue_size_max=5)
    last=value.resources.samples[-1];last=last._replace(capture=last.capture._replace(
        represented_entries=500,rss_before_bytes=800,rss_after_bytes=700,storage_bytes=9000))
    resources=value.resources._replace(preflight=before,samples=(*value.resources.samples[:-1],last))
    result=measure.measure_run(value._replace(resources=resources),LIMITS)
    assert result.observed_resources==ResourceMaxima(20,500,800,9000) and result.observed_resource_limits_met
    lower=replace(LIMITS,disk_bytes_max=8999)
    result=measure.measure_run(value._replace(resources=resources),lower)
    assert not result.observed_resource_limits_met and result.observed_resources.disk_bytes_max==9000


def test_final_resource_sample_keeps_the_original_fixed_terminal_deadline():
    value=run();last=value.resources.samples[-1]
    terminal=value.plan.load.measurement_ns+value.plan.load.drain_ns+5_000_000
    last=last._replace(start_offset_ns=last.scheduled_offset_ns+1_000_000,end_offset_ns=terminal-1)
    accepted=value._replace(resources=value.resources._replace(samples=(*value.resources.samples[:-1],last)))
    assert measure.measure_run(accepted,LIMITS).measurement_requests==100
    rejected=value._replace(resources=value.resources._replace(samples=(
        *value.resources.samples[:-1],last._replace(end_offset_ns=terminal))))
    with pytest.raises(measure.MeasurementError):measure.measure_run(rejected,LIMITS)


@pytest.mark.parametrize('kind',['missing','duplicate','foreign','bool_pid'])
def test_resources_require_four_distinct_original_peer_snapshot_shapes(kind):
    value=run();peers=list(value.peers)
    if kind=='missing':peers.pop()
    elif kind=='duplicate':peers[1]=peers[0]
    elif kind=='foreign':peers[0]=object()
    else:peers[0]=peers[0]._replace(pid=True)
    with pytest.raises(measure.MeasurementError):measure.measure_run(value._replace(peers=tuple(peers)),LIMITS)


@pytest.mark.parametrize('kind',['missing','extra','order','start','duration','capture_sequence','negative','bool','rss','queue','geometry'])
def test_resource_sample_scope_and_bounds_reject(kind):
    value=run();resources=value.resources;rows=list(resources.samples)
    if kind=='missing':rows.pop()
    elif kind=='extra':rows.append(rows[-1])
    elif kind=='order':rows[0],rows[1]=rows[1],rows[0]
    elif kind=='start':rows[0]=rows[0]._replace(start_offset_ns=1_000_001)
    elif kind=='duration':rows[0]=rows[0]._replace(end_offset_ns=5_000_000)
    elif kind=='geometry':value=value._replace(geometry=value.geometry._replace(warmup_ns=1))
    else:
        updates={'capture_sequence':{'sequence':2},'negative':{'storage_bytes':-1},'bool':{'represented_entries':True},
                 'rss':{'rss_before_bytes':0},'queue':{'queue_size_max':0}}[kind]
        rows[0]=rows[0]._replace(capture=rows[0].capture._replace(**updates))
    value=value._replace(resources=resources._replace(samples=tuple(rows)))
    with pytest.raises(measure.MeasurementError):measure.measure_run(value,LIMITS)


def test_exact_throughput_threshold_and_ratio_of_medians():
    plan,values=matrix(one_counts=(1,2,100,150,200),four_counts=(100,150,151,152,149))
    result=measure.measure_experiment(plan,values)
    assert result.one_lane_median_throughput_tps==100 and result.four_lane_median_throughput_tps==150
    assert result.median_throughput_ratio==Fraction(3,2) and result.throughput_criterion_met
    pair_ratios=tuple(result.runs[index+1].committed_throughput_tps/result.runs[index].committed_throughput_tps
        for index in range(0,10,2))
    assert sorted(pair_ratios)[2]==Fraction(151,100) and sorted(pair_ratios)[2]!=result.median_throughput_ratio
    plan,values=matrix(one_counts=(100,)*5,four_counts=(149,)*5)
    result=measure.measure_experiment(plan,values)
    assert result.median_throughput_ratio==Fraction(149,100) and not result.throughput_criterion_met


def test_exact_latency_ratio_boundary_and_one_nanosecond_failure():
    plan,values=matrix(one_latency=4_000_000,four_latency=5_000_000)
    result=measure.measure_experiment(plan,values)
    assert result.pooled_p95_latency_ratio==Fraction(5,4) and result.latency_criterion_met
    plan,values=matrix(one_latency=4_000_000,four_latency=5_000_001)
    result=measure.measure_experiment(plan,values)
    assert result.pooled_p95_latency_ratio==Fraction(5_000_001,4_000_000) and not result.latency_criterion_met


def test_experiment_p95_pools_complete_cohorts_instead_of_median_run_p95():
    plan,values=matrix(one_latency=4,four_latency=5)
    items=list(values);items[-1]=run(plan.trials[-1],latency=1000)
    result=measure.measure_experiment(plan,tuple(items))
    assert result.four_lane_pooled_p95_latency_ns==1000 and result.pooled_p95_latency_ratio==250
    assert not result.latency_criterion_met


@pytest.mark.parametrize('kind',['nine','eleven','order','duplicate_key','work','policy','seed','account_pool','cross_hash','zero_baseline'])
def test_exact_five_matched_pairs_are_required(kind):
    plan,values=matrix();items=list(values)
    if kind=='nine':items.pop()
    elif kind=='eleven':items.append(items[-1])
    elif kind=='order':items[0],items[1]=items[1],items[0]
    elif kind=='duplicate_key':items[1]=items[1]._replace(variant='one_lane')
    elif kind in ('work','policy','seed'):
        change={'work':{'offered_load_tps':'204'},'policy':{'max_in_flight':129},'seed':{'seed':'0'*64}}[kind]
        declared=replace(plan.trials[1],load=replace(plan.trials[1].load,**change))
        declared_trials=list(plan.trials);declared_trials[1]=declared;plan=replace(plan,trials=tuple(declared_trials));items[1]=run(declared)
    elif kind=='account_pool':
        generation=items[1].generation;accounts=list(generation.accounts);accounts[0]=accounts[0]._replace(account_id='different')
        items[1]=items[1]._replace(generation=generation._replace(accounts=tuple(accounts)))
    elif kind=='cross_hash':items[1]=changed_request(items[1],0,transaction_hash=items[0].requests[0].transaction_hash)
    else:
        plan,values=matrix(one_counts=(0,)*5);items=list(values)
    with pytest.raises(measure.MeasurementError):measure.measure_experiment(plan,tuple(items))


def test_resource_failure_is_separate_from_performance_criteria_and_no_pass_exists():
    plan,values=matrix(one_counts=(100,)*5,four_counts=(150,)*5)
    plan=replace(plan,resource_limits=replace(LIMITS,memory_bytes_max=249))
    result=measure.measure_experiment(plan,values)
    assert result.throughput_criterion_met and not result.observed_resource_criterion_met
    assert result.one_lane_observed_resources.memory_bytes_max==250
    assert not hasattr(result,'passed') and not hasattr(result,'authority')


def test_foreign_inputs_reject_before_getters_or_fraction_conversion():
    calls=[]
    class Foreign:
        def __getattr__(self,name):calls.append(name);raise AssertionError('foreign getter')
    with pytest.raises(measure.MeasurementError):measure.measure_run(Foreign(),LIMITS)
    value=run();value=value._replace(plan=value.plan._replace(load=Foreign()))
    with pytest.raises(measure.MeasurementError):measure.measure_run(value,LIMITS)
    assert calls==[]
    declared=trial(rate='1'*65)
    value=run()._replace(plan=snap(declared),load=record(NativeLoadReceipt,plan=snap(declared.load)))
    with pytest.raises(measure.MeasurementError):measure.measure_run(value,LIMITS)


def test_maximum_complete_native_account_pool_remains_supported():
    value=run(trial(rate='65536',accounts=64),latency=1)
    result=measure.measure_run(value,LIMITS)
    assert result.measurement_requests==65536 and result.committed_throughput_tps==65536
    assert result.p95_latency_ns==1
