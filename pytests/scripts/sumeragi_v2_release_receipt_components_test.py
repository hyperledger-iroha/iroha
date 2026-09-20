"""Focused coverage for source-isolated release component retention."""

import importlib.util
from pathlib import Path
import sys
from types import ModuleType

import pytest

from pytests.scripts import sumeragi_v2_release_receipt_test as receipt


@pytest.fixture
def receipt_module(monkeypatch: pytest.MonkeyPatch) -> ModuleType:
    """Load the actual writer and its authenticated component sources."""
    spec = importlib.util.spec_from_file_location(
        "release_receipt_component_inventory_test", receipt.SCRIPT
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    monkeypatch.setitem(sys.modules, spec.name, module)
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize(
    "validator_name",
    ("_prebuilt_directory_inventory", "_require_g12_directory_inventory"),
)
def test_evidence_directory_inventory_accepts_current_artifact_names(
    tmp_path: Path, receipt_module: ModuleType, validator_name: str
) -> None:
    """Both evidence paths use the writer-owned safe component policy."""
    directory = tmp_path.resolve()
    names = {"iroha3d", "proof-v1.0.json", "g12_result.log"}
    for name in names:
        (directory / name).write_bytes(b"evidence\n")
    validator = getattr(receipt_module, validator_name)
    validator(directory, names, "release evidence")
    with pytest.raises(receipt_module.ReceiptError, match="inventory"):
        validator(directory, names | {"missing.json"}, "release evidence")


@pytest.mark.parametrize(
    "validator_name",
    ("_prebuilt_directory_inventory", "_require_g12_directory_inventory"),
)
@pytest.mark.parametrize("name", ("-option", ".hidden", "with space", "caf\u00e9"))
def test_evidence_directory_inventory_rejects_unsafe_artifact_names(
    tmp_path: Path, receipt_module: ModuleType, validator_name: str, name: str
) -> None:
    """An exact expected set never authorizes unsafe directory entry names."""
    directory = tmp_path.resolve()
    (directory / name).write_bytes(b"evidence\n")
    with pytest.raises(receipt_module.ReceiptError):
        getattr(receipt_module, validator_name)(directory, {name}, "release evidence")


def test_run_writer_copies_declared_components_and_fails_closed(
    tmp_path: Path,
) -> None:
    """Retain declared components and reject missing or symlinked sources."""
    evidence = receipt.make_evidence(tmp_path)
    writer = receipt.fixture_writer(tmp_path)
    source_root = writer.parent.parent
    inventory = (
        source_root
        / "scripts"
        / "formal"
        / "sumeragi_v2_proof_ledger_source_inventory.py"
    )
    component_name = "sumeragi_v2_proof_ledger_source_seal_contracts.py"
    inventory_source = inventory.read_text(encoding="utf-8")
    assert inventory_source.count('_CHECKER_COMPONENT_FILES = ("sumeragi_v2_proof_ledger_source_inventory.py",)') == 1
    inventory.write_text(
        inventory_source.replace(
            '_CHECKER_COMPONENT_FILES = ("sumeragi_v2_proof_ledger_source_inventory.py",)',
            f'_CHECKER_COMPONENT_FILES = ("sumeragi_v2_proof_ledger_source_inventory.py", "{component_name}")',
        ),
        encoding="utf-8",
    )
    component = inventory.with_name(component_name)
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
