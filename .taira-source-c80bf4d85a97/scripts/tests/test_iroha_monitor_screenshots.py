from __future__ import annotations

from pathlib import Path

import pytest

from scripts import check_iroha_monitor_screenshots as checker


def test_write_and_load_manifest_roundtrip(tmp_path: Path) -> None:
    asset = tmp_path / "demo.svg"
    asset.write_text("<svg></svg>", encoding="utf-8")
    digest = checker.compute_sha256(asset)

    record = tmp_path / "checksums.json"
    checker.write_manifest(record, {asset.name: digest})

    manifest = checker.load_manifest(record)
    assert manifest == {asset.name: digest}


def test_resolve_expected_prefers_manifest_when_present(tmp_path: Path) -> None:
    record = tmp_path / "checksums.json"
    checker.write_manifest(record, {"first.txt": "hash1", "second.txt": "hash2"})

    expected = checker.resolve_expected(record, explicit=None)
    assert set(expected) == {"first.txt", "second.txt"}


def test_main_update_and_verify(tmp_path: Path) -> None:
    artefact_dir = tmp_path / "artefacts"
    artefact_dir.mkdir()
    overview = artefact_dir / "iroha_monitor_demo_overview.svg"
    pipeline = artefact_dir / "iroha_monitor_demo_pipeline.svg"
    overview.write_text("overview", encoding="utf-8")
    pipeline.write_text("pipeline", encoding="utf-8")
    overview_ans = artefact_dir / "iroha_monitor_demo_overview.ans"
    pipeline_ans = artefact_dir / "iroha_monitor_demo_pipeline.ans"
    overview_ans.write_text("overview ans", encoding="utf-8")
    pipeline_ans.write_text("pipeline ans", encoding="utf-8")

    record = artefact_dir / "checksums.json"
    # Update manifest
    rc_update = checker.main(
        [
            "--dir",
            str(artefact_dir),
            "--record",
            str(record),
            "--update",
        ]
    )
    assert rc_update == 0

    # Verify the recorded digests
    rc_check = checker.main(
        [
            "--dir",
            str(artefact_dir),
            "--record",
            str(record),
        ]
    )
    assert rc_check == 0
