"""Fixed operation replay controls; self-consistency is not producer approval."""
from __future__ import annotations

from copy import deepcopy
from dataclasses import replace
import hashlib
import importlib.util
from pathlib import Path, PurePosixPath
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_commands as commands

SPEC = importlib.util.spec_from_file_location(
    "_runtime_command_fixture", ROOT / "scripts/tests/sorafs_python_runtime_inputs_test.py")
FIXTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURE)


def _catalog(tmp_path, **overrides):
    runtime = FIXTURE.parsed(FIXTURE.fixture(tmp_path))
    args = dict(source_root=PurePosixPath("/recorded/iroha"),
                work=PurePosixPath("/recorded/iroha/target/qualification"), runtime=runtime,
                pip_filename="pip-26.2.1-py3-none-any.whl", native_filename="_crypto.abi3.so")
    args.update(overrides)
    return commands.execution_commands(**args)


def _observed(catalog):
    rows, files = [], {}
    for command in catalog:
        row = {"label": command.label, "argv": list(command.argv), "returncode": 0}
        for stream in ("stdout", "stderr"):
            body = ("inert operation observation: " + command.label).encode() if stream == "stdout" else b""
            files["logs/" + command.label + "." + stream] = body
            row[stream] = {"sha256": hashlib.sha256(body).hexdigest(), "size": len(body)}
        rows.append(row)
    return rows, files


def test_catalog_can_be_replayed_without_access_to_historical_paths(tmp_path, monkeypatch):
    catalog = _catalog(tmp_path)
    assert tuple(command.label for command in catalog) == (
        "runtime-before", "create-environment", "environment-before", "native-before", "install",
        "installed-distributions", "execute", "environment-after", "native-after")
    assert catalog[3].argv == catalog[8].argv and catalog[2].argv == catalog[7].argv
    assert all("-I" in command.argv and "-B" in command.argv for command in catalog)
    rows, files = _observed(catalog)

    def forbidden(*_args, **_kwargs):
        raise AssertionError("captured command replay must not open historical paths")

    monkeypatch.setattr(Path, "open", forbidden)
    commands.verify_command_observations(rows, files, expected=catalog)


@pytest.mark.parametrize("change", (
    {"work": PurePosixPath("/other/target/run")},
    {"work": PurePosixPath("/recorded/iroha/target")},
    {"work": PurePosixPath("/recorded/iroha/target/../escape")},
    {"source_root": PurePosixPath("relative")},
    {"pip_filename": "nested/pip.whl"}, {"pip_filename": "pip.zip"},
    {"native_filename": "../_crypto.so"}, {"native_filename": "/_crypto.so"},
))
def test_catalog_refuses_other_topologies(tmp_path, change):
    with pytest.raises(commands.ArtifactError): _catalog(tmp_path, **change)


@pytest.mark.parametrize("mutation", ("missing", "extra", "reorder", "duplicate", "failed", "bool", "argv",
                                      "nonisolated", "label", "extra_field", "tuple"))
def test_captured_operations_cannot_omit_or_replace_fixed_commands(tmp_path, mutation):
    catalog = _catalog(tmp_path)
    rows, files = _observed(catalog)
    if mutation == "missing": rows.pop()
    elif mutation == "extra": rows.append(deepcopy(rows[-1]))
    elif mutation == "reorder": rows[0], rows[1] = rows[1], rows[0]
    elif mutation == "duplicate": rows[-1] = deepcopy(rows[0])
    elif mutation == "failed": rows[3]["returncode"] = 1
    elif mutation == "bool": rows[3]["returncode"] = False
    elif mutation == "argv": rows[4]["argv"].append("--arbitrary-command")
    elif mutation == "nonisolated": rows[6]["argv"].remove("-I")
    elif mutation == "label": rows[5]["label"] = "claimed-passing"
    elif mutation == "extra_field": rows[0]["passed"] = True
    else: rows = tuple(rows)
    with pytest.raises(commands.ArtifactError):
        commands.verify_command_observations(rows, files, expected=catalog)


@pytest.mark.parametrize("mutation", ("missing", "extra", "changed", "wrong_digest", "wrong_size", "bool_size",
                                      "stderr", "overflow", "extra_identity"))
def test_captured_logs_must_be_exact_owned_and_bounded(tmp_path, mutation):
    catalog = _catalog(tmp_path)
    rows, files = _observed(catalog)
    name = "logs/execute.stdout"
    if mutation == "missing": del files[name]
    elif mutation == "extra": files["logs/hidden.stdout"] = b"hidden"
    elif mutation == "changed": files[name] += b"unaccounted"
    elif mutation == "wrong_digest": rows[6]["stdout"]["sha256"] = "a" * 64
    elif mutation == "wrong_size": rows[6]["stdout"]["size"] += 1
    elif mutation == "bool_size": rows[6]["stderr"]["size"] = False
    elif mutation == "stderr":
        files["logs/execute.stderr"] = b"warning"
        rows[6]["stderr"] = {"sha256": hashlib.sha256(b"warning").hexdigest(), "size": 7}
    elif mutation == "overflow":
        catalog = tuple(replace(command, stdout_limit=1) if command.label == "execute" else command
                        for command in catalog)
    else: rows[6]["stdout"]["claimed"] = True
    with pytest.raises(commands.ArtifactError):
        commands.verify_command_observations(rows, files, expected=catalog)
