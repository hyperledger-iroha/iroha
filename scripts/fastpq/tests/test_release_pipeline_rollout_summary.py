"""Tests for FASTPQ rollout-summary integration in the release pipeline."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
from pathlib import Path

import pytest


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


def test_evidence_inventory_label_keeps_fastpq_paths_after_staging_cleanup() -> None:
    module = _load_module()
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        evidence_stage = root / ".evidence-staging"
        fastpq_bundle = (
            evidence_stage / "fastpq_rollouts" / "lab" / "cuda"
        )
        summary = fastpq_bundle / "fastpq_rollout_summary.md"

        assert module.evidence_inventory_label(fastpq_bundle, evidence_stage) == (
            "evidence/fastpq_rollouts/lab/cuda"
        )
        assert module.evidence_inventory_label(summary, evidence_stage) == (
            "evidence/fastpq_rollouts/lab/cuda/fastpq_rollout_summary.md"
        )

        repo_relative_stage = Path("artifacts/releases/test/.evidence-staging")
        repo_relative_summary = (
            repo_relative_stage
            / "fastpq_rollouts"
            / "lab"
            / "fastpq_rollout_summary.md"
        )
        assert module.evidence_inventory_label(
            repo_relative_summary, repo_relative_stage
        ) == "evidence/fastpq_rollouts/lab/fastpq_rollout_summary.md"

        with pytest.raises(module.PipelineError, match="outside the staging tree"):
            module.evidence_inventory_label(root / "outside.json", evidence_stage)


def test_evidence_inventory_label_rejects_traversal_and_symlink_escape() -> None:
    module = _load_module()
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        evidence_stage = root / ".evidence-staging"

        with pytest.raises(module.PipelineError, match="lexical traversal"):
            module.evidence_inventory_label(
                evidence_stage / "fastpq_rollouts" / ".." / "escape.json",
                evidence_stage,
            )

        evidence_stage.mkdir()
        outside = root / "outside"
        outside.mkdir()
        link = evidence_stage / "linked-outside"
        try:
            link.symlink_to(outside, target_is_directory=True)
        except (NotImplementedError, OSError) as exc:
            pytest.skip(f"symlinks unavailable: {exc}")

        with pytest.raises(module.PipelineError, match="outside the staging tree"):
            module.evidence_inventory_label(link / "escape.json", evidence_stage)


@pytest.mark.parametrize(
    "stamp",
    [
        "",
        ".",
        "..",
        "../escape",
        "nested/stamp",
        r"nested\stamp",
        "/absolute",
        " leading",
        "trailing ",
        "x" * 129,
    ],
)
def test_validate_fastpq_rollout_stamp_rejects_unsafe_components(stamp: str) -> None:
    module = _load_module()

    with pytest.raises(module.PipelineError, match="safe path component"):
        module.validate_fastpq_rollout_stamp(stamp)


def test_validate_fastpq_rollout_stamp_accepts_single_safe_component() -> None:
    module = _load_module()
    stamp = "20260826T0102Z_v3.0.0-rc.1+reviewed"

    assert module.validate_fastpq_rollout_stamp(stamp) == stamp
    assert module.validate_fastpq_rollout_stamp(None) is None


def test_main_rejects_unsafe_fastpq_stamp_before_pipeline_work(monkeypatch) -> None:
    module = _load_module()

    def unexpected_pipeline_work(*args, **kwargs):  # type: ignore[no-untyped-def]
        pytest.fail("pipeline work started before rollout-stamp validation")

    monkeypatch.setattr(module, "release_signing_cli_args", unexpected_pipeline_work)
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "run_release_pipeline.py",
            "--version",
            "3.0.0",
            "--fastpq-rollout-stamp",
            "../escape",
        ],
    )

    with pytest.raises(module.PipelineError, match="safe path component"):
        module.main()
