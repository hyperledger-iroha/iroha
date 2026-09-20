#!/usr/bin/env python3
"""Archive, then retire explicitly selected inactive Taira public executables.

Python 3.11+, Git/GPG locally; Python and root on the approved AArch64 Linux
guest. Only the four named executables of at most twelve pinned, rolled-back
releases are eligible. Source trees, transport packs, receipts, ledger, runtime
configuration and credentials are never archive/removal inputs. SSH uses the
existing strict pinned MacStadium routes. No service or deployment operation is
provided. HOME is preserved. See docs/source/taira_retained_release.md.
"""
from __future__ import annotations

import argparse
import ast
import base64
import contextlib
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
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
import types

sys.dont_write_bytecode = True
SCHEMA = "taira.retained-public-release.v1"
NAMES = ("iroha3d_taira", "iroha", "sorafs-node", "kagami")
MODULES = ("release_artifact_contract", "taira_disk_capacity", "taira_retry", "taira_retained_release")
MAX_RECORD = 8 * 1024**2
MAX_BINARY = 4 * 1024**3
MAX_TOTAL = 32 * 1024**3
MAX_FILES = 48
CHUNK = 1024**2
RESERVE = 256 * 1024**2
RETIRE_GUEST_RESERVE = 32 * 1024**2
RETIRE_BACKING_RESERVE = 32 * 1024**2
TIMEOUT = 3600
SUPERVISOR_ROOT = Path("/var/lib/taira-epoch-supervisor")
SUPERVISOR_UNIT = "iroha-taira-epoch-supervisor.service"


class RetainedReleaseError(ValueError):
    """Preserve every archive, intent and unresolved quarantine on failure."""


def need(value, reason):
    if not value:
        raise RetainedReleaseError(reason)


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode()


def decode(raw):
    def pairs(rows):
        value = {}
        for key, item in rows:
            need(key not in value, "duplicate JSON field")
            value[key] = item
        return value
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(RetainedReleaseError("nonfinite JSON")))


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def identity(info):
    return [getattr(info, key) for key in ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid",
            "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")]


def direct(value):
    path = Path(value)
    need(path.is_absolute() and str(path) == str(value) and str(path) == os.path.normpath(path)
         and not any(ord(c) < 32 for c in str(path)),
         "absolute direct path required")
    return path


def safe_directory(value, modes):
    path = direct(value)
    for index, parent in enumerate((path, *path.parents)):
        info = parent.lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_uid in (0, os.geteuid()) and not info.st_mode & 0o022,
             "unsafe directory ancestry")
        if not index:
            need(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) in modes,
                 "directory owner or mode differs")
    return path


def private_directory(value):
    return safe_directory(value, (0o700,))


def public_binary_directory(value):
    return safe_directory(value, (0o700, 0o755))


@contextlib.contextmanager
def anchored_directory(path):
    from release_artifact_contract import _open_absolute_directory
    fd, _, info = _open_absolute_directory(path, "retained public directory")
    try:
        yield fd
        other, _, after = _open_absolute_directory(path, "retained public directory")
        try:
            need(identity(info)[:2] == identity(after)[:2] == identity(os.fstat(fd))[:2],
                 "directory ancestry changed")
        finally:
            os.close(other)
    finally:
        os.close(fd)


def sync(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def fresh_directory(path):
    private_directory(path.parent)
    path.mkdir(mode=0o700)
    os.chmod(path, 0o700)
    private_directory(path)
    sync(path.parent)
    return path


def write_new(path, raw):
    need(len(raw) <= MAX_RECORD, "record exceeds its bound")
    private_directory(path.parent)
    temporary = path.with_name("." + path.name + ".pending")
    need(not os.path.lexists(path), "record final name already exists")
    with anchored_directory(path.parent) as directory:
        flags = os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
        try:
            fd = os.open(temporary.name, os.O_RDWR | os.O_CREAT | os.O_EXCL | flags, 0o600, dir_fd=directory)
            os.fchmod(fd, 0o600)
        except FileExistsError:
            fd = os.open(temporary.name, os.O_RDONLY | flags, dir_fd=directory)
        try:
            before = os.fstat(fd)
            need(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid() and before.st_nlink == 1
                 and stat.S_IMODE(before.st_mode) in (0o600, 0o400) and before.st_size <= len(raw),
                 "unsafe pending record custody")
            prefix = os.pread(fd, before.st_size + 1, 0)
            need(prefix == raw[:before.st_size] and len(prefix) == before.st_size,
                 "pending record differs from exact intended bytes")
            if before.st_size < len(raw):
                need(stat.S_IMODE(before.st_mode) == 0o600, "incomplete pending record was sealed")
                writable = os.open(temporary.name, os.O_RDWR | flags, dir_fd=directory)
                try:
                    need(identity(os.fstat(writable)) == identity(before), "pending record replaced before continuation")
                except BaseException:
                    os.close(writable)
                    raise
                os.close(fd)
                fd = writable
                os.lseek(fd, before.st_size, os.SEEK_SET)
                write_all(fd, raw[before.st_size:])
            need(os.fstat(fd).st_size == len(raw) and hash_fd(fd, len(raw)) == sha(raw), "pending record bytes changed")
            os.fsync(fd)
            os.fchmod(fd, 0o400)
            os.fsync(fd)
            rename_exclusive(temporary, path, fd, identity(os.fstat(directory))[:2])
        finally:
            os.close(fd)


def write_all(fd, raw):
    view = memoryview(raw)
    while view:
        count = os.write(fd, view)
        need(count > 0, "write made no progress")
        view = view[count:]


def hash_fd(fd, size):
    digest, offset = hashlib.sha256(), 0
    while offset < size:
        chunk = os.pread(fd, min(CHUNK, size - offset), offset)
        need(chunk, "file truncated")
        digest.update(chunk)
        offset += len(chunk)
    need(not os.pread(fd, 1, offset), "file grew")
    return digest.hexdigest()


@contextlib.contextmanager
def held(path, *, digest=None, size=None, mode=None, stamp=None, renamed=False, maximum=MAX_BINARY):
    path = direct(path)
    # Anchor every parent without following links before opening the selected file.
    from release_artifact_contract import _open_absolute_directory
    parent, _, parent_info = _open_absolute_directory(path.parent, "public executable parent")
    fd = -1
    try:
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=parent)
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid() and before.st_nlink == 1
             and not before.st_mode & 0o022 and 0 <= before.st_size <= maximum,
             "unsafe public file custody")
        need(mode is None or stat.S_IMODE(before.st_mode) == mode, "public file mode differs")
        need(size is None or before.st_size == size, "public file size differs")
        if stamp is not None:
            need(identity(before)[:8 if renamed else 9] == stamp[:8 if renamed else 9], "public file identity differs")
        expected = hash_fd(fd, before.st_size)
        need(digest is None or expected == digest, "public file digest differs")
        yield fd, before, expected
        need(identity(before) == identity(os.fstat(fd)) == identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False))
             and hash_fd(fd, before.st_size) == expected, "public file changed during use")
        with anchored_directory(path.parent) as reopened:
            need(identity(parent_info)[:2] == identity(os.fstat(reopened))[:2], "public parent changed")
    finally:
        if fd >= 0:
            os.close(fd)
        os.close(parent)


def read(path, digest=None, *, mode=None):
    with held(path, digest=digest, mode=mode, maximum=MAX_RECORD) as (fd, before, _):
        need(before.st_size <= MAX_RECORD, "public record exceeds its bound")
        raw = os.pread(fd, before.st_size + 1, 0)
        need(len(raw) == before.st_size, "public record changed")
        return raw


def reference(value):
    need(isinstance(value, dict) and set(value) == {"path", "sha256"}
         and isinstance(value["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", value["sha256"]),
         "exact public path/digest reference required")
    direct(value["path"])
    return value


def validate_controller(controller):
    need(isinstance(controller, dict) and set(controller) == {"commit", "signer"}
         and re.fullmatch(r"[0-9a-f]{40}", controller["commit"])
         and re.fullmatch(r"(?:[0-9A-F]{40}|[0-9A-F]{64})", controller["signer"]), "full controller commit and signer required")
    return controller


def validate_plan(plan):
    need(isinstance(plan, dict) and set(plan) == {"schema", "provider", "controller", "guest_ssh",
         "backing_ssh", "backing_path", "deployment", "current_inventory", "units", "releases"}
         and plan["schema"] == SCHEMA and plan["provider"] == "macstadium-dublin", "closed approved plan required")
    validate_controller(plan["controller"])
    reference(plan["deployment"])
    reference(plan["current_inventory"])
    direct(plan["backing_path"])
    need(isinstance(plan["units"], list) and len(plan["units"]) == 4, "four current unit pins required")
    for index, unit in enumerate(plan["units"], 1):
        reference(unit)
        need(unit["path"] == f"/etc/systemd/system/iroha3d-taira-validator-{index}.service", "canonical current unit path required")
    need(isinstance(plan["releases"], list) and 1 <= len(plan["releases"]) <= 12, "one to twelve explicit retained releases required")
    seen = set()
    for release in plan["releases"]:
        need(isinstance(release, dict) and set(release) == {"inventory", "terminal", "binary_manifest", "source_manifest"}, "exact retained release authority required")
        for ref in release.values():
            reference(ref)
        need(release["inventory"]["sha256"] not in seen, "duplicate retained inventory")
        seen.add(release["inventory"]["sha256"])
    need(len(canonical(plan)) <= MAX_RECORD, "plan exceeds its bound")
    return plan


def pin_json(ref):
    return decode(read(ref["path"], ref["sha256"]))


def overlaps(path, roots):
    path = Path(path)
    return any(path == Path(root) or path.is_relative_to(root) or Path(root).is_relative_to(path) for root in roots)


def deployment_projection(plan):
    value = pin_json(plan["deployment"])
    need(value["guest_ssh"] == plan["guest_ssh"] and value["backing_ssh"] == plan["backing_ssh"]
         and value["backing_path"] == plan["backing_path"], "deployment route binding differs")
    return {key: value[key] for key in ("runtime_root", "state_root", "config_root", "current", "roles")}


def unit_command(raw):
    prefix = "ExecStart=/usr/bin/python3 -c "
    lines = [line for line in raw.decode().splitlines() if line.startswith("ExecStart=")]
    need(len(lines) == 1 and lines[0].startswith(prefix), "current unit launcher differs")
    code = json.loads(lines[0][len(prefix):]).replace("%%", "%").replace("$$", "$")
    commands = [ast.literal_eval(node.value) for node in ast.parse(code).body
                if isinstance(node, ast.Assign) and len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name) and node.targets[0].id == "cmd"]
    need(len(commands) == 1, "one literal current daemon command required")
    return commands[0]


def systemd_fields(name, fields):
    child = subprocess.run(["/usr/bin/systemctl", "show", name, "--property=" + ",".join(fields)],
        stdin=subprocess.DEVNULL, capture_output=True, timeout=15)
    need(child.returncode in (0, 4) and len(child.stdout) <= 16384, "cannot observe current unit")
    pairs = [line.split("=", 1) for line in child.stdout.decode().splitlines()]
    need(all(len(row) == 2 for row in pairs) and len(pairs) == len(fields), "unit fields differ")
    result = dict(pairs)
    need(set(result) == set(fields), "unit field set differs")
    return result


def current_bindings(plan, deployment):
    """Read public authority and units only; private configs are protected by path."""
    current_path = Path(plan["current_inventory"]["path"])
    need(current_path.parent.parent == direct(deployment["runtime_root"])
         and re.fullmatch(r"assembly[1-9][0-9]*", current_path.parent.name)
         and current_path.name == "inventory.json", "current public inventory namespace required")
    inventory = pin_json(plan["current_inventory"])
    protected = {deployment["state_root"], deployment["config_root"],
                 str(Path(deployment["current"]["daemon"]).parent), inventory["revision"]["source_root"],
                 str(Path(deployment["runtime_root"]) / "journal-v1"), str(SUPERVISOR_ROOT)}
    for host in inventory["validators"] + [inventory["edge"]]:
        protected.update(host[key] for key in ("service_root", "state_root", "reset_guard"))
        protected.update(row["local_path"] for row in host["artifacts"])
    need(deployment["roles"] == [f"taira-validator-{i}" for i in range(1, 5)], "current role set differs")
    for index, ref in enumerate(plan["units"], 1):
        raw = read(ref["path"], ref["sha256"])
        expected = [deployment["current"]["daemon"], "--config",
                    str(Path(deployment["config_root"]) / f"taira-validator-{index}/current/config/config.toml"), "--sora"]
        need(unit_command(raw) == expected, "current daemon binding differs")
        fields = systemd_fields(Path(ref["path"]).name, ("FragmentPath", "DropInPaths", "Job"))
        need(fields == {"FragmentPath": ref["path"], "DropInPaths": "", "Job": ""}, "current unit has alternate or pending authority")
    # This is a revalidation observation, not a claim of continuously locked absence.
    need(not os.path.lexists(SUPERVISOR_ROOT), "supervisor authority appeared; retain quarantine")
    fields = systemd_fields(SUPERVISOR_UNIT, ("LoadState", "ActiveState", "MainPID", "Job"))
    need(fields == {"LoadState": "not-found", "ActiveState": "inactive", "MainPID": "0", "Job": ""},
         "supervisor absence observation changed")
    return protected


def authority_rows(plan, deployment):
    import taira_retry as retry
    runtime = direct(deployment["runtime_root"])
    protected = current_bindings(plan, deployment)
    protected.update(ref["path"] for release in plan["releases"] for ref in release.values())
    rows, identities = [], []
    for release in plan["releases"]:
        # Classify every authority path before opening any content. The owner
        # accepts only these public receipt namespaces, never runtime configs.
        assembly = Path(release["inventory"]["path"])
        match = re.fullmatch(r"assembly([1-9][0-9]*)", assembly.parent.name)
        need(match and assembly.parent.parent == runtime and assembly.name == "inventory.json",
             "public inventory namespace required")
        number = match.group(1)
        need(Path(release["binary_manifest"]["path"]) == runtime / f"release{number}/verified-manifest.json"
             and Path(release["source_manifest"]["path"]) == runtime / f"source-transfer{number}/verified-manifest.json"
             and Path(release["terminal"]["path"]).parent == runtime / "journal-v1/rolled-back"
             and re.fullmatch(r"[0-9a-f]{64}\.json", Path(release["terminal"]["path"]).name),
             "public receipt namespace required")
        values = {key: pin_json(ref) for key, ref in release.items()}
        inventory, terminal, binary, source = (values[key] for key in ("inventory", "terminal", "binary_manifest", "source_manifest"))
        revision = inventory["revision"]
        retry._retire_validate_terminal(inventory, terminal, expected_commit=revision["commit"], expected_deployment=inventory["deployment_id"])
        need(terminal["inventory_sha256"] == release["inventory"]["sha256"]
             and terminal["authorization_nonce"] == inventory["authorization_nonce"]
             and Path(release["terminal"]["path"]).parent == runtime / "journal-v1/rolled-back"
             and Path(release["terminal"]["path"]).stem == terminal["authorization_sha256"]
             and not os.path.lexists(runtime / "journal-v1" / (inventory["deployment_id"] + ".journal.json")),
             "retained release has no exact closed native rollback")
        need(binary.get("commit") == source.get("commit") == revision["commit"]
             and source.get("tree") == revision["tree"] and source.get("source_root") == revision["source_root"]
             and binary.get("all_hashes_verified") is True and binary.get("activated") is False
             and all(source.get(key) is True for key in ("clean", "signature_verified", "object_inventory_verified"))
             and all(source.get(key) is False for key in ("activated", "runtime_files_transferred", "runtime_files_included", "history_included")),
             "completed public transfer identity differs")
        bins = direct(binary["destination"])
        need(bins.is_relative_to(runtime) and bins.name == "bin"
             and Path(release["binary_manifest"]["path"]) == bins.parent / "verified-manifest.json", "binary namespace differs")
        need(len(binary["artifacts"]) == 4 and [row["name"] for row in binary["artifacts"]] == list(NAMES), "exact four binary roles required")
        roles = {"iroha3d": "iroha3d_taira", "iroha_cli": "iroha", "sorafs_node": "sorafs-node", "kagami": "kagami"}
        by_name = {row["name"]: row for row in binary["artifacts"]}
        admitted = set()
        for host in inventory["validators"] + [inventory["edge"]]:
            for row in host["artifacts"]:
                if row["role"] in roles:
                    name = roles[row["role"]]
                    need(row["local_path"] == str(bins / name) and row["sha256"] == by_name[name]["sha256"]
                         and row["size"] == by_name[name]["size"], "native artifact binding differs")
                    admitted.add(name)
        # The native runtime inventory consumes daemon, CLI and SoraFS only.
        # Kagami belongs to the pinned complete transfer receipt and signed
        # source identity; do not manufacture an absent native role for it.
        need(set(NAMES[:3]) <= admitted <= set(NAMES), "native inventory omits runtime binary role")
        identities.append({"commit": revision["commit"], "tree": revision["tree"]})
        for row in binary["artifacts"]:
            path = bins / row["name"]
            need(not overlaps(path, protected), "selected public binary overlaps retained authority or live binding")
            need(type(row["size"]) is int and 20 <= row["size"] <= MAX_BINARY
                 and re.fullmatch(r"[0-9a-f]{64}", row["sha256"]), "invalid binary size or hash")
            rows.append({"path": str(path), "size": row["size"], "sha256": row["sha256"], "mode": 0o755})
    need(len(rows) <= MAX_FILES and len({row["path"] for row in rows}) == len(rows)
         and sum(row["size"] for row in rows) <= MAX_TOTAL, "duplicate or oversized binary closure")
    return rows, identities


@contextlib.contextmanager
def authority_locks(deployment):
    """Borrow existing update/reset locks; never create absent lifecycle authority."""
    root = private_directory(deployment["runtime_root"])
    fds = []
    with contextlib.ExitStack() as stack:
        try:
            for path in (root / ".routine-update.lock", root / "journal-v1/public-reset.lock"):
                directory = stack.enter_context(anchored_directory(path.parent))
                fd = os.open(path.name, os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=directory)
                fds.append(fd)
                info = os.fstat(fd)
                need(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and info.st_nlink == 1
                     and stat.S_IMODE(info.st_mode) == 0o600 and identity(info) == identity(path.lstat()), "existing authority lock differs")
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                need(identity(info) == identity(os.fstat(fd)) == identity(os.stat(path.name, dir_fd=directory, follow_symlinks=False)),
                     "authority lock replaced during acquisition")
                with anchored_directory(path.parent) as reopened:
                    need(os.fstat(reopened).st_ino == os.fstat(directory).st_ino, "authority lock parent replaced")
            yield
        finally:
            for fd in reversed(fds):
                os.close(fd)

def no_live_references(paths, own_fds=(), *, file_identities=()):
    import taira_retry as retry
    result = retry._retire_live_references(paths, file_identities=file_identities, own_fds=own_fds)
    if not result["passed"]:
        # Persist selected-public-path evidence in the existing bounded stderr.
        # Never include process argv, environment, or unrelated mapped paths.
        rows = result["references"]
        details = []
        for row in rows[:8]:
            path = row["target_root"]
            details.append({"pid": row["pid"], "kind": row["kind"], "target_root": path[:256],
                            "target_root_truncated": len(path) > 256,
                            "target_root_sha256": sha(path.encode())})
        report = {"reference_count": len(rows), "omitted_references": max(0, len(rows) - 8),
                  "references": details}
        need(False, "selected public payload has a live process, inode, or mount reference: "
             + canonical(report).decode().rstrip("\n"))
    return result


def census(rows, *, allowed_quarantines=()):
    allowed = set(allowed_quarantines)
    parents = {Path(row["path"]).parent for row in rows}
    for parent in parents:
        public_binary_directory(parent)
        actual = set(os.listdir(parent))
        expected = {Path(row["path"]).name for row in rows if Path(row["path"]).parent == parent}
        expected.update(Path(path).name for path in allowed if Path(path).parent == parent)
        need(actual <= expected and actual >= expected - {Path(path).name for path in allowed}, "binary directory has unadmitted paths")


def inspect(plan, deployment, *, own_fds=()):
    rows, identities = authority_rows(plan, deployment)
    census(rows)
    for row in rows:
        with held(row["path"], digest=row["sha256"], size=row["size"], mode=0o755) as (_, info, _):
            row.update(identity=identity(info), allocated_bytes=info.st_blocks * 512)
            with anchored_directory(Path(row["path"]).parent) as directory:
                row["parent_identity"] = identity(os.fstat(directory))[:2]
    no_live_references([row["path"] for row in rows], own_fds, file_identities=rows)
    need(authority_rows(plan, deployment)[0] == [{key: row[key] for key in ("path", "size", "sha256", "mode")} for row in rows], "public authority changed")
    return {"schema": SCHEMA, "plan_sha256": sha(canonical(plan)), "deployment": deployment,
            "rows": rows, "sources": identities, "source_files_read": False, "runtime_files_read": False}


def send_frame(fd, value):
    raw = canonical(value)
    need(len(raw) <= MAX_RECORD, "frame exceeds its bound")
    write_all(fd, struct.pack(">I", len(raw)) + raw)


class Reader:
    def __init__(self, fd, timeout=TIMEOUT):
        self.fd, self.deadline = fd, time.monotonic() + timeout

    def exact(self, count):
        result = bytearray()
        while len(result) < count:
            remaining = self.deadline - time.monotonic()
            need(remaining > 0 and select.select([self.fd], [], [], remaining)[0], "stream deadline expired")
            part = os.read(self.fd, min(CHUNK, count - len(result)))
            need(part, "truncated archive stream")
            result.extend(part)
        return bytes(result)

    def frame(self):
        count = struct.unpack(">I", self.exact(4))[0]
        need(0 < count <= MAX_RECORD, "invalid frame size")
        return decode(self.exact(count))


def hash_prefix(fd, size):
    digest, offset = hashlib.sha256(), 0
    while offset < size:
        part = os.pread(fd, min(CHUNK, size - offset), offset)
        need(part, "archive prefix truncated")
        digest.update(part)
        offset += len(part)
    return digest.hexdigest()


def validate_resume(resume, rows):
    need(isinstance(resume, dict) and set(resume) == {"completed", "partial"}
         and type(resume["completed"]) is int and 0 <= resume["completed"] <= len(rows),
         "exact bounded archive resume descriptor required")
    partial = resume["partial"]
    if partial is not None:
        need(isinstance(partial, dict) and set(partial) == {"index", "size", "sha256"}
             and type(partial["index"]) is int and partial["index"] == resume["completed"] < len(rows)
             and type(partial["size"]) is int and 0 <= partial["size"] <= rows[partial["index"]]["size"]
             and isinstance(partial["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", partial["sha256"]),
             "exact final partial archive prefix required")
    return resume


def archive_stream(plan, deployment, expected, output_fd, *, resume=None):
    if resume is None:
        resume = {"completed": 0, "partial": None}
    validate_resume(resume, expected["rows"])
    with authority_locks(deployment), contextlib.ExitStack() as stack:
        admission = inspect(plan, deployment)
        need(admission == expected, "archive admission changed")
        opened = [stack.enter_context(held(row["path"], digest=row["sha256"], size=row["size"], mode=0o755,
                  stamp=row["identity"]))[0] for row in admission["rows"]]
        no_live_references([row["path"] for row in admission["rows"]], opened, file_identities=admission["rows"])
        partial = resume["partial"]
        if partial is not None:
            need(hash_prefix(opened[partial["index"]], partial["size"]) == partial["sha256"],
                 "local archive prefix differs from fresh held source")
        send_frame(output_fd, admission)
        for index, (row, fd) in enumerate(zip(admission["rows"], opened)):
            if index < resume["completed"]:
                continue
            offset = partial["size"] if partial is not None and index == partial["index"] else 0
            while offset < row["size"]:
                data = os.pread(fd, min(CHUNK, row["size"] - offset), offset)
                need(data, "archive source truncated")
                write_all(output_fd, data)
                offset += len(data)
        need(inspect(plan, deployment, own_fds=opened) == admission, "archive source or authority changed during streaming")
        send_frame(output_fd, {"archive_stream_verified": True, "admission_sha256": sha(canonical(admission))})


def allocation(path, payload, files, directories=0, *, reserve=RESERVE):
    import taira_disk_capacity as capacity
    observed = capacity.inspect_filesystem(Path(path))
    bound = capacity.allocation_bound(payload, files, directories, observed["fragment_bytes"])
    plan = {"schema": capacity.PLAN_SCHEMA, "allocations": [
        {"path": str(path), "label": "retained public archive or retirement allocation", **bound},
        {"path": str(path), "label": "retained public operation reserve", "bytes": reserve, "inodes": 256}]}
    result = capacity.evaluate(plan)
    need(result["passed"], "insufficient capacity for bounded retained public operation")
    return plan, result


def quarantine_path(row, token, index):
    return Path(row["path"]).with_name(f".retained-public-{token}-{index:04d}")


def retirement_intent(admission, archive_digest):
    need(1 <= len(admission["rows"]) <= MAX_FILES, "bounded retained binary census required")
    token = sha(canonical({"admission": admission, "archive": archive_digest}))
    rows = [{**row, "quarantine": str(quarantine_path(row, token, index))} for index, row in enumerate(admission["rows"])]
    value = {"schema": SCHEMA, "operation": "retire-public-binaries", "archive_sha256": archive_digest,
             "admission_sha256": sha(canonical(admission)), "token": token, "rows": rows}
    need(len(canonical(value)) <= MAX_RECORD, "retirement intent exceeds its bound")
    return value


def retirement_capacity(admission, archive_digest):
    raw = canonical(retirement_intent(admission, archive_digest))
    # Exact frozen intent plus two simultaneous publication copies, per-file
    # quarantine/delete receipts (each capped at 4KiB), final evidence and dirs.
    payload = 3 * len(raw) + (3 * len(admission["rows"]) + 8) * 4096
    plan, result = allocation(admission["deployment"]["runtime_root"], payload,
                             3 * len(admission["rows"]) + 12, 2, reserve=RETIRE_GUEST_RESERVE)
    # The fixed operating reserve exceeds even the largest supported metadata
    # peak. Larger allocation geometries/records must not silently defeat it.
    need(plan["allocations"][0]["bytes"] <= RETIRE_GUEST_RESERVE,
         "rounded retirement metadata exceeds the fixed operating reserve")
    return plan, result


def marker(work, name, value):
    path = work / name
    raw = canonical(value)
    need(len(raw) <= 4096, "progress marker exceeds its bound")
    if path.exists():
        need(not os.path.lexists(path.with_name("." + path.name + ".pending")), "published record gained pending sibling")
        need(read(path, mode=0o400) == raw, "retirement progress differs")
        sync(path.parent)
    else:
        write_new(path, raw)


def rename_exclusive(source, destination, expected_fd, parent_identity=None):
    import ctypes
    need(sys.platform in ("linux", "darwin"), "exclusive rename unavailable")
    libc = ctypes.CDLL(None, use_errno=True)
    function = libc.renameat2 if sys.platform == "linux" else libc.renameatx_np
    function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    function.restype = ctypes.c_int
    need(source.parent == destination.parent, "quarantine must stay in exact binary parent")
    with anchored_directory(source.parent) as parent:
        need(parent_identity is None or identity(os.fstat(parent))[:2] == parent_identity,
             "binary parent differs from admitted inode")
        need(identity(os.fstat(expected_fd)) == identity(os.stat(source.name, dir_fd=parent, follow_symlinks=False)),
             "original changed before exclusive quarantine")
        need(function(parent, os.fsencode(source.name), parent, os.fsencode(destination.name), 1 if sys.platform == "linux" else 4) == 0,
             "exclusive quarantine failed")
        os.fsync(parent)


def retire(plan, deployment, admission, archive_digest):
    """All names disappear before the first unlink; every interrupted state is owned."""
    intent = retirement_intent(admission, archive_digest)
    runtime = private_directory(deployment["runtime_root"])
    with authority_locks(deployment):
        rows, sources = authority_rows(plan, deployment)
        need(rows == [{key: row[key] for key in ("path", "size", "sha256", "mode")} for row in admission["rows"]]
             and sources == admission["sources"], "retirement authority differs from archived inputs")
        retirement_capacity(admission, archive_digest)
        parent = runtime / "retained-public-release-v1"
        if not parent.exists():
            fresh_directory(parent)
        private_directory(parent)
        with anchored_directory(parent) as directory:
            lock = os.open("custody.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600, dir_fd=directory)
            try:
                info = os.fstat(lock)
                need(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and info.st_nlink == 1
                     and stat.S_IMODE(info.st_mode) == 0o600 and identity(info) == identity((parent / "custody.lock").lstat()), "custody lock differs")
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                need(identity(info) == identity(os.fstat(lock)) == identity(os.stat("custody.lock", dir_fd=directory, follow_symlinks=False)),
                     "custody lock replaced during acquisition")
                with anchored_directory(parent) as reopened:
                    need(identity(os.fstat(directory))[:2] == identity(os.fstat(reopened))[:2], "custody lock parent replaced")
                private_directory(parent)
                return retire_locked(plan, deployment, admission, intent, parent)
            finally:
                os.close(lock)


def retire_locked(plan, deployment, admission, intent, parent):
    work = parent / intent["token"]
    if work.exists():
        private_directory(work)
        if (work / "intent.json").exists():
            need(not os.path.lexists(work / ".intent.json.pending"), "published intent gained pending sibling")
            need(read(work / "intent.json", mode=0o400) == canonical(intent), "retirement intent changed")
            sync(work)
        else:
            need(set(os.listdir(work)) <= {".intent.json.pending"}, "unpublished intent has foreign progress")
            need(inspect(plan, deployment) == admission, "source changed before recovering retirement intent")
            write_new(work / "intent.json", canonical(intent))
    else:
        need(inspect(plan, deployment) == admission, "source changed before retirement intent")
        fresh_directory(work)
        write_new(work / "intent.json", canonical(intent))
    allowed_work = {"intent.json", "quarantine-complete.json", "completed.json"}
    allowed_work.update(f"{i:04d}.{stage}.json" for i in range(len(intent["rows"])) for stage in ("quarantined", "delete-intent", "deleted"))
    allowed_work.update("." + name + ".pending" for name in tuple(allowed_work))
    need(set(os.listdir(work)) <= allowed_work, "retirement directory has unexpected entries")
    paths = [row[key] for row in intent["rows"] for key in ("path", "quarantine")]
    # Every sibling was classified before reading payloads. During resume only
    # the original and our exact per-file quarantine names may remain.
    for directory in {Path(row["path"]).parent for row in intent["rows"]}:
        public_binary_directory(directory)
        with anchored_directory(directory) as parent_fd:
            need(all(row["parent_identity"] == identity(os.fstat(parent_fd))[:2]
                     for row in intent["rows"] if Path(row["path"]).parent == directory),
                 "binary parent differs from admitted inode")
        allowed = {Path(path).name for path in paths if Path(path).parent == directory}
        need(set(os.listdir(directory)) <= allowed, "retained binary directory gained an unadmitted sibling")
    for index, row in enumerate(intent["rows"]):
        original, quarantine = Path(row["path"]), Path(row["quarantine"])
        need(not (os.path.lexists(original) and os.path.lexists(quarantine)), "original binary reappeared")
        deleted = work / f"{index:04d}.delete-intent.json"
        if os.path.lexists(original):
            need(not deleted.exists(), "deleted original binary reappeared")
            with held(original, digest=row["sha256"], size=row["size"], mode=0o755, stamp=row["identity"]) as (fd, _, _):
                no_live_references(paths, [fd], file_identities=intent["rows"])
                current_bindings(plan, deployment)
                # held() verifies its original path at exit, so verify before the
                # namespace mutation and keep the descriptor across rename below.
            with anchored_directory(original.parent) as directory:
                fd = os.open(original.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=directory)
                try:
                    need(identity(os.fstat(fd)) == row["identity"], "binary changed before quarantine")
                    rename_exclusive(original, quarantine, fd, row["parent_identity"])
                    need(identity(os.fstat(fd))[:8] == row["identity"][:8]
                         and identity(os.fstat(fd)) == identity(quarantine.lstat())
                         and hash_fd(fd, row["size"]) == row["sha256"], "quarantine custody changed")
                finally:
                    os.close(fd)
        if os.path.lexists(quarantine):
            with held(quarantine, digest=row["sha256"], size=row["size"], mode=0o755, stamp=row["identity"], renamed=True):
                pass
            marker(work, f"{index:04d}.quarantined.json", {"index": index, "sha256": row["sha256"]})
        else:
            need(deleted.exists() and read(deleted, mode=0o400) == canonical({"index": index, "sha256": row["sha256"]}),
                 "binary absent without durable deletion intent")
    need(not any(os.path.lexists(row["path"]) for row in intent["rows"]), "original binary reappeared")
    current_bindings(plan, deployment)
    no_live_references(paths, file_identities=intent["rows"])
    marker(work, "quarantine-complete.json", {"intent_sha256": sha(canonical(intent))})
    for index, row in enumerate(intent["rows"]):
        original, quarantine = Path(row["path"]), Path(row["quarantine"])
        need(not os.path.lexists(original), "original binary reappeared")
        current_bindings(plan, deployment)
        no_live_references(paths, file_identities=intent["rows"])
        need(os.path.lexists(quarantine) or (work / f"{index:04d}.delete-intent.json").exists(),
             "quarantine absent before deletion intent")
        marker(work, f"{index:04d}.delete-intent.json", {"index": index, "sha256": row["sha256"]})
        if os.path.lexists(quarantine):
            # Recheck bytes through a held FD and exact parent immediately before
            # unlink. The already verified off-host copy remains the rollback path.
            with anchored_directory(quarantine.parent) as directory:
                need(identity(os.fstat(directory))[:2] == row["parent_identity"], "quarantine parent differs from admitted inode")
                fd = os.open(quarantine.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=directory)
                try:
                    before = os.fstat(fd)
                    need(identity(before)[:8] == row["identity"][:8] and before.st_nlink == 1
                         and identity(before) == identity(os.stat(quarantine.name, dir_fd=directory, follow_symlinks=False))
                         and hash_fd(fd, row["size"]) == row["sha256"], "quarantined binary changed before unlink")
                    current_bindings(plan, deployment)
                    no_live_references(paths, [fd], file_identities=intent["rows"])
                    with anchored_directory(quarantine.parent) as reopened:
                        need(identity(os.fstat(reopened))[:2] == identity(os.fstat(directory))[:2], "quarantine parent changed before unlink")
                    need(not os.path.lexists(original) and identity(before) == identity(os.stat(quarantine.name, dir_fd=directory, follow_symlinks=False)), "binary path changed before unlink")
                    os.unlink(quarantine.name, dir_fd=directory)
                    os.fsync(directory)
                finally:
                    os.close(fd)
        marker(work, f"{index:04d}.deleted.json", {"index": index, "sha256": row["sha256"]})
    result = {"schema": SCHEMA, "retired": True, "archive_sha256": intent["archive_sha256"],
              "intent_sha256": sha(canonical(intent)), "files": len(intent["rows"]),
              "allocated_bytes_removed": sum(row["allocated_bytes"] for row in intent["rows"]),
              "source_receipts_history_runtime_preserved": True, "deployment_authorized": False}
    marker(work, "completed.json", result)
    return result


def trim_and_observe(runtime):
    """Complete supported filesystem maintenance without claiming physical reclaim."""
    import taira_disk_capacity as capacity
    runtime = private_directory(runtime)
    mount = subprocess.run(["/usr/bin/findmnt", "--noheadings", "--output", "TARGET", "--target", str(runtime)],
                           stdin=subprocess.DEVNULL, capture_output=True, timeout=15)
    need(mount.returncode == 0 and mount.stdout == b"/\n", "approved runtime must remain on the guest root filesystem")
    observations = {}
    for name, argv in (("sync", ["/usr/bin/sync", "-f", str(runtime)]),
                       ("trim", ["/usr/sbin/fstrim", "--verbose", "/"])):
        result = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, timeout=120)
        need(len(result.stdout) + len(result.stderr) <= 16384, "filesystem maintenance output exceeds bound")
        observations[name] = {"passed": result.returncode == 0, "exit_code": result.returncode,
                              "stdout_sha256": sha(result.stdout), "stderr_sha256": sha(result.stderr)}
        if result.returncode:
            break
    observations["guest_after"] = capacity.inspect_filesystem(runtime)
    observations["physical_reclamation_claimed"] = False
    return observations


def authenticated_modules(root, controller):
    validate_controller(controller)
    root = direct(root)
    environment = {key: os.environ[key] for key in ("PATH", "HOME", "GNUPGHOME") if key in os.environ}
    environment.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null", LC_ALL="C")
    gpg = shutil.which("gpg", path=environment.get("PATH"))
    need(gpg is not None, "GnuPG is required")
    gpg = str(Path(gpg).resolve(strict=True))
    def git(*args):
        child = subprocess.run(["/usr/bin/git", "--no-replace-objects", "-c", "gpg.format=openpgp",
            "-c", "gpg.program=" + gpg, "-c", "gpg.openpgp.program=" + gpg, *args], cwd=root,
            capture_output=True, stdin=subprocess.DEVNULL, timeout=60, env=environment)
        need(child.returncode == 0, "signed controller/source Git verification failed")
        return child.stdout
    need(git("rev-parse", "--show-toplevel").strip() == os.fsencode(root)
         and git("branch", "--show-current").strip() == b"optimizations", "canonical optimizations checkout required")
    git("verify-commit", controller["commit"])
    need(git("show", "--no-patch", "--format=%GF", controller["commit"]).decode().strip() == controller["signer"], "controller signer differs")
    modules = {}
    for name in MODULES:
        raw = read(root / "scripts" / (name + ".py"))
        need(raw == git("show", controller["commit"] + ":scripts/" + name + ".py"), "controller differs from signed source")
        modules[name] = raw
    return modules, git


def archive_controller_provenance(git, archived, execution):
    validate_controller(archived)
    validate_controller(execution)
    need(archived["signer"] == execution["signer"], "archive and execution controller signers differ")
    git("verify-commit", archived["commit"])
    need(git("show", "--no-patch", "--format=%GF", archived["commit"]).decode().strip() == archived["signer"],
         "archive controller signer differs")
    git("merge-base", "--is-ancestor", archived["commit"], execution["commit"])
    return {"archive_controller": archived, "execution_controller": execution,
            "archive_controller_is_verified_ancestor": True}


def load_modules(modules):
    loaded = {}
    for name in MODULES:
        module = types.ModuleType(name)
        module.__file__ = "<signed-retained-public>/" + name + ".py"
        module.__spec__ = importlib.util.spec_from_loader(name, loader=None, origin=module.__file__)
        sys.modules[name] = module
        exec(compile(modules[name], module.__file__, "exec"), module.__dict__)
        loaded[name] = module
    return loaded


BOOTSTRAP = '''import sys,os,struct,json,base64,types,hashlib,select,time
sys.dont_write_bytecode=True
end=time.monotonic()+60
def exact(n):
 r=bytearray()
 while len(r)<n:
  left=end-time.monotonic();assert left>0 and select.select([0],[],[],left)[0]
  b=os.read(0,min(1048576,n-len(r)));assert b;r.extend(b)
 return bytes(r)
n=struct.unpack(">I",exact(4))[0];assert 0<n<=8388608
e=json.loads(exact(n))
for name in ("release_artifact_contract","taira_disk_capacity","taira_retry","taira_retained_release"):
 p=e["modules"][name];b=base64.b64decode(p["source"],validate=True);assert hashlib.sha256(b).hexdigest()==p["sha256"]
 m=types.ModuleType(name);m.__file__="<signed-retained-public>/"+name+".py";sys.modules[name]=m
 exec(compile(b,m.__file__,"exec"),m.__dict__)
sys.modules["taira_retained_release"].remote(e)
'''


def remote(envelope):
    operation = envelope["operation"]
    if operation == "backing-observe":
        need(sys.platform == "darwin", "approved Mac backing host required")
        import taira_disk_capacity as capacity
        send_frame(1, capacity.inspect_filesystem(direct(envelope["path"])))
        return
    if operation in ("backing-capacity", "binary-retirement-backing-capacity"):
        need(sys.platform == "darwin", "approved Mac backing host required")
        import taira_disk_capacity as capacity
        need(type(envelope["bytes"]) is int and 0 <= envelope["bytes"] <= MAX_TOTAL,
             "bounded guest physical demand required")
        path = str(direct(envelope["path"]))
        result = capacity.evaluate({"schema": capacity.PLAN_SCHEMA, "allocations": [
            {"path": path, "label": "rounded guest retirement metadata and reserve",
             "bytes": envelope["bytes"], "inodes": 1},
            {"path": path, "label": "retained public physical backing reserve",
             "bytes": RETIRE_BACKING_RESERVE if operation == "binary-retirement-backing-capacity" else RESERVE,
             "inodes": 256}]})
        need(result["passed"], "insufficient physical backing capacity for retirement")
        send_frame(1, result)
        return
    need(sys.platform == "linux" and platform.machine() in ("aarch64", "arm64") and os.geteuid() == 0,
         "approved root AArch64 Linux guest required")
    plan, deployment = envelope["plan"], envelope["deployment"]
    validate_plan(plan)
    if operation == "inspect":
        with authority_locks(deployment):
            send_frame(1, inspect(plan, deployment))
    elif operation == "archive":
        archive_stream(plan, deployment, envelope["admission"], 1)
    elif operation == "archive-resume":
        archive_stream(plan, deployment, envelope["admission"], 1, resume=envelope["resume"])
    elif operation == "retire-capacity":
        send_frame(1, retirement_capacity(envelope["admission"], envelope["archive_sha256"])[0])
    elif operation == "retire":
        result = retire(plan, deployment, envelope["admission"], envelope["archive_sha256"])
        result["storage_after"] = trim_and_observe(deployment["runtime_root"])
        send_frame(1, result)
    else:
        need(False, "unknown retained public operation")


@contextlib.contextmanager
def session(route, envelope, modules, evidence):
    import taira_retry as retry
    argv = list(retry.validate_ssh(route))
    argv[-1] = "/usr/bin/python3 -I -c " + shlex.quote(BOOTSTRAP)
    envelope = {**envelope, "modules": {name: {"source": base64.b64encode(modules[name]).decode(), "sha256": sha(modules[name])} for name in MODULES}}
    raw = canonical(envelope)
    need(len(raw) <= MAX_RECORD, "bootstrap exceeds its bound")
    fd = os.open(evidence, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as diagnostics:
        child = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=diagnostics,
            env={key: os.environ[key] for key in ("PATH", "HOME") if key in os.environ}, umask=0o077)
        try:
            child.stdin.write(struct.pack(">I", len(raw)) + raw)
            child.stdin.close()
            yield Reader(child.stdout.fileno())
            need(child.wait(timeout=30) == 0, "remote operation failed; retain diagnostics and custody")
            need(os.fstat(diagnostics.fileno()).st_size <= MAX_RECORD, "remote diagnostics exceed bound")
        finally:
            if child.poll() is None:
                child.kill()  # Only this invocation's SSH child; no Cargo/service process.
            child.wait()
            child.stdout.close()


def call(route, envelope, modules, evidence):
    with session(route, envelope, modules, evidence) as reader:
        return reader.frame()


def archive_local(plan, deployment, admission, modules, output):
    allocation(output.parent, sum(row["size"] for row in admission["rows"]) + 3 * MAX_RECORD, len(admission["rows"]) + 8, 2)
    fresh_directory(output)
    write_new(output / "plan.json", canonical(plan))
    write_new(output / "admission.json", canonical(admission))
    fresh_directory(output / "objects")
    envelope = {"operation": "archive", "plan": plan, "deployment": deployment, "admission": admission}
    with session(plan["guest_ssh"], envelope, modules, output / "archive.stderr") as reader:
        need(reader.frame() == admission, "archive stream admission differs")
        for index, row in enumerate(admission["rows"]):
            path = output / "objects" / f"{index:04d}"
            fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            try:
                digest, remaining = hashlib.sha256(), row["size"]
                while remaining:
                    raw = reader.exact(min(CHUNK, remaining))
                    digest.update(raw)
                    write_all(fd, raw)
                    remaining -= len(raw)
                need(digest.hexdigest() == row["sha256"], "off-host archive bytes differ")
                os.fsync(fd)
                need(hash_fd(fd, row["size"]) == row["sha256"], "off-host archive reread differs")
                os.fchmod(fd, 0o400)
                os.fsync(fd)
            finally:
                os.close(fd)
            sync(path.parent)
        need(reader.frame() == {"archive_stream_verified": True, "admission_sha256": sha(canonical(admission))}, "archive final verification differs")
    completed = {"schema": SCHEMA, "archive_complete": True, "admission_sha256": sha(canonical(admission)),
                 "plan_sha256": sha(canonical(plan)), "files": len(admission["rows"]), "retirement_authorized": False}
    write_new(output / "completed.json", canonical(completed))
    verify_archive(output)
    return completed


def verify_archive(path):
    private_directory(path)
    need(set(os.listdir(path)) == {"plan.json", "admission.json", "objects", "archive.stderr", "completed.json"}, "archive has incomplete or unexpected entries")
    plan, admission = archive_inputs(path)
    completed_raw = read(path / "completed.json", mode=0o400)
    completed = decode(completed_raw)
    need(canonical(completed) == completed_raw, "archive completion must retain exact canonical producer bytes")
    need(completed == {"schema": SCHEMA, "archive_complete": True, "admission_sha256": sha(canonical(admission)),
         "plan_sha256": sha(canonical(plan)), "files": len(admission["rows"]), "retirement_authorized": False}
         and admission["plan_sha256"] == completed["plan_sha256"] and 1 <= len(admission["rows"]) <= MAX_FILES,
         "archive completion differs")
    private_directory(path / "objects")
    need(set(os.listdir(path / "objects")) == {f"{index:04d}" for index in range(len(admission["rows"]))}, "archive object census differs")
    for index, row in enumerate(admission["rows"]):
        with held(path / "objects" / f"{index:04d}", digest=row["sha256"], size=row["size"], mode=0o400):
            pass
    return plan, admission, sha(canonical(completed))


def archive_inputs(path):
    plan_raw = read(path / "plan.json", mode=0o400)
    admission_raw = read(path / "admission.json", mode=0o400)
    plan = validate_plan(decode(plan_raw))
    admission = decode(admission_raw)
    need(canonical(plan) == plan_raw and canonical(admission) == admission_raw,
         "archive metadata must retain exact canonical producer bytes")
    need(admission["schema"] == SCHEMA and admission["plan_sha256"] == sha(plan_raw)
         and 1 <= len(admission["rows"]) <= MAX_FILES
         and all(type(row["size"]) is int and 0 < row["size"] <= MAX_BINARY for row in admission["rows"])
         and sum(row["size"] for row in admission["rows"]) <= MAX_TOTAL,
         "archive admission differs from its exact bounded plan")
    return plan, admission


@contextlib.contextmanager
def held_archive(path, expected):
    plan, admission, digest = expected
    with contextlib.ExitStack() as stack:
        for name, expected_digest in (("plan.json", sha(canonical(plan))),
                                      ("admission.json", sha(canonical(admission))),
                                      ("completed.json", digest)):
            stack.enter_context(held(path / name, digest=expected_digest, mode=0o400, maximum=MAX_RECORD))
        for index, row in enumerate(admission["rows"]):
            stack.enter_context(held(path / "objects" / f"{index:04d}", digest=row["sha256"], size=row["size"], mode=0o400))
        need(verify_archive(path) == expected, "archive differs from the original verified metadata")
        yield
        need(verify_archive(path) == expected, "archive changed from the original verified metadata")


def archive_resume_census(path):
    private_directory(path)
    names = set(os.listdir(path))
    required = {"plan.json", "admission.json", "objects", "archive.stderr"}
    need(required <= names <= required | {".completed.json.pending"},
         "resume requires the exact incomplete archive namespace")
    plan, admission = archive_inputs(path)
    objects = private_directory(path / "objects")
    names = set(os.listdir(objects))
    need(len(names) <= len(admission["rows"]) and names == {f"{index:04d}" for index in range(len(names))},
         "archive objects must be a contiguous prefix without missing middle files")
    completed, partial = 0, None
    for index in range(len(names)):
        row = admission["rows"][index]
        obj = objects / f"{index:04d}"
        info = obj.lstat()
        mode = stat.S_IMODE(info.st_mode)
        if mode == 0o400:
            need(partial is None, "completed object follows a partial archive file")
            with held(obj, digest=row["sha256"], size=row["size"], mode=0o400):
                pass
            completed += 1
        else:
            need(mode == 0o600 and index == len(names) - 1 and info.st_size <= row["size"],
                 "only the final archive object may be a bounded private partial file")
            with held(obj, size=info.st_size, mode=0o600) as (_, before, digest):
                partial = {"index": index, "size": before.st_size, "sha256": digest}
    return plan, admission, validate_resume({"completed": completed, "partial": partial}, admission["rows"])


def archive_resume_local(plan, deployment, admission, modules, output, diagnostics, *, expected_resume=None):
    actual_plan, actual_admission, resume = archive_resume_census(output)
    need((actual_plan, actual_admission) == (plan, admission), "resume metadata differs from original admission")
    need(expected_resume is None or resume == expected_resume, "archive prefix changed from the authenticated resume intent")
    need(deployment == admission["deployment"], "resume occupied deployment differs")
    need(diagnostics.parent != output and output not in diagnostics.parents,
         "resume diagnostics must remain outside the immutable archive namespace")
    partial = resume["partial"]
    missing = sum(row["size"] for row in admission["rows"][resume["completed"]:])
    if partial is not None:
        missing -= partial["size"]
    allocation(output.parent, missing + 3 * MAX_RECORD, len(admission["rows"]) - resume["completed"] + 8)
    with contextlib.ExitStack() as stack:
        for name, value in (("plan.json", plan), ("admission.json", admission)):
            stack.enter_context(held(output / name, digest=sha(canonical(value)), mode=0o400, maximum=MAX_RECORD))
        for index, row in enumerate(admission["rows"][:resume["completed"]]):
            stack.enter_context(held(output / "objects" / f"{index:04d}", digest=row["sha256"], size=row["size"], mode=0o400))
        partial_identity = None
        if partial is not None:
            with held(output / "objects" / f"{partial['index']:04d}", digest=partial["sha256"],
                      size=partial["size"], mode=0o600) as (_, info, _):
                partial_identity = identity(info)
        envelope = {"operation": "archive-resume", "plan": plan, "deployment": deployment,
                    "admission": admission, "resume": resume}
        with session(plan["guest_ssh"], envelope, modules, diagnostics) as reader:
            need(reader.frame() == admission, "resumed archive source admission differs")
            for index in range(resume["completed"], len(admission["rows"])):
                row = admission["rows"][index]
                path = output / "objects" / f"{index:04d}"
                continuing = partial is not None and index == partial["index"]
                offset = partial["size"] if continuing else 0
                with anchored_directory(path.parent) as directory:
                    flags = os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
                    fd = os.open(path.name, flags if continuing else flags | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=directory)
                    try:
                        info = os.fstat(fd)
                        need(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid()
                             and info.st_nlink == 1 and stat.S_IMODE(info.st_mode) == 0o600,
                             "unsafe resumed archive object")
                        if continuing:
                            need(identity(info) == partial_identity and hash_fd(fd, offset) == partial["sha256"],
                                 "partial archive changed before continuation")
                        else:
                            need(info.st_size == 0, "new archive object is not empty")
                        os.lseek(fd, offset, os.SEEK_SET)
                        remaining = row["size"] - offset
                        while remaining:
                            raw = reader.exact(min(CHUNK, remaining))
                            write_all(fd, raw)
                            remaining -= len(raw)
                        os.fsync(fd)
                        need(hash_fd(fd, row["size"]) == row["sha256"], "resumed full object digest differs")
                        os.fchmod(fd, 0o400)
                        os.fsync(fd)
                        need(identity(os.fstat(fd)) == identity(os.stat(path.name, dir_fd=directory, follow_symlinks=False)),
                             "resumed archive object replaced")
                        os.fsync(directory)
                    finally:
                        # Interrupted tails remain owned, synchronized and resumable.
                        os.fsync(fd)
                        os.close(fd)
                        os.fsync(directory)
            need(reader.frame() == {"archive_stream_verified": True, "admission_sha256": sha(canonical(admission))},
                 "resumed archive final verification differs")
        need(archive_inputs(output) == (plan, admission), "archive metadata changed during resume")
    completed = {"schema": SCHEMA, "archive_complete": True, "admission_sha256": sha(canonical(admission)),
                 "plan_sha256": sha(canonical(plan)), "files": len(admission["rows"]), "retirement_authorized": False}
    # Verify every complete payload before publishing the same current-format receipt.
    _, _, finished = archive_resume_census(output)
    need(finished == {"completed": len(admission["rows"]), "partial": None}, "resumed archive is incomplete")
    write_new(output / "completed.json", canonical(completed))
    need(verify_archive(output) == (plan, admission, sha(canonical(completed))), "resumed archive verification differs")
    return completed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    sub = parser.add_subparsers(dest="operation", required=True)
    archive = sub.add_parser("archive", help="read-only guest stream into verified off-host custody")
    archive.add_argument("--plan", required=True, type=Path)
    archive.add_argument("--output-dir", required=True, type=Path)
    retire_parser = sub.add_parser("retire", help="retire only the exact binaries of a complete verified archive")
    retire_parser.add_argument("--archive-dir", required=True, type=Path)
    retire_parser.add_argument("--output-dir", required=True, type=Path)
    resume_parser = sub.add_parser("resume-archive", help="verify retained archive prefix and copy only the missing tail")
    resume_parser.add_argument("--archive-dir", required=True, type=Path)
    resume_parser.add_argument("--output-dir", required=True, type=Path)
    for command in (retire_parser, resume_parser):
        command.add_argument("--execution-controller-commit", required=True)
        command.add_argument("--execution-controller-signer", required=True)
    verify = sub.add_parser("verify", help="rehash an existing off-host archive without remote access")
    verify.add_argument("--archive-dir", required=True, type=Path)
    args = parser.parse_args()
    os.umask(0o077)
    if args.operation == "verify":
        _, _, digest = verify_archive(direct(args.archive_dir))
        print(json.dumps({"verified": True, "archive_sha256": digest}))
        return
    if args.operation == "archive":
        plan = validate_plan(decode(read(args.plan, mode=0o600)))
        execution_controller = plan["controller"]
    else:
        execution_controller = validate_controller({"commit": args.execution_controller_commit,
                                                    "signer": args.execution_controller_signer})
        if args.operation == "resume-archive":
            plan, admission, resume = archive_resume_census(direct(args.archive_dir))
        else:
            expected_archive = verify_archive(direct(args.archive_dir))
            plan, admission, archive_digest = expected_archive
    modules, git = authenticated_modules(args.repo_root, execution_controller)
    provenance = archive_controller_provenance(git, plan["controller"], execution_controller)
    provenance.update(archive_plan_sha256=sha(canonical(plan)),
                      execution_module_sha256={name: sha(raw) for name, raw in modules.items()})
    loaded = load_modules(modules)
    owner = loaded["taira_retained_release"]
    retry = loaded["taira_retry"]
    retry.validate_ssh(plan["guest_ssh"])
    retry.validate_ssh(plan["backing_ssh"])
    deployment = deployment_projection(plan)
    output = direct(args.output_dir)
    if args.operation == "archive":
        private_directory(output.parent)
        admission = owner.call(plan["guest_ssh"], {"operation": "inspect", "plan": plan, "deployment": deployment}, modules,
                               output.with_name(output.name + ".inspect.stderr"))
        for source in admission["sources"]:
            git("verify-commit", source["commit"])
            need(git("show", "--no-patch", "--format=%GF", source["commit"]).decode().strip() == plan["controller"]["signer"]
                 and git("rev-parse", source["commit"] + "^{tree}").decode().strip() == source["tree"], "retained signed source identity differs")
        result = owner.archive_local(plan, deployment, admission, modules, output)
    elif args.operation == "resume-archive":
        need(deployment == admission["deployment"], "occupied deployment changed since archive")
        fresh_directory(output)
        provenance.update(archive_admission_sha256=sha(canonical(admission)), resume=resume)
        write_new(output / "controller-provenance.json", canonical(provenance))
        completed = owner.archive_resume_local(plan, deployment, admission, modules, direct(args.archive_dir),
            output / "resume.stderr", expected_resume=resume)
        result = {"schema": SCHEMA, "archive_resumed": True, "archive_complete": completed,
                  "archive_sha256": sha(canonical(completed)), "controller_provenance_sha256": sha(canonical(provenance)),
                  "retirement_authorized": False}
        write_new(output / "completed.json", canonical(result))
    else:
        need(deployment == admission["deployment"], "occupied deployment changed since archive")
        fresh_directory(output)
        provenance.update(archive_admission_sha256=sha(canonical(admission)), archive_completion_sha256=archive_digest)
        write_new(output / "controller-provenance.json", canonical(provenance))
        envelope = {"plan": plan, "deployment": deployment, "admission": admission, "archive_sha256": archive_digest}
        # Pin original A metadata as well as payloads across every probe and
        # retirement dispatch; a separately valid replacement B is never adopted.
        with owner.held_archive(direct(args.archive_dir), expected_archive):
            capacity = owner.call(plan["guest_ssh"], {**envelope, "operation": "retire-capacity"}, modules, output / "capacity.stderr")
            required = sum(row["bytes"] for row in capacity["allocations"])
            backing_before = owner.call(plan["backing_ssh"], {"operation": "binary-retirement-backing-capacity",
                "path": plan["backing_path"], "bytes": required}, modules, output / "backing.stderr")
            result = owner.call(plan["guest_ssh"], {**envelope, "operation": "retire"}, modules, output / "retire.stderr")
        result["controller_provenance_sha256"] = sha(canonical(provenance))
        result["backing_before"] = backing_before
        result["backing_after"] = owner.call(plan["backing_ssh"], {"operation": "backing-observe", "path": plan["backing_path"]}, modules, output / "backing-after.stderr")
        write_new(output / "completed.json", canonical(result))
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (RetainedReleaseError, OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"taira retained release refused: {error}", file=sys.stderr)
        raise SystemExit(2) from None
