#!/usr/bin/env python3
"""Export/import an exact signed Taira source artifact, never a development checkout.

Requires Git, GnuPG and the reviewed full GPG signing fingerprint. Export reads
only Git objects from an optimizations repository, including the public signing
key; dirty/untracked working files are neither read nor changed. Import verifies
the signature in an isolated public-key ring and publishes a fresh, clean,
shallow source artifact for native public-reset admission. No build, activation,
private-key transfer, history traversal or overwrite is supported. HOME is
preserved. GNUPGHOME is used only for public-key verification in private scratch.
"""

from __future__ import annotations

import argparse
import base64
import contextlib
import ctypes
import hashlib
import os
from pathlib import Path, PurePosixPath
import re
import select
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
import time
import uuid
import zlib

from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_json_bytes,
    canonical_relative_path,
    create_fresh_directory,
    exclusive_output_fd,
    exclusive_write_bytes,
    load_json_object,
    stable_hash_path,
    stable_open_relative,
    stable_read_path,
    _open_anchored_regular,
    _open_absolute_directory,
)

SCHEMA = "iroha.taira.signed-source-capture.v1"
MAX_PACK_BYTES = 4 * 1024**3
MAX_MANIFEST_BYTES = 64 * 1024**2
MAX_OBJECT_BYTES = 4 * 1024**3
MAX_TOTAL_SOURCE_BYTES = 8 * 1024**3
MAX_OBJECTS = 200_000
MAX_FILES = 100_000
MAX_PUBLIC_KEY_BYTES = 128 * 1024
CHUNK = 1024 * 1024
GIT_TIMEOUT_SECONDS = 600
_OID = re.compile(r"[0-9a-f]{40}")
_SHA = re.compile(r"[0-9a-f]{64}")
_FINGERPRINT = re.compile(r"(?:[0-9A-F]{40}|[0-9A-F]{64})")


class SourceCaptureError(ReleaseArtifactError):
    """Signed source admission failed; no completed artifact was published."""


def _need(value, message):
    if not value:
        raise SourceCaptureError(message)


def _identity(info):
    return tuple(getattr(info, key) for key in (
        "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
        "st_size", "st_mtime_ns", "st_ctime_ns"))


def _expected(commit, tree, signer):
    _need(isinstance(commit, str) and _OID.fullmatch(commit), "expected commit must be full lowercase SHA1")
    _need(isinstance(tree, str) and _OID.fullmatch(tree), "expected tree must be full lowercase SHA1")
    _need(isinstance(signer, str) and _FINGERPRINT.fullmatch(signer),
          "expected signer must be the full uppercase GPG signing-key fingerprint")


def _absolute(path):
    path = Path(path)
    _need(path.is_absolute() and str(path) == os.path.abspath(path), "source artifact path must be absolute and canonical")
    return path


def _directory(path, *, owner=True, mode=None):
    path = _absolute(path)
    fd, _, info = _open_absolute_directory(path, "source artifact directory")
    os.close(fd)
    _need(not info.st_mode & 0o022 and (not owner or info.st_uid == os.geteuid()),
          "source artifact directory must be owner-held and nonshared")
    if mode is not None:
        _need(stat.S_IMODE(info.st_mode) == mode, "source artifact directory mode differs")
    # Existing ancestors are never repaired or accepted through a symlink.
    for parent in path.parents:
        info = parent.lstat()
        _need(stat.S_ISDIR(info.st_mode) and not info.st_mode & 0o022,
              "source artifact ancestor is shared or unsafe")
    return path


def _environment():
    env = {key: value for key, value in os.environ.items()
           if key in {"HOME", "PATH", "TMPDIR", "GNUPGHOME", "SYSTEMROOT"}}
    env.update(LC_ALL="C", LANG="C", GIT_CONFIG_NOSYSTEM="1",
               GIT_CONFIG_GLOBAL="/dev/null", GIT_TERMINAL_PROMPT="0",
               GIT_OPTIONAL_LOCKS="0", GIT_NO_REPLACE_OBJECTS="1")
    return env


def _tool(name):
    selected = shutil.which(name, path=_environment().get("PATH"))
    _need(selected is not None, f"required source tool is unavailable: {name}")
    path = Path(selected).resolve(strict=True)
    info = path.stat()
    _need(stat.S_ISREG(info.st_mode) and not info.st_mode & 0o022 and os.access(path, os.X_OK),
          f"source tool is nonexecutable or shared: {name}")
    return str(path)


def _read_pipe(fd, maximum, deadline):
    remaining = deadline - time.monotonic()
    _need(remaining > 0 and select.select([fd], [], [], remaining)[0], "source Git command exceeded its deadline")
    return os.read(fd, maximum)


def _run(argv, *, cwd, payload=None, input_fd=None, output_fd=None, maximum=MAX_MANIFEST_BYTES, env=None):
    """Bound every subprocess output and stream packs without holding them in RAM."""
    with contextlib.ExitStack() as stack:
        if payload is not None:
            _need(input_fd is None, "ambiguous source command input")
            incoming = stack.enter_context(tempfile.TemporaryFile())
            incoming.write(payload)
            incoming.seek(0)
            input_fd = incoming.fileno()
        process = subprocess.Popen(argv, cwd=cwd, env=env or _environment(),
                                   stdin=input_fd if input_fd is not None else subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, umask=0o077)
        deadline = time.monotonic() + GIT_TIMEOUT_SECONDS
        result = bytearray()
        size = 0
        try:
            while chunk := _read_pipe(process.stdout.fileno(), CHUNK, deadline):
                size += len(chunk)
                _need(size <= maximum, "source command output exceeds its bound")
                if output_fd is None:
                    result.extend(chunk)
                else:
                    view = memoryview(chunk)
                    while view:
                        count = os.write(output_fd, view)
                        _need(count > 0, "short source artifact write")
                        view = view[count:]
            code = process.wait(timeout=max(0.01, deadline - time.monotonic()))
            _need(code == 0, f"source command failed ({Path(argv[0]).name}, exit {code})")
        finally:
            if process.poll() is None:
                process.kill()  # Only the child started above; never an external process.
            process.wait()
            process.stdout.close()
        return bytes(result)


def _git_argv(root, *args):
    return [_tool("git"), "--no-replace-objects", "--git-dir", str(root / ".git"),
            "--work-tree", str(root), "-c", "core.hooksPath=/dev/null",
            "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false",
            "-c", "core.fileMode=true", "-c", "core.symlinks=true",
            "-c", "core.sparseCheckout=false", "-c", "protocol.allow=never", *args]


def _git(root, *args, **kwargs):
    return _run(_git_argv(root, *args), cwd=root, **kwargs)


def _public_key_packet_check(payload):
    """Accept bounded binary public OpenPGP packets; secret material is forbidden."""
    _need(0 < len(payload) <= MAX_PUBLIC_KEY_BYTES, "public signing key exceeds its bound")
    offset, primary_keys = 0, 0
    while offset < len(payload):
        first = payload[offset]
        offset += 1
        _need(first & 0x80, "invalid public key packet")
        if first & 0x40:
            tag = first & 0x3f
            _need(offset < len(payload), "truncated public key packet")
            length = payload[offset]
            offset += 1
            if 192 <= length < 224:
                _need(offset < len(payload), "truncated public key packet")
                length = ((length - 192) << 8) + payload[offset] + 192
                offset += 1
            elif length == 255:
                _need(offset + 4 <= len(payload), "truncated public key packet")
                length = int.from_bytes(payload[offset:offset + 4], "big")
                offset += 4
            else:
                _need(length < 224, "partial public key packets are not admitted")
        else:
            tag = (first >> 2) & 0xf
            width = (1, 2, 4, 0)[first & 3]
            _need(width and offset + width <= len(payload), "invalid public key packet length")
            length = int.from_bytes(payload[offset:offset + width], "big")
            offset += width
        _need(tag in {2, 6, 13, 14, 17} and offset + length <= len(payload),
              "signer export must contain only complete public key packets")
        primary_keys += tag == 6
        offset += length
    _need(primary_keys == 1, "signer export must contain exactly one public primary key")


def _verify_signature(root, commit, signer, key):
    _public_key_packet_check(key)
    # This is a public-only keyring; import cannot consult or mutate operator keys.
    # GnuPG probes its agent socket even with --no-autostart. A short native
    # temporary path avoids sockaddr_un limits for deeply nested artifact roots.
    # tempfile creates the unpredictable directory atomically with mode 0700.
    with tempfile.TemporaryDirectory(prefix=".taira-pub-", dir=Path("/tmp").resolve(strict=True)) as value:
        keyring = Path(value)
        keyring.chmod(0o700)
        # Public signature verification needs no signing agent or agent socket.
        # Avoid starting one inside long, explicitly selected artifact paths.
        exclusive_write_bytes(keyring / "gpg.conf", b"no-autostart\nno-auto-key-retrieve\n", mode=0o600)
        env = _environment()
        env["GNUPGHOME"] = str(keyring)
        gpg = _tool("gpg")
        _run([gpg, "--batch", "--no-options", "--no-autostart", "--no-auto-key-retrieve", "--import"],
             cwd=root, payload=key, maximum=MAX_PUBLIC_KEY_BYTES, env=env)
        _git(root, "-c", f"gpg.program={gpg}", "-c", "gpg.format=openpgp",
             "verify-commit", commit, env=env)
        fingerprint = _git(root, "-c", f"gpg.program={gpg}", "-c", "gpg.format=openpgp",
                           "show", "--no-patch", "--format=%GF", commit, env=env).decode().strip()
        # Match prepare's %GF contract, including a signing subkey fingerprint.
        _need(fingerprint == signer, "commit signature differs from the expected signing fingerprint")


def _source_path(value):
    canonical_relative_path(value)
    _need(not any(part.casefold() == ".git" for part in PurePosixPath(value).parts)
          and PurePosixPath(value).parts[0] != "target", "source tree contains a repository or output path")
    return value


def _tree(root, commit, tree):
    _need(_git(root, "cat-file", "-t", commit).strip() == b"commit", "selected source is not a commit")
    _need(_git(root, "rev-parse", commit + "^{tree}").decode().strip() == tree, "signed commit tree differs")
    objects = {commit: "commit", tree: "tree"}
    entries = []
    seen = set()
    for row in _git(root, "ls-tree", "-r", "-t", "-z", "--full-tree", tree).split(b"\0"):
        if not row:
            continue
        match = re.fullmatch(rb"(040000|100644|100755|120000|160000) (tree|blob|commit) ([0-9a-f]{40})\t(.+)", row, re.DOTALL)
        _need(match is not None, "signed tree contains an unsupported entry")
        mode, kind, oid, raw_path = match.groups()
        path = _source_path(raw_path.decode("utf-8"))
        _need(path not in seen, "signed tree contains duplicate paths")
        seen.add(path)
        oid, kind, mode = oid.decode(), kind.decode(), mode.decode()
        _need((mode == "040000" and kind == "tree") or (mode == "160000" and kind == "commit")
              or (mode in {"100644", "100755", "120000"} and kind == "blob"), "source entry type differs from its mode")
        if mode != "160000":
            _need(oid not in objects or objects[oid] == kind, "source object has conflicting types")
            objects[oid] = kind
        if mode != "040000":
            entries.append({"path": path, "mode": mode, "object": oid})
        _need(len(objects) <= MAX_OBJECTS and len(entries) <= MAX_FILES, "source object/file count exceeds its bound")
    _need(entries, "signed source tree is empty")
    return objects, sorted(entries, key=lambda row: row["path"])


@contextlib.contextmanager
def _object_reader(root):
    process = subprocess.Popen(_git_argv(root, "cat-file", "--batch"), cwd=root,
                               env=_environment(), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, umask=0o077)
    deadline = time.monotonic() + GIT_TIMEOUT_SECONDS

    def read(oid, kind, output_fd=None, payload=False):
        process.stdin.write((oid + "\n").encode())
        process.stdin.flush()
        header = bytearray()
        while not header.endswith(b"\n"):
            part = _read_pipe(process.stdout.fileno(), 1, deadline)
            _need(part and len(header) < 128, "invalid Git object response")
            header.extend(part)
        match = re.fullmatch(rb"([0-9a-f]{40}) (commit|tree|blob) ([0-9]+)\n", header)
        _need(match is not None and match[1].decode() == oid and match[2].decode() == kind,
              "Git object identity/type differs")
        size = int(match[3])
        _need(size <= MAX_OBJECT_BYTES and (not payload or size <= MAX_PUBLIC_KEY_BYTES), "Git object exceeds its byte bound")
        sha1 = hashlib.sha1(f"{kind} {size}\0".encode())
        sha256 = hashlib.sha256()
        remaining, result = size, bytearray()
        while remaining:
            chunk = _read_pipe(process.stdout.fileno(), min(CHUNK, remaining), deadline)
            _need(chunk, "truncated Git object")
            remaining -= len(chunk)
            sha1.update(chunk)
            sha256.update(chunk)
            if payload:
                result.extend(chunk)
            if output_fd is not None:
                view = memoryview(chunk)
                while view:
                    count = os.write(output_fd, view)
                    _need(count > 0, "short source file write")
                    view = view[count:]
        _need(_read_pipe(process.stdout.fileno(), 1, deadline) == b"\n" and sha1.hexdigest() == oid,
              "Git object bytes do not match their identity")
        return {"object": oid, "type": kind, "size": size, "sha256": sha256.hexdigest()}, bytes(result)

    try:
        yield read
        process.stdin.close()
        _need(process.wait(timeout=max(0.01, deadline - time.monotonic())) == 0, "Git object reader failed")
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        if not process.stdin.closed:
            process.stdin.close()
        process.stdout.close()


def _inventory(root, commit, tree):
    objects, entries = _tree(root, commit, tree)
    rows, total = [], 0
    with _object_reader(root) as read:
        for oid, kind in sorted(objects.items()):
            row, _ = read(oid, kind)
            total += row["size"]
            _need(total <= MAX_TOTAL_SOURCE_BYTES, "source object bytes exceed their aggregate bound")
            rows.append(row)
    sizes = {row["object"]: row["size"] for row in rows}
    source_bytes = sum(sizes[row["object"]] for row in entries if row["mode"] != "160000")
    _need(source_bytes <= MAX_TOTAL_SOURCE_BYTES, "expanded source exceeds its aggregate bound")
    return rows, entries, source_bytes


def _manifest(path, commit, tree, signer):
    _expected(commit, tree, signer)
    info, payload = stable_read_path(_absolute(path), max_size=MAX_MANIFEST_BYTES)
    value = load_json_object(payload, "signed source capture")
    _need(canonical_json_bytes(value) == payload, "source capture JSON is not canonical")
    _need(set(value) == {"schema", "commit", "tree", "signer_fingerprint", "public_key_base64",
                         "pack", "objects", "entries", "source_bytes"}
          and value["schema"] == SCHEMA and value["commit"] == commit and value["tree"] == tree
          and value["signer_fingerprint"] == signer, "source capture identity differs")
    pack = value["pack"]
    _need(isinstance(pack, dict) and set(pack) == {"sha256", "size"}
          and isinstance(pack["sha256"], str) and _SHA.fullmatch(pack["sha256"])
          and type(pack["size"]) is int and 0 < pack["size"] <= MAX_PACK_BYTES, "source pack descriptor is invalid")
    try:
        key = base64.b64decode(value["public_key_base64"], validate=True)
    except (ValueError, TypeError) as error:
        raise SourceCaptureError("source public key encoding is invalid") from error
    _public_key_packet_check(key)
    _need(base64.b64encode(key).decode() == value["public_key_base64"], "public key encoding is not canonical")
    objects, entries = value["objects"], value["entries"]
    _need(isinstance(objects, list) and 0 < len(objects) <= MAX_OBJECTS
          and isinstance(entries, list) and 0 < len(entries) <= MAX_FILES, "source inventory count is invalid")
    for row in objects:
        _need(isinstance(row, dict) and set(row) == {"object", "type", "size", "sha256"}
              and isinstance(row["object"], str) and _OID.fullmatch(row["object"])
              and isinstance(row["type"], str) and row["type"] in {"commit", "tree", "blob"}
              and type(row["size"]) is int and 0 <= row["size"] <= MAX_OBJECT_BYTES
              and isinstance(row["sha256"], str) and _SHA.fullmatch(row["sha256"]), "invalid source object row")
    _need([row["object"] for row in objects] == sorted({row["object"] for row in objects}), "source objects are not uniquely sorted")
    _need(sum(row["size"] for row in objects) <= MAX_TOTAL_SOURCE_BYTES, "source object aggregate exceeds its bound")
    for row in entries:
        _need(isinstance(row, dict) and set(row) == {"path", "mode", "object"}
              and isinstance(row["path"], str) and isinstance(row["mode"], str)
              and row["mode"] in {"100644", "100755", "120000", "160000"}
              and isinstance(row["object"], str) and _OID.fullmatch(row["object"]), "invalid source entry row")
        _source_path(row["path"])
    _need([row["path"] for row in entries] == sorted({row["path"] for row in entries}), "source entries are not uniquely sorted")
    _need(type(value["source_bytes"]) is int and 0 <= value["source_bytes"] <= MAX_TOTAL_SOURCE_BYTES,
          "source aggregate is invalid")
    return value, key, info


def _stage(destination):
    destination = _absolute(destination)
    _directory(destination.parent)
    _need(not os.path.lexists(destination), "source artifact destination already exists")
    stage = destination.parent / (".source-incomplete-" + uuid.uuid4().hex)
    return create_fresh_directory(stage, mode=0o700)


def _publish(stage, destination):
    """Exclusive directory rename; failures retain a private incomplete artifact."""
    _directory(destination.parent)
    _need(stage.parent == destination.parent, "source publication crossed directories")
    parent_fd, _, before = _open_absolute_directory(stage.parent, "source publication parent")
    try:
        staged = stage.lstat()
        libc = ctypes.CDLL(None, use_errno=True)
        if sys.platform == "linux":
            function = libc.renameat2
            flag = 1  # RENAME_NOREPLACE
        elif sys.platform == "darwin":
            function = libc.renameatx_np
            flag = 4  # RENAME_EXCL
        else:
            raise SourceCaptureError("exclusive source publication requires Linux or macOS")
        function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
        function.restype = ctypes.c_int
        code = function(parent_fd, os.fsencode(stage.name), parent_fd, os.fsencode(destination.name), flag)
        if code != 0:
            raise SourceCaptureError(f"exclusive source publication failed (errno {ctypes.get_errno()})")
        os.fsync(parent_fd)
        _need(_identity(before)[:2] == _identity(stage.parent.lstat())[:2]
              and _identity(staged)[:2] == _identity(destination.lstat())[:2], "source publication custody changed")
    finally:
        os.close(parent_fd)


def export_source(repo_root: Path, expected_commit: str, expected_tree: str,
                  expected_signer: str, output_dir: Path) -> dict:
    """Capture signed Git objects only; leave every worktree/index byte untouched."""
    _expected(expected_commit, expected_tree, expected_signer)
    root = _directory(repo_root)
    _directory(root / ".git")
    _need(_git(root, "symbolic-ref", "--short", "HEAD").strip() == b"optimizations", "source export requires optimizations")
    _need(_git(root, "rev-parse", "--show-toplevel").decode().strip() == str(root), "source repository root differs")
    stage = _stage(output_dir)
    key = _run([_tool("gpg"), "--batch", "--no-options", "--export-options", "export-minimal",
                "--export", expected_signer], cwd=root, maximum=MAX_PUBLIC_KEY_BYTES)
    _verify_signature(root, expected_commit, expected_signer, key)
    objects, entries, source_bytes = _inventory(root, expected_commit, expected_tree)
    pack_path = stage / "source.pack"
    with exclusive_output_fd(pack_path, mode=0o600) as fd:
        _git(root, "pack-objects", "--stdout", "--no-reuse-delta", "--no-reuse-object", "--window=0",
             payload="".join(row["object"] + "\n" for row in objects).encode(), output_fd=fd, maximum=MAX_PACK_BYTES)
    pack = stable_hash_path(pack_path, max_size=MAX_PACK_BYTES)
    manifest = {"schema": SCHEMA, "commit": expected_commit, "tree": expected_tree,
                "signer_fingerprint": expected_signer, "public_key_base64": base64.b64encode(key).decode(),
                "pack": {"size": pack.size, "sha256": pack.sha256}, "objects": objects,
                "entries": entries, "source_bytes": source_bytes}
    rendered = canonical_json_bytes(manifest)
    _need(len(rendered) <= MAX_MANIFEST_BYTES, "source capture manifest exceeds its bound")
    exclusive_write_bytes(stage / "source-capture.json", rendered, mode=0o600)
    # Verify the emitted pack independently, including no extra/history objects.
    verification = stage / "verification-source"
    import_source(pack_path, stage / "source-capture.json", expected_commit, expected_tree,
                  expected_signer, verification)
    shutil.rmtree(verification)  # This helper owns this validated disposable import only.
    _need(set(os.listdir(stage)) == {"source.pack", "source-capture.json"}, "source export has unexpected output")
    for path in stage.iterdir():
        path.chmod(0o400)
    _publish(stage, Path(output_dir))
    return {"commit": expected_commit, "tree": expected_tree, "signer_fingerprint": expected_signer,
            "pack_path": str(Path(output_dir) / "source.pack"), "pack_sha256": pack.sha256, "pack_size": pack.size,
            "manifest_path": str(Path(output_dir) / "source-capture.json"),
            "manifest_sha256": hashlib.sha256(rendered).hexdigest(), "manifest_size": len(rendered),
            "source_bytes": source_bytes, "file_count": len(entries), "object_count": len(objects)}


def _git_metadata(commit):
    line = (commit + "\n").encode()
    log = ("0" * 40 + " " + commit + " Taira Artifact <artifact@invalid> 0 +0000\timport signed source\n").encode()
    return {"HEAD": b"ref: refs/heads/optimizations\n", "ORIG_HEAD": line,
            "config": b"[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = true\n\tsymlinks = true\n",
            "shallow": line, "refs/heads/optimizations": line,
            "logs/HEAD": log, "logs/refs/heads/optimizations": log}


def _mkdir_parents(path, root):
    missing = []
    while path != root:
        _need(path.is_relative_to(root), "source parent escaped artifact")
        if path.exists():
            _directory(path, mode=0o755)
            break
        missing.append(path)
        path = path.parent
    for directory in reversed(missing):
        create_fresh_directory(directory, mode=0o755)


def _link_target(relative, payload, paths):
    try:
        target = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SourceCaptureError("source symlink target is not UTF-8") from error
    _need(target and len(payload) <= 4096 and not target.startswith("/")
          and "\\" not in target and "\0" not in target, "source symlink target is not bounded relative UTF-8")
    parts = list(PurePosixPath(relative).parent.parts)
    for part in target.split("/"):
        if part in {"", "."}:
            continue
        if part == "..":
            _need(parts, "source symlink escapes artifact root")
            parts.pop()
        else:
            parts.append(part)
    _need(parts, "source symlink targets artifact root")
    referent = "/".join(parts)
    _need(referent not in paths and not any(referent.startswith(value + "/") for value in paths
                                          if paths[value] in {"120000", "160000", "100644", "100755"}),
          "source symlink target must remain absent without link ancestors")
    return target


def _materialize(root, manifest):
    rows = {row["object"]: row for row in manifest["objects"]}
    paths = {row["path"]: row["mode"] for row in manifest["entries"]}
    all_paths = set(paths)
    for value in paths:
        all_paths.update(str(parent) for parent in PurePosixPath(value).parents if str(parent) != ".")
    with _object_reader(root) as read:
        for entry in manifest["entries"]:
            path = root / entry["path"]
            _mkdir_parents(path.parent, root)
            if entry["mode"] == "160000":
                create_fresh_directory(path, mode=0o755)
            elif entry["mode"] == "120000":
                row, payload = read(entry["object"], "blob", payload=True)
                _need(row == rows[entry["object"]], "source symlink object differs")
                target = _link_target(entry["path"], payload, paths)
                # Native admission requires the normalized referent to be absent,
                # including directories implied by other source entries.
                normalized = os.path.normpath(str(PurePosixPath(entry["path"]).parent / target))
                _need(normalized not in all_paths, "source symlink referent is present")
                os.symlink(target, path)
            else:
                with exclusive_output_fd(path, mode=0o755 if entry["mode"] == "100755" else 0o644) as fd:
                    row, _ = read(entry["object"], "blob", output_fd=fd)
                    _need(row == rows[entry["object"]], "source file object differs")


def _verify_objects(root, manifest):
    found = _git(root, "cat-file", "--batch-all-objects", "--batch-check=%(objectname) %(objecttype) %(objectsize)")
    expected = "".join(f"{row['object']} {row['type']} {row['size']}\n" for row in manifest["objects"]).encode()
    _need(found == expected, "imported Git object inventory contains missing, extra or history objects")
    objects, entries, source_bytes = _inventory(root, manifest["commit"], manifest["tree"])
    _need(objects == manifest["objects"] and entries == manifest["entries"] and source_bytes == manifest["source_bytes"],
          "imported source does not match the complete signed object/tree inventory")


def _validate_pack(fd, manifest):
    """Bound inflation before Git sees input; the maintained producer emits no deltas."""
    size = manifest["pack"]["size"]
    header = os.pread(fd, 12, 0)
    _need(len(header) == 12 and header[:4] == b"PACK"
          and struct.unpack(">II", header[4:]) == (2, len(manifest["objects"])),
          "source pack header/object census differs")
    expected = {row["object"]: row for row in manifest["objects"]}
    offset, total = 12, 0
    deadline = time.monotonic() + GIT_TIMEOUT_SECONDS
    for _ in range(len(expected)):
        first = os.pread(fd, 1, offset)
        _need(first and offset < size - 20, "source pack object is truncated")
        offset += 1
        first = first[0]
        kind = {1: "commit", 2: "tree", 3: "blob"}.get((first >> 4) & 7)
        _need(kind is not None, "source pack must contain only direct commit/tree/blob objects, without deltas")
        length, shift, continuation = first & 15, 4, first & 128
        while continuation:
            part = os.pread(fd, 1, offset)
            _need(part and offset < size - 20 and shift <= 32, "source pack object length is invalid")
            offset += 1
            length |= (part[0] & 127) << shift
            shift += 7
            continuation = part[0] & 128
        total += length
        _need(length <= MAX_OBJECT_BYTES and total <= MAX_TOTAL_SOURCE_BYTES,
              "source pack inflated object bytes exceed their bound")
        sha1 = hashlib.sha1(f"{kind} {length}\0".encode())
        sha256, produced = hashlib.sha256(), 0
        inflater = zlib.decompressobj()
        while not inflater.eof:
            _need(time.monotonic() < deadline, "source pack verification exceeded its deadline")
            compressed = os.pread(fd, min(CHUNK, max(0, size - 20 - offset)), offset)
            _need(compressed, "source pack compressed object is truncated")
            try:
                decoded = inflater.decompress(compressed, min(CHUNK, length - produced + 1))
            except zlib.error as error:
                raise SourceCaptureError("source pack compressed object is invalid") from error
            consumed = len(compressed) - len(inflater.unused_data if inflater.eof else inflater.unconsumed_tail)
            _need(consumed or decoded, "source pack decompressor made no progress")
            offset += consumed
            produced += len(decoded)
            _need(produced <= length, "source pack object inflates beyond its declared size")
            sha1.update(decoded)
            sha256.update(decoded)
        _need(produced == length, "source pack inflated object length differs")
        oid = sha1.hexdigest()
        row = {"object": oid, "type": kind, "size": length, "sha256": sha256.hexdigest()}
        _need(expected.pop(oid, None) == row, "source pack object is duplicate, foreign, or differs from its exact inventory")
    _need(not expected and offset == size - 20, "source pack contains extra or missing object bytes")
    checksum = hashlib.sha1()
    position = 0
    while position < offset:
        chunk = os.pread(fd, min(CHUNK, offset - position), position)
        _need(chunk, "source pack changed while checksumming")
        checksum.update(chunk)
        position += len(chunk)
    _need(os.pread(fd, 20, offset) == checksum.digest(), "source pack checksum differs")


def _freeze_new_pack(root):
    """Set canonical modes only on the new private index-pack outputs we own."""
    directory = root / ".git/objects/pack"
    fd, _, parent = _open_absolute_directory(directory, "new source pack directory")
    try:
        names = os.listdir(fd)
        _need(len(names) == 3 and {Path(name).suffix for name in names} == {".pack", ".idx", ".rev"}
              and len({Path(name).stem for name in names}) == 1
              and re.fullmatch(r"pack-[0-9a-f]{40}", Path(names[0]).stem), "unexpected new Git pack outputs")
        for name in names:
            before = os.stat(name, dir_fd=fd, follow_symlinks=False)
            child = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=fd)
            try:
                opened = os.fstat(child)
                _need(_identity(opened) == _identity(before) and stat.S_ISREG(opened.st_mode)
                      and opened.st_uid == os.geteuid() and opened.st_nlink == 1
                      and stat.S_IMODE(opened.st_mode) in {0o400, 0o444}, "new Git pack custody differs")
                os.fchmod(child, 0o444)
                os.fsync(child)
                _need(_identity(os.fstat(child)) == _identity(os.stat(name, dir_fd=fd, follow_symlinks=False)),
                      "new Git pack changed during publication")
            finally:
                os.close(child)
        os.fsync(fd)
        _need(_identity(directory.lstat())[:2] == _identity(parent)[:2], "new Git pack directory changed")
    finally:
        os.close(fd)


def _verify_source_file(path, row, mode):
    """Source blobs may be empty; shared nonempty artifact policy stays intact."""
    if row["size"] != 0:
        actual = stable_hash_path(path, max_size=MAX_OBJECT_BYTES)
        _need(actual.size == row["size"] and actual.sha256 == row["sha256"] and actual.mode == mode,
              "source file bytes/mode differ")
        return
    _need(row["sha256"] == hashlib.sha256(b"").hexdigest(), "signed empty source digest differs")
    descriptors = []
    try:
        fd, named, parent = _open_anchored_regular(path.parent, path.name)
        descriptors.extend((fd, parent))
        before = os.fstat(fd)
        _need(_identity(before) == _identity(named) and stat.S_ISREG(before.st_mode)
              and before.st_uid == os.geteuid() and before.st_nlink == 1
              and before.st_size == 0 and stat.S_IMODE(before.st_mode) == mode,
              "empty source file custody differs")
        _need(os.pread(fd, 1, 0) == b"", "empty source file grew during verification")
        _need(_identity(before) == _identity(os.fstat(fd))
              == _identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False)),
              "empty source file changed during verification")
        reopened, named, reopened_parent = _open_anchored_regular(path.parent, path.name)
        descriptors.extend((reopened, reopened_parent))
        _need(_identity(before) == _identity(named) == _identity(os.fstat(reopened))
              and os.pread(reopened, 1, 0) == b""
              and _identity(before) == _identity(os.fstat(reopened))
              == _identity(os.stat(path.name, dir_fd=reopened_parent, follow_symlinks=False)),
              "empty source file path changed during verification")
    finally:
        for fd in descriptors:
            os.close(fd)


def _verify_tree_files(root, manifest):
    expected_files = {".git/" + name for name in _git_metadata(manifest["commit"])} | {".git/index"}
    expected_dirs = {".", ".git", ".git/objects", ".git/objects/info", ".git/objects/pack", ".git/refs/tags"}
    pack_dir = root / ".git/objects/pack"
    packs = sorted(pack_dir.iterdir())
    _need(len(packs) == 3 and {path.suffix for path in packs} == {".pack", ".idx", ".rev"}
          and len({path.stem for path in packs}) == 1 and re.fullmatch(r"pack-[0-9a-f]{40}", packs[0].stem),
          "source object database is not exactly one complete pack")
    pack_path = next(path for path in packs if path.suffix == ".pack")
    pack_pin = stable_hash_path(pack_path, max_size=MAX_PACK_BYTES)
    _need(pack_pin.sha256 == manifest["pack"]["sha256"] and pack_pin.size == manifest["pack"]["size"], "imported pack differs")
    with stable_open_relative(pack_path.parent, pack_path.name, expected=pack_pin) as fd:
        _validate_pack(fd, manifest)
    expected_files.update(str(path.relative_to(root)) for path in packs)
    objects = {row["object"]: row for row in manifest["objects"]}
    source_paths = {row["path"]: row["mode"] for row in manifest["entries"]}
    for entry in manifest["entries"]:
        path = root / entry["path"]
        info = path.lstat()
        _need(info.st_uid == os.geteuid(), "source entry owner differs")
        if entry["mode"] == "160000":
            expected_dirs.add(entry["path"])
            _need(stat.S_ISDIR(info.st_mode) and not any(path.iterdir()), "source gitlink must remain an empty direct directory")
        elif entry["mode"] == "120000":
            expected_files.add(entry["path"])
            _need(stat.S_ISLNK(info.st_mode) and info.st_nlink == 1, "source symlink custody differs")
            payload = os.fsencode(os.readlink(path))
            _link_target(entry["path"], payload, source_paths)
            _need(not os.path.lexists(path.parent / os.readlink(path)), "source symlink referent is present")
            row = objects[entry["object"]]
            _need(len(payload) == row["size"] and hashlib.sha256(payload).hexdigest() == row["sha256"], "source symlink bytes differ")
        else:
            expected_files.add(entry["path"])
            row = objects[entry["object"]]
            _verify_source_file(path, row, 0o755 if entry["mode"] == "100755" else 0o644)
    for name in list(expected_files | expected_dirs):
        expected_dirs.update(str(parent) for parent in PurePosixPath(name).parents)
    actual_files, actual_dirs = set(), {"."}
    for current, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = Path(current) / name
            relative = str(path.relative_to(root))
            info = path.lstat()
            _need(info.st_uid == os.geteuid() and not info.st_mode & 0o022 if not stat.S_ISLNK(info.st_mode)
                  else info.st_uid == os.geteuid(), "imported source custody differs")
            if stat.S_ISDIR(info.st_mode):
                _need(stat.S_IMODE(info.st_mode) == 0o755, "imported source directory mode differs")
                actual_dirs.add(relative)
            else:
                _need(stat.S_ISLNK(info.st_mode) or (stat.S_ISREG(info.st_mode) and info.st_nlink == 1), "imported source file custody differs")
                actual_files.add(relative)
    _need(actual_files == expected_files and actual_dirs == expected_dirs, "imported source has missing or extra paths")
    for name, payload in _git_metadata(manifest["commit"]).items():
        info, actual = stable_read_path(root / ".git" / name, max_size=4096)
        _need(actual == payload and info.mode == 0o644, "imported Git control bytes/mode differ")
    for path in packs:
        info = stable_hash_path(path, max_size=MAX_PACK_BYTES)
        _need(info.mode == 0o444, "imported Git pack mode differs")
        if path.suffix == ".pack":
            _need(info.sha256 == manifest["pack"]["sha256"] and info.size == manifest["pack"]["size"], "imported pack differs")
        if path.suffix == ".idx":
            _git(root, "verify-pack", str(path))
    _need(stable_hash_path(root / ".git/index", max_size=MAX_MANIFEST_BYTES).mode in {0o600, 0o644}, "imported Git index mode differs")
    expected_index = b"".join(f"{row['mode']} {row['object']} 0\t{row['path']}\0".encode() for row in manifest["entries"])
    _need(_git(root, "ls-files", "--stage", "-z") == expected_index, "imported index differs from signed tree")
    _need(not _git(root, "status", "--porcelain=v1", "--untracked-files=all"), "imported source is not clean")


def _facts(root, manifest):
    return {"commit": manifest["commit"], "tree": manifest["tree"],
            "signer_fingerprint": manifest["signer_fingerprint"], "source_root": str(root),
            "clean": True, "signature_verified": True, "object_inventory_verified": True,
            "history_included": False, "runtime_files_transferred": False,
            "runtime_files_included": False, "activated": False,
            "sha256": manifest["pack"]["sha256"], "size": manifest["pack"]["size"],
            "source_bytes": manifest["source_bytes"], "file_count": len(manifest["entries"])}


def verify_import(source_root: Path, manifest_path: Path, expected_commit: str,
                  expected_tree: str, expected_signer: str) -> dict:
    """Read-only reauthentication of an exact completed deployment source artifact."""
    root = _directory(source_root, mode=0o755)
    before = _identity(root.lstat())
    manifest, key, pin = _manifest(manifest_path, expected_commit, expected_tree, expected_signer)
    _verify_tree_files(root, manifest)
    _verify_objects(root, manifest)
    _verify_signature(root, expected_commit, expected_signer, key)
    _verify_tree_files(root, manifest)
    _need(stable_hash_path(manifest_path, max_size=MAX_MANIFEST_BYTES) == pin
          and _identity(root.lstat()) == before, "source changed during verification")
    return _facts(root, manifest)


def import_source(pack_path: Path, manifest_path: Path, expected_commit: str,
                  expected_tree: str, expected_signer: str, source_root: Path) -> dict:
    """Verify and exclusively publish one source artifact; retain failures for diagnosis."""
    manifest, key, manifest_pin = _manifest(manifest_path, expected_commit, expected_tree, expected_signer)
    pack_path = _absolute(pack_path)
    pack_pin = stable_hash_path(pack_path, max_size=MAX_PACK_BYTES)
    _need(pack_pin.sha256 == manifest["pack"]["sha256"] and pack_pin.size == manifest["pack"]["size"], "source pack transport digest differs")
    stage = _stage(source_root)
    git_root = stage / ".git"
    for name in ("objects/info", "objects/pack", "refs/tags"):
        _mkdir_parents(git_root / name, stage)
    for name, payload in _git_metadata(expected_commit).items():
        path = git_root / name
        _mkdir_parents(path.parent, stage)
        exclusive_write_bytes(path, payload, mode=0o644)
    with stable_open_relative(pack_path.parent, pack_path.name, expected=pack_pin) as fd:
        _validate_pack(fd, manifest)
        _git(stage, "index-pack", "--stdin", "--rev-index", input_fd=fd)
    _freeze_new_pack(stage)
    _verify_objects(stage, manifest)
    _verify_signature(stage, expected_commit, expected_signer, key)
    _materialize(stage, manifest)
    _git(stage, "read-tree", expected_commit)
    stage.chmod(0o755)
    verify_import(stage, manifest_path, expected_commit, expected_tree, expected_signer)
    _need(stable_hash_path(pack_path, max_size=MAX_PACK_BYTES) == pack_pin
          and stable_hash_path(manifest_path, max_size=MAX_MANIFEST_BYTES) == manifest_pin,
          "source input changed before publication")
    _publish(stage, Path(source_root))
    return _facts(Path(source_root), manifest)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="operation", required=True)
    for name in ("export", "import", "verify"):
        command = commands.add_parser(name)
        for field in ("commit", "tree", "signer"):
            command.add_argument("--expected-" + field, required=True)
        if name == "export":
            command.add_argument("--repo-root", type=Path, required=True)
            command.add_argument("--output-dir", type=Path, required=True)
        else:
            command.add_argument("--manifest", type=Path, required=True)
            command.add_argument("--source-root", type=Path, required=True)
            if name == "import":
                command.add_argument("--pack", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.operation == "export":
            result = export_source(args.repo_root, args.expected_commit, args.expected_tree, args.expected_signer, args.output_dir)
        elif args.operation == "import":
            result = import_source(args.pack, args.manifest, args.expected_commit, args.expected_tree, args.expected_signer, args.source_root)
        else:
            result = verify_import(args.source_root, args.manifest, args.expected_commit, args.expected_tree, args.expected_signer)
        sys.stdout.buffer.write(canonical_json_bytes(result))
        return 0
    except (ReleaseArtifactError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"taira source capture refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
