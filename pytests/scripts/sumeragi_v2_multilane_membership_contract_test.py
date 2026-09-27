"""Positive and mutation controls for retained transaction membership ownership."""

from __future__ import annotations

import ast
import importlib.util
import sys
from pathlib import Path

import pytest


def support():
    path = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")
    spec = importlib.util.spec_from_file_location("membership_test_support", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def validate(fixture):
    root, _, checker, models = fixture
    errors = []
    with checker._reviewed_rust_source_cache():
        checker.membership_contract.validate_membership_contract(
            root, models, errors, checker._rust_binding_item,
        )
    return tuple(errors)


@pytest.fixture
def fixture(tmp_path):
    helper = support()
    checker = helper.load_checker()
    contract = checker.membership_contract
    helper.copy_reviewed_source_fixture_with_includes(
        tmp_path, checker, {*(Path(path) for path, _, _, _ in contract.MEMBERSHIP_BINDINGS), Path(contract.STATE)},
    )
    result = tmp_path, helper, checker, helper.canonical_models()
    assert validate(result) == ()
    return result


def test_membership_accepts_actual_retained_owners(fixture):
    assert validate(fixture) == ()


def test_membership_is_connected_to_release_gate():
    checker = support().load_checker()
    tree = ast.parse(Path(checker.__file__).read_text(encoding="utf-8"))
    owners = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
    assert sum(
        isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
        and isinstance(n.func.value, ast.Name)
        and n.func.value.id == "membership_contract"
        and n.func.attr == "validate_membership_contract"
        for n in ast.walk(owners["_validate"])
    ) == 1
    assert any(
        isinstance(n, ast.Starred) and isinstance(n.value, ast.Attribute)
        and isinstance(n.value.value, ast.Name)
        and n.value.value.id == "membership_contract"
        and n.value.attr == "MEMBERSHIP_SOURCE_RELATIVES"
        for n in ast.walk(owners["source_manifest_sha256"])
    )


@pytest.mark.parametrize("declaration", [
    "impl Owner { fn publish(self) {} }",
    "impl<'storage> Owner<'storage> { fn publish(self) {} }",
    "impl<'a, T> Owner<'a, T> { fn publish(self) {} }",
    "impl Clone for Owner { fn publish(self) {} }",
    "impl<'a> Clone for Owner<'a> { fn publish(self) {} }",
    "impl<'a, T>\n    Owner<'a, T>\n{ fn publish(self) {} }",
    "impl<\n    'a,\n    T,\n>\n    Owner<\n        'a,\n        T,\n    >\n{ fn publish(self) {} }",
    "impl<'a>\n    Clone\n    for Owner<'a>\n{ fn publish(self) {} }",
])
def test_membership_parser_keeps_exact_generic_owner(declaration):
    checker = support().load_checker()
    assert len(checker._extract_rust_binding_items(
        declaration.replace("{ fn", "{\n fn"), "method", "Owner::publish",
    )) == 1


@pytest.mark.parametrize("declaration", [
    "impl<'a> OwnerImposter<'a> { fn publish(self) {} }",
    "impl<'a> OtherOwner<'a> { fn publish(self) {} }",
    "impl<'a> Trait<Owner<'a>> for Other { fn publish(self) {} }",
    "impl<'a> Trait for Other<Owner<'a>> { fn publish(self) {} }",
    "impl<'a, T>\n    OwnerImposter<'a, T>\n{ fn publish(self) {} }",
    "impl<'a>\n    Trait<Owner<'a>>\n    for Other\n{ fn publish(self) {} }",
    "impl<'a>\n    Trait\n    for Other<Owner<'a>>\n{ fn publish(self) {} }",
])
def test_membership_parser_rejects_different_generic_owner(declaration):
    checker = support().load_checker()
    assert checker._extract_rust_binding_items(
        declaration.replace("{ fn", "{\n fn"), "method", "Owner::publish",
    ) == ()


def test_membership_parser_keeps_multiline_method_in_its_original_impl():
    checker = support().load_checker()
    source = """impl<'a> Other<'a> {
    fn publish(self) { unrelated(); }
}
impl<'a, Admission, BindingAdmission>
    Owner<'a, Admission, BindingAdmission>
{
    fn publish(self) { original(); }
}
impl<'a> OwnerImposter<'a> {
    fn publish(self) { substitute(); }
}
"""
    items = checker._extract_rust_binding_items(source, "method", "Owner::publish")
    assert len(items) == 1
    assert "original();" in items[0]
    assert "unrelated();" not in items[0]
    assert "substitute();" not in items[0]


@pytest.mark.parametrize("model_index", [0, 1])
@pytest.mark.parametrize("symbol", [
    "TransactionsStorage", "TransactionsStorage::block_impl",
    "PreparedTransactionsBlock", "TransactionsBlock::prepare_commit",
    "DetachedTransactionsBlock::try_prepare_publication",
    "TransactionsBlock::admit_publication", "PreparedTransactionsBlock::publish",
    "DetachedTransactionsBlock", "PreparedTransactionsBlock::detach",
    "DetachedTransactionsBlock::observe_predecessor",
])
def test_membership_rejects_missing_delegated_owner(fixture, model_index, symbol):
    _, _, checker, models = fixture
    name = checker.membership_contract.MODELS[model_index]
    model = next(m for m in models if m["module"] == name)
    model["production_symbols"] = [b for b in model["production_symbols"]
                                   if b["symbol"] != symbol]
    assert any(f"owner {symbol}" in e for e in validate(fixture))


def test_membership_rejects_weakened_ledger(fixture):
    _, _, checker, models = fixture
    model = next(m for m in models if m["module"] == checker.membership_contract.MODELS[0])
    row = next(b for b in model["production_symbols"]
               if b["symbol"] == "TransactionsBlock::admit_publication")
    row["required_tokens"].remove("_previous: previous_block")
    assert any("reviewed tokens changed" in e for e in validate(fixture))


@pytest.mark.parametrize("owner,old,new,diagnostic", [
    pytest.param('STORAGE', "        block: TransactionsBlock<'storage>,", "        pub(crate) block: TransactionsBlock<'storage>,", 'mutable authority', id="STORAGE-        block: TransactionsBlock<'storage>,-        pub(crate) block: TransactionsBlock<'storage>,-mutable authority"),
    pytest.param('STORAGE', '        pub(super) _guard:', '        pub(super) guard_removed:', 'executable relation', id='STORAGE-        pub(super) _guard:-        pub(super) guard_removed:-executable relation'),
    pytest.param('STORAGE', '            self,\n        ) -> Result<PreparedTransactionsBlock', '            &self,\n        ) -> Result<PreparedTransactionsBlock', 'executable relation', id='STORAGE-            self,\n        ) -> Result<PreparedTransactionsBlock-            &self,\n        ) -> Result<PreparedTransactionsBlock-executable relation'),
    pytest.param('CAPTURE', 'let publication = block.admit_publication()?;', 'let publication = MembershipPublication::Repeated; // let publication = block.admit_publication()?;', 'executable relation', id='CAPTURE-let publication = block.admit_publication()?;-let publication = MembershipPublication::Repeated; // let publication = block.admit_publication()?;-executable relation'),
    pytest.param('CAPTURE', '            block,\n            publication,\n            next_identity,', '            replacement,\n            publication,\n            next_identity,', 'executable relation', id='CAPTURE-            block,\n            publication,\n            next_identity,-            block: replacement,\n            publication,\n            next_identity,-executable relation'),
    pytest.param('STORAGE', 'previous_block.transactions == current_block.transactions', 'previous_block.transactions != current_block.transactions', 'executable relation', id='STORAGE-previous_block.transactions == current_block.transactions-previous_block.transactions != current_block.transactions-executable relation'),
    pytest.param('STORAGE', 'if expected_current_height != current_height', 'if expected_current_height == current_height', 'executable relation', id='STORAGE-if expected_current_height != current_height-if expected_current_height == current_height-executable relation'),
    pytest.param('STORAGE', '_previous: previous_block,', '_previous: None,', 'executable relation', id='STORAGE-previous: previous_block,-previous: None,-executable relation'),
    pytest.param('STORAGE', 'fn publish(mut self)', 'fn publish(&self)', 'executable relation', id='STORAGE-fn publish(self)-fn publish(&self)-executable relation'),
    pytest.param('STORAGE', 'if !matches!(&self.publication, MembershipPublication::Repeated)', 'if true', 'executable relation', id='STORAGE-MembershipPublication::Repeated => None-MembershipPublication::Repeated => block.latest_block_ref.swap(None)-executable relation'),
    pytest.param('HISTORY', 'let len = if self.replacement', 'let len = if !self.replacement', 'executable relation', id='STORAGE-*height < current.height-*height <= current.height-executable relation'),
    pytest.param('HISTORY', 'work.try_insert_admitted(key, height,', 'work.try_insert_admitted(key, other_height,', 'executable relation', id='STORAGE-block.blocks_ref.insert(transaction, previous.height)-block.blocks_ref.insert(transaction, current.height)-executable relation'),
    pytest.param('STORAGE', 'self.publication_started = true;', 'self.block.validate_commit()?; self.publication_started = true;', 'repeats admission', id='STORAGE-let changes_identity = !matches!-block.validate_commit()?; let changes_identity = !matches!-repeats admission'),
    pytest.param('STORAGE', '        predecessor_identity: Identity,', '        pub(crate) predecessor_identity: Identity,', 'mutable authority', id='STORAGE-        predecessor_identity: Identity,-        pub(crate) predecessor_identity: Identity,-mutable authority'),
    pytest.param('STORAGE', 'predecessor_identity: block._guard.identity().clone()', 'predecessor_identity: Arc::new(())', 'executable relation', id='STORAGE-predecessor_identity: block._guard.identity().clone()-predecessor_identity: Arc::new(())-executable relation'),
    pytest.param('STORAGE', 'Identity::ptr_eq(&guard, &self.predecessor_identity)', 'true', 'executable relation', id='STORAGE-Identity::ptr_eq(&guard, &self.predecessor_identity)-true-executable relation'),
    pytest.param('STORAGE', 'storage.write_lock.try_lock()', 'storage.write_lock.lock()', 'executable relation', id='STORAGE-storage.write_lock.try_lock()-storage.write_lock.lock()-executable relation'),
    pytest.param('STORAGE', 'std::mem::swap(self.block._guard.identity_mut(), &mut self.next_identity)', 'std::mem::drop(Arc::clone(&self.next_identity))', 'executable relation', id='STORAGE-std::mem::replace(block._guard.identity_mut(), next_identity)-next_identity-executable relation'),
    pytest.param('STORAGE', '_guard.into_release()', 'other.into_release()', 'executable relation', id='STORAGE-_guard.into_release()-other.into_release()-executable relation'),
    pytest.param('STORAGE', '_tip: retired_tip,', '_tip: None,', 'executable relation', id='STORAGE-_tip: tip,-_tip: None,-executable relation'),
    pytest.param('STORAGE', '_identity: next_identity,', '_identity: Arc::new(()),', 'executable relation', id='STORAGE-_identity: identity,-_identity: Arc::new(()),-executable relation'),
    pytest.param('STORAGE', '_retirement: prepared.publish_prepared(),', '_retirement: replacement,', 'executable relation', id='STORAGE-_retirement: prepared.publish_prepared(),-_retirement: replacement,-executable relation'),
    pytest.param('STATE', 'let tx_validate_result = transactions.try_prepare_publication();', 'let tx_validate_result = transactions.validate_commit();', 'misses or reorders', id='STATE-let tx_validate_result = transactions.prepare_commit();-let tx_validate_result = transactions.validate_commit();-misses or reorders'),
    pytest.param('STATE', '} = this.fields.as_mut().expect("original executing State");', '} = this.fields.take().expect("original executing State");', 'misses or reorders', id='STATE-            membership_retirement = transactions.publish();-            transactions.publish();-misses or reorders'),
])
def test_membership_rejects_semantic_mutation(fixture, owner, old, new, diagnostic):
    root, helper, checker, _ = fixture
    helper.replace_once(root / getattr(checker.membership_contract, owner), old, new)
    errors = validate(fixture)
    assert any(diagnostic in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("earlier,later", [
    pytest.param('let tx_validate_result = transactions.try_prepare_publication();', 'state_ref.apply_committed_autoscale_lane_geometry(', id='let tx_validate_result = transactions.prepare_commit();-state_ref.apply_committed_autoscale_lane_geometry('),
    pytest.param('state_ref.apply_committed_autoscale_lane_geometry(', 'transactions.publish_prepared();', id='state_ref.apply_committed_autoscale_lane_geometry(-transactions.publish();'),
    pytest.param('transactions.publish_prepared();', 'canonical_runtime.publish_prepared();', id='transactions.publish();-canonical_runtime.commit();'),
    pytest.param('let mut commit_fence = self.state_ref.state_commit_lock.defer_notifications();', 'let mut this = self;', id='drop(_state_commit_lock);-drop(membership_retirement);'),
])
def test_membership_rejects_publication_order_drift(fixture, earlier, later):
    root, helper, checker, _ = fixture
    helper.swap_ordered_once_after(root / checker.membership_contract.STATE,
                                   "fn commit_inner(", earlier, later)
    assert any("misses or reorders" in e for e in validate(fixture))


@pytest.mark.parametrize("owner,anchor,old,new", [
    pytest.param('STORAGE', "fn attach_prepared<'a>", 'self.released.guard(guard)', 'guard', id='fn block_impl(-self.released.guard(self.write_lock.lock())-self.write_lock.lock()'),
    pytest.param('STORAGE', "fn attach_prepared<'a>", 'Some(history_slot::Slot::new(self, original))', 'Some(history_slot::Slot::new(self, replacement))', id='fn block_impl(-_guard: block::MembershipWriter::new(guard)-_guard: block::MembershipWriter::new(replacement)'),
    pytest.param('STORAGE', 'pub(crate) fn observe_predecessor(', 'storage.released.guard(guard)', 'guard', id='pub(crate) fn observe_predecessor(-storage.released.guard(guard)-guard'),
    pytest.param('STORAGE', 'pub(crate) fn observe_predecessor(', 'storage.released.guard(guard)', 'storage.released.poisoning_guard(guard)', id='pub(crate) fn observe_predecessor(-storage.released.guard(guard)-storage.released.poisoning_guard(guard)'),
    pytest.param('DETACHED', 'pub(crate) fn try_prepare<E>', 'let wait = target.released.observe();', 'let wait = other.released.observe();', id='pub(crate) fn try_prepare_publication<-let wait = storage.released.observe();-let wait = other.released.observe();'),
    pytest.param('DETACHED', 'let installation = match admit(self.original(), target)', 'let wait = target.released.observe();', 'let wait = other.released.observe();', id='let installation = match admit(&self, storage)-let wait = storage.released.observe();-let wait = other.released.observe();'),
    pytest.param('DETACHED', 'pub(crate) fn try_prepare<E>', 'target.released.guard(guard)', 'guard', id='pub(crate) fn try_prepare_publication<-storage.released.guard(guard)-guard'),
    pytest.param('DETACHED', 'pub(crate) fn try_prepare<E>', 'target.released.guard(guard)', 'target.released.poisoning_guard(guard)', id='pub(crate) fn try_prepare_publication<-storage.released.guard(guard)-storage.released.poisoning_guard(guard)'),
    pytest.param('DETACHED', 'pub(crate) fn try_prepare<E>', '_guard: self\n                    .writer\n                    .take()\n                    .expect("checked original publication writer")', '_guard: replacement', id='pub(crate) fn try_prepare_publication<-_guard: MembershipWriter::new(guard)-_guard: MembershipWriter::new(replacement)'),
])
def test_membership_rejects_detached_or_misdirected_release(fixture, owner, anchor, old, new):
    """Both advisory probes and retained writers must use their actual source."""
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.membership_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param('MembershipWriter::new', 'MembershipWriterPhase::Attached(guard)', 'MembershipWriterPhase::Attached(replacement)', id='MembershipWriter::new-MembershipWriterPhase::Attached(guard)-MembershipWriterPhase::Attached(replacement)'),
    pytest.param('MembershipWriter::identity', 'MembershipWriterPhase::Attached(guard)', 'MembershipWriterPhase::Released(guard)', id='MembershipWriter::identity-MembershipWriterPhase::Attached(guard)-MembershipWriterPhase::Released(guard)'),
    pytest.param('MembershipWriter::identity_mut', 'MembershipWriterPhase::Attached(guard)', 'MembershipWriterPhase::Released(guard)', id='MembershipWriter::identity_mut-MembershipWriterPhase::Attached(guard)-MembershipWriterPhase::Released(guard)'),
    pytest.param('MembershipWriter::release', 'guard.release_deferred(drop)', 'other.release_deferred(drop)', id='MembershipWriter::release-guard.release_deferred(drop)-other.release_deferred(drop)'),
    pytest.param('MembershipWriter::release', 'self.phase = Some(MembershipWriterPhase::Released(release));', 'drop(release);', id='MembershipWriter::release-self.phase = Some(MembershipWriterPhase::Released(release));-drop(release);'),
    pytest.param('MembershipWriter::release', 'other => self.phase = other,', 'other => drop(other),', id='MembershipWriter::release-other => self.phase = other,-other => drop(other),'),
    pytest.param('MembershipWriter::take_release', 'self.release();', '// self.release();', id='MembershipWriter::into_release-self.release();-// self.release();'),
    pytest.param('MembershipWriter::drop', 'self.release();', '// self.release();', id='MembershipWriter::drop-self.release();-// self.release();'),
    pytest.param('TransactionsBlock::capture_slot', 'MembershipCapturePhase::Attached(self)', 'MembershipCapturePhase::Empty', id='TransactionsBlock::capture_slot-MembershipCapturePhase::Attached(self)-MembershipCapturePhase::Empty'),
    pytest.param('TransactionsBlock::release_writers', 'self._guard.release();', '// self._guard.release();', id='TransactionsBlock::release_writers-self._guard.release();-// self._guard.release();'),
    pytest.param('TransactionsCaptureSlot::try_prepare', '!self.attempted && !self.released', '!self.attempted || !self.released', id='TransactionsCaptureSlot::try_prepare-!self.attempted && !self.released-!self.attempted || !self.released'),
    pytest.param('TransactionsCaptureSlot::try_prepare', 'self.attempted = true;', 'self.attempted = false;', id='TransactionsCaptureSlot::try_prepare-self.attempted = true;-self.attempted = false;'),
    pytest.param('TransactionsCaptureSlot::try_prepare', 'let publication = block.admit_publication()?;', 'let unused_identity = Arc::new(()); let publication = block.admit_publication()?;', id='TransactionsCaptureSlot::try_prepare-let publication = block.admit_publication()?;-let unused_identity = Arc::new(()); let publication = block.admit_publication()?;'),
    pytest.param('TransactionsCaptureSlot::try_capture', 'self.try_prepare()?;', 'let _ = self.try_prepare();', id='TransactionsCaptureSlot::try_capture-self.try_prepare()?;-let _ = self.try_prepare();'),
    pytest.param('TransactionsCaptureSlot::try_capture', 'prepared.block._guard.identity();', '// prepared.block._guard.identity();', id='TransactionsCaptureSlot::try_capture-prepared.block._guard.identity();-// prepared.block._guard.identity();'),
    pytest.param('TransactionsCaptureSlot::try_capture', 'prepared.detach_retaining()', 'other.detach_retaining()', id='TransactionsCaptureSlot::try_capture-prepared.detach_retaining()-other.detach_retaining()'),
    pytest.param('TransactionsCaptureSlot::try_capture', 'self.cleanup = Some(release);', 'drop(release);', id='TransactionsCaptureSlot::try_capture-self.cleanup = Some(release);-drop(release);'),
    pytest.param('TransactionsCaptureSlot::release', 'self.released = true;', 'self.released = false;', id='TransactionsCaptureSlot::release-self.released = true;-self.released = false;'),
    pytest.param('TransactionsCaptureSlot::release', 'MembershipCapturePhase::Attached(block) => Some(block),', 'MembershipCapturePhase::Attached(block) => None,', id='TransactionsCaptureSlot::release-MembershipCapturePhase::Attached(block) => block.release_writers(),-MembershipCapturePhase::Attached(block) => {},'),
    pytest.param('TransactionsCaptureSlot::release', '| MembershipCapturePhase::Published(prepared) => Some(&mut prepared.block),', '| MembershipCapturePhase::Published(prepared) => None,', id='TransactionsCaptureSlot::release-MembershipCapturePhase::Prepared(prepared) => prepared.block.release_writers(),-MembershipCapturePhase::Prepared(prepared) => {},'),
    pytest.param('TransactionsCaptureSlot::into_prepared', 'assert!(!self.released, "membership capture was terminally released");', '', id='TransactionsCaptureSlot::into_prepared-assert!(!self.released, "membership capture was terminally released");-'),
    pytest.param('TransactionsCaptureSlot::into_detached', 'assert!(!self.released, "membership capture was terminally released");', '', id='TransactionsCaptureSlot::into_detached-assert!(!self.released, "membership capture was terminally released");-'),
    pytest.param('TransactionsCaptureSlot::into_detached', 'self.phase = original;', 'drop(original);', id='TransactionsCaptureSlot::into_detached-self.phase = original;-drop(original);'),
    pytest.param('TransactionsCaptureSlot::drop', 'self.release();', '// self.release();', id='TransactionsCaptureSlot::drop-self.release();-// self.release();'),
])
def test_membership_capture_requires_original_caller_custody(fixture, symbol, old, new):
    """Admission/refusal and original physical retirement stay in the caller slot."""
    root, helper, checker, _ = fixture
    errors = []
    with checker._reviewed_rust_source_cache():
        item = checker._rust_binding_item(
            root, checker.membership_contract.CAPTURE, "method", symbol,
            "membership capture mutation", errors,
        )
    assert not errors, errors
    assert item is not None and item.count(old) == 1, (symbol, old)
    helper.replace_once(root / checker.membership_contract.CAPTURE, item, item.replace(old, new, 1))
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,symbol,old,new", [
    pytest.param("STORAGE", "PreparedTransactionsBlock::new", "publication_started: false", "publication_started: true", id="original-unstarted-owner"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::assert_unpublished", "!self.publication_started && !self.published", "!self.published", id="started-unwind-cannot-retry"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::publish_prepared", "self.publish_in_place();", "replacement.publish_in_place();", id="consuming-delegates-original-kernel"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::publish_in_place", "self.assert_unpublished();", "", id="reject-repeat-before-mutation"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::publish_in_place", "self.block._guard.identity();", "", id="original-writer-before-publication"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::publish_in_place", "self.publication_started = true;", "self.publication_started = false;", id="attempt-armed-before-map-work"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::publish_in_place", "match &self.publication", "match &replacement.publication", id="borrow-exact-admitted-action"),
    pytest.param("HISTORY", "Pending::advance", "if !self.replacement", "if self.replacement", id="advance-exact-previous-membership"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::publish_in_place", "std::mem::swap(self.block._guard.identity_mut(), &mut self.next_identity)", "*self.block._guard.identity_mut() = Arc::new(())", id="rotate-original-prepaid-identity"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::publish_in_place", "self.block.release_writers();", "", id="physical-release-with-retirement-retained"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::publish_in_place", "self.published = true;", "self.published = false;", id="complete-only-after-native-release"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::into_retirement", "self.published,", "true,", id="no-incomplete-retirement"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::into_retirement", "_publication: publication,", "_publication: MembershipPublication::Repeated,", id="retirement-retains-original-action"),
    pytest.param("STORAGE", "PreparedTransactionsBlock::detach_retaining", "self.assert_unpublished();", "", id="no-journal-after-publication-attempt"),
    pytest.param("CAPTURE", "MembershipWriter::into_release", "self.take_release()", "replacement.take_release()", id="original-release-kernel"),
    pytest.param("CAPTURE", "TransactionsCaptureSlot::publish_prepared", "prepared.publish_in_place();", "replacement.publish_in_place();", id="publish-under-original-caller"),
    pytest.param("CAPTURE", "TransactionsCaptureSlot::publish_prepared", "self.cleanup = Some(prepared.block._guard.take_release());", "drop(prepared.block._guard.take_release());", id="retain-exact-notification"),
    pytest.param("CAPTURE", "TransactionsCaptureSlot::publish_prepared", "self.phase = MembershipCapturePhase::Published(prepared);", "drop(prepared);", id="retain-published-action-payload"),
    pytest.param("CAPTURE", "TransactionsCaptureSlot::release", "if self.cleanup.is_none()", "if self.cleanup.is_some()", id="release-once-without-overwrite"),
    pytest.param("CAPTURE", "TransactionsCaptureSlot::executing", "!self.attempted && !self.released", "!self.released", id="no-read-after-attempt"),
    pytest.param("CAPTURE", "TransactionsCaptureSlot::executing_mut", "!self.attempted && !self.released", "!self.released", id="no-mutation-after-attempt"),
    pytest.param("CAPTURE", "TransactionsCaptureSlot::into_executing", "self.executing();", "", id="no-recovered-execution-authority"),
    pytest.param("CAPTURE", "TransactionsBlockField::new", "slot: block.capture_slot()", "slot: replacement.capture_slot()", id="wrap-only-original-block"),
    pytest.param("CAPTURE", "TransactionsBlockField::try_prepare_publication", "self.slot.try_prepare()", "replacement.slot.try_prepare()", id="prepare-original-slot"),
    pytest.param("CAPTURE", "TransactionsBlockField::into_capture", "self.slot.executing();", "", id="capture-transfer-rejects-attempted-owner"),
    pytest.param("CAPTURE", "TransactionsBlockField::deref_mut", "self.slot.executing_mut()", "replacement.slot.executing_mut()", id="original-execution-borrow"),
])
def test_membership_attached_publication_keeps_original_caller(fixture, owner, symbol, old, new):
    """Publication, partial panic and cleanup all remain in the original slot."""
    root, helper, checker, _ = fixture
    errors = []
    path = getattr(checker.membership_contract, owner)
    with checker._reviewed_rust_source_cache():
        item = checker._rust_binding_item(root, path, "method", symbol, "membership attached mutation", errors)
    assert not errors and item is not None and item.count(old) == 1, (errors, symbol, old)
    helper.replace_once(root / path, item, item.replace(old, new, 1))
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("index", [0, 1], ids=["inherent", "retirement-trait"])
def test_membership_attached_release_delegates_exact_original_slot(fixture, index):
    root, helper, checker, _ = fixture
    path = root / checker.membership_contract.CAPTURE
    items = checker._extract_rust_binding_items(path.read_text(), "method", "TransactionsBlockField::release_writers")
    assert len(items) == 2
    original = items[index]
    helper.replace_once(path, original, original.replace("self.slot.release();", "other.slot.release();"))
    assert any("terminal release delegates" in error for error in validate(fixture))


@pytest.mark.parametrize("method", ["publish_in_place", "publish_prepared"])
def test_membership_attached_rejects_borrowed_executing_publication(fixture, method):
    root, _, checker, _ = fixture
    path = root / checker.membership_contract.CAPTURE
    with path.open("a") as handle:
        handle.write(f"\nimpl TransactionsBlock<'_> {{\n    pub fn {method}(&mut self) {{}}\n}}\n")
    assert any("executing block exposes borrowed publication" in error for error in validate(fixture))


@pytest.mark.parametrize("owner,symbol,old,new", [
    pytest.param("DETACHED", "DetachedTransactionsBlock::publication_slot", "phase: Some(Phase::Original(self))", "phase: None", id="inert-original-installation"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::try_prepare", "!self.attempted && !self.released", "!self.released", id="one-attempt-only"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::try_prepare", "self.retryable = false;", "self.retryable = true;", id="panic-revokes-retry"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::try_prepare", "self.preflight_release = Some(", "drop(Some(", id="retain-actual-advisory-release"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::try_prepare", "admit(self.original(), target)", "admit(replacement, target)", id="admit-borrowed-original"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::try_prepare", "self.installation = Some(installation);", "drop(installation);", id="retain-original-installation"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::try_prepare", 'self.writer\n                .as_ref()\n                .expect("original publication writer")\n                .identity()', "replacement.identity()", id="final-identity-under-original-writer"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::try_prepare", "            current,\n            revert,\n            publication,\n            next_identity,", "            current,\n            revert,\n            publication,\n            next_identity: _,", id="reuse-exact-next-identity"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::try_prepare", "            publication,\n            next_identity,\n        )));", "            MembershipPublication::Repeated,\n            Arc::new(()),\n        )));", id="reuse-exact-pre-admitted-action"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::recover_original", "self.retryable && !self.released", "!self.released", id="no-retry-after-caught-panic"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::recover_original", "prepared.assert_unpublished();", "", id="no-recovery-after-publication-attempt"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::recover_original", "self.writer_release = Some(writer.into_release());", "drop(writer.into_release());", id="normal-refusal-retains-final-release"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::recover_original", "self.writer_release = Some(release);", "drop(release);", id="normal-abort-retains-prepared-release"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::release_writers", "self.retryable = false;", "self.retryable = true;", id="terminal-release-revokes-authority"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::release_writers", "self.writer_release = Some(writer.into_release());", "drop(writer.into_release());", id="terminal-release-retains-notification"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::release_writers", "prepared.block.release_writers();", "", id="terminal-release-frees-prepared-physical-writer"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::into_prepared", "self.complete && !self.released", "self.complete || !self.released", id="complete-live-original-transfer"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::into_prepared", "preflight_release: self.preflight_release.take()", "preflight_release: None", id="transfer-actual-preflight-event"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::into_cleanup", "self.released && self.phase.is_none()", "self.released", id="cleanup-only-after-original-recovery"),
    pytest.param("DETACHED", "DetachedTransactionsPublicationSlot::drop", "self.release_writers();", "", id="drop-physical-first"),
    pytest.param("STORAGE", "PreparedDetachedTransactionsBlock::abort", "_preflight_release: preflight_release", "_preflight_release: None", id="abort-retains-observation-event"),
    pytest.param("STORAGE", "PreparedDetachedTransactionsBlock::publish", "_preflight_release: preflight_release", "_preflight_release: None", id="publish-retains-observation-event"),
    pytest.param("STORAGE", "DetachedTransactionsBlock::try_prepare_publication", "Err((original, error, slot.into_cleanup()))", "Err((original, error, Default::default()))", id="consuming-adapter-preserves-refusal-cleanup"),
])
def test_membership_detached_preparation_keeps_original_caller(fixture, owner, symbol, old, new):
    """Real acquisition, ordinary retry and terminal retirement have one owner."""
    root, helper, checker, _ = fixture
    path = getattr(checker.membership_contract, owner)
    errors = []
    with checker._reviewed_rust_source_cache():
        item = checker._rust_binding_item(root, path, "method", symbol, "membership retained mutation", errors)
    assert not errors and item is not None and item.count(old) == 1, (errors, symbol, old)
    helper.replace_once(root / path, item, item.replace(old, new, 1))
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,symbol,old,new", [
    ('STORAGE', 'PreparedTransactionsBlock::publish_in_place', 'self.is_physically_prepared()', 'true'),
    ('STORAGE', 'PreparedTransactionsBlock::publish_in_place', '.store(next - 1, Ordering::Release)', '.store(next, Ordering::Release)'),
    ('STORAGE', 'PreparedTransactionsBlock::publish_in_place', '.store(next, Ordering::Release)', '.store(next - 1, Ordering::Release)'),
    ('STORAGE', 'PreparedTransactionsBlock::publish_in_place', 'self.retired_tip = self.block.latest_block_ref.swap(Some(current.clone()));', 'drop(self.block.latest_block_ref.swap(Some(current.clone())));'),
    ('STORAGE', 'PreparedTransactionsBlock::try_prepare_physical', '.map_err(TransactionsBlockError::MembershipAdmission)?', '.unwrap_or(())'),
    ('CAPTURE', 'TransactionsCaptureSlot::try_prepare', '.next_identity();', '.next_identity().clone();'),
    ('CAPTURE', 'TransactionsBlockField::recover_preparation', 'original.predecessor.clone()', 'other.predecessor.clone()'),
    ('CAPTURE', 'MembershipWriter::into_writer_release', 'assert!(self.history.is_none(), "observation-only writer");', ''),
    ('CAPTURE', 'MembershipWriter::release', 'history.release();', 'drop(history);'),
    ('HISTORY_SLOT', 'Slot::prepare', 'self.target.blocks.try_acquire_owned_retained(work)', 'other.blocks.try_acquire_owned_retained(work)'),
    ('HISTORY_SLOT', 'Slot::prepare', 'self.phase = Some(Phase::Original(work));', 'drop(work);'),
    ('HISTORY_SLOT', 'Slot::prepare', '            self.phase = Some(Phase::Acquired(acquired));\n        }', '            drop(acquired);\n        }'),
    ('HISTORY_SLOT', 'Slot::prepare', '                    self.phase = Some(Phase::Acquired(acquired));', '                    drop(acquired);'),
    ('HISTORY_SLOT', 'Slot::prepare', 'acquired.try_map_preserving_release(|a| a.validate())', 'acquired.try_map_preserving_release(|a| Ok(a))'),
    ('HISTORY_SLOT', 'Slot::prepare', 'self.target.blocks.observe_reader_release()', 'self.target.released.observe()'),
    ('HISTORY_SLOT', 'Slot::prepare', 'self.prepared = true;', 'self.prepared = false;'),
    ('HISTORY_SLOT', 'Slot::publish', 'assert!(self.prepared, "original prepared membership history");', ''),
    ('HISTORY_SLOT', 'Slot::publish', 'self.writer_release = Some(writer);', 'drop(writer);'),
    ('HISTORY_SLOT', 'Slot::release', 'self.reader_release = reader;', 'drop(reader);'),
    ('HISTORY_SLOT', 'Slot::recover', 'pending.work = Some(work);', 'pending.work = Some(other);'),
    ('HISTORY_SLOT', 'Slot::recover_detached', 'self.loan_release = pending.release_loan();', 'drop(pending.release_loan());'),
    ('HISTORY_SLOT', 'Slot::cleanup', '_pending: pending,', '_pending: None,'),
    ('HISTORY_SLOT', 'Slot::cleanup', '_loan: self.loan_release.take(),', '_loan: None,'),
    ('HISTORY', 'Pending::advance', 'if self.work.is_none()', 'if true'),
    ('HISTORY', 'Pending::advance', 'let key = batch.as_slice()[self.next];', 'let key = batch.as_slice()[0];'),
    ('HISTORY', 'Pending::advance', 'self.next += 1;', 'self.next = 0;'),
    ('HISTORY', 'Pending::advance', 'self.baseline = Some(writer.predecessor().retain());', 'self.baseline = Some(other.predecessor().retain());'),
    ('HISTORY', 'Pending::release_loan', 'self.predecessor.loaned.store(false, Ordering::Release);', 'self.predecessor.loaned.store(true, Ordering::Release);'),
    ('DETACHED', 'DetachedTransactionsPublicationSlot::try_prepare', 'MembershipAdmissionError::Busy(wait) => PublicationPreparationError::Busy(wait)', 'MembershipAdmissionError::Busy(wait) => PublicationPreparationError::Changed'),
    ('DETACHED', 'DetachedTransactionsPublicationSlot::try_prepare', '.history = Some(history_slot::Slot::new(target, history));', '.history = Some(history_slot::Slot::new(target, other));'),
])
def test_membership_current_history_preserves_original_physical_authority(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    errors = []
    path = getattr(checker.membership_contract, owner)
    item = checker._rust_binding_item(root, path, "method", symbol, "current membership mutation", errors)
    assert item is not None and errors == []
    assert item.count(old) == 1, (symbol, old)
    helper.replace_once(root / path, item, item.replace(old, new, 1))
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("must have one" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("owner,method,attribute", [
    ("STORAGE", "TransactionsStorage::block_impl", '#[cfg(any(test, feature = "iroha-core-tests", feature = "bench"))]'),
    ("STORAGE", "PreparedTransactionsBlock::publish", '#[cfg(test)]'),
])
@pytest.mark.parametrize("replacement", ["", "// {attribute}", "/* {attribute} */"])
def test_membership_fixture_helpers_never_become_production(fixture, owner, method, attribute, replacement):
    root, _, checker, _ = fixture
    errors = []
    path = root / getattr(checker.membership_contract, owner)
    item = checker._rust_binding_item(root, str(path.relative_to(root)), "method", method, "fixture gate", errors)
    assert item is not None and errors == []
    source = path.read_text()
    start = source.index(item)
    cut = source.rfind(attribute, 0, start)
    assert cut >= 0
    path.write_text(source[:cut] + source[cut:].replace(attribute, replacement.format(attribute=attribute), 1))
    assert any("fixture-only owner became production" in error for error in validate(fixture))


@pytest.mark.parametrize("old,new", [
    ("TransactionsBlockError::MembershipAdmission(\n                    storage_transactions::MembershipAdmissionError::Busy(_))", "TransactionsBlockError::MembershipAdmission(_)"),
    ("membership_retry = Some(transactions.recover_preparation());", "membership_retry = None;"),
    ("membership_target.retain_preparation(original);", "other.retain_preparation(original);"),
    ("Ok(())\n        }));\n        if let Some(original) = membership_retry", "Ok(())\n        });\n        if let Some(original) = membership_retry"),
])
def test_membership_busy_retry_keeps_original_until_siblings_retire(fixture, old, new):
    root, helper, checker, _ = fixture
    path = root / checker.membership_contract.STATE
    source = path.read_text()
    assert source.count(old) == 1, old
    helper.replace_once(path, old, new)
    assert any("commit_inner" in error for error in validate(fixture))
