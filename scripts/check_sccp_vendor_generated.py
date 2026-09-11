#!/usr/bin/env python3
"""Authenticate registered SCCP vendor outputs against their pinned Go modules.

Requires Python 3.10+ with scripts/requirements.txt installed. No Go toolchain or
environment variables are required. The default reads the official Go proxy;
--module-cache selects an existing read-only Go cache/download directory offline.

This read-only check verifies imported upstream bytes, not execution of upstream
generators. Selection comes from generated-files.toml; ownership and versions
come from vendor/modules.txt and go.mod; authentication uses the existing go.sum.
Module h1 uses Go's sorted filename/content digest (including the ZIP prefix):
https://go.dev/ref/mod#authenticating
https://go.googlesource.com/mod/+/refs/heads/master/sumdb/dirhash/hash.go
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
import time
from typing import Callable
import urllib.error
import urllib.request
import zipfile

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10
    import tomli as tomllib


ENTRY_PREFIX = "sccp-vendor-"
VENDOR_PREFIX = "circuits/sccp/vendor/"
PIN_PATHS = ("circuits/sccp/go.mod", "circuits/sccp/go.sum", VENDOR_PREFIX + "modules.txt")
MAX_PIN_BYTES = 2 * 1024 * 1024
MAX_SOURCE_BYTES = 8 * 1024 * 1024
MAX_ARCHIVE_BYTES = 32 * 1024 * 1024
MAX_UNCOMPRESSED_BYTES = 128 * 1024 * 1024
MAX_ARCHIVE_ENTRIES = 20_000
NETWORK_TIMEOUT_SECONDS = 20
NETWORK_DEADLINE_SECONDS = 60
MODULE = re.compile(r"[A-Za-z0-9._~-]+(?:/[A-Za-z0-9._~-]+)+\Z")
VERSION = re.compile(
    r"v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z.-]+)?(?:\+incompatible)?\Z"
)
CHECKSUM = re.compile(r"h1:[A-Za-z0-9+/]{43}=\Z")


class VerificationError(ValueError):
    """A source, ownership, authentication, or resource-bound check failed."""


def normalized_path(value: str) -> str:
    """Require an unambiguous relative POSIX path."""
    if (
        not isinstance(value, str)
        or not value
        or "\\" in value
        or any(ord(char) < 32 or ord(char) == 127 for char in value)
        or any(part in ("", ".", "..") for part in value.split("/"))
    ):
        raise VerificationError(f"unsafe path: {value!r}")
    return value


def read_regular(root: Path, relative: str, limit: int) -> bytes:
    """Read a bounded regular file without following any relative symlink."""
    parts = normalized_path(relative).split("/")
    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    directory = os.open(root, directory_flags)
    try:
        for part in parts[:-1]:
            next_directory = os.open(part, directory_flags, dir_fd=directory)
            os.close(directory)
            directory = next_directory
        descriptor = os.open(
            parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory
        )
        with os.fdopen(descriptor, "rb") as source:
            before = os.fstat(source.fileno())
            if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
                raise VerificationError(f"non-regular or oversized input: {relative}")
            data = source.read(limit + 1)
            after = os.fstat(source.fileno())
        fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
        if len(data) > limit or any(getattr(before, key) != getattr(after, key) for key in fields):
            raise VerificationError(f"oversized or changing input: {relative}")
        return data
    finally:
        os.close(directory)


def require_pin(module: str, version: str) -> None:
    """Reject replacement syntax and malformed module/version paths."""
    normalized_path(module)
    if not MODULE.fullmatch(module) or not VERSION.fullmatch(version):
        raise VerificationError(f"invalid module pin: {module} {version}")


def requirements(source: str) -> dict[str, str]:
    """Parse this repository's explicit, replacement-free go.mod format."""
    result: dict[str, str] = {}
    headers: set[str] = set()
    in_require = False
    for raw in source.splitlines():
        line = raw.partition("//")[0].strip()
        if not line:
            continue
        if not in_require and line == "require (":
            in_require = True
            continue
        if in_require and line == ")":
            in_require = False
            continue
        words = line.split()
        if not in_require:
            if words[0] in ("module", "go", "toolchain") and len(words) == 2:
                if words[0] in headers:
                    raise VerificationError(f"duplicate go.mod header: {words[0]}")
                headers.add(words[0])
                continue
            if words[0] != "require":
                raise VerificationError(f"unsupported go.mod directive: {line}")
            words = words[1:]
        if len(words) != 2:
            raise VerificationError(f"ambiguous go.mod requirement: {line}")
        module, version = words
        require_pin(module, version)
        if module in result:
            raise VerificationError(f"duplicate go.mod requirement: {module}")
        result[module] = version
    if in_require or not result or not {"module", "go"} <= headers:
        raise VerificationError("incomplete go.mod requirements or headers")
    return result


def checksums(source: str) -> dict[tuple[str, str], str]:
    """Read unique module and go.mod checksums without adding a baseline."""
    result: dict[tuple[str, str], str] = {}
    for line in source.splitlines():
        words = line.split()
        if len(words) != 3:
            raise VerificationError(f"malformed go.sum row: {line}")
        module, version, checksum = words
        require_pin(module, version.removesuffix("/go.mod"))
        key = (module, version)
        if not CHECKSUM.fullmatch(checksum) or key in result:
            raise VerificationError(f"invalid or duplicate go.sum checksum: {module} {version}")
        result[key] = checksum
    return result


def vendor_modules(source: str) -> tuple[dict[str, str], dict[str, str], set[str]]:
    """Resolve vendored packages to unique explicit module owners."""
    modules: dict[str, str] = {}
    packages: dict[str, str] = {}
    explicit: set[str] = set()
    metadata: set[str] = set()
    owner: str | None = None
    for line in source.splitlines():
        if line.startswith("# "):
            words = line[2:].split()
            if len(words) != 2:
                raise VerificationError(f"unsupported vendor module header: {line}")
            owner, version = words
            require_pin(owner, version)
            if owner in modules:
                raise VerificationError(f"duplicate vendor module: {owner}")
            modules[owner] = version
        elif line.startswith("## "):
            if owner is None or owner in metadata:
                raise VerificationError("unowned or duplicate vendor module metadata")
            metadata.add(owner)
            if "explicit" in [part.strip() for part in line[3:].split(";")]:
                explicit.add(owner)
        elif line:
            normalized_path(line)
            if owner is None or not (line == owner or line.startswith(owner + "/")):
                raise VerificationError(f"package outside its vendor module: {line}")
            if line in packages:
                raise VerificationError(f"duplicate vendor package: {line}")
            packages[line] = owner
    return modules, packages, explicit


def registered_outputs(source: bytes) -> dict[str, str]:
    """Select SCCP vendor files exclusively from the generated-source registry."""
    manifest = tomllib.loads(source.decode("utf-8"))
    entries = manifest.get("generated")
    if manifest.get("schema_version") != 1 or not isinstance(entries, list):
        raise VerificationError("unsupported generated-source registry")
    result: dict[str, str] = {}
    names: set[str] = set()
    for entry in entries:
        if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
            raise VerificationError("malformed generated-source entry")
        name = entry["name"]
        if not name.startswith(ENTRY_PREFIX):
            continue
        outputs = entry.get("outputs")
        if (
            name in names
            or entry.get("external") is not True
            or entry.get("kind") != "file"
            or not isinstance(outputs, list)
            or not outputs
        ):
            raise VerificationError(f"invalid vendor registry entry: {name}")
        names.add(name)
        for output in outputs:
            normalized_path(output)
            if (
                not output.startswith(VENDOR_PREFIX)
                or not output.endswith(".go")
                or output in result
            ):
                raise VerificationError(f"invalid or duplicate vendor output: {output}")
            result[output] = name
    if not result:
        raise VerificationError("no SCCP vendor generated-source entries registered")
    return result


def h1_summary(file_hashes: dict[str, str]) -> str:
    """Implement the Go Hash1 filename/content summary, not a ZIP byte hash."""
    digest = hashlib.sha256()
    for name in sorted(file_hashes):
        digest.update(f"{file_hashes[name]}  {name}\n".encode("utf-8"))
    return "h1:" + base64.b64encode(digest.digest()).decode("ascii")


def authenticated_outputs(
    blob: bytes,
    module: str,
    version: str,
    checksum: str,
    mod_checksum: str,
    outputs: set[str],
) -> tuple[dict[str, bytes], int]:
    """Authenticate the whole bounded archive before exposing selected bytes."""
    require_pin(module, version)
    if len(blob) > MAX_ARCHIVE_BYTES:
        raise VerificationError("oversized module archive")
    prefix = f"{module}@{version}/"
    file_hashes: dict[str, str] = {}
    selected: dict[str, bytes] = {}
    folded: set[str] = set()
    total = 0
    with zipfile.ZipFile(io.BytesIO(blob)) as archive:
        if len(archive.infolist()) > MAX_ARCHIVE_ENTRIES:
            raise VerificationError("too many module archive entries")
        for info in archive.infolist():
            name = normalized_path(info.filename)
            mode = stat.S_IFMT(info.external_attr >> 16)
            if (
                info.orig_filename != info.filename
                or not name.startswith(prefix)
                or name in file_hashes
                or name.casefold() in folded
                or mode not in (0, stat.S_IFREG)
                or info.flag_bits & 1
                or info.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED)
                or info.file_size > MAX_SOURCE_BYTES
            ):
                raise VerificationError(f"invalid, duplicate, or oversized archive entry: {name}")
            folded.add(name.casefold())
            with archive.open(info) as member:
                data = member.read(MAX_SOURCE_BYTES + 1)
            total += len(data)
            if len(data) > MAX_SOURCE_BYTES or total > MAX_UNCOMPRESSED_BYTES:
                raise VerificationError("module archive exceeds decoded size limit")
            file_hashes[name] = hashlib.sha256(data).hexdigest()
            relative = name[len(prefix):]
            if relative in outputs or relative == "go.mod":
                selected[relative] = data
    if h1_summary(file_hashes) != checksum:
        raise VerificationError(f"module archive does not authenticate against go.sum: {module} {version}")
    if "go.mod" not in selected or h1_summary(
        {"go.mod": hashlib.sha256(selected["go.mod"]).hexdigest()}
    ) != mod_checksum:
        raise VerificationError(f"archive go.mod does not authenticate against go.sum: {module} {version}")
    missing = outputs - selected.keys()
    if missing:
        raise VerificationError(f"registered outputs absent from authenticated archive: {sorted(missing)}")
    return {name: selected[name] for name in outputs}, len(file_hashes)


def escaped_pin(value: str) -> str:
    """Apply Go module cache/proxy uppercase escaping."""
    return "".join("!" + char.lower() if "A" <= char <= "Z" else char for char in value)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    """Require the official proxy response itself; permit no redirect follow-up."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise VerificationError("module proxy redirect rejected; use an authenticated offline module cache")


def fetch_archive(module: str, version: str) -> bytes:
    """Download once with byte, read-timeout, and overall deadline bounds."""
    require_pin(module, version)
    url = f"https://proxy.golang.org/{escaped_pin(module)}/@v/{escaped_pin(version)}.zip"
    opener = urllib.request.build_opener(NoRedirect())
    deadline = time.monotonic() + NETWORK_DEADLINE_SECONDS
    with opener.open(url, timeout=NETWORK_TIMEOUT_SECONDS) as response:
        if response.status != 200:
            raise VerificationError(f"module proxy HTTP status: {response.status}")
        lengths = response.headers.get_all("Content-Length", [])
        if len(lengths) > 1 or (
            lengths and (not lengths[0].isascii() or not lengths[0].isdigit())
        ):
            raise VerificationError("ambiguous module proxy content length")
        expected = int(lengths[0]) if lengths else None
        if expected is not None and expected > MAX_ARCHIVE_BYTES:
            raise VerificationError("oversized module proxy response")
        result = bytearray()
        while True:
            if time.monotonic() >= deadline:
                raise VerificationError("module proxy deadline exceeded")
            chunk = response.read1(min(64 * 1024, MAX_ARCHIVE_BYTES + 1 - len(result)))
            if not chunk:
                break
            result.extend(chunk)
            if len(result) > MAX_ARCHIVE_BYTES:
                raise VerificationError("oversized module proxy response")
        if time.monotonic() >= deadline:
            raise VerificationError("module proxy deadline exceeded")
        if expected is not None and len(result) != expected:
            raise VerificationError("truncated module proxy response")
        return bytes(result)


def verify(
    root: Path,
    manifest_path: str = "generated-files.toml",
    archive_loader: Callable[[str, str], bytes] = fetch_archive,
) -> dict:
    """Verify registered ownership, pinned archive authenticity, and byte drift."""
    inputs = {
        path: read_regular(root, path, MAX_PIN_BYTES)
        for path in (*PIN_PATHS, manifest_path)
    }
    outputs = registered_outputs(inputs[manifest_path])
    pins = requirements(inputs[PIN_PATHS[0]].decode("utf-8"))
    sums = checksums(inputs[PIN_PATHS[1]].decode("utf-8"))
    modules, packages, explicit = vendor_modules(inputs[PIN_PATHS[2]].decode("utf-8"))
    groups: dict[str, dict[str, str]] = {}
    for path in outputs:
        relative = path.removeprefix(VENDOR_PREFIX)
        package = relative.rsplit("/", 1)[0]
        owner = packages.get(package)
        if owner is None or owner not in explicit or pins.get(owner) != modules[owner]:
            raise VerificationError(f"output has no matching explicit go.mod/vendor owner: {path}")
        groups.setdefault(owner, {})[path] = relative.removeprefix(owner + "/")
    reports = []
    for module, paths in sorted(groups.items()):
        version = modules[module]
        checksum = sums.get((module, version))
        mod_checksum = sums.get((module, version + "/go.mod"))
        if checksum is None or mod_checksum is None:
            raise VerificationError(f"missing archive or go.mod checksum: {module} {version}")
        authenticated, entry_count = authenticated_outputs(
            archive_loader(module, version), module, version, checksum, mod_checksum, set(paths.values())
        )
        verified = []
        for path, relative in sorted(paths.items()):
            upstream = authenticated[relative]
            header = upstream.split(b"\n", 1)[0]
            if not header.startswith(b"// Code generated "):
                raise VerificationError(f"registered file lacks upstream generated header: {path}")
            local = read_regular(root, path, MAX_SOURCE_BYTES)
            if local != upstream:
                raise VerificationError(f"vendored generated bytes differ from authenticated upstream: {path}")
            inputs[path] = local
            verified.append({
                "path": path,
                "registry_owner": outputs[path],
                "sha256": hashlib.sha256(local).hexdigest(),
                "upstream_header": header.decode("utf-8"),
            })
        reports.append({
            "module": module,
            "version": version,
            "h1": checksum,
            "go_mod_h1": mod_checksum,
            "authenticated_archive_entries": entry_count,
            "outputs": verified,
        })
    for path, initial in inputs.items():
        if read_regular(root, path, MAX_SOURCE_BYTES if path in outputs else MAX_PIN_BYTES) != initial:
            raise VerificationError(f"source changed during verification: {path}")
    return {
        "modules": reports,
        "verified_output_count": len(outputs),
        "inputs_sha256": {
            path: hashlib.sha256(data).hexdigest()
            for path, data in sorted(inputs.items())
        },
        "upstream_generator_execution": "not performed",
        "repository_writes": False,
    }


def main(argv: list[str] | None = None) -> int:
    """Check the live registry with official downloads or a read-only Go cache."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument(
        "--manifest", default="generated-files.toml", help="registry path relative to --root"
    )
    parser.add_argument(
        "--module-cache", type=Path, help="offline Go cache/download directory (read-only)"
    )
    args = parser.parse_args(argv)
    loader = fetch_archive
    if args.module_cache is not None:
        def loader(module: str, version: str) -> bytes:
            require_pin(module, version)
            return read_regular(
                args.module_cache,
                f"{escaped_pin(module)}/@v/{escaped_pin(version)}.zip",
                MAX_ARCHIVE_BYTES,
            )
    try:
        print(json.dumps(verify(args.root, args.manifest, loader), indent=2, sort_keys=True))
    except (OSError, ValueError, zipfile.BadZipFile, zipfile.LargeZipFile, urllib.error.URLError) as error:
        print(f"SCCP vendor generated-source verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
