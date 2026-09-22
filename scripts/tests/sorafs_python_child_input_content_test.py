"""Pure input parsing and real live-path controls; no SDK/native execution."""
from __future__ import annotations

import builtins
import copy
import importlib.util
import io
import os
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("child_input_content_controls", ROOT / "scripts/fixtures/SorafsPythonConsumerQualificationRunner.py")
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


def content(root="/historical/producer"):
    names = (runner.RUNNER, runner.VERIFIER, runner.CASES, runner.TEST,
             "fixtures/sorafs_manifest/control.bin", "python/norito_py/src/norito/__init__.py",
             "python/iroha_torii_client/__init__.py")
    return {"schema": runner.INPUT_SCHEMA, "snapshot_root": root + "/snapshot",
            "environment_root": root + "/environment",
            "native_wheel": {"path": root + "/native.whl", "seal": "a" * 64 + ":1:2:3:4:5:600"},
            "sdk_wheel": {"path": root + "/sdk.whl", "seal": "b" * 64 + ":1:3:3:4:5:600"},
            "source_files": [{"path": name, "sha256": "c" * 64, "size": 1} for name in sorted(names)],
            "python": {"path": root + "/python", "sha256": "d" * 64, "size": 1}}


def live_content(tmp_path):
    value = content(str(tmp_path))
    for name in ("snapshot_root", "environment_root"):
        Path(value[name]).mkdir()
    for name in ("native_wheel", "sdk_wheel", "python"):
        Path(value[name]["path"]).write_bytes(b"inert one-byte-path control")
    return value


def test_pure_parser_never_opens_resolves_stats_or_loads_historical_paths(monkeypatch):
    value = content(); raw = runner.canonical_json(value)
    calls = []
    def forbidden(*args, **kwargs):
        calls.append(args)
        raise AssertionError("pure parser touched a filesystem or live Path")
    with monkeypatch.context() as scope:
        scope.setattr(runner, "Path", forbidden)
        scope.setattr(runner, "_absolute", forbidden)
        scope.setattr(runner, "read_stable", forbidden)
        scope.setattr(runner.importlib.util, "spec_from_file_location", forbidden)
        scope.setattr(builtins, "open", forbidden)
        scope.setattr(io, "open", forbidden)
        for name in ("open", "stat", "lstat", "scandir", "listdir", "readlink"):
            scope.setattr(os, name, forbidden)
        parsed = runner.parse_input_content(raw)
    assert parsed == value and not calls


def test_historical_absence_is_not_mistaken_for_live_custody(tmp_path):
    raw = runner.canonical_json(content(str(tmp_path / "missing")))
    assert runner.parse_input_content(raw)["schema"] == runner.INPUT_SCHEMA
    with pytest.raises(FileNotFoundError): runner.parse_input(raw)


def test_live_parser_delegates_once_then_keeps_real_path_checks(tmp_path, monkeypatch):
    value = live_content(tmp_path); raw = runner.canonical_json(value)
    original = runner.parse_input_content; calls = []
    def observed(raw): calls.append(raw); return original(raw)
    monkeypatch.setattr(runner, "parse_input_content", observed)
    assert runner.parse_input(raw) == value and calls == [raw]
    Path(value["native_wheel"]["path"]).unlink()
    with pytest.raises(FileNotFoundError): runner.parse_input(raw)
    assert calls == [raw, raw]


@pytest.mark.parametrize("owner", ("snapshot_root", "environment_root", "native_wheel", "sdk_wheel", "python"))
def test_live_parser_still_rejects_original_path_symlink_aliases(tmp_path, owner):
    value = live_content(tmp_path)
    original = Path(value[owner] if owner.endswith("root") else value[owner]["path"])
    alias = tmp_path / "alias"; alias.symlink_to(original)
    if owner.endswith("root"): value[owner] = str(alias)
    else: value[owner]["path"] = str(alias)
    raw = runner.canonical_json(value)
    assert runner.parse_input_content(raw) == value
    with pytest.raises(runner.QualificationError, match="canonical"): runner.parse_input(raw)


@pytest.mark.parametrize("owner", ("snapshot_root", "environment_root"))
def test_live_roots_remain_actual_directories(tmp_path, owner):
    value = live_content(tmp_path)
    root = Path(value[owner]); root.rmdir(); root.write_bytes(b"inert file")
    raw = runner.canonical_json(value)
    assert runner.parse_input_content(raw) == value
    with pytest.raises(runner.QualificationError, match="directories"): runner.parse_input(raw)


@pytest.mark.parametrize("path", ("relative", "", "/tmp/../elsewhere", "/tmp/./a", "/tmp//a",
                                  "//host/a", "/tmp/a/", "/tmp/line\nfeed", "/tmp/cafe\u0301", "x" * 4097))
def test_logical_absolute_aliases_refused_before_live_access(monkeypatch, path):
    value = content(); value["python"]["path"] = path
    def forbidden(*args, **kwargs): raise AssertionError("live path used for syntax")
    monkeypatch.setattr(runner, "_absolute", forbidden)
    with pytest.raises(runner.QualificationError): runner.parse_input_content(runner.canonical_json(value))


@pytest.mark.parametrize("mutation", ("extra", "bool-size", "bad-digest", "foreign-source", "source-parent",
                                      "duplicate-source", "unsorted-source", "missing-fixed", "excessive-file",
                                      "excessive-total", "same-roots", "wheel-extra", "seal-type", "seal-size"))
def test_both_entrypoints_share_schema_and_bound_refusals(tmp_path, mutation):
    value = live_content(tmp_path)
    if mutation == "extra": value["execute"] = True
    elif mutation == "bool-size": value["python"]["size"] = True
    elif mutation == "bad-digest": value["python"]["sha256"] = "A" * 64
    elif mutation == "foreign-source": value["source_files"][0]["path"] = "foreign.py"
    elif mutation == "source-parent": value["source_files"][0]["path"] = "ci/../verifier.py"
    elif mutation == "duplicate-source": value["source_files"].append(copy.deepcopy(value["source_files"][0]))
    elif mutation == "unsorted-source": value["source_files"].reverse()
    elif mutation == "missing-fixed": value["source_files"] = value["source_files"][1:]
    elif mutation == "excessive-file": value["source_files"][0]["size"] = runner.MAX_FILE_BYTES + 1
    elif mutation == "excessive-total":
        for row in value["source_files"]: row["size"] = runner.MAX_FILE_BYTES
    elif mutation == "same-roots": value["environment_root"] = value["snapshot_root"]
    elif mutation == "wheel-extra": value["native_wheel"]["members"] = []
    elif mutation == "seal-type": value["native_wheel"]["seal"] = {}
    elif mutation == "seal-size": value["native_wheel"]["seal"] = "a" * 513
    raw = runner.canonical_json(value)
    for parser in (runner.parse_input_content, runner.parse_input):
        with pytest.raises(runner.QualificationError): parser(raw)


@pytest.mark.parametrize("suffix", (b" ", b"\n", b"\r\n"))
def test_noncanonical_input_bytes_refused(suffix):
    with pytest.raises(runner.QualificationError):
        runner.parse_input_content(runner.canonical_json(content()) + suffix)


@pytest.mark.parametrize("seal", ("", "inert-noncanonical-seal", "a" * 512))
def test_seal_interpretation_remains_with_later_authenticated_owner(tmp_path, seal):
    value = live_content(tmp_path); value["native_wheel"]["seal"] = seal
    raw = runner.canonical_json(value)
    assert runner.parse_input_content(raw) == runner.parse_input(raw) == value
    # A parsed string never grants FileSeal, wheel, path, or execution authority.


def test_duplicate_fields_and_input_byte_bound_refused():
    raw = runner.canonical_json(content())
    duplicate = raw.replace(b'{"environment_root":', b'{"schema":"duplicate","environment_root":', 1)
    with pytest.raises(runner.QualificationError, match="duplicate"): runner.parse_input_content(duplicate)
    with pytest.raises(runner.QualificationError, match="byte bound"):
        runner.parse_input_content(b" " * (runner.MAX_INPUT + 1))
