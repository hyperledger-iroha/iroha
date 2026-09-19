"""Executable positive/mutation controls for Native preparation/accounting owners."""
from __future__ import annotations

import ast
import importlib.util
import sys
from pathlib import Path

import pytest


def support():
    path = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")
    spec = importlib.util.spec_from_file_location("native_preparation_support", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def validate(fixture):
    root, _, checker, models = fixture
    errors = []
    with checker._reviewed_rust_source_cache():
        checker.native_preparation_contract.validate_native_preparation_contract(
            root, models, errors, checker._rust_binding_item,
        )
    return tuple(errors)


@pytest.fixture
def fixture(tmp_path):
    helper = support()
    checker = helper.load_checker()
    c = checker.native_preparation_contract
    helper.copy_reviewed_source_fixture_with_includes(tmp_path, checker, {
        *(p for p in c.NATIVE_PREPARATION_SOURCE_RELATIVES if p.suffix == ".rs"),
        checker.REVIEWED_RUST_SOURCE_HELPER_RELATIVE,
        checker.REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE,
    })
    result = tmp_path, helper, checker, helper.canonical_models()
    assert validate(result) == ()
    return result


def test_native_preparation_accepts_actual_owners(fixture):
    assert validate(fixture) == ()


def test_native_preparation_is_connected_to_release_gate():
    checker = support().load_checker()
    tree = ast.parse(Path(checker.__file__).read_text())
    owners = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
    assert sum(isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
               and isinstance(n.func.value, ast.Name)
               and n.func.value.id == "native_preparation_contract"
               and n.func.attr == "validate_native_preparation_contract"
               for n in ast.walk(owners["_validate"])) == 1
    assert any(isinstance(n, ast.Starred) and isinstance(n.value, ast.Attribute)
               and isinstance(n.value.value, ast.Name)
               and n.value.value.id == "native_preparation_contract"
               and n.value.attr == "NATIVE_PREPARATION_SOURCE_RELATIVES"
               for n in ast.walk(owners["source_manifest_sha256"]))


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "weakened"])
def test_native_preparation_rejects_each_owner_ledger_mutation(fixture, mutation):
    _, _, checker, models = fixture
    c = checker.native_preparation_contract
    model = next(m for m in models if m["module"] == c.MODEL)
    baseline = model["production_symbols"]
    # The source is immutable throughout these ledger-only mutations. Preserve
    # the real parsed items from the positive check rather than parse Kura again
    # for every independent ledger row.
    items = {}
    def cached_item(root, path, kind, symbol, label, errors):
        key = path, kind, symbol
        if key not in items:
            items[key] = checker._rust_binding_item(root, path, kind, symbol, label, errors)
        return items[key]
    def ledger_errors():
        errors = []
        with checker._reviewed_rust_source_cache():
            c.validate_native_preparation_contract(fixture[0], models, errors, cached_item)
        return errors
    assert ledger_errors() == []
    for path, kind, symbol, _ in c.PREPARATION_OWNER_BINDINGS:
        index = next(i for i, r in enumerate(baseline)
                     if (r["path"], r["kind"], r["symbol"]) == (path, kind, symbol))
        altered = list(baseline)
        if mutation == "missing":
            altered.pop(index)
        elif mutation == "duplicate":
            altered.append(dict(baseline[index]))
        else:
            altered[index] = dict(baseline[index], required_tokens=[])
        model["production_symbols"] = altered
        assert any("ledger owner" in e or "reviewed tokens changed" in e for e in ledger_errors()), symbol
    model["production_symbols"] = baseline


@pytest.mark.parametrize("owner,symbol,old,new", [
    ("BODY_STORE", "verify_origin_block_signature", "context.leader(block.header().view_change_index())", "context.leader(0)"),
    ("BODY_STORE", "verify_origin_block_signature", "signatures.next().is_some() || signature.index() != expected_index", "false"),
    ("BODY_STORE", "validate_envelope", "verify_origin_block_signature(&self.context, &block, &self.signature_policy)?", "Ok::<(), V2BodyStoreError>(())?"),
    ("CONTROLS", "validate_execution_context_header", "bundle.native_lane_decisions.is_some()", "false"),
    ("BLOCK", "prepare_native_candidate", "verify_origin_block_signature(", "unchecked_origin_signature("),
    ("BLOCK", "prepare_native_candidate", "length > frozen.da_layout.max_payload_size_bytes", "false"),
    ("BLOCK", "prepare_native_candidate", "Self::validate_static_state_dependent(", "unchecked_static_state("),
    ("BLOCK", "prepare_native_candidate", "Self::validate_static_with_snapshot(", "unchecked_snapshot("),
    ("BLOCK", "prepare_native_candidate", "generation != state.state_view_generation()", "false"),
    ("NATIVE_SOURCE", "preparation_input", "self.is_current()", "true"),
    ("NATIVE_SOURCE", "preparation_input", "self.generation", "self.state.state_view_generation()"),
    ("BLOCK", "prepare_native_candidate", "native: Some(native)", "native: None"),
    ("NATIVE_STAGE", "into_preparation_parts", "context: self.context", "context: other_context"),
    ("NATIVE_STAGE", "into_preparation_parts", "verify_execution_output_seal(&self.carrier)", "verify_execution_output_seal(&other_carrier)"),
    ("NATIVE_STAGE", "into_preparation_parts", "validate_native_output_source(&self.carrier)", "validate_native_output_source(&other_carrier)"),
    ("NATIVE_STAGE", "validate_native_output_carrier", "self.validate_native_output_source(block)", "self.validate_native_output_source(other_block)"),
    ("NATIVE_STAGE", "validate_native_output_source", "!block.external_entrypoints_slice().is_empty()", "block.external_entrypoints_slice().is_empty()"),
    ("NATIVE_STAGE", "retains_state", "Arc::ptr_eq(seal, &self.seal)", "true"),
    ("NATIVE_STAGE", "retains_state", "source.decisions() == wire.decisions", "true"),
    ("PREFIX", "retains_closed_state", "native.retains_state(state)", "true"),
    ("TAIL", "finalize_owned_execution_metadata", "            block,\n            state,\n            routes,", "            other_block,\n            state,\n            routes,"),
    ("TAIL", "finalize_common_execution_metadata", "routes.len() != block.network_entrypoint_count()", "routes.len() == block.network_entrypoint_count()"),
    ("TAIL", "finalize_common_execution_metadata", "evaluate_nexus_autoscale(block, fragments)", "evaluate_nexus_autoscale(block, 0)"),
    ("TAIL", "finalize_common_execution_metadata", "state.finalize_axt_asset_incarnations()", "other.finalize_axt_asset_incarnations()"),
    ("TAIL", "finalize_common_execution_metadata", "Self::validate_advertised_axt_post_state(advertised_policy, &policy)?;", "// Self::validate_advertised_axt_post_state(advertised_policy, &policy)?;"),
    ("NATIVE_METADATA", "seal_native_execution_outputs", "verify_native_execution_metadata(block, executions)", "verify_native_execution_metadata(block, other_executions)"),
    ("NATIVE_METADATA", "seal_native_execution_outputs", "verify_native_execution_metadata(source, executions)", "verify_native_execution_metadata(source, other_executions)"),
    ("NATIVE_METADATA", "seal_native_execution_outputs", "                    routes,", "                    other_routes,"),
    ("NATIVE_METADATA", "seal_native_execution_outputs", "std::iter::empty::<HashOf<TransactionEntrypoint>>()", "ordinary_membership.into_iter()"),
    ("NATIVE_METADATA", "seal_native_execution_outputs", "verify_execution_output_seal(block)", "verify_execution_output_seal(other_block)"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "executions.len() != routes.len()", "executions.len() == routes.len()"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "plan.coordinator_route() != *route", "plan.coordinator_route() == *route"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "slot.lane_incarnation,", "other_incarnation,"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "!= execution.settlement_hash", "== execution.settlement_hash"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "!Self::native_settlement_requires_relay(commitment)?", "Self::native_settlement_requires_relay(commitment)?"),
    ("NATIVE_METADATA", "native_settlement_requires_relay", "|| !commitment.nexus_fee_receipts.is_empty()", "&& !commitment.nexus_fee_receipts.is_empty()"),
    ("NATIVE_METADATA", "native_settlement_requires_relay", "!commitment.total_xor_due.is_zero()", "commitment.total_xor_due.is_zero()"),
    ("NATIVE_METADATA", "native_settlement_requires_relay", "commitment.tx_count == 0", "commitment.tx_count != 0"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "decision.manifest.byte_len,", "0,"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "with_lane_block_descriptor_hash(Some(descriptor_hash))", "with_lane_block_descriptor_hash(None)"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "entry.dsid == commitment.dataspace_id", "entry.dsid != commitment.dataspace_id"),
    ("NATIVE_METADATA", "native_execution_finality_statements", "envelope.lane_finality_statement()", "unchecked_statement()"),
    ("NATIVE_STAGE", "verify_native_execution_metadata", "!self.settlement_accumulator.is_empty()", "self.settlement_accumulator.is_empty()"),
    ("NATIVE_STAGE", "verify_native_execution_metadata", "executions.len() != seal.settlement_hashes.len()", "executions.len() == seal.settlement_hashes.len()"),
    ("NATIVE_STAGE", "verify_native_execution_metadata", "execution.source != *source", "execution.source == *source"),
    ("NATIVE_STAGE", "verify_native_execution_metadata", "execution.authenticated_signed_replay_alias != *alias", "execution.authenticated_signed_replay_alias == *alias"),
    ("NATIVE_STAGE", "verify_native_execution_metadata", "execution.settlement_hash != *settlement", "execution.settlement_hash == *settlement"),
    ("APPLY", "validate_candidate", "body.clone(),", "other_body.clone(),"),
    ("APPLY", "validate_candidate", "SumeragiV2ValidationContext::from_height_context(context)", "SumeragiV2ValidationContext::from_height_context(other_context)"),
    ("APPLY", "validate_candidate", "prepared.native_amx_manifest(),", "other.native_amx_manifest(),"),
    ("APPLY", "validate_candidate", "Ok(prepared.execution_prefix_commitment())", "Ok(other.execution_prefix_commitment())"),
    ("BLOCK", "validate_and_prepare_sumeragi_v2_candidate_keep_voting_block", "context.height == block.header().height().get()", "context.height <= block.header().height().get()"),
    ("BLOCK", "validate_and_prepare_sumeragi_v2_candidate_keep_voting_block", "context.network_id == *state.network_id_ref()", "context.network_id != *state.network_id_ref()"),
    ("BLOCK", "validate_and_prepare_sumeragi_v2_candidate_keep_voting_block", "context.id() == validation_context.context_id", "context.id() != validation_context.context_id"),
    ("BLOCK", "validate_and_prepare_sumeragi_v2_candidate_keep_voting_block", "context.roster.iter().map(|entry| &entry.validator)", "other.roster.iter().map(|entry| &entry.validator)"),
    ("PREPARED", "prepare", "execution_prefix::prepare(input)", "execution_prefix::prepare(other_input)"),
    ("PREFIX", "capture", "state.verify_execution_output_seal(block)?;", "// state.verify_execution_output_seal(block)?;"),
    ("PREFIX", "capture", "state.staged_merge_entry.is_some()", "state.staged_merge_entry.is_none()"),
    ("PREFIX", "capture", "native.retains_state(&state)", "true"),
    ("PREFIX", "capture", "context.native_lane_decisions.is_some()", "context.native_lane_decisions.is_none()"),
    ("PREFIX", "capture", "verify_cached_ordinary_witness_content(&verified_inventory)", "verify_cached_ordinary_witness_content(&other_inventory)"),
    ("PREFIX", "capture", "execution_commitment_from_validated_block(witness,", "execution_commitment_from_validated_block(other_witness,"),
    ("PREFIX", "capture", "witness, &manifest, &lanes, block", "witness, &other_manifest, &lanes, block"),
    ("PREFIX", "capture", "witness, &manifest, &lanes, block", "witness, &manifest, &other_lanes, block"),
    ("PREFIX", "capture", ".replace(output_capacity::ExecutionOutputPlanState::Captured)", ".take()"),
    ("PREFIX", "capture", "Arc::ptr_eq(&inventory, &verified_inventory)", "Arc::ptr_eq(&inventory, &inventory)"),
    ("PREFIX", "capture", "fastpq_witness_context: state.fastpq_witness_context.take()", "fastpq_witness_context: None"),
    ("PREFIX", "prepare", "PrefixPreparation::capture(state, &valid, native)?", "PrefixPreparation::capture(other_state, &valid, native)?"),
    ("PREFIX", "prepare", "Err(error) => Err((Box::new(valid.into()), error))", "Err(error) => retain_partial(error)"),
    ("PREFIX", "prepare_world_effects", "!self.prefix.retains_closed_state(state)", "false"),
    ("PREFIX", "prepare_world_effects", "state.verify_lane_consensus_contexts_publication()?;", "// state.verify_lane_consensus_contexts_publication()?;"),
    ("JOURNALS", "prepare_journals", "prefix: &source_prefix,", "prefix: &other_prefix,"),
    ("SEAL", "seal_execution_outputs", "                sources,", "                sources: other_sources,"),
    ("CAPACITY", "native_amx_publication_plan_under_prune_and_canonical_guards", "NativeAmxPublicationStorage::Active", "NativeAmxPublicationStorage::JournalPhysical"),
    ("CAPACITY", "native_amx_publication_plan_for_storage_under_prune_and_canonical_guards", "from_result_bearing_block_and_merge_entry(block, merge_entry)", "from_result_bearing_block_and_merge_entry(block, None)"),
    ("CAPACITY", "native_amx_publication_plan_for_storage_under_prune_and_canonical_guards", "            &manifest,", "            &other_manifest,"),
    ("CAPACITY", "native_amx_publication_plan_for_storage_under_prune_and_canonical_guards", "if routes.insert(route, capacity).is_some()", "if routes.insert(route, capacity).is_none()"),
    ("CAPACITY", "native_amx_route_publication_capacity_for_storage_locked", "&entry, manifest, receipt,", "&entry, other_manifest, receipt,"),
    ("CAPACITY", "native_amx_route_publication_capacity_for_storage_locked", "self.require_native_amx_reservation_physical_target(&target)?;", "// self.require_native_amx_reservation_physical_target(&target)?;"),
    ("CAPACITY", "native_amx_route_publication_capacity_at_target_locked", "u64::try_from(expected_manifest.encode_framed()?.len())?", "0"),
    ("CAPACITY", "native_amx_route_publication_capacity_at_target_locked", "u64::try_from(expected_receipt.encode_framed()?.len())?", "0"),
    ("CAPACITY", "native_amx_route_publication_capacity_at_target_locked", "u64::try_from(norito::encode_canonical(&expected_latest)?.len())?", "0"),
    ("CAPACITY", "native_amx_route_publication_capacity_at_target_locked", "component_allocation_bytes.insert(NativeAmxPublicationComponent::Manifest, 0)", "component_allocation_bytes.insert(NativeAmxPublicationComponent::Receipt, 0)"),
    ("CAPACITY", "native_amx_route_publication_capacity_at_target_locked", "if !outstanding_components.contains(kind)", "if outstanding_components.contains(kind)"),
    ("CAPACITY", "native_amx_route_publication_capacity_at_target_locked", "intent.protected_latest.identity != expected_latest", "intent.protected_latest.identity == expected_latest"),
    ("CAPACITY", "native_amx_route_publication_capacity_at_target_locked", "Some(intent) => u64::try_from(norito::encode_canonical(&intent)?.len())?", "Some(_intent) => 0"),
    ("CAPACITY", "admit_native_amx_publication_capacity_plan", "Some(*other_carrier) != replaced", "Some(*other_carrier) == replaced"),
    ("CAPACITY", "admit_native_amx_publication_capacity_plan", "old.component_bytes != new.component_bytes", "old.component_bytes == new.component_bytes"),
    ("CAPACITY", "admit_native_amx_publication_capacity_plan", "new.prune_journal_bytes > old.prune_journal_bytes", "new.prune_journal_bytes < old.prune_journal_bytes"),
    ("CAPACITY", "lane_publication_budget_reserved_bytes", "merge.checked_add(native)", "merge.checked_add(0)"),
    ("ORDINARY", "lane_artifact_required_bytes_for_block", "Ok(total)", "Ok(total.saturating_add(self.native_amx_publication_capacity_reserved_bytes()?))"),
    ("KURA", "check_storage_budget", ".saturating_add(lane_publication_reservations)", ".saturating_add(0)"),
])
def test_native_preparation_rejects_semantic_mutation(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    path = root / getattr(checker.native_preparation_contract, owner)
    helper.replace_once_after(path, f"fn {symbol}", old, new)
    errors = validate(fixture)
    assert any("executable relation" in e or "duplicates Native reservation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


def test_native_preparation_rejects_late_budget_admission(fixture):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.DURABLE
    helper.replace_once_after(path, "fn store_block_durable(",
        "self.check_storage_budget(block, merge_entry)?;\n        if let Some(owner) = &mut native_capacity {\n            owner.publish_pending_index()?;\n        }",
        "if let Some(owner) = &mut native_capacity {\n            owner.publish_pending_index()?;\n        }\n        self.check_storage_budget(block, merge_entry)?;")
    assert any("missing or reorders executable relation" in e for e in validate(fixture))


@pytest.mark.parametrize("owner,old,new", [
    ("NativeAmxRoutePublicationCapacity", "self.prune_journal_bytes", "0"),
    ("NativeAmxRoutePublicationCapacity", "self.component_allocation_bytes.get(kind)", "self.component_bytes.get(kind)"),
    ("NativeAmxPublicationCapacityReservation", "self.index_additional_bytes", "0"),
])
def test_native_preparation_rejects_component_or_index_charge_loss(fixture, owner, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.CAPACITY,
                              f"impl {owner} {{", old, new)
    assert any("executable relation" in e for e in validate(fixture))


@pytest.mark.parametrize("old,new", [
    ("self.lane_publication_budget_reserved_bytes()?", "self.post_wsv_lane_artifact_budget_reserved_bytes()?"),
    ("self.certified_bundle_capacity_reserved_bytes()?", "0"),
    (".checked_add(pending_canonical_bytes)", ".checked_add(0)"),
    ("bytes.checked_add(additional_unreserved_stable_bytes)", "bytes.checked_add(0)"),
    ("bytes.checked_add(physical_and_transient)", "bytes.checked_add(0)"),
    ("bytes.checked_add(stable_terminal_reservations)", "bytes.checked_add(0)"),
    ("bytes.checked_add(lane_publication_reservations)", "bytes.checked_add(0)"),
    ("bytes.checked_add(certified_bundle_reservations)", "bytes.checked_add(0)"),
    ("bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes())", "bytes.checked_add(0)"),
    ("if required > self.max_disk_usage_bytes", "if required < self.max_disk_usage_bytes"),
    ("let lane_publication_reservations =", "return Ok(()); let lane_publication_reservations ="),
    ("if required > self.max_disk_usage_bytes", "let required = 0; if required > self.max_disk_usage_bytes"),
])
def test_native_preparation_terminal_capacity_preserves_all_reserved_families(fixture, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.AUTONOMOUS,
        "fn validate_configured_autonomous_mutation_disk_peak_with_reservation_deltas_locked(",
        old, new)
    errors = validate(fixture)
    assert any("executable relation" in e or "early success or replaced total" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("owner,symbol", [("PREFIX", "ValidatedExecutionPrefix"), ("PREFIX", "PrefixPreparation"), ("OUTPUT", "SealedExecutionOutputs")])
def test_native_preparation_rejects_public_custody_fields(fixture, owner, symbol):
    root, helper, checker, _ = fixture
    path = root / getattr(checker.native_preparation_contract, owner)
    source = path.read_text()
    anchor = source.index(f"struct {symbol}")
    field = source.index("\n    ", source.index("{", anchor)) + 5
    source = source[:field] + "pub(crate) " + source[field:]
    path.write_text(source)
    assert any("forgeable owner fields" in error for error in validate(fixture))


def test_native_preparation_rejects_source_capture_after_metadata_tail(fixture):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.PREFIX
    helper.replace_once_after(path, "fn prepare<'state>",
        "PrefixPreparation::capture(state, &valid, native)?;",
        "prepare_deterministic_carrier_metadata(); PrefixPreparation::capture(state, &valid, native)?;")
    # The additional pre-capture tail is forbidden even when the original owner
    # calls remain in order later in the function.
    assert any("before prefix capture" in error for error in validate(fixture))


@pytest.mark.parametrize("extra", [
    "state.stage_ordinary_lane_frontiers(block);",
    "state.drain_lane_execution_settlement();",
    "crate::sumeragi::witness::exec_witness_guard();",
])
def test_native_metadata_refuses_second_execution_or_ordinary_tail(fixture, extra):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.NATIVE_METADATA,
        "fn seal_native_execution_outputs(",
        "let advertised_fragments = block.committed_fragment_count();",
        extra + " let advertised_fragments = block.committed_fragment_count();")
    assert any("forbidden executable relation" in error for error in validate(fixture))


def test_native_common_metadata_refuses_policy_before_autoscale(fixture):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.TAIL,
        "fn finalize_common_execution_metadata(",
        "state.finalize_axt_asset_incarnations()",
        "state.finalize_axt_policy_transition_ratchets()?; state.finalize_axt_asset_incarnations()")
    # The original calls still exist later: the first policy mutation cannot
    # precede asset/autoscale processing in this one common owner.
    assert any("metadata before" in error for error in validate(fixture))


@pytest.mark.parametrize("owner,symbol,old,new", [
    pytest.param("CONTROLS", "prepare_native_execution_controls",
                 "block.header().da_proof_policies_hash() != Some(HashOf::new(&expected_da_policy))",
                 "block.header().da_proof_policies_hash() != block.header().da_proof_policies_hash()",
                 id="active-da-policy"),
    pytest.param("NATIVE_FINALIZED", "project",
                 "carrier: block.canonical_resultless_proposal()", "carrier: carrier_without_controls(block)",
                 id="complete-finalized-projection"),
    pytest.param("NATIVE_STAGE", "validate_native_pristine_control_owner",
                 "!std::ptr::eq(self.state_ref, state)", "!std::ptr::eq(self.state_ref, self.state_ref)",
                 id="original-state-owner"),
    pytest.param("CONTROLS", "prepare_native_execution_controls",
                 "Self::validate_npos_effects_with_state(block, state, Some(frozen.mode), Some(frozen))?;",
                 "// Self::validate_npos_effects_with_state(block, state, Some(frozen.mode), Some(frozen))?;",
                 id="authenticated-npos-controls"),
    pytest.param("NATIVE_SOURCE", "record_execution",
                 "carrier != *self.input", "carrier.header() != self.input.header()",
                 id="complete-original-carrier"),
    pytest.param("NATIVE_SOURCE", "record_execution",
                 "record_native_lane_decision_batch(carrier, self.groups, context)",
                 "record_native_lane_decision_batch(carrier, self.groups.clone(), context)",
                 id="original-source-custody"),
    pytest.param("NATIVE_SOURCE", "stage_with_start_hooks",
                 "crate::block::native_lane_batch_for_scratch(&self.input)",
                 "crate::block::native_lane_batch_for_execution(&self.input)",
                 id="scratch-cannot-drop-controls"),
    pytest.param("NATIVE_STAGE", "validate_native_lane_stage_membership",
                 "HashOf::new(&self.staged_queue_plan_admissions) != seal.queue_plan_admissions_hash",
                 "HashOf::new(&self.staged_queue_plan_admissions) == seal.queue_plan_admissions_hash",
                 id="sealed-control-rejoin"),
    pytest.param("CONTROLS", "finalize_native_execution_contexts",
                 "finalize_lane_consensus_contexts(block, Some(context.context()))",
                 "finalize_lane_consensus_contexts(block, None)",
                 id="authenticated-suffix-opening"),
])
def test_native_control_owner_rejects_semantic_mutation(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner),
                              f"fn {symbol}", old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


def test_native_control_recording_refuses_capture_before_final_contexts(fixture):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.NATIVE_STAGE
    helper.replace_once_after(path, "fn record_native_lane_decision_batch(",
        "crate::block::ValidBlock::finalize_native_execution_contexts(",
        "overlay.capture_exec_witness().map_err(invalid)?; "
        "crate::block::ValidBlock::finalize_native_execution_contexts(")
    # Keeping the original correct tail must not mask an earlier reset/capture.
    errors = validate(fixture)
    assert any("recorder lifecycle" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,symbol,old,new", [
    ("PREFIX", "retains_carrier", "native.retains_carrier(block, context)", "true"),
    ("NATIVE_STAGE", "retains_carrier", "self.context.context() == context", "true"),
    ("NATIVE_STAGE", "retains_carrier", "self.seal.completed_write_set_root.is_some()", "true"),
    ("NATIVE_STAGE", "retains_carrier", "source.decisions() == wire.decisions", "true"),
    ("ARCHIVE_CARRIER", "publish_archives", "drop(lease);", "// drop(lease);"),
    ("ARCHIVE_CARRIER", "publish_archives", "self.finality.artifact()", "other.finality.artifact()"),
    ("ARCHIVE_CARRIER", "publish_archives", "self.checkpoint.finality_receipt()", "other.checkpoint.finality_receipt()"),
    ("ARCHIVE_CARRIER", "publish_archives", ".publish(receipt)", ".publish(other_receipt)"),
    ("PHYSICAL_CARRIER", "try_prepare_physical", "original.publish_archives()", "Ok::<_, CarrierArchivePublicationError>(())"),
    ("PHYSICAL_CARRIER", "try_prepare_physical", "admit(&original, target)", "admit(&other, target)"),
    ("GEOMETRY_CARRIER", "is_identity_transition", "self._pending.is_none()", "true"),
    ("GEOMETRY_CARRIER", "is_identity_transition", "self._previous_runtime_catalog == self._accepted_runtime_catalog", "true"),
    ("TERMINAL_CARRIER", "publish", "journals.effects.replay_prevalidation", "false"),
    ("TERMINAL_CARRIER", "publish", "journals.native_amx_manifest.entries().is_empty()", "true"),
    ("TERMINAL_CARRIER", "publish", "return Err((self.abort(), error));", "return Err((other.abort(), error));"),
    ("TERMINAL_CARRIER", "publish", "runtime.publish();", "// runtime.publish();"),
    ("TERMINAL_CARRIER", "publish", "world_effects.publish(target);", "// world_effects.publish(target);"),
    ("TERMINAL_CARRIER", "publish", "source: source_prefix,", "source: other_prefix,"),
])
def test_terminal_carrier_rejects_owner_mutation(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner),
                              f"fn {symbol}", old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("old,new", [
    ("transactions.publish();", "transactions.publish(); return Err(other);"),
    ("transactions.publish();", "transactions.publish(); fallible_effect()?;"),
    ("transactions.publish();", "transactions.publish(); transactions.publish();"),
    ("drop(generation);", "drop(generation); target.begin_state_view_write();"),
])
def test_terminal_carrier_rejects_post_write_retry_and_repeated_visibility(fixture, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.TERMINAL_CARRIER,
                              "fn publish(", old, new)
    errors = validate(fixture)
    assert any("terminal lifecycle" in error or "post-write retry" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,symbol,old,new", [
    ("WITNESS_CARRIER", "publish_execution_witness", "drop(lease);", "// drop(lease);"),
    ("WITNESS_CARRIER", "publish_execution_witness", "self.journals.source_prefix.witness()", "other.source_prefix.witness()"),
    ("WITNESS_CARRIER", "publish_execution_witness", "self.journals.execution_prefix", "other.execution_prefix"),
    ("WITNESS_CARRIER", "publish_execution_witness", "self.checkpoint.finality_receipt()", "other.checkpoint.finality_receipt()"),
    ("WITNESS_LEASE", "reauthenticate_execution_witness", "Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)?;", "// Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)?;"),
    ("WITNESS_LEASE", "reauthenticate_execution_witness", "Kura::stable_sidecar_metadata_unchanged(&read.metadata, current)", "true"),
    ("PHYSICAL_CARRIER", "try_prepare_physical", "original.publish_execution_witness()", "Ok::<_, CarrierExecutionWitnessPublicationError>(())"),
    ("PHYSICAL_CARRIER", "try_prepare_physical", "kura.reauthenticate_execution_witness(original.finality.artifact())", "Ok::<(), crate::kura::Error>(())"),
])
def test_terminal_carrier_requires_its_actual_durable_execution_witness(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner),
                              f"fn {symbol}", old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,anchor,old,new", [
    ("QUEUE_OWNER", "pub struct Queue {", "lane_reservation_transition_lock: PublicationMutex,", "lane_reservation_transition_lock: parking_lot::Mutex<()>,"),
    ("QUEUE_OWNER", "fn from_config_with_router_limits_and_catalogs", "lane_reservation_transition_lock: PublicationMutex::default(),", "lane_reservation_transition_lock: other_mutex,"),
    ("QUEUE_OWNER", "struct QueueLaneRetirementObserver", "_reservation_transition_guard: PublicationGuard<'queue>", "_reservation_transition_guard: parking_lot::MutexGuard<'queue, ()>"),
    ("QUEUE_OWNER", "fn try_lock_lane_retirement_observer", "self.lane_reservation_transition_lock.try_lock_or_wait()?", "other.lane_reservation_transition_lock.try_lock_or_wait()?"),
    ("QUEUE_OWNER", "fn try_lock_lane_retirement_observer", "_reservation_transition_guard: guard,", "_reservation_transition_guard: other_guard,"),
    ("QUEUE_OWNER", "fn lane_has_pending_work_under_retirement_observer", "key.lane_incarnation == lane_incarnation", "true"),
    ("QUEUE_OWNER", "fn lane_has_pending_work_under_retirement_observer", "!reservation_owned_hashes.contains(entry.key())", "true"),
    ("PUBLICATION_MUTEX", "fn wrap", "self.released.guard(PhysicalPublicationGuard", "self.released.poisoning_guard(PhysicalPublicationGuard"),
    ("PUBLICATION_MUTEX", "fn try_lock_or_wait", "self.released.observe()", "other.released.observe()"),
    ("WITNESS_LEASE", "fn from_geometry_guards", "_canonical: canonical,", "_canonical: prune,"),
    ("GEOMETRY_OWNER", "fn apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner", "self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?", "0"),
    ("GEOMETRY_OWNER", "fn mark_lane_geometry_catalog_published_with_lineage_root", "self.finish_pending_lane_geometry_gc_locked(&mut journal)?", "()"),
    ("GEOMETRY_OWNER", "fn mark_lane_geometry_catalog_published_with_lineage_root", "self.sidecar_lock.lock()", "other.sidecar_lock.lock()"),
    ("GUARDED_GEOMETRY", "fn apply_prepared_lane_geometry", "let kura = self.kura_under_publication_guards();", "let kura = other.kura_under_publication_guards();"),
    ("GUARDED_GEOMETRY", "fn apply_prepared_lane_geometry", "kura.ensure_lane_retirement_admissible_locked(\n                pending_canonical_bytes,\n                &retiring,\n                &certified_retirements,\n            )?;", "// retained retry admission removed"),
    ("GUARDED_GEOMETRY", "fn apply_prepared_lane_geometry", "prepared.persist(kura, LaneGeometryPhase::Intent)?;", "// no durable intent"),
    ("GUARDED_GEOMETRY", "fn publish_prepared_lane_geometry_catalog", "record.updated_bindings != bindings", "false"),
    ("GUARDED_GEOMETRY", "fn publish_prepared_lane_geometry_catalog", "kura.restore_lane_geometry_journal_file(", "kura.unchecked_replace_journal("),
])
def test_queue_geometry_owner_rejects_semantic_substitution(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner),
                              anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "both retained retry" in error
               for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("symbol,operation", [
    ("apply_prepared_lane_geometry", "let _again = kura.sidecar_lock.lock();"),
    ("publish_prepared_lane_geometry_catalog", "let _again = kura.lane_geometry_lock.lock();"),
    ("apply_prepared_lane_geometry", "kura.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;"),
    ("publish_prepared_lane_geometry_catalog", "kura.finish_pending_lane_geometry_gc_locked(&mut journal)?;"),
])
def test_guarded_geometry_rejects_locking_prelude_reentry(fixture, symbol, operation):
    root, helper, checker, _ = fixture
    original = "let kura = self.kura_under_publication_guards();"
    helper.replace_once_after(root / checker.native_preparation_contract.GUARDED_GEOMETRY,
                              f"fn {symbol}", original, original + "\n" + operation)
    errors = validate(fixture)
    assert any("reenters prelude or locking owner" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


def test_queue_release_observation_must_precede_actual_probe(fixture):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.PUBLICATION_MUTEX,
                              "fn try_lock_or_wait",
                              "let wait = self.released.observe();\n        self.try_lock().ok_or(wait)",
                              "let result = self.try_lock().ok_or(wait);\n        let wait = self.released.observe();\n        result")
    errors = validate(fixture)
    assert any("reorders executable relation" in error for error in errors), errors


def test_geometry_capacity_and_gc_remain_before_sidecar_transfer(fixture):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.GEOMETRY_OWNER
    anchor = "fn apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner"
    capacity = "let pending_canonical_bytes =\n            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;"
    helper.replace_once_after(path, anchor, capacity, "")
    helper.replace_once_after(path, anchor, "lease.apply_prepared_lane_geometry(",
                              capacity + "\n        lease.apply_prepared_lane_geometry(")
    errors = validate(fixture)
    assert any("reorders executable relation" in error for error in errors), errors
