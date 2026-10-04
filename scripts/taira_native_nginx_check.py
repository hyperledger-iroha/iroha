#!/usr/bin/env python3
"""Check a public nginx candidate in the observed host's full native context.

Requires Python 3, native nginx/awk, and an explicit pinned MacStadium SSH route.
The controller reads only the public plan, candidate and source. Existing nginx
configuration and TLS keys stay on their host: only awk/nginx consume them.
No activation, reload, credential transfer or environment override is supported.
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
import subprocess
import sys

PLAN_SCHEMA = "iroha.taira.native-nginx-check.plan.v1"
RECEIPT_SCHEMA = "iroha.taira.native-nginx-check.receipt.v1"
MAX_PUBLIC_BYTES = 1024 * 1024

# Existing private bytes flow directly from the retained main fd through native
# awk into an owner-only file. Lexical state ignores quoted/commented braces;
# exactly one top-level http block must receive exactly one explicit include.
AWK_PROGRAM = r'''
function fail() { bad=1; exit 41 }
BEGIN { depth=0; count=0; quote=""; escape=0; word=""; ended=0; bytes=0 }
{
  bytes += length($0)+1
  if (bytes > 4194304 || length($0) > 262144) fail()
  output=""
  for (i=1; i<=length($0); i++) {
    c=substr($0,i,1); output=output c
    if (escape) { escape=0; continue }
    if (c == "\\") { escape=1; continue }
    if (quote != "") { if (c == quote) quote=""; continue }
    if (c == "\"" || c == "\047") { quote=c; ended=1; continue }
    if (c == "#") { output=output substr($0,i+1); break }
    if (c == "{") {
      if (word == "http") {
        if (depth != 0) fail()
        count++
        if (count != 1) fail()
        output=output "\n  include " candidate ";\n"
      }
      depth++; if (depth > 128) fail()
      word=""; ended=0; continue
    }
    if (c == "}") { depth--; if (depth < 0) fail(); word=""; ended=0; continue }
    if (c == ";") { word=""; ended=0; continue }
    if (c ~ /[ \t\r]/) { if (word != "") ended=1; continue }
    if (!ended) { word=word c; if (length(word)>4096) fail() }
  }
  if (word != "") ended=1
  print output
}
END { if (bad || count != 1 || depth != 0 || quote != "" || escape) exit 41 }
'''


class CheckError(RuntimeError):
    """A bounded public error code, never a native configuration diagnostic."""


def require(condition: bool, code: str) -> None:
    """Reject without rendering inputs or captured private diagnostics."""
    if not condition:
        raise CheckError(code)


def canonical_path(value: object) -> str:
    """Admit an explicit absolute path safe for one nginx include directive."""
    require(isinstance(value, str) and re.fullmatch(r"/[A-Za-z0-9_./-]+", value) is not None
            and str(Path(value)) == value and os.path.normpath(value) == value,
            "noncanonical_path")
    return value


def read_declared_public_file(path: str, expected_sha256: str, *, owner: int | None = None,
                              limit: int = MAX_PUBLIC_BYTES) -> bytes:
    """Read one declared public input under bounded, no-follow pathname/FD custody.

    Native local callers use this without importing controller transport code.
    This entry point is exclusively for public candidates and maintained source;
    existing configuration, keys and tokens are never declared public inputs.
    """
    import stat

    canonical_path(path)
    require(isinstance(expected_sha256, str)
            and re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is not None
            and type(limit) is int and 0 < limit <= MAX_PUBLIC_BYTES
            and (owner is None or type(owner) is int and owner >= 0), "public_input_reference")
    owners = {0, os.geteuid()}
    def open_parent():
        current = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            for part in Path(path).parts[1:-1]:
                next_directory = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                                         dir_fd=current)
                info = os.fstat(next_directory)
                if not (info.st_uid in owners and not stat.S_IMODE(info.st_mode) & 0o022):
                    os.close(next_directory)
                    raise CheckError("unsafe_public_ancestor")
                os.close(current)
                current = next_directory
            return current
        except BaseException:
            os.close(current)
            raise
    directory = open_parent()
    opened = None
    try:
        name = Path(path).name
        opened = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK,
                         dir_fd=directory)
        def snapshot(info):
            return (info.st_dev, info.st_ino, info.st_uid, info.st_gid, stat.S_IMODE(info.st_mode),
                    info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
        before = os.fstat(opened)
        observed = snapshot(before)
        require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                and before.st_uid == (os.geteuid() if owner is None else owner)
                and not stat.S_IMODE(before.st_mode) & 0o022
                and 0 < before.st_size <= limit, "unsafe_public_input")
        require(observed == snapshot(os.stat(name, dir_fd=directory, follow_symlinks=False)),
                "public_input_changed")
        body = os.pread(opened, before.st_size + 1, 0)
        require(len(body) == before.st_size and observed == snapshot(os.fstat(opened))
                == snapshot(os.stat(name, dir_fd=directory, follow_symlinks=False)), "public_input_changed")
        # Reopen the complete path after the read: a renamed ancestor must not
        # hide a replacement that the retained parent descriptor cannot see.
        current_parent = open_parent()
        try:
            require(snapshot(os.stat(name, dir_fd=current_parent, follow_symlinks=False)) == observed,
                    "public_input_changed")
        finally:
            os.close(current_parent)
        require(hashlib.sha256(body).hexdigest() == expected_sha256, "public_input_digest_changed")
        return body
    finally:
        if opened is not None:
            os.close(opened)
        os.close(directory)


def validate_plan(value: object) -> dict:
    """Validate the closed public plan before reading any referenced input."""
    require(isinstance(value, dict) and set(value) == {
        "schema", "provider", "host_kind", "deployment_reference", "native",
        "candidate", "renderer_source"}, "plan_fields")
    require(value["schema"] == PLAN_SCHEMA and value["provider"] == "macstadium-dublin"
            and value["host_kind"] in {"macos", "linux"}, "approved_host_required")
    for name in ("deployment_reference", "renderer_source"):
        reference = value[name]
        require(isinstance(reference, dict) and set(reference) == {"path", "sha256"}, "reference_fields")
        canonical_path(reference["path"])
        require(re.fullmatch(r"[0-9a-f]{64}", reference["sha256"]) is not None, "reference_digest")
    candidate = value["candidate"]
    require(isinstance(candidate, dict) and set(candidate) == {"path", "sha256", "owner_uid"}, "candidate_fields")
    canonical_path(candidate["path"])
    require(re.fullmatch(r"[0-9a-f]{64}", candidate["sha256"]) is not None
            and type(candidate["owner_uid"]) is int, "candidate_identity")
    native = value["native"]
    require(isinstance(native, dict) and set(native) == {
        "nginx", "awk", "main", "directory", "owner_uid", "trusted_group_gids"}, "native_fields")
    require(type(native["owner_uid"]) is int and isinstance(native["trusted_group_gids"], list)
            and all(type(gid) is int and gid >= 0 for gid in native["trusted_group_gids"]), "native_owner")
    file_keys = {"device", "inode", "uid", "gid", "mode", "links", "size", "mtime_ns", "ctime_ns"}
    for name in ("nginx", "awk", "main", "directory"):
        reference = native[name]
        require(isinstance(reference, dict) and set(reference) == {"path", "identity"}, "native_reference")
        canonical_path(reference["path"])
        identity = reference["identity"]
        keys = {"device", "inode", "uid", "gid", "mode"} if name == "directory" else file_keys
        # Inodes and nanosecond timestamps exceed JavaScript's exact integer
        # range. Decimal text preserves native identity through every consumer.
        require(isinstance(identity, dict) and set(identity) == keys
                and all(isinstance(item, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", item)
                        for item in identity.values()), "native_identity")
    require(str(Path(native["main"]["path"]).parent) == native["directory"]["path"], "main_directory")
    return value


def remote_check(request: dict, operation=None) -> dict:
    """Retain native ownership through both bounded child processes and cleanup."""
    import fcntl
    import stat
    import uuid

    receipt = {"schema": "iroha.taira.native-nginx-check.receipt.v1", "activated": False,
               "reloaded": False, "private_config_read_by_controller": False,
               "validation_files_removed": False, "exit_code": 1}
    handles, created = [], []
    directory = lock = None
    lock_identity = None

    def need(condition, code):
        if not condition:
            raise RuntimeError(code)

    def identity(info, directory=False):
        value = dict(device=info.st_dev, inode=info.st_ino, uid=info.st_uid,
                     gid=info.st_gid, mode=stat.S_IMODE(info.st_mode))
        if not directory:
            value.update(links=info.st_nlink, size=info.st_size,
                         mtime_ns=info.st_mtime_ns, ctime_ns=info.st_ctime_ns)
        return {name: str(number) for name, number in value.items()}

    def path(value):
        need(isinstance(value, str) and re.fullmatch(r"/[A-Za-z0-9_./-]+", value) is not None
             and str(Path(value)) == value and os.path.normpath(value) == value, "noncanonical_path")
        return Path(value)

    def open_bound(reference, is_directory=False):
        target = path(reference["path"])
        parent = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        parents = [parent]
        try:
            for component in target.parts[1:-1]:
                child = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                                dir_fd=parent)
                parents.append(child)
                info = os.fstat(child)
                need(info.st_uid in {0, owner} and not info.st_mode & 0o002
                     and (not info.st_mode & 0o020 or info.st_gid in groups), "untrusted_parent")
                parent = child
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
            if is_directory:
                flags |= os.O_DIRECTORY
            opened = os.open(target.name, flags, dir_fd=parent)
        finally:
            for ancestor in reversed(parents):
                os.close(ancestor)
        handles.append(opened)
        info = os.fstat(opened)
        need((stat.S_ISDIR(info.st_mode) if is_directory else stat.S_ISREG(info.st_mode))
             and identity(info, is_directory) == reference["identity"], "native_identity_changed")
        need(is_directory or info.st_nlink == 1, "native_single_link_required")
        need(not info.st_mode & 0o022 and info.st_uid in {0, owner}, "unsafe_native_owner")
        return opened

    def revalidate(reference, opened, is_directory=False):
        current = open_bound(reference, is_directory)
        need(identity(os.fstat(opened), is_directory) == reference["identity"]
             and identity(os.fstat(current), is_directory) == reference["identity"],
             "native_identity_changed")

    def stage(name, data):
        opened = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                         0o600, dir_fd=directory)
        handles.append(opened)
        info = os.fstat(opened)
        created.append((name, info.st_dev, info.st_ino))
        with os.fdopen(os.dup(opened), "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        return opened

    def revalidate_stage(name, opened, expected):
        current = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK,
                          dir_fd=directory)
        handles.append(current)
        need(stat.S_ISREG(os.fstat(current).st_mode)
             and identity(os.fstat(opened)) == expected
             and identity(os.fstat(current)) == expected, "validation_file_identity_changed")

    try:
        native = request["native"]
        owner, groups = native["owner_uid"], set(native["trusted_group_gids"])
        need(os.geteuid() == owner and request["host_kind"] ==
             ("macos" if sys.platform == "darwin" else "linux"), "host_owner_changed")
        candidate = base64.b64decode(request["candidate_base64"], validate=True)
        need(0 < len(candidate) <= 1048576 and hashlib.sha256(candidate).hexdigest() ==
             request["candidate_sha256"], "candidate_digest_changed")
        directory = open_bound(native["directory"], True)
        need(os.fstat(directory).st_uid == owner, "configuration_directory_owner")
        lock = os.open(".taira-native-nginx-check.lock",
                       os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=directory)
        handles.append(lock)
        lock_info = os.fstat(lock)
        need(stat.S_ISREG(lock_info.st_mode) and lock_info.st_uid == owner and lock_info.st_nlink == 1
             and stat.S_IMODE(lock_info.st_mode) == 0o600, "unsafe_check_lock")
        lock_identity = identity(lock_info)
        need(identity(os.stat(".taira-native-nginx-check.lock", dir_fd=directory, follow_symlinks=False))
             == identity(os.fstat(lock)) == lock_identity, "check_lock_identity_changed")
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError("check_owner_busy") from None
        bound = {name: open_bound(native[name]) for name in ("nginx", "awk", "main")}
        need(int(native["main"]["identity"]["size"]) <= 4194304, "main_size_bound")
        for name in ("nginx", "awk"):
            need(os.fstat(bound[name]).st_mode & 0o111, "native_executable_required")
        token = uuid.uuid4().hex
        candidate_name = ".taira-nginx-check-" + token + ".candidate"
        main_name = ".taira-nginx-check-" + token + ".main"
        candidate_path = str(path(native["directory"]["path"]) / candidate_name)
        validation_main = str(path(native["directory"]["path"]) / main_name)
        candidate_fd = stage(candidate_name, candidate)
        candidate_identity = identity(os.fstat(candidate_fd))
        if operation is not None and request["publication"]["kind"] in {"replace", "reconcile"}:
            # An already included public source cannot be injected a second
            # time. The apply owner admits its exact journal/inode under this
            # same lock, then checks the complete native context after exchange.
            operation(receipt, request, dict(native=native, bound=bound, lock=lock,
                directory=directory, candidate_fd=candidate_fd, handles=handles, lock_identity=lock_identity,
                path=path, identity=identity, open_bound=open_bound, revalidate=revalidate))
            return receipt
        main_fd = stage(main_name, b"")
        environment = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
        guard = subprocess.run([native["awk"]["path"], "-v", "candidate=" + candidate_path, AWK_PROGRAM],
                               stdin=bound["main"], stdout=main_fd, stderr=subprocess.DEVNULL,
                               pass_fds=(lock, directory), env=environment, timeout=15)
        need(guard.returncode == 0, "main_http_guard_rejected")
        os.fsync(main_fd)
        main_identity = identity(os.fstat(main_fd))
        revalidate_stage(candidate_name, candidate_fd, candidate_identity)
        revalidate_stage(main_name, main_fd, main_identity)
        for name, opened in bound.items():
            revalidate(native[name], opened)
        revalidate(native["directory"], directory, True)
        need(os.fstat(main_fd).st_size <= 4194304 + 512, "validation_main_size_bound")
        checked = subprocess.run([native["nginx"]["path"], "-t", "-q", "-c", validation_main],
                                 stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                 stderr=subprocess.DEVNULL, pass_fds=(lock, directory, candidate_fd, main_fd),
                                 env=environment, timeout=45)
        receipt["nginx_exit_code"] = checked.returncode
        revalidate_stage(candidate_name, candidate_fd, candidate_identity)
        revalidate_stage(main_name, main_fd, main_identity)
        for name, opened in bound.items():
            revalidate(native[name], opened)
        revalidate(native["directory"], directory, True)
        need(identity(os.stat(".taira-native-nginx-check.lock", dir_fd=directory, follow_symlinks=False))
             == identity(os.fstat(lock)) == lock_identity, "check_lock_identity_changed")
        need(checked.returncode == 0, "native_nginx_rejected")
        receipt.update(host_kind=request["host_kind"], owner_uid=owner,
                       native_executable=native["nginx"], main_configuration=native["main"],
                       configuration_directory=native["directory"],
                       candidate_sha256=request["candidate_sha256"],
                       renderer_source_sha256=request["renderer_source_sha256"],
                       inherited_configuration_checked=True, relative_include_prefix_preserved=True,
                       exit_code=0)
        if operation is not None:
            # The apply owner receives the same still-held native
            # descriptors and lock after successful check admission. The CLI
            # for this module never supplies an operation.
            operation(receipt, request, dict(native=native, bound=bound, lock=lock,
                directory=directory, candidate_fd=candidate_fd, handles=handles, lock_identity=lock_identity,
                path=path, identity=identity, open_bound=open_bound, revalidate=revalidate))
    except Exception as error:
        receipt["exit_code"] = 1
        code = error.args[0] if type(error) is RuntimeError and error.args else "native_check_failed"
        receipt["error_code"] = code if isinstance(code, str) and re.fullmatch(r"[a-z_]+", code) else "native_check_failed"
    finally:
        removed = True
        for name, device, inode in reversed(created):
            try:
                info = os.stat(name, dir_fd=directory, follow_symlinks=False)
                need(stat.S_ISREG(info.st_mode) and (info.st_dev, info.st_ino) == (device, inode)
                     and info.st_uid == owner and info.st_nlink == 1 and stat.S_IMODE(info.st_mode) == 0o600,
                     "validation_file_identity_changed")
                os.unlink(name, dir_fd=directory)
            except Exception:
                removed = False
        if directory is not None:
            os.fsync(directory)
        receipt["validation_files_removed"] = removed
        if not removed:
            receipt.update(exit_code=1, error_code="validation_cleanup_refused")
        for opened in reversed(handles):
            os.close(opened)
    return receipt


def remote_program(request: dict) -> bytes:
    """Build a public stdin program; no private host file is transported."""
    code = ("import base64, hashlib, json, os, re, subprocess, sys\nfrom pathlib import Path\n"
            + "AWK_PROGRAM = " + repr(AWK_PROGRAM) + "\n" + inspect.getsource(remote_check)
            + "\nresult = remote_check(json.loads(" + repr(json.dumps(request)) + "))\n"
            + "print(json.dumps(result, sort_keys=True))\nsys.exit(result['exit_code'])\n")
    return code.encode()


def admit_receipt(receipt: object, exit_code: int, plan: dict) -> dict:
    """Allow only metadata bound to the explicit plan, never arbitrary host text."""
    base = {"schema", "activated", "reloaded", "private_config_read_by_controller",
            "validation_files_removed", "exit_code"}
    success = {"host_kind", "owner_uid", "native_executable", "main_configuration",
               "configuration_directory", "candidate_sha256", "renderer_source_sha256",
               "inherited_configuration_checked", "relative_include_prefix_preserved"}
    require(isinstance(receipt, dict) and base <= set(receipt)
            and set(receipt) <= base | success | {"nginx_exit_code", "error_code"}, "native_receipt_fields")
    require(receipt["schema"] == RECEIPT_SCHEMA and receipt["activated"] is False
            and receipt["reloaded"] is False and receipt["private_config_read_by_controller"] is False
            and type(receipt["validation_files_removed"]) is bool
            and type(receipt["exit_code"]) is int and receipt["exit_code"] in {0, 1}
            and receipt["exit_code"] == exit_code, "native_receipt_binding")
    expected = dict(host_kind=plan["host_kind"], owner_uid=plan["native"]["owner_uid"],
                    native_executable=plan["native"]["nginx"], main_configuration=plan["native"]["main"],
                    configuration_directory=plan["native"]["directory"],
                    candidate_sha256=plan["candidate"]["sha256"], renderer_source_sha256=plan["renderer_source"]["sha256"],
                    inherited_configuration_checked=True, relative_include_prefix_preserved=True)
    require(all(receipt[name] == expected[name] for name in success & set(receipt)), "native_receipt_identity")
    require("nginx_exit_code" not in receipt or
            (type(receipt["nginx_exit_code"]) is int and -128 <= receipt["nginx_exit_code"] <= 255),
            "native_receipt_exit_status")
    if receipt["exit_code"] == 0:
        require(success <= set(receipt) and receipt.get("nginx_exit_code") == 0
                and receipt["validation_files_removed"] and "error_code" not in receipt, "native_receipt_success")
    else:
        require(isinstance(receipt.get("error_code"), str)
                and re.fullmatch(r"[a-z_]{1,64}", receipt["error_code"]) is not None, "native_receipt_error")
    return receipt


def check(plan: dict) -> dict:
    """Reuse the approved fixed SSH transport and emit only a closed public receipt."""
    import taira_retry

    validate_plan(plan)
    deployment = json.loads(taira_retry.public_record(
        plan["deployment_reference"]["path"], plan["deployment_reference"]["sha256"]))
    route = deployment["backing_ssh" if plan["host_kind"] == "macos" else "guest_ssh"]
    require(not any(word in json.dumps(route).lower() for word in ("vultr", "amazonaws", "aws.amazon")),
            "banned_provider")
    argv = taira_retry.validate_ssh(route)
    candidate_path = Path(plan["candidate"]["path"])
    info = candidate_path.lstat()
    require(info.st_uid == plan["candidate"]["owner_uid"] and info.st_nlink == 1, "candidate_owner_changed")
    candidate = taira_retry.public_record(str(candidate_path), plan["candidate"]["sha256"],
                                         owner=plan["candidate"]["owner_uid"], limit=MAX_PUBLIC_BYTES)
    taira_retry.public_record(plan["renderer_source"]["path"], plan["renderer_source"]["sha256"], limit=MAX_PUBLIC_BYTES)
    request = dict(host_kind=plan["host_kind"], native=plan["native"],
                   candidate_sha256=plan["candidate"]["sha256"], candidate_base64=base64.b64encode(candidate).decode(),
                   renderer_source_sha256=plan["renderer_source"]["sha256"])
    result = subprocess.run(argv, input=remote_program(request), stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=80)
    require(len(result.stdout) <= 65536, "receipt_size_bound")
    try:
        receipt = json.loads(result.stdout)
    except (ValueError, UnicodeDecodeError):
        raise CheckError("native_receipt_unavailable") from None
    receipt = admit_receipt(receipt, result.returncode, plan)
    receipt["ssh_stderr_bytes"] = len(result.stderr)
    receipt["approved_deployment_sha256"] = plan["deployment_reference"]["sha256"]
    return receipt


def main(argv: list[str] | None = None) -> int:
    """Run one explicit check without installing or reloading a candidate."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True, help="Public observed host/candidate metadata plan")
    args = parser.parse_args(argv)
    try:
        require(args.plan.stat().st_size <= MAX_PUBLIC_BYTES, "plan_size_bound")
        result = check(json.loads(args.plan.read_bytes()))
    except Exception as error:
        result = {"schema": RECEIPT_SCHEMA, "exit_code": 1, "activated": False, "reloaded": False,
                  "error_code": error.args[0] if isinstance(error, CheckError) else "controller_check_failed"}
    print(json.dumps(result, sort_keys=True))
    return result["exit_code"]


if __name__ == "__main__":
    raise SystemExit(main())
