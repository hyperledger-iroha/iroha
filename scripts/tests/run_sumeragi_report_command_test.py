"""Regression tests for the gated Sumeragi report binaries."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import sys

import pytest


SCRIPTS = Path(__file__).resolve().parents[1]


def _load_script(name: str):
    path = SCRIPTS / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize(
    ("module", "binary", "report_name"),
    (
        (
            _load_script("run_sumeragi_baseline"),
            "sumeragi_baseline_report",
            "sumeragi-baseline-report.md",
        ),
        (
            _load_script("run_sumeragi_da"),
            "sumeragi_da_report",
            "sumeragi-da-report.md",
        ),
    ),
)
def test_report_command_enables_build_support_dev_tools(
    module, binary: str, report_name: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    captured: list[list[str]] = []

    def run(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
        captured.append(command)
        return subprocess.CompletedProcess(command, 0, stdout="# report\n", stderr="")

    monkeypatch.setattr(module.subprocess, "run", run)

    report_path, return_code = module._render_markdown_report(
        "cargo-under-test", tmp_path, {}
    )

    assert return_code == 0
    assert report_path == tmp_path / report_name
    command = captured.pop()
    assert command[command.index("-p") + 1] == "build-support"
    assert command[command.index("--features") + 1] == "build-support/dev-tools"
    assert command[command.index("--bin") + 1] == binary
