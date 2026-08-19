"""Focused tests for the guarded Taira v21 fresh-reset controller."""

from __future__ import annotations

import argparse
import contextlib
import copy
import dataclasses
import grp
import hashlib
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
from scripts.tests.taira_receipt_signer_test_support import (
    projection_config_text as _support_projection_config_text,
    receipt_keypair as _support_receipt_keypair,
    receipt_signer_map as _support_receipt_signer_map,
)

from scripts.tests.deploy_taira_v21_reset_test_support import (
    DPN_VALIDATOR_RELEASE_COMMIT,
    GENESIS_EXPECTED_HASH,
    GENESIS_PUBLIC_KEY,
    MODULE,
)

GENESIS_EXPECTED_HASH_LITERAL = MODULE.validator_renderer._format_literal(
    "hash", GENESIS_EXPECTED_HASH.upper()
)


def _receipt_keypair(index: int) -> tuple[str, str, str]:
    return _support_receipt_keypair(index)


def _receipt_signer_map() -> dict[str, dict[str, object]]:
    return _support_receipt_signer_map()


def _projection_config_text() -> str:
    return _support_projection_config_text()

def _write(path: Path, body: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(body)
    path.chmod(0o600)

def _mkdir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.chmod(0o700)

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
        (
            "genesis.identity.toml",
            MODULE.canonical_genesis_identity(GENESIS_EXPECTED_HASH),
        ),
        ("genesis.json", b'{"chain":"taira"}\n'),
        ("genesis.signed.nrt", b"signed-genesis"),
        ("validator-roster.toml", b"roster\n"),
    ):
        _write(bundle / name, body)

    rendered = bundle / "rendered"
    _mkdir(rendered)
    _write(rendered / "genesis.json", (bundle / "genesis.json").read_bytes())
    config_hashes: dict[str, str] = {}
    for index, slug in enumerate(MODULE.SLUGS):
        receipt_public_key, receipt_private_key, _ = _receipt_keypair(index + 1)
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
receipt_public_key = "{receipt_public_key}"
receipt_private_key = "{receipt_private_key}"

[nexus.storage]
local_budget_bytes = {MODULE.NODE_STORAGE_BUDGET_BYTES}

[nexus.storage.disk_budget_weights]
kura_blocks_bps = 7499
wsv_snapshots_bps = 2000
sorafs_bps = 1
soranet_spool_bps = 250
soravpn_spool_bps = 250

[genesis]
file = "{bundle / "genesis.signed.nrt"}"
public_key = "{GENESIS_PUBLIC_KEY}"
expected_hash = "{GENESIS_EXPECTED_HASH_LITERAL}"
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
        "source_commit": source_commit,
        "dpn_validator_release_commit": DPN_VALIDATOR_RELEASE_COMMIT,
        "irohad_sha256": binary_sha,
        "genesis_public_key": GENESIS_PUBLIC_KEY,
        "genesis_expected_hash": GENESIS_EXPECTED_HASH,
        "genesis_identity_sha256": hashlib.sha256(
            (bundle / "genesis.identity.toml").read_bytes()
        ).hexdigest(),
        "signed_genesis_sha256": hashlib.sha256(
            (bundle / "genesis.signed.nrt").read_bytes()
        ).hexdigest(),
        "unsigned_genesis_sha256": hashlib.sha256(
            (bundle / "genesis.json").read_bytes()
        ).hexdigest(),
        "base_config_sha256": hashlib.sha256(
            (bundle / "base-config.toml").read_bytes()
        ).hexdigest(),
        "configs": config_hashes,
        "receipt_signers": _receipt_signer_map(),
        "prewarmed_storage_sha256": {
            slug: MODULE.EMPTY_TREE_SHA256 for slug in MODULE.SLUGS
        },
    }
    _write(
        bundle / "reset-manifest.json",
        (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode(),
    )
    return bundle


def _write_reset_manifest(bundle: Path, manifest: dict[str, object]) -> None:
    _write(
        bundle / "reset-manifest.json",
        (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode(),
    )


def _kagemusha_projection(release_root: Path) -> dict[str, object]:
    return {
        "schema": MODULE.KAGEMUSHA_CONFIG_PROJECTION_SCHEMA,
        "release_root": str(release_root),
        "release_policy_path": str(
            release_root / MODULE.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
        ),
        "artifact_dir": str(release_root / MODULE.KAGEMUSHA_ARTIFACT_RELATIVE_PATH),
        "catalog_qualification_seal_path": str(
            release_root / MODULE.KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH
        ),
        "max_decoded_bytes": MODULE.KAGEMUSHA_MAX_DECODED_BYTES,
    }


def _configure_kagemusha_bundle(
    bundle: Path,
    release_root: Path,
    *,
    materialize_external_release: bool,
) -> tuple[str, str]:
    policy = b"canonical Kagemusha release policy\n"
    release_manifest = b"canonical Kagemusha release manifest\n"
    policy_sha256 = hashlib.sha256(policy).hexdigest()
    release_manifest_sha256 = hashlib.sha256(release_manifest).hexdigest()
    if materialize_external_release:
        _write(
            release_root / MODULE.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH,
            policy,
        )
        release_dir = (
            release_root
            / MODULE.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
            / release_manifest_sha256
        )
        _write(release_dir / "manifest.norito", release_manifest)
        _write(
            release_dir / "manifest.norito.sha256",
            f"{release_manifest_sha256}\n".encode("ascii"),
        )
        _write(
            release_root / MODULE.KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH,
            b"canonical bounded qualification seal\n",
        )

    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    projection = _kagemusha_projection(release_root)
    for slug in MODULE.SLUGS:
        config_path = bundle / "rendered" / slug / "config.toml"
        config = config_path.read_text(encoding="utf-8") + f"""

[settlement.offline]
kagemusha_release_policy_path = "{projection['release_policy_path']}"
kagemusha_artifact_dir = "{projection['artifact_dir']}"
kagemusha_catalog_qualification_seal_path = "{projection['catalog_qualification_seal_path']}"
kagemusha_max_decoded_bytes = {projection['max_decoded_bytes']}
"""
        _write(config_path, config.encode())
        manifest["configs"][slug] = hashlib.sha256(config.encode()).hexdigest()
    manifest.update(
        {
            "kagemusha_activation_authority": "test-kagemusha-activation-authority",
            "kagemusha_config_projection": projection,
            "kagemusha_config_projection_sha256": hashlib.sha256(
                MODULE._canonical_kagemusha_config_projection_bytes(projection)
            ).hexdigest(),
            "kagemusha_release_policy_sha256": policy_sha256,
            "kagemusha_release_root": str(release_root),
        }
    )
    _write_reset_manifest(bundle, manifest)
    return policy_sha256, release_manifest_sha256


def _allow_test_owned_kagemusha_release(
    monkeypatch: pytest.MonkeyPatch,
    release_root: Path,
) -> None:
    """Exercise custody checks below a test-owned temporary trust boundary."""

    capture = MODULE._capture_root_controlled_kagemusha_paths

    def capture_test_paths(root: Path, **kwargs: object):
        assert root == release_root
        return capture(
            root,
            **kwargs,
            _trust_boundary=release_root,
            _trusted_uid=os.getuid(),
        )

    monkeypatch.setattr(
        MODULE,
        "_capture_root_controlled_kagemusha_paths",
        capture_test_paths,
    )


def _validate(bundle: Path, binary_sha: str, source_commit: str) -> MODULE.BundlePlan:
    manifest_raw = (bundle / "reset-manifest.json").read_bytes()
    return MODULE.validate_bundle(
        bundle,
        expected_reset_manifest_sha256=hashlib.sha256(manifest_raw).hexdigest(),
        expected_binary_sha256=binary_sha,
        expected_source_commit=source_commit,
        expected_dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        minimum_free_bytes=0,
        maximum_fsync_latency_ms=10_000,
    )


def test_projection_parser_extracts_all_required_fields() -> None:
    config = MODULE.parse_config_projection_text(
        _projection_config_text(),
        "validator config",
    )

    assert config["chain"] == MODULE.CHAIN_ID
    assert config["chain_discriminant"] == MODULE.CHAIN_DISCRIMINANT
    assert config["genesis"]["public_key"] == GENESIS_PUBLIC_KEY
    assert config["genesis"]["expected_hash"] == GENESIS_EXPECTED_HASH_LITERAL
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


def test_projection_parser_extracts_managed_kagemusha_fields() -> None:
    release_root = Path("/srv/iroha-kagemusha/taira-v4-r1")
    expected = _kagemusha_projection(release_root)
    text = _projection_config_text() + f"""

[settlement.offline]
kagemusha_release_policy_path = "{expected['release_policy_path']}"
kagemusha_artifact_dir = "{expected['artifact_dir']}"
kagemusha_catalog_qualification_seal_path = "{expected['catalog_qualification_seal_path']}"
kagemusha_max_decoded_bytes = {expected['max_decoded_bytes']}
"""

    config = MODULE.parse_config_projection_text(text, "validator config")

    assert config["settlement"]["offline"] == {
        "kagemusha_release_policy_path": expected["release_policy_path"],
        "kagemusha_artifact_dir": expected["artifact_dir"],
        "kagemusha_catalog_qualification_seal_path": expected[
            "catalog_qualification_seal_path"
        ],
        "kagemusha_max_decoded_bytes": expected["max_decoded_bytes"],
    }


@pytest.mark.parametrize(
    "assignment",
    (
        'kagemusha_artifact_dir = "/hidden/catalog"',
        'settlement.offline.kagemusha_artifact_dir = "/hidden/catalog"',
        '"kagemusha_artifact_dir" = "/hidden/catalog"',
        (
            '[settlement.offline]\n'
            '"kagemusha\\u005fartifact_dir" = "/hidden/catalog"'
        ),
        'settlement = { offline = { kagemusha_artifact_dir = "/hidden/catalog" } }',
    ),
)
def test_projection_parser_rejects_noncanonical_managed_kagemusha_assignment(
    assignment: str,
) -> None:
    with pytest.raises(MODULE.DeploymentError, match="managed Kagemusha"):
        MODULE.parse_config_projection_text(
            _projection_config_text() + f"\n{assignment}\n",
            "validator config",
        )


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
    assert plan.genesis_identity_file_identity == MODULE.metadata_identity(
        (bundle / "genesis.identity.toml").lstat()
    )


def test_bundle_preflight_rejects_rebound_genesis_identity(tmp_path: Path) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    identity_path = bundle / "genesis.identity.toml"
    rebound_hash = "02" * 31 + "03"
    _write(identity_path, MODULE.canonical_genesis_identity(rebound_hash))
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["genesis_identity_sha256"] = hashlib.sha256(
        identity_path.read_bytes()
    ).hexdigest()
    _write_reset_manifest(bundle, manifest)

    with pytest.raises(MODULE.DeploymentError, match="canonical paired"):
        _validate(bundle, binary_sha, source_commit)


def test_bundle_preflight_binds_kagemusha_projection_and_bounded_external_bytes(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from scripts import prepare_taira_empty_reset_bundle as reset_composer

    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    release_root = tmp_path / "kagemusha-release"
    _allow_test_owned_kagemusha_release(monkeypatch, release_root)
    _policy_sha256, manifest_sha256 = _configure_kagemusha_bundle(
        bundle,
        release_root,
        materialize_external_release=True,
    )
    second_manifest = b"second canonical Kagemusha release manifest\n"
    second_manifest_sha256 = hashlib.sha256(second_manifest).hexdigest()
    second_release_dir = (
        release_root
        / MODULE.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
        / second_manifest_sha256
    )
    _write(second_release_dir / "manifest.norito", second_manifest)
    _write(
        second_release_dir / "manifest.norito.sha256",
        f"{second_manifest_sha256}\n".encode("ascii"),
    )

    plan = _validate(bundle, binary_sha, source_commit)

    projection = _kagemusha_projection(release_root)
    assert projection == reset_composer._kagemusha_config_projection(release_root)
    assert MODULE._canonical_kagemusha_config_projection_bytes(
        projection
    ) == reset_composer.canonical_json_bytes(projection)
    expected_projection_sha256 = hashlib.sha256(
        MODULE._canonical_kagemusha_config_projection_bytes(projection)
    ).hexdigest()
    assert plan.kagemusha_config_projection_sha256 == expected_projection_sha256
    assert plan.kagemusha_external_release is not None
    external = plan.kagemusha_external_release
    assert external.bounded_material_present is True
    assert external.protected_path_identities
    expected_manifest_digests = tuple(
        sorted((manifest_sha256, second_manifest_sha256))
    )
    assert external.manifest_directory_digests == expected_manifest_digests
    expected_inventory = {
        "schema": MODULE.KAGEMUSHA_MANIFEST_DIRECTORY_INVENTORY_SCHEMA,
        "manifest_sha256": list(expected_manifest_digests),
    }
    assert external.manifest_directory_inventory_sha256 == hashlib.sha256(
        MODULE.taira_authority_client.canonical_json_bytes(expected_inventory)
    ).hexdigest()
    assert external.qualification_seal_sha256 == hashlib.sha256(
        b"canonical bounded qualification seal\n"
    ).hexdigest()
    subject = MODULE._kagemusha_authority_subject(plan)
    assert subject["config_projection_sha256"] == expected_projection_sha256
    assert subject["bounded_material_present"] is True
    assert subject["external_release_verified"] is False
    assert subject["manifest_directory_digests"] == list(
        expected_manifest_digests
    )
    artifact_names = {
        artifact.name for artifact in MODULE._kagemusha_authority_artifacts(plan)
    }
    expected_artifact_names = {
        "kagemusha/policy/release-policy-v1.norito",
        "kagemusha/seals/catalog-qualification-v1.norito",
    }
    for digest in expected_manifest_digests:
        expected_artifact_names.add(
            f"kagemusha/catalog/{digest}/manifest.norito"
        )
        expected_artifact_names.add(
            f"kagemusha/catalog/{digest}/manifest.norito.sha256"
        )
    assert artifact_names == expected_artifact_names
    preflight_fields = MODULE._kagemusha_report_fields(
        plan, exact_binary_config_verified=False
    )
    assert preflight_fields["kagemusha_external_release_material_present"] is True
    assert preflight_fields["kagemusha_external_release_verified"] is False
    assert preflight_fields["kagemusha_exact_binary_config_verified"] is False
    assert (
        preflight_fields["kagemusha_external_release_status"]
        == "blocked-exact-installed-binary-config-pending"
    )


def test_bundle_preflight_marks_unavailable_kagemusha_release_blocked(
    tmp_path: Path,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    release_root = tmp_path / "not-installed-kagemusha-release"
    _configure_kagemusha_bundle(
        bundle,
        release_root,
        materialize_external_release=False,
    )

    plan = _validate(bundle, binary_sha, source_commit)
    fields = MODULE._kagemusha_report_fields(
        plan, exact_binary_config_verified=False
    )

    assert plan.kagemusha_external_release is not None
    assert plan.kagemusha_external_release.bounded_material_present is False
    assert fields["kagemusha_external_release_verified"] is False
    assert (
        fields["kagemusha_external_release_status"]
        == "blocked-external-release-unavailable"
    )
    assert fields["kagemusha_exact_binary_config_verified"] is False


def test_bundle_preflight_rejects_hidden_kagemusha_config_without_manifest(
    tmp_path: Path,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    slug = MODULE.SLUGS[0]
    config_path = bundle / "rendered" / slug / "config.toml"
    config = config_path.read_text(encoding="utf-8") + """

[settlement.offline]
kagemusha_artifact_dir = "/hidden/catalog"
"""
    _write(config_path, config.encode())
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["configs"][slug] = hashlib.sha256(config.encode()).hexdigest()
    _write_reset_manifest(bundle, manifest)

    with pytest.raises(MODULE.DeploymentError, match="differs from the reset manifest"):
        _validate(bundle, binary_sha, source_commit)


def test_bundle_preflight_rejects_kagemusha_projection_digest_drift(
    tmp_path: Path,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    _configure_kagemusha_bundle(
        bundle,
        tmp_path / "kagemusha-release",
        materialize_external_release=False,
    )
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["kagemusha_config_projection_sha256"] = "0" * 64
    _write_reset_manifest(bundle, manifest)

    with pytest.raises(MODULE.DeploymentError, match="SHA-256 is not canonical"):
        _validate(bundle, binary_sha, source_commit)


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        ("schema", "projection is not canonical"),
        ("extra-key", "projection keys are not exact"),
        ("partial-top-level", "partial Kagemusha projection"),
    ),
)
def test_bundle_preflight_requires_exact_kagemusha_manifest_schema_and_keys(
    tmp_path: Path,
    mutation: str,
    message: str,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    _configure_kagemusha_bundle(
        bundle,
        tmp_path / "kagemusha-release",
        materialize_external_release=False,
    )
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if mutation == "schema":
        manifest["kagemusha_config_projection"]["schema"] = "wrong"
        manifest["kagemusha_config_projection_sha256"] = hashlib.sha256(
            MODULE._canonical_kagemusha_config_projection_bytes(
                manifest["kagemusha_config_projection"]
            )
        ).hexdigest()
    elif mutation == "extra-key":
        manifest["kagemusha_config_projection"]["unreviewed"] = True
        manifest["kagemusha_config_projection_sha256"] = hashlib.sha256(
            MODULE._canonical_kagemusha_config_projection_bytes(
                manifest["kagemusha_config_projection"]
            )
        ).hexdigest()
    else:
        del manifest["kagemusha_activation_authority"]
    _write_reset_manifest(bundle, manifest)

    with pytest.raises(MODULE.DeploymentError, match=message):
        _validate(bundle, binary_sha, source_commit)


def test_bundle_preflight_rejects_one_peer_kagemusha_projection_drift(
    tmp_path: Path,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    _configure_kagemusha_bundle(
        bundle,
        tmp_path / "kagemusha-release",
        materialize_external_release=False,
    )
    slug = MODULE.SLUGS[-1]
    config_path = bundle / "rendered" / slug / "config.toml"
    config = config_path.read_text(encoding="utf-8").replace(
        str(MODULE.KAGEMUSHA_MAX_DECODED_BYTES),
        str(MODULE.KAGEMUSHA_MAX_DECODED_BYTES - 1),
    )
    _write(config_path, config.encode())
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["configs"][slug] = hashlib.sha256(config.encode()).hexdigest()
    _write_reset_manifest(bundle, manifest)

    with pytest.raises(MODULE.DeploymentError, match="differs from the reset manifest"):
        _validate(bundle, binary_sha, source_commit)


def test_bundle_preflight_rejects_available_kagemusha_policy_or_manifest_drift(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    release_root = tmp_path / "kagemusha-release"
    _allow_test_owned_kagemusha_release(monkeypatch, release_root)
    _policy_sha256, manifest_sha256 = _configure_kagemusha_bundle(
        bundle,
        release_root,
        materialize_external_release=True,
    )
    policy_path = release_root / MODULE.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
    _write(policy_path, b"substituted policy\n")
    with pytest.raises(MODULE.DeploymentError, match="release policy differs"):
        _validate(bundle, binary_sha, source_commit)

    _write(policy_path, b"canonical Kagemusha release policy\n")
    sidecar = (
        release_root
        / MODULE.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
        / manifest_sha256
        / "manifest.norito.sha256"
    )
    _write(sidecar, f"{'0' * 64}\n".encode("ascii"))
    with pytest.raises(MODULE.DeploymentError, match="digest sidecar"):
        _validate(bundle, binary_sha, source_commit)


def test_bundle_preflight_rejects_writable_kagemusha_external_file(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    release_root = tmp_path / "kagemusha-release"
    _allow_test_owned_kagemusha_release(monkeypatch, release_root)
    _configure_kagemusha_bundle(
        bundle,
        release_root,
        materialize_external_release=True,
    )
    policy_path = release_root / MODULE.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
    policy_path.chmod(0o620)

    with pytest.raises(MODULE.DeploymentError, match="unsafe protected Kagemusha"):
        _validate(bundle, binary_sha, source_commit)


def test_kagemusha_external_identity_recheck_rejects_same_byte_substitution(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    release_root = tmp_path / "kagemusha-release"
    _allow_test_owned_kagemusha_release(monkeypatch, release_root)
    _configure_kagemusha_bundle(
        bundle,
        release_root,
        materialize_external_release=True,
    )
    plan = _validate(bundle, binary_sha, source_commit)
    policy_path = release_root / MODULE.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
    replacement = policy_path.with_name("replacement.norito")
    _write(replacement, policy_path.read_bytes())
    os.replace(replacement, policy_path)

    with pytest.raises(
        MODULE.DeploymentError,
        match="protected Kagemusha external release changed after preflight",
    ):
        MODULE.require_kagemusha_external_release_unchanged(
            plan,
            phase="after preflight",
        )


def test_bundle_preflight_rejects_a_config_with_an_alternate_genesis_hash(
    tmp_path: Path,
) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    slug = MODULE.SLUGS[0]
    config_path = bundle / "rendered" / slug / "config.toml"
    alternate_hash = "02" * 31 + "03"
    alternate_hash_literal = MODULE.validator_renderer._format_literal(
        "hash", alternate_hash.upper()
    )
    config = config_path.read_text().replace(
        f'expected_hash = "{GENESIS_EXPECTED_HASH_LITERAL}"',
        f'expected_hash = "{alternate_hash_literal}"',
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


def test_bundle_preflight_rejects_a_raw_unwrapped_genesis_hash(tmp_path: Path) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    slug = MODULE.SLUGS[0]
    config_path = bundle / "rendered" / slug / "config.toml"
    config = config_path.read_text().replace(
        f'expected_hash = "{GENESIS_EXPECTED_HASH_LITERAL}"',
        f'expected_hash = "{GENESIS_EXPECTED_HASH}"',
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
            expected_dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
            minimum_free_bytes=0,
            maximum_fsync_latency_ms=10_000,
        )

def test_bundle_preflight_rejects_dpn_only_identity_mismatch(tmp_path: Path) -> None:
    binary_sha = "8" * 64
    source_commit = "9" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)

    with pytest.raises(MODULE.DeploymentError, match="DPN release commit"):
        MODULE.validate_bundle(
            bundle,
            expected_reset_manifest_sha256=hashlib.sha256(
                (bundle / "reset-manifest.json").read_bytes()
            ).hexdigest(),
            expected_binary_sha256=binary_sha,
            expected_source_commit=source_commit,
            expected_dpn_validator_release_commit="e" * 40,
            minimum_free_bytes=0,
            maximum_fsync_latency_ms=10_000,
        )

def test_binary_config_gate_checks_every_peer_with_bounded_redacted_command(
    tmp_path: Path,
) -> None:
    binary = tmp_path / "iroha3d"
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
        SimpleNamespace(peers=peers, owner_uid=501, owner_gid=502),
        runner=runner,
    )

    assert MODULE.CONFIG_CHECK_TIMEOUT_SECONDS == 30
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
        and kwargs["stdout"] is MODULE.subprocess.DEVNULL
        and kwargs["stderr"] is MODULE.subprocess.DEVNULL
        and "capture_output" not in kwargs
        and kwargs["timeout"] == MODULE.CONFIG_CHECK_TIMEOUT_SECONDS
        and kwargs["env"] == {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"}
        and callable(kwargs["preexec_fn"])
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
            tmp_path / "iroha3d",
            SimpleNamespace(peers=peers, owner_uid=501, owner_gid=502),
            runner=runner,
        )

    assert calls == 2


def test_kagemusha_dry_run_readiness_requires_exact_installed_binary_semantics(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    body = b"exact installed candidate"
    digest = hashlib.sha256(body).hexdigest()
    install_root = tmp_path / "install"
    installed = install_root / "binaries" / digest / "iroha3d"
    _write(installed, body)
    installed.chmod(0o700)
    bundle = SimpleNamespace(
        kagemusha_config_projection_sha256="5" * 64,
        kagemusha_external_release=SimpleNamespace(
            bounded_material_present=True,
            manifest_directory_digests=(),
            manifest_directory_inventory_sha256=None,
            expected_policy_sha256="6" * 64,
            qualification_seal_sha256=None,
        ),
    )
    sources = SimpleNamespace(binary_sha256=digest)
    events: list[str] = []
    monkeypatch.setattr(MODULE, "INSTALL_ROOT", install_root)
    monkeypatch.setattr(
        MODULE,
        "require_root_controlled_file",
        lambda path, *, executable: path.lstat(),
    )
    monkeypatch.setattr(
        MODULE,
        "require_mutable_bundle_identities",
        lambda _bundle, *, phase: events.append(phase),
    )

    def accept(path: Path, checked_bundle: object) -> None:
        assert path == installed
        assert checked_bundle is bundle
        events.append("exact-check")

    assert MODULE.validate_dry_run_kagemusha_exact_config(
        sources,
        bundle,
        checker=accept,
    )
    fields = MODULE._kagemusha_report_fields(
        bundle,
        exact_binary_config_verified=True,
    )
    assert fields["kagemusha_external_release_material_present"] is True
    assert fields["kagemusha_exact_binary_config_verified"] is True
    assert fields["kagemusha_external_release_verified"] is True
    assert MODULE._kagemusha_authority_subject(
        bundle,
        exact_binary_config_verified=True,
    )["external_release_verified"] is True
    assert events == [
        "before exact dry-run config validation",
        "exact-check",
        "after exact dry-run config validation",
    ]

    events.clear()

    def reject(_path: Path, _bundle: object) -> None:
        events.append("semantic-reject")
        raise MODULE.DeploymentError("injected semantic rejection")

    with pytest.raises(MODULE.DeploymentError, match="semantic rejection"):
        MODULE.validate_dry_run_kagemusha_exact_config(
            sources,
            bundle,
            checker=reject,
        )
    assert events == [
        "before exact dry-run config validation",
        "semantic-reject",
    ]


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        (
            "config",
            "validator config changed after exact dry-run config validation",
        ),
        (
            "genesis",
            "signed genesis changed after exact dry-run config validation",
        ),
        (
            "runtime",
            "fresh-reset runtime path changed after exact dry-run config validation",
        ),
    ),
)
def test_dry_run_checker_cannot_mutate_bundle_before_authority_or_report(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: str,
    message: str,
) -> None:
    body = b"exact installed candidate"
    digest = hashlib.sha256(body).hexdigest()
    source_commit = "b" * 40
    bundle_root = _build_bundle(tmp_path, digest, source_commit)
    release_root = tmp_path / "kagemusha-release"
    _allow_test_owned_kagemusha_release(monkeypatch, release_root)
    _configure_kagemusha_bundle(
        bundle_root,
        release_root,
        materialize_external_release=True,
    )
    bundle = _validate(bundle_root, digest, source_commit)
    install_root = tmp_path / "install"
    installed = install_root / "binaries" / digest / "iroha3d"
    _write(installed, body)
    installed.chmod(0o700)
    sources = SimpleNamespace(binary_sha256=digest, supervisor_sha256="c" * 64)
    admission = SimpleNamespace(
        binary_sha256=digest,
        source_commit=source_commit,
        dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        restart_generation="d" * 64,
    )
    monkeypatch.setattr(MODULE, "INSTALL_ROOT", install_root)
    monkeypatch.setattr(
        MODULE,
        "require_root_controlled_file",
        lambda path, *, executable: path.lstat(),
    )

    checker_calls = 0

    def mutate_and_accept(_path: Path, checked_bundle: MODULE.BundlePlan) -> None:
        nonlocal checker_calls
        checker_calls += 1
        peer = checked_bundle.peers[0]
        if mutation == "config":
            _write(peer.config, peer.config.read_bytes() + b"\n# substituted\n")
        elif mutation == "genesis":
            _write(checked_bundle.root / "genesis.signed.nrt", b"substituted")
        else:
            peer.storage.rename(tmp_path / "displaced-storage")
            _mkdir(peer.storage)

    exact_validator = MODULE.validate_dry_run_kagemusha_exact_config
    monkeypatch.setattr(
        MODULE,
        "validate_dry_run_kagemusha_exact_config",
        lambda checked_sources, checked_bundle: exact_validator(
            checked_sources,
            checked_bundle,
            checker=mutate_and_accept,
        ),
    )
    monkeypatch.setattr(MODULE, "require_sealed_external_tool_identity", lambda: None)
    monkeypatch.setattr(MODULE, "validate_arguments", lambda _args: None)
    monkeypatch.setattr(MODULE, "verify_deployment_admission", lambda _args: admission)
    monkeypatch.setattr(MODULE, "validate_bundle", lambda *_args, **_kwargs: bundle)
    monkeypatch.setattr(MODULE, "validate_sources", lambda *_args, **_kwargs: sources)
    monkeypatch.setattr(MODULE, "require_inputs_match_admission", lambda *_args: None)
    monkeypatch.setattr(
        MODULE,
        "capture_old_cohort",
        lambda *_args, **_kwargs: pytest.fail(
            "dry-run mutation reached cohort capture and report construction"
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "_authorize_deploy_lease",
        lambda *_args, **_kwargs: pytest.fail(
            "dry-run mutation reached deploy authority"
        ),
    )
    args = SimpleNamespace(
        apply=False,
        bundle=bundle_root,
        expected_production_reset_manifest_sha256=bundle.manifest_sha256,
        minimum_free_bytes=0,
        maximum_fsync_latency_ms=10_000,
        allow_absent_old_child=False,
    )

    with pytest.raises(MODULE.DeploymentError, match=message):
        MODULE._execute_after_provisioned_authority_contracts(args)

    assert checker_calls == 1


def test_binary_config_gate_privilege_drop_clears_groups_before_uid(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[str, object]] = []
    monkeypatch.setattr(MODULE.os, "setgroups", lambda value: calls.append(("groups", value)))
    monkeypatch.setattr(MODULE.os, "setgid", lambda value: calls.append(("gid", value)))
    monkeypatch.setattr(MODULE.os, "setuid", lambda value: calls.append(("uid", value)))
    monkeypatch.setattr(MODULE.os, "umask", lambda value: calls.append(("umask", value)))

    MODULE._drop_config_check_privileges(501, 502)()

    assert calls == [
        ("groups", []),
        ("gid", 502),
        ("uid", 501),
        ("umask", 0o077),
    ]

@pytest.mark.parametrize(("uid", "gid"), ((0, 502), (501, 0), (-1, 502)))
def test_binary_config_gate_rejects_root_or_invalid_runtime_identity(
    uid: int,
    gid: int,
) -> None:
    with pytest.raises(MODULE.DeploymentError, match="non-root runtime identity"):
        MODULE._drop_config_check_privileges(uid, gid)

@pytest.mark.parametrize(
    "mutation",
    ["source", "budget", "port", "storage"],
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
    binary = tmp_path / "iroha3d"
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
        f"/Library/SORA/Taira/binaries/{sources.binary_sha256}/iroha3d"
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
        lifecycle_journal_root=runtime / "lifecycle" / bundle.peers[0].slug,
        authenticated_node_id=_receipt_keypair(1)[2],
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
    expected_terminal = runtime / "terminal" / f"validator-1-{terminal_binding}-terminal-unhealthy.json"
    assert arguments[arguments.index("--terminal-unhealthy-file") + 1] == str(expected_terminal)
    assert payload["EnvironmentVariables"]["GENESIS"] == str(bundle.root / "genesis.signed.nrt")

def test_validate_sources_uses_validated_runtime_not_controller_python(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    binary = tmp_path / "iroha3d"
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
    monkeypatch.setattr(MODULE, "require_system_python_launcher", lambda _path: stable)
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
    assert (
        MODULE.metadata_identity(
            MODULE.require_root_controlled_file(runtime, executable=True)
        )
        == identity
    )

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

    def get(url: str, _timeout: float) -> dict:
        port = int(url.split(":")[2].split("/")[0])
        index = MODULE.TORII_PORTS.index(port)
        if url.endswith("/v1/nexus/lifecycle"):
            return {
                "version": 1,
                "lane_count": MODULE.TAIRA_LANE_COUNT,
                "lanes": [
                    {
                        "id": lane_id,
                        "alias": lane_alias,
                        "dataspace_id": dataspace_id,
                    }
                    for lane_id, lane_alias, _dataspace_alias, dataspace_id in (
                        MODULE.TAIRA_LANE_DATASPACE_BINDINGS
                    )
                ],
                "catalog_hash": "hash:" + "c" * 64,
                "incarnations": [
                    {"lane_id": lane_id, "incarnation": "hash:" + f"{lane_id + 1:x}" * 64}
                    for lane_id in range(MODULE.TAIRA_LANE_COUNT)
                ],
                "incarnation_root": "hash:" + "d" * 64,
            }
        if "/v1/sumeragi/status" in url:
            subject = {"block_hash": f"hash:{block_hash.upper()}#A1b2"}
            return {
                "protocol_version": 4,
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
                "build": {
                    "dpn_validator_release_commit": DPN_VALIDATOR_RELEASE_COMMIT,
                    "git_commit_sha": source_commit,
                },
            }
        raise AssertionError(f"unexpected JSON health route: {url}")

    return get

def test_operator_http_getter_signs_each_exact_target_without_fallback(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    key_file = tmp_path / "operator.key"
    key_file.write_text("runtime-only\n", encoding="ascii")
    key_file.chmod(0o600)
    signed: list[tuple[str, str, bytes]] = []
    requests = []

    class Context:
        @staticmethod
        def headers(method: str, target: str, body: bytes) -> dict[str, str]:
            signed.append((method, target, body))
            return {
                "x-iroha-operator-public-key": "ed0120" + "11" * 32,
                "x-iroha-operator-timestamp-ms": "1800000000000",
                "x-iroha-operator-nonce": "22" * 16,
                "x-iroha-operator-signature": "33" * 64,
            }

    class Response:
        status = 200

        def __enter__(self):
            return self

        def __exit__(self, *_args) -> None:
            return None

        @staticmethod
        def read(_limit: int) -> bytes:
            return b'{"protocol_version":4}'

    class Opener:
        @staticmethod
        def open(request, *, timeout: float):
            assert timeout == 3.0
            requests.append(request)
            return Response()

    monkeypatch.setattr(
        MODULE,
        "load_operator_context_from_file",
        lambda network_id, path: (
            Context()
            if (network_id, path) == ("network-id", key_file)
            else pytest.fail("unexpected operator context")
        ),
    )
    monkeypatch.setattr(MODULE.urllib.request, "build_opener", lambda *_handlers: Opener())
    getter = MODULE.build_operator_http_getter("network-id", key_file)

    url = "https://validator.test/v1/sumeragi/status?view=2&height=1"
    assert getter(url, 3.0) == {"protocol_version": 4}
    assert getter(url, 3.0) == {"protocol_version": 4}

    assert signed == [
        ("GET", "/v1/sumeragi/status?view=2&height=1", b""),
        ("GET", "/v1/sumeragi/status?view=2&height=1", b""),
    ]
    assert len(requests) == 2
    for request in requests:
        assert request.full_url == url
        assert request.data is None
        names = {name.lower() for name, _ in request.header_items()}
        assert "authorization" not in names
        assert "x-api-token" not in names
        assert "x-iroha-operator-signature" in names
    assert MODULE._RejectRedirects().redirect_request(None, None, 302, "", {}, url) is None

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

def test_four_peer_health_requires_exact_common_status_and_dataspaces(
    tmp_path: Path,
) -> None:
    source_commit = "4" * 40
    bundle = _build_bundle(tmp_path, "5" * 64, source_commit)
    plan = _validate(bundle, "5" * 64, source_commit)

    health_urls: list[str] = []
    sample = MODULE.capture_fleet(
        plan,
        source_commit,
        DPN_VALIDATOR_RELEASE_COMMIT,
        getter=_health_getter(plan, source_commit),
        health_getter=lambda url, _timeout: health_urls.append(url),
    )
    assert sample.height == 7
    assert sample.block_hash == "ab" * 32
    assert len(sample.nodes) == MODULE.PEER_COUNT
    topology = json.loads(sample.nexus_topology)
    assert topology == {
        "observed_catalog_hash": "hash:" + "c" * 64,
        "observed_lane_count": MODULE.TAIRA_LANE_COUNT,
        "canonical_lane_bindings": [
            {
                "lane_id": lane_id,
                "lane_alias": lane_alias,
                "dataspace_id": dataspace_id,
                "dataspace_alias": dataspace_alias,
            }
            for lane_id, lane_alias, dataspace_alias, dataspace_id in (
                MODULE.TAIRA_LANE_DATASPACE_BINDINGS
            )
        ],
        "canonical_physical_dataspaces": [
            {
                "dataspace_id": dataspace_id,
                "dataspace_alias": dataspace_alias,
            }
            for dataspace_alias, dataspace_id in MODULE.TAIRA_PHYSICAL_DATASPACES
        ],
    }
    assert health_urls == [
        url
        for port in MODULE.TORII_PORTS
        for url in (
            f"http://127.0.0.1:{port}/health",
            f"http://127.0.0.1:{port}/readyz",
        )
    ]

    with pytest.raises(MODULE.DeploymentError, match="status.blocks"):
        MODULE.capture_fleet(
            plan,
            source_commit,
            DPN_VALIDATOR_RELEASE_COMMIT,
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
            DPN_VALIDATOR_RELEASE_COMMIT,
            getter=_health_getter(plan, source_commit),
            health_getter=unhealthy,
        )

def test_four_peer_health_rejects_dpn_only_runtime_mismatch(tmp_path: Path) -> None:
    source_commit = "4" * 40
    bundle = _build_bundle(tmp_path, "5" * 64, source_commit)
    plan = _validate(bundle, "5" * 64, source_commit)
    healthy = _health_getter(plan, source_commit)

    def wrong_dpn(url: str, timeout: float) -> dict:
        payload = copy.deepcopy(healthy(url, timeout))
        if url.endswith("/status"):
            payload["build"]["dpn_validator_release_commit"] = "e" * 40
        return payload

    with pytest.raises(MODULE.DeploymentError, match="wrong DPN validator"):
        MODULE.capture_fleet(
            plan,
            source_commit,
            DPN_VALIDATOR_RELEASE_COMMIT,
            getter=wrong_dpn,
            health_getter=lambda _url, _timeout: None,
        )

def test_four_peer_health_requires_exact_seven_lane_five_dataspace_topology(
    tmp_path: Path,
) -> None:
    source_commit = "4" * 40
    bundle = _build_bundle(tmp_path, "5" * 64, source_commit)
    plan = _validate(bundle, "5" * 64, source_commit)
    healthy = _health_getter(plan, source_commit)

    def wrong_dataspace(url: str, timeout: float) -> dict:
        payload = copy.deepcopy(healthy(url, timeout))
        if url.endswith("/v1/nexus/lifecycle"):
            payload["lanes"][4]["dataspace_id"] = 9
        return payload

    with pytest.raises(
        MODULE.DeploymentError,
        match="exact canonical seven-lane/five-dataspace topology",
    ):
        MODULE.capture_fleet(
            plan,
            source_commit,
            DPN_VALIDATOR_RELEASE_COMMIT,
            getter=wrong_dataspace,
            health_getter=lambda _url, _timeout: None,
        )

@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        pytest.param(
            lambda lifecycle: lifecycle["lanes"].pop(),
            "exactly seven lanes",
            id="missing-lane",
        ),
        pytest.param(
            lambda lifecycle: lifecycle["lanes"].append(
                {"id": 7, "alias": "extra", "dataspace_id": 0}
            ),
            "exactly seven lanes",
            id="extra-lane",
        ),
        pytest.param(
            lambda lifecycle: lifecycle["lanes"][1].update({"id": 0}),
            "duplicates a lane id or alias",
            id="duplicate-id",
        ),
        pytest.param(
            lambda lifecycle: lifecycle["lanes"][1].update({"alias": "core"}),
            "duplicates a lane id or alias",
            id="duplicate-alias",
        ),
        pytest.param(
            lambda lifecycle: lifecycle["lanes"][1].update({"alias": "council"}),
            "exact canonical seven-lane/five-dataspace topology",
            id="wrong-alias",
        ),
        pytest.param(
            lambda lifecycle: lifecycle["lanes"][1].pop("alias"),
            "invalid alias",
            id="missing-alias",
        ),
        pytest.param(
            lambda lifecycle: lifecycle["lanes"][3].update({"dataspace_id": 0}),
            "exact canonical seven-lane/five-dataspace topology",
            id="wrong-dataspace-id",
        ),
        pytest.param(
            lambda lifecycle: lifecycle["lanes"][3].pop("dataspace_id"),
            "invalid dataspace id",
            id="missing-dataspace-id",
        ),
        pytest.param(
            lambda lifecycle: lifecycle["lanes"][6].update({"id": 8}),
            "exact canonical seven-lane/five-dataspace topology",
            id="wrong-lane-id",
        ),
        pytest.param(
            lambda lifecycle: lifecycle.update({"lane_count": 8}),
            "lane_count is not exactly 7",
            id="wrong-lane-count",
        ),
    ],
)
def test_four_peer_health_rejects_noncanonical_lane_bindings(
    tmp_path: Path,
    mutation,
    message: str,
) -> None:
    source_commit = "4" * 40
    bundle = _build_bundle(tmp_path, "5" * 64, source_commit)
    plan = _validate(bundle, "5" * 64, source_commit)
    healthy = _health_getter(plan, source_commit)

    def malformed_topology(url: str, timeout: float) -> dict:
        payload = copy.deepcopy(healthy(url, timeout))
        if url.endswith("/v1/nexus/lifecycle"):
            mutation(payload)
        return payload

    with pytest.raises(MODULE.DeploymentError, match=message):
        MODULE.capture_fleet(
            plan,
            source_commit,
            DPN_VALIDATOR_RELEASE_COMMIT,
            getter=malformed_topology,
            health_getter=lambda _url, _timeout: None,
        )

@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("protocol_version",), 3),
        (("height_context", "validator_count"), 1),
        (("last_commit_qc", "signer_count"), 2), (("last_commit_qc", "signer_count"), 4),
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
            DPN_VALIDATOR_RELEASE_COMMIT,
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
            DPN_VALIDATOR_RELEASE_COMMIT,
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
            DPN_VALIDATOR_RELEASE_COMMIT,
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
        DPN_VALIDATOR_RELEASE_COMMIT,
        tmp_path,
        {peer.label: b"plist"},
        Path("/iroha3d"),
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
            DPN_VALIDATOR_RELEASE_COMMIT,
            tmp_path,
            {peer.label: b"plist"},
            Path("/iroha3d"),
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
            DPN_VALIDATOR_RELEASE_COMMIT,
            tmp_path,
            {peer.label: b"plist"},
            Path("/iroha3d"),
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
            DPN_VALIDATOR_RELEASE_COMMIT,
            Path("/runtime"),
            {},
            Path("/iroha3d"),
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
    monkeypatch.setattr(MODULE, "read_darwin_process_argv", lambda _pid: next(samples))

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
        "/old/iroha3d",
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


def test_old_supervisor_requires_exact_program_arguments(
    tmp_path: Path,
) -> None:
    payload, plist_argv = _old_capture_payload(tmp_path / "absent.pid")
    runtime_argv = ("/different/python3", *plist_argv[1:])
    ops = _OldCaptureOps(46, runtime_argv)

    with pytest.raises(MODULE.DeploymentError, match="differs from its plist"):
        MODULE.inspect_old_managed_identity(
            payload,
            "old-job",
            46,
            ops,
            allow_absent_child=True,
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
    ops.child_pids = lambda parent_pid: next(child_samples) if parent_pid == 45 else ()

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


@pytest.mark.parametrize(
    ("material_present", "expected_mode", "expected_status"),
    (
        (
            False,
            "blocked-kagemusha-external-release-dry-run",
            "blocked-external-release-unavailable",
        ),
        (
            True,
            "blocked-kagemusha-semantic-validation-dry-run",
            "blocked-exact-installed-binary-config-pending",
        ),
    ),
)
def test_dry_run_execute_never_calls_apply(
    monkeypatch: pytest.MonkeyPatch,
    material_present: bool,
    expected_mode: str,
    expected_status: str,
) -> None:
    events: list[str] = []
    admission = SimpleNamespace(
        archive_sha256="0" * 64,
        boi_artifact_inventory_sha256="2" * 64,
        boi_qualified_inventory_sha256="3" * 64,
        boi_qualification_receipt_id="4" * 64,
        receipt_id="f" * 64,
        reset_manifest_sha256="1" * 64,
        binary_sha256="a" * 64,
        supervisor_sha256="b" * 64,
        source_commit="c" * 40,
        dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        restart_generation="9" * 64,
    )
    bundle = SimpleNamespace(
        root=Path("/bundle"),
        bundle_bytes=1,
        free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        fsync_latency_ms=1.0,
        kagemusha_config_projection_sha256="5" * 64,
        kagemusha_external_release=SimpleNamespace(
            bounded_material_present=material_present,
            manifest_directory_inventory_sha256=None,
            qualification_seal_sha256=None,
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
        lambda _args: events.append("admission-verify") or admission,
    )
    monkeypatch.setattr(MODULE, "require_inputs_match_admission", lambda *args: None)
    monkeypatch.setattr(
        MODULE,
        "validate_dry_run_kagemusha_exact_config",
        lambda *_args: False,
    )
    monkeypatch.setattr(
        MODULE,
        "require_mutable_bundle_identities",
        lambda *_args, phase: events.append(f"bundle-recheck:{phase}"),
    )
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
        lambda _ops, *, allow_absent_child: events.append("capture") or cohort,
    )
    monkeypatch.setattr(
        MODULE,
        "apply_reset",
        lambda *args, **kwargs: pytest.fail("dry run called apply_reset"),
    )
    dry_run_authority = MODULE.taira_authority_client.AuthorityResult(
        role="deploy-issuance",
        operation_id="7" * 64,
        run_id="8" * 64,
        status="verified",
        authority_envelope={},
        durable_receipt={},
    )
    monkeypatch.setattr(
        MODULE,
        "_authorize_deploy_lease",
        lambda *_args, apply, **_kwargs: (
            events.append(f"authority:{apply}") or dry_run_authority
        ),
    )
    monkeypatch.setattr(
        MODULE.taira_authority_client,
        "verify_receipt",
        lambda *_args, **_kwargs: pytest.fail("dry run historically verified a lease"),
    )
    monkeypatch.setattr(
        MODULE,
        "_finalize_deploy_lease",
        lambda *_args, **_kwargs: pytest.fail("dry run finalized a lease"),
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
        expected_dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        expected_cargo_lock_sha256="d" * 64,
        expected_workspace_source_manifest_sha256="e" * 64,
        expected_receipt_id="f" * 64,
        expected_artifact_handoff_sha256="9" * 64,
        expected_production_reset_manifest_sha256="a" * 64,
        trusted_signing_fingerprint="1" * 64,
        trusted_boi_qualification_signing_fingerprint="3" * 64,
        release_manifest_verifier=Path("/sorafs-validate"),
        trusted_release_manifest_verifier_sha256="2" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        allow_absent_old_child=False,
        apply=False,
    )

    report = MODULE._execute_after_provisioned_authority_contracts(
        args, ops=MODULE.SystemOps()
    )
    assert report["mode"] == expected_mode
    assert report["deployment_ready"] is False
    assert report["kagemusha_config_projection_sha256"] == "5" * 64
    assert report["kagemusha_external_release_material_present"] is material_present
    assert report["kagemusha_external_release_verified"] is False
    assert report["kagemusha_external_release_status"] == expected_status
    assert report["applied"] is False
    assert report["admission_receipt_consumed"] is False
    assert report["boi_artifact_inventory_sha256"] == "2" * 64
    assert report["boi_qualified_inventory_sha256"] == "3" * 64
    assert report["boi_qualification_receipt_id"] == "4" * 64
    assert events == [
        "admission-verify",
        "capture",
        "archive-recheck",
        "bundle-recheck:immediately before dry-run authority",
        "authority:False",
        "bundle-recheck:immediately after dry-run authority",
    ]


def test_apply_rejects_missing_kagemusha_material_before_authority_or_receipt(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    events: list[str] = []
    admission = SimpleNamespace(
        binary_sha256="a" * 64,
        supervisor_sha256="b" * 64,
        source_commit="c" * 40,
        dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        restart_generation="9" * 64,
    )
    bundle = SimpleNamespace(
        kagemusha_config_projection_sha256="5" * 64,
        kagemusha_external_release=SimpleNamespace(
            bounded_material_present=False,
        ),
    )
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_UID_ENV, "41")
    monkeypatch.setenv(MODULE.EXTERNAL_TOOL_GID_ENV, "42")
    monkeypatch.setattr(
        MODULE,
        "verify_deployment_admission",
        lambda _args: events.append("admission") or admission,
    )
    monkeypatch.setattr(MODULE, "validate_bundle", lambda *_args, **_kwargs: bundle)
    monkeypatch.setattr(
        MODULE,
        "validate_sources",
        lambda *_args, **_kwargs: SimpleNamespace(),
    )
    monkeypatch.setattr(MODULE, "require_inputs_match_admission", lambda *_args: None)
    monkeypatch.setattr(
        MODULE,
        "_authorize_deploy_lease",
        lambda *_args, **_kwargs: pytest.fail("apply reached deploy authority"),
    )
    monkeypatch.setattr(
        MODULE,
        "consume_admission_receipt",
        lambda *_args: pytest.fail("apply reached receipt consumption"),
    )
    monkeypatch.setattr(
        MODULE,
        "exclusive_deployment_lock",
        lambda: pytest.fail("blocked apply acquired deployment lock"),
    )
    args = argparse.Namespace(
        bundle=Path("/bundle"),
        binary=Path("/binary"),
        supervisor=Path("/supervisor"),
        admission_archive=Path("/candidate.tar.gz"),
        admission_authority_dir=Path("/authority"),
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
        expected_source_commit="c" * 40,
        expected_dpn_validator_release_commit=DPN_VALIDATOR_RELEASE_COMMIT,
        expected_cargo_lock_sha256="d" * 64,
        expected_workspace_source_manifest_sha256="e" * 64,
        expected_receipt_id="f" * 64,
        expected_artifact_handoff_sha256="9" * 64,
        expected_production_reset_manifest_sha256="a" * 64,
        trusted_signing_fingerprint="1" * 64,
        trusted_boi_qualification_signing_fingerprint="3" * 64,
        release_manifest_verifier=Path("/sorafs-validate"),
        trusted_release_manifest_verifier_sha256="2" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        allow_absent_old_child=False,
        operator_network_id="taira",
        operator_private_key_file=Path("/operator.key"),
        apply=True,
    )

    with pytest.raises(
        MODULE.DeploymentError,
        match="requires protected bounded external release material",
    ):
        MODULE._execute_after_provisioned_authority_contracts(
            args,
            ops=MODULE.SystemOps(),
        )
    assert events == ["admission"]


pytest.register_assert_rewrite(
    "scripts.tests.deploy_taira_v21_reset_test_components"
)
from scripts.tests.deploy_taira_v21_reset_test_components import (
    EXPORTED_TESTS as _EXPORTED_TESTS,
    _receipt_transaction_plan,
)

for _test in _EXPORTED_TESTS:
    _test.__module__ = __name__
    globals()[_test.__name__] = _test
del _test, _EXPORTED_TESTS
