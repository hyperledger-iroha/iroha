"""Actual protected producer metadata and standalone consumer, without children."""
from __future__ import annotations
import ast
import hashlib
import inspect
import json
import os
from pathlib import Path
import sys
import types
import pytest
from sumeragi_v2_release_scaling_main_test import put

ROOT=Path(__file__).resolve().parents[2]


def load(name,path,monkeypatch):
    module=types.ModuleType(name);module.__file__=str(path)
    monkeypatch.setitem(sys.modules,name,module)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module


@pytest.fixture
def contract(tmp_path,monkeypatch):
    m=load('scaling_actual_bootstrap_main',ROOT/'scripts/bootstrap_sumeragi_v2_release.py',monkeypatch)
    v=load('scaling_actual_bootstrap_validator',ROOT/'scripts/validate_sumeragi_v2_release_bootstrap.py',monkeypatch)
    evidence=tmp_path/'evidence';evidence.mkdir(mode=0o700)
    candidate=tmp_path/'candidate';candidate.mkdir(mode=0o700)
    base=tmp_path/'base+plus';base.mkdir(mode=0o700)
    invocation=base/'iroha-sumeragi-v2-release.fixture';invocation.mkdir(mode=0o700)
    (invocation/'source').mkdir(mode=0o500)
    names=('python','git','bash','ssh_keygen','runtime_helper','tool_probe_helper','allowed_signers',
        'revocation','sdk_dependency_bundle_manifest','scaling_handoff_helper')
    archives={}
    for name in names:
        path=evidence/v._TRUSTED_ARCHIVE_NAMES[name]
        put(path,b'protected '+name.encode())
        if name in ('python','git','bash','ssh_keygen'):path.chmod(0o500)
        archives[name]=types.SimpleNamespace(path=path,sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    if archives['python'].path.name!='python3':raise AssertionError('profile has no python3 alias')
    runner_bin=evidence/'runner-bin';runner_bin.mkdir(mode=0o700)
    identity_outputs={'attestation':evidence/'identity-attestation.json','transcript':evidence/'identity-transcript.json'}
    identity_snapshot=types.SimpleNamespace(path=evidence/'candidate-identity.json')
    scope=dict(m.__dict__,archives=archives,protected=archives,evidence=evidence,runner_bin=runner_bin,
        identity_outputs=identity_outputs,identity_snapshot=identity_snapshot,
        scaling_invocation=types.SimpleNamespace(path=invocation,base=base),
        scaling_handoff=types.SimpleNamespace(runner_descriptor=27,challenge='c'*64),
        scaling_operation=types.SimpleNamespace(invocation_sha256='d'*64),
        args=types.SimpleNamespace(expected_signer_fingerprint='SHA256:'+'A'*43,
            scaling_preflight_timeout_seconds=28800),runner_extra_environment={'CARGO_HOME':str(tmp_path/'cargo')})
    # Execute the exact production expression sequence. No fixture copy of the
    # handoff field names, policy aliases or closed environment is authoritative.
    main=next(n for n in ast.parse((ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_bytes()).body
        if isinstance(n,ast.FunctionDef) and n.name=='bootstrap')
    target_names={'scaling_environment','completion_path','policy_environment_without_self_digest',
        'alias_environment_without_self_digest','runner_environment_without_self_digest'}
    selected=[]
    for n in ast.walk(main):
        if (isinstance(n,ast.Assign) and len(n.targets)==1 and isinstance(n.targets[0],ast.Name)
                and n.targets[0].id in target_names):selected.append(n)
        elif (isinstance(n,ast.Expr) and isinstance(n.value,ast.Call) and isinstance(n.value.func,ast.Attribute)
              and isinstance(n.value.func.value,ast.Name) and n.value.func.value.id=='alias_environment_without_self_digest'):
            selected.append(n)
    assert len(selected)==6
    exec(compile(ast.Module(body=sorted(selected,key=lambda n:n.lineno),type_ignores=[]),'<actual producer metadata>','exec'),scope)
    yield types.SimpleNamespace(m=m,v=v,evidence=evidence,candidate=candidate,invocation=invocation,
        archives=archives,scope=scope,handoff=scope['scaling_environment'],environment=scope['runner_environment_without_self_digest'],
        identity_records={key:types.SimpleNamespace(path=identity_outputs[name]) for key,name in
            (('identity_attestation','attestation'),('identity_transcript','transcript'))})


def test_actual_protected_component_startup_and_new_signature_contract(contract):
    f=contract
    for name,row in f.m._BOOTSTRAP_COMPONENT_SOURCES.items():
        assert row.sha256==f.m._BOOTSTRAP_COMPONENT_SHA256[name]==f.v._BOOTSTRAP_COMPONENT_SHA256[name]
    assert f.m._RECEIPT_VALIDATOR_COMPONENT_SHA256==f.v._RECEIPT_VALIDATOR_COMPONENT_SHA256
    for function in (f.m._retained_release_layout,f.m._validate_terminal_receipt,f.m._run_protected_receipt_validator):
        assert inspect.signature(function).parameters['scaling_execution'].default is inspect.Parameter.empty
    assert '--expected-scaling-execution-sha256' in f.m._VALIDATOR_OPTION_ORDER
    assert not any('scaling-execution-record-sha256' in value for value in f.m._VALIDATOR_OPTION_ORDER)


def test_actual_producer_handoff_maps_exactly_to_standalone_consumer(contract):
    f=contract;digest=f.archives['scaling_handoff_helper'].sha256
    decoded=f.v._scaling_handoff_contract(f.handoff,types.SimpleNamespace(path=f.evidence),f.candidate,digest)
    assert decoded==f.handoff and decoded is not f.handoff
    for label,name in (('scaling_plan','scaling-plan.json'),('scaling_budget','scaling-budget.json'),('scaling_handoff_helper','scaling-handoff.py')):
        assert label in f.v._TRUSTED_INPUT_KEYS and f.v._TRUSTED_ARCHIVE_NAMES[label]==name
    assert f.v._artifact_limit('scaling_plan')==f.v._artifact_limit('scaling_budget')==1024*1024


@pytest.mark.parametrize('mutation',('extra','missing','bool','zero','too_large','leading_zero','helper','digest','relative','parent','overlap','dotdot'))
def test_handoff_rejects_foreign_or_noncanonical_metadata(contract,mutation):
    f=contract;value=dict(f.handoff)
    key='IROHA_RELEASE_SCALING_GATE_FD'
    if mutation=='extra':value['unexpected']='x'
    elif mutation=='missing':del value[key]
    elif mutation=='bool':value[key]=True
    elif mutation=='zero':value[key]='0'
    elif mutation=='too_large':value[key]=str(1<<20)
    elif mutation=='leading_zero':value[key]='027'
    elif mutation=='helper':value['IROHA_RELEASE_SCALING_HANDOFF_HELPER_SHA256']='f'*64
    elif mutation=='digest':value['IROHA_RELEASE_SCALING_CHALLENGE']='C'*64
    elif mutation=='relative':value['IROHA_RELEASE_INVOCATION_ROOT']='relative'
    elif mutation=='parent':value['IROHA_RELEASE_TEMP_BASE']=str(f.candidate)
    elif mutation=='overlap':
        value['IROHA_RELEASE_INVOCATION_ROOT']=str(f.candidate/'nested');value['IROHA_RELEASE_TEMP_BASE']=str(f.candidate)
    elif mutation=='dotdot':value['IROHA_RELEASE_INVOCATION_ROOT']=str(f.invocation)+'/../other'
    with pytest.raises(f.v.ValidationError):f.v._scaling_handoff_contract(value,types.SimpleNamespace(path=f.evidence),f.candidate,f.archives['scaling_handoff_helper'].sha256)


def test_actual_producer_closed_environment_passes_entry_and_channel_free_sealed(contract,monkeypatch):
    f=contract;v=f.v;marker='a'*64
    runtime={'PWD':str(f.candidate),'SHLVL':'1','_':str(f.archives['python'].path)}
    if sys.platform=='darwin':runtime['__CF_USER_TEXT_ENCODING']=f'0x{os.geteuid():X}:0x1:0xE'
    current={**f.environment,**{name:marker for name in v._SELF_DIGEST_VARIABLES},**runtime}
    with monkeypatch.context() as patch:
        patch.setattr(os,'environ',current)
        v._environment_contract(f.environment,types.SimpleNamespace(path=f.evidence),f.candidate,f.archives,
            f.identity_records,'SHA256:'+'A'*43,marker,f.scope['runner_bin'],'entry',f.handoff)
        for name in v._SCALING_CHANNEL_KEYS:del current[name]
        v._environment_contract(f.environment,types.SimpleNamespace(path=f.evidence),f.candidate,f.archives,
            f.identity_records,'SHA256:'+'A'*43,marker,f.scope['runner_bin'],'sealed',f.handoff)
        assert not any(name in os.environ for name in v._SCALING_CHANNEL_KEYS)
        changed=dict(f.environment,IROHA_RELEASE_SCALING_CONFIGURATION_SHA256='f'*64)
        with pytest.raises(v.ValidationError):v._environment_contract(changed,types.SimpleNamespace(path=f.evidence),f.candidate,f.archives,
            f.identity_records,'SHA256:'+'A'*43,marker,f.scope['runner_bin'],'sealed',f.handoff)


def test_sealed_source_uses_authenticated_root_without_environment_descriptor_or_path(contract,monkeypatch):
    f=contract
    with monkeypatch.context() as patch:
        patch.setattr(os,'environ',{})
        source,result=f.v._sealed_release_root(types.SimpleNamespace(path=f.evidence),f.handoff)
    assert source==f.invocation/'source' and result is None


@pytest.mark.parametrize('mutation',(None,'missing','alias','bytes'))
def test_actual_producer_trusted_records_are_consumed_with_original_caps(contract,monkeypatch,mutation):
    f=contract;m,v=f.m,f.v;archives={}
    for label,name in v._TRUSTED_ARCHIVE_NAMES.items():
        path=f.evidence/name;put(path,('protected '+label).encode())
        path.chmod(0o500 if label in v._EXECUTABLE_INPUTS else 0o400)
        archives[label]=m._read_file(path,'fixture protected input',maximum_bytes=16*1024*1024)
    components={}
    for group,pins in (('bootstrap',v._BOOTSTRAP_COMPONENT_SHA256),('receipt',v._RECEIPT_VALIDATOR_COMPONENT_SHA256)):
        components[group]={}
        for name,digest in pins.items():
            path=f.evidence/name;put(path,(ROOT/'scripts'/name).read_bytes())
            row=m._read_file(path,'fixture component',maximum_bytes=16*1024*1024)
            assert row.sha256==digest;components[group][name]=row
    tree=ast.parse((ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_bytes())
    owner=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='bootstrap')
    statements=[]
    for n in ast.walk(owner):
        if isinstance(n,ast.Assign) and len(n.targets)==1:
            target=n.targets[0]
            if ((isinstance(target,ast.Name) and target.id=='trusted_input_records')
                or (isinstance(target,ast.Subscript) and isinstance(target.value,ast.Name)
                    and target.value.id=='trusted_input_records')):statements.append(n)
    assert len(statements)==4
    scope=dict(m.__dict__,archives=archives,protected=archives,framework_python_record={},
        bootstrap_component_archives=components['bootstrap'],receipt_component_archives=components['receipt'])
    exec(compile(ast.Module(body=sorted(statements,key=lambda n:n.lineno),type_ignores=[]),'<actual producer records>','exec'),scope)
    records=scope['trusted_input_records']
    # Framework byte verification has its existing separate suite. This test's
    # explicit framework seam isolates the unchanged source/archive data reader.
    monkeypatch.setattr(v,'_validate_framework_python_runtime',lambda *a:(None,None))
    if mutation=='missing':del records['scaling_plan']
    elif mutation=='alias':records['scaling_plan']['archive_name']='old-scaling-plan.json'
    elif mutation=='bytes':put(f.evidence/'scaling-budget.json',b'changed')
    evidence=v._open_directory(f.evidence,'fixture evidence',expected_mode=0o700)
    try:
        if mutation:
            with pytest.raises(v.ValidationError):v._trusted_inputs(records,evidence,f.candidate)
        else:
            checked,_,_=v._trusted_inputs(records,evidence,f.candidate)
            assert checked['scaling_plan'].data==archives['scaling_plan'].data
            assert checked['scaling_budget'].sha256==archives['scaling_budget'].sha256
            assert checked['scaling_handoff_helper'].data==archives['scaling_handoff_helper'].data
    finally:os.close(evidence.descriptor)


@pytest.mark.parametrize('mutation',(None,'partial','extra','writable','symlink','entry','retained_600','retained_400','retained_missing','preflight_missing','preflight_writable','preflight_symlink'))
def test_actual_bounded_top_level_inventory_tracks_complete_late_group(contract,mutation):
    f=contract;v=f.v
    # Create the existing required inventory from the actual predicate expression.
    tree=ast.parse((ROOT/'scripts/validate_sumeragi_v2_release_bootstrap.py').read_bytes())
    method=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_inventory')
    expression=next(n.value for n in method.body if isinstance(n,ast.Assign)
        and isinstance(n.targets[0],ast.Name) and n.targets[0].id=='required')
    required=eval(compile(ast.Expression(expression),'<existing inventory>','eval'),v.__dict__)
    if v._FRAMEWORK_PYTHON:required.update(('python-runtime','python-runtime-input.json'))
    directories={'home','tmp','runner-bin','runner-tools','python-runtime'}
    for name in required:
        path=f.evidence/name
        if name in directories:
            path.mkdir(exist_ok=True,mode=0o700);path.chmod(0o500 if name=='python-runtime' else 0o700)
        elif not path.exists():put(path,b'fixture')
    for name in ('runner-stdout.log','runner-stderr.log'):(f.evidence/name).chmod(0o600)
    group={'scaling-source-paths.txt','scaling-rustc-version.txt','scaling-python-runtime.json','scaling-execution.json','scaling-verifier-python','scaling-preflight'}
    for name in group:
        path=f.evidence/name
        if name in {'scaling-verifier-python','scaling-preflight'}:path.mkdir(mode=0o700)
        else:put(path,b'fixture')
    if mutation=='partial':(f.evidence/'scaling-execution.json').unlink()
    elif mutation=='extra':put(f.evidence/'unexpected',b'foreign')
    elif mutation=='writable':(f.evidence/'scaling-execution.json').chmod(0o644)
    elif mutation=='symlink':
        (f.evidence/'scaling-execution.json').unlink();(f.evidence/'scaling-execution.json').symlink_to('scaling-python-runtime.json')
    elif mutation=='preflight_missing':(f.evidence/'scaling-preflight').rmdir()
    elif mutation=='preflight_writable':(f.evidence/'scaling-preflight').chmod(0o777)
    elif mutation=='preflight_symlink':
        (f.evidence/'scaling-preflight').rmdir();(f.evidence/'scaling-preflight').symlink_to('scaling-verifier-python')
    elif mutation in ('retained_600','retained_400','retained_missing'):
        for name in ('RELEASE_COMPLETED.json','receipt-validation-ack.json','sealed-identity.json',
            'release-retained-inventory.json','release-runner-result.json','release-runner-private-provenance.json'):put(f.evidence/name,b'fixture')
        if mutation=='retained_missing':
            for name in group:
                path=f.evidence/name
                if path.is_dir():path.rmdir()
                else:path.unlink()
        if mutation=='retained_400':
            for name in ('runner-stdout.log','runner-stderr.log'):(f.evidence/name).chmod(0o400)
    evidence=v._open_directory(f.evidence,'fixture evidence',expected_mode=0o700)
    try:
        if mutation in ('partial','extra','writable','symlink','entry','retained_missing','preflight_missing','preflight_writable','preflight_symlink'):
            with pytest.raises(v.ValidationError):v._inventory(evidence,'entry' if mutation=='entry' else 'sealed')
        else:v._inventory(evidence,'sealed')
    finally:os.close(evidence.descriptor)


@pytest.mark.parametrize('stage',('entry','sealed','retained_400','leaked_channel','wrong_channel'))
def test_full_original_producer_runner_record_is_consumed_with_actual_tools(contract,monkeypatch,stage):
    f=contract;m,v=f.m,f.v
    runner=f.candidate/'scripts/run_sumeragi_v2_release_gates.sh';put(runner,b'fixture runner')
    runner_snapshot=m._read_file(runner,'fixture runner',maximum_bytes=1024)
    tool=f.evidence/'runner-tools/rustc';put(tool,b'fixture rustc');tool.chmod(0o500)
    source=m._read_file(tool,'fixture tool',maximum_bytes=1024)
    alias=f.scope['runner_bin']/'rustc';alias.symlink_to('../runner-tools/rustc')
    alias_snapshot=m._runner_alias_snapshot(alias,tool,'fixture tool alias')
    manifest=f.evidence/'runner-tool-manifest.json';put(manifest,m._canonical_json(dict(schema_version=1,
        tools={'rustc':dict(path=str(tool),sha256=source.sha256)})))
    archives=dict(f.archives,runner_tool_manifest=m._read_file(manifest,'fixture manifest',maximum_bytes=4096))
    for name in ('runner-stdout.log','runner-stderr.log'):
        put(f.evidence/name,b'');(f.evidence/name).chmod(0o600)
    scope=dict(f.scope,runner_snapshot=runner_snapshot,runner_tool_sources={'rustc':source},
        runner_tool_archives={'rustc':source},runner_tool_aliases={'rustc':alias_snapshot},
        runner_stdout_path=f.evidence/'runner-stdout.log',runner_stderr_path=f.evidence/'runner-stderr.log',
        self_digest_variables=v._SELF_DIGEST_VARIABLES)
    tree=ast.parse((ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_bytes())
    marker=next(n.value for n in ast.walk(tree) if isinstance(n,ast.Assign) and len(n.targets)==1
        and isinstance(n.targets[0],ast.Name) and n.targets[0].id=='marker_value')
    expression=next(value for key,value in zip(marker.keys,marker.values) if isinstance(key,ast.Constant) and key.value=='runner')
    record=eval(compile(ast.Expression(expression),'<actual producer runner>','eval'),scope)
    environment=dict(f.environment)
    checkpoint='entry' if stage in ('entry','wrong_channel') else 'sealed'
    if checkpoint=='sealed':
        for name in v._SCALING_CHANNEL_KEYS:environment.pop(name)
    if stage=='leaked_channel':environment['IROHA_RELEASE_SCALING_GATE_FD']='27'
    if stage=='wrong_channel':environment['IROHA_RELEASE_SCALING_GATE_FD']='28'
    if stage=='retained_400':
        put(f.evidence/'release-runner-result.json',b'fixture protected result')
        for name in ('runner-stdout.log','runner-stderr.log'):(f.evidence/name).chmod(0o400)
    evidence=v._open_directory(f.evidence,'fixture evidence',expected_mode=0o700)
    try:
        with monkeypatch.context() as patch:
            patch.setattr(os,'environ',environment)
            if stage in ('leaked_channel','wrong_channel'):
                with pytest.raises(v.ValidationError):v._runner_contract(record,evidence,f.candidate,runner,archives,checkpoint)
            else:
                observed,closed,_,aliases,tools=v._runner_contract(record,evidence,f.candidate,runner,archives,checkpoint)
                assert observed.data==runner_snapshot.data and closed==f.environment
                assert len(aliases)==1 and tools['rustc'].sha256==source.sha256
                if checkpoint=='sealed':assert not any(name in os.environ for name in v._SCALING_CHANNEL_KEYS)
    finally:os.close(evidence.descriptor)
