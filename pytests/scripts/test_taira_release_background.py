"""Real detached diagnostic processes and flock custody; no Cargo or network.

The fixture replaces only native gate work and Cargo tool discovery. The maintained
launcher, worker, record writer, status reader and lane locks execute unchanged.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import time

import pytest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"


@pytest.fixture
def runner(tmp_path):
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in ("taira_release.py", "release_artifact_contract.py", "taira_cargo_cache.py",
                 "taira_cargo_artifact.py"):
        shutil.copy2(SCRIPTS / name, scripts / ("implementation.py" if name == "taira_release.py" else name))
    # Child re-exec enters the same maintained implementation with tool discovery
    # isolated. This fixture is never exposed by the production command.
    entry = scripts / "taira_release.py"
    entry.write_text("import implementation as release\n"
                     "release.__file__ = __file__\n"
                     "release.isolated_cargo_environment = lambda root, source, env: (env, [])\n"
                     "raise SystemExit(release.main())\n")
    (scripts / "taira_release_check.py").write_text('''
import os
from pathlib import Path
import subprocess
import sys
import time
class CheckError(RuntimeError):
    pass

def focused_regression_stages(scope, focused):
    if focused not in (["core=fixture"], [f"core=fixture-{index}" for index in range(496)]):
        raise CheckError("unknown exact fixture regression")

def wait_for(path):
    deadline = time.monotonic() + 15
    while not path.exists():
        if time.monotonic() > deadline:
            raise CheckError("fixture rendezvous timed out")
        time.sleep(0.02)

def run_checks(root, *, environment, lock_fds, qualification_scope):
    assert len(lock_fds) == 2
    assert "PRIVATE_KEY" not in os.environ and "PRIVATE_KEY" not in environment
    assert "RUSTFLAGS" not in environment
    for fd in lock_fds:
        os.fstat(fd)
    print("fixture native gate started", flush=True)
    (root / "ready").write_text("ready")
    mode = (root / "mode").read_text()
    if mode == "fail":
        raise CheckError("fixture gate failure")
    if mode == "orphan":
        subprocess.Popen([sys.executable, str(Path(__file__).with_name("descendant.py")), str(root)], pass_fds=lock_fds)
        os._exit(17)
    wait_for(root / "continue")
    print("fixture native gate passed", flush=True)

def run_prequalification(root, *, focused_regressions, **kwargs):
    focused_regression_stages(kwargs["qualification_scope"], focused_regressions)
    run_checks(root, **kwargs)
''')
    (scripts / "descendant.py").write_text(
        "from pathlib import Path; import sys; from taira_release_check import wait_for\n"
        "wait_for(Path(sys.argv[1]) / 'continue')\n")
    root = tmp_path / "repo"
    root.mkdir()
    lane = tmp_path / "lane"
    lane.mkdir(mode=0o700)
    (root / "mode").write_text("pass")
    env = dict(os.environ, PRIVATE_KEY="must-never-cross-or-be-recorded", RUSTFLAGS="fixture poison")

    def invoke(*args):
        return subprocess.run([sys.executable, str(entry), *map(str, args)],
                              env=env, text=True, capture_output=True, timeout=10)

    def launch(session, *extra):
        result = invoke("check", "--repo-root", root, "--target-dir", lane,
                        "--native-linker", "system", "--session-dir", session, *extra)
        assert result.returncode == 0, result.stderr
        return result

    def status(session):
        result = invoke("check-status", "--session-dir", session)
        assert result.stdout, result.stderr
        observation = json.loads(result.stdout)
        assert set(observation) == {"schema", "session_id", "state", "request_path",
                                    "focused_regression_count", "result", "log",
                                    "release_qualified"}
        assert observation["request_path"] == str(session / "request.json")
        assert type(observation["focused_regression_count"]) is int
        return result.returncode, observation

    yield root, lane, invoke, launch, status
    # Never kill or restart a worker, even on assertion failure. Release the
    # fixture rendezvous and let its own bounded deadline/process exit run.
    (root / "continue").touch()


def eventually(operation, predicate):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        value = operation()
        if predicate(value):
            return value
        time.sleep(0.03)
    pytest.fail(f"fixture did not reach expected state; last observation: {value}")


def test_detached_check_survives_launcher_exit_and_keeps_real_lane_lock(runner, tmp_path):
    root, lane, invoke, launch, status = runner
    session = tmp_path / "session"
    launch(session, "--focus-regression", "core=fixture")
    eventually(lambda: (root / "ready").exists(), bool)
    code, observation = status(session)
    assert code == 2 and observation["state"] == "running"
    assert observation["release_qualified"] is False
    assert observation["focused_regression_count"] == 1
    assert stat.S_IMODE(session.stat().st_mode) == 0o700
    assert stat.S_IMODE((session / "check.log").stat().st_mode) == 0o600
    assert stat.S_IMODE((session / "request.json").stat().st_mode) == 0o400
    assert stat.S_IMODE((session / "started.json").stat().st_mode) == 0o400
    assert "must-never-cross" not in (session / "request.json").read_text()
    assert not (session / "environment.json").exists()
    competitor = tmp_path / "competitor"
    launch(competitor)
    code, failed = eventually(lambda: status(competitor), lambda row: row[1]["state"] != "running")
    assert code == 1 and failed["state"] == "failed"
    assert "still running" in (competitor / "check.log").read_text()
    (root / "continue").touch()
    code, passed = eventually(lambda: status(session), lambda row: row[1]["state"] != "running")
    assert code == 0 and passed["state"] == "passed"
    assert passed["result"]["exit_code"] == 0
    assert stat.S_IMODE((session / "result.json").stat().st_mode) == 0o400
    assert "fixture native gate passed" in (session / "check.log").read_text()
    assert not (session / "checks.json").exists()
    # Re-reading does not invoke a gate, mutate records or restart completed work.
    before = {p.name: p.read_bytes() for p in session.iterdir()}
    assert status(session) == (code, passed)
    assert {p.name: p.read_bytes() for p in session.iterdir()} == before


def test_status_keeps_large_focus_request_in_immutable_record(runner, tmp_path):
    root, _, _, launch, status = runner
    session = tmp_path / "large-focus"
    focused = [f"core=fixture-{index}" for index in range(496)]
    launch(session, *(argument for name in focused for argument in ("--focus-regression", name)))
    eventually(lambda: (root / "ready").exists(), bool)
    request_path = session / "request.json"
    original = request_path.read_bytes()
    assert json.loads(original)["focused_regressions"] == focused
    code, running = status(session)
    assert code == 2 and running["state"] == "running"
    (root / "continue").touch()
    code, passed = eventually(lambda: status(session), lambda row: row[1]["state"] != "running")
    assert code == 0 and passed["state"] == "passed"
    for observation in (running, passed):
        assert observation["focused_regression_count"] == len(focused)
        assert observation["request_path"] == str(request_path)
        encoded = json.dumps(observation)
        assert all(name not in encoded for name in focused)
        assert len(encoded) < len(original) // 4
    assert request_path.read_bytes() == original
    assert stat.S_IMODE(request_path.stat().st_mode) == 0o400


def test_failed_gate_has_durable_failure(runner, tmp_path):
    root, _, _, launch, status = runner
    (root / "mode").write_text("fail")
    session = tmp_path / "failed"
    launch(session)
    code, observation = eventually(lambda: status(session), lambda row: row[1]["state"] != "running")
    assert code == 1 and observation["state"] == "failed"
    assert observation["focused_regression_count"] == 0
    assert observation["result"]["error_type"] == "PrepareError"
    assert "fixture gate failure" in (session / "check.log").read_text()


def test_worker_exit_retains_descendant_locks_then_reports_incomplete(runner, tmp_path):
    root, _, _, launch, status = runner
    (root / "mode").write_text("orphan")
    session = tmp_path / "orphan"
    launch(session)
    eventually(lambda: (root / "ready").exists(), bool)
    # Give the worker time to self-exit; the spawned fixture process still owns
    # both inherited locks. No PID liveness probe or process signal is used.
    time.sleep(0.2)
    assert status(session)[1]["state"] == "running"
    (root / "mode").write_text("pass")
    competitor = tmp_path / "competitor"
    launch(competitor)
    _, failed = eventually(lambda: status(competitor), lambda row: row[1]["state"] != "running")
    assert failed["state"] == "failed"
    assert "still running" in (competitor / "check.log").read_text()
    (root / "continue").touch()
    code, observation = eventually(lambda: status(session), lambda row: row[1]["state"] != "running")
    assert code == 1 and observation["state"] == "incomplete"
    assert observation["result"] is None
    assert not (session / "result.json").exists()
    # Fresh explicit invocation is allowed once the actual descendant retires.
    fresh = tmp_path / "fresh"
    launch(fresh)
    _, passed = eventually(lambda: status(fresh), lambda row: row[1]["state"] != "running")
    assert passed["state"] == "passed"


@pytest.mark.parametrize("kind", ["existing", "symlink", "relative"])
def test_session_path_must_be_fresh_absolute_and_not_symlinked(runner, tmp_path, kind):
    root, lane, invoke, _, _ = runner
    session = tmp_path / "session"
    if kind == "existing":
        session.mkdir(mode=0o700)
    elif kind == "symlink":
        session.symlink_to(root, target_is_directory=True)
    else:
        session = Path("relative-session")
    result = invoke("check", "--repo-root", root, "--target-dir", lane,
                    "--native-linker", "system", "--session-dir", session)
    assert result.returncode == 1
    assert not (root / "ready").exists()


def test_status_rejects_wrong_result_identity_and_insecure_directory(runner, tmp_path):
    root, _, invoke, launch, status = runner
    (root / "continue").touch()
    session = tmp_path / "session"
    launch(session)
    eventually(lambda: status(session), lambda row: row[1]["state"] == "passed")
    path = session / "result.json"
    result = json.loads(path.read_text())
    result["session_id"] = "f" * 32
    path.chmod(0o600)
    path.write_text(json.dumps(result, sort_keys=True, indent=2, ensure_ascii=True) + "\n")
    path.chmod(0o400)
    rejected = invoke("check-status", "--session-dir", session)
    assert rejected.returncode == 1 and "invalid development check result" in rejected.stderr
    session.chmod(0o755)
    rejected = invoke("check-status", "--session-dir", session)
    assert rejected.returncode == 1 and "owner-private" in rejected.stderr


def test_worker_rejects_independent_descriptor_while_original_holder_is_running(runner, tmp_path):
    root, _, _, launch, status = runner
    session = tmp_path / "session"
    launch(session)
    eventually(lambda: (root / "ready").exists(), bool)
    # This descriptor names the right inode, but does not share the launcher's
    # locked open-file description. It cannot impersonate the actual worker.
    with (session / "session.lock").open("r+b") as lock:
        result = subprocess.run(
            [sys.executable, str(tmp_path / "scripts/taira_release.py"), "_check-runner",
             "--session-dir", str(session), "--session-fd", str(lock.fileno())],
            pass_fds=(lock.fileno(),), text=True, capture_output=True, timeout=5,
        )
    assert result.returncode == 1
    assert "does not own its inherited lock" in result.stderr
    assert status(session)[1]["state"] == "running"
    (root / "continue").touch()
    eventually(lambda: status(session), lambda row: row[1]["state"] == "passed")


def test_completed_session_cannot_be_restarted_by_worker_entrypoint(runner, tmp_path):
    root, _, _, launch, status = runner
    (root / "continue").touch()
    session = tmp_path / "session"
    launch(session)
    eventually(lambda: status(session), lambda row: row[1]["state"] == "passed")
    original = (session / "result.json").read_bytes()
    with (session / "session.lock").open("r+b") as lock:
        result = subprocess.run(
            [sys.executable, str(tmp_path / "scripts/taira_release.py"), "_check-runner",
             "--session-dir", str(session), "--session-fd", str(lock.fileno())],
            pass_fds=(lock.fileno(),), text=True, capture_output=True, timeout=5,
        )
    assert result.returncode == 1
    assert "requires its inherited session lock" in result.stderr
    assert (session / "result.json").read_bytes() == original
