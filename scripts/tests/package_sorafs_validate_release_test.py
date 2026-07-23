"""Tests for scripts/package_sorafs_validate_release.sh."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "package_sorafs_validate_release.sh"

FAKE_RELEASE_MANIFEST_HANDLER = r"""
import hashlib
import sys
from pathlib import Path

def _release_fail(message):
    print(message, file=sys.stderr)
    raise SystemExit(2)

def _release_signature(public_key, manifest):
    return hashlib.sha512(
        b"sorafs-test-release-signature-v1\x00" + public_key + manifest
    ).digest()

def _release_option_values(arguments):
    values = {}
    development = False
    index = 0
    value_options = {
        "--manifest",
        "--public-key",
        "--public-key-fingerprint",
        "--signature",
        "--signing-seed",
        "--signature-out",
    }
    while index < len(arguments):
        option = arguments[index]
        if option == "--development-local-signing":
            if development:
                _release_fail("duplicate development signing flag")
            development = True
            index += 1
            continue
        if option not in value_options or index + 1 >= len(arguments):
            _release_fail(f"unsupported fake release-manifest option: {option}")
        if option in values:
            _release_fail(f"duplicate fake release-manifest option: {option}")
        values[option] = arguments[index + 1]
        index += 2
    return values, development

if len(sys.argv) > 1 and sys.argv[1] == "release-manifest":
    values, development = _release_option_values(sys.argv[2:])
    for required in ("--manifest", "--public-key", "--public-key-fingerprint"):
        if required not in values:
            _release_fail(f"missing fake release-manifest option: {required}")
    manifest = Path(values["--manifest"]).read_bytes()
    public_key = Path(values["--public-key"]).read_bytes()
    if len(public_key) != 32:
        _release_fail("release manifest public key must contain exactly 32 raw bytes")
    if not any(public_key):
        _release_fail("release manifest public key must not be all zero")
    if public_key == b"\x01" + (b"\x00" * 31):
        _release_fail("release manifest public key must not be weak or small-order")
    fingerprint = hashlib.sha256(public_key).hexdigest()
    if values["--public-key-fingerprint"] != fingerprint:
        _release_fail(
            "release manifest public key does not match the reviewed fingerprint"
        )
    if "--signature" in values:
        if development or "--signing-seed" in values or "--signature-out" in values:
            _release_fail("external verification received development signing options")
        signature = Path(values["--signature"]).read_bytes()
        if len(signature) != 64:
            _release_fail("release manifest signature must contain exactly 64 raw bytes")
        if not any(signature):
            _release_fail("release manifest signature must not be all zero")
        if signature != _release_signature(public_key, manifest):
            _release_fail("release manifest Ed25519 signature verification failed")
        raise SystemExit(0)
    if not development or "--signing-seed" not in values or "--signature-out" not in values:
        _release_fail("development signing option set is incomplete")
    seed = Path(values["--signing-seed"]).read_bytes()
    if len(seed) != 32:
        _release_fail(
            "release manifest development signing seed must contain exactly 32 raw bytes"
        )
    derived_public = hashlib.sha256(
        b"sorafs-test-release-public-key-v1\x00" + seed
    ).digest()
    if derived_public != public_key:
        _release_fail(
            "release manifest public key does not match the development signing seed"
        )
    output = Path(values["--signature-out"])
    with output.open("xb") as handle:
        handle.write(_release_signature(public_key, manifest))
    raise SystemExit(0)
"""


def write_fake_validator(path: Path, body: str) -> Path:
    """Write an executable fake sorafs-validate binary."""

    shebang, separator, remainder = body.partition("\n")
    assert separator and shebang == "#!/usr/bin/env python3"
    path.write_text(
        f"{shebang}\n{FAKE_RELEASE_MANIFEST_HANDLER}\n{remainder}",
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path


def run_packager(
    tmp_path: Path,
    fake_binary: Path,
    *,
    out_dir: Path | None = None,
    extra_args: list[str] | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the release packager without invoking Cargo."""

    package_out_dir = out_dir or tmp_path / "out"
    command = [
        "bash",
        str(SCRIPT),
        "--workspace",
        str(REPO_ROOT),
        "--binary",
        str(fake_binary),
        "--out-dir",
        str(package_out_dir),
        "--target",
        "test-target",
        "--version",
        "test-version",
        "--skip-smoke",
    ]
    if extra_args is not None:
        command.extend(extra_args)
    command_env = os.environ.copy()
    if env is not None:
        command_env.update(env)
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        env=command_env,
        check=False,
    )


def write_signing_keypair(tmp_path: Path) -> tuple[Path, Path]:
    """Write a deterministic raw-seed/key pair for packager plumbing tests."""

    private_key = tmp_path / "manifest-private.seed"
    public_key = tmp_path / "manifest-public.key"
    seed = hashlib.sha256(
        b"sorafs-test-release-seed-v1\x00" + str(tmp_path).encode("utf-8")
    ).digest()
    private_key.write_bytes(seed)
    private_key.chmod(0o600)
    public_key.write_bytes(
        hashlib.sha256(b"sorafs-test-release-public-key-v1\x00" + seed).digest()
    )
    return private_key, public_key


def write_incompatible_keypair(tmp_path: Path) -> tuple[Path, Path]:
    """Write encoded key material that violates the strict raw-key contract."""

    private_key = tmp_path / "encoded-private.pem"
    public_key = tmp_path / "encoded-public.pem"
    private_key.write_bytes(b"-----BEGIN PRIVATE KEY-----\nnot-raw\n")
    private_key.chmod(0o600)
    public_key.write_bytes(b"-----BEGIN PUBLIC KEY-----\nnot-raw\n")
    return private_key, public_key


def public_key_fingerprint(public_key: Path) -> str:
    """Return the SHA256 fingerprint of the exact raw public-key bytes."""

    return hashlib.sha256(public_key.read_bytes()).hexdigest()


def signing_args(private_key: Path, public_key: Path) -> list[str]:
    """Build the complete reviewed manifest-signing argument set."""

    return [
        "--manifest-signing-key",
        str(private_key),
        "--development-local-signing",
        "--manifest-public-key",
        str(public_key),
        "--manifest-public-key-fingerprint",
        public_key_fingerprint(public_key),
    ]


def external_signature_args(signature: Path, public_key: Path) -> list[str]:
    """Build the complete externally signed manifest argument set."""

    return [
        "--manifest-signature-in",
        str(signature),
        "--manifest-public-key",
        str(public_key),
        "--manifest-public-key-fingerprint",
        public_key_fingerprint(public_key),
    ]


def fake_release_signature(manifest: Path, public_key: Path) -> bytes:
    """Return the fake validator's deterministic detached signature."""

    return hashlib.sha512(
        b"sorafs-test-release-signature-v1\x00"
        + public_key.read_bytes()
        + manifest.read_bytes()
    ).digest()


def test_release_packager_accepts_regular_staged_files(tmp_path: Path) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(tmp_path, fake_binary)

    assert result.returncode == 0, result.stderr
    package = tmp_path / "out" / "sorafs-validate-test-version-test-target"
    assert (tmp_path / "out" / f"{package.name}.tar.gz").is_file()
    assert (tmp_path / "out" / f"{package.name}.tar.gz.sha256").is_file()
    assert (tmp_path / "out" / f"{package.name}.sha256").is_file()
    assert (tmp_path / "out" / f"{package.name}.manifest.json").is_file()
    assert (tmp_path / "out" / f"{package.name}.manifest.json.sha256").is_file()


def test_release_packager_uses_windows_executable_name_for_windows_target(
    tmp_path: Path,
) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate.exe",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=["--target", "x86_64-pc-windows-msvc"],
    )

    assert result.returncode == 0, result.stderr
    manifest_path = (
        tmp_path
        / "out"
        / "sorafs-validate-test-version-x86_64-pc-windows-msvc.manifest.json"
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["binary"] == "sorafs-validate.exe"
    assert manifest["stage_files"][0]["path"] == "sorafs-validate.exe"
    assert (
        tmp_path
        / "out"
        / "sorafs-validate-test-version-x86_64-pc-windows-msvc"
        / "sorafs-validate.exe"
    ).is_file()


def test_release_packager_rejects_missing_option_value_without_shell_error(
    tmp_path: Path,
) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(tmp_path, fake_binary, extra_args=["--out-dir"])

    assert result.returncode != 0
    assert "error: --out-dir requires a value" in result.stderr
    assert "unbound variable" not in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_option_shaped_value_before_artifacts(
    tmp_path: Path,
) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=["--out-dir", "--version", "shadow"],
    )

    assert result.returncode != 0
    assert "error: --out-dir requires a value" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_symlinked_binary_before_artifacts(
    tmp_path: Path,
) -> None:
    target = write_fake_validator(
        tmp_path / "real-sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    binary_link = tmp_path / "sorafs-validate-link"
    binary_link.symlink_to(target)

    result = run_packager(tmp_path, binary_link)

    assert result.returncode != 0
    assert "sorafs-validate binary must not be a symlink" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_symlinked_manifest_signing_key_before_artifacts(
    tmp_path: Path,
) -> None:
    key_target = tmp_path / "real-key.pem"
    key_link = tmp_path / "key-link.pem"
    key_target.write_text("fixture key", encoding="utf-8")
    key_link.symlink_to(key_target)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=["--manifest-signing-key", str(key_link)],
    )

    assert result.returncode != 0
    assert "manifest signing key must not be a symlink" in result.stderr
    assert key_target.read_text(encoding="utf-8") == "fixture key"
    assert not (tmp_path / "out").exists()


def test_release_packager_writes_manifest_signature_through_hardened_path(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fingerprint = public_key_fingerprint(public_key)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(private_key, public_key),
    )

    assert result.returncode == 0, result.stderr
    package = tmp_path / "out" / "sorafs-validate-test-version-test-target"
    manifest = tmp_path / "out" / f"{package.name}.manifest.json"
    signature = tmp_path / "out" / f"{package.name}.manifest.json.sig"
    assert signature.is_file()
    signature_bytes = signature.read_bytes()
    assert len(signature_bytes) == 64
    assert any(signature_bytes)
    assert f"Manifest signer fingerprint (reviewed): {fingerprint}" in result.stdout
    assert signature_bytes == fake_release_signature(manifest, public_key)
    assert private_key.name not in manifest.read_text(encoding="utf-8")
    assert public_key.name not in manifest.read_text(encoding="utf-8")


def test_release_packager_pins_signing_inputs_before_packaging(
    tmp_path: Path,
) -> None:
    key_a_dir = tmp_path / "key-a"
    key_b_dir = tmp_path / "key-b"
    key_a_dir.mkdir()
    key_b_dir.mkdir()
    private_key, public_key = write_signing_keypair(key_a_dir)
    replacement_private, replacement_public = write_signing_keypair(key_b_dir)
    trusted_public = tmp_path / "trusted-public.key"
    shutil.copyfile(public_key, trusted_public)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "\n".join(
            [
                "#!/usr/bin/env python3",
                "from pathlib import Path",
                f"Path({str(private_key)!r}).write_bytes("
                f"Path({str(replacement_private)!r}).read_bytes())",
                f"Path({str(public_key)!r}).write_bytes("
                f"Path({str(replacement_public)!r}).read_bytes())",
                "print('fake help')",
                "",
            ]
        ),
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(private_key, public_key),
    )

    assert result.returncode == 0, result.stderr
    package_name = "sorafs-validate-test-version-test-target"
    manifest = tmp_path / "out" / f"{package_name}.manifest.json"
    signature = tmp_path / "out" / f"{package_name}.manifest.json.sig"
    assert signature.read_bytes() == fake_release_signature(manifest, trusted_public)
    assert public_key.read_bytes() == replacement_public.read_bytes()


def test_release_packager_verifies_external_hsm_signature(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    unsigned = run_packager(tmp_path, fake_binary)
    assert unsigned.returncode == 0, unsigned.stderr
    package_name = "sorafs-validate-test-version-test-target"
    manifest = tmp_path / "out" / f"{package_name}.manifest.json"
    manifest_before = manifest.read_bytes()
    external_signature = tmp_path / "hsm-manifest.sig"
    external_signature.write_bytes(fake_release_signature(manifest, public_key))

    signed = run_packager(
        tmp_path,
        fake_binary,
        extra_args=external_signature_args(external_signature, public_key),
    )

    assert signed.returncode == 0, signed.stderr
    assert manifest.read_bytes() == manifest_before
    installed_signature = tmp_path / "out" / f"{package_name}.manifest.json.sig"
    assert installed_signature.read_bytes() == external_signature.read_bytes()
    assert private_key.name not in manifest.read_text(encoding="utf-8")
    assert external_signature.name not in manifest.read_text(encoding="utf-8")


def test_release_packager_rejects_unsigned_rerun_with_stale_signature(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    signed = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(private_key, public_key),
    )
    assert signed.returncode == 0, signed.stderr
    package_name = "sorafs-validate-test-version-test-target"
    manifest = tmp_path / "out" / f"{package_name}.manifest.json"
    signature = tmp_path / "out" / f"{package_name}.manifest.json.sig"
    manifest_before = manifest.read_bytes()
    signature_before = signature.read_bytes()

    unsigned = run_packager(tmp_path, fake_binary)

    assert unsigned.returncode != 0
    assert "stale default manifest signature exists" in unsigned.stderr
    assert manifest.read_bytes() == manifest_before
    assert signature.read_bytes() == signature_before


def test_release_packager_rejects_two_manifest_signature_sources(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    external_signature = tmp_path / "external.sig"
    external_signature.write_bytes(b"\x01" * 64)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            *signing_args(private_key, public_key),
            "--manifest-signature-in",
            str(external_signature),
        ],
    )

    assert result.returncode != 0
    assert (
        "--manifest-signing-key and --manifest-signature-in are mutually exclusive"
        in result.stderr
    )
    assert not (tmp_path / "out").exists()


def test_release_packager_requires_public_key_for_signed_manifest(
    tmp_path: Path,
) -> None:
    private_key, _public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            "--manifest-signing-key",
            str(private_key),
            "--development-local-signing",
        ],
    )

    assert result.returncode != 0
    assert "signed manifests require --manifest-public-key" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_keeps_local_raw_seed_signing_development_only(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            "--manifest-signing-key",
            str(private_key),
            "--manifest-public-key",
            str(public_key),
            "--manifest-public-key-fingerprint",
            public_key_fingerprint(public_key),
        ],
    )

    assert result.returncode != 0
    assert "--manifest-signing-key is development-only" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_requires_reviewed_public_key_fingerprint(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            "--manifest-signing-key",
            str(private_key),
            "--development-local-signing",
            "--manifest-public-key",
            str(public_key),
        ],
    )

    assert result.returncode != 0
    assert (
        "signed manifests require --manifest-public-key-fingerprint"
        in result.stderr
    )
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_unsafe_private_key_permissions(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    private_key.chmod(0o644)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(private_key, public_key),
    )

    assert result.returncode != 0
    assert "manifest signing key permissions must be owner-only 0400 or 0600" in (
        result.stderr
    )
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_hardlinked_private_key(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    hardlink = tmp_path / "release-key-copy.pem"
    os.link(private_key, hardlink)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(hardlink, public_key),
    )

    assert result.returncode != 0
    assert "manifest signing key must have exactly one hard link" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_encoded_private_key_without_leaking_material(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_incompatible_keypair(tmp_path)
    public_key.write_bytes(b"\x33" * 32)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(private_key, public_key),
    )

    assert result.returncode != 0
    assert "manifest signing key must contain exactly 32 raw bytes" in result.stderr
    assert "-----BEGIN PRIVATE KEY-----" not in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_non_ed25519_public_key(
    tmp_path: Path,
) -> None:
    private_key, _public_key = write_signing_keypair(tmp_path)
    _encoded_private_key, encoded_public_key = write_incompatible_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(private_key, encoded_public_key),
    )

    assert result.returncode != 0
    assert "manifest public key must contain exactly 32 raw bytes" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_weak_ed25519_public_key(
    tmp_path: Path,
) -> None:
    _private_key, public_key = write_signing_keypair(tmp_path)
    public_key.write_bytes(b"\x01" + (b"\x00" * 31))
    external_signature = tmp_path / "external.sig"
    external_signature.write_bytes(b"\x01" * 64)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=external_signature_args(external_signature, public_key),
    )

    assert result.returncode != 0
    assert "release manifest public key must not be weak or small-order" in result.stderr
    assert not list((tmp_path / "out").glob("*.manifest.json.sig"))


def test_release_packager_rejects_mismatched_ed25519_keypair(
    tmp_path: Path,
) -> None:
    first_dir = tmp_path / "first"
    second_dir = tmp_path / "second"
    first_dir.mkdir()
    second_dir.mkdir()
    private_key, _public_key = write_signing_keypair(first_dir)
    _other_private_key, other_public_key = write_signing_keypair(second_dir)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(private_key, other_public_key),
    )

    assert result.returncode != 0
    assert (
        "release manifest public key does not match the development signing seed"
        in result.stderr
    )
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_mismatched_reviewed_fingerprint(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    wrong_fingerprint = "0" * 64

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            "--manifest-signing-key",
            str(private_key),
            "--development-local-signing",
            "--manifest-public-key",
            str(public_key),
            "--manifest-public-key-fingerprint",
            wrong_fingerprint,
        ],
    )

    assert result.returncode != 0
    assert (
        "manifest public key does not match the reviewed fingerprint" in result.stderr
    )
    assert public_key_fingerprint(public_key) not in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_malformed_reviewed_fingerprint(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            "--manifest-signing-key",
            str(private_key),
            "--development-local-signing",
            "--manifest-public-key",
            str(public_key),
            "--manifest-public-key-fingerprint",
            public_key_fingerprint(public_key).upper(),
        ],
    )

    assert result.returncode != 0
    assert (
        "--manifest-public-key-fingerprint must be exact lowercase 32-byte SHA256 hex"
        in result.stderr
    )
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_symlinked_manifest_public_key(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    public_key_link = tmp_path / "manifest-public-link.pem"
    public_key_link.symlink_to(public_key)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=signing_args(private_key, public_key_link),
    )

    assert result.returncode != 0
    assert "manifest public key must not be a symlink" in result.stderr
    assert not (tmp_path / "out").exists()


@pytest.mark.parametrize(
    ("signature", "expected_error"),
    [
        (b"\x01" * 63, "must contain exactly 64 raw bytes"),
        (b"\x00" * 64, "must not be all zero"),
        (
            b"\x01" * 64,
            "release manifest Ed25519 signature verification failed",
        ),
    ],
    ids=["short", "all-zero", "cryptographically-invalid"],
)
def test_release_packager_rejects_malformed_ed25519_signature(
    tmp_path: Path,
    signature: bytes,
    expected_error: str,
) -> None:
    _private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    unsigned = run_packager(tmp_path, fake_binary)
    assert unsigned.returncode == 0, unsigned.stderr
    external_signature = tmp_path / "malformed-external.sig"
    external_signature.write_bytes(signature)

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=external_signature_args(external_signature, public_key),
    )

    assert result.returncode != 0
    assert expected_error in result.stderr
    assert not list((tmp_path / "out").glob("*.manifest.json.sig"))
    assert not list((tmp_path / "out").glob(".sorafs-manifest-signature.*"))


def test_release_packager_rejects_symlinked_staged_entries(tmp_path: Path) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "\n".join(
            [
                "#!/usr/bin/env python3",
                "from pathlib import Path",
                "Path(__file__).resolve().parent.joinpath('symlinked.txt').symlink_to(__file__)",
                "print('fake help')",
                "",
            ]
        ),
    )

    result = run_packager(tmp_path, fake_binary)

    assert result.returncode != 0
    assert "release package entry" in result.stderr
    assert "symlinked.txt" in result.stderr
    assert "must not be a symlink" in result.stderr


def test_release_packager_rejects_symlinked_output_parent_before_archive(
    tmp_path: Path,
) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    target_out_dir = tmp_path / "target-out"
    target_out_dir.mkdir()
    package = target_out_dir / "sorafs-validate-test-version-test-target"
    package.mkdir()
    sentinel = package / "sentinel.txt"
    sentinel.write_text("keep", encoding="utf-8")
    out_dir = tmp_path / "out-link"
    out_dir.symlink_to(target_out_dir, target_is_directory=True)

    result = run_packager(tmp_path, fake_binary, out_dir=out_dir)

    assert result.returncode != 0
    assert "release output directory" in result.stderr
    assert "out-link" in result.stderr
    assert "must not be a symlink" in result.stderr
    assert sentinel.read_text(encoding="utf-8") == "keep"
    assert not list(target_out_dir.glob("*.tar.gz"))


def test_release_packager_rejects_symlinked_manifest_signature_output(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    linked_target = tmp_path / "linked-signature-target.sig"
    linked_target.write_text("old", encoding="utf-8")
    signature_link = tmp_path / "signature-link.sig"
    signature_link.symlink_to(linked_target)

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            *signing_args(private_key, public_key),
            "--manifest-signature-out",
            str(signature_link),
        ],
    )

    assert result.returncode != 0
    assert "release manifest signature output" in result.stderr
    assert "must not be a symlink" in result.stderr
    assert linked_target.read_text(encoding="utf-8") == "old"


def test_release_packager_does_not_clobber_existing_manifest_signature_output(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    signature_output = tmp_path / "existing-signature-output.sig"
    signature_output.write_bytes(b"preserve-unrelated-file")

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            *signing_args(private_key, public_key),
            "--manifest-signature-out",
            str(signature_output),
        ],
    )

    assert result.returncode != 0
    assert "must not already exist" in result.stderr
    assert signature_output.read_bytes() == b"preserve-unrelated-file"


@pytest.mark.parametrize(
    "collision",
    [
        "archive",
        "manifest",
        "manifest-sha",
        "binary-sha",
        "archive-sha",
        "stage-help",
    ],
)
def test_release_packager_rejects_manifest_signature_generated_path_collisions(
    tmp_path: Path,
    collision: str,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    out_dir = tmp_path / "out"
    package_name = "sorafs-validate-test-version-test-target"
    paths = {
        "archive": out_dir / f"{package_name}.tar.gz",
        "manifest": out_dir / f"{package_name}.manifest.json",
        "manifest-sha": out_dir / f"{package_name}.manifest.json.sha256",
        "binary-sha": out_dir / f"{package_name}.sha256",
        "archive-sha": out_dir / f"{package_name}.tar.gz.sha256",
        "stage-help": out_dir / package_name / "HELP.txt",
    }

    result = run_packager(
        tmp_path,
        fake_binary,
        out_dir=out_dir,
        extra_args=[
            *signing_args(private_key, public_key),
            "--manifest-signature-out",
            str(paths[collision]),
        ],
    )

    assert result.returncode != 0
    assert "must not collide with" in result.stderr
    assert not paths[collision].exists()


def test_release_packager_does_not_overwrite_runtime_manifest_key(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    original_private_key = private_key.read_bytes()
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            *signing_args(private_key, public_key),
            "--manifest-signature-out",
            str(private_key),
        ],
    )

    assert result.returncode != 0
    assert (
        "release manifest signature output must not overwrite a manifest key"
        in result.stderr
    )
    assert private_key.read_bytes() == original_private_key


def test_release_packager_rejects_manifest_signature_overwriting_manifest(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    out_dir = tmp_path / "out"
    package = out_dir / "sorafs-validate-test-version-test-target"
    manifest_path = out_dir / f"{package.name}.manifest.json"

    result = run_packager(
        tmp_path,
        fake_binary,
        out_dir=out_dir,
        extra_args=[
            *signing_args(private_key, public_key),
            "--manifest-signature-out",
            str(manifest_path),
        ],
    )

    assert result.returncode != 0
    assert "release manifest signature output" in result.stderr
    assert "must not collide with a generated release artifact" in result.stderr
