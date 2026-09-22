"""Source controls for original direct State/World publication custody."""
from __future__ import annotations
import importlib.util
import sys
from pathlib import Path
import pytest

_path = Path(__file__).with_name("sumeragi_v2_multilane_native_preparation_contract_test.py")
_spec = importlib.util.spec_from_file_location("direct_state_native_support", _path)
assert _spec and _spec.loader
_support = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = _support
_spec.loader.exec_module(_support)
fixture = _support.fixture


def test_direct_publication_accepts_original_owners(fixture):
    assert _support.validate(fixture) == ()


@pytest.mark.parametrize("path,symbol,old,new", [
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::new', 'phase: Some(Phase::Executing(block))', 'phase: None', id='retain-executing-original'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::into_executing', 'assert!(!self.released, "field was terminally released");', 'let _ = self.released;', id='capture-rejects-released-field'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::prepare_publication', 'self.phase = Some(Phase::Publishing(block.into_publication()));', 'drop(block);', id='install-original-before-preparation'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::prepare_publication', 'slot.prepare_publication();', 'let _ = slot;', id='prepare-actual-original-slot'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::publish_prepared', 'slot.publish_prepared();', 'let _ = slot;', id='publish-actual-original-slot'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::deref', 'assert!(!self.released, "field was terminally released");', 'let _ = self.released;', id='no-read-after-release'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::deref_mut', 'assert!(!self.released, "field was terminally released");', 'let _ = self.released;', id='no-write-after-release'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::release_writers', 'Some(Phase::Publishing(slot)) => slot.release_writers(),', 'Some(Phase::Publishing(slot)) => { let _ = slot; },', id='release-prepared-native-roles'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::release_writers', 'Some(Phase::Executing(block)) => block.release_writers(),', 'Some(Phase::Executing(block)) => { let _ = block; },', id='release-executing-native-roles'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::drop', 'self.release_writers();', 'let _ = self;', id='drop-releases-original'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'BlockField::json_serialize_to', 'self.deref().json_serialize_to(out)', 'Err(json::BoundedJsonError::Unsupported)', id='bounded-json-keeps-original-semantics'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'AggregatePublication::begin_preparation', 'self.assert_executing();', 'let _ = self;', id='preparation-is-one-shot'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'AggregatePublication::finish_preparation', 'assert_eq!(*self, Self::Preparing);', 'let _ = self;', id='all-preparations-must-complete'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'AggregatePublication::begin_publication', 'Self::Prepared,', 'Self::Preparing,', id='no-partial-publication'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'AggregatePublication::finish_publication', 'assert_eq!(*self, Self::Publishing);', 'let _ = self;', id='publication-completes-once'),
    pytest.param('crates/iroha_core/src/state/block_field.rs', 'AggregatePublication::release', '*self = Self::Released;', '*self = Self::Executing;', id='release-permanently-revokes-authority'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::new', 'target: block.inner,', 'target: other,', id='hash-original-target'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::executing', '!self.attempted && !self.released', '!self.released', id='hash-no-execution-after-attempt'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::try_prepare_publication', 'self.attempted = true;', 'self.attempted = false;', id='hash-attempt-is-terminal'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::try_prepare_publication', 'if block.reserved_tip.is_some()', 'if false', id='hash-rejects-unfilled-tip'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::try_prepare_publication', 'map.try_acquire_owned(work)', 'map.try_acquire_owned(other)', id='hash-acquires-exact-original-root'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::try_prepare_publication', '        self.phase = Some(Phase::Acquired(acquired));\n        let Some(Phase::Acquired(acquired))', '        drop(acquired);\n        let Some(Phase::Acquired(acquired))', id='hash-retains-actual-acquisition'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::try_prepare_publication', 'acquired.try_map_preserving_release(|a| a.validate())', 'acquired.try_map_preserving_release(|a| Ok(a))', id='hash-validates-original-predecessor'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::try_prepare_publication', 'slot.try_prepare()', 'other.try_prepare()', id='hash-reader-preparation-stays-in-caller'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::publish_prepared', 'if slot.is_prepared()', 'if true', id='hash-requires-complete-preparation'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::publish_prepared', 's.into_prepared().publish()', 'other.into_prepared().publish()', id='hash-publishes-original-slot'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::publish_prepared', '.store(self.height, Ordering::Release)', '.store(0, Ordering::Release)', id='hash-keeps-actual-height'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::publish_prepared', 'mv::BlockRetirement::release_writers(self);', 'let _ = self;', id='hash-unlocks-before-cache-read'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::release_writers', 's.abort_retaining()', 'other.abort_retaining()', id='hash-abort-retains-original-reader'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::release_writers', '_reader: reader,', '_reader: None,', id='hash-keeps-reader-notification'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::release_writers', 'published.release_deferred(|p| p.release())', 'other.release_deferred(|p| p.release())', id='hash-keeps-writer-notification'),
    pytest.param('crates/iroha_core/src/state/block_hash_field.rs', 'BlockHashField::drop', 'mv::BlockRetirement::release_writers(self);', 'let _ = self;', id='hash-drop-releases-before-cleanup'),
    pytest.param('crates/iroha_core/src/publication_lock.rs', 'DeferredPublicationFence::lock', 'guard: Some(self.mutex.lock()),', 'guard: Some(other.lock()),', id='fence-acquires-same-source'),
    pytest.param('crates/iroha_core/src/publication_lock.rs', 'DeferredPublicationFence::lock', 'releases: &mut self.releases,', 'releases: &mut other.releases,', id='fence-keeps-original-batch'),
    pytest.param('crates/iroha_core/src/publication_lock.rs', 'DeferredPublicationGuard::drop', 'guard.try_release_into(self.releases)', 'other.try_release_into(self.releases)', id='fence-unlock-retains-original-wake'),
    pytest.param('crates/iroha_core/src/publication_lock.rs', 'PublicationMutex::defer_notifications', 'releases: self.deferred_releases(),', 'releases: other.deferred_releases(),', id='fence-binds-source-at-construction'),
    pytest.param('crates/iroha_core/src/state.rs', 'WorldBlock::commit', 'self.prepare_publication();', 'let _ = &self;', id='world-prepares-all-before-any-write'),
    pytest.param('crates/iroha_core/src/state.rs', 'WorldBlock::commit', 'self.publish_prepared();', 'let _ = &self;', id='world-publishes-its-originals'),
    pytest.param('crates/iroha_core/src/state.rs', 'StateBlock::commit_inner', 'let mut commit_fence = self.state_ref.state_commit_lock.defer_notifications();', 'let mut commit_fence = other.defer_notifications();', id='state-retains-original-commit-wake'),
    pytest.param('crates/iroha_core/src/state.rs', 'StateBlock::commit_inner', 'let mut write_fence = self.state_write_lock.defer_notifications();', 'let mut write_fence = other.defer_notifications();', id='state-retains-original-write-wake'),
    pytest.param('crates/iroha_core/src/state.rs', 'StateBlock::commit_inner', 'let mut lifecycle_fence = self.state_ref.lane_lifecycle_lock.defer_notifications();', 'let mut lifecycle_fence = other.defer_notifications();', id='state-retains-original-lifecycle-wake'),
    pytest.param('crates/iroha_core/src/state.rs', 'StateBlock::commit_inner', '} = this.fields.as_mut().expect("original executing State");', '} = this.into_fields();', id='state-does-not-disarm-original-on-refusal'),
    pytest.param('crates/iroha_core/src/state.rs', 'StateBlock::commit_inner', 'world.prepare_publication();', 'let _ = &world;', id='state-prepares-world-before-visible-write'),
    pytest.param('crates/iroha_core/src/state.rs', 'StateBlock::commit_inner', '            committed_topology.prepare_publication();', '            let _ = &committed_topology;', id='state-prepares-last-runtime-owner'),
    pytest.param('crates/iroha_core/src/state.rs', 'StateBlock::commit_inner', 'block_hashes.publish_prepared();', 'let _ = &block_hashes;', id='state-publishes-original-history'),
    pytest.param('crates/iroha_core/src/state.rs', 'StateBlock::commit_world_overlay_for_testing', 'this.world.prepare_publication();', 'let _ = &this.world;', id='fixture-uses-same-attached-kernel'),
])
def test_direct_publication_rejects_custody_substitution(fixture, path, symbol, old, new):
    root, _, checker, _ = fixture
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, "method", symbol)
    assert len(items) == 1
    item = items[0]
    assert item.count(old) == 1
    owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0]) if item in owner]
    assert len(owners) == 1
    original = owners[0]
    assert source.count(original) == 1
    target.write_text(source.replace(original, original.replace(item, item.replace(old, new, 1), 1), 1))
    errors = _support.validate(fixture)
    assert any("executable relation" in error or "retires original ownership" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors
