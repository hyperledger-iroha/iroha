"""Source-owner and semantic controls for current Kura in-flight durability.

These use actual Rust owners and canonical declarations. They do not replace
native crash tests, model checking, or authenticated recursive source admission.
"""
from __future__ import annotations

import hashlib
import json

import pytest

from pytests.scripts.sumeragi_v2_multilane_semantic_binding_reconciliation_test import (
    ROOT,
    actual_item,
    checker,
)
from sumeragi_v2_multilane_inflight_contract import (
    INFLIGHT_LAYOUT_FORBIDDEN_SOURCE_CHECKS,
    INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS,
    INFLIGHT_LAYOUT_PRODUCTION_BINDINGS,
)

SYMBOLS = (
    "Kura::persist_autonomous_lifecycle_bootstrap_with_authentication",
    "Kura::complete_autonomous_lifecycle_bootstrap",
    "Kura::persist_lane_payload_availability_certificate",
    "Kura::transition_autonomous_lane_entrypoint_claims_locked",
    "Kura::write_autonomous_lane_block_view_state_record_locked",
    "Kura::update_disk_usage_delta",
    "Kura::add_disk_usage_bytes",
    "Kura::add_disk_usage_bytes_locked",
    "Kura::sub_disk_usage_bytes",
    "Kura::sub_disk_usage_bytes_locked",
    "Kura::add_total_disk_usage_bytes_locked",
    "Kura::sub_total_disk_usage_bytes_locked",
)


def key_for(symbol):
    rows = [row for row in INFLIGHT_LAYOUT_PRODUCTION_BINDINGS if row[2] == symbol]
    assert len(rows) == 1, f"canonical owner declaration missing or repeated: {symbol}"
    return rows[0][:3]


def evaluate(symbol, item):
    errors = []
    key = key_for(symbol)
    for table, mode in ((INFLIGHT_LAYOUT_PRODUCTION_BINDINGS, "required"),
                        (INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS, "ordered"),
                        (INFLIGHT_LAYOUT_FORBIDDEN_SOURCE_CHECKS, "forbidden")):
        for row in table:
            if row[:3] != key:
                continue
            cursor = 0
            for token in row[3]:
                position = item.find(token, cursor if mode == "ordered" else 0)
                if (position >= 0) == (mode == "forbidden"):
                    errors.append((mode, token))
                elif mode == "ordered":
                    cursor = position + len(token)
    return errors


@pytest.mark.parametrize("symbol", SYMBOLS)
def test_current_kura_durability_owner_matches_all_canonical_checks(symbol):
    errors = []
    item = actual_item(ROOT, *key_for(symbol), "current Kura in-flight owner", errors)
    assert errors == [] and item is not None
    assert evaluate(symbol, item) == []


@pytest.mark.parametrize("symbol", SYMBOLS)
def test_current_kura_durability_owner_cannot_lose_its_physical_provider(tmp_path, symbol):
    errors = []
    assert actual_item(tmp_path, *key_for(symbol), "missing Kura owner", errors) is None
    assert any("missing physical provider" in error for error in errors)


MUTATIONS = (
    (SYMBOLS[0], ".with_resource_paths(vec![path.clone()])", ".with_resource_paths(Vec::new())", False),
    (SYMBOLS[0], "self.update_disk_usage_delta(0, next_len);", "self.update_disk_usage_delta(0, next_len); self.update_total_disk_usage_delta(0, next_len);", False),
    (SYMBOLS[1], "receipt.proposal == payload.origin_proposal", "true", False),
    (SYMBOLS[1], 'payload.origin_proposal.descriptor.lane_block_height,\n            )?\n            .is_some_and', 'payload.origin_proposal.descriptor.lane_block_height,\n            ).ok().flatten()\n            .is_some_and', False),
    (SYMBOLS[1], "payload.origin_proposal.descriptor.lane_block_height,", "0,", False),
    (SYMBOLS[1], "Self::consume_autonomous_lifecycle_bootstrap_completion_fence(fence);", "let _ = fence;", False),
    (SYMBOLS[2], "let slot_is_certified =\n            self.autonomous_lane_slot_is_certified_locked(&entry, lane_block_height)?;", "let _ = self.autonomous_lane_slot_is_certified_locked(&entry, lane_block_height)?;\n        let slot_is_certified = false;", False),
    (SYMBOLS[2], "if slot_is_certified {", "if false {", False),
    (SYMBOLS[2], "if existing == &certificate {", "if true {", False),
    (SYMBOLS[3], ".with_resource_children(plan.len())", ".with_resource_children(0)", False),
    (SYMBOLS[3], "resource_child.finish();", "let _ = &resource_child;", False),
    (SYMBOLS[3], "resource_child.finish();", "let _ = &resource_child;", True),
    (SYMBOLS[4], ".with_resource_paths(vec![path.to_path_buf(), temp_path.clone()])", ".with_resource_paths(vec![path.to_path_buf()])", False),
    (SYMBOLS[5], "self.add_disk_usage_bytes(after - before);", "let _ = (before, after);", False),
    (SYMBOLS[5], "self.sub_disk_usage_bytes(before - after);", "let _ = (before, after);", False),
    (SYMBOLS[6], "self.add_disk_usage_bytes_locked(delta);", "let _ = delta;", False),
    (SYMBOLS[7], "self.add_total_disk_usage_bytes_locked(delta);", "let _ = delta;", False),
    (SYMBOLS[8], "self.sub_disk_usage_bytes_locked(delta);", "let _ = delta;", False),
    (SYMBOLS[9], "self.sub_total_disk_usage_bytes_locked(delta);", "let _ = delta;", False),
    (SYMBOLS[10], "current.saturating_add(delta)", "current", False),
    (SYMBOLS[11], "current.saturating_sub(delta)", "current", False),
)


@pytest.mark.parametrize("symbol,before,after,last", MUTATIONS)
def test_kura_durability_rejects_rehashed_semantic_provider_drift(tmp_path, symbol, before, after, last):
    key = key_for(symbol)
    errors = []
    item = actual_item(ROOT, *key, "mutation preimage", errors)
    assert errors == [] and item is not None and before in item
    assert evaluate(symbol, item) == []
    index = item.rfind(before) if last else item.find(before)
    changed_item = item[:index] + after + item[index + len(before):]
    provider = ROOT / key[0]
    original = provider.read_text()
    assert original.count(item) == 1
    destination = tmp_path / key[0]
    destination.parent.mkdir(parents=True)
    destination.write_text(original.replace(item, changed_item, 1))
    digest = hashlib.sha256(destination.read_bytes()).hexdigest()
    assert digest != hashlib.sha256(provider.read_bytes()).hexdigest()
    (tmp_path / "source-digest.json").write_text(json.dumps({key[0]: digest}))
    changed = actual_item(tmp_path, *key, "mutated physical provider", errors)
    assert errors == [] and changed is not None
    assert evaluate(symbol, changed), "changed behavior must fail the current canonical checks"


def test_inflight_canonical_declarations_match_the_current_checker():
    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    errors = []
    checker._validate_inflight_binding_inventory(document["inflight_first_release_layout_contract"], errors)
    assert errors == []
