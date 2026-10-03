"""Exercise runner failure handling with subprocess fixtures, not proof evidence."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from contextlib import contextmanager
from pathlib import Path
import subprocess
import sys
import tempfile

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_ivm_native_frame_equations.py"
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("native_frame_runner", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


@pytest.fixture
def workspace():
    """Keep fixture processes and every output inside the repository target tree."""
    (ROOT / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="native-frame-runner-") as directory:
        yield Path(directory)


def fixture_binary(workspace: Path, name: str, *, fault: str = "") -> Path:
    """Emit deterministic libtest-shaped tooling output; no AIR is executed."""
    path = workspace / name
    path.write_text(
        "#!" + sys.executable + "\n"
        + f"fault = {fault!r}\n"
        + """
import json, os, pathlib, sys, time
test = sys.argv[2]
assert sys.argv[1:] == ['--exact', test, '--ignored', '--nocapture', '--test-threads=1']
if fault == 'timeout':
    time.sleep(5)
if fault == 'overflow':
    print('x' * (21 * 1024 * 1024), flush=True)
if fault == 'exit':
    sys.exit(7)
if fault == 'zero':
    print('running 0 tests\\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s')
    sys.exit(0)
if fault == 'mutation':
    source = pathlib.Path(os.environ['MUTATE_BINARY'])
    source.write_text(source.read_text() + '\\n# changed during execution\\n')
print('running 1 test')
print('test ' + test + ' ... ', end='')
if 'capture_actual_native_frame_owner_equations' in test:
    marker = 'IVM_NATIVE_FRAME_OWNER_CAPTURE='
    value = {'schema': 'ivm.native-frame-owner-equations.v1', 'cells': 4097,
             'cases': [{'result_words': words, 'shift': shift, 'root_return': root}
                       for words in (1, 2, 8192) for shift in (0, 8) for root in (True, False)]}
elif 'capture_native_runtime_frame_equations' in test:
    marker = 'IVM_NATIVE_CALL_RUNTIME_CAPTURE='
    value = {'schema': 'ivm.native-call-runtime-equations.v1', 'program_bytes': [73],
             'native_program_result': 1, 'root_entry': {}, 'child_entry': {},
             'root_return': {}, 'child_return': {}}
else:
    marker = None
    for variable in ('IROHA_IVM_NATIVE_FRAME_OWNER_CAPTURE', 'IROHA_IVM_NATIVE_CALL_RUNTIME_CAPTURE'):
        capture = pathlib.Path(os.environ[variable])
        assert capture.parent == pathlib.Path(sys.argv[0]).parent
        assert json.loads(capture.read_bytes())['schema'].startswith('ivm.native-')
        if fault == 'capture_mutation':
            capture.write_bytes(capture.read_bytes() + b' ')
if marker is not None:
    payload = json.dumps(value, separators=(',', ':'))
    if fault == 'invalid_json': payload = '{bad'
    if fault == 'duplicate_key': payload = '{"schema":"x","schema":"y"}'
    if fault == 'nonfinite': payload = '{"number":NaN}'
    if fault == 'float': payload = '{"number":1e999}'
    if fault == 'wrong_schema': payload = '{"schema":"old"}'
    if fault == 'trailing_json': payload += '{}'
    if fault == 'geometry':
        value['cases'][0] = value['cases'][1]
        payload = json.dumps(value)
    if fault != 'missing_marker': print(marker + payload)
    if fault == 'duplicate_marker': print(marker + payload)
print('ok')
count = 2 if fault == 'count' else 1
ignored = 1 if fault == 'ignored' else 0
print(f'test result: ok. {count} passed; 0 failed; {ignored} ignored; 0 measured; 17 filtered out; finished in 0.00s')
if fault == 'double_summary':
    print('test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.00s')
""",
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path


def run_tool(workspace: Path, *, producer_fault: str = "", consumer_fault: str = "", output: Path | None = None):
    ivm = fixture_binary(workspace, "ivm", fault=producer_fault)
    privacy = fixture_binary(workspace, "privacy", fault=consumer_fault)
    destination = output or workspace / "output"
    environment = os.environ.copy()
    environment.update({
        "PYTHONDONTWRITEBYTECODE": "1",
        "MUTATE_BINARY": str(ivm),
        "IROHA_IVM_NATIVE_FRAME_OWNER_CAPTURE": "must-not-use-inherited-capture",
        "IROHA_IVM_NATIVE_CALL_RUNTIME_CAPTURE": "must-not-use-inherited-capture",
    })
    return subprocess.run(
        [sys.executable, "-B", str(SCRIPT), "--ivm-test-binary", str(ivm),
         "--privacy-test-binary", str(privacy), "--output-dir", str(destination),
         "--test-timeout-seconds", "1"],
        cwd=ROOT, env=environment, capture_output=True, text=True, timeout=15, check=False,
    )


def test_fresh_outputs_exact_tests_hashes_and_private_custody(workspace: Path):
    result = run_tool(workspace)
    assert result.returncode == 0, result.stderr
    output = workspace / "output"
    assert output.stat().st_mode & 0o777 == 0o700
    receipt = json.loads((output / "receipt.json").read_bytes())
    assert len(receipt["tests"]) == 6
    assert receipt["totals"] == {"producers_passed": 2, "consumers_passed": 4, "failed": 0, "ignored": 0}
    assert receipt["complete_invocation_proof"] is False
    assert receipt["source_build_provenance"].startswith("unverified;")
    assert [test["test"] for test in receipt["tests"]] == [p[0] for p in runner.PRODUCERS] + list(runner.CONSUMERS)
    for artifact in receipt["binaries"] + receipt["captures"] + [test["log"] for test in receipt["tests"]]:
        data = (output / artifact["path"]).read_bytes()
        assert len(data) == artifact["size"]
        assert hashlib.sha256(data).hexdigest() == artifact["sha256"]
    for binary in receipt["binaries"]:
        assert (output / binary["path"]).resolve() != ROOT / binary["source_path"]
        assert hashlib.sha256((ROOT / binary["source_path"]).read_bytes()).hexdigest() == binary["sha256"]
    for path in output.iterdir():
        assert path.stat().st_mode & 0o777 == (0o755 if path.name.endswith("-binary") else 0o600)


@pytest.mark.parametrize("fault", [
    "exit", "zero", "count", "ignored", "double_summary", "invalid_json", "duplicate_key",
    "nonfinite", "float", "wrong_schema", "trailing_json", "geometry", "missing_marker",
    "duplicate_marker", "timeout", "overflow", "mutation",
])
def test_producer_failures_never_publish_success(workspace: Path, fault: str):
    result = run_tool(workspace, producer_fault=fault)
    assert result.returncode != 0, result.stdout
    reason = {
        "exit": "exit 7", "zero": "one passing test", "count": "one passing test",
        "ignored": "one passing test", "double_summary": "one passing test",
        "invalid_json": "valid UTF-8 JSON", "duplicate_key": "duplicate key",
        "nonfinite": "finite integer", "float": "finite integer", "wrong_schema": "wrong schema",
        "trailing_json": "valid UTF-8 JSON", "geometry": "geometry is incomplete",
        "missing_marker": "one capture marker", "duplicate_marker": "one capture marker",
        "timeout": "exceeded 1s", "overflow": "output exceeds byte limit",
        "mutation": "changed during execution",
    }[fault]
    assert reason in result.stderr, result.stderr
    assert not (workspace / "output" / "receipt.json").exists()


@pytest.mark.parametrize("fault", ["exit", "zero", "count", "ignored", "capture_mutation"])
def test_consumer_failures_never_publish_success(workspace: Path, fault: str):
    result = run_tool(workspace, consumer_fault=fault)
    assert result.returncode != 0, result.stdout
    expected = "changed during execution" if fault == "capture_mutation" else "one passing test"
    assert expected in result.stderr, result.stderr
    assert (workspace / "output" / "native-call-runtime-capture.json").exists()
    assert not (workspace / "output" / "receipt.json").exists()


def test_refuses_existing_output_without_overwriting(workspace: Path):
    output = workspace / "output"
    output.mkdir()
    sentinel = output / "receipt.json"
    sentinel.write_bytes(b"existing")
    result = run_tool(workspace)
    assert result.returncode != 0
    assert sentinel.read_bytes() == b"existing"
    assert sorted(p.name for p in output.iterdir()) == ["receipt.json"]


def test_refuses_symlink_output_ancestor(workspace: Path):
    actual = workspace / "actual"
    actual.mkdir()
    link = workspace / "link"
    link.symlink_to(actual, target_is_directory=True)
    result = run_tool(workspace, output=link / "output")
    assert result.returncode != 0
    assert not (actual / "output").exists()


def test_refuses_output_outside_target_without_creating_it(workspace: Path):
    outside = ROOT / f"{workspace.name}-must-not-create"
    result = run_tool(workspace, output=outside)
    assert result.returncode != 0
    assert not outside.exists()


def test_refuses_parent_traversal_before_creating_output(workspace: Path):
    result = run_tool(workspace, output=workspace / "missing" / ".." / "output")
    assert result.returncode != 0
    assert not (workspace / "missing").exists()
    assert not (workspace / "output").exists()


@pytest.mark.parametrize("replace_ancestor", [False, True])
def test_directory_replace_execute_restore_is_rejected(workspace: Path, monkeypatch, replace_ancestor: bool):
    ivm = fixture_binary(workspace, "ivm")
    privacy = fixture_binary(workspace, "privacy")
    output = workspace / "ancestor" / "output"
    parked = workspace / "parked-original"
    replacement = workspace / "replacement"
    executed = workspace / "alternate-executed"
    real_popen = runner.subprocess.Popen

    def swapped_popen(command, **kwargs):
        replaced = output.parent if replace_ancestor else output
        replaced.rename(parked)
        output.mkdir(mode=0o700, parents=True)
        path = output / Path(command[0]).name
        path.write_text(ivm.read_text().replace(
            "import json, os, pathlib, sys, time",
            f"import json, os, pathlib, sys, time\npathlib.Path({str(executed)!r}).write_text('yes')",
        ))
        path.chmod(0o755)
        child = real_popen(command, **kwargs)
        # Fixture output fits the pipe; wait until the alternate image really
        # ran before restoring every original file and its unchanged identity.
        child.wait(timeout=5)
        replaced.rename(replacement)
        parked.rename(replaced)
        return child

    monkeypatch.setattr(runner.subprocess, "Popen", swapped_popen)
    with pytest.raises(runner.ReleaseArtifactError, match="directory changed during launch"):
        runner.validate(ivm, privacy, output, 5)
    assert executed.read_text() == "yes"
    assert not (output / "receipt.json").exists()


def test_replaced_publication_parent_cannot_retain_success_receipt(workspace: Path, monkeypatch):
    ivm = fixture_binary(workspace, "ivm")
    privacy = fixture_binary(workspace, "privacy")
    output = workspace / "output"
    parked = workspace / "original-output"
    replacement = workspace / "foreign-output"
    real_output_fd = runner.exclusive_output_fd

    @contextmanager
    def substitute_publication(path, **kwargs):
        if path.name != "receipt.json":
            with real_output_fd(path, **kwargs) as fd:
                yield fd
            return
        output.rename(parked)
        output.mkdir(mode=0o700)
        try:
            with real_output_fd(path, **kwargs) as fd:
                yield fd
        finally:
            output.rename(replacement)
            parked.rename(output)

    monkeypatch.setattr(runner, "exclusive_output_fd", substitute_publication)
    with pytest.raises(runner.ReleaseArtifactError, match="launch-path directory was replaced"):
        runner.validate(ivm, privacy, output, 5)
    assert not (output / "receipt.json").exists()
    assert not (replacement / "receipt.json").exists()
