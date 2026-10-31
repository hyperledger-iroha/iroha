"""Mutation controls for the single X509 original-attachment effect owner."""
from __future__ import annotations

import copy
import importlib.util
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("dedicated_request_inventory", ROOT / "scripts/check_zk_operation_inventory.py")
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
SPEC.loader.exec_module(CHECKER)
INVENTORY = CHECKER.load()
SOURCE = CHECKER.Tree()
OWNER = "crates/iroha_core/src/smartcontracts/isi/privacy/x509_submission.rs"
GENERIC = "crates/iroha_core/src/smartcontracts/isi/privacy.rs"


def check(overrides=None, inventory=None):
    inventory = copy.deepcopy(INVENTORY if inventory is None else inventory)
    tree = CHECKER.Tree(SOURCE.root, {**SOURCE.overrides, **(overrides or {})})
    checker = CHECKER.Checker(inventory, tree)
    owners = checker.dedicated_effect_owners(inventory["exact12"]["single_route"], inventory["exact12"]["roles"]["list"])
    return owners, checker.errors


def test_current_dedicated_request_has_one_checked_effect_owner():
    assert check() == ([OWNER], [])


@pytest.mark.parametrize("path,needle,replacement,reason", (
    (GENERIC, "if instruction.envelope.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 {", "if false {", "generic preparation must reject"),
    (GENERIC, "VerifiedPrivacyLedgerEffectsV1::ZkX509Certificate(_) => Err(", "VerifiedPrivacyLedgerEffectsV1::ZkX509Certificate(_) => Ok(", "generic executor must reject"),
    (OWNER, "binding.require_original_execution(request, proof, authority, state_transaction)?;", "let _ = binding.require_original_execution(request, proof, authority, state_transaction);", "must reject a foreign original binding"),
    (OWNER, "let admitted = resolve_admitted_privacy_protocol_v1(", "let admitted = unchecked_protocol_v1(", "selected implementation and policy admission must reject"),
    (OWNER, "if effects.role() != PrivacyActionRoleV1::ZkX509CertificatePresentation", "if false && effects.role() != PrivacyActionRoleV1::ZkX509CertificatePresentation", "verified effect bindings and variant"),
    (OWNER, "|| effects.protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V1", "|| false", "verified effect bindings and variant"),
    (OWNER, "|| effects.statement_digest() != statement_digest", "|| false", "verified effect bindings and variant"),
    (OWNER, "|| effects.action_index() != expected_action_index", "|| false", "verified effect bindings and variant"),
    (OWNER, "|| effects.encoded_action_bytes() != encoded_action_bytes", "|| false", "verified effect bindings and variant"),
    (OWNER, "= effects.into_ledger() else {", "= unchecked_effect() else {", "verified effect bindings and variant"),
    (OWNER, "let effects = verify_privacy_zk_x509_attached_v1(", "let effects = unchecked_native_result(", "selected verifier and effect application must each occur once"),
    (OWNER, "    apply_verified_effect(\n", "    unchecked_effect_application(\n", "selected verifier and effect application must each occur once"),
))
def test_each_original_owner_boundary_is_required(path, needle, replacement, reason):
    source = SOURCE.read(path)
    assert source is not None and needle in source
    _, errors = check({path: source.replace(needle, replacement)})
    assert any(reason in error for error in errors), errors


def test_comment_and_unrelated_owner_cannot_satisfy_original_binding_guard():
    needle = "binding.require_original_execution(request, proof, authority, state_transaction)?;"
    source = SOURCE.read(OWNER)
    assert source is not None and source.count(needle) == 2
    source = source.replace(needle, "// " + needle)
    source += "\nfn unrelated() { " + needle + " }\n"
    _, errors = check({OWNER: source})
    assert sum("must reject a foreign original binding" in error for error in errors) == 2


def test_duplicate_role_or_foreign_owner_is_rejected():
    inventory = copy.deepcopy(INVENTORY)
    request = inventory["exact12"]["single_route"]["dedicated_requests"][0]
    inventory["exact12"]["single_route"]["dedicated_requests"].append(copy.deepcopy(request))
    assert any("must be unique" in error for error in check(inventory=inventory)[1])
    inventory = copy.deepcopy(INVENTORY)
    inventory["exact12"]["single_route"]["dedicated_requests"][0]["path"] = GENERIC
    assert any("effect owner differs" in error for error in check(inventory=inventory)[1])


@pytest.mark.parametrize("stand_in", ("comment", "unrelated_function"))
def test_generic_refusal_must_belong_to_actual_executor_not_a_stand_in(stand_in):
    needle = "VerifiedPrivacyLedgerEffectsV1::ZkX509Certificate(_) => Err("
    source = SOURCE.read(GENERIC)
    assert source is not None and source.count(needle) == 1
    # The source checker must reject the real arm's removal even when another
    # location supplies the exact original text and whole-file arm count.
    changed = source.replace(needle, "_ => Ok(")
    if stand_in == "comment":
        changed += "\n// " + needle + "\n"
    else:
        changed += ("\nfn unrelated_effect_rejection(value: VerifiedPrivacyLedgerEffectsV1) { "
                    "match value { VerifiedPrivacyLedgerEffectsV1::ZkX509Certificate(_) => Err(()), "
                    "_ => Ok(()) } }\n")
    _, errors = check({GENERIC: changed})
    assert any("generic executor must reject this effect" in error for error in errors), errors
    checker = CHECKER.Checker(copy.deepcopy(INVENTORY), CHECKER.Tree(SOURCE.root, {**SOURCE.overrides, GENERIC: changed}))
    checker.check_exact12_single_route()
    assert any("generic executor must reject this effect" in error for error in checker.errors), checker.errors
