#!/usr/bin/env python3
"""Verify the immutable Git lineage behind the build-efficiency contract.

The signed anchor is pinned by its commit object and by the issuer metadata
embedded in its OpenPGP signature.  This checker deliberately does not claim
that the signature or its issuer is cryptographically authenticated: no
trusted public-key material is part of this repository contract.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, MutableMapping, Sequence


SCHEMA_VERSION = 1
OBJECT_FORMAT = "sha1"
OID_RE = re.compile(r"[0-9a-f]{40}\Z")
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
REGULAR_FILE_MODES = frozenset({"100644", "100755"})

REQUIRED_ROLE_IDS = {
    "implementation_origin": "6552815b5abb0ebe8921bcfcdbcec76f464216ae",
    "donor": "d130c985b46ff8beb99c20b25b36bbdeb506e4c9",
    "source_budget_baseline": "cd05eebfc07c9742734b9d684394c4fe89cdb7c5",
    "protected_integration": "d248cbd127b0188282b761c85239a62c0c4c3d80",
    "signed_lock_anchor": "69dc60078fbeb015ad3da0987aaa823373fd0fc4",
}
REQUIRED_ANCESTRY = (
    ("implementation_origin", "donor"),
    ("donor", "protected_integration"),
    ("source_budget_baseline", "protected_integration"),
    ("protected_integration", "signed_lock_anchor"),
    ("signed_lock_anchor", "HEAD"),
)
REQUIRED_SELECTED_ORIGINS = {
    ".github/workflows/pr.yml": "donor",
    "ci/README.md": "donor",
    "ci/compile_unit_baselines.json": "donor",
    "ci/dependency_budget.json": "protected_integration",
    "ci/source_file_budget.json": "donor",
    "docs/profile_build.md": "donor",
    "scripts/check_compile_unit_budget.py": "donor",
    "scripts/check_dependency_budget.py": "donor",
    "scripts/check_source_file_budget.py": "donor",
    "scripts/profile_cargo_build.py": "protected_integration",
    "scripts/tests/check_compile_unit_budget_test.py": "donor",
    "scripts/tests/check_dependency_budget_test.py": "protected_integration",
    "scripts/tests/check_source_file_budget_test.py": "donor",
    "scripts/tests/profile_cargo_build_test.py": "protected_integration",
}
REQUIRED_SOURCE_BUDGET = {
    "path": "ci/source_file_budget.json",
    "schema_version": 1,
    "baseline": 5_067_263,
    "ceiling": 4_540_000,
    "excluded_prefixes": (
        "docs/portal/node_modules/",
        "target/",
        "vendor/",
    ),
    "commit_role": "source_budget_baseline",
}
REQUIRED_LOCK_PATH = "Cargo.lock"
REQUIRED_SIGNATURE_KIND = "openpgp_v4_issuer_structure"


class ProvenanceError(ValueError):
    """Raised when provenance evidence is malformed or does not match Git."""


@dataclass(frozen=True)
class TreeEntry:
    """One exact entry from a Git tree."""

    mode: str
    object_type: str
    oid: str
    path: str


@dataclass(frozen=True)
class CommitHeaders:
    """The commit headers relevant to the provenance contract."""

    tree: str
    parents: tuple[str, ...]
    gpgsig: str | None


def parse_args() -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Verify build-efficiency Git lineage without fetching objects."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (default: inferred from this script)",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("ci/build_efficiency_provenance.json"),
        help="repository-relative provenance manifest",
    )
    return parser.parse_args()


def _reject_json_constant(value: str) -> None:
    raise ProvenanceError(f"non-standard JSON constant is forbidden: {value}")


def _unique_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ProvenanceError(f"duplicate JSON key: {key!r}")
        result[key] = value
    return result


def strict_json_loads(text: str, label: str) -> Any:
    """Decode strict JSON, rejecting duplicate keys and non-standard numbers."""
    try:
        return json.loads(
            text,
            object_pairs_hook=_unique_json_object,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as err:
        raise ProvenanceError(f"{label} is not valid JSON: {err}") from err


def read_regular_bytes(path: Path, label: str) -> bytes:
    """Read one regular, non-symlink file as exact bytes."""
    try:
        metadata = path.lstat()
    except OSError as err:
        raise ProvenanceError(f"cannot stat {label}: {err}") from err
    if not stat.S_ISREG(metadata.st_mode):
        raise ProvenanceError(f"{label} must be a regular file")
    try:
        return path.read_bytes()
    except OSError as err:
        raise ProvenanceError(f"cannot read {label}: {err}") from err


def read_regular_utf8(path: Path, label: str) -> str:
    """Read one regular, non-symlink UTF-8 file."""
    try:
        return read_regular_bytes(path, label).decode("utf-8")
    except UnicodeError as err:
        raise ProvenanceError(f"cannot read {label} as UTF-8: {err}") from err


def strict_json_file(path: Path, label: str) -> Any:
    """Read one strict JSON object from a regular file."""
    return strict_json_loads(read_regular_utf8(path, label), label)


def require_object(value: Any, label: str) -> dict[str, Any]:
    """Require a JSON object."""
    if not isinstance(value, dict):
        raise ProvenanceError(f"{label} must be a JSON object")
    return value


def require_exact_keys(
    value: Mapping[str, Any], expected: set[str], label: str
) -> None:
    """Require exactly the named object keys."""
    actual = set(value)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        details: list[str] = []
        if missing:
            details.append(f"missing={missing}")
        if extra:
            details.append(f"extra={extra}")
        raise ProvenanceError(f"{label} has invalid keys ({', '.join(details)})")


def require_string(value: Any, label: str) -> str:
    """Require a non-empty string."""
    if not isinstance(value, str) or not value:
        raise ProvenanceError(f"{label} must be a non-empty string")
    return value


def require_int(value: Any, label: str, *, positive: bool = False) -> int:
    """Require an integer while rejecting booleans."""
    if isinstance(value, bool) or not isinstance(value, int):
        raise ProvenanceError(f"{label} must be an integer")
    if positive and value <= 0:
        raise ProvenanceError(f"{label} must be greater than zero")
    if not positive and value < 0:
        raise ProvenanceError(f"{label} must be non-negative")
    return value


def require_bool(value: Any, label: str) -> bool:
    """Require a JSON boolean."""
    if not isinstance(value, bool):
        raise ProvenanceError(f"{label} must be a boolean")
    return value


def require_oid(value: Any, label: str) -> str:
    """Require one canonical SHA-1 object identifier."""
    oid = require_string(value, label)
    if OID_RE.fullmatch(oid) is None:
        raise ProvenanceError(f"{label} must be a lowercase 40-hex SHA-1")
    return oid


def require_sha256(value: Any, label: str) -> str:
    """Require one canonical SHA-256 digest."""
    digest = require_string(value, label)
    if SHA256_RE.fullmatch(digest) is None:
        raise ProvenanceError(f"{label} must be a lowercase 64-hex SHA-256")
    return digest


def require_safe_path(value: Any, label: str) -> str:
    """Require a canonical repository-relative POSIX path."""
    path = require_string(value, label)
    if "\\" in path or "\0" in path:
        raise ProvenanceError(f"{label} is not a safe repository-relative path")
    pure = PurePosixPath(path)
    if (
        pure.is_absolute()
        or path != pure.as_posix()
        or path in {".", ".."}
        or any(part in {"", ".", ".."} for part in pure.parts)
    ):
        raise ProvenanceError(f"{label} is not a canonical repository-relative path")
    return path


def require_prefix(value: Any, label: str) -> str:
    """Require a canonical repository-relative directory prefix."""
    prefix = require_string(value, label)
    if not prefix.endswith("/"):
        raise ProvenanceError(f"{label} must end with '/'")
    require_safe_path(prefix[:-1], label)
    return prefix


def sanitized_git_environment(
    source: Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Return a minimal environment with Git configuration injection disabled."""
    inherited = os.environ if source is None else source
    environment: dict[str, str] = {}
    for key in ("PATH", "SYSTEMROOT", "WINDIR", "TMPDIR", "TMP", "TEMP"):
        value = inherited.get(key)
        if value:
            environment[key] = value
    environment.update(
        {
            "LANG": "C",
            "LC_ALL": "C",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_SYSTEM": os.devnull,
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_TERMINAL_PROMPT": "0",
        }
    )
    return environment


class GitObjectStore:
    """Read-only access to local Git objects with a sanitized environment."""

    def __init__(self, root: Path) -> None:
        self.root = root.resolve(strict=True)
        environment = sanitized_git_environment()
        executable = shutil.which("git", path=environment.get("PATH"))
        if executable is None:
            raise ProvenanceError("git executable was not found in PATH")
        self._environment = environment
        self._prefix = (
            executable,
            "--no-pager",
            "--no-replace-objects",
            "-c",
            "core.fsmonitor=false",
            "-c",
            f"core.hooksPath={os.devnull}",
        )
        top = self._run("rev-parse", "--show-toplevel").stdout
        try:
            top_path = Path(top.decode("utf-8").strip()).resolve(strict=True)
        except (OSError, UnicodeError) as err:
            raise ProvenanceError(f"Git returned an invalid repository root: {err}") from err
        if top_path != self.root:
            raise ProvenanceError(
                f"--root is {self.root}, but Git reports repository root {top_path}"
            )

    def _run(
        self,
        *arguments: str,
        input_bytes: bytes | None = None,
        accepted_codes: frozenset[int] = frozenset({0}),
    ) -> subprocess.CompletedProcess[bytes]:
        try:
            result = subprocess.run(
                [*self._prefix, *arguments],
                cwd=self.root,
                env=self._environment,
                input=input_bytes,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
        except OSError as err:
            raise ProvenanceError(f"cannot execute local Git: {err}") from err
        if result.returncode not in accepted_codes:
            detail = result.stderr.decode("utf-8", errors="replace").strip()
            if len(detail) > 500:
                detail = f"{detail[:500]}..."
            raise ProvenanceError(
                f"git {' '.join(arguments)} failed with {result.returncode}: {detail}"
            )
        return result

    def object_format(self) -> str:
        """Return the repository object format."""
        raw = self._run("rev-parse", "--show-object-format").stdout
        try:
            return raw.decode("ascii").strip()
        except UnicodeError as err:
            raise ProvenanceError("Git returned a non-ASCII object format") from err

    def head(self) -> str:
        """Resolve HEAD to one commit without accepting an abbreviated ID."""
        raw = self._run("rev-parse", "--verify", "HEAD^{commit}").stdout
        try:
            return require_oid(raw.decode("ascii").strip(), "HEAD")
        except UnicodeError as err:
            raise ProvenanceError("Git returned a non-ASCII HEAD") from err

    def object_bytes(self, oid: str, expected_type: str) -> bytes:
        """Read one local object after checking its type."""
        require_oid(oid, "Git object id")
        if expected_type not in {"blob", "commit", "tree"}:
            raise ProvenanceError(f"unsupported Git object type: {expected_type}")
        actual = self._run("cat-file", "-t", oid).stdout
        try:
            actual_type = actual.decode("ascii").strip()
        except UnicodeError as err:
            raise ProvenanceError(f"object {oid} has a non-ASCII type") from err
        if actual_type != expected_type:
            raise ProvenanceError(
                f"object {oid} is {actual_type!r}, expected {expected_type!r}"
            )
        return self._run("cat-file", expected_type, oid).stdout

    @staticmethod
    def _parse_tree_output(raw: bytes, label: str) -> list[TreeEntry]:
        entries: list[TreeEntry] = []
        seen: set[str] = set()
        for record in raw.split(b"\0"):
            if not record:
                continue
            try:
                metadata, raw_path = record.split(b"\t", 1)
                mode, object_type, raw_oid = metadata.split(b" ")
                path = raw_path.decode("utf-8")
                entry = TreeEntry(
                    mode=mode.decode("ascii"),
                    object_type=object_type.decode("ascii"),
                    oid=raw_oid.decode("ascii"),
                    path=path,
                )
            except (ValueError, UnicodeError) as err:
                raise ProvenanceError(f"{label} contains a malformed tree entry") from err
            require_safe_path(entry.path, f"{label} path")
            require_oid(entry.oid, f"{label} object id")
            if entry.path in seen:
                raise ProvenanceError(f"{label} repeats path {entry.path!r}")
            seen.add(entry.path)
            entries.append(entry)
        return entries

    def tree_entries(self, commit: str) -> list[TreeEntry]:
        """Return all entries in one commit tree."""
        require_oid(commit, "tree commit")
        raw = self._run("ls-tree", "-r", "-z", "--full-tree", commit).stdout
        return self._parse_tree_output(raw, f"tree {commit}")

    def tree_entry(self, commit: str, path: str) -> TreeEntry | None:
        """Return one exact path from a commit tree, or ``None`` if absent."""
        require_oid(commit, "tree commit")
        require_safe_path(path, "tree path")
        raw = self._run(
            "ls-tree", "-r", "-z", "--full-tree", commit, "--", path
        ).stdout
        entries = self._parse_tree_output(raw, f"tree {commit}")
        exact = [entry for entry in entries if entry.path == path]
        if len(exact) > 1:
            raise ProvenanceError(f"tree {commit} repeats exact path {path!r}")
        return exact[0] if exact else None

    def is_ancestor(self, ancestor: str, descendant: str) -> bool:
        """Return whether one pinned commit is an ancestor of another."""
        require_oid(ancestor, "ancestor commit")
        require_oid(descendant, "descendant commit")
        result = self._run(
            "merge-base",
            "--is-ancestor",
            ancestor,
            descendant,
            accepted_codes=frozenset({0, 1}),
        )
        return result.returncode == 0

    def blob_bytes_many(self, oids: Sequence[str]) -> dict[str, bytes]:
        """Read exact blob objects through one local ``cat-file --batch`` call."""
        ordered = list(dict.fromkeys(oids))
        for oid in ordered:
            require_oid(oid, "batch blob id")
        if not ordered:
            return {}
        raw = self._run(
            "cat-file",
            "--batch",
            input_bytes="".join(f"{oid}\n" for oid in ordered).encode("ascii"),
        ).stdout
        cursor = 0
        blobs: dict[str, bytes] = {}
        for expected_oid in ordered:
            newline = raw.find(b"\n", cursor)
            if newline < 0:
                raise ProvenanceError("git cat-file --batch returned a truncated header")
            try:
                oid, object_type, size_text = raw[cursor:newline].decode("ascii").split()
                size = int(size_text)
            except (UnicodeError, ValueError) as err:
                raise ProvenanceError("git cat-file --batch returned a malformed header") from err
            cursor = newline + 1
            if oid != expected_oid or object_type != "blob" or size < 0:
                raise ProvenanceError(
                    f"git cat-file --batch returned unexpected header for {expected_oid}"
                )
            end = cursor + size
            if end >= len(raw) or raw[end : end + 1] != b"\n":
                raise ProvenanceError(f"git cat-file --batch truncated blob {expected_oid}")
            blobs[oid] = raw[cursor:end]
            cursor = end + 1
        if cursor != len(raw):
            raise ProvenanceError("git cat-file --batch returned trailing data")
        return blobs


def git_object_id(object_type: str, payload: bytes) -> str:
    """Compute a SHA-1 Git object identifier from canonical object bytes."""
    header = f"{object_type} {len(payload)}\0".encode("ascii")
    return hashlib.sha1(header + payload).hexdigest()


def verify_object_id(oid: str, object_type: str, payload: bytes) -> None:
    """Verify that bytes hash to their advertised SHA-1 object identifier."""
    observed = git_object_id(object_type, payload)
    if observed != oid:
        raise ProvenanceError(
            f"{object_type} object {oid} hashes to unexpected id {observed}"
        )


def parse_commit_headers(raw: bytes) -> CommitHeaders:
    """Parse tree, ordered parents, and an optional folded ``gpgsig`` header."""
    header_block, separator, _message = raw.partition(b"\n\n")
    if not separator:
        raise ProvenanceError("commit object has no header/message separator")
    unfolded: list[tuple[str, str]] = []
    current_key: str | None = None
    current_value: list[str] = []
    for raw_line in header_block.split(b"\n"):
        if raw_line.startswith(b" "):
            if current_key is None:
                raise ProvenanceError("commit begins with a continuation header")
            try:
                current_value.append(raw_line[1:].decode("utf-8"))
            except UnicodeError as err:
                raise ProvenanceError("commit header is not UTF-8") from err
            continue
        if current_key is not None:
            unfolded.append((current_key, "\n".join(current_value)))
        key_bytes, separator, value_bytes = raw_line.partition(b" ")
        if not separator:
            raise ProvenanceError("commit contains a header without a value")
        try:
            current_key = key_bytes.decode("ascii")
            current_value = [value_bytes.decode("utf-8")]
        except UnicodeError as err:
            raise ProvenanceError("commit header is not valid text") from err
    if current_key is not None:
        unfolded.append((current_key, "\n".join(current_value)))

    trees = [value for key, value in unfolded if key == "tree"]
    parents = [value for key, value in unfolded if key == "parent"]
    signatures = [value for key, value in unfolded if key == "gpgsig"]
    if len(trees) != 1:
        raise ProvenanceError("commit must contain exactly one tree header")
    if len(signatures) > 1:
        raise ProvenanceError("commit contains multiple gpgsig headers")
    tree = require_oid(trees[0], "commit tree")
    for index, parent in enumerate(parents):
        require_oid(parent, f"commit parent {index}")
    return CommitHeaders(
        tree=tree,
        parents=tuple(parents),
        gpgsig=signatures[0] if signatures else None,
    )


def _crc24(payload: bytes) -> int:
    crc = 0xB704CE
    for byte in payload:
        crc ^= byte << 16
        for _ in range(8):
            crc <<= 1
            if crc & 0x1000000:
                crc ^= 0x1864CFB
    return crc & 0xFFFFFF


def decode_ascii_armored_signature(armor: str) -> bytes:
    """Decode one ASCII-armored signature and verify its CRC-24 checksum."""
    try:
        armor.encode("ascii")
    except UnicodeError as err:
        raise ProvenanceError("gpgsig armor must be ASCII") from err
    lines = armor.splitlines()
    if (
        len(lines) < 5
        or lines[0] != "-----BEGIN PGP SIGNATURE-----"
        or lines[-1] != "-----END PGP SIGNATURE-----"
    ):
        raise ProvenanceError("gpgsig is not a complete PGP SIGNATURE armor block")
    try:
        separator = lines.index("", 1)
    except ValueError as err:
        raise ProvenanceError("gpgsig armor has no header separator") from err
    for header in lines[1:separator]:
        if ":" not in header:
            raise ProvenanceError("gpgsig armor contains a malformed header")
    encoded_lines = lines[separator + 1 : -1]
    checksum_lines = [line for line in encoded_lines if line.startswith("=")]
    payload_lines = [line for line in encoded_lines if not line.startswith("=")]
    if len(checksum_lines) != 1 or encoded_lines[-1:] != checksum_lines:
        raise ProvenanceError("gpgsig armor must contain one final CRC-24 checksum")
    if not payload_lines or any(not line for line in payload_lines):
        raise ProvenanceError("gpgsig armor contains an empty payload line")
    try:
        payload = base64.b64decode("".join(payload_lines), validate=True)
        checksum = base64.b64decode(checksum_lines[0][1:], validate=True)
    except binascii.Error as err:
        raise ProvenanceError("gpgsig armor contains invalid base64") from err
    if len(checksum) != 3 or int.from_bytes(checksum, "big") != _crc24(payload):
        raise ProvenanceError("gpgsig armor CRC-24 does not match its payload")
    return payload


def _packet_length(payload: bytes, cursor: int, new_format: bool) -> tuple[int, int]:
    if cursor >= len(payload):
        raise ProvenanceError("OpenPGP packet has no length")
    first = payload[cursor]
    cursor += 1
    if not new_format:
        raise AssertionError("old-format length type must be handled by its header")
    if first < 192:
        return first, cursor
    if first < 224:
        if cursor >= len(payload):
            raise ProvenanceError("OpenPGP packet has a truncated two-octet length")
        return ((first - 192) << 8) + payload[cursor] + 192, cursor + 1
    if first == 255:
        if cursor + 4 > len(payload):
            raise ProvenanceError("OpenPGP packet has a truncated five-octet length")
        return int.from_bytes(payload[cursor : cursor + 4], "big"), cursor + 4
    raise ProvenanceError("partial OpenPGP packet lengths are not accepted")


def extract_signature_packet(payload: bytes) -> bytes:
    """Return the body of one complete OpenPGP signature packet."""
    if not payload or payload[0] & 0x80 == 0:
        raise ProvenanceError("OpenPGP payload has no packet header")
    first = payload[0]
    cursor = 1
    if first & 0x40:
        tag = first & 0x3F
        length, cursor = _packet_length(payload, cursor, True)
    else:
        tag = (first >> 2) & 0x0F
        length_type = first & 0x03
        width = (1, 2, 4, 0)[length_type]
        if width == 0:
            raise ProvenanceError("indeterminate old-format packet lengths are forbidden")
        if cursor + width > len(payload):
            raise ProvenanceError("OpenPGP packet has a truncated old-format length")
        length = int.from_bytes(payload[cursor : cursor + width], "big")
        cursor += width
    if tag != 2:
        raise ProvenanceError(f"OpenPGP packet tag is {tag}, expected signature tag 2")
    if cursor + length != len(payload):
        raise ProvenanceError("OpenPGP armor must contain exactly one signature packet")
    return payload[cursor:]


def _subpacket_length(payload: bytes, cursor: int) -> tuple[int, int]:
    if cursor >= len(payload):
        raise ProvenanceError("OpenPGP signature subpacket has no length")
    first = payload[cursor]
    cursor += 1
    if first < 192:
        return first, cursor
    if first < 255:
        if cursor >= len(payload):
            raise ProvenanceError("OpenPGP subpacket has a truncated length")
        return ((first - 192) << 8) + payload[cursor] + 192, cursor + 1
    if cursor + 4 > len(payload):
        raise ProvenanceError("OpenPGP subpacket has a truncated long length")
    return int.from_bytes(payload[cursor : cursor + 4], "big"), cursor + 4


def parse_signature_subpackets(payload: bytes, label: str) -> list[tuple[int, bytes]]:
    """Parse a complete sequence of OpenPGP signature subpackets."""
    result: list[tuple[int, bytes]] = []
    cursor = 0
    while cursor < len(payload):
        length, cursor = _subpacket_length(payload, cursor)
        if length < 1 or cursor + length > len(payload):
            raise ProvenanceError(f"{label} contains a malformed subpacket length")
        packet = payload[cursor : cursor + length]
        cursor += length
        result.append((packet[0] & 0x7F, packet[1:]))
    return result


def verify_openpgp_issuer_structure(
    armor: str, expected_fingerprint: str, expected_key_id: str
) -> None:
    """Verify issuer subpacket structure without authenticating the signature."""
    packet = extract_signature_packet(decode_ascii_armored_signature(armor))
    if len(packet) < 10 or packet[0] != 4:
        raise ProvenanceError("gpgsig must contain a version 4 signature packet")
    hashed_length = int.from_bytes(packet[4:6], "big")
    hashed_end = 6 + hashed_length
    if hashed_end + 2 > len(packet):
        raise ProvenanceError("gpgsig has a truncated hashed subpacket area")
    unhashed_length = int.from_bytes(packet[hashed_end : hashed_end + 2], "big")
    unhashed_start = hashed_end + 2
    unhashed_end = unhashed_start + unhashed_length
    if unhashed_end + 2 > len(packet):
        raise ProvenanceError("gpgsig has a truncated unhashed subpacket area")
    hashed = parse_signature_subpackets(packet[6:hashed_end], "hashed issuer area")
    unhashed = parse_signature_subpackets(
        packet[unhashed_start:unhashed_end], "unhashed issuer area"
    )
    fingerprints = [body for packet_type, body in hashed if packet_type == 33]
    key_ids = [body for packet_type, body in unhashed if packet_type == 16]
    expected_fingerprint_bytes = bytes.fromhex(expected_fingerprint)
    expected_key_id_bytes = bytes.fromhex(expected_key_id)
    if fingerprints != [b"\x04" + expected_fingerprint_bytes]:
        raise ProvenanceError("gpgsig hashed issuer fingerprint does not match")
    if key_ids != [expected_key_id_bytes]:
        raise ProvenanceError("gpgsig unhashed issuer key id does not match")
    if expected_fingerprint_bytes[-8:] != expected_key_id_bytes:
        raise ProvenanceError("manifest issuer key id is not the fingerprint suffix")


def validate_manifest_schema(payload: Any) -> dict[str, Any]:
    """Validate every manifest key and primitive type."""
    manifest = require_object(payload, "provenance manifest")
    require_exact_keys(
        manifest,
        {
            "schema_version",
            "object_format",
            "lineage",
            "ancestry",
            "selected_paths",
            "signed_lock_anchor",
            "source_budget",
        },
        "provenance manifest",
    )
    if require_int(manifest["schema_version"], "schema_version") != SCHEMA_VERSION:
        raise ProvenanceError(f"schema_version must be {SCHEMA_VERSION}")
    if require_string(manifest["object_format"], "object_format") != OBJECT_FORMAT:
        raise ProvenanceError(f"object_format must be {OBJECT_FORMAT!r}")

    lineage = require_object(manifest["lineage"], "lineage")
    require_exact_keys(lineage, set(REQUIRED_ROLE_IDS), "lineage")
    for role, pinned_commit in REQUIRED_ROLE_IDS.items():
        record = require_object(lineage[role], f"lineage.{role}")
        require_exact_keys(record, {"commit", "tree", "parents", "rust"}, f"lineage.{role}")
        commit = require_oid(record["commit"], f"lineage.{role}.commit")
        if commit != pinned_commit:
            raise ProvenanceError(
                f"lineage.{role}.commit is {commit}, expected pinned {pinned_commit}"
            )
        require_oid(record["tree"], f"lineage.{role}.tree")
        parents = record["parents"]
        if not isinstance(parents, list) or not parents:
            raise ProvenanceError(f"lineage.{role}.parents must be a non-empty array")
        for index, parent in enumerate(parents):
            require_oid(parent, f"lineage.{role}.parents[{index}]")
        rust = require_object(record["rust"], f"lineage.{role}.rust")
        require_exact_keys(rust, {"paths", "lines"}, f"lineage.{role}.rust")
        require_int(rust["paths"], f"lineage.{role}.rust.paths", positive=True)
        require_int(rust["lines"], f"lineage.{role}.rust.lines", positive=True)

    ancestry = manifest["ancestry"]
    if not isinstance(ancestry, list):
        raise ProvenanceError("ancestry must be an array")
    parsed_ancestry: list[tuple[str, str]] = []
    for index, raw_edge in enumerate(ancestry):
        edge = require_object(raw_edge, f"ancestry[{index}]")
        require_exact_keys(edge, {"ancestor", "descendant"}, f"ancestry[{index}]")
        parsed_ancestry.append(
            (
                require_string(edge["ancestor"], f"ancestry[{index}].ancestor"),
                require_string(edge["descendant"], f"ancestry[{index}].descendant"),
            )
        )
    if tuple(parsed_ancestry) != REQUIRED_ANCESTRY:
        raise ProvenanceError("ancestry does not match the required ordered lineage")

    selected = manifest["selected_paths"]
    if not isinstance(selected, list):
        raise ProvenanceError("selected_paths must be an array")
    expected_paths = sorted(REQUIRED_SELECTED_ORIGINS)
    observed_paths: list[str] = []
    for index, raw_entry in enumerate(selected):
        entry = require_object(raw_entry, f"selected_paths[{index}]")
        require_exact_keys(
            entry,
            {"path", "origin", "donor", "protected_integration"},
            f"selected_paths[{index}]",
        )
        path = require_safe_path(entry["path"], f"selected_paths[{index}].path")
        origin = require_string(entry["origin"], f"selected_paths[{index}].origin")
        observed_paths.append(path)
        expected_origin = REQUIRED_SELECTED_ORIGINS.get(path)
        if origin != expected_origin:
            raise ProvenanceError(
                f"selected path {path!r} origin is {origin!r}, expected {expected_origin!r}"
            )
        for state_name in ("donor", "protected_integration"):
            raw_state = entry[state_name]
            if raw_state is None:
                if state_name != "donor" or origin != "protected_integration":
                    raise ProvenanceError(f"selected path {path!r} has an invalid null state")
                continue
            state = require_object(raw_state, f"selected path {path!r} {state_name}")
            require_exact_keys(state, {"mode", "blob"}, f"selected path {path!r} {state_name}")
            mode = require_string(state["mode"], f"selected path {path!r} {state_name}.mode")
            if mode not in REGULAR_FILE_MODES:
                raise ProvenanceError(f"selected path {path!r} has invalid mode {mode!r}")
            require_oid(state["blob"], f"selected path {path!r} {state_name}.blob")
        if origin == "donor" and entry["donor"] is None:
            raise ProvenanceError(f"donor-origin path {path!r} must have a donor state")
    if observed_paths != expected_paths:
        raise ProvenanceError(
            "selected_paths must contain exactly "
            f"the {len(expected_paths)} required paths in sorted order"
        )

    anchor = require_object(manifest["signed_lock_anchor"], "signed_lock_anchor")
    require_exact_keys(
        anchor,
        {"commit_role", "signature", "cargo_lock", "source_file_budget"},
        "signed_lock_anchor",
    )
    if anchor["commit_role"] != "signed_lock_anchor":
        raise ProvenanceError("signed_lock_anchor.commit_role must name signed_lock_anchor")
    signature = require_object(anchor["signature"], "signed_lock_anchor.signature")
    require_exact_keys(
        signature,
        {
            "kind",
            "issuer_fingerprint",
            "issuer_key_id",
            "cryptographic_signer_authentication",
        },
        "signed_lock_anchor.signature",
    )
    if signature["kind"] != REQUIRED_SIGNATURE_KIND:
        raise ProvenanceError(
            f"signed_lock_anchor.signature.kind must be {REQUIRED_SIGNATURE_KIND!r}"
        )
    if require_bool(
        signature["cryptographic_signer_authentication"],
        "signed_lock_anchor.signature.cryptographic_signer_authentication",
    ):
        raise ProvenanceError(
            "cryptographic_signer_authentication must remain false without a trusted key"
        )
    fingerprint = require_string(
        signature["issuer_fingerprint"],
        "signed_lock_anchor.signature.issuer_fingerprint",
    )
    key_id = require_string(signature["issuer_key_id"], "signed_lock_anchor.signature.issuer_key_id")
    if re.fullmatch(r"[0-9a-f]{40}", fingerprint) is None:
        raise ProvenanceError("issuer_fingerprint must be lowercase 40-hex")
    if re.fullmatch(r"[0-9a-f]{16}", key_id) is None:
        raise ProvenanceError("issuer_key_id must be lowercase 16-hex")
    if not fingerprint.endswith(key_id):
        raise ProvenanceError("issuer_key_id must be the issuer_fingerprint suffix")

    lock = require_object(anchor["cargo_lock"], "signed_lock_anchor.cargo_lock")
    require_exact_keys(lock, {"path", "mode", "blob", "bytes", "sha256"}, "signed_lock_anchor.cargo_lock")
    if require_safe_path(lock["path"], "signed_lock_anchor.cargo_lock.path") != REQUIRED_LOCK_PATH:
        raise ProvenanceError(f"cargo_lock.path must be {REQUIRED_LOCK_PATH!r}")
    if lock["mode"] != "100644":
        raise ProvenanceError("cargo_lock.mode must be '100644'")
    require_oid(lock["blob"], "signed_lock_anchor.cargo_lock.blob")
    require_int(lock["bytes"], "signed_lock_anchor.cargo_lock.bytes", positive=True)
    require_sha256(lock["sha256"], "signed_lock_anchor.cargo_lock.sha256")

    budget_artifact = require_object(
        anchor["source_file_budget"], "signed_lock_anchor.source_file_budget"
    )
    require_exact_keys(
        budget_artifact,
        {"path", "mode", "blob", "bytes", "sha256"},
        "signed_lock_anchor.source_file_budget",
    )
    budget_path = require_safe_path(
        budget_artifact["path"], "signed_lock_anchor.source_file_budget.path"
    )
    if budget_path != REQUIRED_SOURCE_BUDGET["path"]:
        raise ProvenanceError(
            "source_file_budget.path must be "
            f"{REQUIRED_SOURCE_BUDGET['path']!r}"
        )
    if budget_artifact["mode"] != "100644":
        raise ProvenanceError("source_file_budget.mode must be '100644'")
    require_oid(
        budget_artifact["blob"], "signed_lock_anchor.source_file_budget.blob"
    )
    require_int(
        budget_artifact["bytes"],
        "signed_lock_anchor.source_file_budget.bytes",
        positive=True,
    )
    require_sha256(
        budget_artifact["sha256"],
        "signed_lock_anchor.source_file_budget.sha256",
    )

    source_budget = require_object(manifest["source_budget"], "source_budget")
    require_exact_keys(
        source_budget,
        {"path", "schema_version", "baseline", "ceiling", "excluded_prefixes", "commit_role"},
        "source_budget",
    )
    for key in ("path", "commit_role"):
        actual = require_string(source_budget[key], f"source_budget.{key}")
        if actual != REQUIRED_SOURCE_BUDGET[key]:
            raise ProvenanceError(
                f"source_budget.{key} is {actual!r}, expected {REQUIRED_SOURCE_BUDGET[key]!r}"
            )
    require_safe_path(source_budget["path"], "source_budget.path")
    for key in ("schema_version", "baseline", "ceiling"):
        actual = require_int(source_budget[key], f"source_budget.{key}", positive=True)
        if actual != REQUIRED_SOURCE_BUDGET[key]:
            raise ProvenanceError(
                f"source_budget.{key} is {actual}, expected {REQUIRED_SOURCE_BUDGET[key]}"
            )
    prefixes = source_budget["excluded_prefixes"]
    if not isinstance(prefixes, list):
        raise ProvenanceError("source_budget.excluded_prefixes must be an array")
    parsed_prefixes = tuple(
        require_prefix(prefix, f"source_budget.excluded_prefixes[{index}]")
        for index, prefix in enumerate(prefixes)
    )
    if parsed_prefixes != REQUIRED_SOURCE_BUDGET["excluded_prefixes"]:
        raise ProvenanceError("source_budget.excluded_prefixes do not match the source guard")
    return manifest


def _state_from_manifest(raw: Any, label: str) -> tuple[str, str] | None:
    if raw is None:
        return None
    state = require_object(raw, label)
    return str(state["mode"]), str(state["blob"])


def verify_tree_state(
    store: Any,
    commit: str,
    path: str,
    expected: tuple[str, str] | None,
    label: str,
    verified_blobs: MutableMapping[str, bytes],
) -> None:
    """Compare one exact tree state and verify the referenced blob object."""
    observed = store.tree_entry(commit, path)
    if expected is None:
        if observed is not None:
            raise ProvenanceError(f"{label} unexpectedly contains {path!r}")
        return
    mode, oid = expected
    if observed is None:
        raise ProvenanceError(f"{label} is missing {path!r}")
    if observed.object_type != "blob" or (observed.mode, observed.oid) != (mode, oid):
        raise ProvenanceError(
            f"{label} state for {path!r} is "
            f"{observed.mode} {observed.object_type} {observed.oid}, "
            f"expected {mode} blob {oid}"
        )
    if oid not in verified_blobs:
        blob = store.object_bytes(oid, "blob")
        verify_object_id(oid, "blob", blob)
        verified_blobs[oid] = blob


def historical_rust_count(
    store: Any,
    commit: str,
    excluded_prefixes: tuple[str, ...],
    line_cache: MutableMapping[str, int],
) -> tuple[int, int]:
    """Count tracked regular Rust files with source-budget line semantics."""
    entries: list[TreeEntry] = []
    for entry in store.tree_entries(commit):
        if (
            PurePosixPath(entry.path).suffix.lower() != ".rs"
            or entry.path.startswith(excluded_prefixes)
        ):
            continue
        if entry.object_type != "blob" or entry.mode not in REGULAR_FILE_MODES:
            raise ProvenanceError(
                f"historical Rust path {entry.path!r} is not a regular blob"
            )
        entries.append(entry)
    missing = [entry.oid for entry in entries if entry.oid not in line_cache]
    for oid, blob in store.blob_bytes_many(missing).items():
        verify_object_id(oid, "blob", blob)
        try:
            line_cache[oid] = len(blob.decode("utf-8").splitlines())
        except UnicodeError as err:
            raise ProvenanceError(f"historical Rust blob {oid} is not UTF-8") from err
    return len(entries), sum(line_cache[entry.oid] for entry in entries)


def verify_current_source_budget(
    root: Path,
    contract: Mapping[str, Any],
    pinned_bytes: bytes,
) -> None:
    """Verify the current source budget retains the complete pinned policy.

    Even a policy-tightening ratchet therefore requires an explicit signed-anchor
    retarget rather than an unpinned edit to limits, exceptions, or other fields.
    """
    path = root / str(contract["path"])
    label = str(contract["path"])
    current_bytes = read_regular_bytes(path, label)
    try:
        current_text = current_bytes.decode("utf-8")
    except UnicodeError as err:
        raise ProvenanceError(f"cannot read {label} as UTF-8: {err}") from err
    payload = require_object(
        strict_json_loads(current_text, label), "current source budget"
    )
    schema = require_int(payload.get("schema_version"), "current source budget.schema_version")
    if schema != contract["schema_version"]:
        raise ProvenanceError("current source budget schema_version changed")
    aggregate = require_object(payload.get("aggregate_rust"), "current source budget.aggregate_rust")
    baseline = require_int(aggregate.get("baseline"), "current source budget.aggregate_rust.baseline", positive=True)
    ceiling = require_int(aggregate.get("ceiling"), "current source budget.aggregate_rust.ceiling", positive=True)
    if baseline != contract["baseline"]:
        raise ProvenanceError(
            f"current source budget baseline is {baseline}, expected {contract['baseline']}"
        )
    if ceiling != contract["ceiling"]:
        raise ProvenanceError(
            f"current source budget ceiling is {ceiling}, expected {contract['ceiling']}"
        )
    prefixes = payload.get("excluded_prefixes")
    if not isinstance(prefixes, list):
        raise ProvenanceError("current source budget excluded_prefixes must be an array")
    parsed_prefixes = tuple(
        require_prefix(prefix, f"current source budget excluded_prefixes[{index}]")
        for index, prefix in enumerate(prefixes)
    )
    if parsed_prefixes != tuple(contract["excluded_prefixes"]):
        raise ProvenanceError("current source budget exclusions changed")
    if current_bytes != pinned_bytes:
        raise ProvenanceError(
            "current source budget bytes differ from the signed lock anchor"
        )


def validate_provenance(
    root: Path,
    payload: Any,
    store: Any,
    *,
    head_commit: str | None = None,
) -> dict[str, int]:
    """Validate the complete contract against one immutable HEAD snapshot."""
    manifest = validate_manifest_schema(payload)
    if store.object_format() != manifest["object_format"]:
        raise ProvenanceError(
            f"repository object format is not {manifest['object_format']!r}"
        )
    head = (
        store.head()
        if head_commit is None
        else require_oid(head_commit, "provenance HEAD snapshot")
    )
    head_raw = store.object_bytes(head, "commit")
    verify_object_id(head, "commit", head_raw)

    lineage = manifest["lineage"]
    commit_headers: dict[str, CommitHeaders] = {}
    line_cache: dict[str, int] = {}
    verified_parent_objects: set[str] = set()
    total_historical_paths = 0
    for role in REQUIRED_ROLE_IDS:
        record = lineage[role]
        commit = record["commit"]
        raw_commit = store.object_bytes(commit, "commit")
        verify_object_id(commit, "commit", raw_commit)
        headers = parse_commit_headers(raw_commit)
        commit_headers[role] = headers
        if headers.tree != record["tree"]:
            raise ProvenanceError(
                f"lineage.{role}.tree is {record['tree']}, commit records {headers.tree}"
            )
        expected_parents = tuple(record["parents"])
        if headers.parents != expected_parents:
            raise ProvenanceError(
                f"lineage.{role}.parents do not match the commit's ordered parents"
            )
        tree_raw = store.object_bytes(headers.tree, "tree")
        verify_object_id(headers.tree, "tree", tree_raw)
        for parent in headers.parents:
            if parent in verified_parent_objects:
                continue
            parent_raw = store.object_bytes(parent, "commit")
            verify_object_id(parent, "commit", parent_raw)
            verified_parent_objects.add(parent)
        observed_paths, observed_lines = historical_rust_count(
            store,
            commit,
            tuple(manifest["source_budget"]["excluded_prefixes"]),
            line_cache,
        )
        total_historical_paths += observed_paths
        rust = record["rust"]
        if (observed_paths, observed_lines) != (rust["paths"], rust["lines"]):
            raise ProvenanceError(
                f"lineage.{role}.rust observed {observed_paths} paths/{observed_lines} lines, "
                f"expected {rust['paths']} paths/{rust['lines']} lines"
            )

    for ancestor_role, descendant_role in REQUIRED_ANCESTRY:
        ancestor = lineage[ancestor_role]["commit"]
        descendant = head if descendant_role == "HEAD" else lineage[descendant_role]["commit"]
        if not store.is_ancestor(ancestor, descendant):
            raise ProvenanceError(
                f"required ancestry is false: {ancestor_role} -> {descendant_role}"
            )

    verified_blobs: dict[str, bytes] = {}
    donor_commit = lineage["donor"]["commit"]
    integration_commit = lineage["protected_integration"]["commit"]
    for entry in manifest["selected_paths"]:
        path = entry["path"]
        verify_tree_state(
            store,
            donor_commit,
            path,
            _state_from_manifest(entry["donor"], f"selected path {path!r} donor"),
            "donor",
            verified_blobs,
        )
        verify_tree_state(
            store,
            integration_commit,
            path,
            _state_from_manifest(entry["protected_integration"], f"selected path {path!r} integration"),
            "protected integration",
            verified_blobs,
        )

    anchor_contract = manifest["signed_lock_anchor"]
    signature_contract = anchor_contract["signature"]
    anchor_headers = commit_headers[anchor_contract["commit_role"]]
    if anchor_headers.gpgsig is None:
        raise ProvenanceError("signed lock anchor has no gpgsig header")
    verify_openpgp_issuer_structure(
        anchor_headers.gpgsig,
        signature_contract["issuer_fingerprint"],
        signature_contract["issuer_key_id"],
    )

    lock = anchor_contract["cargo_lock"]
    lock_state = (lock["mode"], lock["blob"])
    anchor_commit = lineage[anchor_contract["commit_role"]]["commit"]
    verify_tree_state(
        store,
        anchor_commit,
        lock["path"],
        lock_state,
        "signed lock anchor",
        verified_blobs,
    )
    verify_tree_state(
        store,
        head,
        lock["path"],
        lock_state,
        "HEAD",
        verified_blobs,
    )
    lock_bytes = verified_blobs[lock["blob"]]
    if len(lock_bytes) != lock["bytes"]:
        raise ProvenanceError(
            f"Cargo.lock has {len(lock_bytes)} bytes, expected {lock['bytes']}"
        )
    lock_sha256 = hashlib.sha256(lock_bytes).hexdigest()
    if lock_sha256 != lock["sha256"]:
        raise ProvenanceError(
            f"Cargo.lock SHA-256 is {lock_sha256}, expected {lock['sha256']}"
        )

    budget_artifact = anchor_contract["source_file_budget"]
    budget_state = (budget_artifact["mode"], budget_artifact["blob"])
    verify_tree_state(
        store,
        anchor_commit,
        budget_artifact["path"],
        budget_state,
        "signed lock anchor",
        verified_blobs,
    )
    verify_tree_state(
        store,
        head,
        budget_artifact["path"],
        budget_state,
        "HEAD",
        verified_blobs,
    )
    budget_bytes = verified_blobs[budget_artifact["blob"]]
    if len(budget_bytes) != budget_artifact["bytes"]:
        raise ProvenanceError(
            "ci/source_file_budget.json has "
            f"{len(budget_bytes)} bytes, expected {budget_artifact['bytes']}"
        )
    budget_sha256 = hashlib.sha256(budget_bytes).hexdigest()
    if budget_sha256 != budget_artifact["sha256"]:
        raise ProvenanceError(
            "ci/source_file_budget.json SHA-256 is "
            f"{budget_sha256}, expected {budget_artifact['sha256']}"
        )

    source_budget = manifest["source_budget"]
    baseline_role = source_budget["commit_role"]
    if lineage[baseline_role]["rust"]["lines"] != source_budget["baseline"]:
        raise ProvenanceError(
            "source budget baseline does not equal its pinned commit's Rust count"
        )
    verify_current_source_budget(root, source_budget, budget_bytes)
    return {
        "roles": len(lineage),
        "selected_paths": len(manifest["selected_paths"]),
        "historical_rust_paths": total_historical_paths,
        "cargo_lock_bytes": len(lock_bytes),
        "source_budget_bytes": len(budget_bytes),
    }


def main() -> int:
    args = parse_args()
    try:
        root = args.root.resolve(strict=True)
        manifest_path = args.manifest
        if manifest_path.is_absolute():
            raise ProvenanceError("--manifest must be repository-relative")
        manifest_relative = require_safe_path(manifest_path.as_posix(), "--manifest")
        manifest = strict_json_file(root / manifest_relative, manifest_relative)
        store = GitObjectStore(root)
        head_commit = store.head()
        report = validate_provenance(
            root,
            manifest,
            store,
            head_commit=head_commit,
        )
        if store.head() != head_commit:
            raise ProvenanceError("HEAD changed during provenance validation")
    except (OSError, ProvenanceError) as err:
        print(f"ERROR: build-efficiency provenance check failed: {err}", file=sys.stderr)
        return 2
    print(
        "build_efficiency_provenance: "
        f"roles={report['roles']} selected_paths={report['selected_paths']} "
        f"historical_rust_paths={report['historical_rust_paths']} "
        f"cargo_lock_bytes={report['cargo_lock_bytes']} "
        f"source_budget_bytes={report['source_budget_bytes']} "
        "structural_signature_only=true "
        f"baseline={REQUIRED_SOURCE_BUDGET['baseline']} "
        f"ceiling={REQUIRED_SOURCE_BUDGET['ceiling']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
