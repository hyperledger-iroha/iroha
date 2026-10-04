#!/usr/bin/env python3
"""Create, replace or reconcile one owned public nginx include natively.

Requires an explicit approved MacStadium route, native identities, fresh nginx
master metadata and an explicit publication kind. Replacement pins the prior
unqualified owner journal and public inode; native exchange retains that inode
for exact-own rollback. Existing private config/key bytes remain native.
An owner-private append-only host journal precedes publication and records every
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
    destination = request["destination"]
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
            "destination_path", "destination_directory", "publication_identity"}
    extras = {"prior", "temporary_basename", "cause", "nginx_exit_code", "source_guard_exit_code",
              "reload_signal_mode", "backup_identity"}
    phases = {"prepared", "publishing", "published", "context_checking", "context_checked", "reload_requested",
              "awaiting_readiness", "rollback_requested", "rollback_source_removed", "rollback_reload_requested",
              "rolled_back_unqualified", "rollback_ambiguous", "refused_before_publication"}
    first = records[0]
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
        if "prior" in record:
            validate_public_prior_reference(record["prior"], native)
        for name in ("candidate_sha256", "renderer_source_sha256", "master", "prior"):
            need(record.get(name) == first.get(name), "prior_journal_binding_changed")
    return opened, records


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
        nonlocal journal_identity, sequence
        current = os.stat(journal_name, dir_fd=main_directory, follow_symlinks=False)
        need(identity(current) == journal_identity == identity(os.fstat(journal_fd)), "journal_identity_changed")
        sequence += 1
        need(sequence <= 128, "journal_record_bound")
        record = dict(schema="iroha.taira.native-nginx-apply.journal.v1", sequence=sequence,
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
        data = (json.dumps(record, sort_keys=True) + "\n").encode()
        need(len(data) <= 65536 and os.fstat(journal_fd).st_size + len(data) <= 1048576,
             "journal_record_size_bound")
        need(os.write(journal_fd, data) == len(data), "journal_write_incomplete")
        os.fsync(journal_fd)
        journal_identity = identity(os.fstat(journal_fd))
        need(identity(os.stat(journal_name, dir_fd=main_directory, follow_symlinks=False)) == journal_identity,
             "journal_identity_changed")
        receipt.update(journal_sequence=sequence, phase=phase, journal_identity=journal_identity)

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
        journal_flags = os.O_WRONLY | os.O_APPEND | os.O_NOFOLLOW | os.O_CLOEXEC
        if kind != "reconcile":
            journal_flags |= os.O_CREAT | os.O_EXCL
        journal_fd = os.open(journal_name, journal_flags, 0o600, dir_fd=main_directory)
        context["handles"].append(journal_fd)
        journal_identity = identity(os.fstat(journal_fd))
        if kind == "reconcile":
            need(journal_identity == prior["journal"]["identity"], "prior_journal_changed")
            sequence = int(sequence or len(read_journal(prior["journal"])[1]))
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
            need(os.write(publication_fd, payload) == len(payload), "publication_write_incomplete")
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


def remote_program(request: dict) -> bytes:
    """Send only maintained public code and the explicit public candidate."""
    code = ("import base64, hashlib, json, os, re, signal, subprocess, sys\nfrom pathlib import Path\n"
            "from types import SimpleNamespace\n" + "AWK_PROGRAM = " + repr(checked.AWK_PROGRAM) + "\n"
            + "SOURCE_GUARD = " + repr(SOURCE_GUARD) + "\n" + inspect.getsource(checked.remote_check)
            + "\nchecked = SimpleNamespace(remote_check=remote_check)\n" + inspect.getsource(observe_master)
            + inspect.getsource(open_master_handle) + inspect.getsource(signal_master) + inspect.getsource(native_exchange)
            + inspect.getsource(_owned_need) + inspect.getsource(verify_owned_public_file)
            + inspect.getsource(validate_public_owner_identity) + inspect.getsource(validate_public_journal_reference)
            + inspect.getsource(validate_public_prior_reference) + inspect.getsource(read_owned_publication_journal)
            + inspect.getsource(apply_in_context) + inspect.getsource(remote_apply)
            + "\nresult = remote_apply(json.loads(" + repr(json.dumps(request)) + "))\n"
            + "print(json.dumps(result, sort_keys=True))\nsys.exit(result['exit_code'])\n")
    return code.encode()


def admit_receipt(receipt: object, exit_code: int, plan: dict) -> dict:
    """Accept only bounded public metadata with exact plan and journal bindings."""
    core = {"schema", "private_config_read_by_controller", "validation_files_removed", "exit_code",
            "nginx_exit_code", "error_code", "host_kind", "owner_uid", "native_executable",
            "main_configuration", "configuration_directory", "candidate_sha256", "renderer_source_sha256",
            "inherited_configuration_checked", "relative_include_prefix_preserved"}
    effects = {"destination_path", "journal_path", "operation_id", "configuration_published", "reload_requested",
               "qualified", "journal_sequence", "phase", "journal_identity", "active_context_nginx_exit_code",
               "active_source_guard_exit_code", "reload_signal_mode", "rollback_context_exit_code",
               "rollback_reload_signal_mode", "rollback_ambiguous", "journal_ambiguous", "publication_cleanup_ambiguous",
               "publication_kind", "prior_operation_id", "backup_path", "backup_identity", "recovery_pending"}
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
    if exit_code == 0:
        checked.require(set(expected) <= set(receipt) and receipt.get("configuration_published") is True
            and receipt.get("reload_requested") is True and receipt.get("phase") == "awaiting_readiness"
            and receipt.get("active_context_nginx_exit_code") == receipt.get("active_source_guard_exit_code") == 0
            and "reload_signal_mode" in receipt and "journal_identity" in receipt and "journal_sequence" in receipt
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
