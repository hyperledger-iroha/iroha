"""Actual-source and mutation controls for process-lived Native ingress custody."""
from __future__ import annotations

import ast
import importlib.util
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]


def checker():
    path = ROOT / "scripts/formal/check_sumeragi_v2_multilane_models.py"
    spec = importlib.util.spec_from_file_location("native_ingress_checker", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def captured():
    module = checker()
    contract = module.native_ingress_contract
    items, errors = {}, []
    with module._reviewed_rust_source_cache():
        for path, kind, symbol, _ in (*contract.EXACT, *contract.ORDERED):
            key = path, kind, symbol
            items[key] = module._rust_binding_item(ROOT, *key, "Native ingress fixture", errors)
    assert errors == []
    result = module, items
    assert validate(result) == []
    return result


def validate(captured, altered=None):
    module, original = captured
    items = original if altered is None else altered
    errors = []
    module.native_ingress_contract.validate_owners(
        ROOT, errors,
        lambda _root, path, kind, symbol, _label, _errors: items[path, kind, symbol],
    )
    return errors


def test_native_ingress_accepts_actual_separate_custody_owners(captured):
    assert validate(captured) == []


@pytest.mark.parametrize("old,new", (
    ("self.service_lock.lock()", "foreign.service_lock.lock()"),
    ("self.producer_publication_lock.lock()", "foreign.producer_publication_lock.lock()"),
    ("self.state.lock()", "foreign.state.lock()"),
    ("if state.open", "if false"),
    ("validate_live_queue_structure(&state)", "validate_live_queue_structure(&foreign)"),
    ("state.has_global_ingress()", "false"),
    ("state.last_service_attempt_at.is_some()", "false"),
    ("super::super::FairV2IngressLeaderWireStatus::Runtime", "super::super::FairV2IngressLeaderWireStatus::VolatileTerminal"),
    ("Arc::clone(&entry.inbound)", "Arc::clone(&foreign.inbound)"),
    ("Arc::clone(&entry.ownership_snapshot)", "Arc::clone(&foreign.ownership_snapshot)"),
    ("!inbound.message().is_native_lane()", "false"),
    ("!snapshot.validate_exact()", "false"),
    ("!live.validate_exact()", "false"),
    ("!= live.process_local_projection_hash_with_peer_encodings", "== live.process_local_projection_hash_with_peer_encodings"),
    ("!live.matches_message(inbound.message())", "false"),
    ("!live.matches_semantic_origin(inbound.sender())", "false"),
    ("!live.matches_reply_routes(inbound.reply_routes())", "false"),
    ("live.leader_wire_token().is_some()", "false"),
    ("live.leader_wire_runtime_receipt().is_some()", "false"),
    ("live.runtime_lifecycle_ordinal().is_some()", "false"),
))
def test_closed_global_cut_rejects_owner_substitution(captured, old, new):
    _, original = captured
    symbol = "FairV2Ingress::ensure_closed_global_drained_cut"
    key = next(k for k in original if k[2] == symbol)
    assert original[key].count(old) == 1, old
    changed = {**original, key: original[key].replace(old, new, 1)}
    assert any(symbol in error for error in validate(captured, changed))


@pytest.mark.parametrize("injected", (
    "drop(_service_guard);", "drop(_publication_guard);", "return Ok(());",
    "state.lanes.clear();", "state.ready.clear();", "state.pending_wire_owners.clear();",
    "state.retain_native_ingress();", "state.len = 0;", "state.bytes = 0;",
    "state.open = true;", "self.try_recv_if_checked(|_| true)?;",
))
def test_closed_global_cut_cannot_release_fences_or_rewrite_custody(captured, injected):
    _, original = captured
    symbol = "FairV2Ingress::ensure_closed_global_drained_cut"
    key = next(k for k in original if k[2] == symbol)
    anchor = "let state = self.state.lock();"
    assert original[key].count(anchor) == 1
    changed = {**original, key: original[key].replace(anchor, anchor + injected, 1)}
    assert any(symbol in error for error in validate(captured, changed))


@pytest.mark.parametrize("old,new", (
    ("!ready_sources.insert(source.clone())", "false"),
    ("ready_sources != nonempty_sources", "false"),
    ("!entry_storage_is_exact(state, source, entry)", "false"),
    ("!admission_ordinals.insert(entry.admission_ordinal)", "false"),
    ("!pending_wire.insert(key.clone())", "false"),
    (".insert(key.clone(), source.clone())", ".insert(key.clone(), foreign.clone())"),
    ("lane.pending_wire != pending_wire", "false"),
    ("lane.progress_len != progress_len", "false"),
    ("lane.certified_fence_escape_len != certified_fence_escape_len", "false"),
    ("lane.timeout_vote_len != timeout_vote_len", "false"),
    ("lane.transport_completion_len != transport_completion_len", "false"),
    ("lane.bytes != lane_bytes", "false"),
    ("lane.certified_fence_escape_bytes != certified_fence_escape_bytes", "false"),
    ("lane.timeout_vote_bytes != timeout_vote_bytes", "false"),
    ("lane.transport_completion_bytes != transport_completion_bytes", "false"),
    ("state.pending_wire_owners != pending_wire_owners", "false"),
    ("state.len != total_len", "false"),
    ("state.bytes != total_bytes", "false"),
    ("state.nonempty_since.is_some() != (total_len != 0)", "false"),
    ("*ordinal > state.last_admission_ordinal", "false"),
))
def test_closed_global_cut_requires_real_subordinate_accounting(captured, old, new):
    _, original = captured
    symbol = "validate_live_queue_structure"
    key = next(k for k in original if k[2] == symbol)
    assert original[key].count(old) == 1, old
    changed = {**original, key: original[key].replace(old, new, 1)}
    assert any(symbol in error for error in validate(captured, changed))


@pytest.mark.parametrize("symbol", (
    "FairV2Ingress::ensure_closed_global_drained_cut", "validate_live_queue_structure",
))
@pytest.mark.parametrize("gate", ("#[cfg(debug_assertions)]", "#[cfg_attr(not(debug_assertions), cfg(any()))]"))
def test_closed_global_cut_cannot_disable_release_authentication(captured, symbol, gate):
    _, original = captured
    key = next(k for k in original if k[2] == symbol)
    changed = {**original, key: gate + original[key]}
    assert any("conditionally disables release-mode authentication" in error for error in validate(captured, changed))


def test_native_ingress_gate_and_source_manifest_are_connected():
    module = checker()
    tree = ast.parse(Path(module.__file__).read_text())
    functions = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
    assert sum(isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
               and isinstance(n.func.value, ast.Name)
               and n.func.value.id == "native_ingress_contract"
               and n.func.attr == "validate_owners"
               for n in ast.walk(functions["_validate"])) == 1
    assert any(isinstance(n, ast.Attribute) and n.attr == "SOURCE_RELATIVES"
               and isinstance(n.value, ast.Name) and n.value.id == "native_ingress_contract"
               for n in ast.walk(functions["source_manifest_sha256"]))


@pytest.mark.parametrize("symbol,old,new", (
    ("FairV2Ingress::try_push_at", "self.try_push_owned_at(inbound, enqueued_at)", "if inbound.message().is_native_lane() { return Err(FairV2IngressPushError::rejected(inbound, FairV2IngressRejectReason::UnsupportedEnvelope)); } self.try_push_owned_at(inbound, enqueued_at)"),
    ("FairV2Ingress::try_push_at", "self.try_push_owned_at(inbound, enqueued_at)", "Ok(FairV2IngressPushDisposition::Coalesced)"),
    ("FairV2IngressSource::uses_authenticated_capacity", " | Self::Native(_)", ""),
    ("FairV2IngressSource::class", "Self::Authenticated(_) | Self::Native(_)", "Self::Authenticated(_)"),
    ("fair_v2_ingress_append_source_identity", "projection.push(2)", "projection.push(1)"),
    ("FairV2IngressState::retain_native_ingress", "self.lanes.retain(|source, _| source.is_native());", "self.lanes.clear();"),
    ("FairV2IngressState::retain_native_ingress", ".retain(|_, source| source.is_native())", ".clear()"),
    ("FairV2IngressState::retain_native_ingress", "self.ready.retain(|source| source.is_native());", "self.ready.clear();"),
    ("FairV2IngressState::retain_native_ingress", "self.lanes.values().map(|lane| lane.entries.len()).sum()", "0"),
    ("FairV2IngressState::retain_native_ingress", "self.lanes.values().map(|lane| lane.bytes).sum()", "0"),
    ("FairV2IngressState::retain_native_ingress", "entry.enqueued_at", "Instant::now()"),
    ("FairV2IngressState::has_global_ingress", "!source.is_native()", "false"),
    ("FairV2Ingress::try_push_owned_at", "if encoded_len > lane_limit", "if false"),
    ("FairV2Ingress::try_push_owned_at", "FairV2IngressSource::Native(inbound.via.clone())", "FairV2IngressSource::Validator(inbound.via.clone())"),
    ("FairV2Ingress::try_push_owned_at", "source.uses_authenticated_capacity()", "matches!(source, FairV2IngressSource::Authenticated(_))"),
    ("FairV2Ingress::try_push_owned_at", "retained_authenticated_non_validator_sources >= capacity", "false"),
    ("FairV2Ingress::try_push_owned_at", "if source.is_native()", "if false"),
    ("fair_v2_ingress_current_protected_slots", "source.uses_authenticated_capacity()", "matches!(source, FairV2IngressSource::Authenticated(_))"),
    ("FairV2Ingress::configure_roster_with_byte_requirements", "state.retain_native_ingress();", "state.lanes.clear();"),
    ("FairV2Ingress::retire_leader_wire_lifecycle_gate", "state.retain_native_ingress();", "state.lanes.clear();"),
    ("FairV2Ingress::retire_leader_wire_lifecycle_gate", "bound.park_sealed_ingress(carriers)?", "unreachable!()"),
    ("FairV2Ingress::configure_roster_with_byte_requirements", "required.max(state.len.saturating_add(", "required.min(state.len.saturating_add("),
    ("FairV2Ingress::open", "required.max(state.len.saturating_add(", "required.min(state.len.saturating_add("),
    ("FairV2Ingress::bind_leader_wire_lifecycle_gate", "state.has_global_ingress()", "state.len != 0"),
    ("FairV2Ingress::dequeue_selected_locked", "else if source.uses_authenticated_capacity()", "else if false"),
    ("FairV2IngressOwnershipOccurrence::validate_exact", "native == self.authenticated_source.is_native()", "true"),
    ("FairV2IngressOwnershipOccurrence::validate_exact", "(!native || self.lifecycle_ordinal.is_none())", "true"),
    ("entry_storage_is_exact", "FairV2IngressSource::Native(entry.inbound.via().clone())", "FairV2IngressSource::Validator(entry.inbound.via().clone())"),
    ("FairV2Ingress::ensure_closed_drained_cut", "if state.len != 0", "if state.has_global_ingress()"),
))
def test_native_ingress_rejects_custody_mutation(captured, symbol, old, new):
    _, original = captured
    key = next(k for k in original if k[2] == symbol)
    assert old in original[key], (symbol, old)
    changed = dict(original)
    changed[key] = original[key].replace(old, new, 1)
    assert any(symbol in e for e in validate(captured, changed))


@pytest.mark.parametrize("symbol", (
    "FairV2Ingress::configure_roster_with_byte_requirements",
    "FairV2Ingress::retire_leader_wire_lifecycle_gate",
))
def test_native_ingress_rejects_destruction_after_correct_retention(captured, symbol):
    _, original = captured
    key = next(k for k in original if k[2] == symbol)
    for added in ("state.lanes.clear();", "state.ready.clear();",
                  "state.pending_wire_owners.clear();", "state.len = 0;",
                  "state.bytes = 0;", "state.last_admission_ordinal = 0;"):
        changed = dict(original)
        changed[key] = original[key].replace(
            "state.retain_native_ingress();", f"state.retain_native_ingress(); {added}", 1,
        )
        assert any("destroys retained custody" in e for e in validate(captured, changed))


@pytest.mark.parametrize("old,new", (
    ("let ingress_barrier_allows = source.is_native()", "let ingress_barrier_allows = false"),
    ("let ingress_barrier_allows = source.is_native()", "let ingress_barrier_allows = true"),
    ("predecessor.is_native() || *count == 0", "*count == 0"),
    ("dependency_bypass || (source.is_native() && leader_wire_barrier.is_some())", "dependency_bypass"),
    ("source.is_native() && leader_wire_barrier.is_some()", "false"),
    ("source.is_native() && leader_wire_barrier.is_some()", "true"),
    ("predecessor.is_native() || *count == 0", "true"),
    ("index < owner.ingress_predecessors", "index > owner.ingress_predecessors"),
    ("entry.leader_wire_token.as_ref() == Some(&owner.token)", "true"),
    ("let entry = &lane.entries[index];", "return FairV2IngressQueueGateVerdict::Strict; let entry = &lane.entries[index];"),
))
def test_native_and_global_selectors_preserve_independent_domain_order(captured, old, new):
    _, original = captured
    symbol = "fair_v2_ingress_queue_gate_verdict"
    key = next(k for k in original if k[2] == symbol)
    assert original[key].count(old) == 1
    changed = {**original, key: original[key].replace(old, new, 1)}
    assert any("queue gate" in error or symbol in error for error in validate(captured, changed))


@pytest.mark.parametrize("gate", ("#[cfg(test)]", "#[cfg_attr(not(debug_assertions), cfg(any()))]"))
def test_native_domain_ordering_is_unconditional(captured, gate):
    _, original = captured
    key = next(k for k in original if k[2] == "fair_v2_ingress_queue_gate_verdict")
    changed = {**original, key: gate + original[key]}
    assert any("conditionally disables domain ordering" in error for error in validate(captured, changed))
