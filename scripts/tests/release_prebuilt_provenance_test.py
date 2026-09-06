"""Regression coverage for authenticated release-prebuilt provenance."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest


REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "verify_release_prebuilt_provenance.py"
ISOLATED_RUNNER = REPO / "scripts" / "run_isolated_release_tool.py"
SOURCE_COMMIT = "a" * 40
TARGET = "x86_64-unknown-linux-gnu"


def load_verifier():
    runner = load_isolated_runner()
    runner._load_contract()
    spec = importlib.util.spec_from_file_location(
        "verify_release_prebuilt_provenance", SCRIPT
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_isolated_runner():
    spec = importlib.util.spec_from_file_location(
        "run_isolated_release_tool", ISOLATED_RUNNER
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def prepare_prebuilt(tmp_path: Path):
    verifier = load_verifier()
    directory = tmp_path / "prebuilt"
    directory.mkdir()
    binary = directory / "iroha3d"
    binary.write_bytes(b"reviewed iroha3d binary\n")
    binary.chmod(0o755)
    cargo_lock = tmp_path / "Cargo.lock"
    cargo_lock.write_bytes(b"reviewed Cargo.lock\n")
    cargo_lock.chmod(0o644)

    def write_manifest() -> str:
        payload = verifier.canonical_json_bytes(
            {
                "schema": verifier.SCHEMA,
                "schema_version": verifier.SCHEMA_VERSION,
                "source_commit": SOURCE_COMMIT,
                "cargo_lock_sha256": hashlib.sha256(
                    cargo_lock.read_bytes()
                ).hexdigest(),
                "target": TARGET,
                "cargo_profile": "deploy",
                "default_features": True,
                "selected_features": [],
                "binaries": [
                    {
                        "name": "iroha3d",
                        "package": "irohad",
                        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                        "size": binary.stat().st_size,
                    }
                ],
            }
        )
        manifest = directory / verifier.MANIFEST_NAME
        manifest.write_bytes(payload)
        manifest.chmod(0o644)
        return hashlib.sha256(payload).hexdigest()

    return verifier, directory, binary, cargo_lock, write_manifest


def verify(verifier, directory, cargo_lock, digest, output):
    return verifier.verify_prebuilt_directory(
        directory,
        trusted_manifest_sha256=digest,
        source_commit=SOURCE_COMMIT,
        cargo_lock=cargo_lock,
        target=TARGET,
        cargo_profile="deploy",
        selected_features=(),
        binaries={"iroha3d": "irohad"},
        output_directory=output,
    )


def test_authenticated_prebuilt_manifest_creates_private_binary_snapshot(
    tmp_path: Path,
) -> None:
    verifier, directory, binary, cargo_lock, write_manifest = prepare_prebuilt(
        tmp_path
    )
    digest = write_manifest()

    assert verify(
        verifier, directory, cargo_lock, digest, tmp_path / "snapshot"
    ) == digest
    snapshot = tmp_path / "snapshot/iroha3d"
    assert snapshot.read_bytes() == binary.read_bytes()
    assert snapshot.stat().st_mode & 0o777 == 0o755


def test_prebuilt_binary_tampering_cannot_be_self_attested(tmp_path: Path) -> None:
    verifier, directory, binary, cargo_lock, write_manifest = prepare_prebuilt(
        tmp_path
    )
    trusted_digest = write_manifest()
    binary.write_bytes(b"unreviewed replacement\n")
    binary.chmod(0o755)

    try:
        verify(
            verifier,
            directory,
            cargo_lock,
            trusted_digest,
            tmp_path / "first-snapshot",
        )
    except verifier.ReleaseArtifactError as error:
        assert "does not match authenticated provenance" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("binary drift was accepted")

    assert write_manifest() != trusted_digest
    try:
        verify(
            verifier,
            directory,
            cargo_lock,
            trusted_digest,
            tmp_path / "second-snapshot",
        )
    except verifier.ReleaseArtifactError as error:
        assert "manifest SHA256 is not the reviewed digest" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("self-attested replacement manifest was accepted")


def test_prebuilt_provenance_binds_release_metadata_and_closed_inventory(
    tmp_path: Path,
) -> None:
    verifier, directory, _binary, cargo_lock, write_manifest = prepare_prebuilt(
        tmp_path
    )
    digest = write_manifest()
    try:
        verifier.verify_prebuilt_directory(
            directory,
            trusted_manifest_sha256=digest,
            source_commit="b" * 40,
            cargo_lock=cargo_lock,
            target=TARGET,
            cargo_profile="deploy",
            selected_features=(),
            binaries={"iroha3d": "irohad"},
            output_directory=tmp_path / "metadata-snapshot",
        )
    except verifier.ReleaseArtifactError as error:
        assert "source_commit does not match" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("source-commit mismatch was accepted")

    (directory / "unreviewed-helper").write_bytes(b"extra\n")
    try:
        verify(
            verifier,
            directory,
            cargo_lock,
            digest,
            tmp_path / "inventory-snapshot",
        )
    except verifier.ReleaseArtifactError as error:
        assert "inventory must contain exactly" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("extra prebuilt input was accepted")


@pytest.mark.parametrize(
    ("field", "replacement", "message"),
    (
        ("cargo_lock_sha256", "0" * 64, "cargo_lock_sha256 does not match"),
        ("schema_version", True, "schema_version must be an integer"),
        ("target", "aarch64-unknown-linux-gnu", "target does not match"),
        ("cargo_profile", "release", "cargo_profile does not match"),
        ("default_features", False, "default_features does not match"),
        ("default_features", 1, "default_features must be a boolean"),
        ("selected_features", ["test-fixtures"], "selected_features does not match"),
    ),
)
def test_prebuilt_provenance_rejects_each_release_profile_mismatch(
    tmp_path: Path,
    field: str,
    replacement: object,
    message: str,
) -> None:
    verifier, directory, _binary, cargo_lock, write_manifest = prepare_prebuilt(
        tmp_path
    )
    write_manifest()
    manifest_path = directory / verifier.MANIFEST_NAME
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest[field] = replacement
    payload = verifier.canonical_json_bytes(manifest)
    manifest_path.write_bytes(payload)
    digest = hashlib.sha256(payload).hexdigest()

    try:
        verify(
            verifier,
            directory,
            cargo_lock,
            digest,
            tmp_path / "profile-snapshot",
        )
    except verifier.ReleaseArtifactError as error:
        assert message in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError(f"prebuilt provenance mismatch was accepted: {field}")


def test_prebuilt_provenance_rejects_binary_package_mismatch(tmp_path: Path) -> None:
    verifier, directory, _binary, cargo_lock, write_manifest = prepare_prebuilt(
        tmp_path
    )
    write_manifest()
    manifest_path = directory / verifier.MANIFEST_NAME
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["binaries"][0]["package"] = "unreviewed-package"
    payload = verifier.canonical_json_bytes(manifest)
    manifest_path.write_bytes(payload)
    digest = hashlib.sha256(payload).hexdigest()

    try:
        verify(
            verifier,
            directory,
            cargo_lock,
            digest,
            tmp_path / "package-snapshot",
        )
    except verifier.ReleaseArtifactError as error:
        assert "package does not match binary" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("prebuilt binary package mismatch was accepted")


def test_isolated_verifier_ignores_an_ignored_stdlib_shadow(tmp_path: Path) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    verifier = scripts / SCRIPT.name
    contract = scripts / "release_artifact_contract.py"
    runner = scripts / "run_isolated_release_tool.py"
    shutil.copyfile(SCRIPT, verifier)
    shutil.copyfile(REPO / "scripts/release_artifact_contract.py", contract)
    shutil.copyfile(ISOLATED_RUNNER, runner)
    marker = tmp_path / "shadow-executed"
    (scripts / "json.py").write_text(
        f"open({str(marker)!r}, 'w', encoding='utf-8').write('executed')\n"
        "raise RuntimeError('ignored json shadow executed')\n",
        encoding="utf-8",
    )
    (tmp_path / ".gitignore").write_text("/scripts/json.py\n", encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(runner),
            str(verifier),
            "--help",
        ],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert not marker.exists()


@pytest.mark.parametrize(
    "source_name",
    ("release_artifact_contract.py", "verify_release_prebuilt_provenance.py"),
)
@pytest.mark.parametrize("attack", ("content", "hardlink", "mode", "symlink"))
def test_isolated_verifier_rejects_bootstrap_source_before_side_effects(
    tmp_path: Path, source_name: str, attack: str
) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in (
        "run_isolated_release_tool.py",
        "release_artifact_contract.py",
        "verify_release_prebuilt_provenance.py",
    ):
        shutil.copyfile(REPO / "scripts" / name, scripts / name)
    marker = tmp_path / f"{source_name}-{attack}-executed"
    malicious = (
        "from pathlib import Path\n"
        f"Path({str(marker)!r}).write_text('executed', encoding='utf-8')\n"
    ).encode()
    victim = scripts / source_name
    if attack == "content":
        victim.write_bytes(malicious)
    elif attack == "hardlink":
        victim.unlink()
        backing = tmp_path / f"{source_name}.backing"
        backing.write_bytes(malicious)
        os.link(backing, victim)
    elif attack == "mode":
        victim.write_bytes(malicious)
        victim.chmod(0o664)
    else:
        victim.unlink()
        backing = tmp_path / f"{source_name}.backing"
        backing.write_bytes(malicious)
        victim.symlink_to(backing)

    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(scripts / "run_isolated_release_tool.py"),
            str(scripts / "verify_release_prebuilt_provenance.py"),
            "--help",
        ],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode != 0
    assert not marker.exists()


def test_release_builders_launch_prebuilt_verifier_in_isolated_mode() -> None:
    marker = (
        '"${release_python[@]}" "$repo_root/scripts/'
        'verify_release_prebuilt_provenance.py"'
    )
    for relative in (
        "scripts/build_release_bundle.sh",
        "scripts/build_release_image.sh",
    ):
        source = (REPO / relative).read_text(encoding="utf-8")
        assert source.count(marker) == 1
        assert source.count(
            'release_python=(python3 -I -S "$repo_root/scripts/'
            'run_isolated_release_tool.py")'
        ) == 1
        assert re.search(r'python3[ \t]+"\$repo_root/scripts/', source) is None
        assert len(re.findall(r"(?m)^validate_release_source$", source)) == 2
        for environment_marker in (
            "CARGO_ENCODED_RUSTFLAGS CARGO_ENCODED_RUSTDOCFLAGS CARGO_HOME",
            "RUSTC RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTDOC RUSTDOCFLAGS RUSTFLAGS",
            "for release_environment_name in ${!CARGO_BUILD_@}; do",
            "for release_environment_name in ${!CARGO_TARGET_@}; do",
            "*_LINKER|*_RUNNER|*_RUSTFLAGS|*_RUSTDOCFLAGS)",
        ):
            assert environment_marker in source
    bundle = (REPO / "scripts/build_release_bundle.sh").read_text(encoding="utf-8")
    assert "cargo build" not in bundle
    assert "--prebuilt-bin-dir is required for deterministic release bundles" in bundle


@pytest.mark.parametrize(
    "builder", ("build_release_bundle.sh", "build_release_image.sh")
)
def test_release_builder_privileged_shell_ignores_hostile_startup_environment(
    tmp_path: Path, builder: str
) -> None:
    marker = tmp_path / f"{builder}-shell-hook-executed"
    hook = tmp_path / "bash-env-hook"
    hook.write_text(f"touch {str(marker)!r}\n", encoding="utf-8")
    environment = os.environ.copy()
    environment.update(
        {
            "BASH_ENV": str(hook),
            "SHELLOPTS": "xtrace",
            "PS4": f"$(touch {str(marker)!r})",
            "BASH_FUNC_cat%%": f"() {{ touch {str(marker)!r}; }}",
        }
    )
    completed = subprocess.run(
        [str(REPO / "scripts" / builder), "--help"],
        cwd=REPO,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert "Usage:" in completed.stdout
    assert not marker.exists()


@pytest.mark.parametrize(
    "builder",
    ("build_release_bundle.sh", "build_release_image.sh"),
)
def test_builder_isolated_helpers_ignore_post_verification_stdlib_shadows(
    tmp_path: Path, builder: str
) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in (
        "run_isolated_release_tool.py",
        "release_artifact_contract.py",
        "copy_release_file.py",
    ):
        shutil.copyfile(REPO / "scripts" / name, scripts / name)
    marker = tmp_path / f"{builder}-shadow-executed"
    shadow = (
        f"open({str(marker)!r}, 'w', encoding='utf-8').write('executed')\n"
        "raise RuntimeError('ignored stdlib shadow executed')\n"
    )
    (scripts / "json.py").write_text(shadow, encoding="utf-8")
    (scripts / "hashlib.py").write_text(shadow, encoding="utf-8")
    (tmp_path / ".gitignore").write_text(
        "/scripts/json.py\n/scripts/hashlib.py\n", encoding="utf-8"
    )
    source = tmp_path / "input.bin"
    output = tmp_path / "output.bin"
    source.write_bytes(b"reviewed private snapshot\n")

    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(scripts / "run_isolated_release_tool.py"),
            str(scripts / "copy_release_file.py"),
            "--source",
            str(source),
            "--output",
            str(output),
            "--mode",
            "0644",
        ],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert output.read_bytes() == source.read_bytes()
    assert not marker.exists()
    builder_source = (REPO / "scripts" / builder).read_text(encoding="utf-8")
    assert 'run_isolated_release_tool.py")' in builder_source


@pytest.mark.parametrize(
    "hardlinked_name", ("release_artifact_contract.py", "copy_release_file.py")
)
def test_isolated_release_runner_rejects_hardlinked_sources(
    tmp_path: Path, hardlinked_name: str
) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in (
        "run_isolated_release_tool.py",
        "release_artifact_contract.py",
        "copy_release_file.py",
    ):
        shutil.copyfile(REPO / "scripts" / name, scripts / name)
    victim = scripts / hardlinked_name
    payload = victim.read_bytes()
    victim.unlink()
    backing = tmp_path / f"{hardlinked_name}.backing"
    backing.write_bytes(payload)
    os.link(backing, victim)

    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(scripts / "run_isolated_release_tool.py"),
            str(scripts / "copy_release_file.py"),
            "--help",
        ],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode != 0
    assert "singly linked" in completed.stderr


@pytest.mark.parametrize(
    "writable_name", ("release_artifact_contract.py", "copy_release_file.py")
)
def test_isolated_release_runner_rejects_shared_writable_sources(
    tmp_path: Path, writable_name: str
) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in (
        "run_isolated_release_tool.py",
        "release_artifact_contract.py",
        "copy_release_file.py",
    ):
        shutil.copyfile(REPO / "scripts" / name, scripts / name)
    (scripts / writable_name).chmod(0o666)
    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(scripts / "run_isolated_release_tool.py"),
            str(scripts / "copy_release_file.py"),
            "--help",
        ],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode != 0
    assert "non-shared-writable regular file" in completed.stderr


def test_isolated_release_runner_rejects_shared_writable_script_directory(
    tmp_path: Path,
) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in (
        "run_isolated_release_tool.py",
        "release_artifact_contract.py",
        "copy_release_file.py",
    ):
        shutil.copyfile(REPO / "scripts" / name, scripts / name)
    scripts.chmod(0o777)
    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(scripts / "run_isolated_release_tool.py"),
            str(scripts / "copy_release_file.py"),
            "--help",
        ],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode != 0
    assert "directory must not be group- or world-writable" in completed.stderr


@pytest.mark.parametrize(
    "target_name",
    ("copy_release_file.py", "verify_release_prebuilt_provenance.py"),
)
def test_isolated_release_runner_rejects_a_concurrent_path_swap(
    tmp_path: Path, monkeypatch, target_name: str
) -> None:
    runner = load_isolated_runner()
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    target = scripts / target_name
    original = b"captured = 'reviewed'\n"
    target.write_bytes(original)
    runner.SCRIPT_DIRECTORY = scripts
    real_read = runner.os.read
    swapped = False

    def swap_then_read(descriptor: int, size: int) -> bytes:
        nonlocal swapped
        if not swapped:
            swapped = True
            target.rename(scripts / "reviewed.saved")
            target.write_bytes(b"raise RuntimeError('replacement executed')\n")
        return real_read(descriptor, size)

    monkeypatch.setattr(runner.os, "read", swap_then_read)
    with pytest.raises(RuntimeError, match="changed while being read"):
        runner._stable_regular_sibling(target.name)
    assert swapped


def test_isolated_pipeline_ignores_shadow_modules_and_hostile_python_env(
    tmp_path: Path,
) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    dependencies = (
        "run_release_pipeline.py",
        "release_artifact_contract.py",
        "release_manifest_signing.py",
        "publish_plan.py",
        "check_release_feature_graph.py",
    )
    for name in dependencies:
        shutil.copyfile(REPO / "scripts" / name, scripts / name)
    marker = tmp_path / "pipeline-shadow-executed"
    (scripts / "json.py").write_text(
        f"open({str(marker)!r}, 'w', encoding='utf-8').write('executed')\n"
        "raise RuntimeError('ignored json shadow executed')\n",
        encoding="utf-8",
    )
    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(scripts)
    environment["PYTHONHOME"] = str(tmp_path / "missing-python-home")
    environment["BASH_ENV"] = str(scripts / "json.py")

    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(scripts / "run_release_pipeline.py"),
            "--help",
        ],
        cwd=tmp_path,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert not marker.exists()


def test_nested_release_helper_uses_safe_path_and_ignores_stdlib_shadow(
    tmp_path: Path,
) -> None:
    scripts = tmp_path / "scripts"
    fastpq = scripts / "fastpq"
    fastpq.mkdir(parents=True)
    for name in ("run_isolated_release_tool.py", "release_artifact_contract.py"):
        shutil.copyfile(REPO / "scripts" / name, scripts / name)
    for name in (
        "__init__.py",
        "rollout_manifest_summary.py",
        "validate_row_usage_snapshot.py",
        "wrap_benchmark.py",
    ):
        shutil.copyfile(REPO / "scripts" / "fastpq" / name, fastpq / name)
    acceleration = scripts / "acceleration"
    acceleration.mkdir()
    shutil.copyfile(
        REPO / "scripts" / "acceleration" / "export_prometheus.py",
        acceleration / "export_prometheus.py",
    )
    marker = tmp_path / "nested-shadow-executed"
    (fastpq / "json.py").write_text(
        f"open({str(marker)!r}, 'w', encoding='utf-8').write('executed')\n"
        "raise RuntimeError('ignored nested json shadow executed')\n",
        encoding="utf-8",
    )
    (tmp_path / "scripts.py").write_text(
        f"open({str(marker)!r}, 'w', encoding='utf-8').write('executed')\n"
        "raise RuntimeError('ignored root scripts module executed')\n",
        encoding="utf-8",
    )
    (tmp_path / ".gitignore").write_text(
        "/scripts/fastpq/json.py\n/scripts.py\n", encoding="utf-8"
    )
    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(scripts / "run_isolated_release_tool.py"),
            str(fastpq / "rollout_manifest_summary.py"),
            "--help",
        ],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert not marker.exists()

    for unsafe_directory in (fastpq, acceleration):
        unsafe_directory.chmod(0o777)
        rejected = subprocess.run(
            [
                sys.executable,
                "-I",
                "-S",
                str(scripts / "run_isolated_release_tool.py"),
                str(fastpq / "rollout_manifest_summary.py"),
                "--help",
            ],
            cwd=tmp_path,
            check=False,
            capture_output=True,
            text=True,
        )
        assert rejected.returncode != 0
        assert "parent must not be group- or world-writable" in rejected.stderr
        unsafe_directory.chmod(0o755)
