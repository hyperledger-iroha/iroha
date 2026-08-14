"""Runtime-only exact operator signer for ISO 20022 adapter requests."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class OperatorSigningContext:
    """Network-bound SDK key pair used to generate fresh operator headers."""

    network_id: Any
    key_pair: Any

    def headers(self, method: str, path: str, body: bytes) -> dict[str, str]:
        """Sign the exact canonical request through the maintained Python SDK."""

        from iroha_python import ToriiClient

        return ToriiClient.build_operator_signature_headers(
            network_id=self.network_id,
            method=method,
            path=path,
            body=body,
            key_pair=self.key_pair,
        )


def load_operator_signing_context(
    network_id_literal: str,
    private_key_literal: str,
) -> OperatorSigningContext:
    """Parse exact runtime signing inputs without persisting them."""

    from iroha_python import NetworkId
    from iroha_python.crypto import CryptoKeyPair, Ed25519KeyPair

    network_id = NetworkId.parse(network_id_literal)
    try:
        key_pair = CryptoKeyPair.from_private_key_multihash(private_key_literal)
    except Exception as multihash_error:
        try:
            raw = bytes.fromhex(private_key_literal)
        except ValueError as error:
            raise ValueError(
                "operator private key must be a multihash or raw hex"
            ) from error
        if len(raw) != 32:
            raise ValueError(
                "raw Ed25519 operator private key must be 32 bytes"
            ) from multihash_error
        key_pair = Ed25519KeyPair.from_private_key(raw)
    return OperatorSigningContext(network_id=network_id, key_pair=key_pair)
