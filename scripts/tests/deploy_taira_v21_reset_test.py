"""Focused tests for the guarded Taira v21 fresh-reset controller."""

from __future__ import annotations

import argparse
import contextlib
import copy
import dataclasses
import hashlib
import importlib.util
import json
import os
import plistlib
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


def _write(path: Path, body: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(body)
    path.chmod(0o600)


def _mkdir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.chmod(0o700)


def _tree_sha(root: Path) -> str:
    return MODULE.release_tree_sha256(root, os.getuid(), os.getgid())


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
        config = f'''chain = "{MODULE.CHAIN_ID}"
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

[genesis]
file = "{bundle / 'genesis.signed.nrt'}"
'''
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
    manifest = json.loads((bundle / "reset-manifest.json").read_text())
    return MODULE.validate_bundle(
        bundle,
        expected_binary_sha256=binary_sha,
        expected_source_commit=source_commit,
        expected_kagemusha_manifest_sha256=manifest["kagemusha_manifest_sha256"],
        expected_kagemusha_release_policy_sha256=manifest[
            "kagemusha_release_policy_sha256"
        ],
        expected_kagemusha_release_attestation_sha256=manifest[
            "kagemusha_release_attestation_sha256"
        ],
        minimum_free_bytes=0,
        maximum_fsync_latency_ms=10_000,
    )


def test_bundle_preflight_authenticates_exact_four_peer_reset(tmp_path: Path) -> None:
    binary_sha = "a" * 64
    source_commit = "b" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)

    plan = _validate(bundle, binary_sha, source_commit)

    assert plan.manifest["nexus_storage_budget_policy"] == MODULE.NODE_STORAGE_BUDGET_POLICY
    assert [peer.torii_port for peer in plan.peers] == list(MODULE.TORII_PORTS)
    assert [peer.p2p_port for peer in plan.peers] == list(MODULE.P2P_PORTS)
    assert all(not any(peer.storage.iterdir()) for peer in plan.peers)


@pytest.mark.parametrize("pin", ["manifest", "policy", "attestation"])
def test_bundle_preflight_requires_operator_pinned_release_digests(
    tmp_path: Path, pin: str
) -> None:
    binary_sha = "8" * 64
    source_commit = "9" * 40
    bundle = _build_bundle(tmp_path, binary_sha, source_commit)
    manifest = json.loads((bundle / "reset-manifest.json").read_text())
    pins = {
        "manifest": manifest["kagemusha_manifest_sha256"],
        "policy": manifest["kagemusha_release_policy_sha256"],
        "attestation": manifest["kagemusha_release_attestation_sha256"],
    }
    pins[pin] = "0" * 64

    with pytest.raises(MODULE.DeploymentError):
        MODULE.validate_bundle(
            bundle,
            expected_binary_sha256=binary_sha,
            expected_source_commit=source_commit,
            expected_kagemusha_manifest_sha256=pins["manifest"],
            expected_kagemusha_release_policy_sha256=pins["policy"],
            expected_kagemusha_release_attestation_sha256=pins["attestation"],
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

    MODULE.move_release_to_root_store(bundle)

    assert not release.source_root.exists()
    assert (destination / relative).stat().st_ino == original_inode
    assert list(tmp_path.rglob(source_file.name)) == [destination / relative]

    MODULE.restore_release_to_bundle(bundle)
    assert not destination.exists()
    assert (release.source_root / relative).stat().st_ino == original_inode


@pytest.mark.parametrize("mutation", ["source", "budget", "port", "storage"])
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
        manifest["configs"][MODULE.SLUGS[0]] = hashlib.sha256(config.read_bytes()).hexdigest()
        _write(manifest_path, (json.dumps(manifest) + "\n").encode())
    else:
        _write(bundle / "rendered" / MODULE.SLUGS[0] / "storage" / "stale", b"block")

    with pytest.raises(MODULE.DeploymentError):
        _validate(bundle, binary_sha, source_commit)


def _fake_plan(tmp_path: Path) -> tuple[MODULE.BundlePlan, MODULE.SourcePlan, os.stat_result]:
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
    )
    return bundle, sources, binary.lstat()


def test_fresh_plist_has_all_five_binary_stat_seals_and_known_paths(tmp_path: Path) -> None:
    bundle, sources, binary_info = _fake_plan(tmp_path)
    runtime = tmp_path / "runtime"
    installed_binary = Path(f"/Library/SORA/Taira/binaries/{sources.binary_sha256}/irohad")
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
    )
    payload = plistlib.loads(body)
    arguments = payload["ProgramArguments"]

    assert payload["Label"] == MODULE.LABELS[0]
    assert payload["UserName"] == bundle.runtime_user
    assert arguments[:2] == [str(sources.python), str(installed_supervisor)]
    for field in (
        "--binary-device",
        "--binary-inode",
        "--binary-size",
        "--binary-mtime-ns",
        "--binary-ctime-ns",
    ):
        assert arguments.count(field) == 1
    assert arguments[arguments.index("--config") + 1] == str(bundle.peers[0].config)
    assert payload["EnvironmentVariables"]["GENESIS"] == str(
        bundle.root / "genesis.signed.nrt"
    )


def test_validate_sources_uses_system_launcher_not_controller_python(
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
        expected_binary_sha256=binary_sha,
        supervisor=supervisor,
        expected_supervisor_sha256=supervisor_sha,
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
    )
    monkeypatch.setattr(
        MODULE.sys,
        "executable",
        "/opt/homebrew/Cellar/python@3.14/3.14.4/bin/python3.14",
    )
    monkeypatch.setattr(MODULE, "require_root_controlled_file", lambda *args, **kwargs: None)
    monkeypatch.setattr(
        MODULE,
        "validate_supervisor_python",
        lambda path: MODULE.DEFAULT_SUPERVISOR_PYTHON,
    )

    sources = MODULE.validate_sources(args, bundle)

    assert sources.python == Path("/usr/bin/python3")
    assert str(sources.python) != MODULE.sys.executable


@pytest.mark.parametrize(
    ("returncode", "stdout"),
    [(1, ""), (0, "3.8.19\n"), (0, "not-a-version\n"), (0, "4.0.0\n")],
)
def test_supervisor_python_probe_fails_closed(
    monkeypatch: pytest.MonkeyPatch, returncode: int, stdout: str
) -> None:
    monkeypatch.setattr(MODULE, "canonical_path", lambda path, _label: path)
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
    monkeypatch.setattr(MODULE, "canonical_path", lambda path, _label: path)
    monkeypatch.setattr(
        MODULE, "require_root_controlled_file", lambda *args, **kwargs: None
    )
    monkeypatch.setattr(
        MODULE.subprocess,
        "run",
        lambda *args, **kwargs: SimpleNamespace(returncode=0, stdout="3.9.6\n"),
    )

    assert (
        MODULE.validate_supervisor_python(MODULE.DEFAULT_SUPERVISOR_PYTHON)
        == MODULE.DEFAULT_SUPERVISOR_PYTHON
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


def _health_getter(bundle: MODULE.BundlePlan, source_commit: str, *, bad_blocks: bool = False):
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
            subject = {"block_hash": f"hash:{block_hash.upper()}"}
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
            "evaluated_block_hash": block_hash,
            "artifact_set": artifact,
        }

    return get


def test_four_peer_health_requires_exact_common_status_and_offline_block(tmp_path: Path) -> None:
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


def test_dry_run_execute_never_calls_apply(monkeypatch: pytest.MonkeyPatch) -> None:
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
    cohort = tuple(object() for _ in range(MODULE.PEER_COUNT))
    monkeypatch.setattr(MODULE, "validate_bundle", lambda *args, **kwargs: bundle)
    monkeypatch.setattr(MODULE, "validate_sources", lambda *args, **kwargs: sources)
    monkeypatch.setattr(MODULE, "capture_old_cohort", lambda _ops: cohort)
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
        expected_binary_sha256="a" * 64,
        supervisor=Path("/supervisor"),
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
        expected_supervisor_sha256="b" * 64,
        expected_source_commit="c" * 40,
        expected_kagemusha_manifest_sha256="d" * 64,
        expected_kagemusha_release_policy_sha256="e" * 64,
        expected_kagemusha_release_attestation_sha256="f" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        apply=False,
    )

    report = MODULE.execute(args, ops=MODULE.SystemOps())
    assert report["mode"] == "read-only-dry-run"
    assert report["applied"] is False


def test_apply_lock_spans_old_cohort_capture_and_rollout(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    events: list[str] = []
    bundle = SimpleNamespace()
    sources = SimpleNamespace()
    cohort = tuple(object() for _ in range(MODULE.PEER_COUNT))
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    monkeypatch.setattr(MODULE, "validate_bundle", lambda *args, **kwargs: bundle)
    monkeypatch.setattr(MODULE, "validate_sources", lambda *args, **kwargs: sources)
    monkeypatch.setattr(
        MODULE,
        "capture_old_cohort",
        lambda _ops: (events.append("capture") or cohort),
    )
    monkeypatch.setattr(
        MODULE,
        "apply_reset",
        lambda *args, **kwargs: (events.append("apply") or {"applied": True}),
    )

    @contextlib.contextmanager
    def lock():
        events.append("lock-enter")
        try:
            yield
        finally:
            events.append("lock-exit")

    monkeypatch.setattr(MODULE, "exclusive_deployment_lock", lock)
    args = argparse.Namespace(
        bundle=Path("/bundle"),
        binary=Path("/binary"),
        expected_binary_sha256="a" * 64,
        supervisor=Path("/supervisor"),
        supervisor_python=MODULE.DEFAULT_SUPERVISOR_PYTHON,
        expected_supervisor_sha256="b" * 64,
        expected_source_commit="c" * 40,
        expected_kagemusha_manifest_sha256="d" * 64,
        expected_kagemusha_release_policy_sha256="e" * 64,
        expected_kagemusha_release_attestation_sha256="f" * 64,
        health_timeout_seconds=240,
        minimum_free_bytes=MODULE.DEFAULT_MINIMUM_FREE_BYTES,
        maximum_fsync_latency_ms=250,
        apply=True,
    )

    assert MODULE.execute(args, ops=MODULE.SystemOps()) == {"applied": True}
    assert events == ["lock-enter", "capture", "apply", "lock-exit"]


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
    monkeypatch.setattr(
        MODULE, "existing_ancestor", lambda path: roots[path]
    )
    monkeypatch.setattr(
        MODULE.shutil,
        "disk_usage",
        lambda path: SimpleNamespace(
            free=20_000 if path is first else 9_999
        ),
    )

    with pytest.raises(MODULE.DeploymentError, match="device 22"):
        MODULE.require_filesystem_headroom(
            [Path("/first"), Path("/second")], 10_000
        )


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
            snapshot.path.stem: 40 + index
            for index, snapshot in enumerate(snapshots)
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
            f"\tpid = {self.supervisor_pids[label]}\n"
            if label in self.loaded
            else None
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
            "--expected-binary-sha256",
            "a" * 64,
            "--supervisor",
            "/supervisor",
            "--expected-supervisor-sha256",
            "b" * 64,
            "--expected-source-commit",
            "c" * 40,
            "--expected-kagemusha-manifest-sha256",
            "d" * 64,
            "--expected-kagemusha-release-policy-sha256",
            "e" * 64,
            "--expected-kagemusha-release-attestation-sha256",
            "f" * 64,
        ]
    )
    assert args.health_timeout_seconds == 240
    assert args.minimum_free_bytes == 17_179_869_184
    assert args.maximum_fsync_latency_ms == 250
    assert args.supervisor_python == MODULE.DEFAULT_SUPERVISOR_PYTHON
    assert args.apply is False
