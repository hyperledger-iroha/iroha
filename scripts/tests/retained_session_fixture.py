"""Complete retained-session protocol fixture; all process/capture facts are synthetic."""
from __future__ import annotations

import hashlib
import io
from pathlib import Path

import private_settlement_release_runner as runner
import private_settlement_session_control as control
import private_settlement_attempt_accounting as accounting
from retained_native_fixture import SyntheticSampleBuilder


class FixtureRecords:
    """Buffer only synthetic test records, then write independent real files.

Production uses the descriptor-owned RecordDirectory. This fixture supplies
generated inputs, never admission or a validation result. Replay always uses
the real immutable filesystem provider after materialization.
"""
    def __init__(self,path):self.path=Path(path);self.values={}

    def read(self,reference):
        control.reference(reference)
        value=self.values.get(reference['path'])
        if value is None:value=(self.path/reference['path']).read_bytes()
        assert len(value)==reference['bytes'] and hashlib.sha256(value).hexdigest()==reference['sha256']
        return value

    def locate(self,path):
        value=self.values.get(path)
        if value is None:value=(self.path/path).read_bytes()
        return {'path':path,'sha256':hashlib.sha256(value).hexdigest(),'bytes':len(value)}

    def publish(self,path,raw):
        assert type(raw) is bytes and raw
        if path in self.values:
            assert self.values[path]==raw, 'fixture changed an immutable shared session record: '+path
        else:
            assert not (self.path/path).exists(), 'fixture overwrites an existing file: '+path
            self.values[path]=raw
        return self.locate(path)

    def flush(self):
        for path,raw in self.values.items():
            target=self.path/path;target.parent.mkdir(parents=True,mode=0o700,exist_ok=True)
            with target.open('xb') as output:output.write(raw)
            target.chmod(0o600)
        self.values.clear()


class SessionFixture:
    """Create the ordered owner, attempt, measurement and acceptance chains."""
    def __init__(self,prepared,root,images,session_number,started_ns,budget):
        self.prepared,self.records,self.images=prepared,FixtureRecords(root),images
        self.number,self.started_ns,self.budget=session_number,started_ns,budget
        self.identity=prepared['identity'];self.prefix='sessions/'+self.identity['session_id']
        self.chains={};self.accepted=[];self.samples=[];self.previous=None;self.started_ids=[]
        self.last=None

    def send(self,channel,direction,kind,payload,upstream=None):
        owner,child=control.CHANNEL_ENDPOINTS[channel]
        sender,receiver=(owner,child) if direction=='owner_to_child' else (child,owner)
        stream=io.BytesIO()
        message=self.chains[sender,channel,direction].send(stream,kind,payload,
            forwarded_from=None if upstream is None else upstream.binding)
        stream.seek(0);self.chains[receiver,channel,direction].receive(stream)
        return message

    def initialize_chains(self):
        start=self.records.locate(self.prefix+'/started.json')
        for channel,endpoints in control.CHANNEL_ENDPOINTS.items():
            for direction in control.DIRECTIONS:
                for observer in endpoints:
                    self.chains[observer,channel,direction]=control.ControlChain(self.identity,start['sha256'],channel,
                        direction,self.records,observer=observer,journal_prefix=self.prefix+'/control')
        ready=self.records.locate(self.prefix+'/ready.json')
        upstream=self.send('adapter_worker','child_to_owner','ready',{'ready':ready})
        self.send('runner_adapter','child_to_owner','ready',{'ready':ready,
            'process_observation':self.records.locate(self.prefix+'/process-ready.json')},upstream)

    def success(self,index):
        attempt=self.prepared['request']['attempts'][index]
        f=SyntheticSampleBuilder(self.prepared,attempt,self.records,self.images,self.number,self.started_ns,self.previous,self.budget)
        f.make_evidence();self.last=f
        if index==0:self.initialize_chains()
        self.dispatch(f)
        ref=self.records.locate;protocol=f.output+'/evidence/benchmark-protocol'
        for direction,kind,extra in (
            ('child_to_owner','measurement_ready',{}),
            ('owner_to_child','measurement_begin',{'process_observation':ref(f.output+'/process-observations/baseline.json')}),
            ('child_to_owner','measurement_finished',{}),
            ('owner_to_child','measurement_recorded',{'measurement_window':ref(f.output+'/measurement-window.json')})):
            marker='ready' if kind in {'measurement_ready','measurement_begin'} else 'finished'
            self.send('adapter_worker',direction,kind,{**f.aid,'marker':ref(protocol+'/measurement-'+marker+'.json'),**extra})
        rust=ref(protocol+'/rust-result.json')
        upstream=self.send('adapter_worker','child_to_owner','attempt_completed',{**f.aid,'rust_terminal':rust})
        completed={**f.aid,'rust_terminal':rust,'adapter_outcome':ref(protocol+'/adapter-outcome.json'),'response':ref(f.output+'/response.json')}
        self.send('runner_adapter','child_to_owner','attempt_completed',completed,upstream)
        acceptance={**completed,'validation':ref(f.output+'/validation-outcome.json'),'sample':ref(f.output+'/benchmark-sample.json')}
        acknowledged=self.send('runner_adapter','owner_to_child','accept',acceptance)
        for phase in ('proposed','validated','pipe-written'):
            self.records.publish(f'{self.prefix}/control/ack-{index:06d}-{phase}.json',control.canonical({
                **self.identity,**f.aid,'acknowledgement':acknowledged.binding,'phase':phase}))
        self.send('adapter_worker','owner_to_child','accept',acceptance,acknowledged)
        self.previous=acknowledged.binding;self.accepted.append(attempt['request_id']);self.samples.append(f.bound['sample'])

    def dispatch(self,f):
        payload={**f.aid,'request':f.attempt['request'],'attempt_started':self.records.locate(f.output+'/started.json')}
        upstream=self.send('runner_adapter','owner_to_child','dispatch',payload)
        self.send('adapter_worker','owner_to_child','dispatch',payload,upstream)
        self.started_ids.append(f.attempt['request_id'])

    def failure(self,index):
        assert self.last is not None,'fixture failure requires an established network'
        attempt=self.prepared['request']['attempts'][index]
        f=SyntheticSampleBuilder(self.prepared,attempt,self.records,self.images,self.number,self.started_ns,self.previous,self.budget)
        request=control.decode(self.records.read(attempt['request']));self.last=f
        f.publish(f.output+'/started.json',{**f.identity,**f.aid,'request':attempt['request'],
            'outer_timeout_ms':self.budget,'started_ns':f.wall_start},'durable')
        self.dispatch(f)
        rust=f.publish(f.output+'/evidence/benchmark-protocol/rust-result.json',{
            **{key:request[key] for key in ('version','protocol','request_id','invocation_nonce','commit','participants')},
            'request_sha256':attempt['request']['sha256'],'elapsed_ms':10,
            'outcome':{'kind':'failed','stage':'benchmark_worker','reason':'execution_error'}},'rust_terminal')
        adapter=f.publish(f.output+'/evidence/benchmark-protocol/adapter-outcome.json',{
            **f.identity,**f.aid,'request_sha256':attempt['request']['sha256'],'rust_terminal':rust,
            'kind':'benchmark_session_attempt_validation','status':'failed','measurement_window':None,
            'native_economic_verification':None,'response':None},'adapter_outcome')
        upstream=self.send('adapter_worker','child_to_owner','attempt_completed',{**f.aid,'rust_terminal':rust})
        self.send('runner_adapter','child_to_owner','attempt_completed',{**f.aid,'rust_terminal':rust,'adapter_outcome':adapter,'response':None},upstream)

    def close(self,*,failed=False):
        ref=self.records.locate;identity=self.identity;prefix=self.prefix
        f=self.last;assert f is not None
        ready=control.decode(self.records.read(ref(prefix+'/ready.json')))
        terminal=self.records.publish(prefix+'/worker-terminal.json',control.canonical({**identity,
            'kind':'failed' if failed else 'completed','reason':'attempt_failed' if failed else None,
            'accepted_request_ids':self.accepted,'active_attempt_id':f.aid['attempt_id'] if failed else None,
            'last_owner_message_sha256':self.chains['worker','adapter_worker','owner_to_child'].previous,
            'last_worker_message_sha256':self.chains['worker','adapter_worker','child_to_owner'].previous,
            'network_shutdown_observed':True,'coordinator_reaped_observed':True}))
        upstream=self.send('adapter_worker','child_to_owner','session_completed',{'worker_terminal':terminal})
        group={'process_group':f.worker,'members':[],'utility_sha256':'c'*64}
        tick=self.number*1000000+(f.index+2)*1000+2000
        pids=[f.worker]+[row['pid'] for row in ready['process_inventory']]
        lifecycle=self.records.publish(prefix+'/adapter-lifecycle.json',control.canonical({**identity,
            'kind':'benchmark_session_worker_lifecycle','worker_terminal':terminal,
            'worker_process_start':ref(prefix+'/worker-process-start.json'),'worker_pid':f.worker,
            'worker_exit_code':101 if failed else 0,'worker_wait_completed':True,
            'worker_sha256':self.images['worker']['sha256'],'worker_image_unchanged':True,
            'group_before':{**group,'observed_monotonic_ns':tick},
            'kernel_absences':[{'pid':pid,'kernel_absence_observed':True,'observed_monotonic_ns':tick+1+i} for i,pid in enumerate(pids)],
            'group_after':{**group,'observed_monotonic_ns':tick+1000}}))
        self.send('runner_adapter','child_to_owner','session_completed',{'worker_terminal':terminal,'adapter_lifecycle':lifecycle},upstream)
        closed_ns=self.started_ns+(f.index+3)*2_000_000_000
        closure=self.records.publish(prefix+'/session-closure.json',control.canonical({**identity,
            'session_started':ref(prefix+'/started.json'),'worker_terminal':terminal,'adapter_lifecycle':lifecycle,
            'ready':ref(prefix+'/ready.json'),'process_observation':ref(prefix+'/process-ready.json'),
            'closed_ns':closed_ns,'bindings_unchanged':True,'adapter_thread_joined':True}))
        self.records.flush()
        return closure,closed_ns


def nonbenchmark_records(root,plan,base,ordinal,job,started_ns):
    """Retain explicit synthetic process ownership for a real full-plan job."""
    nonce=hashlib.sha256(control.canonical(['synthetic-nonbenchmark',base,ordinal])).hexdigest()
    request=runner.build_request(plan,root,{**job,'invocation_nonce':nonce})
    prefix=f"attempts/{ordinal:05}-{job['request_id']}"
    records=FixtureRecords(root);request_ref=records.publish(prefix+'/request.json',runner.canonical_bytes(request))
    identity={'version':1,'protocol':control.PROTOCOL,**base,'request_id':job['request_id'],
        'invocation_nonce':nonce,'attempt_id':accounting.registered_attempt_id(**base,request_id=job['request_id'])}
    records.publish(prefix+'/started.json',control.canonical({**identity,'command':['/synthetic/fault-harness'],
        'request':{key:request_ref[key] for key in ('sha256','bytes')},'harness':plan['harness'],
        'timeout_seconds':plan['benchmark_accounting']['outer_timeout_ms']//1000,'started_ns':started_ns}))
    records.publish(prefix+'/process-outcome.json',control.canonical({**identity,'finished_ns':started_ns+1,
        'pid':8000+ordinal,'exit_code':0,'timed_out':False,'error':None,'passed':True,'retained_files':[],
        'completion_kind':'exited','elapsed_ms':1,'owned_process_group_gone':True,'bindings_unchanged':True}))
    records.flush()
    return job['request_id']
