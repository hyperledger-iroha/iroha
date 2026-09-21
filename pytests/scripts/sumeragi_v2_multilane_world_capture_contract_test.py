"""Require caller-owned capture slots and release ordering for World/TriggerSet."""
from __future__ import annotations
import importlib.util
import sys
from pathlib import Path
import pytest

_path = Path(__file__).with_name("sumeragi_v2_multilane_native_preparation_contract_test.py")
_spec = importlib.util.spec_from_file_location("world_capture_native_support", _path)
assert _spec and _spec.loader
_support = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = _support
_spec.loader.exec_module(_support)
fixture = _support.fixture


def test_world_capture_accepts_actual_aggregate_owners(fixture):
    assert _support.validate(fixture) == ()


@pytest.mark.parametrize("owner,symbol,old,new", [
    pytest.param('World','declare_world_capture','$(if let Some(field) = self.$prefix.as_mut() { field.release(); })*','$(if let Some(field) = self.$prefix.as_mut() { let _ = field; })*', id='prefix-unlocks-before-drop'),
    pytest.param('World','declare_world_capture','$(self.$prefix.as_mut().expect("original World capture slot").capture()?;)*','$(let _ = &self.$prefix;)*', id='prefix-captures-every-original'),
    pytest.param('World','capture_world_fields','$(pending.$prefix = Some($prefix.into_capture());)*','$(pending.$prefix = None;)*', id='prefix-retains-inert-slot'),
    pytest.param('World','declare_world_capture','$(if let Some(field) = self.$privacy.as_mut() { field.release(); })*','$(if let Some(field) = self.$privacy.as_mut() { let _ = field; })*', id='privacy-unlocks-before-drop'),
    pytest.param('World','declare_world_capture','$(self.$privacy.as_mut().expect("original World capture slot").capture()?;)*','$(let _ = &self.$privacy;)*', id='privacy-captures-every-original'),
    pytest.param('World','capture_world_fields','$(pending.$privacy = Some($privacy.into_capture());)*','$(pending.$privacy = None;)*', id='privacy-retains-inert-slot'),
    pytest.param('World','declare_world_capture','$(if let Some(field) = self.$suffix.as_mut() { field.release(); })*','$(if let Some(field) = self.$suffix.as_mut() { let _ = field; })*', id='suffix-unlocks-before-drop'),
    pytest.param('World','declare_world_capture','$(self.$suffix.as_mut().expect("original World capture slot").capture()?;)*','$(let _ = &self.$suffix;)*', id='suffix-captures-every-original'),
    pytest.param('World','capture_world_fields','$(pending.$suffix = Some($suffix.into_capture());)*','$(pending.$suffix = None;)*', id='suffix-retains-inert-slot'),
    pytest.param('World','capture_world_fields','pending.capture().map_err(widen_error)?;','let _ = &pending;', id='capture-all-before-notifications'),
    pytest.param('World','capture_world_fields','$admit(&$original)','$admit(&other)', id='admit-original-complete-world'),
    pytest.param('World','capture_world_fields','fill_world_capture(|| {','fill_world_capture(move || {', id='fill-borrows-original-aggregate'),
    pytest.param('World','capture_world_fields','let mut extras = None;','let mut extras = None; drop(admission);', id='admission-outlives-retained-payloads'),
    pytest.param('World','capture_world_fields','extras = Some((dataspace_catalog, external_event_buf));','drop((dataspace_catalog, external_event_buf));', id='retain-original-extras'),
    pytest.param('World','capture_world_fields','let fields = finish_world_capture(|| {','let fields = finish_world_capture(|| { drop(extras.take());', id='extras-outlive-wrapper-materialization'),
    pytest.param('World','retain_field','|target: &World| &target.$field','|target: &World| &other.$field', id='retained-original-target'),
    pytest.param('World','WorldBlock::try_detach_journals','with_world_overlay_fields!(capture_world_fields, self, admit)','with_world_overlay_fields!(capture_world_fields, other, admit)', id='capture-sole-world-inventory'),
    pytest.param('World','StorageCaptureSlot::release','BlockCapture::release(self);','let _ = self;', id='release-StorageCaptureSlot'),
    pytest.param('World','StorageCaptureSlot::retain','let (journal, cleanup) = self.into_detached();','let (journal, cleanup) = other.into_detached();', id='original-journal-StorageCaptureSlot'),
    pytest.param('World','CellCaptureSlot::release','BlockCapture::release(self);','let _ = self;', id='release-CellCaptureSlot'),
    pytest.param('World','CellCaptureSlot::retain','let (journal, cleanup) = self.into_detached();','let (journal, cleanup) = other.into_detached();', id='original-journal-CellCaptureSlot'),
    pytest.param('World','SetBlockCapture::capture','self.try_capture(|_| Ok::<(), Infallible>(()))','other.try_capture(|_| Ok::<(), Infallible>(()))', id='nested-original-trigger-slot'),
    pytest.param('World','SetBlockCapture::release','Self::release(self);','let _ = self;', id='nested-trigger-release-all'),
    pytest.param('Set','capture_fields','$(if let Some(field) = self.$field.as_mut() { field.release(); })+','$(if let Some(field) = self.$field.as_mut() { let _ = field; })+', id='trigger-releases-every-original-child'),
    pytest.param('Set','capture_fields','admission: Option<Admission>,','admission: Option<()>,', id='trigger-keeps-original-admission'),
    pytest.param('Set','capture_fields','let cleanup = SetCaptureCleanup([$(Some($field.1),)+]);','let cleanup = SetCaptureCleanup::default();', id='trigger-keeps-original-notifications'),
    pytest.param('Set','capture_fields','$($field: $field.0,)+','$($field: other.$field,)+', id='trigger-keeps-original-journals'),
    pytest.param('Set','SetBlockCapture','cleanup: SetCaptureCleanup,','cleanup: (),', id='nested-cleanup-remains-owned'),
    pytest.param('Set','SetBlockCapture::try_capture','let admission = admit(original).map_err(DetachError::Admission)?;','let admission = admit(&other).map_err(DetachError::Admission)?;', id='trigger-admits-original-block'),
    pytest.param('Set','SetBlockCapture::try_capture','pending.capture();','drop(pending);', id='trigger-captures-all-before-transfer'),
    pytest.param('Set','SetBlockCapture::try_capture','self.cleanup = cleanup;','drop(cleanup);', id='trigger-defers-original-wakes-through-world'),
    pytest.param('Set','SetBlockCapture::release','CapturePhase::Capturing(pending) => pending.release(),','CapturePhase::Capturing(pending) => { let _ = pending; },', id='partial-trigger-unlocks-before-retirement'),
    pytest.param('Set','SetBlockCapture::drop','self.release();','let _ = &self.phase;', id='trigger-drop-terminal-release'),
 ])
def test_world_capture_requires_original_custody(fixture, owner, symbol, old, new):
    root, _, checker, _ = fixture
    path = "crates/iroha_core/src/state/world_journals.rs" if owner == "World" else "crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs"
    rows = [row for row in checker.native_preparation_contract.CORE_CAPTURE_BINDINGS if row[0] == path and row[2] == symbol]
    assert len(rows) == 1
    _, kind, _, _ = rows[0]
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
