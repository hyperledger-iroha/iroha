#!/usr/bin/env python3
"""Audit an unchanged Apple static archive and its RustCrypto Cargo provenance.

The RustCrypto ML-DSA/ML-KEM implementation has no PQClean C archives to rewrite.
Reject retired providers and duplicate objects instead of repairing the archive.
The owning packager still authenticates compiler inputs, indexes the archive,
links every object, and exercises the public cryptographic ABI.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import stat
import sys
import tomllib

sys.path.insert(0, str(Path(__file__).resolve(strict=True).parent))
from native_sdk_source_custody import cargo_messages

MAGIC = b"!<arch>\n"
INDEX_NAMES = {"__.SYMDEF", "__.SYMDEF SORTED", "__.SYMDEF_64", "__.SYMDEF_64 SORTED", "/", "/SYM64/"}
PROVIDERS = {"ml-dsa", "ml-kem"}
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
RETIRED = re.compile(r"pqcrypto|pqclean|keccak[24]x", re.I)
SCHEMA = "iroha.native-static-archive-audit.v1"

def read_archive(data: bytes) -> list[tuple[str, memoryview, memoryview]]:
    """Read the pinned Apple archive format without extracting member paths."""
    if not data.startswith(MAGIC):
        raise ValueError("archive must be a regular ar archive")
    view = memoryview(data)
    offset = len(MAGIC)
    members = []
    while offset < len(data):
        header = data[offset : offset + 60]
        if len(header) != 60 or header[58:] != b"`\n":
            raise ValueError("archive has a malformed member header")
        encoded_size = header[48:58].strip()
        if not encoded_size or not encoded_size.isdigit():
            raise ValueError("archive has an invalid member length")
        size = int(encoded_size)
        end = offset + 60 + size
        padded_end = end + (size % 2)
        if padded_end > len(data):
            raise ValueError("archive member is truncated")
        if size % 2 and data[end:padded_end] != b"\n":
            raise ValueError("archive has invalid member padding")
        try:
            name = header[:16].decode("ascii").rstrip()
        except UnicodeError as error:
            raise ValueError("archive member name is not ASCII") from error
        body = view[offset + 60 : end]
        if name.startswith("#1/"):
            length_text = name[3:]
            if not length_text.isdigit() or not 0 < int(length_text) <= size:
                raise ValueError("archive has an invalid BSD member name length")
            length = int(length_text)
            encoded_name = bytes(body[:length]).rstrip(b"\0")
            try:
                name = encoded_name.decode("ascii")
            except UnicodeError as error:
                raise ValueError("archive BSD member name is not ASCII") from error
            body = body[length:]
        elif name not in INDEX_NAMES:
            if name.startswith("/"):
                raise ValueError("archive GNU long-name tables are not supported")
            name = name.removesuffix("/")
        if not name or "\0" in name or (name not in INDEX_NAMES and ("/" in name or "\\" in name)):
            raise ValueError("archive has an invalid member name")
        members.append((name, body, view[offset:padded_end]))
        offset = padded_end
    if not members:
        raise ValueError("archive contains no members")
    return members



def audit_archive_bytes(data: bytes) -> int:
    """Reject malformed, duplicate, or retired-provider members without mutation."""
    seen = set()
    for name, payload, _ in read_archive(data):
        if name in INDEX_NAMES:
            continue
        if name in seen:
            raise ValueError(f"duplicate archive member: {name}")
        if RETIRED.search(name) or b"PQCLEAN_" in bytes(payload):
            raise ValueError(f"retired PQClean archive member: {name}")
        seen.add(name)
    if not seen:
        raise ValueError("archive contains no object members")
    return len(seen)


def read_regular(path: Path) -> bytes:
    """Read only a canonical ordinary Cargo artifact or build record."""
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError(f"archive provenance input must be canonical: {path}")
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or path.is_symlink():
        raise ValueError(f"archive provenance input must be a regular file: {path}")
    return path.read_bytes()


def cargo_references(build_dir: Path, target: str, messages_path: Path, lock_path: Path):
    """Bind both pure-Rust providers to artifacts from this completed target build."""
    if not re.fullmatch(r"(?:aarch64|x86_64)-apple-(?:darwin|ios(?:-sim)?)", target):
        raise ValueError("archive audit requires a supported Apple target")
    if not build_dir.is_absolute() or build_dir.resolve(strict=True) != build_dir or not build_dir.is_dir():
        raise ValueError("Cargo provenance directories must be canonical and non-symbolic")
    lock_bytes = read_regular(lock_path)
    packages = tomllib.loads(lock_bytes.decode("utf-8")).get("package", [])
    if any(RETIRED.search(p.get("name", "")) for p in packages):
        raise ValueError("Cargo.lock contains a retired PQClean provider")
    selected = {}
    for name in sorted(PROVIDERS):
        matches = [p for p in packages if p.get("name") == name]
        if (len(matches) != 1 or matches[0].get("source") != REGISTRY
                or re.fullmatch(r"[0-9a-f]{64}", matches[0].get("checksum", "")) is None
                or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", matches[0].get("version", "")) is None):
            raise ValueError(f"Cargo.lock requires one checksummed registry provider: {name}")
        selected[name] = matches[0]
    messages_bytes = read_regular(messages_path)
    def unique_members(pairs):
        values = {}
        for name, value in pairs:
            if name in values:
                raise ValueError("Cargo provenance contains a duplicate JSON member: " + name)
            values[name] = value
        return values
    messages = cargo_messages(messages_bytes.decode("utf-8"), unique_members)
    references = {str(lock_path): lock_bytes, str(messages_path): messages_bytes}
    providers = {}
    for name, package in selected.items():
        package_id = REGISTRY + "#" + name + "@" + package["version"]
        candidates = []
        for message in messages:
            if RETIRED.search(message.get("package_id", "")):
                raise ValueError("Cargo invocation contains a retired PQClean provider")
            if message.get("package_id") != package_id or message.get("reason") != "compiler-artifact":
                continue
            if message.get("target", {}).get("name") != name.replace("-", "_"):
                raise ValueError("Cargo provider target identity differs")
            for value in message.get("filenames", []):
                path = Path(value)
                if path.parent == build_dir.parent / "deps" and re.fullmatch(
                        r"lib" + name.replace("-", "_") + r"-[0-9a-f]+\.rlib", path.name):
                    candidates.append(path)
        if len(candidates) != 1:
            raise ValueError(f"Cargo invocation requires one target rlib for {name}")
        path = candidates[0]
        raw = read_regular(path)
        # Parse the rlib too: source custody proves the crate bytes; this proves
        # the referenced artifact is a real, nonempty archive at verification.
        read_archive(raw)
        references[str(path)] = raw
        providers[name] = {"cargo_package_id": package_id, "cargo_package_checksum": package["checksum"],
                           "artifact": str(path), "artifact_sha256": hashlib.sha256(raw).hexdigest()}
    provenance = {"cargo_lock_sha256": hashlib.sha256(lock_bytes).hexdigest(),
                  "cargo_messages_sha256": hashlib.sha256(messages_bytes).hexdigest(), "providers": providers}
    return references, provenance


def main() -> int:
    """Retain read-only archive and exact Cargo provider evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--cargo-build-dir", type=Path, required=True)
    parser.add_argument("--cargo-messages", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--cargo-lock", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    original = read_regular(args.library)
    count = audit_archive_bytes(original)
    _, provenance = cargo_references(args.cargo_build_dir, args.target, args.cargo_messages, args.cargo_lock)
    report = {"schema": SCHEMA, "target": args.target, "archive_sha256": hashlib.sha256(original).hexdigest(),
              "object_count": count, "references": provenance}
    with args.report.open("x", encoding="utf-8") as output:
        output.write(json.dumps(report, sort_keys=True, indent=2) + "\n")
    print(f"[+] Unchanged native archive audited: {count} objects ({args.target})", file=sys.stderr)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, UnicodeError, ValueError, RuntimeError) as error:
        print(f"[-] Native static archive audit rejected: {error}", file=sys.stderr)
        raise SystemExit(1) from error
