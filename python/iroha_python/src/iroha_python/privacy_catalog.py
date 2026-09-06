"""Closed protocol identifiers for the first-release Exact12 catalog.

The authoritative capability surface is the canonical
``PrivacyExact12CapabilityManifestV1`` Norito archive exposed by the native
bridge. There is deliberately no parallel JSON snapshot decoder in V1.
"""

from __future__ import annotations

from typing import Literal

PrivacyProtocolIdV1 = Literal[
    "zk-ace-pq-authorization-v1",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v1",
    "iroha-zk-x509-stark-p256-v1",
    "iroha-jindo-polynomial-commitment-v1",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v1",
]

PRIVACY_PROTOCOL_IDS_V1: tuple[PrivacyProtocolIdV1, ...] = (
    "zk-ace-pq-authorization-v1",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v1",
    "iroha-zk-x509-stark-p256-v1",
    "iroha-jindo-polynomial-commitment-v1",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v1",
)

__all__ = ["PRIVACY_PROTOCOL_IDS_V1", "PrivacyProtocolIdV1"]
