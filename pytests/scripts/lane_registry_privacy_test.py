"""Tests for strict Nexus lane privacy manifest validation."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts.nexus.lane_registry_bundle import write_manifest
from scripts.nexus.lane_registry_privacy import summarize_merkle_privacy_commitments


def merkle_entry() -> dict:
    """Return a canonical first-release privacy commitment entry."""

    return {
        "id": 7,
        "scheme": "merkle",
        "merkle": {
            "root": "0x" + "ab" * 32,
            "max_depth": 16,
        },
    }


def test_merkle_privacy_commitment_is_summarized() -> None:
    manifest = {"privacy_commitments": [merkle_entry()]}
    assert summarize_merkle_privacy_commitments(manifest) == [
        {"id": 7, "scheme": "merkle"},
    ]


def test_snark_privacy_commitment_is_rejected() -> None:
    entry = merkle_entry()
    entry["scheme"] = "snark"
    entry["snark"] = {
        "circuit_id": 1,
        "verifying_key_digest": "00" * 32,
        "statement_hash": "00" * 32,
        "proof_hash": "00" * 32,
    }
    with pytest.raises(ValueError, match="unsupported fields: snark"):
        summarize_merkle_privacy_commitments({"privacy_commitments": [entry]})


def test_bundle_rejects_snark_before_writing_output(tmp_path: Path) -> None:
    entry = merkle_entry()
    entry["scheme"] = "snark"
    source = tmp_path / "source.json"
    source.write_text(
        json.dumps({"lane": "private", "privacy_commitments": [entry]}),
        encoding="utf-8",
    )
    output_dir = tmp_path / "manifests"

    with pytest.raises(ValueError, match="must be `merkle`"):
        write_manifest(source, output_dir, force=False)

    assert not (output_dir / "private.manifest.json").exists()


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("id", -1, "unsigned 16-bit integer"),
        ("scheme", "plonk", "must be `merkle`"),
        ("merkle", None, "must be an object"),
    ],
)
def test_invalid_privacy_commitment_is_rejected(
    field: str,
    value: object,
    message: str,
) -> None:
    entry = merkle_entry()
    entry[field] = value
    with pytest.raises(ValueError, match=message):
        summarize_merkle_privacy_commitments({"privacy_commitments": [entry]})
