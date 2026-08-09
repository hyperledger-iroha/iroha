"""Validate the SoraFS multi-provider fixture replay for Android codegen."""

from __future__ import annotations

import base64
import json
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
REGISTER_PIN_EXAMPLE = (
    REPO_ROOT
    / "specs"
    / "sdk"
    / "android"
    / "generated"
    / "fixtures"
    / "sorafs_register_pin_manifest_multi_peer_parity_v1.json"
)
FIXTURE_DIR = REPO_ROOT / "fixtures" / "sorafs_orchestrator" / "multi_peer_parity_v1"


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def test_fixture_example_matches_metadata() -> None:
    fixture_example = load_json(REGISTER_PIN_EXAMPLE)

    metadata = load_json(FIXTURE_DIR / "metadata.json")
    assert fixture_example["fixture"] == metadata["fixture"]
    assert fixture_example["plan_file"] == metadata["plan_file"]
    assert fixture_example["providers_file"] == metadata["providers_file"]

    instruction = fixture_example["instruction"]
    plan = load_json(FIXTURE_DIR / metadata["plan_file"])
    assert plan["schema"] == "sorafs.chunk_fetch_plan.v1"
    assert len(plan["payload_digest_blake3_hex"]) == 64
    plan_digests = [
        entry["digest_blake3"] for entry in plan["chunk_fetch_specs"]
    ]
    assert fixture_example["chunk_digests_blake3"] == plan_digests
    assert set(instruction) == {
        "manifest_payload_base64",
        "alias",
        "successor_of",
    }
    manifest_payload = base64.b64decode(
        instruction["manifest_payload_base64"],
        validate=True,
    )
    assert 0 < len(manifest_payload) <= 512 * 1024
    assert base64.b64encode(manifest_payload).decode("ascii") == instruction["manifest_payload_base64"]

    manifest_report_path = fixture_example["manifest_report_path"]
    assert manifest_report_path.startswith(
        "target-codex/android_codegen/sorafs_manifest/"
    )


def test_fixture_example_uses_the_repository_local_specs_source() -> None:
    """Keep the replay guard on the implementation-coupled SDK fixture."""

    assert REGISTER_PIN_EXAMPLE == (
        REPO_ROOT
        / "specs"
        / "sdk"
        / "android"
        / "generated"
        / "fixtures"
        / "sorafs_register_pin_manifest_multi_peer_parity_v1.json"
    )
    assert not (
        REPO_ROOT
        / "docs"
        / "source"
        / "sdk"
        / "android"
        / "generated"
        / "fixtures"
        / REGISTER_PIN_EXAMPLE.name
    ).exists()
