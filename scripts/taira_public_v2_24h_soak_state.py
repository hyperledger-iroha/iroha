#!/usr/bin/env python3
"""Durable fail-closed lease state for a deployed-public Taira soak.

The actual workload runner is intentionally separate.  A runner must hold the
returned :class:`PublicSoakLease` for its whole lifetime; the file lock is never
reacquired and an abandoned state is never resumed or stolen.  This makes a
controller crash, exec boundary, or stale invocation an explicit failed release
attempt instead of an opportunity to manufacture a terminal receipt.

This module does not provision the public-soak observation authority, sign
transactions, contact Taira, or claim completion.  It supplies the durable
state boundary that a protected runner and its independent admission broker
must share.
"""

# TODO: Make the protected public-soak runner own this lease for its single
# long-lived workload/capture invocation, then bind its captured state into the
# independently verified admission flow.  The installed controller must inject
# a preprovisioned state root beneath non-writable trusted ancestry; it must not
# accept that root from release input.  Do not expose this library as a
# controller operation by itself.

from __future__ import annotations

from dataclasses import dataclass
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import stat
import time
from typing import NoReturn

try:
    from scripts import taira_constants
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import taira_constants


SCHEMA = "iroha.taira.public-v2-24h-controller-lease.v1"
SCHEMA_VERSION = 1
STATE_FILENAME = "TAIRA_PUBLIC_V2_24H_LEASE.json"
LOCK_FILENAME = ".TAIRA_PUBLIC_V2_24H_LEASE.lock"
STATE_DOMAIN = b"iroha.taira.public-v2-24h.controller-lease-state.v1\0"
BINDING_DOMAIN = b"iroha.taira.public-v2-24h.controller-lease-binding.v1\0"
LEASE_ID_DOMAIN = b"iroha.taira.public-v2-24h.controller-lease-id.v1\0"
OWNER_DOMAIN = b"iroha.taira.public-v2-24h.controller-owner.v1\0"

DURATION_MS = 86_400_000
CONFIRMATION_DRAIN_MS = 15 * 60 * 1_000
MAX_WALL_MONOTONIC_SKEW_MS = 5_000
MAX_CLOCK_SAMPLE_WINDOW_MS = 1_000
MAX_LEASE_CREATION_DELAY_MS = 1_000
MAX_ANCHOR_TO_WORKLOAD_GAP_MS = 30_000
TARGET_TPS = 5
SLOT_INTERVAL_MS = 200
TRANSFER_SLOTS = 432_000
MAX_STATE_BYTES = 256 * 1024

PHASE_RUNNING = "running"
PHASE_CAPTURED = "captured"
PHASE_ADMISSION_PENDING = "admission_pending"
PHASE_COMPLETED = "completed"
PHASE_FAILED = "failed"
PHASES = {
    PHASE_RUNNING,
    PHASE_CAPTURED,
    PHASE_ADMISSION_PENDING,
    PHASE_COMPLETED,
    PHASE_FAILED,
}
TERMINAL_PHASES = {PHASE_COMPLETED, PHASE_FAILED}
FAILURE_CODES = {
    "controller_shutdown",
    "deadline_missed",
    "workload_failed",
    "capture_failed",
    "evidence_failed",
    "admission_failed",
}

SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
IDENTITY_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:@+-]{7,255}")

SOURCE_FIELDS = {
    "commit",
    "dpn_validator_release_commit",
    "cargo_lock_sha256",
    "workspace_source_manifest_sha256",
}
PREREQUISITE_FIELDS = {
    "candidate_handoff_sha256",
    "publication_handoff_sha256",
    "deploy_handoff_sha256",
}
DEPLOYMENT_FIELDS = {
    "network_name",
    "chain_id",
    "network_id",
    "protocol_version",
    "genesis_block_hash",
    "deploy_end_height",
    "deploy_end_block_hash",
    "deployment_completed_at_unix_ms",
}
HASH_FIELDS = {"algorithm", "type", "value"}
CONTROLLER_FIELDS = {
    "host_id",
    "installation_id",
    "controller_sha256",
    "signing_key_id",
    "native_verifier_binary_sha256",
    "native_verifier_source_sha256",
}
LAUNCH_FIELDS = {
    "soak_anchor_sha256",
    "anchor_observation_completed_at_unix_ms",
    "producer_launch_sha256",
    "producer_identity_sha256",
    "producer_pid",
    "workload_started_at_unix_ms",
    "workload_started_monotonic_ns",
}
BINDING_FIELDS = {
    "source",
    "prerequisites",
    "deployment",
    "controller",
    "launch",
}
PROFILE_FIELDS = {
    "duration_ms",
    "confirmation_drain_ms",
    "target_tps",
    "slot_interval_ms",
    "transfer_slots",
    "validator_count",
    "quorum",
    "fault_injection",
}
TIMING_FIELDS = {
    "workload_started_at_unix_ms",
    "workload_started_monotonic_ns",
}
ARTIFACT_FIELDS = {
    "capture_set_sha256",
    "completion_sha256",
    "authority_subject_sha256",
    "authority_envelope_sha256",
    "admission_receipt_sha256",
}
EVENT_FIELDS = {
    "sequence",
    "phase",
    "recorded_at_unix_ms",
    "elapsed_monotonic_ms",
    "evidence_sha256",
}
FAILURE_FIELDS = {"code", "evidence_sha256"}
TOP_LEVEL_FIELDS = {
    "schema",
    "schema_version",
    "lease_id",
    "owner_nonce_sha256",
    "owner_process_id",
    "lock_device",
    "lock_inode",
    "binding_sha256",
    "binding",
    "profile",
    "timing",
    "phase",
    "artifacts",
    "history",
    "failure",
    "state_sha256",
}

PROFILE = {
    "duration_ms": DURATION_MS,
    "confirmation_drain_ms": CONFIRMATION_DRAIN_MS,
    "target_tps": TARGET_TPS,
    "slot_interval_ms": SLOT_INTERVAL_MS,
    "transfer_slots": TRANSFER_SLOTS,
    "validator_count": 4,
    "quorum": 3,
    "fault_injection": "none",
}


class PublicSoakLeaseError(RuntimeError):
    """The protected public-soak lease contract was not satisfied."""


def _fail(message: str) -> NoReturn:
    raise PublicSoakLeaseError(message)


def _wall_clock_ms() -> int:
    return time.time_ns() // 1_000_000


def _monotonic_ns() -> int:
    return time.monotonic_ns()


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
        raise PublicSoakLeaseError(
            f"lease state is not canonically encodable: {error}"
        ) from error


def _reject_constant(value: str) -> NoReturn:
    _fail(f"non-finite lease JSON number is forbidden: {value}")


def _pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            _fail(f"duplicate lease JSON field is forbidden: {key}")
        value[key] = item
    return value


def _decode_canonical(payload: bytes) -> dict[str, object]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_pairs,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, ValueError) as error:
        raise PublicSoakLeaseError("lease state is not strict JSON") from error
    if not isinstance(value, dict) or _canonical_json(value) != payload:
        _fail("lease state is not one canonical closed JSON object")
    return value


def _exact(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        _fail(f"{label} fields are not exact")
    return value


def _integer(value: object, label: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _digest(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} must be one nonzero lowercase SHA-256 digest")
    return value


def _optional_digest(value: object, label: str) -> str | None:
    if value is None:
        return None
    return _digest(value, label)


def _commit(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or value == "0" * 40
    ):
        _fail(f"{label} must be one nonzero lowercase Git object ID")
    return value


def _identity(value: object, label: str) -> str:
    if not isinstance(value, str) or IDENTITY_RE.fullmatch(value) is None:
        _fail(f"{label} is not one bounded canonical identity")
    return value


def _iroha_hash(value: object, label: str, hash_type: str) -> dict[str, object]:
    marked = _exact(value, HASH_FIELDS, label)
    if marked["algorithm"] != "blake2b-32" or marked["type"] != hash_type:
        _fail(f"{label} algorithm or HashOf type is wrong")
    digest = _digest(marked["value"], f"{label} value")
    if int(digest[-2:], 16) & 1 == 0:
        _fail(f"{label} value is missing the Iroha marker bit")
    return marked


def _domain_digest(domain: bytes, value: object) -> str:
    return hashlib.sha256(domain + _canonical_json(value)).hexdigest()


@dataclass(frozen=True)
class LeaseBinding:
    """Immutable source, deploy, controller, and verifier identity for one run."""

    source_commit: str
    dpn_validator_release_commit: str
    cargo_lock_sha256: str
    workspace_source_manifest_sha256: str
    candidate_handoff_sha256: str
    publication_handoff_sha256: str
    deploy_handoff_sha256: str
    network_id: str
    genesis_block_hash: str
    deploy_end_height: int
    deploy_end_block_hash: str
    deployment_completed_at_unix_ms: int
    controller_host_id: str
    controller_installation_id: str
    controller_sha256: str
    controller_signing_key_id: str
    native_verifier_binary_sha256: str
    native_verifier_source_sha256: str

    def document(self, launch: ProducerLaunch) -> dict[str, object]:
        """Return the closed canonical binding embedded in lease state."""

        document = {
            "source": {
                "commit": self.source_commit,
                "dpn_validator_release_commit": self.dpn_validator_release_commit,
                "cargo_lock_sha256": self.cargo_lock_sha256,
                "workspace_source_manifest_sha256": (
                    self.workspace_source_manifest_sha256
                ),
            },
            "prerequisites": {
                "candidate_handoff_sha256": self.candidate_handoff_sha256,
                "publication_handoff_sha256": self.publication_handoff_sha256,
                "deploy_handoff_sha256": self.deploy_handoff_sha256,
            },
            "deployment": {
                "network_name": taira_constants.NETWORK_NAME,
                "chain_id": taira_constants.CHAIN_ID,
                "network_id": self.network_id,
                "protocol_version": 4,
                "genesis_block_hash": {
                    "algorithm": "blake2b-32",
                    "type": "HashOf<BlockHeader>",
                    "value": self.genesis_block_hash,
                },
                "deploy_end_height": self.deploy_end_height,
                "deploy_end_block_hash": {
                    "algorithm": "blake2b-32",
                    "type": "HashOf<BlockHeader>",
                    "value": self.deploy_end_block_hash,
                },
                "deployment_completed_at_unix_ms": (
                    self.deployment_completed_at_unix_ms
                ),
            },
            "controller": {
                "host_id": self.controller_host_id,
                "installation_id": self.controller_installation_id,
                "controller_sha256": self.controller_sha256,
                "signing_key_id": self.controller_signing_key_id,
                "native_verifier_binary_sha256": (
                    self.native_verifier_binary_sha256
                ),
                "native_verifier_source_sha256": (
                    self.native_verifier_source_sha256
                ),
            },
            "launch": launch.document(),
        }
        return _validate_binding(document)


@dataclass(frozen=True)
class ProducerLaunch:
    """Native-verified anchor and slot-zero producer launch acknowledgement."""

    soak_anchor_sha256: str
    anchor_observation_completed_at_unix_ms: int
    producer_launch_sha256: str
    producer_identity_sha256: str
    producer_pid: int
    workload_started_at_unix_ms: int
    workload_started_monotonic_ns: int

    def document(self) -> dict[str, object]:
        """Return the exact launch acknowledgement bound into lease state."""

        value = {
            "soak_anchor_sha256": self.soak_anchor_sha256,
            "anchor_observation_completed_at_unix_ms": (
                self.anchor_observation_completed_at_unix_ms
            ),
            "producer_launch_sha256": self.producer_launch_sha256,
            "producer_identity_sha256": self.producer_identity_sha256,
            "producer_pid": self.producer_pid,
            "workload_started_at_unix_ms": self.workload_started_at_unix_ms,
            "workload_started_monotonic_ns": self.workload_started_monotonic_ns,
        }
        return _validate_launch(value)


@dataclass
class PublicSoakLease:
    """An active, process-local lease whose lock must span the whole run."""

    root: Path
    _root_fd: int
    _lock_fd: int
    _owner_nonce: bytes
    lease_id: str
    binding_sha256: str
    _state_sha256: str
    owner_process_id: int
    started_monotonic_ns: int
    _closed: bool = False

    def close(self) -> None:
        """Release process resources without changing durable state.

        Closing a nonterminal lease deliberately strands it.  No API can resume
        that state; an operator must preserve it as an abandoned, non-completing
        attempt record and choose a new empty root for a later authorized run.
        """

        if self._closed:
            return
        self._closed = True
        try:
            os.close(self._lock_fd)
        finally:
            os.close(self._root_fd)


def _validate_launch(value: object) -> dict[str, object]:
    launch = _exact(value, LAUNCH_FIELDS, "producer launch")
    for field in (
        "soak_anchor_sha256",
        "producer_launch_sha256",
        "producer_identity_sha256",
    ):
        _digest(launch[field], f"producer launch {field}")
    anchor_completed = _integer(
        launch["anchor_observation_completed_at_unix_ms"],
        "anchor observation completion time",
        minimum=1,
    )
    started_wall = _integer(
        launch["workload_started_at_unix_ms"],
        "producer workload start wall time",
        minimum=1,
    )
    _integer(
        launch["workload_started_monotonic_ns"],
        "producer workload start monotonic time",
        minimum=1,
    )
    _integer(launch["producer_pid"], "producer process ID", minimum=1)
    if not 0 <= started_wall - anchor_completed <= MAX_ANCHOR_TO_WORKLOAD_GAP_MS:
        _fail("producer launch is not within the fresh soak-anchor window")
    return launch


def _validate_binding(value: object) -> dict[str, object]:
    binding = _exact(value, BINDING_FIELDS, "lease binding")
    source = _exact(binding["source"], SOURCE_FIELDS, "lease source")
    _commit(source["commit"], "source commit")
    _commit(source["dpn_validator_release_commit"], "DPN validator commit")
    _digest(source["cargo_lock_sha256"], "Cargo.lock digest")
    _digest(source["workspace_source_manifest_sha256"], "source manifest digest")

    prerequisites = _exact(
        binding["prerequisites"], PREREQUISITE_FIELDS, "lease prerequisites"
    )
    for field in sorted(PREREQUISITE_FIELDS):
        _digest(prerequisites[field], field)

    deployment = _exact(
        binding["deployment"], DEPLOYMENT_FIELDS, "lease deployment"
    )
    protocol_version = _integer(
        deployment["protocol_version"], "deployment protocol version", minimum=1
    )
    genesis = _iroha_hash(
        deployment["genesis_block_hash"],
        "genesis block hash",
        "HashOf<BlockHeader>",
    )
    expected_network_id = taira_constants.network_id_from_genesis_hash(
        str(genesis["value"])
    )
    if (
        deployment["network_name"] != taira_constants.NETWORK_NAME
        or deployment["chain_id"] != taira_constants.CHAIN_ID
        or deployment["network_id"] != expected_network_id
        or protocol_version != 4
    ):
        _fail("lease deployment is not the exact public Taira revision-4 network")
    _iroha_hash(
        deployment["deploy_end_block_hash"],
        "deploy end block hash",
        "HashOf<BlockHeader>",
    )
    _integer(deployment["deploy_end_height"], "deploy end height", minimum=1)
    _integer(
        deployment["deployment_completed_at_unix_ms"],
        "deployment completion time",
        minimum=1,
    )

    controller = _exact(
        binding["controller"], CONTROLLER_FIELDS, "lease controller"
    )
    for field in ("host_id", "installation_id", "signing_key_id"):
        _identity(controller[field], f"controller {field}")
    for field in (
        "controller_sha256",
        "native_verifier_binary_sha256",
        "native_verifier_source_sha256",
    ):
        _digest(controller[field], f"controller {field}")
    launch = _validate_launch(binding["launch"])
    deployment_completed = int(deployment["deployment_completed_at_unix_ms"])
    anchor_completed = int(launch["anchor_observation_completed_at_unix_ms"])
    if anchor_completed < deployment_completed:
        _fail("public-soak anchor predates deployment completion")
    return binding


def _validate_profile(value: object) -> dict[str, object]:
    profile = _exact(value, PROFILE_FIELDS, "lease profile")
    for field, expected in PROFILE.items():
        actual = profile[field]
        if isinstance(expected, int):
            _integer(actual, f"profile {field}")
        elif not isinstance(actual, str):
            _fail(f"profile {field} has the wrong type")
        if type(actual) is not type(expected) or actual != expected:
            _fail(f"profile {field} is not the fixed public-soak value")
    return profile


def _state_without_self_hash(state: dict[str, object]) -> dict[str, object]:
    return {field: state[field] for field in TOP_LEVEL_FIELDS - {"state_sha256"}}


def _state_digest(state: dict[str, object]) -> str:
    return _domain_digest(STATE_DOMAIN, _state_without_self_hash(state))


def _validate_state(value: object) -> dict[str, object]:
    state = _exact(value, TOP_LEVEL_FIELDS, "lease state")
    schema_version = _integer(
        state["schema_version"], "lease schema version", minimum=1
    )
    if (
        not isinstance(state["schema"], str)
        or state["schema"] != SCHEMA
        or schema_version != SCHEMA_VERSION
    ):
        _fail("lease state schema is wrong")
    state_sha256 = _digest(state["state_sha256"], "lease state self-digest")
    if state_sha256 != _state_digest(state):
        _fail("lease state self-digest is wrong")
    _digest(state["lease_id"], "lease ID")
    _digest(state["owner_nonce_sha256"], "owner nonce digest")
    _integer(state["owner_process_id"], "lease owner process ID", minimum=1)
    _integer(state["lock_device"], "lease lock device")
    _integer(state["lock_inode"], "lease lock inode", minimum=1)
    binding = _validate_binding(state["binding"])
    binding_sha256 = _digest(state["binding_sha256"], "binding digest")
    if binding_sha256 != _domain_digest(BINDING_DOMAIN, binding):
        _fail("lease binding digest is wrong")
    _validate_profile(state["profile"])

    timing = _exact(state["timing"], TIMING_FIELDS, "lease timing")
    started_wall = _integer(
        timing["workload_started_at_unix_ms"],
        "workload start wall time",
        minimum=1,
    )
    started_monotonic = _integer(
        timing["workload_started_monotonic_ns"],
        "workload start monotonic time",
        minimum=1,
    )
    deployment = binding["deployment"]
    assert isinstance(deployment, dict)
    if started_wall < int(deployment["deployment_completed_at_unix_ms"]):
        _fail("public soak starts before deployment completion")
    launch = binding["launch"]
    assert isinstance(launch, dict)
    if (
        started_wall != launch["workload_started_at_unix_ms"]
        or started_monotonic != launch["workload_started_monotonic_ns"]
    ):
        _fail("lease clocks differ from the producer launch acknowledgement")

    phase = state["phase"]
    if not isinstance(phase, str) or phase not in PHASES:
        _fail("lease phase is invalid")
    artifacts = _exact(state["artifacts"], ARTIFACT_FIELDS, "lease artifacts")
    for field in ARTIFACT_FIELDS:
        _optional_digest(artifacts[field], f"lease artifact {field}")

    history = state["history"]
    if not isinstance(history, list) or not history or len(history) > 4:
        _fail("lease history length is invalid")
    prior_phase: str | None = None
    prior_wall = started_wall
    prior_elapsed = 0
    allowed = {
        None: {PHASE_RUNNING},
        PHASE_RUNNING: {PHASE_CAPTURED, PHASE_FAILED},
        PHASE_CAPTURED: {PHASE_ADMISSION_PENDING, PHASE_FAILED},
        PHASE_ADMISSION_PENDING: {PHASE_COMPLETED, PHASE_FAILED},
    }
    for sequence, raw_event in enumerate(history):
        event = _exact(raw_event, EVENT_FIELDS, f"lease history event {sequence}")
        if _integer(event["sequence"], "lease event sequence") != sequence:
            _fail("lease history is not exact and contiguous")
        event_phase = event["phase"]
        if not isinstance(event_phase, str) or event_phase not in PHASES:
            _fail("lease history phase is invalid")
        if event_phase not in allowed.get(prior_phase, set()):
            _fail("lease history phase transition is invalid")
        wall = _integer(event["recorded_at_unix_ms"], "lease event wall time", minimum=1)
        elapsed = _integer(
            event["elapsed_monotonic_ms"], "lease event monotonic elapsed"
        )
        evidence = _digest(event["evidence_sha256"], "lease event evidence")
        if wall < prior_wall or elapsed < prior_elapsed:
            _fail("lease history clocks are not monotonic")
        wall_elapsed = wall - started_wall
        if abs(wall_elapsed - elapsed) > MAX_WALL_MONOTONIC_SKEW_MS:
            _fail("lease wall and monotonic clocks diverge beyond the fixed bound")
        if sequence == 0 and (
            event_phase != PHASE_RUNNING
            or wall != started_wall
            or elapsed != 0
            or evidence != binding_sha256
        ):
            _fail("initial lease history event is not exact")
        prior_phase = event_phase
        prior_wall = wall
        prior_elapsed = elapsed
    if prior_phase != phase:
        _fail("lease phase differs from its terminal history event")

    capture = artifacts["capture_set_sha256"]
    completion = artifacts["completion_sha256"]
    subject = artifacts["authority_subject_sha256"]
    envelope = artifacts["authority_envelope_sha256"]
    admission = artifacts["admission_receipt_sha256"]
    prior_to_failure: str | None = None
    if phase == PHASE_FAILED:
        if len(history) < 2:
            _fail("failed lease has no preceding live phase")
        prior_event = history[-2]
        assert isinstance(prior_event, dict)
        prior_to_failure = str(prior_event["phase"])
    evidence_phase = prior_to_failure or phase
    if evidence_phase == PHASE_RUNNING and any(
        item is not None for item in (capture, completion, subject, envelope, admission)
    ):
        _fail("running lease contains premature evidence identities")
    if evidence_phase == PHASE_CAPTURED and (
        capture is None
        or any(item is not None for item in (completion, subject, envelope, admission))
    ):
        _fail("captured lease evidence identities are incomplete or premature")
    if evidence_phase == PHASE_ADMISSION_PENDING and (
        any(item is None for item in (capture, completion, subject, envelope))
        or admission is not None
    ):
        _fail("admission-pending lease evidence identities are incomplete")
    if evidence_phase == PHASE_COMPLETED and any(
        item is None for item in (capture, completion, subject, envelope, admission)
    ):
        _fail("completed lease evidence identities are incomplete")

    failure = state["failure"]
    if phase == PHASE_FAILED:
        failed = _exact(failure, FAILURE_FIELDS, "lease failure")
        if not isinstance(failed["code"], str) or failed["code"] not in FAILURE_CODES:
            _fail("lease failure code is invalid")
        _digest(failed["evidence_sha256"], "lease failure evidence")
        last = history[-1]
        assert isinstance(last, dict)
        if last["evidence_sha256"] != failed["evidence_sha256"]:
            _fail("lease failure evidence differs from its history event")
    elif failure is not None:
        _fail("non-failed lease contains failure state")

    captured_events = [
        event for event in history
        if isinstance(event, dict) and event["phase"] == PHASE_CAPTURED
    ]
    if capture is not None:
        if len(captured_events) != 1:
            _fail("lease capture identity has no unique capture event")
        capture_event = captured_events[0]
        assert isinstance(capture_event, dict)
        capture_elapsed = int(capture_event["elapsed_monotonic_ms"])
        if not DURATION_MS <= capture_elapsed <= DURATION_MS + CONFIRMATION_DRAIN_MS:
            _fail("capture did not close in the fixed workload-and-drain window")
        if capture_event["evidence_sha256"] != capture:
            _fail("capture history and artifact identities differ")
    pending_events = [
        event for event in history
        if isinstance(event, dict) and event["phase"] == PHASE_ADMISSION_PENDING
    ]
    if subject is not None:
        if len(pending_events) != 1:
            _fail("lease authority subject has no unique admission event")
        pending_event = pending_events[0]
        assert isinstance(pending_event, dict)
        if pending_event["evidence_sha256"] != subject:
            _fail("admission history and subject identities differ")
    if phase == PHASE_COMPLETED:
        completion_event = history[3]
        assert isinstance(completion_event, dict)
        if completion_event["evidence_sha256"] != admission:
            _fail("completion history and admission identities differ")

    if started_monotonic <= 0:  # Kept explicit for closed-type audit readability.
        _fail("workload start monotonic time must be positive")
    return state


def _canonical_root(path: Path) -> Path:
    if (
        not path.is_absolute()
        or path == Path("/")
        or Path(os.path.abspath(path)) != path
        or os.path.normpath(str(path)) != str(path)
        or any(component in {"", ".", ".."} for component in path.parts[1:])
    ):
        _fail("lease state root must be one canonical absolute path")
    return path


def _open_root_once(path: Path) -> int:
    """Open every absolute path component without following symbolic links."""

    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY | os.O_NOFOLLOW
    descriptor = os.open("/", flags)
    try:
        for component in path.parts[1:]:
            child = os.open(component, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _open_root(path: Path) -> int:
    canonical = _canonical_root(path)
    descriptor = -1
    confirmation = -1
    try:
        descriptor = _open_root_once(canonical)
        confirmation = _open_root_once(canonical)
    except OSError as error:
        if confirmation >= 0:
            os.close(confirmation)
        if descriptor >= 0:
            os.close(descriptor)
        raise PublicSoakLeaseError(
            "lease state root is unavailable or contains a symbolic link"
        ) from error
    metadata = os.fstat(descriptor)
    confirmation_metadata = os.fstat(confirmation)
    os.close(confirmation)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or metadata.st_uid != os.geteuid()
        or (metadata.st_dev, metadata.st_ino)
        != (confirmation_metadata.st_dev, confirmation_metadata.st_ino)
    ):
        os.close(descriptor)
        _fail(
            "lease state root must be one stable owner-private mode-0700 directory"
        )
    return descriptor


def _regular_private(metadata: os.stat_result, label: str) -> None:
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o600
        or metadata.st_uid != os.geteuid()
        or metadata.st_nlink != 1
    ):
        _fail(f"{label} must be one owner-private single-link regular file")


def _open_lock(root_fd: int) -> int:
    descriptor = -1
    try:
        try:
            descriptor = os.open(
                LOCK_FILENAME,
                os.O_RDWR
                | os.O_CREAT
                | os.O_EXCL
                | os.O_CLOEXEC
                | os.O_NOFOLLOW,
                0o600,
                dir_fd=root_fd,
            )
            os.fchmod(descriptor, 0o600)
        except FileExistsError:
            descriptor = os.open(
                LOCK_FILENAME,
                os.O_RDWR | os.O_CLOEXEC | os.O_NOFOLLOW,
                dir_fd=root_fd,
            )
        _regular_private(os.fstat(descriptor), "lease lock")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise PublicSoakLeaseError(
                "another public-soak controller already owns this state root"
            ) from error
        return descriptor
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise


def _read_state(root_fd: int) -> dict[str, object]:
    try:
        descriptor = os.open(
            STATE_FILENAME,
            os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW,
            dir_fd=root_fd,
        )
    except FileNotFoundError as error:
        raise PublicSoakLeaseError("public-soak lease state is absent") from error
    try:
        before = os.fstat(descriptor)
        _regular_private(before, "lease state")
        if before.st_size <= 0 or before.st_size > MAX_STATE_BYTES:
            _fail("lease state size is outside its fixed bound")
        chunks: list[bytes] = []
        remaining = before.st_size
        while remaining:
            chunk = os.read(descriptor, min(remaining, 64 * 1024))
            if not chunk:
                _fail("lease state was truncated while read")
            chunks.append(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail("lease state grew while read")
        after = os.fstat(descriptor)
        if (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_uid,
            before.st_nlink,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
        ) != (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_uid,
            after.st_nlink,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        ):
            _fail("lease state changed while read")
        return _validate_state(_decode_canonical(b"".join(chunks)))
    finally:
        os.close(descriptor)


def _write_all(descriptor: int, payload: bytes) -> None:
    offset = 0
    while offset < len(payload):
        written = os.write(descriptor, payload[offset:])
        if written <= 0:
            _fail("lease state write did not make progress")
        offset += written


def _publish_state(root_fd: int, state: dict[str, object], *, initial: bool) -> None:
    state["state_sha256"] = _state_digest(state)
    _validate_state(state)
    payload = _canonical_json(state)
    if len(payload) > MAX_STATE_BYTES:
        _fail("lease state exceeds its fixed size bound")
    if initial:
        descriptor = os.open(
            STATE_FILENAME,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW,
            0o600,
            dir_fd=root_fd,
        )
        try:
            os.fchmod(descriptor, 0o600)
            _write_all(descriptor, payload)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        os.fsync(root_fd)
        return

    sequence = len(state["history"])
    temporary = f".{STATE_FILENAME}.{state['lease_id']}.{sequence}.tmp"
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW,
        0o600,
        dir_fd=root_fd,
    )
    try:
        os.fchmod(descriptor, 0o600)
        _write_all(descriptor, payload)
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        os.replace(
            temporary,
            STATE_FILENAME,
            src_dir_fd=root_fd,
            dst_dir_fd=root_fd,
        )
        os.fsync(root_fd)
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            os.unlink(temporary, dir_fd=root_fd)
        except FileNotFoundError:
            pass
        raise


def _replace_state(
    lease: PublicSoakLease,
    state: dict[str, object],
) -> None:
    """Replace state only when the handle still owns the exact prior version."""

    _confirm_named_root(lease)
    if _directory_entries(lease._root_fd) != {LOCK_FILENAME, STATE_FILENAME}:
        _fail("public-soak lease root changed before state replacement")
    _confirm_lock_path(lease._root_fd, lease._lock_fd)
    current = _read_state(lease._root_fd)
    if current["state_sha256"] != lease._state_sha256:
        _fail("public-soak lease state changed outside its active owner")
    if current["binding_sha256"] != lease.binding_sha256:
        _fail("public-soak lease binding changed outside its active owner")
    _confirm_lock_path(lease._root_fd, lease._lock_fd)
    _publish_state(lease._root_fd, state, initial=False)
    _confirm_lock_path(lease._root_fd, lease._lock_fd)
    if _directory_entries(lease._root_fd) != {LOCK_FILENAME, STATE_FILENAME}:
        _fail("public-soak lease root changed after state replacement")
    _confirm_named_root(lease)
    lease._state_sha256 = str(state["state_sha256"])


def _directory_entries(root_fd: int) -> set[str]:
    return set(os.listdir(root_fd))


def _sample_clocks() -> tuple[int, int]:
    """Return a bounded wall/monotonic sample without measuring descheduling."""

    for _attempt in range(3):
        monotonic_before = _monotonic_ns()
        wall = _wall_clock_ms()
        monotonic_after = _monotonic_ns()
        if monotonic_after < monotonic_before:
            _fail("public-soak monotonic clock moved backwards while sampled")
        if (
            monotonic_after - monotonic_before
            <= MAX_CLOCK_SAMPLE_WINDOW_MS * 1_000_000
        ):
            return wall, monotonic_before + (
                monotonic_after - monotonic_before
            ) // 2
    _fail("public-soak clocks could not be sampled inside their fixed window")


def start_lease(
    root: Path,
    binding: LeaseBinding,
    launch: ProducerLaunch,
) -> PublicSoakLease:
    """Create and exclusively own one new public-soak state root.

    Existing state is always rejected, even when its lock is no longer held.
    This function intentionally has no resume mode.
    """

    root_fd = _open_root(root)
    lock_fd = -1
    try:
        lock_fd = _open_lock(root_fd)
        if _directory_entries(root_fd) != {LOCK_FILENAME}:
            _fail("new lease state root is not empty and cannot be resumed")
        binding_document = binding.document(launch)
        binding_sha256 = _domain_digest(BINDING_DOMAIN, binding_document)
        owner_nonce = secrets.token_bytes(32)
        if len(owner_nonce) != 32:
            _fail("lease owner nonce source returned the wrong width")
        owner_nonce_sha256 = hashlib.sha256(OWNER_DOMAIN + owner_nonce).hexdigest()
        sampled_wall, sampled_monotonic = _sample_clocks()
        launch_document = binding_document["launch"]
        assert isinstance(launch_document, dict)
        started_wall = int(launch_document["workload_started_at_unix_ms"])
        started_monotonic = int(
            launch_document["workload_started_monotonic_ns"]
        )
        wall_delay = sampled_wall - started_wall
        monotonic_delay = (sampled_monotonic - started_monotonic) // 1_000_000
        if started_wall <= 0 or started_monotonic <= 0:
            _fail("lease clocks did not return positive values")
        if (
            wall_delay < 0
            or monotonic_delay < 0
            or wall_delay > MAX_LEASE_CREATION_DELAY_MS
            or monotonic_delay > MAX_LEASE_CREATION_DELAY_MS
            or abs(wall_delay - monotonic_delay) > MAX_WALL_MONOTONIC_SKEW_MS
        ):
            _fail("producer launch acknowledgement is stale or from another clock epoch")
        owner_process_id = os.getpid()
        lock_metadata = os.fstat(lock_fd)
        lease_id = hashlib.sha256(
            LEASE_ID_DOMAIN
            + owner_nonce
            + binding_sha256.encode("ascii")
            + str(started_wall).encode("ascii")
            + b"\0"
            + str(started_monotonic).encode("ascii")
        ).hexdigest()
        state: dict[str, object] = {
            "schema": SCHEMA,
            "schema_version": SCHEMA_VERSION,
            "lease_id": lease_id,
            "owner_nonce_sha256": owner_nonce_sha256,
            "owner_process_id": owner_process_id,
            "lock_device": lock_metadata.st_dev,
            "lock_inode": lock_metadata.st_ino,
            "binding_sha256": binding_sha256,
            "binding": binding_document,
            "profile": dict(PROFILE),
            "timing": {
                "workload_started_at_unix_ms": started_wall,
                "workload_started_monotonic_ns": started_monotonic,
            },
            "phase": PHASE_RUNNING,
            "artifacts": {field: None for field in ARTIFACT_FIELDS},
            "history": [
                {
                    "sequence": 0,
                    "phase": PHASE_RUNNING,
                    "recorded_at_unix_ms": started_wall,
                    "elapsed_monotonic_ms": 0,
                    "evidence_sha256": binding_sha256,
                }
            ],
            "failure": None,
            "state_sha256": "0" * 64,
        }
        _publish_state(root_fd, state, initial=True)
        lease = PublicSoakLease(
            root=root,
            _root_fd=root_fd,
            _lock_fd=lock_fd,
            _owner_nonce=owner_nonce,
            lease_id=lease_id,
            binding_sha256=binding_sha256,
            _state_sha256=str(state["state_sha256"]),
            owner_process_id=owner_process_id,
            started_monotonic_ns=started_monotonic,
        )
        _confirm_named_root(lease)
        _confirm_lock_path(lease._root_fd, lease._lock_fd)
        return lease
    except BaseException:
        if lock_fd >= 0:
            os.close(lock_fd)
        os.close(root_fd)
        raise


def _confirm_root_path(root: Path, root_fd: int) -> None:
    """Require a canonical pathname to still name one held root directory."""

    confirmation = _open_root(root)
    try:
        held = os.fstat(root_fd)
        named = os.fstat(confirmation)
        if (held.st_dev, held.st_ino) != (named.st_dev, named.st_ino):
            _fail("public-soak lease root pathname was replaced")
    finally:
        os.close(confirmation)


def _confirm_named_root(lease: PublicSoakLease) -> None:
    """Require the canonical pathname to still name the held root directory."""

    _confirm_root_path(lease.root, lease._root_fd)


def _confirm_lock_path(root_fd: int, lock_fd: int) -> os.stat_result:
    """Require the held lock and its current pathname to be the same inode."""

    held = os.fstat(lock_fd)
    _regular_private(held, "lease lock")
    named = os.stat(LOCK_FILENAME, dir_fd=root_fd, follow_symlinks=False)
    if (held.st_dev, held.st_ino) != (named.st_dev, named.st_ino):
        _fail("public-soak lease lock path was replaced")
    return held


def _active_state(lease: PublicSoakLease) -> tuple[dict[str, object], int, int]:
    if lease._closed:
        _fail("public-soak lease handle is closed")
    if os.getpid() != lease.owner_process_id:
        _fail("public-soak lease ownership cannot cross a process boundary")
    try:
        lock_metadata = os.fstat(lease._lock_fd)
        root_metadata = os.fstat(lease._root_fd)
    except OSError as error:
        raise PublicSoakLeaseError("public-soak lease descriptors are invalid") from error
    _regular_private(lock_metadata, "lease lock")
    if (
        not stat.S_ISDIR(root_metadata.st_mode)
        or stat.S_IMODE(root_metadata.st_mode) != 0o700
        or root_metadata.st_uid != os.geteuid()
    ):
        _fail("public-soak lease root changed under its owner")
    if _directory_entries(lease._root_fd) != {LOCK_FILENAME, STATE_FILENAME}:
        _fail("public-soak lease root contains unexpected entries")
    _confirm_lock_path(lease._root_fd, lease._lock_fd)
    _confirm_named_root(lease)
    state = _read_state(lease._root_fd)
    if state["lease_id"] != lease.lease_id:
        _fail("public-soak lease ID changed under its owner")
    expected_owner = hashlib.sha256(OWNER_DOMAIN + lease._owner_nonce).hexdigest()
    if state["owner_nonce_sha256"] != expected_owner:
        _fail("public-soak lease owner identity changed")
    if state["owner_process_id"] != lease.owner_process_id:
        _fail("public-soak lease owner process identity changed")
    if (state["lock_device"], state["lock_inode"]) != (
        lock_metadata.st_dev,
        lock_metadata.st_ino,
    ):
        _fail("public-soak lease state is bound to another lock inode")
    if state["binding_sha256"] != lease.binding_sha256:
        _fail("public-soak lease binding changed outside its active owner")
    if state["state_sha256"] != lease._state_sha256:
        _fail("public-soak lease state changed outside its active owner")
    timing = state["timing"]
    assert isinstance(timing, dict)
    if timing["workload_started_monotonic_ns"] != lease.started_monotonic_ns:
        _fail("public-soak monotonic epoch changed")
    now_wall, now_monotonic = _sample_clocks()
    if now_monotonic < lease.started_monotonic_ns:
        _fail("public-soak monotonic clock moved backwards")
    elapsed = (now_monotonic - lease.started_monotonic_ns) // 1_000_000
    started_wall = int(timing["workload_started_at_unix_ms"])
    if now_wall < started_wall:
        _fail("public-soak wall clock moved backwards")
    if abs((now_wall - started_wall) - elapsed) > MAX_WALL_MONOTONIC_SKEW_MS:
        _fail("public-soak wall and monotonic clocks diverged")
    return state, now_wall, elapsed


def _append_transition(
    lease: PublicSoakLease,
    state: dict[str, object],
    *,
    phase: str,
    evidence_sha256: str,
    now_wall: int,
    elapsed: int,
) -> None:
    history = state["history"]
    assert isinstance(history, list)
    history.append(
        {
            "sequence": len(history),
            "phase": phase,
            "recorded_at_unix_ms": now_wall,
            "elapsed_monotonic_ms": elapsed,
            "evidence_sha256": evidence_sha256,
        }
    )
    state["phase"] = phase
    _confirm_named_root(lease)
    _replace_state(lease, state)
    _confirm_named_root(lease)


def record_capture(lease: PublicSoakLease, capture_set_sha256: str) -> None:
    """Bind the naturally completed workload/capture set within its drain bound."""

    capture_digest = _digest(capture_set_sha256, "capture-set digest")
    state, now_wall, elapsed = _active_state(lease)
    if state["phase"] != PHASE_RUNNING:
        _fail("capture requires the running lease phase")
    if elapsed < DURATION_MS:
        _fail("public-soak capture is premature")
    if elapsed > DURATION_MS + CONFIRMATION_DRAIN_MS:
        _fail("public-soak capture missed the fixed confirmation-drain deadline")
    artifacts = state["artifacts"]
    assert isinstance(artifacts, dict)
    artifacts["capture_set_sha256"] = capture_digest
    _append_transition(
        lease,
        state,
        phase=PHASE_CAPTURED,
        evidence_sha256=capture_digest,
        now_wall=now_wall,
        elapsed=elapsed,
    )


def record_admission_pending(
    lease: PublicSoakLease,
    *,
    completion_sha256: str,
    authority_subject_sha256: str,
    authority_envelope_sha256: str,
) -> None:
    """Bind a closed receipt and fresh authority envelope before broker admission."""

    completion_digest = _digest(completion_sha256, "completion digest")
    subject_digest = _digest(authority_subject_sha256, "authority subject digest")
    envelope_digest = _digest(authority_envelope_sha256, "authority envelope digest")
    state, now_wall, elapsed = _active_state(lease)
    if state["phase"] != PHASE_CAPTURED:
        _fail("admission preparation requires the captured lease phase")
    artifacts = state["artifacts"]
    assert isinstance(artifacts, dict)
    artifacts["completion_sha256"] = completion_digest
    artifacts["authority_subject_sha256"] = subject_digest
    artifacts["authority_envelope_sha256"] = envelope_digest
    _append_transition(
        lease,
        state,
        phase=PHASE_ADMISSION_PENDING,
        evidence_sha256=subject_digest,
        now_wall=now_wall,
        elapsed=elapsed,
    )


def record_completed(lease: PublicSoakLease, admission_receipt_sha256: str) -> None:
    """Bind the independent durable admission receipt and close the lease."""

    admission_digest = _digest(admission_receipt_sha256, "admission receipt digest")
    state, now_wall, elapsed = _active_state(lease)
    if state["phase"] != PHASE_ADMISSION_PENDING:
        _fail("completion requires the admission-pending lease phase")
    artifacts = state["artifacts"]
    assert isinstance(artifacts, dict)
    artifacts["admission_receipt_sha256"] = admission_digest
    _append_transition(
        lease,
        state,
        phase=PHASE_COMPLETED,
        evidence_sha256=admission_digest,
        now_wall=now_wall,
        elapsed=elapsed,
    )
    lease.close()


def record_failed(
    lease: PublicSoakLease,
    *,
    code: str,
    evidence_sha256: str,
) -> None:
    """Atomically terminalize a nonterminal run without claiming completion."""

    if not isinstance(code, str) or code not in FAILURE_CODES:
        _fail("public-soak failure code is not allow-listed")
    failure_digest = _digest(evidence_sha256, "failure evidence digest")
    state, now_wall, elapsed = _active_state(lease)
    if state["phase"] in TERMINAL_PHASES:
        _fail("terminal public-soak lease cannot transition again")
    state["failure"] = {"code": code, "evidence_sha256": failure_digest}
    _append_transition(
        lease,
        state,
        phase=PHASE_FAILED,
        evidence_sha256=failure_digest,
        now_wall=now_wall,
        elapsed=elapsed,
    )
    lease.close()


def _inspect_lease(root: Path, *, require_quiescent: bool) -> dict[str, object]:
    """Read one lease and optionally prove that no live owner holds its lock."""

    root_fd = _open_root(root)
    lock_fd = -1
    locked = False
    try:
        _confirm_root_path(root, root_fd)
        entries = _directory_entries(root_fd)
        if entries != {LOCK_FILENAME, STATE_FILENAME}:
            _fail("lease state root contains unexpected entries")
        lock_fd = os.open(
            LOCK_FILENAME,
            os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW,
            dir_fd=root_fd,
        )
        try:
            lock_metadata = _confirm_lock_path(root_fd, lock_fd)
            if require_quiescent:
                try:
                    fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError as error:
                    raise PublicSoakLeaseError(
                        "public-soak lease is still owned by a live controller"
                    ) from error
                locked = True
                lock_metadata = _confirm_lock_path(root_fd, lock_fd)
            state = _read_state(root_fd)
            if (state["lock_device"], state["lock_inode"]) != (
                lock_metadata.st_dev,
                lock_metadata.st_ino,
            ):
                _fail("public-soak lease state is bound to another lock inode")
            _confirm_lock_path(root_fd, lock_fd)
            _confirm_root_path(root, root_fd)
            if _directory_entries(root_fd) != {LOCK_FILENAME, STATE_FILENAME}:
                _fail("lease state root changed while inspected")
            return state
        finally:
            if locked:
                fcntl.flock(lock_fd, fcntl.LOCK_UN)
            os.close(lock_fd)
    finally:
        os.close(root_fd)


def inspect_lease(root: Path) -> dict[str, object]:
    """Structurally inspect one active or abandoned lease journal.

    This local self-hashed journal is not release evidence.  Only the
    independently authenticated public-soak admission receipt can establish a
    terminal release fact.
    """

    return _inspect_lease(root, require_quiescent=False)


def inspect_quiescent_lease(root: Path) -> dict[str, object]:
    """Inspect a journal only after proving its process-held lock is released."""

    return _inspect_lease(root, require_quiescent=True)
