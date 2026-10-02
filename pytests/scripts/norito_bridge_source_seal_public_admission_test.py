"""Public synthetic source tests; no Cargo, real materials or provider operations.

Run after guarded application with:
python3 -m pytest pytests/scripts/norito_bridge_source_seal_public_admission_test.py
The fixtures contain public sentinel bytes only, including refusal filename cases.
"""
from __future__ import annotations

import importlib.util
import os
from pathlib import Path
from unittest import mock

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("norito_source_admission", ROOT / "scripts/norito_bridge_source_seal.py")
assert SPEC is not None and SPEC.loader is not None
seal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(seal)


@pytest.mark.parametrize("relative", [
    "crates/example/fixtures/credentials.json",
    "crates/example/materials/public-sentinel.bin",
    "crates/example/fixtures/private/public-sentinel.json",
    "crates/example/operations/public-sentinel.toml",
    "crates/example/fixtures/sydneycreds.txt",
    "crates/example/fixtures/vultr-sentinel.json",
    ".cargo/credentials",
    "crates/example/fixtures/.env.example",
    "crates/example/fixtures/public-sentinel.pem",
    "crates/example/fixtures/unreviewed.opaque",
])
def test_refused_filename_is_never_opened(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    source = root / relative
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_bytes(b"PUBLIC SYNTHETIC SENTINEL; NOT A CREDENTIAL\n")
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no source open permitted")) as opened:
        with pytest.raises(RuntimeError, match="prohibited|admitted public filename"):
            seal._read_public_source_bytes(root, relative)
    opened.assert_not_called()


def test_leaf_alias_is_refused_before_any_content_descriptor(tmp_path: Path) -> None:
    root = tmp_path.resolve()
    (root / "public.rs").write_bytes(b"pub fn public_sentinel() {}\n")
    (root / "alias.rs").symlink_to("public.rs")
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no source open permitted")) as opened:
        with pytest.raises(RuntimeError, match="symlinked"):
            seal._read_public_source_bytes(root, "alias.rs")
    opened.assert_not_called()


def test_ancestor_alias_is_refused_before_any_content_descriptor(tmp_path: Path) -> None:
    root = tmp_path.resolve()
    actual = root / "public-sentinel"
    actual.mkdir()
    (actual / "lib.rs").write_bytes(b"pub fn public_sentinel() {}\n")
    (root / "bridge-src").symlink_to(actual, target_is_directory=True)
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no source open permitted")) as opened:
        with pytest.raises(RuntimeError, match="symlinked"):
            seal._read_public_source_bytes(root, "bridge-src/lib.rs")
    opened.assert_not_called()


def test_ancestor_swap_cannot_follow_alias_during_descriptor_walk(tmp_path: Path) -> None:
    root = tmp_path.resolve()
    original = root / "bridge-src"
    original.mkdir()
    (original / "lib.rs").write_bytes(b"pub fn original_public_sentinel() {}\n")
    replacement = root / "replacement-public-sentinel"
    replacement.mkdir()
    (replacement / "lib.rs").write_bytes(b"pub fn replacement_public_sentinel() {}\n")
    actual_open = os.open
    leaf_opens: list[str] = []

    def open_with_swap(path, flags, *args, **kwargs):
        if str(path) == "bridge-src":
            original.rename(root / "original-public-sentinel")
            original.symlink_to(replacement, target_is_directory=True)
        if str(path) == "lib.rs":
            leaf_opens.append(str(path))
        return actual_open(path, flags, *args, **kwargs)

    with mock.patch.object(seal.os, "open", side_effect=open_with_swap):
        with pytest.raises((OSError, RuntimeError)):
            seal._read_public_source_bytes(root, "bridge-src/lib.rs")
    assert leaf_opens == []


@pytest.mark.parametrize("relative", [
    "crates/example/src/lib.rs",
    "crates/example/src/private_signer_process.rs",
    "crates/example/src/ordinary_credential_union.rs",
    "crates/example/src/provider_policy.rs",
    "vendor/example/.cargo-checksum.json",
    "IrohaSwift/Package.resolved",
])
def test_public_original_bytes_are_preserved(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    source = root / relative
    source.parent.mkdir(parents=True, exist_ok=True)
    original = b"public synthetic source original\n"
    source.write_bytes(original)
    assert seal._read_public_source_bytes(root, relative) == original


def test_fingerprint_retains_original_name_byte_and_lock_domains(tmp_path: Path) -> None:
    import hashlib
    root = tmp_path.resolve()
    source = root / "bridge-src/lib.rs"
    source.parent.mkdir()
    original = b"pub fn public_sentinel() {}\n"
    source.write_bytes(original)
    lock = root / "Cargo.lock"
    lock.write_bytes(b"public synthetic lock original\n")
    expected = hashlib.sha256()
    expected.update(b"bridge-src/lib.rs\0" + original + b"\0")
    expected.update(b"\0selected-cargo-lock-sha256\0")
    expected.update(hashlib.sha256(lock.read_bytes()).digest())
    with mock.patch.object(seal, "listed_files", return_value=["bridge-src/lib.rs"]):
        assert seal.fingerprint(root, ["bridge-src"], lock) == expected.hexdigest()


@pytest.mark.parametrize("relative", sorted(seal._REVIEWED_PUBLIC_ANDROID_RESOURCE_INPUTS))
def test_exact_android_loader_resource_original_is_preserved(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    source = root / relative
    source.parent.mkdir(parents=True, exist_ok=True)
    original = b"public synthetic consumer/loader original; no runtime authority\n"
    source.write_bytes(original)
    assert seal._read_public_source_bytes(root, relative) == original


@pytest.mark.parametrize("relative", [
    "kotlin/client-android/src/main/resources/META-INF/services/unreviewed.Factory",
    "kotlin/client-android/src/main/resources/META-INF/services/org.hyperledger.iroha.sdk.offline.UnreviewedFactoryV1",
    "kotlin/kagemusha-wallet-android/src/main/resources/META-INF/services/org.hyperledger.iroha.sdk.offline.wallet.UnreviewedFactoryV1",
    "kotlin/client-android/src/main/resources/META-INF/proguard/unreviewed.pro",
    "kotlin/client-android/src/main/resources/META-INF/proguard/consumer-proguard-rules.pro",
    "java/iroha_android/core/src/main/resources/META-INF/proguard/other.pro",
    "kotlin/client-android/other-consumer-rules.pro",
    "kotlin/client-android/src/main/resources/META-INF/services/credentials.json",
    "kotlin/client-android/src/main/resources/META-INF/services/private/signing.json",
    "kotlin/client-android/src/main/resources/META-INF/services/public-sentinel.pem",
])
def test_android_resource_exception_never_opens_unreviewed_or_material_input(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    source = root / relative
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_bytes(b"PUBLIC SYNTHETIC SENTINEL; NOT A CREDENTIAL\n")
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no source open permitted")) as opened:
        with pytest.raises(RuntimeError, match="prohibited|admitted public filename"):
            seal._read_public_source_bytes(root, relative)
    opened.assert_not_called()


def test_android_wallet_shipping_sources_and_consumer_rules_are_seal_inputs(tmp_path: Path) -> None:
    root = tmp_path.resolve()
    (root / "Cargo.lock").write_bytes(b"public synthetic lock original\n")
    expected = {
        "kotlin/kagemusha-wallet-android/src/main",
        "kotlin/client-android/consumer-rules.pro",
        "kotlin/kagemusha-wallet-android/consumer-rules.pro",
    }
    for relative in expected:
        source = root / relative
        source.parent.mkdir(parents=True, exist_ok=True)
        if relative.endswith("src/main"):
            source.mkdir()
        else:
            source.write_bytes(b"public synthetic consumer rule\n")
    with mock.patch.object(seal, "local_dependency_roots", return_value=set()) as dependencies:
        inputs = seal.seal_inputs(root, "android", root / "Cargo.lock")
    assert expected <= set(inputs)
    assert "kotlin/kagemusha-wallet-android/src/test" not in inputs
    dependencies.assert_called_once_with(root, seal.ANDROID_TARGETS, root / "Cargo.lock")


@pytest.mark.parametrize("relative", sorted(seal._REVIEWED_PUBLIC_ANDROID_RESOURCE_INPUTS))
def test_reviewed_android_resource_ancestor_alias_is_still_refused(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    path = Path(relative)
    real = root / "public-synthetic-resource"
    real.mkdir()
    (real / path.name).write_bytes(b"public synthetic loader original\n")
    parent = root / path.parent
    parent.parent.mkdir(parents=True, exist_ok=True)
    parent.symlink_to(real, target_is_directory=True)
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no source open permitted")) as opened:
        with pytest.raises(RuntimeError, match="symlinked"):
            seal._read_public_source_bytes(root, relative)
    opened.assert_not_called()
