"""Bind aggregate acquisition/abandonment to its actual generated field owners."""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

_path = Path(__file__).with_name("sumeragi_v2_multilane_native_preparation_contract_test.py")
_spec = importlib.util.spec_from_file_location("world_acquisition_native_support", _path)
assert _spec and _spec.loader
_support = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = _support
_spec.loader.exec_module(_support)
fixture = _support.fixture


def test_world_acquisition_accepts_actual_aggregate_owners(fixture):
    assert _support.validate(fixture) == ()


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param("WorldBlock", "fields: Option<WorldBlockFields<'world>>", "fields: WorldBlockFields<'world>", id="world-original-optional-transfer"),
    pytest.param("SetBlock", "fields: Option<SetBlockFields<'set>>", "fields: SetBlockFields<'set>", id="trigger-original-optional-transfer"),
    pytest.param("declare_world_acquisition", "$(pub(super) $prefix: Option<$prefix>,)*", "$(pub(super) $prefix: $prefix,)*", id="world-slots-own-original-prefix"),
    pytest.param("declare_world_acquisition", '$(self.$prefix.as_mut().expect("original field acquisition").initialize(mode);)*', '$(self.$prefix.as_mut().expect("original field acquisition").release();)*', id="world-initializes-original-slots"),
    pytest.param("declare_world_acquisition", '$(if let Some(field) = self.$prefix.as_mut() { field.release(); })*', '$(if let Some(field) = self.$prefix.as_mut() { let _ = field; })*', id="partial-prefix-unlocks-before-drop"),
    pytest.param("declare_world_acquisition", '$(if let Some(field) = self.$privacy.as_mut() { field.release(); })*', '$(if let Some(field) = self.$privacy.as_mut() { let _ = field; })*', id="partial-privacy-unlocks-before-drop"),
    pytest.param("declare_world_acquisition", '$(if let Some(field) = self.$suffix.as_mut() { field.release(); })*', '$(if let Some(field) = self.$suffix.as_mut() { let _ = field; })*', id="partial-suffix-unlocks-before-drop"),
    pytest.param("declare_world_acquisition", '$(fields.$prefix.release_writers();)*', '$(let _ = &fields.$prefix;)*', id="complete-prefix-releases-before-cleanup"),
    pytest.param("declare_world_acquisition", '$(fields.$privacy.release_writers();)*', '$(let _ = &fields.$privacy;)*', id="complete-privacy-releases-before-cleanup"),
    pytest.param("declare_world_acquisition", '$(fields.$suffix.release_writers();)*', '$(let _ = &fields.$suffix;)*', id="complete-suffix-releases-before-cleanup"),
    pytest.param("build_world_block_from_fields", '$($prefix: Some($state.$prefix.block_acquisition()),)*', '$($prefix: Some($state.$prefix.block()),)*', id="all-slots-exist-before-construction"),
    pytest.param("build_world_block_from_fields", 'pending.initialize($mode);', 'pending.initialize(mv::BlockMode::Ordinary);', id="world-retains-requested-mode"),
    pytest.param("World::block_and_revert", 'mv::BlockMode::Replace', 'mv::BlockMode::Ordinary', id="world-replacement-remains-replacement"),
    pytest.param("WorldBlock::drop", 'self.release_writers();', 'let _ = &self.fields;', id="world-drop-releases-before-field-cleanup"),
    pytest.param("trigger_acquisition", '$($field: Some(target.$field.block_acquisition()),)+', '$($field: Some(target.$field.block()),)+', id="trigger-inert-slots-before-any-constructor"),
    pytest.param("trigger_acquisition", '$(self.$field.as_mut().expect("original trigger slot").initialize(mode);)+', '$(self.$field.as_mut().expect("original trigger slot").initialize(BlockMode::Ordinary);)+', id="trigger-keeps-original-mode"),
    pytest.param("trigger_acquisition", '$(if let Some(field) = self.$field.as_mut() { field.release(); })+', '$(if let Some(field) = self.$field.as_mut() { let _ = field; })+', id="partial-trigger-releases-every-child"),
    pytest.param("trigger_acquisition", '$(fields.$field.release_writers();)+', '$(let _ = &fields.$field;)+', id="complete-trigger-releases-every-child"),
    pytest.param("Set::block_and_revert", 'mv::BlockMode::Replace', 'mv::BlockMode::Ordinary', id="trigger-replacement-remains-replacement"),
    pytest.param("SetBlock::drop", 'self.release_writers();', 'let _ = &self.fields;', id="trigger-drop-releases-before-field-cleanup"),
])
def test_world_acquisition_requires_joint_original_ownership(fixture, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [row for row in checker.native_preparation_contract.WORLD_ACQUISITION_BINDINGS
            if row[2] == symbol]
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
        owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0])
                  if item in owner]
        assert len(owners) == 1
        owner = owners[0]
    else:
        owner = item
    assert source.count(owner) == 1
    target.write_text(source.replace(owner, owner.replace(item, item.replace(old, new, 1), 1), 1))
    errors = _support.validate(fixture)
    assert any(f"{symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors
