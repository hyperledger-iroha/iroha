"""Publish one actual campaign cut using the shared retained accounting owner.

Other registered campaigns remain physically untouched until their own owner
closes them. A complete scope is replayed only when all real cuts exist.
The canonical campaign loop calls this owner after each retained session has
closed; registered scope publication separately enforces full qualification.
"""
from __future__ import annotations

from contextlib import ExitStack
import hashlib
import os
from pathlib import Path
import time

import private_settlement_attempt_accounting as accounting
import private_settlement_session_collection as collection
import private_settlement_session_control as control
import private_settlement_session_closure as session_closure
import private_settlement_record_provider as filesystem
import private_settlement_registered_session_replay as replay
import private_settlement_release_runner as runner


class _ProspectiveCampaign(session_closure._ProposedCut):
    """Expose the unchanged physical namespace plus exactly one proposed cut."""
    @property
    def directories(self):
        return self.provider.directories


def _nonbenchmark_groups(packet):
    """Require the recorded physical group closure after typed accounting."""
    # Numeric PGIDs can be reused after observed closure. This historical replay
    # must not assign a later unrelated group to the completed owner.
    for item in packet['nonbenchmark']:
        if item['started'] is None:
            continue
        value=accounting._document(item['process'],'nonbenchmark process closure')
        pid=value['pid']
        control.require(value['owned_process_group_gone'] is True
                        and (pid is None or (type(pid) is int and pid>1)),
                        'cannot close a campaign with an unconfirmed nonbenchmark process group')


def close_retained_campaign(root, *, plan, scope_path, scope_sha256, campaign_id,
                            plan_sha256, reason, validate_success):
    """Dry-run the immutable cut, publish it, then replay any fully closed scope.

The concrete sample callback must carry the caller's admitted native images.
No per-attempt benchmark process record is required or accepted as its session
closure. Source qualification and permission to end this campaign remain with
the canonical calling execution owner.
"""
    control.require(type(validate_success) is replay.samples.RetainedSampleReplay,
                    'canonical retained sample replay owner is mandatory')
    control.require(reason in {'completed','fail_fast','preparation_failed','recovered_interruption','not_run'},
                    'campaign closure reason is undeclared')
    root,scope_path=Path(root),Path(scope_path)
    control.require(root.is_absolute() and root.resolve(strict=True)==root
                    and scope_path.is_absolute() and scope_path.resolve(strict=True)==scope_path
                    and root==scope_path.parent/'campaigns'/campaign_id,
                    'campaign closure locator differs from registration')
    worker=validate_success.images['worker']
    command=[worker['path'],replay.samples.adapter.WORKER_TEST,'--exact','--ignored','--nocapture','--test-threads=1']
    image={key:worker[key] for key in ('sha256','bytes')}
    with ExitStack() as stack:
        registered=stack.enter_context(filesystem.RetainedRecordProvider(scope_path.parent,[scope_path.name]))
        scope_ref=registered.inventory()[scope_path.name];scope_raw=registered.read(scope_ref)
        control.require(scope_ref['sha256']==scope_sha256,'registered scope bytes differ')
        scope=replay._scope(scope_raw)
        expected=next((slot for slot in scope['campaigns'] if slot['campaign_id']==campaign_id),None)
        control.require(expected is not None and expected['plan']['sha256']==plan_sha256,'campaign is not registered')
        current,_=runner.load_plan(root/'frozen-plan.json')
        control.require(runner.canonical_bytes(current)==runner.canonical_bytes(plan),'admitted plan changed')
        references=runner.frozen_plan_input_records(plan,root)
        roots=['frozen-plan.json','registered-scope.json',*[row['path'] for row in references]]
        for namespace in ('sessions','attempts'):
            if os.path.lexists(root/namespace):roots.append(namespace)
        control.require(not os.path.lexists(root/'campaign-closure.json'),'campaign closure already exists')
        physical=stack.enter_context(filesystem.RetainedRecordProvider(root,roots))
        inventory=physical.inventory()
        for row in references:
            control.require(inventory[row['path']]=={key:row[key] for key in ('path','sha256','bytes')},
                            'frozen campaign input differs')
        starts=[];sessions=[];session_refs=[]
        for ordinal,job in enumerate(plan['jobs'],1):
            if f"attempts/{ordinal:05}-{job['request_id']}/started.json" in inventory:
                starts.append(job['request_id'])
        for descriptor in plan['benchmark_sessions']:
            prefix='sessions/'+descriptor['session_id']
            if prefix+'/started.json' in inventory:
                control.require(prefix+'/session-closure.json' in inventory,
                                'started native session has no authoritative joined closure')
                sessions.append(descriptor['session_id']);session_refs.append(inventory[prefix+'/session-closure.json'])
        document={'version':1,'protocol':control.PROTOCOL,'scope_sha256':scope_sha256,
            'campaign_id':campaign_id,'plan_sha256':plan_sha256,'closed_ns':time.time_ns(),
            'quiescent':True,'started_request_ids':starts,'reason':reason,
            'started_session_ids':sessions,'session_closures':session_refs}
        raw=control.canonical(document)
        prospective=_ProspectiveCampaign(physical,'campaign-closure.json',raw)
        collected=collection.collect_campaign_records(prospective,scope_raw=scope_raw,
            campaign_id=campaign_id,plan_binding=expected['plan'])
        replay._requests(plan,root,collected)
        summary=accounting.reduce_retained_campaign(scope_raw,collected.packet,collected.successful_rows,
            worker_command=command,worker_image=image,validate_success=validate_success)
        _nonbenchmark_groups(collected.packet)
        collected.validate();registered.read(scope_ref)
        control.require(physical.inventory()==inventory,'campaign changed during prospective closure')
        # Close the frozen view before its one authorized sibling publication.
        physical.close()
        with control.RecordDirectory(root) as records:
            published=records.publish('campaign-closure.json',raw)
        control.require(published==prospective.reference,'published campaign differs from validated cut')
        actual=stack.enter_context(collection.collect_closed_campaign(root,scope_raw=scope_raw,
            campaign_id=campaign_id,plan_binding=expected['plan']))
        # The collector owns session evidence; separately bind transitive input
        # files across publication without loading packet bodies into memory.
        complete=stack.enter_context(filesystem.RetainedRecordProvider(root,[*roots,'campaign-closure.json']))
        control.require(complete.inventory()=={**inventory,'campaign-closure.json':published},
                        'campaign changed across immutable closure publication')
        _nonbenchmark_groups(actual.packet);actual.validate();registered.read(scope_ref)
    pending=[slot['campaign_id'] for slot in scope['campaigns']
             if not os.path.lexists(scope_path.parent/'campaigns'/slot['campaign_id']/'campaign-closure.json')]
    scope_result=None
    if not pending:
        scope_result=replay._replay_closed_scope(scope_path,plan_harness=plan['harness'],
            callback=validate_success,expected_commit=plan['commit'])
    return {'path':root/'campaign-closure.json','reference':published,'campaign':summary,
            'pending_campaign_ids':pending,'scope_replay':scope_result,
            'source_and_smoke_admitted':False,'release_qualified':False}
