#!/usr/bin/env python3
"""Render one digest-pinned, custody-local Taira genesis signer wrapper.

The generated executable embeds only public trust material: the canonical
Kagami path and SHA-256 plus the expected genesis public key.  The private key
remains in its fixed owner-private custody file and is never accepted through
argv or the environment.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
from typing import NoReturn, Sequence


PUBLIC_KEY_RE = re.compile(r"ed0120[0-9A-F]{64}")
MAX_KAGAMI_BYTES = 768 * 1024 * 1024


class SignerRenderError(RuntimeError):
    """The requested signer wrapper is unsafe or incomplete."""


def fail(message: str) -> NoReturn:
    raise SignerRenderError(message)


def _secure_ancestry(path: Path, label: str, *, private_parent: bool = False) -> None:
    """Require a canonical, non-replaceable path below trusted directories."""

    parent = path.parent
    first = True
    while True:
        try:
            resolved = parent.resolve(strict=True)
            info = parent.lstat()
        except OSError as error:
            raise SignerRenderError(f"cannot inspect {label} ancestry: {error}") from error
        mode = stat.S_IMODE(info.st_mode)
        if (
            resolved != parent
            or stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or info.st_uid not in {0, os.getuid()}
            or mode & 0o022
        ):
            fail(f"{label} has unsafe replaceable ancestry: {parent}")
        if private_parent and first and (
            info.st_uid != os.getuid() or info.st_gid != os.getgid() or mode != 0o700
        ):
            fail(f"{label} parent must be one owner-controlled mode-0700 directory")
        if parent == parent.parent:
            break
        parent = parent.parent
        first = False


def _regular(path: Path, label: str, *, private: bool, executable: bool = False) -> os.stat_result:
    if not path.is_absolute():
        fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as error:
        raise SignerRenderError(f"cannot inspect {label}: {error}") from error
    if (
        resolved != path
        or stat.S_ISLNK(info.st_mode)
        or not stat.S_ISREG(info.st_mode)
        or info.st_nlink != 1
        or info.st_uid != os.getuid()
        or info.st_gid != os.getgid()
    ):
        fail(f"{label} must be one owner-controlled canonical regular file")
    mode = stat.S_IMODE(info.st_mode)
    if private and mode != 0o600:
        fail(f"{label} must have mode 0600")
    if executable and (mode & 0o111 == 0 or mode & 0o022):
        fail(f"{label} must be executable and not group/world writable")
    if info.st_size <= 0:
        fail(f"{label} must be non-empty")
    _secure_ancestry(path, label, private_parent=private)
    return info


def _private_parent(path: Path) -> None:
    parent = path.parent
    try:
        resolved = parent.resolve(strict=True)
        info = parent.lstat()
    except OSError as error:
        raise SignerRenderError(f"cannot inspect output parent: {error}") from error
    if (
        resolved != parent
        or stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != os.getuid()
        or info.st_gid != os.getgid()
        or stat.S_IMODE(info.st_mode) != 0o700
    ):
        fail("output parent must be one owner-controlled canonical mode-0700 directory")
    if path.exists() or path.is_symlink():
        fail("output signer already exists")
    _secure_ancestry(path, "output signer", private_parent=True)


def _sha256(path: Path, maximum: int) -> str:
    expected = path.stat()
    if expected.st_size > maximum:
        fail(f"executable exceeds {maximum} bytes: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(1024 * 1024):
            digest.update(block)
    observed = path.stat()
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
        value.st_nlink,
    )
    if identity(expected) != identity(observed):
        fail(f"file changed while hashing: {path}")
    return digest.hexdigest()


WRAPPER_TEMPLATE = r'''#!/usr/bin/python3
"""Digest-pinned Taira genesis signer; generated public trust closure."""

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile

KAGAMI = Path(__KAGAMI_PATH__)
KAGAMI_SHA256 = __KAGAMI_SHA256__
PRIVATE_KEY = Path(__PRIVATE_KEY_PATH__)
EXPECTED_PUBLIC_KEY = __EXPECTED_PUBLIC_KEY__
MAX_KAGAMI_BYTES = 805306368


def die(message):
    print(message, file=sys.stderr)
    raise SystemExit(70)


def secure_ancestry(path, label, private_parent=False):
    parent = path.parent
    first = True
    while True:
        try:
            resolved = parent.resolve(strict=True)
            info = parent.lstat()
        except OSError as error:
            die(f"cannot inspect {label} ancestry: {error}")
        mode = stat.S_IMODE(info.st_mode)
        if (resolved != parent or stat.S_ISLNK(info.st_mode)
                or not stat.S_ISDIR(info.st_mode)
                or info.st_uid not in (0, os.getuid()) or mode & 0o022):
            die(f"unsafe replaceable ancestry for {label}: {parent}")
        if (private_parent and first
                and (info.st_uid != os.getuid() or info.st_gid != os.getgid()
                     or mode != 0o700)):
            die(f"{label} parent must have owner-private mode 0700")
        if parent == parent.parent:
            return
        parent = parent.parent
        first = False


def regular(path, label, mode=None, executable=False):
    if not path.is_absolute():
        die(f"{label} must be absolute")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as error:
        die(f"cannot inspect {label}: {error}")
    if (resolved != path or stat.S_ISLNK(info.st_mode)
            or not stat.S_ISREG(info.st_mode) or info.st_nlink != 1
            or info.st_uid != os.getuid() or info.st_gid != os.getgid()):
        die(f"unsafe {label}")
    observed_mode = stat.S_IMODE(info.st_mode)
    if mode is not None and observed_mode != mode:
        die(f"{label} must have mode {mode:04o}")
    if executable and (observed_mode & 0o111 == 0 or observed_mode & 0o022):
        die(f"unsafe executable mode for {label}")
    if info.st_size <= 0:
        die(f"{label} is empty")
    secure_ancestry(path, label, private_parent=mode == 0o600)
    return info


def private_output(path, label, allow_existing=False):
    if not path.is_absolute():
        die(f"{label} must be absolute")
    try:
        parent = path.parent.resolve(strict=True)
        info = path.parent.lstat()
    except OSError as error:
        die(f"cannot inspect {label} parent: {error}")
    if (parent != path.parent or stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
            or info.st_gid != os.getgid() or stat.S_IMODE(info.st_mode) != 0o700):
        die(f"unsafe {label} parent")
    if not allow_existing and (path.exists() or path.is_symlink()):
        die(f"{label} already exists")
    secure_ancestry(path, label, private_parent=True)


def identity(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns,
            value.st_ctime_ns, value.st_nlink)


def snapshot_file(source, destination, label, source_mode, output_mode, maximum,
                  expected_digest=None, executable=False):
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        source_fd = os.open(source, flags)
    except OSError as error:
        die(f"cannot open {label}: {error}")
    try:
        before = os.fstat(source_fd)
        if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
                or before.st_uid != os.getuid() or before.st_gid != os.getgid()
                or (source_mode is not None
                    and stat.S_IMODE(before.st_mode) != source_mode)
                or (executable
                    and (stat.S_IMODE(before.st_mode) & 0o111 == 0
                         or stat.S_IMODE(before.st_mode) & 0o022))
                or before.st_size <= 0
                or before.st_size > maximum):
            die(f"unsafe {label}")
        output_flags = (os.O_WRONLY | os.O_CREAT | os.O_EXCL
                        | getattr(os, "O_NOFOLLOW", 0))
        output_fd = os.open(destination, output_flags, output_mode)
        digest = hashlib.sha256()
        try:
            while True:
                block = os.read(source_fd, 1024 * 1024)
                if not block:
                    break
                digest.update(block)
                pending = memoryview(block)
                while pending:
                    written = os.write(output_fd, pending)
                    if written <= 0:
                        die(f"short write while snapshotting {label}")
                    pending = pending[written:]
            os.fsync(output_fd)
        finally:
            os.close(output_fd)
        after = os.fstat(source_fd)
    finally:
        os.close(source_fd)
    if identity(before) != identity(after):
        die(f"{label} changed while it was snapshotted")
    observed_digest = digest.hexdigest()
    if expected_digest is not None and observed_digest != expected_digest:
        die(f"{label} SHA-256 mismatch")
    return observed_digest


def open_private_key(path):
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        die(f"cannot open genesis private key: {error}")
    info = os.fstat(descriptor)
    if (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1
            or info.st_uid != os.getuid() or info.st_gid != os.getgid()
            or stat.S_IMODE(info.st_mode) != 0o600 or info.st_size <= 0
            or info.st_size > 1024 * 1024):
        os.close(descriptor)
        die("unsafe genesis private key")
    return descriptor, info


def sha256(path):
    before = path.stat()
    if before.st_size > MAX_KAGAMI_BYTES:
        die("Kagami exceeds the signer size bound")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(1024 * 1024):
            digest.update(block)
    after = path.stat()
    if identity(before) != identity(after):
        die("Kagami changed while it was hashed")
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(allow_abbrev=False)
    parser.add_argument("--unsigned-genesis", type=Path, required=True)
    parser.add_argument("--peer-config", type=Path, required=True)
    parser.add_argument("--bound-manifest-out", type=Path, required=True)
    parser.add_argument("--signed-genesis-out", type=Path, required=True)
    parser.add_argument("--expected-hash-out", type=Path, required=True)
    args = parser.parse_args()

    regular(KAGAMI, "pinned Kagami", executable=True)
    if sha256(KAGAMI) != KAGAMI_SHA256:
        die("pinned Kagami SHA-256 mismatch")
    regular(PRIVATE_KEY, "genesis private key", mode=0o600)
    regular(args.unsigned_genesis, "unsigned genesis", mode=0o600)
    regular(args.peer_config, "peer config", mode=0o600)
    if args.bound_manifest_out != args.unsigned_genesis:
        private_output(args.bound_manifest_out, "bound manifest")
    private_output(args.signed_genesis_out, "signed genesis")
    private_output(args.expected_hash_out, "expected hash")

    key_fd, key_identity = open_private_key(PRIVATE_KEY)
    work = None
    try:
        work = Path(tempfile.mkdtemp(prefix=".taira-genesis-sign-", dir=PRIVATE_KEY.parent))
        os.chmod(work, 0o700)
        kagami_snapshot = work / "kagami"
        snapshot_file(KAGAMI, kagami_snapshot, "pinned Kagami", None, 0o700,
                      MAX_KAGAMI_BYTES, KAGAMI_SHA256, executable=True)
        command = [
            str(kagami_snapshot), "genesis", "sign", str(args.unsigned_genesis),
            "--config", str(args.peer_config),
            "--private-key-file", f"/dev/fd/{key_fd}",
            "--expected-public-key", EXPECTED_PUBLIC_KEY,
            "--bound-manifest-out", str(args.bound_manifest_out),
            "--out-file", str(args.signed_genesis_out),
            "--expected-hash-out", str(args.expected_hash_out),
        ]
        try:
            completed = subprocess.run(
                command,
                stdin=subprocess.DEVNULL,
                check=False,
                timeout=540,
                env={"HOME": str(work), "LANG": "C", "LC_ALL": "C",
                     "PATH": "/usr/bin:/bin", "TMPDIR": str(work)},
                umask=0o077,
                pass_fds=(key_fd,),
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            die(f"Kagami genesis signing could not complete: {error}")
        if identity(key_identity) != identity(os.fstat(key_fd)):
            die("genesis private key changed while signing")
    finally:
        os.close(key_fd)
        if work is not None:
            shutil.rmtree(work)
    if completed.returncode != 0:
        die(f"Kagami genesis signing refused with status {completed.returncode}")
    if sha256(KAGAMI) != KAGAMI_SHA256:
        die("pinned Kagami changed during signing")
    regular(PRIVATE_KEY, "genesis private key", mode=0o600)
    regular(args.bound_manifest_out, "bound manifest", mode=0o600)
    regular(args.signed_genesis_out, "signed genesis", mode=0o600)
    regular(args.expected_hash_out, "expected hash", mode=0o600)
    expected_hash = args.expected_hash_out.read_text(encoding="ascii")
    if re.fullmatch(r"[0-9a-f]{64}\n", expected_hash) is None:
        die("Kagami emitted a noncanonical expected hash")


if __name__ == "__main__":
    main()
'''


def render(args: argparse.Namespace) -> dict[str, str]:
    public_key = args.expected_public_key
    if PUBLIC_KEY_RE.fullmatch(public_key) is None:
        fail("expected genesis public key must be one canonical uppercase Ed25519 multihash")
    _regular(args.kagami, "Kagami", private=False, executable=True)
    _regular(args.private_key, "genesis private key", private=True)
    _private_parent(args.output)
    kagami_sha256 = _sha256(args.kagami, MAX_KAGAMI_BYTES)
    rendered = (
        WRAPPER_TEMPLATE.replace("__KAGAMI_PATH__", repr(str(args.kagami)))
        .replace("__KAGAMI_SHA256__", repr(kagami_sha256))
        .replace("__PRIVATE_KEY_PATH__", repr(str(args.private_key)))
        .replace("__EXPECTED_PUBLIC_KEY__", repr(public_key))
    )
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(args.output, flags, 0o700)
        try:
            remaining = memoryview(rendered.encode("utf-8"))
            while remaining:
                written = os.write(descriptor, remaining)
                if written <= 0:
                    fail("short write while rendering the external signer")
                remaining = remaining[written:]
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    except BaseException:
        args.output.unlink(missing_ok=True)
        raise
    os.chmod(args.output, 0o700)
    signer_sha256 = _sha256(args.output, 2 * 1024 * 1024)
    return {
        "expected_public_key": public_key,
        "kagami_sha256": kagami_sha256,
        "signer_sha256": signer_sha256,
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(allow_abbrev=False)
    result.add_argument("--kagami", type=Path, required=True)
    result.add_argument("--private-key", type=Path, required=True)
    result.add_argument("--expected-public-key", required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    try:
        receipt = render(parser().parse_args(argv))
    except SignerRenderError as error:
        print(f"error: {error}", file=os.sys.stderr)
        return 1
    print(json.dumps(receipt, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
