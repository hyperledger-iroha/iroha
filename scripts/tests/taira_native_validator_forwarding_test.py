"""Native forwarding restrictions, credential custody and interrupted journals."""
import base64
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import shutil
import stat
import subprocess
import sys

import pytest

SOURCE = Path(__file__).resolve().parents[1] / "taira_native_validator_forwarding.py"
sys.path.insert(0, str(SOURCE.parent))
SPEC = importlib.util.spec_from_file_location("taira_native_validator_forwarding", SOURCE)
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
KEY_BLOB = b"\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20" + bytes(range(32))
KEY = "ssh-ed25519 " + base64.b64encode(KEY_BLOB).decode()


@pytest.fixture
def plan():
    native = dict(device="1", inode="2", uid="0", gid="0", mode="493", links="1", size="100", mtime_ns="1", ctime_ns="1")
    def file(path): return dict(path=path, identity=copy.deepcopy(native))
    def directory(path, uid):
        return dict(path=path, identity=dict(device="1", inode="9007199254741001", uid=str(uid), gid="20", mode="448"))
    return dict(schema=M.PLAN_SCHEMA, provider="macstadium", operation_id="a" * 32,
        deployment_reference=dict(path="/operator/bound-deployment.json", sha256="b" * 64),
        serving_clients=file("/runtime/public/validator-clients.json") | {"sha256": "c" * 64},
        mac=dict(uid=501, source_address="192.168.64.1", root="/operator/native-forwarding",
            root_parent=directory("/operator", 501), launch_agents=directory("/operator/Library/LaunchAgents", 501),
            label="org.sora.taira.validator-forwarding", domain="gui/501",
            native=dict(ssh=file("/usr/bin/ssh"), ssh_keygen=file("/usr/bin/ssh-keygen"),
                launchctl=file("/bin/launchctl"), plutil=file("/usr/bin/plutil"))),
        guest=dict(address="192.168.64.3", port=22, user="taira-edge-forwarder", authorization_root="/etc/ssh/native-forwarding",
            directory=directory("/etc/ssh", 0), main=file("/etc/ssh/sshd_config"),
            native=dict(sshd=file("/usr/sbin/sshd"), useradd=file("/usr/sbin/useradd"),
                getent=file("/usr/bin/getent"), nologin=file("/usr/sbin/nologin")),
            master=dict(pid=571, uid=0, started="Fri Oct 2 10:21:19 2026", executable="/usr/sbin/sshd",
                start_ticks="311", boot_id="00000000-0000-0000-0000-000000000000")),
        host_pin=dict(alias="approved-guest", record="approved-guest " + KEY,
            fingerprint="SHA256:" + base64.b64encode(hashlib.sha256(KEY_BLOB).digest()).decode().rstrip("=")),
        forwardings=[dict(listen_address="127.0.0.1", listen_port=18480 + i, target_address="127.0.0.1", target_port=10080 + i) for i in range(4)])


def test_generated_native_key_has_exact_forwarding_scope(plan):
    key = M.render_authorized_key(plan, KEY).decode()
    assert key.startswith('restrict,port-forwarding,from="192.168.64.1",')
    assert key.count("permitopen=") == 4
    for port in range(10080, 10084): assert 'permitopen="127.0.0.1:' + str(port) + '"' in key
    assert key.endswith(KEY + "\n")
    fields = M.effective_restrictions(plan)
    assert fields["maxsessions"] == "0" and fields["permitlisten"] == "none"
    assert fields["allowtcpforwarding"] == "local" and fields["allowstreamlocalforwarding"] == "no"
    assert fields["authenticationmethods"] == "publickey" and fields["passwordauthentication"] == "no"
    assert fields["permituserrc"] == fields["allowagentforwarding"] == fields["permittty"] == fields["x11forwarding"] == "no"
    assert M.render_sshd_match(plan).endswith(b"Match all\n")


def test_launchd_is_native_persistent_pinned_and_loopback_only(plan):
    value = plistlib.loads(M.render_launch_agent(plan))
    args = value["ProgramArguments"]
    assert value["KeepAlive"] and value["RunAtLoad"] and value["ThrottleInterval"] == 10
    assert args[0] == "/usr/bin/ssh" and args[1:5] == ["-F", "/dev/null", "-N", "-T"]
    assert "IdentityAgent=none" in args and "StrictHostKeyChecking=yes" in args and "ExitOnForwardFailure=yes" in args
    assert "PasswordAuthentication=no" in args and "UseKeychain=no" in args
    assert args.count("-L") == 4 and "-R" not in args and "ClearAllForwardings=yes" not in args
    assert value["StandardErrorPath"] == "/dev/null"
    assert args[-1] == "taira-edge-forwarder@192.168.64.3"


@pytest.mark.parametrize("change", [
    lambda p: p.update(provider="aws"),
    lambda p: p["mac"].update(source_address="127.0.0.1"),
    lambda p: p["guest"].update(address="169.254.1.1"),
    lambda p: p["guest"].update(user="root"),
    lambda p: p["host_pin"].update(fingerprint="SHA256:" + "a" * 43),
    lambda p: p["forwardings"][0].update(listen_address="0.0.0.0"),
    lambda p: p["forwardings"][0].update(target_address="192.168.64.3"),
    lambda p: p["forwardings"][1].update(listen_port=18480),
    lambda p: p["mac"].update(root="/operator/../foreign"),
    lambda p: p["mac"]["native"]["ssh"]["identity"].update(links="2"),
])
def test_unknown_provider_pin_alias_or_listener_is_refused(plan, change):
    change(plan)
    with pytest.raises(M.ForwardingError): M.validate_plan(plan)


def test_awk_private_main_reader_uses_retained_inode_and_bounds(tmp_path):
    # Fixture private bytes only. Production Python never reads this stream.
    main = tmp_path / "main"; main.write_bytes(b"Port 22\nAllowTcpForwarding no\n")
    fd = os.open(main, os.O_RDONLY | os.O_NOFOLLOW)
    replaced = tmp_path / "foreign"; replaced.write_text("PRIVATE FOREIGN INPUT\n"); os.replace(replaced, main)
    target = tmp_path / "validation"
    try:
        with target.open("wb") as out:
            result = subprocess.run([shutil.which("awk"), "-v", "fragment=/scoped/sshd-match.conf", M.SSHD_APPEND_GUARD],
                stdin=fd, stdout=out, stderr=subprocess.DEVNULL)
        assert result.returncode == 0 and target.read_bytes() == b"Port 22\nAllowTcpForwarding no\n\nMatch all\nInclude /scoped/sshd-match.conf\n"
    finally: os.close(fd)
    main.write_text("Include /scoped/sshd-match.conf\n")
    with main.open("rb") as src, target.open("wb") as out:
        result = subprocess.run([shutil.which("awk"), "-v", "fragment=/scoped/sshd-match.conf", M.SSHD_APPEND_GUARD], stdin=src, stdout=out)
    assert result.returncode == 41


def test_public_descriptor_cas_refuses_same_inode_content_mutation(tmp_path):
    root = M.open_directory(str(tmp_path.resolve()))
    fd, observed, digest = M.create_public(root, "candidate", b"selected public candidate")
    try:
        M.verify_public_fd(str(tmp_path / "candidate"), fd, observed, digest)
        os.pwrite(fd, b"foreign", 0)
        with pytest.raises(M.ForwardingError): M.verify_public_fd(str(tmp_path / "candidate"), fd, observed, digest)
    finally: os.close(fd); os.close(root)


def test_native_inputs_refuse_symlink_or_foreign_inode(tmp_path):
    regular = tmp_path / "native"; regular.write_text("fixture"); regular.chmod(0o600)
    observed = dict(path=str(regular), identity=M.identity(regular.stat()))
    fd = M.open_observed(observed)
    try:
        swap = tmp_path / "swap"; swap.write_text("fixture"); os.replace(swap, regular)
        with pytest.raises(M.ForwardingError): M.verify_path_fd(str(regular), fd, observed["identity"])
    finally: os.close(fd)
    regular.unlink(); regular.symlink_to(tmp_path / "missing")
    with pytest.raises(OSError): M.open_observed(observed)


def test_interrupted_operation_journal_refuses_plan_change_or_path_substitution(plan, tmp_path):
    directory = M.open_directory(str(tmp_path.resolve()))
    fd, append, _ = M.journal_writer(directory, plan, "macos")
    try:
        append("key_generating", private_key_path="/native-only-key")
        mutated = copy.deepcopy(plan); mutated["guest"]["address"] = "192.168.64.4"
        with pytest.raises(M.ForwardingError): M.journal_writer(directory, mutated, "macos", False)
        path = tmp_path / (".taira-validator-forwarding-" + plan["operation_id"] + ".receipt.ndjson")
        foreign = tmp_path / "foreign"; foreign.write_bytes(path.read_bytes()); foreign.chmod(0o600); os.replace(foreign, path)
        with pytest.raises(M.ForwardingError): append("identity_ready")
    finally: os.close(fd); os.close(directory)


def test_closed_receipt_never_admits_raw_native_error_or_fake_qualification(plan):
    core = dict(schema=M.RECEIPT_SCHEMA, operation_id=plan["operation_id"], host_kind="macos", phase="launch_refused",
                exit_code=1, qualified=False, private_bytes_read=False, error_code="native_launch_refused",
                plan_sha256=hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest())
    assert M.admit_receipt(core, plan, "mac-launch", 1) == core
    for change in (dict(error_code="private_secret_from_native_error"), dict(qualified=True),
                   dict(raw_stderr="PRIVATE"), dict(service="gui/501/foreign")):
        with pytest.raises(M.ForwardingError): M.admit_receipt(core | change, plan, "mac-launch", 1)


def test_remote_payload_contains_native_consumer_not_private_key_body(plan):
    code = M.remote_program(plan, "mac-identity")
    compile(code, "remote-public-code", "exec")
    assert b"BEGIN OPENSSH PRIVATE KEY" not in code
    assert b"signal.pidfd_send_signal" in code
    assert b"stdin=main_fd" in code and b"stdout=new_fd" in code
    assert b"-s reload" not in code


@pytest.mark.parametrize("mutation", ["none", "destination_inode", "backup_body"])
def test_private_main_publication_rollback_cas_retains_exact_original(tmp_path, mutation):
    main = tmp_path / "main"; main.write_bytes(b"fixture original native-only configuration\n"); main.chmod(0o600)
    fresh = tmp_path / "validation"; fresh.write_bytes(b"fixture original native-only configuration\nMatch User dedicated\n"); fresh.chmod(0o600)
    directory = M.open_directory(str(tmp_path.resolve()))
    old_fd = os.open(main, os.O_RDONLY | os.O_NOFOLLOW); new_fd = os.open(fresh, os.O_RDONLY | os.O_NOFOLLOW)
    old = M.identity(os.fstat(old_fd)); new = M.identity(os.fstat(new_fd))
    backup_fd, old_backed = M.backup_original(directory, "main", old_fd, old, "backup")
    try:
        M.replace_owned_main(directory, "main", old_fd, old_backed, "validation", new_fd, new)
        published, backup = M.identity(os.fstat(new_fd)), M.identity(os.fstat(backup_fd))
        if mutation == "destination_inode":
            other = tmp_path / "foreign"; other.write_bytes(main.read_bytes()); other.chmod(0o600); os.replace(other, main)
        elif mutation == "backup_body": (tmp_path / "backup").write_text("substituted private backup body\n")
        if mutation != "none":
            with pytest.raises(M.ForwardingError): M.restore_owned_main(directory, "main", new_fd, published, "backup", backup_fd, backup)
            assert main.stat().st_ino != int(old["inode"])
        else:
            M.restore_owned_main(directory, "main", new_fd, published, "backup", backup_fd, backup)
            assert main.stat().st_ino == int(old["inode"]) and main.read_bytes() == b"fixture original native-only configuration\n"
    finally:
        for fd in (directory, old_fd, new_fd, backup_fd): os.close(fd)


def test_native_serving_roster_binds_all_target_ports(plan, tmp_path):
    rows = [dict(slug="validator-" + str(i), torii_origin="https://public.example:" + str(8443+i),
        probe_origin="http://127.0.0.1:" + str(10080+i), account_id="public", peer_id="public-" + str(i)) for i in range(4)]
    file = tmp_path / "validator-clients.json"; raw = json.dumps(rows).encode(); file.write_bytes(raw); file.chmod(0o600)
    plan["serving_clients"] = dict(path=str(file), identity=M.identity(file.stat()), sha256=hashlib.sha256(raw).hexdigest())
    assert M.admit_serving_clients(plan) == rows
    plan["forwardings"][0]["target_port"] = 10084
    with pytest.raises(M.ForwardingError, match="serving_targets_differ"): M.admit_serving_clients(plan)


def test_stage_mutation_is_not_accepted_as_rename_baseline(tmp_path):
    path = tmp_path / "validation"; path.write_bytes(b"native configuration fixture\n")
    before = M.identity(path.stat())
    moved = tmp_path / "main"; os.replace(path, moved)
    M.renamed_identity(before, M.identity(moved.stat()))
    with moved.open("ab") as target: target.write(b"foreign in-place config mutation\n")
    with pytest.raises(M.ForwardingError, match="publication_identity_changed"): M.renamed_identity(before, M.identity(moved.stat()))
