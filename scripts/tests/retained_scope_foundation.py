"""Reusable canonical scope foundations; process/source facts are synthetic."""
from pathlib import Path
import hashlib,io,os,tempfile,unittest
import private_settlement_registered_session_replay as replay
runner,control,accounting=replay.runner,replay.control,replay.accounting

class RegisteredScopeIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name).resolve();self.root.chmod(0o700)
        (self.root/'campaigns').mkdir(mode=0o700)
        self.commit='a'*40;self.harness={'sha256':'e'*64,'bytes':111}
        self.callback=replay.samples.RetainedSampleReplay(
            worker={'path':'/admitted/synthetic/worker','sha256':'f'*64,'bytes':222},
            validator={'path':'/admitted/synthetic/validator','sha256':'b'*64,'bytes':333},
            owner_uid=os.geteuid(),group_utility_sha256='c'*64,listener_utility_sha256='d'*64,
            packet_utility={key:value for key,value in replay.samples.semantics.packets._utility().items() if key in ('path','sha256')})


    def write(self,path,value):
        path.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
        path.write_bytes(runner.canonical_bytes(value));path.chmod(0o600)
        return runner.file_binding(path,relative_to=path.parents[0])


    def plan(self,root):
        hardware={'version':1,'protocol':control.PROTOCOL,'commit':self.commit,
            'collected_at_utc':'2026-09-12T00:00:00Z','physical_cores':4,'logical_cores':4,
            'memory_bytes':1024**3,'virtualized':False,'passed':True}
        for key in ('host_id','operating_system','kernel','architecture','cpu_model','storage_model',
                    'network_description','clock_policy','power_profile'):
            hardware[key]='explicit synthetic fixture'
        self.write(root/'hardware.json',hardware)
        hp=runner.release_evidence._validate_hardware_description(root/'hardware.json',commit=self.commit)
        canary=runner.build_canary_manifest(self.commit);self.write(root/'canaries.json',canary)
        refs=[];digests={}
        for n in runner.PARTICIPANTS:
            path=root/f'configurations/n{n}.json'
            self.write(path,runner.build_configuration(n,seeds=list(range(10)),warmups=5,measured=30))
            ref=runner.file_binding(path,relative_to=root);digests[n]=ref['sha256']
            refs.append({'participants':n,'validators_per_dataspace':4,'quorum':runner.QUORUM,
                         'mandatory_signed_rs16_da_rbc':True,**ref})
        self.write(root/'configuration-manifest.json',{'version':1,'protocol':control.PROTOCOL,
            'commit':self.commit,'configurations':refs,'passed':True})
        workloads=runner.publish_workload_manifests(root)
        policy=runner.benchmark_deadline_policy(runner.DEFAULT_HARNESS_TIMEOUT_SECONDS)
        plan={'version':1,'protocol':control.PROTOCOL,'commit':self.commit,'worktree_clean':True,
            'publication_evidence':False,'execution_required':True,'harness':self.harness,
            'harness_contract':runner.HARNESS_CONTRACT,'benchmark_accounting':policy,'benchmark_baseline':None,
            'hardware':{**runner.file_binding(root/'hardware.json',relative_to=root),'profile_sha256':hp},
            'canary_manifest':runner.file_binding(root/'canaries.json',relative_to=root),
            'configuration_manifest':runner.file_binding(root/'configuration-manifest.json',relative_to=root),
            'workload_manifests':workloads,
            'benchmark_sessions':runner.benchmark_session_plan(digests,list(range(10)),5,30)['sessions'],
            'jobs':runner.build_jobs(digests,list(range(10)),5,30,canary),
            'requirements':{'participants':list(runner.PARTICIPANTS),'primary_participants':runner.PRIMARY_PARTICIPANTS,
                'validators_per_dataspace':4,'quorum':runner.QUORUM,'seeds':list(range(10)),'warmups':5,'measured':30,
                'bootstrap_iterations':runner.MIN_BOOTSTRAP_ITERATIONS,'loss_phases':list(runner.fault_report.REQUIRED_LOSS_PHASES),
                'loss_percentages':list(runner.fault_report.REQUIRED_LOSS_PERCENTAGES),'phase_cuts':list(runner.fault_report.REQUIRED_PHASE_CUTS),
                'crash_boundaries':list(runner.fault_report.REQUIRED_CRASH_BOUNDARIES),'capture_surfaces':sorted(runner.SURFACE_FILES),
                'traffic_count_channels':list(runner.leakage_audit.REQUIRED_COUNT_CHANNELS)}}
        self.write(root/'frozen-plan.json',plan)
        return plan


    def materialize(self,count=1,prepare=False):
        self.plans={}
        slots=[]
        for i in range(count):
            name=f'campaign-{i}';root=self.root/'campaigns'/name;root.mkdir(mode=0o700)
            plan=self.plan(root);self.plans[name]=plan
            slots.append({'campaign_id':name,'plan':runner.file_binding(root/'frozen-plan.json')})
        self.scope={'version':1,'protocol':control.PROTOCOL,'scope_id':'2'*64,'previous_scope_sha256':None,
            'registered_ns':1,'stopping_policy':'fail_fast','deadline_policy':plan['benchmark_accounting'],'campaigns':slots}
        self.scope_path=self.root/'scope.json';self.write(self.scope_path,self.scope)
        digest=hashlib.sha256(self.scope_path.read_bytes()).hexdigest()
        for slot in slots:
            name=slot['campaign_id'];root=self.root/'campaigns'/name
            self.write(root/'registered-scope.json',self.scope)
            identity={'scope_sha256':digest,'campaign_id':name,'plan_sha256':slot['plan']['sha256']}
            self.write(root/'campaign-closure.json',{'version':1,'protocol':control.PROTOCOL,**identity,
                'closed_ns':2,'quiescent':True,'started_request_ids':[],'reason':'not_run',
                'started_session_ids':[],'session_closures':[]})
            if prepare:
                (root/'sessions').mkdir(mode=0o700);(root/'attempts').mkdir(mode=0o700)
                runner.prepare_benchmark_session(self.plans[name],root,root,identity,
                                                self.plans[name]['benchmark_sessions'][0])


    def invoke(self,**overrides):
        arguments={'plan_harness':self.harness,'callback':self.callback,'expected_commit':self.commit}
        arguments.update(overrides)
        return replay._replay_closed_scope(self.scope_path,**arguments)

    def accepted_sample_graph(self):
        """Compose real writers/reducers; every process/capture fact is synthetic."""
        self.materialize()
        root=self.root/'campaigns/campaign-0';plan=self.plans['campaign-0']
        (root/'sessions').mkdir(mode=0o700);(root/'attempts').mkdir(mode=0o700)
        base={'scope_sha256':hashlib.sha256(self.scope_path.read_bytes()).hexdigest(),
              'campaign_id':'campaign-0','plan_sha256':self.scope['campaigns'][0]['plan']['sha256']}
        prepared=runner.prepare_benchmark_session(plan,root,root,base,plan['benchmark_sessions'][0])
        # Use the shipped native-shaped schema builder; every process and
        # capture fact remains synthetic and is checked by the real reducers.
        from retained_native_fixture import SyntheticSampleBuilder
        records=control.RecordDirectory(root);self.addCleanup(records.close)
        images={'worker':{'path':'/exact/native/worker','sha256':'a'*64,'bytes':111},
                'validator':{'path':'/exact/native/validator','sha256':'b'*64,'bytes':222}}
        f=SyntheticSampleBuilder(prepared,prepared['request']['attempts'][0],records,images,
            0,100_000_000,None,plan['benchmark_accounting']['outer_timeout_ms'])
        arguments=dict(worker=images['worker'],validator=images['validator'],owner_uid=os.geteuid(),
            group_utility_sha256='c'*64,listener_utility_sha256='d'*64,
            packet_utility={key:value for key,value in f.observed_packet_utility.items() if key in ('path','sha256')})
        self.callback=replay.samples.RetainedSampleReplay(**arguments)
        def add_actual_start_fields(name,value):
            if name=='session_start':value['started_ns']=100_000_000
            if name=='worker_start':value['started_ns']=100_000_001
            if name=='durable':
                value.update(ordinal=int(Path(f.output).name.split('-')[0]),
                    session_started=f.records.locate(f.prefix+'/started.json'),preceding_acceptance=None)
        f.make_evidence(add_actual_start_fields)
        records=f.records;ref=records.locate;identity=f.identity;prefix=f.prefix
        started=ref(prefix+'/started.json');chains={}
        for channel,endpoints in control.CHANNEL_ENDPOINTS.items():
            for direction in control.DIRECTIONS:
                for observer in endpoints:
                    chains[observer,channel,direction]=control.ControlChain(identity,started['sha256'],channel,
                        direction,records,observer=observer,journal_prefix=prefix+'/control')
        def send(channel,direction,kind,payload,upstream=None):
            owner,child=control.CHANNEL_ENDPOINTS[channel]
            sender,receiver=(owner,child) if direction=='owner_to_child' else (child,owner)
            stream=io.BytesIO()
            message=chains[sender,channel,direction].send(stream,kind,payload,
                forwarded_from=None if upstream is None else upstream.binding)
            stream.seek(0);chains[receiver,channel,direction].receive(stream)
            return message
        ready=ref(prefix+'/ready.json');process_ready=ref(prefix+'/process-ready.json')
        upstream=send('adapter_worker','child_to_owner','ready',{'ready':ready})
        send('runner_adapter','child_to_owner','ready',{'ready':ready,'process_observation':process_ready},upstream)
        dispatch={**f.aid,'request':f.attempt['request'],'attempt_started':ref(f.output+'/started.json')}
        upstream=send('runner_adapter','owner_to_child','dispatch',dispatch)
        send('adapter_worker','owner_to_child','dispatch',dispatch,upstream)
        protocol=f.output+'/evidence/benchmark-protocol'
        for direction,kind,extra in (
            ('child_to_owner','measurement_ready',{}),
            ('owner_to_child','measurement_begin',{'process_observation':ref(f.output+'/process-observations/baseline.json')}),
            ('child_to_owner','measurement_finished',{}),
            ('owner_to_child','measurement_recorded',{'measurement_window':ref(f.output+'/measurement-window.json')})):
            marker='ready' if kind in {'measurement_ready','measurement_begin'} else 'finished'
            send('adapter_worker',direction,kind,{**f.aid,'marker':ref(protocol+'/measurement-'+marker+'.json'),**extra})
        rust=ref(protocol+'/rust-result.json')
        upstream=send('adapter_worker','child_to_owner','attempt_completed',{**f.aid,'rust_terminal':rust})
        send('runner_adapter','child_to_owner','attempt_completed',{**f.aid,'rust_terminal':rust,
            'adapter_outcome':ref(protocol+'/adapter-outcome.json'),'response':ref(f.output+'/response.json')},upstream)
        # Acceptance bytes are retained, but no ACK was delivered before shutdown.
        terminal=f.publish(prefix+'/worker-terminal.json',{**identity,'kind':'incomplete',
            'reason':'transport_interrupted','accepted_request_ids':[],'active_attempt_id':f.aid['attempt_id'],
            'last_owner_message_sha256':chains['worker','adapter_worker','owner_to_child'].previous,
            'last_worker_message_sha256':chains['worker','adapter_worker','child_to_owner'].previous,
            'network_shutdown_observed':True,'coordinator_reaped_observed':True})
        upstream=send('adapter_worker','child_to_owner','session_completed',{'worker_terminal':terminal})
        group={'process_group':f.worker,'members':[],'utility_sha256':'c'*64}
        pids=[f.worker]+[row['pid'] for row in f.ready['process_inventory']]
        lifecycle=f.publish(prefix+'/adapter-lifecycle.json',{**identity,'kind':'benchmark_session_worker_lifecycle',
            'worker_terminal':terminal,'worker_process_start':ref(prefix+'/worker-process-start.json'),
            'worker_pid':f.worker,'worker_exit_code':101,'worker_wait_completed':True,
            'worker_sha256':'a'*64,'worker_image_unchanged':True,
            'group_before':{**group,'observed_monotonic_ns':2000},
            'kernel_absences':[{'pid':pid,'kernel_absence_observed':True,'observed_monotonic_ns':2001+i}
                               for i,pid in enumerate(pids)],
            'group_after':{**group,'observed_monotonic_ns':3000}})
        send('runner_adapter','child_to_owner','session_completed',{'worker_terminal':terminal,
            'adapter_lifecycle':lifecycle},upstream)
        closure=f.publish(prefix+'/session-closure.json',{**identity,'session_started':started,
            'worker_terminal':terminal,'adapter_lifecycle':lifecycle,'ready':ready,
            'process_observation':process_ready,'closed_ns':10**15,'bindings_unchanged':True,
            'adapter_thread_joined':True})
        # Preserve the canonical fifty-fault prefix with explicit nonnative
        # process outcomes. Their inclusion tests dispatch completeness, never
        # fault/network qualification; no release result is asserted here.
        prefix_ids=[]
        for ordinal,job in enumerate(plan['jobs'],1):
            if job['kind']=='benchmark':break
            self.assertEqual(job['kind'],'fault')
            nonce=hashlib.sha256(('synthetic-fault-'+job['request_id']).encode()).hexdigest()
            request=runner.build_request(plan,root,{**job,'invocation_nonce':nonce})
            directory=root/f"attempts/{ordinal:05}-{job['request_id']}";directory.mkdir(mode=0o700)
            request_binding=self.write(directory/'request.json',request)
            identity_other={'version':1,'protocol':control.PROTOCOL,**base,'request_id':job['request_id'],
                'invocation_nonce':nonce,'attempt_id':accounting.registered_attempt_id(**base,request_id=job['request_id'])}
            self.write(directory/'started.json',{**identity_other,'command':['/synthetic/fault-harness'],
                'request':{key:request_binding[key] for key in ('sha256','bytes')},'harness':self.harness,
                'timeout_seconds':plan['benchmark_accounting']['outer_timeout_ms']//1000,'started_ns':10+ordinal*10})
            self.write(directory/'process-outcome.json',{**identity_other,'finished_ns':11+ordinal*10,
                'pid':8000+ordinal,'exit_code':0,'timed_out':False,'error':None,'passed':True,
                'retained_files':[],'completion_kind':'exited','elapsed_ms':1,
                'owned_process_group_gone':True,'bindings_unchanged':True})
            prefix_ids.append(job['request_id'])
        self.assertEqual(len(prefix_ids),50)
        self.write(root/'campaign-closure.json',{'version':1,'protocol':control.PROTOCOL,**base,
            'closed_ns':10**15+1,'quiescent':True,'started_request_ids':prefix_ids+[f.aid['request_id']],
            'reason':'fail_fast','started_session_ids':[identity['session_id']],'session_closures':[closure]})
        self.sample_fixture=f
