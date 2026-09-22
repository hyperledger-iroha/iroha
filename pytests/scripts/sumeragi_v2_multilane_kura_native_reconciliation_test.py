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
    for relative in (KURA, DATA, CAPACITY, contract.INDEX, contract.PREFIX):
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


# Each control mutates the actual reviewed owner; selector labels remain stable.
REPAIR_PREFIX_MUTATIONS = (('strict-inventory-never-adopts-prefix',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_files_locked',
  'None,',
  'Some(candidate),'),
 ('only-real-path',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  'candidate.path == path',
  'candidate.path != path'),
 ('only-manifest',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  'candidate.kind == kind',
  'candidate.kind != kind'),
 ('exact-height',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  'candidate.participant_height == participant_height',
  'true'),
 ('exact-file-metadata',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  'Self::stable_sidecar_metadata_unchanged(&candidate.metadata, &metadata)',
  'true'),
 ('strict-empty-default',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  'len == 0 && !owned_prefix',
  'false'),
 ('physical-prefix-byte-bound',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  'len > self.native_amx_participant_evidence_file_bytes()',
  'false'),
 ('prefix-counts-in-aggregate',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  'bytes > aggregate_limit',
  'false'),
 ('retained-count-bound',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  'count > stable_entry_limit',
  'false'),
 ('completed-origin',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::authenticate_native_amx_completed_repair_on_startup',
  'record.origin != NativeAmxPublicationIndexOriginV1::CompletedRepair',
  'false'),
 ('original-wire-record',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards',
  'record.carrier != carrier',
  'false'),
 ('original-merge-record',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards',
  'record.merge_entry_hash != merge.map(MergeLedgerEntry::canonical_hash)',
  'false'),
 ('selected-full-wire',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards',
  'Self::native_amx_publication_carrier(&selected)? != carrier',
  'false'),
 ('reconstruct-from-full-carrier',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards',
  'from_result_bearing_block_and_merge_entry(block, merge)',
  'from_result_bearing_block_and_merge_entry(block, None)'),
 ('original-stable-receipt',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::require_native_amx_completed_repair_receipt_with_inventory_locked',
  'self.decode_native_amx_receipt_file_locked(entry, &namespace, file)? != *receipt',
  'false'),
 ('original-stable-latest',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::require_native_amx_completed_repair_receipt_with_inventory_locked',
  'NativeAmxParticipantReceiptLatestIndexV2::from_receipt(\n                receipt,\n            )',
  'NativeAmxParticipantReceiptLatestIndexV2::from_receipt(other)'),
 ('full-temp-remains-exact',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::require_native_amx_completed_repair_receipt_with_inventory_locked',
  '!= expected',
  '== expected'),
 ('only-existing-completed-owner',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'index.records.get(&carrier)',
  'index.records.values().next()'),
 ('all-carrier-routes',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'for (manifest, receipt) in artifacts {',
  'for (manifest, receipt) in artifacts.into_iter().take(1) {'),
 ('wsv-authority-before-cleanup',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  '!self\n                        .native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)?',
  'false'),
 ('physical-target-joins-receipt',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'self.native_amx_reservation_physical_target_from_journal(descriptor)?',
  'self.native_amx_reservation_physical_target_from_journal(&other.participant_proposal.descriptor)?'),
 ('exact-inventory-candidate',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'file.path == temporary.path',
  'true'),
 ('later-frontier-refuses-prefix',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  '} else if route_prefix.is_some() {',
  '} else if false {'),
 ('proper-prefix-only',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_indexed_publication_prefix_locked',
  'len >= expected.len()',
  'len > expected.len()'),
 ('exact-canonical-prefix',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_indexed_publication_prefix_locked',
  '!expected.starts_with(&prefix)',
  '!expected.ends_with(&prefix)'),
 ('same-open-descriptor',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_indexed_publication_prefix_locked',
  'Self::open_bound_progress_file(namespace, path, &metadata)?',
  'Self::open_bound_progress_file(namespace, other_path, &metadata)?'),
 ('retained-original-file',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_indexed_publication_prefix_locked',
  '&mut opened,',
  '&mut other_opened,'),
 ('retain-observed-length',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_indexed_publication_prefix_locked',
  'prefix.resize(len, 0);',
  'prefix.resize(0, 0);'),
 ('before-route-preflight-persist_native_amx_participant_application_evidence_under_publication_guard',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'persist_native_amx_participant_application_evidence_under_publication_guard',
  'self.recover_native_amx_indexed_publication_prefixes_under_publication_guard(block)?;',
  'let _ = block;'),
 ('before-route-preflight-persist_native_amx_participant_application_repair_targets_under_publication_guard',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'persist_native_amx_participant_application_repair_targets_under_publication_guard',
  'self.recover_native_amx_indexed_publication_prefixes_under_publication_guard(block)?;',
  'let _ = block;'))

@pytest.mark.parametrize("label,path,kind,symbol,old,new", REPAIR_PREFIX_MUTATIONS,
                         ids=[row[0] for row in REPAIR_PREFIX_MUTATIONS])
def test_completed_repair_prefix_rejects_owner_drift(physical_source, label, path, kind, symbol, old, new):
    mutate(physical_source, path, kind, symbol, old, new)
    errors = validate(physical_source)
    assert any("repair-prefix " in error and f"owner {symbol} " in error
               for error in errors), errors
    assert not any("digest" in error or "has 0 owners" in error for error in errors), errors


@pytest.mark.parametrize("first,second", [
    ("self.verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(",
     "Self::remove_bound_progress_file_if_matches("),
    ("Self::remove_bound_progress_file_if_matches(",
     "self.sync_native_amx_evidence_namespace("),
])
def test_completed_repair_prefix_rejects_unlink_order_drift(physical_source, first, second):
    symbol = "Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards"
    errors = []
    item = actual_item(physical_source, contract.PREFIX, "method", symbol, "prefix order", errors)
    assert errors == [] and item is not None
    # Both complete fallible statements remain syntactically present. Reverse
    # only the descriptor-check/unlink or unlink/directory-sync order.
    def statement(anchor):
        begin = item.index(anchor)
        end = item.index("?;", begin) + 2
        return begin, end, item[begin:end]
    a, a_end, a_text = statement(first)
    b, b_end, b_text = statement(second)
    assert a_end <= b
    mutate(physical_source, contract.PREFIX, "method", symbol,
           item[a:b_end], b_text + item[a_end:b] + a_text)
    assert any(f"repair-prefix executable owner {symbol} " in error
               for error in validate(physical_source))


INDEXED_PREFIX_MUTATIONS = (('no-finality-discovery-uses-local-body',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  '.get_block_without_merge_sidecar(height)',
  '.read_block_body_under_prune_and_canonical_guards(height)?'),
 ('no-temp-bypass-only-canonical-origin',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite\n                && !self',
  'record.origin != NativeAmxPublicationIndexOriginV1::CanonicalWrite\n                && !self'),
 ('no-temp-discovery-result-preserved',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  '&& !self\n'
  '                    .native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards(',
  '&& self\n'
  '                    .native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards('),
 ('original-merge-before-no-temp-bypass',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'record.merge_entry_hash != merge.as_ref().map(MergeLedgerEntry::canonical_hash)',
  'false'),
 ('original-selected-resolution-before-no-temp-bypass',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  ')? != NativeAmxPublicationIndexResolution::Committed',
  ')? == NativeAmxPublicationIndexResolution::Committed'),
 ('receipt-temporary-discovered',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards',
  'receipt_path.with_extension("norito.tmp")',
  'manifest_path.with_extension("norito.tmp")'),
 ('latest-temporary-discovered',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards',
  'directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE)',
  'directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE)'),
 ('discovery-original-route-height',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards',
  'let target = self.native_amx_reservation_physical_target_from_journal(descriptor)?;',
  'let target = self.native_amx_reservation_physical_target_from_journal(other)?;'),
 ('cleanup-artifacts-still-published-authority',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards',
  '.read_block_body_under_prune_and_canonical_guards(height)?',
  '.get_block_without_merge_sidecar(height)'),
 ('cleanup-artifacts-still-finality-archive',
  'crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards',
  '.v2_finality_artifact_with_archive_under_prune_and_canonical_guards(carrier.height)?',
  '.v2_finality_artifact_with_archive_under_prune_and_canonical_guards(other_height)?'),
 ('canonical-origin-does-not-require-completed-wsv',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'record.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair\n                    && !self',
  'true\n                    && !self'),
 ('every-incoming-route-preflight',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'self.preflight_native_amx_incoming_artifacts_locked(\n'
  '                        &target, &namespace, &inventory, &manifest, &receipt,\n'
  '                    )?;',
  'let _ = (&target, &namespace, &inventory, &manifest, &receipt);'),
 ('only-one-partial-phase-per-route',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'if route_prefix.replace(prefix).is_some() {',
  'if route_prefix.replace(prefix).is_none() {'),
 ('latest-phase-discriminator',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::require_native_amx_indexed_latest_prefix_locked',
  'prefix.component != NativeAmxPublicationComponent::Latest',
  'false'),
 ('latest-original-temp-path',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::require_native_amx_indexed_latest_prefix_locked',
  'prefix.path != expected_path',
  'false'),
 ('latest-no-pair-temp-overlap',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::require_native_amx_indexed_latest_prefix_locked',
  '!inventory.temporaries.is_empty()',
  'false'),
 ('latest-prune-absence',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::require_native_amx_indexed_latest_prefix_locked',
  'self.require_native_amx_evidence_prune_intent_absent_locked(namespace)?;',
  'let _ = namespace;'),
 ('latest-original-stable-manifest',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::require_native_amx_indexed_latest_prefix_locked',
  '!= *manifest',
  '== *manifest'),
 ('latest-original-stable-receipt',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::require_native_amx_indexed_latest_prefix_locked',
  '!= *receipt',
  '== *receipt'),
 ('latest-bound-independent-from-evidence',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_indexed_publication_prefix_locked',
  'if component == NativeAmxPublicationComponent::Latest {',
  'if false {'),
 ('pair-cannot-overlap-stable-file',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_indexed_publication_prefix_locked',
  'component != NativeAmxPublicationComponent::Latest',
  'component == NativeAmxPublicationComponent::Latest'),
 ('latest-reconstructed-original-receipt',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  '&NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt)',
  '&NativeAmxParticipantReceiptLatestIndexV2::from_receipt(other_receipt)'),
 ('physical-consumption-original-carrier',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  '.lock()\n            .get(&carrier)',
  '.lock()\n            .get(&other_carrier)'),
 ('physical-consumption-original-route',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  'owner.routes.contains_key(&route)',
  'owner.routes.contains_key(&other_route)'),
 ('physical-consumption-original-index',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  'index.records.get(&carrier)',
  'index.records.get(&other_carrier)'),
 ('physical-consumption-exact-latest',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  ')? != Some(expected)',
  ')? == Some(expected)'),
 ('physical-consumption-exact-manifest',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  '!= *manifest',
  '== *manifest'),
 ('physical-consumption-exact-receipt',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  '!= *receipt',
  '== *receipt'),
 ('physical-consumption-exact-pair-link',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  '!expected.matches_manifest(manifest)',
  'false'),
 ('physical-consumption-exact-latest-bytes',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  'norito::encode_canonical(&expected)?.len()',
  '0'),
 ('physical-consumption-before-wsv-only-cleanup',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'rebuild_native_amx_participant_receipt_latest_indexes_on_startup',
  'self.consume_native_amx_startup_stable_components_locked(\n'
  '                    &entry, &namespace, manifest, receipt,\n'
  '                )?;',
  'if native_amx_startup_retention_cleanup_authorized(expected_startup_evidence, false) { '
  'self.consume_native_amx_startup_stable_components_locked(&entry, &namespace, manifest, receipt)?; }'),
 ('physical-consumption-does-not-grant-cleanup',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'rebuild_native_amx_participant_receipt_latest_indexes_on_startup',
  'if native_amx_startup_retention_cleanup_authorized(\n'
  '                expected_startup_evidence,\n'
  '                !receipt_without_manifest.is_empty() || !manifest_without_receipt.is_empty(),\n'
  '            ) {',
  'if true {'))

@pytest.mark.parametrize("label,path,kind,symbol,old,new", INDEXED_PREFIX_MUTATIONS,
                         ids=[row[0] for row in INDEXED_PREFIX_MUTATIONS])
def test_indexed_publication_prefix_rejects_authority_drift(physical_source, label, path, kind, symbol, old, new):
    mutate(physical_source, path, kind, symbol, old, new)
    errors = validate(physical_source)
    assert any(f"repair-prefix executable owner {symbol} " in error for error in errors), errors
    assert not any("digest" in error or "has 0 owners" in error for error in errors), errors


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "weakened"])
def test_indexed_publication_prefix_ledger_rejects_each_current_owner(mutation):
    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    models = document["models"]
    model = next(m for m in models if m["module"] == checker.NATIVE_PREPUBLICATION_MODULE)
    baseline = model["production_symbols"]
    bindings = (*contract.REPAIR_PREFIX_BINDINGS, next(
        row for row in checker.native_preparation_contract.PREPARATION_OWNER_BINDINGS
        if row[:3] == (CAPACITY, "method", "Kura::native_amx_route_publication_capacity_with_inventory_locked")
    ))
    with checker._reviewed_rust_source_cache():
        errors = []
        checker._validate_native_prepublication_contract(ROOT, models, errors)
        assert errors == []
        for path, kind, symbol, tokens in bindings:
            index = next(i for i, row in enumerate(baseline)
                         if (row["path"], row["kind"], row["symbol"]) == (path, kind, symbol))
            altered = list(baseline)
            if mutation == "missing":
                altered.pop(index)
            elif mutation == "duplicate":
                altered.append(dict(baseline[index]))
            else:
                altered[index] = {**baseline[index], "required_tokens": list(tokens[:-1])}
            model["production_symbols"] = altered
            errors = []
            checker._validate_native_prepublication_contract(ROOT, models, errors)
            if mutation == "weakened":
                expected = f"reviewed prepublication tokens changed for {path}!{symbol}"
            else:
                expected = f"reviewed prepublication binding {path}!{symbol} must occur exactly once"
            assert any(expected in error for error in errors), (mutation, symbol, errors)
        model["production_symbols"] = baseline


MAINTENANCE_PREFIX_MUTATIONS = (('startup-maintenance-scope',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::rebuild_native_amx_publication_capacity_on_startup',
  'NativeAmxPrefixRecoveryScope::Startup',
  'NativeAmxPrefixRecoveryScope::Indexed'),
 ('live-indexed-scope',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_publication_guard',
  'NativeAmxPrefixRecoveryScope::Indexed',
  'NativeAmxPrefixRecoveryScope::Startup'),
 ('startup-empty-index-still-collects',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  'authenticated.is_empty() && scope == NativeAmxPrefixRecoveryScope::Indexed',
  'authenticated.is_empty()'),
 ('maintenance-original-journal-routes',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  'self.native_amx_evidence_physical_locations_from_journal()?',
  'Vec::new()'),
 ('maintenance-indexed-route-separation',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  'indexed_routes.contains(&(location.lane_id(), location.incarnation()))',
  'false'),
 ('maintenance-no-unindexed-pair-temporaries',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  '!inventory.temporaries.is_empty()',
  'false'),
 ('maintenance-highest-stable-manifest',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  'inventory.manifests.get(&height)',
  'inventory.manifests.get(&0)'),
 ('maintenance-highest-stable-receipt',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  'inventory.receipts.get(&height)',
  'inventory.receipts.get(&0)'),
 ('maintenance-cannot-borrow-pending-index',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  'index.records.contains_key(&carrier)',
  'false'),
 ('maintenance-original-durable-carrier',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  'self.ensure_durable_block_at_height(carrier.height, carrier.block_hash)?;',
  'let _ = carrier;'),
 ('maintenance-finality-authority',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  '!self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(&receipt, '
  '&manifest)',
  'false'),
 ('maintenance-wsv-authority',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  '!self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)?',
  'false'),
 ('maintenance-original-latest-encoding',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  'NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt)',
  'NativeAmxParticipantReceiptLatestIndexV2::from_receipt(other)'),
 ('maintenance-only-derived-latest',
  'crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  '*component != NativeAmxPublicationComponent::Latest',
  '*component == NativeAmxPublicationComponent::Latest'),
 ('consumption-retains-original-index-class',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  '.map(|owner| owner.index_record.clone())',
  '.map(|_| None)'),
 ('indexed-consumption-exact-original-index',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  'index.records.get(&carrier) != Some(original)',
  'index.records.get(&carrier) == Some(original)'),
 ('maintenance-consumption-no-foreign-index',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  'index.records.contains_key(&carrier)',
  'false'),
 ('maintenance-consumption-authority-gated',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  'original_index.is_none()',
  'original_index.is_some()'),
 ('maintenance-consumption-original-finality',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  '!self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(receipt, '
  'manifest)',
  'false'),
 ('maintenance-consumption-original-wsv',
  'crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  '!self.native_amx_publication_wsv_join_is_complete_locked(manifest, receipt)?',
  'false'))

@pytest.mark.parametrize("label,path,kind,symbol,old,new", MAINTENANCE_PREFIX_MUTATIONS,
                         ids=[row[0] for row in MAINTENANCE_PREFIX_MUTATIONS])
def test_completed_maintenance_prefix_rejects_authority_drift(physical_source, label, path, kind, symbol, old, new):
    mutate(physical_source, path, kind, symbol, old, new)
    errors = validate(physical_source)
    assert any((f"repair-prefix executable owner {symbol} " in error
                or f"repair-prefix ordered owner {symbol} " in error)
               for error in errors), errors
    assert not any("digest" in error or "has 0 owners" in error for error in errors), errors
