"""Regression tests for the Taikai publisher sample planner."""

from __future__ import annotations

import importlib.util
import json
import stat
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "sdk" / "examples" / "taikai_publisher" / "bundle_sample.py"
SPEC = importlib.util.spec_from_file_location("taikai_publisher_bundle_sample", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
PUBLISHER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = PUBLISHER
SPEC.loader.exec_module(PUBLISHER)


def _write_config(path: Path, payload: Path, rendition: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "payload": str(payload),
                "event_id": "event",
                "stream_id": "stream",
                "rendition_id": rendition,
                "track": {
                    "kind": "video",
                    "codec": "av1-main",
                    "bitrate_kbps": 1_000,
                    "resolution": "1920x1080",
                },
                "segment": {
                    "sequence": 1,
                    "start_pts": 0,
                    "duration": 1_000,
                    "wallclock_unix_ms": 1,
                },
                "ingest": {
                    "manifest_hash": "11" * 32,
                    "storage_ticket": "22" * 32,
                },
            }
        ),
        encoding="utf-8",
    )
    return path


def test_plan_rejects_cross_config_output_collision(tmp_path: Path) -> None:
    first_payload = tmp_path / "first" / "segment.m4s"
    second_payload = tmp_path / "second" / "segment.m4s"
    first_payload.parent.mkdir()
    second_payload.parent.mkdir()
    first_payload.write_bytes(b"first")
    second_payload.write_bytes(b"second")
    first = _write_config(tmp_path / "first.json", first_payload, "1080p")
    second = _write_config(tmp_path / "second.json", second_payload, "720p")

    with pytest.raises(ValueError, match="aliases .* output"):
        PUBLISHER.plan_bundles([first, second], tmp_path / "out", None)


def test_plan_rejects_output_that_aliases_another_config_input(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    first_payload = tmp_path / "first.m4s"
    first_payload.write_bytes(b"first")
    later_payload = out_dir / "first.car"
    later_payload.write_bytes(b"later input")
    first = _write_config(tmp_path / "first.json", first_payload, "1080p")
    second = _write_config(tmp_path / "second.json", later_payload, "720p")

    with pytest.raises(ValueError, match="aliases payload"):
        PUBLISHER.plan_bundles([first, second], out_dir, None)


def test_plan_rejects_summary_target_symlink(tmp_path: Path) -> None:
    payload = tmp_path / "segment.m4s"
    payload.write_bytes(b"segment")
    config = _write_config(tmp_path / "config.json", payload, "1080p")
    target = tmp_path / "target.json"
    target.write_text("preserve\n", encoding="utf-8")
    summary = tmp_path / "summary.json"
    try:
        summary.symlink_to(target)
    except OSError as error:
        pytest.skip(f"symlink creation unavailable: {error}")

    with pytest.raises(ValueError, match="regular file or absent"):
        PUBLISHER.plan_bundles([config], tmp_path / "out", summary)
    assert target.read_text(encoding="utf-8") == "preserve\n"


def test_plan_rejects_bundle_output_symlink(tmp_path: Path) -> None:
    payload = tmp_path / "segment.m4s"
    payload.write_bytes(b"segment")
    config = _write_config(tmp_path / "config.json", payload, "1080p")
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    target = tmp_path / "target.car"
    target.write_text("preserve\n", encoding="utf-8")
    bundle = out_dir / "segment.car"
    try:
        bundle.symlink_to(target)
    except OSError as error:
        pytest.skip(f"symlink creation unavailable: {error}")

    with pytest.raises(ValueError, match="regular file or absent"):
        PUBLISHER.plan_bundles([config], out_dir, None)
    assert target.read_text(encoding="utf-8") == "preserve\n"


def test_write_summary_json_is_atomic_and_leaves_no_temporary_file(
    tmp_path: Path,
) -> None:
    summary = tmp_path / "summary.json"
    PUBLISHER.write_summary_json(summary, [{"event": "event"}])

    assert json.loads(summary.read_text(encoding="utf-8")) == [{"event": "event"}]
    assert stat.S_IMODE(summary.stat().st_mode) == 0o644
    assert not list(tmp_path.glob(".summary.json.tmp-*"))


def test_write_summary_json_preserves_existing_mode(tmp_path: Path) -> None:
    summary = tmp_path / "summary.json"
    summary.write_text("[]\n", encoding="utf-8")
    summary.chmod(0o640)

    PUBLISHER.write_summary_json(summary, [{"event": "event"}])

    assert stat.S_IMODE(summary.stat().st_mode) == 0o640
