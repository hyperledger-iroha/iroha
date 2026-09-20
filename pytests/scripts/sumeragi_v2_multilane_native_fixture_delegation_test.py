"""Scoped semantic controls for real Native fixture dispatch and constructors.

The indexed full gate separately owns include/provider authentication. These
controls parse the actual Rust owners and mutate only their exact source joins.
"""
from __future__ import annotations

import importlib.util
import json
import re
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
    return (
        *(b for b in native.NATIVE_TYPED_SETTLEMENT_SOURCE_BINDINGS if b[0] == FIXTURE.as_posix()),
        *native.NATIVE_CANONICAL_PUBLICATION_FIXTURE_BINDINGS,
    )


def replace_rust_relation_once(source, old, new, start=0):
    """Mutate the same Rust relation after rustfmt changes its whitespace.

    Keep the source offset map so mutations touch the real item instead of a
    normalized copy. Only whitespace and optional trailing commas are ignored;
    identifiers, arguments, operators, and their order remain exact.
    """
    tail = source[start:]
    positions = [index for index, char in enumerate(tail) if not char.isspace()]
    positions = [
        position for index, position in enumerate(positions)
        if not (
            tail[position] == ","
            and index + 1 < len(positions)
            and tail[positions[index + 1]] in ")]}"
        )
    ]
    compact = "".join(tail[position] for position in positions)
    # A snippet ending in a comma does not show whether its following token
    # closes a call. Normalize that boundary before finding the first match,
    # so a later occurrence cannot replace the intended final argument.
    needle = native._normalize_rust_relation(old).rstrip(",")
    boundary = r"(?=,|[)\]}])" if old.rstrip().endswith(",") else ""
    match = re.search(re.escape(needle) + boundary, compact)
    assert match is not None, f"mutation relation is absent after rustfmt: {old!r}"
    offset = match.start()
    first = start + positions[offset]
    end = start + positions[offset + len(needle) - 1] + 1
    if old.rstrip().endswith(","):
        next_token = end
        while next_token < len(source) and source[next_token].isspace():
            next_token += 1
        if next_token < len(source) and source[next_token] == ",":
            end = next_token + 1
    return source[:first] + new + source[end:]


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
    for relative in {FIXTURE, CORRIDOR, *(Path(path) for path, _, _, _ in bindings())}:
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
    path.write_text(replace_rust_relation_once(text, old, new, start))
    assert validate(fixture)


@pytest.mark.parametrize("anchor,old,new", [
    ("historical_certified_source_recovery_cannot_authorize_retired_merge_execution", "run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::HistoricalRecovery);", "run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::SuccessfulApply);"),
    ("historical_certified_source_recovery_cannot_authorize_retired_merge_execution", "run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::HistoricalRecovery);", "if false { run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::HistoricalRecovery); }"),
    ("fn run_autonomous_merge_frontier_fixture", "frontier_case == MergeFrontierFixtureCase::StartupRegistryBoundaries", "frontier_case == MergeFrontierFixtureCase::HistoricalRecovery"),
    ("fn run_autonomous_merge_frontier_fixture", "ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle()", "ApplyFixture::new_for_production_recovered_decision_apply_with_lane_lifecycle()"),
    ("fn run_autonomous_merge_frontier_fixture", "frontier_case == MergeFrontierFixtureCase::SuccessfulApply", "frontier_case == MergeFrontierFixtureCase::HistoricalRecovery"),
    ("fn run_autonomous_merge_frontier_fixture", "for _ in 0..4 {", "for _ in 0..3 {"),
    ("if frontier_case != MergeFrontierFixtureCase::SuccessfulApply {", "assert_eq!(autonomous_balance(), None);", "assert_eq!(autonomous_balance(), Some(1));"),
    ("fn run_autonomous_merge_frontier_fixture", "assert!(queue.has_durable_plan_claim_for_test(key.entrypoint_hash));", "assert!(!queue.has_durable_plan_claim_for_test(key.entrypoint_hash));"),
])
def test_native_fixture_dispatch_and_retained_assertions_reject_substitution(fixture, anchor, old, new):
    path = fixture / CORRIDOR
    text = path.read_text()
    start = text.index(anchor)
    path.write_text(replace_rust_relation_once(text, old, new, start))
    assert any("Native corridor macro test" in e for e in validate(fixture))


@pytest.mark.parametrize("symbol,old,new", [
    ("assert_retired_merge_candidate_rejected", "for _ in 0..2 {", "for _ in 0..0 {"),
    ("assert_retired_merge_candidate_rejected", "assert_eq!(fixture.state.committed_height(), before_height);", "assert_eq!(fixture.state.committed_height(), 0);"),
    ("assert_retired_merge_candidate_rejected", "assert_eq!(fixture.state.has_committed_entrypoint(*hash), *committed);", "let _ = hash;"),
    ("prepared_native_publication_for_test", "overlay.authorize_execution_output_publication(&committed, &witness)", "overlay.authorize_execution_output_publication(&committed, &other_witness)"),
    ("prepared_native_publication_for_test", "overlay.apply_without_execution_with_verified_v2_finality(&committed)", "overlay.apply_without_execution_with_verified_v2_finality(&other_commit)"),
    ("finalize_native_execution_for_test", ".take_exec_witness()", ".fake_witness()"),
    ("finalize_native_execution_for_test", ".take(3)", ".take(2)"),
    ("finalize_native_execution_for_test", "execution_commitment_from_validated_block(&witness, &native, &lanes, &block)", "execution_commitment_from_validated_block(&witness, &native, &lanes, &other_block)"),
    ("finalize_native_execution_for_test", "crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone())", "crate::block::VerifiedV2FinalityArtifact::verify(other_artifact)"),
    ("finalize_native_execution_for_test", ".store_v2_finality_artifact(&artifact)", ".store_v2_finality_artifact(&other_artifact)"),
    ("promote_native_execution_finality_for_test", ".promote_kagemusha_finality_sidecar(artifact, &receipt)", ".promote_kagemusha_finality_sidecar(artifact, &other_receipt)"),
    ("publish_next_native_group_for_test", "overlay.commit()", "overlay.discard()"),
    ("cold_restore_completed_native_history_for_test", "Kura::new_with_configured_lane_catalog(", "Kura::blank_kura_for_testing("),
    ("cold_restore_completed_native_history_for_test", "restored.kura.bind_lane_storage_network(restored.network_id)", "restored.kura.bind_lane_storage_network(other_network)"),
    ("cold_restore_completed_native_history_for_test", "restored.prepare_restored_configured_primary_geometry_anchor(&nexus.configured_lane_catalog)", "restored.prepare_restored_configured_primary_geometry_anchor(&other_catalog)"),
    ("cold_restore_completed_native_history_for_test", "restored.restore_kura_lane_segments_from_nexus()", "restored.skip_lane_restore()"),
    ("cold_restore_completed_native_history_for_test", "copy_durable_tree(&state.kura.store_root(), cold_root.path());", "let _ = cold_root.path();"),
    ("cold_restore_completed_native_history_for_test", "strict_kura_config_for_testing(cold_root.path().to_path_buf())", "strict_kura_config_for_testing(state.kura.store_root())"),
    ("cold_restore_completed_native_history_for_test", "assert_eq!(std::fs::read(&source).unwrap(), bytes,", "assert_eq!(std::fs::read(&source).unwrap(), other_bytes,"),
    ("assert_completed_native_history_for_test", "Some(original_finality)", "None"),
    ("assert_completed_native_history_for_test", "Some(height)", "Some(other_height)"),
    ("assert_completed_native_history_for_test", "if reason == expected_reason", "if true"),
    ("native_completed_history_rejects_reapplication_after_second_economic_commit_impl", "assert_eq!(total, Quantity::from(40_u32))", "assert_eq!(total, Quantity::from(41_u32))"),
    ("native_completed_history_rejects_reapplication_after_second_economic_commit_impl", "current_total > total", "current_total >= total"),
    ("native_completed_history_rejects_reapplication_after_second_economic_commit_impl", "for change_instance in [false, true]", "for change_instance in [false]"),
    ("native_completed_history_rejects_reapplication_after_second_economic_commit_impl", "restored.prepare_proposed_native_lane_batch_source(&candidate, &[]).is_err()", "restored.prepare_proposed_native_lane_batch_source(&candidate, &[]).is_ok()"),
])
def test_canonical_native_publication_rejects_witness_finality_or_custody_substitution(fixture, symbol, old, new):
    path, _, _, _ = next(row for row in native.NATIVE_CANONICAL_PUBLICATION_FIXTURE_BINDINGS if row[2] == symbol)
    target = fixture / path
    source = target.read_text()
    start = source.index(f"fn {symbol}")
    target.write_text(replace_rust_relation_once(source, old, new, start))
    assert any("canonical native publication fixture" in error for error in validate(fixture))


@pytest.mark.parametrize("replacement", [
    "publish_next_native_group_for_test();",
    "if false { native_completed_history_rejects_reapplication_after_second_economic_commit_impl(); }",
])
def test_canonical_native_history_wrapper_cannot_skip_real_regression(fixture, replacement):
    path = fixture / "crates/iroha_core/src/state/native_completed_history_tests.rs"
    source = path.read_text()
    call = "native_completed_history_rejects_reapplication_after_second_economic_commit_impl();"
    assert source.count(call) == 1
    path.write_text(source.replace(call, replacement, 1))
    assert any("canonical native publication wrapper" in error for error in validate(fixture))
