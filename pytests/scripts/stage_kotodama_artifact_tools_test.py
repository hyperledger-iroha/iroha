"""Native file controls for Kotodama's private executable staging boundary."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import stat
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
try:
    import stage_kotodama_artifact_tools as staging
finally:
    sys.path.pop(0)


@pytest.fixture
def build(tmp_path: Path) -> Path:
    """Create genuine executable files with Cargo-like second hard links."""
    directory = tmp_path.resolve() / "build"
    directory.mkdir()
    for name in staging.TOOLS:
        tool = directory / name
        tool.write_bytes(b"#!/bin/sh\nprintf 'native-staging-control\\n'\n")
        tool.chmod(0o755)
        os.link(tool, directory / f"{name}-deps")
    return directory


def test_cargo_links_become_private_executable_copies(build: Path) -> None:
    output = build.parent / "staged"
    originals = {
        name: staging.revision((build / name).stat()) for name in staging.TOOLS
    }
    staging.stage_tools(build, output)
    assert stat.S_IMODE(output.stat().st_mode) == 0o700
    for name in staging.TOOLS:
        source, copied = build / name, output / name
        assert staging.revision(source.stat()) == originals[name]
        assert source.stat().st_nlink == 2
        assert copied.stat().st_nlink == 1
        assert copied.stat().st_ino != source.stat().st_ino
        assert copied.stat().st_uid == os.getuid()
        assert stat.S_IMODE(copied.stat().st_mode) == 0o700
        assert copied.read_bytes() == source.read_bytes()
        assert (
            subprocess.check_output([copied], text=True) == "native-staging-control\n"
        )

    # Exercise the unchanged golden generator's actual sealed-input reader.
    spec = importlib.util.spec_from_file_location(
        "kotodama_staging_goldens", ROOT / "scripts/regenerate_kotodama_goldens.py"
    )
    assert spec and spec.loader
    goldens = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = goldens
    spec.loader.exec_module(goldens)
    for name in staging.TOOLS:
        with pytest.raises(goldens.GoldenError, match="one hard link"):
            goldens.seal_file(build / name, "generation input")
        assert goldens.seal_file(output / name, "generation input").links == 1


@pytest.mark.parametrize(
    "kind", ["symlink", "directory", "non-executable", "writable", "setuid"]
)
def test_refuses_noncanonical_build_inputs(build: Path, kind: str) -> None:
    source = build / "koto"
    if kind in ("symlink", "directory"):
        source.unlink()
        if kind == "symlink":
            source.symlink_to(build / "koto-deps")
        else:
            source.mkdir()
    else:
        source.chmod(
            {"non-executable": 0o644, "writable": 0o777, "setuid": 0o4755}[kind]
        )
    with pytest.raises(staging.ArtifactContractError, match="owned.*executable"):
        staging.stage_tools(build, build.parent / "staged")


def test_refuses_reused_or_symlinked_destination(build: Path) -> None:
    output = build.parent / "staged"
    output.mkdir(mode=0o700)
    marker = output / "preserve"
    marker.write_text("original")
    with pytest.raises(FileExistsError):
        staging.stage_tools(build, output)
    link = build.parent / "linked-output"
    link.symlink_to(output)
    with pytest.raises(FileExistsError):
        staging.stage_tools(build, link)
    assert marker.read_text() == "original"


def test_refuses_staging_digest_mismatch(
    build: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(staging, "stable_artifact_identity", lambda _: ("0" * 64, 1))
    with pytest.raises(staging.ArtifactContractError, match="differs from its pinned"):
        staging.stage_tools(build, build.parent / "staged")


@pytest.mark.parametrize("changed", ["source", "destination-link"])
def test_rechecks_original_identity_after_copy(
    build: Path, monkeypatch: pytest.MonkeyPatch, changed: str
) -> None:
    copy = staging.stage_unique_artifact

    def tampered_copy(source: Path, destination: Path) -> tuple[Path, str, int]:
        result = copy(source, destination)
        if changed == "source":
            source.write_bytes(b"different build output")
        else:
            os.link(destination, destination.with_suffix(".alias"))
        return result

    monkeypatch.setattr(staging, "stage_unique_artifact", tampered_copy)
    with pytest.raises(staging.ArtifactContractError, match="changed|single-link"):
        staging.stage_tools(build, build.parent / "staged")


def test_workflow_builds_then_stages_and_checks_both_exact_copies() -> None:
    workflow = (ROOT / ".github/workflows/kotodama_perf.yml").read_text()
    job = workflow.split("  kotodama-artifact-gate:\n", 1)[1]
    by_name = {}
    for index, step in enumerate(job.split("\n      - name: ")[1:]):
        name, _, command = step.partition("\n")
        by_name[name] = (index, command)
    build, _ = by_name["Build canonical Kotodama release tools"]
    stage, staging_command = by_name["Stage private single-link Kotodama tools"]
    docs, docs_command = by_name["Compile every Kotodama documentation source"]
    guard, guard_command = by_name["Enforce complete artifact release gate"]
    assert build < stage < docs < guard
    assert '--build-directory "$CARGO_TARGET_DIR/debug"' in staging_command
    assert '--koto "$RUNNER_TEMP/kotodama-artifact-tools/koto"' in docs_command
    assert '--koto "$RUNNER_TEMP/kotodama-artifact-tools/koto"' in guard_command
    assert (
        '--admission-tool "$RUNNER_TEMP/kotodama-artifact-tools/ivm_artifact_admit"'
        in guard_command
    )
    assert "--check" in guard_command
