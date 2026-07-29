"""Focused tests for the production NoritoBridge dependency-closure seal."""

from __future__ import annotations

import importlib.util
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "norito_bridge_source_seal.py"
SPEC = importlib.util.spec_from_file_location("norito_bridge_source_seal", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
SOURCE_SEAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SOURCE_SEAL)


def _git(root: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout


@pytest.fixture
def source_fixture(tmp_path: Path) -> Path:
    root = tmp_path / "iroha"
    (root / "bridge-src").mkdir(parents=True)
    (root / ".gitignore").write_text("Cargo.lock\nbridge-src/*.cache\n", encoding="utf-8")
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    (root / "Cargo.lock").write_text("lock-v1\n", encoding="utf-8")
    (root / "bridge-src/lib.rs").write_text("pub fn bridge() {}\n", encoding="utf-8")
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "Source Seal Test")
    _git(root, "config", "user.email", "source-seal@example.invalid")
    _git(root, "add", ".gitignore", "Cargo.toml", "bridge-src/lib.rs")
    _git(root, "commit", "-qm", "fixture")
    return root


def test_explicit_ignored_root_input_is_fingerprinted(source_fixture: Path) -> None:
    inputs = ["Cargo.lock", "Cargo.toml", "bridge-src"]

    listed = SOURCE_SEAL.listed_files(source_fixture, inputs)
    before = SOURCE_SEAL.fingerprint(source_fixture, inputs)
    (source_fixture / "Cargo.lock").write_text("lock-v2\n", encoding="utf-8")
    after = SOURCE_SEAL.fingerprint(source_fixture, inputs)

    assert "Cargo.lock" in listed
    assert before != after
    # The lock remains intentionally ignored/untracked; its exact bytes are
    # bound by the fingerprint rather than misclassified as Git dirt.
    assert SOURCE_SEAL.status(source_fixture, inputs) == ""


def test_nonignored_untracked_dependency_input_is_dirty(source_fixture: Path) -> None:
    inputs = ["Cargo.lock", "Cargo.toml", "bridge-src"]
    (source_fixture / "bridge-src/new.rs").write_text("pub fn new_input() {}\n", encoding="utf-8")

    listed = SOURCE_SEAL.listed_files(source_fixture, inputs)
    status = SOURCE_SEAL.status(source_fixture, inputs)

    assert "bridge-src/new.rs" in listed
    assert "?? bridge-src/new.rs" in status


def test_unnamed_policy_ignored_file_stays_outside_seal(source_fixture: Path) -> None:
    inputs = ["Cargo.lock", "Cargo.toml", "bridge-src"]
    (source_fixture / "bridge-src/local.cache").write_text("generated\n", encoding="utf-8")

    assert "bridge-src/local.cache" not in SOURCE_SEAL.listed_files(source_fixture, inputs)
    assert SOURCE_SEAL.status(source_fixture, inputs) == ""


def test_explicit_symlink_is_rejected(source_fixture: Path) -> None:
    lock = source_fixture / "Cargo.lock"
    lock.unlink()
    lock.symlink_to("Cargo.toml")

    with pytest.raises(
        RuntimeError,
        match="selected Cargo lock must be a non-symbolic regular file",
    ):
        SOURCE_SEAL.listed_files(source_fixture, ["Cargo.lock", "Cargo.toml"])


def test_nested_dependency_symlink_is_rejected(source_fixture: Path) -> None:
    link = source_fixture / "bridge-src/external.rs"
    link.symlink_to(source_fixture / "Cargo.toml")
    _git(source_fixture, "add", "bridge-src/external.rs")

    with pytest.raises(RuntimeError, match="source-seal input is symlinked"):
        SOURCE_SEAL.fingerprint(source_fixture, ["Cargo.toml", "bridge-src"])


def test_android_inputs_and_targets_are_platform_specific(
    source_fixture: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    android_gradle = source_fixture / "kotlin/client-android/build.gradle.kts"
    android_gradle.parent.mkdir(parents=True)
    android_gradle.write_text("plugins {}\n", encoding="utf-8")
    kotlin_source = (
        source_fixture
        / "kotlin/core-jvm/src/main/java/example/CanonicalTransaction.kt"
    )
    kotlin_source.parent.mkdir(parents=True)
    kotlin_source.write_text("internal object CanonicalTransaction\n", encoding="utf-8")
    java_source = (
        source_fixture
        / "java/iroha_android/src/main/java/example/CanonicalTransaction.java"
    )
    java_source.parent.mkdir(parents=True)
    java_source.write_text(
        "final class CanonicalTransaction {}\n",
        encoding="utf-8",
    )
    apple_package = source_fixture / "IrohaSwift/Package.swift"
    apple_package.parent.mkdir(parents=True)
    apple_package.write_text("// fixture\n", encoding="utf-8")
    observed_targets: list[tuple[str, ...]] = []

    def dependency_roots(
        _root: Path,
        targets: tuple[str, ...],
        _lockfile_path: Path | None = None,
    ) -> set[str]:
        observed_targets.append(tuple(targets))
        return {"bridge-src"}

    monkeypatch.setattr(SOURCE_SEAL, "local_dependency_roots", dependency_roots)

    android_inputs = SOURCE_SEAL.seal_inputs(source_fixture, "android")
    apple_inputs = SOURCE_SEAL.seal_inputs(source_fixture, "apple")

    assert "kotlin/client-android/build.gradle.kts" in android_inputs
    assert "kotlin/core-jvm/src/main" in android_inputs
    assert "java/iroha_android/src/main" in android_inputs
    assert "IrohaSwift/Package.swift" not in android_inputs
    assert "IrohaSwift/Package.swift" in apple_inputs
    assert "kotlin/client-android/build.gradle.kts" not in apple_inputs
    assert "kotlin/core-jvm/src/main" not in apple_inputs
    assert "java/iroha_android/src/main" not in apple_inputs
    assert observed_targets == [
        SOURCE_SEAL.ANDROID_TARGETS,
        SOURCE_SEAL.APPLE_TARGETS,
    ]


@pytest.mark.parametrize(
    "relative_path",
    (
        "kotlin/core-jvm/src/main/java/example/Core.kt",
        "kotlin/client-android/src/main/java/example/Client.kt",
        "java/norito_java/src/main/java/example/Norito.java",
        "java/iroha_android/src/main/java/example/Core.java",
        "java/iroha_android/android/src/main/java/example/Android.java",
    ),
)
def test_android_fingerprint_binds_each_shipping_jvm_source_tree(
    source_fixture: Path,
    monkeypatch: pytest.MonkeyPatch,
    relative_path: str,
) -> None:
    source = source_fixture / relative_path
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_text("shipping-source-v1\n", encoding="utf-8")
    _git(source_fixture, "add", relative_path)
    _git(source_fixture, "commit", "-qm", f"add {relative_path}")
    monkeypatch.setattr(
        SOURCE_SEAL,
        "local_dependency_roots",
        lambda _root, _targets, _lockfile_path=None: {"bridge-src"},
    )

    android_inputs = SOURCE_SEAL.seal_inputs(source_fixture, "android")
    before = SOURCE_SEAL.fingerprint(source_fixture, android_inputs)
    source.write_text("shipping-source-v2\n", encoding="utf-8")
    after = SOURCE_SEAL.fingerprint(source_fixture, android_inputs)

    assert relative_path in SOURCE_SEAL.listed_files(
        source_fixture,
        android_inputs,
    )
    assert before != after


def test_android_native_cleanup_passes_each_target_set_in_one_delete_call() -> None:
    """Guard Gradle DeleteSpec's replacement, rather than additive, path semantics."""

    build_script = (ROOT / "kotlin/client-android/build.gradle.kts").read_text(
        encoding="utf-8"
    )

    assert (
        build_script.count(
            "delete(outputRoot, stagingRoot, sealFile, environmentFile)"
        )
        == 1
    )
    assert "delete(outputRoot, stagingRoot, sealFile)" not in build_script
    assert build_script.count("delete(outputRoot, provenanceRoot)") == 1
    assert "delete(outputRoot)\n            delete(stagingRoot)" not in build_script
    assert "delete(sealFile)\n            delete(environmentFile)" not in build_script
    assert "delete(outputRoot)\n            delete(provenanceRoot)" not in build_script


def test_android_snapshot_rejects_source_change_between_abi_builds(
    source_fixture: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    inputs = ["Cargo.lock", "Cargo.toml", "bridge-src"]
    monkeypatch.setattr(
        SOURCE_SEAL,
        "seal_inputs",
        lambda _root, platform="apple", _lockfile_path=None: inputs,
    )
    snapshot_path = source_fixture / "android-source-seal.json"
    snapshot_path.write_bytes(SOURCE_SEAL.snapshot_bytes(source_fixture, "android"))

    SOURCE_SEAL.verify_snapshot(source_fixture, "android", snapshot_path)
    (source_fixture / "bridge-src/lib.rs").write_text(
        "pub fn changed_between_abis() {}\n", encoding="utf-8"
    )

    with pytest.raises(RuntimeError, match="source changed after the build started"):
        SOURCE_SEAL.verify_snapshot(source_fixture, "android", snapshot_path)


def test_android_snapshot_rejects_commit_drift_with_unchanged_selected_source(
    source_fixture: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    inputs = ["Cargo.lock", "Cargo.toml", "bridge-src"]
    monkeypatch.setattr(
        SOURCE_SEAL,
        "seal_inputs",
        lambda _root, platform="apple", _lockfile_path=None: inputs,
    )
    snapshot_path = source_fixture / "android-source-seal.json"
    snapshot_path.write_bytes(SOURCE_SEAL.snapshot_bytes(source_fixture, "android"))
    fingerprint_before = SOURCE_SEAL.fingerprint(source_fixture, inputs)

    _git(source_fixture, "commit", "--allow-empty", "-qm", "move head only")

    assert SOURCE_SEAL.fingerprint(source_fixture, inputs) == fingerprint_before
    with pytest.raises(RuntimeError, match="source changed after the build started"):
        SOURCE_SEAL.verify_snapshot(source_fixture, "android", snapshot_path)


def test_android_snapshot_rejects_commit_drift_during_authentication(
    source_fixture: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(
        SOURCE_SEAL,
        "seal_inputs",
        lambda _root, platform="apple", _lockfile_path=None: ["Cargo.toml"],
    )
    commits = iter(("0" * 40, "1" * 40))
    monkeypatch.setattr(SOURCE_SEAL, "source_commit", lambda _root: next(commits))
    monkeypatch.setattr(
        SOURCE_SEAL,
        "status",
        lambda _root, _inputs, _lockfile_path=None: "",
    )
    monkeypatch.setattr(
        SOURCE_SEAL,
        "fingerprint",
        lambda _root, _inputs, _lockfile_path=None: "a" * 64,
    )

    with pytest.raises(RuntimeError, match="source commit changed while authenticating"):
        SOURCE_SEAL.snapshot(source_fixture, "android")


def test_android_snapshot_rejects_selected_source_drift_during_authentication(
    source_fixture: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(
        SOURCE_SEAL,
        "seal_inputs",
        lambda _root, platform="apple", _lockfile_path=None: ["Cargo.toml"],
    )
    fingerprints = iter(("a" * 64, "b" * 64))
    monkeypatch.setattr(SOURCE_SEAL, "source_commit", lambda _root: "0" * 40)
    monkeypatch.setattr(
        SOURCE_SEAL,
        "status",
        lambda _root, _inputs, _lockfile_path=None: "",
    )
    monkeypatch.setattr(
        SOURCE_SEAL,
        "fingerprint",
        lambda _root, _inputs, _lockfile_path=None: next(fingerprints),
    )

    with pytest.raises(RuntimeError, match="selected source changed while authenticating"):
        SOURCE_SEAL.snapshot(source_fixture, "android")


def test_android_snapshot_rejects_tampering(
    source_fixture: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(
        SOURCE_SEAL,
        "seal_inputs",
        lambda _root, platform="apple", _lockfile_path=None: [
            "Cargo.lock",
            "Cargo.toml",
            "bridge-src",
        ],
    )
    snapshot_path = source_fixture / "android-source-seal.json"
    snapshot_path.write_bytes(SOURCE_SEAL.snapshot_bytes(source_fixture, "android"))
    snapshot_path.write_bytes(snapshot_path.read_bytes().replace(b'"android"', b'"apple"'))

    with pytest.raises(RuntimeError, match="source changed after the build started"):
        SOURCE_SEAL.verify_snapshot(source_fixture, "android", snapshot_path)


def test_android_promotions_authenticate_source_immediately_before_and_after_copy() -> None:
    build_script = (ROOT / "kotlin/client-android/build.gradle.kts").read_text(
        encoding="utf-8"
    )

    pre_promotion = build_script.index(
        '"$abi immediate pre-promotion authentication"',
    )
    copy = build_script.index(
        "Files.copy(\n                stagedLibrary.toPath(),",
        pre_promotion,
    )
    post_promotion = build_script.index(
        '"$abi immediate post-promotion authentication"',
        copy,
    )
    assert pre_promotion < copy < post_promotion

    stripped_pre_promotion = build_script.index(
        '"stripped artifact immediate pre-promotion authentication"',
    )
    provenance_write = build_script.index(
        "provenanceFile.writeText(",
        stripped_pre_promotion,
    )
    stripped_post_promotion = build_script.index(
        '"stripped artifact immediate post-promotion authentication"',
        provenance_write,
    )
    assert stripped_pre_promotion < provenance_write < stripped_post_promotion
