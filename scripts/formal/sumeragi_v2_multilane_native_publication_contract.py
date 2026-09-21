"""Current Native publication branches and delegated bounded evidence owners.

Structural mutation controls only: these do not prove complete State publication,
pre-vote resource reservation, or historical Native producer authority.
"""
from __future__ import annotations

import re
from pathlib import Path

from sumeragi_v2_multilane_geometry_evidence_contract import _code
from sumeragi_v2_multilane_reviewed_rust_source import _mask_rust_comments

KURA = "crates/iroha_core/src/kura.rs"
DATA = "crates/iroha_data_model/src/block/consensus.rs"
REPLICA = "crates/iroha_core/src/kura/retained_finality_replica_authority.rs"
PHYSICAL = "crates/iroha_core/src/kura/physical_resource_accounting.rs"
GUARD = "crates/iroha_core/src/kura/physical_resource_guard.rs"
LEASE = "crates/iroha_core/src/kura/publication_lease.rs"
TOKEN = "crates/iroha_core/src/kura/native_amx_participant_application_artifacts.rs"
APPLY = "crates/iroha_core/src/sumeragi/v2_apply.rs"
NATIVE = "SumeragiV2NativeApplicationEvidence"
INFLIGHT = "SumeragiV2AutonomousReservationCarrier"
PERSIST = "persist_native_amx_participant_application_evidence_under_publication_guard"
REPAIR = "persist_native_amx_participant_application_repair_targets_under_publication_guard"
PUBLICATION_SYMBOLS = (PERSIST, REPAIR)
WRITE_MANIFEST = "write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard"
READ_MANIFEST = "read_back_native_amx_plan_manifests_under_publication_guard"
READ_REPAIR = "read_back_native_amx_repair_target_manifests_under_publication_guard"
WRITE_RECEIPT = "write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard"
WRITE_LATEST = "write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard"
AUTHENTICATE = "authenticate_native_amx_participant_application_prepublication_under_publication_guard"
CLEANUP = "cleanup_native_amx_participant_application_evidence_under_publication_guard"
CONSUME = "consume_native_amx_publication_component_after_durable_publication"
CUSTODY = "Kura::reauthenticate_native_amx_prepublication"
CUSTODY_GUARDED = CUSTODY + "_under_publication_guards"

# Existing constructor/private-field bindings are retained verbatim in the ledger.
# These additional owners close the decoding, sizing and accounting delegations.
BINDINGS = (
    (NATIVE, TOKEN, "struct", "NativeAmxParticipantApplicationPrepublicationToken", (
        "original_kura: KuraInstanceIdentity",
        "identities: Vec<NativeAmxParticipantApplicationPrepublicationIdentity>",
    )),
    (NATIVE, TOKEN, "method", "NativeAmxParticipantApplicationPrepublicationToken::from_plan", (
        "original_kura: KuraInstanceIdentity",
        "if usize::try_from(plan.manifest_leaf_count).ok() != Some(identities.len())",
        "Some(Self {\n            original_kura,",
    )),
    (NATIVE, LEASE, "method", CUSTODY, ('fn reauthenticate_native_amx_prepublication(\n        &self,\n        token: &super::NativeAmxParticipantApplicationPrepublicationToken,\n        block: &super::SignedBlock,\n        manifest: &crate::sumeragi::exec::NativeAmxApplicationManifestV1,\n        finality: &super::V2FinalityArtifact,\n        frontiers: &[crate::state::AppliedNativeAmxParticipantFrontierMarker],\n    ) -> super::Result<()> {\n        self.ensure_canonical_storage_not_poisoned()?;\n        let mut fences = AcquiredKuraPublicationFences::new(self);\n        fences.prune = Some(self.prune_lock.lock());\n        self.ensure_prune_recovery_not_required()?;\n        fences.canonical = Some(self.canonical_chain_lock.lock());\n        fences.geometry = Some(self.lane_geometry_lock.lock());\n        fences.sidecar = Some(self.sidecar_lock.lock());\n        let result = self.reauthenticate_native_amx_prepublication_under_publication_guards(\n            token, block, manifest, finality, frontiers,\n        );\n        drop(fences);\n        result\n    }',)),
    (NATIVE, LEASE, "method", CUSTODY_GUARDED, (
        "if !token.original_kura.matches(self) {\n            return Err(",
        "if !token.authenticates_state_frontiers(block, manifest, finality, frontiers) {\n            return Err(",
        "v2_finality_artifact_with_archive_under_prune_and_canonical_guards(\n                token.application_block_height,\n            )?",
        "if header != block.header()\n            || super::HashOf::new(&durable_finality) != token.finality_artifact_hash\n        {\n            return Err(",
        "super::native_amx_participant_application_artifacts(\n            manifest,\n            token.finality_artifact_hash,\n        )",
        "if artifacts.len() != token.identities.len() {\n            return Err(",
        "for ((expected_manifest, expected_receipt), expected_identity) in\n            artifacts.iter().zip(&token.identities)",
        "authenticate_native_amx_participant_application_prepublication_under_publication_guards(\n                    expected_manifest, expected_receipt, false,\n                )?",
        "if actual != *expected_identity {\n                return Err(",
    )),
    (NATIVE, DATA, "method", "NativeAmxParticipantSettlement::try_from", (
        "Self::try_new(", "wire.lane_id", "wire.dataspace_id", "wire.lane_incarnation",
        "wire.participant_lane_block_height", "wire.authority_context_height",
        "wire.previous_native_settlement_hash", "wire.source_ids",
    )),
    (NATIVE, DATA, "method", "NativeAmxParticipantSettlement::try_deserialize", (
        "NativeAmxParticipantSettlementWire as norito::core::DeserializePayload",
        "archived.cast()", "Self::try_from(wire).map_err",
    )),
    (NATIVE, DATA, "method", "NativeAmxParticipantSettlement::json_deserialize", (
        "NativeAmxParticipantSettlementWire as norito::json::JsonDeserialize",
        "json_deserialize(parser)?", "Self::try_from(wire).map_err",
    )),
    (INFLIGHT, REPLICA, "fn", "verified_v2_finality_wire_hash_for_eviction", (
        "verified_kura_replica_authority_for_eviction(blocks_dir, height, canonical_hash)?",
        "authority.key.executed_block_wire_len", "authority.key.executed_block_wire_hash",
    )),
    (INFLIGHT, REPLICA, "fn", "verified_kura_replica_authority_for_eviction", (
        "decode_v2_finality_record_at(&path, &directory)?",
        "validate_v2_finality_record_at(&path, height, canonical_hash, &record)?",
        "retained_block_record_at_without_live_body(blocks_dir, height, canonical_hash)?",
        "retained_header != record.block_header", "validate_v2_finality_wire_bindings(",
        "verify_v2_finality_artifact_at(&path, &directory, &record.artifact, &read_identity)?",
        "executed_block_wire_len", "executed_block_wire_hash",
    )),
    (INFLIGHT, KURA, "fn", "set_transaction_entrypoint_index_entry", (
        "Self::insert_transaction_entrypoint_heights(&mut index, height, block)",
        "index.incomplete_heights.is_empty() && index.indexed_heights.len() == chain_len",
    )),
    (INFLIGHT, KURA, "fn", "update_disk_usage_delta", (
        "if before == after", "if after > before", "self.add_disk_usage_bytes(after - before)",
        "self.sub_disk_usage_bytes(before - after)",
    )),
    (INFLIGHT, KURA, "fn", "add_disk_usage_bytes", (
        "self.disk_usage_total_accounting.lock()", "self.add_disk_usage_bytes_locked(delta)",
    )),
    (INFLIGHT, KURA, "fn", "sub_disk_usage_bytes", (
        "self.disk_usage_total_accounting.lock()", "self.sub_disk_usage_bytes_locked(delta)",
    )),
    (INFLIGHT, KURA, "fn", "add_disk_usage_bytes_locked", (
        "self", ".disk_usage", "current.saturating_add(delta)", "self.add_total_disk_usage_bytes_locked(delta)",
    )),
    (INFLIGHT, KURA, "fn", "sub_disk_usage_bytes_locked", (
        "self", ".disk_usage", "current.saturating_sub(delta)", "self.sub_total_disk_usage_bytes_locked(delta)",
    )),
    (INFLIGHT, PHYSICAL, "method", "TotalDiskUsageMutation::with_resource_paths", (
        "self.bind_physical_target(PhysicalResourceTarget::Paths(paths))",
    )),
    (INFLIGHT, GUARD, "method", "TotalDiskUsageMutation::finish", (
        "self.published = true", "self.publish_physical_resources()",
    )),
    (INFLIGHT, GUARD, "method", "TotalDiskUsageMutation::drop", (
        "self.kura.finish_total_disk_usage_mutation(self.published)",
    )),
)
EXTRA_ITEMS = (
    (APPLY, "method", "V2ApplyService::validate_and_apply"),
    (KURA, "fn", AUTHENTICATE + "s"),
    (DATA, "struct", "NativeAmxParticipantSettlement"),
    (DATA, "method", "NativeAmxParticipantSettlement::try_new"),
    (DATA, "method", "NativeAmxParticipantSettlement::source_ids"),
    (KURA, "fn", "validate_native_amx_participant_application_receipt_artifact"),
    (KURA, "fn", "durable_block_payload_len_by_hash"),
    (KURA, "fn", "apply_finalized_merge_carrier_repairs"),
    (KURA, "fn", "persist_autonomous_lifecycle_bootstrap_with_authentication"),
    (KURA, "fn", "complete_autonomous_lifecycle_bootstrap"),
    (KURA, "fn", "persist_lane_payload_availability_certificate"),
    (KURA, "fn", "transition_autonomous_lane_entrypoint_claims_locked"),
    (KURA, "fn", "write_autonomous_lane_block_view_state_record_locked"),
    (KURA, "fn", "rebuild_native_amx_participant_receipt_latest_indexes_on_startup"),
)
SOURCE_RELATIVES = (
    Path("scripts/formal/sumeragi_v2_multilane_native_publication_contract.py"),
    Path("pytests/scripts/sumeragi_v2_multilane_native_publication_contract_test.py"),
    *(Path(p) for p in (KURA, DATA, REPLICA, PHYSICAL, GUARD, LEASE, TOKEN, APPLY,
                       "crates/iroha_core/src/native_amx.rs",
                       "crates/iroha_core/src/lane_consensus.rs",
                       "crates/iroha_data_model/src/merge.rs")),
)


def publication_branches(item, symbol, errors):
    """Split the actual completed-retry branch without matching comments/literals."""
    masked = _mask_rust_comments(item)
    matches = list(re.finditer(r"\bif\s+!publication_required\s*\{", masked))
    if len(matches) != 1:
        errors.append(f"Native publication {symbol} requires one exact completed-retry branch")
        return None
    start = matches[0].end()
    depth = 1
    for end in range(start, len(masked)):
        depth += (masked[end] == "{") - (masked[end] == "}")
        if depth == 0:
            return item[:matches[0].start()], item[start:end], item[end + 1:]
    errors.append(f"Native publication {symbol} has an unclosed completed-retry branch")
    return None


def new_publication_path(item, symbol, errors):
    """Retain common admission before the write path, excluding completed retry."""
    branches = publication_branches(item, symbol, errors)
    return branches[0] + branches[2] if branches is not None else None


def validate_phases(binding_items, errors):
    """Check each phase loop and retry branch independently, including reservation use."""
    for symbol in PUBLICATION_SYMBOLS:
        item = binding_items.get((KURA, "fn", symbol))
        if item is None:
            continue
        branches = publication_branches(item, symbol, errors)
        if branches is None:
            continue
        prefix, retry, publish = map(_code, branches)
        repair = symbol == REPAIR
        def require(source, relation):
            if _code(relation) not in source:
                errors.append(f"Native prepublication {symbol} phase relation missing {relation!r}" + (" (cleanup-only-after-WSV)" if "if permit_cleanup" in relation else ""))
        preflight = ("preflight_native_amx_participant_application_repair_targets_under_publication_guard(plan, target_indices)" if repair else "preflight_native_amx_participant_application_plan_under_publication_guard(plan)")
        require(prefix, "let route_preflights = self." + preflight + "?;")
        require(prefix, "let publication_required = self.ensure_native_amx_publication_capacity_under_publication_guard(block, plan, " + ("target_indices" if repair else "&all_targets") + ")?;")
        if prefix.find(_code(preflight)) > prefix.find("ensure_native_amx_publication_capacity_under_publication_guard"):
            errors.append(f"Native prepublication {symbol} capacity precedes exact route preflight")
        require(retry, f"self.{READ_REPAIR}(plan, target_indices)?;" if repair else f"self.{READ_MANIFEST}(plan)?;")
        if any(_code(token) in retry for token in (WRITE_MANIFEST, WRITE_RECEIPT, WRITE_LATEST, CONSUME)):
            errors.append(f"Native prepublication {symbol} completed retry repeats publication or capacity consumption")
        retry_order = (READ_REPAIR if repair else READ_MANIFEST, AUTHENTICATE,
                       CLEANUP if repair else "NativeAmxParticipantApplicationPrepublicationToken::from_plan",
                       "returnOk(")
        positions = [retry.find(_code(token)) for token in retry_order]
        if -1 in positions or positions != sorted(positions):
            errors.append(f"Native prepublication {symbol} completed retry authentication order changed")
        if not repair and retry.find(CLEANUP) < retry.find("NativeAmxParticipantApplicationPrepublicationToken::from_plan"):
            errors.append(f"Native prepublication {symbol} completed retry cleans before complete token")
        if repair:
            require(retry, f"for &index in target_indices {{ let (manifest, receipt) = &plan.artifacts[index]; let _ = self.{AUTHENTICATE}(manifest, receipt, true)?; self.{CLEANUP}(receipt)?; }} return Ok(target_indices.len());")
        else:
            require(retry, f"for (manifest, receipt) in &plan.artifacts {{ identities.push(self.{AUTHENTICATE}(manifest, receipt, mode.requires_post_apply_metadata())?); }}")
            require(retry, "NativeAmxParticipantApplicationPrepublicationToken::from_plan(self.instance_identity(), plan, identities,)")
            require(retry, f"if permit_cleanup {{ for (_, receipt) in &plan.artifacts {{ self.{CLEANUP}(receipt)?; }} }} return Ok(token);")
        loop = "for &index in target_indices { let (manifest, receipt) = &plan.artifacts[index];" if repair else "for (manifest, receipt) in &plan.artifacts {"
        policy = "true" if repair else "permit_cleanup"
        require(publish, f"{loop} self.{WRITE_MANIFEST}(manifest, {policy})?; self.{CONSUME}(receipt, NativeAmxPublicationComponent::Manifest, manifest.encode_framed()?.len())?; }}")
        require(publish, f"{loop} self.{WRITE_RECEIPT}(receipt, manifest, {policy})?; self.{CONSUME}(receipt, NativeAmxPublicationComponent::Receipt, receipt.encode_framed()?.len())?; }}")
        latest_loop = "for (&index, preflight) in target_indices.iter().zip(route_preflights.iter()) { let (manifest, receipt) = &plan.artifacts[index];" if repair else "for ((manifest, receipt), preflight) in plan.artifacts.iter().zip(route_preflights.iter()) {"
        require(publish, f"{latest_loop} self.{WRITE_LATEST}(receipt, manifest, {policy}, preflight)?; self.{CONSUME}(receipt, NativeAmxPublicationComponent::Latest, norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt(receipt))?.len())?; }}")
        if repair:
            require(publish, f"{loop} let _ = self.{AUTHENTICATE}(manifest, receipt, true)?; }} for &index in target_indices {{ let (_, receipt) = &plan.artifacts[index]; self.{CLEANUP}(receipt)?; }}")
        else:
            require(publish, "if plan.artifacts.iter().any(|(manifest, _)| !manifest_readback.authenticates(plan, manifest)) { return Err(")
            require(publish, "NativeAmxParticipantApplicationPrepublicationToken::from_plan(self.instance_identity(), plan, identities,)")
            require(publish, f"if permit_cleanup {{ for (_, receipt) in &plan.artifacts {{ self.{CLEANUP}(receipt)?; }} }}")


def validate_owners(root, models, errors, rust_binding_item):
    """Bind typed bounds, bounded body sizing and once-only physical accounting."""
    items = {}
    for model, path, kind, symbol, tokens in BINDINGS:
        owners = [m for m in models if m.get("module") == model]
        rows = owners[0].get("production_symbols", ()) if len(owners) == 1 else ()
        matches = [b for b in rows if (b.get("path"), b.get("kind"), b.get("symbol")) == (path, kind, symbol)]
        if len(matches) != 1 or tuple(matches[0].get("required_tokens", ())) != tokens:
            errors.append(f"Native publication ledger owner differs for {symbol}")
        item = rust_binding_item(root, path, kind, symbol, "Native publication owner", errors)
        if item is not None:
            items[symbol] = item
            for token in tokens:
                if _code(token) not in _code(item):
                    errors.append(f"Native publication owner {symbol} missing relation {token!r}")
    for path, kind, symbol in EXTRA_ITEMS:
        item = rust_binding_item(root, path, kind, symbol, "Native publication owner", errors)
        if item is not None:
            items[symbol] = item
    def require(symbol, relation):
        if symbol in items and _code(relation) not in _code(items[symbol]):
            errors.append(f"Native publication owner {symbol} missing relation {relation!r}")
    private = items.get("NativeAmxParticipantSettlement", "")
    if re.search(r"\bpub(?:\([^)]*\))?\s+\w+\s*:", _mask_rust_comments(private)):
        errors.append("Native participant bounded settlement exposes mutable fields")
    require("NativeAmxParticipantSettlement::try_new", "if source_ids.is_empty() || source_ids.len() > NATIVE_AMX_GROUP_SOURCES_MAX { return Err(")
    require("NativeAmxParticipantSettlement::try_new", "source_ids.iter().copied().collect::<std::collections::BTreeSet<_>>().len() != source_ids.len()")
    require("NativeAmxParticipantSettlement::source_ids", "&self.source_ids")
    require("NativeAmxParticipantSettlement::try_from", "Self::try_new(wire.lane_id, wire.dataspace_id, wire.lane_incarnation, wire.participant_lane_block_height, wire.authority_context_height, wire.previous_native_settlement_hash, wire.source_ids)")
    receipt = "validate_native_amx_participant_application_receipt_artifact"
    require(receipt, "let settlement_source_ids = settlement.source_ids(); if settlement_source_ids != artifact.source_ids ||")
    for field in ("source_ids", "entrypoint_hashes", "result_hashes", "results"):
        require(receipt, f"artifact.{field}.len() != artifact.entrypoint_indices.len()")
    for path, relation in (
        (DATA, "pub const NATIVE_AMX_GROUP_SOURCES_MAX: usize = 4_096;"),
        ("crates/iroha_data_model/src/merge.rs", "pub const MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_096;"),
        ("crates/iroha_core/src/lane_consensus.rs", "pub(crate) const MAX_LANE_EXECUTABLE_ENTRYPOINTS: usize = MAX_MERGE_EXECUTION_ENTRYPOINTS;"),
        ("crates/iroha_core/src/native_amx.rs", "pub(crate) const MAX_NATIVE_AMX_PARTICIPANT_CONTROL_SOURCES: usize = crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS;"),
    ):
        if _code(relation) not in _code((root / path).read_text()):
            errors.append(f"Native participant source cap parity changed in {path}")
    require("durable_block_payload_len_by_hash", "self.verified_v2_finality_wire_hash_for_eviction(&store.path_to_blockchain, height_u64, hash)?")
    require("durable_block_payload_len_by_hash", "if wire_len != index.length { return Err(")
    require("durable_block_payload_len_by_hash", "Ok(Some((height_u64, wire_len)))")
    require("apply_finalized_merge_carrier_repairs", "self.set_transaction_entrypoint_index_entry(height, &repair.block, durable_count)")
    bootstrap = "persist_autonomous_lifecycle_bootstrap_with_authentication"
    require(bootstrap, "self.begin_total_disk_usage_mutation().with_resource_paths(vec![path.clone()])")
    require(bootstrap, "self.update_disk_usage_delta(0, next_len); accounting_mutation.finish();")
    if "update_total_disk_usage_delta" in _code(items.get(bootstrap, "")):
        errors.append("Native bootstrap publication double-counts the total disk delta")
    require("complete_autonomous_lifecycle_bootstrap", "self.read_lane_application_receipt(payload.origin_proposal.descriptor.lane_id, payload.origin_proposal.descriptor.lane_block_height)?.is_some_and(|receipt| receipt.proposal == payload.origin_proposal)")
    require("transition_autonomous_lane_entrypoint_claims_locked", "self.begin_total_disk_usage_mutation().with_resource_children(plan.len())")
    require("transition_autonomous_lane_entrypoint_claims_locked", "accounting_mutation.resource_child(vec![claim.path.clone(), claim.temp_path.clone()])")
    require("write_autonomous_lane_block_view_state_record_locked", "self.begin_total_disk_usage_mutation().with_resource_paths(vec![path.to_path_buf(), temp_path.clone()])")
    require("persist_lane_payload_availability_certificate", "let slot_is_certified = self.autonomous_lane_slot_is_certified_locked(&entry, lane_block_height)?;")
    require("persist_lane_payload_availability_certificate", "if slot_is_certified { return Err(")
    startup = "rebuild_native_amx_participant_receipt_latest_indexes_on_startup"
    require(startup, "self.complete_native_amx_evidence_prune_intent_locked(recovery.guard(), &entry, &namespace)?; self.recover_native_amx_evidence_publication_temp_locked(recovery.guard(), &entry, &namespace, NativeAmxEvidenceRecoveryPhase::Startup)?; recovery.finish();")
    require(startup, "self.prune_native_amx_evidence_pairs_locked(lane_resources.guard(), &entry, &namespace)?;")
    validate_participant_custody(items, errors)


def validate_participant_custody(items, errors):
    """Keep original identity/readback and lock release before live State staging."""
    wrapper = _code(items.get(CUSTODY, ""))
    order = (
        "fences.prune = Some(self.prune_lock.lock());",
        "fences.canonical = Some(self.canonical_chain_lock.lock());",
        "fences.geometry = Some(self.lane_geometry_lock.lock());",
        "fences.sidecar = Some(self.sidecar_lock.lock());",
        "let result = self.reauthenticate_native_amx_prepublication_under_publication_guards(",
        "drop(fences);",
    )
    positions = [wrapper.find(_code(token)) for token in order]
    if -1 in positions or positions != sorted(positions):
        errors.append("Native participant custody lost original lock/release order")
    guarded = _code(items.get(CUSTODY_GUARDED, ""))
    for symbol in (CUSTODY_GUARDED, AUTHENTICATE + "s"):
        owner = _code(items.get(symbol, ""))
        if any(token in owner for token in (".lock(", ".try_lock(", ".try_lock_or_wait(")):
            errors.append(f"Native participant custody {symbol} reacquires a held publication fence")
    if guarded.count("Ok(())") != 1 or not guarded.endswith("Ok(())}"):
        errors.append("Native participant custody requires complete readback before its sole success")
    live = _code(items.get("V2ApplyService::validate_and_apply", ""))
    if _code('if let Some(token) = native_amx_prepublication.as_ref() { self.kura .reauthenticate_native_amx_prepublication( token, committed_block.as_ref(), &native_amx_manifest, artifact, &native_amx_frontiers, ).map_err(|error| { V2ApplyError::committed_recovery_required("pre-WSV Native AMX participant custody reauthentication", &error,) })?; }') not in live:
        errors.append("Native participant custody lost its exact live owner/projection join")
    order = ("token.authenticates_state_frontiers(",
             ".reauthenticate_native_amx_prepublication(",
             ".authorize_execution_output_publication(",
             ".apply_without_execution_with_verified_v2_finality(")
    positions = [live.find(token) for token in order]
    if -1 in positions or positions != sorted(positions):
        errors.append("Native participant custody must finish before live State staging")
    token = items.get("NativeAmxParticipantApplicationPrepublicationToken", "")
    if "{" in token and re.search(r"\bpub(?:\([^)]*\))?\s+\w+\s*:",
                                  _mask_rust_comments(token.split("{", 1)[1])):
        errors.append("Native participant custody exposes caller-constructed identity")
