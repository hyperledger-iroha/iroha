"""Private synthetic source snapshots for packaging controls, never release evidence.

The fake Git owner reports only files in this disposable fixture. Production source
validation and stable readers run unchanged; only the fixture's embedded seal is
computed here. Public CUDA bytes model build-input identity, not a signed bundle or
hardware qualification. Nothing is written into the caller's checkout.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

SOURCE_COMMIT = "a" * 40
CUDA_PUBLIC_KEY = bytes(range(32))
CUDA_KEY_SHA256 = hashlib.sha256(CUDA_PUBLIC_KEY).hexdigest()
CUDA_BUNDLE = b"synthetic packaging fixture; not a qualified CUDA bundle\n"
CUDA_BUNDLE_SHA256 = hashlib.sha256(CUDA_BUNDLE).hexdigest()
_FILES = (
    "Cargo.toml", "Cargo.lock", "LICENSE", "Dockerfile",
    "scripts/build_release_bundle.sh", "scripts/build_release_image.sh",
    "scripts/check_release_feature_graph.py", "scripts/run_isolated_release_tool.py",
    "scripts/release_artifact_contract.py", "scripts/verify_release_prebuilt_provenance.py",
    "scripts/build_release_oci_archive.py", "scripts/build_release_tar_zst.py",
    "scripts/capture_release_command.py", "scripts/copy_release_file.py",
    "scripts/copy_release_tree.py", "scripts/write_release_checksum.py",
    "scripts/docker_entrypoint.sh", "scripts/ci/package_inrou_runtime_v1.py",
    "scripts/run_release_pipeline.py", "scripts/release_manifest_signing.py", "scripts/publish_plan.py",
)
_TREES = (
    "defaults", "codec/rans/tables", "configs/sorafs/external_software_signer",
    "configs/sorafs/runtime_provider_broker",
)


def write_cuda_approval_source(
    source: Path, key: str = CUDA_KEY_SHA256, manifest: str = CUDA_BUNDLE_SHA256,
    *, present: bool = True,
) -> Path:
    """Write only a disposable literal owner for packaging metadata controls."""
    owner = source / "crates/ivm/src/cuda_build_policy.rs"
    owner.parent.mkdir(parents=True, exist_ok=True)
    value = ('Some(ReviewedCudaBundlePins { public_key_sha256: "' + key
             + '", manifest_sha256: "' + manifest + '" })') if present else "None"
    owner.write_text('pub(crate) const REVIEWED_CUDA_BUNDLE_PINS: Option<ReviewedCudaBundlePins> = '
                     + value + ';\n')
    return owner


def prepare_source_fixture(repo: Path, destination: Path, environment: dict[str, str], *, cuda_approval: bool = True) -> Path:
    """Copy packaging inputs and seal a disposable synthetic source owner."""
    destination.mkdir()
    source = destination / "source"
    source.mkdir()
    for relative in _FILES:
        path = source / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(repo / relative, path)
    # No compiled source is claimed: these two metadata fixtures are sufficient
    # for package version and exact Cargo.lock identity checks in the wrappers.
    (source / "Cargo.toml").write_text('[workspace.package]\nversion = "2.0.0-rc.2.0"\n')
    (source / "Cargo.lock").write_text('# synthetic packaging fixture lock\n')
    for relative in _TREES:
        shutil.copytree(repo / relative, source / relative)
    cuda = source / "crates/ivm/cuda"
    cuda.mkdir(parents=True)
    (cuda / "provenance.v1").write_bytes(CUDA_BUNDLE)
    (cuda / "provenance.v1.pub").write_bytes(CUDA_PUBLIC_KEY)
    write_cuda_approval_source(source, present=cuda_approval)
    inventory = sorted(path.relative_to(source).as_posix() for path in source.rglob("*") if path.is_file())
    tools = destination / "tools"
    tools.mkdir()
    git = tools / "git"
    git.write_text(
        '#!/usr/bin/env python3\nimport os, pathlib, sys\n'
        f'root = {str(source)!r}\ncommit = {SOURCE_COMMIT!r}\ninventory = {inventory!r}\n'
        'arguments = sys.argv[1:]\n'
        'if os.getcwd() != root: raise SystemExit("synthetic Git owner used outside fixture")\n'
        'if arguments == ["rev-parse", "HEAD"]:\n print(commit)\n raise SystemExit(0)\n'
        'if arguments and arguments[0] == "ls-files":\n'
        ' names = inventory if "--others" not in arguments else sorted(path.relative_to(root).as_posix() for path in pathlib.Path(root).rglob("*") if path.is_file() and "__pycache__" not in path.parts)\n'
        ' sys.stdout.buffer.write(b"\\0".join(name.encode() for name in names) + b"\\0")\n raise SystemExit(0)\n'
        'if arguments and arguments[0] == "diff":\n'
        ' counter = os.getenv("IROHA_TEST_GIT_DIFF_COUNTER")\n count = 1\n'
        ' if counter:\n'
        '  path = pathlib.Path(counter)\n'
        '  count = int(path.read_text()) + 1 if path.exists() else 1\n'
        '  path.write_text(str(count))\n'
        ' late = int(os.getenv("IROHA_TEST_GIT_DIRTY_AFTER", "0"))\n'
        ' raise SystemExit(1 if os.getenv("IROHA_TEST_GIT_ALWAYS_DIRTY") == "1" or (late and count > late) else 0)\n'
        'raise SystemExit("unexpected synthetic Git operation")\n'
    )
    git.chmod(0o755)
    environment["PATH"] = f"{tools}{os.pathsep}{environment['PATH']}"
    checker = source / "scripts/check_release_feature_graph.py"
    pipeline = source / "scripts/run_release_pipeline.py"
    body = pipeline.read_text()
    for name in ("release_artifact_contract", "release_manifest_signing", "publish_plan", "check_release_feature_graph"):
        helper = (source / "scripts" / (name + ".py")).read_bytes()
        if name == "check_release_feature_graph":
            helper, count = re.subn(rb'(TRUSTED_RELEASE_SURFACE_SHA256\s*=\s*\(\s*")[0-9a-f]{64}("\s*\))',
                                   lambda match: match[1] + b"0" * 64 + match[2], helper)
            assert count == 1
        body, count = re.subn(r'("' + name + r'": ")[0-9a-f]{64}("[,])',
                             lambda match: match[1] + hashlib.sha256(helper).hexdigest() + match[2], body)
        assert count == 1
    pipeline.write_text(body)
    calculate = (
        "import importlib.util, pathlib, sys; "
        "spec=importlib.util.spec_from_file_location('fixture_guard', sys.argv[1]); "
        "module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); "
        "print(module.trusted_release_surface_digest(pathlib.Path(sys.argv[2])))"
    )
    digest = subprocess.check_output(
        [sys.executable, "-I", "-S", "-c", calculate, str(checker), str(source)],
        env=environment, cwd=source, text=True,
    ).strip()
    payload, count = re.subn(
        r'(TRUSTED_RELEASE_SURFACE_SHA256\s*=\s*\(\s*")[0-9a-f]{64}("\s*\))',
        lambda match: match[1] + digest + match[2], checker.read_text(),
    )
    assert count == 1
    checker.write_text(payload)
    return source


def acceleration_record(target: str) -> dict[str, object]:
    """Return only synthetic public build identities expected by packaging tests."""
    cuda = "-linux-" in target or "-windows-" in target
    return {
        "ivm_features": ["cuda", "default", "metal"] if cuda else ["default", "metal"],
        "cuda_trusted_key_sha256": CUDA_KEY_SHA256 if cuda else None,
        "cuda_bundle_sha256": CUDA_BUNDLE_SHA256 if cuda else None,
    }
