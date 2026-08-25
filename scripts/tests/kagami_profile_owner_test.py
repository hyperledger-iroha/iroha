"""Pure tests for the proposed Kagami profile generated-output owner."""

from __future__ import annotations

import hashlib
import importlib.util
import os
from pathlib import Path
import stat
import sys

import pytest

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10
    import tomli as tomllib


CACHE_DIR = Path(__file__).resolve().parent
DEFAULT_HELPER = CACHE_DIR / "kagami_profile_owner.py"
if not DEFAULT_HELPER.exists():
    DEFAULT_HELPER = CACHE_DIR.parent / "kagami_profile_owner.py"
HELPER = Path(os.environ.get("KAGAMI_PROFILE_OWNER_UNDER_TEST", DEFAULT_HELPER))
DEFAULT_REPO_ROOT = Path(__file__).resolve().parents[2]
REPO_ROOT = Path(
    os.environ.get("IROHA_REPO_ROOT_UNDER_TEST", DEFAULT_REPO_ROOT)
).resolve()
DEFAULT_POST_MANIFEST = CACHE_DIR / "generated-files.post.toml"
if not DEFAULT_POST_MANIFEST.exists():
    DEFAULT_POST_MANIFEST = REPO_ROOT / "generated-files.toml"
POST_MANIFEST = Path(
    os.environ.get("GENERATED_FILES_UNDER_TEST", DEFAULT_POST_MANIFEST)
)

SPEC = importlib.util.spec_from_file_location("kagami_profile_owner", HELPER)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)
MODULE.REPO_ROOT = REPO_ROOT
MODULE.ROOT_CARGO_LOCK = REPO_ROOT / "Cargo.lock"


DEV_FILES = {
    "README.md",
    "config-peer-1.toml",
    "config-peer-2.toml",
    "config-peer-3.toml",
    "config.toml",
    "docker-compose.yml",
    "genesis.expected_hash",
    "genesis.json",
    "genesis.public_key",
    "genesis.signed.nrt",
    "peer0.toml",
    "peer1.toml",
    "peer2.toml",
    "peer3.toml",
    "verify.txt",
}
def _write_dummy_stage(root: Path, profile: str) -> None:
    for relative in MODULE._expected_paths(profile):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f"{relative}\n".encode())


def _valid_cli_tail(tmp_path: Path) -> list[str]:
    cargo = tmp_path / "cargo"
    cargo.write_text("cargo fixture\n", encoding="utf-8")
    cargo.chmod(0o700)
    target = tmp_path / "target"
    target.mkdir(mode=0o700)
    return [
        "--cargo",
        str(cargo),
        "--cargo-target-dir",
        str(target),
        "--cargo-lock-size",
        "311202",
        "--cargo-lock-sha256",
        "ad0d209abaa51d4c77a9e67ccbb0c7660a0f8b7b5dbe3e3fbe4a70e142711bf7",
    ]


def test_profile_allowlist_and_closed_inventories_match_present_bundles() -> None:
    assert set(MODULE.PROFILE_FILES) == {"iroha3-dev"}
    assert set(MODULE.PROFILE_FILES["iroha3-dev"]) == DEV_FILES
    assert len(DEV_FILES) == 15
    present = {
        path.name
        for path in (REPO_ROOT / "defaults" / "kagami" / "iroha3-dev").iterdir()
        if path.is_file()
    }
    assert present == DEV_FILES


def test_profile_command_always_pins_one_profile_output_and_kagami() -> None:
    tools = MODULE.BuiltTools(Path("/external/target/debug/xtask"), Path("/external/target/debug/kagami"))
    command = MODULE._profile_command(tools, "iroha3-dev", Path("/external/stage"))
    assert command == [
        "/external/target/debug/xtask",
        "kagami-profiles",
        "--profile",
        "iroha3-dev",
        "--out",
        "/external/stage/defaults/kagami",
        "--kagami",
        "/external/target/debug/kagami",
    ]
    assert "iroha3-nexus" not in command
    assert "all" not in command


def test_cargo_build_command_is_locked_offline_and_uses_exact_root_lock() -> None:
    expectation = MODULE.LockExpectation(321032, "ab" * 32)
    command = MODULE._cargo_command(Path("/toolchain/cargo"), "xtask", "xtask", expectation)
    for item in ("--locked", "--offline", "--jobs", "1", "--lockfile-path"):
        assert item in command
    assert str(MODULE.ROOT_CARGO_LOCK) in command
    assert command[-6:] == ["-p", "xtask", "--features", "dev-tools", "--bin", "xtask"]


def test_stage_snapshot_rejects_missing_extra_symlink_and_hardlink(tmp_path: Path) -> None:
    root = tmp_path / "stage"
    root.mkdir(mode=0o700)
    _write_dummy_stage(root, "iroha3-dev")
    baseline = MODULE._snapshot(root, "iroha3-dev", closed_stage=True)
    assert len(baseline) == 15

    extra = root / "defaults" / "kagami" / "iroha3-dev" / "extra"
    extra.write_bytes(b"extra")
    with pytest.raises(MODULE.OwnerError, match="topology mismatch"):
        MODULE._snapshot(root, "iroha3-dev", closed_stage=True)
    extra.unlink()

    victim = root / next(iter(MODULE._expected_paths("iroha3-dev")))
    original = victim.read_bytes()
    victim.unlink()
    victim.symlink_to("README.md")
    with pytest.raises(MODULE.OwnerError, match="single-link regular"):
        MODULE._snapshot(root, "iroha3-dev", closed_stage=True)
    victim.unlink()
    victim.write_bytes(original)

    peer = root / "defaults" / "kagami" / "iroha3-dev" / "peer0.toml"
    peer.unlink()
    os.link(root / "defaults" / "kagami" / "iroha3-dev" / "config.toml", peer)
    with pytest.raises(MODULE.OwnerError, match="single-link regular"):
        MODULE._snapshot(root, "iroha3-dev", closed_stage=True)


def test_snapshot_comparison_is_byte_exact() -> None:
    first = {"x": MODULE.ManagedFile(1, hashlib.sha256(b"a").hexdigest(), b"a")}
    same = {"x": MODULE.ManagedFile(1, hashlib.sha256(b"a").hexdigest(), b"a")}
    drift = {"x": MODULE.ManagedFile(1, hashlib.sha256(b"b").hexdigest(), b"b")}
    MODULE._compare_snapshots(first, same, "same")
    with pytest.raises(MODULE.OwnerError, match="byte drift"):
        MODULE._compare_snapshots(first, drift, "drift")


def test_lock_authentication_checks_identity_size_digest_and_permissions(tmp_path: Path) -> None:
    lock = tmp_path / "Cargo.lock"
    lock.write_bytes(b"sealed lock bytes")
    lock.chmod(0o600)
    expected = MODULE.LockExpectation(len(b"sealed lock bytes"), hashlib.sha256(b"sealed lock bytes").hexdigest())
    assert MODULE._authenticate_lock(lock, expected) == b"sealed lock bytes"
    with pytest.raises(MODULE.OwnerError, match="byte length drifted"):
        MODULE._authenticate_lock(lock, MODULE.LockExpectation(1, expected.sha256))
    with pytest.raises(MODULE.OwnerError, match="SHA-256 drifted"):
        MODULE._authenticate_lock(lock, MODULE.LockExpectation(expected.byte_length, "00" * 32))
    lock.chmod(0o620)
    with pytest.raises(MODULE.OwnerError, match="group- or world-writable"):
        MODULE._authenticate_lock(lock, expected)


def test_absent_root_is_external_private_normalized_and_nonoverlapping(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    admitted = MODULE._absent_external_root(str(tmp_path / "stage"), "stage")
    assert admitted == tmp_path / "stage"
    (tmp_path / "stage").mkdir()
    with pytest.raises(MODULE.OwnerError, match="must be absent"):
        MODULE._absent_external_root(str(tmp_path / "stage"), "stage")
    with pytest.raises(MODULE.OwnerError, match="source repository|group or world permissions"):
        MODULE._absent_external_root(str(REPO_ROOT / "stage"), "stage")
    with pytest.raises(MODULE.OwnerError, match="normalized absolute"):
        MODULE._absent_external_root("relative/stage", "stage")


@pytest.mark.skipif(
    not (sys.platform == "darwin" or sys.platform.startswith("linux")),
    reason="atomic no-replace primitive is intentionally fail-closed elsewhere",
)
def test_atomic_directory_publish_never_replaces_existing_destination(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    source = tmp_path / "source"
    source.mkdir()
    (source / "sentinel").write_bytes(b"first")
    destination = tmp_path / "destination"
    MODULE._rename_no_replace(source, destination)
    assert (destination / "sentinel").read_bytes() == b"first"

    second = tmp_path / "second"
    second.mkdir()
    (second / "sentinel").write_bytes(b"second")
    with pytest.raises(MODULE.OwnerError, match="destination appeared"):
        MODULE._rename_no_replace(second, destination)
    assert (destination / "sentinel").read_bytes() == b"first"


def test_cli_rejects_public_profiles_all_and_ambiguous_modes(tmp_path: Path) -> None:
    tail = _valid_cli_tail(tmp_path)
    for profile in ("iroha3-taira", "iroha3-nexus", "all"):
        with pytest.raises(SystemExit):
            MODULE._parse_args(["--write", "--profile", profile, "--output-root", str(tmp_path / "out"), *tail])
    with pytest.raises(SystemExit):
        MODULE._parse_args(
            [
                "--write",
                "--check",
                "--profile",
                "iroha3-dev",
                "--output-root",
                str(tmp_path / "out"),
                *tail,
            ]
        )


def test_post_manifest_has_only_the_dev_profile_owner() -> None:
    if POST_MANIFEST.exists():
        manifest_text = POST_MANIFEST.read_text(encoding="utf-8")
    else:
        manifest_text = (REPO_ROOT / "generated-files.toml").read_text(encoding="utf-8")
        manifest_text += (CACHE_DIR / "generated-files.append.toml").read_text(encoding="utf-8")
    manifest = tomllib.loads(manifest_text)
    owners = {
        entry["name"]: entry
        for entry in manifest["generated"]
        if entry["name"] == "kagami-iroha3-dev-profile-bundle"
    }
    assert set(owners) == {"kagami-iroha3-dev-profile-bundle"}
    dev = set(owners["kagami-iroha3-dev-profile-bundle"]["outputs"])
    assert len(dev) == 15
    assert all("iroha3-taira" not in output and "iroha3-nexus" not in output for output in dev)
    for owner in owners.values():
        assert "Cargo.lock" in owner["inputs"]
        assert "kagami_profile_owner.py" in owner["generator"]
        assert "--write" in owner["generator"]
        assert "--check" in owner["check"]
        assert "--stage-a" in owner["check"] and "--stage-b" in owner["check"]
        assert "--cargo-lock-size 311202" in owner["generator"]
        assert "ad0d209abaa51d4c77a9e67ccbb0c7660a0f8b7b5dbe3e3fbe4a70e142711bf7" in owner["generator"]
        assert "--cargo-lock-size 311202" in owner["check"]
        assert "ad0d209abaa51d4c77a9e67ccbb0c7660a0f8b7b5dbe3e3fbe4a70e142711bf7" in owner["check"]
