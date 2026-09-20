"""Fixed first-release single-trial native lifecycle and retained evidence.

All native operations and final original-custody checks finish before one
unchanged trial deadline. The returned intermediate result is backed by this
owner until explicit close. The outer experiment must transfer public-artifact
custody before closing; this module never emits a release verdict or public
report, adopts a supplied trial command, or interprets Norito.
"""
from dataclasses import dataclass, fields, is_dataclass
from fractions import Fraction
import hashlib
import os
from pathlib import Path
import stat
import time

from resource_bundle import _directory_owner, _DIRECTORY_FLAGS, _root_path
from resource_evidence_budget import PerRunResourceBudget, validate_run_budget, canonical_run_budget_bytes
from resource_process import ExecutableImage
from resource_replay import ExpectedPeer, ReplayGeometry, ReplayResult, replay
from scaling_generator import GeneratorPlan, GenerationReceipt, FixedGenerator
from scaling_launcher import PeerLaunch, PeerLaunchInputs, FourPeerRun, blake3
from scaling_readiness import FourPeerReadiness, ReadyReceipt
from scaling_native_load import NativeLoadPlan, NativeLoadReceipt, FixedNativeLoad
from scaling_load_outputs import LoadPaths
from scaling_native_outputs import NativeOutputBudget, NativeOutputs, _budget as output_budget_snapshot
from scaling_native_facts_inputs import (ReaderBudget, FactsBudget, FactsJournalPlan, JournalInput,
    reader_snapshot, budget_snapshot)
from scaling_native_facts import NativeFacts, StoppedTipReceipt, FactsReceipt
from scaling_vector_collection import (CollectionLimits, NativeVectorCollection, VectorCollectionReceipt,
    _limits as collection_snapshot)
from scaling_replayed_workload import build_replay_plan
from scaling_proof_sequence import NativeProofSequence
from scaling_canonical_proof import CanonicalReplayReceipt
from scaling_trial_captures import TrialCaptures, CaptureCensus
from scaling_command import MAX_TRIAL_NS


class FixedTrialError(ValueError):
    """Closed trial failure; incomplete owners remain available for cleanup."""


def require(value):
    if not value:
        raise FixedTrialError('fixed_trial_failed')


def failure(error):
    if isinstance(error,KeyboardInterrupt):raise KeyboardInterrupt() from None
    if isinstance(error,SystemExit):raise SystemExit(1) from None
    if isinstance(error,GeneratorExit):raise GeneratorExit() from None
    raise FixedTrialError('fixed_trial_failed') from None


def fingerprint(value):
    """Hash admitted immutable structures without materializing signed-body reprs."""
    digest=hashlib.sha256()
    def visit(item,depth):
        require(depth<=24)
        if item is None:digest.update(b'n;')
        elif type(item) is bool:digest.update(b'b1;' if item else b'b0;')
        elif type(item) is int:digest.update(b'i'+str(item).encode()+b';')
        elif type(item) is str:
            raw=item.encode();digest.update(b's'+str(len(raw)).encode()+b':');digest.update(raw)
        elif type(item) is bytes:
            digest.update(b'x'+str(len(item)).encode()+b':');digest.update(item)
        elif type(item) is type(Path('/')):digest.update(b'p');visit(str(item),depth+1)
        elif type(item) is tuple:
            digest.update(b't'+str(len(item)).encode()+b':')
            for child in item:visit(child,depth+1)
        elif is_dataclass(item) and not isinstance(item,type):
            digest.update(b'd');visit(type(item).__module__+'.'+type(item).__qualname__,depth+1)
            for field in fields(item):visit(field.name,depth+1);visit(getattr(item,field.name),depth+1)
        else:require(False)
    visit(value,0)
    return digest.hexdigest()


@dataclass(frozen=True, slots=True)
class TrialPlan:
    """Original generation, workload and every independently admitted native bound."""
    generator: GeneratorPlan
    load: NativeLoadPlan
    reader: ReaderBudget
    collection: CollectionLimits
    facts: FactsBudget
    native_outputs: NativeOutputBudget
    replay_reply_max_bytes: int
    stop_timeout_ns: int


@dataclass(frozen=True, slots=True)
class TrialPaths:
    """Two disjoint original roots; all trial names follow the fixed public map."""
    evidence_root: Path
    runtime_root: Path
    pair_index: int
    variant: str

    @property
    def public(self):return self.evidence_root/'runs'/f'pair-{self.pair_index:02}'/self.variant
    @property
    def private(self):return self.runtime_root/f'pair-{self.pair_index:02}'/self.variant
    @property
    def generated(self):return self.private/'generated'
    @property
    def native(self):return self.public/'native'
    @property
    def captures(self):return self.evidence_root/'resources'/f'pair-{self.pair_index:02}'/self.variant
    @property
    def load(self):return LoadPaths(self.private/'resource-config.json',self.captures,
                                   self.public/'trace.json',self.public/'collector.jsonl')


@dataclass(frozen=True, slots=True)
class TrialRuntime:
    """Borrowed original images and source; caller owns complete runtime closure."""
    kagami: ExecutableImage
    cli: ExecutableImage
    daemon: ExecutableImage
    resource_program: ExecutableImage
    resource_worker: Path
    resource_worker_sha256: str


@dataclass(frozen=True, slots=True)
class TrialResult:
    """Completed scoped evidence; outer publication and ten-run verdict remain open."""
    pair_index: int
    variant: str
    generation: GenerationReceipt
    load: NativeLoadReceipt
    readiness: tuple[ReadyReceipt, ...]
    resources: ReplayResult
    capture_census: CaptureCensus
    stopped_tip: StoppedTipReceipt
    vectors: VectorCollectionReceipt
    facts: FactsReceipt
    canonical: CanonicalReplayReceipt
    original_deadline_ns: int


class _Directories:
    """Retain the three exact pre-created trial parents before any native action."""
    def __init__(self,paths):
        self._rows={}
        try:
            for path in (paths.public,paths.private,paths.captures.parent,paths.evidence_root,paths.runtime_root):
                parent=None;current=Path('/')
                for index,name in enumerate(path.parts):
                    current=Path('/') if index==0 else current/name
                    if current not in self._rows:
                        require(len(self._rows)<160)
                        fd=os.open(name,_DIRECTORY_FLAGS,dir_fd=parent)
                        try:
                            info=os.fstat(fd);require(stat.S_ISDIR(info.st_mode))
                            owner=_directory_owner(info)
                            require(_directory_owner(os.stat(name,dir_fd=parent,follow_symlinks=False))==owner)
                            self._rows[current]=(fd,parent,name,owner)
                        except BaseException:os.close(fd);raise
                    parent=self._rows[current][0]
                info=os.fstat(parent)
                require(info.st_uid==os.geteuid() and stat.S_IMODE(info.st_mode)==0o700)
            self.validate()
        except BaseException:self.close();raise
    def validate(self):
        require(bool(self._rows))
        for fd,parent,name,owner in self._rows.values():
            require(_directory_owner(os.fstat(fd))==owner
                    and _directory_owner(os.stat(name,dir_fd=parent,follow_symlinks=False))==owner)
    def close(self):
        pending={}
        for path,row in reversed(tuple(self._rows.items())):
            fd,_,_,expected=row
            try:
                require(_directory_owner(os.fstat(fd))==expected)
                os.close(fd)
            except (OSError,FixedTrialError):pending[path]=row
        self._rows=dict(reversed(tuple(pending.items())))
        require(not pending)


class FixedTrial:
    """Finite direct composition; no native stage or failure can be retried here."""
    def __init__(self,plan:TrialPlan,paths:TrialPaths,allocation:PerRunResourceBudget,
                 runtime:TrialRuntime,process_reader,trial_deadline_ns:int,verify_runtime):
        self._phase='admitting';self._directories=None;self._result=None
        self._generator=None;self._generated=None;self._peer_inputs=None;self._launch=None
        self._readiness=None;self._load=None;self._captures=None;self._outputs=None
        self._facts=None;self._vectors=None;self._proof=None
        self._in_runtime=False;self._closing_custody=False;self._terminal_pins=[]
        try:
            require(type(plan) is TrialPlan and type(paths) is TrialPaths and type(runtime) is TrialRuntime
                    and type(allocation) is PerRunResourceBudget and callable(verify_runtime)
                    and callable(getattr(process_reader,'sample',None)))
            require(type(trial_deadline_ns) is int and 0<trial_deadline_ns-time.monotonic_ns()<=MAX_TRIAL_NS)
            require(type(plan.generator) is GeneratorPlan and type(plan.load) is NativeLoadPlan
                    and type(plan.reader) is ReaderBudget and type(plan.collection) is CollectionLimits
                    and type(plan.facts) is FactsBudget and type(plan.native_outputs) is NativeOutputBudget)
            plan.generator.validate();validate_run_budget(allocation)
            reader_snapshot(plan.reader);budget_snapshot(plan.facts)
            collection_snapshot(plan.collection);output_budget_snapshot(plan.native_outputs)
            require(type(plan.stop_timeout_ns) is int and 0<plan.stop_timeout_ns<=300_000_000_000
                    and type(plan.replay_reply_max_bytes) is int and 0<plan.replay_reply_max_bytes<=256*1024*1024)
            require(type(paths.pair_index) is int and 1<=paths.pair_index<=5
                    and type(paths.variant) is str and paths.variant in ('one_lane','four_lane'))
            for root in (paths.evidence_root,paths.runtime_root):_root_path(root)
            require(paths.evidence_root!=paths.runtime_root and paths.evidence_root not in paths.runtime_root.parents
                    and paths.runtime_root not in paths.evidence_root.parents)
            require((paths.pair_index,paths.variant)==(plan.load.pair_index,plan.load.variant)
                    and plan.generator.lane_count=={'one_lane':1,'four_lane':4}[paths.variant])
            self._scheduled,_=plan.load.validate(plan.generator.account_count,allocation)
            # The first-release complete ledger has fifteen named roles; no support bucket.
            mapping={'finality':'native_finality','queries':'native_queries','facts':'native_facts',
                     'request':'native_request','bundle':'native_bundle','proof':'canonical_proof'}
            caps=tuple(getattr(allocation.run,mapping[name]).max_bytes for name in mapping)
            require(tuple(getattr(plan.native_outputs,name) for name in mapping)==caps
                    and plan.native_outputs.total==sum(caps))
            self._plan,self._paths,self._allocation,self._runtime=plan,paths,allocation,runtime
            self._plan_pin=fingerprint((plan,paths));self._budget=canonical_run_budget_bytes(allocation)
            self._reader,self._callback=process_reader,verify_runtime
            self._deadline=self._original_deadline=trial_deadline_ns
            images=(runtime.kagami,runtime.cli,runtime.daemon,runtime.resource_program)
            require(all(isinstance(image,ExecutableImage) for image in images))
            self._images=tuple((image,image.path,image.fd,image.identity,image.sha256,image.uuids) for image in images)
            self._runtime_pin=(runtime.resource_worker,runtime.resource_worker_sha256)
            self._directories=_Directories(paths)
            self._phase='admitted';self._base_guard()
        except BaseException as error:
            self._phase='failed'
            if self._directories is not None:self._directories.close()
            failure(error)

    @property
    def original_deadline_ns(self):return self._original_deadline

    def _base_check(self):
        require(self._phase not in ('failed','closed') and self._deadline==self._original_deadline
                and time.monotonic_ns()<self._original_deadline
                and fingerprint((self._plan,self._paths))==self._plan_pin
                and canonical_run_budget_bytes(self._allocation)==self._budget
                and (self._runtime.resource_worker,self._runtime.resource_worker_sha256)==self._runtime_pin)
        require((self._runtime.kagami,self._runtime.cli,self._runtime.daemon,self._runtime.resource_program)
                ==tuple(row[0] for row in self._images))
        for image,path,fd,identity,digest,uuids in self._images:
            require((image.path,image.fd,image.identity,image.sha256,image.uuids)==(path,fd,identity,digest,uuids))
            image.validate()
        self._directories.validate()

    def _base_guard(self):
        require(not self._in_runtime)
        self._base_check()
        if self._closing_custody:return
        self._in_runtime=True
        try:self._callback()
        finally:self._in_runtime=False
        self._base_check()

    def _generated_guard(self):
        self._base_guard()
        require(self._generated is not None)
        self._generated.validate()
        if self._peer_inputs is not None:self._peer_inputs.validate(self._peers)
        self._base_check()

    def _load_guard(self):
        self._generated_guard()
        if self._load is not None and self._load._phase=='complete':self._load.validate()
        self._base_check()

    def _native_guard(self):
        self._load_guard()
        require(self._launch is not None)
        if self._phase=='collecting':
            require(self._launch._stopped=={3})
            require(self._launch._child_handle(3).returncode==0)
            self._launch._live((0,1,2))
        else:
            require(self._phase in ('facts','proof','finalizing','complete'))
            self._launch.verify_stopped()
        if self._captures is not None:self._captures.check_namespace()
        self._base_check()

    def _capture_guard(self):
        # Full generated/load/runtime custody brackets each whole scan. Per-file
        # progress checks preserve the same deadline and original namespaces
        # without recursively hashing every other evidence owner per member.
        require(self._phase not in ('failed','closed') and self._deadline==self._original_deadline
                and time.monotonic_ns()<self._original_deadline)
        self._directories.validate()

    def _geometry(self):
        p=self._plan.load
        return ReplayGeometry(p.warmup_ns,p.measurement_ns,p.drain_ns,p.preparation_ahead_ms*1_000_000,
            p.resource_interval_ms*1_000_000,p.resource_timeout_ms*1_000_000,p.resource_max_start_lag_ms*1_000_000)

    def _journal_plan(self):
        p=self._plan.load;rate=Fraction(p.offered_load_tps)
        return FactsJournalPlan(p.seed,p.pair_index,rate.numerator,rate.denominator,p.warmup_ns,p.measurement_ns,
            p.drain_ns,p.submission_lag_ns,p.preparation_lookahead,p.preparation_concurrency,
            p.preparation_ahead_ms*1_000_000,p.max_submissions,p.max_in_flight,p.max_status_requests,
            p.poll_interval_ms*1_000_000,self._scheduled,p.resource_interval_ms*1_000_000,
            p.resource_timeout_ms*1_000_000,p.resource_max_start_lag_ms*1_000_000)

    def _stop_deadline(self):
        self._base_check()
        return min(self._original_deadline,time.monotonic_ns()+self._plan.stop_timeout_ns)

    def run(self,development_seed:str)->TrialResult:
        try:
            require(self._phase=='admitted');self._phase='generating';self._base_guard()
            runtime=self._runtime;end=self._original_deadline
            self._generator=FixedGenerator(self._plan.generator,self._paths.generated,runtime.kagami,
                self._reader,end,self._base_guard)
            self._generated=self._generator.generate(development_seed)
            del development_seed
            native=self._generated.inputs
            generation=self._generated.receipt
            self._terminal_pins.append((generation,fingerprint(generation)))
            self._peers=tuple(PeerLaunch(role.peer_id,role.node_config,
                blake3.blake3(role.node_config.read_bytes()).hexdigest(),role.block_store) for role in native.roles)
            self._generated_guard()
            self._peer_inputs=PeerLaunchInputs(self._peers)
            self._launch=FourPeerRun(self._peers,runtime.daemon,self._reader,self._peer_inputs,self._generated_guard,end)
            self._outputs=NativeOutputs(self._paths.native,self._plan.native_outputs)
            self._phase='launching';pins=self._launch.launch_owned()
            self._readiness=FourPeerReadiness(native,runtime.cli,self._reader,end,self._generated_guard)
            self._phase='readiness';self._ready_receipts=self._launch.await_genesis_ready(self._readiness)
            self._ready_pin=fingerprint(self._ready_receipts)
            self._phase='loading'
            def load(original_pins):
                require(original_pins==pins)
                self._load=FixedNativeLoad(self._plan.load,native,original_pins,self._allocation,self._paths.load,
                    runtime.cli,self._reader,runtime.resource_program,runtime.resource_worker,runtime.resource_worker_sha256,
                    end,self._generated_guard,lambda:self._launch._live((0,1,2,3)))
                return self._load.run()
            loaded=self._launch.run_load(load)
            self._terminal_pins.append((loaded,fingerprint(loaded)))
            self._phase='resource-replay';self._load_guard()
            self._captures=TrialCaptures(self._paths.captures,self._allocation,self._capture_guard)
            resources=replay(self._paths.captures,loaded.collector_journal.path,loaded.collector_journal.sha256,
                tuple(ExpectedPeer(peer.peer_id,identity) for peer,identity in zip(self._peers,loaded.peers,strict=True)),
                self._geometry(),expected_policy=self._allocation.policy,allocation=self._allocation)
            self._captures.verify();self._load_guard()
            require(resources.journal_sha256==loaded.collector_journal.sha256
                    and resources.journal_bytes==loaded.collector_journal.bytes
                    and len(resources.signed_requests)==loaded.scheduled_requests
                    and resources.capture_file_count==self._captures.census.files
                    and resources.capture_bytes==self._captures.census.bytes)
            self._resources=resources;self._resource_pin=fingerprint(resources)
            self._phase='stopping-reader';self._launch.stop_peer3(self._stop_deadline())
            self._phase='collecting'
            def collect(peer3,peer0):
                require(peer3 is self._peers[3] and peer0 is pins[0])
                journal=JournalInput(loaded.collector_journal.path,loaded.collector_journal.sha256,
                    loaded.collector_journal.bytes,self._allocation.journal.max_bytes)
                self._facts=NativeFacts(native,self._outputs,journal,self._plan.reader,self._journal_plan(),
                    self._plan.facts,runtime.kagami,self._reader,end,self._native_guard)
                tip=self._facts.observe_tip()
                self._vectors=NativeVectorCollection(native,self._outputs,tip.reader,self._plan.collection,
                    runtime.cli,self._reader,end,self._native_guard)
                vectors=self._vectors.run()
                return tip,vectors
            tip,vectors=self._launch.collect_inputs(collect)
            self._terminal_pins.extend((value,fingerprint(value)) for value in (tip,vectors))
            self._phase='stopping-survivors';self._launch.stop_survivors(self._stop_deadline())
            self._phase='facts';self._captures.verify();facts=self._facts.produce_facts()
            self._terminal_pins.append((facts,fingerprint(facts)))
            require(fingerprint(resources)==self._resource_pin)
            proof_plan=build_replay_plan(resources,native,self._journal_plan(),tip.reader)
            self._proof_plan=proof_plan;self._proof_plan_pin=fingerprint(proof_plan)
            self._phase='proof';self._native_guard();self._captures.verify()
            self._proof=NativeProofSequence(self._outputs,tip.reader,runtime.kagami,self._reader,end,
                self._plan.replay_reply_max_bytes,self._native_guard)
            canonical=self._proof.run(proof_plan)
            self._terminal_pins.append((canonical,fingerprint(canonical)))
            self._phase='finalizing';self._final_custody()
            result=TrialResult(self._paths.pair_index,self._paths.variant,generation,loaded,self._ready_receipts,
                resources,self._captures.census,tip,vectors,facts,canonical,end)
            self._result=result;self._result_pin=fingerprint(result);self._phase='complete'
            self._base_check()
            return result
        except BaseException as error:self._phase='failed';failure(error)

    def _final_custody(self):
        require(not self._closing_custody)
        # The mandatory external runtime check precedes a closed batch of all
        # original-owner checks. Nested guards still check original inputs and
        # clock, but cannot invoke another external callback after an earlier
        # capture or artifact has completed its final scan.
        self._base_guard();self._closing_custody=True
        try:
            self._native_guard();self._load.validate();self._facts.validate();self._vectors.validate();self._outputs.validate()
            self._captures.verify()
            require(fingerprint(self._resources)==self._resource_pin
                    and fingerprint(self._proof_plan)==self._proof_plan_pin
                    and fingerprint(self._ready_receipts)==self._ready_pin
                    and len(self._terminal_pins)==6
                    and all(fingerprint(value)==pin for value,pin in self._terminal_pins))
            self._base_check()
        finally:self._closing_custody=False

    def validate(self)->TrialResult:
        """Recheck before the same deadline, while outer artifact handoff is pending."""
        try:
            require(self._phase=='complete' and self._result is not None)
            self._final_custody()
            require(fingerprint(self._result)==self._result_pin)
            return self._result
        except BaseException as error:self._phase='failed';failure(error)

    def handoff(self, slot):
        """Transfer only to the fixed experiment slot which created this trial."""
        from scaling_experiment_custody import FixedRunSlot
        try:
            require(type(slot) is FixedRunSlot)
            return slot._admit_trial(self)
        except BaseException as error:self._phase='failed';failure(error)

    def cleanup(self,deadline_ns:int)->tuple[str,...]:
        """Fail permanently and reap only retained original children, preserving files."""
        self._phase='failed';pending=[];interrupt=None
        for role,owner in (('proof',self._proof),('vectors',self._vectors),('facts',self._facts),
                           ('load',self._load),('peers',self._launch),('generator',self._generator)):
            if owner is None:continue
            try:pending.extend(f'{role}:{item}' for item in owner.cleanup(deadline_ns))
            except BaseException as error:
                pending.append(role)
                if not isinstance(error,Exception) and interrupt is None:interrupt=error
        if interrupt is not None:failure(interrupt)
        return tuple(pending)

    def close(self):
        """Release only trial-owned descriptors after all original children are reaped."""
        try:
            if self._phase=='closed':return
            # Verify every original child before closing any borrowed-dependent owner.
            commands=[owner._commands for owner in (self._proof,self._vectors,self._facts,self._load,self._generator)
                      if owner is not None and getattr(owner,'_commands',None) is not None]
            if self._proof is not None and self._proof._replay is not None:commands.append(self._proof._replay._commands)
            if self._readiness is not None:commands.append(self._readiness._commands)
            for command in commands:
                for child in command._children:
                    command._bound(child);require(child.process.poll() is not None);command._bound(child)
            if self._launch is not None:
                for index in range(len(self._launch._children)):
                    child=self._launch._child_handle(index);require(child.poll() is not None);self._launch._child_handle(index)
            errors=[]
            for owner in (self._captures,self._facts,self._load,self._outputs,self._peer_inputs,self._generator,self._directories):
                if owner is None:continue
                try:owner.close()
                except BaseException as error:errors.append(error)
            require(not errors);self._phase='closed'
        except BaseException as error:self._phase='failed';failure(error)
