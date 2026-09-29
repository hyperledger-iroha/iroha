"""Exercise FASTPQ container command construction without running a container."""

from __future__ import annotations

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

import pytest

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 uses the repository's pinned backport.
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[2]
# These are syntactic test inputs, never fetched or presented as reviewed images.
RUST_IMAGE = "registry.example/rust@sha256:" + "a" * 64
CUDA_IMAGE = "registry.example/cuda@sha256:" + "b" * 64


def fixture(tmp_path: Path, channel: str):
    """Use the real helper with an isolated source pin and recording-only runtime."""
    root = tmp_path / "source"
    shutil.copytree(ROOT / "scripts/fastpq", root / "scripts/fastpq")
    (root / "rust-toolchain.toml").write_text(
        '[toolchain]\nchannel = ' + json.dumps(channel) + '\n', encoding="utf-8"
    )
    binary = tmp_path / "bin"
    binary.mkdir()
    (binary / "python3").symlink_to(sys.executable)
    runtime = binary / "container-recorder"
    log = tmp_path / "calls.jsonl"
    runtime.write_text(
        f"#!{sys.executable}\n"
        "import json, os, sys\n"
        "with open(os.environ['FASTPQ_TEST_CALLS'], 'a') as output:\n"
        "    output.write(json.dumps(sys.argv[1:]) + '\\n')\n",
        encoding="utf-8",
    )
    runtime.chmod(0o700)
    env = {k: v for k, v in os.environ.items() if not k.startswith("FASTPQ_")}
    env.update(PATH=str(binary) + os.pathsep + env["PATH"], FASTPQ_TEST_CALLS=str(log))
    command = [
        "bash", str(root / "scripts/fastpq/repro_build.sh"),
        "--container-runtime", str(runtime), "--rust-image", RUST_IMAGE,
    ]
    return root, env, command, log


@pytest.mark.parametrize("mode", ["cpu", "gpu"])
@pytest.mark.parametrize("channel", ["1.93.1", "1.94.0"])
def test_canonical_source_channel_reaches_build_and_runtime(tmp_path, mode, channel):
    _, env, command, log = fixture(tmp_path, channel)
    result = subprocess.run(
        [*command, "--mode", mode, "--cuda-image", CUDA_IMAGE],
        env=env, text=True, capture_output=True, check=False,
    )
    assert result.returncode == 0, result.stderr
    build, run = [json.loads(row) for row in log.read_text().splitlines()]
    assert build[0] == "build" and run[0] == "run"
    assert "RUST_TOOLCHAIN=" + channel in build
    assert "FASTPQ_RUST_TOOLCHAIN=" + channel in run
    assert "RUST_IMAGE=" + RUST_IMAGE in build
    if mode == "gpu":
        assert "CUDA_IMAGE=" + CUDA_IMAGE in build
        assert "FASTPQ_CUDA_IMAGE=" + CUDA_IMAGE in run


@pytest.mark.parametrize("channel", ["stable", "1.93"])
def test_nonexact_source_channel_rejects_before_container(tmp_path, channel):
    _, env, command, log = fixture(tmp_path, channel)
    result = subprocess.run(command, env=env, text=True, capture_output=True, check=False)
    assert result.returncode != 0
    assert "must pin an exact stable Rust version" in result.stderr
    assert not log.exists()


def test_conflicting_override_rejects_before_container(tmp_path):
    _, env, command, log = fixture(tmp_path, "1.93.1")
    env["FASTPQ_RUST_TOOLCHAIN"] = "1.88.0"
    result = subprocess.run(command, env=env, text=True, capture_output=True, check=False)
    assert result.returncode != 0
    assert "must match rust-toolchain.toml (1.93.1)" in result.stderr
    assert not log.exists()


def test_default_is_current_repository_pin(tmp_path):
    channel = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    _, env, command, log = fixture(tmp_path, channel)
    env["FASTPQ_RUST_TOOLCHAIN"] = channel
    result = subprocess.run(command, env=env, text=True, capture_output=True, check=False)
    assert result.returncode == 0, result.stderr
    build, run = [json.loads(row) for row in log.read_text().splitlines()]
    assert "RUST_TOOLCHAIN=" + channel in build
    assert "FASTPQ_RUST_TOOLCHAIN=" + channel in run


@pytest.mark.parametrize("mode", ["cpu", "gpu"])
def test_docker_toolchain_argument_is_in_consuming_stage(mode):
    """Global ARG before FROM cannot satisfy a later-stage ENV reference."""
    source = (ROOT / f"scripts/fastpq/docker/Dockerfile.{mode}").read_text()
    final_stage = re.split(r"(?m)^FROM .*$", source)[-1]
    declaration = re.search(r"(?m)^ARG RUST_TOOLCHAIN$", final_stage)
    assert declaration is not None
    assert declaration.start() < final_stage.index("FASTPQ_RUST_TOOLCHAIN=${RUST_TOOLCHAIN}")
    assert "ARG RUST_TOOLCHAIN=" not in source
    assert 'RUN test -n "${FASTPQ_RUST_TOOLCHAIN}"' in final_stage
