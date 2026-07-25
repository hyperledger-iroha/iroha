"""Adversarial tests for the Governance DAG cross-SDK fixture inventory."""

from __future__ import annotations

import importlib.util
import json
import os
import shutil
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "check_sorafs_governance_sdk_fixtures.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_governance_sdk_fixtures",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def copy_fixture_set(tmp_path: Path) -> Path:
    """Copy the exact checked-in governance SDK fixture inventory."""

    source = MODULE.DEFAULT_INVENTORY.parent
    target = tmp_path / "governance"
    shutil.copytree(source, target)
    return target / MODULE.DEFAULT_INVENTORY.name


def load_inventory(path: Path) -> dict[str, object]:
    """Load one temporary inventory."""

    return json.loads(path.read_text(encoding="utf-8"))


def write_inventory(path: Path, inventory: dict[str, object]) -> None:
    """Write a deterministic temporary inventory mutation."""

    path.write_text(
        json.dumps(inventory, indent=2, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )


def test_checked_in_inventory_is_valid_and_signed() -> None:
    """The repository inventory must pass the offline verifier."""

    assert MODULE.validate_inventory(MODULE.DEFAULT_INVENTORY) == []
    assert len(MODULE.EXPECTED_PAYLOADS) == 17
    assert len(MODULE.EXPECTED_OUTCOMES) == 8
    assert (
        sum(
            spec[1] == "norito"
            for spec in MODULE.EXPECTED_PAYLOADS.values()
        )
        == 9
    )
    assert (
        sum(
            spec[1] == "json"
            for spec in MODULE.EXPECTED_PAYLOADS.values()
        )
        == 8
    )


def test_fixture_byte_tamper_is_rejected(tmp_path: Path) -> None:
    """A payload mutation cannot retain its checked-in digest binding."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = inventory_path.parent / "dag_block_0_v1.to"
    fixture.write_bytes(fixture.read_bytes() + b"\x00")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("byte_length does not match" in error for error in errors)
    assert any("sha256 does not match" in error for error in errors)


def test_json_sidecar_duplicate_key_is_rejected(tmp_path: Path) -> None:
    """Payload commentary cannot shadow a field inside canonical JSON."""

    inventory_path = copy_fixture_set(tmp_path)
    sidecar = inventory_path.parent / "dag_block_0_v1.json"
    lines = sidecar.read_text(encoding="utf-8").splitlines(keepends=True)
    assert lines[1].lstrip().startswith('"block_cid_hex"')
    lines.insert(2, lines[1])
    sidecar.write_text("".join(lines), encoding="utf-8")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("duplicate JSON key `block_cid_hex`" in error for error in errors)


def test_json_sidecar_nonfinite_number_is_rejected(tmp_path: Path) -> None:
    """Payload commentary rejects JavaScript-style non-finite numbers."""

    inventory_path = copy_fixture_set(tmp_path)
    sidecar = inventory_path.parent / "dag_block_0_v1.json"
    text = sidecar.read_text(encoding="utf-8")
    sidecar.write_text(
        text.replace('"node_timestamp": 1700000790', '"node_timestamp": NaN', 1),
        encoding="utf-8",
    )

    errors = MODULE.validate_inventory(inventory_path)

    assert any("non-finite JSON number `NaN` is forbidden" in error for error in errors)


def test_json_sidecar_noncanonical_layout_is_rejected(tmp_path: Path) -> None:
    """Payload commentary must match the generator's exact pretty bytes."""

    inventory_path = copy_fixture_set(tmp_path)
    sidecar = inventory_path.parent / "dag_head_v1.json"
    sidecar.write_bytes(sidecar.read_bytes() + b"\n")

    errors = MODULE.validate_inventory(inventory_path)

    assert any(
        "JSON sidecar must use canonical sorted pretty bytes" in error
        for error in errors
    )


def test_hardlinked_fixture_is_rejected(tmp_path: Path) -> None:
    """A fixture must not have another filesystem name."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = inventory_path.parent / "dag_block_0_v1.to"
    os.link(fixture, tmp_path / "fixture-hardlink.to")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("fixture must have exactly one hard link" in error for error in errors)


def test_hardlinked_inventory_is_rejected(tmp_path: Path) -> None:
    """The signed inventory itself must have one filesystem name."""

    inventory_path = copy_fixture_set(tmp_path)
    os.link(inventory_path, tmp_path / "inventory-hardlink.json")

    errors = MODULE.validate_inventory(inventory_path)

    assert any(
        "fixture inventory must have exactly one hard link" in error
        for error in errors
    )


def test_missing_fixture_is_rejected(tmp_path: Path) -> None:
    """Every closed-inventory payload must exist as a regular file."""

    inventory_path = copy_fixture_set(tmp_path)
    (inventory_path.parent / "dag_head_v1.json").unlink()

    errors = MODULE.validate_inventory(inventory_path)

    assert any("cannot be inspected" in error for error in errors)
    assert any("exact signed artifact inventory" in error for error in errors)


def test_extra_fixture_is_rejected(tmp_path: Path) -> None:
    """Unmanifested payloads cannot enter the SDK fixture directory."""

    inventory_path = copy_fixture_set(tmp_path)
    (inventory_path.parent / "unreviewed.to").write_bytes(b"unreviewed")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("exact signed artifact inventory" in error for error in errors)


def test_duplicate_inventory_path_is_rejected(tmp_path: Path) -> None:
    """Repeated rows cannot inflate or shadow the closed inventory."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    payloads = inventory["payloads"]
    assert isinstance(payloads, list)
    payloads.append(dict(payloads[0]))
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("unique, sorted, and exactly match" in error for error in errors)
    assert any("Ed25519 signature is invalid" in error for error in errors)


def test_path_traversal_is_rejected_before_file_access(tmp_path: Path) -> None:
    """An inventory row cannot escape the fixture directory."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    payloads = inventory["payloads"]
    assert isinstance(payloads, list)
    payloads[0]["path"] = "../dag_block_0_v1.to"
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("exact canonical basename" in error for error in errors)


def test_duplicate_json_key_is_rejected(tmp_path: Path) -> None:
    """JSON key shadowing cannot alter the signed interpretation."""

    inventory_path = copy_fixture_set(tmp_path)
    text = inventory_path.read_text(encoding="utf-8")
    text = text.replace(
        '  "scope": "governance_sdk_subset",',
        '  "scope": "governance_sdk_subset",\n'
        '  "scope": "governance_sdk_subset",',
        1,
    )
    inventory_path.write_text(text, encoding="utf-8")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("duplicate JSON key `scope`" in error for error in errors)


def test_nonfinite_json_number_is_rejected(tmp_path: Path) -> None:
    """NaN and infinity literals never enter the signed JSON interpretation."""

    inventory_path = copy_fixture_set(tmp_path)
    text = inventory_path.read_text(encoding="utf-8")
    prefix, marker, suffix = text.partition('"byte_length": ')
    assert marker
    _value, newline, remainder = suffix.partition("\n")
    assert newline
    inventory_path.write_text(
        prefix + marker + "NaN" + newline + remainder,
        encoding="utf-8",
    )

    errors = MODULE.validate_inventory(inventory_path)

    assert any("non-finite JSON number `NaN` is forbidden" in error for error in errors)


def test_noncanonical_inventory_layout_is_rejected(tmp_path: Path) -> None:
    """Semantic and signature validity cannot excuse noncanonical JSON bytes."""

    inventory_path = copy_fixture_set(tmp_path)
    canonical = inventory_path.read_text(encoding="utf-8")
    assert canonical.endswith("\n")
    inventory_path.write_text(canonical[:-1], encoding="utf-8")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("canonical checked-in layout" in error for error in errors)


def test_noncanonical_inventory_field_order_is_rejected(tmp_path: Path) -> None:
    """A different field order cannot pass merely because signing sorts keys."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    inventory_path.write_text(
        json.dumps(inventory, indent=2, ensure_ascii=True, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    errors = MODULE.validate_inventory(inventory_path)

    assert any("canonical checked-in layout" in error for error in errors)
    assert not any("Ed25519 signature is invalid" in error for error in errors)


def test_symlinked_inventory_parent_is_rejected(tmp_path: Path) -> None:
    """The fixture root must be a directly opened directory, not a symlink."""

    source = MODULE.DEFAULT_INVENTORY.parent
    real_root = tmp_path / "real-governance"
    shutil.copytree(source, real_root)
    linked_root = tmp_path / "governance"
    linked_root.symlink_to(real_root, target_is_directory=True)

    errors = MODULE.validate_inventory(
        linked_root / MODULE.DEFAULT_INVENTORY.name
    )

    assert any("fixture directory must not be a symlink" in error for error in errors)


def test_inventory_parent_replacement_during_scan_is_rejected(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Replacing the fixture root cannot redirect an in-flight validation."""

    inventory_path = copy_fixture_set(tmp_path)
    original_validate_entries = MODULE._validate_entries

    def validate_then_replace(inventory, fixture_root_fd, errors) -> None:
        original_validate_entries(inventory, fixture_root_fd, errors)
        moved_root = tmp_path / "governance-before-replacement"
        inventory_path.parent.rename(moved_root)
        shutil.copytree(moved_root, inventory_path.parent)

    monkeypatch.setattr(MODULE, "_validate_entries", validate_then_replace)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("fixture directory identity changed" in error for error in errors)


def test_schema_extra_is_rejected(tmp_path: Path) -> None:
    """Unknown fields fail the schema-closed contract."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    inventory["unreviewed"] = True
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("fields must match the V1 schema" in error for error in errors)


def test_payload_encoding_substitution_is_rejected(tmp_path: Path) -> None:
    """A JSON sidecar cannot be relabeled as canonical Norito."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    payloads = inventory["payloads"]
    assert isinstance(payloads, list)
    payloads[0]["encoding"] = "norito"
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any(
        "kind/encoding/signature expectation does not match" in error
        for error in errors
    )
    assert any("Ed25519 signature is invalid" in error for error in errors)


def test_signature_tamper_is_rejected(tmp_path: Path) -> None:
    """A well-shaped inventory with a substituted signature fails closed."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    signature = inventory["signature"]
    assert isinstance(signature, dict)
    signature["signature_hex"] = "00" * 64
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("Ed25519 signature is invalid" in error for error in errors)
