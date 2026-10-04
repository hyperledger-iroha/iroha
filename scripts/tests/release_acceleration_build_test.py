"""Target-based canonical build policy controls without invoking Cargo or hardware."""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import shutil
from scripts.tests.release_builder_fixture import write_cuda_approval_source, CUDA_PUBLIC_KEY, CUDA_BUNDLE, CUDA_KEY_SHA256

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/build_canonical_binaries.sh"


def invoke(tmp_path: Path, target: str, profile: str, fingerprint: str | None, *, ambient: str | None = None):
    source = tmp_path / "source"
    scripts = source / "scripts"
    scripts.mkdir(parents=True)
    shutil.copy2(SCRIPT, scripts / SCRIPT.name)
    shutil.copy2(ROOT / "scripts/release_artifact_contract.py", scripts / "release_artifact_contract.py")
    write_cuda_approval_source(source, key=fingerprint or "0" * 64, present=fingerprint is not None)
    cuda = source / "crates/ivm/cuda"
    cuda.mkdir(parents=True)
    (cuda / "provenance.v1.pub").write_bytes(CUDA_PUBLIC_KEY)
    (cuda / "provenance.v1").write_bytes(CUDA_BUNDLE)
    tools = tmp_path / "tools"
    tools.mkdir()
    log = tmp_path / "cargo.json"
    (tools / "cargo").write_text(
        "#!/usr/bin/env python3\nimport json, os, sys\n"
        "with open(os.environ['TEST_CARGO_LOG'], 'w') as output:\n"
        " json.dump({'args': sys.argv[1:], 'mode': os.getenv('IVM_CUDA_PTX_MODE'), "
        "'key': os.getenv('IVM_CUDA_TRUSTED_KEY_SHA256')}, output)\n"
    )
    (tools / "rustc").write_text("#!/bin/sh\nprintf 'host: aarch64-apple-darwin\\n'\n")
    (tools / "uname").write_text("#!/bin/sh\necho 'build must not select from host uname' >&2\nexit 111\n")
    for path in tools.iterdir():
        path.chmod(0o755)
    environment = os.environ.copy()
    environment.update(PATH=f"{tools}{os.pathsep}{environment['PATH']}", BUILD_PROFILE=profile, TEST_CARGO_LOG=str(log))
    environment.pop("IVM_CUDA_TRUSTED_KEY_SHA256", None)
    environment.pop("IVM_CUDA_PTX_MODE", None)
    if fingerprint is not None:
        environment["IVM_CUDA_TRUSTED_KEY_SHA256"] = fingerprint
    if ambient is not None:
        environment["IVM_CUDA_TRUSTED_KEY_SHA256"] = ambient
    completed = subprocess.run(["bash", str(scripts / SCRIPT.name), "--target", target], cwd=ROOT, env=environment, text=True, capture_output=True)
    return completed, json.loads(log.read_text()) if log.exists() else None


@pytest.mark.parametrize("target", ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-musl", "aarch64-apple-darwin"))
def test_development_build_needs_no_cuda_trust_toolkit_or_driver(tmp_path: Path, target: str):
    result, cargo = invoke(tmp_path, target, "dev", None)
    assert result.returncode == 0, result.stderr
    assert cargo is not None
    assert cargo["args"][cargo["args"].index("--target") + 1] == target
    assert "ivm-cuda" not in cargo["args"][cargo["args"].index("--features") + 1]
    assert cargo["key"] is None and cargo["mode"] is None


@pytest.mark.parametrize("target", ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-musl"))
def test_shipping_linux_uses_target_backend_and_reviewed_public_fingerprint(tmp_path: Path, target: str):
    result, cargo = invoke(tmp_path, target, "deploy", CUDA_KEY_SHA256)
    assert result.returncode == 0, result.stderr
    assert cargo is not None
    assert "ivm-cuda" not in cargo["args"][cargo["args"].index("--features") + 1]
    assert cargo["key"] == CUDA_KEY_SHA256 and cargo["mode"] is None


@pytest.mark.parametrize("fingerprint", (None, "", "0" * 64, "A" * 64, "a" * 63))
def test_shipping_linux_refuses_missing_or_noncanonical_trust_before_cargo(tmp_path: Path, fingerprint: str | None):
    result, cargo = invoke(tmp_path, "x86_64-unknown-linux-gnu", "deploy", fingerprint)
    assert result.returncode != 0
    assert cargo is None
    assert "CUDA" in result.stderr


def test_shipping_mac_retains_default_metal_without_cuda_build_inputs(tmp_path: Path):
    result, cargo = invoke(tmp_path, "aarch64-apple-darwin", "deploy", None)
    assert result.returncode == 0, result.stderr
    assert cargo is not None
    assert "ivm-cuda" not in cargo["args"][cargo["args"].index("--features") + 1]
    assert "--no-default-features" not in cargo["args"]
    assert cargo["key"] is None and cargo["mode"] is None


@pytest.mark.parametrize("target", ("x86_64-pc-windows-msvc", "fake-linux-target", "riscv64gc-unknown-linux-gnu"))
def test_canonical_builder_rejects_targets_outside_its_unix_delivery_matrix(tmp_path: Path, target: str):
    result, cargo = invoke(tmp_path, target, "deploy", "a" * 64)
    assert result.returncode != 0
    assert cargo is None


@pytest.mark.parametrize("dockerfile", ("Dockerfile", "Dockerfile.cross", "Dockerfile.musl"))
@pytest.mark.parametrize("missing", (None, *range(13), "link"))
def test_docker_shipping_admission_requires_backend_and_independent_key(tmp_path: Path, dockerfile: str, missing):
    # Inventory only; none of these synthetic bytes has cryptographic authority.
    source = (ROOT / dockerfile).read_text()
    start = source.index("for input in ")
    end = source.index("done;", start) + len("done;")
    admission = source[start:end].replace("\\\n", "\n")
    names = admission.split("for input in ", 1)[1].split("; do", 1)[0].split()
    assert len(names) == 13 and len(set(names)) == 13
    cuda = tmp_path / "crates/ivm/cuda"
    cuda.mkdir(parents=True)
    for index, name in enumerate(names):
        if missing != index:
            (cuda / name).write_bytes(b"synthetic inventory only")
    if missing == "link":
        (cuda / names[0]).unlink()
        (cuda / names[0]).symlink_to(cuda / names[1])
    result = subprocess.run(["/bin/sh", "-ec", admission], cwd=tmp_path, capture_output=True)
    assert (result.returncode == 0) is (missing is None)
    assert all(token not in source for token in ("ivm-cuda", "IVM_CUDA_TRUSTED_KEY_SHA256", "IVM_CUDA_PTX_MODE"))


@pytest.mark.parametrize("ambient", ("", "0" * 64, "A" * 64, "a" * 64))
def test_canonical_shipping_uses_source_approval_even_with_wrong_ambient_key(tmp_path: Path, ambient: str):
    result, cargo = invoke(tmp_path, "x86_64-unknown-linux-gnu", "deploy", CUDA_KEY_SHA256, ambient=ambient)
    assert result.returncode == 0, result.stderr
    assert cargo is not None and cargo["key"] == ambient
    assert cargo["mode"] is None
    assert "ivm-cuda" not in cargo["args"][cargo["args"].index("--features") + 1]
