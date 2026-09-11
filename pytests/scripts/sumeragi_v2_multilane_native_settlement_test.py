"""Rehashed semantic controls for typed Native settlement source ownership.

This is a bounded unit test of actual contract predicates and actual item
extraction. The separate whole checker authenticates the reviewed include/Git
provider graph; these controls neither replace nor waive that gate.
"""
from __future__ import annotations

import ast
import functools
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
HELPER = "scripts/formal/sumeragi_v2_multilane_native_merge_manifest_contract.py"
CHECKER = "scripts/formal/check_sumeragi_v2_multilane_models.py"
FIXTURE = "crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs"


def _load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def contract():
    native = _load(ROOT / HELPER, "native_settlement_source_contract")
    tree = ast.parse((ROOT / CHECKER).read_text())
    functions = {"_extract_braced_item", "_indexed_rust_binding_items",
                 "_rust_impl_items", "_extract_rust_binding_items"}
    nodes = [node for node in tree.body
             if isinstance(node, ast.FunctionDef) and node.name in functions
             or isinstance(node, ast.Assign) and any(
                 isinstance(target, ast.Name) and target.id == "RUST_DECLARATION_TEMPLATES"
                 for target in node.targets)]
    assert len(nodes) == 5
    namespace = {"re": re, "lru_cache": functools.lru_cache}
    exec(compile(ast.Module(body=nodes, type_ignores=[]), str(ROOT / CHECKER), "exec"), namespace)
    bindings = (*native.NATIVE_PARTICIPANT_APPLICATION_ROLE_BINDINGS,
                *native.NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_BINDINGS,
                native.NATIVE_APPLICATION_MANIFEST_BINDING,
                *native.NATIVE_TYPED_SETTLEMENT_SOURCE_BINDINGS)
    keys = [(path, kind, symbol) for path, kind, symbol, _ in bindings]
    keys.append((FIXTURE, "method", "ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle"))
    def items(root):
        result = {}
        for path, kind, symbol in keys:
            found = namespace["_extract_rust_binding_items"]((root / path).read_text(), kind, symbol)
            assert len(found) == 1, (path, kind, symbol)
            result[path, kind, symbol] = found[0]
        return result
    def errors(root, values):
        result = []
        for path, kind, symbol, tokens in bindings:
            for token in tokens:
                if token not in values[path, kind, symbol]:
                    result.append(f"{symbol}: missing source token {token!r}")
        native.validate_native_merge_manifest_relations(root, values, result)
        return result
    return native, bindings, items, errors


def _hash(data):
    return hashlib.sha256(data).hexdigest()


def test_typed_settlement_current_owners_and_inventory_are_exact(contract):
    native, bindings, items, errors = contract
    assert errors(ROOT, items(ROOT)) == []
    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    rows = next(model for model in document["models"]
                if model["module"] == "SumeragiV2NativeApplicationEvidence")["production_symbols"]
    for path, kind, symbol, tokens in bindings:
        assert [row for row in rows if (row["path"], row["kind"], row["symbol"]) ==
                (path, kind, symbol)] == [dict(path=path, kind=kind, symbol=symbol, required_tokens=list(tokens))]
    assert len(native.NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_BINDINGS) == 6


CONTROLS = [{'id': 'NS001',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_id != leg.lane_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.lane_id != leg.lane_id'},
 {'id': 'NS002',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.dataspace_id != leg.dataspace_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.dataspace_id != leg.dataspace_id'},
 {'id': 'NS003',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.participant_lane_id != leg.lane_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.participant_lane_id != leg.lane_id'},
 {'id': 'NS004',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.participant_lane_id != leg.lane_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.participant_lane_id != leg.lane_id'},
 {'id': 'NS005',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.participant_dataspace_id != leg.dataspace_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.participant_dataspace_id != leg.dataspace_id'},
 {'id': 'NS006',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.participant_dataspace_id != leg.dataspace_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.participant_dataspace_id != leg.dataspace_id'},
 {'id': 'NS007',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_incarnation != prepare.participant_lane_incarnation',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.lane_incarnation != '
            'prepare.participant_lane_incarnation'},
 {'id': 'NS008',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_incarnation != commit.participant_lane_incarnation',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.lane_incarnation != commit.participant_lane_incarnation'},
 {'id': 'NS009',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.proposal_height != prepare.authority_context_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.proposal_height != prepare.authority_context_height'},
 {'id': 'NS010',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.proposal_height != commit.authority_context_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.proposal_height != commit.authority_context_height'},
 {'id': 'NS011',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.previous_lane_block_height != prepare.participant_previous_block_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.previous_lane_block_height != '
            'prepare.participant_previous_block_height'},
 {'id': 'NS012',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.previous_lane_block_height != commit.participant_previous_block_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.previous_lane_block_height != '
            'commit.participant_previous_block_height'},
 {'id': 'NS013',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.previous_lane_block_descriptor_hash\n'
         '            != prepare.participant_previous_block_descriptor_hash',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.previous_lane_block_descriptor_hash != '
            'prepare.participant_previous_block_descriptor_hash'},
 {'id': 'NS014',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.previous_lane_block_descriptor_hash\n'
         '            != commit.participant_previous_block_descriptor_hash',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.previous_lane_block_descriptor_hash != '
            'commit.participant_previous_block_descriptor_hash'},
 {'id': 'NS015',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_block_height != prepare.participant_lane_block_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.lane_block_height != '
            'prepare.participant_lane_block_height'},
 {'id': 'NS016',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_block_height != commit.participant_lane_block_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.lane_block_height != '
            'commit.participant_lane_block_height'},
 {'id': 'NS017',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_block_view != prepare.participant_lane_block_view',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.lane_block_view != prepare.participant_lane_block_view'},
 {'id': 'NS018',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_block_view != commit.participant_lane_block_view',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: descriptor.lane_block_view != commit.participant_lane_block_view'},
 {'id': 'NS019',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'leg.participant_proposal.proposal_hash != prepare.participant_proposal_hash',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: leg.participant_proposal.proposal_hash != '
            'prepare.participant_proposal_hash'},
 {'id': 'NS020',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'leg.participant_proposal.proposal_hash != commit.participant_proposal_hash',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: leg.participant_proposal.proposal_hash != '
            'commit.participant_proposal_hash'},
 {'id': 'NS021',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'settlement_hash != leg.participant_settlement_hash',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: settlement_hash != leg.participant_settlement_hash'},
 {'id': 'NS022',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'leg.participant_settlement.lane_id() != descriptor.lane_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: leg.participant_settlement.lane_id() != descriptor.lane_id'},
 {'id': 'NS023',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'leg.participant_settlement.dataspace_id() != descriptor.dataspace_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: leg.participant_settlement.dataspace_id() != '
            'descriptor.dataspace_id'},
 {'id': 'NS024',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'leg.participant_settlement.lane_incarnation() != descriptor.lane_incarnation',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: leg.participant_settlement.lane_incarnation() != '
            'descriptor.lane_incarnation'},
 {'id': 'NS025',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'leg.participant_settlement.participant_lane_block_height()\n'
         '            != descriptor.lane_block_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: leg.participant_settlement.participant_lane_block_height() != '
            'descriptor.lane_block_height'},
 {'id': 'NS026',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'leg.participant_settlement.authority_context_height() != descriptor.proposal_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: leg.participant_settlement.authority_context_height() != '
            'descriptor.proposal_height'},
 {'id': 'NS027',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'Hash::from(settlement_hash) != prepare.participant_settlement_commitment',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: Hash::from(settlement_hash) != '
            'prepare.participant_settlement_commitment'},
 {'id': 'NS028',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'Hash::from(settlement_hash) != commit.participant_settlement_commitment',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: Hash::from(settlement_hash) != '
            'commit.participant_settlement_commitment'},
 {'id': 'NS029',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.coordinator_lane_id != receipt.lane_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.coordinator_lane_id != receipt.lane_id'},
 {'id': 'NS030',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.coordinator_lane_id != receipt.lane_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.coordinator_lane_id != receipt.lane_id'},
 {'id': 'NS031',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.coordinator_dataspace_id != receipt.dataspace_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.coordinator_dataspace_id != receipt.dataspace_id'},
 {'id': 'NS032',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.coordinator_dataspace_id != receipt.dataspace_id',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.coordinator_dataspace_id != receipt.dataspace_id'},
 {'id': 'NS033',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.coordinator_lane_incarnation != receipt.lane_incarnation',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.coordinator_lane_incarnation != receipt.lane_incarnation'},
 {'id': 'NS034',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.coordinator_lane_incarnation != receipt.lane_incarnation',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.coordinator_lane_incarnation != receipt.lane_incarnation'},
 {'id': 'NS035',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.authority_context_height != receipt.authority_context_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.authority_context_height != '
            'receipt.authority_context_height'},
 {'id': 'NS036',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.authority_context_height != receipt.authority_context_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.authority_context_height != '
            'receipt.authority_context_height'},
 {'id': 'NS037',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.planned_coordinator_block_height != receipt.lane_block_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.planned_coordinator_block_height != '
            'receipt.lane_block_height'},
 {'id': 'NS038',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.planned_coordinator_block_height != receipt.lane_block_height',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.planned_coordinator_block_height != '
            'receipt.lane_block_height'},
 {'id': 'NS039',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.coordinator_lane_block_view != receipt.lane_block_view',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.coordinator_lane_block_view != receipt.lane_block_view'},
 {'id': 'NS040',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.coordinator_lane_block_view != receipt.lane_block_view',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.coordinator_lane_block_view != receipt.lane_block_view'},
 {'id': 'NS041',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'prepare.coordinator_proposal_hash != receipt.coordinator_proposal_hash',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: prepare.coordinator_proposal_hash != '
            'receipt.coordinator_proposal_hash'},
 {'id': 'NS042',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'commit.coordinator_proposal_hash != receipt.coordinator_proposal_hash',
  'new': 'false',
  'reason': 'Each independently bound Prepare/Commit/settlement identity comparison must '
            'reject drift: commit.coordinator_proposal_hash != '
            'receipt.coordinator_proposal_hash'},
 {'id': 'NS043',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': '.computed_hash()',
  'new': '.__unchecked_advertised_hash()',
  'reason': 'The typed control must be canonically hashed, not trust its advertised hash.'},
 {'id': 'NS044',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_id == receipt.lane_id && descriptor.dataspace_id == '
         'receipt.dataspace_id',
  'new': 'descriptor.lane_id == receipt.lane_id || descriptor.dataspace_id == '
         'receipt.dataspace_id',
  'reason': 'Coordinator role requires both route coordinates.'},
 {'id': 'NS045',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'return Ok(NativeAmxParticipantApplicationRole::SeparateParticipant);',
  'new': 'return Ok(NativeAmxParticipantApplicationRole::Coordinator);',
  'reason': 'Remote participant must retain separate application authority.'},
 {'id': 'NS046',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_incarnation != receipt.lane_incarnation',
  'new': 'false',
  'reason': 'Same-route coordinator identity cannot silently drift: lane_incarnation'},
 {'id': 'NS047',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.proposal_height != receipt.authority_context_height',
  'new': 'false',
  'reason': 'Same-route coordinator identity cannot silently drift: proposal_height'},
 {'id': 'NS048',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_block_height != receipt.lane_block_height',
  'new': 'false',
  'reason': 'Same-route coordinator identity cannot silently drift: lane_block_height'},
 {'id': 'NS049',
  'path': 'crates/iroha_core/src/native_amx.rs',
  'kind': 'fn',
  'symbol': 'native_amx_participant_application_role',
  'old': 'descriptor.lane_block_view != receipt.lane_block_view',
  'new': 'false',
  'reason': 'Same-route coordinator identity cannot silently drift: lane_block_view'},
 {'id': 'NS050',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'descriptor.proposal_height != authority_context_height',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'descriptor.proposal_height != authority_context_height'},
 {'id': 'NS051',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'authority_context_height != application_block_height',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'authority_context_height != application_block_height'},
 {'id': 'NS052',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'authority_context_height > application_block_height',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'authority_context_height > application_block_height'},
 {'id': 'NS053',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'prepare.source_id != source.receipt.source_id',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'prepare.source_id != source.receipt.source_id'},
 {'id': 'NS054',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'commit.source_id != source.receipt.source_id',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'commit.source_id != source.receipt.source_id'},
 {'id': 'NS055',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'prepare.tx_entrypoint_hash != source.entrypoint_hash',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'prepare.tx_entrypoint_hash != source.entrypoint_hash'},
 {'id': 'NS056',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'commit.tx_entrypoint_hash != source.entrypoint_hash',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'commit.tx_entrypoint_hash != source.entrypoint_hash'},
 {'id': 'NS057',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'computed_settlement_hash != leg.participant_settlement_hash',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'computed_settlement_hash != leg.participant_settlement_hash'},
 {'id': 'NS058',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'settlement.lane_id() != descriptor.lane_id',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'settlement.lane_id() != descriptor.lane_id'},
 {'id': 'NS059',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'settlement.dataspace_id() != descriptor.dataspace_id',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'settlement.dataspace_id() != descriptor.dataspace_id'},
 {'id': 'NS060',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'settlement.lane_incarnation() != descriptor.lane_incarnation',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'settlement.lane_incarnation() != descriptor.lane_incarnation'},
 {'id': 'NS061',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'settlement.participant_lane_block_height() != descriptor.lane_block_height',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'settlement.participant_lane_block_height() != descriptor.lane_block_height'},
 {'id': 'NS062',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'settlement.authority_context_height() != authority_context_height',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'settlement.authority_context_height() != authority_context_height'},
 {'id': 'NS063',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'group.participant_proposal != leg.participant_proposal',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'group.participant_proposal != leg.participant_proposal'},
 {'id': 'NS064',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'group.participant_settlement != *settlement',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'group.participant_settlement != *settlement'},
 {'id': 'NS065',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'group.participant_settlement_hash != leg.participant_settlement_hash',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'group.participant_settlement_hash != leg.participant_settlement_hash'},
 {'id': 'NS066',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'member.source_id == source.receipt.source_id',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'member.source_id == source.receipt.source_id'},
 {'id': 'NS067',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'source_ids != group.settlement_source_ids',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: source_ids != '
            'group.settlement_source_ids'},
 {'id': 'NS068',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'source_ids.iter().copied().collect::<BTreeSet<_>>().len() != source_ids.len()',
  'new': 'false',
  'reason': 'Manifest grouping/source/QC identity comparison remains mandatory: '
            'source_ids.iter().copied().collect::<BTreeSet<_>>().len() != source_ids.len()'},
 {'id': 'NS069',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': 'let settlement_source_ids = settlement.source_ids().to_vec();',
  'new': 'let settlement_source_ids = Vec::new();',
  'reason': 'Exact ordered typed source vector cannot be invented.'},
 {'id': 'NS070',
  'path': 'crates/iroha_core/src/sumeragi/exec.rs',
  'kind': 'fn',
  'symbol': 'from_result_bearing_block_and_merge_entry',
  'old': '                let computed_settlement_hash =\n'
         '                    leg.participant_settlement.computed_hash().map_err(|_| {\n'
         '                        "Native AMX participant control settlement cannot be '
         'hashed".to_owned()\n'
         '                    })?;\n'
         '                if computed_settlement_hash != leg.participant_settlement_hash {\n'
         '                    return Err(\n'
         '                        "Native AMX participant control settlement hash '
         'mismatch".to_owned()\n'
         '                    );\n'
         '                }\n'
         '                let settlement = &leg.participant_settlement;\n'
         '                if settlement.lane_id() != descriptor.lane_id\n'
         '                    || settlement.dataspace_id() != descriptor.dataspace_id\n'
         '                    || settlement.lane_incarnation() != descriptor.lane_incarnation\n'
         '                    || settlement.participant_lane_block_height() != '
         'descriptor.lane_block_height\n'
         '                    || settlement.authority_context_height() != '
         'authority_context_height\n'
         '                {\n'
         '                    return Err(\n'
         '                        "Native AMX participant settlement differs from its '
         'application context"\n'
         '                            .to_owned(),\n'
         '                    );\n'
         '                }\n',
  'new': '                let settlement = &leg.participant_settlement;\n'
         '                if settlement.lane_id() != descriptor.lane_id\n'
         '                    || settlement.dataspace_id() != descriptor.dataspace_id\n'
         '                    || settlement.lane_incarnation() != descriptor.lane_incarnation\n'
         '                    || settlement.participant_lane_block_height() != '
         'descriptor.lane_block_height\n'
         '                    || settlement.authority_context_height() != '
         'authority_context_height\n'
         '                {\n'
         '                    return Err(\n'
         '                        "Native AMX participant settlement differs from its '
         'application context"\n'
         '                            .to_owned(),\n'
         '                    );\n'
         '                }\n'
         '                let computed_settlement_hash =\n'
         '                    leg.participant_settlement.computed_hash().map_err(|_| {\n'
         '                        "Native AMX participant control settlement cannot be '
         'hashed".to_owned()\n'
         '                    })?;\n'
         '                if computed_settlement_hash != leg.participant_settlement_hash {\n'
         '                    return Err(\n'
         '                        "Native AMX participant control settlement hash '
         'mismatch".to_owned()\n'
         '                    );\n'
         '                }\n',
  'reason': 'Typed hash validation must precede application-context acceptance/grouping, with '
            'all component tokens preserved.'},
 {'id': 'NS071',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'struct',
  'symbol': 'NativeAmxParticipantSettlement',
  'old': '    source_ids: Vec<[u8; Hash::LENGTH]>,',
  'new': '    source_ids: Vec<[u8; Hash::LENGTH]>,\n    receipts: Vec<u8>,',
  'reason': 'First-release control has no economic or recursive receipt field.'},
 {'id': 'NS072',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::lane_id',
  'old': 'self.lane_id',
  'new': 'self.unchecked_lane_id',
  'reason': 'Typed accessor returns exactly its own field; heights are not interchangeable.'},
 {'id': 'NS073',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::dataspace_id',
  'old': 'self.dataspace_id',
  'new': 'self.unchecked_dataspace_id',
  'reason': 'Typed accessor returns exactly its own field; heights are not interchangeable.'},
 {'id': 'NS074',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::lane_incarnation',
  'old': 'self.lane_incarnation',
  'new': 'self.unchecked_lane_incarnation',
  'reason': 'Typed accessor returns exactly its own field; heights are not interchangeable.'},
 {'id': 'NS075',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::participant_lane_block_height',
  'old': 'self.participant_lane_block_height',
  'new': 'self.authority_context_height',
  'reason': 'Typed accessor returns exactly its own field; heights are not interchangeable.'},
 {'id': 'NS076',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::authority_context_height',
  'old': 'self.authority_context_height',
  'new': 'self.participant_lane_block_height',
  'reason': 'Typed accessor returns exactly its own field; heights are not interchangeable.'},
 {'id': 'NS077',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::previous_native_settlement_hash',
  'old': 'self.previous_native_settlement_hash',
  'new': 'self.unchecked_previous_native_settlement_hash',
  'reason': 'Typed accessor returns exactly its own field; heights are not interchangeable.'},
 {'id': 'NS078',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::source_ids',
  'old': 'self.source_ids',
  'new': 'self.unchecked_source_ids',
  'reason': 'Typed accessor returns exactly its own field; heights are not interchangeable.'},
 {'id': 'NS079',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::computed_hash',
  'old': 'iroha:native-amx:participant-settlement:v1',
  'new': 'iroha:native-amx:participant-settlement:v0',
  'reason': 'Exact first-release participant-control hash domain required.'},
 {'id': 'NS080',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::computed_hash',
  'old': 'norito::encode_canonical(self)?',
  'new': 'norito::to_bytes(self)?',
  'reason': 'Hash commits exact canonical encoding, not a distinct framed representation.'},
 {'id': 'NS081',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::computed_hash',
  'old': '&domain_len,\n            DOMAIN,\n            &bytes,',
  'new': 'DOMAIN,\n            &domain_len,\n            &bytes,',
  'reason': 'Domain-length/domain/payload order is cryptographic identity.'},
 {'id': 'NS082',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::try_new',
  'old': 'participant_lane_block_height == 0',
  'new': 'false',
  'reason': 'Typed constructor rejects intrinsically invalid coordinates/membership: '
            'participant_lane_block_height == 0'},
 {'id': 'NS083',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::try_new',
  'old': 'authority_context_height == 0',
  'new': 'false',
  'reason': 'Typed constructor rejects intrinsically invalid coordinates/membership: '
            'authority_context_height == 0'},
 {'id': 'NS084',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::try_new',
  'old': 'participant_lane_block_height == 1 && previous_native_settlement_hash.is_some()',
  'new': 'false',
  'reason': 'Typed constructor rejects intrinsically invalid coordinates/membership: '
            'participant_lane_block_height == 1 && previous_native_settlement_hash.is_some()'},
 {'id': 'NS085',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::try_new',
  'old': 'source_ids.is_empty() || source_ids.len() > NATIVE_AMX_GROUP_SOURCES_MAX',
  'new': 'false',
  'reason': 'Typed constructor rejects intrinsically invalid coordinates/membership: '
            'source_ids.is_empty() || source_ids.len() > NATIVE_AMX_GROUP_SOURCES_MAX'},
 {'id': 'NS086',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::try_new',
  'old': 'source_ids.iter().any(|source| !native_amx_nonzero(source))',
  'new': 'false',
  'reason': 'Typed constructor rejects intrinsically invalid coordinates/membership: '
            'source_ids.iter().any(|source| !native_amx_nonzero(source))'},
 {'id': 'NS087',
  'path': 'crates/iroha_data_model/src/block/consensus.rs',
  'kind': 'method',
  'symbol': 'NativeAmxParticipantSettlement::try_new',
  'old': '!= source_ids.len()',
  'new': '== 0',
  'reason': 'Duplicate source membership cannot be accepted.'},
 {'id': 'NS088',
  'path': 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'kind': 'fn',
  'symbol': 'mutate_participant_settlement',
  'old': '    mutate(&mut fields);',
  'new': '    let _ = mutate;',
  'reason': 'Adverse fixture must execute its independent actual typed mutation.'},
 {'id': 'NS089',
  'path': 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'kind': 'fn',
  'symbol': 'mutate_participant_settlement',
  'old': 'fields.participant_lane_block_height,',
  'new': 'fields.authority_context_height,',
  'reason': 'Fixture must exercise participant height separately from authority height.'},
 {'id': 'NS090',
  'path': 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'kind': 'fn',
  'symbol': 'participant_application_role_rejects_settlement_identity_and_content_tampering',
  'old': 'fields.lane_id = LaneId::new(90)',
  'new': 'fields.lane_id = LaneId::new(7)',
  'reason': 'Actual fixture negative control must preserve semantic mutation intent: '
            'fields.lane_id = LaneId::new(90)'},
 {'id': 'NS091',
  'path': 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'kind': 'fn',
  'symbol': 'participant_application_role_rejects_settlement_identity_and_content_tampering',
  'old': 'fields.dataspace_id = DataSpaceId::new(90)',
  'new': 'fields.dataspace_id = DataSpaceId::new(7)',
  'reason': 'Actual fixture negative control must preserve semantic mutation intent: '
            'fields.dataspace_id = DataSpaceId::new(90)'},
 {'id': 'NS092',
  'path': 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'kind': 'fn',
  'symbol': 'participant_application_role_rejects_settlement_identity_and_content_tampering',
  'old': 'fields.participant_lane_block_height += 1',
  'new': 'fields.participant_lane_block_height += 0',
  'reason': 'Actual fixture negative control must preserve semantic mutation intent: '
            'fields.participant_lane_block_height += 1'},
 {'id': 'NS093',
  'path': 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'kind': 'fn',
  'symbol': 'participant_application_role_rejects_settlement_identity_and_content_tampering',
  'old': 'fields.source_ids[0] = [0x11; Hash::LENGTH]',
  'new': 'fields.source_ids[0] = fields.source_ids[0]',
  'reason': 'Actual fixture negative control must preserve semantic mutation intent: '
            'fields.source_ids[0] = [0x11; Hash::LENGTH]'},
 {'id': 'NS094',
  'path': 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'kind': 'fn',
  'symbol': 'participant_application_role_rejects_settlement_identity_and_content_tampering',
  'old': 'fields.authority_context_height += 1',
  'new': 'fields.authority_context_height += 0',
  'reason': 'Actual fixture negative control must preserve semantic mutation intent: '
            'fields.authority_context_height += 1'},
 {'id': 'NS095',
  'path': 'crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'kind': 'method',
  'symbol': 'ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle',
  'old': 'Self::new_with_options(false, false, true, true)',
  'new': 'Self::new_with_options(false, false, true, false)',
  'reason': 'Recovered Apply fixture must activate actual Native lane lifecycle.'},
 {'id': 'NS096',
  'path': 'crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'kind': 'method',
  'symbol': 'ApplyFixture::new_with_options',
  'old': '            include_lane_lifecycle,',
  'new': '            false,',
  'reason': 'Constructor forwarding cannot drop actual lifecycle owner.'},
 {'id': 'NS097',
  'path': 'crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'kind': 'method',
  'symbol': 'ApplyFixture::new_with_options',
  'old': '            include_native_lane,',
  'new': '            false,',
  'reason': 'Constructor forwarding cannot drop Native lane configuration.'},
 {'id': 'NS098',
  'path': 'crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'kind': 'method',
  'symbol': 'ApplyFixture::new_with_options_and_retention',
  'old': 'if include_lane_lifecycle {',
  'new': 'if false {',
  'reason': 'Fixture uses lifecycle-backed Kura when requested.'},
 {'id': 'NS099',
  'path': 'crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'kind': 'method',
  'symbol': 'ApplyFixture::new_with_options_and_retention',
  'old': 'install_fixture_validator_authority(&state, &context, &validator_set_pops);',
  'new': 'let _ = &validator_set_pops;',
  'reason': 'Fixture must install real canonical validator authority.'},
 {'id': 'NS100',
  'path': 'crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'kind': 'method',
  'symbol': 'ApplyFixture::new_with_options_and_retention',
  'old': 'install_fixture_native_lane(&mut state, &mut context);',
  'new': 'let _ = &mut context;',
  'reason': 'Fixture must install actual Native lane instead of toggling a declaration.'},
 {'id': 'NS101',
  'path': 'crates/iroha_core/src/kura/native_amx_participant_application_artifacts.rs',
  'kind': 'struct',
  'symbol': 'NativeAmxParticipantReceiptLatestIndexV2',
  'old': 'HashOf<iroha_data_model::block::consensus::NativeAmxParticipantSettlement>',
  'new': 'HashOf<iroha_data_model::block::consensus::LaneBlockCommitment>',
  'reason': 'Latest index binds typed participant-control identity, not economic settlement.'}]


@pytest.mark.parametrize("control", CONTROLS, ids=[row["id"] for row in CONTROLS])
def test_rehashed_native_settlement_owner_guards_fail_closed(tmp_path, contract, control):
    native, bindings, items, errors = contract
    paths = {path for path, _, _, _ in bindings}
    paths.add(FIXTURE)
    paths.add(native.NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE.as_posix())
    for relative in paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, destination)
    before = items(tmp_path)
    assert errors(tmp_path, before) == []
    key = (control["path"], control["kind"], control["symbol"])
    original = before[key]
    assert original.count(control["old"]) == 1
    changed = original.replace(control["old"], control["new"], 1)
    path = tmp_path / control["path"]
    source = path.read_text()
    assert source.count(original) == 1
    original_file_hash = _hash(path.read_bytes())
    path.write_text(source.replace(original, changed, 1))
    # Explicitly re-admit the new source and every affected owner; no old hash
    # can cause the required semantic rejection, and no source cache is reused.
    refreshed = items(tmp_path)
    assert refreshed[key] == changed
    admitted_files = {relative: _hash((tmp_path / relative).read_bytes()) for relative in paths}
    admitted_owners = {"!".join(owner): _hash(value.encode()) for owner, value in refreshed.items()}
    assert admitted_files[control["path"]] != original_file_hash
    assert admitted_owners["!".join(key)] != _hash(original.encode())
    failures = errors(tmp_path, refreshed)
    assert any(control["symbol"] in error for error in failures), (control["reason"], failures)
    assert all(_hash((tmp_path / relative).read_bytes()) == digest
               for relative, digest in admitted_files.items())
    (tmp_path / "rehashed-source-receipt.json").write_text(json.dumps(
        dict(control=control["id"], files=admitted_files, owners=admitted_owners,
             semantic_rejections=failures), sort_keys=True))
