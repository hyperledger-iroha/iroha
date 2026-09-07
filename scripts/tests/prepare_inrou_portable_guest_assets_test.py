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

import pytest


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


@pytest.mark.parametrize("option", ["--image-base-url", "--debian-keyring"])
def test_parse_args_rejects_retired_trust_overrides(monkeypatch, option) -> None:
    monkeypatch.setattr(sys, "argv", ["prepare", option, "unused"])
    with pytest.raises(SystemExit):
        MODULE.parse_args()


@pytest.mark.parametrize("url", [
    "http://cloud.debian.org/images/cloud/bookworm/20260413-2447/SHA512SUMS",
    "https://example.invalid/SHA512SUMS",
    "https://cloud.debian.org/images/cloud/bookworm/latest/SHA512SUMS",
    "https://cloud.debian.org/images/cloud/bookworm/20260413-2447/SHA512SUMS?mirror=1",
])
def test_download_rejects_other_sources_even_with_cached_file(monkeypatch, tmp_path, url) -> None:
    destination = tmp_path / "SHA512SUMS"
    destination.write_bytes(b"cache")
    monkeypatch.setattr(MODULE, "open_https_download", lambda _: pytest.fail("network accessed"))
    with pytest.raises(SystemExit, match="exact repository-pinned official HTTPS"):
        MODULE.download(url, destination)
    assert destination.read_bytes() == b"cache"


def test_https_download_installs_pinned_redirect_policy_and_timeout(monkeypatch) -> None:
    class FakeOpener:
        def open(self, url, timeout):
            assert url == "https://cloud.debian.org/"
            assert timeout == 60
            return "response"

    def build(handler):
        with pytest.raises(SystemExit, match="exact official HTTPS mirror"):
            handler.redirect_request(MODULE.urllib.request.Request("https://cloud.debian.org/"), None, 302, "redirect", {}, "https://elsewhere.invalid/")
        return FakeOpener()

    monkeypatch.setattr(MODULE.urllib.request, "build_opener", build)
    assert MODULE.open_https_download("https://cloud.debian.org/") == "response"


def test_official_archive_redirect_preserves_path_and_allows_one_hop() -> None:
    initial = f"{MODULE.default_image_base_url()}/{MODULE.debian_archive_name('arm64')}"
    mirror = initial.replace("https://cloud.debian.org/", "https://laotzu.ftp.acc.umu.se/")
    handler = MODULE.PinnedDebianRedirects(initial)
    request = MODULE.urllib.request.Request(initial)
    redirected = handler.redirect_request(request, None, 302, "Found", {}, mirror)
    assert redirected.full_url == mirror
    with pytest.raises(SystemExit, match="exact official HTTPS mirror"):
        handler.redirect_request(redirected, None, 302, "Found", {}, initial)


@pytest.mark.parametrize("substitution", [
    ("https:", "http:"),
    ("laotzu.ftp.acc.umu.se", "another.ftp.acc.umu.se"),
    ("laotzu.ftp.acc.umu.se", "user@laotzu.ftp.acc.umu.se"),
    ("laotzu.ftp.acc.umu.se", "laotzu.ftp.acc.umu.se:443"),
    ("20260413-2447/", "latest/"),
    (".tar.xz", ".tar.xz?alternate=1"),
    (".tar.xz", ".tar.xz#fragment"),
])
def test_official_archive_redirect_rejects_route_changes(substitution) -> None:
    initial = f"{MODULE.default_image_base_url()}/{MODULE.debian_archive_name('arm64')}"
    mirror = initial.replace("https://cloud.debian.org/", "https://laotzu.ftp.acc.umu.se/")
    changed = mirror.replace(*substitution)
    handler = MODULE.PinnedDebianRedirects(initial)
    with pytest.raises(SystemExit, match="exact official HTTPS mirror"):
        handler.redirect_request(MODULE.urllib.request.Request(initial), None, 302, "Found", {}, changed)


@pytest.mark.parametrize("entry_style", ["plain", "binary", "relative"])
def test_checksum_manifest_agrees_with_independent_pin(monkeypatch, tmp_path, entry_style) -> None:
    name = MODULE.debian_archive_name("arm64")
    digest = "a" * 128
    monkeypatch.setattr(MODULE, "PINNED_ARCHIVE_SHA512", {name: digest})
    prefix = {"plain": " ", "binary": "*", "relative": "*./"}[entry_style]
    sums = tmp_path / "SHA512SUMS"
    sums.write_text(f"{digest} {prefix}{name}\n")
    MODULE.verify_checksum_manifest(sums, name)


@pytest.mark.parametrize("failure", ["different", "duplicate", "missing", "short", "nonhex", "extra_field"])
def test_checksum_manifest_rejects_tampering(monkeypatch, tmp_path, failure) -> None:
    name = MODULE.debian_archive_name("arm64")
    digest = "a" * 128
    monkeypatch.setattr(MODULE, "PINNED_ARCHIVE_SHA512", {name: digest})
    line = f"{digest}  {name}\n"
    content = {
        "different": line.replace(digest, "b" * 128),
        "duplicate": line + line,
        "missing": line.replace(name, "another.tar.xz"),
        "short": line.replace(digest, digest[:-1]),
        "nonhex": line.replace(digest, "z" * 128),
        "extra_field": line.rstrip() + " extra\n",
    }[failure]
    sums = tmp_path / "SHA512SUMS"
    sums.write_text(content)
    with pytest.raises(SystemExit):
        MODULE.verify_checksum_manifest(sums, name)


@pytest.mark.parametrize("failure", [None, "manifest", "archive"])
def test_main_checks_independent_pin_before_extraction(monkeypatch, tmp_path, failure) -> None:
    output = tmp_path / "assets"
    name = MODULE.debian_archive_name("arm64")
    digest = hashlib.sha512(b"archive").hexdigest()
    monkeypatch.setattr(MODULE, "PINNED_ARCHIVE_SHA512", {name: digest})
    monkeypatch.setattr(MODULE, "parse_args", lambda: MODULE.argparse.Namespace(
        output_dir=output, force=True, print_env=False,
    ))
    monkeypatch.setattr(MODULE, "host_asset_arch", lambda: ("arm64", "aarch64", "rootfs-aarch64"))
    monkeypatch.setattr(MODULE, "find_tool", lambda name: name)
    calls = []

    def download(url, path):
        assert url == f"{MODULE.default_image_base_url()}/{path.name}"
        assert path.name in ("SHA512SUMS", name)
        calls.append(path.name)
        if path.name == "SHA512SUMS":
            path.write_text(f"{('b' * 128) if failure == 'manifest' else digest}  {name}\n")
        else:
            path.write_bytes(b"bad archive" if failure == "archive" else b"archive")

    def extract(archive, disk, force):
        assert failure is None
        assert archive.read_bytes() == b"archive"
        assert force is True
        disk.write_bytes(b"disk")
        calls.append("extract")

    def copy(_disk, rootfs, *_args):
        rootfs.write_bytes(b"root")
        calls.append("copy")

    def normalize(*_args):
        assert not (output / "disk.raw").exists()
        calls.append("normalize")

    monkeypatch.setattr(MODULE, "download", download)
    monkeypatch.setattr(MODULE, "extract_disk", extract)
    monkeypatch.setattr(MODULE, "root_partition_range", lambda _: (0, 4))
    monkeypatch.setattr(MODULE, "copy_range", copy)
    monkeypatch.setattr(MODULE, "patch_rootfs", lambda *_args: calls.append("patch"))
    monkeypatch.setattr(MODULE, "normalize_rootfs", normalize)
    monkeypatch.setattr(MODULE, "newest_boot_file", lambda _tool, _root, prefix: prefix + "cloud")
    monkeypatch.setattr(MODULE, "dump_boot_file", lambda *_args: calls.append("dump"))
    monkeypatch.setattr(MODULE, "write_env", lambda *_args: calls.append("env"))
    if failure:
        with pytest.raises(SystemExit, match="SHA512"):
            MODULE.main()
        assert "extract" not in calls
        assert "env" not in calls
    else:
        MODULE.main()
        assert calls == ["SHA512SUMS", name, "extract", "copy", "patch", "normalize", "dump", "dump", "env"]


def _write_ext4_geometry(path: Path, size: int) -> None:
    """Synthetic metadata for control-flow tests, not a valid filesystem."""
    with path.open("w+b") as out:
        out.truncate(size)
        out.seek(1024)
        superblock = bytearray(1024)
        struct.pack_into("<H", superblock, 56, 0xEF53)
        struct.pack_into("<I", superblock, 24, 2)
        struct.pack_into("<I", superblock, 4, size // 4096)
        out.write(superblock)


@pytest.mark.parametrize("precheck", [0, 1])
def test_normalize_rootfs_checks_then_resizes_before_truncating(
    monkeypatch, tmp_path: Path, precheck: int
) -> None:
    assert MODULE.NORMALIZED_ROOTFS_BYTES == 1536 * 1024 * 1024
    target = 1024 * 1024
    monkeypatch.setattr(MODULE, "NORMALIZED_ROOTFS_BYTES", target)
    rootfs = tmp_path / "rootfs.ext4"
    _write_ext4_geometry(rootfs, 2 * target)
    calls = []

    def fake_run(args, *, check=True):
        staged = Path(args[1] if args[0] == "resize2fs" else args[-1])
        assert staged != rootfs
        assert rootfs.stat().st_size == 2 * target
        assert staged.stat().st_mode & 0o077 == 0
        calls.append((args[:-1] if args[0] != "resize2fs" else [args[0], args[-1]], check))
        if args[0] == "resize2fs":
            assert staged.stat().st_size == 2 * target
            assert args[-1] == "1M"
            # Simulate an implementation that resizes the filesystem but leaves
            # the containing file's length alone. Truncation must follow this.
            with staged.open("r+b") as out:
                out.seek(1024 + 4)
                out.write(struct.pack("<I", target // 4096))
        elif len(calls) == 4:
            assert staged.stat().st_size == target
        return subprocess.CompletedProcess(args, precheck if len(calls) == 1 else 0, "", "")

    monkeypatch.setattr(MODULE, "run", fake_run)
    MODULE.normalize_rootfs(rootfs, "e2fsck", "resize2fs")

    assert rootfs.stat().st_size == target
    assert calls == [
        (["e2fsck", "-f", "-p"], False),
        (["e2fsck", "-f", "-n"], True),
        (["resize2fs", "1M"], True),
        (["e2fsck", "-f", "-n"], True),
    ]
    assert not list(tmp_path.glob(".*.tmp"))


@pytest.mark.parametrize("failure", ["precheck2", "precheck4", "precheck8", "clean_check", "resize", "geometry", "short_file", "final_check", "replaced"])
def test_normalize_rootfs_failure_preserves_original(
    monkeypatch, tmp_path: Path, failure: str
) -> None:
    target = 1024 * 1024
    monkeypatch.setattr(MODULE, "NORMALIZED_ROOTFS_BYTES", target)
    rootfs = tmp_path / "rootfs.ext4"
    _write_ext4_geometry(rootfs, 2 * target)
    original = rootfs.read_bytes()
    calls = []

    def fake_run(args, *, check=True):
        calls.append(args)
        staged = Path(args[1] if args[0] == "resize2fs" else args[-1])
        if len(calls) == 1 and failure.startswith("precheck"):
            return subprocess.CompletedProcess(args, int(failure[-1]), "", "fsck failed")
        if (failure, len(calls)) in [("clean_check", 2), ("resize", 3), ("final_check", 4)]:
            raise subprocess.CalledProcessError(4, args, stderr="failed")
        if args[0] == "resize2fs":
            if failure == "replaced":
                staged.unlink()
                _write_ext4_geometry(staged, target)
            elif failure != "geometry":
                with staged.open("r+b") as out:
                    out.seek(1024 + 4)
                    out.write(struct.pack("<I", target // 4096))
                    if failure == "short_file":
                        out.truncate(target // 2)
        return subprocess.CompletedProcess(args, 0, "", "")

    monkeypatch.setattr(MODULE, "run", fake_run)
    with pytest.raises((SystemExit, subprocess.CalledProcessError)):
        MODULE.normalize_rootfs(rootfs, "e2fsck", "resize2fs")
    assert rootfs.read_bytes() == original
    assert not list(tmp_path.glob(".*.tmp"))


@pytest.mark.parametrize("field,value", [(56, 0), (24, 32), (4, 0), (4, 0xFFFFFFFF)])
def test_ext4_geometry_rejects_invalid_or_unbacked_size(tmp_path: Path, field, value) -> None:
    path = tmp_path / "rootfs.ext4"
    _write_ext4_geometry(path, 8192)
    with path.open("r+b") as out:
        out.seek(1024 + field)
        out.write(struct.pack("<H" if field == 56 else "<I", value))
        out.flush()
        with pytest.raises(SystemExit):
            MODULE.ext4_filesystem_bytes(out)


def test_ext4_geometry_honors_64bit_feature_flag(tmp_path: Path) -> None:
    path = tmp_path / "rootfs.ext4"
    _write_ext4_geometry(path, 8192)
    with path.open("r+b") as out:
        out.seek(1024 + 336)
        out.write(struct.pack("<I", 1))
        out.flush()
        assert MODULE.ext4_filesystem_bytes(out) == 8192
        out.seek(1024 + 96)
        out.write(struct.pack("<I", 0x80))
        out.flush()
        with pytest.raises(SystemExit, match="exceeds"):
            MODULE.ext4_filesystem_bytes(out)


def test_normalize_rootfs_rejects_symlink_before_tools(monkeypatch, tmp_path: Path) -> None:
    target = tmp_path / "original.ext4"
    target.write_bytes(b"untouched")
    rootfs = tmp_path / "rootfs.ext4"
    rootfs.symlink_to(target)
    monkeypatch.setattr(MODULE, "run", lambda *_args, **_kwargs: pytest.fail("tool invoked"))
    with pytest.raises(SystemExit, match="non-symlinked"):
        MODULE.normalize_rootfs(rootfs, "e2fsck", "resize2fs")
    assert target.read_bytes() == b"untouched"


def test_main_normalization_failure_removes_stale_env_and_stops_publication(
    monkeypatch, tmp_path: Path
) -> None:
    output = tmp_path / "assets"
    output.mkdir(mode=0o700)
    (output / "env.sh").write_text("stale success")
    monkeypatch.setattr(MODULE, "parse_args", lambda: MODULE.argparse.Namespace(
        output_dir=output, force=False, print_env=True,
    ))
    monkeypatch.setattr(MODULE, "host_asset_arch", lambda: ("arm64", "aarch64", "rootfs-aarch64"))
    monkeypatch.setattr(MODULE, "find_tool", lambda name: name)
    for name in ("verify_checksum_manifest", "verify_pinned_archive", "patch_rootfs"):
        monkeypatch.setattr(MODULE, name, lambda *_args: None)
    monkeypatch.setattr(MODULE, "download", lambda _url, path: path.write_bytes(b"download"))
    monkeypatch.setattr(MODULE, "extract_disk", lambda _archive, disk, _force: disk.write_bytes(b"disk"))
    monkeypatch.setattr(MODULE, "copy_range", lambda _disk, rootfs, *_args: rootfs.write_bytes(b"extracted root"))
    monkeypatch.setattr(MODULE, "root_partition_range", lambda _: (0, 1))

    def fail_normalization(*_args):
        assert not (output / "disk.raw").exists()
        raise SystemExit("normalization failed")

    monkeypatch.setattr(MODULE, "normalize_rootfs", fail_normalization)
    for name in ("newest_boot_file", "dump_boot_file", "write_env"):
        monkeypatch.setattr(MODULE, name, lambda *_args: pytest.fail("published after failed resize"))
    with pytest.raises(SystemExit, match="normalization failed"):
        MODULE.main()
    assert not (output / "env.sh").exists()
    assert (output / MODULE.debian_archive_name("arm64")).read_bytes() == b"download"
    assert (output / "rootfs-aarch64.ext4").read_bytes() == b"extracted root"


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

    owner_tag = str(os.geteuid()) if hasattr(os, "geteuid") else "user"
    assert args.output_dir == tmp_path / f"iroha-inrou-portable-assets-{owner_tag}" / "aarch64"
    assert args.force is False
    assert args.print_env is False


def test_parse_args_accepts_overrides(monkeypatch, tmp_path: Path) -> None:
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
        ],
    )

    args = MODULE.parse_args()

    assert args.output_dir == output_dir
    assert args.force is True
    assert args.print_env is True


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










def test_download_reuses_existing_destination_without_network(monkeypatch, tmp_path: Path) -> None:
    destination = tmp_path / "asset.tar.xz"
    destination.write_bytes(b"existing")

    def fail_urlopen(_url: str):
        raise AssertionError("download should not fetch existing destination")

    monkeypatch.setattr(MODULE, "open_https_download", fail_urlopen)

    MODULE.download(f"{MODULE.default_image_base_url()}/SHA512SUMS", destination)

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

    monkeypatch.setattr(MODULE, "open_https_download", fake_urlopen)
    destination = tmp_path / "nested" / "asset.tar.xz"

    MODULE.download(f"{MODULE.default_image_base_url()}/SHA512SUMS", destination)

    assert calls == [f"{MODULE.default_image_base_url()}/SHA512SUMS"]
    assert destination.read_bytes() == b"downloaded"
    assert not destination.with_suffix(".xz.tmp").exists()


def test_download_rejects_symlinked_destination(monkeypatch, tmp_path: Path) -> None:
    target = tmp_path / "target"
    target.write_bytes(b"attacker-controlled")
    destination = tmp_path / "asset.tar.xz"
    destination.symlink_to(target)

    def fail_urlopen(_url: str):
        raise AssertionError("symlinked destination must fail before network access")

    monkeypatch.setattr(MODULE, "open_https_download", fail_urlopen)

    try:
        MODULE.download(f"{MODULE.default_image_base_url()}/SHA512SUMS", destination)
    except SystemExit as error:
        assert "symlinked download destination" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("symlinked download destination was accepted")
    assert target.read_bytes() == b"attacker-controlled"


def test_download_ignores_preplanted_legacy_temporary_symlink(
    monkeypatch, tmp_path: Path
) -> None:
    class FakeResponse(io.BytesIO):
        def __enter__(self) -> "FakeResponse":
            return self

        def __exit__(self, *_args) -> None:
            self.close()

    target = tmp_path / "target"
    target.write_bytes(b"keep")
    destination = tmp_path / "asset.tar.xz"
    destination.with_suffix(".xz.tmp").symlink_to(target)
    response = FakeResponse(b"verified-download")
    monkeypatch.setattr(MODULE, "open_https_download", lambda _url: response)

    MODULE.download(f"{MODULE.default_image_base_url()}/SHA512SUMS", destination)

    assert destination.read_bytes() == b"verified-download"
    assert target.read_bytes() == b"keep"


def test_prepare_output_directory_rejects_symlink(tmp_path: Path) -> None:
    target = tmp_path / "target"
    target.mkdir()
    output = tmp_path / "assets"
    output.symlink_to(target, target_is_directory=True)

    try:
        MODULE.prepare_output_directory(output)
    except SystemExit as error:
        assert "symlinked Inrou asset output directory" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("symlinked output directory was accepted")


def test_prepare_output_directory_rejects_other_writable_mode(tmp_path: Path) -> None:
    output = tmp_path / "assets"
    output.mkdir(mode=0o700)
    output.chmod(0o707)

    try:
        MODULE.prepare_output_directory(output)
    except SystemExit as error:
        assert "must not be group/other writable" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("other-writable output directory was accepted")


def test_remove_cached_download_rejects_symlink(tmp_path: Path) -> None:
    target = tmp_path / "target"
    target.write_bytes(b"keep")
    cached = tmp_path / "SHA512SUMS"
    cached.symlink_to(target)

    try:
        MODULE.remove_cached_download(cached)
    except SystemExit as error:
        assert "symlinked cached download" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("symlinked cached download was removed")
    assert target.read_bytes() == b"keep"


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
        assert "refusing noncanonical Debian guest assets" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("unsigned unpinned archive was accepted")












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


def test_extract_disk_rejects_non_regular_disk_member(tmp_path: Path) -> None:
    archive = tmp_path / "image.tar.xz"
    with tarfile.open(archive, "w:xz") as tar:
        member = tarfile.TarInfo("disk.raw")
        member.type = tarfile.SYMTYPE
        member.linkname = "elsewhere.raw"
        tar.addfile(member)

    try:
        MODULE.extract_disk(archive, tmp_path / "disk.raw", force=True)
    except SystemExit as error:
        assert "exactly one regular disk.raw" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("non-regular disk.raw was accepted")


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




