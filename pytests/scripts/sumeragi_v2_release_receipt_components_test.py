"""Focused coverage for source-isolated release component retention."""

from pathlib import Path

import pytest

from pytests.scripts import sumeragi_v2_release_receipt_test as receipt


def test_run_writer_copies_declared_components_and_fails_closed(
    tmp_path: Path,
) -> None:
    """Retain declared components and reject missing or symlinked sources."""
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

    receipt_components = receipt.release_receipt_writer_components(source_root)
    assert receipt_components == (
        Path("scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py"),
        Path("scripts/write_sumeragi_v2_release_receipt_corridor_log.py"),
        Path("scripts/write_sumeragi_v2_release_receipt_gate_evidence.py"),
        Path("scripts/write_sumeragi_v2_release_receipt_publication.py"),
    )
    receipt_component = source_root / receipt_components[0]
    receipt_component_bytes = receipt_component.read_bytes()
    receipt_component.unlink()
    missing = receipt.run_writer(
        evidence, receipt.terminal_output_path(evidence), writer
    )
    assert missing.returncode != 0
    assert "release receipt component is unavailable" in missing.stderr

    external_component = tmp_path / "substituted-receipt-component.py"
    external_component.write_bytes(receipt_component_bytes)
    try:
        receipt_component.symlink_to(external_component)
    except (NotImplementedError, OSError) as error:
        pytest.fail(f"release test host cannot exercise symlink rejection: {error}")
    substituted = receipt.run_writer(
        evidence, receipt.terminal_output_path(evidence), writer
    )
    assert substituted.returncode != 0
    assert "release receipt component is unavailable" in substituted.stderr
    receipt_component.unlink()
    receipt_component.write_bytes(receipt_component_bytes)

    digest_bound = source_root / receipt_components[2]
    digest_bound_bytes = digest_bound.read_bytes()
    digest_bound.write_bytes(digest_bound_bytes + b"\n# substituted\n")
    wrong_digest = receipt.run_writer(
        evidence, receipt.terminal_output_path(evidence), writer
    )
    assert wrong_digest.returncode != 0
    assert "release receipt component has the wrong digest" in wrong_digest.stderr
    digest_bound.write_bytes(digest_bound_bytes)

    component.unlink()
    with pytest.raises(
        FileNotFoundError, match="proof-ledger checker component is unavailable"
    ):
        receipt.run_writer(
            evidence, receipt.terminal_output_path(evidence), writer
        )
