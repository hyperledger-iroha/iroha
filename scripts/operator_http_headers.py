#!/usr/bin/env python3
"""Emit fresh exact-network operator headers for one runtime-only HTTP request."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import stat
from urllib.parse import urlsplit

try:
    from scripts.iso_operator_auth import load_operator_signing_context
except ModuleNotFoundError:  # Direct execution sets sys.path to scripts/.
    from iso_operator_auth import load_operator_signing_context


MAX_PRIVATE_KEY_FILE_BYTES = 4096


def _private_key_literal(path: Path) -> str:
    """Read one owner-private, non-linked key file without following a symlink."""

    if not path.is_absolute():
        raise ValueError("operator private-key file path must be absolute")
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise ValueError("operator private-key file must be one regular, singly-linked file")
    if stat.S_IMODE(metadata.st_mode) != 0o600:
        raise ValueError("operator private-key file must have exact mode 0600")
    if metadata.st_size > MAX_PRIVATE_KEY_FILE_BYTES:
        raise ValueError("operator private-key file exceeds the 4096-byte bound")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino):
            raise ValueError("operator private-key file changed while opening")
        payload = os.read(descriptor, MAX_PRIVATE_KEY_FILE_BYTES + 1)
        if len(payload) > MAX_PRIVATE_KEY_FILE_BYTES:
            raise ValueError("operator private-key file exceeds the 4096-byte bound")
    finally:
        os.close(descriptor)
    try:
        literal = payload.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise ValueError("operator private-key file must contain ASCII") from error
    if not literal or any(character.isspace() for character in literal):
        raise ValueError("operator private-key file must contain exactly one key literal")
    return literal


def _request_target(url: str) -> str:
    parsed = urlsplit(url)
    if (
        parsed.scheme not in {"http", "https"}
        or not parsed.netloc
        or parsed.username is not None
        or parsed.password is not None
    ):
        raise ValueError("operator request URL must be absolute credential-free HTTP(S)")
    if parsed.fragment:
        raise ValueError("operator request URL must not contain a fragment")
    path = parsed.path or "/"
    return path + (f"?{parsed.query}" if parsed.query else "")


def load_operator_context_from_file(network_id: str, path: Path):
    """Load one exact-network context from an owner-private runtime file."""

    return load_operator_signing_context(network_id, _private_key_literal(path))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--network-id", required=True)
    parser.add_argument("--private-key-file", required=True, type=Path)
    parser.add_argument("--method", required=True, choices=("GET", "HEAD"))
    parser.add_argument("--url", required=True)
    args = parser.parse_args()

    context = load_operator_context_from_file(args.network_id, args.private_key_file)
    headers = context.headers(args.method, _request_target(args.url), b"")
    for name in (
        "x-iroha-operator-public-key",
        "x-iroha-operator-timestamp-ms",
        "x-iroha-operator-nonce",
        "x-iroha-operator-signature",
    ):
        value = headers[name]
        if "\n" in value or "\r" in value:
            raise ValueError("operator header value contains a line break")
        print(f"{name}: {value}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
