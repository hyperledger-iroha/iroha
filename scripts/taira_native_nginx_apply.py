#!/usr/bin/env python3
"""Create one scoped public nginx include, check native context and request reload.

Requires an explicit approved MacStadium route, native identities, fresh nginx
master metadata and a new destination. Existing config/key bytes remain native.
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
    """Admit one new explicit include and fresh public master identity."""
    checked.require(isinstance(value, dict) and set(value) == {
        "schema", "provider", "host_kind", "deployment_reference", "native", "candidate",
        "renderer_source", "operation_id", "destination", "master"}, "apply_plan_fields")
    base = {name: item for name, item in value.items() if name not in {"operation_id", "destination", "master"}}
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
    return value


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
    temporary = None
    published = False
    master_handle = None
    environment = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    receipt.update(destination_path=destination_path, journal_path=journal_path,
                   operation_id=request["operation_id"], configuration_published=False,
                   reload_requested=False, qualified=False)

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
        verify_master()

    def journal(phase, **details):
        nonlocal journal_identity, sequence
        current = os.stat(journal_name, dir_fd=main_directory, follow_symlinks=False)
        need(identity(current) == journal_identity == identity(os.fstat(journal_fd)), "journal_identity_changed")
        sequence += 1
        record = dict(schema="iroha.taira.native-nginx-apply.journal.v1", sequence=sequence,
            operation_id=request["operation_id"], phase=phase, qualified=False,
            candidate_sha256=request["candidate_sha256"], renderer_source_sha256=request["renderer_source_sha256"],
            host_kind=request["host_kind"], native_executable=native["nginx"], main_configuration=native["main"],
            master=request["master"], destination_path=destination_path,
            destination_directory=destination["directory"], publication_identity=publication_identity,
            **details)
        data = (json.dumps(record, sort_keys=True) + "\n").encode()
        need(len(data) <= 65536, "journal_record_size_bound")
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
             and identity(os.fstat(publication_fd)) == publication_identity, "published_identity_changed")
        payload = os.pread(current, 1048577, 0)
        need(len(payload) <= 1048576 and hashlib.sha256(payload).hexdigest() == request["candidate_sha256"]
             and identity(os.fstat(current)) == publication_identity, "published_content_changed")

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

    def refuse_unknown_retry():
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
            if record.get("destination_path") == destination_path and record.get("phase") not in terminal:
                raise RuntimeError("unresolved_destination_journal")

    try:
        need(os.fstat(destination_directory).st_uid == owner
             and os.fstat(destination_directory).st_dev == os.fstat(main_directory).st_dev, "destination_owner_or_device")
        try:
            os.stat(basename, dir_fd=destination_directory, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise RuntimeError("destination_already_exists")
        verify_native()
        master_handle = open_master_handle(request["master"])
        verify_master()
        refuse_unknown_retry()
        journal_fd = os.open(journal_name, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                             0o600, dir_fd=main_directory)
        context["handles"].append(journal_fd)
        journal_identity = identity(os.fstat(journal_fd))
        os.fsync(main_directory)
        journal("prepared")
        temporary = ".taira-nginx-publish-" + uuid.uuid4().hex + ".candidate"
        publication_fd = os.open(temporary, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                                 0o600, dir_fd=destination_directory)
        context["handles"].append(publication_fd)
        payload = os.pread(context["candidate_fd"], 1048577, 0)
        need(hashlib.sha256(payload).hexdigest() == request["candidate_sha256"], "candidate_digest_changed")
        need(os.write(publication_fd, payload) == len(payload), "publication_write_incomplete")
        os.fsync(publication_fd)
        publication_identity = identity(os.fstat(publication_fd))
        journal("publishing", temporary_basename=temporary)
        verify_native()
        os.link(temporary, basename, src_dir_fd=destination_directory, dst_dir_fd=destination_directory,
                follow_symlinks=False)
        published = True
        os.unlink(temporary, dir_fd=destination_directory)
        temporary = None
        publication_identity = identity(os.fstat(publication_fd))
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
        journal("context_checked", nginx_exit_code=native_status, source_guard_exit_code=source_status)
        journal("reload_requested")
        receipt["reload_requested"] = True
        mode = signal_master(request["master"], master_handle)
        receipt["reload_signal_mode"] = mode
        verify_publication()
        verify_native()
        journal("awaiting_readiness", reload_signal_mode=mode)
    except Exception as error:
        code = error.args[0] if type(error) is RuntimeError and error.args else "apply_failed"
        code = code if isinstance(code, str) and re.fullmatch(r"[a-z_]+", code) else "apply_failed"
        if published:
            try:
                journal("rollback_requested", cause=code)
                verify_publication()
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
            except Exception:
                receipt["rollback_ambiguous"] = True
                try:
                    journal("rollback_ambiguous", cause=code)
                except Exception:
                    pass
        elif journal_fd is not None:
            try:
                journal("refused_before_publication", cause=code)
            except Exception:
                receipt["journal_ambiguous"] = True
        raise RuntimeError(code) from None
    finally:
        if master_handle is not None:
            os.close(master_handle)
        if temporary is not None:
            try:
                info = os.stat(temporary, dir_fd=destination_directory, follow_symlinks=False)
                need(stat.S_ISREG(info.st_mode) and info.st_uid == owner
                     and (info.st_dev, info.st_ino) == (os.fstat(publication_fd).st_dev, os.fstat(publication_fd).st_ino)
                     and stat.S_IMODE(info.st_mode) == 0o600, "publication_temporary_changed")
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


def remote_program(request: dict) -> bytes:
    """Send only maintained public code and the explicit public candidate."""
    code = ("import base64, hashlib, json, os, re, signal, subprocess, sys\nfrom pathlib import Path\n"
            "from types import SimpleNamespace\n" + "AWK_PROGRAM = " + repr(checked.AWK_PROGRAM) + "\n"
            + "SOURCE_GUARD = " + repr(SOURCE_GUARD) + "\n" + inspect.getsource(checked.remote_check)
            + "\nchecked = SimpleNamespace(remote_check=remote_check)\n" + inspect.getsource(observe_master)
            + inspect.getsource(open_master_handle) + inspect.getsource(signal_master)
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
               "rollback_reload_signal_mode", "rollback_ambiguous", "journal_ambiguous", "publication_cleanup_ambiguous"}
    checked.require(isinstance(receipt, dict) and set(receipt) <= core | effects
                    and receipt.get("schema") == RECEIPT_SCHEMA and receipt.get("qualified") is False,
                    "apply_receipt_fields")
    native_receipt = {key: value for key, value in receipt.items() if key in core}
    native_receipt.update(schema=checked.RECEIPT_SCHEMA, activated=False, reloaded=False)
    checked.admit_receipt(native_receipt, exit_code, plan)
    expected = dict(operation_id=plan["operation_id"],
        destination_path=str(Path(plan["destination"]["directory"]["path"]) / plan["destination"]["basename"]),
        journal_path=str(Path(plan["native"]["directory"]["path"]) /
                         (".taira-native-nginx-apply-" + plan["operation_id"] + ".receipt.ndjson")))
    checked.require(all(receipt[key] == value for key, value in expected.items() if key in receipt),
                    "apply_receipt_identity")
    for name in {"configuration_published", "reload_requested", "rollback_ambiguous", "journal_ambiguous", "publication_cleanup_ambiguous"}:
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
    if "journal_identity" in receipt:
        journal = receipt["journal_identity"]
        checked.require(isinstance(journal, dict) and set(journal) == {
            "device", "inode", "uid", "gid", "mode", "links", "size", "mtime_ns", "ctime_ns"}
            and all(isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", value) for value in journal.values())
            and journal["uid"] == str(plan["native"]["owner_uid"]) and journal["mode"] == "384"
            and journal["links"] == "1" and int(journal["size"]) <= 1024 * 1024,
            "apply_receipt_journal")
    checked.require("journal_sequence" not in receipt or
                    (type(receipt["journal_sequence"]) is int and 1 <= receipt["journal_sequence"] <= 32),
                    "apply_receipt_sequence")
    if exit_code == 0:
        checked.require(set(expected) <= set(receipt) and receipt.get("configuration_published") is True
            and receipt.get("reload_requested") is True and receipt.get("phase") == "awaiting_readiness"
            and receipt.get("active_context_nginx_exit_code") == receipt.get("active_source_guard_exit_code") == 0
            and "reload_signal_mode" in receipt and "journal_identity" in receipt and "journal_sequence" in receipt
            and not any(receipt.get(name) for name in ("rollback_ambiguous", "journal_ambiguous", "publication_cleanup_ambiguous")),
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
        destination=plan["destination"], operation_id=plan["operation_id"],
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
