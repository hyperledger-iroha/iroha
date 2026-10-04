"""Create-only native nginx publication, journal and rollback regressions."""
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
assert "-s" not in sys.argv, "PID-file-derived reload is forbidden"
''')
        nginx.chmod(0o700)
        awk = Path(shutil.which("awk")).resolve()
        candidate = b"upstream selected { server 127.0.0.1:10080; }\n"
        master = dict(pid=1001, uid=os.geteuid(), started="Sun Oct 4 01:02:03 2026", executable=str(nginx))
        if sys.platform != "darwin":
            master.update(start_ticks="123456", boot_id="00000000-0000-0000-0000-000000000000")
        request = dict(host_kind="macos" if sys.platform == "darwin" else "linux", operation_id="a" * 32,
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


def _journal(root):
    path = root / (".taira-native-nginx-apply-" + "a" * 32 + ".receipt.ndjson")
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
        operation_id=request["operation_id"], destination=request["destination"], master=request["master"])
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
                renderer_source=dict(sha256=request["renderer_source_sha256"]))
    assert MODULE.admit_receipt(receipt, 0, plan) == receipt
    for mutation in ({"private_body": "do-not-export"}, {"qualified": True}, {"operation_id": "c" * 32},
                     {"journal_identity": {"private": "do-not-export"}}, {"phase": "arbitrary-private-text"},
                     {"configuration_published": False}, {"active_context_nginx_exit_code": "private-text"}):
        with pytest.raises(MODULE.checked.CheckError) as caught:
            MODULE.admit_receipt({**receipt, **mutation}, 0, plan)
        assert "do-not-export" not in str(caught.value)
