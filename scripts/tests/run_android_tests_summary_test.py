"""Exercise the Android CI shell owner with real task processes and summary output."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ci" / "run_android_tests.sh"


@pytest.mark.parametrize("failed_task", [None, ":second:task"])
def test_android_summary_retains_every_task_and_actual_exit_status(
    tmp_path: Path, failed_task: str | None
) -> None:
    tools = tmp_path / "tools"
    tools.mkdir()
    (tools / "python3").symlink_to(sys.executable)
    java_home = tmp_path / "jdk"
    (java_home / "bin").mkdir(parents=True)
    java = java_home / "bin" / "java"
    java.write_text('#!/usr/bin/env bash\nprintf \'openjdk version "21.0.7"\\n\' >&2\n')
    java.chmod(0o700)

    gradle = tools / "gradle-fixture"
    gradle.write_text(
        "#!/usr/bin/env bash\n"
        "set -eu\n"
        'task="${!#}"\n'
        'printf "%s\\n" "$task" >> "$ANDROID_UNIT_GRADLE_INVOCATIONS"\n'
        'if [[ "$task" == "${ANDROID_UNIT_FAILED_TASK:-}" ]]; then\n'
        '  printf "fixture task failed\\n" >&2\n'
        "  exit 7\n"
        "fi\n"
        'printf "fixture task completed\\n"\n'
    )
    gradle.chmod(0o700)

    tasks = [":first:task", ":second:task", ":third:task"]
    summary = tmp_path / "summary with spaces.json"
    invocations = tmp_path / "gradle-invocations.txt"
    env = os.environ.copy()
    env.update(
        PATH=str(tools) + os.pathsep + env.get("PATH", ""),
        JAVA_HOME=str(java_home),
        GRADLE_BIN=str(gradle),
        ANDROID_GRADLE_TASKS=" ".join(tasks),
        ANDROID_TEST_ARTIFACTS=str(tmp_path / "artifacts"),
        ANDROID_TEST_SUMMARY_OUT=str(summary),
        ANDROID_TEST_LOG_OUT=str(tmp_path / "task.log"),
        ANDROID_UNIT_GRADLE_INVOCATIONS=str(invocations),
        ANDROID_UNIT_FAILED_TASK=failed_task or "",
    )
    result = subprocess.run(
        ["bash", str(SCRIPT)],
        cwd=ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )

    assert result.returncode == (1 if failed_task else 0), result.stderr
    assert invocations.read_text().splitlines() == tasks
    payload = json.loads(summary.read_text())
    assert payload["schema_version"] == 1
    assert payload["generated_at"].endswith("Z")
    assert payload["total"] == len(tasks)
    assert payload["failures"] == (1 if failed_task else 0)
    assert [record["name"] for record in payload["results"]] == [
        task.removeprefix(":").replace(":", "_") for task in tasks
    ]
    for task, record in zip(tasks, payload["results"], strict=True):
        assert record["status"] == ("failed" if task == failed_task else "ok")
        assert isinstance(record["duration_ms"], int)
        assert record["duration_ms"] >= 0
        if task == failed_task:
            assert record["error"] == "exit code 7"
        else:
            assert "error" not in record
