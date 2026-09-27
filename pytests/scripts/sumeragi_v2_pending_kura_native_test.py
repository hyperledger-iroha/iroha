"""Pending-Kura recovery preserves actual Native publication through exact rollover.

Execute the real checker consumer statements with the reviewed source loader;
mutations target one real Rust item rather than reproducing contract assertions.
"""
from __future__ import annotations
import ast
import importlib.util
from pathlib import Path
import sys
import pytest

ROOT = Path(__file__).resolve().parents[2]

def load_checker():
    path = ROOT / 'scripts/formal/check_sumeragi_v2_proof_ledger.py'
    spec = importlib.util.spec_from_file_location('pending_kura_native_checker', path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module

def build_probe(checker):
    ns = dict(checker.__dict__)
    tail = Path(checker._lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors.__code__.co_filename)
    owner, = [n for n in ast.parse(tail.read_text()).body if isinstance(n, ast.FunctionDef) and n.name == '_lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors']
    source_loop, = [n for n in owner.body if isinstance(n, ast.For) and isinstance(n.target, ast.Tuple) and isinstance(n.target.elts[0], ast.Name) and (n.target.elts[0].id == 'name')]
    entries = ast.literal_eval(source_loop.iter)
    helpers = [n for n in owner.body if isinstance(n, ast.FunctionDef) and n.name in {'item', 'qualified_item', 'require_order', 'reject_tokens', 'require_tokens'}]
    assert len(helpers) == 5
    f = ast.parse('def probe(paths,sources):\n    errors=[]\n    return errors\n').body[0]
    f.body[-1:-1] = helpers + ast.parse('_successor_recovery_pending_kura_tail_source_fidelity_errors(paths,sources,errors,item,qualified_item,require_order,reject_tokens,require_tokens)').body
    exec(compile(ast.fix_missing_locations(ast.Module(body=[f], type_ignores=[])), str(tail), 'exec'), ns)
    sources = {}
    paths = {}
    errors = []
    for name, relative in entries:
        paths[name], sources[name] = checker._read_reviewed_rust_source(ROOT, relative, errors, 'pending-Kura source fixture')
    assert not errors, errors
    return (ns['probe'], paths, sources, ns)

def scope_probe(checker, scope):
    """Compile existing checker statements; assertions are not duplicated in tests."""
    probe, paths, sources, ns = build_probe(checker)
    tail = Path(checker._lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors.__code__.co_filename)
    tail_owner, = [n for n in ast.parse(tail.read_text()).body if isinstance(n, ast.FunctionDef) and n.name == '_lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors']
    helpers = [n for n in tail_owner.body if isinstance(n, ast.FunctionDef) and n.name in {'item', 'qualified_item', 'require_order', 'reject_tokens', 'require_tokens'}]
    source = Path(checker._successor_recovery_pending_kura_tail_source_fidelity_errors.__code__.co_filename)
    definitions = {n.name: n for n in ast.parse(source.read_text()).body if isinstance(n, ast.FunctionDef)}
    if scope == 'native':
        statements = ast.parse('_pending_kura_native_output_source_fidelity_errors(paths,sources,errors,item,qualified_item,require_order,reject_tokens)').body
    elif scope == 'runner':
        statements = ast.parse('_lifecycle_turn_driver_pending_kura_runner_source_fidelity_errors(paths,sources,errors,item,require_order,reject_tokens,require_tokens)').body
    elif scope == 'lifecycle':
        owner = definitions['_successor_recovery_pending_kura_tail_source_fidelity_errors']

        def index(name):
            return next((i for i, n in enumerate(owner.body) if isinstance(n, ast.Assign) and any((isinstance(t, ast.Name) and t.id == name for t in n.targets))))
        statements = owner.body[index('pending_lane'):index('missing_pending')]
    else:
        raise ValueError(scope)
    f = ast.parse('def scoped(paths,sources):\n    errors=[]\n    return errors\n').body[0]
    f.body[-1:-1] = helpers + statements
    exec(compile(ast.fix_missing_locations(ast.Module(body=[f], type_ignores=[])), str(source), 'exec'), ns)
    return (ns['scoped'], paths, sources)

def production_probe(checker):
    path = Path(checker._successor_production_source_fidelity_errors.__code__.co_filename)
    owner, = [n for n in ast.parse(path.read_text()).body if isinstance(n, ast.FunctionDef) and n.name == '_successor_production_source_fidelity_errors']
    helper, = [n for n in owner.body if isinstance(n, ast.FunctionDef) and n.name == 'require_order']
    assignment, = [n for n in ast.walk(owner) if isinstance(n, ast.Assign) and any((isinstance(t, ast.Name) and t.id == 'pending_loop' for t in n.targets))]
    branch, = [n for n in ast.walk(owner) if isinstance(n, ast.If) and isinstance(n.test, ast.Compare) and isinstance(n.test.left, ast.Name) and (n.test.left.id == 'pending_loop')]
    f = ast.parse('def production(pending_runner_path,pending_runner_source):\n    errors=[]\n    return errors\n').body[0]
    f.body[-1:-1] = [helper, assignment, branch]
    ns = dict(checker.__dict__)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[f], type_ignores=[])), str(path), 'exec'), ns)
    return ns['production']

@pytest.fixture(scope='module')
def checker():
    return load_checker()

@pytest.fixture(scope='module')
def scopes(checker):
    return {name:scope_probe(checker,name) for name in ('lifecycle','runner','native')}

@pytest.mark.parametrize('scope',('lifecycle','runner','native'))
def test_current_pending_kura_contracts(scopes,scope):
    probe,paths,sources=scopes[scope]
    assert probe(paths,sources)==[]

def test_whole_pending_kura_tail(checker):
    probe,paths,sources,_=build_probe(checker)
    assert probe(paths,sources)==[]

# Mutations are confined to exactly one parsed Rust item. This prevents an
# unrelated occurrence or a comment from acting as evidence for the changed owner.
MUTATIONS=(
 ('lifecycle','pending_lifecycle','prepare_lane_recovery','matches_installed_pending_kura_tip(expected)','matches_installed_pending_kura_tip(foreign)'),
 ('lifecycle','pending_lifecycle','prepare_lane_recovery','ProductionLifecyclePreActivationErrorV1::OwnershipMismatch','ProductionLifecyclePreActivationErrorV1::OutputClosed'),
 ('lifecycle','pending_lifecycle','prepare_lane_recovery','let _ = self.installed.take_genesis();',''),
 ('lifecycle','pending_lifecycle','activate_no_clock','lifecycle_live_clocks_are_unarmed()','clocks_are_running()'),
 ('lifecycle','pending_lifecycle','activate_no_clock','pending_kura_activation_status_snapshot()','successor_activation_status_snapshot()'),
 ('lifecycle','pending_lifecycle','activate_no_clock','activate_effect_completion_observer(observer)','activate_effect_completion_observer(foreign)'),
 ('lifecycle','pending_lifecycle','locally_ready_for_finalized_rollover','!self.launched.owner.has_recovered_lifecycle_outputs()','true'),
 ('lifecycle','pending_lifecycle','locally_ready_for_finalized_rollover','PendingKuraApplyRecoveryStage::Completed','PendingKuraApplyRecoveryStage::ApplicationDispatched'),
 ('lifecycle','pending_lifecycle','locally_ready_for_finalized_rollover','exactly_covers_finalization_work(&self.launched.owner.coordinator)','is_empty()'),
 ('lifecycle','pending_lifecycle','into_finalized_rollover','verify_published_store_marker_finalization_census()','skip_census()'),
 ('lifecycle','pending_lifecycle','into_finalized_rollover','finish_height(&receipt, &artifact)','finish_height(&foreign_receipt, &artifact)'),
 ('runner','pending_runner','run_pending_kura_lifecycle_height','prepare_lane_recovery::<V2RunnerError>(&mut setup_runner)','prepare_lane_recovery::<V2RunnerError>(&mut foreign_runner)'),
 ('runner','pending_runner','run_pending_kura_lifecycle_height','prepared.activate_no_clock(activation)?','prepared.activate_no_clock(foreign)?'),
 ('runner','pending_runner','run_pending_active_height','native.take_service_publication(services);',''),
 ('runner','pending_runner','run_pending_active_height','native.service_sources(services, Instant::now())','native.service_sources(foreign_services, Instant::now())'),
 ('runner','pending_runner','run_pending_active_height','super::preflight_finalized_native_rollover(executor, services, native)','super::preflight_finalized_native_rollover(executor, services, replacement)'),
 ('runner','pending_runner','run_pending_active_height','settle_recovered_lifecycle_output_for_no_clock_recovery(&mut active_runner)','skip_recovered_outputs(&mut active_runner)'),
 ('runner','pending_runner','run_pending_active_height','close_runner_ingress_for_finalized_drain(&mut active_runner, receiver)','close_runner_ingress_for_finalized_drain(&mut active_runner, foreign)'),
 ('runner','pending_runner','run_pending_active_height','DecidedLaneRecoveryIngressDrainMode::FinalizedClosedPrefix','DecidedLaneRecoveryIngressDrainMode::OpenPreflight'),
 ('runner','pending_runner','run_pending_active_height','ensure_closed_global_drained_cut()','ensure_open()'),
 ('runner','pending_runner','run_pending_active_height','ensure_closed_global_drained_cut()','ensure_closed_drained_cut()'),
 ('native','native_process','take_service_publication','if self.publication.is_none()','if true'),
 ('native','native_process','take_service_publication','settled: false','settled: true'),
 ('native','native_process','settle_pending_publication','self.publication = Some(publication);',''),
 ('native','native_process','settle_pending_publication','self.settle_published(&publication.published)','Ok(true)'),
 ('native','native_process','preflight_publication','Ok(publication.settled)','Ok(true)'),
 ('native','native_process','authenticate_publication','actual.certificate() != receipt.certificate()','false'),
 ('native','native_process','authenticate_publication','actual.artifact_hash() != receipt.artifact_hash()','false'),
 ('native','native_process','finalized_output_authority','.filter(|publication| publication.settled)','.filter(|_| true)'),
 ('native','native_process','complete_output_handoff','self.finalized_output_authority(receipt, artifact)?;',''),
 ('native','native_finalized_output','preflight_finalized_native_rollover','executor.durable_finality()','foreign.durable_finality()'),
 ('native','native_finalized_output','authenticate','!self.published.matches_state(state)','false'),
 ('native','native_finalized_output','authenticate','original.subject() != receipt.subject()','false'),
 ('native','native_finalized_output','rollover_finalized_height_outputs_for_lifecycle','successor.parent_commit_qc.as_ref() != Some(&artifact.commit_qc)','false'),
 ('native','native_finalized_output','rollover_finalized_height_outputs_for_lifecycle','!handoff.authorizes_immediate_successor(successor)','false'),
 ('native','worker_services','handoff_native_height_output_to_durable_reconstruction','authority.authenticate(&self.state, receipt, artifact)?;',''),
 ('native','worker_services','seal_native_height_output_handoff','authority.authenticate(&self.state, receipt, artifact)?;',''),
 ('native','launch','rollover_outputs','native,','foreign_native,'),
)

@pytest.mark.parametrize(('scope','source_name','name','old','new'),MUTATIONS,ids=[f'{row[2]}-{i}' for i,row in enumerate(MUTATIONS)])
def test_contract_rejects_owner_loss(checker,scopes,scope,source_name,name,old,new):
    probe,paths,sources=scopes[scope]
    source=sources[source_name]
    items=list(checker.rust_items(source,name))
    if name=='authenticate':
        items=[item for item in items if item.brace_context==(('impl','NativeFinalizedOutputAuthority','<',"'",'_','>'),)]
    assert len(items)==1,(name,len(items))
    item=items[0]
    assert item.source.count(old)==1,(name,old,item.source.count(old))
    assert source.count(item.source)==1
    mutated=source.replace(item.source,item.source.replace(old,new,1),1)
    assert probe(paths,{**sources,source_name:mutated}), (name,old,new)

def test_production_pending_loop(checker):
    path=ROOT/'crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs'
    assert production_probe(checker)(path,path.read_text())==[]

@pytest.mark.parametrize(('old','new'),(
    ('pending.prepare_lane_recovery::<V2RunnerError>(&mut setup_runner)','pending.prepare_lane_recovery::<V2RunnerError>(&mut foreign_runner)'),
    ('prepared.activate_no_clock(activation)?','prepared.activate_no_clock(foreign)?'),
    ('Some(successor.pending_activation)','Some(foreign_activation)'),
))
def test_production_pending_loop_rejects_foreign_handoff(checker,old,new):
    path=ROOT/'crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs'
    source=path.read_text()
    assert source.count(old)==1
    assert production_probe(checker)(path,source.replace(old,new,1))

def test_whole_pending_kura_gate_calls_native_contract(checker):
    probe,paths,sources,_=build_probe(checker)
    changed=sources['native_finalized_output'].replace('!self.published.matches_state(state)','false')
    assert changed!=sources['native_finalized_output']
    assert any('original State' in error for error in probe(paths,{**sources,'native_finalized_output':changed}))
