"""Pure bootstrap origin and inert wheel console-name controls; no SDK install."""
from __future__ import annotations

import base64
import builtins
import csv
import hashlib
import io
import os
from pathlib import Path, PurePosixPath
import stat
import sys
from types import SimpleNamespace
import zipfile

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_environment as environment_owner
from sorafs_python_consumer_artifact import ArtifactError
from sorafs_python_dependency_archive import parse_dependency_wheel
from sorafs_python_dependency_inputs import DependencyWheel
from sorafs_sdk_artifact_index import FileReference

BOOTSTRAP = environment_owner.BOOTSTRAP_FILES
PYTHONS = ("bin/python", "bin/python3", "bin/python3.12")
ACTIVATIONS = tuple(sorted(BOOTSTRAP - set(PYTHONS) - {"pyvenv.cfg"}))


def fixture(basename="python3.12"):
    executable = b"inert original executable control"
    runtime = SimpleNamespace(platform="linux", version="3.12.14", executable=SimpleNamespace(
        path="/historical/runtime/bin/" + basename,
        sha256=hashlib.sha256(executable).hexdigest(), size=len(executable)))
    environment = PurePosixPath("/historical/producer/environment")
    files = {name: executable for name in PYTHONS}
    files.update({name: b"inert retained activation bytes" for name in ACTIVATIONS})
    files["pyvenv.cfg"] = (f"home = /historical/runtime/bin\ninclude-system-site-packages = false\n"
                          f"version = 3.12.14\nexecutable = {runtime.executable.path}\n"
                          f"command = {runtime.executable.path} -m venv --copies --without-pip {environment}\n").encode()
    return files, runtime, environment


@pytest.mark.parametrize("basename", ("python", "python3", "python3.12"))
def test_shared_profile_joins_all_three_original_interpreters(basename):
    files, runtime, environment = fixture(basename)
    expected = dict(files)
    files["lib/python3.12/site-packages/pytest/__init__.py"] = b"captured installed member"
    result = environment_owner.verify_environment_bootstrap(files, runtime, environment)
    assert result == expected and set(result) == BOOTSTRAP
    assert tuple(result) == tuple(sorted(BOOTSTRAP))
    assert result is not files


def test_bootstrap_join_has_no_historical_filesystem_access(monkeypatch):
    files, runtime, environment = fixture()
    def forbidden(*args, **kwargs): raise AssertionError("historical file opened")
    with monkeypatch.context() as scope:
        scope.setattr(builtins, "open", forbidden)
        scope.setattr(io, "open", forbidden)
        for name in ("open", "stat", "lstat", "scandir", "readlink"):
            scope.setattr(os, name, forbidden)
        scope.setattr(Path, "resolve", forbidden)
        result = environment_owner.verify_environment_bootstrap(files, runtime, environment)
    assert result == files


@pytest.mark.parametrize("name", sorted(BOOTSTRAP))
def test_each_bootstrap_member_is_mandatory(name):
    files, runtime, environment = fixture(); del files[name]
    with pytest.raises(ArtifactError, match="inventory"):
        environment_owner.verify_environment_bootstrap(files, runtime, environment)


@pytest.mark.parametrize("name", PYTHONS)
@pytest.mark.parametrize("body", (b"", b"substitute", b"inert original executable controL", bytearray(b"mutable")))
def test_each_interpreter_must_match_original_bytes(name, body):
    files, runtime, environment = fixture(); files[name] = body
    with pytest.raises(ArtifactError): environment_owner.verify_environment_bootstrap(files, runtime, environment)


@pytest.mark.parametrize("name", ACTIVATIONS)
def test_activation_is_bounded_retained_output_not_regenerated_source(name, monkeypatch):
    files, runtime, environment = fixture()
    before = environment_owner.verify_environment_bootstrap(files, runtime, environment)
    files[name] = b"different bounded activation output"
    after = environment_owner.verify_environment_bootstrap(files, runtime, environment)
    assert before != after  # The parent must compare with its retained fresh subset.
    files[name] = b""
    with pytest.raises(ArtifactError): environment_owner.verify_environment_bootstrap(files, runtime, environment)
    monkeypatch.setattr(environment_owner, "_MAX_BOOTSTRAP_ACTIVATION_BYTES", 4)
    files[name] = b"12345"
    with pytest.raises(ArtifactError): environment_owner.verify_environment_bootstrap(files, runtime, environment)


@pytest.mark.parametrize("mutation", ("home", "version", "executable", "command-path", "command-options",
                                      "system-site", "prompt", "duplicate", "missing", "order", "crlf", "nonutf8"))
def test_exact_stock_five_field_configuration_is_required(mutation):
    files, runtime, environment = fixture(); raw = files["pyvenv.cfg"]
    if mutation == "home": raw = raw.replace(b"home = /historical/runtime/bin", b"home = /foreign/bin")
    elif mutation == "version": raw = raw.replace(b"version = 3.12.14", b"version = 3.12.13")
    elif mutation == "executable": raw = raw.replace(b"executable = /historical", b"executable = /foreign")
    elif mutation == "command-path": raw = raw.replace(b"/producer/environment", b"/producer/other")
    elif mutation == "command-options": raw = raw.replace(b"--copies --without-pip", b"--without-pip --copies")
    elif mutation == "system-site": raw = raw.replace(b"packages = false", b"packages = true")
    elif mutation == "prompt": raw += b"prompt = custom\n"
    elif mutation == "duplicate": raw += raw.splitlines(keepends=True)[0]
    elif mutation == "missing": raw = b"\n".join(raw.splitlines()[:-1]) + b"\n"
    elif mutation == "order": raw = b"\n".join(reversed(raw.splitlines())) + b"\n"
    elif mutation == "crlf": raw = raw.replace(b"\n", b"\r\n")
    else: raw += b"\xff"
    files["pyvenv.cfg"] = raw
    with pytest.raises(ArtifactError, match="configuration"):
        environment_owner.verify_environment_bootstrap(files, runtime, environment)


@pytest.mark.parametrize("mutation", ("basename", "platform", "version", "zero-size", "bool-size",
                                      "large-size", "digest", "alias-path", "relative-env", "string-env"))
def test_malformed_runtime_or_environment_profile_is_refused(mutation):
    files, runtime, environment = fixture()
    if mutation == "basename": runtime.executable.path = "/historical/runtime/bin/pypy"
    elif mutation == "platform": runtime.platform = "win32"
    elif mutation == "version": runtime.version = "3.12.014"
    elif mutation == "zero-size": runtime.executable.size = 0
    elif mutation == "bool-size": runtime.executable.size = True
    elif mutation == "large-size": runtime.executable.size = 256 * 1024 * 1024 + 1
    elif mutation == "digest": runtime.executable.sha256 = "0" * 64
    elif mutation == "alias-path": runtime.executable.path = "/runtime/../bin/python3.12"
    elif mutation == "relative-env": environment = PurePosixPath("relative")
    elif mutation == "string-env": environment = "/historical/producer/environment"
    with pytest.raises(ArtifactError): environment_owner.verify_environment_bootstrap(files, runtime, environment)


def dependency_wheel(script):
    """Build a tiny ordinary pytest wheel; the production parser owns validation."""
    dist = "pytest-9.0.3.dist-info"
    contents = {"_pytest/__init__.py": b"# inert dependency source\n",
                "pytest/__init__.py": b"# inert facade\n",
                dist + "/METADATA": b"Metadata-Version: 2.3\nName: pytest\nVersion: 9.0.3\n",
                dist + "/WHEEL": b"Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n",
                dist + "/entry_points.txt": ("[console_scripts]\n" + script + "=pytest:main\n").encode()}
    record = dist + "/RECORD"
    text = io.StringIO(newline=""); writer = csv.writer(text, lineterminator="\n")
    for name, raw in sorted(contents.items()):
        digest = base64.urlsafe_b64encode(hashlib.sha256(raw).digest()).rstrip(b"=").decode()
        writer.writerow((name, "sha256=" + digest, str(len(raw))))
    writer.writerow((record, "", "")); contents[record] = text.getvalue().encode()
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED) as wheel:
        for name, raw in sorted(contents.items()):
            info = zipfile.ZipInfo(name); info.create_system = 3
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            wheel.writestr(info, raw)
    raw = output.getvalue()
    return raw, DependencyWheel("pytest", "9.0.3", FileReference(
        "/historical/wheels/pytest.whl", hashlib.sha256(raw).hexdigest(), len(raw)))


@pytest.mark.parametrize("name", ("pytest", "py.test", "cffi-gen-src"))
def test_nonbootstrap_console_entries_keep_the_original_wheel_contract(name):
    raw, original = dependency_wheel(name)
    assert parse_dependency_wheel(raw, wheel=original).console_scripts == (name,)


@pytest.mark.parametrize("name", tuple(sorted(PurePosixPath(name).name for name in BOOTSTRAP if name.startswith("bin/"))))
@pytest.mark.parametrize("uppercase", (False, True))
def test_full_original_wheel_parser_refuses_bootstrap_console_collisions_before_pip(name, uppercase):
    raw, original = dependency_wheel(name.upper() if uppercase else name)
    with pytest.raises(ArtifactError, match="collides with the environment bootstrap"):
        parse_dependency_wheel(raw, wheel=original)
