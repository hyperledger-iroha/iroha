"""Require one caller-owned State capture through refusal, unwind and cleanup."""
from __future__ import annotations
import importlib.util
import sys
from pathlib import Path
import pytest

_path = Path(__file__).with_name("sumeragi_v2_multilane_native_preparation_contract_test.py")
_spec = importlib.util.spec_from_file_location("state_capture_native_support", _path)
assert _spec and _spec.loader
_support = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = _support
_spec.loader.exec_module(_support)
fixture = _support.fixture


def test_state_capture_accepts_actual_aggregate_owners(fixture):
    assert _support.validate(fixture) == ()


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param('PreparedCarrier','parts: Option<PreparedCarrierFields<\'state>>,','parts: (),',id='complete-carrier-keeps-original-fields'),
    *[pytest.param('StateBlock::release_writers',f'fields.{field}.release_writers();',f'let _ = &fields.{field};',id=f'original-carrier-releases-{field}') for field in ('world','canonical_runtime','commit_topology','prev_commit_topology','lane_consensus_contexts','transactions')],
    pytest.param('PreparedCarrier::new','Self { parts: Some(parts) }','Self { parts: None }',id='carrier-keeps-real-original'),
    pytest.param('PreparedCarrier::into_parts','self.parts.take()','other.parts.take()',id='transfer-original-carrier-only'),
    pytest.param('RuntimeCapture','phase: RuntimePhase<\'state>,','phase: (),',id='runtime-keeps-original-phase'),
    pytest.param('RuntimeCapture','admission: Option<Admission>,','admission: Option<()>,',id='runtime-keeps-original-capacity'),
    pytest.param('runtime_capture_fields','$(if let Some(field) = self.$field.as_mut() { field.release_writers(); })+','$(let _ = &self.$field;)+',id='runtime-attached-releases-all'),
    pytest.param('runtime_capture_fields','$(if let Some(field) = self.$field.as_mut() { field.release(); })+','$(let _ = &self.$field;)+',id='runtime-partial-releases-all'),
    pytest.param('runtime_capture_fields','let cleanup = [$($field.1,)+];','let cleanup = [];',id='runtime-retains-original-notifications'),
    pytest.param('runtime_capture_fields','$($field: $field.0,)+','$($field: other.$field,)+',id='runtime-keeps-original-journals'),
    pytest.param('RuntimeCapture::try_capture','self.admission = Some(admit(original.inputs())?);','let _ = admit(original.inputs())?;',id='runtime-retains-admission-before-transfer'),
    pytest.param('RuntimeCapture::try_capture','self.phase = RuntimePhase::Capturing(original.into_slots());','drop(original);',id='runtime-keeps-slots-in-caller'),
    pytest.param('RuntimeCapture::try_capture','pending.capture();','let _ = pending;',id='runtime-captures-every-cell'),
    pytest.param('RuntimeCapture::release','self.complete = false;','self.complete = true;',id='runtime-release-revokes-transfer'),
    pytest.param('RuntimeCapture::release','RuntimePhase::Attached(original) => original.release(),','RuntimePhase::Attached(original) => { let _ = original; },',id='runtime-releases-refused-original'),
    pytest.param('RuntimeCapture::release','RuntimePhase::Capturing(pending) => pending.release(),','RuntimePhase::Capturing(pending) => { let _ = pending; },',id='runtime-releases-partial-original'),
    pytest.param('RuntimeCapture::drop','self.release();','let _ = &self.phase;',id='runtime-drop-keeps-joint-release'),
    pytest.param('RuntimeJournals::capture','pending.try_capture(admit)?;','let _ = admit;',id='standalone-runtime-shares-capture'),
    pytest.param('StateJournalCapture','world: Option<WorldCapture>,','world: (),',id='state-retains-opaque-world'),
    pytest.param('StateJournalCapture::try_capture','.capture()?;','.release();',id='state-captures-world'),
    pytest.param('StateJournalCapture::try_capture','.try_capture(|_| Ok::<(), std::convert::Infallible>(()))','.try_capture(|_| other_admission())',id='state-captures-original-runtime'),
    pytest.param('StateJournalCapture::try_capture','.try_capture()?;','.release();',id='state-captures-membership'),
    *[pytest.param('StateJournalCapture::release',f'{field}.release();',f'let _ = {field};',id=f'state-releases-{field}-before-cleanup') for field in ('world','runtime','transactions')],
    pytest.param('StateJournalCapture::into_components','assert!(self.complete, "original State capture did not complete");','let _ = self.complete;',id='state-rejects-partial-materialization'),
    pytest.param('StateJournalCapture::into_components','drop(cleanup);','std::mem::forget(cleanup);',id='state-retires-original-membership-event'),
    pytest.param('StateJournalCapture::drop','self.release();','let _ = &self.world;',id='state-drop-releases-all-originals'),
 ])
def test_state_capture_requires_original_custody(fixture, symbol, old, new):
    root, _, checker, _ = fixture
    bindings = (
        *checker.native_preparation_contract.STATE_CAPTURE_BINDINGS,
        *checker.native_preparation_contract.STATE_ACQUISITION_BINDINGS,
    )
    rows = [row for row in bindings if row[2] == symbol]
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
        owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0]) if item in owner]
        assert len(owners) == 1
        original = owners[0]
    else:
        original = item
    assert source.count(original) == 1
    target.write_text(source.replace(original, original.replace(item, item.replace(old, new, 1), 1), 1))
    errors = _support.validate(fixture)
    assert any(f"{symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors
