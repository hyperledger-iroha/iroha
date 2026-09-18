"""Mutation checks for lifecycle owner reads; these are not runtime liveness evidence."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import shutil
import sys

import pytest


REPO = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="module")
def checker():
    spec = importlib.util.spec_from_file_location(
        "owner_derived_scheduler_checker",
        REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py",
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def copy_scheduler_sources(root, checker):
    for relative in checker._LIFECYCLE_OWNER_DERIVED_SCHEDULER_PATHS.values():
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / relative, target)


def test_owner_derived_scheduler_current_semantics(tmp_path, checker):
    """A non-Git fixture checks clauses without promoting source-seal evidence."""
    copy_scheduler_sources(tmp_path, checker)
    assert checker._lifecycle_owner_derived_scheduler_errors(tmp_path) == []


@pytest.mark.parametrize("role,old,new,diagnostic", [
    ("turn", "self.executor.lifecycle_decision_apply_is_complete()", "false", "terminal Apply owner"),
    ("turn", "self.pending_lifecycle_completion.as_ref()", "None", "parked completion owner"),
    ("turn", "successor.reducer_fence_wait()", "None", "source-bound successor fence"),
    ("turn", "self.owner.coordinator.active_lease.as_ref()", "None", "active coordinator lease"),
    ("turn", ".has_unleased_lifecycle_completion_work()", ".has_generic_work()", "unleased physical completion owner"),
    ("turn", "self.executor.live_lifecycle_decision_apply_key()", "None", "exact Ready Apply key"),
    ("turn", ".prepare_ready_live_decision_apply_reconciliation(", ".assume_ready_live_decision_apply(", "Ready Apply registry join"),
    ("turn", ".exactly_owns_live_lifecycle_decision_apply(&authority)", ".assumes_live_lifecycle_decision_apply(&authority)", "exact Apply custody"),
    ("turn", ".lifecycle_serve_ownership_snapshot()", ".completed_serve_snapshot()", "retained Serve worker owner"),
    ("turn", "LifecycleWorkClass::Apply => return Ok(Claim::AwaitingApplyCompletion)", "LifecycleWorkClass::Apply => return Ok(Claim::Eligible)", "typed owner classification"),
    ("turn", "if authority.dispatch_key() != key", "if false", "typed owner classification"),
    ("turn", "serve.authority == LifecycleServeAuthorityKindV1::Claimed", "false", "typed owner classification"),
    ("worker_ownership", "!state.lifecycle_validates.is_empty()", "false", "unleased custody survives physical completion"),
    ("worker_ownership", "V2IoWorkDescriptor::PersistCertifiedFetchBody { .. }", "V2IoWorkDescriptor::Store { .. }", "unleased custody survives physical completion"),
    ("worker_ownership", ".lifecycle_serves", ".completed_lifecycle_serves", "Serve census reads retained exact index"),
    ("height", "let mut producer_claim = activated.producer_claim_projection()?;", "let mut producer_claim = LifecycleProducerClaimDispositionV1::Eligible;", "drain starts from actual owner"),
    ("ordinary", "        producer_claim = activated.producer_claim_projection()?;", "        producer_claim = drain_disposition.producer_claim();", "all ingress drains re-read actual owner"),
])
def test_owner_read_removal_is_rejected(tmp_path, checker, role, old, new, diagnostic):
    """Removing one real owner read cannot pass by retaining a familiar type name."""
    copy_scheduler_sources(tmp_path, checker)
    path = tmp_path / checker._LIFECYCLE_OWNER_DERIVED_SCHEDULER_PATHS[role]
    source = path.read_text()
    assert old in source
    if role == "worker_ownership" and diagnostic == "unleased custody survives physical completion":
        # The persistence snapshot reads the same descriptor before this census.
        # Mutate the owner being checked, regardless of other function ordering.
        (census,) = checker.rust_items(source, "has_unleased_lifecycle_completion_work")
        assert census.source.count(old) == 1
        mutated = census.source.replace(old, new, 1)
        path.write_text(source.replace(census.source, mutated, 1))
    else:
        path.write_text(source.replace(old, new, 1))
    errors = checker._lifecycle_owner_derived_scheduler_errors(tmp_path)
    assert any(diagnostic in error for error in errors), errors


def test_post_runtime_projection_refresh_is_required(tmp_path, checker):
    copy_scheduler_sources(tmp_path, checker)
    path = tmp_path / checker._LIFECYCLE_OWNER_DERIVED_SCHEDULER_PATHS["ordinary"]
    source = path.read_text()
    old = """producer_claim = activated.producer_claim_projection()?;
        if producer_claim.requires_yield()"""
    assert source.count(old) == 1
    path.write_text(source.replace(old, "if producer_claim.requires_yield()", 1))
    errors = checker._lifecycle_owner_derived_scheduler_errors(tmp_path)
    assert any("Runtime refresh precedes Producer" in error for error in errors), errors


def test_open_preflight_projection_refresh_is_required(tmp_path, checker):
    """The recovery batch must re-read ownership even when other drains still do."""
    copy_scheduler_sources(tmp_path, checker)
    path = tmp_path / checker._LIFECYCLE_OWNER_DERIVED_SCHEDULER_PATHS["ordinary"]
    source = path.read_text()
    old = """                    producer_claim = activated.producer_claim_projection()?;
                    if let Some(reason) = drain_disposition.advance_executor_yield() {
                        last_advance_executor_yield =
                            Some(("open-preflight", reason, Instant::now()));
                    }"""
    assert source.count(old) == 1
    path.write_text(source.replace(old, old.split("\n", 1)[1], 1))
    errors = checker._lifecycle_owner_derived_scheduler_errors(tmp_path)
    assert any("all ingress drains re-read actual owner" in error for error in errors), errors


def test_owner_derived_scheduler_gate_is_wired(tmp_path, checker, monkeypatch):
    """The existing reconciled owner contract cannot ignore this new semantic gate."""
    calls = []

    def rejected(root):
        calls.append(root)
        return ["owner-derived scheduler rejected"]

    monkeypatch.setattr(checker, "_lifecycle_owner_derived_scheduler_errors", rejected)
    errors = checker._lifecycle_certified_serve_reconciled_owner_errors(tmp_path)
    assert calls == [tmp_path]
    assert "owner-derived scheduler rejected" in errors
