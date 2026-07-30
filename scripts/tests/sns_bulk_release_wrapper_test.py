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
