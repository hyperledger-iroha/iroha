"""Tests for FASTPQ rollout-summary integration in the release pipeline."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
from pathlib import Path


def _load_module():
    scripts_dir = Path(__file__).resolve().parents[2]
    if str(scripts_dir) not in sys.path:
        sys.path.insert(0, str(scripts_dir))
    module_path = scripts_dir / "run_release_pipeline.py"
    spec = importlib.util.spec_from_file_location("run_release_pipeline", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_summarize_fastpq_rollout_bundle_invokes_helper() -> None:
    module = _load_module()
    with tempfile.TemporaryDirectory() as temp_dir:
        bundle_dir = Path(temp_dir) / "bundle"
        bundle_dir.mkdir()
        manifest = bundle_dir / "fastpq_bench_manifest.json"
        manifest.write_text('{"payload":{"benches":[]}}', encoding="utf-8")

        calls: list[list[str]] = []

        def fake_run(cmd: list[str], *, cwd=None, env=None):  # type: ignore[no-untyped-def]
            calls.append(cmd)

        original_run = module.run
        module.run = fake_run
        try:
            summaries = module.summarize_fastpq_rollout_bundle(bundle_dir, dry_run=False)
        finally:
            module.run = original_run

        assert summaries == [
            {
                "manifest": str(manifest),
                "json": str(bundle_dir / "fastpq_rollout_summary.json"),
                "markdown": str(bundle_dir / "fastpq_rollout_summary.md"),
            }
        ]
        assert len(calls) == 1
        command = calls[0]
        assert command[0] == sys.executable
        assert command[1].endswith("scripts/fastpq/rollout_manifest_summary.py")
        assert command[2:] == [
            "--manifest",
            str(manifest),
            "--bundle-dir",
            str(bundle_dir),
            "--repo-root",
            str(module.REPO_ROOT),
            "--json-out",
            str(bundle_dir / "fastpq_rollout_summary.json"),
            "--markdown-out",
            str(bundle_dir / "fastpq_rollout_summary.md"),
        ]


def test_update_release_manifest_evidence_records_fastpq_and_cbdc() -> None:
    module = _load_module()
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        manifest_path = root / "release_manifest.json"
        manifest_path.write_text(
            json.dumps(
                {
                    "version": "2.0.0-rc.3",
                    "commit": "abcdef0",
                    "built_at": "2026-03-27T12:00:00Z",
                    "os": "linux",
                    "arch": "x86_64",
                    "artifacts": [],
                }
            ),
            encoding="utf-8",
        )

        module.update_release_manifest_evidence(
            manifest_path,
            fastpq_grafana_rel="artifacts/fastpq_rollouts/20260327T1200Z/release_pipeline",
            archived_fastpq=[
                {
                    "bundle": "artifacts/releases/2.0.0-rc.3/fastpq_rollouts/lab/cuda",
                    "summaries": [
                        {
                            "manifest": "artifacts/releases/2.0.0-rc.3/fastpq_rollouts/lab/cuda/fastpq_bench_manifest.json",
                            "json": "artifacts/releases/2.0.0-rc.3/fastpq_rollouts/lab/cuda/fastpq_rollout_summary.json",
                            "markdown": "artifacts/releases/2.0.0-rc.3/fastpq_rollouts/lab/cuda/fastpq_rollout_summary.md",
                        }
                    ],
                }
            ],
            cbdc_validation_rel="artifacts/nexus/cbdc_rollouts/20260327T1200Z",
        )

        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        assert manifest["evidence"]["fastpq"]["grafana_export"] == (
            "artifacts/fastpq_rollouts/20260327T1200Z/release_pipeline"
        )
        assert manifest["evidence"]["fastpq"]["rollout_bundles"][0]["bundle"].endswith("/cuda")
        assert manifest["evidence"]["fastpq"]["rollout_bundles"][0]["summaries"][0]["markdown"].endswith(
            "fastpq_rollout_summary.md"
        )
        assert manifest["evidence"]["cbdc"]["validated_bundle"] == (
            "artifacts/nexus/cbdc_rollouts/20260327T1200Z"
        )
