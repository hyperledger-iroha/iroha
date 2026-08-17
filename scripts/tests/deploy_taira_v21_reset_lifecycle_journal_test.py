"""Focused lifecycle-journal wiring tests for the public Taira reset."""

from __future__ import annotations

import hashlib
import json
import os
import plistlib
import pwd
import grp
import stat
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import deploy_taira_v21_reset as deploy


RECEIPT_PRIVATE_PAYLOAD = (1).to_bytes(32, "big")
RECEIPT_PUBLIC_KEY = deploy.validator_renderer.RECEIPT_PUBLIC_KEY_PREFIX + (
    deploy.validator_renderer._secp256k1_public_payload(
        RECEIPT_PRIVATE_PAYLOAD
    ).hex().upper()
)
NODE_ID = deploy.validator_renderer.receipt_node_id(RECEIPT_PUBLIC_KEY)


def test_block_hash_normalization_rejects_an_unmarked_hash() -> None:
    with pytest.raises(deploy.DeploymentError, match="marker bit"):
        deploy.normalized_block_hash("aa" * 32, "test block")


def private_directory(path: Path) -> Path:
    """Create one owner-private fixture directory."""

    path.mkdir(parents=True, mode=0o700)
    path.chmod(0o700)
    return path


def fixture_plan(tmp_path: Path) -> tuple[object, object, os.stat_result]:
    """Return the fields consumed by plist and lifecycle layout helpers."""

    peers = []
    for number, (label, slug) in enumerate(
        zip(deploy.LABELS, deploy.SLUGS, strict=True), start=1
    ):
        workdir = private_directory(tmp_path / "peers" / slug)
        storage = private_directory(workdir / "storage")
        config = workdir / "config.toml"
        config.write_text("chain = \"taira\"\n", encoding="ascii")
        config.chmod(0o600)
        workdir_info = workdir.lstat()
        storage_info = storage.lstat()
        peers.append(
            SimpleNamespace(
                number=number,
                label=label,
                slug=slug,
                config=config,
                config_sha256=hashlib.sha256(config.read_bytes()).hexdigest(),
                workdir=workdir,
                storage=storage,
                workdir_device=workdir_info.st_dev,
                workdir_inode=workdir_info.st_ino,
                storage_device=storage_info.st_dev,
                storage_inode=storage_info.st_ino,
            )
        )
    uid, gid = os.getuid(), os.getgid()
    signer_plans = tuple(
        deploy.ReceiptSignerPlan(
            slug=slug,
            node_id=deploy.validator_renderer.receipt_node_id(
                deploy.validator_renderer.RECEIPT_PUBLIC_KEY_PREFIX
                + deploy.validator_renderer._secp256k1_public_payload(
                    number.to_bytes(32, "big")
                ).hex().upper()
            ),
            public_key=(
                deploy.validator_renderer.RECEIPT_PUBLIC_KEY_PREFIX
                + deploy.validator_renderer._secp256k1_public_payload(
                    number.to_bytes(32, "big")
                ).hex().upper()
            ),
        )
        for number, slug in enumerate(deploy.SLUGS, start=1)
    )
    bundle = SimpleNamespace(
        root=tmp_path,
        owner_uid=uid,
        owner_gid=gid,
        runtime_user=pwd.getpwuid(uid).pw_name,
        runtime_group=grp.getgrgid(gid).gr_name,
        peers=tuple(peers),
        receipt_signers=signer_plans,
        manifest={
            "receipt_signers": deploy.receipt_signer_public_map(signer_plans)
        },
    )
    binary = tmp_path / "iroha3d"
    binary.write_bytes(b"binary")
    binary.chmod(0o700)
    sources = SimpleNamespace(
        python=Path("/usr/bin/python3"),
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
    )
    return bundle, sources, binary.lstat()


def test_plist_carries_exact_authenticated_lifecycle_binding(tmp_path: Path) -> None:
    """The supervisor receives the fixed root and authenticated peer identity."""

    bundle, sources, binary_info = fixture_plan(tmp_path)
    peer = bundle.peers[0]
    runtime = tmp_path / "runtime"
    root = deploy.lifecycle_journal_root(runtime, peer)

    body = deploy.render_plist(
        peer,
        bundle,
        sources,
        installed_binary=Path("/Library/SORA/Taira/binaries/iroha3d"),
        binary_info=binary_info,
        installed_supervisor=Path(
            "/Library/SORA/Taira/supervisors/taira_peer_supervisor.py"
        ),
        runtime_root=runtime,
        restart_generation="4" * 64,
        lifecycle_journal_root=root,
        authenticated_node_id=NODE_ID,
    )
    arguments = plistlib.loads(body)["ProgramArguments"]

    assert arguments[arguments.index("--lifecycle-journal-root") + 1] == str(root)
    assert arguments[arguments.index("--validator-id") + 1] == peer.slug
    assert arguments[arguments.index("--node-id") + 1] == NODE_ID


def test_plist_rejects_unbound_lifecycle_identity(tmp_path: Path) -> None:
    """No caller can redirect a journal or substitute an informal node label."""

    bundle, sources, binary_info = fixture_plan(tmp_path)
    peer = bundle.peers[0]
    runtime = tmp_path / "runtime"
    common = {
        "installed_binary": Path("/Library/SORA/Taira/binaries/iroha3d"),
        "binary_info": binary_info,
        "installed_supervisor": Path(
            "/Library/SORA/Taira/supervisors/taira_peer_supervisor.py"
        ),
        "runtime_root": runtime,
        "restart_generation": "4" * 64,
        "authenticated_node_id": NODE_ID,
    }

    with pytest.raises(deploy.DeploymentError, match="lifecycle journal path"):
        deploy.render_plist(
            peer,
            bundle,
            sources,
            lifecycle_journal_root=runtime / "lifecycle" / "validator-2",
            **common,
        )
    with pytest.raises(deploy.DeploymentError, match="authenticated node ID"):
        deploy.render_plist(
            peer,
            bundle,
            sources,
            lifecycle_journal_root=deploy.lifecycle_journal_root(runtime, peer),
            **{**common, "authenticated_node_id": "not canonical spaces"},
        )


def test_layout_provisions_four_distinct_private_roots(tmp_path: Path) -> None:
    """Every validator receives one fixed mode-0700 owner-private root."""

    bundle, _sources, _binary_info = fixture_plan(tmp_path)
    runtime = private_directory(tmp_path / "runtime")

    roots = deploy.install_lifecycle_journal_layout(bundle, runtime)

    assert set(roots) == set(deploy.LABELS)
    assert tuple(roots.values()) == tuple(
        runtime / "lifecycle" / slug for slug in deploy.SLUGS
    )
    for root in roots.values():
        info = root.lstat()
        assert stat.S_ISDIR(info.st_mode)
        assert stat.S_IMODE(info.st_mode) == 0o700
        assert (info.st_uid, info.st_gid) == (bundle.owner_uid, bundle.owner_gid)


def test_deploy_returns_only_receipt_signer_derived_lifecycle_node_ids(
    tmp_path: Path,
) -> None:
    """The public reset consumes only the authenticated signer projection."""

    bundle, _sources, _binary_info = fixture_plan(tmp_path)
    expected = {
        signer.slug: signer.node_id for signer in bundle.receipt_signers
    }

    assert deploy.require_authenticated_lifecycle_node_ids(bundle) == expected

    bundle.manifest["receipt_signers"][deploy.SLUGS[0]]["node_id"] = (
        deploy.validator_renderer.RECEIPT_NODE_ID_PREFIX + "0" * 64
    )
    with pytest.raises(deploy.DeploymentError, match="not derived"):
        deploy.require_authenticated_lifecycle_node_ids(bundle)


def test_deployment_completion_timestamp_is_positive_and_millisecond_exact(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The public deploy receipt carries the exact post-validation clock sample."""

    monkeypatch.setattr(deploy.time, "time_ns", lambda: 1_800_000_000_123_999_999)
    assert deploy.deployment_completed_at_unix_ms() == 1_800_000_000_123
    monkeypatch.setattr(deploy.time, "time_ns", lambda: 0)
    with pytest.raises(deploy.DeploymentError, match="clock is not positive"):
        deploy.deployment_completed_at_unix_ms()


def test_deploy_derives_exact_config_set_and_canonical_topology_digests(
    tmp_path: Path,
) -> None:
    """Post-deploy identities come from the authenticated plan and fleet sample."""

    bundle, _sources, _binary_info = fixture_plan(tmp_path)
    configs = {peer.slug: peer.config_sha256 for peer in bundle.peers}
    expected = hashlib.sha256(
        json.dumps(configs, sort_keys=True, separators=(",", ":")).encode("ascii")
    ).hexdigest()
    assert deploy.deployed_config_set_sha256(bundle) == expected

    topology = json.dumps(
        {"observed_lane_count": 7, "observed_catalog_hash": "a" * 64},
        sort_keys=True,
        separators=(",", ":"),
    )
    assert deploy.deployed_topology_sha256(topology) == hashlib.sha256(
        topology.encode("ascii")
    ).hexdigest()
    with pytest.raises(deploy.DeploymentError, match="not canonical JSON"):
        deploy.deployed_topology_sha256(topology + "\n")
