"""Keep actual State acquisition and construction in their original joint owners.

These offline source controls require the repository's reviewed-source fixture;
no Rust build, network, environment overrides, or mutable runtime hooks are used.
"""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

_path = Path(__file__).with_name("sumeragi_v2_multilane_native_preparation_contract_test.py")
_spec = importlib.util.spec_from_file_location("state_acquisition_native_support", _path)
assert _spec and _spec.loader
_support = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = _support
_spec.loader.exec_module(_support)
fixture = _support.fixture


def test_state_acquisition_accepts_actual_joint_owners(fixture):
    assert _support.validate(fixture) == ()


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param('runtime_cells', '$($field: Option<mv::cell::BlockAcquisitionSlot<\'state, $value>>,)+', '$($field: Option<CellBlock<\'state, $value>>,)+', id='all-inert-cell-slots-exist-before-initialization'),
    pytest.param('runtime_cells', 'Pending(PendingCells<\'state>),', 'Pending(Box<PendingCells<\'state>>),', id='pending-phase-adds-no-unadmitted-box'),
    pytest.param('runtime_cells', '$(pending.$field.as_mut().expect("original State Cell slot").initialize(mode);)+', '$(let _ = &pending.$field;)+', id='initialize-every-original-cell-slot'),
    pytest.param('runtime_cells', '$(if let Some(field) = pending.$field.as_mut() { field.release(); })+', '$(let _ = &pending.$field;)+', id='partial-cell-phase-releases-before-cleanup'),
    pytest.param('runtime_cells', '$(if let Some(field) = acquired.$field.as_mut() { field.release_writers(); })+', '$(let _ = &acquired.$field;)+', id='complete-cell-phase-releases-before-cleanup'),
    pytest.param('runtime_cells', 'self.world.release_writers();', 'let _ = &self.world;', id='completed-acquisition-releases-world'),
    pytest.param('runtime_cells', 'self.transactions.release_writers();', 'let _ = &self.transactions;', id='completed-acquisition-releases-membership'),
    pytest.param('runtime_cells', '$(self.$field.release_writers();)+', '$(let _ = &self.$field;)+', id='completed-acquisition-releases-all-runtime'),
    pytest.param('runtime_cells', 'let original = self;', 'let original = other;', id='finish-keeps-original-acquisition'),
    pytest.param('runtime_cells', 'assert!(original.complete, "original State acquisition did not complete");', 'let _ = original.complete;', id='finish-rejects-incomplete-acquisition'),
    pytest.param('runtime_cells', 'finish_runtime_acquisition(|| {', 'finish_runtime_acquisition(move || {', id='finish-borrows-original-owner'),
    pytest.param('runtime_cells', 'block_hashes: original.block_hashes.take().expect("original funded hash successor"),', 'block_hashes: other_hashes,', id='finish-retains-funded-original-hash-owner'),
    pytest.param('RuntimeBlockAcquisition', 'world: Option<WorldBlock<\'state>>,', 'world: Option<Box<WorldBlock<\'state>>>,', id='world-keeps-original-inline-owner'),
    pytest.param('RuntimeBlockAcquisition', 'transactions: Option<TransactionsBlock<\'state>>,', 'transactions: (),', id='pending-keeps-original-membership'),
    pytest.param('RuntimeBlockAcquisition::new', 'cells: CellPhase::new(target),', 'cells: CellPhase::Empty,', id='install-all-cell-slots-before-acquisition'),
    pytest.param('RuntimeBlockAcquisition::new', 'block_hashes: Some(block_hashes),', 'block_hashes: None,', id='pending-retains-original-admitted-hash-successor'),
    pytest.param('RuntimeBlockAcquisition::initialize', 'assert!(!self.started, "original State acquisition is one-shot");', 'let _ = self.started;', id='pending-cannot-retry-a-failed-owner'),
    pytest.param('RuntimeBlockAcquisition::initialize', 'self.target.world.block_and_revert()', 'self.target.world.block()', id='world-keeps-replacement-mode'),
    pytest.param('RuntimeBlockAcquisition::initialize', 'self.target.transactions.block_and_revert()', 'self.target.transactions.block()', id='membership-keeps-replacement-mode'),
    pytest.param('RuntimeBlockAcquisition::initialize', 'BlockMode::Replace', 'BlockMode::Ordinary', id='runtime-keeps-replacement-mode'),
    pytest.param('RuntimeBlockAcquisition::initialize', 'self.complete = true;', 'self.complete = false;', id='complete-only-after-all-original-acquisitions'),
    pytest.param('RuntimeBlockAcquisition::release', 'self.complete = false;', 'self.complete = true;', id='release-revokes-completion'),
    *[pytest.param('RuntimeBlockAcquisition::release', f'{field}.release_writers();', f'let _ = {field};', id=f'partial-acquisition-releases-{field}') for field in ('world', 'transactions')],
    pytest.param('RuntimeBlockAcquisition::release', 'self.cells.release();', 'let _ = &self.cells;', id='partial-acquisition-releases-every-cell'),
    pytest.param('RuntimeBlockAcquisition::drop', 'self.release();', 'let _ = &self.world;', id='pending-default-drop-is-joint'),
    pytest.param('AcquiredRuntimeBlock', 'fields: Option<AcquiredRuntimeBlockFields<\'state>>,', 'fields: AcquiredRuntimeBlockFields<\'state>,', id='completed-result-keeps-armed-original-fields'),
    pytest.param('AcquiredRuntimeBlock::drop', 'fields.release();', 'let _ = fields;', id='completed-result-drop-is-joint'),
    pytest.param('AcquiredRuntimeBlock::into_fields', 'self.fields.take()', 'other.fields.take()', id='completed-transfer-takes-only-original-fields'),
    pytest.param('State::acquire_canonical_runtime_block', 'pending.initialize(replacement);', 'pending.initialize(false);', id='canonical-acquisition-preserves-requested-mode'),
    pytest.param('State::acquire_canonical_runtime_block', 'drop(pending);', 'std::mem::forget(pending);', id='generation-retry-releases-original-aggregate'),
    pytest.param('State::acquire_canonical_runtime_block', 'pending.canonical_runtime().get(),\n                pending.world(),', 'self.canonical_runtime.view().get(),\n                pending.world(),', id='projection-uses-acquired-runtime-generation'),
    pytest.param('State::acquire_canonical_runtime_block', 'is_stable_state_view_generation(generation, self.state_view_generation())', 'is_stable_state_view_generation(generation, generation)', id='recheck-actual-generation-after-acquisition'),
    pytest.param('StateBlock', 'fields: Option<StateBlockFields<\'state>>,', 'fields: StateBlockFields<\'state>,', id='executing-owner-remains-armed'),
    pytest.param('StateBlock::from_fields', 'fields: Some(fields),', 'fields: None,', id='executing-owner-keeps-all-original-fields'),
    pytest.param('StateBlock::into_fields', 'self.fields.take()', 'other.fields.take()', id='executing-transfer-keeps-exact-originals'),
    pytest.param('StateBlock::drop', 'mv::BlockRetirement::release_writers(self);', 'let _ = &self.fields;', id='executing-drop-delegates-joint-retirement'),
    pytest.param('PreparedCarrier::drop', 'parts.state.release_writers();', 'let _ = &parts.state;', id='prepared-carrier-keeps-original-state-retirement-delegate'),
    pytest.param('State::construct_acquired_block', 'let mut original = Some(acquired);', 'let mut original = None;', id='metadata-preparation-keeps-original-acquisition'),
    pytest.param('State::construct_acquired_block', 'self.pipeline_ivm_prepared_cache.read().clone()', 'other.pipeline_ivm_prepared_cache.read().clone()', id='prepared-cache-comes-from-original-state'),
    pytest.param('State::construct_acquired_block', 'self.lane_compliance.read().clone()', 'None', id='policy-retains-original-compliance'),
    pytest.param('State::construct_acquired_block', 'finish_state_block_construction(|| {', 'finish_state_block_construction(move || {', id='finish-closure-borrows-all-owner-options'),
    pytest.param('State::construct_acquired_block', 'StateBlock::from_fields(StateBlockFields {', 'other_from_fields(StateBlockFields {', id='executing-owner-arms-before-caller-finish'),
    pytest.param('State::construct_acquired_block', 'finish.take().expect("original State finish continuation")(block)', 'finish.take().expect("original State finish continuation")(other)', id='caller-finishes-the-original-armed-state'),
    pytest.param('State::construct_acquired_block', 'let mut finish = Some(finish);', 'let mut finish = Some(other);', id='retain-original-allocation-convention'),
    pytest.param('State::block_with_owned_start_stages', 'self.construct_acquired_block(acquired, curr_block, Box::new)', 'self.construct_acquired_block(acquired, curr_block, other_box)', id='ordinary-keeps-original-single-box-convention'),
    pytest.param('State::try_merge_preexecution_block', 'self.construct_acquired_block(acquired, curr_block, core::convert::identity)', 'self.construct_acquired_block(acquired, curr_block, Box::new)', id='scratch-does-not-add-allocation'),
    pytest.param('State::block_and_revert_with_pristine_stage', 'self.construct_acquired_block(acquired, curr_block, core::convert::identity)', 'self.construct_acquired_block(acquired, curr_block, Box::new)', id='replacement-does-not-add-allocation'),
])
def test_state_acquisition_requires_original_custody(fixture, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [r for r in checker.native_preparation_contract.PREPARATION_OWNER_BINDINGS if r[2] == symbol]
    assert len(rows) == 1
    path, kind, _, _ = rows[0]
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, kind, symbol)
    assert len(items) == 1
    item = items[0]
    assert item.count(old) == 1, (symbol, old)
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    if kind == "method":
        owners = [o for o in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0]) if item in o]
        assert len(owners) == 1
        original = owners[0]
    else:
        original = item
    assert source.count(original) == 1
    target.write_text(source.replace(original, original.replace(item, item.replace(old, new, 1), 1), 1))
    errors = _support.validate(fixture)
    assert any(f"{symbol} missing executable relation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("mutation", [
    "metadata-declared-after-original", "finish-declared-after-original",
    "original-declared-before-metadata", "metadata-moved-into-closure",
    "clone-after-native-owner-transfer",
])
def test_state_acquisition_keeps_metadata_cleanup_after_physical_release(fixture, mutation):
    root, _, checker, _ = fixture
    symbol = "State::construct_acquired_block"
    path = "crates/iroha_core/src/state/state_block_construction.rs"
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, "method", symbol)
    assert len(items) == 1
    original = items[0]
    changed = original
    owner = "        let mut original = Some(acquired);\n"
    if mutation in ("metadata-declared-after-original", "finish-declared-after-original"):
        declaration = (
            "        let mut pipeline_ivm_prepared_cache;\n"
            if mutation == "metadata-declared-after-original"
            else "        let mut finish = Some(finish);\n"
        )
        assert changed.count(declaration) == changed.count(owner) == 1
        changed = changed.replace(declaration, "", 1).replace(owner, owner + declaration, 1)
    elif mutation == "original-declared-before-metadata":
        declaration = "        let mut finish = Some(finish);\n"
        assert changed.count(declaration) == changed.count(owner) == 1
        changed = changed.replace(owner, "", 1).replace(declaration, owner + declaration, 1)
    elif mutation == "metadata-moved-into-closure":
        old = 'pipeline: pipeline.take().expect("prepared State input"),'
        assert changed.count(old) == 1
        changed = changed.replace(old, 'pipeline: pipeline.expect("prepared State input"),', 1)
    else:
        old = 'pipeline: pipeline.take().expect("prepared State input"),'
        assert changed.count(old) == 1
        changed = changed.replace(old, 'pipeline: self.pipeline.clone(),', 1)
    assert source.count(original) == 1
    target.write_text(source.replace(original, changed, 1))
    errors = _support.validate(fixture)
    assert any(f"{symbol} missing executable relation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("path,old,new,expected", [
    *[pytest.param('crates/iroha_core/src/state/canonical_runtime/acquisition.rs',
                   f'    {field}: {value},', f'    {field}: (),',
                   'sole original four-cell invocation', id=f'actual-runtime-inventory-{field}')
      for field, value in (('commit_topology','Vec<PeerId>'), ('prev_commit_topology','Vec<PeerId>'),
                           ('lane_consensus_contexts','LaneConsensusContextsV1'), ('canonical_runtime','SnapshotNexusRuntime'))],
    pytest.param('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', '#[inline(never)]', '#[inline(always)]', 'outlined original-owner finishing', id='acquisition-finish-retains-bounded-stack-lifetime'),
    pytest.param('crates/iroha_core/src/state/state_block_construction.rs', '#[inline(never)]', '#[inline(always)]', 'outlined borrowed finishing', id='construction-finish-retains-bounded-stack-lifetime'),
])
def test_state_acquisition_keeps_actual_inventory_and_stack_phases(fixture, path, old, new, expected):
    root, _, _, _ = fixture
    target = root / path
    source = target.read_text()
    assert source.count(old) == 1, old
    target.write_text(source.replace(old, new, 1))
    errors = _support.validate(fixture)
    assert any(expected in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors
