"""Positive and semantic mutation controls for geometry evidence ownership."""

from __future__ import annotations

import ast
import importlib.util
import sys
from pathlib import Path

import pytest


def load_support():
    path = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")
    spec = importlib.util.spec_from_file_location("geometry_evidence_test_support", path)
    assert spec is not None and spec.loader is not None
    support = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = support
    spec.loader.exec_module(support)
    return support


@pytest.fixture
def fixture(tmp_path):
    support = load_support()
    checker = support.load_checker()
    contract = checker.geometry_evidence_contract
    relatives = {Path(row[0]) for row in contract.GEOMETRY_EVIDENCE_BINDINGS}
    support.copy_reviewed_source_fixture_with_includes(tmp_path, checker, relatives)
    result = tmp_path, support, checker, support.canonical_models()
    assert validate(result) == ()
    return result


def validate(fixture):
    root, _, checker, models = fixture
    errors = []
    with checker._reviewed_rust_source_cache():
        checker.geometry_evidence_contract.validate_geometry_evidence_contract(
            root, models, errors, checker._rust_binding_item,
        )
    return tuple(errors)


def test_geometry_evidence_contract_accepts_current_owners(fixture):
    assert validate(fixture) == ()


def test_geometry_evidence_contract_is_connected_to_release_gate():
    checker = load_support().load_checker()
    tree = ast.parse(Path(checker.__file__).read_text(encoding="utf-8"))
    validate_body = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "_validate")
    manifest_body = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "source_manifest_sha256")
    assert sum(
        isinstance(n, ast.Call)
        and isinstance(n.func, ast.Attribute)
        and isinstance(n.func.value, ast.Name)
        and n.func.value.id == "geometry_evidence_contract"
        and n.func.attr == "validate_geometry_evidence_contract"
        for n in ast.walk(validate_body)
    ) == 1
    assert any(
        isinstance(n, ast.Starred)
        and isinstance(n.value, ast.Attribute)
        and isinstance(n.value.value, ast.Name)
        and n.value.value.id == "geometry_evidence_contract"
        and n.value.attr == "GEOMETRY_EVIDENCE_SOURCE_RELATIVES"
        for n in ast.walk(manifest_body)
    )


@pytest.mark.parametrize("symbol", [
    "ObservedNativeAmxEvidence", "observe_geometry_native_amx_per_height_evidence",
    "read_and_attest_geometry_native_amx_per_height_evidence", "attest",
    "maintain_lane_retirement_route_locked", "certified_history_has_committed_rewrite_locked",
    "scan_lane_retirement_locked", "ensure_archived_lane_work_released_with_custody",
    "observe_lane_retirement_locked", "RetirementScanEffects", "prepare_route", "native", "into_observed",
])
def test_geometry_evidence_contract_rejects_missing_ledger_owner(fixture, symbol):
    _, _, checker, models = fixture
    native = next(m for m in models if m["module"] == checker.geometry_evidence_contract.NATIVE_MODULE)
    native["production_symbols"] = [r for r in native["production_symbols"] if r["symbol"] != symbol]
    assert any(f"ledger owner {symbol}" in e for e in validate(fixture))


def test_geometry_evidence_contract_rejects_weakened_ledger_tokens(fixture):
    _, _, checker, models = fixture
    native = next(m for m in models if m["module"] == checker.geometry_evidence_contract.NATIVE_MODULE)
    attest = next(r for r in native["production_symbols"] if r["symbol"] == "attest")
    attest["required_tokens"].remove("file.sync_all")
    assert any("reviewed tokens changed for attest" in e for e in validate(fixture))


# These edits retain the same declarations. Rejection must come from source
# semantics, not a changed source digest or a missing/renamed owner.
@pytest.mark.parametrize("owner,old,new,diagnostic", [
    ("EVIDENCE", "retained_count >= retained_record_limit", "retained_count > retained_record_limit", "executable relation"),
    ("EVIDENCE", "evidence_bytes > self.native_amx_participant_evidence_file_bytes()", "evidence_bytes < self.native_amx_participant_evidence_file_bytes()", "executable relation"),
    ("EVIDENCE", "if temporary {", "if false { // if temporary {", "executable relation"),
    ("EVIDENCE", "? != before.bytes", "? == before.bytes", "executable relation"),
    ("EVIDENCE", ".is_err()", ".is_ok()", "executable relation"),
    ("EVIDENCE", "manifests.insert(lane_block_height, artifact).is_some()", "manifests.insert(lane_block_height, artifact).is_none()", "executable relation"),
    ("EVIDENCE", "metadata: before.metadata", "metadata", "executable relation"),
    ("EVIDENCE", "    manifests: BTreeMap", "    pub(super) manifests: BTreeMap", "mutable ownership field"),
    ("EVIDENCE", "fn attest(self)", "fn attest(&self)", "executable relation"),
    ("EVIDENCE", "        .attest()", "        .unattested() // .attest()", "executable relation"),
    ("EVIDENCE", "file.sync_all()", "file.metadata() // file.sync_all()", "missing or reorders"),
    ("EVIDENCE", "after.bytes_hash != observed.bytes_hash", "after.bytes_hash == observed.bytes_hash", "executable relation"),
    ("EVIDENCE", "            != inventory", "            == inventory", "exact"),
    ("EVIDENCE", "        let payload_limit =", "        sync_dir(lane_artifacts)?;\n        let payload_limit =", "observer contains storage effect"),
    ("MAINTENANCE", ".is_none()", ".is_some()", "executable relation"),
    ("MAINTENANCE", "if retiring.contains(&frontier_identity)", "if !retiring.contains(&frontier_identity)", "executable relation"),
    ("MAINTENANCE", "        let lane_artifacts =", "        let _guard = self.sidecar_lock.lock();\n        let lane_artifacts =", "reacquires an inherited lock"),
    ("GEOMETRY", "effects.native(native_observation)?", "native_observation.into_observed()?", "missing or reorders"),
    ("EFFECTS", "Self::Observe => kura.observe_lane_retirement_route_locked(entry)", "Self::Observe => kura.maintain_lane_retirement_route_locked(entry)", "executable relation"),
    ("GEOMETRY", "custody.is_none().then(|| self.sidecar_lock.lock())", "custody.is_some().then(|| self.sidecar_lock.lock())", "missing or reorders"),
])
def test_geometry_evidence_contract_rejects_semantic_mutations(fixture, owner, old, new, diagnostic):
    root, support, checker, _ = fixture
    support.replace_once(root / getattr(checker.geometry_evidence_contract, owner), old, new)
    errors = validate(fixture)
    assert any(diagnostic in error for error in errors), errors
    assert not any("must have one" in error or "digest" in error for error in errors), errors


@pytest.mark.parametrize("owner,anchor,earlier,later", [
    ("EVIDENCE", "fn attest(self)", "file.sync_all()", "read_regular_sidecar_snapshot(&path, lane_artifacts, payload_limit)?"),
    ("MAINTENANCE", "fn maintain_lane_retirement_route_locked(", "recover_certified_lane_block_pair_from_frontier_locked(", "confirm_latest_certified_lane_block_frontier_read_locked("),
    ("MAINTENANCE", "fn maintain_lane_retirement_route_locked(", "compact_lane_histories_through_merge_frontier_locked(", "recover_geometry_progress_pairs_before_snapshot("),
    ("GEOMETRY", "fn scan_lane_retirement_locked(", "effects.prepare_route(", "self.geometry_bound_progress_directory_snapshot("),
])
def test_geometry_evidence_contract_rejects_reordered_effects(fixture, owner, anchor, earlier, later):
    root, support, checker, _ = fixture
    support.swap_ordered_once_after(root / getattr(checker.geometry_evidence_contract, owner), anchor, earlier, later)
    errors = validate(fixture)
    assert any("missing or reorders" in error for error in errors), errors


def test_geometry_evidence_contract_rejects_dropped_progress_pair(fixture):
    root, support, checker, _ = fixture
    support.replace_once_after(
        root / checker.geometry_evidence_contract.GEOMETRY,
        "let fixed_progress_pairs: [(&Path, &Path, &str); 7]",
        "&receipt_data,", "&lane_data,",
    )
    assert any("executable relation" in e for e in validate(fixture))


@pytest.mark.parametrize("anchor,old,new", [
    ("fn certified_history_has_committed_rewrite_locked(", ".is_some()", ".is_none()"),
    ("fn certified_history_has_committed_rewrite_locked(", "Self::autonomous_lane_merge_bundle_paths_for_entry", "Self::certified_lane_block_paths_for_entry"),
    ("if self.certified_history_has_committed_rewrite_locked(entry)? {", ".is_none()", ".is_some()"),
    ("if self.certified_history_has_committed_rewrite_locked(entry)? {", ".saturating_sub(self.lane_history_retention.get() as u64)", ".saturating_add(self.lane_history_retention.get() as u64)"),
    ("if self.certified_history_has_committed_rewrite_locked(entry)? {", ".transpose()?", ".transpose().unwrap_or(None)"),
    ("if self.certified_history_has_committed_rewrite_locked(entry)? {", "retention.as_ref()", "None"),
    ("if self.certified_history_has_committed_rewrite_locked(entry)? {", "Some(&frontier_read.frontier.artifact)", "None"),
    ("if let Some(frontier) =", ".is_none()", ".is_some()"),
])
def test_geometry_evidence_contract_rejects_branch_specific_rewrite_mutations(fixture, anchor, old, new):
    root, support, checker, _ = fixture
    support.replace_once_after(root / checker.geometry_evidence_contract.MAINTENANCE, anchor, old, new)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("must have one" in error or "digest" in error for error in errors), errors


def test_geometry_evidence_contract_requires_terminal_rewrite_before_frontier_recovery(fixture):
    root, support, checker, _ = fixture
    support.swap_ordered_once_after(
        root / checker.geometry_evidence_contract.MAINTENANCE,
        "fn maintain_lane_retirement_route_locked(",
        "recover_certified_bundle_history_rewrites_locked(",
        "recover_certified_lane_block_pair_from_frontier_locked(",
    )
    assert any("missing or reorders" in error for error in validate(fixture))


@pytest.mark.parametrize("symbol,old,new", [
    ("ensure_first_release_lane_retirement_admissible_with_certified_locked", "RetirementScanEffects::MaintainAndAttest", "RetirementScanEffects::Observe"),
    ("observe_lane_retirement_locked", "RetirementScanEffects::Observe", "RetirementScanEffects::MaintainAndAttest"),
    ("ensure_archived_lane_work_released_with_custody", "custody.authenticate(self)?;", "// custody.authenticate(self)?;"),
])
def test_geometry_evidence_contract_rejects_changed_caller_effects(fixture, symbol, old, new):
    root, support, checker, _ = fixture
    support.replace_once_after(
        root / checker.geometry_evidence_contract.GEOMETRY,
        f"fn {symbol}(", old, new,
    )
    assert any("executable relation" in error for error in validate(fixture))


@pytest.mark.parametrize("old,new", [
    ("Self::Observe => observation.into_observed()", "Self::Observe => observation.attest()"),
    ("Self::MaintainAndAttest { .. } => observation.attest()", "Self::MaintainAndAttest { .. } => observation.into_observed()"),
])
def test_geometry_evidence_contract_rejects_changed_native_dispatch(fixture, old, new):
    root, support, checker, _ = fixture
    support.replace_once_after(
        root / checker.geometry_evidence_contract.EFFECTS,
        "fn native(", old, new,
    )
    assert any("executable relation" in error for error in validate(fixture))


@pytest.mark.parametrize("symbol", ["RetirementScanEffects", "prepare_route", "native"])
@pytest.mark.parametrize("mutation", ["removed", "production", "on-maintenance"])
def test_geometry_evidence_contract_requires_test_only_observation(fixture, symbol, mutation):
    root, support, checker, _ = fixture
    path = root / checker.geometry_evidence_contract.EFFECTS
    anchor = f"enum {symbol}" if symbol == "RetirementScanEffects" else f"fn {symbol}("
    support.replace_once_after(path, anchor, "#[cfg(test)]",
                               "#[cfg(not(test))]" if mutation == "production" else "")
    if mutation == "on-maintenance":
        variant = "MaintainAndAttest" if symbol == "RetirementScanEffects" else "Self::MaintainAndAttest"
        support.replace_once_after(path, anchor, variant, "#[cfg(test)] " + variant)
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("symbol,old,new,diagnostic", [
    ("attest", "after.bytes_hash != observed.bytes_hash", "after.bytes_hash == observed.bytes_hash", "executable relation"),
    ("into_observed", "!= self.inventory", "== self.inventory", "executable relation"),
    ("into_observed", "(self)", "(&self)", "executable relation"),
    ("into_observed", "        for observed in &self.files {", "        sync_dir(&self.directory.expected_path)?;\n        for observed in &self.files {", "observer contains storage effect"),
])
def test_geometry_evidence_contract_rejects_changed_consumer(fixture, symbol, old, new, diagnostic):
    root, support, checker, _ = fixture
    support.replace_once_after(
        root / checker.geometry_evidence_contract.EVIDENCE,
        f"fn {symbol}", old, new,
    )
    assert any(diagnostic in error for error in validate(fixture))
