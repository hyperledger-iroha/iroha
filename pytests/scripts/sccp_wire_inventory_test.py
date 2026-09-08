"""Check circuit and contract wire constants against the Rust-owned transfer fixture.

This read-only source inventory complements the circuit solver and contract
runtime tests; it does not interpret source declarations in production.
"""

from __future__ import annotations

import ast
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def _constant(path: str, name: str) -> int:
    source = (ROOT / path).read_text(encoding="utf-8")
    matches = re.findall(
        rf"^\s*(?:pub\s+)?(?:const\s+|uint(?:8|32)\s+(?:internal|private)\s+constant\s+)?"
        rf"{re.escape(name)}(?:\s*:\s*u(?:8|32)|\s+(?:byte|uint32))?\s*=\s*(\d+)\s*;?\s*$",
        source,
        re.MULTILINE,
    )
    assert len(matches) == 1, (path, name, matches)
    return int(matches[0])


def test_circuit_and_contract_wire_inventory_matches_rust_transfer_fixture() -> None:
    fixture = json.loads((ROOT / "fixtures/sccp/native_transfer_event_v1.json").read_text())
    assert fixture["version"] == 1
    vectors = {entry["source_profile"]: entry for entry in fixture["vectors"]}
    assert set(vectors) == {"ethereum-mainnet", "bsc-mainnet", "tron-mainnet", "ton-mainnet"}
    domains: dict[str, int] = {}
    codecs: dict[str, int] = {}
    for profile, entry in vectors.items():
        payload = bytes.fromhex(entry["canonical_payload_hex"])
        domains[profile] = int.from_bytes(payload[2:6], "little")
        codecs[profile] = payload[50]
        assert payload[0:2] == bytes((2, 1))
        assert payload[26] == 1
        recipient_offset = 55 + int.from_bytes(payload[51:55], "little")
        assert payload[recipient_offset] == 1

    assert domains == {
        "ethereum-mainnet": 1,
        "bsc-mainnet": 2,
        "tron-mainnet": 5,
        "ton-mainnet": 4,
    }
    assert codecs == {
        "ethereum-mainnet": 2,
        "bsc-mainnet": 2,
        "tron-mainnet": 5,
        "ton-mainnet": 7,
    }
    inventories = {
        "crates/iroha_sccp/src/lib.rs": {
            "SCCP_DOMAIN_TRON": domains["tron-mainnet"],
            "SCCP_CODEC_CANONICAL_TEXT": 1,
            "SCCP_CODEC_EVM_ADDRESS20": codecs["ethereum-mainnet"],
            "SCCP_CODEC_TRON_ADDRESS21": codecs["tron-mainnet"],
            "SCCP_CODEC_TON_ACCOUNT36": codecs["ton-mainnet"],
        },
        "circuits/sccp/internal/profile/profile.go": {
            "TRONDomain": domains["tron-mainnet"],
            "CanonicalTextCodec": 1,
            "EVMAddress20Codec": codecs["ethereum-mainnet"],
            "TRONAddress21Codec": codecs["tron-mainnet"],
            "TONAccount36Codec": codecs["ton-mainnet"],
            "TransferPayloadDiscriminant": 2,
            "TransferHubMessageKind": 5,
            "EVMBackendTag": 0,
            "TRONBackendTag": 1,
            "TONBackendTag": 2,
        },
        "contracts/evm/sccp/SccpExactTransferCodec.sol": {
            "CODEC_CANONICAL_TEXT": 1,
            "CODEC_EVM_ADDRESS20": codecs["ethereum-mainnet"],
            "CODEC_TRON_ADDRESS21": codecs["tron-mainnet"],
        },
        "contracts/evm/sccp/TairaXorExactEvmSccpBridge.sol": {
            "CODEC_TEXT": 1,
            "CODEC_EVM20": codecs["ethereum-mainnet"],
        },
        "contracts/tron/sccp/TairaXorSccpBridge.sol": {
            "DOMAIN_TRON": domains["tron-mainnet"],
            "CODEC_TEXT": 1,
            "CODEC_TRON21": codecs["tron-mainnet"],
        },
        "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol": {
            "SCCP_DOMAIN_TRON": domains["tron-mainnet"],
        },
        "contracts/ton/sccp/contracts/constants.tolk": {
            "SCCP_CODEC_CANONICAL_TEXT": 1,
            "SCCP_CODEC_TON_ADDRESS36": codecs["ton-mainnet"],
            "SCCP_PAYLOAD_DISCRIMINATOR": 2,
            "SCCP_DESTINATION_PROOF_BACKEND": 2,
        },
    }
    for path, inventory in inventories.items():
        for name, expected in inventory.items():
            assert _constant(path, name) == expected, (path, name, expected)
    module = ast.parse((ROOT / "scripts/sccp_release_common.py").read_text())
    declared = [
        ast.literal_eval(node.value)
        for node in module.body
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Name) and target.id == "PROFILE_DOMAINS"
            for target in node.targets
        )
    ]
    assert declared == [domains]
