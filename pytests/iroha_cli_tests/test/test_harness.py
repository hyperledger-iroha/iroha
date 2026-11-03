"""Local controls for smoke input selection; these do not qualify native queries."""

import os
from pathlib import Path
import stat
import sys

import pytest
import tomlkit

from ..smoke import Localnet, load_localnet, peer_config, run_cli


@pytest.fixture
def generated_inputs(tmp_path):
    """Use harmless input text to test private TOML copying, never ledger responses."""
    binary = tmp_path / "iroha"
    binary.write_text("fixture only\n", encoding="utf-8")
    binary.chmod(0o700)
    config = tmp_path / "client.toml"
    config.write_text(
        'chain = "local-fixture"\n'
        'network_id_file = "genesis.expected.hash"\n'
        'torii_url = "http://127.0.0.1:8080/"\n'
        'http_transport_policy = "ordinary"\n'
        '[account]\npublic_key = "fixture-public"\n'
        'private_key = "fixture-private"\n',
        encoding="utf-8",
    )
    config.chmod(0o600)
    (tmp_path / "genesis.expected.hash").write_text("fixture-network\n")
    return Localnet(binary, config, (8080, 8081, 8082, 8083))


@pytest.mark.parametrize("peer", range(4))
def test_peer_toml_preserves_original_and_relative_identity(generated_inputs, peer):
    original = generated_inputs.config.read_bytes()
    with peer_config(generated_inputs, peer) as path:
        assert path.parent == generated_inputs.config.parent
        assert path != generated_inputs.config
        if os.name == "posix":
            assert stat.S_IMODE(path.stat().st_mode) == 0o600
        actual = tomlkit.parse(path.read_text(encoding="utf-8"))
        expected = tomlkit.parse(original.decode("utf-8"))
        assert actual["torii_url"] == f"http://127.0.0.1:{8080 + peer}/"
        actual["torii_url"] = expected["torii_url"]
        assert actual == expected
        assert (
            path.parent / actual["network_id_file"]
        ).read_text() == "fixture-network\n"
        assert generated_inputs.config.read_bytes() == original
    assert not path.exists()
    assert generated_inputs.config.read_bytes() == original


def test_peer_toml_removed_after_consumer_failure(generated_inputs):
    with pytest.raises(RuntimeError, match="consumer failed"):
        with peer_config(generated_inputs, 0) as path:
            raise RuntimeError("consumer failed")
    assert not path.exists()


@pytest.mark.parametrize(
    ("old", "new", "message"),
    [
        ("http://127.0.0.1:8080/", "https://example.invalid/", "loopback peer"),
        ("http://127.0.0.1:8080/", "http://127.0.0.1:8081/", "loopback peer"),
        ('"ordinary"', '"direct_loopback"', "transport policy"),
        ('"ordinary"', '"unknown"', "transport policy"),
        ('"ordinary"', "0", "transport policy"),
    ],
)
def test_foreign_configuration_is_not_retargeted(generated_inputs, old, new, message):
    config = generated_inputs.config
    config.write_text(config.read_text().replace(old, new), encoding="utf-8")
    with pytest.raises(ValueError, match=message):
        with peer_config(generated_inputs, 0):
            pytest.fail("foreign configuration was accepted")
    assert not list(config.parent.glob(".cli-smoke-*"))


@pytest.mark.parametrize("peer", [-1, 4])
def test_outside_committee_peer_is_refused(generated_inputs, peer):
    with pytest.raises(ValueError, match="outside"):
        with peer_config(generated_inputs, peer):
            pytest.fail("outside committee peer was accepted")


def test_existing_native_inputs_required(generated_inputs):
    root = generated_inputs.config.parent
    environ = {
        "TMP_DIR": ".",
        "IROHA_CLI_BINARY": "iroha",
        "IROHA_CLI_CONFIG": "client.toml",
    }
    assert load_localnet(environ, root) == generated_inputs
    generated_inputs.binary.unlink()
    with pytest.raises(FileNotFoundError):
        load_localnet(environ, root)


@pytest.mark.parametrize(
    "first,last", [(8080, 8080), (8080, 8082), (8080, 8084), (0, 3), (65533, 65536)]
)
def test_incorrect_committee_port_range_is_refused(generated_inputs, first, last):
    environ = {
        "TMP_DIR": ".",
        "IROHA_CLI_BINARY": "iroha",
        "IROHA_CLI_CONFIG": "client.toml",
        "TORII_API_PORT_MIN": str(first),
        "TORII_API_PORT_MAX": str(last),
    }
    with pytest.raises(ValueError, match="exactly four"):
        load_localnet(environ, generated_inputs.config.parent)


def test_native_process_failure_is_not_converted_to_success():
    result = run_cli(
        Path(sys.executable),
        "-I",
        "-S",
        "-c",
        "import sys; print(sys.argv[1]); print('failure', file=sys.stderr); sys.exit(23)",
        "argument with spaces",
    )
    assert result.returncode == 23
    assert result.stdout == "argument with spaces\n"
    assert result.stderr == "failure\n"
