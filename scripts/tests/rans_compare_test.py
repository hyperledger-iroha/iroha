from __future__ import annotations

import importlib.util
import json
import os
import shlex
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "benchmarks" / "nsc" / "rans_compare.py"


@pytest.fixture(scope="module")
def rans_compare_module():
    spec = importlib.util.spec_from_file_location("rans_compare", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module  # ensure dataclasses resolve annotations
    spec.loader.exec_module(module)  # type: ignore[misc]
    return module


def test_load_tables_and_write_csv(tmp_path, rans_compare_module):
    sample = {
        "payload": {
            "version": 1,
            "generated_at": "2026-01-01T00:00:00Z",
            "generator_commit": "abc123",
            "checksum_sha256": [0] * 32,
            "body": {
                "seed": 42,
                "bundle_width": 3,
                "groups": [
                    {
                        "group_size": 4,
                        "precision_bits": 12,
                        "frequencies": [3, 5, 7],
                        "cumulative": [0, 3, 8, 15],
                    },
                    {
                        "group_size": 2,
                        "precision_bits": 10,
                        "frequencies": [11],
                        "cumulative": [0, 11],
                    },
                ],
            },
        }
    }
    artifact = tmp_path / "rans_tables.json"
    artifact.write_text(json.dumps(sample), encoding="utf-8")

    tables = rans_compare_module.load_tables(artifact)
    assert tables.version == 1
    assert len(tables.groups) == 2
    assert tables.groups[0].frequencies == [3, 5, 7]
    assert tables.seed == 42
    assert tables.bundle_width == 3

    csv_path = tmp_path / "out.csv"
    rans_compare_module.write_csv(tables, csv_path)
    csv_contents = csv_path.read_text(encoding="utf-8").splitlines()
    assert csv_contents[0] == "group_size,symbol_index,frequency,cumulative"
    # First group: cumulative value should be taken from the next entry.
    assert csv_contents[1] == "4,0,3,3"
    assert csv_contents[2] == "4,1,5,8"
    assert csv_contents[3] == "4,2,7,15"
    # Second group exercises fallback to the group's final cumulative entry.
    assert csv_contents[4] == "2,0,11,11"


def test_clip_manifest_verification(tmp_path, rans_compare_module):
    clip_root = tmp_path / "clips"
    clip_root.mkdir()
    clip_path = clip_root / "demo.y4m"
    clip_contents = b"demo-clip-data"
    clip_path.write_bytes(clip_contents)
    manifest = tmp_path / "manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "clip_sets": {
                    "objective-1-fast": [
                        {
                            "id": "demo",
                            "path": clip_path.name,
                            "frames": 4,
                            "fps": "30:1",
                            "sha256": "fe8d49773359320ba625477e94c292e3730639bf0370a6c76bf5cb6d50cae9a6",
                        }
                    ]
                }
            }
        ),
        encoding="utf-8",
    )

    entries = rans_compare_module.load_clip_manifest(manifest, clip_root)
    assert len(entries) == 1
    assert entries[0].clip_id == "demo"
    checks = rans_compare_module.verify_clips(entries)
    assert len(checks) == 1
    check = checks[0]
    assert check.exists
    assert check.bytes == len(clip_contents)
    assert check.sha256_actual == "fe8d49773359320ba625477e94c292e3730639bf0370a6c76bf5cb6d50cae9a6"
    assert check.ok


def test_run_runner_creates_logs(tmp_path, rans_compare_module):
    env = os.environ.copy()
    runner = rans_compare_module.RunnerSpec(
        label="python-echo",
        command=f"{shlex.quote(sys.executable)} -c \"print('runner-ok')\"",
    )
    result = rans_compare_module.run_runner(
        runner,
        env=env,
        workdir=REPO_ROOT,
        log_dir=tmp_path / "logs",
        dry_run=False,
    )
    assert result.return_code == 0
    assert result.stdout_path.read_text(encoding="utf-8").strip() == "runner-ok"
    assert result.stderr_path.read_text(encoding="utf-8") == ""

    dry_run_result = rans_compare_module.run_runner(
        runner,
        env=env,
        workdir=REPO_ROOT,
        log_dir=tmp_path / "logs",
        dry_run=True,
    )
    assert dry_run_result.skipped
    assert dry_run_result.return_code == 0
    assert "dry-run" in dry_run_result.stdout_path.read_text(encoding="utf-8")


def test_load_runner_presets(tmp_path, rans_compare_module):
    preset = tmp_path / "preset.toml"
    preset.write_text(
        "[[runner]]\nlabel = \"demo\"\ncommand = [\"echo\", \"hello\"]\n",
        encoding="utf-8",
    )
    specs = rans_compare_module.load_runner_presets([preset])
    assert len(specs) == 1
    assert specs[0].label == "demo"
    assert specs[0].command == ["echo", "hello"]


def test_verify_clips_flags_hash_mismatches(tmp_path, rans_compare_module):
    manifest = tmp_path / "manifest.json"
    clips_dir = tmp_path / "clips"
    clips_dir.mkdir()
    clip_path = clips_dir / "sample.y4m"
    clip_path.write_bytes(b"sample-clip")

    manifest.write_text(
        json.dumps(
            {
                "clip_sets": {
                    "demo": [
                        {
                    "id": "sample",
                    "path": "sample.y4m",
                    "sha256": "0c7ca4ea78554acab52e924c4c58ccba618e3e8a8d1182b91ca345bab2ce98d4",
                }
                    ]
                }
            }
        ),
        encoding="utf-8",
    )

    entries = rans_compare_module.load_clip_manifest(manifest, clips_dir)
    initial_checks = rans_compare_module.verify_clips(entries)
    assert len(initial_checks) == 1
    assert initial_checks[0].ok

    clip_path.write_bytes(b"tampered")
    tampered_checks = rans_compare_module.verify_clips(entries)
    assert not tampered_checks[0].ok
