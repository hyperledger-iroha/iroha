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
        tmp_path, checker, {Path(contract.STATE), Path(contract.STORAGE)},
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
])
def test_membership_parser_rejects_different_generic_owner(declaration):
    checker = support().load_checker()
    assert checker._extract_rust_binding_items(
        declaration.replace("{ fn", "{\n fn"), "method", "Owner::publish",
    ) == ()


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
    row["required_tokens"].remove("previous: previous_block")
    assert any("reviewed tokens changed" in e for e in validate(fixture))


@pytest.mark.parametrize("owner,old,new,diagnostic", [
    ("STORAGE", "        block: TransactionsBlock<'storage>,", "        pub(crate) block: TransactionsBlock<'storage>,", "mutable authority"),
    ("STORAGE", "        pub(super) _guard:", "        pub(super) guard_removed:", "executable relation"),
    ("STORAGE", "            self,\n        ) -> Result<PreparedTransactionsBlock", "            &self,\n        ) -> Result<PreparedTransactionsBlock", "executable relation"),
    ("STORAGE", "let publication = self.admit_publication()?;", "let publication = MembershipPublication::Repeated; // let publication = self.admit_publication()?;", "executable relation"),
    ("STORAGE", "                block: self,", "                block: replacement,", "executable relation"),
    ("STORAGE", "previous_block.transactions == current_block.transactions", "previous_block.transactions != current_block.transactions", "executable relation"),
    ("STORAGE", "if expected_current_height != current_height", "if expected_current_height == current_height", "executable relation"),
    ("STORAGE", "previous: previous_block,", "previous: None,", "executable relation"),
    ("STORAGE", "fn publish(self)", "fn publish(&self)", "executable relation"),
    ("STORAGE", "MembershipPublication::Repeated => {", "MembershipPublication::Repeated => { block.latest_block_ref.store(None);", "executable relation"),
    ("STORAGE", "*height < current.height", "*height <= current.height", "executable relation"),
    ("STORAGE", "block.blocks_ref.insert(transaction, previous.height)", "block.blocks_ref.insert(transaction, current.height)", "executable relation"),
    ("STORAGE", "let changes_identity = !matches!", "block.validate_commit()?; let changes_identity = !matches!", "repeats admission"),
    ("STORAGE", "        predecessor_identity: Arc<()>,", "        pub(crate) predecessor_identity: Arc<()>,", "mutable authority"),
    ("STORAGE", "predecessor_identity: Arc::clone(&block._guard)", "predecessor_identity: Arc::new(())", "executable relation"),
    ("STORAGE", "Arc::ptr_eq(&guard, &self.predecessor_identity)", "true", "executable relation"),
    ("STORAGE", "storage.write_lock.try_lock()", "storage.write_lock.lock()", "executable relation"),
    ("STORAGE", "**block._guard = next_identity;", "// **block._guard = next_identity;", "executable relation"),
    ("STATE", "let tx_validate_result = transactions.prepare_commit();", "let tx_validate_result = transactions.validate_commit();", "misses or reorders"),
    ("STATE", "            transactions.publish();", "            // transactions.publish();", "misses or reorders"),
])
def test_membership_rejects_semantic_mutation(fixture, owner, old, new, diagnostic):
    root, helper, checker, _ = fixture
    helper.replace_once(root / getattr(checker.membership_contract, owner), old, new)
    errors = validate(fixture)
    assert any(diagnostic in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("earlier,later", [
    ("let tx_validate_result = transactions.prepare_commit();", "state_ref.apply_committed_autoscale_lane_geometry("),
    ("state_ref.apply_committed_autoscale_lane_geometry(", "transactions.publish();"),
    ("transactions.publish();", "canonical_runtime.commit();"),
])
def test_membership_rejects_publication_order_drift(fixture, earlier, later):
    root, helper, checker, _ = fixture
    helper.swap_ordered_once_after(root / checker.membership_contract.STATE,
                                   "fn commit_inner(", earlier, later)
    assert any("misses or reorders" in e for e in validate(fixture))


@pytest.mark.parametrize("anchor,old,new", [
    ("fn block_impl(", "self.released.guard(self.write_lock.lock())", "self.write_lock.lock()"),
    ("fn block_impl(", "_guard: guard", "_guard: replacement"),
    ("pub(crate) fn observe_predecessor(", "storage.released.guard(guard)", "guard"),
    ("pub(crate) fn observe_predecessor(", "storage.released.guard(guard)", "storage.released.poisoning_guard(guard)"),
    ("pub(crate) fn try_prepare_publication<", "let wait = storage.released.observe();", "let wait = other.released.observe();"),
    ("let installation = match admit(&self, storage)", "let wait = storage.released.observe();", "let wait = other.released.observe();"),
    ("pub(crate) fn try_prepare_publication<", "storage.released.guard(guard)", "guard"),
    ("pub(crate) fn try_prepare_publication<", "storage.released.guard(guard)", "storage.released.poisoning_guard(guard)"),
    ("pub(crate) fn try_prepare_publication<", "_guard: guard", "_guard: replacement"),
])
def test_membership_rejects_detached_or_misdirected_release(fixture, anchor, old, new):
    """Both advisory probes and retained writers must use their actual source."""
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.membership_contract.STORAGE, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors
