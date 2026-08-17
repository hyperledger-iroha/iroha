"""Focused tests for the deployed-public-Taira 24-hour evidence verifier."""

from __future__ import annotations

import copy
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
from typing import Any, Callable

import pytest

from scripts import check_taira_public_v2_24h_soak_evidence as checker
from scripts import render_taira_validator_bundle as receipt_renderer


TEST_DURATION_MS = 2_000
TEST_SAMPLE_INTERVAL_MS = 1_000
TEST_TRANSFER_COUNT = checker.TARGET_TPS * TEST_DURATION_MS // 1_000
TEST_DRAIN_MS = 60


@pytest.fixture(autouse=True)
def short_structural_profile(monkeypatch: pytest.MonkeyPatch) -> None:
    """Use a small inventory while preserving the production formulas."""

    monkeypatch.setattr(checker, "DURATION_SECS", TEST_DURATION_MS // 1_000)
    monkeypatch.setattr(checker, "DURATION_MS", TEST_DURATION_MS)
    monkeypatch.setattr(checker, "REQUIRED_TRANSFER_COUNT", TEST_TRANSFER_COUNT)
    monkeypatch.setattr(checker, "SAMPLE_INTERVAL_MS", TEST_SAMPLE_INTERVAL_MS)
    monkeypatch.setattr(checker, "MAX_OBSERVATION_START_LATENESS_MS", 100)
    monkeypatch.setattr(checker, "MAXIMUM_SAMPLE_GAP_MS",
                        TEST_SAMPLE_INTERVAL_MS + 100)
    monkeypatch.setattr(checker, "MAX_OBSERVATION_WINDOW_MS", 100)
    monkeypatch.setattr(checker, "MAX_ANCHOR_TO_WORKLOAD_GAP_MS", 100)
    monkeypatch.setattr(checker, "MAX_SUBMISSION_START_LATENESS_MS", 250)
    monkeypatch.setattr(checker, "MAX_SUBMISSION_REQUEST_WINDOW_MS", 500)
    monkeypatch.setattr(checker, "MAX_CONFIRMATION_DRAIN_MS", 1_000)


def digest(label: str) -> str:
    """Return a deterministic nonzero SHA-256-shaped fixture value."""

    return hashlib.sha256(label.encode("ascii")).hexdigest()


def iroha_hash(label: str, hash_type: str) -> dict[str, str]:
    """Return one explicitly marked Iroha HashOf fixture."""

    value = bytearray(hashlib.sha256(label.encode("ascii")).digest())
    value[-1] |= 1
    return {
        "algorithm": checker.IROHA_HASH_ALGORITHM,
        "type": hash_type,
        "value": value.hex(),
    }


def public_key(label: str) -> dict[str, str]:
    """Return one marked compressed secp256k1 receipt key."""

    scalar = (
        int.from_bytes(hashlib.sha256(label.encode("ascii")).digest(), "big")
        % (receipt_renderer.SECP256K1_GROUP_ORDER - 1)
    ) + 1
    payload = receipt_renderer._secp256k1_public_payload(
        scalar.to_bytes(32, "big")
    )
    return {
        "algorithm": "secp256k1",
        "payload_hex": payload.hex(),
    }


def test_receipt_node_domain_matches_runtime_renderer() -> None:
    key = public_key("cross-module-receipt-node")
    canonical = (
        receipt_renderer.RECEIPT_PUBLIC_KEY_PREFIX
        + key["payload_hex"].upper()
    )

    assert checker._receipt_node_id(key) == receipt_renderer.receipt_node_id(canonical)


def artifact(label: str, payload: bytes) -> checker.Artifact:
    """Build one in-memory artifact for structural validation."""

    return checker.Artifact(
        path=Path(f"/evidence/{label}"), payload=payload,
        sha256=hashlib.sha256(payload).hexdigest(), size=len(payload), device=1,
        inode=int.from_bytes(hashlib.sha256(label.encode()).digest()[:8], "big"),
    )


def inventory_artifact(
    label: str, kind: str, schema: str, records: list[dict[str, Any]],
) -> tuple[checker.Artifact, dict[str, Any], str]:
    """Build one canonical typed JSONL inventory and reference."""

    header = {"schema": schema, "schema_version": 1, "record_count": len(records)}
    lines = [checker._canonical_json(record) for record in records]
    value = artifact(
        label, checker._canonical_json(header) + b"".join(lines)
    )
    hasher = hashlib.sha256()
    hasher.update(
        f"iroha.taira.public-v2-24h.{kind}-records.v1\0".encode("ascii")
    )
    for line in lines:
        hasher.update(line)
    records_sha256 = hasher.hexdigest()
    reference = {
        "kind": kind,
        "schema": schema,
        "sha256": value.sha256,
        "size_bytes": value.size,
        "record_count": len(records),
        "records_sha256": records_sha256,
    }
    return value, reference, records_sha256


def handoff_artifact(
    label: str, kind: str, source: dict[str, Any], identity: dict[str, Any],
) -> tuple[checker.Artifact, dict[str, Any]]:
    """Build one canonical prerequisite handoff document and reference."""

    value = artifact(
        label,
        checker._canonical_json(
            {
                "schema": checker.HANDOFF_SCHEMA,
                "schema_version": 1,
                "kind": kind,
                "source": source,
                "identity": identity,
            }
        ),
    )
    return value, {
        "kind": kind,
        "schema": checker.HANDOFF_SCHEMA,
        "sha256": value.sha256,
        "size_bytes": value.size,
        "source": source,
    }


@dataclass
class EvidenceFixture:
    """One internally consistent structural receipt and linked artifacts."""

    receipt: dict[str, Any]
    receipt_artifact: checker.Artifact
    source: dict[str, Any]
    binary_sha256: str
    native_binary_sha256: str
    native_source_sha256: str
    authority: checker.Artifact
    durable_admission: checker.Artifact
    candidate: checker.Artifact
    publication: checker.Artifact
    deploy: checker.Artifact
    workload: checker.Artifact
    submissions: checker.Artifact
    statuses: checker.Artifact
    blocks: checker.Artifact
    lifecycle: checker.Artifact
    lifecycle_journal: checker.Artifact
    lifecycle_native_receipt: checker.Artifact


def make_fixture() -> EvidenceFixture:
    """Create a two-second analogue of the exact public soak contract."""

    source = {
        "commit": "1a" * 20,
        "dpn_validator_release_commit": "3c" * 20,
        "cargo_lock_sha256": digest("cargo-lock"),
        "workspace_source_manifest_sha256": digest("source-manifest"),
    }
    binary_sha256 = digest("iroha3d-release-binary")
    native_binary_sha256 = digest("native-verifier-binary")
    native_source_sha256 = digest("native-verifier-source")
    started_ms = 1_800_000_000_000
    completed_ms = started_ms + TEST_DURATION_MS + TEST_DRAIN_MS
    genesis_block_hash = iroha_hash("genesis-block", checker.BLOCK_HASH_TYPE)
    qualification_id = digest("qualification-receipt")
    admission_archive_sha256 = digest("candidate-admission-archive")
    candidate_identity = {
        "qualification_receipt_id": qualification_id,
        "handoff_inventory_sha256": digest("candidate-handoff-inventory"),
        "admission_archive_sha256": admission_archive_sha256,
        "admission_authority_manifest_sha256": digest("admission-authority"),
        "validator_binary_sha256": binary_sha256,
    }
    candidate, candidate_reference = handoff_artifact(
        "candidate.json", "candidate", source, candidate_identity
    )
    publication_identity = {
        "qualification_receipt_id": qualification_id,
        "candidate_handoff_sha256": candidate.sha256,
        "handoff_inventory_sha256": candidate_identity[
            "handoff_inventory_sha256"
        ],
        "admission_archive_sha256": admission_archive_sha256,
        "validator_binary_sha256": binary_sha256,
        "publication_receipt_sha256": digest("publication-receipt"),
        "publication_signature_sha256": digest("publication-signature"),
        "publication_public_key_sha256": digest("publication-public-key"),
        "published_primary_oci_manifest_sha256": digest("primary-oci"),
        "published_receipt_oci_manifest_sha256": digest("receipt-oci"),
        "publisher_controller_sha256": digest("publisher-controller"),
    }
    publication, publication_reference = handoff_artifact(
        "publication.json", "publication", source, publication_identity
    )
    restart_generation = digest("restart-generation")
    receipt_signers: dict[str, dict[str, object]] = {}
    for validator in checker.VALIDATORS:
        key = public_key(f"receipt-key-{validator}")
        binary_stat_seal = [41, 73, 1_000_000, 2_000_000, 3_000_000]
        config_sha256 = digest(f"config-{validator}")
        runtime_binding = checker._runtime_binding_sha256(
            binary_sha256,
            binary_stat_seal,
            config_sha256,
            restart_generation,
        )
        node_id = checker._receipt_node_id(key)
        receipt_signers[validator] = {
            "binary_stat_seal": binary_stat_seal,
            "config_sha256": config_sha256,
            "lifecycle_binding_sha256": checker._lifecycle_binding_sha256(
                runtime_binding,
                restart_generation,
                validator,
                node_id,
            ),
            "node_id": node_id,
            "public_key": key,
            "runtime_binding_sha256": runtime_binding,
            "native_verifier_binary_sha256": native_binary_sha256,
            "native_verifier_source_sha256": native_source_sha256,
            "native_verifier_receipt_sha256": digest(
                f"native-deploy-receipt-{validator}"
            ),
            "native_verifier_receipt_size_bytes": 256,
            "verification_result": "verified",
        }
    deploy_end_hash = iroha_hash("deploy-end-block", checker.BLOCK_HASH_TYPE)
    deploy_identity = {
        "qualification_receipt_id": qualification_id,
        "candidate_handoff_sha256": candidate.sha256,
        "publication_handoff_sha256": publication.sha256,
        "handoff_inventory_sha256": candidate_identity[
            "handoff_inventory_sha256"
        ],
        "publication_receipt_sha256": publication_identity[
            "publication_receipt_sha256"
        ],
        "published_primary_oci_manifest_sha256": publication_identity[
            "published_primary_oci_manifest_sha256"
        ],
        "admission_receipt_id": digest("deploy-admission-receipt"),
        "admission_archive_sha256": admission_archive_sha256,
        "deploy_receipt_sha256": digest("deploy-receipt"),
        "deploy_handoff_manifest_sha256": digest("deploy-handoff-manifest"),
        "controller_host_id": "taira-deploy-host:fixture",
        "controller_installation_id": "taira-deploy-installation:fixture",
        "controller_sha256": digest("deploy-controller"),
        "validator_binary_sha256": binary_sha256,
        "signed_genesis_sha256": digest("fresh-signed-genesis"),
        "topology_sha256": digest("topology"),
        "config_set_sha256": digest("config-set"),
        "supervisor_sha256": digest("supervisor"),
        "restart_generation": restart_generation,
        "network_name": checker.taira_constants.NETWORK_NAME,
        "chain_id": checker.taira_constants.CHAIN_ID,
        "network_id": checker.taira_constants.NETWORK_ID,
        "protocol_version": checker.PROTOCOL_VERSION,
        "genesis_block_hash": copy.deepcopy(genesis_block_hash),
        "deployment_completed_at_unix_ms": started_ms - 300,
        "start_height": 10,
        "end_height": 11,
        "end_block_hash": deploy_end_hash,
        "receipt_signers": receipt_signers,
    }
    deploy, deploy_reference = handoff_artifact(
        "deploy.json", "deploy", source, deploy_identity
    )

    block_records: list[dict[str, Any]] = []
    previous_hash = iroha_hash("deploy-parent", checker.BLOCK_HASH_TYPE)
    for index in range(4):
        block_hash = (
            copy.deepcopy(deploy_end_hash)
            if index == 0
            else iroha_hash(f"block-{index}", checker.BLOCK_HASH_TYPE)
        )
        block_records.append(
            {
                "index": index,
                "height": 11 + index,
                "block_hash": block_hash,
                "parent_block_hash": copy.deepcopy(previous_hash),
                "signed_block_wire_sha256": digest(f"signed-block-wire-{index}"),
                "signed_block_wire_size_bytes": 1_000 + index,
                "finality_proof_sha256": digest(f"finality-proof-{index}"),
                "finality_proof_size_bytes": 200 + index,
                "finality_validators": list(checker.VALIDATORS[: checker.QUORUM]),
                "native_verifier_receipt_sha256": digest(
                    f"native-block-receipt-{index}"
                ),
                "native_verifier_receipt_size_bytes": 300 + index,
                "verification_result": "verified",
            }
        )
        previous_hash = block_hash
    blocks, blocks_reference, block_records_sha256 = inventory_artifact(
        "blocks.jsonl", "blocks", checker.BLOCK_SCHEMA, block_records
    )

    anchor_validators = []
    for validator in checker.VALIDATORS:
        anchor_validators.append(
            {
                "validator_id": validator,
                "node_id": receipt_signers[validator]["node_id"],
                "challenge_hex": digest(f"anchor-challenge-{validator}"),
                "attestation_sha256": digest(f"anchor-attestation-{validator}"),
                "attestation_size_bytes": 200,
                "attested_at_unix_ms": started_ms - 70,
                "tip_block_evidence_index": 1,
                "ancestry_proof_sha256": digest(f"ancestry-{validator}"),
                "ancestry_proof_size_bytes": 300,
                "native_verifier_receipt_sha256": digest(
                    f"native-anchor-{validator}"
                ),
                "native_verifier_receipt_size_bytes": 150,
                "verification_result": "verified",
            }
        )
    anchor = {
        "schema": "iroha.taira.public-v2-24h-soak-anchor.v1",
        "observation_started_at_unix_ms": started_ms - 100,
        "observation_completed_at_unix_ms": started_ms - 50,
        "controller_host_id": deploy_identity["controller_host_id"],
        "controller_installation_id": deploy_identity[
            "controller_installation_id"
        ],
        "controller_sha256": deploy_identity["controller_sha256"],
        "controller_signing_key_id": digest("anchor-controller-key"),
        "controller_receipt_sha256": digest("anchor-controller-receipt"),
        "controller_signature_sha256": digest("anchor-controller-signature"),
        "deploy_end_height": deploy_identity["end_height"],
        "deploy_end_block_hash": copy.deepcopy(deploy_end_hash),
        "common_start_block_evidence_index": 1,
        "validators": anchor_validators,
    }
    anchor_sha256 = checker._domain_digest(
        b"iroha.taira.public-v2-24h.anchor.v1\0", anchor
    )

    baseline_validators = [
        {
            "validator_id": validator,
            "node_id": receipt_signers[validator]["node_id"],
            "restart_count": 7,
            "supervisor_generation": 2,
            "process_generation": 9,
            "unexpected_exit_total": 3,
        }
        for validator in checker.VALIDATORS
    ]
    lifecycle_journal_records: list[dict[str, Any]] = []
    for pass_index in range(2):
        for validator_index, validator in enumerate(checker.VALIDATORS):
            index = pass_index * checker.VALIDATOR_COUNT + validator_index
            lifecycle_journal_records.append(
                {
                    "index": index,
                    "journal_sequence": 101 + index,
                    "observed_at_unix_ms": (
                        started_ms - 190 + index
                        if pass_index == 0
                        else completed_ms - 10 + validator_index
                    ),
                    "validator_id": validator,
                    "node_id": receipt_signers[validator]["node_id"],
                    "event": "healthy",
                    "restart_count": 7,
                    "supervisor_generation": 2,
                    "process_generation": 9,
                    "unexpected_exit_total": 3,
                }
            )
    lifecycle_journal, lifecycle_journal_reference, journal_records_sha256 = (
        inventory_artifact(
            "lifecycle-journal.jsonl", "lifecycle-journal",
            checker.LIFECYCLE_JOURNAL_SCHEMA, lifecycle_journal_records,
        )
    )
    lifecycle_value = {
        "schema": checker.LIFECYCLE_SCHEMA,
        "schema_version": 1,
        "deployment_completed_at_unix_ms": deploy_identity[
            "deployment_completed_at_unix_ms"
        ],
        "restart_generation": deploy_identity["restart_generation"],
        "config_set_sha256": deploy_identity["config_set_sha256"],
        "topology_sha256": deploy_identity["topology_sha256"],
        "signed_genesis_sha256": deploy_identity["signed_genesis_sha256"],
        "supervisor_sha256": deploy_identity["supervisor_sha256"],
        "genesis_block_hash": copy.deepcopy(genesis_block_hash),
        "raw_windows": [
            {
                "artifact_sha256": digest(f"raw-window-{validator}"),
                "artifact_size_bytes": 4_096 + index,
                "baseline_sequence": 10 * index,
                "binding_sha256": receipt_signers[validator][
                    "lifecycle_binding_sha256"
                ],
                "node_id": receipt_signers[validator]["node_id"],
                "record_count": 2,
                "records_sha256": digest(f"raw-window-records-{validator}"),
                "terminal_sequence": 10 * index + 2,
                "validator_id": validator,
            }
            for index, validator in enumerate(checker.VALIDATORS, start=1)
        ],
        "journal_inventory": lifecycle_journal_reference,
        "native_journal_verifier_receipt": {
            "sha256": digest("placeholder-journal-verifier-receipt"),
            "size_bytes": 1,
        },
        "baseline": {
            "captured_at_unix_ms": started_ms - 200,
            "journal_sequence": 100,
            "journal_chain_sha256": digest("journal-baseline"),
            "validators": baseline_validators,
        },
        "terminal": {
            "captured_at_unix_ms": completed_ms,
            "journal_sequence": 108,
            "journal_chain_sha256": digest("journal-terminal"),
            "validators": copy.deepcopy(baseline_validators),
        },
        "unexpected_exit_events": 0,
        "restart_events": 0,
    }
    lifecycle_window_sha256 = checker._lifecycle_window_digest(lifecycle_value)
    lifecycle_native_receipt_value = {
        "schema": checker.LIFECYCLE_JOURNAL_RECEIPT_SCHEMA,
        "schema_version": 1,
        "protocol": checker.NATIVE_JOURNAL_VERIFIER_PROTOCOL,
        "verifier_binary_sha256": native_binary_sha256,
        "verifier_source_sha256": native_source_sha256,
        "journal_artifact_sha256": lifecycle_journal.sha256,
        "journal_artifact_size_bytes": lifecycle_journal.size,
        "journal_records_sha256": journal_records_sha256,
        "journal_record_count": len(lifecycle_journal_records),
        "lifecycle_window_sha256": lifecycle_window_sha256,
        "verification_result": "verified",
    }
    lifecycle_native_receipt = artifact(
        "lifecycle-native-verifier-receipt.json",
        checker._canonical_json(lifecycle_native_receipt_value),
    )
    lifecycle_value["native_journal_verifier_receipt"] = {
        "sha256": lifecycle_native_receipt.sha256,
        "size_bytes": lifecycle_native_receipt.size,
    }
    lifecycle = artifact(
        "lifecycle.json", checker._canonical_json(lifecycle_value)
    )
    lifecycle_identity_sha256 = checker._domain_digest(
        b"iroha.taira.public-v2-24h.lifecycle.v1\0", lifecycle_value
    )
    lifecycle_reference = {
        "kind": "lifecycle-evidence",
        "schema": checker.LIFECYCLE_SCHEMA,
        "sha256": lifecycle.sha256,
        "size_bytes": lifecycle.size,
        "identity_sha256": lifecycle_identity_sha256,
    }

    samples: list[dict[str, Any]] = []
    for sample_index, scheduled in enumerate((1_000, 2_000)):
        common_index = 2 if sample_index == 0 else 3
        observed_start = started_ms + scheduled + 20
        observed_end = started_ms + scheduled + 60
        samples.append(
            {
                "scheduled_elapsed_ms": scheduled,
                "observation_started_at_unix_ms": observed_start,
                "observation_completed_at_unix_ms": observed_end,
                "applied_transfer_count": 5 * (sample_index + 1),
                "failed_transfer_count": 0,
                "common_block_evidence_index": common_index,
                "validators": [
                    {
                        "validator_id": validator,
                        "node_id": receipt_signers[validator]["node_id"],
                        "challenge_hex": digest(
                            f"sample-{sample_index}-challenge-{validator}"
                        ),
                        "attestation_sha256": digest(
                            f"sample-{sample_index}-attestation-{validator}"
                        ),
                        "attestation_size_bytes": 210,
                        "attested_at_unix_ms": observed_start + 10,
                        "tip_block_evidence_index": common_index,
                        "queue_depth": 1,
                        "queue_capacity": 1_024,
                        "queue_saturated": False,
                        "queue_dropped_total": 0,
                        "restart_count": 7,
                        "supervisor_generation": 2,
                        "process_generation": 9,
                        "unexpected_exit_total": 3,
                        "restart_required": False,
                        "last_restart_successful": True,
                        "healthy": True,
                        "native_verifier_receipt_sha256": digest(
                            f"native-sample-{sample_index}-{validator}"
                        ),
                        "native_verifier_receipt_size_bytes": 160,
                        "verification_result": "verified",
                    }
                    for validator in checker.VALIDATORS
                ],
            }
        )
    sample_set_sha256 = checker._domain_digest(
        b"iroha.taira.public-v2-24h.sample-set.v1\0", samples
    )

    submission_records: list[dict[str, Any]] = []
    status_records: list[dict[str, Any]] = []
    workload_records: list[dict[str, Any]] = []
    for sequence in range(TEST_TRANSFER_COUNT):
        signed = iroha_hash(
            f"signed-transaction-{sequence}", checker.SIGNED_TRANSACTION_HASH_TYPE
        )
        entrypoint = {
            "algorithm": checker.IROHA_HASH_ALGORITHM,
            "type": checker.ENTRYPOINT_HASH_TYPE,
            "value": signed["value"],
        }
        scheduled = sequence * checker.SLOT_INTERVAL_MS
        request_start = scheduled + 10
        # This intentionally overlaps subsequent 200ms slots.
        request_end = request_start + 220
        signer_validator = checker.VALIDATORS[sequence % checker.VALIDATOR_COUNT]
        submission_records.append(
            {
                "index": sequence,
                "signed_transaction_hash": copy.deepcopy(signed),
                "entrypoint_hash": copy.deepcopy(entrypoint),
                "receipt_sha256": digest(f"submission-receipt-{sequence}"),
                "receipt_size_bytes": 180,
                "submitted_at_unix_ms": started_ms + request_start + 5,
                "submitted_at_height": 11,
                "receipt_signer_validator_id": signer_validator,
                "receipt_signer_node_id": receipt_signers[signer_validator][
                    "node_id"
                ],
                "receipt_signer_public_key": copy.deepcopy(
                    receipt_signers[signer_validator]["public_key"]
                ),
                "native_verifier_receipt_sha256": digest(
                    f"native-submission-{sequence}"
                ),
                "native_verifier_receipt_size_bytes": 170,
                "verification_result": "verified",
            }
        )
        observation_index = 0 if sequence < 5 else 1
        observed_ms = started_ms + (900 if observation_index == 0 else 2_030)
        observed_ms += sequence % 5
        block_index = 2 if observation_index == 0 else 3
        status_records.append(
            {
                "index": sequence,
                "signed_transaction_hash": copy.deepcopy(signed),
                "entrypoint_hash": copy.deepcopy(entrypoint),
                "result": "Applied",
                "observed_at_unix_ms": observed_ms,
                "observation_index": observation_index,
                "block_evidence_index": block_index,
                "response_sha256": digest(f"status-response-{sequence}"),
                "response_size_bytes": 190,
                "native_verifier_receipt_sha256": digest(
                    f"native-status-{sequence}"
                ),
                "native_verifier_receipt_size_bytes": 175,
                "verification_result": "verified",
            }
        )
        workload_records.append(
            {
                "sequence": sequence,
                "operation": "transfer",
                "scheduled_elapsed_ms": scheduled,
                "request_started_elapsed_ms": request_start,
                "request_completed_elapsed_ms": request_end,
                "signed_transaction_hash": copy.deepcopy(signed),
                "entrypoint_hash": copy.deepcopy(entrypoint),
                "versioned_signed_transaction_sha256": digest(
                    f"versioned-signed-transaction-{sequence}"
                ),
                "versioned_signed_transaction_size_bytes": 512,
                "submission_receipt_index": sequence,
                "applied_status_index": sequence,
                "block_evidence_index": block_index,
            }
        )
    submissions, submissions_reference, submission_records_sha256 = (
        inventory_artifact(
            "submissions.jsonl", "submissions", checker.SUBMISSION_SCHEMA,
            submission_records
        )
    )
    statuses, statuses_reference, status_records_sha256 = inventory_artifact(
        "statuses.jsonl", "statuses", checker.STATUS_SCHEMA, status_records
    )
    workload, workload_reference, workload_records_sha256 = inventory_artifact(
        "workload.jsonl", "workload", checker.WORKLOAD_SCHEMA, workload_records
    )
    workload_reference.update(
        {
            "first_signed_transaction_hash": copy.deepcopy(
                workload_records[0]["signed_transaction_hash"]
            ),
            "last_signed_transaction_hash": copy.deepcopy(
                workload_records[-1]["signed_transaction_hash"]
            ),
        }
    )

    source_tuple_sha256 = checker._domain_digest(
        b"iroha.taira.public-v2-24h.source-tuple.v1\0", source
    )
    native_verifier = {
        "protocol": checker.NATIVE_VERIFIER_PROTOCOL,
        "binary_sha256": native_binary_sha256,
        "source_sha256": native_source_sha256,
    }
    native_verifier_identity_sha256 = checker._domain_digest(
        b"iroha.taira.public-v2-24h.native-verifier.v1\0", native_verifier
    )
    receipt = {
        "schema": checker.SCHEMA,
        "schema_version": checker.SCHEMA_VERSION,
        "result": checker.RESULT,
        "network": {
            "name": checker.taira_constants.NETWORK_NAME,
            "deployment": "public",
            "chain_id": checker.taira_constants.CHAIN_ID,
            "network_id": checker.taira_constants.NETWORK_ID,
            "protocol_version": checker.PROTOCOL_VERSION,
            "genesis_block_hash": copy.deepcopy(genesis_block_hash),
        },
        "source": source,
        "profile": {
            "duration_ms": TEST_DURATION_MS,
            "validator_count": checker.VALIDATOR_COUNT,
            "quorum": checker.QUORUM,
            "target_tps": checker.TARGET_TPS,
            "slot_interval_ms": checker.SLOT_INTERVAL_MS,
            "required_transfer_slots": TEST_TRANSFER_COUNT,
            "sample_interval_ms": TEST_SAMPLE_INTERVAL_MS,
            "maximum_sample_gap_ms": TEST_SAMPLE_INTERVAL_MS + 100,
            "maximum_observation_start_lateness_ms": 100,
            "maximum_observation_window_ms": 100,
            "maximum_anchor_to_workload_gap_ms": 100,
            "maximum_submission_start_lateness_ms": 250,
            "maximum_submission_request_window_ms": 500,
            "maximum_confirmation_drain_ms": 1_000,
            "maximum_wall_clock_skew_ms": checker.MAX_WALL_CLOCK_SKEW_MS,
            "workload": checker.WORKLOAD,
            "fault_injection": checker.FAULT_INJECTION,
        },
        "candidate_handoff": candidate_reference,
        "publication_handoff": publication_reference,
        "deploy_handoff": deploy_reference,
        "native_verifier": native_verifier,
        "soak_anchor": anchor,
        "samples": samples,
        "workload_inventory": workload_reference,
        "submission_receipt_inventory": submissions_reference,
        "applied_status_inventory": statuses_reference,
        "block_evidence_inventory": blocks_reference,
        "lifecycle": lifecycle_reference,
        "completion": {
            "state": "completed",
            "publication": "atomic-rename",
            "natural_completion": True,
            "workload_started_at_unix_ms": started_ms,
            "workload_ended_at_unix_ms": started_ms + TEST_DURATION_MS,
            "evidence_completed_at_unix_ms": completed_ms,
            "workload_duration_ms": TEST_DURATION_MS,
            "confirmation_drain_ms": TEST_DRAIN_MS,
            "transfer_slot_count": TEST_TRANSFER_COUNT,
            "sample_count": len(samples),
            "anchor_to_workload_gap_ms": 50,
            "maximum_observed_sample_gap_ms": TEST_SAMPLE_INTERVAL_MS + 20,
            "maximum_observation_window_ms": 40,
            "maximum_submission_start_lateness_ms": 10,
            "maximum_submission_request_window_ms": 220,
            "applied_transfer_count": TEST_TRANSFER_COUNT,
            "failed_transfer_count": 0,
            "queue_drop_events": 0,
            "unhealthy_samples": 0,
            "restart_events": 0,
            "unexpected_exit_events": 0,
            "source_tuple_sha256": source_tuple_sha256,
            "candidate_handoff_sha256": candidate.sha256,
            "publication_handoff_sha256": publication.sha256,
            "deploy_handoff_sha256": deploy.sha256,
            "native_verifier_identity_sha256": native_verifier_identity_sha256,
            "anchor_sha256": anchor_sha256,
            "sample_set_sha256": sample_set_sha256,
            "workload_inventory_sha256": workload.sha256,
            "workload_records_sha256": workload_records_sha256,
            "submission_inventory_sha256": submissions.sha256,
            "submission_records_sha256": submission_records_sha256,
            "status_inventory_sha256": statuses.sha256,
            "status_records_sha256": status_records_sha256,
            "block_inventory_sha256": blocks.sha256,
            "block_records_sha256": block_records_sha256,
            "lifecycle_artifact_sha256": lifecycle.sha256,
            "lifecycle_identity_sha256": lifecycle_identity_sha256,
            "lifecycle_journal_artifact_sha256": lifecycle_journal.sha256,
            "lifecycle_journal_records_sha256": journal_records_sha256,
            "lifecycle_journal_record_count": len(lifecycle_journal_records),
            "lifecycle_native_verifier_receipt_sha256": (
                lifecycle_native_receipt.sha256
            ),
            "lifecycle_window_sha256": lifecycle_window_sha256,
        },
    }
    receipt_artifact = artifact(
        checker.COMPLETION_FILENAME, checker._canonical_json(receipt)
    )
    subject_core = {
        "schema": checker.soak_authority.SUBJECT_SCHEMA,
        "receipt": {
            "sha256": receipt_artifact.sha256,
            "size_bytes": receipt_artifact.size,
        },
        "source": {"tuple_sha256": source_tuple_sha256},
        "prerequisites": {
            "candidate_handoff_sha256": candidate.sha256,
            "publication_handoff_sha256": publication.sha256,
            "deploy_handoff_sha256": deploy.sha256,
        },
        "anchor": {"sha256": anchor_sha256, "validator_count": 4},
        "samples": {"sha256": sample_set_sha256, "count": len(samples)},
        "workload": {
            "artifact_sha256": workload.sha256,
            "records_sha256": workload_records_sha256,
            "record_count": TEST_TRANSFER_COUNT,
        },
        "submission_receipts": {
            "artifact_sha256": submissions.sha256,
            "records_sha256": submission_records_sha256,
            "record_count": TEST_TRANSFER_COUNT,
        },
        "applied_statuses": {
            "artifact_sha256": statuses.sha256,
            "records_sha256": status_records_sha256,
            "record_count": TEST_TRANSFER_COUNT,
        },
        "blocks": {
            "artifact_sha256": blocks.sha256,
            "records_sha256": block_records_sha256,
            "record_count": len(block_records),
        },
        "lifecycle": {
            "artifact_sha256": lifecycle.sha256,
            "identity_sha256": lifecycle_identity_sha256,
            "journal_artifact_sha256": lifecycle_journal.sha256,
            "journal_records_sha256": journal_records_sha256,
            "journal_record_count": len(lifecycle_journal_records),
            "native_verifier_receipt_sha256": lifecycle_native_receipt.sha256,
            "window_sha256": lifecycle_window_sha256,
        },
        "native_verifier": {
            "binary_sha256": native_binary_sha256,
            "source_sha256": native_source_sha256,
        },
    }
    completed_at = completed_ms
    issued_at = completed_at + 1_000
    expires_at = issued_at + 60_000
    replay_id = digest("authority-replay")
    authority_payload = checker.soak_authority._canonical_json(
        {
            "schema": checker.AUTHORITY_SCHEMA,
            "schema_version": 1,
            "authority_key_id": digest("independent-authority-key"),
            "signature_algorithm": checker.soak_authority.SIGNATURE_ALGORITHM,
            "claims": {
                "schema": checker.soak_authority.CLAIMS_SCHEMA,
                "subject_digest": checker.soak_authority.subject_digest(subject_core),
                "replay_namespace": checker.soak_authority.REPLAY_NAMESPACE,
                "replay_id": replay_id,
                "issued_at_unix_ms": issued_at,
                "expires_at_unix_ms": expires_at,
            },
            "signature": "ab" * 64,
        }
    )
    authority = artifact("authority.json", authority_payload)
    durable_admission = artifact(
        "durable-admission.json",
        checker.soak_authority._canonical_json(
            {
                "schema": checker.soak_authority.ADMISSION_RECEIPT_SCHEMA,
                "schema_version": 1,
                "broker_key_id": digest("independent-broker-key"),
                "signature_algorithm": checker.soak_authority.SIGNATURE_ALGORITHM,
                "claims": {
                    "schema": checker.soak_authority.ADMISSION_CLAIMS_SCHEMA,
                    "decision": "admitted",
                    "receipt_id": digest("durable-admission-receipt"),
                    "subject_digest": checker.soak_authority.subject_digest(
                        subject_core
                    ),
                    "authority_envelope_sha256": hashlib.sha256(
                        authority_payload
                    ).hexdigest(),
                    "authority_key_id": digest("independent-authority-key"),
                    "replay_namespace": checker.soak_authority.REPLAY_NAMESPACE,
                    "replay_id": replay_id,
                    "admitted_at_unix_ms": issued_at + 1_000,
                },
                "signature": "cd" * 64,
            }
        ),
    )
    return EvidenceFixture(
        receipt, receipt_artifact, source, binary_sha256,
        native_binary_sha256, native_source_sha256, authority,
        durable_admission, candidate, publication, deploy, workload,
        submissions, statuses, blocks, lifecycle, lifecycle_journal,
        lifecycle_native_receipt,
    )


@pytest.fixture
def evidence() -> EvidenceFixture:
    """Provide one fresh valid receipt to each mutation test."""

    return make_fixture()


def validate(evidence: EvidenceFixture) -> checker.StructuralResult:
    """Run the post-authority private structural validator."""

    evidence.receipt_artifact = artifact(
        checker.COMPLETION_FILENAME, checker._canonical_json(evidence.receipt)
    )
    return checker._validate_structural_evidence(
        evidence.receipt, **structural_arguments(evidence)
    )


def structural_arguments(evidence: EvidenceFixture) -> dict[str, object]:
    """Return every captured input for structural or public validation."""

    return {
        "receipt_artifact": evidence.receipt_artifact,
        "expected_source": evidence.source,
        "expected_binary_sha256": evidence.binary_sha256,
        "expected_native_verifier_binary_sha256": evidence.native_binary_sha256,
        "expected_native_verifier_source_sha256": evidence.native_source_sha256,
        "authority_envelope": evidence.authority,
        "durable_admission_receipt": evidence.durable_admission,
        "candidate_handoff": evidence.candidate,
        "publication_handoff": evidence.publication,
        "deploy_handoff": evidence.deploy,
        "workload_inventory": evidence.workload,
        "submission_receipt_inventory": evidence.submissions,
        "applied_status_inventory": evidence.statuses,
        "block_evidence_inventory": evidence.blocks,
        "lifecycle_evidence": evidence.lifecycle,
        "lifecycle_journal": evidence.lifecycle_journal,
        "lifecycle_native_verifier_receipt": (
            evidence.lifecycle_native_receipt),
    }


def replace_inventory_record(
    evidence: EvidenceFixture,
    attribute: str,
    receipt_field: str,
    index: int,
    mutate: Callable[[dict[str, Any]], None],
    *, refresh_records_digest: bool = False,
) -> None:
    """Mutate one JSONL record and refresh only its outer artifact identity."""

    current = getattr(evidence, attribute)
    assert isinstance(current, checker.Artifact)
    lines = current.payload.splitlines(keepends=True)
    record = checker._decode_json(lines[index + 1], "record", canonical=True)
    mutate(record)
    lines[index + 1] = checker._canonical_json(record)
    changed = artifact(f"mutated-{attribute}.jsonl", b"".join(lines))
    setattr(evidence, attribute, changed)
    reference = evidence.receipt[receipt_field]
    reference["sha256"] = changed.sha256
    reference["size_bytes"] = changed.size
    if refresh_records_digest:
        hasher = hashlib.sha256()
        kind = reference["kind"]
        hasher.update(
            f"iroha.taira.public-v2-24h.{kind}-records.v1\0".encode("ascii")
        )
        for line in lines[1:]:
            hasher.update(line)
        reference["records_sha256"] = hasher.hexdigest()


def replace_handoff_identity(
    evidence: EvidenceFixture,
    attribute: str,
    receipt_field: str,
    mutate: Callable[[dict[str, Any]], None],
) -> None:
    """Mutate one handoff identity and refresh its receipt artifact link."""

    current = getattr(evidence, attribute)
    assert isinstance(current, checker.Artifact)
    value = checker._decode_json(current.payload, "handoff", canonical=True)
    identity = value["identity"]
    assert isinstance(identity, dict)
    mutate(identity)
    changed = artifact(f"mutated-{attribute}.json", checker._canonical_json(value))
    setattr(evidence, attribute, changed)
    evidence.receipt[receipt_field]["sha256"] = changed.sha256
    evidence.receipt[receipt_field]["size_bytes"] = changed.size


def replace_lifecycle(
    evidence: EvidenceFixture, mutate: Callable[[dict[str, Any]], None]
) -> None:
    """Mutate lifecycle evidence and refresh only its artifact link."""

    value = checker._decode_json(
        evidence.lifecycle.payload, "lifecycle", canonical=True
    )
    mutate(value)
    changed = artifact("mutated-lifecycle.json", checker._canonical_json(value))
    evidence.lifecycle = changed
    evidence.receipt["lifecycle"]["sha256"] = changed.sha256
    evidence.receipt["lifecycle"]["size_bytes"] = changed.size


def replace_lifecycle_journal_record(
    evidence: EvidenceFixture, index: int,
    mutate: Callable[[dict[str, Any]], None],
) -> None:
    """Mutate a journal row and refresh the journal and lifecycle links."""

    lines = evidence.lifecycle_journal.payload.splitlines(keepends=True)
    row = checker._decode_json(lines[index + 1], "lifecycle row", canonical=True)
    mutate(row)
    lines[index + 1] = checker._canonical_json(row)
    changed = artifact("mutated-lifecycle-journal.jsonl", b"".join(lines))
    evidence.lifecycle_journal = changed
    lifecycle = checker._decode_json(
        evidence.lifecycle.payload, "lifecycle", canonical=True
    )
    journal_reference = lifecycle["journal_inventory"]
    assert isinstance(journal_reference, dict)
    journal_reference["sha256"] = changed.sha256
    journal_reference["size_bytes"] = changed.size
    hasher = hashlib.sha256()
    hasher.update(
        b"iroha.taira.public-v2-24h.lifecycle-journal-records.v1\0"
    )
    for line in lines[1:]:
        hasher.update(line)
    journal_reference["records_sha256"] = hasher.hexdigest()
    changed_lifecycle = artifact(
        "mutated-lifecycle.json", checker._canonical_json(lifecycle)
    )
    evidence.lifecycle = changed_lifecycle
    evidence.receipt["lifecycle"]["sha256"] = changed_lifecycle.sha256
    evidence.receipt["lifecycle"]["size_bytes"] = changed_lifecycle.size


def write_fixture(root: Path, evidence: EvidenceFixture) -> list[str]:
    """Materialize one fixture and return verifier CLI arguments."""

    files = {
        checker.COMPLETION_FILENAME: evidence.receipt_artifact,
        "authority.json": evidence.authority,
        "durable.json": evidence.durable_admission,
        "candidate.json": evidence.candidate,
        "publication.json": evidence.publication,
        "deploy.json": evidence.deploy,
        "workload.jsonl": evidence.workload,
        "submissions.jsonl": evidence.submissions,
        "statuses.jsonl": evidence.statuses,
        "blocks.jsonl": evidence.blocks,
        "lifecycle.json": evidence.lifecycle,
        "lifecycle-journal.jsonl": evidence.lifecycle_journal,
        "lifecycle-native-receipt.json": evidence.lifecycle_native_receipt,
    }
    for name, value in files.items():
        (root / name).write_bytes(value.payload)
    return [
        str(root / checker.COMPLETION_FILENAME),
        "--observation-authority-envelope", str(root / "authority.json"),
        "--durable-admission-receipt", str(root / "durable.json"),
        "--candidate-handoff", str(root / "candidate.json"),
        "--publication-handoff", str(root / "publication.json"),
        "--deploy-handoff", str(root / "deploy.json"),
        "--workload-inventory", str(root / "workload.jsonl"),
        "--submission-receipt-inventory", str(root / "submissions.jsonl"),
        "--applied-status-inventory", str(root / "statuses.jsonl"),
        "--block-evidence-inventory", str(root / "blocks.jsonl"),
        "--lifecycle-evidence", str(root / "lifecycle.json"),
        "--lifecycle-journal", str(root / "lifecycle-journal.jsonl"),
        "--lifecycle-native-verifier-receipt",
        str(root / "lifecycle-native-receipt.json"),
        "--source-commit", evidence.source["commit"],
        "--dpn-validator-release-commit",
        evidence.source["dpn_validator_release_commit"],
        "--cargo-lock-sha256", evidence.source["cargo_lock_sha256"],
        "--workspace-source-manifest-sha256",
        evidence.source["workspace_source_manifest_sha256"],
        "--iroha3d-sha256", evidence.binary_sha256,
        "--native-verifier-binary-sha256", evidence.native_binary_sha256,
        "--native-verifier-source-sha256", evidence.native_source_sha256,
    ]


def test_production_contract_has_exact_432k_monotonic_slots(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.undo()
    assert checker.TARGET_TPS == 5
    assert checker.DURATION_MS == 86_400_000
    assert checker.SLOT_INTERVAL_MS == 200
    assert checker.REQUIRED_TRANSFER_COUNT == 432_000
    assert checker.QUORUM == 3
    assert checker.MAXIMUM_SAMPLE_GAP_MS == 90_000
    assert checker.MAX_ANCHOR_TO_WORKLOAD_GAP_MS == 30_000
    assert (checker.REQUIRED_TRANSFER_COUNT - 1) * checker.SLOT_INTERVAL_MS == 86_399_800
    assert checker.MAX_CONFIRMATION_DRAIN_MS > 0


def test_valid_structural_evidence_passes(evidence: EvidenceFixture) -> None:
    result = validate(evidence)
    assert result.authority_subject_core["workload"]["record_count"] == (
        TEST_TRANSFER_COUNT
    )
    assert result.authority_subject_core["blocks"]["record_count"] == 4
    assert result.authority_subject_core["lifecycle"][
        "journal_record_count"
    ] == 8


@pytest.mark.parametrize("forgery", ("digest", "size"))
def test_public_validator_rederives_captured_artifact_identity(
    evidence: EvidenceFixture, monkeypatch: pytest.MonkeyPatch, forgery: str,
) -> None:
    monkeypatch.setattr(checker, "_require_observation_authority", lambda: None)
    original = evidence.candidate
    if forgery == "digest":
        forged = checker.Artifact(
            original.path, original.payload + b" ", original.sha256,
            original.size + 1, original.device, original.inode,
        )
    else:
        forged = checker.Artifact(
            original.path, original.payload, original.sha256,
            original.size + 1, original.device, original.inode,
        )
    arguments = structural_arguments(evidence)
    arguments["candidate_handoff"] = forged
    with pytest.raises(checker.EvidenceError, match=f"captured {forgery}"):
        checker.validate_evidence(evidence.receipt, **arguments)


def test_public_validator_rejects_programmatic_inode_alias(
    evidence: EvidenceFixture, monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(checker, "_require_observation_authority", lambda: None)
    original = evidence.candidate
    alias = checker.Artifact(
        original.path, original.payload, original.sha256, original.size,
        evidence.publication.device, evidence.publication.inode,
    )
    arguments = structural_arguments(evidence)
    arguments["candidate_handoff"] = alias
    with pytest.raises(checker.EvidenceError, match="file alias"):
        checker.validate_evidence(evidence.receipt, **arguments)


def test_marked_iroha_hash_is_not_an_artifact_sha256() -> None:
    with pytest.raises(checker.EvidenceError, match="fields are not exact"):
        checker._iroha_hash(digest("artifact"), "transaction",  # type: ignore[arg-type]
                            checker.SIGNED_TRANSACTION_HASH_TYPE)
    with pytest.raises(checker.EvidenceError, match="HashOf type is wrong"):
        checker._iroha_hash(
            iroha_hash("transaction", checker.BLOCK_HASH_TYPE),
            "transaction", checker.SIGNED_TRANSACTION_HASH_TYPE,
        )
    unmarked = iroha_hash("unmarked", checker.SIGNED_TRANSACTION_HASH_TYPE)
    unmarked["value"] = unmarked["value"][:-2] + "00"
    with pytest.raises(checker.EvidenceError, match="marker bit"):
        checker._iroha_hash(
            unmarked, "transaction", checker.SIGNED_TRANSACTION_HASH_TYPE
        )
    assert checker._artifact_sha256(digest("artifact"), "artifact")


def test_public_validator_stops_at_unprovisioned_authority(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[str] = []

    def forbidden(*_args: object, **_kwargs: object) -> checker.StructuralResult:
        calls.append("structural validation")
        raise AssertionError("authority barrier crossed")

    monkeypatch.setattr(checker, "_validate_structural_evidence", forbidden)
    with pytest.raises(checker.EvidenceError, match=checker.AUTHORITY_SCHEMA):
        checker.validate_evidence({"attacker": True})
    assert calls == []


def test_cli_stops_before_caller_path_io(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str],
) -> None:
    calls: list[Path] = []

    def forbidden(path: Path, *_args: object, **_kwargs: object) -> checker.Artifact:
        calls.append(path)
        raise AssertionError("path read crossed the authority barrier")

    monkeypatch.setattr(checker, "_read_stable", forbidden)
    missing = "/missing/evidence"
    arguments = [
        f"{missing}/{checker.COMPLETION_FILENAME}",
        "--observation-authority-envelope", f"{missing}/authority",
        "--durable-admission-receipt", f"{missing}/durable",
        "--candidate-handoff", f"{missing}/candidate",
        "--publication-handoff", f"{missing}/publication",
        "--deploy-handoff", f"{missing}/deploy",
        "--workload-inventory", f"{missing}/workload",
        "--submission-receipt-inventory", f"{missing}/submissions",
        "--applied-status-inventory", f"{missing}/statuses",
        "--block-evidence-inventory", f"{missing}/blocks",
        "--lifecycle-evidence", f"{missing}/lifecycle",
        "--lifecycle-journal", f"{missing}/lifecycle-journal",
        "--lifecycle-native-verifier-receipt", f"{missing}/lifecycle-receipt",
        "--source-commit", "1a" * 20,
        "--dpn-validator-release-commit", "3c" * 20,
        "--cargo-lock-sha256", digest("lock"),
        "--workspace-source-manifest-sha256", digest("source"),
        "--iroha3d-sha256", digest("binary"),
        "--native-verifier-binary-sha256", digest("native-binary"),
        "--native-verifier-source-sha256", digest("native-source"),
    ]
    assert checker.main(arguments) == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert checker.AUTHORITY_SCHEMA in captured.err
    assert calls == []


@pytest.mark.parametrize(
    "payload, message",
    (
        (b'{"field":1,"field":2}\n', "duplicate JSON field"),
        (b'{"field":NaN}\n', "non-finite JSON number"),
        (b'{"field":Infinity}\n', "non-finite JSON number"),
    ),
)
def test_strict_json_rejects_duplicates_and_nonfinite_numbers(
    payload: bytes, message: str,
) -> None:
    with pytest.raises(checker.EvidenceError, match=message):
        checker._decode_json(payload, "hostile receipt", canonical=False)


def test_stable_reader_rejects_symlink_and_hard_link(tmp_path: Path) -> None:
    source = tmp_path / "source.json"
    source.write_bytes(b"{}\n")
    symbolic = tmp_path / "symbolic.json"
    symbolic.symlink_to(source)
    with pytest.raises(checker.EvidenceError, match="symbolic links"):
        checker._read_stable(symbolic, 10, "symbolic evidence")
    hard_link = tmp_path / "hard.json"
    os.link(source, hard_link)
    with pytest.raises(checker.EvidenceError, match="bounded owner-controlled"):
        checker._read_stable(source, 10, "linked evidence")


@pytest.mark.parametrize(
    "field, hostile",
    (
        ("target_tps", 3),
        ("required_transfer_slots", 9),
        ("duration_ms", 1_999),
        ("slot_interval_ms", 250),
        ("maximum_confirmation_drain_ms", 9_999),
        ("fault_injection", "packet-loss"),
    ),
)
def test_fixed_profile_cannot_be_weakened(
    evidence: EvidenceFixture, field: str, hostile: object,
) -> None:
    evidence.receipt["profile"][field] = hostile
    with pytest.raises(checker.EvidenceError, match="fixed public soak profile"):
        validate(evidence)


def test_source_tuple_uses_taira_candidate_identity(evidence: EvidenceFixture) -> None:
    hostile_source = dict(evidence.source)
    hostile_source["dpn_validator_release_commit"] = "4d" * 20
    with pytest.raises(checker.EvidenceError, match="trusted invocation"):
        checker._validate_structural_evidence(
            evidence.receipt,
            receipt_artifact=evidence.receipt_artifact,
            expected_source=hostile_source,
            expected_binary_sha256=evidence.binary_sha256,
            expected_native_verifier_binary_sha256=evidence.native_binary_sha256,
            expected_native_verifier_source_sha256=evidence.native_source_sha256,
            authority_envelope=evidence.authority,
            durable_admission_receipt=evidence.durable_admission,
            candidate_handoff=evidence.candidate,
            publication_handoff=evidence.publication,
            deploy_handoff=evidence.deploy,
            workload_inventory=evidence.workload,
            submission_receipt_inventory=evidence.submissions,
            applied_status_inventory=evidence.statuses,
            block_evidence_inventory=evidence.blocks,
            lifecycle_evidence=evidence.lifecycle,
            lifecycle_journal=evidence.lifecycle_journal,
            lifecycle_native_verifier_receipt=(
                evidence.lifecycle_native_receipt),
        )


def test_candidate_publication_and_deploy_qualification_are_cross_bound(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence, "publication", "publication_handoff",
        lambda identity: identity.__setitem__(
            "qualification_receipt_id", digest("other-qualification")
        ),
    )
    with pytest.raises(checker.EvidenceError, match="qualification IDs differ"):
        validate(evidence)


def test_publication_consumes_exact_candidate_handoff(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence, "publication", "publication_handoff",
        lambda identity: identity.__setitem__(
            "candidate_handoff_sha256", digest("other-candidate-handoff")
        ),
    )
    with pytest.raises(checker.EvidenceError, match="exact candidate handoff"):
        validate(evidence)


@pytest.mark.parametrize(
    "field",
    ("handoff_inventory_sha256", "admission_archive_sha256",
     "validator_binary_sha256"),
)
def test_publication_preserves_candidate_artifact_identities(
    evidence: EvidenceFixture, field: str,
) -> None:
    replace_handoff_identity(
        evidence, "publication", "publication_handoff",
        lambda identity: identity.__setitem__(field, digest(f"other-{field}")),
    )
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity.__setitem__(
            "publication_handoff_sha256", evidence.publication.sha256
        ),
    )
    with pytest.raises(checker.EvidenceError,
                       match="inventory identity differs|candidate archive|binary"):
        validate(evidence)


def test_deploy_consumes_exact_publication_handoff(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity.__setitem__(
            "publication_handoff_sha256", digest("other-publication-handoff")
        ),
    )
    with pytest.raises(checker.EvidenceError, match="exact publication handoff"):
        validate(evidence)


def test_deploy_network_and_genesis_are_receipt_bound(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity.__setitem__(
            "genesis_block_hash",
            iroha_hash("other-deploy-genesis", checker.BLOCK_HASH_TYPE),
        ),
    )
    with pytest.raises(checker.EvidenceError, match="deploy genesis block"):
        validate(evidence)


def test_deploy_network_identity_is_receipt_bound(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity.__setitem__("network_id", "other-network-id"),
    )
    with pytest.raises(checker.EvidenceError, match="deploy network identity"):
        validate(evidence)


def test_deploy_consumes_exact_publication_receipt(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity.__setitem__(
            "publication_receipt_sha256", digest("other-publication-receipt")
        ),
    )
    with pytest.raises(checker.EvidenceError,
                       match="authenticated publication"):
        validate(evidence)


def test_deploy_consumes_exact_candidate_archive(evidence: EvidenceFixture) -> None:
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity.__setitem__(
            "admission_archive_sha256", digest("other-archive")
        ),
    )
    with pytest.raises(checker.EvidenceError, match="authenticated candidate archive"):
        validate(evidence)


def test_exact_four_receipt_signers_are_required(evidence: EvidenceFixture) -> None:
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity["receipt_signers"].pop(checker.VALIDATORS[-1]),
    )
    with pytest.raises(checker.EvidenceError, match="receipt signer map"):
        validate(evidence)


def test_deploy_receipt_signer_native_verifier_is_pinned(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity["receipt_signers"][checker.VALIDATORS[0]].__setitem__(
            "native_verifier_binary_sha256", digest("attacker-native-verifier")
        ),
    )
    with pytest.raises(checker.EvidenceError, match="native verifier is not pinned"):
        validate(evidence)


def test_deploy_receipt_key_requires_compressed_sec1_prefix(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence, "deploy", "deploy_handoff",
        lambda identity: identity["receipt_signers"][checker.VALIDATORS[0]][
            "public_key"
        ].__setitem__("payload_hex", "04" + digest("uncompressed-shape")),
    )
    with pytest.raises(checker.EvidenceError, match="payload is noncanonical"):
        validate(evidence)


def test_deploy_receipt_node_must_be_derived_from_exact_key(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence,
        "deploy",
        "deploy_handoff",
        lambda identity: identity["receipt_signers"][
            checker.VALIDATORS[0]
        ].__setitem__("node_id", checker.RECEIPT_NODE_ID_PREFIX + "0" * 64),
    )
    with pytest.raises(checker.EvidenceError, match="not derived from its receipt key"):
        validate(evidence)


@pytest.mark.parametrize("field", ["config_sha256", "binary_stat_seal"])
def test_deploy_runtime_binding_cross_binds_binary_config_and_generation(
    evidence: EvidenceFixture,
    field: str,
) -> None:
    def mutate(identity: dict[str, Any]) -> None:
        signer = identity["receipt_signers"][checker.VALIDATORS[0]]
        signer[field] = digest("attacker-config") if field == "config_sha256" else [9] * 5

    replace_handoff_identity(evidence, "deploy", "deploy_handoff", mutate)
    with pytest.raises(checker.EvidenceError, match="runtime binding.*not derived"):
        validate(evidence)


def test_deploy_lifecycle_binding_cross_binds_runtime_node_and_slug(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence,
        "deploy",
        "deploy_handoff",
        lambda identity: identity["receipt_signers"][
            checker.VALIDATORS[0]
        ].__setitem__("lifecycle_binding_sha256", digest("attacker-lifecycle")),
    )
    with pytest.raises(checker.EvidenceError, match="lifecycle binding.*not derived"):
        validate(evidence)


def test_deploy_receipt_signer_rejects_slug_association_swap(
    evidence: EvidenceFixture,
) -> None:
    def swap(identity: dict[str, Any]) -> None:
        signers = identity["receipt_signers"]
        first, second = checker.VALIDATORS[:2]
        signers[first], signers[second] = signers[second], signers[first]

    replace_handoff_identity(evidence, "deploy", "deploy_handoff", swap)
    with pytest.raises(checker.EvidenceError, match="lifecycle binding.*not derived"):
        validate(evidence)


def test_deploy_receipt_signer_rejects_private_key_field(
    evidence: EvidenceFixture,
) -> None:
    replace_handoff_identity(
        evidence,
        "deploy",
        "deploy_handoff",
        lambda identity: identity["receipt_signers"][
            checker.VALIDATORS[0]
        ].__setitem__("receipt_private_key", "812620" + "01" * 32),
    )
    with pytest.raises(checker.EvidenceError, match="fields are not exact"):
        validate(evidence)


def test_submission_receipt_signer_is_bound_to_deploy_key(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "submissions", "submission_receipt_inventory", 0,
        lambda record: record.__setitem__(
            "receipt_signer_public_key", public_key("attacker")
        ),
    )
    with pytest.raises(checker.EvidenceError, match="key differs from deployment"):
        validate(evidence)


def test_block_inventory_starts_at_deploy_tip_and_proves_contiguous_ancestry(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "blocks", "block_evidence_inventory", 1,
        lambda record: record.__setitem__(
            "parent_block_hash", iroha_hash("fork", checker.BLOCK_HASH_TYPE)
        ),
    )
    with pytest.raises(checker.EvidenceError, match="contiguous deploy-descendant"):
        validate(evidence)


def test_block_finality_and_native_artifacts_are_deduplicated(
    evidence: EvidenceFixture,
) -> None:
    first = checker._decode_json(
        evidence.blocks.payload.splitlines(keepends=True)[1], "block", canonical=True
    )
    replace_inventory_record(
        evidence, "blocks", "block_evidence_inventory", 1,
        lambda record: record.__setitem__(
            "finality_proof_sha256", first["finality_proof_sha256"]
        ),
    )
    with pytest.raises(checker.EvidenceError, match="artifact is duplicated"):
        validate(evidence)


def test_block_native_verification_must_pass(evidence: EvidenceFixture) -> None:
    replace_inventory_record(
        evidence, "blocks", "block_evidence_inventory", 2,
        lambda record: record.__setitem__("verification_result", "unchecked"),
    )
    with pytest.raises(checker.EvidenceError, match="did not pass"):
        validate(evidence)


def test_block_finality_requires_exactly_three_signers(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "blocks", "block_evidence_inventory", 0,
        lambda record: record.__setitem__(
            "finality_validators", list(checker.VALIDATORS)
        ),
    )
    with pytest.raises(checker.EvidenceError, match="exactly one canonical quorum"):
        validate(evidence)


def test_block_finality_rejects_non_string_signer_without_type_error(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "blocks", "block_evidence_inventory", 0,
        lambda record: record.__setitem__(
            "finality_validators", [checker.VALIDATORS[0], 1, checker.VALIDATORS[2]]
        ),
    )
    with pytest.raises(checker.EvidenceError, match="canonical quorum"):
        validate(evidence)


def test_native_verifier_receipt_cannot_be_reused_across_evidence_types(
    evidence: EvidenceFixture,
) -> None:
    first_block = checker._decode_json(
        evidence.blocks.payload.splitlines(keepends=True)[1],
        "first block", canonical=True,
    )
    evidence.receipt["soak_anchor"]["validators"][0][
        "native_verifier_receipt_sha256"
    ] = first_block["native_verifier_receipt_sha256"]
    with pytest.raises(checker.EvidenceError, match="receipt was reused"):
        validate(evidence)


def test_anchor_accepts_descendant_tip_not_delayed_exact_deploy_tip(
    evidence: EvidenceFixture,
) -> None:
    assert evidence.receipt["soak_anchor"][
        "common_start_block_evidence_index"
    ] == 1
    assert validate(evidence).completed_at_unix_ms > 0


def test_anchor_requires_common_chain_advancement(evidence: EvidenceFixture) -> None:
    evidence.receipt["soak_anchor"]["common_start_block_evidence_index"] = 0
    with pytest.raises(checker.EvidenceError, match="integer >= 1|advancement"):
        validate(evidence)


def test_anchor_challenges_are_unique_and_bound_to_four_nodes(
    evidence: EvidenceFixture,
) -> None:
    validators = evidence.receipt["soak_anchor"]["validators"]
    validators[1]["challenge_hex"] = validators[0]["challenge_hex"]
    with pytest.raises(checker.EvidenceError, match="challenge was reused"):
        validate(evidence)


def test_anchor_node_identity_is_deploy_authenticated(evidence: EvidenceFixture) -> None:
    evidence.receipt["soak_anchor"]["validators"][0]["node_id"] = (
        "taira-node:attacker:fixture"
    )
    with pytest.raises(checker.EvidenceError, match="differs from the deploy handoff"):
        validate(evidence)


def test_anchor_must_be_fresh_at_workload_start(
    evidence: EvidenceFixture,
) -> None:
    anchor = evidence.receipt["soak_anchor"]
    anchor["observation_started_at_unix_ms"] -= 101
    anchor["observation_completed_at_unix_ms"] -= 101
    with pytest.raises(checker.EvidenceError, match="anchor is stale"):
        validate(evidence)


def test_exact_slot_schedule_rejects_one_shifted_slot(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "workload", "workload_inventory", 4,
        lambda record: record.__setitem__(
            "scheduled_elapsed_ms", int(record["scheduled_elapsed_ms"]) + 1
        ),
    )
    with pytest.raises(checker.EvidenceError, match="exact 200ms slot schedule"):
        validate(evidence)


def test_bounded_inflight_requests_may_overlap_later_slots(
    evidence: EvidenceFixture,
) -> None:
    lines = evidence.workload.payload.splitlines(keepends=True)
    first = checker._decode_json(lines[1], "first workload", canonical=True)
    second = checker._decode_json(lines[2], "second workload", canonical=True)
    assert first["request_completed_elapsed_ms"] > second["request_started_elapsed_ms"]
    validate(evidence)


def test_applied_confirmations_may_use_the_bounded_post_window_drain(
    evidence: EvidenceFixture,
) -> None:
    last = checker._decode_json(
        evidence.statuses.payload.splitlines(keepends=True)[-1],
        "last Applied status", canonical=True,
    )
    assert last["observed_at_unix_ms"] > evidence.receipt["completion"][
        "workload_ended_at_unix_ms"
    ]
    validate(evidence)


def test_submission_start_lateness_is_bounded(evidence: EvidenceFixture) -> None:
    replace_inventory_record(
        evidence, "workload", "workload_inventory", 3,
        lambda record: record.__setitem__(
            "request_started_elapsed_ms",
            int(record["scheduled_elapsed_ms"]) + 251,
        ),
    )
    with pytest.raises(checker.EvidenceError, match="bounded scheduled start"):
        validate(evidence)


def test_request_window_is_independently_bounded(evidence: EvidenceFixture) -> None:
    replace_inventory_record(
        evidence, "workload", "workload_inventory", 0,
        lambda record: record.__setitem__(
            "request_completed_elapsed_ms",
            int(record["request_started_elapsed_ms"]) + 501,
        ),
    )
    with pytest.raises(checker.EvidenceError, match="request window"):
        validate(evidence)


def test_versioned_bytes_digest_is_separate_from_marked_hashes(
    evidence: EvidenceFixture,
) -> None:
    record = checker._decode_json(
        evidence.workload.payload.splitlines(keepends=True)[1],
        "workload", canonical=True,
    )
    assert isinstance(record["signed_transaction_hash"], dict)
    assert isinstance(record["entrypoint_hash"], dict)
    assert isinstance(record["versioned_signed_transaction_sha256"], str)
    assert record["signed_transaction_hash"]["type"] != record["entrypoint_hash"]["type"]
    assert record["signed_transaction_hash"]["value"] != (
        record["versioned_signed_transaction_sha256"]
    )


def test_workload_signed_and_entrypoint_hash_types_cannot_be_swapped(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "workload", "workload_inventory", 0,
        lambda record: record["signed_transaction_hash"].__setitem__(
            "type", checker.ENTRYPOINT_HASH_TYPE
        ),
    )
    with pytest.raises(checker.EvidenceError, match="HashOf type is wrong"):
        validate(evidence)


def test_workload_submission_status_and_block_indexes_are_cross_linked(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "workload", "workload_inventory", 0,
        lambda record: record.__setitem__("block_evidence_index", 3),
    )
    with pytest.raises(checker.EvidenceError, match="does not bind"):
        validate(evidence)


def test_workload_cannot_reuse_a_submission_index(evidence: EvidenceFixture) -> None:
    replace_inventory_record(
        evidence, "workload", "workload_inventory", 1,
        lambda record: record.__setitem__("submission_receipt_index", 0),
    )
    with pytest.raises(checker.EvidenceError, match="index is reused"):
        validate(evidence)


def test_status_inventory_requires_global_applied(evidence: EvidenceFixture) -> None:
    replace_inventory_record(
        evidence, "statuses", "applied_status_inventory", 0,
        lambda record: record.__setitem__("result", "Pending"),
    )
    with pytest.raises(checker.EvidenceError, match="not Applied"):
        validate(evidence)


def test_status_inventory_requires_native_verified_result(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "statuses", "applied_status_inventory", 0,
        lambda record: record.__setitem__("verification_result", "unchecked"),
    )
    with pytest.raises(checker.EvidenceError, match="status native verification"):
        validate(evidence)


def test_status_block_cannot_exceed_observation_common_tip(
    evidence: EvidenceFixture,
) -> None:
    replace_inventory_record(
        evidence, "statuses", "applied_status_inventory", 0,
        lambda record: record.__setitem__("block_evidence_index", 3),
    )
    with pytest.raises(checker.EvidenceError, match="newer than its observation"):
        validate(evidence)


@pytest.mark.parametrize("hostile_height", (10, 999_999))
def test_submission_height_is_bounded_by_deploy_and_inclusion(
    evidence: EvidenceFixture, hostile_height: int,
) -> None:
    replace_inventory_record(
        evidence, "submissions", "submission_receipt_inventory", 0,
        lambda record: record.__setitem__("submitted_at_height", hostile_height),
        refresh_records_digest=True,
    )
    with pytest.raises(checker.EvidenceError, match="deploy-to-inclusion"):
        validate(evidence)


def test_submission_time_cannot_predate_deployment(
    evidence: EvidenceFixture,
) -> None:
    started_ms = evidence.receipt["completion"]["workload_started_at_unix_ms"]
    replace_inventory_record(
        evidence, "submissions", "submission_receipt_inventory", 0,
        lambda record: record.__setitem__(
            "submitted_at_unix_ms", int(started_ms) - 400
        ),
    )
    with pytest.raises(checker.EvidenceError, match="predates deployment"):
        validate(evidence)


def test_sample_counter_is_derived_from_exact_applied_statuses(
    evidence: EvidenceFixture,
) -> None:
    evidence.receipt["samples"][0]["applied_transfer_count"] = 4
    with pytest.raises(checker.EvidenceError, match="not derived"):
        validate(evidence)


def test_observation_window_has_separate_start_and_end_boundaries(
    evidence: EvidenceFixture,
) -> None:
    sample = evidence.receipt["samples"][0]
    sample["observation_completed_at_unix_ms"] = (
        sample["observation_started_at_unix_ms"] + 101
    )
    with pytest.raises(checker.EvidenceError, match="observation window exceeds"):
        validate(evidence)


def test_actual_sample_gap_is_derived_and_bounded(
    evidence: EvidenceFixture, monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert evidence.receipt["completion"][
        "maximum_observed_sample_gap_ms"
    ] == 1_020
    monkeypatch.setattr(checker, "MAXIMUM_SAMPLE_GAP_MS", 1_010)
    evidence.receipt["profile"]["maximum_sample_gap_ms"] = 1_010
    with pytest.raises(checker.EvidenceError, match="actual sample observation gap"):
        validate(evidence)


def test_completion_cannot_claim_a_smaller_sample_gap(
    evidence: EvidenceFixture,
) -> None:
    evidence.receipt["completion"]["maximum_observed_sample_gap_ms"] = 1_000
    with pytest.raises(checker.EvidenceError, match="atomic completion"):
        validate(evidence)


@pytest.mark.parametrize(
    ("container", "field", "hostile"),
    (
        ("profile", "target_tps", 5.0),
        ("profile", "duration_ms", 2_000.0),
        ("completion", "natural_completion", 1),
        ("completion", "failed_transfer_count", False),
    ),
)
def test_numeric_and_boolean_fields_require_exact_json_types(
    evidence: EvidenceFixture, container: str, field: str, hostile: object,
) -> None:
    evidence.receipt[container][field] = hostile
    with pytest.raises(checker.EvidenceError,
                       match="fixed public soak profile|atomic completion"):
        validate(evidence)


def test_receipt_object_match_is_type_exact(evidence: EvidenceFixture) -> None:
    hostile = copy.deepcopy(evidence.receipt)
    hostile["completion"]["natural_completion"] = 1
    with pytest.raises(checker.EvidenceError, match="exact receipt artifact bytes"):
        checker._validate_structural_evidence(
            hostile, **structural_arguments(evidence)
        )


def test_post_window_confirmation_drain_is_bounded(
    evidence: EvidenceFixture,
) -> None:
    completion = evidence.receipt["completion"]
    completion["evidence_completed_at_unix_ms"] = (
        completion["workload_ended_at_unix_ms"] + 1_001
    )
    with pytest.raises(checker.EvidenceError, match="drain exceeds"):
        validate(evidence)


@pytest.mark.parametrize(
    ("field", "hostile"),
    (
        ("restart_generation", digest("other-restart-generation")),
        ("config_set_sha256", digest("other-config-set")),
        ("topology_sha256", digest("other-topology")),
        ("signed_genesis_sha256", digest("other-signed-genesis")),
        ("supervisor_sha256", digest("other-supervisor")),
        ("genesis_block_hash",
         iroha_hash("other-lifecycle-genesis", checker.BLOCK_HASH_TYPE)),
    ),
)
def test_lifecycle_is_bound_to_deploy_generation_and_configuration(
    evidence: EvidenceFixture, field: str, hostile: object,
) -> None:
    replace_lifecycle(
        evidence, lambda value: value.__setitem__(field, hostile)
    )
    with pytest.raises(checker.EvidenceError, match="differs from deployment"):
        validate(evidence)


def test_lifecycle_is_bound_to_deployment_completion_generation(
    evidence: EvidenceFixture,
) -> None:
    replace_lifecycle(
        evidence,
        lambda value: value.__setitem__(
            "deployment_completed_at_unix_ms",
            int(value["deployment_completed_at_unix_ms"]) - 1,
        ),
    )
    with pytest.raises(checker.EvidenceError, match="generation is spliced"):
        validate(evidence)


def test_lifecycle_raw_windows_cross_bind_deploy_lifecycle_identity(
    evidence: EvidenceFixture,
) -> None:
    replace_lifecycle(
        evidence,
        lambda value: value["raw_windows"][0].__setitem__(
            "binding_sha256", digest("attacker-raw-binding")
        ),
    )
    with pytest.raises(checker.EvidenceError, match="binding differs from deployment"):
        validate(evidence)


def test_lifecycle_raw_windows_reject_omission_and_reorder(
    evidence: EvidenceFixture,
) -> None:
    replace_lifecycle(
        evidence,
        lambda value: value["raw_windows"].__setitem__(
            slice(0, 2), list(reversed(value["raw_windows"][:2]))
        ),
    )
    with pytest.raises(checker.EvidenceError, match="canonical validator order"):
        validate(evidence)


def test_lifecycle_raw_windows_require_exact_four_validators(
    evidence: EvidenceFixture,
) -> None:
    replace_lifecycle(evidence, lambda value: value["raw_windows"].pop())
    with pytest.raises(checker.EvidenceError, match="exactly four validators"):
        validate(evidence)


def test_lifecycle_zero_drift_is_derived_from_canonical_journal(
    evidence: EvidenceFixture,
) -> None:
    replace_lifecycle_journal_record(
        evidence, 4, lambda row: row.__setitem__("event", "restart")
    )
    with pytest.raises(checker.EvidenceError, match="derives a restart"):
        validate(evidence)


def test_lifecycle_journal_native_verifier_is_pinned(
    evidence: EvidenceFixture,
) -> None:
    receipt = checker._decode_json(
        evidence.lifecycle_native_receipt.payload,
        "lifecycle native receipt", canonical=True,
    )
    receipt["verifier_binary_sha256"] = digest("attacker-journal-verifier")
    changed_receipt = artifact(
        "mutated-lifecycle-native-receipt.json",
        checker._canonical_json(receipt),
    )
    evidence.lifecycle_native_receipt = changed_receipt
    lifecycle = checker._decode_json(
        evidence.lifecycle.payload, "lifecycle", canonical=True
    )
    lifecycle["native_journal_verifier_receipt"] = {
        "sha256": changed_receipt.sha256,
        "size_bytes": changed_receipt.size,
    }
    changed_lifecycle = artifact(
        "mutated-lifecycle.json", checker._canonical_json(lifecycle)
    )
    evidence.lifecycle = changed_lifecycle
    evidence.receipt["lifecycle"]["sha256"] = changed_lifecycle.sha256
    evidence.receipt["lifecycle"]["size_bytes"] = changed_lifecycle.size
    with pytest.raises(checker.EvidenceError,
                       match="lifecycle native verifier is not pinned"):
        validate(evidence)


def test_lifecycle_terminal_rejects_one_unexpected_exit(
    evidence: EvidenceFixture,
) -> None:
    def mutate(value: dict[str, Any]) -> None:
        value["unexpected_exit_events"] = 1
        value["terminal"]["validators"][0]["unexpected_exit_total"] += 1

    replace_lifecycle(evidence, mutate)
    with pytest.raises(checker.EvidenceError,
                       match="unexpected.exit|unexpected-exit"):
        validate(evidence)


def test_lifecycle_generation_cannot_change_without_an_exit(
    evidence: EvidenceFixture,
) -> None:
    replace_lifecycle(
        evidence,
        lambda value: value["terminal"]["validators"][0].__setitem__(
            "process_generation", 10
        ),
    )
    with pytest.raises(checker.EvidenceError, match="generations changed"):
        validate(evidence)


def test_lifecycle_baseline_and_terminal_journal_identities_must_advance(
    evidence: EvidenceFixture,
) -> None:
    replace_lifecycle(
        evidence,
        lambda value: value["terminal"].update(
            journal_sequence=value["baseline"]["journal_sequence"],
            journal_chain_sha256=value["baseline"]["journal_chain_sha256"],
        ),
    )
    with pytest.raises(checker.EvidenceError, match="journal did not advance"):
        validate(evidence)


def test_native_verifier_identity_is_pinned_by_invocation(
    evidence: EvidenceFixture,
) -> None:
    evidence.receipt["native_verifier"]["binary_sha256"] = digest(
        "attacker-native-verifier"
    )
    with pytest.raises(checker.EvidenceError, match="trusted invocation"):
        validate(evidence)


def test_durable_admission_binds_exact_receipt_bytes(
    evidence: EvidenceFixture,
) -> None:
    evidence.receipt["network"]["genesis_block_hash"] = iroha_hash(
        "other-genesis", checker.BLOCK_HASH_TYPE
    )
    with pytest.raises(checker.EvidenceError,
                       match="exact evidence set|deploy genesis block"):
        validate(evidence)


def test_cli_cannot_succeed_by_mocking_only_the_provisioning_probe(
    evidence: EvidenceFixture, tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str],
) -> None:
    arguments = write_fixture(tmp_path, evidence)
    monkeypatch.setattr(checker, "_require_observation_authority", lambda: None)
    assert checker.main(arguments) == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert checker.AUTHORITY_SCHEMA in captured.err
