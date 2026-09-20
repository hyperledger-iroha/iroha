"""One original fixed experiment from trial creation through public run custody.

Only the next locally created FixedTrial can hand off to its originating slot.
Every failure invalidates all earlier authorities; deadlines and budgets cannot
be renewed. No receipt/dictionary can enter this authority path.

The caller retains RuntimeAdmission through native cleanup and report readback.
This owner publishes observed measurements, never an independent release PASS.
"""
from dataclasses import dataclass
import hashlib
from pathlib import Path
import os
import time

from resource_evidence_budget import canonical_run_budget_bytes, select_run_budget, RUN_FILE_FIELDS
from scaling_structural_identity import pin_experiment_budget, pin_experiment_plan
from resource_bundle import _identity, _directory_owner
from resource_process import ExecutableImage, ProcessIdentity
from resource_replay import ExpectedPeer, ReplayGeometry, ReplayResult
from scaling_completed_authority import CompletedRunAuthority, _GENESIS, _NATIVE_ROLES
from scaling_experiment_directories import ExperimentDirectories
from scaling_experiment_files import FixedExperimentFiles
from scaling_experiment_inputs import public_inputs
from scaling_experiment_plan import RUN_KEYS, admit_plan, plan_bytes
from scaling_experiment_projection import raw_run, run_receipt
from scaling_fixed_trial import FixedTrial, TrialPaths, TrialRuntime
from scaling_public_files import RunPublicFiles
from scaling_worker_sources import WorkerSourceFiles


class ExperimentCustodyError(ValueError):
    """Closed original-experiment failure, excluding private configuration data."""


def require(value):
    if not value: raise ExperimentCustodyError('fixed_experiment_failed')


def failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise ExperimentCustodyError('fixed_experiment_failed') from None


class FixedRunSlot:
    """Single originating trial slot, available only from begin_run."""
    def __init__(self, *args, **kwargs):
        raise ExperimentCustodyError('fixed_experiment_failed')

    def create_trial(self) -> FixedTrial:
        """Create this slot's actual trial under the unchanged outer deadline."""
        return self._owner._create_trial(self)

    def _admit_trial(self, trial: FixedTrial):
        return self._owner._admit_trial(self, trial)


class CompletedRunHandle:
    """Opaque membership handle; its live originating owner retains all authority."""
    def __init__(self, *args, **kwargs):
        raise ExperimentCustodyError('fixed_experiment_failed')


@dataclass(slots=True)
class _Run:
    handle: CompletedRunHandle
    authority: CompletedRunAuthority
    files: RunPublicFiles
    captures: object
    published: bool = False


class _CompletedScanBoundary:
    """One full check's borrowed selection, never retained verification truth."""
    __slots__=('owner','token','runs','phase','selected','completed','closed')
    def __init__(self, owner):
        require(owner._completed_scan is None and owner._busy)
        self.owner=owner;self.token=owner._original_token
        self.runs=owner._run_pins;self.phase=owner._phase
        self.selected=None;self.completed=0;self.closed=False

    def check(self, owner, capture_scope):
        require(type(self) is _CompletedScanBoundary and not self.closed
                and owner is self.owner and owner._busy
                and owner._completed_scan is self and owner._original_token is self.token
                and owner._run_pins is self.runs and owner._phase==self.phase
                and type(self.completed) is int and 0<=self.completed<len(self.runs)
                and self.selected is self.runs[self.completed]
                and type(capture_scope) is tuple and len(capture_scope)==2
                and type(capture_scope[0]) is int and capture_scope[0]==self.completed
                and capture_scope[1] is self.selected[2]
                and self.selected[3]._busy)
        return self.selected

    def check_directories(self, owner, directories):
        require(owner is self.owner and not self.closed
                and owner._run_pins is self.runs and self.selected is None
                and self.completed==len(self.runs)
                and tuple(owner._directories._managed)==tuple(path for path,_ in directories))
        for path,identity in directories:
            require(_identity(os.fstat(owner._directories._rows[path][0]))==identity)


class FixedExperimentCustody:
    """Retain exact source, roots, allocations and ordered native run ownership."""
    def __init__(self, evidence: Path, runtime_root: Path, plan, budget,
                 runtime: TrialRuntime, process_reader, verify_runtime, identity,
                 worker_sources: WorkerSourceFiles):
        self._phase='admitting';self._busy=True;self._in_runtime=False
        self._directories=self._controls=None;self._active=None;self._runs=[]
        self._active_pin=None
        self._run_pins=()
        self._completed_scan=None
        self._manifest=self._manifest_pin=self._report=self._report_pin=None
        self._replay=self._replay_pin=None;self._replay_released=False
        self._replayed=self._replayed_pin=();self._replay_results=self._replay_results_pin=None
        self._measurements=self._measurement_pin=None
        self._active_public=self._active_captures=self._pending_authority=None
        self._token=self._original_token=object()
        try:
            self._plan,self._budget,self._plan_raw=admit_plan(plan,budget)
            require(type(runtime) is TrialRuntime and type(worker_sources) is WorkerSourceFiles
                    and callable(verify_runtime) and callable(getattr(process_reader,'sample',None)))
            require(all(type(getattr(runtime,name)) is ExecutableImage for name in ('kagami','cli','daemon','resource_program')))
            self._runtime,self._reader,self._workers=runtime,process_reader,worker_sources
            self._callback=self._original_callback=verify_runtime
            self._images=tuple((getattr(runtime,name),getattr(runtime,name).path,
                getattr(runtime,name).fd,getattr(runtime,name).identity,getattr(runtime,name).sha256,
                getattr(runtime,name).uuids) for name in ('kagami','cli','daemon','resource_program'))
            self._runtime_pin=(runtime.resource_worker,runtime.resource_worker_sha256,worker_sources.pins)
            self._budget_raw=canonical_run_budget_bytes(select_run_budget(self._budget,1,'one_lane'))
            self._budget_identity=pin_experiment_budget(self._budget,self._budget_raw)
            self._plan_identity=pin_experiment_plan(self._plan,self._plan_raw)
            self._end=self._original_end=time.monotonic_ns()+self._plan.experiment_timeout_ns
            self._runtime_guard()
            identity_raw,source_raw=public_inputs(identity,runtime,self._plan_raw,worker_sources)
            raws=(identity_raw,self._plan_raw,source_raw)
            require(tuple(len(raw) for raw in raws)==tuple(item.size_bytes for item in self._budget.static_files))
            self._directories=ExperimentDirectories(evidence,runtime_root)
            self._controls=FixedExperimentFiles(evidence,self._directories.descriptor(evidence),
                                               self._budget,self._scope_check)
            for role,raw in zip(('identity','plan','source_closure'),raws,strict=True):
                self._controls.publish_static(role,raw)
            self._full_check();self._phase='ready';self._busy=False
        except BaseException as error:
            self._poison()
            if self._controls is not None:self._controls.close()
            if self._directories is not None:self._directories.close()
            failure(error)

    def _poison(self):
        self._phase='failed'
        boundary=self._completed_scan
        if type(boundary) is _CompletedScanBoundary:boundary.closed=True
        for _,authority,_,_ in self._run_pins:
            try:authority.close(self._original_token)
            except BaseException:pass
        if self._pending_authority is not None:
            try:self._pending_authority.close(self._original_token)
            except BaseException:pass

    def _scope_check(self, *, capture_scope=None):
        """Callback-free checks, safe inside physical publication/capture guards."""
        try:
            require(self._phase not in ('closed','failed') and self._end==self._original_end
                    and time.monotonic_ns()<self._original_end and self._callback is self._original_callback
                    and self._plan_identity.checked_bytes(self._plan,self._plan_raw)==self._plan_raw
                    and self._budget_identity.checked_bytes(self._budget,self._budget_raw)==self._budget_raw)
            require((self._runtime.resource_worker,self._runtime.resource_worker_sha256,self._workers.pins)==self._runtime_pin)
            require(tuple(getattr(self._runtime,name) for name in ('kagami','cli','daemon','resource_program'))
                    ==tuple(row[0] for row in self._images))
            for image,path,fd,identity,digest,uuids in self._images:
                require((image.path,image.fd,image.identity,image.sha256,image.uuids)==(path,fd,identity,digest,uuids))
                image.validate()
            if self._directories is not None:self._directories.validate()
            if self._controls is not None:self._controls.check_namespace()
            require((self._active is None)==(self._active_pin is None))
            if self._active_pin is not None:
                slot,index,created,deadline=self._active_pin
                pair,variant=RUN_KEYS[index]
                expected=TrialPaths(self._directories.evidence,self._directories.runtime,pair,variant)
                require(self._active is slot and slot._owner is self and slot._index==index
                        and type(slot._paths) is TrialPaths and slot._paths==expected and slot._trial is created)
                if created is not None:
                    require(created._deadline==created._original_deadline==deadline and created._phase!='failed')
                    if self._phase=='closing_trial':
                        require(self._pending_authority is not None and created._phase in ('complete','closed'))
                        self._pending_authority.check_provenance(self._token)
                    else:
                        require(created._phase!='closed' and time.monotonic_ns()<deadline)
            require(tuple((item.handle,item.authority,item.files,item.captures) for item in self._runs)==self._run_pins)
            selected=None
            if self._completed_scan is not None:
                require(type(self._completed_scan) is _CompletedScanBoundary)
                selected=_CompletedScanBoundary.check(self._completed_scan,self,capture_scope)
            for item in self._runs:
                item.authority.check_provenance(self._token)
                if selected is None or item.files is selected[2]:
                    item.files._check();item.captures.check_namespace()
            if self._active_public is not None:self._active_public._check()
            if self._active_captures is not None:self._active_captures.check_namespace()
            self._scalar_pins()
        except BaseException:
            self._poison();raise

    def _scalar_pins(self):
        # Publication installs the original report while still replayed, then
        # advances to reported only after its final guard. A reported phase can
        # never substitute for that original publication.
        require(self._token is self._original_token
                and (self._phase!='reported' or self._report is not None)
                and self._manifest is self._manifest_pin and self._report is self._report_pin
                and type(self._replayed) is tuple and self._replayed is self._replayed_pin)
        if self._replay_pin is not None:
            instance,token,pin=self._replay_pin
            require(self._replay is instance and instance._owner is self and instance._token is token
                    and instance._scope_pin is pin)
            if self._replay_released:
                require(self._phase=='reported' and instance._phase in ('complete','closed'))
            else:require(instance._phase not in ('failed','closed'))

    def _runtime_guard(self):
        require(not self._in_runtime)
        self._scope_check();self._in_runtime=True
        try:self._callback()
        finally:self._in_runtime=False
        self._scope_check()

    def _full_check(self):
        boundary=None
        try:
            require(self._completed_scan is None)
            self._runtime_guard()
            if self._controls is not None:self._controls.verify()
            # Each selected scan keeps every scanner callback and all live
            # global checks. Only repeated sibling file/namespace checks move
            # to this full pass and its final all-run metadata fence. The
            # selection cannot authorize another capture, callback or pass.
            if self._runs:
                boundary=_CompletedScanBoundary(self)
                directories=tuple((path,_identity(os.fstat(self._directories._rows[path][0])))
                                  for path in self._directories._managed)
                self._completed_scan=boundary
            for index,item in enumerate(self._runs):
                require(boundary.completed==index and boundary.selected is None)
                boundary.selected=boundary.runs[index]
                item.authority.validate(self._token)
                require(self._completed_scan is boundary and not boundary.closed
                        and type(boundary.completed) is int and boundary.completed==index
                        and boundary.selected is boundary.runs[index])
                boundary.selected=None;boundary.completed+=1
            self._completed_scan=None
            self._scope_check()
            self._semantic_pins()
            if boundary is not None:_CompletedScanBoundary.check_directories(boundary,self,directories)
            self._metadata_fence()
        except BaseException:
            self._poison();raise
        finally:
            if boundary is not None:
                boundary.closed=True
                self._completed_scan=None

    def _semantic_pins(self):
        # One event-level check after all external callbacks and content reads.
        # Physical per-file guards stay nonrecursive and contain no measurement
        # traversal. No callback or body read follows this terminal value check.
        self._scalar_pins()
        expected=self._phase in ('replayed','reported')
        if self._replay_pin is not None:
            instance,_,scope=self._replay_pin
            require(instance._scope_identity()==scope)
            expected=expected or instance._phase in ('finishing','complete','closed')
        if expected:
            from scaling_experiment_final_projection import measurement_identity
            require(self._replay_pin is not None and self._replay_results is not None
                    and self._measurements is not None and self._measurement_pin is not None
                    and type(self._replay_results_pin) is tuple and len(self._replay_results_pin)==10
                    and self._replay_pin[0]._results is self._replay_results
                    and self._replay_pin[0]._result_identity()==self._replay_results_pin
                    and measurement_identity(self._measurements)==self._measurement_pin)
        else:
            require(self._replay_results is None and self._replay_results_pin is None
                    and self._measurements is None and self._measurement_pin is None)

    def _metadata_fence(self):
        # No callback/body read follows this final complete metadata fence.
        before=tuple((path,_identity(os.fstat(self._directories._rows[path][0])))
                     for path in self._directories._managed) if self._directories is not None else ()
        for item in self._runs:item.captures.check_metadata()
        if self._active_captures is not None:self._active_captures.check_metadata()
        for item in self._runs:item.files._check()
        if self._active_public is not None:self._active_public._check()
        if self._controls is not None:self._controls._base_check()
        for path,identity in before:
            fd,parent,name,owner=self._directories._rows[path]
            require(_identity(os.fstat(fd))==identity
                    and _directory_owner(os.stat(name,dir_fd=parent,follow_symlinks=False))==owner)
        self._scalar_pins()
        require(self._phase not in ('failed','closed') and self._end==self._original_end
                and time.monotonic_ns()<self._original_end)

    def _enter(self, phase):
        require(not self._busy and self._phase==phase)
        self._busy=True;self._full_check()

    def begin_run(self, pair_index: int, variant: str) -> FixedRunSlot:
        """Create only the next fixed public/private run namespace, once."""
        try:
            self._enter('ready')
            require(self._active is None and len(self._runs)<10
                    and all(item.published for item in self._runs)
                    and type(pair_index) is int and type(variant) is str
                    and (pair_index,variant)==RUN_KEYS[len(self._runs)])
            self._directories.create_run(pair_index,variant)
            slot=object.__new__(FixedRunSlot)
            slot._owner=self;slot._index=len(self._runs);slot._trial=None
            slot._paths=TrialPaths(self._directories.evidence,self._directories.runtime,pair_index,variant)
            self._active=slot
            self._active_pin=(slot,slot._index,None,None)
            allocation=select_run_budget(self._budget,pair_index,variant)
            self._active_public=RunPublicFiles(slot._paths.public,allocation)
            self._phase='slot';self._scope_check();self._busy=False
            return slot
        except BaseException as error:self._poison();failure(error)

    def _create_trial(self, slot):
        try:
            self._enter('slot')
            require(type(slot) is FixedRunSlot and slot is self._active
                    and slot._owner is self and slot._index==len(self._runs) and slot._trial is None)
            end=time.monotonic_ns()+self._plan.trial_timeout_ns
            require(end<self._original_end)
            pair,variant=RUN_KEYS[slot._index]
            paths=TrialPaths(self._directories.evidence,self._directories.runtime,pair,variant)
            instance=FixedTrial(self._plan.trials[slot._index],paths,
                select_run_budget(self._budget,pair,variant),self._runtime,self._reader,end,self._runtime_guard)
            slot._trial=instance
            self._active_pin=(slot,slot._index,instance,end)
            self._phase='trial';self._scope_check();self._busy=False
            return instance
        except BaseException as error:self._poison();failure(error)

    def _admit_trial(self, slot, trial):
        try:
            self._enter('trial')
            require(type(slot) is FixedRunSlot and slot is self._active and slot._owner is self
                    and slot._index==len(self._runs) and type(trial) is FixedTrial and slot._trial is trial)
            result=trial.validate()
            authority=CompletedRunAuthority.admit(trial,self._token)
            self._pending_authority=authority
            def timed_guard():
                self._scope_check();trial._base_guard();self._scope_check()
            public=self._active_public
            for role in ('collector_journal','transaction_trace'):
                artifact=getattr(result.load,role)
                public.adopt_existing(role,trial._load.public_descriptor(role),artifact.sha256,
                                      artifact.bytes,native_parent=None,guard=timed_guard)
            for role,native_role in _NATIVE_ROLES:
                artifact=trial._outputs.artifact(native_role)
                public.adopt_existing(role,trial._outputs.descriptor(native_role),artifact.sha256,
                    artifact.bytes,native_parent=trial._outputs.directory_descriptor(),guard=timed_guard)
            native=trial._generated.inputs
            artifacts={item.path:item for item in result.generation.artifacts}
            for role,name in _GENESIS:
                digest,size=(result.generation.anchors_sha256,result.generation.anchors_bytes) if name=='genesis-anchors.json' else (
                    artifacts[name].sha256,artifacts[name].bytes)
                public.copy_genesis(role,native.public_descriptor(name),digest,size,guard=timed_guard)
            public.seal_sources(guard=timed_guard)
            capture_scope=(slot._index,public)
            captures=trial._captures.transfer(
                lambda guard=self._scope_check:guard(capture_scope=capture_scope))
            self._active_captures=captures
            authority.commit(self._token,public,captures)
            projection=authority.projection(self._token)
            if projection.variant=='four_lane':
                preceding=self._runs[-1].authority.projection(self._token)
                require(tuple(item.account_id for item in preceding.generation.accounts)
                        ==tuple(item.account_id for item in projection.generation.accounts))
            trial.validate();self._full_check();timed_guard()
            self._phase='closing_trial';trial.close()
            self._directories.finish_run()
            self._full_check();authority.validate(self._token)
            self._scope_check();self._metadata_fence()
            handle=object.__new__(CompletedRunHandle)
            handle._owner=self;handle._index=len(self._runs)
            self._runs.append(_Run(handle,authority,public,captures))
            self._run_pins=(*self._run_pins,(handle,authority,public,captures))
            self._active=self._active_public=self._active_captures=self._pending_authority=None
            self._active_pin=None
            self._phase='completed';self._busy=False
            return handle
        except BaseException as error:self._poison();failure(error)

    def _member(self, handle):
        require(type(handle) is CompletedRunHandle and handle._owner is self
                and type(handle._index) is int and 0<=handle._index<len(self._runs))
        item=self._runs[handle._index]
        require(item.handle is handle)
        return item

    def projection(self, handle):
        """Read one original completed run while the entire experiment is valid."""
        try:
            require(not self._busy);self._busy=True;self._full_check()
            value=self._member(handle).authority.projection(self._token)
            self._full_check();self._busy=False
            return value
        except BaseException as error:self._poison();failure(error)

    def publish_run(self, handle):
        """Derive two bounded public summaries from the exact completed owner."""
        try:
            self._enter('completed');item=self._member(handle)
            require(item is self._runs[-1] and not item.published)
            value=item.authority.projection(self._token)
            allocation=item.files.allocation
            raw=raw_run(value,allocation.run.raw_run.max_bytes)
            item.files.publish_summary('raw_run',raw,guard=self._scope_check)
            receipt=run_receipt(value,hashlib.sha256(raw).hexdigest(),allocation.run.run_receipt.max_bytes)
            item.files.publish_summary('run_receipt',receipt,guard=self._scope_check)
            item.published=True
            self._full_check();self._phase='ready' if len(self._runs)<10 else 'runs_complete';self._busy=False
        except BaseException as error:self._poison();failure(error)

    def _manifest_records(self):
        from scaling_experiment_final_projection import RunManifest
        require(len(self._runs)==10 and all(item.published for item in self._runs))
        records=[]
        for item in self._runs:
            value=item.authority.projection(self._token)
            files=item.files.verify();by_role={row.role:row for row in files}
            require(len(files)==len(by_role)==len(RUN_FILE_FIELDS) and set(by_role)==set(RUN_FILE_FIELDS))
            records.append(RunManifest(value.pair_index,value.variant,value.original_deadline_ns,
                tuple(by_role[name] for name in RUN_FILE_FIELDS),item.captures.census))
        return tuple(records)

    def _derive_manifest(self):
        from scaling_experiment_final_projection import manifest_bytes
        return manifest_bytes(self._plan,self._budget,self._original_end,
            self._controls.controls[:3],self._manifest_records(),self._budget.control_budgets[0].max_bytes)

    def publish_manifest(self):
        """Publish only the manifest derived from all ten original completed runs."""
        try:
            self._enter('runs_complete');require(self._manifest is None and self._replay is None)
            raw=self._derive_manifest()
            binding=self._controls.publish_manifest(raw)
            self._manifest=self._manifest_pin=(binding,raw)
            self._full_check();self._phase='manifest';self._busy=False
            return binding
        except BaseException as error:self._poison();failure(error)

    def _create_replay(self):
        from resource_experiment import ResourceExperiment, RunReplayScope
        try:
            self._enter('manifest');require(self._replay is None and self._replay_pin is None)
            scopes=[]
            for index,item in enumerate(self._runs):
                value=item.authority.projection(self._token)
                pair,variant=RUN_KEYS[index]
                journal=next(row for row in item.files.verify() if row.role=='collector_journal')
                scopes.append(RunReplayScope(pair,variant,item.captures.directory,
                    self._directories.evidence/journal.binding.path,journal.binding.sha256,
                    tuple(ExpectedPeer(f'peer{i}',ProcessIdentity(*peer)) for i,peer in enumerate(value.peers)),
                    ReplayGeometry(*value.geometry),select_run_budget(self._budget,pair,variant)))
            instance=object.__new__(ResourceExperiment);token=object()
            instance._initialize(self,token,tuple(scopes))
            self._replay=instance;self._replay_pin=(instance,token,instance._scope_pin)
            self._full_check();self._busy=False
            return instance
        except BaseException as error:self._poison();failure(error)

    def _replay_origin(self, instance, token, phases):
        from resource_experiment import ResourceExperiment
        require(type(instance) is ResourceExperiment and self._replay_pin is not None)
        original,original_token,pin=self._replay_pin
        require(instance is original is self._replay and type(token) is object and token is original_token
                and instance._owner is self and instance._token is token and instance._scope_pin is pin
                and instance._phase in phases and not self._replay_released
                and instance._scope_identity()==pin)

    def _replay_before(self, instance, token):
        try:
            self._enter('manifest');self._replay_origin(instance,token,('replaying',))
            require(instance._busy and not self._replayed and self._replay_results is None)
            self._phase='replaying'
        except BaseException as error:self._poison();failure(error)

    def _replay_accept(self, instance, token, index, reduced):
        """Join the next actual replay to that run's still-open native authority."""
        try:
            require(self._busy and self._phase=='replaying')
            self._replay_origin(instance,token,('replaying',))
            require(instance._busy and type(index) is int and index==len(self._replayed)<10
                    and type(reduced) is ReplayResult)
            self._full_check()
            value=self._runs[index].authority.reconcile(self._token,reduced)
            require((value.pair_index,value.variant)==RUN_KEYS[index])
            self._replayed=self._replayed_pin=(*self._replayed,value)
            self._full_check()
            return value
        except BaseException as error:self._poison();failure(error)

    def _replay_finish(self, instance, token, results):
        from resource_experiment import RunResourceResult
        from scaling_measurements import measure_experiment
        from scaling_experiment_final_projection import measurement_identity
        try:
            require(self._busy and self._phase=='replaying')
            self._replay_origin(instance,token,('finishing',))
            require(instance._busy and type(results) is tuple and results is instance._results
                    and len(results)==len(self._replayed)==10)
            for key,row,value in zip(RUN_KEYS,results,self._replayed,strict=True):
                require(type(row) is RunResourceResult and (row.pair_index,row.variant)==key
                        and row.resources is value.resources)
            measured=measure_experiment(self._plan,self._replayed)
            require(all(row.maxima==metric.observed_resources for row,metric in zip(results,measured.runs,strict=True)))
            self._replay_results=results;self._replay_results_pin=instance._result_identity()
            self._measurements=measured;self._measurement_pin=measurement_identity(measured)
            self._full_check();self._phase='replayed';self._busy=False
        except BaseException as error:self._poison();failure(error)

    def _measurement_check(self, instance):
        from scaling_measurements import measure_experiment
        from scaling_experiment_final_projection import measurement_identity
        self._scalar_pins()
        require(instance._results is self._replay_results and self._replay_results is not None
                and instance._result_identity()==self._replay_results_pin
                and measurement_identity(self._measurements)==self._measurement_pin
                and measurement_identity(measure_experiment(self._plan,self._replayed))==self._measurement_pin)

    def _verify_replay(self, instance, token):
        try:
            require(not self._busy and self._phase in ('replayed','reported'));self._busy=True
            self._replay_origin(instance,token,('complete',));require(instance._busy)
            self._full_check();self._measurement_check(instance);self._full_check();self._busy=False
        except BaseException as error:self._poison();failure(error)

    def _read_derived(self, role, stored, expected):
        require(stored is not None and type(expected) is bytes and stored[1]==expected)
        binding,raw=stored
        cap=next(item.max_bytes for item in self._budget.control_budgets if item.label==role)
        require(self._controls.read_control(binding,max_bytes=cap)==raw)

    def publish_report(self, instance):
        """Publish measured results only after all ten original replays reconcile."""
        from scaling_experiment_final_projection import report_bytes
        try:
            require(self._replay_pin is not None and instance is self._replay_pin[0])
            results=instance.verify()
            self._enter('replayed');self._replay_origin(instance,self._replay_pin[1],('complete',))
            require(not instance._busy and results is self._replay_results and self._report is None)
            self._measurement_check(instance)
            self._read_derived('manifest',self._manifest,self._derive_manifest())
            raw=report_bytes(self._manifest[0].sha256,self._measurements,self._budget.control_budgets[1].max_bytes)
            binding=self._controls.publish_report(raw)
            self._report=self._report_pin=(binding,raw)
            self._full_check();self._phase='reported';self._busy=False
            return binding
        except BaseException as error:self._poison();failure(error)

    def verify_report(self):
        """Read back both derived controls while all original owners remain open."""
        from scaling_experiment_final_projection import report_bytes
        try:
            require(self._replay_pin is not None)
            self._replay_pin[0].verify()
            self._enter('reported');self._measurement_check(self._replay_pin[0])
            self._read_derived('manifest',self._manifest,self._derive_manifest())
            self._read_derived('report',self._report,report_bytes(self._manifest[0].sha256,
                self._measurements,self._budget.control_budgets[1].max_bytes))
            self._full_check();self._busy=False
            return self._report[0]
        except BaseException as error:self._poison();failure(error)

    def _release_replay(self, instance, token):
        try:
            self._enter('reported');self._replay_origin(instance,token,('complete',))
            require(instance._busy);self._measurement_check(instance);self._full_check()
            self._replay_released=True;self._busy=False
        except BaseException as error:self._poison();failure(error)

    def _reject_replay(self, instance, token):
        # Reentry, a foreign borrower, an incomplete close or any replay failure
        # invalidates the original experiment; no replacement can re-admit it.
        self._poison()

    def close(self):
        """Close owned evidence only after any actual trial's children are reaped."""
        if self._phase=='closed':return
        try:
            if self._active_pin is not None and self._active_pin[2] is not None:self._active_pin[2].close()
            self._poison()
            for _,_,files,captures in reversed(self._run_pins):captures.close();files.close()
            if self._active_captures is not None:self._active_captures.close()
            if self._active_public is not None:self._active_public.close()
            if self._controls is not None:self._controls.close()
            if self._directories is not None:self._directories.close()
            self._phase='closed'
        except BaseException as error:self._poison();failure(error)
