"""Validate the SoraFS multi-provider fixture replay for Android codegen."""

from __future__ import annotations

import json
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
REGISTER_PIN_EXAMPLE = (
    REPO_ROOT
    / "docs"
    / "source"
    / "sdk"
    / "android"
    / "generated"
    / "fixtures"
    / "sorafs_register_pin_manifest_multi_peer_parity_v1.json"
)
FIXTURE_DIR = REPO_ROOT / "fixtures" / "sorafs_orchestrator" / "multi_peer_parity_v1"
CHUNKER_FIXTURE = REPO_ROOT / "fixtures" / "sorafs_chunker" / "sf1_profile_v1.json"


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def test_fixture_example_matches_metadata() -> None:
    fixture_example = load_json(REGISTER_PIN_EXAMPLE)

    metadata = load_json(FIXTURE_DIR / "metadata.json")
    chunker = load_json(CHUNKER_FIXTURE)

    assert fixture_example["fixture"] == metadata["fixture"]
    assert fixture_example["plan_file"] == metadata["plan_file"]
    assert fixture_example["providers_file"] == metadata["providers_file"]

    instruction = fixture_example["instruction"]
    plan = load_json(FIXTURE_DIR / metadata["plan_file"])
    plan_digests = [entry["digest_blake3"] for entry in plan]
    assert fixture_example["chunk_digests_blake3"] == plan_digests
    assert instruction["submitted_epoch"] == metadata["now_unix_secs"]
    assert (
        instruction["chunk_digest_sha3_256_hex"].lower()
        == chunker["chunk_digest_sha3_256"].lower()
    )

    chunker_info = instruction["chunker"]
    assert chunker_info["handle"] == metadata["profile_handle"]
    assert chunker_info["multihash_code"] == 31
    assert chunker_info["profile_id"] == 1

    policy = instruction["policy"]
    assert policy["min_replicas"] == 3
    assert policy["storage_class"] == "Hot"

    manifest_report_path = fixture_example["manifest_report_path"]
    assert manifest_report_path.startswith(
        "target-codex/android_codegen/sorafs_manifest/"
    )
