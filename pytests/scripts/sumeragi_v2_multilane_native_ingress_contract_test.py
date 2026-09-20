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
    ("FairV2Ingress::try_push_at", "if inbound.message().is_native_lane()", "if false"),
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
