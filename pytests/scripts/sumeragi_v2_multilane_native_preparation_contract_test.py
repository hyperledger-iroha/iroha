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


@pytest.mark.parametrize("owner,anchor,old,new", [
    ("DECISION_CARRIER", "enum RetainedCarrier", "Capturing(Box<super::StagedCarrierCapture<Admission>>)", "Capturing(super::StagedCarrierCapture<Admission>)"),
    ("DECISION_CARRIER", "enum RetainedCarrier", "Validated(PreparedCarrierJournals<Admission>)", "Validated(Box<PreparedCarrierJournals<Admission>>)"),
    ("DECISION_CARRIER", "enum RetainedCarrier", "Decided(DecisionBoundCarrierJournals<Admission>)", "Decided(PreparedCarrierJournals<Admission>)"),
    ("DECISION_CARRIER", "enum RetainedCarrier", "crate::kura::KuraWsvCheckpointReceipt", "()"),
    ("VALIDATION_CUSTODY", "struct Candidate", "owner: Option<O>,", "owner: Option<O>, commitment: wire::ExecutionCommitment,"),
    ("VALIDATION_CUSTODY", "struct RetainedBodyValidationService", "limit: usize,", "limit: usize, decided: Vec<P::Owner>,"),
    ("VALIDATION_CUSTODY", "struct SelectedValidationCarrier", "owner: Option<P::Owner>,", "owner: Option<P::Owner>, saved: Option<P::Owner>,"),
    ("JOURNALS", "fn matches_validation_candidate", "if self.context.as_ref() != context", "if false"),
    ("JOURNALS", "fn matches_validation_candidate", "original == candidate", "true"),
    ("JOURNALS", "fn execution_prefix_commitment", "self.execution_prefix", "Default::default()"),
    ("VALIDATION_CUSTODY", "fn new", "candidates.try_reserve_exact(limit)?;", "// descriptor admission removed"),
    ("VALIDATION_CUSTODY", "fn new", "markers.try_reserve_exact(limit)?;", "// marker admission removed"),
    ("VALIDATION_CUSTODY", "fn prepare_marker", "if self.candidates.len() == self.limit", "if false"),
    ("VALIDATION_CUSTODY", "fn prepare_marker", "if requires_existing_owner", "if false"),
    ("VALIDATION_CUSTODY", "fn preflight_marker", "candidate.is_some_and(|row| row.owner.is_none())", "false"),
    ("VALIDATION_CUSTODY", "fn preflight_marker", "self.markers.len() == self.limit", "false"),
    ("VALIDATION_CUSTODY", "fn preflight_marker", "candidate.is_none() && self.candidates.len() == self.limit", "false"),
    ("VALIDATION_CUSTODY", "fn prepare_marker", ".pop()", ".first()"),
    ("VALIDATION_CUSTODY", "fn prepare_marker", "self.candidates[index].owner = Some(owner);", "drop(owner);"),
    ("JOURNALS", "fn matches_candidate", "self.journals\n            .matches_validation_candidate(context, proposal)", "true"),
    ("DECISION_CARRIER", "fn resume_capture", ".map(Self::Validated)", ".map(|_| unreachable!())"),
    ("DECISION_CARRIER", "fn resume_capture", "(Self::Capturing(carrier), error)", "(Self::Capturing(other), error)"),
    ("DECISION_CARRIER", "fn resume_capture", "ready => Ok(ready)", "ready => Ok(ready.clone())"),
    ("VALIDATION_CUSTODY", "fn resume(", "(Self::Owner, LocalValidationRefusal)", "(Self::Owner, Self::Error)"),
    ("VALIDATION_CUSTODY", "let refusal = match self.validator.resume(owner)", "self.candidates[index].owner = Some(owner);", "drop(owner);"),
    ("VALIDATION_CUSTODY", "Err((owner, refusal))", "self.candidates[index].owner = Some(owner);", "drop(owner);"),
    ("VALIDATION_CUSTODY", "let refusal = match self.validator.resume(owner)", "if !owner.matches_candidate(context, body)", "if false"),
    ("VALIDATION_CUSTODY", "fn prepare_marker", ".ok_or(CarrierCustodyError::IncompleteCapture)?", ".unwrap_or_default()"),
    ("VALIDATION_CUSTODY", "fn prepare_marker", "Some(commitment) => commitment", "Some(commitment) => { self.resume_candidate(index, context, body)?; commitment }"),
    ("JOURNALS", "fn try_complete", "mut self: Box<Self>", "mut self: Self"),
    ("JOURNALS", "fn try_complete", "return Err((self, error));", "return Err((Box::new(*self), error));"),
    ("JOURNALS", "fn try_complete", "self.try_prepare_archives()", "Ok::<_, CarrierArchivePreparationError>(())"),
    ("JOURNALS", "fn try_prepare_archives", "if let Some(error) = &self.capture_refusal", "if let Some(error) = &None"),
    ("JOURNALS", "fn into_journals", "self.provider.take()", "other.provider.take()"),
    ("JOURNALS", "fn into_journals", "self.reputation.take()", "other.reputation.take()"),
    ("VALIDATION_CUSTODY", "fn confirm(", "owner.ready_commitment() != Some(receipt.execution_commitment())", "false"),
    ("RETAINED_VALIDATION", "CarrierMarkerPreparation::Deferred(refusal)", "Err(V2BodyStoreError::LocalValidation(refusal))", "Ok(self.persist_rejected_outcome(&durable, 0, refusal.to_string())?)"),
    ("VALIDATION_CUSTODY", "fn try_consume", "self.owner = Some(owner);", "drop(owner);"),
    ("VALIDATION_CUSTODY", "fn try_consume", "Ok(value) => {", "Ok(value) => { self.service.candidates.remove(self.index);"),
    ("VALIDATION_CUSTODY", "fn try_consume", "FnOnce(&P, P::Owner)", "FnOnce(P::Owner)"),
    ("VALIDATION_CUSTODY", "fn try_consume", "FnOnce(&P, P::Owner)", "FnOnce(&mut P, P::Owner)"),
    ("VALIDATION_CUSTODY", "fn try_consume", "FnOnce(&P, P::Owner)", "FnOnce(&'static P, P::Owner)"),
    ("VALIDATION_CUSTODY", "fn try_consume", "match publish(&self.service.validator, owner)", "match publish(&other, owner)"),
    ("VALIDATION_CUSTODY", "fn try_consume", "match publish(&self.service.validator, owner)", "match publish(&self.service.validator, other)"),
    ("VALIDATION_CUSTODY", "fn try_consume", "match publish(&self.service.validator, owner)", "let _old = std::mem::replace(&mut self.service.validator, other); match publish(&self.service.validator, owner)"),
    ("VALIDATION_CUSTODY", "fn drop(&mut self)", "self.service.candidates[self.index].owner = Some(owner);", "self.service.candidates[0].owner = Some(owner);"),
    ("RETAINED_VALIDATION", "fn execute_retained_durable_validation", "if !service.matches_store(&self.instance_identity())", "if false"),
    ("RETAINED_VALIDATION", "fn execute_retained_durable_validation", "already_validated.is_some() || reused.is_some()", "false"),
    ("RETAINED_VALIDATION", "fn execute_retained_durable_validation", "service.preflight_marker(&durable)?;", "// predecode admission removed"),
    ("RETAINED_VALIDATION", "fn execute_retained_durable_validation", "if !self.rejected.contains_key(&key)", "if true"),
])
def test_retained_carrier_rejects_owner_or_refusal_substitution(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in e or "retained carrier" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("owner,anchor,old,new", [
    ("JOURNALS", "struct CarrierJournalInputs", "&'owner crate::block::ValidBlock", "&'owner SignedBlock"),
    ("JOURNALS", "struct CarrierJournalInputs", "&'owner Arc<iroha_data_model::block::consensus_v2::HeightContext>", "&'owner iroha_data_model::block::consensus_v2::HeightContext"),
    ("JOURNALS", "struct CarrierJournalInputs", "&'owner Vec<DaPinIntentWithLocation>", "&'owner [DaPinIntentWithLocation]"),
    ("JOURNALS", "struct CarrierJournalInputs", "&'owner Vec<EventBox>", "&'owner [EventBox]"),
    ("JOURNALS", "struct CarrierJournalInputs", "retained_effects_layout: std::alloc::Layout", "retained_effects_layout: usize"),
    ("JOURNALS", "let admission = match admit_journals", "std::alloc::Layout::new::<RetainedCarrierEffects>()", "std::alloc::Layout::new::<Box<RetainedCarrierEffects>>()"),
    ("JOURNALS", "let admission = match admit_journals", "std::alloc::Layout::new::<RetainedCarrierEffects>()", "std::alloc::Layout::new::<()>()"),
    pytest.param('JOURNALS', 'fn prepare_journals', '        } = &*self;', '            ..\n        } = &*self;', id='JOURNALS-fn prepare_journals-        } = &self;-            ..\n        } = &self;'),
    pytest.param('JOURNALS', 'fn prepare_journals', '} = &*self;', '} = &*other;', id='JOURNALS-fn prepare_journals-} = &self;-} = &other;'),
    ("JOURNALS", "let admission = match admit_journals", "            valid,", "            valid: other_valid,"),
    ("JOURNALS", "let admission = match admit_journals", "            state,", "            state: other_state,"),
    ("JOURNALS", "let admission = match admit_journals", "            context,", "            context: other_context,"),
    ("JOURNALS", "let admission = match admit_journals", "            execution_prefix,", "            execution_prefix: other_commitment,"),
    ("JOURNALS", "let admission = match admit_journals", "            native_amx_manifest,", "            native_amx_manifest: other_manifest,"),
    ("JOURNALS", "let admission = match admit_journals", "da_pins: _world_effects.admission_pins()", "da_pins: &Vec::new()"),
    ("JOURNALS", "let admission = match admit_journals", "publication_events: _publication_events", "publication_events: &Vec::new()"),
    ("JOURNALS", "let admission = match admit_journals", "provider: provider_capture.as_ref()", "provider: None"),
    ("JOURNALS", "let admission = match admit_journals", "reputation: reputation_capture.as_ref()", "reputation: None"),
    ("WORLD_COMMIT", "fn admission_pins", "&Vec<DaPinIntentWithLocation>", "&[DaPinIntentWithLocation]"),
    ("WORLD_COMMIT", "fn admission_pins", "let Self { da_pins } = self;", "let Self { da_pins, .. } = self;"),
    ("WORLD_COMMIT", "fn admission_pins", "let Self { da_pins } = self;", "let Self { da_pins } = other;"),
])
def test_retained_carrier_admission_requires_complete_original_inputs(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in e or "retained carrier" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("projection", [
    "PreparedTieredSnapshot::prepare(&self.state.world, &self.state.state_ref.tiered_snapshot_worker);",
    "state.prepare_carrier_geometry();",
    "owner.capture_original(state.as_ref());",
    pytest.param("world.capture_slot();", id="world.try_detach_journals(|_| Ok(()));"),
    "Box::new(RetainedCarrierEffects {});",
])
def test_retained_carrier_admission_precedes_original_projection(fixture, projection):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.JOURNALS,
                              "fn prepare_journals", "let admission = match admit_journals",
                              projection + "\n        let admission = match admit_journals")
    errors = validate(fixture)
    assert any("journal admission follows projection" in e for e in errors), errors


@pytest.mark.parametrize("phase", ["Capturing", "Validated", "Decided", "Checkpointed"])
@pytest.mark.parametrize("method", ["matches_validation_candidate", "ready_commitment"])
def test_retained_carrier_requires_original_delegation_in_every_phase(fixture, phase, method):
    root, helper, checker, _ = fixture
    argument = "journals" if phase == "Validated" else "carrier"
    if phase == "Capturing" and method == "ready_commitment":
        argument = "_"
    helper.replace_once_after(root / checker.native_preparation_contract.DECISION_CARRIER,
                              f"fn {method}", f"Self::{phase}({argument}) =>",
                              f"Self::{phase}({argument}) => return Default::default(), _ =>")
    errors = validate(fixture)
    assert any("retained carrier" in e or "executable relation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("mutation", ["unsealed", "old-owner", "fixture-production", "fixture-escape", "cached-commitment"])
def test_retained_carrier_requires_sealed_phase_owner(fixture, mutation):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.VALIDATION_CUSTODY
    if mutation == "unsealed":
        helper.replace_once_after(path, "trait RetainedValidationOwner", "sealed::Owner + Send", "Send")
    elif mutation == "old-owner":
        path.write_text(path.read_text() + "\nimpl<A> sealed::Owner for crate::state::PreparedCarrierJournals<A> {}\n")
    elif mutation == "fixture-production":
        helper.replace_once_after(path, "#[cfg(test)]", "pub(in crate::sumeragi) mod test_support", "pub(crate) mod test_support")
        path.write_text(path.read_text().replace("#[cfg(test)]", "", 1))
    elif mutation == "fixture-escape":
        declaration = "impl sealed::Owner for TrackedOwner {}"
        source = path.read_text()
        assert source.count(declaration) == 1
        path.write_text(source.replace(declaration, "", 1) + "\n" + declaration + "\n")
    else:
        helper.replace_once_after(path, "impl<A: Send", "self.ready_commitment()", "Default::default()")
    assert any("retained carrier" in e for e in validate(fixture))


@pytest.mark.parametrize("mutation", ["execute-before-capacity", "execute-before-slot", "resume-before-install", "marker-before-resume", "decode-before-capacity", "persist-before-install", "confirm-before-persist"])
def test_retained_carrier_requires_admission_and_marker_order(fixture, mutation):
    root, helper, checker, _ = fixture
    c = checker.native_preparation_contract
    if mutation == "execute-before-capacity":
        helper.replace_once_after(root / c.VALIDATION_CUSTODY, "fn prepare_marker",
                                  "let existing = self", "self.validator.prepare(context, body); let existing = self")
    elif mutation == "execute-before-slot":
        path = root / c.VALIDATION_CUSTODY
        source = path.read_text()
        start = source.index("                self.candidates.push(Candidate {", source.index("fn prepare_marker"))
        end = source.index("                let owner = match self.validator.prepare", start)
        reservation = source[start:end]
        source = source[:start] + source[end:]
        at = source.index("                self.candidates[index].owner = Some(owner);", start)
        path.write_text(source[:at] + reservation + source[at:])
    elif mutation == "resume-before-install":
        helper.replace_once_after(root / c.VALIDATION_CUSTODY, "fn prepare_marker",
                                  "self.candidates.push(Candidate {", "self.resume_candidate(0, context, body); self.candidates.push(Candidate {")
    elif mutation == "marker-before-resume":
        path = root / c.VALIDATION_CUSTODY
        source = path.read_text()
        start = source.index("        if marker.is_none() {", source.index("fn prepare_marker"))
        end = source.index("        Ok(CarrierMarkerPreparation::Ready(commitment))", start)
        marker = source[start:end]
        source = source[:start] + source[end:]
        at = source.index("        let commitment = match owner.ready_commitment()", source.index("fn prepare_marker"))
        path.write_text(source[:at] + marker + source[at:])
    elif mutation == "decode-before-capacity":
        path = root / c.RETAINED_VALIDATION
        source = path.read_text()
        load = "        let envelope = self.load_validation_envelope(&durable, expected_manifest_hash)?;\n"
        assert source.count(load) == 1
        source = source.replace(load, "", 1)
        at = source.index("        if !self.rejected.contains_key(&key)")
        path.write_text(source[:at] + load + source[at:])
    else:
        path = root / c.RETAINED_VALIDATION
        anchor = "fn execute_retained_durable_validation"
        persistence = "let validated = self.persist_validated_receipt(&durable, commitment)?;"
        if mutation == "persist-before-install":
            helper.replace_once_after(path, anchor, persistence, "")
            helper.replace_once_after(path, anchor, "match service.prepare_marker(", persistence + "\n        match service.prepare_marker(")
        else:
            helper.replace_once_after(path, anchor, "service.confirm(&validated)?;", "")
            helper.replace_once_after(path, anchor, persistence, "service.confirm(&validated)?;\n" + persistence)
    errors = validate(fixture)
    assert any("reorders executable relation" in e or "repeats execution" in e or "installed owner" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("before", ["candidates", "_descriptor_admission"])
def test_retained_service_keeps_original_producer_until_payload_and_descriptor_release(fixture, before):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.VALIDATION_CUSTODY
    helper.replace_once_after(path, "struct RetainedBodyValidationService", "    validator: P,", "")
    helper.replace_once_after(path, "struct RetainedBodyValidationService", f"    {before}:", f"    validator: P,\n    {before}:")
    errors = validate(fixture)
    assert any("retained carrier" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("mutation", ["missing", "wrong-state", "wrong-header", "lost-original", "after-witness"])
def test_retained_carrier_rejects_late_or_substituted_physical_target(fixture, mutation):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.PHYSICAL_CARRIER
    anchor = "fn try_prepare_physical"
    guard = """        if !original
            .journals
            .geometry
            .matches_publication_target(target, original.block().header())
        {
            return Err((original, CarrierPhysicalPreparationError::ForeignTarget));
        }
"""
    source = path.read_text()
    assert source.count(guard) == 1
    if mutation == "missing":
        path.write_text(source.replace(guard, "", 1))
    elif mutation == "wrong-state":
        helper.replace_once_after(path, anchor, ".matches_publication_target(target, original.block().header())",
                                  ".matches_publication_target(other, original.block().header())")
    elif mutation == "wrong-header":
        helper.replace_once_after(path, anchor, ".matches_publication_target(target, original.block().header())",
                                  ".matches_publication_target(target, other.block().header())")
    elif mutation == "lost-original":
        path.write_text(source.replace(guard, guard.replace("return Err((original,", "return Err((other,"), 1))
    else:
        source = source.replace(guard, "", 1)
        start = source.index("        if let Err(error) = self.decision.publish_archives(&self.kura)", source.index(anchor))
        path.write_text(source[:start] + guard + source[start:])
    errors = validate(fixture)
    assert any("executable relation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


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
    ("BLOCK", "validate_and_record_native_candidate", "verify_origin_block_signature(", "unchecked_origin_signature("),
    ("BLOCK", "validate_and_record_native_candidate", "length > frozen.da_layout.max_payload_size_bytes", "false"),
    ("BLOCK", "validate_and_record_native_candidate", "Self::validate_static_state_dependent(", "unchecked_static_state("),
    ("BLOCK", "validate_and_record_native_candidate", "Self::validate_static_with_snapshot(", "unchecked_snapshot("),
    ("BLOCK", "validate_and_record_native_candidate", "generation != state.state_view_generation()", "false"),
    ("NATIVE_SOURCE", "preparation_input", "self.is_current()", "true"),
    ("NATIVE_SOURCE", "preparation_input", "self.generation", "self.state.state_view_generation()"),
    ("BLOCK", "validate_and_record_native_candidate", "native: Some(native)", "native: None"),
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
    ("NATIVE_METADATA", "seal_native_execution_outputs", "Vec::new()", "ordinary_membership"),
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
    ("PREFIX", "capture", "state\n            .verify_execution_output_seal(block)\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;", "/* state\n            .verify_execution_output_seal(block)\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?; */"),
    ("PREFIX", "capture", "state.staged_merge_entry.is_some()", "state.staged_merge_entry.is_none()"),
    ("PREFIX", "capture", "native.retains_state(&state)", "true"),
    ("PREFIX", "capture", "context.native_lane_decisions.is_some()", "context.native_lane_decisions.is_none()"),
    ("PREFIX", "capture", "verify_cached_ordinary_witness_content(&verified_inventory)", "verify_cached_ordinary_witness_content(&other_inventory)"),
    ("PREFIX", "capture", "execution_commitment_from_validated_block(witness,", "execution_commitment_from_validated_block(other_witness,"),
    ("PREFIX", "capture", "witness, &manifest, &lanes, block", "witness, &other_manifest, &lanes, block"),
    ("PREFIX", "capture", "witness, &manifest, &lanes, block", "witness, &manifest, &other_lanes, block"),
    ("PREFIX", "capture", ".replace(output_capacity::ExecutionOutputPlanState::Captured)", ".take()"),
    ("PREFIX", "capture", "Arc::ptr_eq(&inventory, &verified_inventory)", "Arc::ptr_eq(&inventory, &inventory)"),
    ("PREFIX", "capture", "_fastpq_witness_context: state.fastpq_witness_context.take()", "_fastpq_witness_context: None"),
    ("PREFIX", "prepare", "PrefixPreparation::capture(state, &valid, native)?", "PrefixPreparation::capture(other_state, &valid, native)?"),
    ("PREFIX", "prepare", "Err(error) => Err((Box::new(valid.into()), error))", "Err(error) => retain_partial(error)"),
    ("PREFIX", "prepare_world_effects", "!self.prefix.retains_closed_state(state)", "false"),
    ("PREFIX", "prepare_world_effects", "state\n            .verify_lane_consensus_contexts_publication()\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;", "/* state\n            .verify_lane_consensus_contexts_publication()\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?; */"),
    ("JOURNALS", "prepare_journals", "prefix: source_prefix,", "prefix: &other_prefix,"),
    ("JOURNALS", "prepare_journals", "carrier: self,", "carrier: other_carrier,"),
    ("JOURNALS", "prepare_journals", "provider: provider_capture,", "provider: None,"),
    ("JOURNALS", "prepare_journals", "reputation: reputation_capture,", "reputation: None,"),
    ("JOURNALS", "prepare_journals", "&state.state_ref.tiered_snapshot_worker,", "&other_state.tiered_snapshot_worker,"),
    ("JOURNALS", "try_prepare_archives", "provider\n                .try_prepare()", "other_provider\n                .try_prepare()"),
    ("JOURNALS", "try_prepare_archives", "reputation\n                .try_prepare()", "other_reputation\n                .try_prepare()"),
    ("PHYSICAL_CARRIER", "try_new", "&original.checkpoint,", "&other_checkpoint,"),
    ("PHYSICAL_CARRIER", "try_new", "&original.journals.execution_prefix,", "&other_prefix,"),
    ("PHYSICAL_CARRIER", "try_new", "original.checkpoint.finality_receipt(),", "other_checkpoint.finality_receipt(),"),
    ("PHYSICAL_CARRIER", "try_new", 'if let Some(capture) = original.journals.reputation_capture.as_ref() {\n                capture\n                    .reauthenticate_under_publication_lease(\n                        &owner.kura,\n                        original.checkpoint.finality_receipt(),\n                    )\n                    .map_err(CarrierPhysicalPreparationError::Reputation)?;\n            }', 'if let Some(capture) = original.journals.reputation_capture.as_ref() {\n                capture\n                    .reauthenticate_under_publication_lease(\n                        &owner.kura,\n                        other_checkpoint.finality_receipt(),\n                    )\n                    .map_err(CarrierPhysicalPreparationError::Reputation)?;\n            }'),
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
    ("MEMBERSHIP_STORAGE", "all_publication_budget_reserved_bytes", ".checked_add(self.membership_storage.pending_bytes())", ".checked_add(0)"),
    ("MEMBERSHIP_STORAGE", "all_publication_budget_reserved_bytes", "self.lane_publication_budget_reserved_bytes()?", "0_u64"),
    ("ORDINARY", "lane_artifact_required_bytes_for_block", "Ok(total)", "Ok(total.saturating_add(self.native_amx_publication_capacity_reserved_bytes()?))"),
    ("KURA", "check_storage_budget", ".saturating_add(lane_publication_reservations)", ".saturating_add(0)"),
])
def test_native_preparation_rejects_semantic_mutation(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    # Preserve the original selectors while mutating the same defining capacity engine.
    if owner == "CAPACITY" and symbol == "native_amx_route_publication_capacity_at_target_locked":
        symbol = "native_amx_route_publication_capacity_with_inventory_locked"
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
    ("self.all_publication_budget_reserved_bytes()?", "self.post_wsv_lane_artifact_budget_reserved_bytes()?"),
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
    ("ARCHIVE_CARRIER", "publish_archives", "lease: &KuraPublicationLease<'_>", "lease: &ForeignPublicationLease<'_>"),
    ("ARCHIVE_CARRIER", "publish_archives", "self.finality.artifact()", "other.finality.artifact()"),
    ("ARCHIVE_CARRIER", "publish_archives", "self.checkpoint.finality_receipt()", "other.checkpoint.finality_receipt()"),
    ("ARCHIVE_CARRIER", "publish_archives", ".publish_under_publication_lease(lease, receipt)", ".publish_under_publication_lease(lease, other_receipt)"),
    ("ARCHIVE_CARRIER", "publish_archives", ".publish_under_publication_lease(lease, receipt)", ".publish(receipt)"),
    ("ARCHIVE_CARRIER", "publish_archives", "let receipt = self.checkpoint.finality_receipt();", "drop(lease); let receipt = self.checkpoint.finality_receipt();"),
    ("PHYSICAL_CARRIER", "try_prepare_physical", "self.decision.publish_archives(&self.kura)", "Ok::<_, CarrierArchivePublicationError>(())"),
    ("PHYSICAL_CARRIER", "try_prepare_physical", "SourceAuthenticatedCarrier::try_new(original, kura)", "SourceAuthenticatedCarrier::try_new(other, kura)"),
    ("GEOMETRY_CARRIER", "is_identity_transition", "self._pending.is_none()", "true"),
    ("GEOMETRY_CARRIER", "is_identity_transition", "self._previous_runtime_catalog == self._accepted_runtime_catalog", "true"),
    ("TERMINAL_CARRIER", "publish", "journals.effects.replay_prevalidation", "false"),
    ("TERMINAL_CARRIER", "publish", "journals.native_amx_manifest.entries().is_empty()", "true"),
    ("TERMINAL_CARRIER", "publish", "return Err((this.abort(), error));", "return Err((other.abort(), error));"),
    ("TERMINAL_CARRIER", "publish", "runtime.publish();", "// runtime.publish();"),
    pytest.param("TERMINAL_CARRIER", "publish", 'world_effects.publish(\n            target,\n            effect_locks\n                .da_pin_intents\n                .as_mut()\n                .expect("prepared pin cache"),\n        );', "// original World effects omitted", id="TERMINAL_CARRIER-publish-world_effects.publish(target);-// world_effects.publish(target);"),
    ("TERMINAL_CARRIER", "publish", "source: source_prefix,", "source: other_prefix,"),
])
def test_terminal_carrier_rejects_owner_mutation(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner),
                              f"fn {symbol}", old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,symbol,old,new", [
    pytest.param("GEOMETRY_CARRIER", "prepare_carrier_geometry", "self.state_ref.native_lane_state_owner()", "other.native_lane_state_owner()", id="original-state-capture"),
    pytest.param("GEOMETRY_CARRIER", "matches_publication_target", "self._header == header", "true", id="exact-header"),
    pytest.param("GEOMETRY_CARRIER", "matches_publication_target", "self.state_owner.matches_state(target)", "true", id="exact-state"),
    pytest.param("GEOMETRY_CARRIER", "requires_queue_custody", "!pending.plan.retire.is_empty()", "false", id="retired-route"),
    pytest.param("GEOMETRY_CARRIER", "requires_queue_custody", "!pending.catalog_update.replaced_lane_ids.is_empty()", "false", id="replaced-route"),
    pytest.param("GEOMETRY_CARRIER", "complete_under", "!self.matches_publication_target(target, header)", "false", id="completion-binding"),
    pytest.param("GEOMETRY_CARRIER", "complete_under", "!self.has_queue_custody(target, header, queue)", "false", id="completion-queue-refusal"),
    pytest.param("GEOMETRY_CARRIER", "complete_under", "raw.publish_catalog_under(lease, None)", "raw.publish_catalog_under(other_lease, None)", id="catalog-lease"),
    pytest.param("GEOMETRY_CARRIER", "complete_under", "raw.reauthenticate_catalog_under(lease)", "raw.reauthenticate_catalog_under(other_lease)", id="completed-catalog-lease"),
    pytest.param("GEOMETRY_CARRIER", "complete_under", "geometry: self,", "geometry: other,", id="completed-original-owner"),
    pytest.param("PHYSICAL_CARRIER", "try_complete_geometry", "!journals.geometry.requires_storage_transition()", "false", id="identity-needs-no-backend"),
    pytest.param("PHYSICAL_CARRIER", "try_complete_geometry", "&journals.components._fences._kura", "other_lease", id="retained-preparation-lease"),
    pytest.param("PHYSICAL_CARRIER", "try_complete_geometry", "            self.target,", "            other_target,", id="completion-original-target"),
    pytest.param("PHYSICAL_CARRIER", "try_complete_geometry", "let journals = &mut self.decision.journals;", "let journals = &mut self.decision.journals; let _fresh = self.target.kura.try_publication_lease()?;", id="no-fresh-kura-lease"),
    pytest.param("PHYSICAL_CARRIER", "try_complete_geometry", ".try_lock_or_wait()", ".lock()", id="no-blocking-backend-lock"),
    pytest.param("TERMINAL_CARRIER", "publish", "journals.geometry.has_pending_lifecycle() != journals.effects.lifecycle.is_some()", "false", id="exact-lifecycle-effects"),
    pytest.param("TERMINAL_CARRIER", "publish", "!journals.geometry.has_queue_custody(\n            this.target,\n            journals.effects.header,\n            journals.components._fences._queue.as_ref(),\n        )", "false", id="publisher-queue-refusal"),
    pytest.param("TERMINAL_CARRIER", "publish", "journals.components._fences._queue.as_ref(),", "None,", id="publisher-original-queue-proof"),
    pytest.param("TERMINAL_CARRIER", "publish", ".sync_mapping(&effects.nexus.lane_config)", ".sync_mapping(&other_mapping)", id="accepted-mapping"),
    pytest.param("TERMINAL_CARRIER", "publish", ".lifecycle\n            .take()", ".lifecycle\n            .clone()", id="consume-lifecycle"),
    pytest.param("TERMINAL_CARRIER", "publish", "if let Some(post) = lifecycle_post_publication {\n            post.publish(target);\n        }", "drop(lifecycle_post_publication);", id="consume-lifecycle-post-work"),
])
def test_terminal_geometry_rejects_owner_mutation(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner),
                              f"fn {symbol}", old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "geometry completion" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("mutation", ["resume-before-guards", "completion-before-source", "release-before-lifecycle-post"])
def test_terminal_geometry_rejects_effects_before_their_guards(fixture, mutation):
    root, helper, checker, _ = fixture
    contract = checker.native_preparation_contract
    if mutation == "resume-before-guards":
        path = root / contract.GEOMETRY_CARRIER
        statement = "self.resume_under(backend, lease)?;"
        helper.replace_once_after(path, "fn complete_under", statement, "")
        helper.replace_once_after(path, "fn complete_under",
                                  "if !self.matches_publication_target(target, header)",
                                  statement + "\n        if !self.matches_publication_target(target, header)")
    elif mutation == "completion-before-source":
        # Retaining the correct later completion must not hide an earlier effect.
        helper.replace_once_after(root / contract.TERMINAL_CARRIER, "fn publish(",
                                  "let journals = &this.decision.journals;",
                                  "this.try_complete_geometry()?; let journals = &this.decision.journals;")
    else:
        path = root / contract.TERMINAL_CARRIER
        statement = "let commit = fences.release_for_completion();"
        helper.replace_once_after(path, "fn publish(", statement, "")
        helper.replace_once_after(path, "fn publish(",
                                  "if let Some(post) = lifecycle_post_publication",
                                  statement + "\n        if let Some(post) = lifecycle_post_publication")
    errors = validate(fixture)
    assert any("reorders executable relation" in error or "terminal lifecycle" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("old,new", [
    ("transactions.publish();", "transactions.publish(); return Err(other);"),
    ("transactions.publish();", "transactions.publish(); fallible_effect()?;"),
    ("transactions.publish();", "transactions.publish(); transactions.publish();"),
    ("drop(generation);", "drop(generation); publication_notice.begin();"),
])
def test_terminal_carrier_rejects_post_write_retry_and_repeated_visibility(fixture, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.TERMINAL_CARRIER,
                              "fn publish(", old, new)
    errors = validate(fixture)
    assert any("terminal lifecycle" in error or "post-write retry" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,symbol,old,new", [
    ("WITNESS_CARRIER", "publish_execution_witness", "lease: &KuraPublicationLease<'_>", "lease: &ForeignPublicationLease<'_>"),
    ("WITNESS_CARRIER", "publish_execution_witness", "self.journals.source_prefix.witness()", "other.source_prefix.witness()"),
    ("WITNESS_LEASE", "publish_execution_witness", "finality.commit_qc.execution_commitment", "other.execution_prefix"),
    ("WITNESS_CARRIER", "publish_execution_witness", "self.checkpoint.finality_receipt()", "other.checkpoint.finality_receipt()"),
    ("WITNESS_LEASE", "reauthenticate_execution_witness", "Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)?;", "// Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)?;"),
    ("WITNESS_LEASE", "reauthenticate_execution_witness", "Kura::stable_sidecar_metadata_unchanged(&read.metadata, current)", "true"),
    ("PHYSICAL_CARRIER", "try_prepare_physical", "self.decision.publish_execution_witness(&self.kura)", "Ok::<_, CarrierExecutionWitnessPublicationError>(())"),
    ("PHYSICAL_CARRIER", "try_prepare_physical", ".reauthenticate_execution_witness(authenticated.decision.finality.artifact())", ".unchecked_execution_witness(authenticated.decision.finality.artifact())"),
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
    ("QUEUE_OWNER", "fn lane_retirement_reservation_snapshot", "key.lane_incarnation == lane_incarnation", "true"),
    ("QUEUE_OWNER", "fn lane_has_pending_route_work", "!reservation_owned_hashes.contains(entry.key())", "true"),
    ("PUBLICATION_MUTEX", "fn wrap", "self.released.guard(PhysicalPublicationGuard", "self.released.poisoning_guard(PhysicalPublicationGuard"),
    ("PUBLICATION_MUTEX", "fn try_lock_or_wait", "self.released.observe()", "other.released.observe()"),
    ("WITNESS_LEASE", "fn try_publication_lease", 'fences.canonical = Some(acquire("canonical_chain_lock", &self.canonical_chain_lock)?);', 'fences.canonical = Some(acquire("canonical_chain_lock", &self.prune_lock)?);'),
    ("WITNESS_LEASE", "fn pending_canonical_bytes", "self.pending_canonical_bytes", "0"),
    ("WITNESS_LEASE", "fn try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards", "self.sidecar_lock.try_lock_or_wait()", "other.sidecar_lock.try_lock_or_wait()"),
    ("WITNESS_LEASE", "fn try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards", "self.merge_entry_by_hash_after_sidecar(hash, pending)", "self.merge_entry_by_hash(hash)"),
    ("RAW_GEOMETRY", "fn begin_raw_geometry_attempt", "kura.durable_mutation_authorized()?;", "// no durable authority"),
    ("RAW_GEOMETRY", "fn begin_raw_geometry_attempt", "kura.validate_certified_lane_drain_frontier_under_publication_lease(", "kura.unchecked_frontier("),
    ("RAW_GEOMETRY", "fn begin_raw_geometry_attempt", "if observed != journal", "if false"),
    ("RAW_GEOMETRY", "fn authenticate<'lease>", "!self.kura.matches(kura)", "false"),
    ("RAW_GEOMETRY", "fn authenticate<'lease>", "!self.claim.authorizes(&kura.raw_geometry_claim)", "false"),
    ("RAW_GEOMETRY", "fn select_plan", "record.updated_bindings != self.updated_bindings", "false"),
    ("RAW_GEOMETRY", "fn persist_target", "self.pending_phase = Some(phase);", "self.pending_phase = None;"),
    ("RAW_GEOMETRY", "fn prepare_target", "&mut self.maintenance.writer,", "&mut other.maintenance.writer,"),
    ("RAW_GEOMETRY", "fn resume_under", "lease.pending_canonical_bytes()", "0"),
    ("RAW_GEOMETRY", "fn resume_under", "kura.ensure_lane_retirement_admissible_locked(pending, &retiring, &certified)?;", "// retirement admission removed"),
    ("RAW_GEOMETRY", "fn resume_under", "self.persist_target(kura, LaneGeometryPhase::Intent)?;", "// no durable intent"),
    ("RAW_GEOMETRY", "fn resume_under", "self.phase = RawGeometryPhase::RecoveryRequired;", "self.phase = RawGeometryPhase::Applying;"),
    ("RAW_GEOMETRY", "fn resume_under", "self.operation_cursor += 1;", "self.operation_cursor = 0;"),
    ("RAW_GEOMETRY", "fn resume_under", "if !matches!(kind, TargetKind::Published) {\n                    let retiring", "if matches!(kind, TargetKind::Fresh) {\n                    let retiring"),
    ("RAW_GEOMETRY", "fn publish_catalog_under", "original != configured_baseline", "false"),
    ("RAW_GEOMETRY", "fn publish_catalog_under", "self.persist_target(kura, LaneGeometryPhase::CatalogPublished)?;", "// no durable publication"),
    ("RAW_GEOMETRY", "fn reauthenticate_catalog_under", "!self.claim.complete", "false"),
    ("RAW_GEOMETRY", "fn reauthenticate_catalog_under", "!= Some(binding.identity())", "== Some(binding.identity())"),
    ("RAW_GEOMETRY", "fn rollback_under", "self.maintenance.pending.is_some()", "false"),
    ("RAW_GEOMETRY", "fn rollback_under", "target.operations().len() - self.operation_cursor - 1", "self.operation_cursor"),
    ("RAW_GEOMETRY", "impl Drop for RawGeometryClaim", "state.abandoned |= self.effects_started && !self.complete;", "state.abandoned = false;"),
])
def test_queue_geometry_owner_rejects_semantic_substitution(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner),
                              anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "shared new/retained retirement" in error
               for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("symbol,operation", [
    ("resume_under", "let _again = kura.sidecar_lock.lock();"),
    ("publish_catalog_under", "let _again = kura.lane_geometry_lock.lock();"),
    ("rollback_under", "let _again = kura.prune_lock.lock();"),
    ("resume_under", "kura.finish_pending_lane_geometry_gc_locked(&mut self.journal)?;"),
    ("resume_under", "kura.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;"),
    ("resume_under", "kura.try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;"),
])
def test_retained_geometry_rejects_locking_prelude_reentry(fixture, symbol, operation):
    root, helper, checker, _ = fixture
    original = "let kura = self.authenticate(lease)?;"
    helper.replace_once_after(root / checker.native_preparation_contract.RAW_GEOMETRY,
                              f"fn {symbol}", original, original + "\n" + operation)
    errors = validate(fixture)
    assert any("reenters prelude or locking owner" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("symbol", ["resume_under", "publish_catalog_under", "rollback_under"])
def test_retained_geometry_rejects_reconstructed_custody(fixture, symbol):
    root, helper, checker, _ = fixture
    original = "let kura = self.authenticate(lease)?;"
    helper.replace_once_after(root / checker.native_preparation_contract.RAW_GEOMETRY,
                              f"fn {symbol}", original,
                              original + "\n let replacement = kura.read_lane_geometry_journal_structure()?;")
    assert any("reconstructs original custody" in error for error in validate(fixture))


def test_queue_release_observation_must_precede_actual_probe(fixture):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.PUBLICATION_MUTEX,
                              "fn try_lock_or_wait",
                              "let wait = self.released.observe();\n        self.try_lock().ok_or(wait)",
                              "let result = self.try_lock().ok_or(wait);\n        let wait = self.released.observe();\n        result")
    errors = validate(fixture)
    assert any("reorders executable relation" in error for error in errors), errors


def test_geometry_retirement_admission_precedes_retained_writer_transfer(fixture):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.RAW_GEOMETRY
    anchor = "fn resume_under"
    admission = "kura.ensure_lane_retirement_admissible_locked(pending, &retiring, &certified)?;"
    transfer = "self.target = Some(self.prepare_target(kura, index)?);"
    helper.replace_once_after(path, anchor, admission, "")
    helper.replace_once_after(path, anchor, transfer, transfer + "\n" + admission)
    errors = validate(fixture)
    assert any("reorders executable relation" in error for error in errors), errors


@pytest.mark.parametrize("anchor,old,new", [
    ("pub struct Queue {", "push_remove_lock: PublicationMutex,", "push_remove_lock: parking_lot::Mutex<()>,"),
    ("pub struct Queue {", "lane_reservations: PublicationMutex<LaneQueueReservationStore>,", "lane_reservations: parking_lot::Mutex<LaneQueueReservationStore>,"),
    ("fn from_config_with_router_limits_and_catalogs", "lane_reservations: PublicationMutex::new(LaneQueueReservationStore::default()),", "lane_reservations: other_store,"),
    ("fn try_into_cut", "self,", "&self,"),
    ("fn try_into_cut", ".push_remove_lock", ".lane_reservation_transition_lock"),
    ("fn try_into_cut", ".lane_reservations", ".other_lane_reservations"),
    ("fn try_into_cut", 'field: "push_remove_lock",', 'field: "lane_reservations",'),
    ("fn try_into_cut", 'field: "lane_reservations",', 'field: "push_remove_lock",'),
    ("fn try_into_cut", "wait,", "wait: other_wait,"),
    ("fn try_into_cut", "released: [None, None, Some(self.release_deferred())]", "released: [None, None, None]"),
    ("fn try_into_cut", "_mutation: mutation,", "_mutation: other_mutation,"),
    ("fn try_into_cut", "observer: self,", "observer: other_observer,"),
    ("struct QueueRetirementBusy", "pub(crate) wait: concread::release::ReleaseWait,", "pub(crate) wait: concread::release::ReleaseWait, retained: PublicationGuard<'static>,"),
    ("impl QueueLaneRetirementCut<'_>", "queue.transaction_selection_durability_faulted()", "false"),
    ("impl QueueLaneRetirementCut<'_>", "Queue::lane_retirement_reservation_snapshot(\n            &self.reservations,", "Queue::lane_retirement_reservation_snapshot(\n            &other_reservations,"),
    ("impl QueueLaneRetirementCut<'_>", "return true;", "return false;"),
    ("fn lane_retirement_reservation_snapshot", "return None;", "return Some(HashSet::new());"),
    ("fn lane_retirement_reservation_snapshot", "completion.barrier.lane_incarnation == lane_incarnation", "true"),
    ("fn lane_retirement_reservation_snapshot", ".map(|record| record.key.entrypoint_hash)", ".map(|record| other_hash)"),
    ("fn lane_has_pending_route_work", "self.txs.contains_key(entry.key())", "true"),
    ("fn lane_has_pending_route_work", "entry.value().legs().into_iter().any(|leg|", "entry.value().coordinator_only().into_iter().any(|leg|"),
])
def test_retained_queue_cut_rejects_owner_predicate_and_wait_substitution(fixture, anchor, old, new):
    """A real cut keeps exact guards, conservative predicates and the failed owner's event."""
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.QUEUE_OWNER, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "refusal/release relation" in error
               for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("mutation", ["acquisition_order", "field_release_order", "blocking_predicate", "early_guard_drop"])
def test_retained_queue_cut_requires_nonblocking_acquisition_and_reverse_release(fixture, mutation):
    """Token presence cannot replace the actual try-only probe and held-guard ordering."""
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.QUEUE_OWNER
    source = path.read_text()
    if mutation == "acquisition_order":
        start = source.index("        let mutation =", source.index("fn try_into_cut"))
        middle = source.index("        let reservations =", start)
        end = source.index("        Ok(QueueLaneRetirementCut", middle)
        source = source[:start] + source[middle:end] + source[start:middle] + source[end:]
    elif mutation == "field_release_order":
        first = "    reservations: PublicationGuard<'queue, LaneQueueReservationStore>,\n"
        second = "    _mutation: PublicationGuard<'queue>,\n"
        assert source.count(first + second) == 1
        source = source.replace(first + second, second + first, 1)
    elif mutation == "blocking_predicate":
        anchor = "impl QueueLaneRetirementCut<'_>"
        prefix, suffix = source.split(anchor, 1)
        original = "        let queue = self.observer.queue;"
        assert original in suffix
        source = prefix + anchor + suffix.replace(original, original + "\n        let _again = queue.push_remove_lock.lock();", 1)
    else:
        original = "        Ok(QueueLaneRetirementCut {"
        assert source.count(original) == 1
        source = source.replace(original, "        drop(mutation);\n" + original, 1)
    path.write_text(source)
    errors = validate(fixture)
    assert any("reorders executable relation" in error or "blocks or escapes retained Queue ownership" in error
               for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


def test_geometry_pending_capacity_snapshot_precedes_inner_fences(fixture):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.WITNESS_LEASE
    anchor = "fn try_publication_lease"
    snapshot = "let pending_canonical_bytes = self\n            .try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards(&mut fences)?;"
    geometry = 'fences.geometry = Some(acquire("lane_geometry_lock", &self.lane_geometry_lock)?);'
    helper.replace_once_after(path, anchor, snapshot, "")
    helper.replace_once_after(path, anchor, geometry, geometry + "\n" + snapshot)
    assert any("reorders executable relation" in error for error in validate(fixture))


@pytest.mark.parametrize("owner,anchor,old,new", [
    pytest.param("APPLY", "fn carrier_queue_source", "OriginalCarrierQueue::new(&self.state, &self.queue)", "OriginalCarrierQueue::new(&self.state, &other_queue)", id="service-original-queue"),
    pytest.param("SERVICE_QUEUE", "impl<'service> OriginalCarrierQueue", "pub(super) fn new", "pub(crate) fn new", id="no-external-capability-construction"),
    pytest.param("SERVICE_QUEUE", "fn belongs_to", "core::ptr::eq(self.state, state)", "true", id="capability-original-state"),
    pytest.param("SERVICE_QUEUE", "fn try_observe", "self.queue.try_lock_lane_retirement_observer()", "other.try_lock_lane_retirement_observer()", id="original-observer"),
    pytest.param("SERVICE_QUEUE", "fn owns_cut", "cut.belongs_to(self.queue)", "true", id="capability-original-cut"),
    pytest.param("QUEUE_OWNER", "impl QueueLaneRetirementCut", "core::ptr::eq(self.observer.queue, queue)", "true", id="cut-original-queue"),
    pytest.param("QUEUE_OWNER", "impl QueueLaneRetirementCut", "self.observer.durability_faulted()", "false", id="cut-sticky-fault"),
    pytest.param("QUEUE_OWNER", "impl QueueLaneRetirementCut", "&self.reservations,", "&other_reservations,", id="cut-original-reservations"),
    pytest.param("CARRIER_QUEUE", "fn try_new", "!source.belongs_to(target)", "false", id="foreign-state"),
    pytest.param("CARRIER_QUEUE", "fn try_new", "!geometry.matches_publication_target(target, header)", "false", id="foreign-geometry"),
    pytest.param("CARRIER_QUEUE", "fn try_new", "!source.owns_cut(&cut)", "false", id="foreign-cut"),
    pytest.param("CARRIER_QUEUE", "fn try_new", "routes.push((lane, dataspace, incarnation))", "routes.push((lane, dataspace, Hash::zeroed()))", id="exact-predecessor-incarnation"),
    pytest.param("CARRIER_QUEUE", "fn try_new", "cut.lane_pending_work_release(lane, dataspace, incarnation)", "Ok(None)", id="pending-predicate"),
    pytest.param("CARRIER_QUEUE", "fn try_new", "Ok(Some(wait)) => {", "Ok(Some(wait)) if false => {", id="pending-cannot-authorize"),
    pytest.param("CARRIER_QUEUE", "fn try_new", "Err(error) => return Err(CarrierQueueRetirementError::Unavailable(error))", "Err(_) => {}", id="unavailable-cannot-authorize"),
    pytest.param("CARRIER_QUEUE", "fn try_new", "_cut: cut,", "_cut: other_cut,", id="retained-original-cut"),
    pytest.param("CARRIER_QUEUE", "fn ensure_available", "self._cut.durability_faulted()", "false", id="retained-sticky-fault"),
    pytest.param("CARRIER_QUEUE", "fn authenticates", "self.ensure_available().is_err()", "false", id="authentication-rechecks-fault"),
    pytest.param("CARRIER_QUEUE", "fn authenticates", "!self.state_owner.matches_state(target)", "false", id="authentication-original-state"),
    pytest.param("CARRIER_QUEUE", "fn authenticates", "self.header != header", "false", id="authentication-original-header"),
    pytest.param("CARRIER_QUEUE", "fn authenticates", "!geometry.matches_publication_target(target, header)", "false", id="authentication-original-geometry"),
    pytest.param("CARRIER_QUEUE", "fn authenticates", "self.routes.get(index) != Some(&(lane, dataspace, incarnation))", "false", id="authentication-exact-route"),
    pytest.param("CARRIER_QUEUE", "fn authenticates", "result.is_ok() && index == self.routes.len()", "result.is_ok()", id="authentication-complete-route-set"),
    pytest.param("GEOMETRY_CARRIER", "fn for_each_retirement_route", ".previous_catalog", ".updated_catalog", id="predecessor-catalog"),
    pytest.param("GEOMETRY_CARRIER", "fn for_each_retirement_route", "!lane_incarnation_is_zero(*incarnation)", "true", id="nonzero-predecessor-incarnation"),
    pytest.param("GEOMETRY_CARRIER", "fn has_queue_custody", "queue.is_some_and(|queue| queue.authenticates(target, self, header))", "queue.is_none_or(|queue| queue.authenticates(target, self, header))", id="absent-cut-refused"),
    pytest.param("PHYSICAL_CARRIER", "fn try_prepare_physical", "None => Some(CarrierQueueRetirementError::Missing)", "None => None", id="physical-requires-capability"),
    pytest.param("PHYSICAL_CARRIER", "fn try_prepare_physical", "Some(source) if !source.belongs_to(target)", "Some(source) if false", id="physical-original-service"),
    pytest.param("PHYSICAL_CARRIER", "fn try_complete_geometry", "journals.components._fences._queue.as_ref(),", "None,", id="completion-original-cut"),
    pytest.param("TERMINAL_CARRIER", "fn publish", "queue.ensure_available().err()", "None", id="terminal-sticky-fault"),
    pytest.param("PHYSICAL_CARRIER", "struct CarrierFences", "_queue: Option<CarrierQueueRetirement<'target>>,", "_queue: Option<CarrierQueueRetirement<'target>>, second_queue: Queue,", id="no-secondary-queue"),
])
def test_original_service_queue_cut_rejects_authority_substitution(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "retained carrier" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("anchor,old,new", [
    pytest.param("fn descriptor_layouts", "Layout::array::<Candidate<P::Owner>>(limit)", "Layout::array::<Marker>(limit)", id="actual-candidate-layout"),
    pytest.param("fn descriptor_layouts", "Layout::array::<Marker>(limit)", "Layout::array::<Marker>(1)", id="complete-marker-layout"),
    pytest.param("fn descriptor_layouts", ".map_err(|_| AllocationRefusal::DemandOverflow)?", ".unwrap()", id="checked-layout-overflow"),
    pytest.param("fn new", "budget.try_reserve_layouts(layouts)?", "other_budget.try_reserve_layouts(layouts)?", id="original-budget"),
    pytest.param("fn new", "reservation.try_split(layouts[0])?", "reservation.try_split(layouts[1])?", id="candidate-charge"),
    pytest.param("fn new", "reservation.try_split(layouts[1])?", "reservation.try_split(layouts[0])?", id="marker-charge"),
    pytest.param("fn new", "_descriptor_admission: descriptor_admission,", "_descriptor_admission: other_admission,", id="original-retained-charges"),
    pytest.param("fn new", "let mut candidates = Vec::new();", "let mut candidates = Vec::new(); let descriptor_admission = other_admission;", id="no-charge-shadow"),
])
def test_retained_descriptors_require_exact_prepaid_original_owners(fixture, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.VALIDATION_CUSTODY, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "retained carrier" in error for error in errors), errors


@pytest.mark.parametrize("mutation", ["early-allocation", "charge-drops-first", "early-queue-release"])
def test_retained_descriptor_and_cut_lifetimes_reject_early_release(fixture, mutation):
    root, helper, checker, _ = fixture
    contract = checker.native_preparation_contract
    if mutation == "early-allocation":
        path = root / contract.VALIDATION_CUSTODY
        allocation = "let mut candidates = Vec::new();\n        candidates.try_reserve_exact(limit)?;"
        helper.replace_once_after(path, "fn new", allocation, "")
        helper.replace_once_after(path, "fn new", "let layouts = Self::descriptor_layouts(limit)?;",
                                  allocation + "\n        let layouts = Self::descriptor_layouts(limit)?;")
    elif mutation == "charge-drops-first":
        path = root / contract.VALIDATION_CUSTODY
        field = "    _descriptor_admission: [AllocationCharge; 2],"
        helper.replace_once_after(path, "struct RetainedBodyValidationService", field, "")
        helper.replace_once_after(path, "struct RetainedBodyValidationService", "    candidates: Vec<Candidate<P::Owner>>,", field + "\n    candidates: Vec<Candidate<P::Owner>>,")
    else:
        path = root / contract.PHYSICAL_CARRIER
        helper.replace_once_after(path, "fn release_for_completion", "let queue = queue.map(CarrierQueueRetirement::release_deferred);", "")
        helper.replace_once_after(path, "fn release_for_completion", "let state = [write.release_deferred(), lifecycle.release_deferred()];", "let queue = queue.map(CarrierQueueRetirement::release_deferred);\n        let state = [write.release_deferred(), lifecycle.release_deferred()];")
    errors = validate(fixture)
    assert any("executable relation" in error or "retained carrier" in error for error in errors), errors


def test_native_preparation_ledger_tokens_match_exact_reviewed_items(fixture):
    root, _, checker, _ = fixture
    errors = []
    with checker._reviewed_rust_source_cache():
        for path, kind, symbol, tokens in checker.native_preparation_contract.PREPARATION_OWNER_BINDINGS:
            item = checker._rust_binding_item(root, path, kind, symbol, "Native raw ledger", errors)
            assert item is not None, errors
            for token in tokens:
                assert token in item, (symbol, token)
    assert errors == []


@pytest.mark.parametrize("owner,anchor,old,new", [
    pytest.param("STATE", "type BlockHashFamily", "concread::bptree::BptreeMapFamily<usize, HashOf<BlockHeader>, BlockHashMode>", "Arc<()>", id="real-family-storage"),
    pytest.param("STATE", "struct NativeLaneStateOwner", "(BlockHashFamily)", "(Arc<()>)", id="no-synthetic-owner"),
    pytest.param("STATE", "fn matches_state", "self.0.matches(map)", "true", id="exact-state-family"),
    pytest.param("STATE", "fn same_family", "self.0.same_family(&other.0)", "true", id="exact-published-family"),
    pytest.param("STATE", "fn native_lane_state_owner", "NativeLaneStateOwner(map.family())", "NativeLaneStateOwner(other.family())", id="capture-original-root"),
    pytest.param("STATE", "fn map(&self)", "BlockHashStorage::EmergencyFastMapped(_) | BlockHashStorage::EmergencyFastEmpty => None", "BlockHashStorage::EmergencyFastMapped(_) => Some(other)", id="mapped-state-no-mutable-family"),
    pytest.param("STATE", "fn pending(&self)", "start: self.visible_len", "start: 0", id="exact-pending-suffix"),
    pytest.param("STATE", "fn matches_block_predecessor", "self.mode == block.mode", "true", id="exact-replacement-mode"),
    pytest.param("STATE", "fn matches_block_predecessor", ".same_predecessor(&block.work.predecessor())", ".same_predecessor(&self.work.predecessor())", id="exact-predecessor-not-self"),
    pytest.param("HASH_SURFACE", "fn capture", "!std::ptr::eq(block.inner, expected)", "false", id="sealed-original-journal"),
    pytest.param("HASH_SURFACE", "fn capture", "block.work.predecessor().retain()", "other.work.predecessor().retain()", id="seal-retains-original-predecessor"),
    pytest.param('RETAINED_HASH_SLOT', 'fn try_prepare', 'self.original().work.try_matches_current_retaining(map)', 'Ok((true, None))', id='pre-admission-predecessor'),
    pytest.param("STATE", "fn observe_current", "self.work.try_matches_current(map)", "Ok(true)", id="original-advisory-predecessor"),
    pytest.param("STATE", "fn observe_current", "self.work.try_matches_current(map)", "drop(target.released.guard(())); self.work.try_matches_current(map)", id="advisory-release-not-busy-self-wake"),
    pytest.param('RETAINED_HASH_SLOT', 'fn try_prepare', 'let reader_wait = map.observe_reader_release();', 'let reader_wait = target.released.observe();', id='advisory-waits-for-actual-reader'),
    pytest.param('RETAINED_HASH_SLOT', 'fn try_prepare', 'let wait = map.observe_reader_release();', 'let wait = target.released.observe();', id='commit-preparation-cannot-wake-itself'),
    pytest.param('RETAINED_HASH_SLOT', 'fn try_prepare', 'map.try_acquire_owned(original.work)', 'other.try_acquire_owned(original.work)', id='original-final-reacquisition'),
    pytest.param('RETAINED_HASH_SLOT', 'fn recover_original', 'self.writer_release = Some(release);', 'self.writer_release = None;', id='changed-release-notification'),
    pytest.param('RETAINED_HASH_SLOT', 'fn try_prepare', 'slot.try_prepare()', '{ slot.prepare(); Ok(()) }', id='nonblocking-reader-acquisition'),
    pytest.param('RETAINED_HASH_SLOT', 'fn take_prepared', 'NativeLaneStateOwner(self.target.map().expect("original map").family())', 'NativeLaneStateOwner(other.map().expect("original map").family())', id='original-published-family'),
    pytest.param("HASH_PUBLICATION", "fn state_owner", "self.owner.clone()", "other.owner.clone()", id="publication-family-handoff"),
    pytest.param("HASH_PUBLICATION", "fn abort", "prepared.abort_retaining()", "other.abort_retaining()", id="abort-original-work"),
    pytest.param("HASH_PUBLICATION", "fn publish", "committed_height.store(height, Ordering::Release)", "committed_height.store(0, Ordering::Release)", id="exact-published-height"),
    pytest.param("HASH_PUBLICATION", "fn publish", "_retirement: retirement", "_retirement: { drop(retirement); unreachable!() }", id="retain-physical-cleanup"),
    pytest.param("PREFIX", "fn prepare_world_effects", ".eq([state._curr_block.hash()])", ".eq([])", id="exact-single-carrier-suffix"),
])
def test_shared_history_preserves_original_identity_and_refusal(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "shared history" in error or "retained carrier" in error
               for error in errors), errors
    assert not any("digest" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("owner,anchor,first,second", [
    ("HASH_PUBLICATION", "fn abort", "let (writer, reader) = prepared.abort_retaining();", "(writer.detach(), reader)"),
    ("HASH_PUBLICATION", "fn publish", "let published = prepared.map_preserving_release(|prepared| prepared.publish());", "committed_height.store(height, Ordering::Release);"),
    ("TERMINAL_CARRIER", "fn publish", "let hash_retirement;", "let AcquiredCarrierParticipants {"),
    ("TERMINAL_CARRIER", "fn publish", "drop(commit);", "drop(hash_retirement);"),
])
def test_shared_history_cleanup_and_publication_order(fixture, owner, anchor, first, second):
    root, helper, checker, _ = fixture
    path = root / getattr(checker.native_preparation_contract, owner)
    if second == "let AcquiredCarrierParticipants {":
        # Move a complete declaration below the complete destructuring statement.
        helper.replace_once_after(path, anchor, first, "")
        helper.replace_once_after(path, anchor, "} = components.into_original();", "} = components.into_original();\n        " + first)
    else:
        helper.replace_once_after(path, anchor, first, "")
        helper.replace_once_after(path, anchor, second, second + "\n        " + first)
    errors = validate(fixture)
    assert any("reorders executable relation" in error for error in errors), errors


@pytest.mark.parametrize("owner,anchor,old,new", [
    pytest.param("HASH_ADMISSION", "impl BlockHashAdmissionError", "Self::Busy(wait) | Self::Changed(wait) => Some(wait)", "Self::Busy(wait) | Self::Changed(wait) => None", id="original-physical-release"),
    pytest.param("HASH_ADMISSION", "impl<E: std::fmt::Debug> StateBlockStartError", "Self::History(error) => error.release_wait()", "Self::History(error) => None", id="history-release-survives-stage-wrapper"),
    pytest.param("HASH_RESTORE", "fn emergency_fast_block_hashes", "BlockHashes::new_emergency_fast_empty()", "BlockHashes::default()", id="empty-fast-remains-readonly"),
    pytest.param("STATE", "fn new_emergency_fast_empty", "inner: BlockHashStorage::EmergencyFastEmpty", "inner: BlockHashStorage::Owned(other)", id="empty-fast-has-no-mutable-tree"),
    pytest.param("RUNTIME_ACQUISITION", "fn acquire_canonical_runtime_block", "self.block_hashes.try_next_block(replacement)?", "self.block_hashes.try_next_block(false)?", id="exact-admitted-replacement-mode"),
    pytest.param("BODY_STORE", "impl HistoryAdmissionWait", "if pending.is_ready(wake)", "if false", id="release-before-registration-self-wake"),
    pytest.param("BODY_STORE", "impl HistoryAdmissionWait", "Self(wait.wait_for_release())", "Self(other.wait_for_release())", id="original-release-future"),
    pytest.param("BODY_STORE", "impl HistoryAdmissionWait", "std::pin::Pin::new(&mut self.0)", "std::pin::Pin::new(&mut other)", id="poll-original-dependency"),
    pytest.param("RUNNER_HISTORY", "fn history_admission_pending", "*pending_owner == owner", "true", id="proposal-owner-custody"),
    pytest.param("RUNNER_HISTORY", "fn history_admission_pending", "!pending.is_ready(wake)", "true", id="released-owner-can-resume"),
    pytest.param("RUNNER_HISTORY", "fn defer_history_admission", "HistoryAdmissionWait::new(wait.clone(), wake)", "HistoryAdmissionWait::new(other.clone(), wake)", id="retain-original-admission-refusal"),
    pytest.param("STATE", "type BlockHashMode", "concread::bptree::Prepaid<BlockHashPolicy>", "concread::bptree::Untracked", id="no-unfunded-mode"),
    pytest.param("RUNNER_HISTORY", "fn schedule_local_proposal", "proposal_state.history_admission_pending(owner, &queue.sumeragi_waker())", "false", id="dispatch-keeps-original-pending-dependency"),
    pytest.param("RUNNER_HISTORY", "fn schedule_local_proposal", ".rearm_loaded_candidate_delivery(current.tag(), loaded_round, loaded_subject)", ".rearm_loaded_candidate_delivery(current.tag(), 0, loaded_subject)", id="loaded-refusal-retains-round"),
    pytest.param("RUNNER_HISTORY", "let has_work = match candidate_block_has_proposal_work", ".rearm_loaded_candidate_delivery(current.tag(), loaded_round, loaded_subject)", '.rearm_loaded_candidate_delivery(current.tag(), loaded_round, { let mut subject = loaded_subject; subject.payload_hash = iroha_crypto::Hash::new(b"foreign subject"); subject })', id="loaded-refusal-retains-subject"),
    pytest.param("RUNNER_HISTORY", "let has_work = match candidate_block_has_proposal_work", ".map_err(V2RunnerError::Service)?;", ".ok();", id="loaded-refusal-propagates-rearm-failure"),
    pytest.param("RUNNER_HISTORY", "let has_work = match candidate_block_has_proposal_work", "return Ok(());", "continue;", id="loaded-refusal-yields-after-rearm"),
    pytest.param("RUNNER_HISTORY", "let assembly = match assembly", "Err(super::v2_candidate::CandidateError::LocalStateAdmission(error))", "Err(super::v2_candidate::CandidateError::Execution(error))", id="assembly-local-refusal-arms-wait"),
    pytest.param("LANE_WORK_HISTORY", "fn build_and_memoize_merge_execution_candidate", ".build_merge_execution_candidate(application_block_header, self.context.mode)?", ".build_merge_execution_candidate(application_block_header, self.context.mode).unwrap_or(None)", id="memoization-propagates-local-refusal"),
    pytest.param("LANE_WORK_HISTORY", "fn refresh_merge_candidates", "*view == active_view && !pending.is_ready(&wake)", "false", id="merge-waits-on-exact-view-dependency"),
    pytest.param("LANE_WORK_HISTORY", "fn refresh_merge_candidates", "let Some(wait) = error.release_wait() else", "let Some(wait) = None else", id="merge-refusal-retains-original-release"),
    pytest.param("LANE_WORK_HISTORY", "fn refresh_merge_candidates", "HistoryAdmissionWait::new(\n                                    wait.clone(),", "HistoryAdmissionWait::new(\n                                    concread::release::ReleaseNotification::default().observe(),", id="merge-cannot-substitute-release-owner"),
    pytest.param("RUNNER_HISTORY", "fn candidate_attachments", "error.downcast_ref::<crate::state::StateAdmissionError>()", "None::<&crate::state::StateAdmissionError>", id="npos-preserves-typed-history-refusal"),
    pytest.param("RUNNER_HISTORY", "let attachments = match attachments", "&queue.sumeragi_waker()", "&std::task::Waker::noop()", id="npos-arms-original-proposal-runner"),
    pytest.param("LANE_WORK_HISTORY", "fn classify_merge_state_validation", "MergeCandidateValidationError::Frontier(error.to_string())", "MergeCandidateValidationError::Invalid(error.to_string())", id="permanent-history-failure-is-local"),
    pytest.param("LANE_WORK_HISTORY", "fn classify_merge_state_validation", "Ok(MergeCandidateValidation::Deferred)", "Ok(MergeCandidateValidation::Ready)", id="temporary-history-refusal-never-authorizes"),
    pytest.param("LANE_WORK_HISTORY", "fn classify_merge_state_validation", "HistoryAdmissionWait::new(wait.clone(), &wake)", "HistoryAdmissionWait::new(concread::release::ReleaseNotification::default().observe(), &wake)", id="validation-retains-original-refund-wait"),
    pytest.param("LANE_WORK_HISTORY", "fn validate_merge_candidate_for_active_round", "self.classify_merge_state_validation(active_view, validation)?", "MergeCandidateValidation::Ready", id="no-positive-memo-on-history-refusal"),
    pytest.param("HASH_ADMISSION", "struct BlockHashPolicy", "(mv::allocation::AllocationReservation)", "(usize)", id="original-move-only-reservation"),
    pytest.param("HASH_ADMISSION", "impl NodeFunding for BlockHashPolicy", "type Charge = mv::allocation::AllocationCharge;", "type Charge = ();", id="actual-allocation-charge"),
    pytest.param("HASH_ADMISSION", "fn take_node_charge", "self.0", "other", id="no-provider-substitution"),
    pytest.param("HASH_ADMISSION", "fn admit_successor", ".checked_add(additional.bytes())", ".checked_add(0)", id="include-whole-successor-demand"),
    pytest.param("HASH_ADMISSION", "fn admit_successor", "if required > self.budget.limit_bytes()", "if false", id="permanent-floor-cannot-wait"),
    pytest.param("HASH_ADMISSION", "fn admit_successor", "requested_bytes: required", "requested_bytes: additional.bytes()", id="report-current-plus-successor-floor"),
    pytest.param("HASH_ADMISSION", "fn admit_successor", "self.admit(additional)", "self.admit(existing)", id="reserve-complete-new-demand"),
    pytest.param("HASH_ADMISSION", "fn admit(", "self.budget", "other.budget", id="original-configured-pool"),
    pytest.param("HASH_ADMISSION", "fn try_new", "budget.with_deferred_refund_notifications", "other.with_deferred_refund_notifications", id="constructor-original-refund-scope"),
    pytest.param("HASH_ADMISSION", "fn try_new", "budget: budget.clone()", "budget: other.clone()", id="startup-original-pool"),
    pytest.param("HASH_ADMISSION", "fn try_new", "owner.admit_successor(existing, additional)", "owner.admit(additional)", id="startup-permanent-floor"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", "self.map().ok_or(BlockHashAdmissionError::ReadOnly)?", "other.map().unwrap()", id="readonly-original-family"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", "view.len().saturating_sub(1)", "view.len()", id="replacement-tip-overwrite"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", "view.predecessor().retain()", "other.predecessor().retain()", id="pinned-reader-predecessor"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", "self.admit_successor(existing, additional)", "self.admit(additional)", id="no-permanent-capacity-wait"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", ".poisoning_guard(acquired)", ".guard(acquired)", id="actual-successor-poison-notification"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", "drop(acquired);", "std::mem::forget(acquired);", id="refused-acquisition-release-wake"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", "!predecessor.matches(&work.predecessor())", "false", id="reject-concurrent-reader-aba"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", "reserved_tip: Some(prefix)", "reserved_tip: None", id="hide-original-prepaid-tip"),
    pytest.param("HASH_ADMISSION", "fn try_next_block", "fixture_edits: false", "fixture_edits: true", id="no-postexecution-admission"),
    pytest.param("STATE", "fn push(&mut self, hash:", ".try_update_private(&index, hash)", ".try_update_private(&0, hash)", id="fill-exact-private-tip"),
    pytest.param("STATE", "fn push(&mut self, hash:", "self.fixture_edits", "true", id="exactly-one-production-tip"),
    pytest.param("STATE", "fn detach(self) -> DetachedBlockHashes", "reserved_tip: self.reserved_tip", "reserved_tip: None", id="detach-keeps-unfinished-state"),
    pytest.param("STATE", "macro_rules! work_hash_read", "self.work.len() - usize::from(self.reserved_tip.is_some())", "self.work.len()", id="hidden-tip-excluded-from-height"),
    pytest.param("STATE", "macro_rules! work_hash_read", "index < self.hash_count()", "true", id="hidden-tip-cannot-be-looked-up"),
    pytest.param("STATE", "macro_rules! work_hash_read", "end <= self.len()", "true", id="hidden-tip-cannot-escape-range"),
    pytest.param('RETAINED_HASH_SLOT', 'fn try_prepare', 'self.reserved_tip.is_some()', 'false', id='unfinished-tip-not-publishable'),
    pytest.param("HASH_ADMISSION", "impl BlockHashAdmissionError", "Some(release)", "None", id="capacity-refund-schedules-retry"),
    pytest.param("APPLY", "fn classify_validation_failure", "self.queue.sumeragi_waker()", "other.sumeragi_waker()", id="original-service-retry-waker"),
    pytest.param("CONTROLS", "fn map_block_err_to_reason", "| BlockValidationError::BlockHashAdmission(_)\n            | BlockValidationError::MembershipAdmission(_) => return None", "=> return None", id="local-refusal-no-peer-rejection"),
])
def test_prepaid_history_keeps_exact_funding_tip_and_local_refusal(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "prepaid history" in error or "shared history" in error
               or "retained carrier" in error for error in errors), errors
    assert not any("digest" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("move", ["capacity-before-floor", "publish-before-tip-check", "predecessor-after-admission"])
def test_prepaid_history_order_refuses_before_authority_or_new_allocation(fixture, move):
    root, helper, checker, _ = fixture
    c = checker.native_preparation_contract
    if move == "capacity-before-floor":
        path = root / c.HASH_ADMISSION
        helper.replace_once_after(path, "fn admit_successor", "self.admit(additional)", "admitted")
        helper.replace_once_after(path, "fn admit_successor", "let required =", "let admitted = self.admit(additional);\n        let required =")
    elif move == "publish-before-tip-check":
        path = root / c.RETAINED_HASH_SLOT
        guard = "if self.reserved_tip.is_some() {\n            return self.refuse(mv::PublicationPreparationError::Changed);\n        }"
        helper.replace_once_after(path, "fn try_prepare", guard, "")
        helper.replace_once_after(path, "fn try_prepare", "self.height = self.original().len();", guard + "\n        self.height = self.original().len();")
    else:
        path = root / c.HASH_ADMISSION
        # Move the complete retained cut/drop statements after admission; this
        # would compare a freshly observed generation instead of the original view.
        text = path.read_text()
        start = text.index("            let predecessor = match &view.inner", text.index("fn try_next_block"))
        end = text.index("            let wait = self.released.observe();", start)
        capture = text[start:end]
        text = text[:start] + text[end:]
        anchor = "            let (work, _) ="
        pos = text.index(anchor, text.index("fn try_next_block"))
        path.write_text(text[:pos] + capture + text[pos:])
    errors = validate(fixture)
    assert any("reorders executable relation" in error for error in errors), errors


@pytest.mark.parametrize("owner,anchor,old,new", [
    ("APPLY", "fn carrier_queue_source", "&self.queue", "&other.queue"),
    ("APPLY", "fn carrier_queue_source", "&self.state", "&other.state"),
    ("SERVICE_QUEUE", "impl<'service>", "pub(super) fn new", "pub(crate) fn new"),
    ("SERVICE_QUEUE", "fn owns_cut", "#[cfg(test)]", ""),
    ("SERVICE_QUEUE", "fn owns_cut", "    pub(crate) fn for_test", "    pub(crate) fn arbitrary(state: &State, queue: &Queue) -> Self { Self { state, queue } }\n    #[cfg(test)] pub(crate) fn for_test"),
    ("SERVICE_QUEUE", "struct OriginalCarrierQueue", "state: &'service State", "pub(crate) state: &'service State"),
    ("SERVICE_QUEUE", "fn belongs_to", "core::ptr::eq(self.state, state)", "true"),
    ("SERVICE_QUEUE", "fn owns_cut", "cut.belongs_to(self.queue)", "true"),
    ("SERVICE_QUEUE", "fn try_observe", "self.queue.try_lock_lane_retirement_observer()", "other.try_lock_lane_retirement_observer()"),
    ("QUEUE_OWNER", "impl QueueLaneRetirementCut", "core::ptr::eq(self.observer.queue, queue)", "true"),
    ("QUEUE_OWNER", "impl QueueLaneRetirementCut", "self.observer.durability_faulted()", "false"),
    ("QUEUE_OWNER", "impl QueueLaneRetirementCut", "self.observer.queue.lane_pending_work_release_locked(", "other.lane_pending_work_release_locked("),
    ("QUEUE_OWNER", "fn lane_pending_work_release_locked", "let scope = (lane_id, dataspace_id, lane_incarnation)", "let scope = (lane_id, dataspace_id, other_incarnation)"),
    ("QUEUE_OWNER", "fn lane_pending_work_release_locked", "if self.transaction_selection_durability_faulted()", "if false"),
    ("QUEUE_OWNER", "fn lane_pending_work_release_locked", ".is_none_or(|owned| self.lane_has_pending_route_work(&owned, lane_id, dataspace_id))", ".is_some_and(|owned| self.lane_has_pending_route_work(&owned, lane_id, dataspace_id))"),
    ("QUEUE_OWNER", "fn lane_pending_work_release_locked", "Ok(Some(wait))", "Ok(None)"),
    ("CARRIER_QUEUE", "fn try_new", "!source.belongs_to(target)", "false"),
    ("CARRIER_QUEUE", "fn try_new", "!geometry.matches_publication_target(target, header)", "false"),
    ("CARRIER_QUEUE", "fn try_new", "!source.owns_cut(&cut)", "false"),
    ("CARRIER_QUEUE", "fn try_new", "cut.lane_pending_work_release(lane, dataspace, incarnation)", "Ok(None)"),
    ("CARRIER_QUEUE", "fn try_new", "return Err(CarrierQueueRetirementError::Pending", "return Err(CarrierQueueRetirementError::InvalidDecision"),
    ("CARRIER_QUEUE", "fn try_new", "_cut: cut", "_cut: other_cut"),
    ("CARRIER_QUEUE", "fn ensure_available", "self._cut.durability_faulted()", "false"),
    ("CARRIER_QUEUE", "fn authenticates", "self.ensure_available().is_err()", "false"),
    ("CARRIER_QUEUE", "fn authenticates", "self.header != header", "false"),
    ("CARRIER_QUEUE", "fn authenticates", "!self.state_owner.matches_state(target)", "false"),
    ("CARRIER_QUEUE", "fn authenticates", "self.routes.get(index) != Some(&(lane, dataspace, incarnation))", "false"),
    ("CARRIER_QUEUE", "fn authenticates", "index == self.routes.len()", "true"),
    ("GEOMETRY_CARRIER", "fn for_each_retirement_route", ".previous_catalog", ".updated_catalog"),
    ("GEOMETRY_CARRIER", "fn for_each_retirement_route", ".previous_lane_incarnations", ".updated_lane_incarnations"),
    ("GEOMETRY_CARRIER", "fn for_each_retirement_route", "!lane_incarnation_is_zero(*incarnation)", "true"),
    ("GEOMETRY_CARRIER", "fn has_queue_custody", "queue.is_some_and(|queue| queue.authenticates(target, self, header))", "queue.is_none_or(|queue| queue.authenticates(target, self, header))"),
    ("PHYSICAL_CARRIER", "fn try_prepare_physical", "None => Some(CarrierQueueRetirementError::Missing)", "None => None"),
    ("PHYSICAL_CARRIER", "fn try_prepare_physical", "Some(source) if !source.belongs_to(target)", "Some(source) if false"),
    ("PHYSICAL_CARRIER", "fn try_prepare_physical", "observer\n                    .try_into_cut()", "observer\n                    .try_into_cut().await"),
    ("PHYSICAL_CARRIER", "fn try_acquire", "lock.try_lock_or_wait()", "lock.lock()"),
    ("PHYSICAL_CARRIER", "fn try_complete_geometry", "journals.components._fences._queue.as_ref()", "None"),
    ("TERMINAL_CARRIER", "fn publish(", ".and_then(|queue| queue.ensure_available().err())", ".and_then(|queue| None)"),
])
def test_carrier_retirement_requires_original_queue_and_shared_release_predicate(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in e or "sticky Queue fault" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("mutation", ["observer-after-state", "cut-after-components", "release-before-write", "fault-before-storage", "fences-before-components"])
def test_carrier_retirement_rejects_guard_and_sticky_fault_reordering(fixture, mutation):
    root, helper, checker, _ = fixture
    c = checker.native_preparation_contract
    if mutation == "observer-after-state":
        helper.replace_once_after(root / c.PHYSICAL_CARRIER, "fn try_prepare_physical", "let queue_observer = if authenticated", "let _early = StateFences::try_acquire(target); let queue_observer = if authenticated")
    elif mutation == "cut-after-components":
        helper.replace_once_after(root / c.PHYSICAL_CARRIER, "fn try_prepare_physical", "let queue = match queue_observer", "let _early = journals.try_map_components(); let queue = match queue_observer")
    elif mutation == "release-before-write":
        path = root / c.PHYSICAL_CARRIER
        helper.replace_once_after(path, "fn release_for_completion", "let queue = queue.map(CarrierQueueRetirement::release_deferred);", "")
        helper.replace_once_after(path, "fn release_for_completion", "let state = [write.release_deferred(), lifecycle.release_deferred()];", "let queue = queue.map(CarrierQueueRetirement::release_deferred); let state = [write.release_deferred(), lifecycle.release_deferred()];")
    elif mutation == "fault-before-storage":
        path = root / c.TERMINAL_CARRIER
        text = path.read_text()
        start = text.index("        if let Some(error) = this\n", text.index("fn publish("))
        end = text.index("\n        // Reservations", start)
        block = text[start:end]
        text = text[:start] + text[end:]
        target = text.index("        let update_da_mapping = match this.try_complete_geometry()")
        path.write_text(text[:target] + block + "\n" + text[target:])
    else:
        path = root / c.PHYSICAL_CARRIER
        helper.replace_once_after(path, "struct AcquiredCarrierParticipants", "    _fences: CarrierFences<'target>,\n", "")
        helper.replace_once_after(path, "struct AcquiredCarrierParticipants", "    world:", "    _fences: CarrierFences<'target>,\n    world:")
    errors = validate(fixture)
    assert any("executable relation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("owner,anchor,old,new", [
    ("VALIDATION_CUSTODY", "fn descriptor_layouts", "Layout::array::<Candidate<P::Owner>>(limit)", "Layout::array::<usize>(limit)"),
    ("VALIDATION_CUSTODY", "fn descriptor_layouts", "Layout::array::<Marker>(limit)", "Layout::array::<Marker>(1)"),
    ("VALIDATION_CUSTODY", "fn descriptor_layouts", ".map_err(|_| AllocationRefusal::DemandOverflow)?", ".unwrap()"),
    ("VALIDATION_CUSTODY", "fn descriptor_bytes", ".checked_add(layout.size())", ".saturating_add(layout.size())"),
    ("VALIDATION_CUSTODY", "fn new(\n        validator", "budget: &AllocationBudget", "budget: Option<&AllocationBudget>"),
    ("VALIDATION_CUSTODY", "fn new(\n        validator", "budget.try_reserve_layouts(layouts)?", "other.try_reserve_layouts(layouts)?"),
    ("VALIDATION_CUSTODY", "fn new(\n        validator", "reservation.try_split(layouts[1])?", "reservation.try_split(layouts[0])?"),
    ("VALIDATION_CUSTODY", "fn new(\n        validator", "_descriptor_admission: descriptor_admission", "_descriptor_admission: other_charge"),
    ("VALIDATION_CUSTODY", "fn new(\n        validator", "let mut candidates = Vec::new();", "drop(descriptor_admission); let mut candidates = Vec::new();"),
    ("RETAINED_VALIDATION", "fn retained_validation_service", "            budget,", "            other_budget,"),
    ("RETAINED_VALIDATION", "fn retained_validation_descriptor_bytes", "self.capacity.max_body_entries", "1"),
])
def test_retained_descriptor_admission_requires_actual_checked_layouts_and_original_pool(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("mutation", ["allocate-before-reserve", "charge-drops-before-vectors"])
def test_retained_descriptor_admission_precedes_allocation_and_outlives_vectors(fixture, mutation):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.VALIDATION_CUSTODY
    if mutation == "allocate-before-reserve":
        helper.replace_once_after(path, "fn new(\n        validator", "let layouts = Self::descriptor_layouts(limit)?;", "let mut candidates = Vec::new(); candidates.try_reserve_exact(limit)?; let layouts = Self::descriptor_layouts(limit)?;")
    else:
        helper.replace_once_after(path, "struct RetainedBodyValidationService", "    _descriptor_admission: [AllocationCharge; 2],\n", "")
        helper.replace_once_after(path, "struct RetainedBodyValidationService", "    candidates:", "    _descriptor_admission: [AllocationCharge; 2],\n    candidates:")
    errors = validate(fixture)
    assert any("executable relation" in e or "original custody" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("owner,anchor,old,new", [
    ("PUBLICATION_MUTEX", "fn release_deferred", "self.inner.release_deferred(drop).1", "other.release_deferred(drop).1"),
    ("KURA", "fn take_cleanup", "self.prune.take().map(PublicationGuard::release_deferred)", "None"),
    ("QUEUE_OWNER", "fn release_deferred", "reservations.release_deferred()", "other.release_deferred()"),
    ("CARRIER_QUEUE", "fn release_deferred", "_routes: routes", "_routes: Vec::new()"),
    ("TERMINAL_CARRIER", "fn publish(", "membership_retirement = transactions.publish();", "transactions.publish();"),
    ("TERMINAL_CARRIER", "fn publish(", "drop(membership_retirement);", ""),
])
def test_completion_retains_original_release_owners(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    contract = checker.native_preparation_contract
    path = root / getattr(contract, owner)
    if owner == "KURA":
        path = root / "crates/iroha_core/src/kura/publication_lease.rs"
    helper.replace_once_after(path, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "retained carrier" in error for error in errors), errors


def test_completion_unlocks_commit_before_deferred_callbacks(fixture):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.PHYSICAL_CARRIER
    helper.swap_ordered_once_after(path, "struct CompletionFences", "_commit: PublicationGuard<'target>,", "_state: [concread::release::DeferredRelease; 2],")
    assert any("executable relation" in error for error in validate(fixture))


@pytest.mark.parametrize("anchor,old,new", [
    ("impl Drop for AcquiredCarrierComponents", "drop(original.abort())", "drop(original)"),
    ("impl AcquiredCarrierParticipants", "let (world, world_retirement) = world.abort()", "let (world, world_retirement) = replacement.abort()"),
    ("impl AcquiredCarrierParticipants", "drop(fences.release_for_completion())", "drop(fences)"),
])
def test_carrier_abandonment_keeps_joint_original_release(fixture, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.native_preparation_contract.PHYSICAL_CARRIER, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors


@pytest.mark.parametrize("cut", ["runtime", "world"])
def test_partial_carrier_refusal_keeps_cleanup_after_original_fences(fixture, cut):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.PARTICIPANT_PREPARATION
    helper.replace_once_after(path, "fn recover_original",
                              "self.release_fences();",
                              f"drop(self.{cut}.take()); self.release_fences();")
    errors = validate(fixture)
    assert any("executable relation" in e for e in errors), errors


def test_world_refusal_preserves_original_cleanup_shells(fixture):
    root, helper, _, _ = fixture
    path = root / "crates/iroha_core/src/state/world_preparation.rs"
    helper.replace_once_after(path, "fn into_cleanup", "_fields: fields,",
                              "_fields: PreparedWorldFields(Vec::new()),")
    errors = validate(fixture)
    assert any("executable relation" in e for e in errors), errors


@pytest.mark.parametrize("path,owner,first,last", [
    ("crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs", "PreparedRuntimeJournals", "canonical_runtime", "lane_consensus_contexts"),
    ("crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs", "PreparedSet", "data_triggers", "contracts"),
])
@pytest.mark.parametrize("cut", ["raw-drop", "early-notification", "early-capacity"])
def test_aggregate_abandonment_retains_joint_release_and_original_capacity(fixture, path, owner, first, last, cut):
    root, helper, _, _ = fixture
    anchor = f"Drop for {owner}"
    if cut == "raw-drop":
        helper.replace_once_after(root / path, anchor,
                                  f"let {first} = {first}.abort();", f"let {first} = {first};")
    elif cut == "early-notification":
        helper.replace_once_after(root / path, anchor,
                                  f"let {last} = {last}.abort();", f"drop({first}); let {last} = {last}.abort();")
    else:
        helper.replace_once_after(root / path, anchor,
                                  f"let {first} = {first}.abort();", f"drop((admission, installation)); let {first} = {first}.abort();")
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors


@pytest.mark.parametrize("cut", ["state", "queue"])
def test_partial_fence_refusal_retains_original_cleanup_until_all_fences_release(fixture, cut):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.PHYSICAL_CARRIER
    anchor = f"Err((error, {cut}_retirement)) =>"
    helper.replace_once_after(path, anchor, "let kura_retirement = kura.release_deferred();",
                              f"drop({cut}_retirement); let kura_retirement = kura.release_deferred();")
    assert any("executable relation" in error for error in validate(fixture))


def test_autoscale_queue_refusal_retains_cleanup_through_lifecycle(fixture):
    root, helper, checker, _ = fixture
    helper.swap_ordered_once_after(root / checker.native_preparation_contract.APPLY,
                                  "fn try_validate_autoscale_retirement_queue_binding", "drop(lifecycle_guard);", "drop(cleanup);")
    assert any("executable relation" in error for error in validate(fixture))


@pytest.mark.parametrize("path,anchor,old,new", [
    ('crates/iroha_core/src/kura/publication_lease.rs', 'fn take_cleanup', 'self.sidecar.take().map(PublicationGuard::release_deferred)', 'self.sidecar.take().map(|guard| { drop(guard); panic!() })'),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'fn take_cleanup', '_cold_sidecar: self.cold_sidecar.take()', '_cold_sidecar: None'),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'fn release_cold_sidecar', 'self.sidecar = Some(sidecar);', 'drop(sidecar);'),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'fn drop(&mut self)', 'drop(self.take_cleanup());', 'drop(self.sidecar.take());'),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'fn try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards', 'fences.release_cold_sidecar()?;', 'drop(fences.sidecar.take());'),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'fn try_publication_lease', 'let mut fences = AcquiredKuraPublicationFences::new(self);', 'let mut fences = AcquiredKuraPublicationFences::new(other);'),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'fn authenticate_archive_capture', 'fences.canonical = Some(self.canonical_chain_lock.lock());', 'fences.canonical = Some(other.canonical_chain_lock.lock());'),
    ('vendor/concread/src/release.rs', 'fn deferred_batch', 'released: false', 'released: true'),
    ('vendor/concread/src/release.rs', 'fn try_release_into<R>', 'if !Arc::ptr_eq(&self.notification.state, &batch.notification.state)', 'if false'),
    ('vendor/concread/src/release.rs', 'fn try_release_into<R>', 'return Err(self);', 'drop(self); panic!();'),
    ('vendor/concread/src/release.rs', 'fn try_release_into<R>', 'self.batch.released = true;', 'self.batch.released = false;'),
    ('vendor/concread/src/release.rs', 'fn try_release_into<R>', 'self.poison.observe()', 'false'),
    ('vendor/concread/src/release.rs', 'fn try_release_into<R>', 'let result = release(inner);\n        drop(record);', 'drop(record);\n        let result = release(inner);'),
    ('vendor/concread/src/release.rs', 'impl Drop for DeferredReleaseBatch', 'if self.released', 'if true'),
    ('crates/iroha_core/src/publication_lock.rs', 'fn try_release_into', '.try_release_into(batch, drop)', '.try_release_into(other, drop)'),
    ('crates/iroha_core/src/kura.rs', 'fn merge_entry_by_hash_after_sidecar', 'self.ensure_prune_recovery_not_required()?;', '// bypass prune refusal'),
])
def test_kura_joint_release_requires_original_physical_ownership(fixture, path, anchor, old, new):
    root, helper, _, _ = fixture
    helper.replace_once_after(root / path, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("path,anchor,old,new", [
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn acquire_owned', 'if !Shared::ptr_eq(&self.write, &owned.root)', 'if false', id='foreign-before-acquisition'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn acquire_owned', '(error.into_inner(), true)', '(error.into_inner(), false)', id='poison-keeps-real-guard'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn validate(', 'return Err((self, OwnedWriteError::Changed));', 'drop(self); panic!();', id='stale-keeps-custody'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn validate(', '!Shared::ptr_eq(&self.guard.current, &self.owned.base)', 'false', id='exact-original-predecessor'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn abort(self)', 'drop(guard);', 'let _retained = guard;', id='abort-unlocks-first'),
    pytest.param('vendor/concread/src/bptree/mod.rs', 'fn try_acquire_owned', 'try_acquire_owned(owned.inner)', 'try_acquire_owned(other.inner)', id='original-map-root'),
    pytest.param('crates/mv/src/storage/physical.rs', 'fn acquire_owned_writer', '(owned, error, None)', '(owned, error, Some(released.guard(()).release_deferred(drop).1))', id='no-phantom-release'),
    pytest.param('crates/mv/src/storage/physical.rs', 'fn acquire_owned_writer', '.poisoning_guard(acquired)', '.poisoning_guard(())', id='bind-actual-acquisition'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'fn try_prepare', 'self.installation = Some(installation);', 'drop(installation);', id='hash-cleanup-retains-installation'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'fn recover_original', 'self.writer_release = Some(release);', 'drop(release);', id='hash-refusal-keeps-signal'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'fn recover_original', 'self.release_fences();', 'drop(std::mem::replace(&mut self.block_hashes, other)); self.release_fences();', id='aggregate-fences-before-hash-cleanup'),
    pytest.param('crates/iroha_core/src/state.rs', 'fn commit_inner(', '} = this.fields.as_mut().expect("original executing State");', '} = this.into_fields();', id='ordinary-state-keeps-cleanup'),
])
def test_acquired_writer_retains_actual_guard_and_cleanup(fixture, path, anchor, old, new):
    root, helper, _, _ = fixture
    helper.replace_once_after(root / path, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("path,anchor,old,new", [
    pytest.param('crates/iroha_core/src/state/block_hashes_admission.rs', 'fn try_next_block', 'let wait = map.observe_reader_release();', 'let wait = self.released.observe();', id='reader-only-release-wakes-successor'),
    pytest.param('crates/iroha_core/src/state/block_hashes_admission.rs', 'fn try_next_block', '.poisoning_guard(acquired)', '.poisoning_guard(())', id='actual-successor-guard-before-callback'),
    pytest.param('crates/iroha_core/src/state/block_hashes_admission.rs', 'fn try_next_block', 'writer.release_with(|(writer, previous)| (writer.detach(), previous))', 'writer.map_preserving_release(|(writer, previous)| (writer.detach(), previous))', id='detachment-releases-original-notification'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn try_acquire_writer', '(error.into_inner(), true)', '(error.into_inner(), false)', id='acquired-poison-retained'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn try_write_charged', 'if self.poisoned', 'if false', id='poison-before-admission'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn try_write_charged', 'return Err((self, WriterAdmissionError::Refused(error)))', 'panic!("lost original acquisition")', id='refusal-retains-original-guard'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'fn try_write_charged', 'caller.create_writer(guard, admission)', 'other.create_writer(guard, admission)', id='original-construction-owner'),
    pytest.param('vendor/concread/src/bptree/admission.rs', 'fn current_footprint', 'source.node_counts()', '(0, 0)', id='exact-permanent-footprint'),
    pytest.param('vendor/concread/src/bptree/admission.rs', 'fn insert_with_source<E>(\n        self,', 'WriterAdmissionError::Poisoned => MapAdmissionError::Poisoned', 'WriterAdmissionError::Poisoned => MapAdmissionError::Busy', id='poison-is-never-retryable-contention'),
    pytest.param('vendor/concread/src/bptree/admission.rs', 'fn insert_with_source<E>(\n        self,', 'BptreeMapWriteTxn { inner: writer }', 'BptreeMapWriteTxn { inner: other }', id='same-writer-after-admission'),
])
def test_successor_admission_retains_original_acquisition_and_release(fixture, path, anchor, old, new):
    root, helper, _, _ = fixture
    helper.replace_once_after(root / path, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param('LinCowCell::acquire_writer', '(error.into_inner(), true)', '(error.into_inner(), false)', id='blocking-retains-poison'),
    pytest.param('LinCowCellWriterAcquisition::is_poisoned', 'self.poisoned', 'false', id='actual-acquired-poison'),
    pytest.param('LinCowCellWriterAcquisition::write_with', 'input: input(data)', 'input: panic!("skip original input")', id='original-input-construction'),
    pytest.param('LinCowCell::write_with', 'self.acquire_writer().write_with(input)', 'self.try_acquire_writer().expect("busy").write_with(input)', id='blocking-cell-delegates-acquisition'),
    pytest.param('BptreeMap::acquire_writer', 'self.inner.acquire_writer()', 'self.inner.try_acquire_writer().expect("busy")', id='blocking-map-delegates-acquisition'),
    pytest.param('BptreeMapWriterAcquisition::is_poisoned', 'self.inner.is_poisoned()', 'false', id='map-forwards-original-poison'),
    pytest.param('BptreeMapWriterAcquisition::write', 'self.inner.write_with(|_| ())', 'self.inner.write_with(|_| panic!("lost original input"))', id='same-acquired-untracked-cursor'),
    pytest.param('BptreeMap::write', 'self.acquire_writer().write()', 'self.try_acquire_writer().expect("busy").write()', id='ordinary-map-delegates-acquisition'),
    pytest.param('BptreeMap::try_write_admitted', 'acquired.try_write_admitted(admit)', '{ drop(acquired); self.try_acquire_writer().expect("reacquire").try_write_admitted(admit) }', id='start-never-reacquires'),
    pytest.param('BptreeMap::try_clear_admitted', 'acquired.try_clear_admitted(admit)', '{ drop(acquired); self.try_acquire_writer().expect("reacquire").try_clear_admitted(admit) }', id='clear-never-reacquires'),
    pytest.param('BptreeMapWriterAcquisition::write_with_source', 'return Err((Self { inner }, error));', 'drop(inner); panic!("lost refused acquisition");', id='start-refusal-retains-lock'),
    pytest.param('BptreeMapWriterAcquisition::try_clear_admitted', 'return Err((Self { inner }, error));', 'drop(inner); panic!("lost refused acquisition");', id='clear-refusal-retains-lock'),
    pytest.param('BptreeMapWriterAcquisition::write_with_source', 'writer.as_mut().finish_admitted_funding();', '// original unused funding was not sealed', id='start-funding-cleanup-before-return'),
    pytest.param('BptreeMapWriterAcquisition::try_clear_admitted', 'inner.as_mut().finish_admitted_funding();', '// unused original funding was not sealed', id='clear-funding-cleanup-before-return'),
    pytest.param('ReleaseGuard::try_map_pair_preserving_release', 'signals.armed = false;', '// successful ownership transfer still signals', id='success-does-not-wake'),
    pytest.param('ReleaseGuard::try_map_pair_preserving_release', 'let _first = std::mem::ManuallyDrop::new(self);', 'let _first = self;', id='original-first-notification-transferred'),
    pytest.param('ReleaseGuard::try_map_pair_preserving_release', 'let _second = std::mem::ManuallyDrop::new(other);', 'let _second = other;', id='original-second-notification-transferred'),
    pytest.param('ReleaseGuard::try_map_pair_preserving_release', 'match consume(first, second) {', 'signals.first.released(false);\n        match consume(first, second) {', id='no-callback-before-callee-unwind'),
    pytest.param('ReleaseGuard::try_map_pair_preserving_release', 'notification: _second.notification', 'notification: _first.notification', id='second-source-is-original'),
    pytest.param('ReleaseGuard::try_map_pair_preserving_release', 'poisoned: second,', 'poisoned: std::thread::panicking(),', id='second-uses-physical-verdict'),
    pytest.param('ReleaseGuard::try_map_pair_preserving_release', 'let second = Signal {\n                    notification: self.second,\n                    poisoned: second,\n                };', 'drop(first);\n                let second = Signal {\n                    notification: self.second,\n                    poisoned: (self.observe_poison)().1,\n                };\n                let first = ();', id='both-verdicts-before-first-wake'),
    pytest.param('ReleaseGuard::release_pair_with', '|first, second| Err(consume(first, second))', '|first, second| { drop(first); drop(second); panic!("skip release custody") }', id='release-uses-same-transition-engine'),
    pytest.param('BlockAcquisitionSlot::initialize', '.poisoning_guard(target.revert.acquire_writer())', '.poisoning_guard(target.revert.write())', id='ordinary-both-raw-before-first-conversion'),
    pytest.param('BlockAcquisitionSlot::initialize', '    fn initialize(&mut self, mode: BlockMode) {\n        assert!(!self.started, "original storage acquisition is one-shot");\n        self.started = true;\n        let target = self.target;\n        let AcquisitionPhase::Pending { undo, current } = &mut self.phase else {\n            panic!("original pending acquisition");\n        };\n        *undo = WriterPhase::Raw(\n            target\n                .revert_released\n                .poisoning_guard(target.revert.acquire_writer()),\n        );\n        assert!(!undo.is_poisoned(), "original undo writer is poisoned");\n        *current = WriterPhase::Raw(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );\n        assert!(\n            !current.is_poisoned(),\n            "original storage writer is poisoned"\n        );\n        let raw = undo.take_raw();\n        *undo = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.undo_release,\n                    |undo| Ok::<_, (_, std::convert::Infallible)>(undo.write()),\n                    || target.revert.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original undo release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        let raw = current.take_raw();\n        *current = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.current_release,\n                    |current| Ok::<_, (_, std::convert::Infallible)>(current.write()),\n                    || target.blocks.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original current release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        // The caller slot owns completed writers before identity lookup and all\n        // reset/replacement operations that may invoke arbitrary payload code.\n        let predecessor = target.publication.capture();\n        let writers = StorageWriters::new(target, undo.take_writer(), current.take_writer());\n        self.phase = AcquisitionPhase::Block(Block::new(\n            writers,\n            mode == BlockMode::Replace,\n            predecessor,\n            mode,\n        ));\n        let AcquisitionPhase::Block(block) = &mut self.phase else {\n            unreachable!("original storage block");\n        };\n        block.failed = true;\n        let OriginalWriters { revert, blocks } = block.writers.as_mut();\n        if mode == BlockMode::Replace {\n            for (key, value) in revert.iter() {\n                match value {\n                    None => blocks.remove(key),\n                    Some(value) => blocks.insert(key.clone(), value.clone()),\n                };\n            }\n        }\n        revert.clear();\n        block.failed = false;\n        self.complete = true;\n    }', '    fn initialize(&mut self, mode: BlockMode) {\n        assert!(!self.started, "original storage acquisition is one-shot");\n        self.started = true;\n        let target = self.target;\n        let AcquisitionPhase::Pending { undo, current } = &mut self.phase else {\n            panic!("original pending acquisition");\n        };\n        *current = WriterPhase::Raw(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );\n        assert!(\n            !current.is_poisoned(),\n            "original storage writer is poisoned"\n        );\n        *undo = WriterPhase::Raw(\n            target\n                .revert_released\n                .poisoning_guard(target.revert.acquire_writer()),\n        );\n        assert!(!undo.is_poisoned(), "original undo writer is poisoned");\n        let raw = undo.take_raw();\n        *undo = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.undo_release,\n                    |undo| Ok::<_, (_, std::convert::Infallible)>(undo.write()),\n                    || target.revert.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original undo release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        let raw = current.take_raw();\n        *current = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.current_release,\n                    |current| Ok::<_, (_, std::convert::Infallible)>(current.write()),\n                    || target.blocks.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original current release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        // The caller slot owns completed writers before identity lookup and all\n        // reset/replacement operations that may invoke arbitrary payload code.\n        let predecessor = target.publication.capture();\n        let writers = StorageWriters::new(target, undo.take_writer(), current.take_writer());\n        self.phase = AcquisitionPhase::Block(Block::new(\n            writers,\n            mode == BlockMode::Replace,\n            predecessor,\n            mode,\n        ));\n        let AcquisitionPhase::Block(block) = &mut self.phase else {\n            unreachable!("original storage block");\n        };\n        block.failed = true;\n        let OriginalWriters { revert, blocks } = block.writers.as_mut();\n        if mode == BlockMode::Replace {\n            for (key, value) in revert.iter() {\n                match value {\n                    None => blocks.remove(key),\n                    Some(value) => blocks.insert(key.clone(), value.clone()),\n                };\n            }\n        }\n        revert.clear();\n        block.failed = false;\n        self.complete = true;\n    }', id='ordinary-undo-before-current'),
    pytest.param('BlockAcquisitionSlot::initialize', '!current.is_poisoned()', 'true', id='ordinary-checks-current-poison'),
    pytest.param('WriterPhase::release', '|| target.is_poisoned(),', '|| std::thread::panicking(),', id='ordinary-actual-poison-pair'),
    pytest.param('BlockAcquisitionSlot::initialize', '    fn initialize(&mut self, mode: BlockMode) {\n        assert!(!self.started, "original storage acquisition is one-shot");\n        self.started = true;\n        let target = self.target;\n        let AcquisitionPhase::Pending { undo, current } = &mut self.phase else {\n            panic!("original pending acquisition");\n        };\n        *undo = WriterPhase::Raw(\n            target\n                .revert_released\n                .poisoning_guard(target.revert.acquire_writer()),\n        );\n        assert!(!undo.is_poisoned(), "original undo writer is poisoned");\n        *current = WriterPhase::Raw(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );\n        assert!(\n            !current.is_poisoned(),\n            "original storage writer is poisoned"\n        );\n        let raw = undo.take_raw();\n        *undo = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.undo_release,\n                    |undo| Ok::<_, (_, std::convert::Infallible)>(undo.write()),\n                    || target.revert.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original undo release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        let raw = current.take_raw();\n        *current = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.current_release,\n                    |current| Ok::<_, (_, std::convert::Infallible)>(current.write()),\n                    || target.blocks.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original current release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        // The caller slot owns completed writers before identity lookup and all\n        // reset/replacement operations that may invoke arbitrary payload code.\n        let predecessor = target.publication.capture();\n        let writers = StorageWriters::new(target, undo.take_writer(), current.take_writer());\n        self.phase = AcquisitionPhase::Block(Block::new(\n            writers,\n            mode == BlockMode::Replace,\n            predecessor,\n            mode,\n        ));\n        let AcquisitionPhase::Block(block) = &mut self.phase else {\n            unreachable!("original storage block");\n        };\n        block.failed = true;\n        let OriginalWriters { revert, blocks } = block.writers.as_mut();\n        if mode == BlockMode::Replace {\n            for (key, value) in revert.iter() {\n                match value {\n                    None => blocks.remove(key),\n                    Some(value) => blocks.insert(key.clone(), value.clone()),\n                };\n            }\n        }\n        revert.clear();\n        block.failed = false;\n        self.complete = true;\n    }', '    fn initialize(&mut self, mode: BlockMode) {\n        assert!(!self.started, "original storage acquisition is one-shot");\n        self.started = true;\n        let target = self.target;\n        let AcquisitionPhase::Pending { undo, current } = &mut self.phase else {\n            panic!("original pending acquisition");\n        };\n        let predecessor = target.publication.capture();\n        *undo = WriterPhase::Raw(\n            target\n                .revert_released\n                .poisoning_guard(target.revert.acquire_writer()),\n        );\n        assert!(!undo.is_poisoned(), "original undo writer is poisoned");\n        *current = WriterPhase::Raw(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );\n        assert!(\n            !current.is_poisoned(),\n            "original storage writer is poisoned"\n        );\n        let raw = undo.take_raw();\n        *undo = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.undo_release,\n                    |undo| Ok::<_, (_, std::convert::Infallible)>(undo.write()),\n                    || target.revert.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original undo release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        let raw = current.take_raw();\n        *current = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.current_release,\n                    |current| Ok::<_, (_, std::convert::Infallible)>(current.write()),\n                    || target.blocks.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original current release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        // The caller slot owns completed writers before identity lookup and all\n        // reset/replacement operations that may invoke arbitrary payload code.\n        let writers = StorageWriters::new(target, undo.take_writer(), current.take_writer());\n        self.phase = AcquisitionPhase::Block(Block::new(\n            writers,\n            mode == BlockMode::Replace,\n            predecessor,\n            mode,\n        ));\n        let AcquisitionPhase::Block(block) = &mut self.phase else {\n            unreachable!("original storage block");\n        };\n        block.failed = true;\n        let OriginalWriters { revert, blocks } = block.writers.as_mut();\n        if mode == BlockMode::Replace {\n            for (key, value) in revert.iter() {\n                match value {\n                    None => blocks.remove(key),\n                    Some(value) => blocks.insert(key.clone(), value.clone()),\n                };\n            }\n        }\n        revert.clear();\n        block.failed = false;\n        self.complete = true;\n    }', id='ordinary-capture-after-pair'),
    pytest.param('BlockAcquisitionSlot::initialize', '    fn initialize(&mut self, mode: BlockMode) {\n        assert!(!self.started, "original storage acquisition is one-shot");\n        self.started = true;\n        let target = self.target;\n        let AcquisitionPhase::Pending { undo, current } = &mut self.phase else {\n            panic!("original pending acquisition");\n        };\n        *undo = WriterPhase::Raw(\n            target\n                .revert_released\n                .poisoning_guard(target.revert.acquire_writer()),\n        );\n        assert!(!undo.is_poisoned(), "original undo writer is poisoned");\n        *current = WriterPhase::Raw(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );\n        assert!(\n            !current.is_poisoned(),\n            "original storage writer is poisoned"\n        );\n        let raw = undo.take_raw();\n        *undo = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.undo_release,\n                    |undo| Ok::<_, (_, std::convert::Infallible)>(undo.write()),\n                    || target.revert.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original undo release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        let raw = current.take_raw();\n        *current = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.current_release,\n                    |current| Ok::<_, (_, std::convert::Infallible)>(current.write()),\n                    || target.blocks.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original current release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        // The caller slot owns completed writers before identity lookup and all\n        // reset/replacement operations that may invoke arbitrary payload code.\n        let predecessor = target.publication.capture();\n        let writers = StorageWriters::new(target, undo.take_writer(), current.take_writer());\n        self.phase = AcquisitionPhase::Block(Block::new(\n            writers,\n            mode == BlockMode::Replace,\n            predecessor,\n            mode,\n        ));\n        let AcquisitionPhase::Block(block) = &mut self.phase else {\n            unreachable!("original storage block");\n        };\n        block.failed = true;\n        let OriginalWriters { revert, blocks } = block.writers.as_mut();\n        if mode == BlockMode::Replace {\n            for (key, value) in revert.iter() {\n                match value {\n                    None => blocks.remove(key),\n                    Some(value) => blocks.insert(key.clone(), value.clone()),\n                };\n            }\n        }\n        revert.clear();\n        block.failed = false;\n        self.complete = true;\n    }', '    fn initialize(&mut self, mode: BlockMode) {\n        assert!(!self.started, "original storage acquisition is one-shot");\n        self.started = true;\n        let target = self.target;\n        let AcquisitionPhase::Pending { undo, current } = &mut self.phase else {\n            panic!("original pending acquisition");\n        };\n        *undo = WriterPhase::Raw(\n            target\n                .revert_released\n                .poisoning_guard(target.revert.acquire_writer()),\n        );\n        assert!(!undo.is_poisoned(), "original undo writer is poisoned");\n        let predecessor = target.publication.capture();\n        *current = WriterPhase::Raw(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );\n        assert!(\n            !current.is_poisoned(),\n            "original storage writer is poisoned"\n        );\n        let raw = undo.take_raw();\n        *undo = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.undo_release,\n                    |undo| Ok::<_, (_, std::convert::Infallible)>(undo.write()),\n                    || target.revert.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original undo release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        let raw = current.take_raw();\n        *current = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.current_release,\n                    |current| Ok::<_, (_, std::convert::Infallible)>(current.write()),\n                    || target.blocks.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original current release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        // The caller slot owns completed writers before identity lookup and all\n        // reset/replacement operations that may invoke arbitrary payload code.\n        let writers = StorageWriters::new(target, undo.take_writer(), current.take_writer());\n        self.phase = AcquisitionPhase::Block(Block::new(\n            writers,\n            mode == BlockMode::Replace,\n            predecessor,\n            mode,\n        ));\n        let AcquisitionPhase::Block(block) = &mut self.phase else {\n            unreachable!("original storage block");\n        };\n        block.failed = true;\n        let OriginalWriters { revert, blocks } = block.writers.as_mut();\n        if mode == BlockMode::Replace {\n            for (key, value) in revert.iter() {\n                match value {\n                    None => blocks.remove(key),\n                    Some(value) => blocks.insert(key.clone(), value.clone()),\n                };\n            }\n        }\n        revert.clear();\n        block.failed = false;\n        self.complete = true;\n    }', id='replacement-capture-after-pair'),
    pytest.param('Storage::open_admitted_writers', 'let revert = self.revert_released.poisoning_guard(revert);', 'let revert = self.revert_released.poisoning_guard(());', id='admitted-binds-actual-undo'),
    pytest.param('Storage::open_admitted_writers', 'let blocks = self.blocks_released.poisoning_guard(blocks);', 'let blocks = self.blocks_released.poisoning_guard(());', id='admitted-binds-actual-current'),
    pytest.param('Storage::open_admitted_writers', 'if blocks.is_poisoned()', 'if false', id='both-poison-before-first-policy'),
    pytest.param('Storage::open_admitted_writers', 'let (current, undo, identity) = reserve_owners(budget, current, undo, identity)?;', 'let (current, undo, identity) = reserve_owners(budget, undo, current, identity)?;', id='original-component-demands'),
    pytest.param('Storage::open_admitted_writers', 'let writers = StorageWriters::new(self, revert, blocks);\n        let next = NextPublication::from_admission(identity);', 'let next = NextPublication::from_admission(identity);\n        let writers = StorageWriters::new(self, revert, blocks);', id='identity-after-complete-custody'),
    pytest.param('ReleaseGuard::release_with_observed_poison', 'let result = consume(inner);\n        drop(signal);', 'drop(signal);\n        let result = consume(inner);', id='single-release-after-physical-unlock'),
    pytest.param('ReleaseGuard::release_with_observed_poison', 'self.notification.released((self.observe_poison)());', 'self.notification.released(std::thread::panicking());', id='single-release-preserves-native-poison'),
    pytest.param('ReleaseGuard::release_with_observed_poison', 'let _transferred = std::mem::ManuallyDrop::new(self);', 'let _transferred = self;', id='single-original-notification-only'),
    pytest.param('BlockAcquisitionSlot::initialize', '!undo.is_poisoned()', 'true', id='ordinary-undo-poison-before-wait'),
    pytest.param('Storage::open_admitted_writers', 'if revert.is_poisoned()', 'if false', id='admitted-undo-poison-before-busy'),
    pytest.param('Storage::open_admitted_writers', 'revert.release_with_observed_poison(drop, || self.revert.is_poisoned());', 'revert.release_with(drop);', id='admitted-refusal-preserves-actual-poison'),
    pytest.param('Storage::open_admitted_writers', 'if revert.is_poisoned() {\n            revert.release_with_observed_poison(drop, || self.revert.is_poisoned());\n            return Err(AdmittedStorageError::Poisoned {\n                role: StorageRole::Undo,\n            });\n        }\n        let current_wait = self.blocks_released.observe();\n        let blocks = self.blocks.try_acquire_writer().ok_or_else(|| {\n            writer_error(\n                MapAdmissionError::Busy,\n                StorageRole::Current,\n                current_wait.clone(),\n            )\n        })?;\n        let blocks = self.blocks_released.poisoning_guard(blocks);\n', 'let current_wait = self.blocks_released.observe();\n        let blocks = self.blocks.try_acquire_writer().ok_or_else(|| {\n            writer_error(\n                MapAdmissionError::Busy,\n                StorageRole::Current,\n                current_wait.clone(),\n            )\n        })?;\n        let blocks = self.blocks_released.poisoning_guard(blocks);\n        if revert.is_poisoned() {\n            revert.release_with_observed_poison(drop, || self.revert.is_poisoned());\n            return Err(AdmittedStorageError::Poisoned {\n                role: StorageRole::Undo,\n            });\n        }\n', id='admitted-complete-refusal-precedes-current-acquisition'),
])
def test_fresh_pair_acquisition_requires_original_joint_custody(fixture, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [row for row in (*checker.native_preparation_contract.FRESH_PAIR_ACQUISITION_BINDINGS,
                            *checker.native_preparation_contract.MEMBERSHIP_CURRENT_OWNER_BINDINGS)
            if row[2] == symbol]
    assert len(rows) == 1
    path, kind, _, _ = rows[0]
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, kind, symbol)
    assert len(items) == 1
    item = items[0]
    assert item.count(old) == 1, (symbol, old)
    assert old != new
    # Restrict the mutation to this exact impl, even where another owner's
    # delegation has identical source text (for example is_poisoned).
    owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0])
              if item in owner]
    assert len(owners) == 1
    owner = owners[0]
    assert source.count(owner) == 1
    target.write_text(source.replace(owner, owner.replace(item, item.replace(old, new, 1), 1), 1))
    errors = validate(fixture)
    assert any(f"{symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param('EbrCell::acquire_writer', '.unwrap_or_else(|poison| poison.into_inner())', '.unwrap()', id='blocking-raw-retains-poison'),
    pytest.param('EbrCell::try_acquire_writer', 'Err(TryLockError::Poisoned(poison)) => poison.into_inner(),', 'Err(TryLockError::Poisoned(_)) => return None,', id='nonblocking-poison-is-acquired'),
    pytest.param('EbrCellWriterAcquisition::is_poisoned', 'self.caller.is_poisoned()', 'false', id='actual-raw-poison'),
    pytest.param('EbrCellWriterAcquisition::try_clone_charged', 'if self.is_poisoned()', 'if false', id='poison-before-admission'),
    pytest.param('EbrCellWriterAcquisition::try_clone_charged', 'unsafe { epoch::unprotected() }', '&epoch::pin()', id='exclusive-load-does-not-run-collector'),
    pytest.param('EbrCellWriterAcquisition::try_clone_charged', 'EbrCell::<T, Charge>::allocation_layout()', 'Layout::new::<T>()', id='exact-original-generation-layout'),
    pytest.param('EbrCellWriterAcquisition::try_clone_charged', 'let charge = match admit', 'let _speculative = current.value.clone();\n        let charge = match admit', id='admit-before-any-clone'),
    pytest.param('EbrCellWriterAcquisition::try_clone_charged', 'Err(error) => return Err((self, EbrCellWriterAdmissionError::Refused(error))),', 'Err(error) => { drop(self); panic!("lost original refusal owner"); },', id='refusal-retains-raw-owner'),
    pytest.param('EbrCellWriterAcquisition::try_clone_charged', '    pub fn try_clone_charged<E>(\n        self,\n        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,\n    ) -> Result<(Self, EbrCellOwned<T, Charge>), (Self, EbrCellWriterAdmissionError<E>)> {\n        if self.is_poisoned() {\n            return Err((self, EbrCellWriterAdmissionError::Poisoned));\n        }\n        // SAFETY: this original writer excludes replacement, and its borrowed\n        // cell excludes destruction. The active allocation therefore cannot be\n        // unlinked while admission and cloning run; no collector pin is needed.\n        let current = self\n            .caller\n            .active\n            .load(Acquire, unsafe { epoch::unprotected() });\n        let current = unsafe { current.deref() };\n        let charge = match admit(&current.value, EbrCell::<T, Charge>::allocation_layout()) {\n            Ok(charge) => ManuallyDrop::new(charge),\n            Err(error) => return Err((self, EbrCellWriterAdmissionError::Refused(error))),\n        };\n        let allocation = Owned::new(Allocation {\n            value: current.value.clone(),\n            charge,\n        });\n        Ok((\n            self,\n            EbrCellOwned {\n                data: Some(allocation),\n            },\n        ))\n    }', '    pub fn try_clone_charged<E>(\n        self,\n        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,\n    ) -> Result<(Self, EbrCellOwned<T, Charge>), (Self, EbrCellWriterAdmissionError<E>)> {\n        if self.is_poisoned() {\n            return Err((self, EbrCellWriterAdmissionError::Poisoned));\n        }\n        // SAFETY: this original writer excludes replacement, and its borrowed\n        // cell excludes destruction. The active allocation therefore cannot be\n        // unlinked while admission and cloning run; no collector pin is needed.\n        let current = self\n            .caller\n            .active\n            .load(Acquire, unsafe { epoch::unprotected() });\n        let current = unsafe { current.deref() };\n        let charge = match admit(&current.value, EbrCell::<T, Charge>::allocation_layout()) {\n            Ok(charge) => charge,\n            Err(error) => return Err((self, EbrCellWriterAdmissionError::Refused(error))),\n        };\n        let allocation = Owned::new(Allocation {\n            value: current.value.clone(),\n            charge: ManuallyDrop::new(charge),\n        });\n        Ok((\n            self,\n            EbrCellOwned {\n                data: Some(allocation),\n            },\n        ))\n    }', id='partial-clone-retains-charge'),
    pytest.param('EbrCellWriterAcquisition::try_clone_charged', 'value: current.value.clone(),', 'value: { let _duplicate = current.value.clone(); current.value.clone() },', id='clone-exactly-once'),
    pytest.param('EbrCellWriterAcquisition::try_write_owned', 'if self.is_poisoned()', 'if false', id='attachment-preserves-poison'),
    pytest.param('EbrCellWriterAcquisition::install_owned', 'data: owned.data.take(),', 'data: None,', id='attachment-takes-original-allocation'),
    pytest.param('EbrCellWriteTxn::drop', 'drop(self._guard.take());', '// physical guard retained until after payload destruction', id='native-abort-unlocks-before-reclaim'),
    pytest.param('EbrCellWriteTxn::detach', 'data: self.data.take(),', 'data: None,', id='native-detach-retains-allocation'),
    pytest.param('EbrCell::write_from_guard', 'match acquired.try_clone_charged(admit)', 'match acquired.try_clone_charged(|_, _| panic!("lost original admission"))', id='native-singleton-shares-clone-kernel'),
    pytest.param('EbrCell::write_charged', 'self.write_from_guard(self.write.lock().unwrap(), admit)', 'self.write_from_guard(self.write.lock().unwrap(), |_, _| panic!("lost original admission"))', id='blocking-singleton-preserves-admission'),
    pytest.param('EbrCell::try_write_charged', 'self.write_from_guard(mguard, admit).map(Some)', 'self.write_from_guard(mguard, |_, _| panic!("lost original admission")).map(Some)', id='try-singleton-preserves-admission'),
    pytest.param('EbrCell::try_write_owned', 'drop(acquired);', 'std::mem::forget(acquired);', id='owned-refusal-unlocks-before-return'),
    pytest.param('BlockAcquisitionSlot::release', '                let target = self.target;', '                drop(pending.undo_value.take());\n                let target = self.target;', id='completed-undo-retirement-after-pair-unlock'),
    pytest.param('BlockAcquisitionSlot::release', '                let target = self.target;', '                drop(pending.current_charge.take());\n                let target = self.target;', id='partial-abort-keeps-unused-charge'),
    pytest.param('BlockAcquisitionSlot::initialize_writers', '!pending\n                .revert\n                .as_ref()\n                .expect("original undo")\n                .is_poisoned()', 'true', id='first-known-poison-before-current-wait'),
    pytest.param('BlockAcquisitionSlot::release', 'target.revert.is_poisoned()', 'std::thread::panicking()', id='first-poison-uses-observed-verdict'),
    pytest.param('BlockAcquisitionSlot::initialize_writers', '!pending\n                .blocks\n                .as_ref()\n                .expect("original current")\n                .is_poisoned()', 'true', id='known-current-poison-before-clones'),
    pytest.param('BlockAcquisitionSlot::initialize_writers', 'pending.blocks = Some(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );', 'drop(target.blocks_released.poisoning_guard(target.blocks.acquire_writer()));', id='constructor-retains-current-raw'),
    pytest.param('BlockAcquisitionSlot::initialize_writers', 'Ok(undo) => pending.revert = Some(undo),', 'Ok(undo) => drop(undo),', id='undo-raw-survives-current-clone'),
    pytest.param('BlockAcquisitionSlot::initialize_writers', 'Ok((undo, value)) => {\n                        *undo_value = Some(value);', 'Ok((undo, value)) => {\n                        drop(value);', id='completed-undo-survives-current-clone'),
    pytest.param('BlockAcquisitionSlot::initialize_writers', 'let undo_charge = &mut pending.undo_charge;', 'drop(pending.current_charge.take());\n        let undo_charge = &mut pending.undo_charge;', id='unused-current-charge-stays-outside-undo-callee'),
    pytest.param('BlockAcquisitionSlot::release', '    fn release(&mut self) {\n        self.complete = false;\n        self.started = true;\n        match &mut self.phase {\n            AcquisitionPhase::Empty => {}\n            AcquisitionPhase::Block(block) => crate::BlockRetirement::release_writers(block),\n            AcquisitionPhase::Writers(writers) => writers.release(),\n            AcquisitionPhase::Pending(pending) => {\n                let target = self.target;\n                if let Some(current) = pending.blocks.take() {\n                    current\n                        .try_release_into_observed(&mut self.current_release, drop, || {\n                            target.blocks.is_poisoned()\n                        })\n                        .unwrap_or_else(|_| unreachable!("original current release source"));\n                }\n                if let Some(undo) = pending.revert.take() {\n                    undo.try_release_into_observed(&mut self.undo_release, drop, || {\n                        target.revert.is_poisoned()\n                    })\n                    .unwrap_or_else(|_| unreachable!("original undo release source"));\n                }\n            }\n        }\n    }', '    fn release(&mut self) {\n        self.complete = false;\n        self.started = true;\n        match &mut self.phase {\n            AcquisitionPhase::Empty => {}\n            AcquisitionPhase::Block(block) => crate::BlockRetirement::release_writers(block),\n            AcquisitionPhase::Writers(writers) => writers.release(),\n            AcquisitionPhase::Pending(pending) => {\n                let target = self.target;\n                if let Some(current) = pending.blocks.take() {\n                    current\n                        .try_release_into_observed(&mut self.current_release, drop, || {\n                            std::thread::panicking()\n                        })\n                        .unwrap_or_else(|_| unreachable!("original current release source"));\n                }\n                if let Some(undo) = pending.revert.take() {\n                    undo.try_release_into_observed(&mut self.undo_release, drop, || {\n                        std::thread::panicking()\n                    })\n                    .unwrap_or_else(|_| unreachable!("original undo release source"));\n                }\n            }\n        }\n    }', id='both-native-poison-verdicts'),
    pytest.param('CellWriters::detach_retaining', 'let (revert, undo_release) = revert.release_deferred(|writer| writer.detach());', 'let (revert, undo_release) = revert.release_deferred(|writer| writer.detach());\n        drop(revert);\n        let revert = panic!("lost original undo");', id='detach-keeps-both-owned-generations'),
    pytest.param('CellWriters::release', 'let (undo, undo_release) = revert.release_deferred(|writer| writer.detach());', 'let (undo, undo_release) = revert.release_deferred(|writer| writer.detach());\n        drop(undo);\n        let undo = panic!("original undo prematurely reclaimed");', id='abandon-keeps-undo-until-current-unlock'),
    pytest.param('ReleaseGuard::release_deferred', 'let poisoned = policy.observe();', 'let poisoned = false;', id='abandon-reports-physical-verdict'),
    pytest.param('Cell::acquire_charged_writers', 'CellWriters::acquire(self, charges)', '{ drop(charges); panic!("lost original charges") }', id='cell-common-constructor-delegates-pair'),
    pytest.param('BlockAcquisitionSlot::initialize', 'self.initialize_writers();\n        let predecessor = self.target.publication.capture();', 'let predecessor = self.target.publication.capture();\n        self.initialize_writers();', id='block_charged-captures-after-pair'),
    pytest.param('BlockAcquisitionSlot::initialize', 'self.initialize_writers();\n        let predecessor = self.target.publication.capture();', 'let predecessor = self.target.publication.capture();\n        self.initialize_writers();', id='block_and_revert_charged-captures-after-pair'),
    pytest.param('Cell::current_replacement_charged', 'writers: self.acquire_charged_writers(charges),', 'writers: { drop(charges); panic!("lost same-cut pair") },', id='replacement-keeps-original-current-undo-pair'),
    pytest.param('CurrentReplacement::publish', 'publish_pair(writers, publication, NextPublication::new(), true, false);', 'publish_pair(writers, publication, NextPublication::new(), true, true);', id='current-replacement-preserves-undo-publication'),
    pytest.param('PreparedCellWriters::prepare_attached', 'self.revert.as_mut().expect("original undo").prepare();', 'if dirty { self.revert.as_mut().expect("original undo").prepare(); }', id='untouched-block-still-publishes-clear-undo'),
    pytest.param('Block::try_detach', 'crate::BlockCapture::try_capture(&mut slot, admit)?;', 'crate::BlockCapture::try_capture(&mut slot, admit).unwrap_or_else(|_| panic!("lost refusal"));', id='detach-admission-errors-propagate'),
    pytest.param('Block::try_detach', 'let (journal, cleanup) = crate::BlockCapture::into_detached(slot);', 'drop(slot);\n            let (journal, cleanup) = panic!("lost detached generations");', id='detach-does-not-destroy-originals'),
    pytest.param('publish_pair', "fn publish_pair<'a, V: Value, Charge: Send + Sync + 'static>(\n    writers: CellWriters<'a, V, Charge>,\n    publication: &Publication,\n    next: NextPublication,\n    publish_current: bool,\n    publish_undo: bool,\n) {\n    let retirement = publication.publish_retaining(\n        next,\n        || {\n            // Retain the complete joint owner until the fallible identity-lock\n            // acquisition above succeeds. Native preparation and publication\n            // below neither allocate nor execute payload or collector callbacks.\n            let OriginalCellWriters { revert, blocks } = writers.into_original();\n            let (blocks, unchanged_blocks) = if publish_current {\n                (\n                    Some(blocks.map_preserving_release(|writer| writer.prepare_commit())),\n                    None,\n                )\n            } else {\n                (None, Some(blocks))\n            };\n            let (revert, unchanged_revert) = if publish_undo {\n                (\n                    Some(revert.map_preserving_release(|writer| writer.prepare_commit())),\n                    None,\n                )\n            } else {\n                (None, Some(revert))\n            };\n            let blocks =\n                blocks.map(|writer| writer.map_preserving_release(|prepared| prepared.publish()));\n            let revert =\n                revert.map(|writer| writer.map_preserving_release(|prepared| prepared.publish()));\n            (blocks, revert, unchanged_blocks, unchanged_revert)\n        },\n        |(blocks, revert, unchanged_blocks, unchanged_revert)| {\n            let blocks =\n                blocks.map(|writer| writer.release_retaining(|published| published.release()));\n            let revert =\n                revert.map(|writer| writer.release_retaining(|published| published.release()));\n            let unchanged_blocks =\n                unchanged_blocks.map(|writer| writer.release_retaining(|writer| writer.detach()));\n            let unchanged_revert =\n                unchanged_revert.map(|writer| writer.release_retaining(|writer| writer.detach()));\n            (blocks, revert, unchanged_blocks, unchanged_revert)\n        },\n    );\n    drop(retirement);\n}", "fn publish_pair<'a, V: Value, Charge: Send + Sync + 'static>(\n    writers: CellWriters<'a, V, Charge>,\n    publication: &Publication,\n    next: NextPublication,\n    publish_current: bool,\n    publish_undo: bool,\n) {\n    let OriginalCellWriters { revert, blocks } = writers.into_original();\n    let retirement = publication.publish_retaining(\n        next,\n        || {\n            // Retain the complete joint owner until the fallible identity-lock\n            // acquisition above succeeds. Native preparation and publication\n            // below neither allocate nor execute payload or collector callbacks.\n            \n            let (blocks, unchanged_blocks) = if publish_current {\n                (\n                    Some(blocks.map_preserving_release(|writer| writer.prepare_commit())),\n                    None,\n                )\n            } else {\n                (None, Some(blocks))\n            };\n            let (revert, unchanged_revert) = if publish_undo {\n                (\n                    Some(revert.map_preserving_release(|writer| writer.prepare_commit())),\n                    None,\n                )\n            } else {\n                (None, Some(revert))\n            };\n            let blocks =\n                blocks.map(|writer| writer.map_preserving_release(|prepared| prepared.publish()));\n            let revert =\n                revert.map(|writer| writer.map_preserving_release(|prepared| prepared.publish()));\n            (blocks, revert, unchanged_blocks, unchanged_revert)\n        },\n        |(blocks, revert, unchanged_blocks, unchanged_revert)| {\n            let blocks =\n                blocks.map(|writer| writer.release_retaining(|published| published.release()));\n            let revert =\n                revert.map(|writer| writer.release_retaining(|published| published.release()));\n            let unchanged_blocks =\n                unchanged_blocks.map(|writer| writer.release_retaining(|writer| writer.detach()));\n            let unchanged_revert =\n                unchanged_revert.map(|writer| writer.release_retaining(|writer| writer.detach()));\n            (blocks, revert, unchanged_blocks, unchanged_revert)\n        },\n    );\n    drop(retirement);\n}", id='identity-lock-before-pair-extraction'),
    pytest.param('Publication::publish_retaining', 'let mut version = self.lock_version();\n        let published = publish();', 'let published = publish();\n        let mut version = self.lock_version();', id='native-publication-after-identity-lock'),
    pytest.param('Publication::publish_retaining', 'retired_version = std::mem::replace(&mut **version, next.0);\n        let retirement = release(published);', 'let retirement = release(published);\n        retired_version = std::mem::replace(&mut **version, next.0);', id='identity-rotates-before-native-release'),
])
def test_ebr_fresh_pair_acquisition_requires_original_joint_custody(fixture, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [row for row in (*checker.native_preparation_contract.EBR_PAIR_ACQUISITION_BINDINGS,
                            *checker.native_preparation_contract.ATTACHED_PUBLICATION_BINDINGS)
            if row[2] == symbol]
    assert len(rows) == 1
    path, kind, _, _ = rows[0]
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, kind, symbol)
    assert len(items) == 1
    item = items[0]
    assert item.count(old) == 1, (symbol, old)
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    if kind == "method":
        owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0])
                  if item in owner]
        assert len(owners) == 1
        owner = owners[0]
    else:
        owner = item
    assert source.count(owner) == 1
    target.write_text(source.replace(owner, owner.replace(item, item.replace(old, new, 1), 1), 1))
    errors = validate(fixture)
    assert any(f"{symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_map_preserving_release_into', '!Arc::ptr_eq(&self.notification.state, &batch.notification.state)', 'false', id='callee-unwind-keeps-exact-native-source'),
    pytest.param('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_map_preserving_release_into', 'let transferred = std::mem::ManuallyDrop::new(self);', 'let transferred = self;', id='callee-unwind-does-not-run-local-notifier'),
    pytest.param('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_map_preserving_release_into', 'let result = consume(inner);\n        record.armed = false;', 'record.armed = false;\n        let result = consume(inner);', id='callee-unwind-record-remains-armed'),
    pytest.param('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_map_preserving_release_into', 'record.armed = false;\n        drop(record);', 'drop(record);', id='normal-phase-transfer-does-not-release'),
    pytest.param('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_map_preserving_release_into', 'self.batch.poisoned |= (self.observe_poison)();', 'self.batch.poisoned |= std::thread::panicking();', id='callee-unwind-native-poison-only'),
    pytest.param('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_release_into_observed', '!Arc::ptr_eq(&self.notification.state, &batch.notification.state)', 'false', id='retirement-does-not-rebind-source'),
    pytest.param('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_release_into_observed', 'let result = release(inner);\n        drop(record);', 'drop(record);\n        let result = release(inner);', id='physical-release-before-observed-verdict'),
    pytest.param('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_release_into_observed', 'self.batch.poisoned |= (self.observe_poison)();', 'self.batch.poisoned = false;', id='known-poison-survives-normal-retirement'),
    pytest.param('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapWriteTxn::abort_retaining', '_inner: self.inner.detach(),', '_inner: { self.inner.as_ref().len(); self.inner.detach() },', id='failed-map-cursor-can-retire-without-read'),
    pytest.param('vendor/concread/src/bptree/mod.rs', 'struct', 'BptreeMapAbandonment', '    _inner:', '    pub inner:', id='map-retirement-grants-no-public-owner-access'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::new', 'undo_charge: Some(undo),', 'undo_charge: { drop(undo); None },', id='caller-slot-keeps-original-unused-charge'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::initialize', 'let AcquisitionPhase::Block(block) = &mut self.phase else {', 'let mut escaped = std::mem::replace(&mut self.phase, AcquisitionPhase::Empty);\n        let AcquisitionPhase::Block(block) = &mut escaped else {', id='cell-reset-remains-in-caller-slot'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::release', 'AcquisitionPhase::Block(block) => crate::BlockRetirement::release_writers(block),', 'AcquisitionPhase::Block(block) => { let _ = block; },', id='cell-slot-retires-completed-block'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::release', 'AcquisitionPhase::Writers(writers) => writers.release(),', 'AcquisitionPhase::Writers(writers) => { let _ = writers; },', id='cell-slot-retires-completed-pair-before-capture'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::into_block', 'self.complete,', 'true,', id='partial-cell-cannot-transfer'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::drop', 'crate::BlockAcquisition::release(self);', 'let _ = self;', id='default-cell-slot-drop-releases-before-fields'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::drop', 'self.release();', 'let _ = self;', id='default-cell-pair-drop-shares-retirement'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'enum', 'CellWriterState', '        _undo: EbrCellOwned<Option<V>, C>,\n        _current: EbrCellOwned<V, C>,\n        _undo_release: DeferredRelease,\n        _current_release: DeferredRelease,', '        _undo_release: DeferredRelease,\n        _current_release: DeferredRelease,\n        _undo: EbrCellOwned<Option<V>, C>,\n        _current: EbrCellOwned<V, C>,', id='cell-cleanup-before-original-notifications'),
    pytest.param('crates/mv/src/cell.rs', 'method', 'Cell::block_charged', 'crate::BlockAcquisition::initialize(&mut slot, BlockMode::Ordinary);', 'crate::BlockAcquisition::initialize(&mut slot, BlockMode::Replace);', id='ordinary-cell-uses-one-mode-aware-kernel'),
    pytest.param('crates/mv/src/cell.rs', 'method', 'Cell::block_and_revert_charged', 'crate::BlockAcquisition::initialize(&mut slot, BlockMode::Replace);', 'crate::BlockAcquisition::initialize(&mut slot, BlockMode::Ordinary);', id='replacement-cell-uses-one-mode-aware-kernel'),
    pytest.param('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::initialize', '&mut self.current_release,', '&mut self.undo_release,', id='map-callee-current-source-is-original'),
    pytest.param('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::initialize', 'let AcquisitionPhase::Block(block) = &mut self.phase else {', 'let mut escaped = std::mem::replace(&mut self.phase, AcquisitionPhase::Empty);\n        let AcquisitionPhase::Block(block) = &mut escaped else {', id='map-replacement-prefix-remains-in-slot'),
    pytest.param('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::initialize', 'block.failed = true;', 'block.failed = false;', id='map-reset-and-replay-arm-logical-failure'),
    pytest.param('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::initialize', 'revert.clear();\n        block.failed = false;', 'block.failed = false;\n        revert.clear();', id='map-clear-finishes-before-disarming'),
    pytest.param('crates/mv/src/storage/acquisition.rs', 'method', 'WriterPhase::release', '*self = Self::Retired(retirement);', 'drop(retirement);', id='map-retains-cursor-after-physical-release'),
    pytest.param('crates/mv/src/storage/acquisition.rs', 'method', 'inherent BlockAcquisitionSlot::into_block', 'self.complete,', 'true,', id='partial-map-cannot-transfer'),
    pytest.param('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::drop', 'self.release();', 'let _ = self;', id='default-map-slot-drop-releases-before-fields'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'StorageWriters::release', 'let (blocks, blocks_release) = blocks.release_deferred(|writer| writer.abort_retaining());', 'let (blocks, blocks_release) = blocks.release_deferred(|writer| writer.abort_retaining());\n        drop(blocks);\n        let blocks = panic!("premature original current destruction");', id='map-retirement-retains-current-through-undo-unlock'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'StorageWriters::drop', 'self.release();', 'let _ = self;', id='default-map-pair-drop-shares-retirement'),
    pytest.param('crates/mv/src/storage.rs', 'enum', 'StorageWriterState', '        _blocks: BptreeMapAbandonment<K, V, M>,\n        _revert: BptreeMapAbandonment<K, Option<V>, M>,\n        _blocks_release: concread::release::DeferredRelease,\n        _revert_release: concread::release::DeferredRelease,', '        _blocks_release: concread::release::DeferredRelease,\n        _revert_release: concread::release::DeferredRelease,\n        _blocks: BptreeMapAbandonment<K, V, M>,\n        _revert: BptreeMapAbandonment<K, Option<V>, M>,', id='map-cleanup-before-original-notifications'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'Storage::block', 'crate::BlockAcquisition::initialize(&mut slot, BlockMode::Ordinary);', 'crate::BlockAcquisition::initialize(&mut slot, BlockMode::Replace);', id='ordinary-map-uses-one-mode-aware-kernel'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'Storage::block_and_revert', 'crate::BlockAcquisition::initialize(&mut slot, BlockMode::Replace);', 'crate::BlockAcquisition::initialize(&mut slot, BlockMode::Ordinary);', id='replacement-map-uses-one-mode-aware-kernel'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'Block::release_writers', 'self.writers.release();', 'let _ = &self.writers;', id='map-complete-block-retires-original-pair'),
    pytest.param('crates/mv/src/cell.rs', 'method', 'Block::release_writers', 'self.writers.release();', 'let _ = &self.writers;', id='cell-complete-block-retires-original-pair'),
 ])
def test_aggregate_acquisition_requires_retained_caller_slots(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [row for row in checker.native_preparation_contract.PREPARATION_OWNER_BINDINGS
            if row[:3] == (path, kind, symbol)]
    assert len(rows) == 1
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, kind, symbol)
    assert len(items) == 1
    item = items[0]
    assert item.count(old) == 1, (symbol, old)
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    if kind == "method":
        owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0])
                  if item in owner]
        assert len(owners) == 1
        owner = owners[0]
    else:
        owner = item
    assert source.count(owner) == 1
    target.write_text(source.replace(owner, owner.replace(item, item.replace(old, new, 1), 1), 1))
    errors = validate(fixture)
    assert any(f"{symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/mv/src/capture.rs', 'struct', 'CaptureCleanup', '    undo: Option<DeferredRelease>,', '    undo: Option<ReleaseNotification>,', id='cleanup-retains-original-event-not-new-source'),
    pytest.param('crates/mv/src/capture.rs', 'method', 'CaptureCleanup::new', 'current: Some(current),', 'current: None,', id='cleanup-keeps-current-release'),
    pytest.param('crates/mv/src/capture.rs', 'method', 'CaptureCleanup::new', 'undo: Some(undo),', 'undo: None,', id='cleanup-keeps-undo-release'),
    pytest.param('crates/mv/src/capture.rs', 'method', 'CaptureCleanup::drop', 'drop(self.undo.take());', 'std::mem::forget(self.undo.take());', id='cleanup-signals-remaining-undo'),
    pytest.param('crates/mv/src/cell/capture.rs', 'struct', 'BlockCaptureSlot', '    cleanup: CaptureCleanup,', '    cleanup: ReleaseNotification,', id='cell-slot-holds-original-cleanup'),
    pytest.param('crates/mv/src/cell/capture.rs', 'enum', 'CapturePhase', '        block: Block<', '        block: OtherBlock<', id='cell-attached-original-not-reconstruction'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'Block::capture_slot', 'block: self,', 'block: { drop(self); panic!("rebuilt original") },', id='cell-slot-inert-original-move'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::try_capture', 'assert!(!self.started,', 'assert!(true,', id='cell-capture-is-terminal-after-attempt'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::try_capture', '*retained = Some(admit(block)?);', 'let _ = admit(block)?;', id='cell-admission-remains-in-attached-phase'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::release', 'block.release_writers();', 'let _ = block;', id='cell-release-physical-before-slot-cleanup'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::into_detached', 'self.phase = original;', 'drop(original);', id='cell-failed-transfer-retains-slot-on-unwind'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::drop', 'self.release();', 'let _ = self;', id='cell-default-drop-uses-terminal-release'),
    pytest.param('crates/mv/src/storage/capture.rs', 'struct', 'BlockCaptureSlot', '    cleanup: CaptureCleanup,', '    cleanup: ReleaseNotification,', id='map-slot-holds-original-cleanup'),
    pytest.param('crates/mv/src/storage/capture.rs', 'enum', 'CapturePhase', '        block: Block<', '        block: OtherBlock<', id='map-attached-original-not-reconstruction'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'Block::capture_slot', 'block: self,', 'block: { drop(self); panic!("rebuilt original") },', id='map-slot-inert-original-move'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::try_capture', 'assert!(!self.started,', 'assert!(true,', id='map-capture-is-terminal-after-attempt'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::try_capture', '*retained = Some(admit(block)?);', 'let _ = admit(block)?;', id='map-admission-remains-in-attached-phase'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::release', 'block.release_writers();', 'let _ = block;', id='map-release-physical-before-slot-cleanup'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::into_detached', 'self.phase = original;', 'drop(original);', id='map-failed-transfer-retains-slot-on-unwind'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::drop', 'self.release();', 'let _ = self;', id='map-default-drop-uses-terminal-release'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::try_capture', 'block.writers.as_ref();', '// accept cleanup-only abandonment', id='cell-precheck-rejects-retired-authority'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::try_capture', '*retained = Some(admit(block)?);\n        let next = NextPublication::new();', 'let next = NextPublication::new();\n        *retained = Some(admit(block)?);', id='cell-admission-before-identity-allocation'),
    pytest.param('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::try_capture', 'let (revert, blocks, cleanup) = writers.detach_retaining();', 'let (revert, blocks, cleanup) = writers.detach_retaining();\n        drop(cleanup);\n        let cleanup = CaptureCleanup::default();', id='cell-cleanup-stays-in-caller-until-siblings-release'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::try_capture', 'block.assert_operable();', '// omit current and undo fail-closed flags', id='map-precheck-before-capture-transfer'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::capture_admitted', '*retained = Some(admission);\n        block.assert_operable();', 'block.assert_operable();\n        *retained = Some(admission);', id='prepaid-already-owned-credit-before-fallible-check'),
    pytest.param('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::finish_capture', 'let (blocks, revert, cleanup) = detach_pair_retaining(blocks, revert);', 'let (blocks, revert, cleanup) = detach_pair_retaining(blocks, revert);\n        drop(cleanup);\n        let cleanup = CaptureCleanup::default();', id='map-cleanup-remains-in-caller'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::detach_retaining', 'let (blocks, current_release) = blocks.release_deferred(|writer| writer.detach());', 'drop(undo_release);\n        let (blocks, current_release) = blocks.release_deferred(|writer| writer.detach());\n        let undo_release = panic!("lost deferred undo");', id='cell-both-unlock-before-first-signal'),
    pytest.param('crates/mv/src/storage.rs', 'fn', 'detach_pair_retaining', 'let (revert, undo_release) = revert.release_deferred(|writer| writer.detach());', 'drop(current_release);\n    let (revert, undo_release) = revert.release_deferred(|writer| writer.detach());\n    let current_release = panic!("lost deferred current");', id='map-both-unlock-before-first-signal'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'Block::detach_owned', 'slot.capture_admitted(admission);', 'drop(admission);\n            slot.capture_admitted(panic!("lost prepaid admission"));', id='prepaid-capture-delegates-original-admission'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'Block::try_detach', 'crate::BlockCapture::try_capture(&mut slot, admit)?;', 'let _ = crate::BlockCapture::try_capture(&mut slot, admit);', id='map-standalone-refusal-propagates'),
])
def test_capture_slots_require_original_caller_custody(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [row for row in checker.native_preparation_contract.PREPARATION_OWNER_BINDINGS
            if row[:3] == (path, kind, symbol)]
    assert len(rows) == 1
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, kind, symbol)
    assert len(items) == 1
    item = items[0]
    assert item.count(old) == 1, (symbol, old)
    if kind == "method":
        owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0])
                  if item in owner]
        assert len(owners) == 1
        owner = owners[0]
    else:
        owner = item
    assert source.count(owner) == 1
    target.write_text(source.replace(owner, owner.replace(item, item.replace(old, new, 1), 1), 1))
    errors = validate(fixture)
    assert any(f"{symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,symbol,old,new", [
    pytest.param("STATE_PUBLICATION", "fn begin", "before\n            .checked_add(2)", "before\n            .checked_add(1)", id="notification-generation-no-wrap"),
    pytest.param("STATE_PUBLICATION", "fn begin", "changed: &mut self.changed", "changed: &mut unrelated", id="notification-original-completion"),
    pytest.param("STATE_PUBLICATION", "impl Drop for StateViewGenerationWriteGuard", "*self.changed = true;", "*self.changed = false;", id="notification-mark-completed"),
    pytest.param("STATE_PUBLICATION", "impl Drop for StateViewGenerationWriteGuard", "*self.changed = true;", "*self.changed = true; notification.notify_waiters();", id="notification-no-callback-under-writer"),
    pytest.param("STATE_PUBLICATION", "impl Drop for StateViewPublication", "if self.changed {", "if true {", id="notification-no-spurious-completion"),
    pytest.param("STATE", "fn state_view_publication", "&self.publication_notify", "&other.publication_notify", id="notification-original-state"),
    pytest.param("STATE", "fn install_lane_manifests(", "let _state_write_lock = state_write_release.lock();", "drop(publication_notice); let _state_write_lock = state_write_release.lock();", id="notification-manifest-retains-owner"),
    pytest.param("STATE", "fn commit_inner(", "let mut this = self;", "let mut this = other;", id="notification-direct-original-state"),
    pytest.param("TERMINAL_CARRIER", "fn publish(", "let mut this = self;", "let mut this = other;", id="notification-retained-original-state"),
    pytest.param("TERMINAL_CARRIER", "fn publish(", "drop(generation);", "drop(generation); drop(publication_notice);", id="notification-retained-waits-for-fences"),
])
def test_notification_retains_original_owner_until_physical_release(fixture, owner, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), symbol, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "notification" in error or "generation completion" in error for error in errors), errors
    assert not any("digest" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("owner,symbol,statement,after", [
    pytest.param("STATE", "fn install_lane_manifests(", "let mut publication_notice = self.state_view_publication();", "let _state_write_lock = state_write_release.lock();", id="STATE-fn install_lane_manifests(-let mut publication_notice = self.state_view_publication();-let _state_write_lock = self.state_write_lock.lock();"),
    ("STATE", "fn commit_inner(", "let mut publication_notice = self.state_ref.state_view_publication();", "let mut this = self;"),
    ("TERMINAL_CARRIER", "fn publish(", "let mut publication_notice = self.target.state_view_publication();", "let mut this = self;"),
])
def test_notification_scope_must_precede_original_physical_owner(fixture, owner, symbol, statement, after):
    root, helper, checker, _ = fixture
    path = root / getattr(checker.native_preparation_contract, owner)
    helper.replace_once_after(path, symbol, statement, "")
    helper.replace_once_after(path, symbol, after, after + " " + statement)
    errors = validate(fixture)
    assert any("reorders executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellWriteTxn::commit_slot', 'LinCowCellCommitPhase::Writer(self)', 'LinCowCellCommitPhase::Writer(other)', id='native-original-writer'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellCommitSlot::prepare', 'self.install_active(active);\n        self.validate();', 'self.validate();\n        self.install_active(active);', id='native-reader-installed-before-validation'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellCommitSlot::try_prepare', 'self.install_active(active);\n        self.validate();', 'self.validate();\n        self.install_active(active);', id='native-try-reader-installed-before-validation'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellCommitSlot::validate', 'assert!(Shared::ptr_eq(&prepared.base, &prepared.guard.current));', 'assert!(true);', id='native-current-predecessor'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellCommitSlot::validate', 'assert!(Shared::ptr_eq(&prepared.base, &prepared.active));', 'assert!(true);', id='native-reader-predecessor'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellCommitSlot::validate', 'Shared::get_mut(&mut prepared.work)', 'Shared::get_mut(&mut other.work)', id='native-original-unique-cursor'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellCommitSlot::into_prepared', 'assert!(self.ready, "original preparation must complete");', 'assert!(true, "original preparation must complete");', id='native-rejects-partial-publication'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellCommitSlot::abort_retaining', '(writer, Some(release))', '{ drop(release); (writer, None) }', id='native-reader-release-retained'),
    pytest.param('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellWriteTxn::commit_slot', 'writer: Some(self),', 'writer: Some(other),', id='ebr-original-allocation'),
    pytest.param('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellCommitSlot::prepare', 'assert!(!self.ready, "original EBR writer prepares once");', 'assert!(true, "original EBR writer prepares once");', id='ebr-prepare-once'),
    pytest.param('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellCommitSlot::into_prepared', 'assert!(self.ready, "original preparation must complete");', 'assert!(true, "original preparation must complete");', id='ebr-requires-validation'),
    pytest.param('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapCommitSlot::prepare', 'self.inner.as_ref().assert_operable();', '// accept failed cursor', id='map-failed-cursor-rejected'),
    pytest.param('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapCommitSlot::try_prepare', 'self.inner.try_prepare()', 'self.inner.prepare(); Ok(())', id='map-nonblocking-kernel-preserved'),
    pytest.param('crates/mv/src/publication.rs', 'method', 'CapturedPublication::prepare_current_in', '*slot = Some(PreparedIdentity {', 'let _discarded = Some(PreparedIdentity {', id='identity-guard-installed-in-caller'),
    pytest.param('crates/mv/src/publication.rs', 'method', 'CapturedPublication::prepare_current_in', 'Shared::ptr_eq(&self.owner, &publication.owner)', 'true', id='identity-original-owner'),
    pytest.param('crates/mv/src/publication.rs', 'method', 'CapturedPublication::prepare_current_in', 'Shared::ptr_eq(&self.version, &held.version)', 'true', id='identity-original-predecessor'),
    pytest.param('crates/mv/src/cell/physical.rs', 'method', 'CellStage::new', 'writer.commit_slot()', 'writer.prepare_commit()', id='cell-inert-before-callee-work'),
    pytest.param('crates/mv/src/cell/physical.rs', 'method', 'PreparedCellWriters::prepare_attached', 'self.started = true;', '// caught preparation can retry', id='cell-attempt-is-one-shot'),
    pytest.param('crates/mv/src/cell/physical.rs', 'method', 'PreparedCellWriters::prepare_attached', 'self.complete = true;', 'let _unchecked = ();', id='cell-successful-complete-verdict'),
    pytest.param('crates/mv/src/cell/physical.rs', 'method', 'PreparedCellWriters::release', 'CellStage::release(&mut self.blocks);', '// original current remains held', id='cell-release-every-native-writer'),
    pytest.param('crates/mv/src/cell/physical.rs', 'method', 'PreparedCellWriters::release', 'self.released = true;', 'self.released = false;', id='cell-release-revokes-publication'),
    pytest.param('crates/mv/src/cell/physical.rs', 'method', 'PreparedCellWriters::publish', 'self.complete && !self.released', 'true', id='cell-publish-rejects-partial-or-released'),
    pytest.param('crates/mv/src/cell/physical.rs', 'method', 'PreparedCellWriters::publish', 'let identity = self.identity.take()', 'let _speculative = Vec::<u8>::new();\n        let identity = self.identity.take()', id='cell-publication-no-new-control-owner'),
    pytest.param('crates/mv/src/storage/physical.rs', 'method', 'MapStage::new', 'writer.commit_slot()', 'writer.prepare_commit()', id='map-inert-before-callee-work'),
    pytest.param('crates/mv/src/storage/physical.rs', 'method', 'PreparedStorageWriters::prepare_attached', 'self.started = true;', '// caught preparation can retry', id='map-attempt-is-one-shot'),
    pytest.param('crates/mv/src/storage/physical.rs', 'method', 'PreparedStorageWriters::prepare_attached', 'self.complete = true;', 'let _unchecked = ();', id='map-successful-complete-verdict'),
    pytest.param('crates/mv/src/storage/physical.rs', 'method', 'PreparedStorageWriters::release', 'MapStage::release(&mut self.blocks);', '// original current remains held', id='map-release-every-native-writer'),
    pytest.param('crates/mv/src/storage/physical.rs', 'method', 'PreparedStorageWriters::release', 'self.released = true;', 'self.released = false;', id='map-release-revokes-publication'),
    pytest.param('crates/mv/src/storage/physical.rs', 'method', 'PreparedStorageWriters::publish', 'self.complete && !self.released', 'true', id='map-publish-rejects-partial-or-released'),
    pytest.param('crates/mv/src/storage/physical.rs', 'method', 'PreparedStorageWriters::publish', 'let identity = self.identity.take()', 'let _speculative = Vec::<u8>::new();\n        let identity = self.identity.take()', id='map-publication-no-new-control-owner'),
    pytest.param('crates/mv/src/storage/physical.rs', 'method', 'MapStage::release', '(writer.abort_retaining(), reader)', '(writer.detach(), reader)', id='failed-map-retirement-is-not-journal'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::prepare_publication', 'self.as_ref();\n        let next = NextPublication::new();', 'let next = NextPublication::new();\n        self.as_ref();', id='cell-identity-after-original-phase-check'),
    pytest.param('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::publish_prepared', 'if writers.is_prepared()', 'if true', id='cell-readiness-before-taking-owner'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'StorageWriters::publish_prepared', 'if writers.is_prepared()', 'if true', id='map-readiness-before-taking-owner'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'StorageWriters::publish_prepared', 'let next = next.take().expect("checked original successor identity");', 'let next = NextPublication::new();', id='map-keeps-original-funded-identity'),
    pytest.param('crates/mv/src/cell.rs', 'method', 'Block::commit', 'self.prepare_attached_publication();', '// publish without common preparation', id='cell-ordinary-commit-shares-kernel'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'Block::publish', 'self.prepare_attached_publication();', '// publish without common preparation', id='map-admitted-commit-shares-kernel'),
    pytest.param('crates/mv/src/cell/publication_slot.rs', 'method', 'Block::publication_slot', 'publication_slot(self)', 'publication_slot(&mut self)', id='cell-consuming-publication-authority'),
    pytest.param('crates/mv/src/cell/publication_slot.rs', 'struct', 'BlockPublicationSlot', '    block: Block<', '    pub block: Block<', id='cell-original-block-private-after-transfer'),
    pytest.param('crates/mv/src/cell/publication_slot.rs', 'method', 'BlockPublicationSlot::prepare_publication', 'self.block.prepare_attached_publication();', 'self.block.publish_attached_prepared();', id='cell-slot-prepares-before-publication'),
    pytest.param('crates/mv/src/cell/publication_slot.rs', 'method', 'BlockPublicationSlot::release_writers', 'crate::BlockRetirement::release_writers(&mut self.block);', '// keep child locks during aggregate cleanup', id='cell-slot-uses-original-terminal-release'),
    pytest.param('crates/mv/src/storage/publication_slot.rs', 'method', 'Block::publication_slot', 'publication_slot(self)', 'publication_slot(&mut self)', id='map-consuming-publication-authority'),
    pytest.param('crates/mv/src/storage/publication_slot.rs', 'struct', 'BlockPublicationSlot', '    block: Block<', '    pub block: Block<', id='map-original-block-private-after-transfer'),
    pytest.param('crates/mv/src/storage/publication_slot.rs', 'method', 'BlockPublicationSlot::prepare_publication', 'self.block.prepare_attached_publication();', 'self.block.publish_attached_prepared();', id='map-slot-prepares-before-publication'),
    pytest.param('crates/mv/src/storage/publication_slot.rs', 'method', 'BlockPublicationSlot::release_writers', 'crate::BlockRetirement::release_writers(&mut self.block);', '// keep child locks during aggregate cleanup', id='map-slot-uses-original-terminal-release'),
    pytest.param('crates/mv/src/storage/admitted.rs', 'method', 'Storage::try_from_snapshot_admitted', 'writers.prepare_publication(&predecessor, true);', 'writers.prepare_publication(&predecessor, false);', id='snapshot-publishes-actual-current-image'),
])
def test_attached_publication_requires_original_caller_custody(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [row for row in checker.native_preparation_contract.PREPARATION_OWNER_BINDINGS
            if row[:3] == (path, kind, symbol)]
    assert len(rows) == 1
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, kind, symbol)
    assert len(items) == 1
    item = items[0]
    assert item.count(old) == 1, (symbol, old)
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    if kind == "method":
        owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0])
                  if item in owner]
        assert len(owners) == 1
        owner = owners[0]
    else:
        owner = item
    assert source.count(owner) == 1
    target.write_text(source.replace(owner, owner.replace(item, item.replace(old, new, 1), 1), 1))
    errors = validate(fixture)
    assert any(f"{symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("path,original,borrowed", [
    ("crates/mv/src/cell/publication_slot.rs", "BlockPublicationSlot<'_, V, C>", "Block<'_, V, C>"),
    ("crates/mv/src/storage/publication_slot.rs", "BlockPublicationSlot<'_, K, V, M>", "Block<'_, K, V, M>"),
], ids=["cell-borrowed-callback-cannot-publish", "map-borrowed-callback-cannot-publish"])
def test_attached_publication_rejects_borrowed_execution_authority(fixture, path, original, borrowed):
    root, _, checker, _ = fixture
    target = root / path
    source = target.read_text()
    # Preserve every real slot item; append a second authority-bearing impl on a
    # borrowed executing owner. The guard must reject the extra public surface.
    implementations = checker._rust_impl_items(source, "BlockPublicationSlot")
    selected = [item for item in implementations if "crate::BlockPublication" in item.split("{")[0]]
    assert len(selected) == 1
    extra = selected[0].replace("for " + original, "for " + borrowed)
    assert extra != selected[0]
    target.write_text(source + "\n" + extra + "\n")
    errors = validate(fixture)
    assert any("attached publication widens borrowed execution authority" in error for error in errors), errors
    assert not any("digest" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::new', 'phase: Some(Phase::Original(original))', 'phase: Some(Phase::Original(other))', id='hash-slot-original-journal'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::original', 'Some(Phase::Original(original)) => original', 'Some(Phase::Original(original)) => other', id='hash-slot-original-admission-input'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::try_prepare', '!self.attempted && !self.released', 'true', id='hash-slot-one-shot-attempt'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::try_prepare', 'self.retryable = false;', 'self.retryable = true;', id='hash-slot-panic-not-retry'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::try_prepare', 'self.preflight_release = release;', 'drop(release);', id='hash-slot-preflight-callback-retained'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::try_prepare', 'self.phase = Some(Phase::Acquired(acquired));\n        let Some(Phase::Acquired(acquired))', 'drop(acquired);\n        let Some(Phase::Acquired(acquired))', id='hash-slot-acquired-before-callee'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::refuse', 'self.retryable = true;', 'self.retryable = false;', id='hash-slot-normal-refusal-keeps-original'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::restore', 'mode: self.mode', 'mode: mv::BlockMode::Ordinary', id='hash-slot-preserves-replacement-mode'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::recover_original', 'self.retryable && !self.released', 'true', id='hash-slot-refuses-unwound-recovery'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::recover_original', 'self.reader_release = reader;', 'drop(reader);', id='hash-slot-recovery-retains-native-event'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::take_prepared', 'self.complete && !self.released', 'true', id='hash-slot-only-complete-transfer'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::take_prepared', 'preflight_release: self.preflight_release.take()', 'preflight_release: None', id='hash-slot-published-preflight-custody'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::release_writers', 'writer.abort_retaining()', 'writer.detach()', id='hash-slot-terminal-abandonment-not-journal'),
    pytest.param('crates/iroha_core/src/state/retained_hash_slot.rs', 'method', 'RetainedHashSlot::drop', 'self.release_writers();', '// abandon physical writer', id='hash-slot-drop-releases-physical'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::new', 'fences: Some(fences)', 'fences: None', id='hash-fences-installed-before-prepare'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::prepare_inner', '        self.block_hashes\n            .try_prepare(|_, _| Ok::<_, Infallible>(()))\n            .map_err(|cause| CarrierPhysicalPreparationError::Component {\n                field: "block_hashes",\n                cause,\n            })?;\n', '        // hash preparation removed\n', id='hash-fences-require-complete-preparation'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::release_fences', '_state: state.release_deferred()', '_state: { drop(state); unreachable!() }', id='hash-fences-state-event-retained'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::release_fences', 'queue.map(CarrierQueueRetirement::release_deferred)', 'queue.map(|q| { drop(q); unreachable!() })', id='hash-fences-queue-event-retained'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::release_fences', '_kura: kura.release_deferred()', '_kura: { drop(kura); unreachable!() }', id='hash-fences-kura-event-retained'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::into_prepared', 'self.complete && !self.released', 'true', id='hash-fences-complete-handoff'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::drop', 'self.block_hashes.release_writers();', 'drop(std::mem::replace(&mut self.block_hashes, other));', id='hash-fences-native-before-callback'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellOwned::try_matches_current_retaining', '!Shared::ptr_eq(&self.root, &target.write)', 'false', id='hash-advisory-refuses-foreign-family'),
    pytest.param('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellOwned::try_matches_current_retaining', 'active.release_deferred(drop)', '{ drop(active); unreachable!() }', id='hash-advisory-defers-actual-reader-wake'),
    pytest.param('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapOwned::try_matches_current_retaining', 'self.inner.try_matches_current_retaining(&target.inner)', 'self.inner.try_matches_current_retaining(&other.inner)', id='hash-advisory-map-original-target'),
])
def test_retained_hash_preparation_requires_original_caller_custody(
    fixture, path, kind, symbol, old, new,
):
    """Partial preparation retains the original native owner and outer fences."""
    root, _, checker, _ = fixture
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "retained hash mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    source_path = root / path
    source = source_path.read_text(encoding="utf-8")
    assert source.count(item) == 1
    source_path.write_text(source.replace(item, item.replace(old, new, 1), 1), encoding="utf-8")
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::try_prepare', '!self.attempted && !self.released', 'true', id='cell-one-shot'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::try_prepare', 'self.retryable = false;', 'self.retryable = true;', id='cell-panic-revokes-retry'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::try_prepare', 'let result = self.prepare_inner(admit);', 'self.retryable = true;\n        let result = self.prepare_inner(admit);', id='cell-normal-return-before-retry'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::try_prepare', 'self.complete = result.is_ok();', 'self.complete = true;', id='cell-actual-completion'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::prepare_inner', 'self.cleanup.identities[0] = probe;', 'drop(probe);', id='cell-retain-original-probe'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::prepare_inner', 'checked?;', 'let _ = checked;', id='cell-identity-before-admission'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::prepare_inner', 'self.phase = Phase::Acquiring', 'let _discarded = Phase::Acquiring', id='cell-caller-before-acquisition'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::prepare_inner', 'self.phase = Phase::Prepared', 'let _callee = Phase::Prepared', id='cell-caller-before-native-prepare'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::release_writers', 'self.retryable = false;', 'self.retryable = true;', id='cell-terminal-no-retry'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::release_writers', 'blocks.release(', 'drop(', id='cell-current-physical-release'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::release_writers', 'revert.release(', 'drop(', id='cell-undo-physical-release'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::recover_original', 'self.retryable && !self.released', 'true', id='cell-recovery-authority'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::into_prepared', 'self.complete && !self.released', 'true', id='cell-publication-authority'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'Role::acquire', 'try_map_preserving_release_into(', 'try_map_preserving_release(', id='cell-callee-unwind-original-batch'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::drop', 'self.release_writers();', '// forget physical owners', id='cell-drop-physical-pass'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::try_prepare', '!self.attempted && !self.released', 'true', id='storage-one-shot'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::try_prepare', 'self.retryable = false;', 'self.retryable = true;', id='storage-panic-revokes-retry'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::try_prepare', 'let result = self.prepare_inner(admit);', 'self.retryable = true;\n        let result = self.prepare_inner(admit);', id='storage-normal-return-before-retry'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::try_prepare', 'self.complete = result.is_ok();', 'self.complete = true;', id='storage-actual-completion'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::prepare_inner', 'self.cleanup.identities[0] = probe;', 'drop(probe);', id='storage-retain-original-probe'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::prepare_inner', 'checked?;', 'let _ = checked;', id='storage-identity-before-admission'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::prepare_inner', 'self.phase = Phase::Acquiring', 'let _discarded = Phase::Acquiring', id='storage-caller-before-acquisition'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::prepare_inner', 'self.phase = Phase::Prepared', 'let _callee = Phase::Prepared', id='storage-caller-before-native-prepare'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::release_writers', 'self.retryable = false;', 'self.retryable = true;', id='storage-terminal-no-retry'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::release_writers', 'blocks.release(', 'drop(', id='storage-current-physical-release'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::release_writers', 'revert.release(', 'drop(', id='storage-undo-physical-release'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::recover_original', 'self.retryable && !self.released', 'true', id='storage-recovery-authority'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::into_prepared', 'self.complete && !self.released', 'true', id='storage-publication-authority'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'Role::acquire', 'try_map_preserving_release_into(', 'try_map_preserving_release(', id='storage-callee-unwind-original-batch'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::drop', 'self.release_writers();', '// forget physical owners', id='storage-drop-physical-pass'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'Role::acquire', 'if raw.is_poisoned()', 'if false', id='ebr-native-poison-verdict'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'Role::acquire', '|acquired| acquired.validate()', '|acquired| Ok(acquired)', id='map-native-base-validation'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'Role::release', '|w| w.abort_retaining()', '|w| w.detach()', id='map-failed-abandonment-not-journal'),
    pytest.param('crates/mv/src/storage/admitted.rs', 'method', 'Detached::try_publication_slot', 'if !scope.belongs_to(', 'if scope.belongs_to(', id='prepaid-original-scope'),
    pytest.param('crates/mv/src/storage/admitted.rs', 'method', 'AdmittedDetachedPublicationSlot::try_prepare', 'self.inner.try_prepare(|_, _| Ok(()))', 'Ok(())', id='prepaid-defining-kernel'),
    pytest.param('crates/mv/src/cell.rs', 'method', 'Detached::try_prepare_publication', 'let mut slot = self.publication_slot(target);', 'let mut slot = self.publication_slot(other);', id='ebr-original-target'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'Detached::prepare_publication', 'let mut slot = DetachedPublicationSlotInner::new(self, target);', 'let mut slot = DetachedPublicationSlotInner::new(self, other);', id='map-original-target'),
    pytest.param('crates/mv/src/storage.rs', 'method', 'Detached::prepare_publication', '    fn prepare_publication', '    pub fn prepare_publication', id='generic-kernel-does-not-widen-prepaid-authority'),
])
def test_detached_pair_preparation_requires_original_caller_custody(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [row for row in checker.native_preparation_contract.PREPARATION_OWNER_BINDINGS
            if row[:3] == (path, kind, symbol)]
    assert len(rows) == 1
    target = root / path
    source = target.read_text()
    items = checker._extract_rust_binding_items(source, kind, symbol)
    assert len(items) == 1
    item = items[0]
    assert item.count(old) == 1, (symbol, old)
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    if kind == "method":
        owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0])
                  if item in owner]
        assert len(owners) == 1
        owner = owners[0]
    else:
        owner = item
    assert source.count(owner) == 1
    target.write_text(source.replace(owner, owner.replace(item, item.replace(old, new, 1), 1), 1))
    errors = validate(fixture)
    assert any(f"{symbol} missing executable relation" in error or "generic map kernel widens prepaid scope authority" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error or "must occur exactly once" in error for error in errors), errors

@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::try_prepare', '!self.attempted && !self.released', 'true', id='runtime-one-shot'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::try_prepare', 'self.retryable = false;', 'self.retryable = true;', id='runtime-panic-revokes-retry'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::try_prepare', 'let result = self.prepare_inner(admit);', 'self.retryable = true;\n        let result = self.prepare_inner(admit);', id='runtime-normal-return-before-retry'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::try_prepare', 'self.complete = result.is_ok();', 'self.complete = true;', id='runtime-actual-completion'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::prepare_inner', 'admit(original, self.target)', 'admit(other, self.target)', id='runtime-original-admission-input'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::prepare_inner', 'self.phase = Some(RuntimePublicationPhase::Components(', 'let _callee = Some(RuntimePublicationPhase::Components(', id='runtime-caller-before-child-prepare'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::release_writers', 'self.released = true;', 'if self.released { return; }\n        self.released = true;', id='runtime-partial-recovery-always-physical-pass'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::release_writers', 'components.release_writers();', 'drop(components);', id='runtime-release-all-children'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::recover_original', 'self.retryable && !self.released', 'true', id='runtime-normal-recovery-only'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::recover_original', 'components.recover_original()', '{ drop(components); unreachable!() }', id='runtime-retains-lower-cleanup'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::into_cleanup', 'self.released && self.recovered', 'true', id='runtime-original-recovery-before-cleanup'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::into_prepared', 'self.complete && !self.released', 'true', id='runtime-all-prepared-before-publication'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimePublicationSlot::drop', 'self.release_writers();', '// skip physical children', id='runtime-drop-physical-pass'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimeJournals::try_prepare_publication', 'let mut slot = self.publication_slot(target);', 'let mut slot = self.publication_slot(other);', id='runtime-standalone-original-kernel'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'RuntimeJournals::try_prepare_publication', 'Err((original, error, slot.into_cleanup()))', 'Err((original, error, { drop(slot); unreachable!() }))', id='runtime-standalone-retains-cleanup'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'macro', 'define_runtime_publication_components', '$field.publication_slot(&target.$field)', '$field.publication_slot(&other.$field)', id='runtime-all-original-targets'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'macro', 'define_runtime_publication_components', '$field: self.$field.recover_original(),', '$field: { self.$field.release_writers(); self.$field.recover_original() },', id='runtime-no-terminal-promotion'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'macro', 'define_runtime_publication_components', '$(self.$field.release_writers();)+', '$(drop(&mut self.$field);)+', id='runtime-inventory-wide-release'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'macro', 'define_runtime_publication_components', 'admission: Some(admission)', 'admission: None', id='runtime-capture-admission-retained'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'macro', 'define_runtime_publication_components', 'Some(self.$field.into_cleanup())', '{ drop(self.$field); None }', id='runtime-exact-lower-cleanup-transfer'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::try_prepare', '!self.attempted && !self.released', 'true', id='triggers-one-shot'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::try_prepare', 'self.retryable = false;', 'self.retryable = true;', id='triggers-panic-revokes-retry'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::try_prepare', 'let result = self.prepare_inner(admit);', 'self.retryable = true;\n        let result = self.prepare_inner(admit);', id='triggers-normal-return-before-retry'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::try_prepare', 'self.complete = result.is_ok();', 'self.complete = true;', id='triggers-actual-completion'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::prepare_inner', 'admit(original, self.target)', 'admit(other, self.target)', id='triggers-original-admission-input'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::prepare_inner', 'self.phase = Some(SetPublicationPhase::Components(', 'let _callee = Some(SetPublicationPhase::Components(', id='triggers-caller-before-child-prepare'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::release_writers', 'self.released = true;', 'if self.released { return; }\n        self.released = true;', id='triggers-partial-recovery-always-physical-pass'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::release_writers', 'components.release_writers();', 'drop(components);', id='triggers-release-all-children'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::recover_original', 'self.retryable && !self.released', 'true', id='triggers-normal-recovery-only'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::recover_original', 'components.recover_original()', '{ drop(components); unreachable!() }', id='triggers-retains-lower-cleanup'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::into_cleanup', 'self.released && self.recovered', 'true', id='triggers-original-recovery-before-cleanup'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::into_prepared', 'self.complete && !self.released', 'true', id='triggers-all-prepared-before-publication'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSetPublicationSlot::drop', 'self.release_writers();', '// skip physical children', id='triggers-drop-physical-pass'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSet::try_prepare_publication', 'let mut slot = self.publication_slot(target);', 'let mut slot = self.publication_slot(other);', id='triggers-standalone-original-kernel'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'DetachedSet::try_prepare_publication', 'Err((original, error, slot.into_cleanup()))', 'Err((original, error, { drop(slot); unreachable!() }))', id='triggers-standalone-retains-cleanup'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'macro', 'define_set_publication_components', '$field.publication_slot(&target.$field)', '$field.publication_slot(&other.$field)', id='triggers-all-original-targets'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'macro', 'define_set_publication_components', '$field: self.$field.recover_original(),', '$field: { self.$field.release_writers(); self.$field.recover_original() },', id='triggers-no-terminal-promotion'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'macro', 'define_set_publication_components', '$(self.$field.release_writers();)+', '$(drop(&mut self.$field);)+', id='triggers-inventory-wide-release'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'macro', 'define_set_publication_components', 'admission: Some(admission)', 'admission: None', id='triggers-capture-admission-retained'),
    pytest.param('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'macro', 'define_set_publication_components', 'Some(self.$field.into_cleanup())', '{ drop(self.$field); None }', id='triggers-exact-lower-cleanup-transfer'),
    pytest.param('crates/mv/src/cell/detached_publication.rs', 'method', 'DetachedPublicationSlot::into_cleanup', 'self.retryable && self.released && matches!(self.phase, Phase::Empty)', 'self.released && matches!(self.phase, Phase::Empty)', id='cell-cleanup-rejects-unwound-phase'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlotInner::into_cleanup', 'self.retryable && self.released && matches!(self.phase, Phase::Empty)', 'self.released && matches!(self.phase, Phase::Empty)', id='storage-cleanup-rejects-unwound-phase'),
    pytest.param('crates/mv/src/storage/detached_publication.rs', 'method', 'DetachedPublicationSlot::into_cleanup', 'self.inner.into_cleanup()', '{ drop(self.inner); unreachable!() }', id='storage-cleanup-original-engine'),
])
def test_group_preparation_requires_original_caller_custody(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    rows = [row for row in checker.native_preparation_contract.PREPARATION_OWNER_BINDINGS
            if row[:3] == (path, kind, symbol)]
    assert len(rows) == 1
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "group mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    source_path = root / path
    source = source_path.read_text()
    assert source.count(item) == 1
    source_path.write_text(source.replace(item, item.replace(old, new, 1), 1))
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("path,old,new", [
    ("crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs", "lane_consensus_contexts: (LaneConsensusContextsV1),", "lane_consensus_contexts: (Vec<PeerId>),"),
    ("crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs", "contracts: (HashOf<IvmBytecode>, IvmBytecodeEntry),", "contracts: (TriggerId, ()),"),
], ids=["runtime-exact-four-component-inventory", "trigger-exact-ten-component-inventory"])
def test_group_preparation_requires_concrete_original_inventory(fixture, path, old, new):
    root, _, _, _ = fixture
    source_path = root / path
    source = source_path.read_text()
    assert source.count(old) == 1
    source_path.write_text(source.replace(old, new, 1))
    errors = validate(fixture)
    assert any("original component inventory" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors

@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::new', 'world: Some(world.publication_slot(', 'world: Some(other.publication_slot(', id='carrier-original-world-slot'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::drop', 'world.release_writers();', 'drop(world);', id='carrier-releases-world-before-fences'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::new', 'runtime: Some(runtime.publication_slot(', 'runtime: Some(other.publication_slot(', id='carrier-original-runtime-slot'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::drop', 'runtime.release_writers();', 'drop(runtime);', id='carrier-releases-runtime-before-fences'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::new', 'transactions: Some(transactions.publication_slot(', 'transactions: Some(other.publication_slot(', id='carrier-original-transactions-slot'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::drop', 'transactions.release_writers();', 'drop(transactions);', id='carrier-releases-transactions-before-fences'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::try_prepare', '!self.attempted && !self.released', 'true', id='carrier-prepares-once'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::try_prepare', 'self.retryable = false;', 'self.retryable = true;', id='carrier-unwind-no-retry'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::try_prepare', 'let result = self.prepare_inner();', 'self.retryable = true; let result = self.prepare_inner();', id='carrier-normal-return-before-retry'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::try_prepare', 'self.complete = result.is_ok();', 'self.complete = true;', id='carrier-complete-verdict'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::recover_original', 'self.retryable && !self.released', 'true', id='carrier-original-recovery-only'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::recover_original', 'self.release_fences();', 'drop(self.runtime.take()); self.release_fences();', id='carrier-recovery-retains-components-through-fences'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::drop', 'self.block_hashes.release_writers();', 'self.release_fences(); self.block_hashes.release_writers();', id='carrier-all-physical-before-fences'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::prepare_inner', '        self.transactions\n            .as_mut()\n            .expect("original membership slot")\n            .try_prepare(|_, _| Ok::<_, Infallible>(()))\n            .map_err(|cause| CarrierPhysicalPreparationError::Component {\n                field: "transactions",\n                cause,\n            })?;', '// actual transactions preparation skipped', id='carrier-requires-transactions-preparation'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::prepare_inner', '        self.runtime\n            .as_mut()\n            .expect("original runtime slot")\n            .try_prepare(|_, _| Ok::<_, Infallible>(()))\n            .map_err(CarrierPhysicalPreparationError::Runtime)?;', '// actual runtime preparation skipped', id='carrier-requires-runtime-preparation'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::prepare_inner', '        self.world\n            .as_mut()\n            .expect("original World slot")\n            .try_prepare(|_, _| Ok::<_, Infallible>(()))\n            .map_err(CarrierPhysicalPreparationError::World)?;', '// actual world preparation skipped', id='carrier-requires-world-preparation'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'DetachedWorld::publication_slot', 'phase: Some(Phase::Original(self))', 'phase: Some(Phase::Original(other))', id='world-slot-original-owner'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::try_prepare', '!self.attempted && !self.released', 'true', id='world-slot-once'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::try_prepare', 'self.retryable = false;', 'self.retryable = true;', id='world-callee-unwind-no-retry'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::try_prepare', 'admit(self.original(), self.target)', 'admit(other, self.target)', id='world-original-admission'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::try_prepare', 'self.installation = Some(installation);', 'drop(installation);', id='world-installation-retained'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::try_prepare', 'self.phase = Some(Phase::Fields(', 'let _callee = Some(Phase::Fields(', id='world-phase-installed-before-fields'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::try_prepare', 'fields\n                .fields\n                .push(original.publication_slot(self.target, self.scope));', 'let mut field = original.publication_slot(self.target, self.scope); field.try_prepare().unwrap(); fields.fields.push(field);', id='world-all-inert-slots-before-callee'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::try_prepare', 'fields.retry.reverse();', '// reverse omitted', id='world-exact-original-field-order'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::try_prepare', 'return Err(WorldPublicationError::Field(error));', 'let _ignored = error;', id='world-component-refusal-propagates'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::recover_original', 'self.retryable && !self.released', 'true', id='world-recovery-no-unwound-authority'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::recover_original', 'std::mem::take(&mut fields.retry)', 'Vec::new()', id='world-original-retry-allocation'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::release_writers', 'self.released = true;', 'if self.released { return; } self.released = true;', id='world-partial-recovery-full-physical-pass'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::release_writers', 'fields.fields.release_all();', 'drop(&mut fields.fields);', id='world-releases-all-original-fields'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::into_prepared', 'self.complete && !self.released', 'true', id='world-rejects-partial-publication'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::into_cleanup', 'fields.admission.is_none()', 'true', id='world-recovery-before-cleanup'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::drop', 'self.release_writers();', '// physical release lost', id='world-drop-original-slots'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::try_prepare', 'self.phase = FieldPhase::Prepared(M::into_prepared(slot));', 'self.phase = FieldPhase::Prepared(M::into_prepared(other));', id='storage-same-original-prepared-field'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::release', 'M::release_writers(slot);', 'drop(slot);', id='storage-partial-field-physical-pass'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::release', 'self.aborted = Some(retirement);', 'drop(retirement);', id='storage-prepared-field-retains-abort-cleanup'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::release_for_recovery', 'M::recover_original(slot)', '{ M::release_writers(slot); M::recover_original(slot) }', id='storage-normal-recovery-before-terminal-release'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::publish', 'self.published = Some(M::publish(journal));', 'drop(M::publish(journal));', id='storage-published-original-retirement-retained'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::try_prepare', 'self.phase = FieldPhase::Prepared(slot.into_prepared());', 'self.phase = FieldPhase::Prepared(other.into_prepared());', id='cell-same-original-prepared-field'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::release', 'slot.release_writers();', 'drop(slot);', id='cell-partial-field-physical-pass'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::release', 'self.aborted = Some(retirement);', 'drop(retirement);', id='cell-prepared-field-retains-abort-cleanup'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::release_for_recovery', 'slot.recover_original()', '{ slot.release_writers(); slot.recover_original() }', id='cell-normal-recovery-before-terminal-release'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::publish', 'self.published = Some(journal.publish());', 'drop(journal.publish());', id='cell-published-original-retirement-retained'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::try_prepare', 'self.phase = FieldPhase::Prepared(slot.into_prepared());', 'self.phase = FieldPhase::Prepared(other.into_prepared());', id='triggers-same-original-prepared-field'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::release', 'slot.release_writers();', 'drop(slot);', id='triggers-partial-field-physical-pass'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::release', 'self.aborted = Some(retirement);', 'drop(retirement);', id='triggers-prepared-field-retains-abort-cleanup'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::release_for_recovery', 'slot.recover_original()', '{ slot.release_writers(); slot.recover_original() }', id='triggers-normal-recovery-before-terminal-release'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::publish', 'self.published = Some(journal.publish());', 'drop(journal.publish());', id='triggers-published-original-retirement-retained'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'fn', 'storage_slot', '(original.target)(world)', '(original.target)(other)', id='storage_slot-original-target-accessor'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'fn', 'cell_slot', '(original.target)(world)', '(original.target)(other)', id='cell_slot-original-target-accessor'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'fn', 'triggers_slot', '(original.target)(world)', '(original.target)(other)', id='triggers_slot-original-target-accessor'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedWorldFields::release_all', 'field.release();', 'drop(field);', id='world-field-container-releases-every-child'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedWorldFields::drop', 'self.release_all();', '// no cleanup pass', id='world-field-container-drop-physical-pass'),
])
def test_complete_preparation_retains_every_original_through_callee_unwind(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "complete preparation mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    source_path = root / path
    source = source_path.read_text()
    source_path.write_text(
        _mutate_complete_preparation_item(checker, source, kind, symbol, item, old, new)
    )
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error or "must occur exactly once" in error for error in errors), errors

@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedWorldFields::recover_all', 'field.release_for_recovery();', 'field.release();', id='recovery-does-not-revoke-retry'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedWorld::abort', 'fields.recover_all();', '// lost normal physical pass', id='prepared-world-all-physical-before-transfer'),
    pytest.param('crates/iroha_core/src/state/world_preparation.rs', 'method', 'WorldPublicationSlot::recover_original', 'fields.fields.recover_all();', '// lost normal physical pass', id='partial-world-all-physical-before-transfer'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::release_for_recovery', '!self.released', 'true', id='storage-terminal-release-is-not-recovery'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::release_for_recovery', 'self.aborted = Some(retirement);', 'drop(retirement);', id='storage-recovery-retains-notification'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::release_for_recovery', 'self.normal_recovery = true;', 'self.released = true;', id='storage-normal-recovery-distinct-phase'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedStorage::abort', 'self.release_for_recovery();', '// skipped physical recovery', id='storage-abort-delegates-original-recovery'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::release_for_recovery', '!self.released', 'true', id='cell-terminal-release-is-not-recovery'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::release_for_recovery', 'self.aborted = Some(retirement);', 'drop(retirement);', id='cell-recovery-retains-notification'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::release_for_recovery', 'self.normal_recovery = true;', 'self.released = true;', id='cell-normal-recovery-distinct-phase'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedCell::abort', 'self.release_for_recovery();', '// skipped physical recovery', id='cell-abort-delegates-original-recovery'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::release_for_recovery', '!self.released', 'true', id='triggers-terminal-release-is-not-recovery'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::release_for_recovery', 'self.aborted = Some(retirement);', 'drop(retirement);', id='triggers-recovery-retains-notification'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::release_for_recovery', 'self.normal_recovery = true;', 'self.released = true;', id='triggers-normal-recovery-distinct-phase'),
    pytest.param('crates/iroha_core/src/state/world_publication.rs', 'method', 'PreparedTriggers::abort', 'self.release_for_recovery();', '// skipped physical recovery', id='triggers-abort-delegates-original-recovery'),
])
def test_world_normal_recovery_unlocks_all_before_original_box_transfer(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "normal recovery mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    source_path = root / path
    source = source_path.read_text()
    source_path.write_text(
        _mutate_complete_preparation_item(checker, source, kind, symbol, item, old, new)
    )
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error or "must occur exactly once" in error for error in errors), errors


def _mutate_complete_preparation_item(checker, source, kind, symbol, item, old, new):
    """Mutate one reviewed owner even when sibling methods have identical bodies."""
    replacement = item.replace(old, new, 1)
    assert item.count(old) == 1 and replacement != item
    if kind == "method":
        owner, _ = symbol.split("::", 1)
        implementations = [
            scope for scope in checker._rust_impl_items(source, owner)
            if scope.count(item) == 1
        ]
        assert len(implementations) == 1
        scope = implementations[0]
        assert source.count(scope) == 1
        changed = source.replace(scope, scope.replace(item, replacement, 1), 1)
    else:
        assert source.count(item) == 1
        changed = source.replace(item, replacement, 1)
    assert checker._extract_rust_binding_items(changed, kind, symbol) == (replacement,)
    return changed

@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::read', 'self.released.guard(', 'ReleaseNotification::default().guard(', id='read-uses-original-source'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::read', 'self.released.guard(', 'self.released.poisoning_guard(', id='read-preserves-nonpoison-semantics'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::write', 'self.released.guard(', 'ReleaseNotification::default().guard(', id='write-uses-original-source'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::write', 'self.released.guard(', 'self.released.poisoning_guard(', id='write-preserves-nonpoison-semantics'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::try_read', 'self.released.guard(', 'ReleaseNotification::default().guard(', id='try_read-uses-original-source'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::try_read', 'self.released.guard(', 'self.released.poisoning_guard(', id='try_read-preserves-nonpoison-semantics'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::try_write', 'self.released.guard(', 'ReleaseNotification::default().guard(', id='try_write-uses-original-source'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::try_write', 'self.released.guard(', 'self.released.poisoning_guard(', id='try_write-preserves-nonpoison-semantics'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLock::try_write_or_wait', 'let wait = self.released.observe();\n        self.try_write().ok_or(wait)', 'let result = self.try_write();\n        let wait = self.released.observe();\n        result.ok_or(wait)', id='observe-before-actual-probe'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLockReadGuard::release_deferred', 'self.inner.release_deferred(drop).1', '{ drop(self.inner); ReleaseNotification::default().deferred_batch() }', id='read-retains-real-release'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLockReadGuard::try_release_into', '.try_release_into(batch, drop)', '.try_release_into(&mut ReleaseNotification::default().deferred_batch(), drop)', id='read-coalesces-only-original-source'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLockReadGuard::try_release_into', '.map_err(|inner| Self { inner })', '.map_err(|inner| { drop(inner); unreachable!() })', id='read-foreign-batch-keeps-held-guard'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLockWriteGuard::release_deferred', 'self.inner.release_deferred(drop).1', '{ drop(self.inner); ReleaseNotification::default().deferred_batch() }', id='write-retains-real-release'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLockWriteGuard::try_release_into', '.try_release_into(batch, drop)', '.try_release_into(&mut ReleaseNotification::default().deferred_batch(), drop)', id='write-coalesces-only-original-source'),
    pytest.param('crates/iroha_core/src/publication_rwlock.rs', 'method', 'PublicationRwLockWriteGuard::try_release_into', '.map_err(|inner| Self { inner })', '.map_err(|inner| { drop(inner); unreachable!() })', id='write-foreign-batch-keeps-held-guard'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            latest_block_header: Option<BlockHeader>,\n', '', id='latest_block_header-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            merge_admission: MergeAdmissionState,\n', '', id='merge_admission-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            da_commitments: DaCommitmentStore,\n', '', id='da_commitments-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            da_confidential_compute: ConfidentialComputeStore,\n', '', id='da_confidential_compute-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            da_receipt_cursors: DaReceiptCursorIndex,\n', '', id='da_receipt_cursors-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            da_shard_cursors: DaShardCursorIndex,\n', '', id='da_shard_cursors-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            da_pin_intents: DaPinStore,\n', '', id='da_pin_intents-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            lane_relays: LaneRelayStore,\n', '', id='lane_relays-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            lane_manifests: LaneManifestRegistryHandle,\n', '', id='lane_manifests-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            lane_privacy_registry: LanePrivacyRegistryHandle,\n', '', id='lane_privacy_registry-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'effect_indexes', '            da_indexes_hydrated: Option<Result<(), DaIndexHydrationError>>,\n', '', id='da_indexes_hydrated-cannot-disappear-from-physical-inventory'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'self.$field = Some(self.target.$field.try_write_or_wait()', 'self.$field = Some(other.$field.try_write_or_wait()', id='actual-original-target'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', '.map_err(|wait| (stringify!($field), wait))?);)*', '.map_err(|_wait| (stringify!($field), other.observe()))?);)*', id='actual-blocker-wait'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'self.sccp_registry_cache = Some(self.target.sccp_registry_cache.try_lock_or_wait()', 'self.sccp_registry_cache = Some(other.sccp_registry_cache.try_lock_or_wait()', id='twelfth-original-sccp'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'self.complete = true;', 'self.complete = false;', id='actual-complete-verdict'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'retired: [$(target.$field.deferred_releases(),)* target.sccp_registry_cache.deferred_releases()]', 'retired: [$(other.$field.deferred_releases(),)* other.sccp_registry_cache.deferred_releases()]', id='all-original-cleanup-batches'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'while let Err((field, _wait)) = self.prepare_inner() {\n                    self.release_writers();', 'while let Err((field, _wait)) = self.prepare_inner() {', id='synchronous-wait-releases-whole-prefix'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'self.target.$field.write().try_release_into(&mut self.retired[index])', 'self.target.$field.write().try_release_into(&mut self.retired[0])', id='synchronous-exact-blocker-release'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'assert!(guard.try_release_into(retired).is_ok(), "original effect release source");', 'drop(guard);', id='physical-release-retains-every-index-event'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'assert!(guard.try_release_into(slots.next().expect("SCCP release slot")).is_ok(), "original SCCP release source");', 'drop(guard);', id='physical-release-retains-sccp-event'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'guard: Some(self.target.merge_admission.read()),', 'guard: Some(other.merge_admission.read()),', id='short-merge-read-original-target'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'releases: &mut self.retired[index],', 'releases: &mut self.retired[0],', id='short-merge-read-original-batch'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'macro', 'define_indexes', 'self.retired_manifests = Some(std::mem::replace(', 'let _discarded = Some(std::mem::replace(', id='retired-registry-through-fences'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'method', 'DeferredIndexRead::drop', 'guard.try_release_into(self.releases)', '{ drop(guard); Ok::<_, ()>(()) }', id='short-read-unwind-deferred'),
    pytest.param('crates/iroha_core/src/state/effect_publication.rs', 'method', 'EffectLockScope::drop', 'self.0.release_writers();', '// physical pass omitted', id='scope-unwind-before-sibling-cleanup'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::prepare_inner', 'self.effect_locks', 'other.effect_locks', id='carrier-exact-effects-slot'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::prepare_inner', '.map_err(|(field, wait)| CarrierPhysicalPreparationError::Fence { field, wait })?;', '.map_err(|(field, wait)| CarrierPhysicalPreparationError::Fence { field, wait });', id='carrier-propagates-effects-refusal'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::drop', 'indexes.release_writers();', '// lost index physical pass', id='carrier-indexes-before-fences'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/participant_preparation.rs', 'method', 'CarrierPreparation::into_prepared', 'effect_locks: self.effect_locks.take().expect("prepared effect locks"),', 'effect_locks: other.effect_locks.take().expect("prepared effect locks"),', id='complete-carrier-retains-original-effects'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/physical_publication.rs', 'method', 'AcquiredCarrierParticipants::abort', 'let mut effect_cleanup;', '', id='abort-cleanup-declared-before-originals'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/physical_publication.rs', 'method', 'AcquiredCarrierParticipants::abort', 'let mut effect_locks = effect_cleanup.physical_scope();', 'let mut effect_locks = &mut effect_cleanup;', id='abort-unwind-physical-scope'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/physical_publication.rs', 'method', 'AcquiredCarrierParticipants::abort', 'effect_locks.release_writers();', '// lost physical pass', id='abort-indexes-before-outer-fences'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/publication.rs', 'method', 'PhysicallyPreparedCarrier::publish', 'let mut effect_cleanup;', '', id='publish-cleanup-declared-before-originals'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/publication.rs', 'method', 'PhysicallyPreparedCarrier::publish', 'let mut effect_locks = effect_cleanup.physical_scope();', 'let mut effect_locks = &mut effect_cleanup;', id='publish-unwind-physical-scope'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/publication.rs', 'method', 'PhysicallyPreparedCarrier::publish', 'effect_locks.release_writers();', '// lost physical pass', id='publish-indexes-before-outer-fences'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'StateBlock::commit_inner', 'let mut effect_locks = effect_cleanup.physical_scope();', 'let mut effect_locks = &mut effect_cleanup;', id='direct-original-state-before-effect-scope'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'StateBlock::commit_inner', 'effect_locks.prepare_blocking();', '// skipped original indexes', id='direct-effects-before-first-visibility'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'StateBlock::commit_inner', 'effect_locks.with_merge_admission(|admission| admission.validate_next(entry))', 'state_ref.merge_admission.read().validate_next(entry)', id='direct-short-read-no-early-wake'),
    pytest.param('crates/iroha_core/src/state/carrier_da_effects.rs', 'method', 'DaCommitmentPostPublication::capture_snapshot', 'cursors,', '&state.da_shard_cursors.read(),', id='da-snapshot-through-original-writer'),
    pytest.param('crates/iroha_core/src/state/carrier_da_effects.rs', 'method', 'DaCommitmentPostPublication::capture_snapshot', 'self.captured = true;', 'self.captured = false;', id='da-captured-owner-phase'),
    pytest.param('crates/iroha_core/src/state/carrier_da_effects.rs', 'method', 'DaCommitmentPostPublication::publish', 'self.snapshot', '{ let _reader = state.da_shard_cursors.read(); self.snapshot }', id='da-post-never-reopens-index'),
    pytest.param('crates/iroha_core/src/state/carrier_da_effects.rs', 'method', 'DaCommitmentPostPublication::publish', 'state.da_shard_cursor_persistor.schedule(snapshot);', 'snapshot.persist().unwrap();', id='da-preserves-async-scheduling'),
    pytest.param('crates/iroha_core/src/state/carrier_lifecycle_effects.rs', 'method', 'LaneLifecyclePostPublication::capture_snapshot', 'cursors,', '&state.da_shard_cursors.read(),', id='lifecycle-snapshot-through-original-writer'),
    pytest.param('crates/iroha_core/src/state/carrier_lifecycle_effects.rs', 'method', 'LaneLifecyclePostPublication::capture_snapshot', 'self.captured = true;', 'self.captured = false;', id='lifecycle-captured-owner-phase'),
    pytest.param('crates/iroha_core/src/state/carrier_lifecycle_effects.rs', 'method', 'LaneLifecyclePostPublication::publish', 'self.snapshot', '{ let _reader = state.da_shard_cursors.read(); self.snapshot }', id='lifecycle-post-never-reopens-index'),
    pytest.param('crates/iroha_core/src/state/carrier_lifecycle_effects.rs', 'method', 'LaneLifecyclePostPublication::publish', 'snapshot.persist()', 'state.persist_da_shard_cursor_journal_with_config(&self.lane_config)', id='lifecycle-sync-persist-original-snapshot'),
])
def test_effect_publication_retains_original_indexes_until_outer_release(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "effect publication mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    source_path = root / path
    source = source_path.read_text()
    changed = _mutate_complete_preparation_item(checker, source, kind, symbol, item, old, new)
    source_path.write_text(changed)
    errors = validate(fixture)
    assert any("executable relation" in error or "reopens a published index" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error or "must occur exactly once" in error for error in errors), errors

@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'method', 'PublicationRwLock::defer_notifications', 'lock: self,', 'lock: other,', id='deferred-original-lock'),
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'method', 'PublicationRwLock::defer_notifications', 'releases: self.deferred_releases(),', 'releases: other.deferred_releases(),', id='deferred-original-notification'),
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'method', 'DeferredPublicationRwLock::read', 'guard: Some(self.lock.read()),', 'guard: Some(other.lock.read()),', id='read-original-physical-guard'),
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'method', 'DeferredPublicationRwLock::read', 'releases: &mut self.releases,', 'releases: &mut other.releases,', id='read-original-release-batch'),
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'method', 'DeferredPublicationRwLock::write', 'guard: Some(self.lock.write()),', 'guard: Some(other.lock.write()),', id='write-original-physical-guard'),
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'method', 'DeferredPublicationRwLock::write', 'releases: &mut self.releases,', 'releases: &mut other.releases,', id='write-original-release-batch'),
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'macro', 'deferred_guard', 'guard.try_release_into(self.releases).is_ok()', '{ drop(guard); true }', id='guard-unlock-without-early-notification'),
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'macro', 'deferred_guard', 'self.guard.take()', 'None', id='guard-takes-actual-original'),
    pytest.param('crates/iroha_core/src/publication_rwlock/deferred.rs', 'macro', 'deferred_guard', "releases: &'scope mut DeferredReleaseBatch,", 'releases: DeferredReleaseBatch,', id='guard-borrows-enclosing-batch'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'DaHydrationReleases::new', 'commitments: state.da_commitments.defer_notifications(),', 'commitments: other.da_commitments.defer_notifications(),', id='hydration-commitments-original-source'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'DaHydrationReleases::new', 'confidential_compute: state.da_confidential_compute.defer_notifications(),', 'confidential_compute: other.da_confidential_compute.defer_notifications(),', id='hydration-confidential_compute-original-source'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'DaHydrationReleases::new', 'receipt_cursors: state.da_receipt_cursors.defer_notifications(),', 'receipt_cursors: other.da_receipt_cursors.defer_notifications(),', id='hydration-receipt_cursors-original-source'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'DaHydrationReleases::new', 'shard_cursors: state.da_shard_cursors.defer_notifications(),', 'shard_cursors: other.da_shard_cursors.defer_notifications(),', id='hydration-shard_cursors-original-source'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'DaHydrationReleases::new', 'pin_intents: state.da_pin_intents.defer_notifications(),', 'pin_intents: other.da_pin_intents.defer_notifications(),', id='hydration-pin_intents-original-source'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'DaHydrationReleases::new', 'hydrated: state.da_indexes_hydrated.defer_notifications(),', 'hydrated: other.da_indexes_hydrated.defer_notifications(),', id='hydration-hydrated-original-source'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::publish_hydrated_da_indexes', 'let mut published_commitments = releases.commitments.write();', 'let mut published_commitments = self.da_commitments.write();', id='publish-commitments-retains-notification'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::publish_hydrated_da_indexes', 'let mut published_confidential_compute = releases.confidential_compute.write();', 'let mut published_confidential_compute = self.da_confidential_compute.write();', id='publish-confidential_compute-retains-notification'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::publish_hydrated_da_indexes', 'let mut published_receipt_cursors = releases.receipt_cursors.write();', 'let mut published_receipt_cursors = self.da_receipt_cursors.write();', id='publish-receipt_cursors-retains-notification'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::publish_hydrated_da_indexes', 'let mut published_shard_cursors = releases.shard_cursors.write();', 'let mut published_shard_cursors = self.da_shard_cursors.write();', id='publish-shard_cursors-retains-notification'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::publish_hydrated_da_indexes', 'let mut published_pin_intents = releases.pin_intents.write();', 'let mut published_pin_intents = self.da_pin_intents.write();', id='publish-pin_intents-retains-notification'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::publish_hydrated_da_indexes', 'let mut published_pin_intents = releases.pin_intents.write();\n        *published_commitments = commitments;', '*published_commitments = commitments;\n        let mut published_pin_intents = releases.pin_intents.write();', id='acquire-all-five-before-first-mutation'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::persist_hydrated_da_shard_cursor_journal', '&releases.shard_cursors.read()', '&self.da_shard_cursors.read()', id='persist-retains-original-reader-release'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::persist_hydrated_da_shard_cursor_journal', 'snapshot.persist()', 'self.persist_da_shard_cursor_journal_with_config(&lane_config)', id='persist-owned-snapshot-without-reopening-index'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::ensure_da_indexes_hydrated_with_journal_publication', 'let mut releases = DaHydrationReleases::new(self);\n        let mut write_fence = self.state_write_lock.defer_notifications();\n        let _hydration_guard = self.da_index_hydration_fence.lock();', 'let mut write_fence = self.state_write_lock.defer_notifications();\n        let _hydration_guard = self.da_index_hydration_fence.lock();\n        let mut releases = DaHydrationReleases::new(self);', id='ensure-cleanup-before-both-fences'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::ensure_da_indexes_hydrated_with_journal_publication', 'let _state_write_guard = write_fence.lock();', 'let _state_write_guard = self.state_write_lock.lock();', id='ensure-original-state-fence-notice-retained'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::ensure_da_indexes_hydrated_with_journal_publication', 'self.publish_hydrated_da_indexes(hydrated, &mut releases);', 'self.publish_hydrated_da_indexes(hydrated, &mut DaHydrationReleases::new(self));', id='ensure-publish-uses-enclosing-owner'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::ensure_da_indexes_hydrated_with_journal_publication', 'self.persist_hydrated_da_shard_cursor_journal(&mut releases);', 'self.persist_hydrated_da_shard_cursor_journal(&mut DaHydrationReleases::new(self));', id='ensure-persist-uses-enclosing-owner'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::ensure_da_indexes_hydrated_with_journal_publication', '*releases.hydrated.write() = Some(result);', '*self.da_indexes_hydrated.write() = Some(result);', id='ensure-final-status-release-retained'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', 'method', 'AcquiredRuntimeBlock::rewind_da_indexes_to_height', 'let releases = fields\n            .da_rewind_releases\n            .get_or_insert_with(|| da_hydration::DaRewindReleases::new(self.target));', 'let mut local = da_hydration::DaRewindReleases::new(self.target); let releases = &mut local;', id='rewind-cleanup-before-both-fences'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::rewind_da_indexes_to_height_with_releases', 'let _state_write_guard = write_fence.lock();', 'let _state_write_guard = self.state_write_lock.lock();', id='rewind-original-state-fence-notice-retained'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::rewind_da_indexes_to_height_with_releases', 'self.publish_hydrated_da_indexes(hydrated, releases);', 'self.publish_hydrated_da_indexes(hydrated, &mut DaHydrationReleases::new(self));', id='rewind-publish-uses-enclosing-owner'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::rewind_da_indexes_to_height_with_releases', 'self.persist_hydrated_da_shard_cursor_journal(releases);', 'self.persist_hydrated_da_shard_cursor_journal(&mut DaHydrationReleases::new(self));', id='rewind-persist-uses-enclosing-owner'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::rewind_da_indexes_to_height_with_releases', '*releases.hydrated.write() = Some(result);', '*self.da_indexes_hydrated.write() = Some(result);', id='rewind-final-status-release-retained'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::ensure_da_indexes_hydrated_with_journal_publication', 'if let Some(result) = releases.hydrated.read().as_ref()', 'if let Some(result) = self.da_indexes_hydrated.read().as_ref()', id='second-cache-check-retains-release'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::ensure_da_indexes_hydrated_with_journal_publication', 'if persist_journal {', 'if true {', id='isolated-replay-does-not-persist'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::rewind_da_indexes_to_height_with_releases', '*releases.hydrated.write() = None;', '*self.da_indexes_hydrated.write() = None;', id='rewind-reset-status-release-retained'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::rewind_da_indexes_to_height_with_releases', '.build_da_indexes_from_kura(Some(target_height))', '.build_da_indexes_from_kura(None)', id='rewind-authenticates-exact-prefix'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'LaneRelayStore::next_relays_with_merge_material', 'next.into_values().cloned().collect()', 'next.into_values().collect()', id='relay-snapshot-owns-original-envelope'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'LaneRelayStore::next_relays_with_merge_material', 'if envelope.block_height == expected_height {', 'if envelope.block_height >= expected_height {', id='relay-preserves-contiguous-height'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'LaneRelayStore::next_relays_with_merge_material', 'if !envelope.has_merge_admission_material() {', 'if false {', id='relay-preserves-admission-material'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::merge_entry_candidates_from_lane_relays_with_view', 'let relay_candidates = self\n            .lane_relays\n            .read()\n            .next_relays_with_merge_material(previous_snapshots);', 'let relay_guard = self.lane_relays.read();\n        let relay_candidates = relay_guard.next_relays_with_merge_material(previous_snapshots);', id='relay-reader-drops-before-validation'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::merge_entry_candidates_from_lane_relays_with_view', '.verify_lane_relay_fastpq_record(&latest_admissible)', '.verify_lane_relay_fastpq_record(&other)', id='relay-validates-original-snapshot'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::merge_entry_candidates_from_lane_relays_with_view', 'if !lifecycle.lane_route_and_incarnation_matches(', 'if lifecycle.lane_route_and_incarnation_matches(', id='relay-preserves-route-incarnation-check'),

])
def test_hydration_relay_retains_original_release_custody(fixture, path, kind, symbol, old, new):
    root, _, checker, _ = fixture
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "hydration/relay mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    source_path = root / path
    source_path.write_text(_mutate_complete_preparation_item(
        checker, source_path.read_text(), kind, symbol, item, old, new,
    ))
    errors = validate(fixture)
    assert any(f"Native preparation {symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("guard,original", [
    pytest.param("DeferredReadGuard", "PublicationRwLockReadGuard", id="read-shared-kernel"),
    pytest.param("DeferredWriteGuard", "PublicationRwLockWriteGuard", id="write-shared-kernel"),
])
def test_hydration_concrete_guards_use_reviewed_release_kernel(fixture, guard, original):
    root, _, _, _ = fixture
    path = root / "crates/iroha_core/src/publication_rwlock/deferred.rs"
    source = path.read_text()
    invocation = f"deferred_guard!({guard}, {original});"
    assert source.count(invocation) == 1
    path.write_text(source.replace(invocation, f"deferred_guard!({guard}, ForeignGuard);", 1))
    errors = validate(fixture)
    assert any("Native hydration shared guard invocation changed: " + invocation == error for error in errors), errors
    assert not any("digest" in error or "must have one" in error or "must occur exactly once" in error for error in errors), errors

@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/publication_lock.rs', 'method', 'DeferredPublicationGuard::deref', 'self.guard', 'other.guard', id='deferred-guard-keeps-original-deref'),
    pytest.param('crates/iroha_core/src/publication_lock.rs', 'method', 'DeferredPublicationGuard::deref_mut', 'self.guard', 'other.guard', id='deferred-guard-keeps-original-deref_mut'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'DaRewindReleases::new', 'DaHydrationReleases::new(state)', 'DaHydrationReleases::new(other)', id='rewind-original-six-index-sources'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'DaRewindReleases::new', 'state.state_write_lock.defer_notifications()', 'other.state_write_lock.defer_notifications()', id='rewind-original-write-fence-source'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', 'macro', 'runtime_cells', 'target: original.target,', 'target: other,', id='acquired-keeps-original-state-target'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', 'method', 'AcquiredRuntimeBlock::rewind_da_indexes_to_height', 'DaRewindReleases::new(self.target)', 'DaRewindReleases::new(other)', id='rewind-cannot-select-another-state'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', 'method', 'AcquiredRuntimeBlock::rewind_da_indexes_to_height', 'self.target\n            .rewind_da_indexes_to_height_with_releases(target_height, releases)', 'other.rewind_da_indexes_to_height_with_releases(target_height, releases)', id='rewind-engine-uses-acquired-state'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', 'method', 'AcquiredRuntimeBlock::rewind_da_indexes_to_height', 'target_height, releases', 'target_height, &mut da_hydration::DaRewindReleases::new(self.target)', id='rewind-engine-borrows-retained-notices'),
    pytest.param('crates/iroha_core/src/state/da_hydration.rs', 'method', 'State::rewind_da_indexes_to_height_with_releases', '} = releases;', '} = &mut DaRewindReleases::new(self);', id='rewind-does-not-recreate-local-notices'),
    pytest.param('crates/iroha_core/src/state/state_block_construction.rs', 'method', 'State::construct_acquired_block', 'state_ref: self,\n                read_releases: StateViewReleases::new(self),\n                da_rewind_releases,', 'state_ref: self,\n                read_releases: StateViewReleases::new(self),\n                da_rewind_releases: None,', id='construction-transfers-original-rewind-notices'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/journals.rs', 'method', 'PreparedCarrier::prepare_journals', 'let da_rewind_releases;\n        let read_releases;\n        let mut original = self;', '\n        let read_releases;\n        let mut original = self;let da_rewind_releases;', id='capture-rewind-notices-outlive-original'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/journals.rs', 'method', 'PreparedCarrier::prepare_journals', 'da_rewind_releases = original_da_rewind_releases;', 'da_rewind_releases = None; drop(original_da_rewind_releases);', id='capture-keeps-original-rewind-owner'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/journals.rs', 'method', 'PreparedCarrier::prepare_journals', '        let mut pending = StateJournalCapture::new(\n            world.capture_slot(),\n            runtime_journals::RuntimeCapture::new(\n                canonical_runtime.into_executing(),\n                commit_topology.into_executing(),\n                prev_commit_topology.into_executing(),\n                lane_consensus_contexts.into_executing(),\n            ),\n            transactions.into_capture(),\n            block_hashes.into_executing(),\n        );\n        pending.try_capture().map_err(|error| match error {\n            StateCaptureError::World(error) => CarrierJournalPreparationError::WorldCapture(error),\n            StateCaptureError::Membership(error) => {\n                CarrierJournalPreparationError::Membership(error)\n            }\n        })?;\n        let components = pending.into_components();\n        // Successful capture freed all original State writers; no journal authority\n        // is derived from these completed, same-source notification batches.\n        drop(da_rewind_releases);', '        drop(da_rewind_releases);\n        let mut pending = StateJournalCapture::new(\n            world.capture_slot(),\n            runtime_journals::RuntimeCapture::new(\n                canonical_runtime.into_executing(),\n                commit_topology.into_executing(),\n                prev_commit_topology.into_executing(),\n                lane_consensus_contexts.into_executing(),\n            ),\n            transactions.into_capture(),\n            block_hashes.into_executing(),\n        );\n        pending.try_capture().map_err(|error| match error {\n            StateCaptureError::World(error) => CarrierJournalPreparationError::WorldCapture(error),\n            StateCaptureError::Membership(error) => {\n                CarrierJournalPreparationError::Membership(error)\n            }\n        })?;\n        let components = pending.into_components();\n        // Successful capture freed all original State writers; no journal authority\n        // is derived from these completed, same-source notification batches.\n', id='capture-rewind-notices-survive-partial-slots'),
    pytest.param('crates/iroha_core/src/state/carrier_preparation/journals.rs', 'method', 'PreparedCarrier::prepare_journals', '        let components = pending.into_components();\n        // Successful capture freed all original State writers; no journal authority\n        // is derived from these completed, same-source notification batches.\n        drop(da_rewind_releases);', '        drop(da_rewind_releases);\n        let components = pending.into_components();\n        // Successful capture freed all original State writers; no journal authority\n        // is derived from these completed, same-source notification batches.\n', id='capture-rewind-notices-retire-after-all-writers'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::block_and_revert_with_pristine_stage', 'acquired\n                .rewind_da_indexes_to_height(target_height)', 'self.rewind_da_indexes_to_height(target_height)', id='replacement-rewind-retains-original-acquisition'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime.rs', 'method', 'State::acquire_canonical_runtime_block', 'let baseline = self.lane_manifests.read().clone();\n            let mut registry_cache = self.sccp_registry_cache.lock().clone();\n            // All constructors use the same order. Every guard is dropped before\n            // retry; a World-only generation check cannot bind the predecessor.\n            // Hash construction detaches its private tree before waiting for World.\n            let block_hashes = self.block_hashes.try_next_block(replacement)?;\n            let membership = self.transactions.prepare_next_block(replacement)?;\n            // Projection payloads outlive joint physical retirement on refusal\n            // and unwind. Every Cell slot remains in this caller while initializing.\n            let projection_result;\n            let mut projection;\n            let mut sccp_registry;\n            let mut pending =\n                acquisition::RuntimeBlockAcquisition::new(self, block_hashes, membership);\n            if let Err(error) = pending.initialize(replacement) {\n                // The same original prepared generation returns to its sole\n                // preparation slot only after every physical sibling releases.\n                pending.retain_refused_membership();\n                return Err(error.into());\n            }', '\n            let mut registry_cache = self.sccp_registry_cache.lock().clone();\n            // All constructors use the same order. Every guard is dropped before\n            // retry; a World-only generation check cannot bind the predecessor.\n            // Hash construction detaches its private tree before waiting for World.\n            let block_hashes = self.block_hashes.try_next_block(replacement)?;\n            let membership = self.transactions.prepare_next_block(replacement)?;\n            // Projection payloads outlive joint physical retirement on refusal\n            // and unwind. Every Cell slot remains in this caller while initializing.\n            let projection_result;\n            let mut projection;\n            let mut sccp_registry;\n            let mut pending =\n                acquisition::RuntimeBlockAcquisition::new(self, block_hashes, membership);\n            if let Err(error) = pending.initialize(replacement) {\n                // The same original prepared generation returns to its sole\n                // preparation slot only after every physical sibling releases.\n                pending.retain_refused_membership();\n                return Err(error.into());\n            }let baseline = self.lane_manifests.read().clone();', id='acquisition-manifest-snapshot-before-writers'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime.rs', 'method', 'State::acquire_canonical_runtime_block', 'let mut registry_cache = self.sccp_registry_cache.lock().clone();\n            // All constructors use the same order. Every guard is dropped before\n            // retry; a World-only generation check cannot bind the predecessor.\n            // Hash construction detaches its private tree before waiting for World.\n            let block_hashes = self.block_hashes.try_next_block(replacement)?;\n            let membership = self.transactions.prepare_next_block(replacement)?;\n            // Projection payloads outlive joint physical retirement on refusal\n            // and unwind. Every Cell slot remains in this caller while initializing.\n            let projection_result;\n            let mut projection;\n            let mut sccp_registry;\n            let mut pending =\n                acquisition::RuntimeBlockAcquisition::new(self, block_hashes, membership);\n            if let Err(error) = pending.initialize(replacement) {\n                // The same original prepared generation returns to its sole\n                // preparation slot only after every physical sibling releases.\n                pending.retain_refused_membership();\n                return Err(error.into());\n            }', '\n            // All constructors use the same order. Every guard is dropped before\n            // retry; a World-only generation check cannot bind the predecessor.\n            // Hash construction detaches its private tree before waiting for World.\n            let block_hashes = self.block_hashes.try_next_block(replacement)?;\n            let membership = self.transactions.prepare_next_block(replacement)?;\n            // Projection payloads outlive joint physical retirement on refusal\n            // and unwind. Every Cell slot remains in this caller while initializing.\n            let projection_result;\n            let mut projection;\n            let mut sccp_registry;\n            let mut pending =\n                acquisition::RuntimeBlockAcquisition::new(self, block_hashes, membership);\n            if let Err(error) = pending.initialize(replacement) {\n                // The same original prepared generation returns to its sole\n                // preparation slot only after every physical sibling releases.\n                pending.retain_refused_membership();\n                return Err(error.into());\n            }let mut registry_cache = self.sccp_registry_cache.lock().clone();', id='acquisition-sccp-snapshot-before-writers'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime.rs', 'method', 'State::acquire_canonical_runtime_block', '&baseline,', '&self.lane_manifests.read().clone(),', id='acquisition-projection-uses-preacquisition-baseline'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime.rs', 'method', 'State::acquire_canonical_runtime_block', '&mut registry_cache,', '&mut self.sccp_registry_cache.lock(),', id='acquisition-registry-uses-preacquisition-snapshot'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime.rs', 'method', 'State::acquire_canonical_runtime_block', 'pending.world().sccp_registry.get(),', 'other_world.sccp_registry.get(),', id='acquisition-registry-validates-actual-world-wire'),

])
def test_rewind_and_acquisition_keep_original_notification_owners(fixture, path, kind, symbol, old, new):
    """No replacement, capture, or borrowed projection loses actual release custody."""
    root, _, checker, _ = fixture
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "rewind mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    source = root / path
    source.write_text(_mutate_complete_preparation_item(checker, source.read_text(), kind, symbol, item, old, new))
    errors = validate(fixture)
    ordered_capture = symbol == "PreparedCarrier::prepare_journals" and (old.lstrip().startswith("let da_rewind_releases;") or "drop(da_rewind_releases);" in old)
    diagnostic = "missing or reorders executable relation" if ordered_capture else "missing executable relation"
    assert any(f"Native preparation {symbol} {diagnostic}" in error for error in errors), errors
    assert all(error.startswith("Native preparation ") and (" missing executable relation " in error or " missing or reorders executable relation " in error) for error in errors), errors

@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'header: state.latest_block_header.defer_notifications(),', 'header: other.latest_block_header.defer_notifications(),', id='lifecycle-original-header'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'sccp: state.sccp_registry_cache.defer_notifications(),', 'sccp: other.sccp_registry_cache.defer_notifications(),', id='lifecycle-original-sccp'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'merge_admission: state.merge_admission.defer_notifications(),', 'merge_admission: other.merge_admission.defer_notifications(),', id='lifecycle-original-merge_admission'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'relays: state.lane_relays.defer_notifications(),', 'relays: other.lane_relays.defer_notifications(),', id='lifecycle-original-relays'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'manifests: state.lane_manifests.defer_notifications(),', 'manifests: other.lane_manifests.defer_notifications(),', id='lifecycle-original-manifests'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'privacy: state.lane_privacy_registry.defer_notifications(),', 'privacy: other.lane_privacy_registry.defer_notifications(),', id='lifecycle-original-privacy'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'commitments: state.da_commitments.defer_notifications(),', 'commitments: other.da_commitments.defer_notifications(),', id='lifecycle-original-commitments'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'confidential_compute: state.da_confidential_compute.defer_notifications(),', 'confidential_compute: other.da_confidential_compute.defer_notifications(),', id='lifecycle-original-confidential_compute'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'receipt_cursors: state.da_receipt_cursors.defer_notifications(),', 'receipt_cursors: other.da_receipt_cursors.defer_notifications(),', id='lifecycle-original-receipt_cursors'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'shard_cursors: state.da_shard_cursors.defer_notifications(),', 'shard_cursors: other.da_shard_cursors.defer_notifications(),', id='lifecycle-original-shard_cursors'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'pin_intents: state.da_pin_intents.defer_notifications(),', 'pin_intents: other.da_pin_intents.defer_notifications(),', id='lifecycle-original-pin_intents'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'LaneLifecycleReleases::new', 'hydrated: state.da_indexes_hydrated.defer_notifications(),', 'hydrated: other.da_indexes_hydrated.defer_notifications(),', id='lifecycle-original-hydrated'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::reset_lane_scoped_runtime_indexes', 'releases.relays.write()', 'self.lane_relays.write()', id='reset-retains-relays'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::reset_lane_scoped_runtime_indexes', 'releases.commitments.write()', 'self.da_commitments.write()', id='reset-retains-commitments'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::reset_lane_scoped_runtime_indexes', 'releases.receipt_cursors.write()', 'self.da_receipt_cursors.write()', id='reset-retains-receipt_cursors'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::reset_lane_scoped_runtime_indexes', 'releases.shard_cursors.write()', 'self.da_shard_cursors.write()', id='reset-retains-shard_cursors'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::reset_lane_scoped_runtime_indexes', 'releases.pin_intents.write()', 'self.da_pin_intents.write()', id='reset-retains-pin_intents'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::reset_lane_scoped_runtime_indexes', 'releases\n            .confidential_compute\n            .write()', 'self.da_confidential_compute.write()', id='reset-retains-confidential'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::prune_merge_admission_lane_progress', 'releases\n            .merge_admission\n            .write()', 'self.merge_admission.write()', id='reset-retains-admission'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::install_prepared_lane_manifests_in_publication', 'releases.manifests.write()', 'self.lane_manifests.write()', id='manifest-write-caller-release'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::install_prepared_lane_manifests_in_publication', 'releases.privacy.write()', 'self.lane_privacy_registry.write()', id='privacy-write-caller-release'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'State::persist_lane_lifecycle_cursor_journal', '&releases.shard_cursors.read()', '&self.da_shard_cursors.read()', id='persistence-original-cursor-read'),
    pytest.param('crates/iroha_core/src/state/lifecycle_index_publication.rs', 'method', 'State::persist_lane_lifecycle_cursor_journal', 'snapshot.persist()', 'self.persist_da_shard_cursor_journal()', id='persistence-no-fresh-index-read'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::install_lane_manifests', 'let mut releases = LaneLifecycleReleases::new(self);\n        let mut state_write_release = self.state_write_lock.defer_notifications();\n        let _state_write_lock = state_write_release.lock();', '\n        let mut state_write_release = self.state_write_lock.defer_notifications();\n        let _state_write_lock = state_write_release.lock();let mut releases = LaneLifecycleReleases::new(self);', id='manifest-owner-precedes-fence'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::install_lane_manifests', '&mut releases,', '&mut LaneLifecycleReleases::new(self),', id='manifest-shared-caller-owner'),
    pytest.param('crates/iroha_core/src/state/runtime_catalog_startup.rs', 'method', 'State::install_lane_manifests_if_consensus_compatible', 'releases.manifests.read()', 'self.lane_manifests.read()', id='compatible-refusal-read-retained'),
    pytest.param('crates/iroha_core/src/state/runtime_catalog_startup.rs', 'method', 'State::install_lane_manifests_if_consensus_compatible', 'let mut releases = LaneLifecycleReleases::new(self);\n        let mut state_write_release = self.state_write_lock.defer_notifications();\n        let _state_write_lock = state_write_release.lock();', '\n        let mut state_write_release = self.state_write_lock.defer_notifications();\n        let _state_write_lock = state_write_release.lock();let mut releases = LaneLifecycleReleases::new(self);', id='compatible-owner-precedes-fence'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::publish_prevalidated_lane_relay', 'let mut relay_releases = self.lane_relays.defer_notifications();\n        let mut cursor_releases = self.da_shard_cursors.defer_notifications();\n        let lifecycle_guard = self.lane_lifecycle_lock.lock();', '\n        let mut cursor_releases = self.da_shard_cursors.defer_notifications();\n        let lifecycle_guard = self.lane_lifecycle_lock.lock();let mut relay_releases = self.lane_relays.defer_notifications();', id='relay-notice-outlives-lifecycle'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::publish_prevalidated_lane_relay', 'let mut cursor_releases = self.da_shard_cursors.defer_notifications();\n        let lifecycle_guard = self.lane_lifecycle_lock.lock();', '\n        let lifecycle_guard = self.lane_lifecycle_lock.lock();let mut cursor_releases = self.da_shard_cursors.defer_notifications();', id='relay-cursor-notice-outlives-lifecycle'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::publish_prevalidated_lane_relay', 'self.lane_consensus_lifecycle_snapshot_with_cursors(&mut cursor_releases)', 'self.lane_consensus_lifecycle_snapshot()', id='relay-final-snapshot-retains-cursor'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::publish_prevalidated_lane_relay', 'relay_releases.write()', 'self.lane_relays.write()', id='relay-final-write-retained'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::lane_consensus_lifecycle_snapshot', 'self.lane_consensus_lifecycle_snapshot_with_cursors(&mut cursors)', 'self.lane_consensus_lifecycle_snapshot_with_cursors(&mut self.da_shard_cursors.defer_notifications())', id='standalone-snapshot-original-owner'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::lane_consensus_lifecycle_snapshot_with_cursors', 'cursors.read()', 'self.da_shard_cursors.read()', id='snapshot-kernel-original-cursor'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::record_da_lane_reset_watermarks', 'releases.shard_cursors.write()', 'self.da_shard_cursors.write()', id='watermark-write-retained'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::reset_lane_scoped_runtime_state', 'releases.hydrated.read()', 'self.da_indexes_hydrated.read()', id='reset-hydration-read-retained'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::reset_lane_scoped_runtime_state', 'self.persist_lane_lifecycle_cursor_journal(releases)', 'self.persist_da_shard_cursor_journal()', id='original-persistence-reset_lane_scoped_runtime_state'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::record_da_lane_reset_watermarks', 'self.persist_lane_lifecycle_cursor_journal(releases)', 'self.persist_da_shard_cursor_journal()', id='original-persistence-record_da_lane_reset_watermarks'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::apply_lane_geometry_updates', 'releases,', '&mut LaneLifecycleReleases::new(self),', id='geometry-original-apply_lane_geometry_updates'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::apply_lane_geometry_updates_with_certified_drain_frontiers', 'releases,', '&mut LaneLifecycleReleases::new(self),', id='geometry-original-apply_lane_geometry_updates_with_certified_drain_frontiers'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::rollback_lane_geometry_updates', 'releases,', '&mut LaneLifecycleReleases::new(self),', id='geometry-original-rollback_lane_geometry_updates'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::preflight_committed_autoscale_lane_geometry', 'releases,', '&mut LaneLifecycleReleases::new(self),', id='geometry-original-preflight_committed_autoscale_lane_geometry'),
    pytest.param('crates/iroha_core/src/state/geometry_publication.rs', 'method', 'State::resume_lane_geometry_publication', 'releases.shard_cursors.write()', 'self.da_shard_cursors.write()', id='geometry-retains-cursor-resume_lane_geometry_publication'),
    pytest.param('crates/iroha_core/src/state/geometry_publication.rs', 'method', 'State::resume_lane_geometry_publication', 'releases.hydrated.read()', 'self.da_indexes_hydrated.read()', id='geometry-retains-status-resume_lane_geometry_publication'),
    pytest.param('crates/iroha_core/src/state/geometry_publication.rs', 'method', 'State::resume_lane_geometry_publication', 'self.persist_lane_lifecycle_cursor_journal(releases)', 'self.persist_da_shard_cursor_journal()', id='geometry-retains-persistence-resume_lane_geometry_publication'),
    pytest.param('crates/iroha_core/src/state/geometry_publication.rs', 'method', 'State::rollback_owned_lane_geometry', 'releases.shard_cursors.write()', 'self.da_shard_cursors.write()', id='geometry-retains-cursor-rollback_owned_lane_geometry'),
    pytest.param('crates/iroha_core/src/state/geometry_publication.rs', 'method', 'State::rollback_owned_lane_geometry', 'releases.hydrated.read()', 'self.da_indexes_hydrated.read()', id='geometry-retains-status-rollback_owned_lane_geometry'),
    pytest.param('crates/iroha_core/src/state/geometry_publication.rs', 'method', 'State::rollback_owned_lane_geometry', 'self.persist_lane_lifecycle_cursor_journal(releases)', 'self.persist_da_shard_cursor_journal()', id='geometry-retains-persistence-rollback_owned_lane_geometry'),
    pytest.param('crates/iroha_core/src/state/geometry_publication.rs', 'method', 'State::resume_lane_geometry_publication', 'let publish_cursors = releases.is_some();', 'let publish_cursors = true;', id='replay-none-cannot-publish-indexes'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::validate_committed_autoscale_lane_lifecycle', 'prospective_nexus.dataspace_catalog = runtime_catalog_transition_dataspaces(\n                &nexus,\n                releases.manifests.read().as_ref(),', 'prospective_nexus.dataspace_catalog = runtime_catalog_transition_dataspaces(\n                &nexus,\n                self.lane_manifests.read().as_ref(),', id='committed-validation-original-manifests'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::merge_consensus_snapshot_inner_with_releases', 'releases\n                .shard_cursors\n                .read()', 'self.da_shard_cursors.read()', id='drain-original-shard_cursors'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::merge_consensus_snapshot_inner_with_releases', 'releases.merge_admission.read()', 'self.merge_admission.read()', id='drain-original-merge_admission'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::unmerged_merge_admissible_relay_progress_with_releases', 'releases.relays.read()', 'self.lane_relays.read()', id='drain-original-relays'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::lane_has_drain_blocking_evidence_with_releases', 'self.lane_consensus_lifecycle_snapshot_with_cursors(&mut releases.shard_cursors)', 'self.lane_consensus_lifecycle_snapshot()', id='drain-shared-cursor-snapshot'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::native_amx_participant_application_snapshot_with_lifecycle', 'for lane in lifecycle.nexus.lane_catalog.lanes()', 'for lane in self.lane_consensus_lifecycle_snapshot().nexus.lane_catalog.lanes()', id='defining-kernel-borrows-lifecycle-native_amx_participant_application_snapshot_with_lifecycle'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::native_amx_participant_frontiers_pending_durable_evidence_snapshot_with_lifecycle', '.native_amx_participant_application_snapshot_with_lifecycle(lifecycle)?', '.native_amx_participant_application_snapshot()?', id='defining-kernel-borrows-lifecycle-native_amx_participant_frontiers_pending_durable_evidence_snapshot_with_lifecycle'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::unapplied_native_amx_participant_control_heights_snapshot_with_lifecycle', '.native_amx_participant_application_snapshot_with_lifecycle(lifecycle)?', '.native_amx_participant_application_snapshot()?', id='defining-kernel-borrows-lifecycle-unapplied_native_amx_participant_control_heights_snapshot_with_lifecycle'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::unapplied_lane_block_artifact_heights_snapshot_cached_with_lifecycle', 'Self::lane_block_artifact_routes(&lifecycle.nexus)', 'Self::lane_block_artifact_routes(&self.lane_consensus_lifecycle_snapshot().nexus)', id='defining-kernel-borrows-lifecycle-unapplied_lane_block_artifact_heights_snapshot_cached_with_lifecycle'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::unapplied_certified_lane_block_heights_snapshot_cached_with_lifecycle', 'Self::lane_block_artifact_routes(&lifecycle.nexus)', 'Self::lane_block_artifact_routes(&self.lane_consensus_lifecycle_snapshot().nexus)', id='defining-kernel-borrows-lifecycle-unapplied_certified_lane_block_heights_snapshot_cached_with_lifecycle'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::unapplied_certified_lane_block_height_with_lifecycle', '&& lifecycle.lane_route_and_incarnation_matches(', '&& self.lane_consensus_lifecycle_snapshot().lane_route_and_incarnation_matches(', id='defining-kernel-borrows-lifecycle-unapplied_certified_lane_block_height_with_lifecycle'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::pending_queue_plan_admission_blocks_lane_drain_with_releases', 'self.view_with_index_releases(releases)', 'self.view()', id='drain-pending-view-original-owner'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::try_view_once_with_index_releases', 'header.read()', 'self.latest_block_header.read()', id='view-retains-header'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::try_view_once_with_index_releases', 'manifests.read()', 'self.lane_manifests.read()', id='view-retains-manifests'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::try_view_once_with_index_releases', 'sccp.lock()', 'self.sccp_registry_cache.lock()', id='view-retains-sccp'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::try_view_once_with_index_releases', 'self.project_canonical_runtime_with_manifests(\n                canonical_runtime.get(),\n                &world,\n                &baseline,\n            )', 'self.project_canonical_runtime(canonical_runtime.get(), &world)', id='view-projection-uses-captured-baseline'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::try_view_once_with_index_releases', 'latest_hash, cached_header.as_ref()', 'latest_hash, self.latest_block_header.read().as_ref()', id='view-header-kernel-uses-captured-header'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::view_with_index_releases', '&mut releases.sccp,', '&mut self.sccp_registry_cache.defer_notifications(),', id='view-keeps-original-sccp-source'),
    pytest.param('crates/iroha_core/src/state/canonical_runtime.rs', 'method', 'State::project_canonical_runtime_with_manifests', 'baseline.baseline_consensus_policy_digest()', 'self.lane_manifests.read().baseline_consensus_policy_digest()', id='projection-borrows-original-baseline'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::sccp_registry_snapshot_from_cache', 'cache.matches(wire)', 'state.sccp_registry_cache.lock().matches(wire)', id='registry-kernel-borrows-original-cache'),
    pytest.param('crates/iroha_core/src/state.rs', 'fn', 'persist_committed_lane_block_session_lifecycle_bound', '.certified_lane_block_persistence_authority(&session.proposal, &mut cursor_releases)', '.certified_lane_block_persistence_authority(&session.proposal, &mut self.da_shard_cursors.defer_notifications())', id='state-fence-cursor-persist_committed_lane_block_session_lifecycle_bound'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::latest_certified_lane_block_frontier_sessions_snapshot_cached', 'self.lane_consensus_lifecycle_snapshot_with_cursors(&mut cursor_releases)', 'self.lane_consensus_lifecycle_snapshot()', id='state-fence-cursor-latest_certified_lane_block_frontier_sessions_snapshot_cached'),
    pytest.param('crates/iroha_core/src/state.rs', 'fn', 'lane_application_certified_repair_snapshot_cached', 'self.lane_consensus_lifecycle_snapshot_with_cursors(&mut cursor_releases)', 'self.lane_consensus_lifecycle_snapshot()', id='state-fence-cursor-lane_application_certified_repair_snapshot_cached'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::certified_lane_block_persistence_authority', 'self.lane_consensus_lifecycle_snapshot_with_cursors(cursors)', 'self.lane_consensus_lifecycle_snapshot()', id='certificate-cursor-projection-borrows-original'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::record_globally_committed_merge_entry', 'let mut admission_releases = self.merge_admission.defer_notifications();\n        let mut publication_notice = self.state_view_publication();\n        let _state_write_lock = self.state_write_lock.lock();', '\n        let mut publication_notice = self.state_view_publication();\n        let _state_write_lock = self.state_write_lock.lock();let mut admission_releases = self.merge_admission.defer_notifications();', id='merge-notice-outlives-state-fence'),

])
def test_lifecycle_index_retains_original_release_custody(fixture, path, kind, symbol, old, new):
    """Reject a real original-source, projection, or outer-lifetime regression."""
    root, _, checker, _ = fixture
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "lifecycle mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    source = root / path
    source.write_text(_mutate_complete_preparation_item(
        checker, source.read_text(), kind, symbol, item, old, new,
    ))
    errors = validate(fixture)
    diagnostic = "missing or reorders executable relation" if symbol == "State::install_lane_manifests" and old.lstrip().startswith("let mut releases =") else "missing executable relation"
    assert any(f"Native preparation {symbol} {diagnostic}" in error for error in errors), errors
    assert all(error.startswith("Native preparation ") and (" missing executable relation " in error or " missing or reorders executable relation " in error) for error in errors), errors


@pytest.mark.parametrize("path,kind,symbol,old,new", [
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::validate_committed_autoscale_lane_lifecycle', 'return self.validate_committed_autoscale_drain_metadata_update(\n                &nexus,\n                &lane_incarnations,\n                &lane_incarnation_activation_heights,\n                pending,\n                block_height,\n                releases,\n            );', 'return self.validate_committed_autoscale_drain_metadata_update(\n                &nexus,\n                &lane_incarnations,\n                &lane_incarnation_activation_heights,\n                pending,\n                block_height,\n                &mut LaneLifecycleReleases::new(self),\n            );', id='drain-caller-forwards-original-owner'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::validate_committed_autoscale_drain_metadata_update', 'let update = &pending.catalog_update;', 'let mut releases = LaneLifecycleReleases::new(self);\n        let update = &pending.catalog_update;', id='drain-callee-retains-borrowed-owner'),
    pytest.param('crates/iroha_core/src/state.rs', 'method', 'State::validate_committed_autoscale_drain_metadata_update', 'releases.manifests.read().as_ref()', 'self.lane_manifests.read().as_ref()', id='drain-manifest-read-uses-original-source'),
])
def test_committed_drain_metadata_release_custody(fixture, path, kind, symbol, old, new):
    """A real drain must retain its borrowed owner through the defining read."""
    root, _, checker, _ = fixture
    errors = []
    item = checker._rust_binding_item(root, path, kind, symbol, "drain custody mutation", errors)
    assert errors == [] and item is not None and item.count(old) == 1
    assert checker.native_preparation_contract._code(old) != checker.native_preparation_contract._code(new)
    source = root / path
    source.write_text(_mutate_complete_preparation_item(
        checker, source.read_text(), kind, symbol, item, old, new,
    ))
    errors = validate(fixture)
    assert any(f"Native preparation {symbol} missing executable relation " in error for error in errors), errors
    assert all(error.startswith("Native preparation ") and (" missing executable relation " in error or " missing or reorders executable relation " in error) for error in errors), errors


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param("WorldStorageMode<K, V> for Prepaid::publication_slot",
                 "Some(scope) => original.try_publication_slot(scope, target)",
                 "Some(scope) => original.try_publication_slot(scope, other)",
                 id="prepaid-original-target"),
    pytest.param("WorldStorageMode<K, V> for Prepaid::publication_slot",
                 "PublicationPreparationError::Admission(AdmittedStorageError::ScopeIdentity)",
                 "PublicationPreparationError::Changed", id="missing-scope-is-admission-refusal"),
    pytest.param("WorldStorageMode<K, V> for Prepaid::release_writers",
                 "slot.release_writers();", "let _ = slot;", id="prepaid-original-writer-release"),
    pytest.param("WorldStorageMode<K, V> for Prepaid::recover_original",
                 'journal.take().expect("original refused field")',
                 'panic!("discarded original journal")', id="refusal-retains-original-journal"),
    pytest.param("WorldStorageMode<K, V> for Untracked::abort",
                 "prepared.abort()", "other.abort()", id="untracked-original-abort"),
])
def test_world_storage_mode_delegates_exact_original_owners(fixture, symbol, old, new):
    root, _, checker, _ = fixture
    target = root / "crates/iroha_core/src/state/world_storage_mode.rs"
    source = target.read_text()
    (item,) = checker._extract_rust_binding_items(source, "method", symbol)
    assert item.count(old) == 1
    owners = [owner for owner in checker._rust_impl_items(source, symbol.rsplit("::", 1)[0])
              if item in owner]
    assert len(owners) == 1
    owner = owners[0]
    assert source.count(owner) == 1
    target.write_text(source.replace(owner, owner.replace(item, item.replace(old, new), 1), 1))
    errors = validate(fixture)
    assert any(f"{symbol} missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("owner,anchor,old,new", [
    ("BLOCK", "let (block, state, native)", "MergeLedgerCommitError::ExecutionBatchInvalid(reason)", "MergeLedgerCommitError::Storage(reason)"),
    ("PREFIX", "fn capture", ".verify_execution_output_seal(block)\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?", ".verify_execution_output_seal(block).unwrap_or_default()"),
    ("PREFIX", "fn capture", ".verified_fastpq_source_inventory_for_capture()\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?", ".verified_fastpq_source_inventory_for_capture().unwrap_or_default()"),
    ("PREFIX", "fn capture", ".verify_cached_ordinary_witness_content(&verified_inventory)\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?", ".verify_cached_ordinary_witness_content(&verified_inventory).ok()"),
    ("PREFIX", "let manifest =", ".map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?", ".map_err(MergeLedgerCommitError::Storage)?"),
    ("PREFIX", "let lanes =", ".map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?", ".map_err(MergeLedgerCommitError::Storage)?"),
    ("PREFIX", "let commitment =", "MergeLedgerCommitError::ExecutionBatchInvalid(error.to_owned())", "MergeLedgerCommitError::Storage(error.to_owned())"),
    ("PREFIX", "let inventory = state", ".map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?", ".unwrap_or_default()"),
    ("PREFIX", "fn prepare_world_effects", "state.validate_merge_carrier_entrypoint_binding()?;", "state.validate_merge_carrier_entrypoint_binding().map_err(|error| MergeLedgerCommitError::ExecutionBatchInvalid(error.to_string()))?;"),
    ("PREFIX", "fn prepare_world_effects", ".validate_canonical_runtime_projection()\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?", ".validate_canonical_runtime_projection().ok()"),
    ("PREFIX", "fn prepare_world_effects", ".verify_lane_consensus_contexts_publication()\n            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?", ".verify_lane_consensus_contexts_publication().ok()"),
    ("PREFIX", "fn prepare_world_effects", ".map_err(MergeLedgerCommitError::ExecutionBatchInvalid)\n    }", ".map_err(MergeLedgerCommitError::Storage)\n    }"),
    ("PREFIX", "pub(super) fn prepare", "if !preparation.state.autoscale_lifecycle_evaluated", "if false"),
    ("PREFIX", "pub(super) fn prepare", "ApplyTopologyAuthority::V2Finality,\n        )?;", "ApplyTopologyAuthority::V2Finality,\n        ).map_err(|error| MergeLedgerCommitError::ExecutionBatchInvalid(error.to_string()))?;"),
    ("PREFIX", "pub(super) fn prepare", ".prepare_carrier_publication_events(block.header())?", ".prepare_carrier_publication_events(block.header()).map_err(|error| MergeLedgerCommitError::ExecutionBatchInvalid(error.to_string()))?"),
    ("NATIVE_VALIDATION", "struct NativeValidationCandidate", "phase: Box<Option<NativeValidationPhase>>", "phase: Option<NativeValidationPhase>"),
    ("NATIVE_VALIDATION", "enum NativeValidationPhase", "carrier: RetainedCarrier<CarrierShellAdmission>", "carrier: wire::ExecutionCommitment"),
    ("NATIVE_VALIDATION", "struct AwaitingNativeSource", "recovered: Vec<(usize, VerifiedFirstLaneAdmittedInputV1)>", "recovered: Vec<usize>"),
    ("NATIVE_VALIDATION", "fn matches_candidate", "source.context.context() == context && source.proposal == *body", "source.context.context() == context"),
    ("NATIVE_VALIDATION", "fn matches_candidate", "*context_id == context.id()", "true"),
    ("NATIVE_VALIDATION", "fn matches_candidate", "carrier.matches_validation_candidate(context, body)", "true"),
    ("NATIVE_VALIDATION", "fn matches_candidate", "carrier.artifact().height_context == *context", "true"),
    ("NATIVE_VALIDATION", "fn ready_commitment", "evidence_ready.then(|| carrier.ready_commitment()).flatten()", "carrier.ready_commitment()"),
    ("NATIVE_VALIDATION", "fn ready_commitment", "Some(carrier.artifact().commit_qc.execution_commitment)", "Some(Default::default())"),
    ("NATIVE_VALIDATION", "fn try_publish", "&validator.service,", "&other_service,"),
    ("NATIVE_VALIDATION", "fn try_publish", "&validator.service.state,", "&other_state,"),
    ("NATIVE_VALIDATION", "fn try_publish", "return Err((self, refusal));", "return Err((replacement, refusal));"),
    ("SERVICE_QUEUE", "fn from_service", "state: &service.state", "state: other_state"),
    ("SERVICE_QUEUE", "fn from_service", "queue: &service.queue", "queue: other_queue"),
    ("SERVICE_QUEUE", "impl<'service> OriginalCarrierQueue", "#[cfg(test)]", ""),
    ("APPLY", "impl V2ApplyService {\n    /// Borrow this service", "#[cfg(test)]", ""),
    ("RUNNER_HISTORY", "fn schedule_local_proposal", "native.retain_candidate_source(source);", "drop(source);"),
    ("RUNNER_HISTORY", "fn schedule_local_proposal", "native.retain_candidate_source(source);", "let assembly = outcome?; native.retain_candidate_source(source);"),
])
def test_native_preparation_requires_typed_errors_and_current_original_owners(fixture, owner, anchor, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / getattr(checker.native_preparation_contract, owner), anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error or "retained carrier" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


def test_native_preparation_unlocks_before_original_admission_drop(fixture):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.PHYSICAL_CARRIER
    anchor = "struct SourceAuthenticatedCarrier<'target, Admission>"
    helper.replace_once_after(path, anchor, "    kura: KuraPublicationLease<'target>,\n", "")
    helper.replace_once_after(path, anchor, "        KuraWsvCheckpointReceipt,\n    >,", "        KuraWsvCheckpointReceipt,\n    >,\n    kura: KuraPublicationLease<'target>,")
    errors = validate(fixture)
    assert any("Native single-lease SourceAuthenticatedCarrier" in error for error in errors), errors


@pytest.fixture
def generic_fixture(fixture):
    """Copy all owners before mutating either canonical consumer's source."""
    root, helper, checker, models = fixture
    model, = [model for model in models if model["module"] == checker.native_preparation_contract.MODEL]
    relatives = {Path(row["path"]) for row in model["production_symbols"]}
    relatives.update(Path(path) for path, _, _, _ in checker.NATIVE_PREPUBLICATION_BINDINGS)
    relatives.update(path for path, _, _ in checker.native_merge_manifest.NATIVE_MERGE_MANIFEST_RAW_TEST_CHECKS)
    relatives.update(Path(path) for path, _, _, _ in (
        *checker.native_merge_manifest.NATIVE_MERGE_MANIFEST_NORMALIZED_RELATIONS,
        *checker.native_merge_manifest.NATIVE_MERGE_MANIFEST_ORDERED_RELATIONS,
    ))
    helper.copy_reviewed_source_fixture_with_includes(root, checker, relatives)
    return fixture


def generic_native_errors(fixture):
    """Run the canonical literal-token consumer, including its real TLA ledger."""
    root, helper, checker, models = fixture
    model, = [model for model in models if model["module"] == checker.native_preparation_contract.MODEL]
    errors = []
    with checker._reviewed_rust_source_cache():
        checker._validate_model(root, helper.ROOT_DIR / "formal/sumeragi_v2", model, errors)
    return errors


def test_native_preparation_accepts_canonical_generic_consumer(generic_fixture):
    assert generic_native_errors(generic_fixture) == []


@pytest.mark.parametrize("symbol,old,new", [
    ("LifecycleProducerClaimDispositionV1::blocks_runtime", "Self::AwaitingNativeSource", "Self::AwaitingCompletion"),
    ("LaunchedProductionLifecycleV1::settle_lifecycle_decision_apply_completion_owner", "RetainedLifecycleDecisionApplyDeferredV1 { completion }", "RetainedLifecycleDecisionApplyDeferredV1 { completion: replacement }"),
    ("LaunchedProductionLifecycleV1::settle_lifecycle_decision_apply_completion_owner", "let owner = &mut self.owner;", "let owner = &mut other.owner;"),
    ("LaunchedProductionLifecycleV1::settle_lifecycle_decision_apply_completion_owner", "settle_applied_lifecycle_decision_apply_completion(owner, executor, completion)", "settle_applied_lifecycle_decision_apply_completion(owner, executor, replacement)"),
    ("run_lifecycle_active_height", "native.take_service_publication(services);", "native.take_service_publication(other_services);"),
    ('run_lifecycle_active_height', 'native.service_sources(services, now).inspect_err(|error| {\n                    iroha_logger::error!(\n                        ?error,\n                        height = context.height,\n                        "Sumeragi v2 Native source service failed closed"\n                    );\n                })?;', 'native.service_sources(other_services, now).inspect_err(|error| {\n                    iroha_logger::error!(\n                        ?error,\n                        height = context.height,\n                        "Sumeragi v2 Native source service failed closed"\n                    );\n                })?;'),
    ('run_lifecycle_active_height', 'native\n            .poll(native_global, native_network, now, receiver)\n            .inspect_err(|error| {\n                iroha_logger::error!(\n                    ?error,\n                    height = context.height,\n                    "Sumeragi v2 Native process turn failed closed"\n                );\n            })?;', 'native\n            .poll(native_global, native_network, now, other_receiver)\n            .inspect_err(|error| {\n                iroha_logger::error!(\n                    ?error,\n                    height = context.height,\n                    "Sumeragi v2 Native process turn failed closed"\n                );\n            })?;'),
    ('run_lifecycle_active_height', 'native.service_sources(services, now).inspect_err(|error| {\n                    iroha_logger::error!(\n                        ?error,\n                        height = context.height,\n                        "Sumeragi v2 Native source service failed closed"\n                    );\n                })?;', 'native.service_sources(services, now).inspect_err(|error| {\n                    iroha_logger::error!(\n                        ?error,\n                        height = context.height,\n                        "Sumeragi v2 Native source service failed closed"\n                    );\n                });'),
    ('run_lifecycle_active_height', 'native\n            .poll(native_global, native_network, now, receiver)\n            .inspect_err(|error| {\n                iroha_logger::error!(\n                    ?error,\n                    height = context.height,\n                    "Sumeragi v2 Native process turn failed closed"\n                );\n            })?;', 'native\n            .poll(native_global, native_network, now, receiver)\n            .inspect_err(|error| {\n                iroha_logger::error!(\n                    ?error,\n                    height = context.height,\n                    "Sumeragi v2 Native process turn failed closed"\n                );\n            });'),
    ("run_pending_active_height", "native.take_service_publication(services);", "native.take_service_publication(other_services);"),
    ("run_pending_active_height", "native.service_sources(services, Instant::now())", "native.service_sources(other_services, Instant::now())"),
    ("run_pending_active_height", "native.poll(native_global, native_network, Instant::now(), receiver)?;", "native.poll(native_global, native_network, Instant::now(), other_receiver)?;"),
])
def test_native_preparation_both_consumers_bind_current_runner_owners(generic_fixture, symbol, old, new):
    fixture = generic_fixture
    root, helper, checker, _ = fixture
    binding, = [row for row in checker.native_preparation_contract.NATIVE_CURRENT_RUNNER_BINDINGS if row[2] == symbol]
    path = root / binding[0]
    helper.replace_once_after(path, "fn " + symbol.rsplit("::", 1)[-1], old, new)
    assert any("source-binding token" in error for error in generic_native_errors(fixture))
    assert any("executable relation" in error or "retained carrier" in error for error in validate(fixture))


@pytest.mark.parametrize("symbol", ["run_lifecycle_active_height", "run_pending_active_height"])
def test_native_preparation_keeps_native_source_service_order(fixture, symbol):
    root, helper, checker, _ = fixture
    binding, = [row for row in checker.native_preparation_contract.NATIVE_CURRENT_RUNNER_BINDINGS if row[2] == symbol]
    path = root / binding[0]
    helper.replace_once_after(path, "fn " + symbol, "native.take_service_publication(services);", "native.poll(native_global, native_network, now, receiver)?; native.take_service_publication(services);")
    # The canonical raw fragments still exist; the Native order check must reject
    # an additional early poll rather than merely find the later valid sequence.
    assert any("executable relation" in error for error in validate(fixture))


def test_native_preparation_accepts_prepublication_consumer(generic_fixture):
    root, _, checker, models = generic_fixture
    errors = []
    with checker._reviewed_rust_source_cache():
        checker._validate_native_prepublication_contract(root, models, errors)
    assert errors == []


@pytest.mark.parametrize("old,new", [
    ("                block, None,\n", "                block, state.staged_merge_entry(),\n"),
    (".map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;", ".map_err(MergeLedgerCommitError::Storage)?;"),
])
def test_native_preparation_prepublication_keeps_exact_typed_manifest(generic_fixture, old, new):
    root, helper, checker, models = generic_fixture
    path = root / checker.native_preparation_contract.PREFIX
    source = path.read_text()
    start = source.index("let manifest =", source.index("fn capture("))
    assert old in source[start:]
    path.write_text(source[:start] + source[start:].replace(old, new, 1))
    errors = []
    with checker._reviewed_rust_source_cache():
        checker._validate_native_prepublication_contract(root, models, errors)
    assert any("PrefixPreparation::capture" in error for error in errors), errors


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param('archive_refusal', 'E::Provider(error) if matches!(error.as_ref(), P::IndexBusy { .. }) => {\n            let P::IndexBusy { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("provider_archive_index", wait, wake)\n        }', 'E::Provider(error) if matches!(error.as_ref(), P::IndexBusy { .. }) => {\n            let P::IndexBusy { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("provider_archive_index", replacement_wait, wake)\n        }', id='archive_refusal-typed-arm-0'),
    pytest.param('archive_refusal', 'E::Provider(error) if matches!(error.as_ref(), P::IndexBusy { .. }) => {\n            let P::IndexBusy { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("provider_archive_index", wait, wake)\n        }', 'E::Provider(error) if matches!(error.as_ref(), P::IndexBusy { .. }) => {\n            let R::IndexBusy { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("provider_archive_index", wait, wake)\n        }', id='archive_refusal-foreign-destructure-0'),
    pytest.param('archive_refusal', 'E::Reputation(error) if matches!(error.as_ref(), R::IndexBusy { .. }) => {\n            let R::IndexBusy { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("reputation_archive_index", wait, wake)\n        }', 'E::Reputation(error) if matches!(error.as_ref(), R::IndexBusy { .. }) => {\n            let R::IndexBusy { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("reputation_archive_index", replacement_wait, wake)\n        }', id='archive_refusal-typed-arm-1'),
    pytest.param('archive_refusal', 'E::Reputation(error) if matches!(error.as_ref(), R::IndexBusy { .. }) => {\n            let R::IndexBusy { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("reputation_archive_index", wait, wake)\n        }', 'E::Reputation(error) if matches!(error.as_ref(), R::IndexBusy { .. }) => {\n            let P::IndexBusy { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("reputation_archive_index", wait, wake)\n        }', id='archive_refusal-foreign-destructure-1'),
    pytest.param('archive_refusal', 'E::Provider(error) if matches!(error.as_ref(), P::CaptureReserved { .. }) => {\n            let P::CaptureReserved { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("provider_archive_capture", wait.release_wait(), wake)\n        }', 'E::Provider(error) if matches!(error.as_ref(), P::CaptureReserved { .. }) => {\n            let P::CaptureReserved { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("provider_archive_capture", replacement_wait, wake)\n        }', id='archive_refusal-typed-arm-2'),
    pytest.param('archive_refusal', 'E::Provider(error) if matches!(error.as_ref(), P::CaptureReserved { .. }) => {\n            let P::CaptureReserved { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("provider_archive_capture", wait.release_wait(), wake)\n        }', 'E::Provider(error) if matches!(error.as_ref(), P::CaptureReserved { .. }) => {\n            let R::CaptureReserved { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("provider_archive_capture", wait.release_wait(), wake)\n        }', id='archive_refusal-foreign-destructure-2'),
    pytest.param('archive_refusal', 'E::Reputation(error) if matches!(error.as_ref(), R::CaptureReserved { .. }) => {\n            let R::CaptureReserved { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("reputation_archive_capture", wait.release_wait(), wake)\n        }', 'E::Reputation(error) if matches!(error.as_ref(), R::CaptureReserved { .. }) => {\n            let R::CaptureReserved { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("reputation_archive_capture", replacement_wait, wake)\n        }', id='archive_refusal-typed-arm-3'),
    pytest.param('archive_refusal', 'E::Reputation(error) if matches!(error.as_ref(), R::CaptureReserved { .. }) => {\n            let R::CaptureReserved { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("reputation_archive_capture", wait.release_wait(), wake)\n        }', 'E::Reputation(error) if matches!(error.as_ref(), R::CaptureReserved { .. }) => {\n            let P::CaptureReserved { wait } = error.as_ref() else {\n                unreachable!()\n            };\n            busy("reputation_archive_capture", wait.release_wait(), wake)\n        }', id='archive_refusal-foreign-destructure-3'),
    pytest.param('physical_refusal', 'E::Provider(P::IndexBusy { wait }) | E::Archive(A::Provider(P::IndexBusy { wait })) => {\n            busy("provider_archive_index", wait, wake)\n        }', 'E::Provider(P::IndexBusy { wait }) | E::Archive(A::Provider(P::IndexBusy { wait })) => {\n            busy("provider_archive_index", replacement_wait, wake)\n        }', id='physical_refusal-typed-arm-0'),
    pytest.param('physical_refusal', 'E::Reputation(RArch::IndexBusy { wait })\n        | E::Archive(A::Reputation(RArch::IndexBusy { wait })) => {\n            busy("reputation_archive_index", wait, wake)\n        }', 'E::Reputation(RArch::IndexBusy { wait })\n        | E::Archive(A::Reputation(RArch::IndexBusy { wait })) => {\n            busy("reputation_archive_index", replacement_wait, wake)\n        }', id='physical_refusal-typed-arm-1'),
    pytest.param("RetainedCarrier::try_publish", "target: &State,", "target: &mut State,", id="target-borrow"),
    pytest.param("RetainedCarrier::try_publish", "queue: &OriginalCarrierQueue<'_>,", "queue: &Queue,", id="original-queue-owner"),
    pytest.param("RetainedCarrier::try_publish", "finality: VerifiedV2FinalityArtifact,", "finality: V2FinalityArtifact,", id="verified-finality"),
    pytest.param("RetainedCarrier::try_publish", "(Self, LocalValidationRefusal)", "(Self, SemanticRejection)", id="typed-local-refusal"),
    pytest.param("RetainedCarrier::try_publish", "Err((owner, error)) => return Err((owner, archive_refusal(&error, &wake)))", "Err((owner, error)) => return Err((replacement, archive_refusal(&error, &wake)))", id="capture-original-owner"),
    pytest.param("RetainedCarrier::try_publish", "if !matches_target || !queue.belongs_to(target)", "if !matches_target && !queue.belongs_to(target)", id="reject-either-foreign-owner"),
    pytest.param("RetainedCarrier::try_publish", "journals.bind_decision(finality.clone())", "journals.bind_decision(other.clone())", id="bind-original-decision"),
    pytest.param("RetainedCarrier::try_publish", "Self::Validated(refusal.journals)", "Self::Validated(replacement)", id="decision-refusal-journals"),
    pytest.param("RetainedCarrier::try_publish", "Self::Decided(decision) => decision.finality() == finality.artifact()", "Self::Decided(decision) => true", id="decided-retry-finality"),
    pytest.param("RetainedCarrier::try_publish", "Self::Checkpointed(decision) => decision.finality() == finality.artifact()", "Self::Checkpointed(decision) => true", id="checkpointed-retry-finality"),
    pytest.param("RetainedCarrier::try_publish", "if !same_finality", "if false", id="retry-finality-guard"),
    pytest.param("RetainedCarrier::try_publish", "let kura = &decision.journals.kura;", "let kura = &other_kura;", id="original-kura"),
    pytest.param("RetainedCarrier::try_publish", "kura.store_block(decision.block().clone())?;", "kura.store_block(replacement.clone())?;", id="original-body"),
    pytest.param("RetainedCarrier::try_publish", "let artifact = decision.finality();", "let artifact = other.artifact();", id="original-artifact"),
    pytest.param("RetainedCarrier::try_publish", "let checkpoint = decision.journals.checkpoint;", "let checkpoint = CapturedStateSnapshot::capture(target).hash();", id="never-recapture-live-state"),
    pytest.param("RetainedCarrier::try_publish", "kura.store_wsv_checkpoint(artifact.height, artifact.block_hash, checkpoint)?;", "kura.store_wsv_checkpoint(artifact.height + 1, artifact.block_hash, checkpoint)?;", id="exact-checkpoint-boundary"),
    pytest.param("RetainedCarrier::try_publish", "                            checkpoint,\n                            None,", "                            replacement_checkpoint,\n                            None,", id="manifest-captured-checkpoint"),
    pytest.param("RetainedCarrier::try_publish", ".with_authenticated_v2_commit_authority(artifact)", ".with_authenticated_v2_commit_authority(other)", id="manifest-original-authority"),
    pytest.param("RetainedCarrier::try_publish", "let receipt = kura.store_v2_finality_artifact(artifact)?;", "let receipt = kura.store_v2_finality_artifact(other)?;", id="exact-finality-receipt"),
    pytest.param("RetainedCarrier::try_publish", "&receipt,\n                        decision.journals.checkpoint,", "&other_receipt,\n                        decision.journals.checkpoint,", id="checkpoint-original-finality-receipt"),
    pytest.param("RetainedCarrier::try_publish", "decision.attach_checkpoint(checkpoint)", "decision.attach_checkpoint(other_checkpoint)", id="attach-original-receipt"),
    pytest.param("RetainedCarrier::try_publish", "Self::Decided(decision),\n                            LocalValidationRefusal::RecoveryRequired", "Self::Decided(replacement),\n                            LocalValidationRefusal::RecoveryRequired", id="durable-error-retains-decision"),
    pytest.param("RetainedCarrier::try_publish", "decision.try_prepare_physical(target, Some(queue))", "decision.try_prepare_physical(target, None)", id="physical-original-queue"),
    pytest.param("RetainedCarrier::try_publish", "Self::Checkpointed(decision),\n                    physical_refusal", "Self::Checkpointed(replacement),\n                    physical_refusal", id="physical-refusal-retains-checkpoint"),
    pytest.param("RetainedCarrier::try_publish", "(Self::Checkpointed(decision), refusal)", "(Self::Checkpointed(replacement), refusal)", id="visibility-refusal-retains-checkpoint"),
    pytest.param("RetainedCarrier::try_publish", ") => busy(field, wait, &wake)", ") => LocalValidationRefusal::RecoveryRequired(error.to_string())", id="visibility-busy-retains-wake"),
    pytest.param("busy", "wait.clone(), wake.clone()", "other_wait.clone(), wake.clone()", id="busy-original-release"),
    pytest.param("busy", "wait.clone(), wake.clone()", "wait.clone(), other_wake.clone()", id="busy-original-waker"),
    pytest.param("archive_refusal", 'busy("provider_archive_capture", wait.release_wait(), wake)', 'busy("provider_archive_capture", other_wait, wake)', id="archive-release"),
    pytest.param("archive_refusal", "_ => LocalValidationRefusal::RecoveryRequired(error.to_string())", "_ => LocalValidationRefusal::SemanticRejected(error.to_string())", id="archive-not-semantic"),
    pytest.param("physical_refusal", 'E::Queue(Q::Pending { wait, .. }) => busy("retiring_lane_queue", wait, wake)', 'E::Queue(Q::Pending { wait, .. }) => busy("retiring_lane_queue", other_wait, wake)', id="queue-retirement-release"),
    pytest.param("physical_refusal", ")) => busy(refusal.field, release, wake)", ")) => busy(refusal.field, other_release, wake)", id="world-admission-release"),
    pytest.param("physical_refusal", '\n        _ => LocalValidationRefusal::RecoveryRequired(format!("carrier publication: {error:?}"))', '\n        _ => LocalValidationRefusal::SemanticRejected(format!("carrier publication: {error:?}"))', id="physical-not-semantic"),
])
def test_live_retained_publication_both_consumers_reject_owner_substitution(generic_fixture, symbol, old, new):
    """Mutate the actual source while both real consumers retain the reviewed ledger."""
    root, helper, checker, _ = generic_fixture
    path = root / checker.native_preparation_contract.SERVICE_PUBLICATION
    helper.replace_once_after(path, "fn " + symbol.rsplit("::", 1)[-1], old, new)
    assert any("source-binding token" in error for error in generic_native_errors(generic_fixture))
    errors = validate(generic_fixture)
    assert any("executable relation" in error or "Native live publication" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("phase", ["Validated", "Decided", "Checkpointed"])
def test_live_retained_publication_each_phase_authenticates_target(fixture, phase):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.SERVICE_PUBLICATION
    old = "target.matches_kura_instance(&journals.kura)" if phase == "Validated" else "target.matches_kura_instance(&decision.journals.kura)"
    helper.replace_once_after(path, "Self::" + phase + "(", old, "true")
    assert any("exact phase identity" in error for error in validate(fixture))


@pytest.mark.parametrize("operation", [
    "store_block", "store_wsv_checkpoint", "store_commit_manifest",
    "store_v2_finality_artifact", "persist_wsv_checkpoint_for_v2_commit",
    "try_prepare_physical", "publish",
])
def test_live_retained_publication_rejects_early_duplicate_writes(fixture, operation):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.SERVICE_PUBLICATION
    helper.replace_once_after(path, "fn try_publish", "let original = match self.resume_capture()", f"other.{operation}(replacement)?;\n        let original = match self.resume_capture()")
    errors = validate(fixture)
    assert any("repeats or omits original operation" in error for error in errors), errors


@pytest.mark.parametrize("cut", ["finality-before-checkpoint", "receipt-before-manifest", "physical-before-durable"])
def test_live_retained_publication_rejects_durable_order_inversion(fixture, cut):
    root, _, checker, _ = fixture
    path = root / checker.native_preparation_contract.SERVICE_PUBLICATION
    source = path.read_text()
    if cut == "finality-before-checkpoint":
        moving = "                    let receipt = kura.store_v2_finality_artifact(artifact)?;\n"
        before = "                    kura.store_wsv_checkpoint(artifact.height, artifact.block_hash, checkpoint)?;"
    elif cut == "receipt-before-manifest":
        moving = "                    kura.persist_wsv_checkpoint_for_v2_commit(\n                        &receipt,\n                        decision.journals.checkpoint,\n                    )"
        before = "                    kura.store_commit_manifest("
    else:
        moving = "        let prepared = match decision.try_prepare_physical(target, Some(queue)) {\n            Ok(prepared) => prepared,\n            Err((decision, error)) => {\n                return Err((\n                    Self::Checkpointed(decision),\n                    physical_refusal(&error, &wake),\n                ));\n            }\n        };\n"
        before = "        let original = match self.resume_capture()"
    assert source.count(moving) == 1 and source.count(before) == 1
    source = source.replace(moving, "", 1).replace(before, moving + "\n" + before, 1)
    path.write_text(source)
    errors = validate(fixture)
    assert any("reorders authenticated publication" in error for error in errors), errors
    assert not any("missing executable relation" in error for error in errors), errors


def test_live_retained_publication_ledger_cannot_omit_live_owner(fixture):
    _, _, checker, models = fixture
    model, = [row for row in models if row["module"] == checker.native_preparation_contract.MODEL]
    model["production_symbols"] = [row for row in model["production_symbols"] if row["symbol"] != "RetainedCarrier::try_publish"]
    assert any("ledger owner RetainedCarrier::try_publish must occur exactly once" in error for error in validate(fixture))


@pytest.mark.parametrize("symbol,old,new", [
    pytest.param("Kura::store_commit_manifest", "self.ensure_checkpoint_accepts_manifest_write(&manifest)?;", "self.ensure_checkpoint_accepts_manifest_write(&manifest).ok();", id="manifest-original-checkpoint-gate"),
    pytest.param("Kura::store_commit_manifest", "let namespace = self.open_bound_progress_namespace(&path, &path)?;", "let namespace = self.open_bound_progress_namespace(&other, &other)?;", id="manifest-original-namespace"),
    pytest.param("Kura::store_commit_manifest", "file.sync_data()", "Ok(())", id="manifest-bytes-durable"),
    pytest.param("Kura::store_commit_manifest", "Self::sync_bound_progress_intent_directories(&namespace)", "Self::sync_bound_progress_intent_directories(&other)", id="manifest-original-ancestors"),
    pytest.param("Kura::store_commit_manifest", "if !self.bound_progress_namespace_unchanged(&namespace)", "if false", id="manifest-namespace-recheck"),
    pytest.param("Kura::store_commit_manifest", "self.bind_wsv_checkpoint_to_manifest(&manifest)?;", "self.bind_wsv_checkpoint_to_manifest(&replacement)?;", id="manifest-original-binding"),
    pytest.param("Kura::resync_verified_v2_finality_record", "&verified.metadata)?;", "&replacement.metadata)?;", id="resync-original-file"),
    pytest.param("Kura::resync_verified_v2_finality_record", "file.sync_all()", "other.sync_all()", id="resync-file-barrier"),
    pytest.param("Kura::resync_verified_v2_finality_record", "Self::sync_bound_progress_intent_directories(&namespace)", "Self::sync_bound_progress_intent_directories(&other)", id="resync-directory-barriers"),
    pytest.param("Kura::resync_verified_v2_finality_record", "readback.bytes != verified.bytes", "false", id="resync-exact-bytes"),
    pytest.param("Kura::resync_verified_v2_finality_record", "readback.bytes_hash != verified.bytes_hash", "false", id="resync-exact-bytes-hash"),
    pytest.param("Kura::resync_verified_v2_finality_record", "!Self::stable_sidecar_file_binding_unchanged(&verified.metadata, &readback.metadata)", "false", id="resync-path-file-identity"),
    pytest.param("Kura::resync_verified_v2_finality_record", "!Self::sidecar_file_metadata_unchanged(&verified.metadata.file, &opened)", "false", id="resync-open-descriptor-identity"),
    pytest.param("Kura::resync_verified_v2_finality_record", "!self.bound_progress_namespace_unchanged(&namespace)", "false", id="resync-original-namespace"),
    pytest.param("Kura::store_v2_finality_artifact", "let _canonical_chain_guard = self.canonical_chain_lock.lock();", "let _canonical_chain_guard = other.canonical_chain_lock.lock();", id="finality-original-canonical-guard"),
    pytest.param("Kura::store_v2_finality_artifact", "self.verify_v2_finality_crypto(artifact)?;", "self.verify_v2_finality_crypto(artifact).ok();", id="finality-authenticate-before-write"),
])
def test_live_kura_durability_both_consumers_reject_identity_or_barrier_substitution(generic_fixture, symbol, old, new):
    root, helper, checker, _ = generic_fixture
    helper.replace_once_after(root / checker.native_preparation_contract.KURA, "fn " + symbol.rsplit("::", 1)[-1] + "(", old, new)
    assert any("source-binding token" in error for error in generic_native_errors(generic_fixture))
    errors = validate(generic_fixture)
    assert any("executable relation" in error or "Native live durability" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("branch", ["existing", "no-clobber"])
@pytest.mark.parametrize("operation", ["skip-authentication", "skip-resync", "early-receipt", "late-resync"])
def test_live_kura_durability_each_existing_finality_branch_gates_receipt(fixture, branch, operation):
    root, helper, checker, _ = fixture
    path = root / checker.native_preparation_contract.KURA
    anchor = ("if let Some((existing, read_identity)) = self.decode_v2_finality_record_at(&path, &dir)?"
              if branch == "existing" else "if !self.write_atomic_synced_noclobber(&path, &bytes)?")
    verify = "self.verify_v2_finality_artifact_at(&path, &dir, &existing.artifact, &read_identity)?;"
    resync = "self.resync_verified_v2_finality_record(&path, &dir, &read_identity)?;"
    if operation == "skip-authentication":
        helper.replace_once_after(path, anchor, verify, "// authentication removed")
    elif operation == "skip-resync":
        helper.replace_once_after(path, anchor, resync, "// synchronization removed")
    elif operation == "early-receipt":
        helper.replace_once_after(path, anchor, resync, "return Ok(v2_commit_receipt(&existing.artifact));\n            " + resync)
    else:
        source = path.read_text();start = source.index(anchor, source.index("pub fn store_v2_finality_artifact("));tail=source[start:]
        tail=tail.replace(resync, "", 1).replace("return Ok(v2_commit_receipt(&existing.artifact));", "return Ok(v2_commit_receipt(&existing.artifact));\n            " + resync, 1)
        path.write_text(source[:start] + tail)
    assert any("Native live durability Kura::store_v2_finality_artifact" in error for error in validate(fixture))


@pytest.mark.parametrize("cut", ["manifest-binding-before-sync", "manifest-binding-before-recheck", "resync-return-before-sync"])
def test_live_kura_durability_rejects_receipt_or_binding_before_barrier(fixture, cut):
    root, _, checker, _ = fixture
    path = root / checker.native_preparation_contract.KURA
    source = path.read_text()
    if cut.startswith("manifest-"):
        start = source.index("pub(crate) fn store_commit_manifest(")
        moving = "        self.bind_wsv_checkpoint_to_manifest(&manifest)?;\n"
        before = ("        Self::sync_bound_progress_intent_directories(&namespace)"
                  if cut == "manifest-binding-before-sync" else "        if !self.bound_progress_namespace_unchanged(&namespace)")
        tail = source[start:];assert tail.count(moving)==1
        tail = tail.replace(moving, "", 1).replace(before, moving + before, 1)
    else:
        start = source.index("fn resync_verified_v2_finality_record(")
        tail = source[start:]
        tail = tail.replace("        file.sync_all()", "        return Ok(());\n        file.sync_all()", 1)
    path.write_text(source[:start] + tail)
    assert any("Native live durability" in error for error in validate(fixture))

@pytest.mark.parametrize("owner,symbol,old,new", [
    ("BLOCK", "prepare_native_candidate", "PreparedCarrier::prepare(execution)", "PreparedCarrier::prepare(other_execution)"),
    ("BLOCK", "validate_and_record_native_candidate", "source.record_execution(body, context)?", "source.record_execution(body, other_context)?"),
    ("BLOCK", "validate_and_record_native_candidate", "anchor.snapshot_block_hash", "anchor.other_block_hash"),
    ("CONTROLS", "prepare_native_execution_controls", "anchor.snapshot_block_hash", "anchor.other_block_hash"),
    ("CONTROLS", "VerifiedReplayProposal::new", "Hash::new(&wire) != commitment.executed_block_wire_hash", "false"),
    ("CONTROLS", "VerifiedReplayProposal::validate", "Hash::new(&wire) != self.proposal_wire_hash", "false"),
    ("CONTROLS", "validate_sumeragi_v2_replay_keep_voting_block", "authority.validate(&proposal, &state.query_view())?;", "unchecked_prefix(&proposal, &state.query_view())?;"),
    ("CONTROLS", "validate_sumeragi_v2_replay_keep_voting_block", ".eq(frozen.roster.iter().map(|entry| &entry.validator))", ".eq(other_roster.iter())"),
    ("CONTROLS", "validate_sumeragi_v2_replay_keep_voting_block", "record.context == *frozen && record.validator_set_pops == verified.validator_set_pops", "true"),
    ("CONTROLS", "validate_sumeragi_v2_replay_keep_voting_block", "VerifiedHeightContext::snapshot_bootstrap(bootstrap)", "VerifiedHeightContext::snapshot_bootstrap(other_bootstrap)"),
    ("CONTROLS", "validate_sumeragi_v2_replay_keep_voting_block", "frozen.height.checked_sub(1)", "frozen.height.checked_sub(2)"),
    ("CONTROLS", "validate_sumeragi_v2_replay_keep_voting_block", "&parent, &receipt, &parent.validator_set_pops", "&parent, &other_receipt, &parent.validator_set_pops"),
    ("CONTROLS", "validate_sumeragi_v2_replay_keep_voting_block", "prepare_proposed_native_lane_batch_source(&proposal, &[])", "prepare_proposed_native_lane_batch_source(&other_proposal, &[])"),
    ("CONTROLS", "validate_sumeragi_v2_replay_keep_voting_block", "native: input.native", "native: None"),
    ("STATE", "replay_blocks_from_kura_range_inner", "!native.retains_carrier(replay.valid.as_ref(), &finality.height_context)", "!native.retains_carrier(other.as_ref(), &finality.height_context)"),
    ("STATE", "replay_blocks_from_kura_range_inner", "replay.state.staged_merge_entry()", "None"),
    ("STATE", "replay_blocks_from_kura_range_inner", "if replayed_execution_commitment != finality.commit_qc.execution_commitment", "if false"),
    ("STATE", "replay_blocks_from_kura_range_inner", ".authorize_execution_output_publication(&committed_block, &witness)", ".authorize_execution_output_publication(&other_block, &witness)"),
    ("STATE", "replay_blocks_from_kura_range_inner", ".apply_without_execution_with_verified_v2_finality_for_replay(&committed_block)", ".apply_without_execution_with_verified_v2_finality_for_replay(&other_block)"),
    ("STATE", "replay_blocks_from_kura_range_inner", "if actual != wsv_checkpoint.state_hash()", "if false"),
    ("STATE", "replay_blocks_from_kura_range_inner", "!native.retains_carrier(committed_block.as_ref(), &finality.height_context)", "!native.retains_carrier(other.as_ref(), &finality.height_context)"),
    ("STATE", "verify_replay_bootstrap_binding", "if live_state_hash != anchor.snapshot_state_hash", "if false"),
])
def test_native_replay_both_consumers_reject_exact_source_substitution(generic_fixture, owner, symbol, old, new):
    root, helper, checker, _ = generic_fixture
    path = root / getattr(checker.native_preparation_contract, owner)
    if "::" in symbol:
        qualified = symbol
        errors = []
        item = checker._rust_binding_item(root, str(path.relative_to(root)), "method", qualified, "Native replay mutation", errors)
        assert not errors and item is not None and old in item
        text = path.read_text()
        assert text.count(item) == 1
        path.write_text(text.replace(item, item.replace(old, new, 1), 1))
    else:
        helper.replace_once_after(path, "fn " + symbol, old, new)
    assert validate(generic_fixture), (symbol, old)
    assert generic_native_errors(generic_fixture), (symbol, old)


@pytest.mark.parametrize("site,field", [(site, field) for site in (0, 1) for field in ("presence", "state")])
def test_native_replay_both_tail_checks_retain_custody(fixture, site, field):
    root, _, checker, _ = fixture
    path = root / checker.native_preparation_contract.STATE
    old = ("replay.native.is_some() != has_native_inputs" if field == "presence" else "!native.retains_state(&replay.state)")
    text = path.read_text()
    start = text.index("fn replay_blocks_from_kura_range_inner(")
    head, tail = text[:start], text[start:]
    assert tail.count(old) == 2
    index = tail.index(old) if site == 0 else tail.rindex(old)
    tail = tail[:index] + "false" + tail[index + len(old):]
    path.write_text(head + tail)
    assert any("Native replay" in error for error in validate(fixture))


@pytest.mark.parametrize("case", ["drop-order", "early-drop", "live-capture", "source-refusal", "observation-refusal"])
def test_native_replay_rejects_custody_or_phase_shortcuts(fixture, case):
    root, helper, checker, _ = fixture
    c = checker.native_preparation_contract
    if case == "drop-order":
        path = root / c.BLOCK
        old = "pub(crate) state: Box<StateBlock<'state>>,\n    pub(crate) native: Option<crate::state::NativeExecutionCustody>,"
        new = "pub(crate) native: Option<crate::state::NativeExecutionCustody>,\n    pub(crate) state: Box<StateBlock<'state>>,"
        helper.replace_once_after(path, "struct ValidatedReplayExecution", old, new)
    elif case == "early-drop":
        path = root / c.STATE
        text = path.read_text()
        assert text.count("drop(replay.native);") == 1
        text = text.replace("drop(replay.native);", "", 1)
        old = "replay.state.commit().map_err(|err|"
        assert text.count(old) == 1
        path.write_text(text.replace(old, "drop(replay.native); " + old, 1))
    elif case == "live-capture":
        helper.replace_once_after(root / c.CONTROLS, "fn validate_sumeragi_v2_replay_keep_voting_block", "Ok(ValidatedReplayExecution {", "PreparedCarrier::prepare(input)?; Ok(ValidatedReplayExecution {")
    else:
        path = root / c.CONTROLS
        reason = ("Native replay first input body is unavailable" if case == "source-refusal" else "Native replay pre-State observation changed")
        old = 'return Err(Self::execution_context_error("' + reason + '"));'
        helper.replace_once_after(path, "fn validate_sumeragi_v2_replay_keep_voting_block", old, "return Ok(unchecked_execution());")
    assert any("Native replay" in error for error in validate(fixture))


@pytest.mark.parametrize("replacement", ["false", "bundle.merge_entry.is_some()"])
def test_native_replay_custody_presence_comes_from_actual_native_batch(generic_fixture, replacement):
    root, helper, checker, _ = generic_fixture
    path = root / checker.native_preparation_contract.STATE
    helper.replace_once_after(path, "let has_native_inputs = signed_block", "bundle.native_lane_decisions.is_some()", replacement)
    assert validate(generic_fixture)
    assert generic_native_errors(generic_fixture)


@pytest.mark.parametrize("anchor,old,new", [
    ("let native_amx_manifest =", "replay.state.staged_merge_entry()", "None"),
    ("let lane_finality_manifest =", "replay.valid.as_ref()", "other_valid.as_ref()"),
    ("let replayed_execution_commitment =", "&witness,", "&other_witness,"),
    ("let replayed_execution_commitment =", "&native_amx_manifest,", "&other_native_manifest,"),
    ("let replayed_execution_commitment =", "&lane_finality_manifest,", "&other_lane_manifest,"),
    ("let replayed_execution_commitment =", "replay.valid.as_ref()", "other_valid.as_ref()"),
    ("let result_check = ensure_replayed_results_match_committed(", "&signed_block,", "&other_signed_block,"),
])
def test_native_replay_tail_binds_exact_manifest_witness_and_wire(generic_fixture, anchor, old, new):
    root, helper, checker, _ = generic_fixture
    path = root / checker.native_preparation_contract.STATE
    # These names occur in other functions too; isolate the defining replay tail.
    text = path.read_text(); offset = text.index("fn replay_blocks_from_kura_range_inner(")
    index = text.index(anchor, offset); position = text.index(old, index)
    path.write_text(text[:position] + new + text[position + len(old):])
    assert validate(generic_fixture)
    assert generic_native_errors(generic_fixture)


@pytest.mark.parametrize("path,symbol,old,new", [
    ('crates/iroha_core/src/state/block_hashes_admission.rs', 'StateAdmissionError::release_wait', 'Self::Membership(e) => e.release_wait()', 'Self::Membership(e) => None'),
    ('crates/iroha_core/src/state/block_hashes_admission.rs', 'StateBlockStartError::release_wait', 'Self::Membership(error) => error.release_wait()', 'Self::Membership(error) => None'),
    ('crates/iroha_core/src/state/storage_transactions/history.rs', 'MembershipAdmissionError::release_wait', 'Self::Capacity(AllocationRefusal::Capacity { release, .. }) => Some(release)', 'Self::Capacity(AllocationRefusal::Capacity { release, .. }) => None'),
    ('crates/iroha_core/src/state/canonical_runtime.rs', 'State::acquire_canonical_runtime_block', 'self.transactions.prepare_next_block(replacement)?', 'self.transactions.prepare_next_block(false)?'),
    ('crates/iroha_core/src/state/canonical_runtime.rs', 'State::acquire_canonical_runtime_block', 'pending.retain_refused_membership();', 'drop(pending);'),
    ('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', 'RuntimeBlockAcquisition::initialize', '.attach_prepared(&mut self.membership)?', '.attach_prepared(&mut None)?'),
    ('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', 'RuntimeBlockAcquisition::retain_refused_membership', 'self.release();', '// skip original sibling unlock'),
    ('crates/iroha_core/src/state/canonical_runtime/acquisition.rs', 'RuntimeBlockAcquisition::retain_refused_membership', 'self.target.transactions.retain_preparation(original);', 'self.target.transactions.retain_preparation(other);'),
    ('crates/iroha_core/src/state/storage_transactions/history.rs', 'TransactionsStorage::prepare_next_block', 'if guard.loaned.load(Ordering::Acquire)', 'if false'),
    ('crates/iroha_core/src/state/storage_transactions/history.rs', 'TransactionsStorage::prepare_next_block', '!Identity::ptr_eq(&p.predecessor, &guard) || p.replacement != replacement', 'false'),
    ('crates/iroha_core/src/state/storage_transactions/history.rs', 'TransactionsStorage::prepare_next_block', 'ready.lease_release = Some(guard.release_deferred(drop).1);', 'drop(guard);'),
    ('crates/iroha_core/src/state/storage_transactions/history.rs', 'TransactionsStorage::retain_preparation', 'Identity::ptr_eq(&original.predecessor, &guard)', 'true'),
    ('crates/iroha_core/src/state/storage_transactions/history.rs', 'TransactionsStorage::retain_preparation', 'original.leased && pending.is_none()', 'true'),
    ('crates/iroha_core/src/state/storage_transactions/history.rs', 'TransactionsStorage::retain_preparation', 'drop(guard);\n            drop(stale);', 'drop(stale);\n            drop(guard);'),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'TransactionsStorage::attach_prepared', 'if !Identity::ptr_eq(&guard, &original.predecessor)', 'if false'),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'TransactionsStorage::attach_prepared', '.read_predecessor(original.baseline.as_ref().expect("original history cut"))', '.read_predecessor(other.baseline.as_ref().expect("original history cut"))'),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'TransactionsStorage::attach_prepared', 'Some(history_slot::Slot::new(self, original))', 'None'),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'TransactionsStorage::view_retaining', 'if before == self.publication_sequence.load(Ordering::Acquire)', 'if true'),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'TransactionsStorage::view_retaining', 'self.blocks.read_retaining(releases)?', 'self.blocks.read_retaining(other)?'),
    ('crates/iroha_core/src/state/history_reader_releases.rs', 'StateViewReleases::new', 'lifecycle: LaneLifecycleReleases::new(state)', 'lifecycle: LaneLifecycleReleases::new(other)'),
    ('crates/iroha_core/src/state/history_reader_releases.rs', 'StateViewReleases::try_view_once', '&mut self.lifecycle.membership', '&mut other.membership'),
    ('crates/iroha_core/src/state/history_reader_releases.rs', 'StateViewReleases::try_view_once', '&mut self.lifecycle.hashes', '&mut other.hashes'),
    ('crates/iroha_core/src/state/history_reader_releases.rs', 'BlockHashes::view_retaining', 'map.read_retaining(releases)', 'map.read_retaining(other)'),
    ('crates/iroha_core/src/state.rs', 'State::try_view_once_with_index_releases', 'block_hashes.last().copied()', 'self.latest_block_hash_fast()'),
    ('crates/iroha_core/src/state.rs', 'State::try_view_once_with_index_releases', '.view_retaining(membership)', '.view_retaining(other)'),
    ('crates/iroha_core/src/sumeragi/v2_apply.rs', 'V2ApplyService::classify_validation_failure', 'BlockValidationError::MembershipAdmission(error) => Some((', 'BlockValidationError::MembershipAdmission(error) if false => Some(('),
    ('crates/iroha_core/src/sumeragi/v2_apply.rs', 'V2ApplyService::classify_validation_failure', 'BodyValidationBusy::new(owner, wait.clone(), self.queue.sumeragi_waker())', 'BodyValidationBusy::new(owner, other.clone(), self.queue.sumeragi_waker())'),
    ('crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'classify_merge_state_validation', '| crate::state::MergeLedgerCommitError::MembershipAdmission(_)', ''),
    ('vendor/concread/src/release.rs', 'PoisonPolicy::observe', 'Self::Observed(flag) => flag.load(Ordering::Acquire)', 'Self::Observed(flag) => false'),
    ('vendor/concread/src/release.rs', 'ReleaseGuard::release_deferred', 'let mut retirement = self.release_retaining(release);\n        // Actual primitive poison is established when its guard is released.\n        let poisoned = policy.observe();', 'let poisoned = policy.observe();\n        let mut retirement = self.release_retaining(release);'),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'LinCowCell::try_acquire_owned', 'self.acquire_owned(owned, false)', 'self.acquire_owned(owned, true)'),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'LinCowCell::acquire_owned', 'self.write.try_lock_retained()', 'other.try_lock_retained()'),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'LinCowCellCommitSlot::try_prepare', 'writer.caller.try_lock_active_retained()?', 'writer.caller.try_lock_active()?'),
    ('vendor/concread/src/bptree/admission.rs', 'BptreeMapWriterAcquisition::try_write_admitted', 'self.write_with_source(|_, demand| admit(demand).map_err(MapAdmissionError::Refused))', 'self.write_with_source(|_, demand| other(demand).map_err(MapAdmissionError::Refused))'),
    ('vendor/concread/src/bptree/admission.rs', 'BptreeMapWriterAcquisition::write_with_source', 'Prepaid(Some(admit(source, plan.demand)?))', 'Prepaid(Some(admit(other, plan.demand)?))'),
])
def test_current_membership_preparation_preserves_original_authority(fixture, path, symbol, old, new):
    root, _, checker, _ = fixture
    errors = []
    kind = "method" if "::" in symbol else "fn"
    item = checker._rust_binding_item(root, path, kind, symbol, "membership mutation", errors)
    assert item is not None and errors == []
    assert item.count(old) == 1, (symbol, old)
    target = root / path
    source = target.read_text()
    assert source.count(item) == 1
    target.write_text(source.replace(item, item.replace(old, new, 1), 1))
    errors = validate(fixture)
    assert any("executable relation" in error or "retained carrier" in error for error in errors), errors
    assert not any("must have one" in error or "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize("anchor,old,new", [
    ("fn retain_captured_world_field", "field.take()", "foreign_field.take()"),
    ("fn retain_captured_world_field", "slot.retain(name, target)", "slot.retain(name, foreign_target)"),
    ("fn retain_captured_world_field", "slot.retain(name, target)", 'slot.retain("foreign", target)'),
    ("macro_rules! retain_field", "&mut $pending.$field", "&mut foreign.$field"),
    ("macro_rules! retain_field", "stringify!($field)", '"foreign"'),
])
def test_world_materialization_preserves_original_field_owner(fixture, anchor, old, new):
    """Each concrete extraction keeps its exact captured slot and target."""
    root, helper, _, _ = fixture
    helper.replace_once_after(root / "crates/iroha_core/src/state/world_journals.rs", anchor, old, new)
    errors = validate(fixture)
    assert any("missing executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


def test_world_materialization_requires_one_isolated_stack_frame(fixture):
    """Inlining the helper would recreate all-field stack accumulation."""
    root, helper, _, _ = fixture
    helper.replace_once(root / "crates/iroha_core/src/state/world_journals.rs",
                        "#[inline(never)]\nfn retain_captured_world_field",
                        "#[inline(always)]\nfn retain_captured_world_field")
    assert any("must retain its isolated stack frame" in error for error in validate(fixture))


def test_world_materialization_requires_helper_ledger_owner(fixture):
    """The moved implementation remains mandatory alongside its macro caller."""
    _, _, checker, models = fixture
    model = next(m for m in models if m["module"] == checker.native_preparation_contract.MODEL)
    model["production_symbols"] = [row for row in model["production_symbols"]
                                 if row["symbol"] != "retain_captured_world_field"]
    assert any("ledger owner retain_captured_world_field" in error for error in validate(fixture))


@pytest.mark.parametrize("old,new", [
    (".stack_size(2 * 1024 * 1024)", ".stack_size(4 * 1024 * 1024)"),
    ("assert_native_service_body_store_retention(native_service_retention_fixture(false));", "let _ = native_service_retention_fixture(false);"),
    (".join()", ";"),
])
def test_world_materialization_keeps_real_ordinary_stack_regression(fixture, old, new):
    """The regression must execute real retention on the fixed ordinary budget."""
    root, helper, checker, _ = fixture
    path, _ = checker.native_preparation_contract.WORLD_MATERIALIZATION_STACK_TEST
    helper.replace_once_after(root / path,
        "state_test! { sync native_service_body_store_retention_uses_ordinary_stack", old, new)
    assert any("requires the exact ordinary-stack regression" in error for error in validate(fixture))
