#!/usr/bin/env python3
"""Archive and explicitly retire one closed, inactive public Taira source tree.

Python 3.11+, local canonical optimizations Git/GPG, and the existing pinned
MacStadium routes are required. The guest requires root, Linux/AArch64 and Git.
Only one explicitly pinned release85..93 source closure is admitted. Neither
runtime secrets, receipts, native history nor any deployment authority are
created, archived or removed. Archive is read-only on the guest; retire is an
explicit separate action. No environment variables are required; HOME survives.
See docs/source/taira_retained_source.md.
"""
from __future__ import annotations

import argparse
import base64
import contextlib
import fcntl
import hashlib
import importlib.util
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shlex
import stat
import struct
import subprocess
import sys
import types

import taira_retained_release as common
import taira_retry as retry
import taira_source_capture as source

sys.dont_write_bytecode = True
SCHEMA = "taira.retained-public-source.v1"
MODULES = (*common.MODULES, "taira_source_capture", "taira_retained_source")
MAX_RECORD = 32 * 1024**2
MAX_ENVELOPE = 64 * 1024**2
MAX_TOTAL = 4 * 1024**3
MAX_ENTRIES = 65536
BATCH = 256
RESERVE = 256 * 1024**2
CONTROLS = ("HEAD", "ORIG_HEAD", "config", "shallow", "refs/heads/optimizations",
            "logs/HEAD", "logs/refs/heads/optimizations")
need, canonical, decode, sha = common.need, common.canonical, common.decode, common.sha


def binary_plan(plan):
    return {**{key: value for key, value in plan.items()
               if key not in {"source", "source_closure", "git_controls"}},
            "schema": common.SCHEMA}


def validate_plan(plan):
    need(isinstance(plan, dict) and plan.get("schema") == SCHEMA, "exact source retirement schema required")
    common.validate_plan(binary_plan(plan))
    need(set(plan) == set(binary_plan(plan)) | {"source", "source_closure", "git_controls"}
         and len(plan["releases"]) == 1, "exactly one explicit public source required")
    selected = plan["source"]
    need(isinstance(selected, dict) and set(selected) == {"root", "commit", "tree", "signer", "pack"},
         "exact signed source identity required")
    need(all(isinstance(selected[key], str) and re.fullmatch(r"[0-9a-f]{40}", selected[key])
             for key in ("commit", "tree"))
         and selected["signer"] == plan["controller"]["signer"], "source commit/tree/signer differs")
    number = Path(plan["releases"][0]["inventory"]["path"]).parent.name.removeprefix("assembly")
    need(number in {str(value) for value in range(85, 94)}
         and selected["root"] == f"/opt/iroha/taira-source-release{number}-{selected['commit']}",
         "only explicitly pinned compatible inactive source85..93 is eligible")
    common.reference(plan["source_closure"])
    pack = selected["pack"]
    need(isinstance(pack, dict) and set(pack) == {"sha256", "size"}
         and isinstance(pack["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", pack["sha256"])
         and type(pack["size"]) is int and 0 < pack["size"] <= MAX_TOTAL,
         "exact transferred source pack identity required")
    controls = plan["git_controls"]
    need(isinstance(controls, list) and [row.get("name") for row in controls] == list(CONTROLS),
         "exact public Git control pins required")
    for row in controls:
        need(set(row) == {"name", "size", "sha256"} and type(row["size"]) is int
             and 0 < row["size"] <= 4096 and re.fullmatch(r"[0-9a-f]{64}", row["sha256"]),
             "invalid public Git control pin")
    need(len(canonical(plan)) <= common.MAX_RECORD, "plan exceeds bound")
    return plan


def read(path, digest=None, *, mode=0o400):
    with common.held(path, digest=digest, mode=mode, maximum=MAX_RECORD) as (fd, info, _):
        return os.pread(fd, info.st_size + 1, 0)


def write_new(path, raw):
    """Publish bounded source evidence with the common atomic custody contract."""
    need(len(raw) <= MAX_RECORD, "source record exceeds bound")
    # The source manifest may exceed the binary owner's 8MiB record limit.
    # Publish its bytes with the same prefix-resumable custody contract.
    common.private_directory(path.parent)
    temporary = path.with_name("." + path.name + ".pending")
    need(not os.path.lexists(path), "record already published")
    with common.anchored_directory(path.parent) as directory:
        flags = os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
        try:
            fd = os.open(temporary.name, os.O_RDWR | os.O_CREAT | os.O_EXCL | flags, 0o600, dir_fd=directory)
            os.fchmod(fd, 0o600)
        except FileExistsError:
            fd = os.open(temporary.name, os.O_RDONLY | flags, dir_fd=directory)
        try:
            before = os.fstat(fd)
            need(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid() and before.st_nlink == 1
                 and stat.S_IMODE(before.st_mode) in (0o600, 0o400) and before.st_size <= len(raw)
                 and common.identity(before) == common.identity(os.stat(temporary.name, dir_fd=directory, follow_symlinks=False)),
                 "unsafe pending source record")
            need(os.pread(fd, before.st_size + 1, 0) == raw[:before.st_size], "pending source record differs")
            if before.st_size < len(raw):
                need(stat.S_IMODE(before.st_mode) == 0o600, "incomplete source record was sealed")
                writable = os.open(temporary.name, os.O_RDWR | flags, dir_fd=directory)
                try:
                    need(common.identity(os.fstat(writable)) == common.identity(before), "pending source record replaced")
                except BaseException:
                    os.close(writable)
                    raise
                os.close(fd)
                fd = writable
                os.lseek(fd, before.st_size, os.SEEK_SET)
                common.write_all(fd, raw[before.st_size:])
            need(common.hash_fd(fd, len(raw)) == sha(raw), "source record changed")
            os.fsync(fd)
            os.fchmod(fd, 0o400)
            os.fsync(fd)
            common.rename_exclusive(temporary, path, fd, common.identity(os.fstat(directory))[:2])
        finally:
            os.close(fd)


def canonical_proof(root, plan, git):
    selected = plan["source"]
    git("verify-commit", selected["commit"])
    need(git("show", "--no-patch", "--format=%GF", selected["commit"]).decode().strip() == selected["signer"]
         and git("rev-parse", selected["commit"] + "^{tree}").decode().strip() == selected["tree"],
         "retained source signature or tree differs")
    objects, entries, source_bytes = source._inventory(Path(root), selected["commit"], selected["tree"])
    value = {"commit": selected["commit"], "tree": selected["tree"], "signer": selected["signer"],
             "pack": selected["pack"], "objects": objects, "entries": entries, "source_bytes": source_bytes}
    need(len(canonical(value)) <= MAX_RECORD, "signed source proof exceeds bound")
    return value


def authority(plan, deployment, proof):
    """Reuse current terminal admission; no terminal-custody receipt is invented."""
    common.authority_rows(binary_plan(plan), deployment)
    release = plan["releases"][0]
    inventory = common.pin_json(release["inventory"])
    receipt = common.pin_json(release["source_manifest"])
    revision, selected = inventory["revision"], plan["source"]
    number = Path(release["inventory"]["path"]).parent.name.removeprefix("assembly")
    need(plan["source_closure"] == {"path": str(Path(deployment["runtime_root"]) / f"continuation{number}/source-manifest.json"),
         "sha256": revision["source_manifest_sha256"]}
         and revision["source_manifest_path"] == plan["source_closure"]["path"], "public source closure namespace differs")
    closure = common.pin_json(plan["source_closure"])
    need(all(revision[key] == selected[key] == proof[key] for key in ("commit", "tree"))
         and revision["source_root"] == selected["root"]
         and proof["signer"] == selected["signer"] and proof["pack"] == selected["pack"]
         and receipt["sha256"] == selected["pack"]["sha256"] and receipt["size"] == selected["pack"]["size"],
         "source authority differs from signed proof")
    need(closure.get("schema") == "iroha.taira.public-reset.signed-source-closure.v1"
         and closure.get("branch") == revision["branch"] == "optimizations"
         and closure.get("head_commit_sha1") == selected["commit"] and closure.get("head_tree_sha1") == selected["tree"]
         and closure.get("closure_sha256") == revision["source_closure_sha256"]
         and closure.get("cargo_lock_sha256") == revision["cargo_lock_sha256"] and closure.get("untracked_files") == [],
         "native signed source closure differs")
    objects = {row["object"]: row for row in proof["objects"]}
    expected = []
    for row in proof["entries"]:
        mode = int(row["mode"], 8)
        item = objects.get(row["object"])
        expected.append({"path": row["path"], "mode": mode & 0o777 if mode in (0o100644, 0o100755) else mode,
                         "git_blob_sha1": row["object"], "size": item["size"] if item else len(row["object"]),
                         "sha256": item["sha256"] if item else sha(row["object"].encode())})
    # Native gitlink rows retain their indexed commit as the public payload.
    actual = closure["tracked_files"]
    need(len(actual) == len(expected), "native source entry census differs")
    for observed, row in zip(actual, expected):
        need(observed["path"] == row["path"] and observed["mode"] == row["mode"]
             and observed["git_blob_sha1"] == row["git_blob_sha1"], "native source entry identity differs")
        need(observed["sha256"] == row["sha256"] and observed["size"] == row["size"],
             "native source bytes differ from signed Git")
    protected = common.current_bindings(plan, deployment)
    protected.update(ref["path"] for ref in release.values())
    protected.add(plan["source_closure"]["path"])
    need(not common.overlaps(selected["root"], protected), "source overlaps current or retained authority")
    return closure


def controls(root, plan):
    """Only bounded inert import metadata can be read before invoking Git."""
    commit = plan["source"]["commit"]
    raw = {}
    for row in plan["git_controls"]:
        path = root / ".git" / row["name"]
        with common.held(path, digest=row["sha256"], size=row["size"], mode=0o644, maximum=4096) as (fd, _, _):
            raw[row["name"]] = os.pread(fd, row["size"] + 1, 0)
    need(raw["HEAD"] == b"ref: refs/heads/optimizations\n"
         and all(raw[name] == (commit + "\n").encode() for name in ("ORIG_HEAD", "shallow", "refs/heads/optimizations")),
         "Git control identity differs")
    need(raw["config"] == b"[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = true\n",
         "retained Git config has unadmitted settings")
    first = rb"0{40} " + commit.encode() + rb" root <root@taira-linux-bootstrap\.local> [0-9]{1,19} \+0000\n"
    second = commit.encode() + b" " + commit.encode() + rb" root <root@taira-linux-bootstrap\.local> [0-9]{1,19} \+0000\treset: moving to " + commit.encode() + b"\n"
    need(re.fullmatch(first, raw["logs/refs/heads/optimizations"]) is not None
         and re.fullmatch(first + second, raw["logs/HEAD"]) is not None
         and raw["logs/HEAD"].startswith(raw["logs/refs/heads/optimizations"]), "retained reflog is not closed public import history")


def validate_git_index(raw, proof):
    """Accept only stage-zero v2 index entries and the typed public tree cache."""
    need(len(raw) >= 32 and raw[:4] == b"DIRC" and struct.unpack(">I", raw[4:8])[0] == 2
         and hashlib.sha1(raw[:-20]).digest() == raw[-20:], "Git index format/checksum differs")
    entries = proof["entries"]
    need(struct.unpack(">I", raw[8:12])[0] == len(entries), "Git index count differs")
    position = 12
    for row in entries:
        start = position
        need(position + 62 < len(raw) - 20, "Git index entry truncated")
        mode = struct.unpack(">I", raw[position + 24:position + 28])[0]
        flags = struct.unpack(">H", raw[position + 60:position + 62])[0]
        end = raw.find(b"\0", position + 62, len(raw) - 20)
        name = row["path"].encode()
        need(end >= 0 and flags & 0xf000 == 0 and flags & 0xfff == min(len(name), 0xfff)
             and raw[position + 62:end] == name and mode == int(row["mode"], 8)
             and raw[position + 40:position + 60].hex() == row["object"], "Git index entry differs from signed tree")
        position = start + ((end + 1 - start + 7) // 8) * 8
        need(position <= len(raw) - 20 and not any(raw[end:position]), "Git index padding differs")
    seen = set()
    directories = {""}
    for row in entries:
        directories.update(str(parent) for parent in PurePosixPath(row["path"]).parents if str(parent) != ".")
    trees = {row["object"] for row in proof["objects"] if row["type"] == "tree"}
    while position < len(raw) - 20:
        need(position + 8 <= len(raw) - 20, "Git index extension truncated")
        kind, size = raw[position:position + 4], struct.unpack(">I", raw[position + 4:position + 8])[0]
        need(kind == b"TREE" and kind not in seen and position + 8 + size <= len(raw) - 20,
             "Git index contains an unadmitted extension")
        seen.add(kind)
        payload = raw[position + 8:position + 8 + size]
        cursor, cache_paths = 0, set()
        def cache(parent, depth):
            nonlocal cursor
            need(depth <= 64, "Git cache tree exceeds depth")
            end = payload.find(b"\0", cursor)
            need(end >= 0, "Git cache tree name truncated")
            name = payload[cursor:end].decode("utf-8")
            path = name if not parent else parent + "/" + name
            need((depth == 0 and name == "") or (name and "/" not in name and name not in {".", ".."}), "Git cache tree name differs")
            need(path in directories and path not in cache_paths, "Git cache tree escaped signed directories")
            cache_paths.add(path)
            line = payload.find(b"\n", end + 1)
            need(line >= 0, "Git cache tree counts truncated")
            match = re.fullmatch(rb"(-1|[0-9]{1,6}) ([0-9]{1,6})", payload[end + 1:line])
            need(match is not None, "Git cache tree counts malformed")
            count, children = map(int, match.groups())
            need(children <= len(directories), "Git cache tree count exceeds bound")
            cursor = line + 1
            if count >= 0:
                expected = sum(not path or row["path"].startswith(path + "/") for row in entries)
                need(count == expected and cursor + 20 <= len(payload) and payload[cursor:cursor + 20].hex() in trees,
                     "Git cache tree object/count differs")
                cursor += 20
            for _ in range(children):
                cache(path, depth + 1)
        cache("", 0)
        need(cursor == len(payload), "Git cache tree has trailing payload")
        position += 8 + size
    need(position == len(raw) - 20, "Git index has trailing payload")


def validate_retained_pack(fd, proof):
    """Authenticate existing pack bytes without imposing a new-import encoding.

    This is an envelope check for an exact receipt-pinned retained artifact.
    The caller must also validate its indexes and complete canonical Git object
    inventory. It does not decode objects or authorize a shipping source import.
    """
    size = proof["pack"]["size"]
    count = len(proof["objects"])
    need(type(size) is int and 32 <= size <= MAX_TOTAL and 0 < count <= MAX_ENTRIES,
         "retained Git pack envelope exceeds its bound")
    need(os.fstat(fd).st_size == size and os.pread(fd, 12, 0) == b"PACK" + struct.pack(">II", 2, count),
         "retained Git pack header/object census differs")
    transport, checksum = hashlib.sha256(), hashlib.sha1()
    offset = 0
    while offset < size - 20:
        raw = os.pread(fd, min(common.CHUNK, size - 20 - offset), offset)
        need(raw, "retained Git pack is truncated")
        transport.update(raw)
        checksum.update(raw)
        offset += len(raw)
    trailer = os.pread(fd, 20, offset)
    need(len(trailer) == 20 and not os.pread(fd, 1, size), "retained Git pack size changed")
    transport.update(trailer)
    need(transport.hexdigest() == proof["pack"]["sha256"], "retained Git pack transport digest differs")
    need(trailer == checksum.digest(), "retained Git pack trailer checksum differs")


def validate_pack_indexes(index, reverse, proof, pack_trailer):
    """Reject extra bytes/objects in the exact index and reverse-index namespaces."""
    count = len(proof["objects"])
    need(index[:8] == b"\xfftOc\x00\x00\x00\x02" and len(index) >= 1072 + 28 * count,
         "Git pack index format differs")
    fanout = struct.unpack(">256I", index[8:1032])
    identifiers = [row["object"] for row in proof["objects"]]
    need(list(fanout) == [sum(int(oid[:2], 16) <= value for oid in identifiers) for value in range(256)]
         and index[1032:1032 + 20 * count] == b"".join(bytes.fromhex(oid) for oid in identifiers),
         "Git pack index object census differs")
    offset_start = 1032 + 24 * count
    offsets = list(struct.unpack(f">{count}I", index[offset_start:offset_start + 4 * count]))
    large = [value & 0x7fffffff for value in offsets if value & 0x80000000]
    need(sorted(large) == list(range(len(large))) and len(index) == 1072 + 28 * count + 8 * len(large),
         "Git pack index has unadmitted offset payload")
    large_values = struct.unpack(f">{len(large)}Q", index[offset_start + 4 * count:-40]) if large else ()
    offsets = [large_values[value & 0x7fffffff] if value & 0x80000000 else value for value in offsets]
    need(len(set(offsets)) == count and all(12 <= value < proof["pack"]["size"] - 20 for value in offsets)
         and index[-40:-20] == pack_trailer and hashlib.sha1(index[:-20]).digest() == index[-20:],
         "Git pack index checksum/offsets differ")
    need(len(reverse) == 52 + 4 * count and reverse[:12] == b"RIDX\x00\x00\x00\x01\x00\x00\x00\x01"
         and reverse[-40:-20] == pack_trailer and hashlib.sha1(reverse[:-20]).digest() == reverse[-20:],
         "Git reverse index format/checksum differs")
    ordering = struct.unpack(f">{count}I", reverse[12:-40])
    need(list(ordering) == sorted(range(count), key=offsets.__getitem__), "Git reverse index order differs")


def git_metadata(root, records, proof):
    by_path = {row["path"]: row for row in records}
    pack = next(row for row in records if row["path"].startswith(".git/objects/pack/") and row["path"].endswith(".pack"))
    with common.held(root / pack["path"], digest=pack["sha256"], size=pack["size"], mode=0o444) as (fd, _, _):
        trailer = os.pread(fd, 20, pack["size"] - 20)
    need(Path(pack["path"]).stem == "pack-" + trailer.hex(), "Git pack filename checksum differs")
    values = {}
    for name in (".git/index", pack["path"].removesuffix(".pack") + ".idx", pack["path"].removesuffix(".pack") + ".rev"):
        row = by_path[name]
        with common.held(root / name, digest=row["sha256"], size=row["size"], stamp=row["identity"], maximum=MAX_RECORD) as (fd, _, _):
            values[Path(name).suffix] = os.pread(fd, row["size"] + 1, 0)
    validate_git_index(values[""], proof)
    validate_pack_indexes(values[".idx"], values[".rev"], proof, trailer)


def symlink_bytes(root, row):
    path = root / row["path"]
    with common.anchored_directory(path.parent) as parent:
        before = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
        need(stat.S_ISLNK(before.st_mode) and common.identity(before) == row["identity"], "source symlink changed")
        raw = os.fsencode(os.readlink(path.name, dir_fd=parent))
        need(common.identity(before) == common.identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False)), "source symlink raced")
        return raw


def live(admission, roots, own_fds=()):
    rows = [{"path": str(Path(admission["source_root"]) / row["path"]), "identity": row["identity"]}
            for row in admission["records"] if row["kind"] == "file"]
    # Carry original file identities through quarantine and deletion. The common
    # observer checks inode aliases as well as both namespace roots.
    common.no_live_references(roots, own_fds, file_identities=rows)


def validate_admission(plan, proof, admission):
    need(isinstance(admission, dict) and set(admission) == {"schema", "plan_sha256", "proof_sha256",
         "deployment", "source_root", "parent_identity", "records", "payload_bytes", "allocated_bytes"}
         and admission["schema"] == SCHEMA and admission["plan_sha256"] == sha(canonical(plan))
         and admission["proof_sha256"] == sha(canonical(proof))
         and admission["source_root"] == plan["source"]["root"], "source admission binding differs")
    rows = admission["records"]
    need(isinstance(rows, list) and 0 < len(rows) <= MAX_ENTRIES
         and [row["path"] for row in rows] == sorted({row["path"] for row in rows})
         and rows[0]["path"] == "." and rows[0]["kind"] == "directory", "source admission census differs")
    need(isinstance(admission["parent_identity"], list) and len(admission["parent_identity"]) == 2
         and all(type(value) is int and value >= 0 for value in admission["parent_identity"]), "source parent identity differs")
    objects = {row["object"]: row for row in proof["objects"]}
    tracked = [{"path": row["path"], "mode": int(row["mode"], 8) & 0o777 if row["mode"] in {"100644", "100755"} else int(row["mode"], 8),
                "size": objects[row["object"]]["size"] if row["mode"] != "160000" else 40}
               for row in proof["entries"]]
    offset = 0
    entries = {row["path"]: row for row in proof["entries"]}
    controls_by_path = {".git/" + row["name"]: row for row in plan["git_controls"]}
    for row in rows:
        keys = {"path", "kind", "identity", "allocated_bytes"}
        need(row["kind"] in {"directory", "file", "symlink"}, "unadmitted source entry kind")
        if row["kind"] != "directory":
            keys |= {"size", "sha256", "offset"}
        need(set(row) == keys and isinstance(row["identity"], list) and len(row["identity"]) == 9
             and all(type(value) is int and value >= 0 for value in row["identity"])
             and type(row["allocated_bytes"]) is int and row["allocated_bytes"] >= 0,
             "invalid source entry metadata")
        info = row["identity"]
        need(info[0] == rows[0]["identity"][0] and info[3:5] == rows[0]["identity"][3:5]
             and (row["kind"] == "directory" or info[5] == 1)
             and {"directory": stat.S_ISDIR, "file": stat.S_ISREG, "symlink": stat.S_ISLNK}[row["kind"]](info[2]),
             "source entry custody differs")
        if row["kind"] != "directory":
            need(type(row["size"]) is int and row["size"] == info[6]
                 and type(row["offset"]) is int and row["offset"] == offset
                 and isinstance(row["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", row["sha256"]), "source payload census differs")
            offset += row["size"]
            expected = (objects[entries[row["path"]]["object"]] if row["path"] in entries
                        else controls_by_path.get(row["path"]))
            if expected is not None:
                need(row["sha256"] == expected["sha256"] and row["size"] == expected["size"], "source archived bytes differ from public authority")
            if row["path"].startswith(".git/objects/pack/") and row["path"].endswith(".pack"):
                need(row["sha256"] == proof["pack"]["sha256"] and row["size"] == proof["pack"]["size"], "archived Git pack differs")
    retry._retire_source_validate_records(rows, tracked, pack_size=proof["pack"]["size"])
    need(type(admission["payload_bytes"]) is int and admission["payload_bytes"] == offset <= MAX_TOTAL
         and admission["allocated_bytes"] == sum(row["allocated_bytes"] for row in rows), "source archive totals differ")
    return admission


def inspect(plan, deployment, proof, *, own_fds=()):
    closure = authority(plan, deployment, proof)
    root = common.direct(plan["source"]["root"])
    records = retry._retire_source_census(root, closure["tracked_files"], pack_size=proof["pack"]["size"])
    controls(root, plan)
    objects = {row["object"]: row for row in proof["objects"]}
    entries = {row["path"]: row for row in proof["entries"]}
    offset = 0
    for row in records:
        if row["kind"] == "directory":
            continue
        path = root / row["path"]
        if row["kind"] == "file":
            with common.held(path, size=row["identity"][6], stamp=row["identity"], maximum=MAX_TOTAL) as (_, info, digest):
                size = info.st_size
        else:
            payload = symlink_bytes(root, row)
            size, digest = len(payload), sha(payload)
            source._link_target(row["path"], payload, {name: item["mode"] for name, item in entries.items()})
            need(not os.path.lexists(path.parent / os.fsdecode(payload)), "source symlink referent appeared")
        if row["path"] in entries:
            expected = objects[entries[row["path"]]["object"]]
            need(size == expected["size"] and digest == expected["sha256"], "source content differs from signed blob")
        row.update(size=size, sha256=digest, offset=offset)
        offset += size
    need(offset <= MAX_TOTAL, "actual public source archive exceeds bound")
    pack = next(row for row in records if row["path"].endswith(".pack"))
    need(pack["sha256"] == proof["pack"]["sha256"] and pack["size"] == proof["pack"]["size"], "source pack differs from receipt")
    with common.held(root / pack["path"], digest=pack["sha256"], size=pack["size"], mode=0o444) as (fd, _, _):
        validate_retained_pack(fd, proof)
    git_metadata(root, records, proof)
    source._verify_objects(root, proof)
    source._git(root, "verify-pack", str(root / pack["path"].removesuffix(".pack")) + ".idx")
    expected_index = b"".join(f"{row['mode']} {row['object']} 0\t{row['path']}\0".encode() for row in proof["entries"])
    need(source._git(root, "ls-files", "--stage", "-z") == expected_index, "Git index differs from signed source")
    need(not source._git(root, "status", "--porcelain=v1", "--untracked-files=all"), "source is no longer clean")
    need(retry._retire_source_census(root, closure["tracked_files"], pack_size=proof["pack"]["size"])
         == [{key: row[key] for key in ("path", "kind", "identity", "allocated_bytes")} for row in records],
         "source metadata changed during authentication")
    with common.anchored_directory(root.parent) as parent:
        parent_identity = common.identity(os.fstat(parent))[:2]
    admission = {"schema": SCHEMA, "plan_sha256": sha(canonical(plan)), "proof_sha256": sha(canonical(proof)),
                 "deployment": deployment, "source_root": str(root), "parent_identity": parent_identity,
                 "records": records, "payload_bytes": offset, "allocated_bytes": sum(row["allocated_bytes"] for row in records)}
    need(len(canonical(admission)) <= MAX_RECORD, "source admission exceeds bound")
    validate_admission(plan, proof, admission)
    live(admission, [str(root)], own_fds)
    authority(plan, deployment, proof)
    return admission


def frame(fd, value):
    raw = canonical(value)
    need(len(raw) <= MAX_RECORD, "source frame exceeds bound")
    common.write_all(fd, struct.pack(">I", len(raw)) + raw)


class Reader(common.Reader):
    def frame(self):
        size = struct.unpack(">I", self.exact(4))[0]
        need(0 < size <= MAX_RECORD, "source frame exceeds bound")
        return decode(self.exact(size))


def archive_stream(plan, deployment, proof, admission, fd):
    with common.authority_locks(deployment):
        need(inspect(plan, deployment, proof) == admission, "archive source admission changed")
        root = Path(admission["source_root"])
        frame(fd, {"admission_sha256": sha(canonical(admission))})
        for row in admission["records"]:
            if row["kind"] == "directory":
                continue
            if row["kind"] == "symlink":
                raw = symlink_bytes(root, row)
                need(len(raw) == row["size"] and sha(raw) == row["sha256"], "archive symlink changed")
                common.write_all(fd, raw)
            else:
                with common.held(root / row["path"], digest=row["sha256"], size=row["size"], stamp=row["identity"]) as (source_fd, _, _):
                    offset = 0
                    while offset < row["size"]:
                        data = os.pread(source_fd, min(common.CHUNK, row["size"] - offset), offset)
                        need(data, "archive source truncated")
                        common.write_all(fd, data)
                        offset += len(data)
        need(inspect(plan, deployment, proof) == admission, "archive source changed while streaming")
        frame(fd, {"archive_stream_verified": True, "admission_sha256": sha(canonical(admission))})


def removal_order(admission):
    return sorted(admission["records"], key=lambda row: (row["kind"] == "directory", -len(PurePosixPath(row["path"]).parts), row["path"]))


def retirement_intent(admission, archive_digest):
    token = sha(canonical({"admission": sha(canonical(admission)), "archive": archive_digest}))
    return {"schema": SCHEMA, "operation": "retire-one-public-source", "token": token,
            "archive_sha256": archive_digest, "admission_sha256": sha(canonical(admission)),
            "quarantine": str(Path(admission["source_root"]).with_name(".retained-source-" + token))}


def retirement_capacity(admission, archive_digest):
    count = len(admission["records"])
    batches = (count + BATCH - 1) // BATCH
    # Retained admission and proof already live off-host. Guest keeps an exact
    # admission plus intent and two <=4KiB immutable markers per batch. Charge
    # final+pending publication overlap, diagnostics, locks and directory slack.
    payload = 3 * len(canonical(admission)) + 3 * len(canonical(retirement_intent(admission, archive_digest))) + (4 * batches + 16) * 4096
    return common.allocation(admission["deployment"]["runtime_root"], payload, 4 * batches + 24, 2, reserve=RESERVE)


def remaining(admission, root, absent_before):
    expected = {row["path"]: row for row in admission["records"]}
    ordered = removal_order(admission)
    missing_allowed = {row["path"] for row in ordered[:absent_before]}
    if not os.path.lexists(root):
        need(missing_allowed == set(expected), "source absent without exact deletion intent")
        return []
    actual = retry._retire_source_walk(root)
    names = {row["path"] for row in actual}
    need(set(expected) - names <= missing_allowed, "source member absent without deletion intent")
    for row in actual:
        previous = expected.get(row["path"])
        need(previous is not None and row["kind"] == previous["kind"], "source gained an unadmitted path")
        fields = 5 if row["kind"] == "directory" else 9
        need(row["identity"][:fields] == previous["identity"][:fields], "source member identity changed")
    return actual


def unlink_member(root, row):
    path = root if row["path"] == "." else root / row["path"]
    with common.anchored_directory(path.parent) as parent:
        before = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
        fields = 5 if row["kind"] == "directory" else 9
        need(common.identity(before)[:fields] == row["identity"][:fields], "source member replaced before deletion")
        if row["kind"] == "file":
            fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent)
            try:
                need(common.identity(os.fstat(fd)) == common.identity(before)
                     and common.hash_fd(fd, row["size"]) == row["sha256"], "source bytes changed before deletion")
            finally:
                os.close(fd)
        elif row["kind"] == "symlink":
            need(sha(os.fsencode(os.readlink(path.name, dir_fd=parent))) == row["sha256"], "source link changed before deletion")
        with common.anchored_directory(path.parent) as reopened:
            need(common.identity(os.fstat(reopened))[:2] == common.identity(os.fstat(parent))[:2], "source parent changed before deletion")
        need(common.identity(before) == common.identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False)), "source name raced before deletion")
        if row["kind"] == "directory":
            os.rmdir(path.name, dir_fd=parent)
        else:
            os.unlink(path.name, dir_fd=parent)
        os.fsync(parent)


def retire_locked(plan, deployment, proof, admission, intent, parent):
    work = parent / intent["token"]
    root, quarantine = Path(admission["source_root"]), Path(intent["quarantine"])
    if not work.exists():
        need(inspect(plan, deployment, proof) == admission, "source changed before retirement intent")
        common.fresh_directory(work)
    common.private_directory(work)
    if not (work / "intent.json").exists():
        need(set(os.listdir(work)) <= {".intent.json.pending"}, "unpublished source intent has foreign progress")
        need(inspect(plan, deployment, proof) == admission, "source changed before recovering intent")
        write_new(work / "intent.json", canonical(intent))
    need(not os.path.lexists(work / ".intent.json.pending") and read(work / "intent.json") == canonical(intent), "source retirement intent differs")
    common.sync(work)
    if not (work / "admission.json").exists():
        need(not os.path.lexists(quarantine), "quarantine preceded durable admission")
        write_new(work / "admission.json", canonical(admission))
    need(not os.path.lexists(work / ".admission.json.pending") and read(work / "admission.json") == canonical(admission), "source admission changed")
    common.sync(work)
    ordered = removal_order(admission)
    batches = [(start, min(start + BATCH, len(ordered))) for start in range(0, len(ordered), BATCH)]
    allowed = {"intent.json", "admission.json", "quarantined.json", "completed.json"}
    allowed.update(f"{index:04d}.{stage}.json" for index in range(len(batches)) for stage in ("delete-intent", "deleted"))
    allowed |= {"." + name + ".pending" for name in tuple(allowed)}
    need(set(os.listdir(work)) <= allowed, "source retirement has unexpected progress")
    need(not (os.path.lexists(root) and os.path.lexists(quarantine)), "original source reappeared")
    with common.anchored_directory(root.parent) as parent_fd:
        need(common.identity(os.fstat(parent_fd))[:2] == admission["parent_identity"], "source parent was replaced")
    if os.path.lexists(root):
        need(not (work / "quarantined.json").exists(), "quarantined original source reappeared")
        need(inspect(plan, deployment, proof) == admission, "source changed before quarantine")
        with common.anchored_directory(root.parent) as parent_fd:
            fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent_fd)
            try:
                need(common.identity(os.fstat(fd)) == admission["records"][0]["identity"], "source root changed before quarantine")
                common.rename_exclusive(root, quarantine, fd, admission["parent_identity"])
                need(common.identity(os.fstat(fd))[:5] == admission["records"][0]["identity"][:5]
                     and common.identity(os.fstat(fd)) == common.identity(quarantine.lstat()), "source root changed during quarantine")
            finally:
                os.close(fd)
    allowed_missing = 0
    for index, (start, end) in enumerate(batches):
        marker = work / f"{index:04d}.delete-intent.json"
        complete = work / f"{index:04d}.deleted.json"
        value = {"start": start, "end": end, "admission_sha256": intent["admission_sha256"]}
        if marker.exists():
            need(start == allowed_missing and read(marker) == canonical(value), "source deletion progress is not a prefix")
            allowed_missing = end
            if complete.exists():
                need(read(complete) == canonical(value)
                     and not any(os.path.lexists(quarantine if row["path"] == "." else quarantine / row["path"])
                                 for row in ordered[start:end]), "deleted source member reappeared")
        else:
            need(not complete.exists(), "source completion lacks deletion intent")
    remaining(admission, quarantine, allowed_missing)
    authority(plan, deployment, proof)
    live(admission, [str(root), str(quarantine)])
    common.marker(work, "quarantined.json", {"intent_sha256": sha(canonical(intent))})
    for index, (start, end) in enumerate(batches):
        value = {"start": start, "end": end, "admission_sha256": intent["admission_sha256"]}
        if (work / f"{index:04d}.deleted.json").exists():
            need(read(work / f"{index:04d}.deleted.json") == canonical(value), "source deletion completion differs")
            continue
        need(not os.path.lexists(root), "original source reappeared")
        with common.anchored_directory(root.parent) as parent_fd:
            need(common.identity(os.fstat(parent_fd))[:2] == admission["parent_identity"], "source parent changed during retirement")
        authority(plan, deployment, proof)
        live(admission, [str(root), str(quarantine)])
        remaining(admission, quarantine, allowed_missing)
        common.marker(work, f"{index:04d}.delete-intent.json", value)
        allowed_missing = max(allowed_missing, end)
        for row in ordered[start:end]:
            path = quarantine if row["path"] == "." else quarantine / row["path"]
            if os.path.lexists(path):
                unlink_member(quarantine, row)
        common.marker(work, f"{index:04d}.deleted.json", value)
    need(not os.path.lexists(root) and not os.path.lexists(quarantine), "source closure remains after retirement")
    authority(plan, deployment, proof)
    live(admission, [str(root), str(quarantine)])
    result = {"schema": SCHEMA, "retired": True, "archive_sha256": intent["archive_sha256"],
              "intent_sha256": sha(canonical(intent)), "entries": len(ordered),
              "allocated_bytes_removed": admission["allocated_bytes"], "receipts_history_runtime_preserved": True,
              "deployment_authorized": False}
    common.marker(work, "completed.json", result)
    return result


def retire(plan, deployment, proof, admission, archive_digest):
    validate_admission(plan, proof, admission)
    need(deployment == admission["deployment"], "source deployment binding differs")
    intent = retirement_intent(admission, archive_digest)
    with common.authority_locks(deployment):
        authority(plan, deployment, proof)
        retirement_capacity(admission, archive_digest)
        parent = Path(deployment["runtime_root"]) / "retained-public-source-v1"
        if not parent.exists():
            common.fresh_directory(parent)
        common.private_directory(parent)
        with common.anchored_directory(parent) as directory:
            fd = os.open("custody.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=directory)
            try:
                info = os.fstat(fd)
                need(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and info.st_nlink == 1
                     and stat.S_IMODE(info.st_mode) == 0o600 and info.st_size == 0, "source custody lock differs")
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                need(common.identity(info) == common.identity(os.fstat(fd)) == common.identity(os.stat("custody.lock", dir_fd=directory, follow_symlinks=False)), "source custody lock replaced")
                return retire_locked(plan, deployment, proof, admission, intent, parent)
            finally:
                os.close(fd)


def verify_payload(fd, admission):
    offset = 0
    for row in admission["records"]:
        if row["kind"] == "directory":
            continue
        need(row["offset"] == offset and row["size"] >= 0, "archive source offsets differ")
        digest, remaining_bytes = hashlib.sha256(), row["size"]
        while remaining_bytes:
            chunk = os.pread(fd, min(common.CHUNK, remaining_bytes), offset)
            need(chunk, "source archive truncated")
            digest.update(chunk)
            remaining_bytes -= len(chunk)
            offset += len(chunk)
        need(digest.hexdigest() == row["sha256"], "source archive member differs")
    need(offset == admission["payload_bytes"] and not os.pread(fd, 1, offset), "source archive length differs")


def verify_archive(path):
    common.private_directory(path)
    need(set(os.listdir(path)) == {"plan.json", "proof.json", "admission.json", "payload.bin", "archive.stderr", "completed.json"}, "source archive namespace differs")
    plan = validate_plan(decode(read(path / "plan.json")))
    proof, admission, completed = (decode(read(path / name)) for name in ("proof.json", "admission.json", "completed.json"))
    validate_admission(plan, proof, admission)
    need(completed == {"schema": SCHEMA, "archive_complete": True, "admission_sha256": sha(canonical(admission)),
         "plan_sha256": sha(canonical(plan)), "proof_sha256": sha(canonical(proof)), "payload_sha256": completed["payload_sha256"],
         "retirement_authorized": False}, "source archive completion differs")
    with common.held(path / "payload.bin", digest=completed["payload_sha256"], size=admission["payload_bytes"], mode=0o400, maximum=MAX_TOTAL) as (fd, _, _):
        verify_payload(fd, admission)
    return plan, proof, admission, sha(canonical(completed))


@contextlib.contextmanager
def held_archive(path, expected):
    """Hold exactly the initially verified archive across remote retirement."""
    plan, proof, admission, digest = expected
    with contextlib.ExitStack() as stack:
        for name, value in (("plan", plan), ("proof", proof), ("admission", admission)):
            stack.enter_context(common.held(path / (name + ".json"), digest=sha(canonical(value)), mode=0o400, maximum=MAX_RECORD))
        stack.enter_context(common.held(path / "completed.json", digest=digest, mode=0o400, maximum=MAX_RECORD))
        completed = decode(read(path / "completed.json", digest))
        stack.enter_context(common.held(path / "payload.bin", digest=completed["payload_sha256"],
            mode=0o400, size=admission["payload_bytes"], maximum=MAX_TOTAL))
        need(verify_archive(path) == expected, "initially verified source archive was replaced")
        yield
        need(verify_archive(path) == expected, "source archive changed during retirement")


def authenticated_modules(root, controller):
    modules, git = common.authenticated_modules(root, controller)
    for name in MODULES[len(common.MODULES):]:
        raw = common.read(Path(root) / "scripts" / (name + ".py"))
        need(raw == git("show", controller["commit"] + ":scripts/" + name + ".py"), "source owner differs from signed controller")
        modules[name] = raw
    return modules, git


def load_modules(modules):
    loaded = common.load_modules(modules)
    for name in MODULES[len(common.MODULES):]:
        module = types.ModuleType(name)
        module.__file__ = "<signed-retained-source>/" + name + ".py"
        module.__spec__ = importlib.util.spec_from_loader(name, loader=None, origin=module.__file__)
        sys.modules[name] = module
        exec(compile(modules[name], module.__file__, "exec"), module.__dict__)
        loaded[name] = module
    return loaded


BOOTSTRAP = common.BOOTSTRAP.replace("8388608", str(MAX_ENVELOPE)).replace(
    '("release_artifact_contract","taira_disk_capacity","taira_retry","taira_retained_release")', repr(MODULES)).replace(
    'sys.modules["taira_retained_release"].remote(e)', 'sys.modules["taira_retained_source"].remote(e)')


@contextlib.contextmanager
def session(route, envelope, modules, evidence):
    argv = list(retry.validate_ssh(route))
    argv[-1] = "/usr/bin/python3 -I -c " + shlex.quote(BOOTSTRAP)
    raw = canonical({**envelope, "modules": {name: {"source": base64.b64encode(modules[name]).decode(), "sha256": sha(modules[name])} for name in MODULES}})
    need(len(raw) <= MAX_ENVELOPE, "source bootstrap exceeds bound")
    fd = os.open(evidence, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as errors:
        child = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors,
            env={key: os.environ[key] for key in ("PATH", "HOME") if key in os.environ}, umask=0o077)
        try:
            child.stdin.write(struct.pack(">I", len(raw)) + raw)
            child.stdin.close()
            yield Reader(child.stdout.fileno())
            need(child.wait(timeout=30) == 0, "source operation failed; retain evidence")
            need(os.fstat(errors.fileno()).st_size <= common.MAX_RECORD, "source diagnostics exceed bound")
        finally:
            if child.poll() is None:
                child.kill()  # Only this invocation's SSH child, never a service/build.
            child.wait()
            child.stdout.close()


def call(route, envelope, modules, evidence):
    with session(route, envelope, modules, evidence) as reader:
        return reader.frame()


def remote(envelope):
    if envelope["operation"] == "backing-observe":
        return common.remote(envelope)
    if envelope["operation"] == "backing-capacity":
        need(sys.platform == "darwin" and type(envelope["bytes"]) is int and 0 <= envelope["bytes"] <= MAX_TOTAL,
             "bounded source metadata physical demand required")
        import taira_disk_capacity as capacity
        path = str(common.direct(envelope["path"]))
        result = capacity.evaluate({"schema": capacity.PLAN_SCHEMA, "allocations": [
            {"path": path, "label": "source retirement guest allocation including reserve", "bytes": envelope["bytes"], "inodes": 1},
            {"path": path, "label": "source retirement physical operating reserve", "bytes": RESERVE, "inodes": 256}]})
        need(result["passed"], "insufficient physical capacity for source retirement")
        frame(1, result)
        return
    need(sys.platform == "linux" and platform.machine() in {"aarch64", "arm64"} and os.geteuid() == 0,
         "approved root AArch64 Linux guest required")
    plan, deployment, proof = envelope["plan"], envelope["deployment"], envelope["proof"]
    validate_plan(plan)
    operation = envelope["operation"]
    if operation == "inspect":
        with common.authority_locks(deployment):
            frame(1, inspect(plan, deployment, proof))
    elif operation == "archive":
        archive_stream(plan, deployment, proof, envelope["admission"], 1)
    elif operation == "retire-capacity":
        frame(1, retirement_capacity(envelope["admission"], envelope["archive_sha256"])[0])
    elif operation == "retire":
        result = retire(plan, deployment, proof, envelope["admission"], envelope["archive_sha256"])
        result["storage_after"] = common.trim_and_observe(deployment["runtime_root"])
        frame(1, result)
    else:
        need(False, "unknown source retirement operation")


def archive_local(plan, deployment, proof, admission, modules, output):
    common.allocation(output.parent, admission["payload_bytes"] + 3 * (len(canonical(proof)) + len(canonical(admission))) + 65536, 16, 1, reserve=RESERVE)
    common.fresh_directory(output)
    for name, value in (("plan", plan), ("proof", proof), ("admission", admission)):
        write_new(output / (name + ".json"), canonical(value))
    envelope = {"operation": "archive", "plan": plan, "deployment": deployment, "proof": proof, "admission": admission}
    with session(plan["guest_ssh"], envelope, modules, output / "archive.stderr") as reader:
        need(reader.frame() == {"admission_sha256": sha(canonical(admission))}, "source stream admission differs")
        fd = os.open(output / "payload.bin", os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        try:
            left = admission["payload_bytes"]
            while left:
                raw = reader.exact(min(common.CHUNK, left))
                common.write_all(fd, raw)
                left -= len(raw)
            os.fsync(fd)
            verify_payload(fd, admission)
            digest = common.hash_fd(fd, admission["payload_bytes"])
            os.fchmod(fd, 0o400)
            os.fsync(fd)
        finally:
            os.close(fd)
        common.sync(output)
        need(reader.frame() == {"archive_stream_verified": True, "admission_sha256": sha(canonical(admission))}, "source stream final verification differs")
    result = {"schema": SCHEMA, "archive_complete": True, "admission_sha256": sha(canonical(admission)),
              "plan_sha256": sha(canonical(plan)), "proof_sha256": sha(canonical(proof)), "payload_sha256": digest,
              "retirement_authorized": False}
    write_new(output / "completed.json", canonical(result))
    verify_archive(output)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    sub = parser.add_subparsers(dest="operation", required=True)
    archive = sub.add_parser("archive", help="archive one public source through read-only guest admission")
    archive.add_argument("--plan", required=True, type=Path)
    archive.add_argument("--output-dir", required=True, type=Path)
    retire_parser = sub.add_parser("retire", help="explicitly remove only a completely archived exact source")
    retire_parser.add_argument("--archive-dir", required=True, type=Path)
    retire_parser.add_argument("--output-dir", required=True, type=Path)
    verify = sub.add_parser("verify", help="rehash complete archive locally without remote access")
    verify.add_argument("--archive-dir", required=True, type=Path)
    args = parser.parse_args()
    os.umask(0o077)
    if args.operation == "verify":
        *_, digest = verify_archive(common.direct(args.archive_dir))
        print(canonical({"verified": True, "archive_sha256": digest}).decode(), end="")
        return
    if args.operation == "archive":
        plan = validate_plan(decode(read(args.plan, mode=0o600)))
        archived = None
    else:
        archived = verify_archive(common.direct(args.archive_dir))
        plan = archived[0]
    modules, git = authenticated_modules(args.repo_root, plan["controller"])
    owner = load_modules(modules)["taira_retained_source"]
    owner.execute_authenticated(args, plan, archived, modules, git)


def execute_authenticated(args, plan, archived, modules, git):
    """All operational work continues in freshly loaded signed controller bytes."""
    retry.validate_ssh(plan["guest_ssh"])
    retry.validate_ssh(plan["backing_ssh"])
    deployment = common.deployment_projection(plan)
    expected_proof = canonical_proof(args.repo_root, plan, git)
    output = common.direct(args.output_dir)
    if args.operation == "archive":
        common.private_directory(output.parent)
        envelope = {"plan": plan, "deployment": deployment, "proof": expected_proof}
        admission = call(plan["guest_ssh"], {**envelope, "operation": "inspect"}, modules, output.with_name(output.name + ".inspect.stderr"))
        result = archive_local(plan, deployment, expected_proof, admission, modules, output)
    else:
        need(verify_archive(args.archive_dir) == archived, "archive changed before signed controller admission")
        _, proof, admission, digest = archived
        need(proof == expected_proof and deployment == admission["deployment"], "archived source or occupied deployment changed")
        common.fresh_directory(output)
        envelope = {"plan": plan, "deployment": deployment, "proof": proof, "admission": admission, "archive_sha256": digest}
        capacity = call(plan["guest_ssh"], {**envelope, "operation": "retire-capacity"}, modules, output / "capacity.stderr")
        before = call(plan["backing_ssh"], {"operation": "backing-capacity", "path": plan["backing_path"],
            "bytes": sum(row["bytes"] for row in capacity["allocations"])}, modules, output / "backing.stderr")
        with held_archive(args.archive_dir, archived):
            result = call(plan["guest_ssh"], {**envelope, "operation": "retire"}, modules, output / "retire.stderr")
        result["backing_before"] = before
        result["backing_after"] = call(plan["backing_ssh"], {"operation": "backing-observe", "path": plan["backing_path"]}, modules, output / "backing-after.stderr")
        write_new(output / "completed.json", canonical(result))
    print(canonical(result).decode(), end="")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"taira retained source refused: {error}", file=sys.stderr)
        sys.exit(1)
