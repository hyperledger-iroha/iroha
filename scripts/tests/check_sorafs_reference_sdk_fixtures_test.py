"""Adversarial tests for the release-wide SoraFS reference-SDK inventory."""

from __future__ import annotations

import importlib.util
import json
import os
import shutil
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "check_sorafs_reference_sdk_fixtures.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reference_sdk_fixtures",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)
REPO_ROOT = MODULE_PATH.parents[1]
CANCEL_FIXTURE_SUPPORT = (
    REPO_ROOT
    / "crates/iroha_data_model/src/testing/cancel_asset_lock.rs"
)
CANCEL_FIXTURE_GENERATOR = (
    REPO_ROOT
    / "crates/iroha_data_model/src/bin/cancel_asset_lock_fixtures.rs"
)
REFERENCE_INVENTORY_GENERATOR = (
    REPO_ROOT
    / "crates/sorafs_manifest/src/bin/generate_por_fixtures.rs"
)


def copy_fixture_set(tmp_path: Path) -> Path:
    """Copy only the closed release-wide fixture inventory."""

    source_root = MODULE.DEFAULT_INVENTORY.parent
    target_root = tmp_path / "sorafs_manifest"
    target_root.mkdir()
    paths = (
        set(MODULE.EXPECTED_PAYLOADS)
        | set(MODULE.EXPECTED_OUTCOMES)
    )
    for relative in paths:
        source = source_root / relative
        target = target_root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
    shutil.copy2(
        MODULE.DEFAULT_INVENTORY,
        target_root / MODULE.DEFAULT_INVENTORY.name,
    )
    return target_root / MODULE.DEFAULT_INVENTORY.name


def load_inventory(path: Path) -> dict[str, object]:
    """Load one temporary inventory."""

    return json.loads(path.read_text(encoding="utf-8"))


def write_inventory(path: Path, inventory: dict[str, object]) -> None:
    """Write one deterministic inventory mutation."""

    path.write_text(
        json.dumps(inventory, indent=2, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )


def test_checked_in_inventory_is_valid_signed_and_domain_complete() -> None:
    """The repository inventory passes every offline check."""

    assert MODULE.validate_inventory(MODULE.DEFAULT_INVENTORY) == []
    assert len(MODULE.EXPECTED_PAYLOADS) == 82
    assert len(MODULE.EXPECTED_OUTCOMES) == 32
    assert {
        row[0] for row in MODULE.EXPECTED_PAYLOADS.values()
    } == MODULE.REQUIRED_DOMAINS - {"reference_sdk"}
    assert {
        row[0] for row in MODULE.EXPECTED_OUTCOMES.values()
    } == MODULE.REQUIRED_OUTCOME_DOMAINS


def test_cancel_asset_lock_hard_cut_vectors_are_closed_and_boundary_typed() -> None:
    """The signed inventory carries the exact V1 CAS positive and negatives."""

    rows = {
        path: metadata
        for path, metadata in MODULE.EXPECTED_PAYLOADS.items()
        if path.startswith("appeal_finance/")
    }
    assert set(rows) == {
        "appeal_finance/cancel_asset_lock_v1.json",
        "appeal_finance/cancel_asset_lock_v1.to",
        "appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.json",
        "appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.to",
        "appeal_finance/negative/cancel_asset_lock_nested_escrow_id_v1.to",
        "appeal_finance/negative/cancel_asset_lock_noncanonical_quantity_v1.json",
        "appeal_finance/negative/cancel_asset_lock_zero_expected_v1.json",
        "appeal_finance/negative/cancel_asset_lock_zero_expected_v1.to",
    }
    assert {
        metadata[3]
        for metadata in rows.values()
    } == {
        "valid",
        "invalid_missing_expected_remaining_amount",
        "invalid_nested_escrow_id",
        "invalid_noncanonical_quantity",
        "invalid_zero_expected_remaining_amount",
    }
    assert set(MODULE.EXPECTED_CANCEL_ASSET_LOCK_JSON) == {
        path for path in rows if path.endswith(".json")
    }
    assert {
        path
        for path, metadata in MODULE.EXPECTED_OUTCOMES.items()
        if metadata[0] == "appeal_finance"
    } == {
        "reference_sdk/appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json",
        "reference_sdk/appeal_finance_cancel_asset_lock_zero_expected_negative_validation_outcome_v1.json",
    }


def test_cancel_asset_lock_substituted_escrow_id_is_rejected(
    tmp_path: Path,
) -> None:
    """A different scalar hash cannot pass as the closed V1 fixture."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = inventory_path.parent / "appeal_finance/cancel_asset_lock_v1.json"
    original = MODULE.EXPECTED_CANCEL_ASSET_LOCK_ESCROW_ID.encode("ascii")
    substituted = original.replace(b"73CCD4", b"83CCD4", 1)
    fixture_bytes = fixture.read_bytes()
    assert original in fixture_bytes
    fixture.write_bytes(fixture_bytes.replace(original, substituted, 1))

    errors = MODULE.validate_inventory(inventory_path)

    assert any(
        "CancelAssetLock escrow_id does not match its closed V1 vector" in error
        for error in errors
    )


def test_cancel_asset_lock_norito_vectors_are_closed_and_byte_exact() -> None:
    """The canonical scalar and rejected nested frames retain their exact bytes."""

    fixture_root = MODULE.DEFAULT_INVENTORY.parent
    vectors = {
        path: (fixture_root / path).read_bytes()
        for path in MODULE.EXPECTED_CANCEL_ASSET_LOCK_NORITO
    }

    assert vectors == MODULE.EXPECTED_CANCEL_ASSET_LOCK_NORITO
    canonical = vectors["appeal_finance/cancel_asset_lock_v1.to"]
    nested = vectors[
        "appeal_finance/negative/cancel_asset_lock_nested_escrow_id_v1.to"
    ]
    assert len(canonical) == 85
    assert len(nested) == 86
    assert canonical[40] == 0x20
    assert nested[40:42] == b"\x21\x20"


def test_cancel_asset_lock_norito_substitution_is_rejected(
    tmp_path: Path,
) -> None:
    """A signed path cannot substitute different bytes for either closed frame."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = (
        inventory_path.parent
        / "appeal_finance/negative/cancel_asset_lock_nested_escrow_id_v1.to"
    )
    mutated = bytearray(fixture.read_bytes())
    mutated[42] ^= 1
    fixture.write_bytes(mutated)

    errors = MODULE.validate_inventory(inventory_path)

    assert any(
        "CancelAssetLock Norito bytes do not match the closed V1 vector" in error
        for error in errors
    )


def test_cancel_asset_lock_fixture_generator_is_atomic_and_fail_closed() -> None:
    """The typed generator cannot follow links or publish partial fixture bytes."""

    support = CANCEL_FIXTURE_SUPPORT.read_text(encoding="utf-8")
    command = CANCEL_FIXTURE_GENERATOR.read_text(encoding="utf-8")
    inventory = REFERENCE_INVENTORY_GENERATOR.read_text(encoding="utf-8")

    for marker in (
        "OpenOptions::new().write(true).create_new(true)",
        "temporary.sync_all()",
        "fs::rename(&temporary_path, path)",
        "fs::symlink_metadata",
        "ensure_single_hard_link",
        "ensure_same_directory",
        "Component::ParentDir",
        "contains unexpected entry",
    ):
        assert marker in support
    assert "fs::create_dir_all" not in support
    assert "fs::write(path, bytes)" not in support
    assert "write_fixtures(&args.output_dir, &fixtures)" in command

    for relative in (
        "appeal_finance/cancel_asset_lock_v1.json",
        "appeal_finance/cancel_asset_lock_v1.to",
        "appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.json",
        "appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.to",
        "appeal_finance/negative/cancel_asset_lock_nested_escrow_id_v1.to",
        "appeal_finance/negative/cancel_asset_lock_noncanonical_quantity_v1.json",
        "appeal_finance/negative/cancel_asset_lock_zero_expected_v1.json",
        "appeal_finance/negative/cancel_asset_lock_zero_expected_v1.to",
    ):
        assert relative in inventory
        assert relative in MODULE.EXPECTED_PAYLOADS


def test_payload_byte_tamper_is_rejected(tmp_path: Path) -> None:
    """A Norito mutation cannot retain its signed byte binding."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = inventory_path.parent / "por/proof_v1.to"
    fixture.write_bytes(fixture.read_bytes() + b"\x00")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("byte_length does not match" in error for error in errors)
    assert any("sha256 does not match" in error for error in errors)


def test_outcome_tamper_is_rejected(tmp_path: Path) -> None:
    """A golden ValidationOutcomeV1 cannot be changed under its binding."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = (
        inventory_path.parent
        / "reference_sdk/bundle_heterogeneous_positive_validation_outcome_v1.json"
    )
    fixture.write_bytes(fixture.read_bytes().replace(b"SFS-PDP-DIAG-000", b"SFS-PDP-DIAG-999"))

    errors = MODULE.validate_inventory(inventory_path)

    assert any("sha256 does not match" in error for error in errors)
    assert any("status/code does not match" in error for error in errors)


def test_noncanonical_outcome_layout_is_rejected(tmp_path: Path) -> None:
    """Outcome goldens use one exact pretty-JSON representation."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = (
        inventory_path.parent
        / "reference_sdk/bundle_routing_admission_positive_validation_outcome_v1.json"
    )
    fixture.write_bytes(fixture.read_bytes() + b"\n")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("canonical checked-in bytes" in error for error in errors)


def test_payload_duplicate_json_key_is_rejected(tmp_path: Path) -> None:
    """JSON commentary cannot shadow a field."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = inventory_path.parent / "por/challenge_v1.json"
    text = fixture.read_text(encoding="utf-8")
    fixture.write_text(text.replace("{", '{"version": 1,', 1), encoding="utf-8")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("duplicate JSON key `version`" in error for error in errors)


def test_payload_nonfinite_number_is_rejected(tmp_path: Path) -> None:
    """NaN is not valid signed fixture JSON."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = inventory_path.parent / "por/challenge_v1.json"
    text = fixture.read_text(encoding="utf-8")
    fixture.write_text(
        text.replace('"epoch_id": 1700000', '"epoch_id": NaN', 1),
        encoding="utf-8",
    )

    errors = MODULE.validate_inventory(inventory_path)

    assert any("non-finite JSON number `NaN`" in error for error in errors)


def test_inventory_signature_tamper_is_rejected(tmp_path: Path) -> None:
    """Every unsigned field is covered by the Ed25519 signature."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    inventory["scope"] = "substituted"
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("inventory.scope" in error for error in errors)
    assert any("Ed25519 signature is invalid" in error for error in errors)


def test_inventory_unknown_field_is_rejected(tmp_path: Path) -> None:
    """The V1 schema is closed."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    inventory["extension"] = True
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("fields must match V1" in error for error in errors)


def test_inventory_duplicate_json_key_is_rejected(tmp_path: Path) -> None:
    """Duplicate top-level keys cannot alter signed interpretation."""

    inventory_path = copy_fixture_set(tmp_path)
    text = inventory_path.read_text(encoding="utf-8")
    text = text.replace(
        '  "scope": "sorafs_v1_release",',
        '  "scope": "sorafs_v1_release",\n'
        '  "scope": "sorafs_v1_release",',
        1,
    )
    inventory_path.write_text(text, encoding="utf-8")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("duplicate JSON key `scope`" in error for error in errors)


def test_inventory_noncanonical_layout_is_rejected(tmp_path: Path) -> None:
    """Whitespace changes are rejected even when JSON semantics remain."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory_path.write_text(
        json.dumps(load_inventory(inventory_path), separators=(",", ":")),
        encoding="utf-8",
    )

    errors = MODULE.validate_inventory(inventory_path)

    assert any("canonical checked-in JSON bytes" in error for error in errors)


def test_path_traversal_is_rejected(tmp_path: Path) -> None:
    """A signed row cannot escape the fixture root."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    payloads = inventory["payloads"]
    assert isinstance(payloads, list)
    payloads[0]["path"] = "../outside.to"
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("canonical repository-relative ASCII" in error for error in errors)


def test_duplicate_inventory_path_is_rejected(tmp_path: Path) -> None:
    """Repeated rows cannot shadow the closed list."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    payloads = inventory["payloads"]
    assert isinstance(payloads, list)
    payloads.append(dict(payloads[0]))
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("unique, sorted, and exactly match" in error for error in errors)
    assert any("Ed25519 signature is invalid" in error for error in errors)


def test_unsorted_inventory_path_is_rejected(tmp_path: Path) -> None:
    """The signed order is canonical."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    outcomes = inventory["outcomes"]
    assert isinstance(outcomes, list)
    outcomes[0], outcomes[1] = outcomes[1], outcomes[0]
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("unique, sorted, and exactly match" in error for error in errors)


def test_row_metadata_substitution_is_rejected(tmp_path: Path) -> None:
    """A path cannot be relabelled as another validation kind."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    payloads = inventory["payloads"]
    assert isinstance(payloads, list)
    payloads[0]["domain"] = "routing"
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("metadata does not match the closed V1 row" in error for error in errors)


def test_symlink_fixture_is_rejected(tmp_path: Path) -> None:
    """A manifest path cannot be redirected through a symlink."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = inventory_path.parent / "potr/receipt_v1.to"
    replacement = tmp_path / "replacement.to"
    replacement.write_bytes(fixture.read_bytes())
    fixture.unlink()
    fixture.symlink_to(replacement)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("regular non-symlink file" in error for error in errors)


def test_symlink_parent_directory_is_rejected(tmp_path: Path) -> None:
    """Directory substitution is stopped by no-follow traversal."""

    inventory_path = copy_fixture_set(tmp_path)
    directory = inventory_path.parent / "moderation"
    replacement = tmp_path / "replacement-moderation"
    directory.rename(replacement)
    directory.symlink_to(replacement, target_is_directory=True)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("cannot be opened safely" in error for error in errors)


def test_hardlinked_fixture_is_rejected(tmp_path: Path) -> None:
    """Inventoried artifacts have exactly one filesystem name."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = inventory_path.parent / "repair/task_v1.to"
    os.link(fixture, tmp_path / "repair-hardlink.to")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("must have exactly one hard link" in error for error in errors)


def test_hardlinked_inventory_is_rejected(tmp_path: Path) -> None:
    """The signed root inventory has exactly one filesystem name."""

    inventory_path = copy_fixture_set(tmp_path)
    os.link(inventory_path, tmp_path / "inventory-hardlink.json")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("must have exactly one hard link" in error for error in errors)


def test_missing_fixture_is_rejected(tmp_path: Path) -> None:
    """Every closed path must be present."""

    inventory_path = copy_fixture_set(tmp_path)
    (inventory_path.parent / "replication_order/order_v1.to").unlink()

    errors = MODULE.validate_inventory(inventory_path)

    assert any("cannot be inspected" in error for error in errors)


def test_extra_owned_fixture_is_rejected(tmp_path: Path) -> None:
    """New generated directories cannot contain unreviewed artifacts."""

    inventory_path = copy_fixture_set(tmp_path)
    (
        inventory_path.parent / "reference_sdk/unreviewed.json"
    ).write_text("{}\n", encoding="utf-8")

    errors = MODULE.validate_inventory(inventory_path)

    assert any("reference_sdk fixture directory must be path-closed" in error for error in errors)


def test_generated_at_substitution_is_rejected(tmp_path: Path) -> None:
    """Golden timestamps are exact, not merely syntactically valid."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = (
        inventory_path.parent
        / "reference_sdk/bundle_pdp_wrong_provider_negative_validation_outcome_v1.json"
    )
    outcome = json.loads(fixture.read_text(encoding="utf-8"))
    outcome["generated_at"] = 1_700_001_235
    fixture.write_text(
        json.dumps(outcome, indent=2, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )

    errors = MODULE.validate_inventory(inventory_path)

    assert any("generated_at must be the closed value" in error for error in errors)


def test_bundle_payload_code_substitution_is_rejected(tmp_path: Path) -> None:
    """A bundle-level error retains its exact underlying payload failure."""

    inventory_path = copy_fixture_set(tmp_path)
    fixture = (
        inventory_path.parent
        / "reference_sdk/bundle_orderbook_bad_signature_negative_validation_outcome_v1.json"
    )
    outcome = json.loads(fixture.read_text(encoding="utf-8"))
    payload_code = next(
        row for row in outcome["context"] if row["key"] == "payload_code"
    )
    payload_code["value"] = "SFS-NORITO-001"
    fixture.write_text(
        json.dumps(outcome, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    errors = MODULE.validate_inventory(inventory_path)

    assert any("payload_code must be `SFS-SIG-007`" in error for error in errors)


def test_untrusted_key_fingerprint_is_rejected(tmp_path: Path) -> None:
    """Only the deterministic checked-in fixture key is accepted."""

    inventory_path = copy_fixture_set(tmp_path)
    inventory = load_inventory(inventory_path)
    signature = inventory["signature"]
    assert isinstance(signature, dict)
    signature["public_key_fingerprint_sha256"] = "00" * 32
    write_inventory(inventory_path, inventory)

    errors = MODULE.validate_inventory(inventory_path)

    assert any("fingerprint is not trusted" in error for error in errors)
    assert any("fingerprint does not bind" in error for error in errors)
