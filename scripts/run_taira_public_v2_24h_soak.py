#!/usr/bin/env python3
"""Run the protected deployed-public Taira v2 workload/capture attempt.

The public entry point is deliberately disabled.  The repository does not yet
contain the independently administered runtime signer, native evidence broker,
or public-soak admission authority required to turn network observations into
release evidence.  The private implementation below fixes the producer-side
contract without pretending that Python-generated JSON is native authority:
it consumes the three exact prerequisite handoffs and a native-verified anchor,
owns one :class:`PublicSoakLease`, dispatches the immutable 24-hour schedule,
and publishes only structurally verified, no-replace capture artifacts returned
by a protected native backend.

Successful production stops in the ``captured`` lease phase.  The caller must
keep the returned lease alive while the independent lifecycle collector and
admission broker complete their work.  This module never signs transactions,
persists signing material, manufactures native receipts, or claims admission.
"""

from __future__ import annotations

import argparse
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import time
from typing import NoReturn, Protocol

try:
    from scripts import check_taira_public_v2_24h_soak_evidence as checker
    from scripts import taira_constants
    from scripts import taira_public_v2_24h_soak_state as lease_state
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import check_taira_public_v2_24h_soak_evidence as checker
    import taira_constants
    import taira_public_v2_24h_soak_state as lease_state


DURATION_MS = 86_400_000
CONFIRMATION_DRAIN_MS = 15 * 60 * 1_000
SLOT_INTERVAL_MS = 200
TRANSFER_SLOTS = 432_000
SAMPLE_INTERVAL_MS = 60_000
MAX_SUBMISSION_START_LATENESS_MS = 1_000
MAX_ANCHOR_BYTES = 16 * 1024 * 1024
MAX_LAUNCH_ACK_BYTES = 1024 * 1024
MAX_TRANSCRIPT_BYTES = 512 * 1024 * 1024
MAX_TRANSCRIPT_RECORDS = 2_000_000

LAUNCH_ACK_SCHEMA = "iroha.taira.public-v2-24h-producer-launch-ack.v1"
LAUNCH_ACK_PROTOCOL = "iroha-taira-public-soak-native-producer-v1"
LAUNCH_RECEIPT_SCHEMA = (
    "iroha.taira.public-v2-24h-producer-launch-native-verifier-receipt.v1"
)
LAUNCH_RECEIPT_PROTOCOL = "iroha-taira-public-soak-native-launch-verifier-v1"
LAUNCH_SUBJECT_SCHEMA = "iroha.taira.public-v2-24h-producer-launch-subject.v1"
BACKEND_CLOSURE_SCHEMA = (
    "iroha.taira.public-v2-24h-producer-native-backend-closure-receipt.v1"
)
BACKEND_CLOSURE_PROTOCOL = "iroha-taira-public-soak-native-backend-closure-v1"
SAMPLE_SCHEMA = "iroha.taira.public-v2-24h-samples.v1"
TRANSCRIPT_SCHEMA = "iroha.taira.public-v2-24h-producer-transcript.v1"
CAPTURE_SCHEMA = "iroha.taira.public-v2-24h-capture-handoff.v1"
CAPTURE_FILENAME = "TAIRA_PUBLIC_V2_24H_CAPTURED.json"
ANCHOR_FILENAME = "soak-anchor.json"
LAUNCH_ACK_FILENAME = "producer-launch-acknowledgement.json"
LAUNCH_RECEIPT_FILENAME = "producer-launch-native-verifier-receipt.json"
LAUNCH_SUBJECT_FILENAME = "producer-launch-subject.json"
BACKEND_CLOSURE_FILENAME = "producer-native-backend-closure-receipt.json"
WORKLOAD_FILENAME = "workload-inventory.jsonl"
SUBMISSION_FILENAME = "submission-receipt-inventory.jsonl"
STATUS_FILENAME = "applied-status-inventory.jsonl"
BLOCK_FILENAME = "block-evidence-inventory.jsonl"
SAMPLE_FILENAME = "samples.jsonl"
TRANSCRIPT_FILENAME = "producer-transcript.jsonl"
FAILURE_DOMAIN = b"iroha.taira.public-v2-24h.producer-failure.v1\0"
LAUNCH_SUBJECT_DOMAIN = b"iroha.taira.public-v2-24h.producer-launch-subject.v1\0"

LAUNCH_ACK_FIELDS = {
    "schema",
    "schema_version",
    "protocol",
    "launch_subject_sha256",
    "soak_anchor_sha256",
    "anchor_observation_completed_at_unix_ms",
    "producer_identity_sha256",
    "producer_pid",
    "workload_started_at_unix_ms",
    "workload_started_monotonic_ns",
    "native_verifier_binary_sha256",
    "native_verifier_source_sha256",
    "native_verifier_receipt_sha256",
    "native_verifier_receipt_size_bytes",
    "verification_result",
}
LAUNCH_RECEIPT_FIELDS = {
    "schema",
    "schema_version",
    "protocol",
    "launch_subject_sha256",
    "soak_anchor_sha256",
    "producer_identity_sha256",
    "producer_pid",
    "workload_started_at_unix_ms",
    "workload_started_monotonic_ns",
    "verifier_binary_sha256",
    "verifier_source_sha256",
    "verification_result",
}
LAUNCH_SUBJECT_FIELDS = {
    "schema",
    "source",
    "prerequisites",
    "deployment",
    "soak_anchor",
    "iroha3d_sha256",
    "producer_identity_sha256",
    "native_verifier",
}
LAUNCH_PREREQUISITE_FIELDS = {
    "candidate_handoff_sha256",
    "publication_handoff_sha256",
    "deploy_handoff_sha256",
}
LAUNCH_DEPLOYMENT_FIELDS = {
    "qualification_receipt_id",
    "admission_receipt_id",
    "network_name",
    "chain_id",
    "network_id",
    "protocol_version",
    "genesis_block_hash",
    "end_height",
    "end_block_hash",
    "deployment_completed_at_unix_ms",
    "controller_host_id",
    "controller_installation_id",
    "controller_sha256",
    "restart_generation",
    "config_set_sha256",
    "topology_sha256",
    "signed_genesis_sha256",
    "supervisor_sha256",
    "receipt_signers",
}
BACKEND_CLOSURE_FIELDS = {
    "schema",
    "schema_version",
    "protocol",
    "launch_subject_sha256",
    "workload_started_at_unix_ms",
    "evidence_completed_at_unix_ms",
    "workload_closed_at_elapsed_ms",
    "transfer_slot_count",
    "outstanding_submission_count",
    "signer_session_closed",
    "verifier_binary_sha256",
    "verifier_source_sha256",
    "verification_result",
}
TRANSCRIPT_RECORD_FIELDS = {
    "sequence",
    "event",
    "recorded_at_unix_ms",
    "elapsed_monotonic_ms",
    "subject_sha256",
    "native_receipt_sha256",
    "result",
}
TRANSCRIPT_EVENTS = {
    "producer-launched",
    "slot-batch-captured",
    "sample-captured",
    "workload-window-closed",
    "confirmation-drain-closed",
}
CAPTURE_FIELDS = {
    "schema",
    "schema_version",
    "state",
    "lease_id",
    "binding_sha256",
    "source",
    "prerequisites",
    "timing",
    "native_verifier",
    "soak_anchor",
    "producer_launch",
    "artifacts",
    "downstream",
}
TIMING_FIELDS = {
    "workload_started_at_unix_ms",
    "workload_ended_at_unix_ms",
    "evidence_completed_at_unix_ms",
    "workload_duration_ms",
    "confirmation_drain_ms",
}
DOWNSTREAM_FIELDS = {
    "lifecycle_collection_required",
    "lifecycle_collector",
    "authority_admission_required",
    "completion_claimed",
}

RUNNER_AUTHORITY_BARRIER = (
    "missing preprovisioned protected public-soak producer authority: the "
    "86,400,000ms workload requires a runtime-only signer, an independently "
    "built and pinned native anchor/submission/status/block verifier, a "
    "process-bound launch acknowledgement, and external lifecycle/admission "
    "brokers; public production is disabled before caller path or network I/O"
)


class PublicSoakRunnerError(RuntimeError):
    """The protected public-soak producer contract was not satisfied."""


def _fail(message: str) -> NoReturn:
    raise PublicSoakRunnerError(message)


# TODO: Replace this permanent refusal only when a separately administered
# native producer endpoint, runtime signer, verifier pin, lifecycle collector,
# and admission broker are installed by the protected controller.  No
# environment variable or caller file may provision that authority.
def require_public_soak_runner_authority_provisioned() -> NoReturn:
    """Refuse public production until the independent native stack exists."""

    _fail(RUNNER_AUTHORITY_BARRIER)


def _canonical_json(value: object) -> bytes:
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
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise PublicSoakRunnerError(
            f"public-soak capture is not canonically encodable: {error}"
        ) from error


def _exact(value: object, fields: set[str], label: str) -> Mapping[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        _fail(f"{label} fields are not exact")
    return value


def _integer(value: object, label: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _digest(value: object, label: str) -> str:
    try:
        return checker._artifact_sha256(value, label)
    except checker.EvidenceError as error:
        raise PublicSoakRunnerError(str(error)) from error


def _decode_canonical(payload: bytes, label: str) -> dict[str, object]:
    try:
        return checker._decode_json(payload, label, canonical=True)
    except checker.EvidenceError as error:
        raise PublicSoakRunnerError(str(error)) from error


@dataclass(frozen=True)
class PrerequisiteContext:
    """Stable prerequisite captures and their structurally closed identities."""

    source: Mapping[str, object]
    handoff_digests: Mapping[str, str]
    deploy: Mapping[str, object]
    candidate: checker.Artifact
    publication: checker.Artifact
    deploy_handoff: checker.Artifact
    anchor: checker.Artifact
    anchor_document: Mapping[str, object]
    producer: checker.Artifact
    native_binary_sha256: str
    native_source_sha256: str
    iroha3d_sha256: str


@dataclass(frozen=True)
class NativeLaunch:
    """Exact acknowledgement and independently replayable verifier receipt."""

    acknowledgement: bytes
    native_verifier_receipt: bytes


@dataclass(frozen=True)
class NativeCaptureBundle:
    """Secret-free rows emitted by the independently verified native backend."""

    workload_records: Sequence[Mapping[str, object]]
    submission_records: Sequence[Mapping[str, object]]
    status_records: Sequence[Mapping[str, object]]
    block_records: Sequence[Mapping[str, object]]
    samples: Sequence[Mapping[str, object]]
    transcript_records: Sequence[Mapping[str, object]]
    evidence_completed_at_unix_ms: int
    native_backend_closure_receipt: bytes


@dataclass(frozen=True)
class PublishedArtifact:
    """One atomically linked, no-replace canonical output artifact."""

    filename: str
    kind: str
    schema: str
    sha256: str
    size_bytes: int
    record_count: int | None = None
    records_sha256: str | None = None

    def reference(self) -> dict[str, object]:
        """Return the checker-compatible inventory or simple-file reference."""

        value: dict[str, object] = {
            "kind": self.kind,
            "schema": self.schema,
            "sha256": self.sha256,
            "size_bytes": self.size_bytes,
        }
        if self.record_count is not None:
            value["record_count"] = self.record_count
            value["records_sha256"] = self.records_sha256
        return value


@dataclass(frozen=True)
class CapturedAttempt:
    """A captured attempt whose live lease must pass to downstream admission."""

    lease: lease_state.PublicSoakLease
    capture_handoff: Path
    capture_set_sha256: str
    artifacts: Mapping[str, PublishedArtifact]


class MonotonicClock(Protocol):
    """The minimal clock surface used by the exact slot dispatcher."""

    def monotonic_ns(self) -> int:
        """Return the current monotonic clock in nanoseconds."""

    def wall_ms(self) -> int:
        """Return the current Unix wall clock in milliseconds."""

    def sleep_until_ns(self, deadline_ns: int) -> None:
        """Do not return before ``deadline_ns`` on the same monotonic epoch."""


class SystemClock:
    """Production clock implementation with short, interruption-safe sleeps."""

    def monotonic_ns(self) -> int:
        return time.monotonic_ns()

    def wall_ms(self) -> int:
        return time.time_ns() // 1_000_000

    def sleep_until_ns(self, deadline_ns: int) -> None:
        while True:
            remaining = deadline_ns - time.monotonic_ns()
            if remaining <= 0:
                return
            time.sleep(min(remaining / 1_000_000_000, 0.2))


class NativeCaptureBackend(Protocol):
    """Protected native signer/verifier process used only after provisioning."""

    def launch(
        self,
        *,
        anchor_payload: bytes,
        anchor_sha256: str,
        producer_identity_sha256: str,
        native_verifier_binary_sha256: str,
        native_verifier_source_sha256: str,
        launch_subject_sha256: str,
        launch_subject_payload: bytes,
    ) -> NativeLaunch:
        """Return a canonical acknowledgement and its exact native receipt."""

    def submit_slot(self, sequence: int, scheduled_elapsed_ms: int) -> None:
        """Start exactly one pre-authorized signed transfer without blocking."""

    def close_workload(self, scheduled_elapsed_ms: int) -> None:
        """Close submissions at the exact end of the workload window."""

    def seal_capture(self, deadline_monotonic_ns: int) -> NativeCaptureBundle:
        """Drain finality and return the native-verified secret-free capture."""

    def abort(self, reason_code: str) -> None:
        """Stop signer/workload activity; this operation must be idempotent."""


class _OutputRoot:
    """A held, stable owner-private output directory with atomic no-replace writes."""

    def __init__(self, root: Path) -> None:
        if (
            not root.is_absolute()
            or root == Path("/")
            or Path(os.path.abspath(root)) != root
        ):
            _fail("capture root must be one normalized absolute path")
        try:
            if root.resolve(strict=True) != root:
                _fail("capture root must not traverse symbolic links")
            descriptor = os.open(
                root,
                os.O_RDONLY
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_NOFOLLOW", 0),
            )
        except OSError as error:
            raise PublicSoakRunnerError("cannot open the capture root") from error
        try:
            identity = self._validate_metadata(os.fstat(descriptor))
            if os.listdir(descriptor):
                _fail("capture root must be empty and cannot be resumed")
        except BaseException:
            os.close(descriptor)
            raise
        self.root = root
        self.fd = descriptor
        self.identity = identity

    @staticmethod
    def _validate_metadata(info: os.stat_result) -> tuple[int, int]:
        if (
            not stat.S_ISDIR(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o700
            or info.st_uid != os.geteuid()
        ):
            _fail("capture root must be one owner-private mode-0700 directory")
        return info.st_dev, info.st_ino

    def confirm(self) -> None:
        current = self._validate_metadata(os.fstat(self.fd))
        try:
            named_metadata = self.root.lstat()
            if stat.S_ISLNK(named_metadata.st_mode):
                _fail("capture root pathname was replaced by a symbolic link")
            named = self._validate_metadata(named_metadata)
        except OSError as error:
            raise PublicSoakRunnerError("capture root pathname is unavailable") from error
        if current != self.identity or named != self.identity:
            _fail("capture root pathname or identity changed")

    def close(self) -> None:
        if self.fd >= 0:
            os.close(self.fd)
            self.fd = -1

    @staticmethod
    def _write_all(descriptor: int, payload: bytes) -> None:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("capture artifact write did not make progress")
            view = view[written:]

    def _publish_chunks(
        self,
        name: str,
        chunks: Iterable[bytes],
        *,
        maximum_bytes: int,
    ) -> tuple[str, int]:
        if not name or name != Path(name).name or name.startswith("."):
            _fail("capture artifact filename is not one fixed leaf")
        self.confirm()
        temporary = f".{name}.{os.getpid()}.tmp"
        flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = -1
        linked = False
        try:
            descriptor = os.open(temporary, flags, 0o600, dir_fd=self.fd)
            hasher = hashlib.sha256()
            total = 0
            for chunk in chunks:
                if not chunk:
                    _fail("capture artifact contains an empty write chunk")
                total += len(chunk)
                if total > maximum_bytes:
                    _fail(f"capture artifact {name} exceeds its fixed bound")
                self._write_all(descriptor, chunk)
                hasher.update(chunk)
            if total <= 0:
                _fail("capture artifact must not be empty")
            os.fsync(descriptor)
            os.close(descriptor)
            descriptor = -1
            os.link(
                temporary,
                name,
                src_dir_fd=self.fd,
                dst_dir_fd=self.fd,
                follow_symlinks=False,
            )
            linked = True
            os.unlink(temporary, dir_fd=self.fd)
            os.fsync(self.fd)
            metadata = os.stat(name, dir_fd=self.fd, follow_symlinks=False)
            if (
                not stat.S_ISREG(metadata.st_mode)
                or stat.S_IMODE(metadata.st_mode) != 0o600
                or metadata.st_uid != os.geteuid()
                or metadata.st_nlink != 1
                or metadata.st_size != total
            ):
                _fail("published capture artifact identity is not exact")
            self.confirm()
            return hasher.hexdigest(), total
        except FileExistsError as error:
            raise PublicSoakRunnerError(
                f"capture artifact already exists; refusing overwrite: {name}"
            ) from error
        finally:
            if descriptor >= 0:
                os.close(descriptor)
            if not linked:
                try:
                    os.unlink(temporary, dir_fd=self.fd)
                except FileNotFoundError:
                    pass

    def publish_bytes(
        self,
        name: str,
        payload: bytes,
        *,
        kind: str,
        schema: str,
        maximum_bytes: int,
    ) -> PublishedArtifact:
        digest, size = self._publish_chunks(
            name, (payload,), maximum_bytes=maximum_bytes
        )
        return PublishedArtifact(name, kind, schema, digest, size)

    def publish_inventory(
        self,
        name: str,
        records: Sequence[Mapping[str, object]],
        *,
        kind: str,
        schema: str,
        fields: set[str],
        maximum_bytes: int,
    ) -> PublishedArtifact:
        count = len(records)
        if count <= 0:
            _fail(f"{kind} inventory must not be empty")
        header = _canonical_json(
            {"record_count": count, "schema": schema, "schema_version": 1}
        )
        record_hasher = hashlib.sha256()
        record_hasher.update(
            f"iroha.taira.public-v2-24h.{kind}-records.v1\0".encode("ascii")
        )
        def encoded_records() -> Iterable[bytes]:
            yield header
            for index, raw in enumerate(records):
                record = _exact(raw, fields, f"{kind} record {index}")
                payload = _canonical_json(record)
                record_hasher.update(payload)
                yield payload

        digest, size = self._publish_chunks(
            name, encoded_records(), maximum_bytes=maximum_bytes
        )
        return PublishedArtifact(
            name,
            kind,
            schema,
            digest,
            size,
            count,
            record_hasher.hexdigest(),
        )


def _handoff_reference(
    artifact: checker.Artifact,
    *,
    kind: str,
    source: Mapping[str, object],
) -> dict[str, object]:
    return {
        "kind": kind,
        "schema": checker.HANDOFF_SCHEMA,
        "sha256": artifact.sha256,
        "size_bytes": artifact.size,
        "source": dict(source),
    }


def _anchor_prelude(
    anchor: Mapping[str, object], deploy: Mapping[str, object]
) -> None:
    _exact(anchor, checker.ANCHOR_FIELDS, "soak anchor")
    if anchor["schema"] != "iroha.taira.public-v2-24h-soak-anchor.v1":
        _fail("soak anchor schema is wrong")
    started = _integer(
        anchor["observation_started_at_unix_ms"], "anchor observation start", minimum=1
    )
    completed = _integer(
        anchor["observation_completed_at_unix_ms"],
        "anchor observation completion",
        minimum=1,
    )
    if (
        completed < started
        or completed - started > checker.MAX_OBSERVATION_WINDOW_MS
        or started < deploy["deployment_completed_at_unix_ms"]
    ):
        _fail("soak anchor observation window is not post-deploy and bounded")
    for field in ("controller_host_id", "controller_installation_id"):
        if anchor[field] != deploy[field]:
            _fail("soak anchor controller identity differs from deployment")
    if anchor["controller_sha256"] != deploy["controller_sha256"]:
        _fail("soak anchor controller binary differs from deployment")
    for field in (
        "controller_signing_key_id",
        "controller_receipt_sha256",
        "controller_signature_sha256",
    ):
        _digest(anchor[field], f"anchor {field}")
    if (
        _integer(anchor["deploy_end_height"], "anchor deploy height", minimum=1)
        != deploy["end_height"]
    ):
        _fail("soak anchor deploy height differs from the deploy handoff")
    try:
        end_hash = checker._iroha_hash(
            anchor["deploy_end_block_hash"],
            "anchor deploy block hash",
            checker.BLOCK_HASH_TYPE,
        )
    except checker.EvidenceError as error:
        raise PublicSoakRunnerError(str(error)) from error
    if end_hash != deploy["end_hash"]:
        _fail("soak anchor deploy block differs from the deploy handoff")
    common_index = _integer(
        anchor["common_start_block_evidence_index"],
        "anchor common-start block index",
        minimum=1,
    )
    validators = anchor["validators"]
    if not isinstance(validators, list) or len(validators) != 4:
        _fail("soak anchor must contain exactly four validator attestations")
    seen_nodes: set[str] = set()
    seen_challenges: set[str] = set()
    seen_artifacts: set[str] = set()
    used_native_receipts = set(deploy["native_verifier_receipts"])
    for index, validator in enumerate(checker.VALIDATORS):
        row = _exact(
            validators[index],
            checker.ANCHOR_VALIDATOR_FIELDS,
            f"anchor validator {validator}",
        )
        if row["validator_id"] != validator:
            _fail("soak anchor validators are not in canonical order")
        node_id = checker._identity_text(row["node_id"], "anchor node ID")
        if node_id != deploy["receipt_signers"][validator]["node_id"]:
            _fail("soak anchor validator node differs from deployment")
        if node_id in seen_nodes:
            _fail("soak anchor validator nodes are aliased")
        seen_nodes.add(node_id)
        challenge = checker._hex_32(row["challenge_hex"], "anchor challenge")
        if challenge in seen_challenges:
            _fail("soak anchor challenge was reused")
        seen_challenges.add(challenge)
        attested = _integer(row["attested_at_unix_ms"], "anchor attestation", minimum=1)
        if not started <= attested <= completed:
            _fail("soak anchor attestation is outside its observation window")
        _integer(
            row["tip_block_evidence_index"],
            "anchor validator tip index",
            minimum=common_index,
        )
        for digest_field, size_field in (
            ("attestation_sha256", "attestation_size_bytes"),
            ("ancestry_proof_sha256", "ancestry_proof_size_bytes"),
            ("native_verifier_receipt_sha256", "native_verifier_receipt_size_bytes"),
        ):
            artifact_digest = _digest(row[digest_field], f"anchor {digest_field}")
            _integer(row[size_field], f"anchor {size_field}", minimum=1)
            if artifact_digest in seen_artifacts:
                _fail("soak anchor evidence artifact was reused")
            seen_artifacts.add(artifact_digest)
            if digest_field == "native_verifier_receipt_sha256":
                if artifact_digest in used_native_receipts:
                    _fail("soak anchor native verifier receipt was reused")
                used_native_receipts.add(artifact_digest)
        if row["verification_result"] != "verified":
            _fail("soak anchor native verification did not pass")


def _load_prerequisites(
    *,
    candidate_handoff: Path,
    publication_handoff: Path,
    deploy_handoff: Path,
    soak_anchor: Path,
    expected_source: Mapping[str, object],
    iroha3d_sha256: str,
    native_verifier_binary_sha256: str,
    native_verifier_source_sha256: str,
) -> PrerequisiteContext:
    try:
        source = checker._source_identity(expected_source, "expected source")
        candidate = checker._read_stable(
            candidate_handoff, checker.MAX_HANDOFF_BYTES, "candidate handoff"
        )
        publication = checker._read_stable(
            publication_handoff, checker.MAX_HANDOFF_BYTES, "publication handoff"
        )
        deploy_artifact = checker._read_stable(
            deploy_handoff, checker.MAX_HANDOFF_BYTES, "deploy handoff"
        )
        anchor_artifact = checker._read_stable(
            soak_anchor, MAX_ANCHOR_BYTES, "soak anchor"
        )
        producer = checker._read_stable(
            Path(__file__).resolve(), 4 * 1024 * 1024, "public-soak producer"
        )
        captures = (candidate, publication, deploy_artifact, anchor_artifact, producer)
        if len({(item.device, item.inode) for item in captures}) != len(captures):
            _fail("public-soak prerequisites contain a filesystem alias")
        deploy_document = checker._decode_json(
            deploy_artifact.payload, "deploy handoff", canonical=True
        )
        deploy_identity = checker._exact(
            deploy_document["identity"],
            checker.DEPLOY_IDENTITY_FIELDS,
            "deploy handoff identity",
        )
        network = {
            "name": taira_constants.NETWORK_NAME,
            "chain_id": taira_constants.CHAIN_ID,
            "network_id": deploy_identity["network_id"],
            "protocol_version": checker.PROTOCOL_VERSION,
            "genesis_block_hash": deploy_identity["genesis_block_hash"],
        }
        references = {
            "candidate_handoff": _handoff_reference(
                candidate, kind="candidate", source=source
            ),
            "publication_handoff": _handoff_reference(
                publication, kind="publication", source=source
            ),
            "deploy_handoff": _handoff_reference(
                deploy_artifact, kind="deploy", source=source
            ),
        }
        handoff_digests, deploy = checker._validate_prerequisites(
            references,
            source=source,
            expected_binary_sha256=iroha3d_sha256,
            candidate_artifact=candidate,
            publication_artifact=publication,
            deploy_artifact=deploy_artifact,
            network=network,
            expected_native_binary_sha256=native_verifier_binary_sha256,
            expected_native_source_sha256=native_verifier_source_sha256,
        )
        anchor_document = checker._decode_json(
            anchor_artifact.payload, "soak anchor", canonical=True
        )
        _anchor_prelude(anchor_document, deploy)
        return PrerequisiteContext(
            source=source,
            handoff_digests=handoff_digests,
            deploy=deploy,
            candidate=candidate,
            publication=publication,
            deploy_handoff=deploy_artifact,
            anchor=anchor_artifact,
            anchor_document=anchor_document,
            producer=producer,
            native_binary_sha256=_digest(
                native_verifier_binary_sha256, "native verifier binary"
            ),
            native_source_sha256=_digest(
                native_verifier_source_sha256, "native verifier source"
            ),
            iroha3d_sha256=_digest(iroha3d_sha256, "deployed iroha3d binary"),
        )
    except checker.EvidenceError as error:
        raise PublicSoakRunnerError(str(error)) from error


def _launch_subject(context: PrerequisiteContext) -> tuple[dict[str, object], str]:
    """Close every prerequisite consumed by the native slot-zero launch."""

    deploy = context.deploy
    deployment = {
        "qualification_receipt_id": deploy["qualification_receipt_id"],
        "admission_receipt_id": deploy["admission_receipt_id"],
        "network_name": deploy["network_name"],
        "chain_id": deploy["chain_id"],
        "network_id": deploy["network_id"],
        "protocol_version": deploy["protocol_version"],
        "genesis_block_hash": {
            "algorithm": checker.IROHA_HASH_ALGORITHM,
            "type": checker.BLOCK_HASH_TYPE,
            "value": deploy["genesis_block_hash"],
        },
        "end_height": deploy["end_height"],
        "end_block_hash": {
            "algorithm": checker.IROHA_HASH_ALGORITHM,
            "type": checker.BLOCK_HASH_TYPE,
            "value": deploy["end_hash"],
        },
        "deployment_completed_at_unix_ms": deploy[
            "deployment_completed_at_unix_ms"
        ],
        "controller_host_id": deploy["controller_host_id"],
        "controller_installation_id": deploy["controller_installation_id"],
        "controller_sha256": deploy["controller_sha256"],
        "restart_generation": deploy["restart_generation"],
        "config_set_sha256": deploy["config_set_sha256"],
        "topology_sha256": deploy["topology_sha256"],
        "signed_genesis_sha256": deploy["signed_genesis_sha256"],
        "supervisor_sha256": deploy["supervisor_sha256"],
        "receipt_signers": deploy["receipt_signers"],
    }
    _exact(deployment, LAUNCH_DEPLOYMENT_FIELDS, "launch deployment projection")
    prerequisites = {
        "candidate_handoff_sha256": context.handoff_digests["candidate"],
        "publication_handoff_sha256": context.handoff_digests["publication"],
        "deploy_handoff_sha256": context.handoff_digests["deploy"],
    }
    _exact(prerequisites, LAUNCH_PREREQUISITE_FIELDS, "launch prerequisites")
    subject = {
        "schema": LAUNCH_SUBJECT_SCHEMA,
        "source": dict(context.source),
        "prerequisites": prerequisites,
        "deployment": deployment,
        "soak_anchor": {
            "sha256": context.anchor.sha256,
            "size_bytes": context.anchor.size,
        },
        "iroha3d_sha256": context.iroha3d_sha256,
        "producer_identity_sha256": context.producer.sha256,
        "native_verifier": {
            "protocol": checker.NATIVE_VERIFIER_PROTOCOL,
            "binary_sha256": context.native_binary_sha256,
            "source_sha256": context.native_source_sha256,
        },
    }
    _exact(subject, LAUNCH_SUBJECT_FIELDS, "producer launch subject")
    return subject, hashlib.sha256(
        LAUNCH_SUBJECT_DOMAIN + _canonical_json(subject)
    ).hexdigest()


def _validate_launch_ack(
    native_launch: NativeLaunch,
    context: PrerequisiteContext,
    expected_launch_subject_sha256: str,
) -> tuple[Mapping[str, object], lease_state.ProducerLaunch]:
    if not isinstance(native_launch, NativeLaunch):
        _fail("native backend omitted the exact launch receipt bundle")
    payload = native_launch.acknowledgement
    receipt_payload = native_launch.native_verifier_receipt
    if not payload or len(payload) > MAX_LAUNCH_ACK_BYTES:
        _fail("producer launch acknowledgement is empty or oversized")
    if not receipt_payload or len(receipt_payload) > MAX_LAUNCH_ACK_BYTES:
        _fail("producer launch native verifier receipt is empty or oversized")
    launch = _exact(
        _decode_canonical(payload, "producer launch acknowledgement"),
        LAUNCH_ACK_FIELDS,
        "producer launch acknowledgement",
    )
    if (
        launch["schema"] != LAUNCH_ACK_SCHEMA
        or type(launch["schema_version"]) is not int
        or launch["schema_version"] != 1
        or launch["protocol"] != LAUNCH_ACK_PROTOCOL
        or launch["verification_result"] != "verified"
    ):
        _fail("producer launch acknowledgement identity is wrong")
    if launch["launch_subject_sha256"] != expected_launch_subject_sha256:
        _fail("producer launch acknowledgement changes its closed launch subject")
    if launch["soak_anchor_sha256"] != context.anchor.sha256:
        _fail("producer launch acknowledgement does not consume the exact anchor")
    if launch["producer_identity_sha256"] != context.producer.sha256:
        _fail("producer launch acknowledgement does not bind this producer")
    if (
        launch["native_verifier_binary_sha256"] != context.native_binary_sha256
        or launch["native_verifier_source_sha256"] != context.native_source_sha256
    ):
        _fail("producer launch acknowledgement native verifier is not pinned")
    if _integer(launch["producer_pid"], "producer PID", minimum=1) != os.getpid():
        _fail("producer launch acknowledgement belongs to another process")
    anchor_completed = _integer(
        context.anchor_document["observation_completed_at_unix_ms"],
        "anchor completion",
        minimum=1,
    )
    if (
        _integer(
            launch["anchor_observation_completed_at_unix_ms"],
            "launch anchor completion",
            minimum=1,
        )
        != anchor_completed
    ):
        _fail("producer launch acknowledgement changes the anchor completion time")
    started_wall = _integer(
        launch["workload_started_at_unix_ms"], "workload start wall time", minimum=1
    )
    started_monotonic = _integer(
        launch["workload_started_monotonic_ns"],
        "workload start monotonic time",
        minimum=1,
    )
    if not 0 <= started_wall - anchor_completed <= checker.MAX_ANCHOR_TO_WORKLOAD_GAP_MS:
        _fail("producer launch acknowledgement is outside the fresh anchor window")
    native_receipt = _digest(
        launch["native_verifier_receipt_sha256"], "launch native verifier receipt"
    )
    native_receipt_size = _integer(
        launch["native_verifier_receipt_size_bytes"],
        "launch native verifier receipt size",
        minimum=1,
    )
    if (
        native_receipt != hashlib.sha256(receipt_payload).hexdigest()
        or native_receipt_size != len(receipt_payload)
    ):
        _fail("producer launch acknowledgement does not bind its exact native receipt")
    reserved_native_receipts = set(context.deploy["native_verifier_receipts"])
    anchor_validators = context.anchor_document["validators"]
    assert isinstance(anchor_validators, list)
    reserved_native_receipts.update(
        str(row["native_verifier_receipt_sha256"])
        for row in anchor_validators
        if isinstance(row, dict)
    )
    if native_receipt in reserved_native_receipts:
        _fail("producer launch native verifier receipt was reused")
    receipt = _exact(
        _decode_canonical(receipt_payload, "producer launch native verifier receipt"),
        LAUNCH_RECEIPT_FIELDS,
        "producer launch native verifier receipt",
    )
    expected_receipt = {
        "schema": LAUNCH_RECEIPT_SCHEMA,
        "schema_version": 1,
        "protocol": LAUNCH_RECEIPT_PROTOCOL,
        "launch_subject_sha256": expected_launch_subject_sha256,
        "soak_anchor_sha256": context.anchor.sha256,
        "producer_identity_sha256": context.producer.sha256,
        "producer_pid": os.getpid(),
        "workload_started_at_unix_ms": started_wall,
        "workload_started_monotonic_ns": started_monotonic,
        "verifier_binary_sha256": context.native_binary_sha256,
        "verifier_source_sha256": context.native_source_sha256,
        "verification_result": "verified",
    }
    if receipt != expected_receipt:
        _fail("producer launch native verifier receipt claims are not exact")
    return launch, lease_state.ProducerLaunch(
        soak_anchor_sha256=context.anchor.sha256,
        anchor_observation_completed_at_unix_ms=anchor_completed,
        producer_launch_sha256=hashlib.sha256(payload).hexdigest(),
        producer_identity_sha256=context.producer.sha256,
        producer_pid=os.getpid(),
        workload_started_at_unix_ms=started_wall,
        workload_started_monotonic_ns=started_monotonic,
    )


def _lease_binding(
    context: PrerequisiteContext,
    launch: Mapping[str, object],
) -> lease_state.LeaseBinding:
    source = context.source
    deploy = context.deploy
    anchor = context.anchor_document
    genesis = deploy["genesis_block_hash"]
    assert isinstance(genesis, str)
    return lease_state.LeaseBinding(
        source_commit=str(source["commit"]),
        dpn_validator_release_commit=str(source["dpn_validator_release_commit"]),
        cargo_lock_sha256=str(source["cargo_lock_sha256"]),
        workspace_source_manifest_sha256=str(
            source["workspace_source_manifest_sha256"]
        ),
        candidate_handoff_sha256=context.handoff_digests["candidate"],
        publication_handoff_sha256=context.handoff_digests["publication"],
        deploy_handoff_sha256=context.handoff_digests["deploy"],
        network_id=str(deploy["network_id"]),
        genesis_block_hash=genesis,
        deploy_end_height=int(deploy["end_height"]),
        deploy_end_block_hash=str(deploy["end_hash"]),
        deployment_completed_at_unix_ms=int(
            deploy["deployment_completed_at_unix_ms"]
        ),
        controller_host_id=str(deploy["controller_host_id"]),
        controller_installation_id=str(deploy["controller_installation_id"]),
        controller_sha256=str(deploy["controller_sha256"]),
        controller_signing_key_id=str(anchor["controller_signing_key_id"]),
        native_verifier_binary_sha256=str(
            launch["native_verifier_binary_sha256"]
        ),
        native_verifier_source_sha256=str(
            launch["native_verifier_source_sha256"]
        ),
    )


def _dispatch_exact_slots(
    backend: NativeCaptureBackend,
    clock: MonotonicClock,
    started_monotonic_ns: int,
) -> None:
    """Dispatch all 432,000 fixed slots against one monotonic epoch."""

    if started_monotonic_ns <= 0:
        _fail("workload start monotonic time must be positive")
    for sequence in range(TRANSFER_SLOTS):
        scheduled_ms = sequence * SLOT_INTERVAL_MS
        deadline = started_monotonic_ns + scheduled_ms * 1_000_000
        clock.sleep_until_ns(deadline)
        observed = clock.monotonic_ns()
        if observed < deadline:
            _fail("slot clock returned before its monotonic deadline")
        if observed - deadline > MAX_SUBMISSION_START_LATENESS_MS * 1_000_000:
            _fail(f"transfer slot {sequence} missed its fixed start bound")
        backend.submit_slot(sequence, scheduled_ms)
    boundary = started_monotonic_ns + DURATION_MS * 1_000_000
    clock.sleep_until_ns(boundary)
    if clock.monotonic_ns() < boundary:
        _fail("workload clock returned before the exact 24-hour boundary")
    backend.close_workload(DURATION_MS)


def _baseline_from_first_sample(
    samples: Sequence[Mapping[str, object]],
) -> dict[str, Mapping[str, object]]:
    if not samples:
        _fail("native capture did not return samples")
    first = _exact(samples[0], checker.SAMPLE_FIELDS, "first sample")
    validators = first["validators"]
    if not isinstance(validators, list) or len(validators) != 4:
        _fail("first sample does not contain four validators")
    baseline: dict[str, Mapping[str, object]] = {}
    for index, validator in enumerate(checker.VALIDATORS):
        row = _exact(
            validators[index],
            checker.VALIDATOR_SAMPLE_FIELDS,
            f"first sample validator {validator}",
        )
        if row["validator_id"] != validator:
            _fail("first sample validators are not in canonical order")
        baseline[validator] = {
            field: row[field]
            for field in (
                "restart_count",
                "supervisor_generation",
                "process_generation",
                "unexpected_exit_total",
            )
        }
    return baseline


def _validate_transcript(
    records: Sequence[Mapping[str, object]],
    *,
    expected_sample_count: int | None = None,
) -> None:
    if not 1 <= len(records) <= MAX_TRANSCRIPT_RECORDS:
        _fail("producer transcript count is outside its fixed bound")
    prior_wall = 0
    prior_elapsed = 0
    events: list[str] = []
    for sequence, raw in enumerate(records):
        row = _exact(raw, TRANSCRIPT_RECORD_FIELDS, f"transcript record {sequence}")
        if _integer(row["sequence"], "transcript sequence") != sequence:
            _fail("producer transcript sequence is not exact and contiguous")
        if row["event"] not in TRANSCRIPT_EVENTS or row["result"] != "verified":
            _fail("producer transcript event or result is not allow-listed")
        wall = _integer(row["recorded_at_unix_ms"], "transcript wall time", minimum=1)
        elapsed = _integer(row["elapsed_monotonic_ms"], "transcript elapsed time")
        if wall < prior_wall or elapsed < prior_elapsed:
            _fail("producer transcript clocks regressed")
        _digest(row["subject_sha256"], "transcript subject")
        _digest(row["native_receipt_sha256"], "transcript native receipt")
        event = str(row["event"])
        if event == "producer-launched" and (sequence != 0 or elapsed != 0):
            _fail("producer launch is not the exact first transcript event")
        if event == "workload-window-closed" and elapsed != DURATION_MS:
            _fail("workload close transcript event is not at 86,400,000ms")
        if event == "confirmation-drain-closed" and not (
            DURATION_MS <= elapsed <= DURATION_MS + CONFIRMATION_DRAIN_MS
        ):
            _fail("confirmation-drain close is outside its fixed window")
        events.append(event)
        prior_wall = wall
        prior_elapsed = elapsed
    if (
        events[0] != "producer-launched"
        or events[-1] != "confirmation-drain-closed"
        or events.count("producer-launched") != 1
        or events.count("workload-window-closed") != 1
        or events.count("confirmation-drain-closed") != 1
        or "slot-batch-captured" not in events
        or events.index("workload-window-closed")
        >= events.index("confirmation-drain-closed")
    ):
        _fail("producer transcript lifecycle is not exact and naturally ordered")
    if (
        expected_sample_count is not None
        and events.count("sample-captured") != expected_sample_count
    ):
        _fail("producer transcript does not cover every captured sample exactly once")


def _validate_backend_closure(
    payload: bytes,
    *,
    context: PrerequisiteContext,
    launch: Mapping[str, object],
    completed_ms: int,
) -> Mapping[str, object]:
    """Require a durable native receipt proving signer/workload shutdown."""

    if not payload or len(payload) > MAX_LAUNCH_ACK_BYTES:
        _fail("native backend closure receipt is empty or oversized")
    receipt = _exact(
        _decode_canonical(payload, "native backend closure receipt"),
        BACKEND_CLOSURE_FIELDS,
        "native backend closure receipt",
    )
    expected = {
        "schema": BACKEND_CLOSURE_SCHEMA,
        "schema_version": 1,
        "protocol": BACKEND_CLOSURE_PROTOCOL,
        "launch_subject_sha256": launch["launch_subject_sha256"],
        "workload_started_at_unix_ms": launch["workload_started_at_unix_ms"],
        "evidence_completed_at_unix_ms": completed_ms,
        "workload_closed_at_elapsed_ms": DURATION_MS,
        "transfer_slot_count": TRANSFER_SLOTS,
        "outstanding_submission_count": 0,
        "signer_session_closed": True,
        "verifier_binary_sha256": context.native_binary_sha256,
        "verifier_source_sha256": context.native_source_sha256,
        "verification_result": "verified",
    }
    if receipt != expected:
        _fail("native backend closure receipt claims are not exact")
    return receipt


def _publish_capture(
    *,
    output: _OutputRoot,
    context: PrerequisiteContext,
    lease: lease_state.PublicSoakLease,
    launch: Mapping[str, object],
    launch_subject_artifact: PublishedArtifact,
    launch_artifact: PublishedArtifact,
    launch_receipt_artifact: PublishedArtifact,
    anchor_artifact: PublishedArtifact,
    bundle: NativeCaptureBundle,
) -> tuple[PublishedArtifact, dict[str, PublishedArtifact]]:
    started_ms = _integer(
        launch["workload_started_at_unix_ms"], "workload start", minimum=1
    )
    completed_ms = _integer(
        bundle.evidence_completed_at_unix_ms, "evidence completion", minimum=1
    )
    workload_end_ms = started_ms + DURATION_MS
    if not workload_end_ms <= completed_ms <= workload_end_ms + CONFIRMATION_DRAIN_MS:
        _fail("native capture completion is outside the bounded finality drain")
    _validate_backend_closure(
        bundle.native_backend_closure_receipt,
        context=context,
        launch=launch,
        completed_ms=completed_ms,
    )
    if (
        len(bundle.workload_records) != TRANSFER_SLOTS
        or len(bundle.submission_records) != TRANSFER_SLOTS
        or len(bundle.status_records) != TRANSFER_SLOTS
    ):
        _fail("native capture does not contain exactly 432,000 transfer rows")
    maximum_samples = (DURATION_MS + CONFIRMATION_DRAIN_MS) // SAMPLE_INTERVAL_MS
    if not DURATION_MS // SAMPLE_INTERVAL_MS <= len(bundle.samples) <= maximum_samples:
        _fail("native capture sample count cannot cover the workload and drain")
    if not 2 <= len(bundle.block_records) <= checker.MAX_BLOCK_COUNT:
        _fail("native capture block count cannot prove deploy-descendant finality")
    _validate_transcript(
        bundle.transcript_records, expected_sample_count=len(bundle.samples)
    )

    workload = output.publish_inventory(
        WORKLOAD_FILENAME,
        bundle.workload_records,
        kind="workload",
        schema=checker.WORKLOAD_SCHEMA,
        fields=checker.WORKLOAD_RECORD_FIELDS,
        maximum_bytes=checker.MAX_WORKLOAD_BYTES,
    )
    submissions = output.publish_inventory(
        SUBMISSION_FILENAME,
        bundle.submission_records,
        kind="submissions",
        schema=checker.SUBMISSION_SCHEMA,
        fields=checker.SUBMISSION_RECORD_FIELDS,
        maximum_bytes=checker.MAX_SUBMISSION_BYTES,
    )
    statuses = output.publish_inventory(
        STATUS_FILENAME,
        bundle.status_records,
        kind="statuses",
        schema=checker.STATUS_SCHEMA,
        fields=checker.STATUS_RECORD_FIELDS,
        maximum_bytes=checker.MAX_STATUS_BYTES,
    )
    blocks = output.publish_inventory(
        BLOCK_FILENAME,
        bundle.block_records,
        kind="blocks",
        schema=checker.BLOCK_SCHEMA,
        fields=checker.BLOCK_RECORD_FIELDS,
        maximum_bytes=checker.MAX_BLOCK_BYTES,
    )
    samples = output.publish_inventory(
        SAMPLE_FILENAME,
        bundle.samples,
        kind="samples",
        schema=SAMPLE_SCHEMA,
        fields=checker.SAMPLE_FIELDS,
        maximum_bytes=checker.MAX_STATUS_BYTES,
    )
    transcript = output.publish_inventory(
        TRANSCRIPT_FILENAME,
        bundle.transcript_records,
        kind="transcript",
        schema=TRANSCRIPT_SCHEMA,
        fields=TRANSCRIPT_RECORD_FIELDS,
        maximum_bytes=MAX_TRANSCRIPT_BYTES,
    )
    backend_closure = output.publish_bytes(
        BACKEND_CLOSURE_FILENAME,
        bundle.native_backend_closure_receipt,
        kind="producer-native-backend-closure",
        schema=BACKEND_CLOSURE_SCHEMA,
        maximum_bytes=MAX_LAUNCH_ACK_BYTES,
    )

    try:
        used_native_receipts = set(context.deploy["native_verifier_receipts"])
        launch_receipt_sha256 = _digest(
            launch["native_verifier_receipt_sha256"],
            "launch native verifier receipt",
        )
        if launch_receipt_sha256 in used_native_receipts:
            _fail("launch native verifier receipt was reused")
        used_native_receipts.add(launch_receipt_sha256)
        closure_receipt_sha256 = hashlib.sha256(
            bundle.native_backend_closure_receipt
        ).hexdigest()
        if closure_receipt_sha256 in used_native_receipts:
            _fail("native backend closure receipt was reused")
        used_native_receipts.add(closure_receipt_sha256)
        block_capture = checker._read_stable(
            output.root / BLOCK_FILENAME, checker.MAX_BLOCK_BYTES, "block inventory"
        )
        verified_blocks, block_records_sha256 = checker._validate_block_inventory(
            block_capture,
            blocks.reference(),
            context.deploy,
            used_native_receipts,
        )
        used_challenges: set[str] = set()
        anchor_sha256, _gap = checker._validate_anchor(
            context.anchor_document,
            deploy=context.deploy,
            blocks=verified_blocks,
            started_ms=started_ms,
            used_challenges=used_challenges,
            used_native_receipts=used_native_receipts,
        )
        baseline = _baseline_from_first_sample(bundle.samples)
        verified_samples, _sample_metrics, sample_set_sha256 = (
            checker._validate_samples(
                bundle.samples,
                started_ms=started_ms,
                completed_ms=completed_ms,
                blocks=verified_blocks,
                deploy=context.deploy,
                lifecycle_baseline=baseline,
                used_challenges=used_challenges,
                used_native_receipts=used_native_receipts,
            )
        )
        submission_capture = checker._read_stable(
            output.root / SUBMISSION_FILENAME,
            checker.MAX_SUBMISSION_BYTES,
            "submission inventory",
        )
        verified_submissions, submission_records_sha256 = (
            checker._validate_submission_inventory(
                submission_capture,
                submissions.reference(),
                started_ms=started_ms,
                completed_ms=completed_ms,
                deploy=context.deploy,
                used_native_receipts=used_native_receipts,
            )
        )
        status_capture = checker._read_stable(
            output.root / STATUS_FILENAME,
            checker.MAX_STATUS_BYTES,
            "Applied status inventory",
        )
        verified_statuses, status_records_sha256 = checker._validate_status_inventory(
            status_capture,
            statuses.reference(),
            started_ms=started_ms,
            completed_ms=completed_ms,
            samples=verified_samples,
            blocks=verified_blocks,
            used_native_receipts=used_native_receipts,
        )
        workload_reference = workload.reference()
        workload_reference["first_signed_transaction_hash"] = bundle.workload_records[
            0
        ]["signed_transaction_hash"]
        workload_reference["last_signed_transaction_hash"] = bundle.workload_records[
            -1
        ]["signed_transaction_hash"]
        workload_capture = checker._read_stable(
            output.root / WORKLOAD_FILENAME,
            checker.MAX_WORKLOAD_BYTES,
            "workload inventory",
        )
        _workload_sha256, workload_records_sha256, _metrics = (
            checker._validate_workload_inventory(
                workload_capture,
                workload_reference,
                started_ms=started_ms,
                completed_ms=completed_ms,
                submissions=verified_submissions,
                statuses=verified_statuses,
                blocks=verified_blocks,
            )
        )
        checker._cross_validate_sample_counts(verified_samples, verified_statuses)
    except checker.EvidenceError as error:
        raise PublicSoakRunnerError(str(error)) from error
    if (
        workload_records_sha256 != workload.records_sha256
        or submission_records_sha256 != submissions.records_sha256
        or status_records_sha256 != statuses.records_sha256
        or block_records_sha256 != blocks.records_sha256
    ):
        _fail("published inventory identity differs from structural validation")

    artifacts = {
        "anchor": anchor_artifact,
        "producer_launch_subject": launch_subject_artifact,
        "producer_launch": launch_artifact,
        "producer_launch_native_receipt": launch_receipt_artifact,
        "producer_native_backend_closure": backend_closure,
        "workload": workload,
        "submissions": submissions,
        "statuses": statuses,
        "blocks": blocks,
        "samples": samples,
        "transcript": transcript,
    }
    timing = {
        "workload_started_at_unix_ms": started_ms,
        "workload_ended_at_unix_ms": workload_end_ms,
        "evidence_completed_at_unix_ms": completed_ms,
        "workload_duration_ms": DURATION_MS,
        "confirmation_drain_ms": completed_ms - workload_end_ms,
    }
    _exact(timing, TIMING_FIELDS, "capture timing")
    downstream = {
        "lifecycle_collection_required": True,
        "lifecycle_collector": "scripts/collect_taira_public_v2_lifecycle_evidence.py",
        "authority_admission_required": True,
        "completion_claimed": False,
    }
    _exact(downstream, DOWNSTREAM_FIELDS, "capture downstream handoff")
    artifact_references = {
        label: artifact.reference() for label, artifact in artifacts.items()
    }
    artifact_references["anchor"]["semantic_sha256"] = anchor_sha256
    artifact_references["samples"]["sample_set_sha256"] = sample_set_sha256
    manifest = {
        "schema": CAPTURE_SCHEMA,
        "schema_version": 1,
        "state": "captured-not-admitted",
        "lease_id": lease.lease_id,
        "binding_sha256": lease.binding_sha256,
        "source": dict(context.source),
        "prerequisites": {
            "candidate_handoff_sha256": context.handoff_digests["candidate"],
            "publication_handoff_sha256": context.handoff_digests["publication"],
            "deploy_handoff_sha256": context.handoff_digests["deploy"],
        },
        "timing": timing,
        "native_verifier": {
            "protocol": checker.NATIVE_VERIFIER_PROTOCOL,
            "binary_sha256": context.native_binary_sha256,
            "source_sha256": context.native_source_sha256,
        },
        "soak_anchor": {
            "sha256": context.anchor.sha256,
            "semantic_sha256": anchor_sha256,
        },
        "producer_launch": {
            "sha256": launch_artifact.sha256,
            "launch_subject_sha256": launch["launch_subject_sha256"],
            "launch_subject_artifact_sha256": launch_subject_artifact.sha256,
            "native_verifier_receipt_sha256": launch[
                "native_verifier_receipt_sha256"
            ],
            "native_verifier_receipt_size_bytes": launch[
                "native_verifier_receipt_size_bytes"
            ],
        },
        "artifacts": artifact_references,
        "downstream": downstream,
    }
    _exact(manifest, CAPTURE_FIELDS, "capture handoff")
    capture = output.publish_bytes(
        CAPTURE_FILENAME,
        _canonical_json(manifest),
        kind="capture",
        schema=CAPTURE_SCHEMA,
        maximum_bytes=16 * 1024 * 1024,
    )
    return capture, artifacts


def _replay_context(context: PrerequisiteContext) -> None:
    for artifact, maximum, label in (
        (context.candidate, checker.MAX_HANDOFF_BYTES, "candidate handoff"),
        (context.publication, checker.MAX_HANDOFF_BYTES, "publication handoff"),
        (context.deploy_handoff, checker.MAX_HANDOFF_BYTES, "deploy handoff"),
        (context.anchor, MAX_ANCHOR_BYTES, "soak anchor"),
        (context.producer, 4 * 1024 * 1024, "public-soak producer"),
    ):
        try:
            replay = checker._read_stable(artifact.path, maximum, label)
        except checker.EvidenceError as error:
            raise PublicSoakRunnerError(str(error)) from error
        if replay != artifact:
            _fail(f"{label} changed during the public-soak attempt")


def _failure_digest(lease_id: str, stage: str, error: BaseException) -> str:
    document = {
        "error_type": type(error).__name__,
        "lease_id": lease_id,
        "stage": stage,
    }
    return hashlib.sha256(FAILURE_DOMAIN + _canonical_json(document)).hexdigest()


def _run_with_native_backend(
    *,
    candidate_handoff: Path,
    publication_handoff: Path,
    deploy_handoff: Path,
    soak_anchor: Path,
    state_root: Path,
    capture_root: Path,
    expected_source: Mapping[str, object],
    iroha3d_sha256: str,
    native_verifier_binary_sha256: str,
    native_verifier_source_sha256: str,
    backend: NativeCaptureBackend,
    clock: MonotonicClock | None = None,
) -> CapturedAttempt:
    """Execute one structural attempt through an injected protected backend.

    This private seam does not provision or authenticate the backend.  It is
    usable by tests and, eventually, by a sealed controller that has already
    established the missing independent authority.
    """

    active_clock = clock or SystemClock()
    context = _load_prerequisites(
        candidate_handoff=candidate_handoff,
        publication_handoff=publication_handoff,
        deploy_handoff=deploy_handoff,
        soak_anchor=soak_anchor,
        expected_source=expected_source,
        iroha3d_sha256=iroha3d_sha256,
        native_verifier_binary_sha256=native_verifier_binary_sha256,
        native_verifier_source_sha256=native_verifier_source_sha256,
    )
    output = _OutputRoot(capture_root)
    lease: lease_state.PublicSoakLease | None = None
    stage = "workload"
    succeeded = False
    launch_attempted = False
    try:
        if state_root.resolve(strict=True) == capture_root:
            _fail("lease state and capture roots must be distinct")
        launch_subject_document, launch_subject_sha256 = _launch_subject(context)
        launch_subject_payload = _canonical_json(launch_subject_document)
        launch_attempted = True
        native_launch = backend.launch(
            anchor_payload=context.anchor.payload,
            anchor_sha256=context.anchor.sha256,
            producer_identity_sha256=context.producer.sha256,
            native_verifier_binary_sha256=context.native_binary_sha256,
            native_verifier_source_sha256=context.native_source_sha256,
            launch_subject_sha256=launch_subject_sha256,
            launch_subject_payload=launch_subject_payload,
        )
        launch, producer_launch = _validate_launch_ack(
            native_launch,
            context,
            launch_subject_sha256,
        )
        lease = lease_state.start_lease(
            state_root,
            _lease_binding(context, launch),
            producer_launch,
        )
        anchor_publication = output.publish_bytes(
            ANCHOR_FILENAME,
            context.anchor.payload,
            kind="anchor",
            schema="iroha.taira.public-v2-24h-soak-anchor.v1",
            maximum_bytes=MAX_ANCHOR_BYTES,
        )
        launch_publication = output.publish_bytes(
            LAUNCH_ACK_FILENAME,
            native_launch.acknowledgement,
            kind="producer-launch",
            schema=LAUNCH_ACK_SCHEMA,
            maximum_bytes=MAX_LAUNCH_ACK_BYTES,
        )
        launch_subject_publication = output.publish_bytes(
            LAUNCH_SUBJECT_FILENAME,
            launch_subject_payload,
            kind="producer-launch-subject",
            schema=LAUNCH_SUBJECT_SCHEMA,
            maximum_bytes=MAX_LAUNCH_ACK_BYTES,
        )
        launch_receipt_publication = output.publish_bytes(
            LAUNCH_RECEIPT_FILENAME,
            native_launch.native_verifier_receipt,
            kind="producer-launch-native-receipt",
            schema=LAUNCH_RECEIPT_SCHEMA,
            maximum_bytes=MAX_LAUNCH_ACK_BYTES,
        )
        _dispatch_exact_slots(
            backend,
            active_clock,
            producer_launch.workload_started_monotonic_ns,
        )
        stage = "capture"
        deadline_ns = producer_launch.workload_started_monotonic_ns + (
            DURATION_MS + CONFIRMATION_DRAIN_MS
        ) * 1_000_000
        bundle = backend.seal_capture(deadline_ns)
        if active_clock.monotonic_ns() > deadline_ns:
            _fail("native capture returned after the fixed confirmation-drain deadline")
        capture, artifacts = _publish_capture(
            output=output,
            context=context,
            lease=lease,
            launch=launch,
            launch_subject_artifact=launch_subject_publication,
            launch_artifact=launch_publication,
            launch_receipt_artifact=launch_receipt_publication,
            anchor_artifact=anchor_publication,
            bundle=bundle,
        )
        _replay_context(context)
        output.confirm()
        capture_set_sha256 = capture.sha256
        lease_state.record_capture(lease, capture_set_sha256)
        succeeded = True
        return CapturedAttempt(
            lease=lease,
            capture_handoff=capture_root / CAPTURE_FILENAME,
            capture_set_sha256=capture_set_sha256,
            artifacts=artifacts,
        )
    except BaseException as error:
        code = (
            "controller_shutdown"
            if isinstance(error, (KeyboardInterrupt, SystemExit))
            else "workload_failed"
            if stage == "workload"
            else "capture_failed"
        )
        abort_error: BaseException | None = None
        if launch_attempted:
            try:
                backend.abort(code)
            except BaseException as failure:
                abort_error = failure
        if lease is not None and not lease._closed:
            try:
                lease_state.record_failed(
                    lease,
                    code=code,
                    evidence_sha256=_failure_digest(lease.lease_id, stage, error),
                )
            except BaseException:
                lease.close()
        if abort_error is not None:
            raise PublicSoakRunnerError(
                "public-soak attempt failed with "
                f"{type(error).__name__}; native backend abort also failed with "
                f"{type(abort_error).__name__}"
            ) from error
        raise
    finally:
        output.close()
        if not succeeded and lease is not None and not lease._closed:
            lease.close()


def run_public_soak(
    *,
    candidate_handoff: Path,
    publication_handoff: Path,
    deploy_handoff: Path,
    soak_anchor: Path,
    state_root: Path,
    capture_root: Path,
    native_producer_endpoint: str,
    expected_source: Mapping[str, object],
    iroha3d_sha256: str,
    native_verifier_binary_sha256: str,
    native_verifier_source_sha256: str,
) -> NoReturn:
    """Public runner barrier, intentionally before every supplied path or endpoint."""

    require_public_soak_runner_authority_provisioned()


def build_parser() -> argparse.ArgumentParser:
    """Build the disabled protected public-soak producer command line."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--candidate-handoff", type=Path, required=True)
    parser.add_argument("--publication-handoff", type=Path, required=True)
    parser.add_argument("--deploy-handoff", type=Path, required=True)
    parser.add_argument("--soak-anchor", type=Path, required=True)
    parser.add_argument("--state-root", type=Path, required=True)
    parser.add_argument("--capture-root", type=Path, required=True)
    parser.add_argument("--native-producer-endpoint", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--dpn-validator-release-commit", required=True)
    parser.add_argument("--cargo-lock-sha256", required=True)
    parser.add_argument("--workspace-source-manifest-sha256", required=True)
    parser.add_argument("--iroha3d-sha256", required=True)
    parser.add_argument("--native-verifier-binary-sha256", required=True)
    parser.add_argument("--native-verifier-source-sha256", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Refuse the unprovisioned public producer without inspecting its inputs."""

    args = build_parser().parse_args(argv)
    source = {
        "commit": args.source_commit,
        "dpn_validator_release_commit": args.dpn_validator_release_commit,
        "cargo_lock_sha256": args.cargo_lock_sha256,
        "workspace_source_manifest_sha256": args.workspace_source_manifest_sha256,
    }
    try:
        run_public_soak(
            candidate_handoff=args.candidate_handoff,
            publication_handoff=args.publication_handoff,
            deploy_handoff=args.deploy_handoff,
            soak_anchor=args.soak_anchor,
            state_root=args.state_root,
            capture_root=args.capture_root,
            native_producer_endpoint=args.native_producer_endpoint,
            expected_source=source,
            iroha3d_sha256=args.iroha3d_sha256,
            native_verifier_binary_sha256=args.native_verifier_binary_sha256,
            native_verifier_source_sha256=args.native_verifier_source_sha256,
        )
    except PublicSoakRunnerError as error:
        print(f"Taira public-v2 24h soak refused: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
