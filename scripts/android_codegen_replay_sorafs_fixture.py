#!/usr/bin/env python3
"""
Replay the multi-provider SoraFS manifest fixture and inject a concrete example
into the Android codegen assets.

This script:
1. Runs `sorafs_manifest_stub` against the shared orchestrator fixture to
   generate a fresh manifest report.
2. Copies the report under `target-codex/android_codegen/sorafs_manifest/`.
3. Updates the RegisterPinManifest instruction example with a `fixture_example`
   payload that mirrors the generated manifest so Kotlin/Java builders can
   consume real-world data when replaying the fixture.
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import subprocess
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FIXTURE_DIR = REPO_ROOT / "fixtures/sorafs_orchestrator/multi_peer_parity_v1"
DEFAULT_CHUNKER_FIXTURE = REPO_ROOT / "fixtures/sorafs_chunker/sf1_profile_v1.json"
DEFAULT_REGISTER_PIN_EXAMPLE = (
    REPO_ROOT
    / "target-codex"
    / "android_codegen"
    / "instruction_examples"
    / "iroha_data_model::isi::sorafs::RegisterPinManifest.json"
)
DEFAULT_REPORT_DIR = (
    REPO_ROOT / "target-codex" / "android_codegen" / "sorafs_manifest"
)
DEFAULT_TRACKED_FIXTURE = (
    REPO_ROOT
    / "docs"
    / "source"
    / "sdk"
    / "android"
    / "generated"
    / "fixtures"
    / "sorafs_register_pin_manifest_multi_peer_parity_v1.json"
)


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")


def run_manifest_stub(
    cargo_bin: str,
    payload_path: Path,
    plan_path: Path,
    profile_handle: str,
    min_replicas: int,
    storage_class: str,
    retention_epoch: int,
    json_out: Path,
) -> None:
    cmd = [
        cargo_bin,
        "run",
        "--quiet",
        "-p",
        "sorafs_car",
        "--features",
        "cli",
        "--bin",
        "sorafs_manifest_stub",
        str(payload_path),
        f"--plan={plan_path}",
        f"--chunker-profile={profile_handle}",
        f"--min-replicas={min_replicas}",
        f"--storage-class={storage_class}",
        f"--retention-epoch={retention_epoch}",
        "--council-signature-public-key="
        "1111111111111111111111111111111111111111111111111111111111111111",
        "--council-signature="
        "2222222222222222222222222222222222222222222222222222222222222222"
        "2222222222222222222222222222222222222222222222222222222222222222",
        f"--json-out={json_out}",
    ]
    subprocess.run(cmd, check=True, cwd=REPO_ROOT)


def build_fixture_example(
    fixture_meta: dict,
    chunker_fixture: dict,
    manifest_report: dict,
    manifest_report_path: Path,
) -> dict:
    chunking = manifest_report["chunking"]
    manifest = manifest_report["manifest"]
    timestamp = datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")
    instruction = {
        "digest_hex": manifest["digest_hex"],
        "chunker": {
            "profile_id": chunking["profile_id"],
            "namespace": chunking["namespace"],
            "name": chunking["name"],
            "semver": chunking["semver"],
            "handle": chunking["handle"],
            "aliases": chunking["profile_aliases"],
            "multihash_code": chunking["multihash_code"],
        },
        "chunk_digest_sha3_256_hex": chunker_fixture["chunk_digest_sha3_256"],
        "policy": manifest["pin_policy"],
        "submitted_epoch": fixture_meta["now_unix_secs"],
        "alias": None,
        "successor_of": None,
    }
    example = {
        "fixture": fixture_meta["fixture"],
        "generated_at": timestamp,
        "plan_file": fixture_meta["plan_file"],
        "providers_file": fixture_meta["providers_file"],
        "telemetry_file": fixture_meta["telemetry_file"],
        "manifest_report_path": str(manifest_report_path.relative_to(REPO_ROOT)),
        "chunk_digests_blake3": [
            entry["digest_blake3"] for entry in manifest_report["chunk_digests"]
        ],
        "instruction": instruction,
    }
    return example


def update_register_pin_example(example_path: Path, fixture_example: dict) -> None:
    data = load_json(example_path)
    data["fixture_example"] = fixture_example
    write_json(example_path, data)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Replay the SoraFS orchestrator fixture for Android codegen."
    )
    parser.add_argument(
        "--fixture-dir",
        type=Path,
        default=DEFAULT_FIXTURE_DIR,
        help="Directory that hosts multi-provider orchestrator fixture JSON files.",
    )
    parser.add_argument(
        "--chunker-fixture",
        type=Path,
        default=DEFAULT_CHUNKER_FIXTURE,
        help="Chunker profile fixture that includes the SHA3 digest metadata.",
    )
    parser.add_argument(
        "--register-pin-example",
        type=Path,
        default=DEFAULT_REGISTER_PIN_EXAMPLE,
        help="Path to the RegisterPinManifest instruction example JSON.",
    )
    parser.add_argument(
        "--report-dir",
        type=Path,
        default=DEFAULT_REPORT_DIR,
        help="Directory for storing generated manifest reports.",
    )
    parser.add_argument(
        "--tracked-fixture-out",
        type=Path,
        default=DEFAULT_TRACKED_FIXTURE,
        help="Tracked JSON file that mirrors the replayed fixture example.",
    )
    parser.add_argument(
        "--cargo-bin",
        default=os.environ.get("CARGO_BIN", "cargo"),
        help="Cargo binary to invoke (defaults to `cargo`).",
    )
    args = parser.parse_args()

    fixture_meta = load_json(args.fixture_dir / "metadata.json")
    chunker_fixture = load_json(args.chunker_fixture)

    payload_path = (REPO_ROOT / fixture_meta["payload_path"]).resolve()
    if not payload_path.exists():
        raise SystemExit(f"payload path missing: {payload_path}")

    plan_path = (args.fixture_dir / fixture_meta["plan_file"]).resolve()
    if not plan_path.exists():
        raise SystemExit(f"plan file missing: {plan_path}")

    retention_epoch = fixture_meta.get("retention_epoch", fixture_meta["now_unix_secs"])
    report_path = args.report_dir / f"{fixture_meta['fixture']}.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_report = Path(tmpdir) / "manifest_report.json"
        run_manifest_stub(
            args.cargo_bin,
            payload_path,
            plan_path,
            fixture_meta["profile_handle"],
            min_replicas=fixture_meta.get("min_replicas", 3),
            storage_class=fixture_meta.get("storage_class", "hot"),
            retention_epoch=retention_epoch,
            json_out=tmp_report,
        )
        manifest_report = load_json(tmp_report)
        write_json(report_path, manifest_report)

    fixture_example = build_fixture_example(
        fixture_meta,
        chunker_fixture,
        manifest_report,
        report_path,
    )
    update_register_pin_example(args.register_pin_example, fixture_example)
    write_json(args.tracked_fixture_out, fixture_example)
    print(
        f"[android-codegen] updated fixture example in "
        f"{args.register_pin_example.relative_to(REPO_ROOT)}"
    )
    print(
        f"[android-codegen] manifest report written to "
        f"{report_path.relative_to(REPO_ROOT)}"
    )
    print(
        f"[android-codegen] tracked fixture written to "
        f"{args.tracked_fixture_out.relative_to(REPO_ROOT)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
