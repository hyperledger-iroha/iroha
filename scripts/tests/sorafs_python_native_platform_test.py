"""Execute portable SDK gate dispatch controls without claiming a native build."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
from types import SimpleNamespace

import pytest
from packaging.requirements import Requirement
import native_bash_fixture


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "ci/check_sorafs_python_native_sdk.sh"
BASH = native_bash_fixture.bash_executable()


def test_fixture_shell_selection_never_falls_back_to_windows_wsl(tmp_path, monkeypatch):
    program_files = tmp_path / "Program Files"
    expected = program_files / "Git/bin/bash.exe"
    with monkeypatch.context() as changes:
        changes.setattr(native_bash_fixture, "os", SimpleNamespace(name="nt", environ={"ProgramFiles": str(program_files)}))
        def no_windows_path_lookup(_name):
            raise AssertionError("Windows fixture must not select a PATH or System32 Bash")
        changes.setattr(native_bash_fixture.shutil, "which", no_windows_path_lookup)
        with pytest.raises(FileNotFoundError, match="native Git Bash"):
            native_bash_fixture.bash_executable()
        expected.parent.mkdir(parents=True)
        expected.write_bytes(b"path-selection fixture only, not an executable")
        assert native_bash_fixture.bash_executable() == str(expected)
    with monkeypatch.context() as changes:
        changes.setattr(native_bash_fixture, "os", SimpleNamespace(name="posix"))
        changes.setattr(native_bash_fixture.shutil, "which", lambda name: "/fixture/bin/bash" if name == "bash" else None)
        assert native_bash_fixture.bash_executable() == "/fixture/bin/bash"
        changes.setattr(native_bash_fixture.shutil, "which", lambda _name: None)
        with pytest.raises(FileNotFoundError, match="Bash fixture host"):
            native_bash_fixture.bash_executable()


def test_actual_fixture_shell_preserves_unicode_paths_and_literal_arguments(tmp_path):
    directory = tmp_path / "shell space-é-東京"
    directory.mkdir()
    path = directory / "quoted ' file.txt"
    path.write_text("original shell path fixture", encoding="utf-8")
    values = ["literal $dollar and 'single' and \"double\"", "back\\slash-é-東京", path.as_posix()]
    command = 'test -f "$3"; printf "%s\\n" "$1" "$2" "$3"; uname -s'
    result = subprocess.run([BASH, "--noprofile", "--norc", "-euc", command,
                             "native-bash-fixture", *values], capture_output=True,
                            encoding="utf-8", timeout=20, check=False)
    assert result.returncode == 0, result.stderr
    lines = result.stdout.splitlines()
    assert lines[:3] == values
    assert len(lines) == 4
    if os.name == "nt":
        assert lines[3].startswith(("MINGW", "MSYS")), lines[3]
    else:
        assert not lines[3].startswith(("MINGW", "MSYS")), lines[3]


def section(start: str, end: str) -> str:
    source = RUNNER.read_text(encoding="utf-8")
    return source[source.index(start):source.index(end, source.index(start))]


def capture_function() -> str:
    return section("capture_python() {", "\nPYTHON_VERSION=")


@pytest.mark.parametrize("host, scripts, executable", [("posix", "bin", "python"), ("nt", "Scripts", "python.exe")])
def test_selected_interpreter_controls_venv_dispatch(tmp_path: Path, host: str, scripts: str, executable: str):
    # This is a command-dispatch fixture. It supplies no actual Windows interpreter,
    # native ABI, filesystem ACL or SDK qualification evidence.
    session = tmp_path / "session with spaces"
    venv = session / "venv"
    commands = tmp_path / "commands"
    commands.mkdir()
    launcher = commands / "selected-python"
    target = venv / scripts / executable
    target.parent.mkdir(parents=True)
    terminator = "\\r\\n" if host == "nt" else "\\n"
    target.write_text("#!/bin/sh\nprintf '%s" + terminator + "' " + shlex.quote(venv.as_posix()) + "\n", encoding="utf-8", newline="\n")
    target.chmod(0o700)
    launcher.write_text(
        "#!/bin/sh\n"
        'if [ "$1" = -m ] && [ "$2" = venv ]; then exit 0; fi\n'
        "printf '%s" + terminator + "' " + shlex.quote(host) + "\n", encoding="utf-8", newline="\n"
    )
    launcher.chmod(0o700)
    command = capture_function() + section('"${PYTHON_BIN}" -m venv', '"${VENV_PYTHON}" -m pip install')
    command += '\nprintf "%s\\n" "$VENV_PYTHON" "$VIRTUAL_ENV" "$PATH" "$NATIVE_HELPER_SUFFIX"\n'
    env = dict(os.environ, PYTHON_BIN=launcher.as_posix(), SDK_SESSION=session.as_posix())
    result = subprocess.run([BASH, "-eu", "-c", command], env=env, capture_output=True, text=True, timeout=20, check=False)
    assert result.returncode == 0, result.stderr
    output = result.stdout.splitlines()
    assert output[0] == target.as_posix()
    assert output[1] == venv.as_posix()
    assert output[2].startswith(target.parent.as_posix() + ":")
    assert output[3:] == ([".exe"] if host == "nt" else [""])


def test_pythonpath_uses_interpreter_separator_and_canonical_inputs(tmp_path: Path):
    root = tmp_path / "source with spaces"
    (root / "python/norito_py/src").mkdir(parents=True)
    command = capture_function() + section('PYTHONPATH="$', '\nNATIVE_TARGET=')
    command += '\n"${VENV_PYTHON}" -I -c \'import json, os; print(json.dumps(os.environ["PYTHONPATH"]))\'\n'
    result = subprocess.run(
        [BASH, "-eu", "-c", command],
        env=dict(os.environ, VENV_PYTHON=Path(sys.executable).as_posix(), ROOT_DIR=root.as_posix()),
        capture_output=True, text=True, timeout=20, check=False,
    )
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == os.pathsep.join([str(root / "python/norito_py/src"), str(root / "python")])


def test_backup_selection_leaves_creation_to_native_private_owner(tmp_path: Path):
    command = capture_function() + section('SOURCE_BACKUP_DIRECTORY="$', '\nSOURCE_RECEIPT_SEAL=')
    command += '\nprintf "%s\\n" "$SOURCE_BACKUP_DIRECTORY"\n'
    env = dict(os.environ, VENV_PYTHON=Path(sys.executable).as_posix(), ROOT_DIR=tmp_path.as_posix())
    selected = []
    for _ in range(2):
        result = subprocess.run([BASH, "-eu", "-c", command], env=env, capture_output=True, text=True, timeout=20, check=False)
        assert result.returncode == 0, result.stderr
        candidate = Path(result.stdout.strip())
        assert candidate.parent == tmp_path.resolve()
        assert candidate.name.startswith(".codex-python-native-source.")
        assert not candidate.exists()
        selected.append(candidate)
    assert selected[0] != selected[1]


def test_helper_build_and_promotion_stay_inside_original_source_pin():
    source = RUNNER.read_text(encoding="utf-8")
    pin = source.index("SOURCE_BEFORE_PIN=")
    target = source.index('NATIVE_HELPER_TARGET="${SDK_SESSION}/native-helper-target"')
    build = source.index("cargo build --locked -p iroha_native_source_delivery")
    check = source.index("assert_build_source_unchanged", build)
    controls = source.index('IROHA_NATIVE_SOURCE_DELIVERY_TEST_HELPER="${NATIVE_HELPER}"')
    promotion = source.index('--native-helper "${NATIVE_HELPER}"')
    assert pin < target < build < check < controls < promotion
    build_command = source[build:check]
    assert '--target "${NATIVE_HELPER_HOST}"' in build_command
    assert '--target-dir "${NATIVE_HELPER_TARGET}"' in build_command
    assert 'NATIVE_HELPER="$(capture_python "${VENV_PYTHON}"' in source
    assert "SORAFS_NATIVE_HELPER" not in source
    for workflow in ["sorafs-orchestrator-sdk.yml", "pr_privacy_sdk_guard.yml"]:
        paths = (ROOT / ".github/workflows" / workflow).read_text(encoding="utf-8").split("paths:", 1)[1].split("workflow_dispatch:", 1)[0]
        for owner in ["crates/iroha_fs/**", "tools/iroha_native_source_delivery/**", "scripts/tests/sorafs_python_native_platform_test.py"]:
            assert f'- "{owner}"' in paths


def test_capture_removes_native_line_endings_without_masking_failure():
    function = capture_function()
    payload = json.dumps({"pin": "literal\rcarriage"}, separators=(",", ":"))
    success = subprocess.run([BASH, "-euo", "pipefail", "-c", function + '\nvalue=$(capture_python printf \'%s\\r\\n\' "$IROHA_CAPTURE_PAYLOAD"); printf "%s" "$value"'], env=dict(os.environ, IROHA_CAPTURE_PAYLOAD=payload), capture_output=True, text=True, timeout=20, check=False)
    assert success.returncode == 0, success.stderr
    assert success.stdout == payload
    failed = subprocess.run([BASH, "-euo", "pipefail", "-c", function + '\nvalue=$(capture_python sh -c "exit 37"); echo incorrectly-continued'], capture_output=True, text=True, timeout=20, check=False)
    assert failed.returncode == 37
    assert not failed.stdout


def test_windows_pytest_console_dependency_is_pinned_and_hashed():
    requirements = ROOT / "python/iroha_python"
    source = (requirements / "requirements-ci.in").read_text(encoding="utf-8")
    locked = (requirements / "requirements-ci.lock").read_text(encoding="utf-8")
    declaration = next(line for line in source.splitlines() if line.startswith("colorama=="))
    requirement = Requirement(declaration)
    assert str(requirement.specifier) == "==0.4.6"
    assert requirement.marker.evaluate({"sys_platform": "win32"})
    assert not requirement.marker.evaluate({"sys_platform": "linux"})
    assert not requirement.marker.evaluate({"sys_platform": "darwin"})
    block = locked.split(declaration + " \\\n", 1)[1].split("\n    #", 1)[0]
    assert set(line.strip().rstrip(" \\") for line in block.splitlines()) == {
        "--hash=sha256:08695f5cb7ed6e0531a20572697297273c47b8cae5a63ffc6d6ed5c201be6e44",
        "--hash=sha256:4f1d9991f5acc0ca119f9d443620b77f9d6b33703e51011c16baf57afb285fc6",
    }
