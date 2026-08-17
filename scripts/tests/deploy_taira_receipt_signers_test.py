"""Focused deploy-bound Torii receipt-signer identity tests."""

from __future__ import annotations

import copy
import dataclasses
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import taira_peer_supervisor as supervisor
from scripts.tests import deploy_taira_v21_reset_test as base


deploy = base.MODULE


def test_receipt_signer_map_rejects_omission_reorder_node_mismatch_and_secret() -> None:
    accepted = base._receipt_signer_map()
    assert deploy.receipt_signer_public_map(
        deploy.require_receipt_signer_map(accepted, "fixture")
    ) == accepted

    omitted = dict(accepted)
    omitted.pop(deploy.SLUGS[-1])
    reordered = dict(reversed(list(accepted.items())))
    mismatched = copy.deepcopy(accepted)
    mismatched[deploy.SLUGS[0]]["node_id"] = (
        deploy.validator_renderer.RECEIPT_NODE_ID_PREFIX + "0" * 64
    )
    private_leak = copy.deepcopy(accepted)
    private_leak[deploy.SLUGS[0]]["receipt_private_key"] = "812620" + "01" * 32

    for tampered in (omitted, reordered, mismatched, private_leak):
        with pytest.raises(deploy.DeploymentError):
            deploy.require_receipt_signer_map(tampered, "fixture")


def test_bundle_preflight_rejects_receipt_signer_slug_association_swap(
    tmp_path: Path,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = base._build_bundle(tmp_path, binary_sha, source_commit)
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    signers = manifest["receipt_signers"]
    first, second = deploy.SLUGS[:2]
    signers[first], signers[second] = signers[second], signers[first]
    base._write(
        manifest_path,
        (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode(),
    )

    with pytest.raises(deploy.DeploymentError, match="differs from the reset manifest"):
        base._validate(bundle, binary_sha, source_commit)


def test_bundle_preflight_rejects_receipt_config_keypair_mismatch(
    tmp_path: Path,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = base._build_bundle(tmp_path, binary_sha, source_commit)
    slug = deploy.SLUGS[0]
    config_path = bundle / "rendered" / slug / "config.toml"
    _, private_one, _ = base._receipt_keypair(1)
    _, private_two, _ = base._receipt_keypair(2)
    config = config_path.read_text(encoding="utf-8").replace(
        f'receipt_private_key = "{private_one}"',
        f'receipt_private_key = "{private_two}"',
        1,
    )
    base._write(config_path, config.encode())
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["configs"][slug] = hashlib.sha256(config.encode()).hexdigest()
    base._write(
        manifest_path,
        (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode(),
    )

    with pytest.raises(deploy.DeploymentError, match="receipt keypair is invalid"):
        base._validate(bundle, binary_sha, source_commit)


def test_deployed_receipt_signers_bind_exact_supervisor_runtime_identity(
    tmp_path: Path,
) -> None:
    bundle, sources, binary_info = base._fake_plan(tmp_path)
    restart_generation = "4" * 64
    signers = deploy.deployed_receipt_signer_map(
        bundle, sources, binary_info, restart_generation
    )

    assert list(signers) == list(deploy.SLUGS)
    assert "812620" not in json.dumps(signers)
    for peer in bundle.peers:
        signer = signers[peer.slug]
        assert set(signer) == {
            "binary_stat_seal",
            "config_sha256",
            "lifecycle_binding_sha256",
            "node_id",
            "public_key",
            "runtime_binding_sha256",
        }
        args = SimpleNamespace(
            binary_sha256=sources.binary_sha256,
            binary_device=binary_info.st_dev,
            binary_inode=binary_info.st_ino,
            binary_size=binary_info.st_size,
            binary_mtime_ns=binary_info.st_mtime_ns,
            binary_ctime_ns=binary_info.st_ctime_ns,
            config_sha256=peer.config_sha256,
            restart_generation=restart_generation,
        )
        assert signer["runtime_binding_sha256"] == supervisor.terminal_binding_sha256(
            args
        )
        assert signer["lifecycle_binding_sha256"] == supervisor.lifecycle_binding_sha256(
            args, peer.slug, signer["node_id"]
        )


def test_deploy_rejects_qualification_receipt_signer_association_swap(
    tmp_path: Path,
) -> None:
    admission = base._receipt_transaction_plan(tmp_path)
    peers = tuple(
        SimpleNamespace(slug=slug, config_sha256=digest)
        for slug, digest in admission.validator_config_sha256
    )
    bundle = SimpleNamespace(
        manifest_sha256=admission.reset_manifest_sha256,
        manifest={
            "source_commit": admission.source_commit,
            "dpn_validator_release_commit": admission.dpn_validator_release_commit,
        },
        peers=peers,
        receipt_signers=admission.receipt_signers,
    )
    sources = SimpleNamespace(
        binary_sha256=admission.binary_sha256,
        supervisor_sha256=admission.supervisor_sha256,
    )
    swapped = (
        admission.receipt_signers[1],
        admission.receipt_signers[0],
        *admission.receipt_signers[2:],
    )

    with pytest.raises(deploy.DeploymentError, match="receipt-signer inputs"):
        deploy.require_inputs_match_admission(
            bundle,
            sources,
            dataclasses.replace(admission, receipt_signers=swapped),
        )
