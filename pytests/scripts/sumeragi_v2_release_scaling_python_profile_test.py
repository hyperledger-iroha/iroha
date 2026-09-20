"""Protected Python process arguments, using real callers and bounded process seams."""
from __future__ import annotations
import ast
import os
from pathlib import Path
import types
import pytest
from sumeragi_v2_release_scaling_validator_test import contract
from sumeragi_v2_release_scaling_main_test import put

ROOT=Path(__file__).resolve().parents[2]


class Captured(Exception):
    """The exact command reached the process owner; no process was started."""


@pytest.mark.parametrize('operation',('identity','copy_framework','verify_framework','tool_probe'))
def test_actual_protected_helper_calls_use_one_isolated_profile(contract,monkeypatch,operation):
    f=contract;m=f.m;calls=[]
    def capture(executable,argv,**kwargs):calls.append((executable,tuple(argv),kwargs));raise Captured()
    monkeypatch.setattr(m,'_run_bounded',capture)
    python=f.archives['python'];helper=f.archives['runtime_helper']
    fd=os.open(f.evidence,os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC)
    try:
        with pytest.raises(Captured):
            if operation=='identity':m._compute_identity(python.path,helper.path,f.candidate,f.environment,31)
            elif operation=='copy_framework':m._copy_framework_python_archive(evidence=f.evidence,
                protected_python=python,runtime_helper=helper,timeout_seconds=31)
            elif operation=='verify_framework':m._verify_framework_python_archive(evidence=f.evidence,
                protected_python=python,runtime_helper=helper,inventory=types.SimpleNamespace(path=f.evidence/'inventory'),
                marker_record={},timeout_seconds=31)
            else:m._run_tool_probe_closure(evidence=f.evidence,evidence_fd=fd,python=python,helper=helper,
                tools={'rustc':python},timeout_seconds=31)
    finally:os.close(fd)
    assert len(calls)==1 and calls[0][0]==python.path
    assert calls[0][1][:4]==('-I','-B','-S',str(helper.path))
    assert calls[0][2]['maximum_output_bytes']==m._MAX_HELPER_OUTPUT_BYTES
    assert calls[0][2]['environment']


def test_actual_probe_call_and_both_serialized_expectations_have_exact_same_argv(contract):
    f=contract;m,v=f.m,f.v;calls=[]
    def capture(executable,argv,**kwargs):calls.append((executable,tuple(argv)));return types.SimpleNamespace(returncode=0)
    tree=ast.parse((ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_bytes())
    probe=next(n for n in ast.walk(tree) if isinstance(n,ast.Assign) and len(n.targets)==1
        and isinstance(n.targets[0],ast.Name) and n.targets[0].id=='python_probe')
    code="import sys;sys.stdout.write(sys.executable+'\\n')"
    scope=dict(f.scope,_run_bounded=capture,python_probe_code=code,environment=f.environment,
        args=types.SimpleNamespace(command_timeout_seconds=31))
    exec(compile(ast.Module(body=[probe],type_ignores=[]),'<actual bounded probe call>','exec'),scope)
    marker=next(n.value for n in ast.walk(tree) if isinstance(n,ast.Assign) and len(n.targets)==1
        and isinstance(n.targets[0],ast.Name) and n.targets[0].id=='marker_value')
    probes=next(value for key,value in zip(marker.keys,marker.values) if isinstance(key,ast.Constant) and key.value=='trusted_execution_probes')
    python=next(value for key,value in zip(probes.keys,probes.values) if isinstance(key,ast.Constant) and key.value=='python')
    argv=next(value for key,value in zip(python.keys,python.values) if isinstance(key,ast.Constant) and key.value=='argv')
    produced=eval(compile(ast.Expression(argv),'<actual serialized probe>','eval'),scope)
    validator=ast.parse((ROOT/'scripts/validate_sumeragi_v2_release_bootstrap.py').read_bytes())
    expected=next(n for n in ast.walk(validator) if isinstance(n,ast.List) and len(n.elts)==6
        and all(isinstance(n.elts[index],ast.Constant) and n.elts[index].value==flag
            for index,flag in ((1,'-I'),(2,'-B'),(3,'-S'),(4,'-c'))))
    consumed=eval(compile(ast.Expression(expected),'<actual standalone probe expectation>','eval'),scope)
    assert produced==consumed==[str(f.archives['python'].path),'-I','-B','-S','-c',code]
    assert calls==[(f.archives['python'].path,tuple(produced[1:]))]


def test_all_owned_protected_python_literals_use_canonical_profile_and_no_missing_B():
    counts=[]
    for name in ('bootstrap_sumeragi_v2_release.py','validate_sumeragi_v2_release_bootstrap.py'):
        tree=ast.parse((ROOT/'scripts'/name).read_bytes());count=0
        for node in ast.walk(tree):
            if not isinstance(node,(ast.List,ast.Tuple)):continue
            for index,item in enumerate(node.elts):
                if isinstance(item,ast.Constant) and item.value=='-I':
                    flags=node.elts[index:index+3]
                    assert len(flags)==3 and all(isinstance(value,ast.Constant) for value in flags)
                    assert tuple(value.value for value in flags)==('-I','-B','-S')
                    count+=1
        counts.append(count)
    assert counts[0]>=8 and counts[1]==3



def test_actual_standalone_tool_probe_replay_uses_isolated_profile(contract,monkeypatch):
    f=contract;v=f.v;calls=[]
    tool=types.SimpleNamespace(data=b'inert tool',sha256='f'*64)
    tools={f'tool-{index}':tool for index in range(41)}
    rows={name:dict(archive_id=f'release-runner-tool.{name}.v1',mode='0500',sha256=tool.sha256,
        size_bytes=len(tool.data),exit_status=0,invocation_sha256='a'*64,operation_id='fixture',
        postcondition_sha256='b'*64,stderr_sha256='c'*64,stderr_size_bytes=0,
        stdout_sha256='d'*64,stdout_size_bytes=0) for name in tools}
    result=dict(format='iroha-sumeragi-v2-release-tool-functional-probes',host_family='fixture',
        probe_contract_sha256='e'*64,schema_version=1,tool_count=41,tools=rows)
    closure={'value':result}
    for key,name,value,archive_id in (
        ('manifest','runner-tool-probe-manifest.json',{},'release-bootstrap.runner-tool-probe-manifest.v1'),
        ('result','runner-tool-probes.json',result,'release-bootstrap.runner-tool-probes.v1')):
        path=f.evidence/name;put(path,v._canonical_json(value))
        snapshot=v._read_path(path,'fixture probe',maximum_bytes=v._MAX_HELPER_OUTPUT_BYTES,expected_mode=0o400)
        closure[key]=dict(archive_id=archive_id,archive_name=name,mode='0400',sha256=snapshot.sha256,size_bytes=len(snapshot.data))
    def capture(executable,argv,**kwargs):calls.append((executable,tuple(argv),kwargs));raise Captured()
    monkeypatch.setattr(v,'_run_bounded',capture)
    evidence=v._open_directory(f.evidence,'fixture evidence',expected_mode=0o700)
    try:
        with pytest.raises(Captured):v._replay_runner_tool_probes(closure,evidence=evidence,
            archives=f.archives,tools=tools,environment=f.environment)
    finally:os.close(evidence.descriptor)
    assert calls[0][0]==f.archives['python'].path
    assert calls[0][1][:4]==('-I','-B','-S',str(f.archives['tool_probe_helper'].path))
    assert calls[0][2]['environment']==f.environment


@pytest.mark.parametrize('missing',('isolated','dont_write_bytecode','no_site'))
def test_standalone_startup_rejects_any_missing_profile_flag_before_paths(contract,monkeypatch,missing):
    f=contract;flags=dict(isolated=1,dont_write_bytecode=1,no_site=1);flags[missing]=0
    monkeypatch.setattr(f.v,'sys',types.SimpleNamespace(flags=types.SimpleNamespace(**flags)))
    with pytest.raises(f.v.ValidationError,match='-I -B -S'):f.v.validate(types.SimpleNamespace())
