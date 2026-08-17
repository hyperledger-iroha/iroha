"""Deterministic deploy-authority projections for the Taira reset controller.

This module deliberately does not import :mod:`deploy_taira_v21_reset`.  The
controller injects the artifact factory, canonical encoder, bounds, and
contract names so direct script execution cannot load a second copy of a
running ``__main__`` module.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Protocol


class _PeerPlan(Protocol):
    slug: str
    config: Path
    config_sha256: str


class _ExternalReleasePlan(Protocol):
    policy_path: Path
    qualification_seal_path: Path
    expected_policy_sha256: str
    policy_sha256: str | None
    qualification_seal_sha256: str | None
    manifest_directory_digests: tuple[str, ...]
    manifest_directory_inventory_sha256: str | None
    manifest_files: tuple[Path, ...]
    manifest_digest_sidecars: tuple[Path, ...]
    verified: bool


class _BundlePlan(Protocol):
    root: Path
    manifest_sha256: str
    peers: tuple[_PeerPlan, ...]
    kagemusha_config_projection_sha256: str | None
    kagemusha_external_release: _ExternalReleasePlan | None


class _StableArchive(Protocol):
    size: int


class _QualifiedHandoff(Protocol):
    root: Path


class _AdmissionPlan(Protocol):
    archive: Path
    archive_state: _StableArchive
    authority_dir: Path
    authority_state: tuple[tuple[str, object], ...]
    boi_qualified_handoff: _QualifiedHandoff
    archive_sha256: str
    artifact_handoff_sha256: str
    boi_qualified_inventory_sha256: str
    receipt_id: str
    release_manifest_sha256: str
    reset_manifest_sha256: str
    cargo_lock_sha256: str
    source_commit: str
    dpn_validator_release_commit: str
    workspace_source_manifest_sha256: str
    restart_generation: str


class _SourcePlan(Protocol):
    binary: Path
    binary_sha256: str
    supervisor: Path
    supervisor_sha256: str


@dataclass(frozen=True)
class ArtifactBounds:
    """Maximum bytes admitted for each deploy-authority artifact class."""

    binary: int
    config: int
    manifest: int
    supervisor: int
    kagemusha_policy: int
    kagemusha_qualification_seal: int
    kagemusha_manifest: int
    kagemusha_manifest_sidecar: int


@dataclass(frozen=True)
class AuthorityContracts:
    """Stable contract names embedded in the deploy-authority subject."""

    complete_source: str
    run_assignment: str
    lease_authorization: str
    result_binding: str


class DeploymentAuthorityProjection:
    """Build authority inputs without depending on controller module identity."""

    def __init__(
        self,
        *,
        artifact_factory: Callable[..., Any],
        canonical_json_bytes: Callable[[object], bytes],
        bounds: ArtifactBounds,
        contracts: AuthorityContracts,
        qualified_handoff_manifest: str,
        qualified_handoff_maximum: int,
    ) -> None:
        self._artifact = artifact_factory
        self._canonical_json_bytes = canonical_json_bytes
        self._bounds = bounds
        self._contracts = contracts
        self._qualified_handoff_manifest = qualified_handoff_manifest
        self._qualified_handoff_maximum = qualified_handoff_maximum

    @staticmethod
    def kagemusha_subject(bundle: _BundlePlan) -> dict[str, object]:
        """Return the digest-only Kagemusha deployment authority projection."""

        projection_sha256 = getattr(
            bundle, "kagemusha_config_projection_sha256", None
        )
        external = getattr(bundle, "kagemusha_external_release", None)
        if projection_sha256 is None:
            return {"configured": False}
        return {
            "configured": True,
            "config_projection_sha256": projection_sha256,
            "external_release_verified": bool(external and external.verified),
            "manifest_directory_digests": (
                list(external.manifest_directory_digests) if external else []
            ),
            "manifest_directory_inventory_sha256": (
                external.manifest_directory_inventory_sha256 if external else None
            ),
            "policy_sha256": external.expected_policy_sha256 if external else None,
            "qualification_seal_sha256": (
                external.qualification_seal_sha256 if external else None
            ),
        }

    def kagemusha_artifacts(self, bundle: _BundlePlan) -> tuple[Any, ...]:
        """Expose only bounded external Kagemusha bytes to deploy authority."""

        external = getattr(bundle, "kagemusha_external_release", None)
        if external is None:
            return ()
        artifacts: list[Any] = []
        if external.policy_sha256 is not None:
            artifacts.append(
                self._artifact(
                    "kagemusha/policy/release-policy-v1.norito",
                    external.policy_path,
                    maximum=self._bounds.kagemusha_policy,
                )
            )
        if external.qualification_seal_sha256 is not None:
            artifacts.append(
                self._artifact(
                    "kagemusha/seals/catalog-qualification-v1.norito",
                    external.qualification_seal_path,
                    maximum=self._bounds.kagemusha_qualification_seal,
                )
            )
        for digest, manifest_path, sidecar_path in zip(
            external.manifest_directory_digests,
            external.manifest_files,
            external.manifest_digest_sidecars,
        ):
            artifacts.extend(
                (
                    self._artifact(
                        f"kagemusha/catalog/{digest}/manifest.norito",
                        manifest_path,
                        maximum=self._bounds.kagemusha_manifest,
                    ),
                    self._artifact(
                        f"kagemusha/catalog/{digest}/manifest.norito.sha256",
                        sidecar_path,
                        maximum=self._bounds.kagemusha_manifest_sidecar,
                    ),
                )
            )
        return tuple(artifacts)

    @staticmethod
    def report_fields(
        bundle: _BundlePlan, *, exact_binary_config_verified: bool
    ) -> dict[str, object]:
        """Return unambiguous Kagemusha readiness fields for reports."""

        projection_sha256 = getattr(
            bundle, "kagemusha_config_projection_sha256", None
        )
        external = getattr(bundle, "kagemusha_external_release", None)
        configured = projection_sha256 is not None
        external_verified = bool(external and external.verified)
        if not configured:
            status = "not-configured"
        elif external_verified:
            status = "bounded-external-release-verified"
        else:
            status = "blocked-external-release-unavailable"
        return {
            "kagemusha_config_projection_sha256": projection_sha256,
            "kagemusha_exact_binary_config_verified": bool(
                configured and exact_binary_config_verified
            ),
            "kagemusha_external_release_status": status,
            "kagemusha_external_release_verified": external_verified,
            "kagemusha_manifest_directory_inventory_sha256": (
                external.manifest_directory_inventory_sha256 if external else None
            ),
            "kagemusha_qualification_seal_sha256": (
                external.qualification_seal_sha256 if external else None
            ),
        }

    def subject(
        self,
        admission: _AdmissionPlan,
        bundle: _BundlePlan,
        sources: _SourcePlan,
    ) -> dict[str, object]:
        """Build the stable digest-only subject shared by every lease phase."""

        return {
            "admission": {
                "archive_sha256": admission.archive_sha256,
                "artifact_handoff_sha256": admission.artifact_handoff_sha256,
                "boi_qualified_inventory_sha256": (
                    admission.boi_qualified_inventory_sha256
                ),
                "receipt_id": admission.receipt_id,
                "release_manifest_sha256": admission.release_manifest_sha256,
                "reset_manifest_sha256": admission.reset_manifest_sha256,
            },
            "bundle": {
                "kagemusha": self.kagemusha_subject(bundle),
                "manifest_sha256": bundle.manifest_sha256,
                "peer_config_sha256": {
                    peer.slug: peer.config_sha256 for peer in bundle.peers
                },
            },
            "contracts": {
                "complete_source": self._contracts.complete_source,
                "run_assignment": self._contracts.run_assignment,
            },
            "lease": {
                "authorization": self._contracts.lease_authorization,
                "result_binding": self._contracts.result_binding,
            },
            "source": {
                "cargo_lock_sha256": admission.cargo_lock_sha256,
                "commit": admission.source_commit,
                "dpn_validator_release_commit": (
                    admission.dpn_validator_release_commit
                ),
                "workspace_source_manifest_sha256": (
                    admission.workspace_source_manifest_sha256
                ),
            },
            "runtime": {
                "binary_sha256": sources.binary_sha256,
                "restart_generation": admission.restart_generation,
                "supervisor_sha256": sources.supervisor_sha256,
            },
        }

    def artifacts(
        self,
        admission: _AdmissionPlan,
        bundle: _BundlePlan,
        sources: _SourcePlan,
    ) -> tuple[Any, ...]:
        """Build the complete bounded artifact inventory for one lease."""

        artifacts = [
            self._artifact(
                "admission/archive",
                admission.archive,
                maximum=admission.archive_state.size,
            ),
            self._artifact(
                "qualified/handoff-inventory-v1.json",
                admission.boi_qualified_handoff.root
                / self._qualified_handoff_manifest,
                maximum=self._qualified_handoff_maximum,
            ),
            self._artifact(
                "runtime/iroha3d", sources.binary, maximum=self._bounds.binary
            ),
            self._artifact(
                "runtime/supervisor",
                sources.supervisor,
                maximum=self._bounds.supervisor,
            ),
            self._artifact(
                "bundle/reset-manifest.json",
                bundle.root / "reset-manifest.json",
                maximum=self._bounds.manifest,
            ),
        ]
        artifacts.extend(
            self._artifact(
                f"admission/authority/{relative}",
                admission.authority_dir / relative,
                maximum=self._bounds.manifest,
            )
            for relative, _state in admission.authority_state
        )
        artifacts.extend(
            self._artifact(
                f"bundle/config/{peer.slug}",
                peer.config,
                maximum=self._bounds.config,
            )
            for peer in bundle.peers
        )
        artifacts.extend(self.kagemusha_artifacts(bundle))
        return tuple(artifacts)

    def result_sha256(self, outcome: str, result: dict[str, object]) -> str:
        """Digest the exact terminal result without a circular sidecar."""

        payload = {
            "outcome": outcome,
            "result": result,
            "schema": self._contracts.result_binding,
        }
        return hashlib.sha256(self._canonical_json_bytes(payload)).hexdigest()
