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


def _vacant_plan(native_apply):
    request, root, candidate = native_apply
    public = root / "first-candidate.conf"
    public.write_bytes(candidate)
    public.chmod(0o600)
    renderer = root / "public-renderer.py"
    renderer.write_bytes(b"# maintained first-publication renderer fixture\n")
    renderer.chmod(0o600)
    reference = dict(path=str(renderer), sha256=hashlib.sha256(renderer.read_bytes()).hexdigest())
    plan = dict(schema=MODULE.PLAN_SCHEMA, provider="macstadium-dublin", host_kind=request["host_kind"],
        deployment_reference=reference, native=request["native"], renderer_source=reference,
        candidate=dict(path=str(public), sha256=request["candidate_sha256"], owner_uid=os.geteuid()),
        operation_id=request["operation_id"], destination=request["destination"], master=request["master"],
        publication=request["publication"])
    return plan, request, root


def test_initial_vacancy_is_numeric_read_only_and_preserves_unrelated_namespace(native_apply, monkeypatch):
    plan, request, root = _vacant_plan(native_apply)
    unrelated = root / "conf.d/older-namespace.conf"
    unrelated.write_bytes(b"public unrelated namespace\n")
    unrelated.chmod(0o600)
    journal = root / (".taira-native-nginx-apply-" + "c" * 32 + ".receipt.ndjson")
    journal.write_text(json.dumps(dict(schema="iroha.taira.native-nginx-apply.journal.v1",
        destination_path=str(unrelated), phase="awaiting_readiness")) + "\n")
    journal.chmod(0o600)
    before = unrelated.read_bytes(), journal.read_bytes()
    def forbidden(*args):
        raise AssertionError("vacancy inspection performed an effect")
    monkeypatch.setattr(MODULE, "signal_master", forbidden)
    monkeypatch.setattr(MODULE, "native_exchange", forbidden)
    result = MODULE.inspect_vacant_publication(plan)
    assert set(result) == {"schema", "owned_publication", "nginx_config", "nginx", "main_configuration", "master", "phase"}
    assert result["owned_publication"] is None and result["phase"] == "vacant"
    assert result["nginx_config"] == str(root / "conf.d/new-scoped.conf")
    assert all(type(value) is int for value in result["main_configuration"]["identity"].values())
    assert "private-existing" not in json.dumps(result) and "private-native" not in json.dumps(result)
    assert not (root / "conf.d/new-scoped.conf").exists()
    assert not (root / (".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson")).exists()
    assert not (root / "reload-count").exists()
    assert (unrelated.read_bytes(), journal.read_bytes()) == before
    _clean(root)


@pytest.mark.parametrize("obstruction", ["publication", "dangling_symlink", "intended_journal", "unresolved_journal", "master", "lock"])
def test_initial_vacancy_refuses_existing_or_changed_owner_without_effect(native_apply, monkeypatch, obstruction):
    plan, request, root = _vacant_plan(native_apply)
    destination = root / "conf.d/new-scoped.conf"
    if obstruction == "publication":
        destination.write_bytes(b"existing public owner\n")
    elif obstruction == "dangling_symlink":
        destination.symlink_to(root / "absent-public-target")
    elif obstruction == "intended_journal":
        (root / (".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson")).write_text("existing exact journal\n")
    elif obstruction == "unresolved_journal":
        path = root / (".taira-native-nginx-apply-" + "c" * 32 + ".receipt.ndjson")
        path.write_text(json.dumps(dict(schema="iroha.taira.native-nginx-apply.journal.v1",
            destination_path=str(destination), phase="publishing")) + "\n")
        path.chmod(0o600)
    elif obstruction == "master":
        monkeypatch.setattr(MODULE, "observe_master", lambda expected: {**expected, "started":"changed"})
    else:
        def changed_lock(expected):
            (root / ".taira-native-nginx-check.lock").chmod(0o666)
            return expected
        monkeypatch.setattr(MODULE, "observe_master", changed_lock)
    with pytest.raises(RuntimeError, match="vacant_publication_inspection_refused"):
        MODULE.inspect_vacant_publication(plan)
    assert not (root / "reload-count").exists()
    if obstruction == "publication": assert destination.read_bytes() == b"existing public owner\n"
    if obstruction == "dangling_symlink": assert destination.is_symlink()
    if obstruction not in {"publication", "dangling_symlink"}: assert not destination.exists()
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
    assert set(result) == {"schema", "owned_publication", "nginx_config", "nginx", "main_configuration", "master", "phase"}
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


@pytest.mark.parametrize("phase", ["publishing", "published", "context_checking", "context_checked", "reload_requested"])
def test_owned_interruption_inspection_preserves_actual_first_publisher_and_strict_readiness(native_apply, phase):
    request, root, _ = native_apply
    assert MODULE.remote_apply(request)["exit_code"] == 0
    journal = root / (".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson")
    rows = _journal(root)
    index = next(index for index, row in enumerate(rows) if row["phase"] == phase)
    journal.write_text("".join(json.dumps(row) + "\n" for row in rows[:index+1]))
    request["publication"] = dict(kind="reconcile", prior=_prior(root, request["operation_id"], request["candidate_sha256"]))
    before = journal.read_bytes(), (root / "conf.d/new-scoped.conf").read_bytes(), (root / "reload-count").read_bytes()
    with pytest.raises(RuntimeError, match="owned_publication_inspection_refused"):
        MODULE.inspect_owned_publication(request)
    if phase == "publishing":
        # A hardlink interrupted before single-link/ctime acknowledgement stays ambiguous.
        with pytest.raises(RuntimeError, match="owned_publication_inspection_refused"):
            MODULE.inspect_owned_publication(request, allow_pending=True)
    else:
        result = MODULE.inspect_owned_publication(request, allow_pending=True)
        assert result["phase"] == phase and result["owned_publication"]["operation_id"] == request["operation_id"]
    assert (journal.read_bytes(), (root / "conf.d/new-scoped.conf").read_bytes(), (root / "reload-count").read_bytes()) == before
    _clean(root)


def _terminal_vacant_fixture(native_apply, monkeypatch, phase):
    plan, request, root = _vacant_plan(native_apply)
    request["renderer_source_sha256"] = plan["renderer_source"]["sha256"]
    if phase == "rolled_back_unqualified":
        (root / "reject-context").touch()
    else:
        def refused_link(*args, **kwargs):
            raise RuntimeError("fixture_refused_before_publication")
        monkeypatch.setattr(MODULE.os, "link", refused_link)
    result = MODULE.remote_apply(request)
    assert result["exit_code"] == 1 and _journal(root)[-1]["phase"] == phase
    assert not (root / "conf.d/new-scoped.conf").exists()
    journal = root / (".taira-native-nginx-apply-" + request["operation_id"] + ".receipt.ndjson")
    reference = dict(path=str(journal), identity=_identity(journal),
        sha256=hashlib.sha256(journal.read_bytes()).hexdigest())
    _clean(root)
    return plan, request, root, reference


@pytest.mark.parametrize("phase", ["rolled_back_unqualified", "refused_before_publication"])
def test_requested_terminal_vacancy_retains_actual_journal_without_fabricating_prior(native_apply, monkeypatch, phase):
    plan, request, root, reference = _terminal_vacant_fixture(native_apply, monkeypatch, phase)
    journal = Path(reference["path"])
    before = journal.read_bytes(), ((root / "reload-count").read_bytes() if (root / "reload-count").exists() else None)
    result = MODULE.inspect_terminal_vacant_publication(plan, reference)
    assert result["schema"] == "iroha.taira.native-nginx-terminal-vacancy-inspection.v1"
    assert set(result) == {"schema", "owned_publication", "publisher_journal", "terminal_phase", "nginx_config",
                           "nginx", "main_configuration", "master", "phase"}
    assert result["owned_publication"] is None and result["phase"] == "vacant_publisher_terminal"
    assert result["terminal_phase"] == phase and result["publisher_journal"]["sha256"] == reference["sha256"]
    assert result["publisher_journal"]["file"]["path"] == reference["path"]
    assert all(type(value) is int for value in result["publisher_journal"]["file"]["identity"].values())
    assert result["nginx_config"] == str(root / "conf.d/new-scoped.conf")
    assert "private-existing" not in json.dumps(result) and "private-native" not in json.dumps(result)
    with pytest.raises(RuntimeError, match="vacant_publication_inspection_refused"):
        MODULE.inspect_vacant_publication(plan)
    assert (journal.read_bytes(), ((root / "reload-count").read_bytes() if (root / "reload-count").exists() else None)) == before
    assert not (root / "conf.d/new-scoped.conf").exists()
    _clean(root)


@pytest.mark.parametrize("obstruction", ["nonterminal", "journal_substitution", "dangling_publication", "temporary_residue", "wrong_phase", "unresolved_owner"])
def test_requested_terminal_vacancy_refuses_changed_or_unfinished_owner(native_apply, monkeypatch, obstruction):
    plan, request, root, reference = _terminal_vacant_fixture(native_apply, monkeypatch, "rolled_back_unqualified")
    journal = Path(reference["path"])
    expected_phase = "rolled_back_unqualified"
    if obstruction == "nonterminal":
        rows = _journal(root)
        rows[-1]["phase"] = "publishing"
        journal.write_text("".join(json.dumps(row) + "\n" for row in rows))
        reference = dict(path=str(journal), identity=_identity(journal), sha256=hashlib.sha256(journal.read_bytes()).hexdigest())
    elif obstruction == "journal_substitution":
        replacement = root / "foreign-terminal-journal"
        replacement.write_bytes(journal.read_bytes()); replacement.chmod(0o600)
        os.replace(replacement, journal)
    elif obstruction == "dangling_publication":
        (root / "conf.d/new-scoped.conf").symlink_to(root / "absent-public-target")
    elif obstruction == "temporary_residue":
        temporary = next(row["temporary_basename"] for row in _journal(root) if "temporary_basename" in row)
        (root / "conf.d" / temporary).write_bytes(b"preserve unfinished public temporary\n")
    elif obstruction == "wrong_phase":
        expected_phase = "refused_before_publication"
    else:
        other = root / (".taira-native-nginx-apply-" + "c" * 32 + ".receipt.ndjson")
        other.write_text(json.dumps(dict(schema="iroha.taira.native-nginx-apply.journal.v1",
            destination_path=str(root / "conf.d/new-scoped.conf"), phase="publishing")) + "\n")
        other.chmod(0o600)
    before = journal.read_bytes(), (root / "reload-count").read_bytes()
    with pytest.raises(RuntimeError, match="terminal_vacant_publication_inspection_refused"):
        MODULE.inspect_terminal_vacant_publication(plan, reference, expected_phase)
    assert (journal.read_bytes(), (root / "reload-count").read_bytes()) == before
    assert obstruction == "dangling_publication" or not (root / "conf.d/new-scoped.conf").exists()
    assert not list(root.glob(".taira-nginx-check-*"))


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
    monkeypatch.setattr(MODULE, "native_exchange", crash)
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
        monkeypatch.setattr(MODULE, "native_exchange", change_stage_after_exchange)
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
