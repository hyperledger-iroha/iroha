from __future__ import annotations

import os
import json
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
    capture_options: list[str] | None = None,
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
    command.extend(capture_options or [])
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


def _validation_outcome(*, status: str = "Ok", code: str = "SFS-OK-000") -> dict[str, object]:
    return {
        "status": status,
        "code": code,
        "category": "validation",
        "message": "provider advert accepted",
        "action": None,
        "docs_url": "https://docs.iroha.tech/",
        "telemetry_tags": [
            "sorafs.reference.advert",
            f"sorafs.reference.code.{code}",
        ],
        "context": [{"key": "provider_id", "value": "11" * 32}],
        "inputs": [{"kind": "provider_advert", "path": "advert.to"}],
        "version": 1,
        "generated_at": 123,
    }


def _validation_capture_options() -> list[str]:
    return [
        "--require-validation-outcome-ok-v1",
        "--expected-validation-code",
        "SFS-OK-000",
        "--expected-generated-at",
        "123",
        "--required-telemetry-tag",
        "sorafs.reference.advert",
        "--required-telemetry-tag",
        "sorafs.reference.code.SFS-OK-000",
    ]


def test_capture_accepts_exact_successful_validation_outcome_v1(tmp_path: Path) -> None:
    payload = json.dumps(_validation_outcome(), separators=(",", ":"))
    executable = _write_executable(
        tmp_path / "tool",
        f"#!/usr/bin/env python3\nprint({payload!r})\n",
    )
    output = tmp_path / "outcome.json"
    result = _run(
        executable,
        output,
        capture_options=_validation_capture_options(),
    )
    assert result.returncode == 0, result.stderr
    assert json.loads(output.read_text(encoding="utf-8")) == _validation_outcome()


def test_capture_rejects_unsuccessful_validation_outcome_without_writing(
    tmp_path: Path,
) -> None:
    payload = json.dumps(
        _validation_outcome(status="Error", code="SFS-POLICY-001"),
        separators=(",", ":"),
    )
    executable = _write_executable(
        tmp_path / "tool",
        f"#!/usr/bin/env python3\nprint({payload!r})\n",
    )
    output = tmp_path / "outcome.json"
    result = _run(
        executable,
        output,
        capture_options=_validation_capture_options(),
    )
    assert result.returncode != 0
    assert "validation outcome did not succeed" in result.stderr
    assert not output.exists()


def test_capture_rejects_duplicate_validation_outcome_fields(tmp_path: Path) -> None:
    payload = json.dumps(_validation_outcome(), separators=(",", ":"))
    payload = payload.replace('{"status":"Ok",', '{"status":"Ok","status":"Ok",', 1)
    executable = _write_executable(
        tmp_path / "tool",
        f"#!/usr/bin/env python3\nprint({payload!r})\n",
    )
    output = tmp_path / "outcome.json"
    result = _run(
        executable,
        output,
        capture_options=_validation_capture_options(),
    )
    assert result.returncode != 0
    assert "duplicate JSON field" in result.stderr
    assert not output.exists()
