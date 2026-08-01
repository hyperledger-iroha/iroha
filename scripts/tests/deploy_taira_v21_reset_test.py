"""Focused tests for the guarded Taira v21 fresh-reset controller."""

from __future__ import annotations

import argparse
import contextlib
import copy
import dataclasses
import grp
import hashlib
import importlib.util
import json
import os
import plistlib
import pwd
import stat
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "deploy_taira_v21_reset.py"
SPEC = importlib.util.spec_from_file_location("deploy_taira_v21_reset", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

GENESIS_PUBLIC_KEY = "ed0120" + "AB" * 32
GENESIS_EXPECTED_HASH = "00" * 31 + "01"


def _write(path: Path, body: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(body)
    path.chmod(0o600)


def _mkdir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.chmod(0o700)


def _tree_sha(root: Path) -> str:
    return MODULE.release_tree_sha256(root, os.getuid(), os.getgid())


def test_acl_gate_is_a_stable_noop_off_macos(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    path = tmp_path / "trusted"
    _write(path, b"trusted")
    expected = path.lstat()
    monkeypatch.setattr(MODULE.sys, "platform", "linux")
    monkeypatch.setattr(
        MODULE.subprocess,
        "run",
        lambda *_args, **_kwargs: pytest.fail("non-macOS ACL command ran"),
    )

    actual = MODULE.require_acl_free_path(path, "test trusted path")

    assert MODULE.metadata_identity(actual) == MODULE.metadata_identity(expected)


def test_acl_gate_fails_closed_when_the_pinned_inspector_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    path = tmp_path / "trusted"
    _write(path, b"trusted")
    monkeypatch.setattr(MODULE.sys, "platform", "darwin")
    monkeypatch.setattr(
        MODULE.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            args=[], returncode=1, stdout=b"", stderr=b"inspection failed"
        ),
    )

    with pytest.raises(MODULE.DeploymentError, match="extended ACL"):
        MODULE.require_acl_free_path(path, "test trusted path")


def test_acl_failure_removes_owned_unpublished_plist_staging_file(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    path = tmp_path / "io.soramitsu.taira.validator-1.plist"
    temporary = path.parent / f".{path.name}.{os.getpid()}.tmp"
    monkeypatch.setattr(MODULE.sys, "platform", "darwin")
    monkeypatch.setattr(
        MODULE,
        "_run_bounded_macos_acl_command",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            args=[], returncode=1, stdout=b"", stderr=b"clear failed"
        ),
    )

    with pytest.raises(MODULE.DeploymentError, match="clear inherited ACL"):
        MODULE.atomic_replace_owned(
            path,
            b"new plist",
            mode=0o600,
            uid=os.getuid(),
            gid=os.getgid(),
        )

    assert not path.exists()
    assert not temporary.exists()


@pytest.mark.skipif(sys.platform != "darwin", reason="macOS ACL semantics")
def test_acl_gate_rejects_everyone_write_and_clears_only_owned_temporary(
    tmp_path: Path,
) -> None:
    path = tmp_path / "owned-staging-file"
    _write(path, b"trusted")
    grant = subprocess.run(
        ["/bin/chmod", "+a", "everyone allow write", str(path)],
        check=False,
        capture_output=True,
    )
    assert grant.returncode == 0, grant.stderr.decode(errors="replace")
    try:
        with pytest.raises(MODULE.DeploymentError, match="extended ACL"):
            MODULE.require_acl_free_path(path, "owned staging fixture")
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        try:
            MODULE.clear_owned_temporary_acl(path, descriptor, "owned staging fixture")
        finally:
            os.close(descriptor)
        MODULE.require_acl_free_path(path, "owned staging fixture")
    finally:
        subprocess.run(
            ["/bin/chmod", "-N", str(path)],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )


def _build_bundle(tmp_path: Path, binary_sha: str, source_commit: str) -> Path:
    bundle = tmp_path / "bundle"
    _mkdir(bundle)
    for name, body in (
        ("base-config.toml", b"base\n"),
        ("genesis.json", b'{"chain":"taira"}\n'),
        ("genesis.signed.nrt", b"signed-genesis"),
        ("validator-roster.toml", b"roster\n"),
        ("validator-secrets.toml", b"secrets\n"),
    ):
        _write(bundle / name, body)

    attestation = b"NRT0-release-attestation"
    attestation_sha = hashlib.sha256(attestation).hexdigest()
    release_manifest = {
        "chain_id": MODULE.CHAIN_ID,
        "asset": MODULE.OFFLINE_ASSET_ID,
        "asset_scale": MODULE.OFFLINE_ASSET_SCALE,
        "activation_height": 2,
        "bridge_abi_version": MODULE.OFFLINE_BRIDGE_ABI,
        "generation": "production-gate-real-artifacts-v4",
        "withdrawal_height": MODULE.MINIMUM_RELEASE_WITHDRAWAL_HEIGHT,
        "max_proof_bytes": 131_072,
        "release_attestation_sha256": attestation_sha,
    }
    manifest_norito = b"NRT0-authenticated-release"
    release_sha = hashlib.sha256(manifest_norito).hexdigest()
    _mkdir(bundle / "kagemusha")
    _mkdir(bundle / "kagemusha" / "catalog")
    release = bundle / "kagemusha" / "catalog" / release_sha
    _mkdir(release)
    _write(release / "manifest.norito", manifest_norito)
    _write(release / "manifest.norito.sha256", f"{release_sha}\n".encode())
    _write(
        release / "manifest.json",
        (json.dumps(release_manifest, sort_keys=True) + "\n").encode(),
    )
    _write(release / MODULE.RELEASE_ATTESTATION_FILE_NAME, attestation)
    for name in sorted(
        MODULE.EXPECTED_RELEASE_FILE_NAMES
        - {
            "manifest.norito",
            "manifest.norito.sha256",
            "manifest.json",
            MODULE.RELEASE_ATTESTATION_FILE_NAME,
        }
    ):
        _write(release / name, f"authenticated-{name}".encode())
    _write(bundle / "kagemusha" / "release-policy-v1.norito", b"policy")
    tree_sha = _tree_sha(bundle / "kagemusha")
    installed_release = MODULE.INSTALL_ROOT / "releases" / tree_sha
    operator_identity = {
        "cash_handoff_capability": MODULE.OFFLINE_CAPABILITY,
        "required_bridge_abi_version": MODULE.OFFLINE_BRIDGE_ABI,
        "asset_definition_id": MODULE.OFFLINE_ASSET_ID,
        "asset_scale": MODULE.OFFLINE_ASSET_SCALE,
        "artifact_set": {
            "generation": "production-gate-real-artifacts-v4",
            "manifest_sha256": release_sha,
            "release_policy_sha256": hashlib.sha256(b"policy").hexdigest(),
            "release_attestation_sha256": attestation_sha,
            "activation_height": 2,
            "withdrawal_height": MODULE.MINIMUM_RELEASE_WITHDRAWAL_HEIGHT,
            "max_proof_bytes": 131_072,
            "asset_scale": MODULE.OFFLINE_ASSET_SCALE,
        },
    }
    _write(
        bundle / "operator-identity.json",
        (json.dumps(operator_identity, sort_keys=True) + "\n").encode(),
    )

    rendered = bundle / "rendered"
    _mkdir(rendered)
    _write(rendered / "genesis.json", (bundle / "genesis.json").read_bytes())
    config_hashes: dict[str, str] = {}
    for index, slug in enumerate(MODULE.SLUGS):
        workdir = rendered / slug
        _mkdir(workdir)
        for name in ("codec", "configs", "manifests", "runtime", "storage"):
            _mkdir(workdir / name)
        config = f"""chain = "{MODULE.CHAIN_ID}"
chain_discriminant = {MODULE.CHAIN_DISCRIMINANT}

[network]
address = "addr:127.0.0.1:{MODULE.P2P_PORTS[index]}#0000"

[torii]
address = "addr:127.0.0.1:{MODULE.TORII_PORTS[index]}#0000"

[torii.kagemusha_commands]
enabled = true

[nexus.storage]
local_budget_bytes = {MODULE.NODE_STORAGE_BUDGET_BYTES}

[nexus.storage.disk_budget_weights]
kura_blocks_bps = 7500
wsv_snapshots_bps = 2000
sorafs_bps = 0
soranet_spool_bps = 250
soravpn_spool_bps = 250

[settlement.offline]
enabled = true
escrow_required = true
kagemusha_release_policy_path = "{installed_release / 'release-policy-v1.norito'}"
kagemusha_artifact_dir = "{installed_release / 'catalog'}"
kagemusha_catalog_qualification_seal_path = "{MODULE.qualification_seal_path(tree_sha)}"

[genesis]
file = "{bundle / 'genesis.signed.nrt'}"
public_key = "{GENESIS_PUBLIC_KEY}"
expected_hash = "{GENESIS_EXPECTED_HASH}"
"""
        _write(workdir / "config.toml", config.encode())
        config_hashes[slug] = hashlib.sha256(config.encode()).hexdigest()

    manifest = {
        "schema": "taira-exact2f-reset-bundle",
        "peer_count": MODULE.PEER_COUNT,
        "chain_id": MODULE.CHAIN_ID,
        "chain_discriminant": MODULE.CHAIN_DISCRIMINANT,
        "node_storage_budget_bytes": MODULE.NODE_STORAGE_BUDGET_BYTES,
        "node_storage_budget_weights": MODULE.NODE_STORAGE_WEIGHTS,
        "nexus_storage_budget_policy": MODULE.NODE_STORAGE_BUDGET_POLICY,
        "offline_release_policy": "mandatory-authenticated-kagemusha-v4-activation-height-2",
        "offline_asset_definition_id": MODULE.OFFLINE_ASSET_ID,
        "offline_asset_scale": MODULE.OFFLINE_ASSET_SCALE,
        "source_commit": source_commit,
        "irohad_sha256": binary_sha,
        "genesis_public_key": GENESIS_PUBLIC_KEY,
        "genesis_expected_hash": GENESIS_EXPECTED_HASH,
        "signed_genesis_sha256": hashlib.sha256(
            (bundle / "genesis.signed.nrt").read_bytes()
        ).hexdigest(),
        "unsigned_genesis_sha256": hashlib.sha256(
            (bundle / "genesis.json").read_bytes()
        ).hexdigest(),
        "base_config_sha256": hashlib.sha256(
            (bundle / "base-config.toml").read_bytes()
        ).hexdigest(),
        "operator_identity_sha256": hashlib.sha256(
            (bundle / "operator-identity.json").read_bytes()
        ).hexdigest(),
        "kagemusha_manifest_sha256": release_sha,
        "kagemusha_release_attestation_sha256": attestation_sha,
        "kagemusha_release_policy_sha256": hashlib.sha256(b"policy").hexdigest(),
        "kagemusha_release_tree_sha256": tree_sha,
        "configs": config_hashes,
        "prewarmed_storage_sha256": {
            slug: MODULE.EMPTY_TREE_SHA256 for slug in MODULE.SLUGS
        },
    }
    _write(
        bundle / "reset-manifest.json",
        (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode(),
    )
    return bundle


def _validate(bundle: Path, binary_sha: str, source_commit: str) -> MODULE.BundlePlan:
    manifest_raw = (bundle / "reset-manifest.json").read_bytes()
    return MODULE.validate_bundle(
        bundle,
        expected_reset_manifest_sha256=hashlib.sha256(manifest_raw).hexdigest(),
        expected_binary_sha256=binary_sha,
        expected_source_commit=source_commit,
        minimum_free_bytes=0,
        maximum_fsync_latency_ms=10_000,
    )


def _projection_config_text() -> str:
    return f"""chain = "{MODULE.CHAIN_ID}"
chain_discriminant = {MODULE.CHAIN_DISCRIMINANT}
trusted_peers = [
  "peer-one",
]

[network]
address = "addr:127.0.0.1:1337#ABCD"

[torii]
address = "addr:127.0.0.1:8080#1234"

[torii.kagemusha_commands]
enabled = true

[nexus.storage]
local_budget_bytes = {MODULE.NODE_STORAGE_BUDGET_BYTES}

[nexus.storage.disk_budget_weights]
kura_blocks_bps = 7500
wsv_snapshots_bps = 2000
sorafs_bps = 0
soranet_spool_bps = 250
soravpn_spool_bps = 250

[settlement.offline]
enabled = true
escrow_required = true
kagemusha_release_policy_path = "/Library/SORA/Taira/releases/tree/release-policy-v1.norito"
kagemusha_artifact_dir = "/Library/SORA/Taira/releases/tree/catalog"
kagemusha_catalog_qualification_seal_path = "/Library/SORA/Taira/seals/kagemusha-v4-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.norito"

[genesis]
file = "/private/reset/genesis.signed.nrt"
public_key = "{GENESIS_PUBLIC_KEY}"
expected_hash = "{GENESIS_EXPECTED_HASH}"
"""


def test_projection_parser_extracts_all_required_fields() -> None:
    config = MODULE.parse_config_projection_text(
        _projection_config_text(),
        "validator config",
    )

    assert config["chain"] == MODULE.CHAIN_ID
    assert config["chain_discriminant"] == MODULE.CHAIN_DISCRIMINANT
    assert config["torii"]["kagemusha_commands"]["enabled"] is True
    assert config["genesis"]["public_key"] == GENESIS_PUBLIC_KEY
    assert config["genesis"]["expected_hash"] == GENESIS_EXPECTED_HASH
    assert (
        config["nexus"]["storage"]["disk_budget_weights"] == MODULE.NODE_STORAGE_WEIGHTS
    )


def test_projection_parser_rejects_malformed_required_field() -> None:
    malformed = _projection_config_text().replace(
        f"chain_discriminant = {MODULE.CHAIN_DISCRIMINANT}",
        "chain_discriminant = 01",
    )

    with pytest.raises(MODULE.DeploymentError, match="malformed integer"):
        MODULE.parse_config_projection_text(malformed, "validator config")


def test_projection_parser_rejects_duplicate_required_field() -> None:
    duplicate = _projection_config_text().replace(
        '[network]\naddress = "addr:127.0.0.1:1337#ABCD"',
        (
            '[network]\naddress = "addr:127.0.0.1:1337#ABCD"\n'
            'address = "addr:127.0.0.1:1337#DCBA"'
        ),
    )

    with pytest.raises(MODULE.DeploymentError, match="duplicates required field"):
        MODULE.parse_config_projection_text(duplicate, "validator config")


def test_projection_parser_keeps_hash_inside_quoted_address() -> None:
    config = MODULE.parse_config_projection_text(
        _projection_config_text(),
        "validator config",
    )

    assert config["network"]["address"] == "addr:127.0.0.1:1337#ABCD"
    assert config["torii"]["address"] == "addr:127.0.0.1:8080#1234"


def test_bundle_preflight_authenticates_exact_four_peer_reset(tmp_path: Path) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)

    plan = _validate(bundle, binary_sha, source_commit)

    assert (
        plan.manifest["nexus_storage_budget_policy"]
        == MODULE.NODE_STORAGE_BUDGET_POLICY
    )
    assert [peer.torii_port for peer in plan.peers] == list(MODULE.TORII_PORTS)
    assert [peer.p2p_port for peer in plan.peers] == list(MODULE.P2P_PORTS)
    assert all(not any(peer.storage.iterdir()) for peer in plan.peers)


def test_bundle_preflight_rejects_a_config_with_an_alternate_genesis_hash(
    tmp_path: Path,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    slug = MODULE.SLUGS[0]
    config_path = bundle / "rendered" / slug / "config.toml"
    alternate_hash = "02" * 31 + "03"
    config = config_path.read_text().replace(
        f'expected_hash = "{GENESIS_EXPECTED_HASH}"',
        f'expected_hash = "{alternate_hash}"',
    )
    _write(config_path, config.encode())
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["configs"][slug] = hashlib.sha256(config.encode()).hexdigest()
    _write(
        manifest_path,
        (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode(),
    )

    with pytest.raises(MODULE.DeploymentError, match="exact expected hash"):
        _validate(bundle, binary_sha, source_commit)


def test_bundle_preflight_requires_receipt_bound_reset_manifest_digest(
    tmp_path: Path,
) -> None:
    binary_sha = "8" * 64
    source_commit = "9" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)

    with pytest.raises(MODULE.DeploymentError, match="verified admission receipt"):
        MODULE.validate_bundle(
            bundle,
            expected_reset_manifest_sha256="0" * 64,
            expected_binary_sha256=binary_sha,
            expected_source_commit=source_commit,
            minimum_free_bytes=0,
            maximum_fsync_latency_ms=10_000,
        )


def test_bundle_preflight_requires_exact_release_file_names(tmp_path: Path) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    release = next((bundle / "kagemusha" / "catalog").iterdir())
    (release / "cryptographic-review.evidence").rename(
        release / "unreviewed-placeholder.evidence"
    )

    with pytest.raises(MODULE.DeploymentError, match="exact authenticated"):
        _validate(bundle, binary_sha, source_commit)


def test_release_cutover_moves_one_physical_copy_and_restores_on_rollback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    bundle_path = _build_bundle(tmp_path, "c" * 64, "d" * 40)
    bundle = _validate(bundle_path, "c" * 64, "d" * 40)
    destination = tmp_path / "root-store" / bundle.release.tree_sha256
    _mkdir(destination.parent)
    release = dataclasses.replace(bundle.release, installed_root=destination)
    bundle = dataclasses.replace(bundle, release=release)
    source_file = next(
        path
        for path in release.source_root.rglob("*")
        if path.is_file() and path.name.endswith(".proving-key.krv4")
    )
    original_inode = source_file.stat().st_ino
    relative = source_file.relative_to(release.source_root)
    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(
        MODULE, "rewrite_release_tree_ownership", lambda *args, **kwargs: None
    )
    monkeypatch.setattr(
        MODULE, "require_hardened_release_identity", lambda _bundle: None
    )

    MODULE.move_release_to_root_store(bundle)

    assert not release.source_root.exists()
    assert (destination / relative).stat().st_ino == original_inode
    assert list(tmp_path.rglob(source_file.name)) == [destination / relative]

    MODULE.restore_release_to_bundle(bundle)
    assert not destination.exists()
    assert (release.source_root / relative).stat().st_ino == original_inode


def test_release_cutover_recovers_signal_window_after_rename(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    bundle_path = _build_bundle(tmp_path, "e" * 64, "f" * 40)
    bundle = _validate(bundle_path, "e" * 64, "f" * 40)
    destination = tmp_path / "root-store" / bundle.release.tree_sha256
    _mkdir(destination.parent)
    release = dataclasses.replace(bundle.release, installed_root=destination)
    bundle = dataclasses.replace(bundle, release=release)
    calls = 0

    def interrupted_fsync(_path: Path) -> None:
        nonlocal calls
        calls += 1
        if calls == 1:
            raise MODULE.DeploymentError("injected post-rename interruption")

    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(
        MODULE, "rewrite_release_tree_ownership", lambda *args, **kwargs: None
    )
    monkeypatch.setattr(
        MODULE, "require_hardened_release_identity", lambda _bundle: None
    )
    monkeypatch.setattr(MODULE, "fsync_directory", interrupted_fsync)

    with pytest.raises(MODULE.DeploymentError, match="post-rename"):
        MODULE.move_release_to_root_store(bundle)

    assert release.source_root.is_dir()
    assert not destination.exists()


def test_release_cutover_rejects_root_swap_during_hardening(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    bundle_path = _build_bundle(tmp_path, "1" * 64, "2" * 40)
    bundle = _validate(bundle_path, "1" * 64, "2" * 40)
    destination = tmp_path / "root-store" / bundle.release.tree_sha256
    _mkdir(destination.parent)
    release = dataclasses.replace(bundle.release, installed_root=destination)
    bundle = dataclasses.replace(bundle, release=release)
    displaced = release.source_root.with_name("displaced-kagemusha")
    calls = 0

    def swap_once(root: Path, **_kwargs: object) -> None:
        nonlocal calls
        calls += 1
        if calls == 1:
            root.rename(displaced)
            _mkdir(root)

    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(MODULE, "rewrite_release_tree_ownership", swap_once)

    with pytest.raises(MODULE.DeploymentError, match="rollback is incomplete"):
        MODULE.move_release_to_root_store(bundle)

    assert displaced.is_dir()
    assert release.source_root.is_dir()
    assert not destination.exists()


def test_release_hardening_failure_restores_exact_private_source(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    bundle_path = _build_bundle(tmp_path, "7" * 64, "8" * 40)
    bundle = _validate(bundle_path, "7" * 64, "8" * 40)
    destination = tmp_path / "root-store" / bundle.release.tree_sha256
    _mkdir(destination.parent)
    bundle = dataclasses.replace(
        bundle,
        release=dataclasses.replace(
            bundle.release,
            installed_root=destination,
        ),
    )
    target = next(
        path for path in bundle.release.source_root.rglob("*") if path.is_file()
    )
    real_rewrite = MODULE.rewrite_release_tree_ownership
    calls = 0

    def fail_then_restore(root: Path, **kwargs: object) -> None:
        nonlocal calls
        calls += 1
        if calls == 1:
            target.chmod(0o400)
            raise MODULE.DeploymentError("injected partial hardening failure")
        real_rewrite(root, **kwargs)

    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(MODULE, "rewrite_release_tree_ownership", fail_then_restore)

    with pytest.raises(
        MODULE.DeploymentError, match="injected partial hardening failure"
    ):
        MODULE.move_release_to_root_store(bundle)

    assert calls == 2
    assert bundle.release.source_root.is_dir()
    assert not destination.exists()
    MODULE.require_restored_release_source_identity(bundle)


def test_release_hardening_cleanup_failure_is_explicitly_incomplete(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    bundle_path = _build_bundle(tmp_path, "9" * 64, "a" * 40)
    bundle = _validate(bundle_path, "9" * 64, "a" * 40)
    destination = tmp_path / "root-store" / bundle.release.tree_sha256
    _mkdir(destination.parent)
    bundle = dataclasses.replace(
        bundle,
        release=dataclasses.replace(
            bundle.release,
            installed_root=destination,
        ),
    )
    target = next(
        path for path in bundle.release.source_root.rglob("*") if path.is_file()
    )
    calls = 0

    def fail_hardening_and_cleanup(_root: Path, **_kwargs: object) -> None:
        nonlocal calls
        calls += 1
        if calls == 1:
            target.chmod(0o400)
            raise MODULE.DeploymentError("injected partial hardening failure")
        raise OSError("injected ownership cleanup failure")

    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(
        MODULE,
        "rewrite_release_tree_ownership",
        fail_hardening_and_cleanup,
    )

    with pytest.raises(
        MODULE.DeploymentError,
        match=r"release move failed.*rollback is incomplete",
    ):
        MODULE.move_release_to_root_store(bundle)

    assert calls == 2
    assert bundle.release.source_root.is_dir()
    assert not destination.exists()
    assert stat.S_IMODE(target.stat().st_mode) == 0o400


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        ("manifest", "reset manifest"),
        ("signed-genesis", "signed genesis"),
        ("config", "validator config"),
        ("runtime", "runtime path"),
        ("storage", "fresh-reset storage"),
    ],
)
def test_post_qualification_gate_rechecks_every_mutable_runtime_identity(
    tmp_path: Path, mutation: str, message: str
) -> None:
    bundle_path = _build_bundle(tmp_path, "3" * 64, "4" * 40)
    bundle = _validate(bundle_path, "3" * 64, "4" * 40)
    MODULE.require_mutable_bundle_identities(bundle, phase="during qualification")

    if mutation == "manifest":
        with (bundle.root / "reset-manifest.json").open("ab") as output:
            output.write(b"\n")
    elif mutation == "signed-genesis":
        with (bundle.root / "genesis.signed.nrt").open("ab") as output:
            output.write(b"mutation")
    elif mutation == "config":
        with bundle.peers[0].config.open("ab") as output:
            output.write(b"\n# mutation\n")
    elif mutation == "runtime":
        info = bundle.peers[0].workdir.stat()
        os.utime(
            bundle.peers[0].workdir,
            ns=(info.st_atime_ns, info.st_mtime_ns + 1),
        )
    else:
        _write(bundle.peers[0].storage / "unexpected", b"mutation")

    with pytest.raises(MODULE.DeploymentError, match=message):
        MODULE.require_mutable_bundle_identities(bundle, phase="during qualification")


def test_hardened_release_gate_binds_all_immutable_and_root_metadata(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    bundle_path = _build_bundle(tmp_path, "5" * 64, "6" * 40)
    bundle = _validate(bundle_path, "5" * 64, "6" * 40)
    destination = tmp_path / "root-store" / bundle.release.tree_sha256
    destination.parent.mkdir(parents=True)
    bundle.release.source_root.rename(destination)
    bundle = dataclasses.replace(
        bundle,
        release=dataclasses.replace(bundle.release, installed_root=destination),
    )
    target_file = next(path for path in destination.rglob("*") if path.is_file())
    real_lstat = Path.lstat
    mutation: dict[str, object] = {"field": None, "path": target_file}

    def hardened_lstat(path: Path) -> SimpleNamespace | os.stat_result:
        info = real_lstat(path)
        try:
            path.relative_to(destination)
        except ValueError:
            return info
        is_directory = MODULE.stat.S_ISDIR(info.st_mode)
        values = {
            "st_dev": info.st_dev,
            "st_ino": info.st_ino,
            "st_mode": (
                MODULE.stat.S_IFDIR | 0o550
                if is_directory
                else MODULE.stat.S_IFREG | 0o440
            ),
            "st_uid": 0,
            "st_gid": bundle.owner_gid,
            "st_nlink": info.st_nlink,
            "st_size": info.st_size,
            "st_mtime_ns": info.st_mtime_ns,
            "st_ctime_ns": info.st_ctime_ns,
        }
        if path == mutation["path"]:
            field = mutation["field"]
            if field == "type":
                values["st_mode"] = MODULE.stat.S_IFSOCK | 0o440
            elif field == "mode":
                values["st_mode"] = MODULE.stat.S_IFMT(values["st_mode"]) | 0o640
            elif field is not None:
                key = f"st_{field}"
                values[key] = int(values[key]) + 1
        return SimpleNamespace(**values)

    monkeypatch.setattr(Path, "lstat", hardened_lstat)
    MODULE.require_hardened_release_identity(bundle)

    for field in ("dev", "ino", "type", "nlink", "size", "mtime_ns"):
        mutation.update(field=field, path=target_file)
        with pytest.raises(MODULE.DeploymentError, match="identity changed"):
            MODULE.require_hardened_release_identity(bundle)
    for field in ("uid", "gid", "mode"):
        mutation.update(field=field, path=destination)
        with pytest.raises(MODULE.DeploymentError, match="ownership or mode changed"):
            MODULE.require_hardened_release_identity(bundle)


def test_qualification_seal_path_is_bound_to_release_tree_digest() -> None:
    digest = "a1" * 32

    assert MODULE.MAX_QUALIFICATION_SEAL_BYTES == 8 * 1024 * 1024
    assert MODULE.qualification_seal_path(digest) == Path(
        f"/Library/SORA/Taira/seals/kagemusha-v4-{digest}.norito"
    )
    with pytest.raises(MODULE.DeploymentError, match="SHA-256"):
        MODULE.qualification_seal_path("stale")


def test_qualification_seal_preflight_never_replaces_existing_path(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    install_root = tmp_path / "Taira"
    seals = install_root / "seals"
    seals.mkdir(parents=True)
    existing = seals / f"kagemusha-v4-{'ab' * 32}.norito"
    existing.write_bytes(b"preexisting")
    real_lstat = Path.lstat

    def root_directory_lstat(path: Path) -> object:
        if path == seals:
            return SimpleNamespace(
                st_mode=MODULE.stat.S_IFDIR | 0o755,
                st_uid=0,
                st_gid=0,
            )
        return real_lstat(path)

    monkeypatch.setattr(MODULE, "INSTALL_ROOT", install_root)
    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args: None)
    monkeypatch.setattr(Path, "lstat", root_directory_lstat)
    bundle = SimpleNamespace(
        release=SimpleNamespace(tree_sha256="ab" * 32),
    )

    with pytest.raises(MODULE.DeploymentError, match="preexisting"):
        MODULE.prepare_qualification_seal_path(bundle)

    assert existing.read_bytes() == b"preexisting"


def test_candidate_writes_qualification_seal_with_exact_full_check_command(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    binary = tmp_path / "irohad"
    seal = tmp_path / f"kagemusha-v4-{'ab' * 32}.norito"
    bundle = SimpleNamespace(
        release=SimpleNamespace(tree_sha256="ab" * 32),
        peers=tuple(
            SimpleNamespace(config=tmp_path / slug / "config.toml")
            for slug in MODULE.SLUGS
        ),
    )
    calls: list[tuple[list[str], dict[str, object]]] = []

    def runner(command: list[str], **kwargs: object) -> SimpleNamespace:
        calls.append((command, kwargs))
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setattr(
        MODULE,
        "qualification_seal_path",
        lambda _digest: seal,
    )
    monkeypatch.setattr(
        MODULE,
        "authenticate_qualification_seal",
        lambda path: (17,) if path == seal else (),
    )

    assert MODULE.write_catalog_qualification_seal(
        binary,
        bundle,
        seal,
        runner=runner,
    ) == (17,)
    assert calls == [
        (
            [
                str(binary),
                "--sora",
                "--config",
                str(bundle.peers[0].config),
                "--check-config",
                "--write-kagemusha-catalog-qualification-seal",
                str(seal),
            ],
            {
                "check": False,
                "stdin": MODULE.subprocess.DEVNULL,
                "capture_output": True,
                "timeout": MODULE.CATALOG_QUALIFICATION_TIMEOUT_SECONDS,
                "env": {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
            },
        )
    ]


def test_qualification_seal_requires_root_wheel_regular_0444(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    path = tmp_path / "seal.norito"
    identity = SimpleNamespace(
        st_dev=1,
        st_ino=2,
        st_mode=MODULE.stat.S_IFREG | 0o444,
        st_uid=0,
        st_gid=0,
        st_nlink=1,
        st_size=128,
        st_mtime_ns=3,
        st_ctime_ns=4,
    )
    monkeypatch.setattr(MODULE, "open_regular", lambda *_args: (73, identity))
    monkeypatch.setattr(MODULE.os, "fstat", lambda _descriptor: identity)
    monkeypatch.setattr(MODULE.os, "fsync", lambda _descriptor: None)
    monkeypatch.setattr(MODULE.os, "close", lambda _descriptor: None)
    monkeypatch.setattr(MODULE, "fsync_directory", lambda _path: None)

    assert MODULE.authenticate_qualification_seal(path) == (
        1,
        2,
        MODULE.stat.S_IFREG | 0o444,
        0,
        0,
        1,
        128,
        3,
        4,
    )

    unsafe = SimpleNamespace(**vars(identity))
    unsafe.st_mode = MODULE.stat.S_IFREG | 0o644
    monkeypatch.setattr(MODULE, "open_regular", lambda *_args: (73, unsafe))
    with pytest.raises(MODULE.DeploymentError, match="0444"):
        MODULE.authenticate_qualification_seal(path)


def test_binary_config_gate_checks_every_peer_with_bounded_redacted_command(
    tmp_path: Path,
) -> None:
    binary = tmp_path / "irohad"
    peers = tuple(
        SimpleNamespace(
            slug=slug,
            config=tmp_path / slug / "config.toml",
        )
        for slug in MODULE.SLUGS
    )
    calls: list[tuple[list[str], dict[str, object]]] = []

    def runner(command: list[str], **kwargs: object) -> SimpleNamespace:
        calls.append((command, kwargs))
        return SimpleNamespace(returncode=0, stdout=b"", stderr=b"")

    MODULE.validate_installed_peer_configs(
        binary,
        SimpleNamespace(peers=peers),
        runner=runner,
    )

    assert [command for command, _kwargs in calls] == [
        [
            str(binary),
            "--sora",
            "--config",
            str(peer.config),
            "--check-config",
        ]
        for peer in peers
    ]
    assert all(
        kwargs["stdin"] is MODULE.subprocess.DEVNULL
        and kwargs["capture_output"] is True
        and kwargs["timeout"] == MODULE.CONFIG_CHECK_TIMEOUT_SECONDS
        and kwargs["env"] == {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"}
        for _command, kwargs in calls
    )


def test_binary_config_gate_stops_on_first_rejected_peer(tmp_path: Path) -> None:
    peers = tuple(
        SimpleNamespace(
            slug=slug,
            config=tmp_path / slug / "config.toml",
        )
        for slug in MODULE.SLUGS
    )
    calls = 0

    def runner(_command: list[str], **_kwargs: object) -> SimpleNamespace:
        nonlocal calls
        calls += 1
        return SimpleNamespace(returncode=0 if calls == 1 else 78)

    with pytest.raises(
        MODULE.DeploymentError,
        match=f"peer={MODULE.SLUGS[1]}, status=78",
    ):
        MODULE.validate_installed_peer_configs(
            tmp_path / "irohad",
            SimpleNamespace(peers=peers),
            runner=runner,
        )

    assert calls == 2


def _apply_gate_harness(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> SimpleNamespace:
    install_root = tmp_path / "installed"
    launch_daemons = tmp_path / "launch-daemons"
    monkeypatch.setattr(MODULE, "INSTALL_ROOT", install_root)
    bundle, sources, binary_info = _fake_plan(tmp_path)
    old_cohort = tuple(
        SimpleNamespace(
            path=launch_daemons / f"{label}.plist",
            body=f"old-{label}".encode(),
            mode=0o600,
            uid=os.getuid(),
            gid=os.getgid(),
        )
        for label in MODULE.LABELS
    )
    for snapshot in old_cohort:
        _write(snapshot.path, snapshot.body)
    events: list[str] = []

    class GateOps:
        def bootout(self, _label: str) -> None:
            events.append("bootout")

    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setattr(MODULE, "LAUNCH_DAEMONS", launch_daemons)
    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(
        MODULE,
        "install_immutable",
        lambda *args, **kwargs: binary_info,
    )
    monkeypatch.setattr(MODULE, "require_exact_names", lambda *args, **kwargs: None)
    monkeypatch.setattr(MODULE.os, "chmod", lambda *args, **kwargs: None)
    monkeypatch.setattr(MODULE, "fsync_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(
        MODULE,
        "require_root_controlled_file",
        lambda *args, **kwargs: SimpleNamespace(),
    )
    monkeypatch.setattr(
        MODULE,
        "install_runtime_layout",
        lambda _bundle: tmp_path / "runtime",
    )
    monkeypatch.setattr(
        MODULE,
        "require_filesystem_headroom",
        lambda *args, **kwargs: (),
    )
    monkeypatch.setattr(
        MODULE,
        "render_plist",
        lambda peer, *args, **kwargs: peer.label.encode(),
    )
    monkeypatch.setattr(
        MODULE.plistlib,
        "loads",
        lambda body: {"Label": body.decode()},
    )

    initial_identity_checker = MODULE.require_bundle_runtime_unchanged

    def check_initial_identities(checked_bundle: MODULE.BundlePlan) -> None:
        events.append("runtime-recheck")
        initial_identity_checker(checked_bundle)

    monkeypatch.setattr(
        MODULE,
        "require_bundle_runtime_unchanged",
        check_initial_identities,
    )

    def move_release(checked_bundle: MODULE.BundlePlan) -> Path:
        events.append("release-move")
        source = checked_bundle.release.source_root
        destination = checked_bundle.release.installed_root
        destination.parent.mkdir(parents=True, exist_ok=True)
        source.rename(destination)
        return destination

    def restore_release(checked_bundle: MODULE.BundlePlan) -> None:
        events.append("release-restore")
        checked_bundle.release.installed_root.rename(checked_bundle.release.source_root)

    monkeypatch.setattr(MODULE, "move_release_to_root_store", move_release)
    monkeypatch.setattr(MODULE, "restore_release_to_bundle", restore_release)
    monkeypatch.setattr(MODULE, "verify_restored_snapshot", lambda *_args: None)

    seal_path = MODULE.qualification_seal_path(bundle.release.tree_sha256)
    seal_path.parent.mkdir(parents=True, exist_ok=True)
    monkeypatch.setattr(
        MODULE,
        "qualification_seal_path",
        lambda _release_tree_sha256: seal_path,
    )

    def prepare_seal(_bundle: object) -> Path:
        events.append("seal-prepare")
        return seal_path

    def authenticate_seal(path: Path) -> tuple[int, ...]:
        assert path == seal_path
        return MODULE.metadata_identity(path.lstat())

    def remove_seal(path: Path, identity: tuple[int, ...]) -> None:
        assert MODULE.metadata_identity(path.lstat()) == identity
        events.append("seal-remove")
        path.unlink()

    return SimpleNamespace(
        args=SimpleNamespace(
            minimum_free_bytes=0,
            restart_generation="9" * 64,
        ),
        bundle=bundle,
        sources=sources,
        old_cohort=old_cohort,
        ops=GateOps(),
        events=events,
        seal_path=seal_path,
        seal_preparer=prepare_seal,
        seal_authenticator=authenticate_seal,
        seal_remover=remove_seal,
    )


def test_apply_binary_gate_failure_restores_release_before_any_bootout(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    harness = _apply_gate_harness(tmp_path, monkeypatch)

    def reject_configs(_binary: Path, _bundle: object) -> None:
        harness.events.append("binary-config-gate")
        raise MODULE.DeploymentError("injected binary config rejection")

    def write_seal(_binary: Path, _bundle: object, path: Path) -> tuple[int, ...]:
        assert path == harness.seal_path
        harness.events.append("seal-write")
        path.write_bytes(b"created seal")
        return MODULE.metadata_identity(path.lstat())

    with pytest.raises(
        MODULE.DeploymentError, match="injected binary config rejection"
    ):
        MODULE.apply_reset(
            harness.args,
            harness.bundle,
            harness.sources,
            harness.old_cohort,
            rollout_starter=lambda: pytest.fail("rollout started"),
            ops=harness.ops,
            config_checker=reject_configs,
            seal_preparer=harness.seal_preparer,
            seal_writer=write_seal,
            seal_authenticator=harness.seal_authenticator,
            seal_remover=harness.seal_remover,
        )

    assert harness.events == [
        "runtime-recheck",
        "release-move",
        "seal-prepare",
        "seal-write",
        "binary-config-gate",
        "seal-remove",
        "release-restore",
    ]
    assert not harness.seal_path.exists()
    assert harness.bundle.release.source_root.is_dir()
    assert not harness.bundle.release.installed_root.exists()


def test_apply_preserves_unattributed_seal_and_installed_release_when_writer_raises(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    harness = _apply_gate_harness(tmp_path, monkeypatch)

    def write_then_raise(_binary: Path, _bundle: object, path: Path) -> tuple[int, ...]:
        harness.events.append("seal-write")
        path.write_bytes(b"unattributed seal")
        raise MODULE.DeploymentError("injected writer failure after publication")

    with pytest.raises(
        MODULE.DeploymentError,
        match=r"rollback is incomplete.*unattributed qualification seal",
    ):
        MODULE.apply_reset(
            harness.args,
            harness.bundle,
            harness.sources,
            harness.old_cohort,
            rollout_starter=lambda: pytest.fail("rollout started"),
            ops=harness.ops,
            config_checker=lambda *_args: pytest.fail("config checker ran"),
            seal_preparer=harness.seal_preparer,
            seal_writer=write_then_raise,
            seal_authenticator=harness.seal_authenticator,
            seal_remover=lambda *_args: pytest.fail("seal remover ran"),
        )

    assert harness.events == [
        "runtime-recheck",
        "release-move",
        "seal-prepare",
        "seal-write",
    ]
    assert harness.seal_path.read_bytes() == b"unattributed seal"
    assert harness.bundle.release.installed_root.is_dir()
    assert not harness.bundle.release.source_root.exists()


def test_apply_restores_release_when_failed_writer_left_seal_absent(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    harness = _apply_gate_harness(tmp_path, monkeypatch)

    def fail_without_seal(
        _binary: Path, _bundle: object, _path: Path
    ) -> tuple[int, ...]:
        harness.events.append("seal-write")
        raise MODULE.DeploymentError("injected writer failure before publication")

    with pytest.raises(
        MODULE.DeploymentError,
        match="injected writer failure before publication",
    ):
        MODULE.apply_reset(
            harness.args,
            harness.bundle,
            harness.sources,
            harness.old_cohort,
            rollout_starter=lambda: pytest.fail("rollout started"),
            ops=harness.ops,
            config_checker=lambda *_args: pytest.fail("config checker ran"),
            seal_preparer=harness.seal_preparer,
            seal_writer=fail_without_seal,
            seal_authenticator=harness.seal_authenticator,
            seal_remover=lambda *_args: pytest.fail("seal remover ran"),
        )

    assert harness.events == [
        "runtime-recheck",
        "release-move",
        "seal-prepare",
        "seal-write",
        "release-restore",
    ]
    assert not harness.seal_path.exists()
    assert harness.bundle.release.source_root.is_dir()
    assert not harness.bundle.release.installed_root.exists()


def test_apply_rechecks_config_identity_after_qualification_before_any_bootout(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    harness = _apply_gate_harness(tmp_path, monkeypatch)

    def write_seal(_binary: Path, _bundle: object, path: Path) -> tuple[int, ...]:
        harness.events.append("seal-write")
        path.write_bytes(b"created seal")
        return MODULE.metadata_identity(path.lstat())

    def mutate_config(_binary: Path, bundle: MODULE.BundlePlan) -> None:
        harness.events.append("binary-config-gate")
        with bundle.peers[0].config.open("ab") as config:
            config.write(b"\n# post-qualification mutation\n")

    with pytest.raises(
        MODULE.DeploymentError,
        match="validator config changed during qualification",
    ):
        MODULE.apply_reset(
            harness.args,
            harness.bundle,
            harness.sources,
            harness.old_cohort,
            rollout_starter=lambda: pytest.fail("rollout started"),
            ops=harness.ops,
            config_checker=mutate_config,
            seal_preparer=harness.seal_preparer,
            seal_writer=write_seal,
            seal_authenticator=harness.seal_authenticator,
            seal_remover=harness.seal_remover,
        )

    assert harness.events == [
        "runtime-recheck",
        "release-move",
        "seal-prepare",
        "seal-write",
        "binary-config-gate",
        "seal-remove",
        "release-restore",
    ]
    assert not harness.seal_path.exists()
    assert harness.bundle.release.source_root.is_dir()
    assert not harness.bundle.release.installed_root.exists()


def test_apply_marks_admitted_rollout_immediately_before_first_bootout(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    harness = _apply_gate_harness(tmp_path, monkeypatch)

    def write_seal(_binary: Path, _bundle: object, path: Path) -> tuple[int, ...]:
        harness.events.append("seal-write")
        path.write_bytes(b"created seal")
        return MODULE.metadata_identity(path.lstat())

    def start_rollout() -> None:
        harness.events.append("rollout-start")
        raise MODULE.DeploymentError("injected refusal at rollout boundary")

    with pytest.raises(MODULE.DeploymentError, match="rollout boundary"):
        MODULE.apply_reset(
            harness.args,
            harness.bundle,
            harness.sources,
            harness.old_cohort,
            rollout_starter=start_rollout,
            ops=harness.ops,
            config_checker=lambda *_args: harness.events.append("binary-config-gate"),
            cutover_identity_checker=lambda _bundle: harness.events.append(
                "cutover-recheck"
            ),
            seal_preparer=harness.seal_preparer,
            seal_writer=write_seal,
            seal_authenticator=harness.seal_authenticator,
            seal_remover=harness.seal_remover,
        )

    assert "rollout-start" in harness.events
    assert "bootout" not in harness.events
    assert harness.events.index("cutover-recheck") + 1 == harness.events.index(
        "rollout-start"
    )
    assert harness.bundle.release.source_root.is_dir()
    assert not harness.bundle.release.installed_root.exists()


@pytest.mark.parametrize(
    "mutation",
    ["source", "budget", "port", "seal", "storage"],
)
def test_bundle_preflight_rejects_identity_and_freshness_drift(
    tmp_path: Path, mutation: str
) -> None:
    binary_sha = "c" * 64
    source_commit = "d" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    if mutation == "source":
        manifest["source_commit"] = "e" * 40
        _write(manifest_path, (json.dumps(manifest) + "\n").encode())
    elif mutation == "budget":
        manifest["nexus_storage_budget_policy"] = "unbounded"
        _write(manifest_path, (json.dumps(manifest) + "\n").encode())
    elif mutation == "port":
        config = bundle / "rendered" / MODULE.SLUGS[0] / "config.toml"
        _write(config, config.read_bytes().replace(b":29080#", b":19080#"))
        manifest["configs"][MODULE.SLUGS[0]] = hashlib.sha256(
            config.read_bytes()
        ).hexdigest()
        _write(manifest_path, (json.dumps(manifest) + "\n").encode())
    elif mutation == "seal":
        config = bundle / "rendered" / MODULE.SLUGS[0] / "config.toml"
        expected = (
            "kagemusha-v4-" f"{manifest['kagemusha_release_tree_sha256']}.norito"
        ).encode()
        stale = f"kagemusha-v4-{'f' * 64}.norito".encode()
        _write(config, config.read_bytes().replace(expected, stale))
        manifest["configs"][MODULE.SLUGS[0]] = hashlib.sha256(
            config.read_bytes()
        ).hexdigest()
        _write(manifest_path, (json.dumps(manifest) + "\n").encode())
    else:
        _write(bundle / "rendered" / MODULE.SLUGS[0] / "storage" / "stale", b"block")

    with pytest.raises(MODULE.DeploymentError):
        _validate(bundle, binary_sha, source_commit)


def _fake_plan(
    tmp_path: Path,
) -> tuple[MODULE.BundlePlan, MODULE.SourcePlan, os.stat_result]:
    binary_sha = "1" * 64
    source_commit = "2" * 40
    root = _build_bundle(tmp_path, binary_sha, source_commit)
    bundle = _validate(root, binary_sha, source_commit)
    binary = tmp_path / "irohad"
    supervisor = tmp_path / "supervisor.py"
    _write(binary, b"binary")
    binary.chmod(0o555)
    _write(supervisor, b"supervisor")
    sources = MODULE.SourcePlan(
        binary=binary,
        binary_sha256=binary_sha,
        supervisor=supervisor,
        supervisor_sha256="3" * 64,
        python=Path("/usr/bin/python3"),
        python_identity=(0,) * 9,
    )
    return bundle, sources, binary.lstat()


def test_fresh_plist_has_all_five_binary_stat_seals_and_known_paths(
    tmp_path: Path,
) -> None:
    bundle, sources, binary_info = _fake_plan(tmp_path)
    runtime = tmp_path / "runtime"
    installed_binary = Path(
        f"/Library/SORA/Taira/binaries/{sources.binary_sha256}/irohad"
    )
    installed_supervisor = Path(
        f"/Library/SORA/Taira/supervisors/{sources.supervisor_sha256}/taira_peer_supervisor.py"
    )

    body = MODULE.render_plist(
        bundle.peers[0],
        bundle,
        sources,
        installed_binary=installed_binary,
        binary_info=binary_info,
        installed_supervisor=installed_supervisor,
        runtime_root=runtime,
        restart_generation="4" * 64,
    )
    payload = plistlib.loads(body)
    arguments = payload["ProgramArguments"]

    assert payload["Label"] == MODULE.LABELS[0]
    assert payload["UserName"] == bundle.runtime_user
    assert arguments[:4] == [
        str(sources.python),
        "-I",
        "-S",
        str(installed_supervisor),
    ]
    for field in (
        "--binary-device",
        "--binary-inode",
        "--binary-size",
        "--binary-mtime-ns",
        "--binary-ctime-ns",
    ):
        assert arguments.count(field) == 1
    assert arguments[arguments.index("--config") + 1] == str(bundle.peers[0].config)
    assert arguments[arguments.index("--restart-generation") + 1] == "4" * 64
    terminal_binding = MODULE.supervisor_terminal_binding(
        sources.binary_sha256,
        binary_info,
        bundle.peers[0].config_sha256,
        "4" * 64,
    )
    assert arguments[arguments.index("--terminal-unhealthy-file") + 1] == str(
        runtime / "terminal" / f"validator-1-{terminal_binding}-terminal-unhealthy.json"
    )
    assert payload["EnvironmentVariables"]["GENESIS"] == str(
        bundle.root / "genesis.signed.nrt"
    )


def test_validate_sources_uses_validated_runtime_not_controller_python(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    binary = tmp_path / "irohad"
    supervisor = tmp_path / "supervisor.py"
    _write(binary, b"binary")
    binary.chmod(0o555)
    _write(supervisor, b"supervisor")
    binary_sha = hashlib.sha256(b"binary").hexdigest()
    supervisor_sha = hashlib.sha256(b"supervisor").hexdigest()
    bundle = SimpleNamespace(owner_uid=os.getuid(), owner_gid=os.getgid())
    args = SimpleNamespace(
        binary=binary,
        supervisor=supervisor,
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
    )
    admission = SimpleNamespace(
        binary_sha256=binary_sha,
        supervisor_sha256=supervisor_sha,
    )
    monkeypatch.setattr(
        MODULE.sys,
        "executable",
        "/opt/homebrew/Cellar/python@3.14/3.14.4/bin/python3.14",
    )
    monkeypatch.setattr(
        MODULE, "require_root_controlled_file", lambda *args, **kwargs: None
    )
    monkeypatch.setattr(
        MODULE,
        "validate_supervisor_python",
        lambda path: (Path("/System/Python.app/Contents/MacOS/Python"), (7,) * 9),
    )

    sources = MODULE.validate_sources(args, bundle, admission)

    assert sources.python == Path("/System/Python.app/Contents/MacOS/Python")
    assert sources.python_identity == (7,) * 9
    assert str(sources.python) != MODULE.sys.executable


@pytest.mark.parametrize(
    ("returncode", "stdout"),
    [
        (1, ""),
        (0, f"3.8.19\n{os.fsencode('/System/Python').hex()}\n"),
        (0, f"not-a-version\n{os.fsencode('/System/Python').hex()}\n"),
        (0, f"4.0.0\n{os.fsencode('/System/Python').hex()}\n"),
    ],
)
def test_supervisor_python_probe_fails_closed(
    monkeypatch: pytest.MonkeyPatch, returncode: int, stdout: str
) -> None:
    monkeypatch.setattr(MODULE, "canonical_path", lambda path, _label: path)
    monkeypatch.setattr(
        MODULE, "require_system_python_launcher", lambda _path: SimpleNamespace()
    )
    monkeypatch.setattr(
        MODULE, "require_root_controlled_file", lambda *args, **kwargs: None
    )
    monkeypatch.setattr(
        MODULE.subprocess,
        "run",
        lambda *args, **kwargs: SimpleNamespace(returncode=returncode, stdout=stdout),
    )

    with pytest.raises(MODULE.DeploymentError):
        MODULE.validate_supervisor_python(MODULE.DEFAULT_SUPERVISOR_PYTHON)


def test_supervisor_python_accepts_root_controlled_python_39(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    base_prefix = Path("/System/Python3.framework/Versions/3.9")
    runtime = base_prefix / "Resources/Python.app/Contents/MacOS/Python"
    identity = SimpleNamespace(
        st_dev=1,
        st_ino=2,
        st_mode=stat.S_IFREG | 0o555,
        st_uid=0,
        st_gid=0,
        st_nlink=1,
        st_size=3,
        st_mtime_ns=4,
        st_ctime_ns=5,
    )
    monkeypatch.setattr(MODULE, "canonical_path", lambda path, _label: path)
    monkeypatch.setattr(
        MODULE, "require_system_python_launcher", lambda _path: identity
    )
    monkeypatch.setattr(
        MODULE, "require_root_controlled_file", lambda *args, **kwargs: identity
    )
    probes = iter(
        (
            SimpleNamespace(
                returncode=0,
                stdout=f"3.9.6\n{os.fsencode(base_prefix).hex()}\n",
            ),
            SimpleNamespace(
                returncode=0,
                stdout=f"3.9.6\n{os.fsencode(runtime).hex()}\n",
            ),
        )
    )
    monkeypatch.setattr(MODULE.subprocess, "run", lambda *args, **kwargs: next(probes))

    assert MODULE.validate_supervisor_python(MODULE.DEFAULT_SUPERVISOR_PYTHON) == (
        runtime,
        MODULE.metadata_identity(identity),
    )


def test_supervisor_python_rejects_runtime_identity_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    base_prefix = Path("/System/Python3.framework/Versions/3.9")
    runtime = base_prefix / "Resources/Python.app/Contents/MacOS/Python"
    stable = SimpleNamespace(
        st_dev=1,
        st_ino=2,
        st_mode=stat.S_IFREG | 0o555,
        st_uid=0,
        st_gid=0,
        st_nlink=1,
        st_size=3,
        st_mtime_ns=4,
        st_ctime_ns=5,
    )
    changed = copy.copy(stable)
    changed.st_ino = 9
    monkeypatch.setattr(MODULE, "canonical_path", lambda path, _label: path)
    monkeypatch.setattr(
        MODULE, "require_system_python_launcher", lambda _path: stable
    )
    identities = iter((stable, changed))
    monkeypatch.setattr(
        MODULE,
        "require_root_controlled_file",
        lambda *args, **kwargs: next(identities),
    )
    probes = iter(
        (
            SimpleNamespace(
                returncode=0,
                stdout=f"3.9.6\n{os.fsencode(base_prefix).hex()}\n",
            ),
            SimpleNamespace(
                returncode=0,
                stdout=f"3.9.6\n{os.fsencode(runtime).hex()}\n",
            ),
        )
    )
    monkeypatch.setattr(MODULE.subprocess, "run", lambda *args, **kwargs: next(probes))

    with pytest.raises(MODULE.DeploymentError, match="identity changed"):
        MODULE.validate_supervisor_python(MODULE.DEFAULT_SUPERVISOR_PYTHON)


@pytest.mark.skipif(sys.platform != "darwin", reason="macOS deployment invariant")
def test_supervisor_python_live_probe_resolves_direct_clt_runtime() -> None:
    runtime, identity = MODULE.validate_supervisor_python(
        MODULE.DEFAULT_SUPERVISOR_PYTHON
    )

    assert str(runtime).startswith(f"{MODULE.SYSTEM_PYTHON_DEVELOPER_DIR}/")
    assert str(runtime).endswith("/Resources/Python.app/Contents/MacOS/Python")
    assert MODULE.metadata_identity(
        MODULE.require_root_controlled_file(runtime, executable=True)
    ) == identity


def test_supervisor_python_rejects_homebrew_path(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    homebrew = Path(
        "/opt/homebrew/Cellar/python@3.14/3.14.4/Frameworks/"
        "Python.framework/Versions/3.14/bin/python3.14"
    )
    monkeypatch.setattr(MODULE, "canonical_path", lambda path, _label: path)

    with pytest.raises(MODULE.DeploymentError, match="exactly /usr/bin/python3"):
        MODULE.validate_supervisor_python(homebrew)


def _health_getter(
    bundle: MODULE.BundlePlan, source_commit: str, *, bad_blocks: bool = False
):
    block_hash = "ab" * 32
    artifact = {
        "generation": bundle.release.generation,
        "manifest_sha256": bundle.release.manifest_sha256,
        "release_policy_sha256": bundle.release.release_policy_sha256,
        "release_attestation_sha256": bundle.release.release_attestation_sha256,
        "activation_height": bundle.release.activation_height,
        "withdrawal_height": bundle.release.withdrawal_height,
        "max_proof_bytes": bundle.release.max_proof_bytes,
        "asset_scale": bundle.release.asset_scale,
    }

    def get(url: str, _timeout: float) -> dict:
        port = int(url.split(":")[2].split("/")[0])
        index = MODULE.TORII_PORTS.index(port)
        if url.endswith("/readyz"):
            return {
                "live": True,
                "mandatory": True,
                "ready": True,
                "cash_handoff_capability": MODULE.OFFLINE_CAPABILITY,
                "required_bridge_abi_version": MODULE.OFFLINE_BRIDGE_ABI,
                "blockers": [],
            }
        if "/v1/sumeragi/status" in url:
            subject = {"block_hash": f"hash:{block_hash.upper()}#A1b2"}
            return {
                "protocol_version": 3,
                "restart_required": False,
                "height": 8,
                "last_committed_height": 7,
                "last_committed_subject": subject,
                "last_commit_qc": {
                    "certificate": {
                        "round": {"height": 7, "view": 1},
                        "phase": {"phase": "commit", "details": None},
                        "subject": subject,
                    },
                    "validator_count": 4,
                    "signer_count": 3,
                    "min_signers": 3,
                    "signed_power": 3,
                    "total_power": 4,
                },
                "height_context_id": {"height": 8, "epoch": 1},
                "height_context": {
                    "mode": {"mode": "permissioned", "details": None},
                    "validator_count": 4,
                    "quorum": {"min_signers": 3, "total_power": 4},
                },
                "node_fingerprint": {"peer": index + 1},
                "build_fingerprint": {"commit": source_commit},
                "config_fingerprint": {"chain": MODULE.CHAIN_ID},
            }
        if url.endswith("/status"):
            return {
                "blocks": 6 if bad_blocks and index == 0 else 7,
                "build": {"git_commit_sha": source_commit},
            }
        assert "/v1/offline/readiness?" in url
        return {
            "ready": True,
            "blockers": [],
            "cash_handoff_capability": MODULE.OFFLINE_CAPABILITY,
            "required_bridge_abi_version": MODULE.OFFLINE_BRIDGE_ABI,
            "asset_definition_id": MODULE.OFFLINE_ASSET_ID,
            "asset_scale": MODULE.OFFLINE_ASSET_SCALE,
            "evaluated_block_height": 7,
            "evaluated_block_hash": f"{block_hash}#a1B2",
            "artifact_set": artifact,
        }

    return get


@pytest.mark.parametrize(
    "value",
    [
        "ab" * 32,
        "AB" * 32,
        "hash:" + "aB" * 32,
        "ab" * 32 + "#0fA9",
        "hash:" + "AB" * 32 + "#aB01",
    ],
)
def test_block_hash_normalization_accepts_exact_canonical_forms(value: str) -> None:
    assert MODULE.normalized_block_hash(value, "test block") == "ab" * 32


@pytest.mark.parametrize(
    "value",
    [
        "hash:" + "ab" * 32 + "#123",
        "hash:" + "ab" * 32 + "#12345",
        "hash:" + "ab" * 32 + "#12xz",
        "hash:" + "ab" * 32 + "#1234trailing",
        "HASH:" + "ab" * 32 + "#1234",
        "hash:" + "ab" * 32 + "#1234\n",
    ],
)
def test_block_hash_normalization_rejects_noncanonical_suffixes(value: str) -> None:
    with pytest.raises(MODULE.DeploymentError, match="canonical block hash"):
        MODULE.normalized_block_hash(value, "test block")


def test_four_peer_health_requires_exact_common_status_and_offline_block(
    tmp_path: Path,
) -> None:
    source_commit = "4" * 40
    bundle = _build_bundle(tmp_path, "5" * 64, source_commit)
    plan = _validate(bundle, "5" * 64, source_commit)

    health_urls: list[str] = []
    sample = MODULE.capture_fleet(
        plan,
        source_commit,
        getter=_health_getter(plan, source_commit),
        health_getter=lambda url, _timeout: health_urls.append(url),
    )
    assert sample.height == 7
    assert sample.block_hash == "ab" * 32
    assert len(sample.nodes) == MODULE.PEER_COUNT
    assert health_urls == [
        f"http://127.0.0.1:{port}/health" for port in MODULE.TORII_PORTS
    ]

    with pytest.raises(MODULE.DeploymentError, match="status.blocks"):
        MODULE.capture_fleet(
            plan,
            source_commit,
            getter=_health_getter(plan, source_commit, bad_blocks=True),
            health_getter=lambda _url, _timeout: None,
        )


def test_four_peer_health_fails_closed_when_health_is_not_200(tmp_path: Path) -> None:
    source_commit = "4" * 40
    bundle = _build_bundle(tmp_path, "5" * 64, source_commit)
    plan = _validate(bundle, "5" * 64, source_commit)

    def unhealthy(_url: str, _timeout: float) -> None:
        raise MODULE.DeploymentError("HTTP 503")

    with pytest.raises(MODULE.DeploymentError, match="HTTP 503"):
        MODULE.capture_fleet(
            plan,
            source_commit,
            getter=_health_getter(plan, source_commit),
            health_getter=unhealthy,
        )


def test_four_peer_health_requires_full_pinned_offline_release_identity(
    tmp_path: Path,
) -> None:
    source_commit = "4" * 40
    bundle = _build_bundle(tmp_path, "5" * 64, source_commit)
    plan = _validate(bundle, "5" * 64, source_commit)
    healthy = _health_getter(plan, source_commit)

    def wrong_attestation(url: str, timeout: float) -> dict:
        payload = copy.deepcopy(healthy(url, timeout))
        if "/v1/offline/readiness?" in url:
            payload["artifact_set"]["release_attestation_sha256"] = "0" * 64
        return payload

    with pytest.raises(MODULE.DeploymentError, match="offline release"):
        MODULE.capture_fleet(
            plan,
            source_commit,
            getter=wrong_attestation,
            health_getter=lambda _url, _timeout: None,
        )


@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("height_context", "validator_count"), 1),
        (("last_commit_qc", "signer_count"), 2),
        (("last_commit_qc", "signed_power"), 2),
        (("last_commit_qc", "certificate", "phase", "phase"), "prepare"),
    ],
)
def test_four_peer_health_rejects_underquorum_or_noncommit_qc(
    tmp_path: Path, path: tuple[str, ...], value: object
) -> None:
    source_commit = "6" * 40
    bundle = _build_bundle(tmp_path, "7" * 64, source_commit)
    plan = _validate(bundle, "7" * 64, source_commit)
    healthy = _health_getter(plan, source_commit)

    def getter(url: str, timeout: float) -> dict:
        payload = copy.deepcopy(healthy(url, timeout))
        if "/v1/sumeragi/status" in url:
            current = payload
            for key in path[:-1]:
                current = current[key]
            current[path[-1]] = value
        return payload

    with pytest.raises(MODULE.DeploymentError):
        MODULE.capture_fleet(
            plan,
            source_commit,
            getter=getter,
            health_getter=lambda _url, _timeout: None,
        )


def test_controller_terminal_marker_is_private_bounded_and_redaction_safe(
    tmp_path: Path,
) -> None:
    source_commit = "6" * 40
    bundle_root = _build_bundle(tmp_path, "7" * 64, source_commit)
    plan = _validate(bundle_root, "7" * 64, source_commit)
    runtime_root = tmp_path / "runtime"
    binding = "8" * 64
    marker = MODULE.terminal_unhealthy_path(runtime_root, plan.peers[0], binding)
    fatal = "9" * 64
    body = (
        json.dumps(
            {
                "binding_sha256": binding,
                "fatal_fingerprint_sha256": fatal,
                "hit_count": 3,
                "schema": MODULE.TERMINAL_UNHEALTHY_SCHEMA,
            },
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")
    _write(marker, body)

    with pytest.raises(
        MODULE.DeploymentError,
        match=f"{MODULE.LABELS[0]} entered terminal-unhealthy state",
    ) as caught:
        MODULE.require_no_terminal_unhealthy(
            plan,
            runtime_root,
            {peer.label: binding for peer in plan.peers},
        )

    message = str(caught.value)
    assert binding not in message
    assert fatal not in message
    assert str(marker) not in message
    assert stat.S_IMODE(marker.stat().st_mode) == 0o600
    assert marker.stat().st_size <= MODULE.MAX_TERMINAL_UNHEALTHY_BYTES


def test_new_binding_ignores_stale_marker_but_rejects_misbinding(
    tmp_path: Path,
) -> None:
    source_commit = "6" * 40
    bundle_root = _build_bundle(tmp_path, "7" * 64, source_commit)
    plan = _validate(bundle_root, "7" * 64, source_commit)
    runtime_root = tmp_path / "runtime"
    stale_binding = "8" * 64
    current_binding = "9" * 64
    stale_marker = MODULE.terminal_unhealthy_path(
        runtime_root, plan.peers[0], stale_binding
    )
    stale_body = (
        json.dumps(
            {
                "binding_sha256": stale_binding,
                "fatal_fingerprint_sha256": "a" * 64,
                "hit_count": 3,
                "schema": MODULE.TERMINAL_UNHEALTHY_SCHEMA,
            },
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")
    _write(stale_marker, stale_body)
    bindings = {peer.label: current_binding for peer in plan.peers}

    MODULE.require_no_terminal_unhealthy(plan, runtime_root, bindings)

    current_marker = MODULE.terminal_unhealthy_path(
        runtime_root, plan.peers[0], current_binding
    )
    _write(current_marker, stale_body)
    with pytest.raises(
        MODULE.DeploymentError,
        match="terminal-unhealthy marker is unsafe",
    ):
        MODULE.require_no_terminal_unhealthy(plan, runtime_root, bindings)


def test_controller_fails_before_initial_health_when_terminal_latched() -> None:
    calls: list[str] = []

    def terminal_checker() -> None:
        calls.append("terminal")
        raise MODULE.DeploymentError("terminal-unhealthy")

    with pytest.raises(MODULE.DeploymentError, match="terminal-unhealthy"):
        MODULE.wait_for_fleet_sample(
            SimpleNamespace(),
            "1" * 40,
            MODULE.time.monotonic() + 10,
            getter=lambda *_args: pytest.fail("health getter ran"),
            health_getter=lambda *_args: pytest.fail("health endpoint ran"),
            terminal_checker=terminal_checker,
        )

    assert calls == ["terminal"]


def test_controller_fails_before_advancement_when_terminal_latched() -> None:
    calls: list[str] = []

    def terminal_checker() -> None:
        calls.append("terminal")
        raise MODULE.DeploymentError("terminal-unhealthy")

    with pytest.raises(MODULE.DeploymentError, match="terminal-unhealthy"):
        MODULE.wait_for_advancement(
            SimpleNamespace(),
            "1" * 40,
            SimpleNamespace(),
            MODULE.time.monotonic() + 10,
            getter=lambda *_args: pytest.fail("health getter ran"),
            health_getter=lambda *_args: pytest.fail("health endpoint ran"),
            terminal_checker=terminal_checker,
        )

    assert calls == ["terminal"]


def test_restart_log_gate_accepts_snapshot_restore_and_ignores_stale_prefix(
    tmp_path: Path,
) -> None:
    log = tmp_path / "validator-1-supervisor.log"
    stale = b"\n".join(
        (
            MODULE.SNAPSHOT_LOAD_SUCCESS_MARKER,
            *MODULE.SNAPSHOT_LOAD_FALLBACK_MARKERS,
        )
    )
    _write(log, stale + b"\n")
    cursor = MODULE.bind_restart_log_cursor(log, os.getuid(), os.getgid())

    with log.open("ab") as stream:
        stream.write(MODULE.SNAPSHOT_LOAD_SUCCESS_MARKER + b"\n")

    MODULE.require_snapshot_backed_restart(cursor)


@pytest.mark.parametrize(
    ("suffix", "message"),
    [
        pytest.param(b"unrelated restart output\n", "exactly one", id="missing"),
        *[
            pytest.param(
                MODULE.SNAPSHOT_LOAD_SUCCESS_MARKER + b"\n" + marker + b"\n",
                "fallback",
                id=f"forbidden-{index}",
            )
            for index, marker in enumerate(MODULE.SNAPSHOT_LOAD_FALLBACK_MARKERS)
        ],
    ],
)
def test_restart_log_gate_rejects_missing_or_forbidden_marker(
    tmp_path: Path, suffix: bytes, message: str
) -> None:
    log = tmp_path / "validator-1-supervisor.log"
    _write(log, b"historical output\n")
    cursor = MODULE.bind_restart_log_cursor(log, os.getuid(), os.getgid())
    with log.open("ab") as stream:
        stream.write(suffix)

    with pytest.raises(MODULE.DeploymentError, match=message):
        MODULE.require_snapshot_backed_restart(cursor)


@pytest.mark.parametrize("mutation", ["truncate", "replace"])
def test_restart_log_gate_rejects_truncated_or_replaced_inode(
    tmp_path: Path, mutation: str
) -> None:
    log = tmp_path / "validator-1-supervisor.log"
    _write(log, b"historical output that must remain bound\n")
    cursor = MODULE.bind_restart_log_cursor(log, os.getuid(), os.getgid())

    if mutation == "truncate":
        log.write_bytes(b"")
    else:
        replacement = tmp_path / "replacement.log"
        _write(replacement, MODULE.SNAPSHOT_LOAD_SUCCESS_MARKER + b"\n")
        os.replace(replacement, log)

    with pytest.raises(MODULE.DeploymentError, match="truncated|replaced|changed"):
        MODULE.require_snapshot_backed_restart(cursor)


def test_restart_log_cursor_rejects_symlink_wrong_mode_owner_and_link_count(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target.log"
    _write(target, b"historical output\n")
    symlink = tmp_path / "symlink.log"
    symlink.symlink_to(target)
    with pytest.raises(MODULE.DeploymentError, match="regular file"):
        MODULE.bind_restart_log_cursor(symlink, os.getuid(), os.getgid())

    target.chmod(0o666)
    with pytest.raises(MODULE.DeploymentError, match="owner or mode"):
        MODULE.bind_restart_log_cursor(target, os.getuid(), os.getgid())
    target.chmod(0o600)

    alias = tmp_path / "alias.log"
    os.link(target, alias)
    with pytest.raises(MODULE.DeploymentError, match="exactly one link"):
        MODULE.bind_restart_log_cursor(target, os.getuid(), os.getgid())

    info = target.lstat()
    wrong_owner = SimpleNamespace(
        st_uid=max(os.getuid(), 0) + 10_000,
        st_gid=info.st_gid,
        st_mode=info.st_mode,
    )
    with pytest.raises(MODULE.DeploymentError, match="owner or mode"):
        MODULE._require_safe_restart_log_owner_mode(
            wrong_owner, os.getuid(), os.getgid()
        )


def test_restart_proof_reverifies_same_child_and_reports_ceil_duration(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    peer = SimpleNamespace(number=1, label="validator-1")
    bundle = SimpleNamespace(
        peers=(peer,), owner_uid=os.getuid(), owner_gid=os.getgid()
    )
    log = tmp_path / "logs" / "validator-1-supervisor.log"
    _write(log, b"historical output\n")
    events: list[object] = []
    managed = iter(((11, 22), (11, 33), (11, 33)))

    def verify(*_args: object, **_kwargs: object) -> tuple[int, int]:
        identity = next(managed)
        events.append(identity)
        return identity

    def terminate(pid: int) -> None:
        events.append(("terminate", pid))
        with log.open("ab") as stream:
            stream.write(MODULE.SNAPSHOT_LOAD_SUCCESS_MARKER + b"\n")

    advanced = object()

    def wait(*_args: object, **_kwargs: object) -> object:
        events.append("advanced")
        return advanced

    times_ns = iter((1_000_000_000, 1_001_000_001))
    monkeypatch.setattr(MODULE.time, "monotonic_ns", lambda: next(times_ns))
    monkeypatch.setattr(MODULE, "verify_managed_peer", verify)
    monkeypatch.setattr(MODULE, "wait_for_advancement", wait)
    ops = SimpleNamespace(terminate=terminate, process_exists=lambda _pid: False)

    actual = MODULE.restart_proof(
        bundle,
        "1" * 40,
        tmp_path,
        {peer.label: b"plist"},
        Path("/irohad"),
        object(),
        ops,
    )

    assert actual.fleet is advanced
    assert actual.duration_ms == 2
    assert events == [(11, 22), ("terminate", 22), (11, 33), "advanced", (11, 33)]


@pytest.mark.parametrize("final_identity", [(11, 44), (12, 33)])
def test_restart_proof_rejects_child_or_supervisor_drift_after_advancement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    final_identity: tuple[int, int],
) -> None:
    peer = SimpleNamespace(number=1, label="validator-1")
    bundle = SimpleNamespace(
        peers=(peer,), owner_uid=os.getuid(), owner_gid=os.getgid()
    )
    log = tmp_path / "logs" / "validator-1-supervisor.log"
    _write(log, b"historical output\n")
    managed = iter(((11, 22), (11, 33), final_identity))

    def terminate(_pid: int) -> None:
        with log.open("ab") as stream:
            stream.write(MODULE.SNAPSHOT_LOAD_SUCCESS_MARKER + b"\n")

    monkeypatch.setattr(
        MODULE,
        "verify_managed_peer",
        lambda *_args, **_kwargs: next(managed),
    )
    monkeypatch.setattr(
        MODULE, "wait_for_advancement", lambda *_args, **_kwargs: object()
    )
    ops = SimpleNamespace(terminate=terminate, process_exists=lambda _pid: False)

    with pytest.raises(MODULE.DeploymentError, match="supervisor or replacement child"):
        MODULE.restart_proof(
            bundle,
            "1" * 40,
            tmp_path,
            {peer.label: b"plist"},
            Path("/irohad"),
            object(),
            ops,
        )


def test_restart_proof_rejects_measured_duration_beyond_bound(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    peer = SimpleNamespace(number=1, label="validator-1")
    bundle = SimpleNamespace(
        peers=(peer,), owner_uid=os.getuid(), owner_gid=os.getgid()
    )
    log = tmp_path / "logs" / "validator-1-supervisor.log"
    _write(log, b"historical output\n")
    managed = iter(((11, 22), (11, 33), (11, 33)))

    def terminate(_pid: int) -> None:
        with log.open("ab") as stream:
            stream.write(MODULE.SNAPSHOT_LOAD_SUCCESS_MARKER + b"\n")

    times_ns = iter((1_000_000_000, 46_000_000_001))
    monkeypatch.setattr(MODULE.time, "monotonic_ns", lambda: next(times_ns))
    monkeypatch.setattr(
        MODULE,
        "verify_managed_peer",
        lambda *_args, **_kwargs: next(managed),
    )
    monkeypatch.setattr(
        MODULE, "wait_for_advancement", lambda *_args, **_kwargs: object()
    )
    ops = SimpleNamespace(terminate=terminate, process_exists=lambda _pid: False)

    with pytest.raises(MODULE.DeploymentError, match="exceeded 45 seconds"):
        MODULE.restart_proof(
            bundle,
            "1" * 40,
            tmp_path,
            {peer.label: b"plist"},
            Path("/irohad"),
            object(),
            ops,
        )


def test_controller_fails_before_restart_proof_when_terminal_latched() -> None:
    calls: list[str] = []

    def terminal_checker() -> None:
        calls.append("terminal")
        raise MODULE.DeploymentError("terminal-unhealthy")

    with pytest.raises(MODULE.DeploymentError, match="terminal-unhealthy"):
        MODULE.restart_proof(
            SimpleNamespace(),
            "1" * 40,
            Path("/runtime"),
            {},
            Path("/irohad"),
            SimpleNamespace(),
            SimpleNamespace(),
            terminal_checker=terminal_checker,
        )

    assert calls == ["terminal"]


def _darwin_procargs_payload(
    executable: str,
    argv: tuple[str, ...],
    *,
    trailing: bytes = b"",
) -> bytes:
    argc = len(argv).to_bytes(
        MODULE.ctypes.sizeof(MODULE.ctypes.c_int),
        byteorder=sys.byteorder,
        signed=True,
    )
    encoded_argv = b"".join(os.fsencode(argument) + b"\0" for argument in argv)
    return argc + os.fsencode(executable) + b"\0\0\0" + encoded_argv + trailing


def test_darwin_procargs2_parser_preserves_exact_nul_delimited_arguments() -> None:
    argv = (
        "/System Path/Python.app/Contents/MacOS/Python",
        "argument with spaces",
        "literal'quote",
        'literal"quote',
    )
    payload = _darwin_procargs_payload(
        argv[0], argv, trailing=b"KEY=environment value\0"
    )

    assert MODULE.parse_darwin_procargs2(payload) == argv


@pytest.mark.parametrize(
    ("payload", "message"),
    [
        pytest.param(b"", "invalid size", id="empty"),
        pytest.param(
            (0).to_bytes(
                MODULE.ctypes.sizeof(MODULE.ctypes.c_int),
                byteorder=sys.byteorder,
                signed=True,
            )
            + b"/runtime\0",
            "count",
            id="zero-argc",
        ),
        pytest.param(
            _darwin_procargs_payload("/runtime", ("/other",)),
            "differs from argv",
            id="executable-mismatch",
        ),
        pytest.param(
            _darwin_procargs_payload("/runtime", ("/runtime",))[:-1],
            "incomplete",
            id="truncated-argv",
        ),
        pytest.param(
            _darwin_procargs_payload("/runtime", ("/runtime", "")),
            "empty argument",
            id="empty-argument",
        ),
    ],
)
def test_darwin_procargs2_parser_rejects_malformed_payloads(
    payload: bytes, message: str
) -> None:
    with pytest.raises(MODULE.DeploymentError, match=message):
        MODULE.parse_darwin_procargs2(payload)


def test_darwin_procargs2_parser_rejects_payload_above_allocation_bound() -> None:
    payload = b"\0" * (MODULE.MAX_PROCESS_ARGUMENT_BYTES + 1)

    with pytest.raises(MODULE.DeploymentError, match="invalid size"):
        MODULE.parse_darwin_procargs2(payload)


def test_process_inspection_rejects_native_argv_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    ops = MODULE.SystemOps()
    monkeypatch.setattr(
        ops,
        "run",
        lambda *_args, **_kwargs: SimpleNamespace(returncode=0, stdout="1 501\n"),
    )
    samples = iter((("/runtime", "first"), ("/runtime", "second")))
    monkeypatch.setattr(
        MODULE, "read_darwin_process_argv", lambda _pid: next(samples)
    )

    with pytest.raises(MODULE.DeploymentError, match="changed during capture"):
        ops.inspect_process(77)


def test_process_inspection_preserves_stable_native_argv(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    ops = MODULE.SystemOps()
    monkeypatch.setattr(
        ops,
        "run",
        lambda *_args, **_kwargs: SimpleNamespace(returncode=0, stdout="1 501\n"),
    )
    argv = ("/runtime path", "argument with spaces")
    monkeypatch.setattr(MODULE, "read_darwin_process_argv", lambda _pid: argv)

    assert ops.inspect_process(77) == MODULE.ProcessInfo(
        pid=77,
        ppid=1,
        uid=501,
        argv=argv,
    )


class _OldCaptureOps:
    def __init__(
        self,
        supervisor_pid: int,
        supervisor_argv: tuple[str, ...],
        *,
        child_pids: tuple[int, ...] = (),
        child_processes: dict[int, MODULE.ProcessInfo] | None = None,
    ) -> None:
        self.supervisor_pid = supervisor_pid
        self._child_pids = child_pids
        self.processes = {
            supervisor_pid: MODULE.ProcessInfo(
                pid=supervisor_pid,
                ppid=1,
                uid=os.getuid(),
                argv=supervisor_argv,
            ),
            **(child_processes or {}),
        }

    def inspect_process(self, pid: int) -> MODULE.ProcessInfo:
        return self.processes[pid]

    def launchd_print(self, _label: str) -> str:
        return f"\tpid = {self.supervisor_pid}\n"

    def child_pids(self, parent_pid: int) -> tuple[int, ...]:
        assert parent_pid == self.supervisor_pid
        return self._child_pids


def _old_capture_payload(pid_file: Path) -> tuple[dict[str, object], tuple[str, ...]]:
    supervisor_argv = (
        "/usr/bin/python3",
        "/old/taira_peer_supervisor.py",
        "--binary",
        "/old/irohad",
        "--config",
        "/old/config.toml",
        "--pid-file",
        str(pid_file),
    )
    return (
        {
            "ProgramArguments": list(supervisor_argv),
            "UserName": pwd.getpwuid(os.getuid()).pw_name,
            "GroupName": grp.getgrgid(os.getgid()).gr_name,
        },
        supervisor_argv,
    )


def _framework_python_capture_payload(
    tmp_path: Path,
) -> tuple[
    dict[str, object],
    tuple[str, ...],
    tuple[str, ...],
    Path,
]:
    package = tmp_path / "Cellar/python@3.14/3.14.6"
    version_root = (
        package / "Frameworks/Python.framework/Versions/3.14"
    )
    resolved_launcher = version_root / "bin/python3.14"
    runtime = version_root / "Resources/Python.app/Contents/MacOS/Python"
    _write(resolved_launcher, b"launcher")
    _write(runtime, b"runtime")
    resolved_launcher.chmod(0o500)
    runtime.chmod(0o500)
    launcher = package / "bin/python3.14"
    launcher.parent.mkdir(parents=True, exist_ok=True)
    launcher.symlink_to(resolved_launcher)
    pid_file = tmp_path / "absent-framework.pid"
    tail = (
        "/old/taira_peer_supervisor.py",
        "--binary",
        "/old/irohad",
        "--config",
        "/old/config.toml",
        "--pid-file",
        str(pid_file),
    )
    plist_argv = (str(launcher), *tail)
    runtime_argv = (str(runtime.resolve(strict=True)), *tail)
    payload = {
        "ProgramArguments": list(plist_argv),
        "UserName": pwd.getpwuid(os.getuid()).pw_name,
        "GroupName": grp.getgrgid(os.getgid()).gr_name,
    }
    return payload, plist_argv, runtime_argv, runtime


def test_framework_python_rewrite_requires_flag_and_binds_observed_rollback_argv(
    tmp_path: Path,
) -> None:
    payload, _plist_argv, runtime_argv, _runtime = (
        _framework_python_capture_payload(tmp_path)
    )
    ops = _OldCaptureOps(46, runtime_argv)

    with pytest.raises(MODULE.DeploymentError, match="differs from its plist"):
        MODULE.inspect_old_managed_identity(
            payload,
            "old-job",
            46,
            ops,
            allow_absent_child=True,
        )

    managed = MODULE.inspect_old_managed_identity(
        payload,
        "old-job",
        46,
        ops,
        allow_absent_child=True,
        allow_framework_python_argv0_rewrite=True,
    )
    assert managed.supervisor_argv == runtime_argv
    snapshot = MODULE.PlistSnapshot(
        path=tmp_path / "old-job.plist",
        body=b"plist",
        mode=0o644,
        uid=0,
        gid=0,
        managed=managed,
    )
    MODULE.verify_restored_snapshot(snapshot, ops)

    ops.processes[46] = dataclasses.replace(
        ops.processes[46], argv=tuple(payload["ProgramArguments"])
    )
    with pytest.raises(MODULE.DeploymentError, match="identity is wrong"):
        MODULE.verify_restored_snapshot(snapshot, ops)


@pytest.mark.parametrize("mutation", ["wrong-root", "tail", "writable"])
def test_framework_python_rewrite_rejects_any_nonstructural_difference(
    tmp_path: Path, mutation: str
) -> None:
    _payload, plist_argv, runtime_argv, runtime = (
        _framework_python_capture_payload(tmp_path)
    )
    if mutation == "wrong-root":
        other = tmp_path / "other/Resources/Python.app/Contents/MacOS/Python"
        _write(other, b"runtime")
        other.chmod(0o500)
        runtime_argv = (str(other.resolve(strict=True)), *runtime_argv[1:])
    elif mutation == "tail":
        runtime_argv = (*runtime_argv[:-1], "/other/pid")
    else:
        runtime.chmod(0o520)

    assert not MODULE.framework_python_argv0_rewrite_matches(
        plist_argv, runtime_argv, owner_uid=os.getuid()
    )


def test_absent_old_child_requires_explicit_reset_authorization(
    tmp_path: Path,
) -> None:
    pid_file = tmp_path / "absent.pid"
    payload, supervisor_argv = _old_capture_payload(pid_file)
    ops = _OldCaptureOps(41, supervisor_argv)

    with pytest.raises(MODULE.DeploymentError, match="PID file is absent"):
        MODULE.inspect_old_managed_identity(payload, "old-job", 41, ops)

    managed = MODULE.inspect_old_managed_identity(
        payload,
        "old-job",
        41,
        ops,
        allow_absent_child=True,
    )
    assert managed.child_was_present is False


def test_absent_old_pid_rejects_any_untracked_supervisor_child(
    tmp_path: Path,
) -> None:
    pid_file = tmp_path / "absent.pid"
    payload, supervisor_argv = _old_capture_payload(pid_file)
    ops = _OldCaptureOps(42, supervisor_argv, child_pids=(142,))

    with pytest.raises(MODULE.DeploymentError, match="still owns a child"):
        MODULE.inspect_old_managed_identity(
            payload,
            "old-job",
            42,
            ops,
            allow_absent_child=True,
        )


def test_absent_old_pid_rejects_child_emerging_between_samples(
    tmp_path: Path,
) -> None:
    pid_file = tmp_path / "absent.pid"
    payload, supervisor_argv = _old_capture_payload(pid_file)
    ops = _OldCaptureOps(45, supervisor_argv)
    child_samples = iter(((), (145,)))
    ops.child_pids = lambda parent_pid: (
        next(child_samples) if parent_pid == 45 else ()
    )

    with pytest.raises(MODULE.DeploymentError, match="still owns a child"):
        MODULE.inspect_old_managed_identity(
            payload,
            "old-job",
            45,
            ops,
            allow_absent_child=True,
        )


def test_existing_old_pid_rejects_a_mismatched_child_even_when_relaxed(
    tmp_path: Path,
) -> None:
    pid_file = tmp_path / "managed.pid"
    _write(pid_file, b"143\n")
    payload, supervisor_argv = _old_capture_payload(pid_file)
    wrong_child = MODULE.ProcessInfo(
        pid=143,
        ppid=43,
        uid=os.getuid(),
        argv=("/old/other", "--sora", "--config", "/old/config.toml"),
    )
    ops = _OldCaptureOps(
        43,
        supervisor_argv,
        child_pids=(143,),
        child_processes={143: wrong_child},
    )

    with pytest.raises(MODULE.DeploymentError, match="identity differs"):
        MODULE.inspect_old_managed_identity(
            payload,
            "old-job",
            43,
            ops,
            allow_absent_child=True,
        )


def test_degraded_rollback_accepts_absence_or_exact_recovery_only(
    tmp_path: Path,
) -> None:
    pid_file = tmp_path / "managed.pid"
    payload, supervisor_argv = _old_capture_payload(pid_file)
    ops = _OldCaptureOps(44, supervisor_argv)
    managed = MODULE.inspect_old_managed_identity(
        payload,
        "old-job",
        44,
        ops,
        allow_absent_child=True,
    )
    snapshot = MODULE.PlistSnapshot(
        path=tmp_path / "old-job.plist",
        body=b"plist",
        mode=0o644,
        uid=0,
        gid=0,
        managed=managed,
    )

    MODULE.verify_restored_snapshot(snapshot, ops)

    _write(pid_file, b"144\n")
    ops._child_pids = (144,)
    ops.processes[144] = MODULE.ProcessInfo(
        pid=144,
        ppid=44,
        uid=os.getuid(),
        argv=managed.child_argv,
    )
    MODULE.verify_restored_snapshot(snapshot, ops)

    ops.processes[144] = dataclasses.replace(
        ops.processes[144],
        argv=("/old/wrong",),
    )
    with pytest.raises(MODULE.DeploymentError, match="identity differs"):
        MODULE.verify_restored_snapshot(snapshot, ops)


def test_dry_run_execute_never_calls_apply(monkeypatch: pytest.MonkeyPatch) -> None:
    events: list[str] = []
    admission = SimpleNamespace(
        archive_sha256="0" * 64,
        receipt_id="f" * 64,
        reset_manifest_sha256="1" * 64,
        binary_sha256="a" * 64,
        supervisor_sha256="b" * 64,
        source_commit="c" * 40,
        restart_generation="9" * 64,
    )
    bundle = SimpleNamespace(
        root=Path("/bundle"),
        bundle_bytes=1,
        free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        fsync_latency_ms=1.0,
        release=SimpleNamespace(
            release_attestation_sha256="d" * 64,
            manifest_sha256="e" * 64,
            release_policy_sha256="f" * 64,
            tree_sha256="1" * 64,
        ),
    )
    sources = SimpleNamespace(
        binary_sha256="a" * 64,
        supervisor_sha256="b" * 64,
    )
    cohort = tuple(
        SimpleNamespace(
            path=Path(f"/Library/LaunchDaemons/{label}.plist"),
            managed=SimpleNamespace(child_was_present=True),
        )
        for label in MODULE.LABELS
    )
    monkeypatch.setattr(MODULE, "validate_bundle", lambda *args, **kwargs: bundle)
    monkeypatch.setattr(MODULE, "validate_sources", lambda *args, **kwargs: sources)
    monkeypatch.setattr(
        MODULE,
        "verify_deployment_admission",
        lambda _args: (events.append("admission-verify") or admission),
    )
    monkeypatch.setattr(MODULE, "require_inputs_match_admission", lambda *args: None)
    monkeypatch.setattr(
        MODULE,
        "require_admission_archive_unchanged",
        lambda _admission: events.append("archive-recheck"),
    )
    monkeypatch.setattr(
        MODULE,
        "consume_admission_receipt",
        lambda *_args: pytest.fail("dry run consumed an admission receipt"),
    )
    monkeypatch.setattr(
        MODULE,
        "capture_old_cohort",
        lambda _ops, *, allow_absent_child: (events.append("capture") or cohort),
    )
    monkeypatch.setattr(
        MODULE,
        "apply_reset",
        lambda *args, **kwargs: pytest.fail("dry run called apply_reset"),
    )
    monkeypatch.setattr(
        MODULE,
        "exclusive_deployment_lock",
        lambda: pytest.fail("dry run acquired the deployment lock"),
    )
    args = argparse.Namespace(
        bundle=Path("/bundle"),
        binary=Path("/binary"),
        supervisor=Path("/supervisor"),
        admission_archive=Path("/candidate.tar.gz"),
        admission_authority_dir=Path("/authority"),
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
        expected_source_commit="c" * 40,
        expected_cargo_lock_sha256="d" * 64,
        expected_workspace_source_manifest_sha256="e" * 64,
        expected_receipt_id="f" * 64,
        trusted_signing_fingerprint="1" * 64,
        release_manifest_verifier=Path("/sorafs-validate"),
        trusted_release_manifest_verifier_sha256="2" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        allow_absent_old_child=False,
        apply=False,
    )

    report = MODULE.execute(args, ops=MODULE.SystemOps())
    assert report["mode"] == "verified-read-only-dry-run"
    assert report["applied"] is False
    assert report["admission_receipt_consumed"] is False
    assert events == ["admission-verify", "capture", "archive-recheck"]
    assert report["qualification_seal"] == str(MODULE.qualification_seal_path("1" * 64))


@pytest.mark.parametrize("apply", [False, True], ids=("dry-run", "apply"))
def test_admission_failure_precedes_every_deployment_preflight(
    monkeypatch: pytest.MonkeyPatch,
    apply: bool,
) -> None:
    events: list[str] = []
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)

    def reject_admission(_args):
        events.append("admission-verify")
        raise MODULE.DeploymentError("injected admission refusal")

    monkeypatch.setattr(MODULE, "verify_deployment_admission", reject_admission)
    monkeypatch.setattr(
        MODULE,
        "validate_bundle",
        lambda *_args, **_kwargs: pytest.fail("bundle preflight preceded admission"),
    )
    args = argparse.Namespace(
        bundle=Path("/bundle"),
        binary=Path("/binary"),
        supervisor=Path("/supervisor"),
        admission_archive=Path("/candidate.tar.gz"),
        admission_authority_dir=Path("/authority"),
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
        expected_source_commit="c" * 40,
        expected_cargo_lock_sha256="d" * 64,
        expected_workspace_source_manifest_sha256="e" * 64,
        expected_receipt_id="f" * 64,
        trusted_signing_fingerprint="1" * 64,
        release_manifest_verifier=Path("/sorafs-validate"),
        trusted_release_manifest_verifier_sha256="2" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        allow_absent_old_child=False,
        apply=apply,
    )

    with pytest.raises(MODULE.DeploymentError, match="admission refusal"):
        MODULE.execute(args, ops=MODULE.SystemOps())

    assert events == ["admission-verify"]


def test_apply_lock_spans_old_cohort_capture_and_rollout(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    events: list[str] = []
    admission = SimpleNamespace(
        archive_sha256="0" * 64,
        receipt_id="f" * 64,
        reset_manifest_sha256="1" * 64,
        binary_sha256="a" * 64,
        supervisor_sha256="b" * 64,
        source_commit="c" * 40,
        restart_generation="9" * 64,
    )
    bundle = SimpleNamespace()
    sources = SimpleNamespace()
    cohort = tuple(object() for _ in range(MODULE.PEER_COUNT))
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setattr(MODULE, "validate_bundle", lambda *args, **kwargs: bundle)
    monkeypatch.setattr(MODULE, "validate_sources", lambda *args, **kwargs: sources)
    monkeypatch.setattr(
        MODULE,
        "verify_deployment_admission",
        lambda _args: (events.append("admission-verify") or admission),
    )
    monkeypatch.setattr(
        MODULE,
        "require_inputs_match_admission",
        lambda *args: events.append("bind-inputs"),
    )
    monkeypatch.setattr(
        MODULE,
        "require_admission_bound_inputs_unchanged",
        lambda *args: events.append("recheck-inputs"),
    )
    monkeypatch.setattr(
        MODULE,
        "capture_old_cohort",
        lambda _ops, *, allow_absent_child: (
            events.append(f"capture:{allow_absent_child}") or cohort
        ),
    )
    def apply(*_args, **kwargs):
        events.append("apply")
        kwargs["rollout_starter"]()
        return {"applied": True}

    monkeypatch.setattr(MODULE, "apply_reset", apply)

    @contextlib.contextmanager
    def consume(_admission):
        events.append("consume-enter")
        transaction = SimpleNamespace(
            mark_rollout_started=lambda: events.append("rollout-start")
        )
        try:
            yield transaction
        finally:
            events.append("consume-exit")

    @contextlib.contextmanager
    def lock():
        events.append("lock-enter")
        try:
            yield
        finally:
            events.append("lock-exit")

    monkeypatch.setattr(MODULE, "exclusive_deployment_lock", lock)
    monkeypatch.setattr(MODULE, "consume_admission_receipt", consume)
    args = argparse.Namespace(
        bundle=Path("/bundle"),
        binary=Path("/binary"),
        supervisor=Path("/supervisor"),
        admission_archive=Path("/candidate.tar.gz"),
        admission_authority_dir=Path("/authority"),
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
        expected_source_commit="c" * 40,
        expected_cargo_lock_sha256="d" * 64,
        expected_workspace_source_manifest_sha256="e" * 64,
        expected_receipt_id="f" * 64,
        trusted_signing_fingerprint="1" * 64,
        release_manifest_verifier=Path("/sorafs-validate"),
        trusted_release_manifest_verifier_sha256="2" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        allow_absent_old_child=True,
        apply=True,
    )

    assert MODULE.execute(args, ops=MODULE.SystemOps()) == {
        "admission_archive_sha256": "0" * 64,
        "admission_receipt_consumed": True,
        "admission_receipt_id": "f" * 64,
        "applied": True,
    }
    assert events == [
        "admission-verify",
        "bind-inputs",
        "lock-enter",
        "admission-verify",
        "recheck-inputs",
        "capture:True",
        "recheck-inputs",
        "consume-enter",
        "apply",
        "rollout-start",
        "consume-exit",
        "lock-exit",
    ]


def _receipt_transaction_plan(tmp_path: Path) -> MODULE.AdmissionPlan:
    archive = tmp_path / "candidate.tar.gz"
    _write(archive, b"signed candidate archive")
    ledger = tmp_path / "rollout-admission-replay-v1.json"
    ledger.write_bytes(MODULE.rollout_admission.canonical_replay_ledger_bytes([]))
    return MODULE.AdmissionPlan(
        archive=archive,
        archive_state=MODULE._stable_admission_file(archive, "test archive"),
        authority_dir=tmp_path,
        replay_ledger=ledger,
        receipt_id="a" * 64,
        archive_sha256=hashlib.sha256(archive.read_bytes()).hexdigest(),
        source_commit="b" * 40,
        cargo_lock_sha256="c" * 64,
        workspace_source_manifest_sha256="d" * 64,
        reset_manifest_sha256="e" * 64,
        binary_sha256="f" * 64,
        supervisor_sha256="1" * 64,
        validator_config_sha256=tuple(
            (slug, f"{index}" * 64)
            for index, slug in enumerate(MODULE.SLUGS, start=2)
        ),
        restart_generation="6" * 64,
        signer_fingerprint_sha256="7" * 64,
        release_manifest_verifier_sha256="8" * 64,
    )


def _use_unprivileged_transaction_ledger(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        MODULE,
        "require_protected_replay_ledger",
        MODULE.rollout_admission.load_replay_ledger,
    )
    monkeypatch.setattr(
        MODULE,
        "atomic_replace_owned",
        lambda path, body, **_kwargs: path.write_bytes(body),
    )


def test_receipt_consumption_restores_exact_ledger_when_rollout_does_not_begin(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    prior = admission.replay_ledger.read_bytes()

    with pytest.raises(MODULE.DeploymentError, match="injected pre-cutover failure"):
        with MODULE.consume_admission_receipt(admission):
            assert admission.receipt_id in MODULE.rollout_admission.load_replay_ledger(
                admission.replay_ledger
            ).consumed_receipt_ids
            raise MODULE.DeploymentError("injected pre-cutover failure")

    assert admission.replay_ledger.read_bytes() == prior


def test_receipt_consumption_remains_committed_after_rollout_begins(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)

    with pytest.raises(MODULE.DeploymentError, match="injected post-cutover failure"):
        with MODULE.consume_admission_receipt(admission) as transaction:
            transaction.mark_rollout_started()
            raise MODULE.DeploymentError("injected post-cutover failure")

    consumed = MODULE.rollout_admission.load_replay_ledger(
        admission.replay_ledger
    ).consumed_receipt_ids
    assert consumed == (admission.receipt_id,)


def test_successful_receipt_transaction_rechecks_committed_ledger(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)

    with MODULE.consume_admission_receipt(admission) as transaction:
        transaction.mark_rollout_started()

    assert MODULE.rollout_admission.load_replay_ledger(
        admission.replay_ledger
    ).consumed_receipt_ids == (admission.receipt_id,)


def test_receipt_consumption_cannot_succeed_without_rollout_start(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    prior = admission.replay_ledger.read_bytes()

    with pytest.raises(MODULE.DeploymentError, match="without beginning"):
        with MODULE.consume_admission_receipt(admission):
            pass

    assert admission.replay_ledger.read_bytes() == prior


def test_rollout_start_rejects_removed_receipt_and_preserves_prior_ledger(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    prior = admission.replay_ledger.read_bytes()

    with pytest.raises(MODULE.DeploymentError, match="changed before rollout"):
        with MODULE.consume_admission_receipt(admission) as transaction:
            admission.replay_ledger.write_bytes(prior)
            transaction.mark_rollout_started()

    assert admission.replay_ledger.read_bytes() == prior


def test_unstarted_receipt_rollback_refuses_foreign_ledger_change(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    foreign_receipt = "b" * 64

    with pytest.raises(MODULE.DeploymentError, match="receipt rollback failed"):
        with MODULE.consume_admission_receipt(admission):
            admission.replay_ledger.write_bytes(
                MODULE.rollout_admission.canonical_replay_ledger_bytes(
                    [admission.receipt_id, foreign_receipt]
                )
            )
            raise MODULE.DeploymentError("injected failure after foreign mutation")

    assert MODULE.rollout_admission.load_replay_ledger(
        admission.replay_ledger
    ).consumed_receipt_ids == (admission.receipt_id, foreign_receipt)


def test_receipt_consumption_rejects_replay_under_lock(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    admission.replay_ledger.write_bytes(
        MODULE.rollout_admission.canonical_replay_ledger_bytes(
            [admission.receipt_id]
        )
    )
    _use_unprivileged_transaction_ledger(monkeypatch)

    with pytest.raises(MODULE.DeploymentError, match="already consumed"):
        with MODULE.consume_admission_receipt(admission):
            pytest.fail("replayed receipt entered deployment transaction")


def test_receipt_consumption_rejects_ledger_capacity_before_publication(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    _use_unprivileged_transaction_ledger(monkeypatch)
    prior = admission.replay_ledger.read_bytes()
    consumed = MODULE.rollout_admission.canonical_replay_ledger_bytes(
        [admission.receipt_id]
    )
    assert len(prior) < len(consumed)
    monkeypatch.setattr(
        MODULE.rollout_admission,
        "MAX_JSON_BYTES",
        len(consumed) - 1,
    )

    with pytest.raises(MODULE.DeploymentError, match="no capacity"):
        with MODULE.consume_admission_receipt(admission):
            pytest.fail("oversized replay ledger was published")

    assert admission.replay_ledger.read_bytes() == prior


def test_archive_substitution_is_rejected_before_rollout(tmp_path: Path) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    replacement = tmp_path / "replacement.tar.gz"
    _write(replacement, b"signed candidate archive")
    os.replace(replacement, admission.archive)

    with pytest.raises(MODULE.DeploymentError, match="substituted"):
        MODULE.require_admission_archive_unchanged(admission)


def test_receipt_config_binding_rejects_one_peer_substitution(tmp_path: Path) -> None:
    admission = _receipt_transaction_plan(tmp_path)
    peers = tuple(
        SimpleNamespace(slug=slug, config_sha256=digest)
        for slug, digest in admission.validator_config_sha256
    )
    peers = (
        SimpleNamespace(slug=peers[0].slug, config_sha256="9" * 64),
        *peers[1:],
    )
    bundle = SimpleNamespace(
        manifest_sha256=admission.reset_manifest_sha256,
        manifest={"source_commit": admission.source_commit},
        peers=peers,
    )
    sources = SimpleNamespace(
        binary_sha256=admission.binary_sha256,
        supervisor_sha256=admission.supervisor_sha256,
    )

    with pytest.raises(MODULE.DeploymentError, match="do not match"):
        MODULE.require_inputs_match_admission(bundle, sources, admission)


def test_under_lock_recheck_rejects_python_runtime_identity_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    binary = Path("/candidate/irohad")
    supervisor = Path("/candidate/supervisor.py")
    runtime = Path("/Library/Developer/CommandLineTools/Python.app/Python")
    stable = SimpleNamespace(
        st_dev=1,
        st_ino=2,
        st_mode=stat.S_IFREG | 0o555,
        st_uid=0,
        st_gid=0,
        st_nlink=1,
        st_size=3,
        st_mtime_ns=4,
        st_ctime_ns=5,
    )
    changed = copy.copy(stable)
    changed.st_ino = 9
    sources = MODULE.SourcePlan(
        binary=binary,
        binary_sha256="a" * 64,
        supervisor=supervisor,
        supervisor_sha256="b" * 64,
        python=runtime,
        python_identity=MODULE.metadata_identity(stable),
    )
    admission = SimpleNamespace(
        binary_sha256=sources.binary_sha256,
        supervisor_sha256=sources.supervisor_sha256,
    )
    monkeypatch.setattr(MODULE, "require_bundle_runtime_unchanged", lambda _bundle: None)
    monkeypatch.setattr(
        MODULE,
        "sha256_regular",
        lambda path, _maximum: (
            sources.binary_sha256 if path == binary else sources.supervisor_sha256,
            stable,
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "require_root_controlled_file",
        lambda path, *, executable: changed,
    )

    with pytest.raises(MODULE.DeploymentError, match="Python changed"):
        MODULE.require_admission_bound_inputs_unchanged(
            SimpleNamespace(), sources, admission
        )


def test_exclusive_deployment_lock_refuses_contention(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    lock_path = tmp_path / "deploy.lock"
    _write(lock_path, b"")
    real_fstat = os.fstat

    def root_fstat(descriptor: int) -> SimpleNamespace:
        info = real_fstat(descriptor)
        return SimpleNamespace(
            st_mode=info.st_mode,
            st_nlink=info.st_nlink,
            st_uid=0,
            st_gid=0,
        )

    def contended_flock(_descriptor: int, operation: int) -> None:
        if operation & MODULE.fcntl.LOCK_NB:
            raise BlockingIOError

    monkeypatch.setattr(MODULE, "DEPLOYMENT_LOCK", lock_path)
    monkeypatch.setattr(MODULE, "ensure_root_directory", lambda *args, **kwargs: None)
    monkeypatch.setattr(MODULE.os, "fstat", root_fstat)
    monkeypatch.setattr(MODULE.fcntl, "flock", contended_flock)

    with pytest.raises(MODULE.DeploymentError, match="holds the deployment lock"):
        with MODULE.exclusive_deployment_lock():
            pytest.fail("contended lock was acquired")


def test_headroom_is_required_on_every_distinct_filesystem(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    first = SimpleNamespace(
        stat=lambda: SimpleNamespace(st_dev=11),
        name="first",
    )
    second = SimpleNamespace(
        stat=lambda: SimpleNamespace(st_dev=22),
        name="second",
    )
    roots = {Path("/first"): first, Path("/second"): second}
    monkeypatch.setattr(MODULE, "existing_ancestor", lambda path: roots[path])
    monkeypatch.setattr(
        MODULE.shutil,
        "disk_usage",
        lambda path: SimpleNamespace(free=20_000 if path is first else 9_999),
    )

    with pytest.raises(MODULE.DeploymentError, match="device 22"):
        MODULE.require_filesystem_headroom([Path("/first"), Path("/second")], 10_000)


class _RollbackOps:
    def __init__(
        self,
        snapshots: tuple[MODULE.PlistSnapshot, ...],
        *,
        fail_bootout_label: str | None = None,
    ) -> None:
        self.loaded = set(MODULE.LABELS)
        self.calls: list[tuple[str, str]] = []
        self.fail_bootout_label = fail_bootout_label
        self.supervisor_pids = {
            snapshot.path.stem: 40 + index for index, snapshot in enumerate(snapshots)
        }
        self.processes: dict[int, MODULE.ProcessInfo] = {}
        for index, snapshot in enumerate(snapshots):
            supervisor_pid = self.supervisor_pids[snapshot.path.stem]
            child_pid = 140 + index
            self.processes[supervisor_pid] = MODULE.ProcessInfo(
                pid=supervisor_pid,
                ppid=1,
                uid=snapshot.managed.supervisor_uid,
                argv=snapshot.managed.supervisor_argv,
            )
            self.processes[child_pid] = MODULE.ProcessInfo(
                pid=child_pid,
                ppid=supervisor_pid,
                uid=snapshot.managed.child_uid,
                argv=snapshot.managed.child_argv,
            )

    def launchd_print(self, label: str) -> str | None:
        return (
            f"\tpid = {self.supervisor_pids[label]}\n" if label in self.loaded else None
        )

    def bootout(self, label: str) -> None:
        self.calls.append(("bootout", label))
        self.loaded.discard(label)
        if label == self.fail_bootout_label:
            raise MODULE.DeploymentError("injected bootout failure")

    def bootstrap(self, path: Path) -> None:
        self.calls.append(("bootstrap", path.stem))
        self.loaded.add(path.stem)

    def inspect_process(self, pid: int) -> MODULE.ProcessInfo:
        return self.processes[pid]

    def child_pids(self, parent_pid: int) -> tuple[int, ...]:
        return tuple(
            sorted(
                process.pid
                for process in self.processes.values()
                if process.ppid == parent_pid
            )
        )


def _rollback_snapshots(tmp_path: Path) -> tuple[MODULE.PlistSnapshot, ...]:
    snapshots: list[MODULE.PlistSnapshot] = []
    for index, label in enumerate(MODULE.LABELS):
        pid_file = tmp_path / f"{label}.pid"
        _write(pid_file, f"{140 + index}\n".encode())
        binary = f"/old/bin/irohad-{index}"
        config = f"/old/config-{index}.toml"
        supervisor_argv = (
            "/usr/bin/python3",
            "/old/taira_peer_supervisor.py",
            "--binary",
            binary,
            "--config",
            config,
            "--pid-file",
            str(pid_file),
        )
        managed = MODULE.OldManagedIdentity(
            supervisor_uid=os.getuid(),
            supervisor_argv=supervisor_argv,
            child_uid=os.getuid(),
            child_argv=(binary, "--sora", "--config", config),
            pid_file=pid_file,
            pid_file_gid=os.getgid(),
            child_was_present=True,
        )
        snapshots.append(
            MODULE.PlistSnapshot(
                path=tmp_path / f"{label}.plist",
                body=f"old-{label}".encode(),
                mode=0o644,
                uid=0,
                gid=0,
                managed=managed,
            )
        )
    return tuple(snapshots)


def test_rollback_unloads_and_restores_the_whole_four_job_cohort(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    snapshots = _rollback_snapshots(tmp_path)
    restored: list[str] = []
    monkeypatch.setattr(
        MODULE,
        "atomic_replace_owned",
        lambda path, body, **kwargs: restored.append(path.stem),
    )
    ops = _RollbackOps(snapshots)

    MODULE.rollback_cohort(snapshots, ops)  # type: ignore[arg-type]

    assert restored == list(MODULE.LABELS)
    assert [label for action, label in ops.calls if action == "bootout"] == list(
        MODULE.LABELS
    )
    assert [label for action, label in ops.calls if action == "bootstrap"] == list(
        MODULE.LABELS
    )
    assert ops.loaded == set(MODULE.LABELS)


def test_rollback_attempts_full_restore_after_injected_bootout_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    snapshots = _rollback_snapshots(tmp_path)
    restored: list[str] = []
    monkeypatch.setattr(
        MODULE,
        "atomic_replace_owned",
        lambda path, body, **kwargs: restored.append(path.stem),
    )
    ops = _RollbackOps(snapshots, fail_bootout_label=MODULE.LABELS[1])

    with pytest.raises(MODULE.DeploymentError, match="rollback was incomplete"):
        MODULE.rollback_cohort(snapshots, ops)  # type: ignore[arg-type]

    assert restored == list(MODULE.LABELS)
    assert [label for action, label in ops.calls if action == "bootstrap"] == list(
        MODULE.LABELS
    )


def test_cli_defaults_match_the_audited_operator_contract() -> None:
    args = MODULE.build_parser().parse_args(
        [
            "--bundle",
            "/bundle",
            "--binary",
            "/binary",
            "--supervisor",
            "/supervisor",
            "--admission-archive",
            "/candidate.tar.gz",
            "--admission-authority-dir",
            "/authority",
            "--expected-source-commit",
            "c" * 40,
            "--expected-cargo-lock-sha256",
            "d" * 64,
            "--expected-workspace-source-manifest-sha256",
            "e" * 64,
            "--expected-receipt-id",
            "f" * 64,
            "--trusted-signing-fingerprint",
            "1" * 64,
            "--release-manifest-verifier",
            "/sorafs-validate",
            "--trusted-release-manifest-verifier-sha256",
            "2" * 64,
        ]
    )
    assert args.health_timeout_seconds == 240
    assert args.minimum_free_bytes == 17_179_869_184
    assert args.maximum_fsync_latency_ms == 250
    assert args.supervisor_python == MODULE.DEFAULT_SUPERVISOR_PYTHON
    assert not hasattr(args, "restart_generation")
    assert not hasattr(args, "expected_binary_sha256")
    assert not hasattr(args, "expected_supervisor_sha256")
    assert args.allow_absent_old_child is False
    assert args.allow_framework_python_argv0_rewrite is False
    assert args.apply is False
