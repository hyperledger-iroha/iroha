"""Native-context, private-byte custody and cleanup regressions for nginx checks."""
from __future__ import annotations

import base64
import copy
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "taira_native_nginx_check.py"
SPEC = importlib.util.spec_from_file_location("taira_native_nginx_check", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def _identity(path: Path, directory: bool = False) -> dict:
    info = path.lstat()
    result = dict(device=info.st_dev, inode=info.st_ino, uid=info.st_uid,
                  gid=info.st_gid, mode=stat.S_IMODE(info.st_mode))
    if not directory:
        result.update(links=info.st_nlink, size=info.st_size,
                      mtime_ns=info.st_mtime_ns, ctime_ns=info.st_ctime_ns)
    return {name: str(number) for name, number in result.items()}


def _guard(configuration: str) -> subprocess.CompletedProcess:
    return subprocess.run([str(Path(shutil.which("awk")).resolve()), "-v", "candidate=/native/public.candidate",
                           MODULE.AWK_PROGRAM], input=configuration.encode(), capture_output=True, timeout=10)


def test_native_guard_injects_only_inside_one_real_http_block() -> None:
    private = '''# http { is a comment }
events { worker_connections 20; }
http # opening brace follows on the next line
{
  log_format hidden 'http { "a}b"';
  map $a $b { default "a\\\"{quoted}"; }
  include inherited.native;
}
'''
    guarded = _guard(private)
    assert guarded.returncode == 0
    result = guarded.stdout.decode()
    assert result.count("include /native/public.candidate;") == 1
    assert result.index("worker_connections") < result.index("include /native/public.candidate;")
    assert result.index("include /native/public.candidate;") < result.index("log_format")
    assert "include inherited.native;" in result


@pytest.mark.parametrize("configuration", [
    "events {}", "http {} http {}", "events { http {} }", "http {", "http { } }",
    'http { log_format x "unterminated; }', "http { # no closing brace\n",
    "#" + "x" * 262145 + "\nhttp {}", "http {" + "block {" * 128 + "}" * 129,
])
def test_native_guard_refuses_ambiguous_or_unbounded_main(configuration: str) -> None:
    assert _guard(configuration).returncode == 41


@pytest.fixture
def native_request():
    # An owner-controlled repository ancestor avoids relying on a writable /tmp
    # namespace; production must satisfy the same no-follow parent checks.
    with tempfile.TemporaryDirectory(prefix=".nginx-check-test-", dir=MODULE_PATH.parent) as temporary:
        directory = Path(temporary).resolve()
        main = directory / "nginx.conf"
        private = "events { worker_connections 20; }\nhttp { include inherited.native; }\n"
        main.write_text(private)
        (directory / "inherited.native").write_text("private-native-marker\n")
        native = directory / "nginx-native"
        native.write_text('''#!/usr/bin/python3
import pathlib, re, stat, sys
config = pathlib.Path(sys.argv[sys.argv.index("-c") + 1])
root = config.parent
body = config.read_text()
candidate = pathlib.Path(re.findall(r"include (/[^;]+\\.candidate);", body)[0])
assert "events { worker_connections 20; }" in body
assert "include inherited.native;" in body
assert (root / "inherited.native").read_text() == "private-native-marker\\n"
assert stat.S_IMODE(config.stat().st_mode) == 0o600
assert stat.S_IMODE(candidate.stat().st_mode) == 0o600
assert candidate.read_text() == "upstream selected { server 127.0.0.1:10080; }\\n"
print("private-native-error-body", file=sys.stderr)
if (root / "reject").exists(): sys.exit(7)
if (root / "replace-main").exists(): (root / "nginx.conf").write_text("http {}\\n")
if (root / "mutate-candidate").exists(): candidate.write_text(candidate.read_text() + "# changed\\n")
if (root / "mutate-validation-main").exists(): config.write_text(body + "# changed\\n")
if (root / "mutate-lock-mode").exists(): (root / ".taira-native-nginx-check.lock").chmod(0o666)
(root / "native-completed").touch()
''')
        native.chmod(0o700)
        awk = Path(shutil.which("awk")).resolve()
        candidate = b"upstream selected { server 127.0.0.1:10080; }\n"
        request = dict(host_kind="macos" if sys.platform == "darwin" else "linux",
                       candidate_base64=base64.b64encode(candidate).decode(),
                       candidate_sha256=hashlib.sha256(candidate).hexdigest(), renderer_source_sha256="a" * 64,
                       native=dict(owner_uid=os.geteuid(), trusted_group_gids=[],
                           directory=dict(path=str(directory), identity=_identity(directory, True)),
                           main=dict(path=str(main), identity=_identity(main)),
                           nginx=dict(path=str(native), identity=_identity(native)),
                           awk=dict(path=str(awk), identity=_identity(awk))))
        yield request, directory, private


def _assert_clean(directory: Path) -> None:
    assert not list(directory.glob(".taira-nginx-check-*"))
    assert stat.S_IMODE((directory / ".taira-native-nginx-check.lock").stat().st_mode) == 0o600


def test_real_native_children_check_inherited_context_without_exporting_private_bytes(native_request) -> None:
    request, directory, original = native_request
    result = MODULE.remote_check(request)
    assert result["exit_code"] == 0, result
    assert result["nginx_exit_code"] == 0
    assert result["inherited_configuration_checked"] is True
    assert result["relative_include_prefix_preserved"] is True
    assert result["validation_files_removed"] is True
    assert "private-native" not in json.dumps(result)
    assert (directory / "native-completed").exists()
    assert (directory / "nginx.conf").read_text() == original
    _assert_clean(directory)


def test_repeated_native_custody_guards_release_only_their_path_check_duplicates(native_request) -> None:
    request, directory, _ = native_request
    request = dict(request,publication=dict(kind="create"))
    def repeated_guard(receipt, request, context):
        retained = list(context["handles"])
        snapshots = {opened:context["identity"](os.fstat(opened),stat.S_ISDIR(os.fstat(opened).st_mode))
                     for opened in retained}
        for _ in range(200):
            for name, opened in context["bound"].items():
                context["revalidate"](context["native"][name],opened)
            context["revalidate"](context["native"]["directory"],context["directory"],True)
            if context["handles"] != retained:
                raise RuntimeError("revalidation_duplicate_retained")
        if not all(context["identity"](os.fstat(opened),stat.S_ISDIR(os.fstat(opened).st_mode)) == expected
                   for opened,expected in snapshots.items()):
            raise RuntimeError("retained_descriptor_identity_changed")
    result = MODULE.remote_check(request,repeated_guard)
    assert result["exit_code"] == 0 and result["validation_files_removed"] is True, result
    _assert_clean(directory)


def test_native_rejection_and_lexical_refusal_cleanup_without_private_error_body(native_request) -> None:
    request, directory, original = native_request
    (directory / "reject").touch()
    result = MODULE.remote_check(request)
    assert result["exit_code"] == 1 and result["nginx_exit_code"] == 7
    assert result["error_code"] == "native_nginx_rejected"
    assert "private-native" not in json.dumps(result)
    assert (directory / "nginx.conf").read_text() == original
    _assert_clean(directory)
    (directory / "nginx.conf").write_text("http {} http {}\n")
    request["native"]["main"]["identity"] = _identity(directory / "nginx.conf")
    result = MODULE.remote_check(request)
    assert result["error_code"] == "main_http_guard_rejected"
    assert "nginx_exit_code" not in result
    _assert_clean(directory)


def test_retained_lock_contention_refuses_before_staging(native_request) -> None:
    request, directory, _ = native_request
    lock = os.open(directory / ".taira-native-nginx-check.lock", os.O_RDWR | os.O_CREAT, 0o600)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        result = MODULE.remote_check(request)
        assert result["error_code"] == "check_owner_busy"
        assert not (directory / "native-completed").exists()
        _assert_clean(directory)
    finally:
        os.close(lock)


def test_changed_owner_digest_or_symlink_main_refuses_before_native_check(native_request) -> None:
    request, directory, _ = native_request
    wrong_owner = copy.deepcopy(request)
    wrong_owner["native"]["owner_uid"] += 1
    assert MODULE.remote_check(wrong_owner)["error_code"] == "host_owner_changed"
    wrong_digest = copy.deepcopy(request)
    wrong_digest["candidate_sha256"] = "0" * 64
    assert MODULE.remote_check(wrong_digest)["error_code"] == "candidate_digest_changed"
    main = directory / "nginx.conf"
    main.rename(directory / "private-main")
    main.symlink_to(directory / "private-main")
    assert MODULE.remote_check(request)["exit_code"] == 1
    assert not (directory / "native-completed").exists()
    _assert_clean(directory)


def test_main_replacement_during_native_check_is_detected_and_temporary_files_removed(native_request) -> None:
    request, directory, _ = native_request
    (directory / "replace-main").touch()
    result = MODULE.remote_check(request)
    assert result["error_code"] == "native_identity_changed"
    assert result["validation_files_removed"] is True
    _assert_clean(directory)


@pytest.mark.parametrize("marker", ["mutate-candidate", "mutate-validation-main"])
def test_in_place_stage_mutation_cannot_emit_success_for_original_candidate(native_request, marker) -> None:
    request, directory, _ = native_request
    (directory / marker).touch()
    result = MODULE.remote_check(request)
    assert result["error_code"] == "validation_file_identity_changed"
    assert result["exit_code"] == 1 and result["validation_files_removed"] is True
    _assert_clean(directory)


def test_public_remote_program_runs_same_maintained_owner_protocol(native_request) -> None:
    request, directory, _ = native_request
    result = subprocess.run([sys.executable, "-I", "-"], input=MODULE.remote_program(request),
                            capture_output=True, timeout=20)
    assert result.returncode == 0, result.stderr
    receipt = json.loads(result.stdout)
    assert receipt["exit_code"] == 0 and receipt["validation_files_removed"] is True
    assert b"private-native" not in result.stdout and not result.stderr
    _assert_clean(directory)


def test_controller_refuses_arbitrary_host_text_and_mismatched_candidate_receipts(native_request) -> None:
    request, _, _ = native_request
    receipt = MODULE.remote_check(request)
    plan = dict(host_kind=request["host_kind"], native=request["native"],
                candidate=dict(sha256=request["candidate_sha256"]),
                renderer_source=dict(sha256=request["renderer_source_sha256"]))
    assert MODULE.admit_receipt(receipt, 0, plan) == receipt
    for changed in ({**receipt, "private_body": "must-never-escape"},
                    {**receipt, "candidate_sha256": "0" * 64},
                    {**receipt, "validation_files_removed": False}):
        with pytest.raises(MODULE.CheckError) as caught:
            MODULE.admit_receipt(changed, 0, plan)
        assert "must-never-escape" not in str(caught.value)


def test_closed_plan_preserves_large_native_identities_as_exact_decimal_text(native_request) -> None:
    request, directory, _ = native_request
    reference = dict(path=str(directory / "public.json"), sha256="a" * 64)
    plan = dict(schema=MODULE.PLAN_SCHEMA, provider="macstadium-dublin", host_kind=request["host_kind"],
                deployment_reference=reference, renderer_source=reference,
                candidate=dict(path=str(directory / "candidate.conf"), sha256="b" * 64, owner_uid=os.geteuid()),
                native=request["native"])
    assert MODULE.validate_plan(json.loads(json.dumps(plan))) == plan
    for changed in ({**plan, "provider": "unapproved"}, {**plan, "unknown": True}):
        with pytest.raises(MODULE.CheckError):
            MODULE.validate_plan(changed)
    for value in (1790834296307656104, "01790834296307656104", "1.790834296307656e18"):
        changed = copy.deepcopy(plan)
        changed["native"]["main"]["identity"]["ctime_ns"] = value
        with pytest.raises(MODULE.CheckError):
            MODULE.validate_plan(changed)


def test_retained_lock_refuses_same_inode_permission_drift(native_request):
    request, directory, _ = native_request
    (directory / "mutate-lock-mode").touch()
    result = MODULE.remote_check(request)
    assert result["exit_code"] == 1 and result["error_code"] == "check_lock_identity_changed"
    assert result["validation_files_removed"] is True
    assert stat.S_IMODE((directory / ".taira-native-nginx-check.lock").stat().st_mode) == 0o666
    assert not list(directory.glob(".taira-nginx-check-*"))


def test_declared_public_reader_is_bounded_and_refuses_links_and_unsafe_ancestors(native_request):
    _, directory, _ = native_request
    public = directory / "declared-public.conf"
    body = b"upstream public { server 127.0.0.1:18480; }\n"
    public.write_bytes(body)
    public.chmod(0o600)
    digest = hashlib.sha256(body).hexdigest()
    assert MODULE.read_declared_public_file(str(public), digest, limit=len(body)) == body
    with pytest.raises(MODULE.CheckError, match="unsafe_public_input"):
        MODULE.read_declared_public_file(str(public), digest, limit=len(body)-1)
    with pytest.raises(MODULE.CheckError, match="public_input_digest_changed"):
        MODULE.read_declared_public_file(str(public), "0" * 64)
    linked = directory / "linked-public.conf"
    os.link(public, linked)
    with pytest.raises(MODULE.CheckError, match="unsafe_public_input"):
        MODULE.read_declared_public_file(str(public), digest)
    linked.unlink()
    linked.symlink_to(public)
    with pytest.raises(OSError):
        MODULE.read_declared_public_file(str(linked), digest)
    linked.unlink()
    unsafe = directory / "unsafe"
    unsafe.mkdir(mode=0o777)
    unsafe.chmod(0o777)
    nested = unsafe / "public.conf"
    nested.write_bytes(body)
    with pytest.raises(MODULE.CheckError, match="unsafe_public_ancestor"):
        MODULE.read_declared_public_file(str(nested), digest)


@pytest.mark.parametrize("substitute", [False, True])
def test_declared_public_reader_refuses_inode_or_in_place_drift(native_request, monkeypatch, substitute):
    _, directory, _ = native_request
    public = directory / "declared-public.conf"
    body = b"public input\n"
    public.write_bytes(body)
    public.chmod(0o600)
    native_pread = os.pread
    def changing_read(fd, count, offset):
        result = native_pread(fd, count, offset)
        if substitute:
            foreign = directory / "foreign.conf"
            foreign.write_bytes(body)
            foreign.chmod(0o600)
            os.replace(foreign, public)
        else:
            public.chmod(0o644)
        return result
    monkeypatch.setattr(MODULE.os, "pread", changing_read)
    with pytest.raises(MODULE.CheckError, match="public_input_changed"):
        MODULE.read_declared_public_file(str(public), hashlib.sha256(body).hexdigest())
