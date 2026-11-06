"""Mutation tests for frozen Exact12 and additive Ironwood source inventories."""
from __future__ import annotations

import copy
import importlib.util
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("privacy_wire_inventory", ROOT / "scripts/check_zk_operation_inventory.py")
assert SPEC and SPEC.loader
C = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = C
SPEC.loader.exec_module(C)
FROZEN = "crates/iroha_data_model/src/privacy/exact12_wire.rs"
RUNTIME = "crates/iroha_data_model/src/privacy.rs"
ISI = "crates/iroha_data_model/src/isi/privacy.rs"


def check(*, overrides=None, inventory=None):
    inventory = copy.deepcopy(C.load() if inventory is None else inventory)
    checker = C.Checker(inventory, C.Tree(overrides=overrides or {}))
    checker.check_exact12()
    checker.check_privacy_wire_separation()
    checker.check_action_roles()
    return checker.errors


def test_current_baseline_and_extension_are_both_fully_inventoried():
    assert check() == []


@pytest.mark.parametrize("original,runtime", [
    ("PrivacyProofEnvelopeV1", "PrivacyRuntimeProofEnvelopeV1"),
    ("PrivacyProtocolLimitsV1", "PrivacyRuntimeProtocolLimitsV1"),
    ("PrivacyRootPublicationV1", "PrivacyRuntimeRootPublicationV1"),
    ("PrivacyPgcAccountBootstrapV1", "PrivacyRuntimePgcAccountBootstrapV1"),
])
def test_runtime_union_cannot_replace_a_frozen_payload(original, runtime):
    source = (ROOT / ISI).read_text()
    needle = ": " + original + ","
    assert needle in source
    errors = check(overrides={ISI: source.replace(needle, ": " + runtime + ",")})
    assert any("privacy.wire_separation" in error for error in errors), errors


def test_extension_cannot_be_added_to_frozen_protocol_grammar():
    source = (ROOT / FROZEN).read_text()
    needle = 'pub enum PrivacyProtocolIdV1 {'
    errors = check(overrides={FROZEN: source.replace(needle, needle + '\n    IronwoodHalo2ActionsV1,')})
    assert any("frozen twelve variants" in error for error in errors), errors


def test_unrecorded_runtime_protocol_is_still_rejected():
    source = (ROOT / RUNTIME).read_text()
    needle = 'pub enum PrivacyRuntimeProtocolIdV1 {'
    errors = check(overrides={RUNTIME: source.replace(needle, needle + '\n    UnknownRuntimeV1,')})
    assert any("privacy.runtime" in error for error in errors), errors


def test_dedicated_ironwood_payload_cannot_reuse_frozen_envelope():
    source = (ROOT / ISI).read_text()
    needle = ': PrivacyIronwoodProofEnvelopeV1,'
    assert needle in source
    errors = check(overrides={ISI: source.replace(needle, ': PrivacyProofEnvelopeV1,')})
    assert any("SubmitPrivacyIronwoodProofV1.envelope" in error for error in errors), errors


def test_removing_extension_inventory_does_not_hide_runtime_support():
    inventory = C.load()
    inventory["exact12"]["runtime_extensions"] = []
    assert any("privacy.runtime" in error for error in check(inventory=inventory))
