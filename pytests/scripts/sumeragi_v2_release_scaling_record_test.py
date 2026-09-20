"""Canonical parent data, actual archive files, and native-output seams.

Synthetic terminal records and synthetic Norito bytes are not execution or
cryptographic qualification. The fixed native call is an explicit process seam;
actual data reduction, canonical row joining, reader paths and parser execute.
"""
from __future__ import annotations
import ast
from dataclasses import replace
import hashlib
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
from types import SimpleNamespace as NS
import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT/'scripts/nexus'),str(ROOT/'scripts'),str(ROOT/'scripts/tests'),str(ROOT/'pytests/scripts')]
import scaling_release_record as api
import scaling_cli_bootstrap as package_api
import scaling_archive_data as archive
import scaling_preflight_archive as preflight_api
from scaling_preflight_archive_fixture import build_archive
from scaling_experiment_plan import encode, plan_bytes
from resource_evidence_budget import canonical_run_budget_bytes, select_run_budget
from scaling_canonical_proof import AppliedObservation, ReplayPlan, ReplayBindings, _plan_snapshot, _bindings_snapshot, _ProjectionJoiner
from scaling_measurements import _schedule
import sumeragi_v2_release_scaling_operation_test as operation_fixture
import write_sumeragi_v2_release_receipt as receipt


def sha(raw):return hashlib.sha256(raw).hexdigest()


@pytest.fixture(autouse=True)
def no_children(monkeypatch):
    def denied(*a,**k):raise AssertionError('native/child/signal forbidden in this source/file slice')
    monkeypatch.setattr(subprocess,'Popen',denied)
    for name in ('system','fork','posix_spawn','posix_spawnp','kill','killpg'):
        if hasattr(os,name):monkeypatch.setattr(os,name,denied)


def reply(argv):return operation_fixture.Session.reply(None,argv)


def command(argv,start,end,deadline,stdout=b'',stderr=b''):
    return NS(argv=tuple(argv),environment_sha256='d'*64,started_ns=start,completed_ns=end,
        deadline_ns=deadline,returncode=0,stdout_bytes=len(stdout),stderr_bytes=len(stderr),
        stdout_sha256=sha(stdout),stderr_sha256=sha(stderr),violations=())


@pytest.fixture(scope='module')
def complete(tmp_path_factory):
    root,plan,budget,bindings,measurements=operation_fixture.completed_archive.__wrapped__(tmp_path_factory)
    checked=archive.inspect_archive(root,plan,budget,bindings)
    start=100_000;overhead=600
    deadline=start+((plan.experiment_timeout_ns+999_999_999)//1_000_000_000+overhead)*1_000_000_000
    collector=command(('/historical/python','-I','-B','-S','/historical/collector'),start+1,start+2,deadline,b'collector done')
    identity=dict(head_commit='a'*40,head_tree='b'*40,workspace_source_manifest_sha256='f'*64)
    selection=NS(plan_bytes=plan_bytes(plan),budget_bytes=canonical_run_budget_bytes(select_run_budget(budget,1,'one_lane')),
                 kagami_sha256='c'*64,source_revision='a'*40,workspace_source_sha256='f'*64)
    natives=[]
    for index,run in enumerate(checked.native_runs):
        files={row.role:row for row in run.files};request,proof=files['native_request'],files['canonical_proof']
        original=json.loads(run.canonical_receipt_json);invocation=f'{index+1:064x}'
        load_data=run.measurement;_,load,accounts,counts,_=_schedule(load_data)
        replay_plan=ReplayPlan(load.seed,load_data.plan.generator.lane_count,accounts,counts[0],counts[1],
            max(row.block_height for row in load_data.requests),tuple(AppliedObservation(row.transaction_hash,row.block_height,row.local_block_height) for row in load_data.requests))
        native_bindings=ReplayBindings(root/request.binding.path,request.binding.sha256,request.max_bytes,
            root/proof.binding.path,proof.binding.sha256,original['proof_iroha_hash'],proof.bytes,proof.max_bytes,load_data.plan.replay_reply_max_bytes)
        args=['/fixture/kagami','--ui-mode','plain','advanced','kura','scaling-evidence','replay',
            '--invocation-id',invocation,'--request',str(root/request.binding.path),'--request-sha256',request.binding.sha256,
            '--request-max-bytes',str(request.max_bytes),'--input',str(root/proof.binding.path),'--input-sha256',proof.binding.sha256,
            '--input-max-bytes',str(proof.max_bytes),'--reply-max-bytes',str(load_data.plan.replay_reply_max_bytes),'--proof-iroha-hash',original['proof_iroha_hash']]
        raw=reply(args);joiner=_ProjectionJoiner(_plan_snapshot(replay_plan),_bindings_snapshot(native_bindings),invocation)
        for offset in range(0,len(raw),65536):joiner.consume(raw[offset:offset+65536])
        count,size,digest,joined=joiner.finish()
        natives.append(NS(pair_index=run.pair_index,variant=run.variant,invocation_id=invocation,
            request_sha256=request.binding.sha256,proof_sha256=proof.binding.sha256,proof_iroha_hash=original['proof_iroha_hash'],
            proof_bytes=proof.bytes,row_count=count,reply_bytes=size,reply_sha256=digest,joined_rows_sha256=joined,
            command=command(args,start+3+index*2,start+4+index*2,deadline,raw)))
    artifacts=tuple(NS(relative_path=name,sha256=sha((root/name).read_bytes()),size_bytes=(root/name).stat().st_size,mode=0o600) for name in ('manifest.json','report.json'))
    observation=NS(command=collector,artifacts=artifacts,invocation_sha256='e'*64,launch_input_sha256='f'*64,original_started_ns=start)
    verification=NS(collector=collector,artifacts=artifacts,invocation_sha256='e'*64,launch_input_sha256='f'*64,
        plan_sha256=sha(selection.plan_bytes),budget_sha256=sha(selection.budget_bytes),kagami_sha256='c'*64,
        inventory=checked.inventory,archive_checks=checked,native_replays=tuple(natives),original_deadline_ns=deadline,verification_completed_ns=start+24)
    verifier_root=tmp_path_factory.mktemp('record-verifier');verifier_root.chmod(0o700)
    package_api.stage_dependency_source(Path(__import__('blake3').__file__).parent.parent,verifier_root/'source')
    package=package_api.PythonDependencies.provision(verifier_root/'source',verifier_root/'bundle',verifier_root/'inventory.json')
    try:verifier=dict(inventory_sha256=package.paths.inventory_sha256,files=list(package_api.dependency_package_census(verifier_root/'source')))
    finally:package.close()
    preflight_root=tmp_path_factory.mktemp('record-preflight')/'scaling-preflight'
    preflight=build_archive(ROOT,preflight_root,identity)
    raw=api.encode_parent_execution(identity,selection,observation,verification,
        observation_overhead_seconds=overhead,verifier_python=verifier,preflight=preflight)
    return NS(root=root,plan=plan,budget=budget,checked=checked,identity=identity,selection=selection,
        observation=observation,verification=verification,raw=raw,record=api.decode_parent_execution(raw),verifier=verifier,verifier_root=verifier_root,preflight=preflight,preflight_root=preflight_root)


def test_canonical_parent_projection_and_full_data_join(complete):
    c=complete
    assert api.inspect_parent_archive(c.root,c.record,c.identity)==c.checked
    assert c.record.inventory==archive.inventory_archive(c.root,c.plan,c.budget)
    assert len(c.record.document['execution']['native_replays'])==10
    assert api.receipt_projection(c.record)['parent_execution']['sha256']==sha(c.raw)
    assert not any(hasattr(c.record,name) for name in ('passed','accepted','authenticated','qualified'))
    value=c.record.document;value['execution']['command']['returncode']=12
    assert c.record.document['execution']['command']['returncode']==0
    assert b'/historical/' not in c.raw and b'/fixture/' not in c.raw
    assert api.decode_parent_execution(c.raw).canonical==c.raw


@pytest.mark.parametrize('mutation',[ 'unknown','duplicate','schema','float','bool','foreign','too_large','depth','trailing','negative_time','late','negative_exit','violations','budget_variant','budget_unknown','plan_version','candidate_width','inventory_files','inventory_bytes','duplicate_run','wrong_order','duplicate_nonce','zero_nonce','native_hash','native_stream','native_bound','native_late','verification_early'])
def test_exact_parent_schema_rejects_tampering(complete,mutation):
    value=complete.record.document;ex=value['execution'];row=ex['native_replays'][0]
    if mutation=='unknown':value['accepted']=True
    elif mutation=='duplicate':
        with pytest.raises(ValueError):api.decode_parent_execution(complete.raw.replace(b'{',b'{"schema":"foreign",',1))
        return
    elif mutation=='schema':value['schema']='retired'
    elif mutation=='float':ex['command']['returncode']=0.0
    elif mutation=='bool':ex['command']['returncode']=False
    elif mutation=='foreign':value['inputs']['plan']=[]
    elif mutation=='too_large':
        with pytest.raises(ValueError):api.decode_parent_execution(b' '* (api.MAX_RECORD_BYTES+1))
        return
    elif mutation=='depth':
        with pytest.raises(ValueError):api.decode_parent_execution(b'['*65+b'0'+b']'*65)
        return
    elif mutation=='trailing':
        with pytest.raises(ValueError):api.decode_parent_execution(complete.raw+b'\n')
        return
    elif mutation=='negative_time':ex['original_started_ns']=-1
    elif mutation=='late':ex['command']['completed_ns']=ex['deadline_ns']+1
    elif mutation=='negative_exit':ex['command']['returncode']=-9
    elif mutation=='violations':ex['command']['violations']=['timeout']
    elif mutation=='budget_variant':value['inputs']['budget']['variant']='four_lane'
    elif mutation=='budget_unknown':value['inputs']['budget']['accepted']=True
    elif mutation=='plan_version':value['inputs']['plan']['schema']='old'
    elif mutation=='candidate_width':value['candidate']['head_tree']='a'*64
    elif mutation=='inventory_files':value['publication']['inventory']['files']-=1
    elif mutation=='inventory_bytes':value['publication']['inventory']['bytes']=complete.budget.total_bytes+1
    elif mutation=='duplicate_run':ex['native_replays'][1]=row
    elif mutation=='wrong_order':ex['native_replays'].reverse()
    elif mutation=='duplicate_nonce':ex['native_replays'][1]['invocation_id']=row['invocation_id']
    elif mutation=='zero_nonce':row['invocation_id']='0'*64
    elif mutation=='native_hash':row['request_sha256']='bad'
    elif mutation=='native_stream':row['command']['stdout']['bytes']+=1
    elif mutation=='native_bound':row['command']['stderr']['bytes']=complete.plan.trials[0].replay_reply_max_bytes
    elif mutation=='native_late':row['command']['completed_ns']=ex['deadline_ns']+1
    elif mutation=='verification_early':ex['verification_completed_ns']=ex['original_started_ns']
    with pytest.raises(ValueError):api.decode_parent_execution(encode(value,api.MAX_RECORD_BYTES))


@pytest.mark.parametrize('field',['head_commit','head_tree','workspace_source_manifest_sha256'])
def test_archive_source_join_rejects_other_selected_candidate(complete,field):
    identity=complete.identity|{field:'0'*len(complete.identity[field])}
    with pytest.raises(ValueError):api.inspect_parent_archive(complete.root,complete.record,identity)


def test_encoder_preserves_readable_original_policy_and_checks_original_object(complete):
    c=complete;selection=NS(**vars(c.selection));selection.plan_bytes=json.dumps(json.loads(selection.plan_bytes),indent=2).encode()
    selection.budget_bytes=json.dumps(json.loads(selection.budget_bytes),indent=2).encode()
    verified=NS(**vars(c.verification));verified.plan_sha256=sha(selection.plan_bytes);verified.budget_sha256=sha(selection.budget_bytes)
    assert api.encode_parent_execution(c.identity,selection,c.observation,verified,observation_overhead_seconds=600,verifier_python=c.verifier,preflight=c.preflight)==c.raw
    verified.collector=NS(**vars(c.observation.command))
    with pytest.raises(ValueError):api.encode_parent_execution(c.identity,selection,c.observation,verified,observation_overhead_seconds=600,verifier_python=c.verifier,preflight=c.preflight)


@pytest.fixture
def copied(complete,tmp_path):
    invocation=tmp_path/'invocation';invocation.mkdir(mode=0o700)
    source=invocation/'source';source.mkdir(mode=0o700)
    inventory=json.loads((ROOT/preflight_api.INVENTORY_PATH).read_bytes())
    for name in (*inventory['sources'],preflight_api.INVENTORY_PATH):
        path=source/name;path.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(ROOT/name,path)
    source.chmod(0o500)
    output=invocation/'output';output.mkdir(mode=0o700)
    root=output/'scaling';shutil.copytree(complete.root,root)
    bootstrap=tmp_path/'bootstrap';bootstrap.mkdir(mode=0o700)
    execution=bootstrap/'scaling-execution.json';execution.write_bytes(complete.raw);execution.chmod(0o400)
    shutil.copytree(complete.verifier_root,bootstrap/'scaling-verifier-python')
    shutil.copytree(complete.preflight_root,bootstrap/'scaling-preflight')
    for name,data in [('plan',complete.selection.plan_bytes),('budget',complete.selection.budget_bytes)]:
        path=bootstrap/('scaling-'+name+'.json');path.write_bytes(data);path.chmod(0o400)
    return NS(root=root,source=source,bootstrap=bootstrap,execution=execution)


def test_portable_archive_changes_inodes_and_keeps_canonical_census(complete,copied):
    assert (copied.root/'manifest.json').stat().st_ino!=(complete.root/'manifest.json').stat().st_ino
    for path in copied.root.rglob('*'):
        if path.is_file():path.chmod(0o400)
    assert api.inspect_parent_archive(copied.root,complete.record,complete.identity)==complete.checked
    files,directories=api.capture_public_archive(copied.root,complete.record)
    assert len(files)==complete.record.inventory.files==2225
    assert all(row['mode']=='0400' for row in files)
    assert len(directories)>10


@pytest.mark.parametrize('mutation',['changed_bytes','extra','missing','symlink','wrong_mode'])
def test_exact_archive_census_rejects_copy_mutations(complete,copied,mutation):
    path=copied.root/'manifest.json'
    if mutation=='changed_bytes':path.write_bytes(path.read_bytes()+b' ')
    elif mutation=='extra':(copied.root/'extra').write_bytes(b'private')
    elif mutation=='missing':path.unlink()
    elif mutation=='symlink':path.unlink();path.symlink_to(complete.root/'manifest.json')
    else:path.chmod(0o644)
    with pytest.raises(ValueError):api.capture_public_archive(copied.root,complete.record)


def test_actual_receipt_record_path_digest_and_metadata(complete,copied):
    record,snapshot=receipt._scaling_execution_record(api,copied.execution,sha(complete.raw),copied.bootstrap)
    assert record==complete.record and snapshot.sha256==record.sha256
    for path,digest in [(copied.execution,'0'*64),(copied.bootstrap/'foreign.json',sha(complete.raw))]:
        with pytest.raises(receipt.ReceiptError):receipt._scaling_execution_record(api,path,digest,copied.bootstrap)
    copied.execution.chmod(0o600)
    with pytest.raises(receipt.ReceiptError):receipt._scaling_execution_record(api,copied.execution,sha(complete.raw),copied.bootstrap)


def native_seam(monkeypatch,complete,copied,mutation=None):
    binary_dir=copied.root.parent/'programs';(binary_dir/'release').mkdir(parents=True)
    path=binary_dir/'release/kagami';path.write_bytes(b'inert native input');path.chmod(0o500)
    actual=receipt._bounded_path_contract
    def capture(selected,*a,**k):
        value=actual(selected,*a,**k)
        return replace(value,sha256='c'*64) if selected==path else value
    monkeypatch.setattr(receipt,'_bounded_path_contract',capture)
    monkeypatch.setattr(receipt,'_scaling_record_support',lambda root,**kwargs:api)
    calls=[]
    def run(executable,arguments,**options):
        assert executable==path and options['executable_contract'].sha256=='c'*64
        assert len(options['watched_contracts'])==2
        assert options['maximum_output_bytes']>0
        calls.append(arguments);raw=reply([str(path),*arguments]);status=0
        if mutation=='status':status=23
        elif mutation=='row':
            value=json.loads(raw);value['rows'][0]['carrier_height']+=1
            rows=value.pop('rows');raw=json.dumps(value,separators=(',',':')).encode()[:-1]+b',"rows":'+json.dumps(rows,separators=(',',':')).encode()+b'}\n'
        elif mutation=='trailing':raw+=b'\n'
        elif mutation=='late_archive':(copied.root/'manifest.json').write_bytes(b'changed')
        elif mutation=='late_preflight':
            changed=copied.bootstrap/'scaling-preflight/unit-000.stdout'
            changed.chmod(0o600);changed.write_bytes(b'changed');changed.chmod(0o400)
        return status,raw,b''
    monkeypatch.setattr(receipt,'_run_bounded_replay',run)
    args=dict(execution_record_path=copied.execution,expected_execution_sha256=complete.record.sha256,
        bootstrap_evidence=copied.bootstrap,sealed=complete.identity,repo_root=copied.source,checker_environment={},
        prebuilt_bundle={'binaries':[dict(role='kagami',relative_path='release/kagami',sha256='c'*64,size_bytes=path.stat().st_size,mode='0500')]},
        prebuilt_bundle_dir=binary_dir,bootstrap_authentication={"trusted_input_digests":{"scaling_plan":sha(complete.selection.plan_bytes),"scaling_budget":sha(complete.selection.budget_bytes)},"runner":{"scaling_preflight_timeout_seconds":600,"scaling_handoff":{"IROHA_RELEASE_SCALING_INVOCATION_SHA256":"e"*64}}})
    return calls,args


def test_actual_receipt_native_adapter_requires_all_ten_full_replies(monkeypatch,complete,copied):
    calls,args=native_seam(monkeypatch,complete,copied)
    assert receipt._validate_fixed_scaling_archive(**args)==api.receipt_projection(complete.record)
    assert len(calls)==10
    assert all(tuple(row[:6])==('--ui-mode','plain','advanced','kura','scaling-evidence','replay') for row in calls)
    assert len({row[row.index('--invocation-id')+1] for row in calls})==10


@pytest.mark.parametrize('mutation',['status','row','trailing','late_archive','late_preflight'])
def test_actual_receipt_rejects_native_or_post_replay_failure(monkeypatch,complete,copied,mutation):
    _,args=native_seam(monkeypatch,complete,copied,mutation)
    with pytest.raises(receipt.ReceiptError):receipt._validate_fixed_scaling_archive(**args)


def test_actual_source_loader_rejects_foreign_cached_module(monkeypatch, copied):
    monkeypatch.setitem(sys.modules,'scaling_release_record',NS(__file__='/foreign/source.py'))
    with pytest.raises(receipt.ReceiptError):
        receipt._scaling_record_support(ROOT, execution_record_path=copied.execution,
            expected_execution_sha256='0'*64,bootstrap_evidence=copied.bootstrap)


def selected_bootstrap(name):
    path=ROOT/'scripts/bootstrap_sumeragi_v2_release_receipt_replay.py'
    tree=ast.parse(path.read_text());node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name==name)
    module=ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0),node],type_ignores=[])
    scope=dict(Path=Path,os=os,BootstrapError=ValueError)
    exec(compile(ast.fix_missing_locations(module),str(path),'exec'),scope)
    return scope


def test_actual_bootstrap_terminal_scaling_join(complete,copied):
    scope=selected_bootstrap('_validate_terminal_fixed_scaling')
    snapshot=NS(path=copied.execution,data=complete.raw,sha256=complete.record.sha256,mode=0o400,nlink=1,owner=os.getuid())
    def unchanged(value,*a,**k):assert value.path.read_bytes()==value.data
    scope['_require_unchanged']=unchanged
    scope['_terminal_relative_path']=lambda value,label:tuple(Path(value).parts)
    files=[];directories=[]
    def capture(row,label,**options):
        path=options['expected_path'];assert sha(path.read_bytes())==row['sha256']
        assert path.stat().st_size==row['size_bytes']<=options['maximum_bytes'];files.append(path)
    def directory(path,label,**options):assert path.is_dir();directories.append(path)
    identity=dict(head_commit='a'*40,head_tree='b'*40,sealed_source_manifest_sha256='f'*64)
    args=dict(receipt_evidence={'multilane_scaling':api.receipt_projection(complete.record)},release_root=copied.source,
        receipt_identity=identity,runner_record=dict(scaling_preflight_timeout_seconds=600,
            scaling_handoff={'IROHA_RELEASE_SCALING_INVOCATION_SHA256':'e'*64}),scaling_execution=snapshot,scaling_record_api=api,capture_archive=capture,capture_directory=directory)
    scope['_validate_terminal_fixed_scaling'](**args)
    assert len(files)==2243+complete.preflight['inventory']['files'] and copied.execution in files
    assert copied.bootstrap/'scaling-preflight/index.json' in files
    args['receipt_evidence']['multilane_scaling']['parent_execution']['sha256']='0'*64
    with pytest.raises(ValueError):scope['_validate_terminal_fixed_scaling'](**args)


def test_actual_protected_replay_argv_and_new_order():
    tree=ast.parse((ROOT/'scripts/bootstrap_sumeragi_v2_release_receipt_replay.py').read_text())
    function=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_run_protected_receipt_validator')
    assignment=next(n for n in ast.walk(function) if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='arguments' for t in n.targets))
    class AnyValue:
        def __iter__(self):return iter(())
        def __getitem__(self,key):return self
        def __getattr__(self,key):return self
        def __call__(self,*a,**k):return self
        def __truediv__(self,key):return self
        def __str__(self):return '/fixture'
    names={n.id for n in ast.walk(assignment) if isinstance(n,ast.Name) and isinstance(n.ctx,ast.Load)}
    scope={name:AnyValue() for name in names};scope.update(str=str,
        scaling_execution=NS(path=Path('/protected/scaling-execution.json'),sha256='f'*64))
    exec(compile(ast.fix_missing_locations(ast.Module(body=[assignment],type_ignores=[])),'<actual protected replay argv>','exec'),scope)
    argv=scope['arguments'];assert argv[:4]==['-I','-B','-S','/fixture']
    index=argv.index('--scaling-execution-record')
    assert argv[index-2]=='--g12-fault-soak-completion'
    assert argv[index:index+4]==['--scaling-execution-record','/protected/scaling-execution.json','--expected-scaling-execution-sha256','f'*64]
    assert not any('scaling-evidence' in item or 'scaling-trial' in item or 'scaling-configuration' in item or 'scaling-iroha' in item for item in argv)
    assert '--scaling-execution-record' in receipt._RECEIPT_VALIDATION_PATH_OPTIONS
    assert '--expected-scaling-execution-sha256' not in receipt._RECEIPT_VALIDATION_PATH_OPTIONS


def test_retirement_and_component_pins_are_coherent():
    paths=[ROOT/'scripts'/name for name in ('write_sumeragi_v2_release_receipt.py',
        'write_sumeragi_v2_release_receipt_gate_evidence.py','write_sumeragi_v2_release_receipt_publication.py',
        'write_sumeragi_v2_release_receipt_corridor_log.py','bootstrap_sumeragi_v2_release_receipt_replay.py')]
    for path in paths:
        source=path.read_text();ast.parse(source)
        for retired in ('scaling_evidence.json','validate_multilane_scaling_evidence.py','--scaling-evidence-manifest','--expected-scaling-trial-harness-sha256'):
            assert retired not in source
    for filename,digest in receipt._RELEASE_RECEIPT_COMPONENT_SHA256.items():
        assert sha((ROOT/'scripts'/filename).read_bytes())==digest

@pytest.mark.parametrize('mutation',['plan','budget','invocation','preflight_timeout'])
def test_original_selected_inputs_and_handoff_are_required(monkeypatch,complete,copied,mutation):
    calls,args=native_seam(monkeypatch,complete,copied)
    if mutation in ('plan','budget'):
        path=copied.bootstrap/('scaling-'+mutation+'.json');path.chmod(0o600);path.write_bytes(b'{}');path.chmod(0o400)
    elif mutation=='preflight_timeout':args['bootstrap_authentication']['runner']['scaling_preflight_timeout_seconds']=601
    else:args['bootstrap_authentication']['runner']['scaling_handoff']['IROHA_RELEASE_SCALING_INVOCATION_SHA256']='0'*64
    with pytest.raises(receipt.ReceiptError):receipt._validate_fixed_scaling_archive(**args)
    assert calls==[]


def final_marker(complete):
    return dict(schema_version=2,result='release-complete',bootstrap_completion_sha256='1'*64,
        candidate_identity_sha256='2'*64,candidate_commit_oid=complete.identity['head_commit'],
        candidate_tree_oid=complete.identity['head_tree'],release_approvals={},runner={},
        retained_source={'source_manifest_sha256':complete.identity['workspace_source_manifest_sha256']},
        receipt_validator={'exit_status':0},terminal_receipt={'sha256':'3'*64},
        scaling_execution=api.receipt_projection(complete.record))


def test_externally_authenticated_final_marker_binds_exact_execution(complete):
    raw=encode(final_marker(complete),package_api.MAX_FINAL_MARKER_BYTES)
    assert package_api.final_execution_record_digest(raw,sha(raw))==complete.record.sha256
    api.verify_final_execution_join(raw,sha(raw),complete.record,'3'*64)
    with pytest.raises(ValueError):package_api.final_execution_record_digest(raw,'0'*64)
    with pytest.raises(ValueError):api.verify_final_execution_join(raw,sha(raw),complete.record,'4'*64)


@pytest.mark.parametrize('mutation',['extra','schema','result','record','commit','tree','workspace','publication','validator','terminal'])
def test_final_marker_tamper_cannot_rebind_execution(complete,mutation):
    value=final_marker(complete)
    if mutation=='extra':value['accepted']=True
    elif mutation=='schema':value['schema_version']=True
    elif mutation=='result':value['result']='pending'
    elif mutation=='record':value['scaling_execution']['parent_execution']['sha256']='0'*64
    elif mutation=='commit':value['candidate_commit_oid']='0'*40
    elif mutation=='tree':value['candidate_tree_oid']='0'*40
    elif mutation=='workspace':value['retained_source']['source_manifest_sha256']='0'*64
    elif mutation=='publication':value['scaling_execution']['publication']['report_sha256']='0'*64
    elif mutation=='validator':value['receipt_validator']['exit_status']=False
    else:value['terminal_receipt']['sha256']='0'*64
    raw=encode(value,package_api.MAX_FINAL_MARKER_BYTES)
    with pytest.raises(ValueError):api.verify_final_execution_join(raw,sha(raw),complete.record,'3'*64)


def test_exact_localnet_source_buffer_ignores_poisoned_bytecode(monkeypatch,tmp_path):
    import importlib.util,marshal,types
    source=tmp_path/'localnet.py';raw=b'VALUE = "original source"\n';source.write_bytes(raw)
    monkeypatch.setattr(receipt,'_LOCALNET_MANIFEST_MODULE_PATH',source)
    monkeypatch.setattr(receipt,'_LOCALNET_MANIFEST_SOURCE_SHA256',sha(raw))
    monkeypatch.setattr(sys,'pycache_prefix',str(tmp_path/'foreign-cache'))
    cache=Path(importlib.util.cache_from_source(str(source)));cache.parent.mkdir(parents=True)
    poison=compile('VALUE = "poisoned cache"',str(source),'exec')
    cache.write_bytes(importlib.util.MAGIC_NUMBER+(1).to_bytes(4,'little')+b'0'*8+marshal.dumps(poison))
    spec=importlib.util.spec_from_file_location('cache_control',source)
    control=importlib.util.module_from_spec(spec);spec.loader.exec_module(control)
    assert control.VALUE=='poisoned cache'
    actual=receipt._load_localnet_manifest()
    assert actual.VALUE=='original source' and actual.__file__==str(source)
    assert actual.__spec__.origin==str(source)


@pytest.mark.parametrize('mutation',['digest','link','mode','oversize'])
def test_exact_localnet_loader_rejects_unsafe_source(monkeypatch,tmp_path,mutation):
    source=tmp_path/'localnet.py';raw=b'VALUE = 1\n';source.write_bytes(raw)
    monkeypatch.setattr(receipt,'_LOCALNET_MANIFEST_MODULE_PATH',source)
    monkeypatch.setattr(receipt,'_LOCALNET_MANIFEST_SOURCE_SHA256',sha(raw))
    if mutation=='digest':source.write_bytes(b'VALUE = 2\n')
    elif mutation=='link':os.link(source,tmp_path/'alias.py')
    elif mutation=='mode':source.chmod(0o666)
    else:source.write_bytes(b' '* (1024*1024+1))
    with pytest.raises(RuntimeError):receipt._load_localnet_manifest()


@pytest.mark.parametrize('mutation',['missing','foreign','unknown','bool_timeout','renewed_deadline','after_collector','late','index_mode','index_digest','census'])
def test_mandatory_preflight_binding_cannot_be_rebound(complete,mutation):
    value=complete.record.document
    preflight=value['preflight']
    if mutation=='missing':del value['preflight']
    elif mutation=='foreign':preflight['archive_id']='foreign'
    elif mutation=='unknown':preflight['accepted']=True
    elif mutation=='bool_timeout':preflight['scope']['timeout_seconds']=True
    elif mutation=='renewed_deadline':preflight['scope']['deadline_ns']+=1
    elif mutation=='after_collector':preflight['scope']['completed_ns']=value['execution']['original_started_ns']+1
    elif mutation=='late':preflight['scope']['completed_ns']=preflight['scope']['deadline_ns']+1
    elif mutation=='index_mode':preflight['index']['mode']='0600'
    elif mutation=='index_digest':preflight['index']['sha256']='invalid'
    else:preflight['inventory']['files']=0
    with pytest.raises(ValueError):api.decode_parent_execution(encode(value,api.MAX_RECORD_BYTES))


def test_preflight_projection_is_owned_and_required_for_framing(complete):
    projection=api.receipt_projection(complete.record)
    assert projection['preflight']==complete.preflight
    projection['preflight']['scope']['timeout_seconds']=601
    assert complete.record.document['preflight']['scope']['timeout_seconds']==600
    value=complete.record.document;del value['preflight'];raw=encode(value,api.MAX_RECORD_BYTES)
    with pytest.raises(ValueError):package_api.parent_execution_dependency_binding(raw,sha(raw))
    marker=final_marker(complete);del marker['scaling_execution']['preflight']
    raw=encode(marker,package_api.MAX_FINAL_MARKER_BYTES)
    with pytest.raises(ValueError):package_api.final_execution_record_digest(raw,sha(raw))


def preflight_context(complete,copied):
    return dict(source_root=copied.source,candidate_identity=complete.identity,
        invocation_sha256='e'*64,timeout_seconds=600)


def test_preflight_archive_survives_runtime_pruning_and_relocation(complete,copied):
    context=preflight_context(complete,copied)
    runtime=copied.source.parent/'runtime';runtime.mkdir(mode=0o700)
    (runtime/'temporary-results').write_bytes(b'pruned scratch')
    root=copied.bootstrap/'scaling-preflight'
    before=api.inspect_preflight_archive(root,complete.record,**context)
    shutil.rmtree(runtime)
    assert api.inspect_preflight_archive(root,complete.record,**context)==before
    files,directories=api.capture_preflight_archive(root,complete.record,**context)
    assert len(files)==complete.preflight['inventory']['files']
    assert len(directories)==1 and directories[0]['relative_path']==''
    assert (root/'index.json').stat().st_ino!=(complete.preflight_root/'index.json').stat().st_ino
    assert not hasattr(before,'accepted') and not hasattr(before,'command_owner')


@pytest.mark.parametrize('mutation',['source','candidate','invocation','timeout','missing','extra','stream'])
def test_preflight_archive_requires_selected_context_and_complete_bytes(complete,copied,mutation):
    context=preflight_context(complete,copied);root=copied.bootstrap/'scaling-preflight'
    if mutation=='source':
        path=copied.source/preflight_api.INVENTORY_PATH;path.chmod(0o600);path.write_bytes(b'{}')
    elif mutation=='candidate':context['candidate_identity']=dict(complete.identity,head_tree='0'*40)
    elif mutation=='invocation':context['invocation_sha256']='0'*64
    elif mutation=='timeout':context['timeout_seconds']=601
    elif mutation=='missing':(root/'unit-000.result.json').unlink()
    elif mutation=='extra':(root/'unexpected').write_bytes(b'extra')
    else:
        path=root/'unit-000.stdout';path.chmod(0o600);path.write_bytes(b'changed');path.chmod(0o400)
    for check in (api.inspect_preflight_archive,api.capture_preflight_archive):
        with pytest.raises(ValueError):check(root,complete.record,**context)


@pytest.mark.parametrize('mutation',['missing','changed'])
def test_final_marker_preflight_projection_is_mandatory_and_exact(complete,mutation):
    value=final_marker(complete)
    if mutation=='missing':del value['scaling_execution']['preflight']
    else:value['scaling_execution']['preflight']['inventory']['sha256']='0'*64
    raw=encode(value,package_api.MAX_FINAL_MARKER_BYTES)
    with pytest.raises(ValueError):api.verify_final_execution_join(raw,sha(raw),complete.record,'3'*64)


def _selected_bounded_capture_scope(tmp_path):
    """Execute current capture owners without importing/bootstraping a runner."""
    from dataclasses import dataclass
    import re
    selections = {
        'bootstrap_sumeragi_v2_release.py': {
            'LargeFileSnapshot', 'DirectorySnapshot', '_absolute_resolved_existing',
            '_capture_descriptor_pin', '_require_capture_descriptor',
            '_close_capture_descriptor', '_open_capture_descriptor',
            '_inside', '_capture_large_file_at', '_capture_bounded_large_file',
            '_require_large_file_unchanged', '_terminal_directory_snapshot',
            '_terminal_mode', '_require_exact_json_fields', '_scaling_require'},
        'bootstrap_sumeragi_v2_release_receipt_replay.py': {'capture_archive', 'capture_directory'},
        'write_sumeragi_v2_release_receipt.py': {'PathContract', '_require_digest'},
        'write_sumeragi_v2_release_receipt_publication.py': {'_capture_path_contract'},
    }
    nodes = [ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)]
    for name, expected in selections.items():
        tree = ast.parse((ROOT / 'scripts' / name).read_bytes())
        selected = [node for node in ast.walk(tree)
                    if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name in expected]
        assert sorted(node.name for node in selected) == sorted(expected)
        nodes.extend(selected)
    scope = dict(__name__=__name__, Path=Path, os=os, stat=stat, hashlib=hashlib,
        dataclass=dataclass, re=re, fcntl=__import__('fcntl'), BootstrapError=ValueError, ReceiptError=ValueError,
        _DIGEST_RE=re.compile(r'[a-f0-9]{64}'), evidence=tmp_path,
        _MAX_TERMINAL_ARTIFACT_BYTES=8, artifact_paths={}, artifact_inodes={},
        artifact_snapshots=[], directories={}, directory_inodes={}, directory_snapshots=[])
    exec(compile(ast.fix_missing_locations(ast.Module(body=nodes, type_ignores=[])),
                 '<captured-current-bounded-capture-owners>', 'exec'), scope)
    return scope


def _capture_file(tmp_path, raw=b'12345678'):
    parent = tmp_path / 'scaling-preflight'
    parent.mkdir(mode=0o700)
    path = parent / 'unit-000.stdout'
    path.write_bytes(raw)
    path.chmod(0o400)
    return path


def _append_capture_byte(path):
    path.chmod(0o600)
    with path.open('ab') as stream:
        stream.write(b'9')
    path.chmod(0o400)


def _capture_read_trace(monkeypatch, before_first_read=None):
    original = os.read
    sizes = []
    def read(descriptor, count):
        if not sizes and before_first_read is not None:
            before_first_read()
        raw = original(descriptor, count)
        sizes.append(len(raw))
        return raw
    monkeypatch.setattr(os, 'read', read)
    return sizes


@pytest.mark.parametrize('owner', ['terminal', 'original_recheck'])
def test_capture_large_growth_reads_only_the_declared_bound_plus_one(tmp_path, monkeypatch, owner):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    original = scope['_capture_bounded_large_file'](path, 'original stdout', maximum_bytes=8)
    def grow():
        path.chmod(0o600)
        with path.open('ab') as stream: stream.write(b'x' * 8192)
        path.chmod(0o400)
    sizes = _capture_read_trace(monkeypatch, grow)
    with pytest.raises(ValueError):
        if owner == 'original_recheck':
            scope['_require_large_file_unchanged'](original, 'final stdout')
        else:
            record = dict(archive_id='release-scaling.preflight-file.v1:unit-000.stdout',
                          sha256=sha(b'12345678'), size_bytes=8, mode='0400')
            scope['capture_archive'](record, 'terminal stdout', archive_id=record['archive_id'],
                expected_path=path, maximum_bytes=8, expected_mode=0o400,
                containment_root=path.parent)
    assert sizes == [9]
    assert scope['artifact_snapshots'] == []


@pytest.mark.parametrize('mutation', ['unchanged', 'oversized_before_read', 'growth_during_read'])
def test_terminal_preflight_capture_preserves_exact_byte_bound(tmp_path, monkeypatch, mutation):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    record = dict(archive_id='release-scaling.preflight-file.v1:unit-000.stdout',
                  sha256=sha(b'12345678'), size_bytes=8, mode='0400')
    if mutation == 'oversized_before_read':
        _append_capture_byte(path)
    sizes = _capture_read_trace(monkeypatch,
        (lambda: _append_capture_byte(path)) if mutation == 'growth_during_read' else None)
    def capture():
        return scope['capture_archive'](record, 'terminal preflight stdout',
            archive_id=record['archive_id'], expected_path=path, maximum_bytes=8,
            expected_mode=0o400, containment_root=path.parent)
    if mutation == 'unchanged':
        assert capture().sha256 == record['sha256']
        assert sum(sizes) == 8
        assert len(scope['artifact_snapshots']) == 1
    else:
        with pytest.raises(ValueError):
            capture()
        assert sum(sizes) == (0 if mutation == 'oversized_before_read' else 9)
        assert scope['artifact_snapshots'] == []


@pytest.mark.parametrize('mutation', ['unchanged', 'oversized_before_read', 'growth_during_read'])
def test_receipt_preflight_capture_preserves_exact_byte_bound(tmp_path, monkeypatch, mutation):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    if mutation == 'oversized_before_read':
        _append_capture_byte(path)
    sizes = _capture_read_trace(monkeypatch,
        (lambda: _append_capture_byte(path)) if mutation == 'growth_during_read' else None)
    def capture():
        return scope['_capture_path_contract'](path, 'receipt preflight stdout',
            expected_sha256=sha(b'12345678'), expected_mode=0o400,
            expected_owner=os.getuid(), expected_nlink=1, expected_size=8)
    if mutation == 'unchanged':
        assert capture().size == 8
        assert sum(sizes) == 8
    else:
        with pytest.raises(ValueError):
            capture()
        assert sum(sizes) == (0 if mutation == 'oversized_before_read' else 9)


@pytest.mark.parametrize('moment', ['before_parent_open', 'during_read'])
def test_terminal_preflight_capture_rejects_parent_replacement(tmp_path, monkeypatch, moment):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    original_parent = path.parent
    def replace_parent():
        original_parent.rename(tmp_path / 'displaced-preflight')
        original_parent.mkdir(mode=0o700)
        path.write_bytes(b'12345678')
        path.chmod(0o400)
    if moment == 'before_parent_open':
        # Pin the exact original directory through the production owner, then
        # substitute it before the terminal artifact owner opens its parent.
        scope['capture_directory'](original_parent, 'preflight root', containment_root=original_parent)
        replace_parent()
    sizes = _capture_read_trace(monkeypatch, replace_parent if moment == 'during_read' else None)
    record = dict(archive_id='release-scaling.preflight-file.v1:unit-000.stdout',
                  sha256=sha(b'12345678'), size_bytes=8, mode='0400')
    with pytest.raises(ValueError):
        scope['capture_archive'](record, 'terminal preflight stdout',
            archive_id=record['archive_id'], expected_path=path, maximum_bytes=8,
            expected_mode=0o400, containment_root=original_parent)
    assert sum(sizes) == (0 if moment == 'before_parent_open' else 8)
    assert scope['artifact_snapshots'] == []


@pytest.mark.parametrize('mutation', ['unchanged', 'oversized_before_read', 'growth_during_read'])
def test_original_preflight_snapshot_final_recheck_preserves_size(tmp_path, monkeypatch, mutation):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    original = scope['_capture_bounded_large_file'](path, 'original preflight stdout', maximum_bytes=8)
    if mutation == 'oversized_before_read':
        _append_capture_byte(path)
    sizes = _capture_read_trace(monkeypatch,
        (lambda: _append_capture_byte(path)) if mutation == 'growth_during_read' else None)
    if mutation == 'unchanged':
        scope['_require_large_file_unchanged'](original, 'final preflight stdout')
        assert sum(sizes) == 8
    else:
        with pytest.raises(ValueError):
            scope['_require_large_file_unchanged'](original, 'final preflight stdout')
        assert sum(sizes) == (0 if mutation == 'oversized_before_read' else 9)


@pytest.mark.parametrize('owner', ['terminal_parent', 'bounded_parent', 'file'])
@pytest.mark.parametrize('drift', ['foreign', 'inheritable', 'nonblocking'])
def test_capture_read_error_preserves_reused_descriptor(tmp_path, monkeypatch, owner, drift):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    foreign_dir = tmp_path / 'foreign'; foreign_dir.mkdir(mode=0o700)
    foreign_file = foreign_dir / path.name; foreign_file.write_bytes(b'foreign')
    replaced = []
    original_at = scope['_capture_large_file_at']
    parent_slot = []
    def at(fd, *args, **kwargs):
        parent_slot[:] = [fd]
        return original_at(fd, *args, **kwargs)
    scope['_capture_large_file_at'] = at
    original_read = os.read
    def failed(fd, count):
        slot = fd if owner == 'file' else parent_slot[0]
        assert not replaced
        if drift == 'foreign':
            other = os.open(foreign_file if owner == 'file' else foreign_dir,
                os.O_RDONLY | os.O_CLOEXEC | (0 if owner == 'file' else os.O_DIRECTORY))
            os.dup2(other, slot, inheritable=False); os.close(other)
        elif drift == 'inheritable': os.set_inheritable(slot, True)
        else: fcntl.fcntl(slot, fcntl.F_SETFL, fcntl.fcntl(slot, fcntl.F_GETFL) | os.O_NONBLOCK)
        replaced.append(slot)
        raise OSError('read-error reproduction')
    monkeypatch.setattr(os, 'read', failed)
    borrowed = None
    try:
        with pytest.raises((ValueError, OSError)):
            if owner == 'terminal_parent':
                record = dict(archive_id='release-scaling.preflight-file.v1:unit-000.stdout',
                    sha256=sha(b'12345678'), size_bytes=8, mode='0400')
                scope['capture_archive'](record, 'preflight', archive_id=record['archive_id'],
                    expected_path=path, maximum_bytes=8, expected_mode=0o400, containment_root=path.parent)
            elif owner == 'bounded_parent':
                scope['_capture_bounded_large_file'](path, 'preflight', maximum_bytes=8)
            else:
                borrowed = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
                scope['_capture_large_file_at'](borrowed, path.name, path, 'preflight', maximum_bytes=8)
        assert len(replaced) == 1
        try: current = os.fstat(replaced[0])
        except OSError: pytest.fail('cleanup closed the foreign or drifted descriptor slot')
        assert current
    finally:
        if borrowed is not None: os.close(borrowed)
        for fd in replaced:
            try: os.fstat(fd)
            except OSError: pass
            else: os.close(fd)


def _invoke_capture_owner(scope, path, owner):
    if owner == 'terminal_parent':
        record = dict(archive_id='release-scaling.preflight-file.v1:unit-000.stdout',
            sha256=sha(b'12345678'), size_bytes=8, mode='0400')
        return scope['capture_archive'](record, 'preflight', archive_id=record['archive_id'],
            expected_path=path, maximum_bytes=8, expected_mode=0o400, containment_root=path.parent)
    if owner == 'bounded_parent':
        return scope['_capture_bounded_large_file'](path, 'preflight', maximum_bytes=8)
    parent = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        return scope['_capture_large_file_at'](parent, path.name, path, 'preflight', maximum_bytes=8)
    finally: os.close(parent)


@pytest.mark.parametrize('owner', ['terminal_parent', 'bounded_parent', 'file'])
def test_capture_read_error_closes_the_still_original_slot(tmp_path, monkeypatch, owner):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    parent = []
    original = scope['_capture_large_file_at']
    def at(fd, *args, **kwargs): parent[:] = [fd]; return original(fd, *args, **kwargs)
    scope['_capture_large_file_at'] = at
    seen = []
    def failed(fd, count):
        seen[:] = [fd if owner == 'file' else parent[0]]
        raise OSError('unchanged original read error')
    monkeypatch.setattr(os, 'read', failed)
    with pytest.raises((ValueError, OSError)): _invoke_capture_owner(scope, path, owner)
    assert len(seen) == 1
    with pytest.raises(OSError): os.fstat(seen[0])


@pytest.mark.parametrize('owner', ['terminal_parent', 'bounded_parent', 'file'])
@pytest.mark.parametrize('drift', ['inheritable', 'nonblocking'])
def test_capture_successful_read_cannot_hide_original_flag_drift(tmp_path, monkeypatch, owner, drift):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    parent = []
    original = scope['_capture_large_file_at']
    def at(fd, *args, **kwargs): parent[:] = [fd]; return original(fd, *args, **kwargs)
    scope['_capture_large_file_at'] = at
    original_read, replaced = os.read, []
    def changed(fd, count):
        slot = fd if owner == 'file' else parent[0]
        if not replaced:
            if drift == 'inheritable': os.set_inheritable(slot, True)
            else: fcntl.fcntl(slot, fcntl.F_SETFL, fcntl.fcntl(slot, fcntl.F_GETFL) | os.O_NONBLOCK)
            replaced.append(slot)
        return original_read(fd, count)
    monkeypatch.setattr(os, 'read', changed)
    try:
        with pytest.raises(ValueError): _invoke_capture_owner(scope, path, owner)
        assert os.fstat(replaced[0])
    finally:
        for fd in replaced:
            try: os.fstat(fd)
            except OSError: pass
            else: os.close(fd)


@pytest.mark.parametrize('kind,drift', [
    ('file', 'unchanged'), ('file', 'foreign'), ('file', 'writable'), ('file', 'inheritable'), ('file', 'nonblocking'),
    ('directory', 'unchanged'), ('directory', 'foreign'), ('directory', 'inheritable'), ('directory', 'nonblocking')])
def test_capture_acquisition_failure_preserves_foreign_slots(tmp_path, monkeypatch, kind, drift):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    selected = path if kind == 'file' else path.parent
    foreign = tmp_path / 'foreign'
    if kind == 'file': foreign.write_bytes(b'foreign')
    else: foreign.mkdir(mode=0o700)
    writable = None
    if drift == 'writable':
        selected.chmod(0o600)
        writable = os.open(selected, os.O_RDWR | os.O_CLOEXEC)
        selected.chmod(0o400)
    before = selected.stat()
    identity = (before.st_dev, before.st_ino, stat.S_IFMT(before.st_mode), before.st_uid)
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | (os.O_DIRECTORY if kind == 'directory' else 0)
    original_pin = scope['_capture_descriptor_pin']
    seen = []
    def failed(fd):
        if not seen:
            seen.append(fd)
            if drift in ('foreign', 'writable'):
                other = writable if drift == 'writable' else os.open(foreign,
                    os.O_RDONLY | os.O_CLOEXEC | (os.O_DIRECTORY if kind == 'directory' else 0))
                os.dup2(other, fd, inheritable=False); os.close(other)
            elif drift == 'inheritable': os.set_inheritable(fd, True)
            elif drift == 'nonblocking': fcntl.fcntl(fd, fcntl.F_SETFL, fcntl.fcntl(fd, fcntl.F_GETFL) | os.O_NONBLOCK)
            raise OSError('initial capture pin unavailable')
        return original_pin(fd)
    scope['_capture_descriptor_pin'] = failed
    with pytest.raises(OSError): scope['_open_capture_descriptor'](selected, flags, identity, 'acquisition')
    if drift == 'unchanged':
        with pytest.raises(OSError): os.fstat(seen[0])
    else:
        try: assert os.fstat(seen[0])
        finally: os.close(seen[0])


@pytest.mark.parametrize('owner', ['terminal_parent', 'bounded_parent', 'file'])
def test_capture_mode_drift_rejects_semantics_but_closes_original_slot(tmp_path, monkeypatch, owner):
    scope = _selected_bounded_capture_scope(tmp_path)
    path = _capture_file(tmp_path)
    parent = []
    original = scope['_capture_large_file_at']
    def at(fd, *args, **kwargs): parent[:] = [fd]; return original(fd, *args, **kwargs)
    scope['_capture_large_file_at'] = at
    original_read, changed = os.read, []
    def read(fd, count):
        if not changed:
            slot = fd if owner == 'file' else parent[0]
            os.fchmod(slot, 0o600 if owner == 'file' else 0o500)
            changed.append(slot)
        return original_read(fd, count)
    monkeypatch.setattr(os, 'read', read)
    with pytest.raises(ValueError): _invoke_capture_owner(scope, path, owner)
    assert len(changed) == 1
    with pytest.raises(OSError): os.fstat(changed[0])
