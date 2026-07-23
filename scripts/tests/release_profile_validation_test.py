"""Regression tests for release script profile validation."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
RELEASE_SCRIPTS = (
    REPO_ROOT / "scripts" / "build_release_bundle.sh",
    REPO_ROOT / "scripts" / "build_release_image.sh",
)


def _heredoc_program(source: str, delimiter: str) -> str:
    marker = f"<<'{delimiter}'\n"
    assert marker in source
    return source.split(marker, 1)[1].split(f"\n{delimiter}", 1)[0]


def _fake_tool(directory: Path, name: str) -> None:
    tool = directory / name
    tool.write_text(
        "#!/bin/sh\n"
        'printf \'%s\\n\' "${0##*/}" >>"$RELEASE_TOOL_CALLS"\n'
        "exit 97\n",
        encoding="utf-8",
    )
    tool.chmod(0o700)


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
@pytest.mark.parametrize("profile", ("iroha4", "../escaped-release"))
def test_release_script_rejects_invalid_profile_before_tools_or_outputs(
    tmp_path: Path,
    script: Path,
    profile: str,
) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    for name in ("cargo", "docker", "zstd"):
        _fake_tool(fake_bin, name)

    tool_calls = tmp_path / "tool-calls.txt"
    artifacts_dir = tmp_path / "artifacts"
    environment = dict(os.environ)
    environment.update(
        {
            "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
            "RELEASE_TOOL_CALLS": str(tool_calls),
        }
    )

    result = subprocess.run(
        [
            "bash",
            str(script),
            "--profile",
            profile,
            "--config",
            "single",
            "--artifacts-dir",
            str(artifacts_dir),
        ],
        cwd=REPO_ROOT,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 1
    assert (
        f"Unsupported profile value: {profile} (expected iroha2 or iroha3)"
        in result.stderr
    )
    assert not tool_calls.exists()
    assert not artifacts_dir.exists()


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_manifest_values_are_passed_as_data(tmp_path: Path, script: Path) -> None:
    source = script.read_text(encoding="utf-8")
    program = _heredoc_program(source, "MANIFEST_PY")
    assert "${" not in program

    sentinel = tmp_path / "unexpected-side-effect"
    unusual = f'feature-");open("{sentinel}","w").write("bad");#\nnext'
    manifest_path = tmp_path / 'manifest-"quoted".json'
    common = [
        str(manifest_path),
        "iroha2",
        "single",
        "1.2.3",
        "abcdef0",
        "2026-07-22T00:00:00Z",
        "mac",
        "arm64",
    ]
    if script.name == "build_release_bundle.sh":
        arguments = [
            *common,
            unusual,
            "dist/archive.tar.zst",
            "aa" * 32,
            "",
            "",
        ]
    else:
        arguments = [
            *common,
            unusual,
            "",
            "",
            "registry.example/iroha:quoted",
            "sha256:image-id",
            "dist/image.tar",
            "bb" * 32,
            "",
            "",
        ]

    result = subprocess.run(
        [sys.executable, "-", *arguments],
        input=program,
        text=True,
        capture_output=True,
        check=False,
        cwd=tmp_path,
    )

    assert result.returncode == 0, result.stderr
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["features"] == unusual
    assert manifest["profile"] == "iroha2"
    assert not sentinel.exists()


def test_bundle_profile_values_are_toml_escaped(tmp_path: Path) -> None:
    script = RELEASE_SCRIPTS[0]
    program = _heredoc_program(script.read_text(encoding="utf-8"), "PROFILE_PY")
    assert "${" not in program

    sentinel = tmp_path / "unexpected-profile-side-effect"
    unusual = f'path-"\\\nvalue; open("{sentinel}", "w")'
    profile_path = tmp_path / 'PROFILE-"quoted".toml'
    arguments = [
        str(profile_path),
        "iroha2",
        unusual,
        "1.2.3",
        "abcdef0",
        "2026-07-22T00:00:00Z",
        "mac",
        "arm64",
        unusual,
    ]

    result = subprocess.run(
        [sys.executable, "-", *arguments],
        input=program,
        text=True,
        capture_output=True,
        check=False,
        cwd=tmp_path,
    )

    assert result.returncode == 0, result.stderr
    with profile_path.open("rb") as profile_file:
        profile = tomllib.load(profile_file)
    assert profile["config"] == unusual
    assert profile["features"] == unusual
    assert not sentinel.exists()
