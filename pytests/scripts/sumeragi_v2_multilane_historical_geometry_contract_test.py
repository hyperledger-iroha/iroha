"""Scoped source and mutation checks for immutable historical geometry evidence."""

from __future__ import annotations

import ast
import importlib.util
import sys
from pathlib import Path

import pytest


def load_support():
    path = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")
    spec = importlib.util.spec_from_file_location("historical_geometry_test_support", path)
    assert spec is not None and spec.loader is not None
    support = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = support
    spec.loader.exec_module(support)
    return support


def validate(fixture):
    root, _, checker, models = fixture
    errors = []
    with checker._reviewed_rust_source_cache():
        checker.historical_geometry_contract.validate_historical_geometry_contract(
            root, models, errors, checker._rust_binding_item,
        )
    return tuple(errors)


@pytest.fixture
def fixture(tmp_path):
    support = load_support()
    checker = support.load_checker()
    contract = checker.historical_geometry_contract
    relatives = {Path(row[0]) for row in contract.HISTORICAL_GEOMETRY_BINDINGS}
    relatives.add(Path(contract.GEOMETRY))
    support.copy_reviewed_source_fixture_with_includes(tmp_path, checker, relatives)
    result = tmp_path, support, checker, support.canonical_models()
    assert validate(result) == ()
    return result


def test_historical_geometry_contract_accepts_current_owners(fixture):
    assert validate(fixture) == ()


def test_historical_geometry_contract_is_connected_to_release_gate():
    checker = load_support().load_checker()
    tree = ast.parse(Path(checker.__file__).read_text(encoding="utf-8"))
    owners = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
    assert sum(
        isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
        and isinstance(n.func.value, ast.Name)
        and n.func.value.id == "historical_geometry_contract"
        and n.func.attr == "validate_historical_geometry_contract"
        for n in ast.walk(owners["_validate"])
    ) == 1
    assert any(
        isinstance(n, ast.Starred) and isinstance(n.value, ast.Attribute)
        and isinstance(n.value.value, ast.Name)
        and n.value.value.id == "historical_geometry_contract"
        and n.value.attr == "HISTORICAL_GEOMETRY_SOURCE_RELATIVES"
        for n in ast.walk(owners["source_manifest_sha256"])
    )


@pytest.mark.parametrize("symbol", [
    "ObservedHistoricalRecoveryEvidence", "observe_geometry_historical_autonomous_recovery_records",
    "ensure_unchanged", "attest", "read_historical_autonomous_recovery_record_with_identity",
    "historical", "into_observed",
])
def test_historical_geometry_contract_rejects_missing_owner(fixture, symbol):
    _, _, checker, models = fixture
    c = checker.historical_geometry_contract
    native = next(m for m in models if m["module"] == c.NATIVE_MODULE)
    native["production_symbols"] = [b for b in native["production_symbols"]
        if not (b["symbol"] == symbol and b["path"] in (c.HISTORICAL, c.RECOVERY, c.EFFECTS))]
    assert any(f"ledger owner {symbol}" in e for e in validate(fixture))


def test_historical_geometry_contract_rejects_weakened_tokens(fixture):
    _, _, checker, models = fixture
    c = checker.historical_geometry_contract
    native = next(m for m in models if m["module"] == c.NATIVE_MODULE)
    row = next(b for b in native["production_symbols"] if b["symbol"] == "attest" and b["path"] == c.HISTORICAL)
    row["required_tokens"].remove("file.sync_all()")
    assert any("reviewed tokens changed for attest" in e for e in validate(fixture))


@pytest.mark.parametrize("owner,old,new,diagnostic", [
    ("HISTORICAL", "    records: Vec<HistoricalAutonomousLaneRecoveryRecordV1>", "    pub(super) records: Vec<HistoricalAutonomousLaneRecoveryRecordV1>", "exposes mutable ownership"),
    ("HISTORICAL", "fn attest(self)", "fn attest(&self)", "executable relation"),
    ("HISTORICAL", "        observed.ensure_unchanged()?;", "        // observed.ensure_unchanged()?;", "exact"),
    ("HISTORICAL", "snapshot.kind != BoundProgressDirectoryEntryKind::Directory", "snapshot.kind == BoundProgressDirectoryEntryKind::Directory", "executable relation"),
    ("HISTORICAL", "Some(&accounted)", "None", "executable relation"),
    ("HISTORICAL", "descriptor.lane_incarnation != expected_incarnation", "descriptor.lane_incarnation == expected_incarnation", "executable relation"),
    ("HISTORICAL", "descriptor.proposal_height <= activation_height", "descriptor.proposal_height < activation_height", "executable relation"),
    ("HISTORICAL", "finality.height_context != record.historical_context", "finality.height_context == record.historical_context", "executable relation"),
    ("HISTORICAL", "finality.verify().is_err()", "finality.verify().is_ok()", "executable relation"),
    ("HISTORICAL", "        .attest()", "        .unattested() // .attest()", "executable relation"),
    ("HISTORICAL", "        let outer = Self::open_bound_progress_directory", "        sync_dir(lane_artifacts)?;\n        let outer = Self::open_bound_progress_directory", "forbidden observation effect"),
    ("HISTORICAL", "            namespace.files.len(),", "            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,", "executable relation"),
    ("HISTORICAL", "encoded_bytes != self.encoded_bytes", "encoded_bytes > self.encoded_bytes", "executable relation"),
    ("HISTORICAL", "path != &observed.path", "path == &observed.path", "executable relation"),
    ("HISTORICAL", "after.bytes_hash != observed.bytes_hash", "after.bytes_hash == observed.bytes_hash", "executable relation"),
    ("RECOVERY", "!historical_autonomous_recovery_read_matches_accounting(accounted, &snapshot)", "historical_autonomous_recovery_read_matches_accounting(accounted, &snapshot)", "executable relation"),
    ("RECOVERY", "historical_autonomous_recovery_record_bytes(&record) != snapshot.bytes", "historical_autonomous_recovery_record_bytes(&record) == snapshot.bytes", "executable relation"),
    ("RECOVERY", "bounded.len() >= record_limit", "bounded.len() > record_limit", "executable relation"),
    ("RECOVERY", "*bytes <= aggregate_byte_limit", "*bytes >= aggregate_byte_limit", "executable relation"),
    ("RECOVERY", "!Kura::sidecar_is_single_link(&metadata)", "Kura::sidecar_is_single_link(&metadata)", "executable relation"),
    ("GEOMETRY", "read_and_attest_geometry_historical_autonomous_recovery_records(", "observe_geometry_historical_autonomous_recovery_records(", "exact"),
])
def test_historical_geometry_contract_rejects_semantic_mutation(fixture, owner, old, new, diagnostic):
    root, support, checker, _ = fixture
    support.replace_once(root / getattr(checker.historical_geometry_contract, owner), old, new)
    errors = validate(fixture)
    assert any(diagnostic in e for e in errors), errors
    assert not any("must have one" in e or "digest" in e for e in errors), errors


@pytest.mark.parametrize("old,new", [
    ("Self::Observe => observation.into_observed()", "Self::Observe => observation.attest()"),
    ("Self::MaintainAndAttest { .. } => observation.attest()", "Self::MaintainAndAttest { .. } => observation.into_observed()"),
])
def test_historical_geometry_contract_rejects_changed_dispatch(fixture, old, new):
    root, support, checker, _ = fixture
    support.replace_once_after(
        root / checker.historical_geometry_contract.EFFECTS,
        "fn historical(", old, new,
    )
    assert any("executable relation" in e for e in validate(fixture))


@pytest.mark.parametrize("mutation", ["removed", "production", "on-maintenance"])
def test_historical_geometry_contract_requires_test_only_observation(fixture, mutation):
    root, support, checker, _ = fixture
    path = root / checker.historical_geometry_contract.EFFECTS
    anchor = "fn historical("
    support.replace_once_after(path, anchor, "#[cfg(test)]",
                               "#[cfg(not(test))]" if mutation == "production" else "")
    if mutation == "on-maintenance":
        support.replace_once_after(path, anchor, "Self::MaintainAndAttest",
                                   "#[cfg(test)] Self::MaintainAndAttest")
    errors = validate(fixture)
    assert any("executable relation" in error for error in errors), errors
    assert not any("digest" in error or "must have one" in error for error in errors), errors


@pytest.mark.parametrize("observation", ["historical_observation", "confirmed_historical_observation"])
def test_historical_geometry_contract_rejects_bypassed_dispatch(fixture, observation):
    root, support, checker, _ = fixture
    support.replace_once(
        root / checker.historical_geometry_contract.GEOMETRY,
        f"effects.historical({observation})?", f"{observation}.into_observed()?",
    )
    assert any("missing or reorders" in e for e in validate(fixture))


@pytest.mark.parametrize("old,new,diagnostic", [
    ("self.ensure_unchanged()?;", "// self.ensure_unchanged()?;", "executable relation"),
    ("        self,", "        &self,", "executable relation"),
    ("        self.ensure_unchanged()?;", "        sync_dir(&self.outer.expected_path)?;\n        self.ensure_unchanged()?;", "forbidden observation effect"),
])
def test_historical_geometry_contract_rejects_changed_observed_consumer(fixture, old, new, diagnostic):
    root, support, checker, _ = fixture
    support.replace_once_after(
        root / checker.historical_geometry_contract.HISTORICAL,
        "fn into_observed(", old, new,
    )
    assert any(diagnostic in e for e in validate(fixture))


@pytest.mark.parametrize("earlier,later", [
    ("sidecar_file_metadata_unchanged(&observed.metadata.file, &opened)", "file.sync_all()"),
    ("file.sync_all()", "read_regular_sidecar_snapshot("),
    ("sync_dir(&namespace.directory.expected_path)", "self.ensure_unchanged()?;"),
])
def test_historical_geometry_contract_rejects_reordered_attestation(fixture, earlier, later):
    root, support, checker, _ = fixture
    support.swap_ordered_once_after(root / checker.historical_geometry_contract.HISTORICAL, "fn attest(self)", earlier, later)
    assert any("missing or reorders" in e for e in validate(fixture))
