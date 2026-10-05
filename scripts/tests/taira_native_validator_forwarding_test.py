"""Native forwarding restrictions, credential custody and interrupted journals."""
import base64
import copy
import fcntl
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
from types import SimpleNamespace

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
    assert fields["allowusers"] == "taira-edge-forwarder"
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
        with pytest.raises(M.ForwardingError, match="journal_operation_in_progress"): M.journal_writer(directory, mutated, "macos", False)
        fcntl.flock(fd, fcntl.LOCK_UN)
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


def test_exchange_ambiguity_receipt_requires_preserved_unqualified_no_signal(plan):
    receipt=dict(schema=M.RECEIPT_SCHEMA,operation_id=plan["operation_id"],host_kind="linux",phase="recovery_pending",
        exit_code=1,qualified=False,private_bytes_read=False,error_code="publication_recovery_pending",
        plan_sha256=hashlib.sha256(json.dumps(plan,sort_keys=True).encode()).hexdigest(),
        recovery_pending=True,reload_requested=False,fragment_published=False)
    assert M.admit_receipt(receipt,plan,"guest-reconcile-auth",1)==receipt
    for change in (dict(reload_requested=True),dict(recovery_pending=False),dict(error_code="native_exchange_failed")):
        with pytest.raises(M.ForwardingError):M.admit_receipt(receipt|change,plan,"guest-reconcile-auth",1)


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
        published, backup = M.replace_owned_main(directory, "main", old_fd, old_backed, "validation", new_fd, new)
        assert backup["links"] == "2"
        assert fresh.stat().st_ino == int(old["inode"]) == (tmp_path / "backup").stat().st_ino
        if mutation == "destination_inode":
            other = tmp_path / "foreign"; other.write_bytes(main.read_bytes()); other.chmod(0o600); os.replace(other, main)
        elif mutation == "backup_body": (tmp_path / "backup").write_text("substituted private backup body\n")
        if mutation != "none":
            with pytest.raises(M.ForwardingError): M.restore_owned_main(directory, "main", new_fd, published, "backup", backup_fd, backup)
            assert main.stat().st_ino != int(old["inode"])
        else:
            M.restore_owned_main(directory, "main", new_fd, published, "backup", backup_fd, backup)
            assert main.stat().st_ino == int(old["inode"]) and main.read_bytes() == b"fixture original native-only configuration\n"
            assert (tmp_path / "backup").stat().st_ino == int(new["inode"])
    finally:
        for fd in (directory, old_fd, new_fd, backup_fd): os.close(fd)


@pytest.mark.parametrize("phase,target", [("publish", "main"), ("publish", "validation"), ("rollback", "main"), ("rollback", "backup")])
def test_atomic_exchange_retains_substitution_at_publication_boundary(tmp_path, monkeypatch, phase, target):
    main = tmp_path / "main"; main.write_bytes(b"native-only original fixture\n"); main.chmod(0o600)
    stage = tmp_path / "validation"; stage.write_bytes(b"public candidate fixture\n"); stage.chmod(0o600)
    directory = M.open_directory(str(tmp_path.resolve()))
    old_fd = os.open(main, os.O_RDONLY | os.O_NOFOLLOW); new_fd = os.open(stage, os.O_RDONLY | os.O_NOFOLLOW)
    old, new = M.identity(os.fstat(old_fd)), M.identity(os.fstat(new_fd))
    backup_fd, backed = M.backup_original(directory, "main", old_fd, old, "backup")
    exchange = M.exchange_owned_paths
    try:
        if phase == "rollback":
            new, backed = M.replace_owned_main(directory, "main", old_fd, backed, "validation", new_fd, new)
        foreign = tmp_path / "foreign"; foreign.write_bytes(b"foreign retained fixture\n"); foreign.chmod(0o600)
        foreign_inode = foreign.stat().st_ino
        def substitute_then_exchange(directory, first, second):
            os.replace(foreign, tmp_path / target)
            exchange(directory, first, second)
        monkeypatch.setattr(M, "exchange_owned_paths", substitute_then_exchange)
        with pytest.raises(M.ForwardingError, match="publication_recovery_pending"):
            if phase == "publish": M.replace_owned_main(directory, "main", old_fd, backed, "validation", new_fd, new)
            else: M.restore_owned_main(directory, "main", new_fd, new, "backup", backup_fd, backed)
        names = ("main", "validation") if phase == "publish" else ("main", "backup")
        assert all((tmp_path / name).exists() for name in names)
        assert foreign_inode in {(tmp_path / name).stat().st_ino for name in names}
        assert os.fstat(old_fd).st_nlink > 0
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


def test_denied_channel_native_child_receipt_is_closed(plan, monkeypatch):
    calls = []
    def native(argv, local_probe_port=None):
        calls.append((argv, local_probe_port))
        categories = {"remote_forward_refused"} if "-R" in argv else {"administratively_prohibited"}
        return dict(categories=categories, native_exit=255 if local_probe_port is None else -15,
                    owned_probe_terminated=local_probe_port is not None)
    monkeypatch.setattr(M, "native_denial", native)
    assert M.verify_denied_channels(plan) == dict(session_refused=True, remote_forward_refused=True, unauthorized_target_refused=True)
    assert len(calls) == 3 and calls[0][0][-1] == "/bin/true" and "-N" not in calls[0][0]
    assert calls[1][0][calls[1][0].index("-R")+1].startswith("127.0.0.1:0:")
    assert calls[2][1] is not None and calls[2][0][calls[2][0].index("-L")+1].endswith(":127.0.0.1:1")
    monkeypatch.setattr(M, "native_denial", lambda *a: dict(categories={"authentication_refused"},native_exit=255,owned_probe_terminated=False))
    with pytest.raises(M.ForwardingError): M.verify_denied_channels(plan)


def test_denial_stderr_native_guard_only_emits_closed_categories():
    result = subprocess.run([shutil.which("awk"), M.DENIAL_GUARD],
        input=b"secret error body must disappear\nchannel 0: open failed: administratively prohibited: open failed\nError: remote port forwarding failed for listen port 0\n",
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    assert result.returncode == 0 and set(result.stdout.decode().splitlines()) == {"administratively_prohibited", "remote_forward_refused"}


def test_native_verification_receipt_binds_exact_ports_no_api_body(plan):
    result = dict(schema=M.RECEIPT_SCHEMA, operation_id=plan["operation_id"], host_kind="macos", phase="forwarding_ready_unqualified",
        exit_code=0, qualified=False, private_bytes_read=False,
        plan_sha256=hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest(),
        service_metadata=dict(pid=10001, uid=501, started="Sun Oct 4 12:34:56 2026", executable="/usr/bin/ssh"),
        requests=[dict(listen_port=18480+i,target_port=10080+i,http_status=200,bytes=24808) for i in range(4)],
        denied_channels=dict(session_refused=True,remote_forward_refused=True,unauthorized_target_refused=True))
    assert M.admit_receipt(result,plan,"mac-verify",0) == result
    changed=copy.deepcopy(result); changed["requests"][0]["body"]="arbitrary API body"
    with pytest.raises(M.ForwardingError): M.admit_receipt(changed,plan,"mac-verify",0)
    changed=copy.deepcopy(result); changed["requests"][0]["listen_port"]=8443
    with pytest.raises(M.ForwardingError): M.admit_receipt(changed,plan,"mac-verify",0)


def test_success_hup_journal_append_keeps_one_unqualified_field(plan, tmp_path):
    directory = M.open_directory(str(tmp_path.resolve()))
    fd, append, _ = M.journal_writer(directory, plan, "linux")
    try:
        record = append("authorization_ready", reload_signal_mode="pidfd", publication_identity=plan["guest"]["main"]["identity"])
        assert record["qualified"] is False and record["phase"] == "authorization_ready"
        assert json.loads(os.pread(fd, os.fstat(fd).st_size, 0))["qualified"] is False
    finally: os.close(fd); os.close(directory)


def test_server_banner_and_zero_exit_cannot_prove_denied_channel(plan, monkeypatch):
    result = subprocess.run([shutil.which("awk"), M.DENIAL_GUARD],
        input=b"Welcome: administratively prohibited; remote port forwarding failed\n", stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    assert result.returncode == 0 and result.stdout == b""
    monkeypatch.setattr(M,"native_denial",lambda *a:dict(categories={"administratively_prohibited"},native_exit=0,owned_probe_terminated=False))
    with pytest.raises(M.ForwardingError,match="session_restriction_not_verified"): M.verify_denied_channels(plan)


def test_metadata_timeout_reaps_only_owned_native_producer(monkeypatch):
    real = M.subprocess.Popen
    children = []
    def capture(*args, **kwargs):
        child = real(*args, **kwargs); children.append(child); return child
    monkeypatch.setattr(M.subprocess, "Popen", capture)
    with pytest.raises(subprocess.TimeoutExpired):
        M.native_projection([sys.executable, "-c", "import time; time.sleep(30)"],
            [shutil.which("awk"), "{print}"], timeout=0.05)
    assert children and all(child.poll() is not None for child in children)


@pytest.mark.parametrize("matches",[0,1,2])
def test_reconcile_validation_native_main_repoints_exactly_one_owned_include(tmp_path,matches):
    main=tmp_path/"main";main.write_text("AllowUsers root\n"+"Include /root/owned/sshd-match.conf\n"*matches)
    validation=tmp_path/"validation"
    with main.open("rb") as source,validation.open("wb") as output:
        result=subprocess.run([shutil.which("awk"),"-v","original=/root/owned/sshd-match.conf","-v","candidate=/root/owned/.candidate.conf",M.SSHD_REPOINT_GUARD],stdin=source,stdout=output,stderr=subprocess.DEVNULL)
    assert result.returncode == (0 if matches == 1 else 41)
    if matches==1:assert validation.read_text()=="AllowUsers root\nInclude /root/owned/.candidate.conf\n"


def test_reconcile_sources_require_one_completed_immutable_native_authorization(plan,tmp_path):
    directory=M.open_directory(str(tmp_path.resolve()));fd,append,_=M.journal_writer(directory,plan,"linux")
    try:
        append("public_authorization_created",authorized_key_identity=plan["guest"]["main"]["identity"],authorized_key_sha256="d"*64,
            fragment_identity=plan["guest"]["main"]["identity"],fragment_sha256="e"*64)
        with pytest.raises(M.ForwardingError):M.authorization_binding(fd,plan)
        append("authorization_ready",publication_identity=plan["guest"]["main"]["identity"])
        original,current=M.authorization_binding(fd,plan)
        assert original["fragment_sha256"]=="e"*64 and current==plan["guest"]["main"]["identity"]
        append("fragment_publishing")
        with pytest.raises(M.ForwardingError):M.authorization_binding(fd,plan)
    finally:os.close(fd);os.close(directory)


def test_shared_guest_owner_lock_refuses_overlap_and_path_substitution(tmp_path):
    directory=M.open_directory(str(tmp_path.resolve()))
    fd,snapshot=M.open_owner_lock(directory,os.geteuid())
    try:
        with pytest.raises(M.ForwardingError,match="guest_operation_in_progress"):M.open_owner_lock(directory,os.geteuid())
        foreign=tmp_path/"foreign";foreign.write_bytes(b"");foreign.chmod(0o600)
        os.replace(foreign,tmp_path/".taira-validator-forwarding.lock")
        with pytest.raises(M.ForwardingError,match="guest_lock_identity_changed"):M.verify_owner_lock(directory,fd,snapshot)
    finally:os.close(fd);os.close(directory)


@pytest.mark.parametrize("service_fields,expected",[("\tstate = running\n\tpid = 59064\n",0),
    ("\tstate = running\n\tpid = 59064\n\tstate = running\n",41),
    ("\tstate = spawn scheduled\n\tpid = 59064\n",41),("",41)])
def test_launchctl_projects_only_top_level_service_fields(service_fields,expected):
    native_fixture="gui/501/org.sora.taira.validator-forwarding = {\n"+service_fields+"\tresource group = {\n\t\tstate = active\n\t\tpid = 999\n\t}\n}\n"
    result=subprocess.run([shutil.which("awk"),M.LAUNCHCTL_SERVICE_GUARD],input=native_fixture.encode(),stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
    assert result.returncode==expected
    if expected==0:assert result.stdout==b"state running\npid 59064\n"


def test_native_openssh_crlf_session_refusal_does_not_weaken_target_proof():
    diagnostic=b"channel 0: open failed: connect failed: open failed\r\n"
    result=subprocess.run([shutil.which("awk"),M.DENIAL_GUARD],input=diagnostic,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
    assert result.returncode==0 and result.stdout==b"session_channel_refused\n"
    assert b"administratively_prohibited" not in result.stdout
    banner=subprocess.run([shutil.which("awk"),M.DENIAL_GUARD],input=b"banner says "+diagnostic,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
    assert banner.returncode==0 and banner.stdout==b""


def test_readiness_rechecks_require_same_native_launch_lineage(plan,tmp_path):
    for phase in ("awaiting_forwarding_readiness","forwarding_ready_unqualified","verification_requested","verification_refused"):
        M.admit_readiness_phase(dict(phase=phase))
    for phase in ("identity_ready","launch_requested","authentication_eligibility_ready"):
        with pytest.raises(M.ForwardingError):M.admit_readiness_phase(dict(phase=phase))
    directory=M.open_directory(str(tmp_path.resolve()));fd,append,_=M.journal_writer(directory,plan,"macos")
    try:
        source=dict(service=plan["mac"]["domain"]+"/"+plan["mac"]["label"],
            plist_identity=plan["guest"]["main"]["identity"]|dict(uid="501",mode="384"),
            plist_sha256=hashlib.sha256(M.render_launch_agent(plan)).hexdigest())
        append("awaiting_forwarding_readiness",**source)
        for _ in range(20):append("verification_requested");append("forwarding_ready_unqualified")
        assert M.launch_binding(fd,plan)["plist_sha256"]==source["plist_sha256"]
        append("awaiting_forwarding_readiness",**source)
        with pytest.raises(M.ForwardingError,match="launch_journal_binding"):M.launch_binding(fd,plan)
    finally:os.close(fd);os.close(directory)


@pytest.fixture
def incumbent_forwarder(plan, tmp_path, monkeypatch):
    """Retain real public/key metadata while stubbing only service/network effects."""
    root = tmp_path.resolve()
    native_root = root / "native-forwarding"
    native_root.mkdir(mode=0o700)
    agents = root / "LaunchAgents"
    agents.mkdir(mode=0o700)
    uid = os.geteuid()
    def directory(path):
        actual = M.identity(path.stat())
        return dict(path=str(path), identity={key: actual[key] for key in ("device", "inode", "uid", "gid", "mode")})
    plan["mac"].update(uid=uid, domain="gui/" + str(uid), root=str(native_root),
        root_parent=directory(root), launch_agents=directory(agents))
    # The four programs remain the real root-owned native executables; no
    # service command is executed by this fixture.
    for key, path in dict(ssh="/usr/bin/ssh", ssh_keygen="/usr/bin/ssh-keygen",
                          launchctl="/bin/launchctl", plutil="/usr/bin/plutil").items():
        native = Path(path)
        if not native.exists():
            pytest.skip("native forwarding inspection fixture requires macOS tools")
        plan["mac"]["native"][key] = dict(path=str(native), identity=M.identity(native.stat()))
    private = native_root / "id_ed25519"
    private.write_bytes(b"opaque fixture native key; Python must never read this")
    private.chmod(0o600)
    known = native_root / "known_hosts"
    known.write_bytes((plan["host_pin"]["record"] + "\n").encode())
    known.chmod(0o600)
    plist = agents / (plan["mac"]["label"] + ".plist")
    plist.write_bytes(M.render_launch_agent(plan))
    plist.chmod(0o600)
    root_identity = directory(native_root)["identity"]
    receipt = dict(schema=M.RECEIPT_SCHEMA, phase="identity_ready", exit_code=0, qualified=False,
        private_bytes_read=False, operation_id=plan["operation_id"], host_kind="macos",
        plan_sha256=hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest(),
        private_key_identity=M.identity(private.stat()), public_key=KEY,
        public_key_fingerprint="SHA256:" + base64.b64encode(hashlib.sha256(KEY_BLOB).digest()).decode().rstrip("="),
        known_hosts_identity=M.identity(known.stat()), known_hosts_sha256=hashlib.sha256(known.read_bytes()).hexdigest(),
        root_identity=root_identity, journal_path=str(root / (".taira-validator-forwarding-" + plan["operation_id"] + ".receipt.ndjson")))
    parent = M.open_directory(str(root))
    fd, append, _ = M.journal_writer(parent, plan, "macos")
    append("identity_ready")
    append("awaiting_forwarding_readiness", service=plan["mac"]["domain"] + "/" + plan["mac"]["label"],
        plist_identity=M.identity(plist.stat()), plist_sha256=hashlib.sha256(plist.read_bytes()).hexdigest())
    append("forwarding_ready_unqualified")
    os.close(fd); os.close(parent)
    service = dict(pid=10001, uid=uid, started="Sun Oct 4 12:34:56 2026", executable="/usr/bin/ssh")
    calls = []
    class Connection:
        def __init__(self, host, port, timeout):
            assert host == "127.0.0.1" and timeout == 15
            self.port = port
        def request(self, method, path, headers):
            assert method == "GET" and path == "/status"
            assert headers == {"Host":"taira.sora.org", "Accept":"application/json"}
            calls.append(self.port)
        def getresponse(self):
            return SimpleNamespace(status=200, read=lambda limit: b'{"public":"status"}')
        def close(self):
            pass
    monkeypatch.setattr(M.http.client, "HTTPConnection", Connection)
    monkeypatch.setattr(M, "native_service_metadata", lambda selected: dict(service))
    monkeypatch.setattr(M, "verify_denied_channels", lambda selected: dict(session_refused=True,
        remote_forward_refused=True, unauthorized_target_refused=True))
    native_pread = os.pread
    key_identity = private.stat()
    def public_only(fd, count, offset):
        info = os.fstat(fd)
        assert (info.st_dev, info.st_ino) != (key_identity.st_dev, key_identity.st_ino), "Python read a private key"
        return native_pread(fd, count, offset)
    monkeypatch.setattr(M.os, "pread", public_only)
    yield dict(plan=plan, identity=receipt, root=root, private=private, plist=plist, calls=calls, service=service)


def test_incumbent_inspection_preserves_exact_signed_journal_and_returns_closed_numeric_metadata(incumbent_forwarder):
    fixture = incumbent_forwarder
    path = Path(fixture["identity"]["journal_path"])
    before, observed = path.read_bytes(), M.identity(path.stat())
    result = M.inspect_mac_forwarding(fixture["plan"], fixture["identity"])
    assert set(result) == {"schema", "operation_id", "plan_sha256", "qualified", "private_bytes_read",
        "service_metadata", "requests", "denied_channels", "journal"}
    assert result["schema"] == M.INSPECTION_SCHEMA
    assert result["qualified"] is result["private_bytes_read"] is False
    assert result["journal"] == dict(file=dict(path=str(path), identity={key:int(value) for key,value in observed.items()}),
        sha256=hashlib.sha256(before).hexdigest())
    assert result["journal"]["file"]["identity"]["mtime_ns"] > 2**53
    assert fixture["calls"] == [18480, 18481, 18482, 18483]
    assert path.read_bytes() == before and M.identity(path.stat()) == observed
    assert "public" not in json.dumps(result["requests"])


@pytest.mark.parametrize("fault", ["service_changed", "private_mode", "plist_substitution", "journal_append", "denial_failed"])
def test_incumbent_inspection_refuses_owner_or_readiness_drift_without_appending(incumbent_forwarder, monkeypatch, fault):
    fixture = incumbent_forwarder
    journal = Path(fixture["identity"]["journal_path"])
    before = journal.read_bytes()
    calls = 0
    def changing_service(selected):
        nonlocal calls
        calls += 1
        if calls == 2:
            if fault == "service_changed":
                return fixture["service"] | dict(pid=10002)
            if fault == "private_mode":
                fixture["private"].chmod(0o644)
            if fault == "plist_substitution":
                foreign = fixture["root"] / "foreign.plist"
                foreign.write_bytes(fixture["plist"].read_bytes()); foreign.chmod(0o600)
                os.replace(foreign, fixture["plist"])
            if fault == "journal_append":
                with journal.open("ab") as target: target.write(b'{"foreign":"public appendix"}\n')
        return dict(fixture["service"])
    monkeypatch.setattr(M, "native_service_metadata", changing_service)
    if fault == "denial_failed":
        def deny(selected):
            raise M.ForwardingError("session_restriction_not_verified")
        monkeypatch.setattr(M, "verify_denied_channels", deny)
    with pytest.raises(M.ForwardingError):
        M.inspect_mac_forwarding(fixture["plan"], fixture["identity"])
    assert journal.read_bytes() == (before + b'{"foreign":"public appendix"}\n' if fault == "journal_append" else before)


def test_incumbent_inspection_refuses_active_journal_owner_before_service_or_api_probe(incumbent_forwarder):
    fixture = incumbent_forwarder
    parent = M.open_directory(fixture["plan"]["mac"]["root_parent"]["path"])
    fd, _, _ = M.journal_writer(parent, fixture["plan"], "macos", False)
    try:
        with pytest.raises(M.ForwardingError, match="journal_operation_in_progress"):
            M.inspect_mac_forwarding(fixture["plan"], fixture["identity"])
        assert fixture["calls"] == []
    finally:
        os.close(fd); os.close(parent)


def test_canonical_mac_verification_still_records_progress_after_shared_readiness(incumbent_forwarder):
    fixture = incumbent_forwarder
    result = M.mac_verify(fixture["plan"], fixture["identity"])
    assert result["exit_code"] == 0 and result["phase"] == "forwarding_ready_unqualified", result
    rows = [json.loads(line) for line in Path(fixture["identity"]["journal_path"]).read_text().splitlines()]
    assert [row["phase"] for row in rows][-2:] == ["verification_requested", "forwarding_ready_unqualified"]


@pytest.mark.parametrize("rollback_allowed",[True,False])
def test_reconciliation_failure_after_hup_reports_requested_reload_and_owned_publication(plan,tmp_path,monkeypatch,rollback_allowed):
    """Exercise the real exchange/journal/rollback phase with native child stubs."""
    import taira_native_nginx_apply as owner
    root=tmp_path.resolve();authorization=root/"authorization";authorization.mkdir(mode=0o755)
    plan["guest"]["directory"]["path"]=str(root);plan["guest"]["authorization_root"]=str(authorization)
    plan["guest"]["main"]["path"]=str(root/"sshd_config")
    main=root/"sshd_config";main.write_text("AllowUsers root\nInclude "+str(authorization/"sshd-match.conf")+"\n");main.chmod(0o600)
    fragment=authorization/"sshd-match.conf";fragment.write_bytes(M.render_sshd_match(plan).replace(b"    allowusers taira-edge-forwarder\n",b""));fragment.chmod(0o600)
    key=authorization/"authorized_keys";key.write_bytes(M.render_authorized_key(plan,KEY));key.chmod(0o644)
    original=dict(authorized_key_identity=M.identity(key.stat()),authorized_key_sha256=hashlib.sha256(key.read_bytes()).hexdigest(),
        fragment_identity=M.identity(fragment.stat()),fragment_sha256=hashlib.sha256(fragment.read_bytes()).hexdigest())
    plan["guest"]["main"]["identity"]=M.identity(main.stat())
    directory=M.open_directory(str(root));fd,append,_=M.journal_writer(directory,plan,"linux")
    append("public_authorization_created",**original);append("authorization_ready",publication_identity=plan["guest"]["main"]["identity"])
    os.close(fd);os.close(directory)
    native_paths={row["path"] for row in plan["guest"]["native"].values()}
    actual_open,actual_verify,actual_public,actual_lock,actual_run=M.open_observed,M.verify_path_fd,M.verify_public_fd,M.open_owner_lock,subprocess.run
    actual_need=M.need
    monkeypatch.setattr(M,"sys",SimpleNamespace(platform="linux"))
    monkeypatch.setattr(M,"validate_plan",lambda value:value)
    monkeypatch.setattr(M,"admit_identity_receipt",lambda value,plan:value)
    monkeypatch.setattr(M,"need",lambda condition,code:None if code in {"guest_root_required","authorization_directory_custody"} else actual_need(condition,code))
    monkeypatch.setattr(M,"open_observed",lambda reference,directory=False:os.open("/dev/null",os.O_RDONLY) if reference["path"] in native_paths else (M.open_directory(reference["path"]) if directory else actual_open(reference)))
    monkeypatch.setattr(M,"verify_path_fd",lambda path,fd,snapshot:None if path in native_paths else actual_verify(path,fd,snapshot))
    monkeypatch.setattr(M,"open_owner_lock",lambda directory:actual_lock(directory,os.geteuid()))
    monkeypatch.setattr(M,"admit_serving_clients",lambda plan:None)
    monkeypatch.setattr(M,"native_effective",lambda plan,main,user,expected=None:expected if expected is not None else {})
    monkeypatch.setattr(subprocess,"run",lambda argv,**kwargs:subprocess.CompletedProcess(argv,0) if argv[0] in native_paths else actual_run(argv,**kwargs))
    monkeypatch.setattr(owner,"observe_master",lambda expected:expected)
    monkeypatch.setattr(owner,"open_master_handle",lambda expected:os.open("/dev/null",os.O_RDONLY))
    signals=[];failure=[False]
    def hup(expected,handle):
        signals.append(expected["pid"])
        if len(signals)==1 and not rollback_allowed:
            (authorization/(".sshd-match-original-"+plan["operation_id"]+".conf")).write_bytes(b"foreign backup mutation\n")
        return "pidfd"
    def public(path,fd,snapshot,digest):
        if signals and not failure[0]:failure[0]=True;raise M.ForwardingError("public_content_changed")
        return actual_public(path,fd,snapshot,digest)
    monkeypatch.setattr(owner,"signal_master",hup);monkeypatch.setattr(M,"verify_public_fd",public)
    receipt=M.guest_reconcile_auth(plan,dict(public_key=KEY,public_key_fingerprint="public fixture"))
    assert receipt["reload_requested"] is True and receipt["exit_code"]==1 and failure[0]
    assert receipt["fragment_published"] is (not rollback_allowed)
    assert receipt["phase"]==("reconcile_rolled_back_unqualified" if rollback_allowed else "rollback_ambiguous")
    assert len(signals)==(2 if rollback_allowed else 1)
