#!/usr/bin/env python3
"""Prepare Debian genericcloud guest assets for the Inrou PortableVm smoke."""

from __future__ import annotations

import argparse
import hashlib
import os
import platform
import re
import shlex
import shutil
import struct
import subprocess
import sys
import tarfile
import urllib.error
import urllib.request
import uuid
from pathlib import Path


DEBIAN_CODENAME = "bookworm"
DEBIAN_RELEASE = "12"
DEBIAN_IMAGE_BUILD = "20260413-2447"
DEBIAN_VARIANT = "genericcloud"
EFI_SYSTEM_PARTITION = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b"
SECTOR_SIZE = 512
PINNED_ARCHIVE_SHA512 = {
    f"debian-{DEBIAN_RELEASE}-{DEBIAN_VARIANT}-amd64-{DEBIAN_IMAGE_BUILD}.tar.xz": (
        "1995b19708ba5a7eec0ffb98ddd58dfa0dab09afee96e57bf243b2540688c5b6"
        "f3d7ec7a3fa7b233beb61e9c2ef4afd1f2164a2b8e19f480e2c7d512d2b1450c"
    ),
    f"debian-{DEBIAN_RELEASE}-{DEBIAN_VARIANT}-arm64-{DEBIAN_IMAGE_BUILD}.tar.xz": (
        "25dc6eb7173b92e6d8243745488405c223d656c974fa4b458cf1b65250b08e"
        "1ec66a4ba92864ca84ed6e06d96d3592008ef49d3e438c4b63b70f86f5681c09c7"
    ),
}
DEBIAN_KEYRING_ENV = "DEBIAN_ARCHIVE_KEYRING"
DEBIAN_KEYRING_CANDIDATES = (
    "/usr/share/keyrings/debian-archive-keyring.gpg",
    "/usr/share/keyrings/debian-cloud-images-archive-keyring.gpg",
    "/usr/share/keyrings/debian-cloud-images-keyring.gpg",
    "/usr/share/keyrings/debian-role-keys.gpg",
    "/etc/apt/trusted.gpg.d/debian-archive-bookworm-stable.gpg",
    "/etc/apt/trusted.gpg.d/debian-archive-bookworm-security-automatic.gpg",
    "/etc/apt/trusted.gpg.d/debian-archive-bookworm-automatic.gpg",
    "/opt/homebrew/share/keyrings/debian-archive-keyring.gpg",
    "/usr/local/share/keyrings/debian-archive-keyring.gpg",
)


def default_image_base_url() -> str:
    return (
        f"https://cloud.debian.org/images/cloud/"
        f"{DEBIAN_CODENAME}/{DEBIAN_IMAGE_BUILD}"
    )


def debian_archive_name(deb_arch: str) -> str:
    return f"debian-{DEBIAN_RELEASE}-{DEBIAN_VARIANT}-{deb_arch}-{DEBIAN_IMAGE_BUILD}.tar.xz"


def parse_args() -> argparse.Namespace:
    default_output = (
        Path(os.environ.get("TMPDIR", "/tmp"))
        / "iroha-inrou-portable-assets"
        / host_asset_arch()[1]
    )
    parser = argparse.ArgumentParser(
        description=(
            "Download and prepare verified Debian genericcloud assets for "
            "cargo xtask soracloud-inrou-smoke portable."
        )
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=default_output,
        help="directory where prepared assets and env.sh are written",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="rebuild prepared assets even when output files already exist",
    )
    parser.add_argument(
        "--print-env",
        action="store_true",
        help="print shell exports for the prepared IROHA_INROU_PORTABLE_* paths",
    )
    parser.add_argument(
        "--image-base-url",
        default=default_image_base_url(),
        help=(
            "base URL for Debian cloud image assets; defaults to the pinned "
            f"{DEBIAN_CODENAME} build {DEBIAN_IMAGE_BUILD}"
        ),
    )
    parser.add_argument(
        "--debian-keyring",
        type=Path,
        help=(
            "GPG keyring used to verify SHA512SUMS.sign; defaults to "
            f"${DEBIAN_KEYRING_ENV} or common Debian archive keyring paths"
        ),
    )
    return parser.parse_args()


def host_asset_arch() -> tuple[str, str, str]:
    machine = platform.machine().lower()
    if machine in {"x86_64", "amd64"}:
        return ("amd64", "x86_64", "rootfs-x86_64")
    if machine in {"arm64", "aarch64"}:
        return ("arm64", "aarch64", "rootfs-aarch64")
    raise SystemExit(f"unsupported host architecture for PortableVm assets: {machine}")


def find_tool(name: str) -> str:
    if resolved := shutil.which(name):
        return resolved
    for directory in (
        "/opt/homebrew/opt/e2fsprogs/sbin",
        "/opt/homebrew/sbin",
        "/usr/local/opt/e2fsprogs/sbin",
        "/usr/local/sbin",
        "/usr/sbin",
        "/sbin",
    ):
        candidate = Path(directory) / name
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    raise SystemExit(
        f"required e2fsprogs tool `{name}` was not found; install e2fsprogs "
        "or add its sbin directory to PATH"
    )


def find_gpg_tool() -> str:
    for name in ("gpgv", "gpg"):
        if resolved := shutil.which(name):
            return resolved
    raise SystemExit(
        "required GPG verifier `gpgv` or `gpg` was not found; install GnuPG "
        "before preparing Inrou PortableVm guest assets"
    )


def run(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, check=check, text=True, capture_output=True)


def download(url: str, destination: Path) -> None:
    if destination.is_file():
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(destination.suffix + ".tmp")
    with urllib.request.urlopen(url) as response, temporary.open("wb") as out:
        shutil.copyfileobj(response, out, length=1024 * 1024)
    temporary.replace(destination)


def download_optional_signature(url: str, destination: Path) -> bool:
    try:
        download(url, destination)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise
    return True


def sha512(path: Path) -> str:
    digest = hashlib.sha512()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_archive(archive: Path, sums_path: Path) -> None:
    wanted = archive.name
    expected = None
    for line in sums_path.read_text(encoding="utf-8").splitlines():
        parts = line.split()
        if len(parts) < 2:
            continue
        filename = parts[-1].removeprefix("*").removeprefix("./")
        if filename == wanted:
            expected = parts[0]
            break
    if expected is None:
        raise SystemExit(f"{sums_path} does not list {wanted}")
    actual = sha512(archive)
    if actual.lower() != expected.lower():
        raise SystemExit(f"SHA512 mismatch for {archive}: expected {expected}, got {actual}")


def verify_pinned_archive(archive: Path) -> None:
    expected = PINNED_ARCHIVE_SHA512.get(archive.name)
    if expected is None:
        raise SystemExit(
            f"{archive.name} has no pinned SHA512 digest and SHA512SUMS was not GPG verified; "
            "refusing to use unsigned Debian guest assets"
        )
    actual = sha512(archive)
    if actual.lower() != expected.lower():
        raise SystemExit(
            f"pinned SHA512 mismatch for {archive}: expected {expected}, got {actual}"
        )


def resolve_debian_keyrings(configured: Path | None = None) -> list[Path]:
    if configured is not None:
        keyring = configured.expanduser().resolve()
        if not keyring.is_file():
            raise SystemExit(f"--debian-keyring does not exist or is not a file: {keyring}")
        return [keyring]

    candidates: list[Path] = []
    if env_value := os.environ.get(DEBIAN_KEYRING_ENV):
        candidates.extend(Path(value).expanduser() for value in env_value.split(os.pathsep))
    candidates.extend(Path(value) for value in DEBIAN_KEYRING_CANDIDATES)

    resolved: list[Path] = []
    seen: set[Path] = set()
    for candidate in candidates:
        if not str(candidate):
            continue
        path = candidate.expanduser().resolve()
        if path in seen or not path.is_file():
            continue
        seen.add(path)
        resolved.append(path)
    if resolved:
        return resolved

    raise SystemExit(
        "Debian SHA512SUMS signature verification requires a Debian archive "
        "or cloud-image GPG keyring. Install `debian-archive-keyring`, set "
        f"${DEBIAN_KEYRING_ENV}, or pass --debian-keyring."
    )


def verify_signed_sums(
    sums_path: Path,
    signature_path: Path,
    keyrings: list[Path],
    gpg_tool: str,
) -> None:
    keyring_args = [
        value
        for keyring in keyrings
        for value in ("--keyring", str(keyring))
    ]
    if Path(gpg_tool).name == "gpgv":
        command = [gpg_tool, *keyring_args, str(signature_path), str(sums_path)]
    else:
        command = [
            gpg_tool,
            "--batch",
            "--no-default-keyring",
            *keyring_args,
            "--verify",
            str(signature_path),
            str(sums_path),
        ]
    try:
        run(command)
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or error.stdout or str(error)).strip()
        raise SystemExit(
            f"failed to verify Debian SHA512SUMS signature with {gpg_tool}: {detail}"
        ) from error


def verify_debian_sums_signature_if_available(
    base_url: str,
    sums_path: Path,
    signature_path: Path,
    configured_keyring: Path | None,
) -> bool:
    if not download_optional_signature(f"{base_url}/SHA512SUMS.sign", signature_path):
        return False
    gpg_tool = find_gpg_tool()
    debian_keyrings = resolve_debian_keyrings(configured_keyring)
    verify_signed_sums(sums_path, signature_path, debian_keyrings, gpg_tool)
    return True


def extract_disk(archive: Path, disk: Path, force: bool) -> None:
    if disk.is_file() and not force:
        return
    temporary = disk.with_suffix(".raw.tmp")
    with tarfile.open(archive, "r:xz") as tar:
        member = next(
            (entry for entry in tar.getmembers() if Path(entry.name).name == "disk.raw"),
            None,
        )
        if member is None:
            raise SystemExit(f"{archive} does not contain disk.raw")
        source = tar.extractfile(member)
        if source is None:
            raise SystemExit(f"unable to extract disk.raw from {archive}")
        with temporary.open("wb") as out:
            shutil.copyfileobj(source, out, length=1024 * 1024)
    temporary.replace(disk)


def guid_from_gpt(raw: bytes) -> str:
    return str(uuid.UUID(bytes_le=raw))


def root_partition_range(disk: Path) -> tuple[int, int]:
    with disk.open("rb") as stream:
        stream.seek(SECTOR_SIZE)
        header = stream.read(SECTOR_SIZE)
        if header[:8] != b"EFI PART":
            raise SystemExit(f"{disk} does not contain a GPT header")
        entries_lba = struct.unpack_from("<Q", header, 72)[0]
        entry_count = struct.unpack_from("<I", header, 80)[0]
        entry_size = struct.unpack_from("<I", header, 84)[0]
        stream.seek(entries_lba * SECTOR_SIZE)
        entries = stream.read(entry_count * entry_size)

    candidates: list[tuple[int, int]] = []
    for index in range(entry_count):
        entry = entries[index * entry_size : (index + 1) * entry_size]
        type_guid = guid_from_gpt(entry[:16])
        if type_guid == "00000000-0000-0000-0000-000000000000":
            continue
        if type_guid == EFI_SYSTEM_PARTITION:
            continue
        first_lba = struct.unpack_from("<Q", entry, 32)[0]
        last_lba = struct.unpack_from("<Q", entry, 40)[0]
        if last_lba < first_lba:
            continue
        candidates.append((first_lba, last_lba))
    if not candidates:
        raise SystemExit(f"{disk} does not contain a non-EFI root partition")
    first_lba, last_lba = max(candidates, key=lambda item: item[1] - item[0])
    return first_lba * SECTOR_SIZE, (last_lba - first_lba + 1) * SECTOR_SIZE


def copy_range(source: Path, destination: Path, offset: int, length: int, force: bool) -> None:
    if destination.is_file() and not force:
        return
    temporary = destination.with_suffix(destination.suffix + ".tmp")
    remaining = length
    with source.open("rb") as src, temporary.open("wb") as out:
        src.seek(offset)
        while remaining:
            chunk = src.read(min(1024 * 1024, remaining))
            if not chunk:
                raise SystemExit(f"unexpected EOF while extracting root partition from {source}")
            out.write(chunk)
            remaining -= len(chunk)
    temporary.replace(destination)


def patch_rootfs(rootfs: Path, root_label: str, debugfs: str, tune2fs: str) -> None:
    run([tune2fs, "-L", root_label, str(rootfs)])
    fstab = rootfs.parent / "fstab"
    fstab.write_text(
        f"LABEL={root_label} / ext4 rw,discard,errors=remount-ro,x-systemd.growfs 0 1\n",
        encoding="utf-8",
    )
    run([debugfs, "-w", "-R", "rm /etc/fstab", str(rootfs)], check=False)
    run([debugfs, "-w", "-R", f"write {fstab} /etc/fstab", str(rootfs)])


def debugfs_stdout(debugfs: str, command: str, rootfs: Path) -> str:
    return run([debugfs, "-R", command, str(rootfs)]).stdout


def newest_boot_file(debugfs: str, rootfs: Path, prefix: str) -> str:
    listing = debugfs_stdout(debugfs, "ls -p /boot", rootfs)
    names = re.findall(rf"/([^/]*{re.escape(prefix)}[^/]*)/", listing)
    names = [name for name in names if name.startswith(prefix)]
    if not names:
        raise SystemExit(f"unable to find /boot/{prefix}* in {rootfs}")
    names.sort(key=lambda name: ("cloud" in name, name))
    return names[-1]


def dump_boot_file(debugfs: str, rootfs: Path, source_name: str, destination: Path) -> None:
    if destination.exists():
        destination.unlink()
    run([debugfs, "-R", f"dump -p /boot/{source_name} {destination}", str(rootfs)])


def write_env(output_dir: Path, kernel: Path, rootfs: Path, initrd: Path, print_env: bool) -> None:
    lines = [
        f"export IROHA_INROU_PORTABLE_KERNEL_IMAGE={shlex.quote(str(kernel))}",
        f"export IROHA_INROU_PORTABLE_ROOTFS_IMAGE={shlex.quote(str(rootfs))}",
        f"export IROHA_INROU_PORTABLE_INITRD_IMAGE={shlex.quote(str(initrd))}",
    ]
    env_path = output_dir / "env.sh"
    env_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    if print_env:
        print("\n".join(lines))
    else:
        print(f"Wrote {env_path}")


def main() -> None:
    args = parse_args()
    deb_arch, guest_arch, root_label = host_asset_arch()
    output_dir = args.output_dir.expanduser().resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    base_url = args.image_base_url.rstrip("/")
    archive_name = debian_archive_name(deb_arch)
    archive = output_dir / archive_name
    sums = output_dir / "SHA512SUMS"
    sums_signature = output_dir / "SHA512SUMS.sign"
    disk = output_dir / "disk.raw"
    rootfs = output_dir / f"rootfs-{guest_arch}.ext4"
    kernel = output_dir / f"vmlinux-{guest_arch}"
    initrd = output_dir / f"initrd-{guest_arch}.img"

    debugfs = find_tool("debugfs")
    tune2fs = find_tool("tune2fs")

    download(f"{base_url}/SHA512SUMS", sums)
    sums_verified = verify_debian_sums_signature_if_available(
        base_url, sums, sums_signature, args.debian_keyring
    )
    download(f"{base_url}/{archive_name}", archive)
    if sums_verified:
        verify_archive(archive, sums)
    else:
        verify_pinned_archive(archive)
    extract_disk(archive, disk, args.force)
    offset, length = root_partition_range(disk)
    copy_range(disk, rootfs, offset, length, args.force)
    patch_rootfs(rootfs, root_label, debugfs, tune2fs)

    kernel_name = newest_boot_file(debugfs, rootfs, "vmlinuz-")
    initrd_name = newest_boot_file(debugfs, rootfs, "initrd.img-")
    dump_boot_file(debugfs, rootfs, kernel_name, kernel)
    dump_boot_file(debugfs, rootfs, initrd_name, initrd)
    write_env(output_dir, kernel, rootfs, initrd, args.print_env)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        sys.stderr.write(error.stderr or "")
        raise SystemExit(error.returncode) from error
