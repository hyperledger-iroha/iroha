"""Bind membership admission and consuming publication to their actual owners.

Used by the multilane release checker; no environment configuration is required.
These structural obligations do not qualify the unfinished State/Apply publisher.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable

from sumeragi_v2_multilane_geometry_evidence_contract import _code


STATE = "crates/iroha_core/src/state.rs"
STORAGE = "crates/iroha_core/src/state/storage_transactions.rs"
MODELS = (
    "SumeragiV2NativeApplicationEvidence",
    "SumeragiV2AutonomousReservationCarrier",
)
MEMBERSHIP_BINDINGS = (
    (STORAGE, "struct", "TransactionsStorage", (
        "write_lock: Mutex<Arc<()>>", "released: concread::release::ReleaseNotification",
    )),
    (STORAGE, "method", "TransactionsStorage::block_impl", (
        "let guard = self.released.guard(self.write_lock.lock());",
        "_guard: guard",
    )),
    (STORAGE, "struct", "TransactionsBlock", (
        "_guard:\n            concread::release::ReleaseGuard<'storage, MutexGuard<'storage, RawMutex, Arc<()>>>",
        "latest_block_ref: &'storage ArcSwapOption<BlockInfo>",
        "blocks_ref: &'storage DashMap<Key, Value>",
    )),
    (STORAGE, "struct", "PreparedTransactionsBlock", (
        "block: TransactionsBlock<'storage>", "publication: MembershipPublication",
        "next_identity: Arc<()>",
    )),
    (STORAGE, "enum", "MembershipPublication", (
        "Repeated", "Replace", "Advance", "previous: Option<Arc<BlockInfo>>",
    )),
    (STORAGE, "method", "TransactionsBlock::prepare_commit", (
        "let publication = self.admit_publication()?;",
        "Ok(PreparedTransactionsBlock", "block: self", "publication",
    )),
    (STORAGE, "method", "TransactionsBlock::admit_publication", (
        "self.latest_block_ref.load_full()", "TransactionsBlockError::MissingInsertBlock",
        "previous_block.height == current_block.height",
        "previous_block.transactions == current_block.transactions",
        "MembershipPublication::Repeated", "usize::from(!self.revert)",
        ".checked_add(addition)", "TransactionsBlockError::HeightOverflow",
        "expected_current_height != current_height", "TransactionsBlockError::HeightMismatch",
        "MembershipPublication::Replace", "MembershipPublication::Advance",
        "previous: previous_block",
    )),
    (STORAGE, "method", "PreparedTransactionsBlock::as_block", (
        "&TransactionsBlock<'storage>", "&self.block",
    )),
    (STORAGE, "method", "PreparedTransactionsBlock::publish", (
        "fn publish(self)", "mut block,", "publication,", "next_identity",
        "let changes_identity = !matches!(&publication, MembershipPublication::Repeated)",
        "if changes_identity", "std::mem::replace(&mut **block._guard, next_identity)",
        "MembershipPublication::Repeated", "MembershipPublication::Replace",
        ".retain(|_, height| *height < current.height)",
        "MembershipPublication::Advance", "if let Some(previous) = &previous",
        "block.blocks_ref.insert(transaction, previous.height)",
        "block.latest_block_ref.swap(Some(current))", "_guard.release_deferred(drop)",
        "TransactionsPublicationRetirement {", "_tip: tip", "_staged: current_block",
        "_identity: identity", "_release: release",
    )),
    (STORAGE, "struct", "TransactionsPublicationRetirement", (
        "_tip: Option<Arc<BlockInfo>>", "_staged: Option<Arc<BlockInfo>>",
        "_identity: Arc<()>", "_release: concread::release::DeferredRelease",
    )),
    (STORAGE, "struct", "PublishedTransactions", (
        "_retirement: TransactionsPublicationRetirement", "_installation: Installation",
    )),
    (STORAGE, "method", "PreparedDetachedTransactionsBlock::publish", (
        "fn publish(self) -> PublishedTransactions<Installation>",
        "PublishedTransactions {",
        "_retirement: prepared.publish()", "_installation: installation",
    )),
    (STORAGE, "struct", "DetachedTransactionsBlock", (
        "predecessor_identity: Arc<()>",
        "predecessor: Option<Arc<BlockInfo>>", "current: Arc<BlockInfo>",
        "revert: bool", "publication: MembershipPublication", "next_identity: Arc<()>",
    )),
    (STORAGE, "method", "PreparedTransactionsBlock::detach", (
        "fn detach(self)", "self.detach_retaining().0",
    )),
    (STORAGE, "method", "PreparedTransactionsBlock::detach_retaining", (
        "fn detach_retaining(\n            self,\n        )", "predecessor_identity: Arc::clone(&block._guard)",
        "predecessor: block.latest_block_ref.load_full()", "revert: block.revert",
        "_guard.release_deferred(drop)", "(detached, release)",
    )),
    (STORAGE, "method", "DetachedTransactionsBlock::observe_predecessor", (
        "storage.write_lock.try_lock()", "MembershipPredecessorStatus::Busy",
        "let guard = storage.released.guard(guard);",
        "Arc::ptr_eq(&guard, &self.predecessor_identity)",
        "MembershipPredecessorStatus::Current", "MembershipPredecessorStatus::Changed",
    )),
    (STORAGE, "method", "DetachedTransactionsBlock::try_prepare_publication", (
        "let wait = storage.released.observe();", "self.observe_predecessor(storage)",
        "mv::PublicationPreparationError::after_failed_acquisition(wait)",
        "let installation = match admit(&self, storage)",
        "storage.write_lock.try_lock()", "let guard = storage.released.guard(guard);",
        "Arc::ptr_eq(&guard, &self.predecessor_identity)", "_guard: guard",
        "PreparedDetachedTransactionsBlock", "installation,",
    )),
)
MEMBERSHIP_SOURCE_RELATIVES = (
    Path("scripts/formal/sumeragi_v2_multilane_membership_contract.py"),
    Path("pytests/scripts/sumeragi_v2_multilane_membership_contract_test.py"),
    Path(STATE), Path(STORAGE),
)


def validate_membership_contract(
    root: Path, models: Any, errors: list[str], rust_binding_item: Callable,
) -> None:
    """Require original writer ownership, exact admission and infallible consumption."""

    for name in MODELS:
        owners = [m for m in models if isinstance(m, dict) and m.get("module") == name]
        bindings = owners[0].get("production_symbols", ()) if len(owners) == 1 else ()
        for path, kind, symbol, tokens in MEMBERSHIP_BINDINGS:
            matches = [b for b in bindings if isinstance(b, dict)
                       and (b.get("path"), b.get("kind"), b.get("symbol")) == (path, kind, symbol)]
            if len(matches) != 1:
                errors.append(f"membership ledger {name} owner {symbol} must occur exactly once")
            elif tuple(matches[0].get("required_tokens", ())) != tokens:
                errors.append(f"membership reviewed tokens changed for {name}::{symbol}")

    items: dict[str, str] = {}
    raw_items: dict[str, str] = {}
    for path, kind, symbol, tokens in MEMBERSHIP_BINDINGS + ((STATE, "fn", "commit_inner", ()),):
        item = rust_binding_item(root, path, kind, symbol, "membership ownership", errors)
        if item is not None:
            raw_items[symbol] = item
            items[symbol] = _code(item)
            for token in tokens:
                if _code(token) not in items[symbol]:
                    errors.append(f"membership {symbol} is missing executable relation {token!r}")

    def require(symbol: str, relation: str) -> None:
        if symbol in items and _code(relation) not in items[symbol]:
            errors.append(f"membership {symbol} is missing executable relation {relation!r}")

    require("TransactionsBlock::prepare_commit", "fn prepare_commit(self,)")
    require("TransactionsBlock::prepare_commit",
            "let publication = self.admit_publication()?; Ok(PreparedTransactionsBlock { block: self, publication, next_identity: Arc::new(()), })")
    require("TransactionsBlock::admit_publication",
            "if !self.revert && previous_block.as_ref().is_some_and(|previous_block| { previous_block.height == current_block.height && previous_block.transactions == current_block.transactions }) { return Ok(MembershipPublication::Repeated); }")
    require("TransactionsBlock::admit_publication",
            "if expected_current_height != current_height { return Err(TransactionsBlockError::HeightMismatch {")
    require("TransactionsBlock::admit_publication",
            "if self.revert { Ok(MembershipPublication::Replace { current: Arc::clone(current_block), }) } else { Ok(MembershipPublication::Advance { previous: previous_block, current: Arc::clone(current_block), }) }")
    require("PreparedTransactionsBlock::publish", "MembershipPublication::Repeated => None")
    require("PreparedTransactionsBlock::publish",
            "let Self { mut block, publication, next_identity, } = self;")
    require("PreparedTransactionsBlock::publish",
            "if changes_identity { std::mem::replace(&mut **block._guard, next_identity) } else { next_identity }")
    require("PreparedTransactionsBlock::detach", "self.detach_retaining().0")
    require("PreparedTransactionsBlock::detach_retaining", "let ((), release) = _guard.release_deferred(drop);")
    require("DetachedTransactionsBlock::observe_predecessor",
            "let Some(guard) = storage.write_lock.try_lock() else { return MembershipPredecessorStatus::Busy; }; let guard = storage.released.guard(guard); if Arc::ptr_eq(&guard, &self.predecessor_identity)")
    require("DetachedTransactionsBlock::try_prepare_publication",
            "let wait = storage.released.observe(); match self.observe_predecessor(storage)")
    require("DetachedTransactionsBlock::try_prepare_publication",
            "let wait = storage.released.observe(); let Some(guard) = storage.write_lock.try_lock() else { return Err((self, mv::PublicationPreparationError::after_failed_acquisition(wait),)); }; let guard = storage.released.guard(guard); if !Arc::ptr_eq(&guard, &self.predecessor_identity)")
    require("PreparedTransactionsBlock::as_block",
            "fn as_block(&self) -> &TransactionsBlock<'storage> { &self.block }")

    prepared = raw_items.get("PreparedTransactionsBlock", "")
    if re.search(r"(?m)^\s*pub(?:\([^)]*\))?\s+\w+\s*:", prepared) or "&mut" in prepared:
        errors.append("membership prepared owner exposes mutable authority")
    detached = raw_items.get("DetachedTransactionsBlock", "")
    if re.search(r"(?m)^\s*pub(?:\([^)]*\))?\s+\w+\s*:", detached) or "&mut" in detached:
        errors.append("membership detached owner exposes mutable authority")
    if any(owner in detached for owner in ("MutexGuard", "ReleaseGuard", ": TransactionsBlock<")):
        errors.append("membership detached owner retains a physical writer")
    for forbidden in ("admit_publication(", "validate_commit(", "prepare_commit(",
                      "load_full(", "Err(", "Result<", "?", ".lock("):
        if forbidden in items.get("PreparedTransactionsBlock::publish", ""):
            errors.append(f"membership publication repeats admission or can refuse: {forbidden}")

    # Bind the still-active raw State path without implying complete prepared Apply.
    commit = items.get("commit_inner", "")
    cursor = 0
    for relation in (
        "let membership_retirement;",
        "let _state_commit_lock = state_ref.state_commit_lock.lock();",
        "let tx_validate_result = transactions.prepare_commit();",
        "let transactions = tx_validate_result?;",
        "let autoscale_lifecycle_guard",
        "autoscale_retirement_queue_veto.as_mut()",
        "state_ref.apply_committed_autoscale_lane_geometry(",
        "membership_retirement = transactions.publish();",
        "canonical_runtime.commit();",
        "world.commit();",
        "drop(_state_commit_lock);", "drop(membership_retirement);",
    ):
        normalized = _code(relation)
        position = commit.find(normalized, cursor)
        if position < 0 or commit.count(normalized) != 1:
            errors.append(f"membership State publication misses or reorders {relation!r}")
            break
        cursor = position + len(normalized)
