#!/usr/bin/env python3
"""Inspect the exact user-owned source closure for a local Taira reset."""

from __future__ import annotations

import argparse
import os
import shutil
import stat
import sys
from collections.abc import Sequence
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    from . import prepare_taira_empty_reset_bundle as reset_bundle
except ImportError:
    import prepare_taira_empty_reset_bundle as reset_bundle


def _write_all(descriptor: int, payload: bytes) -> None:
    view = memoryview(payload)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise RuntimeError("failed to stage private source closure bytes")
        view = view[written:]


def stage_source_closure(target: Path) -> tuple[dict[str, object], str]:
    """Copy the reviewed sources into one exact owner-private execution root."""

    if not target.is_absolute() or target.exists():
        raise RuntimeError("source closure target must be one absent absolute path")
    parent = target.parent
    parent_info = parent.lstat()
    if (
        not stat.S_ISDIR(parent_info.st_mode)
        or stat.S_ISLNK(parent_info.st_mode)
        or parent_info.st_uid != 501
        or stat.S_IMODE(parent_info.st_mode) != 0o700
    ):
        raise RuntimeError("source closure parent must be UID501 mode 0700")
    source_root = SCRIPT_DIR.parent.resolve(strict=True)
    manifest, digest = reset_bundle.local_testnet_source_closure()
    target.mkdir(mode=0o700)
    target.chmod(0o700)
    try:
        for relative in reset_bundle.LOCAL_TESTNET_SOURCE_CLOSURE_FILES:
            destination = target / relative
            pending: list[Path] = []
            parent_path = destination.parent
            while parent_path != target and not parent_path.exists():
                pending.append(parent_path)
                parent_path = parent_path.parent
            for directory in reversed(pending):
                directory.mkdir(mode=0o700)
                directory.chmod(0o700)
            _, payload = reset_bundle.stable_read_path(
                source_root / relative,
                max_size=reset_bundle.MAX_NATIVE_TOOL_BYTES,
            )
            flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            descriptor = os.open(destination, flags, 0o600)
            try:
                _write_all(descriptor, payload)
                os.fchmod(descriptor, 0o600)
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        reset_bundle.verify_private_python_source_closure(
            target,
            manifest,
            digest,
            owner_uid=501,
        )
    except BaseException:
        shutil.rmtree(target)
        raise
    return manifest, digest


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument(
        "--digest-only",
        action="store_true",
        help="print only the lowercase SHA-256 identity",
    )
    parser.add_argument(
        "--stage-root",
        type=Path,
        help="copy the exact closure into one absent UID501-private root",
    )
    args = parser.parse_args(argv)
    if args.stage_root is None:
        manifest, digest = reset_bundle.local_testnet_source_closure()
    else:
        manifest, digest = stage_source_closure(args.stage_root)
    if args.digest_only:
        print(digest)
    else:
        sys.stdout.buffer.write(
            reset_bundle.canonical_json_bytes({**manifest, "sha256": digest})
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
