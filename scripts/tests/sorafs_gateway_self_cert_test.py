"""Focused tests for the SoraFS gateway self-certification wrapper."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "sorafs_gateway_self_cert.sh"
TEST_PUBLIC_KEY = bytes.fromhex(
    "2152f8d19b791d24453242e15f2eab6c"
    "b7cffa7b6a5ed30097960e069881db12"
)
TEST_FINGERPRINT = hashlib.sha256(TEST_PUBLIC_KEY).hexdigest()
TEST_SIGNATURE = bytes.fromhex(
    "5a9e89b16ce487ecf4667ac0cf84ea79"
    "4b4730d440f3c2ca64143267204e0ccb"
    "e818d9f87a9e0be8bab2d7ba31f19afa"
    "4553ba8427bb493e24c2c5edd90a020e"
)


def write_file(path: Path, content: str = "{}") -> Path:
    path.write_text(content, encoding="utf-8")
    return path


def write_executable(path: Path, body: str) -> Path:
    path.write_text("#!/usr/bin/env python3\n" + body, encoding="utf-8")
    path.chmod(0o700)
    return path


def install_fake_cargo(tmp_path: Path) -> tuple[dict[str, str], Path]:
    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir()
    invocation_log = tmp_path / "cargo-invocations.log"
    cargo = fake_bin / "cargo"
    cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--list" ]]; then
  echo "xtask"
  exit 0
fi
if [[ "${1:-}" == "xtask" ]]; then
  printf '%s\n' "$*" >> "${SELF_CERT_CARGO_LOG}"
  exit 0
fi
echo "unexpected cargo invocation: $*" >&2
exit 127
""",
        encoding="utf-8",
    )
    cargo.chmod(0o700)
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    env["SELF_CERT_CARGO_LOG"] = str(invocation_log)
    return env, invocation_log


def write_release_inputs(tmp_path: Path) -> tuple[Path, Path, Path, Path, str]:
    manifest = tmp_path / "release_manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "arch": "x86_64",
                "artifacts": [],
                "commit": "abcdef0",
                "version": "1.0.0",
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    signature = tmp_path / "release_manifest.ed25519.sig"
    signature.write_bytes(TEST_SIGNATURE)
    public_key = tmp_path / "release_manifest.ed25519.pub"
    public_key.write_bytes(TEST_PUBLIC_KEY)
    public_key.chmod(0o600)
    native_log = tmp_path / "native-verifier-invocations.log"
    verifier = write_executable(
        tmp_path / "sorafs-validate",
        "import sys\n"
        "from pathlib import Path\n"
        f"log = Path({str(native_log)!r})\n"
        "args = sys.argv[1:]\n"
        "if len(args) != 9 or args[0] != 'release-manifest':\n"
        "    raise SystemExit(4)\n"
        "options = dict(zip(args[1::2], args[2::2]))\n"
        "if set(options) != {\n"
        "    '--manifest', '--public-key', '--public-key-fingerprint', '--signature'\n"
        "}:\n"
        "    raise SystemExit(4)\n"
        "with log.open('a', encoding='utf-8') as handle:\n"
        "    handle.write('release-manifest\\n')\n",
    )
    return (
        manifest,
        signature,
        public_key,
        verifier,
        hashlib.sha256(verifier.read_bytes()).hexdigest(),
    )


def required_args(tmp_path: Path) -> list[str]:
    manifest, signature, public_key, verifier, verifier_digest = write_release_inputs(
        tmp_path
    )
    signing_key = write_file(tmp_path / "gateway-attestor.hex", "00")
    signing_key.chmod(0o600)
    return [
        "--workspace",
        str(tmp_path),
        "--signing-key",
        str(signing_key),
        "--signer",
        "admin@operator",
        "--gateway",
        "https://gateway.example/",
        "--release-manifest",
        str(manifest),
        "--release-manifest-signature",
        str(signature),
        "--release-manifest-public-key",
        str(public_key),
        "--trusted-signing-fingerprint",
        TEST_FINGERPRINT,
        "--release-manifest-verifier",
        str(verifier),
        "--trusted-release-manifest-verifier-sha256",
        verifier_digest,
    ]


def run_script(
    *args: str,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(SCRIPT), *args],
        cwd=REPO_ROOT,
        env=env or os.environ.copy(),
        text=True,
        capture_output=True,
        check=False,
    )


def test_gateway_self_cert_verifies_release_before_running_harness(
    tmp_path: Path,
) -> None:
    env, cargo_log = install_fake_cargo(tmp_path)

    result = run_script(*required_args(tmp_path), env=env)

    receipt = (
        tmp_path
        / "artifacts"
        / "sorafs_gateway_attest"
        / "release_manifest.verify.json"
    )
    assert result.returncode == 0, result.stderr
    summary = json.loads(receipt.read_text(encoding="utf-8"))
    assert summary["signature_verified"] is True
    assert summary["signer_fingerprint_sha256"] == TEST_FINGERPRINT
    assert receipt.stat().st_mode & 0o077 == 0
    assert cargo_log.read_text(encoding="utf-8").startswith(
        "xtask sorafs-gateway-attest"
    )
    assert "--gateway https://gateway.example/" in cargo_log.read_text(encoding="utf-8")
    assert (
        tmp_path / "native-verifier-invocations.log"
    ).read_text(encoding="utf-8").splitlines() == ["release-manifest"]


def test_gateway_self_cert_requires_explicit_release_verification_tuple(
    tmp_path: Path,
) -> None:
    signing_key = write_file(tmp_path / "gateway-attestor.hex", "00")

    result = run_script(
        "--workspace",
        str(tmp_path),
        "--signing-key",
        str(signing_key),
        "--signer",
        "admin@operator",
    )

    assert result.returncode == 1
    assert "required gateway self-cert options are missing" in result.stderr
    assert "--release-manifest-signature" in result.stderr
    assert "--trusted-signing-fingerprint" in result.stderr
    assert "--release-manifest-verifier" in result.stderr
    assert "--trusted-release-manifest-verifier-sha256" in result.stderr
    assert "--gateway" in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_gateway_self_cert_requires_explicit_gateway_target_before_harness(
    tmp_path: Path,
) -> None:
    env, cargo_log = install_fake_cargo(tmp_path)
    args = required_args(tmp_path)
    gateway_index = args.index("--gateway")
    del args[gateway_index : gateway_index + 2]

    result = run_script(*args, env=env)

    assert result.returncode == 1
    assert "required gateway self-cert options are missing: --gateway" in result.stderr
    assert not cargo_log.exists()
    assert not (tmp_path / "artifacts").exists()


def test_gateway_self_cert_rejects_removed_signature_bundle_interface(
    tmp_path: Path,
) -> None:
    result = run_script(*required_args(tmp_path), "--manifest-bundle", "bundle.json")

    assert result.returncode == 1
    assert "unknown argument: --manifest-bundle" in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_gateway_self_cert_rejects_unreviewed_native_verifier_before_harness(
    tmp_path: Path,
) -> None:
    env, cargo_log = install_fake_cargo(tmp_path)
    args = required_args(tmp_path)
    digest_index = args.index("--trusted-release-manifest-verifier-sha256") + 1
    args[digest_index] = "00" * 32

    result = run_script(*args, env=env)

    assert result.returncode == 1
    assert "native release-manifest verifier does not match the reviewed SHA256" in (
        result.stderr
    )
    assert not cargo_log.exists()
    assert not (
        tmp_path
        / "artifacts"
        / "sorafs_gateway_attest"
        / "release_manifest.verify.json"
    ).exists()


def test_gateway_self_cert_rejects_symlinked_output_dir(tmp_path: Path) -> None:
    target = tmp_path / "real-output"
    output_dir = tmp_path / "attest-output"
    target.mkdir()
    output_dir.symlink_to(target, target_is_directory=True)

    result = run_script(*required_args(tmp_path), "--out", str(output_dir))

    assert result.returncode == 1
    assert "gateway self-cert output directory must not be a symlink" in result.stderr


def test_gateway_self_cert_rejects_existing_verification_receipt(
    tmp_path: Path,
) -> None:
    output_dir = tmp_path / "attest"
    output_dir.mkdir()
    receipt = write_file(output_dir / "release_manifest.verify.json", "unchanged")

    result = run_script(
        *required_args(tmp_path),
        "--out",
        str(output_dir),
    )

    assert result.returncode == 1
    assert "release manifest verification summary must not already exist" in result.stderr
    assert receipt.read_text(encoding="utf-8") == "unchanged"


def test_gateway_self_cert_rejects_unknown_config_key(tmp_path: Path) -> None:
    config = write_file(tmp_path / "self-cert.conf", "manifest_bundle=obsolete.json\n")

    result = run_script("--workspace", str(tmp_path), "--config", str(config))

    assert result.returncode == 1
    assert "unknown config key 'manifest_bundle'" in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_gateway_self_cert_rejects_symlinked_config(tmp_path: Path) -> None:
    target = write_file(tmp_path / "config-target", "signer=admin@operator\n")
    config = tmp_path / "self-cert.conf"
    config.symlink_to(target)

    result = run_script("--workspace", str(tmp_path), "--config", str(config))

    assert result.returncode == 1
    assert "gateway self-cert config must not be a symlink" in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_gateway_self_cert_rejects_missing_option_value_without_shell_error(
    tmp_path: Path,
) -> None:
    result = run_script(*required_args(tmp_path), "--out")

    assert result.returncode == 1
    assert "error: --out requires a value" in result.stderr
    assert "unbound variable" not in result.stderr
    assert not (tmp_path / "artifacts").exists()
