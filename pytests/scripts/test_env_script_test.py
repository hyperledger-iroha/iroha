"""Tests for the disposable network helper used by the Python suites."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("iroha_test_env", ROOT / "scripts" / "test_env.py")
assert SPEC is not None and SPEC.loader is not None
test_env = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = test_env
SPEC.loader.exec_module(test_env)


def test_binary_override_wins_and_missing_binary_names_the_build_command(tmp_path):
    binary = tmp_path / "iroha3d"
    binary.write_text("")
    assert test_env.binary_path("iroha3d", "BIN_IROHAD", {"BIN_IROHAD": str(binary)}) == binary
    with pytest.raises(test_env.TestEnvError, match="cargo build -p irohad"):
        test_env.binary_path("iroha3d", "BIN_IROHAD", {"BIN_IROHAD": str(tmp_path / "missing")})


def test_default_binary_follows_the_cargo_target_dir(tmp_path):
    debug = tmp_path / "debug"
    debug.mkdir()
    (debug / "kagami").write_text("")
    selected = test_env.binary_path("kagami", "BIN_KAGAMI", {"CARGO_TARGET_DIR": str(tmp_path)})
    assert selected == debug / "kagami"


def test_generate_command_requests_the_ports_the_suites_expect(tmp_path):
    command = test_env.generate_command(Path("/bin/kagami"), tmp_path)
    assert command[:3] == ["/bin/kagami", "localnet", "generate"]
    assert command[command.index("--base-api-port") + 1] == "8080"
    assert command[command.index("--out-dir") + 1] == str(tmp_path)
    assert test_env.torii_ports() == [8080, 8081, 8082, 8083]


def test_env_lines_match_the_cli_suite_settings(tmp_path):
    lines = test_env.env_lines(tmp_path)
    assert f"TMP_DIR={tmp_path}" in lines
    assert "IROHA_CLI_BINARY=iroha" in lines
    assert "IROHA_CLI_CONFIG=client.toml" in lines
    assert "TORII_API_PORT_MAX=8083" in lines


def test_cleanup_refuses_a_directory_it_did_not_create(tmp_path):
    (tmp_path / "keep").write_text("user data")
    with pytest.raises(test_env.TestEnvError, match="refusing"):
        test_env.cleanup(tmp_path)
    assert (tmp_path / "keep").exists()


def test_cleanup_of_a_missing_directory_is_a_no_op(tmp_path):
    test_env.cleanup(tmp_path / "absent")


def test_setup_refuses_an_existing_directory(tmp_path):
    with pytest.raises(test_env.TestEnvError, match="already exists"):
        test_env.setup(tmp_path)
