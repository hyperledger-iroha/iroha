"""Read the four real peers started by ``scripts/test_env.py setup``.

These checks exercise current public HTTP routes. They neither submit transactions
nor establish whole-network or release qualification.
"""

import time

import pytest
import requests

# Keep these aligned with scripts/test_env.py's generated four-validator network.
TORII_URLS = tuple(f"http://127.0.0.1:{port}" for port in range(8080, 8084))
SCHEMA_KINDS = (
    "structs",
    "tuples",
    "enums",
    "ints",
    "floats",
    "strings",
    "bools",
    "fixed_points",
    "arrays",
    "vecs",
    "maps",
    "options",
    "results",
    "bitmaps",
)


@pytest.fixture(params=TORII_URLS)
def torii_url(request):
    """Select every generated peer, without silently reducing the committee."""
    return request.param


def _get(torii_url, path, *, accept="application/json", wait_for_genesis=False):
    """Bound reads, retrying only the version route's explicit startup response."""
    deadline = time.monotonic() + 30
    while True:
        remaining = deadline - time.monotonic()
        assert remaining > 0, f"{torii_url}{path}: genesis was not applied within 30s"
        response = requests.get(
            torii_url + path,
            headers={"Accept": accept},
            timeout=(min(5, remaining), min(10, remaining)),
            allow_redirects=False,
        )
        if (
            wait_for_genesis
            and response.status_code == 503
            and response.text == "genesis not applied"
        ):
            time.sleep(min(0.1, max(0, deadline - time.monotonic())))
            continue
        assert (
            response.status_code == 200
        ), f"{torii_url}{path}: HTTP {response.status_code}: {response.text[:256]}"
        return response


def test_peer_health(torii_url):
    """Each peer serves the current liveness response, without redirects."""
    response = _get(torii_url, "/health", accept="text/plain")
    assert response.headers["Content-Type"].split(";")[0] == "text/plain"
    assert response.text == "Healthy"


def test_peer_canonical_block_wire_version(torii_url):
    """The applied-genesis route advertises the sole canonical block-wire V1."""
    response = _get(
        torii_url, "/v1/api/version", accept="text/plain", wait_for_genesis=True
    )
    assert response.headers["Content-Type"].split(";")[0] == "text/plain"
    assert response.text == "1"


def test_peer_status(torii_url):
    """After genesis, each node reports a real committed height and build identity."""
    test_peer_canonical_block_wire_version(torii_url)
    response = _get(torii_url, "/status")
    assert response.headers["Content-Type"].split(";")[0] == "application/json"
    status = response.json()
    for field in (
        "blocks",
        "blocks_non_empty",
        "peers",
        "txs_approved",
        "txs_rejected",
    ):
        assert type(status[field]) is int and status[field] >= 0, field
    assert status["blocks"] >= 1
    assert status["blocks_non_empty"] <= status["blocks"]
    assert status["peers"] <= 3
    assert type(status["observed_at_ms"]) is int and status["observed_at_ms"] > 0
    build = status["build"]
    assert isinstance(build["version"], str) and build["version"]
    assert isinstance(build["target_triple"], str) and build["target_triple"]
    wire_hash = build["wire_schema_hash"]
    assert isinstance(wire_hash, str) and len(wire_hash) == 64
    assert all(char in "0123456789abcdef" for char in wire_hash)


def test_peer_schema(torii_url):
    """The current schema summary accounts for every advertised type exactly once."""
    response = _get(torii_url, "/v1/schema")
    assert response.headers["Content-Type"].split(";")[0] == "application/json"
    schema = response.json()
    for kind in ("types_total", *SCHEMA_KINDS):
        assert type(schema[kind]) is int and schema[kind] >= 0, kind
    assert schema["types_total"] > 0
    assert sum(schema[kind] for kind in SCHEMA_KINDS) == schema["types_total"]
    sample = schema["type_names_sample"]
    assert len(sample) == min(64, schema["types_total"])
    assert all(isinstance(name, str) and name for name in sample)
    assert sample == sorted(sample)
