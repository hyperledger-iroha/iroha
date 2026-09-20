"""Actual source/file bootstrap flow with explicit provisioning/native process seams."""
from __future__ import annotations

import ast
from dataclasses import fields
import hashlib
import json
import os
from pathlib import Path
import sys
import types

import pytest
from sumeragi_v2_release_scaling_selection_test import bootstrap_definitions, selection

ROOT = Path(__file__).resolve().parents[2]


def put(path, raw):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if path.exists(): path.chmod(0o600)
    path.write_bytes(raw); path.chmod(0o400)


@pytest.fixture
def module_files(tmp_path):
    m = bootstrap_definitions()
    root = tmp_path/'source'; root.mkdir(mode=0o700)
    members = ('scripts/compute_workspace_source_manifest.py',
               'scripts/nexus/scaling_cli_bootstrap.py', *m._SCALING_PARENT_EXTENSIONS)
    registry = ('PYTHON_SOURCE_FILES = '+repr(members[:2])+'\n').encode()
    put(root/members[1], registry)
    for name in members[2:]: put(root/name, b'SENTINEL = "captured source"\n')
    put(root/members[0], (ROOT/members[0]).read_bytes())
    # Use the existing source-list writer under a distinct test-only module name.
    source = types.ModuleType('scaling_main_fixture_source_writer')
    source.__file__ = str(root/members[0]); sys.modules[source.__name__] = source
    exec(compile((root/members[0]).read_bytes(), source.__file__, 'exec'),source.__dict__)
    paths=tmp_path/'source-paths';source.write_source_path_list(paths,members)
    del sys.modules[source.__name__]
    for directory in sorted((p for p in root.rglob('*') if p.is_dir()),reverse=True):directory.chmod(0o500)
    root.chmod(0o500)
    snapshot=lambda p:m._read_file(p,'fixture',maximum_bytes=16*1024*1024)
    other=tmp_path/'other';put(other,b'{}\n')
    selected=m.ScalingSourceSelection(m._sealed_directory_snapshot(root,'fixture source'),
        snapshot(other),snapshot(paths),snapshot(other),snapshot(other),snapshot(other))
    owners=[]
    def load():
        owner=m.ScalingParentModules(selected);owners.append(owner);return owner
    # These fixtures simulate first import in the original bootstrap process.
    # Combined pytest collection imports receipt modules before this fixture;
    # preserve those exact identities outside its deliberately fresh namespace.
    names = tuple(Path(member).stem for member in members)
    previous = {name: sys.modules.pop(name) for name in names if name in sys.modules}
    try:
        yield types.SimpleNamespace(m=m,root=root,selected=selected,load=load,owners=owners,members=members)
    finally:
        for owner in reversed(owners):owner.release()
        for name in names:sys.modules.pop(name, None)
        sys.modules.update(previous)
        for directory in (root,*[p for p in root.rglob('*') if p.is_dir()]):directory.chmod(0o700)


def test_captured_byte_loader_has_exact_origins_and_no_ambient_path(module_files):
    f=module_files;before=tuple(sys.path)
    owner=f.load();value=owner.load('scaling_release_record')
    assert value.SENTINEL=='captured source'
    assert value.__loader__ is owner and value.__spec__.loader is owner
    assert value.__file__==str(f.root/'scripts/nexus/scaling_release_record.py')
    assert tuple(sys.path)==before
    owner.close();owner.verify_retained()
    assert owner not in sys.meta_path
    with pytest.raises(f.m.BootstrapError):owner.load('scaling_release_record')


def test_loader_rejects_preexisting_same_name_without_replacing_it(module_files,monkeypatch):
    f=module_files;foreign=types.ModuleType('scaling_release_record')
    monkeypatch.setitem(sys.modules,'scaling_release_record',foreign)
    before=tuple(sys.meta_path)
    with pytest.raises(f.m.BootstrapError):f.load()
    assert sys.modules['scaling_release_record'] is foreign and tuple(sys.meta_path)==before


def test_loader_cleanup_preserves_foreign_module_replacement(module_files,monkeypatch):
    f=module_files;owner=f.load();owner.load('scaling_release_record')
    foreign=types.ModuleType('scaling_release_record')
    monkeypatch.setitem(sys.modules,'scaling_release_record',foreign)
    with pytest.raises(f.m.BootstrapError):owner.close()
    assert owner not in sys.meta_path
    owner.release();assert sys.modules['scaling_release_record'] is foreign


def test_loader_rejects_changed_bytes_before_execution(module_files):
    f=module_files;owner=f.load()
    put(f.root/'scripts/nexus/scaling_release_record.py',b'raise AssertionError("must not execute")\n')
    with pytest.raises(f.m.BootstrapError):owner.load('scaling_release_record')
    owner.release()
    assert 'scaling_release_record' not in sys.modules


def test_loader_rejects_missing_authenticated_source_member(module_files):
    f=module_files
    # Corrupting even a bounded path-list snapshot rejects before source execution.
    put(f.selected.source_paths.path,b'wrong path list\n')
    before=tuple(sys.meta_path)
    with pytest.raises(f.m.BootstrapError):f.load()
    assert tuple(sys.meta_path)==before


def test_loader_isolates_direct_source_loader_from_inherited_pyc(module_files, monkeypatch, tmp_path):
    import importlib.machinery
    import importlib.util
    import importlib._bootstrap_external
    f=module_files
    inherited=tmp_path/'inherited-cache'
    monkeypatch.setattr(sys,'pycache_prefix',str(inherited))
    path=f.root/'scripts/nexus/scaling_release_record.py'
    cache=Path(importlib.util.cache_from_source(str(path)))
    poison=compile('SENTINEL = "inherited poison"\n',str(path),'exec')
    raw=importlib._bootstrap_external._code_to_timestamp_pyc(
        poison,mtime=int(path.stat().st_mtime),source_size=path.stat().st_size)
    put(cache,raw)
    def directly():
        spec=importlib.util.spec_from_file_location('_direct_scaling_fixture',path)
        value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value)
        return value.SENTINEL
    assert directly()=='inherited poison'
    owner=f.load()
    assert sys.pycache_prefix != str(inherited)
    assert directly()=='captured source'
    assert not Path(sys.pycache_prefix).exists()
    owner.close()
    assert sys.pycache_prefix==str(inherited)


def test_loader_refuses_preexisting_cache_namespace(module_files):
    f=module_files;cache=f.selected.source_paths.path.parent/'scaling-unused-parent-cache'
    cache.mkdir();before=sys.pycache_prefix
    with pytest.raises(f.m.BootstrapError):f.load()
    assert sys.pycache_prefix==before


def test_loader_detects_cache_substitution_and_preserves_foreign_prefix(module_files,monkeypatch):
    f=module_files;owner=f.load()
    monkeypatch.setattr(sys,'pycache_prefix','foreign-cache-choice')
    with pytest.raises(f.m.BootstrapError):owner.close()
    assert owner not in sys.meta_path and sys.pycache_prefix=='foreign-cache-choice'
    owner.release();assert sys.pycache_prefix=='foreign-cache-choice'


def test_loader_refuses_bytecode_write_mode(module_files,monkeypatch):
    f=module_files;before=sys.pycache_prefix
    monkeypatch.setattr(sys,'dont_write_bytecode',False)
    with pytest.raises(f.m.BootstrapError):f.load()
    assert sys.pycache_prefix==before


@pytest.fixture
def lazy(selection,monkeypatch):
    s=selection;m=s.m;events=[];seeds=[];choices=[]
    state=types.SimpleNamespace(pending=False,closed=False,fail_validate=False,verify=None)
    class Prepared:
        _dependencies=None
        def verification_inputs(self):return types.SimpleNamespace(plan_bytes=b'plan',budget_bytes=b'budget')
        def validate(self):events.append('prepared validate')
        def close(self):
            if state.pending:raise m.BootstrapError('original child still pending')
            state.closed=True;events.append('prepared close')
    prepared=Prepared()
    def prepare(selected,seed):
        choices.append(selected);seeds.append(seed);events.append('prepare inputs');return prepared
    class Selection:
        def __init__(self,*args):self.values=args
    provisioning=types.SimpleNamespace(ParentScalingSelection=Selection,
        PreparedScalingInputs=types.SimpleNamespace(prepare=prepare))
    class Concrete:
        def __init__(self,actual,digest,api):
            assert actual is prepared
            self.digest=digest;self.api=api;self.verification=None
        def prepare(self):
            state.pending=True;events.append('concrete prepare')
            return types.SimpleNamespace(deadline_ns=10**30)
        def validate(self,launch):
            if state.fail_validate:raise m.BootstrapError('fixture validation failed')
            events.append('concrete validate')
        def child_finished(self,launch,observation):state.pending=False;events.append(('finished',observation))
        def verify_publication(self,launch,observation):
            events.append('verify publication');self.verification=types.SimpleNamespace(collector=observation.command)
        def close(self):prepared.close()
    def api(*args):
        assert args==(m.FixedScalingLaunch,m.CommandObservationOwner,m.TerminalCommandObservation,
            m.ParentScalingObservation,m.ScalingArtifactObservation,m.CommandResult,m._run_bounded,m._validated_pass_fds)
        return args
    concrete=types.SimpleNamespace(BootstrapScalingApi=api,FixedScalingOperation=Concrete)
    def encode(identity,inputs,observation,verification,**kwargs):
        assert identity==s.sealed and verification.collector is observation.command
        assert kwargs['observation_overhead_seconds']==60
        events.append('encode original objects')
        return m._canonical_json(dict(invocation=observation.invocation_sha256,
                                     command_id=id(observation.command),verifier_python=kwargs['verifier_python']))
    record=types.SimpleNamespace(encode_parent_execution=encode,MAX_RECORD_BYTES=1024*1024)
    class Preflight:
        def __init__(self,*args,**kwargs):
            assert args[5] == 28800
            assert set(kwargs) == {'archive','candidate_identity','invocation_sha256'}
            assert type(kwargs['archive']) is m.BootstrapPreflightArchive
            assert kwargs['candidate_identity'] == s.sealed
            self.archive_binding = {'original_preflight': kwargs['invocation_sha256']}
            self.passed=False;events.append('select complete preflight')
        def run(self):self.passed=True;events.append('complete preflight')
        def verify(self):assert self.passed
        def verify_retained(self):assert self.passed
        def close(self):events.append('preflight close')
        def require_terminal(self):events.append('preflight natural terminal')
        def release(self):events.append('preflight release')
    preflight=types.SimpleNamespace(CompleteScalingPreflight=Preflight,PreflightApi=lambda *values:values)
    class Loader:
        def __init__(self,selected):events.append('capture authenticated source')
        def load(self,name):return {'scaling_release_preflight':preflight,'scaling_cli_bootstrap':types.SimpleNamespace(),
            'scaling_release_provisioning':provisioning,
            'sumeragi_v2_release_scaling_operation':concrete,'scaling_release_record':record}[name]
        def verify(self):events.append('loader verify')
        def verify_retained(self):events.append('loader retained')
        def close(self):events.append('loader close')
        def release(self):events.append('loader release')
    monkeypatch.setattr(m,'ScalingParentModules',Loader)
    def stage(owner):
        events.append('stage verifier')
        owner._verifier_python_owner=types.SimpleNamespace(verify=lambda:events.append('verifier verify'),close=lambda:events.append('verifier close'))
        owner._verifier_python_json=b'{"inventory_sha256":"fixture","files":[]}\n'
    monkeypatch.setattr(m.BootstrapScalingOperation,'_stage_verifier_python',stage)
    plan=s.write(s.values['evidence']/'plan.json',b'plan')
    budget=s.write(s.values['evidence']/'budget.json',b'budget')
    helper=s.write(s.values['evidence']/'scaling-handoff.py',b'# fixed helper\n')
    value=m.BootstrapScalingOperation(s.values['invocation'],s.values['candidate_identity'],
        s.values['python'],s.values['manifest_helper'],s.values['rustc'],plan,budget,helper,
        s.values['evidence'],s.values['evidence_fd'],s.values['framework_binding'],
        s.values['environment'],30,s.values['evidence']/'installed','machine','storage',60,28800)
    yield types.SimpleNamespace(s=s,m=m,value=value,events=events,seeds=seeds,choices=choices,state=state,record=record)
    if state.pending:value.child_finished(value._launch,None)
    value.release()


def test_lazy_preparation_uses_only_original_roots_and_selected_policy(lazy):
    f=lazy;assert not f.events and not f.s.calls
    launch=f.value.prepare();values=f.choices[0].values;root=f.s.values['invocation'].path
    assert values[18:20]==(root/'output/scaling',root/'runtime/scaling-work')
    assert values[17]==root/'runtime/scaling-control'
    assert values[-1]==60 and len(values)==25
    assert len(f.seeds)==1 and len(f.seeds[0])==64
    assert f.seeds[0] not in repr(values)
    assert f.events.index('capture authenticated source')<f.events.index('complete preflight')<f.events.index('prepare inputs')<f.events.index('stage verifier')<f.events.index('concrete prepare')
    with pytest.raises(f.m.BootstrapError):f.value.prepare()
    with pytest.raises(f.m.BootstrapError):f.value.close()
    f.value.child_finished(launch,None);f.value.close()


def test_no_request_close_before_prepare_creates_no_inputs(lazy):
    f=lazy;f.value.close();assert not f.events and not f.seeds and not f.s.calls


def test_failed_no_spawn_after_borrow_releases_original_inputs(lazy):
    f=lazy;f.state.fail_validate=True
    with pytest.raises(f.m.BootstrapError):f.value.prepare()
    assert not f.state.pending and f.state.closed
    assert ('finished',None) in f.events


def test_original_observation_record_is_reprojected_after_close(lazy):
    f=lazy;launch=f.value.prepare();command=object()
    observation=types.SimpleNamespace(command=command,invocation_sha256=f.value.invocation_sha256)
    f.value.child_finished(launch,observation)
    f.value.verify_publication(launch,observation)
    f.value.close();snapshot=f.value.revalidate_final(observation)
    assert snapshot.path==f.s.values['evidence']/'scaling-execution.json' and snapshot.mode==0o400
    assert f.events.count('encode original objects')==2
    clone=types.SimpleNamespace(command=command,invocation_sha256=observation.invocation_sha256)
    with pytest.raises(f.m.BootstrapError):f.value.revalidate_final(clone)
    put(snapshot.path,b'changed record\n')
    with pytest.raises(f.m.BootstrapError):f.value.revalidate_final(observation)


def test_final_encoder_must_be_same_captured_function(lazy):
    f=lazy;launch=f.value.prepare();observation=types.SimpleNamespace(command=object(),invocation_sha256=f.value.invocation_sha256)
    f.value.child_finished(launch,observation);f.value.verify_publication(launch,observation);f.value.close()
    f.record.encode_parent_execution=lambda *a,**k:b'forged'
    with pytest.raises(f.m.BootstrapError):f.value.revalidate_final(observation)


def test_main_call_site_passes_exact_handoff_and_final_original_record():
    tree=ast.parse((ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_bytes())
    function=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='bootstrap')
    calls=[n for n in ast.walk(function) if isinstance(n,ast.Call)]
    runner=[n for n in calls if isinstance(n.func,ast.Name) and n.func.id=='_run_release_runner']
    assert len(runner)==1
    assert next(n.value.id for n in runner[0].keywords if n.arg=='scaling_handoff')=='scaling_handoff'
    for name in ('_validate_terminal_receipt','_run_protected_receipt_validator'):
        call=next(n for n in calls if isinstance(n.func,ast.Name) and n.func.id==name)
        assert next(n.value.id for n in call.keywords if n.arg=='scaling_execution')=='scaling_execution'
    assert len([n for n in calls if isinstance(n.func,ast.Attribute) and n.func.attr=='revalidate_final'])>=4
    assert all(name not in bootstrap_definitions()._RUNNER_ENV_ALLOWLIST for name in
               ('IROHA_RELEASE_SCALING_CHALLENGE','IROHA_RELEASE_SCALING_GATE_FD','IROHA_RELEASE_SCALING_CONFIGURATION_SHA256'))


def test_original_protected_input_drift_rejects_before_lazy_source_selection(lazy):
    f=lazy;put(f.value._original[4].path,b'changed plan')
    with pytest.raises(f.m.BootstrapError):f.value.prepare()
    assert not f.seeds and not f.s.calls and not f.events


def test_loader_failure_before_preparation_never_borrows_inputs(lazy,monkeypatch):
    f=lazy
    def fail(_selection):raise f.m.BootstrapError('captured loader failed')
    monkeypatch.setattr(f.m,'ScalingParentModules',fail)
    with pytest.raises(f.m.BootstrapError):f.value.prepare()
    assert not f.seeds and not f.state.pending and f.value._prepared is None


@pytest.mark.parametrize('status',(37,74,125))
def test_runner_failure_without_gate_preserves_natural_terminal_status(status):
    m=bootstrap_definitions();closed=[]
    operation=types.SimpleNamespace(close=lambda:closed.append('closed'))
    handoff=m.FixedScalingHandoff('a'*64,operation)
    class Runner:
        def __init__(self):self.waits=0
        def poll(self):return status
        def wait(self):self.waits+=1;return status
    process=Runner()
    try:
        handoff._started(process)
        assert handoff.wait_runner(process)==status and process.waits==1
        assert handoff.observation is None
    finally:handoff.close()
    assert closed==['closed']


def test_protected_parser_requires_fixed_policy_and_has_no_seed_or_argv_option():
    m=bootstrap_definitions();parser=m._parser();actions={a.dest:a for a in parser._actions}
    for name in ('scaling_plan','expected_scaling_plan_sha256','scaling_budget',
        'expected_scaling_budget_sha256','scaling_handoff_helper','expected_scaling_handoff_helper_sha256',
        'scaling_dependency_source','scaling_machine_id','scaling_storage_model','scaling_observation_overhead_seconds'):
        assert actions[name].required
    assert not any(name in actions for name in ('seed','seed_hex','trial_command','scaling_argv'))


def test_terminal_invocation_digest_uses_original_parent_record(tmp_path):
    m=bootstrap_definitions();evidence=tmp_path/'evidence';evidence.mkdir(mode=0o700)
    put(evidence/'scaling-execution.json',b'{"parent":"record"}\n')
    record=m._read_file(evidence/'scaling-execution.json','record',maximum_bytes=1024)
    data={'bootstrap':{'completion':{},'candidate_identity':{},'identity_verification':{'identity_attestation':{},'identity_transcript':{}}}}
    for label in ('release_signature_attestation','release_signature_transcript','release_signature_raw_commit',
        'release_signature_cargo_lock','release_signature_allowed_signers','release_signature_revocation',
        'release_signature_git','release_signature_ssh_keygen','corridor_completion','formal_completion',
        'seed_matrix_completion','chaos_completion'):
        data[label]={'path':str(evidence/label)}
    data['g4p_multilane']={'completion':{'path':str(evidence/'g4')}}
    data['g12_cross_dataspace']={key:{'path':str(evidence/key)} for key in ('seed_completion','fault_soak_completion')}
    data['formal_replay_release']={'principal':'fixture','signature':{'sha256':'f'*64},
        'source_receipt':{'path':str(evidence/'formal-source')},'receipt':{'path':str(evidence/'formal/receipt')}}
    data['multilane_scaling']={'parent_execution':dict(archive_id='release-scaling.parent-execution.v1',
        sha256=record.sha256,size_bytes=record.size,mode='0400')}
    trust={key:'f'*64 for key in ('git_sha256','ssh_keygen_sha256','allowed_signers_sha256','revocation_sha256')}
    trust['signer_fingerprint']='SHA256:fixture'
    receipt={'authentication':{'release_identity':{'trust_policy':trust},'bootstrap':{'completion_sha256':'b'*64}},'evidence':data}
    values=m._terminal_validator_invocation_values(receipt,evidence=evidence,candidate=tmp_path/'candidate',
        release_runner=tmp_path/'release',receipt_path=tmp_path/'receipt.json',acknowledgment_path=tmp_path/'ack.json',
        source_manifest_sha256='c'*64,authenticated_environment={},scaling_execution=record)
    assert tuple(values)==m._VALIDATOR_OPTION_ORDER
    assert values['--scaling-execution-record']==('path',str(record.path))
    assert values['--expected-scaling-execution-sha256']==('text',record.sha256)
    data['multilane_scaling']['parent_execution']['sha256']='e'*64
    with pytest.raises(m.BootstrapError):
        m._terminal_validator_invocation_values(receipt,evidence=evidence,candidate=tmp_path/'candidate',
            release_runner=tmp_path/'release',receipt_path=tmp_path/'receipt.json',acknowledgment_path=tmp_path/'ack.json',
            source_manifest_sha256='c'*64,authenticated_environment={},scaling_execution=record)


@pytest.fixture
def archive_writer(tmp_path):
    m = bootstrap_definitions()
    evidence = tmp_path/'bootstrap'; evidence.mkdir(mode=0o700)
    descriptor = os.open(evidence, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    owner = m.BootstrapPreflightArchive(evidence, descriptor)
    try:
        yield types.SimpleNamespace(m=m, evidence=evidence, descriptor=descriptor, owner=owner)
    finally:
        owner.close()
        os.close(descriptor)


def test_preflight_archive_publishes_exact_buffers_and_retains_only_bounded_metadata(archive_writer):
    f = archive_writer
    caps = {'inventory.json': 1024, 'unit-000.stdout': 16, 'index.json': 1024}
    f.owner.create(caps)
    values = {'inventory.json': b'{ "exact": 1 }\n', 'unit-000.stdout': b'', 'index.json': b'{}\n'}
    for name, raw in values.items():
        observed = f.owner.write(name, raw, caps[name])
        assert type(observed) is f.m.LargeFileSnapshot and not hasattr(observed, 'data')
        assert (f.owner.path/name).read_bytes() == raw
        assert observed.sha256 == hashlib.sha256(raw).hexdigest()
        assert observed.mode == 0o400 and observed.nlink == 1
    f.owner.verify()
    assert len(f.owner.rows) == 3 and f.owner.path.stat().st_mode & 0o777 == 0o700
    descriptor = f.owner._descriptor
    f.owner.close()
    with pytest.raises(OSError): os.fstat(descriptor)
    assert os.fstat(f.descriptor) and (f.owner.path/'index.json').read_bytes() == b'{}\n'


@pytest.mark.parametrize('case', ['duplicate', 'oversize', 'wrong_cap', 'unknown', 'early_index'])
def test_preflight_archive_refuses_partial_or_unbounded_publication(archive_writer, case):
    f = archive_writer
    caps = {'inventory.json': 8, 'index.json': 8}
    f.owner.create(caps)
    if case == 'duplicate': f.owner.write('inventory.json', b'{}\n', 8)
    name, raw, cap = 'inventory.json', b'{}\n', 8
    if case == 'oversize': raw = b'x'*9
    elif case == 'wrong_cap': cap = 9
    elif case == 'unknown': name = 'other.json'
    elif case == 'early_index': name = 'index.json'
    with pytest.raises(f.m.BootstrapError): f.owner.write(name, raw, cap)
    assert not (f.owner.path/'index.json').exists()


@pytest.mark.parametrize('case', ['bytes', 'mode', 'hardlink', 'symlink', 'extra', 'root', 'parent'])
def test_preflight_archive_rechecks_original_paths_and_file_identities(archive_writer, case):
    f = archive_writer; caps = {'inventory.json': 8, 'index.json': 8}
    f.owner.create(caps)
    f.owner.write('inventory.json', b'{}\n', 8); f.owner.write('index.json', b'{}\n', 8)
    path = f.owner.path/'inventory.json'
    if case == 'bytes': path.chmod(0o600); path.write_bytes(b'[]\n'); path.chmod(0o400)
    elif case == 'mode': path.chmod(0o644)
    elif case == 'hardlink': os.link(path, f.evidence/'foreign')
    elif case == 'symlink': path.unlink(); path.symlink_to(f.owner.path/'index.json')
    elif case == 'extra': (f.owner.path/'other').write_bytes(b'')
    elif case == 'root': f.owner.path.rename(f.evidence/'old'); f.owner.path.mkdir(mode=0o700)
    elif case == 'parent': f.evidence.chmod(0o755)
    with pytest.raises((f.m.BootstrapError, OSError)): f.owner.verify()


def test_preflight_archive_close_preserves_reused_foreign_descriptor(archive_writer):
    f = archive_writer; f.owner.create({'inventory.json': 8, 'index.json': 8})
    old = f.owner._descriptor; os.close(old)
    replacement = os.open(f.evidence, os.O_RDONLY | os.O_DIRECTORY)
    if replacement != old:
        os.dup2(replacement, old); os.close(replacement)
    try:
        f.owner.close()
        assert os.fstat(old).st_ino == f.evidence.stat().st_ino
    finally:
        os.close(old)


@pytest.mark.parametrize('mode', [0o500, 0o755])
def test_preflight_archive_mode_drift_rejects_and_closes_original_handle(archive_writer, mode):
    f = archive_writer
    f.owner.create({'inventory.json': 8, 'index.json': 8})
    descriptor = f.owner._descriptor
    f.owner.path.chmod(mode)
    with pytest.raises(f.m.BootstrapError): f.owner.verify()
    f.owner.close()
    with pytest.raises(OSError): os.fstat(descriptor)
    assert os.fstat(f.descriptor)


@pytest.mark.parametrize('drift', ['inheritable', 'nonblocking'])
def test_preflight_archive_close_preserves_changed_descriptor_flags(archive_writer, drift):
    import fcntl
    f = archive_writer
    f.owner.create({'inventory.json': 8, 'index.json': 8})
    descriptor = f.owner._descriptor
    if drift == 'inheritable': os.set_inheritable(descriptor, True)
    else: fcntl.fcntl(descriptor, fcntl.F_SETFL, fcntl.fcntl(descriptor, fcntl.F_GETFL) | os.O_NONBLOCK)
    try:
        with pytest.raises(f.m.BootstrapError): f.owner.verify()
        f.owner.close()
        assert os.fstat(descriptor)
    finally: os.close(descriptor)


def test_preflight_archive_failure_never_deletes_published_prefix(archive_writer, monkeypatch):
    f = archive_writer; f.owner.create({'inventory.json': 8, 'index.json': 8})
    f.owner.write('inventory.json', b'{}\n', 8)
    monkeypatch.setattr(f.m, '_capture_large_file_at', lambda *args, **kwargs: (_ for _ in ()).throw(OSError('readback')))
    with pytest.raises(OSError): f.owner.write('index.json', b'{}\n', 8)
    assert f.owner._phase == 'failed'
    with pytest.raises(f.m.BootstrapError): f.owner.verify()
    f.owner.close()
    assert (f.owner.path/'inventory.json').read_bytes() == b'{}\n'


@pytest.mark.parametrize('case', ['unchanged','growth','empty_growth','same_size','parent_symlink'])
def test_large_file_recheck_never_reads_beyond_original_size(tmp_path, monkeypatch, case):
    m=bootstrap_definitions(); root=tmp_path/'files';root.mkdir(mode=0o700)
    path=root/'member';path.write_bytes(b'' if case=='empty_growth' else b'exact');path.chmod(0o400)
    snapshot=m._capture_bounded_large_file(path,'original',maximum_bytes=5)
    if case in ('growth','empty_growth'):
        path.chmod(0o600);path.write_bytes(b'x'*64);path.chmod(0o400)
    elif case=='same_size':path.chmod(0o600);path.write_bytes(b'other');path.chmod(0o400)
    elif case=='parent_symlink':root.rename(tmp_path/'renamed');root.symlink_to(tmp_path/'renamed')
    calls=[];original=m.os.read
    def read(fd,size):calls.append(size);return original(fd,size)
    monkeypatch.setattr(m.os,'read',read)
    if case=='unchanged':m._require_large_file_unchanged(snapshot,'original')
    else:
        with pytest.raises(m.BootstrapError):m._require_large_file_unchanged(snapshot,'original')
    if case in ('growth','empty_growth','parent_symlink'):assert calls==[]


def test_preflight_archive_namespace_scan_bounds_every_yielded_entry(archive_writer,monkeypatch):
    f=archive_writer;f.owner.create({'inventory.json':8,'index.json':8});seen=[]
    class Entries:
        def __enter__(self):return self
        def __exit__(self,*args):pass
        def __iter__(self):
            for index in range(1000):
                seen.append(index);yield types.SimpleNamespace(name='inventory.json')
    monkeypatch.setattr(f.m.os,'scandir',lambda descriptor:Entries())
    with pytest.raises(f.m.BootstrapError):f.owner._guard()
    assert seen==[0,1,2]
