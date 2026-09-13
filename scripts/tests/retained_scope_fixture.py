"""Generate a failed predecessor and complete retained-session scope for tests.

Every process, native result and capture is explicitly synthetic fixture data.
The real collector, accounting, sample, archive and statistics validators read
the final filesystem. No successful replay here qualifies a native benchmark.
"""
from __future__ import annotations
import hashlib,json
from pathlib import Path

import private_settlement_registered_session_replay as replay
import private_settlement_retained_scope_publication as publication
from retained_session_fixture import SessionFixture,nonbenchmark_records

RUNNER=replay.runner
SEEDS=tuple(range(10));WARMUPS=5;MEASURED=30


def raw(document):
    return (json.dumps(document,ensure_ascii=False,sort_keys=True,allow_nan=False)+'\n').encode()


def canonical_configuration_inputs(commit):
    paths={n:Path('evidence/configurations')/f'private-settlement-n{n}.json' for n in RUNNER.PARTICIPANTS}
    payloads={n:(json.dumps(RUNNER.build_configuration(n,seeds=SEEDS,warmups=WARMUPS,measured=MEASURED), ensure_ascii=False,indent=2,sort_keys=True)+'\n').encode() for n in RUNNER.PARTICIPANTS}
    manifest_path=Path('evidence/configuration_manifest.json')
    manifest={'version':1,'protocol':replay.control.PROTOCOL,'commit':commit,'passed':True,
        'configurations':[{'participants':n,'validators_per_dataspace':4,'quorum':RUNNER.QUORUM,
            'mandatory_signed_rs16_da_rbc':True,'path':str(paths[n]),**replay.accounting.accounting_file_binding(payloads[n])} for n in RUNNER.PARTICIPANTS]}
    return paths,payloads,manifest_path,raw(manifest)


def build_scope(root,*,commit,hardware_path,hardware_payload,configuration_manifest_path,
                configuration_manifest_payload,configuration_payloads,images,plan_harness,
                include_failed_predecessor=True,complete_session_count=None):
    root=Path(root).resolve(strict=True);root.chmod(0o700)
    accounting_root=root/'accounting';accounting_root.mkdir(mode=0o700)
    (accounting_root/'campaigns').mkdir(mode=0o700)
    def put(path,value):
        value=value if type(value) is bytes else raw(value)
        path.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
        with path.open('xb') as output:output.write(value)
        path.chmod(0o600)
        # Aggregate inventory follows the release manifest bound. Referenced
        # protocol records retain their own smaller decoder bounds.
        return {'sha256':hashlib.sha256(value).hexdigest(),'bytes':len(value)}
    reference=lambda path,value:{'path':str(path),**replay.accounting.accounting_file_binding(value)}
    configurations={item['participants']:item['sha256'] for item in json.loads(configuration_manifest_payload)['configurations']}
    canaries=RUNNER.build_canary_manifest(commit);canary_path=Path('evidence/registered-canaries.json');canary_payload=raw(canaries)
    policies={Path(f'workloads/n{n}.json'):replay.control.canonical(replay.accounting.build_benchmark_workload_policy(n)) for n in RUNNER.PARTICIPANTS}
    plan={'version':1,'protocol':replay.control.PROTOCOL,'commit':commit,'worktree_clean':True,'publication_evidence':False,
        'execution_required':True,'harness':plan_harness,'harness_contract':dict(RUNNER.HARNESS_CONTRACT),'benchmark_baseline':None,
        'benchmark_accounting':RUNNER.benchmark_deadline_policy(RUNNER.DEFAULT_HARNESS_TIMEOUT_SECONDS),
        'hardware':{**reference(hardware_path,hardware_payload),'profile_sha256':RUNNER.release_evidence._hardware_profile_sha256(json.loads(hardware_payload))},
        'canary_manifest':reference(canary_path,canary_payload),
        'configuration_manifest':reference(configuration_manifest_path,configuration_manifest_payload),
        'workload_manifests':[{'participants':n,**reference(Path(f'workloads/n{n}.json'),policies[Path(f'workloads/n{n}.json')])} for n in RUNNER.PARTICIPANTS],
        'benchmark_sessions':RUNNER.benchmark_session_plan(configurations,SEEDS,WARMUPS,MEASURED)['sessions'],
        'jobs':RUNNER.build_jobs(configurations,SEEDS,WARMUPS,MEASURED,canaries),
        'requirements':{'participants':list(RUNNER.PARTICIPANTS),'primary_participants':RUNNER.PRIMARY_PARTICIPANTS,
            'validators_per_dataspace':4,'quorum':RUNNER.QUORUM,'seeds':list(SEEDS),'warmups':WARMUPS,'measured':MEASURED,
            'bootstrap_iterations':100,'loss_phases':list(RUNNER.fault_report.REQUIRED_LOSS_PHASES),
            'loss_percentages':list(RUNNER.fault_report.REQUIRED_LOSS_PERCENTAGES),'phase_cuts':list(RUNNER.fault_report.REQUIRED_PHASE_CUTS),
            'crash_boundaries':list(RUNNER.fault_report.REQUIRED_CRASH_BOUNDARIES),'capture_surfaces':sorted(RUNNER.SURFACE_FILES),
            'traffic_count_channels':list(RUNNER.leakage_audit.REQUIRED_COUNT_CHANNELS)}}
    plan_raw=raw(plan);plan_binding=replay.accounting.accounting_file_binding(plan_raw)
    names=['campaign-0-failed','campaign-1-complete'] if include_failed_predecessor else ['campaign-1-complete']
    scope={'version':1,'protocol':replay.control.PROTOCOL,'scope_id':hashlib.sha256(b'synthetic retained release scope').hexdigest(),
        'previous_scope_sha256':None,'registered_ns':1,'stopping_policy':'fail_fast','deadline_policy':plan['benchmark_accounting'],
        'campaigns':[{'campaign_id':name,'plan':plan_binding} for name in names]}
    scope_raw=raw(scope);put(accounting_root/'scope.json',scope_raw);scope_sha=hashlib.sha256(scope_raw).hexdigest()
    callback=replay.samples.RetainedSampleReplay(**images,owner_uid=__import__('os').geteuid(),group_utility_sha256='c'*64,
        listener_utility_sha256='d'*64,packet_utility={key:value for key,value in replay.samples.semantics.packets._utility().items() if key in ('path','sha256')})
    serial=0
    for campaign_index,name in enumerate(names):
        campaign=accounting_root/'campaigns'/name;campaign.mkdir(mode=0o700)
        put(campaign/'registered-scope.json',scope_raw);put(campaign/'frozen-plan.json',plan_raw)
        for path,value in {hardware_path:hardware_payload,configuration_manifest_path:configuration_manifest_payload,
                           canary_path:canary_payload,**configuration_payloads,**policies}.items():put(campaign/path,value)
        actual,_=RUNNER.load_plan(campaign/'frozen-plan.json');assert actual==plan
        (campaign/'attempts').mkdir(mode=0o700);(campaign/'sessions').mkdir(mode=0o700)
        base={'scope_sha256':scope_sha,'campaign_id':name,'plan_sha256':plan_binding['sha256']}
        started=[];refs=[];sids=[];epoch=100_000_000+campaign_index*10**14
        for ordinal,job in enumerate(plan['jobs'],1):
            if job['kind']=='benchmark':break
            started.append(nonbenchmark_records(campaign,plan,base,ordinal,job,10+campaign_index*10**14+ordinal*10))
        failed=name.endswith('failed')
        selected=plan['benchmark_sessions'][:1 if failed else complete_session_count]
        for descriptor in selected:
            prepared=RUNNER.prepare_benchmark_session(plan,campaign,campaign,base,descriptor)
            fixture=SessionFixture(prepared,campaign,images,serial,epoch,plan['benchmark_accounting']['outer_timeout_ms'])
            if failed:
                fixture.success(0);fixture.failure(1)
            else:
                for index in range(len(prepared['request']['attempts'])):fixture.success(index)
            closure,closed=fixture.close(failed=failed);serial+=1
            started.extend(fixture.started_ids);refs.append(closure);sids.append(descriptor['session_id']);epoch=closed+1
        complete=not failed and len(selected)==len(plan['benchmark_sessions'])
        if complete:
            for ordinal,job in enumerate(plan['jobs'],1):
                if job['kind']=='leakage':
                    started.append(nonbenchmark_records(campaign,plan,base,ordinal,job,epoch));epoch+=2
        put(campaign/'campaign-closure.json',{'version':1,'protocol':replay.control.PROTOCOL,**base,
            'closed_ns':epoch+1,'quiescent':True,'started_request_ids':started,
            'reason':'completed' if complete else 'fail_fast' if failed else 'recovered_interruption',
            'started_session_ids':sids,'session_closures':refs})
    with replay._open_closed_scope(accounting_root/'scope.json',plan_harness=plan_harness,callback=callback,expected_commit=commit) as held:
        physical=held.physical_inventory();report=publication.build_report(held,100)
        counts=held.result['accounting'];rows=[json.loads(value) for value in held.successful_rows]
    artifacts=[{'kind':'benchmark_scope' if path=='scope.json' else 'benchmark_accounting_record',
        **binding,'path':'accounting/'+path} for path,binding in physical['files'].items()]
    for path,value,kind in [('reports/benchmark-physical-inventory-v1.json',physical,'benchmark_accounting_record'),
                            ('reports/benchmark-accounting-v1.json',counts,'benchmark_accounting_report')]:
        binding=put(root/path,value);artifacts.append({'kind':kind,'path':path,**binding})
    return {'artifacts':artifacts,'rows':rows,'report':report,'accounting':counts,'plan':plan,'scope':scope,
            'images':images,'plan_harness':plan_harness,'callback':callback,'physical':physical}
