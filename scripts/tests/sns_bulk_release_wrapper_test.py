"""Tests for the safe declarative alias release wrapper."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "sns_bulk_release.sh"


def _run(*arguments: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(SCRIPT), *arguments],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
        env=env,
    )


def test_wrapper_help_only_documents_typed_plan_and_local_apply() -> None:
    result = _run("--help")

    assert result.returncode == 0
    assert "--intent PATH" in result.stdout
    assert "--apply" in result.stdout
    assert "one normal" in result.stdout
    for retired in (
        "--csv",
        "--manifest",
        "--submission-log",
        "--submit-token",
        "--torii-url",
        "--suffix-map",
    ):
        assert f"  {retired}" not in result.stdout


def test_wrapper_rejects_raw_secret_and_unknown_options_without_reflection() -> None:
    secret = "do-not-reflect-this-token"
    result = _run(f"--token={secret}")

    assert result.returncode != 0
    assert "raw token" in result.stderr
    assert secret not in result.stderr

    unknown = _run("--submission-log", "/tmp/private-result")
    assert unknown.returncode != 0
    assert "unsupported command-line argument" in unknown.stderr
    assert "/tmp/private-result" not in unknown.stderr


def _fake_python(path: Path) -> Path:
    executable = path / "fake-python"
    executable.write_text(
        """#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

capture = Path(os.environ["ALIAS_WRAPPER_CAPTURE"])
with capture.open("a", encoding="utf-8") as stream:
    stream.write(json.dumps(sys.argv[1:]) + "\\n")

if sys.argv[1].endswith("sns_bulk_onboard.py"):
    arguments = sys.argv[2:]
    plan_path = Path(arguments[arguments.index("--plan-file") + 1])
    plan_path.parent.mkdir(parents=True, exist_ok=True)
    plan_path.write_text("{}\\n", encoding="utf-8")
elif sys.argv[1].endswith("sns_bulk_metrics.py"):
    arguments = sys.argv[2:]
    output_path = Path(arguments[arguments.index("--output") + 1])
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("# safe plan metrics\\n", encoding="utf-8")
elif sys.argv[1] == "-":
    Path(sys.argv[2]).write_text("{}\\n", encoding="utf-8")
else:
    raise SystemExit(9)
""",
        encoding="utf-8",
    )
    executable.chmod(0o700)
    return executable


def test_wrapper_plans_by_default_and_apply_is_explicit(tmp_path: Path) -> None:
    intent = tmp_path / "intent.json"
    intent.write_text('{"schema_version":1,"intents":[]}\n', encoding="utf-8")
    capture = tmp_path / "calls.ndjson"
    fake_python = _fake_python(tmp_path)
    environment = dict(os.environ)
    environment.update(
        {
            "PYTHON": str(fake_python),
            "ALIAS_WRAPPER_CAPTURE": str(capture),
        }
    )

    planned = _run(
        "--intent",
        str(intent),
        "--release-dir",
        str(tmp_path / "releases"),
        "--release-name",
        "planned",
        env=environment,
    )
    assert planned.returncode == 0, planned.stderr

    applied = _run(
        "--intent",
        str(intent),
        "--release-dir",
        str(tmp_path / "releases"),
        "--release-name",
        "applied",
        "--apply",
        env=environment,
    )
    assert applied.returncode == 0, applied.stderr

    calls = [json.loads(line) for line in capture.read_text(encoding="utf-8").splitlines()]
    onboarding_calls = [call for call in calls if call[0].endswith("sns_bulk_onboard.py")]
    assert len(onboarding_calls) == 2
    assert "--apply" not in onboarding_calls[0]
    assert "--apply" in onboarding_calls[1]
    assert all("--plan-only" not in call for call in onboarding_calls)
    assert all("token" not in " ".join(call).lower() for call in calls)



def test_wrapper_starts_planner_and_metrics_in_its_checkout_from_another_cwd(
    tmp_path: Path, monkeypatch
) -> None:
    intent = tmp_path / "intent.json"
    intent.write_text('{"schema_version":1,"intents":[]}\n', encoding="utf-8")
    calls = tmp_path / "calls.ndjson"
    observed = tmp_path / "child-cwd.ndjson"
    fake_python = _fake_python(tmp_path)
    body = fake_python.read_text(encoding="utf-8")
    anchor = 'capture = Path(os.environ["ALIAS_WRAPPER_CAPTURE"])'
    assert body.count(anchor) == 1
    observation = """cwd_capture = Path(os.environ["ALIAS_WRAPPER_CWD_CAPTURE"])
with cwd_capture.open("a", encoding="utf-8") as stream:
    stream.write(json.dumps({"argv": sys.argv[1:], "cwd": str(Path.cwd()),
        "script_exists": Path(sys.argv[1]).is_file()}) + "\\n")

"""
    fake_python.write_text(body.replace(anchor, observation + anchor), encoding="utf-8")
    environment = dict(os.environ, PYTHON=str(fake_python),
        ALIAS_WRAPPER_CAPTURE=str(calls), ALIAS_WRAPPER_CWD_CAPTURE=str(observed))
    caller = tmp_path / "another-caller-directory"
    caller.mkdir()
    monkeypatch.chdir(caller)
    result = _run("--intent", str(intent), "--release-dir", str(tmp_path / "releases"),
        "--release-name", "cwd-regression", env=environment)
    assert result.returncode == 0, result.stderr
    rows = [json.loads(line) for line in observed.read_text(encoding="utf-8").splitlines()]
    children = [row for row in rows if row["argv"][0] != "-"]
    assert [row["argv"][0] for row in children] == [
        "scripts/sns_bulk_onboard.py", "scripts/sns_bulk_metrics.py"]
    expected_checkout = str(SCRIPT.resolve().parent.parent)
    assert all(row["cwd"] == expected_checkout for row in children), children
    assert all(row["script_exists"] for row in children), children
    assert "--apply" not in children[0]["argv"]
    assert len(rows) == 3
    assert (tmp_path / "releases/cwd-regression/summary.json").is_file()



def _path_recording_fake_python(path: Path) -> Path:
    executable = _fake_python(path)
    body = executable.read_text(encoding="utf-8")
    anchor = 'capture = Path(os.environ["ALIAS_WRAPPER_CAPTURE"])'
    assert body.count(anchor) == 1
    observation = """arguments = sys.argv[1:]
inputs = {}
if arguments[0].endswith("sns_bulk_onboard.py"):
    inputs["intent"] = Path(arguments[1]).is_file()
    if "--config" in arguments:
        inputs["config"] = Path(arguments[arguments.index("--config") + 1]).is_file()
    cli = arguments[arguments.index("--iroha-cli") + 1]
    if "/" in cli:
        inputs["cli"] = Path(cli).is_file()
elif arguments[0].endswith("sns_bulk_metrics.py"):
    inputs["plan"] = Path(arguments[arguments.index("--plan") + 1]).is_file()
observed = Path(os.environ["ALIAS_WRAPPER_PATH_CAPTURE"])
with observed.open("a", encoding="utf-8") as stream:
    stream.write(json.dumps({"argv": arguments, "cwd": str(Path.cwd()),
        "script_exists": Path(arguments[0]).is_file(), "inputs_exist": inputs}) + "\\n")
if not all(inputs.values()):
    raise SystemExit(19)

"""
    executable.write_text(body.replace(anchor, observation + anchor), encoding="utf-8")
    return executable


def test_wrapper_resolves_relative_files_and_cli_from_the_callers_directory(
    tmp_path: Path, monkeypatch
) -> None:
    caller = tmp_path / "caller with spaces"
    caller.mkdir()
    (caller / "inputs").mkdir()
    (caller / "inputs/intent.json").write_text('{"schema_version":1,"intents":[]}\n', encoding="utf-8")
    (caller / "settings with spaces").mkdir()
    (caller / "settings with spaces/client.toml").write_text("# synthetic config\n", encoding="utf-8")
    (caller / "bin").mkdir()
    (caller / "bin/iroha stub").write_text("# synthetic unexecuted CLI\n", encoding="utf-8")
    observed = tmp_path / "relative-paths.ndjson"
    fake_python = _path_recording_fake_python(tmp_path)
    environment = dict(os.environ, PYTHON=str(fake_python),
        ALIAS_WRAPPER_CAPTURE=str(tmp_path / "calls.ndjson"),
        ALIAS_WRAPPER_PATH_CAPTURE=str(observed))
    monkeypatch.chdir(caller)
    result = _run("--intent", "inputs/intent.json", "--release-dir", "release output",
        "--release-name", "relative", "--plan-file", "reports/plan.json",
        "--metrics", "reports/metrics.prom", "--summary", "reports/summary.json",
        "--config", "settings with spaces/client.toml", "--iroha-cli", "bin/iroha stub",
        env=environment)
    assert result.returncode == 0, result.stderr
    rows = [json.loads(line) for line in observed.read_text(encoding="utf-8").splitlines()]
    assert len(rows) == 3
    planner, metrics, summary = [row["argv"] for row in rows]
    assert planner[1] == str(caller / "inputs/intent.json")
    assert planner[planner.index("--config") + 1] == str(caller / "settings with spaces/client.toml")
    assert planner[planner.index("--iroha-cli") + 1] == str(caller / "bin/iroha stub")
    assert planner[planner.index("--plan-file") + 1] == str(caller / "reports/plan.json")
    assert metrics[metrics.index("--plan") + 1] == str(caller / "reports/plan.json")
    assert metrics[metrics.index("--output") + 1] == str(caller / "reports/metrics.prom")
    assert summary[1:5] == [str(caller / name) for name in (
        "reports/summary.json", "inputs/intent.json", "reports/plan.json", "reports/metrics.prom")]
    assert all(row["script_exists"] and all(row["inputs_exist"].values()) for row in rows[:2])
    assert all(row["cwd"] == str(SCRIPT.resolve().parent.parent) for row in rows[:2])
    assert rows[2]["cwd"] == str(caller)
    assert (caller / "reports/summary.json").is_file()
    assert (caller / "release output/relative").is_dir()
    assert "--apply" not in planner


def test_wrapper_resolves_relative_python_and_default_outputs_but_preserves_command_names(
    tmp_path: Path, monkeypatch
) -> None:
    caller = tmp_path / "default caller"
    caller.mkdir()
    (caller / "intent.json").write_text('{"schema_version":1,"intents":[]}\n', encoding="utf-8")
    python_dir = caller / "python tools"
    python_dir.mkdir()
    _path_recording_fake_python(python_dir)
    observed = tmp_path / "default-paths.ndjson"
    environment = dict(os.environ, PYTHON="python tools/fake-python",
        ALIAS_WRAPPER_CAPTURE=str(tmp_path / "calls.ndjson"),
        ALIAS_WRAPPER_PATH_CAPTURE=str(observed))
    environment.pop("SNS_RELEASE_DIR", None)
    environment.pop("SNS_RELEASE_NAME", None)
    monkeypatch.chdir(caller)
    result = _run("--intent", "intent.json", "--release-name", "defaults",
        "--iroha-cli", "named-iroha", env=environment)
    assert result.returncode == 0, result.stderr
    rows = [json.loads(line) for line in observed.read_text(encoding="utf-8").splitlines()]
    assert len(rows) == 3
    planner, metrics, summary = [row["argv"] for row in rows]
    release = caller / "artifacts/sns/releases/defaults"
    assert planner[planner.index("--iroha-cli") + 1] == "named-iroha"
    assert planner[1] == str(caller / "intent.json")
    assert planner[planner.index("--plan-file") + 1] == str(release / "alias-plan.json")
    assert metrics[metrics.index("--plan") + 1] == str(release / "alias-plan.json")
    assert metrics[metrics.index("--output") + 1] == str(release / "metrics.prom")
    assert summary[1] == str(release / "summary.json")
    assert (release / "summary.json").is_file()
    assert all(row["script_exists"] and all(row["inputs_exist"].values()) for row in rows[:2])
    assert all(row["cwd"] == str(SCRIPT.resolve().parent.parent) for row in rows[:2])
    assert rows[2]["cwd"] == str(caller)
