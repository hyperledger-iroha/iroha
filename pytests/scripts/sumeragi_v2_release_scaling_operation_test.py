"""Actual archive/descriptor/command-owner joins with explicit process doubles.

The provisioning source/runtime admission and native cryptography are seams here.
No synthetic archive or process double is release evidence. Production bounded
Popen supervision, natural wait, output accounting and all data joins execute.
"""
from dataclasses import fields, replace
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT/'scripts'), str(ROOT/'scripts/nexus')]
import sumeragi_v2_release_scaling_operation as operation
import scaling_release_provisioning as provisioning
import scaling_archive_data_test as archive_fixture
from scaling_measurements_test import run as measurement_run
from scaling_experiment_plan import plan_bytes
from resource_evidence_budget import canonical_run_budget_bytes, select_run_budget
from scaling_seed_pipe import DevelopmentSeedPipe
from sumeragi_v2_release_scaling_handoff_test import load, Process


def sha(raw): return hashlib.sha256(raw).hexdigest()


@pytest.fixture(scope='module')
def completed_archive(tmp_path_factory):
    where = tmp_path_factory.mktemp('operation-archive')
    original = archive_fixture.measurement_fixture
    def measured(trial):
        value = measurement_run(trial)
        if trial.load.variant == 'one_lane':
            drain_offset = trial.load.measurement_ns + trial.load.drain_ns // 2
            value = value._replace(requests=tuple(row if index < 50 else row._replace(
                applied_offset_ns=drain_offset, local_applied_offset_ns=drain_offset)
                for index, row in enumerate(value.requests)))
        return value
    archive_fixture.measurement_fixture = measured
    try: result = archive_fixture.build_archive(where)
    finally: archive_fixture.measurement_fixture = original
    assert result[4].throughput_criterion_met
    assert result[4].latency_criterion_met
    assert result[4].observed_resource_criterion_met
    return result


def api(module):
    return operation.BootstrapScalingApi(module.FixedScalingLaunch, module.CommandObservationOwner,
        module.TerminalCommandObservation, module.ParentScalingObservation,
        module.ScalingArtifactObservation, module.CommandResult, module._run_bounded,
        module._validated_pass_fds)


class Session:
    def __init__(self, monkeypatch, tmp_path, completed_archive):
        self.module = load('bootstrap_sumeragi_v2_release')
        self.template, plan, budget, _, _ = completed_archive
        self.root, self.calls, self.processes = tmp_path, [], []
        self.change = None
        self.collector_change = None
        self.collector_code = self.native_code = 0
        self.public = tmp_path/'publication'
        private = tmp_path/'original'; private.mkdir(mode=0o700)
        python = private/'python'; python.write_bytes(b'inert Python fixture')
        kagami = private/'kagami'; kagami.write_bytes(b'inert Kagami fixture')
        control = private/'launch.json'; control.write_bytes(b'{}'); control.chmod(0o400)
        files = provisioning._OriginalInputs()
        _, control_fd = files.hold(control, 16, sha(b'{}'))
        files.hold(kagami, 1024, sha(kagami.read_bytes()), payload=False)
        seed = DevelopmentSeedPipe('ab'*32)
        now = time.monotonic_ns(); deadline = now + 600*1_000_000_000
        launch = provisioning.PreparedScalingLaunch(python, private,
            (str(python), '-I', '-B', '-S', str(private/'scripts/nexus/run_multilane_scaling_gate.py'),
             '--launch-input-fd', str(control_fd), '--launch-input-sha256', sha(b'{}'),
             '--seed-fd', str(seed.fd)), control_fd, sha(b'{}'), seed.fd, private,
            (('PATH','/usr/bin:/bin'),), 600,
            budget.control_budgets[0].max_bytes + budget.control_budgets[1].max_bytes,
            self.public, budget.control_budgets[0].max_bytes, budget.control_budgets[1].max_bytes,
            now, deadline)
        source = json.loads((self.template/'inputs/source_closure.json').read_bytes())
        identity = json.loads((self.template/'inputs/identity.json').read_bytes())
        selected = provisioning.PreparedScalingVerification(plan_bytes(plan),
            canonical_run_budget_bytes(select_run_budget(budget,1,'one_lane')),
            kagami, source['executable_images']['kagami'], source['workspace_source_sha256'],
            source['source_revision'], tuple(source['executable_images'].items()),
            tuple((row['name'],row['bytes'],row['sha256']) for row in source['resource_sources']),
            identity['hardware']['machine_id'], identity['hardware']['storage_model'])
        # Explicit provisioning seam only: the original file and seed owners are
        # real; the full release framework/native admission is tested elsewhere.
        prepared = object.__new__(provisioning.PreparedScalingInputs)
        prepared._state, prepared._launch, prepared._deadline = 'prepared', launch, deadline
        prepared._files = prepared._original_files = files
        prepared._original_dependencies = None
        prepared._original_seed = seed
        def validate(owner):
            assert owner is prepared and owner._state != 'closed'
            files.validate(); seed.validate()
        def verification_inputs(owner):
            validate(owner); return self.selection
        self.selection = selected
        monkeypatch.setattr(provisioning.PreparedScalingInputs, 'validate', validate)
        monkeypatch.setattr(provisioning.PreparedScalingInputs, 'verification_inputs', verification_inputs)
        self.operation = operation.FixedScalingOperation(prepared, 'e'*64, api(self.module))
        self.prepared, self.launch = prepared, launch
        self.handoff = self.module.FixedScalingHandoff('e'*64, self.operation)
        def popen(argv, **options):
            self.calls.append((tuple(argv), options))
            if argv[0] == str(python):
                assert not self.public.exists()
                shutil.copytree(self.template, self.public)
                if self.collector_change is not None: self.collector_change(self)
                payload, code = b'collector complete', self.collector_code
            else:
                assert argv[0] == str(kagami)
                assert tuple(argv[1:7]) == ('--ui-mode','plain','advanced','kura','scaling-evidence','replay')
                assert options['pass_fds'] == ()
                payload, code = self.reply(argv), self.native_code
                if self.change is not None: payload = self.change(self, argv, payload)
            process = Process(code, payload)
            self.processes.append(process)
            return process
        monkeypatch.setattr(self.module.subprocess, 'Popen', popen)

    def reply(self, argv):
        arguments = dict(zip(argv[7::2],argv[8::2],strict=True))
        request = Path(arguments['--request'])
        raw = json.loads((request.parent.parent/'raw_samples.json').read_bytes())
        receipt = json.loads((request.parent.parent/'run_receipt.json').read_bytes())
        accounts = [row['account_id'] for row in receipt['generation']['accounts']]
        lanes = 1 if raw['variant']=='one_lane' else 4
        rows = []
        for row in raw['requests']:
            rows.append(dict(logical_id=row['logical_id'],phase=row['cohort'],sequence=row['sequence'],
                authority=accounts[row['account_index']],entrypoint_hash=row['transaction_hash'],
                carrier_height=row['block_height'],carrier_hash='a'*63+'1',merge_entry_hash='b'*63+'1',
                merge_epoch=1,leaf_index=row['index'],lane_id=row['account_index']%lanes,
                dataspace_id=0,incarnation='d'*63+'1'))
        header = dict(version=1,operation='replay',invocation_id=arguments['--invocation-id'],
            request_sha256=arguments['--request-sha256'],input_sha256=arguments['--input-sha256'],
            proof_sha256=arguments['--input-sha256'],proof_iroha_hash=arguments['--proof-iroha-hash'],
            proof_bytes=Path(arguments['--input']).stat().st_size)
        compact=lambda value:json.dumps(value,separators=(',',':')).encode('ascii')
        return compact(header)[:-1]+b',"rows":'+compact(rows)+b'}\n'

    def close(self):
        self.handoff.close()


@pytest.fixture
def session(monkeypatch,tmp_path,completed_archive):
    for name in ('system','fork','posix_spawn','posix_spawnp','kill','killpg'):
        if hasattr(os,name): monkeypatch.setattr(os,name,lambda *a,**k:pytest.fail('OS child/signal forbidden'))
    value=Session(monkeypatch,tmp_path,completed_archive)
    try: yield value
    finally:
        if value.operation._phase == 'borrowed':
            value.operation.child_finished(value.operation._launch, None)
            value.handoff._child_finished = True
        value.close()


def test_original_handoff_all_ten_native_replays_and_complete_receipt_data(session):
    session.handoff._execute()
    verified=session.operation.verification
    assert len(session.calls)==11 and all(process.waits==1 for process in session.processes)
    assert len(verified.native_replays)==10 and len({row.invocation_id for row in verified.native_replays})==10
    assert verified.collector is session.handoff._command.terminal
    assert verified.collector is not verified.native_replays[0].command
    assert all(row.command.deadline_ns==verified.original_deadline_ns for row in verified.native_replays)
    assert verified.verification_completed_ns<verified.original_deadline_ns==session.launch.deadline_ns
    assert verified.inventory.sha256==archive_fixture.independent_inventory(session.public)
    assert verified.archive_checks.inventory == verified.inventory
    assert len(verified.archive_checks.native_runs) == 10
    assert verified.archive_checks.measurements.throughput_criterion_met is True
    assert all(row.row_count==100 and row.reply_bytes==row.command.stdout_bytes for row in verified.native_replays)
    assert session.handoff.revalidate_observation().command is verified.collector
    session.close()
    assert session.operation.verification is verified
    with pytest.raises(OSError):os.fstat(session.launch.launch_input_fd)


@pytest.mark.parametrize('mutation', ['missing','reorder','height','authority','header','trailer'])
def test_fresh_native_reply_requires_every_exact_row(session,mutation):
    def change(_session,_argv,payload):
        value=json.loads(payload)
        if mutation=='missing':value['rows'].pop()
        elif mutation=='reorder':value['rows'][0],value['rows'][1]=value['rows'][1],value['rows'][0]
        elif mutation=='height':value['rows'][0]['carrier_height']+=1
        elif mutation=='authority':value['rows'][0]['authority']='another-account'
        elif mutation=='header':value['invocation_id']='0'*64
        else:return payload+b'\n'
        rows=value.pop('rows')
        return json.dumps(value,separators=(',',':')).encode()[:-1]+b',"rows":'+json.dumps(rows,separators=(',',':')).encode()+b'}\n'
    session.change=change
    with pytest.raises((ValueError, session.module.BootstrapError)):session.handoff._execute()
    assert len(session.calls)==2 and all(process.waits==1 for process in session.processes)
    with pytest.raises(ValueError):_ = session.operation.verification


@pytest.mark.parametrize('target', ['collector','native'])
def test_nonzero_natural_exit_does_not_become_verified_archive(session,target):
    if target=='collector':session.collector_code=19
    else:session.native_code=23
    with pytest.raises((ValueError, session.module.BootstrapError)):session.handoff._execute()
    assert all(process.waits==1 for process in session.processes)
    assert session.prepared._state=='reaped'
    with pytest.raises(ValueError):_ = session.operation.verification


def test_archive_changed_during_native_verification_rejects_final_inventory(session):
    def change(current,_argv,payload):
        path=current.public/'inputs/identity.json'
        if len(current.calls)==2:path.write_bytes(path.read_bytes()+b' ')
        return payload
    session.change=change
    with pytest.raises(ValueError):session.handoff._execute()
    assert all(process.waits==1 for process in session.processes)


@pytest.mark.parametrize('field', ['kagami_sha256','workspace_source_sha256','source_revision','worker_sources','executable_images'])
def test_original_selection_drift_rejects_before_any_child(session,field):
    values=dict(kagami_sha256='1'*64,workspace_source_sha256='2'*64,source_revision='3'*40,
        worker_sources=(),executable_images=())
    session.selection=replace(session.selection,**{field:values[field]})
    with pytest.raises(ValueError):session.handoff._execute()
    assert session.calls==[]


def test_pending_original_borrow_cannot_be_closed_or_claimed_twice(session):
    launch=session.operation.prepare()
    with pytest.raises(provisioning.ScalingInputsBorrowedError):session.operation.close()
    with pytest.raises(ValueError):session.operation.prepare()
    os.fstat(launch.seed_fd);os.fstat(launch.launch_input_fd)
    session.operation.child_finished(launch,None)
    with pytest.raises(ValueError):session.operation.child_finished(launch,None)


def test_verified_archive_exposes_exact_original_canonical_controls(completed_archive):
    root,plan,budget,expected,_=completed_archive
    checked=archive_fixture.archive.inspect_archive(root,plan,budget,expected)
    assert checked.identity_json==(root/'inputs/identity.json').read_bytes()
    assert checked.source_closure_json==(root/'inputs/source_closure.json').read_bytes()


@pytest.mark.parametrize('changed', ['source_revision','workspace_source_sha256','worker','resource_program'])
def test_self_consistent_archive_source_claim_cannot_replace_original_selection(session,changed):
    def change(current):
        manifest_path=current.public/'manifest.json'
        manifest=json.loads(manifest_path.read_bytes())
        for label in ('identity','source_closure'):
            path=current.public/'inputs'/f'{label}.json'
            value=json.loads(path.read_bytes())
            if changed in ('source_revision','workspace_source_sha256'):
                target=value['software'] if label=='identity' else value
                target[changed]='1'*(40 if changed=='source_revision' else 64)
            elif label=='source_closure' and changed=='worker':
                value['resource_sources'][0]['sha256']='1'*64
            elif label=='source_closure':
                value['executable_images']['resource_program']='1'*64
            raw=archive_fixture.encode(value);path.write_bytes(raw)
            next(row for row in manifest['inputs'] if row['label']==label)['sha256']=sha(raw)
        raw=archive_fixture.encode(manifest);manifest_path.write_bytes(raw)
        report_path=current.public/'report.json';report=json.loads(report_path.read_bytes())
        report['manifest_sha256']=sha(raw)
        report_path.write_bytes(archive_fixture.encode(report))
        # This is a valid, internally consistent data archive. Only comparison
        # with the original parent selection distinguishes the substituted pins.
        archive_fixture.archive.inspect_archive(current.public, current.operation._plan,
            current.operation._budget, archive_fixture.bindings(current.public))
    session.collector_change=change
    with pytest.raises(ValueError):session.handoff._execute()
    assert len(session.calls)==1


def test_real_recomputed_failing_measurement_criterion_never_starts_native_replay(session):
    where=session.root/'failed-criterion';where.mkdir(mode=0o700)
    root,plan,budget,expected,measurements=archive_fixture.build_archive(where)
    assert measurements.throughput_criterion_met is False
    archive_fixture.archive.inspect_archive(root,plan,budget,expected)
    session.template=root
    with pytest.raises(ValueError):session.handoff._execute()
    assert len(session.calls)==1


def test_fresh_native_runtime_violation_keeps_original_deadline_and_natural_reap(session,monkeypatch):
    clock=[session.launch.original_started_ns+1_000_000]
    monkeypatch.setattr(time,'monotonic_ns',lambda:clock[0])
    monkeypatch.setattr(time,'monotonic',lambda:clock[0]/1_000_000_000)
    def late(_session,_argv,payload):
        clock[0]=session.launch.deadline_ns+1_000_000_000
        return payload
    session.change=late
    with pytest.raises(session.module.BootstrapError):session.handoff._execute()
    owner=session.operation._native_owners[0]
    assert owner._reaped and owner.terminal.returncode==0
    assert owner.terminal.violations==('runtime',)
    assert owner.terminal.deadline_ns==session.launch.deadline_ns
    assert all(process.waits==1 for process in session.processes)


def test_another_parent_invocation_cannot_adopt_the_collector_observation(session):
    session.handoff.invocation_sha256='f'*64
    with pytest.raises(ValueError):session.handoff._execute()
    assert len(session.calls)==1


def test_pending_native_owner_preserves_inputs_until_natural_terminal_observation(session):
    owner=session.module.CommandObservationOwner()
    process=Process()
    now=time.monotonic_ns()
    owner._begin(('in-process-test',),session.root,{},(),now,session.launch.deadline_ns)
    owner._spawned(process)
    session.operation._native_owners.append(owner)
    with pytest.raises(provisioning.ScalingInputsBorrowedError):session.operation.close()
    os.fstat(session.launch.launch_input_fd);os.fstat(session.launch.seed_fd)
    owner._waited(process,0)
    digests={name:hashlib.sha256() for name in ('stdout','stderr')}
    owner._finish(process,now,0,digests,{'stdout':0,'stderr':0},())
    session.operation.close()
    with pytest.raises(OSError):os.fstat(session.launch.launch_input_fd)
