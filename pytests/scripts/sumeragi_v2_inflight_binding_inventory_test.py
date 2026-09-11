"""Exact current in-flight inventory rejects declaration and token substitution."""

from __future__ import annotations

import copy
import importlib.util
import json
import sys
from functools import lru_cache
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
FORMAL = ROOT / "scripts" / "formal"
BINDINGS = ROOT / "formal" / "sumeragi_v2" / "multilane_source_bindings.json"


@lru_cache(maxsize=1)
def checker():
    spec = importlib.util.spec_from_file_location(
        "inflight_exact_inventory_checker", FORMAL / "check_sumeragi_v2_multilane_models.py"
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def contract():
    return json.loads(BINDINGS.read_text())["inflight_first_release_layout_contract"]


def inventory_errors(value):
    errors: list[str] = []
    checker()._validate_inflight_binding_inventory(value, errors)
    return errors


def test_current_exact_inventory_is_accepted_without_rebinding():
    assert inventory_errors(contract()) == []
    assert not hasattr(checker(), "_current_inflight_production_binding")
    assert not hasattr(checker(), "_INFLIGHT_CURRENT_ORDERED_BINDINGS")


@pytest.mark.parametrize("field,key", [
    ("production_symbols", "required_tokens"), ("ordered_source_checks", "tokens")
])
def test_every_inventory_row_rejects_removed_or_substituted_tokens(field, key):
    baseline = contract()
    for index, row in enumerate(baseline[field]):
        changed = copy.deepcopy(baseline)
        changed[field][index][key] = row[key][:-1]
        assert inventory_errors(changed), (field, row["symbol"], "removed")
        changed[field][index][key] = ["UNREVIEWED_BINDING_SUBSTITUTION"]
        assert inventory_errors(changed), (field, row["symbol"], "substituted")


@pytest.mark.parametrize("field", ["production_symbols", "ordered_source_checks"])
def test_inventory_rejects_missing_duplicate_reordered_and_extra_rows(field):
    baseline = contract()
    for mutation in ("missing", "duplicate", "reordered", "extra"):
        changed = copy.deepcopy(baseline)
        if mutation == "missing":
            changed[field].pop()
        elif mutation == "duplicate":
            changed[field][-1] = copy.deepcopy(changed[field][0])
        elif mutation == "reordered":
            changed[field][0], changed[field][1] = changed[field][1], changed[field][0]
        else:
            changed[field].append(copy.deepcopy(changed[field][0]))
        assert inventory_errors(changed), (field, mutation)


@pytest.mark.parametrize("obsolete", [
    "candidate_work_requires_wait", "claim_certified_execution_proposal_turn",
    "Kura::persist_lane_block_execution_input_under_prune_guard",
])
def test_retired_owner_names_cannot_be_translated_to_current_authority(obsolete):
    baseline = contract()
    target = (
        "Kura::persist_lane_block_execution_input_under_prune_and_canonical_guards"
        if obsolete.startswith("Kura::") else "schedule_local_proposal"
    )
    index = next(i for i, row in enumerate(baseline["production_symbols"]) if row["symbol"] == target)
    baseline["production_symbols"][index]["symbol"] = obsolete
    assert inventory_errors(baseline)


@pytest.mark.parametrize("field,key", [
    ("production_symbols", "required_tokens"), ("ordered_source_checks", "tokens")
])
def test_inventory_rejects_malformed_rows_and_token_arrays(field, key):
    baseline = contract()
    for malformed in (None, {}, {"unexpected": "row"}):
        changed = copy.deepcopy(baseline)
        changed[field][0] = malformed
        assert inventory_errors(changed)
    for tokens in (None, [], [""], ["same", "same"], [1]):
        changed = copy.deepcopy(baseline)
        changed[field][0][key] = tokens
        assert inventory_errors(changed)


def test_inflight_inventory_regression_source_is_committed_by_manifest(tmp_path, monkeypatch):
    """The actual inventory and hash loop must own these regression declarations."""
    module = checker()
    relative = Path("pytests/scripts/sumeragi_v2_inflight_binding_inventory_test.py")

    def focused_inventory(paths):
        assert relative in paths, "the manifest must include its in-flight regression source"
        return {relative}

    # Recursive Git/include authentication has separate tests. Keep this test at
    # that seam while exercising the actual inventory assembly and byte hash loop.
    monkeypatch.setattr(module, "_expanded_source_manifest_paths", focused_inventory)
    ledger = tmp_path / "formal/sumeragi_v2/multilane_source_bindings.json"
    ledger.parent.mkdir(parents=True)
    ledger.write_bytes(BINDINGS.read_bytes())
    source = tmp_path / relative
    source.parent.mkdir(parents=True)
    original = (ROOT / relative).read_bytes()
    source.write_bytes(original)
    baseline = module.source_manifest_sha256(tmp_path)
    source.write_bytes(original + b"\n# changed regression source\n")
    assert module.source_manifest_sha256(tmp_path) != baseline
    source.write_bytes(original)
    assert module.source_manifest_sha256(tmp_path) == baseline
    source.unlink()
    with pytest.raises(FileNotFoundError):
        module.source_manifest_sha256(tmp_path)
