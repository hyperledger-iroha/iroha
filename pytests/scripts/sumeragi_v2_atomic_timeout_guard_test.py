"""Mutation tests for the atomic local-timeout completion source contract."""

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


def load_checker():
    """Load the proof-ledger checker from the worktree under test."""

    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_atomic_timeout_guard_checker", CHECKER
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def copy_formal_fixture(tmp_path: Path) -> Path:
    """Copy only the two modules consumed by the focused source checker."""

    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    source_dir = ROOT_DIR / "formal" / "sumeragi_v2"
    for name in ("SumeragiV2Core.tla", "SumeragiV2AsyncNetwork.tla"):
        shutil.copyfile(source_dir / name, formal_dir / name)
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
    position = source.find(needle, declaration.end(), operator_end)
    assert position >= 0, (symbol, needle)
    return source[:position] + replacement + source[position + len(needle) :]


def test_atomic_timeout_guard_rejects_ownership_and_current_vote_mutations(
    tmp_path: Path,
) -> None:
    """Deleting sole ownership or current-vote binding must fail closed."""

    module = load_checker()
    formal_dir = copy_formal_fixture(tmp_path)
    core_path = formal_dir / "SumeragiV2Core.tla"
    canonical = core_path.read_text(encoding="utf-8")
    assert module._atomic_timeout_completion_source_fidelity_errors(
        formal_dir
    ) == []

    for fragment in (
        "     /\\ node \\notin PendingNodes\n",
        "     /\\ vote = LocalTimeoutVoteFor(node)\n",
        "     /\\ vote.context = context\n",
    ):
        core_path.write_text(
            mutate_operator(
                canonical,
                "LocalTimeoutCompletionGuard",
                fragment,
                "",
            ),
            encoding="utf-8",
        )
        errors = module._atomic_timeout_completion_source_fidelity_errors(
            formal_dir
        )
        assert any(
            "LocalTimeoutCompletionGuard must equal only the exact reviewed"
            in error
            for error in errors
        ), errors


def test_atomic_timeout_guard_call_sites_are_exact(
    tmp_path: Path,
) -> None:
    """The Core action and async readiness alias must not bypass the guard."""

    module = load_checker()
    formal_dir = copy_formal_fixture(tmp_path)
    core_path = formal_dir / "SumeragiV2Core.tla"
    network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    canonical_core = core_path.read_text(encoding="utf-8")
    canonical_network = network_path.read_text(encoding="utf-8")
    assert module._atomic_timeout_completion_source_fidelity_errors(
        formal_dir
    ) == []

    core_path.write_text(
        mutate_operator(
            canonical_core,
            "CompleteTimeoutSignature",
            "  IN /\\ LocalTimeoutCompletionGuard(request)\n",
            "  IN /\\ TRUE\n",
        ),
        encoding="utf-8",
    )
    errors = module._atomic_timeout_completion_source_fidelity_errors(formal_dir)
    assert any(
        "CompleteTimeoutSignature must invoke "
        "LocalTimeoutCompletionGuard(request) exactly once"
        in error
        for error in errors
    ), errors
    core_path.write_text(canonical_core, encoding="utf-8")

    network_path.write_text(
        mutate_operator(
            canonical_network,
            "CompleteTimeoutSignatureReady",
            "  LocalTimeoutCompletionGuard(request)\n",
            "  TRUE\n",
        ),
        encoding="utf-8",
    )
    errors = module._atomic_timeout_completion_source_fidelity_errors(formal_dir)
    assert any(
        "CompleteTimeoutSignatureReady must equal only "
        "'LocalTimeoutCompletionGuard(request)'"
        in error
        for error in errors
    ), errors


def test_atomic_timeout_kernels_reject_projection_owner_and_survivor_mutations(
    tmp_path: Path,
) -> None:
    """Atomic certificate/install kernels must reject semantic shortcuts."""

    module = load_checker()
    formal_dir = copy_formal_fixture(tmp_path)
    core_path = formal_dir / "SumeragiV2Core.tla"
    canonical = core_path.read_text(encoding="utf-8")
    assert module._atomic_timeout_completion_source_fidelity_errors(
        formal_dir
    ) == []

    mutations = (
        (
            "TimeoutCertificateAfterReceipt",
            "  TC(context, vote.view,\n",
            "  TC(context, nodeView[node],\n",
        ),
        (
            "TimeoutCertificateAfterReceipt",
            "     TimeoutVotesIn(TimeoutReceiptsAfter(node, vote), "
            "node, vote.view))\n",
            "     TimeoutVotesAt(node, vote.view))\n",
        ),
        (
            "TimeoutInstallRequestAfterReceipt",
            "  InstallTcWal(node, TimeoutCertificateAfterReceipt(node, vote), "
            "TRUE)\n",
            "  InstallTcWal(vote.signer, "
            "TimeoutCertificateAfterReceipt(node, vote), TRUE)\n",
        ),
        (
            "TimeoutReceiptSurvivesInstall",
            "  \\/ received.node # node\n",
            "  \\/ TRUE\n",
        ),
    )
    for symbol, needle, replacement in mutations:
        core_path.write_text(
            mutate_operator(canonical, symbol, needle, replacement),
            encoding="utf-8",
        )
        errors = module._atomic_timeout_completion_source_fidelity_errors(
            formal_dir
        )
        assert any(
            f"{symbol} must equal only the exact reviewed atomic timeout"
            in error
            for error in errors
        ), errors
