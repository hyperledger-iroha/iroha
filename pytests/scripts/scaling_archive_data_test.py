"""Offline file/data tests; synthetic Norito bytes are deliberately unverified."""
from dataclasses import asdict, fields, replace
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import time

import pytest
ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT/'scripts/nexus'), str(ROOT/'scripts/tests')]
import scaling_archive_data as archive
from scaling_experiment_plan import RUN_KEYS, plan_bytes, encode, admit_plan
from scaling_experiment_custody_test import fixed_plan, budget_for
from resource_evidence_budget import select_run_budget, RUN_FILE_FIELDS
from resource_replay_test import Fixture, identities
from resource_replay import replay, ReplayGeometry
from scaling_measurements_test import run as measurement_fixture
from scaling_measurements import MeasurementRun, measurement_data, measure_data_experiment, measure_experiment
from scaling_completed_authority import PublicRequest, ResourceSnapshot
from scaling_experiment_final_projection import RunManifest, manifest_bytes, report_bytes
from scaling_trial_captures import CaptureCensus
from scaling_public_files import PublicFile, _PATHS, _SOURCE_ROLES
from resource_bundle import ControlBinding
from signed_request_fixture import add_retention
from scaling_worker_sources import SOURCE_NAMES


def sha(value): return hashlib.sha256(value).hexdigest()

def put(path, raw):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    for parent in (path.parent, *path.parents):
        if parent.name == 'evidence': break
        if parent == path.parent: parent.chmod(0o700)
    path.write_bytes(raw); path.chmod(0o600)


def independent_inventory(root):
    body = hashlib.sha256()
    paths = sorted(path for path in root.rglob('*') if path.is_file())
    for path in paths:
        raw = path.read_bytes()
        row = json.dumps([path.relative_to(root).as_posix(),len(raw),sha(raw)],sort_keys=True,ensure_ascii=True,separators=(',',':')).encode('ascii')
        body.update(row+b'\n')
    return sha(b'iroha.scaling.archive.inventory.v1\n'+body.digest())


def bindings(root):
    return archive.ArchiveBindings(sha((root/'manifest.json').read_bytes()),sha((root/'report.json').read_bytes()), independent_inventory(root))


def build_archive(where):
    root = where/'evidence'; root.mkdir(mode=0o700)
    plan = fixed_plan(); raw_plan = plan_bytes(plan)
    images = dict(kagami='c'*64,cli='b'*64,daemon='a'*64,resource_program='d'*64)
    identity = dict(schema=archive._PREFIX+'fixed_identity.v1',hardware=dict(machine_id='synthetic',cpu_model='synthetic',storage_model='synthetic',physical_cores=4,logical_cores=8,memory_bytes=1<<30),
        software=dict(os='historical-os',kernel='historical-kernel',architecture='fixture',python_version='3.12',rustc_version='1.93',source_revision='a'*40,workspace_source_sha256='f'*64,plan_sha256=sha(raw_plan),irohad_sha256=images['daemon'],iroha_cli_sha256=images['cli']))
    source = dict(schema=archive._PREFIX+'source_inputs.v1',source_revision='a'*40,workspace_source_sha256='f'*64,
        executable_images=images,resource_sources=[dict(name=name,sha256=sha(name.encode()),bytes=123) for name in SOURCE_NAMES],scope='main_executable_images_and_fixed_resource_sources')
    controls = {'identity':encode(identity),'plan':raw_plan,'source_closure':encode(source)}
    budget = budget_for(plan, tuple(map(len,controls.values())))
    admit_plan(plan,budget)
    inputs=[]
    for label,raw in controls.items():
        path=f'inputs/{label}.json';put(root/path,raw);inputs.append(ControlBinding(label,path,sha(raw)))
    native_process=dict(pid=200,uid=501,start_seconds=1,start_microseconds=0,start_abstime=1,image_uuid='ab'*16,executable_sha256=images['kagami'])
    cli_process=native_process|dict(pid=201,executable_sha256=images['cli'])
    original_deadline=plan.experiment_timeout_ns+1000000
    manifests=[]; measures=[]
    for index, ((pair,variant),trial) in enumerate(zip(RUN_KEYS,plan.trials,strict=True)):
        allocation=select_run_budget(budget,pair,variant)
        temporary=where/f'raw-{pair}-{variant}';temporary.mkdir(mode=0o700)
        fixture=Fixture(temporary)
        load=trial.load
        geometry=ReplayGeometry(load.warmup_ns,load.measurement_ns,load.drain_ns,load.preparation_ahead_ms*1000000,load.resource_interval_ms*1000000,load.resource_timeout_ms*1000000,load.resource_max_start_lag_ms*1000000)
        rows=[row for row in fixture.events if row['event'] in ('plan','resource_preflight','clock_started','resource_request','resource_observation','resource_collection_finished')]
        planned=measurement_fixture(trial)
        rows[0].update(scheduled_requests=len(planned.requests),pair_index=pair,variant=variant,
            **{name:getattr(geometry,name) for name in ('warmup_ns','measurement_ns','drain_ns','preparation_ahead_ns')})
        next(row for row in rows if row['event']=='clock_started')['initial_offset_ns']=-(geometry.warmup_ns+geometry.drain_ns+geometry.preparation_ahead_ns)
        for request in planned.requests:
            rows.append(dict(event='request_final',plan={name:getattr(request,name) for name in ('cohort','sequence','logical_id','scheduled_offset_ns','account_index')},
                hash=request.transaction_hash,**{name:getattr(request,name) for name in ('offer_offset_ns','acknowledgment_offset_ns','applied_offset_ns','block_height','status_attempts','local_applied_offset_ns','local_block_height','local_status_attempts')},submission_finished=True,failure=None))
        rows.append(dict(event='collection_finished',passed=True,failure=None))
        fixture.events=add_retention(rows);fixture.save()
        prefix=f'runs/pair-{pair:02}/{variant}'
        capture=f'resources/pair-{pair:02}/{variant}'
        (root/capture).parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        fixture.directory.rename(root/capture)
        put(root/prefix/_PATHS['collector_journal'],fixture.journal.read_bytes())
        for role in _SOURCE_ROLES:
            if role!='collector_journal':put(root/prefix/_PATHS[role],('SYNTHETIC UNVERIFIED '+role).encode())
        observed=replay(root/capture,root/prefix/_PATHS['collector_journal'],fixture.digest,fixture.peers,geometry,expected_policy=allocation.policy,allocation=allocation)
        requests=[]
        for signed,applied in zip(observed.signed_requests,observed.applied_requests,strict=True):
            requests.append(PublicRequest(signed.index,*(getattr(signed.plan,name) for name in ('cohort','sequence','logical_id','scheduled_offset_ns','account_index')),signed.hash,signed.canonical_sha256,len(signed.canonical_bytes),*(getattr(applied,f.name) for f in fields(type(applied))[2:])))
        accounts=tuple(f'public-account-{i}' for i in range(4))
        measures.append(MeasurementRun(pair,variant,trial,accounts,tuple(peer.identity.pid for peer in fixture.peers),geometry,observed.preflight,observed.samples,tuple(requests)))
        raw=dict(schema=archive._PREFIX+'raw_run.v1',pair_index=pair,variant=variant,load=asdict(load),geometry=asdict(geometry),resources={name:archive._data(getattr(observed,name)) for name in ResourceSnapshot._fields},requests=[dict(row._asdict()) for row in requests])
        put(root/prefix/_PATHS['raw_run'],encode(raw,allocation.run.raw_run.max_bytes))
        source_rows=[]
        for role in _SOURCE_ROLES:
            path=f'{prefix}/{_PATHS[role]}';raw=(root/path).read_bytes();cap=getattr(allocation.run,role)
            source_rows.append(dict(role=role,label=cap.label,path=path,sha256=sha(raw),bytes=len(raw),max_bytes=cap.max_bytes))
        byrole={row['role']:row for row in source_rows}
        generation=dict(network_id='hash:'+'E'*63+'1#0000',genesis_hash='hash:'+'E'*63+'1#0000',context_id='hash:'+'F'*63+'1#0000',genesis_public_key='ed0120'+'A'*64,chain_discriminant=753,
            anchors_sha256=byrole['genesis_anchors']['sha256'],generator_sha256=images['kagami'],process=native_process,accounts=[dict(index=i,account_id=account) for i,account in enumerate(accounts)])
        ready=[]
        for i,peer in enumerate(fixture.peers):
            ready.append(dict(peer_id=f'peer{i}',process=asdict(peer.identity),node_id='ea0130'+'A'*96,network_id=generation['network_id'],genesis_hash=generation['genesis_hash'],context_id=generation['context_id'],challenge='e'*64,cli_sha256=images['cli'],cli_process=cli_process,client_config_sha256='f'*64,anchors_sha256=generation['anchors_sha256'],report_sha256='a'*64,attestation='dW52ZXJpZmllZA=='))
        canonical=dict(invocation_id='a'*64,request_sha256=byrole['native_request']['sha256'],proof_sha256=byrole['canonical_proof']['sha256'],proof_iroha_hash='c'*63+'1',proof_bytes=byrole['canonical_proof']['bytes'],row_count=len(requests),reply_bytes=123,reply_sha256='d'*64,joined_rows_sha256='e'*64,verifier_sha256=images['kagami'],verifier_process=native_process)
        deadline=1000000+plan.trial_timeout_ns+index*1000000
        receipt=dict(schema=archive._PREFIX+'run_receipt.v1',pair_index=pair,variant=variant,original_deadline_ns=deadline,raw_run_sha256=sha((root/prefix/_PATHS['raw_run']).read_bytes()),generation=generation,readiness=ready,canonical=canonical,load_process=cli_process,peers=[asdict(peer.identity) for peer in fixture.peers],artifacts=source_rows,proof_commands=[dict(role=role,process=native_process) for role in ('proof-prepare','proof-export')])
        put(root/prefix/_PATHS['run_receipt'],encode(receipt,allocation.run.run_receipt.max_bytes))
        files=[]
        for role in RUN_FILE_FIELDS:
            path=f'{prefix}/{_PATHS[role]}';raw=(root/path).read_bytes();cap=getattr(allocation.run,role)
            files.append(PublicFile(role,ControlBinding(cap.label,path,sha(raw)),len(raw),cap.max_bytes))
        # Recorded original metadata commitments remain data; no fake owner is built.
        census=CaptureCensus(observed.capture_file_count,observed.capture_bytes,sha(f'original-census:{pair}:{variant}'.encode()),sha(f'original-metadata:{pair}:{variant}'.encode()))
        manifests.append(RunManifest(pair,variant,deadline,tuple(files),census))
    raw_manifest=manifest_bytes(plan,budget,original_deadline,tuple(inputs),tuple(manifests),budget.control_budgets[0].max_bytes)
    measurements=measure_data_experiment(plan,tuple(measures))
    put(root/'manifest.json',raw_manifest)
    put(root/'report.json',report_bytes(sha(raw_manifest),measurements,budget.control_budgets[1].max_bytes))
    for directory in root.rglob('*'):
        if directory.is_dir():directory.chmod(0o700)
    return root,plan,budget,bindings(root),measurements


@pytest.fixture(scope='module')
def complete(tmp_path_factory):return build_archive(tmp_path_factory.mktemp('archive-source'))


def test_complete_real_file_resource_replay_recomputes_all_ten_runs(complete):
    root,plan,budget,expected,measurements=complete
    result=archive.inspect_archive(root,plan,budget,expected)
    assert result.measurements==measurements
    assert result.inventory.files==budget.resource_member_count+budget.control_file_count==2225
    assert result.inventory.sha256==expected.inventory_sha256
    assert len(result.native_runs)==10
    assert result.required_obligations==('canonical_native_cryptographic_replay','authenticated_original_parent_execution')
    assert not any(hasattr(result,name) for name in ('passed','accepted','qualified','success'))
    assert result.measurements.throughput_criterion_met is False


def test_native_replay_inputs_keep_the_complete_recomputed_measurement_data(complete):
    root,plan,budget,expected,measurements=complete
    result=archive.inspect_archive(root,plan,budget,expected)
    values=tuple(run.measurement for run in result.native_runs)
    assert measure_data_experiment(plan,values)==measurements
    for run,trial,measured in zip(result.native_runs,plan.trials,measurements.runs,strict=True):
        value=run.measurement
        assert type(value) is MeasurementRun
        assert (value.pair_index,value.variant)==(run.pair_index,run.variant)
        assert value.plan==trial
        assert len(value.requests)==measured.warmup_requests+measured.measurement_requests
        assert tuple(request.index for request in value.requests)==tuple(range(len(value.requests)))
        assert all(request.block_height==request.local_block_height for request in value.requests)
        with pytest.raises((AttributeError,TypeError)):
            value.requests=()
        with pytest.raises((AttributeError,TypeError)):
            value.requests[0].block_height=0


def test_copy_with_new_inodes_and_times_replays_without_old_processes_or_clock(complete,tmp_path,monkeypatch):
    root,plan,budget,expected,_=complete
    copied=tmp_path/'evidence';shutil.copytree(root,copied,copy_function=shutil.copyfile)
    for path in copied.rglob('*'):
        path.chmod(0o700 if path.is_dir() else 0o600)
        os.utime(path,ns=(1,1))
    copied.chmod(0o700)
    assert (root/'manifest.json').stat().st_ino != (copied/'manifest.json').stat().st_ino
    def forbidden(*args,**kwargs):raise AssertionError('offline archive used a live capability')
    import resource_process, scaling_completed_authority, scaling_experiment_custody, resource_experiment
    monkeypatch.setattr(time,'monotonic_ns',forbidden)
    monkeypatch.setattr(resource_process,'DarwinProcessReader',forbidden)
    monkeypatch.setattr(scaling_completed_authority,'CompletedRunAuthority',forbidden)
    monkeypatch.setattr(scaling_experiment_custody,'FixedExperimentCustody',forbidden)
    monkeypatch.setattr(resource_experiment,'ResourceExperiment',forbidden)
    assert archive.inspect_archive(copied,plan,budget,expected).inventory.sha256==expected.inventory_sha256


@pytest.mark.parametrize('case',['extra','missing','symlink','hardlink','directory_symlink','wrong_mode','fifo','oversize','changed_bytes','missing_report','changed_report'])
def test_inventory_rejects_namespace_and_physical_changes(complete,tmp_path,case):
    root,plan,budget,expected,_=complete
    copied=tmp_path/'evidence';shutil.copytree(root,copied)
    path=(copied/'report.json' if case in ('missing_report','changed_report')
          else copied/'runs/pair-01/one_lane/native/proof.nrt')
    if case=='extra':put(copied/'extra',b'x')
    elif case in ('missing','missing_report'):path.unlink()
    elif case=='symlink':path.unlink();path.symlink_to(root/'runs/pair-01/one_lane/native/proof.nrt')
    elif case=='hardlink':path.unlink();os.link(root/'runs/pair-01/one_lane/native/proof.nrt',path)
    elif case=='directory_symlink':
        directory=copied/'runs/pair-01/one_lane/native';shutil.rmtree(directory);directory.symlink_to(root/'runs/pair-01/one_lane/native',target_is_directory=True)
    elif case=='wrong_mode':path.chmod(0o644)
    elif case=='fifo':path.unlink();os.mkfifo(path,0o600)
    elif case=='oversize':
        with path.open('wb') as output:output.truncate(65537)
    elif case=='changed_bytes':path.write_bytes(b'X'*path.stat().st_size)
    elif case=='changed_report':
        value=json.loads(path.read_bytes())
        value['throughput_criterion_met']=not value['throughput_criterion_met']
        path.write_bytes(json.dumps(value,sort_keys=True,separators=(',',':')).encode('ascii'))
    try:
        with pytest.raises(archive.ArchiveDataError):archive.inspect_archive(copied,plan,budget,expected)
    finally:
        if case=='hardlink':path.unlink()


def test_data_api_keeps_identical_measurement_arithmetic_and_rejects_foreign_data():
    from scaling_measurements_test import matrix
    plan,values=matrix()
    data=tuple(measurement_data(value) for value in values)
    assert measure_data_experiment(plan,data)==measure_experiment(plan,values)
    from scaling_measurements import MeasurementError
    with pytest.raises(MeasurementError):measure_data_experiment(plan,(object(),*data[1:]))


def refresh_public_hashes(root):
    """Refresh ordinary byte bindings; this deliberately supplies no authority."""
    manifest=json.loads((root/'manifest.json').read_bytes())
    for item in manifest['inputs']:
        item['sha256']=sha((root/item['path']).read_bytes())
    for run in manifest['runs']:
        files={row['role']:row for row in run['files']}
        receipt_path=root/files['run_receipt']['path']
        receipt=json.loads(receipt_path.read_bytes())
        receipt['raw_run_sha256']=sha((root/files['raw_run']['path']).read_bytes())
        for row in receipt['artifacts']:
            raw=(root/row['path']).read_bytes();row.update(sha256=sha(raw),bytes=len(raw))
        put(receipt_path,encode(receipt))
        for row in run['files']:
            raw=(root/row['path']).read_bytes();row.update(sha256=sha(raw),bytes=len(raw))
    put(root/'manifest.json',encode(manifest))
    report=json.loads((root/'report.json').read_bytes())
    report['manifest_sha256']=sha((root/'manifest.json').read_bytes())
    put(root/'report.json',encode(report))
    return bindings(root)


@pytest.mark.parametrize('case',['report_flag','report_ratio','report_unknown','manifest_unknown','manifest_order','raw_reduction','raw_request','receipt_unknown','receipt_peer','receipt_canonical_count','receipt_artifact'])
def test_refreshed_hashes_cannot_replace_schema_or_recomputed_data(complete,tmp_path,case):
    root,plan,budget,_,_=complete
    copied=tmp_path/'evidence';shutil.copytree(root,copied)
    if case.startswith('report'):
        path=copied/'report.json';value=json.loads(path.read_bytes())
        if case=='report_flag':value['throughput_criterion_met']=True
        elif case=='report_ratio':value['median_throughput_ratio']['numerator']+=1
        else:value['accepted']=True
    elif case.startswith('manifest'):
        path=copied/'manifest.json';value=json.loads(path.read_bytes())
        if case=='manifest_unknown':value['accepted']=True
        else:value['runs'][0],value['runs'][1]=value['runs'][1],value['runs'][0]
    elif case.startswith('raw'):
        path=copied/'runs/pair-01/one_lane/raw_samples.json';value=json.loads(path.read_bytes())
        if case=='raw_reduction':value['resources']['preflight']['queue_size_sum']+=1
        else:value['requests'][0]['applied_offset_ns']+=1
    else:
        path=copied/'runs/pair-01/one_lane/run_receipt.json';value=json.loads(path.read_bytes())
        if case=='receipt_unknown':value['success']=True
        elif case=='receipt_peer':value['peers'][0]['pid']+=99
        elif case=='receipt_canonical_count':value['canonical']['row_count']+=1
        else:value['artifacts'].reverse()
    put(path,encode(value))
    expected=refresh_public_hashes(copied)
    with pytest.raises(archive.ArchiveDataError):archive.inspect_archive(copied,plan,budget,expected)


@pytest.mark.parametrize('raw',[b'{"a":1,"a":1}',b'{"a":1.0}',b'{"a":NaN}',b'{"a":1}\n',b'{ "a":1}',b'{"a":'+b'1'*129+b'}',b'['*65+b'0'+b']'*65])
def test_canonical_framing_rejects_duplicates_floats_whitespace_and_unbounded_shapes(raw):
    with pytest.raises((archive.ArchiveDataError,ValueError)):
        archive._canonical(raw,1024*1024)


@pytest.mark.parametrize('case',['hardware_bool','software_hash','source_scope','source_extra','worker_order','worker_extra','worker_size','identity_extra','worker_hash'])
def test_input_control_shapes_have_no_compatibility_form(complete,case):
    root,_,_,_,_=complete
    identity=json.loads((root/'inputs/identity.json').read_bytes())
    source=json.loads((root/'inputs/source_closure.json').read_bytes())
    if case=='hardware_bool':identity['hardware']['physical_cores']=True
    elif case=='software_hash':identity['software']['iroha_cli_sha256']='z'*64
    elif case=='source_scope':source['scope']='whole_host_qualified'
    elif case=='source_extra':source['accepted']=True
    elif case=='worker_order':source['resource_sources'].reverse()
    elif case=='worker_extra':source['resource_sources'][0]['legacy']=True
    elif case=='worker_size':source['resource_sources'][0]['bytes']=True
    elif case=='identity_extra':identity['success']=True
    elif case=='worker_hash':source['resource_sources'][0]['sha256']='A'*64
    with pytest.raises(archive.ArchiveDataError):archive._controls(identity,source,identity['software']['plan_sha256'])


def test_archive_computed_inventory_is_only_data_and_wrong_expected_commitment_rejects(complete):
    root,plan,budget,expected,_=complete
    inventory=archive.inventory_archive(root,plan,budget)
    assert inventory.sha256==expected.inventory_sha256
    assert not hasattr(inventory,'verified')
    with pytest.raises(archive.ArchiveDataError):archive.inspect_archive(root,plan,budget,replace(expected,inventory_sha256='0'*64))


def test_mutation_during_replay_cannot_survive_final_inventory(complete,tmp_path,monkeypatch):
    root,plan,budget,expected,_=complete
    copied=tmp_path/'evidence';shutil.copytree(root,copied)
    original=archive.replay
    def mutate(*args,**kwargs):
        value=original(*args,**kwargs)
        path=copied/'runs/pair-05/four_lane/native/proof.nrt'
        path.write_bytes(b'X'*path.stat().st_size)
        return value
    monkeypatch.setattr(archive,'replay',mutate)
    with pytest.raises(archive.ArchiveDataError):archive.inspect_archive(copied,plan,budget,expected)
