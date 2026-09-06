"""Adversarial unit tests for the fail-closed TON SCCP release builder."""

from __future__ import annotations

import base64
import copy
import hashlib
import os
import subprocess
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import sccp_release_common as common  # noqa: E402
import ton_sccp_builder as builder  # noqa: E402


def _keypair(label: str) -> tuple[bytes, bytes, int]:
    entropy = hashlib.sha256(f"ton-builder-test:{label}".encode("ascii")).digest()
    digest = hashlib.sha512(entropy).digest()
    scalar_bytes = bytearray(digest[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, scalar))
    return public, digest[32:], scalar


def _sign(keypair: tuple[bytes, bytes, int], message: bytes) -> str:
    public, prefix, scalar = keypair
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % common._ED_L
    encoded_r = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, nonce))
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public + message).digest(), "little"
    ) % common._ED_L
    encoded_s = ((nonce + challenge * scalar) % common._ED_L).to_bytes(32, "little")
    signature = encoded_r + encoded_s
    assert common.verify_ed25519(public, signature, message)
    return base64.b64encode(signature).decode("ascii")


def _policy() -> tuple[dict[str, object], dict[str, tuple[bytes, bytes, int]]]:
    keys = {role: _keypair(role) for role in builder.APPROVER_ROLES}
    policy: dict[str, object] = {
        "schema": builder.POLICY_SCHEMA,
        "source": {
            "commit": "10" * 20,
            "commit_signer_fingerprint": "0123456789abcdef",
            "source_date_epoch": 1_700_000_000,
        },
        "builder": {
            "image": f"registry.example/iroha-ton-builder@sha256:{'20' * 32}",
            "platform": builder.PLATFORM,
            "driver_path": "/usr/local/bin/iroha-sccp-ton-builder-final-v1",
            "acton_archive_sha256": builder.ACTON_ARCHIVE_SHA256,
            "acton_reported_version": builder.ACTON_VERSION,
            "tolk_reported_version": builder.TOLK_VERSION,
            "host_python_sha256": "2f" * 32,
            "host_git_sha256": "30" * 32,
            "host_docker_sha256": "40" * 32,
            "toolchain_inventory": [
                {
                    "path": "toolchain/acton",
                    "role": "acton-executable",
                    "sha256": "50" * 32,
                    "size_bytes": 100,
                    "executable": True,
                },
                {
                    "path": "toolchain/driver",
                    "role": "builder-driver",
                    "sha256": "60" * 32,
                    "size_bytes": 101,
                    "executable": True,
                },
                {
                    "path": "toolchain/stdlib/common.tolk",
                    "role": "tolk-stdlib",
                    "sha256": "70" * 32,
                    "size_bytes": 102,
                    "executable": False,
                },
            ],
        },
        "limits": {
            "max_artifacts": 128,
            "max_artifact_bytes": 16 * 1024 * 1024,
            "max_total_bytes": 256 * 1024 * 1024,
            "max_log_bytes": 1024 * 1024,
            "timeout_seconds": 1800,
        },
        "approvers": [
            {
                "role": role,
                "signer_id": f"ton-{role}",
                "public_key_hex": keys[role][0].hex(),
            }
            for role in builder.APPROVER_ROLES
        ],
    }
    return policy, keys


def _unsigned_lock() -> dict[str, object]:
    return {
        "schema": builder.LOCK_SCHEMA,
        "builder_policy_sha256": "80" * 32,
        "source_closure_sha256": "90" * 32,
        "source_commit": "10" * 20,
        "artifact_tree_sha256": "a0" * 32,
        "artifacts": [
            {
                "path": "build/TairaXorSccpBridge.json",
                "sha256": "b0" * 32,
                "size_bytes": 100,
                "executable": False,
            }
        ],
        "toolchain_inventory": [
            {
                "path": "toolchain/acton",
                "role": "acton-executable",
                "sha256": "50" * 32,
                "size_bytes": 100,
                "executable": True,
            }
        ],
    }


def _signed_lock(
    unsigned: dict[str, object],
    policy: dict[str, object],
    keys: dict[str, tuple[bytes, bytes, int]],
) -> dict[str, object]:
    payload = builder.output_lock_signing_payload(unsigned)
    approvers = policy["approvers"]
    assert isinstance(approvers, list)
    return {
        **copy.deepcopy(unsigned),
        "provenance": [
            {
                "role": role,
                "signer_id": approvers[index]["signer_id"],
                "algorithm": "ed25519",
                "public_key_hex": approvers[index]["public_key_hex"],
                "signature_b64": _sign(keys[role], payload),
            }
            for index, role in enumerate(builder.APPROVER_ROLES)
        ],
    }


def test_policy_closes_versions_image_host_tools_inventory_and_approvers() -> None:
    policy, _ = _policy()
    assert builder.validate_policy(policy) == policy
    for mutation in range(10):
        candidate = copy.deepcopy(policy)
        if mutation == 0:
            candidate["builder"]["image"] = "registry.example/builder:latest"
        elif mutation == 1:
            candidate["builder"]["platform"] = "linux/arm64"
        elif mutation == 2:
            candidate["builder"]["acton_archive_sha256"] = "00" * 32
        elif mutation == 3:
            candidate["builder"]["acton_reported_version"] = "acton 1.1.0"
        elif mutation == 4:
            candidate["builder"]["tolk_reported_version"] = "1.4.0"
        elif mutation == 5:
            candidate["builder"]["host_git_sha256"] = "00" * 32
        elif mutation == 6:
            candidate["builder"]["toolchain_inventory"].pop()
        elif mutation == 7:
            candidate["approvers"][1]["public_key_hex"] = candidate["approvers"][0][
                "public_key_hex"
            ]
        elif mutation == 8:
            candidate["source"]["commit"] = "1" * 39
        else:
            candidate["extra"] = True
        with pytest.raises(builder.TonBuilderError):
            builder.validate_policy(candidate)


def test_output_lock_requires_two_exact_fresh_independent_signatures() -> None:
    policy, keys = _policy()
    unsigned = _unsigned_lock()
    signed = _signed_lock(unsigned, policy, keys)
    assert builder.validate_signed_lock(
        signed,
        expected_unsigned=unsigned,
        policy=policy,
    ) == signed

    for mutation in range(6):
        candidate = copy.deepcopy(signed)
        if mutation == 0:
            candidate["artifact_tree_sha256"] = "c0" * 32
        elif mutation == 1:
            candidate["provenance"].reverse()
        elif mutation == 2:
            candidate["provenance"][0]["signature_b64"] = candidate["provenance"][1][
                "signature_b64"
            ]
        elif mutation == 3:
            candidate["provenance"][0]["algorithm"] = "ed25519ph"
        elif mutation == 4:
            candidate["provenance"][0]["signature_b64"] = "AA=="
        else:
            candidate["legacy"] = True
        with pytest.raises(builder.TonBuilderError):
            builder.validate_signed_lock(
                candidate,
                expected_unsigned=unsigned,
                policy=policy,
            )


def test_tree_scanner_hashes_regular_files_and_rejects_symlinks(tmp_path: Path) -> None:
    root = tmp_path / "artifacts"
    root.mkdir(mode=0o700)
    artifact = root / "contract.json"
    artifact.write_bytes(b'{"code":"bounded-public-bytecode"}\n')
    artifact.chmod(0o600)
    entries = builder._scan_tree(
        root,
        label="test artifact tree",
        maximum_files=4,
        maximum_file_bytes=1024,
        maximum_total_bytes=4096,
        scan_text=True,
    )
    assert entries == [
        {
            "path": "contract.json",
            "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
            "size_bytes": artifact.stat().st_size,
            "executable": False,
        }
    ]
    (root / "alias.json").symlink_to(artifact)
    with pytest.raises(builder.TonBuilderError, match="symlink"):
        builder._scan_tree(
            root,
            label="test artifact tree",
            maximum_files=4,
            maximum_file_bytes=1024,
            maximum_total_bytes=4096,
            scan_text=True,
        )


def test_candidate_publication_is_private_exclusive_and_manifest_last(tmp_path: Path) -> None:
    source = tmp_path / "source"
    artifact = source / "artifacts" / "build" / "contract.json"
    artifact.parent.mkdir(parents=True, mode=0o700)
    payload = b'{"contract":"canonical"}\n'
    artifact.write_bytes(payload)
    artifact.chmod(0o600)
    unsigned = _unsigned_lock()
    unsigned["artifacts"] = [
        {
            "path": "build/contract.json",
            "sha256": hashlib.sha256(payload).hexdigest(),
            "size_bytes": len(payload),
            "executable": False,
        }
    ]
    output = tmp_path / "candidate"
    builder._publish_candidate(output, build_output=source, unsigned_lock=unsigned)
    assert (output.stat().st_mode & 0o077) == 0
    published = output / "artifacts" / "build" / "contract.json"
    assert published.read_bytes() == payload
    assert (published.stat().st_mode & 0o077) == 0
    assert (output / "unsigned-output-lock.json").read_bytes() == common.canonical_json_file_bytes(
        unsigned
    )
    assert (output / "output-lock-signing-payload.bin").read_bytes() == (
        builder.output_lock_signing_payload(unsigned)
    )
    with pytest.raises(builder.TonBuilderError, match="never overwrites"):
        builder._publish_candidate(output, build_output=source, unsigned_lock=unsigned)


def test_release_builder_has_no_path_acton_or_single_build_production_escape() -> None:
    python_source = (SCRIPTS / "ton_sccp_builder.py").read_text(encoding="utf-8")
    wrapper = (SCRIPTS / "sccp_ton_contract_build.sh").read_text(encoding="utf-8")
    assert "ACTON_BIN" not in python_source + wrapper
    assert "--network=none" in python_source
    assert "--platform=linux/amd64" in python_source
    assert "--pull=never" in python_source
    assert "--read-only" in python_source
    assert "--cap-drop=ALL" in python_source
    assert python_source.count("_run_container_build(") >= 3
    assert "report_one != report_two" in python_source
    assert builder.APPROVER_ROLES == ("release-engineering", "release-security")
    for field in (
        "ton_builder_policy_sha256",
        "ton_source_closure_sha256",
        "ton_output_lock_sha256",
    ):
        assert field in python_source
    mode = os.stat(SCRIPTS / "ton_sccp_builder.py").st_mode
    assert mode & 0o111


def test_cli_shape_errors_do_not_echo_untrusted_arguments() -> None:
    marker = "authorization=Bearer-do-not-echo"
    result = subprocess.run(
        [sys.executable, str(SCRIPTS / "ton_sccp_builder.py"), "--unknown", marker],
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert result.returncode == 2
    assert marker not in result.stdout + result.stderr
    assert len(result.stderr) < 1024
