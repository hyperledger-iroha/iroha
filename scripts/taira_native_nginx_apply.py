#!/usr/bin/env python3
"""Create, replace or reconcile one owned public nginx include natively.

Requires an explicit approved MacStadium route, native identities, fresh nginx
master metadata and an explicit publication kind. Replacement pins the prior
unqualified owner journal and public inode; native exchange retains that inode
for exact-own rollback. Existing private config/key bytes remain native.
An owner-private atomic-chain host journal precedes publication and records every
phase. An exact-master reload signal remains unqualified until API checks.
Inspect an interrupted operation's durable journal before attempting recovery;
a new operation ID cannot retry an unresolved publication for that destination.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import inspect
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys

import taira_native_nginx_check as checked

PLAN_SCHEMA = "iroha.taira.native-nginx-apply.plan.v1"
RECEIPT_SCHEMA = "iroha.taira.native-nginx-apply.receipt.v1"
JOURNAL_SCHEMA = "iroha.taira.native-nginx-apply.journal.v1"
SOURCE_GUARD = r'''
BEGIN { count=0; bytes=0; bad=0 }
{ bytes+=length($0)+1; if (bytes>16777216 || length($0)>262144) { bad=1; exit 41 }
  if ($0 == "# configuration file " expected ":") count++ }
END { if (bad || count!=1) exit 41 }
'''


def validate_plan(value: object) -> dict:
    """Admit one explicit publication and fresh public master identity."""
    checked.require(isinstance(value, dict) and set(value) == {
        "schema", "provider", "host_kind", "deployment_reference", "native", "candidate",
        "renderer_source", "operation_id", "destination", "master", "publication"}, "apply_plan_fields")
    base = {name: item for name, item in value.items() if name not in {"operation_id", "destination", "master", "publication"}}
    base["schema"] = checked.PLAN_SCHEMA
    checked.validate_plan(base)
    checked.require(value["schema"] == PLAN_SCHEMA and re.fullmatch(r"[0-9a-f]{32}", value["operation_id"])
                    is not None, "apply_operation_identity")
    destination = value["destination"]
    checked.require(isinstance(destination, dict) and set(destination) == {"directory", "basename"}, "destination_fields")
    directory = destination["directory"]
    checked.require(isinstance(directory, dict) and set(directory) == {"path", "identity"}, "destination_directory")
    checked.canonical_path(directory["path"])
    identity = directory["identity"]
    checked.require(isinstance(identity, dict) and set(identity) == {"device", "inode", "uid", "gid", "mode"}
                    and all(isinstance(item, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", item)
                            for item in identity.values()), "destination_identity")
    checked.require(isinstance(destination["basename"], str)
                    and re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,127}\.conf", destination["basename"])
                    is not None, "scoped_destination_name")
    checked.require(Path(directory["path"]).parent == Path(value["native"]["directory"]["path"]),
                    "scoped_include_directory")
    master = value["master"]
    master_keys = {"pid", "uid", "started", "executable"}
    if value["host_kind"] == "linux":
        master_keys |= {"start_ticks", "boot_id"}
    checked.require(isinstance(master, dict) and set(master) == master_keys
                    and type(master["pid"]) is int and 1 < master["pid"] <= 2147483647
                    and type(master["uid"]) is int and master["uid"] == value["native"]["owner_uid"]
                    and isinstance(master["started"], str) and re.fullmatch(r"[A-Za-z0-9: ]{20,32}", master["started"])
                    is not None and master["executable"] == value["native"]["nginx"]["path"], "master_identity")
    if value["host_kind"] == "linux":
        checked.require(isinstance(master["start_ticks"], str)
                        and re.fullmatch(r"[1-9][0-9]{0,19}", master["start_ticks"]) is not None
                        and isinstance(master["boot_id"], str)
                        and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", master["boot_id"])
                        is not None, "linux_master_start_identity")
    publication = value["publication"]
    checked.require(isinstance(publication, dict) and publication.get("kind") in {"create", "replace", "reconcile"},
                    "publication_kind")
    checked.require(set(publication) == ({"kind"} if publication["kind"] == "create" else {"kind", "prior"}),
                    "publication_fields")
    if publication["kind"] != "create":
        prior = publication["prior"]
        checked.require(isinstance(prior, dict) and set(prior) == {"operation_id", "journal", "publication"}
                        and isinstance(prior["operation_id"], str)
                        and re.fullmatch(r"[0-9a-f]{32}", prior["operation_id"]) is not None,
                        "prior_owner_fields")
        checked.require((prior["operation_id"] == value["operation_id"]) == (publication["kind"] == "reconcile"),
                        "prior_operation_identity")
        journal = prior["journal"]
        expected = str(Path(value["native"]["directory"]["path"]) /
                       (".taira-native-nginx-apply-" + prior["operation_id"] + ".receipt.ndjson"))
        checked.require(isinstance(journal, dict) and set(journal) == {"path", "identity", "sha256"}
                        and journal["path"] == expected, "prior_journal_reference")
        old = prior["publication"]
        checked.require(isinstance(old, dict) and set(old) == {"identity", "sha256"}, "prior_publication_reference")
        keys = {"device", "inode", "uid", "gid", "mode", "links", "size", "mtime_ns", "ctime_ns"}
        for reference in (journal, old):
            observed = reference["identity"]
            checked.require(isinstance(observed, dict) and set(observed) == keys
                            and all(isinstance(item, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", item)
                                    for item in observed.values())
                            and observed["uid"] == str(value["native"]["owner_uid"])
                            and observed["mode"] == "384" and observed["links"] == "1"
                            and 0 < int(observed["size"]) <= checked.MAX_PUBLIC_BYTES
                            and isinstance(reference["sha256"], str)
                            and re.fullmatch(r"[0-9a-f]{64}", reference["sha256"]) is not None,
                            "prior_owned_identity")
    return value


def native_exchange(directory: int, first: str, second: str) -> None:
    """Atomically exchange two anchored names, retaining both original inodes."""
    import ctypes
    library = ctypes.CDLL(None, use_errno=True)
    name = "renameatx_np" if sys.platform == "darwin" else "renameat2"
    operation = getattr(library, name, None)
    if operation is None:
        raise RuntimeError("native_exchange_unavailable")
    operation.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    operation.restype = ctypes.c_int
    # Darwin RENAME_SWAP and Linux RENAME_EXCHANGE are both exactly 2.
    if operation(directory, os.fsencode(first), directory, os.fsencode(second), 2) != 0:
        raise RuntimeError("native_exchange_failed")


def native_publish_no_replace(directory: int, source: str, destination: str) -> None:
    """Publish a complete single-link journal without replacing any name."""
    import ctypes
    library = ctypes.CDLL(None, use_errno=True)
    operation = getattr(library, "renameatx_np" if sys.platform == "darwin" else "renameat2", None)
    _owned_need(operation is not None, "native_publish_unavailable")
    operation.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    operation.restype = ctypes.c_int
    # Darwin RENAME_EXCL=4; Linux RENAME_NOREPLACE=1.
    _owned_need(operation(directory, os.fsencode(source), directory, os.fsencode(destination),
                          4 if sys.platform == "darwin" else 1) == 0, "native_publish_failed")


def write_all_owned(opened: int, payload: bytes) -> None:
    """Complete bounded native regular-file writes, including EINTR/short writes."""
    _owned_need(isinstance(payload, bytes) and len(payload) <= 1048576, "owned_write_size_bound")
    offset = 0
    while offset < len(payload):
        try:
            written = os.write(opened, payload[offset:])
        except InterruptedError:
            continue
        _owned_need(type(written) is int and 0 < written <= len(payload)-offset, "owned_write_incomplete")
        offset += written


def public_journal_record_projection(record: dict) -> dict:
    """Compare canonical phase semantics independently of its created inode."""
    return {key: value for key, value in record.items()
            if key not in {"journal_publication_identity", "journal_write_intent"}}


def read_public_journal_write_intent(reference, request, context, operation, sequence,
                                    prefix, projection, created):
    """Join the independently published immutable created-stage witness."""
    need, native = _owned_need, context["native"]
    need(isinstance(reference, dict) and set(reference) == {"path", "identity", "sha256"}
         and isinstance(reference["path"], str), "journal_intent_reference")
    basename = Path(reference["path"]).name
    need(re.fullmatch(r"\.taira-native-nginx-write-"+operation+"-"+str(sequence)
         +r"-[0-9a-f]{32}\.intent\.json", basename) is not None
         and reference["path"] == str(Path(native["directory"]["path"]) / basename),
         "journal_intent_reference")
    validate_public_owner_identity(reference["identity"], native)
    need(int(reference["identity"]["size"]) <= 65536
         and isinstance(reference["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", reference["sha256"]),
         "journal_intent_reference")
    before = len(context["handles"])
    try:
        opened = context["open_bound"]({key: reference[key] for key in ("path", "identity")})
        body = os.pread(opened, 65537, 0)
        need(len(body) <= 65536 and hashlib.sha256(body).hexdigest() == reference["sha256"],
             "journal_intent_changed")
        context["revalidate"]({key: reference[key] for key in ("path", "identity")}, opened)
        def unique(entries):
            result = {}
            for key, value in entries:
                need(key not in result, "journal_intent_invalid")
                result[key] = value
            return result
        intent = json.loads(body, object_pairs_hook=unique)
        need(isinstance(intent, dict) and set(intent) == {"schema", "operation_id", "sequence", "journal_path",
             "staging_basename", "created_identity", "predecessor", "record_sha256"}
             and intent["schema"] == "iroha.taira.native-nginx-write.intent.v1"
             and intent["operation_id"] == operation and type(intent["sequence"]) is int
             and intent["sequence"] == sequence and intent["created_identity"] == created
             and intent["journal_path"] == str(Path(native["directory"]["path"]) /
                 (".taira-native-nginx-apply-"+operation+".receipt.ndjson"))
             and isinstance(intent["staging_basename"], str)
             and re.fullmatch(r"\.taira-nginx-journal-"+operation+r"-[0-9a-f]{32}\.stage",
                              intent["staging_basename"]) is not None
             and intent["record_sha256"] == hashlib.sha256(json.dumps(projection, sort_keys=True).encode()).hexdigest(),
             "journal_intent_binding_changed")
        if sequence == 1:
            need(intent["predecessor"] is None and not prefix, "journal_intent_prefix_changed")
        else:
            previous = intent["predecessor"]
            need(validate_public_journal_reference(previous, native) == operation
                 and previous["path"] == intent["journal_path"]
                 and int(previous["identity"]["size"]) == len(prefix)
                 and previous["sha256"] == hashlib.sha256(prefix).hexdigest(), "journal_intent_prefix_changed")
        return intent
    finally:
        # Every opened duplicate belongs only to this bounded admission read.
        # The writer retains its separately created intent FD through effects.
        for retained in context["handles"][before:]:
            os.close(retained)
        del context["handles"][before:]


def append_owned_publication_record(context, request, reference, opened, rows, record, effect_guard, prepared_write):
    """Atomically append one canonical row while preserving its exact raw prefix.

    The final row names this newly created stage inode before any bytes are
    written or published. The sole decoder joins that identity to the current
    canonical inode after a lost acknowledgment; a same-content foreign copy
    cannot become this owner. Retained predecessor descriptors survive native
    exchange/unlink, with only their controlled ctime/link changes permitted.
    """
    import stat
    import uuid
    need, identity = _owned_need, context["identity"]
    directory, native = context["directory"], context["native"]
    operation = record.get("operation_id")
    need(isinstance(operation, str) and re.fullmatch(r"[0-9a-f]{32}", operation), "journal_operation_identity")
    name = ".taira-native-nginx-apply-" + operation + ".receipt.ndjson"
    path = str(Path(native["directory"]["path"]) / name)
    need(callable(effect_guard) and (reference is None) == (opened is None)
         and isinstance(rows, list) and len(rows) < 128
         and record.get("sequence") == len(rows)+1, "journal_record_bound")

    def native_guard():
        for key, retained in context["bound"].items():
            context["revalidate"](native[key], retained)
        context["revalidate"](native["directory"], directory, True)
        need(identity(os.fstat(context["lock"])) == context["lock_identity"]
             == identity(os.stat(".taira-native-nginx-check.lock", dir_fd=directory,
                                 follow_symlinks=False)), "check_lock_identity_changed")

    def bound_file(basename, fd, expected, *, renamed=False):
        current = identity(os.fstat(fd))
        stable = set(expected) - ({"ctime_ns"} if renamed else set())
        need(stat.S_ISREG(os.fstat(fd).st_mode)
             and all(current[key] == expected[key] for key in stable)
             and current == identity(os.stat(basename, dir_fd=directory, follow_symlinks=False))
             and current["uid"] == str(native["owner_uid"]) and current["mode"] == "384"
             and current["links"] == "1", "journal_identity_changed")
        return current

    native_guard()
    prefix = b""
    if reference is not None:
        need(validate_public_journal_reference(reference, native) == operation
             and reference["path"] == path, "journal_operation_identity")
        context["revalidate"]({key: reference[key] for key in ("path", "identity")}, opened)
        prefix = os.pread(opened, 1048577, 0)
        need(len(prefix) == int(reference["identity"]["size"])
             and len(prefix) <= 1048576 and prefix.endswith(b"\n")
             and hashlib.sha256(prefix).hexdigest() == reference["sha256"], "journal_prefix_changed")
        decoded_fd, decoded = read_owned_publication_journal(reference, request, context,
                                                           record["destination_path"])
        need(decoded == rows, "journal_prefix_changed")
        context["revalidate"]({key: reference[key] for key in ("path", "identity")}, decoded_fd)
    else:
        need(not rows, "journal_prefix_changed")
        try:
            os.stat(name, dir_fd=directory, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise RuntimeError("journal_already_exists")

    projection = public_journal_record_projection(record)
    if prepared_write is None:
        temporary = ".taira-nginx-journal-" + operation + "-" + uuid.uuid4().hex + ".stage"
        stage_fd = os.open(temporary, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                           0o600, dir_fd=directory)
        context["handles"].append(stage_fd)
        created = identity(os.fstat(stage_fd))
        need(stat.S_ISREG(os.fstat(stage_fd).st_mode) and created["uid"] == str(native["owner_uid"])
             and created["mode"] == "384" and created["links"] == "1"
             and created == identity(os.stat(temporary, dir_fd=directory, follow_symlinks=False)),
             "journal_stage_changed")
        created_binding = {key: created[key] for key in ("device", "inode", "uid", "gid", "mode")}
        intent_name = ".taira-native-nginx-write-"+operation+"-"+str(len(rows)+1)+"-"+uuid.uuid4().hex+".intent.json"
        intent_fd = os.open(intent_name+".stage", os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                            0o600, dir_fd=directory)
        context["handles"].append(intent_fd)
        intent_created = identity(os.fstat(intent_fd))
        need(stat.S_ISREG(os.fstat(intent_fd).st_mode) and intent_created["uid"] == str(native["owner_uid"])
             and intent_created["mode"] == "384" and intent_created["links"] == "1"
             and intent_created == identity(os.stat(intent_name+".stage", dir_fd=directory, follow_symlinks=False)),
             "journal_intent_changed")
        intent = dict(schema="iroha.taira.native-nginx-write.intent.v1", operation_id=operation,
            sequence=len(rows)+1, journal_path=path, staging_basename=temporary, created_identity=created_binding,
            predecessor=reference, record_sha256=hashlib.sha256(json.dumps(projection, sort_keys=True).encode()).hexdigest())
        intent_body = json.dumps(intent, sort_keys=True).encode()+b"\n"
        need(len(intent_body) <= 65536, "journal_intent_size_bound")
        write_all_owned(intent_fd, intent_body)
        os.fsync(intent_fd)
        intent_staged = identity(os.fstat(intent_fd))
        need(all(intent_staged[key] == intent_created[key] for key in ("device", "inode", "uid", "gid", "mode", "links"))
             and os.pread(intent_fd, 65537, 0) == intent_body, "journal_intent_changed")
        bound_file(intent_name+".stage", intent_fd, intent_staged)
        native_guard()
        native_publish_no_replace(directory, intent_name+".stage", intent_name)
        os.fsync(directory)
        intent_identity = bound_file(intent_name, intent_fd, intent_staged, renamed=True)
        intent_reference = dict(path=str(Path(native["directory"]["path"]) / intent_name), identity=intent_identity,
                               sha256=hashlib.sha256(intent_body).hexdigest())
        committed = dict(record, journal_publication_identity=created_binding, journal_write_intent=intent_reference)
        suffix = (json.dumps(committed, sort_keys=True) + "\n").encode()
        need(len(suffix) <= 65536 and len(prefix)+len(suffix) <= 1048576, "journal_record_size_bound")
        chain = prefix + suffix
        write_all_owned(stage_fd, chain)
        os.fsync(stage_fd)
        staged = identity(os.fstat(stage_fd))
        need(all(staged[key] == created[key] for key in ("device", "inode", "uid", "gid", "mode", "links"))
             and int(staged["size"]) == len(chain), "journal_stage_changed")
    else:
        need(isinstance(prepared_write, dict) and set(prepared_write) == {"intent", "stage", "record"}
             and prepared_write["record"] == projection, "journal_prepared_write_changed")
        intent_reference = prepared_write["intent"]
        staged = prepared_write["stage"]
        validate_public_owner_identity(staged, native)
        created_binding = {key: staged[key] for key in ("device", "inode", "uid", "gid", "mode")}
        intent = read_public_journal_write_intent(intent_reference, request, context, operation,
            len(rows)+1, prefix, projection, created_binding)
        need(intent["predecessor"] == reference, "journal_prepared_prefix_changed")
        temporary = intent["staging_basename"]
        stage_fd = context["open_bound"](dict(path=str(Path(native["directory"]["path"])/temporary), identity=staged))
        chain = os.pread(stage_fd, 1048577, 0)
        committed = dict(record, journal_publication_identity=created_binding, journal_write_intent=intent_reference)
        suffix = (json.dumps(committed, sort_keys=True)+"\n").encode()
        need(len(suffix) <= 65536 and len(chain) <= 1048576 and chain == prefix+suffix,
             "journal_prepared_write_changed")
    bound_file(temporary, stage_fd, staged)
    need(os.pread(stage_fd, 1048577, 0) == chain, "journal_stage_changed")
    need(decode_owned_publication_records(chain, operation, request, staged,
          record["destination_path"], context) == rows+[committed], "journal_record_changed")
    bound_file(temporary, stage_fd, staged)
    os.fsync(directory)
    native_guard()
    if reference is not None:
        bound_file(name, opened, reference["identity"])
        need(os.pread(opened, 1048577, 0) == prefix, "journal_prefix_changed")
    bound_file(temporary, stage_fd, staged)
    # The caller owns its admitted master/host lease/progress/fence and runs
    # their last complete guard before the first canonical journal effect.
    # Its old journal references become historical after this atomic effect;
    # only the exact owner/stage guards below run until it registers our result.
    effect_guard(intent_reference, staged, projection)
    native_guard()
    if reference is not None:
        bound_file(name, opened, reference["identity"])
        need(os.pread(opened, 1048577, 0) == prefix, "journal_prefix_changed")
    bound_file(temporary, stage_fd, staged)
    read_public_journal_write_intent(intent_reference, request, context, operation, len(rows)+1,
                                    prefix, projection, created_binding)
    if reference is None:
        native_publish_no_replace(directory, temporary, name)
    else:
        native_exchange(directory, temporary, name)
    os.fsync(directory)
    observed = bound_file(name, stage_fd, staged, renamed=True)
    need(os.pread(stage_fd, 1048577, 0) == chain, "journal_stage_changed")
    observed = bound_file(name, stage_fd, observed)
    if reference is not None:
        displaced = bound_file(temporary, opened, reference["identity"], renamed=True)
        need(os.pread(opened, 1048577, 0) == prefix, "journal_prefix_changed")
        bound_file(temporary, opened, displaced)
        native_guard()
        bound_file(name, stage_fd, observed)
        bound_file(temporary, opened, displaced)
        os.unlink(temporary, dir_fd=directory)
        os.fsync(directory)
        after = identity(os.fstat(opened))
        need(all(after[key] == displaced[key] for key in set(displaced)-{"links", "ctime_ns"})
             and after["links"] == "0" and os.pread(opened, 1048577, 0) == prefix,
             "journal_predecessor_changed")
    native_guard()
    observed = bound_file(name, stage_fd, observed)
    result = dict(path=path, identity=observed, sha256=hashlib.sha256(chain).hexdigest())
    final_fd, final_rows = read_owned_publication_journal(result, request, context, record["destination_path"])
    need(final_rows == rows+[committed], "journal_record_changed")
    context["revalidate"]({key: result[key] for key in ("path", "identity")}, final_fd)
    return result, stage_fd, committed


def observe_master(expected: dict) -> dict:
    """Observe only UID/start time and executable metadata, never command tokens."""
    result = subprocess.run(["/bin/ps", "-p", str(expected["pid"]), "-o", "uid=", "-o", "lstart="],
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"}, timeout=10)
    fields = result.stdout.decode().split()
    if result.returncode or len(fields) != 6:
        raise RuntimeError("master_identity_changed")
    if sys.platform == "darwin":
        result = subprocess.run(["/usr/sbin/lsof", "-nP", "-a", "-p", str(expected["pid"]), "-d", "txt", "-Fn"],
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=10)
        paths = {line[1:] for line in result.stdout.decode().splitlines() if line.startswith("n")}
        if result.returncode or expected["executable"] not in paths:
            raise RuntimeError("master_executable_changed")
    elif os.readlink("/proc/" + str(expected["pid"]) + "/exe") != expected["executable"]:
        raise RuntimeError("master_executable_changed")
    observed = dict(pid=expected["pid"], uid=int(fields[0]), started=" ".join(fields[1:]),
                    executable=expected["executable"])
    if sys.platform != "darwin":
        process = Path("/proc") / str(expected["pid"])
        # These are public native process identity records, never config bodies.
        metadata = (process / "stat").read_bytes()
        if len(metadata) > 8192 or b") " not in metadata:
            raise RuntimeError("master_identity_changed")
        tail = metadata.rsplit(b") ", 1)[1].split()
        if len(tail) < 20 or not tail[19].isdigit():
            raise RuntimeError("master_identity_changed")
        observed.update(start_ticks=tail[19].decode(),
                        boot_id=Path("/proc/sys/kernel/random/boot_id").read_text().strip())
    return observed


def open_master_handle(expected: dict):
    """Require a retained Linux process handle before admitting publication."""
    if sys.platform == "darwin":
        return None
    if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
        raise RuntimeError("master_handle_unavailable")
    try:
        return os.pidfd_open(expected["pid"], 0)
    except OSError as error:
        raise RuntimeError("master_handle_unavailable") from None


def signal_master(expected: dict, retained) -> str:
    """Signal only the adjacent revalidated approved positive nginx master."""
    if type(expected.get("pid")) is not int or not 1 < expected["pid"] <= 2147483647:
        raise RuntimeError("master_signal_target")
    if observe_master(expected) != expected:
        raise RuntimeError("master_identity_changed")
    try:
        if retained is not None:
            signal.pidfd_send_signal(retained, signal.SIGHUP, None, 0)
            mode = "pidfd"
        else:
            if sys.platform != "darwin":
                raise RuntimeError("master_handle_unavailable")
            os.kill(expected["pid"], signal.SIGHUP)
            mode = "adjacent_native_identity"
        if observe_master(expected) != expected:
            raise RuntimeError("reload_signal_ambiguous")
    except Exception:
        raise RuntimeError("reload_signal_ambiguous") from None
    return mode


def _owned_need(condition, code):
    """Preserve one closed ownership refusal across native owner operations."""
    if not condition:
        raise RuntimeError(code)


def verify_owned_public_file(context, destination_directory, name, expected, digest, *, renamed=False):
    """Retain and recheck only a journal-proven public renderer inode."""
    import stat
    need = _owned_need
    owner = context["native"]["owner_uid"]
    identity = context["identity"]
    opened = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK,
                     dir_fd=destination_directory)
    context["handles"].append(opened)
    observed = identity(os.fstat(opened))
    # A controlled native rename may update ctime, but never the identity,
    # public bytes or mtime. Full observed identity is retained afterward.
    keys = set(expected) - ({"ctime_ns"} if renamed else set())
    need(stat.S_ISREG(os.fstat(opened).st_mode) and set(observed) == set(expected)
         and all(observed[key] == expected[key] for key in keys)
         and observed["uid"] == str(owner) and observed["links"] == "1"
         and observed["mode"] == "384" and int(observed["size"]) <= 1048576,
         "owned_public_identity_changed")
    need(identity(os.stat(name, dir_fd=destination_directory, follow_symlinks=False)) == observed,
         "owned_public_identity_changed")
    # Only a journal-proven maintained renderer publication is read here.
    payload = os.pread(opened, 1048577, 0)
    need(hashlib.sha256(payload).hexdigest() == digest and len(payload) <= 1048576
         and identity(os.fstat(opened)) == observed
         == identity(os.stat(name, dir_fd=destination_directory, follow_symlinks=False)),
         "owned_public_content_changed")
    return opened, observed


def validate_public_journal_reference(reference, native):
    """Admit a closed maintained public metadata filename before any file read."""
    need = _owned_need
    need(isinstance(reference, dict) and set(reference) == {"path", "identity", "sha256"},
         "prior_journal_reference")
    candidate = reference["path"]
    need(isinstance(candidate, str), "prior_journal_reference")
    match = re.fullmatch(r"\.taira-native-nginx-apply-([0-9a-f]{32})\.receipt\.ndjson", Path(candidate).name)
    need(match is not None and candidate == str(Path(native["directory"]["path"]) / Path(candidate).name),
         "prior_journal_reference")
    validate_public_owner_identity(reference["identity"], native)
    need(isinstance(reference["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", reference["sha256"]) is not None,
         "prior_journal_reference")
    return match.group(1)


def validate_public_owner_identity(observed, native):
    """Require the original owner-private single-link public record layout."""
    need = _owned_need
    keys = {"device", "inode", "uid", "gid", "mode", "links", "size", "mtime_ns", "ctime_ns"}
    need(isinstance(observed, dict) and set(observed) == keys
         and all(isinstance(item, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", item) is not None
                 and int(item) <= 18446744073709551615 for item in observed.values())
         and observed["uid"] == str(native["owner_uid"]) and observed["mode"] == "384"
         and observed["links"] == "1" and 0 < int(observed["size"]) <= 1048576,
         "prior_owned_identity")


def validate_public_prior_reference(prior, native):
    """Admit every ancestor's complete public owner envelope without opening it."""
    need = _owned_need
    need(isinstance(prior, dict) and set(prior) == {"operation_id", "journal", "publication"}
         and isinstance(prior["operation_id"], str) and re.fullmatch(r"[0-9a-f]{32}", prior["operation_id"]) is not None,
         "prior_owner_fields")
    need(validate_public_journal_reference(prior["journal"], native) == prior["operation_id"],
         "prior_operation_identity")
    publication = prior["publication"]
    need(isinstance(publication, dict) and set(publication) == {"identity", "sha256"}, "prior_publication_reference")
    validate_public_owner_identity(publication["identity"], native)
    need(isinstance(publication["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", publication["sha256"]) is not None,
         "prior_publication_reference")


def read_owned_publication_journal(reference, request, context, destination_path):
    """Admit the same bounded immutable owner chain for apply and native capture."""
    need = _owned_need
    native = context["native"]
    owner = native["owner_uid"]
    open_bound, revalidate = context["open_bound"], context["revalidate"]
    expected_operation = validate_public_journal_reference(reference, native)
    opened = open_bound({key: reference[key] for key in ("path", "identity")})
    need(reference["identity"]["uid"] == str(owner) and reference["identity"]["mode"] == "384"
         and reference["identity"]["links"] == "1"
         and 0 < int(reference["identity"]["size"]) <= 1048576, "unsafe_prior_public_journal")
    data = os.pread(opened, 1048577, 0)
    need(hashlib.sha256(data).hexdigest() == reference["sha256"] and len(data) <= 1048576
         and data.endswith(b"\n"), "prior_journal_digest_changed")
    revalidate({key: reference[key] for key in ("path", "identity")}, opened)
    records = decode_owned_publication_records(data, expected_operation, request,
                                              reference["identity"], destination_path, context)
    return opened, records


def decode_owned_publication_records(data, expected_operation, request, journal_identity, destination_path, context):
    """The sole bounded canonical row decoder, also used before publication."""
    need = _owned_need
    native, destination = request["native"], request["destination"]
    need(isinstance(data, bytes) and 0 < len(data) <= 1048576 and data.endswith(b"\n"),
         "prior_journal_digest_changed")
    def unique_members(entries):
        result = {}
        for name, item in entries:
            need(name not in result, "prior_journal_duplicate_member")
            result[name] = item
        return result
    records = [json.loads(line, object_pairs_hook=unique_members) for line in data.splitlines()]
    need(0 < len(records) <= 128, "prior_journal_record_bound")
    base = {"schema", "sequence", "operation_id", "phase", "qualified", "candidate_sha256",
            "renderer_source_sha256", "host_kind", "native_executable", "main_configuration", "master",
            "destination_path", "destination_directory", "publication_identity", "journal_publication_identity",
            "journal_write_intent"}
    extras = {"prior", "temporary_basename", "cause", "nginx_exit_code", "source_guard_exit_code",
              "reload_signal_mode", "backup_identity", "restored_owned_publication"}
    phases = {"prepared", "publishing", "published", "context_checking", "context_checked", "reload_requested",
              "awaiting_readiness", "rollback_requested", "rollback_source_removed", "rollback_reload_requested",
              "rolled_back_unqualified", "rollback_ambiguous", "refused_before_publication"}
    first = records[0]
    raw_lines = data.splitlines(keepends=True)
    for ordinal, record in enumerate(records, 1):
        need(isinstance(record, dict) and base <= set(record) <= base | extras
             and record["schema"] == "iroha.taira.native-nginx-apply.journal.v1"
             and type(record["sequence"]) is int and record["sequence"] == ordinal
             and record["operation_id"] == expected_operation and record["qualified"] is False
             and record["phase"] in phases and record["host_kind"] == request["host_kind"]
             and record["destination_path"] == destination_path
             and record["destination_directory"] == destination["directory"]
             and record["native_executable"] == native["nginx"]
             and record["main_configuration"] == native["main"], "prior_journal_invalid")
        need(all(isinstance(record[name], str) and re.fullmatch(r"[0-9a-f]{64}", record[name]) is not None
                 for name in ("candidate_sha256", "renderer_source_sha256")), "prior_journal_invalid")
        staged = record["journal_publication_identity"]
        need(isinstance(staged, dict) and set(staged) == {"device", "inode", "uid", "gid", "mode"}
             and all(isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", value) is not None
                     and int(value) <= 18446744073709551615 for value in staged.values())
             and staged["uid"] == str(native["owner_uid"]) and staged["mode"] == "384",
             "prior_journal_stage_identity")
        if "prior" in record:
            validate_public_prior_reference(record["prior"], native)
        witness = read_public_journal_write_intent(record["journal_write_intent"], request, context,
            expected_operation, ordinal, b"".join(raw_lines[:ordinal-1]),
            public_journal_record_projection(record), staged)
        if ordinal > 1:
            need(all(witness["predecessor"]["identity"][key] == records[ordinal-2]["journal_publication_identity"][key]
                     for key in ("device", "inode", "uid", "gid", "mode")), "journal_intent_prefix_changed")
        for name in ("candidate_sha256", "renderer_source_sha256", "master", "prior"):
            need(record.get(name) == first.get(name), "prior_journal_binding_changed")
    need(all(records[-1]["journal_publication_identity"][key] == journal_identity[key]
             for key in ("device", "inode", "uid", "gid", "mode")), "prior_journal_created_inode_changed")
    return records


def inspect_owned_publication_in_context(receipt: dict, request: dict, context: dict) -> None:
    """Capture exact public ownership under the same native lock without an effect."""
    need = _owned_need
    prior = request["publication"]["prior"]
    need(request["publication"]["kind"] == "reconcile"
         and prior["operation_id"] == request["operation_id"], "inspection_owner_identity")
    directory = context["open_bound"](request["destination"]["directory"], True)
    destination = str(Path(request["destination"]["directory"]["path"]) / request["destination"]["basename"])
    journal_fd, records = read_owned_publication_journal(prior["journal"], request, context, destination)
    last = records[-1]
    need(last["phase"] == "awaiting_readiness" and last["qualified"] is False
         and last["master"] == request["master"]
         and last["candidate_sha256"] == request["candidate_sha256"]
         and last["renderer_source_sha256"] == request["renderer_source_sha256"]
         and last["publication_identity"] == prior["publication"]["identity"]
         and prior["publication"]["sha256"] == request["candidate_sha256"],
         "inspection_publication_binding")
    publication_fd, publication_identity = verify_owned_public_file(
        context, directory, request["destination"]["basename"],
        prior["publication"]["identity"], prior["publication"]["sha256"])
    need(observe_master(request["master"]) == request["master"], "master_identity_changed")
    for name, opened in context["bound"].items():
        context["revalidate"](context["native"][name], opened)
    context["revalidate"](context["native"]["directory"], context["directory"], True)
    context["revalidate"](request["destination"]["directory"], directory, True)
    context["revalidate"]({name: prior["journal"][name] for name in ("path", "identity")}, journal_fd)
    verify_owned_public_file(context, directory, request["destination"]["basename"],
                             publication_identity, prior["publication"]["sha256"])
    need(context["identity"](os.fstat(publication_fd)) == publication_identity
         and context["identity"](os.stat(".taira-native-nginx-check.lock", dir_fd=context["directory"],
             follow_symlinks=False)) == context["identity"](os.fstat(context["lock"])) == context["lock_identity"],
         "inspection_owner_changed")
    need(observe_master(request["master"]) == request["master"], "master_identity_changed")

    def public_reference(path, identity, digest):
        # Native JSON u64 values never pass through a JavaScript number.
        return dict(file=dict(path=path, identity={key: int(value) for key, value in identity.items()}),
                    sha256=digest)

    receipt.update(exit_code=0, operation_id=request["operation_id"], phase=last["phase"],
        owned_publication_checked=True, master=request["master"],
        journal=public_reference(prior["journal"]["path"], prior["journal"]["identity"], prior["journal"]["sha256"]),
        publication=public_reference(destination, publication_identity, prior["publication"]["sha256"]),
        native_inputs={name: dict(path=context["native"][name]["path"],
            identity={key: int(value) for key, value in context["identity"](os.fstat(opened)).items()})
            for name, opened in context["bound"].items()})


def inspect_owned_publication(request: dict) -> dict:
    """Native capture uses the maintained owner contract, never its own journal decoder."""
    _owned_need(isinstance(request, dict) and set(request) == {
        "host_kind", "native", "candidate_sha256", "candidate_base64", "renderer_source_sha256",
        "master", "destination", "operation_id", "publication"}, "inspection_request_fields")
    # This validates the existing owner's sole canonical wire. These public
    # placeholder paths are never opened; the actual inherited native input
    # metadata and journal/publication references remain the authority.
    validate_plan(dict(schema=PLAN_SCHEMA, provider="macstadium-dublin",
        host_kind=request["host_kind"], native=request["native"],
        deployment_reference=dict(path="/native-inspection/deployment.json", sha256="0" * 64),
        candidate=dict(path="/native-inspection/candidate.conf", owner_uid=request["native"]["owner_uid"],
                       sha256=request["candidate_sha256"]),
        renderer_source=dict(path="/native-inspection/renderer.py", sha256=request["renderer_source_sha256"]),
        master=request["master"], destination=request["destination"],
        operation_id=request["operation_id"], publication=request["publication"]))
    result = checked.remote_check(request, inspect_owned_publication_in_context)
    _owned_need(result.get("exit_code") == 0
                and result.get("owned_publication_checked") is True
                and result.get("validation_files_removed") is True
                and result.get("private_config_read_by_controller") is False,
                "owned_publication_inspection_refused")
    return dict(schema="iroha.taira.native-nginx-owned-publication-inspection.v1",
        owned_publication=dict(operation_id=result["operation_id"],
            journal=result["journal"], publication=result["publication"]),
        nginx=result["native_inputs"]["nginx"],
        main_configuration=result["native_inputs"]["main"],
        master=result["master"], phase=result["phase"])


def check_owned_public_context(request: dict, context: dict, destination_directory: int, publication_fd: int):
    """Keep the actual full private -T stream between native nginx and awk."""
    native = context["native"]
    lock, main_directory = context["lock"], context["directory"]
    destination_path = str(Path(request["destination"]["directory"]["path"]) / request["destination"]["basename"])
    environment = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    # The private -T stream moves from nginx directly to bounded native awk.
    # Python retains pipe handles but never reads that stream.
    child = subprocess.Popen([native["nginx"]["path"], "-T", "-q", "-c", native["main"]["path"]],
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        pass_fds=(lock, main_directory, destination_directory, publication_fd), env=environment)
    guard = None
    try:
        guard = subprocess.Popen([native["awk"]["path"], "-v", "expected=" + destination_path, SOURCE_GUARD],
            stdin=child.stdout, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            pass_fds=(lock, main_directory, destination_directory), env=environment)
        child.stdout.close()
        guard_status = guard.wait(timeout=45)
        native_status = child.wait(timeout=45)
        return native_status, guard_status
    finally:
        for owned in (guard, child):
            if owned is not None and owned.poll() is None:
                owned.kill()
                owned.wait()



def numeric_owned_reference(operation, journal, publication, destination):
    """Project native decimal metadata directly to the canonical numeric wire."""
    def public(path, observed, digest):
        return dict(file=dict(path=path, identity={key: int(value) for key, value in observed.items()}), sha256=digest)
    return dict(operation_id=operation,
        journal=public(journal["path"], journal["identity"], journal["sha256"]),
        publication=public(destination, publication["identity"], publication["sha256"]))


def rollback_interrupted_in_context(receipt, request, context, reference=None, records=None, journal_fd=None):
    """Restore a proven predecessor directly, without candidate admission/HUP."""
    import stat
    need = _owned_need
    native, original = context["native"], request["publication"]
    predecessor = original.get("prior")
    destination = request["destination"]
    directory = context["open_bound"](destination["directory"], True)
    basename = destination["basename"]
    path = str(Path(destination["directory"]["path"]) / basename)
    backup = ".taira-nginx-backup-" + request["operation_id"] + ".public"
    owner = native["owner_uid"]
    current_reference = reference
    master_handle = None
    restored = None
    if records is not None:
        last = records[-1]
        need(last["phase"] in {"prepared", "publishing", "published", "context_checking", "context_checked",
            "reload_requested", "awaiting_readiness", "refused_before_publication", "rollback_requested",
            "rollback_source_removed", "rollback_reload_requested", "rolled_back_unqualified"}, "recovery_pending")
        need(last["candidate_sha256"] == request["candidate_sha256"]
             and last["renderer_source_sha256"] == request["renderer_source_sha256"]
             and last["master"] == request["master"] and records[0].get("prior") == predecessor,
             "interrupted_plan_binding")
    receipt.update(operation_id=request["operation_id"], publication_kind=original["kind"], destination_path=path,
        qualified=False, configuration_published=False, reload_requested=False, recovery_pending=True)

    def revalidate():
        for key, opened in context["bound"].items():
            context["revalidate"](native[key], opened)
        context["revalidate"](native["directory"], context["directory"], True)
        context["revalidate"](destination["directory"], directory, True)
        if current_reference is not None:
            context["revalidate"]({key: current_reference[key] for key in ("path", "identity")}, journal_fd)
        need(context["identity"](os.stat(".taira-native-nginx-check.lock", dir_fd=context["directory"],
            follow_symlinks=False)) == context["identity"](os.fstat(context["lock"])) == context["lock_identity"],
            "check_lock_identity_changed")
        need(observe_master(request["master"]) == request["master"], "master_identity_changed")

    def exists(name):
        try:
            os.stat(name, dir_fd=directory, follow_symlinks=False)
            return True
        except FileNotFoundError:
            return False

    def owned(name, expected, digest, *, transitioned=False):
        need(isinstance(expected, dict), "interrupted_publication_identity")
        opened = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=directory)
        context["handles"].append(opened)
        observed = context["identity"](os.fstat(opened))
        ignored = {"ctime_ns", "links"} if transitioned else set()
        need(stat.S_ISREG(os.fstat(opened).st_mode)
             and all(observed[key] == expected[key] for key in set(expected) - ignored)
             and observed["uid"] == str(owner) and observed["mode"] == "384"
             and observed["links"] in ({"1", "2"} if transitioned else {"1"})
             and observed == context["identity"](os.stat(name, dir_fd=directory, follow_symlinks=False)),
             "interrupted_publication_changed")
        body = os.pread(opened, 1048577, 0)
        need(len(body) <= 1048576 and hashlib.sha256(body).hexdigest() == digest
             and observed == context["identity"](os.fstat(opened))
             == context["identity"](os.stat(name, dir_fd=directory, follow_symlinks=False)),
             "interrupted_publication_changed")
        return opened, observed

    def append(reference, opened, rows, record):
        revalidate()
        record = dict(record, sequence=len(rows)+1)
        updated, successor_fd, committed = append_owned_publication_record(
            context, request, reference, opened, rows, record, lambda *_: revalidate(), None)
        rows.append(committed)
        return updated, successor_fd

    def journal(phase, **details):
        nonlocal current_reference, journal_fd
        need(current_reference is not None, "rollback_journal_missing")
        record = dict(records[-1], phase=phase, **details)
        current_reference, journal_fd = append(current_reference, journal_fd, records, record)
        receipt.update(journal_path=current_reference["path"], journal_identity=current_reference["identity"],
                       journal_sequence=len(records), phase=phase,
                       journal_write_intent=dict(file=dict(path=records[-1]["journal_write_intent"]["path"],
                           identity={key:int(value) for key,value in records[-1]["journal_write_intent"]["identity"].items()}),
                           sha256=records[-1]["journal_write_intent"]["sha256"]))

    def prior_journal():
        # A previous exact rollback may already have appended the refreshed
        # predecessor row. Its original immutable prefix must still hash to
        # the signed input; the sole owner decoder admits the complete chain.
        reference = predecessor["journal"]
        validate_public_journal_reference(reference, native)
        observed = context["identity"](os.stat(reference["path"], follow_symlinks=False))
        validate_public_owner_identity(observed, native)
        opened = context["open_bound"](dict(path=reference["path"], identity=observed))
        body = os.pread(opened, 1048577, 0)
        context["revalidate"](dict(path=reference["path"], identity=observed), opened)
        size = int(reference["identity"]["size"])
        need(len(body) <= 1048576 and len(body) >= size
             and hashlib.sha256(body[:size]).hexdigest() == reference["sha256"], "restored_journal_prefix_changed")
        actual = dict(path=reference["path"], identity=observed, sha256=hashlib.sha256(body).hexdigest())
        opened, rows = read_owned_publication_journal(actual, request, context, path)
        return actual, opened, rows, size

    def native_context(publication_fd):
        revalidate()
        if predecessor is not None:
            native_status, source_status = check_owned_public_context(request, context, directory, publication_fd)
            need(native_status == source_status == 0, "rollback_context_rejected")
        else:
            native_status = subprocess.run([native["nginx"]["path"], "-t", "-q", "-c", native["main"]["path"]],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                pass_fds=(context["lock"], context["directory"], directory),
                env={"PATH":"/usr/bin:/bin", "LC_ALL":"C"}, timeout=45).returncode
            need(native_status == 0, "rollback_context_rejected")
        revalidate()
        receipt.update(rollback_context_exit_code=native_status, nginx_exit_code=0,
            host_kind=request["host_kind"], owner_uid=owner, native_executable=native["nginx"],
            main_configuration=native["main"], configuration_directory=native["directory"],
            candidate_sha256=request["candidate_sha256"], renderer_source_sha256=request["renderer_source_sha256"],
            inherited_configuration_checked=True, relative_include_prefix_preserved=True)

    try:
        revalidate()
        master_handle = open_master_handle(request["master"])
        stage = records[-1]["publication_identity"] if records is not None else None
        candidate_at_destination = False
        candidate_name = None
        candidate_snapshot = None
        if predecessor is not None:
            old = predecessor["publication"]
            if exists(basename):
                observed = context["identity"](os.stat(basename, dir_fd=directory, follow_symlinks=False))
                candidate_at_destination = stage is not None and all(observed[key] == stage[key] for key in ("device", "inode"))
            if candidate_at_destination:
                _, candidate_snapshot = owned(basename, stage, request["candidate_sha256"], transitioned=True)
                restored_fd, restored_snapshot = owned(backup, old["identity"], old["sha256"], transitioned=True)
                need(candidate_snapshot["links"] == restored_snapshot["links"] == "1", "interrupted_link_pair_changed")
            else:
                was_rollback = records is not None and any(row["phase"].startswith("rollback_")
                    or row["phase"] == "rolled_back_unqualified" for row in records)
                restored_fd, restored_snapshot = owned(basename, old["identity"], old["sha256"], transitioned=was_rollback)
                need(restored_snapshot["links"] == "1", "interrupted_link_pair_changed")
                if stage is None and exists(backup):
                    raise RuntimeError("interrupted_stage_unowned")
                if stage is not None and exists(backup):
                    _, candidate_snapshot = owned(backup, stage, request["candidate_sha256"], transitioned=True)
                    need(candidate_snapshot["links"] == "1", "interrupted_link_pair_changed")
                    candidate_name = backup
        else:
            need(not exists(basename) or stage is not None, "interrupted_unowned_destination")
            if exists(basename):
                _, candidate_snapshot = owned(basename, stage, request["candidate_sha256"], transitioned=True)
                candidate_at_destination = True
            restored_fd = None
            restored_snapshot = None
        if records is not None and stage is not None and original["kind"] == "create":
            publishing = next((row for row in records if row["phase"] == "publishing"), None)
            need(publishing is not None and isinstance(publishing.get("temporary_basename"), str)
                 and re.fullmatch(r"\.taira-nginx-publish-[0-9a-f]{32}\.candidate", publishing["temporary_basename"]),
                 "interrupted_stage_name")
            temporary = publishing["temporary_basename"]
            if exists(temporary):
                _, staged = owned(temporary, stage, request["candidate_sha256"], transitioned=True)
                if candidate_snapshot is not None and candidate_snapshot["links"] == "2":
                    need(staged == candidate_snapshot, "interrupted_link_pair_changed")
                else:
                    need(staged["links"] == "1" and not candidate_at_destination, "interrupted_stage_ambiguous")
                candidate_name = temporary
                candidate_snapshot = staged
            elif candidate_snapshot is not None:
                need(candidate_snapshot["links"] == "1", "interrupted_stage_ambiguous")
        receipt["configuration_published"] = candidate_at_destination
        if reference is None:
            native_context(restored_fd)
            if predecessor is not None:
                actual, opened, rows, prefix_size = prior_journal()
                need(actual == predecessor["journal"] and rows[-1]["phase"] == "awaiting_readiness"
                     and rows[-1]["publication_identity"] == restored_snapshot, "incumbent_owner_changed")
                restored = numeric_owned_reference(predecessor["operation_id"], actual, old, path)
            intended_journal = ".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson"
            try:
                os.stat(intended_journal, dir_fd=context["directory"], follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                raise RuntimeError("interrupted_journal_already_exists")
            revalidate()
            receipt.update(exit_code=0, phase="not_requested", journal=None, restored_owned_publication=restored)
            receipt.pop("recovery_pending", None)
            return
        terminal = records[-1]["phase"] == "rolled_back_unqualified"
        if not terminal and records[-1]["phase"] not in {"rollback_requested", "rollback_source_removed", "rollback_reload_requested"}:
            journal("rollback_requested", cause="exact_operation_rollback")
        revalidate()
        if candidate_at_destination:
            _, checked_candidate = owned(basename, candidate_snapshot, request["candidate_sha256"], transitioned=True)
            need(checked_candidate == candidate_snapshot, "interrupted_publication_changed")
            if predecessor is not None:
                owned(backup, restored_snapshot, old["sha256"])
                revalidate()
                native_exchange(directory, backup, basename)
                os.fsync(directory)
                restored_fd, restored_snapshot = owned(basename, restored_snapshot, old["sha256"], transitioned=True)
                _, candidate_snapshot = owned(backup, candidate_snapshot, request["candidate_sha256"], transitioned=True)
                candidate_name = backup
            else:
                revalidate()
                os.unlink(basename, dir_fd=directory)
                os.fsync(directory)
                if candidate_name is not None:
                    _, candidate_snapshot = owned(candidate_name, stage, request["candidate_sha256"], transitioned=True)
        elif predecessor is not None:
            restored_fd, restored_snapshot = owned(basename, restored_snapshot, old["sha256"])
        else:
            need(not exists(basename), "interrupted_unowned_destination")
        receipt["configuration_published"] = False
        if not terminal:
            journal("rollback_source_removed", cause="exact_operation_rollback")
        native_context(restored_fd)
        if predecessor is not None:
            owned(basename, restored_snapshot, old["sha256"])
        else:
            need(not exists(basename), "interrupted_unowned_destination")
        # Only an actual/previously ambiguous candidate publication requests an
        # original-context HUP. A prepared/no-effect rollback never reloads.
        needs_reload = candidate_at_destination or any(row["phase"] in {"reload_requested", "awaiting_readiness",
            "rollback_reload_requested"} for row in records)
        if needs_reload and not terminal:
            journal("rollback_reload_requested", cause="exact_operation_rollback")
            receipt["reload_requested"] = True
            revalidate()
            if predecessor is not None:
                owned(basename, restored_snapshot, old["sha256"])
            mode = signal_master(request["master"], master_handle)
            receipt["rollback_reload_signal_mode"] = mode
            revalidate()
        if predecessor is not None:
            actual, opened, rows, prefix_size = prior_journal()
            prefix = os.pread(opened, prefix_size, 0)
            prefix_count = len(prefix.splitlines())
            need(rows[prefix_count-1]["phase"] == "awaiting_readiness"
                 and rows[prefix_count-1]["publication_identity"] == old["identity"], "restored_owner_binding")
            template = dict(rows[prefix_count-1], sequence=prefix_count+1,
                            publication_identity=restored_snapshot, cause="exact_operation_rollback_restore")
            if restored_snapshot == old["identity"]:
                need(len(rows) == prefix_count, "restored_journal_unrelated_append")
            elif len(rows) == prefix_count:
                actual, opened = append(actual, opened, rows, template)
            else:
                need(len(rows) == prefix_count+1 and public_journal_record_projection(rows[-1])
                     == public_journal_record_projection(template), "restored_journal_unrelated_append")
            context["revalidate"]({key: actual[key] for key in ("path", "identity")}, opened)
            owned(basename, restored_snapshot, old["sha256"])
            restored = numeric_owned_reference(predecessor["operation_id"], actual,
                dict(identity=restored_snapshot, sha256=old["sha256"]), path)
        if candidate_name is not None and not terminal:
            revalidate()
            _, cleanup_snapshot = owned(candidate_name, candidate_snapshot, request["candidate_sha256"], transitioned=True)
            need(cleanup_snapshot["links"] == "1", "interrupted_link_pair_changed")
            owned(candidate_name, cleanup_snapshot, request["candidate_sha256"])
            os.unlink(candidate_name, dir_fd=directory)
            os.fsync(directory)
        if not terminal:
            journal("rolled_back_unqualified", cause="exact_operation_rollback", restored_owned_publication=restored)
        else:
            need(records[-1].get("restored_owned_publication") == restored, "restored_owner_changed")
        receipt.update(exit_code=0, phase="rolled_back_unqualified", configuration_published=False,
            journal=dict(file=dict(path=current_reference["path"],
                identity={key:int(value) for key,value in current_reference["identity"].items()}),
                sha256=current_reference["sha256"]), restored_owned_publication=restored)
        witness = records[-1]["journal_write_intent"]
        receipt["journal_write_intent"] = dict(file=dict(path=witness["path"],
            identity={key:int(value) for key,value in witness["identity"].items()}), sha256=witness["sha256"])
        receipt.pop("recovery_pending", None)
    finally:
        if master_handle is not None:
            os.close(master_handle)


def capture_interrupted_journal(receipt, request, context):
    """Capture the complete current public journal while its owner lock is held."""
    observed = receipt["journal_identity"]
    path = str(Path(context["native"]["directory"]["path"]) / (
        ".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson"))
    opened = context["open_bound"](dict(path=path, identity=observed))
    body = os.pread(opened, 1048577, 0)
    _owned_need(len(body) <= 1048576 and body.endswith(b"\n"), "journal_record_size_bound")
    context["revalidate"](dict(path=path, identity=observed), opened)
    records = decode_owned_publication_records(body, request["operation_id"], request, observed,
        str(Path(request["destination"]["directory"]["path"])/request["destination"]["basename"]), context)
    receipt["journal"] = dict(file=dict(path=path, identity={key:int(value) for key,value in observed.items()}),
                              sha256=hashlib.sha256(body).hexdigest())
    witness = records[-1]["journal_write_intent"]
    receipt["journal_write_intent"] = dict(file=dict(path=witness["path"],
        identity={key:int(value) for key,value in witness["identity"].items()}), sha256=witness["sha256"])


def reconcile_interrupted_in_context(receipt: dict, request: dict, context: dict) -> None:
    """Resume only the original operation's journal-proven public arrangement."""
    import stat

    need = _owned_need
    native, original = context["native"], request["publication"]
    reference = request["interrupted_journal"]
    name = ".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson"
    if reference is None:
        try:
            os.stat(name, dir_fd=context["directory"], follow_symlinks=False)
        except FileNotFoundError:
            if request["recovery_direction"] == "rollback":
                rollback_interrupted_in_context(receipt, request, context)
            else:
                apply_in_context(receipt, request, context)
                capture_interrupted_journal(receipt, request, context)
            return
        raise RuntimeError("interrupted_journal_already_exists")
    need(validate_public_journal_reference(reference, native) == request["operation_id"],
         "interrupted_operation_identity")
    destination = request["destination"]
    directory = context["open_bound"](destination["directory"], True)
    basename = destination["basename"]
    destination_path = str(Path(destination["directory"]["path"]) / basename)
    journal_fd, records = read_owned_publication_journal(reference, request, context, destination_path)
    last = records[-1]
    if request["recovery_direction"] == "rollback":
        rollback_interrupted_in_context(receipt, request, context, reference, records, journal_fd)
        return
    predecessor = original.get("prior")
    need(last["candidate_sha256"] == request["candidate_sha256"]
         and last["renderer_source_sha256"] == request["renderer_source_sha256"]
         and last["master"] == request["master"]
         and records[0].get("prior") == predecessor, "interrupted_plan_binding")
    need(last["phase"] in {"publishing", "published", "context_checking", "context_checked",
                           "reload_requested", "awaiting_readiness"}, "recovery_pending")
    receipt.update(operation_id=request["operation_id"], destination_path=destination_path,
        journal_path=reference["path"], journal_identity=reference["identity"],
        journal_sequence=len(records), phase=last["phase"], publication_kind=original["kind"],
        configuration_published=False, reload_requested=False, qualified=False, recovery_pending=True)
    retained = []
    if predecessor is not None:
        ancestor = predecessor
        seen = set()
        for _ in range(32):
            validate_public_prior_reference(ancestor, native)
            need(ancestor["operation_id"] not in seen, "prior_journal_chain_invalid")
            seen.add(ancestor["operation_id"])
            opened, rows = read_owned_publication_journal(ancestor["journal"], request, context, destination_path)
            need(rows[-1]["phase"] == "awaiting_readiness"
                 and rows[-1]["publication_identity"] == ancestor["publication"]["identity"]
                 and rows[-1]["candidate_sha256"] == ancestor["publication"]["sha256"],
                 "prior_publication_binding")
            retained.append((ancestor["journal"], opened))
            ancestor = rows[0].get("prior")
            if ancestor is None:
                break
        else:
            raise RuntimeError("prior_journal_chain_bound")

    def revalidate():
        for key, opened in context["bound"].items():
            context["revalidate"](native[key], opened)
        context["revalidate"](native["directory"], context["directory"], True)
        context["revalidate"](destination["directory"], directory, True)
        context["revalidate"]({key: reference[key] for key in ("path", "identity")}, journal_fd)
        for ancestor, opened in retained:
            context["revalidate"]({key: ancestor[key] for key in ("path", "identity")}, opened)
        need(context["identity"](os.stat(".taira-native-nginx-check.lock", dir_fd=context["directory"],
            follow_symlinks=False)) == context["identity"](os.fstat(context["lock"])) == context["lock_identity"],
            "check_lock_identity_changed")
        need(observe_master(request["master"]) == request["master"], "master_identity_changed")

    def public_file(name, expected, digest, *, renamed=False, linked=False):
        # The journal proves this inode contains the declared public renderer.
        # Relax only the ctime/link transition caused by exchange/link/unlink.
        need(isinstance(expected, dict), "interrupted_publication_identity")
        opened = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=directory)
        context["handles"].append(opened)
        observed = context["identity"](os.fstat(opened))
        ignored = ({"ctime_ns"} if renamed else set()) | ({"links", "ctime_ns"} if linked else set())
        need(stat.S_ISREG(os.fstat(opened).st_mode)
             and all(observed[key] == expected[key] for key in set(expected) - ignored)
             and observed["uid"] == str(native["owner_uid"]) and observed["mode"] == "384"
             and observed["links"] in ({"1", "2"} if linked else {"1"})
             and observed == context["identity"](os.stat(name, dir_fd=directory, follow_symlinks=False)),
             "interrupted_publication_changed")
        payload = os.pread(opened, 1048577, 0)
        need(len(payload) <= 1048576 and hashlib.sha256(payload).hexdigest() == digest
             and observed == context["identity"](os.fstat(opened))
             == context["identity"](os.stat(name, dir_fd=directory, follow_symlinks=False)),
             "interrupted_publication_changed")
        return opened, observed

    def exists(name):
        try:
            os.stat(name, dir_fd=directory, follow_symlinks=False)
            return True
        except FileNotFoundError:
            return False

    def lineage(name, expected):
        if not exists(name) or not isinstance(expected, dict):
            return False
        observed = context["identity"](os.stat(name, dir_fd=directory, follow_symlinks=False))
        return all(observed[key] == expected[key] for key in ("device", "inode"))

    revalidate()
    master_handle = open_master_handle(request["master"])
    try:
        stage = last["publication_identity"]
        validate_public_owner_identity(stage, native)
        if last["phase"] == "publishing":
            temporary = last.get("temporary_basename")
            if original["kind"] == "replace":
                need(temporary == ".taira-nginx-backup-" + request["operation_id"] + ".public",
                     "interrupted_stage_name")
                old = predecessor["publication"]
                if lineage(basename, stage):
                    public_file(basename, stage, request["candidate_sha256"], renamed=True)
                    public_file(temporary, old["identity"], old["sha256"], renamed=True)
                else:
                    public_file(basename, old["identity"], old["sha256"])
                    stage_fd, stage_snapshot = public_file(temporary, stage, request["candidate_sha256"])
                    revalidate()
                    public_file(basename, old["identity"], old["sha256"])
                    public_file(temporary, stage_snapshot, request["candidate_sha256"])
                    native_exchange(directory, temporary, basename)
                    os.fsync(directory)
                    public_file(basename, stage_snapshot, request["candidate_sha256"], renamed=True)
                    public_file(temporary, old["identity"], old["sha256"], renamed=True)
            else:
                need(isinstance(temporary, str) and re.fullmatch(r"\.taira-nginx-publish-[0-9a-f]{32}\.candidate", temporary),
                     "interrupted_stage_name")
                if exists(basename):
                    publication_fd, observed = public_file(basename, stage, request["candidate_sha256"], linked=True)
                    if observed["links"] == "2":
                        _, staged = public_file(temporary, stage, request["candidate_sha256"], linked=True)
                        need(staged == observed, "interrupted_link_pair_changed")
                        revalidate()
                        public_file(basename, observed, request["candidate_sha256"], linked=True)
                        public_file(temporary, observed, request["candidate_sha256"], linked=True)
                        os.unlink(temporary, dir_fd=directory)
                        os.fsync(directory)
                    else:
                        need(not exists(temporary), "interrupted_stage_ambiguous")
                else:
                    stage_fd, staged = public_file(temporary, stage, request["candidate_sha256"])
                    revalidate()
                    public_file(temporary, staged, request["candidate_sha256"])
                    os.link(temporary, basename, src_dir_fd=directory, dst_dir_fd=directory, follow_symlinks=False)
                    _, linked = public_file(basename, staged, request["candidate_sha256"], linked=True)
                    _, staged_linked = public_file(temporary, staged, request["candidate_sha256"], linked=True)
                    need(linked == staged_linked and linked["links"] == "2", "interrupted_link_pair_changed")
                    revalidate()
                    public_file(basename, linked, request["candidate_sha256"], linked=True)
                    public_file(temporary, linked, request["candidate_sha256"], linked=True)
                    os.unlink(temporary, dir_fd=directory)
                    os.fsync(directory)
            revalidate()
            _, publication = public_file(basename, stage, request["candidate_sha256"], renamed=True)
        else:
            _, publication = public_file(basename, stage, request["candidate_sha256"])
        resumed = dict(request, publication=dict(kind="reconcile", prior=dict(operation_id=request["operation_id"],
            journal=reference, publication=dict(identity=publication, sha256=request["candidate_sha256"]))))
        if last["phase"] == "awaiting_readiness":
            observed = {}
            inspect_owned_publication_in_context(observed, resumed, context)
            publication_fd, publication = public_file(basename, publication, request["candidate_sha256"])
            native_status, source_status = check_owned_public_context(resumed, context, directory, publication_fd)
            public_file(basename, publication, request["candidate_sha256"])
            revalidate()
            need(native_status == source_status == 0, "active_context_rejected")
            # A completed exact owner is observed, never reloaded or appended.
            receipt.update(configuration_published=True, reload_requested=True,
                active_context_nginx_exit_code=native_status, active_source_guard_exit_code=source_status,
                reload_signal_mode=last.get("reload_signal_mode"), exit_code=0,
                host_kind=request["host_kind"], owner_uid=native["owner_uid"],
                native_executable=native["nginx"], main_configuration=native["main"],
                configuration_directory=native["directory"], candidate_sha256=request["candidate_sha256"],
                renderer_source_sha256=request["renderer_source_sha256"], nginx_exit_code=0,
                inherited_configuration_checked=True, relative_include_prefix_preserved=True)
        else:
            apply_in_context(receipt, resumed, context)
        receipt["publication_kind"] = original["kind"]
        if predecessor is None:
            receipt.pop("prior_operation_id", None)
        else:
            receipt["prior_operation_id"] = predecessor["operation_id"]
        receipt.pop("recovery_pending", None)
        capture_interrupted_journal(receipt, request, context)
    finally:
        # The receipt describes continuation of the immutable original operation,
        # including a refused/ambiguous native gate after ownership admission.
        receipt["publication_kind"] = original["kind"]
        if predecessor is None:
            receipt.pop("prior_operation_id", None)
        else:
            receipt["prior_operation_id"] = predecessor["operation_id"]
        if master_handle is not None:
            os.close(master_handle)


def apply_in_context(receipt: dict, request: dict, context: dict) -> None:
    """Publish/check/reload while the shared native owner remains retained."""
    import stat
    import uuid

    native, bound = context["native"], context["bound"]
    main_directory, lock = context["directory"], context["lock"]
    identity, open_bound, revalidate = context["identity"], context["open_bound"], context["revalidate"]
    owner = native["owner_uid"]
    destination = request["destination"]
    destination_directory = open_bound(destination["directory"], True)
    basename = destination["basename"]
    destination_path = str(Path(destination["directory"]["path"]) / basename)
    journal_name = ".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson"
    journal_path = str(Path(native["directory"]["path"]) / journal_name)
    journal_fd = None
    journal_identity = None
    journal_reference = None
    journal_records = []
    sequence = 0
    publication_fd = None
    publication_identity = None
    stage_identity = None
    temporary = None
    published = False
    kind = request["publication"]["kind"]
    prior = request["publication"].get("prior")
    predecessor = None
    prior_fd = None
    prior_identity = None
    backup_name = ".taira-nginx-backup-" + request["operation_id"] + ".public"
    backup_identity = None
    exchange_attempted = False
    retained_journals = []
    master_handle = None
    environment = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    receipt.update(destination_path=destination_path, journal_path=journal_path,
                   operation_id=request["operation_id"], configuration_published=False,
                   reload_requested=False, qualified=False, publication_kind=kind)
    if prior is not None:
        receipt["prior_operation_id"] = prior["operation_id"]

    def need(condition, code):
        if not condition:
            raise RuntimeError(code)

    def verify_master():
        need(observe_master(request["master"]) == request["master"], "master_identity_changed")

    def verify_native():
        for name, opened in bound.items():
            revalidate(native[name], opened)
        revalidate(native["directory"], main_directory, True)
        revalidate(destination["directory"], destination_directory, True)
        for reference, opened in retained_journals:
            revalidate(reference, opened)
        if backup_identity is not None and published:
            owned_file(backup_name, backup_identity, predecessor["publication"]["sha256"])
            need(identity(os.fstat(prior_fd)) == backup_identity, "rollback_owner_changed")
        need(identity(os.stat(".taira-native-nginx-check.lock", dir_fd=main_directory, follow_symlinks=False))
             == identity(os.fstat(lock)) == context["lock_identity"], "check_lock_identity_changed")
        verify_master()

    def owned_file(name, expected, digest, *, renamed=False):
        return verify_owned_public_file(context, destination_directory, name, expected, digest, renamed=renamed)

    def read_journal(reference):
        return read_owned_publication_journal(reference, request, context, destination_path)

    def journal(phase, **details):
        nonlocal journal_identity, sequence, journal_fd, journal_reference
        record = dict(schema="iroha.taira.native-nginx-apply.journal.v1", sequence=len(journal_records)+1,
            operation_id=request["operation_id"], phase=phase, qualified=False,
            candidate_sha256=request["candidate_sha256"], renderer_source_sha256=request["renderer_source_sha256"],
            host_kind=request["host_kind"], native_executable=native["nginx"], main_configuration=native["main"],
            master=request["master"], destination_path=destination_path,
            destination_directory=destination["directory"], publication_identity=publication_identity,
            **details)
        if predecessor is not None:
            record["prior"] = predecessor
        if backup_identity is not None:
            record["backup_identity"] = backup_identity
        verify_native()
        journal_reference, journal_fd, committed = append_owned_publication_record(
            context, request, journal_reference, journal_fd, journal_records, record, lambda *_: verify_native(), None)
        journal_records.append(committed)
        journal_identity = journal_reference["identity"]
        sequence = len(journal_records)
        receipt.update(journal_sequence=sequence, phase=phase, journal_identity=journal_identity)
        witness = committed["journal_write_intent"]
        receipt["journal_write_intent"] = dict(file=dict(path=witness["path"],
            identity={key:int(value) for key,value in witness["identity"].items()}), sha256=witness["sha256"])

    def verify_publication():
        current = os.open(basename, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK,
                          dir_fd=destination_directory)
        context["handles"].append(current)
        need(stat.S_ISREG(os.fstat(current).st_mode) and identity(os.fstat(current)) == publication_identity
             and identity(os.fstat(publication_fd)) == publication_identity
             == identity(os.stat(basename, dir_fd=destination_directory, follow_symlinks=False)),
             "published_identity_changed")
        payload = os.pread(current, 1048577, 0)
        need(len(payload) <= 1048576 and hashlib.sha256(payload).hexdigest() == request["candidate_sha256"]
             and identity(os.fstat(current)) == publication_identity == identity(os.fstat(publication_fd))
             == identity(os.stat(basename, dir_fd=destination_directory, follow_symlinks=False)),
             "published_content_changed")

    def full_context_check():
        return check_owned_public_context(request, context, destination_directory, publication_fd)

    def native_command(*arguments):
        return subprocess.run([native["nginx"]["path"], *arguments, "-c", native["main"]["path"]],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            pass_fds=(lock, main_directory, destination_directory), env=environment, timeout=45).returncode

    def refuse_unknown_retry(allowed):
        terminal = {"rolled_back_unqualified", "refused_before_publication"}
        with os.scandir(main_directory) as entries:
            names = []
            for entry in entries:
                need(len(names) < 4096, "journal_directory_size_bound")
                names.append(entry.name)
        for name in names:
            if re.fullmatch(r"\.taira-native-nginx-apply-[0-9a-f]{32}\.receipt\.ndjson", name) is None:
                continue
            opened = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK,
                             dir_fd=main_directory)
            context["handles"].append(opened)
            info = os.fstat(opened)
            need(stat.S_ISREG(info.st_mode) and info.st_uid == owner and info.st_nlink == 1
                 and stat.S_IMODE(info.st_mode) == 0o600 and info.st_size <= 1048576,
                 "unsafe_prior_public_journal")
            need(identity(os.stat(name, dir_fd=main_directory, follow_symlinks=False)) == identity(info),
                 "prior_journal_changed")
            # These are this maintained owner's public metadata journals, not
            # existing nginx config/key bodies. Never export their raw bytes.
            rows = os.pread(opened, info.st_size + 1, 0).splitlines()
            need(identity(info) == identity(os.fstat(opened))
                 == identity(os.stat(name, dir_fd=main_directory, follow_symlinks=False)) and rows,
                 "prior_journal_changed")
            record = json.loads(rows[-1])
            need(isinstance(record, dict) and record.get("schema") == "iroha.taira.native-nginx-apply.journal.v1",
                 "prior_journal_invalid")
            if record.get("destination_path") == destination_path and record.get("phase") not in terminal and name not in allowed:
                raise RuntimeError("unresolved_destination_journal")

    try:
        need(os.fstat(destination_directory).st_uid == owner
             and os.fstat(destination_directory).st_dev == os.fstat(main_directory).st_dev, "destination_owner_or_device")
        if kind == "create":
            try:
                os.stat(basename, dir_fd=destination_directory, follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                raise RuntimeError("destination_already_exists")
        else:
            need(kind in {"replace", "reconcile"} and isinstance(prior, dict), "publication_kind")
        verify_native()
        master_handle = open_master_handle(request["master"])
        verify_master()
        allowed = set()
        old_records = None
        if prior is not None:
            need((prior["operation_id"] == request["operation_id"]) == (kind == "reconcile"),
                 "prior_operation_identity")
            expected_journal = str(Path(native["directory"]["path"]) /
                (".taira-native-nginx-apply-" + prior["operation_id"] + ".receipt.ndjson"))
            need(prior["journal"]["path"] == expected_journal, "prior_journal_reference")
            reference = prior["journal"]
            journal_owner, old_records = read_journal(reference)
            last = old_records[-1]
            if kind == "replace":
                need(last["phase"] == "awaiting_readiness" and last["qualified"] is False,
                     "prior_owner_not_awaiting_readiness")
                need(prior["publication"]["identity"] == last["publication_identity"]
                     and prior["publication"]["sha256"] == last["candidate_sha256"], "prior_publication_binding")
                predecessor = prior
            else:
                need(last["phase"] in {"publishing", "published", "context_checking", "context_checked",
                                      "reload_requested", "awaiting_readiness"}, "recovery_pending")
                need(last["candidate_sha256"] == request["candidate_sha256"]
                     and last["renderer_source_sha256"] == request["renderer_source_sha256"]
                     and prior["publication"]["sha256"] == request["candidate_sha256"]
                     and last["master"] == request["master"], "recovery_pending")
                predecessor = old_records[0].get("prior")
                expected = last["publication_identity"]
                need(expected is not None and all(prior["publication"]["identity"][name] == item
                     for name, item in expected.items() if name != "ctime_ns"), "recovery_pending")
            prior_fd, prior_identity = owned_file(basename, prior["publication"]["identity"],
                                                  prior["publication"]["sha256"])
            # A new journal explicitly supersedes the exact retained immutable
            # predecessor chain. Unrelated unresolved owners remain blocking.
            for _ in range(32):
                allowed.add(Path(reference["path"]).name)
                if kind == "reconcile" and reference == prior["journal"]:
                    pass
                else:
                    retained_journals.append(({key: reference[key] for key in ("path", "identity")}, journal_owner))
                ancestor = old_records[0].get("prior")
                if ancestor is None:
                    break
                reference = ancestor["journal"]
                need(Path(reference["path"]).parent == Path(native["directory"]["path"])
                     and Path(reference["path"]).name not in allowed, "prior_journal_chain_invalid")
                journal_owner, old_records = read_journal(reference)
                need(old_records[-1]["phase"] == "awaiting_readiness", "prior_journal_chain_invalid")
            else:
                raise RuntimeError("prior_journal_chain_bound")
            if kind == "reconcile":
                if predecessor is not None:
                    prior_fd, backup_identity = owned_file(backup_name, predecessor["publication"]["identity"],
                        predecessor["publication"]["sha256"], renamed=True)
                    receipt.update(backup_path=str(Path(destination["directory"]["path"]) / backup_name),
                                   backup_identity=backup_identity)
                publication_fd, publication_identity = owned_file(basename, prior["publication"]["identity"],
                                                                   request["candidate_sha256"])
                published = True
        refuse_unknown_retry(allowed)
        if kind == "reconcile":
            journal_reference = prior["journal"]
            journal_fd, journal_records = read_journal(journal_reference)
            journal_identity = journal_reference["identity"]
            sequence = len(journal_records)
            need(sequence + 12 <= 128, "journal_record_bound")
        os.fsync(main_directory)
        if kind != "reconcile":
            journal("prepared")
            temporary = backup_name if kind == "replace" else ".taira-nginx-publish-" + uuid.uuid4().hex + ".candidate"
            publication_fd = os.open(temporary, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                                     0o600, dir_fd=destination_directory)
            context["handles"].append(publication_fd)
            payload = os.pread(context["candidate_fd"], 1048577, 0)
            need(hashlib.sha256(payload).hexdigest() == request["candidate_sha256"], "candidate_digest_changed")
            write_all_owned(publication_fd, payload)
            os.fsync(publication_fd)
            publication_identity = stage_identity = identity(os.fstat(publication_fd))
            os.fsync(destination_directory)
            journal("publishing", temporary_basename=temporary)
            verify_native()
            owned_file(temporary, stage_identity, request["candidate_sha256"])
            need(identity(os.fstat(publication_fd)) == stage_identity, "publication_stage_changed")
            if kind == "replace":
                owned_file(basename, prior_identity, prior["publication"]["sha256"])
                exchange_attempted = True
                native_exchange(destination_directory, temporary, basename)
                published = True
                # Exchange leaves the original inode under a non-conf name.
                # Preserve it even if subsequent identity proof is ambiguous.
                temporary = None
                prior_fd, backup_identity = owned_file(backup_name, prior_identity,
                    prior["publication"]["sha256"], renamed=True)
                receipt.update(backup_path=str(Path(destination["directory"]["path"]) / backup_name),
                               backup_identity=backup_identity)
            else:
                os.link(temporary, basename, src_dir_fd=destination_directory, dst_dir_fd=destination_directory,
                        follow_symlinks=False)
                published = True
                linked = identity(os.fstat(publication_fd))
                keys = set(stage_identity) - {"links", "ctime_ns"}
                need(linked["links"] == "2" and all(linked[name] == stage_identity[name] for name in keys)
                     and linked == identity(os.stat(temporary, dir_fd=destination_directory, follow_symlinks=False))
                     == identity(os.stat(basename, dir_fd=destination_directory, follow_symlinks=False)),
                     "publication_stage_changed")
                os.unlink(temporary, dir_fd=destination_directory)
                temporary = None
            observed = identity(os.fstat(publication_fd))
            need(stat.S_ISREG(os.fstat(publication_fd).st_mode)
                 and all(observed[name] == stage_identity[name] for name in set(stage_identity) - {"ctime_ns"})
                 and observed["uid"] == str(owner) and observed["mode"] == "384" and observed["links"] == "1",
                 "publication_stage_changed")
            publication_identity = observed
            os.fsync(destination_directory)
        receipt["configuration_published"] = True
        journal("published")
        verify_publication()
        verify_native()
        journal("context_checking")
        native_status, source_status = full_context_check()
        receipt.update(active_context_nginx_exit_code=native_status, active_source_guard_exit_code=source_status)
        verify_publication()
        verify_native()
        need(native_status == source_status == 0, "active_context_rejected")
        receipt.update(host_kind=request["host_kind"], owner_uid=owner,
            native_executable=native["nginx"], main_configuration=native["main"],
            configuration_directory=native["directory"], candidate_sha256=request["candidate_sha256"],
            renderer_source_sha256=request["renderer_source_sha256"], nginx_exit_code=0,
            inherited_configuration_checked=True, relative_include_prefix_preserved=True)
        journal("context_checked", nginx_exit_code=native_status, source_guard_exit_code=source_status)
        journal("reload_requested")
        receipt["reload_requested"] = True
        mode = signal_master(request["master"], master_handle)
        receipt["reload_signal_mode"] = mode
        verify_publication()
        verify_native()
        journal("awaiting_readiness", reload_signal_mode=mode)
        receipt["exit_code"] = 0
    except Exception as error:
        code = error.args[0] if type(error) is RuntimeError and error.args else "apply_failed"
        code = code if isinstance(code, str) and re.fullmatch(r"[a-z_]+", code) else "apply_failed"
        if published:
            try:
                journal("rollback_requested", cause=code)
                verify_publication()
                if predecessor is not None:
                    need(backup_identity is not None and prior_fd is not None, "rollback_owner_missing")
                    owned_file(backup_name, backup_identity, predecessor["publication"]["sha256"])
                    need(identity(os.fstat(prior_fd)) == backup_identity, "rollback_owner_changed")
                    verify_native()
                    native_exchange(destination_directory, backup_name, basename)
                    owned_file(basename, backup_identity, predecessor["publication"]["sha256"], renamed=True)
                else:
                    os.unlink(basename, dir_fd=destination_directory)
                os.fsync(destination_directory)
                published = False
                receipt["configuration_published"] = False
                journal("rollback_source_removed", cause=code)
                verify_native()
                check_status = native_command("-t", "-q")
                receipt["rollback_context_exit_code"] = check_status
                need(check_status == 0, "rollback_context_rejected")
                journal("rollback_reload_requested", cause=code)
                mode = signal_master(request["master"], master_handle)
                receipt["rollback_reload_signal_mode"] = mode
                verify_native()
                journal("rolled_back_unqualified", cause=code, reload_signal_mode=mode)
                if predecessor is not None:
                    owned_file(backup_name, publication_identity, request["candidate_sha256"], renamed=True)
                    os.unlink(backup_name, dir_fd=destination_directory)
                    os.fsync(destination_directory)
            except Exception:
                receipt.update(rollback_ambiguous=True, recovery_pending=True)
                try:
                    journal("rollback_ambiguous", cause=code)
                except Exception:
                    pass
        elif exchange_attempted:
            receipt["recovery_pending"] = True
            try:
                journal("rollback_ambiguous", cause=code)
            except Exception:
                receipt["journal_ambiguous"] = True
        elif journal_fd is not None:
            try:
                journal("refused_before_publication", cause=code)
            except Exception:
                receipt["journal_ambiguous"] = True
        raise RuntimeError(code) from None
    finally:
        if master_handle is not None:
            os.close(master_handle)
        if temporary is not None and not exchange_attempted:
            try:
                observed = identity(os.fstat(publication_fd))
                if kind == "create" and observed["links"] == "2":
                    # An interrupted create link leaves two names for the same
                    # admitted inode. Only that exact pair may lose its stage
                    # name; controlled link metadata never relaxes byte/mtime
                    # or pathname custody for a changed temporary.
                    keys = set(publication_identity) - {"links", "ctime_ns"}
                    need(all(observed[name] == publication_identity[name] for name in keys),
                         "publication_temporary_changed")
                    def verify_link_pair():
                        need(observed == identity(os.fstat(publication_fd))
                             == identity(os.stat(temporary, dir_fd=destination_directory, follow_symlinks=False))
                             == identity(os.stat(basename, dir_fd=destination_directory, follow_symlinks=False)),
                             "publication_temporary_changed")
                    verify_link_pair()
                    payload = os.pread(publication_fd, 1048577, 0)
                    need(len(payload) <= 1048576 and hashlib.sha256(payload).hexdigest() == request["candidate_sha256"],
                         "publication_temporary_changed")
                    verify_link_pair()
                else:
                    owned_file(temporary, publication_identity, request["candidate_sha256"])
                    need(identity(os.fstat(publication_fd)) == publication_identity, "publication_temporary_changed")
                os.unlink(temporary, dir_fd=destination_directory)
                os.fsync(destination_directory)
            except Exception:
                receipt.update(exit_code=1, publication_cleanup_ambiguous=True)


def remote_apply(request: dict) -> dict:
    """Return an inspectable apply receipt while leaving API qualification open."""
    result = checked.remote_check(request, apply_in_context)
    result["schema"] = "iroha.taira.native-nginx-apply.receipt.v1"
    result.pop("activated", None)
    result.pop("reloaded", None)
    result.setdefault("configuration_published", False)
    result.setdefault("reload_requested", False)
    result["qualified"] = False
    return result


def apply_owned_publication(plan: dict) -> dict:
    """Use the maintained owner directly on this native host, without transport.

    The authenticated native Rust dispatcher admits the inventory, lease and
    public plan before calling this entry point. Only its declared public
    candidate and renderer are read here; native nginx configuration stays in
    the retained descriptor/native consumer corridor.
    """
    validate_plan(plan)
    candidate = checked.read_declared_public_file(plan["candidate"]["path"], plan["candidate"]["sha256"],
        owner=plan["candidate"]["owner_uid"], limit=checked.MAX_PUBLIC_BYTES)
    checked.read_declared_public_file(plan["renderer_source"]["path"], plan["renderer_source"]["sha256"],
                                     limit=checked.MAX_PUBLIC_BYTES)
    request = dict(host_kind=plan["host_kind"], native=plan["native"], master=plan["master"],
        destination=plan["destination"], operation_id=plan["operation_id"], publication=plan["publication"],
        candidate_sha256=plan["candidate"]["sha256"], candidate_base64=base64.b64encode(candidate).decode(),
        renderer_source_sha256=plan["renderer_source"]["sha256"])
    result = remote_apply(request)
    return admit_receipt(result, result["exit_code"], plan)


def adopt_owned_publication(plan: dict, operation) -> dict:
    """Enter exact existing-source custody for a separately authenticated adoption.

    The fixed native dispatcher and adoption admission own authorization; this
    entry point admits only public source bytes and the maintained native lock.
    The callback must prove and preserve the unchanged included publication.
    """
    validate_plan(plan)
    _owned_need(plan["publication"] == {"kind": "create"} and callable(operation),
                "adoption_capture_plan")
    candidate = checked.read_declared_public_file(plan["candidate"]["path"], plan["candidate"]["sha256"],
        owner=plan["candidate"]["owner_uid"], limit=checked.MAX_PUBLIC_BYTES)
    checked.read_declared_public_file(plan["renderer_source"]["path"], plan["renderer_source"]["sha256"],
                                     limit=checked.MAX_PUBLIC_BYTES)
    request = dict(host_kind=plan["host_kind"], native=plan["native"], master=plan["master"],
        destination=plan["destination"], operation_id=plan["operation_id"], publication=plan["publication"],
        candidate_sha256=plan["candidate"]["sha256"], candidate_base64=base64.b64encode(candidate).decode(),
        renderer_source_sha256=plan["renderer_source"]["sha256"])
    return checked.remote_check(request, operation, existing_publication=True)


def reconcile_interrupted_publication(original_plan: dict, journal_reference: dict | None, direction: str) -> dict:
    """Reconcile one immutable operation locally; absence is an explicit argument.

    The admitted Rust receiver retains the exact journal descriptor and both
    operation leases through this call. Null permits only a journal that is
    absent under the native owner lock, before this same operation began. A
    present journal must prove every public stage/backup inode; ambiguous files
    are preserved. Completed owners receive read-only admission, never HUP.
    """
    checked.require(direction in {"resume", "rollback"}, "interrupted_recovery_direction")
    validate_plan(original_plan)
    checked.require(original_plan["publication"]["kind"] in {"create", "replace"},
                    "interrupted_original_publication_kind")
    if journal_reference is not None:
        checked.require(validate_public_journal_reference(journal_reference, original_plan["native"])
                        == original_plan["operation_id"], "interrupted_operation_identity")
    candidate = checked.read_declared_public_file(original_plan["candidate"]["path"],
        original_plan["candidate"]["sha256"], owner=original_plan["candidate"]["owner_uid"],
        limit=checked.MAX_PUBLIC_BYTES)
    checked.read_declared_public_file(original_plan["renderer_source"]["path"],
                                     original_plan["renderer_source"]["sha256"], limit=checked.MAX_PUBLIC_BYTES)
    request = dict(host_kind=original_plan["host_kind"], native=original_plan["native"],
        master=original_plan["master"], destination=original_plan["destination"],
        operation_id=original_plan["operation_id"], publication=original_plan["publication"],
        candidate_sha256=original_plan["candidate"]["sha256"], candidate_base64=base64.b64encode(candidate).decode(),
        renderer_source_sha256=original_plan["renderer_source"]["sha256"], interrupted_journal=journal_reference,
        recovery_direction=direction)
    result = checked.remote_check(request, reconcile_interrupted_in_context)
    result["schema"] = RECEIPT_SCHEMA
    result.pop("activated", None)
    result.pop("reloaded", None)
    result.setdefault("configuration_published", False)
    result.setdefault("reload_requested", False)
    result["qualified"] = False
    if result["exit_code"] != 0:
        result["recovery_pending"] = True
    result["recovery_direction"] = direction
    result.setdefault("journal", None)
    result.setdefault("restored_owned_publication", None)
    result.setdefault("journal_write_intent", None)
    return admit_receipt(result, result["exit_code"], original_plan, recovery_direction=direction)


def remote_program(request: dict) -> bytes:
    """Send only maintained public code and the explicit public candidate."""
    code = ("import base64, hashlib, json, os, re, signal, subprocess, sys\nfrom pathlib import Path\n"
            "from types import SimpleNamespace\n" + "AWK_PROGRAM = " + repr(checked.AWK_PROGRAM) + "\n"
            + "SOURCE_GUARD = " + repr(SOURCE_GUARD) + "\n" + inspect.getsource(checked.remote_check)
            + "\nchecked = SimpleNamespace(remote_check=remote_check)\n" + inspect.getsource(observe_master)
            + inspect.getsource(open_master_handle) + inspect.getsource(signal_master) + inspect.getsource(native_exchange)
            + inspect.getsource(native_publish_no_replace) + inspect.getsource(write_all_owned)
            + inspect.getsource(public_journal_record_projection) + inspect.getsource(read_public_journal_write_intent)
            + inspect.getsource(append_owned_publication_record)
            + inspect.getsource(_owned_need) + inspect.getsource(verify_owned_public_file)
            + inspect.getsource(validate_public_owner_identity) + inspect.getsource(validate_public_journal_reference)
            + inspect.getsource(validate_public_prior_reference) + inspect.getsource(decode_owned_publication_records)
            + inspect.getsource(read_owned_publication_journal)
            + inspect.getsource(check_owned_public_context) + inspect.getsource(apply_in_context) + inspect.getsource(remote_apply)
            + "\nresult = remote_apply(json.loads(" + repr(json.dumps(request)) + "))\n"
            + "print(json.dumps(result, sort_keys=True))\nsys.exit(result['exit_code'])\n")
    return code.encode()


def admit_receipt(receipt: object, exit_code: int, plan: dict, *, recovery_direction: str | None = None) -> dict:
    """Accept only bounded public metadata with exact plan and journal bindings."""
    core = {"schema", "private_config_read_by_controller", "validation_files_removed", "exit_code",
            "nginx_exit_code", "error_code", "host_kind", "owner_uid", "native_executable",
            "main_configuration", "configuration_directory", "candidate_sha256", "renderer_source_sha256",
            "inherited_configuration_checked", "relative_include_prefix_preserved"}
    effects = {"destination_path", "journal_path", "operation_id", "configuration_published", "reload_requested",
               "qualified", "journal_sequence", "phase", "journal_identity", "active_context_nginx_exit_code",
               "active_source_guard_exit_code", "reload_signal_mode", "rollback_context_exit_code",
               "rollback_reload_signal_mode", "rollback_ambiguous", "journal_ambiguous", "publication_cleanup_ambiguous",
               "publication_kind", "prior_operation_id", "backup_path", "backup_identity", "recovery_pending",
               "journal_write_intent"}
    checked.require(isinstance(receipt, dict), "apply_receipt_fields")
    if recovery_direction is not None:
        effects |= {"recovery_direction", "journal", "restored_owned_publication"}
        checked.require(receipt.get("recovery_direction") == recovery_direction
                        and {"recovery_direction", "journal", "restored_owned_publication", "journal_write_intent"} <= set(receipt),
                        "interrupted_receipt_fields")
    checked.require(isinstance(receipt, dict) and set(receipt) <= core | effects
                    and receipt.get("schema") == RECEIPT_SCHEMA and receipt.get("qualified") is False,
                    "apply_receipt_fields")
    native_receipt = {key: value for key, value in receipt.items() if key in core}
    native_receipt.update(schema=checked.RECEIPT_SCHEMA, activated=False, reloaded=False)
    checked.admit_receipt(native_receipt, exit_code, plan)
    expected = dict(operation_id=plan["operation_id"],
        publication_kind=plan["publication"]["kind"],
        destination_path=str(Path(plan["destination"]["directory"]["path"]) / plan["destination"]["basename"]),
        journal_path=str(Path(plan["native"]["directory"]["path"]) /
                         (".taira-native-nginx-apply-" + plan["operation_id"] + ".receipt.ndjson")))
    checked.require(all(receipt[key] == value for key, value in expected.items() if key in receipt),
                    "apply_receipt_identity")
    if "prior_operation_id" in receipt:
        checked.require(receipt["prior_operation_id"] == plan["publication"]["prior"]["operation_id"], "apply_receipt_prior")
    if "backup_path" in receipt:
        checked.require(receipt["backup_path"] == str(Path(plan["destination"]["directory"]["path"]) /
            (".taira-nginx-backup-" + plan["operation_id"] + ".public")), "apply_receipt_backup")
    for name in {"configuration_published", "reload_requested", "rollback_ambiguous", "journal_ambiguous", "publication_cleanup_ambiguous", "recovery_pending"}:
        checked.require(name not in receipt or type(receipt[name]) is bool, "apply_receipt_boolean")
    for name in {"active_context_nginx_exit_code", "active_source_guard_exit_code", "rollback_context_exit_code"}:
        checked.require(name not in receipt or (type(receipt[name]) is int and -128 <= receipt[name] <= 255),
                        "apply_receipt_status")
    for name in {"reload_signal_mode", "rollback_reload_signal_mode"}:
        checked.require(name not in receipt or receipt[name] in {"pidfd", "adjacent_native_identity"}, "apply_receipt_signal")
    phases = {"prepared", "publishing", "published", "context_checking", "context_checked", "reload_requested",
              "awaiting_readiness", "rollback_requested", "rollback_source_removed", "rollback_reload_requested",
              "rolled_back_unqualified", "rollback_ambiguous", "refused_before_publication"}
    if recovery_direction == "rollback":
        phases.add("not_requested")
    checked.require("phase" not in receipt or receipt["phase"] in phases, "apply_receipt_phase")
    for name in ("journal_identity", "backup_identity"):
        if name not in receipt:
            continue
        journal = receipt[name]
        checked.require(isinstance(journal, dict) and set(journal) == {
            "device", "inode", "uid", "gid", "mode", "links", "size", "mtime_ns", "ctime_ns"}
            and all(isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", value) for value in journal.values())
            and journal["uid"] == str(plan["native"]["owner_uid"]) and journal["mode"] == "384"
            and journal["links"] == "1" and int(journal["size"]) <= 1024 * 1024,
            "apply_receipt_journal")
    checked.require("journal_sequence" not in receipt or
                    (type(receipt["journal_sequence"]) is int and 1 <= receipt["journal_sequence"] <= 128),
                    "apply_receipt_sequence")
    def public_reference(reference, expected_path=None):
        checked.require(isinstance(reference, dict) and set(reference) == {"file", "sha256"}
            and isinstance(reference["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", reference["sha256"]),
            "interrupted_receipt_public_reference")
        file = reference["file"]
        checked.require(isinstance(file, dict) and set(file) == {"path", "identity"},
                        "interrupted_receipt_public_reference")
        checked.canonical_path(file["path"])
        if expected_path is not None:
            checked.require(file["path"] == expected_path, "interrupted_receipt_public_path")
        observed = file["identity"]
        keys = {"device", "inode", "uid", "gid", "mode", "links", "size", "mtime_ns", "ctime_ns"}
        checked.require(isinstance(observed, dict) and set(observed) == keys
            and all(type(value) is int and 0 <= value <= 18446744073709551615 for value in observed.values())
            and observed["uid"] == plan["native"]["owner_uid"] and observed["mode"] == 0o600
            and observed["links"] == 1 and 0 < observed["size"] <= 1048576,
            "interrupted_receipt_public_identity")
    witness = receipt.get("journal_write_intent")
    if witness is not None:
        public_reference(witness)
        file = witness["file"]
        match = re.fullmatch(r"\.taira-native-nginx-write-"+plan["operation_id"]
            +r"-([1-9][0-9]{0,2})-[0-9a-f]{32}\.intent\.json", Path(file["path"]).name)
        checked.require(match is not None and int(match.group(1)) <= 128
            and file["path"] == str(Path(plan["native"]["directory"]["path"]) / Path(file["path"]).name)
            and file["identity"]["size"] <= 65536
            and ("journal_sequence" not in receipt or int(match.group(1)) == receipt["journal_sequence"]),
            "apply_receipt_write_intent")
    if recovery_direction is not None:
        if receipt["journal"] is not None:
            public_reference(receipt["journal"], expected["journal_path"])
        restored = receipt["restored_owned_publication"]
        if restored is not None:
            prior = plan["publication"].get("prior")
            checked.require(prior is not None and isinstance(restored, dict)
                and set(restored) == {"operation_id", "journal", "publication"}
                and restored["operation_id"] == prior["operation_id"], "interrupted_receipt_restored_owner")
            public_reference(restored["journal"], prior["journal"]["path"])
            public_reference(restored["publication"], expected["destination_path"])
            checked.require(restored["publication"]["sha256"] == prior["publication"]["sha256"],
                            "interrupted_receipt_restored_owner")
        if exit_code == 0:
            checked.require((receipt.get("phase") == "not_requested") == (receipt["journal"] is None)
                and (recovery_direction != "rollback" or (plan["publication"]["kind"] == "replace") == (restored is not None)),
                "interrupted_receipt_owner_join")
            checked.require((receipt["journal"] is None) == (witness is None), "interrupted_receipt_write_intent_join")
    if exit_code == 0 and recovery_direction == "rollback":
        checked.require(receipt.get("phase") in {"rolled_back_unqualified", "not_requested"}
            and receipt.get("configuration_published") is False and receipt.get("rollback_context_exit_code") == 0
            and not receipt.get("recovery_pending"), "interrupted_rollback_receipt_success")
    elif exit_code == 0:
        checked.require(set(expected) <= set(receipt) and receipt.get("configuration_published") is True
            and receipt.get("reload_requested") is True and receipt.get("phase") == "awaiting_readiness"
            and receipt.get("active_context_nginx_exit_code") == receipt.get("active_source_guard_exit_code") == 0
            and "reload_signal_mode" in receipt and "journal_identity" in receipt and "journal_sequence" in receipt
            and witness is not None
            and not any(receipt.get(name) for name in ("rollback_ambiguous", "journal_ambiguous", "publication_cleanup_ambiguous", "recovery_pending")),
            "apply_receipt_success")
    return receipt


def apply(plan: dict) -> dict:
    """Use only the pinned approved transport; never reuse a Linux installer."""
    import taira_retry

    validate_plan(plan)
    deployment = json.loads(taira_retry.public_record(plan["deployment_reference"]["path"], plan["deployment_reference"]["sha256"]))
    route = deployment["backing_ssh" if plan["host_kind"] == "macos" else "guest_ssh"]
    checked.require(not any(word in json.dumps(route).lower() for word in ("vultr", "amazonaws", "aws.amazon")), "banned_provider")
    argv = taira_retry.validate_ssh(route)
    candidate = taira_retry.public_record(plan["candidate"]["path"], plan["candidate"]["sha256"],
        owner=plan["candidate"]["owner_uid"], limit=checked.MAX_PUBLIC_BYTES)
    taira_retry.public_record(plan["renderer_source"]["path"], plan["renderer_source"]["sha256"], limit=checked.MAX_PUBLIC_BYTES)
    request = dict(host_kind=plan["host_kind"], native=plan["native"], master=plan["master"],
        destination=plan["destination"], operation_id=plan["operation_id"], publication=plan["publication"],
        candidate_sha256=plan["candidate"]["sha256"], candidate_base64=base64.b64encode(candidate).decode(),
        renderer_source_sha256=plan["renderer_source"]["sha256"])
    result = subprocess.run(argv, input=remote_program(request), stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=300)
    checked.require(len(result.stdout) <= 65536, "receipt_size_bound")
    receipt = json.loads(result.stdout)
    receipt = admit_receipt(receipt, result.returncode, plan)
    receipt["ssh_stderr_bytes"] = len(result.stderr)
    receipt["approved_deployment_sha256"] = plan["deployment_reference"]["sha256"]
    return receipt


def main(argv: list[str] | None = None) -> int:
    """Apply a reviewed new scoped include; API readiness remains a separate gate."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True, help="Explicit public observed native apply plan")
    args = parser.parse_args(argv)
    try:
        checked.require(args.plan.stat().st_size <= checked.MAX_PUBLIC_BYTES, "plan_size_bound")
        result = apply(json.loads(args.plan.read_bytes()))
    except Exception as error:
        result = dict(schema=RECEIPT_SCHEMA, exit_code=1, qualified=False,
                      error_code=error.args[0] if isinstance(error, checked.CheckError) else "controller_apply_failed")
    print(json.dumps(result, sort_keys=True))
    return result["exit_code"]


if __name__ == "__main__":
    raise SystemExit(main())
