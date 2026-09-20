#!/usr/bin/env python3
"""Import one prepared Taira release into fresh, inactive custody on the approved guest.

Requires Python 3.11+, local Git/GPG and the explicitly pinned MacStadium SSH
routes. The Linux receiver requires root, Git, GPG and an existing private runtime
root. Only signed public source, four executables and public receipts cross SSH.
No service, deployment, runtime credential or activation command is exposed.
See docs/source/taira_release_transfer.md for the closed public plan.
"""
from __future__ import annotations

import argparse
import base64
import contextlib
import fcntl
import hashlib
import importlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import select
import shlex
import shutil
import stat
import struct
import subprocess
import sys
import time
import threading
import types

sys.dont_write_bytecode = True
SCHEMA = "taira.release-transfer.v1"
RESULT_SCHEMA = "taira.release-transfer.completed.v1"
NAMES = ("iroha3d_taira", "iroha", "sorafs-node", "kagami")
PACKAGES = ("irohad", "iroha_cli", "sorafs_node", "iroha_kagami")
PROOF_NAMES = tuple("preparation/" + name for name in ("result.json", "request.json", "checks.json", "capture.json"))
PAYLOAD_NAMES = (*NAMES, "source.pack", "source-capture.json", *PROOF_NAMES)
MAX_BINARY = 512 * 1024**2
MAX_PROOF = 16 * 1024**2
MAX_BOOTSTRAP = 4 * 1024**2
MAX_REPORT = 1024**2
CHUNK = 1024**2
TIMEOUT = 1800
HEADROOM = 2 * 1024**3
MODULES = ("release_artifact_contract", "taira_disk_capacity", "taira_source_capture",
           "taira_release_transfer")
CONTROLLERS = ("release_artifact_contract", "taira_cargo_cache", "taira_cargo_artifact",
               "taira_disk_capacity", "taira_source_capture", "taira_retry",
               "taira_release", "taira_release_transfer")
BASE_FIELDS = frozenset("commit signer_fingerprint native_check_scope native_incremental native_linker "
    "environment_sha256 native_environment_sha256 tree target profile jobs source_unchanged "
    "toolchain_unchanged source_snapshot_sha256 source_root source_output_target compiler_tools "
    "tools command release_qualified deployed".split())


class TransferError(ValueError):
    """Custody or transport admission failed; retain the partial evidence."""


def need(value, message):
    if not value:
        raise TransferError(message)


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode()


def decode(raw):
    def reject_constant(value):
        raise TransferError("non-finite JSON value")
    def unique(items):
        result = {}
        for key, value in items:
            need(key not in result, "duplicate JSON field")
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique, parse_constant=reject_constant)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def digest(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def direct(value):
    need(isinstance(value, (str, Path)), "absolute direct path required")
    path = Path(value)
    need(path.is_absolute() and str(path) == str(value) and path == Path(os.path.normpath(path))
         and not any(ord(c) < 32 for c in str(path)) and path.resolve() == path,
         "absolute direct path required")
    return path


def identity(info):
    return tuple(getattr(info, key) for key in ("st_dev", "st_ino", "st_mode", "st_uid",
        "st_gid", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns"))


def private_directory(path, *, mode=0o700):
    path = direct(path)
    for index, ancestor in enumerate((path, *path.parents)):
        info = ancestor.lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_uid in (0, os.geteuid())
             and not info.st_mode & 0o022, "unsafe directory ancestry")
        if index == 0:
            need(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == mode,
                 "directory must have its exact private owner/mode")
    return path


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def fresh_directory(path):
    private_directory(path.parent)
    path.mkdir(mode=0o700)
    private_directory(path)
    sync_directory(path.parent)
    return path


@contextlib.contextmanager
def pinned(path, *, expected=None, maximum=MAX_REPORT, mode=None):
    path = direct(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
             and before.st_nlink == 1 and not before.st_mode & 0o022
             and before.st_size <= maximum and identity(before) == identity(path.lstat()),
             "unsafe input custody")
        if mode is not None:
            need(stat.S_IMODE(before.st_mode) == mode, "input mode differs")
        def check_bytes():
            hasher, offset = hashlib.sha256(), 0
            while offset < before.st_size:
                data = os.pread(fd, min(CHUNK, before.st_size - offset), offset)
                need(data, "input truncated")
                hasher.update(data)
                offset += len(data)
            need(not os.pread(fd, 1, offset), "input grew")
            return hasher.hexdigest()
        observed = check_bytes()
        need(expected is None or observed == expected, "input digest differs")
        yield fd, before.st_size, observed
        need(identity(before) == identity(os.fstat(fd)) == identity(path.lstat())
             and check_bytes() == observed, "input changed during use")
    finally:
        os.close(fd)


def read(path, expected=None, *, maximum=MAX_REPORT, mode=None):
    with pinned(path, expected=expected, maximum=maximum, mode=mode) as (fd, size, _):
        raw = os.pread(fd, size + 1, 0)
        need(len(raw) == size, "input changed while reading")
        return raw


def write_new(path, raw, *, mode=0o400):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        view = memoryview(raw)
        while view:
            count = os.write(fd, view)
            need(count > 0, "write made no progress")
            view = view[count:]
        os.fchmod(fd, mode)
        os.fsync(fd)
    finally:
        os.close(fd)
    sync_directory(path.parent)


def validate_plan(value):
    need(isinstance(value, dict) and set(value) == {"schema", "preparation", "expected_commit",
         "expected_signer", "guest_ssh", "backing_ssh", "backing_path", "runtime_root", "provider"}
         and value["schema"] == SCHEMA and value["provider"] == "macstadium-dublin",
         "exact MacStadium transfer plan required")
    need(isinstance(value["expected_commit"], str) and isinstance(value["expected_signer"], str)
         and re.fullmatch(r"[0-9a-f]{40}", value["expected_commit"]) is not None
         and re.fullmatch(r"(?:[0-9A-F]{40}|[0-9A-F]{64})", value["expected_signer"]) is not None,
         "full commit and GPG signer fingerprint required")
    reference = value["preparation"]
    need(isinstance(reference, dict) and set(reference) == {"path", "sha256"}
         and digest(reference["sha256"]), "preparation path/digest required")
    for path in (reference["path"], value["runtime_root"], value["backing_path"]):
        # Remote paths are normalized here, but resolved only on their actual host.
        need(isinstance(path, str) and Path(path).is_absolute() and str(Path(path)) == path
             and path == os.path.normpath(path) and not any(ord(c) < 32 for c in path),
             "normalized absolute plan path required")
    route_text = json.dumps([value["guest_ssh"], value["backing_ssh"], value["backing_path"]]).lower()
    need(not any(word in route_text for word in ("vultr", "amazonaws", "aws.amazon")),
         "banned infrastructure is not an operational input")
    return value


def authenticated_modules(root, commit, signer):
    """Pin the complete executed local controller closure to the actual signed tree."""
    root = direct(root)
    environment = {key: os.environ[key] for key in ("PATH", "HOME", "GNUPGHOME") if key in os.environ}
    environment.update(LC_ALL="C", GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null")
    gpg = shutil.which("gpg", path=environment.get("PATH"))
    need(gpg is not None, "GnuPG is required")
    gpg = Path(gpg).resolve(strict=True)
    info = gpg.stat()
    need(stat.S_ISREG(info.st_mode) and not info.st_mode & 0o022 and os.access(gpg, os.X_OK),
         "GnuPG executable is unsafe")
    def git(*args):
        result = subprocess.run(["/usr/bin/git", "--no-replace-objects", "-c", "gpg.format=openpgp",
            "-c", "gpg.program=" + str(gpg), "-c", "gpg.openpgp.program=" + str(gpg), *args], cwd=root,
            stdin=subprocess.DEVNULL, capture_output=True, env=environment, timeout=60)
        need(result.returncode == 0, "signed source Git verification failed")
        return result.stdout
    need(git("rev-parse", "--show-toplevel").strip() == os.fsencode(root)
         and git("branch", "--show-current").strip() == b"optimizations", "canonical optimizations checkout required")
    git("verify-commit", commit)
    need(git("show", "--no-patch", "--format=%GF", commit).decode().strip() == signer,
         "actual commit signer differs")
    tree = git("rev-parse", commit + "^{tree}").decode().strip()
    modules = {}
    for name in CONTROLLERS:
        relative = "scripts/" + name + ".py"
        path = root / relative
        raw = read(path, maximum=2 * 1024**2)
        need(raw == git("show", commit + ":" + relative), "controller differs from signed source: " + name)
        modules[name] = raw
    return tree, modules, load_signed_modules(modules, root)


def load_signed_modules(modules, root):
    """Execute only the authenticated byte closure, never a cached/path-loaded controller."""
    need(set(modules) == set(CONTROLLERS), "controller byte closure differs")
    loaded = {}
    for name in CONTROLLERS:
        path = str(root / "scripts" / (name + ".py"))
        module = types.ModuleType(name)
        module.__file__ = path
        module.__spec__ = importlib.util.spec_from_loader(name, loader=None, origin=path)
        sys.modules[name] = module
        exec(compile(modules[name], path, "exec"), module.__dict__)
        loaded[name] = module
    return loaded


def admit_preparation(plan, loaded, tree):
    release = loaded["taira_release"]
    path = direct(plan["preparation"]["path"])
    need(path.name == "result.json", "preparation result.json required")
    output = private_directory(path.parent, mode=0o500)
    raw = read(path, plan["preparation"]["sha256"], maximum=16 * 1024**2, mode=0o400)
    result = decode(raw)
    need(release.canonical_json_bytes(result) == raw, "preparation result is not canonical")
    request = release.read_record(output / "request.json")
    need(set(request) == BASE_FIELDS | {"schema", "repo_root", "target_dir"}
         and request["schema"] == release.SESSION_SCHEMA, "current exact preparation request required")
    need(Path(request["repo_root"]) == Path(release.__file__).resolve().parents[1],
         "preparation belongs to a different checkout")
    need(release.read_record(output / "checks.json") == {"request": request, "passed": True},
         "completed native qualification checkpoint differs")
    base = {key: request[key] for key in BASE_FIELDS}
    need(base["commit"] == plan["expected_commit"] and base["tree"] == tree
         and base["signer_fingerprint"] == plan["expected_signer"]
         and base["target"] == "aarch64-unknown-linux-gnu" and base["profile"] == "release"
         and type(base["jobs"]) is int and base["jobs"] > 0
         and base["native_check_scope"] in ("basic", "full")
         and base["source_unchanged"] is True and base["toolchain_unchanged"] is True
         and base["release_qualified"] is False and base["deployed"] is False,
         "successful matching maintained preparation required")
    release.verify_capture(result, base, output)
    need(read(output / result["attempt"] / "capture.json", maximum=16 * 1024**2, mode=0o400) == raw,
         "completed capture differs from result")
    entries = release.commit_entries(Path(request["repo_root"]), plan["expected_commit"])
    snapshot = release.frozen_snapshot(Path(base["source_root"]), entries, Path(request["target_dir"]))
    need(sha(release.canonical_json_bytes(snapshot)) == base["source_snapshot_sha256"],
         "captured source snapshot differs from preparation")
    for row in result["artifacts"]:
        need(type(row["size"]) is int and 20 <= row["size"] <= MAX_BINARY, "native artifact size exceeds role bound")
        with pinned(row["path"], expected=row["sha256"], maximum=MAX_BINARY, mode=0o500) as (fd, size, _):
            need(size == row["size"] and release.valid_elf(os.pread(fd, 20, 0)), "prepared binary is not exact AArch64 ELF")
    return result


def import_name(request):
    return "release-import-" + request["commit"] + "-" + request["result_sha256"]


def allocation_plan(request, path, capacity):
    observed = capacity.inspect_filesystem(Path(path))
    bound = capacity.allocation_bound(request["allocation"]["bytes"], request["allocation"]["files"],
        request["allocation"]["directories"], observed["fragment_bytes"])
    return {"schema": capacity.PLAN_SCHEMA, "allocations": [
        {"path": path, "label": "complete inactive release import", **bound},
        {"path": path, "label": "filesystem operating headroom", "bytes": HEADROOM, "inodes": 1024}]}


def validate_request(request):
    need(isinstance(request, dict) and set(request) == {"schema", "commit", "tree", "signer_fingerprint",
        "result_sha256", "runtime_root", "rows", "allocation"} and request["schema"] == SCHEMA,
        "exact transfer request required")
    need(all(isinstance(request[key], str) for key in ("commit", "tree", "signer_fingerprint"))
         and re.fullmatch(r"[0-9a-f]{40}", request["commit"]) is not None
         and re.fullmatch(r"[0-9a-f]{40}", request["tree"]) is not None
         and re.fullmatch(r"(?:[0-9A-F]{40}|[0-9A-F]{64})", request["signer_fingerprint"]) is not None
         and digest(request["result_sha256"]), "invalid transfer source identity")
    need(isinstance(request["rows"], list) and len(request["rows"]) == 10, "ten exact payload rows required")
    for row, name, limit in zip(request["rows"], PAYLOAD_NAMES,
                                (*([MAX_BINARY] * 4), 4 * 1024**3, 64 * 1024**2, *([MAX_PROOF] * 4))):
        need(isinstance(row, dict) and set(row) == {"name", "size", "sha256"}
             and row["name"] == name and type(row["size"]) is int and 0 < row["size"] <= limit
             and digest(row["sha256"]), "invalid payload row")
    result, capture = request["rows"][6], request["rows"][9]
    need(result["sha256"] == request["result_sha256"]
         and (result["sha256"], result["size"]) == (capture["sha256"], capture["size"]),
         "preparation result/capture payload binding differs")
    allocation = request["allocation"]
    need(isinstance(allocation, dict) and set(allocation) == {"bytes", "files", "directories"}
         and all(type(value) is int and 0 < value < 2**50 for value in allocation.values())
         and allocation["bytes"] >= sum(row["size"] for row in request["rows"]),
         "invalid allocation bound")
    return request


@contextlib.contextmanager
def import_lock(runtime_root):
    root = private_directory(runtime_root)
    fd = os.open(root / ".release-import.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        info = os.fstat(fd)
        need(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and info.st_nlink == 1
             and stat.S_IMODE(info.st_mode) == 0o600
             and identity(info) == identity((root / ".release-import.lock").lstat()), "unsafe import lock")
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield root
    finally:
        os.close(fd)


def capacity_probe(request, capacity):
    validate_request(request)
    with import_lock(request["runtime_root"]) as root:
        destination = root / import_name(request)
        reuse = os.path.lexists(destination)
        if reuse:
            private_directory(destination)
            need(read(destination / "request.json", mode=0o400) == canonical(request),
                 "existing import request differs")
            completed = decode(read(destination / "completed.json", mode=0o400))
            need(set(completed) == {"schema", "request_sha256", "binary_transfer", "source_transfer", "activated"}
                 and completed["schema"] == RESULT_SCHEMA and completed["activated"] is False
                 and completed["request_sha256"] == sha(canonical(request)),
                 "existing import is incomplete or belongs to another request")
            # Existing bytes already consume space. The import operation refuses
            # every write on this branch and reauthenticates all contents before
            # reporting success; this read only classifies additional allocation.
            plan = {"schema": capacity.PLAN_SCHEMA, "allocations": [{
                "path": request["runtime_root"], "label": "filesystem operating headroom",
                "bytes": HEADROOM, "inodes": 1024}]}
        else:
            plan = allocation_plan(request, request["runtime_root"], capacity)
        return {"plan": plan, "observation": capacity.evaluate(plan), "reuse": reuse}


def payload_mode(name):
    # Retained public-import retirement owns the transport pack as0600.
    # Captured source manifests remain read-only; source exports are unchanged.
    return 0o755 if name in NAMES else 0o600 if name == "source.pack" else 0o400


def payload_path(destination, name):
    need(name in PAYLOAD_NAMES, "unknown payload role")
    if name in PROOF_NAMES:
        return destination / name
    return destination / ("artifacts/bin" if name in NAMES else "source") / name


def expected_receipts(request, destination, facts):
    need(all(facts.get(key) is True for key in ("clean", "signature_verified", "object_inventory_verified"))
         and facts.get("commit") == request["commit"] and facts.get("tree") == request["tree"]
         and facts.get("signer_fingerprint") == request["signer_fingerprint"]
         and facts.get("source_root") == str(destination / "source/source")
         and all(facts.get(key) is False for key in ("activated", "history_included",
                 "runtime_files_transferred", "runtime_files_included")),
         "source owner did not verify the exact signed source")
    binary = {"commit": request["commit"], "destination": str(destination / "artifacts/bin"),
        "artifacts": request["rows"][:4], "all_hashes_verified": True, "activated": False}
    source = {**facts, "commit": request["commit"], "tree": request["tree"],
        "source_root": str(destination / "source/source"), "result_sha256": request["result_sha256"],
        "activated": False, "runtime_files_transferred": False, "history_included": False}
    return binary, source


def verify_completed(request, destination, source_owner):
    for directory, names in (
        (destination, {"request.json", "artifacts", "source", "preparation", "completed.json"}),
        (destination / "artifacts", {"bin", "verified-manifest.json"}),
        (destination / "artifacts/bin", set(NAMES)),
        (destination / "source", {"source.pack", "source-capture.json", "source", "verified-manifest.json"}),
        (destination / "preparation", {"result.json", "request.json", "checks.json", "capture.json"}),
    ):
        private_directory(directory)
        need(set(os.listdir(directory)) == names, "completed import has missing or unexpected entries")
    need(read(destination / "request.json", mode=0o400) == canonical(request), "existing import request differs")
    for row in request["rows"]:
        mode = payload_mode(row["name"])
        with pinned(payload_path(destination, row["name"]), expected=row["sha256"],
                    maximum=row["size"], mode=mode) as (_, size, _):
            need(size == row["size"], "retained payload size differs")
    facts = source_owner.verify_import(destination / "source/source", destination / "source/source-capture.json",
        request["commit"], request["tree"], request["signer_fingerprint"])
    binary, source = expected_receipts(request, destination, facts)
    need(read(destination / "artifacts/verified-manifest.json", mode=0o400) == canonical(binary)
         and read(destination / "source/verified-manifest.json", maximum=64 * 1024**2, mode=0o400) == canonical(source),
         "retained transfer receipt differs")
    completed = {"schema": RESULT_SCHEMA, "request_sha256": sha(canonical(request)),
        "binary_transfer": {"path": str(destination / "artifacts/verified-manifest.json"), "sha256": sha(canonical(binary))},
        "source_transfer": {"path": str(destination / "source/verified-manifest.json"), "sha256": sha(canonical(source))},
        "activated": False}
    need(read(destination / "completed.json", mode=0o400) == canonical(completed), "completion receipt differs")
    return {**completed, "binary": binary, "source": source}


def receive(request, stream, source_owner, capacity):
    """Consume the fixed bounded stream; publish receipts only after complete verification."""
    validate_request(request)
    with import_lock(request["runtime_root"]) as root:
        destination = root / import_name(request)
        if os.path.lexists(destination):
            private_directory(destination)
            # A repeated import still consumes and authenticates its exact stream;
            # it never repairs or overwrites retained bytes.
            completed = verify_completed(request, destination, source_owner)
            for row in request["rows"]:
                remaining, hasher = row["size"], hashlib.sha256()
                while remaining:
                    data = stream.read(min(CHUNK, remaining))
                    need(data and len(data) <= remaining, "truncated repeated transfer")
                    remaining -= len(data)
                    hasher.update(data)
                need(hasher.hexdigest() == row["sha256"], "repeated transfer digest differs")
            need(not stream.read(1), "unexpected trailing transfer bytes")
            return verify_completed(request, destination, source_owner)
        observation = capacity_probe_unlocked(request, capacity)
        need(observation["observation"]["passed"] is True, "insufficient guest capacity before import")
        fresh_directory(destination)
        write_new(destination / "request.json", canonical(request))
        fresh_directory(destination / "artifacts")
        fresh_directory(destination / "artifacts/bin")
        fresh_directory(destination / "source")
        fresh_directory(destination / "preparation")
        for row in request["rows"]:
            path = payload_path(destination, row["name"])
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            try:
                remaining, hasher = row["size"], hashlib.sha256()
                while remaining:
                    data = stream.read(min(CHUNK, remaining))
                    need(data and len(data) <= remaining, "truncated transfer payload")
                    hasher.update(data)
                    remaining -= len(data)
                    view = memoryview(data)
                    while view:
                        count = os.write(fd, view)
                        need(count > 0, "payload write made no progress")
                        view = view[count:]
                need(hasher.hexdigest() == row["sha256"], "transferred payload digest differs")
                os.fchmod(fd, payload_mode(row["name"]))
                os.fsync(fd)
            finally:
                os.close(fd)
            sync_directory(path.parent)
        need(not stream.read(1), "unexpected trailing transfer bytes")
        source_owner.import_source(destination / "source/source.pack", destination / "source/source-capture.json",
            request["commit"], request["tree"], request["signer_fingerprint"], destination / "source/source")
        facts = source_owner.verify_import(destination / "source/source", destination / "source/source-capture.json",
            request["commit"], request["tree"], request["signer_fingerprint"])
        binary, source = expected_receipts(request, destination, facts)
        # Rehash all received inputs after source processing, before success publication.
        for row in request["rows"]:
            with pinned(payload_path(destination, row["name"]), expected=row["sha256"], maximum=row["size"],
                        mode=payload_mode(row["name"])) as (_, size, _):
                need(size == row["size"], "received payload changed before publication")
        write_new(destination / "artifacts/verified-manifest.json", canonical(binary))
        write_new(destination / "source/verified-manifest.json", canonical(source))
        completed = {"schema": RESULT_SCHEMA, "request_sha256": sha(canonical(request)),
            "binary_transfer": {"path": str(destination / "artifacts/verified-manifest.json"), "sha256": sha(canonical(binary))},
            "source_transfer": {"path": str(destination / "source/verified-manifest.json"), "sha256": sha(canonical(source))},
            "activated": False}
        write_new(destination / "completed.json", canonical(completed))
        return verify_completed(request, destination, source_owner)


def capacity_probe_unlocked(request, capacity):
    plan = allocation_plan(request, request["runtime_root"], capacity)
    return {"plan": plan, "observation": capacity.evaluate(plan)}


class DeadlineReader:
    """Bound receiver stdin independently of the sender's SSH lifetime."""
    def __init__(self, stream, timeout):
        self.fd = stream.fileno()
        self.deadline = time.monotonic() + timeout

    def read(self, count):
        remaining = self.deadline - time.monotonic()
        need(remaining > 0 and select.select([self.fd], [], [], remaining)[0],
             "receiver stream deadline expired")
        return os.read(self.fd, min(count, CHUNK))


def remote_entry(envelope, stream):
    """Only these three maintained operations are reachable through the fixed bootstrap."""
    capacity = importlib.import_module("taira_disk_capacity")
    operation = envelope["operation"]
    stream = DeadlineReader(stream, TIMEOUT if operation == "import" else 60)
    if operation != "import":
        need(not stream.read(1), "capacity operation has unexpected payload")
    need(set(envelope) == ({"operation", "path", "required", "modules"}
         if operation == "backing-capacity" else {"operation", "request", "modules"}),
         "remote operation fields differ")
    if operation == "backing-capacity":
        need(sys.platform == "darwin", "backing route must be the approved Mac")
        path = direct(envelope["path"])
        need(path.is_dir(), "backing directory is absent")
        plan = {"schema": capacity.PLAN_SCHEMA, "allocations": [
            {"path": str(path), "label": "guest complete allocation and Mac reserve",
             "bytes": envelope["required"] + HEADROOM, "inodes": 1024}]}
        return capacity.evaluate(plan)
    need(sys.platform == "linux" and platform.machine() in ("aarch64", "arm64")
         and os.geteuid() == 0, "import receiver requires AArch64 Linux root")
    if operation == "guest-capacity":
        return capacity_probe(envelope["request"], capacity)
    need(operation == "import", "unknown transfer operation")
    return receive(envelope["request"], stream, importlib.import_module("taira_source_capture"), capacity)


# A constant command reads one bounded authenticated module envelope, then leaves
# stdin available to the length-bounded binary protocol. No user command is accepted.
BOOTSTRAP = '''import sys,os,struct,json,base64,hashlib,types,select,time
deadline=time.monotonic()+60
def exact(n):
 parts=[]
 while n:
  left=deadline-time.monotonic()
  assert left>0 and select.select([0],[],[],left)[0], "bootstrap deadline expired"
  b=os.read(0,min(n,1048576));assert b, "truncated bootstrap"
  parts.append(b);n-=len(b)
 return b"".join(parts)
n=struct.unpack(">I",exact(4))[0]
assert 0<n<=4194304
raw=exact(n)
e=json.loads(raw)
for name in ("release_artifact_contract","taira_disk_capacity","taira_source_capture","taira_release_transfer"):
 r=e["modules"][name]; b=base64.b64decode(r["source"],validate=True)
 assert hashlib.sha256(b).hexdigest()==r["sha256"]
 m=types.ModuleType(name);m.__file__="<signed-transfer>/"+name+".py";sys.modules[name]=m
 exec(compile(b,m.__file__,"exec"),m.__dict__)
result=sys.modules["taira_release_transfer"].remote_entry(e,sys.stdin.buffer)
print(json.dumps(result,sort_keys=True,separators=(",",":")))
'''
REMOTE_COMMAND = "/usr/bin/python3 -I -c " + shlex.quote(BOOTSTRAP)


def remote_call(route, envelope, modules, output, retry, payloads=()):
    argv = list(retry.validate_ssh(route))
    argv[-1] = REMOTE_COMMAND
    envelope = {**envelope, "modules": {name: {"source": base64.b64encode(modules[name]).decode(),
                                              "sha256": sha(modules[name])} for name in MODULES}}
    raw = canonical(envelope)
    need(len(raw) <= MAX_BOOTSTRAP, "remote bootstrap exceeds bound")
    started = time.monotonic()
    timeout = TIMEOUT if envelope["operation"] == "import" else 60
    with (output.with_suffix(".stdout")).open("xb") as stdout, (output.with_suffix(".stderr")).open("xb") as stderr:
        os.chmod(stdout.name, 0o600)
        os.chmod(stderr.name, 0o600)
        with subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=stdout, stderr=stderr,
                              env={key: os.environ[key] for key in ("PATH", "HOME") if key in os.environ}, umask=0o077) as process:
            def send(data):
                view = memoryview(data)
                while view:
                    need(time.monotonic() - started < timeout, "transfer deadline expired")
                    _, ready, _ = select.select([], [process.stdin], [], min(5, timeout))
                    need(os.fstat(stdout.fileno()).st_size <= MAX_REPORT
                         and os.fstat(stderr.fileno()).st_size <= MAX_REPORT,
                         "remote diagnostics exceed bound")
                    if not ready:
                        continue
                    try:
                        count = os.write(process.stdin.fileno(), view[:CHUNK])
                    except BlockingIOError:
                        continue
                    need(count > 0, "transport made no progress")
                    view = view[count:]
            os.set_blocking(process.stdin.fileno(), False)
            try:
                send(struct.pack(">I", len(raw)) + raw)
                for row, path in payloads:
                    with pinned(path, expected=row["sha256"], maximum=row["size"]) as (fd, size, _):
                        need(size == row["size"], "sender payload size differs")
                        offset = 0
                        while offset < size:
                            data = os.pread(fd, min(CHUNK, size - offset), offset)
                            need(data, "sender payload truncated")
                            send(data)
                            offset += len(data)
                process.stdin.close()
                while process.poll() is None:
                    need(time.monotonic() - started < timeout, "transfer deadline expired")
                    need(os.fstat(stdout.fileno()).st_size <= MAX_REPORT
                         and os.fstat(stderr.fileno()).st_size <= MAX_REPORT,
                         "remote diagnostics exceed bound")
                    try:
                        process.wait(timeout=min(5, max(0.01, timeout - (time.monotonic() - started))))
                    except subprocess.TimeoutExpired:
                        continue
                code = process.returncode
            except BaseException:
                if not process.stdin.closed:
                    process.stdin.close()
                # Only this invocation's SSH child is stopped on a bounded failure.
                process.kill()
                process.wait()
                raise
            need(code == 0, "remote transfer stopped; retain its stderr and partial import")
    result_raw = read(output.with_suffix(".stdout"), maximum=MAX_REPORT)
    return decode(result_raw)


def preparation_payloads(plan, build):
    """Carry the already-qualified producer evidence, with no separate operator upload."""
    from release_artifact_contract import canonical_json_bytes
    output = direct(plan["preparation"]["path"]).parent
    private_directory(output, mode=0o500)
    attempt = build.get("attempt")
    need(isinstance(attempt, str) and re.fullmatch(r"attempts/[0-9]{6}", attempt),
         "invalid completed preparation attempt")
    private_directory(output / attempt, mode=0o500)
    paths = (output / "result.json", output / "request.json", output / "checks.json",
             output / attempt / "capture.json")
    raw = [read(path, maximum=MAX_PROOF, mode=0o400) for path in paths]
    records = [decode(value) for value in raw[:3]]
    need(all(canonical_json_bytes(record) == value for record, value in zip(records, raw)),
         "preparation proof is not canonical")
    result, request, checks = records
    need(result == build and sha(raw[0]) == plan["preparation"]["sha256"] and raw[3] == raw[0],
         "preparation result/capture changed before transport")
    need(isinstance(request, dict) and set(request) == BASE_FIELDS | {"schema", "repo_root", "target_dir"}
         and request["schema"] == "taira.local-preparation.v1"
         and all(request[key] == build[key] for key in BASE_FIELDS)
         and Path(request["repo_root"]) == Path(__file__).resolve().parents[1]
         and checks == {"request": request, "passed": True},
         "preparation request/checkpoint changed before transport")
    return [({"name": name, "size": len(value), "sha256": sha(value)}, path)
            for name, value, path in zip(PROOF_NAMES, raw, paths)]


def make_request(plan, build, exported):
    rows = [{key: row[key] for key in ("name", "size", "sha256")} for row in build["artifacts"]]
    payloads = [(row, Path(original["path"])) for row, original in zip(rows, build["artifacts"])]
    for name, path in (("source.pack", Path(exported["pack_path"])),
                       ("source-capture.json", Path(exported["manifest_path"]))):
        with pinned(path, maximum=4 * 1024**3) as (_, size, checksum):
            row = {"name": name, "size": size, "sha256": checksum}
        rows.append(row)
        payloads.append((row, path))
    proof_payloads = preparation_payloads(plan, build)
    rows.extend(row for row, _ in proof_payloads)
    payloads.extend(proof_payloads)
    source_bytes = exported["source_bytes"]
    source_files = exported["file_count"]
    objects = exported["object_count"]
    manifest = decode(read(exported["manifest_path"], exported["manifest_sha256"],
                           maximum=64 * 1024**2, mode=0o400))
    directories = {"."}
    for entry in manifest["entries"]:
        directories.update(str(parent) for parent in PurePosixPath(entry["path"]).parents)
        if entry["mode"] == "160000":
            directories.add(entry["path"])
    need(all(type(value) is int and value >= 0 for value in (source_bytes, source_files, objects)),
         "source owner allocation census missing")
    request = {"schema": SCHEMA, "commit": build["commit"], "tree": build["tree"],
        "signer_fingerprint": plan["expected_signer"], "result_sha256": plan["preparation"]["sha256"],
        "runtime_root": plan["runtime_root"], "rows": rows,
        "allocation": {"bytes": sum(row["size"] for row in rows) + 2 * source_bytes + 2 * rows[4]["size"]
                                + 2 * rows[5]["size"] + 128 * objects + 64 * 1024**2,
                       "files": 2 * source_files + len(PROOF_NAMES) + 256,
                       "directories": 2 * len(directories) + 1 + 256}}
    return validate_request(request), payloads


@contextlib.contextmanager
def progress(label):
    started, stopped = time.monotonic(), threading.Event()
    print(f"[taira-transfer] start {label}", flush=True)
    def heartbeat():
        while not stopped.wait(30):
            print(f"[taira-transfer] {label} running ({time.monotonic() - started:.0f}s)", flush=True)
    worker = threading.Thread(target=heartbeat, daemon=True)
    worker.start()
    try:
        yield
    finally:
        stopped.set()
        worker.join()
        print(f"[taira-transfer] finished {label} ({time.monotonic() - started:.1f}s)", flush=True)


def transfer(plan, root, output):
    plan = validate_plan(plan)
    with progress("signed controller admission"):
        tree, modules, loaded = authenticated_modules(root, plan["expected_commit"], plan["expected_signer"])
    return loaded["taira_release_transfer"].transfer_admitted(plan, root, output, tree, modules, loaded)


def transfer_admitted(plan, root, output, tree, modules, loaded):
    """Continue only in the driver freshly loaded from its authenticated signed bytes."""
    with progress("completed preparation admission"):
        build = admit_preparation(plan, loaded, tree)
    retry, source_owner = loaded["taira_retry"], loaded["taira_source_capture"]
    retry.validate_ssh(plan["guest_ssh"])
    retry.validate_ssh(plan["backing_ssh"])
    output = fresh_directory(direct(output))
    write_new(output / "plan.json", canonical(plan))
    with progress("signed source capture"):
        exported = source_owner.export_source(Path(root), plan["expected_commit"], tree,
                                             plan["expected_signer"], output / "source-capture")
    request, payloads = make_request(plan, build, exported)
    write_new(output / "request.json", canonical(request))
    preliminary = request["allocation"]["bytes"] + HEADROOM
    def backing(required, label):
        result = remote_call(plan["backing_ssh"], {"operation": "backing-capacity",
            "path": plan["backing_path"], "required": required}, modules, output / label, retry)
        need(result.get("schema") == "taira.disk-capacity.result.v1" and result.get("passed") is True
             and result.get("errors") == [], "insufficient physical backing capacity")
    backing(HEADROOM, "backing-first")
    guest = remote_call(plan["guest_ssh"], {"operation": "guest-capacity", "request": request},
                        modules, output / "guest-capacity", retry)
    need(type(guest.get("reuse")) is bool
         and guest.get("observation", {}).get("schema") == "taira.disk-capacity.result.v1"
         and guest["observation"].get("passed") is True and guest["observation"].get("errors") == [],
         "insufficient guest import capacity")
    allocations = loaded["taira_disk_capacity"].validate_plan(guest["plan"])
    need(all(row["path"] == request["runtime_root"] for row in allocations)
         and len(allocations) == (1 if guest["reuse"] else 2), "guest capacity path/census differs")
    required = sum(row["bytes"] for row in allocations)
    need(required >= (HEADROOM if guest["reuse"] else preliminary),
         "guest capacity omitted declared payload/reserve")
    backing(required, "backing-final")
    with progress("inactive guest import"):
        completed = remote_call(plan["guest_ssh"], {"operation": "import", "request": request},
                                modules, output / "import", retry, payloads)
    destination = Path(plan["runtime_root"]) / import_name(request)
    binary = {"commit": request["commit"], "destination": str(destination / "artifacts/bin"),
              "artifacts": request["rows"][:4], "all_hashes_verified": True, "activated": False}
    need(completed.get("schema") == RESULT_SCHEMA and completed.get("request_sha256") == sha(canonical(request))
         and completed.get("binary") == binary and completed.get("activated") is False,
         "receiver completion identity differs")
    retry.validate_artifact_receipts(build, completed["binary"], completed["source"])
    source = completed["source"]
    need(source.get("source_root") == str(destination / "source/source")
         and source.get("result_sha256") == request["result_sha256"]
         and source.get("signer_fingerprint") == request["signer_fingerprint"]
         and source.get("sha256") == request["rows"][4]["sha256"]
         and source.get("size") == request["rows"][4]["size"]
         and source.get("source_bytes") == exported["source_bytes"]
         and source.get("file_count") == exported["file_count"]
         and source.get("runtime_files_included") is False,
         "receiver source receipt differs from exact exported source")
    for key in ("binary", "source"):
        receipt = completed[key + "_transfer"]
        expected_path = destination / ("artifacts" if key == "binary" else "source") / "verified-manifest.json"
        need(receipt == {"path": str(expected_path), "sha256": sha(canonical(completed[key]))}, "receipt path/digest differs")
        write_new(output / (key + "-transfer.json"), canonical(completed[key]))
    write_new(output / "completed.json", canonical(completed))
    return completed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=Path, help="closed owner-private public transfer plan")
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output-dir", required=True, type=Path, help="fresh evidence directory under an existing private parent")
    args = parser.parse_args()
    os.umask(0o077)
    result = transfer(decode(read(args.plan, mode=0o600)), args.repo_root, args.output_dir)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (TransferError, OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"taira-release-transfer: {error}", file=sys.stderr)
        raise SystemExit(2) from None
