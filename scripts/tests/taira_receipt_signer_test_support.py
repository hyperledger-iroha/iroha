"""Shared canonical Taira receipt-signer fixtures for focused script tests."""

from __future__ import annotations

from scripts import deploy_taira_v21_reset as deploy


def receipt_keypair(index: int) -> tuple[str, str, str]:
    """Return deterministic canonical secp256k1 config keys and node ID."""

    private_payload = index.to_bytes(32, "big")
    public_payload = deploy.validator_renderer._secp256k1_public_payload(private_payload)
    public_key = (
        deploy.validator_renderer.RECEIPT_PUBLIC_KEY_PREFIX
        + public_payload.hex().upper()
    )
    private_key = (
        deploy.validator_renderer.RECEIPT_PRIVATE_KEY_PREFIX
        + private_payload.hex().upper()
    )
    return public_key, private_key, deploy.validator_renderer.receipt_node_id(public_key)


def receipt_signer_map() -> dict[str, dict[str, object]]:
    """Return the exact ordered four-validator public signer projection."""

    result: dict[str, dict[str, object]] = {}
    for index, slug in enumerate(deploy.SLUGS, start=1):
        public_key, _, node_id = receipt_keypair(index)
        result[slug] = {
            "node_id": node_id,
            "public_key": {
                "algorithm": "secp256k1",
                "payload_hex": public_key[
                    len(deploy.validator_renderer.RECEIPT_PUBLIC_KEY_PREFIX) :
                ].lower(),
            },
        }
    return result


def projection_config_text() -> str:
    """Return one complete deploy projection with an explicit receipt signer."""

    public_key, private_key, _ = receipt_keypair(1)
    expected_hash = deploy.validator_renderer._format_literal(
        "hash", ("00" * 31 + "01").upper()
    )
    return f'''chain = "{deploy.CHAIN_ID}"
chain_discriminant = {deploy.CHAIN_DISCRIMINANT}
trusted_peers = [
  "peer-one",
]

[network]
address = "addr:127.0.0.1:1337#ABCD"

[torii]
address = "addr:127.0.0.1:8080#1234"
receipt_public_key = "{public_key}"
receipt_private_key = "{private_key}"

[nexus.storage]
local_budget_bytes = {deploy.NODE_STORAGE_BUDGET_BYTES}

[nexus.storage.disk_budget_weights]
kura_blocks_bps = 7499
wsv_snapshots_bps = 2000
sorafs_bps = 1
soranet_spool_bps = 250
soravpn_spool_bps = 250

[genesis]
file = "/private/reset/genesis.signed.nrt"
public_key = "ed0120{'AB' * 32}"
expected_hash = "{expected_hash}"
'''
