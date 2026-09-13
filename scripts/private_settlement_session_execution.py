"""Execute one already-admitted retained session and preserve every live owner.

The canonical campaign caller must authenticate clean source, ten-smoke images,
registration and the complete preceding job prefix before calling this module.
This owner checks their exact local bindings; it does not grant that authority.
The canonical campaign composes this owner with its source admission and
joined closure. This component alone does not qualify a native benchmark.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat

import private_settlement_attempt_accounting as accounting
import private_settlement_release_runner as runner
import private_settlement_session_control as control
import private_settlement_session_adapter as adapter
import private_settlement_session_runtime as runtime
import private_settlement_session_semantics as semantics
import private_settlement_session_closure as closure
import private_settlement_sample_replay as samples
import private_settlement_process_observer as processes
import private_settlement_network_observer as network
import private_settlement_packet_window as packets


class SessionExecutionIncomplete(RuntimeError):
    """Expose the complete owner after any setup, process, replay or cut failure."""

    def __init__(self, owner):
        self.owner = owner
        super().__init__('admitted session incomplete; exact owners and durable starts retained')


class AdmittedSessionExecution:
    """Single-use owner, including after a partially successful spawn.

No automatic retry, signal, synthetic terminal or per-attempt process exit is
introduced. The caller must retain an incomplete instance; close() refuses to
release observation/image owners until no child was spawned or a full session
closure has authenticated the actual joined lifecycle.
"""

    def __init__(self):
        self.used = self.closed = False
        self.records = self.reader = self.listener = self.observations = None
        self.prepared = self.started = self.worker = self.runtime = None
        self.semantic_owner = self.callbacks = self.sample_replay = None
        self.exchange = self.closure = self.result = self.error = None
        self.images = {}
        self.foundations = {}
        self.utility = None

    def _foundation_check(self):
        for reference in self.foundations.values():
            self.records.read(reference)
        for image in self.images.values():
            image.validate()
        control.require(network._utility_identity() == self.utility['listener']
                        and packets._utility() == self.utility['packet'],
                        'native observation utility changed during session execution')
        control.require(runner.verify_harness(Path('/bin/ps').resolve(strict=True)) == self.utility['group'],
                        'native process-group utility changed during session execution')

    def run(self, *, frozen_plan, registered_scope, campaign_id, descriptor,
            plan_harness, worker_image, validator_image, cwd, runtime_root):
        """Compose canonical writers and actual native owners exactly once."""
        control.require(not self.used, 'admitted session owner cannot be reused')
        self.used = True
        try:
            self._run(frozen_plan=Path(frozen_plan), registered_scope=Path(registered_scope),
                campaign_id=campaign_id, descriptor=descriptor, plan_harness=plan_harness,
                worker_image=worker_image, validator_image=validator_image,
                cwd=Path(cwd), runtime_root=Path(runtime_root))
            return self
        except BaseException as error:
            self.error = error
            # These established exceptions retain objects even when their
            # constructors fail after Popen. Never discard their ownership.
            if isinstance(error, adapter.NativeWorkerStartFailure):
                self.worker = error.worker
            if isinstance(error, runtime.SessionRuntimeIncomplete):
                self.runtime = error.runtime
                self.worker = error.runtime.worker
            raise SessionExecutionIncomplete(self) from error

    def _run(self, *, frozen_plan, registered_scope, campaign_id, descriptor,
             plan_harness, worker_image, validator_image, cwd, runtime_root):
        root = frozen_plan.parent
        control.require(frozen_plan.name == 'frozen-plan.json'
                        and registered_scope == root/'registered-scope.json'
                        and root.is_absolute() and root.resolve(strict=True) == root
                        and cwd.is_absolute() and cwd.resolve(strict=True) == cwd,
                        'session foundation or execution directory is not canonical')
        runner.require_external_output(root, cwd)
        self.records = control.RecordDirectory(root)
        self.foundations = {name:self.records.locate(name) for name in ('frozen-plan.json','registered-scope.json')}
        plan, plan_root = runner.load_plan(frozen_plan)
        control.require(plan_root == root and plan['harness'] == plan_harness,
                        'session plan differs from its independently admitted harness')
        self.plan = plan
        scope_raw = self.records.read(self.foundations['registered-scope.json'])
        scope = accounting._document(scope_raw, 'registered session scope')
        plan_ref = self.foundations['frozen-plan.json']
        binding = {key:plan_ref[key] for key in ('sha256','bytes')}
        runner.load_benchmark_scope(registered_scope, campaign_id=campaign_id,
            plan_binding=binding, deadline_policy=plan['benchmark_accounting'])
        self.campaign = {'scope_sha256':hashlib.sha256(scope_raw).hexdigest(),
                         'campaign_id':campaign_id,'plan_sha256':plan_ref['sha256']}
        self.registered_ns = scope['registered_ns']
        for reference in runner.frozen_plan_input_records(plan, root):
            exact = {key:reference[key] for key in ('path','sha256','bytes')}
            control.require(self.records.locate(exact['path']) == exact, 'admitted plan input changed')
            self.foundations[exact['path']] = exact
        for name,value in (('worker',worker_image),('validator',validator_image)):
            value = samples._image(value)
            path = Path(value['path'])
            control.require(path.resolve(strict=True) == path, 'admitted native image path is not canonical')
            image = processes.ExecutableImage(path,value['sha256'])
            self.images[name] = image
            control.require(image.path.stat().st_size == value['bytes'], 'admitted native image size changed')
        info = runtime_root.lstat()
        control.require(runtime_root.is_absolute() and runtime_root.resolve(strict=True) == runtime_root
                        and stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700
                        and info.st_uid == os.geteuid() and not any(runtime_root.iterdir()),
                        'native session runtime must be fresh canonical owner-only storage')
        self.runtime_root = runtime_root
        self.utility = {'group':runner.verify_harness(Path('/bin/ps').resolve(strict=True)),
                        'listener':network._utility_identity(),'packet':packets._utility()}
        self.reader = processes.native_reader()
        self.listener = network.DarwinListenerReader()
        self.observations = adapter.NativeObservations(process_reader=self.reader,
            listener_reader=self.listener,images=self.images)
        self.sample_replay = samples.RetainedSampleReplay(worker=worker_image,validator=validator_image,
            owner_uid=os.geteuid(),group_utility_sha256=self.utility['group']['sha256'],
            listener_utility_sha256=self.utility['listener'][1],
            packet_utility={key:self.utility['packet'][key] for key in ('path','sha256')})
        self._foundation_check()
        for name in ('sessions','attempts'):
            if not (root/name).exists():
                runner.fresh_private_directory(root/name)
        self.prepared = runner.prepare_benchmark_session(plan,root,root,self.campaign,descriptor)
        command = [str(self.images['worker'].path),adapter.WORKER_TEST,
                   '--exact','--ignored','--nocapture','--test-threads=1']
        image_binding = {key:worker_image[key] for key in ('sha256','bytes')}
        self.started = runner.publish_benchmark_session_start(self.prepared,records=self.records,
            command=command,harness=image_binding)
        budget = plan['benchmark_accounting']['outer_timeout_ms']
        deadline = adapter.start_deadline(control.decode(self.records.read(self.started)),budget)
        self.semantic_owner = semantics.SessionSemantics(self.prepared,records=self.records,
            observations=self.observations,packet_utility=self.utility['packet'],cwd=cwd,
            deadline=lambda:deadline,outer_timeout_ms=budget)
        self._foundation_check()
        self.worker = runtime.spawn_bound_worker(self.prepared,self.started,records=self.records,
            cwd=cwd,images=self.images,runtime_root=runtime_root,outer_timeout_ms=budget)
        self.runtime = runtime.SessionRuntime(self.prepared,self.started,records=self.records,worker=self.worker,
            observations=self.observations,packet_owner=packets,semantic_owner=self.semantic_owner,outer_timeout_ms=budget)
        first = next(i for i,job in enumerate(plan['jobs'],1) if job.get('session_id') == descriptor['session_id'])
        def close_session(bound,accepted,complete):
            self._foundation_check()
            self.closure = closure.close_joined_session(self.runtime,bound,accepted,complete,
                plan=plan,registered_ns=self.registered_ns,worker_command=command,worker_image=image_binding,
                group_utility_sha256=self.utility['group']['sha256'],validate_success=self.sample_replay)
            self._foundation_check()
            return self.closure
        self.callbacks = runtime.RunnerSemanticCallbacks(self.prepared,self.started,records=self.records,
            semantic_owner=self.semantic_owner,first_ordinal=first,outer_timeout_ms=budget,validate_lifecycle=close_session)
        self.exchange = self.runtime.run(publish_start=self.callbacks.publish_start,
            validate_ready=self.callbacks.validate_ready,validate_completion=self.callbacks.validate_completion,
            validate_acceptance=self.callbacks.validate_acceptance,validate_terminal=self.callbacks.validate_terminal)
        control.require(self.closure is not None, 'session returned without its authoritative closure')
        self._foundation_check()
        self.result = {'session_id':self.prepared['identity']['session_id'],
            'all_attempts_accepted':self.exchange.all_attempts_accepted,'session_closure':self.closure['reference'],
            'source_and_smoke_admitted':False,'release_qualified':False}

    @property
    def packet_owners(self):
        """Expose all actual packet owners retained by the in-process adapter."""
        if self.runtime is None or self.runtime.owner is None:
            return {}
        return self.runtime.owner.packet_owners

    @property
    def resource_owners(self):
        """Expose retained sampler threads, including failed publication paths."""
        if self.runtime is None or self.runtime.owner is None:
            return {}
        return self.runtime.owner.resource_owners

    def require_physical_owners_closed(self):
        """Check retained object facts without re-probing historical numeric PIDs."""
        if self.runtime is not None:
            control.require(self.runtime.thread is None or not self.runtime.thread.is_alive(),
                            'adapter still running')
        for resources in self.resource_owners.values():
            control.require(not resources._thread.is_alive(), 'resource sampler still running')
        for packet in self.packet_owners.values():
            control.require(packet.closed and (packet.child is None or packet.child.returncode is not None)
                            and (packet.thread is None or not packet.thread.is_alive()),
                            'capture physical owner remains unresolved')
        if self.worker is not None:
            control.require(self.closure is not None,
                            'incomplete native owner must remain available for recovery')
            control.require(self.worker.reaped, 'worker has no actual completed wait')
            control.require(self.worker.physical_exit is not None,
                            'worker lacks canonical physical closure')
        if self.semantic_owner is not None:
            control.require(all(native.reaped and native.physical_exit is not None
                                for native in self.semantic_owner.native_invocations.values()),
                            'native verifier lacks canonical physical closure')

    def close(self):
        """Release descriptors after safe completion; unresolved owners stay held."""
        self.require_physical_owners_closed()
        if self.closed:
            return
        control.require(self.worker is None or self.closure is not None,
                        'incomplete native owner must remain available for recovery')
        if self.runtime is not None:
            control.require(self.runtime.thread is None or not self.runtime.thread.is_alive(), 'adapter still running')
            self.runtime.close_inactive_descriptors()
        if self.worker is not None:
            control.require(self.worker.reaped, 'worker has no actual completed wait')
            self.worker.close()
        if self.semantic_owner is not None:
            for invocation in self.semantic_owner.native_invocations.values():
                control.require(invocation.reaped, 'native verifier has no completed wait')
                invocation.close()
        if self.reader is not None:
            self.reader.close()
        for image in self.images.values():
            image.close()
        if self.records is not None:
            self.records.close()
        self.closed = True


def execute_admitted_session(**arguments):
    """Return the closed owner, or raise with the exact incomplete owner attached."""
    return AdmittedSessionExecution().run(**arguments)
