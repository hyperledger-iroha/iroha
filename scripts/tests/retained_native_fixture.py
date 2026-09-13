"""Synthetic native/process/capture records for retained schema tests only.

This factory executes no native worker or verifier. Every result is a fixture
fact, consumed by the actual strict Python reducers. Native Iroha and BPF
qualification remain separate requirements and cannot be inferred from tests.
"""
from __future__ import annotations
import copy,hashlib,json,os
from pathlib import Path
import private_settlement_sample_replay as replay
import private_settlement_session_control as control
import private_settlement_session_economics as economics
import private_settlement_session_semantics as semantics
import retained_packet_fixture as packet_fixture

class SyntheticSampleBuilder:
    """Generate the real record schema over an exclusively owned fixture graph."""
    def __init__(self,prepared,attempt,records,images,session_number,session_start,preceding_acceptance,budget):
        self.prepared,self.attempt,self.records=prepared,attempt,records
        self.root=records.path;self.identity=prepared['identity'];self.request=prepared['request']
        self.output=attempt['output_directory'];self.prefix='sessions/'+self.identity['session_id']
        self.images=images;self.session_number=session_number;self.session_start=session_start
        self.preceding_acceptance=preceding_acceptance
        self.worker=3000+session_number*100;self.parent=4000
        self.deadline=1000;self.budget=budget
        self.aid={key:attempt[key] for key in control.ATTEMPT_FIELDS}
        self.index=attempt['session_attempt_index'];self.wall_start=session_start+(self.index+1)*2_000_000_000
        self.verifier_pid=600000+session_number*100+self.index
        self.capture_pid=700000+session_number*100+self.index
        self.observed_packet_utility=semantics.packets._utility()
        coordinate=[self.request['participants'],self.request['seed'],self.index,self.index<self.request['warmups']]
        encoded=control.canonical({'synthetic_vector_coordinates':coordinate})
        self.vector_sha256=hashlib.sha256(encoded).hexdigest();self.vector_hex=encoded.hex()
        self._shifted={};self.change=self._change

    def _change(self,name,value):
        offset=self.session_number*1000000+(0 if name in {'ports','ready','session_start','worker_start','process_ready'} else (self.index+1)*1000)
        def walk(item):
            if isinstance(item,dict):
                if id(item) in self._shifted:return
                self._shifted[id(item)]=item
                for key,child in item.items():
                    if key.endswith('_monotonic_ns') and type(child) is int:item[key]=child+offset
                    elif key=='birth':
                        child['started_seconds']+=self.session_number
                        child['start_abstime']+=self.session_number*1000000
                    else:walk(child)
            elif isinstance(item,list):
                for child in item:walk(child)
        walk(value)
        if name=='session_start':value['started_ns']=self.session_start
        if name=='worker_start':value['started_ns']=self.session_start+1
        if name=='durable':
            value.update(started_ns=self.wall_start,outer_timeout_ms=self.budget,
                ordinal=int(Path(self.output).name.split('-')[0]),
                session_started=self.records.locate(self.prefix+'/started.json'),preceding_acceptance=self.preceding_acceptance)
        if name=='verification_launch':value['started_ns']=self.wall_start+1
        if name=='window':
            value['boundaries']={key:child+offset for key,child in value['boundaries'].items()}

    def publish(self,path,value,name=None,*,measurement=False):
        self.change(name or path,value)
        raw=semantics.measurement_bytes(value) if measurement else control.canonical(value)
        return self.records.publish(path,raw)

    def observation(self, start, cpu, rss=100):
        if start >= 20: cpu += self.index*1000
        rows = []
        for declaration in self.declarations:
            image = self.images[declaration['image']]
            rows.append({'label': declaration['label'], 'identity': {
                **{key: declaration[key] for key in ('pid', 'ppid', 'pgid')}, 'uid': os.geteuid(),
                'birth': {'kind': 'darwin_bsdinfo', 'started_seconds': 1,
                          'started_microseconds': 2, 'start_abstime': declaration['pid']},
                'loaded_image_uuid': '1'*32, 'executable_path': image['path'], 'executable_sha256': image['sha256']},
                'cpu_time_ns': cpu, 'cpu_counter_unit_ns': 1, 'rss_bytes': rss})
        return {'started_monotonic_ns': start, 'finished_monotonic_ns': start+1, 'processes': rows,
                'cpu_time_ns': len(rows)*cpu, 'rss_bytes': len(rows)*rss}

    def listener(self, start):
        pids = tuple(sorted({row['pid'] for row in self.endpoints}))
        raw = b''
        for pid in pids:
            raw += f'p{pid}\0\n'.encode()
            for fd, row in enumerate(r for r in self.endpoints if r['pid'] == pid):
                raw += f'f{fd}\0tIPv4\0PTCP\0n127.0.0.1:{row["port"]}\0TST=LISTEN\0\n'.encode()
        return {'process_before': self.observation(start, 104 if start >= 160 else 2), 'process_after': self.observation(start+3, 104 if start >= 160 else 2),
            'expected_endpoints': self.endpoints,
            'scope': 'kernel listener attribution only; packet completeness and byte totals unmeasured',
            'listener_observation': {'started_monotonic_ns': start+1, 'finished_monotonic_ns': start+2,
                'command': [str(replay.network.LSOF), '-nP', '-a', '-p', ','.join(map(str,pids)),
                            '-iTCP', '-sTCP:LISTEN', '-F0pftPnT'], 'utility_sha256': 'd'*64,
                'raw_utf8': raw.decode('ascii'), 'raw_sha256': hashlib.sha256(raw).hexdigest(),
                'listeners': replay.network.parse_listener_output(raw,pids)}}

    def make_evidence(self, mutate=None):
        if mutate is not None: self.change = mutate
        raw_request = self.records.read(self.attempt['request']); single = control.decode(raw_request)
        inventory = []
        coordinates = [('coordinator', None, None)] + [('global_validator', None, i) for i in range(4)]
        coordinates += [('dataspace_validator', n, i) for n in range(single['participants']) for i in range(4)]
        for index, (role, n, v) in enumerate(coordinates):
            inventory.append({'role':role,'dataspace_ordinal':n,'validator_ordinal':v,'pid':5000+self.session_number*100+index,
                'executable_sha256':self.images['worker' if role=='coordinator' else 'validator']['sha256'],
                'revision':single['commit'],'health_observed':True})
        self.declarations = replay.process.benchmark_process_declarations(inventory, participants=single['participants'],
            worker_pid=self.worker,adapter_pid=self.parent,process_group=self.worker,commit=single['commit'],
            worker_sha256=self.images['worker']['sha256'],validator_sha256=self.images['validator']['sha256'])
        self.endpoints, peers = [], []
        groups={'torii':[],'public_p2p':[],'restricted_p2p':[]}
        for index,row in enumerate(inventory[1:]):
            visibility='public' if index<8 else 'restricted'
            torii={'address':'127.0.0.1','transport':'tcp','port':30001+index*2}
            p2p={'address':'127.0.0.1','transport':'tcp','port':30002+index*2,'visibility':visibility}
            peers.append({**{key:row[key] for key in ('pid','role','dataspace_ordinal','validator_ordinal')},
                          'peer_index':index,'torii':torii,'p2p':p2p})
            self.endpoints += [{'pid':row['pid'],**torii}, {'pid':row['pid'],**{k:p2p[k] for k in ('address','transport','port')}}]
            groups['torii'].append(torii['port']);groups[visibility+'_p2p'].append(p2p['port'])
        groups=semantics.packets.split.canonical_port_manifest_document(groups)
        ports=self.publish(self.prefix+'/network-ports.json', {**self.identity,'kind':'benchmark_network_ports',
            'network_id':'synthetic-retained-'+str(self.session_number),'participants':single['participants'],'groups':groups,'peers':peers},'ports')
        self.ready={**self.identity,'network_id':'synthetic-retained-'+str(self.session_number),'genesis_sha256':'f'*64,
            'configuration_sha256':single['configuration_sha256'],'workload_manifest_sha256':single['workload_manifest_sha256'],
            'activated_height':301,'process_inventory':inventory,'worker_pid':self.worker,'network_ports':ports}
        ready_ref=self.publish(self.prefix+'/ready.json',self.ready,'ready')
        command=[self.images['worker']['path'],replay.adapter.WORKER_TEST,'--exact','--ignored','--nocapture','--test-threads=1']
        self.publish(self.prefix+'/started.json',{**self.identity,'request':self.prepared['reference'],
            'command':command,'harness':{'sha256':self.images['worker']['sha256'],'bytes':self.images['worker']['bytes']},'started_ns':1},'session_start')
        self.publish(self.prefix+'/worker-process-start.json',{'command':command,'pid':self.worker,'parent_pid':self.parent,
            'process_group':self.worker,'worker_sha256':self.images['worker']['sha256'],'started_ns':2},'worker_start')
        self.publish(self.prefix+'/process-ready.json',{**self.identity,'kind':'benchmark_session_process_ready',
            'ready':ready_ref,'network_ports':ports,'process_scope':self.observation(10,1),'listeners':self.listener(12)},'process_ready')
        self.publish(self.output+'/started.json',{**self.identity,**self.aid,'request':self.attempt['request'],
            'outer_timeout_ms':self.budget,'started_ns':1_000_000_000},'durable')
        workload={'economic_vector_sha256':self.vector_sha256,'canonical_economic_vector_hex':self.vector_hex}
        workload_ref=self.publish(self.output+'/evidence/matched-workload.json',workload,'workload')
        inputs=economics.prepare_verification(self.prepared,self.attempt,records=self.records)
        vector={**self.identity,**self.aid,**{key:inputs['document'][key] for key in economics.INPUT_NAMES},
            'kind':'benchmark_economic_vector_verified','verified':True,'request_sha256':self.attempt['request']['sha256'],
            'network_id':self.ready['network_id'],'workload_manifest_sha256':single['workload_manifest_sha256'],
            'economic_vector_sha256':workload['economic_vector_sha256'],
            'canonical_economic_vector_sha256':hashlib.sha256(bytes.fromhex(workload['canonical_economic_vector_hex'])).hexdigest(),
            'primary_payment_count':single['participants'],'monetary_movement_count':single['participants']+1,
            'verification_request':inputs['reference']}
        native_prefix=str(Path(inputs['output_path']).parent)+'/native-vector-process'
        launch=self.publish(native_prefix+'-launch.json',{'verification_request':inputs['reference'],
            'command':[self.images['worker']['path'],economics.VERIFIER_TEST,'--exact','--ignored','--nocapture','--test-threads=1'],
            'executable_sha256':self.images['worker']['sha256'],'started_ns':1_000_000_001,'started_monotonic_ns':500,
            'deadline_monotonic_ns':self.deadline},'verification_launch')
        native_start=self.publish(native_prefix+'-start.json',{'launch':launch,'pid':self.verifier_pid,'parent_pid':self.parent,
            'process_group':self.verifier_pid,'spawned_monotonic_ns':510},'verification_start')
        native_exit=self.publish(native_prefix+'-exit.json',{'launch':launch,'pid':self.verifier_pid,'exit_code':0,
            'natural_wait_observed':True,'observed_monotonic_ns':520},'verification_exit')
        stdout=('running 1 test\ntest '+economics.VERIFIER_TEST+' ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; '
                '0 measured; 4 filtered out; finished in 0.01s\n').encode()
        logs=self.publish(native_prefix+'-logs.json',{'stdout_hex':stdout.hex(),'stderr_hex':''},'verification_logs')
        group={'process_group':self.verifier_pid,'members':[],'utility_sha256':'c'*64}
        terminal=self.publish(native_prefix+'-terminal.json',{'launch':launch,'process_start':native_start,
            'exit_observation':native_exit,'pid':self.verifier_pid,'exit_code':0,'natural_wait_observed':True,
            'finished_monotonic_ns':560,'group_before':{**group,'observed_monotonic_ns':530},
            'kernel_absence':{'pid':self.verifier_pid,'kernel_absence_observed':True,'observed_monotonic_ns':540},
            'group_after':{**group,'observed_monotonic_ns':550},'logs':logs},'verification_terminal')
        vector_ref=self.publish(inputs['output_path'],vector,'vector')
        execution=self.publish(native_prefix+'-verified.json',{'kind':'benchmark_native_economic_verification_execution',
            'verification_request':inputs['reference'],'native_result':vector_ref,'process_terminal':terminal,
            'executable_sha256':self.images['worker']['sha256'],'executed_native_verifier':True},'verification')
        protocol=self.output+'/evidence/benchmark-protocol'
        ready_marker=self.publish(protocol+'/measurement-ready.json',{**self.identity,**self.aid,'boundary':'ready'},'ready_marker')
        finished_marker=self.publish(protocol+'/measurement-finished.json',{**self.identity,**self.aid,'boundary':'finished'},'finished_marker')
        for path in (self.output+'/process-observations',self.output+'/packet-capture'):
            (self.root/path).mkdir(mode=0o700)
        before=self.publish(self.output+'/listeners-before.json',self.listener(20),'listener_before')
        after=self.publish(self.output+'/listeners-after.json',self.listener(160),'listener_after')
        baseline=self.observation(40,3); final=self.observation(140,103,200)
        self.change('resource_baseline',baseline);self.change('resource_final',final)
        baseline_ref=self.publish(self.output+'/process-observations/baseline.json',baseline)
        journal=replay.process.ResourceObservationJournal(self.records,self.output+'/process-observations',
                                                          replay.process.resource_window_policy(self.budget,100))
        journal.append(baseline);journal.append(final);journal.flush()
        count=len(self.declarations)
        metrics={'cpu_time_ns':count*100,'sampled_peak_rss_bytes':count*200,'maximum_observation_gap_ns':99,
            'baseline_started_monotonic_ns':40,'baseline_finished_monotonic_ns':41,
            'final_started_monotonic_ns':140,'final_finished_monotonic_ns':141}
        resource={'version':1,'kind':'benchmark_process_resource_window','baseline':baseline_ref,
            'sampler_stopped_observed':True,'journal':journal.manifest(),'outcome':{'kind':'succeeded',**metrics}}
        resource_ref=self.publish(self.output+'/process-window.json',resource,'resource')
        pp=self.output+'/packet-capture';marker_port=49000;nonce=b'x'*32
        capstart=self.publish(pp+'/capture-start.json',{**self.identity,**self.aid,
            'command':[str(semantics.packets.TCPDUMP),'-n','-U','-i','lo0','-s','0','-w','-',
                       semantics.packets.capture_filter(groups,marker_port)],'utility':copy.deepcopy(self.observed_packet_utility),
            'references':{'ready_marker':ready_marker,'ports':ports,'listener_before':before,'resource_baseline':resource['baseline']},
            'marker_port':marker_port,'nonce_hex':nonce.hex(),'deadline_monotonic_ns':self.deadline,'started_monotonic_ns':42},'packet_start')
        capprocess=self.publish(pp+'/capture-process.json',{'start':capstart,'pid':self.capture_pid,'parent_pid':self.parent,
            'spawned_monotonic_ns':43},'packet_process')
        begin,end=[semantics.packets.marker_payload(self.identity,self.aid,nonce,n) for n in (0,1)]
        raw=packet_fixture.pcap([packet_fixture.ip_packet(begin,source=marker_port,destination=marker_port,udp=True),
            packet_fixture.ip_packet(b'abc',source=30001),packet_fixture.ip_packet(end,source=marker_port,destination=marker_port,udp=True)])
        raw_holder={'raw':raw};self.change('pcap',raw_holder);raw=raw_holder['raw']
        chunk=self.records.publish(pp+'/capture-000000.pcap-part',raw)
        stderr=self.publish(pp+'/capture-stderr.json',{'raw_hex':packet_fixture.stats(3).hex()},'packet_stats')
        packet_metrics=semantics.packets.reduce_chunks([raw],groups=groups,marker_port=marker_port,
            begin_payload=begin,end_payload=end,statistics=packet_fixture.stats(3))
        packet_value={**self.identity,**self.aid,'kind':'benchmark_packet_window','start':capstart,'process':capprocess,
            'chunks':[chunk],'stderr':stderr,'terminal':{'exit_code':0,'pid':self.capture_pid,'reader_stopped':True,'finished_monotonic_ns':150},
            'events':[{'phase':0,'send_started_monotonic_ns':44,'send_finished_monotonic_ns':45,'observed_monotonic_ns':46},
                      {'phase':1,'send_started_monotonic_ns':130,'send_finished_monotonic_ns':131,'observed_monotonic_ns':132}],
            'finished_marker':finished_marker,'listener_after':after,'outcome':{'kind':'succeeded',**packet_metrics,
                'resource_offsets':{'baseline_to_start_marker_ns':3,'end_marker_to_final_counter_ns':10}}}
        packet_ref=self.publish(pp+'/packet-window.json',packet_value,'packet')
        window_ref=self.publish(self.output+'/measurement-window.json',{**self.identity,**self.aid,
            'kind':'benchmark_measurement_window','ready_marker':ready_marker,'finished_marker':finished_marker,
            'process_resource_window':resource_ref,'packet_window':packet_ref,'listener_before':before,'listener_after':after,
            'boundaries':{'ready_received_ns':30,'baseline_published_ns':41,'begin_written_ns':47,
                'finished_received_ns':129,'packet_end_returned_ns':133,'resources_finished_ns':142}},'window')
        payload={'economic_vector_sha256':self.vector_sha256,'primary_payment_count':single['participants'],
            'monetary_movement_count':single['participants']+1,'successful_leg_applications':single['participants'],
            'stages_ms':{stage:10 if stage=='end_to_end' else 1.25 for stage in single['payload']['stages']},
            'proof_bytes':100 if single['payload']['profile']=='private' else 0,'receipt_bytes':200,'storage_growth_bytes':300,'finalized_receipt_observed':True,
            'each_leg_applied_exactly_once':True,'partial_visible_observations':0,'partial_spendable_observations':0}
        native={**{key:single[key] for key in ('version','protocol','request_id','invocation_nonce','commit','participants')},
            'request_sha256':self.attempt['request']['sha256'],'mandatory_signed_rs16_da_rbc':True,
            'authenticated_message_control':True,'signed_rs16_da_observations':semantics.runner.minimum_signed_rs16_da_observations(single['participants']),
            'process_inventory':inventory,'payload':payload}
        rust_ref=self.publish(protocol+'/rust-result.json',{**{key:native[key] for key in ('version','protocol','request_id',
            'invocation_nonce','commit','participants','request_sha256')},'elapsed_ms':100,
            'outcome':{'kind':'succeeded','result':native}},'rust_terminal',measurement=True)
        environment={key:single[key] for key in ('commit','hardware_sha256','hardware_profile_sha256','configuration_sha256',
                                               'participants','seed','workload_manifest_sha256')}
        provenance={'rust_terminal':rust_ref,'measurement_window':window_ref,'native_economic_verification':execution,
                    'request_sha256':self.attempt['request']['sha256']}
        measured={'stages_ms':payload['stages_ms'],'throughput_bundles_per_second':100.0,'cpu_seconds':count*100/1e9,
            'peak_rss_bytes':count*200,'network_bytes':43,'proof_bytes':100 if single['payload']['profile']=='private' else 0,'receipt_bytes':200,'storage_growth_bytes':300}
        sample={**self.identity,**self.aid,**environment,**provenance,**measured,'profile':single['payload']['profile'],
            'warmup':single['payload']['warmup'],'economic_vector_sha256':self.vector_sha256,'primary_payment_count':single['participants'],
            'monetary_movement_count':single['participants']+1,'throughput_basis':'serial_completed_bundle_elapsed',
            'network_counting_unit':'ipv4_packet_bytes_including_ip_tcp_headers','rss_observation':'sampled_aggregate_peak'}
        response={**self.identity,**self.aid,**environment,**provenance,'kind':'benchmark','passed':True,
            **{key:native[key] for key in ('mandatory_signed_rs16_da_rbc','signed_rs16_da_observations',
                                          'authenticated_message_control','process_inventory')},'payload':{**payload,**measured}}
        response_ref=self.publish(self.output+'/response.json',response,'response',measurement=True)
        sample_ref=self.publish(self.output+'/benchmark-sample.json',sample,'sample',measurement=True)
        outcome_ref=self.publish(protocol+'/adapter-outcome.json',{**self.identity,**self.aid,
            'request_sha256':self.attempt['request']['sha256'],'rust_terminal':rust_ref,
            'kind':'benchmark_session_attempt_validation','status':'succeeded','measurement_window':window_ref,
            'native_economic_verification':execution,'response':response_ref},'adapter_outcome')
        validation_ref=self.publish(self.output+'/validation-outcome.json',{**self.identity,**self.aid,
            'passed':True,'validation_kind':'accepted','response':response_ref,'sample':sample_ref},'validation')
        self.bound={key:self.records.read(ref) for key,ref in {'rust_terminal':rust_ref,'adapter_outcome':outcome_ref,
                    'response':response_ref,'sample':sample_ref,'validation':validation_ref}.items()}
