#!/usr/bin/env python3
"""Validate the canonical four-wave Taira Exact12 rollout contract.

This helper is deliberately verification-only.  It does not contact Taira,
submit governance transactions, restart validators, sign evidence, authorize
publication, or turn an untrusted record into release authority.  A sealed
deployment controller must create the observation record from direct peer
queries and lifecycle operations, and admission must authenticate that record
before this structural validator is used.

The checked-in plan fixes the protocol waves, notice/observation intervals,
resource ceilings, halt/rollback policy, and the mandatory least-privilege
post-cutover write/privacy canary.  ``--skip-write-canary`` has no qualifying
representation in the result schema.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

try:
    from . import taira_authority_client
except ImportError:
    import taira_authority_client


PLAN_SCHEMA = "iroha.taira.privacy_rollout_plan"
RESULT_SCHEMA = "iroha.taira.privacy_rollout_observation"
SCHEMA_VERSION = 1
CANDIDATE_ID_DOMAIN = b"iroha.taira.privacy_rollout_candidate.v1\0"
ROLLOUT_ID_DOMAIN = b"iroha.taira.privacy_rollout_observation.v1\0"
MAX_PLAN_BYTES = 256 * 1024
MAX_RESULT_BYTES = 8 * 1024 * 1024
MAX_AUTHORITY_SIDECAR_BYTES = 8 * 1024 * 1024
AUTHORITY_ENVELOPE_SUFFIX = ".rollout-authority-envelope-v1.json"
DURABLE_RECEIPT_SUFFIX = ".rollout-authority-receipt-v1.json"
FROZEN_CARGO_LOCK_SHA256 = (
    "cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79"
)
EXACT12_MATRIX_SHA256 = (
    "7336d0221fddc51486ee53d4203f5a92d560d0ec9104a49de25896a8b10673d0"
)
CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")

ENDPOINTS = (
    "public-torii",
    "taira-validator-1",
    "taira-validator-2",
    "taira-validator-3",
    "taira-validator-4",
)
VALIDATORS = ENDPOINTS[1:]
PROTOCOLS = (
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
)
WAVES = (
    (
        "zk-ace-pq-authorization-v0",
        "anonymous-pgc-k-out-of-n-v1",
        "verange-transparent-range-v1",
        "iroha-bootle-lantern-anoncred-v1",
    ),
    (
        "orchard-halo2-actions-v1",
        "monero-fcmp-plus-plus-v1",
        "iroha-ivm-private-note-stark-v1",
        "pq-masp-stark-v0",
    ),
    (
        "vega-existing-credential-zk-v0",
        "iroha-jindo-polynomial-commitment-v0",
    ),
    (
        "iroha-zk-x509-stark-p256-v0",
        "iroha-zk-ams-v1",
    ),
)
JINDO_PROTOCOL = "iroha-jindo-polynomial-commitment-v0"
JINDO_ASSURANCE = "available-experimental"
JINDO_MISSING_EVIDENCE = (
    "MissingDistributionWideKnowledgeSoundnessEvidence",
)
RETIRED_PROTOCOLS = (
    "zkat-policy-private-auth-v1",
    "zk-ams-recursive-admission-v0",
    "silent-threshold-anoncred-v0",
    "zk-x509-onchain-identity-v0",
    "jindo-lattice-pcs-zk-v0",
    "sis-hints-anoncred-pq-v0",
    "sis-with-hints",
    "penumbra-masp-v1",
    "miden-stark-note-v1",
    "aztec-private-rollup-v1",
)

NOTICE_INTERVAL_BLOCKS = 300
OBSERVATION_INTERVAL_BLOCKS = 300
RESOURCE_CEILINGS = {
    "max_address_space_bytes": 1024 * 1024 * 1024 * 1024,
    "max_elapsed_millis": 60 * 60 * 1000,
    "max_peak_rss_bytes": 64 * 1024 * 1024 * 1024,
    "max_transport_bytes": 128 * 1024 * 1024,
    "max_work_units": 100_000_000_000,
    "zk_ams_evaluated_key_bytes": 48_452_611_616,
}
HALT_CONDITIONS = (
    "state-or-hash-divergence",
    "retained-protocol-unavailable",
    "malformed-or-replayed-input-accepted",
    "native-resource-ceiling-exceeded",
    "restart-convergence-failed",
    "receipt-or-candidate-binding-mismatch",
)

AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA = (
    "iroha.taira.authenticated-rollout-observation-authority.v1"
)
AUTHENTICATED_ROLLOUT_OBSERVATION_REPLAY_NAMESPACE = (
    "iroha.taira.authenticated-rollout-observation-replay.v1"
)
AUTHENTICATED_ROLLOUT_OBSERVATION_PROVISIONING_ERROR = (
    f"{AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA} is not provisioned: "
    "publication requires a canonical authority-origin envelope signed under a "
    "separately pinned trust root inaccessible to the rollout runtime, deploy "
    "controller, candidate signer, release signer, and publication signer; the "
    "envelope must bind the exact rollout-plan and observation bytes and digests, "
    "the admitted candidate, source and DPN commits, Cargo.lock and workspace-source "
    "digests, candidate OCI digest, qualification receipt, deploy receipt and "
    "deployed binary/config/capability identities, all four peer and public-Torii "
    "identities, the supervisor, authority host and installation identities, the "
    "installed controller digest, and a fresh authenticated run nonce, issued time, "
    "expiry, and replay identity in "
    f"{AUTHENTICATED_ROLLOUT_OBSERVATION_REPLAY_NAMESPACE}; the authority must "
    "independently verify governance, canary, resource, restart, convergence, and "
    "post-cutover semantics and reject self-hashes, owner/path-only trust, workflow "
    "IDs, caller markers or environment switches, reused signing keys, stale or "
    "replayed runs, spliced source/controller/host/table/result fields, and legacy "
    "unsigned observations"
)


class RolloutContractError(RuntimeError):
    """The plan or observation record violates the closed rollout contract."""


@dataclass(frozen=True)
class AuthenticatedRolloutObservation:
    """Validated identifiers and the two explicit authenticated sidecars."""

    plan_sha256: str
    rollout_id: str
    operation_id: str
    authority_envelope: bytes
    durable_receipt: bytes

    def __iter__(self):
        """Preserve the historical two-value unpacking interface."""

        yield self.plan_sha256
        yield self.rollout_id


def _fail(message: str) -> NoReturn:
    raise RolloutContractError(message)


def _require_authenticated_rollout_observation_authority(
    *, require_signing: bool
) -> None:
    """Authenticate the fixed rollout-observation binding and live service."""

    try:
        if require_signing:
            taira_authority_client.preflight("rollout-observation")
        else:
            taira_authority_client.preflight(
                "rollout-observation", require_signing=False
            )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise RolloutContractError(
            f"{AUTHENTICATED_ROLLOUT_OBSERVATION_PROVISIONING_ERROR}: {error}"
        ) from error


def require_authenticated_rollout_observation_authority_provisioned() -> None:
    """Authenticate the fixed rollout-observation binding and live service."""

    _require_authenticated_rollout_observation_authority(require_signing=True)


def canonical_json_bytes(value: object) -> bytes:
    """Return the sole accepted ASCII JSON representation."""

    try:
        return (
            json.dumps(
                value,
                ensure_ascii=True,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise RolloutContractError(
            f"rollout value is not canonically encodable: {exc}"
        ) from exc


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _read_stable(path: Path, maximum: int, label: str) -> bytes:
    try:
        before = path.lstat()
    except OSError as exc:
        raise RolloutContractError(f"cannot inspect {label}: {path}") from exc
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum
    ):
        _fail(f"{label} is not one bounded single-link regular file")
    flags = os.O_RDONLY | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            _fail(f"{label} changed while opening")
        payload = bytearray()
        while len(payload) < before.st_size:
            chunk = os.read(descriptor, min(64 * 1024, before.st_size - len(payload)))
            if not chunk:
                _fail(f"{label} was truncated while reading")
            payload.extend(chunk)
        if os.read(descriptor, 1) or _identity(os.fstat(descriptor)) != _identity(before):
            _fail(f"{label} changed while reading")
        return bytes(payload)
    finally:
        os.close(descriptor)


def _reject_constant(value: str) -> NoReturn:
    _fail(f"non-finite JSON number is forbidden: {value}")


def _pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            _fail(f"duplicate JSON field is forbidden: {key}")
        value[key] = item
    return value


def _decode_canonical(payload: bytes, maximum: int, label: str) -> dict[str, object]:
    if not payload or len(payload) > maximum:
        _fail(f"{label} is empty or exceeds its {maximum}-byte bound")
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_pairs,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RolloutContractError(f"{label} is not strict JSON") from exc
    if not isinstance(value, dict) or canonical_json_bytes(value) != payload:
        _fail(f"{label} is not one canonical closed JSON object")
    return value


def _exact(value: object, fields: set[str], label: str) -> Mapping[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        _fail(f"{label} fields are not exact")
    return value


def _array(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        _fail(f"{label} must be an array")
    return value


def _integer(value: object, label: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _sha256(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
    return value


def _commit(value: object, label: str) -> str:
    if not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase 40-hex commit")
    if value == "0" * 40:
        _fail(f"{label} must be nonzero")
    return value


def _oci_digest(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.startswith("sha256:"):
        _fail(f"{label} must be one sha256 OCI digest")
    _sha256(value.removeprefix("sha256:"), label)
    return value


def _expected_protocol_rows() -> list[dict[str, object]]:
    rows = []
    for index, protocol in enumerate(PROTOCOLS):
        rows.append(
            {
                "assurance": JINDO_ASSURANCE if protocol == JINDO_PROTOCOL else "available",
                "index": index,
                "label": protocol,
                "missing_evidence": (
                    list(JINDO_MISSING_EVIDENCE) if protocol == JINDO_PROTOCOL else []
                ),
                "release_status": "retained-required",
            }
        )
    return rows


def expected_plan() -> dict[str, object]:
    """Return the immutable first-release rollout policy."""

    return {
        "activation_contract": {
            "active_lifecycle": "Active",
            "mode": "governance",
            "one_committed_transaction_per_protocol": True,
            "proposed_lifecycle": "Proposed",
        },
        "canary_contract": {
            "malformed_failure_code": "privacy-malformed-proof",
            "per_wave": ["positive", "negative", "replay"],
            "positive_query_scope": list(ENDPOINTS),
            "rejection_scope": list(ENDPOINTS),
            "replay_failure_code": "privacy-replay",
        },
        "cargo_lock_sha256": FROZEN_CARGO_LOCK_SHA256,
        "chain_id": CHAIN_ID,
        "endpoints": list(ENDPOINTS),
        "halt_conditions": list(HALT_CONDITIONS),
        "intervals": {
            "notice_blocks": NOTICE_INTERVAL_BLOCKS,
            "observation_blocks": OBSERVATION_INTERVAL_BLOCKS,
        },
        "legacy_policy": {
            "decoder_or_migration_fallback": False,
            "retired_labels": list(RETIRED_PROTOCOLS),
        },
        "post_cutover_contract": {
            "authority_scope": "dedicated-no-governance-canary",
            "bootstrap_authority_forbidden": True,
            "governance_permission_forbidden": True,
            "mode": "signed-write-and-privacy",
            "privacy_protocol": "verange-transparent-range-v1",
            "skip_write_canary_forbidden": True,
        },
        "protocol_matrix_sha256": EXACT12_MATRIX_SHA256,
        "protocols": _expected_protocol_rows(),
        "publication_contract": {
            "network_success_is_native_authority": False,
            "only_deployed_and_readmitted_oci_digest": True,
            "requires_rollout_status": "passed",
        },
        "resource_ceilings": dict(RESOURCE_CEILINGS),
        "restart_contract": {
            "exact_sentinel_height_and_hash": True,
            "one_validator_per_wave": True,
            "peer_finality_scope": list(ENDPOINTS),
            "successor_finality_required": True,
        },
        "rollback_contract": {
            "armed_before_first_wave": True,
            "legacy_fallback_forbidden": True,
            "restore_mode": "immutable-candidate-and-capability-set",
        },
        "schema": PLAN_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "waves": [
            {"index": index, "label": f"wave-{index}", "protocols": list(protocols)}
            for index, protocols in enumerate(WAVES, 1)
        ],
    }


def validate_plan(plan: Mapping[str, object]) -> str:
    """Validate and hash the exact checked-in rollout plan."""

    if dict(plan) != expected_plan():
        _fail("rollout plan differs from the immutable first-release four-wave policy")
    encoded = canonical_json_bytes(dict(plan))
    return hashlib.sha256(encoded).hexdigest()


def candidate_binding(candidate: Mapping[str, object]) -> str:
    """Derive the candidate identity after validating every immutable binding."""

    fields = _exact(
        candidate,
        {
            "archive_sha256",
            "candidate_binding_sha256",
            "candidate_oci_digest",
            "capability_schema_sha256",
            "cargo_lock_sha256",
            "dpn_validator_release_commit",
            "irohad_sha256",
            "protocol_matrix_sha256",
            "source_commit",
            "workspace_source_manifest_sha256",
        },
        "candidate binding",
    )
    _commit(fields["source_commit"], "source commit")
    _commit(fields["dpn_validator_release_commit"], "DPN validator release commit")
    if fields["cargo_lock_sha256"] != FROZEN_CARGO_LOCK_SHA256:
        _fail("candidate Cargo.lock differs from the frozen release lock")
    if fields["protocol_matrix_sha256"] != EXACT12_MATRIX_SHA256:
        _fail("candidate protocol matrix differs from Exact12 v1")
    for name in (
        "archive_sha256",
        "capability_schema_sha256",
        "irohad_sha256",
        "workspace_source_manifest_sha256",
    ):
        _sha256(fields[name], name.replace("_", " "))
    _oci_digest(fields["candidate_oci_digest"], "candidate OCI digest")
    body = {key: value for key, value in fields.items() if key != "candidate_binding_sha256"}
    derived = hashlib.sha256(CANDIDATE_ID_DOMAIN + canonical_json_bytes(body)[:-1]).hexdigest()
    if _sha256(fields["candidate_binding_sha256"], "candidate binding") != derived:
        _fail("candidate binding is not derived from the immutable candidate tuple")
    return derived


def _expected_active(wave_count: int) -> list[str]:
    selected = {protocol for wave in WAVES[:wave_count] for protocol in wave}
    return [protocol for protocol in PROTOCOLS if protocol in selected]


def _validate_snapshots(
    value: object,
    *,
    candidate_sha256: str,
    active_protocols: Sequence[str],
    expected_height: int | None,
    label: str,
) -> tuple[int, str, str]:
    rows = _array(value, label)
    if len(rows) != len(ENDPOINTS):
        _fail(f"{label} must contain public Torii plus all four validators")
    common: tuple[int, str, str] | None = None
    for index, (row_value, endpoint) in enumerate(zip(rows, ENDPOINTS)):
        row = _exact(
            row_value,
            {
                "active_protocols",
                "available_protocols",
                "block_hash",
                "candidate_binding_sha256",
                "capability_manifest_sha256",
                "endpoint",
                "height",
                "jindo_assurance",
                "jindo_missing_evidence",
                "unavailable_protocols",
            },
            f"{label} row {index}",
        )
        if row["endpoint"] != endpoint:
            _fail(f"{label} endpoint order differs from the exact five-root order")
        if row["candidate_binding_sha256"] != candidate_sha256:
            _fail(f"{label} does not bind the exact candidate")
        height = _integer(row["height"], f"{label} height", minimum=1)
        if expected_height is not None and height != expected_height:
            _fail(f"{label} height {height} differs from expected {expected_height}")
        block_hash = _sha256(row["block_hash"], f"{label} block hash")
        manifest = _sha256(
            row["capability_manifest_sha256"], f"{label} capability manifest"
        )
        if row["available_protocols"] != list(PROTOCOLS) or row["unavailable_protocols"] != []:
            _fail(f"{label} contains an unavailable, missing, reordered, or extra protocol")
        if row["active_protocols"] != list(active_protocols):
            _fail(f"{label} active protocol set differs from the completed wave prefix")
        if (
            row["jindo_assurance"] != JINDO_ASSURANCE
            or row["jindo_missing_evidence"] != list(JINDO_MISSING_EVIDENCE)
        ):
            _fail(f"{label} falsely certifies or hides revised Jindo's missing evidence")
        observed = (height, block_hash, manifest)
        if common is None:
            common = observed
        elif common != observed:
            _fail(f"{label} has a height, hash, or capability-manifest divergence")
    assert common is not None
    return common


def _validate_queries(value: object, label: str) -> tuple[int, str]:
    rows = _array(value, label)
    if len(rows) != len(ENDPOINTS):
        _fail(f"{label} must contain all five direct query outcomes")
    common: tuple[int, str] | None = None
    for index, (row_value, endpoint) in enumerate(zip(rows, ENDPOINTS)):
        row = _exact(
            row_value,
            {"endpoint", "height", "state_sha256"},
            f"{label} row {index}",
        )
        if row["endpoint"] != endpoint:
            _fail(f"{label} endpoint order differs")
        observed = (
            _integer(row["height"], f"{label} height", minimum=1),
            _sha256(row["state_sha256"], f"{label} state digest"),
        )
        if common is None:
            common = observed
        elif common != observed:
            _fail(f"{label} direct peers disagree on queried privacy state")
    assert common is not None
    return common


def _validate_rejections(
    value: object,
    *,
    failure_code: str,
    minimum_height: int,
    maximum_height: int | None,
    label: str,
) -> int:
    rows = _array(value, label)
    if len(rows) != len(ENDPOINTS):
        _fail(f"{label} must preserve direct rejection from every endpoint")
    maximum_observed = minimum_height
    for index, (row_value, endpoint) in enumerate(zip(rows, ENDPOINTS)):
        row = _exact(
            row_value,
            {"endpoint", "failure_code", "observed_height"},
            f"{label} row {index}",
        )
        if row["endpoint"] != endpoint or row["failure_code"] != failure_code:
            _fail(f"{label} has a missing, reordered, or wrongly classified rejection")
        observed_height = _integer(
            row["observed_height"], f"{label} observed height", minimum=1
        )
        if observed_height < minimum_height:
            _fail(f"{label} predates activation")
        if maximum_height is not None and observed_height > maximum_height:
            _fail(f"{label} falls outside the governed observation window")
        maximum_observed = max(maximum_observed, observed_height)
    return maximum_observed


def _validate_canaries(
    value: object,
    *,
    protocols: Sequence[str],
    candidate_sha256: str,
    activation_height: int,
    observation_height: int,
    used_transactions: set[str],
    label: str,
) -> int:
    rows = _array(value, label)
    if len(rows) != len(protocols):
        _fail(f"{label} must contain one canary for every protocol in the wave")
    maximum_height = activation_height
    seen_statements: set[str] = set()
    for index, (row_value, protocol) in enumerate(zip(rows, protocols)):
        row = _exact(
            row_value,
            {
                "candidate_binding_sha256",
                "negative",
                "positive",
                "protocol",
                "replay",
            },
            f"{label} row {index}",
        )
        if row["protocol"] != protocol or row["candidate_binding_sha256"] != candidate_sha256:
            _fail(f"{label} protocol order or candidate binding differs")
        positive = _exact(
            row["positive"],
            {
                "accepted_height",
                "peer_queries",
                "statement_sha256",
                "submitted_via",
                "transaction_sha256",
            },
            f"{label} positive {protocol}",
        )
        if positive["submitted_via"] != "public-torii":
            _fail(f"{label} positive canary did not traverse public Torii")
        accepted_height = _integer(
            positive["accepted_height"], f"{label} accepted height", minimum=1
        )
        if not activation_height <= accepted_height <= observation_height:
            _fail(f"{label} positive canary is outside the governed observation window")
        transaction = _sha256(
            positive["transaction_sha256"], f"{label} positive transaction"
        )
        statement = _sha256(
            positive["statement_sha256"], f"{label} positive statement"
        )
        if statement in seen_statements:
            _fail(f"{label} reuses a proof statement across protocols")
        seen_statements.add(statement)
        if transaction in used_transactions:
            _fail(f"{label} reuses a transaction from an earlier rollout action")
        used_transactions.add(transaction)
        query_height, _ = _validate_queries(
            positive["peer_queries"], f"{label} positive peer queries for {protocol}"
        )
        if not accepted_height <= query_height <= observation_height:
            _fail(f"{label} positive peer query is outside the observation window")
        negative = _exact(
            row["negative"],
            {"rejections", "transaction_sha256"},
            f"{label} negative {protocol}",
        )
        negative_tx = _sha256(
            negative["transaction_sha256"], f"{label} malformed transaction"
        )
        if negative_tx in used_transactions:
            _fail(f"{label} malformed canary reused a rollout transaction")
        used_transactions.add(negative_tx)
        negative_height = _validate_rejections(
            negative["rejections"],
            failure_code="privacy-malformed-proof",
            minimum_height=activation_height,
            maximum_height=observation_height,
            label=f"{label} malformed rejection for {protocol}",
        )
        replay = _exact(
            row["replay"],
            {"rejections", "transaction_sha256"},
            f"{label} replay {protocol}",
        )
        if _sha256(replay["transaction_sha256"], f"{label} replay transaction") != transaction:
            _fail(f"{label} replay did not resubmit the exact accepted transaction")
        replay_height = _validate_rejections(
            replay["rejections"],
            failure_code="privacy-replay",
            minimum_height=accepted_height,
            maximum_height=observation_height,
            label=f"{label} replay rejection for {protocol}",
        )
        maximum_height = max(
            maximum_height,
            accepted_height,
            query_height,
            negative_height,
            replay_height,
        )
    return maximum_height


def _validate_resources(value: object, protocols: Sequence[str], label: str) -> None:
    rows = _array(value, label)
    if len(rows) != len(protocols):
        _fail(f"{label} must measure every protocol in the wave")
    for index, (row_value, protocol) in enumerate(zip(rows, protocols)):
        row = _exact(
            row_value,
            {
                "address_space_bytes",
                "elapsed_millis",
                "evaluated_key_publication_bytes",
                "evaluated_key_retrieval_bytes",
                "peak_rss_bytes",
                "protocol",
                "transport_bytes",
                "work_units",
            },
            f"{label} row {index}",
        )
        if row["protocol"] != protocol:
            _fail(f"{label} protocol order differs")
        elapsed = _integer(row["elapsed_millis"], f"{label} elapsed", minimum=1)
        rss = _integer(row["peak_rss_bytes"], f"{label} RSS", minimum=1)
        address = _integer(
            row["address_space_bytes"], f"{label} address space", minimum=1
        )
        transport = _integer(
            row["transport_bytes"], f"{label} transport", minimum=1
        )
        work = _integer(row["work_units"], f"{label} work", minimum=1)
        if elapsed > RESOURCE_CEILINGS["max_elapsed_millis"]:
            _fail(f"{label} elapsed time exceeds its frozen ceiling")
        if rss > RESOURCE_CEILINGS["max_peak_rss_bytes"]:
            _fail(f"{label} RSS exceeds its frozen ceiling")
        if address < rss or address > RESOURCE_CEILINGS["max_address_space_bytes"]:
            _fail(f"{label} address-space measurement is invalid or over ceiling")
        if transport > RESOURCE_CEILINGS["max_transport_bytes"]:
            _fail(f"{label} transport exceeds its frozen ceiling")
        if work > RESOURCE_CEILINGS["max_work_units"]:
            _fail(f"{label} work exceeds the 100-billion-unit ceiling")
        publication = _integer(
            row["evaluated_key_publication_bytes"],
            f"{label} evaluated-key publication",
        )
        retrieval = _integer(
            row["evaluated_key_retrieval_bytes"],
            f"{label} evaluated-key retrieval",
        )
        expected = (
            RESOURCE_CEILINGS["zk_ams_evaluated_key_bytes"]
            if protocol == "iroha-zk-ams-v1"
            else 0
        )
        if publication != expected or retrieval != expected:
            _fail(f"{label} evaluated-key publication/retrieval accounting differs")


def _validate_restart(
    value: object,
    *,
    wave_index: int,
    minimum_stopped_height: int,
    observation_height: int,
    label: str,
) -> int:
    restart = _exact(
        value,
        {
            "peer_finality",
            "recovered_hash",
            "recovered_height",
            "sentinel_hash",
            "sentinel_height",
            "stopped_height",
            "successor_hash",
            "successor_height",
            "validator",
        },
        label,
    )
    expected_validator = VALIDATORS[wave_index - 1]
    if restart["validator"] != expected_validator:
        _fail(f"{label} must rotate through {expected_validator}")
    stopped = _integer(restart["stopped_height"], f"{label} stopped height", minimum=1)
    sentinel = _integer(restart["sentinel_height"], f"{label} sentinel height", minimum=1)
    recovered = _integer(restart["recovered_height"], f"{label} recovered height", minimum=1)
    successor = _integer(restart["successor_height"], f"{label} successor height", minimum=1)
    sentinel_hash = _sha256(restart["sentinel_hash"], f"{label} sentinel hash")
    recovered_hash = _sha256(restart["recovered_hash"], f"{label} recovered hash")
    successor_hash = _sha256(restart["successor_hash"], f"{label} successor hash")
    if (
        stopped < minimum_stopped_height
        or stopped >= sentinel
        or sentinel < minimum_stopped_height + 1
        or recovered != sentinel
        or recovered_hash != sentinel_hash
        or successor != sentinel + 1
        or successor > observation_height
    ):
        _fail(f"{label} does not prove exact sentinel catch-up followed by one successor")
    rows = _array(restart["peer_finality"], f"{label} peer finality")
    if len(rows) != len(ENDPOINTS):
        _fail(f"{label} successor finality omits an endpoint")
    for index, (row_value, endpoint) in enumerate(zip(rows, ENDPOINTS)):
        row = _exact(
            row_value,
            {"block_hash", "endpoint", "height"},
            f"{label} peer finality row {index}",
        )
        if (
            row["endpoint"] != endpoint
            or row["height"] != successor
            or row["block_hash"] != successor_hash
        ):
            _fail(f"{label} successor did not finalize identically on all five roots")
    return successor


def _validate_post_cutover(
    value: object,
    *,
    candidate: Mapping[str, object],
    candidate_sha256: str,
    minimum_height: int,
    used_transactions: set[str],
) -> tuple[int, str]:
    post = _exact(
        value,
        {
            "canary",
            "deployed_candidate_oci_digest",
            "readmitted_candidate_oci_digest",
            "snapshots",
        },
        "post-cutover record",
    )
    expected_oci = candidate["candidate_oci_digest"]
    if (
        post["deployed_candidate_oci_digest"] != expected_oci
        or post["readmitted_candidate_oci_digest"] != expected_oci
    ):
        _fail("post-cutover deploy and re-admission do not bind the same OCI digest")
    canary = _exact(
        post["canary"],
        {
            "authority_scope",
            "bootstrap_authority",
            "governance_permission_present",
            "mode",
            "privacy",
            "replay",
            "skipped",
            "write",
        },
        "post-cutover canary",
    )
    if (
        canary["mode"] != "signed-write-and-privacy"
        or canary["authority_scope"] != "dedicated-no-governance-canary"
        or canary["skipped"] is not False
        or canary["bootstrap_authority"] is not False
        or canary["governance_permission_present"] is not False
    ):
        _fail("post-cutover canary was skipped, unsigned, privileged, or not privacy-capable")
    write = _exact(
        canary["write"],
        {
            "accepted_height",
            "peer_queries",
            "submitted_via",
            "transaction_sha256",
        },
        "post-cutover write canary",
    )
    if write["submitted_via"] != "public-torii":
        _fail("post-cutover write canary did not traverse public Torii")
    write_height = _integer(
        write["accepted_height"], "post-cutover write height", minimum=minimum_height + 1
    )
    write_transaction = _sha256(
        write["transaction_sha256"], "post-cutover write transaction"
    )
    if write_transaction in used_transactions:
        _fail("post-cutover write canary reused an earlier rollout transaction")
    used_transactions.add(write_transaction)
    write_query_height, _ = _validate_queries(
        write["peer_queries"], "post-cutover write peer queries"
    )
    if write_query_height < write_height:
        _fail("post-cutover write query predates its accepted action")
    privacy = _exact(
        canary["privacy"],
        {
            "accepted_height",
            "peer_queries",
            "protocol",
            "statement_sha256",
            "submitted_via",
            "transaction_sha256",
        },
        "post-cutover privacy canary",
    )
    if (
        privacy["protocol"] != "verange-transparent-range-v1"
        or privacy["submitted_via"] != "public-torii"
    ):
        _fail("post-cutover privacy canary uses the wrong protocol or ingress")
    privacy_height = _integer(
        privacy["accepted_height"],
        "post-cutover privacy height",
        minimum=write_height + 1,
    )
    privacy_transaction = _sha256(
        privacy["transaction_sha256"], "post-cutover privacy transaction"
    )
    if privacy_transaction in used_transactions:
        _fail("post-cutover privacy canary reused a rollout transaction")
    used_transactions.add(privacy_transaction)
    _sha256(privacy["statement_sha256"], "post-cutover privacy statement")
    query_height, _ = _validate_queries(
        privacy["peer_queries"], "post-cutover privacy peer queries"
    )
    if query_height < privacy_height:
        _fail("post-cutover privacy query predates its accepted action")
    replay = _exact(
        canary["replay"],
        {"rejections", "transaction_sha256"},
        "post-cutover replay canary",
    )
    if replay["transaction_sha256"] != privacy_transaction:
        _fail("post-cutover replay did not reuse the accepted privacy transaction")
    replay_height = _validate_rejections(
        replay["rejections"],
        failure_code="privacy-replay",
        minimum_height=privacy_height,
        maximum_height=None,
        label="post-cutover replay rejection",
    )
    snapshot_height, _block_hash, manifest = _validate_snapshots(
        post["snapshots"],
        candidate_sha256=candidate_sha256,
        active_protocols=_expected_active(len(WAVES)),
        expected_height=None,
        label="post-cutover capability snapshots",
    )
    if snapshot_height < max(
        write_query_height, query_height, privacy_height, replay_height
    ):
        _fail("post-cutover capability snapshots predate a mandatory canary outcome")
    return snapshot_height, manifest


def _validate_unsigned_result_structure(
    result: Mapping[str, object], *, plan: Mapping[str, object]
) -> tuple[str, str]:
    """Validate the complete rollout-record structure without claiming origin.

    This checks structure and cross-field semantics only.  The caller remains
    responsible for authenticating the controller-origin envelope and replay.
    """

    plan_sha256 = validate_plan(plan)
    root = _exact(
        result,
        {
            "baseline",
            "candidate",
            "completed_at_unix",
            "plan_sha256",
            "post_cutover",
            "rollback",
            "rollout_id",
            "schema",
            "schema_version",
            "started_at_unix",
            "terminal",
            "waves",
        },
        "rollout observation",
    )
    if (
        root["schema"] != RESULT_SCHEMA
        or root["schema_version"] != SCHEMA_VERSION
        or root["plan_sha256"] != plan_sha256
    ):
        _fail("rollout observation schema or plan binding differs")
    started = _integer(root["started_at_unix"], "rollout start time", minimum=1)
    completed = _integer(root["completed_at_unix"], "rollout completion time", minimum=1)
    if completed < started:
        _fail("rollout completion precedes its start")
    candidate = _exact(
        root["candidate"],
        {
            "archive_sha256",
            "candidate_binding_sha256",
            "candidate_oci_digest",
            "capability_schema_sha256",
            "cargo_lock_sha256",
            "dpn_validator_release_commit",
            "irohad_sha256",
            "protocol_matrix_sha256",
            "source_commit",
            "workspace_source_manifest_sha256",
        },
        "candidate binding",
    )
    candidate_sha256 = candidate_binding(candidate)
    baseline = _exact(root["baseline"], {"snapshots"}, "rollout baseline")
    baseline_height, _baseline_hash, _baseline_manifest = _validate_snapshots(
        baseline["snapshots"],
        candidate_sha256=candidate_sha256,
        active_protocols=[],
        expected_height=None,
        label="inactive Exact12 baseline snapshots",
    )

    waves = _array(root["waves"], "rollout waves")
    if len(waves) != len(WAVES):
        _fail("rollout must contain exactly four governance waves")
    prior_observation_height = baseline_height
    final_manifest = ""
    used_transactions: set[str] = set()
    for offset, (wave_value, protocols) in enumerate(zip(waves, WAVES), 1):
        label = f"wave {offset}"
        wave = _exact(
            wave_value,
            {
                "activate_at_height",
                "activation_transactions",
                "canaries",
                "candidate_binding_sha256",
                "index",
                "label",
                "observation_completed_at_height",
                "observation_snapshots",
                "post_activation_snapshots",
                "post_restart_snapshots",
                "pre_activation_snapshots",
                "proposed_at_height",
                "protocols",
                "resources",
                "restart",
            },
            label,
        )
        if (
            wave["index"] != offset
            or wave["label"] != f"wave-{offset}"
            or wave["protocols"] != list(protocols)
            or wave["candidate_binding_sha256"] != candidate_sha256
        ):
            _fail(f"{label} identity, order, or candidate binding differs")
        proposed = _integer(wave["proposed_at_height"], f"{label} proposal height", minimum=2)
        activation = _integer(
            wave["activate_at_height"], f"{label} activation height", minimum=2
        )
        observation = _integer(
            wave["observation_completed_at_height"],
            f"{label} observation height",
            minimum=2,
        )
        if (
            proposed <= prior_observation_height
            or activation - proposed < NOTICE_INTERVAL_BLOCKS
            or observation - activation < OBSERVATION_INTERVAL_BLOCKS
        ):
            _fail(f"{label} violates the notice or observation interval")
        transactions = _array(
            wave["activation_transactions"], f"{label} activation transactions"
        )
        if len(transactions) != len(protocols):
            _fail(f"{label} lacks one governed activation transaction per protocol")
        for index, (row_value, protocol) in enumerate(
            zip(transactions, protocols)
        ):
            row = _exact(
                row_value,
                {"governance_outcome", "protocol", "transaction_sha256"},
                f"{label} activation transaction {index}",
            )
            if row["protocol"] != protocol or row["governance_outcome"] != "committed":
                _fail(f"{label} activation transaction order or governance outcome differs")
            transaction_sha256 = _sha256(
                row["transaction_sha256"],
                f"{label} activation transaction digest",
            )
            if transaction_sha256 in used_transactions:
                _fail(f"{label} reuses a transaction from an earlier rollout action")
            used_transactions.add(transaction_sha256)
        prior_active = _expected_active(offset - 1)
        active = _expected_active(offset)
        _validate_snapshots(
            wave["pre_activation_snapshots"],
            candidate_sha256=candidate_sha256,
            active_protocols=prior_active,
            expected_height=proposed - 1,
            label=f"{label} pre-activation snapshots",
        )
        _validate_snapshots(
            wave["post_activation_snapshots"],
            candidate_sha256=candidate_sha256,
            active_protocols=active,
            expected_height=activation,
            label=f"{label} post-activation snapshots",
        )
        maximum_canary_height = _validate_canaries(
            wave["canaries"],
            protocols=protocols,
            candidate_sha256=candidate_sha256,
            activation_height=activation,
            observation_height=observation,
            used_transactions=used_transactions,
            label=f"{label} canaries",
        )
        _validate_resources(wave["resources"], protocols, f"{label} resources")
        successor = _validate_restart(
            wave["restart"],
            wave_index=offset,
            minimum_stopped_height=maximum_canary_height,
            observation_height=observation,
            label=f"{label} restart",
        )
        _validate_snapshots(
            wave["post_restart_snapshots"],
            candidate_sha256=candidate_sha256,
            active_protocols=active,
            expected_height=successor,
            label=f"{label} post-restart snapshots",
        )
        _, _, final_manifest = _validate_snapshots(
            wave["observation_snapshots"],
            candidate_sha256=candidate_sha256,
            active_protocols=active,
            expected_height=observation,
            label=f"{label} observation snapshots",
        )
        prior_observation_height = observation

    post_height, post_manifest = _validate_post_cutover(
        root["post_cutover"],
        candidate=candidate,
        candidate_sha256=candidate_sha256,
        minimum_height=prior_observation_height,
        used_transactions=used_transactions,
    )
    rollback = _exact(
        root["rollback"],
        {
            "armed",
            "invoked",
            "legacy_fallback_used",
            "previous_candidate_oci_digest",
            "previous_capability_manifest_sha256",
            "restore_mode",
        },
        "rollback record",
    )
    if (
        rollback["armed"] is not True
        or rollback["invoked"] is not False
        or rollback["legacy_fallback_used"] is not False
        or rollback["restore_mode"] != "immutable-candidate-and-capability-set"
    ):
        _fail("passing rollout did not preserve the armed immutable rollback contract")
    previous_oci = _oci_digest(
        rollback["previous_candidate_oci_digest"], "previous candidate OCI digest"
    )
    previous_manifest = _sha256(
        rollback["previous_capability_manifest_sha256"],
        "previous capability manifest",
    )
    if previous_oci == candidate["candidate_oci_digest"] or previous_manifest == post_manifest:
        _fail("rollback target must identify the distinct previous immutable release")
    terminal = _exact(
        root["terminal"],
        {
            "final_candidate_oci_digest",
            "final_capability_manifest_sha256",
            "halt_reason",
            "halted",
            "publication_authorized",
            "status",
        },
        "terminal rollout state",
    )
    if (
        terminal["status"] != "passed"
        or terminal["halted"] is not False
        or terminal["halt_reason"] is not None
        or terminal["publication_authorized"] is not True
        or terminal["final_candidate_oci_digest"] != candidate["candidate_oci_digest"]
        or terminal["final_capability_manifest_sha256"] != post_manifest
        or post_height <= prior_observation_height
        or not final_manifest
    ):
        _fail("terminal rollout state cannot authorize publication")
    body = {key: value for key, value in root.items() if key != "rollout_id"}
    derived_rollout_id = hashlib.sha256(
        ROLLOUT_ID_DOMAIN + canonical_json_bytes(body)[:-1]
    ).hexdigest()
    if _sha256(root["rollout_id"], "rollout ID") != derived_rollout_id:
        _fail("rollout ID is not derived from the complete canonical observation")
    return plan_sha256, derived_rollout_id


def validate_result(
    result: Mapping[str, object], *, plan: Mapping[str, object]
) -> AuthenticatedRolloutObservation:
    """Validate and authorize one exact rollout observation."""

    require_authenticated_rollout_observation_authority_provisioned()
    identifiers = _validate_unsigned_result_structure(result, plan=plan)
    subject = _rollout_authority_subject(result, plan)
    try:
        authority = taira_authority_client.authorize(
            "rollout-observation", subject
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise RolloutContractError(
            f"rollout-observation authority refused the exact result: {error}"
        ) from error
    return AuthenticatedRolloutObservation(
        plan_sha256=identifiers[0],
        rollout_id=identifiers[1],
        operation_id=authority.operation_id,
        authority_envelope=authority.authority_envelope_bytes,
        durable_receipt=authority.durable_receipt_bytes,
    )


def _rollout_authority_subject(
    result: Mapping[str, object], plan: Mapping[str, object]
) -> dict[str, object]:
    return {
        "authority_schema": AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA,
        "observation": dict(result),
        "plan": dict(plan),
        "replay_namespace": (
            AUTHENTICATED_ROLLOUT_OBSERVATION_REPLAY_NAMESPACE
        ),
    }


def verify_authenticated_result(
    result: Mapping[str, object],
    *,
    plan: Mapping[str, object],
    authority_envelope: bytes,
    durable_receipt: bytes,
) -> tuple[str, str]:
    """Historically verify one observation receipt without issuing another."""

    _require_authenticated_rollout_observation_authority(require_signing=False)
    identifiers = _validate_unsigned_result_structure(result, plan=plan)
    try:
        envelope = taira_authority_client.decode_canonical_json(
            authority_envelope, "rollout authority envelope"
        )
        receipt = taira_authority_client.decode_canonical_json(
            durable_receipt, "rollout durable receipt"
        )
        taira_authority_client.verify_receipt(
            "rollout-observation",
            _rollout_authority_subject(result, plan),
            authority_envelope=envelope,
            durable_receipt=receipt,
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise RolloutContractError(
            f"rollout-observation historical receipt verification failed: {error}"
        ) from error
    return identifiers


def verify_authenticated_result_files(
    *,
    plan_path: Path,
    result_path: Path,
    authority_envelope_path: Path,
    durable_receipt_path: Path,
) -> tuple[str, str]:
    """Stable-load and historically verify four explicit observation files."""

    # This native authentication is deliberately before the first caller path
    # read.  ``verify_authenticated_result`` repeats it immediately before the
    # native historical-verification operation.
    _require_authenticated_rollout_observation_authority(require_signing=False)
    plan = _load(plan_path, MAX_PLAN_BYTES, "rollout plan")
    result = _load(result_path, MAX_RESULT_BYTES, "rollout observation")
    authority_envelope = _read_stable(
        authority_envelope_path,
        MAX_AUTHORITY_SIDECAR_BYTES,
        "rollout authority envelope",
    )
    durable_receipt = _read_stable(
        durable_receipt_path,
        MAX_AUTHORITY_SIDECAR_BYTES,
        "rollout durable receipt",
    )
    return verify_authenticated_result(
        result,
        plan=plan,
        authority_envelope=authority_envelope,
        durable_receipt=durable_receipt,
    )


def _write_sidecar(path: Path, payload: bytes) -> None:
    """Publish one new receipt sidecar without following or replacing links."""

    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(path, flags, 0o644)
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("short write while publishing rollout authority sidecar")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _load(path: Path, maximum: int, label: str) -> dict[str, object]:
    return _decode_canonical(_read_stable(path, maximum, label), maximum, label)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    plan_parser = subparsers.add_parser(
        "verify-plan", help="verify the immutable checked-in rollout plan"
    )
    plan_parser.add_argument("--plan", type=Path, required=True)
    result_parser = subparsers.add_parser(
        "verify-result", help="verify one authenticated-controller observation record"
    )
    result_parser.add_argument("--plan", type=Path, required=True)
    result_parser.add_argument("--result", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        # Refuse observation validation before plan/result path access.  A
        # canonical self-hash and an owner-private path do not prove runtime
        # origin or bind the record to the admitted and deployed candidate.
        if args.command == "verify-result":
            require_authenticated_rollout_observation_authority_provisioned()
        plan = _load(args.plan, MAX_PLAN_BYTES, "rollout plan")
        if args.command == "verify-plan":
            plan_sha256 = validate_plan(plan)
            output = {
                "plan_sha256": plan_sha256,
                "qualification_authority": False,
                "schema": "iroha.taira.privacy_rollout_plan_verification",
                "schema_version": 1,
                "status": "structurally-valid",
            }
        else:
            result = _load(args.result, MAX_RESULT_BYTES, "rollout observation")
            authenticated = validate_result(result, plan=plan)
            plan_sha256, rollout_id = authenticated
            result_path = Path(os.path.abspath(args.result))
            envelope_path = result_path.with_name(
                result_path.name + AUTHORITY_ENVELOPE_SUFFIX
            )
            receipt_path = result_path.with_name(
                result_path.name + DURABLE_RECEIPT_SUFFIX
            )
            _write_sidecar(envelope_path, authenticated.authority_envelope)
            _write_sidecar(receipt_path, authenticated.durable_receipt)
            output = {
                "authority_operation_id": authenticated.operation_id,
                "authority_receipt_sha256": hashlib.sha256(
                    authenticated.durable_receipt
                ).hexdigest(),
                "plan_sha256": plan_sha256,
                "qualification_authority": True,
                "rollout_id": rollout_id,
                "schema": "iroha.taira.privacy_rollout_observation_verification",
                "schema_version": 1,
                "status": "structurally-valid",
            }
        sys.stdout.buffer.write(canonical_json_bytes(output))
        return 0
    except (OSError, RolloutContractError) as exc:
        print(f"Taira privacy rollout contract rejected: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
