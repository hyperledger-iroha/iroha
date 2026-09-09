"""Copied-source controls for worker read authority and actual actor handoff."""
from __future__ import annotations

import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

import pytest

REPO = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("worker_handoff_reconciliation_checker",
    REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py")
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
assert SPEC.loader is not None
SPEC.loader.exec_module(CHECKER)
NAMES = (
    "sweep_buffered_payload_chunk_lifecycles",
    "autonomous_lane_output_has_durable_reconstruction_source",
    "applied_height_handoff_accepts_kura_applied_ordinary_historical_lane_output",
    "applied_height_handoff_accepts_record_backed_autonomous_historical_lane_certificate",
)
FIXTURE = "install_applied_height_ranked_backpressure"
PATHS = {
    NAMES[0]: "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
    NAMES[1]: "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
    NAMES[2]: "crates/iroha_core/src/sumeragi/v2_worker/applied_height_handoff_tests.rs",
    NAMES[3]: "crates/iroha_core/src/sumeragi/v2_worker/applied_height_handoff_tests.rs",
    FIXTURE: "crates/iroha_core/src/sumeragi/v2_worker/applied_height_handoff_tests.rs",
}


def exact_replace(source, old, new):
    assert source.count(old) == 1, (source.count(old), old)
    return source.replace(old, new, 1)

def controls(owners):
    cases = []
    def add(label, name, old, new, diagnostic):
        cases.append((label, name, exact_replace(owners[name], old, new), diagnostic))
    sweep, auto, ordinary, autonomous = NAMES
    final_reset = "        if self.orphan_chunks.is_empty() {\n            self.orphan_lifecycle_sweep_cursor = None;\n        }\n        first_error.map_or(Ok(retired), Err)"
    add("sweep-missing-final-reset", sweep, final_reset, "        first_error.map_or(Ok(retired), Err)", "clearing its cursor only after the final removal")
    add("sweep-inverted-final-reset", sweep, final_reset, final_reset.replace("if self.orphan_chunks", "if !self.orphan_chunks"), "clearing its cursor only after the final removal")
    add("sweep-clears-live-cursor", sweep, final_reset, "        self.orphan_lifecycle_sweep_cursor = None;\n        first_error.map_or(Ok(retired), Err)", "clearing its cursor only after the final removal")
    add("sweep-unbounded-visits", sweep, ".min(MAX_ORPHAN_LIFECYCLE_VISITS_PER_REPLAY)", ".max(MAX_ORPHAN_LIFECYCLE_VISITS_PER_REPLAY)", "bound visits and terminalize")
    add("sweep-wrong-owner-offset", sweep, ".and_then(|chunks| chunks.get(cursor.chunk_offset))", ".and_then(|chunks| chunks.get(0))", "bound visits and terminalize")
    add("sweep-removes-nonterminal", sweep, "Ok(true) => {", "Ok(false) => {", "bound visits and terminalize")
    add("sweep-skips-shifted-owner", sweep, "self.orphan_lifecycle_sweep_cursor = Some(cursor);", "self.orphan_lifecycle_sweep_cursor = Some(OrphanPayloadLifecycleSweepCursor { manifest_hash: cursor.manifest_hash, chunk_offset: cursor.chunk_offset.saturating_add(1), });", "revisit the shifted offset")
    add("sweep-loses-first-error", sweep, "first_error = Some(error);", "let _ = error;", "preserve unresolved owners and the first error")
    add("sweep-swallows-error", sweep, "first_error.map_or(Ok(retired), Err)", "Ok(retired)", "preserve unresolved owners and the first error")
    add("sweep-does-not-debit-owner", sweep, "self.orphan_chunk_count.saturating_sub(1)", "self.orphan_chunk_count.saturating_sub(0)", "subtract the removed owner")
    read_calls = (
        ("payload", "read_current_autonomous_lane_block_artifact", "read_autonomous_lane_block_artifact", "descriptor", "autonomous-lane payload has no durable reconstruction artifact", "autonomous payload reconstruction"),
        ("vote", "read_current_autonomous_lane_payload", "current_autonomous_lane_payload", "body", "autonomous NewView vote has no durable payload cursor", "autonomous NewView vote must preserve strict"),
        ("certificate", "read_current_autonomous_lane_block_artifact", "read_autonomous_lane_block_artifact", "body", "autonomous NewView certificate has no durable payload cursor", "autonomous NewView certificate must preserve strict"),
    )
    for label, current, old, descriptor, message, diagnostic in read_calls:
        chunk = f'''                    .{current}(
                        {descriptor}.lane_id,
                        {descriptor}.lane_block_height,
                        network_id,
                        epoch,
                    )
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| {{
                        "{message}".to_owned()
                    }})?;'''
        add(f"{label}-restores-lossy-read", auto, chunk, chunk.replace(current, old).replace("                    .map_err(|error| error.to_string())?\n", ""), diagnostic)
        add(f"{label}-flattens-read-error", auto, chunk, chunk.replace(".map_err(|error| error.to_string())?", ".ok().flatten()"), diagnostic)
        add(f"{label}-reads-wrong-lane", auto, chunk, chunk.replace(f"{descriptor}.lane_id,", "LaneId::SINGLE,"), diagnostic)
    add("vote-skips-local-signer", auto, "if vote.signer != *local_peer", "if false", "exact signer, height, payload and roster")
    add("vote-skips-roster", auto, "|| !current.descriptor.validator_set.contains(&vote.signer)", "|| false", "exact signer, height, payload and roster")
    add("certificate-skips-roster", auto, "|| certificate.validator_set != payload.origin_proposal.descriptor.validator_set", "|| false", "exact body and roster")
    add("certificate-accepts-missing-exact-record", auto, "|| exact_durable.is_none()", "|| exact_durable.is_some()", "exact stored certificate and local retransmit authority")
    add("autonomous-succeeds-before-reads", auto, "    for message in messages {", "    return Ok(());\n    for message in messages {", "must not succeed before its durable checks")
    for name, prefix in ((ordinary, "ordinary"), (autonomous, "autonomous-history")):
        add(prefix + "-restores-ticketless-fixture", name, "let actor_owners = install_applied_height_ranked_backpressure(&mut service);", "install_exact_output_backpressure(&mut service);", "retain actual ranked actor ownership before enqueue")
        before = 'actor_owners.lock().expect("inspect owned handoff waiter")[0].waiter_count(),\n        1'
        add(prefix + "-missing-live-waiter", name, before, before[:-1] + "0", "assert one live waiter")
        add(prefix + "-waiter-not-released", name, "assert_eq!(owners[0].waiter_count(), 0);", "assert_eq!(owners[0].waiter_count(), 1);", "one real ticket cancellation")
        add(prefix + "-ticket-not-cancelled", name, "assert_eq!(owners[0].ticket_drop_cancellations(), 1);", "assert_eq!(owners[0].ticket_drop_cancellations(), 0);", "one real ticket cancellation")
        add(prefix + "-claim-not-message-bound", name, "&& *message_hash == HashOf::new(&historical_output)", "&& true", "exact target, source, proposal and message")
        handoff = owners[name].index("    assert_eq!(\n        service\n            .handoff_applied_height_output_to_durable_reconstruction(")
        waiter = owners[name].index('    let owners = actor_owners.lock().expect("inspect handoff ticket release");')
        after = owners[name].index("    drop(owners);", waiter) + len("    drop(owners);\n")
        reordered = owners[name][:handoff] + owners[name][waiter:after] + owners[name][handoff:waiter] + owners[name][after:]
        cases.append((prefix + "-asserts-release-before-handoff", name, reordered, "retire one exact durable owner before asserting"))
    add("fixture-discards-real-actor-owner", FIXTURE, ".push(fixture);", ".clear();", "retain the real actor owner")
    add("fixture-discards-ranked-ticket", FIXTURE, "ticket: Some(ticket)", "ticket: None", "retain the real actor owner")
    return cases


@pytest.fixture(scope="module")
def current_owners():
    sources = {name: (REPO / path).read_text() for name, path in PATHS.items()}
    items = {}
    for name, source in sources.items():
        matches = CHECKER.rust_items(source, name)
        assert len(matches) == 1
        items[name] = matches[0]
    return sources, items


def test_all_worker_handoff_real_contexts_and_clauses():
    assert tuple(CHECKER._WORKER_HANDOFF_RECONCILIATION_CONTEXTS) == (NAMES[0], NAMES[1], FIXTURE, NAMES[2], NAMES[3])
    assert CHECKER._worker_handoff_reconciled_source_fidelity_errors(REPO) == []
    source = (REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py").read_text()
    tree = ast.parse(source)
    function = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                    and node.name == "_exact_output_production_source_fidelity_errors")
    calls = [node for node in ast.walk(function) if isinstance(node, ast.Call)
             and isinstance(node.func, ast.Name)
             and node.func.id == "_worker_handoff_reconciled_source_fidelity_errors"]
    assert len(calls) == 1 and len(calls[0].args) == 1
    assert isinstance(calls[0].args[0], ast.Name) and calls[0].args[0].id == "repo_root"


@pytest.mark.parametrize("case_index", range(38))
def test_worker_handoff_mutants_reject_refreshed_whole_item_hashes(case_index, current_owners, tmp_path):
    sources, items = current_owners
    baseline = []
    CHECKER._require_worker_handoff_reconciled_source_contracts(REPO, items, baseline)
    assert baseline == []
    cases = controls({name: item.source for name, item in items.items()})
    assert len(cases) == 38
    label, name, modified, diagnostic = cases[case_index]
    path = tmp_path / PATHS[name]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(exact_replace(sources[name], items[name].source, modified))
    (changed,) = CHECKER.rust_items(path.read_text(), name)
    independent_hash = hashlib.sha256("\0".join(CHECKER.rust_code_tokens(changed.source)).encode()).hexdigest()
    assert independent_hash == CHECKER._rust_item_token_sha256(changed)
    assert independent_hash != CHECKER._rust_item_token_sha256(items[name])
    seal_errors = []
    CHECKER._require_rust_item_token_sha256(path, changed, independent_hash, label, seal_errors)
    assert seal_errors == []
    errors = []
    CHECKER._require_worker_handoff_reconciled_source_contracts(path, {**items, name: changed}, errors)
    (tmp_path / "rehashed-control.json").write_text(json.dumps({
        "case": label, "owner": name,
        "old_file_sha256": hashlib.sha256(sources[name].encode()).hexdigest(),
        "new_file_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "refreshed_owner_sha256": independent_hash, "seal_errors": seal_errors, "errors": errors,
    }, indent=2) + "\n")
    assert errors and any(diagnostic in error for error in errors)
    assert not any("digest" in error for error in errors)
