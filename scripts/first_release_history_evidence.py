#!/usr/bin/env python3
"""Record or verify the evidence of a history that the current binary no longer reads.

Contract: `specs/first_release_history_cutover.md`, "Obsolete histories". An
obsolete history is kept as opaque bytes with digests; the current tree gains
no decoder for it. This tool hashes files. It never opens a store through Kura,
decodes a block or changes an input.

`record` writes one JSON manifest (`iroha.obsolete_history_evidence.v1`) of:

- the stopped node's store directory: every file with its size and SHA-256;
- the signed genesis file;
- the `iroha3d --check-storage` and `iroha3d --check-config --json` reports
  printed by the build that wrote the store;
- optional logs and exported telemetry;
- the source revision and build identity of that build.

The genesis hash, network, build identity and storage tip are copied from the
two reports. They are what the writing build reported, not values this tool
verified: diagnose the history with a binary built from the recorded revision.

`verify` recomputes every digest from the same inputs and fails on any
difference, so custody can be checked later without the old binary.

Prerequisites: Python 3.10+, no third-party module. The node must be stopped.
Inputs are opened read-only; the manifest must be written outside the store.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

SCHEMA = "iroha.obsolete_history_evidence.v1"
CHUNK = 1024 * 1024
# The store lock belongs to whichever process holds the store; it is not history.
STORE_LOCK = ".kura.lock"


class EvidenceError(Exception):
    """An input cannot be recorded or no longer matches its manifest."""


def sha256_file(path: Path) -> tuple[int, str]:
    """Size and SHA-256 of one regular file, read without following a final symlink."""
    digest = hashlib.sha256()
    size = 0
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "rb") as file:
        if not stat.S_ISREG(os.fstat(file.fileno()).st_mode):
            raise EvidenceError(f"{path}: not a regular file")
        while chunk := file.read(CHUNK):
            digest.update(chunk)
            size += len(chunk)
    return size, digest.hexdigest()


def store_entries(store: Path) -> list[dict]:
    """Every entry below the store directory, sorted by path, with its digest or link target."""
    if not store.is_dir() or store.is_symlink():
        raise EvidenceError(f"{store}: the store must be a real directory")
    entries: list[dict] = []
    for parent, directories, names in os.walk(store, followlinks=False):
        directories.sort()
        base = Path(parent)
        for name in sorted(directories):
            path = base / name
            if path.is_symlink():
                entries.append(
                    {
                        "path": path.relative_to(store).as_posix(),
                        "kind": "symlink",
                        "target": os.readlink(path),
                    }
                )
        directories[:] = [name for name in directories if not (base / name).is_symlink()]
        for name in sorted(names):
            path = base / name
            relative = path.relative_to(store).as_posix()
            if relative == STORE_LOCK:
                continue
            if path.is_symlink():
                entries.append({"path": relative, "kind": "symlink", "target": os.readlink(path)})
                continue
            if not stat.S_ISREG(path.lstat().st_mode):
                raise EvidenceError(f"{path}: not a regular file, directory or symlink")
            size, digest = sha256_file(path)
            entries.append({"path": relative, "kind": "file", "bytes": size, "sha256": digest})
    return sorted(entries, key=lambda entry: entry["path"])


def tree_digest(entries: list[dict]) -> str:
    """One digest over the sorted store entries."""
    digest = hashlib.sha256()
    for entry in entries:
        value = entry.get("sha256") or entry.get("target")
        digest.update(f"{entry['kind']}\0{entry['path']}\0{entry.get('bytes', 0)}\0{value}\n".encode())
    return digest.hexdigest()


def single_file(role: str, path: Path) -> dict:
    """The manifest entry of one evidence file outside the store."""
    if path.is_symlink() or not path.is_file():
        raise EvidenceError(f"{path}: {role} must be a regular file")
    size, digest = sha256_file(path)
    return {"role": role, "name": path.name, "bytes": size, "sha256": digest}


def read_report(path: Path, role: str) -> dict:
    """A probe report as the JSON object the writing build printed."""
    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"{path}: unreadable {role}: {error}") from error
    if not isinstance(report, dict):
        raise EvidenceError(f"{path}: {role} must be a JSON object")
    return report


def reported_values(storage: dict, config: dict) -> dict:
    """What the writing build reported about the history; copied, not verified."""
    identity = config.get("node_identity") or {}
    build = config.get("diagnostic_build") or {}
    return {
        "genesis_hash": identity.get("genesis_hash"),
        "network_id": identity.get("network_id"),
        "tip_height": storage.get("tip_height"),
        "tip_hash": storage.get("tip_hash"),
        "protocol_version": config.get("protocol_version"),
        "wire_schema_hash": config.get("wire_schema_hash"),
        "gas_schedule_hash": config.get("gas_schedule_hash"),
        "build_version": build.get("version"),
        "build_source_revision": build.get("source_revision"),
        "build_fingerprint": build.get("build_fingerprint"),
    }


def build_manifest(args: argparse.Namespace) -> dict:
    """Hash every input and assemble the evidence manifest."""
    store = args.store.resolve()
    storage = read_report(args.check_storage_report, "--check-storage report")
    config = read_report(args.check_config_report, "--check-config report")
    reported = reported_values(storage, config)
    revision = args.source_revision or reported["build_source_revision"]
    identity = args.build_identity or reported["build_fingerprint"]
    if not revision or not identity:
        raise EvidenceError(
            "the source revision and build identity of the writing build are required: pass "
            "--source-revision and --build-identity, or a --check-config report that carries "
            "`diagnostic_build`"
        )
    if not reported["genesis_hash"]:
        raise EvidenceError(
            "the --check-config report carries no genesis hash (`node_identity` is null): run "
            "the probe with the signed genesis available"
        )
    files = [
        single_file("genesis", args.genesis),
        single_file("check_storage_report", args.check_storage_report),
        single_file("check_config_report", args.check_config_report),
    ]
    files += [single_file("log", log) for log in args.log]
    entries = store_entries(store)
    if not entries:
        raise EvidenceError(f"{store}: the store holds no file")
    return {
        "schema": SCHEMA,
        "source_revision": revision,
        "build_identity": identity,
        "reported_by_writing_build": reported,
        "files": files,
        "store": {
            "entries": entries,
            "file_count": sum(1 for entry in entries if entry["kind"] == "file"),
            "bytes": sum(entry.get("bytes", 0) for entry in entries),
            "tree_sha256": tree_digest(entries),
        },
    }


def record(args: argparse.Namespace) -> int:
    """Write the manifest of the given evidence; refuse to write into the store."""
    output = args.output.resolve()
    store = args.store.resolve()
    if output == store or store in output.parents:
        raise EvidenceError(f"{output}: the manifest must be written outside the store")
    manifest = build_manifest(args)
    with open(output, "x", encoding="utf-8") as file:
        json.dump(manifest, file, indent=2, sort_keys=True)
        file.write("\n")
    print(f"recorded {manifest['store']['file_count']} store files, tree {manifest['store']['tree_sha256']}")
    return 0


def verify(args: argparse.Namespace) -> int:
    """Recompute the digests and compare them with a recorded manifest."""
    try:
        recorded = json.loads(args.manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"{args.manifest}: unreadable manifest: {error}") from error
    if not isinstance(recorded, dict) or recorded.get("schema") != SCHEMA:
        raise EvidenceError(f"{args.manifest}: schema must be {SCHEMA}")
    args.source_revision = recorded.get("source_revision")
    args.build_identity = recorded.get("build_identity")
    current = build_manifest(args)
    differences: list[str] = []
    for key in ("reported_by_writing_build", "files"):
        if current[key] != recorded.get(key):
            differences.append(f"`{key}` differs from the manifest")
    before = {entry["path"]: entry for entry in recorded.get("store", {}).get("entries", [])}
    after = {entry["path"]: entry for entry in current["store"]["entries"]}
    for path in sorted(set(before) | set(after)):
        if path not in after:
            differences.append(f"store file {path} is missing")
        elif path not in before:
            differences.append(f"store file {path} is not in the manifest")
        elif before[path] != after[path]:
            differences.append(f"store file {path} changed")
    if current["store"]["tree_sha256"] != recorded.get("store", {}).get("tree_sha256"):
        differences.append("store tree digest differs from the manifest")
    if differences:
        raise EvidenceError("\n".join(differences))
    print(f"evidence matches the manifest, tree {current['store']['tree_sha256']}")
    return 0


def parser() -> argparse.ArgumentParser:
    """Command-line interface."""
    result = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    commands = result.add_subparsers(dest="command", required=True)
    for name, text in (("record", "write a new manifest"), ("verify", "check a manifest")):
        command = commands.add_parser(name, help=text)
        command.add_argument("--store", type=Path, required=True, help="stopped store directory")
        command.add_argument("--genesis", type=Path, required=True, help="signed genesis file")
        command.add_argument(
            "--check-storage-report",
            type=Path,
            required=True,
            help="output of `iroha3d --check-storage` from the writing build",
        )
        command.add_argument(
            "--check-config-report",
            type=Path,
            required=True,
            help="output of `iroha3d --check-config --json` from the writing build",
        )
        command.add_argument(
            "--log", type=Path, action="append", default=[], help="log or telemetry export"
        )
        if name == "record":
            command.add_argument("--source-revision", help="source revision of the writing build")
            command.add_argument("--build-identity", help="build identity of the writing build")
            command.add_argument("--output", type=Path, required=True, help="new manifest file")
        else:
            command.add_argument("--manifest", type=Path, required=True, help="recorded manifest")
    return result


def main(argv: list[str] | None = None) -> int:
    """Record or verify; print one line on success and the reason on failure."""
    args = parser().parse_args(argv)
    try:
        return record(args) if args.command == "record" else verify(args)
    except FileExistsError:
        print(f"{args.output}: already exists; a manifest is never overwritten", file=sys.stderr)
        return 1
    except EvidenceError as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
