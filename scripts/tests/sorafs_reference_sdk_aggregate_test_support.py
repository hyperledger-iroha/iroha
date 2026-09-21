"""Test-only SF-11 source result for synthetic aggregate checker composition.

This double supplies the authenticated-source boundary to the aggregate unit
fixture, like its separate supply-chain source double. It performs no native
cryptographic verification and produces no release qualification evidence.
The signed-manifest and native receipt suites exercise that boundary directly.
"""

from pathlib import Path
from types import ModuleType

from taira_constants import CHAIN_ID
from sorafs_reference_sdk_signed_manifest import (
    NATIVE_RECEIPT_SCHEMA,
    NATIVE_SOURCE_FIELDS,
    VerifiedSignedManifestSources,
)


def install_synthetic_manifest_source_result(
    fixture_module: ModuleType,
    evidence_root: Path,
    *,
    deployment_id: str,
    environment: str,
) -> None:
    """Install one explicitly simulated source result on this fixture's checker only."""

    digest = fixture_module.DIGEST
    native_fields = {
        **{field: digest for field in NATIVE_SOURCE_FIELDS},
        "schema": NATIVE_RECEIPT_SCHEMA,
        "status": "verified",
        "manifest_size": 1,
        "operation_id": digest,
        "custody_record_digest": digest,
        "policy_digest": digest,
        "key_revision": 1,
        "policy_revision": 1,
        "service_id": "sf11-release-signer",
        "administrator_id": "sf11-release-administrator",
        "role": "release_manifest",
        "deployment_id": deployment_id,
        "chain_id": CHAIN_ID,
        "network_id": digest,
        "finalized_height": 1,
        "finalized_block_hash": digest,
        "verified_at_unix_ms": fixture_module.NOW_UNIX * 1_000,
    }
    verified = VerifiedSignedManifestSources(
        digest, digest, tuple(sorted(native_fields.items()))
    )

    def synthetic_authenticator(_context, _context_digest, now_unix):
        assert now_unix == fixture_module.NOW_UNIX
        return verified

    fixture_module.MODULE.authenticate_signed_manifest_sources = synthetic_authenticator
    payload = fixture_module.base("sorafs.reference_sdk.signed_manifest_canary.v1")
    payload.update(verified.canary_fields())
    payload["environment"] = environment
    fixture_module.write_json(evidence_root / "signed-manifest.json", payload)
