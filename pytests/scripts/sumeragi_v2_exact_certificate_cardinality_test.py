"""Mutation coverage for exact Sumeragi v2 wire-certificate cardinality."""

from __future__ import annotations

import importlib.util
import re
import shutil
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parents[2]
CHECKER = (
    ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"
)
FORMAL_SOURCE = ROOT_DIR / "formal" / "sumeragi_v2"
FORMAL_FILES = (
    "SumeragiV2Quorums.tla",
    "SumeragiV2Core.tla",
    "SumeragiV2Inductive.tla",
    "SumeragiV2ChainEpoch.tla",
    "SumeragiV2AsyncNetwork.tla",
    "SumeragiV2OwnershipInvariantCheck.tla",
    "SumeragiV2QuorumProofs.tla",
    "SumeragiV2Proofs.tla",
    "SumeragiV2InductiveProofs.tla",
    "SumeragiV2AsyncInstallRunnerProofs.tla",
    "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
    "SumeragiV2AsyncTimeoutOwnershipProofs.tla",
)


def load_checker():
    """Load the proof-ledger checker from the worktree under test."""

    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_exact_certificate_cardinality_checker", CHECKER
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def copy_formal_fixture(tmp_path: Path) -> Path:
    """Copy only the modules consumed by the exact-cardinality contract."""

    formal_dir = tmp_path / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in FORMAL_FILES:
        shutil.copyfile(FORMAL_SOURCE / name, formal_dir / name)
    return formal_dir


def mutate_operator(
    source: str,
    symbol: str,
    needle: str,
    replacement: str,
) -> str:
    """Replace one exact fragment only inside a named TLA+ operator."""

    declaration = re.search(
        rf"(?m)^{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*==",
        source,
    )
    assert declaration is not None, symbol
    next_declaration = re.search(
        r"(?m)^(?:[A-Za-z_][A-Za-z0-9_]*\s*"
        r"(?:\([^)=]*\))?\s*==|={4,}\s*$)",
        source[declaration.end() :],
    )
    operator_end = (
        len(source)
        if next_declaration is None
        else declaration.end() + next_declaration.start()
    )
    operator = source[declaration.end() : operator_end]
    assert operator.count(needle) == 1, (symbol, needle, operator.count(needle))
    position = source.find(needle, declaration.end(), operator_end)
    return source[:position] + replacement + source[position + len(needle) :]


def test_exact_wire_predicate_cannot_regress_to_dual_quorum(
    tmp_path: Path,
) -> None:
    """A mathematically sufficient signer superset is not an exact wire QC."""

    checker = load_checker()
    formal_dir = copy_formal_fixture(tmp_path)
    assert checker._exact_certificate_cardinality_source_fidelity_errors(
        formal_dir
    ) == []

    path = formal_dir / "SumeragiV2Inductive.tla"
    canonical = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_operator(
            canonical,
            "HistoricalQcValid",
            "ExactCertificateQuorum(qc.context.epoch, qc.signers)",
            "DualQuorum(qc.context.epoch, qc.signers)",
        ),
        encoding="utf-8",
    )
    errors = checker._exact_certificate_cardinality_source_fidelity_errors(
        formal_dir
    )
    assert any("HistoricalQcValid must match reviewed" in error for error in errors)


def test_qc_construction_cannot_serialize_raw_vote_pool(tmp_path: Path) -> None:
    """QC construction must project the canonical q-prefix of collected votes."""

    checker = load_checker()
    formal_dir = copy_formal_fixture(tmp_path)
    assert checker._exact_certificate_cardinality_source_fidelity_errors(
        formal_dir
    ) == []

    path = formal_dir / "SumeragiV2Core.tla"
    canonical = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_operator(
            canonical,
            "FormPrepareQC",
            "ProjectedVoteSignersAt(node, roundView, \"Prepare\", subject)",
            "VoteSignersAt(node, roundView, \"Prepare\", subject)",
        ),
        encoding="utf-8",
    )
    errors = checker._exact_certificate_cardinality_source_fidelity_errors(
        formal_dir
    )
    assert any("FormPrepareQC" in error for error in errors)


def test_canonical_projection_order_is_source_sealed(tmp_path: Path) -> None:
    """Flipping the roster-prefix order must invalidate the reviewed seal."""

    checker = load_checker()
    formal_dir = copy_formal_fixture(tmp_path)
    assert checker._exact_certificate_cardinality_source_fidelity_errors(
        formal_dir
    ) == []

    path = formal_dir / "SumeragiV2Quorums.tla"
    canonical = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_operator(
            canonical,
            "CanonicalCertificateSigners",
            "RosterIndex(epoch, other) <= RosterIndex(epoch, validator)",
            "RosterIndex(epoch, other) >= RosterIndex(epoch, validator)",
        ),
        encoding="utf-8",
    )
    errors = checker._exact_certificate_cardinality_source_fidelity_errors(
        formal_dir
    )
    assert any(
        "CanonicalCertificateSigners must match reviewed" in error
        for error in errors
    )
