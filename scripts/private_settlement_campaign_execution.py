"""Canonical campaign execution owners, admission, and retained-session dispatch.

Each session has one actual native owner. Exceptions expose every owner; no
retry, signal, synthetic process exit, or benchmark metric is introduced.
The canonical runner publishes only after complete retained scope replay,
archival, grouped statistics and final source/image checks.
"""
from pathlib import Path
import json
import os
import time

import private_settlement_release_runner as runner
import private_settlement_registered_session_replay as replay
import private_settlement_session_execution as sessions
import private_settlement_session_control as control
import private_settlement_campaign_lifetime as lifetime


class CampaignExecutionIncomplete(RuntimeError):
    """Keep the original failure and all actual session/verifier/capture owners."""
    def __init__(self,owner):
        self.owner=owner
        super().__init__('campaign incomplete; actual owners and durable starts retained')


class CampaignExecution:
    """One campaign owner that survives failures in execution or publication."""
    def __init__(self):
        self.session_owners={}
        self.active_session=None
        self.images=self.callback=self.utilities=None
        self.error=self.closure=self.closure_error=None
        self.source_root=self.smoke_campaign=self.harness=None
        self.prerequisite=None
        self.output_root=self.physical_record_error=None
        self.lifetime=lifetime.CampaignLifetime(self)

    def admit(self,*,source_root,harness,smoke_campaign,plan,worker_path,validator_path,prerequisite):
        """Use only exact images authenticated by the existing ten-smoke gate."""
        self.source_root,self.harness,self.smoke_campaign=source_root,harness,smoke_campaign
        self.prerequisite=prerequisite
        self.images=replay._admitted_images(worker_path,validator_path,prerequisite)
        group=runner.verify_harness(Path('/bin/ps').resolve(strict=True))
        listener=replay.samples.network._utility_identity()
        packet=replay.samples.semantics.packets._utility()
        self.utilities=(group,listener,packet)
        self.callback=replay.samples.RetainedSampleReplay(**self.images,owner_uid=os.geteuid(),
            group_utility_sha256=group['sha256'],listener_utility_sha256=listener[1],
            packet_utility={key:packet[key] for key in ('path','sha256')})

    def revalidate(self):
        """Reject changed native and measurement images before campaign closure."""
        for value in self.images.values():
            control.require(runner.verify_harness(Path(value['path']))=={key:value[key] for key in ('sha256','bytes')},
                            'admitted native executable changed during campaign')
        group,listener,packet=self.utilities
        control.require(runner.verify_harness(Path('/bin/ps').resolve(strict=True))==group
            and replay.samples.network._utility_identity()==listener
            and replay.samples.semantics.packets._utility()==packet,
            'admitted measurement utility changed during campaign')

    def retain_session_owner(self,sid,owner):
        """Admit only the canonical owner before adding physical drain work.

        Incomplete canonical owners remain retained. This checks the programmer
        boundary, never closure, deadlines, reaping or child success.
        """
        control.require(isinstance(owner,sessions.AdmittedSessionExecution),
                        'campaign session owner is not canonical')
        control.require(sid not in self.session_owners,'session owner cannot be retried')
        self.session_owners[sid]=owner

    def run_session(self,descriptor,*,root,plan,completed_ids):
        """Dispatch exactly one registered contiguous session after its prefix."""
        group=[(ordinal,job) for ordinal,job in enumerate(plan['jobs'],1)
               if job.get('session_id')==descriptor['session_id']]
        control.require(group and [ordinal for ordinal,_ in group]==list(range(group[0][0],group[0][0]+len(group))),
                        'registered session is not contiguous')
        control.require(completed_ids==[job['request_id'] for job in plan['jobs'][:group[0][0]-1]],
                        'retained session cannot bypass unsuccessful full-plan predecessor')
        self.validate_prefix_owners(root,plan,group[0][0]-1)
        sid=descriptor['session_id']
        control.require(sid not in self.session_owners,'session owner cannot be retried')
        self.active_session=sid
        runtime_root=root/'runtime'/sid
        runner.fresh_private_directory(runtime_root)
        self.revalidate()
        try:
            owner=sessions.execute_admitted_session(frozen_plan=root/'frozen-plan.json',
                registered_scope=root/'registered-scope.json',campaign_id=root.name,descriptor=descriptor,
                plan_harness=plan['harness'],worker_image=self.images['worker'],validator_image=self.images['validator'],
                cwd=self.source_root,runtime_root=runtime_root)
        except sessions.SessionExecutionIncomplete as error:
            self.retain_session_owner(sid,error.owner)
            raise
        self.retain_session_owner(sid,owner)
        control.require(owner.closure is not None and owner.result is not None,
                        'native session returned without a validated joined closure')
        rows=owner.closure['rows'];raw_samples=owner.closure['samples']
        control.require([row['request_id'] for row in rows]==[job['request_id'] for _,job in group]
                        and owner.closure['summary']['session_id']==sid,
                        'returned session closure differs from registered full-plan jobs')
        parsed=[runner.strict_json_loads(raw.decode('utf-8'),'retained session sample') for raw in raw_samples]
        succeeded=[row for row in rows if row['state']=='succeeded']
        control.require([row['attempt_id'] for row in parsed]==[row['attempt_id'] for row in succeeded],
                        'returned closure sample inventory differs from validated attempts')
        completed=[]
        for sample in parsed:
            ordinal,job=next((ordinal,job) for ordinal,job in group if job['request_id']==sample['request_id'])
            completed.append({'ordinal':ordinal,'request_id':job['request_id'],'kind':'benchmark',
                **{key:sample[key] for key in ('attempt_id','invocation_nonce','scope_sha256','campaign_id','plan_sha256')}})
        complete=(owner.result['all_attempts_accepted'] is True
                  and owner.closure['summary']['terminal_kind']=='completed'
                  and len(succeeded)==len(group))
        self.revalidate()
        if complete:self.active_session=None
        return owner,group,parsed,completed,complete

    def validate_prefix_owners(self,root,plan,count):
        """Join real predecessor lifetimes before publishing the next owner."""
        accounting=runner.attempt_accounting
        scope=runner.read_json_file(root/'registered-scope.json','registered scope')
        identity={'scope_sha256':runner.file_binding(root/'registered-scope.json')['sha256'],
            'campaign_id':root.name,'plan_sha256':runner.file_binding(root/'frozen-plan.json')['sha256']}
        intervals=[];seen=set();now=time.time_ns()
        for ordinal,job in enumerate(plan['jobs'][:count],1):
            if job['kind']=='benchmark':
                sid=job['session_id']
                if sid in seen:continue
                seen.add(sid);owner=self.session_owners[sid]
                control.require(owner.closure is not None,'predecessor session remains open')
                cut=control.decode(owner.records.read(owner.closure['reference']))
                summary=owner.closure['summary']
                control.require(summary['terminal_kind']=='completed' and cut['closed_ns']==summary['closed_ns'],
                                'predecessor session has not completed its exact lifetime')
                intervals.append((summary['started_ns'],summary['closed_ns']))
                continue
            directory=root/'attempts'/f"{ordinal:05}-{job['request_id']}"
            start=accounting._document(runner.retained_accounting_bytes(directory/'started.json'),'predecessor start')
            accounting.exact_fields(start,accounting.START_FIELDS,'predecessor start')
            expected={**identity,'request_id':job['request_id'],
                'attempt_id':accounting.registered_attempt_id(**identity,request_id=job['request_id']),
                'invocation_nonce':accounting._digest(start['invocation_nonce'],'predecessor nonce')}
            accounting._record_identity(start,expected,'predecessor start')
            request=runner.retained_accounting_bytes(directory/'request.json')
            control.require(start['request']==accounting.accounting_file_binding(request)
                            and start['harness']==plan['harness'],'predecessor request/image changed')
            process=accounting._process_record(accounting._document(
                runner.retained_accounting_bytes(directory/'process-outcome.json'),'predecessor process'),
                expected,start,now,plan['benchmark_accounting'])
            control.require(process['completion_kind']=='exited' and process['exit_code']==0
                            and process['passed'] is True and process['owned_process_group_gone'] is True,
                            'predecessor nonbenchmark lacks successful physical closure')
            intervals.append((start['started_ns'],process['finished_ns']))
        accounting.validate_serial_owner_lifetimes(intervals,scope['registered_ns'])

    def close_completed_owners(self):
        """Release descriptors only through each actual closed session owner."""
        for owner in self.session_owners.values():
            owner.close()


def durable_start_projection(root,plan):
    """Read actual immutable starts; never infer the denominator from results."""
    present=[]
    identity={'scope_sha256':runner.file_binding(root/'registered-scope.json')['sha256'],
        'campaign_id':root.name,'plan_sha256':runner.file_binding(root/'frozen-plan.json')['sha256']}
    for ordinal,job in enumerate(plan['jobs'],1):
        path=root/'attempts'/f"{ordinal:05}-{job['request_id']}"/'started.json'
        if not os.path.lexists(path):continue
        raw=runner.retained_accounting_bytes(path)
        value=runner.strict_json_loads(raw.decode('utf-8'),'durable attempt start')
        control.require(all(value[key]==expected for key,expected in identity.items())
                        and value['request_id']==job['request_id']
                        and value['attempt_id']==runner.attempt_accounting.registered_attempt_id(
                            **identity,request_id=job['request_id']),
                        'durable start differs from its full-plan identity')
        present.append(job['request_id'])
    return present


def execute_plan(*args,**kwargs):
    """Return a published fragment, or an exception retaining all actual owners."""
    owner=CampaignExecution()
    try:
        with owner.lifetime:
            result=runner._execute_retained_plan(*args,owner=owner,**kwargs)
            owner.lifetime.require_launch()
            return result
    except BaseException as error:
        owner.error=error
        if owner.output_root is not None:
            try:
                runner.private_record(owner.output_root/'physical-owner-drain.json', {
                    'version':1,'kind':'campaign_physical_drain','qualification':False,
                    'original_error_type':type(error).__name__,
                    'drain_request':owner.lifetime.drain_reason,
                    'sessions':owner.lifetime.observations,'cleanup_errors':owner.lifetime.cleanup_errors,
                    'recorded_ns':time.time_ns(),
                })
            except BaseException as publication_error:
                # Physical waits are already complete. Preserve the original
                # cause even if this diagnostic cannot be made durable.
                owner.physical_record_error=type(publication_error).__name__
        raise CampaignExecutionIncomplete(owner) from error
