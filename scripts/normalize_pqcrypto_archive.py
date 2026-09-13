#!/usr/bin/env python3
"""Normalize duplicated, provenance-checked PQClean members in Apple archives.

The locked pqcrypto-internals 0.2.11 build script emits both cc's ``static=``
link directive and an additional unqualified directive for its common archives.
Rust can consequently bundle those exact objects twice in a staticlib. Only
the second byte-identical copy of a known object from the authenticated Cargo
build output is removed here. Complete-archive linking and crypto checks still
run on the resulting archive in the owning Apple packager.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
import tomllib


MAGIC = b"!<arch>\n"
INDEX_NAMES = {"__.SYMDEF", "__.SYMDEF SORTED", "__.SYMDEF_64", "__.SYMDEF_64 SORTED", "/", "/SYM64/"}
COMMON_OBJECTS = {"aes", "fips202", "sha2", "nistseedexpander", "sp800-185"}
ARCH_OBJECTS = {
    "libkeccak2x.a": {"fips202x2", "feat"},
    "libkeccak4x.a": {"KeccakP-1600-times4-SIMD256"},
}
OBJECT_NAME = re.compile(r"[0-9a-f]{16}-(.+)\.o\Z")


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


def trusted_reference_members(reference_archives: dict[str, bytes]) -> dict[str, memoryview]:
    """Validate the exact locked common-library inventory before trusting it."""
    keys = set(reference_archives)
    architecture = keys & ARCH_OBJECTS.keys()
    if len(architecture) != 1 or keys != {"libpqclean_common.a", *architecture}:
        raise ValueError("reference archives must be pqclean_common and one known keccak backend")
    trusted = {}
    for archive_name, data in reference_archives.items():
        expected = COMMON_OBJECTS if archive_name == "libpqclean_common.a" else ARCH_OBJECTS[archive_name]
        found = set()
        for name, payload, _ in read_archive(data):
            if name in INDEX_NAMES:
                continue
            match = OBJECT_NAME.fullmatch(name)
            if match is None or match[1] not in expected or match[1] in found or name in trusted:
                raise ValueError(f"reference archive has an unexpected or duplicated member: {name}")
            found.add(match[1])
            trusted[name] = payload
        if found != expected:
            raise ValueError(f"reference archive has an incomplete object inventory: {archive_name}")
    return trusted


def normalize_archive_bytes(data: bytes, reference_archives: dict[str, bytes]) -> tuple[bytes, list[str]]:
    """Drop only proven second PQClean copies, preserving every other member."""
    trusted = trusted_reference_members(reference_archives)
    members = read_archive(data)
    seen = {}
    counts: Counter[str] = Counter()
    removed = []
    kept = []
    for name, payload, raw in members:
        if name in INDEX_NAMES:
            continue
        counts[name] += 1
        if name in trusted and payload != trusted[name]:
            raise ValueError(f"conflicting payload for reference member: {name}")
        if name in seen:
            if name not in trusted:
                raise ValueError(f"unknown duplicate archive member: {name}")
            if payload != seen[name]:
                raise ValueError(f"conflicting duplicate archive member: {name}")
            if counts[name] > 2:
                raise ValueError(f"reference archive member occurs more than twice: {name}")
            removed.append(name)
            continue
        seen[name] = payload
        kept.append(raw)
    if not trusted.keys() <= seen.keys():
        raise ValueError("archive is missing an authenticated reference member")
    if not removed:
        return data, []
    # The original symbol index points to old offsets. The owner must regenerate
    # it with the selected Xcode ranlib before complete-archive linking.
    return MAGIC + b"".join(kept), removed


def read_regular(path: Path) -> bytes:
    """Read an ordinary, non-symbolic Cargo artifact or build record."""
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or path.is_symlink():
        raise ValueError(f"archive provenance input must be a regular file: {path}")
    return path.read_bytes()


def cargo_references(
    build_dir: Path, target: str, messages_path: Path, package_root: Path,
) -> tuple[dict[str, bytes], dict[str, str]]:
    """Bind references to the exact package selected by this completed Cargo invocation."""
    if target.startswith("aarch64-apple-"):
        backend = "libkeccak2x.a"
    elif target.startswith("x86_64-apple-"):
        backend = "libkeccak4x.a"
    else:
        raise ValueError("archive normalization requires a supported Apple target")
    for path in (build_dir, package_root):
        if not path.is_absolute() or path.resolve(strict=True) != path or not path.is_dir():
            raise ValueError("Cargo provenance directories must be canonical and non-symbolic")
    manifest_bytes = read_regular(package_root / "Cargo.toml")
    package = tomllib.loads(manifest_bytes.decode("utf-8")).get("package", {})
    if package.get("name") != "pqcrypto-internals" or package.get("version") != "0.2.11":
        raise ValueError("Cargo package provenance requires pqcrypto-internals 0.2.11")
    package_id = package_root.as_uri().replace("file:", "path+file:", 1) + "#pqcrypto-internals@0.2.11"
    messages_bytes = read_regular(messages_path)
    messages = [json.loads(line) for line in messages_bytes.decode("utf-8").splitlines()]
    if (not messages or any(not isinstance(message, dict) for message in messages)
            or messages[-1] != {"reason": "build-finished", "success": True}
            or sum(message.get("reason") == "build-finished" for message in messages) != 1):
        raise ValueError("Cargo provenance requires one successfully completed build")
    candidates = []
    for message in messages:
        if message.get("reason") != "build-script-executed" or message.get("package_id") != package_id:
            continue
        out_value = message.get("out_dir")
        if not isinstance(out_value, str):
            raise ValueError("Cargo package output directory is malformed")
        out_dir = Path(out_value)
        # A package may also be built for the host. Only this target's exact output is eligible.
        if out_dir.parent.parent != build_dir:
            continue
        if (out_dir.name != "out"
                or not re.fullmatch(r"pqcrypto-internals-[0-9a-f]+", out_dir.parent.name)
                or not out_dir.is_absolute() or out_dir.resolve(strict=True) != out_dir):
            raise ValueError("Cargo package output directory is not canonical")
        candidates.append((message, out_dir))
    if len(candidates) != 1:
        raise ValueError(f"reference provenance requires exactly one current Cargo build output, found {len(candidates)}")
    message, out_dir = candidates[0]
    search = f"native={out_dir}"
    links = {"static=pqclean_common", f"static={backend[3:-2]}"}
    if (not isinstance(message.get("linked_paths"), list)
            or search not in message["linked_paths"]
            or not isinstance(message.get("linked_libs"), list)
            or not links <= set(message["linked_libs"])):
        raise ValueError("current Cargo build output has conflicting link directives")
    output = out_dir.parent / "output"
    output_bytes = read_regular(output)
    lines = output_bytes.decode("utf-8").splitlines()
    required = {f"cargo:rustc-link-search={search}", *{f"cargo:rustc-link-lib={link}" for link in links}}
    if not required <= set(lines):
        raise ValueError("recorded Cargo build output has conflicting link directives")
    archives = {name: read_regular(out_dir / name) for name in ("libpqclean_common.a", backend)}
    trusted_reference_members(archives)
    return archives, {
        "cargo_build_output": str(output),
        "cargo_build_output_sha256": hashlib.sha256(output_bytes).hexdigest(),
        "cargo_messages_sha256": hashlib.sha256(messages_bytes).hexdigest(),
        "cargo_package_id": package_id,
        "cargo_package_manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
        **{name: hashlib.sha256(value).hexdigest() for name, value in archives.items()},
    }


def main() -> int:
    """Normalize one staged archive, retaining a local provenance report."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--cargo-build-dir", type=Path, required=True)
    parser.add_argument("--cargo-messages", type=Path, required=True)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--cargo-lock", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    lock = tomllib.loads(read_regular(args.cargo_lock).decode("utf-8"))
    packages = [item for item in lock.get("package", []) if item.get("name") == "pqcrypto-internals"]
    if len(packages) != 1 or packages[0].get("version") != "0.2.11":
        raise ValueError("reference provenance requires locked pqcrypto-internals 0.2.11")
    original = read_regular(args.library)
    references, provenance = cargo_references(
        args.cargo_build_dir, args.target, args.cargo_messages, args.package_root,
    )
    normalized, removed = normalize_archive_bytes(original, references)
    report = {
        "schema": "iroha.pqcrypto-common-archive-normalization.v1",
        "target": args.target,
        "input_sha256": hashlib.sha256(original).hexdigest(),
        "unindexed_output_sha256": hashlib.sha256(normalized).hexdigest(),
        "removed_identical_members": removed,
        "references": provenance,
    }
    if removed:
        fd, temporary = tempfile.mkstemp(prefix=".pqcrypto-normalized-", dir=args.library.parent)
        try:
            with os.fdopen(fd, "wb") as handle:
                handle.write(normalized)
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temporary, args.library)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
    args.report.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n")
    print(f"[+] Proven PQClean duplicate members normalized: {len(removed)} ({args.target})", file=sys.stderr)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, UnicodeError, ValueError) as error:
        print(f"[-] PQClean archive normalization rejected: {error}", file=sys.stderr)
        raise SystemExit(1) from error
