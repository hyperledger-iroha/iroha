"""Regression and adversarial tests for signed Taira privacy resets."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import pytest

from scripts import prepare_taira_empty_reset_bundle as reset_bundle

SCRIPT = Path(reset_bundle.__file__).resolve()
DPN_COMMIT = "12" * 20
KAGEMUSHA_ACTIVATION_AUTHORITY = (
    "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
)


def test_isolated_cli_loads_only_its_trusted_sibling_modules(
    tmp_path: Path,
) -> None:
    result = subprocess.run(
        [sys.executable, "-I", "-S", str(SCRIPT), "--help"],
        cwd=tmp_path,
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "--genesis-external-signer" in result.stdout
    assert "--trusted-genesis-external-signer-sha256" in result.stdout
    assert "genesis-private-key" not in result.stdout
    assert "--kagami" not in result.stdout
    assert "--onboarding-token-hash-tool" in result.stdout
    assert "--source-bundle-sha256" in result.stdout
    assert "--kagemusha-release-root" in result.stdout
    assert "--kagemusha-activation-authority" in result.stdout


class TairaResetFreeSpaceTests(unittest.TestCase):
    """Exercise the fail-closed free-space guard before reset materialization."""

    def test_accepts_filesystem_at_or_above_required_free_space(self) -> None:
        with mock.patch.object(
            reset_bundle.shutil,
            "disk_usage",
            return_value=SimpleNamespace(free=16_384),
        ) as disk_usage:
            self.assertEqual(
                reset_bundle.require_minimum_free_space(Path("/sealed"), 16_384),
                16_384,
            )
        disk_usage.assert_called_once_with(Path("/sealed"))

    def test_rejects_filesystem_below_required_free_space(self) -> None:
        with (
            mock.patch.object(
                reset_bundle.shutil,
                "disk_usage",
                return_value=SimpleNamespace(free=16_383),
            ),
            self.assertRaisesRegex(
                RuntimeError,
                "16383 bytes available, 16384 required",
            ),
        ):
            reset_bundle.require_minimum_free_space(Path("/sealed"), 16_384)

    def test_rejects_negative_required_free_space(self) -> None:
        with self.assertRaisesRegex(
            RuntimeError, "minimum free bytes must be non-negative"
        ):
            reset_bundle.require_minimum_free_space(Path("/sealed"), -1)


class TairaResetIdentityTests(unittest.TestCase):
    """Exercise self-contained config retargeting and artifact identity checks."""

    def test_accepts_only_lowercase_sha256(self) -> None:
        digest = "ab" * 32
        self.assertEqual(
            reset_bundle.require_sha256(digest, "artifact"),
            digest,
        )
        with self.assertRaisesRegex(RuntimeError, "must be a lowercase SHA-256 digest"):
            reset_bundle.require_sha256(digest.upper(), "artifact")

    def test_accepts_only_nonzero_lowercase_source_commit(self) -> None:
        commit = "ab" * 20
        self.assertEqual(reset_bundle.require_source_commit(commit), commit)
        for rejected in (commit.upper(), "0" * 40, commit[:-1], f"{commit}0"):
            with (
                self.subTest(rejected=rejected),
                self.assertRaisesRegex(
                    RuntimeError,
                    "source commit must be a nonzero lowercase Git object id",
                ),
            ):
                reset_bundle.require_source_commit(rejected)


def test_kagemusha_activation_authority_requires_both_effective_genesis_grants() -> None:
    def permission(operation: str, name: str) -> dict[str, object]:
        return {
            operation: {
                "Permission": {
                    "destination": KAGEMUSHA_ACTIVATION_AUTHORITY,
                    "object": {"name": name},
                }
            }
        }

    required = sorted(reset_bundle.KAGEMUSHA_IMMUTABLE_ACTIVATION_PERMISSIONS)
    accepted = reset_bundle.canonical_json_bytes(
        {
            "transactions": [
                {
                    "instructions": [
                        permission("Grant", required[0]),
                        permission("Grant", required[1]),
                    ]
                }
            ]
        }
    )
    assert (
        reset_bundle._require_kagemusha_activation_authority_permissions(
            accepted, KAGEMUSHA_ACTIVATION_AUTHORITY
        )
        == KAGEMUSHA_ACTIVATION_AUTHORITY
    )

    revoked = reset_bundle.canonical_json_bytes(
        {
            "transactions": [
                {
                    "instructions": [
                        permission("Grant", required[0]),
                        permission("Grant", required[1]),
                        permission("Revoke", required[0]),
                    ]
                }
            ]
        }
    )
    with pytest.raises(RuntimeError, match=required[0]):
        reset_bundle._require_kagemusha_activation_authority_permissions(
            revoked, KAGEMUSHA_ACTIVATION_AUTHORITY
        )

    with pytest.raises(RuntimeError, match="activation-authority"):
        reset_bundle._require_kagemusha_activation_authority_permissions(
            accepted, None
        )


def test_kagemusha_genesis_staging_requires_policy_but_no_catalog_or_seal(
    tmp_path: Path,
) -> None:
    release_root = tmp_path / "kagemusha-release"
    _mkdir_private(release_root / "policy")
    policy = (
        release_root
        / reset_bundle.renderer.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
    )
    _write_private(policy, b"reviewed release policy")
    assert reset_bundle._kagemusha_release_policy_sha256(
        release_root
    ) == hashlib.sha256(b"reviewed release policy").hexdigest()

    artifact_dir = release_root / reset_bundle.renderer.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
    _mkdir_private(artifact_dir)
    with pytest.raises(RuntimeError, match="artifact directory must not exist"):
        reset_bundle._kagemusha_release_policy_sha256(release_root)
    artifact_dir.rmdir()

    seal = release_root / reset_bundle.renderer.KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH
    _write_private(seal, b"stale seal")
    with pytest.raises(RuntimeError, match="qualification seal must not exist"):
        reset_bundle._kagemusha_release_policy_sha256(release_root)


def test_rendered_kagemusha_projection_is_identical_across_exact_four_peers(
    tmp_path: Path,
) -> None:
    output = tmp_path / "reset"
    release_root = Path("/srv/iroha-kagemusha/taira-v4-r1")
    _fake_renderer(
        tmp_path / "base-config.toml",
        tmp_path / "validator-roster.toml",
        output / "rendered",
        base_genesis_path=None,
        kagemusha_release_root=release_root,
    )

    projection = reset_bundle._require_rendered_kagemusha_config_projection(
        output,
        release_root,
        include_qualification_seal=True,
    )

    assert projection == reset_bundle._kagemusha_config_projection(release_root)
    assert reset_bundle._kagemusha_config_projection_sha256(
        release_root
    ) == hashlib.sha256(
        reset_bundle.canonical_json_bytes(projection)
    ).hexdigest()

    drifted = output / "rendered" / reset_bundle.SLUGS[-1] / "config.toml"
    drifted.write_text(
        drifted.read_text(encoding="utf-8").replace(
            str(reset_bundle.renderer.KAGEMUSHA_MAX_DECODED_BYTES),
            str(reset_bundle.renderer.KAGEMUSHA_MAX_DECODED_BYTES - 1),
        ),
        encoding="utf-8",
    )
    with pytest.raises(RuntimeError, match=reset_bundle.SLUGS[-1]):
        reset_bundle._require_rendered_kagemusha_config_projection(
            output,
            release_root,
            include_qualification_seal=True,
        )


def test_rendered_kagemusha_projection_distinguishes_staging_and_disabled_modes(
    tmp_path: Path,
) -> None:
    release_root = Path("/srv/iroha-kagemusha/taira-v4-r1")
    staged = tmp_path / "staged"
    _fake_renderer(
        tmp_path / "base-config.toml",
        tmp_path / "validator-roster.toml",
        staged / "rendered",
        base_genesis_path=None,
        kagemusha_release_root=release_root,
        include_kagemusha_qualification_seal=False,
    )
    assert reset_bundle._require_rendered_kagemusha_config_projection(
        staged,
        release_root,
        include_qualification_seal=False,
    ) == reset_bundle._kagemusha_config_projection(release_root)
    with pytest.raises(RuntimeError, match=reset_bundle.SLUGS[0]):
        reset_bundle._require_rendered_kagemusha_config_projection(
            staged,
            release_root,
            include_qualification_seal=True,
        )

    disabled = tmp_path / "disabled"
    _fake_renderer(
        tmp_path / "base-config.toml",
        tmp_path / "validator-roster.toml",
        disabled / "rendered",
        base_genesis_path=None,
    )
    config = disabled / "rendered" / reset_bundle.SLUGS[2] / "config.toml"
    config.write_text(
        config.read_text(encoding="utf-8")
        + "\n[settlement.offline]\n"
        + 'kagemusha_artifact_dir = "/unreviewed/catalog"\n',
        encoding="utf-8",
    )
    with pytest.raises(RuntimeError, match=reset_bundle.SLUGS[2]):
        reset_bundle._require_rendered_kagemusha_config_projection(
            disabled,
            None,
            include_qualification_seal=False,
        )


def _write_private(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(payload)
    path.chmod(0o600)


def _mkdir_private(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.chmod(0o700)


def _privacy_release(
    root: Path,
    *,
    source_commit: str,
    dpn_validator_release_commit: str,
    cargo_lock_sha256: str,
    workspace_sha256: str,
) -> dict[str, bytes]:
    _mkdir_private(root)
    genesis = {
        "transactions": [
            {
                "instructions": [
                    {
                        "Grant": {
                            "Permission": {
                                "destination": KAGEMUSHA_ACTIVATION_AUTHORITY,
                                "object": {
                                    "name": "CanActivateKagemushaRecursiveReleaseV4"
                                },
                            }
                        }
                    },
                    {
                        "Grant": {
                            "Permission": {
                                "destination": KAGEMUSHA_ACTIVATION_AUTHORITY,
                                "object": {
                                    "name": "CanManageOfflineDeviceAttestationPolicy"
                                },
                            }
                        }
                    },
                ]
            }
        ]
    }
    payloads = {
        "privacy_bootstrap_plan.json": b'{"plan":true}\n',
        "config.toml": (
            b'[genesis]\npublic_key = "ed0120'
            + b"AB" * 32
            + b'"\nexpected_hash = "REPLACE_WITH_GENESIS_EXPECTED_HASH"\n'
        ),
        "genesis.json": reset_bundle.canonical_json_bytes(genesis),
        "nevo-reset.review.json": b'{"review":true}\n',
        "bootle_lantern_broker_public.json": b'{"broker":true}\n',
    }
    rows: dict[str, object] = {}
    for name, payload in payloads.items():
        _write_private(root / name, payload)
        rows[name] = {
            "rollout_path": reset_bundle.privacy_release.PRIVACY_INPUTS[name][
                "rollout_path"
            ],
            "sha256": hashlib.sha256(payload).hexdigest(),
            "size": len(payload),
        }
    manifest = {
        "schema": reset_bundle.privacy_release.SCHEMA,
        "schema_version": reset_bundle.privacy_release.SCHEMA_VERSION,
        "source": {
            "commit": source_commit,
            "dpn_validator_release_commit": dpn_validator_release_commit,
            "cargo_lock_sha256": cargo_lock_sha256,
            "workspace_source_manifest_sha256": workspace_sha256,
        },
        "linux_archive": {
            "name": "release.tar.gz",
            "sha256": "11" * 32,
            "size": 1234,
        },
        "authority": {
            "manifest_sha256": "22" * 32,
            "native_verifier_sha256": "33" * 32,
            "signer_fingerprint_sha256": "44" * 32,
        },
        "rollout_manifest_sha256": "55" * 32,
        "privacy_inputs": rows,
    }
    _write_private(
        root / reset_bundle.privacy_release.OUTPUT_MANIFEST,
        reset_bundle.canonical_json_bytes(manifest),
    )
    return payloads


def _source_reset(root: Path) -> None:
    _mkdir_private(root)
    for name, body in (
        ("genesis.signed.nrt", b"old signed"),
        ("genesis.json", b"{}\n"),
        ("base-config.toml", b"old = true\n"),
        ("validator-roster.toml", b"sealed roster\n"),
        ("validator-secrets.toml", b"sealed secrets\n"),
    ):
        _write_private(root / name, body)
    manifest = {
        "schema": "taira-exact2f-reset-bundle",
        "peer_count": 4,
        "chain_id": reset_bundle.CHAIN_ID,
        "chain_discriminant": reset_bundle.CHAIN_DISCRIMINANT,
        "node_storage_budget_bytes": 68_719_476_736,
        "node_storage_budget_weights": {
            "kura_blocks_bps": 7499,
            "wsv_snapshots_bps": 2000,
            "sorafs_bps": 1,
            "soranet_spool_bps": 250,
            "soravpn_spool_bps": 250,
        },
        "nexus_storage_budget_policy": "bounded-64-gib-per-validator",
    }
    _write_private(
        root / "reset-manifest.json",
        (json.dumps(manifest, sort_keys=True) + "\n").encode(),
    )
    rendered = root / "rendered"
    _mkdir_private(rendered)
    _write_private(rendered / "genesis.json", b"{}\n")
    for slug in reset_bundle.SLUGS:
        peer = rendered / slug
        _mkdir_private(peer)
        _write_private(peer / "config.toml", b"old config\n")
        for tree in ("codec", "configs", "manifests", "runtime", "storage"):
            _mkdir_private(peer / tree)
        _write_private(peer / "codec/schema.nrt", b"codec")
        _write_private(peer / "configs/runtime.toml", b"runtime")


def _fake_renderer(
    _base_config: Path,
    _roster: Path,
    output_dir: Path,
    *,
    base_genesis_path: Path | None,
    genesis_expected_hash: str | None = None,
    kagemusha_release_root: Path | None = None,
    include_kagemusha_qualification_seal: bool = True,
    **_kwargs,
) -> list[Path]:
    _mkdir_private(output_dir)
    if base_genesis_path is not None:
        _write_private(output_dir / "genesis.json", base_genesis_path.read_bytes())
        _write_private(
            output_dir / "genesis-signing-command.txt",
            (
                b'"$TAIRA_GENESIS_EXTERNAL_SIGNER" --unsigned-genesis genesis.json '
                b"--peer-config config.toml --bound-manifest-out genesis.json "
                b"--signed-genesis-out genesis.signed.nrt "
                b"--expected-hash-out genesis.expected_hash\n"
            ),
        )
    written: list[Path] = []
    for index, slug in enumerate(reset_bundle.SLUGS, start=1):
        peer = output_dir / slug
        _mkdir_private(peer)
        _mkdir_private(peer / "runtime")
        _mkdir_private(peer / "manifests")
        for sidecar in reset_bundle.RUNTIME_SIDECARS:
            _write_private(peer / "runtime" / sidecar, f"{slug}-{sidecar}".encode())
        _write_private(
            peer / "manifests/governance.manifest.json", b'{"lane":"governance"}\n'
        )
        expected = (
            genesis_expected_hash
            or reset_bundle.renderer.GENESIS_EXPECTED_HASH_PLACEHOLDER
        )
        config = f'peer = {index}\nexpected = "{expected}"\n'
        if kagemusha_release_root is not None:
            config += (
                "\n[settlement.offline]\n"
                "kagemusha_release_policy_path = "
                f'"{kagemusha_release_root / reset_bundle.renderer.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH}"\n'
                "kagemusha_artifact_dir = "
                f'"{kagemusha_release_root / reset_bundle.renderer.KAGEMUSHA_ARTIFACT_RELATIVE_PATH}"\n'
            )
            if include_kagemusha_qualification_seal:
                config += (
                    "kagemusha_catalog_qualification_seal_path = "
                    f'"{kagemusha_release_root / reset_bundle.renderer.KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH}"\n'
                )
            config += (
                "kagemusha_max_decoded_bytes = "
                f"{reset_bundle.renderer.KAGEMUSHA_MAX_DECODED_BYTES}\n"
            )
        _write_private(
            peer / "config.toml",
            config.encode(),
        )
        written.append(peer / "config.toml")
    return written


def _fake_receipt_signers() -> dict[str, dict[str, object]]:
    signers: dict[str, dict[str, object]] = {}
    for scalar, slug in enumerate(reset_bundle.SLUGS, start=1):
        public_payload = reset_bundle.renderer._secp256k1_public_payload(
            scalar.to_bytes(32, "big")
        )
        public_key = (
            reset_bundle.renderer.RECEIPT_PUBLIC_KEY_PREFIX
            + public_payload.hex().upper()
        )
        signers[slug] = {
            "node_id": reset_bundle.renderer.receipt_node_id(public_key),
            "public_key": {
                "algorithm": "secp256k1",
                "payload_hex": public_payload.hex(),
            },
        }
    return signers


def _stub_receipt_signer_loading(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        reset_bundle.renderer,
        "load_roster",
        lambda *_args, **_kwargs: object(),
    )
    monkeypatch.setattr(
        reset_bundle.renderer,
        "receipt_signer_map",
        lambda _validators: _fake_receipt_signers(),
    )


def _prepare_args(
    private: Path,
    source: Path,
    privacy: Path,
    genesis_signer: Path,
) -> argparse.Namespace:
    token_hash_tool = private / "onboarding-token-hash-tool"
    if not token_hash_tool.exists():
        _write_private(token_hash_tool, b"fake native token hash tool")
        token_hash_tool.chmod(0o700)
    controller_manifest = private / "authority-controller-v1.json"
    if not controller_manifest.exists():
        _write_private(controller_manifest, b'{"test":"controller"}\n')
    return argparse.Namespace(
        source_bundle=source,
        source_bundle_sha256=reset_bundle.source_bundle_sha256(source),
        privacy_release_dir=privacy,
        genesis_external_signer=genesis_signer,
        trusted_genesis_external_signer_sha256=reset_bundle.sha256(genesis_signer),
        onboarding_token_hash_tool=token_hash_tool,
        output_bundle=private / "output",
        irohad_sha256="66" * 32,
        source_commit="ab" * 20,
        dpn_validator_release_commit=DPN_COMMIT,
        cargo_lock_sha256="cd" * 32,
        workspace_source_manifest_sha256="ef" * 32,
        controller_manifest=controller_manifest,
        controller_digest="12" * 32,
        kagemusha_release_root=None,
        kagemusha_activation_authority=None,
        minimum_free_bytes=0,
    )


def _trust_test_controller(
    args: argparse.Namespace, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(
        reset_bundle,
        "_sealed_controller_manifest_path",
        lambda: args.controller_manifest,
    )
    monkeypatch.setattr(
        reset_bundle.controller_seal,
        "verify",
        lambda *_args, **_kwargs: {"verified": True},
    )
    monkeypatch.setattr(
        reset_bundle,
        "_validate_authenticated_nevo_release",
        lambda _payloads: {
            "schema": "iroha.taira.nevo-reset-review.v1",
            "public_inputs_sha256": "91" * 32,
            "unsigned_genesis_sha256": "92" * 32,
            "public_identities": {
                "onboarding_authority_account_id": "reviewed-onboarding",
                "api_signer_account_id": "reviewed-api",
            },
            "credential_hash_bindings": [
                {
                    "scope": {"dataspace": "is2"},
                    "token_hash": "blake3:" + "93" * 32,
                },
                {
                    "scope": {"dataspace": "dpn"},
                    "token_hash": "blake3:" + "94" * 32,
                },
            ],
        },
    )
    monkeypatch.setattr(
        reset_bundle,
        "_validate_rendered_nevo_bindings",
        lambda *_args, **_kwargs: None,
    )


def test_private_file_guard_rejects_symlink_hardlink_and_permissive_mode(
    tmp_path: Path,
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    original = private / "key"
    _write_private(original, b"secret")
    hardlink = private / "hardlink"
    os.link(original, hardlink)
    with pytest.raises(RuntimeError, match="unsafe private file identity"):
        reset_bundle.require_private_regular_file(original)
    hardlink.unlink()
    symlink = private / "symlink"
    symlink.symlink_to(original)
    with pytest.raises(RuntimeError, match="canonical non-symlink"):
        reset_bundle.require_private_regular_file(symlink)
    original.chmod(0o640)
    with pytest.raises(RuntimeError, match="unsafe private file identity"):
        reset_bundle.require_private_regular_file(original)


def test_receipt_signer_public_map_rejects_omission_reorder_mismatch_and_secret() -> None:
    accepted = _fake_receipt_signers()
    assert reset_bundle.require_receipt_signer_public_map(accepted) == accepted

    omitted = dict(accepted)
    omitted.pop(reset_bundle.SLUGS[-1])
    reordered = dict(reversed(list(accepted.items())))
    mismatched = json.loads(json.dumps(accepted))
    mismatched[reset_bundle.SLUGS[0]]["node_id"] = (
        reset_bundle.renderer.RECEIPT_NODE_ID_PREFIX + "0" * 64
    )
    private_leak = json.loads(json.dumps(accepted))
    private_leak[reset_bundle.SLUGS[0]]["receipt_private_key"] = "812620" + "01" * 32

    for tampered in (
        omitted,
        reordered,
        mismatched,
        private_leak,
    ):
        with pytest.raises(RuntimeError):
            reset_bundle.require_receipt_signer_public_map(tampered)


def test_authenticated_privacy_snapshot_rejects_file_substitution(
    tmp_path: Path,
) -> None:
    root = tmp_path / "release"
    payloads = _privacy_release(
        root,
        source_commit="ab" * 20,
        dpn_validator_release_commit=DPN_COMMIT,
        cargo_lock_sha256="cd" * 32,
        workspace_sha256="ef" * 32,
    )
    _write_private(root / "genesis.json", b'{"substituted":true}\n')

    with pytest.raises(RuntimeError, match="differs from its manifest"):
        reset_bundle._load_authenticated_privacy_release(
            root,
            source_commit="ab" * 20,
            dpn_validator_release_commit=DPN_COMMIT,
            cargo_lock_sha256="cd" * 32,
            workspace_source_manifest_sha256="ef" * 32,
        )
    assert payloads["genesis.json"] != (root / "genesis.json").read_bytes()


def test_authenticated_privacy_snapshot_rejects_dpn_only_mismatch(
    tmp_path: Path,
) -> None:
    root = tmp_path / "release"
    _privacy_release(
        root,
        source_commit="ab" * 20,
        dpn_validator_release_commit=DPN_COMMIT,
        cargo_lock_sha256="cd" * 32,
        workspace_sha256="ef" * 32,
    )

    with pytest.raises(RuntimeError, match="source differs"):
        reset_bundle._load_authenticated_privacy_release(
            root,
            source_commit="ab" * 20,
            dpn_validator_release_commit="34" * 20,
            cargo_lock_sha256="cd" * 32,
            workspace_source_manifest_sha256="ef" * 32,
        )


def test_source_reset_snapshot_rejects_any_post_review_byte_mutation(
    tmp_path: Path,
) -> None:
    private = tmp_path / "private"
    source = private / "source"
    snapshot = private / "snapshot"
    _mkdir_private(private)
    _source_reset(source)
    expected = reset_bundle.source_bundle_sha256(source)
    _write_private(source / "rendered/taira-validator-4/codec/schema.nrt", b"mutated")

    with pytest.raises(RuntimeError, match="protected inventory digest"):
        reset_bundle._snapshot_authenticated_source_bundle(
            source,
            snapshot,
            expected,
        )

    assert not snapshot.exists()


@pytest.mark.parametrize("attack", ("symlink", "hardlink"))
def test_source_reset_digest_rejects_link_substitution(
    tmp_path: Path, attack: str
) -> None:
    private = tmp_path / "private"
    source = private / "source"
    _mkdir_private(private)
    _source_reset(source)
    target = source / "rendered/taira-validator-2/config.toml"
    victim = private / "victim"
    _write_private(victim, b"victim\n")
    target.unlink()
    if attack == "symlink":
        target.symlink_to(victim)
    else:
        os.link(victim, target)

    with pytest.raises((RuntimeError, reset_bundle.ReleaseArtifactError)):
        reset_bundle.source_bundle_sha256(source)

    assert victim.read_bytes() == b"victim\n"


def test_external_genesis_signer_never_receives_private_key_material(
    tmp_path: Path,
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    genesis = private / "genesis.json"
    config = private / "config.toml"
    signed = private / "genesis.signed.nrt"
    _write_private(genesis, b'{"bound":true}\n')
    _write_private(config, b"config\n")
    record = private / "argv.json"
    signer = private / "external-genesis-signer"
    signer.write_text(
        "#!/usr/bin/env python3\n"
        "import json,os,sys\n"
        f"open({str(record)!r}, 'w').write(json.dumps({{'argv':sys.argv,'env':dict(os.environ)}}))\n"
        "args=sys.argv\n"
        "open(args[args.index('--signed-genesis-out')+1], 'wb').write(b'signed')\n"
        "open(args[args.index('--expected-hash-out')+1], 'w').write('"
        + "00" * 31
        + "01\\n')\n",
        encoding="utf-8",
    )
    signer.chmod(0o700)

    expected = reset_bundle._sign_genesis(
        external_signer=signer,
        trusted_external_signer_sha256=reset_bundle.sha256(signer),
        rendered_genesis=genesis,
        peer_one_config=config,
        signed_genesis=signed,
        temporary_root=private,
    )

    invocation = json.loads(record.read_text(encoding="utf-8"))
    argv = invocation["argv"]
    assert expected == "00" * 31 + "01"
    assert "--unsigned-genesis" in argv
    assert "--peer-config" in argv
    assert "--signed-genesis-out" in argv
    assert argv[1::2] == [
        "--unsigned-genesis",
        "--peer-config",
        "--bound-manifest-out",
        "--signed-genesis-out",
        "--expected-hash-out",
    ]
    assert len(argv) == 11
    assert "private-key" not in json.dumps(argv).lower()
    assert {"HOME", "LANG", "LC_ALL", "PATH", "TMPDIR"} <= set(invocation["env"])
    assert not any(
        token in name.upper()
        for name in invocation["env"]
        for token in (
            "TAIRA",
            "GITHUB",
            "IROHA",
            "PASSWORD",
            "PRIVATE_KEY",
            "SECRET",
            "SIGNER",
            "TOKEN",
        )
    )
    assert not (private / "genesis-signer.snapshot").exists()
    assert not (private / "genesis.expected_hash").exists()


def test_external_genesis_signer_digest_mismatch_fails_before_execution(
    tmp_path: Path,
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    genesis = private / "genesis.json"
    config = private / "config.toml"
    signed = private / "genesis.signed.nrt"
    marker = private / "signer-executed"
    _write_private(genesis, b"{}\n")
    _write_private(config, b"config\n")
    signer = private / "external-genesis-signer"
    signer.write_text(
        f"#!/bin/sh\ntouch {str(marker)!r}\nexit 99\n",
        encoding="utf-8",
    )
    signer.chmod(0o700)

    with pytest.raises(RuntimeError, match="differs from its trusted SHA-256"):
        reset_bundle._sign_genesis(
            external_signer=signer,
            trusted_external_signer_sha256="f" * 64,
            rendered_genesis=genesis,
            peer_one_config=config,
            signed_genesis=signed,
            temporary_root=private,
        )

    assert not marker.exists()
    assert not signed.exists()


@pytest.mark.parametrize("attack", ("symlink", "hardlink"))
def test_external_genesis_signer_rejects_link_aliases(
    tmp_path: Path,
    attack: str,
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    signer = private / "reviewed-signer"
    signer.write_text("#!/bin/sh\nexit 99\n", encoding="utf-8")
    signer.chmod(0o700)
    trusted_sha256 = hashlib.sha256(signer.read_bytes()).hexdigest()
    alias = private / "signer-alias"
    if attack == "symlink":
        alias.symlink_to(signer.name)
    else:
        os.link(signer, alias)

    with pytest.raises(RuntimeError):
        reset_bundle._sign_genesis(
            external_signer=alias,
            trusted_external_signer_sha256=trusted_sha256,
            rendered_genesis=private / "genesis.json",
            peer_one_config=private / "config.toml",
            signed_genesis=private / "genesis.signed.nrt",
            temporary_root=private,
        )


def test_external_genesis_signer_replacement_during_execution_is_rejected(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    genesis = private / "genesis.json"
    config = private / "config.toml"
    signed = private / "genesis.signed.nrt"
    _write_private(genesis, b"{}\n")
    _write_private(config, b"config\n")
    signer = private / "external-genesis-signer"
    signer.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    signer.chmod(0o700)
    replacement = private / "replacement-signer"
    replacement.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
    replacement.chmod(0o700)

    def replace_during_run(command, **_kwargs):
        os.replace(replacement, signer)
        Path(command[command.index("--signed-genesis-out") + 1]).write_bytes(b"signed")
        Path(command[command.index("--expected-hash-out") + 1]).write_text(
            "00" * 31 + "01\n",
            encoding="ascii",
        )
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr(reset_bundle.subprocess, "run", replace_during_run)

    with pytest.raises(RuntimeError, match="changed during genesis signing"):
        reset_bundle._sign_genesis(
            external_signer=signer,
            trusted_external_signer_sha256=reset_bundle.sha256(signer),
            rendered_genesis=genesis,
            peer_one_config=config,
            signed_genesis=signed,
            temporary_root=private,
        )


def test_external_genesis_signer_diagnostics_cannot_inject_controller_output(
    tmp_path: Path,
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    genesis = private / "genesis.json"
    config = private / "config.toml"
    signed = private / "genesis.signed.nrt"
    _write_private(genesis, b"{}\n")
    _write_private(config, b"config\n")
    signer = private / "external-genesis-signer"
    injected = "::error::FORGED_WORKFLOW_OUTPUT"
    signer.write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        f"print({injected!r})\n"
        f"sys.stderr.write({(injected + chr(27) + '[31m')!r})\n"
        "raise SystemExit(23)\n",
        encoding="utf-8",
    )
    signer.chmod(0o700)

    with pytest.raises(RuntimeError) as error:
        reset_bundle._sign_genesis(
            external_signer=signer,
            trusted_external_signer_sha256=reset_bundle.sha256(signer),
            rendered_genesis=genesis,
            peer_one_config=config,
            signed_genesis=signed,
            temporary_root=private,
        )

    assert str(error.value).endswith("exit status 23")
    assert injected not in str(error.value)
    assert not (private / "genesis-signer.stdout").exists()
    assert not (private / "genesis-signer.stderr").exists()
    assert not (private / "genesis-signer.snapshot").exists()


def test_prepare_recomposes_signed_reset_and_binds_all_four_reviewed_inputs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    source = private / "source"
    privacy = private / "privacy"
    signer = private / "external-genesis-signer"
    _source_reset(source)
    payloads = _privacy_release(
        privacy,
        source_commit="ab" * 20,
        dpn_validator_release_commit=DPN_COMMIT,
        cargo_lock_sha256="cd" * 32,
        workspace_sha256="ef" * 32,
    )
    _write_private(signer, b"fake external signer")
    signer.chmod(0o700)
    render_release_roots: list[Path | None] = []
    render_seal_modes: list[bool] = []
    render_genesis_hashes: list[str | None] = []

    def render_with_release_root(*render_args, **render_kwargs):
        render_release_roots.append(render_kwargs.get("kagemusha_release_root"))
        render_seal_modes.append(
            render_kwargs.get("include_kagemusha_qualification_seal", True)
        )
        render_genesis_hashes.append(render_kwargs.get("genesis_expected_hash"))
        return _fake_renderer(*render_args, **render_kwargs)

    monkeypatch.setattr(
        reset_bundle.renderer, "render_bundle", render_with_release_root
    )
    _stub_receipt_signer_loading(monkeypatch)
    monkeypatch.setattr(
        reset_bundle,
        "_validate_rendered_configs",
        lambda output, _expected: {
            slug: reset_bundle.sha256(output / "rendered" / slug / "config.toml")
            for slug in reset_bundle.SLUGS
        },
    )
    monkeypatch.setattr(
        reset_bundle,
        "_sign_genesis",
        lambda **kwargs: (
            _write_private(kwargs["signed_genesis"], b"new signed genesis")
            or ("00" * 31 + "01")
        ),
    )
    args = _prepare_args(private, source, privacy, signer)
    args.kagemusha_release_root = tmp_path / "kagemusha-release"
    _mkdir_private(args.kagemusha_release_root / "policy")
    _write_private(
        args.kagemusha_release_root
        / reset_bundle.renderer.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH,
        b"authenticated release policy",
    )
    args.kagemusha_activation_authority = KAGEMUSHA_ACTIVATION_AUTHORITY
    _trust_test_controller(args, monkeypatch)

    result = reset_bundle.prepare(args)

    output = args.output_bundle
    manifest = json.loads((output / "reset-manifest.json").read_text(encoding="utf-8"))
    assert result["peer_count"] == 4
    assert result["kagemusha_release_root"] == str(args.kagemusha_release_root)
    assert render_release_roots == [
        args.kagemusha_release_root,
        args.kagemusha_release_root,
    ]
    assert render_seal_modes == [False, True]
    assert render_genesis_hashes == [
        reset_bundle.renderer.GENESIS_EXPECTED_HASH_PLACEHOLDER,
        "00" * 31 + "01",
    ]
    assert (output / "base-config.toml").read_bytes() == payloads["config.toml"]
    assert (output / "genesis.signed.nrt").read_bytes() == b"new signed genesis"
    assert not (output / "validator-secrets.toml").exists()
    assert "receipt_private" not in json.dumps(manifest).lower()
    assert manifest["chain_id"] == reset_bundle.CHAIN_ID
    assert manifest["dpn_validator_release_commit"] == DPN_COMMIT
    assert manifest["kagemusha_release_root"] == str(args.kagemusha_release_root)
    assert (
        manifest["kagemusha_activation_authority"]
        == KAGEMUSHA_ACTIVATION_AUTHORITY
    )
    assert manifest["kagemusha_release_policy_sha256"] == hashlib.sha256(
        b"authenticated release policy"
    ).hexdigest()
    expected_kagemusha_projection = reset_bundle._kagemusha_config_projection(
        args.kagemusha_release_root
    )
    assert manifest["kagemusha_config_projection"] == expected_kagemusha_projection
    assert manifest["kagemusha_config_projection_sha256"] == hashlib.sha256(
        reset_bundle.canonical_json_bytes(expected_kagemusha_projection)
    ).hexdigest()
    assert (
        result["kagemusha_config_projection_sha256"]
        == manifest["kagemusha_config_projection_sha256"]
    )
    assert manifest["source_reset_bundle_sha256"] == args.source_bundle_sha256
    assert (
        manifest["signed_genesis_sha256"]
        == hashlib.sha256(b"new signed genesis").hexdigest()
    )
    assert manifest["onboarding_token_hash_tool_sha256"] == reset_bundle.sha256(
        args.onboarding_token_hash_tool
    )
    assert (
        manifest["bound_genesis_manifest_sha256"] == manifest["unsigned_genesis_sha256"]
    )
    reviewed = manifest["privacy_bootstrap_release"]["reviewed_inputs"]
    assert set(reviewed) == set(reset_bundle.privacy_release.PRIVACY_INPUTS)
    for name, payload in payloads.items():
        assert reviewed[name]["sha256"] == hashlib.sha256(payload).hexdigest()
    assert set(manifest["configs"]) == set(reset_bundle.SLUGS)
    for slug in reset_bundle.SLUGS:
        assert not any((output / "rendered" / slug / "storage").iterdir())


def test_prepare_removes_all_partial_output_when_native_signing_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    source = private / "source"
    privacy = private / "privacy"
    signer = private / "external-genesis-signer"
    _source_reset(source)
    _privacy_release(
        privacy,
        source_commit="ab" * 20,
        dpn_validator_release_commit=DPN_COMMIT,
        cargo_lock_sha256="cd" * 32,
        workspace_sha256="ef" * 32,
    )
    _write_private(signer, b"fake external signer")
    signer.chmod(0o700)
    monkeypatch.setattr(reset_bundle.renderer, "render_bundle", _fake_renderer)
    _stub_receipt_signer_loading(monkeypatch)
    monkeypatch.setattr(
        reset_bundle,
        "_validate_rendered_configs",
        lambda output, _expected: {
            slug: reset_bundle.sha256(output / "rendered" / slug / "config.toml")
            for slug in reset_bundle.SLUGS
        },
    )
    monkeypatch.setattr(
        reset_bundle,
        "_sign_genesis",
        lambda **_kwargs: (_ for _ in ()).throw(RuntimeError("signing rejected")),
    )
    args = _prepare_args(private, source, privacy, signer)
    _trust_test_controller(args, monkeypatch)

    with pytest.raises(RuntimeError, match="signing rejected"):
        reset_bundle.prepare(args)
    assert not args.output_bundle.exists()


def test_prepare_refuses_invalid_first_pass_before_using_external_signer(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    private = tmp_path / "private"
    _mkdir_private(private)
    source = private / "source"
    privacy = private / "privacy"
    external_signer = private / "external-genesis-signer"
    _source_reset(source)
    _privacy_release(
        privacy,
        source_commit="ab" * 20,
        dpn_validator_release_commit=DPN_COMMIT,
        cargo_lock_sha256="cd" * 32,
        workspace_sha256="ef" * 32,
    )
    _write_private(external_signer, b"fake external signer")
    external_signer.chmod(0o700)
    signer = mock.Mock()
    monkeypatch.setattr(reset_bundle.renderer, "render_bundle", _fake_renderer)
    _stub_receipt_signer_loading(monkeypatch)
    monkeypatch.setattr(reset_bundle, "_sign_genesis", signer)
    monkeypatch.setattr(
        reset_bundle,
        "_validate_rendered_configs",
        mock.Mock(side_effect=RuntimeError("invalid first-pass configs")),
    )
    args = _prepare_args(private, source, privacy, external_signer)
    _trust_test_controller(args, monkeypatch)

    with pytest.raises(RuntimeError, match="invalid first-pass configs"):
        reset_bundle.prepare(args)

    signer.assert_not_called()
    assert not args.output_bundle.exists()


if __name__ == "__main__":
    unittest.main()
