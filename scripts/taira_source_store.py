#!/usr/bin/env python3
"""Authenticate additional direct signed source packs in an existing warm builder.

Requires explicit admission of current custody from independently pinned source
captures, then its current receipt and an explicit signed successor.
Uses the maintained Cargo/source leases; never fetches, checks out, rewrites
Git controls, runs Cargo, removes objects or moves private runtime inputs.
Interrupted publication requires the exact durable intent SHA before resuming.
All metadata below target/.taira-source-store is public source evidence held in
owner-private directories. Fresh import remains a separate, exact contract.
"""

from __future__ import annotations

import base64
import contextlib
import ctypes
import hashlib
import os
from pathlib import Path
import re
import stat
import sys
import uuid

import taira_source_capture as source
import taira_release as release
from release_artifact_contract import canonical_json_bytes, load_json_object

SCHEMA = "iroha.taira.builder-source-store.v1"
INTENT_SCHEMA = "iroha.taira.builder-source-operation-intent.v1"
CUSTODY_INPUT_SCHEMA = "iroha.taira.builder-source-custody-input.v1"
MAX_CAPTURES = 32
MAX_RECORD_BYTES = source.MAX_MANIFEST_BYTES
_OPERATION = re.compile(r"[0-9a-f]{32}")
_PACK = re.compile(r"pack-[0-9a-f]{40}\.(?:pack|idx|rev)")


def _identity_wire(value, length):
    return (isinstance(value, list) and len(value) == length
            and all(isinstance(part, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", part)
                    and int(part) <= 2**64 - 1 for part in value))


def _stable_directory(info):
    return [str(getattr(info, key)) for key in ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid")]


def _file_identity(info):
    # Decimal strings preserve inode/ns values through public JSON consumers.
    return [str(value) for value in source._identity(info)]


def _digest_fd(fd, maximum):
    before = os.fstat(fd)
    source._need(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                 and before.st_nlink == 1 and not before.st_mode & 0o022
                 and 0 <= before.st_size <= maximum, "source-store file custody/bound differs")
    digest = hashlib.sha256()
    offset = 0
    while offset < before.st_size:
        block = os.pread(fd, min(source.CHUNK, before.st_size - offset), offset)
        source._need(block, "source-store file became short")
        digest.update(block)
        offset += len(block)
    source._need(source._identity(before) == source._identity(os.fstat(fd)), "source-store file changed while reading")
    return digest.hexdigest()


@contextlib.contextmanager
def _retained_file(path, maximum, expected=None, *, renamed=False, publication=None):
    path = source._absolute(path)
    fd, named, parent = source._open_anchored_regular(path.parent, path.name)
    try:
        before = os.fstat(fd)
        source._need(source._identity(before) == source._identity(named), "source-store path/FD differs")
        digest = _digest_fd(fd, maximum)
        reference = {"path": str(path), "identity": _file_identity(before), "sha256": digest}
        if expected is not None:
            source._need(isinstance(expected, dict) and set(expected) == {"path", "identity", "sha256"}
                         and isinstance(expected["path"], str) and _identity_wire(expected["identity"], 9)
                         and isinstance(expected["sha256"], str) and source._SHA.fullmatch(expected["sha256"]),
                         "source-store reference shape differs")
            # An intent predates our controlled rename, which changes only ctime.
            actual_identity = reference["identity"][:-1] if renamed else reference["identity"]
            expected_identity = expected["identity"][:-1] if renamed else expected["identity"]
            source._need(actual_identity == expected_identity and digest == expected["sha256"]
                         and (renamed or reference["path"] == expected["path"]), "source-store retained file differs")
        yield fd, reference
        final_path = publication[0] if publication else path
        # A controlled rename legitimately changes ctime. Retain byte custody
        # through the original descriptor as well as the inode/metadata checks;
        # a same-size write can otherwise restore mtime across publication.
        source._need(_digest_fd(fd, maximum) == digest, "source-store retained file bytes changed")
        final = os.fstat(fd)
        wanted = source._identity(before)[:-1] if publication else source._identity(before)
        actual = source._identity(final)[:-1] if publication else source._identity(final)
        source._need(wanted == actual, "source-store retained inode changed")
        final_parent, _, _ = source._open_absolute_directory(final_path.parent, "source-store final parent")
        try:
            source._need(source._identity(final) == source._identity(os.stat(final_path.name, dir_fd=final_parent, follow_symlinks=False)),
                     "source-store file/path changed during admission")
        finally:
            os.close(final_parent)
        fresh, _, fresh_parent = source._open_anchored_regular(final_path.parent, final_path.name)
        try:
            source._need(source._identity(final) == source._identity(os.fstat(fresh)), "source-store ancestor/path changed")
        finally:
            os.close(fresh)
            os.close(fresh_parent)
    finally:
        os.close(fd)
        os.close(parent)


def _reference(path, maximum=MAX_RECORD_BYTES):
    with _retained_file(path, maximum) as (_, reference):
        return reference


def _names(directory, maximum):
    names = set()
    with os.scandir(directory) as entries:
        for entry in entries:
            names.add(entry.name)
            source._need(len(names) <= maximum, "source-store directory census exceeds its bound")
    return names


def _read_record(path, expected_sha):
    source._need(isinstance(expected_sha, str) and source._SHA.fullmatch(expected_sha), "expected source-store SHA must be full lowercase SHA256")
    with _retained_file(path, MAX_RECORD_BYTES) as (fd, reference):
        source._need(reference["sha256"] == expected_sha, "source-store record SHA differs")
        payload = os.pread(fd, os.fstat(fd).st_size + 1, 0)
        value = load_json_object(payload, "builder source store")
        source._need(canonical_json_bytes(value) == payload, "source-store record is not canonical")
        return value, reference


def _create(path, payload, mode=0o400):
    """Create a complete immutable record; no empty canonical journal is published."""
    source._need(len(payload) <= MAX_RECORD_BYTES, "source-store record exceeds its byte bound")
    parent, _, _ = source._open_absolute_directory(path.parent, "source-store record parent")
    stage = path.parent / (".record-" + uuid.uuid4().hex)
    try:
        fd = os.open(stage.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=parent)
        try:
            position = 0
            while position < len(payload):
                written = os.write(fd, payload[position:position + source.CHUNK])
                source._need(written > 0, "source-store record write stopped")
                position += written
            os.fchmod(fd, mode)
            os.fsync(fd)
            source._need(source._identity(os.fstat(fd)) == source._identity(os.stat(stage.name, dir_fd=parent, follow_symlinks=False)),
                         "source-store created record path changed")
            before = os.fstat(fd)
            parent_identity = _stable_directory(os.fstat(parent))
            _rename_noreplace(stage, path, lambda: None, parent_identity, parent_identity)
            source._need(source._identity(os.fstat(fd))[:-1] == source._identity(before)[:-1]
                         and source._identity(os.fstat(fd)) == source._identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False)),
                         "source-store created record inode changed during publication")
        finally:
            os.close(fd)
        os.fsync(parent)
    finally:
        os.close(parent)
    return _reference(path)


def _mkdir(path):
    parent, _, _ = source._open_absolute_directory(path.parent, "source-store directory parent")
    try:
        os.mkdir(path.name, 0o700, dir_fd=parent)
        os.fsync(parent)
    finally:
        os.close(parent)
    source._directory(path, mode=0o700)


def _receipt_anchor(info):
    return [str(getattr(info, key)) for key in ("st_dev", "st_ino", "st_uid", "st_gid", "st_nlink")]


def _allocate_receipt(operation):
    path = operation / ".receipt-stage"
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    try:
        before = os.fstat(fd)
        source._need(source._identity(before) == source._identity(path.lstat()), "reserved receipt inode changed")
        os.fsync(fd)
        parent, _, _ = source._open_absolute_directory(operation, "reserved receipt parent")
        try:
            os.fsync(parent)
        finally:
            os.close(parent)
        return {"path": str(path), "identity": _receipt_anchor(before)}
    finally:
        os.close(fd)


def _finish_receipt(operation, allocation, value, guard):
    payload = canonical_json_bytes(value)
    source._need(len(payload) <= MAX_RECORD_BYTES and isinstance(allocation, dict) and set(allocation) == {"path", "identity"}
                 and _identity_wire(allocation["identity"], 5)
                 and allocation["path"] == str(operation / ".receipt-stage"), "reserved receipt binding differs")
    stage, terminal = Path(allocation["path"]), operation / "receipt.json"
    source._need(os.path.lexists(stage) != os.path.lexists(terminal), "receipt publication arrangement is ambiguous")
    path = stage if os.path.lexists(stage) else terminal
    fd, _, parent = source._open_anchored_regular(path.parent, path.name)
    try:
        info = os.fstat(fd)
        source._need(_receipt_anchor(info) == allocation["identity"] and stat.S_ISREG(info.st_mode)
                     and stat.S_IMODE(info.st_mode) in {0o600, 0o400} and info.st_size <= len(payload), "reserved receipt inode differs")
        existing = os.pread(fd, info.st_size + 1, 0)
        source._need(existing == payload[:len(existing)], "reserved receipt contains foreign or ambiguous bytes")
        if path == terminal:
            source._need(existing == payload and stat.S_IMODE(info.st_mode) == 0o400, "published receipt bytes/mode differ")
            return _reference(terminal)
        if stat.S_IMODE(info.st_mode) == 0o600:
            writable = os.open(stage.name, os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent)
            try:
                source._need(source._identity(os.fstat(writable)) == source._identity(info), "reserved receipt changed before write")
                offset = len(existing)
                while offset < len(payload):
                    written = os.pwrite(writable, payload[offset:offset + source.CHUNK], offset)
                    source._need(written > 0, "reserved receipt write stopped")
                    offset += written
                os.fchmod(writable, 0o400)
                os.fsync(writable)
                source._need(_digest_fd(writable, MAX_RECORD_BYTES) == hashlib.sha256(payload).hexdigest(), "reserved receipt changed while writing")
            finally:
                os.close(writable)
        else:
            source._need(existing == payload, "frozen reserved receipt is incomplete")
        guard()
        parent_identity = _stable_directory(os.fstat(parent))
        source._need(_receipt_anchor(stage.lstat()) == allocation["identity"], "reserved receipt path changed before publication")
        _rename_noreplace(stage, terminal, guard, parent_identity, parent_identity)
        source._need(_receipt_anchor(terminal.lstat()) == allocation["identity"], "receipt native publication inode differs")
        return _reference(terminal)
    finally:
        os.close(fd)
        os.close(parent)


def _raw_parent(root, commit):
    """Shallow Git traversal hides parents; the signed raw header never does."""
    payload = source._git(root, "cat-file", "commit", commit, maximum=MAX_RECORD_BYTES)
    header, separator, _ = payload.partition(b"\n\n")
    source._need(separator, "signed successor commit has no native header separator")
    parents = [line[7:].decode("ascii") for line in header.split(b"\n") if line.startswith(b"parent ")]
    source._need(len(parents) == 1 and source._OID.fullmatch(parents[0]), "signed successor must have exactly one native parent")
    return parents[0]


def _controller(root, manifest):
    # Use the controller's exact table and snapshot primitive. This byte gate is
    # not its process module-origin gate; prepare-client still executes that gate.
    entries = release.commit_entries(root, manifest["commit"])
    required = set(release.BUILD_SOURCES)
    selected = []
    found = set()
    for row in entries.split(b"\0"):
        if not row:
            continue
        metadata, path = row.split(b"\t", 1)
        name = os.fsdecode(path)
        if name in required:
            source._need(metadata.split()[0] in (b"100644", b"100755"), "selected build controller is not regular")
            found.add(name)
            if name in release.BOOTSTRAP_SOURCES:
                selected.append(row)
    source._need(found == required, "selected source lacks a required build controller")
    # source_snapshot hashes raw bytes including trailing newlines against the
    # selected Git OID; never use taira_release.git's stripped blob output.
    release.source_snapshot(root, b"\0".join(selected) + b"\0")


def _union(manifests):
    source._need(1 <= len(manifests) <= MAX_CAPTURES, "source-store capture count exceeds its bound")
    rows = {}
    for manifest in manifests:
        for row in manifest["objects"]:
            source._need(row["object"] not in rows or rows[row["object"]] == row, "signed source object collision")
            rows[row["object"]] = row
    source._need(len(rows) <= source.MAX_OBJECTS and sum(row["size"] for row in rows.values()) <= source.MAX_TOTAL_SOURCE_BYTES,
                 "source-store exact object union exceeds its bound")
    source._need(sum(manifest["pack"]["size"] for manifest in manifests) <= source.MAX_PACK_BYTES,
                 "source-store physical pack aggregate exceeds its bound")
    return [rows[oid] for oid in sorted(rows)]


def _verify_union(root, manifests):
    expected = b"".join(f"{row['object']} {row['type']} {row['size']}\n".encode() for row in _union(manifests))
    actual = source._git(root, "cat-file", "--batch-all-objects", "--batch-check=%(objectname) %(objecttype) %(objectsize)")
    source._need(actual == expected, "builder object database differs from the exact authenticated union")
    for manifest in manifests:
        rows, entries, size = source._inventory(root, manifest["commit"], manifest["tree"])
        source._need(rows == manifest["objects"] and entries == manifest["entries"] and size == manifest["source_bytes"],
                     "builder selected signed tree/object bytes differ")
        source._verify_signature(root, manifest["commit"], manifest["signer_fingerprint"], base64.b64decode(manifest["public_key_base64"], validate=True))
        source._verify_source_links(root, manifest)


def _pack_files(root, manifest):
    directory = root / ".git/objects/pack"
    names = sorted(directory.iterdir())
    source._need(len(names) == 3 and {path.suffix for path in names} == {".pack", ".idx", ".rev"}
                 and len({path.stem for path in names}) == 1 and all(_PACK.fullmatch(path.name) for path in names),
                 "new signed source pack is not one complete native trio")
    refs = [_reference(path, source.MAX_PACK_BYTES) for path in names]
    _verify_pack_refs(root, manifest, refs)
    return refs


def _existing_pack_files(root, manifests):
    """Admit existing packs without manufacturing earlier publication ownership."""
    directory = root / ".git/objects/pack"
    names = _names(directory, 3 * MAX_CAPTURES)
    source._need(len(names) == 3 * len(manifests) and all(_PACK.fullmatch(name) for name in names),
                 "initial custody requires exactly the selected complete pack trios")
    packs = {}
    for name in sorted(names):
        if name.endswith(".pack"):
            reference = _reference(directory / name, source.MAX_PACK_BYTES)
            source._need(reference["sha256"] not in packs, "initial custody has duplicate physical packs")
            packs[reference["sha256"]] = Path(name).stem
    result = []
    for manifest in manifests:
        source._need(manifest["pack"]["sha256"] in packs, "initial custody selected pack is absent or foreign")
        stem = packs.pop(manifest["pack"]["sha256"])
        refs = [_reference(directory / (stem + suffix), source.MAX_PACK_BYTES) for suffix in (".idx", ".pack", ".rev")]
        _verify_pack_refs(root, manifest, refs)
        result.append(refs)
    source._need(not packs, "initial custody has an unselected native pack")
    return result


def _native_capture(root, manifest):
    """Join actual frozen public source, checkpoint and original warm target."""
    target = root / "target/taira-macos-client"
    key = hashlib.sha256(os.fsencode(target)).hexdigest()[:24]
    parent = target / "taira-release-sources" / key
    captured = parent / "source"
    state = release.read_record(parent / "source-state.json")
    source._need(state == {"commit": manifest["commit"]}, "actual captured source frontier differs")
    state_reference = _reference(parent / "source-state.json")
    entries = release.commit_entries(root, manifest["commit"])
    rows = release.frozen_snapshot(captured, entries, target)
    paths = {Path(row["path"]) for row in rows} | {Path("target"), Path(".")}
    for path in list(paths):
        paths.update(path.parents)
    snapshots = [{"path": str(path), "identity": _file_identity((captured / path).lstat())} for path in sorted(paths)]
    binding = {"target": str(target), "target_identity": _stable_directory(target.lstat()),
               "source_parent": str(parent), "source_parent_identity": _stable_directory(parent.lstat()),
               "source": str(captured), "state": state_reference, "snapshot_sha256": hashlib.sha256(canonical_json_bytes(rows)).hexdigest(),
               "files": snapshots, "frontier": {"commit": manifest["commit"], "tree": manifest["tree"], "signer_fingerprint": manifest["signer_fingerprint"]}}
    _check_native_capture(binding)
    return binding


def _check_native_capture(binding):
    for path, identity in ((binding["target"], binding["target_identity"]), (binding["source_parent"], binding["source_parent_identity"])):
        source._need(_stable_directory(Path(path).lstat()) == identity, "native warm target/source lane changed")
    source._need(_file_identity(Path(binding["state"]["path"]).lstat()) == binding["state"]["identity"], "native capture checkpoint changed")
    captured = Path(binding["source"])
    for row in binding["files"]:
        source._need(_file_identity((captured / row["path"]).lstat()) == row["identity"], "native frozen capture inode changed")
    source._need(os.readlink(captured / "target") == binding["target"], "native frozen capture target binding changed")


def _initialization_binding(root, value):
    source._need(isinstance(value, dict) and set(value) == {"operation_id", "capture_count", "native_capture"}
                 and isinstance(value["operation_id"], str) and _OPERATION.fullmatch(value["operation_id"])
                 and type(value["capture_count"]) is int and 1 <= value["capture_count"] <= MAX_CAPTURES,
                 "initialization lineage shape differs")
    native = value["native_capture"]
    target = root / "target/taira-macos-client"
    parent = target / "taira-release-sources" / hashlib.sha256(os.fsencode(target)).hexdigest()[:24]
    source._need(isinstance(native, dict) and set(native) == {"target", "target_identity", "source_parent", "source_parent_identity", "source", "state", "snapshot_sha256", "files", "frontier"}
                 and native["target"] == str(target) and native["source_parent"] == str(parent) and native["source"] == str(parent / "source"),
                 "initialization native lane binding differs")
    # Source refresh is authorized by the maintained builder, so the initial
    # capture record remains historical; physical lane identities stay fixed.
    source._need(_stable_directory(target.lstat()) == native["target_identity"]
                 and _stable_directory(parent.lstat()) == native["source_parent_identity"], "initialized native warm lane inode changed")


def _verify_pack_refs(root, manifest, references):
    source._need(isinstance(references, list) and len(references) == 3, "stored pack reference count differs")
    paths = [Path(row["path"]) for row in references]
    source._need(all(path.parent == root / ".git/objects/pack" and _PACK.fullmatch(path.name) for path in paths)
                 and {path.suffix for path in paths} == {".pack", ".idx", ".rev"}
                 and len({path.stem for path in paths}) == 1, "stored pack reference paths differ")
    for path, expected in zip(paths, references):
        with _retained_file(path, source.MAX_PACK_BYTES, expected) as (fd, reference):
            source._need(stat.S_IMODE(os.fstat(fd).st_mode) == 0o444, "stored pack mode differs")
            if path.suffix == ".pack":
                source._need(reference["sha256"] == manifest["pack"]["sha256"]
                             and os.fstat(fd).st_size == manifest["pack"]["size"], "stored source pack differs")
                source._validate_pack(fd, manifest)
            elif path.suffix == ".idx":
                source._git(root, "verify-pack", str(path))


def _baseline(root, manifest, pack_names):
    """Exact original checkout/controls/index, with only private target excluded."""
    expected = {".git/" + name for name in source._git_metadata(manifest["commit"])} | {".git/index"} | set(pack_names)
    directories = {".", ".git", ".git/objects", ".git/objects/info", ".git/objects/pack", ".git/refs/tags"}
    objects = {row["object"]: row for row in manifest["objects"]}
    paths = {entry["path"]: entry["mode"] for entry in manifest["entries"]}
    for entry in manifest["entries"]:
        path = root / entry["path"]
        if entry["mode"] == "160000":
            source._directory(path, mode=0o755)
            source._need(not any(path.iterdir()), "original gitlink is no longer empty")
            directories.add(entry["path"])
        elif entry["mode"] == "120000":
            info = path.lstat()
            payload = os.fsencode(os.readlink(path))
            source._need(stat.S_ISLNK(info.st_mode) and info.st_uid == os.geteuid() and info.st_nlink == 1,
                         "original symlink custody differs")
            source._link_target(entry["path"], payload, paths)
            source._need(not os.path.lexists(path.parent / os.readlink(path))
                         and hashlib.sha256(payload).hexdigest() == objects[entry["object"]]["sha256"], "original symlink bytes/referent differ")
            expected.add(entry["path"])
        else:
            source._verify_source_file(path, objects[entry["object"]], 0o755 if entry["mode"] == "100755" else 0o644)
            expected.add(entry["path"])
    for path in list(expected | directories):
        directories.update(str(parent) for parent in Path(path).parents)
    actual_files, actual_dirs = set(), {"."}
    for current, children, files in os.walk(root, followlinks=False):
        if Path(current) == root:
            source._directory(root / "target", mode=0o700)
            children.remove("target")
        for name in children + files:
            path = Path(current) / name
            relative = str(path.relative_to(root))
            info = path.lstat()
            source._need(info.st_uid == os.geteuid() and (stat.S_ISLNK(info.st_mode) or not info.st_mode & 0o022),
                         "original source-store custody differs")
            if stat.S_ISDIR(info.st_mode):
                source._need(stat.S_IMODE(info.st_mode) == 0o755, "original source directory mode differs")
                actual_dirs.add(relative)
            else:
                source._need(stat.S_ISLNK(info.st_mode) or (stat.S_ISREG(info.st_mode) and info.st_nlink == 1), "original source file kind differs")
                actual_files.add(relative)
            source._need(len(actual_files) <= source.MAX_FILES + 3 * MAX_CAPTURES + 32
                         and len(actual_dirs) <= source.MAX_FILES * 8 + 32, "original source layout exceeds its bound")
    source._need(actual_files == expected and actual_dirs == directories, "original checkout has missing or extra paths")
    for name, payload in source._git_metadata(manifest["commit"]).items():
        pin, actual = source.stable_read_path(root / ".git" / name, max_size=4096)
        source._need(actual == payload and pin.mode == 0o644, "original Git control bytes/mode changed")
    source._need(stat.S_IMODE((root / ".git/index").lstat().st_mode) in {0o600, 0o644}, "original native Git index mode changed")
    wanted_index = b"".join(f"{row['mode']} {row['object']} 0\t{row['path']}\0".encode() for row in manifest["entries"])
    source._need(source._git(root, "ls-files", "--stage", "-z") == wanted_index, "original native Git index changed")
    # Pack directory timestamps change only when we append. Root itself changes
    # only before the warm lane exists; target is independently retained below.
    rows = []
    for name in sorted((actual_dirs | actual_files) - set(pack_names)):
        info = (root / name).lstat()
        identity = _stable_directory(info) if stat.S_ISDIR(info.st_mode) else _file_identity(info)
        rows.append({"path": name, "directory": stat.S_ISDIR(info.st_mode), "identity": identity})
    return rows


def _check_baseline(root, rows):
    source._need(isinstance(rows, list) and len(rows) <= source.MAX_FILES * 9 + 64, "source-store baseline exceeds its bound")
    for row in rows:
        source._need(isinstance(row, dict) and set(row) == {"path", "directory", "identity"}
                     and type(row["directory"]) is bool, "source-store baseline row shape differs")
        source._need(row["path"] == "." or source.canonical_relative_path(row["path"]), "source-store baseline path differs")
        info = (root / row["path"]).lstat()
        actual = _stable_directory(info) if row["directory"] else _file_identity(info)
        source._need(actual == row["identity"], "original checkout/control/index inode changed")


def _layout_census(root, baseline, allowed_packs):
    expected = {row["path"] for row in baseline}
    stack, actual = [root], {"."}
    while stack:
        current = stack.pop()
        with os.scandir(current) as entries:
            for entry in entries:
                path = current / entry.name
                name = str(path.relative_to(root))
                if name == "target":
                    source._directory(path, mode=0o700)
                    continue
                source._need(name in expected or name in allowed_packs, "original checkout has an extra or foreign path")
                actual.add(name)
                info = entry.stat(follow_symlinks=False)
                if stat.S_ISDIR(info.st_mode):
                    source._need(name in expected, "extra builder object directory")
                    stack.append(path)
    source._need(expected <= actual, "original checkout has a missing path")


@contextlib.contextmanager
def _leases(root):
    """Hold the actual admitted build/source leases, with no parallel lock."""
    with contextlib.ExitStack() as stack:
        held = []
        anchored = []
        for directory in (root, root / "target", root / ".git", root / ".git/objects", root / ".git/objects/pack"):
            fd, _, identity = source._open_absolute_directory(directory, "original builder directory")
            stack.callback(os.close, fd)
            anchored.append((directory, fd, _stable_directory(identity)))
        client = root / "target/taira-macos-client"
        source._need(client.is_dir(), "append requires an already admitted native client build lane")
        # All maintained native builders share this fixed acquisition order. A
        # nonblocking refusal avoids waiting while retaining another lane lease.
        for target in (root / "target", client, root / "target/taira-macos-runtime"):
            marker = target / ".taira-build-lane/role.json"
            if target != client and not os.path.lexists(marker):
                continue
            source._directory(target, mode=0o700)
            source._need(marker.is_file(), "native builder lane has no admitted mode record")
            key = hashlib.sha256(os.fsencode(target)).hexdigest()[:24]
            source_parent = target / "taira-release-sources" / key
            source._directory(source_parent, mode=0o700)
            for lock in (target / ".taira-build-lane/session.lock", source_parent / "session.lock"):
                source._need(os.path.lexists(lock), "append requires an existing native build/source lease inode")
            stack.enter_context(release.retained_native_target(target))
            cargo_fd = stack.enter_context(release.cargo_lane(root, target, "release"))
            _, source_fd = stack.enter_context(release.source_lane(root, target))
            for path, fd in ((target / ".taira-build-lane/session.lock", cargo_fd), (source_parent / "session.lock", source_fd)):
                held.append((path, fd, _file_identity(os.fstat(fd))))
            held.append((marker, stack.enter_context(_retained_file(marker, 16 * 1024))[0], _file_identity(marker.lstat())))
        def guard():
            for path, fd, identity in anchored:
                source._need(_stable_directory(os.fstat(fd)) == identity == _stable_directory(path.lstat()), "original builder ancestor changed")
            for path, fd, identity in held:
                source._need(_file_identity(os.fstat(fd)) == identity == _file_identity(path.lstat()), "native build/source lease path changed")
            source._directory(root, mode=0o755)
            source._directory(root / "target", mode=0o700)
        guard()
        yield guard
        guard()


def _load_receipt(root, path, sha):
    value, reference = _read_record(path, sha)
    source._need(set(value) == {"schema", "kind", "initialization", "operation_id", "source_root", "root_identity", "baseline", "admissions", "operations", "prior_receipt", "intent_sha256"}
                 and value["schema"] == SCHEMA and value["source_root"] == str(root)
                 and value["kind"] in {"initialization", "append"}
                 and value["root_identity"] == _stable_directory(root.lstat()), "builder source receipt identity differs")
    source._need(isinstance(value["operation_id"], str) and _OPERATION.fullmatch(value["operation_id"])
                 and isinstance(value["operations"], list) and value["operations"] and value["operations"][-1] == value["operation_id"]
                 and 1 <= len(value["operations"]) <= MAX_CAPTURES and len(set(value["operations"])) == len(value["operations"])
                 and all(isinstance(operation, str) and _OPERATION.fullmatch(operation) for operation in value["operations"]), "builder receipt operation chain differs")
    expected_path = root / "target/.taira-source-store" / value["operation_id"] / "receipt.json"
    _initialization_binding(root, value["initialization"])
    source._need(Path(path) == expected_path and value["initialization"]["operation_id"] == value["operations"][0]
                 and len(value["admissions"]) == value["initialization"]["capture_count"] + len(value["operations"]) - 1
                 and (value["kind"] == "initialization") == (len(value["operations"]) == 1), "builder receipt path/admission chain differs")
    manifests = []
    for admission in value["admissions"]:
        source._need(isinstance(admission, dict) and set(admission) == {"manifest", "pack_files"}, "builder admission shape differs")
        manifest_ref = admission["manifest"]
        manifest_path = Path(manifest_ref["path"])
        source._need(manifest_path.parent.parent == root / "target/.taira-source-store"
                     and manifest_path.parent.name in value["operations"]
                     and (re.fullmatch(r"capture-[0-9]{2}\.json", manifest_path.name) or manifest_path.name == "new-manifest.json"), "builder admitted manifest path differs")
        with _retained_file(manifest_path, source.MAX_MANIFEST_BYTES, manifest_ref) as (fd, _):
            raw = load_json_object(os.pread(fd, os.fstat(fd).st_size + 1, 0), "stored signed capture")
            manifest, _, _ = source._manifest(manifest_path, raw["commit"], raw["tree"], raw["signer_fingerprint"])
        _verify_pack_refs(root, manifest, admission["pack_files"])
        manifests.append(manifest)
    _union(manifests)
    intent, _ = _read_record(expected_path.parent / "intent.json", value["intent_sha256"])
    source._need(intent.get("schema") == INTENT_SCHEMA and intent.get("kind") == value["kind"]
                 and intent["initialization"] == value["initialization"]
                 and _receipt_anchor(expected_path.lstat()) == intent["receipt_allocation"]["identity"],
                 "terminal receipt differs from its independently retained reserved inode")
    return value, reference, manifests


def _capacity(root, manifest, original, baseline, previous=None):
    # Stage contains one direct pack + both native indexes and Git controls.
    # Intent and terminal receipt overlap; each also retains the baseline and
    # admitted references. No successor working tree is materialized here.
    # Use the actual admitted baseline, including all unique parent paths. A
    # file-count/path-length estimate can undercount a deep signed tree. The
    # remainder bounds full inode refs and request/control fields; existing
    # admissions are represented exactly, not expanded into duplicate graphs.
    ledger = len(canonical_json_bytes(baseline)) + len(canonical_json_bytes((previous or {}).get("admissions", [])))
    ledger += 32 * 1024 + 32 * len(os.fsencode(root))
    source._need(ledger <= MAX_RECORD_BYTES, "source-store ledger geometry exceeds its bound")
    payload = manifest["pack"]["size"] + 1124 + 40 * len(manifest["objects"])
    payload += 2 * ledger + len(canonical_json_bytes(original)) + len(canonical_json_bytes(manifest))
    allocation = source._source_capacity_allocation(root / "target", "authenticated warm builder append", payload, 24, 16)
    source._check_source_capacity([allocation, *source._scratch_capacity_allocations(source.MAX_PUBLIC_KEY_BYTES)])


def _rename_noreplace(source_path, destination, guard, source_parent_identity, destination_parent_identity):
    guard()
    source_parent, _, _ = source._open_absolute_directory(source_path.parent, "append stage parent")
    destination_parent, _, _ = source._open_absolute_directory(destination.parent, "append destination parent")
    try:
        source._need(_stable_directory(os.fstat(source_parent)) == source_parent_identity
                     and _stable_directory(os.fstat(destination_parent)) == destination_parent_identity,
                     "append retained publication parent changed")
        library = ctypes.CDLL(None, use_errno=True)
        name, flag = {"darwin": ("renameatx_np", 4), "linux": ("renameat2", 1)}.get(sys.platform, (None, None))
        source._need(name and hasattr(library, name), "append requires native no-replace rename")
        function = getattr(library, name)
        function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
        function.restype = ctypes.c_int
        source._need(function(source_parent, os.fsencode(source_path.name), destination_parent, os.fsencode(destination.name), flag) == 0,
                     "append native no-replace publication refused: " + os.strerror(ctypes.get_errno()))
        os.fsync(source_parent)
        os.fsync(destination_parent)
    finally:
        os.close(source_parent)
        os.close(destination_parent)
    guard()


def _fault(phase):
    """No production effect; test seam exercises interrupted durable boundaries."""


def custody_inputs(path, expected_sha256):
    """Decode only the explicit closed, independently pinned public input."""
    value, _ = _read_record(path, expected_sha256)
    source._need(set(value) == {"schema", "captures"} and value["schema"] == CUSTODY_INPUT_SCHEMA, "initial custody input schema differs")
    return value["captures"]


def initialize_store(source_root, captures, captured_commit, captured_tree, captured_signer, operation_id, *, intent_sha256=None):
    """Admit current native custody without copying or publishing any Git object."""
    root = source._directory(source_root, mode=0o755)
    source._expected(captured_commit, captured_tree, captured_signer)
    source._need(isinstance(operation_id, str) and _OPERATION.fullmatch(operation_id), "initialization operation must be 32 lowercase hex digits")
    source._need(isinstance(captures, list) and 1 <= len(captures) <= MAX_CAPTURES, "initial custody capture count differs")
    manifests = []
    for row in captures:
        source._need(isinstance(row, dict) and set(row) == {"manifest", "sha256", "commit", "tree", "signer_fingerprint"}, "initial custody capture shape differs")
        manifest, _, reference = source._manifest(row["manifest"], row["commit"], row["tree"], row["signer_fingerprint"])
        source._need(reference.sha256 == row["sha256"], "initial custody independently pinned manifest differs")
        manifests.append(manifest)
    _union(manifests)
    source._need(len({manifest["commit"] for manifest in manifests}) == len(manifests)
                 and (manifests[-1]["commit"], manifests[-1]["tree"], manifests[-1]["signer_fingerprint"]) == (captured_commit, captured_tree, captured_signer),
                 "initial custody selected captured frontier differs")
    request = {"captures": captures, "captured_commit": captured_commit, "captured_tree": captured_tree,
               "captured_signer": captured_signer, "operation_id": operation_id}
    with _leases(root) as lease_guard:
        store, operation = root / "target/.taira-source-store", root / "target/.taira-source-store" / operation_id
        if os.path.lexists(store):
            source._directory(store, mode=0o700)
            source._need(_names(store, MAX_CAPTURES) <= {operation_id}, "initial custody already has another operation")
        pack_sets = _existing_pack_files(root, manifests)
        pack_names = [str(Path(row["path"]).relative_to(root)) for refs in pack_sets for row in refs]
        baseline = _baseline(root, manifests[0], pack_names)
        for previous, successor in zip(manifests, manifests[1:]):
            source._need(_raw_parent(root, successor["commit"]) == previous["commit"], "initial custody signed native parent chain differs")
        _verify_union(root, manifests)
        for manifest in manifests:
            _controller(root, manifest)
        native = _native_capture(root, manifests[-1])
        if os.path.lexists(operation):
            source._directory(operation, mode=0o700)
            source._need(intent_sha256 is not None, "existing initialization requires its independently pinned durable intent SHA")
            intent, intent_ref = _read_record(operation / "intent.json", intent_sha256)
            source._need(set(intent) == {"schema", "kind", "request", "root_identity", "baseline", "admissions", "operations", "prior_receipt", "initialization", "receipt_allocation", "store_directory_identity", "operation_directory_identity"}
                         and intent["schema"] == INTENT_SCHEMA and intent["kind"] == "initialization" and intent["request"] == request
                         and intent["root_identity"] == _stable_directory(root.lstat()) and intent["baseline"] == baseline
                         and intent["operations"] == [operation_id] and intent["prior_receipt"] is None
                         and intent["initialization"] == {"operation_id": operation_id, "capture_count": len(manifests), "native_capture": native},
                         "initial custody durable intent differs")
            source._need(len(intent["admissions"]) == len(manifests), "initial custody intent admission count differs")
            for index, (manifest, refs, admission) in enumerate(zip(manifests, pack_sets, intent["admissions"])):
                path = operation / f"capture-{index:02}.json"
                with _retained_file(path, MAX_RECORD_BYTES, admission["manifest"]):
                    source._need(admission["manifest"]["sha256"] == hashlib.sha256(canonical_json_bytes(manifest)).hexdigest()
                                 and admission["pack_files"] == refs, "initial custody intent manifest/native pack differs")
        else:
            source._need(intent_sha256 is None, "fresh initialization has no previous durable intent")
            initialization = {"operation_id": operation_id, "capture_count": len(manifests), "native_capture": native}
            bound = len(canonical_json_bytes(baseline)) + len(canonical_json_bytes(initialization)) + len(canonical_json_bytes(pack_sets))
            bound += len(canonical_json_bytes(request)) + 32 * 2048 + 32 * len(os.fsencode(root))
            source._need(bound <= MAX_RECORD_BYTES, "initial custody record geometry exceeds its bound")
            allocation = source._source_capacity_allocation(root / "target", "current native source custody records",
                                                            2 * bound + sum(len(canonical_json_bytes(manifest)) for manifest in manifests), MAX_CAPTURES + 8, 2)
            source._check_source_capacity([allocation, *source._scratch_capacity_allocations(source.MAX_PUBLIC_KEY_BYTES)])
            lease_guard()
            if not os.path.lexists(store):
                _mkdir(store)
            _mkdir(operation)
            admissions = [{"manifest": _create(operation / f"capture-{index:02}.json", canonical_json_bytes(manifest)), "pack_files": refs}
                          for index, (manifest, refs) in enumerate(zip(manifests, pack_sets))]
            intent = {"schema": INTENT_SCHEMA, "kind": "initialization", "request": request, "root_identity": _stable_directory(root.lstat()),
                      "baseline": baseline, "admissions": admissions, "operations": [operation_id], "prior_receipt": None,
                      "initialization": initialization, "receipt_allocation": _allocate_receipt(operation),
                      "store_directory_identity": _stable_directory(store.lstat()), "operation_directory_identity": _stable_directory(operation.lstat())}
            _check_baseline(root, baseline)
            _check_native_capture(native)
            lease_guard()
            intent_ref = _create(operation / "intent.json", canonical_json_bytes(intent))
            _fault("initialization_intent_durable")
        with _retained_file(operation / "intent.json", MAX_RECORD_BYTES, intent_ref):
            def guard():
                lease_guard()
                source._need(_stable_directory(store.lstat()) == intent["store_directory_identity"]
                             and _stable_directory(operation.lstat()) == intent["operation_directory_identity"], "initial custody ancestor changed")
                _check_baseline(root, baseline)
                _layout_census(root, baseline, set(pack_names))
                _check_native_capture(native)
                for refs in pack_sets:
                    for reference in refs:
                        source._need(_file_identity(Path(reference["path"]).lstat()) == reference["identity"], "initial native pack inode changed")
            guard()
            receipt = {"schema": SCHEMA, "kind": "initialization", "initialization": intent["initialization"], "operation_id": operation_id,
                       "source_root": str(root), "root_identity": intent["root_identity"], "baseline": baseline,
                       "admissions": intent["admissions"], "operations": [operation_id], "prior_receipt": None, "intent_sha256": intent_ref["sha256"]}
            receipt_ref = _finish_receipt(operation, intent["receipt_allocation"], receipt, guard)
            _fault("initialization_receipt_durable")
            guard()
            return {"receipt": receipt_ref, "intent": intent_ref, "commit": captured_commit, "tree": captured_tree,
                    "signature_verified": True, "object_union_verified": True, "activated": False}


def append_source(source_root, *, receipt, expected_receipt_sha256,
                  pack, manifest, manifest_sha256, commit, tree, signer, operation_id, intent_sha256=None):
    """Append one exact full direct pack, returning immutable public custody refs."""
    root = source._directory(source_root, mode=0o755)
    pack = source._absolute(pack)
    source._need(isinstance(operation_id, str) and _OPERATION.fullmatch(operation_id), "append operation must be 32 lowercase hex digits")
    source._expected(commit, tree, signer)
    new, key, new_pin = source._manifest(manifest, commit, tree, signer)
    source._need(new_pin.sha256 == manifest_sha256, "independently pinned source manifest differs")
    pack_pin = source.stable_hash_path(pack, max_size=source.MAX_PACK_BYTES)
    source._need(pack_pin.sha256 == new["pack"]["sha256"] and pack_pin.size == new["pack"]["size"], "new source pack differs")
    request = {"manifest_sha256": manifest_sha256, "commit": commit, "tree": tree, "signer": signer,
               "receipt": str(receipt), "expected_receipt_sha256": expected_receipt_sha256, "operation_id": operation_id}
    with _leases(root) as lease_guard:
        store = root / "target/.taira-source-store"
        operation = store / operation_id
        if os.path.lexists(store):
            source._directory(store, mode=0o700)
        prior, prior_ref, manifests = _load_receipt(root, receipt, expected_receipt_sha256)
        original = manifests[0]
        _union([*manifests, new])
        previous_commit = manifests[-1]["commit"]
        operations = prior["operations"]
        source._need(operation_id not in operations, "append operation already belongs to the prior receipt")
        names = _names(store, MAX_CAPTURES) if store.exists() else set()
        source._need(names <= set(operations) | {operation_id}, "another incomplete or foreign append operation exists")
        if operation.exists():
            source._directory(operation, mode=0o700)
            source._need(intent_sha256 is not None, "existing append requires its independently pinned durable intent SHA")
            intent, intent_ref = _read_record(operation / "intent.json", intent_sha256)
            source._need(set(intent) == {"schema", "kind", "initialization", "request", "root_identity", "baseline", "admissions", "operations", "prior_receipt", "staged_pack_files", "stage_directory_identity", "receipt_allocation", "store_directory_identity", "operation_directory_identity"}
                         and intent["schema"] == INTENT_SCHEMA and intent["kind"] == "append" and intent["request"] == request
                         and intent["initialization"] == prior["initialization"]
                         and intent["root_identity"] == _stable_directory(root.lstat())
                         and intent["store_directory_identity"] == _stable_directory(store.lstat())
                         and intent["operation_directory_identity"] == _stable_directory(operation.lstat()), "append intent identity differs")
            source._need(intent["operations"] == [*operations, operation_id] and intent["prior_receipt"] == prior_ref
                         and isinstance(intent["admissions"], list) and len(intent["admissions"]) == len(manifests) + 1
                         and isinstance(intent["staged_pack_files"], list) and len(intent["staged_pack_files"]) == 3,
                         "append intent admitted lineage differs")
            source._need(intent["admissions"][:-1] == prior["admissions"] and intent["baseline"] == prior["baseline"],
                         "append intent prior prefix differs")
            for admission, expected_manifest in zip(intent["admissions"], [*manifests, new]):
                source._need(isinstance(admission, dict) and set(admission) == {"manifest", "pack_files"}, "append intent admission shape differs")
                with _retained_file(Path(admission["manifest"]["path"]), MAX_RECORD_BYTES, admission["manifest"]):
                    source._need(admission["manifest"]["sha256"] == hashlib.sha256(canonical_json_bytes(expected_manifest)).hexdigest(),
                                 "append intent signed manifest differs")
            _check_baseline(root, intent["baseline"])
        else:
            source._need(intent_sha256 is None, "fresh append cannot invent a missing durable intent")
            pack_names = [str(Path(row["path"]).relative_to(root)) for admission in prior["admissions"] for row in admission["pack_files"]]
            baseline = _baseline(root, original, pack_names)
            source._need(baseline == prior["baseline"], "original source baseline changed")
            _capacity(root, new, original, baseline, prior)
            _verify_union(root, manifests)
            _controller(root, manifests[-1])
            lease_guard()
            _mkdir(operation)
            new_ref = _create(operation / "new-manifest.json", canonical_json_bytes(new))
            admissions = list(prior["admissions"])
            stage = operation / "objects"
            _mkdir(stage)
            source._import_object_database(stage, pack, pack_pin, new, key)
            source._verify_source_links(stage, new)
            source._need(_raw_parent(stage, commit) == previous_commit, "signed successor native parent differs from the admitted frontier")
            # The selected successor's required controller blobs must remain
            # byte-identical to the original live checkout, not normalized text.
            for path in release.BOOTSTRAP_SOURCES:
                selected = next((row for row in new["entries"] if row["path"] == path), None)
                live = next((row for row in original["entries"] if row["path"] == path), None)
                source._need(selected == live and selected is not None, "successor changes a selected build Bootstrap source")
            source._need(set(release.BUILD_SOURCES) <= {row["path"] for row in new["entries"]}, "successor lacks required build sources")
            staged = _pack_files(stage, new)
            for row in staged:
                source._need(not os.path.lexists(root / ".git/objects/pack" / Path(row["path"]).name), "successor pack basename already exists")
            intent = {"schema": INTENT_SCHEMA, "kind": "append", "initialization": prior["initialization"], "request": request, "root_identity": _stable_directory(root.lstat()),
                      "baseline": baseline, "admissions": [*admissions, {"manifest": new_ref, "pack_files": []}],
                      "operations": [*operations, operation_id], "prior_receipt": prior_ref, "staged_pack_files": staged,
                      "stage_directory_identity": _stable_directory((stage / ".git/objects/pack").lstat()),
                      "receipt_allocation": _allocate_receipt(operation),
                      "store_directory_identity": _stable_directory(store.lstat()),
                      "operation_directory_identity": _stable_directory(operation.lstat())}
            _check_baseline(root, baseline)
            lease_guard()
            intent_ref = _create(operation / "intent.json", canonical_json_bytes(intent))
            _fault("intent_durable")
        # Keep the immutable intent FD and ancestor chain through every effect.
        with _retained_file(operation / "intent.json", MAX_RECORD_BYTES, intent_ref):
            original_refs = [row for admission in intent["admissions"][:-1] for row in admission["pack_files"]]
            allowed_packs = {str(Path(row["path"]).relative_to(root)) for row in original_refs}
            allowed_packs.update(".git/objects/pack/" + Path(row["path"]).name for row in intent["staged_pack_files"])
            def guard():
                lease_guard()
                source._need(_stable_directory(store.lstat()) == intent["store_directory_identity"]
                             and _stable_directory(operation.lstat()) == intent["operation_directory_identity"], "append operation ancestor changed")
                _check_baseline(root, intent["baseline"])
                _layout_census(root, intent["baseline"], allowed_packs)
                for reference in original_refs:
                    source._need(_file_identity(Path(reference["path"]).lstat()) == reference["identity"], "original admitted pack inode changed")
            guard()
            terminal = operation / "receipt.json"
            if terminal.exists():
                receipt_ref = _reference(terminal)
                receipt, _, current = _load_receipt(root, terminal, receipt_ref["sha256"])
                source._need(receipt["intent_sha256"] == intent_ref["sha256"] and current == [*manifests, new]
                             and receipt["kind"] == "append" and receipt["initialization"] == prior["initialization"]
                             and receipt["operations"] == intent["operations"] and receipt["baseline"] == intent["baseline"]
                             and receipt["prior_receipt"] == intent["prior_receipt"]
                             and receipt["admissions"][:-1] == intent["admissions"][:-1]
                             and receipt["admissions"][-1]["manifest"] == intent["admissions"][-1]["manifest"],
                             "completed append receipt differs from the exact intent")
                for staged_ref, published_ref in zip(intent["staged_pack_files"], receipt["admissions"][-1]["pack_files"]):
                    source._need(Path(published_ref["path"]) == root / ".git/objects/pack" / Path(staged_ref["path"]).name
                                 and not os.path.lexists(staged_ref["path"]), "completed append arrangement differs")
                    with _retained_file(Path(published_ref["path"]), source.MAX_PACK_BYTES, staged_ref, renamed=True):
                        pass
                _verify_union(root, current)
                _controller(root, new)
                guard()
                return {"receipt": receipt_ref, "intent": intent_ref, "commit": commit, "tree": tree, "signature_verified": True, "activated": False}
            published = []
            for index, staged_ref in enumerate(intent["staged_pack_files"]):
                staged_path = Path(staged_ref["path"])
                source._need(staged_path.parent == operation / "objects/.git/objects/pack" and _PACK.fullmatch(staged_path.name), "intent stage path differs")
                destination = root / ".git/objects/pack" / staged_path.name
                staged_exists, published_exists = os.path.lexists(staged_path), os.path.lexists(destination)
                source._need(staged_exists != published_exists, "append stage/publication arrangement is ambiguous")
                if staged_exists:
                    publication = []
                    with _retained_file(staged_path, source.MAX_PACK_BYTES, staged_ref, publication=publication) as (fd, _):
                        guard()
                        destination_identity = next(row["identity"] for row in intent["baseline"] if row["path"] == ".git/objects/pack")
                        _rename_noreplace(staged_path, destination, guard, intent["stage_directory_identity"], destination_identity)
                        publication.append(destination)
                        source._need(_file_identity(os.fstat(fd))[:-1] == staged_ref["identity"][:-1], "controlled pack rename changed its inode/content metadata")
                else:
                    with _retained_file(destination, source.MAX_PACK_BYTES, staged_ref, renamed=True):
                        guard()
                published.append(_reference(destination, source.MAX_PACK_BYTES))
                _fault(f"pack_{index + 1}_durable")
            admissions = intent["admissions"]
            admissions[-1] = {"manifest": admissions[-1]["manifest"], "pack_files": published}
            # Reopen all immutable manifest refs and verify native union before
            # committing an immutable terminal receipt (lost ack is read-only).
            current = [*manifests, new]
            _verify_union(root, current)
            source._need(_raw_parent(root, commit) == previous_commit, "published successor native parent differs")
            _controller(root, new)
            guard()
            receipt = {"schema": SCHEMA, "kind": "append", "initialization": prior["initialization"], "operation_id": operation_id, "source_root": str(root),
                       "root_identity": intent["root_identity"], "baseline": intent["baseline"], "admissions": admissions,
                       "operations": intent["operations"], "prior_receipt": prior_ref, "intent_sha256": intent_ref["sha256"]}
            receipt_ref = _finish_receipt(operation, intent["receipt_allocation"], receipt, guard)
            _fault("receipt_durable")
            guard()
            return {"receipt": receipt_ref, "intent": intent_ref, "commit": commit, "tree": tree, "signature_verified": True, "activated": False}


def verify_store(source_root, receipt, expected_receipt_sha256, commit, tree, signer):
    """Read-only current union proof; never reinterpret the fresh-import receipt."""
    root = source._directory(source_root, mode=0o755)
    source._expected(commit, tree, signer)
    with _leases(root) as guard:
        value, reference, manifests = _load_receipt(root, receipt, expected_receipt_sha256)
        source._need((manifests[-1]["commit"], manifests[-1]["tree"], manifests[-1]["signer_fingerprint"]) == (commit, tree, signer),
                     "selected builder frontier differs from the independently pinned receipt")
        names = _names(root / "target/.taira-source-store", MAX_CAPTURES)
        source._need(names == set(value["operations"]), "builder has an incomplete or foreign append")
        _check_baseline(root, value["baseline"])
        packs = [str(Path(row["path"]).relative_to(root)) for admission in value["admissions"] for row in admission["pack_files"]]
        source._need(_baseline(root, manifests[0], packs) == value["baseline"], "original builder baseline differs")
        _verify_union(root, manifests)
        _controller(root, manifests[-1])
        _check_baseline(root, value["baseline"])
        guard()
        return {"receipt": reference, "commit": commit, "tree": tree, "signature_verified": True,
                "object_union_verified": True, "capture_count": len(manifests), "activated": False}
