from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "capture_release_command.py"


def _write_executable(path: Path, payload: str) -> Path:
    path.write_text(payload, encoding="utf-8")
    path.chmod(0o755)
    return path


def _run(
    executable: Path,
    output: Path,
    *,
    env: dict[str, str] | None = None,
    trusted_sha256: str | None = None,
) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment.update(env or {})
    command = [
        sys.executable,
        str(SCRIPT),
        "--output",
        str(output),
        "--executable-root",
        str(executable.parent),
        "--executable-relative",
        executable.name,
    ]
    if trusted_sha256 is not None:
        command.extend(["--trusted-executable-sha256", trusted_sha256])
    command.append("--")
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )


def test_capture_executes_a_private_pinned_copy(tmp_path: Path) -> None:
    executable = _write_executable(
        tmp_path / "tool",
        "#!/bin/sh\nprintf '%s\\n' \"$0\"\n",
    )
    output = tmp_path / "output.txt"
    result = _run(executable, output)
    assert result.returncode == 0, result.stderr
    executed_path = output.read_text(encoding="utf-8").strip()
    assert executed_path != str(executable)
    assert "iroha-release-command." in executed_path


def test_capture_rejects_replace_execute_restore_race(tmp_path: Path) -> None:
    executable = _write_executable(
        tmp_path / "tool",
        "#!/usr/bin/env python3\n"
        "import os, pathlib\n"
        "source = pathlib.Path(os.environ['MUTATE_CAPTURE_SOURCE'])\n"
        "saved = source.with_name(source.name + '.saved')\n"
        "source.rename(saved)\n"
        "source.write_text('#!/bin/sh\\nprintf malicious\\\\n', encoding='utf-8')\n"
        "source.chmod(0o755)\n"
        "source.unlink()\n"
        "saved.rename(source)\n"
        "print('trusted')\n",
    )
    output = tmp_path / "output.txt"
    result = _run(
        executable,
        output,
        env={"MUTATE_CAPTURE_SOURCE": str(executable)},
    )
    assert result.returncode != 0
    assert "release executable changed while its output was captured" in result.stderr
    assert not output.exists()


def test_capture_rejects_untrusted_executable_before_launch(tmp_path: Path) -> None:
    marker = tmp_path / "launched"
    executable = _write_executable(
        tmp_path / "tool",
        f"#!/bin/sh\nprintf launched >{marker}\n",
    )
    output = tmp_path / "output.txt"
    result = _run(
        executable,
        output,
        trusted_sha256="0" * 64,
    )
    assert result.returncode != 0
    assert "SHA256 is not trusted" in result.stderr
    assert not marker.exists()
    assert not output.exists()
