"""Focused tests for the hardened SoraFS CLI release-signing wrapper."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "release_sorafs_cli.sh"
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


def write_executable(path: Path, body: str) -> Path:
    path.write_text("#!/usr/bin/env python3\n" + body, encoding="utf-8")
    path.chmod(0o700)
    return path


def write_inputs(tmp_path: Path) -> tuple[Path, Path, Path, str, Path]:
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
    public_key = tmp_path / "release-public.raw"
    public_key.write_bytes(TEST_PUBLIC_KEY)
    public_key.chmod(0o600)
    signer = write_executable(
        tmp_path / "external-ed25519-signer",
        "import sys\n"
        "from pathlib import Path\n"
        f"Path(sys.argv[2]).write_bytes(bytes.fromhex({TEST_SIGNATURE.hex()!r}))\n",
    )
    invocation_log = tmp_path / "native-verifier-invocations.log"
    verifier = write_executable(
        tmp_path / "sorafs-validate",
        "import sys\n"
        "from pathlib import Path\n"
        f"log = Path({str(invocation_log)!r})\n"
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
    verifier_digest = hashlib.sha256(verifier.read_bytes()).hexdigest()
    return manifest, signer, public_key, verifier_digest, verifier


def required_args(tmp_path: Path) -> list[str]:
    manifest, signer, public_key, verifier_digest, verifier = write_inputs(tmp_path)
    return [
        "--workspace",
        str(tmp_path),
        "--manifest",
        str(manifest),
        "--external-signer",
        str(signer),
        "--signing-public-key",
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


def test_release_wrapper_signs_and_verifies_with_pinned_native_validator(
    tmp_path: Path,
) -> None:
    result = run_script(*required_args(tmp_path))

    output_dir = tmp_path / "artifacts" / "sorafs_cli_release"
    signature = output_dir / "release_manifest.ed25519.sig"
    public_key = output_dir / "release_manifest.ed25519.pub"
    receipt = output_dir / "release_manifest.verify.json"
    invocation_log = tmp_path / "native-verifier-invocations.log"

    assert result.returncode == 0, result.stderr
    assert signature.read_bytes() == TEST_SIGNATURE
    assert public_key.read_bytes() == TEST_PUBLIC_KEY
    summary = json.loads(receipt.read_text(encoding="utf-8"))
    assert summary["signature_algorithm"] == "ed25519"
    assert summary["signer_fingerprint_sha256"] == TEST_FINGERPRINT
    assert summary["signature_verified"] is True
    assert summary["native_verifier_sha256"] == hashlib.sha256(
        (tmp_path / "sorafs-validate").read_bytes()
    ).hexdigest()
    for output in (signature, public_key, receipt):
        assert output.stat().st_mode & 0o077 == 0
    assert invocation_log.read_text(encoding="utf-8").splitlines() == [
        "release-manifest",
        "release-manifest",
    ]


def test_release_wrapper_requires_all_governed_signing_inputs(tmp_path: Path) -> None:
    result = run_script("--workspace", str(tmp_path))

    assert result.returncode == 1
    assert "required release signing options are missing" in result.stderr
    assert "--external-signer" in result.stderr
    assert "--trusted-signing-fingerprint" in result.stderr
    assert "--release-manifest-verifier" in result.stderr
    assert "--trusted-release-manifest-verifier-sha256" in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_removed_oidc_bundle_interface(
    tmp_path: Path,
) -> None:
    result = run_script(*required_args(tmp_path), "--identity-token", "not-a-token")

    assert result.returncode == 1
    assert "unknown argument: --identity-token" in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_missing_option_value_without_shell_error(
    tmp_path: Path,
) -> None:
    result = run_script(*required_args(tmp_path), "--signature-out")

    assert result.returncode == 1
    assert "error: --signature-out requires a value" in result.stderr
    assert "unbound variable" not in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_unreviewed_signing_fingerprint(
    tmp_path: Path,
) -> None:
    args = required_args(tmp_path)
    fingerprint_index = args.index("--trusted-signing-fingerprint") + 1
    args[fingerprint_index] = "00" * 32

    result = run_script(*args)

    assert result.returncode == 1
    assert "public key does not match the reviewed fingerprint" in result.stderr
    output_dir = tmp_path / "artifacts" / "sorafs_cli_release"
    assert not (output_dir / "release_manifest.ed25519.sig").exists()
    assert not (output_dir / "release_manifest.ed25519.pub").exists()
    assert not (output_dir / "release_manifest.verify.json").exists()


def test_release_wrapper_rejects_unreviewed_native_verifier(
    tmp_path: Path,
) -> None:
    args = required_args(tmp_path)
    digest_index = args.index("--trusted-release-manifest-verifier-sha256") + 1
    args[digest_index] = "00" * 32

    result = run_script(*args)

    assert result.returncode == 1
    assert "native release-manifest verifier does not match the reviewed SHA256" in (
        result.stderr
    )
    output_dir = tmp_path / "artifacts" / "sorafs_cli_release"
    assert not (output_dir / "release_manifest.ed25519.sig").exists()
    assert not (output_dir / "release_manifest.ed25519.pub").exists()
    assert not (output_dir / "release_manifest.verify.json").exists()


def test_release_wrapper_rejects_symlinked_signature_output(tmp_path: Path) -> None:
    target = tmp_path / "signature-target"
    target.write_bytes(b"unchanged")
    signature = tmp_path / "release.sig"
    signature.symlink_to(target)

    result = run_script(
        *required_args(tmp_path),
        "--signature-out",
        str(signature),
    )

    assert result.returncode == 1
    assert "release signature output must not be a symlink" in result.stderr
    assert target.read_bytes() == b"unchanged"


def test_release_wrapper_rejects_existing_verification_receipt(
    tmp_path: Path,
) -> None:
    receipt = tmp_path / "verification.json"
    receipt.write_text("do not overwrite", encoding="utf-8")

    result = run_script(
        *required_args(tmp_path),
        "--verification-summary-out",
        str(receipt),
    )

    assert result.returncode == 1
    assert "release verification summary output must not already exist" in result.stderr
    assert receipt.read_text(encoding="utf-8") == "do not overwrite"


@pytest.mark.parametrize("link_kind", ["symlink", "hardlink"])
def test_release_wrapper_rejects_receipt_link_created_during_signing(
    tmp_path: Path,
    link_kind: str,
) -> None:
    manifest, _signer, public_key, verifier_digest, verifier = write_inputs(tmp_path)
    receipt = tmp_path / "verification.json"
    sentinel = tmp_path / "receipt-target"
    sentinel.write_text("do not overwrite", encoding="utf-8")
    link_statement = (
        f"Path({str(receipt)!r}).symlink_to(Path({str(sentinel)!r}))\n"
        if link_kind == "symlink"
        else f"os.link(Path({str(sentinel)!r}), Path({str(receipt)!r}))\n"
    )
    racing_signer = write_executable(
        tmp_path / "racing-ed25519-signer",
        "import os\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"Path(sys.argv[2]).write_bytes(bytes.fromhex({TEST_SIGNATURE.hex()!r}))\n"
        + link_statement,
    )
    result = run_script(
        "--workspace",
        str(tmp_path),
        "--manifest",
        str(manifest),
        "--external-signer",
        str(racing_signer),
        "--signing-public-key",
        str(public_key),
        "--trusted-signing-fingerprint",
        TEST_FINGERPRINT,
        "--release-manifest-verifier",
        str(verifier),
        "--trusted-release-manifest-verifier-sha256",
        verifier_digest,
        "--verification-summary-out",
        str(receipt),
    )

    assert result.returncode == 1
    assert "cannot create aggregate verification-summary output" in result.stderr
    assert sentinel.read_text(encoding="utf-8") == "do not overwrite"
    output_dir = tmp_path / "artifacts" / "sorafs_cli_release"
    assert not (output_dir / "release_manifest.ed25519.sig").exists()
    assert not (output_dir / "release_manifest.ed25519.pub").exists()
