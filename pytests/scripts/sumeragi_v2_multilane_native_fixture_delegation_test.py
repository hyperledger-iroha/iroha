"""Scoped semantic controls for real Native fixture dispatch and constructors.

The indexed full gate separately owns include/provider authentication. These
controls parse the actual Rust owners and mutate only their exact source joins.
"""
from __future__ import annotations

import importlib.util
import json
import shutil
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
FORMAL = ROOT / "scripts/formal"
sys.path.insert(0, str(FORMAL))
import sumeragi_v2_multilane_native_merge_manifest_contract as native

FIXTURE = native.NATIVE_MERGE_MANIFEST_FIXTURE_RELATIVE
CORRIDOR = native.NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE
CONSTRUCTOR = "ApplyFixture::new_with_options_and_retention_and_genesis_and_archival_kura"


def checker():
    spec = importlib.util.spec_from_file_location("native_fixture_checker", FORMAL / "check_sumeragi_v2_multilane_models.py")
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def bindings():
    return tuple(b for b in native.NATIVE_TYPED_SETTLEMENT_SOURCE_BINDINGS if b[0] == FIXTURE.as_posix())


def validate(root):
    module = checker()
    items = {}
    errors = []
    for path, kind, symbol, tokens in bindings():
        found = module._extract_rust_binding_items((root / path).read_text(), kind, symbol)
        assert len(found) == 1
        items[path, kind, symbol] = found[0]
        for token in tokens:
            if token not in found[0]:
                errors.append(f"{symbol}: missing constructor token {token}")
    symbol = "ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle"
    items[FIXTURE.as_posix(), "method", symbol] = module._extract_rust_binding_items(
        (root / FIXTURE).read_text(), "method", symbol)[0]
    native.validate_native_merge_manifest_relations(root, items, errors)
    return errors


@pytest.fixture
def fixture(tmp_path):
    for relative in (FIXTURE, CORRIDOR):
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, target)
    assert validate(tmp_path) == []
    return tmp_path


def test_native_fixture_delegation_accepts_actual_source_and_exact_ledger(fixture):
    assert validate(fixture) == []
    ledger = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    rows = next(m for m in ledger["models"] if m["module"] == "SumeragiV2NativeApplicationEvidence")["production_symbols"]
    for path, kind, symbol, tokens in (*bindings(), native.NATIVE_MERGE_MANIFEST_CORRIDOR_HELPER_BINDING):
        assert [r for r in rows if (r["path"], r["kind"], r["symbol"]) == (path, kind, symbol)] == [
            dict(path=path, kind=kind, symbol=symbol, required_tokens=list(tokens))]
    assert Path(__file__).relative_to(ROOT) in native.NATIVE_MERGE_MANIFEST_SOURCE_RELATIVES


@pytest.mark.parametrize("owner,old,new", [
    ("new_with_options", "            include_native_lane,", "            false,"),
    ("new_with_options", "            include_lane_lifecycle,", "            false,"),
    ("new_with_options_and_retention", "            include_lane_payload,", "            false,"),
    ("new_with_options_and_retention", "            include_projection_policies,", "            false,"),
    ("new_with_options_and_retention", "            include_native_lane,", "            false,"),
    ("new_with_options_and_retention", "            blocks_in_memory,", "            other_limit,"),
    ("new_with_options_and_retention", "            false,", "            true,"),
    ("new_with_options_and_retention_and_genesis", "            seed_genesis_domain,", "            false,"),
    ("new_with_options_and_retention_and_genesis", "            include_lane_lifecycle,", "            false,"),
    ("new_with_options_and_retention_and_genesis", "            None,", "            Some(other_kura),"),
    (CONSTRUCTOR.split("::")[1], "(1_u8..=4)", "(1_u8..=3)"),
    (CONSTRUCTOR.split("::")[1], "assert!(include_native_lane && include_lane_lifecycle);", "assert!(include_native_lane || include_lane_lifecycle);"),
    (CONSTRUCTOR.split("::")[1], "else if include_lane_lifecycle {", "else if false {"),
    (CONSTRUCTOR.split("::")[1], "            context.network_id,", "            other_network_id,"),
    (CONSTRUCTOR.split("::")[1], "install_fixture_validator_authority(&state, &context, &validator_set_pops);", "let _ = &validator_set_pops;"),
    (CONSTRUCTOR.split("::")[1], "install_fixture_native_lane(&mut state, &mut context);", "let _ = &mut context;"),
    ("new_for_production_recovered_decision_apply_with_native_lane_lifecycle", "Self::new_with_options(false, false, true, true)", "Self::new_with_options(false, false, true, false)"),
])
def test_native_fixture_constructor_rejects_argument_or_owner_substitution(fixture, owner, old, new):
    path = fixture / FIXTURE
    text = path.read_text()
    start = text.index(f"fn {owner}(")
    assert old in text[start:]
    path.write_text(text[:start] + text[start:].replace(old, new, 1))
    assert validate(fixture)


@pytest.mark.parametrize("anchor,old,new", [
    ("historical_autonomous_recovery_reaches_exactly_once_canonical_merge_application", "run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::HistoricalRecovery);", "run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::SuccessfulApply);"),
    ("historical_autonomous_recovery_reaches_exactly_once_canonical_merge_application", "run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::HistoricalRecovery);", "if false { run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::HistoricalRecovery); }"),
    ("fn run_autonomous_merge_frontier_fixture", "frontier_case == MergeFrontierFixtureCase::StartupRegistryBoundaries", "frontier_case == MergeFrontierFixtureCase::HistoricalRecovery"),
    ("fn run_autonomous_merge_frontier_fixture", "ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle()", "ApplyFixture::new_for_production_recovered_decision_apply_with_lane_lifecycle()"),
    ("fn run_autonomous_merge_frontier_fixture", "frontier_case == MergeFrontierFixtureCase::SuccessfulApply", "frontier_case == MergeFrontierFixtureCase::HistoricalRecovery"),
    ("fn run_autonomous_merge_frontier_fixture", "for _ in 0..4 {", "for _ in 0..3 {"),
    ("fn run_autonomous_merge_frontier_fixture", '"planned merge association authorizes exact Native startup repair"', '"unverified repair"'),
    ("fn run_autonomous_merge_frontier_fixture", "assert!(empty_plan.is_empty())", "assert!(!empty_plan.is_empty())"),
])
def test_native_fixture_dispatch_and_retained_assertions_reject_substitution(fixture, anchor, old, new):
    path = fixture / CORRIDOR
    text = path.read_text()
    start = text.index(anchor)
    assert old in text[start:]
    path.write_text(text[:start] + text[start:].replace(old, new, 1))
    assert any("Native corridor macro test" in e for e in validate(fixture))
