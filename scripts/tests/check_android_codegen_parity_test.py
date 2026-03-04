"""Tests for scripts/check_android_codegen_parity.py."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

MODULE_PATH = (Path(__file__).resolve().parents[1] / "check_android_codegen_parity.py")
SPEC = importlib.util.spec_from_file_location("check_android_codegen_parity", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
SPEC.loader.exec_module(MODULE)


def _write_manifest(root: Path, filename: str, key: str, entries: list[dict]) -> Path:
    path = root / filename
    path.write_text(json.dumps({key: entries}, indent=2), encoding="utf-8")
    return path


def _manifest_sha(payload: dict) -> str:
    canonical = dict(payload)
    canonical["generated_at"] = canonical.get("generated_at", "")
    canonical["instructions"] = sorted(canonical.get("instructions", []), key=lambda entry: entry.get("discriminant", ""))
    return MODULE._canonical_sha256(canonical)  # type: ignore[attr-defined]


def _builder_sha(payload: dict) -> str:
    canonical = dict(payload)
    canonical["generated_at"] = canonical.get("generated_at", "")
    canonical["builders"] = sorted(canonical.get("builders", []), key=lambda entry: entry.get("discriminant", ""))
    return MODULE._canonical_sha256(canonical)  # type: ignore[attr-defined]


def test_parity_success(tmp_path: Path) -> None:
    manifest = _write_manifest(tmp_path, "manifest.json", "instructions", [{"discriminant": "alpha"}])
    builder_index = _write_manifest(tmp_path, "builders.json", "builders", [{"builder": "alpha"}])

    manifest_payload = json.loads(manifest.read_text(encoding="utf-8"))
    builder_payload = json.loads(builder_index.read_text(encoding="utf-8"))
    metadata = {
        "instruction_manifest": {
            "sha256": _manifest_sha(manifest_payload),
            "entry_count": 1,
        },
        "builder_index": {
            "sha256": _builder_sha(builder_payload),
            "entry_count": 1,
        },
    }
    metadata_path = tmp_path / "metadata.json"
    metadata_path.write_text(json.dumps(metadata, indent=2), encoding="utf-8")

    summary_path = tmp_path / "summary.json"
    exit_code = MODULE.main(
        [
            "--manifest",
            str(manifest),
            "--builder-index",
            str(builder_index),
            "--metadata",
            str(metadata_path),
            "--json-out",
            str(summary_path),
            "--quiet",
        ]
    )
    assert exit_code == 0
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["status"] == "ok"
    assert summary["errors"] == []


def test_parity_failure(tmp_path: Path) -> None:
    manifest = _write_manifest(tmp_path, "manifest.json", "instructions", [{"discriminant": "alpha"}])
    builder_index = _write_manifest(tmp_path, "builders.json", "builders", [{"builder": "alpha"}])

    metadata = {
        "instruction_manifest": {
            "sha256": "deadbeef",
            "entry_count": 2,
        },
        "builder_index": {"sha256": "deadbeef", "entry_count": 1},
    }
    metadata_path = tmp_path / "metadata.json"
    metadata_path.write_text(json.dumps(metadata, indent=2), encoding="utf-8")

    exit_code = MODULE.main(
        [
            "--manifest",
            str(manifest),
            "--builder-index",
            str(builder_index),
            "--metadata",
            str(metadata_path),
            "--quiet",
        ]
    )
    assert exit_code == 1
