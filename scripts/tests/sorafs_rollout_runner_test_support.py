"""Shared fixture support for SoraFS rollout evidence runner tests."""

from __future__ import annotations

import hashlib
import json
import tempfile
from pathlib import Path
from typing import Any, Callable, Sequence

from sorafs_topology_qualification import (
    CANONICAL_READINESS_LANES,
    SIGNED_QUALIFICATION_ENVELOPE_SCHEMA,
    SUMMARY_SCHEMA,
    load_topology_qualification_binding,
    topology_qualification_envelope_signing_bytes,
)
from sorafs_resilience_test_support import public_key_from_seed, sign


TOPOLOGY_SIGNING_SEED = hashlib.sha256(b"sorafs-topology-test-key").digest()
TOPOLOGY_VERIFICATION_PUBLIC_KEY = public_key_from_seed(TOPOLOGY_SIGNING_SEED)
TOPOLOGY_SIGNER_SERVICE_ID = "sorafs-topology-signer-a"
TOPOLOGY_SIGNER_ADMINISTRATOR_ID = "sorafs-topology-admin-b"
TOPOLOGY_SIGNER_KEY_REVISION = 3
TOPOLOGY_SIGNER_POLICY_REVISION = 5
TOPOLOGY_SIGNER_POLICY_DIGEST = hashlib.sha256(
    b"sorafs-topology-test-policy-v1"
).hexdigest()
TOPOLOGY_MAX_REVIEW_AGE_SECS = 3_600


def authenticated_topology_binding(binding: dict[str, Any]) -> dict[str, Any]:
    """Attach the test topology signer's payload-free public provenance."""

    return {
        **binding,
        "signer_authentication_kind": "external-ed25519",
        "signer_backend": "software",
        "signer_service_id": TOPOLOGY_SIGNER_SERVICE_ID,
        "signer_administrator_id": TOPOLOGY_SIGNER_ADMINISTRATOR_ID,
        "signer_key_revision": TOPOLOGY_SIGNER_KEY_REVISION,
        "signer_policy_revision": TOPOLOGY_SIGNER_POLICY_REVISION,
        "signer_policy_digest_sha256": TOPOLOGY_SIGNER_POLICY_DIGEST,
        "signer_public_key_fingerprint_sha256": hashlib.sha256(
            TOPOLOGY_VERIFICATION_PUBLIC_KEY
        ).hexdigest(),
    }


def write_topology_qualification(
    path: Path,
    *,
    deployment_id: str,
    environment: str = "production",
) -> Path:
    """Write one valid, explicitly non-promotable L1 topology summary."""

    manifest_binding = f"{deployment_id}:{environment}".encode()
    payload = {
        "schema": SUMMARY_SCHEMA,
        "status": "configuration-qualified",
        "qualification_scope": "pre-deployment-configuration",
        "live_evidence_recognized": False,
        "promotion_eligible": False,
        "manifest_sha256": hashlib.sha256(
            b"runner-test-manifest:" + manifest_binding
        ).hexdigest(),
        "canonical_manifest_sha256": hashlib.sha256(
            b"runner-test-canonical-manifest:" + manifest_binding
        ).hexdigest(),
        "deployment": {
            "deployment_id": deployment_id,
            "environment": environment,
            "network": "taira",
            "chain_id": "fc56984b-2be7-431d-840e-21514d1883f0",
            "chain_discriminant": 369,
        },
        "validator_count": 4, "validator_ids": ["taira-validator-1", "taira-validator-2", "taira-validator-3", "taira-validator-4"],
        "storage_provider_count": 2,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "runtime_handle_kinds": ["monitoring", "external_signer", "kms", "webauthn"],
        "runtime_material_policy_valid": True,
        "signed_model_artifact_count": 1,
        "required_lane_slots": list(CANONICAL_READINESS_LANES),
        "recognized_lane_slot_count": len(CANONICAL_READINESS_LANES),
        "errors": [],
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return path


def signed_topology_cli_args(
    qualification_path: Path,
    *,
    deployment_id: str,
    environment: str,
    now_unix: int,
) -> list[str]:
    """Sign an existing valid topology summary and return its public trust tuple."""

    binding, errors = load_topology_qualification_binding(
        qualification_path,
        expected_deployment_id=deployment_id,
        expected_environment=environment,
    )
    envelope_path = qualification_path.with_name(
        f"{qualification_path.name}.ed25519"
    )
    envelope: dict[str, Any] = {}
    if not errors and binding is not None:
        envelope = {
            "schema": SIGNED_QUALIFICATION_ENVELOPE_SCHEMA,
            **authenticated_topology_binding(binding),
            "reviewed_at_unix": now_unix - 60,
            "signature_algorithm": "ed25519",
            "signature_hex": "00" * 64,
        }
        envelope["signature_hex"] = sign(
            TOPOLOGY_SIGNING_SEED,
            topology_qualification_envelope_signing_bytes(envelope),
        ).hex()
    envelope_path.write_text(
        json.dumps(envelope, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return [
        "--topology-qualification-summary", str(qualification_path),
        "--topology-qualification-envelope", str(envelope_path),
        "--topology-qualification-verification-public-key-hex", TOPOLOGY_VERIFICATION_PUBLIC_KEY.hex(),
        "--topology-qualification-signer-service-id", TOPOLOGY_SIGNER_SERVICE_ID,
        "--topology-qualification-signer-administrator-id", TOPOLOGY_SIGNER_ADMINISTRATOR_ID,
        "--topology-qualification-signer-key-revision", str(TOPOLOGY_SIGNER_KEY_REVISION),
        "--topology-qualification-signer-policy-revision", str(TOPOLOGY_SIGNER_POLICY_REVISION),
        "--topology-qualification-signer-policy-digest-hex", TOPOLOGY_SIGNER_POLICY_DIGEST,
        "--max-topology-qualification-review-age-secs", str(TOPOLOGY_MAX_REVIEW_AGE_SECS),
    ]


class TopologyBoundChecker:
    """Invoke one lane checker with an explicit valid topology test fixture.

    Production parsers still require callers to provide the argument. This
    harness only keeps payload-validation tests focused on their intended
    mutations after the first-release topology binding became mandatory.
    """

    def __init__(
        self,
        checker_main: Callable[[list[str] | None], int],
        *,
        deployment_id: str,
        environment: str,
        name: str,
    ) -> None:
        self._checker_main = checker_main
        self._temporary_directory = tempfile.TemporaryDirectory(
            prefix=f"sorafs-{name}-topology-"
        )
        try:
            self.topology_path = write_topology_qualification(
                Path(self._temporary_directory.name).resolve() / "qualification.json",
                deployment_id=deployment_id,
                environment=environment,
            )
        except BaseException:
            self._temporary_directory.cleanup()
            raise

    def __call__(self, arguments: Sequence[str]) -> int:
        """Run the checker with the exact topology fixture when not supplied."""

        values = list(arguments)
        if "--topology-qualification-summary" not in values:
            values.extend(
                [
                    "--topology-qualification-summary",
                    str(self.topology_path),
                ]
            )
        return self._checker_main(values)

    def close(self) -> None:
        """Remove the private topology fixture deterministically."""

        self._temporary_directory.cleanup()
