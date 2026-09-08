"""Independent source controls for all 28 actions and replica Queue authority."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sys

import pytest


REPO = Path(__file__).resolve().parents[2]
TAG = "IN_FLIGHT_FIRST_RELEASE_ACTION_"
REFINEMENT = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
BINDING = "authenticated_replica_queue_disposition_observation"


def _load_checker():
    name = "action28_source_reconciliation_checker"
    spec = importlib.util.spec_from_file_location(
        name, REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py",
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


CHECKER = _load_checker()
ROW = next(row for row in CHECKER.PRODUCTION_TRACE_EXTRACTION_BINDINGS if row["id"] == BINDING)
OWNERS = (
    {**ROW, "required_tokens": (*ROW["action_tags"], *ROW["additional_tokens"])},
    ROW["checked_transition_source"],
    *ROW["supporting_sources"],
    ROW["authorization_source"],
    ROW["checked_transition_consumer"],
)


@pytest.fixture(scope="module")
def source_fixture(tmp_path_factory):
    """Retain exact production owners in small, independently copied source files."""
    root = tmp_path_factory.mktemp("action28-exact-owners")
    grouped = {}
    manifest = []
    for owner in OWNERS:
        errors = []
        item = CHECKER._production_trace_unique_function(
            root_dir=REPO, relative=owner["path"], symbol=owner["symbol"],
            impl_name=owner["impl"], errors=errors,
        )
        assert errors == []
        assert item is not None
        text = "\n".join((*item.attributes, item.source))
        if owner["impl"] is not None:
            text = f"impl {owner['impl']} {{\n{text}\n}}"
        grouped.setdefault(owner["path"], []).append(text)
        manifest.append({
            "path": owner["path"], "symbol": owner["symbol"],
            "full_production_file_sha256": hashlib.sha256((REPO / owner["path"]).read_bytes()).hexdigest(),
            "exact_owner_sha256": hashlib.sha256(item.source.encode()).hexdigest(),
            "owner_token_sha256": CHECKER._rust_sealed_item_token_sha256(item),
        })
    for relative, owners in grouped.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n\n".join(owners) + "\n")
    (root / "production-owner-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    assert CHECKER._production_trace_replica_observation_source_errors(root) == []
    return root


def _record_mutant(root, path, before, reason, errors):
    payload = path.read_bytes()
    assert payload != before
    result = {
        "reason": reason, "path": str(path.relative_to(root)),
        "baseline_sha256": hashlib.sha256(before).hexdigest(),
        "mutant_sha256": hashlib.sha256(payload).hexdigest(),
        "errors": errors,
    }
    (root / "rehashed-negative-control.json").write_text(json.dumps(result, indent=2) + "\n")


def test_action28_complete_production_authority_acceptance():
    assert CHECKER._production_trace_replica_observation_source_errors(REPO) == []


@pytest.mark.parametrize(
    ("owner_index", "token"),
    [(index, token) for index, owner in enumerate(OWNERS) for token in owner["required_tokens"]],
    ids=[f"owner{index}-guard{guard}" for index, owner in enumerate(OWNERS) for guard, _ in enumerate(owner["required_tokens"])],
)
def test_action28_each_required_authority_guard_is_source_bound(source_fixture, tmp_path, owner_index, token):
    shutil.copytree(source_fixture, tmp_path, dirs_exist_ok=True)
    owner = OWNERS[owner_index]
    path = tmp_path / owner["path"]
    before = path.read_bytes()
    errors = []
    item = CHECKER._production_trace_unique_function(
        root_dir=tmp_path, relative=owner["path"], symbol=owner["symbol"],
        impl_name=owner["impl"], errors=errors,
    )
    assert errors == []
    tokens = CHECKER.rust_code_tokens(item.source)
    required = CHECKER.rust_code_tokens(token)
    positions = CHECKER._token_sequence_positions(tokens, required)
    assert positions
    # Change the exact executable owner, then rehash the replacement. No stale
    # source digest can reject this control in place of the authority clause.
    spans = list(CHECKER._RUST_TOKEN_RE.finditer(CHECKER.mask_rust_comments_and_literals(item.source)))
    identifier = next(index for index, value in enumerate(required) if re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", value))
    replacement = item.source
    for position in sorted({position + identifier for position in positions}, reverse=True):
        span = spans[position]
        replacement = replacement[:span.start()] + "removed_action28_authority" + replacement[span.end():]
    source = before.decode()
    assert source.count(item.source) == 1
    path.write_text(source.replace(item.source, replacement))
    errors = CHECKER._production_trace_replica_observation_source_errors(tmp_path)
    _record_mutant(tmp_path, path, before, f"{owner['symbol']}: {token}", errors)
    assert errors
    assert any(owner["symbol"] in error and "lost required code" in error for error in errors)


@pytest.fixture(scope="module")
def shared_macro():
    macros = CHECKER.rust_macro_items((REPO / REFINEMENT).read_text(), "production_in_flight_first_release_transition_body")
    assert len(macros) == 1
    return macros[0].source


def _dispatch_arm_ranges(source):
    """Locate actual equality arms independently of the production extractor."""
    structural = CHECKER.mask_rust_comments_and_literals(source)
    pattern = re.compile(r"(?:else\s+)?if\s+projection\.action\s*==\s*refinement_tag_value!\(\s*(IN_FLIGHT_FIRST_RELEASE_ACTION_[A-Z_0-9]+)\s*\)\s*\{")
    result = []
    for match in pattern.finditer(structural):
        depth = 1
        end = match.end()
        while depth:
            depth += (structural[end] == "{") - (structural[end] == "}")
            end += 1
        result.append((match.group(1), match.start(), end))
    # The global ActivateKura conditional is outside the final 28-arm chain.
    assert len(result) == 29
    return result[-28:]


def test_dispatch_recognizes_every_actual_arm_once(shared_macro):
    errors = []
    tags = CHECKER._production_trace_shared_kernel_dispatch_tags(shared_macro, errors)
    assert errors == []
    assert len(tags) == len(set(tags)) == 28
    assert set(tags) == {row[1] for row in CHECKER.PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS}
    assert CHECKER.rust_code_tokens(shared_macro).count(TAG + "ACTIVATE_KURA") == 2
    assert tags.count(TAG + "ACTIVATE_KURA") == 1


@pytest.mark.parametrize("tag", [row[1] for row in CHECKER.PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS])
def test_dispatch_rejects_each_missing_actual_arm(shared_macro, tmp_path, tag):
    ranges = _dispatch_arm_ranges(shared_macro)
    _, start, end = next(row for row in ranges if row[0] == tag)
    changed = shared_macro[:start] + shared_macro[end:]
    if start == ranges[0][1]:
        changed = changed[:start] + re.sub(r"^\s*else\s+", "", changed[start:], count=1)
    path = tmp_path / "refinement.rs"
    path.write_text(changed)
    errors = []
    CHECKER._production_trace_shared_kernel_dispatch_tags(path.read_text(), errors)
    _record_mutant(tmp_path, path, shared_macro.encode(), "missing arm " + tag, errors)
    assert errors


@pytest.mark.parametrize("mutation", ["duplicate", "unknown", "true_fallback", "disjoined", "nested", "header_or", "comment_spoof"])
def test_dispatch_rejects_noncanonical_partition(shared_macro, tmp_path, mutation):
    ranges = _dispatch_arm_ranges(shared_macro)
    tag, start, end = ranges[-1]
    arm = shared_macro[start:end]
    if mutation == "duplicate":
        changed = shared_macro[:end] + arm + shared_macro[end:]
    elif mutation == "unknown":
        changed = shared_macro[:start] + arm.replace(tag, TAG + "UNKNOWN") + shared_macro[end:]
    elif mutation == "true_fallback":
        changed = shared_macro[:end] + shared_macro[end:].replace("false", "true", 1)
    elif mutation == "disjoined":
        first = ranges[0][1]
        prefix = shared_macro[:first]
        cut = prefix.rfind("&&")
        changed = prefix[:cut] + "||" + prefix[cut + 2:] + shared_macro[first:]
    elif mutation == "nested":
        first = ranges[0][1]
        close = shared_macro.rfind("\n    }}")
        changed = shared_macro[:first] + "{ " + shared_macro[first:close] + " }" + shared_macro[close:]
    elif mutation == "header_or":
        changed = shared_macro[:start] + arm.replace("{", "|| true {", 1) + shared_macro[end:]
    else:
        changed = shared_macro[:start] + "/* " + arm + " */" + shared_macro[end:]
    path = tmp_path / "refinement.rs"
    path.write_text(changed)
    errors = []
    CHECKER._production_trace_shared_kernel_dispatch_tags(path.read_text(), errors)
    _record_mutant(tmp_path, path, shared_macro.encode(), mutation, errors)
    assert errors


def test_complete_verus_observation_proof_includes_conditional_ensures_and_body():
    owner = next(owner for owner in OWNERS if owner["symbol"] == "production_in_flight_first_release_replica_queue_observation_is_exact")
    errors = []
    item = CHECKER._production_trace_unique_function(
        root_dir=REPO, relative=owner["path"], symbol=owner["symbol"], impl_name=None, errors=errors,
    )
    assert errors == []
    assert "IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_ABSENT" in item.source
    assert "IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_FIFO_PRESERVED" in item.source
    assert "projection.after.release.fifo_restored == projection.before.release.fifo_restored" in item.source
    assert CHECKER.rust_code_tokens(item.body) == CHECKER.rust_code_tokens("reveal(production_in_flight_first_release_transition_kernel);")
