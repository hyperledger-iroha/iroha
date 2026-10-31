"""Tests for the contributor environment check."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("dev_doctor", ROOT / "scripts" / "dev_doctor.py")
assert SPEC is not None and SPEC.loader is not None
doctor = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = doctor
SPEC.loader.exec_module(doctor)


def fake_which(*missing):
    return lambda tool: None if tool in missing else f"/usr/bin/{tool}"


def fake_run(python_version="Python 3.12.1", modules_ok=True, base_ok=True):
    def run(command):
        command = list(command)
        if command[:3] == ["rustup", "toolchain", "list"]:
            return 0, "1.99.1-aarch64-apple-darwin (default)"
        if command[:2] == ["rustup", "component"]:
            return 0, "rustfmt-aarch64-apple-darwin\nclippy-aarch64-apple-darwin"
        if command == ["python3", "--version"]:
            return 0, python_version
        if command[:2] == ["python3", "-c"]:
            return (0, "") if modules_ok else (1, "ModuleNotFoundError: No module named 'pytest'")
        if command[:2] == ["git", "rev-parse"]:
            return (0, "abc") if base_ok else (1, "")
        raise AssertionError(command)

    return run


def test_missing_ripgrep_is_a_failure_with_a_fix():
    finding = doctor.check_ripgrep(fake_which("rg"))
    assert finding.status == doctor.FAIL
    assert "brew install ripgrep" in finding.fix
    assert doctor.check_ripgrep(fake_which()).status == doctor.OK


def test_python_below_the_floor_fails():
    findings = doctor.check_python(fake_which(), fake_run(python_version="Python 3.9.6"))
    assert [finding.status for finding in findings] == [doctor.FAIL]
    assert "3.9 is below the 3.11" in findings[0].detail


def test_python_at_the_floor_passes_and_reports_missing_packages():
    findings = doctor.check_python(fake_which(), fake_run(python_version="Python 3.11.4"))
    assert [finding.status for finding in findings] == [doctor.OK]
    findings = doctor.check_python(fake_which(), fake_run(modules_ok=False))
    assert [finding.status for finding in findings] == [doctor.OK, doctor.WARN]
    assert "requirements.txt" in findings[1].fix


def test_missing_toolchain_prints_the_install_command():
    def run(command):
        return 0, "stable-aarch64-apple-darwin (default)"

    findings = doctor.check_rust(fake_which(), run, "1.99.1")
    assert findings[0].status == doctor.FAIL
    assert "rustup toolchain install 1.99.1" in findings[0].fix


def test_missing_rust_components_fail():
    def run(command):
        if list(command)[:2] == ["rustup", "component"]:
            return 0, "rustfmt-aarch64-apple-darwin"
        return 0, "1.99.1-aarch64-apple-darwin"

    findings = doctor.check_rust(fake_which(), run, "1.99.1")
    assert [(finding.status, finding.name) for finding in findings] == [
        (doctor.OK, "toolchain"),
        (doctor.FAIL, "clippy"),
    ]


def test_missing_base_ref_points_at_the_fetch():
    finding = doctor.check_base_ref(fake_run(base_ok=False), "origin/optimizations")
    assert finding.status == doctor.WARN
    assert finding.fix == "git fetch origin optimizations"
    assert doctor.check_base_ref(fake_run(), "origin/optimizations").status == doctor.OK


def test_configured_base_ref_skips_comments(tmp_path):
    (tmp_path / "ci").mkdir()
    (tmp_path / "ci" / "default_base_ref").write_text("# why\n\norigin/next  # trailing\n")
    assert doctor.configured_base_ref(tmp_path) == "origin/next"
    assert doctor.configured_base_ref(tmp_path / "missing") is None


def test_repository_records_a_base_ref_and_a_toolchain():
    assert doctor.configured_base_ref() == "origin/optimizations"
    assert doctor.pinned_toolchain() is not None


def test_only_one_of_the_localnet_binaries_is_reported(tmp_path):
    debug = tmp_path / "debug"
    debug.mkdir()
    assert doctor.check_localnet_binaries(tmp_path) is None
    (debug / "kagami").write_text("")
    finding = doctor.check_localnet_binaries(tmp_path)
    assert finding.status == doctor.WARN
    assert finding.fix == "cargo build -p iroha_kagami -p irohad"
    (debug / "iroha3d").write_text("")
    assert doctor.check_localnet_binaries(tmp_path) is None


def test_low_disk_warns():
    assert doctor.check_disk(10 * 1024**3).status == doctor.WARN
    assert doctor.check_disk(500 * 1024**3).status == doctor.OK


def test_collect_reports_failures_and_optional_notes(tmp_path):
    (tmp_path / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.99.1"\n')
    findings = doctor.collect(fake_which("rg", "docker"), fake_run(), tmp_path)
    statuses = {finding.name: finding.status for finding in findings}
    assert statuses["rg"] == doctor.FAIL
    assert statuses["docker"] == doctor.NOTE
    assert "sccache" not in statuses


def test_render_includes_the_fix_on_its_own_line():
    text = doctor.render(doctor.Finding(doctor.FAIL, "rg", "not found", "brew install ripgrep"))
    assert text == "FAIL rg: not found\n     fix: brew install ripgrep"
