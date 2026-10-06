"""Guarded host emitter failure controls; no mocked run is artifact qualification."""
from pathlib import Path
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import build_native_sdk_host_guarded as emitter


def test_changes_include_membership_and_value():
    assert emitter.changes({"a": 1, "b": 2}, {"a": 2, "c": 3}) == ["a", "b", "c"]
    assert emitter.changes({"a": 1}, {"a": 1}) == []


def test_identity_refuses_alias_and_detects_edit_restore(tmp_path):
    path = tmp_path / "source"
    path.write_bytes(b"original")
    original = emitter.file_identity(path)
    path.write_bytes(b"changed")
    path.write_bytes(b"original")
    assert emitter.file_identity(path) != original
    alias = tmp_path / "alias"
    alias.symlink_to(path)
    with pytest.raises(emitter.unit.Refused):
        emitter.file_identity(alias)


def test_source_identity_resolves_declared_external_prefixes(tmp_path):
    local = tmp_path / "local"
    external = tmp_path / "external"
    local.write_text("local")
    external.write_text("external")
    keys = {"local": "x", "registry:" + str(external): "y"}
    values = emitter.source_identities(tmp_path, keys)
    assert values["local"] == emitter.file_identity(local)
    assert values["registry:" + str(external)] == emitter.file_identity(external)


def test_tool_alias_retargeting_is_detected_even_when_bytes_match(tmp_path):
    first, second, alias = (tmp_path / name for name in ["first", "second", "alias"])
    first.write_bytes(b"same")
    second.write_bytes(b"same")
    alias.symlink_to(first)
    before = emitter.tool_identities({str(alias): "same hash"})
    alias.unlink()
    alias.symlink_to(second)
    assert emitter.tool_identities({str(alias): "same hash"}) != before


@pytest.mark.parametrize("name", ["RUSTFLAGS", "RUSTC_WRAPPER", "CARGO_TARGET_DIR", "CARGO_PROFILE_DEV_DEBUG", "CC", "LDFLAGS", "CC_aarch64_apple_darwin", "CARGO_BUILD_TARGET"])
def test_inherited_compiler_overrides_are_not_silently_adopted(monkeypatch, tmp_path, name):
    monkeypatch.setenv(name, "unreviewed")
    with pytest.raises(emitter.unit.Refused, match="unreviewed inherited"):
        emitter.build_environment(tmp_path / "rustc", tmp_path / "target", 2)


def test_run_text_preserves_natural_failure_without_signal(monkeypatch, tmp_path):
    calls = []
    def run(command, **kwargs):
        calls.append((command, kwargs))
        return subprocess.CompletedProcess(command, 23, "", "actual failure")
    monkeypatch.setattr(emitter.subprocess, "run", run)
    with pytest.raises(emitter.unit.Refused, match="actual failure"):
        emitter.run_text(["cargo", "build"], root=tmp_path)
    assert len(calls) == 1
    assert calls[0][1]["check"] is False
    assert "timeout" not in calls[0][1]


def test_non_macos_is_refused_before_creating_output(monkeypatch, tmp_path):
    monkeypatch.setattr(emitter.sys, "platform", "linux")
    output = tmp_path / "output"
    with pytest.raises(emitter.unit.Refused, match="macOS"):
        emitter.host_build(tmp_path, output, tmp_path / "target", 2)
    assert not output.exists()
