"""Focused coverage for split proof-ledger checker retention."""

from pathlib import Path

import pytest

from pytests.scripts import sumeragi_v2_release_receipt_test as receipt


def test_run_writer_copies_declared_checker_components_and_fails_closed(
    tmp_path: Path,
) -> None:
    """Retain every declared component and reject an absent component."""
    evidence = receipt.make_evidence(tmp_path)
    writer = receipt.fixture_writer(tmp_path)
    source_root = writer.parent.parent
    checker = (
        source_root
        / "scripts"
        / "formal"
        / "check_sumeragi_v2_proof_ledger.py"
    )
    component_name = "sumeragi_v2_proof_ledger_source_seal_contracts.py"
    checker_source = checker.read_text(encoding="utf-8")
    assert checker_source.count("_CHECKER_COMPONENT_FILES = ()") == 1
    checker.write_text(
        checker_source.replace(
            "_CHECKER_COMPONENT_FILES = ()",
            f'_CHECKER_COMPONENT_FILES = ("{component_name}",)',
        ),
        encoding="utf-8",
    )
    component = checker.with_name(component_name)
    component.write_text("# isolated checker component\n", encoding="utf-8")

    result = receipt.run_writer(
        evidence, receipt.terminal_output_path(evidence), writer
    )

    assert result.returncode == 0, result.stderr
    release_root = evidence["release_root"]
    assert isinstance(release_root, Path)
    retained = release_root / "scripts" / "formal" / component_name
    assert retained.read_bytes() == component.read_bytes()

    component.unlink()
    with pytest.raises(
        FileNotFoundError, match="proof-ledger checker component is unavailable"
    ):
        receipt.run_writer(
            evidence, receipt.terminal_output_path(evidence), writer
        )
