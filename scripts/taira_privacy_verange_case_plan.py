#!/usr/bin/env python3
"""Immutable, non-executing VeRange qualification case planning.

This module binds public action-driver setup requirements to an authenticated
Taira reset plan and live supervisor launch contracts.  It deliberately does
not sign setup instructions, contact Torii, restart a validator, register a
controller case, or issue evidence.  A reset manifest that lacks the future
source-closed qualification-genesis row fails closed.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, NoReturn, Sequence

try:
    from . import deploy_taira_v21_reset as deploy
except ImportError:
    import deploy_taira_v21_reset as deploy


PLAN_SCHEMA = "iroha.taira.verange_qualification_case_plan"
PLAN_SCHEMA_VERSION = 1
GENESIS_PLAN_SCHEMA = "iroha.taira.verange_qualification_genesis_plan"
GENESIS_PLAN_SCHEMA_VERSION = 1
GENESIS_PLAN_MANIFEST_FIELD = "privacy_qualification_setup"
PEER_COUNT = 4
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
MAX_TEXT_BYTES = 4096
MAX_ARGV_ITEMS = 128
MAX_ARG_BYTES = 4096


class VeRangeQualificationPlanError(RuntimeError):
    """The public qualification plan is incomplete, mutable, or substituted."""


def _fail(message: str) -> NoReturn:
    raise VeRangeQualificationPlanError(message)


def _sha256(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
    return value


def _commit(value: object, label: str) -> str:
    if not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None:
        _fail(f"{label} must be one full lowercase source commit")
    return value


def _text(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or not value.isascii()
        or len(value.encode("ascii")) > MAX_TEXT_BYTES
    ):
        _fail(f"{label} must be bounded nonempty ASCII")
    return value


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise VeRangeQualificationPlanError(
            f"VeRange qualification plan is not canonically encodable: {exc}"
        ) from exc


def _absolute_path(value: object, label: str) -> str:
    path = Path(value) if isinstance(value, (str, os.PathLike)) else None
    if (
        path is None
        or not path.is_absolute()
        or str(path) != os.path.normpath(str(path))
    ):
        _fail(f"{label} must be one normalized absolute path")
    return str(path)


@dataclass(frozen=True)
class VeRangeQualificationSetupRequirementsV1:
    action_authority_account_id: str
    action_authority_public_key_hex: str
    activation_height_rule: str
    activation_instruction: str
    activation_lifecycle: str
    activation_minimum_delay_blocks: int
    activation_template_activate_at_height: int
    activation_template_norito: bytes
    activation_template_proposed_at_height: int
    activation_template_sha256: str
    asset_definition_id: str
    candidate_binding_sha256: str
    compiled_profile_sha256: str
    domain_id: str
    governance_permission: str
    protocol_id: str
    schema: str
    schema_version: int
    setup_authority_account_id: str
    setup_authority_public_key_hex: str
    setup_identity_binding_sha256: str

    @classmethod
    def from_ipc(
        cls, value: Mapping[str, object]
    ) -> "VeRangeQualificationSetupRequirementsV1":
        return cls(
            action_authority_account_id=str(value["action_authority_account_id"]),
            action_authority_public_key_hex=str(
                value["action_authority_public_key_hex"]
            ),
            activation_height_rule=str(value["activation_height_rule"]),
            activation_instruction=str(value["activation_instruction"]),
            activation_lifecycle=str(value["activation_lifecycle"]),
            activation_minimum_delay_blocks=int(
                value["activation_minimum_delay_blocks"]
            ),
            activation_template_activate_at_height=int(
                value["activation_template_activate_at_height"]
            ),
            activation_template_norito=bytes(value["activation_template_norito"]),
            activation_template_proposed_at_height=int(
                value["activation_template_proposed_at_height"]
            ),
            activation_template_sha256=str(value["activation_template_sha256"]),
            asset_definition_id=str(value["asset_definition_id"]),
            candidate_binding_sha256=str(value["candidate_binding_sha256"]),
            compiled_profile_sha256=str(value["compiled_profile_sha256"]),
            domain_id=str(value["domain_id"]),
            governance_permission=str(value["governance_permission"]),
            protocol_id=str(value["protocol_id"]),
            schema=str(value["schema"]),
            schema_version=int(value["schema_version"]),
            setup_authority_account_id=str(value["setup_authority_account_id"]),
            setup_authority_public_key_hex=str(value["setup_authority_public_key_hex"]),
            setup_identity_binding_sha256=str(value["setup_identity_binding_sha256"]),
        )


@dataclass(frozen=True)
class VeRangePublicAdmissionArtifactsV1:
    action_authority_account_id: str
    action_authority_public_key_hex: str
    compiled_profile_norito: bytes
    compiled_profile_sha256: str
    engine_id: str
    engine_manifest_digest_hex: str
    max_aggregation_count: int
    parameter_digest_hex: str
    parameter_id_hex: str
    policy_id_hex: str
    proof_system_id: str
    protocol_id: str
    schema: str
    schema_version: int
    setup_requirements: VeRangeQualificationSetupRequirementsV1
    setup_requirements_sha256: str
    statement_schema_digest_hex: str
    verifier_digest_hex: str

    @classmethod
    def from_ipc(
        cls, value: Mapping[str, object]
    ) -> "VeRangePublicAdmissionArtifactsV1":
        setup = value["setup_requirements"]
        if not isinstance(setup, Mapping):
            _fail("validated IPC omitted typed VeRange setup requirements")
        return cls(
            action_authority_account_id=str(value["action_authority_account_id"]),
            action_authority_public_key_hex=str(
                value["action_authority_public_key_hex"]
            ),
            compiled_profile_norito=bytes(value["compiled_profile_norito"]),
            compiled_profile_sha256=str(value["compiled_profile_sha256"]),
            engine_id=str(value["engine_id"]),
            engine_manifest_digest_hex=str(value["engine_manifest_digest_hex"]),
            max_aggregation_count=int(value["max_aggregation_count"]),
            parameter_digest_hex=str(value["parameter_digest_hex"]),
            parameter_id_hex=str(value["parameter_id_hex"]),
            policy_id_hex=str(value["policy_id_hex"]),
            proof_system_id=str(value["proof_system_id"]),
            protocol_id=str(value["protocol_id"]),
            schema=str(value["schema"]),
            schema_version=int(value["schema_version"]),
            setup_requirements=VeRangeQualificationSetupRequirementsV1.from_ipc(setup),
            setup_requirements_sha256=str(value["setup_requirements_sha256"]),
            statement_schema_digest_hex=str(value["statement_schema_digest_hex"]),
            verifier_digest_hex=str(value["verifier_digest_hex"]),
        )


@dataclass(frozen=True)
class VeRangeQualificationPeerPlanV1:
    number: int
    label: str
    slug: str
    direct_torii_root: str
    torii_port: int
    config_sha256: str
    config_identity: tuple[int, ...]
    workdir_identity: tuple[int, ...]
    storage_identity: tuple[int, ...]


@dataclass(frozen=True)
class VeRangeQualificationSupervisorPlanV1:
    peer_label: str
    binary_path: str
    child_argv: tuple[str, ...]
    child_argv_sha256: str
    pid_file: str
    restart_generation: str
    storage: str
    supervisor_path: str
    terminal_file: str
    workdir: str


@dataclass(frozen=True)
class VeRangeQualificationCasePlanV1:
    action_authority_account_id: str
    action_authority_public_key_hex: str
    candidate_binding_sha256: str
    cargo_lock_sha256: str
    dpn_validator_release_commit: str
    genesis_expected_hash: str
    genesis_public_key: str
    irohad_sha256: str
    peers: tuple[VeRangeQualificationPeerPlanV1, ...]
    plan_binding_sha256: str
    reset_manifest_sha256: str
    schema: str
    schema_version: int
    setup_authority_account_id: str
    setup_authority_public_key_hex: str
    setup_requirements_sha256: str
    signed_genesis_sha256: str
    source_commit: str
    supervisor_sha256: str
    supervisors: tuple[VeRangeQualificationSupervisorPlanV1, ...]
    unsigned_genesis_sha256: str
    workspace_source_manifest_sha256: str

    def transcript_fields(self) -> dict[str, object]:
        """Return a bounded public event body; this does not execute the plan."""

        return {
            "action_authority_account_id": self.action_authority_account_id,
            "action_authority_public_key_hex": self.action_authority_public_key_hex,
            "candidate_binding_sha256": self.candidate_binding_sha256,
            "cargo_lock_sha256": self.cargo_lock_sha256,
            "config_sha256": [peer.config_sha256 for peer in self.peers],
            "dpn_validator_release_commit": self.dpn_validator_release_commit,
            "genesis_expected_hash": self.genesis_expected_hash,
            "genesis_public_key": self.genesis_public_key,
            "irohad_sha256": self.irohad_sha256,
            "peer_roots": [peer.direct_torii_root for peer in self.peers],
            "plan_binding_sha256": self.plan_binding_sha256,
            "reset_manifest_sha256": self.reset_manifest_sha256,
            "setup_authority_account_id": self.setup_authority_account_id,
            "setup_authority_public_key_hex": self.setup_authority_public_key_hex,
            "setup_requirements_sha256": self.setup_requirements_sha256,
            "signed_genesis_sha256": self.signed_genesis_sha256,
            "source_commit": self.source_commit,
            "supervisor_argv_sha256": [
                supervisor.child_argv_sha256 for supervisor in self.supervisors
            ],
            "supervisor_sha256": self.supervisor_sha256,
            "unsigned_genesis_sha256": self.unsigned_genesis_sha256,
            "workspace_source_manifest_sha256": (self.workspace_source_manifest_sha256),
        }


def _argv_option(argv: tuple[str, ...], flag: str) -> str:
    indices = [index for index, value in enumerate(argv) if value == flag]
    if len(indices) != 1 or indices[0] + 1 >= len(argv):
        _fail(f"supervisor argv must carry exactly one {flag}")
    return argv[indices[0] + 1]


def _supervisor_plan(
    row: object,
    peer: deploy.PeerPlan,
    *,
    irohad_sha256: str,
    restart_generation: str,
) -> VeRangeQualificationSupervisorPlanV1:
    observed_peer = getattr(row, "peer", None)
    if (
        observed_peer is None
        or getattr(observed_peer, "number", None) != peer.number
        or getattr(observed_peer, "label", None) != peer.label
        or getattr(observed_peer, "slug", None) != peer.slug
        or getattr(observed_peer, "config_sha256", None) != peer.config_sha256
    ):
        _fail("supervisor identity differs from its authenticated reset peer")
    argv_value = getattr(row, "child_argv", None)
    if (
        not isinstance(argv_value, tuple)
        or not 1 <= len(argv_value) <= MAX_ARGV_ITEMS
        or any(
            not isinstance(item, str)
            or not item
            or not item.isascii()
            or len(item.encode("ascii")) > MAX_ARG_BYTES
            for item in argv_value
        )
    ):
        _fail("supervisor child argv is not one bounded immutable tuple")
    argv = tuple(argv_value)
    pid_file = _absolute_path(getattr(row, "pid_file", None), "supervisor PID file")
    terminal_file = _absolute_path(
        getattr(row, "terminal_file", None), "supervisor terminal file"
    )
    workdir = _absolute_path(getattr(row, "workdir", None), "supervisor workdir")
    storage = _absolute_path(getattr(row, "storage", None), "supervisor storage")
    if (
        _argv_option(argv, "--binary-sha256") != irohad_sha256
        or _argv_option(argv, "--config-sha256") != peer.config_sha256
        or _argv_option(argv, "--config") != str(peer.config)
        or _argv_option(argv, "--workdir") != workdir
        or _argv_option(argv, "--storage-dir") != storage
        or _argv_option(argv, "--pid-file") != pid_file
        or _argv_option(argv, "--terminal-unhealthy-file") != terminal_file
        or _argv_option(argv, "--restart-generation") != restart_generation
    ):
        _fail("supervisor argv differs from the reset plan or restart identity")
    isolated_indices = [index for index, value in enumerate(argv) if value == "-S"]
    if len(isolated_indices) != 1:
        _fail("supervisor argv must carry one isolated-Python source identity")
    isolated_index = isolated_indices[0]
    if isolated_index + 1 >= len(argv):
        _fail("supervisor argv truncated its source identity")
    binary_path = _absolute_path(_argv_option(argv, "--binary"), "supervisor binary")
    supervisor_path = _absolute_path(argv[isolated_index + 1], "supervisor source")
    argv_sha256 = hashlib.sha256(_canonical(list(argv))).hexdigest()
    return VeRangeQualificationSupervisorPlanV1(
        peer_label=peer.label,
        binary_path=binary_path,
        child_argv=argv,
        child_argv_sha256=argv_sha256,
        pid_file=pid_file,
        restart_generation=restart_generation,
        storage=storage,
        supervisor_path=supervisor_path,
        terminal_file=terminal_file,
        workdir=workdir,
    )


def _peer_body(peer: VeRangeQualificationPeerPlanV1) -> dict[str, object]:
    return {
        "config_identity": list(peer.config_identity),
        "config_sha256": peer.config_sha256,
        "direct_torii_root": peer.direct_torii_root,
        "label": peer.label,
        "number": peer.number,
        "slug": peer.slug,
        "storage_identity": list(peer.storage_identity),
        "torii_port": peer.torii_port,
        "workdir_identity": list(peer.workdir_identity),
    }


def _supervisor_body(
    supervisor: VeRangeQualificationSupervisorPlanV1,
) -> dict[str, object]:
    return {
        "binary_path": supervisor.binary_path,
        "child_argv": list(supervisor.child_argv),
        "child_argv_sha256": supervisor.child_argv_sha256,
        "peer_label": supervisor.peer_label,
        "pid_file": supervisor.pid_file,
        "restart_generation": supervisor.restart_generation,
        "storage": supervisor.storage,
        "supervisor_path": supervisor.supervisor_path,
        "terminal_file": supervisor.terminal_file,
        "workdir": supervisor.workdir,
    }


def build_verange_qualification_case_plan_v1(
    *,
    bundle: deploy.BundlePlan,
    candidate_binding_sha256: str,
    cargo_lock_sha256: str,
    workspace_source_manifest_sha256: str,
    public_artifacts: VeRangePublicAdmissionArtifactsV1,
    supervisors: Sequence[object],
    supervisor_sha256: str,
    restart_generation: str,
) -> VeRangeQualificationCasePlanV1:
    """Build a public immutable plan; absence from genesis remains fatal."""

    candidate = _sha256(candidate_binding_sha256, "candidate binding")
    cargo_lock = _sha256(cargo_lock_sha256, "Cargo.lock digest")
    workspace_source = _sha256(
        workspace_source_manifest_sha256, "workspace source manifest digest"
    )
    supervisor_digest = _sha256(supervisor_sha256, "supervisor digest")
    restart = _sha256(restart_generation, "restart generation")
    manifest = bundle.manifest
    if not isinstance(manifest, dict):
        _fail("authenticated reset plan omitted its manifest")
    source_commit = _commit(manifest.get("source_commit"), "reset source commit")
    dpn_commit = _commit(
        manifest.get("dpn_validator_release_commit"), "reset DPN release commit"
    )
    irohad_sha256 = _sha256(manifest.get("irohad_sha256"), "reset irohad digest")
    signed_genesis_sha256 = _sha256(
        manifest.get("signed_genesis_sha256"), "signed genesis digest"
    )
    unsigned_genesis_sha256 = _sha256(
        manifest.get("unsigned_genesis_sha256"), "unsigned genesis digest"
    )
    genesis_expected_hash = _sha256(
        manifest.get("genesis_expected_hash"), "expected genesis hash"
    )
    genesis_public_key = _text(manifest.get("genesis_public_key"), "genesis public key")
    if (
        manifest.get("candidate_binding_sha256") != candidate
        or manifest.get("cargo_lock_sha256") != cargo_lock
        or manifest.get("workspace_source_manifest_sha256") != workspace_source
    ):
        _fail("reset manifest is not bound to the exact candidate source closure")

    setup = public_artifacts.setup_requirements
    setup_requirements_sha256 = _sha256(
        public_artifacts.setup_requirements_sha256, "setup requirements digest"
    )
    if (
        setup.candidate_binding_sha256 != candidate
        or setup.action_authority_account_id
        != public_artifacts.action_authority_account_id
        or setup.action_authority_public_key_hex
        != public_artifacts.action_authority_public_key_hex
        or setup.compiled_profile_sha256 != public_artifacts.compiled_profile_sha256
    ):
        _fail("driver public setup requirements are internally substituted")
    expected_genesis_plan = {
        "candidate_binding_sha256": candidate,
        "schema": GENESIS_PLAN_SCHEMA,
        "schema_version": GENESIS_PLAN_SCHEMA_VERSION,
        "setup_authority_account_id": setup.setup_authority_account_id,
        "setup_authority_public_key_hex": setup.setup_authority_public_key_hex,
        "setup_requirements_sha256": setup_requirements_sha256,
    }
    if manifest.get(GENESIS_PLAN_MANIFEST_FIELD) != expected_genesis_plan:
        _fail("reset genesis plan does not admit the exact driver setup identity")

    if len(bundle.peers) != PEER_COUNT or len(supervisors) != PEER_COUNT:
        _fail("VeRange qualification plan requires exactly four validators")
    config_manifest = manifest.get("configs")
    if not isinstance(config_manifest, dict):
        _fail("reset manifest omitted exact validator config identities")
    peers: list[VeRangeQualificationPeerPlanV1] = []
    for expected_number, peer in enumerate(bundle.peers, start=1):
        if (
            peer.number != expected_number
            or not 1 <= peer.torii_port <= 65_535
            or config_manifest.get(peer.slug) != peer.config_sha256
        ):
            _fail("reset peer ordering, port, or config identity is substituted")
        peers.append(
            VeRangeQualificationPeerPlanV1(
                number=peer.number,
                label=_text(peer.label, "peer label"),
                slug=_text(peer.slug, "peer slug"),
                direct_torii_root=f"http://127.0.0.1:{peer.torii_port}",
                torii_port=peer.torii_port,
                config_sha256=_sha256(peer.config_sha256, "peer config digest"),
                config_identity=tuple(peer.config_identity),
                workdir_identity=tuple(peer.workdir_identity),
                storage_identity=tuple(peer.storage_identity),
            )
        )
    if (
        len({peer.label for peer in peers}) != PEER_COUNT
        or len({peer.direct_torii_root for peer in peers}) != PEER_COUNT
    ):
        _fail("reset plan has duplicate peer labels or direct roots")

    supervisor_plans = tuple(
        _supervisor_plan(
            row,
            peer,
            irohad_sha256=irohad_sha256,
            restart_generation=restart,
        )
        for row, peer in zip(supervisors, bundle.peers)
    )
    if (
        len({row.binary_path for row in supervisor_plans}) != 1
        or len({row.supervisor_path for row in supervisor_plans}) != 1
        or len({row.child_argv_sha256 for row in supervisor_plans}) != PEER_COUNT
    ):
        _fail("four supervisors do not have one binary/source and unique peer argv")

    body: dict[str, object] = {
        "action_authority_account_id": public_artifacts.action_authority_account_id,
        "action_authority_public_key_hex": public_artifacts.action_authority_public_key_hex,
        "candidate_binding_sha256": candidate,
        "cargo_lock_sha256": cargo_lock,
        "dpn_validator_release_commit": dpn_commit,
        "genesis_expected_hash": genesis_expected_hash,
        "genesis_public_key": genesis_public_key,
        "irohad_sha256": irohad_sha256,
        "peers": [_peer_body(peer) for peer in peers],
        "reset_manifest_sha256": _sha256(
            bundle.manifest_sha256, "reset manifest digest"
        ),
        "schema": PLAN_SCHEMA,
        "schema_version": PLAN_SCHEMA_VERSION,
        "setup_authority_account_id": setup.setup_authority_account_id,
        "setup_authority_public_key_hex": setup.setup_authority_public_key_hex,
        "setup_requirements_sha256": setup_requirements_sha256,
        "signed_genesis_sha256": signed_genesis_sha256,
        "source_commit": source_commit,
        "supervisor_sha256": supervisor_digest,
        "supervisors": [_supervisor_body(row) for row in supervisor_plans],
        "unsigned_genesis_sha256": unsigned_genesis_sha256,
        "workspace_source_manifest_sha256": workspace_source,
    }
    plan_binding_sha256 = hashlib.sha256(_canonical(body)).hexdigest()
    return VeRangeQualificationCasePlanV1(
        action_authority_account_id=public_artifacts.action_authority_account_id,
        action_authority_public_key_hex=public_artifacts.action_authority_public_key_hex,
        candidate_binding_sha256=candidate,
        cargo_lock_sha256=cargo_lock,
        dpn_validator_release_commit=dpn_commit,
        genesis_expected_hash=genesis_expected_hash,
        genesis_public_key=genesis_public_key,
        irohad_sha256=irohad_sha256,
        peers=tuple(peers),
        plan_binding_sha256=plan_binding_sha256,
        reset_manifest_sha256=str(body["reset_manifest_sha256"]),
        schema=PLAN_SCHEMA,
        schema_version=PLAN_SCHEMA_VERSION,
        setup_authority_account_id=setup.setup_authority_account_id,
        setup_authority_public_key_hex=setup.setup_authority_public_key_hex,
        setup_requirements_sha256=setup_requirements_sha256,
        signed_genesis_sha256=signed_genesis_sha256,
        source_commit=source_commit,
        supervisor_sha256=supervisor_digest,
        supervisors=supervisor_plans,
        unsigned_genesis_sha256=unsigned_genesis_sha256,
        workspace_source_manifest_sha256=workspace_source,
    )
