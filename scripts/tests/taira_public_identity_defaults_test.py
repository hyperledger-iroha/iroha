"""Semantic guards for the canonical public Taira identity defaults."""

from __future__ import annotations

import binascii
import importlib.util
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONSTANTS_PATH = ROOT / "scripts/taira_constants.py"
SPEC = importlib.util.spec_from_file_location("taira_constants", CONSTANTS_PATH)
CONSTANTS = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = CONSTANTS
SPEC.loader.exec_module(CONSTANTS)

LIVE_GENESIS_HASH = (
    "0466da18c70ca8cbd51b8cc60b1d4a4802fc5d7f928d505806d7cd6cb61d60ef"
)
LIVE_NETWORK_ID = (
    "hash:0466DA18C70CA8CBD51B8CC60B1D4A4802FC5D7F928D505806D7CD6CB61D60EF#BA85"
)
RETIRED_NETWORK_ID = (
    "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
)


def _independent_network_id(genesis_hash: str) -> str:
    body = genesis_hash.upper()
    payload = f"hash:{body}".encode("ascii")
    return f"hash:{body}#{binascii.crc_hqx(payload, 0xFFFF):04X}"


def _rust_string_constant(source: str, name: str) -> str:
    match = re.search(
        rf'pub const {re.escape(name)}: &str =\s*"([^"]+)";', source
    )
    assert match is not None, f"missing Rust string constant {name}"
    return match.group(1)


def _toml_string_assignment(source: str, name: str) -> str:
    matches = re.findall(rf'^\s*{re.escape(name)}\s*=\s*"([^"]+)"\s*$', source, re.M)
    assert len(matches) == 1, f"expected exactly one TOML assignment for {name}"
    return matches[0]


def test_public_taira_network_id_is_derived_from_the_live_genesis() -> None:
    assert CONSTANTS.GENESIS_HASH == LIVE_GENESIS_HASH
    assert _independent_network_id(CONSTANTS.GENESIS_HASH) == LIVE_NETWORK_ID
    assert CONSTANTS.network_id_from_genesis_hash(CONSTANTS.GENESIS_HASH) == LIVE_NETWORK_ID
    assert CONSTANTS.NETWORK_ID == LIVE_NETWORK_ID
    assert CONSTANTS.canonical_network_id(CONSTANTS.NETWORK_ID) == LIVE_NETWORK_ID


def test_every_canonical_public_default_uses_the_same_live_identity() -> None:
    rust_source = (ROOT / "crates/iroha_sccp/src/lib.rs").read_text(encoding="utf-8")
    canary_source = (
        ROOT / "configs/soranexus/taira/taira-canary-client.example.toml"
    ).read_text(encoding="utf-8")
    explorer = json.loads(
        (ROOT / "configs/soranexus/taira/explorer.runtime-config.json").read_text(
            encoding="utf-8"
        )
    )

    defaults = {
        "python": CONSTANTS.NETWORK_ID,
        "sccp": _rust_string_constant(
            rust_source, "SCCP_TAIRA_FINALITY_NETWORK_ID_V1"
        ),
        "canary": _toml_string_assignment(canary_source, "network_id"),
        "explorer": explorer["networkId"],
    }
    assert set(defaults.values()) == {LIVE_NETWORK_ID}, defaults
    assert (
        _rust_string_constant(rust_source, "SCCP_TAIRA_GENESIS_HASH_V1")
        == LIVE_GENESIS_HASH
    )

    for path in (
        CONSTANTS_PATH,
        ROOT / "crates/iroha_sccp/src/lib.rs",
        ROOT / "configs/soranexus/taira/taira-canary-client.example.toml",
        ROOT / "configs/soranexus/taira/explorer.runtime-config.json",
    ):
        assert RETIRED_NETWORK_ID not in path.read_text(encoding="utf-8")
