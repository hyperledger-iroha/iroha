"""All-live source census retains exact active leases and parked paired votes."""
from __future__ import annotations

import ast
import importlib.util
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="module")
def checker():
    path = ROOT / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
    spec = importlib.util.spec_from_file_location("live_census_owner_checker", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def source(checker):
    errors = []
    result = checker._read_reviewed_rust_source(
        ROOT, "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs",
        errors, "all-live exact lease census",
    )
    assert not errors, errors
    return result


@pytest.fixture(scope="module")
def probe(checker):
    owner_path = Path(checker._successor_production_source_fidelity_errors.__code__.co_filename)
    owner, = [node for node in ast.parse(owner_path.read_text()).body
              if isinstance(node, ast.FunctionDef) and node.name == "_successor_production_source_fidelity_errors"]
    helpers = [node for node in owner.body if isinstance(node, ast.FunctionDef)
               and node.name in {"require_tokens", "require_order", "require_token_count"}]
    assert len(helpers) == 3
    path = Path(checker._successor_recovery_lifecycle_source_fidelity_errors.__code__.co_filename)
    tree = ast.parse(path.read_text())
    assignment, = [node for node in ast.walk(tree) if isinstance(node, ast.Assign)
                   and any(isinstance(t, ast.Name) and t.id == "all_live_census" for t in node.targets)]
    branch, = [node for node in ast.walk(tree) if isinstance(node, ast.If)
               and isinstance(node.test, ast.Compare) and isinstance(node.test.left, ast.Name)
               and node.test.left.id == "all_live_census"]
    function = ast.parse("def probe(registry_validate_path, registry_validate_source):\n    errors=[]\n    return errors\n").body[0]
    function.body[-1:-1] = helpers + [assignment, branch]
    namespace = dict(checker.__dict__)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[function], type_ignores=[])), str(path), "exec"), namespace)
    return namespace["probe"]


def test_current_live_census(probe, source):
    assert probe(*source) == []


@pytest.mark.parametrize(("old", "new"), (
    ("active == expected", "true"),
    ("|| !active_lease_is_exact", "|| false"),
    ("LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn", "LifecycleWorkClass::Apply | LifecycleWorkClass::ProducerTurn"),
    ("|| lease_id != lease.id", "|| false"),
    ("!paired_next_vote_addresses.insert(next_address)", "false"),
    ("!paired_live_next_vote_addresses.insert(next_address)", "false"),
    ("|| next_work.digest != next_digest", "|| false"),
    (".paired_next_sign_matches_terminal_record(coordinator, &exact_ledger)", ".unchecked_terminal_record(coordinator, &exact_ledger)"),
    ("broadcast.matches_current_live_census_record(", "broadcast.matches_reconstructed_record("),
    ("serve.matches_claimed_record(record, metadata, digest, lease)", "serve.matches_claimed_record(record, metadata, digest, foreign_lease)"),
    ("producer.matches_claimed_record(record, metadata, digest, lease)", "producer.matches_claimed_record(record, metadata, digest, foreign_lease)"),
    ("coordinator.owner_index != exact_owners", "false"),
    ("coordinator.capacity_used != exact_capacity_used", "false"),
    ("self.entries.len() != live.len()", "false"),
))
def test_live_census_rejects_foreign_or_missing_authority(checker, probe, source, old, new):
    path, body = source
    item, = checker.rust_items(body, "exactly_covers_all_live_work_with_optional_active_lease")
    assert item.source.count(old) == 1
    changed = body.replace(item.source, item.source.replace(old, new, 1), 1)
    assert probe(path, changed)
