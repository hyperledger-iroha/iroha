"""Shared fixture support for SoraFS rollout evidence runner tests."""

from __future__ import annotations

import hashlib
import json
import tempfile
from pathlib import Path
from typing import Callable, Sequence

from sorafs_topology_qualification import (
    CANONICAL_READINESS_LANES,
    SUMMARY_SCHEMA,
)


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
        },
        "validator_count": 4,
        "storage_provider_count": 2,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "runtime_handle_kinds": ["monitoring", "hsm", "kms", "webauthn"],
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
        self.topology_path = write_topology_qualification(
            Path(self._temporary_directory.name).resolve() / "qualification.json",
            deployment_id=deployment_id,
            environment=environment,
        )

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
