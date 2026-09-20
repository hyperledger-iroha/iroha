"""Protected Python contract tests with explicit flag seams and no child launch."""
from __future__ import annotations
import ast
import copy
import hashlib
import itertools
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import types

import pytest

ROOT=Path(__file__).resolve().parents[2]
BOOTSTRAP='scripts/bootstrap_sumeragi_v2_release.py'
CORRIDOR='scripts/write_sumeragi_v2_release_receipt_corridor_log.py'
CACHE='scripts/copy_sumeragi_v2_release_cargo_cache.py'
FLAGS=['-I','-B','-S']


class Rejection(RuntimeError): pass
class ReachedInspection(RuntimeError): pass


@pytest.fixture(autouse=True)
def no_children_or_signals(monkeypatch):
    def forbidden(*args,**kwargs): raise AssertionError('child execution is forbidden')
    monkeypatch.setattr(subprocess,'Popen',forbidden)
    for name in ('system','fork','posix_spawn','posix_spawnp','kill','killpg'):
        if hasattr(os,name): monkeypatch.setattr(os,name,forbidden)


def load_selected(path,functions=(),constants=(),extra=None,cls=None):
    """Evaluate exact selected source definitions, excluding all module startup."""
    tree=ast.parse((ROOT/path).read_text());body=[]
    nodes=tree.body
    if cls is not None:
        nodes=next(node.body for node in nodes if isinstance(node,ast.ClassDef) and node.name==cls)
    for node in nodes:
        if isinstance(node,(ast.FunctionDef,ast.AsyncFunctionDef)) and node.name in functions:
            body.append(node)
        elif isinstance(node,ast.Assign) and any(isinstance(t,ast.Name) and t.id in constants for t in node.targets):
            body.append(node)
    assert len(body)==len(functions)+len(constants)
    future=ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)
    module=ast.fix_missing_locations(ast.Module(body=[future,*body],type_ignores=[]))
    namespace={'json':json,'hashlib':hashlib,'os':os,'re':re,'Path':Path,
        'BootstrapError':Rejection,'CacheCopyError':Rejection,'ReceiptError':Rejection,
        '_DIGEST_RE':re.compile('[0-9a-f]{64}')}
    namespace.update(extra or {})
    exec(compile(module,str(ROOT/path),'exec'),namespace)
    return namespace


def flags(values):
    return types.SimpleNamespace(isolated=values[0],dont_write_bytecode=values[1],no_site=values[2])


@pytest.mark.parametrize('values',list(itertools.product((0,1),repeat=3)))
def test_bootstrap_requires_all_three_before_candidate_inspection(values):
    calls=[]
    def inspect(*args): calls.append(args);raise ReachedInspection()
    module=load_selected(BOOTSTRAP,('bootstrap',),extra={
        'sys':types.SimpleNamespace(flags=flags(values)), '_absolute_resolved_existing':inspect})
    if values==(1,1,1):
        with pytest.raises(ReachedInspection): module['bootstrap'](types.SimpleNamespace(candidate_root=Path('/candidate')))
        assert len(calls)==1
    else:
        with pytest.raises(Rejection,match='-I -B -S'): module['bootstrap'](types.SimpleNamespace(candidate_root=Path('/candidate')))
        assert calls==[]


@pytest.mark.parametrize('values',list(itertools.product((0,1),repeat=3)))
def test_receipt_ack_requires_all_three_before_path_or_file_inspection(values):
    calls=[]
    class Root:
        @property
        def parent(self): calls.append(True);raise ReachedInspection()
    module=load_selected(CORRIDOR,('_publish_receipt_validation_ack',),extra={
        'sys':types.SimpleNamespace(flags=flags(values))})
    args=dict(ack_path=Path('/ack'),receipt_path=Path('/receipt'),release_root=Root(),
        source_manifest_sha256='1'*64,bootstrap_completion_sha256='2'*64,revalidate=lambda:None)
    if values==(1,1,1):
        with pytest.raises(ReachedInspection): module['_publish_receipt_validation_ack'](**args)
        assert calls==[True]
    else:
        with pytest.raises(Rejection,match='-I -B -S'): module['_publish_receipt_validation_ack'](**args)
        assert calls==[]


@pytest.mark.parametrize('values',list(itertools.product((0,1),repeat=3)))
def test_existing_package_admission_keeps_its_original_strict_guard(values):
    calls=[]
    def verify(): calls.append(True);raise ReachedInspection()
    def require(value):
        if not value: raise Rejection()
    def fail(error): raise error
    module=load_selected('scripts/nexus/scaling_cli_bootstrap.py',('load',),cls='PythonDependencies',extra={
        'sys':types.SimpleNamespace(flags=flags(values),modules={}), '_require':require,'_failure':fail})
    owner=types.SimpleNamespace(_loaded=False,_busy=False,verify=verify)
    if values==(1,1,1):
        with pytest.raises(ReachedInspection): module['load'](owner)
        assert calls==[True]
    else:
        with pytest.raises(Rejection): module['load'](owner)
        assert calls==[]
    assert owner._failed is True


def record_fixture():
    module=load_selected(CORRIDOR,('_receipt_validation_invocation_value_sha256','_receipt_validation_invocation_binding'),
        ('_RECEIPT_VALIDATION_OPTION_ORDER','_RECEIPT_VALIDATION_PATH_OPTIONS'))
    args=[];expected={}
    for i,name in enumerate(module['_RECEIPT_VALIDATION_OPTION_ORDER']):
        args.append(name)
        kind='flag' if name=='--verify-existing' else 'path' if name in module['_RECEIPT_VALIDATION_PATH_OPTIONS'] else 'text'
        value=True if kind=='flag' else '/owned/value-'+str(i) if kind=='path' else 'exact-selected-value'
        expected[name]=(kind,value)
        if kind!='flag': args.append(value)
    return module['_receipt_validation_invocation_binding'](args),expected


def validator(path):
    prefix='_' if path==BOOTSTRAP else ''
    module=load_selected(path,('_validator_invocation_value_sha256','_validate_validator_invocation'),
        (prefix+'VALIDATOR_OPTION_ORDER',prefix+'VALIDATOR_PATH_OPTIONS'))
    return module['_validate_validator_invocation']


@pytest.mark.parametrize('path',[BOOTSTRAP,CACHE])
def test_canonical_receipt_record_passes_both_independent_consumers(path):
    record,expected=record_fixture()
    assert record['python_flags']==FLAGS
    core={k:v for k,v in record.items() if k!='invocation_sha256'}
    digest=hashlib.sha256(json.dumps(core,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()
    assert record['invocation_sha256']==digest
    assert validator(path)(record,expected_values=expected) is None


@pytest.mark.parametrize('path',[BOOTSTRAP,CACHE])
@pytest.mark.parametrize('bad',[['-I','-S'],['-B','-S'],['-I','-B'],['-I','-S','-B'],
    ['-I','-B','-S','-B'],('-I','-B','-S'),['-I',True,'-S'],[],None])
def test_rehashed_old_reordered_or_foreign_flag_record_is_rejected(path,bad):
    record,expected=record_fixture();record['python_flags']=bad
    core={k:v for k,v in record.items() if k!='invocation_sha256'}
    record['invocation_sha256']=hashlib.sha256(json.dumps(core,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()
    with pytest.raises(Rejection,match='not exact'): validator(path)(record,expected_values=expected)


def fixed_failure_record(path):
    tree=ast.parse((ROOT/path).read_text());records=[]
    for node in ast.walk(tree):
        if isinstance(node,ast.Dict) and {key.value for key in node.keys if isinstance(key,ast.Constant)}=={
            'profile','python_flags','validator','operation','invocation_binding'}:
            records.append(ast.literal_eval(node))
    assert len(records)==1
    return records[0]


def test_failure_record_producer_and_original_parent_replay_use_exact_same_flags():
    producer=fixed_failure_record('scripts/copy_sumeragi_v2_release_cargo_cache_validation_ack.py')
    consumer=fixed_failure_record('scripts/bootstrap_sumeragi_v2_release_receipt_replay.py')
    assert producer==consumer and producer['python_flags']==FLAGS
    assert producer['invocation_binding']=='not-published-validation-failed'


def test_actual_protected_launches_and_fixture_index_match_new_flag_contract():
    shell=(ROOT/'scripts/run_sumeragi_v2_release_gates.sh').read_text()
    assert shell.count('"$release_python_bin" -I -B -S "$release_bootstrap_evidence_dir/validate-receipt.py"')==1
    assert '"$release_python_bin" -I -S "$release_bootstrap_evidence_dir/validate-receipt.py"' not in shell
    fixture=(ROOT/'pytests/scripts/sumeragi_v2_release_bootstrap_test.py').read_text()
    assert '"-I",\n            "-B",\n            "-S",\n            str(BOOTSTRAP)' in fixture
    assert '[str(PYTHON), "-I", "-B", "-S", str(copied), "--help"]' in fixture
    assert 'python3 -I -B -S "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/validate-receipt.py"' in fixture
    terminal=(ROOT/'pytests/scripts/sumeragi_v2_release_bootstrap_terminal_cases.py').read_text()
    assert 'del arguments[1:4]' in terminal and 'del arguments[1:3]' not in terminal
    assert terminal.count('arguments[4] = str(')==2 and 'arguments[3] = str(' not in terminal
    assert terminal.count('python3 -I -B -S "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/validate-receipt.py"')==2


def test_bootstrap_documented_invocation_uses_the_same_contract():
    bootstrap=(ROOT/BOOTSTRAP).read_text();spec=(ROOT/'specs/sumeragi_v2_liveness.md').read_text()
    assert '/absolute/python3 -I -B -S /absolute/bootstrap_sumeragi_v2_release.py' in bootstrap
    assert '/protected/python3 -I -B -S /protected/bootstrap_sumeragi_v2_release.py' in spec
    assert 'isolated, no-bytecode, no-site mode (`-I -B -S`)' in spec


def test_registered_python_sources_form_a_closed_local_import_set():
    source=ROOT/'scripts/nexus/scaling_cli_bootstrap.py'
    tree=ast.parse(source.read_text())
    registry=next(ast.literal_eval(node.value) for node in tree.body if isinstance(node,ast.Assign)
        and any(isinstance(target,ast.Name) and target.id=='PYTHON_SOURCE_FILES' for target in node.targets))
    assert len(registry)==58 and tuple(sorted(set(registry)))==registry
    local={path.stem:path.relative_to(ROOT).as_posix() for directory in ('scripts','scripts/nexus')
        for path in (ROOT/directory).glob('*.py')}
    for relative in registry:
        assert (ROOT/relative).is_file()
        for node in ast.walk(ast.parse((ROOT/relative).read_text())):
            names=[alias.name.split('.')[0] for alias in node.names] if isinstance(node,ast.Import) else (
                [node.module.split('.')[0]] if isinstance(node,ast.ImportFrom) and node.module else [])
            for name in names:
                if name in local: assert local[name] in registry


def test_shell_guard_receipt_record_join_and_support_binding_are_current():
    after=(ROOT/'scripts/run_sumeragi_v2_release_gates.sh').read_text()
    assert 'original protected parent runs the complete source-bound scaling preflight' in after
    assert 'parent scaling observation receipt join is not integrated' not in after
    assert after.count('--scaling-execution-record "$release_scaling_execution_record"')==2
    assert after.count('--expected-scaling-execution-sha256 "$release_scaling_execution_sha256"')==2
    assert '"$release_python_bin" -I -B -S "$release_bootstrap_evidence_dir/validate-receipt.py"' in after
    support_digest=hashlib.sha256((ROOT/'scripts/run_sumeragi_v2_release_gates_support.sh').read_bytes()).hexdigest()
    assert 'readonly release_runner_support_sha256="'+support_digest+'"' in after


def test_current_component_consumers_bind_exact_component_bytes():
    def assignments(path):
        values={}
        for node in ast.parse((ROOT/path).read_text()).body:
            if isinstance(node,ast.Assign):
                try: value=ast.literal_eval(node.value)
                except (ValueError,TypeError): continue
                for target in node.targets:
                    if isinstance(target,ast.Name): values[target.id]=value
        return values
    consumers={path:assignments(path) for path in (BOOTSTRAP,CACHE,'scripts/write_sumeragi_v2_release_receipt.py')}
    components=['bootstrap_sumeragi_v2_release_receipt_replay.py',Path(CORRIDOR).name,
        'copy_sumeragi_v2_release_cargo_cache_validation_ack.py']
    for name in components:
        expected=hashlib.sha256((ROOT/'scripts'/name).read_bytes()).hexdigest();bindings=[]
        for values in consumers.values():
            for constant,value in values.items():
                if type(value) is dict and name in value and 'SHA256' in constant: bindings.append(value[name])
                if name=='copy_sumeragi_v2_release_cargo_cache_validation_ack.py' and constant=='VALIDATION_ACK_COMPONENT_SHA256': bindings.append(value)
        assert bindings and all(value==expected for value in bindings)


def test_component_admission_uses_its_declared_exact_digest_tables():
    source=(ROOT/BOOTSTRAP).read_text()
    assert 'snapshot.sha256 != _BOOTSTRAP_COMPONENT_SHA256[filename]' in source
    assert 'set(_BOOTSTRAP_COMPONENT_FILES) != set(_BOOTSTRAP_COMPONENT_SHA256)' in source
    cache=(ROOT/CACHE).read_text()
    assert 'hashlib.sha256(payload).hexdigest() != VALIDATION_ACK_COMPONENT_SHA256' in cache
    receipt=(ROOT/'scripts/write_sumeragi_v2_release_receipt.py').read_text()
    assert '_RELEASE_RECEIPT_COMPONENT_SHA256[filename]' in receipt
