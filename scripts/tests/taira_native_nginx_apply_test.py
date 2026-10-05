"""Owned native nginx publication, replacement, reconciliation and rollback."""
from __future__ import annotations

import base64
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import sys
import tempfile

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "taira_native_nginx_apply.py"
sys.path.insert(0, str(MODULE_PATH.parent))
SPEC = importlib.util.spec_from_file_location("taira_native_nginx_apply", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
SIGNAL_MASTER = MODULE.signal_master
OPEN_MASTER_HANDLE = MODULE.open_master_handle
OBSERVE_MASTER = MODULE.observe_master


def _identity(path: Path, directory=False):
    s = path.lstat()
    fields = dict(device=s.st_dev, inode=s.st_ino, uid=s.st_uid, gid=s.st_gid, mode=stat.S_IMODE(s.st_mode))
    if not directory:
        fields.update(links=s.st_nlink, size=s.st_size, mtime_ns=s.st_mtime_ns, ctime_ns=s.st_ctime_ns)
    return {key: str(value) for key, value in fields.items()}


@pytest.fixture
def native_apply(monkeypatch):
    with tempfile.TemporaryDirectory(prefix=".nginx-apply-test-", dir=MODULE_PATH.parent) as temporary:
        root = Path(temporary).resolve()
        active = root / "conf.d"
        active.mkdir(mode=0o700)
        main = root / "nginx.conf"
        main.write_text("events {}\nhttp { include conf.d/*.conf; }\n")
        nginx = root / "nginx-native"
        nginx.write_text('''#!/usr/bin/python3
import os, pathlib, sys
config = pathlib.Path(sys.argv[sys.argv.index("-c")+1])
root = config.parent
candidate = root / "conf.d" / "new-scoped.conf"
print("private-native-error", file=sys.stderr)
if "-T" in sys.argv:
    print("# configuration file " + str(config) + ":")
    print("private-existing-config-body")
    if candidate.exists() and not (root / "omit-source").exists():
        print("# configuration file " + str(candidate) + ":")
        print(candidate.read_text())
    if (root / "mutate-published").exists(): candidate.write_text("foreign replacement body")
    if (root / "substitute-published").exists():
        replacement=root/"foreign"; replacement.write_text("foreign replacement body"); os.replace(replacement,candidate)
    if (root / "reject-context").exists(): sys.exit(7)
    (root / "context-completed").touch()
assert "-s" not in sys.argv, "PID-file-derived reload is forbidden"
''')
        nginx.chmod(0o700)
        awk = Path(shutil.which("awk")).resolve()
        candidate = b"upstream selected { server 127.0.0.1:10080; }\n"
        master = dict(pid=1001, uid=os.geteuid(), started="Sun Oct 4 01:02:03 2026", executable=str(nginx))
        if sys.platform != "darwin":
            master.update(start_ticks="123456", boot_id="00000000-0000-0000-0000-000000000000")
        request = dict(host_kind="macos" if sys.platform == "darwin" else "linux", operation_id="a" * 32,
            publication=dict(kind="create"),
            master=master, candidate_base64=base64.b64encode(candidate).decode(),
            candidate_sha256=hashlib.sha256(candidate).hexdigest(), renderer_source_sha256="b" * 64,
            native=dict(owner_uid=os.geteuid(), trusted_group_gids=[],
                directory=dict(path=str(root), identity=_identity(root, True)),
                main=dict(path=str(main), identity=_identity(main)),
                nginx=dict(path=str(nginx), identity=_identity(nginx)),
                awk=dict(path=str(awk), identity=_identity(awk))),
            destination=dict(directory=dict(path=str(active), identity=_identity(active, True)), basename="new-scoped.conf"))
        monkeypatch.setattr(MODULE, "observe_master", lambda expected: expected)
        monkeypatch.setattr(MODULE, "open_master_handle", lambda expected: None)
        def reload_fixture(expected, retained):
            count = root / "reload-count"
            number = int(count.read_text()) + 1 if count.exists() else 1
            count.write_text(str(number))
            if (root / "reject-all-reload").exists() or ((root / "reject-reload").exists() and number == 1):
                raise RuntimeError("reload_signal_ambiguous")
            return "adjacent_native_identity"
        monkeypatch.setattr(MODULE, "signal_master", reload_fixture)
        yield request, root, candidate


def _journal(root, operation="a" * 32):
    path = root / (".taira-native-nginx-apply-" + operation + ".receipt.ndjson")
    assert stat.S_IMODE(path.stat().st_mode) == 0o600
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    assert [row["sequence"] for row in rows] == list(range(1, len(rows)+1))
    assert all(row["qualified"] is False for row in rows)
    return rows


def _configuration_exchange(callback):
    """Interrupt config publication while permitting atomic owner journals."""
    exchange = MODULE.native_exchange
    def guarded(directory, first, second):
        if first.endswith('.receipt.ndjson') or second.endswith('.receipt.ndjson'):
            return exchange(directory, first, second)
        return callback(directory, first, second)
    return guarded


def _clean(root):
    assert not list(root.glob(".taira-nginx-check-*"))
    assert not list((root / "conf.d").glob(".taira-nginx-publish-*"))


def test_publication_is_complete_included_once_durable_and_unqualified(native_apply):
    request, root, candidate = native_apply
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 0, result
    assert result["configuration_published"] and result["reload_requested"]
    assert result["qualified"] is False and result["phase"] == "awaiting_readiness"
    assert result["active_context_nginx_exit_code"] == result["active_source_guard_exit_code"] == 0
    assert (root / "conf.d/new-scoped.conf").read_bytes() == candidate
    assert stat.S_IMODE((root / "conf.d/new-scoped.conf").stat().st_mode) == 0o600
    assert "private-existing" not in json.dumps(result) and "private-native" not in json.dumps(result)
    phases = [row["phase"] for row in _journal(root)]
    assert phases == ["prepared", "publishing", "published", "context_checking", "context_checked",
                      "reload_requested", "awaiting_readiness"]
    _clean(root)


@pytest.mark.parametrize("flag", ["reject-context", "omit-source", "reject-reload"])
def test_failed_context_source_proof_or_reload_rolls_back_only_new_file(native_apply, flag):
    request, root, _ = native_apply
    (root / flag).touch()
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 1 and result["qualified"] is False
    assert not (root / "conf.d/new-scoped.conf").exists()
    assert result["configuration_published"] is False
    assert result["rollback_context_exit_code"] == 0
    assert result["rollback_reload_signal_mode"] == "adjacent_native_identity"
    assert _journal(root)[-1]["phase"] == "rolled_back_unqualified"
    _clean(root)


@pytest.mark.parametrize("flag", ["mutate-published", "substitute-published"])
def test_substituted_published_inode_or_content_is_preserved_and_reported_ambiguous(native_apply, flag):
    request, root, _ = native_apply
    (root / flag).touch()
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 1 and result["rollback_ambiguous"] is True
    assert (root / "conf.d/new-scoped.conf").read_text() == "foreign replacement body"
    assert not (root / "reload-count").exists()
    assert _journal(root)[-1]["phase"] == "rollback_ambiguous"
    _clean(root)


def test_failed_original_reload_remains_inspectable_and_unqualified(native_apply):
    request, root, _ = native_apply
    (root / "reject-all-reload").touch()
    result = MODULE.remote_apply(request)
    assert result["rollback_ambiguous"] and result["qualified"] is False
    assert not (root / "conf.d/new-scoped.conf").exists()
    assert "rollback_reload_signal_mode" not in result
    assert _journal(root)[-1]["phase"] == "rollback_ambiguous"
    _clean(root)


def test_create_only_refuses_existing_destination_and_existing_operation_journal(native_apply):
    request, root, _ = native_apply
    existing = root / "conf.d/new-scoped.conf"
    existing.write_text("existing privately owned configuration")
    before = existing.stat().st_ino
    result = MODULE.remote_apply(request)
    assert result["error_code"] == "destination_already_exists"
    assert existing.stat().st_ino == before
    assert existing.read_text() == "existing privately owned configuration"
    existing.unlink()
    journal = root / (".taira-native-nginx-apply-" + "a" * 32 + ".receipt.ndjson")
    journal.write_text("existing exact owner journal")
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 1 and not existing.exists()
    assert journal.read_text() == "existing exact owner journal"
    _clean(root)


def test_changed_master_is_refused_before_publication(native_apply, monkeypatch):
    request, root, _ = native_apply
    monkeypatch.setattr(MODULE, "observe_master", lambda expected: {**expected, "started": "different"})
    result = MODULE.remote_apply(request)
    assert result["error_code"] == "master_identity_changed"
    assert not (root / "conf.d/new-scoped.conf").exists()
    assert not (root / "reload-count").exists()
    _clean(root)


def test_closed_apply_plan_refuses_foreign_directory_overwrite_name_and_unbound_master(native_apply):
    request, root, _ = native_apply
    reference = dict(path=str(root / "public.json"), sha256="c" * 64)
    plan = dict(schema=MODULE.PLAN_SCHEMA, provider="macstadium-dublin", host_kind=request["host_kind"],
        deployment_reference=reference, native=request["native"], renderer_source=reference,
        candidate=dict(path=str(root / "public.conf"), sha256=request["candidate_sha256"], owner_uid=os.geteuid()),
        operation_id=request["operation_id"], destination=request["destination"], master=request["master"],
        publication=request["publication"])
    assert MODULE.validate_plan(plan) == plan
    for target, value in [("basename", "../nginx.conf"), ("basename", "nginx.conf;reload"), ("basename", "nginx.conf/other")]:
        changed = copy.deepcopy(plan)
        changed["destination"][target] = value
        with pytest.raises(MODULE.checked.CheckError): MODULE.validate_plan(changed)
    changed = copy.deepcopy(plan)
    changed["master"]["executable"] = "/foreign/nginx"
    with pytest.raises(MODULE.checked.CheckError): MODULE.validate_plan(changed)


def test_linux_reload_uses_retained_process_handle_and_never_pidfile_or_kill(monkeypatch):
    expected = dict(pid=1001, uid=501, started="Sun Oct 4 01:02:03 2026", executable="/native/nginx")
    monkeypatch.setattr(MODULE.sys, "platform", "linux")
    monkeypatch.setattr(MODULE, "observe_master", lambda value: value)
    signals = []
    monkeypatch.setattr(MODULE.signal, "pidfd_send_signal", lambda *args: signals.append(args), raising=False)
    monkeypatch.setattr(MODULE.os, "kill", lambda *args: pytest.fail("numeric Linux PID signaling is forbidden"))
    assert SIGNAL_MASTER(expected, 41) == "pidfd"
    assert signals == [(41, signal.SIGHUP, None, 0)]
    with pytest.raises(RuntimeError, match="reload_signal_ambiguous"):
        SIGNAL_MASTER(expected, None)
    assert len(signals) == 1


def test_linux_missing_pidfd_refuses_before_publication(native_apply, monkeypatch):
    request, root, _ = native_apply
    monkeypatch.setattr(MODULE.sys, "platform", "linux")
    monkeypatch.delattr(MODULE.os, "pidfd_open", raising=False)
    monkeypatch.setattr(MODULE, "open_master_handle", OPEN_MASTER_HANDLE)
    with pytest.raises(RuntimeError, match="master_handle_unavailable"):
        OPEN_MASTER_HANDLE(request["master"])


def test_mac_reload_adjacent_revalidation_refuses_changed_or_group_target(monkeypatch):
    expected = dict(pid=1001, uid=501, started="Sun Oct 4 01:02:03 2026", executable="/native/nginx")
    monkeypatch.setattr(MODULE.sys, "platform", "darwin")
    kills = []
    monkeypatch.setattr(MODULE.os, "kill", lambda *args: kills.append(args))
    monkeypatch.setattr(MODULE, "observe_master", lambda value: value)
    assert SIGNAL_MASTER(expected, None) == "adjacent_native_identity"
    assert kills == [(1001, signal.SIGHUP)]
    monkeypatch.setattr(MODULE, "observe_master", lambda value: {**value, "started": "changed"})
    with pytest.raises(RuntimeError, match="master_identity_changed"):
        SIGNAL_MASTER(expected, None)
    for target in (0, 1, -1001, True, 2147483648):
        with pytest.raises(RuntimeError, match="master_signal_target"):
            SIGNAL_MASTER({**expected, "pid": target}, None)
    assert len(kills) == 1


def test_master_change_after_hup_remains_ambiguous(monkeypatch):
    expected = dict(pid=1001, uid=501, started="Sun Oct 4 01:02:03 2026", executable="/native/nginx")
    monkeypatch.setattr(MODULE.sys, "platform", "darwin")
    observations = iter([expected, {**expected, "started": "changed"}])
    monkeypatch.setattr(MODULE, "observe_master", lambda value: next(observations))
    kills = []
    monkeypatch.setattr(MODULE.os, "kill", lambda *args: kills.append(args))
    with pytest.raises(RuntimeError, match="reload_signal_ambiguous"):
        SIGNAL_MASTER(expected, None)
    assert kills == [(1001, signal.SIGHUP)]


@pytest.mark.parametrize("after_link", [False, True])
def test_interrupted_link_leaves_durable_exact_candidate_identity_and_refuses_new_operation(native_apply, monkeypatch, after_link):
    request, root, candidate = native_apply
    real_link = os.link
    def interrupted_link(*args, **kwargs):
        if after_link:
            real_link(*args, **kwargs)
        raise KeyboardInterrupt("owned test crash window")
    monkeypatch.setattr(MODULE.os, "link", interrupted_link)
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    last = _journal(root)[-1]
    assert last["phase"] == "publishing" and last["candidate_sha256"] == request["candidate_sha256"]
    assert last["publication_identity"]["inode"] and last["temporary_basename"]
    destination = root / "conf.d/new-scoped.conf"
    assert destination.exists() is after_link
    if after_link:
        assert destination.read_bytes() == candidate
        assert str(destination.stat().st_ino) == last["publication_identity"]["inode"]
    _clean(root)
    monkeypatch.setattr(MODULE.os, "link", real_link)
    retry = {**request, "operation_id": "b" * 32}
    result = MODULE.remote_apply(retry)
    assert result["error_code"] == ("destination_already_exists" if after_link else "unresolved_destination_journal")
    assert not (root / "reload-count").exists()


def test_interrupted_after_signal_keeps_reload_requested_journal_and_never_claims_qualification(native_apply, monkeypatch):
    request, root, candidate = native_apply
    def interrupted_signal(expected, retained):
        (root / "signal-requested").touch()
        raise KeyboardInterrupt("owned test crash window")
    monkeypatch.setattr(MODULE, "signal_master", interrupted_signal)
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    record = _journal(root)[-1]
    assert record["phase"] == "reload_requested" and record["qualified"] is False
    assert record["master"] == request["master"]
    assert (root / "signal-requested").exists()
    assert (root / "conf.d/new-scoped.conf").read_bytes() == candidate
    _clean(root)


def test_closed_apply_receipt_rejects_private_text_changed_identities_or_false_success(native_apply):
    request, root, _ = native_apply
    receipt = MODULE.remote_apply(request)
    plan = dict(host_kind=request["host_kind"], native=request["native"], destination=request["destination"],
                operation_id=request["operation_id"], candidate=dict(sha256=request["candidate_sha256"]),
                renderer_source=dict(sha256=request["renderer_source_sha256"]), publication=request["publication"])
    assert MODULE.admit_receipt(receipt, 0, plan) == receipt
    for mutation in ({"private_body": "do-not-export"}, {"qualified": True}, {"operation_id": "c" * 32},
                     {"journal_identity": {"private": "do-not-export"}}, {"phase": "arbitrary-private-text"},
                     {"configuration_published": False}, {"active_context_nginx_exit_code": "private-text"}):
        with pytest.raises(MODULE.checked.CheckError) as caught:
            MODULE.admit_receipt({**receipt, **mutation}, 0, plan)
        assert "do-not-export" not in str(caught.value)


def _prior(root, operation, digest):
    journal = root / (".taira-native-nginx-apply-" + operation + ".receipt.ndjson")
    return dict(operation_id=operation,
                journal=dict(path=str(journal), identity=_identity(journal),
                             sha256=hashlib.sha256(journal.read_bytes()).hexdigest()),
                publication=dict(identity=_identity(root / "conf.d/new-scoped.conf"), sha256=digest))


def _replacement(native_apply):
    request, root, original = native_apply
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 0, result
    candidate = b"upstream selected { server 127.0.0.1:18480; }\n"
    replacement = {**request, "operation_id": "b" * 32,
                   "candidate_base64": base64.b64encode(candidate).decode(),
                   "candidate_sha256": hashlib.sha256(candidate).hexdigest(),
                   "publication": dict(kind="replace", prior=_prior(root, request["operation_id"], request["candidate_sha256"]))}
    return replacement, root, original, candidate


def _inspection(native_apply):
    request, root, original, candidate = _replacement(native_apply)
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 0, result
    request["publication"] = dict(kind="reconcile", prior=_prior(
        root, request["operation_id"], request["candidate_sha256"]))
    return request, root


def test_local_owned_apply_runs_in_isolated_four_source_capsule_without_retry(native_apply):
    request, root, candidate = native_apply
    capsule = root / "capsule"
    capsule.mkdir(mode=0o700)
    for name in ("taira_native_nginx_check.py", "taira_native_nginx_apply.py",
                 "taira_native_validator_forwarding.py", "taira_native_edge_completion.py"):
        shutil.copyfile(MODULE_PATH.parent / name, capsule / name)
    public = capsule / "candidate.conf"
    public.write_bytes(candidate)
    public.chmod(0o600)
    renderer = capsule / "renderer.py"
    renderer.write_bytes(b"# maintained public renderer fixture\n")
    renderer.chmod(0o600)
    reference = dict(path=str(renderer), sha256=hashlib.sha256(renderer.read_bytes()).hexdigest())
    plan = dict(schema=MODULE.PLAN_SCHEMA, provider="macstadium-dublin", host_kind=request["host_kind"],
        deployment_reference=reference, native=request["native"], renderer_source=reference,
        candidate=dict(path=str(public), sha256=request["candidate_sha256"], owner_uid=os.geteuid()),
        operation_id=request["operation_id"], destination=request["destination"], master=request["master"],
        publication=request["publication"])
    input_path = capsule / "plan.json"
    input_path.write_text(json.dumps(plan))
    runner = '''import json, pathlib, sys, types
root=pathlib.Path(sys.argv[1])
for name in ("taira_native_nginx_check", "taira_native_nginx_apply"):
    module=types.ModuleType(name); module.__file__=str(root/(name+".py"))
    sys.modules[name]=module
    exec(compile(pathlib.Path(module.__file__).read_bytes(),module.__file__,"exec"),module.__dict__)
owner=sys.modules["taira_native_nginx_apply"]
owner.observe_master=lambda expected: expected
owner.open_master_handle=lambda expected: None
owner.signal_master=lambda expected, retained: "adjacent_native_identity"
assert "taira_retry" not in sys.modules
result=owner.apply_owned_publication(json.loads((root/"plan.json").read_bytes()))
assert "taira_retry" not in sys.modules
print(json.dumps(result))
'''
    result = subprocess.run([sys.executable, "-I", "-c", runner, str(capsule)],
                            capture_output=True, timeout=30)
    assert result.returncode == 0, result.stderr.decode()
    receipt = json.loads(result.stdout)
    assert receipt["configuration_published"] and receipt["qualified"] is False
    assert (root / "conf.d/new-scoped.conf").read_bytes() == candidate


def test_owned_inspection_is_numeric_read_only_and_has_one_closed_output(native_apply, monkeypatch):
    request, root = _inspection(native_apply)
    original_journal = Path(request["publication"]["prior"]["journal"]["path"]).read_bytes()
    reloads = (root / "reload-count").read_bytes()
    def forbidden(*args):
        raise AssertionError("read-only inspection performed a native effect")
    monkeypatch.setattr(MODULE, "signal_master", forbidden)
    monkeypatch.setattr(MODULE, "native_exchange", forbidden)
    result = MODULE.inspect_owned_publication(request)
    assert set(result) == {"schema", "owned_publication", "nginx", "main_configuration", "master", "phase"}
    assert result["phase"] == "awaiting_readiness"
    assert set(result["owned_publication"]) == {"operation_id", "journal", "publication"}
    identity = result["owned_publication"]["publication"]["file"]["identity"]
    assert all(type(value) is int for value in identity.values())
    assert identity["mtime_ns"] > 2 ** 53
    assert set(result["main_configuration"]) == {"path", "identity"}
    assert Path(request["publication"]["prior"]["journal"]["path"]).read_bytes() == original_journal
    assert (root / "reload-count").read_bytes() == reloads
    _clean(root)


@pytest.mark.parametrize("replacement", ["journal", "publication"])
def test_owned_inspection_rejects_substitution_without_effect(native_apply, replacement):
    request, root = _inspection(native_apply)
    reloads = (root / "reload-count").read_bytes()
    target = (Path(request["publication"]["prior"]["journal"]["path"])
              if replacement == "journal" else root / "conf.d/new-scoped.conf")
    substitute = root / "foreign-inspection-input"
    substitute.write_bytes(target.read_bytes())
    substitute.chmod(0o600)
    os.replace(substitute, target)
    with pytest.raises(RuntimeError, match="owned_publication_inspection_refused"):
        MODULE.inspect_owned_publication(request)
    assert (root / "reload-count").read_bytes() == reloads
    _clean(root)


def _plan(request):
    return dict(host_kind=request["host_kind"], native=request["native"], destination=request["destination"],
                operation_id=request["operation_id"], candidate=dict(sha256=request["candidate_sha256"]),
                renderer_source=dict(sha256=request["renderer_source_sha256"]), publication=request["publication"])


def test_owned_replacement_retains_original_inode_and_explicit_journal_chain(native_apply):
    request, root, original, candidate = _replacement(native_apply)
    prior = copy.deepcopy(request["publication"]["prior"])
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 0, result
    assert MODULE.admit_receipt(result, 0, _plan(request)) == result
    assert result["qualified"] is False and result["phase"] == "awaiting_readiness"
    assert result["publication_kind"] == "replace" and result["prior_operation_id"] == "a" * 32
    current = root / "conf.d/new-scoped.conf"
    backup = root / "conf.d" / (".taira-nginx-backup-" + "b" * 32 + ".public")
    assert current.read_bytes() == candidate and backup.read_bytes() == original
    assert str(backup.stat().st_ino) == prior["publication"]["identity"]["inode"]
    assert current.stat().st_nlink == backup.stat().st_nlink == 1
    rows = _journal(root, "b" * 32)
    assert all(row["prior"] == prior for row in rows)
    assert rows[1]["phase"] == "publishing" and rows[1]["temporary_basename"] == backup.name
    assert rows[-1]["backup_identity"] == _identity(backup)
    assert _journal(root)[-1]["phase"] == "awaiting_readiness"
    assert (root / "reload-count").read_text() == "2"
    _clean(root)


@pytest.mark.parametrize("flag", ["reject-context", "omit-source", "reject-reload"])
def test_replacement_failure_restores_only_original_public_inode(native_apply, flag):
    request, root, original, _ = _replacement(native_apply)
    old_inode = request["publication"]["prior"]["publication"]["identity"]["inode"]
    if flag == "reject-reload":
        (root / "reload-count").unlink()
    (root / flag).touch()
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 1 and result["qualified"] is False, result
    current = root / "conf.d/new-scoped.conf"
    assert current.read_bytes() == original and str(current.stat().st_ino) == old_inode
    assert result["configuration_published"] is False
    assert _journal(root, "b" * 32)[-1]["phase"] == "rolled_back_unqualified", result
    assert not list((root / "conf.d").glob(".taira-nginx-backup-*"))
    _clean(root)


@pytest.mark.parametrize("field", ["journal_digest", "publication_digest", "publication_inode", "journal_identity"])
def test_replacement_refuses_drifted_prior_before_effect(native_apply, field):
    request, root, original, _ = _replacement(native_apply)
    prior = request["publication"]["prior"]
    if field == "journal_digest": prior["journal"]["sha256"] = "0" * 64
    elif field == "publication_digest": prior["publication"]["sha256"] = "0" * 64
    elif field == "publication_inode": prior["publication"]["identity"]["inode"] = "1"
    else: prior["journal"]["identity"]["ctime_ns"] = "1"
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 1 and result["configuration_published"] is False
    assert (root / "conf.d/new-scoped.conf").read_bytes() == original
    assert (root / "reload-count").read_text() == "1"
    assert not (root / (".taira-native-nginx-apply-" + "b" * 32 + ".receipt.ndjson")).exists()
    _clean(root)


def test_replacement_cannot_supersede_interrupted_or_qualified_owner(native_apply):
    request, root, original, _ = _replacement(native_apply)
    journal = Path(request["publication"]["prior"]["journal"]["path"])
    rows = _journal(root)
    for mutation in ({"phase": "reload_requested"}, {"qualified": True}):
        edited = [*rows[:-1], {**rows[-1], **mutation}]
        journal.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in edited))
        request["publication"]["prior"] = _prior(root, "a" * 32, hashlib.sha256(original).hexdigest())
        result = MODULE.remote_apply(request)
        assert result["exit_code"] == 1 and (root / "conf.d/new-scoped.conf").read_bytes() == original
        assert (root / "reload-count").read_text() == "1"


def test_exact_same_operation_reconciliation_resumes_after_ambiguous_reload(native_apply, monkeypatch):
    request, root, original, candidate = _replacement(native_apply)
    signal_owner = MODULE.signal_master
    def crash(expected, retained):
        raise KeyboardInterrupt("owned test crash after journaled reload intent")
    monkeypatch.setattr(MODULE, "signal_master", crash)
    with pytest.raises(KeyboardInterrupt): MODULE.remote_apply(request)
    assert _journal(root, "b" * 32)[-1]["phase"] == "reload_requested"
    monkeypatch.setattr(MODULE, "signal_master", signal_owner)
    reconciliation = {**request, "publication": dict(kind="reconcile", prior=_prior(root, "b" * 32, request["candidate_sha256"]))}
    result = MODULE.remote_apply(reconciliation)
    assert result["exit_code"] == 0, result
    assert result["publication_kind"] == "reconcile" and result["qualified"] is False
    assert MODULE.admit_receipt(result, 0, _plan(reconciliation)) == result
    assert (root / "conf.d/new-scoped.conf").read_bytes() == candidate
    assert Path(result["backup_path"]).read_bytes() == original
    assert _journal(root, "b" * 32)[-1]["phase"] == "awaiting_readiness"
    assert len(list(root.glob(".taira-native-nginx-apply-*"))) == 2


@pytest.mark.parametrize("after_exchange", [False, True])
def test_interrupted_exchange_keeps_durable_intent_and_never_allows_blind_new_owner(native_apply, monkeypatch, after_exchange):
    request, root, original, candidate = _replacement(native_apply)
    exchange = MODULE.native_exchange
    def crash(*args):
        if after_exchange: exchange(*args)
        raise KeyboardInterrupt("owned test crash at native exchange")
    monkeypatch.setattr(MODULE, "native_exchange", _configuration_exchange(crash))
    with pytest.raises(KeyboardInterrupt): MODULE.remote_apply(request)
    assert _journal(root, "b" * 32)[-1]["phase"] == "publishing"
    assert (root / "conf.d/new-scoped.conf").read_bytes() == (candidate if after_exchange else original)
    backup = root / "conf.d" / (".taira-nginx-backup-" + "b" * 32 + ".public")
    assert backup.read_bytes() == (original if after_exchange else candidate)
    blind = {**request, "operation_id": "c" * 32}
    result = MODULE.remote_apply(blind)
    assert result["exit_code"] == 1
    assert (root / "reload-count").read_text() == "1"
    assert (root / "conf.d/new-scoped.conf").read_bytes() == (candidate if after_exchange else original)


def test_generated_remote_replacement_executes_the_same_owner_and_exchange(native_apply, monkeypatch):
    request, root, original, candidate = _replacement(native_apply)
    with monkeypatch.context() as original_functions:
        original_functions.setattr(MODULE, "observe_master", OBSERVE_MASTER)
        original_functions.setattr(MODULE, "open_master_handle", OPEN_MASTER_HANDLE)
        original_functions.setattr(MODULE, "signal_master", SIGNAL_MASTER)
        program = MODULE.remote_program(request).decode()
    # Only native master observation/signaling is replaced in this physical
    # child fixture; all generated custody, journal and exchange code executes.
    program = program.replace("result = remote_apply(json.loads(",
        "observe_master = lambda expected: expected\n"
        "open_master_handle = lambda expected: None\n"
        "signal_master = lambda expected, retained: 'adjacent_native_identity'\n"
        "result = remote_apply(json.loads(")
    child = subprocess.run([sys.executable, "-I", "-"], input=program.encode(),
                           capture_output=True, timeout=30)
    assert child.returncode == 0, child.stderr
    receipt = json.loads(child.stdout)
    assert MODULE.admit_receipt(receipt, 0, _plan(request)) == receipt
    assert receipt["qualified"] is False and Path(receipt["backup_path"]).read_bytes() == original
    assert (root / "conf.d/new-scoped.conf").read_bytes() == candidate
    assert b"private-existing" not in child.stdout and not child.stderr


def test_final_pre_signal_path_swap_is_preserved_and_never_reloaded(native_apply, monkeypatch):
    request, root, original, candidate = _replacement(native_apply)
    (root / "context-completed").unlink()
    read = os.pread
    swapped = False
    def swap_after_read(opened, count, offset):
        nonlocal swapped
        data = read(opened, count, offset)
        if not swapped and data == candidate and (root / "context-completed").exists():
            foreign = root / "foreign-owner"
            foreign.write_bytes(b"foreign owner body")
            os.replace(foreign, root / "conf.d/new-scoped.conf")
            swapped = True
        return data
    monkeypatch.setattr(MODULE.os, "pread", swap_after_read)
    result = MODULE.remote_apply(request)
    assert swapped and result["exit_code"] == 1 and result["recovery_pending"] is True
    assert result["rollback_ambiguous"] and result["qualified"] is False
    assert (root / "conf.d/new-scoped.conf").read_bytes() == b"foreign owner body"
    backup = root / "conf.d" / (".taira-nginx-backup-" + "b" * 32 + ".public")
    assert backup.read_bytes() == original
    assert (root / "reload-count").read_text() == "1"
    assert _journal(root, "b" * 32)[-1]["phase"] == "rollback_ambiguous"


def test_changed_temporary_before_exchange_is_preserved_with_cleanup_ambiguity(native_apply, monkeypatch):
    request, root, original, _ = _replacement(native_apply)
    read = os.pread
    temporary = root / "conf.d" / (".taira-nginx-backup-" + "b" * 32 + ".public")
    changed = False
    def change_temporary_and_refuse(opened, count, offset):
        nonlocal changed
        data = read(opened, count, offset)
        if not changed and data == original and temporary.exists():
            temporary.write_bytes(b"foreign temporary mutation")
            changed = True
            raise RuntimeError("test_refusal")
        return data
    monkeypatch.setattr(MODULE.os, "pread", change_temporary_and_refuse)
    result = MODULE.remote_apply(request)
    assert changed and result["exit_code"] == 1 and result["publication_cleanup_ambiguous"] is True
    assert temporary.read_bytes() == b"foreign temporary mutation"
    assert (root / "conf.d/new-scoped.conf").read_bytes() == original
    assert (root / "reload-count").read_text() == "1"


@pytest.mark.parametrize("kind", ["create", "replace"])
def test_publication_refuses_stage_permission_drift_without_rebaselining(native_apply, monkeypatch, kind):
    if kind == "replace":
        request, root, original, candidate = _replacement(native_apply)
        exchange = MODULE.native_exchange
        def change_stage_after_exchange(directory, first, second):
            exchange(directory, first, second)
            os.chmod(second, 0o666, dir_fd=directory)
        monkeypatch.setattr(MODULE, "native_exchange", _configuration_exchange(change_stage_after_exchange))
    else:
        request, root, candidate = native_apply
        link = os.link
        def change_stage_after_link(*args, **kwargs):
            link(*args, **kwargs)
            os.chmod(args[0], 0o666, dir_fd=kwargs["src_dir_fd"])
        monkeypatch.setattr(MODULE.os, "link", change_stage_after_link)
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 1 and result["error_code"] == "publication_stage_changed"
    assert result["recovery_pending"] and result["rollback_ambiguous"]
    assert (root / "conf.d/new-scoped.conf").read_bytes() == candidate
    assert stat.S_IMODE((root / "conf.d/new-scoped.conf").stat().st_mode) == 0o666
    if kind == "replace":
        assert (root / "reload-count").read_text() == "1"
        assert (root / "conf.d" / (".taira-nginx-backup-" + request["operation_id"] + ".public")).read_bytes() == original
    else:
        assert not (root / "reload-count").exists()
    assert not list(root.glob(".taira-nginx-check-*"))


def test_nested_ancestor_private_main_is_refused_before_python_read(native_apply, monkeypatch):
    request, root, original, _ = _replacement(native_apply)
    reference = request["publication"]["prior"]["journal"]
    path = Path(reference["path"])
    rows = [json.loads(row) for row in path.read_text().splitlines()]
    foreign = dict(operation_id="c" * 32,
        journal=dict(path=str(root / "nginx.conf"), identity=_identity(root / "nginx.conf"), sha256="0" * 64),
        publication=copy.deepcopy(request["publication"]["prior"]["publication"]))
    for row in rows: row["prior"] = foreign
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))
    reference.update(identity=_identity(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    private = (root / "nginx.conf").stat()
    read = os.pread
    def no_private_read(opened, count, offset):
        observed = os.fstat(opened)
        assert (observed.st_dev, observed.st_ino) != (private.st_dev, private.st_ino), "private native main reached Python pread"
        return read(opened, count, offset)
    monkeypatch.setattr(MODULE.os, "pread", no_private_read)
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 1 and result["error_code"] == "prior_journal_reference"
    assert (root / "conf.d/new-scoped.conf").read_bytes() == original
    assert (root / "reload-count").read_text() == "1"
    assert not (root / (".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson")).exists()
    _clean(root)


def test_apply_refuses_same_inode_lock_drift_adjacent_to_reload(native_apply, monkeypatch):
    request, root, original, candidate = _replacement(native_apply)
    (root / "context-completed").unlink()
    read = os.pread
    changed = False
    def change_lock_after_read(opened, count, offset):
        nonlocal changed
        data = read(opened, count, offset)
        if not changed and data == candidate and (root / "context-completed").exists():
            (root / ".taira-native-nginx-check.lock").chmod(0o666)
            changed = True
        return data
    monkeypatch.setattr(MODULE.os, "pread", change_lock_after_read)
    result = MODULE.remote_apply(request)
    assert changed and result["exit_code"] == 1 and result["error_code"] == "check_lock_identity_changed"
    assert result["rollback_ambiguous"] and result["recovery_pending"]
    assert (root / "reload-count").read_text() == "1"
    assert (root / "conf.d/new-scoped.conf").read_bytes() == candidate
    assert (root / "conf.d" / (".taira-nginx-backup-" + request["operation_id"] + ".public")).read_bytes() == original
    assert not list(root.glob(".taira-nginx-check-*"))


def test_read_only_inspection_refuses_same_inode_lock_drift(native_apply, monkeypatch):
    request, root = _inspection(native_apply)
    reloads = (root / "reload-count").read_bytes()
    def change_lock(expected):
        (root / ".taira-native-nginx-check.lock").chmod(0o666)
        return expected
    monkeypatch.setattr(MODULE, "observe_master", change_lock)
    with pytest.raises(RuntimeError, match="owned_publication_inspection_refused"):
        MODULE.inspect_owned_publication(request)
    assert (root / "reload-count").read_bytes() == reloads
    assert not list(root.glob(".taira-nginx-check-*"))


def _interrupted_local_plan(request, root):
    candidate = root / 'public-candidate.conf'
    candidate.write_bytes(base64.b64decode(request['candidate_base64']))
    candidate.chmod(0o600)
    renderer = root / 'public-renderer.py'
    renderer.write_bytes(b'# maintained public renderer\n')
    renderer.chmod(0o600)
    request['renderer_source_sha256'] = hashlib.sha256(renderer.read_bytes()).hexdigest()
    return dict(schema=MODULE.PLAN_SCHEMA, provider='macstadium-dublin', host_kind=request['host_kind'],
        deployment_reference=dict(path=str(root/'bound-deployment.json'), sha256='0'*64),
        native=request['native'], candidate=dict(path=str(candidate), sha256=request['candidate_sha256'],
        owner_uid=os.geteuid()), renderer_source=dict(path=str(renderer), sha256=request['renderer_source_sha256']),
        master=request['master'], operation_id=request['operation_id'], destination=request['destination'],
        publication=request['publication'])


def _interrupted_journal(root, operation):
    path = root / ('.taira-native-nginx-apply-' + operation + '.receipt.ndjson')
    return dict(path=str(path), identity=_identity(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def test_same_operation_recovery_before_child_started_requires_native_journal_absence(native_apply):
    request, root, candidate = native_apply
    plan = _interrupted_local_plan(request, root)
    result = MODULE.reconcile_interrupted_publication(plan, None, "resume")
    assert result['exit_code'] == 0 and result['phase'] == 'awaiting_readiness', result
    assert result['operation_id'] == request['operation_id'] and result['publication_kind'] == 'create'
    assert (root/'conf.d/new-scoped.conf').read_bytes() == candidate
    before = _interrupted_journal(root, request['operation_id'])
    reloads = (root/'reload-count').read_bytes()
    refused = MODULE.reconcile_interrupted_publication(plan, None, "resume")
    assert refused['exit_code'] == 1 and refused['recovery_pending'] is True
    assert refused['error_code'] == 'interrupted_journal_already_exists'
    assert _interrupted_journal(root, request['operation_id']) == before
    assert (root/'reload-count').read_bytes() == reloads


@pytest.mark.parametrize('kind', ['create', 'replace'])
@pytest.mark.parametrize('after_effect', [False, True])
def test_same_operation_recovers_journal_proven_link_or_exchange(native_apply, monkeypatch, kind, after_effect):
    if kind == 'replace':
        request, root, original, candidate = _replacement(native_apply)
    else:
        request, root, candidate = native_apply
    plan = _interrupted_local_plan(request, root)
    exchange, link, unlink = MODULE.native_exchange, MODULE.os.link, MODULE.os.unlink
    def interrupt(*args, **kwargs):
        if after_effect:
            (exchange if kind == 'replace' else link)(*args, **kwargs)
        raise KeyboardInterrupt('owned publication crash window')
    monkeypatch.setattr(MODULE, 'native_exchange' if kind == 'replace' else 'unused_exchange', _configuration_exchange(interrupt), raising=False)
    if kind == 'create':
        monkeypatch.setattr(MODULE.os, 'link', interrupt)
        # A killed owner would not execute finally; keep its admitted stage for
        # the resumed process instead of simulating an orderly cleanup.
        def retained_stage(name, *args, **kwargs):
            if str(name).startswith('.taira-nginx-publish-'):
                raise OSError('owned killed process does not clean its stage')
            return unlink(name, *args, **kwargs)
        monkeypatch.setattr(MODULE.os, 'unlink', retained_stage)
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    journal = _interrupted_journal(root, request['operation_id'])
    assert _journal(root, request['operation_id'])[-1]['phase'] == 'publishing'
    monkeypatch.setattr(MODULE, 'native_exchange', exchange)
    monkeypatch.setattr(MODULE.os, 'link', link)
    monkeypatch.setattr(MODULE.os, 'unlink', unlink)
    result = MODULE.reconcile_interrupted_publication(plan, journal, "resume")
    assert result['exit_code'] == 0 and result['phase'] == 'awaiting_readiness', result
    assert result['publication_kind'] == kind and result['operation_id'] == request['operation_id']
    assert (root/'conf.d/new-scoped.conf').read_bytes() == candidate
    assert not list((root/'conf.d').glob('.taira-nginx-publish-*'))
    if kind == 'replace':
        assert (root/'conf.d'/('.taira-nginx-backup-'+request['operation_id']+'.public')).read_bytes() == original
    assert all(row['operation_id'] == request['operation_id'] for row in _journal(root, request['operation_id']))


def test_completed_same_operation_recovery_is_read_only_no_hup_or_journal_append(native_apply, monkeypatch):
    request, root, candidate = native_apply
    plan = _interrupted_local_plan(request, root)
    assert MODULE.apply_owned_publication(plan)['exit_code'] == 0
    before = _interrupted_journal(root, request['operation_id'])
    publication = _identity(root/'conf.d/new-scoped.conf')
    monkeypatch.setattr(MODULE, 'signal_master', lambda *args: pytest.fail('completed owner was reloaded'))
    monkeypatch.setattr(MODULE, 'native_exchange', lambda *args: pytest.fail('completed owner was exchanged'))
    result = MODULE.reconcile_interrupted_publication(plan, before, "resume")
    assert result['exit_code'] == 0 and result['phase'] == 'awaiting_readiness', result
    assert _interrupted_journal(root, request['operation_id']) == before
    assert _identity(root/'conf.d/new-scoped.conf') == publication
    assert result['qualified'] is False and result['private_config_read_by_controller'] is False


@pytest.mark.parametrize('fault', ['foreign_destination', 'changed_journal', 'changed_original_plan', 'wrong_operation'])
def test_interrupted_reconciliation_refuses_foreign_or_unbound_publication(native_apply, monkeypatch, fault):
    request, root, original, candidate = _replacement(native_apply)
    plan = _interrupted_local_plan(request, root)
    exchange = MODULE.native_exchange
    def crash(*args):
        exchange(*args)
        raise KeyboardInterrupt('owned publication interrupted after exchange')
    monkeypatch.setattr(MODULE, 'native_exchange', _configuration_exchange(crash))
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    monkeypatch.setattr(MODULE, 'native_exchange', exchange)
    reference = _interrupted_journal(root, request['operation_id'])
    destination = root/'conf.d/new-scoped.conf'
    if fault == 'foreign_destination':
        foreign = root/'foreign-public.conf'
        foreign.write_bytes(b'foreign data must survive\n'); foreign.chmod(0o600)
        os.replace(foreign, destination)
    elif fault == 'changed_journal':
        Path(reference['path']).write_bytes(b'foreign record\n')
    elif fault == 'changed_original_plan':
        plan['master']['started'] = 'Sun Oct 4 02:03:04 2026'
    else:
        plan['operation_id'] = 'c'*32
    publication_before = destination.read_bytes(), _identity(destination)
    journal_before = Path(reference['path']).read_bytes()
    reloads = (root/'reload-count').read_bytes()
    try:
        result = MODULE.reconcile_interrupted_publication(plan, reference, "resume")
    except RuntimeError as error:
        assert fault == 'wrong_operation' and error.args[0] == 'interrupted_operation_identity'
    else:
        assert result['exit_code'] == 1 and result['recovery_pending'] is True, result
    assert (destination.read_bytes(), _identity(destination)) == publication_before
    assert Path(reference['path']).read_bytes() == journal_before
    assert (root/'reload-count').read_bytes() == reloads


@pytest.mark.parametrize('kind', ['create', 'replace'])
@pytest.mark.parametrize('after_effect', [False, True])
def test_direct_partial_rollback_restores_without_candidate_context_or_hup(native_apply, monkeypatch, kind, after_effect):
    if kind == 'replace':
        request, root, original, candidate = _replacement(native_apply)
    else:
        request, root, candidate = native_apply
        original = None
    plan = _interrupted_local_plan(request, root)
    exchange, link, unlink, signal = MODULE.native_exchange, MODULE.os.link, MODULE.os.unlink, MODULE.signal_master
    def interrupt(*args, **kwargs):
        if after_effect:
            (exchange if kind == 'replace' else link)(*args, **kwargs)
        raise KeyboardInterrupt('owned publication crash')
    if kind == 'replace':
        monkeypatch.setattr(MODULE, 'native_exchange', _configuration_exchange(interrupt))
    else:
        monkeypatch.setattr(MODULE.os, 'link', interrupt)
        def retain_stage(name, *args, **kwargs):
            if str(name).startswith('.taira-nginx-publish-'):
                raise OSError('killed owner retained stage')
            return unlink(name, *args, **kwargs)
        monkeypatch.setattr(MODULE.os, 'unlink', retain_stage)
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    reference = _interrupted_journal(root, request['operation_id'])
    monkeypatch.setattr(MODULE, 'native_exchange', exchange)
    monkeypatch.setattr(MODULE.os, 'link', link)
    monkeypatch.setattr(MODULE.os, 'unlink', unlink)
    reloads = int((root/'reload-count').read_text()) if (root/'reload-count').exists() else 0
    def only_original_reload(expected, retained):
        destination = root/'conf.d/new-scoped.conf'
        if original is None:
            assert not destination.exists()
        else:
            assert destination.read_bytes() == original, 'candidate was loaded during rollback'
        return signal(expected, retained)
    monkeypatch.setattr(MODULE, 'signal_master', only_original_reload)
    result = MODULE.reconcile_interrupted_publication(plan, reference, 'rollback')
    assert result.get('error_code') is None, result
    assert result['exit_code'] == 0 and result['phase'] == 'rolled_back_unqualified', result
    assert result['configuration_published'] is False and result['qualified'] is False
    assert result['journal']['file']['identity'] == {key:int(value) for key,value in _identity(Path(reference['path'])).items()}
    assert result['journal']['sha256'] == hashlib.sha256(Path(reference['path']).read_bytes()).hexdigest()
    actual_reloads = int((root/'reload-count').read_text()) if (root/'reload-count').exists() else 0
    assert actual_reloads == reloads + int(after_effect)
    if kind == 'replace':
        restored = result['restored_owned_publication']
        assert restored['operation_id'] == 'a'*32
        assert (root/'conf.d/new-scoped.conf').read_bytes() == original
        assert restored['publication']['file']['identity'] == {key:int(value) for key,value in _identity(root/'conf.d/new-scoped.conf').items()}
        assert restored['journal']['file']['identity'] == {key:int(value) for key,value in _identity(Path(restored['journal']['file']['path'])).items()}
        assert _journal(root, 'a'*32)[-1]['phase'] == 'awaiting_readiness'
        assert _journal(root, 'a'*32)[-1]['publication_identity'] == _identity(root/'conf.d/new-scoped.conf')
    else:
        assert not (root/'conf.d/new-scoped.conf').exists()
        assert result['restored_owned_publication'] is None
    assert not list((root/'conf.d').glob('.taira-nginx-backup-*'))
    assert not list((root/'conf.d').glob('.taira-nginx-publish-*'))
    current = _interrupted_journal(root, request['operation_id'])
    monkeypatch.setattr(MODULE, 'signal_master', lambda *args: pytest.fail('terminal rollback repeated HUP'))
    again = MODULE.reconcile_interrupted_publication(plan, current, 'rollback')
    assert again['exit_code'] == 0 and again['journal'] == result['journal'], again


@pytest.mark.parametrize('kind', ['create', 'replace'])
def test_absent_publisher_rollback_proves_no_effect_and_creates_no_journal(native_apply, monkeypatch, kind):
    if kind == 'replace':
        request, root, original, candidate = _replacement(native_apply)
    else:
        request, root, candidate = native_apply
    plan = _interrupted_local_plan(request, root)
    before = _prior(root, 'a'*32, hashlib.sha256(original).hexdigest()) if kind == 'replace' else None
    monkeypatch.setattr(MODULE, 'signal_master', lambda *args: pytest.fail('absent publisher rollback HUP'))
    monkeypatch.setattr(MODULE, 'native_exchange', lambda *args: pytest.fail('absent publisher rollback exchange'))
    result = MODULE.reconcile_interrupted_publication(plan, None, 'rollback')
    assert result['exit_code'] == 0 and result['phase'] == 'not_requested', result
    assert result['journal'] is None and result['configuration_published'] is False
    assert not (root/('.taira-native-nginx-apply-'+request['operation_id']+'.receipt.ndjson')).exists()
    if before is not None:
        assert _prior(root, 'a'*32, hashlib.sha256(original).hexdigest()) == before
        assert result['restored_owned_publication']['operation_id'] == 'a'*32
    else:
        assert result['restored_owned_publication'] is None


def test_prepared_partial_rollback_never_publishes_or_loads_candidate(native_apply, monkeypatch):
    request, root, original, candidate = _replacement(native_apply)
    plan = _interrupted_local_plan(request, root)
    native_open = MODULE.os.open
    def before_stage(path, *args, **kwargs):
        if str(path).startswith('.taira-nginx-backup-'):
            raise KeyboardInterrupt('owned crash before allocating stage')
        return native_open(path, *args, **kwargs)
    monkeypatch.setattr(MODULE.os, 'open', before_stage)
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    monkeypatch.setattr(MODULE.os, 'open', native_open)
    reference = _interrupted_journal(root, request['operation_id'])
    assert _journal(root, request['operation_id'])[-1]['phase'] == 'prepared'
    monkeypatch.setattr(MODULE, 'signal_master', lambda *args: pytest.fail('prepared rollback HUP'))
    monkeypatch.setattr(MODULE, 'native_exchange', _configuration_exchange(lambda *args: pytest.fail('prepared rollback exchange')))
    result = MODULE.reconcile_interrupted_publication(plan, reference, 'rollback')
    assert result.get('error_code') is None, result
    assert result['exit_code'] == 0 and result['phase'] == 'rolled_back_unqualified', result
    assert (root/'conf.d/new-scoped.conf').read_bytes() == original
    assert result['restored_owned_publication']['operation_id'] == 'a'*32


@pytest.mark.parametrize('fault', ['after_restore_exchange', 'after_original_hup', 'after_predecessor_append', 'after_cleanup'])
def test_partial_rollback_resumes_exact_owned_terminal_frontier(native_apply, monkeypatch, fault):
    request, root, original, candidate = _replacement(native_apply)
    plan = _interrupted_local_plan(request, root)
    exchange = MODULE.native_exchange
    def interrupt_publication(*args):
        exchange(*args)
        raise KeyboardInterrupt('owned cutover interrupted after exchange')
    monkeypatch.setattr(MODULE, 'native_exchange', _configuration_exchange(interrupt_publication))
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    monkeypatch.setattr(MODULE, 'native_exchange', exchange)
    reference = _interrupted_journal(root, request['operation_id'])
    signal, unlink = MODULE.signal_master, MODULE.os.unlink
    predecessor = root/('.taira-native-nginx-apply-'+('a'*32)+'.receipt.ndjson')
    exchanges = 0
    crashed = False
    def interrupt_exchange(*args):
        nonlocal exchanges, crashed
        if args[1].endswith('.receipt.ndjson') or args[2].endswith('.receipt.ndjson'):
            exchange(*args)
            if fault == 'after_predecessor_append' and predecessor.name in args[1:] and not crashed:
                crashed = True
                raise KeyboardInterrupt('owned crash after refreshed predecessor journal')
            return
        exchanges += 1
        exchange(*args)
        if fault == 'after_restore_exchange' and not crashed:
            crashed = True
            raise KeyboardInterrupt('owned crash after direct predecessor exchange')
    def interrupt_signal(*args):
        nonlocal crashed
        assert (root/'conf.d/new-scoped.conf').read_bytes() == original
        result = signal(*args)
        if fault == 'after_original_hup' and not crashed:
            crashed = True
            raise KeyboardInterrupt('owned crash after original-context HUP')
        return result
    def interrupt_unlink(name, *args, **kwargs):
        nonlocal crashed
        unlink(name, *args, **kwargs)
        if fault == 'after_cleanup' and str(name).startswith('.taira-nginx-backup-') and not crashed:
            crashed = True
            raise KeyboardInterrupt('owned crash after candidate backup cleanup')
    monkeypatch.setattr(MODULE, 'native_exchange', interrupt_exchange)
    monkeypatch.setattr(MODULE, 'signal_master', interrupt_signal)
    monkeypatch.setattr(MODULE.os, 'unlink', interrupt_unlink)
    with pytest.raises(KeyboardInterrupt):
        MODULE.reconcile_interrupted_publication(plan, reference, 'rollback')
    assert crashed and (root/'conf.d/new-scoped.conf').read_bytes() == original
    current = _interrupted_journal(root, request['operation_id'])
    result = MODULE.reconcile_interrupted_publication(plan, current, 'rollback')
    assert result['exit_code'] == 0 and result['phase'] == 'rolled_back_unqualified', result
    assert exchanges == 1
    assert result['restored_owned_publication']['publication']['file']['identity'] == {
        key:int(value) for key,value in _identity(root/'conf.d/new-scoped.conf').items()}
    assert not list((root/'conf.d').glob('.taira-nginx-backup-*'))
    assert _journal(root, 'a'*32)[-1]['publication_identity'] == _identity(root/'conf.d/new-scoped.conf')
    assert sum(row.get('cause') == 'exact_operation_rollback_restore' for row in _journal(root, 'a'*32)) == 1


@pytest.mark.parametrize('fault', ['foreign_backup', 'foreign_destination', 'changed_lock'])
def test_partial_rollback_preserves_foreign_custody_and_refuses_reload(native_apply, monkeypatch, fault):
    request, root, original, candidate = _replacement(native_apply)
    plan = _interrupted_local_plan(request, root)
    exchange = MODULE.native_exchange
    def crash(*args):
        exchange(*args)
        raise KeyboardInterrupt('owned publication interrupted')
    monkeypatch.setattr(MODULE, 'native_exchange', _configuration_exchange(crash))
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    monkeypatch.setattr(MODULE, 'native_exchange', exchange)
    reference = _interrupted_journal(root, request['operation_id'])
    destination = root/'conf.d/new-scoped.conf'
    backup = root/'conf.d'/('.taira-nginx-backup-'+request['operation_id']+'.public')
    if fault in {'foreign_backup', 'foreign_destination'}:
        foreign = root/'foreign-public'
        foreign.write_bytes(b'foreign retained bytes\n'); foreign.chmod(0o600)
        os.replace(foreign, backup if fault == 'foreign_backup' else destination)
    else:
        def change_lock(expected):
            (root/'.taira-native-nginx-check.lock').chmod(0o640)
            return None
        monkeypatch.setattr(MODULE, 'open_master_handle', change_lock)
    before = destination.read_bytes(), _identity(destination), backup.read_bytes(), _identity(backup)
    monkeypatch.setattr(MODULE, 'signal_master', lambda *args: pytest.fail('unowned rollback HUP'))
    result = MODULE.reconcile_interrupted_publication(plan, reference, 'rollback')
    assert result['exit_code'] == 1 and result['recovery_pending'] is True, result
    assert (destination.read_bytes(), _identity(destination), backup.read_bytes(), _identity(backup)) == before


def test_no_journal_rollback_refuses_intervening_foreign_intent_after_native_context(native_apply, monkeypatch):
    request, root, original, candidate = _replacement(native_apply)
    plan = _interrupted_local_plan(request, root)
    intended = root/('.taira-native-nginx-apply-'+request['operation_id']+'.receipt.ndjson')
    context_check = MODULE.check_owned_public_context
    def foreign_intent(*args):
        result = context_check(*args)
        intended.write_bytes(b'foreign intent must be preserved\n'); intended.chmod(0o600)
        return result
    monkeypatch.setattr(MODULE, 'check_owned_public_context', foreign_intent)
    monkeypatch.setattr(MODULE, 'signal_master', lambda *args: pytest.fail('foreign intent rollback HUP'))
    before = _prior(root, 'a'*32, hashlib.sha256(original).hexdigest())
    result = MODULE.reconcile_interrupted_publication(plan, None, 'rollback')
    assert result['exit_code'] == 1 and result['recovery_pending'] is True, result
    assert result['error_code'] == 'interrupted_journal_already_exists'
    assert result['journal'] is None and result['restored_owned_publication'] is None
    assert intended.read_bytes() == b'foreign intent must be preserved\n'
    assert _prior(root, 'a'*32, hashlib.sha256(original).hexdigest()) == before
