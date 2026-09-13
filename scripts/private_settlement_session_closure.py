"""Publish a session cut only after native owners join and complete replay passes.

The runtime calls this owner after consuming the terminal frame and joining
its actual adapter thread. A failed replay leaves the session open; a durable
start is never erased or replaced with a fabricated per-attempt process exit.
The campaign execution owner retains this cut and every accepted sample for
subsequent full-scope replay and qualification.
"""
from __future__ import annotations

import hashlib
import time

import private_settlement_attempt_accounting as accounting
import private_settlement_record_provider as filesystem
import private_settlement_session_control as control


class _ProposedCut:
    """Overlay exactly one unpublished cut for validation before publication."""

    def __init__(self, provider, path, raw):
        self.provider, self.path, self.raw = provider, path, raw
        self.reference = {'path': path, 'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw)}
        control.require(path not in provider.inventory(), 'session cut already exists')

    def inventory(self):
        return {**self.provider.inventory(), self.path: dict(self.reference)}

    def read(self, reference):
        if reference['path'] == self.path:
            control.require(reference == self.reference, 'proposed session cut changed')
            return self.raw
        return self.provider.read(reference)


def _joined_owner(runtime, bound, accepted, *, worker_command, worker_image, group_utility_sha256):
    """Join retained terminal bytes to the actual runtime's completed owners."""
    control.exact(bound, {'worker_terminal', 'adapter_lifecycle'}, 'joined session terminal')
    control.require(runtime.used and runtime.thread is not None and not runtime.thread.is_alive()
                    and runtime.adapter_error is None and runtime.lifecycle is not None,
                    'session adapter has not joined its completed native lifecycle')
    worker = runtime.worker
    control.require(worker.process is not None and worker.reaped,
                    'session worker has not completed its actual native wait')
    worker.image.validate()
    control.require(worker_command[0] == str(worker.image.path)
                    and worker_image == {'sha256': worker.image.sha256,
                                         'bytes': worker.image.path.stat().st_size},
                    'joined worker differs from its source-admitted image')
    lifecycle = control.decode(bound['adapter_lifecycle'])
    control.require(runtime.records.read(runtime.lifecycle) == bound['adapter_lifecycle']
                    and runtime.lifecycle['path'] == 'sessions/'+runtime.prepared['identity']['session_id']+'/adapter-lifecycle.json'
                    and lifecycle['worker_process_start'] == worker.started
                    and lifecycle['worker_pid'] == worker.process.pid
                    and type(worker.process.returncode) is int
                    and worker.process.returncode == lifecycle['worker_exit_code']
                    and lifecycle['worker_wait_completed'] is True
                    and lifecycle['worker_image_unchanged'] is True,
                    'retained lifecycle differs from the actual native worker owner')
    for key in ('group_before', 'group_after'):
        control.require(lifecycle[key]['utility_sha256'] == group_utility_sha256,
                        'session group observation uses an unadmitted utility')
    control.require(tuple(accepted) == tuple(row['request_id'] for row in
                    runtime.prepared['request']['attempts'][:len(runtime.controller.written)]),
                    'runner accepted prefix differs from its completed pipe writes')


def close_joined_session(runtime, bound, accepted, complete, *, plan, registered_ns,
                         worker_command, worker_image, group_utility_sha256, validate_success):
    """Replay the closed namespace before publishing its immutable session cut.

``validate_success`` is the mandatory native economics/resource/packet replay
owner. The canonical execution owner must already have admitted ``plan`` and
the worker image. A prospective cut is never returned as physical evidence;
the published inventory is independently compared before returning its binding.
The next session cannot start until this call returns successfully.
"""
    control.require(type(complete) is bool and callable(validate_success),
                    'session completion requires typed status and semantic replay')
    control.require(control.unsigned(registered_ns) > 0, 'session registration time is missing')
    control.digest(group_utility_sha256)
    identity = control.identity(runtime.prepared['identity'])
    prefix = 'sessions/'+identity['session_id']
    campaign = {key: identity[key] for key in ('scope_sha256', 'campaign_id', 'plan_sha256')}
    admitted_plan = accounting.accounting_canonical_bytes(plan)
    plan = accounting._document(admitted_plan, 'admitted session plan')
    descriptors = [row for row in plan['benchmark_sessions'] if row['session_id'] == identity['session_id']]
    control.require(len(descriptors) == 1, 'session closure has no unique registered descriptor')
    jobs = [(ordinal, job) for ordinal, job in enumerate(plan['jobs'], 1) if job['kind'] == 'benchmark']
    own_jobs = [(ordinal, job) for ordinal, job in jobs if job['session_id'] == identity['session_id']]
    control.require(bool(own_jobs)
                    and [ordinal for ordinal, _ in own_jobs] == list(range(own_jobs[0][0], own_jobs[0][0]+len(own_jobs))),
                    'session closure interrupts the full-plan dispatch order')
    roots = ['frozen-plan.json', 'registered-scope.json', prefix,
             *[f"attempts/{ordinal:05}-{job['request_id']}" for ordinal, job in own_jobs]]
    _joined_owner(runtime, bound, accepted, worker_command=worker_command,
                  worker_image=worker_image, group_utility_sha256=group_utility_sha256)
    with filesystem.RetainedRecordProvider(runtime.records.path, roots) as provider:
        inventory = provider.inventory()
        plan_raw = provider.read(inventory['frozen-plan.json'])
        scope_raw = provider.read(inventory['registered-scope.json'])
        scope = accounting._document(scope_raw, 'registered session scope')
        control.require(hashlib.sha256(plan_raw).hexdigest() == identity['plan_sha256']
                        and accounting.accounting_canonical_bytes(accounting._document(plan_raw, 'frozen session plan')) == admitted_plan,
                        'session closure differs from the exact admitted frozen-plan bytes')
        registered = [row for row in scope['campaigns'] if row['campaign_id'] == identity['campaign_id']]
        control.require(hashlib.sha256(scope_raw).hexdigest() == identity['scope_sha256']
                        and scope['registered_ns'] == registered_ns and len(registered) == 1
                        and registered[0]['plan'] == accounting.accounting_file_binding(plan_raw),
                        'session closure differs from its registered scope or plan binding')
        for key, leaf in (('worker_terminal', 'worker-terminal.json'), ('adapter_lifecycle', 'adapter-lifecycle.json')):
            control.require(provider.read(inventory[prefix+'/'+leaf]) == bound[key],
                            'joined terminal differs from the frozen physical record')
        ready, observed = (inventory.get(prefix+'/'+leaf) for leaf in ('ready.json', 'process-ready.json'))
        cut = {**identity, 'session_started': runtime.started,
               'worker_terminal': inventory[prefix+'/worker-terminal.json'],
               'adapter_lifecycle': inventory[prefix+'/adapter-lifecycle.json'],
               'ready': ready, 'process_observation': observed,
               'closed_ns': time.time_ns(), 'bindings_unchanged': True, 'adapter_thread_joined': True}
        raw = control.canonical(cut)
        proposed = _ProposedCut(provider, prefix+'/session-closure.json', raw)
        replay_records = accounting._SessionRecords(proposed)
        rows, samples, summary = accounting.reduce_retained_session(descriptors[0],
            {'session_id': identity['session_id'], 'closure': proposed.reference}, records=replay_records,
            campaign_identity=campaign, jobs=jobs, registered_ns=registered_ns, closed_ns=cut['closed_ns'],
            policy=plan['benchmark_accounting'], worker_command=worker_command, worker_image=worker_image,
            validate_success=validate_success)
        control.require(complete == (summary['terminal_kind'] == 'completed'),
                        'runner completion disagrees with the fully replayed native session')
        replay_records.validate()
        _joined_owner(runtime, bound, accepted, worker_command=worker_command,
                      worker_image=worker_image, group_utility_sha256=group_utility_sha256)
        control.require(provider.inventory() == inventory, 'session inventory changed during cut replay')
    # Publishing changes directory metadata, so open a new physical inventory
    # and compare every original content binding plus the one authorized leaf.
    published = runtime.records.publish(proposed.path, raw)
    control.require(published == proposed.reference, 'published session cut differs from its validated bytes')
    with filesystem.RetainedRecordProvider(runtime.records.path, roots) as owner:
        control.require(owner.inventory() == {**inventory, proposed.path: proposed.reference}
                        and owner.read(published) == raw,
                        'session records changed across immutable cut publication')
    return {'reference': published, 'rows': rows, 'samples': samples, 'summary': summary}
