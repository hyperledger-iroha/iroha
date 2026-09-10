"""Synthetic main component evidence using actual publisher/scanner/replay owners.

This is test input construction, never canonical or release qualification. Trusted
scope objects are built from fixture-owned configuration and fixed peer identities,
not learned from capture manifests. Every capture is emitted by the real publisher.
"""
from dataclasses import asdict
from pathlib import Path
import copy
from signed_request_fixture import add_retention
import hashlib
import json
import os
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'nexus'))
import resource_evidence_budget as budget
import resource_experiment as experiment
import resource_bundle as bundle
import resource_replay as replay
import resource_probe as probe
import resource_probe_worker as worker
import resource_process as process
import kura_resource_metrics as metrics

INSTANCES = {}
FIELDS = ('queue_depth', 'index_entries', 'memory_bytes', 'disk_bytes')
BASE = dict(zip(FIELDS, (12, 24, 1019, 2019)))
ROLES = ('nexus_load_test_manifest','lifecycle_snapshot','metrics_snapshot','load_generator_log')
POLICY = budget.CapturePolicy(4096, 16384)

def encode(value): return json.dumps(value, separators=(',', ':'), sort_keys=True).encode()
def sha(value): return hashlib.sha256(value).hexdigest()

def geometry(fixture):
    w = fixture.workload
    return replay.ReplayGeometry(round(w['warmup_seconds']*1e9), round(w['measurement_seconds']*1e9),
        round(w['drain_seconds']*1e9), 1_000_000_000, 1_000_000_000, 400_000_000, 100_000_000)

def observation(peers, values):
    def share(total, index): return total // len(peers) + (index < total % len(peers))
    rows=[]
    for index, peer in enumerate(peers):
        queue, entries, rss, disk=(share(values[name],index) for name in FIELDS)
        raw=['iroha_kura_resource_available 1','iroha_kura_resource_status{reason="available"} 1',
             'iroha_kura_resource_generation 1','iroha_kura_resource_fault_count 0']
        fields=('resident_associations','persisted_entries','index_bytes','temporary_index_bytes','storage_bytes')
        for family in metrics.FAMILIES:
            for field in fields:
                amount = entries if family == metrics.FAMILIES[0] and field == 'resident_associations' else disk if family == metrics.FAMILIES[-1] and field == 'storage_bytes' else 0
                raw.append(f'iroha_kura_resource_{field}{{family="{family}"}} {amount}')
        for field in fields:
            amount=entries if field=='resident_associations' else disk if field=='storage_bytes' else 0
            raw.append(f'iroha_kura_resource_{field}_sum {amount}')
        raw.append(f'iroha_kura_resource_represented_entries {entries}')
        raw=('\n'.join(raw)+'\n').encode(); status=encode({'queue_size':queue})
        provenance=lambda route,body,kind:probe.HttpProvenance(route,sha(body),len(body),kind,body)
        sample=process.ProcessSample(peer.identity,rss)
        rows.append(probe.PeerObservation(peer.peer_id,queue,provenance('/status',status,'application/json'),
            provenance('/metrics',raw,'text/plain'),metrics.parse_kura_resource_metrics(raw),sample,sample))
    return probe.ProbeObservation(tuple(rows),1_000_000,
        sum(len(r.status.raw_body)+len(r.metrics.raw_body)+100 for r in rows), values['memory_bytes'],values['memory_bytes'],
        values['queue_depth'],max(r.queue_size for r in rows),probe.InventoryAggregate(values['disk_bytes'],values['index_entries']),POLICY)

def recount(raw, trace):
    rows=[r for r in trace['transactions'] if r['cohort']=='measurement']
    for phase in (raw,raw['drain']):
        for sample in phase['samples']:
            start=round(sample['start_offset_seconds']*1e9); end=round(sample['end_offset_seconds']*1e9)
            def inside(value):return start<=value<end or (phase is raw['drain'] and sample is phase['samples'][-1] and value==end)
            sample['offered_count']=sum(inside(r['offer_offset_ns']) for r in rows)
            sample['accepted_count']=sum(r['acknowledgment']['status']=='Accepted' and inside(r['acknowledgment']['offset_ns']) for r in rows)
            applied=sorted((r['applied']['offset_ns'],r['sequence'],(r['applied']['offset_ns']-r['offer_offset_ns'])/1e6)
                for r in rows if r['applied'] is not None and inside(r['applied']['offset_ns']))
            sample['committed_count']=len(applied);sample['commit_latencies_ms']=[x[2] for x in applied]
        for name in ('offered_count','accepted_count','committed_count'):
            phase['summary'][name]=sum(s[name] for s in phase['samples'])

def complete_outcomes(fixture, raw, trace, committed, latency):
    """All offers succeed; one-lane measurement loss is successful drain work.

    Exactly 20 observations exceed a selected latency marker, making nearest-rank
    cohort p95 exactly 1600*latency milliseconds. This lets existing ratio controls
    keep 1.2 and 1.3 while every outcome finishes by the same bounded drain.
    """
    rows=[r for r in trace['transactions'] if r['cohort']=='measurement']
    target=round(latency*1_600_000_000)
    marker=max(20,(20_000_000_000-target+49_999_999)//50_000_000)
    fast=set(i for i in range(20,len(rows)) if i!=marker)
    fast=set(sorted(fast)[:committed])
    for i,row in enumerate(rows):
        row['acknowledgment'].update(status='Accepted',rejection=None)
        applied=row['offer_offset_ns']+round(latency*1e6) if i in fast else (
            21_900_000_000 if i<20 else row['offer_offset_ns']+target if i==marker else 20_000_000_000)
        row['applied']={'offset_ns':applied,'hash':row['hash'],'scope':'global','resolved_from':'state','status':'Applied','block_height':1+i//20}
    for phase in (raw,raw['drain']):
        for sample in phase['samples']:sample.update(BASE)
        for name in FIELDS:phase['summary'][name+'_max']=BASE[name]
    recount(raw,trace)
    if hasattr(fixture, 'resource_events') and (raw['pair_index'], raw['variant']) in fixture.resource_events:
        previous=fixture.load_raw(raw['pair_index'],raw['variant'])
        for role in ('collector_journal','canonical_proof'):raw['artifacts'][role]=previous['artifacts'][role]
    fixture.write_json(fixture.root/raw['artifacts']['transaction_trace']['path'],trace)
    raw['artifacts']['transaction_trace']=fixture.ref(fixture.root/raw['artifacts']['transaction_trace']['path'])
    return raw

def make_budget(fixture):
    static={role:fixture.root/fixture.manifest[role]['path'] for role in ('identity','configuration','trial_harness','validator')}
    static.update({row['role']:fixture.root/row['artifact']['path'] for row in fixture.manifest['tooling']})
    geom=geometry(fixture);cg=budget.CaptureGeometry(4,geom.interval_ns,geom.measurement_ns,geom.drain_ns)
    runs=[];paths={}
    for entry in fixture.manifest['runs']:
        pair,variant=entry['pair_index'],entry['variant'];prefix=f'pair-{pair:02}.{variant}'
        directory=fixture.root/'runs'/f'pair_{pair:02}'/variant
        raw=fixture.load_raw(pair,variant)
        own={role:directory/'support'/name for role,name in [('collector_journal','collector.jsonl'),('canonical_proof','canonical-proof.json')]}
        own.update(transaction_trace=fixture.root/raw['artifacts']['transaction_trace']['path'],trial_log=fixture.root/entry['command_log']['path'],raw_run=fixture.root/entry['raw_samples']['path'])
        support=tuple(budget.FileBudget(prefix+'.'+role,1_000_000) for role in ROLES)
        runs.append(budget.RunBudget(pair,variant,cg,*(budget.FileBudget(prefix+'.'+role,2_000_000) for role in ('collector_journal','transaction_trace','canonical_proof','trial_log','raw_run')),support=support))
        paths.update({prefix+'.'+role:path for role,path in own.items()})
        paths.update({prefix+'.'+role:fixture.root/raw['artifacts'][role]['path'] for role in ROLES})
    paths.update(static);paths['manifest']=fixture.manifest_path
    admitted=budget.admit_experiment(policy=POLICY,runs=tuple(runs),static_files=tuple(budget.StaticFile(role,path.stat().st_size) for role,path in static.items()),manifest=budget.FileBudget('manifest',1_000_000),report=budget.FileBudget('report',1_000_000),other_control=())
    return admitted,paths

def journal(fixture,pair,variant,resource_events):
    trace=fixture.load_trace(pair,variant); rows=trace['transactions'];g=geometry(fixture);seed=trace['seed']
    accounts=tuple(f'fixture-account-{i}' for i in range(4));effects=len(rows)//4
    offset=int.from_bytes(hashlib.sha256(f'gscale-account-offset-v1:{seed}'.encode()).digest()[:8],'little')%4
    plan=dict(submission_lag_bound_ns=round(fixture.workload['max_submission_lag_ms']*1e6), preparation_lookahead=64, preparation_concurrency=4, max_submissions=32, max_in_flight=512, max_status_requests=64, poll_interval_ns=10_000_000);plan.update(event='plan',schema=replay.JOURNAL_SCHEMA,pair_index=pair,variant=variant,seed=seed,accounts=[{'authority':a} for a in accounts],
        account_selection='(zero_based_cohort_sequence + first8le(sha256(gscale-account-offset-v1:seed))) modulo pool_length',workload='self_owned_account_metadata_insert_v1',max_effects_per_account=1024,scheduled_requests=len(rows),
        **{name:getattr(g,name) for name in ('warmup_ns','measurement_ns','drain_ns','preparation_ahead_ns')})
    def planned(row):return {k:row[k] for k in ('cohort','sequence','logical_id','scheduled_offset_ns')}|{'account_index':(row['sequence']-1+offset)%4}
    events=[plan,resource_events[0]]
    events.extend({'event':'scheduled','index':i,'plan':planned(row)} for i,row in enumerate(rows))
    digests=tuple(sha(f'{seed}:account-post-state:{i}'.encode()) for i in range(4))
    events.extend({'event':'workload_account_preflight','authority':a,'account_index':i,'expected_effects':effects,'expected_account_sha256':digests[i],'expected_account_frame_bytes':128} for i,a in enumerate(accounts))
    events.append({'event':'clock_started','initial_offset_ns':-(g.warmup_ns+g.drain_ns+g.preparation_ahead_ns)})
    events.extend(resource_events[1:]);events.append({'event':'workload_postconditions_started'})
    events.extend({'event':'workload_account_postcondition','authority':a,'account_index':i,'verified_effects':effects,'account_sha256':digests[i],'read_source':'signed_find_account_by_id_after_complete_drain'} for i,a in enumerate(accounts))
    events.extend({'event':'request_final','plan':planned(r),'hash':r['hash'],'offer_offset_ns':r['offer_offset_ns'],
        'acknowledgment_offset_ns':r['acknowledgment']['offset_ns'],'applied_offset_ns':r['applied']['offset_ns'],'block_height':r['applied']['block_height'],'status_attempts':1,'submission_finished':True,'failure':None} for r in rows)
    events.append({'event':'collection_finished','passed':True,'failure':None})
    events=add_retention(events)
    path=fixture.root/'runs'/f'pair_{pair:02}'/variant/'support/collector.jsonl';path.write_bytes(b''.join(encode(row)+b'\n' for row in events));path.chmod(0o600)
    return path

def initialize(fixture,validator):
    fixture.root.chmod(0o700);fixture._validator=validator;fixture.resource_events={};fixture.resource_runs=[];fixture.expected_executable_sha256=fixture.identity['software']['irohad_sha256']
    # Make all role files before deriving the complete immutable experiment budget.
    for entry in fixture.manifest['runs']:
        pair,variant=entry['pair_index'],entry['variant'];raw=fixture.load_raw(pair,variant);support=fixture.root/'runs'/f'pair_{pair:02}'/variant/'support'
        for role,name in [('collector_journal','collector.jsonl'),('canonical_proof','canonical-proof.json')]:
            path=support/name;path.write_bytes(b'{"fixture":"unverified canonical placeholder; component only"}\n');path.chmod(0o600);raw['artifacts'][role]=fixture.ref(path)
        fixture.replace_raw(pair,variant,raw)
    fixture.resource_budget,fixture.control_paths=make_budget(fixture)
    for run_index,entry in enumerate(fixture.manifest['runs']):
        pair,variant=entry['pair_index'],entry['variant'];g=geometry(fixture)
        peers=tuple(replay.ExpectedPeer(f'validator{i}',process.ProcessIdentity(1000+run_index*10+i,os.geteuid(),run_index+1,0,100+run_index*10+i,'01'*16,fixture.expected_executable_sha256)) for i in range(4))
        fixture.resource_runs.append(experiment.RunReplayInput(pair,variant,peers,g))
        path=fixture.root/'resources'/f'pair-{pair:02}'/variant;path.mkdir(mode=0o700,parents=True)
        allocation=budget.select_run_budget(fixture.resource_budget,pair,variant);events=[]
        with worker.CaptureDirectory(path,allocation) as owner:
            ref=owner.publish('preflight',0,observation(peers,BASE),probe._Deadline(5))
            events.append({'event':'resource_preflight','sequence':0,'outcome':'complete','manifest':asdict(ref),'sampling':g.sampling()})
            for index in range(g.samples):
                scheduled=index*g.interval_ns;ref=owner.publish('sample',index+1,observation(peers,BASE),probe._Deadline(5))
                events.extend(({'event':'resource_request','kind':'sample','sequence':index+1,'scheduled_offset_ns':scheduled,'start_offset_ns':scheduled},
                    {'event':'resource_observation','sequence':index+1,'scheduled_offset_ns':scheduled,'start_offset_ns':scheduled,'end_offset_ns':scheduled+1_000_000,'outcome':'complete','manifest':asdict(ref)}))
        finish=g.final+1_000_000;events.extend(({'event':'resource_request','kind':'finish','sequence':g.samples+1,'start_offset_ns':finish},{'event':'resource_collection_finished','sequence':g.samples+1,'start_offset_ns':finish,'end_offset_ns':finish+1_000_000,'sampling':g.sampling()}))
        fixture.resource_events[pair,variant]=events
        path=journal(fixture,pair,variant,events);raw=fixture.load_raw(pair,variant);raw['artifacts']['collector_journal']=fixture.ref(path);fixture.replace_raw(pair,variant,raw)
    fixture.resource_runs=tuple(fixture.resource_runs)
    for path in fixture.root.rglob('*'):path.chmod(0o700 if path.is_dir() else 0o600)
    fixture.controls=tuple(bundle.ControlBinding(label,str(path.relative_to(fixture.root)),sha(path.read_bytes())) for label,path in fixture.control_paths.items())
    INSTANCES[fixture.root]=fixture

def pin(fixture,path,digest=None):
    """Explicit fixture-owner rehash; validation never learns its own expected hash."""
    if not hasattr(fixture,'controls'):return
    from dataclasses import replace
    matching={label for label,owned in fixture.control_paths.items() if owned==path}
    if matching:
        digest=digest or sha(path.read_bytes())
        fixture.controls=tuple(replace(row,sha256=digest) if row.label in matching else row for row in fixture.controls)

def refresh_journal(fixture,pair,variant):
    if not hasattr(fixture,'resource_events') or (pair,variant) not in fixture.resource_events:return
    path=journal(fixture,pair,variant,fixture.resource_events[pair,variant]);raw=fixture.load_raw(pair,variant);raw['artifacts']['collector_journal']=fixture.ref(path);fixture.replace_raw(pair,variant,raw)

def validate_component(manifest_path,**expected):
    """Actual component path; no public release wrapper or qualifying proof claim."""
    fixture=INSTANCES[manifest_path.parent];validator=fixture._validator
    controls=fixture.controls
    admission=validator.ResourceAdmission(fixture.resource_budget,controls,fixture.resource_runs,fixture.expected_executable_sha256,False)
    with experiment.ResourceExperiment(fixture.root,admission.budget,admission.controls,admission.runs,expected_executable_sha256=admission.expected_executable_sha256,reported=False) as owner:
        selected=validator.EvidenceControls(owner,admission)
        owner.collect_replay()
        result=validator._validate_admitted_evidence(manifest_path,controls=selected,**expected)
        verified=owner.verify();assert len(verified)==10
        return result


def refresh_warmup_scope(fixture):
    """A separate synthetic zero-warmup run uses matching trusted input geometry."""
    from dataclasses import replace
    fixture.resource_runs=tuple(replace(run,geometry=geometry(fixture)) for run in fixture.resource_runs)


def republish_drain_queue(fixture,pair,variant,value):
    """Replace only this fixture-owned input before admitting its next owner.

    Uses the actual no-clobber publisher in a fresh directory; never edits a
    manifest or substitutes a replay result inside an admitted experiment.
    """
    import shutil
    run=next(r for r in fixture.resource_runs if (r.pair_index,r.variant)==(pair,variant));g=run.geometry
    path=fixture.root/'resources'/f'pair-{pair:02}'/variant
    assert path.is_dir() and path.parent.parent == fixture.root/'resources'
    shutil.rmtree(path);path.mkdir(mode=0o700)
    allocation=budget.select_run_budget(fixture.resource_budget,pair,variant);events=[]
    with worker.CaptureDirectory(path,allocation) as owner:
        ref=owner.publish('preflight',0,observation(run.peers,BASE),probe._Deadline(5))
        events.append({'event':'resource_preflight','sequence':0,'outcome':'complete','manifest':asdict(ref),'sampling':g.sampling()})
        for index in range(g.samples):
            scheduled=index*g.interval_ns;values=BASE|({'queue_depth':value} if scheduled==g.measurement_ns+g.interval_ns else {})
            ref=owner.publish('sample',index+1,observation(run.peers,values),probe._Deadline(5))
            events.extend(({'event':'resource_request','kind':'sample','sequence':index+1,'scheduled_offset_ns':scheduled,'start_offset_ns':scheduled},
                {'event':'resource_observation','sequence':index+1,'scheduled_offset_ns':scheduled,'start_offset_ns':scheduled,'end_offset_ns':scheduled+1_000_000,'outcome':'complete','manifest':asdict(ref)}))
    finish=g.final+1_000_000;events.extend(({'event':'resource_request','kind':'finish','sequence':g.samples+1,'start_offset_ns':finish},{'event':'resource_collection_finished','sequence':g.samples+1,'start_offset_ns':finish,'end_offset_ns':finish+1_000_000,'sampling':g.sampling()}))
    fixture.resource_events[pair,variant]=events;refresh_journal(fixture,pair,variant)


def clone_fixture(template,root):
    """Independent regular-file copy of actual publisher output for each control."""
    import shutil
    root=root.resolve(strict=True);shutil.copytree(template.root,root,dirs_exist_ok=True)
    result=object.__new__(type(template))
    values=copy.deepcopy({key:value for key,value in template.__dict__.items() if key!='_validator'})
    values.update(root=root,manifest_path=root/'scaling_evidence.json',_validator=template._validator,
        control_paths={label:root/path.relative_to(template.root) for label,path in template.control_paths.items()})
    result.__dict__.update(values);INSTANCES[root]=result;return result
