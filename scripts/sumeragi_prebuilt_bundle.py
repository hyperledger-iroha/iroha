#!/usr/bin/env python3
"""Bounded protocol primitives for the reviewed native release pipeline.

This module grants no executable authority and has no standalone publication CLI.
The structured pipeline admits independent entry/bridge pins and original sources;
filesystem ACL admission, creation, locking and bundle decoding belong to Rust.
"""
from __future__ import annotations

import os
from pathlib import Path
import re

_DIGEST = re.compile(r"[0-9a-f]{64}")
_NAME = re.compile(r"invocation\.[A-Za-z0-9]{6}")


class PrebuiltBundleError(RuntimeError):
    """Native publication or original source custody was refused."""


class NativeFailure(PrebuiltBundleError):
    """Preserve the native child's explicit refusal classification."""

    def __init__(self, status: int):
        super().__init__(f"native entry exited unsuccessfully (exit {status})")
        self.status = status if 0 < status < 256 else 1


def _path(value: str) -> Path:
    if not os.path.isabs(value) or os.path.normpath(value) != value:
        raise PrebuiltBundleError("paths require normalized absolute spelling")
    if any(ord(character) < 32 or ord(character) == 127 for character in value):
        raise PrebuiltBundleError("paths contain control characters")
    return Path(value)


def _digest(value: str) -> str:
    if _DIGEST.fullmatch(value) is None:
        raise PrebuiltBundleError("invalid independent digest anchor")
    return value


def _acknowledgement(raw: bytes, programs: Path) -> tuple[Path, str]:
    try:
        lines = raw.decode("utf-8", errors="strict").split("\n")
    except UnicodeError as error:
        raise PrebuiltBundleError("native acknowledgement is not UTF-8") from error
    if len(lines) != 3 or lines[2] or not lines[0].startswith("bundle_dir\t") or not lines[1].startswith("manifest_sha256\t"):
        raise PrebuiltBundleError("native acknowledgement does not contain exactly two canonical fields")
    path = _path(lines[0].removeprefix("bundle_dir\t"))
    manifest = _digest(lines[1].removeprefix("manifest_sha256\t"))
    if str(path.parent) != str(programs) or _NAME.fullmatch(path.name) is None:
        raise PrebuiltBundleError("native acknowledgement escapes the exact invocation namespace")
    return path, manifest



if __name__ == "__main__":
    raise SystemExit("use the reviewed run_release_pipeline.py prepare-sumeragi-programs operation")
