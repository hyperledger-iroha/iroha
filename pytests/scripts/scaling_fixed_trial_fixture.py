"""Actual lifecycle owners with fake native pipes and real synthetic resource replay.

Signed/proof bytes are deliberately unverified fixture data. No native command,
cryptographic verification, network action or operating-system child is executed.
"""
from dataclasses import asdict
from fractions import Fraction
import hashlib
import json
import math
import os
from pathlib import Path
from types import SimpleNamespace

import pytest
from scaling_readiness_fixture import ready_setup, CommandFactory, PipeChild, compact
from scaling_generator_fixture import consume_seed_pipe, emit, refresh
from scaling_canonical_proof_test import projection, wire
from resource_replay_test import Fixture
from signed_request_fixture import add_retention
import resource_evidence_budget as budget
import resource_process as process
import scaling_command as command
import scaling_fixed_trial as trial
from scaling_canonical_proof import ReplayBindings
from scaling_native_outputs import _STAGES


def plan(lanes=4):
    generator=trial.GeneratorPlan('trial-test-chain',lanes,4,'127.0.0.1','127.0.0.1',8080,1337)
    load=trial.NativeLoadPlan(1,'one_lane' if lanes==1 else 'four_lane','a'*64,'40',100_000_000,
        200_000_000,10_000_000,0,2,2,20,4,32,4,1,256,10,4,2)
    reader=trial.ReaderBudget(1000,8*budget.MIB,65536,budget.MIB,1000,budget.MIB,2*budget.MIB,os.geteuid())
    collection=trial.CollectionLimits(65536,65536,budget.MIB,4096,1000,1000,4*budget.MIB)
    facts=trial.FactsBudget(65536,65536,65536,65536,8*budget.MIB,65536,
        8*budget.MIB+65536,8*budget.MIB,4*budget.MIB,2*budget.MIB,2*budget.MIB,1000,128,1024)
    outputs=trial.NativeOutputBudget(*(65536 for _ in range(6)),65536*6)
    return trial.TrialPlan(generator,load,reader,collection,facts,outputs,128*1024,5_000_000_000)


def allocation(value):
    p=value.load;geometry=budget.CaptureGeometry(4,p.resource_interval_ms*1_000_000,p.measurement_ns,p.drain_ns)
    rows=[]
    for pair in range(1,6):
        for variant in ('one_lane','four_lane'):
            caps={name:budget.FileBudget(f'pair{pair}.{variant}.{name}',
                  65536 if name in ('canonical_proof','native_finality','native_queries','native_facts','native_request','native_bundle')
                  else 4*budget.MIB if name=='collector_journal' else budget.MIB)
                  for name in budget.RUN_FILE_FIELDS}
            rows.append(budget.RunBudget(pair,variant,geometry,**caps))
    complete=budget.admit_experiment(policy=budget.CapturePolicy(),runs=tuple(rows),
        static_files=(budget.StaticFile('software',budget.MIB),),manifest=budget.FileBudget('manifest',budget.MIB),
        report=budget.FileBudget('report',budget.MIB),other_control=())
    return budget.select_run_budget(complete,p.pair_index,p.variant)


class TrialFactory(CommandFactory):
    def __init__(self,c):
        super().__init__(c.clock,{})
        self.c=c;self.trial=None;self.operations=[];self.fixture=None
        self.before=lambda operation,argv:None
        self.transform=lambda operation,value:value
        self.after=lambda operation,child:None
        self.proof_rows=None

    def _load_files(self,argv):
        owner=self.trial;native=owner._generated.inputs
        root=self.c.root/f'synthetic-resource-source-{owner._paths.pair_index}-{owner._paths.variant}';root.mkdir(mode=0o700)
        f=Fixture(root)
        identities=[asdict(pin.identity) for pin in owner._launch._pinned]
        for event in f.events:
            if event['event'] in ('resource_preflight','resource_observation'):
                path=f.directory/event['manifest']['name'];value=json.loads(path.read_bytes())
                for index,peer in enumerate(value['peers']):
                    peer['process_before']['identity']=identities[index]
                    peer['process_after']['identity']=identities[index]
                f.rewrite(event,path,value)
        value=owner._plan.load;geometry=owner._geometry()
        rows=[row for row in f.events if row['event'] in ('plan','resource_preflight','clock_started',
            'resource_request','resource_observation','resource_collection_finished')]
        next(row for row in rows if row['event']=='clock_started')['initial_offset_ns']=-(
            geometry.warmup_ns+geometry.drain_ns+geometry.preparation_ahead_ns)
        rows[0].update(pair_index=value.pair_index,variant=value.variant,seed=value.seed,
            accounts=[account.account_id for account in native.generation.accounts],local_applied_required=True,
            account_selection='(zero_based_cohort_sequence + first8le(sha256(gscale-account-offset-v1:seed))) modulo pool_length',
            workload='self_owned_account_metadata_insert_v1',max_effects_per_account=1024,
            scheduled_requests=owner._scheduled,submission_lag_bound_ns=value.submission_lag_ns,
            preparation_lookahead=value.preparation_lookahead,preparation_concurrency=value.preparation_concurrency,
            max_submissions=value.max_submissions,max_in_flight=value.max_in_flight,max_status_requests=value.max_status_requests,
            poll_interval_ns=value.poll_interval_ms*1_000_000,
            **{name:getattr(geometry,name) for name in ('warmup_ns','measurement_ns','drain_ns','preparation_ahead_ns')})
        rate=Fraction(value.offered_load_tps,1) if type(value.offered_load_tps) is int else Fraction(value.offered_load_tps)
        counts=tuple(math.ceil(Fraction(duration,1_000_000_000)*rate) for duration in (value.warmup_ns,value.measurement_ns))
        rotation=int.from_bytes(hashlib.sha256(f'gscale-account-offset-v1:{value.seed}'.encode()).digest()[:8],'little')%4
        index=0;finals=[]
        for cohort,count,start in (('warmup',counts[0],-(value.warmup_ns+value.drain_ns)),('measurement',counts[1],0)):
            for ordinal in range(count):
                offset=start+math.floor(Fraction(ordinal*1_000_000_000,1)/rate)
                tx_hash=hashlib.sha256(f'opaque trial fixture {value.seed}:{value.variant}:{index}'.encode()).hexdigest()[:-1]+'1'
                applied=geometry.final if cohort=='measurement' and ordinal+1==count else offset+2
                row=dict(event='request_final',plan=dict(cohort=cohort,sequence=ordinal+1,
                    logical_id=hashlib.sha256(f'{value.seed}:{cohort}:{ordinal+1}'.encode()).hexdigest(),
                    scheduled_offset_ns=offset,account_index=(ordinal+rotation)%4),hash=tx_hash,
                    offer_offset_ns=offset,acknowledgment_offset_ns=offset+3,applied_offset_ns=applied,
                    block_height=index+2,status_attempts=1,local_applied_offset_ns=offset+1,local_block_height=index+2,
                    local_status_attempts=1,submission_finished=True,failure=None)
                finals.append(row);index+=1
        rows.extend(finals);rows.append(dict(event='collection_finished',passed=True,failure=None))
        f.events=add_retention(rows);f.save()
        self.fixture=f
        paths=owner._paths.load
        f.directory.rename(paths.resource_capture_dir);f.directory=paths.resource_capture_dir
        f.journal.rename(paths.collector_journal);f.journal=paths.collector_journal
        trace={'schema':'iroha.sumeragi_v2.multilane_scaling.transaction_trace.v1','pair_index':value.pair_index,
            'variant':value.variant,'seed':value.seed,'clock':'monotonic_nanoseconds_relative_to_measurement_start',
            "logical_id_derivation":"sha256(seed + ':' + cohort + ':' + decimal_sequence)",'transaction_hash_source':'iroha_data_model::transaction::SignedTransaction::hash',
            'transactions':[dict(cohort=row['plan']['cohort'],sequence=row['plan']['sequence'],logical_id=row['plan']['logical_id'],
                hash=row['hash'],scheduled_offset_ns=row['plan']['scheduled_offset_ns'],offer_offset_ns=row['offer_offset_ns'],submission_lag_ns=0,
                acknowledgment=dict(offset_ns=row['acknowledgment_offset_ns'],hash=row['hash'],status='Accepted',rejection=None),
                applied=dict(offset_ns=row['applied_offset_ns'],hash=row['hash'],scope='global',resolved_from='state',status='Applied',block_height=row['block_height']))
                for row in finals]}
        paths.transaction_trace.write_bytes(compact(trace));paths.transaction_trace.chmod(0o600)
        return paths

    def _publish(self,role,path):
        stage=path.with_name(path.name+_STAGES[role]);stage.write_bytes(('unverified-native-'+role).encode());stage.chmod(0o600);stage.rename(path)
        return hashlib.sha256(path.read_bytes()).hexdigest(),path.stat().st_size

    def __call__(self,argv,**kwargs):
        if argv[0]==str(self.c.images[0].path):
            self.operations.append('launch-peer');self.before('launch-peer',argv)
            return self.c.daemons(argv,**kwargs)
        if '--out-dir' in argv:operation='generator'
        elif '--challenge' in argv:operation='readiness'
        elif 'load' in argv:operation='load'
        elif 'collect-scaling-inputs' in argv:operation='vectors'
        else:operation=argv[6]
        self.operations.append(operation);self.before(operation,argv)
        index=len(self.calls);self.calls.append((argv,kwargs))
        flag=lambda name:argv[argv.index('--'+name)+1]
        if operation=='generator':
            consume_seed_pipe(argv,kwargs['pass_fds'])
            root=Path(flag('out-dir'));self.anchors=emit(root,argv);raw=refresh(root,self.anchors);value=None
        elif operation=='readiness':value=self.ready(argv)
        elif operation=='load':
            paths=self._load_files(argv)
            value=dict(version=1,operation='transaction_load',invocation_id=flag('invocation-id'),
                pair_index=int(flag('pair-index')),variant=flag('variant'),seed=flag('seed'),
                resource_budget_sha256=flag('resource-budget-sha256'),scheduled_requests=self.trial._scheduled)
            for prefix,path in (('trace',paths.transaction_trace),('collector_journal',paths.collector_journal)):
                value[prefix+'_sha256']=hashlib.sha256(path.read_bytes()).hexdigest();value[prefix+'_bytes']=path.stat().st_size
        elif operation=='stopped-tip':
            path=Path(flag('signed-genesis'))
            value=dict(version=1,operation='stopped_tip',invocation_id=flag('invocation-id'),
                genesis_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),genesis_bytes=path.stat().st_size,
                committed_height=max(100,self.trial._scheduled+1))
        elif operation=='vectors':
            path=Path(flag('context'))
            value=dict(version=1,operation='collect_scaling_inputs',invocation_id=flag('invocation-id'),
                client_config_sha256=flag('client-config-sha256'),context_sha256=flag('context-sha256'),
                context_bytes=path.stat().st_size,committed_height=max(100,self.trial._scheduled+1),
                finality_count=max(100,self.trial._scheduled+1),query_count=self.trial._scheduled)
            for role in ('finality','queries'):
                digest,size=self._publish(role,Path(flag(role+'-out')));value[role+'_sha256']=digest;value[role+'_bytes']=size
        elif operation=='facts':
            digest,size=self._publish('facts',Path(flag('facts-output')))
            value=dict(version=1,operation='facts',invocation_id=flag('invocation-id'),facts_sha256=digest,facts_bytes=size)
        elif operation=='prepare':
            facts=self.trial._outputs.artifact('facts')
            value=dict(version=1,operation='prepare',invocation_id=flag('invocation-id'),facts_sha256=facts.sha256,facts_bytes=facts.bytes)
            for role in ('request','bundle'):
                digest,size=self._publish(role,Path(flag(role+'-output')));value[role+'_sha256']=digest;value[role+'_bytes']=size
        elif operation=='export':
            digest,size=self._publish('proof',Path(flag('output')))
            value=dict(version=1,operation='export',invocation_id=flag('invocation-id'),request_sha256=flag('request-sha256'),
                input_sha256=flag('input-sha256'),proof_sha256=digest,proof_iroha_hash='7'*64,proof_bytes=size)
        else:
            assert operation=='replay'
            out=self.trial._outputs;request,proof=out.artifact('request'),out.artifact('proof')
            binding=ReplayBindings(request.path,request.sha256,request.max_bytes,proof.path,proof.sha256,'7'*64,
                proof.bytes,proof.max_bytes,self.trial._plan.replay_reply_max_bytes)
            value,rows=projection(self.trial._proof_plan,binding,flag('invocation-id'));self.proof_rows=rows
        if value is not None:
            value=self.transform(operation,value)
            raw=wire(value,self.proof_rows) if operation=='replay' else compact(value)
        child=PipeChild(self,index,b'x',b'');child.payload=raw;self.children.append(child)
        self.after(operation,child);return child

    def sample(self,pid,image):
        return self.c.daemons.sample(pid,image) if pid<2000 else super().sample(pid,image)


@pytest.fixture
def trial_setup(ready_setup,monkeypatch,request):
    c=ready_setup;value=plan(getattr(request,'param',4));variant=value.load.variant
    public=c.root/'evidence';private=c.root/'runtime'
    for root in (public,private):root.mkdir(mode=0o700)
    paths=trial.TrialPaths(public,private,1,variant)
    for path in (paths.public,paths.private,paths.captures.parent):path.mkdir(mode=0o700,parents=True)
    created=[]
    for name in ('kagami','python3'):
        path=c.root/name;path.write_bytes(c.images[1].path.read_bytes());path.chmod(0o700)
        created.append(process.ExecutableImage(path,hashlib.sha256(path.read_bytes()).hexdigest()))
    worker=c.root/'resource_probe_worker.py';worker.write_bytes((Path(__file__).resolve().parents[2]/'scripts/nexus/resource_probe_worker.py').read_bytes());worker.chmod(0o644)
    runtime=trial.TrialRuntime(created[0],c.images[1],c.images[0],created[1],worker,hashlib.sha256(worker.read_bytes()).hexdigest())
    factory=TrialFactory(c);monkeypatch.setattr(command.subprocess,'Popen',factory)
    original=os.read
    def read(fd,count):
        for child in factory.children:
            if not child.stdout.closed and child.stdout.fileno()==fd:
                raw=child.payload[:count];child.payload=child.payload[count:];return raw
        return original(fd,count)
    monkeypatch.setattr(os,'read',read)
    checks=[];owners=[]
    kwargs=dict(plan=value,paths=paths,allocation=allocation(value),runtime=runtime,process_reader=factory,
        trial_deadline_ns=c.clock.end(600),verify_runtime=lambda:checks.append(c.clock.now))
    def create(**changes):
        owner=trial.FixedTrial(**(kwargs|changes));factory.trial=owner;owners.append(owner);return owner
    yield SimpleNamespace(c=c,plan=value,paths=paths,runtime=runtime,factory=factory,kwargs=kwargs,checks=checks,owners=owners,create=create)
    for owner in owners:
        try:owner.cleanup(c.clock.end(10));owner.close()
        except trial.FixedTrialError:pass
    for child in factory.children:child.close()
    for image in created:image.close()
