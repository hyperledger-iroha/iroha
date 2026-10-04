#!/usr/bin/env python3
"""Complete native Taira edge custody as a direct authenticated Rust child.

Requires Darwin, Python 3.11+, the four embedded owner helpers, and inherited
public descriptors retained by the independently guarded native dispatcher.
There are no environment overrides, SSH routes or private configuration reads.
Rust verifies signatures and the ordered cross-host proof before admitting a
child. This helper rechecks its actual parent, held lock, exact public inputs,
irreversible proof fence and native publication before every owned effect.
--inspect-request-fd performs the maintained read-only owner observation only.
"""
from __future__ import annotations

import argparse
import base64
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import struct
import subprocess
import sys
import types

ADMISSION_SCHEMA = "iroha.taira.public-reset.native-edge-completion-admission.v1"
RECEIPT_SCHEMA = "iroha.taira.public-reset.native-edge-completion-receipt.v1"
JOURNAL_SCHEMA = "iroha.taira.public-reset.native-edge-completion-journal.v1"
PROGRESS_SCHEMA = "iroha.taira.public-reset.native-edge-progress.v1"
FENCE_SCHEMA = "iroha.taira.public-reset.native-edge-global-proof.v1"
SOURCE_NAMES = ("taira_native_nginx_check.py", "taira_native_nginx_apply.py",
                "taira_native_validator_forwarding.py", "taira_native_edge_completion.py")
IDENTITY_KEYS = {"device", "inode", "uid", "gid", "mode", "links", "size", "mtime_ns", "ctime_ns"}
BINDING_KEYS = {"operation_id", "inventory_sha256", "authorization_sha256", "authorization_nonce",
                "host_pair_sha256", "host_identity_sha256", "custody_root"}
MAX_PUBLIC_BYTES = 1024 * 1024


def need(condition, code):
    """Refuse using a bounded code, never a rendered input or native diagnostic."""
    if not condition:
        raise RuntimeError(code)


def fields(value, expected, code):
    need(isinstance(value, dict) and set(value) == set(expected), code)


def hex_value(value, length, code):
    need(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{" + str(length) + "}", value)
         is not None, code)


def path_value(value):
    need(isinstance(value, str) and re.fullmatch(r"/[A-Za-z0-9_./-]+", value) is not None
         and str(Path(value)) == value and os.path.normpath(value) == value, "noncanonical_path")
    return Path(value)


def identity(info):
    """Preserve exact native integers, including nanosecond values above 2**53."""
    return dict(device=info.st_dev, inode=info.st_ino, uid=info.st_uid, gid=info.st_gid,
                mode=stat.S_IMODE(info.st_mode), links=info.st_nlink, size=info.st_size,
                mtime_ns=info.st_mtime_ns, ctime_ns=info.st_ctime_ns)


def unique_members(entries):
    result = {}
    for name, value in entries:
        need(name not in result, "duplicate_json_member")
        result[name] = value
    return result


def public_json(body, limit=MAX_PUBLIC_BYTES):
    need(0 < len(body) <= limit, "public_json_size_bound")
    return json.loads(body, object_pairs_hook=unique_members)


def descriptor_value(fd):
    need(type(fd) is int and 2 < fd <= 2147483647, "inherited_descriptor_required")
    os.fstat(fd)
    return fd


def open_anchored(target, *, directory=False):
    """Never follow any ancestor symlink or a writable foreign parent."""
    target = path_value(str(target))
    parent = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        for component in target.parts[1:-1]:
            child = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                            dir_fd=parent)
            info = os.fstat(child)
            try:
                need(info.st_uid in {0, os.geteuid()} and not info.st_mode & 0o022,
                     "untrusted_native_parent")
            except BaseException:
                os.close(child)
                raise
            os.close(parent)
            parent = child
        flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
        if directory:
            flags |= os.O_DIRECTORY
        return os.open(target.name, flags, dir_fd=parent)
    finally:
        os.close(parent)


def validate_observed(reference, *, directory=False):
    fields(reference, {"path", "identity"}, "observed_file_fields")
    path_value(reference["path"])
    observed = reference["identity"]
    fields(observed, IDENTITY_KEYS, "native_identity_fields")
    need(all(type(value) is int and 0 <= value <= 2**64 - 1 for value in observed.values()),
         "native_identity_numbers")
    need(observed["uid"] == os.geteuid() and observed["gid"] == os.getegid()
         and observed["mode"] == (0o700 if directory else 0o600)
         and (directory or observed["links"] == 1), "unsafe_retained_owner")


def recheck_observed(reference, fd, *, directory=False, executable=False):
    """Join the inherited descriptor to a newly anchored pathname observation."""
    expected = reference["identity"]
    current = open_anchored(reference["path"], directory=directory)
    try:
        info = os.fstat(fd)
        need((stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode))
             and identity(info) == expected == identity(os.fstat(current)), "retained_identity_changed")
        if executable:
            need(info.st_uid == os.geteuid() and info.st_nlink == 1
                 and not info.st_mode & 0o022 and info.st_mode & 0o111,
                 "unsafe_parent_executable")
    finally:
        os.close(current)


def retained_public(value, *, limit=MAX_PUBLIC_BYTES, executable=False):
    fields(value, {"fd", "reference"}, "retained_public_fields")
    descriptor_value(value["fd"])
    reference = value["reference"]
    fields(reference, {"file", "sha256"}, "public_reference_fields")
    if executable:
        fields(reference["file"], {"path", "identity"}, "observed_file_fields")
        path_value(reference["file"]["path"])
        fields(reference["file"]["identity"], IDENTITY_KEYS, "native_identity_fields")
        need(all(type(item) is int and 0 <= item <= 2**64 - 1
                 for item in reference["file"]["identity"].values()), "native_identity_numbers")
    else:
        validate_observed(reference["file"])
    hex_value(reference["sha256"], 64, "public_digest")
    recheck_observed(reference["file"], value["fd"], executable=executable)
    size = reference["file"]["identity"]["size"]
    need(0 < size <= limit, "retained_public_size_bound")
    digest = hashlib.sha256()
    offset = 0
    body = bytearray() if not executable else None
    while offset < size:
        chunk = os.pread(value["fd"], min(65536, size - offset), offset)
        need(bool(chunk), "retained_public_truncated")
        digest.update(chunk)
        if body is not None:
            body.extend(chunk)
        offset += len(chunk)
    need(not os.pread(value["fd"], 1, size) and digest.hexdigest() == reference["sha256"],
         "retained_public_digest_changed")
    recheck_observed(reference["file"], value["fd"], executable=executable)
    return bytes(body) if body is not None else None


def source_closure():
    """Read only public embedded helpers; compile imports from these exact bytes."""
    source_root = Path(__file__).resolve().parent
    digest = hashlib.sha256(b"iroha:taira:public-reset:native-helper-source:v1\0")
    sources = {}
    for name in SOURCE_NAMES:
        fd = open_anchored(source_root / name)
        try:
            before = identity(os.fstat(fd))
            need(stat.S_ISREG(os.fstat(fd).st_mode) and before["uid"] in {0, os.geteuid()}
                 and before["links"] == 1 and not before["mode"] & 0o022
                 and 0 < before["size"] <= MAX_PUBLIC_BYTES, "unsafe_helper_source")
            source = os.pread(fd, MAX_PUBLIC_BYTES + 1, 0)
            need(len(source) == before["size"], "helper_source_changed")
            recheck_observed(dict(path=str(source_root / name), identity=before), fd)
            digest.update(name.encode() + b"\0" + struct.pack(">Q", len(source)) + source)
            sources[name] = source
        finally:
            os.close(fd)
    return digest.hexdigest(), sources


def owner_modules(sources):
    """Load the exact maintained owner bytes without an import-search fallback."""
    check = types.ModuleType("taira_native_nginx_check")
    check.__file__ = str(Path(__file__).parent / SOURCE_NAMES[0])
    exec(compile(sources[SOURCE_NAMES[0]], check.__file__, "exec"), check.__dict__)
    previous = sys.modules.get(check.__name__)
    sys.modules[check.__name__] = check
    try:
        apply = types.ModuleType("taira_native_nginx_apply")
        apply.__file__ = str(Path(__file__).parent / SOURCE_NAMES[1])
        exec(compile(sources[SOURCE_NAMES[1]], apply.__file__, "exec"), apply.__dict__)
    finally:
        if previous is None:
            del sys.modules[check.__name__]
        else:
            sys.modules[check.__name__] = previous
    return apply


def inspect_owned_publication(request):
    """Return only the maintained owner's closed read-only numeric observation."""
    _, sources = source_closure()
    return owner_modules(sources).inspect_owned_publication(request)


class Admission:
    """Retain public child custody and revalidate it before each owned effect."""

    def __init__(self, packet):
        fields(packet, BINDING_KEYS | {"schema", "action", "helper_source_closure_sha256", "parent", "lock",
            "guard", "inventory", "authorization", "progress", "checkpoints", "fence", "plan"},
            "completion_admission_fields")
        need(packet["schema"] == ADMISSION_SCHEMA and packet["action"] in {"rollback", "seal", "cleanup"}
             and sys.platform == "darwin", "native_completion_admission")
        for key in ("operation_id", "authorization_nonce"):
            hex_value(packet[key], 32, "completion_operation_identity")
        for key in BINDING_KEYS - {"operation_id", "authorization_nonce", "custody_root"}:
            hex_value(packet[key], 64, "completion_binding_digest")
        hex_value(packet["helper_source_closure_sha256"], 64, "helper_closure_digest")
        self.packet = packet
        self.bindings = {key: packet[key] for key in BINDING_KEYS}
        self.path = path_value(packet["custody_root"]) / "taira-edge/operations" / packet["authorization_sha256"]
        self.directory = open_anchored(self.path, directory=True)
        self.directory_identity = identity(os.fstat(self.directory))
        self.directory_owned_change = False
        self.plan = None
        self.progress = None
        self.fence_body = None
        self.sources = None
        self.predecessor = None
        try:
            need(self.directory_identity["uid"] == os.geteuid()
                 and self.directory_identity["gid"] == os.getegid()
                 and self.directory_identity["mode"] == 0o700, "unsafe_operation_directory")
            self.verify()
        except BaseException:
            self.close()
            raise

    def close(self):
        if self.predecessor is not None:
            os.close(self.predecessor["fd"])
        os.close(self.directory)

    def verify(self):
        packet = self.packet
        current = open_anchored(self.path, directory=True)
        try:
            need(identity(os.fstat(current)) == self.directory_identity == identity(os.fstat(self.directory)),
                 "operation_directory_changed")
        finally:
            os.close(current)
        fields(packet["parent"], {"pid", "uid", "started", "executable"}, "native_parent_fields")
        parent = packet["parent"]
        need(type(parent["pid"]) is int and parent["pid"] == os.getppid() and parent["pid"] > 1
             and type(parent["uid"]) is int and parent["uid"] == os.geteuid()
             and isinstance(parent["started"], str)
             and re.fullmatch(r"[A-Za-z0-9: ]{20,32}", parent["started"]) is not None,
             "native_parent_changed")
        retained_public(parent["executable"], limit=512 * 1024 * 1024, executable=True)
        closure, sources = source_closure()
        need(closure == packet["helper_source_closure_sha256"], "helper_closure_changed")
        self.sources = sources
        owner = owner_modules(sources)
        expected = dict(pid=parent["pid"], uid=parent["uid"], started=parent["started"],
                        executable=parent["executable"]["reference"]["file"]["path"])
        need(owner.observe_master(expected) == expected, "native_parent_changed")
        fields(packet["lock"], {"fd", "file"}, "native_lock_fields")
        lock = packet["lock"]
        descriptor_value(lock["fd"])
        validate_observed(lock["file"])
        need(lock["file"]["path"] == str(self.path / "operation.lock"), "operation_lock_path")
        recheck_observed(lock["file"], lock["fd"])
        competing = open_anchored(lock["file"]["path"])
        try:
            try:
                fcntl.flock(competing, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                pass
            else:
                fcntl.flock(competing, fcntl.LOCK_UN)
                raise RuntimeError("native_lock_not_held")
        finally:
            os.close(competing)
        for key, name in (("inventory", "inventory.json"), ("authorization", "authorization.json"),
                          ("progress", "progress.json"), ("plan", "completion-plan.json")):
            need(packet[key]["reference"]["file"]["path"] == str(self.path / name), "operation_public_path")
        inventory = public_json(retained_public(packet["inventory"]))
        authorization = public_json(retained_public(packet["authorization"]))
        guard = public_json(retained_public(packet["guard"]))
        fields(guard, {"schema", "host_slug", "service_root", "state_root", "trusted_key_sha256",
                       "dispatcher_path", "dispatcher_sha256", "upload_parent"}, "host_guard_fields")
        native = inventory["hosts"]["native_edge"]
        need(inventory["hosts"]["provider"] == "macstadium-dublin"
             and inventory["authorization_nonce"] == packet["authorization_nonce"]
             and packet["inventory"]["reference"]["sha256"] == packet["inventory_sha256"]
             and native["owner_uid"] == os.geteuid() and native["owner_gid"] == os.getegid()
             and native["custody_root"] == packet["custody_root"]
             and native["endpoint"]["host_identity_sha256"] == packet["host_identity_sha256"]
             and packet["guard"]["reference"]["sha256"] == native["guard_sha256"]
             and guard["schema"] == "iroha.taira.public-reset.host-guard.v1"
             and guard["dispatcher_path"] == native["dispatcher_path"] == expected["executable"]
             and guard["dispatcher_sha256"] == native["dispatcher_sha256"]
             == parent["executable"]["reference"]["sha256"], "native_guard_binding")
        claims = authorization["claims"]
        need(claims["inventory_sha256"] == packet["inventory_sha256"]
             and claims["authorization_nonce"] == packet["authorization_nonce"]
             and claims["deployment_id"] == inventory["deployment_id"], "authorization_binding")
        # The envelope's raw file hash intentionally differs from the semantic
        # authorization digest verified by the guarded Rust parent.
        self.progress = public_json(retained_public(packet["progress"], limit=16 * 1024))
        fields(self.progress, BINDING_KEYS | {"schema", "sequence", "predecessor_sha256", "request_sha256",
            "publication_operation_id", "status", "checkpoint_sha256", "completion_receipt_sha256"},
            "native_progress_fields")
        need(self.progress["schema"] == PROGRESS_SCHEMA
             and all(self.progress[key] == value for key, value in self.bindings.items())
             and type(self.progress["sequence"]) is int and 0 < self.progress["sequence"] <= 2**64 - 1,
             "native_progress_binding")
        need((self.progress["sequence"] == 1) == (self.progress["predecessor_sha256"] is None),
             "native_progress_sequence")
        for key in ("predecessor_sha256", "checkpoint_sha256", "completion_receipt_sha256"):
            if self.progress[key] is not None:
                hex_value(self.progress[key], 64, "native_progress_digest")
        hex_value(self.progress["request_sha256"], 64, "native_progress_request")
        hex_value(self.progress["publication_operation_id"], 32, "native_progress_publication")
        allowed_status = {"rollback": {"admitted", "staged", "cutover_requested", "awaiting_readiness",
            "edge_ready_unqualified", "rollback_requested", "rolled_back", "recovery_pending"},
            "seal": {"edge_ready_unqualified", "sealing", "sealed", "recovery_pending"},
            "cleanup": {"sealed", "cleanup_requested", "cleaned", "recovery_pending"}}
        need(self.progress["status"] in allowed_status[packet["action"]], "completion_progress_phase")
        self.plan = public_json(retained_public(packet["plan"]))
        fields(self.plan, {"schema", "nginx", "completion_journal_basename"}, "completion_plan_fields")
        need(self.plan["schema"] == "iroha.taira.public-reset.native-nginx-completion-plan.v1"
             and self.plan["completion_journal_basename"] == "native-completion.ndjson",
             "completion_plan_schema")
        owner.validate_plan(self.plan["nginx"])
        plan = self.plan["nginx"]
        need(plan["host_kind"] == "macos" and plan["publication"]["kind"] == "reconcile"
             and plan["operation_id"] == plan["publication"]["prior"]["operation_id"]
             == self.progress["publication_operation_id"], "completion_publication_binding")
        checkpoints = packet["checkpoints"]
        need(isinstance(checkpoints, list) and len(checkpoints) <= 3, "checkpoint_count")
        hashes = []
        for index, reference in enumerate(checkpoints):
            need(reference["reference"]["file"]["path"] == str(self.path / f"checkpoint-{index+1}.json"),
                 "checkpoint_path")
            checkpoint = public_json(retained_public(reference, limit=16 * 1024))
            fields(checkpoint, {"claims", "signature_hex"}, "signed_checkpoint_fields")
            claim = checkpoint["claims"]
            phase = {"kind": ("candidate_frontier", "native_edge_ready", "deployment_proven")[index],
                     "value": None}
            fields(claim, {"schema", "phase", "deployment_id", "inventory_sha256", "authorization_sha256",
                "authorization_nonce", "host_pair_sha256", "guest_host_identity_sha256",
                "native_edge_host_identity_sha256", "guest_custody_root", "native_edge_custody_root",
                "source_commit", "next_genesis_hash", "evidence_sha256", "predecessor_sha256",
                "execution_expires_at_unix_ms"}, "checkpoint_claims_fields")
            hex_value(checkpoint["signature_hex"], 128, "checkpoint_signature")
            hex_value(claim["evidence_sha256"], 64, "checkpoint_evidence")
            need(claim["schema"] == "iroha.taira.public-reset.host-phase-checkpoint.v1"
                 and claim["phase"] == phase
                 and claim["deployment_id"] == inventory["deployment_id"]
                 and claim["source_commit"] == inventory["revision"]["commit"]
                 and claim["next_genesis_hash"] == inventory["next_genesis_hash"]
                 and claim["guest_host_identity_sha256"] == inventory["hosts"]["validator_guest"]["endpoint"]["host_identity_sha256"]
                 and claim["guest_custody_root"] == inventory["hosts"]["validator_guest"]["custody_root"]
                 and claim["execution_expires_at_unix_ms"] == claims["execution_expires_at_unix_ms"]
                 and claim["inventory_sha256"] == packet["inventory_sha256"]
                 and claim["authorization_sha256"] == packet["authorization_sha256"]
                 and claim["authorization_nonce"] == packet["authorization_nonce"]
                 and claim["host_pair_sha256"] == packet["host_pair_sha256"]
                 and claim["native_edge_host_identity_sha256"] == packet["host_identity_sha256"]
                 and claim["native_edge_custody_root"] == packet["custody_root"]
                 and claim["predecessor_sha256"] == (hashes[-1] if hashes else None),
                 "checkpoint_binding")
            hashes.append(reference["reference"]["sha256"])
        fields(packet["fence"], {"kind", "value"}, "proof_fence_fields")
        fence = packet["fence"]
        if packet["action"] == "rollback":
            need(fence["kind"] == "absent" and len(hashes) < 3, "rollback_after_global_proof")
            fields(fence["value"], {"directory_fd", "directory", "basename"}, "absent_fence_fields")
            absent = fence["value"]
            descriptor_value(absent["directory_fd"])
            need(absent["directory"]["path"] == str(self.path) and absent["basename"] == "global-proof.json",
                 "absent_fence_path")
            validate_observed(absent["directory"], directory=True)
            # Only this helper's own controlled directory effects may refresh
            # the volatile directory metadata, never its inode/owner/mode.
            stable = {"device", "inode", "uid", "gid", "mode", "links"}
            need(all(absent["directory"]["identity"][key] == self.directory_identity[key] for key in stable)
                 and (self.directory_owned_change
                      or absent["directory"]["identity"] == self.directory_identity)
                 and identity(os.fstat(absent["directory_fd"])) == self.directory_identity,
                 "absent_fence_directory_changed")
            try:
                os.stat(absent["basename"], dir_fd=absent["directory_fd"], follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                raise RuntimeError("rollback_after_global_proof")
            self.fence_body = None
        else:
            need(fence["kind"] == "present" and len(hashes) == 3, "global_proof_required")
            fields(fence["value"], {"reference"}, "present_fence_fields")
            retained = fence["value"]["reference"]
            need(retained["reference"]["file"]["path"] == str(self.path / "global-proof.json"),
                 "global_proof_path")
            self.fence_body = public_json(retained_public(retained, limit=16 * 1024))
            fields(self.fence_body, BINDING_KEYS | {"schema", "checkpoint_sha256", "final_checkpoint_sha256",
                "progress_predecessor_sha256", "publication_operation_id"}, "global_proof_fields")
            need(self.fence_body["schema"] == FENCE_SCHEMA
                 and all(self.fence_body[key] == value for key, value in self.bindings.items())
                 and self.fence_body["checkpoint_sha256"] == hashes
                 and self.fence_body["final_checkpoint_sha256"] == hashes[-1]
                 and self.fence_body["publication_operation_id"] == plan["operation_id"], "global_proof_binding")
            if self.progress["status"] == "edge_ready_unqualified":
                need(self.fence_body["progress_predecessor_sha256"] == packet["progress"]["reference"]["sha256"],
                     "global_proof_progress_binding")
            hex_value(self.fence_body["progress_predecessor_sha256"], 64, "global_proof_progress_digest")
            if self.predecessor is None:
                opened = open_anchored(self.path / "global-proof-predecessor.json")
                self.predecessor = dict(fd=opened, reference=dict(file=dict(
                    path=str(self.path / "global-proof-predecessor.json"), identity=identity(os.fstat(opened))),
                    sha256=self.fence_body["progress_predecessor_sha256"]))
            predecessor = public_json(retained_public(self.predecessor, limit=16 * 1024))
            fields(predecessor, set(self.progress), "proof_predecessor_fields")
            need(predecessor["schema"] == PROGRESS_SCHEMA
                 and all(predecessor[key] == value for key, value in self.bindings.items())
                 and predecessor["status"] == "edge_ready_unqualified"
                 and predecessor["publication_operation_id"] == plan["operation_id"]
                 and predecessor["checkpoint_sha256"] == hashes[1], "proof_predecessor_binding")
        return owner

    def refresh_directory(self):
        self.directory_identity = identity(os.fstat(self.directory))
        self.directory_owned_change = True


class CompletionJournal:
    """One finite append-only effect chain, retained under the inherited lock."""

    def __init__(self, admission):
        self.admission = admission
        self.name = "native-completion.ndjson"
        flags = os.O_RDWR | os.O_APPEND | os.O_NOFOLLOW | os.O_CLOEXEC
        self.fd = None
        try:
            try:
                self.fd = os.open(self.name, flags | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=admission.directory)
                os.fsync(admission.directory)
                admission.refresh_directory()
            except FileExistsError:
                self.fd = os.open(self.name, flags, dir_fd=admission.directory)
        except BaseException:
            if self.fd is not None:
                os.close(self.fd)
            raise
        self.observed = identity(os.fstat(self.fd))
        self.records = []
        try:
            self.load()
        except BaseException:
            self.close()
            raise

    def load(self):
        admission = self.admission
        self.check()
        body = os.pread(self.fd, MAX_PUBLIC_BYTES + 1, 0)
        need(len(body) <= MAX_PUBLIC_BYTES and (not body or body.endswith(b"\n")), "completion_journal_bound")
        for ordinal, line in enumerate(body.splitlines(), 1):
            record = public_json(line, 65536)
            fields(record, BINDING_KEYS | {"schema", "sequence", "plan_sha256", "phase", "action",
                "publication_operation_id", "publication_identity", "backup_identity", "publisher_journal",
                "progress_before_sha256", "global_proof_sha256", "error_code", "publisher_terminal_record",
                "restored_owner"}, "completion_journal_fields")
            need(record["schema"] == JOURNAL_SCHEMA and type(record["sequence"]) is int
                 and record["sequence"] == ordinal
                 and all(record[key] == value for key, value in admission.bindings.items())
                 and record["plan_sha256"] == admission.packet["plan"]["reference"]["sha256"]
                 and record["publication_operation_id"] == admission.plan["nginx"]["operation_id"]
                 and record["phase"] in {"admitted", "rollback_requested", "source_restored",
                     "reload_requested", "publisher_terminal_requested", "publisher_terminal", "rolled_back",
                     "seal_requested", "sealed", "cleanup_requested", "cleaned", "recovery_pending"}
                 and record["action"] in {"rollback", "seal", "cleanup"}, "completion_journal_binding")
            self.records.append(record)
        need(len(self.records) <= 128, "completion_journal_record_bound")
        self.check()

    def close(self):
        os.close(self.fd)

    def check(self):
        info = os.fstat(self.fd)
        need(stat.S_ISREG(info.st_mode) and identity(info) == self.observed
             == identity(os.stat(self.name, dir_fd=self.admission.directory, follow_symlinks=False))
             and info.st_uid == os.geteuid() and info.st_gid == os.getegid() and info.st_nlink == 1
             and stat.S_IMODE(info.st_mode) == 0o600, "completion_journal_changed")

    def append(self, phase, *, publication_identity, backup_identity, publisher_journal, error_code=None,
               publisher_terminal_record=None, restored_owner=None):
        self.admission.verify()
        self.check()
        packet = self.admission.packet
        fence_sha = (packet["fence"]["value"]["reference"]["reference"]["sha256"]
                     if packet["fence"]["kind"] == "present" else None)
        record = dict(self.admission.bindings, schema=JOURNAL_SCHEMA, sequence=len(self.records)+1,
            plan_sha256=packet["plan"]["reference"]["sha256"], phase=phase, action=packet["action"],
            publication_operation_id=self.admission.plan["nginx"]["operation_id"],
            publication_identity=publication_identity, backup_identity=backup_identity,
            publisher_journal=publisher_journal,
            progress_before_sha256=packet["progress"]["reference"]["sha256"],
            global_proof_sha256=fence_sha, error_code=error_code,
            publisher_terminal_record=publisher_terminal_record,
            restored_owner=restored_owner)
        body = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode()
        need(len(self.records) < 128 and len(body) <= 65536
             and os.fstat(self.fd).st_size + len(body) <= MAX_PUBLIC_BYTES, "completion_journal_bound")
        need(os.write(self.fd, body) == len(body), "completion_journal_write_incomplete")
        os.fsync(self.fd)
        self.observed = identity(os.fstat(self.fd))
        self.check()
        # Never retain aliases to mutable caller metadata: a later controlled
        # rename/append must not silently rewrite an earlier durable intent.
        self.records.append(public_json(body, 65536))

    def reference(self):
        self.check()
        body = os.pread(self.fd, MAX_PUBLIC_BYTES+1, 0)
        need(0 < len(body) <= MAX_PUBLIC_BYTES, "completion_journal_empty_or_oversized")
        self.check()
        return dict(file=dict(path=str(self.admission.path / self.name), identity=self.observed),
                    sha256=hashlib.sha256(body).hexdigest())


def complete(packet):
    """Run one authenticated, exact-owner native terminal action locally."""
    admission = Admission(packet)
    journal = None
    try:
        journal = CompletionJournal(admission)
        return complete_owned(admission, journal)
    finally:
        if journal is not None:
            journal.close()
        admission.close()


def complete_owned(admission, journal):
    """Dispatch effects under the maintained nginx owner lock and decoder."""
    owner = admission.verify()
    plan = admission.plan["nginx"]
    # The public candidate is already declared by the canonical owner plan.
    # Read its exact inode natively, never the private main/TLS configuration.
    fd = open_anchored(plan["candidate"]["path"])
    try:
        observed = identity(os.fstat(fd))
        need(stat.S_ISREG(os.fstat(fd).st_mode) and observed["uid"] == plan["candidate"]["owner_uid"]
             and observed["links"] == 1 and not observed["mode"] & 0o022
             and 0 < observed["size"] <= MAX_PUBLIC_BYTES, "unsafe_public_candidate")
        candidate = os.pread(fd, MAX_PUBLIC_BYTES + 1, 0)
        recheck_observed(dict(path=plan["candidate"]["path"], identity=observed), fd)
        need(hashlib.sha256(candidate).hexdigest() == plan["candidate"]["sha256"], "candidate_digest_changed")
    finally:
        os.close(fd)
    request = dict(host_kind=plan["host_kind"], native=plan["native"], master=plan["master"],
        destination=plan["destination"], operation_id=plan["operation_id"], publication=plan["publication"],
        candidate_sha256=plan["candidate"]["sha256"], candidate_base64=base64.b64encode(candidate).decode(),
        renderer_source_sha256=plan["renderer_source"]["sha256"])
    result = owner.checked.remote_check(request, lambda receipt, request, context:
        finish_in_context(receipt, request, context, admission, journal, owner))
    status = result.get("completion_status", "recovery_pending")
    need(status in {"rolled_back", "sealed", "cleaned", "recovery_pending"}, "completion_status")
    code = result.get("error_code")
    if result.get("exit_code") != 0 or result.get("validation_files_removed") is not True:
        status = "recovery_pending"
        code = code or "native_completion_refused"
    need(code is None or isinstance(code, str) and re.fullmatch(r"[a-z_]{1,128}", code) is not None,
         "completion_error_code")
    fence_sha = (admission.packet["fence"]["value"]["reference"]["reference"]["sha256"]
                 if admission.packet["fence"]["kind"] == "present" else None)
    return dict(admission.bindings, schema=RECEIPT_SCHEMA, action=packet_action(admission),
        progress_before_sha256=admission.packet["progress"]["reference"]["sha256"],
        global_proof_sha256=fence_sha, publication_operation_id=plan["operation_id"],
        completion_journal=journal.reference(), status=status, error_code=code,
        restored_owned_publication=(result.get("restored_owned_publication") if status == "rolled_back" else None))


def packet_action(admission):
    return admission.packet["action"]


def finish_in_context(receipt, request, context, admission, journal, owner):
    """Only the owner-proven public include and retained backup can be changed."""
    action = packet_action(admission)
    directory = context["open_bound"](request["destination"]["directory"], True)
    destination = str(Path(request["destination"]["directory"]["path"]) / request["destination"]["basename"])
    basename = request["destination"]["basename"]
    backup_name = ".taira-nginx-backup-" + request["operation_id"] + ".public"
    prior = request["publication"]["prior"]
    publisher = prior["journal"]
    active = prior["publication"]["identity"]
    backup = None
    restored = None
    terminal_record = None
    master_handle = None
    retained_journals = {}

    def retain_journal(reference, opened):
        retained_journals[reference["path"]] = (reference, opened)

    def journal_effect(phase, **details):
        journal.append(phase, publication_identity=active, backup_identity=backup,
            publisher_journal=publisher, publisher_terminal_record=terminal_record,
            restored_owner=restored, **details)

    def owned(name, expected, digest, renamed=False):
        return owner.verify_owned_public_file(context, directory, name, expected, digest, renamed=renamed)

    def native_guard():
        admission.verify()
        journal.check()
        for name, fd in context["bound"].items():
            context["revalidate"](context["native"][name], fd)
        for reference, fd in retained_journals.values():
            context["revalidate"]({key: reference[key] for key in ("path", "identity")}, fd)
        context["revalidate"](context["native"]["directory"], context["directory"], True)
        context["revalidate"](request["destination"]["directory"], directory, True)
        need(context["identity"](os.fstat(context["lock"])) == context.get("lock_identity")
             == context["identity"](os.stat(".taira-native-nginx-check.lock", dir_fd=context["directory"],
                                            follow_symlinks=False)), "check_lock_identity_changed")
        need(owner.observe_master(request["master"]) == request["master"], "master_identity_changed")

    def immutable_publisher(reference, template):
        # A crash can occur after our exact owned terminal append but before
        # its completion-journal acknowledgement. Admit only that one suffix
        # over the immutable original public prefix, then use the shared owner
        # decoder for the complete chain. No alternate NDJSON decoder exists.
        owner.validate_public_journal_reference(reference, context["native"])
        opened = open_anchored(reference["path"])
        context["handles"].append(opened)
        observed = context["identity"](os.fstat(opened))
        stable = set(reference["identity"]) - {"size", "mtime_ns", "ctime_ns"}
        need(all(observed[key] == reference["identity"][key] for key in stable), "publisher_journal_changed")
        body = os.pread(opened, MAX_PUBLIC_BYTES+1, 0)
        prefix_size = int(reference["identity"]["size"])
        need(len(body) <= MAX_PUBLIC_BYTES and len(body) >= prefix_size
             and hashlib.sha256(body[:prefix_size]).hexdigest() == reference["sha256"],
             "publisher_journal_prefix_changed")
        suffix = ((json.dumps(template, sort_keys=True) + "\n").encode() if template is not None else b"")
        need(body[prefix_size:] in ({b"", suffix} if suffix else {b""}), "publisher_journal_suffix_changed")
        need(observed == context["identity"](os.fstat(opened))
             == context["identity"](os.stat(reference["path"], follow_symlinks=False)),
             "publisher_journal_changed")
        actual = dict(path=reference["path"], identity=observed, sha256=hashlib.sha256(body).hexdigest())
        retained, records = owner.read_owned_publication_journal(actual, request, context, destination)
        retain_journal(actual, retained)
        return actual, records, bool(body[prefix_size:])

    def append_publisher(reference, template):
        native_guard()
        actual, records, appended = immutable_publisher(reference, template)
        if records[-1] == template:
            return actual
        if not appended:
            need(template["sequence"] == len(records)+1, "publisher_terminal_sequence")
            opened = os.open(Path(reference["path"]).name, os.O_WRONLY | os.O_APPEND | os.O_NOFOLLOW | os.O_CLOEXEC,
                             dir_fd=context["directory"])
            context["handles"].append(opened)
            need(context["identity"](os.fstat(opened)) == actual["identity"], "publisher_journal_changed")
            data = (json.dumps(template, sort_keys=True) + "\n").encode()
            need(len(data) <= 65536 and int(actual["identity"]["size"]) + len(data) <= MAX_PUBLIC_BYTES,
                 "publisher_terminal_size_bound")
            need(os.write(opened, data) == len(data), "publisher_terminal_write_incomplete")
            os.fsync(opened)
            actual, records, appended = immutable_publisher(reference, template)
        need(appended and records[-1] == template, "publisher_terminal_changed")
        return actual

    try:
        native_guard()
        last = journal.records[-1] if journal.records else None
        if last is not None:
            need(last["global_proof_sha256"] == (admission.packet["fence"]["value"]["reference"]["reference"]["sha256"]
                 if admission.packet["fence"]["kind"] == "present" else None), "completion_fence_changed")
            need(not (action == "rollback" and any(row["action"] != "rollback" for row in journal.records))
                 and not (action != "rollback" and any(row["action"] == "rollback" for row in journal.records)),
                 "completion_action_order")
            need(not (action == "seal" and any(row["action"] == "cleanup" for row in journal.records)),
                 "completion_action_order")
            publisher = last["publisher_journal"]
            active, backup = last["publication_identity"], last["backup_identity"]
            restored = last["restored_owner"]
            terminal_record = last["publisher_terminal_record"]
            if restored is not None:
                fields(restored, {"owner", "terminal_record"}, "restored_owner_fields")
                owner.validate_public_prior_reference(restored["owner"], context["native"])
        # Terminal owner acknowledgement can be resumed from its exact suffix.
        if action == "rollback" and terminal_record is not None:
            publisher, records, _ = immutable_publisher(prior["journal"], terminal_record)
        else:
            retained, records = owner.read_owned_publication_journal(publisher, request, context, destination)
            retain_journal(publisher, retained)
        original = records[0].get("prior")
        if restored is not None and restored["terminal_record"] is not None:
            restored_reference, _, _ = immutable_publisher(restored["owner"]["journal"],
                                                          restored["terminal_record"])
            restored["owner"]["journal"] = restored_reference
        if last is None or not any(row["phase"] == "admitted" for row in journal.records):
            active = prior["publication"]["identity"]
            need(records[-1]["phase"] == "awaiting_readiness" and records[-1]["qualified"] is False
                 and records[-1]["candidate_sha256"] == request["candidate_sha256"]
                 and records[-1]["renderer_source_sha256"] == request["renderer_source_sha256"]
                 and records[-1]["master"] == request["master"]
                 and records[-1]["publication_identity"] == active, "completion_owner_binding")
            owned(basename, active, request["candidate_sha256"])
            if original is not None:
                # The maintained owner decoder admits the full public ancestor
                # contract before any ancestor file can be opened.
                retained, previous = owner.read_owned_publication_journal(original["journal"], request, context, destination)
                retain_journal(original["journal"], retained)
                need(previous[-1]["phase"] == "awaiting_readiness"
                     and previous[-1]["master"] == request["master"], "restored_owner_binding")
                _, backup = owned(backup_name, records[-1]["backup_identity"], original["publication"]["sha256"])
                restored = dict(owner=original, terminal_record=None)
            journal_effect("admitted")
            last = journal.records[-1]
        if action == "seal":
            need(last["phase"] in {"admitted", "seal_requested", "sealed", "recovery_pending"}, "seal_phase")
            owned(basename, active, request["candidate_sha256"])
            if backup is not None:
                owned(backup_name, backup, original["publication"]["sha256"])
            native_guard()
            if last["phase"] != "sealed":
                journal_effect("seal_requested")
                native_context_check(context, request, owner, includes=True)
                owned(basename, active, request["candidate_sha256"])
                native_guard()
                journal_effect("sealed")
            receipt.update(exit_code=0, completion_status="sealed")
            return
        if action == "cleanup":
            need(any(row["phase"] == "sealed" for row in journal.records)
                 and last["phase"] in {"sealed", "cleanup_requested", "cleaned", "recovery_pending"}, "cleanup_before_seal")
            owned(basename, active, request["candidate_sha256"])
            native_guard()
            if last["phase"] != "cleaned":
                cleanup_was_requested = any(row["phase"] == "cleanup_requested" for row in journal.records)
                if not cleanup_was_requested and backup is not None:
                    owned(backup_name, backup, original["publication"]["sha256"])
                if last["phase"] not in {"cleanup_requested", "recovery_pending"}:
                    journal_effect("cleanup_requested")
                if backup is not None:
                    try:
                        owned(backup_name, backup, original["publication"]["sha256"])
                    except FileNotFoundError:
                        need(cleanup_was_requested, "cleanup_backup_missing")
                    else:
                        native_guard()
                        owned(backup_name, backup, original["publication"]["sha256"])
                        os.unlink(backup_name, dir_fd=directory)
                        os.fsync(directory)
                owned(basename, active, request["candidate_sha256"])
                native_guard()
                journal_effect("cleaned")
            receipt.update(exit_code=0, completion_status="cleaned")
            return

        need(last["phase"] in {"admitted", "rollback_requested", "source_restored", "reload_requested",
             "publisher_terminal_requested", "publisher_terminal", "rolled_back", "recovery_pending"}, "rollback_phase")
        master_handle = owner.open_master_handle(request["master"])
        # Ignore only our own uncertain marker when recovering an interrupted
        # action. The last durable intent remains the authority for effects.
        effective = next(row for row in reversed(journal.records) if row["phase"] != "recovery_pending")
        phase = effective["phase"]
        if phase == "admitted":
            owned(basename, active, request["candidate_sha256"])
            if backup is not None:
                owned(backup_name, backup, original["publication"]["sha256"])
            native_guard()
            journal_effect("rollback_requested")
            phase = "rollback_requested"
        if phase == "rollback_requested":
            if original is not None:
                # A retained intent admits both exact inode arrangements. The
                # exchange can be observed after a crash without doing it twice.
                try:
                    owned(basename, active, request["candidate_sha256"])
                    owned(backup_name, backup, original["publication"]["sha256"])
                    was_active = True
                except RuntimeError:
                    owned(basename, backup, original["publication"]["sha256"], renamed=True)
                    owned(backup_name, active, request["candidate_sha256"], renamed=True)
                    was_active = False
                if was_active:
                    native_guard()
                    owned(basename, active, request["candidate_sha256"])
                    owned(backup_name, backup, original["publication"]["sha256"])
                    owner.native_exchange(directory, basename, backup_name)
                    os.fsync(directory)
                _, restored_identity = owned(basename, backup, original["publication"]["sha256"], renamed=True)
                _, removed_identity = owned(backup_name, active, request["candidate_sha256"], renamed=True)
                active, backup = restored_identity, removed_identity
                retained, previous = owner.read_owned_publication_journal(original["journal"], request, context, destination)
                retain_journal(original["journal"], retained)
                restored["terminal_record"] = dict(previous[-1], sequence=len(previous)+1,
                    phase="awaiting_readiness", publication_identity=active, cause="native_completion_restored")
            else:
                try:
                    owned(basename, active, request["candidate_sha256"])
                except FileNotFoundError:
                    pass
                else:
                    native_guard()
                    owned(basename, active, request["candidate_sha256"])
                    os.unlink(basename, dir_fd=directory)
                    os.fsync(directory)
                active = None
            native_guard()
            journal_effect("source_restored")
            phase = "source_restored"
        if phase in {"source_restored", "reload_requested"}:
            if active is not None:
                owned(basename, active, original["publication"]["sha256"])
            native_guard()
            native_context_check(context, request, owner, includes=active is not None)
            native_guard()
            if phase != "reload_requested":
                journal_effect("reload_requested")
            owner.signal_master(request["master"], master_handle)
            native_guard()
            # The original publisher remains public, unqualified provenance;
            # its terminal annotation prevents future blind unresolved-owner
            # refusal after a controlled rollback.
            terminal_record = dict(records[-1], sequence=len(records)+1,
                phase="rolled_back_unqualified", cause="native_completion_rollback")
            journal_effect("publisher_terminal_requested")
            phase = "publisher_terminal_requested"
        if phase == "publisher_terminal_requested":
            publisher = append_publisher(prior["journal"], terminal_record)
            if restored is not None:
                restored_journal = append_publisher(restored["owner"]["journal"], restored["terminal_record"])
                restored["owner"]["journal"] = restored_journal
                restored["owner"]["publication"]["identity"] = active
                original["journal"] = restored_journal
                original["publication"]["identity"] = active
            journal_effect("publisher_terminal")
            phase = "publisher_terminal"
        if phase == "publisher_terminal":
            if backup is not None:
                try:
                    owned(backup_name, backup, request["candidate_sha256"])
                except FileNotFoundError:
                    pass
                else:
                    native_guard()
                    owned(backup_name, backup, request["candidate_sha256"])
                    os.unlink(backup_name, dir_fd=directory)
                    os.fsync(directory)
            journal_effect("rolled_back")
        if active is not None:
            owned(basename, active, original["publication"]["sha256"])
        native_guard()
        observed_restored = None
        if restored is not None:
            restored_owner = restored["owner"]
            retained, previous = owner.read_owned_publication_journal(
                restored_owner["journal"], request, context, destination)
            retain_journal(restored_owner["journal"], retained)
            need(previous[-1] == restored["terminal_record"]
                 and previous[-1]["publication_identity"] == active, "restored_terminal_binding")
            def numeric_public(path, observed, digest):
                return dict(file=dict(path=path, identity={key: int(value) for key, value in observed.items()}),
                            sha256=digest)
            observed_restored = dict(operation_id=restored_owner["operation_id"],
                journal=numeric_public(restored_owner["journal"]["path"], restored_owner["journal"]["identity"],
                                       restored_owner["journal"]["sha256"]),
                publication=numeric_public(destination, active, original["publication"]["sha256"]))
            native_guard()
        receipt.update(exit_code=0, completion_status="rolled_back", restored_owned_publication=observed_restored)
    except Exception as error:
        code = error.args[0] if type(error) is RuntimeError and error.args else "native_completion_refused"
        code = code if isinstance(code, str) and re.fullmatch(r"[a-z_]{1,128}", code) else "native_completion_refused"
        try:
            journal_effect("recovery_pending", error_code=code)
        except Exception:
            pass
        receipt.update(exit_code=1, completion_status="recovery_pending", error_code=code)
    finally:
        if master_handle is not None:
            os.close(master_handle)


def native_context_check(context, request, owner, *, includes):
    """Private nginx streams pass directly through bounded native consumers."""
    native = context["native"]
    environment = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    inherited = (context["lock"], context["directory"], *context["bound"].values())
    status = subprocess.run([native["nginx"]["path"], "-t", "-q", "-c", native["main"]["path"]],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        pass_fds=inherited, env=environment, timeout=45).returncode
    need(status == 0, "completion_native_context_rejected")
    destination = str(Path(request["destination"]["directory"]["path"]) / request["destination"]["basename"])
    # The maintained source guard's sole expected-count predicate is made
    # explicit for a first-publication rollback that restores absence.
    source_guard = owner.SOURCE_GUARD.replace("count!=1", "count!=expected_count")
    child = subprocess.Popen([native["nginx"]["path"], "-T", "-q", "-c", native["main"]["path"]],
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        pass_fds=inherited, env=environment)
    guard = None
    try:
        guard = subprocess.Popen([native["awk"]["path"], "-v", "expected=" + destination,
            "-v", "expected_count=" + str(int(includes)), source_guard], stdin=child.stdout,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, pass_fds=inherited, env=environment)
        child.stdout.close()
        guard_status = guard.wait(timeout=45)
        child_status = child.wait(timeout=45)
        need(guard_status == child_status == 0, "completion_native_source_rejected")
    finally:
        for owned in (guard, child):
            if owned is not None and owned.poll() is None:
                owned.kill()
                owned.wait()


def read_request_fd(fd, limit):
    descriptor_value(fd)
    info = os.fstat(fd)
    need(stat.S_ISREG(info.st_mode) and 0 < info.st_size <= limit
         and info.st_uid == os.geteuid() and info.st_nlink == 1 and not info.st_mode & 0o077,
         "request_descriptor_custody")
    observed = identity(info)
    body = os.pread(fd, limit+1, 0)
    need(identity(os.fstat(fd)) == observed, "request_descriptor_changed")
    return public_json(body, limit)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    entry = parser.add_mutually_exclusive_group(required=True)
    entry.add_argument("--admission-fd", type=int, help="Direct native Rust parent's retained admission descriptor")
    entry.add_argument("--inspect-request-fd", type=int, help="Retained public request for a read-only owner observation")
    args = parser.parse_args(argv)
    try:
        result = (complete(read_request_fd(args.admission_fd, 64 * 1024)) if args.admission_fd is not None
                  else inspect_owned_publication(read_request_fd(args.inspect_request_fd, 64 * 1024)))
        print(json.dumps(result, sort_keys=True, separators=(",", ":")))
        return int(result.get("status") == "recovery_pending")
    except Exception as error:
        code = error.args[0] if type(error) is RuntimeError and error.args else "native_completion_failed"
        code = code if isinstance(code, str) and re.fullmatch(r"[a-z_]{1,128}", code) else "native_completion_failed"
        print(json.dumps(dict(error_code=code), separators=(",", ":")))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
