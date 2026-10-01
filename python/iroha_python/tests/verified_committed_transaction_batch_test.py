"""Execute the real verified batch-result parser without native dependencies.

The native verifier remains responsible for proof authentication; these tests
qualify only the strict Python model for its exact per-leg result schema.
"""

from __future__ import annotations

import ast
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
import re
import sys
import types

import pytest


SOURCE = Path(__file__).resolve().parents[1] / "src" / "iroha_python" / "client.py"


def _load_verified_model():
    module = types.ModuleType("_iroha_verified_batch_parser_test")
    module.Mapping = Mapping
    module.dataclass = dataclass
    module.re = re
    sys.modules[module.__name__] = module
    functions = {
        "_require_non_empty_string",
        "_require_exact_non_empty_string",
        "_normalize_hex_string",
        "_normalize_hash_hex",
        "_normalize_positive_int",
    }
    tree = ast.parse(SOURCE.read_text(encoding="utf-8"))
    body = [ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0)]
    body.extend(
        node for node in tree.body
        if (isinstance(node, ast.FunctionDef) and node.name in functions)
        or (isinstance(node, ast.ClassDef) and node.name == "VerifiedCommittedTransaction")
    )
    exec(compile(ast.fix_missing_locations(ast.Module(body=body, type_ignores=[])),
                 str(SOURCE), "exec"), module.__dict__)
    return module.VerifiedCommittedTransaction


VerifiedCommittedTransaction = _load_verified_model()


def test_verified_batch_outcomes_accept_exact_native_leg_fields() -> None:
    outcomes = [
        {
            "leg_index": 0,
            "leg_id": "first-leg",
            "asset": "usd#payments",
            "destination": "first-controller",
            "amount": "12.50",
            "status": "Applied",
            "rejection_code": None,
            "rejection_message": None,
        },
        {
            "leg_index": 1,
            "leg_id": "second-leg",
            "asset": "usd#payments",
            "destination": "second-controller",
            "amount": "4",
            "status": "Rejected",
            "rejection_code": "NotPermitted",
            "rejection_message": "transfer is not permitted",
        },
    ]
    # Native batch_outcome_json serializes these eight fields per leg. Proof
    # identity authenticates the whole verified result, rather than each leg.
    payload = {
        "proof_kind": "selective-v1",
        "transaction_hash": "11" * 32,
        "block_hash": "22" * 32,
        "block_height": 7,
        "output_hash": "33" * 32,
        "network_id": "trusted-network",
        "context_id": "trusted-context",
        "promoted_checkpoint": b"promoted-checkpoint",
        "execution_commitment": {"executed_block_wire_len": 123},
        "executed_block_wire_hash": "55" * 32,
        "executed_block_wire_len": 123,
        "entrypoint_kind": "External",
        "authority": "authority@payments",
        "signer_public_key_hex": "44" * 32,
        "metadata": {},
        "executable": {"Instructions": []},
        "result_ok": True,
        "rejection_code": None,
        "rejection_message": None,
        "contract_rejection": None,
        "batch_outcomes": outcomes,
        "committed_transaction": {},
    }

    verified = VerifiedCommittedTransaction.from_payload(payload)
    assert verified.proof_kind == "selective-v1"
    assert verified.batch_outcomes == tuple(outcomes)

    for malformed_outcome in (
        {**outcomes[0], "proof_kind": "selective-v1"},
        {key: value for key, value in outcomes[0].items() if key != "status"},
        {**outcomes[0], "unverified_hint": "ignored"},
    ):
        with pytest.raises(ValueError, match="must contain exactly"):
            VerifiedCommittedTransaction.from_payload(
                {**payload, "batch_outcomes": [malformed_outcome]}
            )

    for malformed_outcome in (
        {**outcomes[0], "rejection_code": "NotPermitted"},
        {**outcomes[1], "rejection_message": None},
        {**outcomes[0], "status": "Unknown"},
    ):
        with pytest.raises((TypeError, ValueError), match="verified batch outcome"):
            VerifiedCommittedTransaction.from_payload(
                {**payload, "batch_outcomes": [malformed_outcome]}
            )
