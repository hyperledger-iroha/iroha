"""The actual Rust bootstrap's read-only native owner-input action.

Four maintained sources and the public request are inherited real descriptors
in an isolated Python child. Only master observation is stubbed against a
separate public fixture snapshot; the master PID belongs to an owned signal
observer child. This proves finite native-context/custody behavior, not signed
Rust-parent authority or production readiness. No live/private files are read.
"""
from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
import re
import select
import subprocess
import sys

import pytest

from taira_native_edge_completion_test import MODULE as NATIVE, _plan
from taira_native_nginx_apply_test import MODULE as OWNER, _identity, native_apply


RUST_PATH = Path(__file__).resolve().parents[2] / "crates/iroha_cli/src/taira_public_reset_native_edge.rs"
pytestmark = pytest.mark.skipif(sys.platform != "darwin",
    reason="The native owner runtime admission requires the actual Darwin kernel image")


def _actual_bootstrap():
    source = RUST_PATH.read_text()
    match = re.search(r'const BOOTSTRAP: &str = r#"(.*?)"#;', source, re.DOTALL)
    assert match is not None
    sources = source.split("const SOURCES:", 1)[1].split("const BOOTSTRAP:", 1)[0]
    paths = [RUST_PATH.parent / path for path in re.findall(r'include_bytes!\("([^\"]+)"\)', sources)]
    assert [path.name for path in paths] == ["taira_native_nginx_check.py", "taira_native_nginx_apply.py",
        "taira_native_validator_forwarding.py", "taira_native_edge_completion.py"]
    assert "elif action == 'validate-owner-inputs':" in match[1]
    return match[1].encode(), [path.resolve() for path in paths]


def _write_public(path, body):
    path.write_bytes(body)
    path.chmod(0o600)


@pytest.fixture
def bootstrap_context(native_apply):
    request, root, _ = native_apply
    original = OWNER.remote_apply(request)
    assert original["exit_code"] == 0, original
    marker = root / "owned-master-hup-observed"
    observer_program = """import os, signal, sys, time
def observed(*_):
    fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try: os.write(fd, b'owned master received HUP\\n')
    finally: os.close(fd)
signal.signal(signal.SIGHUP, observed)
print('ready', flush=True)
time.sleep(90)
"""
    observer = subprocess.Popen([sys.executable, "-I", "-c", observer_program, str(marker)],
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    try:
        ready, _, _ = select.select([observer.stdout], [], [], 5)
        assert ready and observer.stdout.readline(32) == b"ready\n"
        request = copy.deepcopy(request)
        request["master"]["pid"] = observer.pid
        request["operation_id"] = "b" * 32
        plan = _plan(request, root)
        plan["renderer_source"]["sha256"] = hashlib.sha256(Path(plan["renderer_source"]["path"]).read_bytes()).hexdigest()
        # Read-only owner-input validation must not decode or rewrite opaque
        # existing incident history, even though its include is still serving.
        journal = root / (".taira-native-nginx-apply-" + "a" * 32 + ".receipt.ndjson")
        _write_public(journal, b"opaque fixture history that is deliberately not JSON\n")
        yield plan, root, observer, marker
    finally:
        if observer.poll() is None:
            observer.terminate()
        observer.wait(timeout=5)
        observer.stdout.close()


def _run_actual_bootstrap(plan, root, observed_master, *, master_changes_after_first=False,
        omit_helper_descriptor=False):
    bootstrap, sources = _actual_bootstrap()
    # Pin the actual Darwin kernel image, rather than a Homebrew bin launcher.
    # The bootstrap itself checks this real image, retained FD, SHA and version;
    # none of that capability admission is mocked by the isolated master hook.
    runtime_path = Path(NATIVE.kernel_executable_path(os.getpid()))
    runtime = dict(schema="iroha.taira.public-reset.native-python-runtime.v1",
        executable=dict(file=dict(path=str(runtime_path), identity=NATIVE.identity(runtime_path.stat())),
            sha256=hashlib.sha256(runtime_path.read_bytes()).hexdigest()), version=list(sys.version_info[:3]))
    capsule = root / "bootstrap-capsule"
    capsule.mkdir(mode=0o700)
    bootstrap_path, request_path, observed_path = (capsule / name for name in
        ("actual-bootstrap.py", "public-plan.json", "isolated-master-observation.json"))
    _write_public(bootstrap_path, bootstrap)
    _write_public(request_path, json.dumps(plan, sort_keys=True, separators=(",", ":")).encode())
    _write_public(observed_path, json.dumps(dict(master=observed_master,
        changes_after_first=master_changes_after_first), sort_keys=True, separators=(",", ":")).encode())
    paths = [bootstrap_path, observed_path, request_path, runtime_path, *sources]
    descriptors = []
    program = """import builtins, json, os, sys
bootstrap_fd, observed_fd = int(sys.argv[1]), int(sys.argv[2])
bootstrap = os.pread(bootstrap_fd, 1048577, 0)
observation = json.loads(os.pread(observed_fd, 65537, 0))
sys.argv = ['isolated-native-bootstrap', *sys.argv[3:]]
def execute_source(code, namespace):
    builtins.exec(code, namespace)
    if namespace.get('__name__') == 'taira_native_nginx_apply':
        calls = [0]
        def observe_master(_expected):
            calls[0] += 1
            master = dict(observation['master'])
            if observation['changes_after_first'] and calls[0] > 1:
                master['started'] = 'Sun Oct 4 02:03:04 2026'
            return master
        namespace['observe_master'] = observe_master
builtins.exec(compile(bootstrap, '<actual Rust BOOTSTRAP>', 'exec'),
    {'__name__': '__main__', 'exec': execute_source})
"""
    try:
        for path in paths:
            descriptors.append(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC))
        before = [_identity(path) for path in sources]
        helper_descriptors = descriptors[4:-1] if omit_helper_descriptor else descriptors[4:]
        result = subprocess.run([str(runtime_path), "-B", "-I", "-c", program,
            str(descriptors[0]), str(descriptors[1]), str(capsule), "validate-owner-inputs",
            str(descriptors[2]), json.dumps(runtime, sort_keys=True, separators=(",", ":")),
            str(descriptors[3]), *(str(fd) for fd in helper_descriptors)],
            pass_fds=[*descriptors[:4], *helper_descriptors],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
        assert [_identity(path) for path in sources] == before
        assert _actual_bootstrap()[0] == bootstrap
        return result
    finally:
        for fd in descriptors:
            os.close(fd)


def _serving_snapshot(root):
    paths = [root / "conf.d/new-scoped.conf", root / "reload-count",
        *sorted(root.glob(".taira-native-nginx-apply-*.receipt.ndjson")),
        *sorted(root.glob(".taira-native-nginx-write-*.intent.json"))]
    return {path: (path.read_bytes(), _identity(path)) for path in paths}


def _assert_no_effect(root, observer, marker, before):
    assert _serving_snapshot(root) == before
    assert observer.poll() is None and not marker.exists()
    assert not list(root.glob(".taira-nginx-check-*"))
    assert not list((root / "conf.d").glob(".taira-nginx-publish-*"))
    assert not (root / (".taira-native-nginx-apply-" + "b" * 32 + ".receipt.ndjson")).exists()


def test_actual_bootstrap_owner_inputs_checks_context_without_publication_decode_or_hup(bootstrap_context):
    plan, root, observer, marker = bootstrap_context
    before = _serving_snapshot(root)
    result = _run_actual_bootstrap(plan, root, copy.deepcopy(plan["master"]))
    assert result.returncode == 0, result.stderr.decode()
    assert result.stdout == b"{}\n" and result.stderr == b""
    _assert_no_effect(root, observer, marker, before)


@pytest.mark.parametrize("fault,code", [
    ("malformed_master", "master_identity"),
    ("stale_master", "native_owner_inputs_refused"),
    ("master_changes_after_context", "native_owner_inputs_refused"),
    ("malformed_native", "native_identity"),
    ("stale_native", "native_owner_inputs_refused"),
    ("missing_native_include_source", "native_owner_inputs_refused"),
    ("missing_helper_descriptor", "zip() argument 2 is shorter than argument 1"),
    ("stale_renderer", "public_input_digest_changed"),
])
def test_actual_bootstrap_owner_inputs_refuses_faults_without_effect(bootstrap_context, fault, code):
    plan, root, observer, marker = bootstrap_context
    observed_master = copy.deepcopy(plan["master"])
    if fault == "malformed_master":
        plan["master"]["pid"] = 1
    elif fault == "stale_master":
        plan["master"]["started"] = "Sun Oct 4 02:03:04 2026"
    elif fault == "malformed_native":
        plan["native"]["main"]["identity"]["size"] = 1
    elif fault == "stale_native":
        plan["native"]["nginx"]["identity"]["mtime_ns"] = str(
            int(plan["native"]["nginx"]["identity"]["mtime_ns"]) - 1)
    elif fault == "missing_native_include_source":
        (root / "omit-source").touch()
    elif fault == "stale_renderer":
        plan["renderer_source"]["sha256"] = "9" * 64
    before = _serving_snapshot(root)
    result = _run_actual_bootstrap(plan, root, observed_master,
        master_changes_after_first=fault == "master_changes_after_context",
        omit_helper_descriptor=fault == "missing_helper_descriptor")
    assert result.returncode != 0 and result.stdout == b""
    assert code.encode() in result.stderr
    assert b"private-native-error" not in result.stderr and b"private-existing-config-body" not in result.stderr
    _assert_no_effect(root, observer, marker, before)
