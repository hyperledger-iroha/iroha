"""Mutation controls for current publication branches and delegated Native bounds."""
from __future__ import annotations

import ast
import copy
import importlib.util
import sys
from pathlib import Path

import pytest


def support():
    path = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")
    spec = importlib.util.spec_from_file_location("publication_support", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def captured(tmp_path_factory):
    helper = support()
    checker = helper.load_checker()
    c = checker.native_publication_contract
    root = tmp_path_factory.mktemp("native-publication")
    helper.copy_reviewed_source_fixture_with_includes(root, checker, {
        *(p for p in c.SOURCE_RELATIVES if p.suffix == ".rs"),
    })
    keys = {(p, k, s) for _, p, k, s, _ in c.BINDINGS} | set(c.EXTRA_ITEMS)
    keys.update((c.KURA, "fn", s) for s in c.PUBLICATION_SYMBOLS)
    items, errors = {}, []
    with checker._reviewed_rust_source_cache():
        for key in sorted(keys):
            items[key] = checker._rust_binding_item(root, *key, "publication mutation fixture", errors)
    assert errors == []
    models = helper.canonical_models()
    result = root, checker, models, items
    assert validate(result) == []
    return result


def validate(captured, *, altered=None, models=None):
    root, checker, original_models, original = captured
    items = original if altered is None else altered
    def exact_item(_root, path, kind, symbol, _label, _errors):
        return items[(path, kind, symbol)]
    errors = []
    checker.native_publication_contract.validate_owners(
        root, original_models if models is None else models, errors, exact_item)
    checker.native_publication_contract.validate_phases(items, errors)
    return errors


def test_current_native_publication_owners_accept(captured):
    assert validate(captured) == []


def test_native_publication_owner_checks_and_source_closure_are_connected():
    checker = support().load_checker()
    tree = ast.parse(Path(checker.__file__).read_text())
    defs = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
    for owner, called in (("_validate", "validate_owners"),
                          ("_validate_native_prepublication_contract", "validate_phases")):
        assert sum(isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
                   and isinstance(n.func.value, ast.Name)
                   and n.func.value.id == "native_publication_contract"
                   and n.func.attr == called for n in ast.walk(defs[owner])) == 1
    assert any(isinstance(n, ast.Attribute) and n.attr == "SOURCE_RELATIVES"
               and isinstance(n.value, ast.Name) and n.value.id == "native_publication_contract"
               for n in ast.walk(defs["source_manifest_sha256"]))


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "weakened"])
def test_native_publication_requires_exact_delegated_ledger_owners(captured, mutation):
    _, checker, models, _ = captured
    for model, path, kind, symbol, _ in checker.native_publication_contract.BINDINGS:
        changed = copy.deepcopy(models)
        owner = next(m for m in changed if m["module"] == model)
        rows = owner["production_symbols"]
        index = next(i for i, r in enumerate(rows)
                     if (r["path"], r["kind"], r["symbol"]) == (path, kind, symbol))
        if mutation == "missing":
            rows.pop(index)
        elif mutation == "duplicate":
            rows.append(dict(rows[index]))
        else:
            rows[index]["required_tokens"] = []
        assert any("ledger owner differs" in e for e in validate(captured, models=changed)), symbol


@pytest.mark.parametrize("symbol,old,new", [
    ("NativeAmxParticipantSettlement", "    source_ids:", "    pub source_ids:"),
    ("NativeAmxParticipantSettlement::try_new", "source_ids.len() > NATIVE_AMX_GROUP_SOURCES_MAX", "false"),
    ("NativeAmxParticipantSettlement::try_from", "wire.source_ids,", "Vec::new(),"),
    ("NativeAmxParticipantSettlement::try_deserialize", "Self::try_from(wire)", "Self::unchecked(wire)"),
    ("NativeAmxParticipantSettlement::json_deserialize", "Self::try_from(wire)", "Self::unchecked(wire)"),
    ("validate_native_amx_participant_application_receipt_artifact", "settlement_source_ids != artifact.source_ids", "settlement_source_ids.len() != artifact.source_ids.len()"),
    ("validate_native_amx_participant_application_receipt_artifact", "artifact.results.len() != artifact.entrypoint_indices.len()", "false"),
    ("verified_v2_finality_wire_hash_for_eviction", "blocks_dir, height, canonical_hash", "blocks_dir, height, other_hash"),
    ("verified_kura_replica_authority_for_eviction", "verify_v2_finality_artifact_at(&path, &directory, &record.artifact, &read_identity)?", "verify_nothing()?"),
    ("durable_block_payload_len_by_hash", "if wire_len != index.length", "if wire_len > index.length"),
    ("apply_finalized_merge_carrier_repairs", "height, &repair.block, durable_count", "height, &other.block, durable_count"),
    ("add_disk_usage_bytes_locked", "self.add_total_disk_usage_bytes_locked(delta)", "self.add_total_disk_usage_bytes_locked(0)"),
    ("sub_disk_usage_bytes_locked", "self.sub_total_disk_usage_bytes_locked(delta)", "self.sub_total_disk_usage_bytes_locked(0)"),
    ("persist_autonomous_lifecycle_bootstrap_with_authentication", "vec![path.clone()]", "vec![]"),
    ("persist_autonomous_lifecycle_bootstrap_with_authentication", "self.update_disk_usage_delta(0, next_len);", "self.update_disk_usage_delta(0, next_len); self.update_total_disk_usage_delta(0, next_len);"),
    ("complete_autonomous_lifecycle_bootstrap", "receipt.proposal == payload.origin_proposal", "receipt.proposal.descriptor.lane_id == payload.origin_proposal.descriptor.lane_id"),
    ("transition_autonomous_lane_entrypoint_claims_locked", "with_resource_children(plan.len())", "with_resource_children(0)"),
    ("transition_autonomous_lane_entrypoint_claims_locked", "vec![claim.path.clone(), claim.temp_path.clone()]", "vec![claim.path.clone()]"),
    ("write_autonomous_lane_block_view_state_record_locked", "vec![path.to_path_buf(), temp_path.clone()]", "vec![path.to_path_buf()]"),
    ("persist_lane_payload_availability_certificate", "if slot_is_certified {", "if false {"),
    ("rebuild_native_amx_participant_receipt_latest_indexes_on_startup", "recovery.guard(),", "other.guard(),"),
    ("rebuild_native_amx_participant_receipt_latest_indexes_on_startup", "recovery.finish();", "other.finish();"),
])
def test_native_publication_rejects_delegation_or_accounting_mutation(captured, symbol, old, new):
    original = captured[3]
    key = next(k for k in original if k[2] == symbol)
    changed = dict(original)
    assert old in changed[key], (symbol, old)
    changed[key] = changed[key].replace(old, new, 1)
    assert validate(captured, altered=changed), symbol


@pytest.mark.parametrize("symbol", ["persist_native_amx_participant_application_evidence_under_publication_guard",
                                    "persist_native_amx_participant_application_repair_targets_under_publication_guard"])
@pytest.mark.parametrize("mutation", ["inverted-branch", "retry-readback", "retry-auth", "retry-cleanup", "manifest-consumption", "receipt-consumption", "latest-consumption", "latest-target", "retry-new-write", "preflight-target", "capacity-target"])
def test_native_publication_checks_both_actual_branches(captured, symbol, mutation):
    _, checker, _, original = captured
    c = checker.native_publication_contract
    key = (c.KURA, "fn", symbol)
    changed = dict(original)
    raw = original[key]
    if mutation == "inverted-branch":
        raw = raw.replace("if !publication_required", "if publication_required", 1)
    elif mutation in ("preflight-target", "capacity-target"):
        prefix, retry, publish = c.publication_branches(raw, symbol, [])
        name = ("preflight_native_amx_participant_application_repair_targets_under_publication_guard" if symbol == c.REPAIR else "preflight_native_amx_participant_application_plan_under_publication_guard") if mutation == "preflight-target" else "ensure_native_amx_publication_capacity_under_publication_guard"
        assert name in prefix
        prefix = prefix.replace(name, "wrong_target_operation", 1)
        raw = prefix + "if !publication_required {" + retry + "}" + publish
    elif mutation.startswith("retry-"):
        prefix, retry, publish = c.publication_branches(raw, symbol, [])
        if mutation == "retry-new-write":
            retry = "self." + c.WRITE_RECEIPT + "(receipt, manifest, true)?;" + retry
        else:
            name = {"retry-readback": c.READ_REPAIR if symbol == c.REPAIR else c.READ_MANIFEST,
                    "retry-auth": c.AUTHENTICATE, "retry-cleanup": c.CLEANUP}[mutation]
            assert name in retry
            retry = retry.replace(name, "omitted_operation", 1)
        raw = prefix + "if !publication_required {" + retry + "}" + publish
    else:
        prefix, retry, publish = c.publication_branches(raw, symbol, [])
        if mutation == "latest-target":
            publish = publish.replace("permit_cleanup,\n                preflight,", "permit_cleanup,\n                other_preflight,", 1) if symbol == c.PERSIST else publish.replace("true,\n                preflight,", "true,\n                other_preflight,", 1)
        else:
            component = mutation.split("-")[0].title()
            publish = publish.replace("NativeAmxPublicationComponent::" + component, "NativeAmxPublicationComponent::Other", 1)
        raw = prefix + "if !publication_required {" + retry + "}" + publish
    assert raw != original[key]
    changed[key] = raw
    assert any("Native prepublication" in e or "Native publication" in e
               for e in validate(captured, altered=changed)), mutation


@pytest.mark.parametrize("path,old,new", [
    ("crates/iroha_data_model/src/block/consensus.rs", "NATIVE_AMX_GROUP_SOURCES_MAX: usize = 4_096", "NATIVE_AMX_GROUP_SOURCES_MAX: usize = 4_097"),
    ("crates/iroha_core/src/native_amx.rs", "crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS;", "usize::MAX;"),
])
def test_native_publication_source_cap_parity_is_checked(captured, path, old, new):
    source = captured[0] / path
    before = source.read_text()
    assert old in before
    try:
        source.write_text(before.replace(old, new, 1))
        assert any("source cap parity" in e for e in validate(captured))
    finally:
        source.write_text(before)


@pytest.mark.parametrize("symbol,old,new", [
    ("NativeAmxParticipantApplicationPrepublicationToken", "    original_kura:", "    pub original_kura:"),
    ("NativeAmxParticipantApplicationPrepublicationToken::from_plan", "            original_kura,", "            original_kura: other_identity,"),
    ("Kura::reauthenticate_native_amx_prepublication", "        drop(sidecar);", "        // drop(sidecar);"),
    ("Kura::reauthenticate_native_amx_prepublication", "let canonical = self.canonical_chain_lock.lock();", "let canonical = other.canonical_chain_lock.lock();"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "if !token.original_kura.matches(self)", "if false"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "if !token.authenticates_state_frontiers(block, manifest, finality, frontiers)", "if false"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "token.application_block_height,", "other_height,"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "!= token.finality_artifact_hash", "== token.finality_artifact_hash"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "artifacts.len() != token.identities.len()", "artifacts.len() > token.identities.len()"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "artifacts.iter().zip(&token.identities)", "artifacts.iter().take(1).zip(&token.identities)"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "expected_manifest, expected_receipt, false,", "expected_manifest, expected_receipt, true,"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "if actual != *expected_identity", "if actual == *expected_identity"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "        self.ensure_prune_recovery_not_required()?;", "        let _reentrant = self.prune_lock.lock();\n        self.ensure_prune_recovery_not_required()?;"),
    ("Kura::reauthenticate_native_amx_prepublication_under_publication_guards", "        self.ensure_prune_recovery_not_required()?;", "        if manifest.entries().is_empty() { return Ok(()); }\n        self.ensure_prune_recovery_not_required()?;"),
    ("authenticate_native_amx_participant_application_prepublication_under_publication_guards", "        let descriptor =", "        let _reentrant = self.canonical_chain_lock.lock();\n        let descriptor ="),
    ("V2ApplyService::validate_and_apply", ".reauthenticate_native_amx_prepublication(", ".skip_participant_readback("),
    ("V2ApplyService::validate_and_apply", '"pre-WSV Native AMX participant custody reauthentication",\n                        &error,\n                    )\n                })?;', '"pre-WSV Native AMX participant custody reauthentication",\n                        &error,\n                    )\n                });'),
])
def test_native_participant_custody_rejects_changed_owner_or_incomplete_readback(
    captured, symbol, old, new,
):
    original = captured[3]
    key = next(k for k in original if k[2] == symbol)
    assert old in original[key], (symbol, old)
    changed = dict(original)
    changed[key] = changed[key].replace(old, new, 1)
    assert validate(captured, altered=changed), symbol


def test_native_participant_custody_checks_actual_lock_and_state_staging_order(captured):
    original = captured[3]
    cases = (
        ("Kura::reauthenticate_native_amx_prepublication",
         "        let canonical = self.canonical_chain_lock.lock();\n        let geometry = self.lane_geometry_lock.lock();",
         "        let geometry = self.lane_geometry_lock.lock();\n        let canonical = self.canonical_chain_lock.lock();"),
        ("V2ApplyService::validate_and_apply",
         "        if let Some(token) = native_amx_prepublication.as_ref() {",
         "        state_block.authorize_execution_output_publication(&committed_block, &witness)?;\n        if let Some(token) = native_amx_prepublication.as_ref() {"),
    )
    for symbol, old, new in cases:
        key = next(k for k in original if k[2] == symbol)
        assert old in original[key], symbol
        changed = dict(original)
        changed[key] = changed[key].replace(old, new, 1)
        assert any("custody" in error for error in validate(captured, altered=changed)), symbol


@pytest.mark.parametrize("retry", [True, False])
def test_native_participant_token_minting_requires_original_kura_in_both_paths(captured, retry):
    _, checker, _, original = captured
    c = checker.native_publication_contract
    key = (c.KURA, "fn", c.PERSIST)
    prefix, repeated, fresh = c.publication_branches(original[key], c.PERSIST, [])
    changed = dict(original)
    if retry:
        assert "self.instance_identity()" in repeated
        repeated = repeated.replace("self.instance_identity()", "other.instance_identity()", 1)
    else:
        assert "self.instance_identity()" in fresh
        fresh = fresh.replace("self.instance_identity()", "other.instance_identity()", 1)
    changed[key] = prefix + "if !publication_required {" + repeated + "}" + fresh
    assert any("from_plan" in error for error in validate(captured, altered=changed))
