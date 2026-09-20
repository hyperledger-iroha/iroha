"""Actual Rust-provider controls for Kura Native source ownership and ordering.

These tests use the production item parser over complete physical providers.
They qualify these structural relations, not native execution or release input
admission. Negative receipts bind each changed provider's actual bytes.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
FORMAL = ROOT / "scripts/formal"
sys.path.insert(0, str(FORMAL))
spec = importlib.util.spec_from_file_location(
    "kura_native_reconciliation_checker", FORMAL / "check_sumeragi_v2_multilane_models.py"
)
assert spec is not None and spec.loader is not None
checker = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = checker
spec.loader.exec_module(checker)
contract = checker.kura_native_contract
KURA, DATA, CAPACITY = contract.KURA, contract.DATA, contract.CAPACITY
JOIN = "Kura::native_amx_publication_plan_is_durably_complete_under_prune_and_canonical_guards"
ENSURE = "Kura::ensure_native_amx_publication_capacity_under_publication_guard"


def actual_item(root, path, kind, symbol, label, errors):
    provider = root / path
    if not provider.is_file() or provider.is_symlink():
        errors.append(f"{label}: missing physical provider {path}")
        return None
    items = checker._extract_rust_binding_items(provider.read_text(), kind, symbol)
    if len(items) != 1:
        errors.append(f"{label}: {path}!{symbol} has {len(items)} owners")
        return None
    return items[0]


def validate(root):
    errors = []
    contract.validate(root, {}, errors, actual_item, checker._extract_braced_item)
    return errors


@pytest.fixture
def physical_source(tmp_path):
    for relative in (KURA, DATA, CAPACITY):
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, target)
    return tmp_path


def mutate(root, path, kind, symbol, before, after):
    provider = root / path
    original = provider.read_text()
    if symbol is None:
        owner = original
    else:
        errors = []
        owner = actual_item(root, path, kind, symbol, "mutation preimage", errors)
        assert errors == [] and owner is not None
    assert before != after and owner.count(before) == 1, (symbol, before)
    changed_owner = owner.replace(before, after, 1)
    assert original.count(owner) == 1
    changed = original.replace(owner, changed_owner, 1)
    provider.write_text(changed)
    receipt = {"path": path, "symbol": symbol,
               "before_sha256": hashlib.sha256(original.encode()).hexdigest(),
               "after_sha256": hashlib.sha256(provider.read_bytes()).hexdigest()}
    assert receipt["before_sha256"] != receipt["after_sha256"]
    (root / "changed-provider.json").write_text(json.dumps(receipt, sort_keys=True))


def test_current_kura_native_relations_accept_actual_providers():
    assert validate(ROOT) == []


def test_reconciled_declarations_preserve_every_unmoved_owner():
    reconciled = checker.NATIVE_PREPUBLICATION_BINDINGS
    assert len({row[:3] for row in reconciled}) == len(reconciled)
    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    model = next(m for m in document["models"] if m["module"] == "SumeragiV2NativeApplicationEvidence")
    rows = {(r["path"], r["kind"], r["symbol"]): r for r in model["production_symbols"]}
    for path, kind, symbol, tokens in reconciled:
        assert rows[(path, kind, symbol)]["required_tokens"] == list(tokens)
    sentinel = ("other/owner.rs", "fn", "unmoved", ("unchanged",))
    assert contract.reconcile_bindings((sentinel,))[0] == sentinel


@pytest.mark.parametrize("path", (KURA, DATA, CAPACITY))
def test_native_relation_requires_physical_provider(physical_source, path):
    (physical_source / path).unlink()
    assert any("missing" in e for e in validate(physical_source))


@pytest.mark.parametrize("symbol", contract.BRANCHED_SYMBOLS)
def test_discarded_capacity_result_cannot_force_retry(physical_source, symbol):
    mutate(physical_source, KURA, "fn", symbol,
           "let publication_required = self", "let publication_required = false;\n        self")
    assert any("capacity admission" in e for e in validate(physical_source))


@pytest.mark.parametrize("symbol", contract.BRANCHED_SYMBOLS)
def test_exact_capacity_result_cannot_be_shadowed(physical_source, symbol):
    mutate(physical_source, KURA, "fn", symbol,
           "if !publication_required {", "let publication_required = false;\n        if !publication_required {")
    assert any("without shadowing or reassignment" in e for e in validate(physical_source))


def test_all_route_admission_vector_cannot_be_shadowed(physical_source):
    mutate(physical_source, KURA, "fn", contract.LIVE,
           "let publication_required = self", "let all_targets = vec![0];\n        let publication_required = self")
    assert any("all-target vector" in e for e in validate(physical_source))


def test_fresh_manifest_comment_is_not_a_loop_owner(physical_source):
    mutate(physical_source, KURA, "fn", contract.LIVE,
           "for crash recovery:", "during recovery:")
    assert validate(physical_source) == []


MUTATIONS = (
    (CAPACITY, "method", ENSURE, "if let Some(existing) = existing {", "if let Some(existing) = None::<NativeAmxPublicationCapacityReservation> {", "existing-owner"),
    (CAPACITY, "method", ENSURE, "plan.routes.entry(route).or_insert(capacity);", "let _ = (route, capacity);", "existing-owner"),
    (CAPACITY, "method", ENSURE, "target_indices.len() != evidence.artifacts.len()", "false", "existing-owner"),
    (CAPACITY, "method", JOIN, "for (route, capacity) in &plan.routes {", "for (route, capacity) in plan.routes.iter().take(1) {", "all-route"),
    (CAPACITY, "method", JOIN, ".contains_key(&carrier)", ".contains_key(&carrier) && false", "all-route"),
    (CAPACITY, "method", JOIN, "latest.application_block_hash != carrier.block_hash", "false", "all-route"),
    (CAPACITY, "method", JOIN, "latest.lane_incarnation != route.incarnation", "false", "all-route"),
    (CAPACITY, "method", JOIN, "latest.participant_proposal_hash != capacity.proposal_hash", "false", "all-route"),
    (CAPACITY, "method", JOIN, "if !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)? {", "if false {", "all-route"),
    (KURA, "fn", "validate_native_amx_participant_application_receipt_artifact", "settlement_source_ids != artifact.source_ids", "settlement_source_ids == artifact.source_ids", "membership"),
    (DATA, "method", "NativeAmxParticipantSettlement::try_new", "source_ids.len() > NATIVE_AMX_GROUP_SOURCES_MAX", "source_ids.len() > usize::MAX", "lacks"),
    (DATA, "method", "NativeAmxParticipantSettlement::try_from", "Self::try_new(", "Self::unchecked_new(", "lacks"),
    (DATA, "method", "NativeAmxParticipantSettlement::json_deserialize", "Self::try_from(wire).map_err", "Self::unchecked_from(wire).map_err", "lacks"),
    (DATA, "struct", "NativeAmxParticipantSettlement", "source_ids: Vec<[u8; Hash::LENGTH]>", "pub source_ids: Vec<[u8; Hash::LENGTH]>", "private"),
    (DATA, None, None, "pub const NATIVE_AMX_GROUP_SOURCES_MAX: usize = 4_096;", "pub const NATIVE_AMX_GROUP_SOURCES_MAX: usize = 8_192;", "4096"),
    (KURA, "fn", "validate_native_amx_settlement_chain_links", "previous_native_hash != Some(previous)", "false", "lacks"),
    (KURA, "fn", "validate_native_amx_evidence_prune_intent_locked", "preimage_heights != removal_heights", "false", "lacks"),
    (KURA, "fn", "validate_native_amx_evidence_prune_intent_locked", "height <= highest_removal && !removal_heights.contains(height)", "false", "lacks"),
    (KURA, "fn", "plan_native_amx_evidence_prune_intent_from_artifacts", "complete.iter().rev()", "complete.iter()", "lacks"),
    (KURA, "fn", "plan_native_amx_evidence_prune_intent_from_artifacts", "pair_len > stable_byte_limit", "false", "lacks"),
    (KURA, "fn", "plan_native_amx_evidence_prune_intent_from_artifacts", "kept_complete.len() < retention.get()", "true", "lacks"),
    (KURA, "fn", "plan_native_amx_evidence_pair_prune_locked", "Hash::new(bytes) != removal.artifact_hash", "false", "lacks"),
    (KURA, "fn", "collect_native_amx_prune_settlement_preimages", "next_bytes > byte_limit", "false", "lacks"),
    (KURA, "fn", "rebuild_native_amx_participant_receipt_latest_indexes_on_startup", "self.complete_native_amx_evidence_prune_intent_locked(\n                    recovery.guard(),", "self.complete_native_amx_evidence_prune_intent_locked(\n                    lane_resources.guard(),", "ordered"),
    (KURA, "fn", contract.LIVE, "NativeAmxPublicationComponent::Manifest,", "NativeAmxPublicationComponent::Receipt,", "fresh publication"),
    (KURA, "fn", contract.REPAIR, "NativeAmxPublicationComponent::Receipt,", "NativeAmxPublicationComponent::Latest,", "fresh publication"),
    (CAPACITY, "method", "Kura::consume_native_amx_publication_component_after_durable_publication", "Some(u64::try_from(encoded_len)?)", "Some(0)", "lacks"),
)


@pytest.mark.parametrize("path,kind,symbol,before,after,diagnostic", MUTATIONS,
                         ids=[f"{i:02d}-{m[2] or 'source-cap'}" for i, m in enumerate(MUTATIONS)])
def test_native_relation_rejects_behavior_drift(physical_source, path, kind, symbol, before, after, diagnostic):
    mutate(physical_source, path, kind, symbol, before, after)
    assert any(diagnostic in e for e in validate(physical_source))


def test_binary_decoder_cannot_bypass_checked_constructor(physical_source):
    provider = physical_source / DATA
    original = provider.read_text()
    match = re.search(r"(?m)^impl<'de> norito::core::DeserializePayload<'de> for NativeAmxParticipantSettlement\s*", original)
    assert match is not None
    implementation = checker._extract_braced_item(original, match)
    assert implementation is not None and implementation.count("Self::try_from(wire)") == 1
    mutate(physical_source, DATA, None, None, implementation,
           implementation.replace("Self::try_from(wire)", "Self::unchecked_from(wire)"))
    assert any("binary decoder" in e for e in validate(physical_source))


@pytest.mark.parametrize("symbol", contract.BRANCHED_SYMBOLS)
def test_fresh_writer_must_precede_capacity_consumption(physical_source, symbol):
    errors = []
    item = actual_item(physical_source, KURA, "fn", symbol, "phase preimage", errors)
    assert errors == [] and item is not None
    # Match the real fresh writer and consume calls, preserving both complete
    # statements while reversing their order. This is more than token deletion.
    writer = re.search(r"self\.write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard\([\s\S]*?\)\?;", item)
    consume = re.search(r"self\.consume_native_amx_publication_component_after_durable_publication\([\s\S]*?\)\?;", item)
    assert writer is not None and consume is not None and writer.end() < consume.start()
    old = item[writer.start():consume.end()]
    new = consume.group() + item[writer.end():consume.start()] + writer.group()
    mutate(physical_source, KURA, "fn", symbol, old, new)
    assert any("fresh publication" in e for e in validate(physical_source))


def test_replica_registry_uses_accounted_owner_and_preserves_configuration(physical_source):
    rows = checker.KURA_RETENTION_REQUIRED_BINDINGS
    row = next(r for r in rows if r[2] == "Kura::new_inner")
    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    declared = next(r for r in document["kura_replica_retention_contract"]["production_symbols"]
                    if r["symbol"] == "Kura::new_inner")
    assert declared["required_tokens"] == list(row[3])
    errors = []
    item = actual_item(ROOT, *row[:3], "registry owner", errors)
    assert errors == [] and item is not None and all(t in item for t in row[3])
    before = "replica_registry: ResidentMutex::new(NestedMap::default(), &resource_inventory)"
    mutate(physical_source, KURA, "method", "Kura::new_inner", before,
           "replica_registry: Mutex::new(BTreeMap::new())")
    changed = actual_item(physical_source, *row[:3], "changed registry", errors)
    assert changed is not None and before not in changed
    assert before in row[3] and any(t not in changed for t in row[3])
