"""Tests for scripts/ci/prepare_inrou_portable_guest_assets.py."""

from __future__ import annotations

import importlib.util
import hashlib
import io
import os
import subprocess
import struct
import sys
import tarfile
import urllib.error
import uuid
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


def _gpt_entry(type_guid: str, first_lba: int, last_lba: int) -> bytes:
    entry = bytearray(128)
    entry[:16] = uuid.UUID(type_guid).bytes_le
    struct.pack_into("<Q", entry, 32, first_lba)
    struct.pack_into("<Q", entry, 40, last_lba)
    return bytes(entry)


def _write_gpt_disk(path: Path, entries: list[bytes]) -> None:
    image = bytearray(MODULE.SECTOR_SIZE * 8)
    header = bytearray(MODULE.SECTOR_SIZE)
    header[:8] = b"EFI PART"
    struct.pack_into("<Q", header, 72, 2)
    struct.pack_into("<I", header, 80, len(entries))
    struct.pack_into("<I", header, 84, 128)
    image[MODULE.SECTOR_SIZE : MODULE.SECTOR_SIZE * 2] = header
    image[MODULE.SECTOR_SIZE * 2 : MODULE.SECTOR_SIZE * 2 + len(entries) * 128] = b"".join(entries)
    path.write_bytes(image)


def test_default_image_base_url_uses_pinned_bookworm_build() -> None:
    assert MODULE.default_image_base_url().endswith("/bookworm/20260413-2447")
    assert "/latest" not in MODULE.default_image_base_url()


def test_debian_archive_name_uses_pinned_build_suffix() -> None:
    assert (
        MODULE.debian_archive_name("arm64")
        == "debian-12-genericcloud-arm64-20260413-2447.tar.xz"
    )


def test_parse_args_uses_host_arch_and_tmpdir_for_default_output(
    monkeypatch, tmp_path: Path
) -> None:
    monkeypatch.setattr(MODULE, "host_asset_arch", lambda: ("arm64", "aarch64", "rootfs-aarch64"))
    monkeypatch.setenv("TMPDIR", str(tmp_path))
    monkeypatch.setattr(sys, "argv", ["prepare"])

    args = MODULE.parse_args()

    assert args.output_dir == tmp_path / "iroha-inrou-portable-assets" / "aarch64"
    assert args.image_base_url == MODULE.default_image_base_url()
    assert args.force is False
    assert args.print_env is False


def test_parse_args_accepts_overrides(monkeypatch, tmp_path: Path) -> None:
    keyring = tmp_path / "archive.gpg"
    output_dir = tmp_path / "assets"
    monkeypatch.setattr(MODULE, "host_asset_arch", lambda: ("amd64", "x86_64", "rootfs-x86_64"))
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "prepare",
            "--output-dir",
            str(output_dir),
            "--force",
            "--print-env",
            "--image-base-url",
            "https://images.example/debian",
            "--debian-keyring",
            str(keyring),
        ],
    )

    args = MODULE.parse_args()

    assert args.output_dir == output_dir
    assert args.force is True
    assert args.print_env is True
    assert args.image_base_url == "https://images.example/debian"
    assert args.debian_keyring == keyring


def test_host_asset_arch_maps_x86_64(monkeypatch) -> None:
    monkeypatch.setattr(MODULE.platform, "machine", lambda: "x86_64")

    assert MODULE.host_asset_arch() == ("amd64", "x86_64", "rootfs-x86_64")


def test_host_asset_arch_maps_arm64(monkeypatch) -> None:
    monkeypatch.setattr(MODULE.platform, "machine", lambda: "arm64")

    assert MODULE.host_asset_arch() == ("arm64", "aarch64", "rootfs-aarch64")


def test_host_asset_arch_rejects_unsupported_arch(monkeypatch) -> None:
    monkeypatch.setattr(MODULE.platform, "machine", lambda: "mips64")

    try:
        MODULE.host_asset_arch()
    except SystemExit as error:
        assert "unsupported host architecture" in str(error)
        assert "mips64" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("unsupported host architecture was accepted")


def test_find_tool_prefers_path_resolution(monkeypatch) -> None:
    monkeypatch.setattr(MODULE.shutil, "which", lambda name: f"/usr/bin/{name}")

    assert MODULE.find_tool("debugfs") == "/usr/bin/debugfs"


def test_find_tool_falls_back_to_known_e2fsprogs_locations(monkeypatch) -> None:
    expected = "/opt/homebrew/sbin/debugfs"
    monkeypatch.setattr(MODULE.shutil, "which", lambda _name: None)
    monkeypatch.setattr(MODULE.Path, "is_file", lambda self: str(self) == expected)
    monkeypatch.setattr(MODULE.os, "access", lambda path, _mode: str(path) == expected)

    assert MODULE.find_tool("debugfs") == expected


def test_find_tool_reports_missing_tool(monkeypatch) -> None:
    monkeypatch.setattr(MODULE.shutil, "which", lambda _name: None)
    monkeypatch.setattr(MODULE.Path, "is_file", lambda _self: False)

    try:
        MODULE.find_tool("debugfs")
    except SystemExit as error:
        assert "required e2fsprogs tool `debugfs` was not found" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("missing e2fsprogs tool was accepted")


def test_find_gpg_tool_prefers_gpgv_then_gpg(monkeypatch) -> None:
    monkeypatch.setattr(
        MODULE.shutil,
        "which",
        lambda name: "/usr/bin/gpg" if name == "gpg" else None,
    )

    assert MODULE.find_gpg_tool() == "/usr/bin/gpg"


def test_find_gpg_tool_reports_missing_verifier(monkeypatch) -> None:
    monkeypatch.setattr(MODULE.shutil, "which", lambda _name: None)

    try:
        MODULE.find_gpg_tool()
    except SystemExit as error:
        assert "required GPG verifier" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("missing GPG verifier was accepted")


def test_run_invokes_subprocess_with_text_capture(monkeypatch) -> None:
    calls = []

    def fake_run(
        args: list[str],
        *,
        check: bool,
        text: bool,
        capture_output: bool,
    ) -> subprocess.CompletedProcess[str]:
        calls.append((args, check, text, capture_output))
        return subprocess.CompletedProcess(args, 0, "stdout", "stderr")

    monkeypatch.setattr(MODULE.subprocess, "run", fake_run)

    result = MODULE.run(["tool", "--flag"], check=False)

    assert result.stdout == "stdout"
    assert calls == [(["tool", "--flag"], False, True, True)]


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


def test_resolve_debian_keyrings_reports_when_no_keyrings_exist(
    monkeypatch, tmp_path: Path
) -> None:
    monkeypatch.delenv(MODULE.DEBIAN_KEYRING_ENV, raising=False)
    monkeypatch.setattr(
        MODULE,
        "DEBIAN_KEYRING_CANDIDATES",
        (str(tmp_path / "missing-archive.gpg"), str(tmp_path / "missing-cloud.gpg")),
    )

    try:
        MODULE.resolve_debian_keyrings()
    except SystemExit as error:
        message = str(error)
        assert "Debian SHA512SUMS signature verification requires" in message
        assert MODULE.DEBIAN_KEYRING_ENV in message
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("missing Debian keyrings were accepted")


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


def test_download_optional_signature_returns_true_when_download_succeeds(
    monkeypatch, tmp_path: Path
) -> None:
    calls = []

    def fake_download(url: str, destination: Path) -> None:
        calls.append((url, destination))
        destination.write_bytes(b"signature")

    monkeypatch.setattr(MODULE, "download", fake_download)
    destination = tmp_path / "SHA512SUMS.sign"

    assert MODULE.download_optional_signature("https://example.invalid/sig", destination) is True
    assert calls == [("https://example.invalid/sig", destination)]
    assert destination.read_bytes() == b"signature"


def test_download_reuses_existing_destination_without_network(monkeypatch, tmp_path: Path) -> None:
    destination = tmp_path / "asset.tar.xz"
    destination.write_bytes(b"existing")

    def fail_urlopen(_url: str):
        raise AssertionError("download should not fetch existing destination")

    monkeypatch.setattr(MODULE.urllib.request, "urlopen", fail_urlopen)

    MODULE.download("https://example.invalid/asset.tar.xz", destination)

    assert destination.read_bytes() == b"existing"


def test_download_writes_temporary_file_then_replaces_destination(
    monkeypatch, tmp_path: Path
) -> None:
    class FakeResponse(io.BytesIO):
        def __enter__(self) -> "FakeResponse":
            return self

        def __exit__(self, *_args) -> None:
            self.close()

    calls = []

    def fake_urlopen(url: str) -> FakeResponse:
        calls.append(url)
        return FakeResponse(b"downloaded")

    monkeypatch.setattr(MODULE.urllib.request, "urlopen", fake_urlopen)
    destination = tmp_path / "nested" / "asset.tar.xz"

    MODULE.download("https://example.invalid/asset.tar.xz", destination)

    assert calls == ["https://example.invalid/asset.tar.xz"]
    assert destination.read_bytes() == b"downloaded"
    assert not destination.with_suffix(".xz.tmp").exists()


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


def test_sha512_hashes_large_files_in_chunks(tmp_path: Path) -> None:
    payload = (b"0123456789abcdef" * 70_000) + b"tail"
    archive = tmp_path / "large.tar.xz"
    archive.write_bytes(payload)

    assert MODULE.sha512(archive) == hashlib.sha512(payload).hexdigest()


def test_verify_pinned_archive_rejects_digest_mismatch(
    monkeypatch, tmp_path: Path
) -> None:
    archive = tmp_path / "archive.tar.xz"
    archive.write_bytes(b"changed archive")
    monkeypatch.setattr(MODULE, "PINNED_ARCHIVE_SHA512", {archive.name: "0" * 128})

    try:
        MODULE.verify_pinned_archive(archive)
    except SystemExit as error:
        assert "pinned SHA512 mismatch" in str(error)
        assert str(archive) in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("archive with mismatched pinned digest was accepted")


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


def test_verify_signed_sums_reports_verifier_failure(
    monkeypatch, tmp_path: Path
) -> None:
    def fake_run(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
        raise subprocess.CalledProcessError(1, args, stderr="bad signature")

    monkeypatch.setattr(MODULE, "run", fake_run)
    sums = tmp_path / "SHA512SUMS"
    signature = tmp_path / "SHA512SUMS.sign"
    keyring = tmp_path / "archive.gpg"

    try:
        MODULE.verify_signed_sums(sums, signature, [keyring], "/usr/bin/gpgv")
    except SystemExit as error:
        assert "failed to verify Debian SHA512SUMS signature" in str(error)
        assert "bad signature" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("GPG verification failure was accepted")


def test_verify_archive_accepts_star_prefixed_relative_filename(tmp_path: Path) -> None:
    archive = tmp_path / "debian-12-genericcloud-amd64-20260413-2447.tar.xz"
    archive.write_bytes(b"archive")
    sums = tmp_path / "SHA512SUMS"
    sums.write_text(
        f"{hashlib.sha512(b'archive').hexdigest()}  *./{archive.name}\n",
        encoding="utf-8",
    )

    MODULE.verify_archive(archive, sums)


def test_verify_archive_rejects_missing_archive_entry(tmp_path: Path) -> None:
    archive = tmp_path / "debian-12-genericcloud-amd64-20260413-2447.tar.xz"
    archive.write_bytes(b"archive")
    sums = tmp_path / "SHA512SUMS"
    sums.write_text(f"{hashlib.sha512(b'archive').hexdigest()}  other.tar.xz\n", encoding="utf-8")

    try:
        MODULE.verify_archive(archive, sums)
    except SystemExit as error:
        assert "does not list" in str(error)
        assert archive.name in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("archive missing from SHA512SUMS was accepted")


def test_verify_archive_rejects_checksum_mismatch(tmp_path: Path) -> None:
    archive = tmp_path / "debian-12-genericcloud-amd64-20260413-2447.tar.xz"
    archive.write_bytes(b"archive")
    sums = tmp_path / "SHA512SUMS"
    sums.write_text(f"{'0' * 128}  {archive.name}\n", encoding="utf-8")

    try:
        MODULE.verify_archive(archive, sums)
    except SystemExit as error:
        assert "SHA512 mismatch" in str(error)
        assert str(archive) in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("archive with mismatched SHA512SUMS digest was accepted")


def test_download_optional_signature_propagates_non_404_http_errors(
    monkeypatch, tmp_path: Path
) -> None:
    def fake_download(url: str, destination: Path) -> None:
        raise urllib.error.HTTPError(url, 500, "server error", {}, None)

    monkeypatch.setattr(MODULE, "download", fake_download)

    try:
        MODULE.download_optional_signature(
            "https://example.invalid/SHA512SUMS.sign",
            tmp_path / "SHA512SUMS.sign",
        )
    except urllib.error.HTTPError as error:
        assert error.code == 500
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("non-404 signature download error was swallowed")


def test_extract_disk_rejects_archive_without_disk_raw(tmp_path: Path) -> None:
    archive = tmp_path / "image.tar.xz"
    payload = tmp_path / "not-disk.txt"
    payload.write_text("not a disk", encoding="utf-8")
    with tarfile.open(archive, "w:xz") as tar:
        tar.add(payload, arcname="not-disk.txt")

    try:
        MODULE.extract_disk(archive, tmp_path / "disk.raw", force=True)
    except SystemExit as error:
        assert "does not contain disk.raw" in str(error)
        assert str(archive) in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("archive without disk.raw was accepted")


def test_extract_disk_reuses_existing_disk_when_not_forced(tmp_path: Path) -> None:
    archive = tmp_path / "missing.tar.xz"
    disk = tmp_path / "disk.raw"
    disk.write_bytes(b"existing")

    MODULE.extract_disk(archive, disk, force=False)

    assert disk.read_bytes() == b"existing"


def test_root_partition_range_selects_largest_non_efi_partition(tmp_path: Path) -> None:
    disk = tmp_path / "disk.raw"
    linux_root_guid = "0fc63daf-8483-4772-8e79-3d69d8477de4"
    _write_gpt_disk(
        disk,
        [
            _gpt_entry(MODULE.EFI_SYSTEM_PARTITION, 10, 20),
            _gpt_entry(linux_root_guid, 30, 40),
            _gpt_entry(linux_root_guid, 50, 80),
            _gpt_entry(linux_root_guid, 90, 85),
        ],
    )

    assert MODULE.root_partition_range(disk) == (
        50 * MODULE.SECTOR_SIZE,
        31 * MODULE.SECTOR_SIZE,
    )


def test_root_partition_range_rejects_missing_gpt_header(tmp_path: Path) -> None:
    disk = tmp_path / "disk.raw"
    disk.write_bytes(b"not-gpt".ljust(MODULE.SECTOR_SIZE * 2, b"\0"))

    try:
        MODULE.root_partition_range(disk)
    except SystemExit as error:
        assert "does not contain a GPT header" in str(error)
        assert str(disk) in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("disk without GPT header was accepted")


def test_root_partition_range_rejects_disk_without_non_efi_root(tmp_path: Path) -> None:
    disk = tmp_path / "disk.raw"
    _write_gpt_disk(
        disk,
        [
            _gpt_entry("00000000-0000-0000-0000-000000000000", 0, 0),
            _gpt_entry(MODULE.EFI_SYSTEM_PARTITION, 10, 20),
        ],
    )

    try:
        MODULE.root_partition_range(disk)
    except SystemExit as error:
        assert "does not contain a non-EFI root partition" in str(error)
        assert str(disk) in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("disk without root partition was accepted")


def test_copy_range_extracts_requested_slice(tmp_path: Path) -> None:
    source = tmp_path / "disk.raw"
    destination = tmp_path / "rootfs.ext4"
    source.write_bytes(b"0123456789")

    MODULE.copy_range(source, destination, offset=2, length=4, force=True)

    assert destination.read_bytes() == b"2345"


def test_copy_range_reuses_existing_destination_when_not_forced(tmp_path: Path) -> None:
    source = tmp_path / "missing.raw"
    destination = tmp_path / "rootfs.ext4"
    destination.write_bytes(b"existing")

    MODULE.copy_range(source, destination, offset=0, length=4, force=False)

    assert destination.read_bytes() == b"existing"


def test_copy_range_rejects_unexpected_eof(tmp_path: Path) -> None:
    source = tmp_path / "disk.raw"
    destination = tmp_path / "rootfs.ext4"
    source.write_bytes(b"short")

    try:
        MODULE.copy_range(source, destination, offset=2, length=8, force=True)
    except SystemExit as error:
        assert "unexpected EOF" in str(error)
        assert str(source) in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("short disk image was accepted")


def test_newest_boot_file_prefers_cloud_suffix_and_highest_name(monkeypatch, tmp_path: Path) -> None:
    listing = (
        "/vmlinuz-6.1.0-1-amd64/"
        "/vmlinuz-6.1.0-2-cloud-amd64/"
        "/vmlinuz-6.1.0-1-cloud-amd64/"
    )
    monkeypatch.setattr(MODULE, "debugfs_stdout", lambda *_args: listing)

    assert MODULE.newest_boot_file("/usr/sbin/debugfs", tmp_path / "rootfs.ext4", "vmlinuz-") == (
        "vmlinuz-6.1.0-2-cloud-amd64"
    )


def test_newest_boot_file_rejects_missing_prefix(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setattr(MODULE, "debugfs_stdout", lambda *_args: "/initrd.img-6.1.0/")
    rootfs = tmp_path / "rootfs.ext4"

    try:
        MODULE.newest_boot_file("/usr/sbin/debugfs", rootfs, "vmlinuz-")
    except SystemExit as error:
        assert "unable to find /boot/vmlinuz-*" in str(error)
        assert str(rootfs) in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("missing boot file prefix was accepted")


def test_write_env_quotes_paths_and_prints_exports(tmp_path: Path, capsys) -> None:
    kernel = tmp_path / "kernel image"
    rootfs = tmp_path / "rootfs image.ext4"
    initrd = tmp_path / "initrd image.img"

    MODULE.write_env(tmp_path, kernel, rootfs, initrd, print_env=True)

    expected_lines = [
        f"export IROHA_INROU_PORTABLE_KERNEL_IMAGE='{kernel}'",
        f"export IROHA_INROU_PORTABLE_ROOTFS_IMAGE='{rootfs}'",
        f"export IROHA_INROU_PORTABLE_INITRD_IMAGE='{initrd}'",
    ]
    assert (tmp_path / "env.sh").read_text(encoding="utf-8") == "\n".join(expected_lines) + "\n"
    assert capsys.readouterr().out == "\n".join(expected_lines) + "\n"


def test_patch_rootfs_sets_label_and_replaces_fstab(monkeypatch, tmp_path: Path) -> None:
    calls = []

    def fake_run(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
        calls.append((args, check))
        return subprocess.CompletedProcess(args, 0, "", "")

    monkeypatch.setattr(MODULE, "run", fake_run)
    rootfs = tmp_path / "rootfs.ext4"

    MODULE.patch_rootfs(rootfs, "rootfs-aarch64", "/sbin/debugfs", "/sbin/tune2fs")

    fstab = tmp_path / "fstab"
    assert fstab.read_text(encoding="utf-8") == (
        "LABEL=rootfs-aarch64 / ext4 rw,discard,errors=remount-ro,x-systemd.growfs 0 1\n"
    )
    assert calls == [
        (["/sbin/tune2fs", "-L", "rootfs-aarch64", str(rootfs)], True),
        (["/sbin/debugfs", "-w", "-R", "rm /etc/fstab", str(rootfs)], False),
        (["/sbin/debugfs", "-w", "-R", f"write {fstab} /etc/fstab", str(rootfs)], True),
    ]


def test_debugfs_stdout_returns_captured_stdout(monkeypatch, tmp_path: Path) -> None:
    calls = []

    def fake_run(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
        calls.append((args, check))
        return subprocess.CompletedProcess(args, 0, "debugfs listing", "")

    monkeypatch.setattr(MODULE, "run", fake_run)
    rootfs = tmp_path / "rootfs.ext4"

    assert MODULE.debugfs_stdout("/sbin/debugfs", "ls -p /boot", rootfs) == "debugfs listing"
    assert calls == [(["/sbin/debugfs", "-R", "ls -p /boot", str(rootfs)], True)]


def test_dump_boot_file_replaces_existing_destination(monkeypatch, tmp_path: Path) -> None:
    calls = []

    def fake_run(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
        calls.append(args)
        destination = Path(args[2].split()[-1])
        assert not destination.exists()
        destination.write_bytes(b"dumped")
        return subprocess.CompletedProcess(args, 0, "", "")

    monkeypatch.setattr(MODULE, "run", fake_run)
    rootfs = tmp_path / "rootfs.ext4"
    destination = tmp_path / "vmlinux"
    destination.write_bytes(b"stale")

    MODULE.dump_boot_file("/sbin/debugfs", rootfs, "vmlinuz-cloud", destination)

    assert destination.read_bytes() == b"dumped"
    assert calls == [
        ["/sbin/debugfs", "-R", f"dump -p /boot/vmlinuz-cloud {destination}", str(rootfs)]
    ]


def test_main_orchestrates_unsigned_pinned_asset_flow(monkeypatch, tmp_path: Path) -> None:
    output_dir = tmp_path / "assets"
    keyring = tmp_path / "archive.gpg"
    calls = []

    monkeypatch.setattr(
        MODULE,
        "parse_args",
        lambda: MODULE.argparse.Namespace(
            output_dir=output_dir,
            force=True,
            print_env=True,
            image_base_url="https://images.example/base///",
            debian_keyring=keyring,
        ),
    )
    monkeypatch.setattr(MODULE, "host_asset_arch", lambda: ("amd64", "x86_64", "rootfs-x86_64"))
    monkeypatch.setattr(MODULE, "find_tool", lambda name: f"/tools/{name}")

    def record_download(url: str, destination: Path) -> None:
        calls.append(("download", url, destination.name))

    monkeypatch.setattr(MODULE, "download", record_download)

    def fake_verify_signature(base_url: str, sums_path: Path, signature_path: Path, configured: Path) -> bool:
        calls.append(("verify_signature", base_url, sums_path.name, signature_path.name, configured))
        return False

    monkeypatch.setattr(MODULE, "verify_debian_sums_signature_if_available", fake_verify_signature)
    monkeypatch.setattr(
        MODULE,
        "verify_archive",
        lambda *_args: (_ for _ in ()).throw(AssertionError("verified flow should not run")),
    )
    monkeypatch.setattr(
        MODULE,
        "verify_pinned_archive",
        lambda archive: calls.append(("verify_pinned", archive.name)),
    )
    monkeypatch.setattr(
        MODULE,
        "extract_disk",
        lambda archive, disk, force: calls.append(("extract_disk", archive.name, disk.name, force)),
    )
    monkeypatch.setattr(MODULE, "root_partition_range", lambda disk: calls.append(("root_range", disk.name)) or (64, 128))
    monkeypatch.setattr(
        MODULE,
        "copy_range",
        lambda disk, rootfs, offset, length, force: calls.append(
            ("copy_range", disk.name, rootfs.name, offset, length, force)
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "patch_rootfs",
        lambda rootfs, label, debugfs, tune2fs: calls.append(
            ("patch_rootfs", rootfs.name, label, debugfs, tune2fs)
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "newest_boot_file",
        lambda debugfs, rootfs, prefix: calls.append(("newest_boot_file", debugfs, rootfs.name, prefix))
        or f"{prefix}cloud",
    )
    monkeypatch.setattr(
        MODULE,
        "dump_boot_file",
        lambda debugfs, rootfs, source, destination: calls.append(
            ("dump_boot_file", debugfs, rootfs.name, source, destination.name)
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "write_env",
        lambda out, kernel, rootfs, initrd, print_env: calls.append(
            ("write_env", out, kernel.name, rootfs.name, initrd.name, print_env)
        ),
    )

    MODULE.main()

    archive_name = MODULE.debian_archive_name("amd64")
    assert output_dir.is_dir()
    assert calls == [
        ("download", "https://images.example/base/SHA512SUMS", "SHA512SUMS"),
        ("verify_signature", "https://images.example/base", "SHA512SUMS", "SHA512SUMS.sign", keyring),
        ("download", f"https://images.example/base/{archive_name}", archive_name),
        ("verify_pinned", archive_name),
        ("extract_disk", archive_name, "disk.raw", True),
        ("root_range", "disk.raw"),
        ("copy_range", "disk.raw", "rootfs-x86_64.ext4", 64, 128, True),
        ("patch_rootfs", "rootfs-x86_64.ext4", "rootfs-x86_64", "/tools/debugfs", "/tools/tune2fs"),
        ("newest_boot_file", "/tools/debugfs", "rootfs-x86_64.ext4", "vmlinuz-"),
        ("newest_boot_file", "/tools/debugfs", "rootfs-x86_64.ext4", "initrd.img-"),
        ("dump_boot_file", "/tools/debugfs", "rootfs-x86_64.ext4", "vmlinuz-cloud", "vmlinux-x86_64"),
        (
            "dump_boot_file",
            "/tools/debugfs",
            "rootfs-x86_64.ext4",
            "initrd.img-cloud",
            "initrd-x86_64.img",
        ),
        ("write_env", output_dir.resolve(), "vmlinux-x86_64", "rootfs-x86_64.ext4", "initrd-x86_64.img", True),
    ]


def test_main_uses_verified_sums_when_signature_is_available(monkeypatch, tmp_path: Path) -> None:
    output_dir = tmp_path / "assets"
    calls = []

    monkeypatch.setattr(
        MODULE,
        "parse_args",
        lambda: MODULE.argparse.Namespace(
            output_dir=output_dir,
            force=False,
            print_env=False,
            image_base_url="https://images.example/base",
            debian_keyring=None,
        ),
    )
    monkeypatch.setattr(MODULE, "host_asset_arch", lambda: ("arm64", "aarch64", "rootfs-aarch64"))
    monkeypatch.setattr(MODULE, "find_tool", lambda name: f"/tools/{name}")
    monkeypatch.setattr(MODULE, "download", lambda url, destination: calls.append(("download", url, destination.name)))
    monkeypatch.setattr(MODULE, "verify_debian_sums_signature_if_available", lambda *_args: True)
    monkeypatch.setattr(
        MODULE,
        "verify_archive",
        lambda archive, sums: calls.append(("verify_archive", archive.name, sums.name)),
    )
    monkeypatch.setattr(
        MODULE,
        "verify_pinned_archive",
        lambda *_args: (_ for _ in ()).throw(AssertionError("pinned fallback should not run")),
    )
    monkeypatch.setattr(MODULE, "extract_disk", lambda *_args: None)
    monkeypatch.setattr(MODULE, "root_partition_range", lambda _disk: (0, 1))
    monkeypatch.setattr(MODULE, "copy_range", lambda *_args: None)
    monkeypatch.setattr(MODULE, "patch_rootfs", lambda *_args: None)
    monkeypatch.setattr(
        MODULE,
        "newest_boot_file",
        lambda _debugfs, _rootfs, prefix: "vmlinuz-cloud"
        if prefix == "vmlinuz-"
        else "initrd.img-cloud",
    )
    monkeypatch.setattr(MODULE, "dump_boot_file", lambda *_args: None)
    monkeypatch.setattr(MODULE, "write_env", lambda *_args: None)

    MODULE.main()

    archive_name = MODULE.debian_archive_name("arm64")
    assert calls == [
        ("download", "https://images.example/base/SHA512SUMS", "SHA512SUMS"),
        ("download", f"https://images.example/base/{archive_name}", archive_name),
        ("verify_archive", archive_name, "SHA512SUMS"),
    ]
