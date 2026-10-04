#!/usr/bin/env python3
"""Render and provision an explicitly pinned native Taira validator SSH tunnel.

Prerequisites: approved MacStadium deployment descriptor, observed native file
identities, a native Mac ssh-keygen/ssh and a dedicated guest account. No private
key or existing SSH configuration body is read by the controller. Native keys
stay on the Mac. Each apply phase has a create-only owner-private journal; an
unknown outcome must be reconciled on that host before any retry.

The default command only renders public files. Apply requires an explicit phase
and reviewed public plan. API readiness and nginx CAS publication remain separate
deployment gates. There are no provider, credential or destination fallbacks.
"""
from __future__ import annotations

import argparse
import base64
import fcntl
import hashlib
import inspect
import ipaddress
import json
import os
from pathlib import Path
import plistlib
import re
import signal
import stat
import subprocess
import sys
import time

PLAN_SCHEMA = "iroha.taira.native-validator-forwarding.plan.v1"
RECEIPT_SCHEMA = "iroha.taira.native-validator-forwarding.receipt.v1"
JOURNAL_SCHEMA = "iroha.taira.native-validator-forwarding.journal.v1"
MAX_PUBLIC_BYTES = 262144
IDENTITY_FIELDS = {"device", "inode", "uid", "gid", "mode", "links", "size", "mtime_ns", "ctime_ns"}


class ForwardingError(RuntimeError):
    """A closed public refusal; native diagnostic bodies remain on their host."""


def need(condition, code):
    if not condition:
        raise ForwardingError(code)


def direct(value):
    need(isinstance(value, str) and re.fullmatch(r"/[A-Za-z0-9_./-]{1,1023}", value)
         and value == os.path.normpath(value) and ".." not in Path(value).parts, "direct_path_required")
    return value


def rfc1918(value):
    address = ipaddress.IPv4Address(value)
    return any(address in ipaddress.IPv4Network(block) for block in ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"))


def identity(info):
    return {name: str(getattr(info, "st_" + attr)) for name, attr in (
        ("device", "dev"), ("inode", "ino"), ("uid", "uid"), ("gid", "gid"),
        ("mode", "mode"), ("links", "nlink"), ("size", "size"),
        ("mtime_ns", "mtime_ns"), ("ctime_ns", "ctime_ns"))} | {"mode": str(stat.S_IMODE(info.st_mode))}


def validate_identity(value, directory=False):
    fields = {"device", "inode", "uid", "gid", "mode"} if directory else IDENTITY_FIELDS
    need(isinstance(value, dict) and set(value) == fields and all(isinstance(v, str)
         and re.fullmatch(r"0|[1-9][0-9]{0,19}", v) for v in value.values()), "native_identity_fields")
    need(not int(value["mode"]) & 0o022, "native_owner_custody")
    if not directory:
        need(value["links"] == "1", "native_single_link_required")


def validate_plan(plan):
    need(isinstance(plan, dict) and set(plan) == {"schema", "provider", "deployment_reference",
         "operation_id", "mac", "guest", "forwardings", "host_pin", "serving_clients"}, "plan_fields")
    need(plan["schema"] == PLAN_SCHEMA and plan["provider"] == "macstadium", "approved_provider_required")
    need(re.fullmatch(r"[0-9a-f]{32}", plan["operation_id"]) is not None, "operation_id")
    ref = plan["deployment_reference"]
    need(isinstance(ref, dict) and set(ref) == {"path", "sha256"}, "deployment_reference_fields")
    direct(ref["path"])
    need(re.fullmatch(r"[0-9a-f]{64}", ref["sha256"]) is not None, "deployment_reference_digest")
    clients = plan["serving_clients"]
    need(isinstance(clients, dict) and set(clients) == {"path", "identity", "sha256"}, "serving_clients_fields")
    direct(clients["path"]); validate_identity(clients["identity"])
    need(clients["identity"]["uid"] == "0" and re.fullmatch(r"[0-9a-f]{64}", clients["sha256"]), "serving_clients_custody")
    mac = plan["mac"]
    need(isinstance(mac, dict) and set(mac) == {"uid", "source_address", "root", "root_parent",
        "launch_agents", "label", "domain", "native"}, "mac_fields")
    need(type(mac["uid"]) is int and mac["uid"] > 0, "mac_owner_uid")
    need(rfc1918(mac["source_address"]), "guest_link_source")
    direct(mac["root"])
    need(re.fullmatch(r"[a-z][a-z0-9.-]{2,100}", mac["label"]) is not None
         and mac["domain"] == "gui/" + str(mac["uid"]), "launchd_identity")
    for directory in (mac["root_parent"], mac["launch_agents"]):
        need(isinstance(directory, dict) and set(directory) == {"path", "identity"}, "mac_directory_fields")
        direct(directory["path"]); validate_identity(directory["identity"], True)
        need(directory["identity"]["uid"] == str(mac["uid"]), "mac_directory_owner")
    need(str(Path(mac["root"]).parent) == mac["root_parent"]["path"], "root_parent_binding")
    need(isinstance(mac["native"], dict) and set(mac["native"]) == {"ssh", "ssh_keygen", "launchctl", "plutil"}, "mac_native_fields")
    for value in mac["native"].values():
        need(isinstance(value, dict) and set(value) == {"path", "identity"}, "native_fields")
        direct(value["path"]); validate_identity(value["identity"])
        need(value["identity"]["uid"] == "0", "native_program_owner")
    guest = plan["guest"]
    need(isinstance(guest, dict) and set(guest) == {"address", "port", "user", "authorization_root",
        "directory", "main", "master", "native"}, "guest_fields")
    need(rfc1918(guest["address"]) and guest["address"] != mac["source_address"], "guest_private_address")
    need(type(guest["port"]) is int and 1 <= guest["port"] <= 65535, "guest_port")
    need(re.fullmatch(r"[a-z][a-z0-9-]{2,31}", guest["user"]) is not None
         and guest["user"] not in {"root", "administrator", "admin"}, "dedicated_guest_user")
    direct(guest["authorization_root"])
    need(str(Path(guest["authorization_root"]).parent) == guest["directory"]["path"], "authorization_parent_binding")
    need(set(guest["directory"]) == {"path", "identity"}, "guest_directory_fields")
    direct(guest["directory"]["path"]); validate_identity(guest["directory"]["identity"], True)
    need(guest["directory"]["identity"]["uid"] == "0", "guest_directory_owner")
    need(isinstance(guest["native"], dict) and set(guest["native"]) == {"sshd", "useradd", "getent", "nologin"}, "guest_native_fields")
    for value in [guest["main"], *guest["native"].values()]:
        need(isinstance(value, dict) and set(value) == {"path", "identity"}, "guest_native_input_fields")
        direct(value["path"]); validate_identity(value["identity"])
        need(value["identity"]["uid"] == "0", "guest_native_input_owner")
    need(str(Path(guest["main"]["path"]).parent) == guest["directory"]["path"], "main_directory_binding")
    master = guest["master"]
    need(isinstance(master, dict) and set(master) == {"pid", "uid", "started", "executable", "start_ticks", "boot_id"}
         and type(master["pid"]) is int and 1 < master["pid"] <= 2147483647
         and master["uid"] == 0 and master["executable"] == guest["native"]["sshd"]["path"]
         and re.fullmatch(r"[A-Za-z0-9: ]{20,32}", master["started"])
         and re.fullmatch(r"[1-9][0-9]{0,19}", master["start_ticks"])
         and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", master["boot_id"]), "guest_master_identity")
    pin = plan["host_pin"]
    need(isinstance(pin, dict) and set(pin) == {"alias", "record", "fingerprint"}, "host_pin_fields")
    need(re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9._-]{0,100}", pin["alias"]) is not None
         and re.fullmatch(re.escape(pin["alias"]) + r" ssh-ed25519 [A-Za-z0-9+/]{68}", pin["record"]) is not None,
         "public_guest_host_pin")
    blob = base64.b64decode(pin["record"].split()[2], validate=True)
    need(len(blob) == 51 and blob[:19] == b"\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20"
         and pin["fingerprint"] == "SHA256:" + base64.b64encode(hashlib.sha256(blob).digest()).decode().rstrip("="), "guest_host_pin_fingerprint")
    rows = plan["forwardings"]
    need(isinstance(rows, list) and 1 <= len(rows) <= 32, "forwarding_count")
    local, target = set(), set()
    for row in rows:
        need(isinstance(row, dict) and set(row) == {"listen_address", "listen_port", "target_address", "target_port"}, "forwarding_fields")
        need(row["listen_address"] == row["target_address"] == "127.0.0.1"
             and all(type(row[k]) is int and 1024 <= row[k] <= 65535 for k in ("listen_port", "target_port")), "loopback_forwarding_only")
        need(row["listen_port"] not in local and row["target_port"] not in target, "forwarding_unique")
        local.add(row["listen_port"]); target.add(row["target_port"])
    return plan


def validate_public_key(public_key):
    need(isinstance(public_key, str) and re.fullmatch(r"ssh-ed25519 [A-Za-z0-9+/]{68}(?: [A-Za-z0-9._-]{1,128})?", public_key), "native_public_key")
    blob = base64.b64decode(public_key.split()[1], validate=True)
    need(len(blob) == 51 and blob[:19] == b"\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20", "native_public_key_shape")
    return " ".join(public_key.split()[:2])


def render_authorized_key(plan, public_key):
    validate_plan(plan)
    options = ["restrict", "port-forwarding", 'from="' + plan["mac"]["source_address"] + '"']
    options += ['permitopen="127.0.0.1:' + str(row["target_port"]) + '"' for row in plan["forwardings"]]
    return (",".join(options) + " " + validate_public_key(public_key) + "\n").encode()


def effective_restrictions(plan):
    return {"authenticationmethods": "publickey", "pubkeyauthentication": "yes", "passwordauthentication": "no",
        "kbdinteractiveauthentication": "no", "allowtcpforwarding": "local", "allowstreamlocalforwarding": "no",
        "permitopen": " ".join("127.0.0.1:" + str(r["target_port"]) for r in plan["forwardings"]),
        "permitlisten": "none", "permittty": "no", "x11forwarding": "no", "allowagentforwarding": "no",
        "disableforwarding": "no", "permituserrc": "no",
        "permittunnel": "no", "gatewayports": "no", "maxsessions": "0",
        "authorizedkeysfile": plan["guest"]["authorization_root"] + "/authorized_keys"}


def render_sshd_match(plan):
    validate_plan(plan)
    fields = effective_restrictions(plan)
    return ("# Dedicated validator forwarding; no session channels.\nMatch User " + plan["guest"]["user"] + "\n"
        + "".join("    " + key + " " + value + "\n" for key, value in fields.items()) + "Match all\n").encode()


def ssh_argv(plan):
    validate_plan(plan)
    mac, guest = plan["mac"], plan["guest"]
    argv = [mac["native"]["ssh"]["path"], "-F", "/dev/null", "-N", "-T", "-b", mac["source_address"],
            "-i", mac["root"] + "/id_ed25519"]
    settings = {"BatchMode": "yes", "IdentitiesOnly": "yes", "IdentityAgent": "none", "ForwardAgent": "no",
        "StrictHostKeyChecking": "yes", "UserKnownHostsFile": mac["root"] + "/known_hosts",
        "GlobalKnownHostsFile": "/dev/null", "HostKeyAlias": plan["host_pin"]["alias"],
        "UpdateHostKeys": "no", "VerifyHostKeyDNS": "no", "PreferredAuthentications": "publickey",
        "PasswordAuthentication": "no", "KbdInteractiveAuthentication": "no", "UseKeychain": "no",
        "AddKeysToAgent": "no", "ExitOnForwardFailure": "yes", "ServerAliveInterval": "15",
        "ServerAliveCountMax": "3", "ConnectTimeout": "8"}
    for key, value in settings.items(): argv += ["-o", key + "=" + value]
    for row in plan["forwardings"]:
        argv += ["-L", "127.0.0.1:" + str(row["listen_port"]) + ":127.0.0.1:" + str(row["target_port"])]
    return argv + ["-p", str(guest["port"]), guest["user"] + "@" + guest["address"]]


def render_launch_agent(plan):
    validate_plan(plan)
    mac = plan["mac"]
    return plistlib.dumps(dict(Label=mac["label"], ProgramArguments=ssh_argv(plan), RunAtLoad=True, KeepAlive=True,
        ThrottleInterval=10, ProcessType="Background", Umask=0o077,
        StandardOutPath="/dev/null", StandardErrorPath="/dev/null"), sort_keys=True)


def render_bundle(plan, public_key=None):
    validate_plan(plan)
    files = {"known_hosts": (plan["host_pin"]["record"] + "\n").encode(),
             "sshd-match.conf": render_sshd_match(plan), "forwarder.plist": render_launch_agent(plan)}
    if public_key is not None: files["authorized_keys"] = render_authorized_key(plan, public_key)
    return files


def open_directory(path):
    """Resolve each absolute path component without following a symlink."""
    direct(path)
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        for component in Path(path).parts[1:]:
            next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=fd)
            os.close(fd); fd = next_fd
        return fd
    except BaseException:
        os.close(fd); raise


def open_observed(reference, directory=False):
    parent = open_directory(str(Path(reference["path"]).parent))
    try:
        fd = os.open(Path(reference["path"]).name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
                     | (os.O_DIRECTORY if directory else os.O_NONBLOCK), dir_fd=parent)
    finally:
        os.close(parent)
    try:
        actual = identity(os.fstat(fd))
        if directory: actual = {k: actual[k] for k in reference["identity"]}
        need(actual == reference["identity"], "observed_identity_changed")
        need((stat.S_ISDIR if directory else stat.S_ISREG)(os.fstat(fd).st_mode), "observed_input_type")
        return fd
    except BaseException:
        os.close(fd); raise


def verify_path_fd(path, fd, expected):
    parent = open_directory(str(Path(path).parent))
    try:
        now = os.stat(Path(path).name, dir_fd=parent, follow_symlinks=False)
        need(identity(now) == identity(os.fstat(fd)) == expected, "retained_path_identity_changed")
    finally:
        os.close(parent)


def backup_original(directory, main_name, main_fd, original, backup_name):
    """Retain the exact private source inode; compare all unchanged attributes."""
    need(identity(os.stat(main_name, dir_fd=directory, follow_symlinks=False)) == identity(os.fstat(main_fd)) == original,
         "original_backup_identity_changed")
    os.link(main_name, backup_name, src_dir_fd=directory, dst_dir_fd=directory, follow_symlinks=False)
    opened = os.open(backup_name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=directory)
    backed = identity(os.fstat(main_fd))
    try:
        need(identity(os.fstat(opened)) == identity(os.stat(main_name, dir_fd=directory, follow_symlinks=False)) == backed
             and all(backed[k] == original[k] for k in original if k not in {"links", "ctime_ns"})
             and backed["links"] == "2", "original_backup_identity_changed")
        os.fsync(directory)
        return opened, backed
    except BaseException: os.close(opened); raise


def replace_owned_main(directory, main_name, main_fd, expected_main, validation_name, new_fd, expected_new):
    need(identity(os.stat(main_name, dir_fd=directory, follow_symlinks=False)) == identity(os.fstat(main_fd)) == expected_main,
         "main_cas_identity_changed")
    need(identity(os.stat(validation_name, dir_fd=directory, follow_symlinks=False)) == identity(os.fstat(new_fd)) == expected_new,
         "main_cas_identity_changed")
    os.replace(validation_name, main_name, src_dir_fd=directory, dst_dir_fd=directory)


def renamed_identity(before, after, unlink_original=False):
    """Admit only the rename/link-count changes caused by our publication."""
    ignored = {"ctime_ns", "links"} if unlink_original else {"ctime_ns"}
    need(all(before[k] == after[k] for k in before if k not in ignored)
         and int(after["ctime_ns"]) >= int(before["ctime_ns"]), "publication_identity_changed")
    if unlink_original: need(before["links"] == "2" and after["links"] == "1", "publication_identity_changed")
    return after


def restore_owned_main(directory, main_name, new_fd, expected_new, backup_name, backup_fd, expected_backup):
    """Refuse rollback on pathname substitution or in-place source mutation."""
    need(identity(os.stat(main_name, dir_fd=directory, follow_symlinks=False)) == identity(os.fstat(new_fd)) == expected_new,
         "main_cas_identity_changed")
    need(identity(os.stat(backup_name, dir_fd=directory, follow_symlinks=False)) == identity(os.fstat(backup_fd)) == expected_backup,
         "backup_identity_changed")
    os.replace(backup_name, main_name, src_dir_fd=directory, dst_dir_fd=directory)
    os.fsync(directory)


def create_public(directory, name, raw, mode=0o600):
    """Create exact public bytes and retain their descriptor/inode/content digest."""
    need(re.fullmatch(r"[A-Za-z0-9._-]{1,160}", name) and len(raw) <= MAX_PUBLIC_BYTES, "public_file_bound")
    fd = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, mode, dir_fd=directory)
    try:
        need(os.write(fd, raw) == len(raw), "public_write_incomplete")
        os.fchmod(fd, mode); os.fsync(fd); os.fsync(directory)
        return fd, identity(os.fstat(fd)), hashlib.sha256(raw).hexdigest()
    except BaseException:
        os.close(fd); raise


def journal_writer(directory, plan, host_kind, create=True):
    """Retain a bounded owner-private append-only public metadata journal."""
    name = ".taira-validator-forwarding-" + plan["operation_id"] + ".receipt.ndjson"
    flags = os.O_RDWR | os.O_APPEND | os.O_NOFOLLOW | os.O_CLOEXEC
    if create: flags |= os.O_CREAT | os.O_EXCL
    fd = os.open(name, flags, 0o600, dir_fd=directory)
    before = identity(os.fstat(fd))
    need(before["uid"] == str(os.geteuid()) and before["mode"] == "384" and before["links"] == "1", "journal_custody")
    sequence, last = 0, None
    if not create:
        need(int(before["size"]) <= 1024 * 1024, "journal_size_bound")
        raw = os.pread(fd, int(before["size"]) + 1, 0)
        need(identity(os.stat(name, dir_fd=directory, follow_symlinks=False)) == identity(os.fstat(fd)) == before,
             "journal_path_changed")
        for line in raw.splitlines():
            record = json.loads(line)
            need(record.get("schema") == JOURNAL_SCHEMA and record.get("operation_id") == plan["operation_id"]
                 and record.get("host_kind") == host_kind and record.get("sequence") == sequence + 1
                 and record.get("plan_sha256") == hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest(), "journal_operation_changed")
            sequence += 1; last = record
    def append(phase, **details):
        nonlocal before, sequence, last
        need(identity(os.stat(name, dir_fd=directory, follow_symlinks=False)) == identity(os.fstat(fd)) == before,
             "journal_path_changed")
        need(sequence < 32 and int(before["size"]) < 1024 * 1024, "journal_size_bound")
        sequence += 1
        record = dict(schema=JOURNAL_SCHEMA, operation_id=plan["operation_id"], host_kind=host_kind,
            sequence=sequence, phase=phase, qualified=False,
            plan_sha256=hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest(), **details)
        raw = (json.dumps(record, sort_keys=True) + "\n").encode()
        need(len(raw) <= 65536 and os.write(fd, raw) == len(raw), "journal_write_incomplete")
        os.fsync(fd); os.fsync(directory); before = identity(os.fstat(fd)); last = record
        need(identity(os.stat(name, dir_fd=directory, follow_symlinks=False)) == before, "journal_path_changed")
        return record
    return fd, append, last


def mac_identity(plan):
    """Generate a new native-only key; the controller receives public metadata."""
    validate_plan(plan)
    mac = plan["mac"]
    need(sys.platform == "darwin" and os.geteuid() == mac["uid"], "mac_owner_required")
    parent = open_observed(mac["root_parent"], True)
    programs = {k: open_observed(v) for k, v in mac["native"].items()}
    directory = key_fd = journal_fd = None
    receipt = dict(schema=RECEIPT_SCHEMA, phase="identity_refused", exit_code=1, qualified=False,
                   private_bytes_read=False, operation_id=plan["operation_id"], host_kind="macos",
                   plan_sha256=hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest())
    try:
        journal_fd, journal, _ = journal_writer(parent, plan, "macos")
        journal("identity_prepared", root=mac["root"], native_programs=mac["native"])
        os.mkdir(Path(mac["root"]).name, 0o700, dir_fd=parent)
        os.fsync(parent); directory = open_directory(mac["root"])
        for name, fd in programs.items(): verify_path_fd(mac["native"][name]["path"], fd, mac["native"][name]["identity"])
        journal("key_generating", private_key_path=mac["root"] + "/id_ed25519")
        result = subprocess.run([mac["native"]["ssh_keygen"]["path"], "-q", "-t", "ed25519", "-N", "", "-C",
            mac["label"], "-f", mac["root"] + "/id_ed25519"], stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
        need(result.returncode == 0, "native_key_generation_failed")
        key_fd = os.open("id_ed25519", os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=directory)
        key_identity = identity(os.fstat(key_fd))
        need(stat.S_ISREG(os.fstat(key_fd).st_mode) and key_identity["uid"] == str(mac["uid"])
             and key_identity["mode"] == "384" and key_identity["links"] == "1", "native_key_custody")
        public = subprocess.run([mac["native"]["ssh_keygen"]["path"], "-y", "-P", "", "-f", mac["root"] + "/id_ed25519"],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=10)
        need(public.returncode == 0 and len(public.stdout) <= 1024, "native_public_derivation_failed")
        key = validate_public_key(public.stdout.decode().strip())
        verify_path_fd(mac["root"] + "/id_ed25519", key_fd, key_identity)
        fd, known_identity, known_sha = create_public(directory, "known_hosts", (plan["host_pin"]["record"] + "\n").encode())
        os.close(fd)
        fingerprint = "SHA256:" + base64.b64encode(hashlib.sha256(base64.b64decode(key.split()[1])).digest()).decode().rstrip("=")
        root_identity = {k: identity(os.fstat(directory))[k] for k in ("device", "inode", "uid", "gid", "mode")}
        journal("identity_ready", private_key_identity=key_identity, public_key=key,
                public_key_fingerprint=fingerprint, known_hosts_identity=known_identity, known_hosts_sha256=known_sha)
        receipt.update(phase="identity_ready", exit_code=0, private_key_identity=key_identity, public_key=key,
            public_key_fingerprint=fingerprint, known_hosts_identity=known_identity, known_hosts_sha256=known_sha,
            root_identity=root_identity, journal_path=mac["root_parent"]["path"] + "/.taira-validator-forwarding-" + plan["operation_id"] + ".receipt.ndjson")
    except Exception as error:
        receipt["error_code"] = error.args[0] if isinstance(error, ForwardingError) else "native_identity_refused"
    finally:
        for fd in [parent, directory, key_fd, journal_fd, *programs.values()]:
            if fd is not None: os.close(fd)
    return receipt


def closed_error_codes():
    """Codes are source constants, never arbitrary native diagnostic text."""
    return {"native_phase_refused", "native_identity_refused", "native_authorization_refused", "native_launch_refused",
        "native_key_generation_failed", "native_key_custody", "native_public_derivation_failed", "native_public_key",
        "native_public_key_shape", "retained_path_identity_changed", "observed_identity_changed", "observed_input_type",
        "journal_custody", "journal_size_bound", "journal_operation_changed", "journal_path_changed", "journal_write_incomplete",
        "public_write_incomplete", "public_file_bound", "public_content_changed", "guest_lock_custody", "master_identity_changed", "master_handle_unavailable",
        "dedicated_account_already_exists", "native_account_creation_failed", "native_main_transform_failed", "native_main_bound",
        "native_effective_policy_failed", "effective_restrictions_refused", "native_sshd_check_failed", "unrelated_ssh_policy_changed",
        "original_backup_identity_changed", "main_cas_identity_changed", "publication_identity_changed", "active_sshd_check_failed", "reload_signal_ambiguous", "backup_identity_changed",
        "rollback_sshd_check_failed", "prior_operation_needs_reconciliation", "launchd_service_already_exists",
        "native_plist_check_failed", "launchd_bootstrap_ambiguous", "mac_owner_required", "guest_root_required"}


SSHD_APPEND_GUARD = r'''
BEGIN { bytes=0; count=0; bad=0 }
{ bytes+=length($0)+1; if(bytes>1048576 || length($0)>8192) {bad=1;exit 41}
  if(index($0, fragment)) {bad=1;exit 41} print }
END { if(bad) exit 41; print "\nMatch all\nInclude " fragment }
'''


def native_effective(plan, main, user, expected=None):
    """Native SSH consumes private config; a bounded awk projects only public policy."""
    guest = plan["guest"]
    context = "user=" + user + ",addr=" + plan["mac"]["source_address"] + ",laddr=" + guest["address"] + ",lport=" + str(guest["port"])
    fields = effective_restrictions(plan)
    # The projection is an allowlisted public SSH policy record, never main text.
    projection = r'''
BEGIN { bytes=0; count=0; n=split(keys, names, " "); for(i in names) allowed[names[i]]=1 }
{ bytes+=length($0)+1; if(bytes>1048576 || length($0)>8192) exit 41
 if($1 in allowed) { if(seen[$1]++) exit 41; if($0 !~ /^[A-Za-z0-9_~%\/.*:@+ -]+$/) exit 41; print; count++ } }
END { if(count!=n) exit 41 }
'''
    source = subprocess.Popen([guest["native"]["sshd"]["path"], "-T", "-f", main, "-C", context],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    try:
        guarded = subprocess.run(["/usr/bin/awk", "-v", "keys=" + " ".join(fields), projection], stdin=source.stdout,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=15)
        source.stdout.close(); native_exit = source.wait(timeout=5)
        need(native_exit == guarded.returncode == 0 and len(guarded.stdout) <= 8192, "native_effective_policy_failed")
        values = dict(line.split(" ", 1) for line in guarded.stdout.decode().splitlines())
        if expected is not None: need(values == expected, "effective_restrictions_refused")
        return values
    finally:
        if source.poll() is None: source.terminate(); source.wait(timeout=5)


def guest_authorize(plan, identity_receipt):
    """Create one account and Match, preserving private main custody and rollback."""
    validate_plan(plan); admit_identity_receipt(identity_receipt, plan)
    need(sys.platform.startswith("linux") and os.geteuid() == 0, "guest_root_required")
    import taira_native_nginx_apply as process_owner
    guest = plan["guest"]
    parent = open_observed(guest["directory"], True)
    main_fd = open_observed(guest["main"])
    programs = {k: open_observed(v) for k, v in guest["native"].items()}
    directory = journal_fd = lock_fd = master_handle = new_fd = backup_fd = key_fd = fragment_fd = None
    originals = guest["main"]["identity"]
    receipt = dict(schema=RECEIPT_SCHEMA, operation_id=plan["operation_id"], host_kind="linux", phase="authorization_refused",
        exit_code=1, qualified=False, private_bytes_read=False, main_published=False, reload_requested=False,
        plan_sha256=hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest())
    published = False; backup = ".taira-forwarding-original-" + plan["operation_id"]
    validation = ".taira-forwarding-validation-" + plan["operation_id"]
    main_name = Path(guest["main"]["path"]).name
    try:
        lock_fd = os.open(".taira-validator-forwarding.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=parent)
        need(stat.S_ISREG(os.fstat(lock_fd).st_mode) and os.fstat(lock_fd).st_uid == 0
             and stat.S_IMODE(os.fstat(lock_fd).st_mode) == 0o600 and os.fstat(lock_fd).st_nlink == 1, "guest_lock_custody")
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        admit_serving_clients(plan)
        verify_path_fd(guest["main"]["path"], main_fd, originals)
        need(process_owner.observe_master(guest["master"]) == guest["master"], "master_identity_changed")
        master_handle = process_owner.open_master_handle(guest["master"])
        need(subprocess.run([guest["native"]["getent"]["path"], "passwd", guest["user"]],
             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 2, "dedicated_account_already_exists")
        journal_fd, journal, _ = journal_writer(parent, plan, "linux")
        journal("authorization_prepared", main=guest["main"], master=guest["master"],
            public_key_fingerprint=identity_receipt["public_key_fingerprint"], user=guest["user"])
        os.mkdir(Path(guest["authorization_root"]).name, 0o755, dir_fd=parent); os.fsync(parent)
        directory = open_directory(guest["authorization_root"])
        controls = {user: native_effective(plan, guest["main"]["path"], user) for user in ("root", "nobody")}
        journal("account_creating", user=guest["user"])
        result = subprocess.run([guest["native"]["useradd"]["path"], "-r", "-M", "-d", guest["authorization_root"],
             "-s", guest["native"]["nologin"]["path"], "-p", "*", guest["user"]], stdout=subprocess.DEVNULL,
             stderr=subprocess.DEVNULL, timeout=15)
        need(result.returncode == 0, "native_account_creation_failed")
        key_fd, key_identity, key_sha = create_public(directory, "authorized_keys", render_authorized_key(plan, identity_receipt["public_key"]), 0o644)
        fragment_fd, fragment_identity, fragment_sha = create_public(directory, "sshd-match.conf", render_sshd_match(plan))
        journal("public_authorization_created", authorized_key_identity=key_identity, authorized_key_sha256=key_sha,
            fragment_identity=fragment_identity, fragment_sha256=fragment_sha)
        new_fd = os.open(validation, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=parent)
        journal("native_main_preparing", validation_path=str(Path(guest["directory"]["path"]) / validation))
        os.lseek(main_fd, 0, os.SEEK_SET)
        result = subprocess.run(["/usr/bin/awk", "-v", "fragment=" + guest["authorization_root"] + "/sshd-match.conf",
            SSHD_APPEND_GUARD], stdin=main_fd, stdout=new_fd, stderr=subprocess.DEVNULL, timeout=15)
        need(result.returncode == 0, "native_main_transform_failed")
        os.fsync(new_fd); new_identity = identity(os.fstat(new_fd))
        need(int(new_identity["size"]) <= 1048576 and new_identity["mode"] == "384", "native_main_bound")
        new_path = str(Path(guest["directory"]["path"]) / validation)
        verify_path_fd(guest["main"]["path"], main_fd, originals)
        verify_path_fd(new_path, new_fd, new_identity)
        need(subprocess.run([guest["native"]["sshd"]["path"], "-t", "-f", new_path],
             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15).returncode == 0, "native_sshd_check_failed")
        native_effective(plan, new_path, guest["user"], effective_restrictions(plan))
        need(all(native_effective(plan, new_path, u) == policy for u, policy in controls.items()), "unrelated_ssh_policy_changed")
        verify_public_fd(guest["authorization_root"] + "/authorized_keys", key_fd, key_identity, key_sha)
        verify_public_fd(guest["authorization_root"] + "/sshd-match.conf", fragment_fd, fragment_identity, fragment_sha)
        verify_path_fd(new_path, new_fd, new_identity)
        verify_path_fd(guest["main"]["path"], main_fd, originals)
        for name, fd in programs.items(): verify_path_fd(guest["native"][name]["path"], fd, guest["native"][name]["identity"])
        need(process_owner.observe_master(guest["master"]) == guest["master"], "master_identity_changed")
        journal("main_publishing", original_identity=originals, original_backup_path=str(Path(guest["directory"]["path"]) / backup),
            publication_identity=new_identity, destination=guest["main"]["path"])
        backup_fd, original_backed = backup_original(parent, main_name, main_fd, originals, backup)
        replace_owned_main(parent, main_name, main_fd, original_backed, validation, new_fd, new_identity)
        published = True; os.fsync(parent)
        new_identity = renamed_identity(new_identity, identity(os.fstat(new_fd)))
        verify_path_fd(guest["main"]["path"], new_fd, new_identity)
        backup_identity = renamed_identity(original_backed, identity(os.fstat(backup_fd)), True)
        journal("main_published", publication_identity=new_identity, original_backup_identity=backup_identity)
        need(subprocess.run([guest["native"]["sshd"]["path"], "-t", "-f", guest["main"]["path"]],
             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15).returncode == 0, "active_sshd_check_failed")
        native_effective(plan, guest["main"]["path"], guest["user"], effective_restrictions(plan))
        need(all(native_effective(plan, guest["main"]["path"], u) == policy for u, policy in controls.items()), "unrelated_ssh_policy_changed")
        verify_public_fd(guest["authorization_root"] + "/authorized_keys", key_fd, key_identity, key_sha)
        verify_public_fd(guest["authorization_root"] + "/sshd-match.conf", fragment_fd, fragment_identity, fragment_sha)
        verify_path_fd(guest["main"]["path"], new_fd, new_identity)
        journal("reload_requested", master=guest["master"])
        mode = process_owner.signal_master(guest["master"], master_handle)
        journal("authorization_ready", reload_signal_mode=mode, publication_identity=new_identity, qualified=False)
        receipt.update(phase="authorization_ready", exit_code=0, main_published=True, reload_requested=True,
            reload_signal_mode=mode, journal_path=guest["directory"]["path"] + "/.taira-validator-forwarding-" + plan["operation_id"] + ".receipt.ndjson",
            main_identity=new_identity, original_backup_path=str(Path(guest["directory"]["path"]) / backup),
            original_backup_identity=identity(os.fstat(backup_fd)), fragment_sha256=fragment_sha,
            authorized_key_sha256=key_sha, public_key_fingerprint=identity_receipt["public_key_fingerprint"],
            effective_restrictions_verified=True, unrelated_contexts_unchanged=True)
    except Exception as error:
        receipt["error_code"] = error.args[0] if isinstance(error, ForwardingError) else "native_authorization_refused"
        if published:
            try:
                verify_path_fd(guest["main"]["path"], new_fd, new_identity)
                journal("rollback_requested", publication_identity=new_identity)
                restore_owned_main(parent, main_name, new_fd, new_identity, backup, backup_fd, backup_identity)
                need(subprocess.run([guest["native"]["sshd"]["path"], "-t", "-f", guest["main"]["path"]],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15).returncode == 0, "rollback_sshd_check_failed")
                journal("rollback_reload_requested"); process_owner.signal_master(guest["master"], master_handle)
                journal("rolled_back_unqualified"); receipt.update(phase="rolled_back_unqualified", main_published=False)
            except Exception:
                receipt.update(phase="rollback_ambiguous", rollback_ambiguous=True)
        # Retain created account/public files and the private journal for explicit reconciliation.
    finally:
        for fd in [parent, main_fd, directory, journal_fd, lock_fd, master_handle, new_fd, backup_fd, key_fd, fragment_fd, *programs.values()]:
            if fd is not None: os.close(fd)
    return receipt


def verify_public_fd(path, fd, expected, digest):
    verify_path_fd(path, fd, expected)
    need(int(expected["size"]) <= MAX_PUBLIC_BYTES, "public_file_bound")
    raw = os.pread(fd, int(expected["size"]) + 1, 0)
    need(len(raw) == int(expected["size"]) and hashlib.sha256(raw).hexdigest() == digest, "public_content_changed")
    verify_path_fd(path, fd, expected)


def admit_serving_clients(plan):
    """Bind forwarding targets to the current native-generated public roster."""
    reference = plan["serving_clients"]
    fd = open_observed(reference)
    try:
        verify_public_fd(reference["path"], fd, reference["identity"], reference["sha256"])
        clients = json.loads(os.pread(fd, int(reference["identity"]["size"]) + 1, 0))
        verify_path_fd(reference["path"], fd, reference["identity"])
        need(isinstance(clients, list) and len(clients) == len(plan["forwardings"])
             and all(isinstance(row, dict) and set(row) == {"slug", "torii_origin", "probe_origin", "account_id", "peer_id"} for row in clients), "serving_clients_shape")
        need([row["probe_origin"].rstrip("/") for row in clients] == ["http://127.0.0.1:" + str(row["target_port"]) for row in plan["forwardings"]], "serving_targets_differ")
        return clients
    finally: os.close(fd)


def admit_identity_receipt(receipt, plan):
    fields = {"schema", "phase", "exit_code", "qualified", "private_bytes_read", "operation_id", "host_kind", "plan_sha256",
        "private_key_identity", "public_key", "public_key_fingerprint", "known_hosts_identity", "known_hosts_sha256",
        "root_identity", "journal_path"}
    need(isinstance(receipt, dict) and set(receipt) == fields and receipt["schema"] == RECEIPT_SCHEMA
         and receipt["phase"] == "identity_ready" and receipt["exit_code"] == 0
         and receipt["qualified"] is receipt["private_bytes_read"] is False
         and receipt["host_kind"] == "macos" and receipt["operation_id"] == plan["operation_id"]
         and receipt["plan_sha256"] == hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest(), "identity_receipt_fields")
    validate_identity(receipt["private_key_identity"]); validate_identity(receipt["known_hosts_identity"])
    validate_identity(receipt["root_identity"], True)
    for field in ("private_key_identity", "known_hosts_identity", "root_identity"):
        need(receipt[field]["uid"] == str(plan["mac"]["uid"]), "identity_receipt_owner")
    need(receipt["private_key_identity"]["mode"] == receipt["known_hosts_identity"]["mode"] == "384"
         and receipt["root_identity"]["mode"] == "448", "identity_receipt_mode")
    key = validate_public_key(receipt["public_key"])
    need(receipt["public_key_fingerprint"] == "SHA256:" + base64.b64encode(hashlib.sha256(base64.b64decode(key.split()[1])).digest()).decode().rstrip("=")
         and receipt["known_hosts_sha256"] == hashlib.sha256((plan["host_pin"]["record"] + "\n").encode()).hexdigest()
         and receipt["journal_path"] == plan["mac"]["root_parent"]["path"] + "/.taira-validator-forwarding-" + plan["operation_id"] + ".receipt.ndjson", "identity_receipt_binding")
    return receipt


def mac_launch(plan, identity_receipt):
    """Publish one persistent launchd agent, never a controller-owned SSH child."""
    validate_plan(plan); admit_identity_receipt(identity_receipt, plan)
    mac = plan["mac"]
    need(sys.platform == "darwin" and os.geteuid() == mac["uid"], "mac_owner_required")
    parent = open_observed(mac["root_parent"], True)
    directory = open_observed(dict(path=mac["root"], identity=identity_receipt["root_identity"]), True)
    agents = open_observed(mac["launch_agents"], True)
    programs = {k: open_observed(v) for k, v in mac["native"].items()}
    key_fd = open_observed(dict(path=mac["root"] + "/id_ed25519", identity=identity_receipt["private_key_identity"]))
    known_fd = open_observed(dict(path=mac["root"] + "/known_hosts", identity=identity_receipt["known_hosts_identity"]))
    journal_fd = plist_fd = None
    target = mac["domain"] + "/" + mac["label"]
    plist_path = mac["launch_agents"]["path"] + "/" + mac["label"] + ".plist"
    receipt = dict(schema=RECEIPT_SCHEMA, operation_id=plan["operation_id"], host_kind="macos", phase="launch_refused",
        exit_code=1, qualified=False, private_bytes_read=False, service_started=False,
        plan_sha256=hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest())
    try:
        fcntl.flock(directory, fcntl.LOCK_EX | fcntl.LOCK_NB)
        journal_fd, journal, last = journal_writer(parent, plan, "macos", False)
        need(last is not None and last["phase"] == "identity_ready", "prior_operation_needs_reconciliation")
        need(subprocess.run([mac["native"]["launchctl"]["path"], "print", target],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10).returncode != 0, "launchd_service_already_exists")
        for name, fd in programs.items(): verify_path_fd(mac["native"][name]["path"], fd, mac["native"][name]["identity"])
        verify_path_fd(mac["root"] + "/id_ed25519", key_fd, identity_receipt["private_key_identity"])
        verify_path_fd(mac["root"] + "/known_hosts", known_fd, identity_receipt["known_hosts_identity"])
        journal("launch_preparing", service=target, plist_path=plist_path,
                private_key_identity=identity_receipt["private_key_identity"], guest_host_fingerprint=plan["host_pin"]["fingerprint"])
        plist_fd, plist_identity, plist_sha = create_public(agents, mac["label"] + ".plist", render_launch_agent(plan))
        verify_path_fd(plist_path, plist_fd, plist_identity)
        need(subprocess.run([mac["native"]["plutil"]["path"], "-lint", plist_path],
             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10).returncode == 0, "native_plist_check_failed")
        journal("launch_requested", service=target, plist_identity=plist_identity, plist_sha256=plist_sha)
        result = subprocess.run([mac["native"]["launchctl"]["path"], "bootstrap", mac["domain"], plist_path],
             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
        need(result.returncode == 0, "launchd_bootstrap_ambiguous")
        verify_path_fd(plist_path, plist_fd, plist_identity)
        journal("awaiting_forwarding_readiness", service=target, plist_identity=plist_identity, plist_sha256=plist_sha)
        receipt.update(phase="awaiting_forwarding_readiness", exit_code=0, service_started=True,
            service=target, plist_path=plist_path, plist_identity=plist_identity, plist_sha256=plist_sha,
            journal_path=identity_receipt["journal_path"], public_key_fingerprint=identity_receipt["public_key_fingerprint"])
    except Exception as error:
        receipt["error_code"] = error.args[0] if isinstance(error, ForwardingError) else "native_launch_refused"
        # A bootstrap response cannot prove whether a durable service started.
        # Preserve exact public plist/journal, never bootout an uncertain owner.
    finally:
        for fd in [parent, directory, agents, key_fd, known_fd, journal_fd, plist_fd, *programs.values()]:
            if fd is not None: os.close(fd)
    return receipt


def remote_program(plan, phase, identity_receipt=None):
    """Send maintained source and public metadata only through pinned transport."""
    body = Path(__file__).read_text()
    body = body.rsplit('\nif __name__ == "__main__":', 1)[0]
    # Only the generic exact-master process owner is included, never its plans.
    import taira_native_nginx_apply as process_owner
    owner_source = "\n".join(inspect.getsource(f) for f in (process_owner.observe_master,
        process_owner.open_master_handle, process_owner.signal_master))
    body = body.replace("    import taira_native_nginx_apply as process_owner\n", "")
    code = body + "\n" + owner_source + "\nfrom types import SimpleNamespace\nprocess_owner=SimpleNamespace(observe_master=observe_master,open_master_handle=open_master_handle,signal_master=signal_master)\n"
    request = dict(plan=plan, phase=phase, identity_receipt=identity_receipt)
    code += "request=json.loads(" + repr(json.dumps(request)) + ")\n"
    code += "try:\n    result=" + phase.replace("-", "_") + "(request['plan']" + (",request['identity_receipt']" if phase != "mac-identity" else "") + ")\n"
    code += "except Exception:\n    result=dict(schema=RECEIPT_SCHEMA,exit_code=1,qualified=False,private_bytes_read=False,error_code='native_phase_refused',operation_id=request['plan']['operation_id'],plan_sha256=hashlib.sha256(json.dumps(request['plan'],sort_keys=True).encode()).hexdigest())\n"
    code += "print(json.dumps(result,sort_keys=True))\nsys.exit(result['exit_code'])\n"
    return code.encode()


def apply_phase(plan, phase, identity_receipt=None):
    """Admit explicit deployment pins and exact guest endpoint before remote apply."""
    import taira_retry
    validate_plan(plan)
    need(phase in {"mac-identity", "guest-authorize", "mac-launch"}, "phase_required")
    if phase != "mac-identity": admit_identity_receipt(identity_receipt, plan)
    descriptor = json.loads(taira_retry.public_record(plan["deployment_reference"]["path"], plan["deployment_reference"]["sha256"]))
    route = descriptor["guest_ssh" if phase == "guest-authorize" else "backing_ssh"]
    need(not any(word in json.dumps(route).lower() for word in ("vultr", "amazonaws", "aws.amazon")), "banned_provider")
    guest_argv = taira_retry.validate_ssh(descriptor["guest_ssh"])
    need("root@" + plan["guest"]["address"] in guest_argv and plan["guest"]["port"] == 22, "approved_guest_route_binding")
    argv = taira_retry.validate_ssh(route)
    result = subprocess.run(argv, input=remote_program(plan, phase, identity_receipt), stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=180)
    need(len(result.stdout) <= 65536, "receipt_size_bound")
    receipt = json.loads(result.stdout)
    admit_receipt(receipt, plan, phase, result.returncode)
    return receipt


def admit_receipt(receipt, plan, phase, exit_code):
    core = {"schema", "operation_id", "phase", "host_kind", "exit_code", "qualified", "private_bytes_read", "error_code", "plan_sha256"}
    effects = {"private_key_identity", "public_key", "public_key_fingerprint", "known_hosts_identity", "known_hosts_sha256",
        "root_identity", "journal_path", "main_published", "reload_requested", "reload_signal_mode", "main_identity",
        "original_backup_path", "original_backup_identity", "fragment_sha256", "authorized_key_sha256",
        "effective_restrictions_verified", "unrelated_contexts_unchanged", "rollback_ambiguous", "service_started",
        "service", "plist_path", "plist_identity", "plist_sha256"}
    need(isinstance(receipt, dict) and set(receipt) <= core | effects and receipt.get("schema") == RECEIPT_SCHEMA
         and receipt.get("operation_id") == plan["operation_id"] and receipt.get("exit_code") == exit_code
         and receipt.get("plan_sha256") == hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest()
         and type(exit_code) is int and exit_code in {0, 1} and receipt.get("qualified") is receipt.get("private_bytes_read") is False,
         "closed_receipt_fields")
    if "error_code" in receipt:
        need(receipt["error_code"] in closed_error_codes(), "closed_receipt_error")
    need(receipt.get("host_kind") in {"linux" if phase == "guest-authorize" else "macos", None}, "closed_receipt_host")
    need(receipt.get("phase") in {"identity_refused", "identity_ready", "authorization_refused", "authorization_ready",
         "rolled_back_unqualified", "rollback_ambiguous", "launch_refused", "awaiting_forwarding_readiness", None}, "closed_receipt_phase")
    for name in {"main_published", "reload_requested", "effective_restrictions_verified", "unrelated_contexts_unchanged", "rollback_ambiguous", "service_started"}:
        need(name not in receipt or type(receipt[name]) is bool, "closed_receipt_boolean")
    for name in {"private_key_identity", "known_hosts_identity", "main_identity", "original_backup_identity", "plist_identity"}:
        if name in receipt: validate_identity(receipt[name])
    if "root_identity" in receipt: validate_identity(receipt["root_identity"], True)
    if "public_key" in receipt: validate_public_key(receipt["public_key"])
    if "public_key_fingerprint" in receipt:
        need(re.fullmatch(r"SHA256:[A-Za-z0-9+/]{43}", receipt["public_key_fingerprint"]), "closed_receipt_fingerprint")
    for name in {"known_hosts_sha256", "fragment_sha256", "authorized_key_sha256", "plist_sha256"}:
        need(name not in receipt or re.fullmatch(r"[0-9a-f]{64}", receipt[name]), "closed_receipt_digest")
    if "journal_path" in receipt:
        parent = plan["guest"]["directory"]["path"] if phase == "guest-authorize" else plan["mac"]["root_parent"]["path"]
        need(receipt["journal_path"] == parent + "/.taira-validator-forwarding-" + plan["operation_id"] + ".receipt.ndjson", "closed_receipt_journal")
    if "original_backup_path" in receipt:
        need(receipt["original_backup_path"] == plan["guest"]["directory"]["path"] + "/.taira-forwarding-original-" + plan["operation_id"], "closed_receipt_backup")
    if "reload_signal_mode" in receipt: need(receipt["reload_signal_mode"] == "pidfd", "closed_receipt_signal")
    if "service" in receipt: need(receipt["service"] == plan["mac"]["domain"] + "/" + plan["mac"]["label"], "closed_receipt_service")
    if "plist_path" in receipt:
        need(receipt["plist_path"] == plan["mac"]["launch_agents"]["path"] + "/" + plan["mac"]["label"] + ".plist", "closed_receipt_plist")
    if exit_code == 0:
        if phase == "mac-identity": admit_identity_receipt(receipt, plan)
        elif phase == "guest-authorize":
            need(receipt.get("phase") == "authorization_ready" and receipt.get("main_published") is receipt.get("reload_requested") is True
                 and receipt.get("effective_restrictions_verified") is receipt.get("unrelated_contexts_unchanged") is True
                 and receipt.get("reload_signal_mode") == "pidfd", "authorization_receipt_success")
        else:
            need(receipt.get("phase") == "awaiting_forwarding_readiness" and receipt.get("service_started") is True
                 and receipt.get("service") == plan["mac"]["domain"] + "/" + plan["mac"]["label"]
                 and receipt.get("plist_path") == plan["mac"]["launch_agents"]["path"] + "/" + plan["mac"]["label"] + ".plist"
                 and receipt.get("plist_sha256") == hashlib.sha256(render_launch_agent(plan)).hexdigest(), "launch_receipt_success")
    return receipt


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--render-directory", type=Path, help="New owner-private public output directory")
    mode.add_argument("--apply-phase", choices=("mac-identity", "guest-authorize", "mac-launch"), help="Explicit reviewed provisioning phase")
    parser.add_argument("--identity-receipt", type=Path, help="Pinned native public key receipt; never a private key")
    parser.add_argument("--identity-receipt-sha256", help="Exact public identity receipt digest")
    parser.add_argument("--public-key", help="Native-derived public ed25519 key only")
    args = parser.parse_args(argv)
    import taira_retry
    try:
        plan = validate_plan(json.loads(taira_retry.public_record(args.plan, limit=MAX_PUBLIC_BYTES)))
        if args.apply_phase:
            identity_receipt = None
            if args.apply_phase != "mac-identity":
                need(args.identity_receipt is not None and args.identity_receipt_sha256 is not None, "identity_receipt_pin_required")
                identity_receipt = json.loads(taira_retry.public_record(args.identity_receipt, args.identity_receipt_sha256, limit=MAX_PUBLIC_BYTES))
            result = apply_phase(plan, args.apply_phase, identity_receipt)
            print(json.dumps(result, sort_keys=True)); return result["exit_code"]
        args.render_directory.mkdir(mode=0o700)
        files = render_bundle(plan, args.public_key)
        records = []
        for name, raw in files.items():
            path = args.render_directory / name
            digest = taira_retry.write_public_bytes(path, raw)
            records.append(dict(path=str(path), sha256=digest, bytes=len(raw)))
        result = dict(schema=RECEIPT_SCHEMA, phase="rendered", exit_code=0, qualified=False,
                      operation_id=plan["operation_id"], files=records, private_bytes_read=False, activated=False)
    except Exception as error:
        result = dict(schema=RECEIPT_SCHEMA, exit_code=1, qualified=False,
                      error_code=error.args[0] if isinstance(error, ForwardingError) else "render_refused")
    print(json.dumps(result, sort_keys=True))
    return result["exit_code"]


if __name__ == "__main__":
    raise SystemExit(main())
