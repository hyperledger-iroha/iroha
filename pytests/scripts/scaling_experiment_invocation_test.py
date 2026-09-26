"""Actual invocation composition with explicit full-runtime and execution seams.

Real retained files, binary/source manifest parsers, five-file WorkerSourceFiles,
Mach-O header/hash ExecutableImage and plan/budget decode execute. Framework
closure, PythonDependencies.verify/validate, host observations, RuntimeAdmission.admit
and experiment native execution are explicit seams; no release claim follows.
"""
from dataclasses import fields, replace
import hashlib
import os
from pathlib import Path
import struct
from types import SimpleNamespace

import pytest
import scaling_experiment_invocation as invocation
import scaling_experiment_execution as service
import scaling_cli_bootstrap as bootstrap
import scaling_runtime_admission as admission_module
from scaling_experiment_cli_inputs import LaunchInputs
from scaling_experiment_custody_test import fixed_plan, budget_for
from scaling_experiment_plan import plan_bytes
from resource_evidence_budget import canonical_run_budget_bytes, select_run_budget
from scaling_worker_sources import SOURCE_NAMES, WorkerSourceFiles
from resource_process import ExecutableImage

ROOT = Path(__file__).resolve().parents[2]
SEED = 'ab' * 32


def digest(raw): return hashlib.sha256(raw).hexdigest()


@pytest.fixture
def assembled(monkeypatch, tmp_path):
    root = tmp_path.resolve(); events = []; owned = []
    def put(path, raw, mode=0o600):
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        path.write_bytes(raw); path.chmod(mode); return digest(raw)
    values = {field.name: ('a' * 64 if field.name.endswith('sha256') else
        'scripts/nexus/run_multilane_scaling_gate.py' if field.name == 'python_entrypoint' else root / field.name)
        for field in fields(admission_module.ReleaseRuntimePaths)}
    for name in ('source_root','cargo_target_root','artifact_root','binary_bundle','python_sources','python_evidence'):
        values[name].mkdir(mode=0o700)
    dependency_paths = bootstrap.PythonDependencyPaths(root/'dependency-source',root/'dependency-bundle',root/'dependency-inventory','a'*64)
    dependencies = object.__new__(bootstrap.PythonDependencies); dependencies._paths = dependency_paths
    monkeypatch.setattr(bootstrap.PythonDependencies, 'verify', lambda self: events.append(('dependencies_verify',)))
    monkeypatch.setattr(bootstrap.PythonDependencies, 'validate', lambda self: events.append(('dependencies_validate',)))
    worker_root = root/'workers'; worker_root.mkdir(mode=0o700)
    rows = []
    for name in SOURCE_NAMES:
        raw = (ROOT/'scripts/nexus'/name).read_bytes()
        sha = put(worker_root/name,raw)
        rows.append(dict(path='scripts/nexus/'+name,sha256=sha,size=len(raw)))
    rows.sort(key=lambda item:item['path'])
    python_raw = admission_module.artifact_contract.canonical_json_bytes({'schema':'shape-only-source-manifest','files':rows})
    values['python_source_manifest_sha256'] = put(values['python_source_manifest'],python_raw)
    rustc_raw = b'rustc 1.93.0\nhost: aarch64-apple-darwin\n'
    rustc_sha = put(values['rustc_version'],rustc_raw)
    manifest = {key:'shape-only' for key in invocation.binary_contract._KEYS}
    manifest.update(source_manifest_sha256='b'*64, rustc_version_sha256=rustc_sha)
    image_rows = []
    for index, role in enumerate(('kagami','iroha','irohad','python')):
        raw = struct.pack('<8I',0xfeedfacf,0x0100000c,0,2,1,24,0,0)+struct.pack('<II',0x1b,24)+bytes([index+1])*16
        path = values['python_evidence']/'python-runtime/bin/python3' if role=='python' else values['binary_bundle']/('release/'+role)
        sha = put(path,raw,0o500); image_rows.append((path,sha))
        if role!='python':manifest[role+'_relative_path']='release/'+role;manifest[role+'_sha256']=sha
    binary_raw=''.join(key+'\t'+manifest[key]+'\n' for key in invocation.binary_contract._KEYS).encode()
    values['binary_manifest_sha256']=put(values['binary_bundle']/invocation.binary_contract._MANIFEST_NAME,binary_raw)
    framework={'records':[dict(path='bin/python3',kind='file',mode='0500',size=56,sha256=image_rows[-1][1])]}
    values['python_runtime_binding_sha256']=put(values['python_runtime_binding'],invocation.receipt_contract._canonical_json(framework))
    def framework_seam(value, directory):
        assert value==framework and directory==values['python_evidence'];events.append(('framework_validate',));return ()
    monkeypatch.setattr(invocation.receipt_contract,'_validate_framework_python_runtime',framework_seam)
    host=admission_module.HostObservation('cpu',4,8,1<<30,'Darwin','kernel','arm64','3.12','private-node-name')
    monkeypatch.setattr(invocation,'observe_host',lambda:host)
    plan=fixed_plan();budget=budget_for(plan)
    plan_path=root/'plan.json';budget_path=root/'budget.json'
    plan_sha=put(plan_path,plan_bytes(plan));budget_sha=put(budget_path,canonical_run_budget_bytes(select_run_budget(budget,1,'one_lane')))
    launch=LaunchInputs(admission_module.ReleaseRuntimePaths(**values),dependency_paths,plan_path,plan_sha,budget_path,budget_sha,
        root/'evidence',root/'runtime',worker_root,'lab','storage','c'*40)
    admissions=[]
    def admit_seam(cls,paths,identity,runtime,workers,plan,got_dependencies):
        assert cls is admission_module.RuntimeAdmission and paths is launch.runtime_paths
        assert type(workers) is WorkerSourceFiles and all(type(image) is ExecutableImage for image in (runtime.kagami,runtime.cli,runtime.daemon,runtime.resource_program))
        assert got_dependencies is dependencies
        assert [(image.path,image.sha256) for image in (runtime.kagami,runtime.cli,runtime.daemon,runtime.resource_program)]==image_rows
        assert runtime.resource_worker==worker_root/SOURCE_NAMES[0] and runtime.resource_worker_sha256==digest((worker_root/SOURCE_NAMES[0]).read_bytes())
        assert identity.source_revision=='c'*40 and identity.workspace_source_sha256=='b'*64
        assert identity.rustc_version=='rustc 1.93.0' and identity.machine_id=='lab'
        assert not hasattr(identity,'node_name') and len(plan.trials)==10
        owner=object.__new__(cls);owner._runtime=runtime;owner._workers=workers;owner._identity=identity;owner._plan=plan
        admissions.append(owner);events.append(('runtime_admit',));return owner
    monkeypatch.setattr(admission_module.RuntimeAdmission,'admit',classmethod(admit_seam))
    monkeypatch.setattr(admission_module.RuntimeAdmission,'close',lambda self:events.append(('admission_close',)))
    def execute_seam(admission,evidence,runtime,budget,seed):
        assert admission is admissions[-1];events.append(('execute',evidence,runtime,seed));return 'execution-observation'
    monkeypatch.setattr(invocation,'execute_experiment',execute_seam)
    def create(value=None):
        result=invocation.InvocationResources.admit(launch if value is None else value,dependencies);owned.append(result);return result
    yield SimpleNamespace(root=root,launch=launch,dependencies=dependencies,create=create,events=events,admissions=admissions,put=put,owned=owned)
    for owner in reversed(owned):
        if owner._pending is not None:
            owner._pending._complete=True
        owner.close()


def test_actual_parser_file_worker_and_image_composition(assembled):
    owner=assembled.create()
    assert owner.run(SEED)=='execution-observation'
    assert assembled.events[-1]==('execute',assembled.launch.evidence_root,assembled.launch.runtime_root,SEED)
    assert ('framework_validate',) in assembled.events and ('runtime_admit',) in assembled.events
    assert not assembled.launch.evidence_root.exists() and not assembled.launch.runtime_root.exists()
    fds=[image.fd for image in owner._images]
    owner.close();owner.close()
    for fd in fds:
        with pytest.raises(OSError):os.fstat(fd)
    assert assembled.events.count(('admission_close',))==1


def test_seed_cannot_start_a_second_experiment(assembled):
    owner=assembled.create();owner.run(SEED)
    with pytest.raises(invocation.InvocationError):owner.run('cd'*32)
    assert len([event for event in assembled.events if event[0]=='execute'])==1


@pytest.mark.parametrize('kind',['same','nested','existing','source_tree','control_parent'])
def test_output_namespace_exclusion_before_runtime_admission(assembled,kind):
    launch=assembled.launch
    evidence={'same':launch.runtime_root,'nested':launch.runtime_root/'nested','existing':assembled.root,
        'source_tree':launch.runtime_paths.source_root/'trial',
        'control_parent':launch.plan_path.parent}[kind]
    changed=replace(launch,evidence_root=evidence)
    with pytest.raises((invocation.InvocationError,admission_module.RuntimeAdmissionError)):
        assembled.create(changed)
    assert ('runtime_admit',) not in assembled.events
    assert not launch.runtime_root.exists()


def test_output_parent_symlink_rejected_before_images_or_runtime(assembled):
    link=assembled.root/'alias';link.symlink_to(assembled.launch.runtime_paths.source_root,target_is_directory=True)
    changed=replace(assembled.launch,evidence_root=link/'output')
    with pytest.raises((invocation.InvocationError,admission_module.RuntimeAdmissionError,OSError)):
        assembled.create(changed)
    assert ('runtime_admit',) not in assembled.events
    assert not (assembled.launch.runtime_paths.source_root/'output').exists()


def test_original_output_paths_are_retained_after_launch_object_mutation(assembled):
    launch=assembled.launch;expected=(launch.evidence_root,launch.runtime_root);owner=assembled.create()
    object.__setattr__(launch,'evidence_root',launch.runtime_paths.source_root/'changed-output')
    try:owner.run(SEED)
    except invocation.InvocationError:pass
    assert all(event[1:3]==expected for event in assembled.events if event[0]=='execute')


def test_created_output_after_admission_rejected_before_execution(assembled):
    owner=assembled.create();assembled.launch.evidence_root.mkdir()
    with pytest.raises(invocation.InvocationError):owner.run(SEED)
    assert not any(event[0]=='execute' for event in assembled.events)


def test_changed_control_file_rejected_before_execution(assembled):
    owner=assembled.create();assembled.launch.plan_path.write_bytes(b'private-invalid-control')
    with pytest.raises(admission_module.RuntimeAdmissionError):owner.run(SEED)
    assert not any(event[0]=='execute' for event in assembled.events)


@pytest.mark.parametrize('role', (
    'plan', 'budget', 'python_source_manifest', 'rustc_version', 'binary_manifest',
    'python_runtime_binding', 'kagami', 'iroha', 'irohad', 'python',
    *(f'worker:{name}' for name in SOURCE_NAMES),
))
def test_original_candidate_and_worker_hashes_reject_mutation_before_runtime_admission(assembled, role):
    """Every executable and source pin is checked by its original admission owner."""
    launch=assembled.launch;paths=launch.runtime_paths
    candidates={
        'plan':launch.plan_path,
        'budget':launch.budget_path,
        'python_source_manifest':paths.python_source_manifest,
        'rustc_version':paths.rustc_version,
        'binary_manifest':paths.binary_bundle/invocation.binary_contract._MANIFEST_NAME,
        'python_runtime_binding':paths.python_runtime_binding,
        'kagami':paths.binary_bundle/'release/kagami',
        'iroha':paths.binary_bundle/'release/iroha',
        'irohad':paths.binary_bundle/'release/irohad',
        'python':paths.python_evidence/'python-runtime/bin/python3',
    }
    if role.startswith('worker:'):
        path=assembled.root/'workers'/role.partition(':')[2]
    else:
        path=candidates[role]
    mode=path.stat().st_mode & 0o777
    path.chmod(mode | 0o200)
    path.write_bytes(path.read_bytes()+b'\n')
    path.chmod(mode)
    with pytest.raises((ValueError,OSError)):
        assembled.create()
    assert ('runtime_admit',) not in assembled.events
    assert not any(event[0]=='execute' for event in assembled.events)
    assert not launch.evidence_root.exists() and not launch.runtime_root.exists()


def test_pending_cleanup_retains_all_original_inputs(assembled,monkeypatch):
    owner=assembled.create();original_images=tuple(owner._images);workers=owner._workers;admission=owner._admission
    state=SimpleNamespace(alive=True,closes=0)
    class ExperimentOwner:
        def close(self):
            state.closes+=1
            if state.alive:raise ValueError('pending synthetic child')
    pending=object.__new__(service.PendingExperimentCleanup)
    pending._retained=(ExperimentOwner(),object(),admission);pending._complete=pending._busy=False
    def fail(*args):raise service.ExperimentCleanupRequired(pending,'keyboard_interrupt')
    monkeypatch.setattr(invocation,'execute_experiment',fail)
    with pytest.raises(service.ExperimentCleanupRequired) as caught:owner.run(SEED)
    assert caught.value.cleanup is pending and owner._pending is pending
    assert owner._dependencies is assembled.dependencies and owner._admission is admission
    assert owner._workers is workers and tuple(owner._images)==original_images
    assert not owner.poll_cleanup()
    with pytest.raises(invocation.InvocationError):owner.close()
    assert not owner._files.closed and ('admission_close',) not in assembled.events
    for image in original_images:image.validate()
    workers.validate();state.alive=False
    assert owner.poll_cleanup();owner.close()
    assert owner._files.closed and assembled.events.count(('admission_close',))==1


def test_failed_admission_closes_already_open_images_and_workers(assembled,monkeypatch):
    captured=[]
    def fail(cls,*args):
        captured.extend((args[2],args[3]));raise ValueError('closed-test-marker')
    monkeypatch.setattr(admission_module.RuntimeAdmission,'admit',classmethod(fail))
    with pytest.raises(ValueError):assembled.create()
    runtime,workers=captured
    assert all(image.fd==-1 for image in (runtime.kagami,runtime.cli,runtime.daemon,runtime.resource_program))
    assert workers._closed and not workers._handles


def test_fresh_artifact_container_child_is_supported_by_actual_directory_owner(assembled,monkeypatch):
    from scaling_experiment_directories import ExperimentDirectories
    launch=replace(assembled.launch,evidence_root=assembled.launch.runtime_paths.artifact_root/'trial-evidence')
    owner=assembled.create(launch)
    def create_actual_directories(admission,evidence,runtime,budget,seed):
        directories=ExperimentDirectories(evidence,runtime)
        try:
            directories.validate()
            assert evidence==launch.evidence_root and evidence.parent==launch.runtime_paths.artifact_root
            assert sorted(path.name for path in evidence.iterdir())==['resources','runs']
        finally:directories.close()
        return 'actual-directories-created'
    monkeypatch.setattr(invocation,'execute_experiment',create_actual_directories)
    assert owner.run(SEED)=='actual-directories-created'


def test_replaced_output_parent_rejected_by_original_retained_control_owner(assembled):
    parent=assembled.root/'output-parent';parent.mkdir(mode=0o700)
    launch=replace(assembled.launch,evidence_root=parent/'evidence',runtime_root=parent/'runtime')
    owner=assembled.create(launch)
    parent.rename(assembled.root/'detached-parent');parent.mkdir(mode=0o700)
    with pytest.raises(admission_module.RuntimeAdmissionError):owner.run(SEED)
    assert not any(event[0]=='execute' for event in assembled.events)
    assert not list(parent.iterdir())


def test_actual_downstream_directory_owner_already_rejects_symlinked_parent(assembled):
    from scaling_experiment_directories import ExperimentDirectories
    link=assembled.root/'downstream-alias';link.symlink_to(assembled.launch.runtime_paths.source_root,target_is_directory=True)
    with pytest.raises(OSError):
        ExperimentDirectories(link/'evidence',assembled.launch.runtime_root)
    assert not (assembled.launch.runtime_paths.source_root/'evidence').exists()
    assert not assembled.launch.runtime_root.exists()
