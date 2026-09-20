"""Exact complete-preflight protocol using original-process doubles only.

A synthetic complete outcome mapping tests the execution protocol. It does not
qualify the complete current inventory or execute its native/child cases.
"""
import ast
import copy
from dataclasses import dataclass
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
from types import SimpleNamespace as NS

import pytest

ROOT = Path(__file__).resolve().parents[2]
OWNER = ROOT/'scripts/nexus/scaling_release_preflight.py'
DRIVER = ROOT/'pytests/scripts/run_scaling_collector_preflight.py'


def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec);sys.modules[name]=module
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module


m=load('tested_scaling_preflight',OWNER)
d=load('tested_scaling_preflight_driver',DRIVER)


def inventory():
    return json.loads((ROOT/m.INVENTORY_PATH).read_bytes())


def complete_fixture():
    value=inventory()
    available=[node for nodes in value['pytest_suites'].values() for node in nodes
        if node not in value['migrated_outcomes'].values()]
    for row,node in zip(value['pending_outcomes'],available):
        value['migrated_outcomes'][row['id']]=node
    value['pending_outcomes']=[]
    return value


def test_real_inventory_preserves_exact_canonical_and_unresolved_outcomes():
    value=m.decode_inventory((ROOT/m.INVENTORY_PATH).read_bytes())
    assert set(value['phases'])==set(m.PHASE_COUNTS)
    assert sum(map(len,value['phases'].values()))==137
    assert set(value['migrated_outcomes'])|{row['id'] for row in value['pending_outcomes']}==set(m.REQUIRED_MIGRATION_OUTCOMES)
    assert len(value['migrated_outcomes'])==101
    assert value['pending_outcomes']==[]
    assert len(value['pytest_suites'])>=63
    assert len({n for rows in value['pytest_suites'].values() for n in rows})>=4418


@pytest.mark.parametrize('change',['empty','missing_suite','duplicate_node','wrong_node','zero_phase',
    'missing_phase','extra_phase','drop_outcome','fake_migration','duplicate_mapping','unknown',
    'wrong_schema','wrong_digest','unsafe_source','bool_phase','duplicate_json','depth','oversize',
    'missing_data','foreign_data','missing_spec'])
def test_inventory_rejects_partial_unknown_or_erased_assertions(change):
    value=inventory();name=next(iter(value['pytest_suites']))
    if change=='empty':value['pytest_suites']={}
    elif change=='missing_suite':del value['pytest_suites'][name]
    elif change=='duplicate_node':value['pytest_suites'][name].append(value['pytest_suites'][name][0])
    elif change=='wrong_node':value['pytest_suites'][name][0]='foreign::test_case'
    elif change=='zero_phase':value['phases']['bootstrap']=[]
    elif change=='missing_phase':del value['phases']['runtime']
    elif change=='extra_phase':value['phases']['unknown']=[]
    elif change=='drop_outcome':value['migrated_outcomes'].pop(next(iter(value['migrated_outcomes'])))
    elif change=='fake_migration':value['migrated_outcomes']['fake']='foreign::test_case'
    elif change=='duplicate_mapping':
        names=list(value['migrated_outcomes']);value['migrated_outcomes'][names[1]]=value['migrated_outcomes'][names[0]]
    elif change=='unknown':value['accepted']=True
    elif change=='wrong_schema':value['schema']='retired'
    elif change=='wrong_digest':value['sources'][next(iter(value['sources']))]=True
    elif change=='unsafe_source':value['sources']['scripts/../../outside.py']='0'*64
    elif change=='missing_data':del value['sources'][m.DATA_FIXTURES[0]]
    elif change=='missing_spec':del value['sources']['specs/sumeragi_v2_liveness.md']
    elif change=='foreign_data':value['sources']['pytests/fixtures/unselected.json']='0'*64
    elif change=='bool_phase':value['phases']['runtime']=[True]*28
    raw=m.canonical(value)
    if change=='duplicate_json':raw=raw.replace(b'{',b'{"schema":"x",',1)
    elif change=='depth':raw=b'['*13+b'0'+b']'*13
    elif change=='oversize':raw=b' '*(m.MAX_INVENTORY_BYTES+1)
    with pytest.raises(m.ScalingPreflightError):m.decode_inventory(raw)



@pytest.mark.parametrize('path', [
    'specs/torii/api_contract.md',
    'docs/source/taira_dataspace_deploy.md',
])
def test_inventory_requires_current_finality_contract_documents(path):
    value = inventory()
    assert path in m.DATA_FIXTURES and path in value['sources']
    del value['sources'][path]
    with pytest.raises(m.ScalingPreflightError):
        m.decode_inventory(m.canonical(value))

def report(node,when='call',outcome='passed',**extras):
    return NS(nodeid=node,when=when,outcome=outcome,failed=outcome=='failed',
        skipped=outcome=='skipped',**extras)


def finish(observer,node):
    for when in ('setup','call','teardown'):observer.pytest_runtest_logreport(report(node,when))


def test_exact_pytest_observes_all_collection_and_terminal_phases():
    node='suite.py::test_case';observer=d.ExactPytestOutcomes([d.node_digest(node)])
    observer.pytest_collection_modifyitems(None,None,[NS(nodeid=node)])
    finish(observer,node)
    actual=observer.result(0)
    assert actual['passed'] is True and actual['tests_run']==1
    assert actual['outcome_node_sha256s']==actual['collected_node_sha256s']==actual['node_sha256s']==[d.node_digest(node)]


@pytest.mark.parametrize('change',['empty','missing_phase','duplicate','nonzero','skipped','failed',
    'deselected','foreign','xfail','subtest_failed','collect_error','collect_skip'])
def test_actual_pytest_protocol_rejects_every_unexecuted_or_failed_outcome(change):
    node='suite.py::test_case';observer=d.ExactPytestOutcomes([d.node_digest(node)])
    observer.pytest_collection_modifyitems(None,None,[NS(nodeid=node)])
    if change!='empty':finish(observer,node)
    if change=='missing_phase':del observer.reports[d.node_digest(node)]['teardown']
    elif change=='duplicate':observer.pytest_runtest_logreport(report(node))
    elif change=='skipped':observer.pytest_runtest_logreport(report(node,outcome='skipped'))
    elif change=='failed':observer.pytest_runtest_logreport(report(node,outcome='failed'))
    elif change=='deselected':observer.pytest_deselected([NS(nodeid=node)])
    elif change=='foreign':observer.pytest_runtest_logreport(report('other::test_case'))
    elif change=='xfail':observer.pytest_runtest_logreport(report(node,wasxfail='expected'))
    elif change=='subtest_failed':observer.pytest_runtest_logreport(report(node,outcome='failed',context=NS()))
    elif change=='collect_error':observer.pytest_collectreport(NS(failed=True,skipped=False))
    elif change=='collect_skip':observer.pytest_collectreport(NS(failed=False,skipped=True))
    assert observer.result(1 if change=='nonzero' else 0)['passed'] is False


def test_wrong_or_zero_collection_fails_before_test_execution():
    for nodes in ([],[NS(nodeid='foreign::test_case')],[NS(nodeid='a::b'),NS(nodeid='a::b')]):
        with pytest.raises(ValueError):d.ExactPytestOutcomes([d.node_digest('a::b')]).pytest_collection_modifyitems(None,None,nodes)


def test_execute_suite_has_one_exact_source_selection_and_no_partial_options(tmp_path):
    name='pytests/scripts/test.py';node=name+'::test_value';seen=[]
    def main(argv,plugins):
        seen.extend(argv);observer=plugins[0]
        observer.pytest_collection_modifyitems(None,None,[NS(nodeid=node)]);finish(observer,node)
        return 0
    result=d.execute_suite(NS(main=main),tmp_path,tmp_path/'work',name,[d.node_digest(node)])
    assert (tmp_path/'work/pytest.ini').read_bytes()==b'[pytest]\n'
    assert seen==['-q','--disable-plugin-autoload','-p','no:cacheprovider','--noconftest','-c',str(tmp_path/'work/pytest.ini'),'--rootdir='+str(tmp_path),
        '--confcutdir='+str(tmp_path),'--basetemp='+str(tmp_path/'work/pytest'),str(tmp_path/name)]
    assert result['passed'] is True


@pytest.mark.parametrize('failure',['inputs','inventory','blake3_close','package_close'])
def test_actual_driver_finalbody_closes_both_owners_after_final_check_failure(tmp_path,failure):
    tree=ast.parse(DRIVER.read_bytes())
    main=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='main')
    final=next(n for n in main.body if isinstance(n,ast.Try))
    body=ast.Module(body=final.finalbody,type_ignores=[])
    closed=[];raw=b'original';before={'source':'digest'}
    def inputs():
        if failure=='inputs':raise OSError('final source unreadable')
        return before
    class Inventory:
        def read_bytes(self):
            if failure=='inventory':raise OSError('final inventory unreadable')
            return raw
    class Root:
        def __truediv__(self,other):return Inventory()
    def owner(name):
        def close():
            closed.append(name)
            if failure==name+'_close':raise OSError('original owner close failed')
        return NS(close=close)
    result=tmp_path/'result.json'
    env=dict(inputs=inputs,before=before,raw=raw,root=Root(),contract=m,
        record={'passed':True},blake3=owner('blake3'),package=owner('package'),args=NS(result=result))
    with pytest.raises(OSError):exec(compile(body,str(DRIVER),'exec'),env)
    assert closed==['blake3','package']
    assert not result.exists()


@dataclass(frozen=True)
class Snapshot:
    path: Path
    data: bytes
    sha256: str


class Commands:
    def __init__(self):self._process=None;self._reaped=False;self.terminal=None


class Dependency:
    def __init__(self,source,bundle,inventory):
        self.paths=NS(source_root=source,bundle_root=bundle,inventory=inventory,inventory_sha256='d'*64)
        self.closed=False;self.checked=0
    def verify(self):
        assert not self.closed;self.checked+=1
    def close(self):self.closed=True


_SEAM_ARCHIVES = []


@pytest.fixture(autouse=True)
def close_test_only_archive_descriptors():
    yield
    # Explicit Popen doubles own no processes; close only this harness's actual
    # file descriptors after each test, even an unknown-terminal negative.
    for archive, descriptor in _SEAM_ARCHIVES:
        archive.close(); os.close(descriptor)
    _SEAM_ARCHIVES.clear()


class ProcessSeam:
    def __init__(self,tmp_path,monkeypatch,case='positive',pending=False):
        self.case=case;self.clock=100_000_000_000;self.rows={};self.calls=[];self.dependencies=[]
        value=inventory() if pending else complete_fixture()
        if pending:
            missing=next(iter(value['migrated_outcomes']))
            del value['migrated_outcomes'][missing]
            value['pending_outcomes']=[dict(id=missing,reason='explicit unresolved protocol fixture')]
        self.inventory_raw=m.canonical(value)
        self.path=tmp_path/'work'
        monkeypatch.setattr(m.time,'monotonic_ns',lambda:self.clock)
        def stage(source,destination):destination.mkdir(mode=0o700)
        def provision(source,bundle,inventory):
            result=Dependency(source,bundle,inventory);self.dependencies.append(result);return result
        dependency_api=NS(stage_dependency_source=stage,stage_test_dependency_source=stage,
            PythonDependencies=NS(provision=provision),PythonTestDependencies=NS(provision=provision))
        from sumeragi_v2_release_scaling_selection_test import bootstrap_definitions
        bootstrap=bootstrap_definitions();self.bootstrap=bootstrap; evidence=tmp_path/'bootstrap';evidence.mkdir(mode=0o700)
        descriptor=os.open(evidence,os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC)
        self.archive=bootstrap.BootstrapPreflightArchive(evidence,descriptor)
        _SEAM_ARCHIVES.append((self.archive,descriptor))
        self.identity=dict(head_commit='a'*40,head_tree='b'*40,workspace_source_manifest_sha256='c'*64)
        self.owner=m.CompleteScalingPreflight(ROOT,tmp_path/'installed',self.path,Path(sys.executable).resolve(),
            {'PATH':'/protected'},600,m.PreflightApi(Commands,self.run,self.read,self.unchanged),dependency_api,
            archive=self.archive,candidate_identity=self.identity,invocation_sha256='e'*64)
    def read(self,path,label,maximum_bytes):
        if path in self.rows:return self.rows[path]
        raw=self.inventory_raw if path==ROOT/m.INVENTORY_PATH else path.read_bytes()
        assert len(raw)<=maximum_bytes
        result=Snapshot(path,raw,hashlib.sha256(raw).hexdigest());self.rows[path]=result;return result
    def unchanged(self,row,label,maximum_bytes):assert self.rows.get(row.path,row)==row
    def run(self,executable,argv,**kwargs):
        command=kwargs['observation'];index=len(self.calls);self.calls.append((executable,argv,kwargs))
        assert kwargs['pass_fds']==()
        if self.case=='no_spawn' and index==0:raise OSError('proved Popen failure')
        command._process=object()
        if self.case=='unknown_terminal' and index==0:raise RuntimeError('wait has no terminal observation')
        self.clock+=1_000_000
        rc=7 if self.case=='nonzero' and index==0 else 0
        deadline=kwargs['deadline_ns'];completed=self.clock
        if self.case=='deadline' and index==0:self.clock=completed=deadline+1
        command._reaped=True
        stdout=b'unit stdout\n';stderr=b''
        command.terminal=NS(pid=100+index,stdout_bytes=len(stdout),stderr_bytes=len(stderr),
            stdout_sha256=hashlib.sha256(stdout).hexdigest(),stderr_sha256=hashlib.sha256(stderr).hexdigest(),
            returncode=rc,violations=(),descriptors=(),argv=(str(executable),*argv),
            cwd=str(ROOT),deadline_ns=deadline,started_ns=self.clock-1_000_000,completed_ns=completed,
            environment_sha256=hashlib.sha256(m.canonical(kwargs['environment'])).hexdigest())
        if self.case=='descriptor' and index==0:command.terminal.descriptors=(99,)
        if self.case=='argv' and index==0:command.terminal.argv=()
        if self.case=='environment' and index==0:command.terminal.environment_sha256='0'*64
        if self.case=='violation' and index==0:command.terminal.violations=('output',)
        if self.case=='renewed' and index==1:command.terminal.deadline_ns+=1
        if self.case=='wait_error' and index==0:raise RuntimeError('observed error after natural reap')
        name=argv[5]
        expected=self.owner._inventory['phases' if argv[4]=='--phase' else 'pytest_suites'][name]
        value=dict(passed=True,tests_run=len(expected),failures=0,errors=0,skipped=0,
            isolated=True,no_site=True,no_bytecode=True,inputs_unchanged=True,subtest_observations=0)
        if argv[4]=='--phase':
            inputs,copies=self.owner._phase_inputs()
            value.update(phase=name,node_ids=expected,expected_node_ids=expected,source_registry_count=58,
                external_native_processes=False,qualification='explicit process seam',elapsed_ns=1,inputs_before=inputs,inputs_after=inputs,
                copied_sources_after=copies,forbidden_process_attempts=[])
            if name=='bootstrap':value.update(actual_private_blake3_import=True,actual_bootstrap_composition=True)
        else:
            value.update(suite=name,node_sha256s=expected,inventory_sha256=self.owner._inventory_snapshot.sha256,
                inputs_before=self.owner._inventory['sources'],inputs_after=self.owner._inventory['sources'],
                collected_node_sha256s=expected,outcome_node_sha256s=expected)
        if self.case=='report_false' and index==0:value['passed']=False
        if self.case=='report_skipped' and index==0:value['skipped']=1
        if self.case=='report_empty' and index==0:value['node_ids']=[]
        if self.case=='report_isolation' and index==0:value['isolated']=False
        result_path=Path(argv[argv.index('--result')+1]);raw=m.canonical(value)
        self.rows[result_path]=Snapshot(result_path,raw,hashlib.sha256(raw).hexdigest())
        return NS(returncode=rc,stdout=stdout,stderr=stderr)


def test_complete_protocol_owns_every_phase_once_and_one_original_deadline(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch);rows=seam.owner.run()
    assert len(rows)==len(m.PHASE_COUNTS)+len(m.REQUIRED_PYTEST_SUITES)
    assert len(seam.calls)==len(rows) and len({call[2]['deadline_ns'] for call in seam.calls})==1
    assert [row.name for row in rows[:len(m.PHASE_COUNTS)]]==list(m.PHASE_COUNTS)
    assert [row.name for row in rows[len(m.PHASE_COUNTS):]]==list(m.REQUIRED_PYTEST_SUITES)
    assert all(call[1][:3]==('-I','-B','-S') for call in seam.calls)
    for _,argv,_ in seam.calls[len(m.PHASE_COUNTS):]:
        assert argv[-16:-8]==('--dependency-source',str(seam.path/'dependencies-source'),
            '--dependency-bundle',str(seam.path/'dependencies-bundle'),
            '--dependency-inventory',str(seam.path/'dependencies-inventory.json'),
            '--dependency-inventory-sha256','d'*64)
        assert argv[-8:]==('--blake3-dependency-source',str(seam.path/'blake3-source'),
            '--blake3-dependency-bundle',str(seam.path/'blake3-bundle'),
            '--blake3-dependency-inventory',str(seam.path/'blake3-inventory.json'),
            '--blake3-dependency-inventory-sha256','d'*64)
        assert '--dependency-root' not in argv
    with pytest.raises(m.ScalingPreflightError):seam.owner.run()
    # A repeated call cannot replace the retained successful observations.
    seam.owner.verify_retained();seam.owner.close();seam.owner.verify_retained()
    assert all(owner.closed for owner in seam.dependencies)


def test_changed_required_data_fixture_rejects_before_any_dependency_or_child(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch);fixture=ROOT/m.DATA_FIXTURES[0]
    raw=fixture.read_bytes()+b'\n'
    seam.rows[fixture]=Snapshot(fixture,raw,hashlib.sha256(raw).hexdigest())
    with pytest.raises(m.ScalingPreflightError,match='source inventory changed'):seam.owner.run()
    assert seam.calls==[] and seam.dependencies==[]
    seam.owner.close()


@pytest.mark.parametrize('case',['nonzero','descriptor','argv','environment','violation','renewed',
    'deadline','report_false','report_skipped','report_empty','report_isolation','wait_error'])
def test_no_partial_or_failed_original_operation_can_mint_preflight_success(tmp_path,monkeypatch,case):
    seam=ProcessSeam(tmp_path,monkeypatch,case)
    with pytest.raises((m.ScalingPreflightError,RuntimeError)):seam.owner.run()
    with pytest.raises(m.ScalingPreflightError):seam.owner.verify_retained()
    seam.owner.close()
    assert all(owner.closed for owner in seam.dependencies)


def test_unknown_terminal_retains_dependencies_until_same_original_owner_reaps(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch,'unknown_terminal')
    with pytest.raises(RuntimeError):seam.owner.run()
    with pytest.raises(m.ScalingPreflightError,match='natural terminal'):seam.owner.close()
    assert not any(owner.closed for owner in seam.dependencies)
    command=seam.owner._commands[0];original=command._process
    command._reaped=True;command.terminal=NS(returncode=7)
    assert command._process is original
    seam.owner.close();assert all(owner.closed for owner in seam.dependencies)


def test_failed_final_verification_is_never_promoted_by_close(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch)
    verify=seam.owner.verify
    def fail():
        verify()
        raise OSError('source became unreadable during final check')
    monkeypatch.setattr(seam.owner,'verify',fail)
    with pytest.raises(OSError):seam.owner.run()
    assert seam.owner._result_pin is not None and seam.owner._phase=='failed'
    seam.owner.close()
    assert all(owner.closed for owner in seam.dependencies) and seam.owner._phase=='failed'
    with pytest.raises(m.ScalingPreflightError):seam.owner.verify_retained()


@pytest.mark.parametrize('failure',['verify','pytest_close','blake3_close'])
def test_parent_close_releases_both_reaped_dependency_owners_on_failure(tmp_path,monkeypatch,failure):
    seam=ProcessSeam(tmp_path,monkeypatch);seam.owner.run();closed=[]
    def fail():raise OSError('final verification failed')
    if failure=='verify':monkeypatch.setattr(seam.owner,'verify',fail)
    for name,owner in (('pytest',seam.owner._pytest),('blake3',seam.owner._blake3)):
        def close(name=name,owner=owner):
            closed.append(name);owner.closed=True
            if failure==name+'_close':raise OSError('original dependency close failed')
        monkeypatch.setattr(owner,'close',close)
    with pytest.raises(OSError):seam.owner.close()
    assert closed==['pytest','blake3'] and seam.owner._phase=='failed'
    with pytest.raises(m.ScalingPreflightError):seam.owner.verify_retained()


def test_proved_no_spawn_and_unresolved_inventory_never_count_as_complete(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch,'no_spawn')
    with pytest.raises(OSError):seam.owner.run()
    assert seam.owner._commands[0]._process is None
    seam.owner.close();assert all(owner.closed for owner in seam.dependencies)
    second=tmp_path/'second';second.mkdir()
    seam=ProcessSeam(second,monkeypatch,pending=True)
    with pytest.raises(m.ScalingPreflightError,match='unresolved'):seam.owner.run()
    assert len(seam.calls)==len(m.PHASE_COUNTS)+len(m.REQUIRED_PYTEST_SUITES)
    seam.owner.close()


def test_parent_preflight_precedes_collector_clock_and_close_precedes_loader_release():
    tree=ast.parse((ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_bytes())
    cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='BootstrapScalingOperation')
    methods={n.name:n for n in cls.body if isinstance(n,ast.FunctionDef)}
    calls=sorted((n.lineno,ast.unparse(n.func)) for n in ast.walk(methods['prepare']) if isinstance(n,ast.Call))
    names=[name for _,name in calls]
    assert names.index('preflight.CompleteScalingPreflight')<names.index('self._preflight.run')<names.index('provisioning.PreparedScalingInputs.prepare')
    assert names[names.index('self._preflight.run')+1]=='self._preflight.verify'
    closes=sorted((n.lineno,ast.unparse(n.func)) for n in ast.walk(methods['close']) if isinstance(n,ast.Call))
    assert [name for _,name in closes].index('self._preflight.close')<[name for _,name in closes].index('self._loader.close')
    assert 'self._preflight.verify_retained()' in ast.unparse(methods['_encode_record'])


def test_retained_preflight_join_survives_runtime_pruning_but_rejects_changed_original_record(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch);seam.owner.run();seam.owner.close()
    def no_reopen(*args,**kwargs):raise AssertionError('pruned runtime was reopened')
    monkeypatch.setattr(m.Path,'read_bytes',no_reopen)
    seam.owner.verify_retained()
    prior=seam.owner._results[0]
    seam.owner._results[0]=Snapshot(prior.path,prior.data+b' ',prior.sha256)
    with pytest.raises(m.ScalingPreflightError):seam.owner.verify_retained()


def test_corridor_receipt_has_no_retired_scaling_53_row_and_binds_new_component():
    path=ROOT/'scripts/write_sumeragi_v2_release_receipt_corridor_log.py'
    raw=path.read_bytes()
    assert b'preflight-multilane-scaling' not in raw
    assert b'validate_multilane_scaling_evidence_test.py' not in raw
    digest=hashlib.sha256(raw).hexdigest()
    receipt=(ROOT/'scripts/write_sumeragi_v2_release_receipt.py').read_text()
    assert receipt.count(digest)==2
    parent=(ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_text()
    assert 'self._preflight.verify_retained()' in parent


@pytest.mark.parametrize('value',['600','28800','86400'])
def test_preflight_timeout_has_one_bounded_cli_policy(value):
    from sumeragi_v2_release_scaling_selection_test import bootstrap_definitions
    b=bootstrap_definitions();parser=b._parser();actions={row.dest:row for row in parser._actions}
    action=actions['scaling_preflight_timeout_seconds']
    assert action.default==28800 and action.type(value)==int(value)
    assert actions['command_timeout_seconds'].default==600
    assert action.option_strings==['--scaling-preflight-timeout-seconds']


@pytest.mark.parametrize('value',['0','599','86401','no','1.5'])
def test_preflight_timeout_rejects_invalid_cli_values(value):
    import argparse
    from sumeragi_v2_release_scaling_selection_test import bootstrap_definitions
    with pytest.raises(argparse.ArgumentTypeError):bootstrap_definitions()._scaling_preflight_timeout(value)


@pytest.mark.parametrize('value',[600,28800,86400,True,False,599,86401,28800.0,'28800',None])
def test_receipt_policy_requires_exact_parent_integer(value):
    path=ROOT/'scripts/write_sumeragi_v2_release_receipt.py';tree=ast.parse(path.read_bytes())
    owner=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_validate_bootstrap_evidence')
    offset=next(i for i,n in enumerate(owner.body) if isinstance(n,ast.Assign) and
        any(isinstance(t,ast.Name) and t.id=='preflight_timeout' for t in n.targets))
    block=ast.Module(body=owner.body[offset:offset+2],type_ignores=[])
    env=dict(runner={'scaling_preflight_timeout_seconds':value},ReceiptError=ValueError)
    if type(value) is int and 600<=value<=86400:
        exec(compile(block,str(path),'exec'),env);assert env['preflight_timeout']==value
    else:
        with pytest.raises(ValueError):exec(compile(block,str(path),'exec'),env)


def test_durable_preflight_preserves_each_original_buffer_and_command_after_runtime_pruning(tmp_path,monkeypatch):
    import shutil
    seam=ProcessSeam(tmp_path,monkeypatch);seam.owner.run();seam.owner.close()
    binding=seam.owner.archive_binding
    assert binding['inventory']['files']==4*len(seam.calls)+2
    assert (seam.archive.path/'inventory.json').read_bytes()==seam.inventory_raw
    index=json.loads((seam.archive.path/'index.json').read_bytes())
    assert index['invocation_sha256']=='e'*64 and index['candidate']==seam.identity
    for number,observed in enumerate(seam.owner._result_pin):
        prefix='unit-%03d.' % number
        assert (seam.archive.path/(prefix+'result.json')).read_bytes()==seam.owner._results[number].data
        assert (seam.archive.path/(prefix+'stdout')).read_bytes()==b'unit stdout\n'
        assert (seam.archive.path/(prefix+'stderr')).read_bytes()==b''
        assert (seam.archive.path/(prefix+'command.json')).read_bytes()==m.project_unit_command(number,observed.name,observed.terminal)
    shutil.rmtree(seam.path)
    assert seam.owner.archive_binding==binding
    assert index['scope']['verification_completed_ns']<=binding['scope']['completed_ns']<=seam.owner._deadline
    assert not any(hasattr(row,'data') for row in seam.archive._files.values())


@pytest.mark.parametrize('case',['result','stdout','stderr','index','inventory','extra'])
def test_durable_preflight_rejects_changed_archive_before_parent_projection(tmp_path,monkeypatch,case):
    seam=ProcessSeam(tmp_path,monkeypatch);seam.owner.run();seam.owner.close()
    name={'result':'unit-000.result.json','stdout':'unit-000.stdout','stderr':'unit-000.stderr',
        'index':'index.json','inventory':'inventory.json','extra':'unexpected'}[case]
    path=seam.archive.path/name
    if path.exists():path.chmod(0o600)
    path.write_bytes(b'changed');path.chmod(0o400)
    from sumeragi_v2_release_scaling_selection_test import bootstrap_definitions
    with pytest.raises((m.ScalingPreflightError,seam.bootstrap.BootstrapError)):
        _=seam.owner.archive_binding


def test_durable_preflight_partial_write_and_expired_publication_withhold_binding(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch);original=seam.archive.write
    def write(name,raw,cap):
        result=original(name,raw,cap)
        if name=='index.json':seam.clock=seam.owner._deadline+1
        return result
    monkeypatch.setattr(seam.archive,'write',write)
    with pytest.raises(m.ScalingPreflightError):seam.owner.run()
    assert (seam.archive.path/'index.json').exists()
    assert seam.owner._archive_binding_json is None
    with pytest.raises(m.ScalingPreflightError):_=seam.owner.archive_binding
    seam.owner.release();assert seam.archive._phase=='closed'


def test_durable_preflight_unknown_terminal_blocks_archive_release(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch,'unknown_terminal')
    with pytest.raises(RuntimeError):seam.owner.run()
    with pytest.raises(m.ScalingPreflightError,match='natural terminal'):seam.owner.release()
    assert seam.archive._phase=='writing' and os.fstat(seam.archive._descriptor)
    assert not any(owner.closed for owner in seam.dependencies)


def test_durable_preflight_failed_final_check_closes_archive_only_after_natural_terminal(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch);seam.owner.run()
    def fail():raise OSError('archive final check')
    monkeypatch.setattr(seam.archive,'verify',fail)
    with pytest.raises(OSError):seam.owner.release()
    assert seam.archive._phase=='closed' and all(owner.closed for owner in seam.dependencies)
    with pytest.raises(m.ScalingPreflightError):_=seam.owner.archive_binding


@pytest.mark.parametrize('case',['stdout_hash','stderr_count','combined_cap','nonbytes'])
def test_durable_preflight_rejects_unobserved_or_truncated_stream_buffers(tmp_path,monkeypatch,case):
    from dataclasses import replace
    seam=ProcessSeam(tmp_path,monkeypatch);original=seam.owner._api.run_bounded
    def run(*args,**kwargs):
        result=original(*args,**kwargs);terminal=kwargs['observation'].terminal
        if case=='stdout_hash':terminal.stdout_sha256='0'*64
        elif case=='stderr_count':terminal.stderr_bytes=1
        elif case=='combined_cap':result.stdout=b'x'*(m.MAX_OUTPUT_BYTES+1)
        elif case=='nonbytes':result.stdout=bytearray(result.stdout)
        return result
    seam.owner._api=replace(seam.owner._api,run_bounded=run)
    with pytest.raises(m.ScalingPreflightError):seam.owner.run()
    assert len(seam.calls)==1 and not (seam.archive.path/'index.json').exists()
    assert not seam.owner._archive_units and seam.owner._archive_binding_json is None
    seam.owner.release()


def test_durable_preflight_partial_unit_write_retains_evidence_without_acceptance(tmp_path,monkeypatch):
    seam=ProcessSeam(tmp_path,monkeypatch);original=seam.archive.write
    def write(name,raw,cap):
        observed=original(name,raw,cap)
        if name=='unit-000.stdout':raise OSError('observed publication failure')
        return observed
    monkeypatch.setattr(seam.archive,'write',write)
    with pytest.raises(OSError):seam.owner.run()
    assert (seam.archive.path/'unit-000.stdout').read_bytes()==b'unit stdout\n'
    assert not (seam.archive.path/'index.json').exists()
    assert not seam.owner._archive_units and seam.owner._archive_binding_json is None
    seam.owner.release();assert seam.archive._phase=='closed'


def test_phase_driver_helper_import_reuses_source_counts_without_module_or_cache_mutation():
    existing=sys.modules['scaling_preflight_archive'];before=sys.pycache_prefix
    driver=load('phase_import_state_probe',ROOT/'pytests/scripts/run_scaling_preflight.py')
    try:
        assert driver.PHASE_COUNTS==m.PHASE_COUNTS
        assert sys.modules['scaling_preflight_archive'] is existing
        assert sys.pycache_prefix==before
    finally:
        sys.modules.pop('phase_import_state_probe',None)


def test_original_writer_archive_matches_portable_inspector_after_source_and_archive_relocation(tmp_path,monkeypatch):
    import shutil
    from scaling_preflight_archive import inspect_preflight_archive,capture_preflight_archive
    seam=ProcessSeam(tmp_path,monkeypatch);seam.owner.run();seam.owner.close()
    binding=seam.owner.archive_binding
    selected=tmp_path/'selected-source';selected.mkdir(mode=0o700)
    for name in seam.owner._inventory['sources']:
        path=selected/name;path.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        shutil.copy2(ROOT/name,path)
    inventory_path=selected/m.INVENTORY_PATH;inventory_path.write_bytes(seam.inventory_raw)
    context=dict(source_root=selected,candidate_identity=seam.identity,
        invocation_sha256='e'*64,collector_started_ns=binding['scope']['completed_ns']+1,timeout_seconds=600)
    original=inspect_preflight_archive(seam.archive.path,binding,**context)
    rows,directories=capture_preflight_archive(seam.archive.path,binding,**context)
    assert len(rows)==binding['inventory']['files'] and len(directories)==1
    copied=tmp_path/'relocated-bootstrap/scaling-preflight';copied.parent.mkdir(mode=0o700)
    shutil.copytree(seam.archive.path,copied)
    relocated_source=tmp_path/'relocated-source';shutil.copytree(selected,relocated_source)
    shutil.rmtree(seam.path);shutil.rmtree(selected)
    context['source_root']=relocated_source
    relocated=inspect_preflight_archive(copied,binding,**context)
    assert relocated==original
    # Portable data remains data: only the actual original owner exposes its
    # guarded original-terminal binding after these independent inspections.
    assert seam.owner.archive_binding==binding


def test_inventory_requires_exact_ci_parent_handoff_source_and_rejects_aliases():
    """Admit the one consumed CI source without permitting other CI paths."""
    path = 'ci/check_sumeragi_v2_multilane_release_inventory.sh'
    value = inventory()
    assert path in m.DATA_FIXTURES
    assert value['sources'][path] == hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
    assert m.decode_inventory(m.canonical(value)) == value
    missing = copy.deepcopy(value)
    del missing['sources'][path]
    with pytest.raises(m.ScalingPreflightError):
        m.decode_inventory(m.canonical(missing))
    for alias in ('ci/unselected.sh', 'ci/nested/check_sumeragi_v2_multilane_release_inventory.sh',
                  './' + path, '/'+path, 'ci//check_sumeragi_v2_multilane_release_inventory.sh',
                  'ci/../'+path, 'ci/./check_sumeragi_v2_multilane_release_inventory.sh',
                  path.upper(), path+'/', path+'.backup', path+'\n',
                  'ci\\check_sumeragi_v2_multilane_release_inventory.sh',
                  'ci/chéck_sumeragi_v2_multilane_release_inventory.sh'):
        changed = copy.deepcopy(value)
        changed['sources'][alias] = value['sources'][path]
        with pytest.raises(m.ScalingPreflightError):
            m.decode_inventory(m.canonical(changed))
