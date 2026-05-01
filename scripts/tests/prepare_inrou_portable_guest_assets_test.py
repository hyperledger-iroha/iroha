"""Tests for scripts/ci/prepare_inrou_portable_guest_assets.py."""

from __future__ import annotations

import importlib.util
import hashlib
import os
import subprocess
import sys
import urllib.error
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "ci"
    / "prepare_inrou_portable_guest_assets.py"
)
SPEC = importlib.util.spec_from_file_location(
    "prepare_inrou_portable_guest_assets", MODULE_PATH
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_default_image_base_url_uses_pinned_bookworm_build() -> None:
    assert MODULE.default_image_base_url().endswith("/bookworm/20260413-2447")
    assert "/latest" not in MODULE.default_image_base_url()


def test_debian_archive_name_uses_pinned_build_suffix() -> None:
    assert (
        MODULE.debian_archive_name("arm64")
        == "debian-12-genericcloud-arm64-20260413-2447.tar.xz"
    )


def test_resolve_debian_keyrings_rejects_missing_explicit_path(tmp_path: Path) -> None:
    missing = tmp_path / "missing.gpg"

    try:
        MODULE.resolve_debian_keyrings(missing)
    except SystemExit as error:
        assert "--debian-keyring" in str(error)
        assert str(missing) in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("missing explicit keyring path was accepted")


def test_resolve_debian_keyrings_accepts_pathsep_env(
    monkeypatch, tmp_path: Path
) -> None:
    keyring_a = tmp_path / "archive.gpg"
    keyring_b = tmp_path / "cloud.gpg"
    keyring_a.write_bytes(b"archive")
    keyring_b.write_bytes(b"cloud")
    monkeypatch.setenv(
        MODULE.DEBIAN_KEYRING_ENV,
        os.pathsep.join([str(tmp_path / "missing.gpg"), str(keyring_a), str(keyring_b)]),
    )

    assert MODULE.resolve_debian_keyrings() == [
        keyring_a.resolve(),
        keyring_b.resolve(),
    ]


def test_verify_signed_sums_uses_gpgv_with_all_keyrings(
    monkeypatch, tmp_path: Path
) -> None:
    calls = []

    def fake_run(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
        calls.append(args)
        return subprocess.CompletedProcess(args, 0, "", "")

    monkeypatch.setattr(MODULE, "run", fake_run)
    sums = tmp_path / "SHA512SUMS"
    signature = tmp_path / "SHA512SUMS.sign"
    keyrings = [tmp_path / "archive.gpg", tmp_path / "cloud.gpg"]

    MODULE.verify_signed_sums(sums, signature, keyrings, "/usr/bin/gpgv")

    assert calls == [
        [
            "/usr/bin/gpgv",
            "--keyring",
            str(keyrings[0]),
            "--keyring",
            str(keyrings[1]),
            str(signature),
            str(sums),
        ]
    ]


def test_download_optional_signature_returns_false_for_missing_signature(
    monkeypatch, tmp_path: Path
) -> None:
    def fake_download(url: str, destination: Path) -> None:
        raise urllib.error.HTTPError(url, 404, "not found", {}, None)

    monkeypatch.setattr(MODULE, "download", fake_download)

    assert (
        MODULE.download_optional_signature(
            "https://example.invalid/SHA512SUMS.sign",
            tmp_path / "SHA512SUMS.sign",
        )
        is False
    )


def test_verify_debian_sums_signature_skips_gpg_when_signature_missing(
    monkeypatch, tmp_path: Path
) -> None:
    monkeypatch.setattr(MODULE, "download_optional_signature", lambda *_args: False)

    def fail_find_gpg_tool() -> str:
        raise AssertionError("gpg should not be required when falling back to pinned hash")

    monkeypatch.setattr(MODULE, "find_gpg_tool", fail_find_gpg_tool)

    assert (
        MODULE.verify_debian_sums_signature_if_available(
            "https://example.invalid",
            tmp_path / "SHA512SUMS",
            tmp_path / "SHA512SUMS.sign",
            None,
        )
        is False
    )


def test_verify_debian_sums_signature_verifies_when_signature_exists(
    monkeypatch, tmp_path: Path
) -> None:
    calls = []
    keyring = tmp_path / "archive.gpg"
    keyring.write_bytes(b"keyring")

    monkeypatch.setattr(MODULE, "download_optional_signature", lambda *_args: True)
    monkeypatch.setattr(MODULE, "find_gpg_tool", lambda: "/usr/bin/gpgv")
    monkeypatch.setattr(MODULE, "resolve_debian_keyrings", lambda configured: [keyring])

    def fake_verify_signed_sums(
        sums_path: Path,
        signature_path: Path,
        keyrings: list[Path],
        gpg_tool: str,
    ) -> None:
        calls.append((sums_path, signature_path, keyrings, gpg_tool))

    monkeypatch.setattr(MODULE, "verify_signed_sums", fake_verify_signed_sums)
    sums = tmp_path / "SHA512SUMS"
    signature = tmp_path / "SHA512SUMS.sign"

    assert (
        MODULE.verify_debian_sums_signature_if_available(
            "https://example.invalid", sums, signature, keyring
        )
        is True
    )
    assert calls == [(sums, signature, [keyring], "/usr/bin/gpgv")]


def test_verify_pinned_archive_accepts_matching_digest(
    monkeypatch, tmp_path: Path
) -> None:
    archive = tmp_path / "archive.tar.xz"
    archive.write_bytes(b"pinned archive")
    digest = hashlib.sha512(b"pinned archive").hexdigest()
    monkeypatch.setattr(MODULE, "PINNED_ARCHIVE_SHA512", {archive.name: digest})

    MODULE.verify_pinned_archive(archive)


def test_verify_pinned_archive_rejects_unpinned_archive(tmp_path: Path) -> None:
    archive = tmp_path / "archive.tar.xz"
    archive.write_bytes(b"unpinned archive")

    try:
        MODULE.verify_pinned_archive(archive)
    except SystemExit as error:
        assert "no pinned SHA512 digest" in str(error)
        assert "refusing to use unsigned Debian guest assets" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("unsigned unpinned archive was accepted")


def test_verify_signed_sums_uses_gpg_without_default_keyring(
    monkeypatch, tmp_path: Path
) -> None:
    calls = []

    def fake_run(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
        calls.append(args)
        return subprocess.CompletedProcess(args, 0, "", "")

    monkeypatch.setattr(MODULE, "run", fake_run)
    sums = tmp_path / "SHA512SUMS"
    signature = tmp_path / "SHA512SUMS.sign"
    keyring = tmp_path / "archive.gpg"

    MODULE.verify_signed_sums(sums, signature, [keyring], "/usr/bin/gpg")

    assert calls == [
        [
            "/usr/bin/gpg",
            "--batch",
            "--no-default-keyring",
            "--keyring",
            str(keyring),
            "--verify",
            str(signature),
            str(sums),
        ]
    ]
