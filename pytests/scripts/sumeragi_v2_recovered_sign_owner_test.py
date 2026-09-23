"""Current recovered Sign defining-owner positive and mutation checks."""
from __future__ import annotations

import ast
import importlib.util
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT/'crates/iroha_core/src/sumeragi/v2_worker_completion.rs'
SOURCE = SOURCE_PATH.read_text()
OWNER = 'PreparedRecoveredLifecycleSignCompletionV1'
CONSUMERS = (
    ('successor_recovery_lifecycle', '_successor_recovery_lifecycle_source_fidelity_errors'),
    ('successor_production_recovery', '_successor_production_recovery_source_fidelity_errors'),
)

@pytest.fixture(scope='module')
def checker():
    path=ROOT/'scripts/formal/check_sumeragi_v2_proof_ledger.py'
    spec=importlib.util.spec_from_file_location('parked_sign_owner_checker',path)
    module=importlib.util.module_from_spec(spec)
    sys.modules[spec.name]=module
    spec.loader.exec_module(module)
    return module

@pytest.fixture(params=CONSUMERS,ids=lambda value:value[0])
def probe(checker,request):
    suffix,name=request.param
    path=ROOT/f'scripts/formal/sumeragi_v2_proof_ledger_{suffix}_contracts.py'
    owner,=[n for n in ast.parse(path.read_text()).body if isinstance(n,ast.FunctionDef) and n.name==name]
    calls=[n for n in ast.walk(owner) if isinstance(n,ast.Expr) and isinstance(n.value,ast.Call)
           and isinstance(n.value.func,ast.Attribute) and n.value.func.attr=='extend'
           and any(isinstance(a,ast.Call) and isinstance(a.func,ast.Name)
                   and a.func.id=='_parked_recovered_sign_completion_owner_errors' for a in n.value.args)]
    assert len(calls)==1
    fn=ast.parse('def probe(worker_path, worker_source):\n    errors=[]\n    return errors\n').body[0]
    fn.body[-1:-1]=calls
    tree=ast.fix_missing_locations(ast.Module(body=[fn],type_ignores=[]))
    ns=dict(checker.__dict__)
    exec(compile(tree,str(path),'exec'),ns)
    return ns['probe']


def test_current_owner(probe):
    assert probe(SOURCE_PATH,SOURCE)==[]


def test_unrelated_apply_comment_is_not_authority(probe):
    old='/// Result of atomically returning one guarded deferred Apply to the worker FIFO.'
    assert SOURCE.count(old)==1
    assert probe(SOURCE_PATH,SOURCE.replace(old,'/// A different explanation of the next Apply item.'))==[]


MUTATIONS=(
    ('wrong_owner', 'impl PreparedRecoveredLifecycleSignCompletionV1 {',
     'impl ForeignCompletion {'),
    ('raw_result', '    guarded: Box<GuardedRecoveredLifecycleSignWorkerResultV1>,\n    queue:',
     '    pub guarded: Box<GuardedRecoveredLifecycleSignWorkerResultV1>,\n    queue:'),
    ('unchecked_projection', '        if !result.is_exact() {\n            return None;\n        }',
     '        if false { return None; }'),
    ('foreign_signature', '            signature: result.signature.clone(),',
     '            signature: foreign_signature.clone(),'),
    ('foreign_key', '            key: result.dispatch_key(),', '            key: foreign_key,'),
    ('foreign_payload', '            outbound_payload: result.outbound_payload.clone(),',
     '            outbound_payload: foreign_payload.clone(),'),
    ('ack_order', '        self.queue.acknowledge_recovered_lifecycle_sign(key);\n        self.guarded.acknowledge_after_publication();',
     '        self.guarded.acknowledge_after_publication();\n        self.queue.acknowledge_recovered_lifecycle_sign(key);'),
    ('ack_missing', '        self.queue.acknowledge_recovered_lifecycle_sign(key);', ''),
    ('ack_duplicate', '        self.queue.acknowledge_recovered_lifecycle_sign(key);',
     '        self.queue.acknowledge_recovered_lifecycle_sign(key);\n        self.queue.acknowledge_recovered_lifecycle_sign(key);'),
    ('exposed_result', 'impl PreparedRecoveredLifecycleSignCompletionV1 {',
     'impl PreparedRecoveredLifecycleSignCompletionV1 {\n    pub fn result(&self) {}'),
    ('wrong_transfer', '.transfer_recovered_lifecycle_sign_completion(\n                guarded.result().dispatch_key(),',
     '.transfer_recovered_lifecycle_sign_completion(\n                foreign_key,'),
    ('borrowed_ack', 'pub(in crate::sumeragi) fn acknowledge_after_publication(self) {\n        let key = self.guarded.result().dispatch_key();',
     'pub(in crate::sumeragi) fn acknowledge_after_publication(&self) {\n        let key = self.guarded.result().dispatch_key();'),
    ('guard_not_disarmed', '    fn acknowledge_after_publication(mut self) {\n        self.drop_guard.disarm();\n    }',
     '    fn acknowledge_after_publication(mut self) {}'),
)

@pytest.mark.parametrize(('name','old','new'),MUTATIONS,ids=[m[0] for m in MUTATIONS])
def test_owner_mutations_rejected(probe,name,old,new):
    assert SOURCE.count(old)==1,(name,SOURCE.count(old))
    assert probe(SOURCE_PATH,SOURCE.replace(old,new,1)),name


def test_foreign_neighbor_cannot_replace_projection(probe):
    old='pub(in crate::sumeragi) fn project_adapter_completion_authority('
    assert SOURCE.count(old)==1
    changed=SOURCE.replace(old,'pub(in crate::sumeragi) fn unreviewed_projection(',1)
    changed+='\nimpl ForeignCompletion {\n    pub(in crate::sumeragi) fn project_adapter_completion_authority(&self) {}\n}\n'
    assert probe(SOURCE_PATH,changed)


def test_abandonment_guard_cannot_leave_outputs_open(probe):
    old='impl Drop for RecoveredLifecycleSignCompletionDropGuardV1 {\n    fn drop(&mut self) {\n        if self.armed {\n            self.output_guard.close_admission_for_restart();\n        }\n    }\n}'
    assert SOURCE.count(old)==1
    changed=SOURCE.replace(old,'impl Drop for RecoveredLifecycleSignCompletionDropGuardV1 {\n    fn drop(&mut self) {}\n}',1)
    assert probe(SOURCE_PATH,changed)
