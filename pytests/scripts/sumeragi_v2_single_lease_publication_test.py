"""Actual-owner and mutation controls for continuous Kura publication custody."""
from __future__ import annotations

import ast
import copy
import importlib.util
import json
from pathlib import Path
import sys

import pytest

SOURCE_ROOT = Path(__file__).resolve().parents[2]
# An overlaid candidate may live in this checkout's ignored evidence directory.
# Always authenticate Rust through the real checkout, never a symlinked fixture.
ROOT = next(path for path in Path(__file__).resolve().parents if (path / ".git").exists())


@pytest.fixture(scope="module")
def contract():
    directory = SOURCE_ROOT / "scripts/formal"
    sys.path.insert(0, str(directory))
    name = "sumeragi_v2_multilane_native_preparation_contract"
    spec = importlib.util.spec_from_file_location(name, directory / (name + ".py"))
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def checker(contract):
    path = ROOT / "scripts/formal/check_sumeragi_v2_multilane_models.py"
    spec = importlib.util.spec_from_file_location("single_lease_publication_checker", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    assert module.native_preparation_contract is contract
    return module


@pytest.fixture(scope="module")
def rows(contract):
    ledger = json.loads((SOURCE_ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    model, = [model for model in ledger["models"] if model["module"] == contract.MODEL]
    return model["production_symbols"]


@pytest.fixture(scope="module")
def items(checker, contract):
    errors, items = [], {}
    with checker._reviewed_rust_source_cache():
        for path, kind, symbol, _ in contract.SINGLE_LEASE_OWNER_BINDINGS:
            items[path, kind, symbol] = checker._rust_binding_item(
                ROOT, path, kind, symbol, "single-lease actual owner", errors,
            )
    assert not errors, errors
    return items


def validate(contract, rows, items):
    """Use the same defining-item checker as the production contract consumer."""
    errors = []
    contract._validate_single_lease_publication_contract(
        ROOT, rows, errors, lambda _root, path, kind, symbol, _label, _errors: items[path, kind, symbol],
    )
    return errors


def test_single_lease_contract_accepts_actual_owners_and_ledger(checker, contract, rows):
    errors = []
    with checker._reviewed_rust_source_cache():
        contract._validate_single_lease_publication_contract(ROOT, rows, errors, checker._rust_binding_item)
    assert not errors, errors


def test_single_lease_contract_is_called_by_actual_native_gate(contract):
    tree = ast.parse(Path(contract.__file__).read_text())
    owner, = [node for node in tree.body if isinstance(node, ast.FunctionDef)
              and node.name == "validate_native_preparation_contract"]
    calls = [node for node in ast.walk(owner) if isinstance(node, ast.Call)
             and isinstance(node.func, ast.Name)
             and node.func.id == "_validate_single_lease_publication_contract"]
    assert len(calls) == 1
    assert [argument.id for argument in calls[0].args] == ["root", "rows", "errors", "rust_binding_item"]
    assert set(contract.SINGLE_LEASE_OWNER_BINDINGS) <= set(contract.PREPARATION_OWNER_BINDINGS)
    assert all(Path(path) in contract.NATIVE_PREPARATION_SOURCE_RELATIVES
               for path, _, _, _ in contract.SINGLE_LEASE_OWNER_BINDINGS)


MUTATIONS = (
    ("SourceAuthenticatedCarrier", "struct SourceAuthenticatedCarrier", "pub struct SourceAuthenticatedCarrier"),
    ("SourceAuthenticatedCarrier::try_prepare", "fn try_prepare(", "pub(crate) fn try_prepare("),
    ("try_prepare_physical", "target.matches_kura_instance(&original.journals.kura)", "target.matches_kura_instance(&foreign)"),
    ("try_prepare_physical", ".matches_publication_target(target, original.block().header())", ".matches_publication_target(target, foreign.header())"),
    ("try_prepare_physical", "None => Some(CarrierQueueRetirementError::Missing)", "None => None"),
    ("try_prepare_physical", "Some(source) if !source.belongs_to(target)", "Some(source) if false"),
    ("try_prepare_physical", "SourceAuthenticatedCarrier::try_new(original, kura)?", "SourceAuthenticatedCarrier::try_new(foreign, kura)?"),
    ("try_prepare_physical", "authenticated.try_prepare(target, queue_source)", "authenticated.try_prepare(foreign, queue_source)"),
    ("try_prepare_physical", "authenticated.try_prepare(target, queue_source)", "let original = authenticated.release(); let kura = target.kura.try_publication_lease()?; SourceAuthenticatedCarrier::try_new(original, kura)?.try_prepare(target, queue_source)"),
    ("SourceAuthenticatedCarrier", "kura: KuraPublicationLease<'target>", "kura: bool"),
    ("SourceAuthenticatedCarrier::try_new", "&original.journals.execution_prefix", "&foreign.execution_prefix"),
    ("SourceAuthenticatedCarrier::try_new", "&original.journals.execution_prefix,\n                    &owner.kura", "&original.journals.execution_prefix,\n                    &foreign_lease"),
    ("SourceAuthenticatedCarrier::try_new", "&original.checkpoint", "&foreign_checkpoint"),
    ("SourceAuthenticatedCarrier::try_new", "original.journals.provider_capture.as_ref()", "None"),
    ("SourceAuthenticatedCarrier::try_new", "original.journals.reputation_capture.as_ref()", "None"),
    ("SourceAuthenticatedCarrier::try_new", "Ok(()) => Ok(owner)", "Ok(()) => Ok(foreign)"),
    ("SourceAuthenticatedCarrier::release", "drop(kura.release_deferred());", "std::mem::forget(kura);"),
    ("SourceAuthenticatedCarrier::try_prepare", "self.decision.publish_execution_witness(&self.kura)", "Ok::<_, CarrierExecutionWitnessPublicationError>(())"),
    ("SourceAuthenticatedCarrier::try_prepare", "self.decision.publish_archives(&self.kura)", "Ok::<_, CarrierArchivePublicationError>(())"),
    ("SourceAuthenticatedCarrier::try_prepare", ".reauthenticate_execution_witness(authenticated.decision.finality.artifact())", ".unchecked_execution_witness(authenticated.decision.finality.artifact())"),
    ("SourceAuthenticatedCarrier::try_prepare", "let authenticated = self;", "drop(self.kura); let authenticated = self;"),
    ("SourceAuthenticatedCarrier::try_prepare", "let authenticated = self;", "let _other = target.kura.try_publication_lease(); let authenticated = self;"),
    ("SourceAuthenticatedCarrier::try_prepare", "let authenticated = self;", "let _discarded = self.release(); let authenticated = self;"),
    ("SourceAuthenticatedCarrier::try_prepare", "let queue_observer = if authenticated", "let _early = StateFences::try_acquire(target); let queue_observer = if authenticated"),
    ("SourceAuthenticatedCarrier::try_prepare", "observer\n                    .try_into_cut()", "observer.try_into_cut().await"),
    ("SourceAuthenticatedCarrier::try_prepare", "drop((state_retirement, queue_retirement, kura_retirement));", "drop(kura_retirement); drop((state_retirement, queue_retirement));"),
    ("SourceAuthenticatedCarrier::try_prepare", "drop((queue_retirement, state_retirement, kura_retirement));", "drop(queue_retirement); drop((state_retirement, kura_retirement));"),
    ("SourceAuthenticatedCarrier::try_prepare", "_kura: kura,", "_kura: foreign_lease,"),
    ("SourceAuthenticatedCarrier::try_prepare", "CarrierPreparation::new(original, target, fences)", "CarrierPreparation::new(original, target, foreign_fences)"),
    ("SourceAuthenticatedCarrier::try_prepare", "preparation.recover_original()", "foreign_journals"),
    ("publish_execution_witness", "&self.checkpoint", "&foreign_checkpoint"),
    ("publish_execution_witness", "self.journals.source_prefix.witness()", "foreign_witness"),
    ("publish_execution_witness", "self.checkpoint.finality_receipt()", "foreign_receipt"),
    ("publish_execution_witness", "lease\n            .publish_execution_witness(", "self.journals.kura.try_publication_lease()?.publish_execution_witness("),
    ("publish_archives", "let receipt = self.checkpoint.finality_receipt();", "drop(lease); let receipt = self.checkpoint.finality_receipt();"),
    ("publish_archives", "self.journals.provider_capture.as_mut()", "None"),
    ("publish_archives", "self.journals.reputation_capture.as_mut()", "None"),
    ("publish_archives", "let receipt = self.checkpoint.finality_receipt();", "let receipt = foreign_receipt;"),
    ("KuraPublicationLease::publish_execution_witness", "self.kura.durable_mutation_authorized()?;", ""),
    ("KuraPublicationLease::publish_execution_witness", ".authenticate_kagemusha_finality_receipt_under_publication_guards(finality, receipt)?;", ".unchecked_finality(finality, receipt)?;"),
    ("KuraPublicationLease::publish_execution_witness", "finality.commit_qc.execution_commitment", "foreign_commitment"),
    ("KuraPublicationLease::publish_execution_witness", ".stage_kagemusha_finality_sidecar_under_sidecar_guard(&staged)?;", ".stage_kagemusha_finality_sidecar(&staged)?;"),
    ("KuraPublicationLease::publish_execution_witness", "self.reauthenticate_execution_witness(finality)", "Ok(())"),
    ("KuraPublicationLease::reauthenticate_execution_witness", "Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)?;", ""),
    ("KuraPublicationLease::reauthenticate_execution_witness", "Kura::stable_sidecar_metadata_unchanged(&read.metadata, current)", "true"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", "receipt.height != artifact.height", "false"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", "receipt.block_hash != artifact.block_hash", "false"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", "receipt.context_id != artifact.context_id()", "false"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", "receipt.subject != artifact.subject", "false"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", "receipt.certificate != artifact.commit_qc.as_ref()", "false"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", "receipt.artifact_hash != HashOf::new(artifact)", "false"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", "durable != *artifact", "false"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", "HashOf::new(&durable) != receipt.artifact_hash", "false"),
    ("Kura::authenticate_kagemusha_finality_receipt_under_publication_guards", ".v2_finality_artifact_with_archive_under_prune_and_canonical_guards(artifact.height)?", ".v2_finality_artifact(artifact.height)?"),
    ("Kura::prepare_kagemusha_finality_sidecar", "validation_fee_root != expected.ordinary_writes_root", "false"),
    ("Kura::prepare_kagemusha_finality_sidecar", "lane_consensus_contexts_witness.carrier_height() != height", "false"),
    ("Kura::prepare_kagemusha_finality_sidecar", "casting_snapshot != rebuilt_casting_snapshot", "false"),
    ("Kura::stage_kagemusha_finality_sidecar_under_sidecar_guard", ".write_atomic_synced_noclobber(&path, &bytes)?", ".write_atomic_synced(&path, &bytes)?"),
    ("Kura::stage_kagemusha_finality_sidecar_under_sidecar_guard", "if &persisted != staged", "if false"),
    ("Kura::stage_kagemusha_finality_sidecar_under_sidecar_guard", "resource_mutation.finish_resources_before_disk_rescan();", ""),
    ("Kura::promote_kagemusha_finality_sidecar_under_sidecar_guard", "Self::validate_kagemusha_finality_sidecar(&final_sidecar, artifact)?;", ""),
    ("Kura::promote_kagemusha_finality_sidecar_under_sidecar_guard", "if persisted != final_sidecar", "if false"),
    ("Kura::promote_kagemusha_finality_sidecar_under_sidecar_guard", "self.remove_exact_staged_kagemusha_finality(&staged_path, &staged_identity)?;", ""),
    ("Kura::promote_kagemusha_finality_sidecar_under_sidecar_guard", "resource_mutation.finish_resources_before_disk_rescan();", ""),
    ("Kura::stage_kagemusha_finality_sidecar", "self.stage_kagemusha_finality_sidecar_under_sidecar_guard(&staged)", "self.parallel_stage_implementation(&staged)"),
)


@pytest.mark.parametrize(("symbol", "old", "new"), MUTATIONS,
                         ids=[f"{row[0]}-{index}" for index, row in enumerate(MUTATIONS)])
def test_single_lease_rejects_custody_or_authentication_mutation(contract, rows, items, symbol, old, new):
    key, = [key for key in items if key[2] == symbol]
    source = items[key]
    assert source.count(old) == 1, (symbol, old, source.count(old))
    changed = {**items, key: source.replace(old, new, 1)}
    errors = validate(contract, rows, changed)
    assert errors, (symbol, old, new)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("operation", ("try_publication_lease()", "release_deferred()", "release()"))
@pytest.mark.parametrize("symbol", ("publish_execution_witness", "publish_archives"))
def test_passed_lease_continuations_reject_extra_lock_boundary(contract, rows, items, symbol, operation):
    key, = [key for key in items if key[2] == symbol]
    source = items[key]
    # Keep every correct call; a stray boundary must independently fail.
    changed = {**items, key: source.replace("{", f"{{ let _other = lease.{operation};", 1)}
    assert validate(contract, rows, changed)


@pytest.mark.parametrize("symbol", ("try_prepare_physical", "SourceAuthenticatedCarrier::try_prepare",
                                    "KuraPublicationLease::publish_execution_witness"))
def test_ledger_cannot_silently_drift_from_single_lease_owners(contract, rows, items, symbol):
    changed = copy.deepcopy(rows)
    owner, = [row for row in changed if row["symbol"] == symbol]
    owner["required_tokens"] = []
    errors = validate(contract, changed, items)
    assert any("reviewed tokens changed" in error for error in errors), errors
