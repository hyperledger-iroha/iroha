"""Exercise the native CLI and real four-peer queries; no simulated responses.

Run `scripts/test_env.py setup`, export its printed environment, then run
`poetry run pytest` in `pytests/iroha_cli_tests`. CI supplies the same inputs.
"""

import json
from pathlib import Path
import tomllib

from ..smoke import run_cli


def test_native_cli_version(localnet):
    """The delivered native executable reports this checkout's package version."""
    manifest = Path(__file__).resolve().parents[3] / "Cargo.toml"
    with manifest.open("rb") as source:
        version = tomllib.load(source)["workspace"]["package"]["version"]
    result = run_cli(localnet.binary, "--version")
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == f"iroha {version}"


def test_signed_genesis_domain_query(localnet, client_config):
    """Every peer answers a signed current-API query for the generated domain."""
    result = run_cli(
        localnet.binary,
        "--config",
        str(client_config),
        "--machine",
        "ledger",
        "domain",
        "get",
        "--id",
        "universal.universal",
    )
    assert result.returncode == 0, result.stderr
    domain = json.loads(result.stdout)
    assert domain["id"] == "universal.universal"
    assert isinstance(domain["owned_by"], str) and domain["owned_by"]


def test_bounded_account_collection(localnet, client_config):
    """The current list command returns an actual nonempty bounded page."""
    result = run_cli(
        localnet.binary,
        "--config",
        str(client_config),
        "--machine",
        "account",
        "list",
        "--limit",
        "1",
        "--select",
        "id",
        "--include-total",
    )
    assert result.returncode == 0, result.stderr
    page = json.loads(result.stdout)
    assert len(page["items"]) == 1
    assert set(page["items"][0]) == {"id"}
    assert isinstance(page["items"][0]["id"], str) and page["items"][0]["id"]
    assert type(page["total"]) is int and page["total"] >= 1
    assert page["next_cursor"] is None or isinstance(page["next_cursor"], str)


def test_invalid_collection_limit_is_refused(localnet, client_config):
    """Invalid current-API input keeps its dedicated nonzero exit status."""
    result = run_cli(
        localnet.binary,
        "--config",
        str(client_config),
        "--machine",
        "account",
        "list",
        "--limit",
        "0",
    )
    assert result.returncode == 4, result.stderr
    assert "invalid_limit" in result.stderr
    assert result.stdout == ""
