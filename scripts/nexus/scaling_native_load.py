"""Fixed native tx-load invocation with original peers, inputs and allocations.

Requires retained CLI/worker interpreter images, authenticated worker source and
an explicit runtime-dependency verifier. All four original peers must remain live
through collection. This owner returns artifact custody only: resource, Applied,
canonical-proof and throughput acceptance remain with their joined replay owners.
"""
from dataclasses import dataclass, fields
from fractions import Fraction
import json
import re
import secrets
import time
from pathlib import Path

from resource_process import ExecutableImage, PinnedProcess, ProcessIdentity
from resource_evidence_budget import (PerRunResourceBudget, validate_run_budget,
    run_budget_inputs, canonical_run_budget_bytes, run_budget_sha256)
from scaling_command import BoundedCommand, MAX_TRIAL_NS
from scaling_readiness_inputs import ReadinessInputs
from scaling_load_outputs import LoadPaths, LoadFiles, LoadArtifact, path_check

MAX_REPLY_BYTES = 4096
SHA = re.compile(r'[0-9a-f]{64}')
REPLY_FIELDS = {'version','operation','invocation_id','pair_index','variant','seed',
    'resource_budget_sha256','scheduled_requests','collector_journal_sha256',
    'collector_journal_bytes','trace_sha256','trace_bytes'}


class NativeLoadError(ValueError):
    """Closed public load failure; never includes config, child output or URL text."""


def require(value):
    if not value: raise NativeLoadError('native_load_failed')


def fail(error):
    if isinstance(error,KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error,SystemExit): raise SystemExit(1) from None
    if isinstance(error,GeneratorExit): raise GeneratorExit() from None
    raise NativeLoadError('native_load_failed') from None


def integer(value,low,high): require(type(value) is int and low<=value<=high)


def decimal_ns(value,units):
    whole,remainder=divmod(value,units)
    return str(whole) if not remainder else f'{whole}.{remainder:0{len(str(units))-1}d}'.rstrip('0')


@dataclass(frozen=True, slots=True)
class NativeLoadPlan:
    """Immutable full offered schedule and every native collector/resource bound."""
    pair_index: int
    variant: str
    seed: str
    offered_load_tps: str
    warmup_ns: int
    measurement_ns: int
    drain_ns: int
    submission_lag_ns: int
    preparation_lookahead: int
    preparation_concurrency: int
    preparation_ahead_ms: int
    max_submissions: int
    max_in_flight: int
    max_status_requests: int
    poll_interval_ms: int
    journal_capacity: int
    resource_interval_ms: int
    resource_timeout_ms: int
    resource_max_start_lag_ms: int

    def validate(self,accounts,allocation):
        integer(self.pair_index,1,5)
        require(type(self.variant) is str and self.variant in ('one_lane','four_lane'))
        require(type(self.seed) is str and SHA.fullmatch(self.seed))
        require(type(self.offered_load_tps) is str and len(self.offered_load_tps)<=64
                and re.fullmatch(r'(?:0|[1-9][0-9]*)(?:\.[0-9]+)?',self.offered_load_tps))
        rate=Fraction(self.offered_load_tps);require(0<rate<=1_000_000_000 and rate.numerator < 1<<128 and rate.denominator < 1<<128)
        for value in (self.warmup_ns,self.measurement_ns,self.drain_ns,self.submission_lag_ns):integer(value,0,(1<<63)-1)
        require(self.measurement_ns>0 and 0<self.drain_ns<=300_000_000_000
                and self.submission_lag_ns*4*rate<=1_000_000_000)
        integer(accounts,4,64);require(accounts%4==0)
        counts=tuple(-(-(duration*rate).numerator//((duration*rate).denominator*1_000_000_000)) for duration in (self.warmup_ns,self.measurement_ns))
        require(all(count%accounts==0 for count in counts) and 0<sum(counts)<=accounts*1024)
        for name,maximum in (('preparation_lookahead',4096),('preparation_concurrency',32),
                ('max_submissions',4096),('max_in_flight',16384),('max_status_requests',256),('journal_capacity',16384),
                ('preparation_ahead_ms',30000),('poll_interval_ms',10000)):
            integer(getattr(self,name),1,maximum)
        integer(self.resource_interval_ms,2,60000);integer(self.resource_timeout_ms,1,30000)
        integer(self.resource_max_start_lag_ms,0,15000)
        require(self.resource_timeout_ms*2<=self.resource_interval_ms and self.resource_max_start_lag_ms*4<=self.resource_interval_ms)
        geometry=allocation.geometry
        require((self.pair_index,self.variant)==(allocation.run.pair_index,allocation.run.variant)
                and geometry.peers==4 and geometry.interval_ns==self.resource_interval_ms*1_000_000
                and geometry.measurement_ns==self.measurement_ns and geometry.drain_ns==self.drain_ns)
        lifetime=self.warmup_ns+self.measurement_ns+2*self.drain_ns+(self.preparation_ahead_ms+3*self.resource_timeout_ms+self.resource_max_start_lag_ms)*1_000_000
        require(0<lifetime<=MAX_TRIAL_NS)
        return sum(counts),lifetime


@dataclass(frozen=True, slots=True)
class NativeLoadReceipt:
    """Terminal transport and original files; no measured rate or resource maxima."""
    invocation_id: str
    plan: NativeLoadPlan
    process: ProcessIdentity
    peers: tuple[ProcessIdentity,...]
    cli_sha256: str
    budget_sha256: str
    scheduled_requests: int
    transaction_trace: LoadArtifact
    collector_journal: LoadArtifact
    resource_capture_dir: Path


class FixedNativeLoad:
    """Single actual CLI load under one original deadline and complete input custody."""
    def __init__(self,plan:NativeLoadPlan,inputs:ReadinessInputs,peers:tuple[PinnedProcess,...],
            allocation:PerRunResourceBudget,paths:LoadPaths,cli:ExecutableImage,reader,
            resource_program:ExecutableImage,resource_worker:Path,resource_worker_sha256:str,
            trial_deadline_ns:int,verify_runtime,verify_all_peers_live):
        self._files=None;self._commands=None;self._phase='admitting';self._receipt=None
        self._borrowing_public=False
        try:
            require(type(plan) is NativeLoadPlan and type(inputs) is ReadinessInputs
                    and type(allocation) is PerRunResourceBudget and type(paths) is LoadPaths)
            require(isinstance(cli,ExecutableImage) and isinstance(resource_program,ExecutableImage)
                    and callable(verify_runtime) and callable(verify_all_peers_live))
            integer(trial_deadline_ns,1,(1<<63)-1)
            require(0<trial_deadline_ns-time.monotonic_ns()<=MAX_TRIAL_NS)
            require(type(resource_worker_sha256) is str and SHA.fullmatch(resource_worker_sha256))
            path_check(resource_worker)
            inputs.validate();validate_run_budget(allocation)
            require(type(peers) is tuple and len(peers)==4 and all(type(peer) is PinnedProcess for peer in peers))
            require(tuple(peer.peer_id for peer in peers)==tuple(role.peer_id for role in inputs.roles))
            require(len({peer.pid for peer in peers})==4)
            self._plan,self._inputs,self._peers,self._allocation=plan,inputs,peers,allocation
            self._plan_snapshot=tuple(getattr(plan,field.name) for field in fields(plan))
            self._budget=canonical_run_budget_bytes(allocation);self._budget_sha=run_budget_sha256(allocation)
            self._scheduled,lifetime=plan.validate(len(inputs.generation.accounts),allocation)
            require(lifetime<trial_deadline_ns-time.monotonic_ns())
            require(inputs.generation.lane_count=={'one_lane':1,'four_lane':4}[plan.variant])
            self._cli,self._program=cli,resource_program
            self._image_bindings=tuple((image,image.path,image.fd,image.identity,image.sha256,image.uuids) for image in (cli,resource_program))
            self._peer_bindings=tuple((peer,peer.peer_id,peer.pid,peer.identity,peer.image,peer.reader) for peer in peers)
            self._end,self._admitted_end=trial_deadline_ns,trial_deadline_ns
            self._runtime,self._live_callback=verify_runtime,verify_all_peers_live
            self._paths,self._worker=paths,resource_worker
            self._path_snapshot=tuple(getattr(paths,f.name) for f in fields(paths)),resource_worker
            self._input_snapshot=repr((inputs.input_directory,inputs.anchors_sha256,inputs.roles,inputs.generation))
            self._daemon_images=tuple((peer.image,peer.image.path,peer.image.fd,peer.image.identity,peer.image.sha256,peer.image.uuids) for peer in peers)
            require(all(inputs.input_directory not in path.parents and inputs.input_directory!=path
                        for path in (paths.resource_config,paths.resource_capture_dir,paths.transaction_trace,paths.collector_journal)))
            self._invocation=secrets.token_hex(32)
            self._original_invocation=self._invocation
            require(SHA.fullmatch(self._invocation) and self._invocation!='0'*64)
            self._common();self._live()
            config={'schema':'iroha.sumeragi_v2.resource_probe.config.v1',
                'resource_budget':run_budget_inputs(allocation),
                'peers':[{'peer_id':peer.peer_id,'pid':peer.identity.pid,'executable_path':str(peer.image.path),
                    'executable_sha256':peer.identity.executable_sha256,'endpoint':role.torii_url,'headers':{}}
                    for peer,role in zip(peers,inputs.roles,strict=True)]}
            raw=json.dumps(config,separators=(',',':'),ensure_ascii=True,allow_nan=False).encode()
            self._files=LoadFiles(paths,resource_worker,resource_worker_sha256,raw,allocation)
            self._commands=BoundedCommand(cli,reader,trial_deadline_ns,self._guard)
            self._phase='admitted';self._common();self._live()
        except BaseException as error:
            self._phase='failed'
            if self._files is not None:self._files.close()
            fail(error)

    @property
    def trial_deadline_ns(self):return self._admitted_end

    def _common(self):
        require(self._phase in ('admitting','admitted','running','complete'))
        require(self._end==self._admitted_end and tuple(getattr(self._plan,f.name) for f in fields(self._plan))==self._plan_snapshot)
        require(canonical_run_budget_bytes(self._allocation)==self._budget)
        self._inputs.validate()
        require((tuple(getattr(self._paths,f.name) for f in fields(self._paths)),self._worker)==self._path_snapshot
                and self._invocation==self._original_invocation
                and repr((self._inputs.input_directory,self._inputs.anchors_sha256,self._inputs.roles,self._inputs.generation))==self._input_snapshot)
        for image,path,fd,identity,digest,uuids in (*self._image_bindings,*self._daemon_images):
            require((image.path,image.fd,image.identity,image.sha256,image.uuids)==(path,fd,identity,digest,uuids));image.validate()
        for peer,role,pid,identity,image,reader in self._peer_bindings:
            require((peer.peer_id,peer.pid,peer.identity,peer.image,peer.reader)==(role,pid,identity,image,reader))
        if self._files is not None:self._files.validate()
        if self._phase!='complete':require(time.monotonic_ns()<self._end)

    def _live(self):
        self._live_callback()
        self._common()
        for peer,_,_,identity,_,_ in self._peer_bindings:require(peer.sample().identity==identity)
        self._common()

    def _guard(self):
        self._common();self._runtime();self._common();self._live();self._common()

    def _argv(self):
        plan=self._plan;inputs=self._inputs
        fd=inputs.client_fd(0)
        argv=[str(self._cli.path),'--machine','--config-fd',str(fd),'--config-source-path',str(inputs.roles[0].client_config),
              '--output-format','json','--fee-payer','authority','tx','load']
        for flag,value in (('invocation-id',self._invocation),('pair-index',plan.pair_index),('variant',plan.variant),
                ('seed',plan.seed),('offered-load-tps',plan.offered_load_tps),
                ('warmup-seconds',decimal_ns(plan.warmup_ns,1_000_000_000)),('measurement-seconds',decimal_ns(plan.measurement_ns,1_000_000_000)),
                ('drain-seconds',decimal_ns(plan.drain_ns,1_000_000_000)),('max-submission-lag-ms',decimal_ns(plan.submission_lag_ns,1_000_000))):
            argv.extend(('--'+flag,str(value)))
        for account in inputs.generation.accounts:argv.extend(('--account-config',str(inputs.input_directory/account.config)))
        argv.extend(('--local-observer-config',str(inputs.roles[3].client_config),
                     '--trace-out',str(self._paths.transaction_trace),'--diagnostic-out',str(self._paths.collector_journal)))
        for name in ('preparation_lookahead','preparation_concurrency','preparation_ahead_ms','max_submissions','max_in_flight',
                     'max_status_requests','poll_interval_ms','journal_capacity','resource_interval_ms','resource_timeout_ms','resource_max_start_lag_ms'):
            argv.extend(('--'+name.replace('_','-'),str(getattr(plan,name))))
        for flag,value in (('resource-program',self._program.path),('resource-worker',self._worker),
                ('resource-config',self._paths.resource_config),('resource-budget-sha256',self._budget_sha),
                ('resource-capture-dir',self._paths.resource_capture_dir)):
            argv.extend(('--'+flag,str(value)))
        return tuple(argv),(fd,)

    def _reply(self,raw):
        require(type(raw) is bytes and 0<len(raw)<=MAX_REPLY_BYTES and raw.startswith(b'{') and raw.endswith(b'}\n') and b'\n' not in raw[:-1])
        def pairs(rows):
            value={}
            for key,item in rows:require(key not in value);value[key]=item
            return value
        def number(value):require(len(value)<=10 and value.isascii() and value.isdigit());return int(value)
        def reject(_):require(False)
        report=json.loads(raw.decode(),object_pairs_hook=pairs,parse_int=number,parse_float=reject,parse_constant=reject)
        require(type(report) is dict and report.keys()==REPLY_FIELDS)
        require(type(report['version']) is int and report['version']==1 and report['operation']=='transaction_load')
        for key,expected in (('invocation_id',self._invocation),('pair_index',self._plan.pair_index),('variant',self._plan.variant),
                ('seed',self._plan.seed),('resource_budget_sha256',self._budget_sha),('scheduled_requests',self._scheduled)):
            require(type(report[key]) is type(expected) and report[key]==expected)
        for prefix,cap in (('trace',self._allocation.run.transaction_trace),('collector_journal',self._allocation.journal)):
            require(type(report[prefix+'_sha256']) is str and SHA.fullmatch(report[prefix+'_sha256']))
            integer(report[prefix+'_bytes'],1,cap.max_bytes)
        return report

    def run(self):
        try:
            require(self._phase=='admitted');self._phase='running';self._guard();self._files.before_launch()
            argv,fds=self._argv();transport=self._commands.run('native-load',argv,fds,MAX_REPLY_BYTES)
            report=self._reply(transport.stdout);self._guard()
            trace,journal=self._files.capture(report,self._guard);self._guard()
            receipt=NativeLoadReceipt(self._invocation,self._plan,transport.process,tuple(row[3] for row in self._peer_bindings),
                self._image_bindings[0][4],self._budget_sha,self._scheduled,trace,journal,self._paths.resource_capture_dir)
            self._receipt=receipt;self._receipt_snapshot=repr(receipt);self._phase='complete';return receipt
        except BaseException as error:self._phase='failed';fail(error)

    def validate(self):
        try:
            require(self._phase=='complete' and self._receipt is not None)
            self._common();self._runtime();self._common()
            require(type(self._receipt) is NativeLoadReceipt and repr(self._receipt)==self._receipt_snapshot)
            return self._receipt
        except BaseException as error:self._phase='failed';fail(error)

    def public_descriptor(self, role: str) -> int:
        """Borrow a completed journal/trace only before the original deadline.

        The destination immediately duplicates this read-only original FD and
        must finish its own custody admission before this owner closes. This
        accessor preserves runtime/receipt checks and authorizes no new load.
        """
        try:
            require(not self._borrowing_public and self._phase=='complete'
                    and type(role) is str and role in ('collector_journal','transaction_trace')
                    and time.monotonic_ns()<self._admitted_end)
            self._borrowing_public=True
            files=self._files
            require(type(files) is LoadFiles)
            self.validate()
            descriptor=files.public_descriptor(role)
            self.validate()
            require(self._files is files and files.public_descriptor(role)==descriptor
                    and time.monotonic_ns()<self._admitted_end)
            return descriptor
        except BaseException as error:self._phase='failed';fail(error)
        finally:self._borrowing_public=False

    def cleanup(self,deadline_ns):
        self._phase='failed'
        return self._commands.cleanup(deadline_ns) if self._commands is not None else ()

    def close(self):
        try:
            if self._phase=='closed':return
            if self._commands is not None:
                for child in self._commands._children:
                    self._commands._bound(child);require(child.process.poll() is not None);self._commands._bound(child)
            if self._files is not None:self._files.close()
            self._phase='closed'
        except BaseException as error:self._phase='failed';fail(error)
