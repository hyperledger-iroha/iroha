#!/usr/bin/env python3
"""Canonical Exact12 four-peer qualification evidence contract.

The macOS build job may produce candidate binaries, but it cannot produce this
evidence.  A secret-free qualification controller executes the frozen native
test drivers and preserves their bounded output in the signed candidate.  The
unsigned v2 format below is retained only for structural diagnostics.  Its
self-hashes do not prove that the installed controller produced the libtest
bytes, so it cannot authorize candidate signing or admission.

Release use requires the separately pinned controller-origin authority to
authenticate a canonical envelope under the provisioning contract below.  A
receipt row, self-reported digest, caller marker, or candidate signing identity
is never sufficient by itself.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import os
import re
from collections.abc import Mapping
from pathlib import Path
from typing import NoReturn

try:
    from . import taira_authority_client
    from .release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_write_bytes,
        load_json_object,
        scan_inventory_paths,
        stable_hash_relative,
        stable_read_relative,
    )
except ImportError:
    import taira_authority_client
    from release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_write_bytes,
        load_json_object,
        scan_inventory_paths,
        stable_hash_relative,
        stable_read_relative,
    )


RECEIPT_SCHEMA = "iroha.taira.privacy_protocol_four_peer_receipt"
RECEIPT_SCHEMA_VERSION = 2
TRANSCRIPT_SCHEMA = "iroha.taira.privacy_protocol_case_transcript"
TRANSCRIPT_SCHEMA_VERSION = 1
RESULT_SCHEMA = "iroha.taira.privacy_protocol_case_result"
RESULT_SCHEMA_VERSION = 1

RECEIPT_NAME = "privacy-protocol-four-peer-receipt-v2.json"
MAX_RECEIPT_BYTES = 1024 * 1024
MAX_AUTHORITY_SIDECAR_BYTES = 4 * 1024 * 1024
MAX_RESULT_BYTES = 128 * 1024
MAX_COMMAND_OUTPUT_BYTES = 2 * 1024 * 1024
MAX_TRANSCRIPT_BYTES = 6 * 1024 * 1024
MAX_DRIVER_BYTES = 512 * 1024 * 1024
MAX_EVIDENCE_TOTAL_BYTES = 2 * 1024 * 1024 * 1024
MAX_RECEIPT_LIFETIME_SECONDS = 24 * 60 * 60
MAX_FUTURE_CLOCK_SKEW_SECONDS = 5 * 60
PEER_COUNT = 4

CONTROLLER_ORIGIN_AUTHORITY_CONTRACT = (
    "iroha.taira.privacy-protocol-controller-origin-authority.v1"
)
CONTROLLER_ORIGIN_AUTHENTICATED_RUN_CONTRACT = (
    "iroha.taira.privacy-protocol-authenticated-run-nonce.v1"
)
CONTROLLER_ORIGIN_REPLAY_NAMESPACE = (
    "iroha.taira.privacy-protocol-controller-origin-replay.v1"
)
AUTHORITY_ENVELOPE_SUFFIX = (
    ".privacy-protocol-origin-authority-envelope-v1.json"
)
DURABLE_RECEIPT_SUFFIX = ".privacy-protocol-origin-durable-receipt-v1.json"
CONTROLLER_ORIGIN_AUTHORITY_PROVISIONING_BARRIER = (
    "missing preprovisioned "
    "iroha.taira.privacy-protocol-controller-origin-authority.v1 and "
    "iroha.taira.privacy-protocol-authenticated-run-nonce.v1: unsigned legacy "
    "v2 receipts are structural data, not release evidence; an authority-only "
    "signer and separately pinned trust root, both inaccessible to the capture "
    "runtime and candidate signer, must authenticate one canonical "
    "controller-origin envelope binding the exact libtest output bytes and "
    "digests, preserved driver/transcript/result bytes and digests, the closed "
    "case/operation/outcome table, candidate and complete source identity "
    "including source commit, DPN validator release commit, canonical "
    "Cargo.lock digest, and workspace source-manifest digest, the four-peer "
    "validator/supervisor and authority host/installation identities, the "
    "installed controller closure digest, and authority-issued run nonce, "
    "issue time, expiry, and replay namespace "
    "iroha.taira.privacy-protocol-controller-origin-replay.v1; no self-hash, "
    "caller value, marker, or candidate-signing credential may mint or bypass "
    "that provenance"
)

SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
PASS_SUMMARY_RE = re.compile(
    r"(?m)^test result: ok\. ([1-9][0-9]*) passed; 0 failed; 0 ignored;"
)
RUNNING_RE = re.compile(r"(?m)^running ([1-9][0-9]*) tests?$", re.IGNORECASE)

NETWORK_DRIVER = "network-functional"
JINDO_DRIVER = "iroha-core-tests"
ACTION_DRIVER = "privacy-exact12-action-driver"
DRIVER_EVIDENCE_NAMES = {
    ACTION_DRIVER: "privacy-action-driver-v1.bin",
    JINDO_DRIVER: "privacy-jindo-test-driver-v1.bin",
    NETWORK_DRIVER: "privacy-network-test-driver-v1.bin",
}
JINDO_SECURITY_FILTER = "privacy_engines::jindo::security::tests"

# These are the six protocol qualification cases plus the independent whole
# governance case.  The positive ZK-AMS and ZK-X509 names deliberately do not
# alias their former unavailable-profile tests: v2 remains closed until real
# release-shape positive cases exist and pass.
CASE_DEFINITIONS = (
    (
        "privacy_exact12_retained_network::canonical_retained_exact12_actions_survive_four_peer_adversarial_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_activation_network::canonical_zk_ams_action_survives_four_peer_activation_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_zk_ams_vega_network::canonical_zk_ams_and_vega_actions_survive_four_validator_activation_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_zk_x509_network::canonical_zk_x509_action_survives_four_peer_activation_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_jindo_network::canonical_jindo_direct_action_survives_four_peer_activation_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_orchard_pq_masp_network::canonical_orchard_and_pq_masp_actions_survive_four_peer_da_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_activation_network::canonical_exact12_governance_survives_four_peer_activation_replay_and_restart",
        "governance",
    ),
)

# Ordered exactly as PrivacyProtocolIdV1::ALL / exact12_v1.tsv.  Every retained
# v2 row is usable.  Revised Jindo stays explicitly experimental and exposes
# the one missing certificate instead of claiming it.
OUTCOMES = (
    (
        "zk-ace-pq-authorization-v0",
        0,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "anonymous-pgc-k-out-of-n-v1",
        0,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "verange-transparent-range-v1",
        0,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "iroha-zk-ams-v1",
        1,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "vega-existing-credential-zk-v0",
        2,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "iroha-zk-x509-stark-p256-v0",
        3,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "iroha-jindo-polynomial-commitment-v0",
        4,
        "available-experimental",
        "accepted-and-queried-across-four-peers",
        "none",
        "JindoSecurityCertificateErrorV1::MissingDistributionWideKnowledgeSoundnessEvidence",
    ),
    (
        "iroha-bootle-lantern-anoncred-v1",
        0,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "orchard-halo2-actions-v1",
        5,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "monero-fcmp-plus-plus-v1",
        0,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "iroha-ivm-private-note-stark-v1",
        0,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
    (
        "pq-masp-stark-v0",
        5,
        "available",
        "accepted-and-queried-across-four-peers",
        "none",
        "none",
    ),
)


class PrivacyProtocolEvidenceError(RuntimeError):
    """Exact12 qualification evidence is absent, mutable, or noncanonical."""


class AuthenticatedPrivacyProtocolEvidence(dict[str, object]):
    """Structurally compatible evidence with explicit authenticated sidecars."""

    __slots__ = (
        "_authority_envelope",
        "_durable_receipt",
        "_evidence_root",
        "_operation_id",
        "_run_id",
    )

    def __init__(
        self,
        structural_subject: Mapping[str, object],
        *,
        evidence_root: Path,
        authority_result: taira_authority_client.AuthorityResult,
    ) -> None:
        super().__init__(structural_subject)
        self._authority_envelope = authority_result.authority_envelope_bytes
        self._durable_receipt = authority_result.durable_receipt_bytes
        self._evidence_root = Path(os.path.abspath(evidence_root))
        self._operation_id = authority_result.operation_id
        self._run_id = authority_result.run_id

    @property
    def authority_envelope(self) -> bytes:
        """Return the canonical authority-envelope sidecar bytes."""

        return self._authority_envelope

    @property
    def durable_receipt(self) -> bytes:
        """Return the canonical durable-receipt sidecar bytes."""

        return self._durable_receipt

    @property
    def operation_id(self) -> str:
        """Return the native authority operation identifier."""

        return self._operation_id

    @property
    def run_id(self) -> str:
        """Return the administrator-assigned run identifier."""

        return self._run_id

    def sidecar_paths(self) -> tuple[Path, Path]:
        """Return deterministic sibling paths outside the evidence inventory."""

        return authority_sidecar_paths(self._evidence_root)

    def persist_sidecars(self) -> tuple[Path, Path]:
        """Exclusively persist both authenticated sidecars beside the evidence."""

        return persist_authenticated_sidecars(self)


def authority_sidecar_paths(evidence_root: Path) -> tuple[Path, Path]:
    """Return the fixed sibling sidecar paths for one evidence directory."""

    absolute = Path(os.path.abspath(evidence_root))
    return (
        absolute.with_name(absolute.name + AUTHORITY_ENVELOPE_SUFFIX),
        absolute.with_name(absolute.name + DURABLE_RECEIPT_SUFFIX),
    )


def persist_authenticated_sidecars(
    evidence: AuthenticatedPrivacyProtocolEvidence,
) -> tuple[Path, Path]:
    """Exclusively write an authenticated result's two canonical sidecars."""

    if not isinstance(evidence, AuthenticatedPrivacyProtocolEvidence):
        _fail("privacy protocol sidecars require an authenticated evidence result")
    envelope_path, receipt_path = evidence.sidecar_paths()
    try:
        exclusive_write_bytes(envelope_path, evidence.authority_envelope, mode=0o600)
        exclusive_write_bytes(receipt_path, evidence.durable_receipt, mode=0o600)
    except ReleaseArtifactError as error:
        raise PrivacyProtocolEvidenceError(
            f"cannot persist privacy protocol authority sidecars: {error}"
        ) from error
    return envelope_path, receipt_path


def _fail(message: str) -> NoReturn:
    raise PrivacyProtocolEvidenceError(message)


def require_controller_origin_authority_provisioned(
    *, require_signing: bool = True
) -> None:
    """Authenticate the fixed privacy-protocol-origin authority service."""

    try:
        taira_authority_client.preflight(
            "privacy-protocol-origin", require_signing=require_signing
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PrivacyProtocolEvidenceError(
            f"{CONTROLLER_ORIGIN_AUTHORITY_PROVISIONING_BARRIER}: {error}"
        ) from error


def transcript_name(index: int) -> str:
    return f"privacy-protocol-case-{index:02d}-transcript-v1.json"


def result_name(index: int) -> str:
    return f"privacy-protocol-case-{index:02d}-result-v1.json"


EVIDENCE_NAMES = (
    RECEIPT_NAME,
    *(DRIVER_EVIDENCE_NAMES[name] for name in sorted(DRIVER_EVIDENCE_NAMES)),
    *(
        name
        for index in range(len(CASE_DEFINITIONS))
        for name in (result_name(index), transcript_name(index))
    ),
)


def evidence_file_maximum(name: str) -> int:
    """Return the closed per-file bound for one v2 evidence member."""

    if name in DRIVER_EVIDENCE_NAMES.values():
        return MAX_DRIVER_BYTES
    if name == RECEIPT_NAME:
        return MAX_RECEIPT_BYTES
    if name.endswith("-transcript-v1.json"):
        return MAX_TRANSCRIPT_BYTES
    if name.endswith("-result-v1.json"):
        return MAX_RESULT_BYTES
    _fail("privacy protocol evidence member is not part of the v2 contract")


def command_plan(case_index: int) -> tuple[tuple[str, tuple[str, ...]], ...]:
    try:
        case, _kind = CASE_DEFINITIONS[case_index]
    except IndexError as exc:
        raise PrivacyProtocolEvidenceError("privacy case index is out of range") from exc
    network = (
        NETWORK_DRIVER,
        (case, "--exact", "--nocapture", "--test-threads=1"),
    )
    if case_index != 4:
        return (network,)
    return (
        network,
        (
            JINDO_DRIVER,
            (JINDO_SECURITY_FILTER, "--nocapture", "--test-threads=1"),
        ),
    )


def _exact_fields(value: Mapping[str, object], expected: set[str], label: str) -> None:
    if set(value) != expected:
        missing = sorted(expected - set(value))
        extra = sorted(set(value) - expected)
        _fail(f"{label} fields are not exact: missing={missing}, extra={extra}")


def _integer(
    value: object,
    label: str,
    *,
    minimum: int = 0,
    maximum: int | None = None,
) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer greater than or equal to {minimum}")
    if maximum is not None and value > maximum:
        _fail(f"{label} exceeds its {maximum}-byte bound")
    return value


def _sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    return value


def _canonical_object(payload: bytes, label: str, maximum: int) -> dict[str, object]:
    if not payload or len(payload) > maximum:
        _fail(f"{label} is empty or exceeds its {maximum}-byte bound")
    try:
        value = load_json_object(payload, label)
        rendered = canonical_json_bytes(value)
    except (ReleaseArtifactError, TypeError, ValueError) as exc:
        raise PrivacyProtocolEvidenceError(f"{label} is invalid: {exc}") from exc
    if rendered != payload:
        _fail(f"{label} is not canonical deterministic JSON")
    return value


def _source_identity(value: object, label: str) -> dict[str, str]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    _exact_fields(
        value,
        {
            "cargo_lock_sha256",
            "commit",
            "dpn_validator_release_commit",
            "workspace_source_manifest_sha256",
        },
        label,
    )
    commit = value["commit"]
    dpn_commit = value["dpn_validator_release_commit"]
    if not isinstance(commit, str) or COMMIT_RE.fullmatch(commit) is None:
        _fail(f"{label} commit is not one full lowercase commit")
    if not isinstance(dpn_commit, str) or COMMIT_RE.fullmatch(dpn_commit) is None:
        _fail(f"{label} DPN commit is not one full lowercase commit")
    return {
        "cargo_lock_sha256": _sha256(
            value["cargo_lock_sha256"], f"{label} Cargo.lock digest"
        ),
        "commit": commit,
        "dpn_validator_release_commit": dpn_commit,
        "workspace_source_manifest_sha256": _sha256(
            value["workspace_source_manifest_sha256"],
            f"{label} workspace source digest",
        ),
    }


def _domain_id(domain: bytes, value: Mapping[str, object], forbidden: str) -> str:
    if forbidden in value:
        _fail(f"{forbidden} input must omit {forbidden}")
    digest = hashlib.sha256()
    digest.update(domain)
    digest.update(canonical_json_bytes(value))
    return digest.hexdigest()


def compute_candidate_binding_sha256(candidate: Mapping[str, object]) -> str:
    return _domain_id(
        b"iroha.taira.privacy_protocol_candidate.v2\0",
        candidate,
        "candidate_binding_sha256",
    )


def compute_transcript_id(transcript_without_id: Mapping[str, object]) -> str:
    return _domain_id(
        b"iroha.taira.privacy_protocol_case_transcript.v1\0",
        transcript_without_id,
        "transcript_id",
    )


def compute_result_id(result_without_id: Mapping[str, object]) -> str:
    return _domain_id(
        b"iroha.taira.privacy_protocol_case_result.v1\0",
        result_without_id,
        "result_id",
    )


def compute_receipt_id(receipt_without_id: Mapping[str, object]) -> str:
    return _domain_id(
        b"iroha.taira.privacy_protocol_four_peer_receipt.v2\0",
        receipt_without_id,
        "receipt_id",
    )


def _read_evidence(
    root: Path, relative: str, maximum: int, label: str
) -> tuple[bytes, str, int]:
    try:
        info, payload = stable_read_relative(
            root, relative, max_size=maximum, return_payload=True
        )
    except ReleaseArtifactError as exc:
        raise PrivacyProtocolEvidenceError(f"cannot read {label}: {exc}") from exc
    assert payload is not None
    return payload, info.sha256, info.size


def _validated_candidate(
    value: object,
    *,
    expected_source: Mapping[str, object],
    expected_validator_binary_sha256: str,
    expected_linux_release_archive_sha256: str,
    expected_exact12_matrix_sha256: str,
    expected_artifact_handoff_sha256: str,
) -> tuple[dict[str, object], dict[str, str]]:
    if not isinstance(value, dict):
        _fail("privacy protocol candidate binding must be an object")
    _exact_fields(
        value,
        {
            "artifact_handoff_sha256",
            "drivers",
            "exact12_matrix_sha256",
            "linux_release_archive_sha256",
            "source",
            "validator_binary_sha256",
        },
        "privacy protocol candidate binding",
    )
    source = _source_identity(value["source"], "privacy protocol candidate source")
    if source != _source_identity(expected_source, "expected source identity"):
        _fail("privacy protocol evidence is bound to a different source identity")
    drivers = value["drivers"]
    if not isinstance(drivers, dict):
        _fail("privacy protocol candidate drivers must be an object")
    _exact_fields(
        drivers,
        {ACTION_DRIVER, JINDO_DRIVER, NETWORK_DRIVER},
        "privacy protocol candidate drivers",
    )
    normalized_drivers = {
        name: _sha256(drivers[name], f"privacy protocol {name} driver digest")
        for name in sorted(drivers)
    }
    expected = {
        "artifact_handoff_sha256": _sha256(
            expected_artifact_handoff_sha256, "expected artifact handoff digest"
        ),
        "exact12_matrix_sha256": _sha256(
            expected_exact12_matrix_sha256, "expected Exact12 matrix digest"
        ),
        "linux_release_archive_sha256": _sha256(
            expected_linux_release_archive_sha256,
            "expected Linux release archive digest",
        ),
        "validator_binary_sha256": _sha256(
            expected_validator_binary_sha256, "expected validator binary digest"
        ),
    }
    for field, digest in expected.items():
        if _sha256(value[field], f"privacy protocol candidate {field}") != digest:
            _fail(f"privacy protocol evidence has a stale {field} binding")
    return (
        {
            "artifact_handoff_sha256": expected["artifact_handoff_sha256"],
            "drivers": normalized_drivers,
            "exact12_matrix_sha256": expected["exact12_matrix_sha256"],
            "linux_release_archive_sha256": expected[
                "linux_release_archive_sha256"
            ],
            "source": source,
            "validator_binary_sha256": expected["validator_binary_sha256"],
        },
        normalized_drivers,
    )


def _validate_test_output(output: bytes, *, case: str, driver: str) -> None:
    try:
        text = output.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise PrivacyProtocolEvidenceError(
            f"privacy case {case!r} output is not UTF-8"
        ) from exc
    lowered = text.lower()
    for forbidden in (
        "engine-unavailable",
        "engine_unavailable",
        "fixture-only",
        "fixture_only",
        "running 0 tests",
        "test result: failed",
    ):
        if forbidden in lowered:
            _fail(f"privacy case {case!r} output contains {forbidden!r}")
    summaries = [int(value) for value in PASS_SUMMARY_RE.findall(text)]
    running = [int(value) for value in RUNNING_RE.findall(text)]
    if len(summaries) != 1 or len(running) != 1 or summaries[0] != running[0]:
        _fail(f"privacy case {case!r} lacks one internally consistent test summary")
    if driver == NETWORK_DRIVER:
        if summaries[0] != 1:
            _fail(f"privacy case {case!r} did not execute exactly one named test")
        passed_line = re.compile(rf"(?m)^test {re.escape(case)} \.\.\. ok\r?$")
        if passed_line.search(text) is None:
            _fail(f"privacy case {case!r} lacks its exact libtest pass record")
    else:
        passed_lines = re.findall(r"(?m)^test ([^\r\n]+) \.\.\. ok\r?$", text)
        if len(passed_lines) != summaries[0] or not all(
            "jindo" in value.lower() for value in passed_lines
        ):
            _fail("Jindo security output lacks exact positive native test records")


def _validate_transcript(
    payload: bytes,
    *,
    index: int,
    candidate_binding_sha256: str,
    driver_digests: Mapping[str, str],
) -> tuple[dict[str, object], str]:
    case, kind = CASE_DEFINITIONS[index]
    transcript = _canonical_object(
        payload, f"privacy protocol transcript {index}", MAX_TRANSCRIPT_BYTES
    )
    _exact_fields(
        transcript,
        {
            "candidate_binding_sha256",
            "case",
            "commands",
            "index",
            "kind",
            "schema",
            "schema_version",
            "transcript_id",
        },
        f"privacy protocol transcript {index}",
    )
    if (
        transcript["schema"] != TRANSCRIPT_SCHEMA
        or transcript["schema_version"] != TRANSCRIPT_SCHEMA_VERSION
        or transcript["index"] != index
        or transcript["case"] != case
        or transcript["kind"] != kind
        or transcript["candidate_binding_sha256"] != candidate_binding_sha256
    ):
        _fail(f"privacy protocol transcript {index} identity differs")
    transcript_id = _sha256(
        transcript["transcript_id"], f"privacy protocol transcript {index} ID"
    )
    body = dict(transcript)
    del body["transcript_id"]
    if compute_transcript_id(body) != transcript_id:
        _fail(f"privacy protocol transcript {index} ID is fabricated")
    commands = transcript["commands"]
    plan = command_plan(index)
    if not isinstance(commands, list) or len(commands) != len(plan):
        _fail(f"privacy protocol transcript {index} command count differs")
    for command_index, (command, expected_command) in enumerate(zip(commands, plan)):
        if not isinstance(command, dict):
            _fail(f"privacy protocol transcript {index} command is not an object")
        _exact_fields(
            command,
            {
                "args",
                "driver",
                "driver_sha256",
                "exit_code",
                "index",
                "output_base64",
                "output_sha256",
                "output_size",
            },
            f"privacy protocol transcript {index} command {command_index}",
        )
        expected_driver, expected_args = expected_command
        if (
            command["index"] != command_index
            or command["driver"] != expected_driver
            or command["args"] != list(expected_args)
            or command["exit_code"] != 0
        ):
            _fail(
                f"privacy protocol transcript {index} command ordering or result differs"
            )
        if _sha256(
            command["driver_sha256"],
            f"privacy protocol transcript {index} driver digest",
        ) != driver_digests[expected_driver]:
            _fail(f"privacy protocol transcript {index} used a different driver")
        output_size = _integer(
            command["output_size"],
            f"privacy protocol transcript {index} output size",
            minimum=1,
            maximum=MAX_COMMAND_OUTPUT_BYTES,
        )
        encoded = command["output_base64"]
        if not isinstance(encoded, str) or not encoded.isascii():
            _fail(f"privacy protocol transcript {index} output is not base64 text")
        try:
            output = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error) as exc:
            raise PrivacyProtocolEvidenceError(
                f"privacy protocol transcript {index} output is malformed base64"
            ) from exc
        if base64.b64encode(output).decode("ascii") != encoded:
            _fail(f"privacy protocol transcript {index} output base64 is noncanonical")
        if len(output) != output_size:
            _fail(f"privacy protocol transcript {index} output is truncated")
        if hashlib.sha256(output).hexdigest() != _sha256(
            command["output_sha256"],
            f"privacy protocol transcript {index} output digest",
        ):
            _fail(f"privacy protocol transcript {index} output digest differs")
        _validate_test_output(output, case=case, driver=expected_driver)
    return transcript, transcript_id


def _validate_result(
    payload: bytes,
    *,
    index: int,
    candidate_binding_sha256: str,
    transcript_id: str,
    transcript_sha256: str,
    transcript_size: int,
) -> tuple[dict[str, object], str]:
    case, kind = CASE_DEFINITIONS[index]
    result = _canonical_object(
        payload, f"privacy protocol result {index}", MAX_RESULT_BYTES
    )
    _exact_fields(
        result,
        {
            "candidate_binding_sha256",
            "case",
            "index",
            "kind",
            "result_id",
            "schema",
            "schema_version",
            "status",
            "transcript_id",
            "transcript_path",
            "transcript_sha256",
            "transcript_size",
        },
        f"privacy protocol result {index}",
    )
    expected_values = {
        "candidate_binding_sha256": candidate_binding_sha256,
        "case": case,
        "index": index,
        "kind": kind,
        "schema": RESULT_SCHEMA,
        "schema_version": RESULT_SCHEMA_VERSION,
        "status": "passed",
        "transcript_id": transcript_id,
        "transcript_path": transcript_name(index),
        "transcript_sha256": transcript_sha256,
        "transcript_size": transcript_size,
    }
    if any(result[field] != value for field, value in expected_values.items()):
        _fail(f"privacy protocol result {index} is not derived from its transcript")
    result_id = _sha256(result["result_id"], f"privacy protocol result {index} ID")
    body = dict(result)
    del body["result_id"]
    if compute_result_id(body) != result_id:
        _fail(f"privacy protocol result {index} ID is fabricated")
    return result, result_id


def validate_unsigned_v2_structure(
    root: Path,
    *,
    expected_source: Mapping[str, object],
    expected_validator_binary_sha256: str,
    expected_linux_release_archive_sha256: str,
    expected_exact12_matrix_sha256: str,
    expected_artifact_handoff_sha256: str,
    expected_receipt_id: str,
    now_unix: int,
) -> dict[str, object]:
    """Validate unsigned v2 structure without granting release authority.

    This helper exists for capture diagnostics and hostile-format tests.  Code
    that signs or admits a release must call ``validate_evidence_directory``
    and therefore remains closed until controller-origin authority is
    provisioned.
    """

    try:
        inventory = scan_inventory_paths(root)
    except ReleaseArtifactError as exc:
        raise PrivacyProtocolEvidenceError(
            f"cannot scan privacy protocol evidence: {exc}"
        ) from exc
    if inventory != sorted(EVIDENCE_NAMES):
        missing = sorted(set(EVIDENCE_NAMES) - set(inventory))
        extra = sorted(set(inventory) - set(EVIDENCE_NAMES))
        _fail(f"privacy protocol evidence inventory differs: missing={missing}, extra={extra}")

    receipt_payload, _receipt_sha, receipt_size = _read_evidence(
        root, RECEIPT_NAME, MAX_RECEIPT_BYTES, "privacy protocol receipt"
    )
    total_size = receipt_size
    receipt = _canonical_object(
        receipt_payload, "privacy protocol four-peer receipt", MAX_RECEIPT_BYTES
    )
    _exact_fields(
        receipt,
        {
            "candidate",
            "cases",
            "expires_at_unix",
            "issued_at_unix",
            "outcomes",
            "platform",
            "receipt_id",
            "schema",
            "schema_version",
        },
        "privacy protocol four-peer receipt",
    )
    if receipt["schema"] != RECEIPT_SCHEMA:
        _fail("privacy protocol receipt schema identifier is unsupported")
    if receipt["schema_version"] != RECEIPT_SCHEMA_VERSION:
        _fail("privacy protocol receipt schema version is unsupported; v1 is rejected")
    receipt_id = _sha256(receipt["receipt_id"], "privacy protocol receipt ID")
    body = dict(receipt)
    del body["receipt_id"]
    if compute_receipt_id(body) != receipt_id:
        _fail("privacy protocol receipt ID is fabricated")
    if receipt_id != _sha256(expected_receipt_id, "expected privacy receipt ID"):
        _fail("privacy protocol receipt ID differs from the independently expected ID")

    if receipt["platform"] != {"arch": "arm64", "os": "macos", "peer_count": PEER_COUNT}:
        _fail("privacy protocol receipt platform must be exact four-peer macos/arm64")
    issued = _integer(receipt["issued_at_unix"], "privacy receipt issue time")
    expires = _integer(receipt["expires_at_unix"], "privacy receipt expiry time")
    if expires <= issued or expires - issued > MAX_RECEIPT_LIFETIME_SECONDS:
        _fail("privacy protocol receipt lifetime is invalid")
    current = _integer(now_unix, "current Unix time")
    if issued > current + MAX_FUTURE_CLOCK_SKEW_SECONDS or current > expires:
        _fail("privacy protocol receipt is stale or issued in the future")

    candidate, driver_digests = _validated_candidate(
        receipt["candidate"],
        expected_source=expected_source,
        expected_validator_binary_sha256=expected_validator_binary_sha256,
        expected_linux_release_archive_sha256=expected_linux_release_archive_sha256,
        expected_exact12_matrix_sha256=expected_exact12_matrix_sha256,
        expected_artifact_handoff_sha256=expected_artifact_handoff_sha256,
    )
    candidate_binding_sha256 = compute_candidate_binding_sha256(candidate)

    for driver_name, evidence_name in sorted(DRIVER_EVIDENCE_NAMES.items()):
        try:
            driver_info = stable_hash_relative(
                root, evidence_name, max_size=MAX_DRIVER_BYTES
            )
        except ReleaseArtifactError as exc:
            raise PrivacyProtocolEvidenceError(
                f"cannot authenticate preserved {driver_name} bytes: {exc}"
            ) from exc
        if driver_info.sha256 != driver_digests[driver_name]:
            _fail(f"preserved {driver_name} bytes differ from the candidate binding")
        total_size += driver_info.size
        if total_size > MAX_EVIDENCE_TOTAL_BYTES:
            _fail("privacy protocol evidence exceeds its aggregate byte bound")

    case_rows = receipt["cases"]
    if not isinstance(case_rows, list) or len(case_rows) != len(CASE_DEFINITIONS):
        _fail("privacy protocol receipt must contain exactly seven case rows")
    seen_case_ids: set[str] = set()
    seen_result_ids: set[str] = set()
    normalized_cases: list[dict[str, object]] = []
    for index, (row, expected_definition) in enumerate(
        zip(case_rows, CASE_DEFINITIONS)
    ):
        if not isinstance(row, dict):
            _fail(f"privacy protocol case row {index} must be an object")
        _exact_fields(
            row,
            {
                "case",
                "index",
                "kind",
                "result_id",
                "result_path",
                "result_sha256",
                "result_size",
                "transcript_id",
                "transcript_path",
                "transcript_sha256",
                "transcript_size",
            },
            f"privacy protocol case row {index}",
        )
        expected_case, expected_kind = expected_definition
        if (
            row["index"] != index
            or row["case"] != expected_case
            or row["kind"] != expected_kind
            or row["transcript_path"] != transcript_name(index)
            or row["result_path"] != result_name(index)
        ):
            _fail("privacy protocol case rows are missing, duplicated, or reordered")
        transcript_payload, transcript_sha, transcript_size = _read_evidence(
            root,
            transcript_name(index),
            MAX_TRANSCRIPT_BYTES,
            f"privacy protocol transcript {index}",
        )
        result_payload, result_sha, result_size = _read_evidence(
            root,
            result_name(index),
            MAX_RESULT_BYTES,
            f"privacy protocol result {index}",
        )
        total_size += transcript_size + result_size
        if total_size > MAX_EVIDENCE_TOTAL_BYTES:
            _fail("privacy protocol evidence exceeds its aggregate byte bound")
        if (
            _sha256(row["transcript_sha256"], f"case {index} transcript digest")
            != transcript_sha
            or _integer(
                row["transcript_size"], f"case {index} transcript size", minimum=1
            )
            != transcript_size
            or _sha256(row["result_sha256"], f"case {index} result digest")
            != result_sha
            or _integer(row["result_size"], f"case {index} result size", minimum=1)
            != result_size
        ):
            _fail(f"privacy protocol case {index} stored bytes differ from its receipt")
        _transcript, transcript_id = _validate_transcript(
            transcript_payload,
            index=index,
            candidate_binding_sha256=candidate_binding_sha256,
            driver_digests=driver_digests,
        )
        _result, result_id = _validate_result(
            result_payload,
            index=index,
            candidate_binding_sha256=candidate_binding_sha256,
            transcript_id=transcript_id,
            transcript_sha256=transcript_sha,
            transcript_size=transcript_size,
        )
        if (
            row["transcript_id"] != transcript_id
            or row["result_id"] != result_id
        ):
            _fail(f"privacy protocol case {index} IDs differ from preserved bytes")
        if transcript_id in seen_case_ids or result_id in seen_result_ids:
            _fail("privacy protocol receipt repeats transcript or result evidence")
        seen_case_ids.add(transcript_id)
        seen_result_ids.add(result_id)
        normalized_cases.append(dict(row))

    outcomes = receipt["outcomes"]
    if not isinstance(outcomes, list) or len(outcomes) != len(OUTCOMES):
        _fail("privacy protocol receipt must contain exactly twelve outcomes")
    normalized_outcomes: list[dict[str, object]] = []
    for index, (row, expected) in enumerate(zip(outcomes, OUTCOMES)):
        if not isinstance(row, dict):
            _fail(f"privacy protocol outcome {index} must be an object")
        _exact_fields(
            row,
            {
                "case_index",
                "closed_reason",
                "index",
                "production_outcome",
                "profile",
                "protocol",
                "security_boundary",
            },
            f"privacy protocol outcome {index}",
        )
        (
            protocol,
            case_index,
            profile,
            production_outcome,
            closed_reason,
            security_boundary,
        ) = expected
        expected_row = {
            "case_index": case_index,
            "closed_reason": closed_reason,
            "index": index,
            "production_outcome": production_outcome,
            "profile": profile,
            "protocol": protocol,
            "security_boundary": security_boundary,
        }
        if row != expected_row:
            _fail(
                f"privacy protocol outcome {index} is unavailable, substituted, or noncanonical"
            )
        normalized_outcomes.append(dict(row))

    return {
        "artifact_handoff_sha256": candidate["artifact_handoff_sha256"],
        "case_count": len(normalized_cases),
        "cases": normalized_cases,
        "drivers": driver_digests,
        "exact12_matrix_sha256": candidate["exact12_matrix_sha256"],
        "linux_release_archive_sha256": candidate[
            "linux_release_archive_sha256"
        ],
        "outcomes": normalized_outcomes,
        "receipt_id": receipt_id,
        "validator_binary_sha256": candidate["validator_binary_sha256"],
    }


def _validated_authority_request(
    root: Path,
    *,
    expected_source: Mapping[str, object],
    expected_validator_binary_sha256: str,
    expected_linux_release_archive_sha256: str,
    expected_exact12_matrix_sha256: str,
    expected_artifact_handoff_sha256: str,
    expected_receipt_id: str,
    now_unix: int,
) -> tuple[
    dict[str, object],
    dict[str, object],
    tuple[taira_authority_client.Artifact, ...],
]:
    normalized_source = _source_identity(expected_source, "expected source identity")
    result = validate_unsigned_v2_structure(
        root,
        expected_source=normalized_source,
        expected_validator_binary_sha256=expected_validator_binary_sha256,
        expected_linux_release_archive_sha256=(
            expected_linux_release_archive_sha256
        ),
        expected_exact12_matrix_sha256=expected_exact12_matrix_sha256,
        expected_artifact_handoff_sha256=expected_artifact_handoff_sha256,
        expected_receipt_id=expected_receipt_id,
        now_unix=now_unix,
    )
    subject = {
        "authority_schema": CONTROLLER_ORIGIN_AUTHORITY_CONTRACT,
        "authenticated_run_schema": CONTROLLER_ORIGIN_AUTHENTICATED_RUN_CONTRACT,
        "expected": {
            "artifact_handoff_sha256": expected_artifact_handoff_sha256,
            "exact12_matrix_sha256": expected_exact12_matrix_sha256,
            "linux_release_archive_sha256": expected_linux_release_archive_sha256,
            "receipt_id": expected_receipt_id,
            "source": normalized_source,
            "validator_binary_sha256": expected_validator_binary_sha256,
        },
        "replay_namespace": CONTROLLER_ORIGIN_REPLAY_NAMESPACE,
        "structural_subject": result,
        "validation_time_unix": now_unix,
    }
    artifacts = tuple(
        taira_authority_client.Artifact(
            f"evidence/{name}",
            root / name,
            maximum=evidence_file_maximum(name),
        )
        for name in sorted(EVIDENCE_NAMES)
    )
    return result, subject, artifacts


def validate_evidence_directory(
    root: Path,
    *,
    expected_source: Mapping[str, object],
    expected_validator_binary_sha256: str,
    expected_linux_release_archive_sha256: str,
    expected_exact12_matrix_sha256: str,
    expected_artifact_handoff_sha256: str,
    expected_receipt_id: str,
    now_unix: int,
) -> AuthenticatedPrivacyProtocolEvidence:
    """Validate and authorize the exact controller-origin evidence inventory."""

    require_controller_origin_authority_provisioned()
    result, subject, artifacts = _validated_authority_request(
        root,
        expected_source=expected_source,
        expected_validator_binary_sha256=expected_validator_binary_sha256,
        expected_linux_release_archive_sha256=(
            expected_linux_release_archive_sha256
        ),
        expected_exact12_matrix_sha256=expected_exact12_matrix_sha256,
        expected_artifact_handoff_sha256=expected_artifact_handoff_sha256,
        expected_receipt_id=expected_receipt_id,
        now_unix=now_unix,
    )
    try:
        authority_result = taira_authority_client.authorize(
            "privacy-protocol-origin", subject, artifacts=artifacts
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PrivacyProtocolEvidenceError(
            f"controller-origin authority refused the exact evidence: {error}"
        ) from error
    return AuthenticatedPrivacyProtocolEvidence(
        result,
        evidence_root=root,
        authority_result=authority_result,
    )


def verify_authenticated_evidence_directory(
    root: Path,
    *,
    expected_source: Mapping[str, object],
    expected_validator_binary_sha256: str,
    expected_linux_release_archive_sha256: str,
    expected_exact12_matrix_sha256: str,
    expected_artifact_handoff_sha256: str,
    expected_receipt_id: str,
    now_unix: int,
) -> AuthenticatedPrivacyProtocolEvidence:
    """Historically verify deterministic sidecars without issuing a new receipt."""

    require_controller_origin_authority_provisioned(require_signing=False)
    result, subject, artifacts = _validated_authority_request(
        root,
        expected_source=expected_source,
        expected_validator_binary_sha256=expected_validator_binary_sha256,
        expected_linux_release_archive_sha256=(
            expected_linux_release_archive_sha256
        ),
        expected_exact12_matrix_sha256=expected_exact12_matrix_sha256,
        expected_artifact_handoff_sha256=expected_artifact_handoff_sha256,
        expected_receipt_id=expected_receipt_id,
        now_unix=now_unix,
    )
    envelope_path, receipt_path = authority_sidecar_paths(root)
    try:
        _, envelope_payload = stable_read_relative(
            envelope_path.parent,
            envelope_path.name,
            max_size=MAX_AUTHORITY_SIDECAR_BYTES,
            return_payload=True,
        )
        _, receipt_payload = stable_read_relative(
            receipt_path.parent,
            receipt_path.name,
            max_size=MAX_AUTHORITY_SIDECAR_BYTES,
            return_payload=True,
        )
        assert envelope_payload is not None
        assert receipt_payload is not None
        envelope = taira_authority_client.decode_canonical_json(
            envelope_payload, "privacy protocol authority envelope"
        )
        receipt = taira_authority_client.decode_canonical_json(
            receipt_payload, "privacy protocol durable receipt"
        )
        authority_result = taira_authority_client.verify_receipt(
            "privacy-protocol-origin",
            subject,
            authority_envelope=envelope,
            durable_receipt=receipt,
            artifacts=artifacts,
        )
    except (
        ReleaseArtifactError,
        taira_authority_client.TairaAuthorityClientError,
    ) as error:
        raise PrivacyProtocolEvidenceError(
            f"controller-origin historical receipt verification failed: {error}"
        ) from error
    return AuthenticatedPrivacyProtocolEvidence(
        result,
        evidence_root=root,
        authority_result=authority_result,
    )
